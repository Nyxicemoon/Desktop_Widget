# DeskHub M4 桌面图标管理 — 设计文档

> 日期：2026-06-16
> 关联 issue：[#1](https://github.com/Nyxicemoon/Desktop_Widget/issues/1)
> 范围：完成 [项目.md](../../../项目.md) 4.2「桌面图标管理」（规格第九章列为难点：Windows 快捷方式和图标读取）。

---

## 一、目标与范围

把桌面快捷方式聚合成一个可分类、可收藏、可一键启动的应用中心，并以**主窗口页面 + 透明桌面 widget** 两种形态呈现。

**本轮做：**
1. **自动扫描**用户桌面 + 公共桌面的 `.lnk`，解析名称/目标/参数。
2. **真实图标提取**（Win32 → PNG → base64），best-effort，失败降级为占位图。
3. **拖拽补充**：把桌面（或任意位置）的 `.lnk`/`.exe` 拖进主窗页或 widget 即可加入。
4. **收藏 + 自定义分类**，按分类/收藏筛选。
5. **一键启动**（`ShellExecuteW`）。
6. **桌面 widget**：收藏应用的透明快捷启动网格。

**非目标（明确排除）：**
- 扫描开始菜单 / `%APPDATA%` 程序（仅桌面 + 拖入）。
- 图标缓存到磁盘（本轮按需实时提取，返回 base64；性能不足再加缓存）。
- 拖拽排序持久化的复杂交互（`sort_order` 字段预留，本轮可不实现拖序）。
- UWP / Store 应用（`.lnk` 指向的 Win32 程序为主）。

---

## 二、关键决策（已与用户确认）

| 决策点 | 选择 |
| -- | -- |
| 图标 | **提取真实图标**（Win32），隔离为 best-effort，失败返回 `None` → UI 占位 |
| 添加方式 | **自动扫描 + 拖拽补充** |
| 分类 | **收藏（星标）+ 自定义分类名** |
| 展示位置 | **主窗口「应用」页 + 透明桌面 widget** |

---

## 三、数据模型（migration v4）

扫描列表每次启动实时生成（桌面会变），故**只持久化**：拖入的自定义应用 + 偏好覆盖层。

```sql
CREATE TABLE custom_apps (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  target     TEXT NOT NULL,            -- 解析后的 exe/文件路径
  args       TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE app_prefs (
  target     TEXT PRIMARY KEY,         -- 小写化的目标路径，关联扫描与自定义两类
  category   TEXT,                     -- 自定义分类名，可空
  favorite   INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**合并规则（`apps_scan` 命令）：**
1. 实时扫描桌面 → `ShortcutRaw { name, lnk_path, target, args }`。
2. 读 `custom_apps` → `{ name, target, args, lnk_path: None }`。
3. 按 `target.to_lowercase()` 去重合并（扫描项优先保留其 `lnk_path`/`name`）。
4. 叠加 `app_prefs`（`category` / `favorite` / `sort_order`），无记录则默认（None/false/0）。
5. 返回 `Vec<AppEntry>`，按 `favorite desc, sort_order asc, name asc` 排序。

> **图标识别键**：用 `launch_path`（扫描项=`lnk_path`，自定义项=`target`）作为 `app_icon` 与 `app_launch` 的入参，保证图标与启动行为一致于「用户看到的那个图标」。

---

## 四、后端结构

### `system/shortcuts.rs`（Win32，难点；非 windows 提供 stub）

```rust
pub struct ShortcutRaw {
    pub name: String,
    pub lnk_path: String,
    pub target: String,
    pub args: Option<String>,
}

pub fn scan() -> AppResult<Vec<ShortcutRaw>>;
/// best-effort：返回 data:image/png;base64,...；任何失败 Ok(None)
pub fn icon_data_url(path: &str) -> AppResult<Option<String>>;
pub fn resolve_dropped(path: &str) -> AppResult<ShortcutRaw>;  // .lnk → 解析；.exe → 直接
pub fn launch(path: &str) -> AppResult<()>;                    // ShellExecuteW("open")
```

- **扫描目录**：`%USERPROFILE%\Desktop`、`%PUBLIC%\Desktop`（即 `C:\Users\Public\Desktop`）。
- **解析 `.lnk`**：`CoCreateInstance(ShellLink)` → `IPersistFile::Load` → `IShellLinkW::GetPath/GetArguments`；`name` = 文件名去 `.lnk`。
- **图标**：`SHGetFileInfoW(path, SHGFI_ICON | SHGFI_LARGEICON)` → `HICON`；`GetIconInfo` → 颜色位图；`GetDIBits` 取 32-bit BGRA → 转 RGBA → `image` crate PNG 编码 → base64。失败任一步 → `Ok(None)`，并 `DestroyIcon`/释放 GDI 资源。
- **拖入解析**：`.lnk` 走解析；否则视为可执行/文件，`name` = 文件名，`target` = 原路径。
- **启动**：`ShellExecuteW(None, "open", path, None, None, SW_SHOWNORMAL)`。

COM 线程：命令在 Tauri 线程池执行，每次调用前 `CoInitializeEx(COINIT_APARTMENTTHREADED)`（忽略已初始化错误），用完不强制 `CoUninitialize`（保持简单，进程级）。

### `db/apps.rs`

```rust
pub fn list_custom(conn) -> AppResult<Vec<(String,String,Option<String>)>>; // name,target,args
pub fn add_custom(conn, name, target, args) -> AppResult<()>;               // 按 target 去重(INSERT OR IGNORE 思路)
pub fn remove_custom(conn, target) -> AppResult<()>;
pub fn prefs_map(conn) -> AppResult<HashMap<String,(Option<String>,bool,i64)>>; // target -> (category,favorite,sort_order)
pub fn set_favorite(conn, target, favorite) -> AppResult<()>;               // upsert app_prefs
pub fn set_category(conn, target, category: Option<&str>) -> AppResult<()>; // upsert app_prefs
```

### `commands/apps.rs`

```rust
apps_scan(db) -> Vec<AppEntry>
app_icon(path: String) -> Option<String>
app_launch(path: String) -> ()
app_add_dropped(db, path: String) -> AppEntry      // resolve + add_custom + 返回合并后的条目
app_remove_custom(db, target: String) -> ()
app_set_favorite(db, target: String, favorite: bool) -> ()
app_set_category(db, target: String, category: Option<String>) -> ()
```

### models

```rust
#[derive(Serialize)]
pub struct AppEntry {
    pub name: String,
    pub launch_path: String,   // lnk_path（扫描）或 target（自定义）
    pub target: String,        // 偏好/去重键
    pub args: Option<String>,
    pub is_custom: bool,
    pub category: Option<String>,
    pub favorite: bool,
}
```

---

## 五、桌面 widget

- `window::widget_config` 增加分支：`"apps" => ("widget-apps", "/widgets/apps", 320.0, 220.0, 40.0, 420.0)`。
- `models::WidgetVisibility` 增加 `apps: bool`；`window::read_visibility` 读 kv `widget.apps.visible`。
- `lib.rs` setup 恢复 widget 时增加 `if vis.apps { open_widget("apps") }`。
- 托盘菜单增加「应用 widget」开关项（复用 `set_widget_visible("apps", _)`）。
- 现有 `widget_set_visible(kind)` 命令已是泛型字符串，无需改动。
- widget 内容：`/widgets/apps` 透明卡片，展示**收藏**应用图标网格，点击 `app_launch`；支持把图标拖入（`tauri://drag-drop` → `app_add_dropped`，并自动标记 favorite=true 以出现在 widget）。

---

## 六、前端

- **`routes/(app)/apps/+page.svelte`**：
  - `onMount` → `apps_scan`，渲染网格（图标懒加载：每条 `app_icon(launch_path)`，失败用首字母占位）。
  - 每条：星标切换（`app_set_favorite`）、分类输入/选择（`app_set_category`）、点击启动（`app_launch`）、自定义项可移除（`app_remove_custom`）。
  - 顶部筛选：全部 / 收藏 / 按分类。
  - 整页监听 `tauri://drag-drop` → 对每个拖入路径 `app_add_dropped` → 刷新列表。
- **`routes/(widget)/widgets/apps/+page.svelte`**：透明收藏网格，点击启动，监听拖入。
- 导航加「应用 / Apps」。
- `lib/api/index.ts` 增加封装；`lib/stores/apps.ts`（可选）。

拖放接收用 `@tauri-apps/api/webview` 的 `getCurrentWebview().onDragDropEvent(cb)`（drop 时拿 `event.payload.paths`）。

---

## 七、风险与降级

- **图标提取是规格难点**：隔离在 `icon_data_url` 返回 `Option`，任何 Win32/GDI 失败 → `Ok(None)` → UI 首字母占位。扫描/启动/分类/收藏/拖入**均不依赖图标**，故图标失败不影响核心功能。
- **widget 拖放**：`WS_EX_NOACTIVATE` + bottommost 下拖放为已知不确定点；若 widget 内拖放在实测中不可用，主窗「应用」页拖放仍可用，可作为降级（spec 验收以主窗页拖放为准，widget 拖放为加分项）。
- **COM/GDI 资源**：每次提取后必须 `DestroyIcon` 与释放位图，避免句柄泄漏。

---

## 八、测试策略

Win32 代码依赖真实 shell，难做纯单元测试；策略：
- **可单测**（rusqlite，in-memory）：`db/apps.rs` 的 `add_custom` 去重、`prefs_map`、`set_favorite`/`set_category` upsert、合并排序逻辑（把合并函数抽成纯函数 `merge(scanned, custom, prefs) -> Vec<AppEntry>` 并单测）。
- **migration**：v4 两表建表 + 幂等（扩展既有 migration 测试的表清单）。
- **window**：`widget_config("apps")`、`read_visibility` 含 apps 的断言。
- **Win32**（`scan`/`icon_data_url`/`launch`）：不写单测，靠 `cargo build` 通过 + 用户手动验证。
- **门禁**：`cargo test`、`cargo clippy -- -D warnings`、`npm run check` 全绿。

---

## 九、依赖

- 新增 crate：`image = { version = "0.25", default-features = false, features = ["png"] }`（HICON→PNG 编码）。
- `windows` crate 增加 features：`Win32_System_Com`、`Win32_UI_Shell`、`Win32_UI_Shell_Common`、`Win32_Storage_FileSystem`、`Win32_Graphics_Gdi`（`Win32_Foundation`/`Win32_UI_WindowsAndMessaging` 已有）。
- 前端无新增 npm 包（drag-drop 用 `@tauri-apps/api` 内置）。

---

## 十、文件清单（预计改动）

**后端**
- `Cargo.toml`：+ `image`；windows features 扩展
- `src/db/migrations.rs`：+ migration (4)
- `src/db/apps.rs`（新）+ `src/db/mod.rs`：挂载
- `src/system/shortcuts.rs`（新）+ `src/system/mod.rs`：挂载（`pub mod shortcuts;`）
- `src/commands/apps.rs`（新）+ `src/commands/mod.rs`：挂载
- `src/models/mod.rs`：+ `AppEntry`，`WidgetVisibility` + `apps`
- `src/window/mod.rs`：`widget_config` + apps，`read_visibility` + apps
- `src/tray.rs`：菜单 + apps widget 开关
- `src/lib.rs`：注册命令、setup 恢复 apps widget

**前端**
- `routes/(app)/apps/+page.svelte`（新）
- `routes/(widget)/widgets/apps/+page.svelte`（新）
- `routes/(app)/+layout.svelte`：导航 + Apps
- `lib/api/index.ts`：封装

---

## 十一、修订（2026-06-16）：widget 即管理面板

用户明确：图标管理**完全在桌面半透明 widget 内**完成，不要主窗口页。以下**取代**前文相应部分：

**形态**：widget 是一个可拖动移动、**右下角可缩放**的半透明桌面面板；尺寸/位置经 `window-state` 持久化。`window::open_widget` 对 `apps` 设 `resizable(true)`；前端右下角缩放手柄调用 `getCurrentWindow().startResizeDragging("SouthEast")`。

**内容（策展式，取代自动扫描）**：widget 初始为空，仅显示用户**拖入**的应用（`.lnk`/`.exe`）。**不再自动扫描桌面**。

**两种模式**：
- 普通：点击图标 = 启动。
- 编辑（角上 ✎ 切换）：每项显示 ✕ 移除、支持**拖拽排序**、双击名称**重命名**。

**数据模型简化（重写 migration v4，未合并未运行，直接改）**：
```sql
CREATE TABLE custom_apps (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  target     TEXT NOT NULL,
  args       TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```
**删除 `app_prefs` 表**（收藏/分类不再需要）。

**后端调整**：
- `system/shortcuts.rs`：**移除 `scan` 与 `desktop_dirs`**；保留 `parse_lnk`/`resolve_dropped`/`launch`/`icon_data_url`。
- `models::AppEntry` 简化为 `{ id: i64, name: String, target: String, args: Option<String> }`（启动与取图标均用 `target`）。
- `db/apps.rs`：`list`(按 sort_order) / `add`(去重+下一个 sort_order) / `remove`(按 id) / `rename`(id,name) / `reorder`(ids 按序写 sort_order)；移除 `merge`/`prefs_map`/`set_favorite`/`set_category`。
- `commands/apps.rs`：`app_list` / `app_add_dropped` / `app_remove(id)` / `app_rename(id,name)` / `app_reorder(ids)` / `app_icon(path)` / `app_launch(path)`；移除 scan/favorite/category 命令。

**删除**：`routes/(app)/apps/+page.svelte` 与 `+layout.svelte` 的「应用」导航链接（主窗口不再承载图标管理）。

**保留**：托盘「应用 widget」开关、`WidgetVisibility.apps`、widget 显隐持久化。

> 详见实施计划 [m4-widget-rework](../plans/2026-06-16-deskhub-m4-widget-rework.md)。

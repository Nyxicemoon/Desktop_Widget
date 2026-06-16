# DeskHub M3 收尾 — 设计文档

> 日期：2026-06-16
> 范围：完成 M3「系统集成与打包」剩余项，使 DeskHub 成为可分发、可后台常驻的 MVP。
> 前置：托盘 + 关闭到托盘 + 测试通知已完成（见 [m3-tray-notify spec](2026-06-16-deskhub-m3-tray-notify-design.md)）。

---

## 一、目标与范围

本轮完成 [开发计划.md](../../../开发计划.md) M3 的剩余条目：

1. **开机自启**（`tauri-plugin-autostart`）
2. **数据备份**：SQLite 导出 / 导入
3. **任务到期提醒**（到期当天早上发本地通知）
4. **打包**：Windows NSIS 安装器
5. **内存优化**：本轮降范围为「核查项」，不做重度改造（见第七节）
6. **设置页**：承载自启开关与备份入口

**非目标（明确排除）：**
- 精确到时刻的提醒（todo 目前仅有日期）
- MSI / 跨用户安装（本轮仅 NSIS 用户级）
- 激进的内存/渲染优化（如需另起一轮）
- 任何新建表 / migration（仅复用 kv）

---

## 二、关键决策（已与用户确认）

| 决策点 | 选择 |
| -- | -- |
| 安装器格式 | **NSIS (.exe)**，`installMode: currentUser`（免管理员） |
| 开机自启默认 | **首次运行默认开启**（隐藏到托盘），之后尊重用户设置 |
| 到期提醒时机 | **到期当天早上**首次检查时提醒一次；过期未提醒的补发一次 |
| 数据导入策略 | **覆盖并提示重启**（暂存文件 + 启动时应用，避开 Windows 文件锁） |

---

## 三、开机自启

### 依赖与注册
- 引入 `tauri-plugin-autostart`。
- 注册时传入参数 `--hidden`，使自启进程能被识别：
  ```rust
  .plugin(tauri_plugin_autostart::init(
      tauri_plugin_autostart::MacosLauncher::LaunchAgent,
      Some(vec!["--hidden"]),
  ))
  ```
  Windows 下走 `HKCU\...\Run` 注册表项（用户级，与 NSIS currentUser 一致）。

### 首次默认开启
- `setup()` 中检查 kv `autostart.initialized`：
  - 未置位 → 调用 `app.autolaunch().enable()`，写 kv `autostart.initialized = "1"`。
  - 已置位 → 不动，尊重用户后续的开/关。

### 命令
- `autostart_get() -> bool`：`app.autolaunch().is_enabled()`。
- `autostart_set(enabled: bool)`：`enable()` / `disable()`。
- 统一返回 `AppResult<T>`，前端 `lib/api` 封装。

### 隐藏到托盘启动
- `tauri.conf.json` 主窗口设 `"visible": false`。
- `setup()` 中检测启动参数：
  - `std::env::args()` **不含** `--hidden` → `main_window.show()`（手动启动正常弹窗）。
  - 含 `--hidden` → 保持隐藏，仅托盘驻留（自启静默）。
- 托盘「显示主窗口」菜单与左键点击仍可随时唤出主窗口（已有逻辑）。

---

## 四、数据备份：导出 / 导入

### 依赖
- 引入 `tauri-plugin-dialog`（文件保存/打开对话框）。
- `capabilities/default.json` 增加 `dialog:default` 权限。

### 导出 `db_export(dest_path: String)`
- 用 SQLite `VACUUM INTO ?1` 生成一致、紧凑的副本：
  ```rust
  conn.execute("VACUUM INTO ?1", [dest_path])?;
  ```
- 优于裸文件拷贝：自动处理 WAL、产出单一干净文件、无脏读。
- 前端：dialog 选保存路径（默认名 `deskhub-backup-YYYYMMDD.db`）→ 调命令 → toast 成功。

### 导入 `db_import(src_path: String)`
- **校验**：以只读方式打开 `src_path`，确认是合法 SQLite 且含本应用 schema：
  ```sql
  SELECT name FROM sqlite_master WHERE type='table' AND name='kv';
  ```
  无 `kv` 表 → 返回 `AppError`（提示「不是有效的 DeskHub 备份」）。
- **不直接覆盖在用库**（Windows 下在用文件被连接锁定，覆盖会失败）。
- 校验通过 → 复制到暂存文件 `<app_data>/deskhub.db.import`。
- 返回成功，前端提示「请重启应用以应用导入 / Restart to apply」。

### 启动时应用导入
- `db::open()` 在打开连接**之前**检查暂存文件：
  - 若 `deskhub.db.import` 存在：
    1. 删除 `deskhub.db`、`deskhub.db-wal`、`deskhub.db-shm`（存在则删）。
    2. 将 `deskhub.db.import` 重命名为 `deskhub.db`。
    3. 删除暂存文件（重命名后已不存在，容错处理）。
  - 然后正常 `open` + 迁移（迁移框架幂等，导入的旧版本库会被补齐到当前 `user_version`）。

---

## 五、任务到期提醒

### 触发
- `setup()` 启动后 `tauri::async_runtime::spawn` 一个循环任务：
  - 立即检查一次。
  - 之后每 **60 分钟** `sleep` 后再检查（轻量，空闲无忙轮询）。

### 检查逻辑 `db::todos::list_due_unnotified` + 发送
- SQL 选出到期且未完成的任务：
  ```sql
  SELECT id, title FROM todos
  WHERE done = 0 AND due_date IS NOT NULL
    AND date(due_date) <= date('now','localtime');
  ```
- 对每条：读 kv `reminder:notified:<id>`，未置位则：
  - 通过 `tauri-plugin-notification` 发送：标题「任务到期 / Task due」，正文 = todo 标题。
  - 置 kv `reminder:notified:<id> = "1"`，防止重复提醒。
- 「到期当天早上」语义：若到期当天应用未运行，则当天/之后首次检查时补发（一次）。

### 边界
- 已完成任务不提醒（`done=0` 过滤）。
- 重新创建的任务 id 不同，互不影响。
- 通知权限沿用已接入的 notification 插件。

---

## 六、打包（NSIS）

`tauri.conf.json` `bundle` 配置：
- `"active": true`，`"targets": ["nsis"]`。
- `productName`、`version`、`identifier`（已有 `com.deskhub.app`）、`publisher`、`icon`（已有 icons）。
- `windows.nsis.installMode: "currentUser"`（用户级，免管理员，与自启一致）。
- 产出：`npm run tauri build` → `src-tauri/target/release/bundle/nsis/*.exe`。

**验收**：在干净 Windows 上安装运行、托盘常驻、（默认）开机自启生效。
（构建为重操作；实现阶段至少完成 bundle 配置并尝试一次 `tauri build`，若耗时过长以配置正确为准并记录。）

---

## 七、内存优化（降范围）

- Tauri（vs Electron）本身低占用；当前架构无空闲忙轮询，到期提醒用 60 分钟 sleep 而非紧循环。
- 本轮**不做**重度渲染/内存改造，仅作为核查项：确认空闲占用合理、无新增轮询热点。
- 如后续需要：可加「后台时降低 widget 刷新」「按需懒加载视图」，另起一轮。

---

## 八、设置页

- 新增路由 `src/routes/(app)/settings/+page.svelte`：
  - **开机自启**开关：`onMount` 读 `autostart_get`，切换调 `autostart_set`。
  - **数据备份**：「导出备份」「导入备份」按钮（走 dialog），导入后显示重启提示。
  - 文案中英双语。
- `(app)/+layout.svelte` 导航增加「设置 / Settings」链接。

---

## 九、数据层影响

- **无新建表、无 migration**。
- 仅新增 kv 键：`autostart.initialized`、`reminder:notified:<id>`。
- 新增 db 访问函数：`todos::list_due_unnotified`（或在命令层直接查询，归入 `db/todos.rs` 保持 SQL 隔离）。

---

## 十、测试策略

- **Rust 单测**：
  - `db_import` 校验逻辑：合法库通过、非库/缺 `kv` 表被拒（用临时文件）。
  - 导出 `VACUUM INTO` 产出文件存在且可重新打开。
  - 启动时暂存文件应用：构造暂存文件 → `open` 后正库被替换、暂存消失。
  - 到期查询：构造到期/未到期/已完成 todo，断言只返回到期未完成项。
- **门禁**：`cargo test`、`cargo clippy -D warnings`、`npm run check` 全绿。
- **手动验证**（交用户）：自启注册项、托盘静默启动、导出/导入往返、到期通知、安装器安装运行。

---

## 十一、文件清单（预计改动）

**后端（src-tauri/）**
- `Cargo.toml`：+ `tauri-plugin-autostart`、`tauri-plugin-dialog`
- `src/lib.rs`：注册两插件、autostart 首次默认、`--hidden` 控制主窗口显隐、spawn 到期提醒循环、注册新命令
- `src/commands/autostart.rs`（新）：`autostart_get` / `autostart_set`
- `src/commands/backup.rs`（新）：`db_export` / `db_import`
- `src/commands/mod.rs`：挂载
- `src/db/mod.rs`：`open()` 启动时应用暂存导入文件
- `src/db/backup.rs`（新）或并入 mod：导出/导入校验逻辑
- `src/db/todos.rs`：`list_due_unnotified`
- `src/reminder.rs`（新）：到期检查 + 发通知 + kv 去重
- `tauri.conf.json`：主窗口 `visible:false`、bundle NSIS 配置
- `capabilities/default.json`：+ `dialog:default`

**前端（src/）**
- `routes/(app)/settings/+page.svelte`（新）
- `routes/(app)/+layout.svelte`：导航 + Settings
- `lib/api/index.ts`：`autostartGet/Set`、`dbExport/dbImport` 封装

# DeskHub M2 — 背景图片管理 设计 (Spec)

> 日期：2026-06-16
> 范围：[开发计划.md](../../../开发计划.md) 的 **M2 — 背景图片管理**（对应 [项目.md](../../../项目.md) 4.4 与第十章）
> 前置：M0/M1 已完成（rusqlite + 迁移框架、`AppError`、命令模式、前端 api/stores/路由）。

---

## 一、目标与验收

**目标：** 用 Pexels 在线搜自然风景图，下载到本地并设为**应用窗口背景**，强制记录来源（licensing）。

**验收标准：**
1. 在设置里填入 Pexels API key（保存到本地，不入库、不进 git）。
2. 输入关键词（或点预置词）→ 显示缩略图网格。
3. 点某张 → 下载到本地 → 设为应用背景（窗口出现该图，文字仍可读）。
4. `backgrounds.source_url` 完整记录来源（spec §10）。
5. 重启后背景保持；「恢复默认」可清除背景回到主题底色。
6. `cargo test` 全过；`cargo clippy -D warnings` 干净；`npm run check` 0 errors。

**关键决策（已确认）：**
- 图片 API = **Pexels**。
- API key 由**应用内设置输入框**填写，存 `app_data_dir/config.json`（明文、不入库、不进 git）。
- 「应用背景」= **应用窗口自身的 CSS 背景**，不是 Windows 桌面壁纸（M2 不用 windows-rs）。

---

## 二、数据流

```
设置：用户粘贴 key → config.json（app_data_dir）
搜索：关键词 → 后端 reqwest(blocking) 调 Pexels（带 key）→ 缩略图列表 → 前端网格
选用：点图 → 后端下载原图到 app_data_dir/backgrounds/{id}.jpg → 写 backgrounds 行(source_url/author/license/keyword) → set_current
显示：前端取 current → 后端读文件 → base64 data URL 回传 → 设为背景层
```

- **key 永不进前端**：搜索/下载都在后端，前端只发关键词、收结果。
- **图片以 base64 data URL 回传**：避开 Tauri asset 协议的权限配置；单张当前背景，开销可接受。

---

## 三、数据模型（迁移 0003）

```sql
CREATE TABLE backgrounds (
  id         INTEGER PRIMARY KEY,
  local_path TEXT NOT NULL,
  source_url TEXT NOT NULL,            -- Pexels 页面链接，licensing 必需(§10)
  author     TEXT,
  license    TEXT,                      -- 'Pexels License'
  keyword    TEXT,
  is_current INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
```

`is_current` 唯一：`set_current(id)` 在事务内先 `UPDATE backgrounds SET is_current=0`，再把选中行置 1。

---

## 四、后端模块

- `config.rs`（新建，非 DB）：`AppConfig { pexels_api_key: Option<String> }`，`load(dir)/save(dir,cfg)` 读写 `dir/config.json`（`dir` 入参便于用临时目录测试）。
- `pexels.rs`（新建）：
  - `parse_search_response(json: &str) -> AppResult<Vec<PhotoResult>>`（**纯函数、可单测**）。
  - `search(query, key) -> AppResult<Vec<PhotoResult>>`、`download(url, dest) -> AppResult<()>`（网络，调纯函数）。
- `db/backgrounds.rs`（新建）：`insert(...)->id`、`set_current(&mut Connection, id)`、`get_current(&Connection)->Option<Background>`、`restore_default(&Connection)`、`list(&Connection)->Vec<Background>`。
- `error.rs`（修改）：新增 `AppError::Network(String)`（pexels 网络错误用），更新 `kind()`。
- `models`（修改）：`PhotoResult`、`Background`、`CurrentBackground`。
- `db/migrations.rs`（修改）：新增迁移 `(3, ...)`。
- 新依赖：`reqwest = { version = "0.12", features = ["blocking","json"] }`（Windows 走 schannel，不引 OpenSSL）、`base64 = "0.22"`。

**模型字段：**
- `PhotoResult`（`Serialize + Deserialize`，既回前端也作命令入参）：`id:i64, source_url, author, author_url, thumb_url, download_url, alt`。
- `Background`（`Serialize`）：`id, local_path, source_url, author:Option, license:Option, keyword:Option, is_current:bool, created_at`。
- `CurrentBackground`（`Serialize`）：`data_url, source_url, author:Option`。

**Pexels 字段映射（parse_search_response）：** `photo.id→id`、`photo.url→source_url`、`photo.photographer→author`、`photo.photographer_url→author_url`、`photo.src.medium→thumb_url`、`photo.src.large2x→download_url`、`photo.alt→alt`。

---

## 五、命令（均 `AppResult<T>`）

- `config_has_key(app) -> bool`
- `config_set_pexels_key(app, key: String) -> ()`
- `bg_search(app, keyword: String) -> Vec<PhotoResult>`（无 key → `AppError::Other("Pexels API key not set")`）
- `bg_download_and_set(app, db, photo: PhotoResult, keyword: String) -> ()`
- `bg_get_current(app, db) -> Option<CurrentBackground>`
- `bg_restore_default(db) -> ()`

命令通过 `app: AppHandle` 解析 `app_data_dir`；DB 操作经 `State<Db>`。

---

## 六、前端

- `lib/api`：新增 `configHasKey / configSetPexelsKey / bgSearch / bgDownloadAndSet / bgGetCurrent / bgRestoreDefault` 与类型 `PhotoResult`、`CurrentBackground`。
- `lib/stores/background.ts`：`currentBg` store；`loadBackground()` 调 `bgGetCurrent` 并应用；`applyBackground(dataUrl|null)`。
- 导航：把顶部栏（金币 + 主题 + 导航链接「待办 / 背景」）上移到 `routes/+layout.svelte`，`routes/+page.svelte`（待办）去掉自带 header。
- `routes/backgrounds/+page.svelte`：Pexels key 设置框（`configHasKey` 决定提示/已设）；关键词输入 + 预置词（森林/雪山/湖泊/海边/星空）；结果缩略图网格，点图 → `bgDownloadAndSet` → 重新 `loadBackground`；「恢复默认」按钮。
- 背景层：`+layout.svelte` 渲染固定全屏 `.bg-layer`（`background-image` 来自 current data URL），其 `::after` 用 `var(--bg)` 半透明遮罩保证文字可读；无当前背景时显示主题底色。

---

## 七、测试与门禁

**Rust 单测（内存库 / 临时目录，TDD）：**
- `pexels::parse_search_response`：解析样例 JSON → 字段映射正确；空 photos → 空 Vec；非法 JSON → Err。
- `db/backgrounds`：`insert` 后 `get_current` 初始 None（未设当前）；`set_current` 后 `get_current` 返回该行且唯一（再 `set_current` 另一张，旧的 `is_current` 归 0）；`restore_default` 后 `get_current` 为 None。
- `config`：`load` 不存在文件 → 默认（key None）；`save` 后 `load` 往返拿回 key。

**网络部分（`pexels::search/download`）：** 需真实 key，留**手动验收**。

**门禁：** `cargo test`、`cargo clippy -- -D warnings`、`npm run check`。
**手动验收：** 设 key → 搜「雪山」→ 点图设背景 → 重启保持 → 恢复默认。

---

## 八、不在 M2 范围（YAGNI）

随机背景、每日自动更换、收藏、删除本地图（表字段已支撑，后续里程碑）；Windows 桌面壁纸；图片缓存/磁盘清理；多来源（Unsplash）。

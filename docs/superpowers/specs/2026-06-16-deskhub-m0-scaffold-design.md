# DeskHub M0 工程脚手架 — 设计 (Spec)

> 日期：2026-06-16
> 范围：[开发计划.md](../../../开发计划.md) 的 **M0 — 工程脚手架**
> 前置已完成：Tauri v2 + SvelteKit + Svelte 5 + TS 脚手架已生成；工具链（rustup + MSVC Build Tools + WebView2）已就位；git 仓库已初始化。
> 关键决策：SQLite 访问层 = **rusqlite**（见 [CLAUDE.md](../../../CLAUDE.md)）。

---

## 一、目标与验收

**目标：** 一个能 `npm run tauri dev` 启动的应用，具备数据库迁移框架、前后端通信约定（统一错误）、最小持久化能力与基础质量门禁。

**验收标准：**
1. 应用可启动（`npm run tauri dev`）。
2. **主题持久化**：切到 dark → 关闭应用 → 重开仍是 dark（端到端打通 前端 store → invoke → AppResult → rusqlite → `app_data_dir` → 重启回读）。
3. `cargo test` 通过：迁移幂等、`kv` 往返、覆盖写更新 `updated_at`。
4. 质量门禁可运行：`cargo test`、`cargo clippy`、`npm run check`。

**M0 范围裁剪（YAGNI）：** 不做 todos/金币（M1）、不做托盘/自启（M3）、不引入 toast/prettier/eslint 额外配置（沿用脚手架默认）、不引入 Rust 时间库（时间戳用 SQLite `datetime('now')`）。

---

## 二、架构与模块边界

后端按职责拆分，**所有 SQL 只存在于 `db/`**，以保证未来 rusqlite → sqlx 的迁移是单模块内的受控改写。

```
src-tauri/src/
├── main.rs              # 入口（脚手架已有，不改）
├── lib.rs              # 组装：setup → 打开DB+迁移 → manage(state) → 注册 commands
├── error.rs            # AppError + AppResult<T>（统一错误约定）
├── db/
│   ├── mod.rs          # 连接管理：Db(Mutex<Connection>)、open()、app_data_dir 解析、迁移调用
│   ├── migrations.rs   # 迁移框架（PRAGMA user_version 驱动）+ 迁移清单
│   └── kv.rs           # kv 表读写（M0 唯一数据访问）
├── models/mod.rs       # 共享数据结构（M0 仅占位）
├── commands/
│   ├── mod.rs          # 命令注册聚合
│   └── kv.rs           # kv_get / kv_set 命令（薄封装，转调 db::kv）
└── system/mod.rs       # 占位：M3 托盘/自启接入点（M0 为空模块 + 文档注释）
```

前端：
```
src/lib/
├── api/index.ts        # invoke 的类型化封装 + 统一错误处理
└── stores/theme.ts     # 主题 store（'light' | 'dark'），变更时写回 kv
```

每个单元的职责：
- `error.rs`：定义错误类型，提供 `From` 转换，使命令体内可用 `?`。
- `db/mod.rs`：拥有 DB 连接的生命周期；对外暴露 `open(app) -> AppResult<Connection>` 与状态类型 `Db`。
- `db/migrations.rs`：拥有 schema 版本演进；对外暴露 `apply(&mut Connection) -> AppResult<()>`。
- `db/kv.rs`：键值读写；对外 `get(&Connection, key)`、`set(&Connection, key, value)`。
- `commands/*`：唯一被前端调用的边界；只做参数转发与错误传播，不写 SQL。

---

## 三、数据层

- **DB 路径：** `app_data_dir()/deskhub.db`（用户级，便于备份）。首启自动创建目录与库文件。
- **rusqlite：** 依赖 `rusqlite = { version = "0.32", features = ["bundled"] }`，自带 SQLite，不依赖系统库（确保干净 Windows 可运行）。
- **连接管理：** `pub struct Db(pub Mutex<Connection>);`，通过 Tauri `app.manage(Db(...))` 注入；命令经 `State<Db>` 获取，加锁后使用。
- **迁移框架：** 基于 SQLite 内置 `PRAGMA user_version`：
  - 维护 `const MIGRATIONS: &[(i32, &str)]`（版本号, SQL）。
  - 启动读 `user_version`；对每条 `version > current` 的迁移，在**一个事务**内执行 SQL 并 `PRAGMA user_version = version`。
  - 幂等：重复启动不会重复应用已生效的迁移。
- **迁移 0001：**
  ```sql
  CREATE TABLE kv (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
  ```
  覆盖写（`set` 已存在的 key）需更新 `updated_at`：用 `INSERT ... ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')`。

---

## 四、错误约定

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")] Database(String),
    #[error("io error: {0}")]       Io(String),
    #[error("not found: {0}")]      NotFound(String),
    #[error("{0}")]                 Other(String),
}
pub type AppResult<T> = Result<T, AppError>;
```

- 为返回前端，给 `AppError` 实现 `serde::Serialize`，序列化为 `{ "kind": <variant 名>, "message": <Display 文本> }`，前端可据 `kind` 分支处理。
- 实现 `From<rusqlite::Error>`（→ `Database`）与 `From<std::io::Error>`（→ `Io`），命令体内用 `?` 传播。
- 新增依赖：`thiserror = "2"`。

---

## 五、命令约定

- 所有 Tauri 命令签名返回 `AppResult<T>`；命名规则 `模块_动作`。
- M0 命令：
  - `kv_set(db: State<Db>, key: String, value: String) -> AppResult<()>`
  - `kv_get(db: State<Db>, key: String) -> AppResult<Option<String>>`
- 前端 `api/index.ts`：封装 `invoke`，统一 `try/catch`；M0 阶段错误处理为 `console.error` + 重新抛出（toast 留到 M1）。导出类型化函数 `kvGet(key): Promise<string | null>`、`kvSet(key, value): Promise<void>`。

---

## 六、前端骨架

- `stores/theme.ts`：
  - `writable<'light' | 'dark'>`。
  - 初始化：首次读 `kvGet('theme')`；为空则跟随系统 `window.matchMedia('(prefers-color-scheme: dark)')`。
  - 订阅变更：`kvSet('theme', value)` 写回，并设置 `document.documentElement.dataset.theme = value`。
- CSS：在全局样式用 CSS 变量定义 light/dark 两套色板，由 `[data-theme="dark"]` 切换（替换脚手架 `+page.svelte` 里硬编码的 `@media (prefers-color-scheme: dark)`）。
- `+page.svelte`：移除 demo（greet/logos），替换为「标题 + 主题切换按钮」，按钮调用 theme store。

---

## 七、测试与质量门禁

- **Rust 集成测试**（`db/` 逻辑，针对 `Connection::open_in_memory()`）：
  1. 迁移可在空库应用；重复 `apply` 幂等（`user_version` 不回退、不重复建表报错）。
  2. `kv::set` 后 `kv::get` 返回写入值；`get` 不存在的 key 返回 `None`。
  3. 对同 key 覆盖写：`value` 更新且 `updated_at` 变化。
- **门禁命令：** `cargo test`、`cargo clippy`（`src-tauri/` 内）、`npm run check`（svelte-check）。
- **手动验收：** `npm run tauri dev` → 切主题 → 重启保持。

---

## 八、依赖增量

- 后端 `Cargo.toml`：`rusqlite = { version = "0.32", features = ["bundled"] }`、`thiserror = "2"`。（`serde`/`serde_json` 脚手架已有。）
- 前端：无新增（用 `@tauri-apps/api` 已有的 `invoke`）。

---

## 九、不在本 spec 范围（后续里程碑）

- todos / game_profile / coin_ledger 表与金币闭环 → M1。
- 系统托盘、开机自启、本地通知、安装包 → M3。
- toast 错误提示、prettier/eslint、CI → 视需要在后续里程碑引入。

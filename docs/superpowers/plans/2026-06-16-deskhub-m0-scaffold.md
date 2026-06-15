# DeskHub M0 工程脚手架 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在已生成的 Tauri v2 + SvelteKit 脚手架上，建立后端模块结构、rusqlite 迁移框架、统一错误约定与最小持久化（kv），并以「主题持久化」打通端到端。

**Architecture:** 后端按职责分层，所有 SQL 仅存在于 `db/`（保证 rusqlite→sqlx 可迁移）；命令层薄封装并统一返回 `AppResult<T>`；前端用类型化 `invoke` 封装 + 主题 store，主题选择写入 kv 表，重启后回读。

**Tech Stack:** Rust + Tauri v2 + rusqlite(bundled) + thiserror + serde；前端 SvelteKit + Svelte 5 + TypeScript。

> **依赖说明 / Note:** 后端逻辑用 TDD（`Connection::open_in_memory()` 单元测试）。前端无测试运行器（M0 不引入 vitest，YAGNI），用 `npm run check`（svelte-check）+ 手动验收。
> **编译前置 / Prerequisite:** `tauri::generate_context!` 要求 `frontendDist`（`../build`）存在。任何 `cargo build/test/clippy` 之前必须先 `npm run build` 生成 `build/`。

---

## 文件结构 / File Structure

后端（`src-tauri/src/`）：
- `error.rs`（新建）— `AppError` 枚举 + `AppResult<T>` + `Serialize` + `From` 转换。
- `db/mod.rs`（新建）— `Db(Mutex<Connection>)` 状态、`open(app)` 解析 `app_data_dir` 并迁移。
- `db/migrations.rs`（新建）— `PRAGMA user_version` 驱动的迁移框架 + 迁移清单（0001 建 kv）。
- `db/kv.rs`（新建）— `get`/`set` 键值读写（唯一数据访问）。
- `models/mod.rs`（新建）— 占位（M1 填充）。
- `system/mod.rs`（新建）— 占位（M3 填充）。
- `commands/mod.rs`（新建）— 命令聚合。
- `commands/kv.rs`（新建）— `kv_get`/`kv_set` 命令。
- `lib.rs`（修改）— 注册模块、setup 打开 DB、manage 状态、注册命令。
- `Cargo.toml`（修改）— 增加 rusqlite、thiserror。

前端（`src/`）：
- `lib/api/index.ts`（新建）— 类型化 invoke 封装。
- `lib/stores/theme.ts`（新建）— 主题 store。
- `app.css`（新建）— light/dark CSS 变量。
- `routes/+layout.svelte`（新建）— 引入 app.css。
- `routes/+page.svelte`（修改）— 替换 demo 为标题 + 主题切换。

---

## Task 1: 后端依赖 + AppError 错误约定

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/error.rs`

- [ ] **Step 1: 增加依赖**

修改 `src-tauri/Cargo.toml` 的 `[dependencies]`，在 `serde_json = "1"` 之后追加：

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
thiserror = "2"
```

- [ ] **Step 2: 写失败测试（错误序列化）**

创建 `src-tauri/src/error.rs`：

```rust
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Database(_) => "Database",
            AppError::Io(_) => "Io",
            AppError::NotFound(_) => "NotFound",
            AppError::Other(_) => "Other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let e = AppError::NotFound("widget".into());
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"NotFound","message":"not found: widget"}"#
        );
    }
}
```

在 `src-tauri/src/lib.rs` 顶部加入模块声明（仅本任务所需这一行）：

```rust
mod error;
```

- [ ] **Step 3: 运行测试确认失败/未编译**

先生成前端 dist（编译前置），再测试：

Run: `npm run build` 然后在 `src-tauri/` 内 `cargo test error::`
Expected: 编译通过、测试 PASS（若 `mod error;` 未加会编译失败 → 加上即过）。

> 注：`error` 模块此时虽未被其他代码使用，会有 dead_code 警告，属正常，后续任务会用到。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat(m0): add AppError error convention"
```

---

## Task 2: 迁移框架（PRAGMA user_version）

**Files:**
- Create: `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/src/lib.rs`（加 `mod db;`）
- Create: `src-tauri/src/db/mod.rs`（先仅声明子模块）

- [ ] **Step 1: 建 db 模块骨架**

创建 `src-tauri/src/db/mod.rs`：

```rust
pub mod kv;
pub mod migrations;
```

> 暂时会因 `kv` 不存在而编译失败 —— 本任务先建 `migrations`，Task 3 建 `kv`。为避免中途无法编译，本步同时创建一个空的 `src-tauri/src/db/kv.rs`：

创建 `src-tauri/src/db/kv.rs`（占位，Task 3 实现）：

```rust
// Implemented in Task 3.
```

在 `src-tauri/src/lib.rs` 顶部加：

```rust
mod db;
```

- [ ] **Step 2: 写失败测试（迁移幂等）**

创建 `src-tauri/src/db/migrations.rs`：

```rust
use crate::error::AppResult;
use rusqlite::Connection;

const MIGRATIONS: &[(i32, &str)] = &[(
    1,
    "CREATE TABLE kv (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
)];

pub fn apply(conn: &mut Connection) -> AppResult<()> {
    let current: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (version, sql) in MIGRATIONS {
        if *version > current {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", *version)?;
            tx.commit()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn version(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn applies_migrations_on_empty_db() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        assert_eq!(version(&conn), 1);
        let table_count: i32 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='kv'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn apply_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        apply(&mut conn).unwrap(); // 不应报 "table already exists"
        assert_eq!(version(&conn), 1);
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `src-tauri/` 内 `cargo test migrations::`
Expected: 两个测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/ src-tauri/src/lib.rs
git commit -m "feat(m0): add user_version migration framework + kv migration 0001"
```

---

## Task 3: kv 读写

**Files:**
- Modify: `src-tauri/src/db/kv.rs`（替换 Task 2 的占位）

- [ ] **Step 1: 写失败测试 + 实现**

把 `src-tauri/src/db/kv.rs` 整个替换为：

```rust
use crate::error::AppResult;
use rusqlite::{Connection, OptionalExtension};

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        (key, value),
    )?;
    Ok(())
}

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row("SELECT value FROM kv WHERE key = ?1", [key], |row| row.get(0))
        .optional()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        conn
    }

    #[test]
    fn set_then_get_returns_value() {
        let conn = setup();
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme").unwrap(), Some("dark".to_string()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let conn = setup();
        assert_eq!(get(&conn, "missing").unwrap(), None);
    }

    #[test]
    fn set_overwrites_value_and_keeps_single_row() {
        let conn = setup();
        set(&conn, "theme", "light").unwrap();
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme").unwrap(), Some("dark".to_string()));
        let row_count: i32 = conn
            .query_row("SELECT count(*) FROM kv", [], |r| r.get(0))
            .unwrap();
        assert_eq!(row_count, 1);
        let updated_at: String = conn
            .query_row("SELECT updated_at FROM kv WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert!(!updated_at.is_empty());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `src-tauri/` 内 `cargo test kv::`
Expected: 三个测试 PASS。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/db/kv.rs
git commit -m "feat(m0): add kv get/set with upsert"
```

---

## Task 4: DB 连接模块（app_data_dir + 状态）

**Files:**
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 实现连接打开与状态**

把 `src-tauri/src/db/mod.rs` 整个替换为：

```rust
pub mod kv;
pub mod migrations;

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Tauri-managed database state. Guard the connection with the mutex before use.
pub struct Db(pub Mutex<Connection>);

/// Open (creating if needed) the SQLite database at `app_data_dir/deskhub.db`
/// and run pending migrations.
pub fn open(app: &AppHandle) -> AppResult<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    let mut conn = Connection::open(dir.join("deskhub.db"))?;
    migrations::apply(&mut conn)?;
    Ok(conn)
}
```

> 此模块依赖 Tauri 运行时上下文，不做单元测试；由 Task 9 的手动验收覆盖（启动后 DB 文件生成、可读写）。

- [ ] **Step 2: 编译检查**

Run: `src-tauri/` 内 `cargo build`
Expected: 编译通过（`open` 未被调用会有 dead_code 警告，Task 6 接入后消失）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/db/mod.rs
git commit -m "feat(m0): add Db state and app_data_dir connection opener"
```

---

## Task 5: 命令层 + 占位模块

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/kv.rs`
- Create: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/system/mod.rs`

- [ ] **Step 1: 命令实现**

创建 `src-tauri/src/commands/mod.rs`：

```rust
pub mod kv;
```

创建 `src-tauri/src/commands/kv.rs`：

```rust
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use tauri::State;

#[tauri::command]
pub fn kv_set(db: State<Db>, key: String, value: String) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::kv::set(&conn, &key, &value)
}

#[tauri::command]
pub fn kv_get(db: State<Db>, key: String) -> AppResult<Option<String>> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::kv::get(&conn, &key)
}
```

创建 `src-tauri/src/models/mod.rs`：

```rust
//! Shared data structures. Populated in M1 (todos, game_profile, coin_ledger).
```

创建 `src-tauri/src/system/mod.rs`：

```rust
//! System integration (tray, autostart, notifications). Wired up in M3.
```

- [ ] **Step 2: 编译检查（先不接 lib）**

在 `src-tauri/src/lib.rs` 顶部补齐模块声明：

```rust
mod commands;
mod models;
mod system;
```

Run: `src-tauri/` 内 `cargo build`
Expected: 编译通过（命令暂未注册，dead_code 警告正常）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/models/ src-tauri/src/system/ src-tauri/src/lib.rs
git commit -m "feat(m0): add kv commands and placeholder modules"
```

---

## Task 6: 组装 lib.rs + 后端门禁

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 完成 lib.rs**

把 `src-tauri/src/lib.rs` 整个替换为：

```rust
mod commands;
mod db;
mod error;
mod models;
mod system;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let conn = db::open(app.handle())?;
            app.manage(db::Db(std::sync::Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::kv::kv_get,
            commands::kv::kv_set
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> 删除了脚手架的 `greet` 命令（M0 不需要）。`setup` 闭包错误类型为 `Box<dyn Error>`，`AppError` 实现了 `std::error::Error`，`?` 可自动装箱。

- [ ] **Step 2: 跑全部后端测试 + lint**

Run（确保 `build/` 已存在；如无先 `npm run build`）:
```
# 项目根
npm run build
# src-tauri/
cargo test
cargo clippy -- -D warnings
```
Expected: `cargo test` 全 PASS；`cargo clippy` 无警告（dead_code 此时应已消除，因 `open`/命令均被引用）。

> 若 clippy 报个别非关键 lint，按提示修正后重跑直至通过。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(m0): wire DB state, migrations, and kv commands into app"
```

---

## Task 7: 前端 API 封装

**Files:**
- Create: `src/lib/api/index.ts`

- [ ] **Step 1: 实现类型化 invoke 封装**

创建 `src/lib/api/index.ts`：

```ts
import { invoke } from "@tauri-apps/api/core";

export interface AppErrorShape {
  kind: string;
  message: string;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`command ${cmd} failed:`, err);
    throw err;
  }
}

export function kvGet(key: string): Promise<string | null> {
  return call<string | null>("kv_get", { key });
}

export function kvSet(key: string, value: string): Promise<void> {
  return call<void>("kv_set", { key, value });
}
```

- [ ] **Step 2: 类型检查**

Run: 项目根 `npm run check`
Expected: 无类型错误（svelte-check 0 errors）。

- [ ] **Step 3: 提交**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m0): add typed invoke api wrapper"
```

---

## Task 8: 主题 store + 样式 + 页面

**Files:**
- Create: `src/lib/stores/theme.ts`
- Create: `src/app.css`
- Create: `src/routes/+layout.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: 主题 store**

创建 `src/lib/stores/theme.ts`：

```ts
import { get, writable } from "svelte/store";
import { kvGet, kvSet } from "$lib/api";

export type Theme = "light" | "dark";

function systemTheme(): Theme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function apply(value: Theme): void {
  document.documentElement.dataset.theme = value;
}

export const theme = writable<Theme>("light");

/** Load persisted theme (or system default) and apply it. Call once on mount. */
export async function initTheme(): Promise<void> {
  const saved = await kvGet("theme");
  const value: Theme = saved === "dark" || saved === "light" ? saved : systemTheme();
  apply(value);
  theme.set(value);
}

export function setTheme(value: Theme): void {
  apply(value);
  theme.set(value);
  void kvSet("theme", value);
}

export function toggleTheme(): void {
  setTheme(get(theme) === "dark" ? "light" : "dark");
}
```

- [ ] **Step 2: 全局样式**

创建 `src/app.css`：

```css
:root {
  --bg: #f6f6f6;
  --fg: #0f0f0f;
  --surface: #ffffff;
  --border: #d0d0d0;
}

:root[data-theme="dark"] {
  --bg: #2f2f2f;
  --fg: #f6f6f6;
  --surface: #1a1a1a;
  --border: #444444;
}

html,
body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  transition: background 0.2s, color 0.2s;
}
```

- [ ] **Step 3: 布局引入样式**

创建 `src/routes/+layout.svelte`：

```svelte
<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

{@render children()}
```

- [ ] **Step 4: 页面替换为标题 + 切换按钮**

把 `src/routes/+page.svelte` 整个替换为：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";

  onMount(() => {
    void initTheme();
  });
</script>

<main class="container">
  <h1>DeskHub</h1>
  <p>当前主题 / Theme: {$theme}</p>
  <button onclick={toggleTheme}>切换主题 / Toggle theme</button>
</main>

<style>
  .container {
    padding: 10vh 1rem;
    text-align: center;
  }

  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.6em 1.2em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }
</style>
```

- [ ] **Step 5: 类型检查**

Run: 项目根 `npm run check`
Expected: 0 errors。

- [ ] **Step 6: 提交**

```bash
git add src/lib/stores/theme.ts src/app.css src/routes/+layout.svelte src/routes/+page.svelte
git commit -m "feat(m0): add theme store, css variables, and theme toggle UI"
```

---

## Task 9: 端到端验收

**Files:** 无（验证 + 文档勾选）

- [ ] **Step 1: 启动应用**

Run: 项目根 `npm run tauri dev`
Expected: 编译后弹出 DeskHub 窗口，显示标题、当前主题、切换按钮。

- [ ] **Step 2: 验证持久化（核心验收）**

操作：点击「切换主题」切到 dark → 完全关闭应用窗口 → 再次 `npm run tauri dev`。
Expected: 重新打开后主题仍为 dark（说明已从 `app_data_dir/deskhub.db` 的 kv 表回读）。

- [ ] **Step 3: 验证 DB 文件落地**

确认存在文件：`%APPDATA%\com.deskhub.app\deskhub.db`（或 Tauri 解析的 app_data_dir）。
Expected: 文件存在。

- [ ] **Step 4: 勾选开发计划 M0**

在 `开发计划.md` 的 M0 小节，把已完成项的 `- [ ]` 改为 `- [x]`（脚手架、目录结构、SQLite+迁移、command 约定、DB 路径、主题骨架；质量门禁中 cargo test/clippy/svelte-check 已落地，prettier/eslint 按 spec 暂缓——在该行后补注 `（prettier/eslint 暂缓，见 spec）`）。

- [ ] **Step 5: 提交**

```bash
git add 开发计划.md
git commit -m "docs(m0): mark M0 scaffold milestone complete"
```

---

## 自检 / Self-Review

- **Spec 覆盖：** 模块边界(Task1-6) / DB路径+rusqlite+迁移(Task2,4) / AppError(Task1) / 命令约定(Task5,6) / 前端骨架(Task7,8) / 验收+测试(Task2,3,6,9) / 门禁(Task6,7,8,9) —— 均有对应任务。
- **无占位符：** 所有代码步骤含完整代码；`npm run build` 前置在 Task1/6/9 明确。
- **类型一致：** `Db(Mutex<Connection>)`、`AppResult<T>`、`db::kv::get/set`、`kvGet/kvSet`、`Theme`、`initTheme/setTheme/toggleTheme` 在各任务间命名一致。

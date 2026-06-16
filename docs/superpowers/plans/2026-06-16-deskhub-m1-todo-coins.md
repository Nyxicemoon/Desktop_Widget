# DeskHub M1 — Todo + 金币系统 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Todo 增删改/完成 + 完成发金币的闭环，金币赚到即保留、反复勾选不刷币，今日列表含全部未完成 + 今天完成。

**Architecture:** 复用 M0 的 `db/`(rusqlite + `user_version` 迁移) 与命令模式；新增迁移 0002 建 `todos`/`game_profile`/`coin_ledger`；金币经济（去重发奖 + 余额 + 流水）聚合在 `db/game.rs`，任务逻辑在 `db/todos.rs`，`toggle_done` 在单事务内完成状态切换与发奖。

**Tech Stack:** Rust + Tauri v2 + rusqlite(bundled) + serde；前端 SvelteKit + Svelte 5 + TypeScript。

> **编译前置：** 任何 `cargo` 命令前确保 `build/` 存在（M0 已生成；若清理过先 `npm run build`）。
> **Tauri 参数命名：** JS 用 camelCase，Tauri v2 自动转 Rust snake_case（如 JS `dueDate` → Rust `due_date`）。
> **cargo 路径：** 本机用 `"$USERPROFILE/.cargo/bin/cargo.exe" --manifest-path src-tauri/Cargo.toml`。

---

## 文件结构 / File Structure

后端（`src-tauri/src/`）：
- `db/migrations.rs`（修改）— 增加迁移 `(2, ...)`；更新版本测试为版本无关。
- `models/mod.rs`（替换）— `Todo`、`GameProfile`、`ToggleResult`。
- `db/game.rs`（新建）— 金币经济：`ensure_profile`/`get_profile`/`award_for_todo` + 测试。
- `db/todos.rs`（新建）— 任务 CRUD + `list_today` + `toggle_done` + 测试。
- `db/mod.rs`（修改）— `pub mod game; pub mod todos;`
- `commands/todos.rs`（新建）、`commands/game.rs`（新建）、`commands/mod.rs`（修改）。
- `lib.rs`（修改）— 注册新命令。

前端（`src/`）：
- `lib/api/index.ts`（修改）— 类型 + `todo*`/`game*` 封装。
- `lib/stores/todos.ts`（新建）、`lib/stores/game.ts`（新建）。
- `routes/+page.svelte`（替换）— 今日任务 UI + 金币 + 主题。

---

## Task 1: 迁移 0002（三张表）

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: 增加迁移并改为版本无关的测试**

把 `src-tauri/src/db/migrations.rs` 的 `MIGRATIONS` 常量替换为（保留迁移 1，新增迁移 2）：

```rust
const MIGRATIONS: &[(i32, &str)] = &[
    (
        1,
        "CREATE TABLE kv (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ),
    (
        2,
        "CREATE TABLE todos (
            id          INTEGER PRIMARY KEY,
            title       TEXT NOT NULL,
            note        TEXT,
            done        INTEGER NOT NULL DEFAULT 0,
            due_date    TEXT,
            reward_coin INTEGER NOT NULL DEFAULT 10,
            created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            done_at     TEXT
        );
        CREATE TABLE game_profile (
            id        INTEGER PRIMARY KEY CHECK (id = 1),
            coins     INTEGER NOT NULL DEFAULT 0,
            exp       INTEGER NOT NULL DEFAULT 0,
            level     INTEGER NOT NULL DEFAULT 1,
            last_tick TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE coin_ledger (
            id         INTEGER PRIMARY KEY,
            amount     INTEGER NOT NULL,
            reason     TEXT NOT NULL,
            ref_id     INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    ),
];
```

然后把该文件 `#[cfg(test)] mod tests` 里的 `applies_migrations_on_empty_db` 测试替换为版本无关版本（其余测试不动）：

```rust
    #[test]
    fn applies_migrations_on_empty_db() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        let latest = MIGRATIONS.last().unwrap().0;
        assert_eq!(version(&conn), latest);
        for t in ["kv", "todos", "game_profile", "coin_ledger"] {
            let c: i32 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(c, 1, "table {t} missing");
        }
    }
```

- [ ] **Step 2: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml migrations::`
Expected: `applies_migrations_on_empty_db` 和 `apply_is_idempotent` 均 PASS（版本到 2、四张表存在）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/db/migrations.rs
git commit -m "feat(m1): add migration 0002 (todos, game_profile, coin_ledger)"
```

---

## Task 2: 数据模型

**Files:**
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: 定义结构体**

把 `src-tauri/src/models/mod.rs` 整个替换为：

```rust
//! Shared data structures.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub note: Option<String>,
    pub done: bool,
    pub due_date: Option<String>,
    pub reward_coin: i64,
    pub created_at: String,
    pub done_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GameProfile {
    pub coins: i64,
    pub exp: i64,
    pub level: i64,
    pub last_tick: String,
}

#[derive(Debug, Serialize)]
pub struct ToggleResult {
    pub todo: Todo,
    pub awarded: i64,
    pub coins: i64,
}
```

- [ ] **Step 2: 编译检查**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（结构体暂未使用，dead_code 警告正常）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/models/mod.rs
git commit -m "feat(m1): add Todo, GameProfile, ToggleResult models"
```

---

## Task 3: 金币经济 `db/game.rs`

**Files:**
- Create: `src-tauri/src/db/game.rs`
- Modify: `src-tauri/src/db/mod.rs`（加 `pub mod game;`）

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/db/mod.rs` 顶部的 `pub mod kv;` 之后加一行：

```rust
pub mod game;
```

- [ ] **Step 2: 实现 + 测试**

创建 `src-tauri/src/db/game.rs`：

```rust
use crate::error::AppResult;
use crate::models::GameProfile;
use rusqlite::Connection;

pub fn ensure_profile(conn: &Connection) -> AppResult<()> {
    conn.execute("INSERT OR IGNORE INTO game_profile (id) VALUES (1)", [])?;
    Ok(())
}

pub fn get_profile(conn: &Connection) -> AppResult<GameProfile> {
    ensure_profile(conn)?;
    let profile = conn.query_row(
        "SELECT coins, exp, level, last_tick FROM game_profile WHERE id = 1",
        [],
        |r| {
            Ok(GameProfile {
                coins: r.get(0)?,
                exp: r.get(1)?,
                level: r.get(2)?,
                last_tick: r.get(3)?,
            })
        },
    )?;
    Ok(profile)
}

/// Award coins for completing a todo, deduplicated by ledger `ref_id`.
/// Returns the amount actually awarded (0 if this todo was already rewarded).
/// Call inside the caller's transaction.
pub fn award_for_todo(conn: &Connection, todo_id: i64, amount: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let already: i64 = conn.query_row(
        "SELECT count(*) FROM coin_ledger WHERE reason = 'todo_done' AND ref_id = ?1",
        [todo_id],
        |r| r.get(0),
    )?;
    if already > 0 {
        return Ok(0);
    }
    conn.execute(
        "INSERT INTO coin_ledger (amount, reason, ref_id) VALUES (?1, 'todo_done', ?2)",
        (amount, todo_id),
    )?;
    conn.execute(
        "UPDATE game_profile SET coins = coins + ?1 WHERE id = 1",
        [amount],
    )?;
    Ok(amount)
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
    fn ensure_profile_is_idempotent_and_starts_at_zero() {
        let conn = setup();
        ensure_profile(&conn).unwrap();
        ensure_profile(&conn).unwrap();
        let p = get_profile(&conn).unwrap();
        assert_eq!(p.coins, 0);
        assert_eq!(p.level, 1);
    }

    #[test]
    fn award_adds_once_and_dedups_by_ref_id() {
        let conn = setup();
        assert_eq!(award_for_todo(&conn, 42, 10).unwrap(), 10);
        assert_eq!(get_profile(&conn).unwrap().coins, 10);
        // same todo again -> no double reward
        assert_eq!(award_for_todo(&conn, 42, 10).unwrap(), 0);
        assert_eq!(get_profile(&conn).unwrap().coins, 10);
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml game::`
Expected: 两个测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/game.rs src-tauri/src/db/mod.rs
git commit -m "feat(m1): add coin economy (profile + dedup award + ledger)"
```

---

## Task 4: 任务逻辑 `db/todos.rs`

**Files:**
- Create: `src-tauri/src/db/todos.rs`
- Modify: `src-tauri/src/db/mod.rs`（加 `pub mod todos;`）

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/db/mod.rs` 的 `pub mod game;` 之后加一行：

```rust
pub mod todos;
```

- [ ] **Step 2: 实现 + 测试**

创建 `src-tauri/src/db/todos.rs`：

```rust
use crate::db::game;
use crate::error::{AppError, AppResult};
use crate::models::{Todo, ToggleResult};
use rusqlite::{Connection, OptionalExtension, Row};

const COLS: &str =
    "id, title, note, done, due_date, reward_coin, created_at, done_at";

fn row_to_todo(row: &Row) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: row.get("id")?,
        title: row.get("title")?,
        note: row.get("note")?,
        done: row.get("done")?,
        due_date: row.get("due_date")?,
        reward_coin: row.get("reward_coin")?,
        created_at: row.get("created_at")?,
        done_at: row.get("done_at")?,
    })
}

fn get_by_id(conn: &Connection, id: i64) -> AppResult<Todo> {
    let sql = format!("SELECT {COLS} FROM todos WHERE id = ?1");
    conn.query_row(&sql, [id], row_to_todo)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("todo {id}")))
}

pub fn create(
    conn: &Connection,
    title: &str,
    note: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<Todo> {
    conn.execute(
        "INSERT INTO todos (title, note, due_date) VALUES (?1, ?2, ?3)",
        (title, note, due_date),
    )?;
    get_by_id(conn, conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    note: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<Todo> {
    let n = conn.execute(
        "UPDATE todos SET title = ?1, note = ?2, due_date = ?3 WHERE id = ?4",
        (title, note, due_date, id),
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("todo {id}")));
    }
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM todos WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("todo {id}")));
    }
    Ok(())
}

pub fn list_today(conn: &Connection) -> AppResult<Vec<Todo>> {
    let sql = format!(
        "SELECT {COLS} FROM todos
         WHERE done = 0 OR (done = 1 AND date(done_at) = date('now','localtime'))
         ORDER BY done ASC, created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_todo)?;
    let mut todos = Vec::new();
    for r in rows {
        todos.push(r?);
    }
    Ok(todos)
}

pub fn toggle_done(conn: &mut Connection, id: i64) -> AppResult<ToggleResult> {
    let tx = conn.transaction()?;
    let current = get_by_id(&tx, id)?;
    let awarded = if !current.done {
        tx.execute(
            "UPDATE todos SET done = 1, done_at = datetime('now','localtime') WHERE id = ?1",
            [id],
        )?;
        game::award_for_todo(&tx, id, current.reward_coin)?
    } else {
        tx.execute(
            "UPDATE todos SET done = 0, done_at = NULL WHERE id = ?1",
            [id],
        )?;
        0
    };
    let todo = get_by_id(&tx, id)?;
    let coins: i64 =
        tx.query_row("SELECT coins FROM game_profile WHERE id = 1", [], |r| r.get(0))?;
    tx.commit()?;
    Ok(ToggleResult { todo, awarded, coins })
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
    fn create_update_delete() {
        let conn = setup();
        let t = create(&conn, "task", None, None).unwrap();
        assert_eq!(t.title, "task");
        assert!(!t.done);

        let u = update(&conn, t.id, "renamed", Some("memo"), Some("2026-06-16")).unwrap();
        assert_eq!(u.title, "renamed");
        assert_eq!(u.note.as_deref(), Some("memo"));

        delete(&conn, t.id).unwrap();
        assert!(get_by_id(&conn, t.id).is_err());
    }

    #[test]
    fn list_today_includes_incomplete_and_today_done_excludes_old_done() {
        let conn = setup();
        create(&conn, "open", None, None).unwrap();
        // a task completed yesterday should NOT appear
        conn.execute(
            "INSERT INTO todos (title, done, done_at)
             VALUES ('old', 1, datetime('now','localtime','-1 day'))",
            [],
        )
        .unwrap();
        let list = list_today(&conn).unwrap();
        let titles: Vec<&str> = list.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"open"));
        assert!(!titles.contains(&"old"));
    }

    #[test]
    fn toggle_awards_once_no_refund_no_reaward() {
        let mut conn = setup();
        let t = create(&conn, "task", None, None).unwrap();

        // complete -> award 10
        let r1 = toggle_done(&mut conn, t.id).unwrap();
        assert!(r1.todo.done);
        assert_eq!(r1.awarded, 10);
        assert_eq!(r1.coins, 10);

        // un-complete -> no refund
        let r2 = toggle_done(&mut conn, t.id).unwrap();
        assert!(!r2.todo.done);
        assert_eq!(r2.awarded, 0);
        assert_eq!(r2.coins, 10);

        // re-complete -> no re-award
        let r3 = toggle_done(&mut conn, t.id).unwrap();
        assert!(r3.todo.done);
        assert_eq!(r3.awarded, 0);
        assert_eq!(r3.coins, 10);
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml todos::`
Expected: 三个测试 PASS。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/todos.rs src-tauri/src/db/mod.rs
git commit -m "feat(m1): add todo CRUD, list_today, and transactional toggle_done"
```

---

## Task 5: 命令层 + 注册 + 后端门禁

**Files:**
- Create: `src-tauri/src/commands/todos.rs`
- Create: `src-tauri/src/commands/game.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 命令实现**

创建 `src-tauri/src/commands/todos.rs`：

```rust
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::models::{Todo, ToggleResult};
use tauri::State;

#[tauri::command]
pub fn todo_create(
    db: State<Db>,
    title: String,
    note: Option<String>,
    due_date: Option<String>,
) -> AppResult<Todo> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::create(&conn, &title, note.as_deref(), due_date.as_deref())
}

#[tauri::command]
pub fn todo_update(
    db: State<Db>,
    id: i64,
    title: String,
    note: Option<String>,
    due_date: Option<String>,
) -> AppResult<Todo> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::update(&conn, id, &title, note.as_deref(), due_date.as_deref())
}

#[tauri::command]
pub fn todo_delete(db: State<Db>, id: i64) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::delete(&conn, id)
}

#[tauri::command]
pub fn todo_list_today(db: State<Db>) -> AppResult<Vec<Todo>> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::list_today(&conn)
}

#[tauri::command]
pub fn todo_toggle_done(db: State<Db>, id: i64) -> AppResult<ToggleResult> {
    let mut conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::toggle_done(&mut conn, id)
}
```

创建 `src-tauri/src/commands/game.rs`：

```rust
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::models::GameProfile;
use tauri::State;

#[tauri::command]
pub fn game_get_profile(db: State<Db>) -> AppResult<GameProfile> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::game::get_profile(&conn)
}
```

把 `src-tauri/src/commands/mod.rs` 整个替换为：

```rust
pub mod game;
pub mod kv;
pub mod todos;
```

- [ ] **Step 2: 注册命令**

把 `src-tauri/src/lib.rs` 的 `invoke_handler(...)` 调用整段替换为：

```rust
        .invoke_handler(tauri::generate_handler![
            commands::kv::kv_get,
            commands::kv::kv_set,
            commands::todos::todo_create,
            commands::todos::todo_update,
            commands::todos::todo_delete,
            commands::todos::todo_list_today,
            commands::todos::todo_toggle_done,
            commands::game::game_get_profile
        ])
```

- [ ] **Step 3: 全量测试 + lint**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
"$USERPROFILE/.cargo/bin/cargo.exe" clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```
Expected: 所有测试 PASS（M0 的 7 + M1 的 7 = 14）；clippy 无警告。

> 若 clippy 报 `get_by_id`/`update` 等"未使用"：它们已被命令引用，应无此问题。其余非关键 lint 按提示修正后重跑。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs
git commit -m "feat(m1): add todo/game commands and register them"
```

---

## Task 6: 前端 API 封装

**Files:**
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: 增加类型与封装**

在 `src/lib/api/index.ts` 末尾（`kvSet` 之后）追加：

```ts
export interface Todo {
  id: number;
  title: string;
  note: string | null;
  done: boolean;
  due_date: string | null;
  reward_coin: number;
  created_at: string;
  done_at: string | null;
}

export interface GameProfile {
  coins: number;
  exp: number;
  level: number;
  last_tick: string;
}

export interface ToggleResult {
  todo: Todo;
  awarded: number;
  coins: number;
}

export function todoCreate(
  title: string,
  note: string | null = null,
  dueDate: string | null = null,
): Promise<Todo> {
  return call<Todo>("todo_create", { title, note, dueDate });
}

export function todoUpdate(
  id: number,
  title: string,
  note: string | null = null,
  dueDate: string | null = null,
): Promise<Todo> {
  return call<Todo>("todo_update", { id, title, note, dueDate });
}

export function todoDelete(id: number): Promise<void> {
  return call<void>("todo_delete", { id });
}

export function todoListToday(): Promise<Todo[]> {
  return call<Todo[]>("todo_list_today");
}

export function todoToggleDone(id: number): Promise<ToggleResult> {
  return call<ToggleResult>("todo_toggle_done", { id });
}

export function gameGetProfile(): Promise<GameProfile> {
  return call<GameProfile>("game_get_profile");
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: 提交**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m1): add todo/game typed api wrappers"
```

---

## Task 7: 前端 stores

**Files:**
- Create: `src/lib/stores/todos.ts`
- Create: `src/lib/stores/game.ts`

- [ ] **Step 1: todos store**

创建 `src/lib/stores/todos.ts`：

```ts
import { writable } from "svelte/store";
import {
  todoListToday,
  todoCreate,
  todoUpdate,
  todoDelete,
  todoToggleDone,
  type Todo,
  type ToggleResult,
} from "$lib/api";

export const todos = writable<Todo[]>([]);

export async function loadTodos(): Promise<void> {
  todos.set(await todoListToday());
}

export async function addTodo(title: string): Promise<void> {
  await todoCreate(title);
  await loadTodos();
}

export async function editTodo(id: number, title: string): Promise<void> {
  await todoUpdate(id, title);
  await loadTodos();
}

export async function removeTodo(id: number): Promise<void> {
  await todoDelete(id);
  await loadTodos();
}

export async function toggleTodo(id: number): Promise<ToggleResult> {
  const res = await todoToggleDone(id);
  await loadTodos();
  return res;
}
```

- [ ] **Step 2: game store**

创建 `src/lib/stores/game.ts`：

```ts
import { writable } from "svelte/store";
import { gameGetProfile } from "$lib/api";

export const coins = writable<number>(0);

export async function refreshCoins(): Promise<void> {
  const profile = await gameGetProfile();
  coins.set(profile.coins);
}
```

- [ ] **Step 3: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 4: 提交**

```bash
git add src/lib/stores/todos.ts src/lib/stores/game.ts
git commit -m "feat(m1): add todos and game svelte stores"
```

---

## Task 8: 今日任务 UI

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: 替换页面**

把 `src/routes/+page.svelte` 整个替换为：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";
  import { coins, refreshCoins } from "$lib/stores/game";
  import {
    todos,
    loadTodos,
    addTodo,
    editTodo,
    removeTodo,
    toggleTodo,
  } from "$lib/stores/todos";

  let newTitle = $state("");
  let editingId = $state<number | null>(null);
  let editingTitle = $state("");
  let reward = $state(0);

  onMount(() => {
    void initTheme();
    void loadTodos();
    void refreshCoins();
  });

  async function submitNew(e: Event) {
    e.preventDefault();
    const t = newTitle.trim();
    if (!t) return;
    newTitle = "";
    await addTodo(t);
  }

  async function onToggle(id: number) {
    const res = await toggleTodo(id);
    coins.set(res.coins);
    if (res.awarded > 0) {
      reward = res.awarded;
      setTimeout(() => (reward = 0), 1200);
    }
  }

  function startEdit(id: number, title: string) {
    editingId = id;
    editingTitle = title;
  }

  async function saveEdit(id: number) {
    const t = editingTitle.trim();
    editingId = null;
    if (t) {
      await editTodo(id, t);
    } else {
      await loadTodos();
    }
  }
</script>

<header class="bar">
  <span class="coins">🪙 {$coins}</span>
  <button class="ghost" onclick={toggleTheme} title="主题 / Theme">
    {$theme === "dark" ? "🌙" : "☀️"}
  </button>
</header>

<main class="container">
  <h1>DeskHub</h1>

  {#if reward > 0}
    <div class="reward">+{reward}🪙</div>
  {/if}

  <form class="add" onsubmit={submitNew}>
    <input placeholder="新建任务 / New task..." bind:value={newTitle} />
    <button type="submit">添加 / Add</button>
  </form>

  <ul class="list">
    {#each $todos as todo (todo.id)}
      <li class:done={todo.done}>
        <input
          type="checkbox"
          checked={todo.done}
          onchange={() => onToggle(todo.id)}
        />
        {#if editingId === todo.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="edit"
            bind:value={editingTitle}
            onblur={() => saveEdit(todo.id)}
            onkeydown={(e) => e.key === "Enter" && saveEdit(todo.id)}
            autofocus
          />
        {:else}
          <span class="title">{todo.title}</span>
        {/if}
        <span class="tag">+{todo.reward_coin}🪙</span>
        <button class="ghost" onclick={() => startEdit(todo.id, todo.title)}>✎</button>
        <button class="ghost" onclick={() => removeTodo(todo.id)}>🗑</button>
      </li>
    {/each}
    {#if $todos.length === 0}
      <li class="empty">今天还没有任务 / No tasks yet</li>
    {/if}
  </ul>
</main>

<style>
  .bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
  }

  .coins {
    font-weight: 600;
  }

  .container {
    max-width: 640px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  h1 {
    text-align: center;
  }

  .reward {
    text-align: center;
    color: #e0a300;
    font-weight: 700;
    animation: floatup 1.2s ease-out;
  }

  @keyframes floatup {
    from {
      opacity: 1;
      transform: translateY(0);
    }
    to {
      opacity: 0;
      transform: translateY(-1.5rem);
    }
  }

  .add {
    display: flex;
    gap: 0.5rem;
    margin: 1rem 0;
  }

  .add input {
    flex: 1;
  }

  input,
  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.5em 0.8em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
  }

  button {
    cursor: pointer;
  }

  .ghost {
    border-color: transparent;
    background: transparent;
    padding: 0.3em 0.5em;
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--border);
  }

  .list li.done .title {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .title {
    flex: 1;
  }

  .edit {
    flex: 1;
  }

  .tag {
    font-size: 0.85em;
    opacity: 0.7;
  }

  .empty {
    justify-content: center;
    opacity: 0.6;
  }
</style>
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors（可能有 a11y/未使用的 warning，但 errors 必须为 0）。

- [ ] **Step 3: 提交**

```bash
git add src/routes/+page.svelte
git commit -m "feat(m1): build today-task UI with coins and reward feedback"
```

---

## Task 9: 端到端验收

**Files:** 无（验证 + 文档勾选）

- [ ] **Step 1: 启动应用**

Run: `npm run tauri dev`
Expected: 窗口显示金币余额、新建框、（空）任务列表。

- [ ] **Step 2: 功能验收**

操作并确认：
1. 新建任务 → 出现在列表。
2. 勾选完成 → 金币 +10、出现 `+10🪙` 浮字、任务标题划线。
3. 取消完成 → 金币不变（仍 10）。
4. 再次完成 → 金币不变（不重发）。
5. 编辑标题（✎）→ 保存生效。
6. 删除（🗑）→ 移除。
7. 关闭应用 → 重新 `npm run tauri dev` → 任务与金币余额保留。

- [ ] **Step 3: 勾选开发计划 M1**

在 `开发计划.md` 的 M1 小节，把已完成项的 `- [ ]` 改为 `- [x]`（后端 commands、同事务发奖防重复、`game_get_profile`、前端今日列表 UI、金币展示+奖励动效、store 同步）。

- [ ] **Step 4: 提交**

```bash
git add 开发计划.md
git commit -m "docs(m1): mark M1 todo + coins milestone complete"
```

---

## 自检 / Self-Review

- **Spec 覆盖：** 数据模型(Task1,2) / 后端模块 todos+game(Task3,4) / 命令(Task5) / 金币不变式(Task4 toggle + Task3 award) / list_today(Task4) / 前端 api+stores+UI(Task6,7,8) / 测试门禁(各任务 + Task5,9) / 验收(Task9) —— 均有任务。
- **无占位符：** 所有步骤含完整代码与确切命令。
- **类型一致：** `Todo`/`GameProfile`/`ToggleResult` 字段在 Rust(Task2) 与 TS(Task6) 对应；命令名 `todo_*`/`game_get_profile` 在 Task5 注册、Task6 调用一致；`toggle_done(&mut Connection)`、`award_for_todo(&Connection,id,amount)`、`list_today` SQL 与测试一致；JS camelCase `dueDate` ↔ Rust `due_date`。

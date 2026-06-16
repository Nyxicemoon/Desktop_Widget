# DeskHub M3 收尾 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成 M3 剩余项 —— 开机自启、SQLite 备份导出/导入、任务到期提醒、NSIS 打包配置、设置页 —— 使 DeskHub 成为可分发、可后台常驻的 MVP。

**Architecture:** 后端新增三块：`db/backup.rs`（导出 VACUUM INTO / 导入校验 + 暂存 / 启动时应用暂存）、`reminder.rs`（独立 OS 线程每 60 分钟检查到期 todo，用 kv 去重发通知）、`commands/{autostart,backup}.rs`（命令层）。`db::open()` 在打开连接前先应用挂起的导入文件。前端新增设置页承载自启开关与备份按钮。无新建表、无 migration，仅复用 kv。

**Tech Stack:** Rust + Tauri v2（`tauri-plugin-autostart`、`tauri-plugin-dialog`、`tauri-plugin-notification`）、rusqlite（bundled，VACUUM INTO）、SvelteKit + TypeScript、`@tauri-apps/plugin-dialog`。

**参考 spec：** `docs/superpowers/specs/2026-06-16-deskhub-m3-finish-design.md`

**质量门禁（每个含 Rust 改动的任务结束跑）：** 在 `src-tauri/` 下 `cargo test` 与 `cargo clippy -- -D warnings`；含前端改动跑 `npm run check`（项目根）。

---

## Task 1: 添加依赖与插件注册

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs:15-18`（plugin 链）
- Modify: `package.json`（前端 dialog 包）

- [ ] **Step 1: 加 Cargo 依赖**

在 `src-tauri/Cargo.toml` `[dependencies]` 末尾追加：

```toml
tauri-plugin-autostart = "2"
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: 加前端 dialog 包**

在项目根运行：

```bash
npm install @tauri-apps/plugin-dialog
```

- [ ] **Step 3: 注册两个插件**

在 `src-tauri/src/lib.rs` 的 builder 链中，`.plugin(tauri_plugin_notification::init())` 之后插入：

```rust
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri; cargo build`
Expected: 编译通过（可能拉取新 crate，首次较慢）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json src-tauri/src/lib.rs
git commit -m "build(m3): add autostart + dialog plugins"
```

---

## Task 2: 数据备份核心 `db/backup.rs`（导出 / 校验 / 暂存 / 启动应用）

**Files:**
- Create: `src-tauri/src/db/backup.rs`
- Modify: `src-tauri/src/db/mod.rs:1-5`（加 `pub mod backup;`）

- [ ] **Step 1: 新建 `src-tauri/src/db/backup.rs`，写实现 + 测试**

```rust
//! Database backup: export a clean copy, validate/stage an import,
//! and apply a staged import at startup.

use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// Export a clean, consistent copy of the live DB to `dest` (overwrites).
/// Uses SQLite `VACUUM INTO`, which produces a single compact file and
/// handles WAL correctly. `VACUUM INTO` requires the destination not exist.
pub fn export(conn: &Connection, dest: &str) -> AppResult<()> {
    if Path::new(dest).exists() {
        std::fs::remove_file(dest)?;
    }
    conn.execute("VACUUM INTO ?1", [dest])?;
    Ok(())
}

/// Return Ok(()) if `src` is a valid DeskHub backup: an SQLite database
/// containing the `kv` table. Otherwise an error suitable for the user.
pub fn validate(src: &Path) -> AppResult<()> {
    let conn = Connection::open(src)?;
    let has_kv: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='kv'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !has_kv {
        return Err(AppError::Other(
            "不是有效的 DeskHub 备份 / Not a valid DeskHub backup".into(),
        ));
    }
    Ok(())
}

/// Validate `src` then copy it to the staging path `deskhub.db.import` under
/// `dir`. The staged file is applied on next startup (see `apply_pending_import`).
/// We do not overwrite the live DB directly because Windows locks the in-use file.
pub fn stage_import(dir: &Path, src: &Path) -> AppResult<()> {
    validate(src)?;
    let staging = dir.join("deskhub.db.import");
    std::fs::copy(src, &staging)?;
    Ok(())
}

/// If a staged import exists in `dir`, replace the live DB with it.
/// Must be called BEFORE opening the connection. Deletes the old db + WAL/SHM
/// sidecars, then renames the staged file into place.
pub fn apply_pending_import(dir: &Path) -> AppResult<()> {
    let staging = dir.join("deskhub.db.import");
    if !staging.exists() {
        return Ok(());
    }
    for ext in ["", "-wal", "-shm"] {
        let p = dir.join(format!("deskhub.db{ext}"));
        let _ = std::fs::remove_file(&p);
    }
    std::fs::rename(&staging, dir.join("deskhub.db"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::Connection;

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "deskhub_backup_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Build a real DeskHub db file at `path` with kv key `marker`=value.
    fn make_db(path: &Path, marker: &str) {
        let mut conn = Connection::open(path).unwrap();
        migrations::apply(&mut conn).unwrap();
        crate::db::kv::set(&conn, "marker", marker).unwrap();
    }

    #[test]
    fn export_produces_reopenable_db() {
        let dir = temp_dir();
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        crate::db::kv::set(&conn, "marker", "hello").unwrap();

        let dest = dir.join("out.db");
        export(&conn, dest.to_str().unwrap()).unwrap();
        assert!(dest.exists());

        let copy = Connection::open(&dest).unwrap();
        let v: String = copy
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_accepts_real_db_rejects_garbage() {
        let dir = temp_dir();
        let good = dir.join("good.db");
        make_db(&good, "x");
        assert!(validate(&good).is_ok());

        let bad = dir.join("bad.db");
        std::fs::write(&bad, b"not a database").unwrap();
        assert!(validate(&bad).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_pending_import_replaces_live_db() {
        let dir = temp_dir();
        // live db has marker "old"; staged import has marker "new"
        make_db(&dir.join("deskhub.db"), "old");
        make_db(&dir.join("deskhub.db.import"), "new");

        apply_pending_import(&dir).unwrap();

        assert!(!dir.join("deskhub.db.import").exists());
        let conn = Connection::open(dir.join("deskhub.db")).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "new");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_pending_import_noop_without_staging() {
        let dir = temp_dir();
        make_db(&dir.join("deskhub.db"), "keep");
        apply_pending_import(&dir).unwrap(); // no staging file -> no-op
        let conn = Connection::open(dir.join("deskhub.db")).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "keep");
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: 注册模块**

在 `src-tauri/src/db/mod.rs` 顶部模块声明区（第 1-5 行附近）按字母序加：

```rust
pub mod backup;
```

- [ ] **Step 3: 跑测试**

Run: `cd src-tauri; cargo test backup`
Expected: 4 个新测试全 PASS。

- [ ] **Step 4: clippy**

Run: `cd src-tauri; cargo clippy -- -D warnings`
Expected: 无警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/backup.rs src-tauri/src/db/mod.rs
git commit -m "feat(m3): db backup export/validate/stage/apply-import"
```

---

## Task 3: 启动时应用挂起的导入

**Files:**
- Modify: `src-tauri/src/db/mod.rs:17-26`（`open` 函数）

- [ ] **Step 1: 在 open() 打开连接前调用 apply_pending_import**

把 `open` 函数体改为：

```rust
pub fn open(app: &AppHandle) -> AppResult<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    backup::apply_pending_import(&dir)?;
    let mut conn = Connection::open(dir.join("deskhub.db"))?;
    migrations::apply(&mut conn)?;
    Ok(conn)
}
```

- [ ] **Step 2: 编译**

Run: `cd src-tauri; cargo build`
Expected: 通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/mod.rs
git commit -m "feat(m3): apply staged import on startup before open"
```

---

## Task 4: 备份命令层 `commands/backup.rs`

**Files:**
- Create: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`（invoke_handler）

- [ ] **Step 1: 新建 `src-tauri/src/commands/backup.rs`**

```rust
use crate::db::{backup, Db};
use crate::error::{AppError, AppResult};
use std::path::Path;
use tauri::{AppHandle, Manager, State};

/// Export the live DB to a user-chosen path (passed from the frontend dialog).
#[tauri::command]
pub fn db_export(db: State<Db>, dest: String) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    backup::export(&conn, &dest)
}

/// Validate a user-chosen backup file and stage it; applied on next restart.
#[tauri::command]
pub fn db_import(app: AppHandle, src: String) -> AppResult<()> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    backup::stage_import(&dir, Path::new(&src))
}
```

- [ ] **Step 2: 注册模块**

在 `src-tauri/src/commands/mod.rs` 按字母序加（在 `backgrounds` 之后）：

```rust
pub mod backup;
```

- [ ] **Step 3: 注册命令**

在 `src-tauri/src/lib.rs` 的 `tauri::generate_handler![...]` 中，`commands::backgrounds::bg_restore_default,` 之后加：

```rust
            commands::backup::db_export,
            commands::backup::db_import,
```

- [ ] **Step 4: 编译 + clippy**

Run: `cd src-tauri; cargo build; cargo clippy -- -D warnings`
Expected: 通过、无警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(m3): db_export / db_import commands"
```

---

## Task 5: 到期 todo 查询 `db::todos::list_due`

**Files:**
- Modify: `src-tauri/src/db/todos.rs`（加函数 + 测试）

- [ ] **Step 1: 加 list_due 函数**

在 `src-tauri/src/db/todos.rs` 的 `toggle_done` 函数之后（`#[cfg(test)]` 之前）加：

```rust
/// IDs + titles of todos that are due today or overdue and not yet done.
/// Used by the reminder loop; notification de-dup is handled via kv.
pub fn list_due(conn: &Connection) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, title FROM todos
         WHERE done = 0 AND due_date IS NOT NULL
           AND date(due_date) <= date('now','localtime')",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
```

- [ ] **Step 2: 加测试**

在 `src-tauri/src/db/todos.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn list_due_returns_only_due_and_undone() {
        let conn = setup();
        // due today, undone -> included
        conn.execute(
            "INSERT INTO todos (title, due_date) VALUES ('due_today', date('now','localtime'))",
            [],
        )
        .unwrap();
        // overdue, undone -> included
        conn.execute(
            "INSERT INTO todos (title, due_date) VALUES ('overdue', date('now','localtime','-2 day'))",
            [],
        )
        .unwrap();
        // future -> excluded
        conn.execute(
            "INSERT INTO todos (title, due_date) VALUES ('future', date('now','localtime','+2 day'))",
            [],
        )
        .unwrap();
        // due today but done -> excluded
        conn.execute(
            "INSERT INTO todos (title, done, due_date) VALUES ('done_due', 1, date('now','localtime'))",
            [],
        )
        .unwrap();
        // no due_date -> excluded
        create(&conn, "no_due", None, None).unwrap();

        let due = list_due(&conn).unwrap();
        let titles: Vec<&str> = due.iter().map(|(_, t)| t.as_str()).collect();
        assert!(titles.contains(&"due_today"));
        assert!(titles.contains(&"overdue"));
        assert!(!titles.contains(&"future"));
        assert!(!titles.contains(&"done_due"));
        assert!(!titles.contains(&"no_due"));
    }
```

- [ ] **Step 3: 跑测试**

Run: `cd src-tauri; cargo test list_due`
Expected: PASS。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/todos.rs
git commit -m "feat(m3): todos::list_due query for reminders"
```

---

## Task 6: 到期提醒模块 `reminder.rs`

**Files:**
- Create: `src-tauri/src/reminder.rs`
- Modify: `src-tauri/src/lib.rs`（mod 声明 + setup 启动循环）

- [ ] **Step 1: 新建 `src-tauri/src/reminder.rs`**

```rust
//! Due-task reminders. A background OS thread checks once at startup and then
//! every 60 minutes, sending a local notification for each due, undone todo
//! that has not been notified before (de-duped via kv `reminder:notified:<id>`).

use crate::db::{kv, todos, Db};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Check due todos and notify for any not yet notified. Best-effort: any lock
/// or query failure is swallowed so the loop keeps running.
pub fn check_and_notify(app: &AppHandle) {
    let state = app.state::<Db>();
    let to_notify: Vec<String> = {
        let conn = match state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let due = match todos::list_due(&conn) {
            Ok(d) => d,
            Err(_) => return,
        };
        let mut pending = Vec::new();
        for (id, title) in due {
            let key = format!("reminder:notified:{id}");
            let seen = kv::get(&conn, &key).unwrap_or(None).is_some();
            if !seen {
                let _ = kv::set(&conn, &key, "1");
                pending.push(title);
            }
        }
        pending
    };
    for title in to_notify {
        let _ = app
            .notification()
            .builder()
            .title("任务到期 / Task due")
            .body(&title)
            .show();
    }
}

/// Spawn the reminder loop on a dedicated, mostly-sleeping OS thread.
pub fn spawn_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        check_and_notify(&app);
        std::thread::sleep(CHECK_INTERVAL);
    });
}
```

- [ ] **Step 2: 声明模块**

在 `src-tauri/src/lib.rs` 顶部模块声明区（`mod pexels;` 与 `mod system;` 之间，保持字母序）加：

```rust
mod reminder;
```

- [ ] **Step 3: 在 setup 中启动循环**

在 `src-tauri/src/lib.rs` 的 `setup` 闭包里，`tray::create(app.handle())?;` 之后、`Ok(())` 之前加：

```rust
            reminder::spawn_loop(app.handle().clone());
```

- [ ] **Step 4: 编译 + clippy**

Run: `cd src-tauri; cargo build; cargo clippy -- -D warnings`
Expected: 通过、无警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/reminder.rs src-tauri/src/lib.rs
git commit -m "feat(m3): due-task reminder loop with kv dedup"
```

---

## Task 7: 开机自启命令 + 首次默认 + `--hidden` 控制主窗口

**Files:**
- Create: `src-tauri/src/commands/autostart.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`（invoke_handler + setup 首次默认 + 主窗口显隐）

- [ ] **Step 1: 新建 `src-tauri/src/commands/autostart.rs`**

```rust
use crate::error::{AppError, AppResult};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn autostart_get(app: AppHandle) -> AppResult<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> AppResult<()> {
    let mgr = app.autolaunch();
    let r = if enabled { mgr.enable() } else { mgr.disable() };
    r.map_err(|e| AppError::Other(e.to_string()))
}
```

- [ ] **Step 2: 注册模块**

在 `src-tauri/src/commands/mod.rs` 按字母序加（在 `autostart` 应排在最前；插到 `backgrounds` 之前）：

```rust
pub mod autostart;
```

- [ ] **Step 3: 注册命令**

在 `src-tauri/src/lib.rs` 的 `generate_handler![...]` 中，`commands::backup::db_import,` 之后加：

```rust
            commands::autostart::autostart_get,
            commands::autostart::autostart_set,
```

- [ ] **Step 4: setup 中首次默认开启自启**

在 `src-tauri/src/lib.rs` 的 `setup` 闭包里，`app.manage(db::Db(...))` 之后、读取 widget 可见性之前，插入：

```rust
            {
                use tauri_plugin_autostart::ManagerExt;
                let first_run = {
                    let state = app.state::<db::Db>();
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    db::kv::get(&conn, "autostart.initialized")
                        .unwrap_or(None)
                        .is_none()
                };
                if first_run {
                    let _ = app.autolaunch().enable();
                    let state = app.state::<db::Db>();
                    let conn = state.0.lock().map_err(|e| e.to_string())?;
                    let _ = db::kv::set(&conn, "autostart.initialized", "1");
                }
            }
```

- [ ] **Step 5: setup 中按 `--hidden` 控制主窗口显隐**

在 `src-tauri/src/lib.rs` 的 `setup` 闭包里，`reminder::spawn_loop(...)` 之后、`Ok(())` 之前加：

```rust
            let hidden = std::env::args().any(|a| a == "--hidden");
            if !hidden {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
```

（主窗口在 `tauri.conf.json` 中将设为 `visible:false`，见 Task 8；故需在此显式 show。）

- [ ] **Step 6: 编译 + clippy**

Run: `cd src-tauri; cargo build; cargo clippy -- -D warnings`
Expected: 通过、无警告。`tauri::Manager` 已在 lib.rs 顶部 `use`（`get_webview_window`/`state` 依赖它），无需额外 import。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/autostart.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(m3): autostart commands, first-run default, --hidden startup"
```

---

## Task 8: 配置 —— 主窗口隐藏 + NSIS 打包 + dialog 权限

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 主窗口默认隐藏 + NSIS bundle**

把 `src-tauri/tauri.conf.json` 改为（仅改 `app.windows[0]` 加 `"visible": false`，并把 `bundle` 段替换为 NSIS 配置）：

`app.windows[0]` 对象改为：

```json
      {
        "label": "main",
        "title": "deskhub",
        "width": 800,
        "height": 600,
        "visible": false
      }
```

`bundle` 段整体替换为：

```json
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "publisher": "DeskHub",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "nsis": {
        "installMode": "currentUser"
      }
    }
  }
```

- [ ] **Step 2: 加 dialog 权限**

把 `src-tauri/capabilities/default.json` 的 `permissions` 数组改为：

```json
  "permissions": [
    "core:default",
    "opener:default",
    "notification:default",
    "dialog:default"
  ]
```

- [ ] **Step 3: 编译验证配置合法**

Run: `cd src-tauri; cargo build`
Expected: 通过（tauri 会校验 conf 与 capabilities schema）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json
git commit -m "feat(m3): hidden main window, NSIS currentUser bundle, dialog perm"
```

---

## Task 9: 前端 API 封装

**Files:**
- Modify: `src/lib/api/index.ts`（文件末尾追加）

- [ ] **Step 1: 追加 API 封装**

在 `src/lib/api/index.ts` 末尾追加：

```ts
export function autostartGet(): Promise<boolean> {
  return call<boolean>("autostart_get");
}

export function autostartSet(enabled: boolean): Promise<void> {
  return call<void>("autostart_set", { enabled });
}

export function dbExport(dest: string): Promise<void> {
  return call<void>("db_export", { dest });
}

export function dbImport(src: string): Promise<void> {
  return call<void>("db_import", { src });
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors（可能有既有 warning，无新增 error）。

- [ ] **Step 3: Commit**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m3): frontend api wrappers for autostart + backup"
```

---

## Task 10: 设置页 + 导航

**Files:**
- Create: `src/routes/(app)/settings/+page.svelte`
- Modify: `src/routes/(app)/+layout.svelte`（导航加链接）

- [ ] **Step 1: 新建设置页**

新建 `src/routes/(app)/settings/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { autostartGet, autostartSet, dbExport, dbImport } from "$lib/api";

  let autostart = $state(false);
  let busy = $state(false);
  let message = $state("");

  onMount(async () => {
    try {
      autostart = await autostartGet();
    } catch (e) {
      message = `读取自启状态失败 / Failed to read autostart: ${e}`;
    }
  });

  async function toggleAutostart() {
    busy = true;
    try {
      const next = !autostart;
      await autostartSet(next);
      autostart = next;
      message = next ? "已开启开机自启 / Autostart on" : "已关闭开机自启 / Autostart off";
    } catch (e) {
      message = `设置失败 / Failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  function dateStamp(): string {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}`;
  }

  async function exportBackup() {
    busy = true;
    message = "";
    try {
      const dest = await save({
        defaultPath: `deskhub-backup-${dateStamp()}.db`,
        filters: [{ name: "DeskHub Backup", extensions: ["db"] }],
      });
      if (!dest) return;
      await dbExport(dest);
      message = "已导出备份 / Backup exported";
    } catch (e) {
      message = `导出失败 / Export failed: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function importBackup() {
    busy = true;
    message = "";
    try {
      const src = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "DeskHub Backup", extensions: ["db"] }],
      });
      if (!src || typeof src !== "string") return;
      await dbImport(src);
      message = "已导入，请重启应用以生效 / Imported — restart the app to apply";
    } catch (e) {
      message = `导入失败 / Import failed: ${e}`;
    } finally {
      busy = false;
    }
  }
</script>

<main class="container">
  <h1>设置 / Settings</h1>

  <section class="card">
    <h2>开机自启 / Autostart</h2>
    <label class="row">
      <input type="checkbox" checked={autostart} onchange={toggleAutostart} disabled={busy} />
      <span>随 Windows 启动（隐藏到托盘） / Start with Windows (hidden to tray)</span>
    </label>
  </section>

  <section class="card">
    <h2>数据备份 / Backup</h2>
    <p>导出当前数据为 .db 文件，或从备份恢复（导入后需重启）。</p>
    <p>Export your data as a .db file, or restore from a backup (restart after import).</p>
    <div class="row">
      <button onclick={exportBackup} disabled={busy}>导出备份 / Export</button>
      <button onclick={importBackup} disabled={busy}>导入备份 / Import</button>
    </div>
  </section>

  {#if message}
    <p class="msg">{message}</p>
  {/if}
</main>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  .card {
    margin-bottom: 1.25rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  h2 {
    font-size: 1.05rem;
    margin: 0 0 0.6rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  button {
    border-radius: 8px;
    border: 1px solid var(--border);
    padding: 0.5em 0.9em;
    font-size: 1em;
    color: var(--fg);
    background: var(--surface);
    cursor: pointer;
  }

  .msg {
    opacity: 0.85;
  }
</style>
```

- [ ] **Step 2: 导航加「设置」链接**

在 `src/routes/(app)/+layout.svelte` 的 `<nav>` 内，`<a href="/backgrounds">背景 / Backgrounds</a>` 之后加：

```svelte
      <a href="/settings">设置 / Settings</a>
```

- [ ] **Step 3: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
git add "src/routes/(app)/settings/+page.svelte" "src/routes/(app)/+layout.svelte"
git commit -m "feat(m3): settings page (autostart toggle + backup export/import)"
```

---

## Task 11: 全量验证

- [ ] **Step 1: Rust 全量测试**

Run: `cd src-tauri; cargo test`
Expected: 全部 PASS（既有 24 个 + 新增 5 个 = 29 个左右）。

- [ ] **Step 2: clippy 全量**

Run: `cd src-tauri; cargo clippy -- -D warnings`
Expected: 无警告。

- [ ] **Step 3: 前端类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 4: 尝试打包（重操作，可较慢）**

Run: `npm run tauri build`
Expected: 在 `src-tauri/target/release/bundle/nsis/` 生成 `*-setup.exe`。
若构建超时或环境受限：以前述 `cargo build` + 配置正确为准，记录「打包配置就绪，未在本机完成完整 build」并继续。

- [ ] **Step 5: 报告**

汇总：测试数、clippy 结果、check 结果、是否产出安装器（路径）。列出需用户手动验证的项：
- 托盘静默启动（`--hidden`）
- 设置页自启开关注册到 `HKCU\...\Run`
- 导出 / 导入往返
- 到期 todo 当天弹通知
- 安装器在干净 Windows 上安装运行

---

## 自检 / Self-Review 结论

- **Spec 覆盖：** 自启(Task 1/7)、备份导出导入(Task 2/3/4)、到期提醒(Task 5/6)、NSIS 打包(Task 8)、设置页(Task 9/10)、内存优化(spec 第七节降范围为核查项，无代码) —— 全覆盖。
- **占位符：** 无 TBD/TODO，所有步骤含完整代码与命令。
- **类型一致：** `export/validate/stage_import/apply_pending_import`、`list_due`、`check_and_notify/spawn_loop`、`autostart_get/set`、`db_export/db_import` 在跨任务引用处签名一致；kv 键 `autostart.initialized`、`reminder:notified:<id>` 一致。
- **依赖顺序：** Task 2 建 backup.rs → Task 3/4 引用；Task 5 建 list_due → Task 6 引用；Task 7 主窗口 show 依赖 Task 8 的 visible:false（已在 Task 7 Step 5 注明），二者均在 Task 11 前完成，最终状态自洽。

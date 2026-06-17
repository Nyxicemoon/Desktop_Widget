# DeskHub M4 桌面图标管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 桌面快捷方式聚合为可分类/收藏/拖拽补充/一键启动的应用中心，呈现为主窗口「应用」页 + 透明桌面 widget，并提取真实图标。

**Architecture:** Win32 难点已 spike 并预置（见下）。本计划在其上构建：`db/apps.rs`（自定义应用 + 偏好覆盖 + 纯函数合并）、`commands/apps.rs`（扫描/图标/启动/拖入/收藏/分类）、主窗页 + widget（拖放接收）、托盘开关。

**Tech Stack:** Rust + Tauri v2、rusqlite、windows-rs（COM/Shell/GDI）、`image`(png)、SvelteKit + TS、`@tauri-apps/api` drag-drop。

**参考 spec：** `docs/superpowers/specs/2026-06-16-deskhub-m4-icons-design.md`

---

## ⚠️ 预置脚手架（已在工作区，且已 `cargo build` + 一次编译通过——不要重写）

以下文件已由主代理 spike 并验证编译通过，**保持原样，勿改、勿重建**：

- `src-tauri/Cargo.toml` — 已加 `image`(png) 依赖 + 扩展 `windows` features（Com/Shell/Shell_Common/Storage_FileSystem/Graphics_Gdi）。
- `src-tauri/src/system/shortcuts.rs` — Win32 实现，公开 API：
  ```rust
  pub struct ShortcutRaw { pub name: String, pub lnk_path: String, pub target: String, pub args: Option<String> }
  pub fn scan() -> AppResult<Vec<ShortcutRaw>>;
  pub fn resolve_dropped(path: &str) -> AppResult<ShortcutRaw>;
  pub fn launch(path: &str) -> AppResult<()>;
  pub fn icon_data_url(path: &str) -> AppResult<Option<String>>;  // best-effort, Ok(None) on failure
  ```
- `src-tauri/src/system/mod.rs` — 已加 `pub mod shortcuts;`。

> 这些文件目前 `cargo clippy` 会报「never used」死代码 —— 因为消费它们的层尚未写。**Task 1 接好线后这些警告消失**，所以 Task 1 之前不要单独跑 clippy 门禁。

**质量门禁（每个任务末尾）：** `src-tauri/` 下 `cargo test`、`cargo clippy -- -D warnings`；含前端跑 `npm run check`。

---

## Task 1: 后端纵向切片（models + 迁移 + db/apps + commands/apps + 注册）

> 一次提交完成一个自洽的后端切片，使预置的 `shortcuts.rs` 被消费、clippy 转绿。

**Files:**
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/src/db/apps.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/commands/apps.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: models — 加 AppEntry，扩展 WidgetVisibility**

在 `src-tauri/src/models/mod.rs` 末尾追加：

```rust
#[derive(Debug, Serialize)]
pub struct AppEntry {
    pub name: String,
    pub launch_path: String,
    pub target: String,
    pub args: Option<String>,
    pub is_custom: bool,
    pub category: Option<String>,
    pub favorite: bool,
}
```

并把 `WidgetVisibility` 改为（增加 `apps` 字段）：

```rust
#[derive(Debug, Serialize)]
pub struct WidgetVisibility {
    pub todo: bool,
    pub coins: bool,
    pub apps: bool,
}
```

- [ ] **Step 2: 迁移 v4**

在 `src-tauri/src/db/migrations.rs` 的 `MIGRATIONS` 数组末尾（`(3, ...)` 之后）加：

```rust
    (
        4,
        "CREATE TABLE custom_apps (
            id         INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            target     TEXT NOT NULL,
            args       TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE app_prefs (
            target     TEXT PRIMARY KEY,
            category   TEXT,
            favorite   INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ),
```

并在 `migrations.rs` 的 `applies_migrations_on_empty_db` 测试里，把表清单
`["kv", "todos", "game_profile", "coin_ledger", "backgrounds"]`
改为
`["kv", "todos", "game_profile", "coin_ledger", "backgrounds", "custom_apps", "app_prefs"]`。

- [ ] **Step 3: 创建 `src-tauri/src/db/apps.rs`**

```rust
use crate::error::AppResult;
use crate::models::AppEntry;
use crate::system::shortcuts::ShortcutRaw;
use rusqlite::Connection;
use std::collections::HashMap;

/// (name, target, args)
pub fn list_custom(conn: &Connection) -> AppResult<Vec<(String, String, Option<String>)>> {
    let mut stmt = conn.prepare("SELECT name, target, args FROM custom_apps ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn add_custom(conn: &Connection, name: &str, target: &str, args: Option<&str>) -> AppResult<()> {
    // de-dup by lowercased target
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM custom_apps WHERE lower(target) = lower(?1) LIMIT 1",
            [target],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if exists {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO custom_apps (name, target, args) VALUES (?1, ?2, ?3)",
        (name, target, args),
    )?;
    Ok(())
}

pub fn remove_custom(conn: &Connection, target: &str) -> AppResult<()> {
    conn.execute("DELETE FROM custom_apps WHERE lower(target) = lower(?1)", [target])?;
    Ok(())
}

/// target(lowercased) -> (category, favorite, sort_order)
pub fn prefs_map(conn: &Connection) -> AppResult<HashMap<String, (Option<String>, bool, i64)>> {
    let mut stmt = conn.prepare("SELECT target, category, favorite, sort_order FROM app_prefs")?;
    let rows = stmt.query_map([], |r| {
        let target: String = r.get(0)?;
        let category: Option<String> = r.get(1)?;
        let favorite: i64 = r.get(2)?;
        let sort_order: i64 = r.get(3)?;
        Ok((target.to_lowercase(), (category, favorite != 0, sort_order)))
    })?;
    let mut map = HashMap::new();
    for r in rows {
        let (k, v) = r?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn set_favorite(conn: &Connection, target: &str, favorite: bool) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_prefs (target, favorite, updated_at)
         VALUES (lower(?1), ?2, datetime('now'))
         ON CONFLICT(target) DO UPDATE SET favorite = excluded.favorite, updated_at = datetime('now')",
        (target, favorite as i64),
    )?;
    Ok(())
}

pub fn set_category(conn: &Connection, target: &str, category: Option<&str>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_prefs (target, category, updated_at)
         VALUES (lower(?1), ?2, datetime('now'))
         ON CONFLICT(target) DO UPDATE SET category = excluded.category, updated_at = datetime('now')",
        (target, category),
    )?;
    Ok(())
}

/// Pure merge: scanned shortcuts ∪ custom apps, de-duped by lowercased target,
/// overlaid with prefs, sorted by favorite desc, sort_order asc, name asc.
pub fn merge(
    scanned: Vec<ShortcutRaw>,
    custom: Vec<(String, String, Option<String>)>,
    prefs: &HashMap<String, (Option<String>, bool, i64)>,
) -> Vec<AppEntry> {
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut entries: Vec<AppEntry> = Vec::new();

    for s in scanned {
        let key = s.target.to_lowercase();
        if seen.insert(key.clone(), ()).is_some() {
            continue;
        }
        let (category, favorite, _) = prefs.get(&key).cloned().unwrap_or((None, false, 0));
        entries.push(AppEntry {
            name: s.name,
            launch_path: s.lnk_path,
            target: s.target,
            args: s.args,
            is_custom: false,
            category,
            favorite,
        });
    }
    for (name, target, args) in custom {
        let key = target.to_lowercase();
        if seen.insert(key.clone(), ()).is_some() {
            continue;
        }
        let (category, favorite, _) = prefs.get(&key).cloned().unwrap_or((None, false, 0));
        entries.push(AppEntry {
            name,
            launch_path: target.clone(),
            target,
            args,
            is_custom: true,
            category,
            favorite,
        });
    }

    entries.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then_with(|| {
                let sa = prefs.get(&a.target.to_lowercase()).map(|p| p.2).unwrap_or(0);
                let sb = prefs.get(&b.target.to_lowercase()).map(|p| p.2).unwrap_or(0);
                sa.cmp(&sb)
            })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        conn
    }

    fn raw(name: &str, target: &str) -> ShortcutRaw {
        ShortcutRaw {
            name: name.to_string(),
            lnk_path: format!("C:\\Desktop\\{name}.lnk"),
            target: target.to_string(),
            args: None,
        }
    }

    #[test]
    fn add_custom_dedups_by_target() {
        let conn = setup();
        add_custom(&conn, "App", "C:\\a\\app.exe", None).unwrap();
        add_custom(&conn, "App2", "C:\\A\\APP.EXE", None).unwrap(); // same target, diff case
        assert_eq!(list_custom(&conn).unwrap().len(), 1);
    }

    #[test]
    fn favorite_and_category_upsert() {
        let conn = setup();
        set_favorite(&conn, "C:\\a\\app.exe", true).unwrap();
        set_category(&conn, "C:\\A\\app.exe", Some("Work")).unwrap();
        let m = prefs_map(&conn).unwrap();
        let p = m.get("c:\\a\\app.exe").unwrap();
        assert_eq!(p.0.as_deref(), Some("Work"));
        assert!(p.1);
    }

    #[test]
    fn merge_dedups_and_sorts_favorites_first() {
        let scanned = vec![raw("Zeta", "C:\\z.exe"), raw("Alpha", "C:\\a.exe")];
        let custom = vec![
            ("AlphaDup".to_string(), "C:\\A.EXE".to_string(), None), // dup of Alpha by target
            ("Beta".to_string(), "C:\\b.exe".to_string(), None),
        ];
        let mut prefs = HashMap::new();
        prefs.insert("c:\\b.exe".to_string(), (Some("Fav".to_string()), true, 0));
        let merged = merge(scanned, custom, &prefs);
        // dup dropped: Zeta, Alpha, Beta = 3
        assert_eq!(merged.len(), 3);
        // Beta is favorite -> first
        assert_eq!(merged[0].name, "Beta");
        assert!(merged[0].favorite);
        // remaining alphabetical: Alpha, Zeta
        assert_eq!(merged[1].name, "Alpha");
        assert_eq!(merged[2].name, "Zeta");
        // Alpha kept scanned (not custom)
        assert!(!merged[1].is_custom);
    }
}
```

- [ ] **Step 4: 挂载 db 模块**

在 `src-tauri/src/db/mod.rs` 顶部模块声明区按字母序加（在 `backgrounds` 之后、`backup` 之前）：

```rust
pub mod apps;
```

- [ ] **Step 5: 创建 `src-tauri/src/commands/apps.rs`**

```rust
use crate::db::{apps, Db};
use crate::error::{AppError, AppResult};
use crate::models::AppEntry;
use crate::system::shortcuts;
use tauri::State;

fn lock(db: &State<Db>) -> AppResult<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    db.0.lock().map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn apps_scan(db: State<Db>) -> AppResult<Vec<AppEntry>> {
    let scanned = shortcuts::scan().unwrap_or_default();
    let conn = lock(&db)?;
    let custom = apps::list_custom(&conn)?;
    let prefs = apps::prefs_map(&conn)?;
    Ok(apps::merge(scanned, custom, &prefs))
}

#[tauri::command]
pub fn app_icon(path: String) -> AppResult<Option<String>> {
    shortcuts::icon_data_url(&path)
}

#[tauri::command]
pub fn app_launch(path: String) -> AppResult<()> {
    shortcuts::launch(&path)
}

#[tauri::command]
pub fn app_add_dropped(db: State<Db>, path: String) -> AppResult<()> {
    let r = shortcuts::resolve_dropped(&path)?;
    let conn = lock(&db)?;
    apps::add_custom(&conn, &r.name, &r.target, r.args.as_deref())
}

#[tauri::command]
pub fn app_remove_custom(db: State<Db>, target: String) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::remove_custom(&conn, &target)
}

#[tauri::command]
pub fn app_set_favorite(db: State<Db>, target: String, favorite: bool) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::set_favorite(&conn, &target, favorite)
}

#[tauri::command]
pub fn app_set_category(db: State<Db>, target: String, category: Option<String>) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::set_category(&conn, &target, category.as_deref())
}
```

- [ ] **Step 6: 挂载 commands 模块**

在 `src-tauri/src/commands/mod.rs` 加（在 `apps` 应排最前，`autostart` 之前）：

```rust
pub mod apps;
```

- [ ] **Step 7: 注册命令**

在 `src-tauri/src/lib.rs` 的 `generate_handler![...]`，`commands::widget::widget_get_visibility` 之前加：

```rust
            commands::apps::apps_scan,
            commands::apps::app_icon,
            commands::apps::app_launch,
            commands::apps::app_add_dropped,
            commands::apps::app_remove_custom,
            commands::apps::app_set_favorite,
            commands::apps::app_set_category,
```

并在 `setup` 中读取 widget 可见性的 fallback 里，把
`WidgetVisibility { todo: false, coins: false }`
改为
`WidgetVisibility { todo: false, coins: false, apps: false }`。

- [ ] **Step 8: 修复 `window::read_visibility`（因 WidgetVisibility 新增字段，必须同步否则编译失败）**

把 `src-tauri/src/window/mod.rs` 的 `read_visibility` 改为：

```rust
pub fn read_visibility(conn: &Connection) -> AppResult<WidgetVisibility> {
    Ok(WidgetVisibility {
        todo: kv::get(conn, "widget.todo.visible")?.as_deref() == Some("1"),
        coins: kv::get(conn, "widget.coins.visible")?.as_deref() == Some("1"),
        apps: kv::get(conn, "widget.apps.visible")?.as_deref() == Some("1"),
    })
}
```

并在 `window/mod.rs` 的 `visibility_defaults_false` 测试里加 `assert!(!v.apps);`。

- [ ] **Step 9: 跑门禁**

Run: `cd src-tauri; cargo test; cargo clippy -- -D warnings`
Expected: 测试全过（含新增 db/apps 测试 + 迁移表清单）；clippy 干净（预置 shortcuts.rs 现已被消费，死代码警告消失）。

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/system src-tauri/src/models src-tauri/src/db src-tauri/src/commands src-tauri/src/window src-tauri/src/lib.rs
git commit -m "feat(m4): backend — scan/icon/launch + custom apps + prefs + commands"
```

---

## Task 2: 窗口/widget/托盘 接入 apps

**Files:**
- Modify: `src-tauri/src/window/mod.rs`
- Modify: `src-tauri/src/lib.rs`（setup 恢复 apps widget）
- Modify: `src-tauri/src/tray.rs`

- [ ] **Step 1: window — widget_config 加 apps 分支**

在 `src-tauri/src/window/mod.rs` 的 `widget_config` match 中，`"coins" =>` 之后加：

```rust
        "apps" => Ok(("widget-apps", "/widgets/apps", 320.0, 220.0, 40.0, 420.0)),
```

（`read_visibility` 与其测试已在 Task 1 Step 8 改好，本步不再动。）

- [ ] **Step 2: lib.rs setup 恢复 apps widget**

在 `src-tauri/src/lib.rs` setup 中，`if vis.coins { ... }` 之后加：

```rust
            if vis.apps {
                let _ = window::open_widget(app.handle(), "apps");
            }
```

- [ ] **Step 3: 托盘加 apps widget 开关**

打开 `src-tauri/src/tray.rs`，参照现有 `toggle_coins` 菜单项与其点击分支，**照搬一份**用于 apps：
- 新建一个 `MenuItem`，id 用 `"toggle_apps"`，文案 `"显示/隐藏 应用 / Toggle Apps"`。
- 加入菜单（在 coins 项之后）。
- 在菜单事件 match 中加 `"toggle_apps" => { spawn_toggle(app, "apps"); }`（与 coins 分支同形，仅 kind 改为 `"apps"`）。

> 注意：保持与现有 `toggle_todo`/`toggle_coins` 完全同构。若现有用的是 `spawn_toggle(app.clone(), "coins")` 之类写法，照抄改 kind。

- [ ] **Step 4: 门禁**

Run: `cd src-tauri; cargo test; cargo clippy -- -D warnings`
Expected: 全过、干净。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window src-tauri/src/lib.rs src-tauri/src/tray.rs
git commit -m "feat(m4): apps widget config, visibility restore, tray toggle"
```

---

## Task 3: 前端 API 封装

**Files:**
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: 追加类型与封装**

在 `src/lib/api/index.ts` 末尾追加：

```ts
export interface AppEntry {
  name: string;
  launch_path: string;
  target: string;
  args: string | null;
  is_custom: boolean;
  category: string | null;
  favorite: boolean;
}

export function appsScan(): Promise<AppEntry[]> {
  return call<AppEntry[]>("apps_scan");
}

export function appIcon(path: string): Promise<string | null> {
  return call<string | null>("app_icon", { path });
}

export function appLaunch(path: string): Promise<void> {
  return call<void>("app_launch", { path });
}

export function appAddDropped(path: string): Promise<void> {
  return call<void>("app_add_dropped", { path });
}

export function appRemoveCustom(target: string): Promise<void> {
  return call<void>("app_remove_custom", { target });
}

export function appSetFavorite(target: string, favorite: boolean): Promise<void> {
  return call<void>("app_set_favorite", { target, favorite });
}

export function appSetCategory(target: string, category: string | null): Promise<void> {
  return call<void>("app_set_category", { target, category });
}
```

并把已有的 `WidgetVisibility` 接口加上 `apps`：

```ts
export interface WidgetVisibility {
  todo: boolean;
  coins: boolean;
  apps: boolean;
}
```

且把 `widgetSetVisible` 的 kind 联合类型放宽：

```ts
export function widgetSetVisible(kind: "todo" | "coins" | "apps", visible: boolean): Promise<void> {
  return call<void>("widget_set_visible", { kind, visible });
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: Commit**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m4): frontend api for apps"
```

---

## Task 4: 主窗「应用」页 + 导航

**Files:**
- Create: `src/routes/(app)/apps/+page.svelte`
- Modify: `src/routes/(app)/+layout.svelte`

- [ ] **Step 1: 新建应用页**

新建 `src/routes/(app)/apps/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    appsScan,
    appIcon,
    appLaunch,
    appAddDropped,
    appRemoveCustom,
    appSetFavorite,
    appSetCategory,
    type AppEntry,
  } from "$lib/api";

  let apps = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let filter = $state<"all" | "favorite">("all");
  let categoryFilter = $state<string>("");
  let message = $state("");
  let unlisten: (() => void) | null = null;

  const categories = $derived(
    Array.from(new Set(apps.map((a) => a.category).filter((c): c is string => !!c))),
  );

  const shown = $derived(
    apps.filter((a) => {
      if (filter === "favorite" && !a.favorite) return false;
      if (categoryFilter && a.category !== categoryFilter) return false;
      return true;
    }),
  );

  async function refresh() {
    apps = await appsScan();
    for (const a of apps) {
      if (!(a.launch_path in icons)) {
        icons[a.launch_path] = null;
        appIcon(a.launch_path)
          .then((d) => (icons[a.launch_path] = d))
          .catch(() => (icons[a.launch_path] = null));
      }
    }
  }

  onMount(async () => {
    await refresh();
    const wv = getCurrentWebview();
    unlisten = await wv.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        for (const p of event.payload.paths) {
          try {
            await appAddDropped(p);
          } catch (e) {
            message = `添加失败 / Add failed: ${e}`;
          }
        }
        await refresh();
      }
    });
  });

  onDestroy(() => unlisten?.());

  async function launch(a: AppEntry) {
    try {
      await appLaunch(a.launch_path);
    } catch (e) {
      message = `启动失败 / Launch failed: ${e}`;
    }
  }

  async function toggleFav(a: AppEntry) {
    await appSetFavorite(a.target, !a.favorite);
    await refresh();
  }

  async function assignCategory(a: AppEntry) {
    const c = prompt("分类名 / Category (留空清除):", a.category ?? "");
    if (c === null) return;
    await appSetCategory(a.target, c.trim() === "" ? null : c.trim());
    await refresh();
  }

  async function remove(a: AppEntry) {
    await appRemoveCustom(a.target);
    await refresh();
  }

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<main class="container">
  <h1>应用 / Apps</h1>
  <p class="hint">把桌面图标拖到这里即可添加 / Drag desktop icons here to add.</p>

  <div class="filters">
    <button class:active={filter === "all"} onclick={() => (filter = "all")}>全部 / All</button>
    <button class:active={filter === "favorite"} onclick={() => (filter = "favorite")}>收藏 / Favorites</button>
    {#if categories.length}
      <select bind:value={categoryFilter}>
        <option value="">所有分类 / All categories</option>
        {#each categories as c}
          <option value={c}>{c}</option>
        {/each}
      </select>
    {/if}
  </div>

  {#if message}<p class="msg">{message}</p>{/if}

  <div class="grid">
    {#each shown as a (a.target)}
      <div class="card">
        <button class="icon-btn" onclick={() => launch(a)} title={a.target}>
          {#if icons[a.launch_path]}
            <img src={icons[a.launch_path]} alt={a.name} />
          {:else}
            <span class="placeholder">{initial(a.name)}</span>
          {/if}
          <span class="name">{a.name}</span>
        </button>
        <div class="row">
          <button class="ghost" onclick={() => toggleFav(a)} title="收藏 / Favorite">
            {a.favorite ? "★" : "☆"}
          </button>
          <button class="ghost" onclick={() => assignCategory(a)} title="分类 / Category">🏷️</button>
          {#if a.is_custom}
            <button class="ghost" onclick={() => remove(a)} title="移除 / Remove">🗑️</button>
          {/if}
        </div>
        {#if a.category}<span class="cat">{a.category}</span>{/if}
      </div>
    {/each}
  </div>
</main>

<style>
  .container { max-width: 900px; margin: 0 auto; padding: 1.5rem 1rem; }
  .hint { opacity: 0.7; font-size: 0.9em; }
  .filters { display: flex; gap: 0.5rem; align-items: center; margin: 0.75rem 0; flex-wrap: wrap; }
  .filters button, .filters select {
    border-radius: 8px; border: 1px solid var(--border);
    padding: 0.4em 0.8em; color: var(--fg); background: var(--surface); cursor: pointer;
  }
  .filters button.active { border-color: var(--fg); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 0.75rem; }
  .card {
    border: 1px solid var(--border); border-radius: 10px; padding: 0.6rem;
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
  }
  .icon-btn {
    display: flex; flex-direction: column; align-items: center; gap: 0.4rem;
    background: transparent; border: none; color: var(--fg); cursor: pointer; width: 100%;
  }
  .icon-btn img { width: 48px; height: 48px; object-fit: contain; }
  .placeholder {
    width: 48px; height: 48px; border-radius: 10px; background: var(--border);
    display: flex; align-items: center; justify-content: center; font-size: 1.4rem; font-weight: 700;
  }
  .name { font-size: 0.85em; text-align: center; word-break: break-word; }
  .row { display: flex; gap: 0.3rem; }
  .ghost { background: transparent; border: none; cursor: pointer; font-size: 1em; }
  .cat { font-size: 0.75em; opacity: 0.7; }
  .msg { opacity: 0.85; }
</style>
```

- [ ] **Step 2: 导航加链接**

在 `src/routes/(app)/+layout.svelte` 的 `<nav>` 内，`<a href="/settings">设置 / Settings</a>` 之前加：

```svelte
      <a href="/apps">应用 / Apps</a>
```

- [ ] **Step 3: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
git add "src/routes/(app)/apps/+page.svelte" "src/routes/(app)/+layout.svelte"
git commit -m "feat(m4): apps page with icons, drag-add, favorites, categories"
```

---

## Task 5: 应用 widget 页

**Files:**
- Create: `src/routes/(widget)/widgets/apps/+page.svelte`

- [ ] **Step 1: 新建 widget 页**

参考现有 `src/routes/(widget)/widgets/coins/+page.svelte` 的透明卡片与 `data-tauri-drag-region` 写法，新建 `src/routes/(widget)/widgets/apps/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { appsScan, appIcon, appLaunch, appAddDropped, type AppEntry } from "$lib/api";

  let favs = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let unlisten: (() => void) | null = null;

  async function refresh() {
    const all = await appsScan();
    favs = all.filter((a) => a.favorite);
    for (const a of favs) {
      if (!(a.launch_path in icons)) {
        icons[a.launch_path] = null;
        appIcon(a.launch_path).then((d) => (icons[a.launch_path] = d)).catch(() => {});
      }
    }
  }

  onMount(async () => {
    await refresh();
    const wv = getCurrentWebview();
    unlisten = await wv.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        for (const p of event.payload.paths) {
          try {
            await appAddDropped(p);
          } catch {
            /* ignore */
          }
        }
        await refresh();
      }
    });
  });

  onDestroy(() => unlisten?.());

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<div class="widget" data-tauri-drag-region>
  <div class="grid">
    {#each favs as a (a.target)}
      <button class="app" onclick={() => appLaunch(a.launch_path)} title={a.name}>
        {#if icons[a.launch_path]}
          <img src={icons[a.launch_path]} alt={a.name} />
        {:else}
          <span class="placeholder">{initial(a.name)}</span>
        {/if}
      </button>
    {/each}
    {#if favs.length === 0}
      <p class="empty">把图标拖进来 / Drag icons here</p>
    {/if}
  </div>
</div>

<style>
  :global(html), :global(body) { background: transparent !important; margin: 0; }
  .widget {
    background: rgba(20, 20, 20, 0.55);
    border-radius: 14px;
    padding: 0.6rem;
    height: 100vh;
    box-sizing: border-box;
    color: #fff;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
  }
  .grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.5rem; }
  .app { background: transparent; border: none; cursor: pointer; padding: 0.2rem; }
  .app img { width: 40px; height: 40px; object-fit: contain; }
  .placeholder {
    display: flex; width: 40px; height: 40px; border-radius: 8px;
    align-items: center; justify-content: center;
    background: rgba(255, 255, 255, 0.2); color: #fff; font-weight: 700;
  }
  .empty { font-size: 0.8em; opacity: 0.8; grid-column: 1 / -1; text-align: center; }
</style>
```

> 若现有 coins widget 的透明样式写法与上面不同，以现有写法为准（保持视觉一致）。

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: Commit**

```bash
git add "src/routes/(widget)/widgets/apps/+page.svelte"
git commit -m "feat(m4): apps quick-launch widget"
```

---

## Task 6: 全量验证

- [ ] **Step 1:** `cd src-tauri; cargo test` → 全过（M3 的 + M4 新增 db/apps 测试）。
- [ ] **Step 2:** `cd src-tauri; cargo clippy -- -D warnings` → 干净。
- [ ] **Step 3:** `npm run check` → 0 errors。
- [ ] **Step 4: 报告** —— 测试数/clippy/check 结果，列出需用户手动验证项：
  - 主窗「应用」页：自动列出桌面快捷方式 + 图标显示
  - 拖桌面图标到主窗页 → 加入列表
  - 收藏/分类/筛选/启动
  - 托盘开关「应用 widget」→ 透明 widget 出现，展示收藏，点击启动
  - widget 内拖入（加分项，若 NOACTIVATE 下不可用则记录，主窗页拖入为准）

---

## 自检 / Self-Review 结论

- **Spec 覆盖：** 扫描(shortcuts.scan/Task1)、图标(icon_data_url/Task1+前端懒加载)、拖拽补充(app_add_dropped/Task4+5)、收藏+分类(app_prefs/Task1,4)、启动(launch/Task1)、widget(Task2,5)、主窗页(Task4) —— 全覆盖。
- **占位符：** 无 TBD；所有步骤含完整代码或对现有同构代码的明确照搬指引（tray/coins-widget）。
- **类型一致：** `AppEntry`(后端/前端字段一致：name/launch_path/target/args/is_custom/category/favorite)；`merge/add_custom/prefs_map/set_favorite/set_category` 跨 db/commands 签名一致；`WidgetVisibility.apps` 在 models/window/read_visibility/lib-fallback/前端接口处一致补齐。
- **依赖顺序：** 预置 shortcuts.rs/Cargo.toml/system-mod 已编译验证；Task1 消费它们使 clippy 转绿（首个绿提交）；Task1 改 models.WidgetVisibility 与 Task2 改 read_visibility 有耦合 —— Task1 Step7 已注明若编译报错就并入 Task2 Step1。

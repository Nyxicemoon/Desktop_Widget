# DeskHub M4 widget-rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`).

**Goal:** 把 M4 改为「桌面半透明 widget 即图标管理面板」：策展式（仅拖入显示）、可移动可缩放、普通模式点击启动、编辑模式可移除/拖拽排序/重命名。删除主窗「应用」页，简化后端。

**Context:** M4 第一版已在 `m4-icons` 分支实现（commits d2bcae7..6b4c20b）。本计划在其上**修改**，不是从零。Win32 模块 `system/shortcuts.rs` 的 `parse_lnk`/`resolve_dropped`/`launch`/`icon_data_url` 已验证，保持不动；仅删除其中的 `scan`/`desktop_dirs`。

**Tech Stack:** Rust + Tauri v2、rusqlite、SvelteKit + TS（`@tauri-apps/api/window` startResizeDragging + drag-region、`@tauri-apps/api/webview` onDragDropEvent）。

**参考 spec：** `docs/superpowers/specs/2026-06-16-deskhub-m4-icons-design.md`（见第十一节「修订」）。

**质量门禁（每任务末）：** `src-tauri/` 下 `cargo test`、`cargo clippy -- -D warnings`；前端 `npm run check`。
> 注意 npm：本机 node 经 nvm，若 `npm` 不在 PATH，先 `$env:Path = "C:\nvm4w\nodejs;" + $env:Path`（PowerShell）。

---

## Task 1: 简化数据模型与后端（migration / models / db / shortcuts / commands / lib）

> 一次提交完成自洽的后端简化，保持编译与 clippy 绿。

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/db/apps.rs`
- Modify: `src-tauri/src/system/shortcuts.rs`
- Modify: `src-tauri/src/commands/apps.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 重写 migration v4（删 app_prefs，custom_apps 加 sort_order）**

在 `src-tauri/src/db/migrations.rs` 把 `(4, "...")` 这一项整体替换为：

```rust
    (
        4,
        "CREATE TABLE custom_apps (
            id         INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            target     TEXT NOT NULL,
            args       TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ),
```

并把 `applies_migrations_on_empty_db` 测试里的表清单中去掉 `"app_prefs"`（保留 `"custom_apps"`）：即
`["kv", "todos", "game_profile", "coin_ledger", "backgrounds", "custom_apps"]`。

- [ ] **Step 2: 简化 `models::AppEntry`**

在 `src-tauri/src/models/mod.rs` 把现有 `AppEntry` 整体替换为：

```rust
#[derive(Debug, Serialize)]
pub struct AppEntry {
    pub id: i64,
    pub name: String,
    pub target: String,
    pub args: Option<String>,
}
```

（`WidgetVisibility` 保持含 `apps` 字段，不改。）

- [ ] **Step 3: 重写 `src-tauri/src/db/apps.rs`**

整体替换文件内容为：

```rust
use crate::error::AppResult;
use crate::models::AppEntry;
use rusqlite::Connection;

/// All curated apps, ordered.
pub fn list(conn: &Connection) -> AppResult<Vec<AppEntry>> {
    let mut stmt =
        conn.prepare("SELECT id, name, target, args FROM custom_apps ORDER BY sort_order, id")?;
    let rows = stmt.query_map([], |r| {
        Ok(AppEntry {
            id: r.get(0)?,
            name: r.get(1)?,
            target: r.get(2)?,
            args: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Add a curated app (de-dup by lowercased target); appended at the end.
pub fn add(conn: &Connection, name: &str, target: &str, args: Option<&str>) -> AppResult<()> {
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
    let next: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM custom_apps",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO custom_apps (name, target, args, sort_order) VALUES (?1, ?2, ?3, ?4)",
        (name, target, args, next),
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM custom_apps WHERE id = ?1", [id])?;
    Ok(())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> AppResult<()> {
    conn.execute("UPDATE custom_apps SET name = ?1 WHERE id = ?2", (name, id))?;
    Ok(())
}

/// Persist a new order: sort_order = position in `ids`.
pub fn reorder(conn: &mut Connection, ids: &[i64]) -> AppResult<()> {
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE custom_apps SET sort_order = ?1 WHERE id = ?2",
            (i as i64, id),
        )?;
    }
    tx.commit()?;
    Ok(())
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

    #[test]
    fn add_dedups_by_target_case_insensitive() {
        let conn = setup();
        add(&conn, "App", "C:\\a\\app.exe", None).unwrap();
        add(&conn, "App2", "C:\\A\\APP.EXE", None).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn add_appends_in_order_then_reorder() {
        let mut conn = setup();
        add(&conn, "A", "C:\\a.exe", None).unwrap();
        add(&conn, "B", "C:\\b.exe", None).unwrap();
        add(&conn, "C", "C:\\c.exe", None).unwrap();
        let l = list(&conn).unwrap();
        assert_eq!(l.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);

        let ids: Vec<i64> = vec![l[2].id, l[0].id, l[1].id]; // C, A, B
        reorder(&mut conn, &ids).unwrap();
        let l2 = list(&conn).unwrap();
        assert_eq!(l2.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["C", "A", "B"]);
    }

    #[test]
    fn rename_and_remove() {
        let conn = setup();
        add(&conn, "Old", "C:\\a.exe", None).unwrap();
        let id = list(&conn).unwrap()[0].id;
        rename(&conn, id, "New").unwrap();
        assert_eq!(list(&conn).unwrap()[0].name, "New");
        remove(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
```

- [ ] **Step 4: 从 `system/shortcuts.rs` 移除 `scan` 与 `desktop_dirs`**

在 `src-tauri/src/system/shortcuts.rs` 中删除：
- `#[cfg(target_os = "windows")] fn desktop_dirs() -> ... { ... }` 整个函数。
- `#[cfg(target_os = "windows")] pub fn scan() -> AppResult<Vec<ShortcutRaw>> { ... }` 整个函数。
- 文件末尾 `#[cfg(not(target_os = "windows"))] pub fn scan() ...` 的非 windows stub。

保留：`ShortcutRaw`、`wide`、`from_wide`、`parse_lnk`、`resolve_dropped`、`launch`、`icon_data_url` 及它们的非 windows stub。
> 删除后 `parse_lnk` 仍被 `resolve_dropped` 使用，`wide`/`from_wide` 仍被多处使用，不会产生死代码。

- [ ] **Step 5: 重写 `src-tauri/src/commands/apps.rs`**

整体替换为：

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
pub fn app_list(db: State<Db>) -> AppResult<Vec<AppEntry>> {
    let conn = lock(&db)?;
    apps::list(&conn)
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
    apps::add(&conn, &r.name, &r.target, r.args.as_deref())
}

#[tauri::command]
pub fn app_remove(db: State<Db>, id: i64) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::remove(&conn, id)
}

#[tauri::command]
pub fn app_rename(db: State<Db>, id: i64, name: String) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::rename(&conn, id, &name)
}

#[tauri::command]
pub fn app_reorder(db: State<Db>, ids: Vec<i64>) -> AppResult<()> {
    let mut conn = lock(&db)?;
    apps::reorder(&mut conn, &ids)
}
```

- [ ] **Step 6: 更新 `lib.rs` 命令注册**

在 `src-tauri/src/lib.rs` 的 `generate_handler![...]` 中，把原 M4 的七个命令
（`commands::apps::apps_scan` / `app_icon` / `app_launch` / `app_add_dropped` / `app_remove_custom` / `app_set_favorite` / `app_set_category`）
替换为：

```rust
            commands::apps::app_list,
            commands::apps::app_icon,
            commands::apps::app_launch,
            commands::apps::app_add_dropped,
            commands::apps::app_remove,
            commands::apps::app_rename,
            commands::apps::app_reorder,
```

- [ ] **Step 7: 门禁**

Run（PowerShell，必要时先加 nvm 到 PATH）: `cd src-tauri; cargo test; cargo clippy -- -D warnings`
Expected: 测试全过（apps 新测试 + 迁移表清单不含 app_prefs）；clippy 干净。

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db src-tauri/src/models src-tauri/src/system src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "refactor(m4): curated app model — drop scan/prefs, add ordering/rename"
```

---

## Task 2: widget 可缩放

**Files:**
- Modify: `src-tauri/src/window/mod.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: apps widget 可缩放，调大默认尺寸**

在 `src-tauri/src/window/mod.rs` 把 apps 的 `widget_config` 分支改为更适合面板的尺寸：

```rust
        "apps" => Ok(("widget-apps", "/widgets/apps", 360.0, 280.0, 40.0, 420.0)),
```

并在 `open_widget` 中，把构建器的 `.resizable(false)` 改为按 kind 决定：

```rust
        .resizable(kind == "apps")
```

（其余构建参数不变。）

- [ ] **Step 2: 允许缩放拖拽权限**

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组追加（保持其它项）：

```json
    "core:window:allow-start-resize-dragging"
```

- [ ] **Step 3: 门禁**

Run: `cd src-tauri; cargo build`
Expected: 通过（conf/capabilities schema 校验）。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/window/mod.rs src-tauri/capabilities/default.json
git commit -m "feat(m4): make apps widget resizable + resize-drag permission"
```

---

## Task 3: 前端 API 调整

**Files:**
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: 替换 apps 相关封装**

在 `src/lib/api/index.ts` 中，把原 M4 追加的 `AppEntry` 接口与七个函数（`appsScan`/`appIcon`/`appLaunch`/`appAddDropped`/`appRemoveCustom`/`appSetFavorite`/`appSetCategory`）整段替换为：

```ts
export interface AppEntry {
  id: number;
  name: string;
  target: string;
  args: string | null;
}

export function appList(): Promise<AppEntry[]> {
  return call<AppEntry[]>("app_list");
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

export function appRemove(id: number): Promise<void> {
  return call<void>("app_remove", { id });
}

export function appRename(id: number, name: string): Promise<void> {
  return call<void>("app_rename", { id, name });
}

export function appReorder(ids: number[]): Promise<void> {
  return call<void>("app_reorder", { ids });
}
```

（`WidgetVisibility` 含 `apps` 与 `widgetSetVisible` 的 `"todo"|"coins"|"apps"` 联合类型保持不变。）

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: Commit**

```bash
git add src/lib/api/index.ts
git commit -m "feat(m4): frontend api for curated apps (list/add/remove/rename/reorder)"
```

---

## Task 4: 删除主窗「应用」页

**Files:**
- Delete: `src/routes/(app)/apps/+page.svelte`
- Modify: `src/routes/(app)/+layout.svelte`

- [ ] **Step 1: 删除页面与导航链接**

```bash
git rm "src/routes/(app)/apps/+page.svelte"
```

在 `src/routes/(app)/+layout.svelte` 的 `<nav>` 中删除这一行：

```svelte
      <a href="/apps">应用 / Apps</a>
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: Commit**

```bash
git add "src/routes/(app)/+layout.svelte"
git commit -m "feat(m4): remove main-window apps page (widget is the manager)"
```

---

## Task 5: 重写应用 widget 为管理面板

**Files:**
- Modify (整体替换): `src/routes/(widget)/widgets/apps/+page.svelte`

- [ ] **Step 1: 整体替换 widget 页内容**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow, ResizeDirection } from "@tauri-apps/api/window";
  import {
    appList,
    appIcon,
    appLaunch,
    appAddDropped,
    appRemove,
    appRename,
    appReorder,
    type AppEntry,
  } from "$lib/api";

  let apps = $state<AppEntry[]>([]);
  let icons = $state<Record<string, string | null>>({});
  let edit = $state(false);
  let renamingId = $state<number | null>(null);
  let renameText = $state("");
  let dragIndex = $state<number | null>(null);
  let unlisten: (() => void) | null = null;

  async function refresh() {
    apps = await appList();
    for (const a of apps) {
      if (!(a.target in icons)) {
        icons[a.target] = null;
        appIcon(a.target)
          .then((d) => (icons[a.target] = d))
          .catch(() => (icons[a.target] = null));
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

  async function onClickApp(a: AppEntry) {
    if (edit) return;
    try {
      await appLaunch(a.target);
    } catch {
      /* ignore */
    }
  }

  async function remove(a: AppEntry) {
    await appRemove(a.id);
    await refresh();
  }

  function beginRename(a: AppEntry) {
    renamingId = a.id;
    renameText = a.name;
  }

  async function commitRename(a: AppEntry) {
    const name = renameText.trim();
    renamingId = null;
    if (name && name !== a.name) {
      await appRename(a.id, name);
      await refresh();
    }
  }

  function onDragStart(i: number) {
    dragIndex = i;
  }

  async function onDrop(j: number) {
    if (dragIndex === null || dragIndex === j) {
      dragIndex = null;
      return;
    }
    const next = [...apps];
    const [moved] = next.splice(dragIndex, 1);
    next.splice(j, 0, moved);
    apps = next;
    dragIndex = null;
    await appReorder(next.map((a) => a.id));
  }

  function startResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    void getCurrentWindow().startResizeDragging(ResizeDirection.SouthEast);
  }

  function initial(name: string): string {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<div class="widget">
  <div class="header" data-tauri-drag-region>
    <span class="dots" data-tauri-drag-region>⋮⋮</span>
    <button class="edit-toggle" onclick={() => (edit = !edit)} title="编辑 / Edit">
      {edit ? "✓" : "✎"}
    </button>
  </div>

  <div class="grid">
    {#each apps as a, i (a.id)}
      <div
        class="app"
        class:editing={edit}
        draggable={edit}
        ondragstart={() => onDragStart(i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => onDrop(i)}
        role="button"
        tabindex="0"
      >
        {#if edit}
          <button class="del" onclick={() => remove(a)} title="移除 / Remove">✕</button>
        {/if}
        <button class="icon" onclick={() => onClickApp(a)} title={a.target}>
          {#if icons[a.target]}
            <img src={icons[a.target]} alt={a.name} />
          {:else}
            <span class="placeholder">{initial(a.name)}</span>
          {/if}
        </button>
        {#if renamingId === a.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameText}
            autofocus
            onblur={() => commitRename(a)}
            onkeydown={(e) => e.key === "Enter" && commitRename(a)}
          />
        {:else}
          <span
            class="name"
            ondblclick={() => edit && beginRename(a)}
            role="textbox"
            tabindex="-1"
          >{a.name}</span>
        {/if}
      </div>
    {/each}
    {#if apps.length === 0}
      <p class="empty">把桌面图标拖进来<br />Drag desktop icons here</p>
    {/if}
  </div>

  <div
    class="resize-grip"
    onmousedown={startResize}
    role="presentation"
    title="缩放 / Resize"
  ></div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
  }
  .widget {
    position: relative;
    height: 100vh;
    box-sizing: border-box;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 14px;
    padding: 0.4rem;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
    user-select: none;
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 22px;
    cursor: move;
  }
  .dots {
    opacity: 0.5;
    font-size: 0.8rem;
  }
  .edit-toggle {
    background: transparent;
    border: none;
    color: #fff;
    cursor: pointer;
    opacity: 0.8;
    font-size: 0.9rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(64px, 1fr));
    gap: 0.4rem;
    overflow-y: auto;
    height: calc(100% - 22px);
    align-content: start;
  }
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
    padding: 0.2rem;
    border-radius: 8px;
  }
  .app.editing {
    background: rgba(255, 255, 255, 0.08);
    cursor: grab;
  }
  .icon {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
  }
  .icon img {
    width: 40px;
    height: 40px;
    object-fit: contain;
  }
  .placeholder {
    display: flex;
    width: 40px;
    height: 40px;
    border-radius: 8px;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.2);
    color: #fff;
    font-weight: 700;
  }
  .name {
    font-size: 0.68rem;
    text-align: center;
    word-break: break-word;
    max-width: 100%;
  }
  .rename {
    width: 90%;
    font-size: 0.68rem;
    border: none;
    border-radius: 4px;
    padding: 1px 2px;
  }
  .del {
    position: absolute;
    top: -2px;
    right: -2px;
    z-index: 2;
    width: 16px;
    height: 16px;
    line-height: 14px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #e0533d;
    color: #fff;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .empty {
    grid-column: 1 / -1;
    text-align: center;
    opacity: 0.8;
    font-size: 0.8rem;
    margin-top: 1rem;
  }
  .resize-grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, rgba(255, 255, 255, 0.5) 50%);
    border-bottom-right-radius: 14px;
  }
</style>
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。
> 若 `ResizeDirection` 导入报类型错误，确认从 `@tauri-apps/api/window` 导入；若版本无该枚举，改用 `getCurrentWindow().startResizeDragging("SouthEast" as never)` 并在报告中注明。

- [ ] **Step 3: Commit**

```bash
git add "src/routes/(widget)/widgets/apps/+page.svelte"
git commit -m "feat(m4): widget as app manager — drag-add, launch, edit/remove/reorder/rename"
```

---

## Task 6: 全量验证

- [ ] **Step 1:** `cd src-tauri; cargo test` → 全过。
- [ ] **Step 2:** `cd src-tauri; cargo clippy -- -D warnings` → 干净。
- [ ] **Step 3:** `npm run check` → 0 errors。
- [ ] **Step 4: 报告** —— 测试数/clippy/check 结果、任何偏离、以及手动验证清单：
  - 托盘开关「应用 widget」→ 出现半透明面板
  - 从桌面拖 `.lnk`/`.exe` 进面板 → 图标出现（图标提取是否成功）
  - 普通模式点击 → 启动
  - ✎ 进入编辑：✕ 移除、拖拽排序、双击重命名
  - 拖动 header 移动窗口；拖右下角手柄缩放；重启后尺寸/位置保留

---

## 自检 / Self-Review 结论

- **Spec(修订节)覆盖：** 策展式(add/list)、可缩放(Task2+resize-grip)、可移动(header drag-region)、点击启动(app_launch)、编辑模式移除/排序/重命名(Task5 + db reorder/rename/remove)、删主窗页(Task4)、删 scan/prefs(Task1) —— 全覆盖。
- **占位符：** 无。改动文件均给整体替换代码。
- **类型一致：** `AppEntry{id,name,target,args}` 后端/前端一致；命令 `app_list/app_add_dropped/app_remove/app_rename/app_reorder/app_icon/app_launch` 在 db/commands/lib/前端 api/widget 五处签名一致；`reorder(&mut Connection)` 用事务，commands 用 `let mut conn`。
- **依赖顺序：** Task1 自洽（删 scan 后 parse_lnk 仍被 resolve_dropped 用，无死代码）；Task3 前端 api 改名后 Task4/5 才引用新名，顺序正确；widget 用到的 `ResizeDirection` 有降级注记。

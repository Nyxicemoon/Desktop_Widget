# DeskHub 桌面透明 Widget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 Todo 与金币做成透明、可拖动、坐在普通窗口之下仍可交互的桌面小组件，主窗口保留作管理；显隐与位置持久化。

**Architecture:** Tauri 多窗口：主窗口（不透明）+ 运行时创建的透明 widget 窗口；用 `windows-rs` 把 widget 设为 bottommost + 不抢焦点（spike）；位置用 `tauri-plugin-window-state`，显隐存 `kv`；前端用路由分组 `(app)`/`(widget)` 分离不透明主界面与透明 widget。

**Tech Stack:** Rust + Tauri v2 + windows-rs + tauri-plugin-window-state + rusqlite；前端 SvelteKit + Svelte 5 + TS。

> **编译前置：** `cargo` 前确保 `build/` 存在。cargo 用 `"$USERPROFILE/.cargo/bin/cargo.exe" --manifest-path src-tauri/Cargo.toml`。
> **GUI 性质：** 透明/z-order/拖动属手动验收；自动化只覆盖显隐 kv 逻辑。
> **Win32 版本偏差：** Task 3 是 spike——若 `windows` crate 的 `SetWindowPos`/`HWND` 签名与下方略有出入，按编译器提示微调，保持意图：`WS_EX_NOACTIVATE` + `SetWindowPos(HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE)`。

---

## 文件结构

后端：
- `Cargo.toml`（改）— `tauri-plugin-window-state`、`windows`。
- `src-tauri/tauri.conf.json`（改）— 主窗口 `label:"main"`。
- `models/mod.rs`（改）— `WidgetVisibility`。
- `window/mod.rs`（新）— `widget_config`、`read_visibility`、`pin_to_desktop`、`open_widget`、`close_widget`。
- `commands/widget.rs`（新）— `widget_set_visible`、`widget_get_visibility`。
- `commands/mod.rs`、`lib.rs`（改）— 声明 window/commands、注册插件与命令、setup 恢复 widget。

前端：
- `src/app.css`（改）— body 透明 + `.app-shell` 不透明壳。
- `src/routes/+layout.svelte`（改）— 极简根布局。
- `src/routes/(app)/+layout.svelte`（新，承接原导航/背景）、`(app)/+page.svelte`、`(app)/backgrounds/+page.svelte`（移动）。
- `src/routes/(widget)/+layout.svelte`（新，透明）、`(widget)/widgets/todo/+page.svelte`、`(widget)/widgets/coins/+page.svelte`（新）。
- `src/lib/api/index.ts`（改）— `widgetSetVisible`/`widgetGetVisibility` + 类型。

---

## Task 1: 依赖 + 插件 + 主窗口 label

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`

- [ ] **Step 1: 加依赖**

`src-tauri/Cargo.toml` 的 `[dependencies]` 末尾追加：

```toml
tauri-plugin-window-state = "2"
windows = { version = "0.61", features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging"] }
```

- [ ] **Step 2: 主窗口 label**

`src-tauri/tauri.conf.json` 的 `app.windows[0]` 对象里加 `"label": "main",`（放在 `"title"` 前）。

- [ ] **Step 3: 注册 window-state 插件**

`src-tauri/src/lib.rs` 的 builder 链中，在 `.plugin(tauri_plugin_opener::init())` 之后加一行：

```rust
        .plugin(tauri_plugin_window_state::Builder::default().build())
```

- [ ] **Step 4: 编译检查（首次拉取 windows/插件）**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/src/lib.rs
git commit -m "feat(widgets): add window-state plugin and windows crate"
```

---

## Task 2: WidgetVisibility 模型 + window 模块（纯逻辑）

**Files:** Modify `src-tauri/src/models/mod.rs`; Create `src-tauri/src/window/mod.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 模型**

`src-tauri/src/models/mod.rs` 末尾追加：

```rust
#[derive(Debug, Serialize)]
pub struct WidgetVisibility {
    pub todo: bool,
    pub coins: bool,
}
```

- [ ] **Step 2: 声明模块**

`src-tauri/src/lib.rs` 顶部模块声明区加入（保持顺序）：

```rust
mod window;
```

- [ ] **Step 3: window 模块（widget_config + read_visibility）+ 测试**

创建 `src-tauri/src/window/mod.rs`：

```rust
use crate::db::kv;
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use rusqlite::Connection;

/// (window label, frontend route, default width, default height)
pub fn widget_config(kind: &str) -> AppResult<(&'static str, &'static str, f64, f64)> {
    match kind {
        "todo" => Ok(("widget-todo", "/widgets/todo", 280.0, 360.0)),
        "coins" => Ok(("widget-coins", "/widgets/coins", 200.0, 90.0)),
        other => Err(AppError::Other(format!("unknown widget kind: {other}"))),
    }
}

pub fn read_visibility(conn: &Connection) -> AppResult<WidgetVisibility> {
    Ok(WidgetVisibility {
        todo: kv::get(conn, "widget.todo.visible")?.as_deref() == Some("1"),
        coins: kv::get(conn, "widget.coins.visible")?.as_deref() == Some("1"),
    })
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
    fn visibility_defaults_false() {
        let conn = setup();
        let v = read_visibility(&conn).unwrap();
        assert!(!v.todo);
        assert!(!v.coins);
    }

    #[test]
    fn visibility_reflects_kv() {
        let conn = setup();
        kv::set(&conn, "widget.todo.visible", "1").unwrap();
        let v = read_visibility(&conn).unwrap();
        assert!(v.todo);
        assert!(!v.coins);
    }

    #[test]
    fn widget_config_unknown_errors() {
        assert!(widget_config("nope").is_err());
        assert_eq!(widget_config("todo").unwrap().0, "widget-todo");
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml window::`
Expected: 3 测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/models/mod.rs src-tauri/src/window/mod.rs src-tauri/src/lib.rs
git commit -m "feat(widgets): add WidgetVisibility and widget config/visibility logic"
```

---

## Task 3: 桌面钉层 + 开关窗口（SPIKE，Win32）

**Files:** Modify `src-tauri/src/window/mod.rs`

- [ ] **Step 1: 加入 pin_to_desktop / open_widget / close_widget**

在 `src-tauri/src/window/mod.rs` 顶部 `use` 区追加：

```rust
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
```

在 `read_visibility` 函数之后、`#[cfg(test)]` 之前插入：

```rust
pub fn open_widget(app: &AppHandle, kind: &str) -> AppResult<()> {
    let (label, route, w, h) = widget_config(kind)?;
    if let Some(win) = app.get_webview_window(label) {
        win.show().map_err(|e| AppError::Other(e.to_string()))?;
        pin_to_desktop(&win)?;
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, label, WebviewUrl::App(route.into()))
        .transparent(true)
        .decorations(false)
        .skip_taskbar(true)
        .shadow(false)
        .always_on_top(false)
        .resizable(false)
        .inner_size(w, h)
        .build()
        .map_err(|e| AppError::Other(e.to_string()))?;
    pin_to_desktop(&win)?;
    Ok(())
}

pub fn close_widget(app: &AppHandle, kind: &str) -> AppResult<()> {
    let (label, _, _, _) = widget_config(kind)?;
    if let Some(win) = app.get_webview_window(label) {
        win.close().map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn pin_to_desktop(win: &WebviewWindow) -> AppResult<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_BOTTOM,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_NOACTIVATE,
    };
    let raw = win.hwnd().map_err(|e| AppError::Other(e.to_string()))?;
    let hwnd = HWND(raw.0);
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 as isize));
        SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn pin_to_desktop(_win: &WebviewWindow) -> AppResult<()> {
    Ok(())
}
```

> **Spike 说明：** 若 `windows` 0.61 的 `SetWindowPos` 第二参不是 `Option<HWND>`（直接 `HWND`），把 `Some(HWND_BOTTOM)` 改为 `HWND_BOTTOM`；若 `HWND(raw.0)` 类型不匹配，用 `HWND(win.hwnd()?.0 as _)`。目标不变：设 `WS_EX_NOACTIVATE` + 置底不激活。

- [ ] **Step 2: 编译检查（关键 spike 验证）**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（`open_widget`/`close_widget` 暂未被调用，dead_code 警告正常）。若因 Win32 签名报错，按 Step 1 注释微调直至通过。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/window/mod.rs
git commit -m "feat(widgets): add transparent widget windows pinned to desktop (win32)"
```

---

## Task 4: 命令 + 注册 + 启动恢复 + 后端门禁

**Files:** Create `src-tauri/src/commands/widget.rs`; Modify `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: 命令**

创建 `src-tauri/src/commands/widget.rs`：

```rust
use crate::db::{kv, Db};
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use crate::window;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn widget_set_visible(
    app: AppHandle,
    db: State<Db>,
    kind: String,
    visible: bool,
) -> AppResult<()> {
    if visible {
        window::open_widget(&app, &kind)?;
    } else {
        window::close_widget(&app, &kind)?;
    }
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    kv::set(
        &conn,
        &format!("widget.{kind}.visible"),
        if visible { "1" } else { "0" },
    )
}

#[tauri::command]
pub fn widget_get_visibility(db: State<Db>) -> AppResult<WidgetVisibility> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    window::read_visibility(&conn)
}
```

把 `src-tauri/src/commands/mod.rs` 整个替换为：

```rust
pub mod backgrounds;
pub mod game;
pub mod kv;
pub mod todos;
pub mod widget;
```

- [ ] **Step 2: 注册命令**

`src-tauri/src/lib.rs` 的 `generate_handler!` 末尾（`bg_restore_default` 之后）追加：

```rust
            ,
            commands::widget::widget_set_visible,
            commands::widget::widget_get_visibility
```

（即在 `commands::backgrounds::bg_restore_default` 后补逗号并加两行。）

- [ ] **Step 3: 启动时恢复 widget**

把 `src-tauri/src/lib.rs` 的 `.setup(...)` 闭包整体替换为：

```rust
        .setup(|app| {
            let conn = db::open(app.handle())?;
            app.manage(db::Db(std::sync::Mutex::new(conn)));

            let vis = {
                let state = app.state::<db::Db>();
                let conn = state
                    .0
                    .lock()
                    .map_err(|e| e.to_string())?;
                window::read_visibility(&conn).unwrap_or(crate::models::WidgetVisibility {
                    todo: false,
                    coins: false,
                })
            };
            if vis.todo {
                let _ = window::open_widget(app.handle(), "todo");
            }
            if vis.coins {
                let _ = window::open_widget(app.handle(), "coins");
            }
            Ok(())
        })
```

- [ ] **Step 4: 全量测试 + lint**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
"$USERPROFILE/.cargo/bin/cargo.exe" clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```
Expected: 全部测试 PASS（M0–M2 的 20 + 本里程碑 3 = 23）；clippy 无警告。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs
git commit -m "feat(widgets): add widget visibility commands and startup restore"
```

---

## Task 5: 前端 API 封装

**Files:** Modify `src/lib/api/index.ts`

- [ ] **Step 1: 增加类型与封装**

`src/lib/api/index.ts` 末尾追加：

```ts
export interface WidgetVisibility {
  todo: boolean;
  coins: boolean;
}

export function widgetSetVisible(kind: "todo" | "coins", visible: boolean): Promise<void> {
  return call<void>("widget_set_visible", { kind, visible });
}

export function widgetGetVisibility(): Promise<WidgetVisibility> {
  return call<WidgetVisibility>("widget_get_visibility");
}
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: 提交**

```bash
git add src/lib/api/index.ts
git commit -m "feat(widgets): add widget visibility api wrappers"
```

---

## Task 6: 前端布局重构（透明基底 + (app) 分组）

**Files:** Modify `src/app.css`, `src/routes/+layout.svelte`; Create `src/routes/(app)/+layout.svelte`; Move `src/routes/+page.svelte` → `src/routes/(app)/+page.svelte`; Move `src/routes/backgrounds/+page.svelte` → `src/routes/(app)/backgrounds/+page.svelte`

- [ ] **Step 1: app.css 改透明基底**

把 `src/app.css` 里的 `html, body { ... }` 整块替换为：

```css
html,
body {
  margin: 0;
  background: transparent;
  color: var(--fg);
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
}

.app-shell {
  min-height: 100vh;
  background: var(--bg);
  color: var(--fg);
  transition:
    background 0.2s,
    color 0.2s;
}
```

（保留文件中已有的 `:root`、`:root[data-theme="dark"]`、`.bg-layer`、`.bg-layer::after` 规则不动。）

- [ ] **Step 2: 根布局改极简**

把 `src/routes/+layout.svelte` 整个替换为：

```svelte
<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

{@render children()}
```

- [ ] **Step 3: 移动主界面页面到 (app) 分组**

- 将 `src/routes/+page.svelte` 移动到 `src/routes/(app)/+page.svelte`。
- 将 `src/routes/backgrounds/+page.svelte` 移动到 `src/routes/(app)/backgrounds/+page.svelte`。

命令：
```bash
mkdir -p "src/routes/(app)/backgrounds"
git mv "src/routes/+page.svelte" "src/routes/(app)/+page.svelte"
git mv "src/routes/backgrounds/+page.svelte" "src/routes/(app)/backgrounds/+page.svelte"
```

- [ ] **Step 4: (app) 分组布局（承接导航 + 背景 + 不透明壳）**

创建 `src/routes/(app)/+layout.svelte`：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";
  import { coins, refreshCoins } from "$lib/stores/game";
  import { currentBg, loadBackground } from "$lib/stores/background";

  let { children } = $props();

  onMount(() => {
    void initTheme();
    void refreshCoins();
    void loadBackground();
  });
</script>

{#if $currentBg}
  <div class="bg-layer" style:background-image={`url(${$currentBg.data_url})`}></div>
{/if}

<div class="app-shell">
  <header class="bar">
    <nav>
      <a href="/">待办 / Todos</a>
      <a href="/backgrounds">背景 / Backgrounds</a>
    </nav>
    <span class="grow"></span>
    <span class="coins">🪙 {$coins}</span>
    <button class="ghost" onclick={toggleTheme} title="主题 / Theme">
      {$theme === "dark" ? "🌙" : "☀️"}
    </button>
  </header>

  {@render children()}
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
  }

  nav {
    display: flex;
    gap: 1rem;
  }

  nav a {
    color: var(--fg);
    text-decoration: none;
    opacity: 0.8;
  }

  nav a:hover {
    opacity: 1;
  }

  .grow {
    flex: 1;
  }

  .coins {
    font-weight: 600;
  }

  .ghost {
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
    padding: 0.3em 0.5em;
    border-radius: 8px;
  }
</style>
```

- [ ] **Step 5: 类型检查（主窗口仍工作）**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 6: 提交**

```bash
git add src/app.css "src/routes/+layout.svelte" "src/routes/(app)"
git commit -m "feat(widgets): transparent base + (app) layout group for main window"
```

---

## Task 7: Widget 路由（透明布局 + Todo/金币 widget）

**Files:** Create `src/routes/(widget)/+layout.svelte`, `src/routes/(widget)/widgets/todo/+page.svelte`, `src/routes/(widget)/widgets/coins/+page.svelte`

- [ ] **Step 1: 透明 widget 布局**

创建 `src/routes/(widget)/+layout.svelte`：

```svelte
<script lang="ts">
  let { children } = $props();
</script>

{@render children()}

<style>
  :global(body) {
    background: transparent;
  }
</style>
```

- [ ] **Step 2: Todo widget**

创建 `src/routes/(widget)/widgets/todo/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { todos, loadTodos, toggleTodo } from "$lib/stores/todos";

  onMount(() => {
    void loadTodos();
  });

  async function onToggle(id: number) {
    await toggleTodo(id);
  }
</script>

<div class="widget">
  <div class="head" data-tauri-drag-region>📋 今日 / Today</div>
  <ul>
    {#each $todos as todo (todo.id)}
      <li>
        <input type="checkbox" checked={todo.done} onchange={() => onToggle(todo.id)} />
        <span class:done={todo.done}>{todo.title}</span>
      </li>
    {/each}
    {#if $todos.length === 0}
      <li class="empty">无任务 / Empty</li>
    {/if}
  </ul>
</div>

<style>
  .widget {
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 12px;
    padding: 0.5rem 0.7rem;
    backdrop-filter: blur(6px);
    overflow: hidden;
  }

  .head {
    font-weight: 600;
    padding: 0.2rem 0.1rem 0.4rem;
    cursor: move;
    user-select: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0;
  }

  .done {
    text-decoration: line-through;
    opacity: 0.6;
  }

  .empty {
    opacity: 0.6;
    justify-content: center;
  }
</style>
```

- [ ] **Step 3: 金币 widget**

创建 `src/routes/(widget)/widgets/coins/+page.svelte`：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { coins, refreshCoins } from "$lib/stores/game";

  onMount(() => {
    void refreshCoins();
  });
</script>

<div class="widget" data-tauri-drag-region>🪙 {$coins}</div>

<style>
  .widget {
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(20, 20, 20, 0.55);
    color: #fff;
    border-radius: 12px;
    font-size: 1.4rem;
    font-weight: 700;
    backdrop-filter: blur(6px);
    cursor: move;
    user-select: none;
  }
</style>
```

- [ ] **Step 4: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 5: 提交**

```bash
git add "src/routes/(widget)"
git commit -m "feat(widgets): add transparent todo and coins widget routes"
```

---

## Task 8: 主窗口 widget 显隐开关

**Files:** Modify `src/routes/(app)/+page.svelte`

- [ ] **Step 1: 在待办页加 widget 开关区**

在 `src/routes/(app)/+page.svelte` 的 `<script>` 顶部 import 区追加：

```ts
  import { widgetSetVisible, widgetGetVisibility } from "$lib/api";
```

在 `<script>` 的状态声明区（`let reward = $state(0);` 之后）追加：

```ts
  let widgetTodo = $state(false);
  let widgetCoins = $state(false);
```

把 `onMount(() => { void loadTodos(); });` 替换为：

```ts
  onMount(async () => {
    void loadTodos();
    const v = await widgetGetVisibility();
    widgetTodo = v.todo;
    widgetCoins = v.coins;
  });

  async function toggleWidget(kind: "todo" | "coins", on: boolean) {
    await widgetSetVisible(kind, on);
    if (kind === "todo") widgetTodo = on;
    else widgetCoins = on;
  }
```

在模板里 `<h1>DeskHub</h1>` 之后插入：

```svelte
  <section class="widgets">
    <label>
      <input
        type="checkbox"
        checked={widgetTodo}
        onchange={(e) => toggleWidget("todo", e.currentTarget.checked)}
      />
      桌面 Todo 组件 / Todo widget
    </label>
    <label>
      <input
        type="checkbox"
        checked={widgetCoins}
        onchange={(e) => toggleWidget("coins", e.currentTarget.checked)}
      />
      桌面金币组件 / Coins widget
    </label>
  </section>
```

在 `<style>` 内追加：

```css
  .widgets {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin: 0.5rem 0 1rem;
    font-size: 0.9rem;
    opacity: 0.9;
  }

  .widgets label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
```

- [ ] **Step 2: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 3: 提交**

```bash
git add "src/routes/(app)/+page.svelte"
git commit -m "feat(widgets): add desktop widget toggles in main window"
```

---

## Task 9: 端到端验收

**Files:** 无（验证 + 文档）

- [ ] **Step 1: 启动**

Run: `npm run tauri dev`
Expected: 主窗口正常（导航/背景/金币/主题），待办页有两个「桌面组件」开关。

- [ ] **Step 2: 手动验收**

1. 勾「桌面 Todo 组件」→ 出现一个透明 Todo 小窗，显示今日任务。
2. 该小窗**坐在普通窗口之下**（点其它应用会盖住它）、显示桌面时可见、可**勾选完成**任务。
3. 拖动小窗标题区可移动。
4. 勾「桌面金币组件」→ 出现透明金币小窗。
5. 关闭应用 → 重新 `npm run tauri dev` → 开着的 widget 自动恢复、位置保持。
6. 取消勾选 → 对应 widget 关闭。

> 若 bottommost 表现异常（频繁被顶起/点击失灵），记录现象，按 spec 第八节回退方案处理并在成果物说明。

- [ ] **Step 3: 标记进度文档**

在 `开发计划.md` 顶部「总体路线图」表后，新增一行说明（或在合适位置）记录本里程碑：「桌面透明 Widget（Todo/金币）已完成，托盘控制留 M3」。

- [ ] **Step 4: 提交**

```bash
git add 开发计划.md
git commit -m "docs(widgets): record desktop widgets milestone"
```

---

## 自检 / Self-Review

- **Spec 覆盖：** 依赖/插件(Task1) / 模型+可见性逻辑(Task2) / 钉层+开关窗口 spike(Task3) / 命令+注册+启动恢复(Task4) / 前端 api(Task5) / 透明基底+(app)分组(Task6) / widget 路由(Task7) / 主窗口开关(Task8) / 验收(Task9) —— 均有任务。
- **无占位符：** 步骤含完整代码与命令；Win32 版本偏差在 Task3 给出明确微调指引（spike 性质）。
- **类型一致：** `WidgetVisibility{todo,coins}` Rust(Task2)/TS(Task5) 一致；命令 `widget_set_visible`/`widget_get_visibility` Task4 注册、Task5 调用、Task8 使用一致；`widget_config`/`read_visibility`/`open_widget`/`close_widget`/`pin_to_desktop` 跨 Task2/3/4 一致；路由 `/widgets/todo`、`/widgets/coins` 与 `widget_config` 一致。

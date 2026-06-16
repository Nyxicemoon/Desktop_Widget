# DeskHub M3（本轮）— 系统托盘 + 关闭到托盘 + 测试通知 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 关闭主窗口隐藏到系统托盘，托盘可显示主窗口/开关 widget/退出，并提供一个发送系统通知的测试按钮；同时修复 widget 窗口的命令授权。

**Architecture:** Tauri v2 托盘（`tray-icon` feature）+ `on_window_event` 关闭拦截 + `tauri-plugin-notification`。托盘开 widget 用 `async_runtime::spawn` 规避主线程 `build()` 死锁；widget 关闭改为隐藏以便无重建复用。

**Tech Stack:** Rust + Tauri v2 (tray-icon) + tauri-plugin-notification + rusqlite；前端 SvelteKit + @tauri-apps/plugin-notification。

> **编译前置：** `cargo` 前确保 `build/` 存在。cargo 用 `"$USERPROFILE/.cargo/bin/cargo.exe" --manifest-path src-tauri/Cargo.toml`。
> **系统/GUI 性质：** 托盘/通知/窗口事件以**手动验收**为主；本轮无新增纯逻辑单测，保持现有 23 个不回归。
> **Tauri 托盘 API 版本差异：** 若 `TrayIconBuilder`/`MenuItem`/`TrayIconEvent` 签名与下方略有出入，按编译器提示微调，保持意图不变。

---

## 文件结构

- `src-tauri/Cargo.toml`（改）— tauri `tray-icon` feature、`tauri-plugin-notification`。
- `src-tauri/capabilities/default.json`（改）— `windows` 通配 widget、加 `notification:default`。
- `src-tauri/src/window/mod.rs`（改）— `set_widget_visible` 辅助；`close_widget` 改隐藏。
- `src-tauri/src/commands/widget.rs`（改）— 命令瘦身调用辅助。
- `src-tauri/src/tray.rs`（新）— 托盘构建与事件。
- `src-tauri/src/lib.rs`（改）— `mod tray`、setup 建托盘、关闭到托盘、注册通知插件。
- `src/lib/api/index.ts`（改）、`src/routes/(app)/+page.svelte`（改）、`package.json`（改）。

---

## Task 1: 依赖 + capabilities

**Files:** Modify `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`

- [ ] **Step 1: tauri 加 tray-icon feature + 通知插件**

把 `src-tauri/Cargo.toml` 中 `tauri = { version = "2", features = [] }` 改为：

```toml
tauri = { version = "2", features = ["tray-icon"] }
```

并在 `[dependencies]` 末尾追加：

```toml
tauri-plugin-notification = "2"
```

- [ ] **Step 2: capabilities 覆盖 widget 窗口 + 通知权限**

把 `src-tauri/capabilities/default.json` 整个替换为：

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window and widget windows",
  "windows": ["main", "widget-*"],
  "permissions": [
    "core:default",
    "opener:default",
    "notification:default"
  ]
}
```

- [ ] **Step 3: 编译检查（拉取通知插件）**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json
git commit -m "feat(m3): enable tray-icon feature, notification plugin, widget capabilities"
```

---

## Task 2: widget 显隐辅助 + 关闭改隐藏 + 命令瘦身

**Files:** Modify `src-tauri/src/window/mod.rs`, `src-tauri/src/commands/widget.rs`

- [ ] **Step 1: window 模块加 Db 引用**

把 `src-tauri/src/window/mod.rs` 顶部的 `use crate::db::kv;` 改为：

```rust
use crate::db::{kv, Db};
```

- [ ] **Step 2: close_widget 改为隐藏，并新增 set_widget_visible**

把 `src-tauri/src/window/mod.rs` 中的 `close_widget` 函数整体替换为下面两个函数：

```rust
pub fn close_widget(app: &AppHandle, kind: &str) -> AppResult<()> {
    let (label, ..) = widget_config(kind)?;
    if let Some(win) = app.get_webview_window(label) {
        win.hide().map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

/// Open or hide a widget and persist its visibility to kv. Shared by command + tray.
pub fn set_widget_visible(app: &AppHandle, kind: &str, visible: bool) -> AppResult<()> {
    if visible {
        open_widget(app, kind)?;
    } else {
        close_widget(app, kind)?;
    }
    let state = app.state::<Db>();
    let conn = state.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    kv::set(
        &conn,
        &format!("widget.{kind}.visible"),
        if visible { "1" } else { "0" },
    )
}
```

- [ ] **Step 3: 命令瘦身**

把 `src-tauri/src/commands/widget.rs` 整个替换为：

```rust
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use crate::window;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn widget_set_visible(app: AppHandle, kind: String, visible: bool) -> AppResult<()> {
    window::set_widget_visible(&app, &kind, visible)
}

#[tauri::command]
pub fn widget_get_visibility(db: State<Db>) -> AppResult<WidgetVisibility> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    window::read_visibility(&conn)
}
```

- [ ] **Step 4: 测试 + 编译**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
```
Expected: 23 passed（不回归）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/window/mod.rs src-tauri/src/commands/widget.rs
git commit -m "feat(m3): hide widgets on close; shared set_widget_visible helper"
```

---

## Task 3: 系统托盘 + 关闭到托盘 + 注册通知插件

**Files:** Create `src-tauri/src/tray.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: tray 模块**

创建 `src-tauri/src/tray.rs`：

```rust
use crate::db::Db;
use crate::window;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn spawn_toggle(app: &AppHandle, kind: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let current = {
            let state = app.state::<Db>();
            let guard = match state.0.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            window::read_visibility(&guard)
                .map(|v| if kind == "todo" { v.todo } else { v.coins })
                .unwrap_or(false)
        };
        let _ = window::set_widget_visible(&app, kind, !current);
    });
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show_main", "显示主窗口 / Show", true, None::<&str>)?;
    let todo = MenuItem::with_id(app, "toggle_todo", "Todo 组件", true, None::<&str>)?;
    let coins = MenuItem::with_id(app, "toggle_coins", "金币组件", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 / Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &todo, &coins, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("DeskHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_main" => show_main(app),
            "toggle_todo" => spawn_toggle(app, "todo"),
            "toggle_coins" => spawn_toggle(app, "coins"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
```

> 若编译报 `Image::from_bytes` 或 `TrayIconBuilder` 方法名/参数不符，按提示微调（意图：用 128x128.png 作托盘图标 + 四项菜单 + 左键显示主窗口）。

- [ ] **Step 2: lib.rs 接线**

在 `src-tauri/src/lib.rs` 顶部模块声明区加入：

```rust
mod tray;
```

把 `.plugin(tauri_plugin_window_state::Builder::default().build())` 之后追加一行注册通知插件：

```rust
        .plugin(tauri_plugin_notification::init())
```

在 builder 链中（`.invoke_handler(...)` 之前）加入关闭到托盘：

```rust
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
```

在 `.setup(...)` 闭包内，恢复 widget 的两段 `if vis... open_widget` 之后、`Ok(())` 之前，加入建托盘：

```rust
            tray::create(app.handle())?;
```

- [ ] **Step 3: 测试 + lint**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
"$USERPROFILE/.cargo/bin/cargo.exe" clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```
Expected: 23 passed；clippy 无警告。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/tray.rs src-tauri/src/lib.rs
git commit -m "feat(m3): add system tray, close-to-tray, register notification plugin"
```

---

## Task 4: 前端测试通知按钮

**Files:** Modify `package.json`, `src/lib/api/index.ts`, `src/routes/(app)/+page.svelte`

- [ ] **Step 1: 安装通知插件 JS 包**

Run: `npm install @tauri-apps/plugin-notification`
Expected: 安装成功，`package.json` dependencies 出现该包。

- [ ] **Step 2: api 封装**

在 `src/lib/api/index.ts` 末尾追加：

```ts
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

export async function sendTestNotification(): Promise<void> {
  let granted = await isPermissionGranted();
  if (!granted) {
    granted = (await requestPermission()) === "granted";
  }
  if (granted) {
    sendNotification({ title: "DeskHub", body: "测试通知 / Test notification" });
  }
}
```

- [ ] **Step 3: 主窗口加按钮**

在 `src/routes/(app)/+page.svelte` 的 import 区追加：

```ts
  import { sendTestNotification } from "$lib/api";
```

在模板的 `<section class="widgets">...</section>` 块**之后**插入：

```svelte
  <section class="widgets">
    <button class="ghost" onclick={() => sendTestNotification()}>
      发送测试通知 / Send test notification
    </button>
  </section>
```

- [ ] **Step 4: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 5: 提交**

```bash
git add package.json package-lock.json src/lib/api/index.ts "src/routes/(app)/+page.svelte"
git commit -m "feat(m3): add send-test-notification button"
```

---

## Task 5: 端到端验收

**Files:** Modify `开发计划.md`

- [ ] **Step 1: 启动**

Run: `npm run tauri dev`
Expected: 主窗口出现，系统托盘出现 DeskHub 图标。

- [ ] **Step 2: 手动验收**

1. 点主窗口 **X** → 主窗口隐藏，app 仍在（托盘图标在）。
2. **左键单击托盘** → 主窗口重新出现。
3. 托盘**右键** → 菜单：显示主窗口 / Todo 组件 / 金币组件 / 退出。
4. 菜单点「Todo 组件」「金币组件」→ 对应 widget 开/关，**不卡死**。
5. 主窗口里勾选 widget、再从托盘关，行为一致。
6. widget 内能看到今日任务（新建一条试）、金币数。
7. 点「发送测试通知」→ 弹出系统通知。
8. 托盘「退出」→ app 真正结束（托盘图标消失）。

- [ ] **Step 3: 标记进度文档**

在 `开发计划.md` 的 M3 小节，把已完成项 `- [ ]` 改为 `- [x]`：系统托盘（含关闭最小化到托盘）、本地通知（测试按钮）。其余（开机自启、备份、打包）保留未勾，并在 M3 小节补注一行说明本轮范围与 widget capabilities 修复。

- [ ] **Step 4: 提交**

```bash
git add 开发计划.md
git commit -m "docs(m3): mark tray + test-notification done (this round)"
```

---

## 自检 / Self-Review

- **Spec 覆盖：** 依赖+capabilities(Task1) / widget 隐藏+共享辅助(Task2) / 托盘+关闭到托盘+通知插件(Task3) / 测试通知按钮(Task4) / 验收+文档(Task5)；widget capabilities 修复在 Task1。均有任务。
- **无占位符：** 步骤含完整代码与命令；Tauri 托盘/Win32 版本差异给出微调指引。
- **类型一致：** `set_widget_visible(app,kind,visible)` 定义于 window(Task2)、被 command(Task2) 与 tray(Task3) 调用；`read_visibility`、`open_widget`、`close_widget`、`Db` 跨文件一致；命令名 `widget_set_visible`/`widget_get_visibility` 未变（前端无需改）；`sendTestNotification` 定义(Task4 api)与使用(Task4 page)一致。

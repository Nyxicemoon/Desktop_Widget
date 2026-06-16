# DeskHub M3（本轮）— 系统托盘 + 关闭到托盘 + 测试通知 设计 (Spec)

> 日期：2026-06-16
> 范围：[开发计划.md](../../../开发计划.md) M3 的子集：**系统托盘 + 关闭到托盘 + 一个「发送测试通知」按钮**。
> 前置：M0–M2 + 桌面 Widget 已完成（主窗口 + 透明 widget 窗口、widget 显隐命令）。
> 顺带修复：widget 窗口的 capabilities 授权缺失。

---

## 一、目标与验收

**目标：** 关闭主窗口不退出而是进托盘；托盘可随时叫回主窗口、开关 widget、退出；提供一个测试系统通知的按钮。

**验收标准：**
1. 主窗口点 X → **隐藏到托盘**（不退出，app 继续运行）。
2. 托盘**左键单击 = 显示主窗口**；右键菜单：显示主窗口 / Todo 组件 / 金币组件 / 退出。
3. 托盘菜单开/关两个 widget 正常（**不卡死**）；「退出」真正结束 app。
4. 主窗口「发送测试通知」按钮 → 弹出系统通知（首次请求权限）。
5. **修复**：widget 窗口能正常调用命令（todo 列表/金币在 widget 内能加载）。
6. `cargo test` 23 不回归；`cargo clippy -D warnings` 干净；`npm run check` 0 errors。

**关键决策（已确认）：** 本轮只做托盘 + 关闭到托盘 + 测试通知按钮；开机自启/备份/打包/到期提醒留后续。

---

## 二、架构

### 系统托盘（`tray.rs` 新建）
- `tauri` 加 `tray-icon` feature；setup 里 `tray::create(app)`。
- 托盘图标用内嵌 PNG（`include_bytes!("../icons/128x128.png")` → `tauri::image::Image::from_bytes`），避免 dev 下 `default_window_icon()` 可能为 None。
- 菜单项（`MenuItem::with_id`）：`show_main` / `toggle_todo` / `toggle_coins` / `quit`。
- `on_menu_event`：
  - `show_main` → 显示并聚焦主窗口。
  - `toggle_todo` / `toggle_coins` → **`tauri::async_runtime::spawn`** 异步执行翻转（读 kv 当前值 → 取反 → `set_widget_visible`）。**必须 spawn**：菜单事件在主线程，直接 `build()` 会复刻上次的死锁。
  - `quit` → `app.exit(0)`。
- `on_tray_icon_event`：左键 Up → 显示主窗口。

### 关闭到托盘（`lib.rs`）
- `.on_window_event`：当 `window.label()=="main"` 且事件为 `CloseRequested` → `api.prevent_close()` + `window.hide()`。
- 只有托盘「退出」`app.exit(0)` 才真正退出。

### Widget 生命周期调整（`window/mod.rs`）
- `close_widget` 由**销毁**改为**隐藏**（`win.hide()`）；`open_widget` 已是「存在则 show，否则 build」。→ 再开是 show（无重建、无死锁）。
- 抽 `set_widget_visible(app, kind, visible)`：开/关 + 写 kv，供**命令**与**托盘**共用（DRY）。
- `commands::widget::widget_set_visible`（保持 **async**）瘦身为调用 `set_widget_visible`，不再直接收 `State<Db>`（helper 内部经 `app.state()` 取）。

### 通知（最小）
- `tauri-plugin-notification`（Rust `.plugin(init())` + npm `@tauri-apps/plugin-notification`）。
- `capabilities/default.json` 加 `notification:default`。
- 前端 `lib/api` 加 `sendTestNotification()`：无权限先 `requestPermission`，再 `sendNotification`。
- 主窗口设置区加「发送测试通知」按钮。

### Capabilities 修复
- `capabilities/default.json` 的 `windows` 由 `["main"]` 改为 `["main", "widget-*"]`，让 widget 窗口也能 invoke 命令（修复 widget 内数据加载）。加 `notification:default`。

---

## 三、模块 / Files

- 新建 `src-tauri/src/tray.rs`。
- 改 `src-tauri/src/window/mod.rs`（`set_widget_visible`、close 改 hide）。
- 改 `src-tauri/src/commands/widget.rs`（瘦身）。
- 改 `src-tauri/src/lib.rs`（mod tray、setup 建托盘、on_window_event 关闭到托盘、注册通知插件）。
- 改 `src-tauri/Cargo.toml`（tauri tray-icon feature、tauri-plugin-notification）。
- 改 `src-tauri/capabilities/default.json`（windows 通配 + notification 权限）。
- 改 `src/lib/api/index.ts`（通知封装）、`src/routes/(app)/+page.svelte`（测试按钮）、`package.json`（通知插件）。

---

## 四、测试与门禁

- 托盘 / 关闭到托盘 / 通知 / 窗口事件均为**系统 GUI 行为 → 手动验收**；本轮无新增可单测的纯逻辑（glue 需 AppHandle）。
- 保持现有 **23 个 Rust 单测**不回归。
- 门禁：`cargo test`、`cargo clippy -- -D warnings`、`npm run check`。
- **手动验收**：关主窗口→托盘常驻→托盘开主窗口/开关 widget/退出；点测试通知→系统通知弹出；widget 内 todo 列表/金币能加载。

---

## 五、不在本轮范围（YAGNI）

开机自启、数据备份（导出/导入）、打包安装包、任务到期定时提醒、widget 与主窗口的实时状态同步（托盘改了显隐后主窗口勾选框不实时刷新，下次打开主窗口时刷新即可）。

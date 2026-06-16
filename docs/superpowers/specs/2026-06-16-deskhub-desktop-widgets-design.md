# DeskHub 桌面透明 Widget 设计 (Spec)

> 日期：2026-06-16
> 范围：把 DeskHub 的功能以**透明桌面小组件**呈现（gadget 风格），同时保留普通主窗口。
> 前置：M0–M2 已完成（rusqlite、命令模式、Todo+金币、Pexels 背景、前端 api/stores/路由）。
> 关联里程碑：托盘控制 widget 显隐留到 M3。

---

## 一、目标与验收

**目标：** 用户可在 Windows 桌面上放置透明、可拖动、可交互的小组件（Todo、金币），坐在普通应用窗口之下；主窗口仍作完整管理与设置。

**验收标准：**
1. 主窗口正常（导航、M2 背景图、Todo/背景管理）。
2. 从主窗口开/关 **Todo widget** 与 **金币 widget**；它们是**透明无边框**小窗，**坐在普通窗口之下**仍可点击。
3. Todo widget 显示今日任务、可勾选完成（复用现有命令）；金币 widget 显示余额。
4. 拖动可移动；**重启后位置与显隐状态保持**。
5. `cargo test` 全过；`cargo clippy -D warnings` 干净；`npm run check` 0 errors。

> **验证性质：** 透明度 / z-order / 拖动 / 桌面可点击属 GUI 行为，**以手动验收为主**；可单测的只有「显隐状态在 kv 的存取」纯逻辑。

**关键决策（已确认）：**
- 多个独立透明 widget 窗口；主窗口 + widget 并存。
- 「桌面钉层」= **bottommost + `WS_EX_NOACTIVATE`**（仍可交互），**不**用 WorkerW/SetParent 壁纸嵌入（那会失去交互）。
- 位置记忆用 `tauri-plugin-window-state`；显隐状态存 `kv`。

---

## 二、风险与 spike

桌面钉层用 Win32（`windows` crate），且 `WebviewWindow::hwnd()` 与 `windows` crate 版本可能有类型偏差 → **先做 spike**：一个透明 + bottommost + 可点击的占位 widget 跑通（能编译、窗口出现、置底无报错），再上真内容。若 bottommost 不稳定，退化为「普通层 + 不抢焦点」并在成果物里说明。

---

## 三、架构

### 窗口
- **主窗口**（`tauri.conf.json` 定义，label `main`，不透明）。
- **Widget 窗口**（运行时按需创建）：label `widget-todo` / `widget-coins`，指向路由 `/widgets/todo` / `/widgets/coins`，属性 `transparent:true, decorations:false, skipTaskbar:true, shadow:false, alwaysOnTop:false, resizable:false`，初始小尺寸。

### 后端模块
- `window/mod.rs`（新建）：`pin_to_desktop(&WebviewWindow)`（Win32：加 `WS_EX_NOACTIVATE`、`SetWindowPos(HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE)`）；`open_widget(app, kind)`（创建/显示窗口并 pin）、`close_widget(app, kind)`。
- `db/kv` 复用：widget 显隐存 `widget.todo.visible` / `widget.coins.visible`（"1"/"0"）。
- `commands/widget.rs`（新建）：`widget_set_visible(app, kind, visible)`、`widget_get_visibility(db) -> WidgetVisibility`。
- `lib.rs` setup：读 kv，恢复应显示的 widget（创建窗口 + pin）。

### 前端（SvelteKit 路由分组）
- `src/routes/(app)/`：主窗口 UI。`(app)/+layout.svelte` = 导航 + M2 背景层 + 不透明外壳（`var(--bg)`）。页面：`(app)/+page.svelte`（Todo 管理，含 widget 显隐开关）、`(app)/backgrounds/+page.svelte`。
- `src/routes/(widget)/+layout.svelte`：透明布局（无导航、无背景、含拖动区）。
- `src/routes/(widget)/widgets/todo/+page.svelte`、`.../widgets/coins/+page.svelte`：透明小卡片，复用 `stores/todos`、`stores/game`。
- `src/app.css`：`html,body` 改为**透明**；不透明背景改由 `(app)` 外壳承担。

### 依赖
`tauri-plugin-window-state`（位置/尺寸持久化）、`windows`（Win32_Foundation、Win32_UI_WindowsAndMessaging）。

---

## 四、数据流

```
启动: setup 读 kv 显隐 → 对应 widget 窗口创建 + pin_to_desktop；window-state 插件恢复位置
开关: 主窗口点开关 → widget_set_visible(kind, on/off) → 创建并 pin / 关闭窗口 → 写 kv
交互: widget 内勾选 todo → 复用 todo_toggle_done；金币 widget 读 game_get_profile
拖动: 拖动区(data-tauri-drag-region) 移动窗口 → 关闭时 window-state 自动存位置
```

---

## 五、命令

- `widget_set_visible(app, kind: String, visible: bool) -> ()`（kind ∈ "todo" | "coins"）
- `widget_get_visibility(db) -> WidgetVisibility { todo: bool, coins: bool }`

模型 `WidgetVisibility { todo: bool, coins: bool }`（Serialize）。

---

## 六、测试与门禁

- **Rust 单测：** widget 显隐在 kv 的读/写默认值（默认未显示 → false；set 后 get 反映）。把「读 kv → 解析为 WidgetVisibility」做成接受 `&Connection` 的纯函数以便测。
- **不单测：** Win32 钉层、窗口创建、透明（GUI，手动验收）。
- **门禁：** `cargo test`、`cargo clippy -- -D warnings`、`npm run check`。
- **手动验收：** 开/关两个 widget、透明、置于应用之下仍可点、勾选 todo、拖动、重启位置与显隐保持。

---

## 七、不在本里程碑范围（YAGNI）

托盘控制 widget（→ M3）；拖拽吸附/对齐；多显示器特殊处理；widget 独立主题；游戏/宠物 widget；widget 尺寸自适应内容。

---

## 八、回退方案

若 bottommost 在实测中不稳定（被频繁顶起或点击异常），退化为 `alwaysOnTop:false` 的普通层 + `WS_EX_NOACTIVATE`（不抢焦点），并在成果物中说明取舍。

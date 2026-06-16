# DeskHub

> A Windows desktop efficiency tool — transparent desktop widgets, todo + coins, real wallpaper management, and tray integration.
> Windows 桌面效率工具 —— 透明桌面小组件、待办 + 金币、真桌面壁纸管理、系统托盘常驻。

Built with **Tauri v2 + SvelteKit + TypeScript + Rust + SQLite**. Local-first, low-memory, designed to run quietly in the background.

基于 **Tauri v2 + SvelteKit + TypeScript + Rust + SQLite**,本地优先、低内存,适合后台长期运行。

---

## ✨ Features / 功能

- **📝 Todo + 金币 / Todo & coins** — 今日任务增删改、完成任务获得金币奖励(同事务防重复发奖)。
  Today's tasks with create/edit/delete; completing a task awards coins (de-duplicated within a transaction).
- **🪟 透明桌面小组件 / Transparent desktop widgets** — Todo 与金币独立透明窗口,贴在桌面层(可交互、可拖动、记忆位置)。
  Independent transparent windows pinned to the desktop layer (interactive, draggable, position remembered).
- **🖼️ 桌面壁纸管理 / Wallpaper management** — 通过 Pexels 搜图,按显示器分辨率裁剪,设为**真 Windows 桌面壁纸**(Fit 保持比例),可一键恢复原壁纸。
  Search via Pexels, crop to monitor resolution, set as the **real Windows wallpaper** (Fit, aspect-preserved), restore original anytime.
- **🔔 通知与到期提醒 / Notifications & due reminders** — 本地通知;任务到期当天自动提醒(后台轮询 + 去重)。
  Local notifications; due tasks are reminded automatically (background polling with de-dup).
- **🗂️ 系统托盘 / System tray** — 托盘菜单控制主窗口与小组件显隐;关闭主窗口最小化到托盘而非退出。
  Tray menu toggles the main window and widgets; closing the main window hides to tray instead of quitting.
- **🚀 开机自启 / Autostart** — 随 Windows 启动并隐藏到托盘,设置页可开关。
  Starts with Windows hidden to tray; toggleable in Settings.
- **💾 数据备份 / Data backup** — 导出/导入本地 SQLite 数据库(导出用 `VACUUM INTO`,导入校验后于重启时应用)。
  Export/import the local SQLite database (export via `VACUUM INTO`, import validated and applied on restart).

---

## 🧱 Tech Stack / 技术栈

| Layer | Choice |
| -- | -- |
| Desktop shell | Tauri v2 (multi-window, transparent/frameless, tray) |
| Frontend | SvelteKit (adapter-static SPA) + TypeScript |
| Backend | Rust |
| Storage | SQLite via [`rusqlite`](https://docs.rs/rusqlite) (bundled), migrations via `PRAGMA user_version` |
| System integration | [`windows-rs`](https://github.com/microsoft/windows-rs) (wallpaper, desktop-pin) |
| Image source | [Pexels API](https://www.pexels.com/api/) via `reqwest` |

Chosen over Electron for low memory and good background-run performance.

---

## 🚀 Getting Started / 快速开始

### Prerequisites / 前置

- [Node.js](https://nodejs.org/) (LTS) + npm
- [Rust](https://rustup.rs/) toolchain
- Windows + [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++ workload)

### Develop / 开发

```bash
npm install
npm run tauri dev
```

### Build installer / 打包安装器

```bash
npm run tauri build
# → src-tauri/target/release/bundle/nsis/deskhub_<version>_x64-setup.exe
```

### Quality gates / 质量门禁

```bash
npm run check        # Svelte/TS type-check
cargo test           # run inside src-tauri/
cargo clippy         # run inside src-tauri/
```

### Pexels API key

壁纸搜索需要你自己的 Pexels API Key(免费申请),在应用「背景 / Backgrounds」页填入。Key 仅保存在本地 `app_data_dir/config.json`,**不入库、不进 git**。

The wallpaper feature needs your own free Pexels API key, entered in the in-app **Backgrounds** page. It is stored only in the local `app_data_dir/config.json` — never committed and never in the database.

---

## 📂 Project Structure / 目录结构

```
src/                     # Frontend (SvelteKit)
  lib/api/               # Typed Tauri command wrappers
  lib/stores/            # Svelte stores
  routes/(app)/          # Main window UI
  routes/(widget)/       # Transparent widget windows
src-tauri/src/           # Backend (Rust)
  commands/              # Tauri command handlers
  db/                    # SQLite access (all SQL isolated here)
  system/                # Win32 integration (wallpaper)
  window/                # Widget window management + desktop-pin
docs/                    # Spec & plan documents (Chinese)
```

---

## 🗺️ Roadmap

- [x] M0 — Scaffolding (SQLite migrations, command conventions)
- [x] M1 — Todo + coins
- [x] M2 — Background / wallpaper management
- [x] Desktop transparent widgets
- [x] M3 — System integration & packaging (tray, autostart, notifications, backup, NSIS)
- [ ] M4 — Desktop icon management
- [ ] M5 — Email (Gmail OAuth)
- [ ] M6 — Idle-game depth (exp/level, auto-production, offline earnings)

✅ **MVP (M0–M3) complete.**

---

## 📄 License

[MIT](LICENSE)

# DeskHub 背景图 → 真·Windows 桌面壁纸 设计 (Spec)

> 日期：2026-06-16
> 背景：M2 把 Pexels 图设为**应用窗口背景**；但随桌面 widget 转向，用户期望换**真正的 Windows 桌面壁纸**（图作为透明 widget 背后的真实桌面）。已确认改为设置系统壁纸。
> 前置：M2 背景管理（搜索/下载/backgrounds 表）已完成且后端经实测正常（下载+写库+set_current 成功）。

---

## 一、目标与验收

**目标：** 在背景页选图后，下载的图被设为 **Windows 桌面壁纸**；「恢复默认」还原用户原先的壁纸；主窗口不再显示应用内背景图。

**验收标准：**
1. 背景页搜图 → 点一张 → **Windows 桌面壁纸变成该图**（透明 widget 背后即为它）。
2. 「恢复默认」→ 壁纸还原为**首次更换前**的原壁纸。
3. `backgrounds.source_url` 仍完整记录来源（licensing 不变）。
4. 主窗口不再渲染应用内背景层。
5. `cargo test` 不回归；`cargo clippy -D warnings` 干净；`npm run check` 0 errors。

**关键决策（已确认）：** 设真·Windows 壁纸（非应用内背景）；「恢复默认」还原原壁纸。

---

## 二、架构

### 系统壁纸（`system/mod.rs`，启用占位模块）
- `set_wallpaper(path: &Path) -> AppResult<()>`：Win32 `SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, path_wide_ptr, SPIF_UPDATEINIFILE|SPIF_SENDCHANGE)`。
- `get_wallpaper() -> AppResult<String>`：`SystemParametersInfoW(SPI_GETDESKWALLPAPER, MAX_PATH, buf, ..)` 读当前壁纸路径。
- 非 Windows：`set_wallpaper` 空实现、`get_wallpaper` 返回空串。
- 依赖：`windows` crate 已有 `Win32_UI_WindowsAndMessaging`（含 SystemParametersInfoW）。

### 命令改动（`commands/backgrounds.rs`）
- `bg_download_and_set`：下载 + `insert` + `set_current` 后——
  - **首次保存原壁纸**：若 kv 无 `wallpaper.original`，`get_wallpaper()` 存入 kv。
  - `system::set_wallpaper(&dest)` 设为新壁纸。
- `bg_restore_default`：清 `is_current` 后，若 kv 有 `wallpaper.original` 则 `set_wallpaper(原路径)` 还原。

### 前端
- `(app)/+layout.svelte`：移除 `.bg-layer` 渲染与 `currentBg`/`loadBackground` 使用（壁纸已是真实背景）。
- `lib/stores/background.ts`：`clearBackground` 简化为只调 `bgRestoreDefault`；移除不再使用的 `currentBg`/`loadBackground`。
- 背景页 `pick` 后不再调用 `loadBackground`。

---

## 三、模块 / Files
- 改 `src-tauri/src/system/mod.rs`（壁纸 get/set）。
- 改 `src-tauri/src/commands/backgrounds.rs`（设/还原壁纸 + 存原壁纸）。
- 改 `src/routes/(app)/+layout.svelte`、`src/lib/stores/background.ts`、`src/routes/(app)/backgrounds/+page.svelte`。

---

## 四、测试与门禁
- Win32 壁纸 get/set 为系统行为 → **手动验收**（壁纸是否真的变/还原）。
- 现有 23 个 Rust 单测不回归。
- 门禁：`cargo test`、`cargo clippy -- -D warnings`、`npm run check`。
- 手动验收：选图→桌面壁纸变；恢复默认→壁纸还原。

---

## 五、不在范围（YAGNI）
随机/每日自动换壁纸、多显示器分别设壁纸、壁纸适应模式（填充/拉伸）配置、删除本地图。

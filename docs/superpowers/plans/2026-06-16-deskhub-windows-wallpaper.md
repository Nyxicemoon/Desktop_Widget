# DeskHub 背景图 → Windows 桌面壁纸 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 选背景图后设为真·Windows 桌面壁纸，「恢复默认」还原原壁纸；移除主窗口应用内背景层。

**Architecture:** `system/mod.rs` 用 Win32 `SystemParametersInfoW` set/get 壁纸；`bg_download_and_set` 设壁纸并首存原壁纸到 kv；`bg_restore_default` 还原；前端移除 in-app 背景层。

**Tech Stack:** Rust + windows-rs (Win32_UI_WindowsAndMessaging)；前端 SvelteKit。

> cargo 用 `"$USERPROFILE/.cargo/bin/cargo.exe" --manifest-path src-tauri/Cargo.toml`；GUI/系统行为以手动验收为主。

---

## Task 1: system 模块壁纸 get/set

**Files:** Modify `src-tauri/src/system/mod.rs`

- [ ] **Step 1: 实现**

把 `src-tauri/src/system/mod.rs` 整个替换为：

```rust
//! System integration. Wallpaper get/set (tray lives in `tray.rs`).

use crate::error::AppResult;
use std::path::Path;

#[cfg(target_os = "windows")]
pub fn set_wallpaper(path: &Path) -> AppResult<()> {
    use crate::error::AppError;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_ptr() as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn get_wallpaper() -> AppResult<String> {
    use crate::error::AppError;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut buf = [0u16; 260];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(String::from_utf16_lossy(&buf)
        .trim_end_matches('\0')
        .to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn set_wallpaper(_path: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn get_wallpaper() -> AppResult<String> {
    Ok(String::new())
}
```

- [ ] **Step 2: 编译检查**

Run: `"$USERPROFILE/.cargo/bin/cargo.exe" build --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过（函数暂未被调用，dead_code 警告正常）。若 Win32 签名不符按提示微调（意图：SPI_SETDESKWALLPAPER 写路径、SPI_GETDESKWALLPAPER 读路径）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/system/mod.rs
git commit -m "feat(wallpaper): add Win32 get/set desktop wallpaper in system module"
```

---

## Task 2: 命令设/还原壁纸

**Files:** Modify `src-tauri/src/commands/backgrounds.rs`

- [ ] **Step 1: 引入 kv**

把 `src-tauri/src/commands/backgrounds.rs` 顶部 `use crate::db::{backgrounds, Db};` 改为：

```rust
use crate::db::{backgrounds, kv, Db};
```

- [ ] **Step 2: bg_download_and_set 设壁纸 + 首存原壁纸**

把 `bg_download_and_set` 结尾的：

```rust
    let mut conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let id = backgrounds::insert(
        &conn,
        &dest.to_string_lossy(),
        &photo.source_url,
        Some(&photo.author),
        Some("Pexels License"),
        Some(&keyword),
    )?;
    backgrounds::set_current(&mut conn, id)
}
```

替换为：

```rust
    let mut conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let id = backgrounds::insert(
        &conn,
        &dest.to_string_lossy(),
        &photo.source_url,
        Some(&photo.author),
        Some("Pexels License"),
        Some(&keyword),
    )?;
    backgrounds::set_current(&mut conn, id)?;

    // Save the user's original wallpaper once, so "restore" can revert later.
    if kv::get(&conn, "wallpaper.original")?.is_none() {
        if let Ok(orig) = crate::system::get_wallpaper() {
            let _ = kv::set(&conn, "wallpaper.original", &orig);
        }
    }
    drop(conn);
    crate::system::set_wallpaper(&dest)
}
```

- [ ] **Step 3: bg_restore_default 还原壁纸**

把 `bg_restore_default` 整个替换为：

```rust
#[tauri::command]
pub fn bg_restore_default(db: State<Db>) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    backgrounds::restore_default(&conn)?;
    let original = kv::get(&conn, "wallpaper.original")?;
    drop(conn);
    if let Some(path) = original {
        let _ = crate::system::set_wallpaper(std::path::Path::new(&path));
    }
    Ok(())
}
```

- [ ] **Step 4: 测试 + lint**

Run:
```
"$USERPROFILE/.cargo/bin/cargo.exe" test --manifest-path src-tauri/Cargo.toml
"$USERPROFILE/.cargo/bin/cargo.exe" clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```
Expected: 23 passed；clippy 无警告。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/commands/backgrounds.rs
git commit -m "feat(wallpaper): set Windows wallpaper on pick; restore original on default"
```

---

## Task 3: 前端移除应用内背景层

**Files:** Modify `src/routes/(app)/+layout.svelte`, `src/lib/stores/background.ts`, `src/routes/(app)/backgrounds/+page.svelte`

- [ ] **Step 1: store 简化**

把 `src/lib/stores/background.ts` 整个替换为：

```ts
import { bgRestoreDefault } from "$lib/api";

export async function clearBackground(): Promise<void> {
  await bgRestoreDefault();
}
```

- [ ] **Step 2: 布局移除背景层**

把 `src/routes/(app)/+layout.svelte` 整个替换为：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { theme, initTheme, toggleTheme } from "$lib/stores/theme";
  import { coins, refreshCoins } from "$lib/stores/game";

  let { children } = $props();

  onMount(() => {
    void initTheme();
    void refreshCoins();
  });
</script>

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

- [ ] **Step 3: 背景页去掉 loadBackground 调用**

在 `src/routes/(app)/backgrounds/+page.svelte` 中，把 import：

```ts
  import { loadBackground, clearBackground } from "$lib/stores/background";
```

改为：

```ts
  import { clearBackground } from "$lib/stores/background";
```

并把 `pick` 函数里的这两行：

```ts
      await bgDownloadAndSet(photo, keyword);
      await loadBackground();
      message = "已设为背景 / Set as background";
```

改为：

```ts
      await bgDownloadAndSet(photo, keyword);
      message = "已设为壁纸 / Set as wallpaper";
```

- [ ] **Step 4: 类型检查**

Run: `npm run check`
Expected: 0 errors。

- [ ] **Step 5: 提交**

```bash
git add "src/routes/(app)/+layout.svelte" src/lib/stores/background.ts "src/routes/(app)/backgrounds/+page.svelte"
git commit -m "feat(wallpaper): drop in-app background layer; wallpaper is the real background"
```

---

## Task 4: 验收

**Files:** 无（验证）

- [ ] **Step 1: 启动**

Run: `npm run tauri dev`
Expected: 主窗口正常（无应用内背景图层）。

- [ ] **Step 2: 手动验收**

1. 背景页填 key（若未填）→ 搜「雪山」→ 点一张 → **Windows 桌面壁纸变成该图**。
2. 「恢复默认」→ 桌面壁纸**还原**为更换前的原壁纸。
3. 透明 widget 此时叠在新壁纸上，观感正确。

- [ ] **Step 3: 清理调试遗留（可选）**

之前调试在 backgrounds 表插了测试行（id=1）。如需干净，可在背景页重新选一张正式图覆盖，或忽略（不影响功能）。

---

## 自检 / Self-Review
- Spec 覆盖：壁纸 get/set(Task1) / 设+存原+还原(Task2) / 移除 in-app 背景层(Task3) / 验收(Task4)。
- 无占位符：完整代码 + 命令。
- 一致性：`system::set_wallpaper/get_wallpaper`、kv 键 `wallpaper.original`、`clearBackground` 跨文件一致；`bg_restore_default` 仍为命令名（前端 `bgRestoreDefault` 不变）。

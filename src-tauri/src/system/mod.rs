//! System integration. Wallpaper get/set (tray lives in `tray.rs`).

use crate::error::AppResult;
use std::path::Path;

/// Set the desktop wallpaper "fit" style to Fit (whole image, aspect preserved).
/// WallpaperStyle: Fit=6, Fill=10, Stretch=2, Center=0, Span=22; TileWallpaper=0.
#[cfg(target_os = "windows")]
fn set_fit_style() -> AppResult<()> {
    use crate::error::AppError;
    use windows::core::w;
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

    fn reg_sz(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let style = reg_sz("6");
    let tile = reg_sz("0");
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            w!("Control Panel\\Desktop"),
            w!("WallpaperStyle"),
            REG_SZ.0,
            Some(style.as_ptr() as *const core::ffi::c_void),
            (style.len() * 2) as u32,
        )
        .ok()
        .map_err(|e| AppError::Other(e.to_string()))?;
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            w!("Control Panel\\Desktop"),
            w!("TileWallpaper"),
            REG_SZ.0,
            Some(tile.as_ptr() as *const core::ffi::c_void),
            (tile.len() * 2) as u32,
        )
        .ok()
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_wallpaper(path: &Path) -> AppResult<()> {
    use crate::error::AppError;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    };
    set_fit_style()?;
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

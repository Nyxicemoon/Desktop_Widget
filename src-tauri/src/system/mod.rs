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

//! Windows desktop shortcut scanning, icon extraction, and launching.
//!
//! All Win32/COM/GDI work is isolated here. Icon extraction is best-effort:
//! any failure yields `Ok(None)` so the rest of the app degrades gracefully.

use crate::error::AppResult;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ShortcutRaw {
    pub name: String,
    pub lnk_path: String,
    pub target: String,
    pub args: Option<String>,
}

#[cfg(target_os = "windows")]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Resolve a `.lnk` to (target, args). Returns ("", None) on failure.
#[cfg(target_os = "windows")]
fn parse_lnk(lnk_path: &str) -> AppResult<(String, Option<String>)> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| crate::error::AppError::Other(e.to_string()))?;
        let persist: IPersistFile = link.cast().map_err(|e| crate::error::AppError::Other(e.to_string()))?;
        let wpath = wide(lnk_path);
        persist
            .Load(PCWSTR(wpath.as_ptr()), STGM_READ)
            .map_err(|e| crate::error::AppError::Other(e.to_string()))?;

        let mut target_buf = [0u16; 260];
        let mut wfd = WIN32_FIND_DATAW::default();
        // fflags = 0 → resolved path as stored
        let target = match link.GetPath(&mut target_buf, &mut wfd, 0) {
            Ok(()) => from_wide(&target_buf),
            Err(_) => String::new(),
        };

        let mut arg_buf = [0u16; 1024];
        let args = match link.GetArguments(&mut arg_buf) {
            Ok(()) => {
                let s = from_wide(&arg_buf);
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            Err(_) => None,
        };

        Ok((target, args))
    }
}

/// Resolve a dropped file path into a ShortcutRaw. `.lnk` is parsed;
/// anything else (exe/file) is taken as-is.
#[cfg(target_os = "windows")]
pub fn resolve_dropped(path: &str) -> AppResult<ShortcutRaw> {
    let p = std::path::Path::new(path);
    let name = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let is_lnk = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false);
    if is_lnk {
        let (target, args) = parse_lnk(path).unwrap_or_default();
        let target = if target.is_empty() {
            path.to_string()
        } else {
            target
        };
        Ok(ShortcutRaw {
            name,
            lnk_path: path.to_string(),
            target,
            args,
        })
    } else {
        Ok(ShortcutRaw {
            name,
            lnk_path: path.to_string(),
            target: path.to_string(),
            args: None,
        })
    }
}

/// Launch a shortcut / executable by path via ShellExecuteW("open").
#[cfg(target_os = "windows")]
pub fn launch(path: &str) -> AppResult<()> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wpath = wide(path);
    let r = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wpath.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns HINSTANCE > 32 on success.
    if r.0 as usize > 32 {
        Ok(())
    } else {
        Err(crate::error::AppError::Other(format!(
            "ShellExecute failed for {path}"
        )))
    }
}

/// Extract the file's large icon as a `data:image/png;base64,...` URL.
/// Best-effort: returns `Ok(None)` on any failure.
#[cfg(target_os = "windows")]
pub fn icon_data_url(path: &str) -> AppResult<Option<String>> {
    use base64::Engine;
    use std::ffi::c_void;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    unsafe {
        let wpath = wide(path);
        let mut shfi = SHFILEINFOW::default();
        let res = SHGetFileInfoW(
            PCWSTR(wpath.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if res == 0 || shfi.hIcon.is_invalid() {
            return Ok(None);
        }
        let hicon = shfi.hIcon;

        let mut ii = ICONINFO::default();
        if GetIconInfo(hicon, &mut ii).is_err() {
            let _ = DestroyIcon(hicon);
            return Ok(None);
        }

        // Read color bitmap dimensions.
        let mut bm = BITMAP::default();
        let got = GetObjectW(
            HGDIOBJ(ii.hbmColor.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut c_void),
        );
        if got == 0 {
            let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            let _ = DestroyIcon(hicon);
            return Ok(None);
        }
        let width = bm.bmWidth;
        let height = bm.bmHeight;
        if width <= 0 || height <= 0 || width > 512 || height > 512 {
            let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            let _ = DestroyIcon(hicon);
            return Ok(None);
        }

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let pixel_count = (width * height) as usize;
        let mut buf = vec![0u8; pixel_count * 4];

        let hdc = GetDC(None);
        let scan = GetDIBits(
            hdc,
            ii.hbmColor,
            0,
            height as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
        let _ = DestroyIcon(hicon);

        if scan == 0 {
            return Ok(None);
        }

        // GetDIBits gives BGRA; convert to RGBA. If every alpha byte is 0,
        // the icon likely had no alpha channel — treat as fully opaque.
        let any_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2); // B<->R
            if !any_alpha {
                px[3] = 255;
            }
        }

        let img = match image::RgbaImage::from_raw(width as u32, height as u32, buf) {
            Some(i) => i,
            None => return Ok(None),
        };
        let mut png: Vec<u8> = Vec::new();
        if image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            return Ok(None);
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        Ok(Some(format!("data:image/png;base64,{b64}")))
    }
}

// ----- non-windows stubs -----

#[cfg(not(target_os = "windows"))]
pub fn resolve_dropped(path: &str) -> AppResult<ShortcutRaw> {
    let name = std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    Ok(ShortcutRaw {
        name,
        lnk_path: path.to_string(),
        target: path.to_string(),
        args: None,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn launch(_path: &str) -> AppResult<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn icon_data_url(_path: &str) -> AppResult<Option<String>> {
    Ok(None)
}

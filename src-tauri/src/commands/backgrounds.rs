use crate::config;
use crate::db::{backgrounds, kv, Db};
use crate::error::{AppError, AppResult};
use crate::models::{CurrentBackground, PhotoResult};
use crate::pexels;
use base64::Engine;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))
}

/// Primary monitor resolution in physical pixels; falls back to 1920x1080.
fn monitor_size(app: &AppHandle) -> (u32, u32) {
    if let Some(win) = app.get_webview_window("main") {
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let size = monitor.size();
            if size.width > 0 && size.height > 0 {
                return (size.width, size.height);
            }
        }
    }
    (1920, 1080)
}

#[tauri::command]
pub fn config_has_key(app: AppHandle) -> AppResult<bool> {
    let cfg = config::load(&data_dir(&app)?)?;
    Ok(cfg
        .pexels_api_key
        .as_deref()
        .map(|k| !k.is_empty())
        .unwrap_or(false))
}

#[tauri::command]
pub fn config_set_pexels_key(app: AppHandle, key: String) -> AppResult<()> {
    let dir = data_dir(&app)?;
    let mut cfg = config::load(&dir)?;
    cfg.pexels_api_key = Some(key);
    config::save(&dir, &cfg)
}

#[tauri::command]
pub async fn bg_search(app: AppHandle, keyword: String) -> AppResult<Vec<PhotoResult>> {
    let cfg = config::load(&data_dir(&app)?)?;
    let key = cfg
        .pexels_api_key
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AppError::Other("Pexels API key not set".into()))?;
    let (w, h) = monitor_size(&app);
    let orientation = if w >= h { "landscape" } else { "portrait" }.to_string();
    tauri::async_runtime::spawn_blocking(move || pexels::search(&keyword, &key, &orientation))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn bg_download_and_set(
    app: AppHandle,
    db: State<'_, Db>,
    photo: PhotoResult,
    keyword: String,
) -> AppResult<()> {
    let dest = data_dir(&app)?
        .join("backgrounds")
        .join(format!("{}.jpg", photo.id));
    let (w, h) = monitor_size(&app);
    let url = pexels::sized_url(&photo.download_url, w, h);
    let dest_for_dl = dest.clone();
    tauri::async_runtime::spawn_blocking(move || pexels::download(&url, &dest_for_dl))
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;

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

#[tauri::command]
pub fn bg_get_current(app: AppHandle, db: State<Db>) -> AppResult<Option<CurrentBackground>> {
    let _ = app;
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let Some(bg) = backgrounds::get_current(&conn)? else {
        return Ok(None);
    };
    let bytes = std::fs::read(&bg.local_path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(CurrentBackground {
        data_url: format!("data:image/jpeg;base64,{b64}"),
        source_url: bg.source_url,
        author: bg.author,
    }))
}

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

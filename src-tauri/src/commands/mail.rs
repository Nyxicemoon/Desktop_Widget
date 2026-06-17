use crate::config;
use crate::db::{kv, Db};
use crate::error::{AppError, AppResult};
use crate::gmail::{api, auth, GmailState};
use crate::models::{GmailStatus, MailDetail, MailSummary};
use tauri::{AppHandle, Manager, State};

fn data_dir(app: &AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub async fn config_has_google(app: AppHandle) -> AppResult<bool> {
    let cfg = config::load(&data_dir(&app)?)?;
    Ok(cfg
        .google_client_id
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
        && cfg
            .google_client_secret
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false))
}

#[tauri::command]
pub async fn config_set_google(app: AppHandle, id: String, secret: String) -> AppResult<()> {
    let dir = data_dir(&app)?;
    let mut cfg = config::load(&dir)?;
    cfg.google_client_id = Some(id);
    cfg.google_client_secret = Some(secret);
    config::save(&dir, &cfg)
}

#[tauri::command]
pub async fn gmail_status(db: State<'_, Db>) -> AppResult<GmailStatus> {
    let email = {
        let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
        kv::get(&conn, "gmail.email")?.filter(|s| !s.is_empty())
    };
    let connected = email.is_some() && auth::load_refresh()?.is_some();
    Ok(GmailStatus { connected, email })
}

#[tauri::command]
pub async fn gmail_connect(app: AppHandle) -> AppResult<GmailStatus> {
    tauri::async_runtime::spawn_blocking(move || api::connect(&app))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn gmail_disconnect(app: AppHandle, db: State<'_, Db>) -> AppResult<()> {
    auth::delete_refresh()?;
    *app.state::<GmailState>()
        .0
        .lock()
        .map_err(|e| AppError::Other(e.to_string()))? = None;
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    kv::set(&conn, "gmail.email", "")
}

#[tauri::command]
pub async fn mail_list(app: AppHandle) -> AppResult<Vec<MailSummary>> {
    tauri::async_runtime::spawn_blocking(move || api::list(&app, "", 25))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_search(app: AppHandle, query: String) -> AppResult<Vec<MailSummary>> {
    tauri::async_runtime::spawn_blocking(move || api::list(&app, &query, 25))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_get(app: AppHandle, id: String) -> AppResult<MailDetail> {
    tauri::async_runtime::spawn_blocking(move || api::get(&app, &id))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_mark_read(app: AppHandle, id: String, read: bool) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || api::mark_read(&app, &id, read))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn mail_unread_count(app: AppHandle) -> AppResult<i64> {
    tauri::async_runtime::spawn_blocking(move || api::unread_count(&app))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

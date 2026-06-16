use crate::db::{backup, Db};
use crate::error::{AppError, AppResult};
use std::path::Path;
use tauri::{AppHandle, Manager, State};

/// Export the live DB to a user-chosen path (passed from the frontend dialog).
#[tauri::command]
pub fn db_export(db: State<Db>, dest: String) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    backup::export(&conn, &dest)
}

/// Validate a user-chosen backup file and stage it; applied on next restart.
#[tauri::command]
pub fn db_import(app: AppHandle, src: String) -> AppResult<()> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    backup::stage_import(&dir, Path::new(&src))
}

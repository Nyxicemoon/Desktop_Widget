use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use tauri::State;

#[tauri::command]
pub fn kv_set(db: State<Db>, key: String, value: String) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::kv::set(&conn, &key, &value)
}

#[tauri::command]
pub fn kv_get(db: State<Db>, key: String) -> AppResult<Option<String>> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::kv::get(&conn, &key)
}

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::models::GameProfile;
use tauri::State;

#[tauri::command]
pub fn game_get_profile(db: State<Db>) -> AppResult<GameProfile> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::game::get_profile(&conn)
}

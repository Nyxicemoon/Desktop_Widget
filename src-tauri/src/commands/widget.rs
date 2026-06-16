use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use crate::window;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn widget_set_visible(app: AppHandle, kind: String, visible: bool) -> AppResult<()> {
    window::set_widget_visible(&app, &kind, visible)
}

#[tauri::command]
pub fn widget_get_visibility(db: State<Db>) -> AppResult<WidgetVisibility> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    window::read_visibility(&conn)
}

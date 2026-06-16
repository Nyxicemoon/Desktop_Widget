use crate::db::{kv, Db};
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use crate::window;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn widget_set_visible(
    app: AppHandle,
    db: State<Db>,
    kind: String,
    visible: bool,
) -> AppResult<()> {
    if visible {
        window::open_widget(&app, &kind)?;
    } else {
        window::close_widget(&app, &kind)?;
    }
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    kv::set(
        &conn,
        &format!("widget.{kind}.visible"),
        if visible { "1" } else { "0" },
    )
}

#[tauri::command]
pub fn widget_get_visibility(db: State<Db>) -> AppResult<WidgetVisibility> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    window::read_visibility(&conn)
}

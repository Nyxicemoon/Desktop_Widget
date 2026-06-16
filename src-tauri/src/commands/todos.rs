use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::models::{Todo, ToggleResult};
use tauri::State;

#[tauri::command]
pub fn todo_create(
    db: State<Db>,
    title: String,
    note: Option<String>,
    due_date: Option<String>,
) -> AppResult<Todo> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::create(&conn, &title, note.as_deref(), due_date.as_deref())
}

#[tauri::command]
pub fn todo_update(
    db: State<Db>,
    id: i64,
    title: String,
    note: Option<String>,
    due_date: Option<String>,
) -> AppResult<Todo> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::update(&conn, id, &title, note.as_deref(), due_date.as_deref())
}

#[tauri::command]
pub fn todo_delete(db: State<Db>, id: i64) -> AppResult<()> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::delete(&conn, id)
}

#[tauri::command]
pub fn todo_list_today(db: State<Db>) -> AppResult<Vec<Todo>> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::list_today(&conn)
}

#[tauri::command]
pub fn todo_toggle_done(db: State<Db>, id: i64) -> AppResult<ToggleResult> {
    let mut conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::todos::toggle_done(&mut conn, id)
}

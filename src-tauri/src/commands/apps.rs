use crate::db::{apps, Db};
use crate::error::{AppError, AppResult};
use crate::models::AppEntry;
use crate::system::shortcuts;
use tauri::State;

fn lock<'a>(db: &'a State<'a, Db>) -> AppResult<std::sync::MutexGuard<'a, rusqlite::Connection>> {
    db.0.lock().map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn app_list(db: State<Db>) -> AppResult<Vec<AppEntry>> {
    let conn = lock(&db)?;
    apps::list(&conn)
}

#[tauri::command]
pub fn app_icon(path: String) -> AppResult<Option<String>> {
    shortcuts::icon_data_url(&path)
}

#[tauri::command]
pub fn app_launch(path: String) -> AppResult<()> {
    shortcuts::launch(&path)
}

#[tauri::command]
pub fn app_add_dropped(db: State<Db>, path: String) -> AppResult<()> {
    let r = shortcuts::resolve_dropped(&path)?;
    let conn = lock(&db)?;
    apps::add(&conn, &r.name, &r.target, r.args.as_deref())
}

#[tauri::command]
pub fn app_remove(db: State<Db>, id: i64) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::remove(&conn, id)
}

#[tauri::command]
pub fn app_rename(db: State<Db>, id: i64, name: String) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::rename(&conn, id, &name)
}

#[tauri::command]
pub fn app_reorder(db: State<Db>, ids: Vec<i64>) -> AppResult<()> {
    let mut conn = lock(&db)?;
    apps::reorder(&mut conn, &ids)
}

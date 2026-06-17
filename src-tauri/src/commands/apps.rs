use crate::db::{apps, Db};
use crate::error::{AppError, AppResult};
use crate::models::AppEntry;
use crate::system::shortcuts;
use tauri::State;

fn lock<'a>(db: &'a State<'a, Db>) -> AppResult<std::sync::MutexGuard<'a, rusqlite::Connection>> {
    db.0.lock().map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub fn apps_scan(db: State<Db>) -> AppResult<Vec<AppEntry>> {
    let scanned = shortcuts::scan().unwrap_or_default();
    let conn = lock(&db)?;
    let custom = apps::list_custom(&conn)?;
    let prefs = apps::prefs_map(&conn)?;
    Ok(apps::merge(scanned, custom, &prefs))
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
    apps::add_custom(&conn, &r.name, &r.target, r.args.as_deref())
}

#[tauri::command]
pub fn app_remove_custom(db: State<Db>, target: String) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::remove_custom(&conn, &target)
}

#[tauri::command]
pub fn app_set_favorite(db: State<Db>, target: String, favorite: bool) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::set_favorite(&conn, &target, favorite)
}

#[tauri::command]
pub fn app_set_category(db: State<Db>, target: String, category: Option<String>) -> AppResult<()> {
    let conn = lock(&db)?;
    apps::set_category(&conn, &target, category.as_deref())
}

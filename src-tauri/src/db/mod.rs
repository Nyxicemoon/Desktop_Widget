pub mod game;
pub mod kv;
pub mod migrations;
pub mod todos;

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Tauri-managed database state. Guard the connection with the mutex before use.
pub struct Db(pub Mutex<Connection>);

/// Open (creating if needed) the SQLite database at `app_data_dir/deskhub.db`
/// and run pending migrations.
pub fn open(app: &AppHandle) -> AppResult<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    let mut conn = Connection::open(dir.join("deskhub.db"))?;
    migrations::apply(&mut conn)?;
    Ok(conn)
}

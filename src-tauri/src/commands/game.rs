use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::models::{GameProfile, GameStatus};
use tauri::State;

#[tauri::command]
pub fn game_get_profile(db: State<Db>) -> AppResult<GameProfile> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    db::game::get_profile(&conn)
}

#[tauri::command]
pub fn game_status(db: State<Db>) -> AppResult<GameStatus> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let _ = db::game::settle_idle(&conn, db::game::OFFLINE_CAP_SECS);
    let p = db::game::get_profile(&conn)?;
    let cum = 100 * (p.level - 1) * p.level / 2;
    Ok(GameStatus {
        coins: p.coins,
        exp: p.exp,
        level: p.level,
        exp_into_level: p.exp - cum,
        exp_for_next: 100 * p.level,
        rate_per_min: db::game::rate_per_min(p.level),
    })
}

#[tauri::command]
pub fn game_take_offline_earned(db: State<Db>) -> AppResult<i64> {
    let conn = db.0.lock().map_err(|e| AppError::Other(e.to_string()))?;
    let v = db::kv::get(&conn, "idle.offline_earned")?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    db::kv::set(&conn, "idle.offline_earned", "0")?;
    Ok(v)
}

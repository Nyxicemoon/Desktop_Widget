//! Shared data structures.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub note: Option<String>,
    pub done: bool,
    pub due_date: Option<String>,
    pub reward_coin: i64,
    pub created_at: String,
    pub done_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GameProfile {
    pub coins: i64,
    pub exp: i64,
    pub level: i64,
    pub last_tick: String,
}

#[derive(Debug, Serialize)]
pub struct ToggleResult {
    pub todo: Todo,
    pub awarded: i64,
    pub coins: i64,
}

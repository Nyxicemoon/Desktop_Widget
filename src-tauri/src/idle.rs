//! Idle-game production loop: settle passive coin production every 60s on a
//! dedicated, mostly-sleeping OS thread. Best-effort — failures are ignored.

use crate::db::{game, Db};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn spawn_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let state = app.state::<Db>();
        let conn = match state.0.lock() {
            Ok(c) => c,
            Err(_) => continue,
        };
        let _ = game::settle_idle(&conn, game::OFFLINE_CAP_SECS);
    });
}

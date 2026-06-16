//! Due-task reminders. A background OS thread checks once at startup and then
//! every 60 minutes, sending a local notification for each due, undone todo
//! that has not been notified before (de-duped via kv `reminder:notified:<id>`).

use crate::db::{kv, todos, Db};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Check due todos and notify for any not yet notified. Best-effort: any lock
/// or query failure is swallowed so the loop keeps running.
pub fn check_and_notify(app: &AppHandle) {
    let state = app.state::<Db>();
    let to_notify: Vec<String> = {
        let conn = match state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let due = match todos::list_due(&conn) {
            Ok(d) => d,
            Err(_) => return,
        };
        let mut pending = Vec::new();
        for (id, title) in due {
            let key = format!("reminder:notified:{id}");
            let seen = kv::get(&conn, &key).unwrap_or(None).is_some();
            if !seen {
                let _ = kv::set(&conn, &key, "1");
                pending.push(title);
            }
        }
        pending
    };
    for title in to_notify {
        let _ = app
            .notification()
            .builder()
            .title("任务到期 / Task due")
            .body(&title)
            .show();
    }
}

/// Spawn the reminder loop on a dedicated, mostly-sleeping OS thread.
pub fn spawn_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        check_and_notify(&app);
        std::thread::sleep(CHECK_INTERVAL);
    });
}

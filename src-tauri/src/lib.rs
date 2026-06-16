mod commands;
mod config;
mod db;
mod error;
mod models;
mod pexels;
mod reminder;
mod system;
mod tray;
mod window;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let conn = db::open(app.handle())?;
            app.manage(db::Db(std::sync::Mutex::new(conn)));

            let vis = {
                let state = app.state::<db::Db>();
                let conn = state.0.lock().map_err(|e| e.to_string())?;
                window::read_visibility(&conn).unwrap_or(crate::models::WidgetVisibility {
                    todo: false,
                    coins: false,
                })
            };
            if vis.todo {
                let _ = window::open_widget(app.handle(), "todo");
            }
            if vis.coins {
                let _ = window::open_widget(app.handle(), "coins");
            }

            tray::create(app.handle())?;
            reminder::spawn_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::kv::kv_get,
            commands::kv::kv_set,
            commands::todos::todo_create,
            commands::todos::todo_update,
            commands::todos::todo_delete,
            commands::todos::todo_list_today,
            commands::todos::todo_toggle_done,
            commands::game::game_get_profile,
            commands::backgrounds::config_has_key,
            commands::backgrounds::config_set_pexels_key,
            commands::backgrounds::bg_search,
            commands::backgrounds::bg_download_and_set,
            commands::backgrounds::bg_get_current,
            commands::backgrounds::bg_restore_default,
            commands::backup::db_export,
            commands::backup::db_import,
            commands::widget::widget_set_visible,
            commands::widget::widget_get_visibility
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

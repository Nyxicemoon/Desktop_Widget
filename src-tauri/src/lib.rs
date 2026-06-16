mod commands;
mod config;
mod db;
mod error;
mod models;
mod pexels;
mod system;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let conn = db::open(app.handle())?;
            app.manage(db::Db(std::sync::Mutex::new(conn)));
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
            commands::backgrounds::bg_restore_default
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

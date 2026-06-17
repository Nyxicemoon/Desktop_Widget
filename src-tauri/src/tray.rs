use crate::db::Db;
use crate::window;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn spawn_toggle(app: &AppHandle, kind: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let current = {
            let state = app.state::<Db>();
            let guard = match state.0.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            window::read_visibility(&guard)
                .map(|v| match kind {
                    "todo" => v.todo,
                    "coins" => v.coins,
                    "apps" => v.apps,
                    _ => false,
                })
                .unwrap_or(false)
        };
        let _ = window::set_widget_visible(&app, kind, !current);
    });
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show_main", "显示主窗口 / Show", true, None::<&str>)?;
    let todo = MenuItem::with_id(app, "toggle_todo", "Todo 组件", true, None::<&str>)?;
    let coins = MenuItem::with_id(app, "toggle_coins", "金币组件", true, None::<&str>)?;
    let apps = MenuItem::with_id(app, "toggle_apps", "显示/隐藏 应用 / Toggle Apps", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 / Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &todo, &coins, &apps, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("DeskHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_main" => show_main(app),
            "toggle_todo" => spawn_toggle(app, "todo"),
            "toggle_coins" => spawn_toggle(app, "coins"),
            "toggle_apps" => spawn_toggle(app, "apps"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

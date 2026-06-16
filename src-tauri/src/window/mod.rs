use crate::db::kv;
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use rusqlite::Connection;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// (window label, frontend route, width, height, default x, default y)
pub fn widget_config(kind: &str) -> AppResult<(&'static str, &'static str, f64, f64, f64, f64)> {
    match kind {
        "todo" => Ok(("widget-todo", "/widgets/todo", 280.0, 360.0, 40.0, 40.0)),
        "coins" => Ok(("widget-coins", "/widgets/coins", 200.0, 90.0, 360.0, 40.0)),
        other => Err(AppError::Other(format!("unknown widget kind: {other}"))),
    }
}

pub fn read_visibility(conn: &Connection) -> AppResult<WidgetVisibility> {
    Ok(WidgetVisibility {
        todo: kv::get(conn, "widget.todo.visible")?.as_deref() == Some("1"),
        coins: kv::get(conn, "widget.coins.visible")?.as_deref() == Some("1"),
    })
}

pub fn open_widget(app: &AppHandle, kind: &str) -> AppResult<()> {
    let (label, route, w, h, x, y) = widget_config(kind)?;
    if let Some(win) = app.get_webview_window(label) {
        win.show().map_err(|e| AppError::Other(e.to_string()))?;
        pin_to_desktop(&win)?;
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, label, WebviewUrl::App(route.into()))
        .transparent(true)
        .decorations(false)
        .skip_taskbar(true)
        .shadow(false)
        .always_on_top(false)
        .resizable(false)
        .inner_size(w, h)
        .position(x, y)
        .build()
        .map_err(|e| AppError::Other(e.to_string()))?;
    pin_to_desktop(&win)
}

pub fn close_widget(app: &AppHandle, kind: &str) -> AppResult<()> {
    let (label, ..) = widget_config(kind)?;
    if let Some(win) = app.get_webview_window(label) {
        win.close().map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn pin_to_desktop(win: &WebviewWindow) -> AppResult<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_BOTTOM,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_NOACTIVATE,
    };
    let raw = win.hwnd().map_err(|e| AppError::Other(e.to_string()))?;
    let hwnd = HWND(raw.0);
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 as isize));
        SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|e| AppError::Other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn pin_to_desktop(_win: &WebviewWindow) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        conn
    }

    #[test]
    fn visibility_defaults_false() {
        let conn = setup();
        let v = read_visibility(&conn).unwrap();
        assert!(!v.todo);
        assert!(!v.coins);
    }

    #[test]
    fn visibility_reflects_kv() {
        let conn = setup();
        kv::set(&conn, "widget.todo.visible", "1").unwrap();
        let v = read_visibility(&conn).unwrap();
        assert!(v.todo);
        assert!(!v.coins);
    }

    #[test]
    fn widget_config_unknown_errors() {
        assert!(widget_config("nope").is_err());
        assert_eq!(widget_config("todo").unwrap().0, "widget-todo");
    }
}

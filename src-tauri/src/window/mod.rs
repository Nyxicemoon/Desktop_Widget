use crate::db::kv;
use crate::error::{AppError, AppResult};
use crate::models::WidgetVisibility;
use rusqlite::Connection;

/// (window label, frontend route, default width, default height)
pub fn widget_config(kind: &str) -> AppResult<(&'static str, &'static str, f64, f64)> {
    match kind {
        "todo" => Ok(("widget-todo", "/widgets/todo", 280.0, 360.0)),
        "coins" => Ok(("widget-coins", "/widgets/coins", 200.0, 90.0)),
        other => Err(AppError::Other(format!("unknown widget kind: {other}"))),
    }
}

pub fn read_visibility(conn: &Connection) -> AppResult<WidgetVisibility> {
    Ok(WidgetVisibility {
        todo: kv::get(conn, "widget.todo.visible")?.as_deref() == Some("1"),
        coins: kv::get(conn, "widget.coins.visible")?.as_deref() == Some("1"),
    })
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

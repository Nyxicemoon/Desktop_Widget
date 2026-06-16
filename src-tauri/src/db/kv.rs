use crate::error::AppResult;
use rusqlite::{Connection, OptionalExtension};

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        (key, value),
    )?;
    Ok(())
}

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row("SELECT value FROM kv WHERE key = ?1", [key], |row| row.get(0))
        .optional()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        conn
    }

    #[test]
    fn set_then_get_returns_value() {
        let conn = setup();
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme").unwrap(), Some("dark".to_string()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let conn = setup();
        assert_eq!(get(&conn, "missing").unwrap(), None);
    }

    #[test]
    fn value_persists_across_reopen() {
        // Mirrors the "theme persists across app restart" acceptance criterion:
        // write to a real file, drop the connection (app close), reopen (app restart), read back.
        let path = std::env::temp_dir().join(format!("deskhub_persist_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let mut conn = Connection::open(&path).unwrap();
            migrations::apply(&mut conn).unwrap();
            set(&conn, "theme", "dark").unwrap();
        } // connection dropped == app closed

        {
            let mut conn = Connection::open(&path).unwrap();
            migrations::apply(&mut conn).unwrap(); // idempotent on restart
            assert_eq!(get(&conn, "theme").unwrap(), Some("dark".to_string()));
        }

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn set_overwrites_value_and_keeps_single_row() {
        let conn = setup();
        set(&conn, "theme", "light").unwrap();
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme").unwrap(), Some("dark".to_string()));
        let row_count: i32 = conn
            .query_row("SELECT count(*) FROM kv", [], |r| r.get(0))
            .unwrap();
        assert_eq!(row_count, 1);
        let updated_at: String = conn
            .query_row("SELECT updated_at FROM kv WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert!(!updated_at.is_empty());
    }
}

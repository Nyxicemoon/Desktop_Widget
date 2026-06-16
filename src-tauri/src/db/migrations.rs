use crate::error::AppResult;
use rusqlite::Connection;

const MIGRATIONS: &[(i32, &str)] = &[(
    1,
    "CREATE TABLE kv (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
)];

pub fn apply(conn: &mut Connection) -> AppResult<()> {
    let current: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (version, sql) in MIGRATIONS {
        if *version > current {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", *version)?;
            tx.commit()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn version(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn applies_migrations_on_empty_db() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        assert_eq!(version(&conn), 1);
        let table_count: i32 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='kv'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn apply_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        apply(&mut conn).unwrap(); // 不应报 "table already exists"
        assert_eq!(version(&conn), 1);
    }
}

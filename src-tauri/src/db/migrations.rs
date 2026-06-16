use crate::error::AppResult;
use rusqlite::Connection;

const MIGRATIONS: &[(i32, &str)] = &[
    (
        1,
        "CREATE TABLE kv (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ),
    (
        2,
        "CREATE TABLE todos (
            id          INTEGER PRIMARY KEY,
            title       TEXT NOT NULL,
            note        TEXT,
            done        INTEGER NOT NULL DEFAULT 0,
            due_date    TEXT,
            reward_coin INTEGER NOT NULL DEFAULT 10,
            created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            done_at     TEXT
        );
        CREATE TABLE game_profile (
            id        INTEGER PRIMARY KEY CHECK (id = 1),
            coins     INTEGER NOT NULL DEFAULT 0,
            exp       INTEGER NOT NULL DEFAULT 0,
            level     INTEGER NOT NULL DEFAULT 1,
            last_tick TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE coin_ledger (
            id         INTEGER PRIMARY KEY,
            amount     INTEGER NOT NULL,
            reason     TEXT NOT NULL,
            ref_id     INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    ),
    (
        3,
        "CREATE TABLE backgrounds (
            id         INTEGER PRIMARY KEY,
            local_path TEXT NOT NULL,
            source_url TEXT NOT NULL,
            author     TEXT,
            license    TEXT,
            keyword    TEXT,
            is_current INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    ),
];

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
        let latest = MIGRATIONS.last().unwrap().0;
        assert_eq!(version(&conn), latest);
        for t in ["kv", "todos", "game_profile", "coin_ledger", "backgrounds"] {
            let c: i32 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(c, 1, "table {t} missing");
        }
    }

    #[test]
    fn apply_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn).unwrap();
        apply(&mut conn).unwrap(); // 不应报 "table already exists"
        assert_eq!(version(&conn), MIGRATIONS.last().unwrap().0);
    }
}

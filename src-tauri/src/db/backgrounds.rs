use crate::error::{AppError, AppResult};
use crate::models::Background;
use rusqlite::{Connection, OptionalExtension, Row};

const COLS: &str = "id, local_path, source_url, author, license, keyword, is_current, created_at";

fn row_to_bg(row: &Row) -> rusqlite::Result<Background> {
    Ok(Background {
        id: row.get("id")?,
        local_path: row.get("local_path")?,
        source_url: row.get("source_url")?,
        author: row.get("author")?,
        license: row.get("license")?,
        keyword: row.get("keyword")?,
        is_current: row.get("is_current")?,
        created_at: row.get("created_at")?,
    })
}

pub fn insert(
    conn: &Connection,
    local_path: &str,
    source_url: &str,
    author: Option<&str>,
    license: Option<&str>,
    keyword: Option<&str>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO backgrounds (local_path, source_url, author, license, keyword)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (local_path, source_url, author, license, keyword),
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_current(conn: &mut Connection, id: i64) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE backgrounds SET is_current = 0", [])?;
    let n = tx.execute("UPDATE backgrounds SET is_current = 1 WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("background {id}")));
    }
    tx.commit()?;
    Ok(())
}

pub fn get_current(conn: &Connection) -> AppResult<Option<Background>> {
    let sql = format!("SELECT {COLS} FROM backgrounds WHERE is_current = 1");
    Ok(conn.query_row(&sql, [], row_to_bg).optional()?)
}

pub fn restore_default(conn: &Connection) -> AppResult<()> {
    conn.execute("UPDATE backgrounds SET is_current = 0", [])?;
    Ok(())
}

#[allow(dead_code)]
pub fn list(conn: &Connection) -> AppResult<Vec<Background>> {
    let sql = format!("SELECT {COLS} FROM backgrounds ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_bg)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
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
    fn insert_get_current_set_current_unique() {
        let mut conn = setup();
        assert!(get_current(&conn).unwrap().is_none());

        let a = insert(
            &conn,
            "/p/a.jpg",
            "http://src/a",
            Some("A"),
            Some("Pexels License"),
            Some("forest"),
        )
        .unwrap();
        let b = insert(
            &conn,
            "/p/b.jpg",
            "http://src/b",
            Some("B"),
            Some("Pexels License"),
            Some("lake"),
        )
        .unwrap();

        set_current(&mut conn, a).unwrap();
        assert_eq!(get_current(&conn).unwrap().unwrap().id, a);

        set_current(&mut conn, b).unwrap();
        let cur = get_current(&conn).unwrap().unwrap();
        assert_eq!(cur.id, b);
        let current_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM backgrounds WHERE is_current = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(current_count, 1);
    }

    #[test]
    fn restore_default_clears_current() {
        let mut conn = setup();
        let a = insert(&conn, "/p/a.jpg", "http://src/a", None, None, None).unwrap();
        set_current(&mut conn, a).unwrap();
        restore_default(&conn).unwrap();
        assert!(get_current(&conn).unwrap().is_none());
    }

    #[test]
    fn set_current_missing_errors() {
        let mut conn = setup();
        assert!(set_current(&mut conn, 999).is_err());
    }
}

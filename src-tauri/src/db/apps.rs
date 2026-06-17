use crate::error::AppResult;
use crate::models::AppEntry;
use rusqlite::Connection;

/// All curated apps, ordered.
pub fn list(conn: &Connection) -> AppResult<Vec<AppEntry>> {
    let mut stmt =
        conn.prepare("SELECT id, name, target, args FROM custom_apps ORDER BY sort_order, id")?;
    let rows = stmt.query_map([], |r| {
        Ok(AppEntry {
            id: r.get(0)?,
            name: r.get(1)?,
            target: r.get(2)?,
            args: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Add a curated app (de-dup by lowercased target); appended at the end.
pub fn add(conn: &Connection, name: &str, target: &str, args: Option<&str>) -> AppResult<()> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM custom_apps WHERE lower(target) = lower(?1) LIMIT 1",
            [target],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if exists {
        return Ok(());
    }
    let next: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM custom_apps",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO custom_apps (name, target, args, sort_order) VALUES (?1, ?2, ?3, ?4)",
        (name, target, args, next),
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM custom_apps WHERE id = ?1", [id])?;
    Ok(())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> AppResult<()> {
    conn.execute("UPDATE custom_apps SET name = ?1 WHERE id = ?2", (name, id))?;
    Ok(())
}

/// Persist a new order: sort_order = position in `ids`.
pub fn reorder(conn: &mut Connection, ids: &[i64]) -> AppResult<()> {
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE custom_apps SET sort_order = ?1 WHERE id = ?2",
            (i as i64, id),
        )?;
    }
    tx.commit()?;
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
    fn add_dedups_by_target_case_insensitive() {
        let conn = setup();
        add(&conn, "App", "C:\\a\\app.exe", None).unwrap();
        add(&conn, "App2", "C:\\A\\APP.EXE", None).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn add_appends_in_order_then_reorder() {
        let mut conn = setup();
        add(&conn, "A", "C:\\a.exe", None).unwrap();
        add(&conn, "B", "C:\\b.exe", None).unwrap();
        add(&conn, "C", "C:\\c.exe", None).unwrap();
        let l = list(&conn).unwrap();
        assert_eq!(l.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);

        let ids: Vec<i64> = vec![l[2].id, l[0].id, l[1].id]; // C, A, B
        reorder(&mut conn, &ids).unwrap();
        let l2 = list(&conn).unwrap();
        assert_eq!(l2.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["C", "A", "B"]);
    }

    #[test]
    fn rename_and_remove() {
        let conn = setup();
        add(&conn, "Old", "C:\\a.exe", None).unwrap();
        let id = list(&conn).unwrap()[0].id;
        rename(&conn, id, "New").unwrap();
        assert_eq!(list(&conn).unwrap()[0].name, "New");
        remove(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}

use crate::db::game;
use crate::error::{AppError, AppResult};
use crate::models::{Todo, ToggleResult};
use rusqlite::{Connection, OptionalExtension, Row};

const COLS: &str = "id, title, note, done, due_date, reward_coin, created_at, done_at";

fn row_to_todo(row: &Row) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: row.get("id")?,
        title: row.get("title")?,
        note: row.get("note")?,
        done: row.get("done")?,
        due_date: row.get("due_date")?,
        reward_coin: row.get("reward_coin")?,
        created_at: row.get("created_at")?,
        done_at: row.get("done_at")?,
    })
}

fn get_by_id(conn: &Connection, id: i64) -> AppResult<Todo> {
    let sql = format!("SELECT {COLS} FROM todos WHERE id = ?1");
    conn.query_row(&sql, [id], row_to_todo)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("todo {id}")))
}

pub fn create(
    conn: &Connection,
    title: &str,
    note: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<Todo> {
    conn.execute(
        "INSERT INTO todos (title, note, due_date) VALUES (?1, ?2, ?3)",
        (title, note, due_date),
    )?;
    get_by_id(conn, conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    note: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<Todo> {
    let n = conn.execute(
        "UPDATE todos SET title = ?1, note = ?2, due_date = ?3 WHERE id = ?4",
        (title, note, due_date, id),
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("todo {id}")));
    }
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM todos WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("todo {id}")));
    }
    Ok(())
}

pub fn list_today(conn: &Connection) -> AppResult<Vec<Todo>> {
    let sql = format!(
        "SELECT {COLS} FROM todos
         WHERE done = 0 OR (done = 1 AND date(done_at) = date('now','localtime'))
         ORDER BY done ASC, created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_todo)?;
    let mut todos = Vec::new();
    for r in rows {
        todos.push(r?);
    }
    Ok(todos)
}

pub fn toggle_done(conn: &mut Connection, id: i64) -> AppResult<ToggleResult> {
    let tx = conn.transaction()?;
    let current = get_by_id(&tx, id)?;
    let awarded = if !current.done {
        tx.execute(
            "UPDATE todos SET done = 1, done_at = datetime('now','localtime') WHERE id = ?1",
            [id],
        )?;
        game::award_for_todo(&tx, id, current.reward_coin)?
    } else {
        tx.execute(
            "UPDATE todos SET done = 0, done_at = NULL WHERE id = ?1",
            [id],
        )?;
        0
    };
    let todo = get_by_id(&tx, id)?;
    let coins: i64 =
        tx.query_row("SELECT coins FROM game_profile WHERE id = 1", [], |r| r.get(0))?;
    tx.commit()?;
    Ok(ToggleResult {
        todo,
        awarded,
        coins,
    })
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
    fn create_update_delete() {
        let conn = setup();
        let t = create(&conn, "task", None, None).unwrap();
        assert_eq!(t.title, "task");
        assert!(!t.done);

        let u = update(&conn, t.id, "renamed", Some("memo"), Some("2026-06-16")).unwrap();
        assert_eq!(u.title, "renamed");
        assert_eq!(u.note.as_deref(), Some("memo"));

        delete(&conn, t.id).unwrap();
        assert!(get_by_id(&conn, t.id).is_err());
    }

    #[test]
    fn list_today_includes_incomplete_and_today_done_excludes_old_done() {
        let conn = setup();
        create(&conn, "open", None, None).unwrap();
        // a task completed yesterday should NOT appear
        conn.execute(
            "INSERT INTO todos (title, done, done_at)
             VALUES ('old', 1, datetime('now','localtime','-1 day'))",
            [],
        )
        .unwrap();
        let list = list_today(&conn).unwrap();
        let titles: Vec<&str> = list.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"open"));
        assert!(!titles.contains(&"old"));
    }

    #[test]
    fn toggle_awards_once_no_refund_no_reaward() {
        let mut conn = setup();
        let t = create(&conn, "task", None, None).unwrap();

        // complete -> award 10
        let r1 = toggle_done(&mut conn, t.id).unwrap();
        assert!(r1.todo.done);
        assert_eq!(r1.awarded, 10);
        assert_eq!(r1.coins, 10);

        // un-complete -> no refund
        let r2 = toggle_done(&mut conn, t.id).unwrap();
        assert!(!r2.todo.done);
        assert_eq!(r2.awarded, 0);
        assert_eq!(r2.coins, 10);

        // re-complete -> no re-award
        let r3 = toggle_done(&mut conn, t.id).unwrap();
        assert!(r3.todo.done);
        assert_eq!(r3.awarded, 0);
        assert_eq!(r3.coins, 10);
    }
}

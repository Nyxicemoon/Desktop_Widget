use crate::error::AppResult;
use crate::models::GameProfile;
use rusqlite::Connection;

pub fn ensure_profile(conn: &Connection) -> AppResult<()> {
    conn.execute("INSERT OR IGNORE INTO game_profile (id) VALUES (1)", [])?;
    Ok(())
}

pub fn get_profile(conn: &Connection) -> AppResult<GameProfile> {
    ensure_profile(conn)?;
    let profile = conn.query_row(
        "SELECT coins, exp, level, last_tick FROM game_profile WHERE id = 1",
        [],
        |r| {
            Ok(GameProfile {
                coins: r.get(0)?,
                exp: r.get(1)?,
                level: r.get(2)?,
                last_tick: r.get(3)?,
            })
        },
    )?;
    Ok(profile)
}

/// Award coins for completing a todo, deduplicated by ledger `ref_id`.
/// Returns the amount actually awarded (0 if this todo was already rewarded).
/// Call inside the caller's transaction.
pub fn award_for_todo(conn: &Connection, todo_id: i64, amount: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let already: i64 = conn.query_row(
        "SELECT count(*) FROM coin_ledger WHERE reason = 'todo_done' AND ref_id = ?1",
        [todo_id],
        |r| r.get(0),
    )?;
    if already > 0 {
        return Ok(0);
    }
    conn.execute(
        "INSERT INTO coin_ledger (amount, reason, ref_id) VALUES (?1, 'todo_done', ?2)",
        (amount, todo_id),
    )?;
    conn.execute(
        "UPDATE game_profile SET coins = coins + ?1 WHERE id = 1",
        [amount],
    )?;
    Ok(amount)
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
    fn ensure_profile_is_idempotent_and_starts_at_zero() {
        let conn = setup();
        ensure_profile(&conn).unwrap();
        ensure_profile(&conn).unwrap();
        let p = get_profile(&conn).unwrap();
        assert_eq!(p.coins, 0);
        assert_eq!(p.level, 1);
    }

    #[test]
    fn award_adds_once_and_dedups_by_ref_id() {
        let conn = setup();
        assert_eq!(award_for_todo(&conn, 42, 10).unwrap(), 10);
        assert_eq!(get_profile(&conn).unwrap().coins, 10);
        // same todo again -> no double reward
        assert_eq!(award_for_todo(&conn, 42, 10).unwrap(), 0);
        assert_eq!(get_profile(&conn).unwrap().coins, 10);
    }
}

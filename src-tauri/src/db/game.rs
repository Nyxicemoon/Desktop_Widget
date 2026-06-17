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
    add_exp(conn, 5)?;
    Ok(amount)
}

/// Passive coin production rate per minute, derived from level.
pub fn rate_per_min(level: i64) -> i64 {
    level.max(1)
}

/// Cumulative exp required to *reach* `level` = 100 * (L-1) * L / 2.
fn cumulative(level: i64) -> i64 {
    100 * (level - 1) * level / 2
}

/// Largest level L with cumulative(L) <= exp (L >= 1).
pub fn level_for_exp(exp: i64) -> i64 {
    let mut l = 1;
    while cumulative(l + 1) <= exp {
        l += 1;
    }
    l
}

/// Coins produced over `elapsed_secs` at `level`, floored to whole minutes, capped.
pub fn compute_earned(elapsed_secs: i64, level: i64, cap_secs: i64) -> i64 {
    let s = elapsed_secs.clamp(0, cap_secs);
    (s / 60) * rate_per_min(level)
}

/// Offline-earnings cap: 8 hours.
pub const OFFLINE_CAP_SECS: i64 = 8 * 3600;

/// Add exp and recompute level. Call inside the caller's transaction if any.
fn add_exp(conn: &Connection, amount: i64) -> AppResult<()> {
    conn.execute("UPDATE game_profile SET exp = exp + ?1 WHERE id = 1", [amount])?;
    let exp: i64 = conn.query_row("SELECT exp FROM game_profile WHERE id = 1", [], |r| r.get(0))?;
    conn.execute(
        "UPDATE game_profile SET level = ?1 WHERE id = 1",
        [level_for_exp(exp)],
    )?;
    Ok(())
}

/// Settle idle production since `last_tick` (capped). Returns coins earned.
/// Only advances `last_tick` when at least one whole minute was credited.
pub fn settle_idle(conn: &Connection, cap_secs: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let (level, elapsed): (i64, i64) = conn.query_row(
        "SELECT level,
                CAST((julianday('now','localtime') - julianday(last_tick)) * 86400 AS INTEGER)
         FROM game_profile WHERE id = 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let earned = compute_earned(elapsed.max(0), level, cap_secs);
    if earned > 0 {
        conn.execute(
            "INSERT INTO coin_ledger (amount, reason) VALUES (?1, 'idle')",
            [earned],
        )?;
        conn.execute(
            "UPDATE game_profile SET coins = coins + ?1, last_tick = datetime('now','localtime') WHERE id = 1",
            [earned],
        )?;
    }
    Ok(earned)
}

/// Reward the first read of an email (deduped via kv). Returns coins awarded (0 if already).
pub fn award_for_mail(conn: &Connection, msg_id: &str, coins: i64, exp: i64) -> AppResult<i64> {
    ensure_profile(conn)?;
    let key = format!("reward:mail:{msg_id}");
    if crate::db::kv::get(conn, &key)?.is_some() {
        return Ok(0);
    }
    conn.execute(
        "INSERT INTO coin_ledger (amount, reason) VALUES (?1, 'mail_read')",
        [coins],
    )?;
    conn.execute("UPDATE game_profile SET coins = coins + ?1 WHERE id = 1", [coins])?;
    add_exp(conn, exp)?;
    crate::db::kv::set(conn, &key, "1")?;
    Ok(coins)
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

    #[test]
    fn level_curve() {
        assert_eq!(level_for_exp(0), 1);
        assert_eq!(level_for_exp(99), 1);
        assert_eq!(level_for_exp(100), 2);
        assert_eq!(level_for_exp(299), 2);
        assert_eq!(level_for_exp(300), 3);
    }

    #[test]
    fn earned_floors_minutes_and_caps() {
        assert_eq!(compute_earned(59, 1, OFFLINE_CAP_SECS), 0);
        assert_eq!(compute_earned(60, 1, OFFLINE_CAP_SECS), 1);
        assert_eq!(compute_earned(600, 3, OFFLINE_CAP_SECS), 30);
        assert_eq!(compute_earned(100_000, 1, OFFLINE_CAP_SECS), 480);
    }

    #[test]
    fn settle_idle_credits_capped() {
        let conn = setup();
        ensure_profile(&conn).unwrap();
        conn.execute(
            "UPDATE game_profile SET last_tick = datetime('now','localtime','-2 hours') WHERE id = 1",
            [],
        )
        .unwrap();
        let earned = settle_idle(&conn, OFFLINE_CAP_SECS).unwrap();
        assert_eq!(earned, 120); // 2h * 60 * level 1
        assert_eq!(get_profile(&conn).unwrap().coins, 120);
    }

    #[test]
    fn mail_reward_dedups() {
        let conn = setup();
        assert_eq!(award_for_mail(&conn, "abc", 2, 3).unwrap(), 2);
        assert_eq!(award_for_mail(&conn, "abc", 2, 3).unwrap(), 0);
        let p = get_profile(&conn).unwrap();
        assert_eq!(p.coins, 2);
        assert_eq!(p.exp, 3);
    }

    #[test]
    fn todo_award_grants_exp() {
        let conn = setup();
        award_for_todo(&conn, 1, 10).unwrap();
        assert_eq!(get_profile(&conn).unwrap().exp, 5);
    }
}

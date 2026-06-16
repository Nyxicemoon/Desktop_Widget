//! Database backup: export a clean copy, validate/stage an import,
//! and apply a staged import at startup.

use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// Export a clean, consistent copy of the live DB to `dest` (overwrites).
/// Uses SQLite `VACUUM INTO`, which produces a single compact file and
/// handles WAL correctly. `VACUUM INTO` requires the destination not exist.
#[allow(dead_code)]
pub fn export(conn: &Connection, dest: &str) -> AppResult<()> {
    if Path::new(dest).exists() {
        std::fs::remove_file(dest)?;
    }
    conn.execute("VACUUM INTO ?1", [dest])?;
    Ok(())
}

/// Return Ok(()) if `src` is a valid DeskHub backup: an SQLite database
/// containing the `kv` table. Otherwise an error suitable for the user.
#[allow(dead_code)]
pub fn validate(src: &Path) -> AppResult<()> {
    let conn = Connection::open(src)?;
    let has_kv: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='kv'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !has_kv {
        return Err(AppError::Other(
            "不是有效的 DeskHub 备份 / Not a valid DeskHub backup".into(),
        ));
    }
    Ok(())
}

/// Validate `src` then copy it to the staging path `deskhub.db.import` under
/// `dir`. The staged file is applied on next startup (see `apply_pending_import`).
/// We do not overwrite the live DB directly because Windows locks the in-use file.
#[allow(dead_code)]
pub fn stage_import(dir: &Path, src: &Path) -> AppResult<()> {
    validate(src)?;
    let staging = dir.join("deskhub.db.import");
    std::fs::copy(src, &staging)?;
    Ok(())
}

/// If a staged import exists in `dir`, replace the live DB with it.
/// Must be called BEFORE opening the connection. Deletes the old db + WAL/SHM
/// sidecars, then renames the staged file into place.
#[allow(dead_code)]
pub fn apply_pending_import(dir: &Path) -> AppResult<()> {
    let staging = dir.join("deskhub.db.import");
    if !staging.exists() {
        return Ok(());
    }
    for ext in ["", "-wal", "-shm"] {
        let p = dir.join(format!("deskhub.db{ext}"));
        let _ = std::fs::remove_file(&p);
    }
    std::fs::rename(&staging, dir.join("deskhub.db"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::Connection;

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "deskhub_backup_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Build a real DeskHub db file at `path` with kv key `marker`=value.
    fn make_db(path: &Path, marker: &str) {
        let mut conn = Connection::open(path).unwrap();
        migrations::apply(&mut conn).unwrap();
        crate::db::kv::set(&conn, "marker", marker).unwrap();
    }

    #[test]
    fn export_produces_reopenable_db() {
        let dir = temp_dir();
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::apply(&mut conn).unwrap();
        crate::db::kv::set(&conn, "marker", "hello").unwrap();

        let dest = dir.join("out.db");
        export(&conn, dest.to_str().unwrap()).unwrap();
        assert!(dest.exists());

        let copy = Connection::open(&dest).unwrap();
        let v: String = copy
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_accepts_real_db_rejects_garbage() {
        let dir = temp_dir();
        let good = dir.join("good.db");
        make_db(&good, "x");
        assert!(validate(&good).is_ok());

        let bad = dir.join("bad.db");
        std::fs::write(&bad, b"not a database").unwrap();
        assert!(validate(&bad).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_pending_import_replaces_live_db() {
        let dir = temp_dir();
        // live db has marker "old"; staged import has marker "new"
        make_db(&dir.join("deskhub.db"), "old");
        make_db(&dir.join("deskhub.db.import"), "new");

        apply_pending_import(&dir).unwrap();

        assert!(!dir.join("deskhub.db.import").exists());
        let conn = Connection::open(dir.join("deskhub.db")).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "new");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_pending_import_noop_without_staging() {
        let dir = temp_dir();
        make_db(&dir.join("deskhub.db"), "keep");
        apply_pending_import(&dir).unwrap(); // no staging file -> no-op
        let conn = Connection::open(dir.join("deskhub.db")).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM kv WHERE key='marker'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "keep");
        std::fs::remove_dir_all(&dir).ok();
    }
}

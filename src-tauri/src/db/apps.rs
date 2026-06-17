use crate::error::AppResult;
use crate::models::AppEntry;
use crate::system::shortcuts::ShortcutRaw;
use rusqlite::Connection;
use std::collections::HashMap;

type AppPrefs = (Option<String>, bool, i64);

/// (name, target, args)
pub fn list_custom(conn: &Connection) -> AppResult<Vec<(String, String, Option<String>)>> {
    let mut stmt = conn.prepare("SELECT name, target, args FROM custom_apps ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn add_custom(conn: &Connection, name: &str, target: &str, args: Option<&str>) -> AppResult<()> {
    // de-dup by lowercased target
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
    conn.execute(
        "INSERT INTO custom_apps (name, target, args) VALUES (?1, ?2, ?3)",
        (name, target, args),
    )?;
    Ok(())
}

pub fn remove_custom(conn: &Connection, target: &str) -> AppResult<()> {
    conn.execute("DELETE FROM custom_apps WHERE lower(target) = lower(?1)", [target])?;
    Ok(())
}

/// target(lowercased) -> (category, favorite, sort_order)
pub fn prefs_map(conn: &Connection) -> AppResult<HashMap<String, AppPrefs>> {
    let mut stmt = conn.prepare("SELECT target, category, favorite, sort_order FROM app_prefs")?;
    let rows = stmt.query_map([], |r| {
        let target: String = r.get(0)?;
        let category: Option<String> = r.get(1)?;
        let favorite: i64 = r.get(2)?;
        let sort_order: i64 = r.get(3)?;
        Ok((target.to_lowercase(), (category, favorite != 0, sort_order)))
    })?;
    let mut map = HashMap::new();
    for r in rows {
        let (k, v) = r?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn set_favorite(conn: &Connection, target: &str, favorite: bool) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_prefs (target, favorite, updated_at)
         VALUES (lower(?1), ?2, datetime('now'))
         ON CONFLICT(target) DO UPDATE SET favorite = excluded.favorite, updated_at = datetime('now')",
        (target, favorite as i64),
    )?;
    Ok(())
}

pub fn set_category(conn: &Connection, target: &str, category: Option<&str>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_prefs (target, category, updated_at)
         VALUES (lower(?1), ?2, datetime('now'))
         ON CONFLICT(target) DO UPDATE SET category = excluded.category, updated_at = datetime('now')",
        (target, category),
    )?;
    Ok(())
}

/// Pure merge: scanned shortcuts ∪ custom apps, de-duped by lowercased target,
/// overlaid with prefs, sorted by favorite desc, sort_order asc, name asc.
pub fn merge(
    scanned: Vec<ShortcutRaw>,
    custom: Vec<(String, String, Option<String>)>,
    prefs: &HashMap<String, (Option<String>, bool, i64)>,
) -> Vec<AppEntry> {
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut entries: Vec<AppEntry> = Vec::new();

    for s in scanned {
        let key = s.target.to_lowercase();
        if seen.insert(key.clone(), ()).is_some() {
            continue;
        }
        let (category, favorite, _) = prefs.get(&key).cloned().unwrap_or((None, false, 0));
        entries.push(AppEntry {
            name: s.name,
            launch_path: s.lnk_path,
            target: s.target,
            args: s.args,
            is_custom: false,
            category,
            favorite,
        });
    }
    for (name, target, args) in custom {
        let key = target.to_lowercase();
        if seen.insert(key.clone(), ()).is_some() {
            continue;
        }
        let (category, favorite, _) = prefs.get(&key).cloned().unwrap_or((None, false, 0));
        entries.push(AppEntry {
            name,
            launch_path: target.clone(),
            target,
            args,
            is_custom: true,
            category,
            favorite,
        });
    }

    entries.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then_with(|| {
                let sa = prefs.get(&a.target.to_lowercase()).map(|p| p.2).unwrap_or(0);
                let sb = prefs.get(&b.target.to_lowercase()).map(|p| p.2).unwrap_or(0);
                sa.cmp(&sb)
            })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
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

    fn raw(name: &str, target: &str) -> ShortcutRaw {
        ShortcutRaw {
            name: name.to_string(),
            lnk_path: format!("C:\\Desktop\\{name}.lnk"),
            target: target.to_string(),
            args: None,
        }
    }

    #[test]
    fn add_custom_dedups_by_target() {
        let conn = setup();
        add_custom(&conn, "App", "C:\\a\\app.exe", None).unwrap();
        add_custom(&conn, "App2", "C:\\A\\APP.EXE", None).unwrap(); // same target, diff case
        assert_eq!(list_custom(&conn).unwrap().len(), 1);
    }

    #[test]
    fn favorite_and_category_upsert() {
        let conn = setup();
        set_favorite(&conn, "C:\\a\\app.exe", true).unwrap();
        set_category(&conn, "C:\\A\\app.exe", Some("Work")).unwrap();
        let m = prefs_map(&conn).unwrap();
        let p = m.get("c:\\a\\app.exe").unwrap();
        assert_eq!(p.0.as_deref(), Some("Work"));
        assert!(p.1);
    }

    #[test]
    fn merge_dedups_and_sorts_favorites_first() {
        let scanned = vec![raw("Zeta", "C:\\z.exe"), raw("Alpha", "C:\\a.exe")];
        let custom = vec![
            ("AlphaDup".to_string(), "C:\\A.EXE".to_string(), None), // dup of Alpha by target
            ("Beta".to_string(), "C:\\b.exe".to_string(), None),
        ];
        let mut prefs = HashMap::new();
        prefs.insert("c:\\b.exe".to_string(), (Some("Fav".to_string()), true, 0));
        let merged = merge(scanned, custom, &prefs);
        // dup dropped: Zeta, Alpha, Beta = 3
        assert_eq!(merged.len(), 3);
        // Beta is favorite -> first
        assert_eq!(merged[0].name, "Beta");
        assert!(merged[0].favorite);
        // remaining alphabetical: Alpha, Zeta
        assert_eq!(merged[1].name, "Alpha");
        assert_eq!(merged[2].name, "Zeta");
        // Alpha kept scanned (not custom)
        assert!(!merged[1].is_custom);
    }
}

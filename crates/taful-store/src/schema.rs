//! Forward-only, numbered migrations embedded in the binary (spec §14.1).
//!
//! The schema version is SQLite's `PRAGMA user_version`. Each migration runs
//! in its own transaction together with the version bump, so a failed
//! migration leaves the file at the previous version. A migration is never
//! edited once released: changes go in a new one.

use rusqlite::Connection;

use crate::Result;

/// Migration `i` takes the schema from version `i` to `i + 1`.
pub const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_initial.sql"),
    include_str!("migrations/0002_hand_review.sql"),
    include_str!("migrations/0003_opponent_profiles.sql"),
];

/// The schema version this binary writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub(crate) fn version(conn: &Connection) -> Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// Applies every migration above the current version.
pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    migrate_to(conn, SCHEMA_VERSION)
}

pub(crate) fn migrate_to(conn: &mut Connection, target: u32) -> Result<()> {
    let from = version(conn)?;
    for (i, sql) in MIGRATIONS
        .iter()
        .enumerate()
        .take(target as usize)
        .skip(from as usize)
    {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Whether a database has the tables of the first migration (guards imports
/// of unrelated SQLite files that happen to have a `user_version`).
pub(crate) fn looks_like_taful(conn: &Connection) -> Result<bool> {
    let n: u32 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' \
         AND name IN ('settings', 'review_items', 'review_log')",
        [],
        |r| r.get(0),
    )?;
    Ok(n == 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every table follows the D9 rules: ULID text primary key, timestamps,
    /// nullable soft-delete column.
    #[test]
    fn schema_rules() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' \
                 AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(tables.len() >= 11, "{tables:?}");
        // User layouts and themes (spec §15.3, §15.4) are user data under the
        // same rules, so backups and future sync carry them.
        for t in ["layouts", "themes"] {
            assert!(tables.iter().any(|n| n == t), "{t} missing: {tables:?}");
        }
        let mut problems = Vec::new();
        for t in &tables {
            // (name, type, notnull, pk)
            let cols: Vec<(String, String, bool, bool)> = conn
                .prepare(&format!("PRAGMA table_info({t})"))
                .unwrap()
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, bool>(3)?,
                        r.get::<_, i32>(5)? > 0,
                    ))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let col = |n: &str| cols.iter().find(|c| c.0 == n);
            match col("id") {
                Some((_, ty, _, true)) if ty == "TEXT" => {}
                _ => problems.push(format!("{t}: `id TEXT PRIMARY KEY` missing")),
            }
            let pks = cols.iter().filter(|c| c.3).count();
            if pks != 1 {
                problems.push(format!("{t}: primary key must be `id` alone"));
            }
            for ts in ["created_at", "updated_at"] {
                match col(ts) {
                    Some((_, ty, true, _)) if ty == "INTEGER" => {}
                    _ => problems.push(format!("{t}: `{ts} INTEGER NOT NULL` missing")),
                }
            }
            match col("deleted_at") {
                Some((_, ty, false, _)) if ty == "INTEGER" => {}
                _ => problems.push(format!("{t}: nullable `deleted_at INTEGER` missing")),
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// Migrating from every past version reaches the same schema as a fresh
    /// install.
    #[test]
    fn migrates_from_every_past_version() {
        let dump = |conn: &Connection| -> Vec<String> {
            conn.prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let mut fresh = Connection::open_in_memory().unwrap();
        migrate(&mut fresh).unwrap();
        assert_eq!(version(&fresh).unwrap(), SCHEMA_VERSION);
        for past in 0..SCHEMA_VERSION {
            let mut conn = Connection::open_in_memory().unwrap();
            migrate_to(&mut conn, past).unwrap();
            assert_eq!(version(&conn).unwrap(), past);
            migrate(&mut conn).unwrap();
            assert_eq!(dump(&conn), dump(&fresh), "from v{past}");
        }
        // Idempotent.
        migrate(&mut fresh).unwrap();
        assert_eq!(version(&fresh).unwrap(), SCHEMA_VERSION);
    }

    /// 0003 (#26): v1 spot-template keys become v2 reference keys, in review
    /// items and in decisions (template and node keys), and old decisions
    /// are against the reference. Other rows are left alone.
    #[test]
    fn opponent_migration_keys_old_rows_as_reference() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_to(&mut conn, 2).unwrap();
        conn.execute_batch(
            "INSERT INTO review_items (id, created_at, updated_at, kind, item_key, due, reps,
                 lapses)
             VALUES ('a', 1, 1, 'spot_template', 'v1|k|preflop|UTG:f|-|HJ', 0, 0, 0),
                    ('b', 1, 1, 'spot_template', 'v1|k|btn-bb-srp|BB:x/|A-r-u-0|BB', 0, 0, 0),
                    ('c', 1, 1, 'calc_exercise', 'v1|odd', 0, 0, 0),
                    ('d', 1, 1, 'spot_template', 'srp/x', 0, 0, 0);
             INSERT INTO decisions (id, created_at, updated_at, node_key, hero_combo,
                 chosen_action, ev_per_action, frequencies, pot, ev_loss, grade,
                 thresholds_version, template_key, spot, street, node_type)
             VALUES ('x', 1, 1, 'v1|k|preflop|UTG:f|-|HJ#12', 'AsKs', 'f', '[]', '[]', 1.5,
                     0.4, 'mistake', 1, 'v1|k|preflop|UTG:f|-|HJ', 'preflop', 'preflop',
                     'rfi'),
                    ('y', 1, 1, 'n', 'AsKs', 'f', '[]', '[]', 1.5, 0.0, 'best', 1, NULL,
                     NULL, NULL, NULL);",
        )
        .unwrap();
        migrate(&mut conn).unwrap();
        let key = |id: &str| -> (String, i64) {
            conn.query_row(
                "SELECT item_key, updated_at FROM review_items WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
        };
        assert_eq!(key("a").0, "v2|k|preflop|UTG:f|-|HJ|reference");
        assert!(key("a").1 > 1, "rewritten rows are touched for sync");
        assert_eq!(key("b").0, "v2|k|btn-bb-srp|BB:x/|A-r-u-0|BB|reference");
        assert_eq!(key("c"), ("v1|odd".into(), 1));
        assert_eq!(key("d"), ("srp/x".into(), 1));
        let decision = |id: &str| -> (String, Option<String>, Option<String>) {
            conn.query_row(
                "SELECT node_key, template_key, opponent FROM decisions WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };
        assert_eq!(
            decision("x"),
            (
                "v2|k|preflop|UTG:f|-|HJ|reference#12".into(),
                Some("v2|k|preflop|UTG:f|-|HJ|reference".into()),
                Some("reference".into())
            )
        );
        assert_eq!(decision("y"), ("n".into(), None, None));
    }
}

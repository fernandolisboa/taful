//! Snapshots and rotation (spec §14.3).
//!
//! A snapshot is a consistent copy made with SQLite's online backup API,
//! named `taful-YYYYMMDD-HHMMSS.sqlite` in local time. Rotation keeps the
//! newest snapshot of each of the last 7 days that have one, plus the newest
//! of each of the last 4 ISO weeks that have one (the union, like restic's
//! `--keep-daily 7 --keep-weekly 4`). Files that do not match the name
//! pattern are never touched.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use rusqlite::Connection;

use crate::{Millis, Result, Store};

const PREFIX: &str = "taful-";
const SUFFIX: &str = ".sqlite";
const STAMP: &str = "%Y%m%d-%H%M%S";

/// Daily snapshots kept.
pub const KEEP_DAILY: usize = 7;
/// Weekly snapshots kept.
pub const KEEP_WEEKLY: usize = 4;

/// `YYYYMMDD-HHMMSS` for `ms`, in the system time zone.
pub(crate) fn stamp(ms: Millis) -> String {
    local(ms).strftime(STAMP).to_string()
}

fn local(ms: Millis) -> DateTime {
    let ts = jiff::Timestamp::from_millisecond(ms).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    ts.to_zoned(TimeZone::system()).datetime()
}

/// `taful-YYYYMMDD-HHMMSS.sqlite` for a snapshot taken at `ms`.
pub fn backup_file_name(ms: Millis) -> String {
    format!("{PREFIX}{}{SUFFIX}", stamp(ms))
}

/// The local time a snapshot name encodes, or `None` for any other file.
pub fn parse_backup_name(name: &str) -> Option<DateTime> {
    let s = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    DateTime::strptime(STAMP, s).ok()
}

pub(crate) fn copy_to(conn: &Connection, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    conn.backup(
        rusqlite::MAIN_DB,
        dest,
        None::<fn(rusqlite::backup::Progress)>,
    )?;
    Ok(())
}

/// Writes a snapshot of `store` into `dir` and rotates the folder. Returns
/// the new file's path.
pub fn snapshot(store: &Store, dir: &Path) -> Result<PathBuf> {
    // A second snapshot in the same second replaces the first: it is newer
    // and just as consistent.
    let dest = dir.join(backup_file_name(store.now()));
    store.backup_to(&dest)?;
    rotate(dir)?;
    Ok(dest)
}

/// Deletes snapshots in `dir` outside the retention window. Returns the
/// removed paths.
pub fn rotate(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut snaps: Vec<(DateTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(dt) = parse_backup_name(name) {
            snaps.push((dt, entry.path()));
        }
    }
    let keep = retained(&snaps);
    let mut removed = Vec::new();
    for (i, (_, path)) in snaps.iter().enumerate() {
        if !keep.contains(&i) {
            std::fs::remove_file(path)?;
            removed.push(path.clone());
        }
    }
    Ok(removed)
}

/// Indices of `snaps` to keep.
fn retained(snaps: &[(DateTime, PathBuf)]) -> BTreeSet<usize> {
    let mut order: Vec<usize> = (0..snaps.len()).collect();
    // Newest first; ties (impossible with one name per second, but cheap to
    // pin) broken by path so the result is deterministic.
    order.sort_by(|&a, &b| {
        snaps[b]
            .0
            .cmp(&snaps[a].0)
            .then_with(|| snaps[b].1.cmp(&snaps[a].1))
    });
    let mut keep = BTreeSet::new();
    let mut days = BTreeSet::new();
    let mut weeks = BTreeSet::new();
    for i in order {
        let dt = snaps[i].0;
        if days.len() < KEEP_DAILY && days.insert(dt.date()) {
            keep.insert(i);
        }
        let week = dt.date().iso_week_date();
        if weeks.len() < KEEP_WEEKLY && weeks.insert((week.year(), week.week())) {
            keep.insert(i);
        }
    }
    keep
}

/// When to take automatic snapshots (spec §14.3): after every 24 h of app
/// use, and on exit unless one was taken in the last hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupPolicy {
    pub use_interval_ms: Millis,
    pub exit_min_gap_ms: Millis,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        BackupPolicy {
            use_interval_ms: 24 * 3_600_000,
            exit_min_gap_ms: 3_600_000,
        }
    }
}

impl BackupPolicy {
    /// Whether accumulated use since the last snapshot calls for one.
    pub fn due_by_use(&self, use_ms_since_last: Millis) -> bool {
        use_ms_since_last >= self.use_interval_ms
    }

    /// Whether to snapshot on exit.
    pub fn due_on_exit(&self, last: Option<Millis>, now: Millis) -> bool {
        last.is_none_or(|t| now - t >= self.exit_min_gap_ms)
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::test_clock;

    #[test]
    fn names_round_trip() {
        let name = backup_file_name(1_762_340_000_000);
        assert!(name.starts_with("taful-") && name.ends_with(".sqlite"));
        assert_eq!(name.len(), "taful-YYYYMMDD-HHMMSS.sqlite".len());
        assert!(parse_backup_name(&name).is_some());
        assert_eq!(
            parse_backup_name("taful-20261105-093000.sqlite"),
            Some(date(2026, 11, 5).at(9, 30, 0, 0))
        );
        assert_eq!(parse_backup_name("taful.sqlite"), None);
        assert_eq!(parse_backup_name("notes.txt"), None);
    }

    #[test]
    fn retention_keeps_7_daily_and_4_weekly() {
        // Two snapshots a day for 60 days ending Thu 2026-11-05.
        let end = date(2026, 11, 5);
        let mut snaps = Vec::new();
        for d in 0..60 {
            let day = end.checked_sub(jiff::Span::new().days(d)).unwrap();
            for h in [9, 21] {
                let dt = day.at(h, 0, 0, 0);
                snaps.push((dt, PathBuf::from(format!("{dt}"))));
            }
        }
        let keep = retained(&snaps);
        let kept: Vec<DateTime> = keep.iter().map(|&i| snaps[i].0).collect();
        // 7 daily (Oct 30 – Nov 5, evening ones) plus the newest of the 4
        // most recent ISO weeks; the current and previous week's newest are
        // already among the dailies.
        let daily: BTreeSet<_> = kept.iter().map(|d| d.date()).collect();
        assert!(kept.iter().all(|d| d.hour() == 21));
        assert!((7..=9).contains(&kept.len()), "{kept:?}");
        for d in 0..7 {
            assert!(daily.contains(&end.checked_sub(jiff::Span::new().days(d)).unwrap()));
        }
        let weeks: BTreeSet<_> = kept
            .iter()
            .map(|d| d.date().iso_week_date().week())
            .collect();
        assert_eq!(weeks.len(), 4);
    }

    #[test]
    fn snapshot_and_rotate_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let (clock, t) = test_clock(1_762_340_000_000);
        let store = Store::open_in_memory(clock).unwrap();
        store.set_setting("x", &serde_json::json!(1)).unwrap();
        std::fs::write(dir.path().join("keep-me.txt"), "x").unwrap();
        // 20 days of snapshots, one a day.
        for _ in 0..20 {
            snapshot(&store, dir.path()).unwrap();
            t.fetch_add(86_400_000, std::sync::atomic::Ordering::SeqCst);
        }
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.contains(&"keep-me.txt".to_owned()));
        let snaps = names
            .iter()
            .filter(|n| parse_backup_name(n).is_some())
            .count();
        assert!((7..=10).contains(&snaps), "{names:?}");
        // A second snapshot in the same second replaces the first.
        let a = snapshot(&store, dir.path()).unwrap();
        let b = snapshot(&store, dir.path()).unwrap();
        assert_eq!(a, b);
        assert!(b.exists());
    }

    #[test]
    fn backup_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let (clock, _) = test_clock(1_000);
        let a = Store::open(&dir.path().join("a.sqlite"), clock.clone()).unwrap();
        a.set_setting("locale", &serde_json::json!("pt-BR"))
            .unwrap();
        a.ensure_review_item("calc_exercise", "k", None, 5).unwrap();
        let snap = dir.path().join("snap.sqlite");
        a.backup_to(&snap).unwrap();

        let mut b = Store::open(&dir.path().join("b.sqlite"), clock).unwrap();
        b.set_setting("locale", &serde_json::json!("en-US"))
            .unwrap();
        b.import_from(&snap).unwrap();
        assert_eq!(
            b.setting("locale").unwrap(),
            Some(serde_json::json!("pt-BR"))
        );
        assert!(b.review_item("calc_exercise", "k").unwrap().is_some());

        // A non-Taful SQLite file is refused and leaves the store untouched.
        let other = dir.path().join("other.sqlite");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x); PRAGMA user_version = 1;")
            .unwrap();
        assert!(matches!(
            b.import_from(&other),
            Err(crate::StoreError::NotATafulDatabase(_))
        ));
        assert_eq!(
            b.setting("locale").unwrap(),
            Some(serde_json::json!("pt-BR"))
        );

        // A newer schema is refused.
        let newer = dir.path().join("newer.sqlite");
        a.backup_to(&newer).unwrap();
        Connection::open(&newer)
            .unwrap()
            .pragma_update(None, "user_version", crate::SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            b.import_from(&newer),
            Err(crate::StoreError::SchemaTooNew { .. })
        ));
    }

    #[test]
    fn opening_refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let (clock, _) = test_clock(1_000);
        drop(Store::open(&path, clock.clone()).unwrap());
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", crate::SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            Store::open(&path, clock),
            Err(crate::StoreError::SchemaTooNew { .. })
        ));
    }

    #[test]
    fn policy() {
        let p = BackupPolicy::default();
        assert!(!p.due_by_use(23 * 3_600_000));
        assert!(p.due_by_use(24 * 3_600_000));
        assert!(p.due_on_exit(None, 0));
        assert!(!p.due_on_exit(Some(0), 59 * 60_000));
        assert!(p.due_on_exit(Some(0), 60 * 60_000));
    }
}

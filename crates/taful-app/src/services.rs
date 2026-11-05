//! [`App`]: the store, the embedded content and the tunables, plus settings
//! and backups (spec §14.3).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use taful_content::Library;
use taful_core::calc::CalcSettings;
use taful_core::grading::Thresholds;
use taful_store::{BackupPolicy, Clock, Millis, Store};

use crate::i18n::Locale;
use crate::review::{DEFAULT_RETENTION, Scheduler};
use crate::{Result, content};

/// Settings keys (`settings` table, JSON values).
pub mod keys {
    pub const LOCALE: &str = "locale";
    pub const RETENTION: &str = "srs.desired_retention";
    pub const BACKUP_DIR: &str = "backup.dir";
    pub const BACKUP_LAST_AT: &str = "backup.last_at";
    pub const BACKUP_USE_MS: &str = "backup.use_ms";
    pub const LAYOUT: &str = "layout";
    pub const THEME: &str = "theme";
    pub const LIBRARY_BASE_URL: &str = "library.base_url";
}

/// The database file name inside the data folder.
pub const DB_FILE: &str = "taful.sqlite";

/// Where Taful keeps its database when nothing else is configured:
/// `TAFUL_DATA_DIR` if set, else `%APPDATA%\Taful` on Windows, else
/// `$XDG_DATA_HOME/taful` or `~/.local/share/taful`.
pub fn default_data_dir() -> PathBuf {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env("TAFUL_DATA_DIR") {
        return d;
    }
    if cfg!(windows)
        && let Some(d) = env("APPDATA")
    {
        return d.join(crate::PRODUCT_NAME);
    }
    if let Some(d) = env("XDG_DATA_HOME") {
        return d.join("taful");
    }
    if let Some(h) = env("HOME") {
        return h.join(".local").join("share").join("taful");
    }
    PathBuf::from("taful-data")
}

/// Suggested file name for a manual export taken at `ms`.
pub fn export_file_name(ms: Millis) -> String {
    taful_store::backup_file_name(ms).replacen("taful-", "taful-export-", 1)
}

pub use crate::review::RETENTION_RANGE;

/// The application services.
pub struct App {
    pub(crate) store: Store,
    pub(crate) library: Library,
    pub(crate) calc: CalcSettings,
    pub(crate) thresholds: Thresholds,
    pub(crate) scheduler: Scheduler,
    pub(crate) data_dir: Option<PathBuf>,
    /// Overrides the postflop library folder (tests, settings).
    pub(crate) library_dir: Option<PathBuf>,
    pub(crate) trainers: Option<std::sync::Arc<crate::training::Trainers>>,
    policy: BackupPolicy,
    seed: u64,
}

/// What the home screen shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Home {
    pub due_now: u32,
    pub next_due: Option<Millis>,
    /// First available lesson not done yet, in tree order: `(id, title)`.
    pub next_lesson: Option<(String, String)>,
    pub lessons_done: usize,
    pub lessons_total: usize,
    /// Calc answers right / answered, over the last 50.
    pub calc_recent: (u32, u32),
    pub review_items: u32,
    pub reviews_logged: u32,
}

impl App {
    /// Opens (or creates) the database in `data_dir` and loads the embedded
    /// content.
    pub fn open(data_dir: &Path) -> Result<App> {
        let store = Store::open(&data_dir.join(DB_FILE), taful_store::system_clock())?;
        Ok(App::with_store(store, Some(data_dir.to_owned())))
    }

    /// An app over an in-memory database with the given clock (tests).
    pub fn open_in_memory(clock: Clock) -> Result<App> {
        Ok(App::with_store(Store::open_in_memory(clock)?, None))
    }

    fn with_store(store: Store, data_dir: Option<PathBuf>) -> App {
        let (library, findings) = content::library();
        // Content is checked in CI; a finding here is a build problem, and
        // the rest of the content still works.
        debug_assert!(findings.is_empty(), "{findings:?}");
        let seed = store.now() as u64;
        let mut app = App {
            store,
            library,
            calc: CalcSettings::from_toml(include_str!("../../../config/calc.toml"))
                .expect("calc.toml is valid (tested in taful-core)"),
            thresholds: Thresholds::from_toml(include_str!("../../../config/grading.toml"))
                .expect("grading.toml is valid (tested in taful-core)"),
            // Replaced below once the retention setting can be read.
            scheduler: Scheduler::new(DEFAULT_RETENTION),
            data_dir,
            library_dir: None,
            trainers: None,
            policy: BackupPolicy::default(),
            seed,
        };
        app.scheduler = Scheduler::new(app.retention());
        app
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn calc_settings(&self) -> &CalcSettings {
        &self.calc
    }

    pub fn thresholds(&self) -> &Thresholds {
        &self.thresholds
    }

    pub fn now(&self) -> Millis {
        self.store.now()
    }

    /// A fresh seed for generators (no OS entropy: clock + counter, mixed).
    pub(crate) fn next_seed(&mut self) -> u64 {
        self.seed = self
            .seed
            .wrapping_add(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(self.store.now() as u64);
        let mut z = self.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    // ---- settings ---------------------------------------------------------------------

    fn setting_str(&self, key: &str) -> Option<String> {
        self.store
            .setting(key)
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    fn setting_f64(&self, key: &str) -> Option<f64> {
        self.store
            .setting(key)
            .ok()
            .flatten()
            .and_then(|v| v.as_f64())
    }

    /// The UI locale (pt-BR unless the user picked another).
    pub fn locale(&self) -> Locale {
        self.setting_str(keys::LOCALE)
            .and_then(|t| Locale::from_tag(&t))
            .unwrap_or(Locale::DEFAULT)
    }

    pub fn set_locale(&self, locale: Locale) -> Result<()> {
        Ok(self.store.set_setting(keys::LOCALE, &json!(locale.tag()))?)
    }

    /// FSRS desired retention (§13), default 0.9.
    pub fn retention(&self) -> f64 {
        self.setting_f64(keys::RETENTION)
            .unwrap_or(DEFAULT_RETENTION)
            .clamp(RETENTION_RANGE.0, RETENTION_RANGE.1)
    }

    pub fn set_retention(&mut self, r: f64) -> Result<f64> {
        let r = (r.clamp(RETENTION_RANGE.0, RETENTION_RANGE.1) * 100.0).round() / 100.0;
        self.store.set_setting(keys::RETENTION, &json!(r))?;
        self.scheduler = Scheduler::new(r);
        Ok(r)
    }

    /// Generic string setting (layout, theme, library URL): stored only.
    pub fn string_setting(&self, key: &str) -> Option<String> {
        self.setting_str(key)
    }

    pub fn set_string_setting(&self, key: &str, value: &str) -> Result<()> {
        Ok(self.store.set_setting(key, &json!(value))?)
    }

    // ---- backups (§14.3) ------------------------------------------------------------------

    /// The backup folder: the user's choice, else `<data dir>/backups`.
    pub fn backup_dir(&self) -> Option<PathBuf> {
        self.setting_str(keys::BACKUP_DIR)
            .map(PathBuf::from)
            .or_else(|| self.data_dir.as_ref().map(|d| d.join("backups")))
    }

    pub fn set_backup_dir(&self, dir: &Path) -> Result<()> {
        Ok(self
            .store
            .set_setting(keys::BACKUP_DIR, &json!(dir.to_string_lossy()))?)
    }

    pub fn last_backup(&self) -> Option<Millis> {
        self.store
            .setting(keys::BACKUP_LAST_AT)
            .ok()
            .flatten()
            .and_then(|v| v.as_i64())
    }

    /// Takes a snapshot now (and rotates the folder). `None` without a
    /// backup folder (in-memory app).
    pub fn backup_now(&self) -> Result<Option<PathBuf>> {
        let Some(dir) = self.backup_dir() else {
            return Ok(None);
        };
        std::fs::create_dir_all(&dir)?;
        let path = taful_store::snapshot(&self.store, &dir)?;
        self.store
            .set_setting(keys::BACKUP_LAST_AT, &json!(self.store.now()))?;
        self.store.set_setting(keys::BACKUP_USE_MS, &json!(0))?;
        Ok(Some(path))
    }

    /// Adds `ms` of app use; takes a snapshot once 24 h of use accumulate.
    pub fn note_use(&self, ms: Millis) -> Result<Option<PathBuf>> {
        let used = self
            .store
            .setting(keys::BACKUP_USE_MS)?
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            + ms.max(0);
        if self.policy.due_by_use(used) {
            return self.backup_now();
        }
        self.store
            .set_setting(keys::BACKUP_USE_MS, &Value::from(used))?;
        Ok(None)
    }

    /// Snapshot on exit, unless one was taken in the last hour.
    pub fn on_exit(&self) -> Result<Option<PathBuf>> {
        if self
            .policy
            .due_on_exit(self.last_backup(), self.store.now())
        {
            self.backup_now()
        } else {
            Ok(None)
        }
    }

    /// Writes a copy of the database to `dest` (manual export).
    pub fn export_to(&self, dest: &Path) -> Result<()> {
        Ok(self.store.backup_to(dest)?)
    }

    /// Replaces the database with `src` (manual import). A safety snapshot
    /// of the current data is taken first when a backup folder exists.
    pub fn import_from(&mut self, src: &Path) -> Result<()> {
        // The safety snapshot rotates the backup folder, which may hold
        // `src` itself: read from a private copy.
        let copy = std::env::temp_dir().join(format!(
            "taful-import-{}-{}.sqlite",
            std::process::id(),
            self.store.now()
        ));
        std::fs::copy(src, &copy)?;
        let result = self.backup_now().map(|_| ()).and_then(|()| {
            self.store.import_from(&copy)?;
            Ok(())
        });
        let _ = std::fs::remove_file(&copy);
        result?;
        self.scheduler = Scheduler::new(self.retention());
        Ok(())
    }

    // ---- home ---------------------------------------------------------------------------------

    pub fn home(&self, locale: Locale) -> Result<Home> {
        let now = self.store.now();
        let tree = self.skill_tree(locale)?;
        let cards: Vec<_> = tree.iter().flat_map(|t| &t.lessons).collect();
        let next_lesson = cards
            .iter()
            .find(|c| c.state == taful_store::LessonState::Available)
            .map(|c| (c.id.clone(), c.title.clone()));
        let recent = self.store.recent_calc_attempts(50)?;
        let (items, reviews) = self.store.review_totals()?;
        Ok(Home {
            due_now: self
                .store
                .count_due(now, crate::review_session::PRESENTABLE)?,
            next_due: self.store.next_due_after(now)?,
            next_lesson,
            lessons_done: cards
                .iter()
                .filter(|c| c.state == taful_store::LessonState::Done)
                .count(),
            lessons_total: cards.len(),
            calc_recent: (
                recent.iter().filter(|a| a.correct).count() as u32,
                recent.len() as u32,
            ),
            review_items: items,
            reviews_logged: reviews,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    pub fn app_at(start: Millis) -> (App, Arc<AtomicI64>) {
        let t = Arc::new(AtomicI64::new(start));
        let t2 = t.clone();
        let app = App::open_in_memory(Arc::new(move || t2.load(Ordering::SeqCst))).unwrap();
        (app, t)
    }

    #[test]
    fn settings() {
        let (mut app, _) = app_at(1_000);
        assert_eq!(app.locale(), Locale::PtBr);
        app.set_locale(Locale::EnUs).unwrap();
        assert_eq!(app.locale(), Locale::EnUs);
        assert_eq!(app.retention(), 0.9);
        assert_eq!(app.set_retention(0.5).unwrap(), RETENTION_RANGE.0);
        assert_eq!(app.retention(), RETENTION_RANGE.0);
        assert_eq!(app.backup_dir(), None);
    }

    #[test]
    fn backups_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path()).unwrap();
        assert!(app.on_exit().unwrap().is_some(), "first exit backs up");
        assert!(
            app.on_exit().unwrap().is_none(),
            "not again within the hour"
        );
        assert!(app.note_use(3_600_000).unwrap().is_none());
        assert!(app.note_use(23 * 3_600_000).unwrap().is_some());
        let snaps = std::fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .count();
        assert!(snaps >= 1);

        let export = dir.path().join("export.sqlite");
        app.set_locale(Locale::EnUs).unwrap();
        app.export_to(&export).unwrap();
        drop(app);

        let other = tempfile::tempdir().unwrap();
        let mut b = App::open(other.path()).unwrap();
        assert_eq!(b.locale(), Locale::PtBr);
        b.import_from(&export).unwrap();
        assert_eq!(b.locale(), Locale::EnUs);
    }

    #[test]
    fn seeds_differ() {
        let (mut app, _) = app_at(5);
        let a = app.next_seed();
        let b = app.next_seed();
        assert_ne!(a, b);
    }

    #[test]
    fn data_dir_default_is_named() {
        let d = default_data_dir();
        assert!(!d.as_os_str().is_empty());
    }
}

//! Taful persistence (spec §14, D9): one SQLite file, forward-only embedded
//! migrations, repositories and rotating backups. Not WASM; the web client
//! gets its own store.
//!
//! # Conventions
//!
//! - Every table has `id TEXT PRIMARY KEY` (a ULID), `created_at`,
//!   `updated_at` and a nullable `deleted_at` (soft delete). `schema_rules`
//!   in the tests fails the build if a table breaks this.
//! - Timestamps are Unix milliseconds (UTC) in `INTEGER` columns, read from
//!   the store's [`Clock`] so tests can pin time.
//! - Structured values (answers, params, settings) are JSON text.
//! - This crate knows nothing about the domain (it depends on no workspace
//!   crate): ratings, grades and exercise kinds arrive as their stable string
//!   ids.

mod backup;
mod repo;
mod schema;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags};

pub use backup::{BackupPolicy, backup_file_name, parse_backup_name, rotate, snapshot};
pub use repo::*;
pub use schema::{MIGRATIONS, SCHEMA_VERSION};

/// Unix time in milliseconds.
pub type Millis = i64;

/// Source of the current time, in Unix milliseconds.
pub type Clock = Arc<dyn Fn() -> Millis + Send + Sync>;

/// The system clock.
pub fn system_clock() -> Clock {
    Arc::new(|| jiff::Timestamp::now().as_millisecond())
}

/// Anything that can go wrong in the store.
#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    /// The file was written by a newer Taful.
    SchemaTooNew {
        found: u32,
        supported: u32,
    },
    /// A file to import is not a Taful database.
    NotATafulDatabase(PathBuf),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "sqlite: {e}"),
            StoreError::Io(e) => write!(f, "io: {e}"),
            StoreError::Json(e) => write!(f, "json: {e}"),
            StoreError::SchemaTooNew { found, supported } => write!(
                f,
                "database schema version {found} is newer than this app supports ({supported})"
            ),
            StoreError::NotATafulDatabase(p) => {
                write!(f, "{} is not a Taful database", p.display())
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Sqlite(e) => Some(e),
            StoreError::Io(e) => Some(e),
            StoreError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}
impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

/// The open database.
pub struct Store {
    conn: Connection,
    clock: Clock,
    path: Option<PathBuf>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Opens (creating if needed) the database at `path` and migrates it to
    /// [`SCHEMA_VERSION`]. If an existing database needs migrating, a copy is
    /// first written next to it as `<file>.pre-v<N>-<time>.sqlite`.
    pub fn open(path: &Path, clock: Clock) -> Result<Store> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let mut store = Store {
            conn,
            clock,
            path: Some(path.to_owned()),
        };
        store.init()?;
        Ok(store)
    }

    /// An in-memory database (tests, previews).
    pub fn open_in_memory(clock: Clock) -> Result<Store> {
        let mut store = Store {
            conn: Connection::open_in_memory()?,
            clock,
            path: None,
        };
        store.init()?;
        Ok(store)
    }

    fn init(&mut self) -> Result<()> {
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        let found = schema::version(&self.conn)?;
        if found > SCHEMA_VERSION {
            return Err(StoreError::SchemaTooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        if found > 0
            && found < SCHEMA_VERSION
            && let Some(path) = &self.path
        {
            let stamp = backup::stamp((self.clock)());
            let mut name = path.file_name().unwrap_or_default().to_os_string();
            name.push(format!(".pre-v{found}-{stamp}.sqlite"));
            backup::copy_to(&self.conn, &path.with_file_name(name))?;
        }
        schema::migrate(&mut self.conn)?;
        Ok(())
    }

    /// The database file, or `None` in memory.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Current time from the store's clock.
    pub fn now(&self) -> Millis {
        (self.clock)()
    }

    /// The schema version of the open database.
    pub fn schema_version(&self) -> Result<u32> {
        schema::version(&self.conn)
    }

    /// Writes a consistent snapshot of the open database to `dest` (SQLite
    /// backup API).
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        backup::copy_to(&self.conn, dest)
    }

    /// Replaces the open database's contents with the database at `src`, then
    /// migrates it. Fails without touching anything if `src` is not a Taful
    /// database or was written by a newer schema.
    pub fn import_from(&mut self, src: &Path) -> Result<()> {
        let check = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let found =
            schema::version(&check).map_err(|_| StoreError::NotATafulDatabase(src.into()))?;
        if found == 0 || !schema::looks_like_taful(&check)? {
            return Err(StoreError::NotATafulDatabase(src.into()));
        }
        if found > SCHEMA_VERSION {
            return Err(StoreError::SchemaTooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        drop(check);
        self.conn.restore(
            rusqlite::MAIN_DB,
            src,
            None::<fn(rusqlite::backup::Progress)>,
        )?;
        schema::migrate(&mut self.conn)?;
        Ok(())
    }

    /// Runs `f` atomically: its writes are kept only when it returns `Ok`.
    /// A savepoint, so it nests: store methods that are atomic themselves
    /// can run inside.
    pub fn atomic<T, E: From<StoreError>>(
        &self,
        f: impl FnOnce(&Store) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E> {
        self.conn
            .execute_batch("SAVEPOINT taful_atomic")
            .map_err(StoreError::from)?;
        match f(self) {
            Ok(v) => {
                self.conn
                    .execute_batch("RELEASE taful_atomic")
                    .map_err(StoreError::from)?;
                Ok(v)
            }
            Err(e) => {
                // Best effort: the original error matters more.
                let _ = self
                    .conn
                    .execute_batch("ROLLBACK TO taful_atomic; RELEASE taful_atomic");
                Err(e)
            }
        }
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

/// A new ULID string for a primary key.
pub(crate) fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

#[cfg(test)]
pub(crate) fn test_clock(start: Millis) -> (Clock, Arc<std::sync::atomic::AtomicI64>) {
    use std::sync::atomic::{AtomicI64, Ordering};
    let t = Arc::new(AtomicI64::new(start));
    let t2 = t.clone();
    (Arc::new(move || t2.load(Ordering::SeqCst)), t)
}

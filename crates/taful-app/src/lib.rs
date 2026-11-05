//! Taful application services (spec §3, §4).
//!
//! This is the only crate the UI talks to. It is UI-free and
//! async-runtime-agnostic (every call is synchronous and quick), so a web
//! (WASM) or Android client is UI-only work.
//!
//! - [`i18n`] and [`theme`]: localized strings and design tokens (M0).
//! - [`App`]: the services over the store and the embedded content (M2):
//!   settings, backups, the calc trainer, lessons and quizzes, and the FSRS
//!   review queue.
//! - [`preflop`]: the preflop spot mode and full hands over the solver library
//!   (M3).

pub mod calc_trainer;
pub mod content;
pub mod fmt;
pub mod i18n;
pub mod lessons;
pub mod model;
pub mod postflop;
pub mod preflop;
pub mod review;
pub mod review_session;
pub mod services;
pub mod theme;

pub use services::{App, Home, default_data_dir, keys};

/// The product name, in one place (D10). User-facing text uses the `-product`
/// Fluent term instead, which must match this constant (checked by a test).
pub const PRODUCT_NAME: &str = "Taful";

/// Anything an [`App`] call can fail with.
#[derive(Debug)]
pub enum Error {
    Store(taful_store::StoreError),
    Scheduler(String),
    /// A lesson, quiz or question id that does not exist.
    NotFound(String),
    /// Content failed to render (a bug: content is checked in CI).
    Content(Vec<String>),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Store(e) => write!(f, "{e}"),
            Error::Scheduler(e) => write!(f, "scheduler: {e}"),
            Error::NotFound(what) => write!(f, "not found: {what}"),
            Error::Content(errs) => write!(f, "content: {}", errs.join("; ")),
            Error::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<taful_store::StoreError> for Error {
    fn from(e: taful_store::StoreError) -> Self {
        Error::Store(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

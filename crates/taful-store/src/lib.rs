//! Taful persistence (spec §14): one SQLite file, forward-only embedded
//! migrations, repositories and rotating backups. Not WASM; the web client
//! gets its own store.
//!
//! M0 only reserves the crate; the schema lands in M2.

/// Schema version the binary expects. `0` means "no schema yet".
pub const SCHEMA_VERSION: u32 = 0;

//! Taful solver crate (spec §7.5, §7.7, §7.8).
//!
//! Library format (read + write), the in-app river solver, best response and
//! the `SubgameSolver` trait. WASM-safe and free of AGPL code: the offline
//! build links `postflop-solver` from `tools/library-build`, never from here
//! (D11, enforced by `tools/arch-check`).
//!
//! M3 adds the library format ([`format`]) and preflop charts ([`preflop`]);
//! the river solver and best response land in M4.

pub mod format;
pub mod postflop;
pub mod preflop;
pub mod river;

/// Version of the Taful library file format (spec §7.5).
pub const LIBRARY_FORMAT_VERSION: u16 = format::FORMAT_VERSION;

/// Version of the library's directory layout (`library/v{layout}/…`, spec
/// §7.6). Independent of the file format: a reader of format v2 still reads
/// v1 files, so the format bump in M4 does not move existing files.
pub const LIBRARY_LAYOUT_VERSION: u16 = 1;

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
pub mod preflop;

/// Version of the Taful library file format (spec §7.5).
pub const LIBRARY_FORMAT_VERSION: u16 = format::FORMAT_VERSION;

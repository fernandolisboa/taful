//! Taful solver crate (spec §7.5, §7.7, §7.8).
//!
//! Library format (read + write), the in-app river solver, best response and
//! the `SubgameSolver` trait. WASM-safe and free of AGPL code: the offline
//! build links `postflop-solver` from `tools/library-build`, never from here
//! (D11, enforced by `tools/arch-check`).
//!
//! M0 only reserves the crate; the library format lands in M3/M4.

/// Version of the Taful library file format (spec §7.5). `0` means "no
/// format yet"; the first real format is `1`.
pub const LIBRARY_FORMAT_VERSION: u32 = 0;

//! Taful application services (spec §3, §4).
//!
//! This is the only crate the UI talks to. It is UI-free and
//! async-runtime-agnostic, so a web (WASM) or Android client is UI-only work.
//!
//! M0 provides the two services the app shell needs: localized strings
//! ([`i18n`]) and design tokens ([`theme`]). Both are pure data, kept here
//! rather than in `apps/desktop` so every future client shares them.

pub mod i18n;
pub mod preflop;
pub mod theme;

/// The product name, in one place (D10). User-facing text uses the `-product`
/// Fluent term instead, which must match this constant (checked by a test).
pub const PRODUCT_NAME: &str = "Taful";

//! Taful domain model (spec §5).
//!
//! Cards, ranges, the hand evaluator trait, equity, `GameConfig`, the rules
//! engine, grading, mental-math generators/solvers and the FSRS rating
//! mapping live here. This crate has no IO and no UI, depends on nothing else
//! in the workspace, and must keep compiling for `wasm32-unknown-unknown`.

pub mod action;
pub mod cards;
pub mod config;
pub mod engine;
pub mod equity;
pub mod eval;
pub mod position;
pub mod range;

/// Version of the domain model, bumped when a stored or keyed representation
/// changes (library keys, `GameConfig::key`, grading thresholds).
pub const MODEL_VERSION: u32 = 1;

//! One module per first-slice screen (spec §15.1). Each adds an `impl Shell`
//! block with its view and its handlers; per-screen state lives in a small
//! struct owned by the shell.

pub mod calc;
pub mod home;
pub mod lesson;
pub mod review;
pub mod settings;
pub mod table;
pub mod tree;

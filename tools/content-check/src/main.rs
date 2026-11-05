//! `content-check [--bless] [ROOT]`: runs every taful-content check over the
//! content tree (default `content/`, with Fluent files from the sibling
//! `i18n/`) and exits non-zero on any finding (spec §12.3).
//!
//! `--bless` first rewrites the placeholder snapshots to the current
//! rendered values (review the diff before committing), then checks.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut bless = false;
    let mut root = None;
    for arg in std::env::args_os().skip(1) {
        if arg == "--bless" {
            bless = true;
        } else if root.is_none() {
            root = Some(PathBuf::from(arg));
        } else {
            eprintln!("usage: content-check [--bless] [ROOT]");
            return ExitCode::FAILURE;
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from("content"));
    if bless {
        match taful_content::bless(&root) {
            Ok(touched) => {
                for p in &touched {
                    println!("content-check: blessed {}", p.display());
                }
            }
            Err(e) => {
                eprintln!("content-check: {}: {e}", root.display());
                return ExitCode::FAILURE;
            }
        }
    }
    match taful_content::check(&root) {
        Ok(findings) if findings.is_empty() => {
            println!("content-check: {} ok", root.display());
            ExitCode::SUCCESS
        }
        Ok(findings) => {
            for f in &findings {
                eprintln!("{f}");
            }
            eprintln!("content-check: {} finding(s)", findings.len());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("content-check: {}: {e}", root.display());
            ExitCode::FAILURE
        }
    }
}

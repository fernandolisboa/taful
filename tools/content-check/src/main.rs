//! `content-check [ROOT]`: runs every taful-content check over the content
//! tree (default `content/`) and exits non-zero on any finding (spec §12.3).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("content"), PathBuf::from);
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

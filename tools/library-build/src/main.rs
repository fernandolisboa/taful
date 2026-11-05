//! Offline library build (spec §7.3). Runs on the author's PC, at low
//! priority and pausable. M3: the preflop solver; the postflop batch lands
//! in M4.
//!
//! ```text
//! taful-library-build preflop <manifest.toml> <out-dir>
//! taful-library-build summary <chart.tafl> [max-depth]
//! ```

mod preflop;

use std::path::Path;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: taful-library-build preflop <manifest.toml> <out-dir>\n\
                taful-library-build summary <chart.tafl> [max-depth]\n\
         (library format v{}, model v{})",
        taful_solver::LIBRARY_FORMAT_VERSION,
        taful_core::MODEL_VERSION,
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, manifest, out] if cmd == "preflop" => {
            match preflop::run(Path::new(manifest), Path::new(out)) {
                Ok(_) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        [cmd, chart, rest @ ..] if cmd == "summary" && rest.len() <= 1 => {
            let depth = rest.first().and_then(|d| d.parse().ok()).unwrap_or(3);
            match std::fs::read(chart)
                .map_err(|e| e.to_string())
                .and_then(|b| preflop::summary(&b, depth))
            {
                Ok(s) => {
                    print!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}

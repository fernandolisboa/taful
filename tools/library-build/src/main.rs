//! Offline library build (spec §7.3). Runs on the author's PC, at low
//! priority and pausable. M3: the preflop solver; M4: the postflop batch.
//!
//! ```text
//! taful-library-build preflop <manifest.toml> <out-dir>
//! taful-library-build preflop-mixed-bench <manifest.toml> <out-dir>
//! taful-library-build summary <chart.tafl> [max-depth]
//! taful-library-build flops <out-file>
//! taful-library-build postflop <manifest.toml> <out-dir> [--only a,b] [--flops N] [--flop-list F,F]
//!                     [--threads N] [--max-mem-gb X] [--limit N] [--estimate]
//! taful-library-build index <library-dir>
//! ```

mod flops;
mod index;
mod postflop;
mod preflop;

use std::path::Path;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: taful-library-build preflop <manifest.toml> <out-dir>\n\
                taful-library-build preflop-mixed-bench <manifest.toml> <out-dir>\n\
                taful-library-build summary <chart.tafl> [max-depth]\n\
                taful-library-build flops <out-file>\n\
                taful-library-build postflop <manifest.toml> <out-dir> [--only a,b] [--flops N] [--flop-list F,F]\n\
                                    [--threads N] [--max-mem-gb X] [--limit N] [--estimate]\n\
                taful-library-build index <library-dir>\n\
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
        [cmd, manifest, out] if cmd == "preflop-mixed-bench" => {
            report(preflop::bench_mixed(Path::new(manifest), Path::new(out)))
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
        [cmd, out] if cmd == "flops" => {
            let order = flops::order(flops::SEED);
            report(std::fs::write(out, flops::render(&order)).map_err(|e| e.to_string()))
        }
        [cmd, dir] if cmd == "index" => report(index::write(Path::new(dir)).map(|n| {
            eprintln!("manifest.json: {n} files");
        })),
        [cmd, manifest, out, rest @ ..] if cmd == "postflop" => match parse_options(rest) {
            Ok(opt) => report(postflop::run(Path::new(manifest), Path::new(out), &opt)),
            Err(e) => {
                eprintln!("error: {e}");
                usage()
            }
        },
        _ => usage(),
    }
}

fn report(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_options(args: &[String]) -> Result<postflop::Options, String> {
    let mut o = postflop::Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        let num = |v: &String| v.parse::<f64>().map_err(|e| format!("{a}: {e}"));
        match a.as_str() {
            "--only" => o.only = val()?.split(',').map(str::to_owned).collect(),
            "--flops" => o.flops = Some(num(val()?)? as usize),
            "--threads" => o.threads = Some(num(val()?)? as usize),
            "--max-mem-gb" => o.max_mem_gb = Some(num(val()?)?),
            "--limit" => o.limit = Some(num(val()?)? as usize),
            "--flop-list" => o.flop_list = val()?.split(',').map(str::to_owned).collect(),
            "--estimate" => o.estimate = true,
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(o)
}

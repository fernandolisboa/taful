//! Postflop batch build (spec §7.3): one `postflop-solver` solve per
//! (spot, flop), exported turn-truncated to Taful's format (spec §7.5, D11).
//!
//! Resumable: a finished file is skipped on rerun, and a file only appears
//! once complete (written to `.tmp`, then renamed). Between solves the run
//! waits while `<out>/PAUSE` exists, so the build can be paused without
//! losing a solve in progress; stopping the process loses at most the
//! current solve. Every solve appends a JSON line to `<out>/build-log.jsonl`.

pub mod export;
pub mod manifest;
pub mod profiles;
mod validate;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use postflop_solver::{
    ActionTree, BetSizeOptions, BoardState, CardConfig, NOT_DEALT, PostFlopGame, Range, TreeConfig,
    compute_exploitability, finalize, solve_step,
};
use taful_core::action::Line;
use taful_core::cards::Hand;
use taful_core::config::GameConfig;
use taful_core::iso::Flop;
use taful_solver::postflop::{SpotGeometry, geometry, line_range};
use taful_solver::preflop::PreflopChart;

use manifest::{Manifest, Spot, Tree};

pub const SOLVER_ID: &str = "postflop-solver 9d1509f";

/// Command-line options of `postflop`.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Spot ids to build (empty = all).
    pub only: Vec<String>,
    /// Override `flop_count`.
    pub flops: Option<usize>,
    /// Solve these flops instead of the order's prefix (canonicalized).
    pub flop_list: Vec<String>,
    /// Worker threads (default: all logical cores).
    pub threads: Option<usize>,
    /// Skip solves whose estimated memory exceeds this (GB). Skipped solves
    /// stay pending for a later run.
    pub max_mem_gb: Option<f64>,
    /// Stop after this many solves.
    pub limit: Option<usize>,
    /// Only print the memory estimate of each pending solve.
    pub estimate: bool,
}

/// Everything one solve needs.
pub struct Job<'a> {
    pub tree: &'a Tree,
    pub line: Line,
    pub geometry: SpotGeometry,
    /// Combo weights at the flop, OOP and IP.
    pub ranges: [Vec<f32>; 2],
    pub flop: Flop,
}

pub fn bet_sizes(pair: &[String; 2]) -> Result<BetSizeOptions, String> {
    BetSizeOptions::try_from((pair[0].as_str(), pair[1].as_str()))
}

/// Build a postflop-solver range from Taful combo weights.
pub fn solver_range(weights: &[f32]) -> Range {
    let mut r = Range::new();
    for h in Hand::all() {
        let w = weights[h.combo()];
        if w > 0.0 {
            let [a, b] = h.cards();
            r.set_weight_by_cards(a.index(), b.index(), w.min(1.0));
        }
    }
    r
}

/// Build the (unallocated) game for a job.
pub fn build_game(job: &Job) -> Result<PostFlopGame, String> {
    let t = job.tree;
    let pot = i32::try_from(job.geometry.pot).map_err(|e| e.to_string())?;
    let stack = i32::try_from(job.geometry.stack).map_err(|e| e.to_string())?;
    let tree = TreeConfig {
        initial_state: BoardState::Flop,
        starting_pot: pot,
        effective_stack: stack,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [bet_sizes(&t.flop)?, bet_sizes(&t.flop)?],
        turn_bet_sizes: [bet_sizes(&t.turn)?, bet_sizes(&t.turn)?],
        river_bet_sizes: [bet_sizes(&t.river)?, bet_sizes(&t.river)?],
        turn_donk_sizes: None,
        river_donk_sizes: None,
        add_allin_threshold: t.add_allin_threshold,
        force_allin_threshold: t.force_allin_threshold,
        merging_threshold: t.merging_threshold,
    };
    let cards = CardConfig {
        range: [solver_range(&job.ranges[0]), solver_range(&job.ranges[1])],
        flop: job.flop.cards().map(|c| c.index()),
        turn: NOT_DEALT,
        river: NOT_DEALT,
    };
    PostFlopGame::with_config(cards, ActionTree::new(tree)?)
}

/// Result of one solve.
#[derive(Debug, Clone, Copy)]
pub struct SolveReport {
    pub iterations: u32,
    /// Chips.
    pub exploitability: f32,
    pub secs: f64,
}

pub fn solve(game: &mut PostFlopGame, s: &manifest::Solve, pot: f32) -> SolveReport {
    let target = pot * s.target_exploitability_pct / 100.0;
    let t0 = Instant::now();
    let mut expl = f32::INFINITY;
    let mut it = 0;
    while it < s.max_iterations {
        solve_step(game, it);
        it += 1;
        if it % s.check_every == 0 || it == s.max_iterations {
            expl = compute_exploitability(game);
            if expl <= target {
                break;
            }
        }
    }
    finalize(game);
    SolveReport {
        iterations: it,
        exploitability: expl,
        secs: t0.elapsed().as_secs_f64(),
    }
}

/// `<out>/v{layout}/<config key>/<spot>/<flop>.tafl`
pub fn file_path(config: &GameConfig, spot: &str, flop: Flop) -> PathBuf {
    PathBuf::from(format!("v{}", taful_solver::LIBRARY_LAYOUT_VERSION))
        .join(config.key().to_string())
        .join(spot)
        .join(format!("{flop}.tafl"))
}

fn resolve(manifest_path: &Path, rel: &str) -> PathBuf {
    manifest_path.parent().unwrap_or(Path::new(".")).join(rel)
}

/// Best effort: lower this process's priority so the PC stays usable.
fn lower_priority() {
    let pid = std::process::id().to_string();
    let ok = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).PriorityClass = 'BelowNormal'"),
            ])
            .status()
    } else {
        std::process::Command::new("renice")
            .args(["-n", "10", "-p", &pid])
            .stdout(std::process::Stdio::null())
            .status()
    };
    match ok {
        Ok(s) if s.success() => eprintln!("priority: below normal"),
        _ => eprintln!("priority: could not lower (continuing)"),
    }
}

fn wait_while_paused(out: &Path) {
    let flag = out.join("PAUSE");
    let mut said = false;
    while flag.exists() {
        if !said {
            eprintln!("paused: delete {} to continue", flag.display());
            said = true;
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

pub fn run(manifest_path: &Path, out: &Path, opt: &Options) -> Result<(), String> {
    let text = std::fs::read_to_string(manifest_path).map_err(|e| e.to_string())?;
    let m = Manifest::parse(&text)?;
    let chart_bytes = std::fs::read(resolve(manifest_path, &m.preflop_chart))
        .map_err(|e| format!("preflop chart: {e}"))?;
    let chart = PreflopChart::decode(&chart_bytes).map_err(|e| e.to_string())?;
    let config = chart.config().clone();
    // The postflop solve below is rake-free; refuse to label rake-free
    // results with a raked config.
    if config.rake != taful_core::config::Rake::ZERO {
        return Err("postflop build does not model rake yet; the chart's config has rake".into());
    }
    let flops_text = std::fs::read_to_string(resolve(manifest_path, &m.flops))
        .map_err(|e| format!("flops: {e}"))?;
    let mut order = crate::flops::parse(&flops_text)?;
    if !opt.flop_list.is_empty() {
        order = opt
            .flop_list
            .iter()
            .map(|s| {
                s.parse::<Flop>()
                    .map(|f| f.canonical().0)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
    }
    let n_flops = if opt.flop_list.is_empty() {
        opt.flops.unwrap_or(m.flop_count).min(order.len())
    } else {
        order.len()
    };
    for id in &opt.only {
        if !m.spots.iter().any(|s| &s.id == id) {
            return Err(format!("--only: unknown spot {id}"));
        }
    }
    let spots: Vec<&Spot> = m
        .spots
        .iter()
        .filter(|s| opt.only.is_empty() || opt.only.contains(&s.id))
        .collect();

    // Ranges and geometry per spot, from the chart.
    let mut prepared = Vec::new();
    for s in &spots {
        let line = s.line()?;
        let g = geometry(&config, &line).map_err(|e| format!("spot {}: {e}", s.id))?;
        let oop = line_range(&chart, &line, g.oop).map_err(|e| format!("spot {}: {e}", s.id))?;
        let ip = line_range(&chart, &line, g.ip).map_err(|e| format!("spot {}: {e}", s.id))?;
        prepared.push((*s, line, g, [oop, ip]));
    }

    if let Some(t) = opt.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .map_err(|e| e.to_string())?;
    }
    if !opt.estimate {
        lower_priority();
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let profiles = m.load_profiles(manifest_path.parent().unwrap_or(Path::new(".")))?;
    let manifest_hash = m.hash(&profiles);
    if !profiles.is_empty() {
        let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        eprintln!("profiles: {} (best responses inline)", ids.join(", "));
    }
    let mut done = 0usize;
    let mut skipped_mem = 0usize;
    let total = n_flops * prepared.len();
    eprintln!(
        "postflop: {} spots × {n_flops} flops = {total} solves, {} threads",
        prepared.len(),
        rayon::current_num_threads()
    );

    // Flop-major: a partial run covers every spot on the first flops.
    for (fi, &flop) in order[..n_flops].iter().enumerate() {
        for (spot, line, g, ranges) in &prepared {
            let rel = file_path(&config, &spot.id, flop);
            let path = out.join(&rel);
            if path.exists() {
                continue;
            }
            if opt.limit.is_some_and(|l| done >= l) {
                eprintln!("limit reached ({done} solves)");
                return finish(out);
            }
            let job = Job {
                tree: &m.trees[&spot.tree],
                line: line.clone(),
                geometry: g.clone(),
                ranges: ranges.clone(),
                flop,
            };
            let mut game = build_game(&job)?;
            let (_, mem16) = game.memory_usage();
            let (mem32, _) = game.memory_usage();
            let mem = if m.solve.compression { mem16 } else { mem32 };
            let gb = mem as f64 / f64::from(1u32 << 30);
            if opt.estimate {
                println!("{:3} {:12} {flop}: est {gb:.1} GB", fi + 1, spot.id);
                continue;
            }
            if opt.max_mem_gb.is_some_and(|max| gb > max) {
                eprintln!(
                    "skip {} {flop}: est {gb:.1} GB > --max-mem-gb (left for a later run)",
                    spot.id
                );
                skipped_mem += 1;
                continue;
            }
            wait_while_paused(out);
            eprintln!(
                "[{}/{}] {} {flop}: est {gb:.1} GB, solving…",
                fi + 1,
                n_flops,
                spot.id
            );
            game.allocate_memory(m.solve.compression);
            let report = solve(&mut game, &m.solve, g.pot as f32);
            let t_export = Instant::now();
            let file = export::export(
                &mut game,
                &job,
                &config,
                &m,
                &report,
                manifest_hash,
                &profiles,
            )?;
            let bytes = file.encode();
            let export_secs = t_export.elapsed().as_secs_f64();
            drop(game);
            std::fs::create_dir_all(path.parent().expect("has parent"))
                .map_err(|e| e.to_string())?;
            let tmp = path.with_extension("tafl.tmp");
            std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
            let expl_pct = 100.0 * report.exploitability / g.pot as f32;
            eprintln!(
                "    done: {} iters, {:.0}s, exploitability {expl_pct:.2}% pot, {} nodes, {:.1} MB",
                report.iterations,
                report.secs,
                file.nodes.len(),
                bytes.len() as f64 / 1e6
            );
            let log = serde_json::json!({
                "spot": spot.id,
                "flop": flop.to_string(),
                "iterations": report.iterations,
                "exploitability_pct": expl_pct,
                "solve_secs": report.secs,
                "export_secs": export_secs,
                "est_mem_bytes": mem,
                "threads": rayon::current_num_threads(),
                "nodes": file.nodes.len(),
                "chance_nodes": file.chance.len(),
                "file_bytes": bytes.len(),
                "unix_time": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs()),
            });
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(out.join("build-log.jsonl"))
                .map_err(|e| e.to_string())?;
            writeln!(f, "{log}").map_err(|e| e.to_string())?;
            done += 1;
        }
    }
    if skipped_mem > 0 {
        eprintln!("{skipped_mem} solves skipped for memory; rerun without --max-mem-gb at night");
    }
    if opt.estimate {
        return Ok(());
    }
    finish(out)
}

fn finish(out: &Path) -> Result<(), String> {
    let n = crate::index::write(out)?;
    eprintln!("manifest.json: {n} files");
    Ok(())
}

//! `taful-profile-stats`: HUD-style stats of the bot profiles, and a search
//! that tunes `content/bots/*.toml` toward target stats (#31). Offline tool,
//! never shipped. Run from the workspace root:
//!
//! ```text
//! profile-stats preflop
//! profile-stats postflop --library <out dir> [--stored]
//! profile-stats check    --library <out dir>
//! profile-stats tune     [--library <out dir>] [--write]
//! ```
//!
//! `--stored` measures the profile strategies the build stored (the knobs
//! it was built with) instead of recomputing them from the current TOMLs.
//! `--library` is a postflop build output folder (`<dir>/v1/<key>/<spot>/
//! <flop>.tafl`). Without it, `tune` only tunes the preflop knobs. Other
//! options: `--bots <dir>` (default `content/bots`), `--targets <file>`
//! (default `tools/profile-stats/targets.toml`), `--only <id,…>`.

mod postflop;
mod preflop;
mod tune;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use taful_core::position::Position;
use taful_core::profile::Profile;
use taful_solver::preflop::PreflopChart;

use postflop::{FlopSums, FlopTree};
use tune::Targets;

const CHART_DIR: &str =
    "library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f";
const DEFAULT_BOTS: &str = "content/bots";
const DEFAULT_TARGETS: &str = "tools/profile-stats/targets.toml";
const FLOPS: &str = "library/flops.txt";

struct Opts {
    command: String,
    bots: PathBuf,
    targets: PathBuf,
    library: Option<PathBuf>,
    only: Option<Vec<String>>,
    write: bool,
    stored: bool,
}

fn parse_args() -> Result<Opts, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Opts {
        command: args.first().cloned().unwrap_or_default(),
        bots: DEFAULT_BOTS.into(),
        targets: DEFAULT_TARGETS.into(),
        library: None,
        only: None,
        write: false,
        stored: false,
    };
    let mut i = 1;
    while i < args.len() {
        let value = || {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} needs a value", args[i]))
        };
        match args[i].as_str() {
            "--bots" => o.bots = value()?.into(),
            "--targets" => o.targets = value()?.into(),
            "--library" => o.library = Some(value()?.into()),
            "--only" => o.only = Some(value()?.split(',').map(str::to_string).collect()),
            "--write" => {
                o.write = true;
                i += 1;
                continue;
            }
            "--stored" => {
                o.stored = true;
                i += 1;
                continue;
            }
            other => return Err(format!("unknown argument {other}")),
        }
        i += 2;
    }
    Ok(o)
}

/// Profiles by id, with the TOML text they came from.
fn load_profiles(dir: &Path) -> Result<Vec<(Profile, PathBuf, String)>, String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let s = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let prof = Profile::from_toml(&s).map_err(|e| format!("{}: {e}", p.display()))?;
            Ok((prof, p, s))
        })
        .collect()
}

fn chart(name: &str) -> Result<PreflopChart, String> {
    let path = Path::new(CHART_DIR).join(name);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    PreflopChart::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Orbit size per canonical flop, from `library/flops.txt`.
fn orbits() -> Result<BTreeMap<String, f64>, String> {
    let s = std::fs::read_to_string(FLOPS).map_err(|e| format!("{FLOPS}: {e}"))?;
    Ok(s.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let _order = f.next()?;
            let flop = f.next()?.to_string();
            let orbit: f64 = f.next()?.parse().ok()?;
            Some((flop, orbit))
        })
        .collect())
}

/// Every `<spot>/<flop>.tafl` under `<root>/v1/<key>/`.
fn load_library(root: &Path) -> Result<Vec<FlopTree>, String> {
    let orbits = orbits()?;
    let mut out = Vec::new();
    let v1 = root.join("v1");
    let read = |p: &Path| -> Result<Vec<PathBuf>, String> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(p)
            .map_err(|e| format!("{}: {e}", p.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        v.sort();
        Ok(v)
    };
    for key in read(&v1)?.into_iter().filter(|p| p.is_dir()) {
        for spot in read(&key)?.into_iter().filter(|p| p.is_dir()) {
            let spot_id = spot
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            for f in read(&spot)? {
                if f.extension().is_none_or(|x| x != "tafl") {
                    continue;
                }
                let flop = f
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let orbit = *orbits
                    .get(&flop)
                    .ok_or_else(|| format!("{flop} is not in {FLOPS}"))?;
                out.push(FlopTree::load(&f, &spot_id, &flop, orbit)?);
            }
        }
    }
    if out.is_empty() {
        return Err(format!("no postflop files under {}", v1.display()));
    }
    eprintln!("loaded {} postflop files", out.len());
    Ok(out)
}

/// One (file, seat) the bot plays: its weight and its flop range.
struct SeatPlan {
    tree: usize,
    pos: Position,
    weight: f64,
    range: Vec<f32>,
}

/// Where the bot plays and with which range, for one set of preflop knobs
/// (`None` = the reference). Seats the bot never reaches are left out.
fn plan(
    trees: &[FlopTree],
    chart: &PreflopChart,
    profile: Option<&Profile>,
    stored: Option<&str>,
) -> Result<Vec<SeatPlan>, String> {
    let mut spot_orbit: BTreeMap<String, f64> = BTreeMap::new();
    for t in trees {
        *spot_orbit.entry(t.spot.clone()).or_default() += t.orbit;
    }
    // How often the bot plays each spot's line from each seat.
    let mut line_w: BTreeMap<(String, Position), f64> = BTreeMap::new();
    let mut out = Vec::new();
    for (i, t) in trees.iter().enumerate() {
        for pos in t.players() {
            let key = (t.spot.clone(), pos);
            if !line_w.contains_key(&key) {
                let w = postflop::line_probability(chart, t.line(), pos, profile)?;
                line_w.insert(key.clone(), w);
            }
            let weight = line_w[&key] * t.orbit / spot_orbit[&t.spot];
            if weight <= 0.0 {
                continue;
            }
            let range = match stored {
                Some(id) => {
                    let sec =
                        t.sections.iter().find(|s| s.id == id).ok_or_else(|| {
                            format!("{} {}: no stored data for {id}", t.spot, t.flop)
                        })?;
                    postflop::stored_range(t, pos, sec)
                }
                None => postflop::bot_range(t, pos, profile, chart)?,
            };
            out.push(SeatPlan {
                tree: i,
                pos,
                weight,
                range,
            });
        }
    }
    Ok(out)
}

/// Flop stats of one bot over the library: the total and per spot.
fn flop_stats<'a>(
    trees: &'a [FlopTree],
    plan: &[SeatPlan],
    source: impl Fn(&'a FlopTree) -> postflop::Source<'a>,
) -> (FlopSums, BTreeMap<String, FlopSums>) {
    let mut total = FlopSums::default();
    let mut per_spot: BTreeMap<String, FlopSums> = BTreeMap::new();
    for sp in plan {
        let t = &trees[sp.tree];
        let s = postflop::measure(t, sp.pos, &sp.range, source(t));
        total.add(&s, sp.weight);
        per_spot
            .entry(t.spot.clone())
            .or_default()
            .add(&s, sp.weight);
    }
    (total, per_spot)
}

/// The source that recomputes `profile`, falling back on what the build
/// stored for the same id where the reference has no row.
fn recompute<'a>(profile: Option<&'a Profile>) -> impl Fn(&'a FlopTree) -> postflop::Source<'a> {
    move |t: &'a FlopTree| match profile {
        None => postflop::Source::Reference,
        Some(p) => postflop::Source::Recompute {
            profile: p,
            stored: t.sections.iter().find(|s| s.id == p.id),
        },
    }
}

fn fmt(v: Option<f64>, digits: usize) -> String {
    v.map_or("-".into(), |v| format!("{v:.digits$}"))
}

fn selected<'a>(
    profiles: &'a [(Profile, PathBuf, String)],
    only: &Option<Vec<String>>,
) -> Vec<&'a (Profile, PathBuf, String)> {
    profiles
        .iter()
        .filter(|(p, _, _)| only.as_ref().is_none_or(|o| o.contains(&p.id)))
        .collect()
}

fn cmd_preflop(o: &Opts) -> Result<(), String> {
    let published = chart("preflop.tafl")?;
    let prep = preflop::Prepared::new(&published);
    let profiles = load_profiles(&o.bots)?;
    println!(
        "{:<16} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "bot", "VPIP", "PFR", "3bet", "F3bet", "RFI"
    );
    let mut rows: Vec<(String, Option<&Profile>)> = vec![("reference".into(), None)];
    rows.extend(
        selected(&profiles, &o.only)
            .into_iter()
            .map(|(p, _, _)| (p.id.clone(), Some(p))),
    );
    for (id, p) in rows {
        let s = preflop::stats(&prep, p);
        println!(
            "{:<16} {:>6.1} {:>6.1} {:>6} {:>6} {:>6}",
            id,
            s.vpip,
            s.pfr,
            fmt(s.threebet, 1),
            fmt(s.fold_to_3bet, 1),
            fmt(s.rfi, 1)
        );
    }
    Ok(())
}

fn print_flop(id: &str, s: &FlopSums) {
    let all = s.decisions() + s.missing;
    let pct = |x: f64| fmt((all > 0.0).then(|| 100.0 * x / all), 1);
    println!(
        "{:<28} {:>6} {:>6} {:>6} {:>7} {:>7}",
        id,
        fmt(s.cbet_pct(), 1),
        fmt(s.fold_to_cbet_pct(), 1),
        fmt(s.af(), 2),
        pct(s.fixed),
        pct(s.missing),
    );
}

fn cmd_postflop(o: &Opts) -> Result<(), String> {
    let lib = o.library.as_ref().ok_or("postflop needs --library <dir>")?;
    let trees = load_library(lib)?;
    let published = chart("preflop.tafl")?;
    let profiles = load_profiles(&o.bots)?;
    println!(
        "{:<28} {:>6} {:>6} {:>6} {:>7} {:>7}",
        "bot / spot", "cbet", "Fcbet", "AF", "fixed%", "nodata%"
    );
    let print = |id: &str, (total, per_spot): (FlopSums, BTreeMap<String, FlopSums>)| {
        print_flop(id, &total);
        for (spot, s) in &per_spot {
            print_flop(&format!("  {spot}"), s);
        }
    };
    if !o.stored {
        let p = plan(&trees, &published, None, None)?;
        print("reference", flop_stats(&trees, &p, recompute(None)));
    }
    for (prof, _, _) in selected(&profiles, &o.only) {
        if o.stored {
            // What the build stored, with the knobs it was built with.
            let Some(sec) = trees[0].sections.iter().find(|s| s.id == prof.id) else {
                eprintln!("{}: not in the library, skipped", prof.id);
                continue;
            };
            let built = Profile::from_toml(&sec.profile)?;
            let p = plan(&trees, &published, Some(&built), Some(&prof.id))?;
            let id = prof.id.clone();
            let stats = flop_stats(&trees, &p, |t: &FlopTree| {
                postflop::Source::Stored(t.sections.iter().find(|s| s.id == id).expect("planned"))
            });
            print(&format!("{} (stored)", prof.id), stats);
        } else {
            let p = plan(&trees, &published, Some(prof), None)?;
            print(&prof.id, flop_stats(&trees, &p, recompute(Some(prof))));
        }
    }
    Ok(())
}

fn cmd_check(o: &Opts) -> Result<(), String> {
    let lib = o.library.as_ref().ok_or("check needs --library <dir>")?;
    let trees = load_library(lib)?;
    let mut worst_mean = 0.0f64;
    for t in &trees {
        for s in &t.sections {
            let (max, mean) = postflop::diff_vs_stored(t, s)?;
            worst_mean = worst_mean.max(mean);
            println!(
                "{:<12} {:<8} {:<16} max {:>5.1}%  mean {:>5.2}%",
                t.spot,
                t.flop,
                s.id,
                100.0 * max,
                100.0 * mean
            );
        }
    }
    println!(
        "worst range-weighted mean difference: {:.2}%",
        100.0 * worst_mean
    );
    Ok(())
}

fn cmd_tune(o: &Opts) -> Result<(), String> {
    let targets_text =
        std::fs::read_to_string(&o.targets).map_err(|e| format!("{}: {e}", o.targets.display()))?;
    let targets = tune::load_targets(&targets_text)?;
    let published = chart("preflop.tafl")?;
    let prep = preflop::Prepared::new(&published);
    let trees = match &o.library {
        Some(lib) => Some(load_library(lib)?),
        None => None,
    };
    let profiles = load_profiles(&o.bots)?;
    let mut all_ok = true;
    for (orig, path, text) in selected(&profiles, &o.only) {
        let Some(t) = targets.get(&orig.id) else {
            eprintln!("{}: no targets, skipped", orig.id);
            continue;
        };
        let mut p = orig.clone();
        eprintln!("== {}", p.id);
        let pre = tune_preflop(&prep, &mut p, t);
        let mut ok = pre;
        if let Some(trees) = &trees {
            ok &= tune_postflop(trees, &published, &mut p, t)?;
        }
        all_ok &= ok;
        let ps = preflop::stats(&prep, Some(&p));
        println!(
            "{:<16} vpip {:.3} pfr {:.3} 3bet {:.3} | VPIP {:.1} PFR {:.1} 3bet {} F3bet {}{}",
            p.id,
            p.preflop.vpip_mult,
            p.preflop.pfr_mult,
            p.preflop.threebet_mult,
            ps.vpip,
            ps.pfr,
            fmt(ps.threebet, 1),
            fmt(ps.fold_to_3bet, 1),
            if ok {
                ""
            } else {
                "  (not all inside the bands)"
            }
        );
        if trees.is_some() {
            println!(
                "{:<16} fold_to_cbet {:.3} aggression {:.3}",
                "", p.postflop.fold_to_cbet, p.postflop.aggression
            );
        }
        if o.write {
            let mut out = tune::rewrite_toml(
                text,
                "preflop",
                &[
                    ("vpip_mult", p.preflop.vpip_mult),
                    ("pfr_mult", p.preflop.pfr_mult),
                    ("threebet_mult", p.preflop.threebet_mult),
                ],
            );
            if trees.is_some() {
                out = tune::rewrite_toml(
                    &out,
                    "postflop",
                    &[
                        ("fold_to_cbet", p.postflop.fold_to_cbet),
                        ("aggression", p.postflop.aggression),
                    ],
                );
            }
            Profile::from_toml(&out)
                .map_err(|e| format!("{}: rewritten TOML: {e}", path.display()))?;
            std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("wrote {}", path.display());
        }
    }
    if !all_ok {
        eprintln!("some stats ended outside their bands");
    }
    Ok(())
}

const ROUNDS: usize = 6;
const STEPS: usize = 14;

/// Tune the three preflop multipliers; true when every stat is in its band.
fn tune_preflop(prep: &preflop::Prepared<'_>, p: &mut Profile, t: &Targets) -> bool {
    let read = |s: &preflop::PreflopStats, knob: &str| match knob {
        "threebet_mult" => s.threebet,
        "vpip_mult" => Some(s.vpip),
        _ => Some(s.pfr),
    };
    let band = |knob: &str| match knob {
        "threebet_mult" => t.threebet,
        "vpip_mult" => t.vpip,
        _ => t.pfr,
    };
    for round in 0..ROUNDS {
        let s = preflop::stats(prep, Some(p));
        let ok = tune::preflop_knobs().iter().all(|(k, _, _)| {
            let b = band(k);
            // Converge well inside the band so the knobs' coupling does
            // not push a stat back out.
            tune::within(read(&s, k), [b[0], b[1] / 4.0])
        });
        eprintln!(
            "  round {round}: VPIP {:.1} PFR {:.1} 3bet {}",
            s.vpip,
            s.pfr,
            fmt(s.threebet, 1)
        );
        if ok {
            break;
        }
        for (knob, lo, hi) in tune::preflop_knobs() {
            let b = band(knob);
            let x = tune::bisect(lo, hi, true, STEPS, b[0], |x| {
                let mut q = p.clone();
                tune::set_preflop(&mut q, knob, x);
                read(&preflop::stats(prep, Some(&q)), knob)
            });
            tune::set_preflop(p, knob, tune::round3(x));
        }
    }
    let s = preflop::stats(prep, Some(p));
    tune::within(Some(s.vpip), t.vpip)
        && tune::within(Some(s.pfr), t.pfr)
        && tune::within(s.threebet, t.threebet)
}

/// Tune `fold_to_cbet` and `aggression` on the flop. The preflop knobs
/// stay fixed meanwhile, so the seats and ranges are planned once.
fn tune_postflop(
    trees: &[FlopTree],
    published: &PreflopChart,
    p: &mut Profile,
    t: &Targets,
) -> Result<bool, String> {
    let seats = plan(trees, published, Some(p), None)?;
    let stats = |q: &Profile| flop_stats(trees, &seats, recompute(Some(q))).0;
    for round in 0..3 {
        let s = stats(p);
        eprintln!(
            "  flop round {round}: fold to cbet {} AF {} (fixed {:.1}%, no data {:.1}%)",
            fmt(s.fold_to_cbet_pct(), 1),
            fmt(s.af(), 2),
            100.0 * s.fixed / (s.decisions() + s.missing),
            100.0 * s.missing / (s.decisions() + s.missing),
        );
        if tune::within(
            s.fold_to_cbet_pct(),
            [t.fold_to_cbet[0], t.fold_to_cbet[1] / 4.0],
        ) && tune::within(s.af(), [t.af[0], t.af[1] / 4.0])
        {
            break;
        }
        let x = tune::bisect(-1.0, 1.0, false, STEPS, t.fold_to_cbet[0], |x| {
            let mut q = p.clone();
            q.postflop.fold_to_cbet = x;
            stats(&q).fold_to_cbet_pct()
        });
        p.postflop.fold_to_cbet = tune::round3(x);
        let x = tune::bisect(-1.0, 1.0, false, STEPS, t.af[0], |x| {
            let mut q = p.clone();
            q.postflop.aggression = x;
            stats(&q).af()
        });
        p.postflop.aggression = tune::round3(x);
    }
    let s = stats(p);
    Ok(tune::within(s.fold_to_cbet_pct(), t.fold_to_cbet) && tune::within(s.af(), t.af))
}

fn run() -> Result<(), String> {
    let o = parse_args()?;
    match o.command.as_str() {
        "preflop" => cmd_preflop(&o),
        "postflop" => cmd_postflop(&o),
        "check" => cmd_check(&o),
        "tune" => cmd_tune(&o),
        _ => Err("usage: profile-stats preflop|postflop|check|tune [--library <dir>] [--bots <dir>] [--targets <file>] [--only <ids>] [--write]".into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("profile-stats: {e}");
            ExitCode::FAILURE
        }
    }
}

//! Own offline preflop solver (spec §6, D12).
//!
//! Pipeline: manifest → 169×169 equity table (cached) → tree from the rules
//! engine → DCFR → evaluation (EVs, exploitability) → chart in the Taful
//! library format (preflop pseudo-board, class rows).

use std::path::{Path, PathBuf};
use std::time::Instant;

use taful_core::config::BB_CHIPS;
use taful_core::range::HandClass;
use taful_solver::format::{
    Child, Domain, Header, LibraryFile, Node, NodeAction, ev_scale_for, quantize_ev,
    quantize_strategy,
};

use taful_core::profile::Profile;
use taful_solver::preflop::PreflopChart;

pub use taful_solver::preflop_game::{cfr, equity, manifest, tree};

use cfr::{Report, Solver, average_strategy};
use equity::{ClassTables, N};
use manifest::Manifest;
use taful_solver::preflop_game::{SeatPlay, chart_ids, lock_published};
use tree::{TNode, Tree};

/// Solver id written into chart headers.
pub const SOLVER_ID: &str = "taful-preflop 1";

/// Result of [`solve`].
pub struct Solved {
    pub tree: Tree,
    pub report: Report,
}

/// Build the tree and run the solve.
pub fn solve(m: &Manifest, tables: ClassTables, mut log: impl FnMut(&str)) -> Solved {
    let t0 = Instant::now();
    let mut tree = Tree::build(m, tables);
    log(&format!(
        "tree: {} decision nodes, {} terminals",
        tree.decisions, tree.terminals
    ));
    let mut solver = Solver { tree: &mut tree };
    solver.train(m.solve.iterations, |i| {
        log(&format!(
            "iteration {i}/{} ({:.0}s)",
            m.solve.iterations,
            t0.elapsed().as_secs_f32()
        ))
    });
    let report = solver.evaluate();
    log(&format!(
        "best-response gain per seat (bb/hand): {:?}; exploitability {:.3}% of the blinds ({:.0}s)",
        report
            .br_gain_bb
            .iter()
            .map(|g| (g * 1000.0).round() / 1000.0)
            .collect::<Vec<_>>(),
        report.exploitability_pct,
        t0.elapsed().as_secs_f32()
    ));
    Solved { tree, report }
}

struct Exporter {
    nodes: Vec<Node>,
    prune_share: f32,
    ev_scale: f32,
    /// Export the locked profile strategy and the best-response EVs instead
    /// of the reference (same nodes and pruning, so node ids match).
    profile: bool,
}

impl Exporter {
    fn export(&mut self, node: &TNode) -> Child {
        let TNode::Decision(d) = node else {
            return Child::End;
        };
        let at = self.nodes.len();
        if at > 0 && d.reach_share < self.prune_share {
            return Child::Pruned;
        }
        let k = d.actions.len();
        let (sigma, values) = if self.profile {
            (d.locked.clone(), &d.br_ev)
        } else {
            (average_strategy(d), &d.ev)
        };
        let mut strategy = Vec::with_capacity(N * k);
        let mut ev = Vec::with_capacity(N * k);
        for h in 0..N {
            let row: Vec<f32> = (0..k).map(|a| sigma[a * N + h]).collect();
            strategy.extend(quantize_strategy(&row));
            for a in 0..k {
                ev.push(quantize_ev(
                    values[a * N + h] / BB_CHIPS as f32,
                    self.ev_scale,
                ));
            }
        }
        self.nodes.push(Node {
            player: d.position,
            pot: d.pot,
            exploitability_pct: d.exploitability_pct.max(0.0),
            actions: Vec::new(),
            live: (0..N as u16).collect(),
            strategy,
            ev,
        });
        let actions = d
            .actions
            .iter()
            .zip(&d.children)
            .map(|(&action, child)| NodeAction {
                action,
                child: self.export(child),
            })
            .collect();
        self.nodes[at].actions = actions;
        Child::Node(at as u32)
    }
}

fn max_abs_ev(node: &TNode, profile: bool) -> f32 {
    match node {
        TNode::Terminal(_) => 0.0,
        TNode::Decision(d) => {
            let values = if profile { &d.br_ev } else { &d.ev };
            values
                .iter()
                .map(|e| e.abs() / BB_CHIPS as f32)
                .chain(d.children.iter().map(|c| max_abs_ev(c, profile)))
                .fold(0.0, f32::max)
        }
    }
}

/// Turn a solved tree into a chart file.
pub fn export(m: &Manifest, solved: &Solved) -> LibraryFile {
    export_with(m, solved, None)
}

/// Header `line` of the unpruned reference chart.
pub const FULL_LINE: &str = "unpruned";

/// The reference chart with every node of the tree (`preflop-full.tafl`):
/// what the app locks profiles on where the chart is pruned, for mixed
/// tables (#29).
pub fn export_full(m: &Manifest, solved: &Solved) -> LibraryFile {
    let mut m = m.clone();
    let hash = m.hash();
    m.solve.prune_reach_pct = 0.0;
    let mut file = export_with(&m, solved, None);
    file.header.line = FULL_LINE.into();
    file.header.manifest_hash = hash;
    file
}

/// Header `line` of a profile's best-response chart.
pub fn profile_line(id: &str) -> String {
    format!("profile:{id}")
}

/// The reference chart (`profile = None`) or, after [`lock_profile`] and
/// [`Solver::best_response_vs_locked`], a profile's best-response chart:
/// the profile's strategy and every actor's best-response EVs against the
/// profile at every other seat, on the reference chart's nodes.
fn export_with(m: &Manifest, solved: &Solved, profile: Option<(&str, [u8; 32])>) -> LibraryFile {
    let config = m.game_config();
    let ev_scale = ev_scale_for(max_abs_ev(&solved.tree.root, profile.is_some()));
    let mut ex = Exporter {
        nodes: Vec::new(),
        prune_share: (m.solve.prune_reach_pct / 100.0) as f32,
        ev_scale,
        profile: profile.is_some(),
    };
    ex.export(&solved.tree.root);
    let seats = solved.tree.seats;
    let root_ranges = (0..seats)
        .map(|s| {
            (
                taful_core::position::Position::of(seats, solved.tree.button, s),
                vec![255u8; HandClass::COUNT],
            )
        })
        .collect();
    LibraryFile {
        header: Header {
            game_config: config.to_toml(),
            config_key: config.key().0,
            line: profile.map_or(String::new(), |(id, _)| profile_line(id)),
            board: Vec::new(),
            bet_tree: m.bet_tree_descriptor(),
            domain: Domain::Classes,
            root_ranges,
            exploitability_pct: solved.report.exploitability_pct,
            ev_scale,
            solver_id: SOLVER_ID.into(),
            manifest_hash: profile.map_or_else(|| m.hash(), |(_, h)| h),
        },
        nodes: ex.nodes,
        chance: Vec::new(),
        profiles: Vec::new(),
    }
}

/// Locks `profile` at every decision node (spec §9): the profile's
/// transform over the published reference strategy, the chart's rows where
/// the chart has the node and the unpruned chart's elsewhere: exactly what
/// the app's `ProfiledBot` sees, and what the app locks for a mixed table
/// (#29), so a table of one profile there reproduces this chart.
pub fn lock_profile(tree: &mut Tree, chart: &PreflopChart, full: &PreflopChart, profile: &Profile) {
    let seats = vec![SeatPlay::Profile(profile); tree.seats];
    let ids = chart_ids(chart, full);
    assert_eq!(ids.len(), tree.decisions, "unpruned chart has every node");
    assert_eq!(
        ids.iter().flatten().count(),
        chart.len(),
        "every chart node is in the tree"
    );
    lock_published(tree, chart, full, &ids, &seats);
}

/// Best-response charts for every profile, from a solved tree and its
/// reference chart.
pub fn profile_charts(
    m: &Manifest,
    solved: &mut Solved,
    chart: &PreflopChart,
    full: &PreflopChart,
    profiles: &[Profile],
    mut log: impl FnMut(&str),
) -> Vec<(String, LibraryFile)> {
    let hash = m.profile_hash(profiles);
    profiles
        .iter()
        .map(|p| {
            let t0 = Instant::now();
            lock_profile(&mut solved.tree, chart, full, p);
            Solver {
                tree: &mut solved.tree,
            }
            .best_response_vs_locked();
            log(&format!(
                "profile {}: best responses ({:.0}s)",
                p.id,
                t0.elapsed().as_secs_f32()
            ));
            (p.id.clone(), export_with(m, solved, Some((&p.id, hash))))
        })
        .collect()
}

/// Path of a profile's best-response chart, next to the reference chart:
/// `v{layout}/{config key}/preflop-{id}.tafl`.
pub fn profile_chart_path(m: &Manifest, id: &str) -> PathBuf {
    chart_path(m).with_file_name(format!("preflop-{id}.tafl"))
}

/// Path of the unpruned reference chart, next to the reference chart.
pub fn full_chart_path(m: &Manifest) -> PathBuf {
    chart_path(m).with_file_name("preflop-full.tafl")
}

/// Path of the 169×169 equity table the tree was built on, next to the
/// reference chart (the app rebuilds the tree from it for mixed tables).
pub fn equity_path(m: &Manifest) -> PathBuf {
    chart_path(m).with_file_name("preflop-equity.bin")
}

/// Path of a preflop chart under the library root, following the static
/// layout of spec §7.6: `v{layout}/{config key}/preflop.tafl`.
pub fn chart_path(m: &Manifest) -> PathBuf {
    PathBuf::from(format!("v{}", taful_solver::LIBRARY_LAYOUT_VERSION))
        .join(m.game_config().key().to_string())
        .join("preflop.tafl")
}

/// `library-build preflop <manifest> <out-dir>`: writes the chart under
/// `<out-dir>/v1/<config key>/preflop.tafl`, the unpruned chart and the
/// equity table next to it (`preflop-full.tafl`, `preflop-equity.bin`, for
/// mixed tables), one best-response chart per profile
/// (`preflop-<id>.tafl`), and caches the equity table in `<out-dir>/cache/`.
pub fn run(manifest_path: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let m = Manifest::from_toml(&text)?;
    let profiles = m.load_profiles(manifest_path.parent().unwrap_or(Path::new(".")))?;
    let log = |s: &str| eprintln!("[preflop] {s}");
    let t0 = Instant::now();
    let tables = ClassTables::cached(&out_dir.join("cache"), m.solve.equity_samples, m.solve.seed)
        .map_err(|e| format!("equity table: {e}"))?;
    log(&format!(
        "equity table ready ({:.0}s)",
        t0.elapsed().as_secs_f32()
    ));
    let mut solved = solve(&m, tables, log);
    let file = export(&m, &solved);
    let bytes = file.encode();
    let path = out_dir.join(chart_path(&m));
    std::fs::create_dir_all(path.parent().expect("has parent")).map_err(|e| e.to_string())?;
    let write = |path: &Path, file: &LibraryFile, bytes: &[u8]| -> Result<(), String> {
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        log(&format!(
            "wrote {} ({} nodes, {} KB)",
            path.display(),
            file.nodes.len(),
            bytes.len() / 1024
        ));
        Ok(())
    };
    write(&path, &file, &bytes)?;
    let full = export_full(&m, &solved);
    let full_bytes = full.encode();
    write(&out_dir.join(full_chart_path(&m)), &full, &full_bytes)?;
    let equity = out_dir.join(equity_path(&m));
    std::fs::write(&equity, solved.tree.tables.equity_bytes())
        .map_err(|e| format!("{}: {e}", equity.display()))?;
    // Profiles lock on what was published, read back from the bytes.
    let chart = PreflopChart::decode(&bytes).map_err(|e| e.to_string())?;
    let full = PreflopChart::decode(&full_bytes).map_err(|e| e.to_string())?;
    for (id, f) in profile_charts(&m, &mut solved, &chart, &full, &profiles, log) {
        write(&out_dir.join(profile_chart_path(&m, &id)), &f, &f.encode())?;
    }
    Ok(path)
}

/// `library-build preflop-mixed-bench <manifest> <out-dir>`: solves the
/// manifest, then times what the app runs for a mixed table (#29): building
/// [`TableGame`] once, and the on-demand best response for a set of seat
/// combinations (one profile per bot seat).
pub fn bench_mixed(manifest_path: &Path, out_dir: &Path) -> Result<(), String> {
    use std::sync::Arc;
    use taful_solver::preflop_game::TableGame;

    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let m = Manifest::from_toml(&text)?;
    let profiles = m.load_profiles(manifest_path.parent().unwrap_or(Path::new(".")))?;
    let log = |s: &str| eprintln!("[bench] {s}");
    let tables = ClassTables::cached(&out_dir.join("cache"), m.solve.equity_samples, m.solve.seed)
        .map_err(|e| format!("equity table: {e}"))?;
    let solved = solve(&m, tables.clone(), log);
    let chart = PreflopChart::new(export(&m, &solved)).map_err(|e| e.to_string())?;
    let full = PreflopChart::new(export_full(&m, &solved)).map_err(|e| e.to_string())?;
    log(&format!(
        "tree {} decisions, chart {} nodes",
        solved.tree.decisions,
        chart.len()
    ));
    let t = Instant::now();
    let mut game = TableGame::new(&m, tables, Arc::new(chart), full)?;
    log(&format!(
        "table game built in {:.3}s",
        t.elapsed().as_secs_f32()
    ));
    let seats = game.seats();
    let by_id = |id: &str| profiles.iter().find(|p| p.id == id).expect("profile");
    let mixes: [[&str; 5]; 4] = [
        ["nit", "nit", "maniac", "tag", "tag"],
        ["calling-station", "maniac", "nit", "tag", "calling-station"],
        ["maniac", "maniac", "maniac", "nit", "nit"],
        ["tag", "calling-station", "tag", "nit", "maniac"],
    ];
    let mut times = Vec::new();
    for mix in &mixes {
        for hero in 0..seats {
            let mut it = mix.iter();
            let plays: Vec<SeatPlay> = (0..seats)
                .map(|s| {
                    if s == hero {
                        SeatPlay::Responder
                    } else {
                        SeatPlay::Profile(by_id(it.next().expect("5 bots")))
                    }
                })
                .collect();
            let t = Instant::now();
            let br = game.best_response(hero, &plays);
            let secs = t.elapsed().as_secs_f32();
            log(&format!(
                "hero seat {hero} vs {mix:?}: {secs:.3}s ({} hero nodes)",
                br.len()
            ));
            times.push(secs);
        }
    }
    times.sort_by(f32::total_cmp);
    log(&format!(
        "{} combinations: median {:.3}s, p90 {:.3}s, max {:.3}s",
        times.len(),
        times[times.len() / 2],
        times[times.len() * 9 / 10],
        times[times.len() - 1]
    ));
    Ok(())
}

/// Human-readable summary of a chart: for every node up to `max_depth`
/// actions deep, the share of the actor's range taking each action.
pub fn summary(bytes: &[u8], max_depth: usize) -> Result<String, String> {
    use std::fmt::Write;
    let chart = taful_solver::preflop::PreflopChart::decode(bytes).map_err(|e| e.to_string())?;
    let h = &chart.file().header;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} | {} nodes | exploitability {:.3}% of the blinds\n{}",
        h.solver_id,
        chart.len(),
        h.exploitability_pct,
        h.bet_tree
    );
    for id in 0..chart.len() {
        let line = chart.line(id);
        if line.0.len() > max_depth {
            continue;
        }
        let node = chart.node(id);
        let range = chart.acting_range(id);
        let reach = chart.reach(id, node.player);
        let combos: f32 = (0..N)
            .map(|c| reach[c] * HandClass::from_index(c).expect("< 169").num_combos() as f32)
            .sum();
        let mut parts = Vec::new();
        for (a, na) in node.actions.iter().enumerate() {
            let share: f32 = (0..N)
                .map(|c| range[c] * node.strategy(c).map_or(0.0, |s| s[a]))
                .sum();
            parts.push(format!("{} {:.1}%", na.action, 100.0 * share));
        }
        let _ = writeln!(
            out,
            "[{}] {} ({:.0} combos, xpl {:.2}%): {}",
            if line.0.is_empty() {
                "-".to_string()
            } else {
                line.to_string()
            },
            node.player,
            combos,
            node.exploitability_pct,
            parts.join(", ")
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use taful_core::action::Action;
    use taful_core::cards::Hand;
    use taful_solver::preflop::{Lookup, PreflopChart};

    use super::*;

    fn manifest(table_size: u8, stack_bb: u32, iterations: u32) -> Manifest {
        let mut m =
            Manifest::from_toml(include_str!("../../manifests/preflop-6max-100bb.toml")).unwrap();
        m.game.table_size = table_size;
        m.game.stack_bb = stack_bb;
        m.solve.iterations = iterations;
        m.solve.equity_samples = 16;
        m
    }

    fn tables() -> ClassTables {
        use std::sync::OnceLock;
        static T: OnceLock<ClassTables> = OnceLock::new();
        T.get_or_init(|| ClassTables::compute(16, 1)).clone()
    }

    fn solved(m: &Manifest) -> (Solved, PreflopChart) {
        let s = solve(m, tables(), |_| {});
        let chart = PreflopChart::decode(&export(m, &s).encode()).unwrap();
        (s, chart)
    }

    #[test]
    fn heads_up_converges_and_exports() {
        let m = manifest(2, 20, 400);
        let early = solve(&manifest(2, 20, 20), tables(), |_| {}).report;
        let (s, chart) = solved(&m);
        assert!(
            s.report.exploitability_pct < early.exploitability_pct,
            "{} !< {}",
            s.report.exploitability_pct,
            early.exploitability_pct
        );
        assert!(
            s.report.exploitability_pct < 1.0,
            "{}",
            s.report.exploitability_pct
        );
        assert_eq!(chart.config(), &m.game_config());
        // Heads-up the button (small blind) acts first.
        let root = chart.root();
        assert_eq!(chart.node(root).player, taful_core::position::Position::Btn);
        let aa: Hand = "AhAd".parse().unwrap();
        let fold = chart.node(root).action_index(Action::Fold).unwrap();
        assert!(chart.strategy(root, aa).unwrap()[fold] < 0.01);
        // Folding aces costs at least the small blind's alternative.
        let ev = chart.ev(root, aa).unwrap();
        assert!(ev.iter().copied().fold(f32::MIN, f32::max) > ev[fold] + 0.5);
        // Fold EV is exactly minus the posted small blind.
        assert!((ev[fold] + 0.5).abs() < 0.01, "{}", ev[fold]);
        // Lines round-trip through the chart.
        for id in 0..chart.len() {
            assert_eq!(chart.find(&chart.line(id)), Lookup::Node(id));
        }
    }

    #[test]
    fn six_max_tree_has_every_d2_spot() {
        let m = manifest(6, 100, 1);
        let tree = Tree::build(&m, tables());
        assert!(tree.decisions > 1000, "{}", tree.decisions);
        let s = solve(&m, tables(), |_| {});
        let file = export(&m, &s);
        // With one iteration nothing is pruned for reach yet except
        // never-played branches; every node has 169 rows.
        assert!(file.nodes.iter().all(|n| n.live.len() == 169));
        let chart = PreflopChart::new(file).unwrap();
        for line in [
            "",
            "UTG:r250",
            "UTG:r250,HJ:c",
            "UTG:r250,HJ:c,CO:r1000",
            "UTG:r250,HJ:r750,CO:f,BTN:f,SB:f,BB:f",
            "UTG:f,HJ:f,CO:f,BTN:f,SB:r300",
        ] {
            assert!(
                matches!(chart.find(&line.parse().unwrap()), Lookup::Node(_)),
                "{line:?}"
            );
        }
        // No limps, no cold calls of a 3-bet.
        assert_eq!(chart.find(&"UTG:c".parse().unwrap()), Lookup::Left);
        assert_eq!(
            chart.find(&"UTG:r250,HJ:r750,CO:c".parse().unwrap()),
            Lookup::Left
        );
    }

    #[test]
    fn best_responses_against_profiles() {
        let m = manifest(2, 20, 300);
        let mut s = solve(&m, tables(), |_| {});
        let file = export(&m, &s);
        let chart = PreflopChart::new(file.clone()).unwrap();
        let mk = |vpip: f64, pfr: f64, threebet: f64| {
            Profile::from_toml(&format!(
                "id = \"p\"\n[preflop]\nvpip_mult = {vpip}\npfr_mult = {pfr}\nthreebet_mult = {threebet}\n\
                 [postflop]\nfold_to_cbet = 0.0\nfold_to_raise = 0.0\nbluff_value_shift = 0.0\naggression = 0.0\n\
                 [sizing]\nprefer = \"reference\"\n"
            ))
            .unwrap()
        };
        let full = PreflopChart::new(export_full(&m, &s)).unwrap();
        let charts = profile_charts(
            &m,
            &mut s,
            &chart,
            &full,
            &[mk(1.0, 1.0, 1.0), mk(1.0, 0.3, 1.0)],
            |_| {},
        );
        let identity = PreflopChart::new(charts[0].1.clone()).unwrap();
        let tight = PreflopChart::new(charts[1].1.clone()).unwrap();
        assert_eq!(identity.len(), chart.len());
        assert_eq!(identity.file().header.line, "profile:p");
        // Against the reference itself the best response gains next to
        // nothing over the reference EVs (the solve is close to converged).
        let root = chart.root();
        let aa: taful_core::cards::Hand = "AhAd".parse().unwrap();
        let (r, i) = (chart.ev(root, aa).unwrap(), identity.ev(root, aa).unwrap());
        let best = |v: &[f32]| v.iter().copied().fold(f32::MIN, f32::max);
        assert!((best(&r) - best(&i)).abs() < 0.2, "{r:?} vs {i:?}");
        // A button that opens far less: the big blind facing the open (a
        // strong range) gains less by calling junk than against the
        // reference.
        let open = chart
            .node(root)
            .actions
            .iter()
            .position(|a| matches!(a.action, Action::Raise(_)))
            .unwrap();
        let taful_solver::format::Child::Node(bb) = chart.node(root).actions[open].child else {
            panic!("big blind node");
        };
        let bb = bb as usize;
        let call = chart.node(bb).action_index(Action::Call).unwrap();
        let junk: taful_core::cards::Hand = "7c2d".parse().unwrap();
        assert!(
            tight.ev(bb, junk).unwrap()[call] < identity.ev(bb, junk).unwrap()[call],
            "{:?} vs {:?}",
            tight.ev(bb, junk),
            identity.ev(bb, junk)
        );
    }

    /// The app's on-demand best response (`TableGame`, #29) at a table
    /// where every bot plays one profile reproduces that profile's
    /// best-response chart: same tree, same locks (the unpruned chart
    /// stands in for the solver below the pruning).
    #[test]
    fn table_game_matches_profile_charts() {
        use std::sync::Arc;
        use taful_solver::preflop_game::TableGame;

        let mut m = manifest(6, 100, 40);
        m.solve.prune_reach_pct = 2.0;
        let mut s = solve(&m, tables(), |_| {});
        let chart = PreflopChart::new(export(&m, &s)).unwrap();
        let full = PreflopChart::new(export_full(&m, &s)).unwrap();
        assert!(full.len() > chart.len(), "something is pruned");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests");
        let profiles = m.load_profiles(&dir).unwrap();
        let charts = profile_charts(&m, &mut s, &chart, &full, &profiles, |_| {});
        let mut game = TableGame::new(&m, tables(), Arc::new(chart.clone()), full).unwrap();
        for (p, (_, file)) in profiles.iter().zip(&charts) {
            let shipped = PreflopChart::new(file.clone()).unwrap();
            for hero in [2, 3] {
                let plays = vec![SeatPlay::Profile(p); 6];
                let br = game.best_response(hero, &plays);
                let pos = taful_core::position::Position::of(6, 0, hero);
                let mut worst = (0.0f32, String::new());
                for n in (0..chart.len()).filter(|&n| chart.node(n).player == pos) {
                    for class in HandClass::all() {
                        let hand = class.combos()[0];
                        let (a, b) = (br.ev(n, hand).unwrap(), shipped.ev(n, hand).unwrap());
                        for (x, y) in a.iter().zip(&b) {
                            if (x - y).abs() > worst.0 {
                                worst = (
                                    (x - y).abs(),
                                    format!("{} {class} {a:?} {b:?}", chart.line(n)),
                                );
                            }
                        }
                    }
                }
                // Same game, same walk: only the export's EV quantization.
                let step = file.header.ev_scale;
                assert!(worst.0 <= step, "{} seat {hero}: {worst:?}", p.id);
            }
        }
    }

    #[test]
    fn summary_lists_the_root() {
        let m = manifest(2, 20, 10);
        let (s, _) = solved(&m);
        let text = summary(&export(&m, &s).encode(), 1).unwrap();
        assert!(text.contains("[-] BTN"), "{text}");
    }
}

#[cfg(test)]
mod shipped {
    use taful_solver::preflop::PreflopChart;

    use super::*;

    /// The chart the app embeds must come from the manifest in the repo:
    /// editing the manifest without rebuilding the chart fails here.
    #[test]
    fn committed_chart_matches_its_manifest() {
        let m =
            Manifest::from_toml(include_str!("../../manifests/preflop-6max-100bb.toml")).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
        let bytes = std::fs::read(root.join(chart_path(&m))).expect("chart committed");
        let chart = PreflopChart::decode(&bytes).unwrap();
        let h = &chart.file().header;
        assert_eq!(
            h.manifest_hash,
            m.hash(),
            "rebuild the chart (docs/development.md)"
        );
        assert_eq!(h.solver_id, SOLVER_ID);
        assert_eq!(h.bet_tree, m.bet_tree_descriptor());
        assert!(h.exploitability_pct < 0.5, "{}", h.exploitability_pct);
    }

    /// The unpruned chart and the equity table the app rebuilds the tree
    /// from (#29) come from this manifest and sit on its tree.
    #[test]
    fn committed_unpruned_chart_and_equity_match_the_tree() {
        let m =
            Manifest::from_toml(include_str!("../../manifests/preflop-6max-100bb.toml")).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
        let read = |p: PathBuf| std::fs::read(root.join(p)).expect("committed");
        let full = PreflopChart::decode(&read(full_chart_path(&m))).unwrap();
        assert_eq!(full.file().header.line, FULL_LINE);
        assert_eq!(
            full.file().header.manifest_hash,
            m.hash(),
            "rebuild the charts"
        );
        let tables = ClassTables::from_equity_bytes(
            &read(equity_path(&m)),
            m.solve.equity_samples,
            m.solve.seed,
        )
        .expect("169×169 equity table");
        let chart = PreflopChart::decode(&read(chart_path(&m))).unwrap();
        taful_solver::preflop_game::TableGame::new(&m, tables, std::sync::Arc::new(chart), full)
            .unwrap();
    }

    /// Every profile in the manifest has a best-response chart built from
    /// this manifest and these profile files: editing a profile without
    /// rebuilding fails here.
    #[test]
    fn committed_profile_charts_match_their_profiles() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let m =
            Manifest::from_toml(include_str!("../../manifests/preflop-6max-100bb.toml")).unwrap();
        let profiles = m.load_profiles(&dir.join("manifests")).unwrap();
        assert!(!profiles.is_empty());
        let root = dir.join("../../library");
        let reference =
            PreflopChart::decode(&std::fs::read(root.join(chart_path(&m))).unwrap()).unwrap();
        for p in &profiles {
            let bytes = std::fs::read(root.join(profile_chart_path(&m, &p.id)))
                .unwrap_or_else(|_| panic!("chart for `{}` committed", p.id));
            let chart = PreflopChart::decode(&bytes).unwrap();
            let h = &chart.file().header;
            assert_eq!(h.line, profile_line(&p.id));
            assert_eq!(
                h.manifest_hash,
                m.profile_hash(&profiles),
                "rebuild the charts (docs/development.md)"
            );
            assert_eq!(chart.len(), reference.len());
        }
    }
}

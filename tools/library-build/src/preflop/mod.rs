//! Own offline preflop solver (spec §6, D12).
//!
//! Pipeline: manifest → 169×169 equity table (cached) → tree from the rules
//! engine → DCFR → evaluation (EVs, exploitability) → chart in the Taful
//! library format (preflop pseudo-board, class rows).

pub mod cfr;
pub mod equity;
pub mod manifest;
pub mod tree;

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

use cfr::{Report, Solver, average_strategy};
use equity::{ClassTables, N};
use manifest::Manifest;
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
    }
}

/// Locks `profile` at every decision node (spec §9): the profile's
/// transform over the reference strategy. Nodes in the chart use the chart's
/// own (quantized) strategy, EVs and range, exactly what the app's
/// `ProfiledBot` sees; nodes pruned from the chart use the solver's.
pub fn lock_profile(tree: &mut Tree, chart: &PreflopChart, profile: &Profile, prune_share: f32) {
    fn visit(
        node: &mut TNode,
        in_chart: bool,
        next: &mut usize,
        chart: &PreflopChart,
        profile: &Profile,
        prune_share: f32,
        sizes: &[f32; N],
    ) {
        let TNode::Decision(d) = node else { return };
        let id = (in_chart && (*next == 0 || d.reach_share >= prune_share)).then(|| {
            *next += 1;
            *next - 1
        });
        let k = d.actions.len();
        let (strategy, ev, weights) = match id {
            Some(id) => {
                let n = chart.node(id);
                debug_assert_eq!(n.actions.len(), k);
                let scale = chart.file().header.ev_scale;
                let mut s = Vec::with_capacity(N * k);
                let mut e = Vec::with_capacity(N * k);
                for c in 0..N {
                    s.extend(n.strategy(c).expect("class rows"));
                    e.extend(n.ev(c, scale).expect("class rows"));
                }
                (s, e, chart.acting_range(id).to_vec())
            }
            None => {
                let sigma = average_strategy(d);
                let mut s = Vec::with_capacity(N * k);
                let mut e = Vec::with_capacity(N * k);
                for h in 0..N {
                    for a in 0..k {
                        s.push(sigma[a * N + h]);
                        e.push(d.ev[a * N + h] / BB_CHIPS as f32);
                    }
                }
                let w = (0..N).map(|h| d.reach_cache[h] * sizes[h]).collect();
                (s, e, w)
            }
        };
        let actions = d.actions.clone();
        let rows = profile.preflop_strategy(&actions, &strategy, &ev, &weights);
        d.locked = vec![0.0; k * N];
        for h in 0..N {
            for a in 0..k {
                d.locked[a * N + h] = rows[h * k + a];
            }
        }
        for child in &mut d.children {
            visit(
                child,
                id.is_some(),
                next,
                chart,
                profile,
                prune_share,
                sizes,
            );
        }
    }
    let sizes = equity::class_sizes();
    let mut next = 0;
    visit(
        &mut tree.root,
        true,
        &mut next,
        chart,
        profile,
        prune_share,
        &sizes,
    );
    assert_eq!(next, chart.len(), "chart nodes follow the tree's DFS order");
}

/// Best-response charts for every profile, from a solved tree and its
/// reference chart.
pub fn profile_charts(
    m: &Manifest,
    solved: &mut Solved,
    chart: &PreflopChart,
    profiles: &[Profile],
    mut log: impl FnMut(&str),
) -> Vec<(String, LibraryFile)> {
    let hash = m.profile_hash(profiles);
    let prune = (m.solve.prune_reach_pct / 100.0) as f32;
    profiles
        .iter()
        .map(|p| {
            let t0 = Instant::now();
            lock_profile(&mut solved.tree, chart, p, prune);
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

/// Path of a preflop chart under the library root, following the static
/// layout of spec §7.6: `v{layout}/{config key}/preflop.tafl`.
pub fn chart_path(m: &Manifest) -> PathBuf {
    PathBuf::from(format!("v{}", taful_solver::LIBRARY_LAYOUT_VERSION))
        .join(m.game_config().key().to_string())
        .join("preflop.tafl")
}

/// `library-build preflop <manifest> <out-dir>`: writes the chart under
/// `<out-dir>/v1/<config key>/preflop.tafl`, one best-response chart per
/// profile next to it (`preflop-<id>.tafl`), and caches the equity table in
/// `<out-dir>/cache/`.
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
    let chart = PreflopChart::new(file).map_err(|e| e.to_string())?;
    for (id, f) in profile_charts(&m, &mut solved, &chart, &profiles, log) {
        write(&out_dir.join(profile_chart_path(&m, &id)), &f, &f.encode())?;
    }
    Ok(path)
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
        let charts = profile_charts(
            &m,
            &mut s,
            &chart,
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

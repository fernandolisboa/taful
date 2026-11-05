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

struct Exporter<'a> {
    nodes: Vec<Node>,
    prune_share: f32,
    ev_scale: f32,
    _m: &'a Manifest,
}

impl Exporter<'_> {
    fn export(&mut self, node: &TNode) -> Child {
        let TNode::Decision(d) = node else {
            return Child::End;
        };
        let at = self.nodes.len();
        if at > 0 && d.reach_share < self.prune_share {
            return Child::Pruned;
        }
        let k = d.actions.len();
        let sigma = average_strategy(d);
        let mut strategy = Vec::with_capacity(N * k);
        let mut ev = Vec::with_capacity(N * k);
        for h in 0..N {
            let row: Vec<f32> = (0..k).map(|a| sigma[a * N + h]).collect();
            strategy.extend(quantize_strategy(&row));
            for a in 0..k {
                ev.push(quantize_ev(
                    d.ev[a * N + h] / BB_CHIPS as f32,
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

fn max_abs_ev(node: &TNode) -> f32 {
    match node {
        TNode::Terminal(_) => 0.0,
        TNode::Decision(d) => {
            d.ev.iter()
                .map(|e| e.abs() / BB_CHIPS as f32)
                .chain(d.children.iter().map(max_abs_ev))
                .fold(0.0, f32::max)
        }
    }
}

/// Turn a solved tree into a chart file.
pub fn export(m: &Manifest, solved: &Solved) -> LibraryFile {
    let config = m.game_config();
    let ev_scale = ev_scale_for(max_abs_ev(&solved.tree.root));
    let mut ex = Exporter {
        nodes: Vec::new(),
        prune_share: (m.solve.prune_reach_pct / 100.0) as f32,
        ev_scale,
        _m: m,
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
            line: String::new(),
            board: Vec::new(),
            bet_tree: m.bet_tree_descriptor(),
            domain: Domain::Classes,
            root_ranges,
            exploitability_pct: solved.report.exploitability_pct,
            ev_scale,
            solver_id: SOLVER_ID.into(),
            manifest_hash: m.hash(),
        },
        nodes: ex.nodes,
    }
}

/// Path of a preflop chart under the library root, following the static
/// layout of spec §7.6: `v{format}/{config key}/preflop.tafl`.
pub fn chart_path(m: &Manifest) -> PathBuf {
    PathBuf::from(format!("v{}", taful_solver::LIBRARY_FORMAT_VERSION))
        .join(m.game_config().key().to_string())
        .join("preflop.tafl")
}

/// `library-build preflop <manifest> <out-dir>`: writes the chart under
/// `<out-dir>/v1/<config key>/preflop.tafl` and caches the equity table in
/// `<out-dir>/cache/`.
pub fn run(manifest_path: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let m = Manifest::from_toml(&text)?;
    let log = |s: &str| eprintln!("[preflop] {s}");
    let t0 = Instant::now();
    let tables = ClassTables::cached(&out_dir.join("cache"), m.solve.equity_samples, m.solve.seed)
        .map_err(|e| format!("equity table: {e}"))?;
    log(&format!(
        "equity table ready ({:.0}s)",
        t0.elapsed().as_secs_f32()
    ));
    let solved = solve(&m, tables, log);
    let file = export(&m, &solved);
    let bytes = file.encode();
    let path = out_dir.join(chart_path(&m));
    std::fs::create_dir_all(path.parent().expect("has parent")).map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    log(&format!(
        "wrote {} ({} nodes, {} KB)",
        path.display(),
        file.nodes.len(),
        bytes.len() / 1024
    ));
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
}

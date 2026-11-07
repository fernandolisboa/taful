//! The preflop game (spec §6, D12): the manifest, the 169×169 class tables,
//! the tree built from the rules engine and the vector-form DCFR / best
//! response over it.
//!
//! The offline build (`tools/library-build`) solves it and exports the
//! charts. The app uses the same tree for one thing only: the best response
//! against a mixed table (one bot profile per seat, #29, D8 amendment),
//! computed on demand by [`TableGame`] because there are too many seat
//! combinations to ship.

pub mod cfr;
pub mod equity;
pub mod manifest;
pub mod tree;

use std::collections::HashMap;

use taful_core::cards::Hand;
use taful_core::config::BB_CHIPS;
use taful_core::position::Seat;
use taful_core::profile::Profile;
use taful_core::range::HandClass;

use crate::preflop::{Lookup, NodeId, PreflopChart};
use cfr::Solver;
use equity::{ClassTables, N, class_sizes};
use manifest::Manifest;
use tree::{Decision, TNode, Tree};

/// What a seat plays while another seat's best response is computed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeatPlay<'a> {
    /// The seat being best-responded for: nothing is locked at its nodes
    /// (its reach follows the reference).
    Responder,
    /// The reference strategy.
    Reference,
    /// A bot profile's transform of the reference strategy (spec §9).
    Profile(&'a Profile),
}

/// The reference data at one decision node, class-major (`169 × actions`):
/// strategy rows, EV rows in bb, and the actor's range weights.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeData {
    pub strategy: Vec<f32>,
    pub ev: Vec<f32>,
    pub weights: Vec<f32>,
}

impl NodeData {
    /// Chart rows at `id` (empty rows for classes without data).
    pub fn of_chart(chart: &PreflopChart, id: NodeId) -> NodeData {
        let n = chart.node(id);
        let k = n.actions.len();
        let scale = chart.file().header.ev_scale;
        let mut strategy = Vec::with_capacity(N * k);
        let mut ev = Vec::with_capacity(N * k);
        for c in 0..N {
            strategy.extend(n.strategy(c).unwrap_or_else(|| vec![0.0; k]));
            ev.extend(n.ev(c, scale).unwrap_or_else(|| vec![0.0; k]));
        }
        NodeData {
            strategy,
            ev,
            weights: chart.acting_range(id).to_vec(),
        }
    }
}

/// Visits every decision node in depth-first order (the order charts store
/// them in) with its index.
pub fn for_each_decision(tree: &mut Tree, mut f: impl FnMut(usize, &mut Decision)) {
    fn visit(node: &mut TNode, next: &mut usize, f: &mut dyn FnMut(usize, &mut Decision)) {
        let TNode::Decision(d) = node else { return };
        let i = *next;
        *next += 1;
        f(i, d);
        for child in &mut d.children {
            visit(child, next, f);
        }
    }
    let mut next = 0;
    visit(&mut tree.root, &mut next, &mut f);
}

/// Locks every non-responding seat's strategy (`Decision::locked`, spec
/// §9): the reference rows for a reference seat, the profile's transform
/// of them for a profiled seat. Every node also gets the reference rows
/// (`Decision::reference`) for the responder's reach. `data(i, d)` gives
/// the published reference data at decision `i` (depth-first index).
pub fn lock_seats(
    tree: &mut Tree,
    seats: &[SeatPlay],
    mut data: impl FnMut(usize, &Decision) -> NodeData,
) {
    assert_eq!(seats.len(), tree.seats, "one entry per seat");
    let transpose = |rows: &[f32], k: usize| {
        let mut out = vec![0.0; k * N];
        for h in 0..N {
            for a in 0..k {
                out[a * N + h] = rows[h * k + a];
            }
        }
        out
    };
    for_each_decision(tree, |i, d| {
        let k = d.actions.len();
        let nd = data(i, d);
        d.locked = match seats[d.seat] {
            SeatPlay::Responder => Vec::new(),
            SeatPlay::Reference => transpose(&nd.strategy, k),
            SeatPlay::Profile(p) => transpose(
                &p.preflop_strategy(&d.actions, &nd.strategy, &nd.ev, &nd.weights),
                k,
            ),
        };
        d.reference = transpose(&nd.strategy, k);
    });
}

/// Per tree decision (depth-first): its node in `chart`, found by line in
/// the unpruned chart `full` (which has every decision, in this order).
pub fn chart_ids(chart: &PreflopChart, full: &PreflopChart) -> Vec<Option<NodeId>> {
    (0..full.len())
        .map(|i| match chart.find(&full.line(i)) {
            Lookup::Node(id) => Some(id),
            _ => None,
        })
        .collect()
}

/// [`lock_seats`] on the published data: the chart's rows where the chart
/// has the node (`ids`, from [`chart_ids`]), the unpruned chart's elsewhere.
/// The build tool locks the shipped best-response charts this way and the
/// app its mixed tables, so both walk the same game.
pub fn lock_published(
    tree: &mut Tree,
    chart: &PreflopChart,
    full: &PreflopChart,
    ids: &[Option<NodeId>],
    seats: &[SeatPlay],
) {
    lock_seats(tree, seats, |i, _| match ids[i] {
        Some(id) => NodeData::of_chart(chart, id),
        None => NodeData::of_chart(full, i),
    });
}

/// Hero's best-response EVs against one table (spec §9, #29): for every
/// node of the chart where hero acts, `169 × actions` EVs in bb.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableBestResponse {
    evs: HashMap<NodeId, Vec<f32>>,
}

impl TableBestResponse {
    /// Hero's EV of each action at chart node `id` for `hand` (`None` at a
    /// node where hero does not act).
    pub fn ev(&self, id: NodeId, hand: Hand) -> Option<Vec<f32>> {
        let rows = self.evs.get(&id)?;
        let k = rows.len() / N;
        let c = HandClass::of(hand).index();
        Some(rows[c * k..(c + 1) * k].to_vec())
    }

    /// Chart nodes covered.
    pub fn len(&self) -> usize {
        self.evs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.evs.is_empty()
    }
}

/// The preflop game held by the app for on-demand best responses against
/// mixed tables (#29). Built once from the manifest, the class tables, the
/// reference chart and the unpruned reference chart (`preflop-full.tafl`,
/// every node of the tree). Profiles are locked on the published data: the
/// chart's where the chart has the node, the unpruned chart's elsewhere,
/// exactly like the build tool does for the shipped best-response charts,
/// so a table where every bot plays one profile reproduces that chart.
pub struct TableGame {
    tree: Tree,
    chart: std::sync::Arc<PreflopChart>,
    full: PreflopChart,
    /// Per tree decision (depth-first): its node in the reference chart.
    chart_ids: Vec<Option<NodeId>>,
}

impl std::fmt::Debug for TableGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableGame")
            .field("decisions", &self.tree.decisions)
            .field("chart_nodes", &self.chart.len())
            .finish()
    }
}

impl TableGame {
    /// Builds the tree and checks that both charts sit on it.
    pub fn new(
        manifest: &Manifest,
        tables: ClassTables,
        chart: std::sync::Arc<PreflopChart>,
        full: PreflopChart,
    ) -> Result<TableGame, String> {
        if full.file().header.manifest_hash != manifest.hash()
            || chart.file().header.manifest_hash != manifest.hash()
        {
            return Err("the charts were not built from this manifest".into());
        }
        let mut tree = Tree::build(manifest, tables);
        if full.len() != tree.decisions {
            return Err(format!(
                "unpruned chart has {} nodes, the tree {}",
                full.len(),
                tree.decisions
            ));
        }
        let chart_ids = self::chart_ids(&chart, &full);
        let mut bad = None;
        for_each_decision(&mut tree, |i, d| {
            let n = full.node(i);
            let same = n.player == d.position
                && n.actions.len() == d.actions.len()
                && n.actions
                    .iter()
                    .zip(&d.actions)
                    .all(|(a, b)| a.action == *b);
            if !same && bad.is_none() {
                bad = Some(i);
            }
            // A profiled best response reads only the locks, the reference
            // rows and its own EVs: the solver's tables are dropped.
            d.regrets = Vec::new();
            d.strategy_sum = Vec::new();
            d.ev = Vec::new();
        });
        if let Some(i) = bad {
            return Err(format!("unpruned chart node {i} differs from the tree"));
        }
        Ok(TableGame {
            tree,
            chart,
            full,
            chart_ids,
        })
    }

    /// Seats at the table.
    pub fn seats(&self) -> usize {
        self.tree.seats
    }

    /// Hero's best response at `hero` against every other seat playing its
    /// entry of `seats` (hero's own entry is ignored). Seats are the tree's:
    /// the button is seat 0.
    pub fn best_response(&mut self, hero: Seat, seats: &[SeatPlay]) -> TableBestResponse {
        let mut plays = seats.to_vec();
        plays[hero] = SeatPlay::Responder;
        let ids = &self.chart_ids;
        lock_published(&mut self.tree, &self.chart, &self.full, ids, &plays);
        Solver {
            tree: &mut self.tree,
        }
        .best_response_for(hero);
        let mut evs = HashMap::new();
        for_each_decision(&mut self.tree, |i, d| {
            if d.seat != hero {
                return;
            }
            let Some(id) = ids[i] else { return };
            let k = d.actions.len();
            let mut rows = vec![0.0; N * k];
            for h in 0..N {
                for a in 0..k {
                    rows[h * k + a] = d.br_ev[a * N + h] / BB_CHIPS as f32;
                }
            }
            evs.insert(id, rows);
        });
        TableBestResponse { evs }
    }
}

/// Combos per class, for range weights.
pub fn combos_per_class() -> [f32; N] {
    class_sizes()
}

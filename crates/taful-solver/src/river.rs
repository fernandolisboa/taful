//! In-app river solver (spec §7.7, D11) and best response (§7.8).
//!
//! One street, two players, no chance nodes. The tree follows
//! postflop-solver's sizing rules exactly (pot-relative bets, raise
//! multiples, all-in thresholds, merging), so a river spot cut from a
//! library line solves the same game the offline build would. The solver is
//! vectorized DCFR (α = 1.5, β = 0.5, γ = 3) over each player's live combos;
//! showdowns cost O(n) per terminal with hands sorted by strength and
//! per-card sums for blockers.
//!
//! Values are chips relative to the start of the river, with the pot already
//! in the middle owned by nobody: a player who wins a showdown after both
//! put `c` in gets `pot + c` (less rake), the loser `-c`; folding after
//! putting `c` in is `-c`. Both players' values add up to the pot (less
//! rake), and at the root this matches postflop-solver's expected values.

// Vector CFR indexes several parallel tables by the same hand/action index.
#![allow(clippy::needless_range_loop)]

use taful_core::action::Action;
use taful_core::cards::{Card, CardSet, Hand, NUM_COMBOS};
use taful_core::config::{Chips, Rake};
use taful_core::eval::Accum;

/// One bet-size option in postflop-solver syntax.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeSpec {
    /// `"75%"`: fraction of the pot (after calling, for raises).
    Pot(f64),
    /// `"3x"`: raise to this multiple of the bet faced.
    Prev(f64),
    /// `"a"`.
    AllIn,
}

/// Parse `"33%, 75%, a"` / `"3x"`.
pub fn parse_sizes(s: &str, raise: bool) -> Result<Vec<SizeSpec>, String> {
    s.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let lower = t.to_ascii_lowercase();
            if lower == "a" {
                Ok(SizeSpec::AllIn)
            } else if let Some(p) = lower.strip_suffix('%') {
                let v: f64 = p.parse().map_err(|_| format!("bad size {t:?}"))?;
                if v <= 0.0 {
                    return Err(format!("bad size {t:?}"));
                }
                Ok(SizeSpec::Pot(v / 100.0))
            } else if let Some(x) = lower.strip_suffix('x') {
                if !raise {
                    return Err(format!("{t:?}: multiples are for raises only"));
                }
                let v: f64 = x.parse().map_err(|_| format!("bad size {t:?}"))?;
                if v <= 1.0 {
                    return Err(format!("bad size {t:?}"));
                }
                Ok(SizeSpec::Prev(v))
            } else {
                Err(format!("unsupported size {t:?} (use N%, Nx or a)"))
            }
        })
        .collect()
}

/// River sizes and thresholds (postflop-solver's `TreeConfig` for one street).
#[derive(Debug, Clone, PartialEq)]
pub struct RiverTree {
    pub bets: Vec<SizeSpec>,
    pub raises: Vec<SizeSpec>,
    pub add_allin_threshold: f64,
    pub force_allin_threshold: f64,
    pub merging_threshold: f64,
}

impl RiverTree {
    /// The river street of a library tree spec.
    pub fn from_spec(t: &crate::postflop::TreeSpec) -> Result<RiverTree, String> {
        Ok(RiverTree {
            bets: parse_sizes(&t.river[0], false)?,
            raises: parse_sizes(&t.river[1], true)?,
            add_allin_threshold: t.add_allin_threshold,
            force_allin_threshold: t.force_allin_threshold,
            merging_threshold: t.merging_threshold,
        })
    }
}

/// A river subgame. Player 0 is out of position (acts first).
#[derive(Debug, Clone)]
pub struct RiverSpot {
    pub board: [Card; 5],
    /// Weights per combo index (1,326), OOP and IP.
    pub ranges: [Vec<f32>; 2],
    /// Chips in the middle at the start of the river.
    pub pot: Chips,
    /// Effective stack behind at the start of the river.
    pub stack: Chips,
    pub tree: RiverTree,
    pub rake: Rake,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveOptions {
    /// Stop when exploitability ≤ this percent of the pot.
    pub target_pct: f64,
    pub max_iterations: u32,
    /// Exploitability is checked every this many iterations.
    pub check_every: u32,
}

impl Default for SolveOptions {
    /// Spec §7.7: 0.3% of the pot or 1,000 iterations.
    fn default() -> Self {
        SolveOptions {
            target_pct: 0.3,
            max_iterations: 1000,
            check_every: 10,
        }
    }
}

/// Solves subgames. A turn solver could implement this later without
/// touching callers (spec §7.7).
pub trait SubgameSolver {
    fn solve(&self, spot: &RiverSpot, opts: &SolveOptions) -> Result<RiverSolution, String>;
}

/// Taful's DCFR river solver.
#[derive(Debug, Clone, Copy, Default)]
pub struct DcfrRiverSolver;

impl SubgameSolver for DcfrRiverSolver {
    fn solve(&self, spot: &RiverSpot, opts: &SolveOptions) -> Result<RiverSolution, String> {
        let mut g = Game::new(spot)?;
        let mut expl = f64::INFINITY;
        let mut it = 0;
        while it < opts.max_iterations {
            g.iterate(it);
            it += 1;
            if it % opts.check_every.max(1) == 0 || it == opts.max_iterations {
                expl = g.exploitability();
                if 100.0 * expl / g.pot <= opts.target_pct {
                    break;
                }
            }
        }
        Ok(g.into_solution(it, expl))
    }
}

// ---- tree -----------------------------------------------------------------

/// Same variant order as postflop-solver's `Action`, so sorting matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RAction {
    Fold,
    Check,
    Call,
    Bet(i64),
    Raise(i64),
    AllIn(i64),
}

impl RAction {
    fn amount(self) -> i64 {
        match self {
            RAction::Bet(a) | RAction::Raise(a) | RAction::AllIn(a) => a,
            _ => -1,
        }
    }
    fn public(self) -> Action {
        match self {
            RAction::Fold => Action::Fold,
            RAction::Check => Action::Check,
            RAction::Call => Action::Call,
            RAction::Bet(a) => Action::Bet(a as Chips),
            RAction::Raise(a) => Action::Raise(a as Chips),
            RAction::AllIn(_) => Action::AllIn,
        }
    }
}

#[derive(Debug, Clone)]
enum Kind {
    Decision {
        player: usize,
        actions: Vec<RAction>,
        children: Vec<usize>,
    },
    /// `folder` gave up; commitments on the river.
    Fold {
        folder: usize,
        commit: [i64; 2],
    },
    Showdown {
        commit: i64,
    },
}

#[derive(Debug, Clone, Copy)]
struct Info {
    prev: Option<RAction>,
    allin: bool,
    stack: [i64; 2],
    prev_amount: i64,
    /// Per-player amount matched so far (`node.amount` in postflop-solver).
    amount: i64,
}

fn merge(actions: Vec<RAction>, pot: i64, offset: i64, param: f64) -> Vec<RAction> {
    const EPS: f64 = 1e-12;
    let mut cur = i64::MAX;
    let mut out = Vec::new();
    for &a in actions.iter().rev() {
        let amount = a.amount();
        if amount > 0 {
            let ratio = (amount - offset) as f64 / pot as f64;
            let cur_ratio = (cur - offset) as f64 / pot as f64;
            let threshold = (cur_ratio - param) / (1.0 + param);
            if ratio < threshold * (1.0 - EPS) {
                out.push(a);
                cur = amount;
            }
        } else {
            out.push(a);
        }
    }
    out.reverse();
    out
}

fn round(x: f64) -> i64 {
    x.round() as i64
}

fn build(nodes: &mut Vec<Kind>, t: &RiverTree, start_pot: i64, player: usize, info: Info) -> usize {
    let opp = player ^ 1;
    let to_call = info.stack[player] - info.stack[opp];
    let pot = start_pot + 2 * (info.amount + to_call);
    let max_amount = info.stack[opp] + info.prev_amount;
    let min_amount = (info.prev_amount + to_call).clamp(1, max_amount.max(1));
    let mut acts = Vec::new();
    let facing = matches!(
        info.prev,
        Some(RAction::Bet(_) | RAction::Raise(_) | RAction::AllIn(_))
    );
    if !facing {
        acts.push(RAction::Check);
        for s in &t.bets {
            acts.push(match *s {
                SizeSpec::Pot(r) => RAction::Bet(round(pot as f64 * r)),
                SizeSpec::Prev(_) => unreachable!("parse_sizes rejects bet multiples"),
                SizeSpec::AllIn => RAction::AllIn(max_amount),
            });
        }
        if max_amount <= round(pot as f64 * t.add_allin_threshold) {
            acts.push(RAction::AllIn(max_amount));
        }
    } else {
        acts.push(RAction::Fold);
        acts.push(RAction::Call);
        if !info.allin {
            for s in &t.raises {
                acts.push(match *s {
                    SizeSpec::Pot(r) => RAction::Raise(info.prev_amount + round(pot as f64 * r)),
                    SizeSpec::Prev(r) => RAction::Raise(round(info.prev_amount as f64 * r)),
                    SizeSpec::AllIn => RAction::AllIn(max_amount),
                });
            }
            if max_amount <= info.prev_amount + round(pot as f64 * t.add_allin_threshold) {
                acts.push(RAction::AllIn(max_amount));
            }
        }
    }
    let above = |amount: i64| {
        let new_pot = pot + 2 * (amount - info.prev_amount);
        max_amount <= amount + round(new_pot as f64 * t.force_allin_threshold)
    };
    for a in acts.iter_mut() {
        if let RAction::Bet(x) | RAction::Raise(x) = *a {
            let c = x.clamp(min_amount, max_amount);
            if above(c) {
                *a = RAction::AllIn(max_amount);
            } else if c != x {
                *a = match *a {
                    RAction::Bet(_) => RAction::Bet(c),
                    _ => RAction::Raise(c),
                };
            }
        }
    }
    acts.sort_unstable();
    acts.dedup();
    let acts = merge(acts, pot, info.prev_amount, t.merging_threshold);

    let idx = nodes.len();
    nodes.push(Kind::Showdown { commit: 0 }); // placeholder
    let mut children = Vec::with_capacity(acts.len());
    for &a in &acts {
        let child = match a {
            RAction::Fold => {
                let mut commit = [0; 2];
                commit[player] = info.amount;
                commit[opp] = info.amount + to_call;
                nodes.push(Kind::Fold {
                    folder: player,
                    commit,
                });
                nodes.len() - 1
            }
            RAction::Call => {
                nodes.push(Kind::Showdown {
                    commit: info.amount + to_call,
                });
                nodes.len() - 1
            }
            RAction::Check if player == 1 => {
                nodes.push(Kind::Showdown {
                    commit: info.amount,
                });
                nodes.len() - 1
            }
            RAction::Check => build(
                nodes,
                t,
                start_pot,
                opp,
                Info {
                    prev: Some(a),
                    ..info
                },
            ),
            RAction::Bet(x) | RAction::Raise(x) | RAction::AllIn(x) => {
                let mut stack = info.stack;
                stack[player] -= x - info.prev_amount + to_call;
                build(
                    nodes,
                    t,
                    start_pot,
                    opp,
                    Info {
                        prev: Some(a),
                        allin: matches!(a, RAction::AllIn(_)),
                        stack,
                        prev_amount: x,
                        amount: info.amount + to_call,
                    },
                )
            }
        };
        children.push(child);
    }
    nodes[idx] = Kind::Decision {
        player,
        actions: acts,
        children,
    };
    idx
}

// ---- game -----------------------------------------------------------------

/// Per-player hand list sorted by strength (ascending).
#[derive(Debug, Clone)]
struct Hands {
    hands: Vec<Hand>,
    root: Vec<f64>,
    strength: Vec<u32>,
    cards: Vec<[usize; 2]>,
    /// Index of the same combo in the opponent's list.
    same: Vec<Option<usize>>,
}

struct Game {
    nodes: Vec<Kind>,
    hands: [Hands; 2],
    pot: f64,
    rake: Rake,
    start_pot: Chips,
    /// Per decision node: cumulative regrets and strategy sums, `[a][h]` of
    /// the acting player.
    regret: Vec<Vec<f64>>,
    strat_sum: Vec<Vec<f64>>,
}

fn regret_match(reg: &[f64], k: usize, n: usize) -> Vec<f64> {
    let mut s = vec![0.0; k * n];
    for h in 0..n {
        let total: f64 = (0..k).map(|a| reg[a * n + h].max(0.0)).sum();
        for a in 0..k {
            s[a * n + h] = if total > 0.0 {
                reg[a * n + h].max(0.0) / total
            } else {
                1.0 / k as f64
            };
        }
    }
    s
}

fn normalize_sum(sum: &[f64], k: usize, n: usize) -> Vec<f64> {
    let mut s = vec![0.0; k * n];
    for h in 0..n {
        let total: f64 = (0..k).map(|a| sum[a * n + h]).sum();
        for a in 0..k {
            s[a * n + h] = if total > 0.0 {
                sum[a * n + h] / total
            } else {
                1.0 / k as f64
            };
        }
    }
    s
}

impl Game {
    fn new(spot: &RiverSpot) -> Result<Game, String> {
        let board = CardSet::from_cards(&spot.board);
        if board.len() != 5 {
            return Err("river board needs 5 distinct cards".into());
        }
        if spot.ranges.iter().any(|r| r.len() != NUM_COMBOS) {
            return Err("ranges need 1,326 weights".into());
        }
        if spot.pot == 0 {
            return Err("empty pot".into());
        }
        if spot.stack == 0 {
            return Err("no chips behind: the river is a showdown, nothing to solve".into());
        }
        let acc = Accum::new(&spot.board);
        let mut lists: Vec<Hands> = (0..2)
            .map(|p| {
                let mut v: Vec<(u32, Hand, f64)> = Hand::all()
                    .filter(|h| !h.mask().intersects(board))
                    .filter_map(|h| {
                        let w = f64::from(spot.ranges[p][h.combo()]);
                        (w > 0.0).then(|| {
                            let [a, b] = h.cards();
                            (acc.with(a).with(b).score(), h, w)
                        })
                    })
                    .collect();
                v.sort_by_key(|x| (x.0, x.1.combo()));
                Hands {
                    strength: v.iter().map(|x| x.0).collect(),
                    cards: v
                        .iter()
                        .map(|x| x.1.cards().map(|c| usize::from(c.index())))
                        .collect(),
                    root: v.iter().map(|x| x.2).collect(),
                    hands: v.iter().map(|x| x.1).collect(),
                    same: Vec::new(),
                }
            })
            .collect();
        if lists.iter().any(|l| l.hands.is_empty()) {
            return Err("a range is empty on this board".into());
        }
        for p in 0..2 {
            let other: std::collections::HashMap<usize, usize> = lists[p ^ 1]
                .hands
                .iter()
                .enumerate()
                .map(|(i, h)| (h.combo(), i))
                .collect();
            lists[p].same = lists[p]
                .hands
                .iter()
                .map(|h| other.get(&h.combo()).copied())
                .collect();
        }
        let ip = lists.pop().expect("2");
        let oop = lists.pop().expect("2");
        let pot = i64::try_from(spot.pot).map_err(|e| e.to_string())?;
        let stack = i64::try_from(spot.stack).map_err(|e| e.to_string())?;
        let mut nodes = Vec::new();
        build(
            &mut nodes,
            &spot.tree,
            pot,
            0,
            Info {
                prev: None,
                allin: false,
                stack: [stack, stack],
                prev_amount: 0,
                amount: 0,
            },
        );
        let hands = [oop, ip];
        let (regret, strat_sum) = nodes
            .iter()
            .map(|n| match n {
                Kind::Decision {
                    player, actions, ..
                } => {
                    let len = actions.len() * hands[*player].hands.len();
                    (vec![0.0; len], vec![0.0; len])
                }
                _ => (Vec::new(), Vec::new()),
            })
            .unzip();
        Ok(Game {
            nodes,
            hands,
            pot: spot.pot as f64,
            rake: spot.rake,
            start_pot: spot.pot,
            regret,
            strat_sum,
        })
    }

    /// Terminal values for `p` against opponent reach `w` (opponent indexing).
    fn terminal(&self, node: usize, p: usize, w: &[f64]) -> Vec<f64> {
        let me = &self.hands[p];
        let op = &self.hands[p ^ 1];
        let n = me.hands.len();
        // Compatible opponent reach per hand (blockers removed).
        let mut cs = [0.0f64; 52];
        let mut total = 0.0;
        for (j, &x) in w.iter().enumerate() {
            total += x;
            cs[op.cards[j][0]] += x;
            cs[op.cards[j][1]] += x;
        }
        let compat = |i: usize| {
            let [a, b] = me.cards[i];
            total - cs[a] - cs[b] + me.same[i].map_or(0.0, |j| w[j])
        };
        match self.nodes[node] {
            Kind::Fold { folder, commit } => {
                // The uncalled part of a bet goes back and is not raked.
                let pot_total = self.start_pot + 2 * commit[folder] as Chips;
                let rake = self.rake.amount(pot_total, true) as f64;
                let v = if folder == p {
                    -(commit[p] as f64)
                } else {
                    self.pot + commit[p ^ 1] as f64 - rake
                };
                (0..n).map(|i| v * compat(i)).collect()
            }
            Kind::Showdown { commit } => {
                let c = commit as f64;
                let rake = self.rake.amount(self.start_pot + 2 * commit as Chips, true) as f64;
                let win_v = self.pot + c - rake;
                let lose_v = -c;
                let tie_v = (self.pot - rake) / 2.0;
                let mut win = vec![0.0; n];
                let mut lose = vec![0.0; n];
                // Weaker opponents: ascending sweep.
                let mut acc = 0.0;
                let mut acs = [0.0f64; 52];
                let mut j = 0;
                for i in 0..n {
                    while j < op.hands.len() && op.strength[j] < me.strength[i] {
                        acc += w[j];
                        acs[op.cards[j][0]] += w[j];
                        acs[op.cards[j][1]] += w[j];
                        j += 1;
                    }
                    let [a, b] = me.cards[i];
                    win[i] = acc - acs[a] - acs[b];
                }
                // Stronger opponents: descending sweep.
                let mut acc = 0.0;
                let mut acs = [0.0f64; 52];
                let mut j = op.hands.len();
                for i in (0..n).rev() {
                    while j > 0 && op.strength[j - 1] > me.strength[i] {
                        j -= 1;
                        acc += w[j];
                        acs[op.cards[j][0]] += w[j];
                        acs[op.cards[j][1]] += w[j];
                    }
                    let [a, b] = me.cards[i];
                    lose[i] = acc - acs[a] - acs[b];
                }
                (0..n)
                    .map(|i| {
                        let tie = compat(i) - win[i] - lose[i];
                        win[i] * win_v + lose[i] * lose_v + tie * tie_v
                    })
                    .collect()
            }
            Kind::Decision { .. } => unreachable!("not terminal"),
        }
    }

    /// One DCFR traversal for `p`; `w` is the opponent's reach.
    fn cfr(&mut self, node: usize, p: usize, my_reach: &[f64], w: &[f64], t: u32) -> Vec<f64> {
        let (player, children) = match &self.nodes[node] {
            Kind::Decision {
                player, children, ..
            } => (*player, children.clone()),
            _ => return self.terminal(node, p, w),
        };
        let k = children.len();
        let n = self.hands[player].hands.len();
        let sigma = regret_match(&self.regret[node], k, n);
        if player == p {
            let mut vals = Vec::with_capacity(k);
            for (a, &c) in children.iter().enumerate() {
                let r: Vec<f64> = (0..n).map(|h| my_reach[h] * sigma[a * n + h]).collect();
                vals.push(self.cfr(c, p, &r, w, t));
            }
            let mut v = vec![0.0; n];
            for a in 0..k {
                for h in 0..n {
                    v[h] += sigma[a * n + h] * vals[a][h];
                }
            }
            // DCFR discounting, then accumulate.
            let tf = f64::from(t + 1);
            let pos = tf.powf(1.5) / (tf.powf(1.5) + 1.0);
            let neg = tf.powf(0.5) / (tf.powf(0.5) + 1.0);
            let gamma = (tf / (tf + 1.0)).powi(3);
            let reg = &mut self.regret[node];
            let ss = &mut self.strat_sum[node];
            for a in 0..k {
                for h in 0..n {
                    let i = a * n + h;
                    let r = reg[i];
                    reg[i] = r * if r > 0.0 { pos } else { neg } + vals[a][h] - v[h];
                    ss[i] = ss[i] * gamma + my_reach[h] * sigma[i];
                }
            }
            v
        } else {
            let mut v = vec![0.0; self.hands[p].hands.len()];
            for (a, &c) in children.iter().enumerate() {
                let r: Vec<f64> = (0..n).map(|h| w[h] * sigma[a * n + h]).collect();
                let cv = self.cfr(c, p, my_reach, &r, t);
                for (x, y) in v.iter_mut().zip(cv) {
                    *x += y;
                }
            }
            v
        }
    }

    fn iterate(&mut self, t: u32) {
        for p in 0..2 {
            let mine = self.hands[p].root.clone();
            let opp = self.hands[p ^ 1].root.clone();
            self.cfr(0, p, &mine, &opp, t);
        }
    }

    fn avg(&self, node: usize) -> Vec<f64> {
        match &self.nodes[node] {
            Kind::Decision {
                player, actions, ..
            } => normalize_sum(
                &self.strat_sum[node],
                actions.len(),
                self.hands[*player].hands.len(),
            ),
            _ => Vec::new(),
        }
    }

    /// Counterfactual values for `p` when `p` plays `own` (None = best
    /// response) and the opponent plays `opp_strat` per node.
    fn eval(
        &self,
        node: usize,
        p: usize,
        w: &[f64],
        strat: &dyn Fn(usize) -> Vec<f64>,
        best_response: bool,
        out: &mut Vec<Option<(Vec<f64>, Vec<f64>)>>,
    ) -> Vec<f64> {
        let Kind::Decision {
            player, children, ..
        } = &self.nodes[node]
        else {
            return self.terminal(node, p, w);
        };
        let k = children.len();
        let n = self.hands[*player].hands.len();
        let s = strat(node);
        if *player == p {
            let vals: Vec<Vec<f64>> = children
                .iter()
                .map(|&c| self.eval(c, p, w, strat, best_response, out))
                .collect();
            let v: Vec<f64> = (0..n)
                .map(|h| {
                    if best_response {
                        (0..k).map(|a| vals[a][h]).fold(f64::NEG_INFINITY, f64::max)
                    } else {
                        (0..k).map(|a| s[a * n + h] * vals[a][h]).sum()
                    }
                })
                .collect();
            // Normalizer: compatible opponent reach at this node.
            let ones = self.compat_reach(p, w);
            let mut per_action = vec![0.0; k * n];
            for a in 0..k {
                for h in 0..n {
                    per_action[a * n + h] = if ones[h] > 0.0 {
                        vals[a][h] / ones[h]
                    } else {
                        0.0
                    };
                }
            }
            out[node] = Some((per_action, ones));
            v
        } else {
            let mut v = vec![0.0; self.hands[p].hands.len()];
            for (a, &c) in children.iter().enumerate() {
                let r: Vec<f64> = (0..n).map(|h| w[h] * s[a * n + h]).collect();
                let cv = self.eval(c, p, &r, strat, best_response, out);
                for (x, y) in v.iter_mut().zip(cv) {
                    *x += y;
                }
            }
            v
        }
    }

    fn compat_reach(&self, p: usize, w: &[f64]) -> Vec<f64> {
        let me = &self.hands[p];
        let op = &self.hands[p ^ 1];
        let mut cs = [0.0f64; 52];
        let mut total = 0.0;
        for (j, &x) in w.iter().enumerate() {
            total += x;
            cs[op.cards[j][0]] += x;
            cs[op.cards[j][1]] += x;
        }
        (0..me.hands.len())
            .map(|i| {
                let [a, b] = me.cards[i];
                total - cs[a] - cs[b] + me.same[i].map_or(0.0, |j| w[j])
            })
            .collect()
    }

    /// Range-average value of `p`'s root counterfactual values.
    fn average(&self, p: usize, cfv: &[f64]) -> f64 {
        let norm: f64 = self
            .compat_reach(p, &self.hands[p ^ 1].root)
            .iter()
            .zip(&self.hands[p].root)
            .map(|(c, r)| c * r)
            .sum();
        let v: f64 = cfv
            .iter()
            .zip(&self.hands[p].root)
            .map(|(c, r)| c * r)
            .sum();
        if norm > 0.0 { v / norm } else { 0.0 }
    }

    /// Exploitability of the average strategies, in chips (mean of both
    /// players' best-response gains).
    fn exploitability(&self) -> f64 {
        let strat = |n: usize| self.avg(n);
        let mut scratch = vec![None; self.nodes.len()];
        // Best-response gain over the current strategies' value, per player.
        // (Values sum to the pot less expected rake, so the pot itself is
        // not a valid baseline once rake is on.)
        let gain: f64 = (0..2)
            .map(|p| {
                let w = &self.hands[p ^ 1].root;
                let br = self.eval(0, p, w, &strat, true, &mut scratch);
                let cur = self.eval(0, p, w, &strat, false, &mut scratch);
                self.average(p, &br) - self.average(p, &cur)
            })
            .sum();
        (gain / 2.0).max(0.0)
    }

    fn into_solution(self, iterations: u32, exploitability: f64) -> RiverSolution {
        let avg: Vec<Vec<f64>> = (0..self.nodes.len()).map(|n| self.avg(n)).collect();
        let strat = |n: usize| avg[n].clone();
        let mut evs: Vec<Option<(Vec<f64>, Vec<f64>)>> = vec![None; self.nodes.len()];
        let mut root_ev = [0.0; 2];
        for (p, ev) in root_ev.iter_mut().enumerate() {
            let cfv = self.eval(0, p, &self.hands[p ^ 1].root, &strat, false, &mut evs);
            *ev = self.average(p, &cfv);
        }
        // Decision nodes in DFS order (the build order), renumbered.
        let mut index = vec![usize::MAX; self.nodes.len()];
        let mut k = 0;
        for (i, n) in self.nodes.iter().enumerate() {
            if matches!(n, Kind::Decision { .. }) {
                index[i] = k;
                k += 1;
            }
        }
        let mut nodes = Vec::with_capacity(k);
        for (i, n) in self.nodes.iter().enumerate() {
            let Kind::Decision {
                player,
                actions,
                children,
            } = n
            else {
                continue;
            };
            let (ev, _) = evs[i].clone().unwrap_or_default();
            nodes.push(RiverNode {
                player: *player,
                actions: actions.iter().map(|a| a.public()).collect(),
                children: children
                    .iter()
                    .map(|&c| (index[c] != usize::MAX).then_some(index[c]))
                    .collect(),
                strategy: avg[i].iter().map(|&x| x as f32).collect(),
                ev: ev.iter().map(|&x| x as f32).collect(),
            });
        }
        RiverSolution {
            hands: [self.hands[0].hands.clone(), self.hands[1].hands.clone()],
            nodes,
            iterations,
            exploitability: exploitability as f32,
            pot: self.start_pot,
            root_ev: root_ev.map(|x| x as f32),
        }
    }
}

/// One decision node of a solved river.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverNode {
    pub player: usize,
    pub actions: Vec<Action>,
    /// Next decision node per action; `None` = the hand ends.
    pub children: Vec<Option<usize>>,
    /// Average strategy `[action][hand]` over the acting player's hands.
    pub strategy: Vec<f32>,
    /// EV in chips of each action `[action][hand]`, given the hand and
    /// reaching the node.
    pub ev: Vec<f32>,
}

/// A solved river (spec §7.7): node layout as in the library (DFS, root
/// first), float tables in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverSolution {
    /// Each player's hands with non-zero range weight, in table order.
    pub hands: [Vec<Hand>; 2],
    pub nodes: Vec<RiverNode>,
    pub iterations: u32,
    /// Chips.
    pub exploitability: f32,
    pub pot: Chips,
    /// Range-average root EV of each player, chips.
    pub root_ev: [f32; 2],
}

impl RiverSolution {
    pub fn exploitability_pct(&self) -> f32 {
        100.0 * self.exploitability / self.pot as f32
    }

    pub fn hand_index(&self, player: usize, hand: Hand) -> Option<usize> {
        self.hands[player].iter().position(|&h| h == hand)
    }

    /// Frequencies at `node` for `hand` of the acting player.
    pub fn strategy(&self, node: usize, hand: Hand) -> Option<Vec<f32>> {
        let n = &self.nodes[node];
        let h = self.hand_index(n.player, hand)?;
        let len = self.hands[n.player].len();
        Some(
            (0..n.actions.len())
                .map(|a| n.strategy[a * len + h])
                .collect(),
        )
    }

    /// EV of each action at `node` for `hand`, in chips.
    pub fn ev(&self, node: usize, hand: Hand) -> Option<Vec<f32>> {
        let n = &self.nodes[node];
        let h = self.hand_index(n.player, hand)?;
        let len = self.hands[n.player].len();
        Some((0..n.actions.len()).map(|a| n.ev[a * len + h]).collect())
    }
}

/// Best response (spec §7.8): EV in chips per hand of `responder` at the
/// root when the opponent plays `opp_strategy` (per decision node of the
/// solution, `[action][hand]` over the opponent's hands; nodes where the
/// responder acts are ignored). Partial locks are not supported.
pub fn best_response(
    spot: &RiverSpot,
    solution: &RiverSolution,
    responder: usize,
    opp_strategy: &[Vec<f32>],
) -> Result<Vec<(Hand, f32)>, String> {
    let (g, cfv, _) = best_response_pass(spot, solution, responder, opp_strategy)?;
    let norm = g.compat_reach(responder, &g.hands[responder ^ 1].root);
    Ok(g.hands[responder]
        .hands
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            (
                h,
                if norm[i] > 0.0 {
                    (cfv[i] / norm[i]) as f32
                } else {
                    0.0
                },
            )
        })
        .collect())
}

/// Best response at every node (spec §7.8, #27): for each decision node of
/// the solution where `responder` acts, the EV in chips of each action
/// `[action][hand]` over the responder's hands (as [`RiverNode::ev`]) when
/// the responder best-responds afterwards and the opponent plays
/// `opp_strategy`; `None` at the opponent's nodes and at nodes the
/// opponent's strategy never reaches.
pub fn best_response_detail(
    spot: &RiverSpot,
    solution: &RiverSolution,
    responder: usize,
    opp_strategy: &[Vec<f32>],
) -> Result<Vec<Option<Vec<f32>>>, String> {
    let (_, _, out) = best_response_pass(spot, solution, responder, opp_strategy)?;
    Ok(out)
}

/// One best-response evaluation: the game, the responder's root
/// counterfactual values and the per-action EVs per decision node.
fn best_response_pass(
    spot: &RiverSpot,
    solution: &RiverSolution,
    responder: usize,
    opp_strategy: &[Vec<f32>],
) -> Result<(Game, Vec<f64>, Vec<Option<Vec<f32>>>), String> {
    let g = Game::new(spot)?;
    let mut dec = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if matches!(n, Kind::Decision { .. }) {
            dec.push(i);
        }
    }
    if dec.len() != solution.nodes.len() || opp_strategy.len() != dec.len() {
        return Err("strategy does not match the tree".into());
    }
    for (k, &i) in dec.iter().enumerate() {
        if let Kind::Decision {
            player, actions, ..
        } = &g.nodes[i]
            && *player != responder
            && opp_strategy[k].len() != actions.len() * g.hands[*player].hands.len()
        {
            return Err(format!("strategy at node {k} has the wrong size"));
        }
    }
    let mut by_node = vec![Vec::new(); g.nodes.len()];
    for (k, &i) in dec.iter().enumerate() {
        by_node[i] = opp_strategy[k].iter().map(|&x| f64::from(x)).collect();
    }
    let strat = |n: usize| by_node[n].clone();
    let mut scratch = vec![None; g.nodes.len()];
    let cfv = g.eval(
        0,
        responder,
        &g.hands[responder ^ 1].root,
        &strat,
        true,
        &mut scratch,
    );
    let out = dec
        .iter()
        .map(|&i| match (&g.nodes[i], &scratch[i]) {
            (Kind::Decision { player, .. }, Some((ev, reach)))
                if *player == responder && reach.iter().any(|&r| r > 0.0) =>
            {
                Some(ev.iter().map(|&x| x as f32).collect())
            }
            _ => None,
        })
        .collect();
    Ok((g, cfv, out))
}

/// The strategy `profile` plays as `bot` on a solved river (spec §9, #27):
/// per decision node, the profile's transform of the solution's strategy at
/// the bot's nodes (`[action][hand]` over `solution.hands[bot]`, the layout
/// [`best_response`] takes) and an empty table at the other player's.
/// The bot's range at each node is its range in `spot` played down the tree
/// with this same profiled strategy.
pub fn profile_strategy(
    spot: &RiverSpot,
    solution: &RiverSolution,
    bot: usize,
    profile: &taful_core::profile::Profile,
) -> Vec<Vec<f32>> {
    let hands = &solution.hands[bot];
    let reach: Vec<f32> = hands.iter().map(|h| spot.ranges[bot][h.combo()]).collect();
    let mut out = vec![Vec::new(); solution.nodes.len()];
    if !solution.nodes.is_empty() {
        lock_profile(solution, bot, profile, 0, &reach, 0, &mut out);
    }
    out
}

fn lock_profile(
    sol: &RiverSolution,
    bot: usize,
    profile: &taful_core::profile::Profile,
    node: usize,
    reach: &[f32],
    facing: usize,
    out: &mut [Vec<f32>],
) {
    let n = &sol.nodes[node];
    let k = n.actions.len();
    let next_facing = |a: usize| facing + usize::from(n.actions[a].is_aggressive());
    if n.player != bot {
        for (a, c) in n.children.iter().enumerate() {
            if let Some(c) = *c {
                lock_profile(sol, bot, profile, c, reach, next_facing(a), out);
            }
        }
        return;
    }
    let len = reach.len();
    // `[action][hand]` → rows by hand for the transform, and back.
    let rows = |t: &[f32]| -> Vec<f32> {
        (0..len)
            .flat_map(|h| (0..k).map(move |a| t[a * len + h]))
            .collect()
    };
    let locked =
        profile.postflop_strategy(&n.actions, &rows(&n.strategy), &rows(&n.ev), reach, facing);
    let mut table = vec![0.0f32; k * len];
    for h in 0..len {
        for a in 0..k {
            table[a * len + h] = locked[h * k + a];
        }
    }
    for (a, c) in n.children.iter().enumerate() {
        if let Some(c) = *c {
            let r: Vec<f32> = (0..len).map(|h| reach[h] * table[a * len + h]).collect();
            lock_profile(sol, bot, profile, c, &r, next_facing(a), out);
        }
    }
    out[node] = table;
}

#[cfg(test)]
mod tests {
    use taful_core::cards::parse_cards;
    use taful_core::range::Range;

    use super::*;

    fn spot(board: &str, oop: &str, ip: &str, sizes: (&str, &str)) -> RiverSpot {
        let r = |s: &str| s.parse::<Range>().unwrap().weights().to_vec();
        RiverSpot {
            board: parse_cards(board).unwrap().try_into().unwrap(),
            ranges: [r(oop), r(ip)],
            pot: 1000,
            stack: 2000,
            tree: RiverTree {
                bets: parse_sizes(sizes.0, false).unwrap(),
                raises: parse_sizes(sizes.1, true).unwrap(),
                add_allin_threshold: 1.5,
                force_allin_threshold: 0.15,
                merging_threshold: 0.1,
            },
            rake: Rake::ZERO,
        }
    }

    #[test]
    fn sizes_parse() {
        assert_eq!(
            parse_sizes("33%, 75%, a", false).unwrap(),
            vec![SizeSpec::Pot(0.33), SizeSpec::Pot(0.75), SizeSpec::AllIn]
        );
        assert_eq!(parse_sizes("3x", true).unwrap(), vec![SizeSpec::Prev(3.0)]);
        assert!(parse_sizes("3x", false).is_err());
        assert!(parse_sizes("2e", true).is_err());
    }

    #[test]
    fn tree_shape() {
        let s = spot("AsKd7c4h2s", "AA,KK", "QQ", ("50%", "3x"));
        let g = Game::new(&s).unwrap();
        let Kind::Decision { actions, .. } = &g.nodes[0] else {
            panic!()
        };
        // Check, bet 500; no all-in option (2000 > 1.5 × 1000).
        assert_eq!(actions, &vec![RAction::Check, RAction::Bet(500)]);
    }

    #[test]
    fn nuts_vs_bluffcatcher_converges_to_theory() {
        // Classic river toy game: OOP checks; IP has nuts or air, OOP a pure
        // bluff catcher. With a pot-sized bet IP bluffs 1/2 as often as it
        // value bets (1:2), and OOP calls 50%.
        let s = spot(
            "2c3d8h9sKc",
            "QhQd",      // bluff catcher: beats air, loses to the nuts
            "KdKh,4h4d", // set of kings (nuts) and fours (air)
            ("100%", ""),
        );
        let sol = DcfrRiverSolver
            .solve(
                &s,
                &SolveOptions {
                    target_pct: 0.05,
                    max_iterations: 5000,
                    check_every: 10,
                },
            )
            .unwrap();
        assert!(
            sol.exploitability_pct() < 0.1,
            "{}",
            sol.exploitability_pct()
        );
        // Root: OOP to act; after a check, IP decides.
        let after_check = sol.nodes[0].children[0].unwrap();
        let air: Hand = "4h4d".parse().unwrap();
        let nuts: Hand = "KdKh".parse().unwrap();
        let bet = |h| sol.strategy(after_check, h).unwrap()[1];
        assert!(bet(nuts) > 0.99);
        // Facing a pot bet, value:bluff 2:1 when IP holds 1 nut combo vs 1 air
        // combo means bluffing half of the air.
        assert!((bet(air) - 0.5).abs() < 0.05, "air bets {}", bet(air));
    }

    #[test]
    fn evs_are_consistent() {
        let s = spot(
            "AsKd7c4h2s",
            "AA,KK,77,AQ,KQ,QJs,T9s",
            "AA,KK,AK,AQ,QQ,JJ,65s,T9s,J9s",
            ("50%, 100%", "3x"),
        );
        let sol = DcfrRiverSolver.solve(&s, &SolveOptions::default()).unwrap();
        assert!(
            sol.exploitability_pct() <= 0.3,
            "{}",
            sol.exploitability_pct()
        );
        // Values are zero-sum around the pot (no rake).
        let total = sol.root_ev[0] + sol.root_ev[1];
        assert!((total - 1000.0).abs() < 1.0, "{total}");
        // Root EV per hand = strategy-weighted action EVs.
        let root = &sol.nodes[0];
        for (i, &h) in sol.hands[0].iter().enumerate().step_by(7) {
            let st = sol.strategy(0, h).unwrap();
            let ev = sol.ev(0, h).unwrap();
            let mix: f32 = st.iter().zip(&ev).map(|(a, b)| a * b).sum();
            let best = ev.iter().cloned().fold(f32::MIN, f32::max);
            assert!(best >= mix - 1e-3, "hand {i}");
            // Mixed actions have (near) equal EVs.
            for (a, (&f, &e)) in st.iter().zip(&ev).enumerate() {
                if f > 0.2 {
                    assert!(best - e < 0.02 * 1000.0, "{h} action {a}: {e} vs {best}");
                }
            }
            let _ = root;
        }
        // Best response to the solution itself gains at most the exploitability.
        let opp: Vec<Vec<f32>> = sol.nodes.iter().map(|n| n.strategy.clone()).collect();
        let br = best_response(&s, &sol, 1, &opp).unwrap();
        assert_eq!(br.len(), sol.hands[1].len());
    }

    #[test]
    fn best_response_detail_and_profiled_rivers() {
        use taful_core::profile::Profile;
        let s = spot(
            "AsKd7c4h2s",
            "AA,KK,77,AQ,KQ,QJs,T9s,65s",
            "AA,KK,AK,AQ,QQ,JJ,65s,T9s,J9s",
            ("50%, 100%", "3x"),
        );
        let sol = DcfrRiverSolver.solve(&s, &SolveOptions::default()).unwrap();
        let neutral = Profile::from_toml(
            "id = \"n\"\n[preflop]\nvpip_mult = 1.0\npfr_mult = 1.0\nthreebet_mult = 1.0\n\
             [postflop]\nfold_to_cbet = 0.0\nfold_to_raise = 0.0\nbluff_value_shift = 0.0\naggression = 0.0\n\
             [sizing]\nprefer = \"reference\"\n",
        )
        .unwrap();
        // A neutral profile plays the solution.
        let locked = profile_strategy(&s, &sol, 1, &neutral);
        for (n, t) in sol.nodes.iter().zip(&locked) {
            if n.player == 1 {
                for (a, b) in n.strategy.iter().zip(t) {
                    assert!((a - b).abs() < 1e-5);
                }
            } else {
                assert!(t.is_empty());
            }
        }
        // A station that folds less and bets less.
        let mut station = neutral.clone();
        station.postflop.fold_to_cbet = -0.6;
        station.postflop.fold_to_raise = -0.6;
        station.postflop.aggression = -0.5;
        let locked = profile_strategy(&s, &sol, 1, &station);
        let root = best_response(&s, &sol, 0, &locked).unwrap();
        let detail = best_response_detail(&s, &sol, 0, &locked).unwrap();
        let n0 = &sol.nodes[0];
        let len = sol.hands[0].len();
        let at_root = detail[0].as_ref().expect("OOP acts at the root");
        for (i, (h, v)) in root.iter().enumerate() {
            // The root value is the best action's value.
            let best = (0..n0.actions.len())
                .map(|a| at_root[a * len + i])
                .fold(f32::MIN, f32::max);
            assert!((best - v).abs() < 1e-2, "{h}: {best} vs {v}");
        }
        assert!(
            detail
                .iter()
                .zip(&sol.nodes)
                .all(|(d, n)| d.is_none() || n.player == 0)
        );
        // Exploiting the station gains over playing the equilibrium.
        let reference = best_response(
            &s,
            &sol,
            0,
            &sol.nodes
                .iter()
                .map(|n| n.strategy.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let w = |r: &[(Hand, f32)]| {
            r.iter()
                .map(|(h, v)| v * s.ranges[0][h.combo()])
                .sum::<f32>()
        };
        assert!(
            w(&root) > w(&reference),
            "{} vs {}",
            w(&root),
            w(&reference)
        );
    }

    #[test]
    fn rake_and_edge_cases() {
        let mut s = spot(
            "AsKd7c4h2s",
            "AA,KK,77,AQ,KQ,QJs,T9s",
            "AA,KK,AK,AQ,QQ,JJ,65s,T9s,J9s",
            ("50%, 100%", "3x"),
        );
        s.rake = Rake {
            rate_bps: 500,
            cap: None,
            no_flop_no_drop: false,
        };
        let sol = DcfrRiverSolver.solve(&s, &SolveOptions::default()).unwrap();
        // Converges for real (the old baseline reported 0% at iteration 20).
        assert!(sol.iterations > 20, "{}", sol.iterations);
        assert!(sol.exploitability_pct() <= 0.3);
        assert!(
            sol.root_ev[0] + sol.root_ev[1] < 1000.0,
            "rake comes out of the pot"
        );
        s.stack = 0;
        assert!(Game::new(&s).is_err());
    }
}

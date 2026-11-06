//! Vector-form DCFR over the preflop tree (spec §6).
//!
//! Every player's strategy is a `[action][class]` table. A traversal carries
//! each seat's reach per class and, per seat, its *blocked mass*
//! `m[j][h] = Σ_h' reach[j][h'] · compat(h, h')`: how many combos of seat `j`
//! a combo of class `h` can face. Counterfactual values are in chips times
//! the product of the other seats' blocked masses, so dividing by that
//! product gives an EV per hand. Card removal is exact between two players
//! and treated as independent across three or more (spec §6 abstraction).

use rayon::prelude::*;
use taful_core::config::BB_CHIPS;

use super::equity::{N, class_sizes};
use super::tree::{Decision, Outcome, TNode, Terminal, Tree};

type V = [f32; N];

const ZERO: V = [0.0; N];

/// DCFR discounting (Brown & Sandholm 2019, α = 1.5, β = 0, γ = 2).
const ALPHA: f64 = 1.5;
const BETA: f64 = 0.0;
const GAMMA: f64 = 2.0;

/// Below this tree depth, children are walked in parallel.
const PAR_DEPTH: usize = 4;

#[derive(Clone, Copy)]
enum Mode {
    /// One DCFR iteration (1-based).
    Train(u32),
    /// Average strategy; fill per-node EVs and values.
    Eval,
    /// Best response for one seat against everyone else's average strategy.
    BestResponse(usize),
    /// Best response for one seat against every other seat playing its
    /// locked profile strategy (`Decision::locked`); fills `br_ev` at the
    /// seat's nodes.
    Profiled(usize),
}

fn matvec(a: &[f32], x: &V) -> V {
    let mut y = ZERO;
    for (row, out) in a.as_chunks::<N>().0.iter().zip(y.iter_mut()) {
        *out = row.iter().zip(x.iter()).map(|(p, q)| p * q).sum();
    }
    y
}

/// Product of every other seat's blocked mass.
fn others_mass(m: &[V], seat: usize) -> V {
    let mut out = [1.0f32; N];
    for (j, mj) in m.iter().enumerate() {
        if j != seat {
            for (o, x) in out.iter_mut().zip(mj) {
                *o *= x;
            }
        }
    }
    out
}

/// Regret matching; uniform where no regret is positive.
fn current_strategy(regrets: &[f32], k: usize) -> Vec<f32> {
    let mut s = vec![0.0f32; k * N];
    for h in 0..N {
        let total: f32 = (0..k).map(|a| regrets[a * N + h].max(0.0)).sum();
        for a in 0..k {
            s[a * N + h] = if total > 0.0 {
                regrets[a * N + h].max(0.0) / total
            } else {
                1.0 / k as f32
            };
        }
    }
    s
}

/// Normalized strategy sum; falls back to regret matching for classes that
/// never reached the node.
pub fn average_strategy(d: &Decision) -> Vec<f32> {
    let k = d.actions.len();
    let cur = current_strategy(&d.regrets, k);
    let mut s = vec![0.0f32; k * N];
    for h in 0..N {
        let total: f32 = (0..k).map(|a| d.strategy_sum[a * N + h]).sum();
        for a in 0..k {
            s[a * N + h] = if total > 0.0 {
                d.strategy_sum[a * N + h] / total
            } else {
                cur[a * N + h]
            };
        }
    }
    s
}

struct Ctx<'a> {
    compat: &'a [f32],
    sizes: V,
    seats: usize,
}

impl Ctx<'_> {
    fn terminal(&self, t: &Terminal, reach: &[V], m: &[V], only: Option<usize>) -> Vec<V> {
        let mut out = vec![ZERO; self.seats];
        let pot = t.pot as f32;
        match &t.outcome {
            Outcome::Fold { winner } => {
                let w = *winner;
                if only.is_none_or(|p| p == w) {
                    let mm = others_mass(m, w);
                    let gain = pot - t.commits[w] as f32;
                    for (o, x) in out[w].iter_mut().zip(mm) {
                        *o = gain * x;
                    }
                }
                // Folded seats get their value at the fold node.
            }
            Outcome::Showdown { pairs } => {
                let live = &t.live;
                // P_i(h): product over live opponents of the realized share
                // against that opponent's range.
                let shares: Vec<(usize, V, V)> = live
                    .iter()
                    .filter(|&&i| live.len() >= 3 || only.is_none_or(|p| p == i))
                    .map(|&i| {
                        let mm = others_mass(m, i);
                        let mut p = [1.0f32; N];
                        for &k in live.iter().filter(|&&k| k != i) {
                            let w = matvec(pairs[i][k].as_ref().expect("live pair"), &reach[k]);
                            for h in 0..N {
                                p[h] *= if m[k][h] > 0.0 { w[h] / m[k][h] } else { 0.0 };
                            }
                        }
                        (i, p, mm)
                    })
                    .collect();
                // Multiway: renormalize the expected shares to sum to 1.
                let z = if live.len() >= 3 {
                    let z: f32 = shares
                        .iter()
                        .map(|(i, p, mm)| {
                            let (mut num, mut den) = (0.0f32, 0.0f32);
                            for h in 0..N {
                                let w = reach[*i][h] * self.sizes[h] * mm[h];
                                num += w * p[h];
                                den += w;
                            }
                            if den > 0.0 { num / den } else { 0.0 }
                        })
                        .sum();
                    if z > 0.0 { z } else { 1.0 }
                } else {
                    1.0
                };
                for (i, p, mm) in shares {
                    if only.is_some_and(|q| q != i) {
                        continue;
                    }
                    let c = t.commits[i] as f32;
                    for h in 0..N {
                        out[i][h] = mm[h] * (pot * p[h] / z - c);
                    }
                }
            }
        }
        out
    }

    fn walk(&self, node: &mut TNode, reach: &[V], m: &[V], mode: Mode, depth: usize) -> Vec<V> {
        let d = match node {
            TNode::Terminal(t) => {
                let only = match mode {
                    Mode::BestResponse(p) | Mode::Profiled(p) => Some(p),
                    _ => None,
                };
                return self.terminal(t, reach, m, only);
            }
            TNode::Decision(d) => d,
        };
        let p = d.seat;
        let k = d.actions.len();
        let sigma = match mode {
            Mode::Train(_) => current_strategy(&d.regrets, k),
            Mode::Profiled(q) if q != p => {
                assert_eq!(d.locked.len(), k * N, "profile locked at every node");
                d.locked.clone()
            }
            _ => average_strategy(d),
        };
        let br_self = matches!(mode, Mode::BestResponse(q) | Mode::Profiled(q) if q == p);

        // The best responder's own reach follows its average strategy too:
        // its values do not depend on it, but the multiway renormalization
        // does, and it must match the evaluation pass (same payoffs). Against
        // a profile the responder's reach stays the reference's as well.
        let child_inputs = |a: usize| -> (Vec<V>, Vec<V>) {
            let mut r = reach.to_vec();
            let mut mm = m.to_vec();
            for h in 0..N {
                r[p][h] *= sigma[a * N + h];
            }
            mm[p] = matvec(self.compat, &r[p]);
            (r, mm)
        };
        let actions = d.actions.clone();
        let commit = d.commit as f32;
        let fold_value = {
            let mm = others_mass(m, p);
            let mut v = ZERO;
            for h in 0..N {
                v[h] = -commit * mm[h];
            }
            v
        };
        let run = |(a, child): (usize, &mut TNode)| -> Vec<V> {
            let (r, mm) = child_inputs(a);
            let mut v = self.walk(child, &r, &mm, mode, depth + 1);
            if actions[a] == taful_core::action::Action::Fold {
                v[p] = fold_value;
            }
            v
        };
        let child_vals: Vec<Vec<V>> = if depth < PAR_DEPTH {
            d.children.par_iter_mut().enumerate().map(run).collect()
        } else {
            d.children.iter_mut().enumerate().map(run).collect()
        };

        let mut out = vec![ZERO; self.seats];
        for (q, o) in out.iter_mut().enumerate() {
            if q == p {
                continue;
            }
            for cv in &child_vals {
                for h in 0..N {
                    o[h] += cv[q][h];
                }
            }
        }
        if br_self {
            for h in 0..N {
                out[p][h] = (0..k)
                    .map(|a| child_vals[a][p][h])
                    .fold(f32::NEG_INFINITY, f32::max);
            }
        } else {
            for (a, cv) in child_vals.iter().enumerate() {
                for h in 0..N {
                    out[p][h] += sigma[a * N + h] * cv[p][h];
                }
            }
        }

        match mode {
            Mode::Train(t) => {
                let t = f64::from(t);
                let prev = t - 1.0;
                let pos = (prev.powf(ALPHA) / (prev.powf(ALPHA) + 1.0)) as f32;
                let neg = (prev.powf(BETA) / (prev.powf(BETA) + 1.0)) as f32;
                let avg = ((prev / t).powf(GAMMA)) as f32;
                for (a, cv) in child_vals.iter().enumerate() {
                    for h in 0..N {
                        let i = a * N + h;
                        let r = d.regrets[i];
                        let r = if r > 0.0 { r * pos } else { r * neg };
                        d.regrets[i] = r + cv[p][h] - out[p][h];
                        d.strategy_sum[i] = d.strategy_sum[i] * avg + reach[p][h] * sigma[i];
                    }
                }
            }
            Mode::Eval => {
                let mm = others_mass(m, p);
                for (a, cv) in child_vals.iter().enumerate() {
                    for h in 0..N {
                        d.ev[a * N + h] = if mm[h] > 0.0 { cv[p][h] / mm[h] } else { 0.0 };
                    }
                }
                let combos: f32 = (0..N).map(|h| reach[p][h] * self.sizes[h]).sum();
                d.reach_share = combos / 1326.0;
                // Stash value and weights for the exploitability pass.
                d.exploitability_pct = 0.0;
                let mut num_w = 0.0f32;
                for h in 0..N {
                    num_w += reach[p][h] * self.sizes[h] * mm[h];
                }
                d.value_weight = num_w;
                d.value = (0..N)
                    .map(|h| reach[p][h] * self.sizes[h] * out[p][h])
                    .sum();
            }
            Mode::BestResponse(q) if q == p => {
                let br: f32 = (0..N)
                    .map(|h| d.reach_cache[h] * self.sizes[h] * out[p][h])
                    .sum();
                d.exploitability_pct = if d.value_weight > 0.0 && d.pot > 0 {
                    let gain_chips = (br - d.value) / d.value_weight;
                    100.0 * gain_chips / d.pot as f32
                } else {
                    0.0
                };
            }
            Mode::BestResponse(_) => {}
            Mode::Profiled(q) if q == p => {
                let mm = others_mass(m, p);
                d.br_ev = vec![0.0; k * N];
                for (a, cv) in child_vals.iter().enumerate() {
                    for h in 0..N {
                        d.br_ev[a * N + h] = if mm[h] > 0.0 { cv[p][h] / mm[h] } else { 0.0 };
                    }
                }
            }
            Mode::Profiled(_) => {}
        }
        if let Mode::Eval = mode {
            d.reach_cache = reach[p];
        }
        out
    }
}

/// Summary of a finished solve.
pub struct Report {
    /// Per seat: best-response gain against the others' average strategies,
    /// in bb per hand.
    pub br_gain_bb: Vec<f32>,
    /// Mean of `br_gain_bb` as a percent of the starting pot (blinds + antes).
    pub exploitability_pct: f32,
}

pub struct Solver<'a> {
    pub tree: &'a mut Tree,
}

impl Solver<'_> {
    fn ctx(&self) -> (Vec<f32>, V, usize) {
        (
            self.tree.tables.compat.clone(),
            class_sizes(),
            self.tree.seats,
        )
    }

    fn root_inputs(&self) -> (Vec<V>, Vec<V>) {
        let reach = vec![[1.0f32; N]; self.tree.seats];
        let m1 = matvec(&self.tree.tables.compat, &[1.0; N]);
        (reach, vec![m1; self.tree.seats])
    }

    /// Run `iterations` DCFR iterations, calling `progress(done)` now and then.
    pub fn train(&mut self, iterations: u32, mut progress: impl FnMut(u32)) {
        let (compat, sizes, seats) = self.ctx();
        let ctx = Ctx {
            compat: &compat,
            sizes,
            seats,
        };
        let (reach, m) = self.root_inputs();
        for t in 1..=iterations {
            ctx.walk(&mut self.tree.root, &reach, &m, Mode::Train(t), 0);
            if t % 50 == 0 || t == iterations {
                progress(t);
            }
        }
    }

    /// Every seat's best-response EVs against the others playing the
    /// locked profile (`Decision::locked` must be set at every node). Fills
    /// `Decision::br_ev`.
    pub fn best_response_vs_locked(&mut self) {
        let (compat, sizes, seats) = self.ctx();
        let ctx = Ctx {
            compat: &compat,
            sizes,
            seats,
        };
        let (reach, m) = self.root_inputs();
        for p in 0..seats {
            ctx.walk(&mut self.tree.root, &reach, &m, Mode::Profiled(p), 0);
        }
    }

    /// Fill EVs and per-node exploitability from the average strategy, and
    /// measure each seat's best-response gain.
    pub fn evaluate(&mut self) -> Report {
        let (compat, sizes, seats) = self.ctx();
        let ctx = Ctx {
            compat: &compat,
            sizes,
            seats,
        };
        let (reach, m) = self.root_inputs();
        let values = ctx.walk(&mut self.tree.root, &reach, &m, Mode::Eval, 0);
        let mut br_gain_bb = Vec::with_capacity(seats);
        for p in 0..seats {
            let br = ctx.walk(&mut self.tree.root, &reach, &m, Mode::BestResponse(p), 0);
            let mm = others_mass(&m, p);
            let (mut num, mut den) = (0.0f64, 0.0f64);
            for h in 0..N {
                num += f64::from(sizes[h] * (br[p][h] - values[p][h]));
                den += f64::from(sizes[h] * mm[h]);
            }
            br_gain_bb.push((num / den / BB_CHIPS as f64) as f32);
        }
        let mean = br_gain_bb.iter().sum::<f32>() / seats as f32;
        // Starting pot (blinds and antes) in bb.
        let start_pot_bb = match &self.tree.root {
            TNode::Decision(d) => d.pot as f32 / BB_CHIPS as f32,
            TNode::Terminal(t) => t.pot as f32 / BB_CHIPS as f32,
        };
        Report {
            br_gain_bb,
            exploitability_pct: 100.0 * mean / start_pot_bb.max(f32::EPSILON),
        }
    }
}

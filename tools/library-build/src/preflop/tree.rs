//! The preflop game tree, built by driving the rules engine with the
//! manifest's sizes so every stored action is legal in the app's engine.

use std::collections::HashMap;
use std::sync::Arc;

use taful_core::action::{Action, Street};
use taful_core::config::Chips;
use taful_core::engine::{GameState, Phase};
use taful_core::position::{Position, Seat};

use super::equity::{ClassTables, N};
use super::manifest::Manifest;

pub enum TNode {
    Decision(Box<Decision>),
    Terminal(Box<Terminal>),
}

pub struct Decision {
    pub seat: Seat,
    pub position: Position,
    /// Chips in the middle before the decision.
    pub pot: Chips,
    /// The actor's total commitment before acting (lost on a fold).
    pub commit: Chips,
    pub actions: Vec<Action>,
    pub children: Vec<TNode>,
    /// `[action][class]` cumulative regrets (DCFR).
    pub regrets: Vec<f32>,
    /// `[action][class]` cumulative weighted strategy.
    pub strategy_sum: Vec<f32>,
    /// Filled by the final evaluation: `[action][class]` EV in chips.
    pub ev: Vec<f32>,
    /// Filled by the final evaluation: local exploitability, % of pot.
    pub exploitability_pct: f32,
    /// Filled by the final evaluation: share of the actor's combos that reach
    /// this node (0..=1).
    pub reach_share: f32,
    /// Evaluation scratch: the actor's reach per class on the average path,
    /// `Σ reach·combos·value` and `Σ reach·combos·opponent mass`.
    pub reach_cache: [f32; N],
    pub value: f32,
    pub value_weight: f32,
}

pub enum Outcome {
    /// Everyone else folded.
    Fold { winner: Seat },
    /// Two or more players see a flop or are all-in. One share matrix per
    /// ordered pair of live players: `pair[i][k]` = `compat ⊙ share(eq)`.
    Showdown {
        pairs: Vec<Vec<Option<Arc<Vec<f32>>>>>,
    },
}

pub struct Terminal {
    /// Pot after rake.
    pub pot: Chips,
    /// Total commitment per seat.
    pub commits: Vec<Chips>,
    /// Seats still in the hand, ascending.
    pub live: Vec<Seat>,
    pub outcome: Outcome,
}

/// Tree plus what the solver needs to evaluate it.
pub struct Tree {
    pub root: TNode,
    pub seats: usize,
    pub button: Seat,
    pub tables: ClassTables,
    pub decisions: usize,
    pub terminals: usize,
}

struct Builder<'a> {
    m: &'a Manifest,
    tables: &'a ClassTables,
    /// Share matrices by `(Rᵢ, Rⱼ, k)` bit patterns.
    shares: HashMap<(u32, u32, u32), Arc<Vec<f32>>>,
    decisions: usize,
    terminals: usize,
}

/// Postflop acting order: SB first, button last. Higher = in position.
fn postflop_order(seats: usize, button: Seat, seat: Seat) -> usize {
    (seat + seats - button - 1) % seats
}

impl Builder<'_> {
    fn share_matrix(&mut self, ri: f32, rk: f32, exp: f32) -> Arc<Vec<f32>> {
        let t = self.tables;
        self.shares
            .entry((ri.to_bits(), rk.to_bits(), exp.to_bits()))
            .or_insert_with(|| {
                let mut w = vec![0.0f32; N * N];
                for a in 0..N {
                    for b in 0..N {
                        let e = t.eq(a, b);
                        let num = e.powf(exp) * ri;
                        let den = num + (1.0 - e).powf(exp) * rk;
                        let s = if den > 0.0 { num / den } else { 0.5 };
                        w[a * N + b] = t.compat[a * N + b] * s;
                    }
                }
                Arc::new(w)
            })
            .clone()
    }

    fn raise_action(&self, s: &GameState) -> Option<Action> {
        let b = s.bounds()?;
        let (min_to, max_to) = (b.min_to?, b.max_to?);
        let sz = &self.m.sizing;
        let level = s.aggressions() as u32;
        if level >= sz.max_raises {
            return None;
        }
        let seat = b.seat;
        let pos = s.position(seat);
        let cur_bb = s.current_bet() as f64 / 100.0;
        let to_bb = if level + 1 == sz.max_raises {
            return Some(Action::AllIn);
        } else if level == 0 {
            if pos == Position::Sb {
                sz.sb_open_bb
            } else {
                sz.open_bb
            }
        } else if level == 1 {
            let ip = !matches!(pos, Position::Sb | Position::Bb);
            let mult = if ip {
                sz.threebet_ip_mult
            } else {
                sz.threebet_oop_mult
            };
            let callers = s
                .players()
                .iter()
                .enumerate()
                .filter(|(i, p)| *i != seat && !p.folded && p.street_commit == s.current_bet())
                .count()
                .saturating_sub(1); // the raiser
            cur_bb * mult + callers as f64 * cur_bb * sz.squeeze_per_caller_mult
        } else {
            cur_bb * sz.fourbet_mult
        };
        let to = self.m.round_chips(to_bb).max(min_to);
        let stack = s.config().stack(seat) as f64;
        if to >= max_to || to as f64 * 100.0 >= sz.allin_above_stack_pct * stack {
            Some(Action::AllIn)
        } else {
            Some(Action::Raise(to))
        }
    }

    fn actions(&self, s: &GameState) -> Vec<Action> {
        let b = s.bounds().expect("decision node");
        let seat = b.seat;
        let mut out = Vec::new();
        if b.to_call > 0 {
            out.push(Action::Fold);
            let voluntary = s.actions()[0]
                .iter()
                .any(|&(st, a)| st == seat && !matches!(a, Action::Fold | Action::Check));
            let level = s.aggressions() as u32;
            let in_after_call = 1 + s
                .players()
                .iter()
                .enumerate()
                .filter(|(i, p)| *i != seat && !p.folded && p.street_commit == s.current_bet())
                .count();
            let cold_ok = voluntary || level <= self.m.tree.cold_call_max_level;
            // Level 0 = only the blinds are in: calling would be a limp, which
            // the sizing set leaves out (spec §6).
            if level > 0 && cold_ok && in_after_call as u32 <= self.m.tree.max_players_postflop {
                out.push(Action::Call);
            }
        } else {
            out.push(Action::Check);
        }
        if let Some(r) = self.raise_action(s)
            && !out.contains(&r)
        {
            out.push(r);
        }
        out
    }

    fn terminal(&mut self, s: &GameState) -> TNode {
        self.terminals += 1;
        let seats = s.players().len();
        let commits: Vec<Chips> = s.players().iter().map(|p| p.total_commit).collect();
        let live: Vec<Seat> = (0..seats).filter(|&i| !s.players()[i].folded).collect();
        let gross = s.pot();
        let outcome = if live.len() == 1 {
            Outcome::Fold { winner: live[0] }
        } else {
            let all_in = live.iter().any(|&i| s.players()[i].all_in());
            let r = &self.m.realization;
            let factors = match s.aggressions() {
                0 | 1 => r.srp,
                2 => r.threebet,
                _ => r.fourbet,
            };
            let behind = live
                .iter()
                .map(|&i| s.players()[i].stack)
                .min()
                .unwrap_or(0) as f64;
            let spr = behind / gross as f64;
            let shrink = |f: f64| {
                if spr < r.low_spr && r.low_spr > 0.0 {
                    1.0 + (f - 1.0) * spr / r.low_spr
                } else {
                    f
                }
            };
            let ip = *live
                .iter()
                .max_by_key(|&&i| postflop_order(seats, s.button(), i))
                .expect("live");
            let factor = |i: Seat| -> f32 {
                if all_in {
                    1.0
                } else if i == ip {
                    shrink(factors[0]) as f32
                } else {
                    shrink(factors[1]) as f32
                }
            };
            let exp = if all_in {
                1.0
            } else {
                shrink(r.equity_exponent) as f32
            };
            let mut pairs = vec![vec![None; seats]; seats];
            for &i in &live {
                for &k in &live {
                    if i != k {
                        pairs[i][k] = Some(self.share_matrix(factor(i), factor(k), exp));
                    }
                }
            }
            Outcome::Showdown { pairs }
        };
        let rake = if live.len() > 1 {
            s.config().rake.amount(gross, true)
        } else {
            0
        };
        TNode::Terminal(Box::new(Terminal {
            pot: gross - rake,
            commits,
            live,
            outcome,
        }))
    }

    fn build(&mut self, s: &GameState) -> TNode {
        match s.phase() {
            Phase::ToAct(seat) if s.street() == Street::Preflop => {
                self.decisions += 1;
                let actions = self.actions(s);
                let children = actions
                    .iter()
                    .map(|&a| {
                        let next = s.apply(a).expect("tree actions are legal");
                        self.build(&next)
                    })
                    .collect();
                let k = actions.len();
                TNode::Decision(Box::new(Decision {
                    seat,
                    position: s.position(seat),
                    pot: s.pot(),
                    commit: s.players()[seat].total_commit,
                    actions,
                    children,
                    regrets: vec![0.0; k * N],
                    strategy_sum: vec![0.0; k * N],
                    ev: vec![0.0; k * N],
                    exploitability_pct: 0.0,
                    reach_share: 0.0,
                    reach_cache: [0.0; N],
                    value: 0.0,
                    value_weight: 0.0,
                }))
            }
            _ => self.terminal(s),
        }
    }
}

impl Tree {
    pub fn build(m: &Manifest, tables: ClassTables) -> Tree {
        let config = m.game_config();
        let seats = usize::from(config.table_size);
        let button = 0;
        let state = GameState::new(config, button, 0).expect("valid config");
        let mut b = Builder {
            m,
            tables: &tables,
            shares: HashMap::new(),
            decisions: 0,
            terminals: 0,
        };
        let root = b.build(&state);
        let (decisions, terminals) = (b.decisions, b.terminals);
        Tree {
            root,
            seats,
            button,
            tables,
            decisions,
            terminals,
        }
    }
}

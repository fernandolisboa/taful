//! Inline best responses against bot profiles (spec §7.3, §7.8, D8
//! amendment "Build sequencing", #27).
//!
//! After the reference solve, while the full tree is still in memory, each
//! profile is locked at every node one player acts (the profile's transform
//! of the reference strategy, `taful_core::profile`) and the other player's
//! best response is evaluated in one pass over the solver's tree. Two
//! passes per profile (the bot out of position, then in position) give, at
//! every node the library keeps, the profile's strategy for the actor and
//! the actor's best-response EVs against the profile.
//!
//! The pass mirrors postflop-solver's own best-response evaluation (the
//! exploitability check): terminal values come from the solver's
//! evaluator, chance nodes scale the reach by the chance factor and fold
//! suit-isomorphic cards back in with the solver's swap lists. What the
//! solver does not expose is rebuilt from its interpreter once per solve:
//! the actions at each node (the same for every runout) and the card behind
//! each chance child.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use postflop_solver::{Action as SAction, Game, GameNode, PostFlopGame, PostFlopNode};
use rayon::prelude::*;
use taful_core::action::Action;
use taful_core::config::BB_CHIPS;
use taful_core::profile::Profile;
use taful_solver::format::{
    ProfileNode, ProfileSection, ev_scale_for, quantize_ev, quantize_strategy,
};

use super::export::map_action;

/// What the interpreter tells about the tree's shape.
pub struct Shape {
    /// Actions at each decision node, by the action indices that lead to it
    /// (dealt cards left out): the action tree is the same on every runout.
    actions: HashMap<Vec<u8>, Vec<(Action, i32)>>,
    /// Card of each child of a turn chance node.
    turn_cards: Vec<u8>,
    /// Card of each child of a river chance node, by turn card.
    river_cards: HashMap<u8, Vec<u8>>,
}

fn chance_cards(game: &PostFlopGame) -> Vec<u8> {
    game.available_actions()
        .iter()
        .map(|a| match a {
            SAction::Chance(c) => *c,
            other => panic!("chance node with action {other:?}"),
        })
        .collect()
}

impl Shape {
    /// Walks every action path once (the first card at each chance node)
    /// and every turn's river chance.
    pub fn of(game: &mut PostFlopGame) -> Result<Shape, String> {
        let mut shape = Shape {
            actions: HashMap::new(),
            turn_cards: Vec::new(),
            river_cards: HashMap::new(),
        };
        let mut hist = Vec::new();
        let mut path = Vec::new();
        shape.walk(game, &mut hist, &mut path)?;
        if !shape.turn_cards.is_empty() {
            // Check-check on the flop and the turn reaches each turn's river.
            for &t in &shape.turn_cards.clone() {
                game.apply_history(&[0, 0, usize::from(t), 0, 0]);
                if !game.is_chance_node() {
                    return Err(format!("no river chance after turn {t}"));
                }
                shape.river_cards.insert(t, chance_cards(game));
            }
        } else if game.current_board().len() == 4 {
            // A turn-start tree (tests): one river chance per runout.
            let t = game.current_board()[3];
            game.back_to_root();
            game.apply_history(&[0, 0]);
            if game.is_chance_node() {
                shape.river_cards.insert(t, chance_cards(game));
            }
        }
        game.back_to_root();
        Ok(shape)
    }

    fn walk(
        &mut self,
        game: &mut PostFlopGame,
        hist: &mut Vec<usize>,
        path: &mut Vec<u8>,
    ) -> Result<(), String> {
        game.apply_history(hist);
        if game.is_terminal_node() {
            return Ok(());
        }
        if game.is_chance_node() {
            let cards = chance_cards(game);
            if game.current_board().len() == 3 && self.turn_cards.is_empty() {
                self.turn_cards = cards.clone();
            }
            hist.push(usize::from(cards[0]));
            self.walk(game, hist, path)?;
            hist.pop();
            return Ok(());
        }
        // Each action with its "to" amount on the street (bets, raises and
        // all-ins).
        let actions = game
            .available_actions()
            .into_iter()
            .map(|a| {
                let amount = match a {
                    SAction::Bet(x) | SAction::Raise(x) | SAction::AllIn(x) => x,
                    _ => 0,
                };
                map_action(a).map(|t| (t, amount))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let k = actions.len();
        self.actions.entry(path.clone()).or_insert(actions);
        for a in 0..k {
            hist.push(a);
            path.push(a as u8);
            self.walk(game, hist, path)?;
            path.pop();
            hist.pop();
        }
        Ok(())
    }

    fn chance_card(&self, board_len: usize, turn: Option<u8>, child: usize) -> u8 {
        if board_len == 3 {
            self.turn_cards[child]
        } else {
            self.river_cards[&turn.expect("river chance after a turn")][child]
        }
    }
}

/// What one pass records at a node it keeps.
#[derive(Debug, Clone, PartialEq)]
pub enum Recorded {
    /// The bot acts: its profiled strategy `[action][hand]` and its reach
    /// there (unscaled by chance factors) per solver hand.
    Bot { strategy: Vec<f32>, reach: Vec<f32> },
    /// The responder acts: its EV in bb of each action `[action][hand]`,
    /// postflop-solver's convention (a fold is 0).
    Responder { ev: Vec<f32> },
}

/// How the responder plays in a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Responder {
    /// Best response (what the library stores).
    Best,
    /// The reference strategy (tests: with a neutral profile this
    /// reproduces the solver's own EVs).
    #[cfg_attr(not(test), allow(dead_code))]
    Reference,
}

/// One pass over a solved game.
pub struct Pass<'a> {
    pub game: &'a PostFlopGame,
    pub shape: &'a Shape,
    pub profile: &'a Profile,
    /// The player the profile is locked for (0 = OOP).
    pub bot: usize,
    pub responder: Responder,
    /// Interpreter histories (actions and card indices) to record; `None`
    /// records every decision node (tests on small trees).
    pub keep: Option<&'a HashSet<Vec<usize>>>,
}

struct Ctx<'a> {
    p: &'a Pass<'a>,
    hero: usize,
    cards: [&'a [(u8, u8)]; 2],
    /// Index of the same two cards in the other player's hand list.
    same: [Vec<Option<usize>>; 2],
    num_combinations: f64,
    half_pot: f64,
    compressed: bool,
    out: Mutex<HashMap<Vec<usize>, Recorded>>,
}

/// Where the walk is.
#[derive(Clone)]
struct At {
    hist: Vec<usize>,
    path: Vec<u8>,
    board: u64,
    turn: Option<u8>,
    board_len: usize,
    /// Chips each player put in since the flop.
    total: [i32; 2],
    /// The street's last bet or raise ("to" amount).
    last_bet: i32,
    /// Bets and raises on this street.
    facing: usize,
    /// Product of the chance factors so far.
    scale: f64,
}

/// The solver's evaluator into a fresh vector.
#[allow(unsafe_code)]
fn evaluate(game: &PostFlopGame, node: &PostFlopNode, player: usize, cfreach: &[f32]) -> Vec<f32> {
    let n = game.num_private_hands(player);
    let mut out: Vec<f32> = Vec::with_capacity(n);
    // Initialized up front, so `set_len` is sound whatever the evaluator
    // writes (it overwrites every element in practice).
    out.spare_capacity_mut()[..n].fill(std::mem::MaybeUninit::new(0.0));
    game.evaluate(&mut out.spare_capacity_mut()[..n], node, player, cfreach);
    // SAFETY: all `n` elements were initialized above.
    unsafe { out.set_len(n) };
    out
}

fn normalized(raw: &[f32], k: usize, n: usize) -> Vec<f32> {
    let mut s = raw.to_vec();
    for h in 0..n {
        let t: f32 = (0..k).map(|a| raw[a * n + h]).sum();
        for a in 0..k {
            s[a * n + h] = if t > 0.0 {
                raw[a * n + h] / t
            } else {
                1.0 / k as f32
            };
        }
    }
    s
}

impl Ctx<'_> {
    fn dead(&self, p: usize, h: usize, board: u64) -> bool {
        let (a, b) = self.cards[p][h];
        board & ((1u64 << a) | (1u64 << b)) != 0
    }

    /// Per hand of `p`: the other player's reach `w` over hands that share
    /// no card with it or the board.
    fn compat(&self, p: usize, w: &[f32], board: u64) -> Vec<f64> {
        let o = p ^ 1;
        let mut total = 0.0f64;
        let mut minus = [0.0f64; 52];
        for (j, &x) in w.iter().enumerate() {
            if x != 0.0 && !self.dead(o, j, board) {
                let (a, b) = self.cards[o][j];
                total += f64::from(x);
                minus[usize::from(a)] += f64::from(x);
                minus[usize::from(b)] += f64::from(x);
            }
        }
        (0..self.cards[p].len())
            .map(|h| {
                if self.dead(p, h, board) {
                    return 0.0;
                }
                let (a, b) = self.cards[p][h];
                let same = self.same[p][h].map_or(0.0, |j| f64::from(w[j]));
                (total + same - minus[usize::from(a)] - minus[usize::from(b)]).max(0.0)
            })
            .collect()
    }

    fn strategy(&self, node: &PostFlopNode, k: usize, n: usize) -> Vec<f32> {
        if self.compressed {
            let raw: Vec<f32> = node
                .strategy_compressed()
                .iter()
                .map(|&x| f32::from(x))
                .collect();
            normalized(&raw, k, n)
        } else {
            normalized(node.strategy(), k, n)
        }
    }

    /// The actor's stored counterfactual values (after `finalize`).
    fn cfvalues(&self, node: &PostFlopNode) -> Vec<f32> {
        if self.compressed {
            let scale = node.cfvalue_scale() / f32::from(i16::MAX);
            node.cfvalues_compressed()
                .iter()
                .map(|&x| f32::from(x) * scale)
                .collect()
        } else {
            node.cfvalues().to_vec()
        }
    }

    fn keep(&self, hist: &[usize]) -> bool {
        self.p.keep.is_none_or(|k| k.contains(hist))
    }

    /// Counterfactual values of the responder's hands at `node`; `bot` is
    /// the bot's reach under the profile, `hero_ref` the responder's reach
    /// under the reference (it normalizes the bot's reference EVs, as the
    /// solver's own values were computed against it).
    fn visit(&self, node: &PostFlopNode, at: &At, bot: &[f32], hero_ref: &[f32]) -> Vec<f32> {
        let g = self.p.game;
        let hero = self.hero;
        if node.is_terminal() {
            return evaluate(g, node, hero, bot);
        }
        let k = node.num_actions();
        let par = node.enable_parallelization();
        if node.is_chance() {
            let f = 1.0 / g.chance_factor(node) as f32;
            let bot: Vec<f32> = bot.iter().map(|x| x * f).collect();
            let hero_ref: Vec<f32> = hero_ref.iter().map(|x| x * f).collect();
            let child = |c: usize| {
                let card = self.p.shape.chance_card(at.board_len, at.turn, c);
                let mut next = at.clone();
                next.hist.push(usize::from(card));
                next.board |= 1u64 << card;
                if at.board_len == 3 {
                    next.turn = Some(card);
                }
                next.board_len += 1;
                next.last_bet = 0;
                next.facing = 0;
                next.scale = at.scale * f64::from(g.chance_factor(node) as u32);
                self.visit(&node.play(c), &next, &bot, &hero_ref)
            };
            let vals: Vec<Vec<f32>> = if par {
                (0..k).into_par_iter().map(child).collect()
            } else {
                (0..k).map(child).collect()
            };
            let n = g.num_private_hands(hero);
            let mut sum = vec![0.0f64; n];
            for v in &vals {
                for (s, &x) in sum.iter_mut().zip(v) {
                    *s += f64::from(x);
                }
            }
            for (i, &repr) in g.isomorphic_chances(node).iter().enumerate() {
                let mut v = vals[usize::from(repr)].clone();
                for &(a, b) in &g.isomorphic_swap(node, i)[hero] {
                    v.swap(usize::from(a), usize::from(b));
                }
                for (s, x) in sum.iter_mut().zip(v) {
                    *s += f64::from(x);
                }
            }
            return sum.into_iter().map(|x| x as f32).collect();
        }

        let player = node.player();
        let raw = &self.p.shape.actions[&at.path];
        let actions: &Vec<Action> = &raw.iter().map(|x| x.0).collect();
        debug_assert_eq!(actions.len(), k);
        let n = g.num_private_hands(player);
        let step = |a: usize| {
            let mut next = at.clone();
            next.hist.push(a);
            next.path.push(a as u8);
            let o = player ^ 1;
            let (action, to) = raw[a];
            match action {
                Action::Call => next.total[player] = next.total[o],
                Action::Bet(_) | Action::Raise(_) | Action::AllIn => {
                    next.total[player] = next.total[o] + to - at.last_bet;
                    next.last_bet = to;
                    next.facing += 1;
                }
                Action::Fold | Action::Check => {}
            }
            next
        };
        let ref_strategy = self.strategy(node, k, n);
        let keep = self.keep(&at.hist);

        if player == self.p.bot {
            // Lock the profile here.
            let board = at.board;
            let weights: Vec<f32> = (0..n)
                .map(|h| {
                    if self.dead(player, h, board) {
                        0.0
                    } else {
                        bot[h]
                    }
                })
                .collect();
            let norm = self.compat(player, hero_ref, board);
            let cfv = self.cfvalues(node);
            let rows = |t: &[f32], scale: Option<&[f64]>| -> Vec<f32> {
                (0..n)
                    .flat_map(|h| {
                        (0..k).map(move |a| match scale {
                            Some(s) if s[h] > 0.0 => (f64::from(t[a * n + h]) / s[h]) as f32,
                            Some(_) => 0.0,
                            None => t[a * n + h],
                        })
                    })
                    .collect()
            };
            let locked = self.p.profile.postflop_strategy(
                actions,
                &rows(&ref_strategy, None),
                &rows(&cfv, Some(&norm)),
                &weights,
                at.facing,
            );
            let mut table = vec![0.0f32; k * n];
            for h in 0..n {
                for a in 0..k {
                    table[a * n + h] = locked[h * k + a];
                }
            }
            let child = |a: usize| {
                let r: Vec<f32> = (0..n).map(|h| bot[h] * table[a * n + h]).collect();
                self.visit(&node.play(a), &step(a), &r, hero_ref)
            };
            let vals: Vec<Vec<f32>> = if par && k > 1 {
                (0..k).into_par_iter().map(child).collect()
            } else {
                (0..k).map(child).collect()
            };
            if keep {
                let reach = weights
                    .iter()
                    .map(|&x| (f64::from(x) * at.scale) as f32)
                    .collect();
                self.out.lock().expect("not poisoned").insert(
                    at.hist.clone(),
                    Recorded::Bot {
                        strategy: table,
                        reach,
                    },
                );
            }
            let m = g.num_private_hands(hero);
            let mut v = vec![0.0f32; m];
            for c in vals {
                for (x, y) in v.iter_mut().zip(c) {
                    *x += y;
                }
            }
            return v;
        }

        // The responder acts.
        let child = |a: usize| {
            let r: Vec<f32> = (0..n)
                .map(|h| hero_ref[h] * ref_strategy[a * n + h])
                .collect();
            self.visit(&node.play(a), &step(a), bot, &r)
        };
        let vals: Vec<Vec<f32>> = if par && k > 1 {
            (0..k).into_par_iter().map(child).collect()
        } else {
            (0..k).map(child).collect()
        };
        if keep {
            let norm = self.compat(hero, bot, at.board);
            let offset = self.half_pot + f64::from(at.total[hero]);
            let mut ev = vec![0.0f32; k * n];
            for a in 0..k {
                let fold = actions[a] == Action::Fold;
                for h in 0..n {
                    ev[a * n + h] = if fold || norm[h] <= 0.0 {
                        0.0
                    } else {
                        ((f64::from(vals[a][h]) * self.num_combinations / norm[h] + offset)
                            / BB_CHIPS as f64) as f32
                    };
                }
            }
            self.out
                .lock()
                .expect("not poisoned")
                .insert(at.hist.clone(), Recorded::Responder { ev });
        }
        (0..n)
            .map(|h| match self.p.responder {
                Responder::Best => (0..k).map(|a| vals[a][h]).fold(f32::NEG_INFINITY, f32::max),
                Responder::Reference => (0..k).map(|a| ref_strategy[a * n + h] * vals[a][h]).sum(),
            })
            .collect()
    }
}

impl Pass<'_> {
    /// Runs the pass; the game must be solved (`finalize`d). Returns the
    /// recorded nodes by history and the responder's root values.
    pub fn run(&self) -> (HashMap<Vec<usize>, Recorded>, Vec<f32>) {
        let g = self.game;
        let hero = self.bot ^ 1;
        let cards = [g.private_cards(0), g.private_cards(1)];
        let same = [0, 1].map(|p| {
            let other: HashMap<(u8, u8), usize> = cards[p ^ 1]
                .iter()
                .enumerate()
                .map(|(i, &c)| (c, i))
                .collect();
            cards[p].iter().map(|c| other.get(c).copied()).collect()
        });
        let w = [g.initial_weights(0), g.initial_weights(1)];
        let mut num_combinations = 0.0f64;
        for (&(a, b), &x) in cards[0].iter().zip(w[0]) {
            let m = (1u64 << a) | (1u64 << b);
            for (&(c, d), &y) in cards[1].iter().zip(w[1]) {
                if m & ((1u64 << c) | (1u64 << d)) == 0 {
                    num_combinations += f64::from(x) * f64::from(y);
                }
            }
        }
        let cc = g.card_config();
        let mut board = 0u64;
        let mut board_len = 3;
        for c in cc.flop {
            board |= 1u64 << c;
        }
        let mut turn = None;
        for c in [cc.turn, cc.river] {
            if c != postflop_solver::NOT_DEALT {
                board |= 1u64 << c;
                board_len += 1;
            }
        }
        if cc.turn != postflop_solver::NOT_DEALT {
            turn = Some(cc.turn);
        }
        let ctx = Ctx {
            p: self,
            hero,
            cards,
            same,
            num_combinations,
            half_pot: f64::from(g.tree_config().starting_pot) / 2.0,
            compressed: g.is_compression_enabled(),
            out: Mutex::new(HashMap::new()),
        };
        let at = At {
            hist: Vec::new(),
            path: Vec::new(),
            board,
            turn,
            board_len,
            total: [0, 0],
            last_bet: 0,
            facing: 0,
            scale: 1.0,
        };
        let v = ctx.visit(&g.root(), &at, w[self.bot], w[hero]);
        (ctx.out.into_inner().expect("not poisoned"), v)
    }
}

/// A node the library keeps, as the export saw it.
pub struct Kept {
    /// Interpreter history (action indices and card indices).
    pub history: Vec<usize>,
    /// 0 = OOP acts.
    pub actor: usize,
    /// Solver hand index of each of the node's live rows.
    pub rows: Vec<usize>,
    pub actions: usize,
}

/// The v3 profile sections for the kept nodes of a finalized game: two
/// passes per profile.
pub fn sections(
    game: &mut PostFlopGame,
    kept: &[Kept],
    combos: &[Vec<u16>; 2],
    root_weights: &[Vec<f32>; 2],
    min_combo_reach: f32,
    profiles: &[Profile],
) -> Result<Vec<ProfileSection>, String> {
    let shape = Shape::of(game)?;
    let keep: HashSet<Vec<usize>> = kept.iter().map(|k| k.history.clone()).collect();
    let game: &PostFlopGame = game;
    let mut out = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let t0 = std::time::Instant::now();
        let passes: Vec<HashMap<Vec<usize>, Recorded>> = (0..2)
            .map(|bot| {
                Pass {
                    game,
                    shape: &shape,
                    profile,
                    bot,
                    responder: Responder::Best,
                    keep: Some(&keep),
                }
                .run()
                .0
            })
            .collect();
        let mut evs: Vec<Vec<f32>> = Vec::with_capacity(kept.len());
        let mut nodes = Vec::with_capacity(kept.len());
        for k in kept {
            let p = k.actor;
            let missing = || {
                format!(
                    "profile {}: node {:?} not in the solver's tree (a stored turn is not one of \
                     the solver's own turn cards?)",
                    profile.id, k.history
                )
            };
            let Some(Recorded::Bot { strategy, reach }) = passes[p].get(&k.history) else {
                return Err(missing());
            };
            let Some(Recorded::Responder { ev }) = passes[p ^ 1].get(&k.history) else {
                return Err(missing());
            };
            let n = reach.len();
            let mut live: Vec<(u16, usize)> = (0..n)
                .filter(|&h| {
                    let root = root_weights[p][h];
                    reach[h] > 0.0 && root > 0.0 && reach[h] >= min_combo_reach * root
                })
                .map(|h| (combos[p][h], h))
                .collect();
            live.sort_unstable();
            let strategy_rows = live
                .iter()
                .flat_map(|&(_, h)| {
                    let row: Vec<f32> = (0..k.actions).map(|a| strategy[a * n + h]).collect();
                    quantize_strategy(&row)
                })
                .collect();
            evs.push(
                k.rows
                    .iter()
                    .flat_map(|&h| (0..k.actions).map(move |a| ev[a * n + h]))
                    .collect(),
            );
            nodes.push(ProfileNode {
                live: live.iter().map(|x| x.0).collect(),
                strategy: strategy_rows,
                ev: Vec::new(),
            });
        }
        let max_ev = evs.iter().flatten().fold(0.0f32, |m, e| m.max(e.abs()));
        let ev_scale = ev_scale_for(max_ev);
        for (node, ev) in nodes.iter_mut().zip(evs) {
            node.ev = ev.iter().map(|&e| quantize_ev(e, ev_scale)).collect();
        }
        eprintln!(
            "    profile {}: best responses in {:.1}s",
            profile.id,
            t0.elapsed().as_secs_f64()
        );
        out.push(ProfileSection {
            id: profile.id.clone(),
            profile: profile.to_toml(),
            ev_scale,
            nodes,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use postflop_solver::{
        ActionTree, BoardState, CardConfig, PostFlopGame, TreeConfig, finalize, solve_step,
    };
    use taful_core::action::Line;
    use taful_core::cards::{Card, Hand, parse_cards};
    use taful_core::config::{BB_CHIPS, Rake};
    use taful_core::profile::Profile;
    use taful_solver::postflop::{geometry, line_range};
    use taful_solver::preflop::PreflopChart;
    use taful_solver::river::{
        DcfrRiverSolver, RiverSpot, RiverTree, SolveOptions, SubgameSolver, best_response_detail,
        parse_sizes,
    };

    use super::super::{bet_sizes, solver_range};
    use super::*;

    fn chart() -> PreflopChart {
        PreflopChart::decode(include_bytes!(
            "../../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
        ))
        .unwrap()
    }

    fn shipped(id: &str) -> Profile {
        let path = format!(
            "{}/../../content/bots/{id}.toml",
            env!("CARGO_MANIFEST_DIR")
        );
        Profile::from_toml(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn neutral() -> Profile {
        let mut p = shipped("tag");
        p.postflop = taful_core::profile::PostflopDeviation {
            fold_to_cbet: 0.0,
            fold_to_raise: 0.0,
            bluff_value_shift: 0.0,
            aggression: 0.0,
        };
        p.sizing.prefer = taful_core::profile::Sizing::Reference;
        p
    }

    const LINE: &str = "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c";

    /// A solved game; `ranges` = small ranges instead of the chart's (fast
    /// flop trees in debug builds).
    fn game(
        board: &str,
        start: BoardState,
        (pot, stack): (i32, i32),
        iters: u32,
        ranges: Option<[&str; 2]>,
    ) -> PostFlopGame {
        let chart = chart();
        let line: Line = LINE.parse().unwrap();
        let g = geometry(chart.config(), &line).unwrap();
        let ranges = match ranges {
            Some(r) => r.map(|s| {
                s.parse::<taful_core::range::Range>()
                    .unwrap()
                    .weights()
                    .to_vec()
            }),
            None => [
                line_range(&chart, &line, g.oop).unwrap(),
                line_range(&chart, &line, g.ip).unwrap(),
            ],
        };
        let cards = parse_cards(board).unwrap();
        let sizes = |b: &str| bet_sizes(&[b.to_string(), String::new()]).unwrap();
        let tree = TreeConfig {
            initial_state: start,
            starting_pot: pot,
            effective_stack: stack,
            rake_rate: 0.0,
            rake_cap: 0.0,
            flop_bet_sizes: [sizes("50%"), sizes("50%")],
            turn_bet_sizes: [sizes("75%"), sizes("75%")],
            river_bet_sizes: [
                bet_sizes(&["50%, 100%".into(), "3x".into()]).unwrap(),
                bet_sizes(&["50%, 100%".into(), "3x".into()]).unwrap(),
            ],
            turn_donk_sizes: None,
            river_donk_sizes: None,
            add_allin_threshold: 1.5,
            force_allin_threshold: 0.15,
            merging_threshold: 0.1,
        };
        let card = |i: usize| {
            cards
                .get(i)
                .map_or(postflop_solver::NOT_DEALT, |c| c.index())
        };
        let cc = CardConfig {
            range: [solver_range(&ranges[0]), solver_range(&ranges[1])],
            flop: [card(0), card(1), card(2)],
            turn: card(3),
            river: card(4),
        };
        let mut game = PostFlopGame::with_config(cc, ActionTree::new(tree).unwrap()).unwrap();
        game.allocate_memory(false);
        for i in 0..iters {
            solve_step(&game, i);
        }
        finalize(&mut game);
        game
    }

    /// Flop and turn decision histories (raw turn children only).
    fn flop_turn_histories(game: &mut PostFlopGame) -> Vec<Vec<usize>> {
        fn walk(game: &mut PostFlopGame, h: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
            game.apply_history(h);
            if game.is_terminal_node() {
                return;
            }
            if game.is_chance_node() {
                if game.current_board().len() >= 4 {
                    return;
                }
                for c in chance_cards(game) {
                    h.push(usize::from(c));
                    walk(game, h, out);
                    h.pop();
                    game.apply_history(h);
                }
                return;
            }
            out.push(h.clone());
            for a in 0..game.available_actions().len() {
                h.push(a);
                walk(game, h, out);
                h.pop();
                game.apply_history(h);
            }
        }
        let mut out = Vec::new();
        walk(game, &mut Vec::new(), &mut out);
        game.back_to_root();
        out
    }

    /// With a neutral profile and the responder playing the reference, the
    /// pass reproduces the solver's own EVs at every flop and turn node:
    /// recursion, chance factors, isomorphic turns, normalization and the
    /// EV convention all match.
    #[test]
    fn reference_pass_reproduces_the_solver_evs() {
        // Two-tone: some turns are isomorphic and folded back by swaps.
        let mut g = game(
            "Td9d6h",
            BoardState::Flop,
            (2000, 9050),
            40,
            Some(["QQ+,AK,T9s,JTs,A5s,Td9d", "JJ-88,AQs,KQs,98s,T8s,AJo"]),
        );
        let hist = flop_turn_histories(&mut g);
        let keep: HashSet<Vec<usize>> = hist.iter().cloned().collect();
        let shape = Shape::of(&mut g).unwrap();
        let p = neutral();
        let mut max_diff = 0.0f32;
        let mut checked = 0;
        for bot in 0..2 {
            let (rec, _) = Pass {
                game: &g,
                shape: &shape,
                profile: &p,
                bot,
                responder: Responder::Reference,
                keep: Some(&keep),
            }
            .run();
            for h in &hist {
                g.apply_history(h);
                g.cache_normalized_weights();
                let actor = g.current_player();
                if actor == bot {
                    // The neutral profile plays the reference.
                    let Some(Recorded::Bot { strategy, .. }) = rec.get(h) else {
                        panic!("{h:?}")
                    };
                    let s = g.strategy();
                    let w = g.weights(actor);
                    let n = w.len();
                    for (i, (x, y)) in strategy.iter().zip(&s).enumerate() {
                        if w[i % n] > 0.0 {
                            assert!((x - y).abs() < 1e-4, "{h:?}: {x} vs {y}");
                        }
                    }
                    continue;
                }
                let Some(Recorded::Responder { ev }) = rec.get(h) else {
                    panic!("{h:?}")
                };
                let theirs = g.expected_values_detail(actor);
                let nw = g.normalized_weights(actor);
                let n = nw.len();
                for (i, (&m, &t)) in ev.iter().zip(&theirs).enumerate() {
                    if nw[i % n] > 0.0 {
                        let d = (m * BB_CHIPS as f32 - t).abs();
                        max_diff = max_diff.max(d);
                        checked += 1;
                    }
                }
            }
        }
        eprintln!("{checked} EVs, max difference {max_diff} chips");
        assert!(checked > 1000, "{checked}");
        // Chips; the pot is 2,000.
        assert!(max_diff < 0.05, "max EV difference {max_diff} chips");
    }

    /// The acceptance check (#27): on a river subgame the pass's best
    /// response against a profile agrees with Taful's river solver's
    /// best-response evaluation against the same locked strategy.
    #[test]
    fn best_response_agrees_with_the_river_solver() {
        let board = "AcKdQh7s2d";
        let (pot, stack) = (2000, 9050);
        let mut g = game(board, BoardState::River, (pot, stack), 200, None);
        let shape = Shape::of(&mut g).unwrap();
        let station = shipped("calling-station");
        let (rec, _) = Pass {
            game: &g,
            shape: &shape,
            profile: &station,
            bot: 1,
            responder: Responder::Best,
            keep: None,
        }
        .run();

        // Taful's river tree for the same spot.
        let chart = chart();
        let line: Line = LINE.parse().unwrap();
        let geo = geometry(chart.config(), &line).unwrap();
        let spot = RiverSpot {
            board: parse_cards(board).unwrap().try_into().unwrap(),
            ranges: [
                line_range(&chart, &line, geo.oop).unwrap(),
                line_range(&chart, &line, geo.ip).unwrap(),
            ],
            pot: pot as u64,
            stack: stack as u64,
            tree: RiverTree {
                bets: parse_sizes("50%, 100%", false).unwrap(),
                raises: parse_sizes("3x", true).unwrap(),
                add_allin_threshold: 1.5,
                force_allin_threshold: 0.15,
                merging_threshold: 0.1,
            },
            rake: Rake::ZERO,
        };
        let sol = DcfrRiverSolver
            .solve(
                &spot,
                &SolveOptions {
                    target_pct: 5.0,
                    max_iterations: 20,
                    check_every: 10,
                },
            )
            .unwrap();
        // Taful node → postflop-solver history (same tree, DFS).
        let mut hist = vec![Vec::new(); sol.nodes.len()];
        let mut oop_unbet = vec![false; sol.nodes.len()];
        oop_unbet[0] = true;
        for i in 0..sol.nodes.len() {
            let n = &sol.nodes[i];
            for (a, c) in n.children.iter().enumerate() {
                if let Some(c) = *c {
                    let mut h = hist[i].clone();
                    h.push(a);
                    hist[c] = h;
                    oop_unbet[c] = oop_unbet[i] && !(n.player == 0 && n.actions[a].is_aggressive());
                }
            }
        }
        let index = |p: usize| -> HashMap<Hand, usize> {
            g.private_cards(p)
                .iter()
                .enumerate()
                .map(|(i, &(a, b))| {
                    (
                        Hand::new(Card::from_index(a).unwrap(), Card::from_index(b).unwrap())
                            .unwrap(),
                        i,
                    )
                })
                .collect()
        };
        let (ix0, ix1) = (index(0), index(1));
        let opp: Vec<Vec<f32>> = sol
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                if n.player != 1 {
                    return Vec::new();
                }
                assert_eq!(
                    shape.actions[&hist[i].iter().map(|&a| a as u8).collect::<Vec<_>>()]
                        .iter()
                        .map(|x| x.0)
                        .collect::<Vec<_>>(),
                    n.actions,
                    "same tree"
                );
                let Some(Recorded::Bot { strategy, .. }) = rec.get(&hist[i]) else {
                    panic!("node {i}")
                };
                let ns = g.private_cards(1).len();
                let len = sol.hands[1].len();
                let mut t = vec![0.0; n.actions.len() * len];
                for (j, h) in sol.hands[1].iter().enumerate() {
                    for a in 0..n.actions.len() {
                        t[a * len + j] = strategy[a * ns + ix1[h]];
                    }
                }
                t
            })
            .collect();
        let detail = best_response_detail(&spot, &sol, 0, &opp).unwrap();
        let mut checked = 0;
        let mut max_diff = 0.0f32;
        for (i, n) in sol.nodes.iter().enumerate() {
            let (0, true, Some(theirs)) = (n.player, oop_unbet[i], &detail[i]) else {
                continue;
            };
            let Some(Recorded::Responder { ev }) = rec.get(&hist[i]) else {
                panic!("node {i}")
            };
            let ns = g.private_cards(0).len();
            let len = sol.hands[0].len();
            for (j, h) in sol.hands[0].iter().enumerate() {
                for a in 0..n.actions.len() {
                    if n.actions[a] == Action::Fold {
                        continue;
                    }
                    let mine = ev[a * ns + ix0[h]] * BB_CHIPS as f32;
                    let d = (mine - theirs[a * len + j]).abs();
                    max_diff = max_diff.max(d);
                    checked += 1;
                }
            }
        }
        eprintln!("{checked} EVs, max difference {max_diff} chips");
        assert!(checked > 200, "{checked}");
        assert!(
            max_diff < 0.05,
            "max EV difference {max_diff} chips (pot {pot})"
        );
    }
}

//! Flop HUD stats of one bot over the postflop library: flop c-bet, fold to
//! a flop c-bet and flop aggression.
//!
//! Only the flop of each stored tree is read (turn subtrees are left out),
//! and the bot's strategy is recomputed from the stored reference strategy
//! and EVs with the profile under test, so a profile can be measured and
//! tuned without re-solving. Its range at the flop is its own preflop range
//! for the spot's line (D15), recomputed from the preflop chart.
//!
//! Weights: within a file every hand the bot brings to the flop counts
//! once; files of a spot count by the number of real flops in their orbit;
//! a spot counts by how often the bot plays the spot's line from that seat
//! against reference players (one hand per seat, as preflop).

use std::path::Path;

use taful_core::action::{Action, Line};
use taful_core::cards::{Card, Hand, NUM_COMBOS};
use taful_core::position::Position;
use taful_core::profile::Profile;
use taful_core::range::HandClass;
use taful_solver::format::{Child, LibraryFile, Node, ProfileSection};
use taful_solver::postflop::profile_line_range;
use taful_solver::preflop::{Lookup, PreflopChart};

/// The flop of one library file, kept in memory for repeated measurements.
pub struct FlopTree {
    pub spot: String,
    pub flop: String,
    /// Real flops in the canonical flop's orbit.
    pub orbit: f64,
    line: Line,
    /// The last raiser preflop.
    aggressor: Position,
    /// Reference root range per position, combo weights 0..=1.
    roots: Vec<(Position, Vec<f32>)>,
    dead: [bool; NUM_COMBOS],
    ev_scale: f32,
    /// Flop nodes by file index (`None` = a turn or later node).
    nodes: Vec<Option<Node>>,
    /// Profile data the build stored (for checks).
    pub sections: Vec<ProfileSection>,
}

impl FlopTree {
    pub fn load(path: &Path, spot: &str, flop: &str, orbit: f64) -> Result<FlopTree, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = LibraryFile::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        FlopTree::new(file, spot, flop, orbit)
    }

    pub fn new(file: LibraryFile, spot: &str, flop: &str, orbit: f64) -> Result<FlopTree, String> {
        let line: Line = file.header.line.parse()?;
        let aggressor = line
            .0
            .iter()
            .rev()
            .find(|(_, a)| a.is_aggressive())
            .map(|(p, _)| *p)
            .ok_or("line has no raise")?;
        let board: Vec<Card> = file.header.board.clone();
        let mut dead = [false; NUM_COMBOS];
        for h in Hand::all() {
            dead[h.combo()] = board.iter().any(|c| h.mask().contains(*c));
        }
        let roots = file
            .header
            .root_ranges
            .iter()
            .map(|(p, r)| (*p, r.iter().map(|&x| f32::from(x) / 255.0).collect()))
            .collect();
        // Keep the flop: nodes reachable from the root without a deal.
        let mut keep = vec![false; file.nodes.len()];
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            keep[i] = true;
            for a in &file.nodes[i].actions {
                if let Child::Node(c) = a.child {
                    stack.push(c as usize);
                }
            }
        }
        let ev_scale = file.header.ev_scale;
        let nodes: Vec<Option<Node>> = file
            .nodes
            .into_iter()
            .zip(keep)
            .map(|(n, k)| k.then_some(n))
            .collect();
        let sections = file
            .profiles
            .into_iter()
            .map(|mut s| {
                // Only flop nodes matter for the checks.
                for (i, pn) in s.nodes.iter_mut().enumerate() {
                    if !nodes_keep(&nodes, i) {
                        pn.live.clear();
                        pn.strategy.clear();
                        pn.ev.clear();
                    }
                }
                s
            })
            .collect();
        Ok(FlopTree {
            spot: spot.into(),
            flop: flop.into(),
            orbit,
            line,
            aggressor,
            roots,
            dead,
            ev_scale,
            nodes,
            sections,
        })
    }

    /// The two players, out of position first.
    pub fn line(&self) -> &Line {
        &self.line
    }

    pub fn players(&self) -> [Position; 2] {
        [self.roots[0].0, self.roots[1].0]
    }

    fn root(&self, pos: Position) -> &[f32] {
        &self
            .roots
            .iter()
            .find(|(p, _)| *p == pos)
            .expect("player")
            .1
    }
}

fn nodes_keep(nodes: &[Option<Node>], i: usize) -> bool {
    nodes.get(i).is_some_and(Option::is_some)
}

/// Raw sums of one measurement; add them up, then read rates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlopSums {
    pub cbet_opp: f64,
    pub cbet: f64,
    pub fcb_opp: f64,
    pub fcb: f64,
    /// Bets and raises on the flop.
    pub aggr: f64,
    /// Calls on the flop.
    pub calls: f64,
    /// Checks and folds on the flop (for the aggression frequency).
    pub passive_other: f64,
    /// Share of the bot's flop mass at combos the file has no data for.
    pub missing: f64,
}

impl FlopSums {
    pub fn add(&mut self, o: &FlopSums, w: f64) {
        self.cbet_opp += w * o.cbet_opp;
        self.cbet += w * o.cbet;
        self.fcb_opp += w * o.fcb_opp;
        self.fcb += w * o.fcb;
        self.aggr += w * o.aggr;
        self.calls += w * o.calls;
        self.passive_other += w * o.passive_other;
        self.missing += w * o.missing;
    }
    pub fn cbet_pct(&self) -> Option<f64> {
        (self.cbet_opp > 1e-12).then(|| 100.0 * self.cbet / self.cbet_opp)
    }
    pub fn fold_to_cbet_pct(&self) -> Option<f64> {
        (self.fcb_opp > 1e-12).then(|| 100.0 * self.fcb / self.fcb_opp)
    }
    /// Aggression factor: bets and raises per call.
    pub fn af(&self) -> Option<f64> {
        (self.calls > 1e-12).then(|| self.aggr / self.calls)
    }
}

/// One file, the bot in seat `pos` playing `profile` (`None` = reference),
/// its flop range from the published preflop chart `chart` (D15). Mass is
/// normalized so the bot's hands at the flop add up to 1.
pub fn measure(
    tree: &FlopTree,
    pos: Position,
    profile: Option<&Profile>,
    chart: &PreflopChart,
) -> Result<FlopSums, String> {
    let mut bot: Vec<f32> = profile_line_range(chart, &tree.line, pos, profile)?;
    for (h, w) in bot.iter_mut().enumerate() {
        if tree.dead[h] {
            *w = 0.0;
        }
    }
    let total: f64 = bot.iter().map(|&w| f64::from(w)).sum();
    if total <= 0.0 {
        return Err(format!(
            "{} {}: empty range for {pos}",
            tree.spot, tree.flop
        ));
    }
    bot.iter_mut()
        .for_each(|w| *w = (f64::from(*w) / total) as f32);
    let opp_pos = tree
        .players()
        .into_iter()
        .find(|&p| p != pos)
        .ok_or("seat")?;
    let opp = tree.root(opp_pos).to_vec();
    let opp_total: f64 = opp.iter().map(|&w| f64::from(w)).sum();
    let mut sums = FlopSums::default();
    let mut ctx = Ctx {
        tree,
        pos,
        profile,
        aggressor: tree.aggressor == pos,
        opp_total,
        sums: &mut sums,
    };
    ctx.visit(0, &bot, &opp, St::default());
    Ok(sums)
}

#[derive(Clone, Copy, Default)]
struct St {
    facing: usize,
    bot_acted: bool,
    bot_aggressive: bool,
}

struct Ctx<'a> {
    tree: &'a FlopTree,
    pos: Position,
    profile: Option<&'a Profile>,
    aggressor: bool,
    opp_total: f64,
    sums: &'a mut FlopSums,
}

impl Ctx<'_> {
    fn visit(&mut self, id: usize, bot: &[f32], opp: &[f32], st: St) {
        let Some(node) = self.tree.nodes[id].as_ref() else {
            return;
        };
        let k = node.actions.len();
        let actions: Vec<Action> = node.actions.iter().map(|a| a.action).collect();
        let next = |a: usize| match node.actions[a].child {
            Child::Node(c) if nodes_keep(&self.tree.nodes, c as usize) => Some(c as usize),
            _ => None,
        };
        let step = |st: St, a: usize, by_bot: bool| St {
            facing: st.facing + usize::from(actions[a].is_aggressive()),
            bot_acted: st.bot_acted || by_bot,
            bot_aggressive: st.bot_aggressive || (by_bot && actions[a].is_aggressive()),
        };
        if node.player != self.pos {
            for a in 0..k {
                let Some(c) = next(a) else { continue };
                let o: Vec<f32> = (0..NUM_COMBOS)
                    .map(|h| opp[h] * node.strategy(h).map_or(0.0, |s| s[a]))
                    .collect();
                if o.iter().all(|&w| w <= 0.0) {
                    continue;
                }
                self.visit(c, bot, &o, step(st, a, false));
            }
            return;
        }
        let others = opp.iter().map(|&w| f64::from(w)).sum::<f64>() / self.opp_total;
        let mut weights = bot.to_vec();
        let mut missing = 0.0f64;
        let mut strategy = vec![0.0f32; NUM_COMBOS * k];
        let mut ev = vec![0.0f32; NUM_COMBOS * k];
        for h in 0..NUM_COMBOS {
            if weights[h] <= 0.0 {
                continue;
            }
            match (node.strategy(h), node.ev(h, self.tree.ev_scale)) {
                (Some(s), Some(e)) => {
                    strategy[h * k..(h + 1) * k].copy_from_slice(&s);
                    ev[h * k..(h + 1) * k].copy_from_slice(&e);
                }
                _ => {
                    missing += f64::from(weights[h]);
                    weights[h] = 0.0;
                }
            }
        }
        let s = match self.profile {
            Some(p) => p.postflop_strategy(&actions, &strategy, &ev, &weights, st.facing),
            None => strategy,
        };
        let mass: Vec<f64> = weights.iter().map(|&w| f64::from(w) * others).collect();
        let total: f64 = mass.iter().sum();
        self.sums.missing += missing * others;
        if total <= 1e-15 {
            return;
        }
        let share = |pick: &dyn Fn(Action) -> bool| -> f64 {
            (0..NUM_COMBOS)
                .filter(|&h| mass[h] > 0.0)
                .map(|h| {
                    mass[h]
                        * (0..k)
                            .filter(|&a| pick(actions[a]))
                            .map(|a| f64::from(s[h * k + a]))
                            .sum::<f64>()
                })
                .sum()
        };
        let aggr = share(&Action::is_aggressive);
        let calls = share(&|a| a == Action::Call);
        self.sums.aggr += aggr;
        self.sums.calls += calls;
        self.sums.passive_other += total - aggr - calls;
        if self.aggressor && st.facing == 0 && !st.bot_acted {
            self.sums.cbet_opp += total;
            self.sums.cbet += aggr;
        }
        if !self.aggressor && st.facing == 1 && !st.bot_aggressive {
            self.sums.fcb_opp += total;
            self.sums.fcb += share(&|a| a == Action::Fold);
        }
        for a in 0..k {
            let Some(c) = next(a) else { continue };
            let b: Vec<f32> = (0..NUM_COMBOS).map(|h| weights[h] * s[h * k + a]).collect();
            if b.iter().all(|&w| w <= 0.0) {
                continue;
            }
            self.visit(c, &b, opp, step(st, a, true));
        }
    }
}

/// How often the bot in seat `pos` plays its part of `line` and the
/// reference players theirs (one hand in that seat).
pub fn line_probability(
    chart: &PreflopChart,
    line: &Line,
    pos: Position,
    profile: Option<&Profile>,
) -> Result<f64, String> {
    let mut reach = vec![1.0f64; HandClass::COUNT];
    let mut others = 1.0f64;
    let mut at = Lookup::Node(chart.root());
    for (i, &(p, action)) in line.0.iter().enumerate() {
        let Lookup::Node(id) = at else {
            return Err(format!("line leaves the chart before action {i}"));
        };
        let node = chart.node(id);
        let a = node
            .action_index(action)
            .ok_or_else(|| format!("{p} {action} is not in the chart"))?;
        let k = node.actions.len();
        if p == pos {
            let s: Vec<f32> = match profile {
                Some(prof) => chart.profile_strategy(id, prof),
                None => (0..HandClass::COUNT)
                    .flat_map(|c| node.strategy(c).unwrap_or_else(|| vec![0.0; k]))
                    .collect(),
            };
            for (c, r) in reach.iter_mut().enumerate() {
                *r *= f64::from(s[c * k + a]);
            }
        } else {
            let range = chart.acting_range(id);
            others *= (0..HandClass::COUNT)
                .map(|c| f64::from(range[c]) * node.strategy(c).map_or(0.0, |s| f64::from(s[a])))
                .sum::<f64>();
        }
        at = chart.step(id, action);
    }
    let bot: f64 = HandClass::all()
        .map(|c| c.num_combos() as f64 / 1326.0 * reach[c.index()])
        .sum();
    Ok(bot * others)
}

/// How far the bot strategy this module computes is from the one the build
/// stored, over the flop nodes the bot acts in (for `section`'s own profile
/// and root ranges): the largest and the range-weighted mean share of a
/// hand's frequencies that differ. A check that the recomputation matches
/// the build (which worked on unquantized solver data).
pub fn diff_vs_stored(tree: &FlopTree, section: &ProfileSection) -> Result<(f32, f64), String> {
    let profile = Profile::from_toml(&section.profile)?;
    let mut worst = 0.0f32;
    let (mut sum, mut mass) = (0.0f64, 0.0f64);
    for pos in tree.players() {
        let mut bot: Vec<f32> = if section.root_ranges.is_empty() {
            tree.root(pos).to_vec()
        } else {
            section
                .root_ranges
                .iter()
                .find(|(p, _)| *p == pos)
                .ok_or("section root range")?
                .1
                .iter()
                .map(|&x| f32::from(x) / 255.0)
                .collect()
        };
        for (h, w) in bot.iter_mut().enumerate() {
            if tree.dead[h] {
                *w = 0.0;
            }
        }
        let mut stack = vec![(0usize, bot, 0usize)];
        while let Some((id, bot, facing)) = stack.pop() {
            let Some(node) = tree.nodes[id].as_ref() else {
                continue;
            };
            let k = node.actions.len();
            let actions: Vec<Action> = node.actions.iter().map(|a| a.action).collect();
            let mut next_bot: Vec<Vec<f32>> = vec![bot.clone(); k];
            if node.player == pos {
                let mut weights = bot.clone();
                let mut strategy = vec![0.0f32; NUM_COMBOS * k];
                let mut ev = vec![0.0f32; NUM_COMBOS * k];
                for h in 0..NUM_COMBOS {
                    match (node.strategy(h), node.ev(h, tree.ev_scale)) {
                        (Some(s), Some(e)) if weights[h] > 0.0 => {
                            strategy[h * k..(h + 1) * k].copy_from_slice(&s);
                            ev[h * k..(h + 1) * k].copy_from_slice(&e);
                        }
                        _ => weights[h] = 0.0,
                    }
                }
                let s = profile.postflop_strategy(&actions, &strategy, &ev, &weights, facing);
                let pn = &section.nodes[id];
                let w_total: f32 = weights.iter().sum();
                for &row in &pn.live {
                    let h = usize::from(row);
                    if weights[h] <= 1e-4 * w_total {
                        continue;
                    }
                    let stored = pn.strategy(h, k).expect("live row");
                    let d: f32 = (0..k)
                        .map(|a| (stored[a] - s[h * k + a]).abs())
                        .sum::<f32>()
                        / 2.0;
                    worst = worst.max(d);
                    sum += f64::from(weights[h] / w_total * d);
                    mass += f64::from(weights[h] / w_total);
                }
                for (a, nb) in next_bot.iter_mut().enumerate() {
                    for h in 0..NUM_COMBOS {
                        nb[h] = weights[h] * s[h * k + a];
                    }
                }
            }
            for (a, nb) in next_bot.into_iter().enumerate() {
                if let Child::Node(c) = node.actions[a].child {
                    stack.push((
                        c as usize,
                        nb,
                        facing + usize::from(actions[a].is_aggressive()),
                    ));
                }
            }
        }
    }
    Ok((worst, if mass > 0.0 { sum / mass } else { 0.0 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/taful-app/tests/fixtures/postflop-profiles/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/btn-sb-3bp"
    );

    fn published() -> PreflopChart {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
        );
        PreflopChart::decode(&std::fs::read(path).expect("chart")).expect("decodes")
    }

    fn tree(flop: &str) -> FlopTree {
        FlopTree::load(
            &Path::new(FIXTURE).join(format!("{flop}.tafl")),
            "btn-sb-3bp",
            flop,
            24.0,
        )
        .expect("loads")
    }

    fn neutral() -> Profile {
        let mut p = Profile::from_toml(include_str!("../../../content/bots/tag.toml")).unwrap();
        p.preflop.vpip_mult = 1.0;
        p.preflop.pfr_mult = 1.0;
        p.preflop.threebet_mult = 1.0;
        p.postflop.fold_to_cbet = 0.0;
        p.postflop.fold_to_raise = 0.0;
        p.postflop.bluff_value_shift = 0.0;
        p.postflop.aggression = 0.0;
        p.sizing.prefer = taful_core::profile::Sizing::Reference;
        p
    }

    #[test]
    fn neutral_profile_measures_like_the_reference() {
        let chart = published();
        let t = tree("Tc9c6d");
        for pos in t.players() {
            let r = measure(&t, pos, None, &chart).unwrap();
            let n = measure(&t, pos, Some(&neutral()), &chart).unwrap();
            assert!(
                (r.aggr - n.aggr).abs() < 1e-3 && (r.calls - n.calls).abs() < 1e-3,
                "{r:?} {n:?}"
            );
            assert!(r.missing < 1e-3, "reference has data for its own range");
        }
    }

    #[test]
    fn roles_get_the_right_opportunities() {
        let chart = published();
        let t = tree("Kc7c2c");
        // btn-sb-3bp: SB 3-bet, so SB c-bets and BTN faces the c-bet.
        let sb = measure(&t, Position::Sb, None, &chart).unwrap();
        let btn = measure(&t, Position::Btn, None, &chart).unwrap();
        assert!(
            (sb.cbet_opp - 1.0).abs() < 1e-6,
            "OOP aggressor acts first: {sb:?}"
        );
        assert_eq!(sb.fcb_opp, 0.0);
        assert_eq!(btn.cbet_opp, 0.0);
        assert!(btn.fcb_opp > 0.3 && btn.fcb_opp <= 1.0, "{btn:?}");
    }

    #[test]
    fn knobs_move_the_flop_stats() {
        let chart = published();
        let t = tree("Tc9c6d");
        let m = |f: f64, a: f64, pos| {
            let mut p = neutral();
            p.postflop.fold_to_cbet = f;
            p.postflop.aggression = a;
            measure(&t, pos, Some(&p), &chart).unwrap()
        };
        let base = m(0.0, 0.0, Position::Btn);
        assert!(m(0.5, 0.0, Position::Btn).fold_to_cbet_pct() > base.fold_to_cbet_pct());
        assert!(m(-0.5, 0.0, Position::Btn).fold_to_cbet_pct() < base.fold_to_cbet_pct());
        let base = m(0.0, 0.0, Position::Sb);
        assert!(m(0.0, 0.5, Position::Sb).af() > base.af());
        assert!(m(0.0, -0.5, Position::Sb).cbet_pct() < base.cbet_pct());
    }

    #[test]
    fn recomputed_strategy_is_close_to_what_the_build_stored() {
        // The fixture predates seeding, so loose profiles miss data at many
        // combos; the tight ones show the recomputation itself is faithful.
        let t = tree("Tc9c6d");
        for s in t.sections.iter().filter(|s| s.id == "nit" || s.id == "tag") {
            let (_, mean) = diff_vs_stored(&t, s).unwrap();
            assert!(mean < 0.01, "{}: {mean}", s.id);
        }
    }

    #[test]
    fn line_probability_of_a_blind_battle() {
        let chart = published();
        let line: Line = "UTG:f,HJ:f,CO:f,BTN:f,SB:r300,BB:c".parse().unwrap();
        let p = line_probability(&chart, &line, Position::Bb, None).unwrap();
        // Everyone folds to SB a fair share of the time; SB opens and BB calls.
        assert!(p > 0.01 && p < 0.2, "{p}");
    }
}

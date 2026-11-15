//! Preflop HUD stats of one bot, computed exactly from the unpruned preflop
//! chart (no sampling noise).
//!
//! The bot sits in each seat in turn at a table of five reference players and
//! plays one hand per seat. Every line of the chart is walked once with the
//! bot's reach per hand class and the probability that the reference players
//! take their part of the line (their range at the node is the reference's,
//! card removal between players is ignored). A hand's weight is
//! `combos / 1326 × bot reach × others' probability`, so the six seats add up
//! to six hands.

use taful_core::action::Action;
use taful_core::position::Position;
use taful_core::profile::Profile;
use taful_core::range::HandClass;
use taful_solver::format::Child;
use taful_solver::preflop::{NodeId, PreflopChart};

/// Profile-independent data of every chart node, computed once.
pub struct Prepared<'a> {
    pub chart: &'a PreflopChart,
    /// Reference strategy, `169 × actions`.
    strategy: Vec<Vec<f32>>,
    /// Probability that the reference player to act takes each action
    /// (its range at the node is the reference's).
    action_prob: Vec<Vec<f64>>,
}

impl<'a> Prepared<'a> {
    pub fn new(chart: &'a PreflopChart) -> Prepared<'a> {
        let mut strategy = Vec::with_capacity(chart.len());
        let mut action_prob = Vec::with_capacity(chart.len());
        for id in 0..chart.len() {
            let node = chart.node(id);
            let k = node.actions.len();
            let s: Vec<f32> = (0..HandClass::COUNT)
                .flat_map(|c| node.strategy(c).unwrap_or_else(|| vec![0.0; k]))
                .collect();
            let range = chart.acting_range(id);
            let p: Vec<f64> = (0..k)
                .map(|a| {
                    (0..HandClass::COUNT)
                        .map(|c| f64::from(range[c]) * f64::from(s[c * k + a]))
                        .sum()
                })
                .collect();
            strategy.push(s);
            action_prob.push(p);
        }
        Prepared {
            chart,
            strategy,
            action_prob,
        }
    }
}

/// Rates in percent; `None` when the bot never had the chance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreflopStats {
    /// Voluntarily put money in the pot (any call or raise), share of hands.
    pub vpip: f64,
    /// Raised at least once, share of hands.
    pub pfr: f64,
    /// Re-raised when facing exactly one raise (an open), share of chances.
    pub threebet: Option<f64>,
    /// Folded to a 3-bet after opening, share of chances.
    pub fold_to_3bet: Option<f64>,
    /// Opened when everyone folded to it, share of chances (UTG–SB).
    pub rfi: Option<f64>,
}

#[derive(Default)]
struct Acc {
    hands: f64,
    vpip: f64,
    pfr: f64,
    tb_opp: f64,
    tb: f64,
    f3_opp: f64,
    f3: f64,
    rfi_opp: f64,
    rfi: f64,
}

#[derive(Clone, Copy)]
struct Path {
    /// Raises made so far (any player).
    raises: u32,
    /// The bot has acted (and is still in).
    acted: bool,
    /// The bot made a raise.
    raised: bool,
    /// The bot made the first raise (it opened).
    opener: bool,
}

/// Stats of `profile` (`None` = the reference) over one hand per seat.
pub fn stats(prep: &Prepared<'_>, profile: Option<&Profile>) -> PreflopStats {
    let chart = prep.chart;
    let mut cache: Vec<Option<Vec<f32>>> = vec![None; chart.len()];
    let table = usize::from(chart.config().table_size);
    let mut acc = Acc::default();
    let combos: Vec<f64> = HandClass::all()
        .map(|c| c.num_combos() as f64 / 1326.0)
        .collect();
    for seat in Position::by_button_offset(table) {
        let reach = vec![1.0f64; HandClass::COUNT];
        let path = Path {
            raises: 0,
            acted: false,
            raised: false,
            opener: false,
        };
        walk(
            prep,
            profile,
            &mut cache,
            seat,
            chart.root(),
            &reach,
            1.0,
            path,
            &combos,
            &mut acc,
        );
        acc.hands += 1.0;
    }
    let pct = |n: f64, d: f64| if d > 1e-12 { Some(100.0 * n / d) } else { None };
    PreflopStats {
        vpip: 100.0 * acc.vpip / acc.hands,
        pfr: 100.0 * acc.pfr / acc.hands,
        threebet: pct(acc.tb, acc.tb_opp),
        fold_to_3bet: pct(acc.f3, acc.f3_opp),
        rfi: pct(acc.rfi, acc.rfi_opp),
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    prep: &Prepared<'_>,
    profile: Option<&Profile>,
    cache: &mut Vec<Option<Vec<f32>>>,
    seat: Position,
    id: NodeId,
    reach: &[f64],
    others: f64,
    path: Path,
    combos: &[f64],
    acc: &mut Acc,
) {
    if others <= 1e-12 {
        return;
    }
    let chart = prep.chart;
    let node = chart.node(id);
    let k = node.actions.len();
    let aggressive: Vec<bool> = node
        .actions
        .iter()
        .map(|a| a.action.is_aggressive())
        .collect();
    let fold = node.actions.iter().position(|a| a.action == Action::Fold);
    let next = |a: usize| match node.actions[a].child {
        Child::Node(c) => Some(c as usize),
        _ => None,
    };
    if node.player == seat {
        if cache[id].is_none() {
            cache[id] = Some(match profile {
                Some(p) => chart.profile_strategy(id, p),
                None => prep.strategy[id].clone(),
            });
        }
        let strategy = cache[id].clone().expect("cached");
        let mass: Vec<f64> = (0..HandClass::COUNT)
            .map(|c| combos[c] * reach[c] * others)
            .collect();
        let total: f64 = mass.iter().sum();
        if total <= 1e-15 {
            return;
        }
        let share = |pick: &dyn Fn(usize) -> bool| -> f64 {
            (0..HandClass::COUNT)
                .map(|c| {
                    mass[c]
                        * (0..k)
                            .filter(|&a| pick(a))
                            .map(|a| f64::from(strategy[c * k + a]))
                            .sum::<f64>()
                })
                .sum()
        };
        let raise = share(&|a| aggressive[a]);
        let folds = fold.map_or(0.0, |f| share(&|a| a == f));
        if !path.acted {
            acc.vpip += total - folds;
            if path.raises == 0 {
                acc.rfi_opp += total;
                acc.rfi += raise;
            }
        }
        if !path.raised {
            acc.pfr += raise;
        }
        if path.raises == 1 && !path.raised {
            acc.tb_opp += total;
            acc.tb += raise;
        }
        if path.raises == 2 && path.opener {
            acc.f3_opp += total;
            acc.f3 += folds;
        }
        for a in 0..k {
            if Some(a) == fold {
                continue;
            }
            let Some(c) = next(a) else { continue };
            let r: Vec<f64> = (0..HandClass::COUNT)
                .map(|h| reach[h] * f64::from(strategy[h * k + a]))
                .collect();
            let p = Path {
                raises: path.raises + u32::from(aggressive[a]),
                acted: true,
                raised: path.raised || aggressive[a],
                opener: path.opener || (aggressive[a] && path.raises == 0),
            };
            walk(prep, profile, cache, seat, c, &r, others, p, combos, acc);
        }
    } else {
        for a in 0..k {
            let Some(c) = next(a) else { continue };
            let p_a = prep.action_prob[id][a];
            let p = Path {
                raises: path.raises + u32::from(aggressive[a]),
                ..path
            };
            walk(
                prep,
                profile,
                cache,
                seat,
                c,
                reach,
                others * p_a,
                p,
                combos,
                acc,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taful_core::profile::{PostflopDeviation, PreflopDeviation, Sizing, SizingPreference};

    pub(crate) fn chart() -> PreflopChart {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop-full.tafl"
        );
        PreflopChart::decode(&std::fs::read(path).expect("chart")).expect("decodes")
    }

    fn profile(vpip: f64, pfr: f64, threebet: f64) -> Profile {
        Profile {
            id: "test".into(),
            preflop: PreflopDeviation {
                vpip_mult: vpip,
                pfr_mult: pfr,
                threebet_mult: threebet,
            },
            postflop: PostflopDeviation {
                fold_to_cbet: 0.0,
                fold_to_raise: 0.0,
                bluff_value_shift: 0.0,
                aggression: 0.0,
            },
            sizing: SizingPreference {
                prefer: Sizing::Reference,
            },
        }
    }

    #[test]
    fn neutral_profile_measures_like_the_reference() {
        let c = chart();
        let prep = Prepared::new(&c);
        let r = stats(&prep, None);
        let n = stats(&prep, Some(&profile(1.0, 1.0, 1.0)));
        for (a, b) in [
            (r.vpip, n.vpip),
            (r.pfr, n.pfr),
            (r.threebet.unwrap(), n.threebet.unwrap()),
            (r.fold_to_3bet.unwrap(), n.fold_to_3bet.unwrap()),
        ] {
            assert!((a - b).abs() < 0.3, "{a} vs {b}");
        }
        // A 6-max reference opens about a quarter of its chances.
        assert!((15.0..35.0).contains(&r.vpip), "{r:?}");
        assert!(r.pfr < r.vpip && r.rfi.unwrap() > r.pfr, "{r:?}");
    }

    #[test]
    fn each_knob_moves_its_stat() {
        let c = chart();
        let prep = Prepared::new(&c);
        let base = stats(&prep, Some(&profile(1.0, 1.0, 1.0)));
        let wide = stats(&prep, Some(&profile(2.0, 1.0, 1.0)));
        assert!(wide.vpip > base.vpip + 3.0, "{base:?} {wide:?}");
        assert!(wide.fold_to_3bet.unwrap() < base.fold_to_3bet.unwrap());
        let opens = stats(&prep, Some(&profile(1.0, 0.5, 1.0)));
        assert!(opens.pfr < base.pfr - 3.0 && opens.rfi.unwrap() < base.rfi.unwrap());
        let tb = stats(&prep, Some(&profile(1.0, 1.0, 2.0)));
        assert!(tb.threebet.unwrap() > base.threebet.unwrap() + 3.0);
    }
}

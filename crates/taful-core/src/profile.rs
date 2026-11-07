//! Bot profiles (spec §9, D8): deviation parameters applied as a transform
//! over the reference strategy.
//!
//! A profile is data (`content/bots/<id>.toml`). The transform is a pure
//! function of one decision node: the node's actions, the reference strategy
//! and EVs per row (hand class preflop, combo postflop) and the reference
//! range at the node (combo weight per row). It needs no RNG, so a bot that
//! samples the transformed strategy with a seeded RNG is deterministic, and
//! the build tool and the app compute exactly the same strategy from the same
//! chart data (the build tool computes best responses against it, D12).
//!
//! **Preflop:**
//!
//! 1. *Entering the pot.* The share of the range that does not fold is scaled
//!    by `pfr_mult` at an unopened node (the tree has no limps, so opening is
//!    the only way in) and by `vpip_mult` at every node facing a raise
//!    (cold calls, defending the blinds, and also continuing against a
//!    3-bet or 4-bet: a station folds too little to aggression at every
//!    level, a nit too much). Widening adds the
//!    folding part of the next-best hands first, by reference EV of the best
//!    non-fold action over folding; narrowing drops the worst continuing
//!    hands first. Only the boundary hand ends up mixed by the transform;
//!    the reference's own mixing is kept elsewhere.
//! 2. *Re-raising.* Facing a raise, the share of the range that re-raises
//!    (3-bet, squeeze, 4-bet and up) is scaled by `threebet_mult` within the
//!    continuing hands, moving call ↔ raise by reference EV of raising over
//!    calling, best raises first (worst first when narrowing).
//! 3. Every row is renormalized.
//!
//! Mass added to a hand's action group follows that hand's reference split
//! between the group's actions, or goes to the group's best-EV action when
//! the reference never took it.
//!
//! **Postflop** (#27; rows are combos, `weights` the bot's range at the
//! node, `facing` the bets and raises already made on the street):
//!
//! 1. *Folding.* Facing a bet (the first bet of the street, so c-bets,
//!    donks and probes alike) the fold share `f` of the range moves to
//!    `f + fold_to_cbet × (1 − f)` when the shift is positive and to
//!    `f × (1 + fold_to_cbet)` when it is negative; facing a raise
//!    `fold_to_raise` does the same. Hands move by reference EV of their
//!    best continuing action over folding, as preflop.
//! 2. *Aggression.* Among the hands that do not fold, the share that bets
//!    or raises moves toward all of them (`aggression > 0`) or none (`< 0`)
//!    the same way, checks and calls ↔ bets and raises, by reference EV of
//!    the best aggressive action over the best passive one.
//! 3. *Bluff/value.* The aggressive share stays; each hand's aggressive
//!    mass moves `|bluff_value_shift|` of the way toward an allocation that
//!    gives it to the strongest hands first (`> 0`, value-heavy) or the
//!    weakest first (`< 0`, bluff-heavy). Strength is the hand's reference
//!    EV at the node.
//! 4. *Sizing.* With two or more bet or raise sizes (all-in aside), every
//!    hand moves [`SIZING_SHARE`] of its mass on the other sizes to the
//!    smallest (`small`) or largest (`large`) one.
//! 5. Every row is renormalized.
//!
//! The build tool runs this at every node the bot acts in a solved tree and
//! stores the result for the flop and turn (#27); the app runs it on rivers
//! it solves on demand.

use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::config::Chips;

/// Multipliers are clamped to this range by validation.
pub const MAX_MULT: f64 = 5.0;

/// A bot profile (`content/bots/<id>.toml`). Its display name is the Fluent
/// message `bot-profile-<id>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub preflop: PreflopDeviation,
    pub postflop: PostflopDeviation,
    pub sizing: SizingPreference,
}

/// Preflop range-width multipliers (1 = reference).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflopDeviation {
    /// Share of the range that continues (calls or re-raises) facing a raise.
    pub vpip_mult: f64,
    /// Share of the range that opens an unopened pot.
    pub pfr_mult: f64,
    /// Share of the range that re-raises facing a raise (3-bet and up).
    pub threebet_mult: f64,
}

/// Postflop shifts, each in `-1..=1` (0 = reference); see the module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostflopDeviation {
    pub fold_to_cbet: f64,
    pub fold_to_raise: f64,
    pub bluff_value_shift: f64,
    pub aggression: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizingPreference {
    pub prefer: Sizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sizing {
    Reference,
    Small,
    Large,
}

impl Profile {
    pub fn from_toml(s: &str) -> Result<Profile, String> {
        let p: Profile = toml::from_str(s).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    /// Canonical TOML (what the build tool hashes).
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("profile serializes")
    }

    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !self.id.starts_with('-')
            && !self.id.ends_with('-');
        if !id_ok {
            return Err(format!(
                "id `{}`: lowercase letters, digits and `-`",
                self.id
            ));
        }
        if self.id == REFERENCE {
            return Err(format!("id `{REFERENCE}` is the reference bot"));
        }
        let p = &self.preflop;
        for (name, v) in [
            ("vpip_mult", p.vpip_mult),
            ("pfr_mult", p.pfr_mult),
            ("threebet_mult", p.threebet_mult),
        ] {
            if !(v.is_finite() && v > 0.0 && v <= MAX_MULT) {
                return Err(format!("preflop.{name} = {v}: must be in (0, {MAX_MULT}]"));
            }
        }
        let q = &self.postflop;
        for (name, v) in [
            ("fold_to_cbet", q.fold_to_cbet),
            ("fold_to_raise", q.fold_to_raise),
            ("bluff_value_shift", q.bluff_value_shift),
            ("aggression", q.aggression),
        ] {
            if !(v.is_finite() && (-1.0..=1.0).contains(&v)) {
                return Err(format!("postflop.{name} = {v}: must be in -1..=1"));
            }
        }
        Ok(())
    }

    /// The profiled strategy at a preflop node (see the module docs).
    ///
    /// `strategy` and `ev` are row-major, `rows × actions.len()`; `weights`
    /// is the reference range at the node, one combo weight per row (any
    /// scale). Returns a strategy of the same shape whose rows sum to 1.
    pub fn preflop_strategy(
        &self,
        actions: &[Action],
        strategy: &[f32],
        ev: &[f32],
        weights: &[f32],
    ) -> Vec<f32> {
        let k = actions.len();
        let n = weights.len();
        assert_eq!(strategy.len(), n * k, "strategy shape");
        assert_eq!(ev.len(), n * k, "ev shape");
        let mut s: Vec<f32> = strategy.to_vec();
        renormalize(&mut s, k);
        let fold = actions.iter().position(|a| *a == Action::Fold);
        let call = actions.iter().position(|a| *a == Action::Call);
        let raises: Vec<usize> = (0..k)
            .filter(|&a| matches!(actions[a], Action::Raise(_) | Action::AllIn))
            .collect();
        let total: f64 = weights.iter().map(|&w| f64::from(w.max(0.0))).sum();
        if k < 2 || total <= 0.0 {
            return s;
        }
        let p = &self.preflop;
        let facing = call.is_some();
        if let Some(fold) = fold {
            let enter: Vec<usize> = (0..k).filter(|&a| a != fold).collect();
            let mult = if facing { p.vpip_mult } else { p.pfr_mult };
            shift(
                &mut s,
                k,
                ev,
                weights,
                &Groups {
                    from: &[fold],
                    to: &enter,
                    base: None,
                },
                mult,
            );
        }
        if let Some(call) = call
            && !raises.is_empty()
        {
            let continuing: Vec<usize> = (0..k).filter(|&a| Some(a) != fold).collect();
            shift(
                &mut s,
                k,
                ev,
                weights,
                &Groups {
                    from: &[call],
                    to: &raises,
                    base: Some(&continuing),
                },
                p.threebet_mult,
            );
        }
        renormalize(&mut s, k);
        s
    }

    /// The profiled strategy at a postflop node (see the module docs).
    ///
    /// Same shapes as [`preflop_strategy`](Self::preflop_strategy): rows are
    /// combos, `weights` is the bot's range at the node (any scale; dead
    /// combos 0). `facing` is the number of bets and raises already made on
    /// this street (0 = nobody bet, 1 = facing a bet, 2+ = facing a raise).
    pub fn postflop_strategy(
        &self,
        actions: &[Action],
        strategy: &[f32],
        ev: &[f32],
        weights: &[f32],
        facing: usize,
    ) -> Vec<f32> {
        let k = actions.len();
        let n = weights.len();
        assert_eq!(strategy.len(), n * k, "strategy shape");
        assert_eq!(ev.len(), n * k, "ev shape");
        let mut s: Vec<f32> = strategy.to_vec();
        renormalize(&mut s, k);
        let total: f64 = weights.iter().map(|&w| f64::from(w.max(0.0))).sum();
        if k < 2 || total <= 0.0 || !total.is_finite() {
            return s;
        }
        // Range shares do not depend on the weights' scale.
        let weights: Vec<f32> = weights
            .iter()
            .map(|&w| (f64::from(w.max(0.0)) / total) as f32)
            .collect();
        let reference = s.clone();
        let q = &self.postflop;
        let fold = actions.iter().position(|a| *a == Action::Fold);
        let aggressive: Vec<usize> = (0..k).filter(|&a| actions[a].is_aggressive()).collect();
        let passive: Vec<usize> = (0..k)
            .filter(|&a| matches!(actions[a], Action::Check | Action::Call))
            .collect();
        let non_fold: Vec<usize> = (0..k).filter(|&a| Some(a) != fold).collect();

        // 1. Folding to a bet or a raise.
        if let Some(fold) = fold
            && facing > 0
        {
            let adj = if facing == 1 {
                q.fold_to_cbet
            } else {
                q.fold_to_raise
            };
            let g = Groups {
                from: &[fold],
                to: &non_fold,
                base: None,
            };
            let (have, cap) = masses(&s, k, &weights, &g);
            // Folding more means continuing less, and the other way round.
            let target = toward(have, -adj, cap);
            move_mass(&mut s, k, ev, &weights, &g, have, cap, target);
        }
        if aggressive.is_empty() || passive.is_empty() {
            renormalize(&mut s, k);
            return s;
        }
        // 2. Aggression: bets and raises against checks and calls, within
        // the hands that do not fold.
        let g = Groups {
            from: &passive,
            to: &aggressive,
            base: Some(&non_fold),
        };
        let (have, cap) = masses(&s, k, &weights, &g);
        let target = toward(have, q.aggression, cap);
        move_mass(&mut s, k, ev, &weights, &g, have, cap, target);
        // 3. Bluff/value: same aggressive share, carried by stronger hands
        // (shift > 0) or weaker ones (shift < 0).
        if q.bluff_value_shift != 0.0 {
            rebalance_aggression(
                &mut s,
                k,
                ev,
                &weights,
                &reference,
                [&passive, &aggressive, &non_fold],
                q.bluff_value_shift,
            );
        }
        // 4. Sizing: part of every row's other bet sizes moves to the
        // preferred one.
        let mut sized: Vec<(Chips, usize)> = aggressive
            .iter()
            .filter_map(|&a| match actions[a] {
                Action::Bet(x) | Action::Raise(x) => Some((x, a)),
                _ => None,
            })
            .collect();
        sized.sort_unstable();
        let preferred = match self.sizing.prefer {
            Sizing::Reference => None,
            Sizing::Small => sized.first().map(|x| x.1),
            Sizing::Large => sized.last().map(|x| x.1),
        };
        if let Some(to) = preferred
            && sized.len() >= 2
        {
            for row in s.chunks_mut(k) {
                let mut moved = 0.0;
                for &(_, a) in &sized {
                    if a != to {
                        moved += row[a] * SIZING_SHARE;
                        row[a] *= 1.0 - SIZING_SHARE;
                    }
                }
                row[to] += moved;
            }
        }
        renormalize(&mut s, k);
        s
    }
}

/// Share of a row's mass on other bet sizes that moves to the preferred size
/// (`sizing.prefer`).
pub const SIZING_SHARE: f32 = 0.5;

/// Moves a share `x` (of `0..=max`) by `t` in `-1..=1`: toward `max` when
/// `t > 0`, toward 0 when `t < 0`.
fn toward(x: f64, t: f64, max: f64) -> f64 {
    if t >= 0.0 {
        x + t * (max - x)
    } else {
        x + t * x
    }
}

/// Step 3 of the postflop transform: each row's aggressive mass moves
/// `|shift|` of the way toward an allocation of the same range share that
/// fills rows strongest first (`shift > 0`) or weakest first (`shift < 0`),
/// each row up to its non-fold mass. Strength is the hand's reference EV at
/// the node.
fn rebalance_aggression(
    s: &mut [f32],
    k: usize,
    ev: &[f32],
    weights: &[f32],
    reference: &[f32],
    [passive, aggressive, non_fold]: [&[usize]; 3],
    shift: f64,
) {
    let n = weights.len();
    let strength = |h: usize| -> f32 {
        (0..k)
            .map(|a| reference[h * k + a] * ev[h * k + a])
            .sum::<f32>()
    };
    let rows: Vec<usize> = (0..n).filter(|&h| weights[h] > 0.0).collect();
    let mass = |s: &[f32], h: usize, g: &[usize]| f64::from(group_mass(&s[h * k..(h + 1) * k], g));
    let total: f64 = rows
        .iter()
        .map(|&h| f64::from(weights[h]) * mass(s, h, aggressive))
        .sum();
    let mut order = rows.clone();
    order.sort_by(|&a, &b| strength(b).total_cmp(&strength(a)).then(a.cmp(&b)));
    if shift < 0.0 {
        order.reverse();
    }
    let mut extreme = vec![0.0f64; n];
    let mut left = total;
    for &h in &order {
        if left <= 0.0 {
            break;
        }
        let w = f64::from(weights[h]);
        let take = mass(s, h, non_fold).min(left / w);
        extreme[h] = take;
        left -= take * w;
    }
    let t = shift.abs().min(1.0);
    for &h in &rows {
        let m = mass(s, h, aggressive);
        let d = (t * (extreme[h] - m)) as f32;
        let e = &ev[h * k..(h + 1) * k];
        let row = &mut s[h * k..(h + 1) * k];
        if d > 0.0 {
            let d = d.min(group_mass(row, passive));
            take_from(row, passive, d);
            add_to(row, e, aggressive, d);
        } else if d < 0.0 {
            let d = (-d).min(group_mass(row, aggressive));
            take_from(row, aggressive, d);
            add_to(row, e, passive, d);
        }
    }
}

/// The id that stands for the reference (GTO) bot; no profile may use it.
pub const REFERENCE: &str = "reference";

/// Two action groups: mass moves between `from` and `to`. The target is
/// `mult ×` the range share of `to`, capped by the share of `base` (all
/// actions when `None`).
struct Groups<'a> {
    from: &'a [usize],
    to: &'a [usize],
    base: Option<&'a [usize]>,
}

fn group_mass(row: &[f32], g: &[usize]) -> f32 {
    g.iter().map(|&a| row[a]).sum()
}

fn group_best_ev(ev: &[f32], g: &[usize]) -> f32 {
    g.iter().map(|&a| ev[a]).fold(f32::NEG_INFINITY, f32::max)
}

/// Add `amount` to the actions of `g` in `row`, split by the row's current
/// split inside `g` (or onto the best-EV action of `g` if it has none).
fn add_to(row: &mut [f32], ev: &[f32], g: &[usize], amount: f32) {
    let m = group_mass(row, g);
    if m > 0.0 {
        let k = (m + amount) / m;
        for &a in g {
            row[a] *= k;
        }
    } else {
        let best = g
            .iter()
            .copied()
            .max_by(|&a, &b| ev[a].total_cmp(&ev[b]).then(b.cmp(&a)))
            .expect("non-empty group");
        row[best] += amount;
    }
}

/// Take `amount` from the actions of `g` in `row`, proportionally.
fn take_from(row: &mut [f32], g: &[usize], amount: f32) {
    let m = group_mass(row, g);
    if m > 0.0 {
        let k = ((m - amount) / m).max(0.0);
        for &a in g {
            row[a] *= k;
        }
    }
}

/// Moves range mass between two action groups until the range share of
/// `to` is `mult ×` what it was (capped by `base`), greedily by each row's
/// EV advantage of `to` over `from`: best rows gain first, worst rows lose
/// first.
fn shift(s: &mut [f32], k: usize, ev: &[f32], weights: &[f32], g: &Groups<'_>, mult: f64) {
    let (have, cap) = masses(s, k, weights, g);
    let target = (have * mult).min(cap);
    move_mass(s, k, ev, weights, g, have, cap, target);
}

/// Weighted mass of `g.to` over the range, and its cap (the mass of
/// `g.base`, or the whole range).
fn masses(s: &[f32], k: usize, weights: &[f32], g: &Groups<'_>) -> (f64, f64) {
    let mut have = 0.0f64;
    let mut cap = 0.0f64;
    for (h, &wh) in weights.iter().enumerate() {
        let w = f64::from(wh.max(0.0));
        let r = &s[h * k..(h + 1) * k];
        have += w * f64::from(group_mass(r, g.to));
        cap += w * f64::from(match g.base {
            Some(b) => group_mass(r, b),
            None => 1.0,
        });
    }
    (have, cap)
}

/// Moves mass between `g.from` and `g.to` until `g.to` holds `target`
/// (weighted), best rows by EV advantage gaining first, worst losing first.
#[allow(clippy::too_many_arguments)]
fn move_mass(
    s: &mut [f32],
    k: usize,
    ev: &[f32],
    weights: &[f32],
    g: &Groups<'_>,
    have: f64,
    cap: f64,
    target: f64,
) {
    let n = weights.len();
    let w = |h: usize| f64::from(weights[h].max(0.0));
    let row = |s: &[f32], h: usize| s[h * k..(h + 1) * k].to_vec();
    let mut delta = target - have;
    if delta.abs() <= 1e-9 * cap.max(1.0) {
        return;
    }
    // Rows by EV advantage of `to` over `from`, best first; ties by row.
    let mut order: Vec<usize> = (0..n).filter(|&h| w(h) > 0.0).collect();
    let adv = |h: usize| {
        let e = &ev[h * k..(h + 1) * k];
        group_best_ev(e, g.to) - group_best_ev(e, g.from)
    };
    order.sort_by(|&a, &b| adv(b).total_cmp(&adv(a)).then(a.cmp(&b)));
    if delta < 0.0 {
        order.reverse();
    }
    for h in order {
        if delta.abs() <= 0.0 {
            break;
        }
        let r = row(s, h);
        let e = &ev[h * k..(h + 1) * k];
        let movable = if delta > 0.0 {
            group_mass(&r, g.from)
        } else {
            group_mass(&r, g.to)
        };
        if movable <= 0.0 {
            continue;
        }
        let want = (delta.abs() / w(h)) as f32;
        let amount = want.min(movable);
        let out = &mut s[h * k..(h + 1) * k];
        if delta > 0.0 {
            take_from(out, g.from, amount);
            add_to(out, e, g.to, amount);
            delta -= f64::from(amount) * w(h);
        } else {
            take_from(out, g.to, amount);
            add_to(out, e, g.from, amount);
            delta += f64::from(amount) * w(h);
        }
        if (amount - want).abs() <= f32::EPSILON * want.max(1.0) {
            break;
        }
    }
}

/// Clamp to `≥ 0` and scale each row to sum to 1 (uniform if it sums to 0).
fn renormalize(s: &mut [f32], k: usize) {
    for row in s.chunks_mut(k) {
        row.iter_mut().for_each(|x| *x = x.max(0.0));
        let t: f32 = row.iter().sum();
        if t > 0.0 {
            row.iter_mut().for_each(|x| *x /= t);
        } else {
            row.iter_mut().for_each(|x| *x = 1.0 / k as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

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

    const FACING: [Action; 3] = [Action::Fold, Action::Call, Action::Raise(750)];
    const UNOPENED: [Action; 2] = [Action::Fold, Action::Raise(250)];

    /// Range share of `actions` (indices) under `s`.
    fn share(s: &[f32], k: usize, w: &[f32], actions: &[usize]) -> f64 {
        let total: f64 = w.iter().map(|&x| f64::from(x)).sum();
        (0..w.len())
            .map(|h| {
                f64::from(w[h])
                    * actions
                        .iter()
                        .map(|&a| f64::from(s[h * k + a]))
                        .sum::<f64>()
            })
            .sum::<f64>()
            / total
    }

    /// Three hands, strong to weak: the strong one raises, the middle one
    /// calls, the weak one folds.
    fn facing_node() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let s = vec![0.0, 0.25, 0.75, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0];
        let ev = vec![0.0, 3.0, 4.0, 0.0, 1.0, 0.5, 0.0, -0.5, -1.0];
        (s, ev, vec![6.0, 6.0, 6.0])
    }

    #[test]
    fn shipped_profiles_parse() {
        let p = Profile::from_toml(
            "id = \"calling-station\"\n[preflop]\nvpip_mult = 1.8\npfr_mult = 0.6\nthreebet_mult = 0.4\n\
             [postflop]\nfold_to_cbet = -0.25\nfold_to_raise = -0.20\nbluff_value_shift = -0.3\naggression = -0.4\n\
             [sizing]\nprefer = \"small\"\n",
        )
        .unwrap();
        assert_eq!(p.sizing.prefer, Sizing::Small);
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut p = profile(0.0, 1.0, 1.0);
        assert!(p.validate().is_err());
        p.preflop.vpip_mult = 1.0;
        p.postflop.aggression = 1.5;
        assert!(p.validate().is_err());
        p.postflop.aggression = 0.0;
        p.id = "Bad Id".into();
        assert!(p.validate().is_err());
        p.id = REFERENCE.into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn identity_profile_keeps_the_reference() {
        let (s, ev, w) = facing_node();
        let out = profile(1.0, 1.0, 1.0).preflop_strategy(&FACING, &s, &ev, &w);
        assert_eq!(out, s);
    }

    #[test]
    fn widening_adds_the_next_best_hand_first() {
        let (s, ev, w) = facing_node();
        // Continue share 2/3 → 1 with vpip 1.5: the weak hand now continues
        // fully, the others are unchanged.
        let out = profile(1.5, 1.0, 1.0).preflop_strategy(&FACING, &s, &ev, &w);
        assert!((share(&out, 3, &w, &[1, 2]) - 1.0).abs() < 1e-6);
        assert_eq!(&out[..6], &s[..6]);
        // The weak hand never raised: its added mass goes to its best
        // non-fold action by EV (call, −0.5 > −1), then the raise stage
        // (multiplier 1) leaves it.
        assert!((out[7] - 1.0).abs() < 1e-6, "{out:?}");
    }

    #[test]
    fn narrowing_drops_the_worst_continuing_hand_first() {
        let (s, ev, w) = facing_node();
        // Continue share 2/3 → 1/3: the middle hand folds, the strong one
        // keeps its mix.
        let out = profile(0.5, 1.0, 1.0).preflop_strategy(&FACING, &s, &ev, &w);
        assert!((share(&out, 3, &w, &[1, 2]) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(&out[..3], &s[..3]);
        assert!((out[3] - 1.0).abs() < 1e-6, "{out:?}");
    }

    #[test]
    fn threebet_mult_moves_calls_to_raises_by_raise_advantage() {
        let (s, ev, w) = facing_node();
        // Raise share 0.25 → 0.375 (× 1.5): the strong hand's call (raise
        // advantage +1) moves first, then part of the middle hand's.
        let out = profile(1.0, 1.0, 1.5).preflop_strategy(&FACING, &s, &ev, &w);
        assert!((share(&out, 3, &w, &[2]) - 0.375).abs() < 1e-6);
        assert!((out[2] - 1.0).abs() < 1e-6, "{out:?}");
        assert!(out[5] > 0.0 && out[5] < 1.0, "{out:?}");
        // Continuing share is unchanged.
        assert!((share(&out, 3, &w, &[1, 2]) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn unopened_nodes_scale_by_pfr() {
        let s = vec![0.0, 1.0, 0.5, 0.5, 1.0, 0.0];
        let ev = vec![0.0, 2.0, 0.0, 0.1, 0.0, -0.4];
        let w = vec![1.0, 1.0, 1.0];
        // vpip is ignored without a raise to face.
        let out = profile(3.0, 0.5, 1.0).preflop_strategy(&UNOPENED, &s, &ev, &w);
        assert!((share(&out, 2, &w, &[1]) - 0.25).abs() < 1e-6);
        assert!((out[1] - 0.75).abs() < 1e-6, "{out:?}");
    }

    fn postflop(
        fold_to_cbet: f64,
        fold_to_raise: f64,
        bvs: f64,
        aggression: f64,
        prefer: Sizing,
    ) -> Profile {
        let mut p = profile(1.0, 1.0, 1.0);
        p.postflop = PostflopDeviation {
            fold_to_cbet,
            fold_to_raise,
            bluff_value_shift: bvs,
            aggression,
        };
        p.sizing.prefer = prefer;
        p
    }

    const FACING_BET: [Action; 3] = [Action::Fold, Action::Call, Action::Raise(900)];
    const OPEN: [Action; 3] = [Action::Check, Action::Bet(150), Action::Bet(450)];

    /// Four combos, strong to weak, facing a bet: a raise, a call, a mixed
    /// call/fold and a fold.
    fn facing_bet() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let s = vec![
            0.0, 0.2, 0.8, //
            0.0, 1.0, 0.0, //
            0.5, 0.5, 0.0, //
            1.0, 0.0, 0.0,
        ];
        let ev = vec![
            0.0, 6.0, 7.0, //
            0.0, 2.0, 1.0, //
            0.0, 0.0, -2.0, //
            0.0, -3.0, -6.0,
        ];
        (s, ev, vec![1.0; 4])
    }

    #[test]
    fn neutral_postflop_profile_keeps_the_reference() {
        let (s, ev, w) = facing_bet();
        let p = postflop(0.0, 0.0, 0.0, 0.0, Sizing::Reference);
        for facing in 0..3 {
            let out = p.postflop_strategy(&FACING_BET, &s, &ev, &w, facing);
            for (a, b) in out.iter().zip(&s) {
                assert!((a - b).abs() < 1e-6, "{out:?}");
            }
        }
    }

    #[test]
    fn fold_to_cbet_and_fold_to_raise_move_the_fold_share() {
        let (s, ev, w) = facing_bet();
        let before = share(&s, 3, &w, &[0]);
        // Facing a bet: fold share f → f + 0.5 (1 − f); the raise share
        // follows the continuing range (aggression 0).
        let nit = postflop(0.5, 0.0, 0.0, 0.0, Sizing::Reference);
        let out = nit.postflop_strategy(&FACING_BET, &s, &ev, &w, 1);
        let want = before + 0.5 * (1.0 - before);
        assert!((share(&out, 3, &w, &[0]) - want).abs() < 1e-5, "{out:?}");
        // The worst continuing hand folds first: the mixed one.
        assert!(out[6] > 0.99, "{out:?}");
        // Facing a raise the other parameter applies.
        let out = nit.postflop_strategy(&FACING_BET, &s, &ev, &w, 2);
        assert!((share(&out, 3, &w, &[0]) - before).abs() < 1e-5);
        // A station folds less: f → f × (1 − 0.5).
        let station = postflop(-0.5, -0.5, 0.0, 0.0, Sizing::Reference);
        let out = station.postflop_strategy(&FACING_BET, &s, &ev, &w, 2);
        assert!((share(&out, 3, &w, &[0]) - before * 0.5).abs() < 1e-5);
        // Nothing to fold to at an unopened node.
        let out = nit.postflop_strategy(&FACING_BET, &s, &ev, &w, 0);
        assert!((share(&out, 3, &w, &[0]) - before).abs() < 1e-6);
    }

    #[test]
    fn aggression_moves_checks_to_bets() {
        let s = vec![0.2, 0.0, 0.8, 1.0, 0.0, 0.0, 0.5, 0.5, 0.0, 1.0, 0.0, 0.0];
        let ev = vec![
            3.0, 3.5, 4.0, 1.0, 0.8, 0.5, 0.0, 0.0, -0.1, -1.0, -1.5, -2.0,
        ];
        let w = vec![1.0, 2.0, 1.0, 3.0];
        let before = share(&s, 3, &w, &[1, 2]);
        let out = postflop(0.0, 0.0, 0.0, 0.5, Sizing::Reference)
            .postflop_strategy(&OPEN, &s, &ev, &w, 0);
        let after = share(&out, 3, &w, &[1, 2]);
        assert!(
            (after - (before + 0.5 * (1.0 - before))).abs() < 1e-5,
            "{before} → {after}"
        );
        let out = postflop(0.0, 0.0, 0.0, -0.5, Sizing::Reference)
            .postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!((share(&out, 3, &w, &[1, 2]) - before * 0.5).abs() < 1e-5);
    }

    /// Strength-weighted mean of the aggressive mass.
    fn aggressive_strength(
        s: &[f32],
        k: usize,
        ev: &[f32],
        reference: &[f32],
        w: &[f32],
        g: &[usize],
    ) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for h in 0..w.len() {
            let st: f32 = (0..k).map(|a| reference[h * k + a] * ev[h * k + a]).sum();
            let m: f32 = g.iter().map(|&a| s[h * k + a]).sum();
            num += f64::from(w[h] * m * st);
            den += f64::from(w[h] * m);
        }
        num / den
    }

    #[test]
    fn bluff_value_shift_keeps_the_bet_share_and_moves_its_strength() {
        // The strong hand checks some, the weak one bluffs some.
        let s = vec![0.5, 0.5, 0.0, 1.0, 0.0, 0.0, 0.6, 0.4, 0.0];
        let ev = vec![5.0, 5.0, 4.0, 1.0, 0.5, 0.2, -1.0, -1.0, -2.0];
        let w = vec![1.0, 1.0, 1.0];
        let before = share(&s, 3, &w, &[1, 2]);
        let base = aggressive_strength(&s, 3, &ev, &s, &w, &[1, 2]);
        let value = postflop(0.0, 0.0, 0.5, 0.0, Sizing::Reference)
            .postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!((share(&value, 3, &w, &[1, 2]) - before).abs() < 1e-5);
        assert!(aggressive_strength(&value, 3, &ev, &s, &w, &[1, 2]) > base + 0.5);
        // Fully value-heavy: the strong hand bets everything it can.
        let full = postflop(0.0, 0.0, 1.0, 0.0, Sizing::Reference)
            .postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!((full[1] + full[2] - 0.9).abs() < 1e-5, "{full:?}");
        let bluffy = postflop(0.0, 0.0, -0.5, 0.0, Sizing::Reference)
            .postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!((share(&bluffy, 3, &w, &[1, 2]) - before).abs() < 1e-5);
        assert!(aggressive_strength(&bluffy, 3, &ev, &s, &w, &[1, 2]) < base - 0.5);
    }

    #[test]
    fn sizing_preference_moves_half_to_the_preferred_size() {
        let s = vec![0.0, 0.4, 0.6, 1.0, 0.0, 0.0];
        let ev = vec![0.0; 6];
        let w = vec![1.0, 1.0];
        let small =
            postflop(0.0, 0.0, 0.0, 0.0, Sizing::Small).postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!(
            (small[1] - 0.7).abs() < 1e-6 && (small[2] - 0.3).abs() < 1e-6,
            "{small:?}"
        );
        let large =
            postflop(0.0, 0.0, 0.0, 0.0, Sizing::Large).postflop_strategy(&OPEN, &s, &ev, &w, 0);
        assert!(
            (large[1] - 0.2).abs() < 1e-6 && (large[2] - 0.8).abs() < 1e-6,
            "{large:?}"
        );
        // One size (plus all-in): nothing to prefer.
        let one = [Action::Check, Action::Bet(150), Action::AllIn];
        let out =
            postflop(0.0, 0.0, 0.0, 0.0, Sizing::Large).postflop_strategy(&one, &s, &ev, &w, 0);
        assert!((out[1] - 0.4).abs() < 1e-6);
    }

    fn node() -> impl Strategy<Value = (Vec<Action>, Vec<f32>, Vec<f32>, Vec<f32>)> {
        (2usize..=4, 1usize..=12).prop_flat_map(|(k, n)| {
            let actions: Vec<Action> = match k {
                2 => vec![Action::Fold, Action::Raise(250)],
                3 => FACING.to_vec(),
                _ => vec![
                    Action::Fold,
                    Action::Call,
                    Action::Raise(750),
                    Action::AllIn,
                ],
            };
            (
                Just(actions),
                prop::collection::vec(0.0f32..1.0, n * k),
                prop::collection::vec(-5.0f32..5.0, n * k),
                prop::collection::vec(0.0f32..12.0, n),
            )
        })
    }

    proptest! {
        #[test]
        fn transform_is_deterministic_and_valid(
            (actions, s, ev, w) in node(),
            vpip in 0.1f64..5.0,
            pfr in 0.1f64..5.0,
            threebet in 0.1f64..5.0,
        ) {
            let p = profile(vpip, pfr, threebet);
            let a = p.preflop_strategy(&actions, &s, &ev, &w);
            let b = p.preflop_strategy(&actions, &s, &ev, &w);
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(a.len(), s.len());
            for row in a.chunks(actions.len()) {
                prop_assert!(row.iter().all(|x| x.is_finite() && *x >= 0.0), "{:?}", row);
                let t: f32 = row.iter().sum();
                prop_assert!((t - 1.0).abs() < 1e-4, "{:?}", row);
            }
        }

        #[test]
        fn postflop_transform_is_deterministic_and_valid(
            (s, ev, w) in (1usize..=12).prop_flat_map(|n| (
                prop::collection::vec(0.0f32..1.0, n * 3),
                prop::collection::vec(-5.0f32..5.0, n * 3),
                prop::collection::vec(0.0f32..12.0, n),
            )),
            q in prop::collection::vec(-1.0f64..=1.0, 4),
            facing in 0usize..3,
            prefer in 0u8..3,
        ) {
            let prefer = [Sizing::Reference, Sizing::Small, Sizing::Large][usize::from(prefer)];
            let p = postflop(q[0], q[1], q[2], q[3], prefer);
            let actions: &[Action] = if facing == 0 { &OPEN } else { &FACING_BET };
            let a = p.postflop_strategy(actions, &s, &ev, &w, facing);
            prop_assert_eq!(&a, &p.postflop_strategy(actions, &s, &ev, &w, facing));
            for row in a.chunks(3) {
                prop_assert!(row.iter().all(|x| x.is_finite() && *x >= 0.0), "{:?}", row);
                prop_assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4, "{:?}", row);
            }
            // Scale-free in the weights.
            let w10: Vec<f32> = w.iter().map(|x| x * 10.0).collect();
            let b = p.postflop_strategy(actions, &s, &ev, &w10, facing);
            for (x, y) in a.iter().zip(&b) {
                prop_assert!((x - y).abs() < 1e-3);
            }
        }

        #[test]
        fn continue_share_moves_toward_the_multiplier(
            (s, ev, w) in (1usize..=12).prop_flat_map(|n| (
                prop::collection::vec(0.0f32..1.0, n * 3),
                prop::collection::vec(-5.0f32..5.0, n * 3),
                prop::collection::vec(0.1f32..12.0, n),
            )),
            vpip in 0.1f64..5.0,
        ) {
            let mut base = s.clone();
            renormalize(&mut base, 3);
            let before = share(&base, 3, &w, &[1, 2]);
            let out = profile(vpip, 1.0, 1.0).preflop_strategy(&FACING, &s, &ev, &w);
            let after = share(&out, 3, &w, &[1, 2]);
            let want = (before * vpip).min(1.0);
            prop_assert!((after - want).abs() < 1e-3, "{} -> {} want {}", before, after, want);
        }
    }
}

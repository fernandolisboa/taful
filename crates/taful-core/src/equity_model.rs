//! Chip EV vs tournament equity (spec §5.5, D1, D3).
//!
//! Grading runs on one ruler: bb for cash, and for tournaments $EV from ICM
//! converted back to bb-equivalents, so the D3 thresholds apply unchanged.

use crate::config::Chips;
use crate::position::Seat;

/// Converts the stacks after a decision into hero's value in bb-equivalents.
pub trait EquityModel {
    fn to_bb_equiv(&self, stacks_after: &[Chips], hero: Seat) -> f64;
}

/// Cash games: value is hero's stack in big blinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChipEv {
    pub bb: Chips,
}

impl EquityModel for ChipEv {
    fn to_bb_equiv(&self, stacks_after: &[Chips], hero: Seat) -> f64 {
        stacks_after[hero] as f64 / self.bb as f64
    }
}

/// Maximum number of players with chips for exact Malmuth–Harville.
pub const ICM_MAX_PLAYERS: usize = 16;

/// Malmuth–Harville ICM: each player's expected prize given stacks.
///
/// Players with no chips get nothing (they are already out). Payouts beyond
/// the number of players with chips are ignored. Exact over all finishing
/// orders in `O(2^n · n)`; panics above [`ICM_MAX_PLAYERS`].
pub fn icm_equities(stacks: &[Chips], payouts: &[f64]) -> Vec<f64> {
    let alive: Vec<usize> = (0..stacks.len()).filter(|&i| stacks[i] > 0).collect();
    let n = alive.len();
    assert!(
        n <= ICM_MAX_PLAYERS,
        "ICM supports at most {ICM_MAX_PLAYERS} players"
    );
    let mut ev = vec![0.0; stacks.len()];
    if n == 0 {
        return ev;
    }
    let full = (1usize << n) - 1;
    let mut reach = vec![0.0f64; 1 << n];
    reach[full] = 1.0;
    // Masks in decreasing order: removing a bit always yields a smaller mask.
    for mask in (1..=full).rev() {
        let p = reach[mask];
        if p == 0.0 {
            continue;
        }
        let place = n - mask.count_ones() as usize;
        let total: f64 = (0..n)
            .filter(|b| mask & (1 << b) != 0)
            .map(|b| stacks[alive[b]] as f64)
            .sum();
        let prize = payouts.get(place).copied().unwrap_or(0.0);
        for b in (0..n).filter(|b| mask & (1 << b) != 0) {
            let q = p * stacks[alive[b]] as f64 / total;
            ev[alive[b]] += q * prize;
            reach[mask & !(1 << b)] += q;
        }
    }
    ev
}

/// Tournaments: hero's ICM $EV divided by a fixed $-per-chip rate taken at
/// hero's stack before the decision, then by the big blind.
#[derive(Debug, Clone, PartialEq)]
pub struct Icm {
    pub payouts: Vec<f64>,
    pub bb: Chips,
    /// $ per chip at hero's stack before the decision.
    pub dollars_per_chip: f64,
}

impl Icm {
    /// Fix the conversion rate at the decision point (`stacks_before`).
    pub fn at(payouts: Vec<f64>, bb: Chips, stacks_before: &[Chips], hero: Seat) -> Icm {
        let ev = icm_equities(stacks_before, &payouts)[hero];
        let chips = stacks_before[hero].max(1) as f64;
        Icm {
            payouts,
            bb,
            dollars_per_chip: ev / chips,
        }
    }
}

impl EquityModel for Icm {
    fn to_bb_equiv(&self, stacks_after: &[Chips], hero: Seat) -> f64 {
        if self.dollars_per_chip <= 0.0 {
            return 0.0;
        }
        icm_equities(stacks_after, &self.payouts)[hero] / self.dollars_per_chip / self.bb as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_ev_is_identity_in_bb() {
        assert_eq!(ChipEv { bb: 100 }.to_bb_equiv(&[2500, 100], 0), 25.0);
    }

    #[test]
    fn icm_known_values() {
        // Equal stacks: equal shares.
        let e = icm_equities(&[1000, 1000, 1000], &[0.5, 0.3, 0.2]);
        for x in &e {
            assert!((x - 1.0 / 3.0).abs() < 1e-12);
        }
        // Classic example: stacks 5000/3000/2000, payouts 50/30/20.
        let e = icm_equities(&[5000, 3000, 2000], &[50.0, 30.0, 20.0]);
        assert!((e[0] - 38.393).abs() < 1e-3, "{e:?}");
        assert!((e[1] - 32.750).abs() < 1e-3, "{e:?}");
        assert!((e[2] - 28.857).abs() < 1e-3, "{e:?}");
        assert!((e.iter().sum::<f64>() - 100.0).abs() < 1e-9);
        // Busted players get nothing; winner-take-all is chip-proportional.
        let e = icm_equities(&[0, 300, 100], &[1.0]);
        assert_eq!(e[0], 0.0);
        assert!((e[1] - 0.75).abs() < 1e-12);
    }

    #[test]
    fn icm_bb_equiv_is_identity_at_decision_point() {
        let stacks = [5000, 3000, 2000];
        let m = Icm::at(vec![50.0, 30.0, 20.0], 100, &stacks, 1);
        assert!((m.to_bb_equiv(&stacks, 1) - 30.0).abs() < 1e-9);
        // Doubling up is worth less than double in ICM.
        let after = [4000, 6000, 0];
        assert!(m.to_bb_equiv(&after, 1) < 60.0);
    }
}

//! EV-loss grading (spec §8, D3).
//!
//! Each decision is graded on two scales, absolute (bb) and relative (% of the
//! pot before the decision), and gets the **worse** grade. An action the
//! reference plays at ≥ `best_min_freq`, or one within the spot's solver
//! tolerance of the best EV, is always [`Grade::Best`].
//!
//! Thresholds live in `config/grading.toml`; this module only parses them.

use serde::{Deserialize, Serialize};

/// Grade of one decision, best first. `Ord`: a greater grade is a worse one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Best,
    Correct,
    Inaccuracy,
    Mistake,
    Blunder,
}

impl Grade {
    pub const ALL: [Grade; 5] = [
        Grade::Best,
        Grade::Correct,
        Grade::Inaccuracy,
        Grade::Mistake,
        Grade::Blunder,
    ];

    /// Stable id for storage and i18n keys (`grade-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Grade::Best => "best",
            Grade::Correct => "correct",
            Grade::Inaccuracy => "inaccuracy",
            Grade::Mistake => "mistake",
            Grade::Blunder => "blunder",
        }
    }
}

/// Grading thresholds (D3). `abs_bb` and `rel_pot_pct` are the upper bounds of
/// correct / inaccuracy / mistake; anything at or above the last is a blunder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// Bumped whenever the values change, stored with every graded decision so
    /// history can be regraded (§8).
    pub version: u32,
    pub best_min_freq: f32,
    pub abs_bb: [f64; 3],
    pub rel_pot_pct: [f64; 3],
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            version: 1,
            best_min_freq: 0.10,
            abs_bb: [0.1, 0.5, 2.0],
            rel_pot_pct: [2.0, 5.0, 15.0],
        }
    }
}

impl Thresholds {
    pub fn from_toml(s: &str) -> Result<Thresholds, String> {
        let t: Thresholds = toml::from_str(s).map_err(|e| e.to_string())?;
        t.validate()?;
        Ok(t)
    }

    pub fn validate(&self) -> Result<(), String> {
        let increasing = |a: &[f64; 3]| {
            a[0] > 0.0 && a[0] < a[1] && a[1] < a[2] && a.iter().all(|x| x.is_finite())
        };
        if !increasing(&self.abs_bb) || !increasing(&self.rel_pot_pct) {
            return Err("thresholds must be positive, finite and strictly increasing".into());
        }
        if !(0.0..=1.0).contains(&self.best_min_freq) {
            return Err("best_min_freq must be in [0, 1]".into());
        }
        Ok(())
    }
}

fn scale(loss: f64, bounds: &[f64; 3]) -> Grade {
    if loss < bounds[0] {
        Grade::Correct
    } else if loss < bounds[1] {
        Grade::Inaccuracy
    } else if loss < bounds[2] {
        Grade::Mistake
    } else {
        Grade::Blunder
    }
}

/// Grade one decision.
///
/// - `ev_loss_bb`: `max_a EV(combo, a) − EV(combo, chosen)` in bb (or
///   bb-equivalents for tournaments).
/// - `pot_bb`: pot before the decision.
/// - `chosen_freq`: reference frequency of the chosen action for hero's combo.
/// - `tolerance_bb`: the spot's exploitability × pot.
pub fn grade(
    ev_loss_bb: f64,
    pot_bb: f64,
    chosen_freq: f32,
    tolerance_bb: f64,
    t: &Thresholds,
) -> Grade {
    let loss = ev_loss_bb.max(0.0);
    if chosen_freq >= t.best_min_freq || loss <= tolerance_bb.max(0.0) {
        return Grade::Best;
    }
    let abs = scale(loss, &t.abs_bb);
    let rel = if pot_bb > 0.0 {
        scale(100.0 * loss / pot_bb, &t.rel_pot_pct)
    } else {
        Grade::Correct
    };
    abs.max(rel)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn defaults_match_config_file() {
        let file = include_str!("../../../config/grading.toml");
        assert_eq!(Thresholds::from_toml(file).unwrap(), Thresholds::default());
    }

    #[test]
    fn table_from_d3() {
        let t = Thresholds::default();
        let g = |loss, pot| grade(loss, pot, 0.0, 0.0, &t);
        // Big pot: the absolute scale decides.
        assert_eq!(g(0.05, 100.0), Grade::Correct);
        assert_eq!(g(0.3, 100.0), Grade::Inaccuracy);
        assert_eq!(g(1.0, 100.0), Grade::Mistake);
        assert_eq!(g(9.0, 300.0), Grade::Blunder);
        // Small pot: the relative scale decides (0.4bb of 5bb = 8%).
        assert_eq!(g(0.4, 5.0), Grade::Mistake);
        // Frequency and tolerance make it Best.
        assert_eq!(grade(5.0, 10.0, 0.10, 0.0, &t), Grade::Best);
        assert_eq!(grade(0.2, 10.0, 0.0, 0.25, &t), Grade::Best);
    }

    #[test]
    fn rejects_bad_thresholds() {
        assert!(
            Thresholds::from_toml(
                "version = 1\nbest_min_freq = 0.1\nabs_bb = [1, 0.5, 2]\nrel_pot_pct = [2, 5, 15]"
            )
            .is_err()
        );
    }

    proptest! {
        #[test]
        fn monotone_in_loss(a in 0.0f64..50.0, d in 0.0f64..50.0, pot in 0.5f64..500.0, f in 0.0f32..0.099, tol in 0.0f64..1.0) {
            let t = Thresholds::default();
            prop_assert!(grade(a, pot, f, tol, &t) <= grade(a + d, pot, f, tol, &t));
        }

        #[test]
        fn never_stricter_than_tolerance(loss in 0.0f64..5.0, pot in 0.5f64..500.0, f in 0.0f32..1.0) {
            prop_assert_eq!(grade(loss, pot, f, loss, &Thresholds::default()), Grade::Best);
        }

        #[test]
        fn frequent_actions_are_best(loss in 0.0f64..50.0, pot in 0.5f64..500.0, f in 0.10f32..=1.0) {
            prop_assert_eq!(grade(loss, pot, f, 0.0, &Thresholds::default()), Grade::Best);
        }
    }
}

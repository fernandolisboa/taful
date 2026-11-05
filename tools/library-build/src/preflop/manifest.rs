//! Preflop build manifest (spec §6, §7.3): game, sizing, tree limits,
//! realization model and solver settings, in one TOML file.

use serde::{Deserialize, Serialize};
use taful_core::config::{BB_CHIPS, Chips, GameConfig};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub game: Game,
    pub sizing: Sizing,
    pub tree: TreeLimits,
    pub realization: Realization,
    pub solve: Solve,
}

/// The game: a uniform-stack cash table (other formats come with their own
/// manifests later).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub table_size: u8,
    pub stack_bb: u32,
}

/// Raise sizes (spec §6: opens 2.5bb, SB 3bb; 3-bets 3x IP / 3.5–4x OOP;
/// 4-bets ~2.2x; 5-bet all-in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sizing {
    pub open_bb: f64,
    pub sb_open_bb: f64,
    /// 3-bet as a multiple of the open, in position / out of position (blinds).
    pub threebet_ip_mult: f64,
    pub threebet_oop_mult: f64,
    /// Added per cold caller of the open (squeeze), as a multiple of the open.
    pub squeeze_per_caller_mult: f64,
    pub fourbet_mult: f64,
    /// Raises allowed in total, the open included; the last one is all-in.
    pub max_raises: u32,
    /// Sizes are rounded to this many bb.
    pub round_bb: f64,
    /// A raise that commits at least this share of the starting stack is
    /// played as all-in.
    pub allin_above_stack_pct: f64,
}

/// Abstractions that keep the tree small.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeLimits {
    /// A call that would put more players than this in the pot is not offered.
    pub max_players_postflop: u32,
    /// Players with no voluntary chips in the pot may call only up to this
    /// raise level (1 = they may cold-call an open, not a 3-bet).
    pub cold_call_max_level: u32,
}

/// Postflop abstraction (spec §6): a terminal is worth
/// `pot × realized share`, where a player's realized share against one
/// opponent with equity `e` is `eᵏ·Rᵢ / (eᵏ·Rᵢ + (1−e)ᵏ·Rⱼ)`.
///
/// `R` is the position factor; `k ≥ 1` is the equity exponent, which lets
/// strong hands realize more than their equity and weak hands less (they get
/// pushed off their equity postflop). Constant-sum; `k = 1, Rᵢ = Rⱼ` gives
/// back raw equity. Multiway: product of pairwise shares, renormalized so
/// shares add up to 1. All-in pots use raw equity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Realization {
    /// Single-raised pot: in position, out of position.
    pub srp: [f64; 2],
    pub threebet: [f64; 2],
    /// 4-bet pots and above.
    pub fourbet: [f64; 2],
    /// Equity exponent `k`.
    pub equity_exponent: f64,
    /// Below this stack-to-pot ratio the factors shrink linearly toward 1
    /// (equity gets realized by getting the money in).
    pub low_spr: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Solve {
    pub iterations: u32,
    /// Board samples per combo pair for the 169×169 equity table.
    pub equity_samples: u32,
    pub seed: u64,
    /// Decision nodes where the acting player's range holds less than this
    /// percent of their combos are dropped from the chart (spec §7.5).
    pub prune_reach_pct: f64,
}

impl Manifest {
    pub fn from_toml(s: &str) -> Result<Manifest, String> {
        let m: Manifest = toml::from_str(s).map_err(|e| e.to_string())?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), String> {
        let g = &self.game;
        if !(2..=9).contains(&g.table_size) || g.stack_bb == 0 {
            return Err("game: table_size 2..=9 and stack_bb > 0".into());
        }
        let s = &self.sizing;
        let pos = [
            s.open_bb,
            s.sb_open_bb,
            s.threebet_ip_mult,
            s.threebet_oop_mult,
            s.fourbet_mult,
            s.round_bb,
            s.allin_above_stack_pct,
        ];
        if pos.iter().any(|x| !x.is_finite() || *x <= 0.0) || s.squeeze_per_caller_mult < 0.0 {
            return Err("sizing: values must be positive and finite".into());
        }
        if s.open_bb < 2.0 || s.max_raises < 1 {
            return Err("sizing: open_bb >= 2 and max_raises >= 1".into());
        }
        if self.tree.max_players_postflop < 2 {
            return Err("tree: max_players_postflop >= 2".into());
        }
        let r = &self.realization;
        let factors = [r.srp, r.threebet, r.fourbet].concat();
        if factors.iter().any(|x| !x.is_finite() || *x <= 0.0)
            || r.low_spr < 0.0
            || !(1.0..=4.0).contains(&r.equity_exponent)
        {
            return Err("realization: factors must be positive, equity_exponent in 1..=4".into());
        }
        if self.solve.iterations == 0 || self.solve.equity_samples == 0 {
            return Err("solve: iterations and equity_samples > 0".into());
        }
        Ok(())
    }

    pub fn game_config(&self) -> GameConfig {
        GameConfig::cash(self.game.table_size, self.game.stack_bb)
    }

    /// bb → chips, rounded to `round_bb`.
    pub fn round_chips(&self, bb: f64) -> Chips {
        let step = self.sizing.round_bb * BB_CHIPS as f64;
        ((bb * BB_CHIPS as f64 / step).round() * step).round() as Chips
    }

    /// BLAKE3 of the canonical TOML, stored in every output file.
    pub fn hash(&self) -> [u8; 32] {
        let canon = toml::to_string(self).expect("manifest serializes");
        *blake3::hash(canon.as_bytes()).as_bytes()
    }

    /// One-line description of the sizing tree for the file header.
    pub fn bet_tree_descriptor(&self) -> String {
        let s = &self.sizing;
        format!(
            "preflop: open {}bb (SB {}bb); 3-bet {}x IP / {}x OOP (+{}x per caller); 4-bet {}x; \
             {} raises max, last all-in; all-in above {}% stack; round {}bb; \
             cold calls up to raise {}; max {} players postflop",
            s.open_bb,
            s.sb_open_bb,
            s.threebet_ip_mult,
            s.threebet_oop_mult,
            s.squeeze_per_caller_mult,
            s.fourbet_mult,
            s.max_raises,
            s.allin_above_stack_pct,
            s.round_bb,
            self.tree.cold_call_max_level,
            self.tree.max_players_postflop,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_manifest_parses() {
        let m =
            Manifest::from_toml(include_str!("../../manifests/preflop-6max-100bb.toml")).unwrap();
        assert_eq!(m.game_config(), GameConfig::cash(6, 100));
        assert_eq!(m.round_chips(2.5), 250);
        assert_eq!(m.round_chips(9.375), 950);
        assert_eq!(m.hash(), m.clone().hash());
    }
}

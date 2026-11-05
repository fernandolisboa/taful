//! `GameConfig` (spec §5.2, D1): every parameter that changes strategy.
//!
//! All amounts are integer chips. Cash games use `bb = 100` chips
//! ([`BB_CHIPS`]) so 2.5bb opens and 33% bets are exact. Nothing in the
//! engine special-cases a table size or stack depth.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Integer chip amount.
pub type Chips = u64;

/// Chips per big blind in cash games.
pub const BB_CHIPS: Chips = 100;

/// Every parameter that changes strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameConfig {
    /// Players dealt in, `2..=9`.
    pub table_size: u8,
    pub format: Format,
    pub blinds: Blinds,
    pub stacks: Stacks,
    #[serde(default)]
    pub rake: Rake,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Format {
    Cash,
    Mtt(TournamentSpec),
    Sng(TournamentSpec),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TournamentSpec {
    /// Prize fractions (or amounts) by finishing place, first place first.
    pub payouts: Vec<f64>,
    pub structure: BlindStructure,
    pub players_left: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlindStructure {
    pub speed: Speed,
    pub levels: Vec<BlindLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Hyper,
    Turbo,
    Regular,
    Slow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlindLevel {
    pub sb: Chips,
    pub bb: Chips,
    #[serde(default)]
    pub ante: Chips,
    pub minutes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blinds {
    pub sb: Chips,
    pub bb: Chips,
    #[serde(default)]
    pub ante: Chips,
    #[serde(default)]
    pub ante_kind: AnteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnteKind {
    #[default]
    None,
    /// Every player posts `ante`.
    PerPlayer,
    /// The big blind posts one `ante` for the table (dead money in the main pot).
    BigBlind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stacks {
    /// Every seat starts with the same stack.
    Uniform { chips: Chips },
    /// Stack per seat, indexed by seat number.
    PerSeat { chips: Vec<Chips> },
}

/// Rake: `rate_bps` basis points of the pot (500 = 5%), capped at `cap` chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rake {
    pub rate_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<Chips>,
    /// No rake when the hand ends before the flop.
    #[serde(default)]
    pub no_flop_no_drop: bool,
}

impl Rake {
    pub const ZERO: Rake = Rake {
        rate_bps: 0,
        cap: None,
        no_flop_no_drop: false,
    };

    /// Rake taken from a pot of `pot` chips.
    pub fn amount(&self, pot: Chips, saw_flop: bool) -> Chips {
        if self.rate_bps == 0 || (self.no_flop_no_drop && !saw_flop) {
            return 0;
        }
        let r = (u128::from(pot) * u128::from(self.rate_bps) / 10_000) as Chips;
        self.cap.map_or(r, |c| r.min(c))
    }
}

/// Invalid configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Stable content hash of a config's canonical TOML (BLAKE3). First component
/// of every library key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfigKey(pub [u8; 32]);

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConfigKey({self})")
    }
}

impl GameConfig {
    /// Cash game, `bb = 100` chips, uniform stacks of `stack_bb`, no ante, no rake.
    pub fn cash(table_size: u8, stack_bb: u32) -> GameConfig {
        GameConfig {
            table_size,
            format: Format::Cash,
            blinds: Blinds {
                sb: BB_CHIPS / 2,
                bb: BB_CHIPS,
                ante: 0,
                ante_kind: AnteKind::None,
            },
            stacks: Stacks::Uniform {
                chips: Chips::from(stack_bb) * BB_CHIPS,
            },
            rake: Rake::ZERO,
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let err = |m: &str| Err(ConfigError(m.into()));
        if !(2..=9).contains(&self.table_size) {
            return err("table_size must be 2..=9");
        }
        let b = &self.blinds;
        if b.bb == 0 || b.sb > b.bb {
            return err("blinds need bb > 0 and sb <= bb");
        }
        if b.ante_kind == AnteKind::None && b.ante != 0 {
            return err("ante set but ante_kind is none");
        }
        match &self.stacks {
            Stacks::Uniform { chips } if *chips == 0 => return err("stacks must be > 0"),
            Stacks::PerSeat { chips } if chips.len() != usize::from(self.table_size) => {
                return err("per-seat stacks must list every seat");
            }
            Stacks::PerSeat { chips } if chips.contains(&0) => return err("stacks must be > 0"),
            _ => {}
        }
        if self.rake.rate_bps > 10_000 {
            return err("rake rate above 100%");
        }
        if let Format::Mtt(t) | Format::Sng(t) = &self.format
            && (t.payouts.is_empty() || t.payouts.iter().any(|p| !p.is_finite() || *p < 0.0))
        {
            return err("payouts must be non-empty and non-negative");
        }
        Ok(())
    }

    /// Starting stack of `seat`.
    pub fn stack(&self, seat: usize) -> Chips {
        match &self.stacks {
            Stacks::Uniform { chips } => *chips,
            Stacks::PerSeat { chips } => chips[seat],
        }
    }

    /// Canonical TOML form (field order is fixed by the struct definitions).
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("GameConfig always serializes")
    }

    pub fn from_toml(s: &str) -> Result<GameConfig, ConfigError> {
        let c: GameConfig = toml::from_str(s).map_err(|e| ConfigError(e.to_string()))?;
        c.validate()?;
        Ok(c)
    }

    /// Stable key: BLAKE3 of [`to_toml`](Self::to_toml).
    pub fn key(&self) -> ConfigKey {
        ConfigKey(*blake3::hash(self.to_toml().as_bytes()).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_roundtrip_and_key_is_stable() {
        let c = GameConfig::cash(6, 100);
        c.validate().unwrap();
        let t = c.to_toml();
        let back = GameConfig::from_toml(&t).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.key(), c.key());
        // Pinned: changing this means every library key changes. Bump
        // `MODEL_VERSION` and the library format together if it ever must.
        assert_eq!(
            c.key().to_string(),
            "c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f",
            "canonical TOML:\n{t}"
        );
    }

    #[test]
    fn keys_differ_by_parameter() {
        let a = GameConfig::cash(6, 100);
        let mut b = a.clone();
        b.rake.rate_bps = 500;
        let mut c = a.clone();
        c.table_size = 9;
        assert_ne!(a.key(), b.key());
        assert_ne!(a.key(), c.key());
    }

    #[test]
    fn tournament_roundtrip() {
        let c = GameConfig {
            table_size: 9,
            format: Format::Mtt(TournamentSpec {
                payouts: vec![0.5, 0.3, 0.2],
                structure: BlindStructure {
                    speed: Speed::Turbo,
                    levels: vec![BlindLevel {
                        sb: 100,
                        bb: 200,
                        ante: 200,
                        minutes: 6,
                    }],
                },
                players_left: 27,
            }),
            blinds: Blinds {
                sb: 100,
                bb: 200,
                ante: 200,
                ante_kind: AnteKind::BigBlind,
            },
            stacks: Stacks::PerSeat {
                chips: vec![5000, 6000, 7000, 8000, 9000, 10000, 11000, 12000, 13000],
            },
            rake: Rake::ZERO,
        };
        assert_eq!(GameConfig::from_toml(&c.to_toml()).unwrap(), c);
    }

    #[test]
    fn validation() {
        let mut c = GameConfig::cash(10, 100);
        assert!(c.validate().is_err());
        c.table_size = 2;
        assert!(c.validate().is_ok());
        c.stacks = Stacks::PerSeat { chips: vec![100] };
        assert!(c.validate().is_err());
    }

    #[test]
    fn rake_amount() {
        let r = Rake {
            rate_bps: 500,
            cap: Some(300),
            no_flop_no_drop: true,
        };
        assert_eq!(r.amount(1000, true), 50);
        assert_eq!(r.amount(100_000, true), 300);
        assert_eq!(r.amount(1000, false), 0);
    }
}

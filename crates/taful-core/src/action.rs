//! Actions, lines, action histories and the bet-sizing tree (spec §5.3).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::config::Chips;
use crate::position::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

impl Street {
    pub const ALL: [Street; 4] = [Street::Preflop, Street::Flop, Street::Turn, Street::River];

    pub fn next(self) -> Option<Street> {
        match self {
            Street::Preflop => Some(Street::Flop),
            Street::Flop => Some(Street::Turn),
            Street::Turn => Some(Street::River),
            Street::River => None,
        }
    }
    /// Board size once this street is dealt.
    pub fn board_len(self) -> usize {
        match self {
            Street::Preflop => 0,
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River => 5,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A player action. `Bet` and `Raise` carry the player's **total** commitment
/// on this street after the action ("raise to"), in chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(Chips),
    Raise(Chips),
    AllIn,
}

impl Action {
    /// A bet, a raise or an all-in.
    pub fn is_aggressive(self) -> bool {
        matches!(self, Action::Bet(_) | Action::Raise(_) | Action::AllIn)
    }
}

impl fmt::Display for Action {
    /// Canonical short form: `f`, `x`, `c`, `b<chips>`, `r<to>`, `a`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Fold => f.write_str("f"),
            Action::Check => f.write_str("x"),
            Action::Call => f.write_str("c"),
            Action::Bet(n) => write!(f, "b{n}"),
            Action::Raise(n) => write!(f, "r{n}"),
            Action::AllIn => f.write_str("a"),
        }
    }
}

impl FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Action, String> {
        let num = |t: &str| {
            t.parse::<Chips>()
                .map_err(|_| format!("bad amount in {s:?}"))
        };
        match s {
            "f" => Ok(Action::Fold),
            "x" => Ok(Action::Check),
            "c" => Ok(Action::Call),
            "a" => Ok(Action::AllIn),
            _ if s.starts_with('b') => Ok(Action::Bet(num(&s[1..])?)),
            _ if s.starts_with('r') => Ok(Action::Raise(num(&s[1..])?)),
            _ => Err(format!("bad action {s:?}")),
        }
    }
}

/// Per-street actions by position. Canonical form: actions as `POS:act`
/// joined by `,`, streets joined by `/`, e.g. `BTN:r250,SB:f,BB:c/BB:x,BTN:b120`.
///
/// A history always has at least the preflop street, so `""` is the empty
/// preflop street and every value round-trips through its string form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionHistory {
    pub streets: Vec<Vec<(Position, Action)>>,
}

impl Default for ActionHistory {
    fn default() -> Self {
        ActionHistory {
            streets: vec![Vec::new()],
        }
    }
}

/// The preflop part of an [`ActionHistory`]: identifies a configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Line(pub Vec<(Position, Action)>);

fn fmt_street(acts: &[(Position, Action)]) -> String {
    acts.iter()
        .map(|(p, a)| format!("{p}:{a}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_street(s: &str) -> Result<Vec<(Position, Action)>, String> {
    if s.is_empty() {
        return Ok(vec![]);
    }
    s.split(',')
        .map(|t| {
            let (p, a) = t
                .split_once(':')
                .ok_or_else(|| format!("bad action {t:?}"))?;
            Ok((p.parse()?, a.parse()?))
        })
        .collect()
}

impl ActionHistory {
    pub fn line(&self) -> Line {
        Line(self.streets.first().cloned().unwrap_or_default())
    }
}

impl fmt::Display for ActionHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.streets.iter().map(|s| fmt_street(s)).collect();
        f.write_str(&parts.join("/"))
    }
}

impl FromStr for ActionHistory {
    type Err = String;
    fn from_str(s: &str) -> Result<ActionHistory, String> {
        Ok(ActionHistory {
            streets: s.split('/').map(parse_street).collect::<Result<_, _>>()?,
        })
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&fmt_street(&self.0))
    }
}

impl FromStr for Line {
    type Err = String;
    fn from_str(s: &str) -> Result<Line, String> {
        parse_street(s).map(Line)
    }
}

/// One sizing option in a [`BetTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetSize {
    /// Percent of the pot. For a raise: call first, then raise this share of
    /// the pot after the call.
    PotPct(u32),
    /// Raise to this multiple (in hundredths) of the bet faced: `300` = 3x.
    Mult(u32),
    /// Absolute size in hundredths of a big blind, e.g. a 2.5bb open = `250`.
    Bb(u32),
    AllIn,
}

/// Sizes allowed on one street.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreetSizes {
    /// First bet on a street with no bet yet (unused preflop: the big blind is the bet).
    #[serde(default)]
    pub bets: Vec<BetSize>,
    /// `raises[d]` = sizes for the `(d + 1)`-th raise on this street; the last
    /// entry repeats for deeper raises. Preflop, `raises[0]` are the opens.
    /// Empty = no raising.
    #[serde(default)]
    pub raises: Vec<Vec<BetSize>>,
    /// A size that would commit at least this percent of the player's stack
    /// becomes all-in (`0` = off).
    #[serde(default)]
    pub allin_threshold_pct: u32,
}

/// Allowed sizes per street, mirroring the solver's tree config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BetTree {
    pub preflop: StreetSizes,
    pub flop: StreetSizes,
    pub turn: StreetSizes,
    pub river: StreetSizes,
}

impl BetTree {
    pub fn street(&self, s: Street) -> &StreetSizes {
        match s {
            Street::Preflop => &self.preflop,
            Street::Flop => &self.flop,
            Street::Turn => &self.turn,
            Street::River => &self.river,
        }
    }

    /// Sizes for the next aggressive action when `aggressions` bets/raises have
    /// already happened on this street (preflop the big blind does not count).
    pub fn sizes(&self, s: Street, aggressions: usize) -> &[BetSize] {
        let st = self.street(s);
        if s != Street::Preflop && aggressions == 0 {
            return &st.bets;
        }
        let depth = if s == Street::Preflop {
            aggressions
        } else {
            aggressions - 1
        };
        match st.raises.get(depth).or(st.raises.last()) {
            Some(v) => v,
            None => &[],
        }
    }

    /// The spike's medium SRP tree (spike §9.2) with standard preflop sizes:
    /// opens 2.5bb, 3-bets 3x, 4-bets 2.2x, then all-in.
    pub fn default_srp() -> BetTree {
        use BetSize::*;
        let post = |bets: Vec<BetSize>| StreetSizes {
            bets,
            raises: vec![vec![Mult(300), AllIn]],
            allin_threshold_pct: 67,
        };
        BetTree {
            preflop: StreetSizes {
                bets: vec![],
                raises: vec![vec![Bb(250)], vec![Mult(300)], vec![Mult(220)], vec![AllIn]],
                allin_threshold_pct: 67,
            },
            flop: post(vec![PotPct(33), PotPct(75)]),
            turn: post(vec![PotPct(50), PotPct(100), AllIn]),
            river: post(vec![PotPct(50), PotPct(100), AllIn]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_roundtrip() {
        let s = "BTN:r250,SB:f,BB:c/BB:x,BTN:b120,BB:a,BTN:c";
        let h: ActionHistory = s.parse().unwrap();
        assert_eq!(h.to_string(), s);
        assert_eq!(h.line().to_string(), "BTN:r250,SB:f,BB:c");
        assert!("BTN:q".parse::<ActionHistory>().is_err());
        let d = ActionHistory::default();
        assert_eq!(d.to_string().parse::<ActionHistory>().unwrap(), d);
    }

    #[test]
    fn tree_depths() {
        let t = BetTree::default_srp();
        assert_eq!(t.sizes(Street::Preflop, 0), &[BetSize::Bb(250)]);
        assert_eq!(t.sizes(Street::Preflop, 7), &[BetSize::AllIn]);
        assert_eq!(t.sizes(Street::Flop, 0).len(), 2);
        assert_eq!(
            t.sizes(Street::Flop, 3),
            &[BetSize::Mult(300), BetSize::AllIn]
        );
    }
}

//! Table positions derived from table size and button seat (spec §5.3).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Seat index, `0..table_size`.
pub type Seat = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Position {
    Utg,
    Utg1,
    Utg2,
    Lj,
    Hj,
    Co,
    /// Button. Heads-up, the button also posts the small blind.
    Btn,
    Sb,
    Bb,
}

impl Position {
    pub const ALL: [Position; 9] = [
        Position::Utg,
        Position::Utg1,
        Position::Utg2,
        Position::Lj,
        Position::Hj,
        Position::Co,
        Position::Btn,
        Position::Sb,
        Position::Bb,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Position::Utg => "UTG",
            Position::Utg1 => "UTG1",
            Position::Utg2 => "UTG2",
            Position::Lj => "LJ",
            Position::Hj => "HJ",
            Position::Co => "CO",
            Position::Btn => "BTN",
            Position::Sb => "SB",
            Position::Bb => "BB",
        }
    }

    /// Positions indexed by offset from the button (offset 0 = BTN).
    ///
    /// 6-max is `BTN SB BB UTG HJ CO`; 9-max adds `UTG1 UTG2 LJ`; heads-up is
    /// `BTN BB`.
    pub fn by_button_offset(table_size: usize) -> Vec<Position> {
        use Position::*;
        assert!((2..=9).contains(&table_size), "table_size {table_size}");
        if table_size == 2 {
            return vec![Btn, Bb];
        }
        let middle = table_size - 3;
        let mut early: Vec<Position> = match middle {
            0 => vec![],
            1 => vec![Co],
            2 => vec![Hj, Co],
            3 => vec![Utg, Hj, Co],
            n => {
                let mut v = vec![Utg, Utg1, Utg2][..n - 3].to_vec();
                v.extend([Lj, Hj, Co]);
                v
            }
        };
        let mut out = vec![Btn, Sb, Bb];
        out.append(&mut early);
        out
    }

    /// Position of `seat` with the button on `button`.
    pub fn of(table_size: usize, button: Seat, seat: Seat) -> Position {
        let offset = (seat + table_size - button) % table_size;
        Position::by_button_offset(table_size)[offset]
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Position {
    type Err = String;
    fn from_str(s: &str) -> Result<Position, String> {
        Position::ALL
            .into_iter()
            .find(|p| p.label().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown position {s:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let s = |n| {
            Position::by_button_offset(n)
                .iter()
                .map(|p| p.label())
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(s(2), "BTN BB");
        assert_eq!(s(3), "BTN SB BB");
        assert_eq!(s(6), "BTN SB BB UTG HJ CO");
        assert_eq!(s(9), "BTN SB BB UTG UTG1 UTG2 LJ HJ CO");
        for n in 2..=9 {
            let v = Position::by_button_offset(n);
            let mut d = v.clone();
            d.sort();
            d.dedup();
            assert_eq!(d.len(), n);
        }
        assert_eq!(Position::of(6, 4, 0), Position::Bb);
        assert_eq!("co".parse::<Position>().unwrap(), Position::Co);
    }
}

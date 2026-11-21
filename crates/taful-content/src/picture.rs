//! Pictures in lessons (D26): fenced blocks whose info string names a
//! picture kind, drawn by the app instead of shown as code.
//!
//! Cards, one row per line, with an optional caption before a colon:
//!
//! ````text
//! ```cards
//! Your cards: As Kd
//! Board: Ah 8c 6d ~4s ~2c
//! Opponent: ?? ??
//! ```
//! ````
//!
//! `??` is a face-down card, `|` a wider gap between groups, and a `~`
//! before a card dims it (a card that is not part of the hand being shown).
//! A card shows at most once per row.
//!
//! A table seen from above, one `key: value` per line, every key optional:
//!
//! ````text
//! ```seats
//! players: 6                     # 2 to 9, default 6
//! you: CO                        # the reader's seat, drawn at the bottom
//! mark: SB BB                    # seats to highlight
//! out: UTG HJ                    # seats that folded, drawn faded
//! order: UTG HJ CO BTN SB BB     # numbered 1, 2, 3… (order of play)
//! blinds: yes                    # the blinds' chips in front of SB and BB
//! ```
//! ````
//!
//! Positions are the usual labels (UTG, HJ, CO, BTN, SB, BB…), which read
//! the same in every locale; only captions are translated, so every locale
//! must draw the same cards and seats (checked by `content-check`).

use std::collections::BTreeSet;

use taful_core::cards::Card;
use taful_core::position::{Position, Seat};

/// The info strings that make a fenced block a picture.
pub const KINDS: [&str; 2] = ["cards", "seats"];

/// One card slot in a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardSlot {
    Face {
        card: Card,
        dim: bool,
    },
    /// Face down.
    Back,
    /// A wider gap between groups.
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRow {
    pub caption: Option<String>,
    pub cards: Vec<CardSlot>,
}

/// A table seen from above. Seats are numbered from the button (seat 0)
/// clockwise, as in the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatMap {
    pub players: u8,
    pub you: Option<Position>,
    pub mark: Vec<Position>,
    pub out: Vec<Position>,
    pub order: Vec<Position>,
    pub blinds: bool,
}

impl SeatMap {
    /// The seat each position sits in (button on seat 0).
    pub fn seat_of(&self, pos: Position) -> Option<Seat> {
        let n = usize::from(self.players);
        (0..n).find(|&s| Position::of(n, 0, s) == pos)
    }

    /// The position at `seat`.
    pub fn position(&self, seat: Seat) -> Position {
        Position::of(usize::from(self.players), 0, seat)
    }

    /// 1-based place of `pos` in `order`.
    pub fn order_of(&self, pos: Position) -> Option<usize> {
        self.order.iter().position(|&p| p == pos).map(|i| i + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picture {
    Cards(Vec<CardRow>),
    Seats(SeatMap),
}

impl Picture {
    /// Parses a fenced block of kind `kind` (one of [`KINDS`]).
    pub fn parse(kind: &str, text: &str) -> Result<Picture, String> {
        match kind {
            "cards" => parse_cards(text).map(Picture::Cards),
            "seats" => parse_seats(text).map(Picture::Seats),
            _ => Err(format!("unknown picture `{kind}`")),
        }
    }

    /// The picture without its captions: what every locale must agree on.
    pub fn shape(&self) -> Picture {
        match self {
            Picture::Cards(rows) => Picture::Cards(
                rows.iter()
                    .map(|r| CardRow {
                        caption: None,
                        cards: r.cards.clone(),
                    })
                    .collect(),
            ),
            Picture::Seats(s) => Picture::Seats(s.clone()),
        }
    }

    /// Captions, for the plain text of a lesson.
    pub fn text(&self) -> String {
        match self {
            Picture::Cards(rows) => rows
                .iter()
                .map(|r| {
                    let cards = r
                        .cards
                        .iter()
                        .map(|c| match c {
                            CardSlot::Face { card, .. } => crate::format::card(*card),
                            CardSlot::Back => "??".to_owned(),
                            CardSlot::Gap => "|".to_owned(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    match &r.caption {
                        Some(c) => format!("{c}: {cards}"),
                        None => cards,
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Picture::Seats(s) => (0..usize::from(s.players))
                .map(|seat| s.position(seat).label())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.split('#').next().unwrap_or("").trim()))
        .filter(|(_, l)| !l.is_empty())
}

fn parse_cards(text: &str) -> Result<Vec<CardRow>, String> {
    let mut rows = Vec::new();
    for (n, line) in lines(text) {
        // A card may show again in another row (the board, then the best
        // five from it), never twice in one.
        let mut seen = BTreeSet::new();
        let (caption, cards) = match line.split_once(':') {
            Some((c, rest)) => (Some(c.trim().to_owned()).filter(|c| !c.is_empty()), rest),
            None => (None, line),
        };
        let mut slots = Vec::new();
        for tok in cards.split_whitespace() {
            let slot = match tok {
                "??" => CardSlot::Back,
                "|" => CardSlot::Gap,
                _ => {
                    let (dim, code) = match tok.strip_prefix('~') {
                        Some(c) => (true, c),
                        None => (false, tok),
                    };
                    let card: Card = code.parse().map_err(|e| format!("cards line {n}: {e}"))?;
                    if !seen.insert(card) {
                        return Err(format!("cards line {n}: {card} shows twice"));
                    }
                    CardSlot::Face { card, dim }
                }
            };
            slots.push(slot);
        }
        if !slots.iter().any(|s| !matches!(s, CardSlot::Gap)) {
            return Err(format!("cards line {n}: no cards"));
        }
        rows.push(CardRow {
            caption,
            cards: slots,
        });
    }
    if rows.is_empty() {
        return Err("cards: empty picture".into());
    }
    Ok(rows)
}

fn parse_seats(text: &str) -> Result<SeatMap, String> {
    let mut map = SeatMap {
        players: 6,
        you: None,
        mark: Vec::new(),
        out: Vec::new(),
        order: Vec::new(),
        blinds: false,
    };
    let mut lists: Vec<(usize, &str, &str)> = Vec::new();
    for (n, line) in lines(text) {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("seats line {n}: expected `key: value`"))?;
        let (key, value) = (key.trim(), value.trim());
        match key {
            "players" => {
                map.players = value
                    .parse()
                    .ok()
                    .filter(|p| (2..=9).contains(p))
                    .ok_or_else(|| format!("seats line {n}: players is 2 to 9"))?;
            }
            "blinds" => {
                map.blinds = match value {
                    "yes" => true,
                    "no" => false,
                    _ => return Err(format!("seats line {n}: blinds is `yes` or `no`")),
                }
            }
            "you" | "mark" | "out" | "order" => lists.push((n, key, value)),
            _ => return Err(format!("seats line {n}: unknown key `{key}`")),
        }
    }
    // Positions after `players`, wherever it was written.
    for (n, key, value) in lists {
        let mut positions = Vec::new();
        for tok in value.split_whitespace() {
            let p: Position = tok.parse().map_err(|e| format!("seats line {n}: {e}"))?;
            if map.seat_of(p).is_none() {
                return Err(format!(
                    "seats line {n}: no {p} at a {}-handed table",
                    map.players
                ));
            }
            if positions.contains(&p) {
                return Err(format!("seats line {n}: {p} twice"));
            }
            positions.push(p);
        }
        match key {
            "you" => {
                if positions.len() != 1 {
                    return Err(format!("seats line {n}: `you` is one seat"));
                }
                map.you = positions.first().copied();
            }
            "mark" => map.mark = positions,
            "out" => map.out = positions,
            _ => map.order = positions,
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_rows() {
        let p =
            Picture::parse("cards", "Your cards: As Kd\n\nBoard: Ah 8c | ~6d\n?? ??\n").unwrap();
        let Picture::Cards(rows) = &p else { panic!() };
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].caption.as_deref(), Some("Your cards"));
        assert_eq!(rows[1].cards.len(), 4);
        assert_eq!(rows[1].cards[2], CardSlot::Gap);
        assert!(matches!(rows[1].cards[3], CardSlot::Face { dim: true, .. }));
        assert_eq!(rows[2].caption, None);
        assert_eq!(rows[2].cards, vec![CardSlot::Back, CardSlot::Back]);
        assert_eq!(p.text(), "Your cards: A♠ K♦\nBoard: A♥ 8♣ | 6♦\n?? ??");
        assert_eq!(
            p.shape(),
            Picture::parse("cards", "Suas cartas: As Kd\nMesa: Ah 8c | ~6d\n?? ??")
                .unwrap()
                .shape()
        );
    }

    #[test]
    fn bad_cards() {
        assert!(Picture::parse("cards", "").is_err());
        assert!(Picture::parse("cards", "Mesa: Zz").is_err());
        assert!(
            Picture::parse("cards", "As Ks As").is_err(),
            "a card twice in a row"
        );
        assert!(
            Picture::parse("cards", "As Ks\nAs").is_ok(),
            "again in another row"
        );
        assert!(Picture::parse("cards", "Mesa: |").is_err());
    }

    #[test]
    fn seats() {
        let p = Picture::parse(
            "seats",
            "you: CO\nmark: SB BB # the blinds\norder: UTG HJ CO BTN SB BB\nblinds: yes\nout: UTG",
        )
        .unwrap();
        let Picture::Seats(s) = p else { panic!() };
        assert_eq!(s.players, 6);
        assert_eq!(s.you, Some(Position::Co));
        assert_eq!(s.mark, vec![Position::Sb, Position::Bb]);
        assert_eq!(s.order_of(Position::Btn), Some(4));
        assert_eq!(s.seat_of(Position::Btn), Some(0));
        assert_eq!(s.position(1), Position::Sb);
        assert!(s.blinds);
        assert_eq!(s.out, vec![Position::Utg]);
        let hu = Picture::parse("seats", "you: BB\nplayers: 2").unwrap();
        assert!(matches!(hu, Picture::Seats(SeatMap { players: 2, .. })));
    }

    #[test]
    fn bad_seats() {
        assert!(Picture::parse("seats", "players: 10").is_err());
        assert!(Picture::parse("seats", "you: CO BTN").is_err());
        assert!(Picture::parse("seats", "mark: XX").is_err());
        assert!(Picture::parse("seats", "players: 3\nmark: UTG").is_err());
        assert!(Picture::parse("seats", "colour: red").is_err());
        assert!(Picture::parse("seats", "blinds: maybe").is_err());
        assert!(Picture::parse("seats", "order: BTN BTN").is_err());
        assert!(Picture::parse("nope", "").is_err());
    }
}

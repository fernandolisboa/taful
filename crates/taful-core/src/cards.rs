//! Cards, hole-card hands, boards and the canonical 1,326-combo index (spec §5.1).
//!
//! Encoding: `Card(rank * 4 + suit)` with ranks `2..=A` as `0..=12` and suits
//! `c, d, h, s` as `0..=3`. The combo index is the triangular index of the two
//! card ids (high id first) and is shared with the library format (§7.5), so it
//! must never change without bumping the library `format_version`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Number of distinct two-card hands.
pub const NUM_COMBOS: usize = 1326;

/// Card rank, `Two` (0) to `Ace` (12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(u8);

impl Rank {
    pub const TWO: Rank = Rank(0);
    pub const FIVE: Rank = Rank(3);
    pub const TEN: Rank = Rank(8);
    pub const JACK: Rank = Rank(9);
    pub const QUEEN: Rank = Rank(10);
    pub const KING: Rank = Rank(11);
    pub const ACE: Rank = Rank(12);
    const CHARS: &'static [u8; 13] = b"23456789TJQKA";

    /// `index` in `0..13`.
    pub const fn new(index: u8) -> Option<Rank> {
        if index < 13 { Some(Rank(index)) } else { None }
    }
    pub const fn index(self) -> u8 {
        self.0
    }
    pub fn all() -> impl DoubleEndedIterator<Item = Rank> {
        (0..13).map(Rank)
    }
    pub fn to_char(self) -> char {
        Self::CHARS[self.0 as usize] as char
    }
    pub fn from_char(c: char) -> Option<Rank> {
        let c = c.to_ascii_uppercase() as u8;
        Self::CHARS
            .iter()
            .position(|&x| x == c)
            .map(|i| Rank(i as u8))
    }
}

/// Card suit: clubs, diamonds, hearts, spades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Suit(u8);

impl Suit {
    pub const CLUBS: Suit = Suit(0);
    pub const DIAMONDS: Suit = Suit(1);
    pub const HEARTS: Suit = Suit(2);
    pub const SPADES: Suit = Suit(3);
    const CHARS: &'static [u8; 4] = b"cdhs";

    pub const fn new(index: u8) -> Option<Suit> {
        if index < 4 { Some(Suit(index)) } else { None }
    }
    pub const fn index(self) -> u8 {
        self.0
    }
    pub fn all() -> impl DoubleEndedIterator<Item = Suit> {
        (0..4).map(Suit)
    }
    pub fn to_char(self) -> char {
        Self::CHARS[self.0 as usize] as char
    }
    pub fn from_char(c: char) -> Option<Suit> {
        let c = c.to_ascii_lowercase() as u8;
        Self::CHARS
            .iter()
            .position(|&x| x == c)
            .map(|i| Suit(i as u8))
    }
}

/// One card, `0..52`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Card(u8);

impl Card {
    pub const fn new(rank: Rank, suit: Suit) -> Card {
        Card(rank.0 * 4 + suit.0)
    }
    pub const fn from_index(index: u8) -> Option<Card> {
        if index < 52 { Some(Card(index)) } else { None }
    }
    pub const fn index(self) -> u8 {
        self.0
    }
    pub const fn rank(self) -> Rank {
        Rank(self.0 / 4)
    }
    pub const fn suit(self) -> Suit {
        Suit(self.0 % 4)
    }
    pub const fn bit(self) -> u64 {
        1u64 << self.0
    }
    /// All 52 cards in index order.
    pub fn all() -> impl DoubleEndedIterator<Item = Card> + Clone {
        (0..52).map(Card)
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank().to_char(), self.suit().to_char())
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Error parsing cards, hands, boards or ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

fn perr(msg: impl Into<String>) -> ParseError {
    ParseError(msg.into())
}

impl FromStr for Card {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Card, ParseError> {
        let mut it = s.chars();
        match (it.next(), it.next(), it.next()) {
            (Some(r), Some(su), None) => {
                let rank = Rank::from_char(r).ok_or_else(|| perr(format!("bad rank in {s:?}")))?;
                let suit = Suit::from_char(su).ok_or_else(|| perr(format!("bad suit in {s:?}")))?;
                Ok(Card::new(rank, suit))
            }
            _ => Err(perr(format!("bad card {s:?}"))),
        }
    }
}

impl TryFrom<String> for Card {
    type Error = ParseError;
    fn try_from(s: String) -> Result<Card, ParseError> {
        s.parse()
    }
}

impl From<Card> for String {
    fn from(c: Card) -> String {
        c.to_string()
    }
}

/// Parse a run of concatenated cards, e.g. `"AsKd7h"`. Spaces and commas are ignored.
pub fn parse_cards(s: &str) -> Result<Vec<Card>, ParseError> {
    let chars: Vec<char> = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if !chars.len().is_multiple_of(2) {
        return Err(perr(format!("odd number of characters in {s:?}")));
    }
    let cards: Vec<Card> = chars
        .chunks(2)
        .map(|p| p.iter().collect::<String>().parse())
        .collect::<Result<_, _>>()?;
    let set = CardSet::from_cards(&cards);
    if set.len() != cards.len() {
        return Err(perr(format!("duplicate card in {s:?}")));
    }
    Ok(cards)
}

/// A set of cards as a 52-bit mask.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CardSet(pub u64);

impl CardSet {
    pub const EMPTY: CardSet = CardSet(0);
    pub const FULL: CardSet = CardSet((1u64 << 52) - 1);

    pub fn from_cards(cards: &[Card]) -> CardSet {
        CardSet(cards.iter().fold(0, |m, c| m | c.bit()))
    }
    pub const fn contains(self, c: Card) -> bool {
        self.0 & c.bit() != 0
    }
    pub fn insert(&mut self, c: Card) {
        self.0 |= c.bit();
    }
    pub const fn with(self, c: Card) -> CardSet {
        CardSet(self.0 | c.bit())
    }
    pub const fn union(self, o: CardSet) -> CardSet {
        CardSet(self.0 | o.0)
    }
    pub const fn intersects(self, o: CardSet) -> bool {
        self.0 & o.0 != 0
    }
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn iter(self) -> impl Iterator<Item = Card> {
        let mut m = self.0;
        std::iter::from_fn(move || {
            if m == 0 {
                return None;
            }
            let i = m.trailing_zeros() as u8;
            m &= m - 1;
            Some(Card(i))
        })
    }
    /// Cards not in this set, in index order.
    pub fn complement(self) -> CardSet {
        CardSet(!self.0 & Self::FULL.0)
    }
}

/// Two hole cards, normalized high card first (by card index).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hand([Card; 2]);

impl Hand {
    /// `None` if both cards are the same.
    pub fn new(a: Card, b: Card) -> Option<Hand> {
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(Hand([a, b])),
            std::cmp::Ordering::Less => Some(Hand([b, a])),
            std::cmp::Ordering::Equal => None,
        }
    }
    pub const fn cards(self) -> [Card; 2] {
        self.0
    }
    pub const fn high(self) -> Card {
        self.0[0]
    }
    pub const fn low(self) -> Card {
        self.0[1]
    }
    pub const fn mask(self) -> CardSet {
        CardSet(self.0[0].bit() | self.0[1].bit())
    }
    pub fn is_pair(self) -> bool {
        self.0[0].rank() == self.0[1].rank()
    }
    pub fn is_suited(self) -> bool {
        self.0[0].suit() == self.0[1].suit()
    }
    /// Canonical combo index in `0..1326`.
    pub const fn combo(self) -> usize {
        let hi = self.0[0].0 as usize;
        let lo = self.0[1].0 as usize;
        hi * (hi - 1) / 2 + lo
    }
    /// Inverse of [`Hand::combo`].
    pub fn from_combo(index: usize) -> Option<Hand> {
        COMBO_TABLE.get(index).copied()
    }
    /// All 1,326 hands in combo-index order.
    pub fn all() -> impl ExactSizeIterator<Item = Hand> + Clone {
        COMBO_TABLE.iter().copied()
    }
}

static COMBO_TABLE: std::sync::LazyLock<Vec<Hand>> = std::sync::LazyLock::new(|| {
    let mut v = Vec::with_capacity(NUM_COMBOS);
    for hi in 1..52u8 {
        for lo in 0..hi {
            v.push(Hand([Card(hi), Card(lo)]));
        }
    }
    v
});

impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Higher rank first for display; ties (pairs) by suit index.
        let [a, b] = self.0;
        write!(f, "{a}{b}")
    }
}

impl fmt::Debug for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Hand {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Hand, ParseError> {
        match parse_cards(s)?.as_slice() {
            [a, b] => Hand::new(*a, *b).ok_or_else(|| perr("duplicate card")),
            _ => Err(perr(format!("a hand has two cards: {s:?}"))),
        }
    }
}

impl TryFrom<String> for Hand {
    type Error = ParseError;
    fn try_from(s: String) -> Result<Hand, ParseError> {
        s.parse()
    }
}

impl From<Hand> for String {
    fn from(h: Hand) -> String {
        h.to_string()
    }
}

/// Community cards: 0 (preflop), 3, 4 or 5 cards, in deal order.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Board {
    cards: Vec<Card>,
}

impl Board {
    pub fn new(cards: &[Card]) -> Result<Board, ParseError> {
        if !matches!(cards.len(), 0 | 3 | 4 | 5) {
            return Err(perr(format!(
                "a board has 0, 3, 4 or 5 cards, got {}",
                cards.len()
            )));
        }
        if CardSet::from_cards(cards).len() != cards.len() {
            return Err(perr("duplicate card on board"));
        }
        Ok(Board {
            cards: cards.to_vec(),
        })
    }
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }
    pub fn len(&self) -> usize {
        self.cards.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
    pub fn mask(&self) -> CardSet {
        CardSet::from_cards(&self.cards)
    }
}

impl FromStr for Board {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Board, ParseError> {
        Board::new(&parse_cards(s)?)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cards.iter().try_for_each(|c| write!(f, "{c}"))
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Board({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_roundtrip() {
        for c in Card::all() {
            assert_eq!(c.to_string().parse::<Card>().unwrap(), c);
        }
        assert_eq!("As".parse::<Card>().unwrap().index(), 51);
        assert_eq!("2c".parse::<Card>().unwrap().index(), 0);
        assert!("1s".parse::<Card>().is_err());
    }

    #[test]
    fn combo_index_is_a_bijection() {
        let mut seen = vec![false; NUM_COMBOS];
        for (i, h) in Hand::all().enumerate() {
            assert_eq!(h.combo(), i);
            assert!(!seen[i]);
            seen[i] = true;
            assert_eq!(Hand::from_combo(i), Some(h));
        }
        assert_eq!(Hand::all().len(), NUM_COMBOS);
    }

    #[test]
    fn hand_is_normalized() {
        let a: Hand = "2cAs".parse().unwrap();
        let b: Hand = "As2c".parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.high().to_string(), "As");
        assert!("AsAs".parse::<Hand>().is_err());
    }

    #[test]
    fn board_sizes() {
        assert!("AsKd".parse::<Board>().is_err());
        assert!("AsKd7h".parse::<Board>().is_ok());
        assert!("AsKdAs".parse::<Board>().is_err());
    }
}

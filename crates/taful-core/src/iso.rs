//! Suit isomorphism and flop textures (spec §7.2, §7.4, D13).
//!
//! Strategies are invariant under relabeling suits when both ranges are suit
//! symmetric (preflop ranges always are). So the 22,100 real flops collapse
//! to 1,755 canonical ones, and a library solve of one canonical flop serves
//! every real flop in its orbit through a [`SuitPerm`].

use std::fmt;

use crate::cards::{Card, Hand, Rank, Suit};

/// Number of distinct 3-card flops.
pub const NUM_FLOPS: usize = 22_100;
/// Number of flops up to suit relabeling.
pub const NUM_CANONICAL_FLOPS: usize = 1_755;

/// A relabeling of the four suits: `map[s]` is the image of suit `s`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SuitPerm([u8; 4]);

impl SuitPerm {
    pub const IDENTITY: SuitPerm = SuitPerm([0, 1, 2, 3]);

    /// `None` unless `map` is a permutation of `0..4`.
    pub fn new(map: [u8; 4]) -> Option<SuitPerm> {
        let mut seen = [false; 4];
        for &s in &map {
            if s > 3 || std::mem::replace(&mut seen[usize::from(s)], true) {
                return None;
            }
        }
        Some(SuitPerm(map))
    }

    /// The transposition of two suits (identity if equal).
    pub fn swap(a: Suit, b: Suit) -> SuitPerm {
        let mut m = [0, 1, 2, 3];
        m.swap(usize::from(a.index()), usize::from(b.index()));
        SuitPerm(m)
    }

    /// All 24 permutations, identity first.
    pub fn all() -> impl Iterator<Item = SuitPerm> {
        let mut out = Vec::with_capacity(24);
        for a in 0..4u8 {
            for b in 0..4u8 {
                for c in 0..4u8 {
                    for d in 0..4u8 {
                        if let Some(p) = SuitPerm::new([a, b, c, d]) {
                            out.push(p);
                        }
                    }
                }
            }
        }
        out.into_iter()
    }

    pub fn map(&self) -> [u8; 4] {
        self.0
    }

    pub fn suit(&self, s: Suit) -> Suit {
        Suit::new(self.0[usize::from(s.index())]).expect("perm is valid")
    }

    pub fn card(&self, c: Card) -> Card {
        Card::new(c.rank(), self.suit(c.suit()))
    }

    pub fn hand(&self, h: Hand) -> Hand {
        let [a, b] = h.cards();
        Hand::new(self.card(a), self.card(b)).expect("a permutation keeps cards distinct")
    }

    pub fn cards(&self, cs: &[Card]) -> Vec<Card> {
        cs.iter().map(|&c| self.card(c)).collect()
    }

    pub fn inverse(&self) -> SuitPerm {
        let mut m = [0u8; 4];
        for (s, &t) in self.0.iter().enumerate() {
            m[usize::from(t)] = s as u8;
        }
        SuitPerm(m)
    }

    /// `self ∘ first`: apply `first`, then `self`.
    pub fn after(&self, first: &SuitPerm) -> SuitPerm {
        let mut m = [0u8; 4];
        for (s, x) in m.iter_mut().enumerate() {
            *x = self.0[usize::from(first.0[s])];
        }
        SuitPerm(m)
    }

    /// Image of every combo index: `table[combo] = self.hand(combo).combo()`.
    pub fn combo_table(&self) -> Vec<u16> {
        Hand::all().map(|h| self.hand(h).combo() as u16).collect()
    }
}

impl fmt::Debug for SuitPerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = Suit::all()
            .map(|x| self.suit(x).to_char())
            .collect::<String>();
        write!(f, "SuitPerm(cdhs→{s})")
    }
}

/// A flop as three cards sorted high to low (by card index).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Flop([Card; 3]);

impl Flop {
    /// `None` if two cards are equal.
    pub fn new(cards: [Card; 3]) -> Option<Flop> {
        let mut c = cards;
        c.sort_by(|a, b| b.cmp(a));
        if c[0] == c[1] || c[1] == c[2] {
            return None;
        }
        Some(Flop(c))
    }

    pub fn cards(&self) -> [Card; 3] {
        self.0
    }

    pub fn permuted(&self, p: &SuitPerm) -> Flop {
        Flop::new(self.0.map(|c| p.card(c))).expect("distinct")
    }

    /// The canonical member of this flop's orbit and the permutation that
    /// maps this flop onto it (`perm` applied to `self` = canonical).
    ///
    /// Canonical = the lexicographically smallest sorted triple of card
    /// indices over all 24 suit relabelings, so it is unique per orbit.
    pub fn canonical(&self) -> (Flop, SuitPerm) {
        SuitPerm::all()
            .map(|p| (self.permuted(&p), p))
            .min_by_key(|(f, _)| f.0.map(Card::index))
            .expect("24 perms")
    }

    pub fn is_canonical(&self) -> bool {
        self.canonical().0 == *self
    }

    /// Permutations that map this flop onto itself (always contains the
    /// identity). Turn cards related by one of these are isomorphic.
    pub fn stabilizer(&self) -> Vec<SuitPerm> {
        SuitPerm::all()
            .filter(|p| self.permuted(p) == *self)
            .collect()
    }

    /// Number of real flops in this flop's orbit (4, 12 or 24).
    pub fn orbit_size(&self) -> usize {
        24 / self.stabilizer().len()
    }

    pub fn texture(&self) -> Texture {
        Texture::of(self)
    }

    /// All 22,100 flops.
    pub fn all() -> impl Iterator<Item = Flop> {
        (2..52u8).flat_map(|a| {
            (1..a).flat_map(move |b| {
                (0..b).map(move |c| {
                    Flop([
                        Card::from_index(a).expect("<52"),
                        Card::from_index(b).expect("<52"),
                        Card::from_index(c).expect("<52"),
                    ])
                })
            })
        })
    }

    /// The 1,755 canonical flops with their orbit sizes (summing to 22,100),
    /// in ascending card-index order.
    pub fn canonical_all() -> Vec<(Flop, usize)> {
        let mut out: Vec<(Flop, usize)> = Flop::all()
            .filter(Flop::is_canonical)
            .map(|f| (f, f.orbit_size()))
            .collect();
        out.sort_by_key(|(f, _)| f.0.map(Card::index));
        out
    }
}

impl fmt::Display for Flop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Ranks high to low, then suits, e.g. `Td9d6h`.
        let mut c = self.0;
        c.sort_by(|a, b| b.rank().cmp(&a.rank()).then(b.suit().cmp(&a.suit())));
        for x in c {
            write!(f, "{x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Flop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flop({self})")
    }
}

impl std::str::FromStr for Flop {
    type Err = crate::cards::ParseError;
    fn from_str(s: &str) -> Result<Flop, Self::Err> {
        let cards = crate::cards::parse_cards(s)?;
        let arr: [Card; 3] = cards
            .try_into()
            .map_err(|_| crate::cards::ParseError(format!("a flop has 3 cards: {s:?}")))?;
        Flop::new(arr).ok_or_else(|| crate::cards::ParseError(format!("repeated card in {s:?}")))
    }
}

/// Suit pattern of a flop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Suitedness {
    Rainbow,
    TwoTone,
    Monotone,
}

/// Rank pattern of a flop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Pairedness {
    Unpaired,
    Paired,
    Trips,
}

/// Texture features used for stratified sampling and board families
/// (spec §7.2, §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Texture {
    pub high: Rank,
    pub suits: Suitedness,
    pub pairs: Pairedness,
    /// Distinct straights (5 consecutive ranks, wheel included) that use at
    /// least two of the flop's distinct ranks: 0 = disconnected, higher =
    /// more connected.
    pub straights: u8,
}

/// Coarse texture class: the stratum for sampling and the board family for
/// spot templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureClass {
    /// 0 = ace high, 1 = king, 2 = queen, 3 = jack or ten, 4 = nine or lower.
    pub high: u8,
    pub suits: Suitedness,
    /// Paired or trips.
    pub paired: bool,
    /// 0 = disconnected (no straight uses two flop ranks), 1 = some, 2 = many
    /// (≥ 3 straights).
    pub connected: u8,
}

impl Texture {
    pub fn of(flop: &Flop) -> Texture {
        let cards = flop.cards();
        let high = cards.iter().map(|c| c.rank()).max().expect("3 cards");
        let mut suit_counts = [0u8; 4];
        for c in &cards {
            suit_counts[usize::from(c.suit().index())] += 1;
        }
        let max_suit = *suit_counts.iter().max().expect("4");
        let suits = match max_suit {
            3 => Suitedness::Monotone,
            2 => Suitedness::TwoTone,
            _ => Suitedness::Rainbow,
        };
        let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank().index()).collect();
        ranks.sort_unstable();
        ranks.dedup();
        let pairs = match ranks.len() {
            3 => Pairedness::Unpaired,
            2 => Pairedness::Paired,
            _ => Pairedness::Trips,
        };
        // The ten straights: the wheel (A-5) and 2-6 .. T-A.
        let has = |r: u8| ranks.contains(&r);
        let wheel = [12u8, 0, 1, 2, 3];
        let straights = std::iter::once(wheel.to_vec())
            .chain((0..=8u8).map(|low| (low..low + 5).collect()))
            .filter(|w: &Vec<u8>| w.iter().filter(|&&r| has(r)).count() >= 2)
            .count() as u8;
        Texture {
            high,
            suits,
            pairs,
            straights,
        }
    }

    pub fn class(&self) -> TextureClass {
        let high = match self.high.index() {
            12 => 0,
            11 => 1,
            10 => 2,
            8 | 9 => 3,
            _ => 4,
        };
        TextureClass {
            high,
            suits: self.suits,
            paired: self.pairs != Pairedness::Unpaired,
            connected: match self.straights {
                0 => 0,
                1 | 2 => 1,
                _ => 2,
            },
        }
    }
}

impl TextureClass {
    /// Stable short id, e.g. `A-2t-u-1` (high, suits, paired, connected).
    pub fn id(&self) -> String {
        let high = ["A", "K", "Q", "JT", "9-"][usize::from(self.high)];
        let suits = match self.suits {
            Suitedness::Rainbow => "r",
            Suitedness::TwoTone => "2t",
            Suitedness::Monotone => "m",
        };
        let paired = if self.paired { "p" } else { "u" };
        format!("{high}-{suits}-{paired}-{}", self.connected)
    }
}

/// Sampling stratum (spec §7.2): high card, suits and pairedness.
/// Connectedness is spread within a stratum by systematic sampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Stratum {
    pub high: u8,
    pub suits: Suitedness,
    pub paired: bool,
}

impl TextureClass {
    pub fn stratum(&self) -> Stratum {
        Stratum {
            high: self.high,
            suits: self.suits,
            paired: self.paired,
        }
    }
}

/// How often to deal each solved flop so textures appear as often as on
/// random boards (spec §7.2, D13): each stratum gets its real-flop share,
/// split among its solved flops by orbit size. Weights sum to 1 over the
/// strata present; a stratum with no solved flop is simply not dealt.
/// `flops` must be canonical.
pub fn deal_weights(flops: &[Flop]) -> Vec<f64> {
    use std::collections::HashMap;
    let mut real: HashMap<Stratum, f64> = HashMap::new();
    for (f, w) in Flop::canonical_all() {
        *real.entry(f.texture().class().stratum()).or_default() += w as f64;
    }
    let mut solved: HashMap<Stratum, f64> = HashMap::new();
    for f in flops {
        *solved.entry(f.texture().class().stratum()).or_default() += f.orbit_size() as f64;
    }
    let covered: f64 = solved.keys().map(|s| real[s]).sum();
    flops
        .iter()
        .map(|f| {
            let s = f.texture().class().stratum();
            real[&s] / covered * f.orbit_size() as f64 / solved[&s]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn counts() {
        assert_eq!(Flop::all().count(), NUM_FLOPS);
        let canon = Flop::canonical_all();
        assert_eq!(canon.len(), NUM_CANONICAL_FLOPS);
        assert_eq!(canon.iter().map(|(_, w)| w).sum::<usize>(), NUM_FLOPS);
        // Every real flop canonicalizes into the list, and the perm maps it there.
        let set: HashMap<Flop, usize> = canon.iter().copied().collect();
        for f in Flop::all().step_by(37) {
            let (c, p) = f.canonical();
            assert!(set.contains_key(&c));
            assert_eq!(f.permuted(&p), c);
            assert_eq!(c.permuted(&p.inverse()), f);
        }
    }

    #[test]
    fn perms() {
        assert_eq!(SuitPerm::all().count(), 24);
        for p in SuitPerm::all() {
            assert_eq!(p.after(&p.inverse()), SuitPerm::IDENTITY);
            let t = p.combo_table();
            let mut s = t.clone();
            s.sort_unstable();
            s.dedup();
            assert_eq!(s.len(), 1326);
        }
        let q = SuitPerm::swap(Suit::CLUBS, Suit::SPADES);
        let p = SuitPerm::swap(Suit::CLUBS, Suit::HEARTS);
        let c: Card = "Ac".parse().unwrap();
        assert_eq!(q.after(&p).card(c), q.card(p.card(c)));
    }

    #[test]
    fn textures() {
        let t = |s: &str| s.parse::<Flop>().unwrap().texture();
        let a = t("AsKd2c");
        assert_eq!(a.suits, Suitedness::Rainbow);
        assert_eq!(a.pairs, Pairedness::Unpaired);
        assert_eq!(a.class().high, 0);
        assert_eq!(t("Td9d6h").suits, Suitedness::TwoTone);
        assert!(t("Td9d6h").straights >= 3);
        assert_eq!(t("Kh7h2h").suits, Suitedness::Monotone);
        assert_eq!(t("7h7d2c").pairs, Pairedness::Paired);
        assert_eq!(t("7h7d7c").pairs, Pairedness::Trips);
        assert_eq!(t("Ks7d2c").straights, 0);
        assert_eq!(t("Ah2d3c").class().connected, 1);
        assert_eq!(
            "Td9d6h".parse::<Flop>().unwrap().to_string(),
            "Td9d6h",
            "display sorts by rank"
        );
        assert_eq!(t("AsKd2c").class().id(), "A-r-u-1");
    }

    #[test]
    fn deal_weights_follow_real_frequencies() {
        let all: Vec<Flop> = Flop::canonical_all().into_iter().map(|(f, _)| f).collect();
        let w = deal_weights(&all);
        // With every flop solved, the weight is the plain orbit share.
        for (f, x) in all.iter().zip(&w).step_by(53) {
            assert!((x - f.orbit_size() as f64 / NUM_FLOPS as f64).abs() < 1e-12);
        }
        let some: Vec<Flop> = ["AsKd2c", "Td9d6h", "Kh7h2h", "7h7d2c"]
            .iter()
            .map(|s| s.parse::<Flop>().unwrap().canonical().0)
            .collect();
        let w = deal_weights(&some);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn orbit_sizes() {
        let o = |s: &str| s.parse::<Flop>().unwrap().orbit_size();
        assert_eq!(o("AsKd2c"), 24);
        assert_eq!(o("AsKs2c"), 12);
        assert_eq!(o("AsKs2s"), 4);
        assert_eq!(o("AsAd2c"), 12);
        assert_eq!(o("AsAd2d"), 12);
        assert_eq!(o("AsAdAc"), 4);
    }
}

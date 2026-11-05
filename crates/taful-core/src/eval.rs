//! Hand evaluation (spec §5.1, spike §9.3).
//!
//! Callers depend on the [`HandEvaluator`] trait; [`RsPokerEvaluator`] is the
//! production implementation. A naive best-of-21 evaluator lives in the tests
//! and cross-checks it.

use crate::cards::Card;

/// Hand category, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::HighCard,
        Category::OnePair,
        Category::TwoPair,
        Category::ThreeOfAKind,
        Category::Straight,
        Category::Flush,
        Category::FullHouse,
        Category::FourOfAKind,
        Category::StraightFlush,
    ];
}

/// Strength of a best five-card hand. Higher is better; equal means a split.
/// Only comparable between ranks produced by the same evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandRank {
    pub category: Category,
    /// Evaluator-specific total order within and across categories.
    pub score: u32,
}

/// A seven-card hand evaluator.
pub trait HandEvaluator: Send + Sync {
    fn rank7(&self, cards: &[Card; 7]) -> HandRank;

    /// Rank the best five of 5 to 7 cards.
    fn rank(&self, cards: &[Card]) -> HandRank;
}

/// `rs_poker` 5.x perfect-hash evaluator (Apache-2.0).
#[derive(Debug, Clone, Copy, Default)]
pub struct RsPokerEvaluator;

fn to_rs(c: Card) -> rs_poker::core::Card {
    // rs_poker: id = suit * 13 + value, suits s, c, h, d.
    let suit = match c.suit().index() {
        0 => 1, // clubs
        1 => 3, // diamonds
        2 => 2, // hearts
        _ => 0, // spades
    };
    rs_poker::core::Card::from(suit * 13 + c.rank().index())
}

fn from_rs(r: rs_poker::core::Rank) -> HandRank {
    use rs_poker::core::CoreRank as R;
    let category = match r.category() {
        R::HighCard => Category::HighCard,
        R::OnePair => Category::OnePair,
        R::TwoPair => Category::TwoPair,
        R::ThreeOfAKind => Category::ThreeOfAKind,
        R::Straight => Category::Straight,
        R::Flush => Category::Flush,
        R::FullHouse => Category::FullHouse,
        R::FourOfAKind => Category::FourOfAKind,
        R::StraightFlush => Category::StraightFlush,
    };
    HandRank {
        category,
        score: u32::from(r.to_raw()),
    }
}

impl HandEvaluator for RsPokerEvaluator {
    #[inline]
    fn rank7(&self, cards: &[Card; 7]) -> HandRank {
        self.rank(cards)
    }

    #[inline]
    fn rank(&self, cards: &[Card]) -> HandRank {
        debug_assert!((5..=7).contains(&cards.len()));
        let mut acc = rs_poker::core::SevenCardAccum::new();
        for &c in cards {
            acc.add(to_rs(c));
        }
        from_rs(acc.rank())
    }
}

/// Incremental evaluation for loops that share cards (board, then hole cards).
#[derive(Clone, Copy)]
pub struct Accum(rs_poker::core::SevenCardAccum);

impl Accum {
    pub fn new(cards: &[Card]) -> Accum {
        let mut acc = rs_poker::core::SevenCardAccum::new();
        for &c in cards {
            acc.add(to_rs(c));
        }
        Accum(acc)
    }
    #[inline]
    pub fn with(self, c: Card) -> Accum {
        let mut a = self.0;
        a.add(to_rs(c));
        Accum(a)
    }
    /// Score comparable with other `Accum` scores (and with [`RsPokerEvaluator`] `score`).
    #[inline]
    pub fn score(self) -> u32 {
        u32::from(self.0.rank().to_raw())
    }
}

#[cfg(test)]
pub(crate) mod naive {
    //! Naive best-of-21 evaluator, used only to cross-check the real one.
    use super::Category;
    use crate::cards::Card;

    /// Comparable key: (category, tie-break ranks).
    pub fn rank5(cards: [Card; 5]) -> (Category, [u8; 5]) {
        let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank().index()).collect();
        ranks.sort_unstable_by(|a, b| b.cmp(a));
        let flush = cards.iter().all(|c| c.suit() == cards[0].suit());
        let mut uniq = ranks.clone();
        uniq.dedup();
        let straight_high = if uniq.len() == 5 && ranks[0] - ranks[4] == 4 {
            Some(ranks[0])
        } else if uniq == [12, 3, 2, 1, 0] {
            Some(3)
        } else {
            None
        };
        // Group by count desc, then rank desc.
        let mut groups: Vec<(u8, u8)> = uniq
            .iter()
            .map(|&r| (ranks.iter().filter(|&&x| x == r).count() as u8, r))
            .collect();
        groups.sort_unstable_by(|a, b| b.cmp(a));
        let mut tb = [0u8; 5];
        let mut i = 0;
        for &(n, r) in &groups {
            for _ in 0..n {
                tb[i] = r;
                i += 1;
            }
        }
        let cat = match (
            straight_high,
            flush,
            groups[0].0,
            groups.get(1).map(|g| g.0),
        ) {
            (Some(h), true, _, _) => return (Category::StraightFlush, [h, 0, 0, 0, 0]),
            (_, _, 4, _) => Category::FourOfAKind,
            (_, _, 3, Some(2)) => Category::FullHouse,
            (_, true, _, _) => Category::Flush,
            (Some(h), false, _, _) => return (Category::Straight, [h, 0, 0, 0, 0]),
            (_, _, 3, _) => Category::ThreeOfAKind,
            (_, _, 2, Some(2)) => Category::TwoPair,
            (_, _, 2, _) => Category::OnePair,
            _ => Category::HighCard,
        };
        (cat, tb)
    }

    pub fn rank7(cards: &[Card; 7]) -> (Category, [u8; 5]) {
        let mut best = None;
        for skip_a in 0..7 {
            for skip_b in skip_a + 1..7 {
                let mut five = [cards[0]; 5];
                let mut k = 0;
                for (i, &c) in cards.iter().enumerate() {
                    if i != skip_a && i != skip_b {
                        five[k] = c;
                        k += 1;
                    }
                }
                let r = rank5(five);
                if best.is_none_or(|b| r > b) {
                    best = Some(r);
                }
            }
        }
        best.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::parse_cards;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    fn seven(s: &str) -> [Card; 7] {
        parse_cards(s).unwrap().try_into().unwrap()
    }

    #[test]
    fn known_hands() {
        let e = RsPokerEvaluator;
        assert_eq!(
            e.rank7(&seven("AsKsQsJsTs2c3d")).category,
            Category::StraightFlush
        );
        assert_eq!(
            e.rank7(&seven("As2s3s4s5d9cKh")).category,
            Category::Straight
        );
        assert_eq!(
            e.rank7(&seven("AsAhAdKcKd2c3d")).category,
            Category::FullHouse
        );
        assert_eq!(e.rank7(&seven("2c3c7c9cJcAdAh")).category, Category::Flush);
        let wheel = e.rank7(&seven("As2d3h4c5s9dJh"));
        let six_high = e.rank7(&seven("6s2d3h4c5s9dJh"));
        assert!(six_high > wheel);
        // Kicker plays: AK vs AQ on A-high board.
        let ak = e.rank7(&seven("AsKd8h7c2sAd4h"));
        let aq = e.rank7(&seven("AsQd8h7c2sAd4h"));
        assert!(ak > aq);
        // Board plays: split.
        let a = e.rank7(&seven("2c3dAsKsQsJsTs"));
        let b = e.rank7(&seven("4c5dAsKsQsJsTs"));
        assert_eq!(a, b);
    }

    #[test]
    fn cross_check_vs_naive_random() {
        let e = RsPokerEvaluator;
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut deck: Vec<Card> = Card::all().collect();
        let n = if cfg!(debug_assertions) {
            20_000
        } else {
            500_000
        };
        type Seen = ([Card; 7], HandRank, (Category, [u8; 5]));
        let mut prev: Option<Seen> = None;
        for _ in 0..n {
            deck.shuffle(&mut rng);
            let h: [Card; 7] = deck[..7].try_into().unwrap();
            let fast = e.rank7(&h);
            let slow = naive::rank7(&h);
            assert_eq!(fast.category, slow.0, "{h:?}");
            if let Some((ph, pf, ps)) = prev {
                assert_eq!(fast.cmp(&pf), slow.cmp(&ps), "{h:?} vs {ph:?}");
            }
            // Mostly compare against a near neighbour so ties/kickers get exercised.
            if rng.random_bool(0.5) {
                prev = Some((h, fast, slow));
            }
        }
    }

    /// Exhaustive 7-card category counts (133,784,560 hands). Slow in debug.
    #[test]
    #[cfg_attr(debug_assertions, ignore = "exhaustive; run with --release")]
    #[allow(clippy::needless_range_loop)]
    fn exhaustive_category_counts() {
        const KNOWN: [u64; 9] = [
            23_294_460, 58_627_800, 31_433_400, 6_461_620, 6_180_020, 4_047_644, 3_473_184,
            224_848, 41_584,
        ];
        let mut counts = [0u64; 9];
        let cards: Vec<Card> = Card::all().collect();
        for a in 0..52 {
            let aa = Accum::new(&[cards[a]]);
            for b in a + 1..52 {
                let ab = aa.with(cards[b]);
                for c in b + 1..52 {
                    let ac = ab.with(cards[c]);
                    for d in c + 1..52 {
                        let ad = ac.with(cards[d]);
                        for e in d + 1..52 {
                            let ae = ad.with(cards[e]);
                            for f in e + 1..52 {
                                let af = ae.with(cards[f]);
                                for g in f + 1..52 {
                                    let r = af.with(cards[g]).0.rank();
                                    counts[from_rs(r).category as usize] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(counts, KNOWN);
    }
}

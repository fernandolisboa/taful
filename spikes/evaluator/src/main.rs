//! Spike §9.3: compare Rust hand evaluators for correctness and speed, and
//! benchmark exhaustive and Monte Carlo equity on top of the fastest one.

use std::time::Instant;

const RANKS: &[u8] = b"23456789TJQKA";
const SUITS: &[u8] = b"cdhs";

fn card_str(i: usize) -> String {
    format!("{}{}", RANKS[i / 4] as char, SUITS[i % 4] as char)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_hands(n: usize, seed: u64) -> Vec<[usize; 7]> {
    let mut rng = Rng(seed);
    (0..n)
        .map(|_| {
            let mut deck: [usize; 52] = std::array::from_fn(|i| i);
            for k in 0..7 {
                let j = k + rng.below(52 - k);
                deck.swap(k, j);
            }
            std::array::from_fn(|k| deck[k])
        })
        .collect()
}

// --- adapters: index 0..52 -> each crate's card type, and a 7-card rank function ---

mod aya {
    use aya_poker::base::{Card, Hand};
    pub fn cards() -> Vec<Card> {
        (0..52)
            .map(|i| super::card_str(i).parse().unwrap())
            .collect()
    }
    #[inline]
    pub fn rank(cards: &[Card], idx: &[usize]) -> u16 {
        let mut h = Hand::new();
        for &i in idx {
            h.insert_unchecked(&cards[i]);
        }
        aya_poker::poker_rank(&h).0
    }
    pub fn category(r: u16) -> usize {
        use aya_poker::PokerRankCategory::*;
        match aya_poker::PokerHandRank(r).rank_category() {
            HighCard => 0,
            Pair => 1,
            TwoPair => 2,
            ThreeOfAKind => 3,
            Straight => 4,
            Flush => 5,
            FullHouse => 6,
            FourOfAKind => 7,
            StraightFlush | RoyalFlush => 8,
            Ineligible => unreachable!(),
        }
    }
}

mod rsp {
    use rs_poker::core::{Card, Rank, SevenCardAccum, Suit, Value};
    pub fn cards() -> Vec<Card> {
        (0..52)
            .map(|i| {
                let s = super::card_str(i);
                let mut ch = s.chars();
                Card::new(
                    Value::from_char(ch.next().unwrap()).unwrap(),
                    Suit::from_char(ch.next().unwrap()).unwrap(),
                )
            })
            .collect()
    }
    #[inline]
    pub fn rank(cards: &[Card], idx: &[usize]) -> Rank {
        let mut acc = SevenCardAccum::new();
        for &i in idx {
            acc.add(cards[i]);
        }
        acc.rank()
    }
    pub fn category(r: Rank) -> usize {
        r.category() as usize
    }
}

mod pk {
    use poker::{Card, Evaluator};
    pub fn cards() -> Vec<Card> {
        (0..52)
            .map(|i| super::card_str(i).parse().unwrap())
            .collect()
    }
    #[inline]
    pub fn rank(ev: &Evaluator, cards: &[Card], idx: &[usize]) -> poker::Eval<poker::FiveCard> {
        let hand: [Card; 7] = std::array::from_fn(|k| cards[idx[k]]);
        ev.evaluate_five(hand).unwrap()
    }
}

// Known 7-card category counts over all C(52,7) = 133,784,560 hands.
const KNOWN: [u64; 9] = [
    23_294_460, 58_627_800, 31_433_400, 6_461_620, 6_180_020, 4_047_644, 3_473_184, 224_848, 41_584,
];

fn exhaustive_counts(rank_cat: impl Fn(&[usize]) -> usize) -> [u64; 9] {
    let mut c = [0u64; 9];
    let mut idx = [0usize; 7];
    for a in 0..46 {
        idx[0] = a;
        for b in a + 1..47 {
            idx[1] = b;
            for cc in b + 1..48 {
                idx[2] = cc;
                for d in cc + 1..49 {
                    idx[3] = d;
                    for e in d + 1..50 {
                        idx[4] = e;
                        for f in e + 1..51 {
                            idx[5] = f;
                            for g in f + 1..52 {
                                idx[6] = g;
                                c[rank_cat(&idx)] += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    c
}

fn throughput<R>(label: &str, hands: &[[usize; 7]], f: impl Fn(&[usize]) -> R) -> f64 {
    let t = Instant::now();
    for hand in hands {
        std::hint::black_box(f(std::hint::black_box(hand)));
    }
    let mps = hands.len() as f64 / t.elapsed().as_secs_f64() / 1e6;
    println!("  {label:<10} {mps:>8.1} M hands/s");
    mps
}

/// Exhaustive heads-up equity of hand A vs hand B over every 5-card board.
fn hu_equity<R: Ord>(a: [usize; 2], b: [usize; 2], rank: &impl Fn(&[usize]) -> R) -> (f64, u64) {
    let dead = |c: usize| a.contains(&c) || b.contains(&c);
    let live: Vec<usize> = (0..52).filter(|&c| !dead(c)).collect();
    let (mut win, mut tie, mut n) = (0u64, 0u64, 0u64);
    let mut ha = [a[0], a[1], 0, 0, 0, 0, 0];
    let mut hb = [b[0], b[1], 0, 0, 0, 0, 0];
    let l = live.len();
    for i in 0..l {
        for j in i + 1..l {
            for k in j + 1..l {
                for m in k + 1..l {
                    for o in m + 1..l {
                        let board = [live[i], live[j], live[k], live[m], live[o]];
                        ha[2..].copy_from_slice(&board);
                        hb[2..].copy_from_slice(&board);
                        let (ra, rb) = (rank(&ha), rank(&hb));
                        win += (ra > rb) as u64;
                        tie += (ra == rb) as u64;
                        n += 1;
                    }
                }
            }
        }
    }
    ((win as f64 + tie as f64 / 2.0) / n as f64, n)
}

fn idx_of(s: &str) -> usize {
    (0..52).find(|&i| card_str(i) == s).unwrap()
}

fn main() {
    let aya_cards = aya::cards();
    let rsp_cards = rsp::cards();
    let pk_cards = pk::cards();
    let pk_eval = poker::Evaluator::new();

    // 1. Cross-check: all three crates must order random hand pairs identically.
    let hands = random_hands(2_000_000, 0x9E3779B97F4A7C15);
    let mut disagreements = 0;
    for pair in hands.chunks_exact(2) {
        let a = aya::rank(&aya_cards, &pair[0]).cmp(&aya::rank(&aya_cards, &pair[1]));
        let r = rsp::rank(&rsp_cards, &pair[0]).cmp(&rsp::rank(&rsp_cards, &pair[1]));
        let p =
            pk::rank(&pk_eval, &pk_cards, &pair[0]).cmp(&pk::rank(&pk_eval, &pk_cards, &pair[1]));
        disagreements += (a != r || a != p) as usize;
    }
    println!("1. pairwise ordering, 1M random 7-card pairs: {disagreements} disagreements");

    // 2. Throughput, single thread, pre-generated random hands.
    println!("2. 7-card evaluation throughput (single thread, 2M random hands):");
    throughput("aya_poker", &hands, |h| aya::rank(&aya_cards, h));
    throughput("rs_poker", &hands, |h| rsp::rank(&rsp_cards, h));
    throughput("poker", &hands, |h| pk::rank(&pk_eval, &pk_cards, h));

    // 3. Exhaustive category counts vs the known table (the two fast crates).
    for (label, counts, secs) in [
        {
            let t = Instant::now();
            let c = exhaustive_counts(|h| aya::category(aya::rank(&aya_cards, h)));
            ("aya_poker", c, t.elapsed().as_secs_f64())
        },
        {
            let t = Instant::now();
            let c = exhaustive_counts(|h| rsp::category(rsp::rank(&rsp_cards, h)));
            ("rs_poker", c, t.elapsed().as_secs_f64())
        },
    ] {
        println!(
            "3. {label}: all 133,784,560 7-card hands in {secs:.1}s, category counts match known table: {}",
            counts == KNOWN
        );
        if counts != KNOWN {
            println!("   got {counts:?}");
        }
    }

    // 4. Exhaustive heads-up preflop equity (1,712,304 boards each).
    let rsp_rank = |h: &[usize]| rsp::rank(&rsp_cards, h);
    for (a, b, reference) in [
        (["As", "Ah"], ["Ks", "Kh"], "AsAh vs KsKh"),
        (["Ah", "Kh"], ["Qs", "Qd"], "AhKh vs QsQd"),
        (["7s", "2d"], ["As", "Kd"], "7s2d vs AsKd"),
    ] {
        let t = Instant::now();
        let (eq, n) = hu_equity(
            [idx_of(a[0]), idx_of(a[1])],
            [idx_of(b[0]), idx_of(b[1])],
            &rsp_rank,
        );
        println!(
            "4. {reference}: equity {:.3}% over {n} boards in {:.0}ms (rs_poker, 1 thread)",
            100.0 * eq,
            t.elapsed().as_secs_f64() * 1e3
        );
    }

    // 5. Monte Carlo hand vs random hand, 10M samples.
    let mut rng = Rng(42);
    let hero = [idx_of("Ah"), idx_of("Kh")];
    let samples = 10_000_000;
    let t = Instant::now();
    let mut score = 0f64;
    for _ in 0..samples {
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        deck.swap(hero[0], 51);
        deck.swap(if hero[1] == 51 { hero[0] } else { hero[1] }, 50);
        for k in 0..7 {
            let j = k + rng.below(50 - k);
            deck.swap(k, j);
        }
        let h = [
            hero[0], hero[1], deck[2], deck[3], deck[4], deck[5], deck[6],
        ];
        let v = [
            deck[0], deck[1], deck[2], deck[3], deck[4], deck[5], deck[6],
        ];
        let (rh, rv) = (rsp_rank(&h), rsp_rank(&v));
        score += if rh > rv {
            1.0
        } else if rh == rv {
            0.5
        } else {
            0.0
        };
    }
    println!(
        "5. AhKh vs random hand, Monte Carlo {samples} samples: {:.2}% in {:.0}ms (rs_poker, 1 thread)",
        100.0 * score / samples as f64,
        t.elapsed().as_secs_f64() * 1e3
    );
}

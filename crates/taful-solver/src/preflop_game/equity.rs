//! 169×169 preflop class tables: card-removal counts and all-in equity.
//!
//! By suit symmetry every combo of a class sees the same thing: the number of
//! compatible combos of another class, and the mean equity against them.
//! So one representative combo per class is exact for the counts, and the
//! equities only carry Monte Carlo error from the board samples.

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use taful_core::cards::{Card, Hand};
use taful_core::eval::Accum;
use taful_core::range::HandClass;

pub const N: usize = HandClass::COUNT;

/// Class-level tables, row-major `[a * N + b]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassTables {
    /// Combos of class `b` that share no card with a given combo of `a`.
    pub compat: Vec<f32>,
    /// Mean all-in equity (win + tie/2) of a combo of `a` against the
    /// compatible combos of `b`. `eq[a][b] + eq[b][a] = 1`.
    pub equity: Vec<f32>,
    pub samples: u32,
    pub seed: u64,
}

/// Combos per class.
pub fn class_sizes() -> [f32; N] {
    std::array::from_fn(|i| HandClass::from_index(i).expect("< 169").num_combos() as f32)
}

/// The card-removal counts (exact, cheap).
fn compat_table() -> Vec<f32> {
    let classes: Vec<HandClass> = HandClass::all().collect();
    let mut compat = vec![0.0f32; N * N];
    for (a, &ca) in classes.iter().enumerate() {
        let h = representative(ca);
        for (b, &cb) in classes.iter().enumerate() {
            compat[a * N + b] = compat_count(h, cb) as f32;
        }
    }
    compat
}

fn representative(c: HandClass) -> Hand {
    c.combos()[0]
}

fn compat_count(h: Hand, b: HandClass) -> usize {
    b.combos()
        .iter()
        .filter(|o| !o.mask().intersects(h.mask()))
        .count()
}

/// Equity of `h` vs every compatible combo of `b`, `samples` boards each.
fn pair_equity(h: Hand, b: HandClass, samples: u32, rng: &mut ChaCha8Rng) -> f64 {
    let mut total = 0.0f64;
    let mut n = 0u64;
    let mut deck: Vec<Card> = Vec::with_capacity(48);
    for o in b.combos() {
        if o.mask().intersects(h.mask()) {
            continue;
        }
        let dead = h.mask().union(o.mask());
        deck.clear();
        deck.extend(Card::all().filter(|c| !dead.contains(*c)));
        for _ in 0..samples {
            // Partial Fisher–Yates: the first five cards are the board.
            for i in 0..5 {
                let j = rng.random_range(i..deck.len());
                deck.swap(i, j);
            }
            let board = Accum::new(&deck[..5]);
            let [h1, h2] = h.cards();
            let [o1, o2] = o.cards();
            let a = board.with(h1).with(h2).score();
            let c = board.with(o1).with(o2).score();
            total += match a.cmp(&c) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Equal => 0.5,
                std::cmp::Ordering::Less => 0.0,
            };
            n += 1;
        }
    }
    if n == 0 { 0.5 } else { total / n as f64 }
}

impl ClassTables {
    /// Compute both tables. Deterministic for a given `(samples, seed)`
    /// whatever the thread count: each class pair has its own RNG stream.
    pub fn compute(samples: u32, seed: u64) -> ClassTables {
        let classes: Vec<HandClass> = HandClass::all().collect();
        let compat = compat_table();
        let pairs: Vec<(usize, usize)> = (0..N)
            .flat_map(|a| ((a + 1)..N).map(move |b| (a, b)))
            .collect();
        #[cfg(feature = "parallel")]
        let it = pairs.par_iter();
        #[cfg(not(feature = "parallel"))]
        let it = pairs.iter();
        let eqs: Vec<f64> = it
            .map(|&(a, b)| {
                let mut rng = ChaCha8Rng::seed_from_u64(
                    seed ^ ((a * N + b) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
                );
                pair_equity(representative(classes[a]), classes[b], samples, &mut rng)
            })
            .collect();
        let mut equity = vec![0.5f32; N * N];
        for (&(a, b), &e) in pairs.iter().zip(&eqs) {
            equity[a * N + b] = e as f32;
            equity[b * N + a] = (1.0 - e) as f32;
        }
        ClassTables {
            compat,
            equity,
            samples,
            seed,
        }
    }

    fn file_name(samples: u32, seed: u64) -> String {
        format!("equity169-s{samples}-{seed}.bin")
    }

    /// Load from `dir` or compute and save there (the table is the slowest
    /// input to build, and is identical across preflop solves).
    pub fn cached(dir: &std::path::Path, samples: u32, seed: u64) -> std::io::Result<ClassTables> {
        let path = dir.join(Self::file_name(samples, seed));
        if let Ok(bytes) = std::fs::read(&path)
            && let Some(t) = ClassTables::from_equity_bytes(&bytes, samples, seed)
        {
            return Ok(t);
        }
        let t = ClassTables::compute(samples, seed);
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, t.equity_bytes())?;
        Ok(t)
    }

    /// The equity table as little-endian `f32`s (`preflop-equity.bin`).
    pub fn equity_bytes(&self) -> Vec<u8> {
        self.equity.iter().flat_map(|e| e.to_le_bytes()).collect()
    }

    /// Tables from [`equity_bytes`](Self::equity_bytes) (the card-removal
    /// counts are exact and cheap, so they are recomputed); `None` if the
    /// bytes are not a 169×169 table.
    pub fn from_equity_bytes(bytes: &[u8], samples: u32, seed: u64) -> Option<ClassTables> {
        if bytes.len() != N * N * 4 {
            return None;
        }
        let equity: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        if equity.iter().any(|e| !(0.0..=1.0).contains(e)) {
            return None;
        }
        Some(ClassTables {
            compat: compat_table(),
            equity,
            samples,
            seed,
        })
    }

    pub fn eq(&self, a: usize, b: usize) -> f32 {
        self.equity[a * N + b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(s: &str) -> usize {
        HandClass::all().position(|c| c.to_string() == s).unwrap()
    }

    #[test]
    fn compat_counts() {
        let t = ClassTables::compute(1, 1);
        let c = |a, b| t.compat[idx(a) * N + idx(b)];
        assert_eq!(c("AA", "AA"), 1.0);
        assert_eq!(c("AA", "KK"), 6.0);
        assert_eq!(c("AA", "AKs"), 2.0);
        assert_eq!(c("AKs", "AKo"), 6.0);
        assert_eq!(c("AKo", "AKs"), 2.0);
        assert_eq!(c("72o", "AKo"), 12.0);
        // Every combo sees 1,225 others.
        let sizes = class_sizes();
        for a in 0..N {
            let total: f32 = (0..N).map(|b| t.compat[a * N + b]).sum();
            assert_eq!(total, 1225.0, "{}", HandClass::from_index(a).unwrap());
        }
        assert_eq!(sizes.iter().sum::<f32>(), 1326.0);
    }

    #[test]
    fn known_equities_within_mc_error() {
        // 6 combo pairs × 4000 boards: SE ≈ 0.25%.
        let t = ClassTables::compute(0, 7);
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let class = |s| HandClass::from_index(idx(s)).unwrap();
        let e = pair_equity(representative(class("AA")), class("KK"), 4000, &mut rng);
        assert!((e - 0.8236).abs() < 0.012, "AA vs KK {e}");
        let e = pair_equity(representative(class("AKo")), class("22"), 4000, &mut rng);
        assert!((e - 0.47).abs() < 0.015, "AKo vs 22 {e}");
        assert_eq!(t.eq(5, 5), 0.5);
    }

    #[test]
    fn symmetric_and_deterministic() {
        let a = ClassTables::compute(2, 9);
        let b = ClassTables::compute(2, 9);
        assert_eq!(a, b);
        for i in 0..N {
            for j in 0..N {
                assert!((a.eq(i, j) + a.eq(j, i) - 1.0).abs() < 1e-6);
            }
        }
    }
}

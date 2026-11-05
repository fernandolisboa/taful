//! Showdown equity (spec §5.1): exact enumeration and seeded Monte Carlo.
//!
//! Equity is the share of the pot won at showdown, ties split evenly, averaged
//! over every remaining runout. Card removal is exact: a pair of combos is
//! only counted when neither shares a card with the other or the board.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::cards::{Board, Card, CardSet, Hand, NUM_COMBOS};
use crate::eval::Accum;
use crate::range::Range;

/// Result of an equity calculation, from the first player's point of view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Equity {
    /// Pot share in `[0, 1]`: `win + tie / 2`.
    pub equity: f64,
    /// Probability of winning outright.
    pub win: f64,
    /// Probability of a split.
    pub tie: f64,
    /// Total weight enumerated (exact) or number of samples (Monte Carlo).
    pub weight: f64,
    /// Standard error of `equity`; `None` for exact results.
    pub std_error: Option<f64>,
}

/// Why an equity query cannot be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquityError {
    /// The hands/board share a card.
    CardConflict,
    /// No combo pair is possible (empty ranges or full card removal).
    EmptyRange,
    /// Exact range-vs-range needs a flop or later; use [`range_vs_range_mc`] preflop.
    NeedsBoard,
}

impl std::fmt::Display for EquityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EquityError::CardConflict => "hands and board share a card",
            EquityError::EmptyRange => "no valid combination of hands",
            EquityError::NeedsBoard => "exact range vs range needs a board; use Monte Carlo",
        })
    }
}

impl std::error::Error for EquityError {}

/// Call `f` with every `k`-card subset of `avail` (in index order).
pub(crate) fn for_each_subset(avail: &[Card], k: usize, mut f: impl FnMut(&[Card])) {
    fn rec(
        avail: &[Card],
        k: usize,
        start: usize,
        cur: &mut Vec<Card>,
        f: &mut impl FnMut(&[Card]),
    ) {
        if cur.len() == k {
            f(cur);
            return;
        }
        let need = k - cur.len();
        for i in start..=avail.len() - need {
            cur.push(avail[i]);
            rec(avail, k, i + 1, cur, f);
            cur.pop();
        }
    }
    if k > avail.len() {
        return;
    }
    rec(avail, k, 0, &mut Vec::with_capacity(k), &mut f);
}

fn finish(win: f64, tie: f64, total: f64) -> Result<Equity, EquityError> {
    if total <= 0.0 {
        return Err(EquityError::EmptyRange);
    }
    Ok(Equity {
        equity: (win + tie / 2.0) / total,
        win: win / total,
        tie: tie / total,
        weight: total,
        std_error: None,
    })
}

/// Exact equity of `a` vs `b`, enumerating every runout of `board`.
pub fn hand_vs_hand(a: Hand, b: Hand, board: &Board) -> Result<Equity, EquityError> {
    let dead = a.mask().union(b.mask()).union(board.mask());
    if a.mask().intersects(b.mask()) || dead.len() != 4 + board.len() {
        return Err(EquityError::CardConflict);
    }
    let avail: Vec<Card> = dead.complement().iter().collect();
    let base = Accum::new(board.cards());
    let (ha, hb) = (a.cards(), b.cards());
    let (mut win, mut tie, mut n) = (0u64, 0u64, 0u64);
    for_each_subset(&avail, 5 - board.len(), |run| {
        let acc = run.iter().fold(base, |acc, &c| acc.with(c));
        let sa = acc.with(ha[0]).with(ha[1]).score();
        let sb = acc.with(hb[0]).with(hb[1]).score();
        n += 1;
        match sa.cmp(&sb) {
            std::cmp::Ordering::Greater => win += 1,
            std::cmp::Ordering::Equal => tie += 1,
            std::cmp::Ordering::Less => {}
        }
    });
    finish(win as f64, tie as f64, n as f64)
}

/// Exact equity of `hero` vs a weighted `villain` range on a flop, turn or river.
pub fn hand_vs_range(hero: Hand, villain: &Range, board: &Board) -> Result<Equity, EquityError> {
    range_vs_range(&single(hero), villain, board)
}

fn single(h: Hand) -> Range {
    let mut r = Range::empty();
    r.set(h, 1.0);
    r
}

/// Exact range-vs-range equity on a known flop, turn or river.
///
/// Per runout, villain combos are sorted by strength and swept once with
/// per-card running sums, so each runout costs `O(n log n)` instead of `O(n²)`.
pub fn range_vs_range(hero: &Range, villain: &Range, board: &Board) -> Result<Equity, EquityError> {
    if board.is_empty() {
        return Err(EquityError::NeedsBoard);
    }
    let bmask = board.mask();
    let hero_live: Vec<(Hand, f64)> = hero.live(bmask).map(|(h, w)| (h, f64::from(w))).collect();
    let vil_live: Vec<(Hand, f64)> = villain
        .live(bmask)
        .map(|(h, w)| (h, f64::from(w)))
        .collect();
    if hero_live.is_empty() || vil_live.is_empty() {
        return Err(EquityError::EmptyRange);
    }
    let avail: Vec<Card> = bmask.complement().iter().collect();
    let base = Accum::new(board.cards());

    let (mut win, mut tie, mut total) = (0.0f64, 0.0f64, 0.0f64);
    let mut vil_w = vec![0.0f64; NUM_COMBOS];
    for &(h, w) in &vil_live {
        vil_w[h.combo()] = w;
    }
    let mut vs: Vec<(u32, f64, Card, Card)> = Vec::with_capacity(vil_live.len());
    let mut hs: Vec<(u32, f64, Hand)> = Vec::with_capacity(hero_live.len());

    for_each_subset(&avail, 5 - board.len(), |run| {
        let rmask = CardSet::from_cards(run);
        let acc = run.iter().fold(base, |acc, &c| acc.with(c));
        vs.clear();
        hs.clear();
        let mut card_total = [0.0f64; 52];
        let mut vtotal = 0.0;
        for &(h, w) in &vil_live {
            if h.mask().intersects(rmask) {
                continue;
            }
            let [a, b] = h.cards();
            vs.push((acc.with(a).with(b).score(), w, a, b));
            card_total[a.index() as usize] += w;
            card_total[b.index() as usize] += w;
            vtotal += w;
        }
        for &(h, w) in &hero_live {
            if h.mask().intersects(rmask) {
                continue;
            }
            let [a, b] = h.cards();
            hs.push((acc.with(a).with(b).score(), w, h));
        }
        vs.sort_unstable_by_key(|x| x.0);
        hs.sort_unstable_by_key(|x| x.0);

        // Running sums over villain combos strictly weaker (lt) and weaker-or-equal (le).
        let (mut lt, mut le) = (0.0f64, 0.0f64);
        let (mut card_lt, mut card_le) = ([0.0f64; 52], [0.0f64; 52]);
        let (mut i_lt, mut i_le) = (0usize, 0usize);
        for &(score, hw, h) in hs.iter() {
            while i_lt < vs.len() && vs[i_lt].0 < score {
                let (_, w, a, b) = vs[i_lt];
                lt += w;
                card_lt[a.index() as usize] += w;
                card_lt[b.index() as usize] += w;
                i_lt += 1;
            }
            while i_le < vs.len() && vs[i_le].0 <= score {
                let (_, w, a, b) = vs[i_le];
                le += w;
                card_le[a.index() as usize] += w;
                card_le[b.index() as usize] += w;
                i_le += 1;
            }
            let [a, b] = h.cards();
            let (ai, bi) = (a.index() as usize, b.index() as usize);
            // The identical combo shares both cards, so it is subtracted twice;
            // it ties with itself, so it sits in `le` and `total`, never in `lt`.
            let same = vil_w[h.combo()];
            let valid = vtotal - card_total[ai] - card_total[bi] + same;
            let w_lt = lt - card_lt[ai] - card_lt[bi];
            let w_le = le - card_le[ai] - card_le[bi] + same;
            win += hw * w_lt;
            tie += hw * (w_le - w_lt);
            total += hw * valid;
        }
    });
    // Float sums can leave tiny negative residues for fully blocked combos.
    finish(win.max(0.0), tie.max(0.0), total)
}

/// Seeded Monte Carlo range-vs-range equity, for preflop or any board.
///
/// Combo pairs are drawn with probability proportional to `w_hero × w_villain`
/// among non-conflicting pairs (rejection sampling), then the board is completed
/// uniformly. Returns the standard error of the estimate.
pub fn range_vs_range_mc(
    hero: &Range,
    villain: &Range,
    board: &Board,
    samples: u32,
    seed: u64,
) -> Result<Equity, EquityError> {
    let bmask = board.mask();
    let h_live: Vec<(Hand, f32)> = hero.live(bmask).collect();
    let v_live: Vec<(Hand, f32)> = villain.live(bmask).collect();
    if h_live.is_empty() || v_live.is_empty() || samples == 0 {
        return Err(EquityError::EmptyRange);
    }
    let cum = |v: &[(Hand, f32)]| -> Vec<f64> {
        let mut acc = 0.0;
        v.iter()
            .map(|&(_, w)| {
                acc += f64::from(w);
                acc
            })
            .collect()
    };
    let (h_cum, v_cum) = (cum(&h_live), cum(&v_live));
    let pick = |rng: &mut ChaCha8Rng, live: &[(Hand, f32)], c: &[f64]| -> Hand {
        let x = rng.random::<f64>() * c[c.len() - 1];
        let i = c.partition_point(|&s| s <= x).min(live.len() - 1);
        live[i].0
    };
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let base = Accum::new(board.cards());
    let need = 5 - board.len();
    let (mut win, mut tie, mut sum_sq) = (0u64, 0u64, 0.0f64);
    let mut drawn = 0u32;
    let mut rejections = 0u64;
    let mut deck: Vec<Card> = Vec::with_capacity(52);
    while drawn < samples {
        let h = pick(&mut rng, &h_live, &h_cum);
        let v = pick(&mut rng, &v_live, &v_cum);
        if h.mask().intersects(v.mask()) {
            rejections += 1;
            if rejections > 1_000 + 100 * u64::from(samples) {
                return Err(EquityError::EmptyRange);
            }
            continue;
        }
        let dead = bmask.union(h.mask()).union(v.mask());
        deck.clear();
        deck.extend(dead.complement().iter());
        let mut acc = base;
        for i in 0..need {
            let j = rng.random_range(i..deck.len());
            deck.swap(i, j);
            acc = acc.with(deck[i]);
        }
        let [h1, h2] = h.cards();
        let [v1, v2] = v.cards();
        let sh = acc.with(h1).with(h2).score();
        let sv = acc.with(v1).with(v2).score();
        let x = match sh.cmp(&sv) {
            std::cmp::Ordering::Greater => {
                win += 1;
                1.0
            }
            std::cmp::Ordering::Equal => {
                tie += 1;
                0.5
            }
            std::cmp::Ordering::Less => 0.0,
        };
        sum_sq += x * x;
        drawn += 1;
    }
    let n = f64::from(samples);
    let mut eq = finish(win as f64, tie as f64, n)?;
    let var = (sum_sq / n - eq.equity * eq.equity).max(0.0);
    eq.std_error = Some((var / n).sqrt());
    Ok(eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Hand {
        s.parse().unwrap()
    }
    fn b(s: &str) -> Board {
        s.parse().unwrap()
    }

    /// Pinned exact preflop equities (spike §9.3).
    #[test]
    #[cfg_attr(debug_assertions, ignore = "1.7M boards each; run with --release")]
    fn pinned_preflop_equities() {
        for (a, v, want) in [
            ("AsAh", "KsKh", 0.8264),
            ("AhKh", "QsQd", 0.4621),
            ("7s2d", "AsKd", 0.3187),
        ] {
            let e = hand_vs_hand(h(a), h(v), &Board::default()).unwrap();
            assert!((e.equity - want).abs() < 5e-5, "{a} vs {v}: {}", e.equity);
        }
    }

    #[test]
    fn river_and_turn_exact() {
        // Set over set on the river: 100%.
        let e = hand_vs_hand(h("9s9h"), h("2s2h"), &b("9d2dKc7h4s")).unwrap();
        assert_eq!(e.equity, 1.0);
        // Board plays: split.
        let e = hand_vs_hand(h("2c3c"), h("4d5d"), &b("AsKsQsJsTs")).unwrap();
        assert_eq!((e.equity, e.tie), (0.5, 1.0));
        // Nut flush draw + two overcards on the turn: 9 hearts + 3 aces + 3 kings = 15 of 44.
        let e = hand_vs_hand(h("AhKh"), h("QsQc"), &b("2h7h9cJd")).unwrap();
        assert!((e.equity - 15.0 / 44.0).abs() < 1e-12, "{}", e.equity);
        assert!(hand_vs_hand(h("AhKh"), h("AhQc"), &b("2h7h9cJd")).is_err());
    }

    #[test]
    fn range_vs_range_matches_pairwise_sum() {
        let board = b("Td9d6h");
        let hero: Range = "AA,KQs,JTs,87s:0.5".parse().unwrap();
        let vil: Range = "TT,99,AdKd,QJo:0.25,AKo".parse().unwrap();
        let fast = range_vs_range(&hero, &vil, &board).unwrap();
        // Brute force: sum over valid pairs of w_h * w_v * equity(h, v).
        let (mut num, mut den) = (0.0, 0.0);
        for (hh, wh) in hero.live(board.mask()) {
            for (vv, wv) in vil.live(board.mask()) {
                if hh.mask().intersects(vv.mask()) {
                    continue;
                }
                let e = hand_vs_hand(hh, vv, &board).unwrap();
                let w = f64::from(wh) * f64::from(wv);
                num += w * e.equity;
                den += w;
            }
        }
        assert!(
            (fast.equity - num / den).abs() < 1e-9,
            "{} vs {}",
            fast.equity,
            num / den
        );
        let one = hand_vs_range(h("AsAh"), &vil, &board).unwrap();
        assert!(one.equity > 0.5 && one.equity < 1.0);
    }

    #[test]
    fn monte_carlo_is_seeded_and_close() {
        let hero: Range = "AhKh".parse().unwrap();
        let full = Range::full();
        let a = range_vs_range_mc(&hero, &full, &Board::default(), 200_000, 1).unwrap();
        let again = range_vs_range_mc(&hero, &full, &Board::default(), 200_000, 1).unwrap();
        assert_eq!(a, again);
        let se = a.std_error.unwrap();
        assert!(se > 0.0 && se < 0.002);
        assert!((a.equity - 0.6703).abs() < 4.0 * se + 1e-3, "{}", a.equity);
        // MC agrees with exact postflop.
        let board = b("Td9d6h");
        let vil: Range = "TT,99,AdKd,QJo,AKo".parse().unwrap();
        let hr: Range = "AA,KQs,JTs".parse().unwrap();
        let exact = range_vs_range(&hr, &vil, &board).unwrap();
        let mc = range_vs_range_mc(&hr, &vil, &board, 200_000, 9).unwrap();
        assert!((exact.equity - mc.equity).abs() < 4.0 * mc.std_error.unwrap());
    }
}

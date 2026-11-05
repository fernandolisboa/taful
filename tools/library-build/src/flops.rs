//! Flop subset and growth order (spec §7.2, D13).
//!
//! The first `FIRST` flops are a weighted stratified sample of the 1,755
//! canonical flops: strata are (high card, suits, pairedness), each gets a
//! share of the sample proportional to its real-flop weight (at least one),
//! and within a stratum a systematic weighted sample over flops sorted by
//! connectedness spreads the picks across textures. The remaining flops
//! follow in order of real-flop weight (orbit size), ties broken round-robin
//! over strata, so any prefix of the order is a usable library.
//!
//! The order is written to `library/flops.txt` and is append-only in
//! practice: changing the seed or the algorithm reorders flops that may
//! already be solved, so a test pins the committed file.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use taful_core::cards::Card;
use taful_core::iso::{Flop, Stratum};

/// Size of the first slice (spec §7.1).
pub const FIRST: usize = 49;
pub const SEED: u64 = 20261105;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub flop: Flop,
    pub orbit: usize,
    pub stratum: Stratum,
    pub class: String,
}

fn entry(flop: Flop, orbit: usize) -> Entry {
    let class = flop.texture().class();
    Entry {
        flop,
        orbit,
        stratum: class.stratum(),
        class: class.id(),
    }
}

/// Largest-remainder allocation of `n` picks over `weights`, at least one
/// per non-empty stratum (and never more than its size).
fn allocate(weights: &[(f64, usize)], n: usize) -> Vec<usize> {
    let total: f64 = weights.iter().map(|w| w.0).sum();
    let mut alloc: Vec<usize> = weights.iter().map(|_| 1).collect();
    let rest = n.saturating_sub(weights.len());
    let quotas: Vec<f64> = weights.iter().map(|w| w.0 / total * rest as f64).collect();
    for (a, q) in alloc.iter_mut().zip(&quotas) {
        *a += q.floor() as usize;
    }
    let mut left = n.saturating_sub(alloc.iter().sum());
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        let ra = quotas[a] - quotas[a].floor();
        let rb = quotas[b] - quotas[b].floor();
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for &i in order.iter().cycle().take(weights.len() * 4) {
        if left == 0 {
            break;
        }
        if alloc[i] < weights[i].1 {
            alloc[i] += 1;
            left -= 1;
        }
    }
    for (a, w) in alloc.iter_mut().zip(weights) {
        *a = (*a).min(w.1);
    }
    alloc
}

/// The full order of all 1,755 canonical flops.
pub fn order(seed: u64) -> Vec<Entry> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut strata: BTreeMap<Stratum, Vec<Entry>> = BTreeMap::new();
    for (f, w) in Flop::canonical_all() {
        let e = entry(f, w);
        strata.entry(e.stratum).or_default().push(e);
    }
    // Within a stratum: by connectedness, then by ranks high to low, so a
    // systematic sample walks across textures.
    for v in strata.values_mut() {
        v.sort_by_key(|e| {
            let t = e.flop.texture();
            let ranks: Vec<u8> = e
                .flop
                .cards()
                .iter()
                .map(|c: &Card| 12 - c.rank().index())
                .collect();
            (t.straights, ranks, e.flop.cards().map(Card::index))
        });
    }
    let weights: Vec<(f64, usize)> = strata
        .values()
        .map(|v| (v.iter().map(|e| e.orbit as f64).sum(), v.len()))
        .collect();
    let alloc = allocate(&weights, FIRST);

    let mut first = Vec::with_capacity(FIRST);
    let mut rest: Vec<Vec<Entry>> = Vec::new();
    for ((members, (w, _)), &k) in strata.into_values().zip(&weights).zip(&alloc) {
        let step = w / k as f64;
        let start = rng.random::<f64>() * step;
        let mut taken = vec![false; members.len()];
        let mut cum = 0.0;
        let mut target = start;
        let mut picks = 0;
        for (i, e) in members.iter().enumerate() {
            cum += e.orbit as f64;
            while picks < k && cum > target {
                if !taken[i] {
                    taken[i] = true;
                    picks += 1;
                }
                target += step;
            }
        }
        // Collisions (a flop heavier than the step) are filled by the next free ones.
        for t in taken.iter_mut() {
            if picks >= k {
                break;
            }
            if !*t {
                *t = true;
                picks += 1;
            }
        }
        let mut left = Vec::new();
        for (e, t) in members.into_iter().zip(taken) {
            if t {
                first.push(e);
            } else {
                left.push(e);
            }
        }
        // Shuffle the rest of the stratum so growth is spread over textures.
        for i in (1..left.len()).rev() {
            let j = rng.random_range(0..=i);
            left.swap(i, j);
        }
        rest.push(left);
    }
    // First slice: heaviest strata first, so a partial first slice covers
    // the common textures.
    first.sort_by(|a, b| b.orbit.cmp(&a.orbit).then(a.stratum.cmp(&b.stratum)));

    let mut out = first;
    for weight in [24, 12, 4] {
        let mut queues: Vec<std::collections::VecDeque<Entry>> = rest
            .iter_mut()
            .map(|v| {
                let (take, keep): (Vec<Entry>, Vec<Entry>) = std::mem::take(v)
                    .into_iter()
                    .partition(|e| e.orbit == weight);
                *v = keep;
                take.into()
            })
            .collect();
        loop {
            let mut any = false;
            for q in queues.iter_mut() {
                if let Some(e) = q.pop_front() {
                    out.push(e);
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
    }
    out
}

/// `library/flops.txt`: one flop per line, in order.
pub fn render(entries: &[Entry]) -> String {
    let mut s = String::from(
        "# Canonical flop order for the postflop library (spec §7.2, D13).\n\
         # Generated by `taful-library-build flops`; do not edit. The first 49 are the\n\
         # stratified first slice; the rest grow coverage toward all 1,755.\n\
         # order flop orbit class\n",
    );
    for (i, e) in entries.iter().enumerate() {
        let _ = writeln!(s, "{:4} {} {:2} {}", i + 1, e.flop, e.orbit, e.class);
    }
    s
}

/// Parse `flops.txt` back into flops, in order.
pub fn parse(text: &str) -> Result<Vec<Flop>, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f = l
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| format!("bad line {l:?}"))?;
            let flop: Flop = f.parse().map_err(|e| format!("{e}"))?;
            if !flop.is_canonical() {
                return Err(format!("{flop} is not canonical"));
            }
            Ok(flop)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn order_is_a_permutation_with_stratified_head() {
        let o = order(SEED);
        assert_eq!(o.len(), 1755);
        let set: HashSet<Flop> = o.iter().map(|e| e.flop).collect();
        assert_eq!(set.len(), 1755);
        let head_strata: HashSet<Stratum> = o[..FIRST].iter().map(|e| e.stratum).collect();
        let all_strata: HashSet<Stratum> = o.iter().map(|e| e.stratum).collect();
        assert_eq!(head_strata, all_strata, "every stratum in the first slice");
        // After the head, orbit weights never increase.
        assert!(o[FIRST..].windows(2).all(|w| w[0].orbit >= w[1].orbit));
        assert_eq!(parse(&render(&o)).unwrap(), set_order(&o));
    }

    fn set_order(o: &[Entry]) -> Vec<Flop> {
        o.iter().map(|e| e.flop).collect()
    }

    #[test]
    fn committed_order_matches() {
        let committed = include_str!("../../../library/flops.txt");
        assert_eq!(
            committed,
            render(&order(SEED)),
            "library/flops.txt is stale: run `taful-library-build flops library/flops.txt` \
             (only if no flop beyond the solved prefix moved)"
        );
    }
}

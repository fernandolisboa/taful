//! Weighted ranges over the 1,326 combos (spec §5.1): standard-notation
//! parsing/printing and the 13×13 grid view.
//!
//! Notation (comma separated, optional `:weight` suffix on each token):
//! `AA`, `AKs`, `AKo`, `AK` (suited + offsuit), `TT+`, `ATs+`, `A5s-A2s`,
//! `22-55`, `KQo:0.5`, and single combos such as `AsKs`.

use std::fmt;
use std::str::FromStr;

use crate::cards::{Card, CardSet, Hand, NUM_COMBOS, ParseError, Rank, Suit};

/// Weights in `[0, 1]` per combo index.
#[derive(Clone, PartialEq)]
pub struct Range {
    weights: Box<[f32; NUM_COMBOS]>,
}

impl Default for Range {
    fn default() -> Self {
        Range::empty()
    }
}

/// A hand class on the 13×13 grid: a pair, a suited or an offsuit hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandClass {
    Pair(Rank),
    Suited(Rank, Rank),
    Offsuit(Rank, Rank),
}

impl HandClass {
    pub fn of(h: Hand) -> HandClass {
        let (a, b) = (h.high().rank(), h.low().rank());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        if hi == lo {
            HandClass::Pair(hi)
        } else if h.is_suited() {
            HandClass::Suited(hi, lo)
        } else {
            HandClass::Offsuit(hi, lo)
        }
    }

    /// The combos of this class (6 for pairs, 4 suited, 12 offsuit).
    pub fn combos(self) -> Vec<Hand> {
        let mut v = Vec::new();
        let (r1, r2) = match self {
            HandClass::Pair(r) => (r, r),
            HandClass::Suited(a, b) | HandClass::Offsuit(a, b) => (a, b),
        };
        for s1 in Suit::all() {
            for s2 in Suit::all() {
                let keep = match self {
                    HandClass::Pair(_) => s1 < s2,
                    HandClass::Suited(..) => s1 == s2,
                    HandClass::Offsuit(..) => s1 != s2,
                };
                if keep {
                    v.push(Hand::new(Card::new(r1, s1), Card::new(r2, s2)).expect("distinct"));
                }
            }
        }
        v
    }

    /// Grid cell: row/col 0 = ace. Suited above the diagonal, offsuit below.
    pub fn grid_pos(self) -> (usize, usize) {
        let idx = |r: Rank| 12 - r.index() as usize;
        match self {
            HandClass::Pair(r) => (idx(r), idx(r)),
            HandClass::Suited(a, b) => (idx(a), idx(b)),
            HandClass::Offsuit(a, b) => (idx(b), idx(a)),
        }
    }

    pub fn from_grid_pos(row: usize, col: usize) -> HandClass {
        let r = |i: usize| Rank::new(12 - i as u8).expect("grid index < 13");
        match row.cmp(&col) {
            std::cmp::Ordering::Equal => HandClass::Pair(r(row)),
            std::cmp::Ordering::Less => HandClass::Suited(r(row), r(col)),
            std::cmp::Ordering::Greater => HandClass::Offsuit(r(col), r(row)),
        }
    }
}

impl fmt::Display for HandClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HandClass::Pair(r) => write!(f, "{0}{0}", r.to_char()),
            HandClass::Suited(a, b) => write!(f, "{}{}s", a.to_char(), b.to_char()),
            HandClass::Offsuit(a, b) => write!(f, "{}{}o", a.to_char(), b.to_char()),
        }
    }
}

/// One cell of the 13×13 grid view.
#[derive(Debug, Clone, PartialEq)]
pub struct GridCell {
    pub class: HandClass,
    /// Mean weight over the class's combos.
    pub weight: f32,
    /// Combos with non-zero weight.
    pub live_combos: u8,
}

impl Range {
    pub fn empty() -> Range {
        Range {
            weights: Box::new([0.0; NUM_COMBOS]),
        }
    }
    pub fn full() -> Range {
        Range {
            weights: Box::new([1.0; NUM_COMBOS]),
        }
    }
    pub fn weights(&self) -> &[f32; NUM_COMBOS] {
        &self.weights
    }
    pub fn weight(&self, h: Hand) -> f32 {
        self.weights[h.combo()]
    }
    /// Weight is clamped to `[0, 1]`.
    pub fn set(&mut self, h: Hand, w: f32) {
        self.weights[h.combo()] = w.clamp(0.0, 1.0);
    }
    pub fn set_class(&mut self, c: HandClass, w: f32) {
        for h in c.combos() {
            self.set(h, w);
        }
    }
    /// Sum of weights (weighted number of combos).
    pub fn combos(&self) -> f64 {
        self.weights.iter().map(|&w| f64::from(w)).sum()
    }
    /// Combos with non-zero weight that do not touch `dead`.
    pub fn live(&self, dead: CardSet) -> impl Iterator<Item = (Hand, f32)> + '_ {
        Hand::all()
            .zip(self.weights.iter().copied())
            .filter(move |(h, w)| *w > 0.0 && !h.mask().intersects(dead))
    }
    pub fn grid(&self) -> [[GridCell; 13]; 13] {
        std::array::from_fn(|row| {
            std::array::from_fn(|col| {
                let class = HandClass::from_grid_pos(row, col);
                let combos = class.combos();
                let ws: Vec<f32> = combos.iter().map(|&h| self.weight(h)).collect();
                GridCell {
                    class,
                    weight: ws.iter().sum::<f32>() / ws.len() as f32,
                    live_combos: ws.iter().filter(|&&w| w > 0.0).count() as u8,
                }
            })
        })
    }
}

impl fmt::Debug for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Range({self})")
    }
}

fn perr(msg: impl Into<String>) -> ParseError {
    ParseError(msg.into())
}

fn parse_weight(s: &str) -> Result<f32, ParseError> {
    let w: f32 = s.parse().map_err(|_| perr(format!("bad weight {s:?}")))?;
    if !(0.0..=1.0).contains(&w) {
        return Err(perr(format!("weight out of [0, 1]: {s:?}")));
    }
    Ok(w)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pair,
    Suited,
    Offsuit,
    Both,
}

/// `"AKs"` → (A, K, Suited); `"TT"` → (T, T, Pair).
fn parse_class(s: &str) -> Result<(Rank, Rank, Kind), ParseError> {
    let ch: Vec<char> = s.chars().collect();
    let (a, b, k) = match ch.as_slice() {
        [a, b] => (*a, *b, None),
        [a, b, k] => (*a, *b, Some(*k)),
        _ => return Err(perr(format!("bad hand class {s:?}"))),
    };
    let ra = Rank::from_char(a).ok_or_else(|| perr(format!("bad rank in {s:?}")))?;
    let rb = Rank::from_char(b).ok_or_else(|| perr(format!("bad rank in {s:?}")))?;
    let kind = match (ra == rb, k) {
        (true, None) => Kind::Pair,
        (true, Some(_)) => return Err(perr(format!("pairs take no suffix: {s:?}"))),
        (false, None) => Kind::Both,
        (false, Some('s' | 'S')) => Kind::Suited,
        (false, Some('o' | 'O')) => Kind::Offsuit,
        _ => return Err(perr(format!("bad suffix in {s:?}"))),
    };
    let (hi, lo) = if ra > rb { (ra, rb) } else { (rb, ra) };
    Ok((hi, lo, kind))
}

fn classes(hi: Rank, lo: Rank, kind: Kind) -> Vec<HandClass> {
    match kind {
        Kind::Pair => vec![HandClass::Pair(hi)],
        Kind::Suited => vec![HandClass::Suited(hi, lo)],
        Kind::Offsuit => vec![HandClass::Offsuit(hi, lo)],
        Kind::Both => vec![HandClass::Suited(hi, lo), HandClass::Offsuit(hi, lo)],
    }
}

fn r(i: u8) -> Rank {
    Rank::new(i).expect("rank index")
}

fn parse_token(tok: &str) -> Result<Vec<HandClassOrCombo>, ParseError> {
    // Single combo, e.g. "AsKs".
    if tok.len() == 4
        && tok
            .chars()
            .nth(1)
            .is_some_and(|c| Suit::from_char(c).is_some())
    {
        let h: Hand = tok.parse()?;
        return Ok(vec![HandClassOrCombo::Combo(h)]);
    }
    let mut out = Vec::new();
    if let Some(base) = tok.strip_suffix('+') {
        let (hi, lo, kind) = parse_class(base)?;
        if kind == Kind::Pair {
            for i in hi.index()..13 {
                out.extend(classes(r(i), r(i), kind));
            }
        } else {
            for i in lo.index()..hi.index() {
                out.extend(classes(hi, r(i), kind));
            }
        }
    } else if let Some((a, b)) = tok.split_once('-') {
        let (ah, al, ak) = parse_class(a)?;
        let (bh, bl, bk) = parse_class(b)?;
        if ak != bk {
            return Err(perr(format!("range ends differ in kind: {tok:?}")));
        }
        if ak == Kind::Pair {
            let (x, y) = (ah.index().min(bh.index()), ah.index().max(bh.index()));
            for i in x..=y {
                out.extend(classes(r(i), r(i), ak));
            }
        } else {
            if ah != bh {
                return Err(perr(format!("range ends need the same high card: {tok:?}")));
            }
            let (x, y) = (al.index().min(bl.index()), al.index().max(bl.index()));
            for i in x..=y {
                out.extend(classes(ah, r(i), ak));
            }
        }
    } else {
        let (hi, lo, kind) = parse_class(tok)?;
        out.extend(classes(hi, lo, kind));
    }
    Ok(out.into_iter().map(HandClassOrCombo::Class).collect())
}

enum HandClassOrCombo {
    Class(HandClass),
    Combo(Hand),
}

impl FromStr for Range {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Range, ParseError> {
        let mut range = Range::empty();
        for raw in s.split(',') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let (tok, w) = match raw.split_once(':') {
                Some((t, w)) => (t.trim(), parse_weight(w.trim())?),
                None => (raw, 1.0),
            };
            for item in parse_token(tok)? {
                match item {
                    HandClassOrCombo::Class(c) => range.set_class(c, w),
                    HandClassOrCombo::Combo(h) => range.set(h, w),
                }
            }
        }
        Ok(range)
    }
}

fn fmt_weight(w: f32) -> String {
    if w == 1.0 {
        String::new()
    } else {
        let s = format!("{w:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!(":{s}")
    }
}

/// Uniform weight of a class, or `None` if its combos differ.
fn uniform(range: &Range, c: HandClass) -> Option<f32> {
    let combos = c.combos();
    let w = range.weight(combos[0]);
    combos.iter().all(|&h| range.weight(h) == w).then_some(w)
}

impl fmt::Display for Range {
    /// Compact standard notation. Parsing the output yields the same range.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut toks: Vec<String> = Vec::new();
        let mut singles: Vec<String> = Vec::new();

        // Emit runs of equal uniform weight along a sequence of classes ordered
        // from strongest to weakest. `top` is true when the run may use "+".
        let mut emit = |seq: &[HandClass], plus_ok_at_top: bool, toks: &mut Vec<String>| {
            let mut i = 0;
            while i < seq.len() {
                let Some(w) = uniform(self, seq[i]) else {
                    for h in seq[i].combos() {
                        let w = self.weight(h);
                        if w > 0.0 {
                            singles.push(format!("{h}{}", fmt_weight(w)));
                        }
                    }
                    i += 1;
                    continue;
                };
                if w == 0.0 {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j + 1 < seq.len() && uniform(self, seq[j + 1]) == Some(w) {
                    j += 1;
                }
                let tok = if i == j {
                    seq[i].to_string()
                } else if i == 0 && plus_ok_at_top {
                    format!("{}+", seq[j])
                } else {
                    format!("{}-{}", seq[i], seq[j])
                };
                toks.push(format!("{tok}{}", fmt_weight(w)));
                i = j + 1;
            }
        };

        let pairs: Vec<HandClass> = (0..13).rev().map(|i| HandClass::Pair(r(i))).collect();
        emit(&pairs, true, &mut toks);
        for hi in (1..13).rev() {
            for suited in [true, false] {
                let seq: Vec<HandClass> = (0..hi)
                    .rev()
                    .map(|lo| {
                        if suited {
                            HandClass::Suited(r(hi), r(lo))
                        } else {
                            HandClass::Offsuit(r(hi), r(lo))
                        }
                    })
                    .collect();
                emit(&seq, true, &mut toks);
            }
        }
        toks.extend(singles);
        f.write_str(&toks.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_counts() {
        let c = |s: &str| s.parse::<Range>().unwrap().combos();
        assert_eq!(c("AA"), 6.0);
        assert_eq!(c("AKs"), 4.0);
        assert_eq!(c("AKo"), 12.0);
        assert_eq!(c("AK"), 16.0);
        assert_eq!(c("TT+"), 30.0);
        assert_eq!(c("22-55"), 24.0);
        assert_eq!(c("55-22"), 24.0);
        assert_eq!(c("A5s-A2s"), 16.0);
        assert_eq!(c("ATs+"), 16.0);
        assert_eq!(c("KQo:0.5"), 6.0);
        assert_eq!(c("AsKs"), 1.0);
        assert_eq!(c("AKs, TT+, A5s-A2s, KQo:0.5"), 4.0 + 30.0 + 16.0 + 6.0);
        assert!("AKx".parse::<Range>().is_err());
        assert!("AKs:2".parse::<Range>().is_err());
        assert!("AKs-QJs".parse::<Range>().is_err());
    }

    #[test]
    fn print_roundtrip() {
        for s in [
            "AA",
            "TT+",
            "77-TT",
            "AKs,TT+,A5s-A2s,KQo:0.5",
            "ATs+,KTo-K8o,QJ,22+:0.25",
            "AsKs,AhKh:0.5",
        ] {
            let r: Range = s.parse().unwrap();
            let printed = r.to_string();
            let back: Range = printed.parse().unwrap();
            assert_eq!(r, back, "{s} → {printed}");
        }
        assert_eq!("TT+".parse::<Range>().unwrap().to_string(), "TT+");
        assert_eq!("A2s-A5s".parse::<Range>().unwrap().to_string(), "A5s-A2s");
        assert_eq!("AKs,AQs".parse::<Range>().unwrap().to_string(), "AQs+");
    }

    #[test]
    fn grid() {
        let r: Range = "AKs,KQo:0.5,22".parse().unwrap();
        let g = r.grid();
        assert_eq!(g[0][1].class.to_string(), "AKs");
        assert_eq!(g[0][1].weight, 1.0);
        assert_eq!(g[2][1].class.to_string(), "KQo");
        assert_eq!(g[2][1].weight, 0.5);
        assert_eq!(g[12][12].class.to_string(), "22");
        for row in 0..13 {
            for col in 0..13 {
                let c = HandClass::from_grid_pos(row, col);
                assert_eq!(c.grid_pos(), (row, col));
            }
        }
    }
}

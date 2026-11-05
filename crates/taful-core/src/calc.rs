//! Mental-math trainer (spec §11): seeded generators plus exact solvers.
//!
//! Every exercise carries the exact answer and the value the table shortcut
//! gives, so the UI can show both side by side. The pure functions at the top
//! are also what lesson placeholders call (`{{ calc: pot_odds(...) }}`, §12.2).

use std::time::Duration;

use rand::seq::{IndexedRandom, SliceRandom};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::cards::{Board, Card, CardSet, Hand, Rank};
use crate::equity;
use crate::eval::{Accum, Category, HandEvaluator, RsPokerEvaluator};
use crate::range::Range;

// ---- pure functions ------------------------------------------------------------

/// Equity needed to call `bet` into `pot` (pot before the bet): `bet / (pot + 2·bet)`.
pub fn pot_odds(pot: f64, bet: f64) -> f64 {
    bet / (pot + 2.0 * bet)
}

/// Minimum defense frequency vs `bet` into `pot`: `pot / (pot + bet)`.
pub fn mdf(pot: f64, bet: f64) -> f64 {
    pot / (pot + bet)
}

/// Break-even fold frequency for a bluff of `bet` into `pot`: `bet / (pot + bet)`.
pub fn alpha(pot: f64, bet: f64) -> f64 {
    bet / (pot + bet)
}

/// Stack-to-pot ratio.
pub fn spr(stack: f64, pot: f64) -> f64 {
    stack / pot
}

/// Exact probability of hitting one of `outs` with `cards_to_come` (1 or 2)
/// cards from `unseen` cards.
pub fn hit_probability(outs: u32, cards_to_come: u32, unseen: u32) -> f64 {
    let (o, u) = (f64::from(outs), f64::from(unseen));
    match cards_to_come {
        0 => 0.0,
        1 => o / u,
        _ => 1.0 - ((u - o) / u) * ((u - o - 1.0) / (u - 1.0)),
    }
}

/// Rule of 2 and 4 as a probability: `outs × 2%` with one card to come,
/// `outs × 4%` with two, minus `(outs − 8)%` above 8 outs.
pub fn rule_of_2_and_4(outs: u32, cards_to_come: u32) -> f64 {
    let o = f64::from(outs);
    let pct = if cards_to_come >= 2 {
        4.0 * o - (o - 8.0).max(0.0)
    } else {
        2.0 * o
    };
    pct / 100.0
}

/// Cards whose arrival on the next street puts `hero` strictly ahead of
/// `villain` (exact enumeration). `board` is a flop or a turn.
pub fn outs(hero: Hand, villain: Hand, board: &Board) -> Vec<Card> {
    let dead = hero.mask().union(villain.mask()).union(board.mask());
    let base = Accum::new(board.cards());
    let [h1, h2] = hero.cards();
    let [v1, v2] = villain.cards();
    dead.complement()
        .iter()
        .filter(|&c| {
            let acc = base.with(c);
            acc.with(h1).with(h2).score() > acc.with(v1).with(v2).score()
        })
        .collect()
}

/// Outs by the table shortcut: flush draw (9), open-ended (8) or gutshot (4)
/// straight draws and unpaired overcards (3 each), each card counted once,
/// then discounted for dirty outs: a flush or straight card that pairs the
/// board is dropped (it can fill up a set or two pair). The exact count
/// ([`outs`]) also catches the dirty outs no table rule sees.
pub fn shortcut_outs(hero: Hand, board: &Board) -> Vec<Card> {
    let seen = hero.mask().union(board.mask());
    let cards: Vec<Card> = hero
        .cards()
        .into_iter()
        .chain(board.cards().iter().copied())
        .collect();
    let mut out = CardSet::EMPTY;
    let board_ranks: u16 = board
        .cards()
        .iter()
        .fold(0, |m, c| m | 1 << c.rank().index());
    let clean = |c: &Card| board_ranks & (1 << c.rank().index()) == 0;
    // Flush draw: four to a suit, using at least one hole card.
    for s in crate::cards::Suit::all() {
        let n = cards.iter().filter(|c| c.suit() == s).count();
        if n == 4 && hero.cards().iter().any(|c| c.suit() == s) {
            for c in seen
                .complement()
                .iter()
                .filter(|c| c.suit() == s && clean(c))
            {
                out.insert(c);
            }
        }
    }
    // Straight draws: ranks that complete a straight not already made.
    let ranks = |cs: &[Card]| -> u16 { cs.iter().fold(0u16, |m, c| m | 1 << c.rank().index()) };
    // Bits: 0 = deuce … 12 = ace; the wheel is A-2-3-4-5.
    let has_straight = |m: u16| -> bool {
        let wheel = (1 << 12) | 0b1111;
        (0..=8).any(|lo| (m >> lo) & 0b11111 == 0b11111) || m & wheel == wheel
    };
    let made = ranks(&cards);
    if !has_straight(made) {
        for r in Rank::all() {
            let with = made | 1 << r.index();
            if made & (1 << r.index()) == 0 && has_straight(with) {
                // Must use a hole card: the board alone plus r is not a straight.
                if !has_straight(ranks(board.cards()) | 1 << r.index()) {
                    for c in seen
                        .complement()
                        .iter()
                        .filter(|c| c.rank() == r && clean(c))
                    {
                        out.insert(c);
                    }
                }
            }
        }
    }
    // Overcards, only without a pair.
    let board_max = board
        .cards()
        .iter()
        .map(|c| c.rank())
        .max()
        .unwrap_or(Rank::TWO);
    let paired = hero.is_pair()
        || hero
            .cards()
            .iter()
            .any(|h| board.cards().iter().any(|b| b.rank() == h.rank()));
    if !paired {
        for h in hero.cards() {
            if h.rank() > board_max {
                for c in seen.complement().iter().filter(|c| c.rank() == h.rank()) {
                    out.insert(c);
                }
            }
        }
    }
    out.iter().collect()
}

fn hero_paired(hero: Hand, board: &Board) -> bool {
    hero.is_pair()
        || hero
            .cards()
            .iter()
            .any(|h| board.cards().iter().any(|b| b.rank() == h.rank()))
}

/// Rough equity of `hero` against a calling range, by made-hand class (the
/// "heuristic bands" shown next to the exact value).
pub fn equity_band(hero: Hand, board: &Board) -> f64 {
    let cards: Vec<Card> = hero
        .cards()
        .into_iter()
        .chain(board.cards().iter().copied())
        .collect();
    let r = RsPokerEvaluator.rank(&cards);
    let board_max = board
        .cards()
        .iter()
        .map(|c| c.rank())
        .max()
        .unwrap_or(Rank::TWO);
    match r.category {
        c if c >= Category::Straight => 0.85,
        Category::ThreeOfAKind | Category::TwoPair => 0.75,
        // A pair that is only on the board is not hero's pair.
        Category::OnePair if hero_paired(hero, board) => {
            let top = hero.is_pair() && hero.high().rank() > board_max
                || hero.cards().iter().any(|h| h.rank() == board_max);
            if top { 0.65 } else { 0.45 }
        }
        _ => {
            let n = shortcut_outs(hero, board).len() as u32;
            let cards_to_come = (5 - board.len()) as u32;
            (0.05 + rule_of_2_and_4(n, cards_to_come)).clamp(0.20, 0.50)
        }
    }
}

// ---- exercises -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseKind {
    Outs,
    RuleOf2And4,
    PotOdds,
    Mdf,
    Alpha,
    Spr,
    EquityVsRange,
}

impl ExerciseKind {
    pub const ALL: [ExerciseKind; 7] = [
        ExerciseKind::Outs,
        ExerciseKind::RuleOf2And4,
        ExerciseKind::PotOdds,
        ExerciseKind::Mdf,
        ExerciseKind::Alpha,
        ExerciseKind::Spr,
        ExerciseKind::EquityVsRange,
    ];

    /// Stable id for storage, review-item keys and i18n (`calc-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            ExerciseKind::Outs => "outs",
            ExerciseKind::RuleOf2And4 => "rule_of_2_and_4",
            ExerciseKind::PotOdds => "pot_odds",
            ExerciseKind::Mdf => "mdf",
            ExerciseKind::Alpha => "alpha",
            ExerciseKind::Spr => "spr",
            ExerciseKind::EquityVsRange => "equity_vs_range",
        }
    }
}

/// What the user is shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Prompt {
    /// Count the cards that put hero ahead of the (shown) villain hand.
    Outs {
        hero: Hand,
        villain: Hand,
        board: String,
    },
    /// Chance to hit with `cards_to_come` cards (1 = turn to river, 2 = flop to river).
    RuleOf2And4 {
        outs: u32,
        cards_to_come: u32,
    },
    /// Equity needed to call `bet_bb` into `pot_bb`.
    PotOdds {
        pot_bb: f64,
        bet_bb: f64,
    },
    /// Minimum defense vs a bet of `bet_pct`% pot.
    Mdf {
        bet_pct: u32,
    },
    /// Folds a bluff of `bet_pct`% pot needs.
    Alpha {
        bet_pct: u32,
    },
    Spr {
        stack_bb: f64,
        pot_bb: f64,
    },
    /// Hero's equity vs `villain` (range notation) on `board`.
    EquityVsRange {
        hero: Hand,
        board: String,
        villain: String,
    },
}

/// How answers are expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Percentage points, 0–100.
    Percent,
    Count,
    Ratio,
}

/// One generated exercise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exercise {
    pub kind: ExerciseKind,
    pub seed: u64,
    pub prompt: Prompt,
    pub unit: Unit,
    /// Exact answer, in `unit`.
    pub answer: f64,
    /// What the table shortcut gives, in `unit`.
    pub shortcut: f64,
    /// Parameter band, e.g. `bet_50`: with the kind, the review-item key (§13).
    pub band: String,
}

/// Tolerance and target time for one exercise kind.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindSettings {
    /// Accepted distance from the exact answer, in the exercise's unit.
    pub tolerance: f64,
    pub target_secs: f64,
}

/// Per-kind settings (`config/calc.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalcSettings {
    pub outs: KindSettings,
    pub rule_of_2_and_4: KindSettings,
    pub pot_odds: KindSettings,
    pub mdf: KindSettings,
    pub alpha: KindSettings,
    pub spr: KindSettings,
    pub equity_vs_range: KindSettings,
}

impl Default for CalcSettings {
    fn default() -> Self {
        let k = |tolerance, target_secs| KindSettings {
            tolerance,
            target_secs,
        };
        CalcSettings {
            outs: k(1.0, 15.0),
            rule_of_2_and_4: k(2.0, 8.0),
            pot_odds: k(1.0, 10.0),
            mdf: k(1.0, 6.0),
            alpha: k(1.0, 6.0),
            spr: k(0.2, 8.0),
            equity_vs_range: k(2.0, 25.0),
        }
    }
}

impl CalcSettings {
    pub fn from_toml(s: &str) -> Result<CalcSettings, String> {
        let c: CalcSettings = toml::from_str(s).map_err(|e| e.to_string())?;
        c.validate()?;
        Ok(c)
    }

    /// Tolerances must be finite and >= 0; target times finite and > 0.
    pub fn validate(&self) -> Result<(), String> {
        for kind in ExerciseKind::ALL {
            let k = self.get(kind);
            if !(k.tolerance.is_finite() && k.tolerance >= 0.0) {
                return Err(format!("{}: tolerance must be finite and >= 0", kind.id()));
            }
            if !(k.target_secs.is_finite() && k.target_secs > 0.0) {
                return Err(format!("{}: target_secs must be finite and > 0", kind.id()));
            }
        }
        Ok(())
    }

    pub fn get(&self, kind: ExerciseKind) -> KindSettings {
        match kind {
            ExerciseKind::Outs => self.outs,
            ExerciseKind::RuleOf2And4 => self.rule_of_2_and_4,
            ExerciseKind::PotOdds => self.pot_odds,
            ExerciseKind::Mdf => self.mdf,
            ExerciseKind::Alpha => self.alpha,
            ExerciseKind::Spr => self.spr,
            ExerciseKind::EquityVsRange => self.equity_vs_range,
        }
    }
}

impl Exercise {
    /// Whether `given` is within tolerance of the exact answer.
    pub fn check(&self, given: f64, settings: &CalcSettings) -> bool {
        given.is_finite() && (given - self.answer).abs() <= settings.get(self.kind).tolerance + 1e-9
    }

    pub fn target(&self, settings: &CalcSettings) -> Duration {
        Duration::from_secs_f64(settings.get(self.kind).target_secs.max(0.0))
    }
}

const BET_PCTS: [u32; 10] = [25, 33, 50, 66, 75, 100, 125, 150, 200, 300];

/// Calling ranges the equity exercise draws from (rough, for practice only).
pub const VILLAIN_RANGES: [&str; 3] = [
    "TT+,AQs+,AKo",
    "22+,A2s+,K9s+,QTs+,JTs,T9s,98s,ATo+,KJo+",
    "22+,A2s+,K2s+,Q8s+,J8s+,T8s+,97s+,86s+,76s,65s,A2o+,K9o+,QTo+,JTo",
];

fn half_bb(x: f64) -> f64 {
    (x * 2.0).round() / 2.0
}

fn deal(rng: &mut ChaCha8Rng, n: usize, avoid: CardSet) -> Vec<Card> {
    let mut deck: Vec<Card> = avoid.complement().iter().collect();
    deck.partial_shuffle(rng, n).0.to_vec()
}

fn hand_of(cs: &[Card]) -> Hand {
    Hand::new(cs[0], cs[1]).expect("distinct")
}

/// Generate an exercise of `kind` from `seed` (deterministic).
pub fn generate(kind: ExerciseKind, seed: u64) -> Exercise {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let rng = &mut rng;
    let ex = |prompt, unit, answer, shortcut, band: String| Exercise {
        kind,
        seed,
        prompt,
        unit,
        answer,
        shortcut,
        band,
    };
    match kind {
        ExerciseKind::Outs => {
            // Hero behind with 4–15 outs on a flop or turn.
            let mut fallback = None;
            for _ in 0..10_000 {
                let n_board = if rng.random_bool(0.6) { 3 } else { 4 };
                let cs = deal(rng, 4 + n_board, CardSet::EMPTY);
                let (hero, villain) = (hand_of(&cs[0..2]), hand_of(&cs[2..4]));
                let board = Board::new(&cs[4..]).expect("valid board");
                let base = Accum::new(board.cards());
                let behind =
                    base.with(cs[0]).with(cs[1]).score() < base.with(cs[2]).with(cs[3]).score();
                if !behind {
                    continue;
                }
                let o = outs(hero, villain, &board).len();
                let short = shortcut_outs(hero, &board).len();
                let e = ex(
                    Prompt::Outs {
                        hero,
                        villain,
                        board: board.to_string(),
                    },
                    Unit::Count,
                    o as f64,
                    short as f64,
                    format!(
                        "{}_{}",
                        if n_board == 3 { "flop" } else { "turn" },
                        if o >= 8 { "draw" } else { "weak" }
                    ),
                );
                if (4..=15).contains(&o) {
                    return e;
                }
                fallback.get_or_insert(e);
            }
            fallback.expect("some hand is behind within 10,000 deals")
        }
        ExerciseKind::RuleOf2And4 => {
            let outs = rng.random_range(2..=17);
            let cards_to_come = if rng.random_bool(0.5) { 2 } else { 1 };
            let unseen = if cards_to_come == 2 { 47 } else { 46 };
            ex(
                Prompt::RuleOf2And4 {
                    outs,
                    cards_to_come,
                },
                Unit::Percent,
                100.0 * hit_probability(outs, cards_to_come, unseen),
                100.0 * rule_of_2_and_4(outs, cards_to_come),
                format!(
                    "{}card_{}",
                    cards_to_come,
                    if outs > 8 { "many" } else { "few" }
                ),
            )
        }
        ExerciseKind::PotOdds => {
            let pot = half_bb(rng.random_range(2.0..150.0));
            let pct = *BET_PCTS.choose(rng).expect("non-empty");
            let bet = half_bb(pot * f64::from(pct) / 100.0).max(0.5);
            let a = 100.0 * pot_odds(pot, bet);
            ex(
                Prompt::PotOdds {
                    pot_bb: pot,
                    bet_bb: bet,
                },
                Unit::Percent,
                a,
                a,
                format!("bet_{pct}"),
            )
        }
        ExerciseKind::Mdf => {
            let pct = *BET_PCTS.choose(rng).expect("non-empty");
            let a = 100.0 * mdf(100.0, f64::from(pct));
            ex(
                Prompt::Mdf { bet_pct: pct },
                Unit::Percent,
                a,
                a,
                format!("bet_{pct}"),
            )
        }
        ExerciseKind::Alpha => {
            let pct = *BET_PCTS.choose(rng).expect("non-empty");
            let a = 100.0 * alpha(100.0, f64::from(pct));
            ex(
                Prompt::Alpha { bet_pct: pct },
                Unit::Percent,
                a,
                a,
                format!("bet_{pct}"),
            )
        }
        ExerciseKind::Spr => {
            let pot = half_bb(rng.random_range(3.0..120.0));
            let stack = half_bb(rng.random_range(10.0..300.0));
            let a = spr(stack, pot);
            let band = match a {
                x if x < 3.0 => "low",
                x if x < 6.0 => "medium",
                x if x < 13.0 => "high",
                _ => "deep",
            };
            ex(
                Prompt::Spr {
                    stack_bb: stack,
                    pot_bb: pot,
                },
                Unit::Ratio,
                a,
                a,
                band.into(),
            )
        }
        ExerciseKind::EquityVsRange => loop {
            let idx = rng.random_range(0..VILLAIN_RANGES.len());
            let range: Range = VILLAIN_RANGES[idx].parse().expect("valid preset");
            let cs = deal(rng, 5, CardSet::EMPTY);
            let hero = hand_of(&cs[0..2]);
            let board = Board::new(&cs[2..5]).expect("valid flop");
            let Ok(e) = equity::hand_vs_range(hero, &range, &board) else {
                continue;
            };
            break ex(
                Prompt::EquityVsRange {
                    hero,
                    board: board.to_string(),
                    villain: VILLAIN_RANGES[idx].into(),
                },
                Unit::Percent,
                100.0 * e.equity,
                100.0 * equity_band(hero, &board),
                format!("range_{idx}"),
            );
        },
    }
}

/// Brute-force outs via the evaluator trait, for cross-checking [`outs`].
#[cfg(test)]
fn brute_outs(hero: Hand, villain: Hand, board: &Board) -> usize {
    use crate::equity::for_each_subset;
    let dead = hero.mask().union(villain.mask()).union(board.mask());
    let avail: Vec<Card> = dead.complement().iter().collect();
    let mut n = 0;
    for_each_subset(&avail, 1, |c| {
        let mut b = board.cards().to_vec();
        b.push(c[0]);
        let e = RsPokerEvaluator;
        let mut hc = b.clone();
        hc.extend(hero.cards());
        let mut vc = b;
        vc.extend(villain.cards());
        if e.rank(&hc) > e.rank(&vc) {
            n += 1;
        }
    });
    n
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

    #[test]
    fn formulas() {
        assert!((pot_odds(100.0, 50.0) - 0.25).abs() < 1e-12);
        assert!((pot_odds(100.0, 100.0) - 1.0 / 3.0).abs() < 1e-12);
        assert!((mdf(100.0, 50.0) - 2.0 / 3.0).abs() < 1e-12);
        assert!((alpha(100.0, 100.0) - 0.5).abs() < 1e-12);
        assert_eq!(spr(400.0, 100.0), 4.0);
        // Flush draw flop → river: 9 outs of 47, two cards.
        assert!((hit_probability(9, 2, 47) - 0.349_676).abs() < 1e-6);
        assert!((hit_probability(9, 1, 46) - 9.0 / 46.0).abs() < 1e-12);
        assert!((rule_of_2_and_4(9, 2) - 0.35).abs() < 1e-12);
        assert!((rule_of_2_and_4(15, 2) - 0.53).abs() < 1e-12);
        assert!((rule_of_2_and_4(9, 1) - 0.18).abs() < 1e-12);
    }

    #[test]
    fn hit_probability_vs_brute_force() {
        // Count two-card runouts from 47 unseen that contain at least one of `o` outs.
        for o in 0..=20u32 {
            let (mut hit, mut all) = (0u32, 0u32);
            for i in 0..47u32 {
                for j in i + 1..47 {
                    all += 1;
                    if i < o || j < o {
                        hit += 1;
                    }
                }
            }
            assert!((hit_probability(o, 2, 47) - f64::from(hit) / f64::from(all)).abs() < 1e-12);
        }
    }

    #[test]
    fn outs_known_spots() {
        // Nut flush draw vs a set: 9 hearts, but 2h pairs the board and gives
        // the set a full house, so 8 clean outs. The shortcut drops 2h too but
        // also counts the ace overcards: 8 + 3 = 11.
        let (hero, vil, board) = (h("AhJh"), h("KsKc"), b("Kh7h2d"));
        let o = outs(hero, vil, &board);
        assert_eq!(o.len(), brute_outs(hero, vil, &board));
        assert_eq!(o.len(), 8);
        assert_eq!(shortcut_outs(hero, &board).len(), 11);
        // Open-ender vs an overpair: 8 clean outs; shortcut adds 6 overcard outs.
        let (hero, vil, board) = (h("9c8c"), h("AsAd"), b("7d6s2h"));
        assert_eq!(outs(hero, vil, &board).len(), 8);
        assert_eq!(shortcut_outs(hero, &board).len(), 14);
        // Gutshot on the turn: 4 outs.
        let (hero, vil, board) = (h("Jc9c"), h("AsAd"), b("Qd8s2h3c"));
        assert_eq!(outs(hero, vil, &board).len(), 4);
        assert_eq!(shortcut_outs(hero, &board).len(), 4);
    }

    #[test]
    fn generators_are_deterministic_and_consistent() {
        for kind in ExerciseKind::ALL {
            for seed in 0..20 {
                let e = generate(kind, seed);
                assert_eq!(e, generate(kind, seed));
                assert!(e.answer.is_finite() && e.shortcut.is_finite(), "{e:?}");
                assert!(e.check(e.answer, &CalcSettings::default()));
                if let Prompt::Outs {
                    hero,
                    villain,
                    board,
                } = &e.prompt
                {
                    let board: Board = board.parse().unwrap();
                    assert_eq!(e.answer as usize, brute_outs(*hero, *villain, &board));
                }
                if let Prompt::EquityVsRange {
                    hero,
                    board,
                    villain,
                } = &e.prompt
                {
                    let board: Board = board.parse().unwrap();
                    let r: Range = villain.parse().unwrap();
                    let exact = equity::hand_vs_range(*hero, &r, &board).unwrap();
                    assert!((e.answer - 100.0 * exact.equity).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn settings_file_matches_defaults() {
        let file = include_str!("../../../config/calc.toml");
        assert_eq!(
            CalcSettings::from_toml(file).unwrap(),
            CalcSettings::default()
        );
    }

    #[test]
    fn rejects_bad_settings() {
        let file = include_str!("../../../config/calc.toml");
        assert!(
            CalcSettings::from_toml(&file.replace("target_secs = 6.0", "target_secs = inf"))
                .is_err()
        );
        assert!(
            CalcSettings::from_toml(&file.replace("tolerance = 0.2", "tolerance = -1.0")).is_err()
        );
    }

    #[test]
    fn equity_band_ignores_board_pairs() {
        let paired = equity_band(h("AsQd"), &b("7c7dKh"));
        let dry = equity_band(h("AsQd"), &b("7c6dKh"));
        assert_eq!(paired, dry);
        assert_eq!(equity_band(h("KsQd"), &b("7c6dKh")), 0.65);
    }

    #[test]
    fn checking_answers() {
        let s = CalcSettings::default();
        let e = generate(ExerciseKind::PotOdds, 3);
        assert!(e.check(e.answer + 0.9, &s));
        assert!(!e.check(e.answer + 1.5, &s));
        assert!(!e.check(f64::NAN, &s));
    }
}

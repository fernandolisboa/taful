//! What a table shows beside a decision, on top of its grade: the acting
//! player's range by hand class with the reference mix of each class, what
//! hero holds in plain words, and hero's equity against the range of the
//! player who bet last.
//!
//! Definitions:
//! - A class's **weight** is the share of its combos (the ones the board
//!   leaves possible) still in the range at this point, from 0 to 1.
//! - A class's **mix** is how the reference plays it here, per action; on
//!   the flop and later, the average over its combos weighted by how often
//!   each is in the range.
//! - **Equity** is raw showdown equity: every runout on the flop and
//!   later, the all-in table of the preflop build before the flop. The
//!   opponent's range follows its profile when it plays one.
//! - **Pot odds** are the share of the final pot hero pays to call.

use taful_core::cards::{Board, Card, CardSet, Hand, Rank};
use taful_core::engine::GameState;
use taful_core::equity::hand_vs_range;
use taful_core::eval::{Category, HandEvaluator, RsPokerEvaluator};
use taful_core::position::Position;
use taful_core::profile::Profile;
use taful_core::range::{HandClass, Range};
use taful_solver::preflop::NodeId;

use crate::hand::FullHand;
use crate::preflop::{Action, PreflopTrainer, Seat};
use crate::training::{DrillSpot, Trainers};
use crate::walk::Walk;

/// One hand class of a range.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeCell {
    /// Share of the class still in the range, 0 to 1.
    pub weight: f32,
    /// Reference frequency of each action; empty when the class has no
    /// data here.
    pub mix: Vec<f32>,
}

impl RangeCell {
    /// Below this weight a class counts as out of the range.
    pub const IN_RANGE: f32 = 0.01;

    pub fn in_range(&self) -> bool {
        self.weight >= Self::IN_RANGE && !self.mix.is_empty()
    }
}

/// Hero's equity against one opponent's range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VsRange {
    pub villain: Position,
    /// 0 to 1.
    pub equity: f64,
}

/// The decision point behind an evaluation panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Insight {
    /// Who acts (hero): the owner of the range.
    pub position: Position,
    pub hand: Hand,
    pub actions: Vec<Action>,
    /// One cell per class, in [`HandClass::index`] order.
    pub cells: Vec<RangeCell>,
    /// Share of the whole range taking each action.
    pub totals: Vec<f32>,
    /// What hero holds, as ids for the UI to name (see [`describe`]).
    pub note: Vec<&'static str>,
    pub equity: Option<VsRange>,
    /// Equity a call needs, when there is a bet to call.
    pub pot_odds: Option<f64>,
}

impl Insight {
    pub fn class(&self) -> HandClass {
        HandClass::of(self.hand)
    }
}

impl Trainers {
    /// The decision point of a spot-mode question.
    pub fn insight(&self, q: &DrillSpot) -> Option<Insight> {
        match q {
            DrillSpot::Preflop(s) => {
                let villain = last_aggressor(&self.preflop, s.node);
                let profile = villain.and_then(|v| {
                    let id = s.opponent.as_deref()?;
                    if crate::mixed::is_table_key(id) {
                        let seats = self.preflop.seats_of_key(id)?;
                        let seat = seat_at(&s.state, v)?;
                        self.preflop.profile(seats.get(seat)?.as_deref()?)
                    } else {
                        self.preflop.profile(id)
                    }
                    .map(|p| p.bot.profile())
                });
                preflop(&self.preflop, s.node, s.hand, &s.state, profile)
            }
            DrillSpot::Postflop(s) => postflop(s.walk(), s.hand, &s.state),
        }
    }
}

impl FullHand {
    /// The decision point hero is at; `None` when it is not hero's turn.
    pub fn insight(&self, t: &Trainers) -> Option<Insight> {
        if self.end.is_some() || self.state.to_act() != Some(self.hero) {
            return None;
        }
        let hand = self.hero_hand();
        if let Some(walk) = self.walk() {
            return postflop(walk, hand, &self.state);
        }
        let node = self.preflop_node()?;
        let profile = last_aggressor(&t.preflop, node).and_then(|v| {
            let seat = seat_at(&self.state, v)?;
            let id = self.seat_profiles.get(seat)?.as_deref()?;
            t.preflop.profile(id).map(|p| p.bot.profile())
        });
        preflop(&t.preflop, node, hand, &self.state, profile)
    }
}

fn seat_at(state: &GameState, pos: Position) -> Option<Seat> {
    (0..state.players().len()).find(|&s| state.position(s) == pos)
}

/// Equity a call needs at `state`: none unless the player to act faces a
/// bet or a raise (completing the blinds is not a price worth quoting).
pub fn pot_odds(state: &GameState) -> Option<f64> {
    let to_call = state.bounds()?.to_call;
    (to_call > 0 && state.aggressions() > 0)
        .then(|| to_call as f64 / (state.pot() + to_call) as f64)
}

/// The last player to bet or raise on the way to `node`, other than the
/// one acting there.
fn last_aggressor(pre: &PreflopTrainer, node: NodeId) -> Option<Position> {
    let chart = pre.chart();
    let actor = chart.node(node).player;
    chart
        .line(node)
        .0
        .iter()
        .rev()
        .find(|(p, a)| a.is_aggressive() && *p != actor)
        .map(|(p, _)| *p)
}

/// How often `pos` gets to `node` with each class, playing `profile`
/// instead of the reference when it has one.
fn reach_as(
    pre: &PreflopTrainer,
    node: NodeId,
    pos: Position,
    profile: Option<&Profile>,
) -> [f32; HandClass::COUNT] {
    let chart = pre.chart();
    let Some(profile) = profile else {
        return chart.reach(node, pos);
    };
    let mut reach = [1.0f32; HandClass::COUNT];
    let mut at = node;
    while let Some((parent, action)) = chart.parent(at) {
        let n = chart.node(parent);
        if n.player == pos {
            let k = n.actions.len();
            let strategy = chart.profile_strategy(parent, profile);
            for (c, w) in reach.iter_mut().enumerate() {
                *w *= strategy[c * k + action];
            }
        }
        at = parent;
    }
    reach
}

fn preflop(
    pre: &PreflopTrainer,
    node: NodeId,
    hand: Hand,
    state: &GameState,
    villain_profile: Option<&Profile>,
) -> Option<Insight> {
    let chart = pre.chart();
    let n = chart.node(node);
    let k = n.actions.len();
    let reach = chart.reach(node, n.player);
    let cells: Vec<RangeCell> = (0..HandClass::COUNT)
        .map(|c| match n.strategy(c) {
            Some(mix) => RangeCell {
                weight: reach[c],
                mix,
            },
            None => RangeCell {
                weight: 0.0,
                mix: Vec::new(),
            },
        })
        .collect();
    let range = chart.acting_range(node);
    let totals = (0..k)
        .map(|a| {
            (0..HandClass::COUNT)
                .map(|c| range[c] * cells[c].mix.get(a).copied().unwrap_or(0.0))
                .sum()
        })
        .collect();
    let class = HandClass::of(hand).index();
    let equity = last_aggressor(pre, node).and_then(|villain| {
        let tables = pre.class_tables()?;
        let reach = reach_as(pre, node, villain, villain_profile);
        let (mut won, mut total) = (0.0f64, 0.0f64);
        for (b, &w) in reach.iter().enumerate() {
            let combos = f64::from(w) * f64::from(tables.compat[class * HandClass::COUNT + b]);
            won += combos * f64::from(tables.equity[class * HandClass::COUNT + b]);
            total += combos;
        }
        (total > 0.0).then(|| VsRange {
            villain,
            equity: won / total,
        })
    });
    Some(Insight {
        position: n.player,
        hand,
        actions: n.actions.iter().map(|a| a.action).collect(),
        cells,
        totals,
        note: describe(hand, &[]),
        equity,
        pot_odds: pot_odds(state),
    })
}

fn postflop(walk: &Walk, hand: Hand, state: &GameState) -> Option<Insight> {
    let node = walk.node()?;
    let k = node.actions.len();
    let dead = CardSet::from_cards(walk.board());
    let reach = walk.reach(node.side);
    let cells: Vec<RangeCell> = HandClass::all()
        .map(|class| {
            let (mut live, mut weight) = (0u32, 0.0f32);
            let mut mix = vec![0.0f32; k];
            for h in class.combos() {
                if h.mask().intersects(dead) {
                    continue;
                }
                live += 1;
                let w = reach[h.combo()];
                if w <= 0.0 {
                    continue;
                }
                if let Some(s) = walk.strategy(h) {
                    weight += w;
                    for (m, f) in mix.iter_mut().zip(s) {
                        *m += w * f;
                    }
                }
            }
            if weight > 0.0 {
                for m in &mut mix {
                    *m /= weight;
                }
                RangeCell {
                    weight: (weight / live as f32).min(1.0),
                    mix,
                }
            } else {
                RangeCell {
                    weight: 0.0,
                    mix: Vec::new(),
                }
            }
        })
        .collect();
    let villain_side = 1 - node.side;
    let equity = Board::new(walk.board()).ok().and_then(|board| {
        let mut range = Range::empty();
        for (i, &w) in walk.reach(villain_side).iter().enumerate() {
            if w > 0.0 {
                range.set(Hand::from_combo(i)?, w);
            }
        }
        let e = hand_vs_range(hand, &range, &board).ok()?;
        Some(VsRange {
            villain: walk.positions()[villain_side],
            equity: e.equity,
        })
    });
    Some(Insight {
        position: node.position,
        hand,
        actions: node.actions,
        cells,
        totals: walk.range_freqs(),
        note: describe(hand, walk.board()),
        equity,
        pot_odds: pot_odds(state),
    })
}

/// What `hand` is on `board` (empty before the flop), as ids: the made
/// hand first when it is worth naming, then a draw. A weak made hand with a
/// draw is named by the draw alone.
///
/// Made hands: `straight-flush`, `quads`, `full-house`, `flush`,
/// `straight`, `set` (pocket pair plus one on the board), `trips`,
/// `two-pair`, `overpair`, `top-pair`, `middle-pair`, `bottom-pair`,
/// `pocket-pair` (between the board's cards), `underpair`, `ace-high`,
/// `overcards`, `high-card`. Draws: `combo-draw` (flush and straight),
/// `flush-draw`, `open-ended`, `gutshot`. Before the flop: `pocket-pair`,
/// `suited-ace`, `suited-broadways`, `suited-connectors`, `suited-gapper`,
/// `suited`, `offsuit-ace`, `offsuit-broadways`, `offsuit-connectors`,
/// `offsuit`.
pub fn describe(hand: Hand, board: &[Card]) -> Vec<&'static str> {
    let [a, b] = hand.cards();
    let (hi, lo) = if a.rank() >= b.rank() {
        (a.rank(), b.rank())
    } else {
        (b.rank(), a.rank())
    };
    if board.len() < 3 {
        let gap = hi.index() - lo.index();
        let broadways = lo >= Rank::TEN;
        return vec![match (hi == lo, hand.is_suited()) {
            (true, _) => "pocket-pair",
            (_, true) if hi == Rank::ACE => "suited-ace",
            (_, true) if broadways => "suited-broadways",
            (_, true) if gap == 1 => "suited-connectors",
            (_, true) if gap == 2 => "suited-gapper",
            (_, true) => "suited",
            (_, false) if hi == Rank::ACE => "offsuit-ace",
            (_, false) if broadways => "offsuit-broadways",
            (_, false) if gap == 1 => "offsuit-connectors",
            (_, false) => "offsuit",
        }];
    }
    let mut all = vec![a, b];
    all.extend_from_slice(board);
    let category = RsPokerEvaluator.rank(&all).category;
    let on_board = |r: Rank| board.iter().filter(|c| c.rank() == r).count();
    let mut ranks: Vec<Rank> = board.iter().map(|c| c.rank()).collect();
    ranks.sort_unstable();
    ranks.dedup();
    let (top, bottom) = (ranks[ranks.len() - 1], ranks[0]);
    let weak = || {
        if hi == Rank::ACE {
            "ace-high"
        } else if lo > top {
            "overcards"
        } else {
            "high-card"
        }
    };
    // Hero's own pair: the board's pairs are everyone's.
    let pair = || {
        if hi == lo {
            return if lo > top {
                "overpair"
            } else if hi < bottom {
                "underpair"
            } else {
                "pocket-pair"
            };
        }
        let hits: Vec<Rank> = [hi, lo].into_iter().filter(|&r| on_board(r) > 0).collect();
        match hits[..] {
            [_, _] => "two-pair",
            [r] if r == top => "top-pair",
            [r] if r == bottom && ranks.len() > 1 => "bottom-pair",
            [_] => "middle-pair",
            _ => weak(),
        }
    };
    let made = match category {
        Category::StraightFlush => "straight-flush",
        Category::FourOfAKind => "quads",
        Category::FullHouse => "full-house",
        Category::Flush => "flush",
        Category::Straight => "straight",
        Category::ThreeOfAKind if hi == lo && on_board(hi) == 1 => "set",
        Category::ThreeOfAKind if on_board(hi) == 2 || on_board(lo) == 2 => "trips",
        Category::ThreeOfAKind => weak(),
        Category::TwoPair | Category::OnePair => pair(),
        Category::HighCard => weak(),
    };
    if category >= Category::Straight || board.len() >= 5 {
        return vec![made];
    }
    let flush_draw = [a.suit(), b.suit()]
        .into_iter()
        .any(|s| all.iter().filter(|c| c.suit() == s).count() == 4);
    let bits = |cards: &[Card]| cards.iter().fold(0u16, |m, c| m | 1 << c.rank().index());
    let outs = straight_outs(bits(&all)).saturating_sub(straight_outs(bits(board)));
    let draw = match (flush_draw, outs) {
        (true, 1..) => Some("combo-draw"),
        (true, 0) => Some("flush-draw"),
        (false, 2..) => Some("open-ended"),
        (false, 1) => Some("gutshot"),
        (false, 0) => None,
    };
    match draw {
        Some(d) if matches!(made, "ace-high" | "overcards" | "high-card") => vec![d],
        Some(d) => vec![made, d],
        None => vec![made],
    }
}

/// Ranks that would complete a straight with the ranks in `bits` (bit 0 is
/// the deuce, bit 12 the ace).
fn straight_outs(bits: u16) -> u32 {
    // Bit 0 becomes the low ace, the deuce moves to bit 1.
    let with_low_ace = |m: u16| (m << 1) | (m >> 12 & 1);
    let straight = |m: u16| (0..10).any(|lowest| (with_low_ace(m) >> lowest) & 0b11111 == 0b11111);
    (0..13)
        .filter(|r| bits & (1 << r) == 0 && straight(bits | 1 << r))
        .count() as u32
}

#[cfg(test)]
mod tests {
    use taful_core::cards::parse_cards;

    use super::*;
    use crate::postflop::SpotRequest;
    use crate::preflop::SpotKind;

    fn note(hand: &str, board: &str) -> Vec<&'static str> {
        let h = parse_cards(hand).unwrap();
        describe(Hand::new(h[0], h[1]).unwrap(), &parse_cards(board).unwrap())
    }

    #[test]
    fn hands_in_plain_words() {
        assert_eq!(note("QsJs", "Tc9h6d"), ["open-ended"]);
        assert_eq!(note("AsKs", "Ts9s6d"), ["flush-draw"]);
        assert_eq!(note("QsJs", "Ts9s6d"), ["combo-draw"]);
        assert_eq!(note("AhTh", "Tc9h6d"), ["top-pair"]);
        assert_eq!(note("Ah9s", "Tc9h6d"), ["middle-pair"]);
        assert_eq!(note("Ah6s", "Tc9h6d"), ["bottom-pair"]);
        assert_eq!(note("JhJs", "Tc9h6d"), ["overpair"]);
        assert_eq!(note("8h8s", "Tc9h6d"), ["pocket-pair", "gutshot"]);
        assert_eq!(note("2h2s", "Tc9h6d"), ["underpair"]);
        assert_eq!(note("9d9s", "Tc9h6d"), ["set"]);
        assert_eq!(note("Td9s", "Tc9h6d"), ["two-pair"]);
        assert_eq!(note("Ts5s", "TcTh6d"), ["trips"]);
        assert_eq!(note("AsKd", "TcTh6d"), ["ace-high"]);
        assert_eq!(note("KsQd", "Tc8h6d"), ["overcards"]);
        assert_eq!(note("KsJd", "Tc8h6dQc"), ["open-ended"]);
        assert_eq!(note("QsJd", "Tc8h6dQc"), ["top-pair", "gutshot"]);
        assert_eq!(note("8s7s", "Tc9h6d"), ["straight"]);
        assert_eq!(note("5s4s", "Ac2h7d"), ["gutshot"]);
        assert_eq!(note("5s4s", "Ac2h3d"), ["straight"]);
        assert_eq!(note("KsQs", "Ac9h6d2s8c"), ["high-card"]);
        // The board's own draw is not hero's.
        assert_eq!(note("AsAd", "Tc9h8d7c"), ["overpair"]);
        assert_eq!(note("AsKs", ""), ["suited-ace"]);
        assert_eq!(note("7d7s", ""), ["pocket-pair"]);
        assert_eq!(note("KsQd", ""), ["offsuit-broadways"]);
        assert_eq!(note("8s7s", ""), ["suited-connectors"]);
        assert_eq!(note("9s7s", ""), ["suited-gapper"]);
        assert_eq!(note("9d4s", ""), ["offsuit"]);
    }

    #[test]
    fn straight_outs_by_rank() {
        let bits = |s: &str| {
            parse_cards(s)
                .unwrap()
                .iter()
                .fold(0u16, |m, c| m | 1 << c.rank().index())
        };
        assert_eq!(straight_outs(bits("QsJsTc9h")), 2);
        assert_eq!(straight_outs(bits("AsKsQcJh")), 1);
        assert_eq!(straight_outs(bits("As2s3c4h")), 1);
        assert_eq!(straight_outs(bits("9s7s6c5h3d")), 2);
        assert_eq!(straight_outs(bits("AsKs7c2h")), 0);
    }

    fn trainers() -> Trainers {
        let dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop");
        Trainers::open(dir).unwrap()
    }

    #[test]
    fn an_opening_range() {
        let t = trainers();
        let spot = t.preflop.spot(SpotKind::Rfi, 7).unwrap();
        let hand = spot.hand;
        let i = t.insight(&DrillSpot::Preflop(Box::new(spot))).unwrap();
        assert_eq!(i.cells.len(), HandClass::COUNT);
        assert_eq!(i.class(), HandClass::of(hand));
        // Nobody acted before an open: every class is still in the range,
        // and the range's shares add up.
        assert!(i.cells.iter().all(|c| (c.weight - 1.0).abs() < 1e-6));
        assert!((i.totals.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        for c in &i.cells {
            assert!((c.mix.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
        // Aces open, seven-deuce folds.
        let at = |s: &str| HandClass::all().position(|c| c.to_string() == s).unwrap();
        let fold = i.actions.iter().position(|a| *a == Action::Fold).unwrap();
        assert!(i.cells[at("AA")].mix[fold] < 0.05);
        assert!(i.cells[at("72o")].mix[fold] > 0.95);
        // No bet to call and nobody to be ahead of yet.
        assert_eq!(i.equity, None);
    }

    #[test]
    fn facing_an_open() {
        let t = trainers();
        let spot = t.preflop.spot(SpotKind::VsOpen, 3).unwrap();
        let i = t.insight(&DrillSpot::Preflop(Box::new(spot))).unwrap();
        let e = i.equity.expect("an opener to be up against");
        assert!((0.15..0.9).contains(&e.equity), "{e:?}");
        assert_ne!(e.villain, i.position);
        // Hands the caller never has here are out of its range.
        assert!(i.cells.iter().any(|c| c.in_range()));
        assert!(i.pot_odds.is_some_and(|p| (0.0..0.5).contains(&p)));
    }

    #[test]
    fn a_flop_decision() {
        let t = trainers();
        let pf = t.post().expect("the fixture library");
        let spot = pf.generate(&SpotRequest::default(), 11).unwrap();
        let (hand, board) = (spot.hand, spot.board.clone());
        let i = t.insight(&DrillSpot::Postflop(Box::new(spot))).unwrap();
        assert_eq!(i.hand, hand);
        assert!(
            i.cells[i.class().index()].in_range(),
            "hero's hand is in hero's range"
        );
        assert!((i.totals.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        for c in i.cells.iter().filter(|c| c.in_range()) {
            assert!((0.0..=1.0).contains(&c.weight));
            assert!((c.mix.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
        let e = i.equity.expect("heads-up after the flop");
        assert!((0.0..=1.0).contains(&e.equity));
        assert_eq!(i.note, describe(hand, &board));
    }
}

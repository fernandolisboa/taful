//! Postflop library files (spec §7): spot geometry, ranges from the preflop
//! chart, and navigation of a turn-truncated tree with suit isomorphism.
//!
//! A file stores one canonical flop (spec §7.2). Play happens on a real flop
//! in the same orbit; a [`Cursor`] carries the [`SuitPerm`] that maps real
//! cards to the file's cards, and turn branches stored as
//! [`Branch::Mirror`] compose another permutation onto it.

use taful_core::action::{Action, Line};
use taful_core::cards::{Card, Hand, NUM_COMBOS};
use taful_core::config::{Chips, GameConfig};
use taful_core::engine::{GameState, Phase};
use taful_core::iso::{Flop, SuitPerm};
use taful_core::position::Position;
use taful_core::range::HandClass;

use crate::format::{Branch, Child, Domain, FormatError, LibraryFile, Node};
use crate::preflop::{Lookup, PreflopChart};

/// Bet sizes of a postflop tree in postflop-solver syntax (`"33%, 75%"`,
/// `"3x"`, `"a"`; the same strings for both players), stored as TOML in the
/// header's `bet_tree` so the in-app river solver uses the same river sizes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeSpec {
    /// `[bets, raises]` per street.
    pub flop: [String; 2],
    pub turn: [String; 2],
    pub river: [String; 2],
    /// Add an all-in option when the stack is at most this many times the
    /// pot (postflop-solver's `add_allin_threshold`).
    pub add_allin_threshold: f64,
    /// A bet that would leave less than this share of the pot behind becomes
    /// all-in.
    pub force_allin_threshold: f64,
    /// Sizes closer than this (fraction of the pot) are merged.
    pub merging_threshold: f64,
}

impl TreeSpec {
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("serializable")
    }
    pub fn from_toml(s: &str) -> Result<TreeSpec, String> {
        toml::from_str(s).map_err(|e| e.to_string())
    }
}

/// Pot, stacks and players of a heads-up postflop spot reached by a
/// preflop line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotGeometry {
    /// First to act postflop.
    pub oop: Position,
    pub ip: Position,
    /// All chips in the middle at the flop (blinds, antes, dead money).
    pub pot: Chips,
    /// Effective stack behind at the flop.
    pub stack: Chips,
}

/// Replay `line` on the rules engine. It must close the preflop round with
/// exactly two players left and nobody all-in.
pub fn geometry(config: &GameConfig, line: &Line) -> Result<SpotGeometry, String> {
    let mut s = GameState::new(config.clone(), 0, 0).map_err(|e| format!("{e:?}"))?;
    for &(pos, action) in &line.0 {
        let seat = s
            .to_act()
            .ok_or_else(|| format!("line continues after the round closed at {pos}"))?;
        if s.position(seat) != pos {
            return Err(format!(
                "{pos} acts out of turn (expected {})",
                s.position(seat)
            ));
        }
        s = s
            .apply(action)
            .map_err(|e| format!("{pos} {action}: {e:?}"))?;
    }
    if s.phase() != Phase::NeedsBoard(taful_core::action::Street::Flop) {
        return Err(format!("line does not end preflop: {:?}", s.phase()));
    }
    let live: Vec<usize> = (0..s.players().len())
        .filter(|&i| !s.players()[i].folded)
        .collect();
    let [a, b] = live[..] else {
        return Err(format!("{} players see the flop, need 2", live.len()));
    };
    if s.players()[a].all_in() || s.players()[b].all_in() {
        return Err("a player is all-in preflop".into());
    }
    // Deal any flop that misses both hands to learn who acts first.
    let dead = s.players()[a].hole.mask().union(s.players()[b].hole.mask());
    let flop: Vec<Card> = Card::all().filter(|c| !dead.contains(*c)).take(3).collect();
    let f = s.deal_board(&flop).map_err(|e| format!("{e:?}"))?;
    let first = f.to_act().ok_or("nobody to act on the flop")?;
    let (oop, ip) = if first == a { (a, b) } else { (b, a) };
    Ok(SpotGeometry {
        oop: s.position(oop),
        ip: s.position(ip),
        pot: s.pot(),
        stack: s.players()[a].stack.min(s.players()[b].stack),
    })
}

/// Combo weights (0..=1) of `pos` after playing its part of `line` in the
/// chart: the product of its frequencies along the line.
pub fn line_range(chart: &PreflopChart, line: &Line, pos: Position) -> Result<Vec<f32>, String> {
    let mut reach = [1.0f32; HandClass::COUNT];
    let mut at = Lookup::Node(chart.root());
    for (i, &(p, action)) in line.0.iter().enumerate() {
        let Lookup::Node(id) = at else {
            return Err(format!("line leaves the chart before action {i}"));
        };
        let node = chart.node(id);
        if node.player != p {
            return Err(format!(
                "chart expects {} at action {i}, line has {p}",
                node.player
            ));
        }
        let a = node
            .action_index(action)
            .ok_or_else(|| format!("{p} {action} is not in the chart"))?;
        if p == pos {
            for (c, w) in reach.iter_mut().enumerate() {
                *w *= node.strategy(c).map_or(0.0, |s| s[a]);
            }
        }
        at = chart.step(id, action);
    }
    if at != Lookup::End {
        return Err("line does not close the preflop round in the chart".into());
    }
    let mut out = vec![0.0f32; NUM_COMBOS];
    for h in Hand::all() {
        out[h.combo()] = reach[HandClass::of(h).index()];
    }
    Ok(out)
}

/// A decoded postflop library file.
#[derive(Debug, Clone)]
pub struct PostflopFile {
    file: LibraryFile,
    flop: Flop,
    config: GameConfig,
}

/// A position in a postflop tree, in real-card space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub node: u32,
    /// Maps real cards to the file's cards.
    pub perm: SuitPerm,
}

/// Where an action or a dealt card leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Node(Cursor),
    /// The street closed: deal the next card with [`PostflopFile::deal`].
    Chance(u32, SuitPerm),
    /// The round or hand ended inside the library: a fold, all-in and
    /// called, or the turn closed (the river is solved on demand).
    End,
    /// Left the library: an action outside the tree or a pruned node (D4).
    Left,
}

impl PostflopFile {
    pub fn decode(bytes: &[u8]) -> Result<PostflopFile, FormatError> {
        PostflopFile::new(LibraryFile::decode(bytes)?)
    }

    pub fn new(file: LibraryFile) -> Result<PostflopFile, FormatError> {
        let bad = |m: &str| FormatError::Corrupt(m.into());
        if file.header.domain != Domain::Combos {
            return Err(bad("postflop files have combo rows"));
        }
        let board: [Card; 3] = file
            .header
            .board
            .clone()
            .try_into()
            .map_err(|_| bad("postflop files start on a 3-card flop"))?;
        let flop = Flop::new(board).ok_or_else(|| bad("repeated flop card"))?;
        if !flop.is_canonical() {
            return Err(bad("flop is not canonical"));
        }
        if file.nodes.is_empty() {
            return Err(bad("empty tree"));
        }
        let config = GameConfig::from_toml(&file.header.game_config)
            .map_err(|e| FormatError::Corrupt(format!("game config: {e}")))?;
        if config.key().0 != file.header.config_key {
            return Err(bad("config key does not match the config"));
        }
        file.validate()?;
        Ok(PostflopFile { file, flop, config })
    }

    pub fn file(&self) -> &LibraryFile {
        &self.file
    }
    pub fn config(&self) -> &GameConfig {
        &self.config
    }
    /// The canonical flop the file was solved on.
    pub fn flop(&self) -> Flop {
        self.flop
    }
    pub fn ev_scale(&self) -> f32 {
        self.file.header.ev_scale
    }

    /// The root for play on `real` (any flop in the same orbit).
    pub fn root(&self, real: Flop) -> Option<Cursor> {
        let (canon, perm) = real.canonical();
        (canon == self.flop).then_some(Cursor { node: 0, perm })
    }

    pub fn node(&self, c: &Cursor) -> &Node {
        &self.file.nodes[c.node as usize]
    }

    /// Frequencies for a real hand (renormalized). `None` if not stored.
    pub fn strategy(&self, c: &Cursor, hand: Hand) -> Option<Vec<f32>> {
        self.node(c).strategy(c.perm.hand(hand).combo())
    }

    /// EV of each action for a real hand, in bb. `None` if not stored.
    pub fn ev(&self, c: &Cursor, hand: Hand) -> Option<Vec<f32>> {
        self.node(c)
            .ev(c.perm.hand(hand).combo(), self.file.header.ev_scale)
    }

    pub fn step(&self, c: &Cursor, action: Action) -> Step {
        let node = self.node(c);
        match node.action_index(action).map(|i| node.actions[i].child) {
            Some(Child::Node(n)) => Step::Node(Cursor {
                node: n,
                perm: c.perm,
            }),
            Some(Child::Chance(k)) => Step::Chance(k, c.perm),
            Some(Child::End) => Step::End,
            Some(Child::Pruned) | None => Step::Left,
        }
    }

    /// Deal the real card `card` at chance node `k`.
    pub fn deal(&self, k: u32, perm: SuitPerm, card: Card) -> Step {
        let Some(ch) = self.file.chance.get(k as usize) else {
            return Step::Left;
        };
        match ch.branch(perm.card(card)) {
            Some(Branch::Node(n)) => Step::Node(Cursor { node: n, perm }),
            Some(Branch::Mirror { of, perm: m }) => match ch.branch(of) {
                Some(Branch::Node(n)) => Step::Node(Cursor {
                    node: n,
                    perm: m.after(&perm),
                }),
                _ => Step::Left,
            },
            Some(Branch::Pruned) | None => Step::Left,
        }
    }

    /// Root range of `pos` in real-card space (weights 0..=1).
    pub fn root_range(&self, pos: Position, real_perm: &SuitPerm) -> Option<Vec<f32>> {
        let (_, r) = self
            .file
            .header
            .root_ranges
            .iter()
            .find(|(p, _)| *p == pos)?;
        let table = real_perm.combo_table();
        Some(
            (0..NUM_COMBOS)
                .map(|h| f32::from(r[usize::from(table[h])]) / 255.0)
                .collect(),
        )
    }

    /// Multiply `reach` (real-card space) of the player acting at `c` by the
    /// frequency of action `a`. Combos not stored at the node get 0.
    pub fn apply_action_reach(&self, c: &Cursor, a: usize, reach: &mut [f32]) {
        let node = self.node(c);
        let table = c.perm.combo_table();
        for (h, w) in reach.iter_mut().enumerate() {
            if *w > 0.0 {
                *w *= node.strategy(usize::from(table[h])).map_or(0.0, |s| s[a]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> PreflopChart {
        PreflopChart::decode(include_bytes!(
            "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
        ))
        .unwrap()
    }

    #[test]
    fn srp_geometry_and_ranges() {
        let c = chart();
        let line: Line = "UTG:f,HJ:f,CO:f,BTN:r250,SB:f,BB:c".parse().unwrap();
        let g = geometry(c.config(), &line).unwrap();
        assert_eq!(g.oop, Position::Bb);
        assert_eq!(g.ip, Position::Btn);
        assert_eq!(g.pot, 550);
        assert_eq!(g.stack, 9750);
        let btn = line_range(&c, &line, Position::Btn).unwrap();
        let bb = line_range(&c, &line, Position::Bb).unwrap();
        let aa: Hand = "AsAh".parse().unwrap();
        let s72: Hand = "7s2d".parse().unwrap();
        assert!(btn[aa.combo()] > 0.9, "BTN opens AA");
        assert!(btn[s72.combo()] < 0.05, "BTN folds 72o");
        // BB 3-bets AA most of the time, so it rarely calls.
        assert!(bb[aa.combo()] < 0.5);
        let total = |r: &[f32]| r.iter().sum::<f32>();
        assert!(total(&btn) > 300.0 && total(&btn) < 900.0);
        assert!(total(&bb) > 300.0);
    }

    #[test]
    fn threebet_geometry() {
        let c = chart();
        let line: Line = "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c"
            .parse()
            .unwrap();
        let g = geometry(c.config(), &line).unwrap();
        assert_eq!((g.oop, g.ip), (Position::Sb, Position::Btn));
        assert_eq!(g.pot, 1900 + 100);
        assert_eq!(g.stack, 10_000 - 950);
        assert!(line_range(&c, &line, Position::Sb).is_ok());
        assert!(geometry(c.config(), &"UTG:f,HJ:f".parse().unwrap()).is_err());
    }
}

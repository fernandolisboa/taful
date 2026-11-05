//! Preflop training (spec §10, M3): spot mode and full-hand mode up to the
//! flop, graded against the preflop chart (spec §6, §8).
//!
//! Both modes drive the rules engine; the chart only supplies strategies and
//! EVs. A line the chart does not cover ends the hand (D4). Postflop play
//! arrives with the postflop library (M4), so a hand that reaches a flop
//! without an all-in ends there for now.

use std::sync::Arc;

use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use taful_core::action::{Line, Street};
use taful_core::config::GameConfig;
use taful_core::engine::{EngineError, Phase, Settlement};
use taful_core::eval::RsPokerEvaluator;
use taful_core::grading::{Grade, Thresholds, grade};
use taful_core::range::HandClass;
use taful_solver::format::FormatError;
use taful_solver::preflop::{Lookup, NodeId, PreflopChart};

/// Domain types the UI needs to draw a table (the UI talks only to
/// `taful-app`, spec §4).
pub use taful_core::action::Action;
pub use taful_core::cards::{Card, Hand, Suit};
pub use taful_core::config::{BB_CHIPS, Chips};
pub use taful_core::engine::GameState;
pub use taful_core::position::{Position, Seat};

/// The built-in chart: cash 6-max 100bb, rake 0 (first slice). Produced by
/// `tools/library-build` from `manifests/preflop-6max-100bb.toml`.
const BUILTIN_CHART: &[u8] = include_bytes!(
    "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop.tafl"
);

/// Grading thresholds shipped with the app (spec §8).
const GRADING_TOML: &str = include_str!("../../../config/grading.toml");

/// The button seat in every preflop hand; positions follow from it.
const BUTTON: Seat = 0;

/// Kinds of preflop spot (spec §6, D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpotKind {
    /// Raise first in (everyone before folded); the small blind's open is
    /// [`BlindVsBlind`](Self::BlindVsBlind).
    Rfi,
    /// Facing a single open, no callers.
    VsOpen,
    /// Facing an open and at least one caller.
    Squeeze,
    /// Facing a 3-bet.
    Vs3Bet,
    /// Facing a 4-bet.
    Vs4Bet,
    /// Facing the 5-bet all-in.
    VsAllIn,
    /// Small blind vs big blind after everyone else folded, at any depth.
    BlindVsBlind,
}

impl SpotKind {
    pub const ALL: [SpotKind; 7] = [
        SpotKind::Rfi,
        SpotKind::VsOpen,
        SpotKind::Squeeze,
        SpotKind::Vs3Bet,
        SpotKind::Vs4Bet,
        SpotKind::VsAllIn,
        SpotKind::BlindVsBlind,
    ];

    /// Stable id for i18n keys (`spot-kind-<id>`) and storage.
    pub fn id(self) -> &'static str {
        match self {
            SpotKind::Rfi => "rfi",
            SpotKind::VsOpen => "vs-open",
            SpotKind::Squeeze => "squeeze",
            SpotKind::Vs3Bet => "vs-3bet",
            SpotKind::Vs4Bet => "vs-4bet",
            SpotKind::VsAllIn => "vs-allin",
            SpotKind::BlindVsBlind => "bvb",
        }
    }

    /// Classify the decision at the end of `line` for `actor`.
    pub fn classify(line: &Line, actor: Position) -> SpotKind {
        let acts = &line.0;
        let raises = acts
            .iter()
            .filter(|(_, a)| matches!(a, Action::Raise(_) | Action::AllIn))
            .count();
        let voluntary: Vec<Position> = acts
            .iter()
            .filter(|(_, a)| !matches!(a, Action::Fold | Action::Check))
            .map(|(p, _)| *p)
            .collect();
        let is_blind = |p: &Position| matches!(p, Position::Sb | Position::Bb);
        // Everyone outside the blinds folded and only blinds put chips in.
        let blind_vs_blind = is_blind(&actor)
            && voluntary.iter().all(is_blind)
            && acts.iter().any(|(p, _)| !is_blind(p))
            && acts
                .iter()
                .filter(|(p, _)| !is_blind(p))
                .all(|(_, a)| *a == Action::Fold);
        if blind_vs_blind {
            return SpotKind::BlindVsBlind;
        }
        match raises {
            0 => SpotKind::Rfi,
            1 => {
                let after_open = acts
                    .iter()
                    .skip_while(|(_, a)| !matches!(a, Action::Raise(_) | Action::AllIn))
                    .skip(1);
                if after_open.clone().any(|(_, a)| *a == Action::Call) {
                    SpotKind::Squeeze
                } else {
                    SpotKind::VsOpen
                }
            }
            2 => SpotKind::Vs3Bet,
            3 => SpotKind::Vs4Bet,
            _ => SpotKind::VsAllIn,
        }
    }
}

/// One hero decision with its grade and the explanation payload (spec §8).
#[derive(Debug, Clone, PartialEq)]
pub struct GradedDecision {
    pub node: NodeId,
    pub line: Line,
    pub position: Position,
    pub hand: Hand,
    pub actions: Vec<Action>,
    pub chosen: usize,
    /// Reference frequency of each action for hero's hand.
    pub hand_freqs: Vec<f32>,
    /// EV of each action for hero's hand, bb.
    pub hand_evs: Vec<f32>,
    /// Frequency of each action over the whole range at the node.
    pub range_freqs: Vec<f32>,
    /// Pot before the decision, bb.
    pub pot_bb: f64,
    pub ev_loss_bb: f64,
    /// The spot's solver tolerance (exploitability × pot), bb.
    pub tolerance_bb: f64,
    pub grade: Grade,
    pub thresholds_version: u32,
}

impl GradedDecision {
    /// Index of the best action by EV for hero's hand.
    pub fn best(&self) -> usize {
        (0..self.hand_evs.len())
            .max_by(|&a, &b| self.hand_evs[a].total_cmp(&self.hand_evs[b]))
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreflopError {
    Chart(FormatError),
    Engine(EngineError),
    /// The action is not one of the node's actions (hero buttons only offer
    /// tree sizes, D4).
    NotInTree(Action),
    NotHeroTurn,
    /// The node holds no data for the hand (cannot happen with class rows).
    NoData,
}

impl std::fmt::Display for PreflopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflopError::Chart(e) => write!(f, "preflop chart: {e}"),
            PreflopError::Engine(e) => write!(f, "engine: {e}"),
            PreflopError::NotInTree(a) => write!(f, "action {a} is not in the tree"),
            PreflopError::NotHeroTurn => f.write_str("not hero's turn"),
            PreflopError::NoData => f.write_str("no chart data for this hand"),
        }
    }
}

impl std::error::Error for PreflopError {}

impl From<EngineError> for PreflopError {
    fn from(e: EngineError) -> Self {
        PreflopError::Engine(e)
    }
}

/// Chart + thresholds: grading and lookups shared by both modes.
#[derive(Debug, Clone)]
pub struct PreflopTrainer {
    chart: Arc<PreflopChart>,
    thresholds: Thresholds,
    /// Spot nodes by kind with their weights (how often the line happens).
    spots: Vec<(SpotKind, NodeId, f64)>,
}

impl PreflopTrainer {
    /// The built-in chart with the shipped thresholds.
    pub fn builtin() -> Result<PreflopTrainer, PreflopError> {
        let chart = PreflopChart::decode(BUILTIN_CHART).map_err(PreflopError::Chart)?;
        let thresholds =
            Thresholds::from_toml(GRADING_TOML).expect("shipped thresholds are valid (tested)");
        Ok(PreflopTrainer::new(chart, thresholds))
    }

    pub fn new(chart: PreflopChart, thresholds: Thresholds) -> PreflopTrainer {
        let mut spots = Vec::new();
        for id in 0..chart.len() {
            let line = chart.line(id);
            let actor = chart.node(id).player;
            let kind = SpotKind::classify(&line, actor);
            // Weight: probability that every player plays their part of the
            // line (independent ranges), times the actor's share of combos.
            let mut w = 1.0f64;
            for pos in Position::by_button_offset(chart.config().table_size.into()) {
                let r = chart.reach(id, pos);
                let combos: f64 = (0..HandClass::COUNT)
                    .map(|c| {
                        f64::from(r[c])
                            * HandClass::from_index(c).expect("< 169").num_combos() as f64
                    })
                    .sum();
                w *= combos / 1326.0;
            }
            if w > 0.0 {
                spots.push((kind, id, w));
            }
        }
        PreflopTrainer {
            chart: Arc::new(chart),
            thresholds,
            spots,
        }
    }

    pub fn chart(&self) -> &PreflopChart {
        &self.chart
    }
    pub fn config(&self) -> &GameConfig {
        self.chart.config()
    }
    pub fn thresholds(&self) -> &Thresholds {
        &self.thresholds
    }

    /// Kinds that have at least one spot in the chart.
    pub fn available_kinds(&self) -> Vec<SpotKind> {
        SpotKind::ALL
            .into_iter()
            .filter(|k| self.spots.iter().any(|(s, ..)| s == k))
            .collect()
    }

    /// Grade hero taking `actions[chosen]` at `node` with `hand`.
    pub fn grade(
        &self,
        node: NodeId,
        hand: Hand,
        chosen: usize,
    ) -> Result<GradedDecision, PreflopError> {
        let c = &self.chart;
        let n = c.node(node);
        let freqs = c.strategy(node, hand).ok_or(PreflopError::NoData)?;
        let evs = c.ev(node, hand).ok_or(PreflopError::NoData)?;
        let best_ev = evs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let ev_loss = f64::from(best_ev - evs[chosen]).max(0.0);
        let pot_bb = c.pot_bb(node);
        let tolerance_bb = f64::from(n.exploitability_pct) / 100.0 * pot_bb;
        let range = c.acting_range(node);
        let range_freqs = (0..n.actions.len())
            .map(|a| {
                (0..HandClass::COUNT)
                    .map(|cl| range[cl] * n.strategy(cl).map_or(0.0, |s| s[a]))
                    .sum()
            })
            .collect();
        Ok(GradedDecision {
            node,
            line: c.line(node),
            position: n.player,
            hand,
            actions: n.actions.iter().map(|a| a.action).collect(),
            chosen,
            grade: grade(
                ev_loss,
                pot_bb,
                freqs[chosen],
                tolerance_bb,
                &self.thresholds,
            ),
            hand_freqs: freqs,
            hand_evs: evs,
            range_freqs,
            pot_bb,
            ev_loss_bb: ev_loss,
            tolerance_bb,
            thresholds_version: self.thresholds.version,
        })
    }

    fn seat_of(&self, pos: Position) -> Seat {
        let n = usize::from(self.config().table_size);
        let offset = Position::by_button_offset(n)
            .iter()
            .position(|&p| p == pos)
            .expect("position exists at this table size");
        (BUTTON + offset) % n
    }

    /// A random spot of `kind` (spec §10.1): the node is drawn by how often
    /// its line happens, hero's hand from hero's range there (by reach).
    pub fn spot(&self, kind: SpotKind, seed: u64) -> Result<Spot, PreflopError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let candidates: Vec<&(SpotKind, NodeId, f64)> =
            self.spots.iter().filter(|(k, ..)| *k == kind).collect();
        let &&(_, node, _) = candidates
            .choose_weighted(&mut rng, |(_, _, w)| *w)
            .map_err(|_| PreflopError::NoData)?;
        let n = self.chart.node(node);
        let range = self.chart.acting_range(node);
        let classes: Vec<usize> = (0..HandClass::COUNT).collect();
        let class = *classes
            .choose_weighted(&mut rng, |&c| range[c])
            .map_err(|_| PreflopError::NoData)?;
        let combos = HandClass::from_index(class).expect("< 169").combos();
        let hand = combos[rng.random_range(0..combos.len())];

        let seats = usize::from(self.config().table_size);
        let hero = self.seat_of(n.player);
        let mut holes = vec![None; seats];
        holes[hero] = Some(hand);
        let mut state =
            GameState::new_with_holes(self.config().clone(), BUTTON, rng.random(), &holes)?;
        for &(_, a) in &self.chart.line(node).0 {
            state = state.apply(a)?;
        }
        debug_assert_eq!(state.to_act(), Some(hero));
        Ok(Spot {
            kind,
            node,
            hero,
            hand,
            state,
        })
    }

    /// Hero's options at a node, in tree order.
    pub fn actions(&self, node: NodeId) -> Vec<Action> {
        self.chart
            .node(node)
            .actions
            .iter()
            .map(|a| a.action)
            .collect()
    }
}

/// One spot-mode question.
#[derive(Debug, Clone)]
pub struct Spot {
    pub kind: SpotKind,
    pub node: NodeId,
    pub hero: Seat,
    pub hand: Hand,
    /// Table state at hero's decision.
    pub state: GameState,
}

impl Spot {
    /// Grade hero's answer.
    pub fn answer(
        &self,
        trainer: &PreflopTrainer,
        action: Action,
    ) -> Result<GradedDecision, PreflopError> {
        let chosen = trainer
            .chart()
            .node(self.node)
            .action_index(action)
            .ok_or(PreflopError::NotInTree(action))?;
        trainer.grade(self.node, self.hand, chosen)
    }
}

/// Why a full hand stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum HandEnd {
    /// Everyone but one folded, or an all-in ran out; settled.
    Settled(Settlement),
    /// The flop was reached with chips behind: postflop play comes with M4.
    ReachedFlop,
    /// The line left the chart (D4); it is a candidate for a future build.
    LeftLibrary(Line),
    /// Hero folded; the rest of the hand does not matter for hero.
    HeroFolded,
}

/// A full hand vs reference bots, preflop only (spec §10.2).
#[derive(Debug, Clone)]
pub struct FullHand {
    pub hero: Seat,
    pub state: GameState,
    /// Current chart node, if still inside the chart and on preflop.
    pub node: Option<NodeId>,
    pub decisions: Vec<GradedDecision>,
    pub end: Option<HandEnd>,
    /// Bot actions so far, in order, for the hand log.
    pub log: Vec<(Seat, Action)>,
    rng: ChaCha8Rng,
}

impl FullHand {
    /// Deal a hand with hero at `hero` (`None` = random seat); bots act until
    /// hero's first decision or the end of the hand.
    pub fn new(
        trainer: &PreflopTrainer,
        hero: Option<Seat>,
        seed: u64,
    ) -> Result<FullHand, PreflopError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let seats = usize::from(trainer.config().table_size);
        let hero = hero.unwrap_or_else(|| rng.random_range(0..seats)) % seats;
        let state = GameState::new(trainer.config().clone(), BUTTON, rng.random())?;
        let mut h = FullHand {
            hero,
            state,
            node: Some(trainer.chart().root()),
            decisions: Vec::new(),
            end: None,
            log: Vec::new(),
            rng,
        };
        h.run_bots(trainer)?;
        Ok(h)
    }

    pub fn hero_hand(&self) -> Hand {
        self.state.players()[self.hero].hole
    }

    /// Hero's options now, or empty when it is not hero's turn.
    pub fn hero_actions(&self, trainer: &PreflopTrainer) -> Vec<Action> {
        match (self.end.is_none(), self.state.to_act(), self.node) {
            (true, Some(s), Some(n)) if s == self.hero => trainer.actions(n),
            _ => Vec::new(),
        }
    }

    /// Hero acts; the decision is graded and the bots play on.
    pub fn act(
        &mut self,
        trainer: &PreflopTrainer,
        action: Action,
    ) -> Result<&GradedDecision, PreflopError> {
        let node = match (self.end.is_none(), self.state.to_act(), self.node) {
            (true, Some(s), Some(n)) if s == self.hero => n,
            _ => return Err(PreflopError::NotHeroTurn),
        };
        let chosen = trainer
            .chart()
            .node(node)
            .action_index(action)
            .ok_or(PreflopError::NotInTree(action))?;
        let graded = trainer.grade(node, self.hero_hand(), chosen)?;
        self.decisions.push(graded);
        self.advance(trainer, node, action)?;
        if action == Action::Fold && self.end.is_none() {
            self.end = Some(HandEnd::HeroFolded);
        }
        self.run_bots(trainer)?;
        Ok(self.decisions.last().expect("just pushed"))
    }

    fn advance(
        &mut self,
        trainer: &PreflopTrainer,
        node: NodeId,
        action: Action,
    ) -> Result<(), PreflopError> {
        let seat = self.state.to_act().expect("someone acts");
        self.state = self.state.apply(action)?;
        self.log.push((seat, action));
        match trainer.chart().step(node, action) {
            Lookup::Node(n) => self.node = Some(n),
            Lookup::End => {
                self.node = None;
                self.finish_preflop()?;
            }
            Lookup::Left => {
                self.node = None;
                self.end = Some(HandEnd::LeftLibrary(self.state.history().line()));
            }
        }
        Ok(())
    }

    /// Preflop closed inside the chart: settle folds and all-ins, or stop at
    /// the flop.
    fn finish_preflop(&mut self) -> Result<(), PreflopError> {
        let live: Vec<&taful_core::engine::Player> =
            self.state.players().iter().filter(|p| !p.folded).collect();
        let all_in = live.iter().filter(|p| !p.all_in()).count() <= 1;
        if matches!(self.state.phase(), Phase::Finished) || all_in {
            while let Phase::NeedsBoard(_) = self.state.phase() {
                self.state = self.state.deal_next()?;
            }
            self.end = Some(HandEnd::Settled(self.state.settle(&RsPokerEvaluator)?));
        } else {
            debug_assert_eq!(self.state.phase(), Phase::NeedsBoard(Street::Flop));
            self.end = Some(HandEnd::ReachedFlop);
        }
        Ok(())
    }

    /// Reference bots (spec §9): sample the chart's strategy for their hand.
    fn run_bots(&mut self, trainer: &PreflopTrainer) -> Result<(), PreflopError> {
        while self.end.is_none() {
            let Some(seat) = self.state.to_act() else {
                break;
            };
            if seat == self.hero {
                break;
            }
            let node = self.node.expect("inside the chart while running");
            let hand = self.state.players()[seat].hole;
            let freqs = trainer
                .chart()
                .strategy(node, hand)
                .ok_or(PreflopError::NoData)?;
            let actions = trainer.actions(node);
            let idx: Vec<usize> = (0..actions.len()).collect();
            let a = *idx
                .choose_weighted(&mut self.rng, |&i| freqs[i])
                .map_err(|_| PreflopError::NoData)?;
            self.advance(trainer, node, actions[a])?;
        }
        Ok(())
    }

    /// Total EV lost by hero this hand, bb.
    pub fn total_ev_loss_bb(&self) -> f64 {
        self.decisions.iter().map(|d| d.ev_loss_bb).sum()
    }

    /// Hero's net result in bb once it is known (settled, or hero folded).
    pub fn hero_net_bb(&self) -> Option<f64> {
        match &self.end {
            Some(HandEnd::Settled(s)) => Some(s.net[self.hero] as f64 / BB_CHIPS as f64),
            Some(HandEnd::HeroFolded) => {
                let lost = self.state.players()[self.hero].total_commit;
                Some(-(lost as f64) / BB_CHIPS as f64)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trainer() -> PreflopTrainer {
        PreflopTrainer::builtin().expect("built-in chart decodes")
    }

    #[test]
    fn builtin_chart_matches_first_slice_config() {
        let t = trainer();
        assert_eq!(t.config(), &GameConfig::cash(6, 100));
        assert_eq!(t.thresholds(), &Thresholds::default());
        assert_eq!(t.available_kinds(), SpotKind::ALL.to_vec());
    }

    #[test]
    fn dynamic_fluent_ids_exist() {
        // The table screen builds these ids at run time, so the desktop
        // source lint cannot see them.
        for locale in crate::i18n::Locale::ALL {
            let i18n = crate::i18n::Localizer::new(locale).unwrap();
            for k in SpotKind::ALL {
                assert!(i18n.has(&format!("table-spot-kind-{}", k.id())), "{k:?}");
            }
            for g in Grade::ALL {
                assert!(i18n.has(&format!("table-grade-{}", g.id())), "{g:?}");
            }
        }
    }

    #[test]
    fn classify_lines() {
        let k = |line: &str, actor: &str| {
            SpotKind::classify(&line.parse().unwrap(), actor.parse().unwrap())
        };
        assert_eq!(k("", "UTG"), SpotKind::Rfi);
        assert_eq!(k("UTG:f,HJ:f", "CO"), SpotKind::Rfi);
        assert_eq!(k("UTG:r250", "HJ"), SpotKind::VsOpen);
        assert_eq!(k("UTG:r250,HJ:c", "CO"), SpotKind::Squeeze);
        assert_eq!(
            k("UTG:r250,HJ:r750,CO:f,BTN:f,SB:f,BB:f", "UTG"),
            SpotKind::Vs3Bet
        );
        assert_eq!(k("UTG:f,HJ:f,CO:f,BTN:f", "SB"), SpotKind::BlindVsBlind);
        assert_eq!(
            k("UTG:f,HJ:f,CO:f,BTN:f,SB:r300", "BB"),
            SpotKind::BlindVsBlind
        );
        assert_eq!(k("UTG:f,HJ:f,CO:f,BTN:r250,SB:f", "BB"), SpotKind::VsOpen);
    }

    #[test]
    fn spots_are_hero_decisions_with_hero_in_range() {
        let t = trainer();
        for kind in SpotKind::ALL {
            for seed in 0..20 {
                let s = t.spot(kind, seed).unwrap();
                assert_eq!(s.state.to_act(), Some(s.hero), "{kind:?}");
                assert_eq!(s.state.players()[s.hero].hole, s.hand);
                assert_eq!(s.state.history().line(), t.chart().line(s.node));
                assert_eq!(
                    SpotKind::classify(&t.chart().line(s.node), t.chart().node(s.node).player),
                    kind
                );
                // Hero's hand is in hero's range there.
                let r = t.chart().acting_range(s.node);
                assert!(r[HandClass::of(s.hand).index()] > 0.0);
                // Every option grades, and the best option is Best.
                let acts = t.actions(s.node);
                let g = s.answer(&t, acts[0]).unwrap();
                assert_eq!(s.answer(&t, acts[g.best()]).unwrap().grade, Grade::Best);
            }
        }
    }

    #[test]
    fn spot_is_deterministic_per_seed() {
        let t = trainer();
        let a = t.spot(SpotKind::VsOpen, 42).unwrap();
        let b = t.spot(SpotKind::VsOpen, 42).unwrap();
        assert_eq!((a.node, a.hand, a.state), (b.node, b.hand, b.state));
    }

    #[test]
    fn folding_aces_first_in_is_a_blunder() {
        let t = trainer();
        let root = t.chart().root();
        let aa: Hand = "AsAh".parse().unwrap();
        let fold = t.chart().node(root).action_index(Action::Fold).unwrap();
        let g = t.grade(root, aa, fold).unwrap();
        assert_eq!(g.grade, Grade::Blunder, "{g:?}");
        assert!(g.ev_loss_bb > 1.0);
        let raise = 1 - fold;
        assert_eq!(t.grade(root, aa, raise).unwrap().grade, Grade::Best);
        // 72o open from UTG loses EV and is never played.
        let trash: Hand = "7c2d".parse().unwrap();
        let g = t.grade(root, trash, raise).unwrap();
        assert!(g.grade > Grade::Correct, "{g:?}");
        assert!(g.hand_freqs[raise] < 0.01);
    }

    #[test]
    fn full_hands_play_out_and_grade_every_hero_decision() {
        let t = trainer();
        let mut ends = [0usize; 3];
        for seed in 0..200 {
            let mut h = FullHand::new(&t, None, seed).unwrap();
            let mut hero_turns = 0;
            while h.end.is_none() {
                let acts = h.hero_actions(&t);
                assert!(!acts.is_empty(), "seed {seed}: stuck");
                // Hero follows the chart's most frequent action.
                let freqs = t.chart().strategy(h.node.unwrap(), h.hero_hand()).unwrap();
                let best = (0..acts.len())
                    .max_by(|&a, &b| freqs[a].total_cmp(&freqs[b]))
                    .unwrap();
                h.act(&t, acts[best]).unwrap();
                hero_turns += 1;
            }
            assert_eq!(h.decisions.len(), hero_turns);
            assert!(h.decisions.iter().all(|d| d.grade == Grade::Best));
            match h.end.as_ref().unwrap() {
                HandEnd::Settled(s) => {
                    assert_eq!(s.net.iter().sum::<i64>(), 0);
                    ends[0] += 1;
                }
                HandEnd::ReachedFlop => ends[1] += 1,
                HandEnd::LeftLibrary(_) => ends[2] += 1,
                HandEnd::HeroFolded => {
                    assert!(h.hero_net_bb().unwrap() <= 0.0);
                    ends[0] += 1;
                }
            }
            assert!(h.hero_actions(&t).is_empty());
            assert!(matches!(
                h.act(&t, Action::Fold),
                Err(PreflopError::NotHeroTurn)
            ));
        }
        // Most hands end preflop by folds; some see a flop.
        assert!(ends[0] > 50 && ends[1] > 10, "{ends:?}");
    }

    #[test]
    fn actions_outside_the_tree_are_rejected() {
        let t = trainer();
        let s = t.spot(SpotKind::Rfi, 1).unwrap();
        assert_eq!(
            s.answer(&t, Action::Raise(123)),
            Err(PreflopError::NotInTree(Action::Raise(123)))
        );
    }
}

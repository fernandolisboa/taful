//! Preflop training (spec §10, M3): spot mode and full-hand mode up to the
//! flop, graded against the preflop chart (spec §6, §8).
//!
//! Spot mode drives the rules engine; the chart only supplies strategies and
//! EVs. Full hands (preflop to river) live in [`crate::hand`].

use std::sync::Arc;

use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use taful_core::action::Line;
use taful_core::config::GameConfig;
use taful_core::engine::EngineError;
use taful_core::grading::{Grade, Thresholds, grade};
use taful_core::range::HandClass;
use taful_solver::format::FormatError;
use taful_solver::preflop::{NodeId, PreflopChart};

pub use crate::bots::{Bot, ProfiledBot, ReferenceBot};
pub use taful_core::profile::Profile;

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

/// Best responses against each bot profile (spec §9, D12), built with the
/// chart (`preflop-<id>.tafl`). A test checks that this list matches
/// `content/bots/`.
const BUILTIN_PROFILE_CHARTS: &[(&str, &[u8])] = &[
    (
        "calling-station",
        include_bytes!(
            "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop-calling-station.tafl"
        ),
    ),
    (
        "maniac",
        include_bytes!(
            "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop-maniac.tafl"
        ),
    ),
    (
        "nit",
        include_bytes!(
            "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop-nit.tafl"
        ),
    ),
    (
        "tag",
        include_bytes!(
            "../../../library/v1/c1c6782d353382947a3bbcf1493b6a17155b2e63a7218bf8f54cc38be59cf37f/preflop-tag.tafl"
        ),
    ),
];

/// Grading thresholds shipped with the app (spec §8).
const GRADING_TOML: &str = include_str!("../../../config/grading.toml");

/// The button seat in every preflop spot; positions follow from it.
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
    /// Graded against a bot profile instead of the reference (spec §8, D8):
    /// `hand_evs` are then hero's best-response EVs against the profile, and
    /// this holds the reference EVs and both lines.
    pub vs_profile: Option<VsProfile>,
}

/// The exploitative side of a decision graded against a profile (spec §8
/// explanation payload: the GTO line, the exploitative line, their EV gap).
#[derive(Debug, Clone, PartialEq)]
pub struct VsProfile {
    /// Profile id (`content/bots/<id>.toml`).
    pub profile: String,
    /// Reference (GTO) EV of each action for hero's hand, bb.
    pub reference_evs: Vec<f32>,
    /// The GTO line: the reference's most frequent action for hero's hand.
    pub gto: usize,
    /// The exploitative line: best action by EV against the profile.
    pub exploit: usize,
    /// EV against the profile of the exploitative line minus that of the
    /// GTO line, bb (≥ 0).
    pub ev_gap_bb: f64,
}

impl GradedDecision {
    /// Index of the best action by EV for hero's hand.
    pub fn best(&self) -> usize {
        argmax(&self.hand_evs)
    }
}

/// Index of the largest value (the last one on ties; 0 if empty).
pub fn argmax(v: &[f32]) -> usize {
    (0..v.len())
        .max_by(|&a, &b| v[a].total_cmp(&v[b]))
        .unwrap_or(0)
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
    /// No best-response chart for this profile id.
    UnknownProfile(String),
}

impl std::fmt::Display for PreflopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflopError::Chart(e) => write!(f, "preflop chart: {e}"),
            PreflopError::Engine(e) => write!(f, "engine: {e}"),
            PreflopError::NotInTree(a) => write!(f, "action {a} is not in the tree"),
            PreflopError::NotHeroTurn => f.write_str("not hero's turn"),
            PreflopError::NoData => f.write_str("no chart data for this hand"),
            PreflopError::UnknownProfile(id) => write!(f, "unknown bot profile `{id}`"),
        }
    }
}

impl std::error::Error for PreflopError {}

impl From<EngineError> for PreflopError {
    fn from(e: EngineError) -> Self {
        PreflopError::Engine(e)
    }
}

/// A bot profile with its best-response chart (spec §9): the profile's
/// strategy and hero's best-response EVs against it, on the reference
/// chart's nodes.
#[derive(Debug, Clone)]
pub struct ProfileChart {
    pub bot: ProfiledBot,
    pub chart: Arc<PreflopChart>,
}

/// Chart + thresholds: grading and lookups shared by both modes.
#[derive(Debug, Clone)]
pub struct PreflopTrainer {
    chart: Arc<PreflopChart>,
    thresholds: Thresholds,
    /// Spot nodes by kind with their weights (how often the line happens).
    spots: Vec<(SpotKind, NodeId, f64)>,
    /// Profiles hero can face, by id.
    profiles: Vec<ProfileChart>,
}

impl PreflopTrainer {
    /// The built-in chart with the shipped thresholds and the shipped
    /// profiles (`content/bots/`) with their best-response charts.
    pub fn builtin() -> Result<PreflopTrainer, PreflopError> {
        let chart = PreflopChart::decode(BUILTIN_CHART).map_err(PreflopError::Chart)?;
        let thresholds =
            Thresholds::from_toml(GRADING_TOML).expect("shipped thresholds are valid (tested)");
        let mut t = PreflopTrainer::new(chart, thresholds);
        // Only the bot files: no need to parse the whole content tree. A
        // profile that fails to load is left out (content checks catch it
        // in CI) rather than taking the reference trainer down with it.
        let bots = taful_content::Files::from_pairs(
            crate::content::CONTENT_FILES
                .iter()
                .copied()
                .filter(|(path, _)| path.starts_with("bots/")),
        );
        let (profiles, findings) = taful_content::library::load_profiles(&bots);
        for f in &findings {
            eprintln!("taful: bot profile: {f}");
        }
        for (id, bytes) in BUILTIN_PROFILE_CHARTS {
            let Some(profile) = profiles.get(*id) else {
                eprintln!("taful: bot profile `{id}` missing");
                continue;
            };
            let added = PreflopChart::decode(bytes)
                .map_err(PreflopError::Chart)
                .and_then(|br| t.add_profile(profile.clone(), br));
            if let Err(e) = added {
                eprintln!("taful: bot profile `{id}`: {e}");
            }
        }
        Ok(t)
    }

    /// Adds a profile with its best-response chart, which must sit on this
    /// chart's nodes (same tree, same pruning).
    pub fn add_profile(&mut self, profile: Profile, br: PreflopChart) -> Result<(), PreflopError> {
        let bad = |m: String| PreflopError::Chart(FormatError::Corrupt(m));
        let c = &self.chart;
        if br.len() != c.len() || br.config() != c.config() {
            return Err(bad(format!(
                "profile `{}`: chart does not match the reference",
                profile.id
            )));
        }
        for id in 0..c.len() {
            let (a, b) = (c.node(id), br.node(id));
            if a.player != b.player || a.actions != b.actions {
                return Err(bad(format!(
                    "profile `{}`: node {id} differs from the reference",
                    profile.id
                )));
            }
        }
        self.profiles.retain(|p| p.bot.profile().id != profile.id);
        self.profiles.push(ProfileChart {
            bot: ProfiledBot::new(profile),
            chart: Arc::new(br),
        });
        self.profiles
            .sort_by(|a, b| a.bot.profile().id.cmp(&b.bot.profile().id));
        Ok(())
    }

    /// Profiles hero can face, sorted by id.
    pub fn profiles(&self) -> &[ProfileChart] {
        &self.profiles
    }

    /// The profile with `id`, if loaded.
    pub fn profile(&self, id: &str) -> Option<&ProfileChart> {
        self.profiles.iter().find(|p| p.bot.profile().id == id)
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
            profiles: Vec::new(),
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

    /// Grade hero taking `actions[chosen]` at `node` with `hand`, against
    /// the reference.
    pub fn grade(
        &self,
        node: NodeId,
        hand: Hand,
        chosen: usize,
    ) -> Result<GradedDecision, PreflopError> {
        self.grade_vs(node, hand, chosen, None)
    }

    /// Grade hero's decision against `opponent`: `None` is the reference
    /// (GTO) bot, `Some(id)` a bot profile (spec §8, D8). Against a profile
    /// the EV loss is measured on hero's best-response EVs against it, and
    /// only EV counts: a frequent GTO action that loses EV against the
    /// profile is not `Best` (the 10% frequency rule applies to the
    /// reference, which mixes; the best response does not). The tolerance is
    /// the spot's solver tolerance either way.
    pub fn grade_vs(
        &self,
        node: NodeId,
        hand: Hand,
        chosen: usize,
        opponent: Option<&str>,
    ) -> Result<GradedDecision, PreflopError> {
        let c = &self.chart;
        let n = c.node(node);
        let freqs = c.strategy(node, hand).ok_or(PreflopError::NoData)?;
        let reference_evs = c.ev(node, hand).ok_or(PreflopError::NoData)?;
        let (evs, vs_profile) = match opponent {
            None => (reference_evs, None),
            Some(id) => {
                let p = self
                    .profile(id)
                    .ok_or_else(|| PreflopError::UnknownProfile(id.into()))?;
                let evs = p.chart.ev(node, hand).ok_or(PreflopError::NoData)?;
                // The GTO line is what the reference plays most with this
                // hand (ties by reference EV), not its EV argmax: at mixed
                // nodes the EVs are near-equal by construction.
                let gto = (0..freqs.len())
                    .max_by(|&a, &b| {
                        freqs[a]
                            .total_cmp(&freqs[b])
                            .then(reference_evs[a].total_cmp(&reference_evs[b]))
                    })
                    .unwrap_or(0);
                let exploit = argmax(&evs);
                let vs = VsProfile {
                    profile: id.into(),
                    gto,
                    exploit,
                    ev_gap_bb: f64::from(evs[exploit] - evs[gto]).max(0.0),
                    reference_evs,
                };
                (evs, Some(vs))
            }
        };
        let best_ev = evs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let ev_loss = f64::from(best_ev - evs[chosen]).max(0.0);
        let pot_bb = c.pot_bb(node);
        let tolerance_bb = f64::from(n.exploitability_pct) / 100.0 * pot_bb;
        let graded_freq = if vs_profile.is_some() {
            0.0
        } else {
            freqs[chosen]
        };
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
            grade: grade(ev_loss, pot_bb, graded_freq, tolerance_bb, &self.thresholds),
            hand_freqs: freqs,
            hand_evs: evs,
            range_freqs,
            pot_bb,
            ev_loss_bb: ev_loss,
            tolerance_bb,
            thresholds_version: self.thresholds.version,
            vs_profile,
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
        self.deal_at(kind, node, &mut rng)
    }

    /// A spot at chart `node` (a review of a spot template): hero's hand is
    /// drawn from hero's range there.
    pub fn spot_at(&self, node: NodeId, seed: u64) -> Result<Spot, PreflopError> {
        if node >= self.chart.len() {
            return Err(PreflopError::NoData);
        }
        let kind = SpotKind::classify(&self.chart.line(node), self.chart.node(node).player);
        self.deal_at(kind, node, &mut ChaCha8Rng::seed_from_u64(seed))
    }

    /// The chart node reached by `line`, if any.
    pub fn node_at(&self, line: &Line) -> Option<NodeId> {
        let mut at = self.chart.root();
        for &(p, a) in &line.0 {
            if self.chart.node(at).player != p {
                return None;
            }
            match self.chart.step(at, a) {
                taful_solver::preflop::Lookup::Node(n) => at = n,
                _ => return None,
            }
        }
        Some(at)
    }

    fn deal_at(
        &self,
        kind: SpotKind,
        node: NodeId,
        rng: &mut ChaCha8Rng,
    ) -> Result<Spot, PreflopError> {
        let n = self.chart.node(node);
        let range = self.chart.acting_range(node);
        let classes: Vec<usize> = (0..HandClass::COUNT).collect();
        let class = *classes
            .choose_weighted(rng, |&c| range[c])
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
            opponent: None,
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
    /// Who the other seats are: `None` = the reference bot, `Some(id)` a
    /// bot profile (spec §9; the spot-mode picker).
    pub opponent: Option<String>,
}

impl Spot {
    /// The same spot against `opponent` (`None` = reference).
    pub fn against(mut self, opponent: Option<&str>) -> Spot {
        self.opponent = opponent.map(str::to_owned);
        self
    }

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
        trainer.grade_vs(self.node, self.hand, chosen, self.opponent.as_deref())
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
            let t = trainer();
            let ids = t.profiles().iter().map(|p| p.bot.profile().id.clone());
            for id in std::iter::once(taful_core::profile::REFERENCE.to_string()).chain(ids) {
                assert!(i18n.has(&format!("bot-profile-{id}")), "{id}");
                assert!(i18n.has(&format!("bot-profile-{id}-about")), "{id}");
            }
        }
    }

    #[test]
    fn every_shipped_profile_has_a_best_response_chart() {
        let (lib, findings) = crate::content::library();
        assert!(findings.is_empty(), "{findings:?}");
        let shipped: Vec<&str> = lib.profiles.keys().map(String::as_str).collect();
        let charts: Vec<&str> = BUILTIN_PROFILE_CHARTS.iter().map(|(id, _)| *id).collect();
        assert_eq!(shipped, charts);
        let t = trainer();
        for (id, bytes) in BUILTIN_PROFILE_CHARTS {
            let br = PreflopChart::decode(bytes).unwrap();
            assert_eq!(br.file().header.line, format!("profile:{id}"));
            assert!(t.profile(id).is_some());
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
    fn actions_outside_the_tree_are_rejected() {
        let t = trainer();
        let s = t.spot(SpotKind::Rfi, 1).unwrap();
        assert_eq!(
            s.answer(&t, Action::Raise(123)),
            Err(PreflopError::NotInTree(Action::Raise(123)))
        );
    }
}

#[cfg(test)]
mod demo {
    use super::*;

    /// #25 demo: against a calling station the big blind 3-bets KTs for
    /// value over a BTN open. The reference flats it; against the station
    /// the 3-bet is Best and the payload shows the GTO line next to it.
    #[test]
    fn value_three_bet_against_a_calling_station() {
        let t = PreflopTrainer::builtin().unwrap();
        let node = t
            .node_at(&"UTG:f,HJ:f,CO:f,BTN:r250,SB:f".parse().unwrap())
            .unwrap();
        assert_eq!(
            SpotKind::classify(&t.chart().line(node), Position::Bb),
            SpotKind::VsOpen
        );
        let n = t.chart().node(node);
        let call = n.action_index(Action::Call).unwrap();
        let raise = n
            .actions
            .iter()
            .position(|a| matches!(a.action, Action::Raise(_)))
            .unwrap();
        let hand: Hand = "KcTc".parse().unwrap();
        let three_bet = t
            .grade_vs(node, hand, raise, Some("calling-station"))
            .unwrap();
        assert_eq!(three_bet.grade, Grade::Best, "{three_bet:?}");
        let vs = three_bet.vs_profile.as_ref().unwrap();
        assert_eq!((vs.gto, vs.exploit), (call, raise));
        assert!(vs.ev_gap_bb > 0.1, "{vs:?}");
        assert!(three_bet.hand_freqs[call] > 0.9);
        // The GTO flat loses EV against the station; against the reference
        // it is the right play and the 3-bet is not.
        let flat = t
            .grade_vs(node, hand, call, Some("calling-station"))
            .unwrap();
        assert!(flat.grade > Grade::Best, "{flat:?}");
        assert_eq!(t.grade(node, hand, call).unwrap().grade, Grade::Best);
        assert!(t.grade(node, hand, raise).unwrap().grade > Grade::Best);
        // And the station's bluff-catching flaw: JTs, a GTO 3-bet bluff, is
        // a flat against it.
        let jt: Hand = "JcTc".parse().unwrap();
        let g = t.grade_vs(node, jt, call, Some("calling-station")).unwrap();
        assert_eq!(g.grade, Grade::Best);
        assert_eq!(g.vs_profile.unwrap().gto, raise);
    }
}

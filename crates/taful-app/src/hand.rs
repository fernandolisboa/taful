//! Full-hand mode end to end (spec §10.2, M5): one driver over the rules
//! engine. Preflop decisions come from the chart; when the preflop line
//! reaches a solved configuration the flop is dealt from its solved flops
//! (D13) and play continues on the library tree; the river is solved in the
//! app (spec §7.7). Every hero decision is graded. A line that leaves the
//! reference strategy ends the hand with a reason (D4).

use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use taful_core::action::{ActionHistory, Street};
use taful_core::cards::CardSet;
use taful_core::engine::{EngineError, Phase, Settlement};
use taful_core::eval::RsPokerEvaluator;
use taful_solver::preflop::{Lookup, NodeId};

use crate::postflop::{PostflopError, PostflopTrainer, grade_walk, seat_of};
use crate::preflop::{
    Action, BB_CHIPS, Card, GameState, GradedDecision, Hand, Position, PreflopError,
    PreflopTrainer, Seat, SpotKind,
};
use crate::template::{NodeType, PREFLOP, SpotContext, SpotTemplate, short_config_key};
use crate::walk::{Progress, Walk};

/// The button seat in every hand; positions follow from it.
const BUTTON: Seat = 0;

/// Why a hand left the reference strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftReason {
    /// The preflop line has no solved postflop configuration (yet).
    Unsolved,
    /// The postflop library is not installed.
    NoLibrary,
    /// An action or card outside the solved tree, or a pruned node.
    OffTree,
    /// A player's hand has no data at the node (it was played off the
    /// strategy earlier).
    NotInRange,
}

impl LeftReason {
    /// Stable id (`hands.ended_reason` suffix, i18n `hand-left-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            LeftReason::Unsolved => "unsolved",
            LeftReason::NoLibrary => "no-library",
            LeftReason::OffTree => "off-tree",
            LeftReason::NotInRange => "not-in-range",
        }
    }

    /// A line worth solving in a future build (`solve_candidates`, §14).
    pub fn is_candidate(self) -> bool {
        matches!(self, LeftReason::Unsolved | LeftReason::OffTree)
    }
}

/// Why a full hand stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum HandEnd {
    /// Everyone but one folded, or the hand went to showdown; settled.
    Settled(Settlement),
    /// Hero folded preflop; the rest of the hand does not matter for hero.
    HeroFolded,
    /// The hand left the reference strategy (D4).
    LeftLibrary {
        history: ActionHistory,
        reason: LeftReason,
    },
}

impl HandEnd {
    /// Stable id for `hands.ended_reason`.
    pub fn id(&self) -> String {
        match self {
            HandEnd::Settled(_) => "settled".into(),
            HandEnd::HeroFolded => "hero-folded".into(),
            HandEnd::LeftLibrary { reason, .. } => format!("left-{}", reason.id()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HandError {
    Preflop(PreflopError),
    Postflop(PostflopError),
    Engine(EngineError),
    NotHeroTurn,
    NotInTree(Action),
}

impl std::fmt::Display for HandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandError::Preflop(e) => e.fmt(f),
            HandError::Postflop(e) => e.fmt(f),
            HandError::Engine(e) => write!(f, "engine: {e}"),
            HandError::NotHeroTurn => f.write_str("not hero's turn"),
            HandError::NotInTree(a) => write!(f, "action {a} is not in the tree"),
        }
    }
}

impl std::error::Error for HandError {}

impl From<PreflopError> for HandError {
    fn from(e: PreflopError) -> Self {
        HandError::Preflop(e)
    }
}
impl From<PostflopError> for HandError {
    fn from(e: PostflopError) -> Self {
        HandError::Postflop(e)
    }
}
impl From<EngineError> for HandError {
    fn from(e: EngineError) -> Self {
        HandError::Engine(e)
    }
}

/// One graded hero decision of a hand, with its context.
#[derive(Debug, Clone, PartialEq)]
pub struct HandDecision {
    pub graded: GradedDecision,
    pub context: SpotContext,
    /// The table right before the decision.
    pub state: GameState,
    /// Index in [`Replay::states`] of that table.
    pub step: usize,
}

/// Where the driver is in the reference strategy.
#[derive(Debug, Clone)]
enum Tree {
    Preflop(NodeId),
    Postflop { walk: Box<Walk>, seats: [Seat; 2] },
}

/// A full hand vs reference bots (spec §9, §10.2).
#[derive(Debug, Clone)]
pub struct FullHand {
    pub seed: u64,
    pub hero: Seat,
    pub state: GameState,
    tree: Option<Tree>,
    pub decisions: Vec<HandDecision>,
    pub end: Option<HandEnd>,
    /// Number of engine steps (actions and deals) so far.
    steps: usize,
    config_key: String,
    rng: ChaCha8Rng,
}

impl FullHand {
    /// Deal a hand with hero at `hero` (`None` = random seat); bots act until
    /// hero's first decision or the end of the hand. Without `post` the hand
    /// ends at the flop.
    pub fn new(
        pre: &PreflopTrainer,
        post: Option<&PostflopTrainer>,
        hero: Option<Seat>,
        seed: u64,
    ) -> Result<FullHand, HandError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let seats = usize::from(pre.config().table_size);
        let hero = hero.unwrap_or_else(|| rng.random_range(0..seats)) % seats;
        let state = GameState::new(pre.config().clone(), BUTTON, rng.random())?;
        let mut h = FullHand {
            seed,
            hero,
            state,
            tree: Some(Tree::Preflop(pre.chart().root())),
            decisions: Vec::new(),
            end: None,
            steps: 0,
            config_key: short_config_key(pre.config()),
            rng,
        };
        h.run_bots(pre, post)?;
        Ok(h)
    }

    pub fn hero_hand(&self) -> Hand {
        self.state.players()[self.hero].hole
    }

    fn hero_turn(&self) -> bool {
        self.end.is_none() && self.state.to_act() == Some(self.hero)
    }

    /// Hero's options now, or empty when it is not hero's turn.
    pub fn hero_actions(&self, pre: &PreflopTrainer) -> Vec<Action> {
        if !self.hero_turn() {
            return Vec::new();
        }
        match &self.tree {
            Some(Tree::Preflop(n)) => pre.actions(*n),
            Some(Tree::Postflop { walk, .. }) => walk.node().map(|n| n.actions).unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// The postflop walk, once the hand is on the flop or later.
    pub fn walk(&self) -> Option<&Walk> {
        match &self.tree {
            Some(Tree::Postflop { walk, .. }) => Some(walk),
            _ => None,
        }
    }

    /// Hero acts; the decision is graded and the bots play on.
    pub fn act(
        &mut self,
        pre: &PreflopTrainer,
        post: Option<&PostflopTrainer>,
        action: Action,
    ) -> Result<&HandDecision, HandError> {
        if !self.hero_turn() {
            return Err(HandError::NotHeroTurn);
        }
        let hand = self.hero_hand();
        let position = self.state.position(self.hero);
        let decision = match self.tree.as_ref().ok_or(HandError::NotHeroTurn)? {
            Tree::Preflop(node) => {
                let chosen = pre
                    .chart()
                    .node(*node)
                    .action_index(action)
                    .ok_or(HandError::NotInTree(action))?;
                let graded = pre.grade(*node, hand, chosen)?;
                let line = pre.chart().line(*node);
                let kind = SpotKind::classify(&line, position);
                HandDecision {
                    context: SpotContext {
                        template: SpotTemplate {
                            config: self.config_key.clone(),
                            spot: PREFLOP.into(),
                            line,
                            postflop: Vec::new(),
                            texture: None,
                            hero: position,
                        },
                        street: Street::Preflop,
                        node_type: NodeType::Preflop(kind),
                    },
                    graded,
                    state: self.state.clone(),
                    step: self.steps,
                }
            }
            Tree::Postflop { walk, .. } => {
                let node = walk.node().ok_or(HandError::NotHeroTurn)?;
                let chosen = node
                    .actions
                    .iter()
                    .position(|&a| a == action)
                    .ok_or(HandError::NotInTree(action))?;
                let thresholds = post.map_or_else(|| pre.thresholds(), |p| p.thresholds());
                let graded = grade_walk(walk, hand, chosen, thresholds)?;
                HandDecision {
                    context: postflop_context(&self.config_key, walk, position),
                    graded,
                    state: self.state.clone(),
                    step: self.steps,
                }
            }
        };
        self.decisions.push(decision);
        self.advance(pre, post, action)?;
        if action == Action::Fold && self.end.is_none() {
            self.end = Some(HandEnd::HeroFolded);
        }
        self.run_bots(pre, post)?;
        Ok(self.decisions.last().expect("just pushed"))
    }

    fn leave(&mut self, reason: LeftReason) {
        self.tree = None;
        self.end = Some(HandEnd::LeftLibrary {
            history: self.state.history(),
            reason,
        });
    }

    fn advance(
        &mut self,
        pre: &PreflopTrainer,
        post: Option<&PostflopTrainer>,
        action: Action,
    ) -> Result<(), HandError> {
        self.state = self.state.apply(action)?;
        self.steps += 1;
        match self.tree.take() {
            Some(Tree::Preflop(node)) => match pre.chart().step(node, action) {
                Lookup::Node(n) => self.tree = Some(Tree::Preflop(n)),
                Lookup::End => self.finish_preflop(post)?,
                Lookup::Left => self.leave(LeftReason::OffTree),
            },
            Some(Tree::Postflop { mut walk, seats }) => {
                let progress = walk.act(action)?;
                self.tree = Some(Tree::Postflop { walk, seats });
                self.postflop_progress(progress)?;
            }
            None => {}
        }
        Ok(())
    }

    fn postflop_progress(&mut self, mut progress: Progress) -> Result<(), HandError> {
        loop {
            match progress {
                Progress::Decision => return Ok(()),
                Progress::Over | Progress::Runout => return self.settle(),
                Progress::Left => {
                    self.leave(LeftReason::OffTree);
                    return Ok(());
                }
                Progress::NeedsCard(_) => {
                    let Some(Tree::Postflop { walk, seats }) = &mut self.tree else {
                        return Ok(());
                    };
                    // The engine's own seeded deck picks the card.
                    let card = *self.state.undealt().last().ok_or(EngineError::WrongPhase)?;
                    let pins = seats.map(|s| self.state.players()[s].hole);
                    progress = match walk.deal(card, &pins) {
                        Ok(p) => p,
                        Err(_) => Progress::Left,
                    };
                    if progress != Progress::Left {
                        self.state = self.state.deal_board(&[card])?;
                        self.steps += 1;
                    }
                }
            }
        }
    }

    /// Deal any missing board cards and split the pot.
    fn settle(&mut self) -> Result<(), HandError> {
        while let Phase::NeedsBoard(_) = self.state.phase() {
            self.state = self.state.deal_next()?;
            self.steps += 1;
        }
        self.tree = None;
        self.end = Some(HandEnd::Settled(self.state.settle(&RsPokerEvaluator)?));
        Ok(())
    }

    /// Preflop closed inside the chart: settle folds and all-ins, or go to
    /// the flop of the line's configuration.
    fn finish_preflop(&mut self, post: Option<&PostflopTrainer>) -> Result<(), HandError> {
        let live: Vec<Seat> = (0..self.state.players().len())
            .filter(|&s| !self.state.players()[s].folded)
            .collect();
        let behind = live
            .iter()
            .filter(|&&s| !self.state.players()[s].all_in())
            .count();
        if matches!(self.state.phase(), Phase::Finished) || behind <= 1 {
            return self.settle();
        }
        let Some(post) = post else {
            self.leave(LeftReason::NoLibrary);
            return Ok(());
        };
        let line = self.state.history().line();
        let Some(cfg) = post.spot_for_line(&line) else {
            self.leave(LeftReason::Unsolved);
            return Ok(());
        };
        let dead = self
            .state
            .players()
            .iter()
            .fold(CardSet::EMPTY, |d, p| d.union(p.hole.mask()));
        let Some((file, flop)) = post.deal_flop(cfg, None, dead, &mut self.rng)? else {
            self.leave(LeftReason::OffTree);
            return Ok(());
        };
        let walk = post.walk(cfg, file, flop)?;
        let n = self.state.players().len();
        let pos = walk.positions();
        let seats = [seat_of(n, pos[0]), seat_of(n, pos[1])];
        self.state = self.state.deal_board(&flop.cards())?;
        self.steps += 1;
        self.tree = Some(Tree::Postflop {
            walk: Box::new(walk),
            seats,
        });
        Ok(())
    }

    /// Reference bots (spec §9): sample the reference strategy for their
    /// hand. Stops at hero's turn; ends the hand if hero has no data there.
    fn run_bots(
        &mut self,
        pre: &PreflopTrainer,
        post: Option<&PostflopTrainer>,
    ) -> Result<(), HandError> {
        while self.end.is_none() {
            let Some(seat) = self.state.to_act() else {
                break;
            };
            let hand = self.state.players()[seat].hole;
            let (actions, freqs) = match &self.tree {
                Some(Tree::Preflop(node)) => (
                    pre.actions(*node),
                    Some(
                        pre.chart()
                            .strategy(*node, hand)
                            .ok_or(PreflopError::NoData)?,
                    ),
                ),
                Some(Tree::Postflop { walk, seats }) => {
                    let node = walk.node().ok_or(PostflopError::NoData)?;
                    if seats[node.side] != seat {
                        return Err(PostflopError::Library(
                            "engine and library disagree on who acts".into(),
                        )
                        .into());
                    }
                    (node.actions, walk.strategy(hand))
                }
                None => break,
            };
            let Some(freqs) = freqs else {
                self.leave(LeftReason::NotInRange);
                break;
            };
            if seat == self.hero {
                break;
            }
            let idx: Vec<usize> = (0..actions.len()).collect();
            let a = *idx
                .choose_weighted(&mut self.rng, |&i| freqs[i])
                .map_err(|_| PreflopError::NoData)?;
            self.advance(pre, post, actions[a])?;
        }
        Ok(())
    }

    /// Total EV lost by hero this hand, bb.
    pub fn total_ev_loss_bb(&self) -> f64 {
        self.decisions.iter().map(|d| d.graded.ev_loss_bb).sum()
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

    /// Everything needed to step through the hand again (spec §15.1 #5).
    pub fn replay(&self) -> Replay {
        Replay::of(&self.state)
    }
}

/// Context of a postflop decision at the walk's node for hero at
/// `position`.
pub(crate) fn postflop_context(config_key: &str, walk: &Walk, position: Position) -> SpotContext {
    let node = walk.node();
    SpotContext {
        template: SpotTemplate {
            config: config_key.to_string(),
            spot: walk.spot_id().to_string(),
            line: Default::default(),
            postflop: walk.history().to_vec(),
            texture: Some(walk.texture().id()),
            hero: position,
        },
        street: walk.street(),
        node_type: NodeType::postflop(node.map_or(0, |n| n.facing)),
    }
}

/// A hand as data: hole cards, board and actions (`hands.replay`, JSON).
/// [`Replay::states`] rebuilds every table state on the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub button: Seat,
    /// Hole cards per seat, e.g. `AsKd`.
    pub holes: Vec<String>,
    /// Board cards in deal order, e.g. `Td9d6h2c`.
    pub board: String,
    /// The action history (`BTN:r250,BB:c/BB:x,...`).
    pub history: String,
}

impl Replay {
    pub fn of(state: &GameState) -> Replay {
        Replay {
            button: state.button(),
            holes: state.players().iter().map(|p| p.hole.to_string()).collect(),
            board: state.board().iter().map(Card::to_string).collect(),
            history: state.history().to_string(),
        }
    }

    /// Every table state of the hand, from the deal to the end (one per
    /// action and per dealt street).
    pub fn states(
        &self,
        config: &taful_core::config::GameConfig,
    ) -> Result<Vec<GameState>, String> {
        let holes: Vec<Option<Hand>> = self
            .holes
            .iter()
            .map(|h| h.parse::<Hand>().map(Some).map_err(|e| format!("{e:?}")))
            .collect::<Result<_, _>>()?;
        let board = taful_core::cards::parse_cards(&self.board).map_err(|e| format!("{e:?}"))?;
        let history: ActionHistory = self.history.parse()?;
        let mut s = GameState::new_with_holes(config.clone(), self.button, 0, &holes)
            .map_err(|e| format!("{e:?}"))?;
        let mut out = vec![s.clone()];
        let deal = |s: &GameState| -> Result<Option<GameState>, String> {
            match s.phase() {
                Phase::NeedsBoard(st) if board.len() >= st.board_len() => s
                    .deal_board(&board[s.board().len()..st.board_len()])
                    .map(Some)
                    .map_err(|e| format!("{e:?}")),
                _ => Ok(None),
            }
        };
        for street in &history.streets {
            for &(pos, a) in street {
                if s.to_act().map(|t| s.position(t)) != Some(pos) {
                    return Err(format!("{pos} acts out of turn"));
                }
                s = s.apply(a).map_err(|e| format!("{e:?}"))?;
                out.push(s.clone());
                if let Some(next) = deal(&s)? {
                    s = next;
                    out.push(s.clone());
                }
            }
        }
        // All-in runouts deal without actions.
        while let Some(next) = deal(&s)? {
            s = next;
            out.push(s.clone());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use taful_core::config::GameConfig;
    use taful_core::grading::{Grade, Thresholds};

    use super::*;
    use crate::postflop::LocalDir;

    /// Same table (the replay does not keep the engine's deck seed).
    fn same_table(a: &GameState, b: &GameState) -> bool {
        a.players() == b.players()
            && a.board() == b.board()
            && a.history() == b.history()
            && a.phase() == b.phase()
            && a.pot() == b.pot()
    }

    fn pre() -> PreflopTrainer {
        PreflopTrainer::builtin().unwrap()
    }

    fn post() -> PostflopTrainer {
        PostflopTrainer::open(
            Box::new(LocalDir(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop"),
            )),
            GameConfig::cash(6, 100),
            Thresholds::default(),
        )
        .unwrap()
    }

    /// Hero plays the most frequent action every time.
    fn play_best(h: &mut FullHand, pre: &PreflopTrainer, post: Option<&PostflopTrainer>) {
        while h.end.is_none() {
            let acts = h.hero_actions(pre);
            assert!(!acts.is_empty(), "seed {}: stuck", h.seed);
            let freqs = match &h.tree {
                Some(Tree::Preflop(n)) => pre.chart().strategy(*n, h.hero_hand()).unwrap(),
                Some(Tree::Postflop { walk, .. }) => walk.strategy(h.hero_hand()).unwrap(),
                None => unreachable!(),
            };
            let best = (0..acts.len())
                .max_by(|&a, &b| freqs[a].total_cmp(&freqs[b]))
                .unwrap();
            h.act(pre, post, acts[best]).unwrap();
        }
    }

    #[test]
    fn preflop_only_hands_end_at_the_flop_without_a_library() {
        let pre = pre();
        let mut ends = [0usize; 3];
        for seed in 0..200 {
            let mut h = FullHand::new(&pre, None, None, seed).unwrap();
            play_best(&mut h, &pre, None);
            assert!(h.decisions.iter().all(|d| d.graded.grade == Grade::Best));
            match h.end.as_ref().unwrap() {
                HandEnd::Settled(s) => {
                    assert_eq!(s.net.iter().sum::<i64>(), 0);
                    ends[0] += 1;
                }
                HandEnd::HeroFolded => {
                    assert!(h.hero_net_bb().unwrap() <= 0.0);
                    ends[0] += 1;
                }
                HandEnd::LeftLibrary { reason, .. } => {
                    assert_eq!(*reason, LeftReason::NoLibrary);
                    ends[1] += 1;
                }
            }
            assert!(h.hero_actions(&pre).is_empty());
            assert!(matches!(
                h.act(&pre, None, Action::Fold),
                Err(HandError::NotHeroTurn)
            ));
            // The replay rebuilds the final table.
            let states = h.replay().states(pre.config()).unwrap();
            assert!(same_table(states.last().unwrap(), &h.state));
            for d in &h.decisions {
                assert!(same_table(&states[d.step], &d.state));
                assert_eq!(d.context.street, Street::Preflop);
            }
        }
        assert!(ends[0] > 50 && ends[1] > 10, "{ends:?}");
    }

    /// Seeds whose hands reach the fixture's configuration (BTN open, SB
    /// 3-bet, BTN call) with hero in it.
    fn library_hands(pre: &PreflopTrainer, post: &PostflopTrainer, want: usize) -> Vec<FullHand> {
        let mut out = Vec::new();
        for seed in 0..20_000 {
            let hero = [0, 1][(seed % 2) as usize]; // BTN or SB
            let mut h = FullHand::new(pre, Some(post), Some(hero), seed).unwrap();
            // Steer hero along the fixture line when the chart allows it.
            while h.end.is_none() && h.walk().is_none() {
                let acts = h.hero_actions(pre);
                let line = h.state.history().line().to_string();
                let want_line = "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c";
                let pick = acts
                    .iter()
                    .copied()
                    .find(|a| {
                        let mut l = line.clone();
                        if !l.is_empty() {
                            l.push(',');
                        }
                        l.push_str(&format!("{}:{a}", h.state.position(h.hero)));
                        want_line.starts_with(&l)
                    })
                    .unwrap_or(Action::Fold);
                h.act(pre, Some(post), pick).unwrap();
            }
            if h.walk().is_some() {
                out.push(h);
                if out.len() == want {
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn hands_play_through_the_library_and_the_river() {
        let (pre, post) = (pre(), post());
        let hands = library_hands(&pre, &post, 12);
        assert!(hands.len() >= 8, "found {}", hands.len());
        let mut streets = std::collections::HashSet::new();
        let mut settled = 0;
        for mut h in hands {
            play_best(&mut h, &pre, Some(&post));
            for d in &h.decisions {
                streets.insert(d.context.street);
                assert!(d.graded.ev_loss_bb >= 0.0);
                if d.context.street != Street::Preflop {
                    let t = &d.context.template;
                    assert_eq!(t.spot, "btn-sb-3bp");
                    assert!(t.texture.is_some());
                    assert_eq!(SpotTemplate::parse(&t.key()).as_ref(), Some(t));
                    assert_eq!(t.street(), d.context.street);
                }
            }
            if let Some(HandEnd::Settled(s)) = &h.end {
                assert_eq!(s.net.iter().sum::<i64>(), 0);
                settled += 1;
            }
            let states = h.replay().states(pre.config()).unwrap();
            assert!(
                same_table(states.last().unwrap(), &h.state),
                "seed {}",
                h.seed
            );
            for d in &h.decisions {
                assert!(same_table(&states[d.step], &d.state), "seed {}", h.seed);
            }
        }
        assert!(settled >= 6, "{settled}");
        assert!(streets.contains(&Street::Flop), "{streets:?}");
        assert!(streets.contains(&Street::River), "{streets:?}");
    }

    #[test]
    fn same_seed_same_hand() {
        let (pre, post) = (pre(), post());
        let mut a = library_hands(&pre, &post, 1).remove(0);
        let mut b = FullHand::new(&pre, Some(&post), Some(a.hero), a.seed).unwrap();
        for d in a.decisions.clone() {
            b.act(&pre, Some(&post), d.graded.actions[d.graded.chosen])
                .unwrap();
        }
        play_best(&mut a, &pre, Some(&post));
        play_best(&mut b, &pre, Some(&post));
        assert_eq!(a.state, b.state);
        assert_eq!(a.end, b.end);
    }
}

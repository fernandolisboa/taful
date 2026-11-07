//! A walk through one postflop configuration (spec §7.6, §7.7, §10): the
//! library tree on the flop and turn, then the river solved on demand.
//!
//! A [`Walk`] carries both players' reach (real-card space), the board and
//! the chips committed, so it can grade any decision on the way and cut the
//! river subgame where the library ends. It does not hold a rules engine:
//! spot mode builds the table state once the question is chosen, and the
//! full-hand driver keeps its own engine in step with the walk.

use std::sync::Arc;

use taful_core::action::{Action, Street};
use taful_core::cards::{Card, CardSet, Hand, NUM_COMBOS};
use taful_core::config::{BB_CHIPS, Chips, Rake};
use taful_core::iso::{Flop, SuitPerm, TextureClass};
use taful_core::position::Position;
use taful_core::profile::Profile;
use taful_solver::postflop::{Cursor, PostflopFile, SpotGeometry, Step, TreeSpec};
use taful_solver::river::{
    DcfrRiverSolver, RiverSolution, RiverSpot, RiverTree, SolveOptions, SubgameSolver,
    best_response_detail, profile_strategy,
};

use crate::postflop::PostflopError;

/// Weight given to a pinned hand (a real hole card combo) whose reach fell
/// to zero, so the river solver still returns its strategy and EVs. Small
/// enough not to move the equilibrium in any measurable way.
const PIN_WEIGHT: f32 = 1e-4;

/// Where the walk is in the game tree.
#[derive(Debug, Clone)]
enum Tree {
    Library(Cursor),
    River {
        sol: Arc<RiverSolution>,
        /// Against a profile: the bot's locked river strategy and the
        /// responder's best-response EVs.
        vs: Option<Arc<RiverVs>>,
        node: usize,
    },
}

/// A solved river against a profile (spec §7.8, #27).
#[derive(Debug)]
struct RiverVs {
    /// Per node, the bot's profiled strategy `[action][hand]` (empty where
    /// the other player acts).
    strategy: Vec<Vec<f32>>,
    /// Per node, the responder's EV in chips of each action `[action][hand]`.
    br: Vec<Option<Vec<f32>>>,
}

/// The opponent a walk plays against when it is a bot profile (spec §9,
/// #27): the profile's data in the file and the side it plays.
#[derive(Debug, Clone)]
struct Vs {
    /// Profile section in the file.
    index: usize,
    /// The profile as the build locked it (the file's copy), for rivers.
    profile: Arc<Profile>,
    /// 0 = the bot is out of position.
    bot: usize,
}

/// What happens after an action or a dealt card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Someone acts next, inside the reference strategy.
    Decision,
    /// The street closed: deal the next card with [`Walk::deal`].
    NeedsCard(Street),
    /// The hand is over: a fold, or a showdown after the river (no more
    /// decisions). An all-in that was called is [`Progress::Runout`].
    Over,
    /// All-in and called before the river: run the board out and settle.
    Runout,
    /// Left the reference strategy (an action outside the tree, a pruned
    /// node or turn card).
    Left,
}

/// One decision node as the UI and grading need it.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeView {
    /// 0 = out of position, 1 = in position.
    pub side: usize,
    pub position: Position,
    pub actions: Vec<Action>,
    /// Chips in the middle before the decision.
    pub pot: Chips,
    /// Solver tolerance at this node, in bb (spec §8).
    pub tolerance_bb: f64,
    pub street: Street,
    /// A bet or raise is faced at this node.
    pub facing: usize,
}

/// A postflop walk (see the module docs).
#[derive(Debug, Clone)]
pub struct Walk {
    pub(crate) spot_id: String,
    file: Arc<PostflopFile>,
    rake: Rake,
    geometry: SpotGeometry,
    tree_spec: Option<TreeSpec>,
    tree: Tree,
    pending: Option<Pending>,
    /// Against a bot profile; `None` = the reference.
    vs: Option<Vs>,
    /// Reach of OOP and IP, real-card space (the bot's under its profile).
    reach: [Vec<f32>; 2],
    board: Vec<Card>,
    /// Chips each side put in on earlier postflop streets.
    committed: [Chips; 2],
    /// Chips each side put in on this street.
    street_commit: [Chips; 2],
    /// Bets and raises on this street.
    aggressions: usize,
    street: Street,
    /// Postflop actions per street (flop first), by position.
    history: Vec<Vec<(Position, Action)>>,
}

#[derive(Debug, Clone, Copy)]
enum Pending {
    Turn(u32, SuitPerm),
    River,
}

impl Walk {
    /// Start at the root of `file` played on `real` (a flop in the file's
    /// orbit).
    pub(crate) fn start(
        spot_id: &str,
        file: Arc<PostflopFile>,
        geometry: SpotGeometry,
        real: Flop,
    ) -> Result<Walk, PostflopError> {
        let cursor = file.root(real).ok_or(PostflopError::NoData)?;
        let pos = [geometry.oop, geometry.ip];
        let range = |p: Position| {
            file.root_range(p, &cursor.perm)
                .ok_or(PostflopError::NoData)
        };
        let reach = [range(pos[0])?, range(pos[1])?];
        let tree_spec = TreeSpec::from_toml(&file.file().header.bet_tree).ok();
        Ok(Walk {
            spot_id: spot_id.to_string(),
            rake: file.config().rake,
            file,
            geometry,
            tree_spec,
            tree: Tree::Library(cursor),
            pending: None,
            vs: None,
            reach,
            board: real.cards().to_vec(),
            committed: [0, 0],
            street_commit: [0, 0],
            aggressions: 0,
            street: Street::Flop,
            history: vec![Vec::new()],
        })
    }

    /// The same walk against bot profile `id` playing side `bot` (0 = out
    /// of position), before any action. `Ok(None)` when the file has no
    /// data for the profile (a reference-only library, #27).
    pub(crate) fn against(mut self, id: &str, bot: usize) -> Result<Option<Walk>, PostflopError> {
        if !self.history.iter().all(Vec::is_empty) || bot > 1 {
            return Err(PostflopError::NoData);
        }
        let Some(index) = self.file.profile_index(id) else {
            return Ok(None);
        };
        let profile = Profile::from_toml(&self.file.profile(index).profile)
            .map_err(|e| PostflopError::Library(format!("profile `{id}` in the file: {e}")))?;
        // The bot starts from the profile's own preflop range along the line
        // (#48; the reference range in a file without one), and follows the
        // profile from here on.
        let Tree::Library(cursor) = &self.tree else {
            return Err(PostflopError::NoData);
        };
        let pos = [self.geometry.oop, self.geometry.ip][bot];
        self.reach[bot] = self
            .file
            .profile_root_range(index, pos, &cursor.perm)
            .ok_or(PostflopError::NoData)?;
        self.vs = Some(Vs {
            index,
            profile: Arc::new(profile),
            bot,
        });
        Ok(Some(self))
    }

    /// The bot profile this walk plays against, if any.
    pub fn vs_profile(&self) -> Option<&str> {
        self.vs
            .as_ref()
            .map(|v| self.file.profile(v.index).id.as_str())
    }

    /// The side the profile plays, if any (0 = out of position).
    pub fn bot_side(&self) -> Option<usize> {
        self.vs.as_ref().map(|v| v.bot)
    }

    pub fn spot_id(&self) -> &str {
        &self.spot_id
    }
    pub fn board(&self) -> &[Card] {
        &self.board
    }
    pub fn street(&self) -> Street {
        self.street
    }
    /// The flop's texture class (the board family of spot templates).
    pub fn texture(&self) -> TextureClass {
        let flop: [Card; 3] = self.board[..3].try_into().expect("a flop");
        Flop::new(flop).expect("distinct").texture().class()
    }
    pub fn positions(&self) -> [Position; 2] {
        [self.geometry.oop, self.geometry.ip]
    }
    /// Postflop actions so far, one list per street from the flop.
    pub fn history(&self) -> &[Vec<(Position, Action)>] {
        &self.history
    }
    /// All postflop actions, in order.
    pub fn actions(&self) -> impl Iterator<Item = (Position, Action)> + '_ {
        self.history.iter().flatten().copied()
    }
    pub fn reach(&self, side: usize) -> &[f32] {
        &self.reach[side]
    }
    /// The river is being played on a solve made in the app.
    pub fn on_solved_river(&self) -> bool {
        matches!(self.tree, Tree::River { .. })
    }

    fn pot(&self) -> Chips {
        self.geometry.pot
            + self.committed.iter().sum::<Chips>()
            + self.street_commit.iter().sum::<Chips>()
    }

    /// The decision node, unless a card is pending.
    pub fn node(&self) -> Option<NodeView> {
        if self.pending.is_some() {
            return None;
        }
        let pos = self.positions();
        let facing = self.aggressions;
        Some(match &self.tree {
            Tree::Library(c) => {
                let n = self.file.node(c);
                let side = usize::from(n.player == pos[1]);
                NodeView {
                    side,
                    position: n.player,
                    actions: n.actions.iter().map(|a| a.action).collect(),
                    pot: n.pot,
                    tolerance_bb: f64::from(n.exploitability_pct) / 100.0 * n.pot as f64
                        / BB_CHIPS as f64,
                    street: self.street,
                    facing,
                }
            }
            Tree::River { sol, node, .. } => {
                let n = &sol.nodes[*node];
                NodeView {
                    side: n.player,
                    position: pos[n.player],
                    actions: n.actions.clone(),
                    pot: self.pot(),
                    tolerance_bb: f64::from(sol.exploitability) / BB_CHIPS as f64,
                    street: Street::River,
                    facing,
                }
            }
        })
    }

    /// Frequencies of the acting player's `hand` at the node; `None` when the
    /// hand has no data there.
    pub fn strategy(&self, hand: Hand) -> Option<Vec<f32>> {
        if self.pending.is_some() {
            return None;
        }
        match &self.tree {
            Tree::Library(c) => self.file.strategy(c, hand),
            Tree::River { sol, node, .. } => sol.strategy(*node, hand),
        }
    }

    /// Frequencies the acting player plays with `hand`: the profile's
    /// strategy when the actor is the bot of a walk against a profile (the
    /// file's on the flop and turn, the locked river strategy on the river),
    /// the reference otherwise. `None` when the hand has no data there.
    pub fn actor_strategy(&self, hand: Hand) -> Option<Vec<f32>> {
        let node = self.node()?;
        let Some(vs) = self.vs.as_ref().filter(|v| v.bot == node.side) else {
            return self.strategy(hand);
        };
        match &self.tree {
            Tree::Library(c) => self.file.profile_strategy(c, vs.index, hand),
            Tree::River { sol, node, vs: rv } => {
                let table = &rv.as_ref()?.strategy[*node];
                let n = &sol.nodes[*node];
                let i = sol.hand_index(n.player, hand)?;
                let len = sol.hands[n.player].len();
                let row: Vec<f32> = (0..n.actions.len()).map(|a| table[a * len + i]).collect();
                let total: f32 = row.iter().sum();
                (total > 0.0).then(|| row.into_iter().map(|x| x / total).collect())
            }
        }
    }

    /// EV of each action for the acting player's `hand`, in bb.
    pub fn ev_bb(&self, hand: Hand) -> Option<Vec<f32>> {
        if self.pending.is_some() {
            return None;
        }
        match &self.tree {
            Tree::Library(c) => self.file.ev(c, hand),
            Tree::River { sol, node, .. } => sol
                .ev(*node, hand)
                .map(|v| v.into_iter().map(|chips| chips / BB_CHIPS as f32).collect()),
        }
    }

    /// Against a profile: the acting player's best-response EV of each
    /// action against it for `hand`, in bb (from the file on the flop and
    /// turn, from the river solve on the river). `None` without a profile,
    /// when the bot acts, or when the hand has no data.
    pub fn profile_ev_bb(&self, hand: Hand) -> Option<Vec<f32>> {
        let vs = self.vs.as_ref()?;
        if self.pending.is_some() || self.node()?.side == vs.bot {
            return None;
        }
        match &self.tree {
            Tree::Library(c) => self.file.profile_ev(c, vs.index, hand),
            Tree::River { sol, node, vs: rv } => {
                let br = rv.as_ref()?.br[*node].as_ref()?;
                let n = &sol.nodes[*node];
                let i = sol.hand_index(n.player, hand)?;
                let len = sol.hands[n.player].len();
                Some(
                    (0..n.actions.len())
                        .map(|a| br[a * len + i] / BB_CHIPS as f32)
                        .collect(),
                )
            }
        }
    }

    /// Multiply `reach` (real-card space) by the actor's frequency of action
    /// `a` at the node; the actor is `side`, and plays its profile if it is
    /// the bot.
    fn apply_reach(&self, side: usize, a: usize, reach: &mut [f32]) {
        let bot = self.vs.as_ref().filter(|v| v.bot == side);
        match &self.tree {
            Tree::Library(c) => match bot {
                Some(v) => self.file.apply_profile_reach(c, v.index, a, reach),
                None => self.file.apply_action_reach(c, a, reach),
            },
            Tree::River { sol, node, vs } => {
                let n = &sol.nodes[*node];
                let len = sol.hands[n.player].len();
                let table = match (bot, vs) {
                    (Some(_), Some(rv)) => &rv.strategy[*node],
                    _ => &n.strategy,
                };
                let mut next = vec![0.0f32; NUM_COMBOS];
                for (i, h) in sol.hands[n.player].iter().enumerate() {
                    next[h.combo()] = reach[h.combo()] * table[a * len + i];
                }
                reach.copy_from_slice(&next);
            }
        }
    }

    /// Share of the actor's range (by reach) taking each action.
    pub fn range_freqs(&self) -> Vec<f32> {
        let Some(node) = self.node() else {
            return Vec::new();
        };
        let w: Vec<f64> = (0..node.actions.len())
            .map(|a| self.action_weight(a))
            .collect();
        let total: f64 = w.iter().sum();
        w.iter()
            .map(|&x| if total > 0.0 { (x / total) as f32 } else { 0.0 })
            .collect()
    }

    /// Reach mass of the actor's range (board-dead combos excluded) that
    /// would take `a`, without moving.
    pub fn action_weight(&self, a: usize) -> f64 {
        let Some(node) = self.node() else { return 0.0 };
        let dead = CardSet::from_cards(&self.board);
        let mut r = self.reach[node.side].clone();
        self.apply_reach(node.side, a, &mut r);
        r.iter()
            .enumerate()
            .filter(|(h, _)| !Hand::from_combo(*h).expect("<1326").mask().intersects(dead))
            .map(|(_, &w)| f64::from(w))
            .sum()
    }

    /// Where `action` would lead, without moving (no river solve).
    pub fn peek(&self, action: Action) -> Progress {
        let Some(node) = self.node() else {
            return Progress::Left;
        };
        if node.actions.iter().all(|&a| a != action) {
            return Progress::Left;
        }
        let mut probe = self.clone();
        probe.tree_spec = None; // never solve while peeking
        probe.act(action).unwrap_or(Progress::Left)
    }

    /// The acting player takes `action`.
    pub fn act(&mut self, action: Action) -> Result<Progress, PostflopError> {
        let node = self.node().ok_or(PostflopError::NoData)?;
        let a = node
            .actions
            .iter()
            .position(|&x| x == action)
            .ok_or(PostflopError::NotInTree(action))?;
        let side = node.side;
        let mut r = std::mem::take(&mut self.reach[side]);
        self.apply_reach(side, a, &mut r);
        self.reach[side] = r;
        self.history
            .last_mut()
            .expect("a street")
            .push((node.position, action));
        self.commit(side, action);
        let closes_street = match action {
            Action::Fold => return Ok(self.finish(Progress::Over)),
            Action::Check => side == 1,
            Action::Call => true,
            Action::Bet(_) | Action::Raise(_) | Action::AllIn => false,
        };
        match self.tree.clone() {
            Tree::Library(c) => match self.file.step(&c, action) {
                Step::Node(next) => {
                    self.tree = Tree::Library(next);
                    Ok(Progress::Decision)
                }
                Step::Chance(k, perm) => {
                    self.pending = Some(Pending::Turn(k, perm));
                    self.next_street();
                    Ok(Progress::NeedsCard(Street::Turn))
                }
                Step::End => {
                    if self.all_in() {
                        Ok(self.finish(Progress::Runout))
                    } else if closes_street && self.street == Street::Turn {
                        self.pending = Some(Pending::River);
                        self.next_street();
                        Ok(Progress::NeedsCard(Street::River))
                    } else {
                        // The tree ended where the rules did not (cannot
                        // happen with files from the build tool).
                        Ok(self.finish(Progress::Left))
                    }
                }
                Step::Left => Ok(self.finish(Progress::Left)),
            },
            Tree::River { sol, node, vs } => match sol.nodes[node].children[a] {
                Some(next) => {
                    self.tree = Tree::River {
                        sol,
                        vs,
                        node: next,
                    };
                    Ok(Progress::Decision)
                }
                None => Ok(self.finish(Progress::Over)),
            },
        }
    }

    fn finish(&mut self, p: Progress) -> Progress {
        self.pending = None;
        p
    }

    fn all_in(&self) -> bool {
        let behind = self.geometry.stack;
        (0..2).any(|s| self.committed[s] + self.street_commit[s] >= behind)
    }

    fn commit(&mut self, side: usize, action: Action) {
        let behind = self.geometry.stack - self.committed[side];
        match action {
            Action::Fold | Action::Check => {}
            Action::Call => {
                self.street_commit[side] = self.street_commit[side ^ 1].min(behind);
            }
            Action::Bet(to) | Action::Raise(to) => {
                self.street_commit[side] = to.min(behind);
                self.aggressions += 1;
            }
            Action::AllIn => {
                self.street_commit[side] = behind;
                self.aggressions += 1;
            }
        }
    }

    fn next_street(&mut self) {
        for s in 0..2 {
            self.committed[s] += self.street_commit[s];
            self.street_commit[s] = 0;
        }
        self.aggressions = 0;
        self.street = self.street.next().unwrap_or(Street::River);
        self.history.push(Vec::new());
    }

    /// Deal the pending street's card. `pins` are real hands (hole cards)
    /// that must keep data on a river solve even if their reach is zero.
    pub fn deal(&mut self, card: Card, pins: &[Hand]) -> Result<Progress, PostflopError> {
        if self.board.contains(&card) {
            return Err(PostflopError::Library(format!("{card} is on the board")));
        }
        match self.pending.take() {
            None => Err(PostflopError::NoData),
            Some(Pending::Turn(k, perm)) => match self.file.deal(k, perm, card) {
                Step::Node(c) => {
                    self.board.push(card);
                    self.tree = Tree::Library(c);
                    Ok(Progress::Decision)
                }
                _ => Ok(Progress::Left),
            },
            Some(Pending::River) => {
                self.board.push(card);
                match self.solve_river(pins) {
                    Ok((sol, vs)) => {
                        self.tree = Tree::River {
                            sol: Arc::new(sol),
                            vs: vs.map(Arc::new),
                            node: 0,
                        };
                        Ok(Progress::Decision)
                    }
                    Err(e) => {
                        self.board.pop();
                        Err(e)
                    }
                }
            }
        }
    }

    /// The river subgame at this point (spec §7.7).
    pub fn river_spot(&self, pins: &[Hand]) -> Result<RiverSpot, PostflopError> {
        let spec = self
            .tree_spec
            .as_ref()
            .ok_or_else(|| PostflopError::Library("file has no river sizes".into()))?;
        let tree = RiverTree::from_spec(spec).map_err(PostflopError::Library)?;
        let board: [Card; 5] = self
            .board
            .clone()
            .try_into()
            .map_err(|_| PostflopError::Library("river needs 5 cards".into()))?;
        let dead = CardSet::from_cards(&board);
        let mut ranges = self.reach.clone();
        for r in &mut ranges {
            for (h, w) in r.iter_mut().enumerate() {
                if Hand::from_combo(h).expect("<1326").mask().intersects(dead) {
                    *w = 0.0;
                }
            }
        }
        // Pins: give each pinned hand a tiny weight in the range of the side
        // that holds it (`pins` is `[oop hand, ip hand]`, or empty).
        for (side, p) in pins.iter().enumerate().take(2) {
            let w = &mut ranges[side][p.combo()];
            if !p.mask().intersects(dead) && *w < PIN_WEIGHT {
                *w = PIN_WEIGHT;
            }
        }
        Ok(RiverSpot {
            board,
            ranges,
            pot: self.pot(),
            stack: self.geometry.stack - self.committed[0].max(self.committed[1]),
            tree,
            rake: self.rake,
        })
    }

    /// The river solve, and against a profile its locked strategy and the
    /// other side's best responses (spec §7.8: rivers on demand).
    fn solve_river(
        &self,
        pins: &[Hand],
    ) -> Result<(RiverSolution, Option<RiverVs>), PostflopError> {
        let spot = self.river_spot(pins)?;
        let sol = DcfrRiverSolver
            .solve(&spot, &river_options())
            .map_err(PostflopError::Library)?;
        let vs = match &self.vs {
            None => None,
            Some(v) => {
                let strategy = profile_strategy(&spot, &sol, v.bot, &v.profile);
                let br = best_response_detail(&spot, &sol, v.bot ^ 1, &strategy)
                    .map_err(PostflopError::Library)?;
                Some(RiverVs { strategy, br })
            }
        };
        Ok((sol, vs))
    }
}

/// River solve settings in the app: spec §7.7 defaults, or a quick coarse
/// solve in debug builds (tests).
fn river_options() -> SolveOptions {
    if cfg!(debug_assertions) {
        SolveOptions {
            target_pct: 2.0,
            max_iterations: 60,
            check_every: 20,
        }
    } else {
        SolveOptions::default()
    }
}

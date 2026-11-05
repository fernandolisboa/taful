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
use taful_solver::postflop::{Cursor, PostflopFile, SpotGeometry, Step, TreeSpec};
use taful_solver::river::{
    DcfrRiverSolver, RiverSolution, RiverSpot, RiverTree, SolveOptions, SubgameSolver,
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
        node: usize,
    },
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
    /// Reach of OOP and IP, real-card space.
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
            reach,
            board: real.cards().to_vec(),
            committed: [0, 0],
            street_commit: [0, 0],
            aggressions: 0,
            street: Street::Flop,
            history: vec![Vec::new()],
        })
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
            Tree::River { sol, node } => {
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
            Tree::River { sol, node } => sol.strategy(*node, hand),
        }
    }

    /// EV of each action for the acting player's `hand`, in bb.
    pub fn ev_bb(&self, hand: Hand) -> Option<Vec<f32>> {
        if self.pending.is_some() {
            return None;
        }
        match &self.tree {
            Tree::Library(c) => self.file.ev(c, hand),
            Tree::River { sol, node } => sol
                .ev(*node, hand)
                .map(|v| v.into_iter().map(|chips| chips / BB_CHIPS as f32).collect()),
        }
    }

    /// Multiply `reach` (real-card space) by the actor's frequency of action
    /// `a` at the node.
    fn apply_reach(&self, a: usize, reach: &mut [f32]) {
        match &self.tree {
            Tree::Library(c) => self.file.apply_action_reach(c, a, reach),
            Tree::River { sol, node } => {
                let n = &sol.nodes[*node];
                let len = sol.hands[n.player].len();
                let mut next = vec![0.0f32; NUM_COMBOS];
                for (i, h) in sol.hands[n.player].iter().enumerate() {
                    next[h.combo()] = reach[h.combo()] * n.strategy[a * len + i];
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
        let dead = CardSet::from_cards(&self.board);
        let live = |r: &[f32]| -> f64 {
            (0..NUM_COMBOS)
                .filter(|&h| !Hand::from_combo(h).expect("<1326").mask().intersects(dead))
                .map(|h| f64::from(r[h]))
                .sum()
        };
        let total = live(&self.reach[node.side]);
        (0..node.actions.len())
            .map(|a| {
                let mut r = self.reach[node.side].clone();
                self.apply_reach(a, &mut r);
                if total > 0.0 {
                    (live(&r) / total) as f32
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Share of the actor's range (by reach, board-dead combos excluded)
    /// that would take `a`, without moving.
    pub fn action_weight(&self, a: usize) -> f64 {
        let Some(node) = self.node() else { return 0.0 };
        let dead = CardSet::from_cards(&self.board);
        let mut r = self.reach[node.side].clone();
        self.apply_reach(a, &mut r);
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
        self.apply_reach(a, &mut r);
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
            Tree::River { sol, node } => match sol.nodes[node].children[a] {
                Some(next) => {
                    self.tree = Tree::River { sol, node: next };
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
                    Ok(sol) => {
                        self.tree = Tree::River {
                            sol: Arc::new(sol),
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

    fn solve_river(&self, pins: &[Hand]) -> Result<RiverSolution, PostflopError> {
        let spot = self.river_spot(pins)?;
        DcfrRiverSolver
            .solve(&spot, &river_options())
            .map_err(PostflopError::Library)
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

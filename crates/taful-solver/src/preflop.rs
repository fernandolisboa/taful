//! Preflop charts (spec §6): a library file over the preflop pseudo-board with
//! one row per hand class, plus navigation by [`Line`].

use taful_core::action::{Action, Line};
use taful_core::cards::Hand;
use taful_core::config::{BB_CHIPS, GameConfig};
use taful_core::position::Position;
use taful_core::range::HandClass;

use crate::format::{Child, Domain, FormatError, LibraryFile, Node};

/// Index of a node in the chart.
pub type NodeId = usize;

/// Where a line ends up in the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    /// A stored decision node.
    Node(NodeId),
    /// The line closed the preflop round (or the hand) inside the chart.
    End,
    /// The line left the chart: an action outside the tree, or a node pruned
    /// for low reach (D4).
    Left,
}

/// A decoded preflop chart.
#[derive(Debug, Clone)]
pub struct PreflopChart {
    file: LibraryFile,
    config: GameConfig,
    /// `(parent, action index)` per node; root has none.
    parents: Vec<Option<(NodeId, usize)>>,
}

impl PreflopChart {
    pub fn decode(bytes: &[u8]) -> Result<PreflopChart, FormatError> {
        PreflopChart::new(LibraryFile::decode(bytes)?)
    }

    pub fn new(file: LibraryFile) -> Result<PreflopChart, FormatError> {
        let bad = |m: &str| FormatError::Corrupt(m.into());
        if file.header.domain != Domain::Classes || !file.header.board.is_empty() {
            return Err(bad("not a preflop chart (needs class rows and no board)"));
        }
        if file.nodes.is_empty() {
            return Err(bad("empty chart"));
        }
        let config = GameConfig::from_toml(&file.header.game_config)
            .map_err(|e| FormatError::Corrupt(format!("game config: {e}")))?;
        if config.key().0 != file.header.config_key {
            return Err(bad("config key does not match the config"));
        }
        let positions = Position::by_button_offset(usize::from(config.table_size));
        if file.nodes.iter().any(|n| !positions.contains(&n.player)) {
            return Err(bad("node player not seated at this table size"));
        }
        let mut parents = vec![None; file.nodes.len()];
        for (i, n) in file.nodes.iter().enumerate() {
            for (a, na) in n.actions.iter().enumerate() {
                if let Child::Node(c) = na.child {
                    if parents[c as usize].is_some() {
                        return Err(bad("node with two parents"));
                    }
                    parents[c as usize] = Some((i, a));
                }
            }
        }
        if parents.iter().skip(1).any(Option::is_none) {
            return Err(bad("unreachable node"));
        }
        Ok(PreflopChart {
            file,
            config,
            parents,
        })
    }

    pub fn file(&self) -> &LibraryFile {
        &self.file
    }
    pub fn config(&self) -> &GameConfig {
        &self.config
    }
    pub fn len(&self) -> usize {
        self.file.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.file.nodes.is_empty()
    }
    pub fn node(&self, id: NodeId) -> &Node {
        &self.file.nodes[id]
    }
    pub fn root(&self) -> NodeId {
        0
    }
    pub fn parent(&self, id: NodeId) -> Option<(NodeId, usize)> {
        self.parents[id]
    }

    /// Where taking `action` at `id` leads.
    pub fn step(&self, id: NodeId, action: Action) -> Lookup {
        let node = self.node(id);
        match node.action_index(action).map(|i| node.actions[i].child) {
            Some(Child::Node(c)) => Lookup::Node(c as usize),
            Some(Child::End) => Lookup::End,
            Some(Child::Pruned) | None => Lookup::Left,
        }
    }

    /// Follow a preflop line from the root.
    pub fn find(&self, line: &Line) -> Lookup {
        let mut at = Lookup::Node(self.root());
        for &(pos, action) in &line.0 {
            let Lookup::Node(id) = at else {
                return Lookup::Left;
            };
            if self.node(id).player != pos {
                return Lookup::Left;
            }
            at = self.step(id, action);
        }
        at
    }

    /// The line from the root to `id`.
    pub fn line(&self, id: NodeId) -> Line {
        let mut out = Vec::new();
        let mut at = id;
        while let Some((p, a)) = self.parents[at] {
            let n = self.node(p);
            out.push((n.player, n.actions[a].action));
            at = p;
        }
        out.reverse();
        Line(out)
    }

    /// Frequencies at `id` for a hand (renormalized).
    pub fn strategy(&self, id: NodeId, hand: Hand) -> Option<Vec<f32>> {
        self.node(id).strategy(HandClass::of(hand).index())
    }

    /// EV of each action at `id` for a hand, in bb.
    pub fn ev(&self, id: NodeId, hand: Hand) -> Option<Vec<f32>> {
        self.node(id)
            .ev(HandClass::of(hand).index(), self.file.header.ev_scale)
    }

    /// Pot before the decision at `id`, in bb.
    pub fn pot_bb(&self, id: NodeId) -> f64 {
        self.node(id).pot as f64 / BB_CHIPS as f64
    }

    /// Probability, per hand class, that `pos` plays its part of the line to
    /// `id` (product of its frequencies along the line; 1 if it has not acted).
    pub fn reach(&self, id: NodeId, pos: Position) -> [f32; HandClass::COUNT] {
        let mut r = [1.0f32; HandClass::COUNT];
        let mut at = id;
        while let Some((p, a)) = self.parents[at] {
            let n = self.node(p);
            if n.player == pos {
                for (c, w) in r.iter_mut().enumerate() {
                    *w *= n.strategy(c).map_or(0.0, |s| s[a]);
                }
            }
            at = p;
        }
        r
    }

    /// Range of the player to act at `id`, as combo weights per class
    /// (`reach × combos in class`), normalized to sum to 1.
    pub fn acting_range(&self, id: NodeId) -> [f32; HandClass::COUNT] {
        let pos = self.node(id).player;
        let mut r = self.reach(id, pos);
        for (c, w) in r.iter_mut().enumerate() {
            *w *= HandClass::from_index(c).expect("< 169").num_combos() as f32;
        }
        let total: f32 = r.iter().sum();
        if total > 0.0 {
            r.iter_mut().for_each(|w| *w /= total);
        }
        r
    }
}

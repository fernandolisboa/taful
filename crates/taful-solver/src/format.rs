//! Taful library format, versions 1 and 2 (spec §7.5).
//!
//! One file per (configuration, board). Layout:
//!
//! ```text
//! "TAFL" magic, format_version u16 LE        -- uncompressed, so a reader can
//!                                               reject a version before inflating
//! zstd frame:
//!   header  game_config (canonical TOML) + its key [u8; 32], line, board
//!           (0 cards = the preflop pseudo-board), bet-tree descriptor,
//!           row domain, root ranges, exploitability_pct f32, ev_scale f32,
//!           solver id, build-manifest hash [u8; 32]
//!   nodes   flat array in DFS order (root = 0); per node:
//!             player (position) u8, pot before the decision u32 (chips),
//!             exploitability_pct f32,
//!             actions: count u8, then per action kind u8 + size u32 (chips)
//!                      + child u32 (node index, or END / PRUNED)
//!             live rows: count u16 + row indices u16
//!             strategy [u8; live × actions]   frequencies × 255, renormalized on read
//!             ev       [i16; live × actions]  × ev_scale = bb, acting player only
//!   chance  (v2 only) count u32, then per chance node: count u8 of branches,
//!           per branch card u8 + tag u8 and
//!             tag 0: node u32      the subtree for this card
//!             tag 1: (nothing)     the subtree was pruned for reach
//!             tag 2: of u8 + perm [u8; 4]   isomorphic to card `of`: read
//!                    that subtree with combos and cards mapped by `perm`
//! ```
//!
//! v3 appends, after the chance section:
//!
//! ```text
//! profiles  count u8, then per bot profile: id, canonical profile TOML,
//!           ev_scale f32, then per node (same order as `nodes`):
//!             live rows: count u16 + row indices u16   rows the profile
//!                        plays to this node
//!             strategy [u8; live × actions]   the profile's strategy when
//!                                             it acts here, × 255
//!             ev       [i16; node live × actions]   the actor's best-response
//!                        EVs against the profile, rows as the node's `live`
//! ```
//!
//! A *row* is a combo (`Domain::Combos`, 1,326 rows, postflop) or a preflop
//! hand class (`Domain::Classes`, 169 rows in [`HandClass::index`] order).
//! Preflop strategy is suit-symmetric, so class rows lose nothing and make
//! the chart ~8x smaller. Rows are stored row-major: `[row][action]`.
//!
//! All integers are little-endian. Strings are a u32 byte length + UTF-8.
//!
//! **v2** (M4) adds the chance layer for postflop files: an action that
//! closes the flop leads to [`Child::Chance`], one branch per turn card, and
//! the turn subtree ends at the river ([`Child::End`]; rivers are solved on
//! demand, D11). The writer emits v1 when a file has no chance nodes, so a
//! preflop chart is byte-identical across the bump.
//!
//! **v3** (#27) adds per-profile sections (D8): for every node, the bot
//! profile's locked strategy and the acting player's best-response EVs
//! against it, computed by the build tool while the full tree is in memory.
//! The writer emits v3 only for files with profile data, so files without
//! it stay v1/v2 and mean "reference only".
//!
//! [`HandClass::index`]: taful_core::range::HandClass::index

use std::fmt;
use std::io::Read;
use std::str::FromStr;

use taful_core::action::Action;
use taful_core::cards::{Card, NUM_COMBOS};
use taful_core::config::Chips;
use taful_core::iso::SuitPerm;
use taful_core::position::Position;
use taful_core::range::HandClass;

pub const MAGIC: [u8; 4] = *b"TAFL";
/// Version written by this crate.
pub const FORMAT_VERSION: u16 = 3;
/// Oldest version this crate still reads (spec §7.5: current and previous;
/// v1 also stays, since files without chance nodes, the preflop charts, are
/// still written as v1).
pub const MIN_READ_VERSION: u16 = 1;
/// Largest inflated body a reader accepts (a turn-truncated postflop file is
/// tens of MB; this leaves ample room).
pub const MAX_BODY_BYTES: u64 = 1 << 30;

/// What a node row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// One row per combo (1,326), postflop.
    Combos,
    /// One row per preflop hand class (169).
    Classes,
}

impl Domain {
    pub fn rows(self) -> usize {
        match self {
            Domain::Combos => NUM_COMBOS,
            Domain::Classes => HandClass::COUNT,
        }
    }
    fn tag(self) -> u8 {
        match self {
            Domain::Combos => 0,
            Domain::Classes => 1,
        }
    }
    fn from_tag(t: u8) -> Result<Domain, FormatError> {
        match t {
            0 => Ok(Domain::Combos),
            1 => Ok(Domain::Classes),
            _ => Err(FormatError::Corrupt(format!("unknown domain {t}"))),
        }
    }
}

/// Where an action leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Child {
    /// Another stored decision node.
    Node(u32),
    /// The betting round (or the hand) ends: everyone folded, the street
    /// closed, or all players are all-in. Not a library exit.
    End,
    /// A decision node exists in the solved tree but was dropped for low
    /// reach (spec §7.5); reaching it means leaving the library (D4).
    Pruned,
    /// The street closed and the next card is dealt: index into
    /// [`LibraryFile::chance`] (v2).
    Chance(u32),
}

const CHILD_END: u32 = u32::MAX;
const CHILD_PRUNED: u32 = u32::MAX - 1;
const CHILD_CHANCE: u32 = 1 << 31;

impl Child {
    fn encode(self) -> u32 {
        match self {
            Child::Node(i) => {
                assert!(i < CHILD_CHANCE, "node index fits 31 bits");
                i
            }
            Child::End => CHILD_END,
            Child::Pruned => CHILD_PRUNED,
            Child::Chance(i) => {
                assert!(i < CHILD_PRUNED - CHILD_CHANCE, "chance index range");
                CHILD_CHANCE | i
            }
        }
    }
    fn decode(v: u32) -> Child {
        match v {
            CHILD_END => Child::End,
            CHILD_PRUNED => Child::Pruned,
            i if i & CHILD_CHANCE != 0 => Child::Chance(i & !CHILD_CHANCE),
            i => Child::Node(i),
        }
    }
}

/// Where one dealt card leads (v2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// The first decision node after the card.
    Node(u32),
    /// The subtree after this card was dropped for reach.
    Pruned,
    /// Isomorphic to the branch of card `of` in the same chance node: read
    /// that subtree with every card and combo mapped through `perm` (which
    /// maps this branch's world onto `of`'s: `perm.card(card) == of`).
    Mirror { of: Card, perm: SuitPerm },
}

/// A chance node: one branch per card that can be dealt.
#[derive(Debug, Clone, PartialEq)]
pub struct ChanceNode {
    /// Ascending by card; dead cards are absent.
    pub branches: Vec<(Card, Branch)>,
}

impl ChanceNode {
    pub fn branch(&self, card: Card) -> Option<Branch> {
        self.branches
            .binary_search_by_key(&card, |(c, _)| *c)
            .ok()
            .map(|i| self.branches[i].1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeAction {
    pub action: Action,
    pub child: Child,
}

/// One decision node.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub player: Position,
    /// Chips in the middle before the decision.
    pub pot: Chips,
    /// How far the stored strategy is from a best response at this node, in
    /// percent of `pot` (grading tolerance, §8).
    pub exploitability_pct: f32,
    pub actions: Vec<NodeAction>,
    /// Rows stored at this node, ascending.
    pub live: Vec<u16>,
    /// `[live.len()][actions.len()]`, frequencies × 255.
    pub strategy: Vec<u8>,
    /// `[live.len()][actions.len()]`, × `Header::ev_scale` = bb.
    pub ev: Vec<i16>,
}

impl Node {
    /// Position of `row` in [`live`](Self::live), if stored.
    pub fn live_index(&self, row: usize) -> Option<usize> {
        let row = u16::try_from(row).ok()?;
        self.live.binary_search(&row).ok()
    }

    /// Frequencies for a row, renormalized to sum to 1. `None` if not live.
    pub fn strategy(&self, row: usize) -> Option<Vec<f32>> {
        let i = self.live_index(row)?;
        let n = self.actions.len();
        let q = &self.strategy[i * n..(i + 1) * n];
        let total: u32 = q.iter().map(|&x| u32::from(x)).sum();
        Some(if total == 0 {
            vec![1.0 / n as f32; n]
        } else {
            q.iter().map(|&x| f32::from(x) / total as f32).collect()
        })
    }

    /// EV of each action for a row, in bb. `None` if not live.
    pub fn ev(&self, row: usize, ev_scale: f32) -> Option<Vec<f32>> {
        let i = self.live_index(row)?;
        let n = self.actions.len();
        Some(
            self.ev[i * n..(i + 1) * n]
                .iter()
                .map(|&e| f32::from(e) * ev_scale)
                .collect(),
        )
    }

    /// Index of `action` among this node's actions.
    pub fn action_index(&self, action: Action) -> Option<usize> {
        self.actions.iter().position(|a| a.action == action)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// Canonical TOML of the `GameConfig` (spec §5.2).
    pub game_config: String,
    pub config_key: [u8; 32],
    /// Preflop line leading to this tree; empty for preflop charts.
    pub line: String,
    /// Board cards; empty = the preflop pseudo-board.
    pub board: Vec<Card>,
    /// Human-readable description of the sizing tree the solve used.
    pub bet_tree: String,
    pub domain: Domain,
    /// Range of each player at the root, quantized × 255 per row.
    pub root_ranges: Vec<(Position, Vec<u8>)>,
    /// Exploitability of the whole tree, percent of the starting pot.
    pub exploitability_pct: f32,
    /// bb per EV unit.
    pub ev_scale: f32,
    /// Solver name and version, e.g. `taful-preflop 1`.
    pub solver_id: String,
    /// BLAKE3 of the build manifest that produced the file.
    pub manifest_hash: [u8; 32],
}

/// A whole library file in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryFile {
    pub header: Header,
    pub nodes: Vec<Node>,
    /// Chance nodes (v2); empty for preflop charts.
    pub chance: Vec<ChanceNode>,
    /// Per bot profile data (v3); empty = reference only.
    pub profiles: Vec<ProfileSection>,
}

/// One bot profile's data in a file (v3, D8).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileSection {
    /// Profile id (`content/bots/<id>.toml`).
    pub id: String,
    /// Canonical TOML of the profile the build locked, so a reader applies
    /// exactly the same transform where it solves on its own (the river).
    pub profile: String,
    /// bb per EV unit of this section.
    pub ev_scale: f32,
    /// One per node of [`LibraryFile::nodes`], same order.
    pub nodes: Vec<ProfileNode>,
}

/// One node's profile data.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileNode {
    /// Rows the profile plays to this node, ascending.
    pub live: Vec<u16>,
    /// `[live.len()][actions]`, the profile's strategy when it acts here,
    /// frequencies × 255.
    pub strategy: Vec<u8>,
    /// `[node.live.len()][actions]`, the acting player's best-response EVs
    /// against the profile (rows as the node's own `live`), × `ev_scale` =
    /// bb, same convention as the reference EVs.
    pub ev: Vec<i16>,
}

impl ProfileNode {
    /// The profile's frequencies for a row (renormalized). `None` if the
    /// profile does not play the row to this node.
    pub fn strategy(&self, row: usize, actions: usize) -> Option<Vec<f32>> {
        let row = u16::try_from(row).ok()?;
        let i = self.live.binary_search(&row).ok()?;
        let q = &self.strategy[i * actions..(i + 1) * actions];
        let total: u32 = q.iter().map(|&x| u32::from(x)).sum();
        Some(if total == 0 {
            vec![1.0 / actions as f32; actions]
        } else {
            q.iter().map(|&x| f32::from(x) / total as f32).collect()
        })
    }

    /// Best-response EVs of `node`'s actor for a row, bb. `None` if the row
    /// is not live at the node.
    pub fn ev(&self, node: &Node, row: usize, ev_scale: f32) -> Option<Vec<f32>> {
        let i = node.live_index(row)?;
        let k = node.actions.len();
        Some(
            self.ev[i * k..(i + 1) * k]
                .iter()
                .map(|&e| f32::from(e) * ev_scale)
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    BadMagic,
    UnsupportedVersion(u16),
    Corrupt(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::BadMagic => f.write_str("not a Taful library file"),
            FormatError::UnsupportedVersion(v) => write!(
                f,
                "library format v{v} is not supported (reader handles v{MIN_READ_VERSION}..=v{FORMAT_VERSION})"
            ),
            FormatError::Corrupt(m) => write!(f, "corrupt library file: {m}"),
        }
    }
}

impl std::error::Error for FormatError {}

fn corrupt(m: impl Into<String>) -> FormatError {
    FormatError::Corrupt(m.into())
}

// ---- quantization ---------------------------------------------------------

/// Frequencies (summing to ~1) → bytes whose sum is exactly 255, using the
/// largest-remainder method so a pure action stays exactly 255.
pub fn quantize_strategy(freqs: &[f32]) -> Vec<u8> {
    let total: f32 = freqs.iter().map(|f| f.max(0.0)).sum();
    if total <= 0.0 || !total.is_finite() {
        let n = freqs.len().max(1) as u32;
        let mut q: Vec<u8> = vec![(255 / n) as u8; freqs.len()];
        for x in q.iter_mut().take((255 % n) as usize) {
            *x += 1;
        }
        return q;
    }
    let scaled: Vec<f32> = freqs.iter().map(|f| f.max(0.0) / total * 255.0).collect();
    let mut q: Vec<u8> = scaled.iter().map(|x| x.floor() as u8).collect();
    let mut left = 255 - q.iter().map(|&x| u32::from(x)).sum::<u32>();
    let mut order: Vec<usize> = (0..freqs.len()).collect();
    order.sort_by(|&a, &b| {
        let ra = scaled[a] - scaled[a].floor();
        let rb = scaled[b] - scaled[b].floor();
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for &i in order.iter().cycle() {
        if left == 0 {
            break;
        }
        q[i] += 1;
        left -= 1;
    }
    q
}

/// The `ev_scale` that maps `max |ev|` to the i16 range.
pub fn ev_scale_for(max_abs_bb: f32) -> f32 {
    if max_abs_bb > 0.0 && max_abs_bb.is_finite() {
        max_abs_bb / f32::from(i16::MAX)
    } else {
        1.0 / f32::from(i16::MAX)
    }
}

pub fn quantize_ev(ev_bb: f32, ev_scale: f32) -> i16 {
    (ev_bb / ev_scale)
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

// ---- encoding -------------------------------------------------------------

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    fn str(&mut self, s: &str) {
        self.u32(u32::try_from(s.len()).expect("string < 4GB"));
        self.bytes(s.as_bytes());
    }
}

fn position_tag(p: Position) -> u8 {
    Position::ALL.iter().position(|&q| q == p).expect("listed") as u8
}

fn position_from_tag(t: u8) -> Result<Position, FormatError> {
    Position::ALL
        .get(usize::from(t))
        .copied()
        .ok_or_else(|| corrupt(format!("bad position {t}")))
}

fn action_parts(a: Action) -> (u8, u32) {
    let size = |c: Chips| u32::try_from(c).expect("action size fits u32 chips");
    match a {
        Action::Fold => (0, 0),
        Action::Check => (1, 0),
        Action::Call => (2, 0),
        Action::Bet(c) => (3, size(c)),
        Action::Raise(c) => (4, size(c)),
        Action::AllIn => (5, 0),
    }
}

fn action_from_parts(kind: u8, size: u32) -> Result<Action, FormatError> {
    let c = Chips::from(size);
    Ok(match kind {
        0 => Action::Fold,
        1 => Action::Check,
        2 => Action::Call,
        3 => Action::Bet(c),
        4 => Action::Raise(c),
        5 => Action::AllIn,
        _ => return Err(corrupt(format!("bad action kind {kind}"))),
    })
}

impl LibraryFile {
    /// Check the internal consistency the reader relies on.
    pub fn validate(&self) -> Result<(), FormatError> {
        let rows = self.header.domain.rows();
        if !matches!(self.header.board.len(), 0 | 3 | 4 | 5) {
            return Err(corrupt(format!(
                "board of {} cards",
                self.header.board.len()
            )));
        }
        if self.header.root_ranges.len() > usize::from(u8::MAX) {
            return Err(corrupt("more than 255 root ranges"));
        }
        for (p, r) in &self.header.root_ranges {
            if r.len() != rows {
                return Err(corrupt(format!("root range of {p} has {} rows", r.len())));
            }
        }
        if !self.header.ev_scale.is_finite() || self.header.ev_scale <= 0.0 {
            return Err(corrupt("ev_scale must be positive"));
        }
        let n_nodes = self.nodes.len();
        for (i, n) in self.nodes.iter().enumerate() {
            let k = n.actions.len();
            if k == 0 || k > usize::from(u8::MAX) {
                return Err(corrupt(format!("node {i}: {k} actions")));
            }
            if n.live.windows(2).any(|w| w[0] >= w[1])
                || n.live.last().is_some_and(|&r| usize::from(r) >= rows)
            {
                return Err(corrupt(format!(
                    "node {i}: live rows not ascending/in range"
                )));
            }
            if n.strategy.len() != n.live.len() * k || n.ev.len() != n.live.len() * k {
                return Err(corrupt(format!("node {i}: table sizes")));
            }
            for a in &n.actions {
                match a.child {
                    Child::Node(c) if c as usize <= i || c as usize >= n_nodes => {
                        return Err(corrupt(format!("node {i}: child {c} not after it (DFS)")));
                    }
                    Child::Chance(k) => {
                        let ch = self
                            .chance
                            .get(k as usize)
                            .ok_or_else(|| corrupt(format!("node {i}: chance {k} missing")))?;
                        for (_, b) in &ch.branches {
                            if let Branch::Node(c) = b
                                && (*c as usize <= i || *c as usize >= n_nodes)
                            {
                                return Err(corrupt(format!(
                                    "node {i}: chance child {c} not after it (DFS)"
                                )));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        for (k, ch) in self.chance.iter().enumerate() {
            if ch.branches.is_empty()
                || ch.branches.len() > usize::from(u8::MAX)
                || ch.branches.windows(2).any(|w| w[0].0 >= w[1].0)
            {
                return Err(corrupt(format!("chance {k}: branches not ascending")));
            }
            for (card, b) in &ch.branches {
                if self.header.board.contains(card) {
                    return Err(corrupt(format!("chance {k}: {card} is on the board")));
                }
                if let Branch::Mirror { of, perm } = b {
                    let ok = perm.card(*card) == *of
                        && matches!(ch.branch(*of), Some(Branch::Node(_) | Branch::Pruned))
                        && perm
                            .cards(&self.header.board)
                            .iter()
                            .all(|c| self.header.board.contains(c));
                    if !ok {
                        return Err(corrupt(format!("chance {k}: bad mirror {card}→{of}")));
                    }
                }
            }
        }
        if self.profiles.len() > usize::from(u8::MAX) {
            return Err(corrupt("more than 255 profiles"));
        }
        for (i, p) in self.profiles.iter().enumerate() {
            if p.id.is_empty() || self.profiles[..i].iter().any(|q| q.id == p.id) {
                return Err(corrupt(format!("profile {i}: empty or repeated id")));
            }
            if !p.ev_scale.is_finite() || p.ev_scale <= 0.0 {
                return Err(corrupt(format!("profile {}: ev_scale", p.id)));
            }
            if p.nodes.len() != n_nodes {
                return Err(corrupt(format!("profile {}: node count", p.id)));
            }
            for (j, (pn, n)) in p.nodes.iter().zip(&self.nodes).enumerate() {
                let k = n.actions.len();
                if pn.live.windows(2).any(|w| w[0] >= w[1])
                    || pn.live.last().is_some_and(|&r| usize::from(r) >= rows)
                    || pn.live.len() > usize::from(u16::MAX)
                {
                    return Err(corrupt(format!("profile {} node {j}: live rows", p.id)));
                }
                if pn.strategy.len() != pn.live.len() * k || pn.ev.len() != n.live.len() * k {
                    return Err(corrupt(format!("profile {} node {j}: table sizes", p.id)));
                }
            }
        }
        Ok(())
    }

    /// Serialize (header + nodes, zstd-compressed after the magic and version).
    pub fn encode(&self) -> Vec<u8> {
        self.validate().expect("encoding an invalid library file");
        let h = &self.header;
        let mut w = W(Vec::new());
        w.str(&h.game_config);
        w.bytes(&h.config_key);
        w.str(&h.line);
        w.u8(h.board.len() as u8);
        for c in &h.board {
            w.u8(c.index());
        }
        w.str(&h.bet_tree);
        w.u8(h.domain.tag());
        w.u8(h.root_ranges.len() as u8);
        for (p, r) in &h.root_ranges {
            w.u8(position_tag(*p));
            w.bytes(r);
        }
        w.f32(h.exploitability_pct);
        w.f32(h.ev_scale);
        w.str(&h.solver_id);
        w.bytes(&h.manifest_hash);

        w.u32(u32::try_from(self.nodes.len()).expect("< 4G nodes"));
        for n in &self.nodes {
            w.u8(position_tag(n.player));
            w.u32(u32::try_from(n.pot).expect("pot fits u32 chips"));
            w.f32(n.exploitability_pct);
            w.u8(n.actions.len() as u8);
            for a in &n.actions {
                let (kind, size) = action_parts(a.action);
                w.u8(kind);
                w.u32(size);
                w.u32(a.child.encode());
            }
            w.u16(n.live.len() as u16);
            for &r in &n.live {
                w.u16(r);
            }
            w.bytes(&n.strategy);
            for &e in &n.ev {
                w.bytes(&e.to_le_bytes());
            }
        }

        // Each version only when the file needs it: no chance nodes and no
        // profiles stays v1 (preflop charts), no profiles stays v2.
        let version: u16 = if !self.profiles.is_empty() {
            3
        } else if !self.chance.is_empty() {
            2
        } else {
            1
        };
        if version >= 2 {
            w.u32(u32::try_from(self.chance.len()).expect("< 4G chance nodes"));
            for ch in &self.chance {
                w.u8(ch.branches.len() as u8);
                for (card, b) in &ch.branches {
                    w.u8(card.index());
                    match b {
                        Branch::Node(i) => {
                            w.u8(0);
                            w.u32(*i);
                        }
                        Branch::Pruned => w.u8(1),
                        Branch::Mirror { of, perm } => {
                            w.u8(2);
                            w.u8(of.index());
                            w.bytes(&perm.map());
                        }
                    }
                }
            }
        }
        if version >= 3 {
            w.u8(self.profiles.len() as u8);
            for p in &self.profiles {
                w.str(&p.id);
                w.str(&p.profile);
                w.f32(p.ev_scale);
                for n in &p.nodes {
                    w.u16(n.live.len() as u16);
                    for &r in &n.live {
                        w.u16(r);
                    }
                    w.bytes(&n.strategy);
                    for &e in &n.ev {
                        w.bytes(&e.to_le_bytes());
                    }
                }
            }
        }

        let mut out = Vec::with_capacity(w.0.len() / 4);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&version.to_le_bytes());
        out.extend(ruzstd::encoding::compress_to_vec(
            w.0.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        ));
        out
    }

    /// Parse and validate a file.
    pub fn decode(bytes: &[u8]) -> Result<LibraryFile, FormatError> {
        if bytes.len() < 6 || bytes[..4] != MAGIC {
            return Err(FormatError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if !(MIN_READ_VERSION..=FORMAT_VERSION).contains(&version) {
            return Err(FormatError::UnsupportedVersion(version));
        }
        let mut body = Vec::new();
        // Cap the inflated size so a crafted file cannot exhaust memory.
        ruzstd::decoding::StreamingDecoder::new(&bytes[6..])
            .map_err(|e| corrupt(format!("zstd: {e}")))?
            .take(MAX_BODY_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|e| corrupt(format!("zstd: {e}")))?;
        if body.len() as u64 > MAX_BODY_BYTES {
            return Err(corrupt("inflated body exceeds the size limit"));
        }
        let mut r = R { b: &body, at: 0 };

        let game_config = r.str()?;
        let config_key = r.arr32()?;
        let line = r.str()?;
        let board_len = r.u8()?;
        if !matches!(board_len, 0 | 3 | 4 | 5) {
            return Err(corrupt(format!("board of {board_len} cards")));
        }
        let board = (0..board_len)
            .map(|_| {
                let i = r.u8()?;
                Card::from_index(i).ok_or_else(|| corrupt(format!("bad card {i}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bet_tree = r.str()?;
        let domain = Domain::from_tag(r.u8()?)?;
        let n_ranges = r.u8()?;
        let mut root_ranges = Vec::with_capacity(usize::from(n_ranges));
        for _ in 0..n_ranges {
            let p = position_from_tag(r.u8()?)?;
            root_ranges.push((p, r.take(domain.rows())?.to_vec()));
        }
        let exploitability_pct = r.f32()?;
        let ev_scale = r.f32()?;
        let solver_id = r.str()?;
        let manifest_hash = r.arr32()?;

        let n_nodes = r.u32()? as usize;
        // Each node takes at least 12 bytes; reject absurd counts before allocating.
        if n_nodes > body.len() / 12 + 1 {
            return Err(corrupt("node count exceeds file size"));
        }
        let mut nodes = Vec::with_capacity(n_nodes);
        for _ in 0..n_nodes {
            let player = position_from_tag(r.u8()?)?;
            let pot = Chips::from(r.u32()?);
            let exploitability_pct = r.f32()?;
            let k = usize::from(r.u8()?);
            let mut actions = Vec::with_capacity(k);
            for _ in 0..k {
                let kind = r.u8()?;
                let size = r.u32()?;
                actions.push(NodeAction {
                    action: action_from_parts(kind, size)?,
                    child: Child::decode(r.u32()?),
                });
            }
            let n_live = usize::from(r.u16()?);
            let live = (0..n_live)
                .map(|_| r.u16())
                .collect::<Result<Vec<_>, _>>()?;
            let strategy = r.take(n_live * k)?.to_vec();
            let ev = (0..n_live * k)
                .map(|_| r.u16().map(|v| v as i16))
                .collect::<Result<Vec<_>, _>>()?;
            nodes.push(Node {
                player,
                pot,
                exploitability_pct,
                actions,
                live,
                strategy,
                ev,
            });
        }
        let mut chance = Vec::new();
        if version >= 2 {
            let n = r.u32()? as usize;
            if n > body.len() / 3 + 1 {
                return Err(corrupt("chance count exceeds file size"));
            }
            for _ in 0..n {
                let k = usize::from(r.u8()?);
                let mut branches = Vec::with_capacity(k);
                for _ in 0..k {
                    let card = r.card()?;
                    let b = match r.u8()? {
                        0 => Branch::Node(r.u32()?),
                        1 => Branch::Pruned,
                        2 => {
                            let of = r.card()?;
                            let map: [u8; 4] = r.take(4)?.try_into().expect("4");
                            let perm =
                                SuitPerm::new(map).ok_or_else(|| corrupt("bad suit perm"))?;
                            Branch::Mirror { of, perm }
                        }
                        t => return Err(corrupt(format!("bad branch tag {t}"))),
                    };
                    branches.push((card, b));
                }
                chance.push(ChanceNode { branches });
            }
        }
        let mut profiles = Vec::new();
        if version >= 3 {
            let count = r.u8()?;
            for _ in 0..count {
                let id = r.str()?;
                let profile = r.str()?;
                let ev_scale = r.f32()?;
                let mut pnodes = Vec::with_capacity(nodes.len());
                for n in &nodes {
                    let k = n.actions.len();
                    let n_live = usize::from(r.u16()?);
                    let live = (0..n_live)
                        .map(|_| r.u16())
                        .collect::<Result<Vec<_>, _>>()?;
                    let strategy = r.take(n_live * k)?.to_vec();
                    let ev = (0..n.live.len() * k)
                        .map(|_| r.u16().map(|v| v as i16))
                        .collect::<Result<Vec<_>, _>>()?;
                    pnodes.push(ProfileNode { live, strategy, ev });
                }
                profiles.push(ProfileSection {
                    id,
                    profile,
                    ev_scale,
                    nodes: pnodes,
                });
            }
        }
        if r.at != body.len() {
            return Err(corrupt("trailing bytes (unsupported extension?)"));
        }
        let file = LibraryFile {
            header: Header {
                game_config,
                config_key,
                line,
                board,
                bet_tree,
                domain,
                root_ranges,
                exploitability_pct,
                ev_scale,
                solver_id,
                manifest_hash,
            },
            nodes,
            chance,
            profiles,
        };
        file.validate()?;
        Ok(file)
    }
}

struct R<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or_else(|| corrupt("truncated"))?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2")))
    }
    fn u32(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn f32(&mut self) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn card(&mut self) -> Result<Card, FormatError> {
        let i = self.u8()?;
        Card::from_index(i).ok_or_else(|| corrupt(format!("bad card {i}")))
    }
    fn arr32(&mut self) -> Result<[u8; 32], FormatError> {
        Ok(self.take(32)?.try_into().expect("32"))
    }
    fn str(&mut self) -> Result<String, FormatError> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| corrupt("bad UTF-8"))
    }
}

impl FromStr for Domain {
    type Err = String;
    fn from_str(s: &str) -> Result<Domain, String> {
        match s {
            "combos" => Ok(Domain::Combos),
            "classes" => Ok(Domain::Classes),
            _ => Err(format!("unknown domain {s:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn sample() -> LibraryFile {
        let rows = Domain::Classes.rows();
        let node = |player, actions: Vec<NodeAction>, live: Vec<u16>| {
            let k = actions.len();
            let n = live.len();
            Node {
                player,
                pot: 150,
                exploitability_pct: 0.25,
                strategy: (0..n * k).map(|i| (i * 37 % 256) as u8).collect(),
                ev: (0..n * k).map(|i| (i as i16) * 13 - 500).collect(),
                actions,
                live,
            }
        };
        LibraryFile {
            header: Header {
                game_config: taful_core::config::GameConfig::cash(6, 100).to_toml(),
                config_key: [7; 32],
                line: String::new(),
                board: vec![],
                bet_tree: "opens 2.5bb".into(),
                domain: Domain::Classes,
                root_ranges: vec![
                    (Position::Utg, vec![255; rows]),
                    (Position::Bb, vec![9; rows]),
                ],
                exploitability_pct: 0.4,
                ev_scale: 0.001,
                solver_id: "test 1".into(),
                manifest_hash: [3; 32],
            },
            nodes: vec![
                node(
                    Position::Utg,
                    vec![
                        NodeAction {
                            action: Action::Fold,
                            child: Child::Node(1),
                        },
                        NodeAction {
                            action: Action::Raise(250),
                            child: Child::Pruned,
                        },
                    ],
                    (0..rows as u16).collect(),
                ),
                node(
                    Position::Hj,
                    vec![
                        NodeAction {
                            action: Action::Fold,
                            child: Child::End,
                        },
                        NodeAction {
                            action: Action::Call,
                            child: Child::End,
                        },
                        NodeAction {
                            action: Action::AllIn,
                            child: Child::End,
                        },
                    ],
                    vec![0, 5, 168],
                ),
            ],
            chance: vec![],
            profiles: vec![],
        }
    }

    /// A postflop-shaped file: flop root, check-check deals a turn.
    fn sample_v2() -> LibraryFile {
        let mut f = sample();
        f.header.domain = Domain::Combos;
        f.header.board = taful_core::cards::parse_cards("Td9d6h").unwrap();
        let rows = Domain::Combos.rows();
        f.header.root_ranges = vec![
            (Position::Bb, vec![1; rows]),
            (Position::Btn, vec![2; rows]),
        ];
        f.nodes[0].player = Position::Bb;
        f.nodes[0].actions[0] = NodeAction {
            action: Action::Check,
            child: Child::Chance(0),
        };
        f.nodes[0].live = vec![3, 900];
        f.nodes[0].strategy = vec![100, 155, 0, 255];
        f.nodes[0].ev = vec![1, 2, 3, 4];
        f.nodes[1].player = Position::Bb;
        let c = |s: &str| s.parse::<Card>().unwrap();
        let swap = SuitPerm::swap(
            taful_core::cards::Suit::CLUBS,
            taful_core::cards::Suit::SPADES,
        );
        f.chance = vec![ChanceNode {
            branches: vec![
                (c("2c"), Branch::Node(1)),
                (
                    c("2s"),
                    Branch::Mirror {
                        of: c("2c"),
                        perm: swap,
                    },
                ),
                (c("3c"), Branch::Pruned),
            ],
        }];
        f
    }

    #[test]
    fn roundtrip() {
        let f = sample();
        let bytes = f.encode();
        assert_eq!(&bytes[..4], b"TAFL");
        assert_eq!(LibraryFile::decode(&bytes).unwrap(), f);
    }

    #[test]
    fn roundtrip_v2_and_v1_compat() {
        let f = sample_v2();
        let bytes = f.encode();
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 2);
        assert_eq!(LibraryFile::decode(&bytes).unwrap(), f);
        // A file without chance nodes stays v1.
        let v1 = sample().encode();
        assert_eq!(u16::from_le_bytes([v1[4], v1[5]]), 1);
    }

    /// `sample_v2` with one profile section.
    fn sample_v3() -> LibraryFile {
        let mut f = sample_v2();
        let nodes = f
            .nodes
            .iter()
            .map(|n| {
                let k = n.actions.len();
                ProfileNode {
                    live: vec![7, 900],
                    strategy: (0..2 * k).map(|i| (i * 51 % 256) as u8).collect(),
                    ev: (0..n.live.len() * k).map(|i| i as i16 - 3).collect(),
                }
            })
            .collect();
        f.profiles = vec![ProfileSection {
            id: "calling-station".into(),
            profile: "id = \"calling-station\"\n".into(),
            ev_scale: 0.002,
            nodes,
        }];
        f
    }

    #[test]
    fn roundtrip_v3_profiles() {
        let f = sample_v3();
        let bytes = f.encode();
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 3);
        let back = LibraryFile::decode(&bytes).unwrap();
        assert_eq!(back, f);
        let p = &back.profiles[0];
        let k = back.nodes[0].actions.len();
        assert!(p.nodes[0].strategy(7, k).is_some());
        assert!(p.nodes[0].strategy(3, k).is_none());
        // EV rows follow the node's own live rows.
        assert_eq!(
            p.nodes[0].ev(&back.nodes[0], 3, p.ev_scale).unwrap().len(),
            k
        );
        assert!(p.nodes[0].ev(&back.nodes[0], 7, p.ev_scale).is_none());
        // Without profiles the same file is still written as v2.
        let v2 = sample_v2().encode();
        assert_eq!(u16::from_le_bytes([v2[4], v2[5]]), 2);
        assert_eq!(LibraryFile::decode(&v2).unwrap().profiles, vec![]);
    }

    #[test]
    fn rejects_bad_profile_sections() {
        let mut f = sample_v3();
        f.profiles[0].nodes.pop();
        assert!(f.validate().is_err());
        let mut f = sample_v3();
        f.profiles[0].nodes[0].ev.pop();
        assert!(f.validate().is_err());
        let mut f = sample_v3();
        f.profiles[0].nodes[1].live = vec![5, 5];
        assert!(f.validate().is_err());
        let mut f = sample_v3();
        let dup = f.profiles[0].clone();
        f.profiles.push(dup);
        assert!(f.validate().is_err());
    }

    #[test]
    fn rejects_bad_mirror() {
        let mut f = sample_v2();
        let c = |s: &str| s.parse::<Card>().unwrap();
        f.chance[0].branches[2].1 = Branch::Mirror {
            of: c("2c"),
            perm: SuitPerm::IDENTITY,
        };
        assert!(f.validate().is_err());
        let mut g = sample_v2();
        g.chance[0].branches.swap(1, 2);
        assert!(g.validate().is_err());
    }

    #[test]
    fn rejects_bad_input() {
        let mut bytes = sample().encode();
        assert_eq!(LibraryFile::decode(b"nope"), Err(FormatError::BadMagic));
        bytes[4] = 99;
        assert_eq!(
            LibraryFile::decode(&bytes),
            Err(FormatError::UnsupportedVersion(99))
        );
        let good = sample().encode();
        assert!(LibraryFile::decode(&good[..good.len() - 3]).is_err());
    }

    #[test]
    fn node_accessors() {
        let f = sample();
        let n = &f.nodes[1];
        assert!(n.strategy(1).is_none());
        let s = n.strategy(5).unwrap();
        assert!((s.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(n.action_index(Action::AllIn), Some(2));
        assert_eq!(n.ev(168, 0.001).unwrap().len(), 3);
    }

    proptest! {
        #[test]
        fn quantized_strategy_sums_to_255(f in proptest::collection::vec(0.0f32..1.0, 1..8)) {
            let q = quantize_strategy(&f);
            prop_assert_eq!(q.iter().map(|&x| u32::from(x)).sum::<u32>(), 255);
            let total: f32 = f.iter().sum();
            if total > 0.0 {
                for (a, b) in f.iter().zip(&q) {
                    prop_assert!((a / total * 255.0 - f32::from(*b)).abs() <= 1.0);
                }
            }
        }

        #[test]
        fn ev_quantization_error_is_bounded(e in -150.0f32..150.0) {
            let s = ev_scale_for(150.0);
            prop_assert!((f32::from(quantize_ev(e, s)) * s - e).abs() <= s);
        }
    }

    #[test]
    fn pure_actions_stay_pure() {
        assert_eq!(quantize_strategy(&[0.0, 1.0, 0.0]), vec![0, 255, 0]);
        assert_eq!(quantize_strategy(&[0.0, 0.0]), vec![128, 127]);
    }
}

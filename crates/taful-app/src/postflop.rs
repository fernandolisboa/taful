//! Postflop spot mode (spec §7.6, §10.1, M4): questions drawn from the
//! postflop library, graded like preflop (spec §8).
//!
//! The library is a folder in the static layout of spec §7.6 with a
//! `manifest.json` (written by `tools/library-build`) listing every file
//! with its size and BLAKE3 hash. Files are verified before use and decoded
//! lazily. Flops are dealt only from solved flops, weighted so textures come
//! up as often as on random boards, with a random suit relabeling (D13).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use taful_core::action::{Line, Street};
use taful_core::cards::{CardSet, NUM_COMBOS};
use taful_core::config::GameConfig;
use taful_core::engine::Phase;
use taful_core::grading::{Thresholds, grade};
use taful_core::iso::{Flop, SuitPerm, deal_weights};
use taful_solver::format::FormatError;
use taful_solver::postflop::{PostflopFile, SpotGeometry, geometry};

use crate::preflop::{Action, BB_CHIPS, Card, GameState, Hand, Position, Seat};
pub use crate::preflop::{GradedDecision, VsProfile};
pub use crate::template::NodeType;
use crate::walk::{Progress, Walk};

/// Where library files come from (spec §7.6). `Http` arrives with the web
/// client (Q5).
pub trait LibrarySource: Send + Sync {
    /// Bytes of the file at `path` (relative, `/`-separated).
    fn read(&self, path: &str) -> Result<Vec<u8>, PostflopError>;
    /// Human-readable location, for messages.
    fn describe(&self) -> String;
}

/// A library folder on disk.
#[derive(Debug, Clone)]
pub struct LocalDir(pub PathBuf);

impl LibrarySource for LocalDir {
    fn read(&self, path: &str) -> Result<Vec<u8>, PostflopError> {
        if path.split('/').any(|c| c == ".." || c.is_empty()) {
            return Err(PostflopError::Library(format!("bad path {path:?}")));
        }
        let p = path.split('/').fold(self.0.clone(), |acc, c| acc.join(c));
        std::fs::read(&p).map_err(|e| PostflopError::Library(format!("{}: {e}", p.display())))
    }
    fn describe(&self) -> String {
        self.0.display().to_string()
    }
}

/// The library folder: `TAFUL_LIBRARY_DIR`, else `<data dir>/library`.
pub fn default_library_dir(data_dir: &Path) -> PathBuf {
    std::env::var_os("TAFUL_LIBRARY_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("library"))
}

#[derive(Debug, Clone, PartialEq)]
pub enum PostflopError {
    /// The library is missing, unreadable or fails verification.
    Library(String),
    Format(FormatError),
    /// No spot or flop to deal.
    Empty,
    NotInTree(Action),
    /// The node holds no data for this hand (left the library, D4).
    NoData,
}

impl std::fmt::Display for PostflopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostflopError::Library(m) => write!(f, "postflop library: {m}"),
            PostflopError::Format(e) => write!(f, "postflop library: {e}"),
            PostflopError::Empty => f.write_str("postflop library has no spots"),
            PostflopError::NotInTree(a) => write!(f, "action {a} is not in the tree"),
            PostflopError::NoData => f.write_str("no library data for this hand"),
        }
    }
}

impl std::error::Error for PostflopError {}

#[derive(Debug, Clone, serde::Deserialize)]
struct IndexFile {
    files: Vec<IndexEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct IndexEntry {
    path: String,
    size: u64,
    blake3: String,
}

/// One solved configuration (e.g. `btn-bb-srp`) and its flops.
#[derive(Debug, Clone)]
pub struct SpotConfig {
    /// Stable id, also the i18n key suffix (`table-postflop-<id>`).
    pub id: String,
    pub line: Line,
    pub geometry: SpotGeometry,
    flops: Vec<(Flop, IndexEntry)>,
    weights: Vec<f64>,
}

impl SpotConfig {
    pub fn flop_count(&self) -> usize {
        self.flops.len()
    }
}

/// A flop dealt in a configuration's cycle (#51), to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DealtFlop {
    pub spot: String,
    pub cycle: u32,
    /// Canonical.
    pub flop: Flop,
}

/// One configuration's cycle: the flops dealt since it started.
#[derive(Debug, Clone, Default)]
struct Cycle {
    number: u32,
    dealt: HashSet<Flop>,
}

impl Cycle {
    /// Starts the next cycle once every solved flop of `cfg` was dealt.
    fn close_if_complete(&mut self, cfg: &SpotConfig) {
        if cfg.flops.iter().all(|(f, _)| self.dealt.contains(f)) {
            self.number += 1;
            self.dealt.clear();
        }
    }
}

/// Which flops each configuration dealt in its current cycle (#51). Off
/// until [`PostflopTrainer::start_cycles`]: then a deal prefers a solved
/// flop not dealt yet, and a new cycle starts once all were.
#[derive(Debug, Default)]
struct Cycles {
    on: bool,
    by_spot: HashMap<String, Cycle>,
    /// Dealt since the last [`PostflopTrainer::take_dealt`]. Lost if the
    /// trainer is replaced or the store write fails first: at worst a flop
    /// repeats early, which is fine for a no-repeat preference.
    pending: Vec<DealtFlop>,
}

/// Postflop library + thresholds: spot generation and grading.
pub struct PostflopTrainer {
    source: Box<dyn LibrarySource>,
    config: GameConfig,
    spots: Vec<SpotConfig>,
    thresholds: Thresholds,
    cache: Mutex<HashMap<String, Arc<PostflopFile>>>,
    cycles: Mutex<Cycles>,
}

impl std::fmt::Debug for PostflopTrainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostflopTrainer")
            .field("source", &self.source.describe())
            .field("spots", &self.spots.len())
            .finish()
    }
}

/// Keep at most this many decoded files in memory.
const CACHE_FILES: usize = 16;

impl PostflopTrainer {
    /// Open the library for `config` from `source`.
    pub fn open(
        source: Box<dyn LibrarySource>,
        config: GameConfig,
        thresholds: Thresholds,
    ) -> Result<PostflopTrainer, PostflopError> {
        let index: IndexFile = serde_json::from_slice(&source.read("manifest.json")?)
            .map_err(|e| PostflopError::Library(format!("manifest.json: {e}")))?;
        let prefix = format!(
            "v{}/{}/",
            taful_solver::LIBRARY_LAYOUT_VERSION,
            config.key()
        );
        let mut by_spot: BTreeMap<String, Vec<(Flop, IndexEntry)>> = BTreeMap::new();
        for e in index.files {
            let Some(rest) = e.path.strip_prefix(&prefix) else {
                continue;
            };
            let Some((spot, file)) = rest.split_once('/') else {
                continue; // preflop.tafl
            };
            let Some(flop) = file
                .strip_suffix(".tafl")
                .and_then(|f| f.parse::<Flop>().ok())
                .filter(Flop::is_canonical)
            else {
                continue;
            };
            by_spot.entry(spot.to_string()).or_default().push((flop, e));
        }
        let mut t = PostflopTrainer {
            source,
            config,
            spots: Vec::new(),
            thresholds,
            cache: Mutex::new(HashMap::new()),
            cycles: Mutex::new(Cycles::default()),
        };
        for (id, mut flops) in by_spot {
            flops.sort_by_key(|(f, _)| f.cards().map(Card::index));
            // The first file tells the spot's line; every file is checked
            // against it when loaded.
            let first = t.load(&flops[0].1)?;
            let line: Line = first
                .file()
                .header
                .line
                .parse()
                .map_err(|e| PostflopError::Library(format!("{id}: line {e:?}")))?;
            let geometry = geometry(&t.config, &line).map_err(PostflopError::Library)?;
            let fl: Vec<Flop> = flops.iter().map(|(f, _)| *f).collect();
            t.spots.push(SpotConfig {
                id,
                line,
                geometry,
                weights: deal_weights(&fl),
                flops,
            });
        }
        if t.spots.is_empty() {
            return Err(PostflopError::Empty);
        }
        Ok(t)
    }

    pub fn spots(&self) -> &[SpotConfig] {
        &self.spots
    }
    pub fn config(&self) -> &GameConfig {
        &self.config
    }
    pub fn thresholds(&self) -> &Thresholds {
        &self.thresholds
    }
    pub fn source(&self) -> String {
        self.source.describe()
    }

    fn load(&self, e: &IndexEntry) -> Result<Arc<PostflopFile>, PostflopError> {
        if let Some(f) = self.cache.lock().expect("not poisoned").get(&e.path) {
            return Ok(f.clone());
        }
        let bytes = self.source.read(&e.path)?;
        if bytes.len() as u64 != e.size || blake3::hash(&bytes).to_hex().as_str() != e.blake3 {
            return Err(PostflopError::Library(format!(
                "{} does not match manifest.json",
                e.path
            )));
        }
        let f = Arc::new(PostflopFile::decode(&bytes).map_err(PostflopError::Format)?);
        if f.config() != &self.config {
            return Err(PostflopError::Library(format!(
                "{}: other game config",
                e.path
            )));
        }
        let mut cache = self.cache.lock().expect("not poisoned");
        if cache.len() >= CACHE_FILES {
            cache.clear();
        }
        cache.insert(e.path.clone(), f.clone());
        Ok(f)
    }

    /// The configuration whose preflop line is `line`, if solved.
    pub fn spot_for_line(&self, line: &Line) -> Option<&SpotConfig> {
        self.spots.iter().find(|s| &s.line == line)
    }

    /// A random solved flop of `cfg` dealt as a real flop that avoids `dead`
    /// (spec §7.4, D13): flops weighted by real frequency, a random suit
    /// relabeling. `None` if no relabeling avoids the dead cards.
    pub(crate) fn deal_flop(
        &self,
        cfg: &SpotConfig,
        texture: Option<&str>,
        dead: CardSet,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<(Arc<PostflopFile>, Flop)>, PostflopError> {
        self.deal(cfg, texture, dead, rng, true)
    }

    /// [`Self::deal_flop`], with `cycle` false dealing from every solved flop.
    fn deal(
        &self,
        cfg: &SpotConfig,
        texture: Option<&str>,
        dead: CardSet,
        rng: &mut ChaCha8Rng,
        cycle: bool,
    ) -> Result<Option<(Arc<PostflopFile>, Flop)>, PostflopError> {
        // Flops not dealt in this cycle first (#51); every solved flop when
        // none of those fits.
        if cycle && let Some(d) = self.deal_flop_from(cfg, texture, dead, rng, true)? {
            return Ok(Some(d));
        }
        self.deal_flop_from(cfg, texture, dead, rng, false)
    }

    fn deal_flop_from(
        &self,
        cfg: &SpotConfig,
        texture: Option<&str>,
        dead: CardSet,
        rng: &mut ChaCha8Rng,
        fresh: bool,
    ) -> Result<Option<(Arc<PostflopFile>, Flop)>, PostflopError> {
        let dealt = if fresh {
            let c = self.cycles.lock().expect("not poisoned");
            if !c.on {
                return Ok(None);
            }
            c.by_spot
                .get(&cfg.id)
                .map(|x| x.dealt.clone())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        let idx: Vec<usize> = (0..cfg.flops.len())
            .filter(|&i| texture.is_none_or(|t| cfg.flops[i].0.texture().class().id() == t))
            .filter(|&i| !dealt.contains(&cfg.flops[i].0))
            .collect();
        let perms: Vec<SuitPerm> = SuitPerm::all().collect();
        for _ in 0..200 {
            let Ok(&fi) = idx.choose_weighted(rng, |&i| cfg.weights[i]) else {
                return Ok(None);
            };
            let relabel = *perms.choose(rng).expect("24");
            let real = cfg.flops[fi].0.permuted(&relabel);
            if real.cards().iter().any(|c| dead.contains(*c)) {
                continue;
            }
            let file = self.load(&cfg.flops[fi].1)?;
            if file.file().header.line != cfg.line.to_string() {
                return Err(PostflopError::Library(format!(
                    "{}: line differs from the spot",
                    cfg.flops[fi].1.path
                )));
            }
            return Ok(Some((file, real)));
        }
        Ok(None)
    }

    /// Turns on dealing in cycles (#51), starting from what the store
    /// remembers. A deal then depends on the seed and the cycle state, so
    /// the same seed replays the same hand only from the same state.
    pub fn start_cycles(&self, dealt: impl IntoIterator<Item = DealtFlop>) {
        let mut c = self.cycles.lock().expect("not poisoned");
        c.on = true;
        c.by_spot.clear();
        for d in dealt {
            let e = c.by_spot.entry(d.spot).or_default();
            if d.cycle > e.number {
                *e = Cycle {
                    number: d.cycle,
                    dealt: HashSet::new(),
                };
            }
            if d.cycle == e.number {
                e.dealt.insert(d.flop);
            }
        }
        // The store keeps the cycle that was closed last; the next one
        // starts empty and lives only in memory until its first deal.
        for cfg in &self.spots {
            if let Some(e) = c.by_spot.get_mut(&cfg.id) {
                e.close_if_complete(cfg);
            }
        }
    }

    /// Records that `real` was dealt in configuration `spot`: a new cycle
    /// starts once every solved flop of it was dealt. No-op while cycles
    /// are off.
    pub(crate) fn note_dealt(&self, spot: &str, real: Flop) {
        let Some(cfg) = self.spots.iter().find(|s| s.id == spot) else {
            return;
        };
        let flop = real.canonical().0;
        let mut guard = self.cycles.lock().expect("not poisoned");
        let c = &mut *guard;
        if !c.on {
            return;
        }
        let e = c.by_spot.entry(spot.to_string()).or_default();
        if e.dealt.insert(flop) {
            c.pending.push(DealtFlop {
                spot: spot.to_string(),
                cycle: e.number,
                flop,
            });
        }
        e.close_if_complete(cfg);
    }

    /// The flops dealt since the last call, for the store.
    pub fn take_dealt(&self) -> Vec<DealtFlop> {
        std::mem::take(&mut self.cycles.lock().expect("not poisoned").pending)
    }

    /// A walk from the root of `cfg` on `real`.
    pub(crate) fn walk(
        &self,
        cfg: &SpotConfig,
        file: Arc<PostflopFile>,
        real: Flop,
    ) -> Result<Walk, PostflopError> {
        Walk::start(&cfg.id, file, cfg.geometry.clone(), real)
    }

    /// A random postflop question (spec §10.1). `spot` picks a configuration
    /// by id (`None` = any). The flop is weighted by real frequency and
    /// relabeled by a random suit permutation; the line is sampled from both
    /// ranges' aggregate frequencies, stopping at a random flop or turn
    /// decision; hands are drawn from each player's reach there.
    pub fn spot(&self, spot: Option<&str>, seed: u64) -> Result<PostflopSpot, PostflopError> {
        self.generate(
            &SpotRequest {
                spot,
                ..SpotRequest::default()
            },
            seed,
        )
    }

    /// A question matching `req` (drills from mistakes and leaks, spec §13).
    /// Tries a bounded number of random deals; [`PostflopError::Empty`] if
    /// none matches.
    pub fn generate(
        &self,
        req: &SpotRequest<'_>,
        seed: u64,
    ) -> Result<PostflopSpot, PostflopError> {
        let s = self.generate_unnoted(req, seed)?;
        self.note_spot(&s);
        Ok(s)
    }

    /// [`Self::generate`] without marking the flop as dealt in its cycle,
    /// for callers that may discard the spot ([`Self::note_spot`] once
    /// kept). A spot counts as dealt when it is shown, answered or not.
    pub(crate) fn generate_unnoted(
        &self,
        req: &SpotRequest<'_>,
        seed: u64,
    ) -> Result<PostflopSpot, PostflopError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut rivers = RIVER_SOLVES;
        for attempt in 0..ATTEMPTS {
            // The first half deals from the cycle; when a request only fits
            // flops already dealt in it, the second half uses every flop.
            let cycle = attempt < ATTEMPTS / 2;
            if let Some(s) = self.try_generate(req, &mut rng, &mut rivers, cycle)? {
                return Ok(s);
            }
        }
        Err(PostflopError::Empty)
    }

    /// Marks the flop of `s` as dealt in its cycle (#51).
    pub(crate) fn note_spot(&self, s: &PostflopSpot) {
        let w = s.walk();
        let board = w.board();
        if let Some(flop) = Flop::new([board[0], board[1], board[2]]) {
            self.note_dealt(w.spot_id(), flop);
        }
    }

    fn try_generate(
        &self,
        req: &SpotRequest<'_>,
        rng: &mut ChaCha8Rng,
        rivers: &mut usize,
        cycle: bool,
    ) -> Result<Option<PostflopSpot>, PostflopError> {
        let cfg = match req.spot {
            Some(id) => self
                .spots
                .iter()
                .find(|s| s.id == id)
                .ok_or(PostflopError::Empty)?,
            None => self.spots.choose(rng).ok_or(PostflopError::Empty)?,
        };
        let Some((file, real)) = self.deal(cfg, req.texture, CardSet::EMPTY, rng, cycle)? else {
            return Ok(None);
        };
        let mut walk = self.walk(cfg, file, real)?;
        // Against a profile hero's side is fixed up front, so the villain
        // plays the profile on the way to hero's decision.
        let mut fallback = None;
        let mut hero_side = req
            .hero
            .and_then(|h| walk.positions().iter().position(|&p| p == h));
        if let Some(id) = req.opponent {
            let side = hero_side.unwrap_or_else(|| usize::from(rng.random::<bool>()));
            match walk.clone().against(id, side ^ 1)? {
                Some(w) => {
                    walk = w;
                    hero_side = Some(side);
                }
                None => fallback = Some(id.to_string()),
            }
        }
        let script: Vec<(Position, Action)> = req
            .script
            .iter()
            .flat_map(|s| s.iter().flatten().copied())
            .collect();
        let max_street = req.street.unwrap_or(Street::Turn);
        let mut done = 0;
        // Stop at each decision with this probability (so flop and turn
        // spots both come up), and always before leaving the library.
        const STOP: f64 = 0.45;
        loop {
            let node = walk.node().expect("at a decision");
            let acceptable = req.hero.is_none_or(|h| h == node.position)
                && hero_side.is_none_or(|h| h == node.side)
                && req.street.is_none_or(|s| s == node.street)
                && req
                    .node_type
                    .is_none_or(|t| t == NodeType::postflop(node.facing));
            let action = if let Some(s) = req.script {
                if done == script.len() {
                    if walk.history().len() != s.len().max(1) || !acceptable {
                        return Ok(None);
                    }
                    break;
                }
                let (p, a) = script[done];
                if p != node.position {
                    return Ok(None);
                }
                done += 1;
                a
            } else {
                if acceptable && rng.random::<f64>() < STOP {
                    break;
                }
                // Aggregate frequencies over the actor's range (folds
                // excluded: they end the hand).
                let freqs: Vec<f64> = (0..node.actions.len())
                    .map(|a| {
                        if node.actions[a] == Action::Fold {
                            0.0
                        } else {
                            walk.action_weight(a)
                        }
                    })
                    .collect();
                let choices: Vec<usize> = (0..freqs.len()).collect();
                let Ok(&a) = choices.choose_weighted(rng, |&i| freqs[i]) else {
                    if acceptable {
                        break;
                    }
                    return Ok(None);
                };
                let action = node.actions[a];
                let next = walk.peek(action);
                let continues = match next {
                    Progress::Decision => true,
                    Progress::NeedsCard(st) => st <= max_street,
                    _ => false,
                };
                if !continues {
                    if acceptable {
                        break;
                    }
                    return Ok(None);
                }
                action
            };
            match walk.act(action)? {
                Progress::Decision => {}
                Progress::NeedsCard(street) => {
                    // Each river deal solves the river: bound the wait.
                    if street == Street::River {
                        if *rivers == 0 {
                            return Ok(None);
                        }
                        *rivers -= 1;
                    }
                    let live: Vec<Card> =
                        Card::all().filter(|c| !walk.board().contains(c)).collect();
                    let card = *live.choose(rng).expect("cards left");
                    if walk.deal(card, &[])? != Progress::Decision {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }
        }

        // Hero acts at the walk's node; draw both hands from reach.
        let node = walk.node().expect("at a decision");
        let hero_p = node.side;
        let dead = CardSet::from_cards(walk.board());
        let draw = |rng: &mut ChaCha8Rng, r: &[f32], dead: CardSet, need_live: bool| {
            let hs: Vec<usize> = (0..NUM_COMBOS)
                .filter(|&h| {
                    let hand = Hand::from_combo(h).expect("<1326");
                    r[h] > 0.0
                        && !hand.mask().intersects(dead)
                        && (!need_live || walk.strategy(hand).is_some())
                })
                .collect();
            hs.choose_weighted(rng, |&h| r[h])
                .ok()
                .map(|&h| Hand::from_combo(h).expect("<1326"))
        };
        let Some(hero_hand) = draw(rng, walk.reach(hero_p), dead, true) else {
            return Ok(None);
        };
        let Some(villain_hand) = draw(
            rng,
            walk.reach(hero_p ^ 1),
            dead.union(hero_hand.mask()),
            false,
        ) else {
            return Ok(None);
        };
        let pos = walk.positions();
        let mut hands = [hero_hand, villain_hand];
        if hero_p == 1 {
            hands.swap(0, 1);
        }
        let state = self.table_state(&cfg.line, &walk, hands, rng)?;
        let n = usize::from(self.config.table_size);
        let hero = seat_of(n, pos[hero_p]);
        if state.to_act() != Some(hero) {
            return Err(PostflopError::Library(
                "engine and library disagree on who acts".into(),
            ));
        }
        Ok(Some(PostflopSpot {
            spot_id: cfg.id.clone(),
            hero,
            hand: hero_hand,
            state,
            postflop_line: Line(walk.actions().collect()),
            board: walk.board().to_vec(),
            opponent: walk.vs_profile().map(str::to_owned),
            fallback,
            walk,
        }))
    }

    /// The table at the walk's point: the preflop line, the board and the
    /// postflop actions on the engine; `hands` are OOP's and IP's.
    fn table_state(
        &self,
        line: &Line,
        walk: &Walk,
        hands: [Hand; 2],
        rng: &mut ChaCha8Rng,
    ) -> Result<GameState, PostflopError> {
        let n = usize::from(self.config.table_size);
        let pos = walk.positions();
        let mut holes = vec![None; n];
        holes[seat_of(n, pos[0])] = Some(hands[0]);
        holes[seat_of(n, pos[1])] = Some(hands[1]);
        // Folded seats get cards off the board too (the engine checks).
        let board = walk.board();
        let used = CardSet::from_cards(board)
            .union(hands[0].mask())
            .union(hands[1].mask());
        let mut deck: Vec<Card> = Card::all().filter(|c| !used.contains(*c)).collect();
        for i in (1..deck.len()).rev() {
            let j = rng.random_range(0..=i);
            deck.swap(i, j);
        }
        for h in holes.iter_mut().filter(|h| h.is_none()) {
            let (a, b) = (deck.pop().expect("cards"), deck.pop().expect("cards"));
            *h = Some(Hand::new(a, b).expect("distinct"));
        }
        let engine = |e| PostflopError::Library(format!("engine: {e:?}"));
        let mut state = GameState::new_with_holes(self.config.clone(), 0, rng.random(), &holes)
            .map_err(engine)?;
        for &(_, a) in &line.0 {
            state = state.apply(a).map_err(engine)?;
        }
        state = state.deal_board(&board[..3]).map_err(engine)?;
        for (_, a) in walk.actions() {
            state = state.apply(a).map_err(engine)?;
            if let Phase::NeedsBoard(_) = state.phase() {
                let k = state.board().len();
                if k < board.len() {
                    state = state.deal_board(&board[k..k + 1]).map_err(engine)?;
                }
            }
        }
        Ok(state)
    }

    /// Grade hero taking `chosen` at the spot's node.
    pub fn grade(&self, s: &PostflopSpot, chosen: usize) -> Result<GradedDecision, PostflopError> {
        grade_walk(&s.walk, s.hand, chosen, &self.thresholds)
    }
}

/// Deals tried by [`PostflopTrainer::generate`] before giving up.
const ATTEMPTS: usize = 60;
/// River solves one [`PostflopTrainer::generate`] call may run (each is
/// about 0.4 s in release; solving off the UI thread is an M6 follow-up).
const RIVER_SOLVES: usize = 8;

/// Seat of `pos` with the button on seat 0.
pub(crate) fn seat_of(n: usize, pos: Position) -> Seat {
    Position::by_button_offset(n)
        .iter()
        .position(|&q| q == pos)
        .expect("seated")
}

/// What a generated question must match (all optional).
#[derive(Debug, Clone, Copy, Default)]
pub struct SpotRequest<'a> {
    /// Configuration id.
    pub spot: Option<&'a str>,
    /// Board family: a [`TextureClass`](taful_core::iso::TextureClass) id.
    pub texture: Option<&'a str>,
    /// Exact postflop history (one list per street from the flop); hero
    /// decides right after it.
    pub script: Option<&'a [Vec<(Position, Action)>]>,
    /// Street of the decision (default: flop or turn; `River` solves the
    /// river in the app).
    pub street: Option<Street>,
    pub node_type: Option<NodeType>,
    pub hero: Option<Position>,
    /// Bot profile id the villain plays (`None` = the reference). A file
    /// without that profile's data deals a reference spot instead and says
    /// so in [`PostflopSpot::fallback`] (#27).
    pub opponent: Option<&'a str>,
}

/// Grade `hand` taking action `chosen` at the walk's node (spec §8).
/// Against a bot profile (the walk's) the EV loss is measured on hero's
/// best-response EVs against it and only EV counts, as preflop (D8).
pub(crate) fn grade_walk(
    walk: &Walk,
    hand: Hand,
    chosen: usize,
    thresholds: &Thresholds,
) -> Result<GradedDecision, PostflopError> {
    let node = walk.node().ok_or(PostflopError::NoData)?;
    let freqs = walk.strategy(hand).ok_or(PostflopError::NoData)?;
    let reference_evs = walk.ev_bb(hand).ok_or(PostflopError::NoData)?;
    if chosen >= node.actions.len() {
        return Err(PostflopError::NoData);
    }
    let (evs, vs_profile) = match walk.vs_profile() {
        None => (reference_evs, None),
        Some(id) => {
            let evs = walk.profile_ev_bb(hand).ok_or(PostflopError::NoData)?;
            let gto = (0..freqs.len())
                .max_by(|&a, &b| {
                    freqs[a]
                        .total_cmp(&freqs[b])
                        .then(reference_evs[a].total_cmp(&reference_evs[b]))
                })
                .unwrap_or(0);
            let exploit = crate::preflop::argmax(&evs);
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
    let pot_bb = node.pot as f64 / BB_CHIPS as f64;
    let graded_freq = if vs_profile.is_some() {
        0.0
    } else {
        freqs[chosen]
    };
    Ok(GradedDecision {
        node: walk.history().iter().map(Vec::len).sum(),
        line: Line(walk.actions().collect()),
        position: node.position,
        hand,
        actions: node.actions.clone(),
        chosen,
        grade: grade(ev_loss, pot_bb, graded_freq, node.tolerance_bb, thresholds),
        hand_freqs: freqs,
        hand_evs: evs,
        range_freqs: walk.range_freqs(),
        pot_bb,
        ev_loss_bb: ev_loss,
        tolerance_bb: node.tolerance_bb,
        thresholds_version: thresholds.version,
        vs_profile,
    })
}

/// One postflop question.
#[derive(Debug, Clone)]
pub struct PostflopSpot {
    pub spot_id: String,
    walk: Walk,
    pub hero: Seat,
    pub hand: Hand,
    /// Table state at hero's decision.
    pub state: GameState,
    /// Postflop actions before hero's decision.
    pub postflop_line: Line,
    /// Real board (3 to 5 cards).
    pub board: Vec<Card>,
    /// The bot profile the villain plays and hero is graded against
    /// (`None` = the reference).
    pub opponent: Option<String>,
    /// A profile was asked for but this spot's library file has no data for
    /// it (a reference-only library): the spot is against the reference.
    pub fallback: Option<String>,
}

impl PostflopSpot {
    /// Hero's options, in tree order.
    pub fn actions(&self) -> Vec<Action> {
        self.walk.node().map(|n| n.actions).unwrap_or_default()
    }

    pub fn walk(&self) -> &Walk {
        &self.walk
    }

    pub fn answer(
        &self,
        trainer: &PostflopTrainer,
        action: Action,
    ) -> Result<GradedDecision, PostflopError> {
        let chosen = self
            .actions()
            .iter()
            .position(|&a| a == action)
            .ok_or(PostflopError::NotInTree(action))?;
        trainer.grade(self, chosen)
    }
}

#[cfg(test)]
mod tests {
    use taful_core::grading::Grade;

    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop")
    }

    fn trainer() -> PostflopTrainer {
        PostflopTrainer::open(
            Box::new(LocalDir(fixture())),
            GameConfig::cash(6, 100),
            Thresholds::default(),
        )
        .expect("fixture library opens")
    }

    /// Postflop spots' stories (#53) rebuild the preflop line, deal the
    /// flop (and turn) as events and end on hero's decision.
    #[test]
    fn spot_stories_deal_the_board() {
        use crate::story::{Story, StoryEvent};
        let t = trainer();
        let mut checked = 0;
        for seed in 0..30 {
            let Ok(s) = t.generate(&SpotRequest::default(), seed) else {
                continue;
            };
            checked += 1;
            let story = Story::of(&s.state).expect("story");
            let last = story.last();
            assert_eq!(last.players(), s.state.players(), "seed {seed}");
            assert_eq!(last.board(), s.state.board(), "seed {seed}");
            assert_eq!(last.history(), s.state.history(), "seed {seed}");
            let dealt: Vec<usize> = story
                .events
                .iter()
                .filter_map(|e| match e {
                    StoryEvent::Dealt { cards, .. } => Some(cards.len()),
                    StoryEvent::Action(_) => None,
                })
                .collect();
            let want: Vec<usize> = match s.state.board().len() {
                3 => vec![3],
                4 => vec![3, 1],
                _ => vec![3, 1, 1],
            };
            assert_eq!(dealt, want, "seed {seed}");
        }
        assert!(checked > 0, "the fixture deals postflop spots");
    }

    fn flop_of(s: &PostflopSpot) -> Flop {
        Flop::new([s.board[0], s.board[1], s.board[2]])
            .unwrap()
            .canonical()
            .0
    }

    #[test]
    fn cycles_deal_every_flop_before_repeating() {
        let t = trainer();
        // Off until started: nothing is noted.
        t.spot(None, 0).unwrap();
        assert!(t.take_dealt().is_empty());
        t.start_cycles([]);
        let flops: Vec<Flop> = (0..6)
            .map(|seed| flop_of(&t.spot(None, seed).unwrap()))
            .collect();
        // Two solved flops: each cycle deals both, whatever the weights.
        for pair in flops.chunks(2) {
            assert_ne!(pair[0], pair[1], "{flops:?}");
        }
        let dealt = t.take_dealt();
        assert_eq!(
            dealt.iter().map(|d| d.cycle).collect::<Vec<_>>(),
            vec![0, 0, 1, 1, 2, 2]
        );
        assert_eq!(dealt.iter().map(|d| d.flop).collect::<Vec<_>>(), flops);
        assert!(t.take_dealt().is_empty());
    }

    #[test]
    fn cycles_resume_where_the_store_left_off() {
        let t = trainer();
        let tc: Flop = "Tc9c6d".parse().unwrap();
        let kc: Flop = "Kc7c2c".parse().unwrap();
        let spot = "btn-sb-3bp".to_string();
        t.start_cycles([
            DealtFlop {
                spot: spot.clone(),
                cycle: 2,
                flop: kc,
            },
            DealtFlop {
                spot: spot.clone(),
                cycle: 3,
                flop: tc,
            },
        ]);
        // Cycle 3 already dealt Tc9c6d: Kc7c2c comes next and closes it.
        assert_eq!(flop_of(&t.spot(None, 0).unwrap()), kc);
        let next = flop_of(&t.spot(None, 1).unwrap());
        let dealt = t.take_dealt();
        assert_eq!(
            dealt,
            vec![
                DealtFlop {
                    spot: spot.clone(),
                    cycle: 3,
                    flop: kc,
                },
                DealtFlop {
                    spot,
                    cycle: 4,
                    flop: next,
                },
            ]
        );
    }

    /// The store's last rows can be a closed cycle (the next one was only
    /// in memory): a reload starts the next cycle instead of sticking.
    #[test]
    fn a_closed_cycle_in_the_store_starts_the_next() {
        let t = trainer();
        let tc: Flop = "Tc9c6d".parse().unwrap();
        let kc: Flop = "Kc7c2c".parse().unwrap();
        let spot = "btn-sb-3bp".to_string();
        t.start_cycles([tc, kc].map(|flop| DealtFlop {
            spot: spot.clone(),
            cycle: 0,
            flop,
        }));
        let a = flop_of(&t.spot(None, 0).unwrap());
        let b = flop_of(&t.spot(None, 1).unwrap());
        assert_ne!(a, b, "a fresh cycle deals both flops");
        let dealt = t.take_dealt();
        assert_eq!(dealt.len(), 2);
        assert_eq!(dealt[0].cycle, 1);
        assert_eq!(dealt[1].cycle, 1);
    }

    #[test]
    fn a_request_the_cycle_cannot_meet_uses_every_flop() {
        let t = trainer();
        let kc: Flop = "Kc7c2c".parse().unwrap();
        t.start_cycles([DealtFlop {
            spot: "btn-sb-3bp".into(),
            cycle: 0,
            flop: kc,
        }]);
        let texture = kc.texture().class().id();
        let req = SpotRequest {
            texture: Some(&texture),
            ..SpotRequest::default()
        };
        let s = t.generate(&req, 1).unwrap();
        assert_eq!(flop_of(&s), kc);
    }

    #[test]
    fn flops_are_dealt_in_every_suit() {
        let t = trainer();
        let real: HashSet<Flop> = (0..60)
            .map(|seed| {
                let s = t.spot(None, seed).unwrap();
                Flop::new([s.board[0], s.board[1], s.board[2]]).unwrap()
            })
            .collect();
        assert!(real.iter().any(|f| !f.is_canonical()));
        assert!(real.len() > 10, "{} distinct real flops", real.len());
    }

    #[test]
    fn opens_fixture() {
        let t = trainer();
        assert_eq!(t.spots().len(), 1);
        let s = &t.spots()[0];
        assert_eq!(s.id, "btn-sb-3bp");
        assert_eq!(s.flop_count(), 2);
        assert_eq!(
            (s.geometry.oop, s.geometry.ip),
            (Position::Sb, Position::Btn)
        );
    }

    #[test]
    fn spots_are_playable_and_graded() {
        let t = trainer();
        let mut turns = 0;
        for seed in 0..40 {
            let s = t.spot(None, seed).unwrap();
            assert!(matches!(s.board.len(), 3 | 4));
            turns += usize::from(s.board.len() == 4);
            assert_eq!(s.state.to_act(), Some(s.hero));
            assert_eq!(s.state.board(), &s.board[..]);
            assert!(!s.hand.mask().intersects(CardSet::from_cards(&s.board)));
            let actions = s.actions();
            assert!(!actions.is_empty());
            // Every offered action is legal in the engine and gradable.
            for &a in &actions {
                assert!(
                    s.state.apply(a).is_ok(),
                    "{a} illegal in {:?}",
                    s.state.history()
                );
                let d = s.answer(&t, a).unwrap();
                assert!(d.ev_loss_bb >= 0.0);
                assert!((d.range_freqs.iter().sum::<f32>() - 1.0).abs() < 0.02);
            }
            let best = s
                .answer(&t, actions[s.answer(&t, actions[0]).unwrap().best()])
                .unwrap();
            assert_eq!(best.ev_loss_bb, 0.0);
            assert_ne!(best.grade, Grade::Blunder);
            // Same seed, same spot.
            let again = t.spot(None, seed).unwrap();
            assert_eq!((again.hand, again.board.clone()), (s.hand, s.board.clone()));
        }
        assert!(turns > 3, "turn spots come up ({turns}/40)");
    }

    #[test]
    fn rejects_tampered_files() {
        let dir = tempfile::tempdir().unwrap();
        fn copy_dir(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap() {
                let e = e.unwrap();
                let t = to.join(e.file_name());
                if e.path().is_dir() {
                    copy_dir(&e.path(), &t);
                } else {
                    std::fs::copy(e.path(), t).unwrap();
                }
            }
        }
        copy_dir(&fixture(), dir.path());
        let manifest = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
        let tampered = manifest.replace("\"blake3\": \"", "\"blake3\": \"00");
        std::fs::write(dir.path().join("manifest.json"), tampered).unwrap();
        let r = PostflopTrainer::open(
            Box::new(LocalDir(dir.path().to_path_buf())),
            GameConfig::cash(6, 100),
            Thresholds::default(),
        );
        assert!(matches!(r, Err(PostflopError::Library(_))), "{r:?}");
        assert!(LocalDir(dir.path().into()).read("../x").is_err());
    }

    /// Verifies an installed library (issue #24): every file matches
    /// `manifest.json` and decodes, and each configuration deals gradable
    /// spots. Run on the machine that has the library:
    /// `cargo test -p taful-app --release -- --ignored installed_library`
    /// (reads `TAFUL_LIBRARY_DIR`, else `<data dir>/library`).
    #[test]
    #[ignore = "needs an installed postflop library"]
    fn installed_library() {
        let dir = default_library_dir(&crate::services::default_data_dir());
        let t = PostflopTrainer::open(
            Box::new(LocalDir(dir.clone())),
            GameConfig::cash(6, 100),
            Thresholds::default(),
        )
        .unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
        let mut files = 0;
        for s in t.spots() {
            for (flop, e) in &s.flops {
                t.load(e)
                    .unwrap_or_else(|err| panic!("{} {flop}: {err:?}", s.id));
                files += 1;
            }
            t.cache.lock().expect("not poisoned").clear();
            for seed in 0..20 {
                let q = t.spot(Some(&s.id), seed).unwrap();
                for &a in &q.actions() {
                    assert!(q.answer(&t, a).unwrap().ev_loss_bb >= 0.0);
                }
            }
            println!("{}: {} flops ok", s.id, s.flop_count());
        }
        println!(
            "{} spots, {files} files verified in {}",
            t.spots().len(),
            dir.display()
        );
    }
}

//! Postflop spot mode (spec §7.6, §10.1, M4): questions drawn from the
//! postflop library, graded like preflop (spec §8).
//!
//! The library is a folder in the static layout of spec §7.6 with a
//! `manifest.json` (written by `tools/library-build`) listing every file
//! with its size and BLAKE3 hash. Files are verified before use and decoded
//! lazily. Flops are dealt only from solved flops, weighted so textures come
//! up as often as on random boards, with a random suit relabeling (D13).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use taful_core::action::Line;
use taful_core::cards::{CardSet, NUM_COMBOS};
use taful_core::config::GameConfig;
use taful_core::engine::Phase;
use taful_core::grading::{Thresholds, grade};
use taful_core::iso::{Flop, SuitPerm, deal_weights};
use taful_solver::format::FormatError;
use taful_solver::postflop::{Cursor, PostflopFile, SpotGeometry, Step, geometry};

pub use crate::preflop::GradedDecision;
use crate::preflop::{Action, BB_CHIPS, Card, GameState, Hand, Position, Seat};

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

/// Postflop library + thresholds: spot generation and grading.
pub struct PostflopTrainer {
    source: Box<dyn LibrarySource>,
    config: GameConfig,
    spots: Vec<SpotConfig>,
    thresholds: Thresholds,
    cache: Mutex<HashMap<String, Arc<PostflopFile>>>,
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

    /// A random postflop question (spec §10.1). `spot` picks a configuration
    /// by id (`None` = any). The flop is weighted by real frequency and
    /// relabeled by a random suit permutation; the line is sampled from both
    /// ranges' aggregate frequencies, stopping at a random decision; hands
    /// are drawn from each player's reach there.
    pub fn spot(&self, spot: Option<&str>, seed: u64) -> Result<PostflopSpot, PostflopError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let cfg = match spot {
            Some(id) => self
                .spots
                .iter()
                .find(|s| s.id == id)
                .ok_or(PostflopError::Empty)?,
            None => self.spots.choose(&mut rng).ok_or(PostflopError::Empty)?,
        };
        let idx: Vec<usize> = (0..cfg.flops.len()).collect();
        let &fi = idx
            .choose_weighted(&mut rng, |&i| cfg.weights[i])
            .map_err(|_| PostflopError::Empty)?;
        let file = self.load(&cfg.flops[fi].1)?;
        if file.file().header.line != cfg.line.to_string() {
            return Err(PostflopError::Library(format!(
                "{}: line differs from the spot",
                cfg.flops[fi].1.path
            )));
        }
        let perms: Vec<SuitPerm> = SuitPerm::all().collect();
        let relabel = *perms.choose(&mut rng).expect("24");
        let real_flop = file.flop().permuted(&relabel);
        let mut cursor = file.root(real_flop).ok_or(PostflopError::NoData)?;
        let g = &cfg.geometry;
        let pos = [g.oop, g.ip];
        let mut reach = [
            file.root_range(g.oop, &cursor.perm)
                .ok_or(PostflopError::NoData)?,
            file.root_range(g.ip, &cursor.perm)
                .ok_or(PostflopError::NoData)?,
        ];
        let mut board: Vec<Card> = real_flop.cards().to_vec();
        let mut actions: Vec<(Position, Action)> = Vec::new();
        // Stop at each decision with this probability (so flop and turn
        // spots both come up), and always before leaving the library.
        const STOP: f64 = 0.45;
        loop {
            let node = file.node(&cursor);
            let p = usize::from(node.player == g.ip);
            if rng.random::<f64>() < STOP {
                break;
            }
            // Aggregate frequencies over the actor's range (folds excluded:
            // they end the hand).
            let dead = CardSet::from_cards(&board);
            let freqs: Vec<f64> = (0..node.actions.len())
                .map(|a| {
                    if node.actions[a].action == Action::Fold {
                        return 0.0;
                    }
                    let mut r = reach[p].clone();
                    file.apply_action_reach(&cursor, a, &mut r);
                    r.iter()
                        .enumerate()
                        .filter(|(h, _)| {
                            !Hand::from_combo(*h).expect("<1326").mask().intersects(dead)
                        })
                        .map(|(_, &w)| f64::from(w))
                        .sum()
                })
                .collect();
            let choices: Vec<usize> = (0..freqs.len()).collect();
            let Ok(&a) = choices.choose_weighted(&mut rng, |&i| freqs[i]) else {
                break;
            };
            let action = node.actions[a].action;
            let next = file.step(&cursor, action);
            let next = match next {
                Step::Chance(k, perm) => {
                    let live: Vec<Card> = Card::all().filter(|c| !board.contains(c)).collect();
                    let turn = *live.choose(&mut rng).expect("cards left");
                    match file.deal(k, perm, turn) {
                        Step::Node(c) => {
                            board.push(turn);
                            Some(c)
                        }
                        _ => None,
                    }
                }
                Step::Node(c) => Some(c),
                Step::End | Step::Left => None,
            };
            let Some(next) = next else {
                break;
            };
            file.apply_action_reach(&cursor, a, &mut reach[p]);
            actions.push((pos[p], action));
            cursor = next;
        }

        // Hero acts at `cursor`; draw both hands from reach.
        let node = file.node(&cursor);
        let hero_p = usize::from(node.player == g.ip);
        let dead = CardSet::from_cards(&board);
        let draw = |rng: &mut ChaCha8Rng, r: &[f32], dead: CardSet, need_live: Option<&Cursor>| {
            let hs: Vec<usize> = (0..NUM_COMBOS)
                .filter(|&h| {
                    let hand = Hand::from_combo(h).expect("<1326");
                    r[h] > 0.0
                        && !hand.mask().intersects(dead)
                        && need_live.is_none_or(|c| file.strategy(c, hand).is_some())
                })
                .collect();
            hs.choose_weighted(rng, |&h| r[h])
                .ok()
                .map(|&h| Hand::from_combo(h).expect("<1326"))
        };
        let hero_hand =
            draw(&mut rng, &reach[hero_p], dead, Some(&cursor)).ok_or(PostflopError::NoData)?;
        let villain_hand = draw(
            &mut rng,
            &reach[hero_p ^ 1],
            dead.union(hero_hand.mask()),
            None,
        )
        .ok_or(PostflopError::NoData)?;

        // Table state: preflop line, flop, postflop actions, turn.
        let n = usize::from(self.config.table_size);
        let seat_of = |p: Position| {
            Position::by_button_offset(n)
                .iter()
                .position(|&q| q == p)
                .expect("seated")
        };
        let hero_pos = pos[hero_p];
        let mut holes = vec![None; n];
        holes[seat_of(hero_pos)] = Some(hero_hand);
        holes[seat_of(pos[hero_p ^ 1])] = Some(villain_hand);
        // Folded seats get cards off the board too (the engine checks).
        let used = dead.union(hero_hand.mask()).union(villain_hand.mask());
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
        for &(_, a) in &cfg.line.0 {
            state = state.apply(a).map_err(engine)?;
        }
        state = state.deal_board(&board[..3]).map_err(engine)?;
        for &(_, a) in &actions {
            state = state.apply(a).map_err(engine)?;
            if let Phase::NeedsBoard(_) = state.phase() {
                state = state.deal_board(&board[3..4]).map_err(engine)?;
            }
        }
        if state.to_act() != Some(seat_of(hero_pos)) {
            return Err(PostflopError::Library(
                "engine and library disagree on who acts".into(),
            ));
        }
        Ok(PostflopSpot {
            spot_id: cfg.id.clone(),
            file,
            cursor,
            hero: seat_of(hero_pos),
            hand: hero_hand,
            state,
            postflop_line: Line(actions),
            hero_reach: reach[hero_p].clone(),
            board,
        })
    }

    /// Grade hero taking `chosen` at the spot's node.
    pub fn grade(&self, s: &PostflopSpot, chosen: usize) -> Result<GradedDecision, PostflopError> {
        let f = &s.file;
        let node = f.node(&s.cursor);
        let freqs = f.strategy(&s.cursor, s.hand).ok_or(PostflopError::NoData)?;
        let evs = f.ev(&s.cursor, s.hand).ok_or(PostflopError::NoData)?;
        let best_ev = evs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let ev_loss = f64::from(best_ev - evs[chosen]).max(0.0);
        let pot_bb = node.pot as f64 / BB_CHIPS as f64;
        let tolerance_bb = f64::from(node.exploitability_pct) / 100.0 * pot_bb;
        let dead = CardSet::from_cards(&s.board);
        let total: f64 = (0..NUM_COMBOS)
            .filter(|&h| !Hand::from_combo(h).expect("<1326").mask().intersects(dead))
            .map(|h| f64::from(s.hero_reach[h]))
            .sum();
        let range_freqs = (0..node.actions.len())
            .map(|a| {
                let mut r = s.hero_reach.clone();
                f.apply_action_reach(&s.cursor, a, &mut r);
                let x: f64 = (0..NUM_COMBOS)
                    .filter(|&h| !Hand::from_combo(h).expect("<1326").mask().intersects(dead))
                    .map(|h| f64::from(r[h]))
                    .sum();
                if total > 0.0 { (x / total) as f32 } else { 0.0 }
            })
            .collect();
        Ok(GradedDecision {
            node: s.cursor.node as usize,
            line: s.postflop_line.clone(),
            position: node.player,
            hand: s.hand,
            actions: node.actions.iter().map(|a| a.action).collect(),
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
}

/// One postflop question.
#[derive(Debug, Clone)]
pub struct PostflopSpot {
    pub spot_id: String,
    file: Arc<PostflopFile>,
    cursor: Cursor,
    pub hero: Seat,
    pub hand: Hand,
    /// Table state at hero's decision.
    pub state: GameState,
    /// Postflop actions before hero's decision.
    pub postflop_line: Line,
    /// Real board (3 or 4 cards).
    pub board: Vec<Card>,
    hero_reach: Vec<f32>,
}

impl PostflopSpot {
    /// Hero's options, in tree order.
    pub fn actions(&self) -> Vec<Action> {
        self.file
            .node(&self.cursor)
            .actions
            .iter()
            .map(|a| a.action)
            .collect()
    }

    pub fn answer(
        &self,
        trainer: &PostflopTrainer,
        action: Action,
    ) -> Result<GradedDecision, PostflopError> {
        let chosen = self
            .file
            .node(&self.cursor)
            .action_index(action)
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
}

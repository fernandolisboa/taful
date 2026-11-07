//! Mixed tables (#29, D8 amendment): one bot profile per seat.
//!
//! Postflop is heads-up in every D2 configuration, so a postflop decision
//! is against the one remaining bot's profile and its best responses come
//! from the library like at a single-profile table. Preflop, hero's best
//! response depends on every opponent still in the hand, so it is computed
//! on demand by the own preflop solver ([`TableGame`]) and cached per table.
//!
//! **Table key.** A preflop decision is filed (decisions, review items,
//! leaks) under the opponents still in the hand at the node, the only seats
//! its best response depends on (folded seats never act again and their
//! card removal cancels out):
//! - all of them play the same profile → that profile's id (or the
//!   reference), exactly what a single-profile table files, and graded on
//!   the same shipped best-response chart;
//! - otherwise → `table:<POS>=<id>,…`, the live opponents in seat order from
//!   the button (e.g. `table:CO=nit,BTN=maniac,BB=tag`; `reference` for a
//!   reference seat). The key names positions, not seats, so a review of
//!   the decision rebuilds the same table.

use std::sync::{Arc, Mutex};

use taful_core::position::{Position, Seat};
use taful_core::profile::REFERENCE;
use taful_solver::preflop::{NodeId, PreflopChart};
use taful_solver::preflop_game::equity::ClassTables;
use taful_solver::preflop_game::manifest::Manifest;
use taful_solver::preflop_game::{SeatPlay, TableBestResponse, TableGame};

use crate::preflop::{PreflopError, PreflopTrainer};

/// Prefix of a table key.
pub const TABLE_PREFIX: &str = "table:";

/// Tables whose best response stays cached. A full hand computes one for
/// its whole table (every decision of the hand reads it), so a table needs
/// at most one entry per hero seat; reviews and drills add one per table
/// key.
const CACHE: usize = 32;

/// Whether `opponent` is a table key.
pub fn is_table_key(opponent: &str) -> bool {
    opponent.starts_with(TABLE_PREFIX)
}

/// The live opponents a table key names, by position (`None` = the
/// reference). `None` if `key` is not a well-formed table key.
pub fn parse_table_key(key: &str) -> Option<Vec<(Position, Option<String>)>> {
    let rest = key.strip_prefix(TABLE_PREFIX)?;
    let mut out = Vec::new();
    for part in rest.split(',') {
        let (pos, id) = part.split_once('=')?;
        let pos: Position = pos.parse().ok()?;
        if id.is_empty() || out.iter().any(|(p, _)| *p == pos) {
            return None;
        }
        out.push((pos, (id != REFERENCE).then(|| id.to_string())));
    }
    (out.len() >= 2).then_some(out)
}

/// The opponent a decision at `node` is filed and graded under: see the
/// module docs. `seat_profiles[s]` is what seat `s` plays (`None` = the
/// reference; hero's entry is ignored); seat 0 is the button.
pub fn opponent_at(
    chart: &PreflopChart,
    node: NodeId,
    seat_profiles: &[Option<String>],
    hero: Seat,
) -> Option<String> {
    let n = seat_profiles.len();
    let folded: Vec<Position> = chart
        .line(node)
        .0
        .iter()
        .filter(|(_, a)| *a == taful_core::action::Action::Fold)
        .map(|(p, _)| *p)
        .collect();
    let live: Vec<(Position, &Option<String>)> = (0..n)
        .filter(|&s| s != hero)
        .map(|s| (Position::of(n, 0, s), &seat_profiles[s]))
        .filter(|(p, _)| !folded.contains(p))
        .collect();
    let first = live.first().map(|(_, id)| *id)?;
    if live.iter().all(|(_, id)| *id == first) {
        return first.clone();
    }
    let parts: Vec<String> = live
        .iter()
        .map(|(p, id)| format!("{p}={}", id.as_deref().unwrap_or(REFERENCE)))
        .collect();
    Some(format!("{TABLE_PREFIX}{}", parts.join(",")))
}

/// The bots of a table clockwise from hero's left (`seat_profiles` has one
/// entry per seat, hero's included; `None` = the reference).
pub fn bots_from_left(
    seat_profiles: &[Option<String>],
    hero: Seat,
) -> impl Iterator<Item = Option<&str>> {
    let n = seat_profiles.len();
    (1..n).map(move |i| seat_profiles[(hero + i) % n].as_deref())
}

/// Whether the bots play different profiles.
pub fn is_mixed(seat_profiles: &[Option<String>], hero: Seat) -> bool {
    let mut bots = bots_from_left(seat_profiles, hero);
    let first = bots.next().flatten();
    bots.any(|p| p != first)
}

/// The profile every bot plays (`None` for the reference or a mixed
/// table).
pub fn uniform_profile(seat_profiles: &[Option<String>], hero: Seat) -> Option<&str> {
    if is_mixed(seat_profiles, hero) {
        return None;
    }
    bots_from_left(seat_profiles, hero).next().flatten()
}

/// What the app needs to rebuild the preflop tree (all embedded).
#[derive(Debug, Clone, Copy)]
pub struct GameSource {
    pub manifest: &'static str,
    pub full_chart: &'static [u8],
    pub equity: &'static [u8],
}

/// Lazily built [`TableGame`] and the best responses computed on it.
#[derive(Debug)]
pub struct MixedTables {
    source: Option<GameSource>,
    game: Mutex<Option<Result<TableGame, String>>>,
    /// Most recently used last.
    cache: Mutex<Vec<(TableId, Arc<TableBestResponse>)>>,
}

/// Hero's seat and what every other seat plays.
type TableId = (Seat, Vec<Option<String>>);

impl MixedTables {
    pub fn new(source: Option<GameSource>) -> MixedTables {
        MixedTables {
            source,
            game: Mutex::new(None),
            cache: Mutex::new(Vec::new()),
        }
    }

    /// Hero's best response at `hero` against `seats` (one entry per seat,
    /// hero's ignored; `None` = the reference), cached per table.
    pub fn best_response(
        &self,
        trainer: &PreflopTrainer,
        hero: Seat,
        seats: &[Option<String>],
    ) -> Result<Arc<TableBestResponse>, PreflopError> {
        let mut id: TableId = (hero, seats.to_vec());
        id.1[hero] = None;
        if let Some(br) = self.cached(&id) {
            return Ok(br);
        }
        let mut plays = Vec::with_capacity(seats.len());
        for (s, p) in id.1.iter().enumerate() {
            plays.push(match p {
                _ if s == hero => SeatPlay::Responder,
                None => SeatPlay::Reference,
                Some(pid) => SeatPlay::Profile(
                    trainer
                        .profile(pid)
                        .ok_or_else(|| PreflopError::UnknownProfile(pid.clone()))?
                        .bot
                        .profile(),
                ),
            });
        }
        let br = {
            let mut game = self.game.lock().unwrap_or_else(|e| e.into_inner());
            // Another thread may have computed it while this one waited.
            if let Some(br) = self.cached(&id) {
                return Ok(br);
            }
            let game = game.get_or_insert_with(|| self.build(trainer));
            let game = game
                .as_mut()
                .map_err(|e| PreflopError::NoTableGame(e.clone()))?;
            Arc::new(game.best_response(hero, &plays))
        };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.push((id, br.clone()));
        if cache.len() > CACHE {
            cache.remove(0);
        }
        Ok(br)
    }

    /// Builds the tree if needed; the error if it cannot be built.
    pub fn ready(&self, trainer: &PreflopTrainer) -> Result<(), PreflopError> {
        let mut game = self.game.lock().unwrap_or_else(|e| e.into_inner());
        match game.get_or_insert_with(|| self.build(trainer)) {
            Ok(_) => Ok(()),
            Err(e) => Err(PreflopError::NoTableGame(e.clone())),
        }
    }

    fn cached(&self, id: &TableId) -> Option<Arc<TableBestResponse>> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let at = cache.iter().position(|(k, _)| k == id)?;
        let hit = cache.remove(at);
        let br = hit.1.clone();
        cache.push(hit);
        Some(br)
    }

    fn build(&self, trainer: &PreflopTrainer) -> Result<TableGame, String> {
        let src = self
            .source
            .ok_or("this chart ships without the unpruned tree")?;
        let m = Manifest::from_toml(src.manifest)?;
        let tables =
            ClassTables::from_equity_bytes(src.equity, m.solve.equity_samples, m.solve.seed)
                .ok_or("equity table is corrupt")?;
        let full = PreflopChart::decode(src.full_chart).map_err(|e| e.to_string())?;
        if &m.game_config() != trainer.config() {
            return Err("the manifest is for another configuration".into());
        }
        TableGame::new(&m, tables, trainer.chart_arc(), full)
    }
}

#[cfg(test)]
mod tests {
    use taful_core::position::Position;

    use super::*;

    fn trainer() -> PreflopTrainer {
        PreflopTrainer::builtin().unwrap()
    }

    fn seats(ids: [&str; 6]) -> Vec<Option<String>> {
        ids.iter()
            .map(|id| (!id.is_empty() && *id != REFERENCE).then(|| id.to_string()))
            .collect()
    }

    #[test]
    fn table_keys_round_trip() {
        let k = "table:CO=nit,BTN=reference,BB=maniac";
        assert!(is_table_key(k));
        assert_eq!(
            parse_table_key(k).unwrap(),
            vec![
                (Position::Co, Some("nit".into())),
                (Position::Btn, None),
                (Position::Bb, Some("maniac".into())),
            ]
        );
        for bad in [
            "nit",
            "table:",
            "table:CO=nit",
            "table:CO=nit,CO=tag",
            "table:XX=nit,BB=tag",
            "table:CO=,BB=tag",
            "table:CO nit,BB=tag",
        ] {
            assert_eq!(parse_table_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn decisions_are_filed_under_the_live_opponents() {
        let t = trainer();
        let c = t.chart();
        let node = |line: &str| t.node_at(&line.parse().unwrap()).unwrap();
        // Seat 0 is the button: BTN, SB, BB, UTG, HJ, CO.
        let table = seats(["maniac", "nit", "nit", "", "tag", "tag"]);
        // UTG opens: everyone else is still in, and they differ.
        assert_eq!(
            opponent_at(c, node(""), &table, 3).as_deref(),
            Some("table:BTN=maniac,SB=nit,BB=nit,HJ=tag,CO=tag")
        );
        // Facing the button's open from the big blind: the button and the
        // small blind are still in.
        assert_eq!(
            opponent_at(c, node("UTG:f,HJ:f,CO:f,BTN:r250"), &table, 2).as_deref(),
            Some("table:BTN=maniac,SB=nit")
        );
        // After the small blind folds only the maniac is left: a plain
        // profile, the same key as a single-profile table.
        assert_eq!(
            opponent_at(c, node("UTG:f,HJ:f,CO:f,BTN:r250,SB:f"), &table, 2).as_deref(),
            Some("maniac")
        );
        // Only reference bots left: the reference.
        let refs = seats(["", "", "", "", "tag", "tag"]);
        assert_eq!(
            opponent_at(c, node("UTG:f,HJ:f,CO:f,BTN:r250,SB:f"), &refs, 2),
            None
        );
        // A uniform table never makes a table key.
        let nits = seats(["nit"; 6]);
        assert_eq!(opponent_at(c, node(""), &nits, 3).as_deref(), Some("nit"));
    }

    /// The on-demand best response at a table where every bot plays one
    /// profile is the shipped best-response chart for that profile: same
    /// tree, same locks on the same published data.
    #[test]
    fn a_uniform_table_matches_the_shipped_best_response() {
        let t = trainer();
        let c = t.chart();
        for (id, hero) in [("nit", 3), ("maniac", 2)] {
            let table: Vec<Option<String>> = (0..6).map(|_| Some(id.to_string())).collect();
            let br = t.mixed.best_response(&t, hero, &table).unwrap();
            let shipped = &t.profile(id).unwrap().chart;
            let pos = Position::of(6, 0, hero);
            let mut worst = 0.0f32;
            let mut nodes = 0;
            for n in (0..c.len()).filter(|&n| c.node(n).player == pos) {
                nodes += 1;
                for class in taful_core::range::HandClass::all() {
                    let hand = class.combos()[0];
                    let (a, b) = (br.ev(n, hand).unwrap(), shipped.ev(n, hand).unwrap());
                    for (x, y) in a.iter().zip(&b) {
                        worst = worst.max((x - y).abs());
                    }
                }
            }
            assert_eq!(br.len(), nodes, "{id}");
            // Same game and walk as the build: only the chart's EV
            // quantization separates them.
            let step = shipped.file().header.ev_scale;
            assert!(worst <= step, "{id}: {worst} bb (step {step})");
        }
    }

    #[test]
    fn table_keys_must_fit_the_table() {
        let t = trainer();
        assert!(t.knows_opponent("table:BTN=nit,BB=tag"));
        assert!(t.knows_opponent("table:BTN=reference,BB=tag"));
        // No lojack at 6-max, no such profile, not a key.
        assert!(!t.knows_opponent("table:LJ=nit,BB=tag"));
        assert!(!t.knows_opponent("table:BTN=nit,BB=nobody"));
        assert!(!t.knows_opponent("table:"));
        assert_eq!(t.seats_of_key("table:LJ=nit,BB=tag"), None);
        assert_eq!(
            t.seats_of_key("table:BTN=nit,BB=tag").unwrap(),
            seats(["nit", "", "tag", "", "", ""])
        );
        assert!(
            t.spot_against(
                crate::preflop::SpotKind::VsOpen,
                Some("table:LJ=nit,BB=tag"),
                1
            )
            .is_err()
        );
    }

    #[test]
    fn a_mixed_table_needs_the_tree() {
        let t = trainer();
        let bots: Vec<Option<String>> = ["nit", "tag", "tag", "tag", "tag"]
            .iter()
            .map(|id| Some(id.to_string()))
            .collect();
        assert!(t.check_mixed().is_ok());
        // A chart shipped without the unpruned tree cannot seat one: the
        // deal fails instead of a decision.
        let mut bare = PreflopTrainer::new(t.chart().clone(), t.thresholds().clone());
        for p in t.profiles() {
            bare.add_profile(p.bot.profile().clone(), (*p.chart).clone())
                .unwrap();
        }
        assert!(matches!(
            crate::hand::FullHand::at_table(&bare, None, None, 1, &bots),
            Err(crate::hand::HandError::Preflop(PreflopError::NoTableGame(
                _
            )))
        ));
        let uniform = vec![Some("nit".to_string()); 5];
        assert!(crate::hand::FullHand::at_table(&bare, None, None, 1, &uniform).is_ok());
    }

    #[test]
    fn best_responses_are_cached_per_table() {
        let t = trainer();
        let table = seats(["maniac", "nit", "nit", "", "tag", "tag"]);
        let a = t.mixed.best_response(&t, 3, &table).unwrap();
        let b = t.mixed.best_response(&t, 3, &table).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        // Hero's own entry does not matter.
        let mut other = table.clone();
        other[3] = Some("nit".into());
        assert!(Arc::ptr_eq(
            &a,
            &t.mixed.best_response(&t, 3, &other).unwrap()
        ));
        assert!(matches!(
            t.mixed
                .best_response(&t, 3, &seats(["nobody", "", "", "", "", ""])),
            Err(PreflopError::UnknownProfile(_))
        ));
        // A chart without the unpruned tree says so.
        let bare = PreflopTrainer::new(t.chart().clone(), t.thresholds().clone());
        assert!(matches!(
            bare.mixed
                .best_response(&bare, 3, &seats(["", "", "", "", "", ""])),
            Err(PreflopError::NoTableGame(_))
        ));
    }
}

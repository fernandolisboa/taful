//! The training loop's persistence (spec §10, §13, §14, M5): graded
//! decisions and hands are stored with their context, decisions below
//! `Correct` feed the FSRS queue by spot template, lines that leave the
//! library are counted as solve candidates, EV loss is aggregated into
//! leaks, and drills deal spots for a template or a leak bucket.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use taful_core::action::Street;
use taful_core::grading::Grade;
use taful_core::srs::Rating;
use taful_store::{DecisionContext, DecisionRow, HandRow, NewDecision, NewHand};

use crate::hand::{FullHand, HandEnd, Replay};
use crate::postflop::{
    LocalDir, PostflopError, PostflopSpot, PostflopTrainer, SpotRequest, default_library_dir,
};
use crate::preflop::{
    Action, GameState, GradedDecision, Hand, PreflopError, PreflopTrainer, Seat, Spot,
};
use crate::review::{ItemKind, Source};
use crate::template::{
    LeakBucket, NodeType, PREFLOP, SpotContext, SpotTemplate, parse_street, short_config_key,
    street_id,
};
use crate::{App, Error, Result};

/// Decisions in the rolling leak window (spec §13).
pub const LEAK_WINDOW: u32 = 500;
/// Leaks shown on the home screen.
pub const TOP_LEAKS: usize = 3;
/// Hands kept in the hand-review list.
pub const RECENT_HANDS: u32 = 30;

/// Why a spot template could not be dealt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateError {
    /// The template no longer exists (other configuration, chart or
    /// library rebuilt): its review item can be retired.
    Gone,
    /// No deal matched this time (unlucky seeds, library not loaded).
    Unavailable,
}

/// The reference strategies: the built-in preflop chart and, when installed,
/// the postflop library.
#[derive(Debug)]
pub struct Trainers {
    pub preflop: Arc<PreflopTrainer>,
    pub postflop: std::result::Result<Arc<PostflopTrainer>, PostflopError>,
    /// Where the postflop library was looked for.
    pub library_dir: PathBuf,
}

impl Trainers {
    /// The built-in chart and the library in `library_dir`.
    pub fn open(library_dir: PathBuf) -> std::result::Result<Trainers, PreflopError> {
        let preflop = Arc::new(PreflopTrainer::builtin()?);
        let postflop = PostflopTrainer::open(
            Box::new(LocalDir(library_dir.clone())),
            preflop.config().clone(),
            preflop.thresholds().clone(),
        )
        .map(Arc::new);
        Ok(Trainers {
            preflop,
            postflop,
            library_dir,
        })
    }

    pub fn post(&self) -> Option<&PostflopTrainer> {
        self.postflop.as_deref().ok()
    }

    pub fn config_key(&self) -> String {
        short_config_key(self.preflop.config())
    }

    /// A spot of `template` (a review of a past mistake, spec §13): same
    /// configuration, line, history and hero position, a fresh board of the
    /// same family and fresh hands.
    pub fn spot_for_template(
        &self,
        t: &SpotTemplate,
        seed: u64,
    ) -> std::result::Result<DrillSpot, TemplateError> {
        if t.config != self.config_key() {
            return Err(TemplateError::Gone);
        }
        if t.is_preflop() {
            let node = self.preflop.node_at(&t.line).ok_or(TemplateError::Gone)?;
            if self.preflop.chart().node(node).player != t.hero {
                return Err(TemplateError::Gone);
            }
            let spot = self
                .preflop
                .spot_at(node, seed)
                .map_err(|_| TemplateError::Unavailable)?;
            return Ok(DrillSpot::Preflop(Box::new(spot)));
        }
        let post = self
            .postflop
            .as_ref()
            .map_err(|_| TemplateError::Unavailable)?;
        // A library without this spot may be partial or older (it grows,
        // D13): the item waits rather than losing its history.
        if !post.spots().iter().any(|s| s.id == t.spot) {
            return Err(TemplateError::Unavailable);
        }
        post.generate(
            &SpotRequest {
                spot: Some(&t.spot),
                texture: t.texture.as_deref(),
                script: Some(&t.postflop),
                hero: Some(t.hero),
                ..SpotRequest::default()
            },
            seed,
        )
        .map(|s| DrillSpot::Postflop(Box::new(s)))
        .map_err(|_| TemplateError::Unavailable)
    }

    /// A spot from a leak bucket (the "drill this" action, spec §13).
    pub fn spot_for_bucket(
        &self,
        b: &LeakBucket,
        seed: u64,
    ) -> std::result::Result<DrillSpot, PostflopError> {
        if b.spot == PREFLOP {
            let NodeType::Preflop(kind) = b.node_type else {
                return Err(PostflopError::Empty);
            };
            return self
                .preflop
                .spot(kind, seed)
                .map(|s| DrillSpot::Preflop(Box::new(s)))
                .map_err(|_| PostflopError::Empty);
        }
        let post = self.postflop.as_ref().map_err(Clone::clone)?;
        post.generate(
            &SpotRequest {
                spot: Some(&b.spot),
                texture: b.texture.as_deref(),
                street: Some(b.street),
                node_type: Some(b.node_type),
                ..SpotRequest::default()
            },
            seed,
        )
        .map(|s| DrillSpot::Postflop(Box::new(s)))
    }

    /// Grade hero's answer to a drill spot.
    pub fn answer(
        &self,
        spot: &DrillSpot,
        action: Action,
    ) -> std::result::Result<GradedDecision, PostflopError> {
        match spot {
            DrillSpot::Preflop(s) => s.answer(&self.preflop, action).map_err(|e| match e {
                PreflopError::NotInTree(a) => PostflopError::NotInTree(a),
                _ => PostflopError::NoData,
            }),
            DrillSpot::Postflop(s) => {
                let post = self.postflop.as_ref().map_err(Clone::clone)?;
                s.answer(post, action)
            }
        }
    }
}

/// A single-decision question: preflop or postflop.
#[derive(Debug, Clone)]
pub enum DrillSpot {
    Preflop(Box<Spot>),
    Postflop(Box<PostflopSpot>),
}

impl DrillSpot {
    pub fn state(&self) -> &GameState {
        match self {
            DrillSpot::Preflop(s) => &s.state,
            DrillSpot::Postflop(s) => &s.state,
        }
    }
    pub fn hero(&self) -> Seat {
        match self {
            DrillSpot::Preflop(s) => s.hero,
            DrillSpot::Postflop(s) => s.hero,
        }
    }
    pub fn hand(&self) -> Hand {
        match self {
            DrillSpot::Preflop(s) => s.hand,
            DrillSpot::Postflop(s) => s.hand,
        }
    }
    pub fn actions(&self, t: &Trainers) -> Vec<Action> {
        match self {
            DrillSpot::Preflop(s) => t.preflop.actions(s.node),
            DrillSpot::Postflop(s) => s.actions(),
        }
    }
    /// The spot's template and leak bucket.
    pub fn context(&self, t: &Trainers) -> SpotContext {
        let position = self.state().position(self.hero());
        match self {
            DrillSpot::Preflop(s) => SpotContext {
                template: SpotTemplate {
                    config: t.config_key(),
                    spot: PREFLOP.into(),
                    line: t.preflop.chart().line(s.node),
                    postflop: Vec::new(),
                    texture: None,
                    hero: position,
                },
                street: Street::Preflop,
                node_type: NodeType::Preflop(s.kind),
            },
            DrillSpot::Postflop(s) => {
                crate::hand::postflop_context(&t.config_key(), s.walk(), position)
            }
        }
    }
}

/// A leak (spec §13): a bucket with its EV loss over the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Leak {
    pub bucket: LeakBucket,
    pub decisions: u32,
    pub mistakes: u32,
    pub total_ev_loss_bb: f64,
}

impl Leak {
    pub fn avg_ev_loss_bb(&self) -> f64 {
        self.total_ev_loss_bb / f64::from(self.decisions.max(1))
    }
}

/// A stored hand for the hand-review list.
#[derive(Debug, Clone, PartialEq)]
pub struct HandSummary {
    pub id: String,
    pub played_at: taful_store::Millis,
    pub hero_hand: Option<Hand>,
    pub ended_reason: String,
    pub total_ev_loss_bb: f64,
    pub hero_net_bb: Option<f64>,
    pub decisions: usize,
    /// The worst grade among hero's decisions.
    pub worst: Option<Grade>,
}

/// One stored decision for the hand review.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewedDecision {
    pub step: usize,
    pub street: Option<Street>,
    pub hero_combo: String,
    pub actions: Vec<Action>,
    pub chosen: usize,
    pub hand_evs: Vec<f32>,
    pub hand_freqs: Vec<f32>,
    pub range_freqs: Vec<f32>,
    pub pot_bb: f64,
    pub ev_loss_bb: f64,
    pub grade: Grade,
}

/// A stored hand, ready to step through (spec §15.1 #5).
#[derive(Debug, Clone, PartialEq)]
pub struct HandReview {
    pub summary: HandSummary,
    pub hero: Seat,
    /// Every table state, from the deal to the end.
    pub states: Vec<GameState>,
    pub decisions: Vec<ReviewedDecision>,
}

fn grade_from_id(id: &str) -> Option<Grade> {
    Grade::ALL.into_iter().find(|g| g.id() == id)
}

fn decision_json(d: &GradedDecision) -> (serde_json::Value, serde_json::Value) {
    let actions: Vec<String> = d.actions.iter().map(|a| a.to_string()).collect();
    (
        json!({ "actions": actions, "ev": d.hand_evs }),
        json!({ "hand": d.hand_freqs, "range": d.range_freqs }),
    )
}

impl App {
    /// The reference strategies, opened on first use (the library folder:
    /// `TAFUL_LIBRARY_DIR`, else `<data dir>/library`).
    pub fn trainers(&mut self) -> Result<Arc<Trainers>> {
        if let Some(t) = &self.trainers {
            return Ok(t.clone());
        }
        let dir = match &self.library_dir {
            Some(d) => d.clone(),
            None => default_library_dir(
                self.data_dir
                    .as_deref()
                    .unwrap_or(&crate::default_data_dir()),
            ),
        };
        let t = Arc::new(Trainers::open(dir).map_err(|e| Error::NotFound(e.to_string()))?);
        self.trainers = Some(t.clone());
        Ok(t)
    }

    /// Loads the library in `dir` and, if it opens, remembers the folder
    /// (settings). A folder without a usable library changes nothing.
    pub fn set_library_folder(&mut self, dir: PathBuf) -> Result<Arc<Trainers>> {
        let t = Trainers::open(dir.clone()).map_err(|e| Error::NotFound(e.to_string()))?;
        if let Err(e) = &t.postflop {
            return Err(Error::NotFound(format!(
                "library in {}: {e}",
                dir.display()
            )));
        }
        self.store.set_setting(
            crate::keys::LIBRARY_DIR,
            &serde_json::json!(dir.to_string_lossy()),
        )?;
        let t = Arc::new(t);
        self.library_dir = Some(dir);
        self.trainers = Some(t.clone());
        Ok(t)
    }

    /// Use the library in `dir` (tests, or a folder picked in settings).
    pub fn set_library_dir(&mut self, dir: PathBuf) {
        self.library_dir = Some(dir);
        self.trainers = None;
    }

    fn store_decision(
        &self,
        hand_id: Option<&str>,
        d: &GradedDecision,
        ctx: &SpotContext,
        step: Option<usize>,
    ) -> Result<String> {
        let (ev, freqs) = decision_json(d);
        let template_key = ctx.template.key();
        let texture = ctx.template.texture.clone();
        Ok(self.store.insert_decision(&NewDecision {
            hand_id,
            node_key: &format!("{}#{}", template_key, d.node),
            hero_combo: &d.hand.to_string(),
            chosen_action: &d.actions[d.chosen].to_string(),
            ev_per_action: ev,
            frequencies: freqs,
            pot: d.pot_bb,
            ev_loss: d.ev_loss_bb,
            grade: d.grade.id(),
            thresholds_version: d.thresholds_version,
            context: Some(DecisionContext {
                template_key: &template_key,
                spot: &ctx.template.spot,
                street: street_id(ctx.street),
                node_type: ctx.node_type.id(),
                texture: texture.as_deref(),
                step: step.map(|s| s as u32),
            }),
        })?)
    }

    /// Feed the FSRS queue (spec §13): a decision below `Correct` creates or
    /// updates its template's item; any decision on an existing item reviews
    /// it.
    fn review_decision(&self, d: &GradedDecision, ctx: &SpotContext, id: &str) -> Result<()> {
        let key = ctx.template.key();
        let exists = self
            .store
            .review_item(ItemKind::SpotTemplate.id(), &key)?
            .is_some();
        if exists || d.grade > Grade::Correct {
            self.scheduler.review(
                &self.store,
                ItemKind::SpotTemplate,
                &key,
                None,
                Rating::from_grade(d.grade),
                Some(Source {
                    kind: "decision",
                    id,
                }),
            )?;
        }
        Ok(())
    }

    /// Store a spot-mode decision (no hand) and feed the queue. With
    /// `review_item`, the decision answers that due item.
    pub fn record_spot_decision(
        &mut self,
        d: &GradedDecision,
        ctx: &SpotContext,
        review_item: Option<&str>,
    ) -> Result<()> {
        let id = self.store_decision(None, d, ctx, None)?;
        match review_item.and_then(|i| self.store.review_item_by_id(i).transpose()) {
            Some(item) => {
                self.scheduler.review_item(
                    &self.store,
                    &item?,
                    Rating::from_grade(d.grade),
                    None,
                    Some(Source {
                        kind: "decision",
                        id: &id,
                    }),
                )?;
            }
            None => self.review_decision(d, ctx, &id)?,
        }
        Ok(())
    }

    /// Store a finished full hand with its decisions; feed the queue and
    /// count a line that left the library. Returns the hand id.
    pub fn record_hand(&mut self, h: &FullHand) -> Result<String> {
        // One hand, all or nothing: a half-stored hand would skew leaks.
        self.store.atomic(|_| self.record_hand_rows(h))
    }

    fn record_hand_rows(&self, h: &FullHand) -> Result<String> {
        let config = h.state.config();
        let replay = serde_json::to_value(h.replay()).map_err(taful_store::StoreError::from)?;
        let id = self.store.insert_hand(&NewHand {
            mode: "full_hand",
            config_key: &short_config_key(config),
            config_toml: &config.to_toml(),
            seed: h.seed,
            hero_seat: h.hero as u32,
            bot_ids: json!(["reference"]),
            library_format_version: Some(u32::from(taful_solver::format::FORMAT_VERSION)),
            replay: Some(replay),
        })?;
        for d in &h.decisions {
            let did = self.store_decision(Some(&id), &d.graded, &d.context, Some(d.step))?;
            self.review_decision(&d.graded, &d.context, &did)?;
        }
        if let Some(HandEnd::LeftLibrary { history, reason }) = &h.end
            && reason.is_candidate()
        {
            let line = history.line().to_string();
            let post = history
                .streets
                .iter()
                .skip(1)
                .map(|s| taful_core::action::Line(s.clone()).to_string())
                .collect::<Vec<_>>()
                .join("/");
            self.store
                .note_solve_candidate(&short_config_key(config), &line, &post)?;
        }
        let reason = h.end.as_ref().map_or("unfinished".into(), HandEnd::id);
        self.store
            .finish_hand(&id, &reason, h.total_ev_loss_bb(), h.hero_net_bb())?;
        Ok(id)
    }

    /// The top leaks over the rolling window (spec §13): buckets with at
    /// least one mistake, by total EV loss.
    pub fn leaks(&self) -> Result<Vec<Leak>> {
        Ok(self
            .store
            .bucket_stats(LEAK_WINDOW)?
            .into_iter()
            .filter(|b| b.mistakes > 0 && b.total_ev_loss > 0.0)
            .filter_map(|b| {
                Some(Leak {
                    bucket: LeakBucket {
                        street: parse_street(&b.street)?,
                        node_type: NodeType::parse(&b.node_type)?,
                        spot: b.spot,
                        texture: b.texture,
                    },
                    decisions: b.decisions,
                    mistakes: b.mistakes,
                    total_ev_loss_bb: b.total_ev_loss,
                })
            })
            .take(TOP_LEAKS)
            .collect())
    }

    fn summary(&self, h: &HandRow, decisions: &[DecisionRow]) -> HandSummary {
        let hero_hand = h
            .replay
            .as_ref()
            .and_then(|r| serde_json::from_value::<Replay>(r.clone()).ok())
            .and_then(|r| r.holes.get(h.hero_seat as usize)?.parse().ok());
        HandSummary {
            id: h.id.clone(),
            played_at: h.created_at,
            hero_hand,
            ended_reason: h.ended_reason.clone().unwrap_or_default(),
            total_ev_loss_bb: h.total_ev_loss.unwrap_or(0.0),
            hero_net_bb: h.hero_net,
            decisions: decisions.len(),
            worst: decisions
                .iter()
                .filter_map(|d| grade_from_id(&d.grade))
                .max(),
        }
    }

    /// The latest finished full hands, newest first.
    pub fn recent_hands(&self) -> Result<Vec<HandSummary>> {
        let hands = self.store.recent_hands("full_hand", RECENT_HANDS)?;
        hands
            .iter()
            .map(|h| Ok(self.summary(h, &self.store.hand_decisions(&h.id)?)))
            .collect()
    }

    /// A stored hand rebuilt for review and replay.
    pub fn hand_review(&self, id: &str) -> Result<HandReview> {
        let h = self
            .store
            .hand(id)?
            .ok_or_else(|| Error::NotFound(format!("hand `{id}`")))?;
        let rows = self.store.hand_decisions(id)?;
        let config = taful_core::config::GameConfig::from_toml(&h.config_toml)
            .map_err(|e| Error::NotFound(format!("hand `{id}` config: {e}")))?;
        let replay: Replay = h
            .replay
            .clone()
            .and_then(|r| serde_json::from_value(r).ok())
            .ok_or_else(|| Error::NotFound(format!("hand `{id}` replay")))?;
        let states = replay.states(&config).map_err(Error::NotFound)?;
        let floats = |v: &serde_json::Value| -> Vec<f32> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64())
                        .map(|x| x as f32)
                        .collect()
                })
                .unwrap_or_default()
        };
        let decisions = rows
            .iter()
            .filter_map(|d| {
                let actions: Vec<Action> = d.ev_per_action["actions"]
                    .as_array()?
                    .iter()
                    .filter_map(|a| a.as_str()?.parse().ok())
                    .collect();
                let chosen_action: Action = d.chosen_action.parse().ok()?;
                Some(ReviewedDecision {
                    step: d.step? as usize,
                    street: d.street.as_deref().and_then(parse_street),
                    hero_combo: d.hero_combo.clone(),
                    chosen: actions.iter().position(|&a| a == chosen_action)?,
                    hand_evs: floats(&d.ev_per_action["ev"]),
                    hand_freqs: floats(&d.frequencies["hand"]),
                    range_freqs: floats(&d.frequencies["range"]),
                    actions,
                    pot_bb: d.pot,
                    ev_loss_bb: d.ev_loss,
                    grade: grade_from_id(&d.grade)?,
                })
            })
            .filter(|d| d.step < states.len())
            .collect();
        Ok(HandReview {
            summary: self.summary(&h, &rows),
            hero: h.hero_seat as usize,
            states,
            decisions,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::services::tests::app_at;

    fn app() -> App {
        let (mut app, _) = app_at(1_000_000);
        app.set_library_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop"));
        app
    }

    #[test]
    fn trainers_open_with_the_fixture() {
        let mut a = app();
        let t = a.trainers().unwrap();
        assert!(t.post().is_some());
        // A missing library leaves preflop working.
        a.set_library_dir("/nonexistent/taful".into());
        let t = a.trainers().unwrap();
        assert!(t.post().is_none());
    }

    #[test]
    fn library_folder_is_saved_only_when_it_opens() {
        let (mut a, _) = app_at(1_000_000);
        assert!(a.set_library_folder("/nonexistent/taful".into()).is_err());
        assert_eq!(a.string_setting(crate::keys::LIBRARY_DIR), None);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop");
        let t = a.set_library_folder(dir.clone()).unwrap();
        assert!(t.post().is_some());
        assert_eq!(
            a.string_setting(crate::keys::LIBRARY_DIR).as_deref(),
            Some(&*dir.to_string_lossy())
        );
    }

    #[test]
    fn mistakes_become_review_items_and_leaks() {
        let mut a = app();
        let t = a.trainers().unwrap();
        // Spot mode: answer the worst action every time.
        let mut keys = std::collections::HashSet::new();
        for seed in 0..30 {
            let spot = if seed % 2 == 0 {
                DrillSpot::Preflop(Box::new(
                    t.preflop.spot(crate::preflop::SpotKind::Rfi, seed).unwrap(),
                ))
            } else {
                DrillSpot::Postflop(Box::new(t.post().unwrap().spot(None, seed).unwrap()))
            };
            let acts = spot.actions(&t);
            let first = t.answer(&spot, acts[0]).unwrap();
            let worst = (0..acts.len())
                .min_by(|&x, &y| first.hand_evs[x].total_cmp(&first.hand_evs[y]))
                .unwrap();
            let d = t.answer(&spot, acts[worst]).unwrap();
            let ctx = spot.context(&t);
            if d.grade > Grade::Correct {
                keys.insert(ctx.template.key());
            }
            a.record_spot_decision(&d, &ctx, None).unwrap();
        }
        assert!(!keys.is_empty());
        let (items, _) = a.store.review_totals().unwrap();
        assert_eq!(items as usize, keys.len());
        for k in &keys {
            let item = a.store.review_item("spot_template", k).unwrap().unwrap();
            // Every template deals a fresh spot of the same template.
            let tpl = SpotTemplate::parse(&item.key).unwrap();
            for seed in 0..3 {
                let s = t.spot_for_template(&tpl, seed).unwrap();
                assert_eq!(s.context(&t).template, tpl, "{k}");
            }
        }
        let leaks = a.leaks().unwrap();
        assert!(!leaks.is_empty() && leaks.len() <= TOP_LEAKS);
        assert!(
            leaks
                .windows(2)
                .all(|w| w[0].total_ev_loss_bb >= w[1].total_ev_loss_bb)
        );
        for l in &leaks {
            let s = t.spot_for_bucket(&l.bucket, 7).unwrap();
            assert_eq!(s.context(&t).bucket(), l.bucket);
        }
    }

    #[test]
    fn river_buckets_and_templates_deal_solved_rivers() {
        let a = app();
        let mut a = a;
        let t = a.trainers().unwrap();
        let post = t.post().unwrap();
        let s = post
            .generate(
                &SpotRequest {
                    street: Some(Street::River),
                    ..SpotRequest::default()
                },
                3,
            )
            .unwrap();
        assert_eq!(s.board.len(), 5);
        assert!(s.walk().on_solved_river());
        let spot = DrillSpot::Postflop(Box::new(s));
        let ctx = spot.context(&t);
        assert_eq!(ctx.street, Street::River);
        let again = t.spot_for_template(&ctx.template, 11).unwrap();
        assert_eq!(again.context(&t).template, ctx.template);
        let acts = again.actions(&t);
        assert!(t.answer(&again, acts[0]).is_ok());
    }

    #[test]
    fn hands_are_stored_and_replayed() {
        let mut a = app();
        let t = a.trainers().unwrap();
        let mut stored = 0;
        for seed in 0..40 {
            let mut h = FullHand::new(&t.preflop, t.post(), None, seed).unwrap();
            while h.end.is_none() {
                let acts = h.hero_actions(&t.preflop);
                // Hero always takes the last option (often a raise).
                h.act(&t.preflop, t.post(), *acts.last().unwrap()).unwrap();
            }
            let id = a.record_hand(&h).unwrap();
            stored += 1;
            let r = a.hand_review(&id).unwrap();
            assert_eq!(r.decisions.len(), h.decisions.len());
            assert_eq!(r.states.last().unwrap().board(), h.state.board());
            for (rd, hd) in r.decisions.iter().zip(&h.decisions) {
                assert_eq!(rd.step, hd.step);
                assert_eq!(rd.grade, hd.graded.grade);
                assert_eq!(r.states[rd.step].to_act(), Some(h.hero));
            }
            assert_eq!(r.summary.hero_hand, Some(h.hero_hand()));
        }
        let recent = a.recent_hands().unwrap();
        assert_eq!(recent.len(), stored.min(RECENT_HANDS as usize));
        // Unsolved lines were counted as solve candidates.
        assert!(!a.store.solve_candidates(5).unwrap().is_empty());
    }
}

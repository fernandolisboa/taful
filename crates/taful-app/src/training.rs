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
use taful_core::iso::Flop;
use taful_core::srs::Rating;
use taful_store::{DecisionContext, DecisionRow, HandRow, NewDecision, NewHand};

use crate::hand::{FullHand, HandEnd, Replay};
use crate::mixed::is_table_key;
use crate::postflop::{
    DealtFlop, LocalDir, PostflopError, PostflopSpot, PostflopTrainer, SpotRequest,
    default_library_dir,
};
use crate::preflop::{
    Action, GameState, GradedDecision, Hand, PreflopError, PreflopTrainer, Seat, Spot, VsProfile,
};
use crate::review::{ItemKind, Source};
use crate::template::{
    LeakBucket, NodeType, PREFLOP, SpotContext, SpotTemplate, opponent_id, parse_opponent,
    parse_street, short_config_key, street_id,
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

    /// Whether this build can deal against `opponent` (`None` = the
    /// reference bot, always).
    pub fn knows_opponent(&self, opponent: Option<&str>) -> bool {
        opponent.is_none_or(|id| self.preflop.knows_opponent(id))
    }

    /// A spot of `template` (a review of a past mistake, spec §13): same
    /// configuration, line, history, hero position and opponent, a fresh
    /// board of the same family and fresh hands.
    pub fn spot_for_template(
        &self,
        t: &SpotTemplate,
        seed: u64,
    ) -> std::result::Result<DrillSpot, TemplateError> {
        if t.config != self.config_key() {
            return Err(TemplateError::Gone);
        }
        // A profile this build no longer ships is gone with its items.
        if !self.knows_opponent(t.opponent.as_deref()) {
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
            // Deals run in the background, answers on the UI thread: a
            // table's best response is computed now.
            if let Some(key) = t.opponent.as_deref().filter(|k| is_table_key(k)) {
                self.preflop
                    .warm_table(key, node)
                    .map_err(|_| TemplateError::Unavailable)?;
            }
            return Ok(DrillSpot::Preflop(Box::new(
                spot.against(t.opponent.as_deref()),
            )));
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
        post.generate_unnoted(
            &SpotRequest {
                spot: Some(&t.spot),
                texture: t.texture.as_deref(),
                script: Some(&t.postflop),
                hero: Some(t.hero),
                opponent: t.opponent.as_deref(),
                ..SpotRequest::default()
            },
            seed,
        )
        // A library without the profile's data (reference-only, #27) deals
        // a reference spot: the item waits instead of being graded against
        // the wrong opponent.
        .ok()
        .filter(|s| s.fallback.is_none())
        .inspect(|s| post.note_spot(s))
        .map(|s| DrillSpot::Postflop(Box::new(s)))
        .ok_or(TemplateError::Unavailable)
    }

    /// A spot from a leak bucket (the "drill this" action, spec §13),
    /// against the bucket's opponent.
    pub fn spot_for_bucket(
        &self,
        b: &LeakBucket,
        seed: u64,
    ) -> std::result::Result<DrillSpot, PostflopError> {
        if !self.knows_opponent(b.opponent.as_deref()) {
            return Err(PostflopError::Empty);
        }
        if b.spot == PREFLOP {
            let NodeType::Preflop(kind) = b.node_type else {
                return Err(PostflopError::Empty);
            };
            let spot = self
                .preflop
                .spot_against(kind, b.opponent.as_deref(), seed)
                .map_err(|_| PostflopError::Empty)?;
            if let Some(key) = b.opponent.as_deref().filter(|k| is_table_key(k)) {
                self.preflop
                    .warm_table(key, spot.node)
                    .map_err(|_| PostflopError::Empty)?;
            }
            return Ok(DrillSpot::Preflop(Box::new(
                spot.against(b.opponent.as_deref()),
            )));
        }
        let post = self.postflop.as_ref().map_err(Clone::clone)?;
        let s = post.generate_unnoted(
            &SpotRequest {
                spot: Some(&b.spot),
                texture: b.texture.as_deref(),
                street: Some(b.street),
                node_type: Some(b.node_type),
                opponent: b.opponent.as_deref(),
                ..SpotRequest::default()
            },
            seed,
        )?;
        // Drilling a leak against a profile on a library without its data
        // would practise against the wrong opponent.
        if s.fallback.is_some() {
            return Err(PostflopError::Empty);
        }
        post.note_spot(&s);
        Ok(DrillSpot::Postflop(Box::new(s)))
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
                    opponent: s.opponent.clone(),
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
    /// Graded against a bot profile: `hand_evs` are hero's best-response
    /// EVs against it, and this holds the reference EVs and both lines.
    pub vs_profile: Option<VsProfile>,
}

/// A stored hand, ready to step through (spec §15.1 #5).
#[derive(Debug, Clone, PartialEq)]
pub struct HandReview {
    pub summary: HandSummary,
    pub hero: Seat,
    /// Who sat at each seat: `None` for hero and reference bots, `Some(id)`
    /// for a bot profile (#28).
    pub seat_profiles: Vec<Option<String>>,
    /// Every table state, from the deal to the end.
    pub states: Vec<GameState>,
    pub decisions: Vec<ReviewedDecision>,
}

fn grade_from_id(id: &str) -> Option<Grade> {
    Grade::ALL.into_iter().find(|g| g.id() == id)
}

fn decision_json(d: &GradedDecision) -> (serde_json::Value, serde_json::Value) {
    let actions: Vec<String> = d.actions.iter().map(|a| a.to_string()).collect();
    let mut ev = json!({ "actions": actions, "ev": d.hand_evs });
    // Against a profile `ev` is against the profile; the reference EVs and
    // both lines come along for the review (spec §8 payload).
    if let Some(v) = &d.vs_profile {
        ev["vs"] = json!({
            "profile": v.profile,
            "reference_ev": v.reference_evs,
            "gto": v.gto,
            "exploit": v.exploit,
            "ev_gap": v.ev_gap_bb,
        });
    }
    (ev, json!({ "hand": d.hand_freqs, "range": d.range_freqs }))
}

fn floats(v: &serde_json::Value) -> Vec<f32> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_f64())
                .map(|x| x as f32)
                .collect()
        })
        .unwrap_or_default()
}

/// The `vs` part of a stored decision's `ev_per_action`, if any.
fn vs_from_json(ev: &serde_json::Value) -> Option<VsProfile> {
    let v = ev.get("vs")?;
    Some(VsProfile {
        profile: v["profile"].as_str()?.to_string(),
        reference_evs: floats(&v["reference_ev"]),
        gto: usize::try_from(v["gto"].as_u64()?).ok()?,
        exploit: usize::try_from(v["exploit"].as_u64()?).ok()?,
        ev_gap_bb: v["ev_gap"].as_f64()?,
    })
}

/// Who sat at each of `seats` seats, from `hands.bot_ids`: one entry per
/// seat since #28; older hands (`["reference"]`) were all reference.
fn seat_profiles(bot_ids: &serde_json::Value, seats: usize) -> Vec<Option<String>> {
    match bot_ids.as_array() {
        Some(ids) if ids.len() == seats => ids
            .iter()
            .map(|id| {
                id.as_str()
                    .filter(|id| !matches!(*id, "hero" | taful_core::profile::REFERENCE))
                    .map(str::to_owned)
            })
            .collect(),
        _ => vec![None; seats],
    }
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
        self.start_cycles(&t)?;
        self.trainers = Some(t.clone());
        Ok(t)
    }

    /// Deal postflop flops in cycles (#51), from where the store left off.
    fn start_cycles(&self, t: &Trainers) -> Result<()> {
        let Some(post) = t.post() else {
            return Ok(());
        };
        let dealt = self
            .store
            .dealt_flops(&t.config_key())?
            .into_iter()
            .filter_map(|(spot, cycle, flop)| {
                Some(DealtFlop {
                    spot,
                    cycle,
                    flop: flop.parse::<Flop>().ok()?.canonical().0,
                })
            })
            .collect::<Vec<_>>();
        post.start_cycles(dealt);
        Ok(())
    }

    /// Store the flops dealt since the last call (#51).
    fn store_dealt(&self) -> Result<()> {
        let Some(t) = &self.trainers else {
            return Ok(());
        };
        let Some(post) = t.post() else {
            return Ok(());
        };
        let key = t.config_key();
        for d in post.take_dealt() {
            self.store
                .note_dealt_flop(&key, &d.spot, d.cycle, &d.flop.to_string())?;
        }
        Ok(())
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
        self.start_cycles(&t)?;
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
        // Decisions are keyed by who they were graded against (#26): a
        // context that names another opponent would file the mistake under
        // the wrong template and leak.
        let graded_vs = d.vs_profile.as_ref().map(|v| v.profile.as_str());
        if graded_vs != ctx.template.opponent.as_deref() {
            return Err(Error::Invalid(format!(
                "a decision graded against `{}` filed under `{}`",
                opponent_id(graded_vs),
                opponent_id(ctx.template.opponent.as_deref())
            )));
        }
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
                opponent: opponent_id(ctx.template.opponent.as_deref()),
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
        // The decision and its review land together, or neither does: a
        // decision without its review would leave the item due forever.
        self.store.atomic(|_| {
            let id = self.store_decision(None, d, ctx, None)?;
            match review_item.and_then(|i| self.store.review_item_by_id(i).transpose()) {
                Some(item) => {
                    let item = item?;
                    // A review answered against another opponent would
                    // reschedule the wrong item.
                    let item_vs = SpotTemplate::parse(&item.key).and_then(|t| t.opponent);
                    if item_vs != ctx.template.opponent {
                        return Err(Error::Invalid(format!(
                            "review item `{}` answered against `{}`",
                            item.key,
                            opponent_id(ctx.template.opponent.as_deref())
                        )));
                    }
                    self.scheduler.review_item(
                        &self.store,
                        &item,
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
            self.store_dealt()
        })
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
            bot_ids: json!(
                (0..h.seat_profiles.len())
                    .map(|s| match &h.seat_profiles[s] {
                        _ if s == h.hero => "hero",
                        Some(id) => id.as_str(),
                        None => taful_core::profile::REFERENCE,
                    })
                    .collect::<Vec<_>>()
            ),
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
        self.store_dealt()?;
        Ok(id)
    }

    /// The top leaks over the rolling window (spec §13): buckets with at
    /// least one mistake, by total EV loss. Buckets against a profile this
    /// build no longer ships are left out (they cannot be drilled).
    pub fn leaks(&mut self) -> Result<Vec<Leak>> {
        let trainers = self.trainers()?;
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
                        opponent: parse_opponent(&b.opponent),
                    },
                    decisions: b.decisions,
                    mistakes: b.mistakes,
                    total_ev_loss_bb: b.total_ev_loss,
                })
            })
            .filter(|l| trainers.knows_opponent(l.bucket.opponent.as_deref()))
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
                    vs_profile: vs_from_json(&d.ev_per_action),
                })
            })
            .filter(|d| d.step < states.len())
            .collect();
        Ok(HandReview {
            summary: self.summary(&h, &rows),
            hero: h.hero_seat as usize,
            seat_profiles: seat_profiles(&h.bot_ids, usize::from(config.table_size)),
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

    /// #25 acceptance: through `App`, a preflop spot against a profile is
    /// graded on the profile's best-response EVs, the payload carries both
    /// EV sets, and reference grades are unchanged.
    #[test]
    fn spots_against_a_profile_grade_on_best_responses() {
        use crate::preflop::SpotKind;
        let mut a = app();
        let t = a.trainers().unwrap();
        let pre = &t.preflop;
        let mut checked = 0;
        for seed in 0..40 {
            let spot = pre.spot(SpotKind::VsOpen, seed).unwrap();
            let station =
                DrillSpot::Preflop(Box::new(spot.clone().against(Some("calling-station"))));
            let reference = DrillSpot::Preflop(Box::new(spot.clone()));
            let br = &pre.profile("calling-station").unwrap().chart;
            for (i, &act) in spot_actions(&reference, &t).iter().enumerate() {
                let r = t.answer(&reference, act).unwrap();
                // Reference grades are what they always were.
                assert_eq!(r, pre.grade(spot.node, spot.hand, i).unwrap());
                assert!(r.vs_profile.is_none());
                let d = t.answer(&station, act).unwrap();
                let vs = d.vs_profile.as_ref().expect("graded against the profile");
                assert_eq!(vs.profile, "calling-station");
                assert_eq!(d.hand_evs, br.ev(spot.node, spot.hand).unwrap());
                assert_eq!(vs.reference_evs, r.hand_evs);
                assert_eq!(d.hand_freqs, r.hand_freqs);
                let best = d.hand_evs.iter().copied().fold(f32::MIN, f32::max);
                assert!((d.ev_loss_bb - f64::from(best - d.hand_evs[i])).abs() < 1e-9);
                assert_eq!(vs.exploit, d.best());
                assert_eq!(vs.gto, crate::preflop::argmax(&r.hand_freqs));
                assert!(
                    (vs.ev_gap_bb - f64::from(d.hand_evs[vs.exploit] - d.hand_evs[vs.gto])).abs()
                        < 1e-9
                );
                checked += 1;
            }
            // The exploitative line is always Best against the profile.
            let exploit = spot_actions(&station, &t)[t
                .answer(&station, spot_actions(&station, &t)[0])
                .unwrap()
                .vs_profile
                .unwrap()
                .exploit];
            assert_eq!(t.answer(&station, exploit).unwrap().grade, Grade::Best);
        }
        assert!(checked > 40);
        assert!(
            pre.spot(SpotKind::VsOpen, 0)
                .unwrap()
                .against(Some("nobody"))
                .answer(
                    pre,
                    pre.actions(pre.spot(SpotKind::VsOpen, 0).unwrap().node)[0]
                )
                .is_err()
        );
    }

    fn profile_app() -> App {
        let (mut app, _) = app_at(1_000_000);
        app.set_library_dir(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop-profiles"),
        );
        app
    }

    /// #27 acceptance: through `App`, flop, turn and river decisions against
    /// a profile are graded on hero's best-response EVs against it (flop and
    /// turn from the file, river solved on demand), with the reference EVs
    /// alongside, and the exploitative action is Best.
    #[test]
    fn postflop_spots_against_a_profile_grade_on_best_responses() {
        let mut a = profile_app();
        let t = a.trainers().unwrap();
        let post = t.post().unwrap();
        let mut streets = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for (seed, street) in (0..24).zip([None, None, Some(Street::River)].into_iter().cycle()) {
            let req = SpotRequest {
                street,
                opponent: Some("calling-station"),
                ..SpotRequest::default()
            };
            let Ok(s) = post.generate(&req, seed) else {
                continue;
            };
            assert_eq!(s.opponent.as_deref(), Some("calling-station"));
            assert!(s.fallback.is_none());
            let w = s.walk();
            assert_eq!(w.vs_profile(), Some("calling-station"));
            let br = w.profile_ev_bb(s.hand).unwrap();
            let reference = w.ev_bb(s.hand).unwrap();
            streets.insert(w.street());
            let spot = DrillSpot::Postflop(Box::new(s));
            let acts = spot_actions(&spot, &t);
            for (i, &act) in acts.iter().enumerate() {
                let d = t.answer(&spot, act).unwrap();
                let vs = d.vs_profile.as_ref().expect("graded against the profile");
                assert_eq!(vs.profile, "calling-station");
                assert_eq!(d.hand_evs, br);
                assert_eq!(vs.reference_evs, reference);
                let best = br.iter().copied().fold(f32::MIN, f32::max);
                assert!((d.ev_loss_bb - f64::from(best - br[i]).max(0.0)).abs() < 1e-9);
                assert_eq!(vs.exploit, crate::preflop::argmax(&br));
                // Stored under the profile (#26 keys).
                let ctx = spot.context(&t);
                assert_eq!(ctx.template.opponent.as_deref(), Some("calling-station"));
                a.record_spot_decision(&d, &ctx, None).unwrap();
                if d.grade > Grade::Correct {
                    keys.insert(ctx.template.key());
                }
            }
            let exploit = t
                .answer(&spot, acts[0])
                .unwrap()
                .vs_profile
                .unwrap()
                .exploit;
            assert_eq!(t.answer(&spot, acts[exploit]).unwrap().grade, Grade::Best);
        }
        for st in [Street::Flop, Street::Turn, Street::River] {
            assert!(streets.contains(&st), "no {st:?} spot vs the profile");
        }
        // Mistakes became review items against the profile; each deals a
        // fresh spot of its template against the same opponent.
        assert!(!keys.is_empty());
        for k in &keys {
            let tpl = SpotTemplate::parse(k).unwrap();
            let s = t.spot_for_template(&tpl, 3).unwrap();
            assert_eq!(s.context(&t).template, tpl, "{k}");
            let g = t.answer(&s, s.actions(&t)[0]).unwrap();
            assert_eq!(g.vs_profile.unwrap().profile, "calling-station");
        }
        let leaks = a.leaks().unwrap();
        assert!(!leaks.is_empty());
        for l in &leaks {
            assert_eq!(l.bucket.opponent.as_deref(), Some("calling-station"));
            let s = t.spot_for_bucket(&l.bucket, 7).unwrap();
            assert_eq!(s.context(&t).bucket(), l.bucket);
        }
    }

    /// #27 acceptance: a reference-only (older format) library still loads,
    /// and asking for a profile deals a reference spot that says so.
    #[test]
    fn reference_only_library_falls_back_to_the_reference() {
        let mut a = app();
        let t = a.trainers().unwrap();
        let post = t.post().unwrap();
        let req = SpotRequest {
            opponent: Some("calling-station"),
            ..SpotRequest::default()
        };
        let s = post.generate(&req, 5).unwrap();
        assert_eq!(s.fallback.as_deref(), Some("calling-station"));
        assert!(s.opponent.is_none());
        assert!(s.walk().vs_profile().is_none());
        let spot = DrillSpot::Postflop(Box::new(s));
        let d = t.answer(&spot, spot_actions(&spot, &t)[0]).unwrap();
        assert!(d.vs_profile.is_none());
        let ctx = spot.context(&t);
        assert_eq!(ctx.template.opponent, None);
        // A review or a leak against the profile waits on this library
        // rather than dealing against the reference.
        let tpl = SpotTemplate {
            opponent: Some("calling-station".into()),
            ..ctx.template.clone()
        };
        assert!(matches!(
            t.spot_for_template(&tpl, 1),
            Err(TemplateError::Unavailable)
        ));
        let bucket = LeakBucket {
            opponent: Some("calling-station".into()),
            ..ctx.bucket()
        };
        assert!(t.spot_for_bucket(&bucket, 1).is_err());
    }

    /// #27 demo: against the calling station some hand bets thin for value
    /// where the reference checks, and the bet is graded Best.
    #[test]
    fn calling_station_rewards_a_thin_value_bet() {
        let mut a = profile_app();
        let t = a.trainers().unwrap();
        let post = t.post().unwrap();
        let found = (0..400).any(|seed| {
            let req = SpotRequest {
                opponent: Some("calling-station"),
                ..SpotRequest::default()
            };
            let Ok(s) = post.generate(&req, seed) else {
                return false;
            };
            let spot = DrillSpot::Postflop(Box::new(s));
            let acts = spot_actions(&spot, &t);
            let Some(check) = acts.iter().position(|&x| x == Action::Check) else {
                return false;
            };
            let d = t.answer(&spot, acts[check]).unwrap();
            let vs = d.vs_profile.unwrap();
            if vs.gto != check || !matches!(acts[vs.exploit], Action::Bet(_)) {
                return false;
            }
            let bet = t.answer(&spot, acts[vs.exploit]).unwrap();
            assert_eq!(bet.grade, Grade::Best);
            assert!(d.grade > Grade::Best);
            true
        });
        assert!(
            found,
            "no spot where GTO checks and a bet exploits the station"
        );
    }

    fn spot_actions(s: &DrillSpot, t: &Trainers) -> Vec<Action> {
        s.actions(t)
    }

    /// #26 acceptance: through `App`, the same mistake against a nit and
    /// against the reference makes two review items; reviewing the nit item
    /// deals against the nit; leaks split by profile and drill against it.
    #[test]
    fn mistakes_are_keyed_by_opponent_profile() {
        use std::sync::atomic::Ordering;

        use crate::i18n::Locale;
        use crate::preflop::SpotKind;
        use crate::review::RELEARN_MS;
        use crate::review_session::ReviewPrompt;

        let (mut a, clock) = app_at(1_000_000);
        a.set_library_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop"));
        let t = a.trainers().unwrap();
        let nit = Some("nit".to_string());
        // A spot where one action is below Correct against both.
        let (reference, vs_nit, act) = (0..500)
            .find_map(|seed| {
                let spot = t.preflop.spot(SpotKind::Rfi, seed).ok()?;
                let reference = DrillSpot::Preflop(Box::new(spot.clone()));
                let vs_nit = DrillSpot::Preflop(Box::new(spot.against(nit.as_deref())));
                let act = spot_actions(&reference, &t).into_iter().find(|&act| {
                    t.answer(&reference, act).unwrap().grade > Grade::Correct
                        && t.answer(&vs_nit, act).unwrap().grade > Grade::Correct
                })?;
                Some((reference, vs_nit, act))
            })
            .expect("a mistake against both");
        let (r_ctx, n_ctx) = (reference.context(&t), vs_nit.context(&t));
        assert_eq!(n_ctx.template.opponent, nit);
        assert_eq!(
            SpotTemplate {
                opponent: None,
                ..n_ctx.template.clone()
            },
            r_ctx.template,
            "same template but the opponent"
        );
        let r_dec = t.answer(&reference, act).unwrap();
        let n_dec = t.answer(&vs_nit, act).unwrap();
        // A decision filed under another opponent is refused.
        assert!(matches!(
            a.record_spot_decision(&n_dec, &r_ctx, None),
            Err(Error::Invalid(_))
        ));
        a.record_spot_decision(&r_dec, &r_ctx, None).unwrap();
        a.record_spot_decision(&n_dec, &n_ctx, None).unwrap();
        let r_key = r_ctx.template.key();
        let n_key = n_ctx.template.key();
        assert!(r_key.ends_with("|reference") && n_key.ends_with("|nit"));
        let item = |a: &App, k: &str| a.store.review_item("spot_template", k).unwrap();
        let r_item = item(&a, &r_key).expect("reference item");
        let n_item = item(&a, &n_key).expect("nit item");
        assert_ne!(r_item.id, n_item.id);
        assert_eq!(a.store.review_totals().unwrap().0, 2);
        // Answering the reference item against the nit is refused.
        assert!(matches!(
            a.record_spot_decision(&n_dec, &n_ctx, Some(&r_item.id)),
            Err(Error::Invalid(_))
        ));

        // Leaks split by profile; drilling the nit leak deals against it.
        let leaks = a.leaks().unwrap();
        let mut opponents: Vec<_> = leaks.iter().map(|l| l.bucket.opponent.clone()).collect();
        opponents.sort();
        assert_eq!(opponents, [None, nit.clone()]);
        for l in &leaks {
            assert_eq!(l.decisions, 1);
            for seed in 0..3 {
                let s = t.spot_for_bucket(&l.bucket, seed).unwrap();
                assert_eq!(s.context(&t).bucket(), l.bucket);
                let d = t.answer(&s, s.actions(&t)[0]).unwrap();
                assert_eq!(
                    d.vs_profile.map(|v| v.profile),
                    l.bucket.opponent,
                    "graded against the bucket's opponent"
                );
            }
        }

        // The review session deals each item against its own opponent.
        clock.fetch_add(RELEARN_MS, Ordering::SeqCst);
        let mut seen = Vec::new();
        while let Some(p) = a.next_review(Locale::PtBr).unwrap() {
            let ReviewPrompt::Spot {
                item_id,
                template,
                spot,
            } = p
            else {
                panic!("spot prompt expected");
            };
            let DrillSpot::Preflop(s) = &*spot else {
                panic!("preflop spot expected");
            };
            assert_eq!(s.opponent, template.opponent);
            assert_eq!(spot.context(&t).template, template);
            let want = if item_id == n_item.id { &n_key } else { &r_key };
            assert_eq!(&template.key(), want);
            let acts = spot.actions(&t);
            let best = acts[t.answer(&spot, acts[0]).unwrap().best()];
            let g = a.answer_spot_review(&item_id, &spot, best).unwrap();
            assert_eq!(g.vs_profile.map(|v| v.profile), template.opponent);
            seen.push(template.opponent);
            assert!(seen.len() <= 2);
        }
        seen.sort();
        assert_eq!(seen, [None, nit]);
        assert_eq!(a.due_count().unwrap(), 0, "both rescheduled");
        assert_eq!(item(&a, &n_key).unwrap().reps, 2);

        // An item against a profile this build no longer ships is retired.
        let gone = SpotTemplate {
            opponent: Some("nobody".into()),
            ..r_ctx.template.clone()
        };
        assert_eq!(
            t.spot_for_template(&gone, 1).err(),
            Some(TemplateError::Gone)
        );
    }

    /// #26: a database from before opponent keys (schema v2, `v1|…` template
    /// keys) opens with its items and leaks as reference ones.
    #[test]
    fn old_databases_keep_their_items_as_reference() {
        use crate::i18n::Locale;
        use crate::review_session::ReviewPrompt;

        let dir = tempfile::tempdir().unwrap();
        let config = App::open(dir.path())
            .unwrap()
            .trainers()
            .unwrap()
            .config_key();
        std::fs::remove_dir_all(dir.path()).unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        let old = format!("v1|{config}|preflop||-|UTG");
        {
            let db = rusqlite::Connection::open(dir.path().join(crate::services::DB_FILE)).unwrap();
            for sql in &taful_store::MIGRATIONS[..2] {
                db.execute_batch(sql).unwrap();
            }
            db.pragma_update(None, "user_version", 2).unwrap();
            db.execute(
                "INSERT INTO review_items (id, created_at, updated_at, kind, item_key, due, reps,
                     lapses)
                 VALUES ('01OLD', 1, 1, 'spot_template', ?1, 0, 1, 0)",
                [&old],
            )
            .unwrap();
            db.execute(
                "INSERT INTO decisions (id, created_at, updated_at, node_key, hero_combo,
                     chosen_action, ev_per_action, frequencies, pot, ev_loss, grade,
                     thresholds_version, template_key, spot, street, node_type)
                 VALUES ('01DEC', 1, 1, ?1 || '#0', 'AsAh', 'f', '[]', '[]', 1.5, 3.0,
                     'blunder', 1, ?1, 'preflop', 'preflop', 'rfi')",
                [&old],
            )
            .unwrap();
        }
        let mut a = App::open(dir.path()).unwrap();
        let new = format!("v2|{config}|preflop||-|UTG|reference");
        let item = a.store.review_item("spot_template", &new).unwrap().unwrap();
        assert_eq!(item.id, "01OLD");
        assert!(
            a.store
                .review_item("spot_template", &old)
                .unwrap()
                .is_none()
        );
        let leaks = a.leaks().unwrap();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].bucket.opponent, None);
        let Some(ReviewPrompt::Spot { item_id, spot, .. }) = a.next_review(Locale::PtBr).unwrap()
        else {
            panic!("the old item is reviewed as a reference spot");
        };
        assert_eq!(item_id, "01OLD");
        let DrillSpot::Preflop(s) = &*spot else {
            panic!("preflop spot expected");
        };
        assert_eq!(s.opponent, None);
        let t = a.trainers().unwrap();
        a.answer_spot_review(&item_id, &spot, spot.actions(&t)[0])
            .unwrap();
        assert_eq!(a.store.review_item_by_id("01OLD").unwrap().unwrap().reps, 2);
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
    fn dealt_flops_are_stored_and_survive_a_reload() {
        let mut a = app();
        let t = a.trainers().unwrap();
        let spot = DrillSpot::Postflop(Box::new(t.post().unwrap().spot(None, 3).unwrap()));
        let DrillSpot::Postflop(s) = &spot else {
            unreachable!()
        };
        let flop = Flop::new([s.board[0], s.board[1], s.board[2]])
            .unwrap()
            .canonical()
            .0;
        let acts = spot.actions(&t);
        let d = t.answer(&spot, acts[0]).unwrap();
        a.record_spot_decision(&d, &spot.context(&t), None).unwrap();
        assert_eq!(
            a.store.dealt_flops(&t.config_key()).unwrap(),
            vec![("btn-sb-3bp".into(), 0, flop.to_string())]
        );
        // A fresh load continues the cycle: the other flop comes next.
        a.set_library_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop"));
        let t = a.trainers().unwrap();
        let next = t.post().unwrap().spot(None, 3).unwrap();
        let other = Flop::new([next.board[0], next.board[1], next.board[2]])
            .unwrap()
            .canonical()
            .0;
        assert_ne!(other, flop);
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

    #[test]
    fn hands_over_before_hero_acts_are_stored() {
        let mut a = app();
        let t = a.trainers().unwrap();
        // The bots end some deals on their own (a walk in the big blind).
        let h = (0..2_000)
            .map(|seed| FullHand::new(&t.preflop, t.post(), None, seed).unwrap())
            .find(|h| h.end.is_some())
            .expect("a hand that ends before hero acts");
        assert!(h.decisions.is_empty());
        let id = a.record_hand(&h).unwrap();
        let r = a.hand_review(&id).unwrap();
        assert!(r.decisions.is_empty());
        assert_eq!(r.summary.hero_hand, Some(h.hero_hand()));
        assert!(a.recent_hands().unwrap().iter().any(|s| s.id == id));
    }

    #[test]
    fn a_failed_review_drops_its_spot_decision() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::open(dir.path()).unwrap();
        let t = a.trainers().unwrap();
        let tpl = SpotTemplate::parse(&format!("v2|{}|preflop||-|UTG|reference", t.config_key()))
            .unwrap();
        let spot = t.spot_for_template(&tpl, 1).unwrap();
        let d = t.answer(&spot, spot.actions(&t)[0]).unwrap();
        let ctx = spot.context(&t);
        let decisions = |a: &App| -> u32 {
            a.store
                .bucket_stats(1_000)
                .unwrap()
                .iter()
                .map(|b| b.decisions)
                .sum()
        };
        let item = a
            .store
            .ensure_review_item("spot_template", &ctx.template.key(), None, 0)
            .unwrap();
        // Make the review write fail after the decision is in.
        let db = rusqlite::Connection::open(dir.path().join(crate::services::DB_FILE)).unwrap();
        let trap = "CREATE TRIGGER trap BEFORE INSERT ON review_log
                    BEGIN SELECT RAISE(ABORT, 'trap'); END;";
        db.execute_batch(trap).unwrap();
        assert!(a.record_spot_decision(&d, &ctx, Some(&item.id)).is_err());
        assert_eq!(decisions(&a), 0, "rolled back with the review");

        db.execute_batch("DROP TRIGGER trap").unwrap();
        a.record_spot_decision(&d, &ctx, Some(&item.id)).unwrap();
        assert_eq!(decisions(&a), 1);
    }

    /// Hero's choice in `h`: the action that keeps the line on the
    /// fixture's configuration (BTN open, SB 3-bet, BTN call) when there is
    /// one, the most frequent reference action otherwise.
    fn steer(h: &FullHand, t: &Trainers) -> Action {
        let acts = h.hero_actions(&t.preflop);
        const FIXTURE: &str = "UTG:f,HJ:f,CO:f,BTN:r250,SB:r950,BB:f,BTN:c";
        if h.walk().is_none() {
            let line = h.state.history().line().to_string();
            let pos = h.state.position(h.hero);
            if let Some(&a) = acts.iter().find(|a| {
                let next = if line.is_empty() {
                    format!("{pos}:{a}")
                } else {
                    format!("{line},{pos}:{a}")
                };
                FIXTURE.starts_with(&next)
            }) {
                return a;
            }
        }
        let freqs = match h.walk() {
            Some(w) => w.strategy(h.hero_hand()),
            None => None,
        };
        match freqs {
            Some(f) => {
                acts[(0..acts.len())
                    .max_by(|&a, &b| f[a].total_cmp(&f[b]))
                    .unwrap()]
            }
            None => acts[0],
        }
    }

    /// #28 acceptance: through `App`, a seeded full hand against the nit
    /// replays identically, grades every hero decision against the nit,
    /// and the stored hand shows the profile on each bot seat.
    #[test]
    fn full_hands_against_a_profile_replay_and_grade_against_it() {
        let mut a = profile_app();
        let t = a.trainers().unwrap();
        let mut postflop = 0;
        let mut hands = 0;
        for seed in 0..4_000 {
            let hero = Some((seed % 2) as usize); // BTN or SB
            let play = || {
                let mut h =
                    FullHand::against(&t.preflop, t.post(), hero, seed, Some("nit")).unwrap();
                while h.end.is_none() {
                    let act = steer(&h, &t);
                    h.act(&t.preflop, t.post(), act).unwrap();
                }
                h
            };
            // Same seed and same flop cycle (#51): the same hand.
            let reset = || t.post().unwrap().start_cycles([]);
            reset();
            let h = play();
            if h.decisions.is_empty() {
                continue;
            }
            let reached = h
                .decisions
                .iter()
                .any(|d| d.context.street != Street::Preflop);
            if !reached && hands >= 3 {
                continue;
            }
            hands += 1;
            postflop += usize::from(reached);
            // Same seed, same hero answers: the same hand, decision for
            // decision.
            reset();
            let again = play();
            assert_eq!(again.state, h.state);
            assert_eq!(again.end, h.end);
            assert_eq!(again.decisions, h.decisions);
            for d in &h.decisions {
                let vs = d
                    .graded
                    .vs_profile
                    .as_ref()
                    .expect("graded against the nit");
                assert_eq!(vs.profile, "nit");
            }
            let id = a.record_hand(&h).unwrap();
            let r = a.hand_review(&id).unwrap();
            for (s, p) in r.seat_profiles.iter().enumerate() {
                assert_eq!(p.as_deref(), (s != h.hero).then_some("nit"), "seat {s}");
            }
            assert_eq!(r.decisions.len(), h.decisions.len());
            for (rd, hd) in r.decisions.iter().zip(&h.decisions) {
                assert_eq!(rd.vs_profile, hd.graded.vs_profile);
                assert_eq!(rd.hand_evs, hd.graded.hand_evs);
                assert_eq!(rd.grade, hd.graded.grade);
            }
            assert_eq!(r.states.last().unwrap().board(), h.state.board());
            if postflop >= 2 && hands >= 5 {
                break;
            }
        }
        assert!(postflop >= 2, "{postflop} hands reached the flop");
        // Mistakes were filed under the nit (#26 keys).
        let buckets = a.store.bucket_stats(LEAK_WINDOW).unwrap();
        assert!(!buckets.is_empty());
        assert!(buckets.iter().all(|b| b.opponent == "nit"), "{buckets:?}");
    }

    /// #29 acceptance: through `App`, seeded hands at the demo mixed table
    /// (two nits, a maniac, two TAGs) replay identically, preflop grades
    /// match the on-demand best response against the opponents still in
    /// the hand, and the stored hands, mistakes, leaks and review items
    /// carry the table.
    #[test]
    fn mixed_table_hands_replay_and_grade_against_the_table() {
        let mut a = profile_app();
        let t = a.trainers().unwrap();
        let bots: Vec<Option<String>> = ["nit", "nit", "maniac", "tag", "tag"]
            .iter()
            .map(|id| Some(id.to_string()))
            .collect();
        let mut against_table = Vec::new();
        let mut hands = 0;
        for seed in 0..400u64 {
            let hero = Some((seed % 6) as usize);
            // Hero answers by rote (not by the chart), so mistakes happen.
            let play = || {
                let mut h = FullHand::at_table(&t.preflop, t.post(), hero, seed, &bots).unwrap();
                let mut i = seed as usize;
                while h.end.is_none() {
                    let acts = h.hero_actions(&t.preflop);
                    let act = if h.walk().is_some() {
                        steer(&h, &t)
                    } else {
                        i += 1;
                        acts[i % acts.len()]
                    };
                    h.act(&t.preflop, t.post(), act).unwrap();
                }
                h
            };
            let h = play();
            if h.decisions.is_empty() {
                continue;
            }
            let again = play();
            assert_eq!(again.state, h.state);
            assert_eq!(again.end, h.end);
            assert_eq!(again.decisions, h.decisions);
            for d in h
                .decisions
                .iter()
                .filter(|d| d.context.street == Street::Preflop)
            {
                let key = t
                    .preflop
                    .opponent_at(d.graded.node, &h.seat_profiles, h.hero);
                assert_eq!(d.context.template.opponent, key);
                if let Some(k) = key.filter(|k| crate::mixed::is_table_key(k)) {
                    let evs = t
                        .preflop
                        .table_evs(&k, d.graded.node, d.graded.hand)
                        .unwrap();
                    crate::hand::tests::assert_close(&d.graded.hand_evs, &evs);
                    assert_eq!(d.graded.vs_profile.as_ref().unwrap().profile, k);
                    against_table.push(d.clone());
                }
            }
            let id = a.record_hand(&h).unwrap();
            let r = a.hand_review(&id).unwrap();
            assert_eq!(r.seat_profiles, h.seat_profiles);
            for (rd, hd) in r.decisions.iter().zip(&h.decisions) {
                assert_eq!(rd.vs_profile, hd.graded.vs_profile);
                assert_eq!(rd.hand_evs, hd.graded.hand_evs);
            }
            hands += 1;
            let mistakes = against_table
                .iter()
                .filter(|d| d.graded.grade > Grade::Correct)
                .count();
            if hands >= 6 && mistakes >= 2 {
                break;
            }
        }
        let mistake = against_table
            .iter()
            .find(|d| d.graded.grade > Grade::Correct)
            .expect("a mistake against a table");
        let key = mistake.context.template.opponent.clone().unwrap();
        assert!(t.knows_opponent(Some(&key)));
        assert!(!t.knows_opponent(Some("table:BTN=nit,BB=nobody")));
        // Filed under the table's key (#26 keys) ...
        let buckets = a.store.bucket_stats(LEAK_WINDOW).unwrap();
        let bucket = buckets
            .iter()
            .find(|b| b.opponent == key)
            .expect("a bucket for the table");
        assert!(bucket.mistakes > 0);
        // ... its review item deals the same line against the same table
        // and grades on the same best response ...
        let template = &mistake.context.template;
        assert!(
            a.store
                .review_item(ItemKind::SpotTemplate.id(), &template.key())
                .unwrap()
                .is_some()
        );
        let DrillSpot::Preflop(s) = t.spot_for_template(template, 9).unwrap() else {
            panic!("a preflop spot");
        };
        assert_eq!(s.opponent.as_deref(), Some(key.as_str()));
        let g = t
            .answer(&DrillSpot::Preflop(s.clone()), t.preflop.actions(s.node)[0])
            .unwrap();
        assert_eq!(g.vs_profile.unwrap().profile, key);
        assert_eq!(
            g.hand_evs,
            t.preflop.table_evs(&key, s.node, s.hand).unwrap()
        );
        // ... and its leak drills spots where exactly that table is left.
        let leak = LeakBucket {
            street: Street::Preflop,
            node_type: NodeType::parse(&bucket.node_type).unwrap(),
            spot: bucket.spot.clone(),
            texture: None,
            opponent: Some(key.clone()),
        };
        let DrillSpot::Preflop(s) = t.spot_for_bucket(&leak, 3).unwrap() else {
            panic!("a preflop spot");
        };
        assert_eq!(s.opponent.as_deref(), Some(key.as_str()));
        let mut seats = vec![None; 6];
        for (pos, id) in crate::mixed::parse_table_key(&key).unwrap() {
            seats[taful_core::position::Position::by_button_offset(6)
                .iter()
                .position(|&p| p == pos)
                .unwrap()] = id;
        }
        assert_eq!(t.preflop.opponent_at(s.node, &seats, s.hero), Some(key));
    }

    #[test]
    fn reference_hands_store_every_seat_as_reference() {
        let mut a = app();
        let t = a.trainers().unwrap();
        let h = FullHand::new(&t.preflop, t.post(), Some(2), 7).unwrap();
        let id = a.record_hand(&h).unwrap();
        let r = a.hand_review(&id).unwrap();
        assert_eq!(r.seat_profiles, vec![None; 6]);
        let row = a.store.hand(&id).unwrap().unwrap();
        assert_eq!(
            row.bot_ids,
            json!([
                "reference",
                "reference",
                "hero",
                "reference",
                "reference",
                "reference"
            ])
        );
        // Hands stored before #28 hold one id for every bot.
        assert_eq!(seat_profiles(&json!(["reference"]), 6), vec![None; 6]);
        assert_eq!(
            seat_profiles(&json!(["hero", "nit"]), 2),
            vec![None, Some("nit".to_string())]
        );
    }
}

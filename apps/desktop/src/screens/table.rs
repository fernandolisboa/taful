//! Table screen (spec §10, §15.1 #4): preflop and postflop spot mode, full
//! hands from preflop to the river against the reference bots (M5), and
//! drills from a leak bucket. Every hero decision is graded; the shell
//! stores decisions and hands through [`TableEvent`]s.
//!
//! Panels sit on the active layout's grid (spec §15.3, M6) and the felt
//! scales to its panel. Work that can solve a river (postflop deals, drills,
//! every full-hand step) runs on the background executor; the screen shows
//! a busy line and ignores clicks until it lands (M6 performance pass).
//!
//! The layout edit mode (#22) lives in `table_edit.rs`.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, Context, Div, EventEmitter, SharedString, Stateful, Task, Window, div, prelude::*,
    px,
};
use taful_app::layout::{Layout, PanelId, Rect};
use taful_app::model::{
    DrillSpot, FullHand, HandEnd, LeakBucket, LeftReason, SpotContext, Street, Trainers,
};
use taful_app::postflop::PostflopError;
use taful_app::preflop::{Action, GameState, GradedDecision, SpotKind};
use taful_app::template::{street_id, texture_label_keys};

#[path = "table_edit.rs"]
mod edit;

use crate::Ui;
use crate::felt::{self, Explain};
use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Spot,
    Postflop,
    Hand,
    Drill,
}

/// What the table tells the shell.
pub enum TableEvent {
    /// A spot-mode or drill answer, to store and feed the review queue.
    Answered(Box<GradedDecision>, Box<SpotContext>),
    /// A full hand ended, to store.
    HandOver(Box<FullHand>),
    /// Open the hand review on the last stored hand.
    ReviewHand,
    /// Save the edited layout as a user copy; the shell answers through
    /// [`TableScreen::layout_saved`].
    SaveLayout(Box<Layout>),
}

impl EventEmitter<TableEvent> for TableScreen {}

pub struct TableScreen {
    ui: Rc<Ui>,
    palette: Palette,
    trainers: Result<Arc<Trainers>, String>,
    mode: Mode,
    kind: SpotKind,
    /// Spot mode's opponent: `None` = the reference bot, else a profile id.
    opponent: Option<String>,
    /// Seeds for spots and hands; bumped for every new deal.
    seed: u64,
    /// Postflop configuration filter (`None` = all).
    postflop_spot: Option<String>,
    /// The leak being drilled.
    drill: Option<LeakBucket>,
    question: Option<DrillSpot>,
    answer: Option<GradedDecision>,
    hand: Option<FullHand>,
    /// The current hand was handed to the shell.
    hand_sent: bool,
    /// Panel placement (settings).
    layout: Layout,
    /// Layout edit mode, while on.
    editing: Option<edit::Editing>,
    /// Grid cell size of the last frame, in pixels (edit-mode drags).
    cell: f32,
    /// Background work in flight, if any.
    busy: Option<Busy>,
    /// Bumped by every deal and action; a background result for an older
    /// generation is dropped.
    generation: u64,
    _job: Option<Task<()>>,
}

/// What the background is doing (the busy line).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    Dealing,
    Acting,
}

/// A background result.
enum Done {
    Question(Option<DrillSpot>),
    Hand(Option<FullHand>),
    Acted(Result<FullHand, String>),
}

/// Smallest grid cell in pixels: below it the window scrolls instead.
const MIN_CELL: f32 = 14.;
/// Room the shell takes above the screen (title and gap), in pixels.
const TITLE_ROOM: f32 = 56.;

impl TableScreen {
    /// Switches the strings (locale change); the deal in progress is kept.
    pub fn set_ui(&mut self, ui: Rc<Ui>) {
        self.palette = Palette::new(&ui.theme.colors);
        self.ui = ui;
    }

    /// New reference strategies (library folder changed in settings).
    pub fn set_trainers(
        &mut self,
        trainers: Result<Arc<Trainers>, String>,
        cx: &mut Context<Self>,
    ) {
        self.trainers = trainers;
        self.deal(cx);
        cx.notify();
    }

    /// Switches the panel placement (settings); ends an edit in progress.
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.editing = None;
    }

    pub fn new(
        ui: Rc<Ui>,
        trainers: Result<Arc<Trainers>, String>,
        layout: Layout,
        cx: &mut Context<Self>,
    ) -> Self {
        let palette = Palette::new(&ui.theme.colors);
        // A fresh sequence every launch; each deal is still reproducible
        // from its own seed.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut s = Self {
            ui,
            palette,
            trainers,
            mode: Mode::Spot,
            kind: SpotKind::Rfi,
            opponent: None,
            seed,
            postflop_spot: None,
            drill: None,
            question: None,
            answer: None,
            hand: None,
            hand_sent: false,
            layout,
            editing: None,
            cell: MIN_CELL,
            busy: None,
            generation: 0,
            _job: None,
        };
        // Dev override (and screenshots): TAFUL_TABLE_MODE=postflop|hand.
        match std::env::var("TAFUL_TABLE_MODE").as_deref() {
            Ok("postflop") => s.mode = Mode::Postflop,
            Ok("hand") => s.mode = Mode::Hand,
            _ => {}
        }
        // Dev overrides (screenshots): TAFUL_TABLE_KIND=<spot kind id>,
        // TAFUL_TABLE_OPPONENT=<profile id>.
        if let Ok(k) = std::env::var("TAFUL_TABLE_KIND")
            && let Some(kind) = SpotKind::ALL.into_iter().find(|x| x.id() == k)
        {
            s.kind = kind;
        }
        s.opponent = std::env::var("TAFUL_TABLE_OPPONENT").ok().filter(|id| {
            s.trainers
                .as_ref()
                .is_ok_and(|t| t.preflop.profile(id).is_some())
        });
        if let Ok(seed) = std::env::var("TAFUL_TABLE_SEED").map(|v| v.parse::<u64>()) {
            s.seed = seed.unwrap_or(s.seed);
        }
        s.deal(cx);
        // Dev override (screenshots): TAFUL_LAYOUT_EDIT=1 opens edit mode.
        if std::env::var_os("TAFUL_LAYOUT_EDIT").is_some() {
            s.start_edit(cx);
        }
        s
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(1);
        self.seed
    }

    /// New spot or hand for the current mode. Postflop deals, drills and
    /// hands run in the background (they can solve rivers).
    fn deal(&mut self, cx: &mut Context<Self>) {
        let Ok(t) = self.trainers.clone() else { return };
        let seed = self.next_seed();
        self.answer = None;
        self.generation += 1;
        match self.mode {
            Mode::Spot => {
                self.busy = None;
                self._job = None;
                let opponent = self.opponent.as_deref();
                self.question = t
                    .preflop
                    .spot(self.kind, seed)
                    .ok()
                    .map(|s| DrillSpot::Preflop(Box::new(s.against(opponent))));
            }
            Mode::Postflop => {
                self.question = None;
                let spot = self.postflop_spot.clone();
                self.run(Busy::Dealing, cx, move || {
                    Done::Question(t.post().and_then(|pf| {
                        pf.spot(spot.as_deref(), seed)
                            .ok()
                            .map(|s| DrillSpot::Postflop(Box::new(s)))
                    }))
                });
            }
            Mode::Drill => {
                self.question = None;
                let bucket = self.drill.clone();
                self.run(Busy::Dealing, cx, move || {
                    Done::Question(bucket.and_then(|b| t.spot_for_bucket(&b, seed).ok()))
                });
            }
            Mode::Hand => {
                self.hand = None;
                self.hand_sent = false;
                // A new hand starts by the button so every seat comes up.
                self.run(Busy::Dealing, cx, move || {
                    Done::Hand(FullHand::new(&t.preflop, t.post(), None, seed).ok())
                });
            }
        }
    }

    /// Runs `work` on the background executor; its result lands through
    /// [`Self::finish`] unless a newer deal or action came first.
    fn run(
        &mut self,
        busy: Busy,
        cx: &mut Context<Self>,
        work: impl FnOnce() -> Done + Send + 'static,
    ) {
        let generation = self.generation;
        self.busy = Some(busy);
        let job = cx.background_executor().spawn(async move { work() });
        self._job = Some(cx.spawn(async move |this, cx| {
            let done = job.await;
            this.update(cx, |s, cx| {
                if s.generation == generation {
                    s.busy = None;
                    s.finish(done, cx);
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn finish(&mut self, done: Done, cx: &mut Context<Self>) {
        match done {
            Done::Question(q) => self.question = q,
            // The bots can end a hand before hero ever acts (a walk in the
            // big blind), so a fresh deal is checked as well.
            Done::Hand(Some(h)) | Done::Acted(Ok(h)) => {
                self.send_if_over(&h, cx);
                self.hand = Some(h);
            }
            Done::Hand(None) => self.hand = None,
            Done::Acted(Err(e)) => eprintln!("taful: hand: {e}"),
        }
    }

    /// Emits [`TableEvent::HandOver`] once per hand, when it has ended.
    fn send_if_over(&mut self, h: &FullHand, cx: &mut Context<Self>) {
        if h.end.is_some() && !self.hand_sent {
            self.hand_sent = true;
            cx.emit(TableEvent::HandOver(Box::new(h.clone())));
        }
    }

    /// Drill a leak (the home screen's "drill this").
    pub fn start_drill(&mut self, bucket: LeakBucket, cx: &mut Context<Self>) {
        self.drill = Some(bucket);
        self.mode = Mode::Drill;
        self.deal(cx);
        cx.notify();
    }

    fn set_postflop_spot(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.postflop_spot = id;
        self.mode = Mode::Postflop;
        self.deal(cx);
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.deal(cx);
            cx.notify();
        }
    }

    fn set_opponent(&mut self, opponent: Option<String>, cx: &mut Context<Self>) {
        self.opponent = opponent;
        self.mode = Mode::Spot;
        self.deal(cx);
        cx.notify();
    }

    fn set_kind(&mut self, kind: SpotKind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.mode = Mode::Spot;
        self.deal(cx);
        cx.notify();
    }

    fn act(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let Ok(t) = self.trainers.clone() else { return };
        match self.mode {
            Mode::Hand => {
                // The bots answer and the river may be solved: background.
                if let Some(h) = &self.hand {
                    let mut h = h.clone();
                    self.generation += 1;
                    self.run(Busy::Acting, cx, move || {
                        Done::Acted(
                            h.act(&t.preflop, t.post(), action)
                                .map(|_| ())
                                .map(|()| h)
                                .map_err(|e| e.to_string()),
                        )
                    });
                }
            }
            _ => {
                // Grading reads the spot's strategy: no solve here.
                if self.answer.is_none()
                    && let Some(q) = &self.question
                {
                    match t.answer(q, action) {
                        Ok(d) => {
                            cx.emit(TableEvent::Answered(
                                Box::new(d.clone()),
                                Box::new(q.context(&t)),
                            ));
                            self.answer = Some(d);
                        }
                        Err(e) => eprintln!("taful: answer: {e}"),
                    }
                }
            }
        }
        cx.notify();
    }

    // ---- pieces ---------------------------------------------------------------

    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        primary: bool,
        cx: &mut Context<Self>,
        on: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        crate::widgets::button(id, label, primary, &self.ui, &self.palette)
            .on_click(cx.listener(move |this, _, _, cx| on(this, cx)))
    }

    fn chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
        on: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        crate::widgets::chip(id, label, selected, &self.ui, &self.palette)
            .on_click(cx.listener(move |this, _, _, cx| on(this, cx)))
    }

    /// Hero's action buttons (exactly the tree's sizes, D4).
    fn actions_bar(&self, state: &GameState, actions: &[Action], cx: &mut Context<Self>) -> Div {
        let t = &self.ui.theme;
        let buttons: Vec<_> = actions
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                let label = felt::action_label(&self.ui, state, a);
                self.button(
                    ("act", i),
                    label,
                    matches!(a, Action::Raise(_) | Action::AllIn),
                    cx,
                    move |this, cx| this.act(a, cx),
                )
            })
            .collect();
        div()
            .flex()
            .flex_wrap()
            .gap(px(t.spacing.sm))
            .children(buttons)
    }

    fn message(&self, text: String) -> Div {
        div()
            .text_size(px(self.ui.theme.text_size.md))
            .text_color(self.palette.text_muted)
            .child(text)
    }

    /// The prompt before hero answers; against a profile, who the bot is.
    fn prompt(&self, q: &DrillSpot) -> Div {
        let (th, i18n) = (&self.ui.theme, &self.ui.i18n);
        let opponent = match q {
            DrillSpot::Preflop(s) => s.opponent.as_deref(),
            DrillSpot::Postflop(_) => None,
        };
        let mut d = div()
            .flex()
            .flex_col()
            .gap(px(th.spacing.md))
            .child(self.message(i18n.tr("table-spot-prompt")));
        if let Some(id) = opponent {
            d = d.child(self.message(i18n.tr_args(
                "table-vs-profile-prompt",
                &[
                    ("profile", i18n.tr(&format!("bot-profile-{id}")).into()),
                    ("about", i18n.tr(&format!("bot-profile-{id}-about")).into()),
                ],
            )));
        }
        d
    }

    /// The busy line while background work runs.
    fn busy_line(&self) -> Option<Div> {
        let key = match self.busy? {
            Busy::Dealing => "table-busy-dealing",
            Busy::Acting => "table-busy-acting",
        };
        Some(self.message(self.ui.i18n.tr(key)))
    }

    /// Why postflop spots cannot be dealt.
    fn postflop_message(&self, t: &Trainers) -> Option<String> {
        let i18n = &self.ui.i18n;
        match &t.postflop {
            Ok(_) => None,
            Err(PostflopError::Library(_) | PostflopError::Empty)
                if !t.library_dir.join("manifest.json").exists() =>
            {
                Some(i18n.tr_args(
                    "table-postflop-missing",
                    &[("dir", t.library_dir.display().to_string().into())],
                ))
            }
            Err(e) => {
                Some(i18n.tr_args("table-postflop-error", &[("error", e.to_string().into())]))
            }
        }
    }

    /// A single-decision question (spot, postflop, drill).
    fn question_parts(&self, t: &Trainers, scale: f32, cx: &mut Context<Self>) -> Parts {
        let (p, i18n) = (&self.palette, &self.ui.i18n);
        if self.mode == Mode::Postflop
            && let Some(msg) = self.postflop_message(t)
        {
            return Parts::message(self.message(msg));
        }
        let Some(q) = &self.question else {
            return match self.busy_line() {
                Some(b) => Parts::message(b),
                None => Parts::message(self.message(i18n.tr("table-no-spot"))),
            };
        };
        let (panel, actions) = match &self.answer {
            Some(d) => (
                div().child(felt::explanation(&self.ui, p, &Explain::of(d))),
                div().child(self.button(
                    "next",
                    i18n.tr("table-next-spot"),
                    true,
                    cx,
                    |this, cx| {
                        this.deal(cx);
                        cx.notify();
                    },
                )),
            ),
            None => (
                self.prompt(q),
                self.actions_bar(q.state(), &q.actions(t), cx),
            ),
        };
        Parts {
            table: felt::table_scaled(&self.ui, p, q.state(), q.hero(), false, scale),
            actions,
            panel,
        }
    }

    fn end_text(&self, h: &FullHand) -> Option<String> {
        let i18n = &self.ui.i18n;
        h.end.as_ref().map(|e| match e {
            HandEnd::Settled(_) | HandEnd::HeroFolded => i18n.tr_args(
                "table-hand-over-settled",
                &[(
                    "amount",
                    felt::num(&self.ui, h.hero_net_bb().unwrap_or(0.), 1).into(),
                )],
            ),
            HandEnd::LeftLibrary { history, reason } => {
                let key = match reason {
                    LeftReason::NoLibrary => "table-hand-over-flop",
                    LeftReason::Unsolved => "table-hand-over-unsolved",
                    LeftReason::OffTree => "table-hand-over-left",
                    LeftReason::NotInRange => "table-hand-over-range",
                };
                i18n.tr_args(key, &[("line", history.to_string().into())])
            }
        })
    }

    fn hand_parts(&self, t: &Trainers, scale: f32, cx: &mut Context<Self>) -> Parts {
        let (p, th, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let Some(h) = &self.hand else {
            return match self.busy_line() {
                Some(b) => Parts::message(b),
                None => Parts::message(self.message(i18n.tr("table-no-spot"))),
            };
        };
        let over = h.end.is_some();
        let decisions = h.decisions.iter().map(|d| {
            div()
                .flex()
                .justify_between()
                .gap(px(th.spacing.sm))
                .text_size(px(th.text_size.sm))
                .child(format!(
                    "{} · {} · {}",
                    i18n.tr(&format!("street-{}", street_id(d.context.street))),
                    d.graded.position.label(),
                    felt::action_short(&self.ui, d.graded.actions[d.graded.chosen])
                ))
                .child(felt::grade_label(&self.ui, p, d.graded.grade))
        });
        let mut panel = div().flex().flex_col().gap(px(th.spacing.lg));
        if h.decisions.is_empty() && !over {
            panel = panel.child(self.message(i18n.tr("table-spot-prompt")));
        }
        if let Some(d) = h.decisions.last() {
            panel = panel.child(felt::explanation(&self.ui, p, &Explain::of(&d.graded)));
        }
        if !h.decisions.is_empty() {
            panel = panel
                .child(
                    div()
                        .text_size(px(th.text_size.sm))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(i18n.tr("table-decisions")),
                )
                .children(decisions);
        }
        if let Some(walk) = h.walk()
            && walk.on_solved_river()
            && !over
        {
            panel = panel.child(
                div()
                    .text_size(px(th.text_size.sm))
                    .text_color(p.text_muted)
                    .child(i18n.tr("table-river-solved")),
            );
        }
        let actions = if let Some(text) = self.end_text(h) {
            panel = panel
                .child(div().text_size(px(th.text_size.md)).child(text))
                .child(
                    div()
                        .text_size(px(th.text_size.sm))
                        .text_color(p.text_muted)
                        .child(i18n.tr_args(
                            "table-hand-ev-loss",
                            &[(
                                "amount",
                                felt::num(&self.ui, h.total_ev_loss_bb(), 2).into(),
                            )],
                        )),
                );
            div()
                .flex()
                .flex_wrap()
                .gap(px(th.spacing.sm))
                .child(self.button(
                    "new_hand",
                    i18n.tr("table-new-hand"),
                    true,
                    cx,
                    |this, cx| {
                        this.deal(cx);
                        cx.notify();
                    },
                ))
                .when(!h.decisions.is_empty(), |d| {
                    d.child(self.button(
                        "review_hand",
                        i18n.tr("table-review-hand"),
                        false,
                        cx,
                        |_, cx| cx.emit(TableEvent::ReviewHand),
                    ))
                })
        } else if let Some(b) = self.busy_line() {
            b
        } else {
            self.actions_bar(&h.state, &h.hero_actions(&t.preflop), cx)
        };
        Parts {
            table: felt::table_scaled(&self.ui, p, &h.state, h.hero, over, scale),
            actions,
            panel,
        }
    }

    /// Mode and spot pickers.
    fn controls(&self, trainers: &Trainers, cx: &mut Context<Self>) -> Div {
        let (p, th) = (self.palette, self.ui.theme.clone());
        let ui = self.ui.clone();
        let i18n = &ui.i18n;
        let mut modes = div()
            .flex()
            .flex_wrap()
            .gap(px(th.spacing.sm))
            .child(self.chip(
                "mode_spot",
                i18n.tr("table-mode-spot"),
                self.mode == Mode::Spot,
                cx,
                |this, cx| this.set_mode(Mode::Spot, cx),
            ))
            .child(self.chip(
                "mode_postflop",
                i18n.tr("table-mode-postflop"),
                self.mode == Mode::Postflop,
                cx,
                |this, cx| this.set_mode(Mode::Postflop, cx),
            ))
            .child(self.chip(
                "mode_hand",
                i18n.tr("table-mode-hand"),
                self.mode == Mode::Hand,
                cx,
                |this, cx| this.set_mode(Mode::Hand, cx),
            ));
        if self.drill.is_some() {
            modes = modes.child(self.chip(
                "mode_drill",
                i18n.tr("table-mode-drill"),
                self.mode == Mode::Drill,
                cx,
                |this, cx| this.set_mode(Mode::Drill, cx),
            ));
        }
        if !self.is_editing() {
            modes = modes.child(self.chip(
                "layout_edit",
                i18n.tr("layout-edit-start"),
                false,
                cx,
                |this, cx| this.start_edit(cx),
            ));
        }
        // Opponent picker (spec §9, D8): reference first, then each profile.
        let mut opponents = vec![self.chip(
            "opp_reference",
            i18n.tr("bot-profile-reference"),
            self.opponent.is_none(),
            cx,
            |this, cx| this.set_opponent(None, cx),
        )];
        for (i, p) in trainers.preflop.profiles().iter().enumerate() {
            let id = p.bot.profile().id.clone();
            let selected = self.opponent.as_deref() == Some(id.as_str());
            let label = i18n.tr(&format!("bot-profile-{id}"));
            opponents.push(self.chip(("opp", i), label, selected, cx, move |this, cx| {
                this.set_opponent(Some(id.clone()), cx)
            }));
        }
        if self.mode == Mode::Spot {
            modes = modes.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(th.spacing.sm))
                    .ml(px(th.spacing.lg))
                    .child(
                        div()
                            .text_size(px(th.text_size.sm))
                            .text_color(p.text_muted)
                            .child(i18n.tr("table-opponent")),
                    )
                    .children(opponents),
            );
        }
        let kinds: Vec<_> = trainers
            .preflop
            .available_kinds()
            .into_iter()
            .enumerate()
            .map(|(i, k)| {
                let label = i18n.tr(&format!("table-spot-kind-{}", k.id()));
                let selected = self.mode == Mode::Spot && self.kind == k;
                self.chip(("kind", i), label, selected, cx, move |this, cx| {
                    this.set_kind(k, cx)
                })
            })
            .collect();
        let mut pf_chips = Vec::new();
        if let Some(pf) = trainers.post() {
            pf_chips.push(self.chip(
                "pf_any",
                i18n.tr("table-postflop-any"),
                self.postflop_spot.is_none(),
                cx,
                |this, cx| this.set_postflop_spot(None, cx),
            ));
            for (i, s) in pf.spots().iter().enumerate() {
                let id = s.id.clone();
                let selected = self.postflop_spot.as_deref() == Some(id.as_str());
                pf_chips.push(self.chip(
                    ("pf", i),
                    felt::spot_label(&ui, &id),
                    selected,
                    cx,
                    move |this, cx| this.set_postflop_spot(Some(id.clone()), cx),
                ));
            }
        }
        let title: SharedString = match self.mode {
            Mode::Postflop => i18n.tr("table-subtitle-postflop"),
            Mode::Hand => i18n.tr("table-subtitle-hand"),
            Mode::Drill => match &self.drill {
                Some(b) => i18n.tr_args(
                    "table-subtitle-drill",
                    &[("bucket", Self::bucket_label(&ui, b).into())],
                ),
                None => String::new(),
            },
            Mode::Spot => i18n.tr("table-subtitle"),
        }
        .into();
        div()
            .flex()
            .flex_col()
            .gap(px(th.spacing.sm))
            .child(
                div()
                    .text_size(px(th.text_size.sm))
                    .text_color(p.text_muted)
                    .child(title),
            )
            .child(modes)
            .when(self.mode == Mode::Spot, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(th.spacing.sm))
                        .children(kinds),
                )
            })
            .when(self.mode == Mode::Postflop, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(th.spacing.sm))
                        .children(pf_chips),
                )
            })
    }

    /// A panel's box on the grid.
    fn place(r: Rect) -> Div {
        div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
    }

    /// The drill's bucket as one line, ending with the bot profile it was
    /// against (nothing for the reference bot).
    pub fn bucket_label(ui: &Ui, b: &LeakBucket) -> String {
        let i18n = &ui.i18n;
        let mut parts = vec![felt::spot_label(ui, &b.spot)];
        if b.street != Street::Preflop {
            parts.push(i18n.tr(&format!("street-{}", street_id(b.street))));
        }
        parts.push(i18n.tr(&format!("node-type-{}", b.node_type.id())));
        if let Some(keys) = b.texture.as_deref().and_then(texture_label_keys) {
            parts.push(keys.map(|k| i18n.tr(&k)).join(", "));
        }
        let opponent = taful_app::template::opponent_id(b.opponent.as_deref());
        parts.push(i18n.tr_args(
            "leak-vs-profile",
            &[(
                "profile",
                i18n.tr(&format!("bot-profile-{opponent}")).into(),
            )],
        ));
        parts.join(" · ")
    }
}

impl Render for TableScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, th) = (self.palette, self.ui.theme.clone());
        let ui = self.ui.clone();
        let trainers = match &self.trainers {
            Ok(t) => t.clone(),
            Err(e) => {
                return div()
                    .text_color(p.text_muted)
                    .child(
                        ui.i18n
                            .tr_args("table-chart-error", &[("error", e.clone().into())]),
                    )
                    .into_any_element();
            }
        };
        // Cells from the space the shell leaves us (spec §15.3); the edit
        // bar takes a strip above the grid while editing.
        let vp = window.viewport_size();
        let editing = self.is_editing();
        let bar = if editing { edit::EDIT_BAR } else { 0. };
        let w = f32::from(vp.width) - crate::shell::NAV_WIDTH - 2. * th.spacing.xxl;
        let h = f32::from(vp.height) - 2. * th.spacing.xxl - TITLE_ROOM - bar;
        let layout = self.shown_layout();
        let cell = layout.cell_size(w, h, MIN_CELL);
        self.cell = cell;
        let scale = self
            .panel_rect(&layout, PanelId::Table, cell)
            .map_or(1., |r| felt::fit_scale(r.w, r.h));
        let parts = match self.mode {
            Mode::Hand => self.hand_parts(&trainers, scale, cx),
            _ => self.question_parts(&trainers, scale, cx),
        };
        let controls = self.controls(&trainers, cx);
        let mut grid = div()
            .relative()
            .flex_none()
            .w(px(cell * f32::from(layout.grid.cols)))
            .h(px(cell * f32::from(layout.grid.rows)));
        grid = self.edit_grid(grid, &layout, cell, cx);
        let mut parts = Some(parts);
        let mut controls = Some(controls);
        let mut on_top = None;
        for id in PanelId::ALL {
            let Some(r) = self.panel_rect(&layout, id, cell) else {
                continue;
            };
            let el: Div = match id {
                PanelId::Controls => match controls.take() {
                    Some(c) => Self::place(r).child(c),
                    None => continue,
                },
                PanelId::Table => {
                    let table = parts
                        .as_mut()
                        .map(|p| std::mem::replace(&mut p.table, div()));
                    Self::place(r)
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(table)
                }
                PanelId::Actions => {
                    let actions = parts
                        .as_mut()
                        .map(|p| std::mem::replace(&mut p.actions, div()));
                    Self::place(r)
                        .flex()
                        .items_start()
                        .justify_center()
                        .pt(px(th.spacing.sm))
                        .children(actions)
                }
                PanelId::Explanation => {
                    let panel = parts.take().map(|p| p.panel);
                    Self::place(r).child(
                        div()
                            .id("explanation")
                            .size_full()
                            .overflow_y_scroll()
                            .p(px(th.spacing.lg))
                            .rounded(px(th.radii.lg))
                            .bg(p.surface)
                            .border_1()
                            .border_color(p.border)
                            .children(panel),
                    )
                }
            };
            let el: AnyElement = self.edit_chrome(id, el, cx);
            if self.dragged_panel() == Some(id) {
                on_top = Some(el);
            } else {
                grid = grid.child(el);
            }
        }
        let grid = grid.children(on_top).children(self.landing_outline(cell));
        if editing {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .child(self.edit_bar(cell * f32::from(layout.grid.cols), cx))
                .child(grid)
                .into_any_element()
        } else {
            grid.into_any_element()
        }
    }
}

/// What the table, actions and explanation panels show.
struct Parts {
    table: Div,
    actions: Div,
    panel: Div,
}

impl Parts {
    /// Only a message (no spot): shown on the felt's panel.
    fn message(text: Div) -> Parts {
        Parts {
            table: text,
            actions: div(),
            panel: div(),
        }
    }
}

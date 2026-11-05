//! Table screen (spec §10, §15.1 #4): preflop and postflop spot mode, full
//! hands from preflop to the river against the reference bots (M5), and
//! drills from a leak bucket. Every hero decision is graded; the shell
//! stores decisions and hands through [`TableEvent`]s.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{Context, Div, EventEmitter, SharedString, Stateful, Window, div, prelude::*, px};
use taful_app::model::{
    DrillSpot, FullHand, HandEnd, LeakBucket, LeftReason, SpotContext, Street, Trainers,
};
use taful_app::postflop::PostflopError;
use taful_app::preflop::{Action, GameState, GradedDecision, SpotKind};
use taful_app::template::{street_id, texture_label_keys};

use crate::Ui;
use crate::felt::{self, Explain, FELT_W};
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
}

impl EventEmitter<TableEvent> for TableScreen {}

pub struct TableScreen {
    ui: Rc<Ui>,
    palette: Palette,
    trainers: Result<Arc<Trainers>, String>,
    mode: Mode,
    kind: SpotKind,
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
}

impl TableScreen {
    /// Switches the strings (locale change); the deal in progress is kept.
    pub fn set_ui(&mut self, ui: Rc<Ui>) {
        self.palette = Palette::new(&ui.theme.colors);
        self.ui = ui;
    }

    pub fn new(ui: Rc<Ui>, trainers: Result<Arc<Trainers>, String>) -> Self {
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
            seed,
            postflop_spot: None,
            drill: None,
            question: None,
            answer: None,
            hand: None,
            hand_sent: false,
        };
        // Dev override (and screenshots): TAFUL_TABLE_MODE=postflop|hand.
        match std::env::var("TAFUL_TABLE_MODE").as_deref() {
            Ok("postflop") => s.mode = Mode::Postflop,
            Ok("hand") => s.mode = Mode::Hand,
            _ => {}
        }
        if let Ok(seed) = std::env::var("TAFUL_TABLE_SEED").map(|v| v.parse::<u64>()) {
            s.seed = seed.unwrap_or(s.seed);
        }
        s.deal();
        s
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(1);
        self.seed
    }

    /// New spot or hand for the current mode.
    fn deal(&mut self) {
        let Ok(t) = self.trainers.clone() else { return };
        let seed = self.next_seed();
        self.answer = None;
        match self.mode {
            Mode::Spot => {
                self.question = t
                    .preflop
                    .spot(self.kind, seed)
                    .ok()
                    .map(|s| DrillSpot::Preflop(Box::new(s)));
            }
            Mode::Postflop => {
                self.question = t.post().and_then(|pf| {
                    pf.spot(self.postflop_spot.as_deref(), seed)
                        .ok()
                        .map(|s| DrillSpot::Postflop(Box::new(s)))
                });
            }
            Mode::Drill => {
                self.question = self
                    .drill
                    .as_ref()
                    .and_then(|b| t.spot_for_bucket(b, seed).ok());
            }
            Mode::Hand => {
                // A new hand starts by the button so every seat comes up.
                self.hand = FullHand::new(&t.preflop, t.post(), None, seed).ok();
                self.hand_sent = false;
            }
        }
    }

    /// Drill a leak (the home screen's "drill this").
    pub fn start_drill(&mut self, bucket: LeakBucket, cx: &mut Context<Self>) {
        self.drill = Some(bucket);
        self.mode = Mode::Drill;
        self.deal();
        cx.notify();
    }

    fn set_postflop_spot(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.postflop_spot = id;
        self.mode = Mode::Postflop;
        self.deal();
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.deal();
            cx.notify();
        }
    }

    fn set_kind(&mut self, kind: SpotKind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.mode = Mode::Spot;
        self.deal();
        cx.notify();
    }

    fn act(&mut self, action: Action, cx: &mut Context<Self>) {
        let Ok(t) = self.trainers.clone() else { return };
        match self.mode {
            Mode::Hand => {
                if let Some(h) = &mut self.hand {
                    if let Err(e) = h.act(&t.preflop, t.post(), action) {
                        eprintln!("taful: hand: {e}");
                    }
                    if h.end.is_some() && !self.hand_sent {
                        self.hand_sent = true;
                        cx.emit(TableEvent::HandOver(Box::new(h.clone())));
                    }
                }
            }
            _ => {
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
        div().flex().gap(px(t.spacing.sm)).children(buttons)
    }

    fn message(&self, text: String) -> Div {
        div()
            .w(px(FELT_W))
            .text_size(px(self.ui.theme.text_size.md))
            .text_color(self.palette.text_muted)
            .child(text)
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
    fn question_view(&self, t: &Trainers, cx: &mut Context<Self>) -> Div {
        let (p, th, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        if self.mode == Mode::Postflop
            && let Some(msg) = self.postflop_message(t)
        {
            return self.message(msg);
        }
        let Some(q) = &self.question else {
            return self.message(i18n.tr("table-no-spot"));
        };
        let panel = match &self.answer {
            Some(d) => felt::panel(&self.ui, p)
                .child(felt::explanation(&self.ui, p, &Explain::of(d)))
                .child(
                    self.button("next", i18n.tr("table-next-spot"), true, cx, |this, cx| {
                        this.deal();
                        cx.notify();
                    }),
                ),
            None => felt::panel(&self.ui, p).child(
                div()
                    .text_size(px(th.text_size.md))
                    .text_color(p.text_muted)
                    .child(i18n.tr("table-spot-prompt")),
            ),
        };
        let actions = if self.answer.is_none() {
            q.actions(t)
        } else {
            vec![]
        };
        div()
            .flex()
            .gap(px(th.spacing.xl))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(th.spacing.lg))
                    .child(felt::table(&self.ui, p, q.state(), q.hero(), false))
                    .child(self.actions_bar(q.state(), &actions, cx)),
            )
            .child(panel)
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

    fn hand_view(&self, t: &Trainers, cx: &mut Context<Self>) -> Div {
        let (p, th, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let Some(h) = &self.hand else {
            return self.message(i18n.tr("table-no-spot"));
        };
        let over = h.end.is_some();
        let decisions = h.decisions.iter().map(|d| {
            div()
                .flex()
                .justify_between()
                .text_size(px(th.text_size.sm))
                .child(format!(
                    "{} · {} · {}",
                    i18n.tr(&format!("street-{}", street_id(d.context.street))),
                    d.graded.position.label(),
                    felt::action_short(&self.ui, d.graded.actions[d.graded.chosen])
                ))
                .child(
                    div()
                        .text_color(p.accent)
                        .child(i18n.tr(&felt::grade_key(d.graded.grade))),
                )
        });
        let mut panel = felt::panel(&self.ui, p);
        if h.decisions.is_empty() && !over {
            panel = panel.child(
                div()
                    .text_size(px(th.text_size.md))
                    .text_color(p.text_muted)
                    .child(i18n.tr("table-spot-prompt")),
            );
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
        if let Some(text) = self.end_text(h) {
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
                )
                .child(
                    div()
                        .flex()
                        .gap(px(th.spacing.sm))
                        .child(self.button(
                            "new_hand",
                            i18n.tr("table-new-hand"),
                            true,
                            cx,
                            |this, cx| {
                                this.deal();
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
                        }),
                );
        }
        let actions = h.hero_actions(&t.preflop);
        div()
            .flex()
            .gap(px(th.spacing.xl))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(th.spacing.lg))
                    .child(felt::table(&self.ui, p, &h.state, h.hero, over))
                    .child(self.actions_bar(&h.state, &actions, cx)),
            )
            .child(panel)
    }

    /// The drill's bucket as one line.
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
        parts.join(" · ")
    }
}

impl Render for TableScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, th) = (self.palette, self.ui.theme.clone());
        let ui = self.ui.clone();
        let i18n = &ui.i18n;
        let trainers = match &self.trainers {
            Ok(t) => t.clone(),
            Err(e) => {
                return div()
                    .text_color(p.text_muted)
                    .child(i18n.tr_args("table-chart-error", &[("error", e.clone().into())]));
            }
        };
        let mut modes = div()
            .flex()
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
        let body = match self.mode {
            Mode::Hand => self.hand_view(&trainers, cx),
            _ => self.question_view(&trainers, cx),
        };
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
            .gap(px(th.spacing.lg))
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
            .child(body)
    }
}

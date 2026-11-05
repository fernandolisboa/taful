//! Table screen (spec §10, §15.1 #4), M3: preflop spot mode and full hands
//! up to the flop against the reference bots, every hero decision graded.

use std::rc::Rc;

use gpui::{Context, Div, FontWeight, SharedString, Stateful, Window, div, prelude::*, px};
use taful_app::i18n::Locale;
use taful_app::preflop::{
    Action, BB_CHIPS, Card, Chips, FullHand, GameState, GradedDecision, HandEnd, PreflopTrainer,
    Seat, Spot, SpotKind, Suit,
};

use crate::Ui;
use crate::theme::Palette;

const FELT_W: f32 = 640.;
const FELT_H: f32 = 300.;
const SEAT_W: f32 = 120.;
const SEAT_H: f32 = 72.;
const CARD_W: f32 = 26.;
const CARD_H: f32 = 36.;
const PANEL_W: f32 = 340.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Spot,
    Hand,
}

pub struct TableScreen {
    ui: Rc<Ui>,
    palette: Palette,
    trainer: Result<Rc<PreflopTrainer>, String>,
    mode: Mode,
    kind: SpotKind,
    /// Seeds for spots and hands; bumped for every new deal.
    seed: u64,
    spot: Option<Spot>,
    answer: Option<GradedDecision>,
    hand: Option<FullHand>,
}

impl TableScreen {
    pub fn new(ui: Rc<Ui>) -> Self {
        let palette = Palette::new(&ui.theme.colors);
        let trainer = PreflopTrainer::builtin()
            .map(Rc::new)
            .map_err(|e| e.to_string());
        // A fresh sequence every launch; each deal is still reproducible
        // from its own seed.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut s = Self {
            ui,
            palette,
            trainer,
            mode: Mode::Spot,
            kind: SpotKind::Rfi,
            seed,
            spot: None,
            answer: None,
            hand: None,
        };
        s.deal();
        s
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(1);
        self.seed
    }

    /// New spot or hand for the current mode.
    fn deal(&mut self) {
        let Ok(t) = self.trainer.clone() else { return };
        let seed = self.next_seed();
        match self.mode {
            Mode::Spot => {
                self.spot = t.spot(self.kind, seed).ok();
                self.answer = None;
            }
            Mode::Hand => self.hand = FullHand::new(&t, None, seed).ok(),
        }
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
        let Ok(t) = self.trainer.clone() else { return };
        match self.mode {
            Mode::Spot => {
                if self.answer.is_none()
                    && let Some(spot) = &self.spot
                {
                    self.answer = spot.answer(&t, action).ok();
                }
            }
            Mode::Hand => {
                if let Some(h) = &mut self.hand {
                    let _ = h.act(&t, action);
                }
            }
        }
        cx.notify();
    }

    // ---- formatting ---------------------------------------------------------

    fn num(&self, x: f64, decimals: usize) -> String {
        let s = format!("{x:.decimals$}");
        match self.ui.i18n.locale() {
            Locale::PtBr => s.replace('.', ","),
            Locale::EnUs => s,
        }
    }

    fn bb(&self, chips: Chips) -> String {
        let v = chips as f64 / BB_CHIPS as f64;
        self.num(v, if chips.is_multiple_of(BB_CHIPS) { 0 } else { 1 })
    }

    fn action_label(&self, state: &GameState, a: Action) -> String {
        let i18n = &self.ui.i18n;
        let amount = |c: Chips| self.bb(c).into();
        match a {
            Action::Fold => i18n.tr("table-action-fold"),
            Action::Check => i18n.tr("table-action-check"),
            Action::Call => {
                let to_call = state.bounds().map_or(0, |b| b.to_call);
                i18n.tr_args("table-action-call", &[("amount", amount(to_call))])
            }
            Action::Bet(to) | Action::Raise(to) => {
                i18n.tr_args("table-action-raise", &[("amount", amount(to))])
            }
            Action::AllIn => {
                let to = state.bounds().and_then(|b| b.max_to).unwrap_or(0);
                i18n.tr_args("table-action-allin", &[("amount", amount(to))])
            }
        }
    }

    /// Short label for logs and the decision table (no table state needed).
    fn action_short(&self, a: Action) -> String {
        let i18n = &self.ui.i18n;
        match a {
            Action::Fold => i18n.tr("table-action-fold"),
            Action::Check => i18n.tr("table-action-check"),
            Action::Call => i18n.tr("table-log-call"),
            Action::Bet(to) | Action::Raise(to) => {
                i18n.tr_args("table-action-raise", &[("amount", self.bb(to).into())])
            }
            Action::AllIn => i18n.tr("table-log-allin"),
        }
    }

    // ---- pieces ---------------------------------------------------------------

    fn card(&self, c: Option<Card>) -> impl IntoElement {
        let (p, t) = (&self.palette, &self.ui.theme);
        let base = div()
            .w(px(CARD_W))
            .h(px(CARD_H))
            .rounded(px(t.radii.sm))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_size(px(t.text_size.sm))
            .font_weight(FontWeight::BOLD);
        match c {
            Some(c) => {
                let suit = match c.suit() {
                    Suit::CLUBS => "♣",
                    Suit::DIAMONDS => "♦",
                    Suit::HEARTS => "♥",
                    _ => "♠",
                };
                base.bg(p.card_face)
                    .text_color(self.palette.suit(c.suit()))
                    .child(c.rank().to_char().to_string())
                    .child(suit)
            }
            None => base.bg(p.card_back).border_1().border_color(p.rail_edge),
        }
    }

    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        primary: bool,
        cx: &mut Context<Self>,
        on: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let (p, t) = (&self.palette, &self.ui.theme);
        div()
            .id(id)
            .px(px(t.spacing.lg))
            .py(px(t.spacing.sm))
            .rounded(px(t.radii.md))
            .cursor_pointer()
            .text_size(px(t.text_size.md))
            .font_weight(FontWeight::SEMIBOLD)
            .when(primary, |d| d.bg(p.accent).text_color(p.on_accent))
            .when(!primary, |d| {
                d.bg(p.surface_raised)
                    .text_color(p.text)
                    .border_1()
                    .border_color(p.border)
                    .hover(|s| s.border_color(p.accent))
            })
            .on_click(cx.listener(move |this, _, _, cx| on(this, cx)))
            .child(label)
    }

    fn chip(
        &self,
        id: impl Into<gpui::ElementId>,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
        on: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let (p, t) = (&self.palette, &self.ui.theme);
        div()
            .id(id)
            .px(px(t.spacing.md))
            .py(px(t.spacing.xs))
            .rounded(px(t.radii.pill))
            .cursor_pointer()
            .text_size(px(t.text_size.sm))
            .border_1()
            .when(selected, |d| {
                d.bg(p.accent_muted)
                    .border_color(p.accent)
                    .text_color(p.text)
            })
            .when(!selected, |d| {
                d.border_color(p.border)
                    .text_color(p.text_muted)
                    .hover(|s| s.border_color(p.accent))
            })
            .on_click(cx.listener(move |this, _, _, cx| on(this, cx)))
            .child(label)
    }

    /// Felt, seats, pot and board. `reveal` shows every hand (end of hand).
    fn table(&self, state: &GameState, hero: Seat, reveal: bool) -> impl IntoElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let n = state.players().len();
        let (cx0, cy0) = (FELT_W / 2., FELT_H / 2.);
        let (rx, ry) = (
            FELT_W / 2. - SEAT_W / 2. + 8.,
            FELT_H / 2. - SEAT_H / 2. + 18.,
        );
        let to_act = state.to_act();
        let seats = (0..n).map(|seat| {
            // Hero sits at the bottom; the others clockwise from hero's left.
            let k = (seat + n - hero) % n;
            let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
            let (x, y) = (cx0 + rx * angle.cos(), cy0 + ry * angle.sin());
            let pl = &state.players()[seat];
            let pos = state.position(seat);
            let active = to_act == Some(seat);
            let show = seat == hero || (reveal && !pl.folded);
            let cards: Vec<Option<Card>> = if pl.folded {
                vec![]
            } else if show {
                pl.hole.cards().map(Some).to_vec()
            } else {
                vec![None, None]
            };
            let last = state.actions()[0]
                .iter()
                .rev()
                .find(|(s, _)| *s == seat)
                .map(|(_, a)| self.action_short(*a));
            div()
                .absolute()
                .left(px(x - SEAT_W / 2.))
                .top(px(y - SEAT_H / 2.))
                .w(px(SEAT_W))
                .h(px(SEAT_H))
                .rounded(px(t.radii.lg))
                .border_1()
                .border_color(if active { p.accent } else { p.rail_edge })
                .bg(if active { p.seat_active } else { p.seat })
                .when(pl.folded, |d| d.opacity(0.5))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .gap(px(t.spacing.xs))
                        .text_size(px(t.text_size.sm))
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if seat == hero { p.accent } else { p.text })
                                .child(pos.label()),
                        )
                        .child(div().text_color(p.text_muted).child(
                            i18n.tr_args("table-stack", &[("amount", self.bb(pl.stack).into())]),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .children(cards.into_iter().map(|c| self.card(c))),
                )
                .when_some(last, |d, l| {
                    d.child(
                        div()
                            .text_size(px(t.text_size.sm))
                            .text_color(p.text_muted)
                            .child(l),
                    )
                })
        });
        // Chips in front of each seat on this street, between seat and pot.
        let bets = (0..n).filter_map(|seat| {
            let c = state.players()[seat].street_commit;
            if c == 0 {
                return None;
            }
            let k = (seat + n - hero) % n;
            let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
            let (x, y) = (cx0 + rx * 0.55 * angle.cos(), cy0 + ry * 0.5 * angle.sin());
            Some(
                div()
                    .absolute()
                    .left(px(x - 28.))
                    .top(px(y - 10.))
                    .w(px(56.))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .px(px(t.spacing.sm))
                            .rounded(px(t.radii.pill))
                            .bg(p.rail)
                            .text_size(px(t.text_size.sm))
                            .text_color(p.accent)
                            .child(self.bb(c)),
                    ),
            )
        });
        let board = state.board().to_vec();
        div()
            .relative()
            .w(px(FELT_W))
            .h(px(FELT_H))
            .flex_none()
            .child(
                div()
                    .absolute()
                    .left(px(SEAT_W / 2.))
                    .top(px(SEAT_H / 2. - 6.))
                    .w(px(FELT_W - SEAT_W))
                    .h(px(FELT_H - SEAT_H + 12.))
                    .rounded(px(t.radii.pill))
                    .bg(p.felt)
                    .border(px(6.))
                    .border_color(p.rail)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(t.spacing.sm))
                    .child(
                        div()
                            .text_size(px(t.text_size.md))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(
                                i18n.tr_args(
                                    "table-pot",
                                    &[("amount", self.bb(state.pot()).into())],
                                ),
                            ),
                    )
                    .when(!board.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .gap(px(t.spacing.xs))
                                .children(board.iter().map(|&c| self.card(Some(c)))),
                        )
                    }),
            )
            .children(bets)
            .children(seats)
    }

    /// Hero's action buttons (exactly the tree's sizes, D4).
    fn actions_bar(&self, state: &GameState, actions: &[Action], cx: &mut Context<Self>) -> Div {
        let t = &self.ui.theme;
        let buttons: Vec<_> = actions
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                let label = self.action_label(state, a);
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

    /// Grade, EV loss and the per-action table for one decision (spec §8).
    fn explanation(&self, d: &GradedDecision) -> impl IntoElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let best = d.best();
        let header = div()
            .flex()
            .items_baseline()
            .gap(px(t.spacing.md))
            .child(
                div()
                    .text_size(px(t.text_size.lg))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.accent)
                    .child(i18n.tr(&format!("table-grade-{}", d.grade.id()))),
            )
            .child(
                div()
                    .text_size(px(t.text_size.sm))
                    .text_color(p.text_muted)
                    .child(i18n.tr_args(
                        "table-ev-loss",
                        &[("amount", self.num(d.ev_loss_bb, 2).into())],
                    )),
            );
        let cell = |s: String, w: f32| div().w(px(w)).child(s);
        let head = div()
            .flex()
            .text_size(px(t.text_size.sm))
            .text_color(p.text_muted)
            .child(cell(i18n.tr("table-col-action"), 120.))
            .child(cell(i18n.tr("table-col-hand-freq"), 70.))
            .child(cell(i18n.tr("table-col-ev"), 60.))
            .child(cell(i18n.tr("table-col-range-freq"), 60.));
        let rows = d.actions.iter().enumerate().map(|(i, &a)| {
            let mark = if i == d.chosen { "▸ " } else { "" };
            div()
                .flex()
                .text_size(px(t.text_size.sm))
                .when(i == best, |r| {
                    r.text_color(p.accent).font_weight(FontWeight::SEMIBOLD)
                })
                .child(cell(format!("{mark}{}", self.action_short(a)), 120.))
                .child(cell(
                    format!("{}%", self.num(f64::from(d.hand_freqs[i]) * 100., 0)),
                    70.,
                ))
                .child(cell(self.num(f64::from(d.hand_evs[i]), 2), 60.))
                .child(cell(
                    format!("{}%", self.num(f64::from(d.range_freqs[i]) * 100., 0)),
                    60.,
                ))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.sm))
            .child(header)
            .child(
                div()
                    .text_size(px(t.text_size.sm))
                    .text_color(p.text_muted)
                    .child(i18n.tr_args(
                        "table-decision-context",
                        &[
                            ("position", d.position.label().into()),
                            ("hand", d.hand.to_string().into()),
                        ],
                    )),
            )
            .child(head)
            .children(rows)
    }

    fn panel(&self) -> Div {
        let (p, t) = (&self.palette, &self.ui.theme);
        div()
            .w(px(PANEL_W))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(t.spacing.lg))
            .p(px(t.spacing.lg))
            .rounded(px(t.radii.lg))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
    }

    fn spot_view(&self, t: &PreflopTrainer, cx: &mut Context<Self>) -> Div {
        let (p, th, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let Some(spot) = &self.spot else {
            return div().child(i18n.tr("table-no-spot"));
        };
        let panel = match &self.answer {
            Some(d) => self.panel().child(self.explanation(d)).child(self.button(
                "next",
                i18n.tr("table-next-spot"),
                true,
                cx,
                |this, cx| {
                    this.deal();
                    cx.notify();
                },
            )),
            None => self.panel().child(
                div()
                    .text_size(px(th.text_size.md))
                    .text_color(p.text_muted)
                    .child(i18n.tr("table-spot-prompt")),
            ),
        };
        let actions = if self.answer.is_none() {
            t.actions(spot.node)
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
                    .child(self.table(&spot.state, spot.hero, false))
                    .child(self.actions_bar(&spot.state, &actions, cx)),
            )
            .child(panel)
    }

    fn hand_view(&self, t: &PreflopTrainer, cx: &mut Context<Self>) -> Div {
        let (p, th, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let Some(h) = &self.hand else {
            return div().child(i18n.tr("table-no-spot"));
        };
        let over = h.end.is_some();
        let end_text = h.end.as_ref().map(|e| match e {
            HandEnd::Settled(_) | HandEnd::HeroFolded => i18n.tr_args(
                "table-hand-over-settled",
                &[("amount", self.num(h.hero_net_bb().unwrap_or(0.), 1).into())],
            ),
            HandEnd::ReachedFlop => i18n.tr("table-hand-over-flop"),
            HandEnd::LeftLibrary(line) => {
                i18n.tr_args("table-hand-over-left", &[("line", line.to_string().into())])
            }
        });
        let decisions = h.decisions.iter().map(|d| {
            div()
                .flex()
                .justify_between()
                .text_size(px(th.text_size.sm))
                .child(format!(
                    "{} · {}",
                    d.position.label(),
                    self.action_short(d.actions[d.chosen])
                ))
                .child(
                    div()
                        .text_color(p.accent)
                        .child(i18n.tr(&format!("table-grade-{}", d.grade.id()))),
                )
        });
        let mut panel = self.panel();
        if let Some(d) = h.decisions.last() {
            panel = panel.child(self.explanation(d));
        }
        panel = panel
            .child(
                div()
                    .text_size(px(th.text_size.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(i18n.tr("table-decisions")),
            )
            .children(decisions);
        if let Some(text) = end_text {
            panel = panel
                .child(div().text_size(px(th.text_size.md)).child(text))
                .child(
                    div()
                        .text_size(px(th.text_size.sm))
                        .text_color(p.text_muted)
                        .child(i18n.tr_args(
                            "table-hand-ev-loss",
                            &[("amount", self.num(h.total_ev_loss_bb(), 2).into())],
                        )),
                )
                .child(self.button(
                    "new_hand",
                    i18n.tr("table-new-hand"),
                    true,
                    cx,
                    |this, cx| {
                        this.deal();
                        cx.notify();
                    },
                ));
        }
        let actions = h.hero_actions(t);
        div()
            .flex()
            .gap(px(th.spacing.xl))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(th.spacing.lg))
                    .child(self.table(&h.state, h.hero, over))
                    .child(self.actions_bar(&h.state, &actions, cx)),
            )
            .child(panel)
    }
}

impl Render for TableScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, th, i18n) = (self.palette, self.ui.theme.clone(), &self.ui.i18n);
        let trainer = match &self.trainer {
            Ok(t) => t.clone(),
            Err(e) => {
                return div()
                    .text_color(p.text_muted)
                    .child(i18n.tr_args("table-chart-error", &[("error", e.clone().into())]));
            }
        };
        let modes = div()
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
                "mode_hand",
                i18n.tr("table-mode-hand"),
                self.mode == Mode::Hand,
                cx,
                |this, cx| this.set_mode(Mode::Hand, cx),
            ));
        let kinds: Vec<_> = trainer
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
        let body = match self.mode {
            Mode::Spot => self.spot_view(&trainer, cx).into_any_element(),
            Mode::Hand => self.hand_view(&trainer, cx).into_any_element(),
        };
        let title: SharedString = i18n.tr("table-subtitle").into();
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
            .child(body)
    }
}

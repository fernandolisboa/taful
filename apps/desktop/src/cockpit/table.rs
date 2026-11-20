//! Cockpit's table screen: a strip of pickers, the table with hero's actions
//! under it, the hand line, and the evaluation of the last decision with
//! the range it was made in. The play itself (dealing, grading, the replay)
//! is the table screen's; this file only draws it.
//!
//! The panels are fixed: the layout editor arranges Classic's panels, so
//! its button is shown and does nothing here yet.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, Div, FontWeight, SharedString, Stateful, Window, deferred, div, point,
    prelude::*, px,
};
// Under another name: the color lint reads the call as a named color.
use gpui::anchored as window_layer;
use taful_app::insight::Insight;
use taful_app::model::{DrillSpot, Grade, HandEnd, Street, Trainers};
use taful_app::preflop::{Action, Card, Chips, GameState, GradedDecision, Seat};
use taful_app::story::{Story, StoryEvent};
use taful_app::template::street_id;

use super::frame::{BAR, RAIL};
use super::kit::{self, Btn, EDGE, HEAD, MONO, STRONG, btn, kbd, label};
use super::pal::{Pal, TablePal, Tint};
use super::{eval, felt, line};
use crate::Ui;
use crate::felt::{action_label, bb, num, spot_label};
use crate::screens::table::{Busy, Mode, TableEvent, TableScreen};
use crate::theme::alpha;

/// Width of the evaluation panel and height of the hand line.
const EVAL_W: f32 = 584.;
const LINE_H: f32 = 162.;
/// The strip of pickers and the gap under it.
const STRIP_TOP: f32 = 10.;
const STRIP_H: f32 = 34.;
const PANELS_TOP: f32 = 54.;
const PANEL_GAP: f32 = 10.;
/// The strip under the felt.
pub(super) const ACTBAR_H: f32 = 62.;
/// From this pane width (a 1280 px window) up the evaluation sits beside
/// the table, as designed. Below it the panels stack on the whole width and
/// the page scrolls down to the evaluation.
const WIDE_PANE: f32 = super::frame::WIDE_PANE;
/// The narrowest pane the stacked panels are laid out in: what the smallest
/// window leaves.
const MIN_PANE_W: f32 = crate::MIN_WINDOW.0 - RAIL;
/// The table panel around the stage at its smallest and at its natural
/// size: the stage, the header, the action bar and the border.
const TABLE_MIN_H: f32 = felt::STAGE_MIN_H + HEAD + ACTBAR_H + 2.;
const TABLE_NATURAL_H: f32 = felt::STAGE_H + HEAD + ACTBAR_H + 2.;

/// An open picker.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Situation,
    Opponent,
    Stack,
}

/// What a deal's decision points were computed for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Deal {
    Question(u64),
    Hand(u64),
}

/// Cockpit's slot on the table screen.
#[derive(Default)]
pub struct TableState {
    menu: Option<Menu>,
    deal: Option<Deal>,
    /// The decision point of each of the deal's decisions, in order (one
    /// for a question; a hand's next one is added when hero's turn comes).
    insights: Vec<Option<Rc<Insight>>>,
    /// A question's story with hero's answer played.
    after: Option<Rc<Story>>,
    /// After an answer, show the table hero decided on (one step back
    /// from the answer) instead of the answer played.
    pub(super) back: bool,
    /// The seed of the first deal seen, to number the deals.
    first_seed: Option<u64>,
    /// The strip's height on the last frame (its pickers wrap).
    strip_h: Rc<std::cell::Cell<f32>>,
}

/// One action of the story, worded.
pub(super) struct Ev {
    /// Index of the state right after it.
    pub step: usize,
    pub street: Street,
    pub seat: Seat,
    /// The seat's position.
    pub who: String,
    /// `open 2,5`, `call 7`, `fold`.
    pub what: String,
    pub action: Action,
    pub hero: bool,
    /// A bet's share of the pot it was made into.
    pub pot_share: Option<f64>,
    /// Hero's graded decisions.
    pub grade: Option<Grade>,
    /// A fold before anyone entered the pot.
    pub opening_fold: bool,
}

/// A street of the story: its cards and the pot it started with.
pub(super) struct Dealt {
    pub street: Street,
    pub cards: Vec<Card>,
    pub pot: Chips,
    pub step: usize,
}

/// Everything the panels draw from.
pub(super) struct Scene<'a> {
    pub ui: &'a Ui,
    pub pal: Pal,
    pub tp: TablePal,
    pub story: &'a Story,
    /// The shown state's index in the story.
    pub at: usize,
    /// The replay is before the story's end.
    pub playing: bool,
    pub hero: Seat,
    pub evs: Vec<Ev>,
    pub streets: Vec<Dealt>,
    /// Show every live hand.
    pub reveal: bool,
    pub wins: Vec<(Seat, Chips)>,
    /// Bot profile names by seat.
    pub tags: Vec<Option<String>>,
    /// Hero is to act on the shown state.
    pub hero_turn: bool,
}

impl Scene<'_> {
    pub fn state(&self) -> &GameState {
        &self.story.states[self.at]
    }
}

/// The story's actions and streets, worded. `grades` are hero's graded
/// decisions as `(state index after the action, grade)`.
fn word(ui: &Ui, story: &Story, hero: Seat, grades: &[(usize, Grade)]) -> (Vec<Ev>, Vec<Dealt>) {
    let i18n = &ui.i18n;
    let mut evs = Vec::new();
    let mut streets = vec![Dealt {
        street: Street::Preflop,
        cards: Vec::new(),
        pot: story.states[0].pot(),
        step: 0,
    }];
    let (mut raises, mut opened) = (0usize, false);
    for e in &story.events {
        match e {
            StoryEvent::Dealt {
                street,
                cards,
                step,
            } => streets.push(Dealt {
                street: *street,
                cards: cards.clone(),
                pot: story.states[*step].pot(),
                step: *step,
            }),
            StoryEvent::Action(a) => {
                let preflop = a.street == Street::Preflop;
                // Hero's own fold is a graded decision: it stays apart.
                let opening_fold = preflop && !opened && a.action == Action::Fold && a.seat != hero;
                opened |= a.action != Action::Fold;
                if preflop && matches!(a.action, Action::Raise(_)) {
                    raises += 1;
                }
                let what = match a.action {
                    Action::Fold => i18n.tr("cockpit-act-fold"),
                    Action::Check => i18n.tr("cockpit-act-check"),
                    Action::Call => {
                        i18n.tr_args("cockpit-act-call", &[("amount", bb(ui, a.paid).into())])
                    }
                    Action::Bet(_) => {
                        i18n.tr_args("cockpit-act-bet", &[("amount", bb(ui, a.to).into())])
                    }
                    Action::Raise(_) if preflop && raises == 1 => {
                        i18n.tr_args("cockpit-act-open", &[("amount", bb(ui, a.to).into())])
                    }
                    Action::Raise(_) if preflop => i18n.tr_args(
                        "cockpit-act-nbet",
                        &[("n", (raises + 1).into()), ("amount", bb(ui, a.to).into())],
                    ),
                    Action::Raise(_) => {
                        i18n.tr_args("cockpit-act-raise", &[("amount", bb(ui, a.to).into())])
                    }
                    Action::AllIn => {
                        i18n.tr_args("cockpit-act-allin", &[("amount", bb(ui, a.to).into())])
                    }
                };
                let before = story.states[a.step - 1].pot();
                evs.push(Ev {
                    step: a.step,
                    street: a.street,
                    seat: a.seat,
                    who: a.position.label().to_string(),
                    what,
                    action: a.action,
                    hero: a.seat == hero,
                    pot_share: (matches!(a.action, Action::Bet(_)) && before > 0)
                        .then(|| a.to as f64 / before as f64),
                    grade: grades.iter().find(|(s, _)| *s == a.step).map(|(_, g)| *g),
                    opening_fold,
                });
            }
        }
    }
    (evs, streets)
}

/// Reads the decision points the panels need, once per deal and decision.
fn sync(screen: &mut TableScreen, t: &Trainers) {
    let stepping = screen.answer.is_some() && !screen.playing();
    let st = &mut screen.cockpit;
    st.back &= stepping;
    st.first_seed.get_or_insert(screen.seed);
    let deal = match screen.mode {
        Mode::Hand => screen.hand.as_ref().map(|h| Deal::Hand(h.seed)),
        _ => screen
            .question
            .as_ref()
            .map(|_| Deal::Question(screen.generation)),
    };
    if st.deal != deal {
        st.deal = deal;
        st.insights.clear();
        st.after = None;
    }
    match screen.mode {
        Mode::Hand => {
            if let Some(h) = &screen.hand {
                while st.insights.len() < h.decisions.len() {
                    st.insights.push(None);
                }
                if st.insights.len() == h.decisions.len()
                    && let Some(i) = h.insight(t)
                {
                    st.insights.push(Some(Rc::new(i)));
                }
            }
        }
        _ => {
            if let Some(q) = &screen.question {
                if st.insights.is_empty() {
                    st.insights.push(t.insight(q).map(Rc::new));
                }
                if let (Some(d), None) = (&screen.answer, &st.after) {
                    st.after = q
                        .state()
                        .apply(d.actions[d.chosen])
                        .ok()
                        .and_then(|s| Story::of(&s).ok())
                        .map(Rc::new);
                }
            }
        }
    }
}

/// The bar's crumb: the mode, the situation and the deal's number.
pub(super) fn crumb(screen: &TableScreen) -> Option<String> {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let n = screen
        .cockpit
        .first_seed
        .map_or(1, |first| screen.seed.wrapping_sub(first) + 1);
    let mut parts = vec![i18n.tr(match screen.mode {
        Mode::Spot => "table-mode-spot",
        Mode::Postflop => "table-mode-postflop",
        Mode::Hand => "table-mode-hand",
        Mode::Drill => "table-mode-drill",
    })];
    match (screen.mode, &screen.question) {
        (Mode::Spot, _) => parts.push(i18n.tr(&format!("table-spot-kind-{}", screen.kind.id()))),
        (Mode::Postflop, Some(DrillSpot::Postflop(s))) => parts.push(spot_label(ui, &s.spot_id)),
        (Mode::Drill, _) => {
            if let Some(b) = &screen.drill {
                parts.push(spot_label(ui, &b.spot));
            }
        }
        _ => {}
    }
    parts.push(i18n.tr_args(
        if screen.mode == Mode::Hand {
            "cockpit-table-crumb-hand"
        } else {
            "cockpit-table-crumb-spot"
        },
        &[("n", n.into())],
    ));
    Some(parts.join(" · "))
}

/// The key shown on each action's button.
fn action_keys(actions: &[Action]) -> Vec<&'static str> {
    const SIZES: [&str; 6] = ["1", "2", "3", "4", "5", "6"];
    let sizes = actions
        .iter()
        .filter(|a| matches!(a, Action::Bet(_) | Action::Raise(_)))
        .count();
    let mut next = 0;
    actions
        .iter()
        .map(|a| match a {
            Action::Fold => "F",
            Action::Check | Action::Call => "C",
            Action::AllIn => "A",
            Action::Bet(_) | Action::Raise(_) if sizes == 1 => "R",
            Action::Bet(_) | Action::Raise(_) => {
                next += 1;
                SIZES.get(next - 1).copied().unwrap_or("")
            }
        })
        .collect()
}

/// Hero's options now: empty unless hero can act.
fn open_actions(screen: &TableScreen, t: &Trainers) -> Vec<Action> {
    if screen.busy.is_some() || screen.playing() {
        return Vec::new();
    }
    match screen.mode {
        Mode::Hand => screen
            .hand
            .as_ref()
            .filter(|h| h.end.is_none())
            .map(|h| h.hero_actions(&t.preflop))
            .unwrap_or_default(),
        _ => match (&screen.question, &screen.answer) {
            (Some(q), None) => q.actions(t),
            _ => Vec::new(),
        },
    }
}

/// The deal is over and the next one can start.
fn can_deal(screen: &TableScreen) -> bool {
    if screen.busy.is_some() || screen.playing() {
        return false;
    }
    match screen.mode {
        Mode::Hand => screen.hand.as_ref().is_some_and(|h| h.end.is_some()),
        _ => screen.answer.is_some(),
    }
}

/// A key press on the table screen; `true` when it did something.
pub(super) fn key(screen: &mut TableScreen, key: &str, cx: &mut Context<TableScreen>) -> bool {
    if screen.cockpit.menu.take().is_some() {
        cx.notify();
        return key == "escape";
    }
    match key {
        "backspace" if screen.playback.is_some() => screen.replay_again(cx),
        "space" if screen.playing() => screen.skip_replay(cx),
        "space" if can_deal(screen) => {
            screen.deal(cx);
            cx.notify();
        }
        _ => {
            let Ok(t) = screen.trainers.clone() else {
                return false;
            };
            let actions = open_actions(screen, &t);
            let hit = action_keys(&actions)
                .iter()
                .position(|k| k.eq_ignore_ascii_case(key));
            match hit {
                Some(i) => screen.act(actions[i], cx),
                None => return false,
            }
        }
    }
    true
}

/// One opponent for the whole table replaces the per-seat choice of a full
/// hand; the other modes leave that choice alone.
fn leave_per_seat(screen: &mut TableScreen) {
    if screen.mode == Mode::Hand {
        screen.seats = None;
    }
}

/// A picker: its caption, its value and, when it has options, a caret.
fn select(
    id: &'static str,
    caption: String,
    value: String,
    caret: bool,
    pal: &Pal,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(32.))
        .px(px(10.))
        .rounded(px(5.))
        .border_1()
        .border_color(pal.line2)
        .bg(pal.panel)
        .text_size(px(12.5))
        .whitespace_nowrap()
        .child(label(&caption, pal).text_color(pal.dim))
        .child(SharedString::from(value))
        .when(caret, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(pal.panel2))
                .child(kit::icon("caret", 8., pal.muted).ml(px(4.)))
        })
}

/// A picker's list, under it. `rows` are `(label, selected, on click)`.
#[allow(clippy::type_complexity)]
fn menu(
    rows: Vec<(
        String,
        bool,
        Box<dyn Fn(&mut TableScreen, &mut Context<TableScreen>)>,
    )>,
    pal: &Pal,
    tp: &TablePal,
    cx: &mut Context<TableScreen>,
) -> impl IntoElement {
    let rows = rows.into_iter().enumerate().map(|(i, (text, on, click))| {
        div()
            .id(("menu_row", i))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(30.))
            .px(px(10.))
            .rounded(px(4.))
            .text_size(px(12.5))
            .whitespace_nowrap()
            .cursor_pointer()
            .when(on, |d| {
                d.text_color(pal.amber).font_weight(FontWeight::SEMIBOLD)
            })
            .hover(|s| s.bg(pal.raised))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.cockpit.menu = None;
                click(this, cx);
                cx.notify();
            }))
            .child(SharedString::from(text))
    });
    deferred(
        div()
            .absolute()
            .top(px(36.))
            .left_0()
            .min_w_full()
            .flex()
            .flex_col()
            .p(px(4.))
            .rounded(px(6.))
            .border_1()
            .border_color(pal.line2)
            .bg(pal.panel2)
            .shadow(kit::shadow(tp.shadow, 12., 28.))
            .occlude()
            .children(rows),
    )
    .with_priority(2)
}

/// The strip above the panels: the mode, the pickers and the layout button.
///
/// The pickers wrap onto more rows when they do not fit; the strip's height
/// is measured, and the panels start under it. `wide` off leaves the layout
/// button out.
fn strip(
    screen: &TableScreen,
    t: &Trainers,
    wide: bool,
    pal: &Pal,
    tp: &TablePal,
    cx: &mut Context<TableScreen>,
) -> Div {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let open = screen.cockpit.menu;
    let mut modes = vec![
        ("mode_spot", "table-mode-spot", Mode::Spot),
        ("mode_postflop", "table-mode-postflop", Mode::Postflop),
        ("mode_hand", "table-mode-hand", Mode::Hand),
    ];
    if screen.drill.is_some() {
        modes.push(("mode_drill", "table-mode-drill", Mode::Drill));
    }
    let last = modes.len() - 1;
    let segments = div()
        .flex()
        .flex_none()
        .overflow_hidden()
        .rounded(px(5.))
        .border_1()
        .border_color(pal.line2)
        .bg(pal.panel)
        .children(modes.into_iter().enumerate().map(|(i, (id, key, mode))| {
            let on = screen.mode == mode;
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(30.))
                .px(px(12.))
                .text_size(px(12.5))
                .whitespace_nowrap()
                .cursor_pointer()
                .when(i < last, |d| d.border_r_1().border_color(pal.line2))
                .map(|d| {
                    if on {
                        d.bg(pal.amber).text_color(pal.on_amber).font_weight(STRONG)
                    } else {
                        d.text_color(pal.muted)
                            .font_weight(FontWeight::MEDIUM)
                            .hover(|s| s.bg(pal.panel2))
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| this.set_mode(mode, cx)))
                .child(i18n.tr(key))
        }));

    fn toggle(
        m: Menu,
        cx: &mut Context<TableScreen>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
        cx.listener(move |this, _, _, cx| {
            this.cockpit.menu = (this.cockpit.menu != Some(m)).then_some(m);
            cx.notify();
        })
    }
    type Row = (
        String,
        bool,
        Box<dyn Fn(&mut TableScreen, &mut Context<TableScreen>)>,
    );
    let mut strip = div()
        .child(crate::tour::anchor("table_controls"))
        .absolute()
        .top(px(STRIP_TOP))
        .left(px(EDGE))
        .right(px(EDGE))
        .min_h(px(STRIP_H))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(crate::screens::table::measure_height(
            screen.cockpit.strip_h.clone(),
        ))
        .child(segments);

    // The situation: a kind of preflop spot, a postflop configuration, or
    // the leak being drilled.
    let situation: Option<(String, Vec<Row>)> = match screen.mode {
        Mode::Spot => Some((
            i18n.tr(&format!("table-spot-kind-{}", screen.kind.id())),
            t.preflop
                .available_kinds()
                .into_iter()
                .map(|k| -> Row {
                    (
                        i18n.tr(&format!("table-spot-kind-{}", k.id())),
                        screen.kind == k,
                        Box::new(move |this, cx| this.set_kind(k, cx)),
                    )
                })
                .collect(),
        )),
        Mode::Postflop => t.post().map(|pf| {
            let any = i18n.tr("table-postflop-any");
            let value = screen
                .postflop_spot
                .as_deref()
                .map_or(any.clone(), |id| spot_label(ui, id));
            let mut rows: Vec<Row> = vec![(
                any,
                screen.postflop_spot.is_none(),
                Box::new(|this, cx| this.set_postflop_spot(None, cx)),
            )];
            for s in pf.spots() {
                let id = s.id.clone();
                rows.push((
                    spot_label(ui, &id),
                    screen.postflop_spot.as_deref() == Some(id.as_str()),
                    Box::new(move |this, cx| this.set_postflop_spot(Some(id.clone()), cx)),
                ));
            }
            (value, rows)
        }),
        Mode::Drill => screen
            .drill
            .as_ref()
            .map(|b| (TableScreen::bucket_label(ui, b), Vec::new())),
        Mode::Hand => None,
    };
    if let Some((value, rows)) = situation {
        let has_rows = !rows.is_empty();
        let sel = select(
            "sel_situation",
            i18n.tr("table-row-spot"),
            value,
            has_rows,
            pal,
        )
        .when(has_rows, |d| d.on_click(toggle(Menu::Situation, cx)));
        strip = strip.child(
            div()
                .relative()
                .flex_none()
                .child(sel)
                .when(open == Some(Menu::Situation), |d| {
                    d.child(menu(rows, pal, tp, cx))
                }),
        );
    }

    // The opponent: the reference or a profile; a full hand can also seat
    // one profile per seat.
    if screen.mode != Mode::Drill {
        let name =
            |id: Option<&str>| i18n.tr(&format!("bot-profile-{}", id.unwrap_or("reference")));
        let per_seat = screen.mode == Mode::Hand && screen.seats.is_some();
        let mut rows: Vec<Row> = vec![(
            name(None),
            !per_seat && screen.opponent.is_none(),
            Box::new(|this, cx| {
                leave_per_seat(this);
                this.set_opponent(None, cx);
            }),
        )];
        for p in t.preflop.profiles() {
            let id = p.bot.profile().id.clone();
            rows.push((
                name(Some(&id)),
                !per_seat && screen.opponent.as_deref() == Some(id.as_str()),
                Box::new(move |this, cx| {
                    leave_per_seat(this);
                    this.set_opponent(Some(id.clone()), cx);
                }),
            ));
        }
        if screen.mode == Mode::Hand {
            rows.push((
                i18n.tr("table-seats-each"),
                per_seat,
                Box::new(|this, cx| {
                    if this.seats.is_none() {
                        this.set_per_seat(true, cx);
                    }
                }),
            ));
        }
        let value = if per_seat {
            i18n.tr("table-seats-each")
        } else {
            name(screen.opponent.as_deref())
        };
        strip = strip.child(
            div()
                .relative()
                .flex_none()
                .child(
                    select("sel_opponent", i18n.tr("table-opponent"), value, true, pal)
                        .on_click(toggle(Menu::Opponent, cx)),
                )
                .when(open == Some(Menu::Opponent), |d| {
                    d.child(menu(rows, pal, tp, cx))
                }),
        );
        if per_seat {
            // One picker per bot seat from hero's left; a click moves it
            // to the next profile.
            for (i, bot) in screen.seats.iter().flatten().enumerate() {
                strip = strip.child(
                    select(
                        "sel_seat",
                        i18n.tr_args("table-seat-left", &[("n", (i + 1).into())]),
                        name(bot.as_deref()),
                        false,
                        pal,
                    )
                    .id(("sel_seat", i))
                    .cursor_pointer()
                    .hover(|s| s.bg(pal.panel2))
                    .on_click(cx.listener(move |this, _, _, cx| this.cycle_seat(i, cx))),
                );
            }
        }
    }

    // The stack depth: the one the library is built for.
    let stack = i18n.tr_args(
        "table-stack",
        &[("amount", bb(ui, t.preflop.config().stack(0)).into())],
    );
    let stack_rows: Vec<Row> = vec![(stack.clone(), true, Box::new(|_, _| {}))];
    strip = strip.child(
        div()
            .relative()
            .flex_none()
            .child(
                select("sel_stack", i18n.tr("cockpit-sel-stack"), stack, true, pal)
                    .on_click(toggle(Menu::Stack, cx)),
            )
            .when(open == Some(Menu::Stack), |d| {
                d.child(menu(stack_rows, pal, tp, cx))
            }),
    );
    strip.when(wide, |d| {
        d.child(div().flex_1()).child(
            // Shown as designed; arranging Cockpit's panels is not built yet.
            btn(
                "layout_edit",
                i18n.tr("layout-edit-start"),
                None,
                Btn::Plain,
                pal,
            )
            .h(px(32.))
            .cursor_default(),
        )
    })
}

/// A message in place of the felt (nothing dealt, or no library).
fn notice(text: String, pal: &Pal) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(40.))
        .text_size(px(13.))
        .text_color(pal.muted)
        .child(text)
}

/// One of hero's actions under the felt. `picked` is the grade it got.
fn act_button(
    i: usize,
    text: String,
    key: &str,
    enabled: bool,
    picked: Option<(Grade, String)>,
    pal: &Pal,
) -> Stateful<Div> {
    let base = div()
        .id(("act", i))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(36.))
        .min_w(px(112.))
        .px(px(10.))
        .rounded(px(5.))
        .border_1()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap();
    let base = match &picked {
        Some((g, _)) => {
            let c: Tint = pal.grade(*g);
            base.border_color(c)
                .border_b_2()
                .bg(alpha(c, 0.1))
                .text_color(pal.text)
        }
        None if enabled => base
            .border_color(pal.line2)
            .bg(pal.panel2)
            .text_color(pal.text)
            .cursor_pointer()
            .hover(|s| s.bg(pal.raised)),
        None => base
            .border_color(pal.line2)
            .bg(pal.panel2)
            .text_color(pal.dim),
    };
    base.child(div().flex_none().child(SharedString::from(text)))
        .children(picked.map(|(g, name)| {
            let mark = if matches!(g, Grade::Best | Grade::Correct) {
                format!("✓ {name}")
            } else {
                name
            };
            kit::tracked(&mark.to_uppercase(), 10.5, 0.06).text_color(pal.grade(g))
        }))
        // The key sits at the button's right edge.
        .child(div().flex().flex_1().justify_end().child(kbd(key, pal)))
}

/// The strip under the felt: hero's actions, then what comes next.
fn actbar(
    screen: &TableScreen,
    t: &Trainers,
    scene: Option<&Scene<'_>>,
    pal: &Pal,
    tp: &TablePal,
    cx: &mut Context<TableScreen>,
) -> Div {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let mut left = div()
        .flex()
        .flex_1()
        .min_w(px(0.))
        .items_center()
        .gap(px(8.))
        .overflow_hidden();
    let message = |text: String| {
        div()
            .flex_shrink()
            .min_w(px(0.))
            .text_size(px(12.5))
            .line_height(px(17.))
            .text_color(pal.muted)
            .child(text)
    };
    let mut right = div().flex().flex_none().items_center().gap(px(8.));
    let playing = screen.playing();
    let open = open_actions(screen, t);
    if playing {
        let ticking = screen.playback.as_ref().is_some_and(|p| p.ticking);
        left = left.child(message(i18n.tr(if ticking {
            "table-replay-playing"
        } else {
            "table-replay-earlier"
        })));
        right = right.child(
            btn(
                "replay_skip",
                i18n.tr(if ticking {
                    "table-replay-skip"
                } else {
                    "table-replay-back"
                }),
                Some(&i18n.tr("cockpit-key-space")),
                Btn::Plain,
                pal,
            )
            .h(px(36.))
            .on_click(cx.listener(|this, _, _, cx| this.skip_replay(cx))),
        );
    } else if !open.is_empty() {
        let state = scene.map(Scene::state);
        let keys = action_keys(&open);
        for (i, &a) in open.iter().enumerate() {
            let text = state.map_or_else(String::new, |s| action_label(ui, s, a));
            left = left.child(
                act_button(i, text, keys[i], true, None, pal)
                    .on_click(cx.listener(move |this, _, _, cx| this.act(a, cx))),
            );
        }
    } else if let (Some(q), Some(d)) = (&screen.question, &screen.answer) {
        // The answered question: the same buttons, the chosen one graded.
        let actions = q.actions(t);
        let keys = action_keys(&actions);
        for (i, &a) in actions.iter().enumerate() {
            let picked =
                (i == d.chosen).then(|| (d.grade, i18n.tr(&crate::felt::grade_key(d.grade))));
            left = left.child(act_button(
                i,
                action_label(ui, q.state(), a),
                keys[i],
                false,
                picked,
                pal,
            ));
        }
    } else if let Some(text) = screen
        .hand
        .as_ref()
        .filter(|_| screen.mode == Mode::Hand)
        .and_then(|h| screen.end_text(h))
    {
        left = left.child(message(text));
    } else if let Some(busy) = screen.busy {
        left = left.child(message(i18n.tr(match busy {
            Busy::Dealing => "table-busy-dealing",
            Busy::Acting => "table-busy-acting",
        })));
    }
    if !playing
        && screen
            .playback
            .as_ref()
            .is_some_and(|p| !p.story.events.is_empty())
    {
        right = right.child(
            btn(
                "repeat",
                i18n.tr("cockpit-repeat"),
                Some("⌫"),
                Btn::Plain,
                pal,
            )
            .h(px(36.))
            .on_click(cx.listener(|this, _, _, cx| this.replay_again(cx))),
        );
    }
    if can_deal(screen) {
        let hand = screen.mode == Mode::Hand;
        if hand
            && screen
                .hand
                .as_ref()
                .is_some_and(|h| !h.decisions.is_empty())
        {
            right = right.child(
                btn(
                    "review_hand",
                    i18n.tr("table-review-hand"),
                    None,
                    Btn::Plain,
                    pal,
                )
                .h(px(36.))
                .on_click(cx.listener(|_, _, _, cx| cx.emit(TableEvent::ReviewHand))),
            );
        }
        right = right.child(
            btn(
                "next",
                i18n.tr(if hand {
                    "table-new-hand"
                } else {
                    "table-next-spot"
                }),
                Some(&i18n.tr("cockpit-key-space")),
                Btn::Amber,
                pal,
            )
            .h(px(36.))
            .px(px(16.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.deal(cx);
                cx.notify();
            })),
        );
    }
    div()
        .relative()
        .child(crate::tour::anchor("table_actions"))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(ACTBAR_H))
        .px(px(14.))
        .border_t_1()
        .border_color(pal.line)
        .bg(tp.actbar)
        .child(left)
        .child(right)
}

/// The table panel's header: where the hand is, the pot and the price.
fn table_head(screen: &TableScreen, scene: Option<&Scene<'_>>, pal: &Pal) -> Div {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let Some(sc) = scene else {
        return kit::head(&i18n.tr("nav-table"), None, true, pal);
    };
    let state = sc.state();
    let street = i18n.tr(&format!("street-{}", street_id(state.street())));
    let over = screen.mode == Mode::Hand
        && !sc.playing
        && screen.hand.as_ref().is_some_and(|h| h.end.is_some());
    let title = if over {
        i18n.tr("cockpit-table-over")
    } else if sc.playing {
        i18n.tr_args("cockpit-table-replay", &[("street", street.into())])
    } else if sc.hero_turn || screen.answer.is_some() {
        i18n.tr_args(
            "cockpit-table-turn",
            &[
                ("street", street.into()),
                ("position", state.position(sc.hero).label().into()),
            ],
        )
    } else {
        i18n.tr_args("cockpit-table-wait", &[("street", street.into())])
    };
    let pot = state.pot();
    let mut notes = vec![i18n.tr_args("cockpit-table-pot", &[("amount", bb(ui, pot).into())])];
    // The stack-to-pot ratio of the smaller of hero's stack and the
    // biggest stack hero is up against.
    let players = state.players();
    let against = players
        .iter()
        .enumerate()
        .filter(|(s, p)| *s != sc.hero && !p.folded)
        .map(|(_, p)| p.stack)
        .max();
    if let (Some(against), true) = (against, pot > 0) {
        let effective = players[sc.hero].stack.min(against);
        notes.push(i18n.tr_args(
            "cockpit-table-spr",
            &[("value", num(ui, effective as f64 / pot as f64, 1).into())],
        ));
    }
    // The price hero is (or was) offered: on the state before the answer.
    let decided_at = &sc.story.states[sc.at.min(decision_index(screen, sc))];
    if let Some(needs) =
        taful_app::insight::pot_odds(decided_at).filter(|_| decided_at.to_act() == Some(sc.hero))
    {
        let ratio = num(ui, (1. - needs) / needs, 1);
        let ratio = ratio.trim_end_matches('0').trim_end_matches([',', '.']);
        notes.push(i18n.tr_args(
            "cockpit-table-odds",
            &[
                ("ratio", format!("{ratio}:1").into()),
                ("percent", super::frame::percent(needs).into()),
            ],
        ));
    }
    let note = div()
        .flex()
        .gap(px(12.))
        .font_family(MONO)
        .whitespace_nowrap()
        .children(notes.into_iter().map(SharedString::from));
    kit::head(&title, Some(note.into_any_element()), true, pal)
}

/// A question is answered and the table shows the answer played (the
/// replay is at its end and `‹` has not stepped back from the answer).
pub(super) fn shows_answer(screen: &TableScreen) -> bool {
    answered(screen) && !screen.cockpit.back
}

/// A question is answered, its answer can be shown played, and the replay
/// is at its end.
pub(super) fn answered(screen: &TableScreen) -> bool {
    screen.mode != Mode::Hand
        && screen.answer.is_some()
        && screen.cockpit.after.is_some()
        && !screen.playing()
}

/// Index of the state hero's last decision was made on: the story's end,
/// or one before it when the story ends with hero's answer.
fn decision_index(screen: &TableScreen, sc: &Scene<'_>) -> usize {
    let last = sc.story.states.len() - 1;
    if shows_answer(screen) {
        last.saturating_sub(1)
    } else {
        last
    }
}

/// Cockpit's table screen.
pub fn table(
    screen: &mut TableScreen,
    window: &mut Window,
    cx: &mut Context<TableScreen>,
) -> AnyElement {
    let ui = screen.ui.clone();
    let (pal, tp) = (Pal::new(&ui), TablePal::new(&ui));
    let i18n = &ui.i18n;
    let vp = crate::caption::viewport(window);
    let (pane_w, pane_h) = (f32::from(vp.width) - RAIL, f32::from(vp.height) - BAR);
    let wide = pane_w >= WIDE_PANE;
    // The panels start under the strip, which may have wrapped.
    let panels_top = PANELS_TOP + (screen.cockpit.strip_h.get() - STRIP_H).max(0.);
    let around = panels_top + EDGE;
    let (w, h) = if wide {
        // The table and the hand line share the pane's height; a window too
        // short for the table at its smallest scrolls.
        let least = around + TABLE_MIN_H + PANEL_GAP + LINE_H;
        (pane_w, pane_h.max(least))
    } else {
        (pane_w.max(MIN_PANE_W), pane_h)
    };
    let pane = div().relative().flex_none().w(px(w));
    let trainers = match &screen.trainers {
        Ok(t) => t.clone(),
        Err(e) => {
            return pane
                .h(px(h))
                .child(notice(
                    i18n.tr_args("table-chart-error", &[("error", e.clone().into())]),
                    &pal,
                ))
                .into_any_element();
        }
    };
    sync(screen, &trainers);
    let screen: &TableScreen = screen;

    // What is on the table: a hand, or a question and maybe its answer.
    let hand = (screen.mode == Mode::Hand)
        .then_some(screen.hand.as_ref())
        .flatten();
    let playing = screen.playing();
    let after = screen
        .cockpit
        .after
        .as_deref()
        .filter(|_| shows_answer(screen));
    let story = after.or(screen.playback.as_ref().map(|p| &p.story));
    let hero = match (hand, &screen.question) {
        (Some(h), _) => Some(h.hero),
        (None, Some(q)) if screen.mode != Mode::Hand => Some(q.hero()),
        _ => None,
    };
    let scene = match (story, hero) {
        (Some(story), Some(hero)) => {
            let at = match (after, &screen.playback) {
                (Some(s), _) => s.states.len() - 1,
                (None, Some(p)) => p.at,
                (None, None) => 0,
            };
            // Hero's graded actions, by the state right after each.
            let grades: Vec<(usize, Grade)> = match hand {
                Some(h) => h
                    .decisions
                    .iter()
                    .map(|d| (d.step + 1, d.graded.grade))
                    .collect(),
                None => screen
                    .answer
                    .iter()
                    .filter(|_| after.is_some())
                    .map(|d| (story.states.len() - 1, d.grade))
                    .collect(),
            };
            let (evs, streets) = word(&ui, story, hero, &grades);
            let over = hand.is_some_and(|h| h.end.is_some()) && !playing;
            // What each winner took once the hand is settled.
            let mut wins: Vec<(Seat, Chips)> = Vec::new();
            if let (Some(HandEnd::Settled(st)), true) = (hand.and_then(|h| h.end.as_ref()), over) {
                for pot in &st.pots {
                    let share = (pot.amount - pot.rake) / pot.winners.len().max(1) as Chips;
                    for &seat in &pot.winners {
                        match wins.iter_mut().find(|(s, _)| *s == seat) {
                            Some((_, c)) => *c += share,
                            None => wins.push((seat, share)),
                        }
                    }
                }
                wins.retain(|(_, c)| *c > 0);
            }
            let state = &story.states[at];
            Some(Scene {
                ui: &ui,
                pal,
                tp,
                story,
                at,
                playing,
                hero,
                evs,
                streets,
                reveal: over,
                wins,
                tags: hand.map_or_else(Vec::new, |h| crate::felt::seat_tags(&ui, &h.seat_profiles)),
                hero_turn: !playing
                    && !over
                    && screen.answer.is_none()
                    && state.to_act() == Some(hero),
            })
        }
        _ => None,
    };

    // The decision the evaluation shows, with the point it was made at.
    let points = &screen.cockpit.insights;
    let (graded, insight): (Option<&GradedDecision>, Option<&Insight>) = match hand {
        Some(h) => match h.decisions.last() {
            Some(d) => (
                Some(&d.graded),
                points.get(h.decisions.len() - 1).and_then(Option::as_deref),
            ),
            None => (None, points.first().and_then(Option::as_deref)),
        },
        None => (
            screen.answer.as_ref().filter(|_| screen.mode != Mode::Hand),
            points.first().and_then(Option::as_deref),
        ),
    };

    // The table that decision was (or is to be) made on.
    let decided_on: Option<&GameState> = match hand {
        Some(h) => h.decisions.last().map(|d| &d.state).or(Some(&h.state)),
        None => screen.question.as_ref().map(DrillSpot::state),
    };

    let (left_w, table_h) = if wide {
        (
            w - 2. * EDGE - kit::GAP - EVAL_W,
            h - around - LINE_H - PANEL_GAP,
        )
    } else {
        // Stacked: the table takes the window's height but for the hand
        // line's header, which shows that the page goes on.
        (
            w - 2. * EDGE,
            (h - panels_top - PANEL_GAP - HEAD - 2.).clamp(TABLE_MIN_H, TABLE_NATURAL_H),
        )
    };
    let body = match &scene {
        Some(sc) => felt::stage(sc, left_w - 2., table_h - 2. - HEAD - ACTBAR_H).into_any_element(),
        None => {
            let text = if screen.mode == Mode::Postflop
                && let Some(msg) = screen.postflop_message(&trainers)
            {
                msg
            } else {
                i18n.tr(match screen.busy {
                    Some(Busy::Dealing) => "table-busy-dealing",
                    Some(Busy::Acting) => "table-busy-acting",
                    None => "table-no-spot",
                })
            };
            notice(text, &pal).into_any_element()
        }
    };
    let table_panel = kit::panel(&pal)
        .w(px(left_w))
        .h(px(table_h))
        .child(table_head(screen, scene.as_ref(), &pal))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .child(crate::tour::anchor("table_felt"))
                .child(body),
        )
        .child(actbar(screen, &trainers, scene.as_ref(), &pal, &tp, cx));
    let line_panel = kit::panel(&pal)
        .w(px(left_w))
        .h(px(LINE_H))
        .child(line::head(screen, &pal, cx))
        .child(line::body(screen, scene.as_ref(), left_w - 2., &pal, cx));
    let eval_panel = kit::panel(&pal)
        .child(crate::tour::anchor("table_explanation"))
        .child(eval::panel(
            screen, &trainers, decided_on, graded, insight, wide, &pal, &tp,
        ));

    // A click anywhere else closes an open picker.
    let scrim = screen.cockpit.menu.map(|_| {
        deferred(
            window_layer().position(point(px(0.), px(0.))).child(
                div()
                    .id("menu_scrim")
                    .w(vp.width)
                    .h(vp.height)
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cockpit.menu = None;
                        cx.notify();
                    })),
            ),
        )
        .with_priority(1)
    });
    let pane = if wide {
        pane.h(px(h))
            .child(table_panel.absolute().left(px(EDGE)).top(px(panels_top)))
            .child(
                line_panel
                    .absolute()
                    .left(px(EDGE))
                    .top(px(panels_top + table_h + PANEL_GAP)),
            )
            .child(
                eval_panel
                    .absolute()
                    .left(px(EDGE + left_w + kit::GAP))
                    .top(px(panels_top))
                    .w(px(EVAL_W))
                    .h(px(h - around)),
            )
    } else {
        pane.flex()
            .flex_col()
            .gap(px(PANEL_GAP))
            .pt(px(panels_top))
            .pb(px(EDGE))
            .px(px(EDGE))
            .child(table_panel)
            .child(line_panel)
            .child(eval_panel.w(px(left_w)))
    };
    pane.child(strip(screen, &trainers, wide, &pal, &tp, cx))
        .children(scrim)
        .into_any_element()
}

// The stacked table panel has a range of heights to move in, and the
// smallest window is on the stacked side of the breakpoint.
const _: () = assert!(TABLE_MIN_H < TABLE_NATURAL_H && MIN_PANE_W < WIDE_PANE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_the_actions() {
        assert_eq!(
            action_keys(&[Action::Fold, Action::Call, Action::Raise(3000)]),
            ["F", "C", "R"]
        );
        assert_eq!(
            action_keys(&[
                Action::Check,
                Action::Bet(300),
                Action::Bet(700),
                Action::AllIn
            ]),
            ["C", "1", "2", "A"]
        );
    }
}

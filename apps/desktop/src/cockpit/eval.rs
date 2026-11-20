//! The evaluation panel: the grade of hero's last decision, every action's
//! frequency and EV with hero's hand, and the range hero was in at that
//! point with the reference mix of each hand, hero's equity and the price.
//!
//! Before hero answers a question, the panel shows the same frame without
//! the strategy: what hero holds, the options and the price. In a full
//! hand it keeps the last graded decision until the next one is graded.

use gpui::{Div, FontWeight, SharedString, Stateful, div, prelude::*, px, relative};
use taful_app::dashboard::HandClass;
use taful_app::insight::Insight;
use taful_app::model::{Grade, Trainers};
use taful_app::preflop::{Action, GameState, GradedDecision};

use super::frame::percent;
use super::kit::{self, MONO, STRONG, Tile, label, tile, tracked};
use super::pal::{Pal, TablePal, Tint};
use crate::Ui;
use crate::felt::{action_label, action_short, grade_key, num};
use crate::screens::table::{Mode, TableScreen};
use crate::theme::alpha;

/// Side of the range matrix.
const MATRIX: f32 = 372.;
/// Horizontal padding of the panel's rows.
const PAD: f32 = 14.;

/// What an action is, for its color: a bet or raise, a check or call, a
/// fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Raise,
    Call,
    Fold,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Raise, Kind::Call, Kind::Fold];

    fn of(a: Action) -> Kind {
        match a {
            Action::Fold => Kind::Fold,
            Action::Check | Action::Call => Kind::Call,
            Action::Bet(_) | Action::Raise(_) | Action::AllIn => Kind::Raise,
        }
    }

    fn color(self, tp: &TablePal) -> Tint {
        match self {
            Kind::Raise => tp.raise,
            Kind::Call => tp.call,
            Kind::Fold => tp.fold,
        }
    }

    /// Fluent id of the kind's name among `actions` (a bet is a raise's
    /// kind when nobody bet yet, a check is a call's).
    fn name_key(self, actions: &[Action]) -> &'static str {
        let has = |f: fn(&Action) -> bool| actions.iter().any(f);
        match self {
            // With nothing to call, the aggressive action is a bet (an
            // all-in over a check included).
            Kind::Raise if has(|a| matches!(a, Action::Bet(_) | Action::Check)) => {
                "cockpit-kind-bet"
            }
            Kind::Raise => "cockpit-kind-raise",
            Kind::Call if has(|a| matches!(a, Action::Check)) => "cockpit-kind-check",
            Kind::Call => "cockpit-kind-call",
            Kind::Fold => "cockpit-kind-fold",
        }
    }
}

/// `freqs` (one per action) summed by kind, in [`Kind::ALL`] order.
fn by_kind(actions: &[Action], freqs: &[f32]) -> [f32; 3] {
    let mut sums = [0.0f32; 3];
    for (a, f) in actions.iter().zip(freqs) {
        sums[Kind::ALL
            .iter()
            .position(|k| *k == Kind::of(*a))
            .unwrap_or(0)] += f;
    }
    sums
}

/// The kinds `actions` has, in [`Kind::ALL`] order.
fn kinds(actions: &[Action]) -> Vec<Kind> {
    Kind::ALL
        .into_iter()
        .filter(|k| actions.iter().any(|a| Kind::of(*a) == *k))
        .collect()
}

/// A stacked bar of the kinds' shares, left to right.
fn mix_bar(shares: [f32; 3], tp: &TablePal) -> Div {
    let total: f32 = shares.iter().sum();
    div().flex().overflow_hidden().children(
        Kind::ALL
            .into_iter()
            .zip(shares)
            .filter(|(_, share)| *share > 0.0005 && total > 0.)
            .map(|(k, share)| {
                div()
                    .h_full()
                    .flex_none()
                    .w(relative(share / total))
                    .bg(k.color(tp))
            }),
    )
}

fn swatch(color: Tint) -> Div {
    div()
        .flex_none()
        .size(px(9.))
        .mr(px(8.))
        .rounded(px(2.))
        .bg(color)
}

/// A signed EV difference with a real minus sign.
fn delta(ui: &Ui, x: f64) -> String {
    let s = num(ui, x.abs(), 2);
    if x <= -0.005 { format!("−{s}") } else { s }
}

/// What hero holds, in words.
fn note_text(ui: &Ui, i: &Insight) -> String {
    i.note
        .iter()
        .map(|id| ui.i18n.tr(&format!("cockpit-hand-{id}")))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// A big number with its caption above, right-aligned.
fn stat(caption: String, value: String, unit: Option<String>, pal: &Pal) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .items_end()
        .child(label(&caption, pal))
        .child(
            div()
                .flex()
                .items_end()
                .mt(px(5.))
                .child(
                    tracked(&value, 26., -0.03)
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(26.)),
                )
                .children(unit.map(|u| {
                    div()
                        .ml(px(3.))
                        .font_family(MONO)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.))
                        .line_height(px(14.))
                        .text_color(pal.muted)
                        .child(u)
                })),
        )
}

/// The grade (or that it is hero's turn), hero's hand and the decision's
/// two numbers.
fn verdict(
    ui: &Ui,
    state: Option<&GameState>,
    graded: Option<&GradedDecision>,
    insight: Option<&Insight>,
    pal: &Pal,
    tp: &TablePal,
) -> Div {
    let i18n = &ui.i18n;
    let tag = |text: String| tracked(&text.to_uppercase(), 15., 0.1).font_weight(FontWeight::BOLD);
    let tag = match graded {
        Some(d) => {
            let c = pal.grade(d.grade);
            div()
                .flex_none()
                .py(px(6.))
                .px(px(12.))
                .rounded(px(4.))
                .bg(c)
                .text_color(tp.on_grade)
                .shadow(kit::shadow(alpha(c, 0.3), 0., 22.))
                .child(tag(i18n.tr(&grade_key(d.grade))))
        }
        None => div()
            .flex_none()
            .py(px(5.))
            .px(px(11.))
            .rounded(px(4.))
            .border_1()
            .border_color(pal.amber)
            .bg(alpha(pal.amber, 0.1))
            .text_color(pal.amber)
            .child(tag(i18n.tr("cockpit-pod-turn"))),
    };
    let hand = graded.map(|d| d.hand).or(insight.map(|i| i.hand));
    let title = match (graded, insight) {
        (Some(d), _) => d
            .actions
            .get(d.chosen)
            .map_or_else(String::new, |a| action_name(ui, state, *a)),
        (None, Some(i)) => i18n.tr_args(
            "table-decision-context",
            &[
                ("position", i.position.label().into()),
                ("hand", i.class().to_string().into()),
            ],
        ),
        (None, None) => i18n.tr("cockpit-none"),
    };
    // Nothing dealt: no tag, and the dash is dimmed.
    let idle = graded.is_none() && insight.is_none();
    let none = || i18n.tr("cockpit-none");
    let bb = || Some(i18n.tr("cockpit-unit-bb"));
    let (loss, ev) = match graded {
        Some(d) => (
            stat(
                i18n.tr("cockpit-eval-loss"),
                num(ui, d.ev_loss_bb, 2),
                bb(),
                pal,
            ),
            stat(
                i18n.tr("cockpit-eval-ev"),
                d.hand_evs
                    .get(d.chosen)
                    .map_or_else(none, |e| delta(ui, f64::from(*e))),
                bb(),
                pal,
            ),
        ),
        None => (
            stat(i18n.tr("cockpit-eval-loss"), none(), None, pal).text_color(pal.dim),
            stat(i18n.tr("cockpit-eval-ev"), none(), None, pal).text_color(pal.dim),
        ),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(14.))
        .h(px(92.))
        .px(px(PAD))
        .border_b_1()
        .border_color(pal.line)
        .when(!idle, |d| d.child(tag))
        .children(hand.map(|h| {
            div()
                .flex()
                .flex_none()
                .gap(px(5.))
                .children(h.cards().map(|c| tile(Some(c), Tile::Small, 1., tp)))
        }))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(STRONG)
                        .truncate()
                        .when(idle, |d| d.text_color(pal.dim))
                        .child(title),
                )
                .children(insight.map(|i| {
                    div()
                        .text_size(px(12.))
                        .text_color(pal.muted)
                        .truncate()
                        .child(note_text(ui, i))
                })),
        )
        .child(loss)
        .child(div().w(px(8.)).flex_none())
        .child(ev)
}

/// An action's name: as the action bar words it (`Call 10`) when the state
/// it is taken on is at hand, the short form otherwise.
fn action_name(ui: &Ui, state: Option<&GameState>, a: Action) -> String {
    match state {
        Some(s) => action_label(ui, s, a),
        None => action_short(ui, a),
    }
}

/// Stands for the hand in a string that names it.
const HAND_SLOT: &str = "@";

/// One row per action: frequency with hero's hand, EV and the EV given up.
fn actions_table(
    ui: &Ui,
    t: &Trainers,
    state: Option<&GameState>,
    graded: Option<&GradedDecision>,
    insight: Option<&Insight>,
    pal: &Pal,
    tp: &TablePal,
) -> Div {
    let i18n = &ui.i18n;
    let actions: &[Action] = match (graded, insight) {
        (Some(d), _) => &d.actions,
        (None, Some(i)) => &i.actions,
        (None, None) => &[],
    };
    let head = |text: String| tracked(&text.to_uppercase(), 10., 0.08).text_color(pal.dim);
    // "Frequency with Q♠J♠": the suits are icons, set in the heading where
    // the string names the hand.
    let freq_head = {
        let text = i18n.tr_args("cockpit-col-freq", &[("hand", HAND_SLOT.into())]);
        let (before, after) = text.split_once(HAND_SLOT).unwrap_or((&text, ""));
        let mut row = div().flex().items_center().child(head(before.to_owned()));
        for c in insight.iter().flat_map(|i| i.hand.cards()) {
            row = row
                .child(head(c.rank().to_char().to_string()))
                .child(kit::icon(kit::suit_icon(c.suit()), 8., pal.dim).mr(px(1.)));
        }
        row.child(head(after.to_owned()))
    };
    let ev_key = if graded.is_some_and(|d| d.vs_profile.is_some()) {
        "table-col-ev-profile"
    } else {
        "cockpit-col-ev"
    };
    let name_w = px(156.);
    let num_w = px(82.);
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .pt(px(9.))
        .pb(px(6.))
        .px(px(PAD))
        .border_b_1()
        .border_color(pal.line)
        .font_weight(FontWeight::MEDIUM)
        .child(
            div()
                .w(name_w)
                .flex_none()
                .child(head(i18n.tr("table-col-action"))),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .child(freq_head),
        )
        .child(
            div()
                .w(num_w)
                .flex_none()
                .flex()
                .justify_end()
                .child(head(i18n.tr(ev_key))),
        )
        .child(
            div()
                .w(num_w)
                .flex_none()
                .flex()
                .justify_end()
                .child(head(i18n.tr("cockpit-col-delta"))),
        );
    let grades: Vec<Grade> = graded
        .map(|d| d.grades(t.preflop.thresholds()))
        .unwrap_or_default();
    let best = graded.map(|d| d.hand_evs.iter().copied().fold(f32::NEG_INFINITY, f32::max));
    let number = |text: String| {
        div()
            .w(num_w)
            .flex_none()
            .flex()
            .justify_end()
            .font_family(MONO)
            .child(SharedString::from(text))
    };
    let rows = actions.iter().enumerate().map(|(i, &a)| {
        let color = Kind::of(a).color(tp);
        let chosen = graded.is_some_and(|d| d.chosen == i);
        let mine = graded.filter(|_| chosen).map(|d| pal.grade(d.grade));
        let freq = graded.and_then(|d| d.hand_freqs.get(i).copied());
        let ev = graded.and_then(|d| d.hand_evs.get(i).copied());
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(38.))
            .px(px(PAD))
            .border_b_1()
            .border_color(pal.line)
            .text_size(px(13.))
            .relative()
            .when_some(mine, |d, c| {
                // The grade's color on the row's left edge.
                d.bg(alpha(c, 0.07))
                    .child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(c))
            })
            .child(
                div()
                    .w(name_w)
                    .flex_none()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(swatch(color))
                    .child(SharedString::from(action_name(ui, state, a)))
                    .children(mine.map(|c| {
                        label(&i18n.tr("cockpit-you"), pal)
                            .ml(px(10.))
                            .text_color(c)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .child(
                        div()
                            .w(px(170.))
                            .h(px(10.))
                            .flex_none()
                            .rounded(px(1.))
                            .overflow_hidden()
                            .bg(pal.raised)
                            .children(
                                freq.map(|f| div().h_full().w(relative(f.clamp(0., 1.))).bg(color)),
                            ),
                    )
                    .child(match freq {
                        Some(f) => SharedString::from(format!("{}%", percent(f64::from(f)))),
                        None => i18n.tr("cockpit-none").into(),
                    }),
            )
            .child(match ev {
                Some(e) => number(delta(ui, f64::from(e))),
                None => number(i18n.tr("cockpit-none")).text_color(pal.dim),
            })
            .child(match (ev, best) {
                (Some(e), Some(b)) => number(delta(ui, f64::from(e - b)))
                    .text_color(grades.get(i).map_or(pal.muted, |g| pal.grade(*g))),
                _ => number(i18n.tr("cockpit-none")).text_color(pal.dim),
            })
    });
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(header)
        .children(rows)
}

/// The 13 × 13 matrix: every hand class with the reference mix as its
/// background, hero's class outlined. `mixed` off draws the range without
/// the strategy.
fn matrix(insight: Option<&Insight>, mixed: bool, pal: &Pal, tp: &TablePal) -> Div {
    let cell = (MATRIX - 2. - 12.) / 13.;
    let hero = insight.map(|i| i.class().index());
    let mut grid = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(1.))
        .w(px(MATRIX))
        .h(px(MATRIX))
        .border_1()
        .border_color(pal.line)
        .bg(pal.line);
    for r in 0..13 {
        let mut row = div().flex().flex_1().gap(px(1.));
        for c in 0..13 {
            let class = HandClass::from_grid_pos(r, c);
            let data = insight.and_then(|i| i.cells.get(class.index()).map(|x| (i, x)));
            let inside = data.is_some_and(|(_, x)| x.in_range());
            let mine = hero == Some(class.index());
            let mut d = div()
                .relative()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .h(px(cell))
                .font_family(MONO)
                .text_size(px(9.5))
                .font_weight(if mine {
                    FontWeight::BOLD
                } else {
                    FontWeight::MEDIUM
                });
            d = if inside {
                d.bg(pal.cell).text_color(pal.cell_text)
            } else {
                d.bg(tp.cell_out).text_color(pal.cell_out_text)
            };
            if let (true, true, Some((i, x))) = (mixed, inside, data) {
                d = d.child(
                    mix_bar(by_kind(&i.actions, &x.mix), tp)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                );
            }
            d = d.child(
                div()
                    .relative()
                    .child(SharedString::from(class.to_string())),
            );
            if mine {
                d = d.child(
                    div()
                        .absolute()
                        .top(px(-1.))
                        .left(px(-1.))
                        .right(px(-1.))
                        .bottom(px(-1.))
                        .border_2()
                        .border_color(tp.hero_outline),
                );
            }
            row = row.child(d);
        }
        grid = grid.child(row);
    }
    grid
}

/// Beside the matrix: the whole range by kind of action, hero's hand, and
/// the equity against what it needs.
fn range_side(ui: &Ui, insight: Option<&Insight>, mixed: bool, pal: &Pal, tp: &TablePal) -> Div {
    let i18n = &ui.i18n;
    let none = || i18n.tr("cockpit-none");
    let row = |name: Div, value: Option<String>| {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(30.))
            .border_b_1()
            .border_color(pal.line)
            .text_size(px(12.5))
            .whitespace_nowrap()
            .child(name)
            .child(match value {
                Some(v) => div()
                    .font_family(MONO)
                    .font_weight(FontWeight::BOLD)
                    .child(v),
                None => div().font_family(MONO).text_color(pal.dim).child(none()),
            })
    };
    let pct = |x: f64| format!("{}%", percent(x));
    let mut col = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.))
        .child(label(&i18n.tr("cockpit-range-all"), pal).mb(px(4.)));
    let Some(i) = insight else {
        return col.child(row(div(), None));
    };
    let totals = by_kind(&i.actions, &i.totals);
    let present = kinds(&i.actions);
    for k in &present {
        let at = Kind::ALL.iter().position(|x| x == k).unwrap_or(0);
        col = col.child(row(
            div()
                .flex()
                .items_center()
                .child(swatch(k.color(tp)))
                .child(i18n.tr(k.name_key(&i.actions))),
            mixed.then(|| pct(f64::from(totals[at]))),
        ));
    }
    // Hero's hand: its class and its own mix.
    let cell = i
        .cells
        .get(i.class().index())
        .filter(|c| mixed && c.in_range());
    let mine = cell.map(|c| by_kind(&i.actions, &c.mix));
    let letters = mine.map(|m| {
        present
            .iter()
            .map(|k| {
                let at = Kind::ALL.iter().position(|x| x == k).unwrap_or(0);
                let key = k.name_key(&i.actions);
                // A check is X; the others take their name's initial.
                let letter = if key == "cockpit-kind-check" {
                    "X".to_string()
                } else {
                    let name = i18n.tr(key);
                    name.chars().take(1).collect::<String>().to_uppercase()
                };
                format!("{letter} {}", percent(f64::from(m[at])))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    });
    col = col.child(
        div()
            .flex()
            .flex_col()
            .flex_none()
            .mt(px(14.))
            .p(px(10.))
            .rounded(px(5.))
            .border_1()
            .border_color(pal.line2)
            .bg(pal.panel2)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_family(MONO)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(14.))
                            .child(SharedString::from(i.class().to_string())),
                    )
                    .child(label(&i18n.tr("cockpit-range-yours"), pal)),
            )
            .child(
                match mine {
                    Some(m) => mix_bar(m, tp),
                    None => div().bg(pal.raised),
                }
                .h(px(10.))
                .mt(px(8.))
                .mb(px(6.))
                .rounded(px(1.)),
            )
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(pal.muted)
                    .child(letters.unwrap_or_else(none)),
            ),
    );
    // Equity against the last bettor's range, and what a call needs.
    if i.equity.is_some() || i.pot_odds.is_some() {
        col = col.child(
            label(&i18n.tr("cockpit-equity"), pal)
                .mt(px(14.))
                .mb(px(4.)),
        );
        if let Some(e) = i.equity {
            col = col.child(row(
                div().child(i18n.tr_args(
                    "cockpit-equity-vs",
                    &[("position", e.villain.label().into())],
                )),
                mixed.then(|| pct(e.equity)),
            ));
        }
        if let Some(p) = i.pot_odds {
            col = col.child(row(
                div().child(i18n.tr("cockpit-equity-needs")),
                Some(pct(p)),
            ));
        }
    }
    col
}

/// The lines Classic's panel shows around a decision: who the opponent
/// is, what the library could not cover, and how a hand ended.
fn remarks(screen: &TableScreen, t: &Trainers, graded: Option<&GradedDecision>) -> Vec<String> {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let name = |id: &str| i18n.tr(&format!("bot-profile-{id}"));
    let about = |id: &str| i18n.tr(&format!("bot-profile-{id}-about"));
    let mut lines = Vec::new();
    if let Some(v) = graded.and_then(|d| d.vs_profile.as_ref().map(|v| (d, v))) {
        let (d, v) = v;
        let action = |a: usize| {
            d.actions
                .get(a)
                .map_or_else(|| i18n.tr("cockpit-none"), |x| action_short(ui, *x))
        };
        lines.push(format!(
            "{} · {} · {}",
            i18n.tr_args(
                "table-exploit-line",
                &[("action", action(v.exploit).into())]
            ),
            i18n.tr_args("table-gto-line", &[("action", action(v.gto).into())]),
            i18n.tr_args(
                "table-ev-gap",
                &[("amount", num(ui, v.ev_gap_bb, 2).into())]
            ),
        ));
    }
    if screen.mode == Mode::Hand {
        let Some(h) = &screen.hand else { return lines };
        if let Some(id) = h.table_profile() {
            lines.push(i18n.tr_args(
                "table-hand-vs-profile",
                &[("profile", name(id).into()), ("about", about(id).into())],
            ));
        } else if h.is_mixed() {
            lines.push(i18n.tr("table-hand-vs-mixed"));
        }
        if let Some(id) = &h.fallback
            && h.walk().is_some()
        {
            lines.push(i18n.tr_args(
                "table-hand-profile-fallback",
                &[("profile", name(id).into())],
            ));
        }
        // The ending stays unsaid while it is still being replayed.
        let over = h.end.is_some() && !screen.playing();
        let next = screen.table_bots(t);
        if !over && next != h.bots() {
            let first = next.first().cloned().flatten();
            lines.push(if next.iter().all(|b| *b == first) {
                i18n.tr_args(
                    "table-hand-next-table",
                    &[(
                        "profile",
                        name(first.as_deref().unwrap_or("reference")).into(),
                    )],
                )
            } else {
                i18n.tr("table-hand-next-mixed")
            });
        }
        if h.walk().is_some_and(|w| w.on_solved_river()) && !over {
            lines.push(i18n.tr("table-river-solved"));
        }
        if over {
            lines.push(i18n.tr_args(
                "table-hand-ev-loss",
                &[("amount", num(ui, h.total_ev_loss_bb(), 2).into())],
            ));
        }
        return lines;
    }
    if let (Some(q), None) = (&screen.question, graded) {
        let (opponent, fallback) = match q {
            taful_app::model::DrillSpot::Preflop(s) => (s.opponent.as_deref(), None),
            taful_app::model::DrillSpot::Postflop(s) => {
                (s.opponent.as_deref(), s.fallback.as_deref())
            }
        };
        if let Some(id) = fallback {
            lines.push(i18n.tr_args("table-profile-fallback", &[("profile", name(id).into())]));
        }
        if let Some(id) = opponent {
            lines.push(if taful_app::mixed::is_table_key(id) {
                i18n.tr_args(
                    "table-vs-table-prompt",
                    &[("table", i18n.opponent(id).into())],
                )
            } else {
                i18n.tr_args(
                    "table-vs-profile-prompt",
                    &[("profile", name(id).into()), ("about", about(id).into())],
                )
            });
        }
    }
    lines
}

/// The whole panel. `scrolls` fills the parent and scrolls inside it; off,
/// the panel is as tall as what it shows.
#[allow(clippy::too_many_arguments)]
pub(super) fn panel(
    screen: &TableScreen,
    t: &Trainers,
    state: Option<&GameState>,
    graded: Option<&GradedDecision>,
    insight: Option<&Insight>,
    scrolls: bool,
    pal: &Pal,
    tp: &TablePal,
) -> Stateful<Div> {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let against = match graded.and_then(|d| d.vs_profile.as_ref()) {
        Some(v) => i18n.tr_args(
            "leak-vs-profile",
            &[("profile", i18n.opponent(&v.profile).into())],
        ),
        None => i18n.tr("cockpit-eval-vs-reference"),
    };
    let mixed = graded.is_some();
    let remarks = remarks(screen, t, graded);
    let range_title = match insight {
        Some(i) => i18n.tr_args(
            "cockpit-range-title",
            &[("position", i.position.label().into())],
        ),
        None => i18n.tr("cockpit-range-title-none"),
    };
    div()
        .id("cockpit_eval")
        .flex()
        .flex_col()
        .map(|d| {
            if scrolls {
                d.size_full().overflow_y_scroll()
            } else {
                d.w_full().flex_none()
            }
        })
        .child(kit::head(
            &i18n.tr("cockpit-eval-title"),
            Some(div().child(against).into_any_element()),
            true,
            pal,
        ))
        .child(verdict(ui, state, graded, insight, pal, tp))
        .child(actions_table(ui, t, state, graded, insight, pal, tp))
        .when(!remarks.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(4.))
                    .py(px(9.))
                    .px(px(PAD))
                    .border_b_1()
                    .border_color(pal.line)
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(pal.muted)
                    .children(remarks),
            )
        })
        .child(kit::head(
            &range_title,
            Some(
                div()
                    .child(i18n.tr("cockpit-range-note"))
                    .into_any_element(),
            ),
            true,
            pal,
        ))
        .child(
            div()
                .flex()
                .flex_none()
                .gap(px(16.))
                .py(px(12.))
                .px(px(PAD))
                .child(matrix(insight, mixed, pal, tp))
                .child(range_side(ui, insight, mixed, pal, tp)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequencies_add_up_by_kind() {
        let actions = [
            Action::Check,
            Action::Bet(300),
            Action::Bet(700),
            Action::AllIn,
        ];
        assert_eq!(by_kind(&actions, &[0.5, 0.2, 0.2, 0.1]), [0.5, 0.5, 0.]);
        assert_eq!(kinds(&actions), [Kind::Raise, Kind::Call]);
        assert_eq!(Kind::Raise.name_key(&actions), "cockpit-kind-bet");
        assert_eq!(Kind::Call.name_key(&actions), "cockpit-kind-check");
    }
}

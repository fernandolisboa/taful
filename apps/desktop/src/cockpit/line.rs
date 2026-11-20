//! The hand line: every street side by side with its actions as chips, and
//! the replay's controls. A click on a chip shows the table at that moment.

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px};
use taful_app::model::{Grade, Street};
use taful_app::template::street_id;

use super::kit::{self, Btn, MONO, Tile, btn, label, tile};
use super::pal::Pal;
use super::table::{Ev, Scene};
use crate::felt::bb;
use crate::screens::table::TableScreen;

/// Share of the line's width each street takes.
const COLUMNS: [(Street, f32); 4] = [
    (Street::Preflop, 1.5),
    (Street::Flop, 1.0),
    (Street::Turn, 0.7),
    (Street::River, 0.7),
];

/// Where a step button of the replay goes.
#[derive(Clone, Copy)]
enum Step {
    /// A state of the replay.
    To(usize),
    /// From the answer played to the table it was made on, or back.
    Back(bool),
}

/// The panel's header: its title and the replay's controls.
pub(super) fn head(screen: &TableScreen, pal: &Pal, cx: &mut Context<TableScreen>) -> Div {
    let i18n = &screen.ui.i18n;
    let (at, last) = screen
        .playback
        .as_ref()
        .map_or((0, 0), |p| (p.at, p.last()));
    // After an answer the line has one more step than the replay: the
    // answer played. `‹` first steps back to the table hero decided on.
    let answered = super::table::answered(screen);
    let back = screen.cockpit.back;
    let (prev, next) = if answered && !back {
        (Some(Step::Back(true)), None)
    } else if answered {
        (last.checked_sub(1).map(Step::To), Some(Step::Back(false)))
    } else {
        (
            at.checked_sub(1).map(Step::To),
            (at < last).then_some(Step::To(at + 1)),
        )
    };
    let step = |id: &'static str, text: &'static str, to: Option<Step>| {
        let b = btn(id, text, None, Btn::Small, pal);
        match to {
            Some(to) => b.on_click(cx.listener(move |this, _, _, cx| match to {
                Step::To(at) => {
                    this.cockpit.back = false;
                    this.jump_to(at, cx);
                }
                Step::Back(back) => {
                    this.cockpit.back = back;
                    cx.notify();
                }
            })),
            None => b.text_color(pal.dim).cursor_default(),
        }
    };
    let auto = screen.auto_replay;
    let controls = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(step("line_prev", "‹", prev))
        .child(step("line_next", "›", next))
        .child(
            btn(
                "line_auto",
                i18n.tr("table-replay-auto"),
                None,
                Btn::Small,
                pal,
            )
            // Off reads as a dimmed button.
            .when(!auto, |b| b.text_color(pal.dim))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_replay(cx))),
        );
    kit::head(
        &i18n.tr("cockpit-line-title"),
        Some(controls.into_any_element()),
        true,
        pal,
    )
}

/// One action (or the folds before the first entry) as a chip.
fn chip(who: &str, what: &str, hero: bool, grade: Option<Grade>, current: bool, pal: &Pal) -> Div {
    let mark = match grade {
        Some(Grade::Best | Grade::Correct) => " ✓",
        _ => "",
    };
    let graded = grade.map(|g| pal.grade(g));
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .py(px(2.))
        .px(px(7.))
        .rounded(px(4.))
        .border_1()
        .border_color(match graded {
            Some(c) => c,
            None if hero => pal.amber_dim,
            None => pal.line2,
        })
        .bg(if current { pal.raised } else { pal.panel2 })
        .font_family(MONO)
        .text_size(px(11.))
        .line_height(px(15.))
        .whitespace_nowrap()
        .text_color(graded.unwrap_or(pal.muted))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(match graded {
                    Some(c) => c,
                    None if hero => pal.amber,
                    None => pal.text,
                })
                .child(SharedString::from(who.to_owned())),
        )
        .child(SharedString::from(format!("{what}{mark}")))
}

/// The streets, each with its board cards, the pot it started with and its
/// actions so far.
pub(super) fn body(
    screen: &TableScreen,
    scene: Option<&Scene<'_>>,
    width: f32,
    pal: &Pal,
    cx: &mut Context<TableScreen>,
) -> Div {
    let (ui, i18n) = (&screen.ui, &screen.ui.i18n);
    let total: f32 = COLUMNS.iter().map(|(_, w)| w).sum();
    let last_step = screen.playback.as_ref().map_or(0, |p| p.last());
    let mut row = div().flex().flex_1().min_h(px(0.));
    for (i, (street, share)) in COLUMNS.into_iter().enumerate() {
        let name = i18n.tr(&format!("street-{}", street_id(street)));
        let dealt = scene.and_then(|sc| {
            sc.streets
                .iter()
                .find(|d| d.street == street && (!sc.playing || d.step <= sc.at))
                .map(|d| (sc, d))
        });
        let mut col = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px((width * share / total).floor()))
            .h_full()
            .py(px(10.))
            .px(px(12.))
            .overflow_hidden()
            .when(i + 1 < COLUMNS.len(), |d| {
                d.border_r_1().border_color(pal.line)
            });
        let Some((sc, d)) = dealt else {
            row = row.child(
                col.child(
                    div()
                        .flex()
                        .items_center()
                        .mb(px(9.))
                        .child(div().flex_1())
                        .child(label(&name, pal).text_color(pal.dim)),
                )
                .child(
                    div()
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(pal.dim)
                        .child(i18n.tr("cockpit-none")),
                ),
            );
            continue;
        };
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .mb(px(9.))
                .child(
                    label(&name, pal)
                        .text_color(pal.text)
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .children(
                    d.cards
                        .iter()
                        .map(|&c| tile(Some(c), Tile::Tiny, 1., &sc.tp)),
                )
                .child(div().flex_1())
                .child(
                    label(
                        &i18n.tr_args("table-tl-pot", &[("amount", bb(ui, d.pot).into())]),
                        pal,
                    )
                    .text_color(pal.text)
                    .font_weight(FontWeight::SEMIBOLD),
                ),
        );
        // The folds before the first entry share one chip.
        let mut chips: Vec<(String, &Ev)> = Vec::new();
        for e in sc.evs.iter().filter(|e| e.street == street) {
            if sc.playing && e.step > sc.at {
                break;
            }
            match chips.last_mut() {
                Some((who, prev)) if e.opening_fold && prev.opening_fold => {
                    who.push(' ');
                    who.push_str(&e.who);
                    *prev = e;
                }
                _ => chips.push((e.who.clone(), e)),
            }
        }
        col = col.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(5.))
                .children(chips.into_iter().map(|(who, e)| {
                    let step = e.step.min(last_step);
                    let current = sc.playing && e.step == sc.at;
                    div()
                        .id(("line_chip", e.step))
                        .flex_none()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.cockpit.back = false;
                            this.jump_to(step, cx);
                        }))
                        .child(chip(&who, &e.what, e.hero, e.grade, current, pal))
                })),
        );
        row = row.child(col);
    }
    row
}

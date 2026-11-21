//! Cockpit's lessons: a summary strip (overall progress, time left, quiz
//! average, the next lesson) over one column per level or theme ("Ver por"
//! sits in the status bar).

use gpui::{
    AnyElement, Context, Div, FontWeight, div, linear_color_stop, linear_gradient, prelude::*, px,
    relative,
};
use taful_app::lessons::{GroupKind, LessonCard, LessonGroup, TestState, ViewBy};
use taful_app::model::LessonState;

use super::kit::{self, Btn, GAP, MONO, STRONG, btn, figure, head, icon, label, mono};
use super::pal::Pal;
use super::state::Lessons;
use super::{Go, go};
use crate::Ui;
use crate::screen::Screen;
use crate::shell::Shell;
use crate::study::{GearLook, on_test_click};

/// How the level question (D25) is dressed: a Cockpit panel, mono labels,
/// and each level's key cap (1–6 pick it from the keyboard).
pub fn level_look(shell: &Shell) -> crate::level_question::Look {
    let pal = Pal::new(&shell.ui);
    crate::level_question::Look {
        scrim: crate::theme::alpha(pal.rail_bg, 0.8),
        card: |s, content| {
            let pal = Pal::new(&s.ui);
            kit::panel(&pal)
                .child(content)
                .border_color(pal.line2)
                .shadow(kit::shadow(crate::theme::alpha(pal.rail_bg, 0.9), 18., 40.))
                .text_color(pal.text)
        },
        pad: 24.,
        eyebrow: |s, text| {
            let pal = Pal::new(&s.ui);
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .size(px(6.))
                        .mr(px(8.))
                        .rounded(px(1.))
                        .bg(pal.amber),
                )
                .child(
                    kit::tracked(&text.to_uppercase(), 10.5, 0.09)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(pal.amber),
                )
        },
        title: |_, text| {
            div()
                .text_size(px(24.))
                .line_height(px(30.))
                .font_weight(STRONG)
                .child(text)
        },
        text: |s, text| {
            div()
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(Pal::new(&s.ui).muted)
                .child(text)
        },
        name: |_, text| {
            div()
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        },
        about: |s, text| {
            mono(text)
                .text_size(px(11.))
                .text_color(Pal::new(&s.ui).muted)
        },
        tag: |s, text, strong| {
            let pal = Pal::new(&s.ui);
            kit::tracked(&text.to_uppercase(), 9.5, 0.08)
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .border_1()
                .border_color(if strong { pal.amber } else { pal.line2 })
                .text_color(if strong { pal.on_amber } else { pal.dim })
                .when(strong, |d| d.bg(pal.amber))
        },
        mark: |s, n, place| {
            let (pal, soon) = (Pal::new(&s.ui), place.is_none());
            kit::kbd(&(n + 1).to_string(), &pal)
                .w(px(22.))
                .flex()
                .justify_center()
                .when(!soon, |d| d.text_color(pal.amber))
                .into_any_element()
        },
        link: (pal.muted, pal.amber),
        row: crate::level_question::RowLook {
            radius: 5.,
            ring: pal.line,
            fill: pal.panel2,
            ring_hot: pal.amber_dim,
            fill_hot: pal.raised,
            ring_on: pal.amber,
            fill_on: pal.amber_at(0.08),
        },
    }
}

/// The least the next lesson and the progress cells get in a narrow window.
const NEXT_MIN: f32 = 380.;
const PROGRESS_MIN: f32 = 240.;
/// The three cells of fixed width: done, time left and the quiz average.
const FIXED_CELLS: f32 = 150. + 170. + 150.;
/// Below this the summary cannot hold the next lesson, which gets a strip
/// of its own.
const SUMMARY_MIN: f32 = 2. + PROGRESS_MIN + FIXED_CELLS + NEXT_MIN;
const SUMMARY_H: f32 = 88.;
const NEXT_H: f32 = 72.;
/// The narrowest a track's column gets: what the design gives it at 1280.
const COLUMN_MIN: f32 = 224.;
const ROW_H: f32 = 54.;
/// A level without lessons yet ("em breve"), when the columns share a row.
const SOON_W: f32 = 190.;

/// The lessons tree's body.
pub fn lessons(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let ui = shell.ui.clone();
    let pal = Pal::new(&ui);
    let (w, _) = shell.cockpit.room;
    let Some(data) = shell.cockpit.lessons() else {
        return div()
            .text_color(pal.muted)
            .child(ui.i18n.tr("cockpit-kpi-empty"))
            .into_any_element();
    };
    let groups = &data.summary.list.groups;
    let next = (
        data.summary.next.as_ref().map(|n| n.id.as_str()),
        data.summary.list.next_test.as_deref(),
    );
    let first = groups.iter().position(|g| !g.is_soon());
    // Levels without lessons yet ("em breve") take narrow columns when
    // everything fits on one row; otherwise the groups with lessons wrap
    // into even rows and the empty ones follow on a short row of their own.
    let (real, soon): (Vec<_>, Vec<_>) = groups.iter().enumerate().partition(|(_, g)| !g.is_soon());
    let one_row = real.len() as f32 * COLUMN_MIN
        + soon.len() as f32 * SOON_W
        + groups.len().saturating_sub(1) as f32 * GAP
        <= w;
    let row = |items: &[(usize, &LessonGroup)], col_w: f32, cx: &mut Context<Shell>| {
        let mut r = div().flex().flex_none().items_start().gap(px(GAP));
        for (k, g) in items {
            r = r.child(column(
                &ui,
                &pal,
                g,
                (*k, first == Some(*k)),
                next,
                col_w,
                cx,
            ));
        }
        r
    };
    let columns = if one_row && !real.is_empty() {
        let full_w =
            (w - (groups.len() - 1) as f32 * GAP - soon.len() as f32 * SOON_W) / real.len() as f32;
        let mut r = div().flex().flex_none().items_start().gap(px(GAP));
        for (k, g) in groups.iter().enumerate() {
            let cw = if g.is_soon() { SOON_W } else { full_w };
            r = r.child(column(&ui, &pal, g, (k, first == Some(k)), next, cw, cx));
        }
        r
    } else {
        let mut block = div().flex().flex_col().flex_none().gap(px(GAP));
        for items in [&real, &soon] {
            if items.is_empty() {
                continue;
            }
            let n = items.len();
            let per = per_row(w, n);
            let col_w = ((w - (per - 1) as f32 * GAP) / per as f32).floor();
            block = block.child(row(items, col_w, cx).when(per < n, |d| d.flex_wrap().w(px(w))));
        }
        block
    };
    let page = div().flex().flex_col().gap(px(GAP)).w(px(w));
    if w < SUMMARY_MIN {
        return page
            .child(summary(&ui, &pal, &data, w, false, cx))
            .child(next_strip(&ui, &pal, &data, w, cx))
            .child(columns)
            .into_any_element();
    }
    page.child(summary(&ui, &pal, &data, w, true, cx))
        .child(columns)
        .into_any_element()
}

/// How many of `n` tracks' columns go on a row `w` pixels wide: all of
/// them as designed; a narrower window wraps them into even rows, so a
/// column keeps at least the width the design gives it at 1280.
fn per_row(w: f32, n: usize) -> usize {
    let fit = (((w + GAP) / (COLUMN_MIN + GAP)).floor() as usize).clamp(1, n);
    n.div_ceil(n.div_ceil(fit))
}

/// The next lesson with its button, or that none is left.
fn next_lesson(ui: &Ui, pal: &Pal, data: &Lessons, meta: bool, cx: &mut Context<Shell>) -> Div {
    let i18n = &ui.i18n;
    let next = match &data.summary.next {
        Some(n) => div()
            .child(
                div()
                    .flex_shrink()
                    .min_w(px(0.))
                    .child(
                        label(&i18n.tr("cockpit-next-lesson"), pal)
                            .text_color(pal.amber)
                            .whitespace_nowrap(),
                    )
                    .child(
                        div()
                            .mt(px(3.))
                            .text_size(px(19.))
                            .font_weight(STRONG)
                            .truncate()
                            .child(n.title.clone()),
                    ),
            )
            .when(meta, |d| {
                d.child(
                    div()
                        .flex_shrink()
                        .min_w(px(0.))
                        .mt(px(16.))
                        .text_size(px(12.))
                        .text_color(pal.muted)
                        .truncate()
                        .child(super::home::next_meta(ui, n)),
                )
            })
            .child(div().flex_1())
            .child(
                btn(
                    "cockpit_continue",
                    i18n.tr("cockpit-continue"),
                    Some("↵"),
                    Btn::Amber,
                    pal,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    go(this, Go::NextLesson, window, cx);
                })),
            ),
        None => match &data.summary.list.next_test {
            // Prova with nothing left to read: the test that opens what
            // follows.
            Some(level) => div()
                .child(
                    div()
                        .flex_shrink()
                        .min_w(px(0.))
                        .child(
                            label(&i18n.tr("tree-test"), pal)
                                .text_color(pal.amber)
                                .whitespace_nowrap(),
                        )
                        .child(
                            div()
                                .mt(px(3.))
                                .text_size(px(13.))
                                .line_height(relative(1.3))
                                .child(crate::study::next_test_text(ui, level)),
                        ),
                )
                .child(div().flex_1())
                .child(
                    btn(
                        "cockpit_next_test",
                        i18n.tr("tree-test-take"),
                        None,
                        Btn::Amber,
                        pal,
                    )
                    .on_click(on_test_click(level.clone(), cx)),
                ),
            None => div()
                .text_color(pal.muted)
                .child(i18n.tr("home-lesson-none")),
        },
    };
    next.flex()
        .flex_1()
        .min_w(px(0.))
        .items_center()
        .gap(px(16.))
        .h_full()
        .px(px(16.))
        .overflow_hidden()
        .bg(linear_gradient(
            90.,
            linear_color_stop(pal.amber_at(0.09), 0.),
            linear_color_stop(pal.amber_at(0.), 0.8),
        ))
}

/// The next lesson as a strip of its own, where the summary has no room
/// for it.
fn next_strip(ui: &Ui, pal: &Pal, data: &Lessons, w: f32, cx: &mut Context<Shell>) -> Div {
    kit::panel(pal)
        .flex_row()
        .w(px(w))
        .h(px(NEXT_H))
        .child(next_lesson(ui, pal, data, true, cx))
}

/// The summary strip. `with_next` off leaves the next lesson out, and the
/// progress cell takes its room.
fn summary(
    ui: &Ui,
    pal: &Pal,
    data: &Lessons,
    w: f32,
    with_next: bool,
    cx: &mut Context<Shell>,
) -> Div {
    let (i18n, s) = (&ui.i18n, &data.summary);
    let cell = |width: f32| {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .justify_center()
            .w(px(width))
            .h_full()
            .px(px(16.))
            .py(px(12.))
            .border_r_1()
            .border_color(pal.line)
    };
    let value = |v: Div| v.mt(px(8.));
    // The fixed cells leave the next lesson what is left of the strip. Below
    // the designed width the progress cell gives up room first.
    let progress_w = if with_next {
        (w - 2. - FIXED_CELLS - NEXT_MIN).clamp(PROGRESS_MIN, 520.)
    } else {
        w - 2. - FIXED_CELLS
    };
    let next_w = w - 2. - progress_w - FIXED_CELLS;
    let narrow = progress_w < 520.;
    // Every lesson as a block, one group per track.
    let all = div().flex().gap(px(8.)).mt(px(10.)).children(
        data.summary
            .list
            .groups
            .iter()
            .filter(|t| !t.lessons.is_empty())
            .map(|t| {
                div()
                    .flex()
                    .gap(px(2.))
                    .children(
                        kit::steps(&t.lessons, s.next.as_ref().map(|n| n.id.as_str()), pal)
                            .into_iter()
                            .map(|b| {
                                let b = b.h(px(12.)).rounded(px(1.5));
                                if narrow {
                                    b.flex_1().max_w(px(10.))
                                } else {
                                    b.flex_none().w(px(10.))
                                }
                            }),
                    )
                    .when(narrow, |d| d.flex_1().min_w(px(0.)))
            }),
    );
    let (hours, minutes) = (s.minutes_left / 60, s.minutes_left % 60);
    let time = if hours > 0 {
        format!("{hours} {} {minutes}", i18n.tr("cockpit-unit-hours"))
    } else {
        minutes.to_string()
    };
    let quiz = match s.quiz_mean {
        Some(q) => figure(
            &super::frame::percent(q),
            Some(i18n.tr("cockpit-unit-percent")),
            24.,
            pal,
        ),
        None => figure(&i18n.tr("cockpit-none"), None, 24., pal).text_color(pal.muted),
    };
    kit::panel(pal)
        .flex_row()
        .w(px(w))
        .h(px(SUMMARY_H))
        .child(
            cell(progress_w)
                .child(label(&i18n.tr("cockpit-lessons-progress"), pal))
                .child(all),
        )
        .child(
            cell(150.)
                .child(label(&i18n.tr("cockpit-lessons-done"), pal))
                .child(value(figure(
                    &s.done.to_string(),
                    Some(i18n.tr_args("cockpit-lessons-of", &[("total", s.total.into())])),
                    24.,
                    pal,
                ))),
        )
        .child(
            cell(170.)
                .child(label(&i18n.tr("cockpit-lessons-time-left"), pal))
                .child(value(figure(
                    &time,
                    Some(i18n.tr("cockpit-unit-minutes")),
                    24.,
                    pal,
                ))),
        )
        .child(
            cell(150.)
                .when(!with_next, |d| d.border_r_0())
                .child(label(&i18n.tr("cockpit-lessons-quiz"), pal))
                .child(value(quiz)),
        )
        .when(with_next, |d| {
            // The detail line goes first when the strip is narrow.
            d.child(next_lesson(ui, pal, data, next_w > 440., cx))
        })
}

/// One group: its lessons in order, each with its state. `first` marks
/// the first group with lessons (the tour points at it).
fn column(
    ui: &Ui,
    pal: &Pal,
    track: &LessonGroup,
    (k, first): (usize, bool),
    (next, next_test): (Option<&str>, Option<&str>),
    w: f32,
    cx: &mut Context<Shell>,
) -> Div {
    let i18n = &ui.i18n;
    let total = track.lessons.len();
    let done = track
        .lessons
        .iter()
        .filter(|l| l.state == LessonState::Done)
        .count();
    let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
    let mut col = kit::panel(pal)
        .relative()
        .w(px(w))
        .when(first, |d| d.child(crate::tour::anchor("tree_first")))
        .child(head(
            &i18n.tr(&track.title_key()),
            (!track.is_soon()).then(|| mono(format!("{done}/{total}")).into_any_element()),
            open,
            pal,
        ))
        .child(
            div().flex_none().h(px(3.)).bg(pal.track).child(
                div()
                    .h_full()
                    .w(relative(done as f32 / total.max(1) as f32))
                    .bg(pal.teal),
            ),
        );
    if track.is_soon() {
        // A level without lessons yet.
        let about = (track.kind == GroupKind::Level)
            .then(|| i18n.tr(&format!("{}-about", track.title_key())));
        col = col.child(
            div()
                .flex()
                .flex_col()
                .justify_center()
                .h(px(ROW_H))
                .px(px(12.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(pal.locked_text)
                        .child(i18n.tr("tree-soon")),
                )
                .children(about.map(|a| {
                    div()
                        .mt(px(2.))
                        .font_family(MONO)
                        .text_size(px(10.))
                        .text_color(pal.dim)
                        .truncate()
                        .child(a)
                })),
        );
    }
    let test = crate::study::test_line(ui, track, next_test);
    let closed = crate::study::opens_with_text(ui, track);
    for (i, lesson) in track.lessons.iter().enumerate() {
        let is_next = next == Some(lesson.id.as_str());
        col = col.child(
            row(ui, pal, lesson, (k, i), is_next, cx).when(i + 1 < total || test.is_some(), |d| {
                d.border_b_1().border_color(pal.line)
            }),
        );
    }
    if let Some(t) = test {
        col = col.child(test_row(ui, pal, track, t, closed, k, cx));
    }
    col
}

/// A level's test, under its lessons: the button, its check once passed,
/// or the lock while the level is closed.
fn test_row(
    ui: &Ui,
    pal: &Pal,
    track: &LessonGroup,
    t: crate::study::TestLine,
    closed: Option<gpui::SharedString>,
    k: usize,
    cx: &mut Context<Shell>,
) -> Div {
    let i18n = &ui.i18n;
    let side = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .font_family(MONO)
        .text_size(px(11.));
    let side = match t.state {
        TestState::Passed => side
            .text_color(pal.teal)
            .child(icon("check", 12., pal.teal))
            .child(
                btn(
                    ("cockpit_test", k),
                    i18n.tr("tree-test-retake"),
                    None,
                    Btn::Small,
                    pal,
                )
                .on_click(on_test_click(track.id.clone(), cx)),
            ),
        TestState::Locked => side.child(icon("lock", 12., pal.dim)),
        TestState::Open => side.child(
            btn(
                ("cockpit_test", k),
                i18n.tr("tree-test-take"),
                None,
                if t.suggested {
                    Btn::SmallAmber
                } else {
                    Btn::Small
                },
                pal,
            )
            .on_click(on_test_click(track.id.clone(), cx)),
        ),
    };
    let detail = match (t.state, closed) {
        (TestState::Locked, Some(c)) => c.to_string(),
        (TestState::Passed, _) => t.action,
        _ => t.meta,
    };
    let locked = t.state == TestState::Locked;
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(ROW_H))
        .px(px(12.))
        .bg(pal.panel2)
        .when(t.suggested, |d| {
            d.bg(linear_gradient(
                90.,
                linear_color_stop(pal.amber_at(0.13), 0.),
                linear_color_stop(pal.amber_at(0.03), 1.),
            ))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.))
                    .bg(pal.amber),
            )
        })
        .child(
            div()
                .flex_none()
                .w(px(26.))
                .font_family(MONO)
                .text_size(px(11.))
                .text_color(if t.suggested { pal.amber } else { pal.dim })
                .child("✦"),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .line_height(relative(1.25))
                        .truncate()
                        .when(locked, |d| d.text_color(pal.locked_text))
                        .child(i18n.tr("tree-test")),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .font_family(MONO)
                        .text_size(px(10.))
                        .text_color(pal.dim)
                        .truncate()
                        .child(detail),
                ),
        )
        .child(side)
}

fn row(
    ui: &Ui,
    pal: &Pal,
    lesson: &LessonCard,
    (k, i): (usize, usize),
    is_next: bool,
    cx: &mut Context<Shell>,
) -> gpui::Stateful<Div> {
    let i18n = &ui.i18n;
    let locked = lesson.state == LessonState::Locked;
    let minutes = lesson.minutes.map(|m| {
        let key = if is_next {
            "cockpit-lesson-minutes-next"
        } else {
            "cockpit-lesson-minutes"
        };
        i18n.tr_args(key, &[("minutes", m.into())])
    });
    let state = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .font_family(MONO)
        .text_size(px(11.));
    let state = match lesson.state {
        LessonState::Done => state
            .text_color(pal.teal)
            .child(icon("check", 12., pal.teal))
            .children(lesson.best_score.map(|s| {
                i18n.tr_args(
                    "cockpit-percent",
                    &[("value", super::frame::percent(s).into())],
                )
            })),
        LessonState::Locked => state.child(icon("lock", 12., pal.dim)),
        _ => state.child(btn(
            ("cockpit_open", k * 100 + i),
            i18n.tr("cockpit-lesson-open"),
            None,
            if is_next { Btn::SmallAmber } else { Btn::Small },
            pal,
        )),
    };
    let id = lesson.id.clone();
    let hover = pal.panel2;
    div()
        .id(("cockpit_lesson", k * 100 + i))
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(ROW_H))
        .px(px(12.))
        .when(is_next, |d| {
            d.bg(linear_gradient(
                90.,
                linear_color_stop(pal.amber_at(0.13), 0.),
                linear_color_stop(pal.amber_at(0.03), 1.),
            ))
            .child(crate::tour::anchor("tree_next"))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.))
                    .bg(pal.amber),
            )
        })
        .when(!locked, |d| {
            d.cursor_pointer()
                .when(!is_next, |d| d.hover(move |s| s.bg(hover)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_lesson(&id);
                    this.select(Screen::Lesson, window, cx);
                }))
        })
        .child(
            div()
                .flex_none()
                .w(px(26.))
                .font_family(MONO)
                .text_size(px(11.))
                .text_color(if is_next { pal.amber } else { pal.dim })
                .child(format!("{:02}", i + 1)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .line_height(relative(1.25))
                        .truncate()
                        .when(locked, |d| d.text_color(pal.locked_text))
                        .child(lesson.title.clone()),
                )
                .children(minutes.map(|m| {
                    div()
                        .mt(px(2.))
                        .font_family(MONO)
                        .text_size(px(10.))
                        .text_color(pal.dim)
                        .child(m)
                })),
        )
        .child(state)
}

/// "Ver por" for the status bar: a label and one small toggle per view.
pub(super) fn view_switch(shell: &mut Shell, pal: &Pal, cx: &mut Context<Shell>) -> Div {
    let current = shell.view_by();
    let ui = shell.ui.clone();
    let i18n = &ui.i18n;
    let item = |view: ViewBy| {
        let on = view == current;
        div()
            .id(("cockpit_view", view as usize))
            .flex()
            .items_center()
            .h_full()
            .px(px(10.))
            .rounded(px(3.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .when(on, |d| d.bg(pal.raised).text_color(pal.amber))
            .when(!on, |d| {
                let hover = pal.text;
                d.text_color(pal.muted).hover(move |s| s.text_color(hover))
            })
            .child(i18n.tr(view.label_key()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_view_by(view);
                cx.notify();
            }))
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .ml(px(10.))
        .child(label(&i18n.tr("tree-view-by"), pal))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(26.))
                .p(px(2.))
                .gap(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(pal.line2)
                .children(ViewBy::ALL.into_iter().map(item)),
        )
        .child(shell.study_gear(
            GearLook {
                size: 26.,
                radius: 4.,
                icon: pal.muted,
                icon_on: pal.amber,
                ring: pal.line2,
                fill_on: pal.raised,
            },
            cx,
        ))
}

// The summary's one-row arrangement fits the width it was designed for.
const _: () = assert!(SUMMARY_MIN <= super::frame::WIDE_ROOM);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_wrap_into_even_rows() {
        // Five tracks: one row from the 1280 px design up.
        assert_eq!(per_row(super::super::frame::WIDE_ROOM, 5), 5);
        // Six themes wrap into two even rows at 1280, not five and one.
        assert_eq!(per_row(super::super::frame::WIDE_ROOM, 6), 3);
        assert_eq!(per_row(1484., 5), 5);
        // The smallest window: three and two, not four and one.
        let least = crate::MIN_WINDOW.0 - super::super::frame::RAIL - 2. * kit::EDGE;
        assert_eq!(per_row(least, 5), 3);
        assert_eq!(per_row(100., 5), 1);
        // The columns of a row never ask for more than the row.
        for w in [least, 900., 1000., 1100., 1175.] {
            let k = per_row(w, 5) as f32;
            assert!(k * COLUMN_MIN + (k - 1.) * GAP <= w, "{w}");
        }
    }
}

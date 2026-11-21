//! Lessons: one row per level (or theme), one playing card per lesson. A finished
//! lesson is a face-up card with a check, the next one is the wide card
//! with the button, and locked lessons lie face down.

use gpui::{AnyElement, Context, Div, div, prelude::*};
use taful_app::lessons::{GroupKind, LessonCard, ViewBy};
use taful_app::model::LessonState;

use super::chrome::{MARGIN, NAV_H, heading};
use super::kit::{self, Fill, Pip, Raised};
use super::paint::{self, Tex};
use super::{Bx, CANVAS_H, CANVAS_W, Tone, bx, typo, u};
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::alpha;

const ROW_TOP: f32 = 170.;
const ROW_STEP: f32 = 143.;
const ROW_H: f32 = 124.;
const LABEL_W: f32 = 262.;
/// Room kept at the right of the cards.
const RIGHT: f32 = 48.;
const CARD_W: f32 = 90.;
const CARD_H: f32 = 118.;
const NEXT_W: f32 = 208.;
const NEXT_H: f32 = 132.;
/// The wide card's side margin, and the gap between cards.
const NEXT_MARGIN: f32 = 6.;
const GAP: f32 = 9.;

/// The corner index of a face-up card: number over suit.
fn index(n: usize, size: f32, pip_size: f32, pip: Pip, number: Tone, tone: Tone) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            typo::disp(size, 800, 40)
                .on(div())
                .text_color(number)
                .child(n.to_string()),
        )
        .child(kit::icon(pip.path(), pip_size, tone).mt(u(2.)))
}

/// A face-up card; `done` adds the check.
fn open_card(n: usize, title: &str, pip: Pip, tone: Tone, done: Option<Tone>, bx: &Bx) -> Div {
    let face = div()
        .relative()
        .flex()
        .flex_col()
        .py(u(7.))
        .px(u(8.))
        .child(
            div()
                .absolute()
                .inset(u(4.))
                .rounded(u(6.))
                .border(u(1.))
                .border_color(alpha(bx.ink, 0.12)),
        )
        .child(index(n, 21., 13., pip, bx.ink, tone))
        .when_some(done, |d, best| {
            d.child(
                div()
                    .absolute()
                    .right(u(7.))
                    .top(u(7.))
                    .size(u(20.))
                    .rounded_full()
                    .bg(bx.deep)
                    .p(u(2.))
                    .child(
                        div()
                            .size_full()
                            .rounded_full()
                            .bg(best)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(kit::icon(kit::CHECK, 11., bx.paper)),
                    ),
            )
        })
        .child(div().flex_1())
        .child(kit::words(title, typo::text(11., 700).lh(1.16)).text_color(bx.ink));
    Raised {
        face: bx.cream,
        edge: bx.deep,
        radius: 11.,
        border: 2.5,
        drop: 5.,
    }
    .sized(CARD_W, CARD_H, face)
}

/// A face-down card: the lesson's number, a lock and its title.
fn locked_card(n: usize, title: &str, bx: &Bx) -> Div {
    let face = div()
        .relative()
        .flex()
        .flex_col()
        .py(u(7.))
        .px(u(8.))
        .child(paint::fill(Tex::Locked, *bx, 8.5).absolute().inset_0())
        .child(
            typo::disp(14., 800, 20)
                .on(div())
                .relative()
                .size(u(26.))
                .rounded_full()
                .bg(alpha(bx.deep, 0.85))
                .flex()
                .items_center()
                .justify_center()
                .text_color(bx.cream)
                .child(n.to_string()),
        )
        .child(
            kit::icon(kit::LOCK, 14., alpha(bx.cream, 0.8))
                .absolute()
                .right(u(9.))
                .top(u(10.)),
        )
        .child(div().flex_1())
        .child(
            div()
                .relative()
                .mx(u(-3.))
                .mb(u(-2.))
                .py(u(4.))
                .px(u(5.))
                .rounded(u(6.))
                .bg(alpha(bx.deep, 0.82))
                .text_color(bx.locked_text)
                .overflow_hidden()
                .child(kit::words(title, typo::text(11., 600).lh(1.16))),
        );
    Raised {
        face: bx.back_locked,
        edge: bx.deep,
        radius: 11.,
        border: 2.5,
        drop: 5.,
    }
    .sized(CARD_W, CARD_H, face)
}

/// A level without lessons yet: its label alone, on a shorter row.
const SOON_STEP: f32 = 80.;
const SOON_H: f32 = 56.;
/// The most cards may overlap on one line before the row takes two.
const MAX_OVERLAP: f32 = 6.;
/// Room under the last row.
const BOTTOM: f32 = 40.;

/// How a row's cards go on lines: the cards' widths are split into the
/// fewest even lines whose cards overlap by `MAX_OVERLAP` at most. Returns
/// each line's card count and gap.
fn lines(widths: &[f32], room: f32) -> Vec<(usize, f32)> {
    let total = widths.len();
    for n in 1..=total.max(1) {
        let per = total.div_ceil(n);
        let mut out = Vec::new();
        let mut fits = true;
        for chunk in widths.chunks(per.max(1)) {
            let natural: f32 = chunk.iter().sum();
            let gap = if chunk.len() > 1 {
                GAP.min((room - natural) / (chunk.len() - 1) as f32)
            } else {
                GAP
            };
            fits &= gap >= -MAX_OVERLAP;
            out.push((chunk.len(), gap));
        }
        if fits || per == 1 {
            return out;
        }
    }
    vec![(total, GAP)]
}

pub fn view(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let (ui, p) = (shell.ui.clone(), shell.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let list = match shell.lesson_list() {
        Ok(l) => l,
        Err(e) => {
            return typo::text(15., 400)
                .on(div())
                .absolute()
                .left(u(MARGIN))
                .top(u(96.))
                .child(e.to_string())
                .into_any_element();
        }
    };
    let done_all = list.done();
    let next = list.next.clone();
    let body = match list.view {
        ViewBy::Journey => "baralho-lessons-body",
        ViewBy::Themes => "baralho-lessons-body-themes",
    };

    // Everything below sits on the page, which scrolls under the
    // navigation: a top `y` on the canvas is `y - NAV_H` in it.
    let at = |y: f32| u(y - NAV_H);
    let head =
        heading(i18n.tr("nav-skill-tree"), Some(i18n.tr(body).into()), &bx)
            .absolute()
            .left(u(MARGIN))
            .right(u(MARGIN))
            .top(at(90.))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(u(10.))
                    .mb(u(2.))
                    .child(
                        typo::text(14., 600)
                            .on(div())
                            .text_color(bx.on_felt_mut)
                            .child(i18n.tr("tree-view-by")),
                    )
                    .child(shell.view_switch(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_baseline()
                    .gap(u(6.))
                    .ml(u(18.))
                    .child(
                        typo::disp(26., 800, 30)
                            .on(div())
                            .child(done_all.to_string()),
                    )
                    .child(
                        typo::text(14., 600)
                            .on(div())
                            .text_color(bx.on_felt_mut)
                            .child(i18n.tr_args(
                                "baralho-lessons-total",
                                &[("total", list.total().into())],
                            )),
                    ),
            );

    let mut page = div().relative().w(u(CANVAS_W)).child(head);
    let mut y = ROW_TOP;
    let mut n = 0usize;
    let mut first = true;
    for (row, track) in list.groups.iter().enumerate() {
        let total = track.lessons.len();
        let pip = Pip::TRACKS[row % Pip::TRACKS.len()];
        let soon = track.is_soon();
        let tone = if soon {
            bx.on_felt_mut
        } else {
            pip.tone(&bx, &p)
        };
        let done = track.done();
        let has_next = track.lessons.iter().any(|l| list.is_next(&l.id));
        let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
        let in_progress = done < total && (done > 0 || has_next);
        let detail = if soon {
            let mut text = i18n.tr("tree-soon");
            if track.kind == GroupKind::Level {
                text = format!(
                    "{text} · {}",
                    i18n.tr(&format!("{}-about", track.title_key()))
                );
            }
            div().child(text)
        } else {
            let count = i18n.tr_args(
                "baralho-lessons-count",
                &[("done", done.into()), ("total", total.into())],
            );
            let state = i18n.tr(if done == total {
                "baralho-lessons-track-done"
            } else if done > 0 || has_next {
                "baralho-lessons-track-open"
            } else if open {
                "baralho-lessons-track-ready"
            } else {
                "baralho-lessons-track-locked"
            });
            div()
                .flex()
                .child(
                    div()
                        .when(in_progress, |d| {
                            d.text_color(bx.amber).font_weight(gpui::FontWeight::BOLD)
                        })
                        .child(count),
                )
                .child(format!(" · {state}"))
        };
        let label = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(u(14.))
            .w(u(LABEL_W))
            .child(
                Raised {
                    face: if soon { bx.back_locked } else { bx.cream },
                    edge: bx.deep,
                    radius: 25.,
                    border: 2.5,
                    drop: 4.,
                }
                .sized(
                    50.,
                    50.,
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(kit::icon(pip.path(), 26., tone)),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        typo::disp(20., 800, 25)
                            .lh(1.08)
                            .on(div())
                            .when(soon, |d| d.text_color(bx.on_felt_mut))
                            .child(i18n.tr(&track.title_key())),
                    )
                    .child(
                        typo::text(13., 600)
                            .on(div())
                            .mt(u(3.))
                            .text_color(bx.on_felt_mut)
                            .child(detail),
                    ),
            );
        if soon {
            page = page.child(
                div()
                    .absolute()
                    .left(u(MARGIN))
                    .top(at(y))
                    .h(u(SOON_H))
                    .flex()
                    .items_center()
                    .opacity(0.72)
                    .child(label),
            );
            y += SOON_STEP;
            continue;
        }

        // Cards overlap a little when a long row holds the wide card; past
        // that the row takes a second line.
        let widths: Vec<f32> = track
            .lessons
            .iter()
            .map(|l| {
                if list.is_next(&l.id) {
                    NEXT_W + 2. * NEXT_MARGIN
                } else {
                    CARD_W
                }
            })
            .collect();
        let room = CANVAS_W - MARGIN - LABEL_W - RIGHT;
        let lines = lines(&widths, room);
        let mut column = div().flex().flex_col().gap(u(ROW_STEP - ROW_H));
        let mut lessons = track.lessons.iter().enumerate();
        for (count, gap) in &lines {
            let mut cards = div().flex().items_center().h(u(ROW_H)).gap(u(*gap));
            for (k, lesson) in lessons.by_ref().take(*count) {
                n += 1;
                let id = lesson.id.clone();
                let open_it =
                    cx.listener(move |this: &mut Shell, _: &gpui::ClickEvent, window, cx| {
                        this.open_lesson(&id);
                        this.select(Screen::Lesson, window, cx);
                    });
                let card = if next.as_deref() == Some(lesson.id.as_str()) {
                    next_card(k, lesson, pip, tone, n, &bx, i18n)
                } else {
                    match lesson.state {
                        LessonState::Locked => locked_card(k + 1, &lesson.title, &bx),
                        state => open_card(
                            k + 1,
                            &lesson.title,
                            pip,
                            tone,
                            (state == LessonState::Done).then_some(p.grade_best),
                            &bx,
                        ),
                    }
                };
                cards = cards.child(
                    card.id(("lesson", n))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.94))
                        .on_click(open_it),
                );
            }
            column = column.child(cards);
        }

        page = page.child(
            div()
                .absolute()
                .left(u(MARGIN))
                .top(at(y))
                .flex()
                .items_center()
                .when(first, |d| d.child(crate::tour::anchor("tree_first")))
                .child(label)
                .child(column),
        );
        first = false;
        y += lines.len() as f32 * ROW_STEP;
    }
    // The last row's cards, their drop and a margin under them.
    let height = (y - (ROW_STEP - ROW_H) + BOTTOM).max(CANVAS_H);
    div()
        .id("lessons_page")
        .absolute()
        .top(u(NAV_H))
        .left_0()
        .right_0()
        .bottom_0()
        .overflow_y_scroll()
        .track_scroll(&shell.scroll)
        .child(page.h(u(height - NAV_H)))
        .into_any_element()
}

/// The wide card of the lesson to continue, with its button.
fn next_card(
    k: usize,
    lesson: &LessonCard,
    pip: Pip,
    tone: Tone,
    n: usize,
    bx: &Bx,
    i18n: &taful_app::i18n::Localizer,
) -> Div {
    let meta = match lesson.minutes {
        Some(m) => i18n.tr_args("baralho-lessons-next-meta", &[("minutes", m.into())]),
        None => i18n.tr("baralho-lessons-next-meta-plain"),
    };
    let face = div()
        .flex()
        .flex_col()
        .py(u(10.))
        .px(u(12.))
        .child(crate::tour::anchor("tree_next"))
        .child(
            div()
                .flex()
                .gap(u(10.))
                .child(index(k + 1, 26., 15., pip, tone, tone).items_center())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            typo::disp(20., 800, 30)
                                .lh(1.05)
                                .on(div())
                                .text_color(bx.ink)
                                .child(lesson.title.clone()),
                        )
                        .child(
                            typo::text(12., 600)
                                .on(div())
                                .mt(u(2.))
                                .text_color(bx.mut_ink)
                                .child(meta),
                        ),
                ),
        )
        .child(div().flex_1())
        .child(
            kit::btn(
                ("tree_start", n),
                i18n.tr("baralho-lessons-continue"),
                Fill::Amber,
                true,
                bx,
            )
            .w_full(),
        );
    div()
        .relative()
        .flex_none()
        .mx(u(NEXT_MARGIN))
        .child(
            // The amber halo around the card to continue.
            div()
                .absolute()
                .top(u(-5.))
                .left(u(-5.))
                .w(u(NEXT_W + 10.))
                .h(u(NEXT_H + 10.))
                .rounded(u(16.))
                .bg(alpha(bx.amber, 0.6)),
        )
        .child(
            Raised {
                face: bx.cream_hero,
                edge: bx.deep,
                radius: 11.,
                border: 2.5,
                drop: 7.,
            }
            .sized(NEXT_W, NEXT_H, face.relative())
            .relative(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_rows_take_even_lines() {
        let room = CANVAS_W - MARGIN - LABEL_W - RIGHT;
        let cards = |n: usize| vec![CARD_W; n];
        // The mockup's longest row (12) stays on one line, overlap-free.
        assert_eq!(lines(&cards(12), room).len(), 1);
        assert!(lines(&cards(12), room)[0].1 >= 0.);
        // A row that would overlap by more than a few pixels splits evenly.
        let sixteen = lines(&cards(16), room);
        assert_eq!(sixteen.iter().map(|l| l.0).collect::<Vec<_>>(), [8, 8]);
        let mut with_next = cards(14);
        with_next[3] = NEXT_W + 2. * NEXT_MARGIN;
        assert_eq!(lines(&with_next, room).len(), 2);
        assert_eq!(lines(&[], room).len(), 0);
    }
}

//! Lessons: one row per track, one playing card per lesson. A finished
//! lesson is a face-up card with a check, the next one is the wide card
//! with the button, and locked lessons lie face down.

use gpui::{AnyElement, Context, Div, div, prelude::*};
use taful_app::model::LessonState;

use super::chrome::{MARGIN, heading};
use super::kit::{self, Fill, Pip, Raised};
use super::paint::{self, Tex};
use super::{Bx, CANVAS_W, Tone, bx, typo, u};
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

pub fn view(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let (ui, p) = (shell.ui.clone(), shell.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let loc = i18n.locale();
    let tracks = match shell.app.skill_tree(loc) {
        Ok(t) => t,
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
    let lessons = || tracks.iter().flat_map(|t| t.lessons.iter());
    let done_all = lessons().filter(|l| l.state == LessonState::Done).count();
    // The first available lesson anywhere is the one to continue.
    let next = lessons()
        .find(|l| l.state == LessonState::Available)
        .map(|l| l.id.clone());

    let head = heading(
        i18n.tr("nav-skill-tree"),
        Some(i18n.tr("baralho-lessons-body").into()),
        &bx,
    )
    .absolute()
    .left(u(MARGIN))
    .right(u(MARGIN))
    .top(u(90.))
    .child(
        div()
            .flex()
            .flex_none()
            .items_baseline()
            .gap(u(6.))
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
                        &[("total", lessons().count().into())],
                    )),
            ),
    );

    let mut page = div().size_full().child(head);
    let mut row = 0usize;
    let mut n = 0usize;
    for track in &tracks {
        let total = track.lessons.len();
        if total == 0 {
            continue;
        }
        let pip = Pip::TRACKS[row % Pip::TRACKS.len()];
        let tone = pip.tone(&bx, &p);
        let done = track
            .lessons
            .iter()
            .filter(|l| l.state == LessonState::Done)
            .count();
        let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
        let has_next = track
            .lessons
            .iter()
            .any(|l| next.as_deref() == Some(l.id.as_str()));
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
        let in_progress = done < total && (done > 0 || has_next);
        let label = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(u(14.))
            .w(u(LABEL_W))
            .child(
                Raised {
                    face: bx.cream,
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
                            .child(i18n.tr(&format!("track-{}", track.id))),
                    )
                    .child(
                        typo::text(13., 600)
                            .on(div())
                            .flex()
                            .mt(u(3.))
                            .text_color(bx.on_felt_mut)
                            .child(
                                div()
                                    .when(in_progress, |d| {
                                        d.text_color(bx.amber).font_weight(gpui::FontWeight::BOLD)
                                    })
                                    .child(count),
                            )
                            .child(format!(" · {state}")),
                    ),
            );

        // Cards overlap a little when a long track holds the wide card.
        let natural = track
            .lessons
            .iter()
            .map(|l| {
                if next.as_deref() == Some(l.id.as_str()) {
                    NEXT_W + 2. * NEXT_MARGIN
                } else {
                    CARD_W
                }
            })
            .sum::<f32>();
        let room = CANVAS_W - MARGIN - LABEL_W - RIGHT;
        let gap = if total > 1 {
            GAP.min((room - natural) / (total - 1) as f32)
        } else {
            GAP
        };
        let mut cards = div().flex().items_center().gap(u(gap));
        for (k, lesson) in track.lessons.iter().enumerate() {
            n += 1;
            let id = lesson.id.clone();
            let open_it = cx.listener(move |this: &mut Shell, _: &gpui::ClickEvent, window, cx| {
                this.open_lesson(&id);
                this.select(Screen::Lesson, window, cx);
            });
            let card = if next.as_deref() == Some(lesson.id.as_str()) {
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
                            &bx,
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

        page = page.child(
            div()
                .absolute()
                .left(u(MARGIN))
                .top(u(ROW_TOP + row as f32 * ROW_STEP))
                .h(u(ROW_H))
                .flex()
                .items_center()
                .when(row == 0, |d| d.child(crate::tour::anchor("tree_first")))
                .child(label)
                .child(cards),
        );
        row += 1;
    }
    page.into_any_element()
}

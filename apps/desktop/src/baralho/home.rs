//! Home: "today's hand". Four big cards (continue the lesson, review,
//! practice the biggest leak, mental math), one suit each, and the tracks
//! along the bottom.

use std::rc::Rc;

use gpui::{AnyElement, Context, Div, Stateful, div, prelude::*};
use taful_app::fmt;
use taful_app::model::LessonState;
use taful_app::today::Today;

use super::chrome::MARGIN;
use super::kit::{self, Fill, Pip, Raised};
use super::paint::{self, Tex};
use super::{Bx, CANVAS_W, bx, typo, u};
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::{Palette, alpha};

const CARD_W: f32 = 344.;
const CARD_H: f32 = 404.;
const CARD_TOP: f32 = 232.;
const CARD_STEP: f32 = 376.;
/// Grade dots and review pips shown at most.
/// A big card's width inside its border and padding, and a strip's mini.
const CARD_INNER_W: f32 = CARD_W - 2. * 2.5 - 2. * 26.;
const MINI_W: f32 = 22.;
const MAX_DOTS: usize = 16;
const MAX_PIPS: usize = 12;

/// One big card: its corner index, texts, a small visual and the action.
struct Big {
    pip: Pip,
    index: String,
    eyebrow: String,
    title: String,
    body: String,
    visual: Option<Div>,
    button: Stateful<Div>,
}

fn big(card: Big, slot: usize, bx: &Bx, p: &Palette) -> Div {
    let tone = card.pip.tone(bx, p);
    let face = div()
        .flex()
        .flex_col()
        .pt(u(22.))
        .px(u(26.))
        .pb(u(26.))
        .child(
            kit::icon(card.pip.path(), 150., alpha(tone, 0.07))
                .absolute()
                .right(u(14.))
                .bottom(u(76.)),
        )
        .child(
            div().flex().child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .min_w(u(44.))
                    .child(
                        typo::disp_at(96, 46., 800, 50)
                            .lh(0.95)
                            .on(div())
                            .text_color(tone)
                            .child(card.index),
                    )
                    .child(kit::icon(card.pip.path(), 28., tone).mt(u(4.))),
            ),
        )
        .child(
            typo::caps(12., 800, 120)
                .on(div())
                .mt(u(16.))
                .text_color(tone)
                .child(card.eyebrow.to_uppercase()),
        )
        .child(
            typo::disp(31., 800, 30)
                .lh(1.08)
                .on(div())
                .mt(u(4.))
                .mb(u(6.))
                .text_color(bx.ink)
                .child(card.title),
        )
        .child(
            typo::text(15., 400)
                .on(div())
                .text_color(bx.ink2)
                .child(card.body),
        )
        .children(card.visual.map(|v| v.mt(u(14.))))
        .child(div().flex_1())
        .child(card.button.w_full());
    kit::sheet(bx, face)
        .absolute()
        .left(u(MARGIN + slot as f32 * CARD_STEP))
        .top(u(CARD_TOP))
        .w(u(CARD_W))
        .h(u(CARD_H + 7.))
}

/// A tiny card of the lesson strip: done (a check), current (raised,
/// amber) or still face down.
fn mini(state: LessonState, current: bool, bx: &Bx, p: &Palette) -> Div {
    let outline = |face: Div| {
        div()
            .flex_none()
            .w(u(22.))
            .h(u(30.))
            .rounded(u(5.))
            .bg(bx.deep)
            .p(u(2.))
            .child(face.size_full().rounded(u(3.)))
    };
    if current {
        return Raised {
            face: bx.amber,
            edge: bx.deep,
            radius: 5.,
            border: 2.,
            drop: 3.,
        }
        .sized(22., 30., div())
        .relative()
        .top(u(-3.));
    }
    match state {
        LessonState::Done => outline(
            div()
                .flex()
                .items_center()
                .justify_center()
                .bg(bx.cream_hi)
                .child(kit::icon(kit::CHECK, 13., p.suit_clubs)),
        ),
        _ => outline(
            div()
                .relative()
                .child(paint::fill(Tex::Stripes, *bx, 3.).absolute().inset_0()),
        ),
    }
}

/// A track's tile: its suit, name, count and one tiny card per lesson.
fn track_tile(
    name: String,
    pip: Pip,
    states: &[LessonState],
    status: String,
    bx: &Bx,
    p: &Palette,
) -> Div {
    let done = states.iter().filter(|s| **s == LessonState::Done).count();
    let current = states.iter().position(|s| *s == LessonState::Available);
    let cards = states.iter().enumerate().map(|(i, s)| {
        let face = div().size_full().rounded(u(2.5));
        div()
            .flex_none()
            .w(u(16.))
            .h(u(23.))
            .rounded(u(4.))
            .bg(bx.deep)
            .p(u(1.5))
            .child(match s {
                LessonState::Done => face.bg(bx.cream),
                _ if current == Some(i) => face.bg(bx.amber),
                _ => face
                    .relative()
                    .child(paint::fill(Tex::Stripes, *bx, 2.5).absolute().inset_0()),
            })
    });
    div()
        .rounded(u(18.))
        .bg(alpha(bx.well, 0.42))
        .border(u(2.))
        .border_color(alpha(bx.well, 0.55))
        .py(u(14.))
        .px(u(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(u(10.))
                .h(u(40.))
                .child(
                    div()
                        .flex_none()
                        .size(u(36.))
                        .rounded_full()
                        .bg(bx.deep)
                        .p(u(2.))
                        .child(
                            div()
                                .size_full()
                                .rounded_full()
                                .bg(bx.cream)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(kit::icon(pip.path(), 19., pip.tone(bx, p))),
                        ),
                )
                .child(
                    typo::disp(17.5, 700, 20)
                        .lh(1.1)
                        .on(div())
                        .flex_1()
                        .min_w_0()
                        .child(name),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_baseline()
                        .gap(u(2.))
                        .whitespace_nowrap()
                        .child(typo::disp(17., 800, 0).on(div()).child(done.to_string()))
                        .child(
                            typo::text(12., 600)
                                .on(div())
                                .text_color(bx.on_felt_mut)
                                .child(format!("/ {}", states.len())),
                        ),
                ),
        )
        .child(div().flex().gap(u(3.)).mt(u(14.)).children(cards))
        .child(
            typo::text(12.5, 600)
                .on(div())
                .mt(u(10.))
                .text_color(bx.on_felt_mut)
                .child(status),
        )
}

pub fn view(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let (ui, p) = (shell.ui.clone(), shell.palette);
    let bx = bx(&ui);
    let i18n = &ui.i18n;
    let loc = i18n.locale();
    let visit = (shell.screen_visit, loc);
    let cached = shell
        .baralho
        .today
        .as_ref()
        .filter(|(at, _)| *at == visit)
        .map(|(_, t)| t.clone());
    let today: Rc<Today> = match cached.map_or_else(|| shell.app.today(loc).map(Rc::new), Ok) {
        Ok(t) => {
            shell.baralho.today = Some((visit, t.clone()));
            t
        }
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
    let num = |x: f64, decimals: u8| fmt::number(x, decimals, loc);

    // ---- heading ---------------------------------------------------------
    let date = i18n.tr_args(
        "baralho-date",
        &[
            (
                "weekday",
                i18n.tr(&format!("baralho-weekday-{}", today.date.weekday))
                    .into(),
            ),
            ("day", today.date.day.to_string().into()),
            (
                "month",
                i18n.tr(&format!("baralho-month-{}", today.date.month))
                    .into(),
            ),
        ],
    );
    let head = div()
        .absolute()
        .left(u(MARGIN))
        .top(u(96.))
        .flex()
        .flex_col()
        .child(
            typo::caps(13., 700, 120)
                .on(div())
                .text_color(bx.on_felt_mut)
                .child(date.to_uppercase()),
        )
        .child(
            typo::disp_at(96, 50., 800, 35)
                .lh(1.05)
                .on(div())
                .mt(u(4.))
                .child(i18n.tr("baralho-home-title")),
        )
        .child(
            typo::text(17., 400)
                .on(div())
                .mt(u(6.))
                .text_color(bx.on_felt_mut)
                .child(i18n.tr("baralho-home-body")),
        );

    // ---- spades: the next lesson -------------------------------------------
    let lesson = match today.next_lesson {
        Some((t, l)) => {
            let track = &today.tracks[t];
            let card = &track.lessons[l];
            let total = track.lessons.len();
            let body = match card.minutes {
                Some(m) => i18n.tr_args(
                    "baralho-home-lesson-body",
                    &[
                        ("n", (l + 1).into()),
                        ("total", total.into()),
                        ("minutes", m.into()),
                    ],
                ),
                None => i18n.tr_args(
                    "baralho-home-lesson-body-plain",
                    &[("n", (l + 1).into()), ("total", total.into())],
                ),
            };
            // A long track keeps to the card with a tighter gap.
            let n = total.max(2) as f32;
            let gap = ((CARD_INNER_W - MINI_W * n) / (n - 1.)).clamp(1., 5.);
            let strip = div().flex().gap(u(gap)).children(
                track
                    .lessons
                    .iter()
                    .enumerate()
                    .map(|(i, c)| mini(c.state, i == l, &bx, &p)),
            );
            let id = card.id.clone();
            Big {
                pip: Pip::Spade,
                index: (l + 1).to_string(),
                eyebrow: i18n.tr_args(
                    "baralho-home-continue",
                    &[("track", i18n.tr(&format!("track-{}", track.id)).into())],
                ),
                title: card.title.clone(),
                body,
                visual: Some(strip),
                button: kit::btn(
                    "home_lesson",
                    i18n.tr("baralho-home-lesson-go"),
                    Fill::Amber,
                    false,
                    &bx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_lesson(&id);
                    this.select(Screen::Lesson, window, cx);
                })),
            }
        }
        None => Big {
            pip: Pip::Spade,
            index: today.lessons_done().to_string(),
            eyebrow: i18n.tr("baralho-home-lessons-done"),
            title: i18n.tr("baralho-home-lessons-done-title"),
            body: i18n.tr("home-lesson-none"),
            visual: None,
            button: kit::btn(
                "home_lesson",
                i18n.tr("baralho-home-all-lessons"),
                Fill::Ink,
                false,
                &bx,
            )
            .on_click(
                cx.listener(|this, _, window, cx| this.select(Screen::SkillTree, window, cx)),
            ),
        },
    };

    // ---- hearts: reviews ---------------------------------------------------
    let due = today.due_now;
    let mut review_body = if due > 0 {
        i18n.tr_args(
            "baralho-home-review-body",
            &[("minutes", today.review_minutes().into())],
        )
    } else {
        match today.next_due {
            Some(when) => i18n.tr_args(
                "review-next-at",
                &[("when", fmt::datetime(when, loc).into())],
            ),
            None => i18n.tr("baralho-home-review-none-body"),
        }
    };
    if today.due_tomorrow > 0 {
        review_body.push(' ');
        review_body.push_str(&i18n.tr_args(
            "baralho-home-review-tomorrow",
            &[("count", today.due_tomorrow.into())],
        ));
    }
    let pips = (due > 0).then(|| {
        div()
            .flex()
            .flex_wrap()
            .w(u(6. * 24. + 5. * 8.))
            .gap_x(u(8.))
            .gap_y(u(6.))
            .children(
                (0..(due as usize).min(MAX_PIPS))
                    .map(|_| kit::icon(Pip::Heart.path(), 24., p.suit_hearts)),
            )
    });
    let review = Big {
        pip: Pip::Heart,
        index: due.to_string(),
        eyebrow: i18n.tr("baralho-home-review"),
        title: if due > 0 {
            i18n.tr_args("baralho-home-review-title", &[("count", due.into())])
        } else {
            i18n.tr("baralho-home-review-none-title")
        },
        body: review_body,
        visual: pips,
        button: kit::btn(
            "home_review",
            i18n.tr("baralho-home-review-go"),
            Fill::Ink,
            false,
            &bx,
        )
        .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Review, window, cx))),
    };

    // ---- clubs: the biggest leak ---------------------------------------------
    let practice = match &today.leak {
        Some((leak, grades)) => {
            let b = &leak.bucket;
            let place = [
                i18n.tr(&format!(
                    "baralho-on-street-{}",
                    taful_app::template::street_id(b.street)
                )),
                i18n.tr(&format!("node-type-{}", b.node_type.id())),
            ];
            let body = i18n.tr_args(
                "baralho-home-leak-body",
                &[
                    ("where", place.join(", ").into()),
                    ("mistakes", leak.mistakes.into()),
                    ("decisions", leak.decisions.into()),
                    (
                        "loss",
                        crate::felt::num(&ui, leak.total_ev_loss_bb, 1).into(),
                    ),
                ],
            );
            let from = grades.len().saturating_sub(MAX_DOTS);
            let dots = div()
                .flex()
                .flex_wrap()
                .gap(u(5.))
                .children(grades[from..].iter().map(|g| kit::dot(&bx, p.grade(*g))));
            let bucket = b.clone();
            Big {
                pip: Pip::Club,
                index: leak.mistakes.to_string(),
                eyebrow: i18n.tr("baralho-home-practice"),
                title: crate::felt::spot_label(&ui, &b.spot),
                body,
                visual: Some(dots),
                button: kit::btn(
                    "home_drill",
                    i18n.tr("home-leak-drill"),
                    Fill::Ink,
                    false,
                    &bx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let b = bucket.clone();
                    this.table.update(cx, |t, cx| t.start_drill(b, cx));
                    this.select(Screen::Table, window, cx);
                })),
            }
        }
        None => Big {
            pip: Pip::Club,
            index: "0".into(),
            eyebrow: i18n.tr("baralho-home-practice-none"),
            title: i18n.tr("baralho-home-practice-none-title"),
            body: i18n.tr("home-leaks-body"),
            visual: None,
            button: kit::btn(
                "home_drill",
                i18n.tr("baralho-home-practice-none-go"),
                Fill::Ink,
                false,
                &bx,
            )
            .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Table, window, cx))),
        },
    };

    // ---- diamonds: mental math -----------------------------------------------
    let pills = div().flex().flex_wrap().gap(u(8.)).children(
        today
            .calc_focus
            .iter()
            .map(|k| kit::pill(&bx, i18n.tr(&format!("calc-kind-{}", k.id())))),
    );
    let calc = Big {
        pip: Pip::Diamond,
        index: today
            .calc
            .map_or_else(|| "–".to_owned(), |c| num(c.mean_secs, 1)),
        eyebrow: i18n.tr("baralho-home-calc"),
        title: i18n.tr("nav-calc"),
        body: match today.calc {
            Some(c) => i18n.tr_args(
                "baralho-home-calc-body",
                &[
                    ("secs", num(c.mean_secs, 1).into()),
                    ("accuracy", num(c.accuracy * 100., 0).into()),
                    ("target", fmt::number_trim(c.target_secs, 1, loc).into()),
                ],
            ),
            None => i18n.tr("home-calc-short"),
        },
        visual: Some(pills),
        button: kit::btn(
            "home_calc",
            i18n.tr("home-calc-start"),
            Fill::Ink,
            false,
            &bx,
        )
        .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Calc, window, cx))),
    };

    // ---- tracks ------------------------------------------------------------
    let row_head = div()
        .absolute()
        .left(u(MARGIN))
        .right(u(MARGIN))
        .top(u(676.))
        .flex()
        .justify_between()
        .items_baseline()
        .child(typo::disp(22., 800, 20).on(div()).child(i18n.tr_args(
            "baralho-home-tracks",
            &[
                ("done", today.lessons_done().into()),
                ("total", today.lessons_total().into()),
            ],
        )))
        .child(
            typo::text(15., 700)
                .on(div())
                .id("home_all_lessons")
                .text_color(bx.amber)
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(
                    cx.listener(|this, _, window, cx| this.select(Screen::SkillTree, window, cx)),
                )
                .child(i18n.tr("baralho-home-all-lessons-link")),
        );
    let shown: Vec<_> = today
        .tracks
        .iter()
        .filter(|t| !t.lessons.is_empty())
        .collect();
    let n = shown.len().max(1) as f32;
    let gap = 16.75;
    let tile_w = (CANVAS_W - 2. * MARGIN - (n - 1.) * gap) / n;
    let tiles = shown.iter().enumerate().map(|(k, t)| {
        let states: Vec<LessonState> = t.lessons.iter().map(|l| l.state).collect();
        let done = states.iter().all(|s| *s == LessonState::Done);
        let open = states.iter().any(|s| *s != LessonState::Locked);
        let status = i18n.tr(if done {
            "baralho-track-done"
        } else if open {
            "baralho-track-open"
        } else {
            "baralho-track-locked"
        });
        track_tile(
            i18n.tr(&format!("track-{}", t.id)),
            Pip::TRACKS[k % Pip::TRACKS.len()],
            &states,
            status,
            &bx,
            &p,
        )
        .id(("home_track", k))
        .absolute()
        .left(u(MARGIN + k as f32 * (tile_w + gap)))
        .top(u(718.))
        .w(u(tile_w))
        .h(u(142.))
        .cursor_pointer()
        .hover(|s| s.bg(alpha(bx.well, 0.55)))
        .on_click(cx.listener(|this, _, window, cx| this.select(Screen::SkillTree, window, cx)))
    });
    let tiles: Vec<_> = tiles.collect();

    div()
        .size_full()
        .child(head)
        .child(big(lesson, 0, &bx, &p).child(crate::tour::anchor("home_continue")))
        .child(big(review, 1, &bx, &p))
        .child(big(practice, 2, &bx, &p))
        .child(big(calc, 3, &bx, &p))
        .child(
            div()
                .absolute()
                .left(u(MARGIN + CARD_STEP))
                .top(u(CARD_TOP))
                .w(u(2. * CARD_STEP + CARD_W))
                .h(u(CARD_H + 7.))
                .child(crate::tour::anchor("home_stats")),
        )
        .child(row_head)
        .children(tiles)
        .into_any_element()
}

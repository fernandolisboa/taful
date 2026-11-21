//! Home: "today's hand". Four big cards (the lesson, review, practice the
//! biggest leak, mental math), one suit each, and the tracks along the
//! bottom. The first card is the next step (D27) with the amber button:
//! the lesson, or the table once nothing is left to read, or the reviews
//! when some are due (the heart is dealt first and the lesson second).

use std::rc::Rc;

use gpui::{AnyElement, Context, Div, Stateful, div, prelude::*};
use taful_app::fmt;
use taful_app::model::LessonState;
use taful_app::next_step::{NextStep, StepLesson};
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
    /// Small controls in the card's top right corner.
    corner: Option<Div>,
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
        .children(card.corner.map(|c| c.absolute().top(u(22.)).right(u(22.))))
        .child(div().flex_1())
        .child(card.button.w_full());
    kit::sheet(bx, face)
        .absolute()
        .left(u(MARGIN + slot as f32 * CARD_STEP))
        .top(u(CARD_TOP))
        .w(u(CARD_W))
        .h(u(CARD_H + 7.))
}

/// A tiny card of the lesson strip, `w` wide: done (a check), current
/// (raised, amber) or still face down.
fn mini(state: LessonState, current: bool, w: f32, bx: &Bx, p: &Palette) -> Div {
    let outline = |face: Div| {
        div()
            .flex_none()
            .w(u(w))
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
        .sized(w, 30., div())
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
    if cached.is_none() {
        // A new visit ranks the leaks again: back to the biggest.
        shell.baralho.leak = 0;
    }
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

    // ---- the next step (D27) ---------------------------------------------
    // The first card is the step, with the amber button; the others follow
    // as shortcuts in their usual order.
    let step = &today.step;
    let words = crate::next_step::words(&shell.ui, step);
    // "Próximo passo · Fundamentos", or "Próximo passo" alone when that
    // would not fit the card (about 9.5 pixels a letter, tracked caps).
    let hero_eyebrow = {
        let full = words.eyebrow(&shell.ui);
        if full.chars().count() as f32 * 9.5 <= CARD_INNER_W {
            full
        } else {
            i18n.tr("next-step-label")
        }
    };
    let lesson_card = |l: &StepLesson, hero: bool, cx: &mut Context<Shell>| {
        let total = l.level_len();
        let body = match l.minutes {
            Some(m) => i18n.tr_args(
                "baralho-home-lesson-body",
                &[
                    ("n", l.number.into()),
                    ("total", total.into()),
                    ("minutes", m.into()),
                ],
            ),
            None => i18n.tr_args(
                "baralho-home-lesson-body-plain",
                &[("n", l.number.into()), ("total", total.into())],
            ),
        };
        // A long level keeps to the card with a tighter gap, then narrower
        // cards.
        let n = total.max(2) as f32;
        let gap = ((CARD_INNER_W - MINI_W * n) / (n - 1.)).clamp(1., 5.);
        let mini_w = MINI_W.min(((CARD_INNER_W - gap * (n - 1.)) / n).floor());
        let strip = div().flex().gap(u(gap)).children(
            l.level_states
                .iter()
                .enumerate()
                .map(|(i, s)| mini(*s, i + 1 == l.number, mini_w, &bx, &p)),
        );
        let id = l.id.clone();
        Big {
            pip: Pip::Spade,
            index: l.number.to_string(),
            eyebrow: if hero {
                hero_eyebrow.clone()
            } else {
                i18n.tr_args(
                    "baralho-home-continue",
                    &[("track", i18n.tr(&format!("level-{}", l.level)).into())],
                )
            },
            title: l.title.clone(),
            body,
            visual: Some(strip),
            corner: None,
            button: kit::btn(
                "home_lesson",
                if hero {
                    words.button.clone()
                } else {
                    i18n.tr("baralho-home-lesson-go")
                },
                if hero { Fill::Amber } else { Fill::Ink },
                false,
                &bx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_lesson(&id);
                this.select(Screen::Lesson, window, cx);
            })),
        }
    };
    let lesson = match step {
        NextStep::Lesson(l) => lesson_card(l, true, cx),
        // Nothing left to read: the spade deals the table practice.
        NextStep::Table { .. } => Big {
            pip: Pip::Spade,
            index: today.lessons_done().to_string(),
            eyebrow: hero_eyebrow.clone(),
            title: words.title.clone(),
            body: words.paragraph(),
            visual: None,
            corner: None,
            button: kit::btn("home_lesson", words.button.clone(), Fill::Amber, false, &bx)
                .on_click(crate::next_step::on_click(step.clone(), cx)),
        },
        // Reviews lead; the lesson stays at hand.
        NextStep::Review { .. } => match today.next_lesson {
            Some((g, l)) => {
                let level = &today.levels[g];
                let card = &level.lessons[l];
                let next = StepLesson {
                    id: card.id.clone(),
                    title: card.title.clone(),
                    level: level.id.clone(),
                    number: l + 1,
                    minutes: card.minutes,
                    level_states: level.lessons.iter().map(|c| c.state).collect(),
                    started: false,
                };
                lesson_card(&next, false, cx)
            }
            None => Big {
                pip: Pip::Spade,
                index: today.lessons_done().to_string(),
                eyebrow: i18n.tr("baralho-home-lessons-done"),
                title: i18n.tr("baralho-home-lessons-done-title"),
                body: crate::study::idle_text(&shell.ui, today.next_test.as_deref()),
                visual: None,
                corner: None,
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
    let review_leads = matches!(step, NextStep::Review { .. });
    let review = Big {
        pip: Pip::Heart,
        index: due.to_string(),
        eyebrow: if review_leads {
            hero_eyebrow.clone()
        } else {
            i18n.tr("baralho-home-review")
        },
        // The card's own short title: a second line would push the pips
        // into the button.
        title: if due > 0 {
            i18n.tr_args("baralho-home-review-title", &[("count", due.into())])
        } else {
            i18n.tr("baralho-home-review-none-title")
        },
        body: review_body,
        visual: pips,
        corner: None,
        button: kit::btn(
            "home_review",
            if review_leads {
                words.button.clone()
            } else {
                i18n.tr("baralho-home-review-go")
            },
            if review_leads { Fill::Amber } else { Fill::Ink },
            false,
            &bx,
        )
        .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Review, window, cx))),
    };

    // ---- clubs: the biggest leak ---------------------------------------------
    let picked = shell.baralho.leak.min(today.leaks.len().saturating_sub(1));
    let practice = match today.leaks.get(picked) {
        Some((leak, grades)) => {
            let b = &leak.bucket;
            // With more than one leak, which one the card shows.
            let picker = (today.leaks.len() > 1).then(|| {
                div()
                    .flex()
                    .gap(u(5.))
                    .children((0..today.leaks.len()).map(|i| {
                        let on = i == picked;
                        typo::text(12.5, 800)
                            .on(div())
                            .id(("home_leak", i))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(u(24.))
                            .rounded_full()
                            .border(u(2.))
                            .border_color(bx.deep)
                            .cursor_pointer()
                            .map(|d| {
                                if on {
                                    d.bg(bx.ink).text_color(bx.cream)
                                } else {
                                    d.bg(bx.cream_hi)
                                        .text_color(bx.ink2)
                                        .hover(|s| s.bg(bx.cream2))
                                }
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.baralho.leak = i;
                                cx.notify();
                            }))
                            .child((i + 1).to_string())
                    }))
            });
            let eyebrow = if picked == 0 {
                i18n.tr("baralho-home-practice")
            } else {
                i18n.tr_args("baralho-home-practice-nth", &[("n", (picked + 1).into())])
            };
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
                eyebrow,
                title: crate::felt::spot_label(&ui, &b.spot),
                body,
                visual: Some(dots),
                corner: picker,
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
            corner: None,
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
        corner: None,
        button: kit::btn(
            "home_calc",
            i18n.tr("home-calc-start"),
            Fill::Ink,
            false,
            &bx,
        )
        .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Calc, window, cx))),
    };

    // Due reviews deal the heart first (D27).
    let (first, second) = if review_leads {
        (review, lesson)
    } else {
        (lesson, review)
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
        .levels
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
            i18n.tr(&t.title_key()),
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
        .child(big(first, 0, &bx, &p).child(crate::tour::anchor("home_continue")))
        .child(big(second, 1, &bx, &p))
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

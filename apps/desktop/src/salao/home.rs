//! Salão's home: the next lesson as a hero, three numbers, the leaks.

use std::rc::Rc;

use gpui::{AnyElement, Context, Div, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::i18n::Locale;
use taful_app::model::{Grade, LessonState};
use taful_app::overview::{NextLesson, Overview, date_parts};

use super::colors::colors;
use super::frame::page_title;
use super::kit::{self, Face};
use crate::screen::Screen;
use crate::screens::table::TableScreen;
use crate::shell::Shell;
use crate::theme::alpha;

/// Minutes one review takes, for the "about N min" estimate.
const MINUTES_PER_REVIEW: f64 = 0.5;

/// The hero's art, as mocked.
const ART_W: f32 = 441.;
const ART_H: f32 = 236.;
/// The smallest the art gets in a narrow column, against the mockup's.
const ART_MIN: f32 = 0.72;
/// What the hero's text side takes with the mockup's padding: the button
/// and the track's ticks, and with the lessons' count after them.
const HERO_TICKS_W: f32 = 496.;
const HERO_COUNT_W: f32 = 599.;
/// A column narrower than this has no room in a number's panel for the
/// mockup's corners (the button, and the grade bar beside the caption):
/// they go under the number.
const STATS_STACK_W: f32 = 1000.;
/// A column narrower than this drops the leaks' bars (the amounts stay).
const LEAK_BAR_W: f32 = 900.;

/// How the hero fits a column `column` pixels wide: the mockup's from
/// 1042 up, then without the lessons' count, then with smaller art, then
/// without the ticks.
struct HeroFit {
    /// The art's scale against the mockup's.
    art: f32,
    /// The text side's padding.
    px: f32,
    py: f32,
    /// The text side's width inside its padding.
    inner: f32,
    ticks: bool,
    count: bool,
}

impl HeroFit {
    fn new(column: f32) -> Self {
        // The art gives its room to the text side until it is at its
        // smallest. The panel's border takes a pixel on each side.
        let art_w = (column - 2. - HERO_TICKS_W)
            .clamp(ART_W * ART_MIN, ART_W)
            .floor();
        let art = art_w / ART_W;
        let small = art < 0.85;
        let (px, py) = if small {
            (24., (30. * art).round())
        } else {
            (34., 30.)
        };
        let left = column - 2. - art_w;
        Self {
            art,
            px,
            py,
            inner: (left - 2. * px).max(0.),
            ticks: left >= HERO_TICKS_W,
            count: left >= HERO_COUNT_W,
        }
    }

    /// Whether this is the mockup's hero, untouched.
    fn full(&self) -> bool {
        self.art >= 1. && self.count
    }
}

/// What Salão keeps on the shell.
#[derive(Default)]
pub struct ShellState {
    /// The home's numbers, read once per visit and locale (a frame is
    /// drawn on every hover).
    overview: Option<((usize, Locale), Rc<Overview>)>,
    /// The content column's width on this frame, set by the frame before
    /// it asks for a screen (a small window leaves less than the mockup's).
    pub(super) column: f32,
}

/// The home screen's body, with its header.
pub fn home(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let (c, p) = (colors(), shell.palette);
    let ui = shell.ui.clone();
    let i18n = &ui.i18n;
    let loc = i18n.locale();
    let visit = (shell.screen_visit, loc);
    let cached = shell
        .salao
        .overview
        .as_ref()
        .filter(|(at, _)| *at == visit)
        .map(|(_, o)| o.clone());
    let o = match cached.map_or_else(|| shell.app.overview(loc).map(Rc::new), Ok) {
        Ok(o) => {
            shell.salao.overview = Some((visit, o.clone()));
            o
        }
        Err(e) => return crate::widgets::muted(e.to_string(), &ui, &p).into_any_element(),
    };
    if shell.leaks.is_none() {
        shell.leaks = Some(shell.app.leaks().unwrap_or_else(|e| {
            eprintln!("taful: leaks failed: {e}");
            Vec::new()
        }));
    }
    let leaks = shell.leaks.clone().unwrap_or_default();

    // Today, as "Friday, November 20".
    let date = date_parts(shell.app.now()).map(|(weekday, day, month)| {
        i18n.tr_args(
            "salao-date",
            &[
                (
                    "weekday",
                    i18n.tr(&format!("salao-weekday-{weekday}")).into(),
                ),
                ("day", day.into()),
                ("month", i18n.tr(&format!("salao-month-{month}")).into()),
            ],
        )
    });

    // ---- the next lesson -------------------------------------------------------
    let total_line = i18n.tr_args(
        "salao-home-lessons-total",
        &[
            ("done", o.home.lessons_done.into()),
            ("total", o.home.lessons_total.into()),
        ],
    );
    let small_note = |text: String| {
        kit::sans_text(12.5, FontWeight::NORMAL)
            .text_color(p.text_muted)
            .whitespace_nowrap()
            .child(text)
    };
    let column = shell.salao.column;
    let fit = HeroFit::new(column);
    let hero_left = match &o.next {
        Some(next) => {
            let id = next.id.clone();
            let track = i18n.tr(&format!("track-{}", next.track));
            let place = i18n.tr_args(
                "salao-home-lesson-place",
                &[
                    ("n", next.number.into()),
                    ("total", next.track_states.len().into()),
                ],
            );
            let meta = match next.minutes {
                Some(m) => format!(
                    "{place} · {}",
                    i18n.tr_args("salao-home-lesson-read", &[("minutes", m.into())])
                ),
                None => place,
            };
            // The mockup's size for a short title; long ones step down to
            // stay on one line.
            let letters = next.title.chars().count();
            let size: f32 = match letters {
                0..=20 => 50.,
                21..=27 => 40.,
                _ => 32.,
            };
            // A smaller hero has a shorter title line, and a narrow one a
            // smaller title still (about half its size per letter).
            let title_h = (51. * fit.art).round();
            let size = if fit.full() {
                size
            } else {
                size.min(title_h - 2.)
                    .min(fit.inner / (letters.max(1) as f32 * 0.5))
                    .max(20.)
                    .floor()
            };
            // The track's name goes when the label would not fit one line
            // (about 8.6 pixels per letter, tracked).
            let label = i18n.tr("home-continue-title");
            let with_track = format!("{label} · {track}");
            let label = if with_track.chars().count() as f32 * 8.6 <= fit.inner {
                with_track
            } else {
                label
            };
            div()
                .child(kit::eyebrow(label, p.accent).when(!fit.full(), |d| d.truncate()))
                .child(
                    kit::HERO
                        .text(size, 1.02)
                        .h(px(title_h))
                        .flex()
                        .items_center()
                        .mt(px(10.))
                        .mb(px(6.))
                        .whitespace_nowrap()
                        .when(!fit.full(), |d| d.overflow_hidden())
                        .child(next.title.clone()),
                )
                .child(
                    kit::sans_text(14., FontWeight::NORMAL)
                        .text_color(p.text_muted)
                        .when(!fit.full(), |d| d.truncate())
                        .child(meta),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(22.))
                        .child(
                            kit::btn(
                                "salao_home_lesson",
                                i18n.tr("salao-home-continue-lesson"),
                                true,
                                fit.art < 0.85,
                                &p,
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.open_lesson(&id);
                                    this.select(Screen::Lesson, window, cx);
                                },
                            )),
                        )
                        .when(fit.ticks, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .children(ticks(next, &p))
                                    .when(fit.count, |d| {
                                        d.child(small_note(total_line).ml(px(8.)))
                                    }),
                            )
                        }),
                )
        }
        None => div()
            .child(kit::eyebrow(i18n.tr("home-continue-title"), p.accent))
            .child(
                // The only text here that may take several lines.
                kit::sans_wraps(14., FontWeight::NORMAL)
                    .mt(px(10.))
                    .text_color(p.text_muted)
                    .child(i18n.tr("home-lesson-none")),
            )
            .child(div().flex_1())
            .child(small_note(total_line)),
    };
    let hero = kit::panel(&p)
        .mt(px(22.))
        .h(px((ART_H * fit.art).round() + 2.))
        .flex()
        .border_color(alpha(p.accent, 0.28))
        .child(crate::tour::anchor("home_continue"))
        .child(
            hero_left
                .relative()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .px(px(fit.px))
                .py(px(fit.py)),
        )
        // The part of the art the mockup's panel shows (see the sprite).
        .child(kit::image(
            kit::IMG_HERO,
            (ART_W * fit.art).round(),
            (ART_H * fit.art).round(),
        ));

    // ---- three numbers ---------------------------------------------------------
    // In a narrow column the three panels stack their parts: the label,
    // a smaller number, its caption and the button under them.
    let stacked = column < STATS_STACK_W;
    let (number_size, unit_size) = if stacked { (44., 19.) } else { (56., 24.) };
    let big = |number: String, unit: Option<String>| {
        div()
            .mt(px(14.))
            .flex()
            .items_end()
            .child(kit::STAT.text(number_size, 1.).child(number))
            .children(unit.map(|u| {
                kit::STAT_UNIT
                    .text(unit_size, 1.)
                    .ml(px(4.))
                    .mb(px(Face::lift(
                        (kit::STAT, number_size, 1.),
                        (kit::STAT_UNIT, unit_size, 1.),
                    )))
                    .text_color(p.text_muted)
                    .child(u)
            }))
    };
    let caption = |text: String| {
        if stacked {
            // May take two lines.
            kit::sans_lines(13., FontWeight::NORMAL)
        } else {
            kit::sans_text(13.5, FontWeight::NORMAL)
        }
        .mt(px(6.))
        .text_color(p.text_muted)
        .child(text)
    };
    let stat = |label: String| {
        kit::panel(&p)
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .map(|d| {
                if stacked {
                    d.px(px(18.)).py(px(16.))
                } else {
                    d.h(px(162.)).px(px(24.)).py(px(20.))
                }
            })
            .child(kit::eyebrow(label, p.accent).when(stacked, |d| d.truncate()))
    };
    let corner = |button: gpui::Stateful<Div>| {
        if stacked {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .justify_end()
                .pt(px(12.))
                .child(button)
        } else {
            div().absolute().right(px(20.)).top(px(16.)).child(button)
        }
    };

    let due = o.home.due_now;
    let reviews = stat(i18n.tr("home-due-title"))
        .child(big(due.to_string(), None))
        .child(caption(if due > 0 {
            let minutes = (f64::from(due) * MINUTES_PER_REVIEW).ceil().max(1.);
            i18n.tr_args("salao-home-due-today", &[("minutes", minutes.into())])
        } else {
            match o.home.next_due {
                Some(when) => i18n.tr_args(
                    "review-next-at",
                    &[("when", fmt::datetime(when, loc).into())],
                ),
                None => i18n.tr("salao-home-due-none"),
            }
        }))
        .child(corner(
            kit::btn(
                "salao_home_review",
                i18n.tr("salao-home-review-start"),
                false,
                true,
                &p,
            )
            .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Review, window, cx))),
        ));

    let (right, answered) = o.home.calc_recent;
    let calc = stat(i18n.tr("home-calc-title"))
        .child(match o.calc_mean_ms {
            Some(ms) => big(
                crate::felt::num(&ui, ms / 1000., 1),
                Some(i18n.tr("salao-seconds")),
            ),
            None => big("–".into(), None),
        })
        .child(caption(if answered > 0 {
            let pct = f64::from(right) * 100. / f64::from(answered);
            i18n.tr_args("salao-home-calc-pace", &[("percent", pct.round().into())])
        } else {
            i18n.tr("salao-home-calc-none")
        }))
        .child(corner(
            kit::btn(
                "salao_home_calc",
                i18n.tr("home-calc-start"),
                false,
                true,
                &p,
            )
            .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Calc, window, cx))),
        ));

    let week = o.week_total();
    let table = stat(i18n.tr("salao-home-table-week"))
        .child(big(
            week.to_string(),
            Some(i18n.tr_args("salao-home-decisions", &[("count", week.into())])),
        ))
        .child(caption(if week > 0 {
            let good = f64::from(o.week[0] + o.week[1]) * 100. / f64::from(week);
            i18n.tr_args("salao-home-table-good", &[("percent", good.round().into())])
        } else {
            i18n.tr("salao-home-table-none")
        }))
        .children(grade_bar(&o, &p, stacked))
        .child(corner(
            kit::btn(
                "salao_home_table",
                i18n.tr("salao-home-play"),
                false,
                true,
                &p,
            )
            .on_click(cx.listener(|this, _, window, cx| this.select(Screen::Table, window, cx))),
        ));

    // ---- the leaks -------------------------------------------------------------
    let worst = leaks.iter().map(|l| l.total_ev_loss_bb).fold(0., f64::max);
    let empty = leaks.is_empty();
    let rows = leaks.into_iter().enumerate().map(|(i, l)| {
        let label = TableScreen::bucket_label(&ui, &l.bucket);
        let stats = i18n.tr_args(
            "salao-leak-stats",
            &[
                ("mistakes", l.mistakes.into()),
                ("decisions", l.decisions.into()),
            ],
        );
        let frac = if worst > 0. {
            (l.total_ev_loss_bb / worst) as f32
        } else {
            0.
        };
        let bucket = l.bucket.clone();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .py(px(11.))
            .border_t_1()
            .border_color(c.line)
            .child(
                kit::ORDINAL
                    .text(26., kit::LEADING)
                    .w(px(40.))
                    .flex_none()
                    .text_color(p.accent)
                    .child((i + 1).to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(kit::sans_lines(14.5, FontWeight::NORMAL).child(label))
                    .child(
                        kit::sans_text(12.5, FontWeight::NORMAL)
                            .mt(px(1.))
                            .text_color(p.text_muted)
                            .child(stats),
                    ),
            )
            .when(column >= LEAK_BAR_W, |d| {
                d.child(
                    kit::bar(
                        frac,
                        6.,
                        alpha(p.text, 0.08),
                        p.grade_mistake,
                        p.grade_inaccuracy,
                    )
                    .w(px(150.))
                    .flex_none(),
                )
            })
            .child(
                div()
                    .w(px(110.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .items_end()
                    .child(
                        kit::AMOUNT
                            .text(21., kit::LEADING)
                            .child(format!("−{}", crate::felt::num(&ui, l.total_ev_loss_bb, 1))),
                    )
                    .child(
                        kit::sans_text(11.5, FontWeight::NORMAL)
                            .ml(px(3.))
                            .mb(px(Face::lift(
                                (kit::AMOUNT, 21., kit::LEADING),
                                (kit::SANS, 11.5, kit::LEADING),
                            )))
                            .text_color(p.text_muted)
                            .child(i18n.tr("salao-bb")),
                    ),
            )
            .child(
                div().w(px(118.)).flex_none().flex().justify_end().child(
                    kit::btn(
                        ("salao_home_drill", i),
                        i18n.tr("home-leak-drill"),
                        false,
                        true,
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let b = bucket.clone();
                        this.table.update(cx, |t, cx| t.start_drill(b, cx));
                        this.select(Screen::Table, window, cx);
                    })),
                ),
            )
    });
    let rows: Vec<_> = rows.collect();
    let leaks = kit::panel(&p)
        .mt(px(20.))
        .flex()
        .flex_col()
        .pt(px(20.))
        .px(px(26.))
        .pb(px(6.))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .child(
                    kit::TITLE
                        .text(23., kit::LEADING)
                        .child(i18n.tr("home-leaks-title")),
                )
                .child(
                    kit::sans_text(12.5, FontWeight::NORMAL)
                        .mb(px(Face::lift(
                            (kit::TITLE, 23., kit::LEADING),
                            (kit::SANS, 12.5, kit::LEADING),
                        )))
                        .text_color(p.text_muted)
                        .child(i18n.tr("salao-home-leaks-sub")),
                ),
        )
        .when(empty, |d| {
            d.child(
                kit::sans_text(14.5, FontWeight::NORMAL)
                    .mt(px(10.))
                    .mb(px(16.))
                    .text_color(p.text_muted)
                    .child(i18n.tr("home-leaks-body")),
            )
        })
        .children(rows);

    div()
        .flex()
        .flex_col()
        .children(date.map(|d| kit::eyebrow(d, c.dim)))
        .child(page_title(shell))
        .child(hero)
        .child(
            div()
                .relative()
                .mt(px(20.))
                .flex()
                .gap(px(20.))
                .child(crate::tour::anchor("home_stats"))
                .child(reviews)
                .child(calc)
                .child(table),
        )
        .child(leaks)
        .into_any_element()
}

/// One tick per lesson of the next lesson's track: done, current, to come.
fn ticks(next: &NextLesson, p: &crate::theme::Palette) -> Vec<Div> {
    let c = colors();
    let gold = p.accent;
    // The mockup's ticks (22 pixels, for a track of ten); a longer track's
    // are narrower so the row keeps its width.
    let n = next.track_states.len().max(1) as f32;
    let width = if n <= 10. {
        22.
    } else {
        ((10. * 22. + 9. * 5. - (n - 1.) * 5.) / n).floor()
    };
    next.track_states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let tick = div()
                .relative()
                .flex_none()
                .w(px(width))
                .h(px(6.))
                .rounded(px(3.));
            if *state == LessonState::Done {
                tick.bg(kit::vgrad(c.gold2, gold)).shadow(vec![kit::shadow(
                    alpha(gold, 0.45),
                    0.,
                    0.,
                    10.,
                    0.,
                )])
            } else if i + 1 == next.number {
                // The mockup's outline, one pixel out.
                tick.bg(alpha(gold, 0.35)).child(
                    div()
                        .absolute()
                        .left(px(-2.))
                        .top(px(-2.))
                        .w(px(width + 4.))
                        .h(px(10.))
                        .rounded(px(5.))
                        .border_1()
                        .border_color(gold),
                )
            } else {
                tick.bg(alpha(p.text, 0.12))
            }
        })
        .collect()
}

/// The week's decisions by grade, as one segmented bar: in the panel's
/// corner as mocked, or (`stacked`) on a row of its own under the caption.
fn grade_bar(o: &Overview, p: &crate::theme::Palette, stacked: bool) -> Option<Div> {
    const W: f32 = 118.;
    const GAP: f32 = 2.;
    let total = o.week_total();
    if total == 0 {
        return None;
    }
    let parts: Vec<(Grade, u32)> = Grade::ALL
        .into_iter()
        .zip(o.week)
        .filter(|(_, n)| *n > 0)
        .collect();
    let room = W - GAP * (parts.len() as f32 - 1.);
    let last = parts.len() - 1;
    Some(
        div()
            .map(|d| {
                if stacked {
                    d.flex_none().mt(px(10.))
                } else {
                    d.absolute().right(px(24.)).bottom(px(28.))
                }
            })
            .w(px(W))
            .h(px(8.))
            .flex()
            .gap(px(GAP))
            .children(parts.into_iter().enumerate().map(|(i, (g, n))| {
                div()
                    .h_full()
                    .w(px(room * n as f32 / total as f32))
                    .when(i == 0, |d| d.rounded_l(px(4.)))
                    .when(i == last, |d| d.rounded_r(px(4.)))
                    .bg(p.grade(g))
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hero_steps_down_with_the_column() {
        // The mockup's column, and the narrowest that still is the mockup.
        assert!(HeroFit::new(1080.).full());
        assert!(HeroFit::new(1042.).full());
        // Then the lessons' count goes.
        let f = HeroFit::new(1000.);
        assert!(f.art >= 1. && f.ticks && !f.count);
        // Then the art shrinks, and the ticks stay while it does.
        let f = HeroFit::new(900.);
        assert!(f.art < 1. && f.art > ART_MIN && f.ticks);
        // Then the ticks go, with the art at its smallest.
        let f = HeroFit::new(700.);
        assert!(!f.ticks && !f.count);
        assert_eq!(f.art, (ART_W * ART_MIN).floor() / ART_W);
        // The text side always has its padding's worth of room.
        for column in [700., 760., 814., 873., 938., 939., 1041.] {
            let f = HeroFit::new(column);
            assert!(f.inner > 300., "{column}");
        }
    }
}

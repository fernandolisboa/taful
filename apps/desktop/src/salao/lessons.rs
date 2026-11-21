//! Salão's lessons: the levels (or themes) down a timeline, the one in
//! progress open. Levels without lessons yet sit on the line as "em breve".

use gpui::{AnyElement, Context, Div, FontWeight, SharedString, Stateful, div, prelude::*, px};
use taful_app::fmt;
use taful_app::lessons::{GroupKind, LessonCard, LessonGroup};
use taful_app::model::LessonState;

use super::colors::colors;
use super::frame::page_title;
use super::kit::{self, Face};
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::{Palette, alpha};

/// A track's panel with its lessons folded: border, padding, the title row.
const TRACK_H: f32 = 74.;
/// Between two tracks.
const TRACK_GAP: f32 = 14.;
/// A lesson row, and the space between two.
const ROW_H: f32 = 52.;
const ROW_GAP: f32 = 4.;
/// From a track's top to the center of its medallion.
const MEDAL_Y: f32 = 38.;
/// A column narrower than this lists a track's lessons in one column
/// instead of the mockup's two.
const TWO_COLUMNS_W: f32 = 800.;

/// Where a group stands.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    /// No lessons yet ("em breve").
    Soon,
    Done,
    /// Holds the next lesson.
    Current,
    /// Has lessons to take, but the next one is in an earlier track.
    Open,
    Locked,
}

/// The lessons screen's body, with its header.
pub fn lessons(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    let (c, p) = (colors(), shell.palette);
    let ui = shell.ui.clone();
    let i18n = &ui.i18n;
    let list = match shell.lesson_list() {
        Ok(list) => list,
        Err(e) => return crate::widgets::muted(e.to_string(), &ui, &p).into_any_element(),
    };
    let next = list.next.clone();
    let (total, done) = (list.total(), list.done());
    let tracks: Vec<LessonGroup> = list.groups;

    // ---- the header ------------------------------------------------------------
    // "Ver por" above the count.
    let switch = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .mb(px(14.))
        .child(kit::label(
            kit::SANS_S14,
            12.5,
            p.text_muted,
            i18n.tr("tree-view-by"),
        ))
        .child(shell.view_switch(cx));
    let overall = div()
        .flex()
        .flex_col()
        .items_end()
        .child(switch)
        .child(
            div()
                .flex()
                .items_end()
                .child(kit::OVERALL.text(30., kit::LEADING).child(done.to_string()))
                .child(
                    kit::OVERALL
                        .text(15., kit::LEADING)
                        .ml(px(5.))
                        .mb(px(Face::lift(
                            (kit::OVERALL, 30., kit::LEADING),
                            (kit::OVERALL, 15., kit::LEADING),
                        )))
                        .text_color(p.text_muted)
                        .child(i18n.tr_args("salao-lessons-of", &[("total", total.into())])),
                ),
        )
        .child(
            kit::bar(
                done as f32 / total.max(1) as f32,
                5.,
                alpha(p.text, 0.1),
                p.accent,
                c.gold2,
            )
            .mt(px(6.))
            .w(px(220.)),
        );
    // The title's side takes the room the count leaves (its description
    // wraps in a narrow column).
    let header = div()
        .flex()
        .items_end()
        .justify_between()
        .gap(px(24.))
        .child(page_title(shell).flex_1().min_w_0())
        .child(overall.flex_none());
    let two_columns = shell.salao.column >= TWO_COLUMNS_W;

    // ---- the tracks ------------------------------------------------------------
    let mut list = div().relative().mt(px(22.)).pl(px(76.)).flex().flex_col();
    let mut panels = Vec::new();
    // Each track's top and the top of the track in progress, for the line.
    let (mut y, mut current_y, mut last_y) = (0., None, 0.);
    // The first and last groups with lessons: the gold runs between them.
    let (mut first_y, mut last_real_y): (Option<f32>, f32) = (None, 0.);
    let mut first_locked = true;
    let mut n = 0usize;
    for (k, track) in tracks.into_iter().enumerate() {
        let count = track.lessons.len();
        let finished = track.done();
        let stage = if track.is_soon() {
            Stage::Soon
        } else if finished == count {
            Stage::Done
        } else if track.lessons.iter().any(|l| Some(&l.id) == next.as_ref()) {
            Stage::Current
        } else if track.lessons.iter().any(|l| l.state != LessonState::Locked) {
            Stage::Open
        } else {
            Stage::Locked
        };
        // Only the track with the next lesson starts open; a click opens
        // or folds the others.
        let open =
            stage == Stage::Current || (stage != Stage::Soon && shell.group_expanded(&track));
        let rows = if two_columns {
            count.div_ceil(2)
        } else {
            count
        };
        let grid_h = rows as f32 * ROW_H + (rows as f32 - 1.) * ROW_GAP;

        let title = kit::TRACK
            .text(24., kit::LEADING)
            .when(matches!(stage, Stage::Locked | Stage::Soon), |d| {
                d.text_color(c.locked_title)
            })
            .child(i18n.tr(&track.title_key()));
        let tag = |text: String, color, bg: f32, ring: f32| {
            div()
                .relative()
                .px(px(9.))
                .py(px(3.))
                .rounded(px(99.))
                .bg(alpha(color, bg))
                .child(kit::ring(99., alpha(color, ring)))
                .child(kit::label(kit::SANS_S14, 10.5, color, text))
        };
        let note = match stage {
            Stage::Soon => div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(tag(i18n.tr("tree-soon"), c.dim, 0.08, 0.3))
                .when(track.kind == GroupKind::Level, |d| {
                    d.child(
                        kit::sans_text(13., FontWeight::NORMAL)
                            .text_color(c.dim)
                            .child(i18n.tr(&format!("{}-about", track.title_key()))),
                    )
                }),
            Stage::Done => tag(i18n.tr("tree-state-done"), p.grade_best, 0.1, 0.3),
            Stage::Current => tag(i18n.tr("salao-track-current"), c.gold2, 0.1, 0.35),
            Stage::Open if finished > 0 => tag(i18n.tr("salao-track-current"), c.gold2, 0.1, 0.35),
            Stage::Open => kit::sans_text(13., FontWeight::NORMAL)
                .text_color(c.dim)
                .child(i18n.tr_args("salao-track-open", &[("count", count.into())])),
            Stage::Locked => {
                let key = if first_locked && !open {
                    "tree-track-locked"
                } else {
                    "salao-track-locked"
                };
                first_locked = false;
                kit::sans_text(13., FontWeight::NORMAL)
                    .text_color(c.dim)
                    .child(i18n.tr_args(key, &[("count", count.into())]))
            }
        };
        let tally = div()
            .flex()
            .items_end()
            .child(
                kit::COUNT
                    .text(18., kit::LEADING)
                    .mr(px(3.))
                    .child(finished.to_string()),
            )
            .child(
                kit::sans_text(13., FontWeight::NORMAL)
                    .mb(px(Face::lift(
                        (kit::COUNT, 18., kit::LEADING),
                        (kit::SANS, 13., kit::LEADING),
                    )))
                    .text_color(p.text_muted)
                    .child(i18n.tr_args("salao-of", &[("total", count.into())])),
            );
        let group = track.clone();
        let head = div()
            .id(("salao_track", k))
            .flex()
            .items_center()
            .h(px(40.))
            .child(title)
            .child(note.ml(px(16.)))
            .child(div().flex_1())
            .when(stage != Stage::Soon, |d| d.child(tally))
            // The group in progress stays open; an empty one has nothing to show.
            .when(!matches!(stage, Stage::Current | Stage::Soon), |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_group(&group);
                        cx.notify();
                    }))
            });

        let mut panel = kit::panel(&p)
            .flex_none()
            .mb(px(TRACK_GAP))
            .px(px(24.))
            .py(px(16.))
            .when(stage != Stage::Soon && first_y.is_none(), |d| {
                d.child(crate::tour::anchor("tree_first"))
            })
            .child(medal(k, stage, &p))
            .child(head);
        panel = match stage {
            Stage::Current => panel.border_color(alpha(p.accent, 0.26)),
            Stage::Locked | Stage::Soon => panel
                .bg(kit::vgrad(c.locked_top, c.locked_bottom))
                .shadow(vec![kit::shadow(alpha(c.black, 0.3), 0., 8., 18., 0.)]),
            Stage::Done | Stage::Open => panel,
        };
        let mut height = TRACK_H;
        if open {
            let bar = (stage == Stage::Current).then(|| {
                kit::bar(
                    finished as f32 / count as f32,
                    5.,
                    alpha(p.text, 0.09),
                    p.accent,
                    c.gold2,
                )
                .mt(px(12.))
            });
            height += if bar.is_some() { 17. } else { 0. } + 12. + grid_h + 2.;
            // Two columns, filled one after the other.
            let (left, right) = track.lessons.split_at(rows);
            let column = |lessons: &[LessonCard], from: usize, cx: &mut Context<Shell>| {
                let mut col = div().flex_1().min_w_0().flex().flex_col().gap(px(ROW_GAP));
                for (i, l) in lessons.iter().enumerate() {
                    let is_next = next.as_deref() == Some(l.id.as_str());
                    col = col.child(lesson_row(shell, l, from + i, n + from + i, is_next, cx));
                }
                col
            };
            let grid = div()
                .mt(px(12.))
                .mb(px(2.))
                .flex()
                .gap(px(22.))
                .child(column(left, 0, cx))
                .when(two_columns, |d| d.child(column(right, rows, cx)));
            panel = panel.children(bar).child(grid);
        }
        n += count;
        if stage != Stage::Soon {
            first_y.get_or_insert(y);
            last_real_y = y;
        }
        if stage == Stage::Current && current_y.is_none() {
            current_y = Some(y);
        }
        last_y = y;
        y += height + TRACK_GAP;
        panels.push(panel);
    }
    // The line behind the medallions: gold down to the track in progress,
    // then fading.
    if !panels.is_empty() {
        let (top, bottom) = (MEDAL_Y - 8., last_y + MEDAL_Y + 2.);
        // Empty levels above the first one with lessons are not progress.
        let gold_from = first_y.map_or(top, |y| (y + MEDAL_Y).max(top));
        let gold_to = current_y
            .map_or(last_real_y + MEDAL_Y, |y| y + MEDAL_Y + 64.6)
            .clamp(gold_from, bottom);
        let fade_to = (gold_to + 67.).min(bottom);
        let piece = |from: f32, to: f32| {
            div()
                .absolute()
                .left(px(27.))
                .w(px(2.))
                .top(px(from))
                .h(px(to - from))
        };
        list = list
            .child(piece(top, gold_from).bg(alpha(p.accent, 0.12)))
            .child(piece(gold_from, gold_to).bg(p.accent))
            .child(piece(gold_to, fade_to).bg(kit::vgrad(p.accent, alpha(p.accent, 0.18))))
            .child(
                piece(fade_to, bottom).bg(kit::vgrad(alpha(p.accent, 0.18), alpha(p.accent, 0.12))),
            );
    }
    div()
        .flex()
        .flex_col()
        .child(header)
        .child(list.children(panels))
        .into_any_element()
}

/// A track's medallion, left of its panel on the line.
fn medal(index: usize, stage: Stage, p: &Palette) -> Div {
    let c = colors();
    let layer = || {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(28.))
    };
    let glow = match stage {
        Stage::Done => Some(kit::shadow(alpha(p.accent, 0.3), 0., 6., 16., 0.)),
        Stage::Current => Some(kit::shadow(alpha(p.accent, 0.35), 0., 0., 22., 0.)),
        Stage::Open | Stage::Locked | Stage::Soon => None,
    };
    let disc = match stage {
        Stage::Done => layer().child(kit::image(kit::IMG_MEDAL_DONE, 56., 56.)),
        Stage::Current | Stage::Open => layer()
            .bg(c.medal_bg)
            .border(px(1.5))
            .border_color(p.accent),
        Stage::Locked | Stage::Soon => layer()
            .bg(c.medal_bg)
            .border(px(1.5))
            .border_color(alpha(p.text, 0.14)),
    };
    let ink = match stage {
        Stage::Done => p.on_accent,
        Stage::Current | Stage::Open => c.gold2,
        Stage::Locked | Stage::Soon => c.dim,
    };
    div()
        .absolute()
        .left(px(-76.))
        .top(px(9.))
        .size(px(56.))
        .flex()
        .items_center()
        .justify_center()
        .children(glow.map(|g| layer().shadow(vec![g])))
        // The ring that cuts the line around the medallion.
        .child(
            div()
                .absolute()
                .top(px(-6.))
                .left(px(-6.))
                .size(px(68.))
                .rounded(px(34.))
                .bg(c.ring_bg),
        )
        .child(disc)
        .child(
            kit::MEDAL
                .text(22., kit::LEADING)
                .relative()
                .text_color(ink)
                .child(roman(index + 1)),
        )
}

/// One lesson of an open track. `k` is its place in the track, `n` among
/// all lessons.
fn lesson_row(
    shell: &Shell,
    l: &LessonCard,
    k: usize,
    n: usize,
    is_next: bool,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let (c, p) = (colors(), shell.palette);
    let i18n = &shell.ui.i18n;
    let loc = i18n.locale();
    let locked = l.state == LessonState::Locked;
    let done = l.state == LessonState::Done;

    let icon = div()
        .relative()
        .size(px(30.))
        .flex_none()
        .rounded(px(15.))
        .flex()
        .items_center()
        .justify_center()
        .border(px(1.5))
        .border_color(alpha(p.text, 0.14))
        .text_color(c.dim)
        .map(|d| {
            if done {
                d.border_color(alpha(p.accent, 0.6))
                    .bg(alpha(p.accent, 0.1))
                    .child(kit::icon(kit::SVG_CHECK, 15., c.gold2))
            } else if locked {
                d.child(kit::icon(kit::SVG_LOCK, 15., c.dim))
            } else {
                d.when(is_next, |d| {
                    d.border_color(p.accent)
                        .bg(kit::vgrad(c.gold_hi, p.accent))
                        .text_color(p.on_accent)
                })
                .child(kit::sans_text(12.5, FontWeight::SEMIBOLD).child((k + 1).to_string()))
            }
        });

    let minutes = l
        .minutes
        .map(|m| i18n.tr_args("tree-minutes", &[("minutes", m.into())]));
    let mut small: Vec<String> = Vec::new();
    if locked {
        small.push(i18n.tr("tree-state-locked"));
    }
    small.extend(minutes);
    if is_next {
        small.push(i18n.tr("salao-lesson-next"));
    }
    let text = div()
        .flex_1()
        .min_w_0()
        .child(
            kit::sans_text(14.5, FontWeight::NORMAL)
                .truncate()
                .when(locked, |d| d.text_color(c.lesson_locked))
                .child(SharedString::from(l.title.clone())),
        )
        .child(
            kit::sans_text(12., FontWeight::NORMAL)
                .text_color(if is_next {
                    c.gold2
                } else if locked {
                    c.lesson_locked_muted
                } else {
                    p.text_muted
                })
                .child(small.join(" · ")),
        );

    let (id, id2) = (l.id.clone(), l.id.clone());
    let side = if is_next {
        Some(
            kit::btn(
                ("salao_lesson_go", n),
                i18n.tr("salao-lesson-continue"),
                true,
                true,
                &p,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.open_lesson(&id2);
                this.select(Screen::Lesson, window, cx);
            }))
            .into_any_element(),
        )
    } else {
        l.best_score.filter(|_| done).map(|s| {
            let score = format!("{}%", fmt::number(s * 100.0, 0, loc));
            kit::sans_text(12., FontWeight::NORMAL)
                .flex_none()
                .text_color(p.text_muted)
                .child(i18n.tr_args("salao-lesson-quiz", &[("score", score.into())]))
                .into_any_element()
        })
    };

    div()
        .id(("salao_lesson", n))
        .relative()
        .h(px(ROW_H))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(13.))
        .px(px(12.))
        .rounded(px(10.))
        // Locked lessons open too (read-only preview, no quiz).
        .cursor_pointer()
        .when(is_next, |d| {
            d.bg(kit::hgrad(alpha(p.accent, 0.16), alpha(p.accent, 0.05), 1.))
                .shadow(vec![kit::shadow(alpha(c.black, 0.3), 0., 8., 20., 0.)])
                .child(kit::ring(10., alpha(p.accent, 0.5)))
                .child(crate::tour::anchor("tree_next"))
        })
        .when(!is_next, |d| d.hover(|s| s.bg(alpha(p.text, 0.03))))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.open_lesson(&id);
            this.select(Screen::Lesson, window, cx);
        }))
        .child(icon)
        .child(text)
        .children(side)
}

/// A small number in roman numerals (the tracks' medallions).
fn roman(mut n: usize) -> String {
    let mut out = String::new();
    for (value, numeral) in [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")] {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::roman;

    #[test]
    fn roman_numerals() {
        let got: Vec<String> = [1, 2, 3, 4, 5, 6, 9, 14].into_iter().map(roman).collect();
        assert_eq!(got, ["I", "II", "III", "IV", "V", "VI", "IX", "XIV"]);
    }
}

//! Skill tree (spec §15.1 item 2): tracks and lessons with lock state.

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::LessonState;

use crate::screen::Screen;
use crate::shell::Shell;
use crate::widgets::{button, card, muted, progress, title};

impl Shell {
    pub(crate) fn tree_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let tracks = match self.app.skill_tree(loc) {
            Ok(tr) => tr,
            Err(e) => return muted(e.to_string(), &ui, &p).into_any_element(),
        };
        // The first available lesson anywhere is "next".
        let next = tracks
            .iter()
            .flat_map(|tr| tr.lessons.iter())
            .find(|l| l.state == LessonState::Available)
            .map(|l| l.id.clone());
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg));
        let mut n = 0usize;
        for track in tracks {
            let total = track.lessons.len();
            if total == 0 {
                continue;
            }
            let done = track
                .lessons
                .iter()
                .filter(|l| l.state == LessonState::Done)
                .count();
            let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
            let expanded = open || self.lesson.expanded.contains(&track.id);
            let header = div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.sm))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            title(ui.i18n.tr(&format!("track-{}", track.id)), &ui)
                                .when(!open, |d| d.text_color(p.text_muted)),
                        )
                        .child(muted(
                            ui.i18n.tr_args(
                                "tree-track-progress",
                                &[("done", done.into()), ("total", total.into())],
                            ),
                            &ui,
                            &p,
                        )),
                )
                .when(open, |d| {
                    d.child(progress(done as f32 / total.max(1) as f32, &ui, &p))
                });
            let mut c = card(&ui, &p)
                .relative()
                .when(n == 0, |c| c.child(crate::tour::anchor("tree_first")))
                .child(header);
            if !expanded {
                // A locked track is one line, not a wall of grey rows; a
                // click expands it to preview its lessons.
                let track_id = track.id.clone();
                c = c.child(
                    div()
                        .id(("track", n))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.lesson.expanded.insert(track_id.clone());
                            cx.notify();
                        }))
                        .child(muted(
                            ui.i18n
                                .tr_args("tree-track-locked", &[("count", total.into())]),
                            &ui,
                            &p,
                        )),
                );
                n += total;
                col = col.child(c);
                continue;
            }
            for (k, lesson) in track.lessons.into_iter().enumerate() {
                n += 1;
                let locked = lesson.state == LessonState::Locked;
                let is_next = next.as_deref() == Some(lesson.id.as_str());
                let mut meta = Vec::new();
                if lesson.state != LessonState::Available {
                    meta.push(ui.i18n.tr(match lesson.state {
                        LessonState::Done => "tree-state-done",
                        _ => "tree-state-locked",
                    }));
                }
                if let Some(m) = lesson.minutes {
                    meta.push(ui.i18n.tr_args("tree-minutes", &[("minutes", m.into())]));
                }
                if let Some(s) = lesson.best_score {
                    meta.push(ui.i18n.tr_args(
                        "tree-best",
                        &[(
                            "score",
                            format!("{}%", fmt::number(s * 100.0, 0, loc)).into(),
                        )],
                    ));
                }
                let badge = match lesson.state {
                    LessonState::Done => "✓".to_owned(),
                    _ => (k + 1).to_string(),
                };
                let id = lesson.id.clone();
                let id2 = lesson.id.clone();
                c = c.child(
                    div()
                        .id(("lesson", n))
                        .flex()
                        .items_center()
                        .gap(px(t.spacing.md))
                        .px(px(t.spacing.md))
                        .py(px(t.spacing.sm))
                        .rounded(px(t.radii.md))
                        // Locked lessons open too (read-only preview, no quiz).
                        .cursor_pointer()
                        .hover(|s| s.bg(p.surface_raised))
                        .relative()
                        .when(is_next, |d| d.child(crate::tour::anchor("tree_next")))
                        .when(is_next, |d| {
                            d.bg(p.surface_raised)
                                .border_1()
                                .border_color(p.accent_muted)
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_lesson(&id);
                            this.select(Screen::Lesson, window, cx);
                        }))
                        .child(
                            div()
                                .size(px(28.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(t.radii.pill))
                                .border_1()
                                .text_size(px(t.text_size.sm))
                                .font_weight(FontWeight::SEMIBOLD)
                                .map(|d| match lesson.state {
                                    LessonState::Done => d
                                        .bg(p.accent)
                                        .border_color(p.accent)
                                        .text_color(p.on_accent),
                                    LessonState::Available => {
                                        d.border_color(p.accent).text_color(p.accent)
                                    }
                                    LessonState::Locked => {
                                        d.border_color(p.border).text_color(p.text_muted)
                                    }
                                })
                                .child(badge),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(t.text_size.md))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if locked { p.text_muted } else { p.text })
                                        .child(lesson.title),
                                )
                                .child(
                                    div()
                                        .text_size(px(t.text_size.sm))
                                        .text_color(p.text_muted)
                                        .child(meta.join(" · ")),
                                ),
                        )
                        .when(is_next, |d| {
                            d.child(
                                button(("start", n), ui.i18n.tr("tree-start"), true, &ui, &p)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.open_lesson(&id2);
                                        this.select(Screen::Lesson, window, cx);
                                    })),
                            )
                        }),
                );
            }
            col = col.child(c);
        }
        col.into_any_element()
    }
}

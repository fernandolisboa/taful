//! Skill tree (spec §15.1 item 2): tracks and lessons with lock state.

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::LessonState;

use crate::screen::Screen;
use crate::shell::Shell;
use crate::widgets::{card, muted, title};

impl Shell {
    pub(crate) fn tree_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let tracks = match self.app.skill_tree(loc) {
            Ok(tr) => tr,
            Err(e) => return muted(e.to_string(), &ui, &p).into_any_element(),
        };
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg)).child(muted(
            ui.i18n.tr("tree-intro"),
            &ui,
            &p,
        ));
        let mut n = 0usize;
        for track in tracks {
            let mut c = card(&ui, &p).child(title(ui.i18n.tr(&format!("track-{}", track.id)), &ui));
            for lesson in track.lessons {
                n += 1;
                let (state_key, badge) = match lesson.state {
                    LessonState::Done => ("tree-state-done", "✓"),
                    LessonState::Available => ("tree-state-available", "●"),
                    LessonState::Locked => ("tree-state-locked", "○"),
                };
                let locked = lesson.state == LessonState::Locked;
                let id = lesson.id.clone();
                let mut meta = vec![ui.i18n.tr(state_key)];
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
                c = c.child(
                    div()
                        .id(("lesson", n))
                        .flex()
                        .items_center()
                        .gap(px(t.spacing.md))
                        .px(px(t.spacing.md))
                        .py(px(t.spacing.sm))
                        .rounded(px(t.radii.md))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.surface_raised))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_lesson(&id);
                            this.select(Screen::Lesson, window, cx);
                        }))
                        .child(
                            div()
                                .w(px(t.spacing.xl))
                                .flex_none()
                                .text_color(if locked { p.text_muted } else { p.accent })
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
                        ),
                );
            }
            col = col.child(c);
        }
        col.into_any_element()
    }
}

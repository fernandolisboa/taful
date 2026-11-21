//! The lessons screen (spec §15.1 item 2, D23): the catalog grouped by
//! level or by theme ("Ver por"), each lesson with its lock state.

use gpui::{AnyElement, Context, Div, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::lessons::{LessonGroup, LessonList, ViewBy};
use taful_app::model::LessonState;

use crate::screen::Screen;
use crate::shell::Shell;
use crate::widgets::{button, card, muted, progress, segment, segmented, title};

impl Shell {
    /// How the lessons screen groups the catalog, as last picked.
    pub(crate) fn view_by(&mut self) -> ViewBy {
        if self.lesson.view_by.is_none() {
            self.lesson.view_by = Some(self.app.lesson_view());
        }
        self.lesson.view_by.unwrap_or_default()
    }

    /// Switches the lessons screen's grouping and remembers it.
    pub(crate) fn set_view_by(&mut self, view: ViewBy) {
        self.lesson.view_by = Some(view);
        if let Err(e) = self.app.set_lesson_view(view) {
            eprintln!("taful: saving the lessons view failed: {e}");
        }
    }

    /// The catalog grouped as the lessons screen shows it now.
    pub(crate) fn lesson_list(&mut self) -> taful_app::Result<LessonList> {
        let view = self.view_by();
        self.app.lesson_list(view, self.ui.i18n.locale())
    }

    /// The "Ver por" segments, one per view, in the style's own segmented
    /// control (without the label: each style places it).
    pub(crate) fn view_switch(&mut self, cx: &mut Context<Self>) -> Div {
        let current = self.view_by();
        let (ui, p) = (self.ui.clone(), self.palette);
        segmented(&ui, &p).children(ViewBy::ALL.into_iter().map(|view| {
            segment(
                ("tree_view", view as usize),
                ui.i18n.tr(view.label_key()),
                view == current,
                &ui,
                &p,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_view_by(view);
                cx.notify();
            }))
        }))
    }

    /// Whether the user unfolded a group (Classic and Salão keep groups
    /// without open lessons folded until clicked).
    pub(crate) fn group_expanded(&self, group: &LessonGroup) -> bool {
        self.lesson.expanded.contains(&group.title_key())
    }

    /// Opens a folded group, or folds an open one.
    pub(crate) fn toggle_group(&mut self, group: &LessonGroup) {
        let key = group.title_key();
        if !self.lesson.expanded.remove(&key) {
            self.lesson.expanded.insert(key);
        }
    }

    /// Classic's lessons tree.
    pub(crate) fn tree_classic(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let list = match self.lesson_list() {
            Ok(l) => l,
            Err(e) => return muted(e.to_string(), &ui, &p).into_any_element(),
        };
        let next = list.next.clone();
        let switch = div()
            .flex()
            .items_center()
            .gap(px(t.spacing.md))
            .child(muted(ui.i18n.tr("tree-view-by"), &ui, &p))
            .child(self.view_switch(cx));
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg)).child(switch);
        let mut n = 0usize;
        let mut g = 0usize;
        for track in list.groups {
            g += 1;
            let total = track.lessons.len();
            if total == 0 {
                // A level without lessons yet: its title and "em breve".
                col = col.child(
                    card(&ui, &p).child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                title(ui.i18n.tr(&track.title_key()), &ui).text_color(p.text_muted),
                            )
                            .child(muted(ui.i18n.tr("tree-soon"), &ui, &p)),
                    ),
                );
                continue;
            }
            let done = track
                .lessons
                .iter()
                .filter(|l| l.state == LessonState::Done)
                .count();
            let open = track.lessons.iter().any(|l| l.state != LessonState::Locked);
            let expanded = open || self.group_expanded(&track);
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
                            title(ui.i18n.tr(&track.title_key()), &ui)
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
                let group = track.clone();
                c = c.child(
                    div()
                        .id(("track", g))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_group(&group);
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

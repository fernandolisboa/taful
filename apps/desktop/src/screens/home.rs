//! Home (spec §15.1 item 1): due reviews, next lesson, calc, quick actions.

use gpui::{AnyElement, Context, div, prelude::*, px};
use taful_app::fmt;

use crate::screen::Screen;
use crate::shell::Shell;
use crate::widgets::{button, card, muted, title};

impl Shell {
    pub(crate) fn home_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let home = match self.app.home(loc) {
            Ok(h) => h,
            Err(e) => return muted(e.to_string(), &ui, &p).into_any_element(),
        };

        let reviews = card(&ui, &p)
            .child(title(ui.i18n.tr("home-due-title"), &ui))
            .child(
                div().text_size(px(t.text_size.md)).child(
                    ui.i18n
                        .tr_args("review-due", &[("count", home.due_now.into())]),
                ),
            )
            .when_some(home.next_due.filter(|_| home.due_now == 0), |d, when| {
                d.child(muted(
                    ui.i18n.tr_args(
                        "review-next-at",
                        &[("when", fmt::datetime(when, loc).into())],
                    ),
                    &ui,
                    &p,
                ))
            })
            .when(home.due_now > 0, |d| {
                d.child(
                    button("home_review", ui.i18n.tr("review-start"), true, &ui, &p).on_click(
                        cx.listener(|this, _, window, cx| this.select(Screen::Review, window, cx)),
                    ),
                )
            });

        let lesson = card(&ui, &p)
            .child(title(ui.i18n.tr("home-lesson-title"), &ui))
            .child(muted(
                ui.i18n.tr_args(
                    "home-progress",
                    &[
                        ("done", home.lessons_done.into()),
                        ("total", home.lessons_total.into()),
                    ],
                ),
                &ui,
                &p,
            ))
            .child(match &home.next_lesson {
                Some((id, name)) => {
                    let id = id.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(t.spacing.md))
                        .child(div().text_size(px(t.text_size.lg)).child(name.clone()))
                        .child(
                            button("home_lesson", ui.i18n.tr("home-open-lesson"), true, &ui, &p)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_lesson(&id);
                                    this.select(Screen::Lesson, window, cx);
                                })),
                        )
                        .into_any_element()
                }
                None => muted(ui.i18n.tr("home-lesson-none"), &ui, &p).into_any_element(),
            });

        let (right, total) = home.calc_recent;
        let calc = card(&ui, &p)
            .child(title(ui.i18n.tr("home-calc-title"), &ui))
            .child(muted(
                if total == 0 {
                    ui.i18n.tr("home-calc-none")
                } else {
                    ui.i18n.tr_args(
                        "home-calc-recent",
                        &[("correct", right.into()), ("total", total.into())],
                    )
                },
                &ui,
                &p,
            ))
            .child(
                button("home_calc", ui.i18n.tr("home-calc-start"), false, &ui, &p).on_click(
                    cx.listener(|this, _, window, cx| this.select(Screen::Calc, window, cx)),
                ),
            );

        let leaks = self.app.leaks().unwrap_or_default();
        let rows = leaks.into_iter().enumerate().map(|(i, l)| {
            let label = crate::screens::table::TableScreen::bucket_label(&ui, &l.bucket);
            let stats = ui.i18n.tr_args(
                "home-leak-stats",
                &[
                    ("mistakes", l.mistakes.into()),
                    ("decisions", l.decisions.into()),
                    ("loss", crate::felt::num(&ui, l.total_ev_loss_bb, 1).into()),
                ],
            );
            let bucket = l.bucket.clone();
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(t.spacing.md))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(t.text_size.md)).child(label))
                        .child(muted(stats, &ui, &p)),
                )
                .child(
                    button(
                        ("home_drill", i),
                        ui.i18n.tr("home-leak-drill"),
                        false,
                        &ui,
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let b = bucket.clone();
                        this.table.update(cx, |t, cx| t.start_drill(b, cx));
                        this.select(Screen::Table, window, cx);
                    })),
                )
        });
        let rows: Vec<_> = rows.collect();
        let leaks = card(&ui, &p)
            .child(title(ui.i18n.tr("home-leaks-title"), &ui))
            .when(rows.is_empty(), |c| {
                c.child(muted(ui.i18n.tr("home-leaks-body"), &ui, &p))
            })
            .children(rows);

        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.lg))
            .child(reviews)
            .child(lesson)
            .child(calc)
            .child(leaks)
            .into_any_element()
    }
}

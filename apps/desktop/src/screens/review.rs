//! Review session (spec §15.1 item 7): the FSRS queue, mixing kinds.

use gpui::{AnyElement, Context, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::{Answer, CalcOutcome, ReviewPrompt};

use crate::screens::calc::{outcome_view, round_view};
use crate::screens::lesson::QuestionState;
use crate::shell::{KeyResult, Shell};
use crate::widgets::{button, card, muted};

#[derive(Default)]
pub struct ReviewView {
    pub prompt: Option<ReviewPrompt>,
    pub feedback: Option<Feedback>,
    /// Reviews answered since the screen was opened.
    pub done: u32,
    pub loaded: bool,
}

pub enum Feedback {
    Calc(CalcOutcome),
    Quiz { answer: Answer, correct: bool },
}

impl ReviewView {
    pub fn calc_running(&self) -> bool {
        matches!(self.prompt, Some(ReviewPrompt::Calc { .. })) && self.feedback.is_none()
    }
}

impl Shell {
    pub(crate) fn review_enter(&mut self) {
        self.review = ReviewView::default();
    }

    fn review_load_next(&mut self) {
        self.review.feedback = None;
        self.input.clear();
        self.review.prompt = match self.app.next_review(self.ui.i18n.locale()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("taful: review queue failed: {e}");
                None
            }
        };
        self.review.loaded = true;
    }

    fn review_calc_answer(&mut self, given: Option<f64>) {
        let Some(ReviewPrompt::Calc { round, .. }) = &self.review.prompt else {
            return;
        };
        if self.review.feedback.is_some() {
            return;
        }
        match self.app.calc_submit(round, given) {
            Ok(o) => {
                self.review.feedback = Some(Feedback::Calc(o));
                self.review.done += 1;
            }
            Err(e) => eprintln!("taful: review submit failed: {e}"),
        }
        self.input.clear();
    }

    fn review_quiz_answer(&mut self, a: Answer) {
        let Some(ReviewPrompt::Quiz { item_id, .. }) = &self.review.prompt else {
            return;
        };
        if self.review.feedback.is_some() {
            return;
        }
        match self.app.answer_quiz_review(&item_id.clone(), a) {
            Ok(correct) => {
                self.review.feedback = Some(Feedback::Quiz { answer: a, correct });
                self.review.done += 1;
            }
            Err(e) => eprintln!("taful: review answer failed: {e}"),
        }
        self.input.clear();
    }

    pub(crate) fn review_key(&mut self, key: KeyResult, _cx: &mut Context<Self>) {
        if !self.review.loaded {
            return;
        }
        match key {
            KeyResult::Submit if self.review.feedback.is_some() => self.review_load_next(),
            KeyResult::Submit => match &self.review.prompt {
                Some(ReviewPrompt::Calc { .. }) => {
                    if let Some(x) = fmt::parse_number(&self.input) {
                        self.review_calc_answer(Some(x));
                    }
                }
                Some(ReviewPrompt::Quiz { question, .. }) if question.numeric.is_some() => {
                    if let Some(x) = fmt::parse_number(&self.input) {
                        self.review_quiz_answer(Answer::Numeric(x));
                    }
                }
                _ => {}
            },
            KeyResult::Escape if self.review.calc_running() => self.review_calc_answer(None),
            _ => {}
        }
    }

    fn review_next_button(&self, label: &str, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        button(
            "review_next",
            self.ui.i18n.tr(label),
            true,
            &self.ui,
            &self.palette,
        )
        .on_click(cx.listener(|this, _, window, cx| {
            this.review_load_next();
            window.focus(&this.focus);
            cx.notify();
        }))
    }

    pub(crate) fn review_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let due = self.app.due_count().unwrap_or(0);
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg)).child(muted(
            ui.i18n.tr("review-intro"),
            &ui,
            &p,
        ));
        if self.review.done > 0 {
            col = col.child(muted(
                ui.i18n
                    .tr_args("review-session-done", &[("count", self.review.done.into())]),
                &ui,
                &p,
            ));
        }

        let body: AnyElement = match (&self.review.prompt, &self.review.feedback) {
            _ if !self.review.loaded => card(&ui, &p)
                .child(ui.i18n.tr_args("review-due", &[("count", due.into())]))
                .when(due > 0, |d| {
                    d.child(self.review_next_button("review-start", cx))
                })
                .into_any_element(),
            (None, _) => {
                let next = self
                    .app
                    .store()
                    .next_due_after(self.app.now())
                    .ok()
                    .flatten();
                card(&ui, &p)
                    .child(ui.i18n.tr("review-empty"))
                    .when_some(next, |d, when| {
                        d.child(muted(
                            ui.i18n.tr_args(
                                "review-next-at",
                                &[("when", fmt::datetime(when, loc).into())],
                            ),
                            &ui,
                            &p,
                        ))
                    })
                    .into_any_element()
            }
            (Some(ReviewPrompt::Calc { round, .. }), None) => card(&ui, &p)
                .child(round_view(
                    round,
                    &self.input,
                    self.app.now(),
                    round
                        .exercise
                        .target(self.app.calc_settings())
                        .as_secs_f64(),
                    &ui,
                    &p,
                ))
                .child(
                    button(
                        "review_calc_submit",
                        ui.i18n.tr("calc-submit"),
                        true,
                        &ui,
                        &p,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(x) = fmt::parse_number(&this.input) {
                            this.review_calc_answer(Some(x));
                        }
                        window.focus(&this.focus);
                        cx.notify();
                    })),
                )
                .into_any_element(),
            (Some(ReviewPrompt::Calc { round, .. }), Some(Feedback::Calc(o))) => card(&ui, &p)
                .child(outcome_view(round, o, &ui, &p))
                .child(self.review_next_button("review-continue", cx))
                .into_any_element(),
            (
                Some(ReviewPrompt::Quiz {
                    question,
                    lesson_title,
                    ..
                }),
                fb,
            ) => {
                let (picked, feedback) = match fb {
                    Some(Feedback::Quiz { answer, correct }) => (Some(*answer), Some(*correct)),
                    _ => (None, None),
                };
                let q = question.clone();
                let mut c = card(&ui, &p)
                    .child(muted(
                        ui.i18n.tr_args(
                            "review-from-lesson",
                            &[("lesson", lesson_title.clone().into())],
                        ),
                        &ui,
                        &p,
                    ))
                    .child(self.question_view(
                        "review_choice",
                        &q,
                        QuestionState {
                            picked,
                            feedback,
                            input: &self.input,
                        },
                        |this, i| this.review_quiz_answer(Answer::Choice(i)),
                        cx,
                    ));
                if feedback.is_some() {
                    c = c.child(self.review_next_button("review-continue", cx));
                } else if q.numeric.is_some() {
                    c = c.child(
                        button("review_quiz_check", ui.i18n.tr("quiz-check"), true, &ui, &p)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(x) = fmt::parse_number(&this.input) {
                                    this.review_quiz_answer(Answer::Numeric(x));
                                }
                                window.focus(&this.focus);
                                cx.notify();
                            })),
                    );
                }
                c.into_any_element()
            }
            (Some(ReviewPrompt::Calc { .. }), Some(Feedback::Quiz { .. })) => {
                card(&ui, &p).into_any_element()
            }
        };
        col.child(body).into_any_element()
    }
}

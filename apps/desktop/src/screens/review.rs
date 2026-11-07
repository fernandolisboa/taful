//! Review session (spec §15.1 item 7): the FSRS queue, mixing kinds.

use gpui::{AnyElement, Context, Task, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::{Answer, CalcOutcome, ReviewPrompt};
use taful_app::preflop::{Action, GradedDecision};
use taful_app::review_session::ReviewStep;

use crate::felt::{self, Explain};
use crate::screen::Screen;
use crate::screens::calc::{outcome_view, round_view};
use crate::screens::lesson::QuestionState;
use crate::shell::{KeyResult, Shell};
use crate::widgets::{button, card, muted};

/// Calc and quiz review cards keep a definite width (wrapped text measures
/// at it) that still fits the default window.
pub(crate) const REVIEW_CARD_WIDTH: f32 = 760.;

#[derive(Default)]
pub struct ReviewView {
    pub prompt: Option<ReviewPrompt>,
    pub feedback: Option<Feedback>,
    /// Reviews answered since the screen was opened.
    pub done: u32,
    pub loaded: bool,
    /// A spot-template deal runs in the background (it can solve rivers).
    pub dealing: bool,
    /// Bumped by every load; an older background deal is dropped.
    generation: u64,
    _deal: Option<Task<()>>,
}

pub enum Feedback {
    Calc(CalcOutcome),
    Quiz { answer: Answer, correct: bool },
    Spot(Box<GradedDecision>),
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

    /// Loads the next due item. Spot templates are dealt on the background
    /// executor (river templates solve the river, spec §7.7).
    pub(crate) fn review_load_next(&mut self, cx: &mut Context<Self>) {
        self.review.feedback = None;
        self.input.clear();
        self.review.prompt = None;
        self.review.generation += 1;
        self.review_step(Vec::new(), cx);
    }

    fn review_step(&mut self, mut skip: Vec<String>, cx: &mut Context<Self>) {
        let locale = self.ui.i18n.locale();
        let step = match self.app.next_review_step(locale, &skip) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("taful: review queue failed: {e}");
                ReviewStep::Empty
            }
        };
        match step {
            ReviewStep::Ready(p) => self.review_show(Some(p)),
            ReviewStep::Empty => self.review_show(None),
            ReviewStep::Deal(deal) => {
                self.review.dealing = true;
                let generation = self.review.generation;
                let job = cx.background_executor().spawn(async move {
                    let outcome = deal.run();
                    (deal, outcome)
                });
                self.review._deal = Some(cx.spawn(async move |this, cx| {
                    let (deal, outcome) = job.await;
                    this.update(cx, |s, cx| {
                        if s.review.generation != generation {
                            return;
                        }
                        let id = deal.item_id.clone();
                        match s.app.finish_review_deal(deal, outcome) {
                            Ok(Some(p)) => s.review_show(Some(p)),
                            Ok(None) => {
                                skip.push(id);
                                s.review_step(skip, cx);
                            }
                            Err(e) => {
                                eprintln!("taful: review deal failed: {e}");
                                s.review_show(None);
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
        }
    }

    fn review_show(&mut self, prompt: Option<ReviewPrompt>) {
        self.review.prompt = prompt;
        self.review.dealing = false;
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

    fn review_spot_answer(&mut self, a: Action) {
        let Some(ReviewPrompt::Spot { item_id, spot, .. }) = &self.review.prompt else {
            return;
        };
        if self.review.feedback.is_some() {
            return;
        }
        let (item_id, spot) = (item_id.clone(), spot.clone());
        match self.app.answer_spot_review(&item_id, &spot, a) {
            Ok(d) => {
                self.review.feedback = Some(Feedback::Spot(Box::new(d)));
                self.review.done += 1;
            }
            Err(e) => eprintln!("taful: review answer failed: {e}"),
        }
    }

    /// A spot review: the table, hero's options, then the explanation.
    fn review_spot_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let Some(ReviewPrompt::Spot { spot, .. }) = &self.review.prompt else {
            return div().into_any_element();
        };
        let trainers = match self.app.trainers() {
            Ok(tr) => tr,
            Err(e) => return muted(e.to_string(), &ui, &p).into_any_element(),
        };
        let feedback = match &self.review.feedback {
            Some(Feedback::Spot(d)) => Some(d.clone()),
            _ => None,
        };
        let state = spot.state().clone();
        let actions = if feedback.is_none() {
            spot.actions(&trainers)
        } else {
            vec![]
        };
        let buttons: Vec<_> = actions
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                button(
                    ("review_act", i),
                    felt::action_label(&ui, &state, a),
                    matches!(a, Action::Raise(_) | Action::AllIn),
                    &ui,
                    &p,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.review_spot_answer(a);
                    window.focus(&this.focus);
                    cx.notify();
                }))
            })
            .collect();
        let bucket = spot.context(&trainers).bucket();
        let mut panel = felt::panel(&ui, &p).child(muted(
            ui.i18n.tr_args(
                "review-from-spot",
                &[(
                    "spot",
                    crate::screens::table::TableScreen::bucket_label(&ui, &bucket).into(),
                )],
            ),
            &ui,
            &p,
        ));
        panel = match &feedback {
            Some(d) => panel
                .child(felt::explanation(&ui, &p, &Explain::of(d)))
                .child(self.review_next_button("review-continue", cx)),
            None => panel.child(muted(ui.i18n.tr("table-spot-prompt"), &ui, &p)),
        };
        div()
            .flex()
            .gap(px(t.spacing.xl))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.lg))
                    .child(felt::table(&ui, &p, &state, spot.hero(), false))
                    .child(div().flex().gap(px(t.spacing.sm)).children(buttons)),
            )
            .child(panel)
            .into_any_element()
    }

    pub(crate) fn review_key(&mut self, key: KeyResult, cx: &mut Context<Self>) {
        if !self.review.loaded || self.review.dealing {
            return;
        }
        match key {
            KeyResult::Submit if self.review.feedback.is_some() => self.review_load_next(cx),
            KeyResult::Submit => match &self.review.prompt {
                Some(ReviewPrompt::Calc { .. }) => {
                    if let Some(x) = fmt::parse_number(&self.input, self.ui.i18n.locale()) {
                        self.review_calc_answer(Some(x));
                    }
                }
                Some(ReviewPrompt::Quiz { question, .. }) if question.numeric.is_some() => {
                    if let Some(x) = fmt::parse_number(&self.input, self.ui.i18n.locale()) {
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
            if !this.review.dealing {
                this.review_load_next(cx);
            }
            window.focus(&this.focus);
            cx.notify();
        }))
    }

    pub(crate) fn review_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let due = self.app.due_count().unwrap_or(0);
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg));
        if self.review.done > 0 {
            col = col.child(muted(
                ui.i18n
                    .tr_args("review-session-done", &[("count", self.review.done.into())]),
                &ui,
                &p,
            ));
        }

        let spot = matches!(self.review.prompt, Some(ReviewPrompt::Spot { .. }));
        let body: AnyElement = match (&self.review.prompt, &self.review.feedback) {
            _ if self.review.dealing => card(&ui, &p)
                .child(muted(ui.i18n.tr("review-dealing"), &ui, &p))
                .into_any_element(),
            _ if spot && self.review.loaded => self.review_spot_view(cx),
            _ if !self.review.loaded && due > 0 => card(&ui, &p)
                .child(ui.i18n.tr_args("review-due", &[("count", due.into())]))
                .child(self.review_next_button("review-start", cx))
                .into_any_element(),
            // The queue is empty (or a session ended with items it could not
            // deal): say what fills it and when the next item is due, and
            // offer to restart while anything is still due.
            _ if !self.review.loaded || self.review.prompt.is_none() => {
                let next = self
                    .app
                    .store()
                    .next_due_after(self.app.now())
                    .ok()
                    .flatten();
                crate::widgets::empty_state(
                    ui.i18n.tr("review-empty"),
                    ui.i18n.tr("review-empty-body"),
                    &ui,
                    &p,
                )
                .when(due == 0, |d| {
                    d.when_some(next, |d, when| {
                        d.child(muted(
                            ui.i18n.tr_args(
                                "review-next-at",
                                &[("when", fmt::datetime(when, loc).into())],
                            ),
                            &ui,
                            &p,
                        ))
                    })
                })
                .when(due > 0, |d| {
                    d.child(muted(
                        ui.i18n.tr_args("review-due", &[("count", due.into())]),
                        &ui,
                        &p,
                    ))
                    .child(self.review_next_button("review-start", cx))
                })
                .child(
                    div()
                        .flex()
                        .gap(px(t.spacing.sm))
                        .pt(px(t.spacing.sm))
                        .child(
                            button("rv_table", ui.i18n.tr("hand-review-play"), true, &ui, &p)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select(Screen::Table, window, cx)
                                })),
                        )
                        .child(
                            button("rv_calc", ui.i18n.tr("review-go-calc"), false, &ui, &p)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select(Screen::Calc, window, cx)
                                })),
                        ),
                )
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
                        if let Some(x) = fmt::parse_number(&this.input, this.ui.i18n.locale()) {
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
                                if let Some(x) =
                                    fmt::parse_number(&this.input, this.ui.i18n.locale())
                                {
                                    this.review_quiz_answer(Answer::Numeric(x));
                                }
                                window.focus(&this.focus);
                                cx.notify();
                            })),
                    );
                }
                c.into_any_element()
            }
            _ => card(&ui, &p).into_any_element(),
        };
        // Calc and quiz cards keep the reading width; a spot needs the table's.
        col.when(!spot, |c| c.w(px(REVIEW_CARD_WIDTH)))
            .child(body)
            .into_any_element()
    }
}

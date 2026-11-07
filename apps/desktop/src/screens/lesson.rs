//! Lesson reader and quiz (spec §15.1 item 3).

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::{Answer, LessonState, OpenLesson, QuizResult, QuizView, RenderedQuestion};

use crate::screen::Screen;
use crate::shell::{KeyResult, Shell};
use crate::widgets::{answer_box, button, card, document, muted, title};

#[derive(Default)]
pub struct LessonView {
    /// The lesson being read (kept across locale changes).
    pub current: Option<String>,
    /// Rendered lesson, or the render error.
    pub open: Option<Result<OpenLesson, String>>,
    pub quiz: Option<QuizRun>,
    /// Locked tracks the user expanded in the tree to preview.
    pub expanded: std::collections::HashSet<String>,
}

pub struct QuizRun {
    pub view: QuizView,
    pub index: usize,
    pub answers: Vec<Answer>,
    /// Whether the current question's answer was right, once given.
    pub feedback: Option<bool>,
    pub started: i64,
    pub result: Option<QuizResult>,
}

/// What the user picked or typed for a question being shown.
pub struct QuestionState<'a> {
    pub picked: Option<Answer>,
    pub feedback: Option<bool>,
    pub input: &'a str,
}

impl Shell {
    pub(crate) fn open_lesson(&mut self, id: &str) {
        self.lesson.current = Some(id.to_owned());
        self.lesson.open = None;
        self.lesson.quiz = None;
        self.input.clear();
        self.scroll_to_top();
    }

    fn ensure_open(&mut self) {
        if self.lesson.open.is_none()
            && let Some(id) = &self.lesson.current
        {
            let loc = self.ui.i18n.locale();
            self.lesson.open = Some(self.app.open_lesson(id, loc).map_err(|e| e.to_string()));
        }
    }

    fn start_quiz(&mut self) {
        let Some(id) = self.lesson.current.clone() else {
            return;
        };
        match self.app.quiz(&id, self.ui.i18n.locale()) {
            Ok(view) => {
                self.lesson.quiz = Some(QuizRun {
                    view,
                    index: 0,
                    answers: Vec::new(),
                    feedback: None,
                    started: self.app.now(),
                    result: None,
                });
                self.input.clear();
            }
            Err(e) => eprintln!("taful: quiz failed to load: {e}"),
        }
    }

    fn quiz_answer(&mut self, a: Answer) {
        let Some(run) = self.lesson.quiz.as_mut() else {
            return;
        };
        if run.feedback.is_some() || run.result.is_some() {
            return;
        }
        let Some(q) = run.view.questions.get(run.index) else {
            return;
        };
        run.feedback = Some(q.is_correct(a));
        run.answers.push(a);
    }

    fn quiz_next(&mut self) {
        let Some(id) = self.lesson.current.clone() else {
            return;
        };
        let now = self.app.now();
        let Some(run) = self.lesson.quiz.as_mut() else {
            return;
        };
        if run.feedback.is_none() {
            return;
        }
        run.index += 1;
        run.feedback = None;
        self.input.clear();
        if run.index >= run.view.questions.len() {
            let answers = run.answers.clone();
            let duration = now - run.started;
            match self.app.submit_quiz(&id, &answers, duration) {
                Ok(r) => {
                    if let Some(run) = self.lesson.quiz.as_mut() {
                        run.result = Some(r);
                    }
                    // State may have changed (done, unlocks).
                    self.lesson.open = None;
                }
                Err(e) => eprintln!("taful: quiz submit failed: {e}"),
            }
        }
    }

    pub(crate) fn quiz_key(&mut self, key: KeyResult, _cx: &mut Context<Self>) {
        let Some(run) = &self.lesson.quiz else {
            return;
        };
        if run.result.is_some() {
            return;
        }
        if !matches!(key, KeyResult::Submit) {
            return;
        }
        if run.feedback.is_some() {
            self.quiz_next();
            return;
        }
        let numeric = run
            .view
            .questions
            .get(run.index)
            .is_some_and(|q| q.numeric.is_some());
        if numeric && let Some(x) = fmt::parse_number(&self.input, self.ui.i18n.locale()) {
            self.quiz_answer(Answer::Numeric(x));
        }
    }

    /// One question with its choices or answer box and, once answered, the
    /// feedback. `on_choice` handles a click on choice `i`.
    pub(crate) fn question_view(
        &self,
        id_prefix: &'static str,
        q: &RenderedQuestion,
        st: QuestionState<'_>,
        on_choice: fn(&mut Shell, usize),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (ui, p) = (&self.ui, &self.palette);
        let t = &ui.theme;
        let mut col = div().flex().flex_col().gap(px(t.spacing.md)).child(
            div()
                .text_size(px(t.text_size.lg))
                .line_height(px(t.text_size.lg * 1.5))
                .child(q.prompt.clone()),
        );
        if q.numeric.is_some() {
            if st.feedback.is_none() {
                col = col.child(answer_box(st.input, ui.i18n.tr("quiz-type-answer"), ui, p));
            } else if let Some(Answer::Numeric(x)) = st.picked {
                col = col.child(muted(
                    ui.i18n.tr_args(
                        "calc-your-answer",
                        &[("value", fmt::number_trim(x, 2, ui.i18n.locale()).into())],
                    ),
                    ui,
                    p,
                ));
            }
        } else {
            for (i, c) in q.choices.iter().enumerate() {
                let picked = st.picked == Some(Answer::Choice(i));
                let right = q.answer_index == Some(i);
                let answered = st.feedback.is_some();
                col = col.child(
                    div()
                        .id((id_prefix, i))
                        .px(px(t.spacing.md))
                        .py(px(t.spacing.sm))
                        .rounded(px(t.radii.md))
                        .border_1()
                        .text_size(px(t.text_size.md))
                        .border_color(if answered && right {
                            p.accent
                        } else if picked {
                            p.text_muted
                        } else {
                            p.border
                        })
                        .bg(if picked { p.surface_raised } else { p.surface })
                        .when(answered && right, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .when(!answered, |d| {
                            d.cursor_pointer()
                                .hover(|s| s.border_color(p.accent))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    on_choice(this, i);
                                    window.focus(&this.focus);
                                    cx.notify();
                                }))
                        })
                        .child(c.clone()),
                );
            }
        }
        if let Some(ok) = st.feedback {
            col = col.child(
                div()
                    .text_size(px(t.text_size.md))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if ok { p.accent } else { p.text })
                    .child(if ok {
                        ui.i18n.tr("quiz-correct")
                    } else {
                        ui.i18n
                            .tr_args("quiz-wrong", &[("answer", q.answer_text().into())])
                    }),
            );
            if let Some(e) = &q.explanation {
                col = col.child(muted(e.clone(), ui, p));
            }
        }
        col.into_any_element()
    }

    fn quiz_block(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let run = self.lesson.quiz.as_ref()?;
        let (ui, p) = (&self.ui, &self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let pct = |x: f64| format!("{}%", fmt::number(x * 100.0, 0, loc));
        let mut c = card(ui, p).child(title(ui.i18n.tr("quiz-title"), ui));
        if let Some(r) = &run.result {
            c = c.child(div().text_size(px(t.text_size.lg)).child(if r.passed {
                ui.i18n
                    .tr_args("quiz-passed", &[("score", pct(r.score).into())])
            } else {
                ui.i18n.tr_args(
                    "quiz-failed",
                    &[
                        ("score", pct(r.score).into()),
                        ("pass", pct(run.view.pass).into()),
                    ],
                )
            }));
            if !r.unlocked.is_empty() {
                let names: Vec<String> = r
                    .unlocked
                    .iter()
                    .filter_map(|id| {
                        self.app
                            .library()
                            .lesson(id, loc.tag())
                            .map(|l| l.meta.title.clone())
                    })
                    .collect();
                c = c.child(muted(
                    ui.i18n
                        .tr_args("quiz-unlocked", &[("lessons", names.join(", ").into())]),
                    ui,
                    p,
                ));
            }
            c = c.child(
                div()
                    .flex()
                    .gap(px(t.spacing.sm))
                    .child(
                        button("quiz_tree", ui.i18n.tr("lesson-back-to-tree"), true, ui, p)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select(Screen::SkillTree, window, cx)
                            })),
                    )
                    .when(!r.passed, |d| {
                        d.child(
                            button("quiz_retry", ui.i18n.tr("lesson-retake-quiz"), false, ui, p)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.start_quiz();
                                    window.focus(&this.focus);
                                    cx.notify();
                                })),
                        )
                    }),
            );
            return Some(c.into_any_element());
        }
        let q = run.view.questions.get(run.index)?;
        let last = run.index + 1 == run.view.questions.len();
        c = c
            .child(muted(
                ui.i18n.tr_args(
                    "quiz-progress",
                    &[
                        ("n", (run.index + 1).into()),
                        ("total", run.view.questions.len().into()),
                    ],
                ),
                ui,
                p,
            ))
            .child(self.question_view(
                "quiz_choice",
                q,
                QuestionState {
                    picked: run.feedback.and(run.answers.last().copied()),
                    feedback: run.feedback,
                    input: &self.input,
                },
                |this, i| this.quiz_answer(Answer::Choice(i)),
                cx,
            ));
        if run.feedback.is_some() {
            let label = if last { "quiz-finish" } else { "quiz-next" };
            c = c.child(
                button("quiz-next", ui.i18n.tr(label), true, ui, p).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.quiz_next();
                        window.focus(&this.focus);
                        cx.notify();
                    },
                )),
            );
        } else if q.numeric.is_some() {
            c = c.child(
                button("quiz-check", ui.i18n.tr("quiz-check"), true, ui, p).on_click(cx.listener(
                    |this, _, window, cx| {
                        if let Some(x) = fmt::parse_number(&this.input, this.ui.i18n.locale()) {
                            this.quiz_answer(Answer::Numeric(x));
                        }
                        window.focus(&this.focus);
                        cx.notify();
                    },
                )),
            );
        }
        Some(c.into_any_element())
    }

    pub(crate) fn lesson_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.lesson.current.is_none() {
            // Default to the next lesson to read.
            let loc = self.ui.i18n.locale();
            if let Ok(h) = self.app.home(loc)
                && let Some((id, _)) = h.next_lesson
            {
                self.open_lesson(&id);
            }
        }
        self.ensure_open();
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let open = match &self.lesson.open {
            None => {
                return card(&ui, &p)
                    .child(muted(ui.i18n.tr("lesson-none"), &ui, &p))
                    .child(
                        button("lesson_tree", ui.i18n.tr("lesson-open-tree"), true, &ui, &p)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select(Screen::SkillTree, window, cx)
                            })),
                    )
                    .into_any_element();
            }
            Some(Err(e)) => {
                return card(&ui, &p)
                    .child(
                        ui.i18n
                            .tr_args("lesson-render-error", &[("error", e.clone().into())]),
                    )
                    .into_any_element();
            }
            Some(Ok(o)) => o.clone(),
        };
        let mut col = div().flex().flex_col().gap(px(t.spacing.lg)).child(
            div()
                .id("lesson_back")
                .text_size(px(t.text_size.md))
                .text_color(p.accent)
                .cursor_pointer()
                .hover(|s| s.opacity(0.8))
                .on_click(
                    cx.listener(|this, _, window, cx| this.select(Screen::SkillTree, window, cx)),
                )
                .child(ui.i18n.tr("lesson-back-to-tree")),
        );
        if open.state == LessonState::Locked {
            col = col.child(muted(ui.i18n.tr("lesson-locked-note"), &ui, &p));
        }
        col = col.child(card(&ui, &p).child(document(&open.doc, &ui, &p)));
        if !open.glossary.is_empty() {
            let mut g = card(&ui, &p).child(title(ui.i18n.tr("lesson-glossary"), &ui));
            for term in &open.glossary {
                g = g.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(t.text_size.md))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.accent)
                                .child(ui.i18n.tr(&format!("glossary-{term}"))),
                        )
                        .child(muted(ui.i18n.tr(&format!("glossary-{term}-def")), &ui, &p)),
                );
            }
            col = col.child(g);
        }
        match self.quiz_block(cx) {
            Some(q) => col = col.child(q),
            None if open.state != LessonState::Locked => {
                let label = if open.state == LessonState::Done {
                    "lesson-retake-quiz"
                } else {
                    "lesson-take-quiz"
                };
                col = col.child(
                    button("lesson_quiz", ui.i18n.tr(label), true, &ui, &p).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.start_quiz();
                            window.focus(&this.focus);
                            cx.notify();
                        },
                    )),
                );
            }
            None => {}
        }
        col.into_any_element()
    }
}

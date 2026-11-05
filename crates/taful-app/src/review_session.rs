//! The daily review session (spec §13, §15.1): the due items of every kind in
//! one queue, earliest first.
//!
//! Versioning (§12.4): a quiz item keeps the version it was created from
//! until its next review, which moves it to the quiz's current version. An
//! item whose question no longer exists is retired (soft-deleted).
//!
//! Spot-template items arrive with the strategy library (M3/M4); until then
//! the session skips them.

use taful_content::quiz::{Answer, RenderedQuestion};
use taful_core::calc::ExerciseKind;
use taful_core::srs::Rating;

use crate::calc_trainer::CalcRound;
use crate::i18n::Locale;
use crate::review::ItemKind;
use crate::{App, Error, Result};

/// The next thing to review.
#[derive(Debug, Clone, PartialEq)]
pub enum ReviewPrompt {
    /// A calc exercise in the item's kind and band. Answer with
    /// [`App::calc_submit`], which reviews the item.
    Calc { item_id: String, round: CalcRound },
    /// A quiz question. Answer with [`App::answer_quiz_review`].
    Quiz {
        item_id: String,
        lesson_id: String,
        lesson_title: String,
        question: RenderedQuestion,
    },
}

/// Item kinds this build can present (spot templates arrive with M4).
pub(crate) const PRESENTABLE: &[&str] = &["calc_exercise", "lesson_quiz"];

impl App {
    /// Number of items due now that this build can present.
    pub fn due_count(&self) -> Result<u32> {
        Ok(self.store.count_due(self.now(), PRESENTABLE)?)
    }

    /// The earliest due item this build can present, or `None` when the
    /// queue is clear for now.
    pub fn next_review(&mut self, locale: Locale) -> Result<Option<ReviewPrompt>> {
        let now = self.now();
        // Bounded: invalid items found on the way are deleted, so a later
        // call moves past them.
        for item in self.store.due_items(now, PRESENTABLE, 100)? {
            match ItemKind::parse(&item.kind) {
                Some(ItemKind::CalcExercise) => {
                    let Some((kind, band)) = item.key.split_once('/') else {
                        self.store.delete_review_item(&item.id)?;
                        continue;
                    };
                    let Some(kind) = ExerciseKind::ALL.into_iter().find(|k| k.id() == kind) else {
                        self.store.delete_review_item(&item.id)?;
                        continue;
                    };
                    let Some(round) = self.calc_start_in_band(kind, band) else {
                        // The band no longer exists (generator changed).
                        self.store.delete_review_item(&item.id)?;
                        continue;
                    };
                    return Ok(Some(ReviewPrompt::Calc {
                        item_id: item.id,
                        round,
                    }));
                }
                Some(ItemKind::LessonQuiz) => match self.quiz_question(&item.key, locale)? {
                    Some((lesson_id, lesson_title, question)) => {
                        return Ok(Some(ReviewPrompt::Quiz {
                            item_id: item.id,
                            lesson_id,
                            lesson_title,
                            question,
                        }));
                    }
                    None => {
                        self.store.delete_review_item(&item.id)?;
                        continue;
                    }
                },
                Some(ItemKind::SpotTemplate) | None => continue, // not queried
            }
        }
        Ok(None)
    }

    /// `(lesson id, lesson title, question)` for a quiz item key.
    fn quiz_question(
        &self,
        key: &str,
        locale: Locale,
    ) -> Result<Option<(String, String, RenderedQuestion)>> {
        let Some((quiz_id, qid)) = key.split_once('/') else {
            return Ok(None);
        };
        let Some(quiz) = self.library.quizzes.get(quiz_id) else {
            return Ok(None);
        };
        let Some(q) = quiz.question(qid) else {
            return Ok(None);
        };
        let rendered = quiz
            .render_question(q, locale.tag(), None)
            .map_err(Error::Content)?;
        let title = self
            .library
            .lesson(&quiz.lesson, locale.tag())
            .map(|l| l.meta.title.clone())
            .unwrap_or_default();
        Ok(Some((quiz.lesson.clone(), title, rendered)))
    }

    /// Grades a quiz review and reschedules the item. Returns whether the
    /// answer was right.
    pub fn answer_quiz_review(&mut self, item_id: &str, answer: Answer) -> Result<bool> {
        let item = self
            .store
            .review_item_by_id(item_id)?
            .ok_or_else(|| Error::NotFound(format!("review item `{item_id}`")))?;
        let (quiz_id, _) = item
            .key
            .split_once('/')
            .ok_or_else(|| Error::NotFound(item.key.clone()))?;
        let version = self.library.quizzes.get(quiz_id).map(|q| q.version);
        let (_, _, q) = self
            .quiz_question(&item.key, Locale::SOURCE)?
            .ok_or_else(|| Error::NotFound(item.key.clone()))?;
        let ok = q.is_correct(answer);
        self.scheduler
            .review_item(&self.store, &item, Rating::from_quiz(ok), version, None)?;
        Ok(ok)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::review::RELEARN_MS;
    use crate::services::tests::app_at;

    #[test]
    fn session_mixes_kinds_and_reschedules() {
        let (mut app, t) = app_at(10_000_000);
        assert_eq!(app.next_review(Locale::PtBr).unwrap(), None);

        // A wrong calc answer and a wrong quiz answer.
        let round = app.calc_start(Some(ExerciseKind::Mdf));
        app.calc_submit(&round, Some(-5.0)).unwrap();
        let lesson = app.library.tree.order()[0].to_owned();
        let view = app.quiz(&lesson, Locale::PtBr).unwrap();
        let mut answers: Vec<Answer> = view
            .questions
            .iter()
            .map(|q| match (q.answer_index, &q.numeric) {
                (Some(i), _) => Answer::Choice(i),
                (_, Some((v, _))) => Answer::Numeric(*v),
                _ => unreachable!(),
            })
            .collect();
        let wrong = match answers[0] {
            Answer::Choice(i) => Answer::Choice(i + 1),
            Answer::Numeric(x) => Answer::Numeric(x + 1000.0),
        };
        let right = answers[0];
        answers[0] = wrong;
        app.submit_quiz(&lesson, &answers, 1_000).unwrap();

        // Nothing due until the relearn delay passes.
        assert_eq!(app.due_count().unwrap(), 0);
        t.fetch_add(RELEARN_MS, Ordering::SeqCst);
        assert_eq!(app.due_count().unwrap(), 2);

        let mut seen = Vec::new();
        while let Some(p) = app.next_review(Locale::PtBr).unwrap() {
            match p {
                ReviewPrompt::Calc { round, .. } => {
                    assert_eq!(round.exercise.kind, ExerciseKind::Mdf);
                    app.calc_submit(&round, Some(round.exercise.answer))
                        .unwrap();
                    seen.push("calc");
                }
                ReviewPrompt::Quiz {
                    item_id, question, ..
                } => {
                    assert!(!question.prompt.is_empty());
                    assert!(app.answer_quiz_review(&item_id, right).unwrap());
                    seen.push("quiz");
                }
            }
            assert!(seen.len() <= 2, "answered items leave the session");
        }
        seen.sort();
        assert_eq!(seen, ["calc", "quiz"]);
        assert_eq!(app.due_count().unwrap(), 0);
    }

    #[test]
    fn retired_questions_leave_the_queue() {
        let (mut app, _) = app_at(0);
        app.store
            .ensure_review_item("lesson_quiz", "no-such-quiz/q", Some(1), 0)
            .unwrap();
        app.store
            .ensure_review_item("spot_template", "srp/x", None, 0)
            .unwrap();
        app.store
            .ensure_review_item("calc_exercise", "pot_odds/bet_7", None, 0)
            .unwrap();
        assert_eq!(app.due_count().unwrap(), 2, "spot items are not counted");
        assert_eq!(app.next_review(Locale::PtBr).unwrap(), None);
        assert_eq!(app.store.review_totals().unwrap().0, 1, "spot item kept");
        assert_eq!(app.due_count().unwrap(), 0);
    }
}

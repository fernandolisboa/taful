//! Skill tree, lessons and quizzes (spec §12, §15.1).
//!
//! Lock state: a lesson is **done** once its quiz was passed at any version,
//! **available** when every prerequisite is done, otherwise **locked**.
//! Progress rows are keyed by `(lesson id, version)` (§12.4), so a content
//! update that bumps a version shows the lesson as done (from the older
//! version) while the new version's quiz is still there to take.

use std::collections::BTreeSet;

use taful_content::lesson::Document;
use taful_content::quiz::{Answer, Quiz, RenderedQuestion};
use taful_core::srs::Rating;
use taful_store::{LessonState, NewQuizAttempt};

use crate::i18n::Locale;
use crate::review::{ItemKind, Source};
use crate::{App, Error, Result};

/// A lesson as the skill tree shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonCard {
    pub id: String,
    pub title: String,
    pub state: LessonState,
    pub minutes: Option<u32>,
    pub best_score: Option<f64>,
}

/// A track with its lessons in order. Title: Fluent `track-<id>`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackView {
    pub id: String,
    pub lessons: Vec<LessonCard>,
}

/// A lesson opened for reading.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenLesson {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub state: LessonState,
    pub doc: Document,
    /// Glossary term ids; strings are Fluent `glossary-<id>` / `-def`.
    pub glossary: Vec<String>,
}

/// A quiz rendered for one locale.
#[derive(Debug, Clone, PartialEq)]
pub struct QuizView {
    pub id: String,
    pub version: u32,
    pub pass: f64,
    pub questions: Vec<RenderedQuestion>,
}

/// The result of a finished quiz.
#[derive(Debug, Clone, PartialEq)]
pub struct QuizResult {
    pub score: f64,
    pub passed: bool,
    pub correct: Vec<bool>,
    /// Lessons this pass made available.
    pub unlocked: Vec<String>,
}

/// Review-item key of a quiz question.
pub fn question_key(quiz_id: &str, question_id: &str) -> String {
    format!("{quiz_id}/{question_id}")
}

impl App {
    fn done_lessons(&self) -> Result<BTreeSet<String>> {
        Ok(self
            .store
            .lesson_progress()?
            .into_iter()
            .filter(|p| p.state == LessonState::Done)
            .map(|p| p.lesson_id)
            .collect())
    }

    fn state_of(&self, id: &str, done: &BTreeSet<String>) -> LessonState {
        if done.contains(id) {
            return LessonState::Done;
        }
        let Some(l) = self.library.lesson(id, taful_content::SOURCE_LOCALE) else {
            return LessonState::Locked;
        };
        if l.meta.prerequisites.iter().all(|p| done.contains(p)) {
            LessonState::Available
        } else {
            LessonState::Locked
        }
    }

    /// The lock state of one lesson.
    pub fn lesson_state(&self, id: &str) -> Result<LessonState> {
        Ok(self.state_of(id, &self.done_lessons()?))
    }

    /// Every track and lesson with its state, in tree order.
    pub fn skill_tree(&self, locale: Locale) -> Result<Vec<TrackView>> {
        let done = self.done_lessons()?;
        let progress = self.store.lesson_progress()?;
        let best = |id: &str| {
            progress
                .iter()
                .filter(|p| p.lesson_id == id)
                .filter_map(|p| p.best_score)
                .fold(None, |acc: Option<f64>, s| {
                    Some(acc.map_or(s, |a| a.max(s)))
                })
        };
        Ok(self
            .library
            .tree
            .tracks
            .iter()
            .map(|t| TrackView {
                id: t.id.clone(),
                lessons: t
                    .lessons
                    .iter()
                    .filter_map(|id| {
                        let l = self.library.lesson(id, locale.tag())?;
                        Some(LessonCard {
                            id: id.clone(),
                            title: l.meta.title.clone(),
                            state: self.state_of(id, &done),
                            minutes: l.meta.minutes,
                            best_score: best(id),
                        })
                    })
                    .collect(),
            })
            .collect())
    }

    /// Renders a lesson for reading. Locked lessons can be previewed; the
    /// state says so.
    pub fn open_lesson(&self, id: &str, locale: Locale) -> Result<OpenLesson> {
        let l = self
            .library
            .lesson(id, locale.tag())
            .ok_or_else(|| Error::NotFound(format!("lesson `{id}`")))?;
        let doc = l.render(None).map_err(Error::Content)?;
        let state = self.lesson_state(id)?;
        if state == LessonState::Available {
            self.store
                .record_lesson_progress(id, l.meta.version, LessonState::Available, None)?;
        }
        Ok(OpenLesson {
            id: id.to_owned(),
            version: l.meta.version,
            title: l.meta.title.clone(),
            state,
            doc,
            glossary: l.meta.glossary.clone(),
        })
    }

    fn quiz_of(&self, lesson_id: &str) -> Result<&Quiz> {
        self.library
            .quiz_for(lesson_id)
            .ok_or_else(|| Error::NotFound(format!("quiz for lesson `{lesson_id}`")))
    }

    /// The lesson's quiz, rendered.
    pub fn quiz(&self, lesson_id: &str, locale: Locale) -> Result<QuizView> {
        let quiz = self.quiz_of(lesson_id)?;
        let questions = quiz
            .questions
            .iter()
            .map(|q| quiz.render_question(q, locale.tag(), None))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::Content)?;
        Ok(QuizView {
            id: quiz.id.clone(),
            version: quiz.version,
            pass: quiz.pass,
            questions,
        })
    }

    /// Grades a finished quiz, stores the attempt, records progress and
    /// feeds the review queue: a wrong answer creates (or reviews) the
    /// question's item as Again; a right answer reviews an existing item as
    /// Good.
    pub fn submit_quiz(
        &mut self,
        lesson_id: &str,
        answers: &[Answer],
        duration_ms: i64,
    ) -> Result<QuizResult> {
        let view = self.quiz(lesson_id, Locale::SOURCE)?;
        if answers.len() != view.questions.len() {
            return Err(Error::NotFound(format!(
                "{} answers for {} questions",
                answers.len(),
                view.questions.len()
            )));
        }
        let correct: Vec<bool> = view
            .questions
            .iter()
            .zip(answers)
            .map(|(q, a)| q.is_correct(*a))
            .collect();
        let score = Quiz::score(&view.questions, answers);
        let passed = score + 1e-9 >= view.pass;
        let attempt_id = self.store.insert_quiz_attempt(&NewQuizAttempt {
            quiz_id: &view.id,
            version: view.version,
            answers: serde_json::to_value(answers).expect("answers serialize"),
            score,
            duration_ms,
        })?;
        for (q, ok) in view.questions.iter().zip(&correct) {
            let key = question_key(&view.id, &q.id);
            let exists = self
                .store
                .review_item(ItemKind::LessonQuiz.id(), &key)?
                .is_some();
            if !*ok || exists {
                self.scheduler.review(
                    &self.store,
                    ItemKind::LessonQuiz,
                    &key,
                    Some(view.version),
                    Rating::from_quiz(*ok),
                    Some(Source {
                        kind: "quiz_attempt",
                        id: &attempt_id,
                    }),
                )?;
            }
        }
        let version = self
            .library
            .lesson(lesson_id, taful_content::SOURCE_LOCALE)
            .map_or(1, |l| l.meta.version);
        let before = self.skill_tree_states()?;
        self.store.record_lesson_progress(
            lesson_id,
            version,
            if passed {
                LessonState::Done
            } else {
                LessonState::Available
            },
            Some(score),
        )?;
        let after = self.skill_tree_states()?;
        let unlocked = after
            .into_iter()
            .filter(|(id, s)| {
                *s == LessonState::Available
                    && before
                        .iter()
                        .any(|(b, bs)| b == id && *bs == LessonState::Locked)
            })
            .map(|(id, _)| id)
            .collect();
        Ok(QuizResult {
            score,
            passed,
            correct,
            unlocked,
        })
    }

    fn skill_tree_states(&self) -> Result<Vec<(String, LessonState)>> {
        let done = self.done_lessons()?;
        Ok(self
            .library
            .tree
            .order()
            .into_iter()
            .map(|id| (id.to_owned(), self.state_of(id, &done)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tests::app_at;

    fn right_answers(view: &QuizView) -> Vec<Answer> {
        view.questions
            .iter()
            .map(|q| match (q.answer_index, &q.numeric) {
                (Some(i), _) => Answer::Choice(i),
                (_, Some((v, _))) => Answer::Numeric(*v),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn first_lesson_is_available_and_passing_unlocks_the_next() {
        let (mut app, _) = app_at(1_000);
        let tree = app.skill_tree(Locale::PtBr).unwrap();
        let first = &tree[0].lessons[0];
        assert_eq!(first.state, LessonState::Available);
        assert!(
            tree.iter()
                .flat_map(|t| &t.lessons)
                .skip(1)
                .any(|l| l.state == LessonState::Locked)
        );

        let open = app.open_lesson(&first.id, Locale::PtBr).unwrap();
        assert!(!open.doc.blocks.is_empty());
        let view = app.quiz(&first.id, Locale::PtBr).unwrap();
        let res = app
            .submit_quiz(&first.id, &right_answers(&view), 30_000)
            .unwrap();
        assert!(res.passed && res.score == 1.0);
        assert!(!res.unlocked.is_empty());
        assert_eq!(app.lesson_state(&first.id).unwrap(), LessonState::Done);
        assert_eq!(app.store.review_totals().unwrap(), (0, 0));
        let home = app.home(Locale::PtBr).unwrap();
        assert_eq!(home.lessons_done, 1);
        assert_eq!(home.next_lesson.unwrap().0, res.unlocked[0]);
    }

    #[test]
    fn wrong_answers_land_in_the_queue() {
        let (mut app, _) = app_at(1_000);
        let id = app.skill_tree(Locale::EnUs).unwrap()[0].lessons[0]
            .id
            .clone();
        let view = app.quiz(&id, Locale::EnUs).unwrap();
        let mut answers = right_answers(&view);
        answers[0] = match answers[0] {
            Answer::Choice(i) => Answer::Choice(i + 1),
            Answer::Numeric(x) => Answer::Numeric(x + 1000.0),
        };
        let res = app.submit_quiz(&id, &answers, 10_000).unwrap();
        assert!(!res.correct[0]);
        let key = question_key(&view.id, &view.questions[0].id);
        let item = app.store.review_item("lesson_quiz", &key).unwrap().unwrap();
        assert_eq!(item.content_version, Some(view.version));
        assert!(
            app.submit_quiz(&id, &[], 1).is_err(),
            "answer count must match"
        );
    }

    #[test]
    fn every_lesson_and_quiz_renders_in_both_locales() {
        let (app, _) = app_at(1_000);
        for id in app.library.tree.order() {
            for loc in Locale::ALL {
                app.open_lesson(id, loc).unwrap();
                app.quiz(id, loc).unwrap();
            }
        }
    }
}

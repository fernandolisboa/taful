//! The daily review session (spec §13, §15.1): the due items of every kind in
//! one queue, earliest first.
//!
//! Versioning (§12.4): a quiz item keeps the version it was created from
//! until its next review, which moves it to the quiz's current version. An
//! item whose question no longer exists is retired (soft-deleted).
//!
//! Spot-template items (M5) deal a fresh spot of their template. A postflop
//! template waits (stays due, not shown) while the postflop library is not
//! installed.

use std::sync::Arc;

use taful_content::quiz::{Answer, RenderedQuestion};
use taful_core::calc::ExerciseKind;
use taful_core::srs::Rating;
use taful_store::{Hold, ReviewItem};

use crate::calc_trainer::CalcRound;
use crate::i18n::Locale;
use crate::preflop::{Action, GradedDecision};
use crate::review::ItemKind;
use crate::template::{KEY_GLOB, PREFLOP_KEY_GLOB, SpotTemplate};
use crate::training::{DrillSpot, TemplateError, Trainers};
use crate::{App, Error, Result};

/// The next thing to review.
#[derive(Debug, Clone)]
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
    /// A spot of a past mistake's template. Answer with
    /// [`App::answer_spot_review`].
    Spot {
        item_id: String,
        template: SpotTemplate,
        spot: Box<DrillSpot>,
    },
}

/// One step of [`App::next_review_step`].
#[derive(Debug)]
pub enum ReviewStep {
    /// Show this.
    Ready(ReviewPrompt),
    /// Deal a spot template, then call [`App::finish_review_deal`].
    Deal(TemplateDeal),
    /// Nothing due that can be shown now.
    Empty,
}

/// A spot-template deal for a review. Owns everything it needs, so it can
/// run on any thread; river templates solve the river (spec §7.7).
#[derive(Debug)]
pub struct TemplateDeal {
    pub item_id: String,
    pub template: SpotTemplate,
    trainers: Arc<Trainers>,
    seeds: [u64; TEMPLATE_TRIES],
}

impl TemplateDeal {
    /// Tries a few seeds; `Unavailable` if none dealt.
    pub fn run(&self) -> std::result::Result<DrillSpot, TemplateError> {
        let mut dealt = Err(TemplateError::Unavailable);
        for &seed in &self.seeds {
            dealt = self.trainers.spot_for_template(&self.template, seed);
            if !matches!(dealt, Err(TemplateError::Unavailable)) {
                break;
            }
        }
        dealt
    }
}

/// Item kinds the review session presents.
pub(crate) const PRESENTABLE: &[&str] = &["calc_exercise", "lesson_quiz", SPOT_TEMPLATE];
const SPOT_TEMPLATE: &str = "spot_template";
/// Deals tried for a spot template before it is skipped for now.
const TEMPLATE_TRIES: usize = 3;

/// Due items read per [`App::next_review`] call.
const SCAN: u32 = 100;

impl App {
    /// Number of items due now that this build can present.
    pub fn due_count(&mut self) -> Result<u32> {
        let now = self.now();
        let hold = self.postflop_hold();
        Ok(self.store.count_due_held(now, PRESENTABLE, hold.as_ref())?)
    }

    fn has_postflop(&mut self) -> bool {
        self.trainers().is_ok_and(|t| t.post().is_some())
    }

    /// Postflop spot items wait while no library is installed.
    fn postflop_hold(&mut self) -> Option<Hold<'static>> {
        (!self.has_postflop()).then_some(Hold {
            kind: SPOT_TEMPLATE,
            key_glob: KEY_GLOB,
            unless_glob: PREFLOP_KEY_GLOB,
        })
    }

    /// The earliest due items this build can present now, earliest first.
    fn presentable_due(&mut self, limit: u32) -> Result<Vec<ReviewItem>> {
        let now = self.now();
        let hold = self.postflop_hold();
        Ok(self
            .store
            .due_items_held(now, PRESENTABLE, hold.as_ref(), limit)?)
    }

    /// The earliest due item this build can present, or `None` when the
    /// queue is clear for now. Deals spot templates on the calling thread;
    /// the desktop uses [`App::next_review_step`] to deal in the background.
    pub fn next_review(&mut self, locale: Locale) -> Result<Option<ReviewPrompt>> {
        let mut skip = Vec::new();
        loop {
            match self.next_review_step(locale, &skip)? {
                ReviewStep::Ready(p) => return Ok(Some(p)),
                ReviewStep::Empty => return Ok(None),
                ReviewStep::Deal(deal) => {
                    let outcome = deal.run();
                    let id = deal.item_id.clone();
                    if let Some(p) = self.finish_review_deal(deal, outcome)? {
                        return Ok(Some(p));
                    }
                    skip.push(id);
                }
            }
        }
    }

    /// One step towards the next review: a prompt that is ready, a spot
    /// template deal to run (it can solve rivers, so the UI runs
    /// [`TemplateDeal::run`] off its thread and passes the result to
    /// [`App::finish_review_deal`]), or an empty queue. Items in `skip`
    /// (deals that came up empty this time) are passed over.
    pub fn next_review_step(&mut self, locale: Locale, skip: &[String]) -> Result<ReviewStep> {
        // Bounded: invalid items found on the way are deleted, so a later
        // call moves past them.
        for item in self.presentable_due(SCAN)? {
            if skip.contains(&item.id) {
                continue;
            }
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
                    return Ok(ReviewStep::Ready(ReviewPrompt::Calc {
                        item_id: item.id,
                        round,
                    }));
                }
                Some(ItemKind::LessonQuiz) => match self.quiz_question(&item.key, locale)? {
                    Some((lesson_id, lesson_title, question)) => {
                        return Ok(ReviewStep::Ready(ReviewPrompt::Quiz {
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
                Some(ItemKind::SpotTemplate) => {
                    let Some(template) = SpotTemplate::parse(&item.key) else {
                        self.store.delete_review_item(&item.id)?;
                        continue;
                    };
                    let trainers = self.trainers()?;
                    // A few fresh seeds: a deal can miss by bad luck.
                    let seeds = std::array::from_fn(|_| self.next_seed());
                    return Ok(ReviewStep::Deal(TemplateDeal {
                        item_id: item.id,
                        template,
                        trainers,
                        seeds,
                    }));
                }
                None => continue,
            }
        }
        Ok(ReviewStep::Empty)
    }

    /// Takes the result of a [`TemplateDeal`]: the prompt, or `None` when
    /// the deal came up empty (the item stays due) or the template is gone
    /// (the item is retired). On `None`, ask [`App::next_review_step`] again
    /// with the item in `skip`.
    pub fn finish_review_deal(
        &mut self,
        deal: TemplateDeal,
        outcome: std::result::Result<DrillSpot, TemplateError>,
    ) -> Result<Option<ReviewPrompt>> {
        match outcome {
            Ok(spot) => Ok(Some(ReviewPrompt::Spot {
                item_id: deal.item_id,
                template: deal.template,
                spot: Box::new(spot),
            })),
            // Other configuration, chart or library rebuilt.
            Err(TemplateError::Gone) => {
                self.store.delete_review_item(&deal.item_id)?;
                Ok(None)
            }
            Err(TemplateError::Unavailable) => Ok(None),
        }
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

    /// Grades a spot review, stores the decision and reschedules the item.
    pub fn answer_spot_review(
        &mut self,
        item_id: &str,
        spot: &DrillSpot,
        action: Action,
    ) -> Result<GradedDecision> {
        let trainers = self.trainers()?;
        let d = trainers
            .answer(spot, action)
            .map_err(|e| Error::NotFound(e.to_string()))?;
        let ctx = spot.context(&trainers);
        self.record_spot_decision(&d, &ctx, Some(item_id))?;
        Ok(d)
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
        assert!(app.next_review(Locale::PtBr).unwrap().is_none());

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
                ReviewPrompt::Spot { .. } => unreachable!("no spot items"),
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
        assert_eq!(app.due_count().unwrap(), 3);
        assert!(app.next_review(Locale::PtBr).unwrap().is_none());
        assert_eq!(app.store.review_totals().unwrap().0, 0, "all retired");
        assert_eq!(app.due_count().unwrap(), 0);
    }

    #[test]
    fn postflop_items_wait_without_hiding_others() {
        let (mut app, _) = app_at(1_000);
        assert!(app.trainers().unwrap().post().is_none(), "no library");
        for i in 0..(SCAN + 20) {
            let key =
                format!("v2|c1c6782d35338294|btn-bb-srp|BB:x,BTN:b{i}/|A-2t-u-1|BB|reference");
            app.store
                .ensure_review_item("spot_template", &key, None, 0)
                .unwrap();
        }
        app.store
            .ensure_review_item("calc_exercise", "pot_odds/bet_50", None, 500)
            .unwrap();
        assert_eq!(app.due_count().unwrap(), 1, "postflop items wait");
        let Some(ReviewPrompt::Calc { .. }) = app.next_review(Locale::PtBr).unwrap() else {
            panic!("calc prompt expected");
        };
        assert_eq!(
            app.store.review_totals().unwrap().0,
            SCAN + 21,
            "none retired"
        );
        // Preflop templates and unknown keys still count; the library
        // brings the postflop ones back.
        let pre = format!(
            "v2|{}|preflop|UTG:f|-|HJ|reference",
            app.trainers().unwrap().config_key()
        );
        app.store
            .ensure_review_item("spot_template", &pre, None, 0)
            .unwrap();
        app.store
            .ensure_review_item("spot_template", "v0|old", None, 0)
            .unwrap();
        assert_eq!(app.due_count().unwrap(), 3);
        app.set_library_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postflop"),
        );
        assert_eq!(app.due_count().unwrap(), SCAN + 23);
    }

    #[test]
    fn spot_items_deal_their_template_and_reschedule() {
        let (mut app, t) = app_at(0);
        let trainers = app.trainers().unwrap();
        // Fold a hand UTG should open: a mistake, so the template gets an item.
        let tpl = SpotTemplate::parse(&format!(
            "v2|{}|preflop||-|UTG|reference",
            trainers.config_key()
        ))
        .unwrap();
        let (d, ctx) = (0..200)
            .find_map(|seed| {
                let spot = trainers.spot_for_template(&tpl, seed).unwrap();
                let d = trainers.answer(&spot, Action::Fold).unwrap();
                (d.grade > taful_core::grading::Grade::Correct)
                    .then(|| (d, spot.context(&trainers)))
            })
            .unwrap();
        assert_eq!(ctx.template, tpl);
        app.record_spot_decision(&d, &ctx, None).unwrap();
        assert!(
            app.store
                .review_item("spot_template", &ctx.template.key())
                .unwrap()
                .is_some()
        );
        t.fetch_add(RELEARN_MS, Ordering::SeqCst);
        let Some(ReviewPrompt::Spot {
            item_id,
            template,
            spot,
        }) = app.next_review(Locale::PtBr).unwrap()
        else {
            panic!("spot prompt expected");
        };
        assert_eq!(template, ctx.template);
        let acts = spot.actions(&trainers);
        let first = trainers.answer(&spot, acts[0]).unwrap();
        let best = acts[first.best()];
        let g = app.answer_spot_review(&item_id, &spot, best).unwrap();
        assert_eq!(g.ev_loss_bb, 0.0);
        assert_eq!(app.due_count().unwrap(), 0, "rescheduled");
    }
}

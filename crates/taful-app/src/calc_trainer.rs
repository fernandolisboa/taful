//! Mental-math trainer (spec §11): timed rounds, every attempt stored, slow
//! or wrong answers land in the review queue (D6).

use std::time::Duration;

use taful_core::calc::{self, Exercise, ExerciseKind};
use taful_core::srs::Rating;
use taful_store::{CalcStats, Millis, NewCalcAttempt};

use crate::review::{ItemKind, Source, rating_id};
use crate::{App, Result};

/// One exercise in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcRound {
    pub exercise: Exercise,
    pub started_at: Millis,
}

/// The graded answer.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcOutcome {
    pub correct: bool,
    /// `None` when skipped.
    pub given: Option<f64>,
    pub exact: f64,
    pub shortcut: f64,
    pub tolerance: f64,
    pub elapsed: Duration,
    pub target: Duration,
    pub rating: Rating,
    /// Whether the review queue now holds this exercise's skill.
    pub queued: bool,
}

/// Review-item key of an exercise: kind and parameter band (§13).
pub fn item_key(e: &Exercise) -> String {
    format!("{}/{}", e.kind.id(), e.band)
}

impl App {
    /// Starts a round of `kind`, or of a random kind when `None` (mixed).
    pub fn calc_start(&mut self, kind: Option<ExerciseKind>) -> CalcRound {
        let seed = self.next_seed();
        let kind =
            kind.unwrap_or(ExerciseKind::ALL[(seed % ExerciseKind::ALL.len() as u64) as usize]);
        CalcRound {
            exercise: calc::generate(kind, seed >> 8),
            started_at: self.now(),
        }
    }

    /// Starts a round of `kind` in parameter `band` (review items keep their
    /// band), or `None` when the band does not come up in a bounded search
    /// (every kind has a handful of bands, so a live band takes a few tries).
    pub fn calc_start_in_band(&mut self, kind: ExerciseKind, band: &str) -> Option<CalcRound> {
        for _ in 0..200 {
            let seed = self.next_seed();
            let e = calc::generate(kind, seed);
            if e.band == band {
                return Some(CalcRound {
                    exercise: e,
                    started_at: self.now(),
                });
            }
        }
        None
    }

    /// Grades `given` (in the exercise's unit; `None` = skipped), stores the
    /// attempt, and updates the review queue: an existing item for the skill
    /// is reviewed; a new one is created when the answer was wrong or slow.
    pub fn calc_submit(&mut self, round: &CalcRound, given: Option<f64>) -> Result<CalcOutcome> {
        let e = &round.exercise;
        let now = self.now();
        let elapsed = Duration::from_millis((now - round.started_at).max(0) as u64);
        let target = e.target(&self.calc);
        let correct = given.is_some_and(|g| e.check(g, &self.calc));
        let rating = Rating::from_calc(correct, elapsed, target);
        let attempt_id = self.store.insert_calc_attempt(&NewCalcAttempt {
            exercise: e.kind.id(),
            band: &e.band,
            params: serde_json::to_value(e).expect("exercises serialize"),
            answer: given,
            correct_value: e.answer,
            correct,
            duration_ms: elapsed.as_millis() as i64,
            rating: rating_id(rating),
        })?;
        let key = item_key(e);
        let exists = self
            .store
            .review_item(ItemKind::CalcExercise.id(), &key)?
            .is_some();
        let queued = exists || rating <= Rating::Hard;
        if queued {
            self.scheduler.review(
                &self.store,
                ItemKind::CalcExercise,
                &key,
                None,
                rating,
                Some(Source {
                    kind: "calc_attempt",
                    id: &attempt_id,
                }),
            )?;
        }
        Ok(CalcOutcome {
            correct,
            given,
            exact: e.answer,
            shortcut: e.shortcut,
            tolerance: self.calc.get(e.kind).tolerance,
            elapsed,
            target,
            rating,
            queued,
        })
    }

    pub fn calc_stats(&self, kind: ExerciseKind) -> Result<CalcStats> {
        Ok(self.store.calc_stats(kind.id())?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::services::tests::app_at;

    #[test]
    fn fast_right_answers_stay_out_of_the_queue() {
        let (mut app, t) = app_at(1_000_000);
        let round = app.calc_start(Some(ExerciseKind::Mdf));
        t.fetch_add(2_000, Ordering::SeqCst);
        let out = app
            .calc_submit(&round, Some(round.exercise.answer))
            .unwrap();
        assert!(out.correct);
        assert_eq!(out.rating, Rating::Easy);
        assert!(!out.queued);
        assert_eq!(app.store.review_totals().unwrap(), (0, 0));
        assert_eq!(app.calc_stats(ExerciseKind::Mdf).unwrap().attempts, 1);
    }

    #[test]
    fn wrong_or_slow_answers_are_queued_and_reviewed() {
        let (mut app, t) = app_at(1_000_000);
        let round = app.calc_start(Some(ExerciseKind::PotOdds));
        let out = app
            .calc_submit(&round, Some(round.exercise.answer + 50.0))
            .unwrap();
        assert!(!out.correct && out.queued);
        assert_eq!(out.rating, Rating::Again);
        let key = item_key(&round.exercise);
        let item = app
            .store
            .review_item("calc_exercise", &key)
            .unwrap()
            .unwrap();
        assert_eq!(item.reps, 1);

        // Same band again, answered right: the item is reviewed (not a new one).
        let again = app
            .calc_start_in_band(ExerciseKind::PotOdds, &round.exercise.band)
            .unwrap();
        assert!(
            app.calc_start_in_band(ExerciseKind::PotOdds, "bet_7")
                .is_none()
        );
        assert_eq!(again.exercise.band, round.exercise.band);
        t.fetch_add(1_000, Ordering::SeqCst);
        let out = app
            .calc_submit(&again, Some(again.exercise.answer))
            .unwrap();
        assert!(out.correct && out.queued);
        let item = app
            .store
            .review_item("calc_exercise", &key)
            .unwrap()
            .unwrap();
        assert_eq!(item.reps, 2);

        // Slow but right on a new skill: Hard, queued.
        let slow = app.calc_start(Some(ExerciseKind::Spr));
        t.fetch_add(60_000, Ordering::SeqCst);
        let out = app.calc_submit(&slow, Some(slow.exercise.answer)).unwrap();
        assert_eq!(out.rating, Rating::Hard);
        assert!(out.queued);

        // Skipped counts as wrong.
        let skip = app.calc_start(Some(ExerciseKind::Alpha));
        let out = app.calc_submit(&skip, None).unwrap();
        assert!(!out.correct && out.rating == Rating::Again);
    }

    #[test]
    fn mixed_mode_covers_kinds() {
        let (mut app, _) = app_at(42);
        let kinds: std::collections::BTreeSet<_> = (0..60)
            .map(|_| app.calc_start(None).exercise.kind.id())
            .collect();
        assert!(kinds.len() >= 5, "{kinds:?}");
    }
}

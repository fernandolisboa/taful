//! The spaced-repetition queue (spec §13, D6): one FSRS queue for every item
//! kind, stored in `review_items` / `review_log`.
//!
//! Scheduling rules (FSRS gives fractional-day intervals):
//!
//! - **Again** comes back after [`RELEARN_MS`] (10 minutes), so a missed item
//!   returns later in the same session.
//! - **Hard / Good / Easy** come back after the FSRS interval rounded to whole
//!   days, at least one day.

use fsrs::{FSRS, MemoryState};
use taful_core::srs::Rating;
use taful_store::{Millis, ReviewItem, ReviewRecord, Store};

use crate::Error;

/// Delay before an item rated Again is due again.
pub const RELEARN_MS: Millis = 10 * 60_000;
const DAY_MS: Millis = 86_400_000;

/// Desired retention when the user has not set one (§13).
pub const DEFAULT_RETENTION: f64 = 0.9;

/// Item kinds (`review_items.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    SpotTemplate,
    CalcExercise,
    LessonQuiz,
}

impl ItemKind {
    pub fn id(self) -> &'static str {
        match self {
            ItemKind::SpotTemplate => "spot_template",
            ItemKind::CalcExercise => "calc_exercise",
            ItemKind::LessonQuiz => "lesson_quiz",
        }
    }

    pub fn parse(s: &str) -> Option<ItemKind> {
        [
            ItemKind::SpotTemplate,
            ItemKind::CalcExercise,
            ItemKind::LessonQuiz,
        ]
        .into_iter()
        .find(|k| k.id() == s)
    }
}

/// Stable id of a rating (`review_log.rating`, `calc_attempts.rating`).
pub fn rating_id(r: Rating) -> &'static str {
    match r {
        Rating::Again => "again",
        Rating::Hard => "hard",
        Rating::Good => "good",
        Rating::Easy => "easy",
    }
}

/// Where a review came from, for the log.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    pub kind: &'a str,
    pub id: &'a str,
}

/// FSRS with the user's desired retention.
pub struct Scheduler {
    fsrs: FSRS,
    retention: f32,
}

impl Scheduler {
    pub fn new(retention: f64) -> Scheduler {
        Scheduler {
            fsrs: FSRS::default(),
            retention: retention.clamp(0.7, 0.99) as f32,
        }
    }

    pub fn retention(&self) -> f64 {
        f64::from(self.retention)
    }

    /// Applies `rating` to `item` at `now`: next memory state and due time.
    pub fn next(
        &self,
        item: &ReviewItem,
        rating: Rating,
        now: Millis,
    ) -> Result<(MemoryState, Millis, f64), Error> {
        let memory = match (item.stability, item.difficulty) {
            (Some(stability), Some(difficulty)) => Some(MemoryState {
                stability,
                difficulty,
            }),
            _ => None,
        };
        let elapsed_ms = item.last_review.map_or(0, |t| (now - t).max(0));
        let elapsed_days = elapsed_ms as f64 / DAY_MS as f64;
        let next = self
            .fsrs
            .next_states(memory, self.retention, elapsed_days.floor() as u32)
            .map_err(|e| Error::Scheduler(e.to_string()))?;
        let state = match rating {
            Rating::Again => next.again,
            Rating::Hard => next.hard,
            Rating::Good => next.good,
            Rating::Easy => next.easy,
        };
        let due = match rating {
            Rating::Again => now + RELEARN_MS,
            _ => {
                let days = f64::from(state.interval).round().max(1.0);
                now + (days as Millis) * DAY_MS
            }
        };
        Ok((state.memory, due, elapsed_days))
    }

    /// Reviews the item `(kind, key)`, creating it first if needed. Returns
    /// the updated item.
    pub fn review(
        &self,
        store: &Store,
        kind: ItemKind,
        key: &str,
        content_version: Option<u32>,
        rating: Rating,
        source: Option<Source<'_>>,
    ) -> Result<ReviewItem, Error> {
        let now = store.now();
        let item = store.ensure_review_item(kind.id(), key, content_version, now)?;
        self.review_item(store, &item, rating, content_version, source)
    }

    /// Reviews an existing item.
    pub fn review_item(
        &self,
        store: &Store,
        item: &ReviewItem,
        rating: Rating,
        content_version: Option<u32>,
        source: Option<Source<'_>>,
    ) -> Result<ReviewItem, Error> {
        let now = store.now();
        let (memory, due, elapsed_days) = self.next(item, rating, now)?;
        store.record_review(
            &item.id,
            &ReviewRecord {
                stability: memory.stability,
                difficulty: memory.difficulty,
                due,
                reviewed_at: now,
                rating: rating_id(rating),
                elapsed_days,
                lapse: rating == Rating::Again && item.reps > 0,
                content_version,
                source_kind: source.map(|s| s.kind),
                source_id: source.map(|s| s.id),
            },
        )?;
        Ok(store
            .review_item_by_id(&item.id)?
            .expect("item exists after review"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    #[test]
    fn intervals_grow_and_again_relearns() {
        let t = Arc::new(AtomicI64::new(1_000_000));
        let t2 = t.clone();
        let store = Store::open_in_memory(Arc::new(move || t2.load(Ordering::SeqCst))).unwrap();
        let s = Scheduler::new(DEFAULT_RETENTION);

        let item = s
            .review(
                &store,
                ItemKind::CalcExercise,
                "mdf/bet_50",
                None,
                Rating::Again,
                None,
            )
            .unwrap();
        assert_eq!(item.due, 1_000_000 + RELEARN_MS);
        assert_eq!(
            (item.reps, item.lapses),
            (1, 0),
            "first Again is not a lapse"
        );

        let mut last_gap = 0;
        for _ in 0..4 {
            t.store(item_due(&store, &item.id), Ordering::SeqCst);
            let now = store.now();
            let it = store.review_item_by_id(&item.id).unwrap().unwrap();
            let after = s
                .review_item(&store, &it, Rating::Good, None, None)
                .unwrap();
            let gap = after.due - now;
            assert!(gap >= DAY_MS, "{gap}");
            assert!(gap >= last_gap, "intervals grow: {gap} < {last_gap}");
            assert_eq!(gap % DAY_MS, 0, "whole days");
            last_gap = gap;
        }
        let it = store.review_item_by_id(&item.id).unwrap().unwrap();
        let after = s
            .review_item(&store, &it, Rating::Again, None, None)
            .unwrap();
        assert_eq!(after.lapses, 1);
        assert_eq!(store.review_totals().unwrap(), (1, 6));
    }

    fn item_due(store: &Store, id: &str) -> Millis {
        store.review_item_by_id(id).unwrap().unwrap().due
    }

    #[test]
    fn easy_beats_hard() {
        let store = Store::open_in_memory(Arc::new(|| 0)).unwrap();
        let s = Scheduler::new(DEFAULT_RETENTION);
        let hard = s
            .review(
                &store,
                ItemKind::LessonQuiz,
                "a",
                Some(1),
                Rating::Hard,
                None,
            )
            .unwrap();
        let easy = s
            .review(
                &store,
                ItemKind::LessonQuiz,
                "b",
                Some(1),
                Rating::Easy,
                None,
            )
            .unwrap();
        assert!(easy.due > hard.due);
    }

    #[test]
    fn kinds_round_trip() {
        for k in [
            ItemKind::SpotTemplate,
            ItemKind::CalcExercise,
            ItemKind::LessonQuiz,
        ] {
            assert_eq!(ItemKind::parse(k.id()), Some(k));
        }
    }
}

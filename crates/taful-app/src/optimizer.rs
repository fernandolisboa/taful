//! FSRS parameter optimization (spec §13, D6): once the review log holds at
//! least [`MIN_REVIEWS`] reviews, the user can re-fit the FSRS parameters to
//! their own memory. The fitted parameters are a setting
//! ([`keys::FSRS_PARAMETERS`]), so backups and future sync carry them, and
//! they schedule every review from then on. Reset goes back to the defaults.
//!
//! Optimizing takes seconds, so it is split for the UI: [`App::optimize_job`]
//! reads the log (quick), [`OptimizeJob::run`] fits the parameters on any
//! thread, and [`App::apply_parameters`] stores them.

use fsrs::{ComputeParametersInput, FSRSItem, FSRSReview, compute_parameters};
use serde_json::json;
use taful_store::{LoggedReview, Millis};

use crate::review::Scheduler;
use crate::{App, Error, Result, keys};

/// Reviews needed before optimizing is offered (§13).
pub const MIN_REVIEWS: u32 = 1_000;

const DAY_MS: Millis = 86_400_000;

/// What settings shows about the optimizer.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerStatus {
    /// Reviews in the log.
    pub reviews: u32,
    /// Reviews needed ([`MIN_REVIEWS`]).
    pub min_reviews: u32,
    /// Whether optimized parameters are in use (else the defaults).
    pub custom: bool,
}

impl OptimizerStatus {
    /// Whether the optimize action is available.
    pub fn available(&self) -> bool {
        self.reviews >= self.min_reviews
    }
}

/// A snapshot of the review log, ready to fit on a background thread.
#[derive(Debug, Clone)]
pub struct OptimizeJob {
    train_set: Vec<FSRSItem>,
}

impl OptimizeJob {
    /// Fits the FSRS parameters. CPU-bound; call it off the UI thread.
    pub fn run(self) -> Result<Vec<f32>> {
        compute_parameters(ComputeParametersInput {
            train_set: self.train_set,
            ..ComputeParametersInput::default()
        })
        .map_err(|e| Error::Scheduler(e.to_string()))
    }
}

/// FSRS rating number (1 = Again … 4 = Easy) of a logged rating id.
fn rating_number(id: &str) -> Option<u32> {
    match id {
        "again" => Some(1),
        "hard" => Some(2),
        "good" => Some(3),
        "easy" => Some(4),
        _ => None,
    }
}

/// Turns the log (grouped by item, in time order) into FSRS training items:
/// one item per review after an item's first, holding that item's history up
/// to and including it. Days between reviews are whole days, rounded down,
/// like the scheduler's elapsed days.
pub(crate) fn train_set(log: &[LoggedReview]) -> Vec<FSRSItem> {
    let mut items = Vec::new();
    for card in log.chunk_by(|a, b| a.item_id == b.item_id) {
        let mut reviews = Vec::with_capacity(card.len());
        let mut prev: Option<Millis> = None;
        for r in card {
            let Some(rating) = rating_number(&r.rating) else {
                continue;
            };
            let delta_t = prev.map_or(0, |p| ((r.reviewed_at - p).max(0) / DAY_MS) as u32);
            prev = Some(r.reviewed_at);
            reviews.push(FSRSReview { rating, delta_t });
            if reviews.len() > 1 {
                items.push(FSRSItem {
                    reviews: reviews.clone(),
                });
            }
        }
    }
    items
}

impl App {
    pub fn optimizer_status(&self) -> Result<OptimizerStatus> {
        let (_, reviews) = self.store.review_totals()?;
        Ok(OptimizerStatus {
            reviews,
            min_reviews: MIN_REVIEWS,
            custom: self.fsrs_parameters().is_some(),
        })
    }

    /// The stored optimized parameters, if any (and valid).
    pub fn fsrs_parameters(&self) -> Option<Vec<f32>> {
        let v = self.store.setting(keys::FSRS_PARAMETERS).ok().flatten()?;
        let params: Vec<f32> = serde_json::from_value(v).ok()?;
        fsrs::FSRS::new(&params).ok().map(|_| params)
    }

    /// Snapshots the review log for optimizing. Fails with
    /// [`Error::NotEnoughReviews`] below [`MIN_REVIEWS`].
    pub fn optimize_job(&self) -> Result<OptimizeJob> {
        let status = self.optimizer_status()?;
        if !status.available() {
            return Err(Error::NotEnoughReviews {
                have: status.reviews,
                need: status.min_reviews,
            });
        }
        Ok(OptimizeJob {
            train_set: train_set(&self.store.review_history()?),
        })
    }

    /// Stores `params` and schedules with them from now on.
    pub fn apply_parameters(&mut self, params: &[f32]) -> Result<()> {
        if params.is_empty() {
            return Err(Error::Scheduler("no FSRS parameters".into()));
        }
        let scheduler = Scheduler::with_parameters(self.retention(), params)?;
        self.store
            .set_setting(keys::FSRS_PARAMETERS, &json!(params))?;
        self.scheduler = scheduler;
        Ok(())
    }

    /// Optimizes and applies in one call (on the calling thread).
    pub fn optimize(&mut self) -> Result<Vec<f32>> {
        let params = self.optimize_job()?.run()?;
        self.apply_parameters(&params)?;
        Ok(params)
    }

    /// Goes back to the default parameters.
    pub fn reset_parameters(&mut self) -> Result<()> {
        self.store.clear_setting(keys::FSRS_PARAMETERS)?;
        self.scheduler = self.make_scheduler();
        Ok(())
    }

    /// The scheduler for the stored retention and parameters. Parameters that
    /// fail to load fall back to the defaults.
    pub(crate) fn make_scheduler(&self) -> Scheduler {
        let retention = self.retention();
        self.fsrs_parameters()
            .and_then(|p| Scheduler::with_parameters(retention, &p).ok())
            .unwrap_or_else(|| Scheduler::new(retention))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use fsrs::DEFAULT_PARAMETERS;
    use taful_core::srs::Rating;
    use taful_store::ReviewRecord;

    use super::*;
    use crate::review::{ItemKind, rating_id};
    use crate::services::tests::app_at;

    /// Seeds `cards × reviews` log rows of a learner who remembers far
    /// better than the FSRS defaults assume (deterministic).
    fn seed_log(app: &App, cards: u32, reviews: u32) {
        let mut rng: u64 = 0x2545_F491_4F6C_DD1D;
        let mut roll = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng % 100) as u32
        };
        let gaps = [1, 4, 12, 30, 75, 160];
        for c in 0..cards {
            let item = app
                .store
                .ensure_review_item(ItemKind::CalcExercise.id(), &format!("card-{c}"), None, 0)
                .unwrap();
            let mut at: Millis = i64::from(c) * 3_600_000;
            let mut step = 0;
            for _ in 0..reviews {
                let r = roll();
                let rating = match r {
                    0..=3 => Rating::Again,
                    4..=9 => Rating::Hard,
                    10..=69 => Rating::Good,
                    _ => Rating::Easy,
                };
                let next = if rating == Rating::Again {
                    step = 0;
                    DAY_MS
                } else {
                    let g = gaps[step.min(gaps.len() - 1)];
                    step += 1;
                    g * DAY_MS
                };
                app.store
                    .record_review(
                        &item.id,
                        &ReviewRecord {
                            stability: 1.0,
                            difficulty: 5.0,
                            due: at + next,
                            reviewed_at: at,
                            rating: rating_id(rating),
                            elapsed_days: 0.0,
                            lapse: false,
                            content_version: None,
                            source_kind: None,
                            source_id: None,
                        },
                    )
                    .unwrap();
                at += next;
            }
        }
    }

    /// Due gap of a fresh item reviewed Good, then Good again when due.
    fn second_good_gap(app: &mut App, t: &std::sync::atomic::AtomicI64, key: &str) -> Millis {
        let item = app
            .scheduler
            .review(
                &app.store,
                ItemKind::LessonQuiz,
                key,
                Some(1),
                Rating::Good,
                None,
            )
            .unwrap();
        t.store(item.due, Ordering::SeqCst);
        let after = app
            .scheduler
            .review_item(&app.store, &item, Rating::Good, None, None)
            .unwrap();
        after.due - item.due
    }

    #[test]
    fn unavailable_below_the_threshold() {
        let (app, _) = app_at(0);
        seed_log(&app, 199, 5);
        let status = app.optimizer_status().unwrap();
        assert_eq!(status.reviews, 995);
        assert!(!status.available());
        assert!(!status.custom);
        match app.optimize_job() {
            Err(Error::NotEnoughReviews { have, need }) => {
                assert_eq!((have, need), (995, MIN_REVIEWS));
            }
            other => panic!("expected NotEnoughReviews, got {other:?}"),
        }
    }

    #[test]
    fn optimizing_stores_new_parameters_that_schedule_and_reset_restores() {
        let (mut app, t) = app_at(0);
        seed_log(&app, 250, 5);
        assert!(app.optimizer_status().unwrap().available());
        let base = 10_000 * DAY_MS;
        t.store(base, Ordering::SeqCst);
        let default_gap = second_good_gap(&mut app, &t, "before");

        let params = app.optimize().unwrap();
        assert_eq!(params.len(), DEFAULT_PARAMETERS.len());
        assert_ne!(params.as_slice(), DEFAULT_PARAMETERS.as_slice());
        assert_eq!(app.fsrs_parameters(), Some(params.clone()));
        assert!(app.optimizer_status().unwrap().custom);

        t.store(base + 1_000 * DAY_MS, Ordering::SeqCst);
        let tuned_gap = second_good_gap(&mut app, &t, "after");
        assert_ne!(
            tuned_gap, default_gap,
            "later intervals use the new parameters"
        );

        app.reset_parameters().unwrap();
        assert_eq!(app.fsrs_parameters(), None);
        assert!(!app.optimizer_status().unwrap().custom);
        t.store(base + 2_000 * DAY_MS, Ordering::SeqCst);
        assert_eq!(second_good_gap(&mut app, &t, "reset"), default_gap);
    }

    #[test]
    fn parameters_survive_backup_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(dir.path()).unwrap();
        let mut params = DEFAULT_PARAMETERS.to_vec();
        params[2] *= 3.0;
        app.apply_parameters(&params).unwrap();
        let export = dir.path().join("export.sqlite");
        app.export_to(&export).unwrap();
        app.reset_parameters().unwrap();
        drop(app);

        let other = tempfile::tempdir().unwrap();
        let mut restored = App::open(other.path()).unwrap();
        assert_eq!(restored.fsrs_parameters(), None);
        restored.import_from(&export).unwrap();
        assert_eq!(restored.fsrs_parameters(), Some(params.clone()));
        assert!(restored.scheduler.uses_parameters(&params));
        drop(restored);
        let reopened = App::open(other.path()).unwrap();
        assert!(
            reopened.scheduler.uses_parameters(&params),
            "loaded on open"
        );
    }

    #[test]
    fn invalid_parameters_are_refused() {
        let (mut app, _) = app_at(0);
        assert!(app.apply_parameters(&[1.0, 2.0, 3.0]).is_err());
        assert!(
            app.apply_parameters(&[]).is_err(),
            "empty means defaults: use reset"
        );
        assert_eq!(app.fsrs_parameters(), None);
    }

    #[test]
    fn train_set_uses_whole_days_and_skips_first_reviews() {
        let r = |id: &str, at: Millis, rating: &str| LoggedReview {
            item_id: id.into(),
            reviewed_at: at,
            rating: rating.into(),
        };
        let log = [
            r("a", 0, "again"),
            r("a", 600_000, "good"),
            r("a", 600_000 + 3 * DAY_MS + 5, "easy"),
            r("b", 50, "hard"),
        ];
        let items = train_set(&log);
        let shape: Vec<Vec<(u32, u32)>> = items
            .iter()
            .map(|i| i.reviews.iter().map(|x| (x.rating, x.delta_t)).collect())
            .collect();
        assert_eq!(
            shape,
            vec![vec![(1, 0), (3, 0)], vec![(1, 0), (3, 0), (4, 3)]]
        );
    }
}

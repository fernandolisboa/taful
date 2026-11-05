//! Repositories: typed reads and writes per table (spec §14.2).

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Millis, Result, Store, new_id};

// ---- settings -------------------------------------------------------------------

impl Store {
    /// A setting's JSON value, or `None` if unset.
    pub fn setting(&self, key: &str) -> Result<Option<Value>> {
        let text: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1 AND deleted_at IS NULL",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
    }

    /// Sets (or replaces) a setting.
    pub fn set_setting(&self, key: &str, value: &Value) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "INSERT INTO settings (id, created_at, updated_at, key, value)
             VALUES (?1, ?2, ?2, ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value,
                 updated_at = excluded.updated_at, deleted_at = NULL",
            params![new_id(), now, key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// Unsets a setting (soft delete).
    pub fn clear_setting(&self, key: &str) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE settings SET deleted_at = ?1, updated_at = ?1
             WHERE key = ?2 AND deleted_at IS NULL",
            params![now, key],
        )?;
        Ok(())
    }
}

// ---- lesson progress ----------------------------------------------------------------

/// Lock state of a lesson at one version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonState {
    Locked,
    Available,
    Done,
}

impl LessonState {
    pub fn id(self) -> &'static str {
        match self {
            LessonState::Locked => "locked",
            LessonState::Available => "available",
            LessonState::Done => "done",
        }
    }

    fn parse(s: &str) -> LessonState {
        match s {
            "done" => LessonState::Done,
            "available" => LessonState::Available,
            _ => LessonState::Locked,
        }
    }
}

/// One `(lesson id, version)` row.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonProgress {
    pub lesson_id: String,
    pub version: u32,
    pub state: LessonState,
    /// Best quiz score at this version, 0..1.
    pub best_score: Option<f64>,
    pub updated_at: Millis,
}

fn lesson_row(r: &Row<'_>) -> rusqlite::Result<LessonProgress> {
    Ok(LessonProgress {
        lesson_id: r.get(0)?,
        version: r.get(1)?,
        state: LessonState::parse(&r.get::<_, String>(2)?),
        best_score: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

impl Store {
    /// Every progress row, all lessons and versions.
    pub fn lesson_progress(&self) -> Result<Vec<LessonProgress>> {
        let mut stmt = self.conn().prepare(
            "SELECT lesson_id, version, state, best_score, updated_at FROM lesson_progress
             WHERE deleted_at IS NULL ORDER BY lesson_id, version",
        )?;
        let rows = stmt.query_map([], lesson_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Records progress at `(lesson_id, version)`. The state only moves
    /// forward (`done` is never undone) and the best score keeps the maximum.
    pub fn record_lesson_progress(
        &self,
        lesson_id: &str,
        version: u32,
        state: LessonState,
        score: Option<f64>,
    ) -> Result<()> {
        let now = self.now();
        let existing: Option<(String, Option<f64>)> = self
            .conn()
            .query_row(
                "SELECT state, best_score FROM lesson_progress
                 WHERE lesson_id = ?1 AND version = ?2",
                params![lesson_id, version],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let rank = |s: LessonState| match s {
            LessonState::Locked => 0,
            LessonState::Available => 1,
            LessonState::Done => 2,
        };
        let (state, best) = match existing {
            Some((old_state, old_best)) => {
                let old_state = LessonState::parse(&old_state);
                let state = if rank(state) >= rank(old_state) {
                    state
                } else {
                    old_state
                };
                let best = match (old_best, score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                (state, best)
            }
            None => (state, score),
        };
        self.conn().execute(
            "INSERT INTO lesson_progress
                 (id, created_at, updated_at, lesson_id, version, state, best_score)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (lesson_id, version) DO UPDATE SET state = excluded.state,
                 best_score = excluded.best_score, updated_at = excluded.updated_at,
                 deleted_at = NULL",
            params![new_id(), now, lesson_id, version, state.id(), best],
        )?;
        Ok(())
    }
}

// ---- attempts -------------------------------------------------------------------------

/// A finished quiz.
#[derive(Debug, Clone, PartialEq)]
pub struct NewQuizAttempt<'a> {
    pub quiz_id: &'a str,
    pub version: u32,
    pub answers: Value,
    /// 0..1.
    pub score: f64,
    pub duration_ms: i64,
}

/// A calc exercise answered (or skipped).
#[derive(Debug, Clone, PartialEq)]
pub struct NewCalcAttempt<'a> {
    pub exercise: &'a str,
    pub band: &'a str,
    pub params: Value,
    pub answer: Option<f64>,
    pub correct_value: f64,
    pub correct: bool,
    pub duration_ms: i64,
    pub rating: &'a str,
}

/// A stored calc attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcAttempt {
    pub id: String,
    pub created_at: Millis,
    pub exercise: String,
    pub band: String,
    pub answer: Option<f64>,
    pub correct_value: f64,
    pub correct: bool,
    pub duration_ms: i64,
    pub rating: String,
}

/// Totals for one exercise kind.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcStats {
    pub attempts: u32,
    pub correct: u32,
    pub mean_duration_ms: f64,
}

impl Store {
    pub fn insert_quiz_attempt(&self, a: &NewQuizAttempt<'_>) -> Result<String> {
        let id = new_id();
        self.conn().execute(
            "INSERT INTO quiz_attempts
                 (id, created_at, updated_at, quiz_id, version, answers, score, duration_ms)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                self.now(),
                a.quiz_id,
                a.version,
                serde_json::to_string(&a.answers)?,
                a.score,
                a.duration_ms
            ],
        )?;
        Ok(id)
    }

    /// Number of attempts at a quiz (any version).
    pub fn quiz_attempt_count(&self, quiz_id: &str) -> Result<u32> {
        Ok(self.conn().query_row(
            "SELECT count(*) FROM quiz_attempts WHERE quiz_id = ?1 AND deleted_at IS NULL",
            [quiz_id],
            |r| r.get(0),
        )?)
    }

    pub fn insert_calc_attempt(&self, a: &NewCalcAttempt<'_>) -> Result<String> {
        let id = new_id();
        self.conn().execute(
            "INSERT INTO calc_attempts
                 (id, created_at, updated_at, exercise, band, params, answer, correct_value,
                  correct, duration_ms, rating)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                self.now(),
                a.exercise,
                a.band,
                serde_json::to_string(&a.params)?,
                a.answer,
                a.correct_value,
                a.correct,
                a.duration_ms,
                a.rating
            ],
        )?;
        Ok(id)
    }

    /// Most recent calc attempts first.
    pub fn recent_calc_attempts(&self, limit: u32) -> Result<Vec<CalcAttempt>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, created_at, exercise, band, answer, correct_value, correct,
                    duration_ms, rating
             FROM calc_attempts WHERE deleted_at IS NULL
             ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(CalcAttempt {
                id: r.get(0)?,
                created_at: r.get(1)?,
                exercise: r.get(2)?,
                band: r.get(3)?,
                answer: r.get(4)?,
                correct_value: r.get(5)?,
                correct: r.get(6)?,
                duration_ms: r.get(7)?,
                rating: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Totals for one exercise kind.
    pub fn calc_stats(&self, exercise: &str) -> Result<CalcStats> {
        Ok(self.conn().query_row(
            "SELECT count(*), coalesce(sum(correct), 0), coalesce(avg(duration_ms), 0)
             FROM calc_attempts WHERE exercise = ?1 AND deleted_at IS NULL",
            [exercise],
            |r| {
                Ok(CalcStats {
                    attempts: r.get(0)?,
                    correct: r.get(1)?,
                    mean_duration_ms: r.get(2)?,
                })
            },
        )?)
    }
}

// ---- review queue ----------------------------------------------------------------------

/// One item in the spaced-repetition queue (spec §13).
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewItem {
    pub id: String,
    /// `spot_template`, `calc_exercise` or `lesson_quiz`.
    pub kind: String,
    pub key: String,
    pub content_version: Option<u32>,
    /// FSRS memory state, `None` before the first review.
    pub stability: Option<f32>,
    pub difficulty: Option<f32>,
    pub due: Millis,
    pub last_review: Option<Millis>,
    pub reps: u32,
    pub lapses: u32,
}

/// The outcome of one review, written to the item and the log together.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewRecord<'a> {
    pub stability: f32,
    pub difficulty: f32,
    pub due: Millis,
    pub reviewed_at: Millis,
    pub rating: &'a str,
    pub elapsed_days: f64,
    /// Whether the rating counts as a lapse (`again`).
    pub lapse: bool,
    /// Version to move the item to (§12.4), if it changes.
    pub content_version: Option<u32>,
    /// What produced the review: `decision`, `calc_attempt` or `quiz_attempt`.
    pub source_kind: Option<&'a str>,
    pub source_id: Option<&'a str>,
}

const ITEM_COLS: &str = "id, kind, item_key, content_version, stability, difficulty, due, \
                         last_review, reps, lapses";

fn item_row(r: &Row<'_>) -> rusqlite::Result<ReviewItem> {
    Ok(ReviewItem {
        id: r.get(0)?,
        kind: r.get(1)?,
        key: r.get(2)?,
        content_version: r.get(3)?,
        stability: r.get(4)?,
        difficulty: r.get(5)?,
        due: r.get(6)?,
        last_review: r.get(7)?,
        reps: r.get(8)?,
        lapses: r.get(9)?,
    })
}

impl Store {
    /// The live item for `(kind, key)`.
    pub fn review_item(&self, kind: &str, key: &str) -> Result<Option<ReviewItem>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {ITEM_COLS} FROM review_items
                     WHERE kind = ?1 AND item_key = ?2 AND deleted_at IS NULL"
                ),
                params![kind, key],
                item_row,
            )
            .optional()?)
    }

    pub fn review_item_by_id(&self, id: &str) -> Result<Option<ReviewItem>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {ITEM_COLS} FROM review_items WHERE id = ?1"),
                [id],
                item_row,
            )
            .optional()?)
    }

    /// Returns the item for `(kind, key)`, creating it (new, due at `due`) if
    /// it does not exist. A soft-deleted item is revived with its history.
    pub fn ensure_review_item(
        &self,
        kind: &str,
        key: &str,
        content_version: Option<u32>,
        due: Millis,
    ) -> Result<ReviewItem> {
        let now = self.now();
        self.conn().execute(
            "INSERT INTO review_items (id, created_at, updated_at, kind, item_key,
                 content_version, due)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (kind, item_key) DO UPDATE SET deleted_at = NULL,
                 updated_at = excluded.updated_at
             WHERE review_items.deleted_at IS NOT NULL",
            params![new_id(), now, kind, key, content_version, due],
        )?;
        Ok(self
            .review_item(kind, key)?
            .expect("row exists after upsert"))
    }

    /// Applies a review to an item and appends it to the log, atomically.
    /// Returns the log row id.
    pub fn record_review(&self, item_id: &str, rec: &ReviewRecord<'_>) -> Result<String> {
        let tx = self.conn().unchecked_transaction()?;
        let now = self.now();
        tx.execute(
            "UPDATE review_items SET stability = ?1, difficulty = ?2, due = ?3,
                 last_review = ?4, reps = reps + 1, lapses = lapses + ?5,
                 content_version = coalesce(?6, content_version), updated_at = ?7
             WHERE id = ?8",
            params![
                rec.stability,
                rec.difficulty,
                rec.due,
                rec.reviewed_at,
                u32::from(rec.lapse),
                rec.content_version,
                now,
                item_id
            ],
        )?;
        let log_id = new_id();
        tx.execute(
            "INSERT INTO review_log (id, created_at, updated_at, item_id, reviewed_at, rating,
                 elapsed_days, source_kind, source_id)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                log_id,
                now,
                item_id,
                rec.reviewed_at,
                rec.rating,
                rec.elapsed_days,
                rec.source_kind,
                rec.source_id
            ],
        )?;
        tx.commit()?;
        Ok(log_id)
    }

    /// Items due at or before `now`, earliest first.
    pub fn due_items(&self, now: Millis, limit: u32) -> Result<Vec<ReviewItem>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {ITEM_COLS} FROM review_items
             WHERE deleted_at IS NULL AND due <= ?1 ORDER BY due, id LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![now, limit], item_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Number of items due at or before `now`.
    pub fn count_due(&self, now: Millis) -> Result<u32> {
        Ok(self.conn().query_row(
            "SELECT count(*) FROM review_items WHERE deleted_at IS NULL AND due <= ?1",
            [now],
            |r| r.get(0),
        )?)
    }

    /// When the next item after `now` becomes due.
    pub fn next_due_after(&self, now: Millis) -> Result<Option<Millis>> {
        Ok(self.conn().query_row(
            "SELECT min(due) FROM review_items WHERE deleted_at IS NULL AND due > ?1",
            [now],
            |r| r.get(0),
        )?)
    }

    /// Total live items, and total reviews ever logged.
    pub fn review_totals(&self) -> Result<(u32, u32)> {
        let items = self.conn().query_row(
            "SELECT count(*) FROM review_items WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let reviews = self.conn().query_row(
            "SELECT count(*) FROM review_log WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok((items, reviews))
    }

    /// Soft-deletes an item (its log is kept).
    pub fn delete_review_item(&self, id: &str) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE review_items SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }
}

// ---- hands and decisions ----------------------------------------------------------------

/// A played or simulated hand (spot mode or full hand).
#[derive(Debug, Clone, PartialEq)]
pub struct NewHand<'a> {
    /// `spot` or `full_hand`.
    pub mode: &'a str,
    pub config_key: &'a str,
    pub config_toml: &'a str,
    pub seed: u64,
    pub hero_seat: u32,
    pub bot_ids: Value,
    pub library_format_version: Option<u32>,
}

/// One graded hero decision.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDecision<'a> {
    /// `None` in spot mode.
    pub hand_id: Option<&'a str>,
    pub node_key: &'a str,
    pub hero_combo: &'a str,
    pub chosen_action: &'a str,
    pub ev_per_action: Value,
    pub frequencies: Value,
    pub pot: f64,
    pub ev_loss: f64,
    pub grade: &'a str,
    pub thresholds_version: u32,
}

impl Store {
    pub fn insert_hand(&self, h: &NewHand<'_>) -> Result<String> {
        let id = new_id();
        self.conn().execute(
            "INSERT INTO hands (id, created_at, updated_at, mode, config_key, config_toml, seed,
                 hero_seat, bot_ids, library_format_version)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                self.now(),
                h.mode,
                h.config_key,
                h.config_toml,
                // SQLite integers are i64; the seed's bits are kept as is.
                h.seed as i64,
                h.hero_seat,
                serde_json::to_string(&h.bot_ids)?,
                h.library_format_version
            ],
        )?;
        Ok(id)
    }

    /// Marks a hand finished.
    pub fn finish_hand(&self, id: &str, ended_reason: &str, total_ev_loss: f64) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE hands SET ended_reason = ?1, total_ev_loss = ?2, updated_at = ?3
             WHERE id = ?4",
            params![ended_reason, total_ev_loss, now, id],
        )?;
        Ok(())
    }

    pub fn insert_decision(&self, d: &NewDecision<'_>) -> Result<String> {
        let id = new_id();
        self.conn().execute(
            "INSERT INTO decisions (id, created_at, updated_at, hand_id, node_key, hero_combo,
                 chosen_action, ev_per_action, frequencies, pot, ev_loss, grade,
                 thresholds_version)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                self.now(),
                d.hand_id,
                d.node_key,
                d.hero_combo,
                d.chosen_action,
                serde_json::to_string(&d.ev_per_action)?,
                serde_json::to_string(&d.frequencies)?,
                d.pot,
                d.ev_loss,
                d.grade,
                d.thresholds_version
            ],
        )?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_clock;

    fn store() -> (Store, std::sync::Arc<std::sync::atomic::AtomicI64>) {
        let (clock, t) = test_clock(1_000);
        (Store::open_in_memory(clock).unwrap(), t)
    }

    #[test]
    fn settings_round_trip() {
        let (s, _) = store();
        assert_eq!(s.setting("locale").unwrap(), None);
        s.set_setting("locale", &json!("pt-BR")).unwrap();
        s.set_setting("locale", &json!("en-US")).unwrap();
        assert_eq!(s.setting("locale").unwrap(), Some(json!("en-US")));
        s.clear_setting("locale").unwrap();
        assert_eq!(s.setting("locale").unwrap(), None);
        s.set_setting("locale", &json!("pt-BR")).unwrap();
        assert_eq!(s.setting("locale").unwrap(), Some(json!("pt-BR")));
    }

    #[test]
    fn lesson_progress_only_moves_forward() {
        let (s, _) = store();
        s.record_lesson_progress("a", 1, LessonState::Available, None)
            .unwrap();
        s.record_lesson_progress("a", 1, LessonState::Done, Some(0.8))
            .unwrap();
        s.record_lesson_progress("a", 1, LessonState::Available, Some(0.6))
            .unwrap();
        s.record_lesson_progress("a", 2, LessonState::Available, None)
            .unwrap();
        let rows = s.lesson_progress().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].state, LessonState::Done);
        assert_eq!(rows[0].best_score, Some(0.8));
        assert_eq!(rows[1].version, 2);
    }

    #[test]
    fn review_queue() {
        let (s, t) = store();
        let a = s
            .ensure_review_item("calc_exercise", "pot_odds/bet_50", None, 500)
            .unwrap();
        let again = s
            .ensure_review_item("calc_exercise", "pot_odds/bet_50", None, 9_999)
            .unwrap();
        assert_eq!(a, again, "ensure does not reset an existing item");
        s.ensure_review_item("lesson_quiz", "q#1", Some(2), 2_000)
            .unwrap();
        assert_eq!(s.count_due(1_000).unwrap(), 1);
        assert_eq!(s.due_items(5_000, 10).unwrap().len(), 2);
        assert_eq!(s.next_due_after(1_000).unwrap(), Some(2_000));

        t.store(1_500, std::sync::atomic::Ordering::SeqCst);
        s.record_review(
            &a.id,
            &ReviewRecord {
                stability: 2.3,
                difficulty: 5.0,
                due: 100_000,
                reviewed_at: 1_500,
                rating: "again",
                elapsed_days: 0.0,
                lapse: true,
                content_version: None,
                source_kind: Some("calc_attempt"),
                source_id: Some("x"),
            },
        )
        .unwrap();
        let a = s.review_item_by_id(&a.id).unwrap().unwrap();
        assert_eq!((a.reps, a.lapses, a.due), (1, 1, 100_000));
        assert_eq!(a.stability, Some(2.3));
        assert_eq!(s.review_totals().unwrap(), (2, 1));

        s.delete_review_item(&a.id).unwrap();
        assert_eq!(
            s.review_item("calc_exercise", "pot_odds/bet_50").unwrap(),
            None
        );
        let revived = s
            .ensure_review_item("calc_exercise", "pot_odds/bet_50", None, 0)
            .unwrap();
        assert_eq!(revived.id, a.id);
        assert_eq!(revived.reps, 1, "history kept");
    }

    #[test]
    fn attempts() {
        let (s, _) = store();
        s.insert_calc_attempt(&NewCalcAttempt {
            exercise: "mdf",
            band: "bet_50",
            params: json!({"bet_pct": 50}),
            answer: Some(66.0),
            correct_value: 66.7,
            correct: true,
            duration_ms: 4_000,
            rating: "easy",
        })
        .unwrap();
        s.insert_calc_attempt(&NewCalcAttempt {
            exercise: "mdf",
            band: "bet_50",
            params: json!({}),
            answer: None,
            correct_value: 66.7,
            correct: false,
            duration_ms: 2_000,
            rating: "again",
        })
        .unwrap();
        let st = s.calc_stats("mdf").unwrap();
        assert_eq!((st.attempts, st.correct), (2, 1));
        assert_eq!(st.mean_duration_ms, 3_000.0);
        assert_eq!(s.recent_calc_attempts(1).unwrap().len(), 1);

        s.insert_quiz_attempt(&NewQuizAttempt {
            quiz_id: "q",
            version: 1,
            answers: json!([1, 2]),
            score: 0.5,
            duration_ms: 10,
        })
        .unwrap();
        assert_eq!(s.quiz_attempt_count("q").unwrap(), 1);
    }

    #[test]
    fn hands_and_decisions() {
        let (s, _) = store();
        let h = s
            .insert_hand(&NewHand {
                mode: "full_hand",
                config_key: "cash6-100bb-r0",
                config_toml: "",
                seed: u64::MAX,
                hero_seat: 2,
                bot_ids: json!(["ref"]),
                library_format_version: Some(1),
            })
            .unwrap();
        s.insert_decision(&NewDecision {
            hand_id: Some(&h),
            node_key: "n",
            hero_combo: "AsKs",
            chosen_action: "raise",
            ev_per_action: json!({}),
            frequencies: json!({}),
            pot: 1.5,
            ev_loss: 0.0,
            grade: "best",
            thresholds_version: 1,
        })
        .unwrap();
        s.finish_hand(&h, "showdown", 0.0).unwrap();
        // Foreign keys are enforced.
        assert!(
            s.insert_decision(&NewDecision {
                hand_id: Some("nope"),
                node_key: "n",
                hero_combo: "AsKs",
                chosen_action: "raise",
                ev_per_action: json!({}),
                frequencies: json!({}),
                pot: 1.5,
                ev_loss: 0.0,
                grade: "best",
                thresholds_version: 1,
            })
            .is_err()
        );
    }
}

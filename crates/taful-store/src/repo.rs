//! Repositories: typed reads and writes per table (spec §14.2).

use rusqlite::{OptionalExtension, Row, named_params, params};
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

/// One row of the review log, as the FSRS optimizer reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct LoggedReview {
    pub item_id: String,
    pub reviewed_at: Millis,
    /// `again`, `hard`, `good` or `easy`.
    pub rating: String,
    /// Days since the item's previous review, as the scheduler saw it: `0`
    /// for a first review, including the first one after a revival.
    pub elapsed_days: f64,
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
                &format!(
                    "SELECT {ITEM_COLS} FROM review_items WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [id],
                item_row,
            )
            .optional()?)
    }

    /// Returns the item for `(kind, key)`, creating it (new, due at `due`) if
    /// it does not exist. A soft-deleted item is revived as a new item (memory
    /// state, due and version reset); its old reviews stay in the log.
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
                 updated_at = excluded.updated_at, content_version = excluded.content_version,
                 due = excluded.due, stability = NULL, difficulty = NULL, last_review = NULL,
                 reps = 0, lapses = 0
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
        self.atomic(|s| {
            let now = s.now();
            s.conn().execute(
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
            s.conn().execute(
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
            Ok(log_id)
        })
    }

    /// Items of `kinds` (all kinds when empty) due at or before `now`,
    /// earliest first.
    pub fn due_items(&self, now: Millis, kinds: &[&str], limit: u32) -> Result<Vec<ReviewItem>> {
        self.due_items_held(now, kinds, None, limit)
    }

    /// [`Store::due_items`] without the items `hold` matches.
    pub fn due_items_held(
        &self,
        now: Millis,
        kinds: &[&str],
        hold: Option<&Hold<'_>>,
        limit: u32,
    ) -> Result<Vec<ReviewItem>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {ITEM_COLS} FROM review_items
             WHERE deleted_at IS NULL AND due <= :now AND {} AND {HOLD_FILTER}
             ORDER BY due, id LIMIT :limit",
            kind_filter(kinds)
        ))?;
        let (k, g, u) = Hold::params(hold);
        let rows = stmt.query_map(
            named_params! {
                ":now": now, ":hold_kind": k, ":key_glob": g, ":unless_glob": u, ":limit": limit,
            },
            item_row,
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Number of items of `kinds` (all kinds when empty) due at or before `now`.
    pub fn count_due(&self, now: Millis, kinds: &[&str]) -> Result<u32> {
        self.count_due_held(now, kinds, None)
    }

    /// [`Store::count_due`] without the items `hold` matches. Counts in
    /// SQLite, so it stays cheap however long the queue grows.
    pub fn count_due_held(
        &self,
        now: Millis,
        kinds: &[&str],
        hold: Option<&Hold<'_>>,
    ) -> Result<u32> {
        let (k, g, u) = Hold::params(hold);
        Ok(self.conn().query_row(
            &format!(
                "SELECT count(*) FROM review_items
                 WHERE deleted_at IS NULL AND due <= :now AND {} AND {HOLD_FILTER}",
                kind_filter(kinds)
            ),
            named_params! { ":now": now, ":hold_kind": k, ":key_glob": g, ":unless_glob": u },
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

    /// The whole review log, grouped by item and in time order within an
    /// item (FSRS optimizer, spec §13). Reviews of soft-deleted items are
    /// included: they are still the user's memory history.
    pub fn review_history(&self) -> Result<Vec<LoggedReview>> {
        let mut stmt = self.conn().prepare(
            "SELECT item_id, reviewed_at, rating, elapsed_days FROM review_log
             WHERE deleted_at IS NULL ORDER BY item_id, reviewed_at, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(LoggedReview {
                item_id: r.get(0)?,
                reviewed_at: r.get(1)?,
                rating: r.get(2)?,
                elapsed_days: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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
    /// Who sits at each seat, one entry per seat (`"hero"`, `"reference"` or
    /// a bot profile id). Hands stored before #28 hold `["reference"]`: every
    /// bot played the reference.
    pub bot_ids: Value,
    pub library_format_version: Option<u32>,
    /// The hand as data for replay (JSON), once known.
    pub replay: Option<Value>,
}

/// Decision context (spec §13): its spot template and leak bucket.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecisionContext<'a> {
    pub template_key: &'a str,
    pub spot: &'a str,
    pub street: &'a str,
    pub node_type: &'a str,
    pub texture: Option<&'a str>,
    /// Index into the hand's replay states (full hands).
    pub step: Option<u32>,
    /// `reference` or a bot profile id.
    pub opponent: &'a str,
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
    pub context: Option<DecisionContext<'a>>,
}

/// A stored hand.
#[derive(Debug, Clone, PartialEq)]
pub struct HandRow {
    pub id: String,
    pub created_at: Millis,
    pub mode: String,
    pub config_key: String,
    pub config_toml: String,
    pub seed: u64,
    pub hero_seat: u32,
    pub ended_reason: Option<String>,
    pub total_ev_loss: Option<f64>,
    pub hero_net: Option<f64>,
    pub replay: Option<Value>,
    /// Who sat at the table (JSON array, see [`NewHand::bot_ids`]).
    pub bot_ids: Value,
}

/// A stored decision.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRow {
    pub id: String,
    pub created_at: Millis,
    pub hand_id: Option<String>,
    pub node_key: String,
    pub hero_combo: String,
    pub chosen_action: String,
    pub ev_per_action: Value,
    pub frequencies: Value,
    pub pot: f64,
    pub ev_loss: f64,
    pub grade: String,
    pub thresholds_version: u32,
    pub template_key: Option<String>,
    pub spot: Option<String>,
    pub street: Option<String>,
    pub node_type: Option<String>,
    pub texture: Option<String>,
    pub step: Option<u32>,
    pub opponent: Option<String>,
}

/// EV loss of one leak bucket over the window (spec §13).
#[derive(Debug, Clone, PartialEq)]
pub struct BucketStats {
    pub spot: String,
    pub street: String,
    pub node_type: String,
    pub texture: Option<String>,
    /// `reference` or a bot profile id.
    pub opponent: String,
    pub decisions: u32,
    pub total_ev_loss: f64,
    /// Decisions graded below `correct`.
    pub mistakes: u32,
}

const HAND_COLS: &str = "id, created_at, mode, config_key, config_toml, seed, hero_seat, \
    ended_reason, total_ev_loss, hero_net, replay, bot_ids";

fn hand_row(r: &Row<'_>) -> rusqlite::Result<HandRow> {
    let replay: Option<String> = r.get(10)?;
    Ok(HandRow {
        id: r.get(0)?,
        created_at: r.get(1)?,
        mode: r.get(2)?,
        config_key: r.get(3)?,
        config_toml: r.get(4)?,
        seed: r.get::<_, i64>(5)? as u64,
        hero_seat: r.get(6)?,
        ended_reason: r.get(7)?,
        total_ev_loss: r.get(8)?,
        hero_net: r.get(9)?,
        replay: replay.and_then(|s| serde_json::from_str(&s).ok()),
        bot_ids: serde_json::from_str(&r.get::<_, String>(11)?).unwrap_or(Value::Null),
    })
}

const DECISION_COLS: &str = "id, created_at, hand_id, node_key, hero_combo, chosen_action, \
    ev_per_action, frequencies, pot, ev_loss, grade, thresholds_version, template_key, spot, \
    street, node_type, texture, step, opponent";

fn decision_row(r: &Row<'_>) -> rusqlite::Result<DecisionRow> {
    let json = |i: usize| -> rusqlite::Result<Value> {
        let s: String = r.get(i)?;
        Ok(serde_json::from_str(&s).unwrap_or(Value::Null))
    };
    Ok(DecisionRow {
        id: r.get(0)?,
        created_at: r.get(1)?,
        hand_id: r.get(2)?,
        node_key: r.get(3)?,
        hero_combo: r.get(4)?,
        chosen_action: r.get(5)?,
        ev_per_action: json(6)?,
        frequencies: json(7)?,
        pot: r.get(8)?,
        ev_loss: r.get(9)?,
        grade: r.get(10)?,
        thresholds_version: r.get(11)?,
        template_key: r.get(12)?,
        spot: r.get(13)?,
        street: r.get(14)?,
        node_type: r.get(15)?,
        texture: r.get(16)?,
        step: r.get(17)?,
        opponent: r.get(18)?,
    })
}

impl Store {
    pub fn insert_hand(&self, h: &NewHand<'_>) -> Result<String> {
        let id = new_id();
        let replay = h.replay.as_ref().map(serde_json::to_string).transpose()?;
        self.conn().execute(
            "INSERT INTO hands (id, created_at, updated_at, mode, config_key, config_toml, seed,
                 hero_seat, bot_ids, library_format_version, replay)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                h.library_format_version,
                replay
            ],
        )?;
        Ok(id)
    }

    /// Marks a hand finished.
    pub fn finish_hand(
        &self,
        id: &str,
        ended_reason: &str,
        total_ev_loss: f64,
        hero_net: Option<f64>,
    ) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE hands SET ended_reason = ?1, total_ev_loss = ?2, hero_net = ?3,
                 updated_at = ?4
             WHERE id = ?5",
            params![ended_reason, total_ev_loss, hero_net, now, id],
        )?;
        Ok(())
    }

    pub fn insert_decision(&self, d: &NewDecision<'_>) -> Result<String> {
        let id = new_id();
        let c = d.context.clone().unwrap_or_default();
        let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
        self.conn().execute(
            "INSERT INTO decisions (id, created_at, updated_at, hand_id, node_key, hero_combo,
                 chosen_action, ev_per_action, frequencies, pot, ev_loss, grade,
                 thresholds_version, template_key, spot, street, node_type, texture, step,
                 opponent)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                 ?16, ?17, ?18, ?19)",
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
                d.thresholds_version,
                some(c.template_key),
                some(c.spot),
                some(c.street),
                some(c.node_type),
                c.texture,
                c.step,
                some(c.opponent)
            ],
        )?;
        Ok(id)
    }

    /// The latest hands of `mode`, newest first.
    pub fn recent_hands(&self, mode: &str, limit: u32) -> Result<Vec<HandRow>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {HAND_COLS} FROM hands
             WHERE deleted_at IS NULL AND mode = ?1 AND ended_reason IS NOT NULL
             ORDER BY created_at DESC, id DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![mode, limit], hand_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn hand(&self, id: &str) -> Result<Option<HandRow>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {HAND_COLS} FROM hands WHERE id = ?1 AND deleted_at IS NULL"),
                params![id],
                hand_row,
            )
            .optional()?)
    }

    /// A hand's decisions in play order.
    pub fn hand_decisions(&self, hand_id: &str) -> Result<Vec<DecisionRow>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {DECISION_COLS} FROM decisions
             WHERE hand_id = ?1 AND deleted_at IS NULL ORDER BY step, created_at, id"
        ))?;
        let rows = stmt.query_map(params![hand_id], decision_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// EV loss per leak bucket over the last `window` decisions that have a
    /// context, worst total first.
    pub fn bucket_stats(&self, window: u32) -> Result<Vec<BucketStats>> {
        let mut stmt = self.conn().prepare(
            "SELECT spot, street, node_type, texture, coalesce(opponent, 'reference'), count(*),
                 sum(ev_loss),
                 sum(CASE WHEN grade IN ('inaccuracy', 'mistake', 'blunder') THEN 1 ELSE 0 END)
             FROM (SELECT * FROM decisions
                   WHERE deleted_at IS NULL AND spot IS NOT NULL
                   ORDER BY created_at DESC, id DESC LIMIT ?1)
             GROUP BY spot, street, node_type, texture, coalesce(opponent, 'reference')
             ORDER BY sum(ev_loss) DESC",
        )?;
        let rows = stmt.query_map(params![window], |r| {
            Ok(BucketStats {
                spot: r.get(0)?,
                street: r.get(1)?,
                node_type: r.get(2)?,
                texture: r.get(3)?,
                opponent: r.get(4)?,
                decisions: r.get(5)?,
                total_ev_loss: r.get(6)?,
                mistakes: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Counts a line that left the library (spec §10.2, `solve_candidates`).
    pub fn note_solve_candidate(&self, config_key: &str, line: &str, history: &str) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "INSERT INTO solve_candidates (id, created_at, updated_at, config_key, line, history,
                 count, last_seen)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5, 1, ?2)
             ON CONFLICT (config_key, line, history) DO UPDATE SET
                 count = count + 1, last_seen = ?2, updated_at = ?2, deleted_at = NULL",
            params![new_id(), now, config_key, line, history],
        )?;
        Ok(())
    }

    /// `(line, history, count)` of the most frequent solve candidates.
    pub fn solve_candidates(&self, limit: u32) -> Result<Vec<(String, String, u32)>> {
        let mut stmt = self.conn().prepare(
            "SELECT line, history, count FROM solve_candidates WHERE deleted_at IS NULL
             ORDER BY count DESC, last_seen DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// Review items to hold back from a due query: those of `kind` whose key
/// matches the SQLite GLOB `key_glob` and not `unless_glob`.
#[derive(Debug, Clone, Copy)]
pub struct Hold<'a> {
    pub kind: &'a str,
    pub key_glob: &'a str,
    pub unless_glob: &'a str,
}

impl<'a> Hold<'a> {
    /// The values of [`HOLD_FILTER`]'s parameters; `None` holds nothing
    /// (a NULL kind matches no row).
    fn params(hold: Option<&Hold<'a>>) -> (Option<&'a str>, &'a str, &'a str) {
        hold.map_or((None, "", ""), |h| {
            (Some(h.kind), h.key_glob, h.unless_glob)
        })
    }
}

/// SQL for a [`Hold`], bound by name: `:hold_kind`, `:key_glob`,
/// `:unless_glob`.
const HOLD_FILTER: &str =
    "NOT (kind IS :hold_kind AND item_key GLOB :key_glob AND item_key NOT GLOB :unless_glob)";

/// `kind IN (...)` for a fixed set of kind ids (never user input; quoted
/// defensively anyway), or `1` for all kinds.
fn kind_filter(kinds: &[&str]) -> String {
    if kinds.is_empty() {
        return "1".into();
    }
    let list: Vec<String> = kinds
        .iter()
        .map(|k| format!("'{}'", k.replace('\'', "''")))
        .collect();
    format!("kind IN ({})", list.join(", "))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{StoreError, test_clock};

    fn store() -> (Store, std::sync::Arc<std::sync::atomic::AtomicI64>) {
        let (clock, t) = test_clock(1_000);
        (Store::open_in_memory(clock).unwrap(), t)
    }

    #[test]
    fn review_history_is_grouped_by_item_in_time_order() {
        let (s, t) = store();
        let a = s.ensure_review_item("calc_exercise", "a", None, 0).unwrap();
        let b = s.ensure_review_item("calc_exercise", "b", None, 0).unwrap();
        let rec = |at: Millis, rating: &'static str| ReviewRecord {
            stability: 1.0,
            difficulty: 5.0,
            due: at + 1,
            reviewed_at: at,
            rating,
            elapsed_days: 0.0,
            lapse: false,
            content_version: None,
            source_kind: None,
            source_id: None,
        };
        // Logged out of order across items.
        s.record_review(&b.id, &rec(300, "good")).unwrap();
        s.record_review(&a.id, &rec(100, "again")).unwrap();
        s.record_review(&a.id, &rec(200, "easy")).unwrap();
        t.store(5_000, std::sync::atomic::Ordering::SeqCst);
        s.delete_review_item(&b.id).unwrap();
        let log = s.review_history().unwrap();
        let got: Vec<_> = log
            .iter()
            .map(|r| (r.item_id.as_str(), r.reviewed_at, r.rating.as_str()))
            .collect();
        let mut want = vec![
            (a.id.as_str(), 100, "again"),
            (a.id.as_str(), 200, "easy"),
            (b.id.as_str(), 300, "good"),
        ];
        want.sort_by(|x, y| (x.0, x.1).cmp(&(y.0, y.1)));
        assert_eq!(got, want, "a deleted item's reviews stay in the log");
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
    fn atomic_keeps_or_drops_everything_and_nests() {
        let (s, _) = store();
        let failed: Result<()> = s.atomic(|s| {
            s.set_setting("a", &json!(1))?;
            Err(StoreError::NotATafulDatabase("x".into()))
        });
        assert!(failed.is_err());
        assert_eq!(s.setting("a").unwrap(), None, "rolled back");
        let ok: Result<()> = s.atomic(|s| {
            s.set_setting("a", &json!(1))?;
            s.atomic(|s| s.set_setting("b", &json!(2)))?;
            Ok(())
        });
        ok.unwrap();
        assert_eq!(s.setting("b").unwrap(), Some(json!(2)));
    }

    #[test]
    fn holds_keep_matching_items_out_of_due_queries() {
        let (s, _) = store();
        for key in ["v1|a|preflop|x", "v1|a|srp|x", "v1|b|srp|y", "old"] {
            s.ensure_review_item("spot_template", key, None, 0).unwrap();
        }
        s.ensure_review_item("calc_exercise", "v1|a|srp|x", None, 0)
            .unwrap();
        let hold = Hold {
            kind: "spot_template",
            key_glob: "v1|*",
            unless_glob: "v1|*|preflop|*",
        };
        assert_eq!(s.count_due(10, &[]).unwrap(), 5);
        assert_eq!(s.count_due_held(10, &[], Some(&hold)).unwrap(), 3);
        let keys: Vec<(String, String)> = s
            .due_items_held(10, &[], Some(&hold), 10)
            .unwrap()
            .into_iter()
            .map(|i| (i.kind, i.key))
            .collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&("spot_template".into(), "v1|a|preflop|x".into())));
        assert!(keys.contains(&("spot_template".into(), "old".into())));
        assert!(keys.contains(&("calc_exercise".into(), "v1|a|srp|x".into())));
        assert_eq!(s.due_items_held(10, &[], Some(&hold), 1).unwrap().len(), 1);
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
        assert_eq!(s.count_due(1_000, &[]).unwrap(), 1);
        assert_eq!(s.count_due(5_000, &["lesson_quiz"]).unwrap(), 1);
        assert_eq!(s.due_items(5_000, &[], 10).unwrap().len(), 2);
        assert_eq!(s.due_items(5_000, &["calc_exercise"], 10).unwrap().len(), 1);
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
        assert_eq!(s.review_item_by_id(&a.id).unwrap(), None, "deleted is gone");
        let revived = s
            .ensure_review_item("calc_exercise", "pot_odds/bet_50", Some(3), 7)
            .unwrap();
        assert_eq!(revived.id, a.id);
        assert_eq!(
            (
                revived.reps,
                revived.stability,
                revived.due,
                revived.content_version
            ),
            (0, None, 7, Some(3)),
            "revived as new"
        );
        assert_eq!(s.review_totals().unwrap(), (2, 1), "the log is kept");
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
        let (s, t) = store();
        let h = s
            .insert_hand(&NewHand {
                mode: "full_hand",
                config_key: "cash6-100bb-r0",
                config_toml: "",
                seed: u64::MAX,
                hero_seat: 2,
                bot_ids: json!(["ref"]),
                library_format_version: Some(1),
                replay: Some(json!({"board": "AsKd7c"})),
            })
            .unwrap();
        fn decision<'a>(hand: Option<&'a str>, grade: &'a str, loss: f64) -> NewDecision<'a> {
            NewDecision {
                hand_id: hand,
                node_key: "n",
                hero_combo: "AsKs",
                chosen_action: "raise",
                ev_per_action: json!([1.0, 2.0]),
                frequencies: json!([0.5, 0.5]),
                pot: 1.5,
                ev_loss: loss,
                grade,
                thresholds_version: 1,
                context: Some(DecisionContext {
                    template_key: "v2|k|btn-bb-srp||A-r-u-0|BB|reference",
                    spot: "btn-bb-srp",
                    street: "flop",
                    node_type: "unopened",
                    texture: Some("A-r-u-0"),
                    step: Some(7),
                    opponent: "reference",
                }),
            }
        }
        let hid = h.as_str();
        s.insert_decision(&decision(Some(hid), "best", 0.0))
            .unwrap();
        t.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        s.insert_decision(&decision(Some(hid), "mistake", 1.25))
            .unwrap();
        assert!(
            s.recent_hands("full_hand", 10).unwrap().is_empty(),
            "unfinished"
        );
        s.finish_hand(&h, "settled", 1.25, Some(-3.0)).unwrap();
        let hands = s.recent_hands("full_hand", 10).unwrap();
        assert_eq!(hands.len(), 1);
        assert_eq!(hands[0].seed, u64::MAX);
        assert_eq!(hands[0].hero_net, Some(-3.0));
        assert_eq!(hands[0].replay, Some(json!({"board": "AsKd7c"})));
        assert_eq!(s.hand(&h).unwrap().unwrap(), hands[0]);
        let ds = s.hand_decisions(&h).unwrap();
        assert_eq!(ds.len(), 2);
        assert_eq!(ds[1].grade, "mistake");
        assert_eq!(ds[1].step, Some(7));
        assert_eq!(ds[1].ev_per_action, json!([1.0, 2.0]));
        assert_eq!(ds[0].texture.as_deref(), Some("A-r-u-0"));

        // Spot-mode decisions count for leaks too; no context = ignored.
        s.insert_decision(&decision(None, "blunder", 3.0)).unwrap();
        s.insert_decision(&NewDecision {
            context: None,
            ..decision(None, "blunder", 9.0)
        })
        .unwrap();
        let b = s.bucket_stats(100).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!((b[0].decisions, b[0].mistakes), (3, 2));
        assert!((b[0].total_ev_loss - 4.25).abs() < 1e-9);
        assert_eq!(s.bucket_stats(1).unwrap()[0].decisions, 1, "window");
        assert_eq!(b[0].opponent, "reference");
        assert_eq!(ds[0].opponent.as_deref(), Some("reference"));

        // The opponent profile is a bucket dimension.
        let mut vs_nit = decision(None, "mistake", 0.5);
        if let Some(c) = vs_nit.context.as_mut() {
            c.opponent = "nit";
        }
        s.insert_decision(&vs_nit).unwrap();
        let b = s.bucket_stats(100).unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!((b[1].opponent.as_str(), b[1].decisions), ("nit", 1));
        assert_eq!((b[0].opponent.as_str(), b[0].decisions), ("reference", 3));

        // Foreign keys are enforced.
        assert!(
            s.insert_decision(&decision(Some("nope"), "best", 0.0))
                .is_err()
        );
    }

    #[test]
    fn solve_candidates_count() {
        let (s, _) = store();
        s.note_solve_candidate("k", "UTG:r250,HJ:c", "").unwrap();
        s.note_solve_candidate("k", "UTG:r250,HJ:c", "").unwrap();
        s.note_solve_candidate("k", "BTN:r250,BB:c", "BB:x")
            .unwrap();
        let c = s.solve_candidates(10).unwrap();
        assert_eq!(c[0], ("UTG:r250,HJ:c".into(), "".into(), 2));
        assert_eq!(c.len(), 2);
    }
}

// ---- layouts (spec §15.3) ---------------------------------------------------------------

/// A user's saved layout (built-in presets are not stored).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRow {
    pub id: String,
    pub name: String,
    pub schema_version: u32,
    pub json: String,
}

impl Store {
    /// Saves a layout; returns its id.
    pub fn insert_layout(&self, name: &str, schema_version: u32, json: &str) -> Result<String> {
        let id = new_id();
        let now = self.now();
        self.conn().execute(
            "INSERT INTO layouts (id, created_at, updated_at, name, schema_version, json)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5)",
            params![id, now, name, schema_version, json],
        )?;
        Ok(id)
    }

    /// Saved layouts, oldest first.
    pub fn layouts(&self) -> Result<Vec<LayoutRow>> {
        let mut st = self.conn().prepare(
            "SELECT id, name, schema_version, json FROM layouts
             WHERE deleted_at IS NULL ORDER BY created_at, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(LayoutRow {
                id: r.get(0)?,
                name: r.get(1)?,
                schema_version: r.get(2)?,
                json: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Soft-deletes a saved layout.
    pub fn delete_layout(&self, id: &str) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE layouts SET deleted_at = ?1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    }
}

// ---- themes (spec §15.4, D7) ------------------------------------------------------------

/// A user's theme (built-in themes are not stored).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRow {
    pub id: String,
    pub name: String,
    pub schema_version: u32,
    pub json: String,
}

impl Store {
    /// Saves a new theme; returns its id.
    pub fn insert_theme(&self, name: &str, schema_version: u32, json: &str) -> Result<String> {
        let id = new_id();
        let now = self.now();
        self.conn().execute(
            "INSERT INTO themes (id, created_at, updated_at, name, schema_version, json)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5)",
            params![id, now, name, schema_version, json],
        )?;
        Ok(id)
    }

    /// Replaces a theme's name and tokens. `false` if no live theme has `id`.
    pub fn update_theme(
        &self,
        id: &str,
        name: &str,
        schema_version: u32,
        json: &str,
    ) -> Result<bool> {
        let now = self.now();
        let n = self.conn().execute(
            "UPDATE themes SET name = ?1, schema_version = ?2, json = ?3, updated_at = ?4
             WHERE id = ?5 AND deleted_at IS NULL",
            params![name, schema_version, json, now, id],
        )?;
        Ok(n > 0)
    }

    /// User themes, oldest first.
    pub fn themes(&self) -> Result<Vec<ThemeRow>> {
        let mut st = self.conn().prepare(
            "SELECT id, name, schema_version, json FROM themes
             WHERE deleted_at IS NULL ORDER BY created_at, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(ThemeRow {
                id: r.get(0)?,
                name: r.get(1)?,
                schema_version: r.get(2)?,
                json: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A live user theme by id.
    pub fn theme(&self, id: &str) -> Result<Option<ThemeRow>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, name, schema_version, json FROM themes
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok(ThemeRow {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        schema_version: r.get(2)?,
                        json: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    /// Soft-deletes a user theme.
    pub fn delete_theme(&self, id: &str) -> Result<()> {
        let now = self.now();
        self.conn().execute(
            "UPDATE themes SET deleted_at = ?1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod theme_tests {
    use crate::{Store, test_clock};

    fn store() -> (Store, std::sync::Arc<std::sync::atomic::AtomicI64>) {
        let (clock, t) = test_clock(1_000);
        (Store::open_in_memory(clock).unwrap(), t)
    }

    #[test]
    fn themes_crud() {
        let (s, t) = store();
        assert!(s.themes().unwrap().is_empty());
        let a = s.insert_theme("Mine", 2, "{\"a\":1}").unwrap();
        t.store(5_000, std::sync::atomic::Ordering::SeqCst);
        let b = s.insert_theme("Other", 2, "{}").unwrap();
        let names: Vec<_> = s.themes().unwrap().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["Mine", "Other"]);

        assert!(s.update_theme(&a, "Renamed", 2, "{\"a\":2}").unwrap());
        let row = s.theme(&a).unwrap().unwrap();
        assert_eq!(
            (row.name.as_str(), row.json.as_str()),
            ("Renamed", "{\"a\":2}")
        );
        assert!(!s.update_theme("nope", "x", 2, "{}").unwrap());

        s.delete_theme(&b).unwrap();
        assert_eq!(s.theme(&b).unwrap(), None);
        assert!(
            !s.update_theme(&b, "x", 2, "{}").unwrap(),
            "deleted rows stay deleted"
        );
        assert_eq!(s.themes().unwrap().len(), 1);
    }
}

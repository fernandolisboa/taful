//! Read-only aggregates for dashboards: decisions over time, the review
//! forecast and recent calc attempts. Like the rest of the crate, nothing
//! here knows the domain: grades and actions stay strings.

use rusqlite::named_params;
use serde_json::Value;

use crate::repo::{HOLD_FILTER, Hold, kind_filter};
use crate::{Millis, Result, Store};

/// One decision reduced to what a chart needs.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionStat {
    pub created_at: Millis,
    /// Grade id (`best`, …, `blunder`).
    pub grade: String,
    /// EV lost, bb.
    pub ev_loss: f64,
}

/// One preflop decision as stored: what hero held and did, and what the
/// reference does with that hand.
#[derive(Debug, Clone, PartialEq)]
pub struct PreflopDecisionStat {
    /// Hero's two cards (`AsKs`).
    pub hero_combo: String,
    /// The action taken, in its short form (`f`, `c`, `r250`, …).
    pub chosen_action: String,
    /// `{"actions": [...], "ev": [...]}` as stored with the decision.
    pub ev_per_action: Value,
    /// `{"hand": [...], "range": [...]}` as stored with the decision.
    pub frequencies: Value,
    /// Preflop node type id (`rfi`, `vs-open`, …).
    pub node_type: String,
}

impl Store {
    /// Rows changed through this store since it was opened: a cheap "did
    /// anything change" stamp for whoever caches the aggregates below.
    pub fn change_count(&self) -> u64 {
        self.conn().total_changes()
    }

    /// Every decision created at or after `since`, oldest first.
    pub fn decision_stats_since(&self, since: Millis) -> Result<Vec<DecisionStat>> {
        let mut stmt = self.conn().prepare(
            "SELECT created_at, grade, ev_loss FROM decisions
             WHERE deleted_at IS NULL AND created_at >= ?1
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([since], |r| {
            Ok(DecisionStat {
                created_at: r.get(0)?,
                grade: r.get(1)?,
                ev_loss: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Grade ids of the latest `limit` decisions, newest first. A hand's
    /// decisions are written together and can share a timestamp, and ids
    /// are not ordered within a millisecond: ties go by insertion order.
    pub fn recent_grades(&self, limit: u32) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT grade FROM decisions WHERE deleted_at IS NULL
             ORDER BY created_at DESC, rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The latest `limit` preflop decisions against the reference that have
    /// a context, newest first. Decisions against a bot profile are left
    /// out: they are graded on the best response to that profile, so their
    /// distance from the reference mix is not a deviation.
    pub fn recent_preflop_decisions(&self, limit: u32) -> Result<Vec<PreflopDecisionStat>> {
        let mut stmt = self.conn().prepare(
            "SELECT hero_combo, chosen_action, ev_per_action, frequencies, node_type
             FROM decisions
             WHERE deleted_at IS NULL AND street = 'preflop' AND node_type IS NOT NULL
               AND coalesce(opponent, 'reference') = 'reference'
             ORDER BY created_at DESC, rowid DESC LIMIT ?1",
        )?;
        let json = |s: String| serde_json::from_str(&s).unwrap_or(Value::Null);
        let rows = stmt.query_map([limit], |r| {
            Ok(PreflopDecisionStat {
                hero_combo: r.get(0)?,
                chosen_action: r.get(1)?,
                ev_per_action: json(r.get(2)?),
                frequencies: json(r.get(3)?),
                node_type: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Number of review items of `kinds` (all kinds when empty) that become
    /// due after `from` and at or before `to`, without the items `hold`
    /// matches.
    pub fn count_due_between(
        &self,
        from: Millis,
        to: Millis,
        kinds: &[&str],
        hold: Option<&Hold<'_>>,
    ) -> Result<u32> {
        let (k, g, u) = Hold::params(hold);
        Ok(self.conn().query_row(
            &format!(
                "SELECT count(*) FROM review_items
                 WHERE deleted_at IS NULL AND due > :from AND due <= :to
                   AND {} AND {HOLD_FILTER}",
                kind_filter(kinds)
            ),
            named_params! {
                ":from": from, ":to": to, ":hold_kind": k, ":key_glob": g, ":unless_glob": u
            },
            |r| r.get(0),
        )?)
    }

    /// Mean best quiz score (0–1) over the lessons that have one, with the
    /// number of such lessons. A lesson's best is its best over versions.
    pub fn mean_best_quiz_score(&self) -> Result<Option<(f64, u32)>> {
        let (sum, n): (Option<f64>, u32) = self.conn().query_row(
            "SELECT sum(best), count(*) FROM (
                 SELECT max(best_score) AS best FROM lesson_progress
                 WHERE deleted_at IS NULL AND best_score IS NOT NULL
                 GROUP BY lesson_id)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(sum.filter(|_| n > 0).map(|s| (s / f64::from(n), n)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use serde_json::json;

    use crate::{DecisionContext, Hold, LessonState, NewDecision, Store};

    fn store() -> (Store, Arc<AtomicI64>) {
        let (clock, t) = crate::test_clock(1_000);
        (Store::open_in_memory(clock).unwrap(), t)
    }

    fn decision(s: &Store, grade: &str, ev_loss: f64, street: &str) {
        decision_against(s, grade, ev_loss, street, "reference");
    }

    fn decision_against(s: &Store, grade: &str, ev_loss: f64, street: &str, opponent: &str) {
        s.insert_decision(&NewDecision {
            hand_id: None,
            node_key: "n",
            hero_combo: "AsKs",
            chosen_action: "f",
            ev_per_action: json!({ "actions": ["f", "r250"], "ev": [0.0, 1.0] }),
            frequencies: json!({ "hand": [0.0, 1.0], "range": [0.5, 0.5] }),
            pot: 1.5,
            ev_loss,
            grade,
            thresholds_version: 1,
            context: Some(DecisionContext {
                template_key: "t",
                spot: "preflop",
                street,
                node_type: "rfi",
                texture: None,
                step: None,
                opponent,
            }),
        })
        .unwrap();
    }

    #[test]
    fn decisions_of_one_instant_keep_their_order() {
        // A hand's decisions are written in one go, on one timestamp.
        let (s, _) = store();
        for grade in ["best", "mistake", "correct", "blunder", "inaccuracy"] {
            decision(&s, grade, 0.0, "preflop");
        }
        assert_eq!(
            s.recent_grades(5).unwrap(),
            ["inaccuracy", "blunder", "correct", "mistake", "best"]
        );
    }

    #[test]
    fn deviation_rows_are_against_the_reference_only() {
        let (s, _) = store();
        decision(&s, "best", 0.0, "preflop");
        decision_against(&s, "best", 0.0, "preflop", "nit");
        assert_eq!(s.recent_preflop_decisions(10).unwrap().len(), 1);
    }

    #[test]
    fn decisions_over_time() {
        let (s, t) = store();
        decision(&s, "best", 0.0, "preflop");
        t.store(2_000, Ordering::SeqCst);
        decision(&s, "mistake", 0.4, "flop");
        t.store(3_000, Ordering::SeqCst);
        decision(&s, "correct", 0.01, "preflop");

        let all = s.decision_stats_since(0).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].grade, "best");
        assert_eq!(all[1].ev_loss, 0.4);
        assert_eq!(s.decision_stats_since(2_000).unwrap().len(), 2);
        assert_eq!(s.decision_stats_since(3_001).unwrap().len(), 0);

        assert_eq!(s.recent_grades(2).unwrap(), ["correct", "mistake"]);

        let pre = s.recent_preflop_decisions(10).unwrap();
        assert_eq!(pre.len(), 2, "the flop decision is left out");
        assert_eq!(pre[0].hero_combo, "AsKs");
        assert_eq!(pre[0].node_type, "rfi");
        assert_eq!(pre[0].frequencies["hand"][1], 1.0);
    }

    #[test]
    fn change_count_moves_on_writes_only() {
        let (s, _) = store();
        decision(&s, "best", 0.0, "preflop");
        let stamp = s.change_count();
        s.recent_grades(5).unwrap();
        assert_eq!(s.change_count(), stamp);
        decision(&s, "best", 0.0, "preflop");
        assert!(s.change_count() > stamp);
    }

    #[test]
    fn due_between() {
        let (s, _) = store();
        for (kind, key, due) in [
            ("calc_exercise", "a", 500),
            ("calc_exercise", "b", 1_500),
            ("lesson_quiz", "c", 2_500),
            ("spot_template", "d", 2_600),
        ] {
            s.ensure_review_item(kind, key, None, due).unwrap();
        }
        assert_eq!(s.count_due_between(1_000, 3_000, &[], None).unwrap(), 3);
        assert_eq!(s.count_due_between(1_000, 2_500, &[], None).unwrap(), 2);
        assert_eq!(
            s.count_due_between(0, 3_000, &["calc_exercise"], None)
                .unwrap(),
            2
        );
        // Held items are not forecast.
        let hold = Hold {
            kind: "spot_template",
            key_glob: "*",
            unless_glob: "",
        };
        assert_eq!(
            s.count_due_between(1_000, 3_000, &[], Some(&hold)).unwrap(),
            2
        );
    }

    #[test]
    fn quiz_mean() {
        let (s, _) = store();
        assert_eq!(s.mean_best_quiz_score().unwrap(), None);
        s.record_lesson_progress("l1", 1, LessonState::Done, Some(0.8))
            .unwrap();
        s.record_lesson_progress("l1", 2, LessonState::Done, Some(1.0))
            .unwrap();
        s.record_lesson_progress("l2", 1, LessonState::Done, Some(0.9))
            .unwrap();
        s.record_lesson_progress("l3", 1, LessonState::Available, None)
            .unwrap();
        let (mean, n) = s.mean_best_quiz_score().unwrap().unwrap();
        assert_eq!(n, 2);
        assert!((mean - 0.95).abs() < 1e-9);
    }
}

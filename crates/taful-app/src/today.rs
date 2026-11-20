//! The day at a glance: the next lesson with its place in its track, the
//! reviews due now and tomorrow, the biggest leak with its recent grades,
//! and the mental-math pace. The Baralho style's home (D19) deals these
//! as four cards.

use taful_core::calc::ExerciseKind;
use taful_store::{LessonState, Millis};

use crate::i18n::Locale;
use crate::lessons::TrackView;
use crate::model::{Grade, Leak};
use crate::template::{opponent_id, street_id};
use crate::training::LEAK_WINDOW;
use crate::{App, Result};

/// Seconds one review takes, for the "about N minutes" estimate.
pub const REVIEW_SECS: u32 = 30;
/// Calc answers the pace is measured over.
const CALC_RECENT: u32 = 50;
/// The kinds suggested before there is anything to measure.
const CALC_DEFAULT: [ExerciseKind; 3] = [
    ExerciseKind::PotOdds,
    ExerciseKind::Outs,
    ExerciseKind::EquityVsRange,
];

/// A local calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// 1 = Monday … 7 = Sunday.
    pub weekday: u8,
    pub day: u8,
    /// 1 = January … 12 = December.
    pub month: u8,
}

/// Recent mental-math answers: how fast and how often right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalcPace {
    pub mean_secs: f64,
    /// 0–1.
    pub accuracy: f64,
    /// The target time, averaged over the same answers' exercise kinds.
    pub target_secs: f64,
}

/// What the home screen's cards show.
#[derive(Debug, Clone, PartialEq)]
pub struct Today {
    pub date: Date,
    pub tracks: Vec<TrackView>,
    /// The first available lesson as `(track, lesson)` indexes into `tracks`.
    pub next_lesson: Option<(usize, usize)>,
    pub due_now: u32,
    /// Reviews that fall due during tomorrow.
    pub due_tomorrow: u32,
    pub next_due: Option<Millis>,
    /// The biggest leak and the grades of its decisions, oldest first.
    pub leak: Option<(Leak, Vec<Grade>)>,
    pub calc: Option<CalcPace>,
    /// Three exercise kinds to practice: the least accurate recently.
    pub calc_focus: [ExerciseKind; 3],
}

impl Today {
    pub fn lessons_done(&self) -> usize {
        self.tracks
            .iter()
            .flat_map(|t| &t.lessons)
            .filter(|l| l.state == LessonState::Done)
            .count()
    }

    pub fn lessons_total(&self) -> usize {
        self.tracks.iter().map(|t| t.lessons.len()).sum()
    }

    /// Minutes the due reviews take, at least one.
    pub fn review_minutes(&self) -> u32 {
        (self.due_now * REVIEW_SECS).div_ceil(60).max(1)
    }
}

/// The local date at `ms`, and when the next two days start.
fn local_day(ms: Millis) -> Option<(Date, Millis, Millis)> {
    let tz = jiff::tz::TimeZone::system();
    let z = jiff::Timestamp::from_millisecond(ms)
        .ok()?
        .to_zoned(tz.clone());
    let tomorrow = z.date().tomorrow().ok()?;
    let start =
        |d: jiff::civil::Date| Some(d.to_zoned(tz.clone()).ok()?.timestamp().as_millisecond());
    Some((
        Date {
            weekday: z.weekday().to_monday_one_offset() as u8,
            day: z.day() as u8,
            month: z.month() as u8,
        },
        start(tomorrow)?,
        start(tomorrow.tomorrow().ok()?)?,
    ))
}

impl App {
    pub fn today(&mut self, locale: Locale) -> Result<Today> {
        let now = self.store.now();
        let (date, tomorrow, day_after) = local_day(now).unwrap_or((
            Date {
                weekday: 1,
                day: 1,
                month: 1,
            },
            now,
            now,
        ));
        let tracks = self.skill_tree(locale)?;
        let next_lesson = tracks.iter().enumerate().find_map(|(t, track)| {
            let l = track
                .lessons
                .iter()
                .position(|l| l.state == LessonState::Available)?;
            Some((t, l))
        });
        let kinds = crate::review_session::PRESENTABLE;
        let due_at = |t: Millis| self.store.count_due(t, kinds);
        let due_tomorrow = due_at(day_after)?.saturating_sub(due_at(tomorrow)?);

        // The lessons and reviews still show when the leaks cannot be read
        // (a library that fails to open).
        let leak = match self.leaks().unwrap_or_default().into_iter().next() {
            Some(leak) => {
                let b = &leak.bucket;
                let raw = self.store.bucket_stats(LEAK_WINDOW)?.into_iter().find(|r| {
                    r.spot == b.spot
                        && r.street == street_id(b.street)
                        && r.node_type == b.node_type.id()
                        && r.texture == b.texture
                        && r.opponent == opponent_id(b.opponent.as_deref())
                });
                let grades = match raw {
                    Some(raw) => self
                        .store
                        .bucket_grades(LEAK_WINDOW, &raw)?
                        .iter()
                        .filter_map(|id| Grade::ALL.into_iter().find(|g| g.id() == id))
                        .collect(),
                    None => Vec::new(),
                };
                Some((leak, grades))
            }
            None => None,
        };

        let recent = self.store.recent_calc_attempts(CALC_RECENT)?;
        let kind_of = |id: &str| ExerciseKind::ALL.into_iter().find(|k| k.id() == id);
        let calc = (!recent.is_empty()).then(|| {
            let n = recent.len() as f64;
            let targets: Vec<f64> = recent
                .iter()
                .filter_map(|a| kind_of(&a.exercise))
                .map(|k| self.calc.get(k).target_secs)
                .collect();
            CalcPace {
                mean_secs: recent.iter().map(|a| a.duration_ms as f64).sum::<f64>() / n / 1000.,
                accuracy: recent.iter().filter(|a| a.correct).count() as f64 / n,
                target_secs: if targets.is_empty() {
                    0.
                } else {
                    targets.iter().sum::<f64>() / targets.len() as f64
                },
            }
        });
        // Least accurate first; kinds not tried lately keep the default order.
        let mut focus: Vec<(ExerciseKind, f64)> = ExerciseKind::ALL
            .into_iter()
            .filter_map(|k| {
                let of_kind: Vec<_> = recent.iter().filter(|a| a.exercise == k.id()).collect();
                let right = of_kind.iter().filter(|a| a.correct).count();
                (!of_kind.is_empty()).then(|| (k, right as f64 / of_kind.len() as f64))
            })
            .collect();
        focus.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut picks: Vec<ExerciseKind> = focus.into_iter().map(|(k, _)| k).collect();
        for k in CALC_DEFAULT.into_iter().chain(ExerciseKind::ALL) {
            if !picks.contains(&k) {
                picks.push(k);
            }
        }

        Ok(Today {
            date,
            tracks,
            next_lesson,
            due_now: self.store.count_due(now, kinds)?,
            due_tomorrow,
            next_due: self.store.next_due_after(now)?,
            leak,
            calc,
            calc_focus: [picks[0], picks[1], picks[2]],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tests::app_at;

    #[test]
    fn a_fresh_profile_has_a_first_lesson_and_nothing_else() {
        let (mut a, _) = app_at(1_800_000_000_000);
        let t = a.today(Locale::SOURCE).unwrap();
        assert_eq!(t.next_lesson, Some((0, 0)));
        assert_eq!(t.lessons_done(), 0);
        assert!(t.lessons_total() > 0);
        assert_eq!((t.due_now, t.due_tomorrow), (0, 0));
        assert!(t.leak.is_none() && t.calc.is_none());
        assert_eq!(t.calc_focus, CALC_DEFAULT);
        assert!((1..=7).contains(&t.date.weekday));
        assert!((1..=12).contains(&t.date.month));
    }

    #[test]
    fn calc_pace_follows_the_answers() {
        let (mut a, clock) = app_at(1_800_000_000_000);
        for (i, right) in [true, true, false, true].into_iter().enumerate() {
            let round = a.calc_start(Some(ExerciseKind::PotOdds));
            clock.fetch_add(4_000, std::sync::atomic::Ordering::SeqCst);
            let exact = round.exercise.answer;
            let given = if right { exact } else { exact + 40. + i as f64 };
            a.calc_submit(&round, Some(given)).unwrap();
        }
        let t = a.today(Locale::SOURCE).unwrap();
        let pace = t.calc.expect("answers were stored");
        assert!((pace.mean_secs - 4.).abs() < 0.01, "{pace:?}");
        assert!((pace.accuracy - 0.75).abs() < 1e-9);
        assert!((pace.target_secs - a.calc.pot_odds.target_secs).abs() < 1e-9);
        assert_eq!(t.calc_focus[0], ExerciseKind::PotOdds);
    }

    #[test]
    fn reviews_split_into_now_and_tomorrow() {
        let (mut a, clock) = app_at(1_800_000_000_000);
        // A wrong calc answer queues its skill for review.
        let round = a.calc_start(Some(ExerciseKind::Outs));
        a.calc_submit(&round, Some(round.exercise.answer + 50.))
            .unwrap();
        let later = a.today(Locale::SOURCE).unwrap();
        assert_eq!(later.due_now, 0);
        let due = later.next_due.expect("an item was queued");
        clock.store(due, std::sync::atomic::Ordering::SeqCst);
        let t = a.today(Locale::SOURCE).unwrap();
        assert_eq!(t.due_now, 1);
        assert_eq!(t.review_minutes(), 1);
    }
}

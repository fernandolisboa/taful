//! Numbers for a dashboard home and a status bar: decisions per day and
//! grade, accuracy and EV-loss trends, the review forecast, mental-math
//! pace, lesson progress and where preflop play leaves the reference.
//!
//! Everything is read from what the app already stores (decisions, review
//! items, calc attempts, lesson progress); nothing here writes. A "day" is
//! a calendar day in the system time zone.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use taful_core::calc::ExerciseKind;
use taful_core::cards::Hand;
use taful_core::grading::Grade;
pub use taful_core::range::HandClass;
use taful_store::{LessonState, Millis};

use crate::i18n::Locale;
use crate::preflop::SpotKind;
use crate::template::NodeType;
use crate::training::Leak;
use crate::{App, Result};

/// Days in the decisions-by-grade chart and the trend lines.
pub const CHART_DAYS: usize = 14;
/// Days in a trend's headline number (and in the span it is compared to).
pub const TREND_DAYS: usize = 7;
/// Days in the review forecast, today first.
pub const FORECAST_DAYS: usize = 7;
/// Leaks listed on the dashboard.
pub const DASHBOARD_LEAKS: usize = 5;
/// Latest grades shown as dots in the status bar.
pub const LAST_GRADES: u32 = 12;
/// Preflop decisions the deviation map looks at (the latest ones).
pub const DEVIATION_WINDOW: u32 = 1000;
/// Hands listed as "most deviated".
pub const TOP_DEVIATIONS: usize = 5;
/// Calc attempts behind the mental-math numbers (the latest ones).
pub const CALC_WINDOW: u32 = 50;
/// Points in the mental-math pace line.
const CALC_POINTS: usize = 10;
/// Seconds one review takes, for the "about N min" estimate.
const SECS_PER_REVIEW: u32 = 30;

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    pub year: i16,
    /// 1–12.
    pub month: i8,
    /// 1–31.
    pub day: i8,
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u8,
}

impl Day {
    fn of(d: Date) -> Day {
        Day {
            year: d.year(),
            month: d.month(),
            day: d.day(),
            weekday: d.weekday().to_monday_zero_offset() as u8,
        }
    }
}

/// Today's numbers (status bar).
#[derive(Debug, Clone, PartialEq)]
pub struct Today {
    pub day: Day,
    pub decisions: u32,
    /// Share of today's decisions graded best or correct, 0–1.
    pub accuracy: Option<f64>,
    /// EV lost today, bb.
    pub ev_lost_bb: f64,
    /// The latest grades (any day), oldest first.
    pub last: Vec<Grade>,
}

/// One day of the decisions-by-grade chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayGrades {
    pub day: Day,
    /// Decisions per grade, in [`Grade::ALL`] order (best first).
    pub counts: [u32; 5],
}

impl DayGrades {
    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }
}

/// A number over the last [`TREND_DAYS`] days, against the span before.
#[derive(Debug, Clone, PartialEq)]
pub struct Trend {
    /// `None` without decisions in the span.
    pub value: Option<f64>,
    /// `value` minus the previous span's value, when both exist.
    pub delta: Option<f64>,
    /// One point per day over [`CHART_DAYS`], oldest first; `None` for a
    /// day without decisions.
    pub series: Vec<Option<f64>>,
}

/// Reviews due now and over the next days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviews {
    pub due_now: u32,
    /// Rough time to clear what is due now.
    pub minutes: u32,
    pub tomorrow: u32,
    /// Due now plus everything coming due in the forecast.
    pub week: u32,
    /// Reviews per day, today (what is due now and later today) first.
    pub forecast: [u32; FORECAST_DAYS],
    /// The forecast's days.
    pub days: [Day; FORECAST_DAYS],
}

/// Mental-math pace over the latest [`CALC_WINDOW`] attempts.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcPace {
    pub attempts: u32,
    /// Mean answer time, seconds.
    pub mean_secs: Option<f64>,
    /// Share answered right, 0–1.
    pub accuracy: Option<f64>,
    /// Mean target time of the attempted exercises, seconds.
    pub target_secs: Option<f64>,
    /// Mean answer time of consecutive groups of attempts, oldest first.
    pub series: Vec<f64>,
}

/// What a preflop decision did, for the deviation labels. A raise is named
/// by what it is in its spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayKind {
    Fold,
    /// Call or check.
    Call,
    Open,
    ThreeBet,
    FourBet,
    /// A raise over a 4-bet.
    Jam,
    /// Any other raise (blind against blind, its all-in included).
    Raise,
}

impl PlayKind {
    pub const ALL: [PlayKind; 7] = [
        PlayKind::Fold,
        PlayKind::Call,
        PlayKind::Open,
        PlayKind::ThreeBet,
        PlayKind::FourBet,
        PlayKind::Jam,
        PlayKind::Raise,
    ];

    /// Stable id (i18n key part).
    pub fn id(self) -> &'static str {
        match self {
            PlayKind::Fold => "fold",
            PlayKind::Call => "call",
            PlayKind::Open => "open",
            PlayKind::ThreeBet => "threebet",
            PlayKind::FourBet => "fourbet",
            PlayKind::Jam => "jam",
            PlayKind::Raise => "raise",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).expect("listed")
    }

    /// The kind of stored action `code` (`f`, `x`, `c`, `b…`, `r…`, `a`)
    /// at a node of type `node`.
    fn of(code: &str, node: Option<SpotKind>) -> PlayKind {
        match code.as_bytes().first() {
            Some(b'f') => PlayKind::Fold,
            Some(b'x' | b'c') => PlayKind::Call,
            _ => match node {
                Some(SpotKind::Rfi) => PlayKind::Open,
                Some(SpotKind::VsOpen | SpotKind::Squeeze) => PlayKind::ThreeBet,
                Some(SpotKind::Vs3Bet) => PlayKind::FourBet,
                Some(SpotKind::Vs4Bet | SpotKind::VsAllIn) => PlayKind::Jam,
                Some(SpotKind::BlindVsBlind) | None => PlayKind::Raise,
            },
        }
    }
}

/// One hand class of the deviation map.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DeviationCell {
    /// Preflop decisions with a hand of this class.
    pub decisions: u32,
    /// How far hero's play is from the reference with this class, 0–1:
    /// the total-variation distance between what hero did and what the
    /// reference does at the same nodes, shrunk toward 0 for few decisions
    /// (one decision cannot tell a mistake from the reference's own mixing).
    pub distance: f32,
    /// `distance` as a heat step, 0 (same as the reference) to 4.
    pub level: u8,
}

/// A hand class hero plays unlike the reference, and how.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopDeviation {
    pub class: HandClass,
    /// The play hero's frequency is furthest off with.
    pub play: PlayKind,
    /// Hero does it more often than the reference (else less often).
    pub too_much: bool,
    pub decisions: u32,
    pub distance: f32,
}

/// Where hero's preflop play leaves the reference, by hand class.
#[derive(Debug, Clone, PartialEq)]
pub struct Deviation {
    /// Decisions looked at.
    pub decisions: u32,
    /// One cell per class, in [`HandClass::index`] order.
    pub cells: Vec<DeviationCell>,
    /// The most deviated classes, worst first.
    pub top: Vec<TopDeviation>,
}

/// Heat steps: a cell's level is the number of bounds its distance reaches.
const LEVEL_BOUNDS: [f32; 4] = [0.08, 0.2, 0.35, 0.5];
/// Two shares closer than this are a tie.
const TIE: f32 = 1e-4;

/// The next lesson to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextLesson {
    pub id: String,
    pub title: String,
    /// The level's id (title: Fluent `level-<id>`).
    pub level: String,
    /// 1-based position in its level.
    pub number: usize,
    pub level_len: usize,
    pub minutes: Option<u32>,
}

/// Lesson progress across the whole catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonsSummary {
    pub done: usize,
    pub total: usize,
    /// The catalog grouped as the lessons screen shows it.
    pub list: crate::lessons::LessonList,
    /// The Jornada's levels with lessons, for the home.
    pub levels: Vec<crate::lessons::LessonGroup>,
    /// Reading time of the lessons not done yet, minutes.
    pub minutes_left: u32,
    /// Mean best quiz score over lessons with one, 0–1.
    pub quiz_mean: Option<f64>,
    pub next: Option<NextLesson>,
}

/// Everything the dashboard shows that is not lessons.
#[derive(Debug, Clone, PartialEq)]
pub struct Dashboard {
    pub today: Today,
    pub reviews: Reviews,
    /// Share of decisions graded best or correct, 0–1; delta in the same
    /// unit (0.04 = four points).
    pub accuracy: Trend,
    /// EV lost per decision, bb.
    pub ev_loss: Trend,
    pub calc: CalcPace,
    /// [`CHART_DAYS`] days, oldest first; the last one is today.
    pub days: Vec<DayGrades>,
    /// The top [`DASHBOARD_LEAKS`] leaks, worst first.
    pub leaks: Vec<Leak>,
    pub deviation: Deviation,
}

fn grade_index(id: &str) -> Option<usize> {
    Grade::ALL.iter().position(|g| g.id() == id)
}

fn is_accurate(grade: usize) -> bool {
    grade
        <= Grade::ALL
            .iter()
            .position(|g| *g == Grade::Correct)
            .expect("listed")
}

/// Midnight starting `date` in `tz`, Unix milliseconds.
fn day_start(date: Date, tz: &TimeZone) -> Millis {
    tz.to_zoned(date.at(0, 0, 0, 0))
        .map(|z| z.timestamp().as_millisecond())
        .unwrap_or(0)
}

fn local_date(ms: Millis, tz: &TimeZone) -> Date {
    Timestamp::from_millisecond(ms)
        .unwrap_or(Timestamp::UNIX_EPOCH)
        .to_zoned(tz.clone())
        .date()
}

fn plus_days(d: Date, n: i64) -> Date {
    d.checked_add(n.days()).unwrap_or(d)
}

/// Sums over a set of decisions.
#[derive(Default, Clone, Copy)]
struct Tally {
    n: u32,
    accurate: u32,
    ev_loss: f64,
}

impl Tally {
    fn add(&mut self, grade: usize, ev_loss: f64) {
        self.n += 1;
        self.accurate += u32::from(is_accurate(grade));
        self.ev_loss += ev_loss;
    }

    fn accuracy(&self) -> Option<f64> {
        (self.n > 0).then(|| f64::from(self.accurate) / f64::from(self.n))
    }

    fn ev_per_decision(&self) -> Option<f64> {
        (self.n > 0).then(|| self.ev_loss / f64::from(self.n))
    }
}

fn trend(days: &[Tally], value: impl Fn(&Tally) -> Option<f64>) -> Trend {
    let span = |range: std::ops::Range<usize>| {
        let mut t = Tally::default();
        for d in &days[range] {
            t.n += d.n;
            t.accurate += d.accurate;
            t.ev_loss += d.ev_loss;
        }
        value(&t)
    };
    let n = days.len();
    let now = span(n - TREND_DAYS..n);
    let before = span(n - 2 * TREND_DAYS..n - TREND_DAYS);
    Trend {
        value: now,
        delta: now.zip(before).map(|(a, b)| a - b),
        series: days.iter().map(value).collect(),
    }
}

impl App {
    /// A stamp that moves when the app writes rows to its database: a view
    /// that caches the numbers below reads them again when it changes. A
    /// restored backup replaces the file without moving it, so a view also
    /// lets its numbers age out.
    pub fn data_stamp(&self) -> u64 {
        self.store.change_count()
    }

    /// The dashboard's numbers as of now, in the system time zone.
    pub fn dashboard(&mut self) -> Result<Dashboard> {
        self.dashboard_in(&TimeZone::system())
    }

    /// Today's numbers alone, for the status bar (two queries over the
    /// day's decisions; a view caches them on [`App::data_stamp`]).
    pub fn today_numbers(&self) -> Result<Today> {
        self.today_numbers_in(&TimeZone::system())
    }

    fn last_grades(&self) -> Result<Vec<Grade>> {
        let mut last: Vec<Grade> = self
            .store
            .recent_grades(LAST_GRADES)?
            .iter()
            .filter_map(|id| grade_index(id).map(|i| Grade::ALL[i]))
            .collect();
        last.reverse();
        Ok(last)
    }

    pub(crate) fn today_numbers_in(&self, tz: &TimeZone) -> Result<Today> {
        let today = local_date(self.now(), tz);
        let mut tally = Tally::default();
        for d in self.store.decision_stats_since(day_start(today, tz))? {
            if let Some(g) = grade_index(&d.grade) {
                tally.add(g, d.ev_loss);
            }
        }
        Ok(Today {
            day: Day::of(today),
            decisions: tally.n,
            accuracy: tally.accuracy(),
            ev_lost_bb: tally.ev_loss,
            last: self.last_grades()?,
        })
    }

    pub(crate) fn dashboard_in(&mut self, tz: &TimeZone) -> Result<Dashboard> {
        let now = self.now();
        let today = local_date(now, tz);
        let first = plus_days(today, 1 - CHART_DAYS as i64);

        // Decisions per day.
        let mut tallies = [Tally::default(); CHART_DAYS];
        let mut days: Vec<DayGrades> = (0..CHART_DAYS)
            .map(|i| DayGrades {
                day: Day::of(plus_days(first, i as i64)),
                counts: [0; 5],
            })
            .collect();
        for d in self.store.decision_stats_since(day_start(first, tz))? {
            let Some(g) = grade_index(&d.grade) else {
                continue;
            };
            let date = local_date(d.created_at, tz);
            let i = (date - first).get_days();
            if (0..CHART_DAYS as i32).contains(&i) {
                days[i as usize].counts[g] += 1;
                tallies[i as usize].add(g, d.ev_loss);
            }
        }
        let t = &tallies[CHART_DAYS - 1];
        let today_numbers = Today {
            day: Day::of(today),
            decisions: t.n,
            accuracy: t.accuracy(),
            ev_lost_bb: t.ev_loss,
            last: self.last_grades()?,
        };

        Ok(Dashboard {
            today: today_numbers,
            reviews: self.review_forecast(now, today, tz)?,
            accuracy: trend(&tallies, Tally::accuracy),
            ev_loss: trend(&tallies, Tally::ev_per_decision),
            calc: self.calc_pace()?,
            days,
            leaks: self.leaks_top(DASHBOARD_LEAKS)?,
            deviation: self.deviation()?,
        })
    }

    fn review_forecast(&mut self, now: Millis, today: Date, tz: &TimeZone) -> Result<Reviews> {
        let due_now = self.due_count()?;
        let kinds = crate::review_session::PRESENTABLE;
        // The items the due count leaves out never come due either.
        let hold = self.postflop_hold();
        let mut forecast = [0u32; FORECAST_DAYS];
        let mut days = [Day::of(today); FORECAST_DAYS];
        let mut from = now;
        for (i, slot) in forecast.iter_mut().enumerate() {
            let end = day_start(plus_days(today, i as i64 + 1), tz);
            *slot = self
                .store
                .count_due_between(from, end, kinds, hold.as_ref())?;
            days[i] = Day::of(plus_days(today, i as i64));
            from = end;
        }
        forecast[0] += due_now;
        Ok(Reviews {
            due_now,
            minutes: (due_now * SECS_PER_REVIEW).div_ceil(60),
            tomorrow: forecast[1],
            week: forecast.iter().sum(),
            forecast,
            days,
        })
    }

    fn calc_pace(&self) -> Result<CalcPace> {
        // Newest first; skipped attempts have no answer time worth counting.
        let mut answered: Vec<_> = self
            .store
            .recent_calc_attempts(CALC_WINDOW)?
            .into_iter()
            .filter(|a| a.answer.is_some())
            .collect();
        answered.reverse();
        let n = answered.len();
        if n == 0 {
            return Ok(CalcPace {
                attempts: 0,
                mean_secs: None,
                accuracy: None,
                target_secs: None,
                series: Vec::new(),
            });
        }
        let secs = |ms: i64| ms as f64 / 1000.0;
        let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
        let times: Vec<f64> = answered.iter().map(|a| secs(a.duration_ms)).collect();
        let targets: Vec<f64> = answered
            .iter()
            .filter_map(|a| ExerciseKind::ALL.into_iter().find(|k| k.id() == a.exercise))
            .map(|k| self.calc.get(k).target_secs)
            .collect();
        let points = n.min(CALC_POINTS);
        let series = (0..points)
            .map(|i| mean(&times[i * n / points..(i + 1) * n / points]))
            .collect();
        Ok(CalcPace {
            attempts: n as u32,
            mean_secs: Some(mean(&times)),
            accuracy: Some(answered.iter().filter(|a| a.correct).count() as f64 / n as f64),
            target_secs: (!targets.is_empty()).then(|| mean(&targets)),
            series,
        })
    }

    /// Where the latest preflop decisions leave the reference.
    pub fn deviation(&self) -> Result<Deviation> {
        const KINDS: usize = PlayKind::ALL.len();
        #[derive(Clone, Copy, Default)]
        struct Acc {
            n: u32,
            hero: [f32; KINDS],
            reference: [f32; KINDS],
        }
        let mut acc = vec![Acc::default(); HandClass::COUNT];
        let mut decisions = 0;
        for d in self.store.recent_preflop_decisions(DEVIATION_WINDOW)? {
            let Ok(hand) = d.hero_combo.parse::<Hand>() else {
                continue;
            };
            let node = match NodeType::parse(&d.node_type) {
                Some(NodeType::Preflop(k)) => Some(k),
                _ => None,
            };
            let actions: Vec<&str> = d.ev_per_action["actions"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let freqs: Vec<f32> = d.frequencies["hand"]
                .as_array()
                .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
                .unwrap_or_default();
            // A row whose payload does not line up says nothing reliable.
            if actions.is_empty()
                || actions.len() != freqs.len()
                || !actions.contains(&d.chosen_action.as_str())
            {
                continue;
            }
            let a = &mut acc[HandClass::of(hand).index()];
            a.n += 1;
            a.hero[PlayKind::of(&d.chosen_action, node).index()] += 1.0;
            for (code, f) in actions.iter().zip(&freqs) {
                a.reference[PlayKind::of(code, node).index()] += f;
            }
            decisions += 1;
        }

        let mut cells = vec![DeviationCell::default(); HandClass::COUNT];
        let mut top = Vec::new();
        for (i, a) in acc.iter().enumerate() {
            if a.n == 0 {
                continue;
            }
            let n = a.n as f32;
            let diff: Vec<f32> = (0..KINDS)
                .map(|k| (a.hero[k] - a.reference[k]) / n)
                .collect();
            let raw = diff.iter().map(|d| d.abs()).sum::<f32>() / 2.0;
            let distance = raw * n / (n + 2.0);
            let level = LEVEL_BOUNDS.iter().filter(|b| distance >= **b).count() as u8;
            cells[i] = DeviationCell {
                decisions: a.n,
                distance,
                level,
            };
            if level > 0 {
                // The differences sum to zero, so one play done too much and
                // one done too little often tie: name what hero did, which
                // reads more directly ("folds too much").
                let most = |sign: f32| {
                    (0..KINDS)
                        .max_by(|&x, &y| (sign * diff[x]).total_cmp(&(sign * diff[y])))
                        .expect("kinds exist")
                };
                let (over, under) = (most(1.0), most(-1.0));
                let k = if diff[over] + TIE >= -diff[under] {
                    over
                } else {
                    under
                };
                top.push(TopDeviation {
                    class: HandClass::from_index(i).expect("< 169"),
                    play: PlayKind::ALL[k],
                    too_much: diff[k] > 0.0,
                    decisions: a.n,
                    distance,
                });
            }
        }
        top.sort_by(|a, b| {
            b.distance
                .total_cmp(&a.distance)
                .then(b.decisions.cmp(&a.decisions))
                .then(a.class.index().cmp(&b.class.index()))
        });
        top.truncate(TOP_DEVIATIONS);
        Ok(Deviation {
            decisions,
            cells,
            top,
        })
    }

    /// Lesson progress and the next lesson, with the catalog grouped as
    /// `view` asks.
    pub fn lessons_summary(
        &self,
        view: crate::lessons::ViewBy,
        locale: Locale,
    ) -> Result<LessonsSummary> {
        let tree = self.journey(locale)?;
        let lessons = || tree.iter().flat_map(|t| &t.lessons);
        let next = tree.iter().find_map(|t| {
            let i = t
                .lessons
                .iter()
                .position(|l| l.state == LessonState::Available)?;
            let l = &t.lessons[i];
            Some(NextLesson {
                id: l.id.clone(),
                title: l.title.clone(),
                level: t.id.clone(),
                number: i + 1,
                level_len: t.lessons.len(),
                minutes: l.minutes,
            })
        });
        Ok(LessonsSummary {
            done: lessons().filter(|l| l.state == LessonState::Done).count(),
            total: lessons().count(),
            list: match view {
                crate::lessons::ViewBy::Journey => crate::lessons::LessonList {
                    view,
                    groups: tree.clone(),
                    next: next.as_ref().map(|n| n.id.clone()),
                },
                _ => self.lesson_list(view, locale)?,
            },
            levels: tree.iter().filter(|g| !g.is_soon()).cloned().collect(),
            minutes_left: lessons()
                .filter(|l| l.state != LessonState::Done)
                .filter_map(|l| l.minutes)
                .sum(),
            quiz_mean: self.store.mean_best_quiz_score()?.map(|(mean, _)| mean),
            next,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use serde_json::json;
    use taful_store::{DecisionContext, NewCalcAttempt, NewDecision};

    use super::*;
    use crate::services::tests::app_at;

    #[test]
    fn every_play_has_a_string() {
        // The dashboard builds these ids at run time, so the desktop source
        // lint cannot see them.
        for locale in crate::i18n::Locale::ALL {
            let i18n = crate::i18n::Localizer::new(locale).unwrap();
            for k in PlayKind::ALL {
                assert!(i18n.has(&format!("cockpit-play-{}", k.id())), "{k:?}");
            }
        }
    }

    const DAY: Millis = 86_400_000;
    /// 2026-11-20 12:00 UTC, a Friday.
    const NOON: Millis = 1_795_176_000_000;

    struct D<'a> {
        grade: &'a str,
        ev_loss: f64,
        hand: &'a str,
        chosen: &'a str,
        actions: &'a [&'a str],
        freqs: &'a [f32],
        node_type: &'a str,
        street: &'a str,
    }

    impl Default for D<'_> {
        fn default() -> Self {
            D {
                grade: "best",
                ev_loss: 0.0,
                hand: "AsKs",
                chosen: "r250",
                actions: &["f", "r250"],
                freqs: &[0.0, 1.0],
                node_type: "rfi",
                street: "preflop",
            }
        }
    }

    fn decide(app: &App, d: D<'_>) {
        app.store
            .insert_decision(&NewDecision {
                hand_id: None,
                node_key: "n",
                hero_combo: d.hand,
                chosen_action: d.chosen,
                ev_per_action: json!({ "actions": d.actions, "ev": vec![0.0; d.actions.len()] }),
                frequencies: json!({ "hand": d.freqs, "range": d.freqs }),
                pot: 1.5,
                ev_loss: d.ev_loss,
                grade: d.grade,
                thresholds_version: 1,
                context: Some(DecisionContext {
                    template_key: "t",
                    spot: "preflop",
                    street: d.street,
                    node_type: d.node_type,
                    texture: None,
                    step: None,
                    opponent: "reference",
                }),
            })
            .unwrap();
    }

    #[test]
    fn noon_is_the_friday_the_tests_assume() {
        let d = local_date(NOON, &TimeZone::UTC);
        assert_eq!((d.year(), d.month(), d.day()), (2026, 11, 20));
        assert_eq!(Day::of(d).weekday, 4);
    }

    #[test]
    fn days_trends_and_today() {
        let (mut app, clock) = app_at(NOON - 20 * DAY);
        // 20 days ago: outside the chart.
        decide(&app, D::default());
        // 10 days ago (previous span): 1 of 2 accurate, 0.6 bb lost.
        clock.store(NOON - 10 * DAY, Ordering::SeqCst);
        decide(&app, D::default());
        decide(
            &app,
            D {
                grade: "mistake",
                ev_loss: 0.6,
                ..D::default()
            },
        );
        // Yesterday: 2 accurate.
        clock.store(NOON - DAY, Ordering::SeqCst);
        decide(&app, D::default());
        decide(
            &app,
            D {
                grade: "correct",
                ev_loss: 0.02,
                ..D::default()
            },
        );
        // Today: best, inaccuracy, blunder.
        clock.store(NOON, Ordering::SeqCst);
        decide(&app, D::default());
        decide(
            &app,
            D {
                grade: "inaccuracy",
                ev_loss: 0.1,
                ..D::default()
            },
        );
        // A moment later, so it is the newest whatever the row ids are.
        clock.store(NOON + 1, Ordering::SeqCst);
        decide(
            &app,
            D {
                grade: "blunder",
                ev_loss: 1.5,
                ..D::default()
            },
        );

        let d = app.dashboard_in(&TimeZone::UTC).unwrap();
        assert_eq!(d.days.len(), CHART_DAYS);
        assert_eq!(d.days[CHART_DAYS - 1].day.day, 20);
        assert_eq!(d.days[CHART_DAYS - 1].counts, [1, 0, 1, 0, 1]);
        assert_eq!(d.days[CHART_DAYS - 2].counts, [1, 1, 0, 0, 0]);
        assert_eq!(d.days[CHART_DAYS - 11].counts, [1, 0, 0, 1, 0]);
        assert_eq!(d.days.iter().map(DayGrades::total).sum::<u32>(), 7);
        assert_eq!(d.days[0].day.day, 7);

        assert_eq!(d.today.decisions, 3);
        assert!((d.today.accuracy.unwrap() - 1.0 / 3.0).abs() < 1e-9);
        assert!((d.today.ev_lost_bb - 1.6).abs() < 1e-9);
        // Oldest first, the newest decision last.
        assert_eq!(d.today.last.len(), 8);
        assert_eq!(d.today.last.last(), Some(&Grade::Blunder));
        assert_eq!(app.today_numbers_in(&TimeZone::UTC).unwrap(), d.today);

        // Last 7 days: 3 of 5 accurate; the 7 before: 1 of 2.
        assert!((d.accuracy.value.unwrap() - 0.6).abs() < 1e-9);
        assert!((d.accuracy.delta.unwrap() - 0.1).abs() < 1e-9);
        assert!((d.ev_loss.value.unwrap() - 1.62 / 5.0).abs() < 1e-9);
        assert!((d.ev_loss.delta.unwrap() - (1.62 / 5.0 - 0.3)).abs() < 1e-9);
        assert_eq!(d.accuracy.series.len(), CHART_DAYS);
        assert_eq!(d.accuracy.series[CHART_DAYS - 2], Some(1.0));
        assert_eq!(d.accuracy.series[CHART_DAYS - 3], None);
    }

    #[test]
    fn a_day_follows_the_time_zone() {
        // 01:00 UTC on the 20th is still the 19th in São Paulo (UTC-3).
        let (mut app, clock) = app_at(NOON - 11 * 3_600_000);
        decide(&app, D::default());
        clock.store(NOON, Ordering::SeqCst);
        let tz = TimeZone::get("America/Sao_Paulo").unwrap();
        let d = app.dashboard_in(&tz).unwrap();
        assert_eq!(d.days[CHART_DAYS - 1].total(), 0);
        assert_eq!(d.days[CHART_DAYS - 2].total(), 1);
        let utc = app.dashboard_in(&TimeZone::UTC).unwrap();
        assert_eq!(utc.days[CHART_DAYS - 1].total(), 1);
    }

    #[test]
    fn empty_database() {
        let (mut app, _) = app_at(NOON);
        let d = app.dashboard_in(&TimeZone::UTC).unwrap();
        assert_eq!(d.today.decisions, 0);
        assert_eq!(d.today.accuracy, None);
        assert!(d.today.last.is_empty());
        assert_eq!(d.accuracy.value, None);
        assert_eq!(d.accuracy.delta, None);
        assert_eq!(d.calc.mean_secs, None);
        assert_eq!(d.reviews.due_now, 0);
        assert_eq!(d.reviews.week, 0);
        assert!(d.leaks.is_empty());
        assert_eq!(d.deviation.decisions, 0);
        assert!(d.deviation.top.is_empty());
        assert_eq!(d.deviation.cells.len(), HandClass::COUNT);
    }

    #[test]
    fn review_forecast_by_day() {
        let (mut app, _) = app_at(NOON);
        let due = |key: &str, at: Millis| {
            app.store
                .ensure_review_item("calc_exercise", key, None, at)
                .unwrap();
        };
        due("a", NOON - DAY); // overdue
        due("b", NOON - 1); // due now
        due("c", NOON + 3_600_000); // later today
        due("d", NOON + DAY); // tomorrow
        due("e", NOON + DAY + 1); // tomorrow
        due("f", NOON + 6 * DAY); // last forecast day
        due("g", NOON + 9 * DAY); // beyond
        let r = app.dashboard_in(&TimeZone::UTC).unwrap().reviews;
        assert_eq!(r.due_now, 2);
        assert_eq!(r.minutes, 1);
        assert_eq!(r.forecast, [3, 2, 0, 0, 0, 0, 1]);
        assert_eq!(r.tomorrow, 2);
        assert_eq!(r.week, 6);
        assert_eq!(r.days[0].weekday, 4);
        assert_eq!(r.days[1].weekday, 5);
        assert_eq!(r.days[6].day, 26);
    }

    #[test]
    fn calc_pace_over_answered_attempts() {
        let (mut app, clock) = app_at(NOON);
        let attempt = |app: &App, ms: i64, correct: bool, answer: Option<f64>| {
            app.store
                .insert_calc_attempt(&NewCalcAttempt {
                    exercise: "pot_odds",
                    band: "b",
                    params: json!({}),
                    answer,
                    correct_value: 25.0,
                    correct,
                    duration_ms: ms,
                    rating: "good",
                })
                .unwrap();
        };
        for (i, (ms, ok)) in [(4_000, true), (8_000, false), (6_000, true)]
            .into_iter()
            .enumerate()
        {
            clock.store(NOON + i as i64, Ordering::SeqCst);
            attempt(&app, ms, ok, Some(25.0));
        }
        // A skipped attempt is left out.
        clock.store(NOON + 10, Ordering::SeqCst);
        attempt(&app, 60_000, false, None);
        let c = app.dashboard_in(&TimeZone::UTC).unwrap().calc;
        assert_eq!(c.attempts, 3);
        assert!((c.mean_secs.unwrap() - 6.0).abs() < 1e-9);
        assert!((c.accuracy.unwrap() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(c.target_secs, Some(app.calc.pot_odds.target_secs));
        assert_eq!(c.series, [4.0, 8.0, 6.0]);
    }

    #[test]
    fn deviation_finds_overfolds() {
        let (app, _) = app_at(NOON);
        // K9o: the reference opens always; hero folds 4 of 4.
        for _ in 0..4 {
            decide(
                &app,
                D {
                    hand: "Ks9d",
                    chosen: "f",
                    ..D::default()
                },
            );
        }
        // A5s facing an open: the reference 3-bets 60%; hero always calls.
        for _ in 0..5 {
            decide(
                &app,
                D {
                    hand: "As5s",
                    chosen: "c",
                    actions: &["f", "c", "r900"],
                    freqs: &[0.0, 0.4, 0.6],
                    node_type: "vs-open",
                    ..D::default()
                },
            );
        }
        // AKs: played like the reference.
        for _ in 0..6 {
            decide(&app, D::default());
        }
        // A flop decision and a malformed row are ignored.
        decide(
            &app,
            D {
                street: "flop",
                node_type: "vs-bet",
                ..D::default()
            },
        );
        decide(
            &app,
            D {
                chosen: "r999",
                ..D::default()
            },
        );

        let dev = app.deviation().unwrap();
        assert_eq!(dev.decisions, 15);
        let cell = |h: &str| dev.cells[HandClass::of(h.parse().unwrap()).index()];
        assert_eq!(cell("AsKs").level, 0);
        assert_eq!(cell("AsKs").decisions, 6);
        // 1.0 raw, shrunk by 4 / 6.
        assert!((cell("Ks9d").distance - 4.0 / 6.0).abs() < 1e-6);
        assert_eq!(cell("Ks9d").level, 4);
        // 0.6 raw, shrunk by 5 / 7.
        assert!((cell("As5s").distance - 0.6 * 5.0 / 7.0).abs() < 1e-6);
        assert_eq!(cell("As5s").level, 3);

        assert_eq!(dev.top.len(), 2);
        assert_eq!(dev.top[0].class.to_string(), "K9o");
        // Hero folds too much, which is also "opens too little": the tie
        // goes to what hero did.
        assert_eq!(
            (dev.top[0].play, dev.top[0].too_much),
            (PlayKind::Fold, true)
        );
        assert_eq!(dev.top[1].class.to_string(), "A5s");
        assert_eq!(
            (dev.top[1].play, dev.top[1].too_much),
            (PlayKind::Call, true)
        );
    }

    #[test]
    fn one_mixed_decision_is_not_a_deviation() {
        let (app, _) = app_at(NOON);
        decide(
            &app,
            D {
                hand: "Qs9s",
                chosen: "f",
                freqs: &[0.8, 0.2],
                ..D::default()
            },
        );
        let dev = app.deviation().unwrap();
        assert_eq!(dev.cells.iter().map(|c| c.level).max(), Some(0));
        assert!(dev.top.is_empty());
    }

    #[test]
    fn lessons_summary_reads_the_tree() {
        let (app, _) = app_at(NOON);
        let s = app
            .lessons_summary(crate::lessons::ViewBy::Journey, Locale::PtBr)
            .unwrap();
        assert!(s.total > 0);
        assert_eq!(s.done, 0);
        assert!(s.list.groups.iter().any(|g| !g.is_soon()));
        assert!(s.minutes_left > 0);
        assert_eq!(s.quiz_mean, None);
        let next = s.next.unwrap();
        assert_eq!(next.number, 1);
        assert!(next.level_len >= 1);
        assert!(!next.title.is_empty());
    }
}

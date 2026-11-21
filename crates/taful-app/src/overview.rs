//! What a home screen with more than the basics shows (the Salão visual
//! style, D19): the next lesson with its place in its level, the calc
//! trainer's pace and the week's table decisions by grade.

use taful_core::grading::Grade;
use taful_store::{LessonState, Millis};

use crate::i18n::Locale;
use crate::{App, Home, Result};

/// The next lesson and where it sits in its level.
#[derive(Debug, Clone, PartialEq)]
pub struct NextLesson {
    pub id: String,
    pub title: String,
    /// The level's id (title: Fluent `level-<id>`).
    pub level: String,
    /// Its place in the level, from 1.
    pub number: usize,
    pub minutes: Option<u32>,
    /// The state of every lesson of the level, in order.
    pub level_states: Vec<LessonState>,
}

/// The home screen's numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    pub home: Home,
    pub next: Option<NextLesson>,
    /// Mean answer time over the latest calc answers, in milliseconds.
    pub calc_mean_ms: Option<f64>,
    /// Table decisions since Monday (local time), per grade in
    /// [`Grade::ALL`] order.
    pub week: [u32; 5],
}

impl Overview {
    /// Decisions this week.
    pub fn week_total(&self) -> u32 {
        self.week.iter().sum()
    }
}

/// The start of the local week (Monday 00:00) that `now` is in.
fn week_start(now: Millis) -> Millis {
    let fallback = now - 7 * 24 * 3600 * 1000;
    let Ok(ts) = jiff::Timestamp::from_millisecond(now) else {
        return fallback;
    };
    let z = ts.to_zoned(jiff::tz::TimeZone::system());
    let back = i64::from(z.weekday().to_monday_zero_offset());
    z.start_of_day()
        .and_then(|d| d.checked_sub(jiff::Span::new().days(back)))
        .map_or(fallback, |d| d.timestamp().as_millisecond())
}

/// A local date's parts: weekday (1 = Monday … 7 = Sunday), day of the month
/// and month (1–12).
pub fn date_parts(ms: Millis) -> Option<(u8, u8, u8)> {
    let z = jiff::Timestamp::from_millisecond(ms)
        .ok()?
        .to_zoned(jiff::tz::TimeZone::system());
    Some((
        z.weekday().to_monday_one_offset() as u8,
        z.day() as u8,
        z.month() as u8,
    ))
}

impl App {
    pub fn overview(&self, locale: Locale) -> Result<Overview> {
        let tree = self.journey(locale)?;
        let recent = self
            .store
            .recent_calc_attempts(crate::services::HOME_CALC_WINDOW)?;
        let home = self.home_from(&tree, &recent)?;
        let next = tree.next_at().map(|(g, at)| {
            let t = &tree.groups[g];
            let l = &t.lessons[at];
            NextLesson {
                id: l.id.clone(),
                title: l.title.clone(),
                level: t.id.clone(),
                number: at + 1,
                minutes: l.minutes,
                level_states: t.lessons.iter().map(|l| l.state).collect(),
            }
        });
        let calc_mean_ms = (!recent.is_empty()).then(|| {
            recent.iter().map(|a| a.duration_ms as f64).sum::<f64>() / recent.len() as f64
        });
        let mut week = [0u32; 5];
        for (grade, n) in self
            .store
            .grade_counts_since(week_start(self.store.now()))?
        {
            if let Some(i) = Grade::ALL.iter().position(|g| g.id() == grade) {
                week[i] += n;
            }
        }
        Ok(Overview {
            home,
            next,
            calc_mean_ms,
            week,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tests::app_at;

    #[test]
    fn overview_of_a_fresh_app() {
        // 2026-11-20 12:00 UTC, a Friday.
        let (app, _) = app_at(1_795_348_800_000);
        let o = app.overview(Locale::PtBr).unwrap();
        assert_eq!(o.week_total(), 0);
        assert_eq!(o.calc_mean_ms, None);
        let next = o.next.expect("a fresh app has a first lesson");
        assert_eq!(next.number, 1);
        assert_eq!(
            Some(&next.id),
            o.home.next_lesson.as_ref().map(|(id, _)| id)
        );
        assert_eq!(next.level, "primeiros-passos");
        // Prova (the default) opens the whole first level.
        assert!(
            next.level_states
                .iter()
                .all(|s| *s == LessonState::Available)
        );
    }

    #[test]
    fn weeks_start_on_monday() {
        let now = 1_795_348_800_000;
        let start = week_start(now);
        assert!(start <= now && now - start < 7 * 24 * 3600 * 1000);
        let (weekday, _, _) = date_parts(start).unwrap();
        assert_eq!(weekday, 1);
        // The same week has the same start.
        assert_eq!(week_start(start + 1), start);
        let (_, day, month) = date_parts(now).unwrap();
        assert!((1..=31).contains(&day) && (1..=12).contains(&month));
    }
}

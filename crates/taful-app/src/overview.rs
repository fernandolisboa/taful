//! What a home screen with more than the basics shows (the Salão visual
//! style, D19): the next step, the calc trainer's pace and the week's table
//! decisions by grade.

use taful_core::grading::Grade;
use taful_store::Millis;

use crate::i18n::Locale;
use crate::{App, Home, Result};

/// The home screen's numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// The basics, with the next step (D27) for the hero.
    pub home: Home,
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
        // A fresh app starts with the first lesson of its level, all of
        // which Prova (the default) opens.
        let crate::next_step::NextStep::Lesson(step) = &o.home.step else {
            panic!("{:?}", o.home.step);
        };
        assert_eq!(step.number, 1);
        assert!(!step.started);
        assert_eq!(
            Some(&step.id),
            o.home.next_lesson.as_ref().map(|(id, _)| id)
        );
        assert!(
            step.level_states
                .iter()
                .all(|s| *s == taful_store::LessonState::Available)
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

//! The Home's one next step (D27): what to do now, so a beginner never has
//! to guess between lessons, reviews and the table. In this order:
//!
//! 1. reviews that are due;
//! 2. the lesson in progress (a quiz tried and not passed, or a guided
//!    hand still to play), else the next lesson in the Jornada as the
//!    unlock mode opens it;
//! 3. practice at the table on what the last lesson done was about.
//!
//! Every style draws the step in its own hero; nothing here is stored.

use taful_store::{LessonState, Millis};

use crate::lessons::LessonList;
use crate::preflop::SpotKind;
use crate::today::REVIEW_SECS;
use crate::{App, Result};

/// What the table is opened on for practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Practice {
    /// Full hands against the bots.
    Hand,
    /// Postflop spots.
    Postflop,
    /// Preflop spots of one kind.
    Spot(SpotKind),
}

impl Practice {
    /// The id the catalog's `practice` field uses (`taful_content::PRACTICES`)
    /// and the Fluent key suffix (`next-step-practice-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Practice::Hand => "hand",
            Practice::Postflop => "postflop",
            Practice::Spot(kind) => kind.id(),
        }
    }

    pub fn from_id(id: &str) -> Option<Practice> {
        match id {
            "hand" => Some(Practice::Hand),
            "postflop" => Some(Practice::Postflop),
            _ => SpotKind::ALL
                .into_iter()
                .find(|k| k.id() == id)
                .map(Practice::Spot),
        }
    }

    /// What a lesson of `theme` is practiced with when its catalog entry
    /// names nothing.
    pub fn for_theme(theme: &str) -> Practice {
        match theme {
            "preflop" => Practice::Spot(SpotKind::Rfi),
            "postflop" | "math" => Practice::Postflop,
            _ => Practice::Hand,
        }
    }
}

/// The lesson a step points at.
#[derive(Debug, Clone, PartialEq)]
pub struct StepLesson {
    pub id: String,
    pub title: String,
    /// Its level's id (title: Fluent `level-<id>`).
    pub level: String,
    /// Its place in the level, from 1.
    pub number: usize,
    pub minutes: Option<u32>,
    /// Every lesson of the level, in order.
    pub level_states: Vec<LessonState>,
    /// The user already began it: a quiz tried, or its guided hand played.
    pub started: bool,
}

impl StepLesson {
    pub fn level_len(&self) -> usize {
        self.level_states.len()
    }
}

/// The one thing the Home suggests doing now.
#[derive(Debug, Clone, PartialEq)]
pub enum NextStep {
    /// Reviews are due.
    Review {
        due: u32,
        /// About how long they take, at least one minute.
        minutes: u32,
        /// The lesson that comes after them, for a screen that still shows
        /// one beside the reviews.
        lesson: Option<StepLesson>,
    },
    /// Read (or finish) a lesson.
    Lesson(StepLesson),
    /// No lesson to take: practice at the table.
    Table {
        practice: Practice,
        /// The title of the last lesson done, which the practice follows.
        after: Option<String>,
        /// In Prova, the level whose (optional) test opens the next one.
        test: Option<String>,
    },
}

impl App {
    /// The Home's next step (D27).
    pub fn next_step(&self, locale: crate::i18n::Locale) -> Result<NextStep> {
        self.next_step_from(&self.journey(locale)?)
    }

    /// [`App::next_step`] from the Jornada already read.
    pub(crate) fn next_step_from(&self, journey: &LessonList) -> Result<NextStep> {
        let progress = self.store.lesson_progress()?;
        // When each lesson last moved, by state, at any version.
        let last = |id: &str, state: LessonState| -> Option<Millis> {
            progress
                .iter()
                .filter(|p| p.lesson_id == id && p.state == state)
                .map(|p| p.updated_at)
                .max()
        };
        let lesson = self.lesson_step(journey, &progress)?;
        // Only what the review screen can show: postflop items wait while
        // no postflop library is installed.
        let due = self.due_count_loaded()?;
        if due > 0 {
            return Ok(NextStep::Review {
                due,
                minutes: (due * REVIEW_SECS).div_ceil(60).max(1),
                lesson,
            });
        }
        if let Some(l) = lesson {
            return Ok(NextStep::Lesson(l));
        }

        // The last lesson done, and what its catalog entry practices.
        let catalog = &self.library.catalog;
        let done = catalog
            .lessons
            .iter()
            .filter_map(|e| Some((last(&e.id, LessonState::Done)?, e)))
            .max_by_key(|(when, _)| *when)
            .map(|(_, e)| e);
        let practice = done.map_or(Practice::Hand, |e| {
            e.practice
                .as_deref()
                .and_then(Practice::from_id)
                .unwrap_or_else(|| Practice::for_theme(&e.theme))
        });
        let after = done.and_then(|e| {
            journey
                .lessons()
                .find(|c| c.id == e.id)
                .map(|c| c.title.clone())
        });
        Ok(NextStep::Table {
            practice,
            after,
            test: journey.next_test.clone(),
        })
    }

    /// The lesson in progress, else the Jornada's next one.
    fn lesson_step(
        &self,
        journey: &LessonList,
        progress: &[taful_store::LessonProgress],
    ) -> Result<Option<StepLesson>> {
        let guided = self.store.guided_done()?;
        // A lesson begun and still open: a quiz tried (its row stays
        // `available` with a score; reading it alone stores no score), or
        // a guided hand played before the quiz. The latest first; a guided
        // hand alone has no time and comes last.
        let begun = |id: &str| -> Option<Millis> {
            progress
                .iter()
                .filter(|p| p.lesson_id == id)
                .filter(|p| p.state == LessonState::Available && p.best_score.is_some())
                .map(|p| p.updated_at)
                .max()
                .or_else(|| {
                    self.library
                        .guided_for(id)
                        .filter(|g| guided.contains(&g.id))
                        .map(|_| Millis::MIN)
                })
        };
        let at = |g: usize, l: usize| {
            let group = &journey.groups[g];
            let lesson = &group.lessons[l];
            StepLesson {
                id: lesson.id.clone(),
                title: lesson.title.clone(),
                level: group.id.clone(),
                number: l + 1,
                minutes: lesson.minutes,
                level_states: group.lessons.iter().map(|c| c.state).collect(),
                started: false,
            }
        };
        let in_progress = journey
            .groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| {
                group
                    .lessons
                    .iter()
                    .enumerate()
                    .map(move |(l, c)| (g, l, c))
            })
            .filter(|(.., c)| c.state == LessonState::Available)
            .filter_map(|(g, l, c)| Some((begun(&c.id)?, g, l)))
            // The first of equals: Jornada order breaks a tie.
            .min_by_key(|(when, g, l)| (std::cmp::Reverse(*when), *g, *l));
        Ok(match in_progress {
            Some((_, g, l)) => Some(StepLesson {
                started: true,
                ..at(g, l)
            }),
            None => journey.next_at().map(|(g, l)| at(g, l)),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::i18n::Locale;
    use crate::lessons::UnlockMode;
    use crate::model::Answer;
    use crate::services::tests::app_at;
    use taful_core::calc::ExerciseKind;

    const START: Millis = 1_800_000_000_000;

    fn lesson(step: NextStep) -> StepLesson {
        match step {
            NextStep::Lesson(l) => l,
            other => panic!("expected a lesson, got {other:?}"),
        }
    }

    /// The lessons of the first level that has any, in order.
    fn first_level(app: &App) -> (String, Vec<String>) {
        let j = app.journey(Locale::PtBr).unwrap();
        let g = j.groups.iter().find(|g| !g.is_soon()).unwrap();
        (
            g.id.clone(),
            g.lessons.iter().map(|l| l.id.clone()).collect(),
        )
    }

    fn version(app: &App, id: &str) -> u32 {
        app.library
            .lesson(id, taful_content::SOURCE_LOCALE)
            .unwrap()
            .meta
            .version
    }

    fn mark_done(app: &App, clock: &std::sync::atomic::AtomicI64, id: &str) {
        clock.fetch_add(1_000, Ordering::SeqCst);
        app.store
            .record_lesson_progress(id, version(app, id), LessonState::Done, Some(1.0))
            .unwrap();
    }

    /// The quiz's answers, right or all wrong.
    fn answers(app: &App, id: &str, right: bool) -> Vec<Answer> {
        app.quiz(id, Locale::PtBr)
            .unwrap()
            .questions
            .iter()
            .map(|q| match (q.answer_index, &q.numeric, right) {
                (Some(i), _, true) => Answer::Choice(i),
                (Some(i), _, false) => Answer::Choice((i + 1) % q.choices.len()),
                (_, Some((v, _)), true) => Answer::Numeric(*v),
                (_, Some((v, _)), false) => Answer::Numeric(v + 1_000.),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn a_fresh_profile_starts_with_the_first_lesson() {
        let (app, _) = app_at(START);
        let step = lesson(app.next_step(Locale::PtBr).unwrap());
        let j = app.journey(Locale::PtBr).unwrap();
        assert_eq!(Some(step.id.as_str()), j.next.as_deref());
        assert_eq!(step.number, 1);
        assert!(!step.started);
        assert!(step.level_len() > 1);
        assert_eq!(step.level, first_level(&app).0);
    }

    #[test]
    fn due_reviews_come_first() {
        let (mut app, clock) = app_at(START);
        // A wrong calc answer queues its skill for review.
        let round = app.calc_start(Some(ExerciseKind::Outs));
        app.calc_submit(&round, Some(round.exercise.answer + 50.))
            .unwrap();
        // Not due yet: the lesson still leads.
        let due = app.today(Locale::PtBr).unwrap().next_due.unwrap();
        let first = lesson(app.next_step(Locale::PtBr).unwrap());
        clock.store(due, Ordering::SeqCst);
        match app.next_step(Locale::PtBr).unwrap() {
            NextStep::Review {
                due: 1,
                minutes: 1,
                lesson: Some(after),
            } => assert_eq!(after.id, first.id, "the lesson waits behind them"),
            other => panic!("expected the reviews, got {other:?}"),
        }
    }

    #[test]
    fn a_lesson_begun_comes_before_the_next_one() {
        let (mut app, clock) = app_at(START);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let (_, ids) = first_level(&app);
        let third = &ids[2];
        // Reading a lesson does not begin it.
        app.open_lesson(&ids[4], Locale::PtBr).unwrap();
        assert_eq!(lesson(app.next_step(Locale::PtBr).unwrap()).id, ids[0]);
        // A failed quiz leaves the lesson begun and open.
        let wrong = answers(&app, third, false);
        clock.fetch_add(1_000, Ordering::SeqCst);
        let res = app.submit_quiz(third, &wrong, 1_000).unwrap();
        assert!(!res.passed);
        let step = lesson(app.next_step(Locale::PtBr).unwrap());
        assert_eq!((&step.id, step.number, step.started), (third, 3, true));
        // The latest one begun wins.
        let second = &ids[1];
        clock.fetch_add(1_000, Ordering::SeqCst);
        let wrong2 = answers(&app, second, false);
        app.submit_quiz(second, &wrong2, 1_000).unwrap();
        assert_eq!(&lesson(app.next_step(Locale::PtBr).unwrap()).id, second);
        // Done, it gives way to the other one.
        mark_done(&app, &clock, second);
        assert_eq!(&lesson(app.next_step(Locale::PtBr).unwrap()).id, third);
    }

    #[test]
    fn a_guided_hand_begins_its_lesson_either_way_round() {
        let (mut app, clock) = app_at(START);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let with_hand: Vec<String> = first_level(&app)
            .1
            .into_iter()
            .filter(|id| app.library.guided_for(id).is_some())
            .collect();
        assert!(with_hand.len() >= 3, "{with_hand:?}");
        // The hand played first: begun, though no quiz was tried.
        let hand = app.library.guided_for(&with_hand[2]).unwrap().id.clone();
        app.finish_guided(&hand).unwrap();
        let step = lesson(app.next_step(Locale::PtBr).unwrap());
        assert_eq!((&step.id, step.started), (&with_hand[2], true));
        // The quiz passed first, the hand still to play: begun, and newer.
        clock.fetch_add(1_000, Ordering::SeqCst);
        let right = answers(&app, &with_hand[1], true);
        let res = app.submit_quiz(&with_hand[1], &right, 1_000).unwrap();
        assert!(res.passed && res.guided_pending.is_some());
        let step = lesson(app.next_step(Locale::PtBr).unwrap());
        assert_eq!((&step.id, step.started), (&with_hand[1], true));
    }

    #[test]
    fn a_lesson_begun_and_now_locked_gives_way() {
        let (mut app, clock) = app_at(START);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let (_, ids) = first_level(&app);
        clock.fetch_add(1_000, Ordering::SeqCst);
        let wrong = answers(&app, &ids[3], false);
        app.submit_quiz(&ids[3], &wrong, 1_000).unwrap();
        assert_eq!(lesson(app.next_step(Locale::PtBr).unwrap()).id, ids[3]);
        // Guiado opens one lesson at a time: the fourth is locked again.
        app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
        let step = lesson(app.next_step(Locale::PtBr).unwrap());
        assert_eq!((&step.id, step.started), (&ids[0], false));
    }

    #[test]
    fn with_every_lesson_done_the_table_follows_the_last_one() {
        let (app, clock) = app_at(START);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let catalog = app.library.catalog.clone();
        let last = catalog
            .lessons
            .iter()
            .find(|e| e.practice.as_deref() == Some("vs-open"))
            .expect("a lesson practiced against an open")
            .id
            .clone();
        for e in catalog.lessons.iter().filter(|e| e.id != last) {
            mark_done(&app, &clock, &e.id);
        }
        mark_done(&app, &clock, &last);
        let title = app.open_lesson(&last, Locale::PtBr).unwrap().title;
        assert_eq!(
            app.next_step(Locale::PtBr).unwrap(),
            NextStep::Table {
                practice: Practice::Spot(SpotKind::VsOpen),
                after: Some(title),
                test: None,
            }
        );
    }

    #[test]
    fn prova_names_the_test_with_the_practice() {
        let (app, clock) = app_at(START);
        assert_eq!(app.unlock_mode(), UnlockMode::Tests);
        let (level, ids) = first_level(&app);
        // Picked on the first run: lessons done above it never move it.
        app.set_start_level(&level).unwrap();
        for id in &ids {
            mark_done(&app, &clock, id);
        }
        let j = app.journey(Locale::PtBr).unwrap();
        assert_eq!(
            (j.next.as_deref(), j.next_test.as_deref()),
            (None, Some(level.as_str()))
        );
        let entry = app.library.catalog.entry(ids.last().unwrap()).unwrap();
        let practice = entry
            .practice
            .as_deref()
            .and_then(Practice::from_id)
            .unwrap_or_else(|| Practice::for_theme(&entry.theme));
        match app.next_step(Locale::PtBr).unwrap() {
            NextStep::Table {
                practice: p,
                after,
                test,
            } => {
                assert_eq!(p, practice);
                assert!(after.is_some());
                assert_eq!(test, Some(level));
            }
            other => panic!("expected the table, got {other:?}"),
        }
    }

    #[test]
    fn lessons_done_at_the_same_time_still_give_a_step() {
        let (app, _) = app_at(START);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        // Every lesson done at once, with the same time: still a step.
        let catalog = app.library.catalog.clone();
        for e in &catalog.lessons {
            app.store
                .record_lesson_progress(&e.id, version(&app, &e.id), LessonState::Done, None)
                .unwrap();
        }
        assert!(matches!(
            app.next_step(Locale::PtBr).unwrap(),
            NextStep::Table {
                after: Some(_),
                test: None,
                ..
            }
        ));
    }

    #[test]
    fn practices_parse_and_every_lesson_has_one() {
        for id in taful_content::PRACTICES {
            let p = Practice::from_id(id).unwrap_or_else(|| panic!("`{id}` is no practice"));
            assert_eq!(p.id(), *id);
        }
        // Every spot kind can be named.
        for k in SpotKind::ALL {
            assert!(taful_content::PRACTICES.contains(&k.id()), "{}", k.id());
        }
        assert_eq!(Practice::from_id("poker"), None);
        assert_eq!(
            Practice::for_theme("preflop"),
            Practice::Spot(SpotKind::Rfi)
        );
        assert_eq!(Practice::for_theme("math"), Practice::Postflop);
        assert_eq!(Practice::for_theme("mind"), Practice::Hand);
        let (app, _) = app_at(START);
        for e in &app.library.catalog.lessons {
            if let Some(p) = &e.practice {
                assert!(Practice::from_id(p).is_some(), "{}: {p}", e.id);
            }
        }
        // Each practice says what it trains, in every locale.
        for locale in [Locale::PtBr, Locale::EnUs] {
            let i18n = crate::i18n::Localizer::new(locale).unwrap();
            for id in taful_content::PRACTICES {
                let key = format!("next-step-practice-{id}");
                assert!(i18n.has(&key), "{key} in {}", locale.tag());
            }
        }
    }
}

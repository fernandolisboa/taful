//! The lesson catalog, lessons and quizzes (spec §12, §15.1, D23).
//!
//! The lessons screen shows one catalog grouped one of several ways ("Ver
//! por", [`ViewBy`]): by level (the Jornada, from zero to pro) or by theme.
//! The grouping never changes a lesson's state, and the next lesson is
//! always the first one open in the Jornada.
//!
//! Lock state (D24, D25): a lesson is **done** once its quiz was passed at
//! any version (and, when it lists a guided hand, that hand was played to
//! the end, D26), and stays done in every mode. Otherwise the user's
//! [`UnlockMode`] decides whether it is **available** or **locked**:
//!
//! - **Prova** (default): every lesson of an unlocked level is open. The
//!   levels up to the starting level are unlocked (the one picked on the
//!   first run, D25), and passing a level's test unlocks the next one. A
//!   level with no lessons yet has nothing to test and lets the next one
//!   through.
//! - **Guiado**: one lesson at a time in Jornada order: within a level the
//!   first lesson not done is open. A level is reached once it is unlocked
//!   as above or every lesson before it is done.
//! - **Livre**: nothing is locked; the reader warns about prerequisites not
//!   done yet.
//!
//! Progress keeps the two facts apart: done lessons (`lesson_progress`) and
//! unlocked levels (the starting level and the passed level tests, both in
//! settings). Changing the mode only changes how they are read, so it never
//! loses progress. Progress rows are keyed by `(lesson id, version)`
//! (§12.4), so a content update that bumps a version shows the lesson as
//! done (from the older version) while the new version's quiz is still
//! there to take.

use std::collections::BTreeSet;

use serde_json::json;

use taful_content::lesson::Document;
use taful_content::quiz::{Answer, Quiz, RenderedQuestion};
use taful_core::srs::Rating;
use taful_store::{LessonState, NewQuizAttempt};

use crate::i18n::Locale;
use crate::review::{ItemKind, Source};
use crate::services::keys;
use crate::{App, Error, Result};

/// How lessons unlock (D24). Stored as `lessons.unlock_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnlockMode {
    /// Prova: the current level is open; a level test opens the next.
    #[default]
    Tests,
    /// Guiado: one lesson at a time.
    Stepwise,
    /// Livre: nothing locked.
    Free,
}

impl UnlockMode {
    pub const ALL: [UnlockMode; 3] = [UnlockMode::Tests, UnlockMode::Stepwise, UnlockMode::Free];

    /// The id stored in settings.
    pub fn id(self) -> &'static str {
        match self {
            UnlockMode::Tests => "prova",
            UnlockMode::Stepwise => "guiado",
            UnlockMode::Free => "livre",
        }
    }

    pub fn from_id(id: &str) -> Option<UnlockMode> {
        UnlockMode::ALL.into_iter().find(|m| m.id() == id)
    }

    /// The Fluent id of its name; `<key>-line` is its one-line label and
    /// `<key>-about` the longer explanation behind the (i).
    pub fn label_key(self) -> &'static str {
        match self {
            UnlockMode::Tests => "study-mode-prova",
            UnlockMode::Stepwise => "study-mode-guiado",
            UnlockMode::Free => "study-mode-livre",
        }
    }
}

/// Where a level test stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestState {
    /// Its level is not open yet in this mode.
    Locked,
    /// Can be taken.
    Open,
    /// Passed at least once (it can be taken again).
    Passed,
}

/// A level's test as the lessons screen shows it (D24).
#[derive(Debug, Clone, PartialEq)]
pub struct TestCard {
    pub state: TestState,
    /// Questions per attempt.
    pub questions: usize,
    /// Share of right answers needed.
    pub pass: f64,
}

/// Why a lesson is locked, for the reader's note.
#[derive(Debug, Clone, PartialEq)]
pub enum LockReason {
    /// Prova: the test of this level opens the lesson's level.
    Test { level: String },
    /// Guiado: lessons open one at a time; this one is next (if any).
    OneAtATime { next: Option<String> },
}

/// How the lessons screen groups the catalog (D23). New views may come;
/// nothing outside this type assumes the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewBy {
    /// By level, from zero to pro (the default).
    #[default]
    Journey,
    /// By theme.
    Themes,
}

impl ViewBy {
    pub const ALL: [ViewBy; 2] = [ViewBy::Journey, ViewBy::Themes];

    /// The id stored in settings.
    pub fn id(self) -> &'static str {
        match self {
            ViewBy::Journey => "journey",
            ViewBy::Themes => "themes",
        }
    }

    pub fn from_id(id: &str) -> Option<ViewBy> {
        ViewBy::ALL.into_iter().find(|v| v.id() == id)
    }

    /// The Fluent id of its name on the switch.
    pub fn label_key(self) -> &'static str {
        match self {
            ViewBy::Journey => "tree-view-journey",
            ViewBy::Themes => "tree-view-themes",
        }
    }
}

/// What a [`LessonGroup`] gathers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Level,
    Theme,
}

/// A lesson as the lessons screen shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonCard {
    pub id: String,
    pub title: String,
    pub state: LessonState,
    pub minutes: Option<u32>,
    pub best_score: Option<f64>,
}

/// A level or a theme with its lessons in Jornada order. A level with no
/// lessons yet is shown as "em breve".
#[derive(Debug, Clone, PartialEq)]
pub struct LessonGroup {
    pub kind: GroupKind,
    pub id: String,
    pub lessons: Vec<LessonCard>,
    /// A level's test, if it has one (levels only).
    pub test: Option<TestCard>,
    /// Prova: the level whose test opens this locked level (levels only).
    pub opens_with: Option<String>,
}

impl LessonGroup {
    /// The Fluent id of its title: `level-<id>` or `theme-<id>`.
    pub fn title_key(&self) -> String {
        match self.kind {
            GroupKind::Level => format!("level-{}", self.id),
            GroupKind::Theme => format!("theme-{}", self.id),
        }
    }

    /// No lessons yet ("em breve").
    pub fn is_soon(&self) -> bool {
        self.lessons.is_empty()
    }

    pub fn done(&self) -> usize {
        self.lessons
            .iter()
            .filter(|l| l.state == LessonState::Done)
            .count()
    }
}

/// The catalog grouped for the lessons screen.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonList {
    pub view: ViewBy,
    pub mode: UnlockMode,
    /// Every level or theme in display order, empty ones included.
    pub groups: Vec<LessonGroup>,
    /// The lesson to take next: the first open one in the Jornada from
    /// where the user stands (see [`App::lesson_list`]).
    pub next: Option<String>,
    /// With no lesson to take, the level whose test opens the next level.
    pub next_test: Option<String>,
}

impl LessonList {
    /// Every lesson, group by group.
    pub fn lessons(&self) -> impl Iterator<Item = &LessonCard> {
        self.groups.iter().flat_map(|g| g.lessons.iter())
    }

    pub fn done(&self) -> usize {
        self.groups.iter().map(LessonGroup::done).sum()
    }

    pub fn total(&self) -> usize {
        self.groups.iter().map(|g| g.lessons.len()).sum()
    }

    /// Whether `id` is the next lesson.
    pub fn is_next(&self, id: &str) -> bool {
        self.next.as_deref() == Some(id)
    }

    /// Where the next lesson is: `(group, lesson)` indexes.
    pub fn next_at(&self) -> Option<(usize, usize)> {
        let next = self.next.as_deref()?;
        self.groups.iter().enumerate().find_map(|(g, group)| {
            let l = group.lessons.iter().position(|l| l.id == next)?;
            Some((g, l))
        })
    }
}

/// A lesson opened for reading.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenLesson {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub state: LessonState,
    pub doc: Document,
    /// Glossary term ids; strings are Fluent `glossary-<id>` / `-def`.
    pub glossary: Vec<String>,
    /// Why it is locked, when it is.
    pub locked_by: Option<LockReason>,
    /// Titles of prerequisites not done yet, for an open lesson (Livre and
    /// Prova open lessons whose prerequisites are not done; D24).
    pub missing: Vec<String>,
    /// The guided hand played after the quiz (D26), if the lesson has one.
    pub guided: Option<GuidedPart>,
    /// The quiz was passed at least once.
    pub quiz_passed: bool,
}

/// A lesson's guided hand (D26).
#[derive(Debug, Clone, PartialEq)]
pub struct GuidedPart {
    pub id: String,
    pub title: String,
    /// Played to the end at least once.
    pub finished: bool,
}

/// A level test drawn for one attempt, rendered for one locale.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelTestView {
    pub level: String,
    pub version: u32,
    pub pass: f64,
    pub questions: Vec<RenderedQuestion>,
}

/// The result of a finished level test.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelTestResult {
    pub score: f64,
    pub passed: bool,
    pub correct: Vec<bool>,
    /// Levels this pass opened in the current mode.
    pub unlocked: Vec<String>,
}

/// Lock inputs read once: the mode, done lessons and unlocked levels.
struct Progress {
    mode: UnlockMode,
    done: BTreeSet<String>,
    passed: BTreeSet<String>,
    /// Per catalog level: unlocked by the starting level, a passed test, or
    /// an empty level before it.
    unlocked: Vec<bool>,
    /// Per catalog level: open in this mode (Guiado also reaches a level
    /// once every lesson before it is done).
    open: Vec<bool>,
    /// Lessons from this level on come first when picking the next one.
    floor: usize,
}

/// A quiz rendered for one locale.
#[derive(Debug, Clone, PartialEq)]
pub struct QuizView {
    pub id: String,
    pub version: u32,
    pub pass: f64,
    pub questions: Vec<RenderedQuestion>,
}

/// The result of a finished quiz.
#[derive(Debug, Clone, PartialEq)]
pub struct QuizResult {
    pub score: f64,
    pub passed: bool,
    pub correct: Vec<bool>,
    /// Lessons this pass made available.
    pub unlocked: Vec<String>,
    /// Passed, but the lesson's guided hand is still to play (D26): its id.
    pub guided_pending: Option<String>,
}

/// Review-item key of a quiz question.
/// Lessons locked in `before` and available in `after`.
pub(crate) fn newly_unlocked(
    before: &[(String, LessonState)],
    after: Vec<(String, LessonState)>,
) -> Vec<String> {
    after
        .into_iter()
        .filter(|(id, s)| {
            *s == LessonState::Available
                && before
                    .iter()
                    .any(|(b, bs)| b == id && *bs == LessonState::Locked)
        })
        .map(|(id, _)| id)
        .collect()
}

pub fn question_key(quiz_id: &str, question_id: &str) -> String {
    format!("{quiz_id}/{question_id}")
}

impl App {
    fn done_lessons(&self) -> Result<BTreeSet<String>> {
        Ok(self
            .store
            .lesson_progress()?
            .into_iter()
            .filter(|p| p.state == LessonState::Done)
            .map(|p| p.lesson_id)
            .collect())
    }

    // ---- mode, starting level and level tests (D24, D25) ------------------------------

    /// How lessons unlock.
    pub fn unlock_mode(&self) -> UnlockMode {
        self.string_setting(keys::LESSONS_UNLOCK_MODE)
            .and_then(|id| UnlockMode::from_id(&id))
            .unwrap_or_default()
    }

    /// Changes how lessons unlock; progress stays as it is.
    pub fn set_unlock_mode(&self, mode: UnlockMode) -> Result<()> {
        self.set_string_setting(keys::LESSONS_UNLOCK_MODE, mode.id())
    }

    /// The level the user picked to start from (D25), if any was stored.
    /// A store error is an error, so a write never replaces a choice it
    /// could not read.
    pub fn chosen_start_level(&self) -> Result<Option<String>> {
        Ok(self
            .store
            .setting(keys::LESSONS_START_LEVEL)?
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|l| self.library.catalog.levels.contains(l)))
    }

    /// Where the Jornada starts: every level up to this one is unlocked.
    /// The stored choice; else, for progress made before D24, the highest
    /// level that had a lesson open (done, or every prerequisite done) so
    /// nothing open before is locked now; else the first level.
    pub fn start_level(&self) -> Result<String> {
        if let Some(l) = self.chosen_start_level()? {
            return Ok(l);
        }
        let levels = &self.library.catalog.levels;
        let done = self.done_lessons()?;
        let rank = |id: &str| {
            let e = self.library.catalog.entry(id)?;
            levels.iter().position(|l| *l == e.level)
        };
        let derived = self
            .library
            .catalog
            .lessons
            .iter()
            .filter(|e| {
                done.contains(&e.id)
                    || (!done.is_empty()
                        && self
                            .library
                            .lesson(&e.id, taful_content::SOURCE_LOCALE)
                            .is_some_and(|l| l.meta.prerequisites.iter().all(|p| done.contains(p))))
            })
            .filter_map(|e| rank(&e.id))
            .max();
        Ok(levels[derived.unwrap_or(0).min(levels.len().saturating_sub(1))].clone())
    }

    /// Sets where the Jornada starts (D25). It only unlocks levels: nothing
    /// is marked done or passed.
    pub fn set_start_level(&self, level: &str) -> Result<()> {
        if !self.library.catalog.levels.iter().any(|l| l == level) {
            return Err(Error::NotFound(format!("level `{level}`")));
        }
        self.set_string_setting(keys::LESSONS_START_LEVEL, level)
    }

    /// Stores the starting level derived from today's progress, if none is
    /// stored, before new progress changes what it would derive.
    fn pin_start_level(&self) -> Result<()> {
        if self.chosen_start_level()?.is_none() {
            let level = self.start_level()?;
            self.set_start_level(&level)?;
        }
        Ok(())
    }

    /// Levels whose test was passed. A store error is an error, so a pass
    /// is never written over a list that could not be read.
    pub fn tests_passed(&self) -> Result<BTreeSet<String>> {
        Ok(self
            .store
            .setting(keys::LESSONS_TESTS_PASSED)?
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
            .into_iter()
            .collect())
    }

    fn progress(&self) -> Result<Progress> {
        let cat = &self.library.catalog;
        let mode = self.unlock_mode();
        let done = self.done_lessons()?;
        let passed = self.tests_passed()?;
        let start = self.start_level()?;
        let start = cat.levels.iter().position(|l| *l == start).unwrap_or(0);
        fn lessons_of<'a>(
            cat: &'a taful_content::Catalog,
            level: &'a str,
        ) -> impl Iterator<Item = &'a taful_content::Entry> + 'a {
            cat.lessons.iter().filter(move |e| e.level == level)
        }
        // A passed test opens the level after it and every level before it,
        // however it was reached (a test passed in Guiado counts in Prova).
        let top = passed
            .iter()
            .filter_map(|l| cat.levels.iter().position(|x| x == l))
            .map(|r| r + 1)
            .max()
            .unwrap_or(0)
            .max(start);
        let mut unlocked = Vec::with_capacity(cat.levels.len());
        let mut open = Vec::with_capacity(cat.levels.len());
        for i in 0..cat.levels.len() {
            let through = |i: usize| {
                let prev = &cat.levels[i - 1];
                passed.contains(prev) || lessons_of(cat, prev).next().is_none()
            };
            let u = i <= top || (unlocked[i - 1] && through(i));
            unlocked.push(u);
            open.push(match mode {
                UnlockMode::Tests => u,
                UnlockMode::Free => true,
                UnlockMode::Stepwise => {
                    u || (open[i - 1] && {
                        let prev = &cat.levels[i - 1];
                        passed.contains(prev) || lessons_of(cat, prev).all(|e| done.contains(&e.id))
                    })
                }
            });
        }
        // Lessons below the starting level come first only where the user
        // already reads; a passed test moves past its level.
        let rank = |level: &str| cat.levels.iter().position(|l| l == level);
        let lowest_done = cat
            .lessons
            .iter()
            .filter(|e| done.contains(&e.id))
            .filter_map(|e| rank(&e.level))
            .min();
        let past_tests = passed.iter().filter_map(|l| rank(l)).map(|r| r + 1).max();
        let floor = start
            .min(lowest_done.unwrap_or(start))
            .max(past_tests.unwrap_or(0));
        Ok(Progress {
            mode,
            done,
            passed,
            unlocked,
            open,
            floor,
        })
    }

    fn level_index(&self, lesson: &str) -> Option<usize> {
        let e = self.library.catalog.entry(lesson)?;
        self.library
            .catalog
            .levels
            .iter()
            .position(|l| *l == e.level)
    }

    fn state_of(&self, id: &str, p: &Progress) -> LessonState {
        if p.done.contains(id) {
            return LessonState::Done;
        }
        let Some(level) = self.level_index(id) else {
            return LessonState::Locked;
        };
        let open = match p.mode {
            UnlockMode::Free => true,
            UnlockMode::Tests => p.open[level],
            UnlockMode::Stepwise => {
                p.open[level] && {
                    let lvl = &self.library.catalog.levels[level];
                    self.library
                        .catalog
                        .lessons
                        .iter()
                        .find(|e| e.level == *lvl && !p.done.contains(&e.id))
                        .is_some_and(|e| e.id == id)
                }
            }
        };
        if open {
            LessonState::Available
        } else {
            LessonState::Locked
        }
    }

    /// The level whose test opens level `level` (Prova): the nearest level
    /// before it that has lessons (an empty level lets the next one
    /// through). `None` when it is open or has no lessons to open.
    fn opening_test(&self, level: usize, p: &Progress) -> Option<String> {
        let cat = &self.library.catalog;
        let has_lessons = |i: usize| cat.lessons.iter().any(|e| e.level == cat.levels[i]);
        if p.unlocked[level] || !has_lessons(level) {
            return None;
        }
        let at = (0..level).rev().find(|&i| has_lessons(i))?;
        Some(cat.levels[at].clone())
    }

    /// The lock state of one lesson.
    pub fn lesson_state(&self, id: &str) -> Result<LessonState> {
        Ok(self.state_of(id, &self.progress()?))
    }

    /// The catalog by level, from zero to pro: every level (empty ones
    /// included) with its lessons in order.
    pub fn journey(&self, locale: Locale) -> Result<LessonList> {
        self.lesson_list(ViewBy::Journey, locale)
    }

    /// The catalog grouped as `view` asks, with the next lesson: the first
    /// open one in Jornada order from where the user stands (the starting
    /// level, or a lower level they already read; past every passed level
    /// test), else the first open one anywhere.
    pub fn lesson_list(&self, view: ViewBy, locale: Locale) -> Result<LessonList> {
        let prog = self.progress()?;
        let progress = self.store.lesson_progress()?;
        let best = |id: &str| {
            progress
                .iter()
                .filter(|p| p.lesson_id == id)
                .filter_map(|p| p.best_score)
                .fold(None, |acc: Option<f64>, s| {
                    Some(acc.map_or(s, |a| a.max(s)))
                })
        };
        let catalog = &self.library.catalog;
        // Each lesson once, in Jornada order; the groups pick from it.
        let cards: Vec<(&taful_content::Entry, LessonCard)> = catalog
            .lessons
            .iter()
            .filter_map(|e| {
                let l = self.library.lesson(&e.id, locale.tag())?;
                Some((
                    e,
                    LessonCard {
                        id: e.id.clone(),
                        title: l.meta.title.clone(),
                        state: self.state_of(&e.id, &prog),
                        minutes: l.meta.minutes,
                        best_score: best(&e.id),
                    },
                ))
            })
            .collect();
        let rank = |level: &str| catalog.levels.iter().position(|l| l == level);
        let available = |(e, c): &&(&taful_content::Entry, LessonCard)| {
            c.state == LessonState::Available && rank(&e.level).is_some()
        };
        let ahead = cards
            .iter()
            .filter(available)
            .find(|(e, _)| rank(&e.level).is_some_and(|r| r >= prog.floor));
        let test_card = |level: usize| {
            let id = &catalog.levels[level];
            let t = self.library.test_for(id)?;
            Some(TestCard {
                state: if prog.passed.contains(id) {
                    TestState::Passed
                } else if prog.open[level] {
                    TestState::Open
                } else {
                    TestState::Locked
                },
                questions: t.draw.min(t.pool.len()),
                pass: t.pass,
            })
        };
        // With nothing left to read from where the user stands in Prova,
        // the test that opens what follows comes before lessons left behind
        // in lower levels.
        let next_test = match (prog.mode, ahead) {
            (UnlockMode::Tests, None) => (0..catalog.levels.len())
                .find(|&i| {
                    prog.unlocked[i]
                        && !prog.passed.contains(&catalog.levels[i])
                        && self.library.test_for(&catalog.levels[i]).is_some()
                        && prog.unlocked.get(i + 1) == Some(&false)
                })
                .map(|i| catalog.levels[i].clone()),
            _ => None,
        };
        let next = ahead
            .or_else(|| {
                next_test
                    .is_none()
                    .then(|| cards.iter().find(available))
                    .flatten()
            })
            .map(|(_, c)| c.id.clone());
        let group = |kind: GroupKind, id: &String| {
            let level = (kind == GroupKind::Level).then(|| rank(id)).flatten();
            LessonGroup {
                kind,
                id: id.clone(),
                lessons: cards
                    .iter()
                    .filter(|(e, _)| match kind {
                        GroupKind::Level => e.level == *id,
                        GroupKind::Theme => e.theme == *id,
                    })
                    .map(|(_, c)| c.clone())
                    .collect(),
                test: level.and_then(test_card),
                opens_with: level
                    .filter(|_| prog.mode == UnlockMode::Tests)
                    .and_then(|l| self.opening_test(l, &prog)),
            }
        };
        let groups = match view {
            ViewBy::Journey => catalog
                .levels
                .iter()
                .map(|id| group(GroupKind::Level, id))
                .collect(),
            ViewBy::Themes => catalog
                .themes
                .iter()
                .map(|id| group(GroupKind::Theme, id))
                .collect(),
        };
        Ok(LessonList {
            view,
            mode: prog.mode,
            groups,
            next,
            next_test,
        })
    }

    /// The view the lessons screen was last left on.
    pub fn lesson_view(&self) -> ViewBy {
        self.string_setting(crate::services::keys::LESSONS_VIEW)
            .and_then(|id| ViewBy::from_id(&id))
            .unwrap_or_default()
    }

    pub fn set_lesson_view(&self, view: ViewBy) -> Result<()> {
        self.set_string_setting(crate::services::keys::LESSONS_VIEW, view.id())
    }

    /// Renders a lesson for reading. Locked lessons can be previewed; the
    /// state says so.
    pub fn open_lesson(&self, id: &str, locale: Locale) -> Result<OpenLesson> {
        let l = self
            .library
            .lesson(id, locale.tag())
            .ok_or_else(|| Error::NotFound(format!("lesson `{id}`")))?;
        let doc = l.render(None).map_err(Error::Content)?;
        let prog = self.progress()?;
        let state = self.state_of(id, &prog);
        if state == LessonState::Available {
            self.store
                .record_lesson_progress(id, l.meta.version, LessonState::Available, None)?;
        }
        let locked_by = match (state, prog.mode) {
            (LessonState::Locked, UnlockMode::Tests) => self
                .level_index(id)
                .and_then(|level| self.opening_test(level, &prog))
                .map(|level| LockReason::Test { level }),
            (LessonState::Locked, UnlockMode::Stepwise) => {
                // The lesson open in its level, else (level not reached yet)
                // the next lesson overall.
                let in_level = self
                    .level_index(id)
                    .filter(|&i| prog.open[i])
                    .and_then(|i| {
                        let level = &self.library.catalog.levels[i];
                        self.library
                            .catalog
                            .lessons
                            .iter()
                            .find(|e| e.level == *level && !prog.done.contains(&e.id))
                            .map(|e| e.id.clone())
                    });
                let next = match in_level {
                    Some(n) => Some(n),
                    None => self.journey(locale)?.next,
                };
                Some(LockReason::OneAtATime { next })
            }
            _ => None,
        };
        let missing = if state == LessonState::Available {
            l.meta
                .prerequisites
                .iter()
                .filter(|p| !prog.done.contains(*p))
                .filter_map(|p| self.library.lesson(p, locale.tag()))
                .map(|p| p.meta.title.clone())
                .collect()
        } else {
            Vec::new()
        };
        let guided = match self.library.guided_for(id) {
            Some(g) => Some(GuidedPart {
                id: g.id.clone(),
                title: crate::guided::text(&g.title, locale).to_owned(),
                finished: self.guided_finished(&g.id)?,
            }),
            None => None,
        };
        Ok(OpenLesson {
            id: id.to_owned(),
            version: l.meta.version,
            title: l.meta.title.clone(),
            state,
            doc,
            glossary: l.meta.glossary.clone(),
            locked_by,
            missing,
            guided,
            quiz_passed: self.quiz_passed(id)?,
        })
    }

    fn quiz_of(&self, lesson_id: &str) -> Result<&Quiz> {
        self.library
            .quiz_for(lesson_id)
            .ok_or_else(|| Error::NotFound(format!("quiz for lesson `{lesson_id}`")))
    }

    /// The lesson's quiz, rendered.
    pub fn quiz(&self, lesson_id: &str, locale: Locale) -> Result<QuizView> {
        let quiz = self.quiz_of(lesson_id)?;
        let questions = quiz
            .questions
            .iter()
            .map(|q| quiz.render_question(q, locale.tag(), None))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::Content)?;
        Ok(QuizView {
            id: quiz.id.clone(),
            version: quiz.version,
            pass: quiz.pass,
            questions,
        })
    }

    /// Grades a finished quiz, stores the attempt, records progress and
    /// feeds the review queue: a wrong answer creates (or reviews) the
    /// question's item as Again; a right answer reviews an existing item as
    /// Good.
    pub fn submit_quiz(
        &mut self,
        lesson_id: &str,
        answers: &[Answer],
        duration_ms: i64,
    ) -> Result<QuizResult> {
        let view = self.quiz(lesson_id, Locale::SOURCE)?;
        if answers.len() != view.questions.len() {
            return Err(Error::NotFound(format!(
                "{} answers for {} questions",
                answers.len(),
                view.questions.len()
            )));
        }
        let correct: Vec<bool> = view
            .questions
            .iter()
            .zip(answers)
            .map(|(q, a)| q.is_correct(*a))
            .collect();
        let score = Quiz::score(&view.questions, answers);
        let passed = score + 1e-9 >= view.pass;
        let attempt_id = self.store.insert_quiz_attempt(&NewQuizAttempt {
            quiz_id: &view.id,
            version: view.version,
            answers: serde_json::to_value(answers).expect("answers serialize"),
            score,
            duration_ms,
        })?;
        for (q, ok) in view.questions.iter().zip(&correct) {
            let key = question_key(&view.id, &q.id);
            let exists = self
                .store
                .review_item(ItemKind::LessonQuiz.id(), &key)?
                .is_some();
            if !*ok || exists {
                self.scheduler.review(
                    &self.store,
                    ItemKind::LessonQuiz,
                    &key,
                    Some(view.version),
                    Rating::from_quiz(*ok),
                    Some(Source {
                        kind: "quiz_attempt",
                        id: &attempt_id,
                    }),
                )?;
            }
        }
        let version = self
            .library
            .lesson(lesson_id, taful_content::SOURCE_LOCALE)
            .map_or(1, |l| l.meta.version);
        self.pin_start_level()?;
        // A guided hand still to play keeps the lesson open (D26). A lesson
        // that is already done stays done.
        let was_done = self.lesson_state(lesson_id)? == LessonState::Done;
        let guided_pending = match self.library.guided_for(lesson_id) {
            Some(g) if passed && !was_done && !self.guided_finished(&g.id)? => Some(g.id.clone()),
            _ => None,
        };
        let before = self.catalog_states()?;
        self.store.record_lesson_progress(
            lesson_id,
            version,
            if passed && guided_pending.is_none() {
                LessonState::Done
            } else {
                LessonState::Available
            },
            Some(score),
        )?;
        let unlocked = newly_unlocked(&before, self.catalog_states()?);
        Ok(QuizResult {
            score,
            passed,
            correct,
            unlocked,
            guided_pending,
        })
    }

    /// Whether the lesson's quiz was passed at any version.
    pub(crate) fn quiz_passed(&self, lesson_id: &str) -> Result<bool> {
        let Some(quiz) = self.library.quiz_for(lesson_id) else {
            return Ok(false);
        };
        Ok(self
            .store
            .lesson_progress()?
            .iter()
            .filter(|p| p.lesson_id == lesson_id)
            .filter_map(|p| p.best_score)
            .any(|s| s + 1e-9 >= quiz.pass))
    }

    pub(crate) fn catalog_states(&self) -> Result<Vec<(String, LessonState)>> {
        let prog = self.progress()?;
        Ok(self
            .library
            .catalog
            .order()
            .into_iter()
            .map(|id| (id.to_owned(), self.state_of(id, &prog)))
            .collect())
    }

    // ---- level tests (D24) ---------------------------------------------------------------

    /// Draws a fresh attempt at `level`'s test, rendered for `locale`. Any
    /// level's test can be taken; the lessons screen offers the ones whose
    /// level is open in the current mode.
    pub fn level_test(&mut self, level: &str, locale: Locale) -> Result<LevelTestView> {
        let test = self
            .library
            .test_for(level)
            .ok_or_else(|| Error::NotFound(format!("level test `{level}`")))?
            .clone();
        let mut seed = self.next_seed();
        let pick = move |n: usize| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) as usize) % n.max(1)
        };
        let lesson_of = |r: &str| {
            r.split_once('/')
                .and_then(|(q, _)| self.library.quizzes.get(q))
                .map(|q| q.lesson.clone())
        };
        let questions = test
            .draw_indexes(lesson_of, pick)
            .into_iter()
            .map(|i| {
                let (quiz, question) = test.pool[i]
                    .split_once('/')
                    .and_then(|(q, x)| {
                        let quiz = self.library.quizzes.get(q)?;
                        Some((quiz, quiz.question(x)?))
                    })
                    .ok_or_else(|| Error::NotFound(format!("question `{}`", test.pool[i])))?;
                quiz.render_question(question, locale.tag(), None)
                    .map_err(Error::Content)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(LevelTestView {
            level: level.to_owned(),
            version: test.version,
            pass: test.pass,
            questions,
        })
    }

    /// Grades a finished level test and stores the attempt. A pass unlocks
    /// the next level (D24); it never marks lessons done or feeds the
    /// review queue (D25: a test is a way to see where one stands).
    pub fn submit_level_test(
        &mut self,
        test: &LevelTestView,
        answers: &[Answer],
        duration_ms: i64,
    ) -> Result<LevelTestResult> {
        if answers.len() != test.questions.len() {
            return Err(Error::NotFound(format!(
                "{} answers for {} questions",
                answers.len(),
                test.questions.len()
            )));
        }
        let correct: Vec<bool> = test
            .questions
            .iter()
            .zip(answers)
            .map(|(q, a)| q.is_correct(*a))
            .collect();
        let score = Quiz::score(&test.questions, answers);
        let passed = score + 1e-9 >= test.pass;
        self.store.insert_quiz_attempt(&NewQuizAttempt {
            quiz_id: &level_test_attempt_id(&test.level),
            version: test.version,
            answers: serde_json::to_value(answers).expect("answers serialize"),
            score,
            duration_ms,
        })?;
        let mut unlocked = Vec::new();
        let mut set = self.tests_passed()?;
        if passed && !set.contains(&test.level) {
            self.pin_start_level()?;
            let before = self.progress()?.open;
            set.insert(test.level.clone());
            self.store.set_setting(
                keys::LESSONS_TESTS_PASSED,
                &json!(set.iter().collect::<Vec<_>>()),
            )?;
            let after = self.progress()?.open;
            unlocked = self
                .library
                .catalog
                .levels
                .iter()
                .zip(before.iter().zip(&after))
                .filter(|(_, (b, a))| !**b && **a)
                .map(|(l, _)| l.clone())
                .collect();
        }
        Ok(LevelTestResult {
            score,
            passed,
            correct,
            unlocked,
        })
    }
}

/// The `quiz_attempts.quiz_id` a level test's attempts are stored under.
pub fn level_test_attempt_id(level: &str) -> String {
    format!("level-test/{level}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tests::app_at;

    fn right_answers(view: &QuizView) -> Vec<Answer> {
        view.questions
            .iter()
            .map(|q| match (q.answer_index, &q.numeric) {
                (Some(i), _) => Answer::Choice(i),
                (_, Some((v, _))) => Answer::Numeric(*v),
                _ => unreachable!(),
            })
            .collect()
    }

    fn lessons_of(app: &App, level: &str) -> Vec<String> {
        app.library
            .catalog
            .lessons
            .iter()
            .filter(|e| e.level == level)
            .map(|e| e.id.clone())
            .collect()
    }

    /// Marks lessons done as a quiz pass would (the starting level is
    /// stored first).
    fn mark_done(app: &App, ids: &[String]) {
        app.pin_start_level().unwrap();
        for id in ids {
            app.store
                .record_lesson_progress(id, 1, LessonState::Done, Some(1.0))
                .unwrap();
        }
    }

    fn states(app: &App) -> Vec<(String, LessonState)> {
        app.lesson_list(ViewBy::Journey, Locale::PtBr)
            .unwrap()
            .lessons()
            .map(|l| (l.id.clone(), l.state))
            .collect()
    }

    fn group<'a>(list: &'a LessonList, id: &str) -> &'a LessonGroup {
        list.groups.iter().find(|g| g.id == id).unwrap()
    }

    fn all(g: &LessonGroup, s: LessonState) -> bool {
        g.lessons.iter().all(|l| l.state == s)
    }

    /// Passes `level`'s test with the right answers.
    fn pass_test(app: &mut App, level: &str) -> LevelTestResult {
        let t = app.level_test(level, Locale::PtBr).unwrap();
        let answers = right_answers_of(&t.questions);
        app.submit_level_test(&t, &answers, 1).unwrap()
    }

    fn right_answers_of(qs: &[RenderedQuestion]) -> Vec<Answer> {
        qs.iter()
            .map(|q| match (q.answer_index, &q.numeric) {
                (Some(i), _) => Answer::Choice(i),
                (_, Some((v, _))) => Answer::Numeric(*v),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn prova_opens_the_level_and_its_test_opens_the_next() {
        let (mut app, _) = app_at(1_000);
        assert_eq!(app.unlock_mode(), UnlockMode::Tests);
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.mode, UnlockMode::Tests);
        // Primeiros passos has no lessons yet, so Fundamentos is open.
        let fund = group(&list, "fundamentos");
        assert!(all(fund, LessonState::Available));
        assert_eq!(fund.test.as_ref().unwrap().state, TestState::Open);
        assert_eq!(fund.test.as_ref().unwrap().questions, 10);
        assert_eq!(fund.opens_with, None);
        let ini = group(&list, "iniciante");
        assert!(all(ini, LessonState::Locked));
        assert_eq!(ini.opens_with.as_deref(), Some("fundamentos"));
        assert_eq!(ini.test.as_ref().unwrap().state, TestState::Locked);
        // Each closed level names the test right before it.
        assert_eq!(
            group(&list, "intermediario").opens_with.as_deref(),
            Some("iniciante")
        );
        assert!(group(&list, "primeiros-passos").test.is_none());
        let soon = list.groups.last().unwrap();
        assert!(
            soon.is_soon() && soon.opens_with.is_none(),
            "nothing to open"
        );
        let first = list.lessons().next().unwrap().clone();
        assert!(list.is_next(&first.id));
        assert_eq!(list.next_test, None);

        // A quiz marks its lesson done and opens nothing else.
        let view = app.quiz(&first.id, Locale::PtBr).unwrap();
        let res = app
            .submit_quiz(&first.id, &right_answers(&view), 30_000)
            .unwrap();
        assert!(res.passed && res.score == 1.0);
        assert!(res.unlocked.is_empty());
        assert_eq!(app.lesson_state(&first.id).unwrap(), LessonState::Done);
        assert_eq!(app.store.review_totals().unwrap(), (0, 0));
        let home = app.home(Locale::PtBr).unwrap();
        assert_eq!(home.lessons_done, 1);
        assert_eq!(
            home.next_lesson.unwrap().0,
            lessons_of(&app, "fundamentos")[1]
        );

        // The level test opens Iniciante; nothing is marked done.
        let res = pass_test(&mut app, "fundamentos");
        assert!(res.passed);
        assert_eq!(res.unlocked, vec!["iniciante".to_string()]);
        assert!(res.correct.iter().all(|c| *c));
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.done(), 1);
        assert!(all(group(&list, "iniciante"), LessonState::Available));
        assert_eq!(
            group(&list, "fundamentos").test.as_ref().unwrap().state,
            TestState::Passed
        );
        assert_eq!(
            group(&list, "iniciante").test.as_ref().unwrap().state,
            TestState::Open
        );
        assert_eq!(
            group(&list, "intermediario").opens_with.as_deref(),
            Some("iniciante")
        );
        // A passed test moves the next lesson past its level.
        assert_eq!(list.next.as_ref(), lessons_of(&app, "iniciante").first());
        // No lesson became done and no review item was made.
        assert_eq!(app.home(Locale::PtBr).unwrap().lessons_done, 1);
        assert_eq!(app.store.review_totals().unwrap(), (0, 0));
        // Passing again unlocks nothing new; attempts are kept.
        assert!(pass_test(&mut app, "fundamentos").unlocked.is_empty());
        assert_eq!(
            app.store
                .quiz_attempt_count(&level_test_attempt_id("fundamentos"))
                .unwrap(),
            2
        );
    }

    #[test]
    fn prova_suggests_the_test_when_the_level_is_done() {
        let (app, _) = app_at(1_000);
        mark_done(&app, &lessons_of(&app, "fundamentos"));
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next, None);
        assert_eq!(list.next_test.as_deref(), Some("fundamentos"));
        // Doing every lesson does not pass the level.
        assert!(all(group(&list, "iniciante"), LessonState::Locked));
        // Other modes never suggest a test.
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next_test, None);
        assert_eq!(list.next.as_ref(), lessons_of(&app, "iniciante").first());
    }

    #[test]
    fn a_failed_test_unlocks_nothing() {
        let (mut app, _) = app_at(1_000);
        let t = app.level_test("fundamentos", Locale::EnUs).unwrap();
        let mut answers = right_answers_of(&t.questions);
        for a in answers.iter_mut().take(3) {
            *a = match *a {
                Answer::Choice(i) => Answer::Choice(i + 1),
                Answer::Numeric(x) => Answer::Numeric(x + 1000.0),
            };
        }
        let res = app.submit_level_test(&t, &answers, 1).unwrap();
        assert!(!res.passed);
        assert!(res.unlocked.is_empty());
        assert!(app.tests_passed().unwrap().is_empty());
        assert_eq!(
            app.store.review_totals().unwrap(),
            (0, 0),
            "no review items"
        );
        assert!(app.submit_level_test(&t, &[], 1).is_err());
        assert!(app.level_test("primeiros-passos", Locale::EnUs).is_err());
    }

    #[test]
    fn a_test_draws_one_question_per_lesson_first() {
        let (mut app, _) = app_at(1_000);
        for (level, draw) in [
            ("fundamentos", 10),
            ("iniciante", 12),
            ("intermediario", 12),
        ] {
            let lessons = lessons_of(&app, level);
            for _ in 0..5 {
                let t = app.level_test(level, Locale::PtBr).unwrap();
                assert_eq!(t.questions.len(), draw, "{level}");
                assert!(t.pass > 0.0);
                // Spread: no lesson twice before every lesson once.
                let mut keys: Vec<String> = t
                    .questions
                    .iter()
                    .map(|q| {
                        let quiz = app
                            .library
                            .quizzes
                            .values()
                            .find(|z| {
                                z.questions.iter().any(|x| x.id == q.id)
                                    && lessons.contains(&z.lesson)
                                    && z.render_question(z.question(&q.id).unwrap(), "pt-BR", None)
                                        .is_ok_and(|r| r.prompt == q.prompt)
                            })
                            .unwrap();
                        quiz.lesson.clone()
                    })
                    .collect();
                keys.sort();
                let n = keys.len();
                keys.dedup();
                assert_eq!(keys.len(), n.min(lessons.len()), "{level}: {keys:?}");
            }
        }
    }

    #[test]
    fn guiado_opens_one_lesson_at_a_time() {
        let (mut app, _) = app_at(1_000);
        app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
        let fund = lessons_of(&app, "fundamentos");
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        let open: Vec<&LessonCard> = list
            .lessons()
            .filter(|l| l.state == LessonState::Available)
            .collect();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, fund[0]);
        // Passing a quiz opens the next one.
        let view = app.quiz(&fund[0], Locale::PtBr).unwrap();
        let res = app.submit_quiz(&fund[0], &right_answers(&view), 1).unwrap();
        assert_eq!(res.unlocked, vec![fund[1].clone()]);
        // A locked lesson says which one is next.
        let open = app.open_lesson(&fund[3], Locale::PtBr).unwrap();
        assert_eq!(open.state, LessonState::Locked);
        assert_eq!(
            open.locked_by,
            Some(LockReason::OneAtATime {
                next: Some(fund[1].clone())
            })
        );
        // The level done, the next level's first lesson opens without a test.
        mark_done(&app, &fund);
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        let ini = group(&list, "iniciante");
        assert_eq!(ini.lessons[0].state, LessonState::Available);
        assert!(
            ini.lessons[1..]
                .iter()
                .all(|l| l.state == LessonState::Locked)
        );
        assert_eq!(ini.opens_with, None, "only Prova waits for a test");
        assert!(all(group(&list, "intermediario"), LessonState::Locked));
        // A level test skips a level here too.
        pass_test(&mut app, "iniciante");
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(
            group(&list, "intermediario").lessons[0].state,
            LessonState::Available
        );
        assert_eq!(
            group(&list, "iniciante").lessons[0].state,
            LessonState::Available
        );
    }

    #[test]
    fn livre_opens_everything_and_warns_about_prerequisites() {
        let (app, _) = app_at(1_000);
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert!(list.lessons().all(|l| l.state == LessonState::Available));
        assert_eq!(list.next.as_ref(), lessons_of(&app, "fundamentos").first());
        // Tests are there to see where one stands.
        assert!(
            list.groups
                .iter()
                .filter_map(|g| g.test.as_ref())
                .all(|t| t.state == TestState::Open)
        );
        let open = app.open_lesson("facing-an-open", Locale::EnUs).unwrap();
        assert_eq!(open.state, LessonState::Available);
        assert_eq!(open.locked_by, None);
        let titles = |ids: &[&str]| -> Vec<String> {
            ids.iter()
                .map(|id| app.library.lesson(id, "en-US").unwrap().meta.title.clone())
                .collect()
        };
        assert_eq!(open.missing, titles(&["opening-ranges", "pot-odds-101"]));
        mark_done(&app, &["opening-ranges".to_string()]);
        let open = app.open_lesson("facing-an-open", Locale::EnUs).unwrap();
        assert_eq!(open.missing, titles(&["pot-odds-101"]));
        assert!(
            app.open_lesson("hand-rankings", Locale::EnUs)
                .unwrap()
                .missing
                .is_empty()
        );
    }

    #[test]
    fn switching_modes_never_loses_progress() {
        let (mut app, _) = app_at(1_000);
        let fund = lessons_of(&app, "fundamentos");
        mark_done(&app, &fund[..3]);
        pass_test(&mut app, "fundamentos");
        let prova = states(&app);
        for mode in [
            UnlockMode::Stepwise,
            UnlockMode::Free,
            UnlockMode::Tests,
            UnlockMode::Free,
            UnlockMode::Stepwise,
        ] {
            app.set_unlock_mode(mode).unwrap();
            assert_eq!(app.unlock_mode(), mode);
            let now = states(&app);
            for id in &fund[..3] {
                assert!(
                    now.contains(&(id.clone(), LessonState::Done)),
                    "{mode:?} {id}"
                );
            }
            assert_eq!(
                app.lesson_list(ViewBy::Journey, Locale::PtBr)
                    .unwrap()
                    .done(),
                3
            );
            assert!(app.tests_passed().unwrap().contains("fundamentos"));
        }
        app.set_unlock_mode(UnlockMode::Tests).unwrap();
        assert_eq!(states(&app), prova, "back to Prova, the same states");
        assert_eq!(UnlockMode::from_id("nope"), None);
        for m in UnlockMode::ALL {
            assert_eq!(UnlockMode::from_id(m.id()), Some(m));
        }
    }

    #[test]
    fn the_starting_level_unlocks_without_marking_anything_done() {
        let (app, _) = app_at(1_000);
        assert_eq!(app.chosen_start_level().unwrap(), None);
        assert_eq!(app.start_level().unwrap(), "primeiros-passos");
        app.set_start_level("intermediario").unwrap();
        assert!(app.set_start_level("nope").is_err());
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        for level in ["fundamentos", "iniciante", "intermediario"] {
            assert!(all(group(&list, level), LessonState::Available), "{level}");
        }
        assert_eq!(list.done(), 0);
        assert!(app.tests_passed().unwrap().is_empty());
        // The next lesson starts at the chosen level...
        assert_eq!(
            list.next.as_ref(),
            lessons_of(&app, "intermediario").first()
        );
        // ...unless the user already reads a lower one.
        mark_done(&app, &lessons_of(&app, "fundamentos")[..1]);
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next.as_ref(), lessons_of(&app, "fundamentos").get(1));
        // Guiado: lower levels stay readable, one lesson at a time each.
        app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        for level in ["fundamentos", "iniciante", "intermediario"] {
            let open = group(&list, level)
                .lessons
                .iter()
                .filter(|l| l.state == LessonState::Available)
                .count();
            assert_eq!(open, 1, "{level}");
        }
    }

    #[test]
    fn progress_from_before_the_modes_keeps_every_lesson_it_had_open() {
        let (mut app, _) = app_at(1_000);
        // The lessons a user had done under the old prerequisite rule.
        let done: Vec<String> = [
            "hand-rankings",
            "positions-6max",
            "stacks-and-blinds",
            "combos-101",
            "equity-101",
            "outs-101",
            "rule-of-2-and-4",
            "pot-odds-101",
            "ranges-101",
            "opening-ranges",
        ]
        .map(String::from)
        .to_vec();
        let old_progress = |ids: &[String]| {
            for id in ids {
                app.store
                    .record_lesson_progress(id, 1, LessonState::Done, Some(1.0))
                    .unwrap();
            }
        };
        old_progress(&done[..5]);
        // No Iniciante lesson had every prerequisite done yet.
        assert_eq!(app.start_level().unwrap(), "fundamentos");
        old_progress(&done[5..]);
        // Now ev-101, mdf-101, facing-an-open… (Iniciante) had.
        assert_eq!(app.start_level().unwrap(), "iniciante");
        assert_eq!(
            app.chosen_start_level().unwrap(),
            None,
            "derived, not stored"
        );
        assert_eq!(
            app.lesson_state("facing-an-open").unwrap(),
            LessonState::Available
        );
        // New progress first stores where the user stood.
        let id = lessons_of(&app, "iniciante")[0].clone();
        let view = app.quiz(&id, Locale::PtBr).unwrap();
        app.submit_quiz(&id, &right_answers(&view), 1).unwrap();
        assert_eq!(
            app.chosen_start_level().unwrap().as_deref(),
            Some("iniciante")
        );
        // Prova shows nothing below Iniciante as next: Fundamentos is done.
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next.as_ref(), lessons_of(&app, "iniciante").get(1));
    }

    #[test]
    fn a_locked_prova_lesson_names_the_test_that_opens_it() {
        let (app, _) = app_at(1_000);
        let id = lessons_of(&app, "intermediario")[0].clone();
        let open = app.open_lesson(&id, Locale::PtBr).unwrap();
        assert_eq!(open.state, LessonState::Locked);
        assert_eq!(
            open.locked_by,
            Some(LockReason::Test {
                level: "iniciante".into()
            })
        );
        assert!(open.missing.is_empty());
    }

    #[test]
    fn a_skipped_level_does_not_hide_the_next_test() {
        // Fundamentos skipped with its test, Iniciante read to the end: the
        // Iniciante test comes before the unread Fundamentos lessons.
        let (mut app, _) = app_at(1_000);
        assert!(pass_test(&mut app, "fundamentos").passed);
        mark_done(&app, &lessons_of(&app, "iniciante"));
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next_test.as_deref(), Some("iniciante"));
        assert_eq!(list.next, None);
        // In Livre the unread lessons are still the next step.
        app.set_unlock_mode(UnlockMode::Free).unwrap();
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert_eq!(list.next_test, None);
        assert!(list.next.is_some());
    }

    #[test]
    fn a_test_passed_in_guiado_still_opens_levels_in_prova() {
        let (mut app, _) = app_at(1_000);
        app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
        mark_done(&app, &lessons_of(&app, "fundamentos"));
        assert!(pass_test(&mut app, "iniciante").passed);
        app.set_unlock_mode(UnlockMode::Tests).unwrap();
        let list = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        assert!(all(group(&list, "iniciante"), LessonState::Available));
        assert!(all(group(&list, "intermediario"), LessonState::Available));
        assert_eq!(group(&list, "intermediario").opens_with, None);
    }

    #[test]
    fn a_guiado_note_names_the_lesson_open_in_that_level() {
        let (app, _) = app_at(1_000);
        app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
        app.set_start_level("intermediario").unwrap();
        let fund = lessons_of(&app, "fundamentos");
        let open = app.open_lesson(&fund[2], Locale::PtBr).unwrap();
        assert_eq!(
            open.locked_by,
            Some(LockReason::OneAtATime {
                next: Some(fund[0].clone())
            })
        );
    }

    #[test]
    fn wrong_answers_land_in_the_queue() {
        let (mut app, _) = app_at(1_000);
        let id = app.library.catalog.order()[0].to_owned();
        let view = app.quiz(&id, Locale::EnUs).unwrap();
        let mut answers = right_answers(&view);
        answers[0] = match answers[0] {
            Answer::Choice(i) => Answer::Choice(i + 1),
            Answer::Numeric(x) => Answer::Numeric(x + 1000.0),
        };
        let res = app.submit_quiz(&id, &answers, 10_000).unwrap();
        assert!(!res.correct[0]);
        let key = question_key(&view.id, &view.questions[0].id);
        let item = app.store.review_item("lesson_quiz", &key).unwrap().unwrap();
        assert_eq!(item.content_version, Some(view.version));
        assert!(
            app.submit_quiz(&id, &[], 1).is_err(),
            "answer count must match"
        );
    }

    /// A lesson with a guided hand is done once both its quiz is passed
    /// and the hand finished, in either order (D26).
    #[test]
    fn a_guided_hand_completes_the_lesson_with_the_quiz() {
        let lesson = "stacks-and-blinds";
        // Guiado, with every lesson before it done, so the next one opens
        // only when this one is done.
        let ready = |app: &mut App| {
            app.set_unlock_mode(UnlockMode::Stepwise).unwrap();
            let before: Vec<String> = app
                .library
                .catalog
                .order()
                .into_iter()
                .take_while(|l| *l != lesson)
                .map(str::to_owned)
                .collect();
            for p in before {
                app.store
                    .record_lesson_progress(&p, 1, LessonState::Done, Some(1.0))
                    .unwrap();
            }
        };
        // Quiz first, then the hand.
        let (mut app, _) = app_at(1_000);
        ready(&mut app);
        let open = app.open_lesson(lesson, Locale::PtBr).unwrap();
        let g = open.guided.expect("the lesson lists a guided hand");
        assert_eq!((g.id.as_str(), g.finished), ("first-hand", false));
        assert_eq!(g.title, "Sua primeira mão");
        let view = app.quiz(lesson, Locale::EnUs).unwrap();
        let res = app.submit_quiz(lesson, &right_answers(&view), 1).unwrap();
        assert!(res.passed);
        assert_eq!(res.guided_pending.as_deref(), Some("first-hand"));
        assert!(res.unlocked.is_empty(), "nothing opens before the hand");
        assert_eq!(app.lesson_state(lesson).unwrap(), LessonState::Available);
        let fin = app.finish_guided("first-hand").unwrap();
        assert_eq!(fin.done, vec![lesson]);
        assert!(fin.unlocked.iter().any(|l| l == "combos-101"));
        assert_eq!(app.lesson_state(lesson).unwrap(), LessonState::Done);
        assert!(
            app.open_lesson(lesson, Locale::EnUs)
                .unwrap()
                .guided
                .unwrap()
                .finished
        );
        assert_eq!(app.finish_guided("first-hand").unwrap(), Default::default());

        // The hand first, then the quiz.
        let (mut app, _) = app_at(1_000);
        ready(&mut app);
        assert_eq!(app.finish_guided("first-hand").unwrap(), Default::default());
        assert_eq!(app.lesson_state(lesson).unwrap(), LessonState::Available);
        let view = app.quiz(lesson, Locale::EnUs).unwrap();
        let res = app.submit_quiz(lesson, &right_answers(&view), 1).unwrap();
        assert!(res.passed && res.guided_pending.is_none());
        assert_eq!(app.lesson_state(lesson).unwrap(), LessonState::Done);
        assert!(app.finish_guided("nope").is_err());

        // A lesson done before it had a guided hand stays done, and a
        // retaken quiz does not ask for the hand.
        let (mut app, _) = app_at(1_000);
        ready(&mut app);
        app.store
            .record_lesson_progress(lesson, 1, LessonState::Done, Some(1.0))
            .unwrap();
        let view = app.quiz(lesson, Locale::EnUs).unwrap();
        let res = app.submit_quiz(lesson, &right_answers(&view), 1).unwrap();
        assert!(res.passed && res.guided_pending.is_none());
        assert_eq!(app.lesson_state(lesson).unwrap(), LessonState::Done);
    }

    #[test]
    fn both_views_hold_every_lesson_with_the_same_state() {
        let (mut app, _) = app_at(1_000);
        let first = app.library.catalog.order()[0].to_owned();
        let view = app.quiz(&first, Locale::PtBr).unwrap();
        app.submit_quiz(&first, &right_answers(&view), 1).unwrap();

        let journey = app.lesson_list(ViewBy::Journey, Locale::PtBr).unwrap();
        let themes = app.lesson_list(ViewBy::Themes, Locale::PtBr).unwrap();
        let total = app.library.catalog.lessons.len();
        assert_eq!(journey.total(), total);
        assert_eq!(themes.total(), total);
        assert_eq!(journey.done(), 1);
        assert_eq!(themes.done(), 1);
        // The next lesson is the Jornada's in either view.
        assert_eq!(journey.next, themes.next);
        let state =
            |list: &LessonList, id: &str| list.lessons().find(|l| l.id == id).map(|l| l.state);
        for l in journey.lessons() {
            assert_eq!(state(&themes, &l.id), Some(l.state), "{}", l.id);
        }
        // Every level shows, the ones without lessons as "em breve".
        assert_eq!(journey.groups.len(), app.library.catalog.levels.len());
        assert!(
            journey.groups[0].is_soon(),
            "primeiros passos has no lessons yet"
        );
        assert_eq!(journey.groups[0].title_key(), "level-primeiros-passos");
        assert!(themes.groups.iter().all(|g| g.kind == GroupKind::Theme));
        // Within a theme, Jornada order.
        let order = app.library.catalog.order();
        for g in &themes.groups {
            let at: Vec<usize> = g
                .lessons
                .iter()
                .map(|l| order.iter().position(|o| *o == l.id).unwrap())
                .collect();
            assert!(at.windows(2).all(|w| w[0] < w[1]), "{}", g.id);
        }
    }

    #[test]
    fn the_view_is_remembered() {
        let (app, _) = app_at(1_000);
        assert_eq!(app.lesson_view(), ViewBy::Journey);
        app.set_lesson_view(ViewBy::Themes).unwrap();
        assert_eq!(app.lesson_view(), ViewBy::Themes);
        assert_eq!(ViewBy::from_id("nope"), None);
    }

    #[test]
    fn every_lesson_and_quiz_renders_in_both_locales() {
        let (app, _) = app_at(1_000);
        for id in app.library.catalog.order() {
            for loc in Locale::ALL {
                app.open_lesson(id, loc).unwrap();
                app.quiz(id, loc).unwrap();
            }
        }
    }
}

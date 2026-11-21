//! Guided hands on the table (D26): the step machine from `taful-content`,
//! the coach lines in the user's locale, and the lessons a finished hand
//! counts for. Nothing here is graded or stored as a decision or a hand.

use std::sync::Arc;

use taful_content::quiz::Localized;
use taful_store::LessonState;

pub use taful_content::guided::{Click, Guide, Pause, Run};

use crate::i18n::Locale;
use crate::{App, Error, Result};

/// A coach line in `locale`, else in the source locale.
pub fn text(t: &Localized, locale: Locale) -> &str {
    t.get(locale.tag())
        .or_else(|| t.get(Locale::SOURCE.tag()))
        .map_or("", String::as_str)
}

/// What finishing a guided hand changed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuidedFinish {
    /// Lessons that are done now.
    pub done: Vec<String>,
    /// Lessons that became available because of them.
    pub unlocked: Vec<String>,
}

impl App {
    /// The guided hand `id`, at its first pause.
    pub fn guided(&self, id: &str) -> Result<Run> {
        let script = self
            .library
            .guided
            .get(id)
            .ok_or_else(|| Error::NotFound(format!("guided hand `{id}`")))?;
        // The same checks as CI, so a broken script never reaches the table.
        let errs = taful_content::guided::check_script(script);
        if !errs.is_empty() {
            return Err(Error::Content(errs));
        }
        let guide = Guide::compile(script).map_err(Error::Content)?;
        Run::start(Arc::new(guide)).map_err(|e| Error::Content(vec![e]))
    }

    /// Notes that the guided hand `id` was played to the end. Every lesson
    /// that lists it and whose quiz is already passed is done.
    pub fn finish_guided(&mut self, id: &str) -> Result<GuidedFinish> {
        let version = self
            .library
            .guided
            .get(id)
            .ok_or_else(|| Error::NotFound(format!("guided hand `{id}`")))?
            .version;
        self.store.record_guided_done(id, version)?;
        let lessons: Vec<(String, u32)> = self
            .library
            .lessons
            .keys()
            .filter_map(|l| {
                let lesson = self.library.lesson(l, taful_content::SOURCE_LOCALE)?;
                (lesson.meta.guided.as_deref() == Some(id))
                    .then(|| (l.clone(), lesson.meta.version))
            })
            .collect();
        let before = self.skill_tree_states()?;
        let mut done = Vec::new();
        for (lesson, version) in lessons {
            if self.lesson_state(&lesson)? != LessonState::Done && self.quiz_passed(&lesson)? {
                self.store
                    .record_lesson_progress(&lesson, version, LessonState::Done, None)?;
                done.push(lesson);
            }
        }
        let unlocked = if done.is_empty() {
            Vec::new()
        } else {
            crate::lessons::newly_unlocked(&before, self.skill_tree_states()?)
        };
        Ok(GuidedFinish { done, unlocked })
    }

    /// Whether the guided hand `id` was ever played to the end.
    pub fn guided_finished(&self, id: &str) -> Result<bool> {
        Ok(self.store.guided_done()?.iter().any(|g| g == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tests::app_at;
    use taful_content::guided::Pause;

    /// The shipped sample plays to its end on the right clicks.
    #[test]
    fn the_sample_hand_plays_through() {
        let (app, _) = app_at(1_000);
        let mut run = app.guided("first-hand").unwrap();
        let mut steps = 0;
        loop {
            // The counter moves one pause at a time, silent steps aside.
            assert_eq!(run.progress().0, steps);
            match run.pause() {
                Pause::Say(t) => {
                    assert!(!text(t, Locale::PtBr).is_empty());
                    run.proceed().unwrap();
                }
                Pause::Hero(_) => {
                    let right = run
                        .actions()
                        .into_iter()
                        .find(|&a| run.clone().act(a).unwrap() == Click::Right)
                        .expect("a right button");
                    run.act(right).unwrap();
                }
                Pause::Done => break,
            }
            steps += 1;
            assert!(steps < 100);
        }
        assert_eq!(run.progress(), (steps, steps));
        let st = run.settlement().unwrap();
        assert!(st.showdown);
        assert_eq!(st.pots[0].winners, vec![run.hero()]);
    }

    #[test]
    fn text_falls_back_to_the_source_locale() {
        let t = Localized::from([("en-US".to_string(), "Hi".to_string())]);
        assert_eq!(text(&t, Locale::PtBr), "Hi");
        assert_eq!(text(&Localized::new(), Locale::PtBr), "");
    }
}

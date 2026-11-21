//! The Home's next step (D27) in words, and where its button goes. Every
//! style draws the step in its own hero from these pieces.

use gpui::{ClickEvent, Context, Window};
use taful_app::next_step::{NextStep, Practice, StepLesson};

use crate::Ui;
use crate::screen::Screen;
use crate::shell::Shell;

/// A step's texts.
pub struct Words {
    /// What the step is about, after "Próximo passo": "Revisões", the
    /// lesson's level, "Mesa".
    pub about: String,
    pub title: String,
    /// One line under the title: the lesson's place and reading time, the
    /// reviews' minutes, what the table practices.
    pub body: String,
    /// The body cut to what one short line holds: the table's practice
    /// alone, without the lesson it follows.
    pub line: String,
    /// A second, optional sentence: the level test that opens the next
    /// level.
    pub note: Option<String>,
    pub button: String,
}

impl Words {
    /// "Próximo passo · Revisões".
    pub fn eyebrow(&self, ui: &Ui) -> String {
        format!("{} · {}", ui.i18n.tr("next-step-label"), self.about)
    }

    /// The body and the note as one paragraph.
    pub fn paragraph(&self) -> String {
        match &self.note {
            Some(note) => format!("{} {note}", self.body),
            None => self.body.clone(),
        }
    }
}

/// "Lição 3 de 10 · 7 min de leitura e um quiz".
pub fn lesson_meta(ui: &Ui, l: &StepLesson) -> String {
    let i18n = &ui.i18n;
    let place = i18n.tr_args(
        "next-step-lesson-place",
        &[("n", l.number.into()), ("total", l.level_len().into())],
    );
    match l.minutes {
        Some(m) => format!(
            "{place} · {}",
            i18n.tr_args("next-step-lesson-minutes", &[("minutes", m.into())])
        ),
        None => place,
    }
}

/// What the table practices, as a phrase: "responder a um aumento antes do
/// flop".
pub fn practice_phrase(ui: &Ui, practice: Practice) -> String {
    ui.i18n.tr(&format!("next-step-practice-{}", practice.id()))
}

pub fn words(ui: &Ui, step: &NextStep) -> Words {
    let i18n = &ui.i18n;
    match step {
        NextStep::Review { due, minutes } => {
            let body = i18n.tr_args("next-step-review-body", &[("minutes", (*minutes).into())]);
            Words {
                about: i18n.tr("next-step-review"),
                title: i18n.tr_args("next-step-review-title", &[("count", (*due).into())]),
                line: body.clone(),
                body,
                note: None,
                button: i18n.tr("next-step-review-go"),
            }
        }
        NextStep::Lesson(l) => Words {
            about: i18n.tr(&format!("level-{}", l.level)),
            title: l.title.clone(),
            body: lesson_meta(ui, l),
            line: lesson_meta(ui, l),
            note: None,
            button: i18n.tr(if l.started {
                "next-step-lesson-continue"
            } else {
                "next-step-lesson-go"
            }),
        },
        NextStep::Table {
            practice,
            after,
            test,
        } => {
            let phrase = practice_phrase(ui, *practice);
            Words {
                about: i18n.tr("next-step-table"),
                title: i18n.tr("next-step-table-title"),
                line: capitalized(&phrase),
                body: match after {
                    Some(lesson) => i18n.tr_args(
                        "next-step-table-after",
                        &[
                            ("lesson", lesson.clone().into()),
                            ("practice", phrase.into()),
                        ],
                    ),
                    None => i18n.tr_args("next-step-table-plain", &[("practice", phrase.into())]),
                },
                note: test.as_ref().map(|level| {
                    let name = i18n.tr(&format!("level-{level}"));
                    i18n.tr_args("next-step-table-test", &[("level", name.into())])
                }),
                button: i18n.tr("next-step-table-go"),
            }
        }
    }
}

/// `text` with its first letter in upper case.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Takes the step: the review session, the lesson, or the table set up for
/// the practice.
pub fn go(shell: &mut Shell, step: &NextStep, window: &mut Window, cx: &mut Context<Shell>) {
    match step {
        NextStep::Review { .. } => shell.select(Screen::Review, window, cx),
        NextStep::Lesson(l) => {
            shell.open_lesson(&l.id);
            shell.select(Screen::Lesson, window, cx);
        }
        NextStep::Table { practice, .. } => {
            let practice = *practice;
            shell
                .table
                .update(cx, |t, cx| t.start_practice(practice, cx));
            shell.select(Screen::Table, window, cx);
        }
    }
}

/// [`go`] on click, for a hero's button.
pub fn on_click(
    step: NextStep,
    cx: &mut Context<Shell>,
) -> impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(move |this: &mut Shell, _: &ClickEvent, window, cx| go(this, &step, window, cx))
}

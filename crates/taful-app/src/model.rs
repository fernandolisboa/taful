//! Domain types the UI needs, re-exported so `apps/desktop` depends on
//! `taful-app` alone (spec §4 dependency rule).

pub use taful_content::lesson::{Block, Document, Span};
pub use taful_content::quiz::{Answer, RenderedQuestion};
pub use taful_core::calc::{CalcSettings, Exercise, ExerciseKind, Prompt, Unit};
pub use taful_core::grading::Thresholds;
pub use taful_core::srs::Rating;
pub use taful_store::{LessonState, Millis};

pub use crate::calc_trainer::{CalcOutcome, CalcRound};
pub use crate::lessons::{LessonCard, OpenLesson, QuizResult, QuizView, TrackView};
pub use crate::review_session::ReviewPrompt;

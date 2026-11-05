//! The first-slice screens (spec §15.1) and their navigation labels.

/// A top-level screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    SkillTree,
    Lesson,
    Table,
    HandReview,
    Calc,
    Review,
    Settings,
}

impl Screen {
    /// Navigation order.
    pub const ALL: [Screen; 8] = [
        Screen::Home,
        Screen::SkillTree,
        Screen::Lesson,
        Screen::Table,
        Screen::HandReview,
        Screen::Calc,
        Screen::Review,
        Screen::Settings,
    ];

    /// Fluent id of the navigation label, also used as the screen title.
    pub fn nav_key(self) -> &'static str {
        match self {
            Screen::Home => "nav-home",
            Screen::SkillTree => "nav-skill-tree",
            Screen::Lesson => "nav-lesson",
            Screen::Table => "nav-table",
            Screen::HandReview => "nav-hand-review",
            Screen::Calc => "nav-calc",
            Screen::Review => "nav-review",
            Screen::Settings => "nav-settings",
        }
    }

    /// Fluent id of the one-line description shown until the screen exists.
    pub fn body_key(self) -> &'static str {
        match self {
            Screen::Home => "screen-home-body",
            Screen::SkillTree => "screen-skill-tree-body",
            Screen::Lesson => "screen-lesson-body",
            Screen::Table => "screen-table-body",
            Screen::HandReview => "screen-hand-review-body",
            Screen::Calc => "screen-calc-body",
            Screen::Review => "screen-review-body",
            Screen::Settings => "screen-settings-body",
        }
    }

    /// Milestone that builds the screen (spec §18).
    pub fn milestone(self) -> &'static str {
        match self {
            Screen::Home
            | Screen::SkillTree
            | Screen::Lesson
            | Screen::Calc
            | Screen::Review
            | Screen::Settings => "M2",
            Screen::Table => "M3",
            Screen::HandReview => "M5",
        }
    }
}

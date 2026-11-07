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

/// The nav rail's groups, top to bottom: optional heading key, screens.
/// The lesson reader has no item (it lives under the lessons tree).
pub const NAV_GROUPS: [(Option<&str>, &[Screen]); 4] = [
    (None, &[Screen::Home]),
    (Some("nav-group-learn"), &[Screen::SkillTree]),
    (
        Some("nav-group-practice"),
        &[Screen::Table, Screen::Calc, Screen::Review],
    ),
    (Some("nav-group-analyze"), &[Screen::HandReview]),
];

/// The item pinned to the bottom of the nav rail.
pub const NAV_BOTTOM: Screen = Screen::Settings;

impl Screen {
    /// Every screen (`from_id` looks ids up here). The nav rail's order is
    /// [`NAV_GROUPS`] + [`NAV_BOTTOM`].
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

    /// The screen whose nav key is `nav-<id>` (e.g. `table`, `skill-tree`).
    pub fn from_id(id: &str) -> Option<Screen> {
        Screen::ALL
            .into_iter()
            .find(|s| s.nav_key().split_once('-').map(|(_, rest)| rest) == Some(id))
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every screen but the lesson reader has exactly one nav item.
    #[test]
    fn nav_covers_every_screen_once() {
        let mut nav: Vec<Screen> = NAV_GROUPS
            .iter()
            .flat_map(|(_, s)| s.iter().copied())
            .collect();
        nav.push(NAV_BOTTOM);
        for s in Screen::ALL {
            let n = nav.iter().filter(|&&x| x == s).count();
            assert_eq!(n, usize::from(s != Screen::Lesson), "{s:?}");
        }
        assert_eq!(nav.len(), Screen::ALL.len() - 1);
    }
}

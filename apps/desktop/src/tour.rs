//! Guided tour (#58): on the first visit to a screen, a few coach marks
//! point at its main parts. Steps are data ([`steps`]); each names an
//! anchor that the screen paints with [`anchor`] and its title and body
//! strings (`i18n/*/tour.ftl`). Seen screens are kept in the `tour.seen`
//! setting (`taful_app::keys::TOUR_SEEN`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gpui::{Bounds, Pixels, canvas, div, prelude::*};

use crate::screen::Screen;

/// One coach mark.
pub struct Step {
    /// The element it points at (see [`anchor`]).
    pub anchor: &'static str,
    /// Fluent ids of its title and body.
    pub title: &'static str,
    pub body: &'static str,
}

const fn step(anchor: &'static str, title: &'static str, body: &'static str) -> Step {
    Step {
        anchor,
        title,
        body,
    }
}

/// The tour of a screen, in order.
pub fn steps(screen: Screen) -> &'static [Step] {
    match screen {
        Screen::Home => {
            const {
                &[
                    step("nav", "tour-home-nav-title", "tour-home-nav-body"),
                    step(
                        "home_continue",
                        "tour-home-continue-title",
                        "tour-home-continue-body",
                    ),
                    step(
                        "home_stats",
                        "tour-home-stats-title",
                        "tour-home-stats-body",
                    ),
                ]
            }
        }
        Screen::SkillTree => {
            const {
                &[
                    step(
                        "tree_first",
                        "tour-tree-track-title",
                        "tour-tree-track-body",
                    ),
                    step("tree_next", "tour-tree-next-title", "tour-tree-next-body"),
                ]
            }
        }
        Screen::Lesson => {
            const {
                &[
                    step(
                        "lesson_doc",
                        "tour-lesson-doc-title",
                        "tour-lesson-doc-body",
                    ),
                    step(
                        "lesson_quiz",
                        "tour-lesson-quiz-title",
                        "tour-lesson-quiz-body",
                    ),
                ]
            }
        }
        Screen::Table => {
            const {
                &[
                    step(
                        "table_controls",
                        "tour-table-controls-title",
                        "tour-table-controls-body",
                    ),
                    step(
                        "table_felt",
                        "tour-table-felt-title",
                        "tour-table-felt-body",
                    ),
                    step(
                        "table_actions",
                        "tour-table-actions-title",
                        "tour-table-actions-body",
                    ),
                    step(
                        "table_explanation",
                        "tour-table-explanation-title",
                        "tour-table-explanation-body",
                    ),
                ]
            }
        }
        Screen::Calc => {
            const {
                &[
                    step(
                        "calc_picker",
                        "tour-calc-picker-title",
                        "tour-calc-picker-body",
                    ),
                    step("calc_body", "tour-calc-body-title", "tour-calc-body-body"),
                ]
            }
        }
        Screen::Review => {
            const {
                &[step(
                    "screen_title",
                    "tour-review-title",
                    "tour-review-body",
                )]
            }
        }
        Screen::HandReview => {
            const {
                &[step(
                    "screen_title",
                    "tour-hand-review-title",
                    "tour-hand-review-body",
                )]
            }
        }
        Screen::Settings => {
            const {
                &[step(
                    "screen_title",
                    "tour-settings-title",
                    "tour-settings-body",
                )]
            }
        }
    }
}

/// Whether `screen`'s tour is in the comma-separated `seen` list.
pub fn is_seen(seen: &str, screen: Screen) -> bool {
    seen.split(',').any(|s| s == screen.id())
}

/// `seen` with `screen` added.
pub fn with_seen(seen: &str, screen: Screen) -> String {
    if is_seen(seen, screen) {
        return seen.to_owned();
    }
    let id = screen.id();
    if seen.is_empty() {
        id.to_owned()
    } else {
        format!("{seen},{id}")
    }
}

thread_local! {
    /// Where each anchor was laid out, and on which frame (window coords).
    static ANCHORS: RefCell<HashMap<&'static str, (u64, Bounds<Pixels>)>> =
        RefCell::new(HashMap::new());
    /// The frame being built (see [`begin_frame`]).
    static FRAME: Cell<u64> = const { Cell::new(0) };
    /// The step a missing anchor already asked one more frame for.
    static RETRIED: Cell<Option<(Screen, usize)>> = const { Cell::new(None) };
}

/// Starts a frame; call once at the top of the root view's render, before
/// any [`anchor_bounds`].
pub fn begin_frame() {
    FRAME.with(|f| f.set(f.get() + 1));
}

/// An invisible layer that records its parent's bounds under `id`. The
/// parent must be positioned (`relative()` or `absolute()`).
pub fn anchor(id: &'static str) -> impl IntoElement {
    div().absolute().top_0().left_0().size_full().child(
        canvas(
            move |bounds, window, _| {
                let frame = FRAME.with(Cell::get);
                let old = ANCHORS.with(|a| a.borrow_mut().insert(id, (frame, bounds)));
                // A tour drawn from the old rect must render again (scroll,
                // resize, first paint).
                if old.map(|(_, b)| b) != Some(bounds) {
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .size_full(),
    )
}

/// Bounds of `id` if it was painted on the last frame. An anchor that
/// stopped painting (a button that went away) is not returned.
pub fn anchor_bounds(id: &'static str) -> Option<Bounds<Pixels>> {
    let frame = FRAME.with(Cell::get);
    ANCHORS.with(|a| {
        a.borrow()
            .get(id)
            .filter(|(f, _)| *f + 1 == frame)
            .map(|(_, b)| *b)
    })
}

/// Whether a step whose anchor is missing should ask for one more frame
/// (true once per step: the anchor may paint on this frame; if it does
/// not, the step stays centered without redrawing every frame).
pub fn retry_once(screen: Screen, step: usize) -> bool {
    RETRIED.with(|r| r.replace(Some((screen, step))) != Some((screen, step)))
}

/// Forgets every anchor (called on screen changes so a stale rect from
/// the previous screen is never pointed at).
pub fn clear_anchors() {
    ANCHORS.with(|a| a.borrow_mut().clear());
    RETRIED.with(|r| r.set(None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_has_a_tour() {
        for s in Screen::ALL {
            assert!(!steps(s).is_empty(), "{s:?}");
        }
    }

    #[test]
    fn seen_list_round_trips() {
        let s = with_seen("", Screen::Home);
        let s = with_seen(&s, Screen::Table);
        assert_eq!(with_seen(&s, Screen::Home), s);
        assert!(is_seen(&s, Screen::Home));
        assert!(is_seen(&s, Screen::Table));
        assert!(!is_seen(&s, Screen::Calc));
        assert!(!is_seen("homex", Screen::Home));
    }
}

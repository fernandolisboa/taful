//! The Cockpit visual style (D19): dense and data-first. An icon rail and a
//! status bar with today's numbers frame every screen; home is a dashboard
//! and the lessons are one column per track. Everything else is Classic's,
//! drawn with Cockpit's colors (see `style.rs` for how styles plug in).

pub use crate::classic::*;

mod frame;
mod home;
mod kit;
mod lessons;
mod pal;
mod state;

pub use frame::frame;
pub use home::home;
pub use lessons::lessons;
pub use state::ShellState;

use gpui::{Context, KeyDownEvent, Window};

use crate::screen::Screen;
use crate::screens::table::Mode;
use crate::shell::Shell;

/// Not offered in settings until the table is built.
pub const READY: bool = false;

macro_rules! assets {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../../assets/", $path)).as_slice())),*]
    };
}

/// Embedded files as `(path, bytes)`.
pub const ASSETS: &[(&str, &[u8])] = assets![
    "cockpit/icons/home.svg",
    "cockpit/icons/book.svg",
    "cockpit/icons/table.svg",
    "cockpit/icons/calc.svg",
    "cockpit/icons/review.svg",
    "cockpit/icons/hands.svg",
    "cockpit/icons/settings.svg",
    "cockpit/icons/logo-wings.svg",
    "cockpit/icons/logo-knot.svg",
    "cockpit/icons/lock.svg",
    "cockpit/icons/check.svg",
];

/// Inter and JetBrains Mono (SIL Open Font License, texts beside the files).
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/cockpit/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/cockpit/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/cockpit/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../assets/cockpit/fonts/Inter-Bold.ttf"),
    include_bytes!("../../assets/cockpit/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../assets/cockpit/fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../../assets/cockpit/fonts/JetBrainsMono-SemiBold.ttf"),
    include_bytes!("../../assets/cockpit/fonts/JetBrainsMono-Bold.ttf"),
];

/// The window's base font.
pub const FONT_FAMILY: Option<&str> = Some(kit::SANS);

/// The size the screens were designed at.
pub const WINDOW: Option<(f32, f32)> = Some((1600., 900.));

/// What a shortcut of the home dashboard does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Go {
    Table(Mode),
    Screen(Screen),
    /// Drill the biggest leak.
    TopLeak,
    /// Open the next lesson.
    NextLesson,
}

/// Runs a dashboard shortcut (a click or its key).
pub(crate) fn go(shell: &mut Shell, to: Go, window: &mut Window, cx: &mut Context<Shell>) {
    match to {
        Go::Table(mode) => {
            shell.table.update(cx, |t, cx| t.set_mode(mode, cx));
            shell.select(Screen::Table, window, cx);
        }
        Go::Screen(screen) => shell.select(screen, window, cx),
        Go::TopLeak => {
            let top = shell
                .cockpit
                .dashboard()
                .and_then(|d| d.leaks.first().cloned());
            if let Some(leak) = top {
                shell
                    .table
                    .update(cx, |t, cx| t.start_drill(leak.bucket, cx));
                shell.select(Screen::Table, window, cx);
            }
        }
        Go::NextLesson => {
            let next = shell
                .cockpit
                .lessons()
                .and_then(|l| l.summary.next.as_ref().map(|n| n.id.clone()));
            if let Some(id) = next {
                shell.open_lesson(&id);
                shell.select(Screen::Lesson, window, cx);
            }
        }
    }
}

/// The dashboard's shortcut keys, in the order home lists them.
pub(crate) const HOME_KEYS: [(&str, Go); 6] = [
    ("1", Go::Table(Mode::Spot)),
    ("2", Go::Table(Mode::Postflop)),
    ("3", Go::Table(Mode::Hand)),
    ("4", Go::Screen(Screen::Calc)),
    ("5", Go::Screen(Screen::HandReview)),
    ("v", Go::TopLeak),
];

/// Cockpit's shortcuts: the keys the dashboard and the lessons show.
pub fn key(
    shell: &mut Shell,
    ev: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> bool {
    let ks = &ev.keystroke;
    if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
        return false;
    }
    let to = match (shell.screen, ks.key.as_str()) {
        (Screen::Home | Screen::SkillTree, "enter") => Go::NextLesson,
        (Screen::Home, "r") => Go::Screen(Screen::Review),
        (Screen::Home, k) => match HOME_KEYS.iter().find(|(key, _)| *key == k) {
            Some((_, to)) => *to,
            None => return false,
        },
        _ => return false,
    };
    go(shell, to, window, cx);
    true
}

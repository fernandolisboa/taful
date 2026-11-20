//! The Classic visual style: the look the app shipped with, and the
//! reference set of style hooks (see `style.rs`). The code itself lives
//! where it always did; this module only names it.

use gpui::{AnyElement, Context, KeyDownEvent, Window};

use crate::screens::table::TableScreen;
use crate::shell::Shell;

/// Offered in settings.
pub const READY: bool = true;
/// Embedded files as `(path, bytes)`; paths start with the style's id.
pub const ASSETS: &[(&str, &[u8])] = &[];
/// Embedded fonts.
pub const FONTS: &[&[u8]] = &[];
/// The window's base font (`None`: the system font).
pub const FONT_FAMILY: Option<&str> = None;
/// Preferred size of a new window (`None`: the app's default).
pub const WINDOW: Option<(f32, f32)> = None;

/// What the style keeps on the shell.
#[derive(Default)]
pub struct ShellState;

/// What the style keeps on the table screen.
#[derive(Default)]
pub struct TableState;

/// Chrome and content: everything under the tour and update overlays.
pub fn frame(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    shell.frame_classic(window, cx)
}

/// A key press, before the screens see it; `true` when the style used it.
pub fn key(_: &mut Shell, _: &KeyDownEvent, _: &mut Window, _: &mut Context<Shell>) -> bool {
    false
}

/// The home screen's body.
pub fn home(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    shell.home_classic(cx)
}

/// The lessons tree's body.
pub fn lessons(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    shell.tree_classic(cx)
}

/// The table screen.
pub fn table(
    screen: &mut TableScreen,
    window: &mut Window,
    cx: &mut Context<TableScreen>,
) -> AnyElement {
    screen.render_classic(window, cx)
}

pub use crate::felt::{
    card_classic as felt_card, explanation_classic as explanation, panel_classic as felt_panel,
    table_classic as felt_table,
};
pub use crate::widgets::{
    answer_box_classic as answer_box, button_classic as button, card_classic as card,
    chip_classic as chip, empty_state_classic as empty_state, eyebrow_classic as eyebrow,
    muted_classic as muted, progress_classic as progress, segment_classic as segment,
    segmented_classic as segmented, subhead_classic as subhead, title_classic as title,
    toggle_classic as toggle,
};

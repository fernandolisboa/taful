//! The app's own title bar (D22): the window has no system title bar, so
//! this strip, in the active style's background color, is where the window
//! is dragged and where minimize, maximize and close live.
//!
//! On Windows the strip and the buttons are window control areas: the
//! system answers them as the caption and the caption buttons, so dragging,
//! double-click to maximize, Aero Snap and the snap layouts flyout on the
//! maximize button behave as with the system bar. Elsewhere (Linux, used
//! only for screenshots) the clicks are handled here.

#[cfg(not(windows))]
use gpui::MouseButton;
use gpui::{AnyElement, Pixels, Size, Window, WindowControlArea, div, prelude::*, px, size, svg};

use crate::shell::Shell;

/// The strip's height, in logical pixels: about the system title bar's at
/// 100% scaling, so the content keeps the room it had under that bar.
pub const HEIGHT: f32 = 32.;

/// The width of one caption button (the system's own).
const BUTTON_W: f32 = 46.;

/// The band along the top edge of a restored window left to the system
/// for resizing.
const RESIZE_BAND: f32 = 4.;

/// The size of a button's glyph.
const GLYPH: f32 = 10.;

const MINIMIZE: &str = "caption/minimize.svg";
const MAXIMIZE: &str = "caption/maximize.svg";
const RESTORE: &str = "caption/restore.svg";
const CLOSE: &str = "caption/close.svg";

/// The buttons' glyphs, served next to the styles' files (every style
/// draws this bar).
pub const ASSETS: &[(&str, &[u8])] = &[
    (MINIMIZE, include_bytes!("../assets/caption/minimize.svg")),
    (MAXIMIZE, include_bytes!("../assets/caption/maximize.svg")),
    (RESTORE, include_bytes!("../assets/caption/restore.svg")),
    (CLOSE, include_bytes!("../assets/caption/close.svg")),
];

/// The room the styles draw in: the window below the strip.
pub fn viewport(window: &Window) -> Size<Pixels> {
    let vp = window.viewport_size();
    size(vp.width, (vp.height - px(HEIGHT)).max(px(0.)))
}

/// The strip.
pub fn bar(shell: &Shell, window: &Window) -> AnyElement {
    let p = &shell.palette;
    let maximized = window.is_maximized();
    let glyph = crate::theme::caption_glyph(p.text, p.background);
    // Background and glyph color under the pointer, and the background
    // while pressed.
    let plain = (
        crate::theme::alpha(glyph, 0.1),
        glyph,
        crate::theme::alpha(glyph, 0.16),
    );
    let red = crate::theme::caption_close_hover();
    let on_red = crate::theme::caption_close_glyph();
    let button = |id: &'static str, icon: &'static str, area, (hover, hover_glyph, pressed)| {
        div()
            .id(id)
            .flex_none()
            .w(px(BUTTON_W))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .group(id)
            .hover(move |s| s.bg(hover))
            .active(move |s| s.bg(pressed))
            // In front of the drag area: the button, not the caption,
            // answers here.
            .occlude()
            .window_control_area(area)
            .child(
                svg()
                    .path(icon)
                    .size(px(GLYPH))
                    .text_color(glyph)
                    .group_hover(id, move |s| s.text_color(hover_glyph)),
            )
    };
    let min = button("caption_min", MINIMIZE, WindowControlArea::Min, plain);
    let max = button(
        "caption_max",
        if maximized { RESTORE } else { MAXIMIZE },
        WindowControlArea::Max,
        plain,
    );
    let close = button(
        "caption_close",
        CLOSE,
        WindowControlArea::Close,
        (red, on_red, crate::theme::alpha(red, 0.85)),
    );
    #[cfg(not(windows))]
    let (min, max, close) = (
        min.on_click(|_, window, _| window.minimize_window()),
        max.on_click(|_, window, _| window.zoom_window()),
        close.on_click(|_, window, _| window.remove_window()),
    );
    div()
        .id("caption")
        .relative()
        .flex_none()
        .w_full()
        .h(px(HEIGHT))
        .flex()
        .bg(p.background)
        .child(drag_area(maximized))
        .child(min)
        .child(max)
        .child(close)
        .into_any_element()
}

/// Everything left of the buttons. A restored window keeps a thin band at
/// the top for resizing from the top edge, as the system bar does.
fn drag_area(maximized: bool) -> impl IntoElement {
    let band = if maximized { 0. } else { RESIZE_BAND };
    let area = div()
        .id("caption_drag")
        .flex_1()
        .mt(px(band))
        .h(px(HEIGHT - band))
        .window_control_area(WindowControlArea::Drag);
    #[cfg(not(windows))]
    let area = area.on_mouse_down(MouseButton::Left, |e, window, _| {
        if e.click_count == 2 {
            window.zoom_window();
        } else {
            window.start_window_move();
        }
    });
    area
}

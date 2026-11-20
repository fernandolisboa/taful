//! The Baralho visual style (D19): the app made of playing cards. Felt as
//! the window's background, cream card panels with thick outlines, a top
//! navigation bar, one suit per track and face-down cards for what is
//! still locked.
//!
//! Every measure is in design pixels of the 1600×900 mockup canvas and
//! goes through [`u`], which scales it to the window.

mod chrome;
mod home;
mod kit;
mod lessons;
mod paint;
mod result;
mod table;
mod tabletop;
mod typo;
mod widgets;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{AnyElement, Context, Div, Pixels, Rgba, Styled, Window, px};
use taful_app::preflop::{Card, GameState, Seat};

use crate::Ui;
use crate::felt::{Explain, SeatExtras};
use crate::screens::table::TableScreen;
use crate::shell::Shell;
use crate::theme::Palette;

pub use self::table::TableState;
pub use self::widgets::{
    answer_box, button, card, chip, empty_state, eyebrow, muted, progress, segment, segmented,
    subhead, title, toggle,
};

pub use crate::classic::*;

/// Offered in settings.
pub const READY: bool = true;
/// The suit glyphs, the icons and the logo (the bow tie, in two colors).
pub const ASSETS: &[(&str, &[u8])] = &[
    (
        "baralho/icons/spade.svg",
        include_bytes!("../../assets/baralho/icons/spade.svg"),
    ),
    (
        "baralho/icons/heart.svg",
        include_bytes!("../../assets/baralho/icons/heart.svg"),
    ),
    (
        "baralho/icons/diamond.svg",
        include_bytes!("../../assets/baralho/icons/diamond.svg"),
    ),
    (
        "baralho/icons/club.svg",
        include_bytes!("../../assets/baralho/icons/club.svg"),
    ),
    (
        "baralho/icons/star.svg",
        include_bytes!("../../assets/baralho/icons/star.svg"),
    ),
    (
        "baralho/icons/lock.svg",
        include_bytes!("../../assets/baralho/icons/lock.svg"),
    ),
    (
        "baralho/icons/check.svg",
        include_bytes!("../../assets/baralho/icons/check.svg"),
    ),
    (
        "baralho/icons/caret.svg",
        include_bytes!("../../assets/baralho/icons/caret.svg"),
    ),
    (
        "baralho/icons/logo-wings.svg",
        include_bytes!("../../assets/baralho/icons/logo-wings.svg"),
    ),
    (
        "baralho/icons/logo-knot.svg",
        include_bytes!("../../assets/baralho/icons/logo-knot.svg"),
    ),
];
/// Bricolage Grotesque (display) and Figtree (text), as static instances.
pub const FONTS: &[&[u8]] = typo::FONTS;
pub const FONT_FAMILY: Option<&str> = Some(typo::TEXT_FAMILY);
/// The mockup canvas.
pub const WINDOW: Option<(f32, f32)> = Some((CANVAS_W, CANVAS_H));

/// The design canvas every measure refers to.
pub(crate) const CANVAS_W: f32 = 1600.;
pub(crate) const CANVAS_H: f32 = 900.;
/// The window scales the canvas within these bounds.
const MIN_SCALE: f32 = 0.5;
const MAX_SCALE: f32 = 3.;

/// The table and the panel beside it in the review screens (a wide sheet
/// holds both).
const HOOK_TABLE_W: f32 = 870.;
const HOOK_PANEL_W: f32 = 452.;

/// A color. (The alias keeps signatures clear of the color-literal lint.)
pub(crate) type Tone = Rgba;

thread_local! {
    /// Window pixels per design pixel, set at the top of each frame.
    static SCALE: Cell<f32> = const { Cell::new(1.) };
    /// The style's colors, read once per token set.
    static COLORS: RefCell<Option<(usize, Bx)>> = const { RefCell::new(None) };
}

/// Window pixels per design pixel.
pub(crate) fn scale() -> f32 {
    SCALE.with(Cell::get)
}

/// A design measure in window pixels.
pub(crate) fn u(v: f32) -> Pixels {
    px(v * scale())
}

/// Runs `draw` with every design measure `k` times as large (a table drawn
/// in a box that is not the mockup's).
pub(crate) fn zoomed<R>(k: f32, draw: impl FnOnce() -> R) -> R {
    let outer = scale();
    SCALE.with(|c| c.set(outer * k));
    let out = draw();
    SCALE.with(|c| c.set(outer));
    out
}

/// Fits the canvas to the window: the largest scale that shows all of it.
pub(crate) fn fit(window: &Window) -> f32 {
    let vp = window.viewport_size();
    let s = (f32::from(vp.width) / CANVAS_W)
        .min(f32::from(vp.height) / CANVAS_H)
        .clamp(MIN_SCALE, MAX_SCALE);
    SCALE.with(|c| c.set(s));
    s
}

macro_rules! colors {
    ($($name:ident),* $(,)?) => {
        /// The style's own colors (`content/styles/baralho.json`, `extra`).
        #[derive(Clone, Copy)]
        pub(crate) struct Bx {
            $(pub $name: Rgba,)*
        }

        impl Bx {
            /// Every color name the style file must define.
            #[cfg(test)]
            pub const NAMES: &'static [&'static str] = &[$(stringify!($name)),*];

            fn read(extra: &crate::theme::Extra) -> Self {
                Self { $($name: extra.get(stringify!($name)),)* }
            }
        }
    };
}

colors! {
    felt, felt_hi, felt_lo, deep, well, cream, cream2, cream_hi, cream_hero, ink, ink2, mut_ink,
    on_felt, on_felt_mut, amber, joker, back, back_hi, back_locked, locked_text, table_hi,
    table_mid, table_lo, rail_drop, seat_fold, seat_fold_edge, seat_fold_badge, seat_fold_text,
    seat_fold_mut, pick, paper, shade, logo_wings, logo_knot,
}

/// The style's colors for this `ui`.
pub(crate) fn bx(ui: &Ui) -> Bx {
    let key = Rc::as_ptr(&ui.extra) as usize;
    COLORS.with(|c| {
        let mut c = c.borrow_mut();
        match *c {
            Some((k, colors)) if k == key => colors,
            _ => {
                let colors = Bx::read(&ui.extra);
                *c = Some((key, colors));
                colors
            }
        }
    })
}

/// What the style keeps on the shell.
#[derive(Default)]
pub struct ShellState {
    /// The shell's `ui` with every size scaled to the window, for the
    /// screens that are laid out from theme sizes.
    pub(crate) scaled: Option<(f32, Rc<Ui>, Rc<Ui>)>,
    /// The home's data, read once per visit and locale (it takes a dozen
    /// queries, and a frame is drawn on every hover).
    pub(crate) today: Option<(
        (usize, taful_app::i18n::Locale),
        Rc<taful_app::today::Today>,
    )>,
    /// Which of the top leaks the home's card shows (0 is the biggest).
    pub(crate) leak: usize,
}

/// Chrome and content: everything under the tour and update overlays.
pub fn frame(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    chrome::frame(shell, window, cx)
}

/// The home screen.
pub fn home(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    home::view(shell, cx)
}

/// The lessons tree.
pub fn lessons(shell: &mut Shell, cx: &mut Context<Shell>) -> AnyElement {
    lessons::view(shell, cx)
}

/// The table screen.
pub fn table(
    screen: &mut TableScreen,
    window: &mut Window,
    cx: &mut Context<TableScreen>,
) -> AnyElement {
    table::view(screen, window, cx)
}

/// The table inside the other screens (the review session, the hand
/// review): the mockup's table, fitted to the box those screens give it.
pub fn felt_table(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    s: f32,
    extra: SeatExtras<'_>,
) -> Div {
    let bx = bx(ui);
    let w = HOOK_TABLE_W * s;
    tabletop::fitted(w, w * tabletop::H / tabletop::W, || {
        // A revealed table is a hand that is over: nobody is to act.
        tabletop::table(ui, p, &bx, state, hero, reveal, extra, None, !reveal)
    })
}

/// The side panel next to a table in those screens: wide enough for the
/// rows of [`explanation`].
pub fn felt_panel(ui: &Ui, p: &Palette) -> Div {
    card(ui, p).flex_none().w(u(HOOK_PANEL_W))
}

/// One card, `s` times the size of the cards in a line of text.
pub fn felt_card(ui: &Ui, p: &Palette, c: Option<Card>, s: f32) -> Div {
    kit::playing_card(&bx(ui), p, c, kit::CardSize::scaled(0.5 * s))
}

/// A graded decision: the stamp, then every action with its frequency
/// and EV.
pub fn explanation(ui: &Ui, p: &Palette, d: &Explain<'_>) -> Div {
    result::graded(ui, p, &bx(ui), d, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every color the code reads is in the style file (a missing one
    /// would draw transparent).
    #[test]
    fn style_file_has_every_color() {
        let tokens = crate::style::Style::Baralho
            .tokens()
            .expect("the style file is embedded");
        for name in Bx::NAMES {
            assert!(tokens.extra.contains_key(*name), "{name} is missing");
        }
        for name in tokens.extra.keys() {
            assert!(Bx::NAMES.contains(&name.as_str()), "{name} is unused");
        }
    }
}

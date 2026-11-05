//! The only module that turns theme tokens into GPUI colors (D7). Every other
//! module asks for colors by token name through [`Palette`]; a test rejects
//! color literals anywhere else in `src/`.

use gpui::Rgba;
use taful_app::theme::{Color, Colors};

/// Converts a token color to GPUI's representation.
pub fn rgba(c: Color) -> Rgba {
    Rgba {
        r: f32::from(c.r) / 255.,
        g: f32::from(c.g) / 255.,
        b: f32::from(c.b) / 255.,
        a: f32::from(c.a) / 255.,
    }
}

/// Theme colors converted once, named like the tokens.
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Rgba,
    pub surface: Rgba,
    pub surface_raised: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
}

impl Palette {
    pub fn new(c: &Colors) -> Self {
        Self {
            background: rgba(c.background),
            surface: rgba(c.surface),
            surface_raised: rgba(c.surface_raised),
            border: rgba(c.border),
            text: rgba(c.text),
            text_muted: rgba(c.text_muted),
            accent: rgba(c.accent),
        }
    }
}

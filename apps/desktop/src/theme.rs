//! The only module that turns theme tokens into GPUI colors (D7). Every other
//! module asks for colors by token name through [`Palette`]; a test rejects
//! color literals anywhere else in `src/`.

use gpui::Rgba;
use taful_app::model::Grade;
use taful_app::preflop::Suit;
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

/// A token color for an element whose token is chosen at run time (theme
/// editor swatches and previews).
pub fn token_color(c: Color) -> Rgba {
    rgba(c)
}

/// Hue (degrees), saturation and lightness (0–1) of a token color, for
/// the editor's steppers. Here because the color-literal lint flags any
/// HSL call outside this module.
pub fn hsl_of(c: Color) -> (f64, f64, f64) {
    c.hsl()
}

/// `c` with its alpha multiplied by `a` (0–1): translucent tints, glows and
/// shadows of a token color.
pub fn alpha(c: Rgba, a: f32) -> Rgba {
    Rgba {
        a: c.a * a.clamp(0., 1.),
        ..c
    }
}

/// The close button's background under the pointer in the app's title bar
/// (D22): the system's own, the same in every style and theme.
pub fn caption_close_hover() -> Rgba {
    rgba(Color {
        r: 0xc4,
        g: 0x2b,
        b: 0x1c,
        a: 0xff,
    })
}

/// The close button's glyph on that background.
pub fn caption_close_glyph() -> Rgba {
    rgba(Color {
        r: 0xff,
        g: 0xff,
        b: 0xff,
        a: 0xff,
    })
}

/// The glyphs of the title bar's buttons over `bg` (D22): the theme's text
/// color when it reads on `bg` (WCAG 4.5:1), else white or black, whichever
/// reads (Baralho's text is drawn for its paper cards, not its felt).
pub fn caption_glyph(text: Rgba, bg: Rgba) -> Rgba {
    fn lum(c: Rgba) -> f32 {
        let ch = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }
    let contrast = |a: Rgba, b: Rgba| {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    if contrast(text, bg) >= 4.5 {
        return text;
    }
    let white = rgba(Color {
        r: 0xff,
        g: 0xff,
        b: 0xff,
        a: 0xff,
    });
    let black = rgba(Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0xff,
    });
    if contrast(white, bg) >= contrast(black, bg) {
        white
    } else {
        black
    }
}

/// A visual style's colors that have no theme token (D19), by name, from
/// the `extra` map of `content/styles/<id>.json`.
#[derive(Default)]
pub struct Extra(std::collections::BTreeMap<String, Rgba>);

impl Extra {
    pub fn new(colors: &std::collections::BTreeMap<String, Color>) -> Self {
        Self(colors.iter().map(|(k, c)| (k.clone(), rgba(*c))).collect())
    }

    /// Whether the style file has a color named `name`. A style's tests
    /// check every name the style asks for (CI builds the desktop app in
    /// release mode, where [`Extra::get`] does not panic).
    #[allow(dead_code)] // For the visual styles (D19).
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// The color named `name`. A name the style file lacks is a bug in the
    /// style: debug builds panic, release builds draw nothing rather than
    /// crash.
    #[allow(dead_code)] // For the visual styles (D19).
    pub fn get(&self, name: &str) -> Rgba {
        match self.0.get(name) {
            Some(c) => *c,
            None => {
                debug_assert!(false, "style color {name:?} is not in the style file");
                Rgba::default()
            }
        }
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
    pub accent_muted: Rgba,
    pub on_accent: Rgba,
    pub felt: Rgba,
    pub rail: Rgba,
    pub rail_edge: Rgba,
    pub seat: Rgba,
    pub seat_active: Rgba,
    pub card_face: Rgba,
    pub card_back: Rgba,
    pub suit_spades: Rgba,
    pub suit_hearts: Rgba,
    pub suit_diamonds: Rgba,
    pub suit_clubs: Rgba,
    pub grade_best: Rgba,
    pub grade_correct: Rgba,
    pub grade_inaccuracy: Rgba,
    pub grade_mistake: Rgba,
    pub grade_blunder: Rgba,
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
            accent_muted: rgba(c.accent_muted),
            on_accent: rgba(c.on_accent),
            felt: rgba(c.felt),
            rail: rgba(c.rail),
            rail_edge: rgba(c.rail_edge),
            seat: rgba(c.seat),
            seat_active: rgba(c.seat_active),
            card_face: rgba(c.card_face),
            card_back: rgba(c.card_back),
            suit_spades: rgba(c.suit_spades),
            suit_hearts: rgba(c.suit_hearts),
            suit_diamonds: rgba(c.suit_diamonds),
            suit_clubs: rgba(c.suit_clubs),
            grade_best: rgba(c.grade_best),
            grade_correct: rgba(c.grade_correct),
            grade_inaccuracy: rgba(c.grade_inaccuracy),
            grade_mistake: rgba(c.grade_mistake),
            grade_blunder: rgba(c.grade_blunder),
        }
    }

    /// Color of a grade label (spec §15.4: no red; blunder is violet).
    pub fn grade(&self, g: Grade) -> Rgba {
        match g {
            Grade::Best => self.grade_best,
            Grade::Correct => self.grade_correct,
            Grade::Inaccuracy => self.grade_inaccuracy,
            Grade::Mistake => self.grade_mistake,
            Grade::Blunder => self.grade_blunder,
        }
    }

    /// Color of a suit (four-color deck, spec §15.2).
    pub fn suit(&self, s: Suit) -> Rgba {
        match s {
            Suit::CLUBS => self.suit_clubs,
            Suit::DIAMONDS => self.suit_diamonds,
            Suit::HEARTS => self.suit_hearts,
            _ => self.suit_spades,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(r: u8, g: u8, b: u8) -> Rgba {
        rgba(Color { r, g, b, a: 0xff })
    }

    #[test]
    fn caption_glyph_keeps_readable_text_and_replaces_the_rest() {
        let (dark, light, teal) = (
            c(0x11, 0x18, 0x1c),
            c(0xf0, 0xf0, 0xf0),
            c(0x0e, 0x4a, 0x52),
        );
        assert_eq!(caption_glyph(light, dark), light);
        assert_eq!(caption_glyph(dark, light), dark);
        // Dark text on a dark felt: white.
        assert_eq!(
            caption_glyph(c(0x20, 0x20, 0x20), teal),
            c(0xff, 0xff, 0xff)
        );
    }
}

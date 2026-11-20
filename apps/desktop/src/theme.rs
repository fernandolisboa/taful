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
#[allow(dead_code)] // For the visual styles (D19); Classic has no tints.
pub fn alpha(c: Rgba, a: f32) -> Rgba {
    Rgba {
        a: c.a * a.clamp(0., 1.),
        ..c
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

    /// The color named `name`. A name the style file lacks is a bug in the
    /// style (its tests look every name up): debug builds panic, release
    /// builds draw nothing rather than crash.
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

/// The notification-area icon (D18), drawn from the theme: a felt disc with
/// an accent rim and a card-face pip. `size`×`size` RGBA pixels.
pub fn tray_icon_pixels(p: &Palette, size: u32) -> Vec<u8> {
    let s = size as f32;
    let c = s / 2.;
    let (outer, rim, pip) = (s / 2. - 0.5, s * 0.1, s * 0.16);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            // Coverage at the disc's edge, for a smooth outline.
            let alpha = (outer - d + 0.5).clamp(0., 1.);
            let color = if d <= pip {
                p.card_face
            } else if d >= outer - rim {
                p.accent
            } else {
                p.felt
            };
            let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
            out.extend([
                byte(color.r),
                byte(color.g),
                byte(color.b),
                byte(alpha * color.a),
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_is_a_disc() {
        let theme = taful_app::theme::Theme::builtin_default();
        let p = Palette::new(&theme.colors);
        let px = tray_icon_pixels(&p, 32);
        assert_eq!(px.len(), 32 * 32 * 4);
        // Transparent corner, opaque centre.
        assert_eq!(px[3], 0);
        let centre = (16 * 32 + 16) * 4;
        assert_eq!(px[centre + 3], 255);
    }
}

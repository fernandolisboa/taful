//! The only module that turns theme tokens into GPUI colors (D7). Every other
//! module asks for colors by token name through [`Palette`]; a test rejects
//! color literals anywhere else in `src/`.

use gpui::Rgba;
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

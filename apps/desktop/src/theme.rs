//! Design tokens for the default theme (brief §6, D7): petrol-teal felt,
//! amber/gold accents, cream type, no red, 4-color deck.

use gpui::{Rgba, rgb};

pub struct Theme {
    pub background: Rgba,
    pub rail: Rgba,
    pub rail_edge: Rgba,
    pub felt: Rgba,
    pub felt_line: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub accent_dark: Rgba,
    pub seat: Rgba,
    pub seat_active: Rgba,
    pub panel: Rgba,
    pub panel_header: Rgba,
    pub grid_dot: Rgba,
    pub card_face: Rgba,
    pub card_back: Rgba,
    pub suit_spades: Rgba,
    pub suit_hearts: Rgba,
    pub suit_diamonds: Rgba,
    pub suit_clubs: Rgba,
}

pub fn default_theme() -> Theme {
    Theme {
        background: rgb(0x0d161b),
        rail: rgb(0x1b2a33),
        rail_edge: rgb(0x2c4250),
        felt: rgb(0x0f5257),
        felt_line: rgb(0x1d6d70),
        text: rgb(0xf3ead3),
        text_muted: rgb(0xa9b8b5),
        accent: rgb(0xe0a83a),
        accent_dark: rgb(0x8a6420),
        seat: rgb(0x16242c),
        seat_active: rgb(0x23404a),
        panel: rgb(0x14212a),
        panel_header: rgb(0x203643),
        grid_dot: rgb(0x2a3f4a),
        card_face: rgb(0xf6efdc),
        card_back: rgb(0x1f4f6b),
        suit_spades: rgb(0x1b2630),
        suit_hearts: rgb(0xc4621c),
        suit_diamonds: rgb(0xb08200),
        suit_clubs: rgb(0x1e7a52),
    }
}

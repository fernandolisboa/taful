//! The Salão visual style (D19): Classic's structure with light and
//! material. Serif display type, gold with volume, a lamp-lit felt with a
//! wooden rail.
//!
//! The approved mockup is the reference for every number here
//! (`design-directions/src/salao*.{css,html}`): sizes, spacings and colors
//! are the mockup's, drawn with what GPUI has. What GPUI cannot paint
//! (radial gradients, rotated cards, letter-spacing, optical sizes) comes
//! from files built once from the same mockup rules, in
//! `apps/desktop/assets/salao/` (see its `README.md`).

mod colors;
mod felt;
mod frame;
mod home;
mod kit;
mod lessons;
mod table;
mod widgets;

pub use crate::classic::*;

pub use self::felt::{FELT_SCALE, explanation, felt_card, felt_panel, felt_table};
pub use self::frame::frame;
pub use self::home::{ShellState, home};
pub use self::lessons::lessons;
pub use self::table::{TableState, story, table};
pub use self::widgets::{
    answer_box, button, card, chip, empty_state, eyebrow, muted, progress, segment, segmented,
    subhead, title, toggle,
};

/// Offered in settings.
pub const READY: bool = true;
/// The window's base font: Inter, as the family built for this style.
pub const FONT_FAMILY: Option<&str> = Some(kit::SANS.family);
/// The mockup's size.
pub const WINDOW: Option<(f32, f32)> = Some((1600., 900.));

macro_rules! image {
    ($name:literal) => {
        (
            concat!("salao/img/", $name),
            include_bytes!(concat!("../../assets/salao/img/", $name)) as &[u8],
        )
    };
}

/// Embedded art (`assets/salao/img/`, built by `assets/salao/build/`).
pub const ASSETS: &[(&str, &[u8])] = &[
    image!("avatar_hero.png"),
    image!("back_lg.png"),
    image!("back_sm.png"),
    image!("bg.webp"),
    image!("card_lg.png"),
    image!("card_sm.png"),
    image!("card_xs.png"),
    image!("check.svg"),
    image!("chip_1.png"),
    image!("chip_100.png"),
    image!("chip_25.png"),
    image!("chip_5.png"),
    image!("chip_500.png"),
    image!("dealer.png"),
    image!("hero.webp"),
    image!("lock.svg"),
    image!("logo_knot.svg"),
    image!("logo_wings.svg"),
    image!("medal_done.png"),
    image!("seal_best.png"),
    image!("seal_blunder.png"),
    image!("seal_correct.png"),
    image!("seal_inaccuracy.png"),
    image!("seal_mistake.png"),
    image!("suit_c.svg"),
    image!("suit_d.svg"),
    image!("suit_h.svg"),
    image!("suit_s.svg"),
    image!("table.webp"),
];

macro_rules! font {
    ($name:literal) => {
        include_bytes!(concat!("../../assets/salao/fonts/", $name)) as &[u8]
    };
}

/// Embedded fonts (`assets/salao/fonts/`, built by `assets/salao/build/`):
/// Fraunces and Inter, one family per use (see `kit.rs`).
pub const FONTS: &[&[u8]] = &[
    font!("amount.ttf"),
    font!("bet.ttf"),
    font!("brand.ttf"),
    font!("count.ttf"),
    font!("h1.ttf"),
    font!("hero.ttf"),
    font!("loss.ttf"),
    font!("medal.ttf"),
    font!("ordinal.ttf"),
    font!("overall.ttf"),
    font!("pot.ttf"),
    font!("rank.ttf"),
    font!("sans-400.otf"),
    font!("sans-500.otf"),
    font!("sans-600.otf"),
    font!("sans-700.otf"),
    font!("sans-800.otf"),
    font!("sans-b02.otf"),
    font!("sans-r02.otf"),
    font!("sans-r04.otf"),
    font!("sans-r14.otf"),
    font!("sans-r16.otf"),
    font!("sans-r18.otf"),
    font!("sans-s14.otf"),
    font!("sans-s18.otf"),
    font!("stack.ttf"),
    font!("stat-unit.ttf"),
    font!("stat.ttf"),
    font!("text.ttf"),
    font!("title.ttf"),
    font!("track.ttf"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    /// Every color the style asks for is in `content/styles/salao.json`, and
    /// the file has none the style does not use.
    #[test]
    fn extra_colors_match_the_style_file() {
        let tokens = Style::Salao.tokens().expect("salao.json is embedded");
        let extra = crate::theme::Extra::new(&tokens.extra);
        for name in colors::NAMES {
            assert!(extra.contains(name), "{name} is not in salao.json");
        }
        for name in tokens.extra.keys() {
            assert!(
                colors::NAMES.contains(&name.as_str()),
                "{name} is in salao.json but unused"
            );
        }
    }

    /// Every image the style draws is embedded.
    #[test]
    fn drawn_images_are_embedded() {
        for path in kit::IMAGES {
            assert!(
                ASSETS.iter().any(|(p, _)| p == path),
                "{path} is not in ASSETS"
            );
        }
        for (path, _) in ASSETS {
            assert!(kit::IMAGES.contains(path), "{path} is embedded but unused");
        }
    }

    /// Every font family the style names is one of the embedded fonts'.
    #[test]
    fn font_files_cover_the_faces() {
        assert_eq!(FONTS.len(), kit::FACES.len() + 4);
        for face in kit::FACES {
            let name = face.family.as_bytes();
            // The family name is stored as UTF-16BE in the font's name table.
            let wide: Vec<u8> = name.iter().flat_map(|b| [0, *b]).collect();
            assert!(
                FONTS
                    .iter()
                    .any(|f| f.windows(wide.len()).any(|w| w == wide.as_slice())),
                "no embedded font is named {}",
                face.family
            );
        }
    }
}

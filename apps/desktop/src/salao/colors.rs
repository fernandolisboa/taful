//! The Salão colors the theme schema has no token for, read once from the
//! `extra` map of `content/styles/salao.json` (a style's colors are fixed).

use std::sync::OnceLock;

use gpui::Rgba;

use crate::style::Style;
use crate::theme::{Extra, Palette, alpha};

macro_rules! extras {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// The extra colors, named as in the style file.
        #[derive(Clone, Copy)]
        pub struct Colors {
            $($(#[$doc])* pub $name: Rgba,)*
            /// Hairlines: the text color at 8% and 14%.
            pub line: Rgba,
            pub line2: Rgba,
        }

        /// Every name [`Colors`] reads (checked against the file by a test).
        #[cfg(test)]
        pub const NAMES: &[&str] = &[$(stringify!($name)),*];

        impl Colors {
            fn new(extra: &Extra, p: &Palette) -> Self {
                Self {
                    $($name: extra.get(stringify!($name)),)*
                    line: alpha(p.text, 0.08),
                    line2: alpha(p.text, 0.14),
                }
            }
        }
    };
}

extras!(
    side_top,
    side_bottom,
    /// Panels: `surf2` at the top, `surf` at the bottom.
    surf,
    surf2,
    /// Text one step below muted.
    dim,
    /// Bright gold: the wordmark, active labels.
    gold2,
    /// The gold button's top and bottom.
    gold_hi,
    gold_lo,
    ghost_top,
    ghost_bottom,
    pill_top,
    pill_bottom,
    /// Sunk tracks (the segmented control).
    well,
    seat_top,
    seat_bottom,
    seat_fold_top,
    seat_fold_bottom,
    seat_fold_muted,
    avatar_top,
    avatar_bottom,
    avatar_fold,
    avatar_fold_text,
    /// Behind a bet's amount, on the felt.
    bet_bg,
    medal_bg,
    /// The ring that separates a medallion from the timeline's line.
    ring_bg,
    locked_top,
    locked_bottom,
    locked_title,
    lesson_locked,
    lesson_locked_muted,
    /// Suit marks inside text, on dark panels.
    suit_light_s,
    suit_light_h,
    suit_light_d,
    suit_light_c,
    logo_wings,
    logo_knot,
    /// Where a frequency bar starts.
    bar_start,
    /// Highlights and shadows.
    white,
    black,
);

/// The style's extra colors.
pub fn colors() -> &'static Colors {
    static COLORS: OnceLock<Colors> = OnceLock::new();
    COLORS.get_or_init(|| match Style::Salao.tokens() {
        Some(t) => Colors::new(&Extra::new(&t.extra), &Palette::new(&t.theme.colors)),
        // `Style::tokens` has logged why; nothing is drawn in these colors.
        None => Colors::new(
            &Extra::default(),
            &Palette::new(&taful_app::theme::Theme::builtin_default().colors),
        ),
    })
}

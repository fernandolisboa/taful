//! Design tokens (spec §15.4, D7).
//!
//! A theme is a JSON token file under `content/themes/`. No widget hard-codes
//! a color: the desktop app converts these tokens to GPUI colors in a single
//! module, and a test there rejects color literals anywhere else.
//!
//! Tests here enforce the default theme's rules on every theme file: WCAG AA
//! contrast for text tokens over their backgrounds, and no red anywhere.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer};

/// Version of the token schema this build reads. A bump ships a migration
/// (missing tokens get defaults), like the layout schema.
pub const SCHEMA_VERSION: u32 = 1;

/// The built-in theme, embedded in the binary.
pub const DEFAULT_THEME_JSON: &str = include_str!("../../../content/themes/default.json");

/// An sRGB color with alpha, written `#rrggbb` or `#rrggbbaa` in token files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red, 0–255.
    pub r: u8,
    /// Green, 0–255.
    pub g: u8,
    /// Blue, 0–255.
    pub b: u8,
    /// Alpha, 0–255 (255 = opaque).
    pub a: u8,
}

impl Color {
    /// Relative luminance per WCAG 2.x, ignoring alpha.
    pub fn luminance(self) -> f64 {
        fn channel(c: u8) -> f64 {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// This color alpha-composited over an opaque `bg`.
    pub fn over(self, bg: Color) -> Color {
        let a = u32::from(self.a);
        let mix = |f: u8, b: u8| ((u32::from(f) * a + u32::from(b) * (255 - a) + 127) / 255) as u8;
        Color {
            r: mix(self.r, bg.r),
            g: mix(self.g, bg.g),
            b: mix(self.b, bg.b),
            a: 255,
        }
    }

    /// WCAG contrast ratio of this color drawn over `bg`, 1.0–21.0. A
    /// translucent foreground is composited over `bg` first; `bg` is treated
    /// as opaque.
    pub fn contrast(self, bg: Color) -> f64 {
        let (a, b) = (self.over(bg).luminance(), bg.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Hue in degrees (0–360), saturation and lightness (0–1), HSL model.
    pub fn hsl(self) -> (f64, f64, f64) {
        let [r, g, b] = [self.r, self.g, self.b].map(|c| f64::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        if d == 0.0 {
            return (0.0, 0.0, l);
        }
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        (h, s, l)
    }

    /// Whether a person would call this color red: a saturated, mid-lightness
    /// hue within 20° of pure red. Burnt orange (≈ 25°) and violet pass.
    pub fn is_red(self) -> bool {
        let (h, s, l) = self.hsl();
        s >= 0.35 && (0.15..=0.85).contains(&l) && !(20.0..340.0).contains(&h)
    }
}

/// A token string that is not `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError(String);

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid color {:?}: expected #rrggbb or #rrggbbaa",
            self.0
        )
    }
}

impl std::error::Error for ParseColorError {}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseColorError(s.to_owned());
        let hex = s.strip_prefix('#').ok_or_else(err)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        Ok(Color {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 255 },
        })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Declares a token struct plus an `entries()` listing, so tests can walk
/// every token without repeating the field list.
macro_rules! tokens {
    ($(#[$meta:meta])* $name:ident: $ty:ty { $($(#[$fmeta:meta])* $field:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
        }

        impl $name {
            /// Every token as `(name, value)`, in declaration order.
            pub fn entries(&self) -> Vec<(&'static str, $ty)> {
                vec![$((stringify!($field), self.$field)),*]
            }
        }
    };
}

tokens! {
    /// Color tokens.
    Colors: Color {
        /// Window background.
        background,
        /// Panels and the navigation rail.
        surface,
        /// Raised elements on a surface: headers, selected items, buttons.
        surface_raised,
        /// Hairlines and outlines.
        border,
        /// Primary text.
        text,
        /// Secondary text.
        text_muted,
        /// Brand accent (amber/gold): primary buttons, selection marks.
        accent,
        /// Darker accent for outlines and pressed states.
        accent_muted,
        /// Text and icons drawn on `accent`.
        on_accent,
        /// Table felt.
        felt,
        /// Felt inner line.
        felt_line,
        /// Table rail.
        rail,
        /// Rail outer edge.
        rail_edge,
        /// Seat plate.
        seat,
        /// Seat plate of the player to act.
        seat_active,
        /// Layout edit-mode grid dots.
        grid_dot,
        /// Card face.
        card_face,
        /// Card back.
        card_back,
        /// Spades (4-color deck).
        suit_spades,
        /// Hearts: burnt orange, never red.
        suit_hearts,
        /// Diamonds: gold.
        suit_diamonds,
        /// Clubs.
        suit_clubs,
        /// Chip, smallest denomination.
        chip_1,
        /// Chip.
        chip_5,
        /// Chip.
        chip_25,
        /// Chip.
        chip_100,
        /// Chip, largest denomination.
        chip_500,
    }
}

tokens! {
    /// Spacing scale, in logical pixels.
    Spacing: f32 { xs, sm, md, lg, xl, xxl }
}

tokens! {
    /// Corner radii, in logical pixels.
    Radii: f32 { sm, md, lg, pill }
}

tokens! {
    /// Text sizes, in logical pixels.
    TextSize: f32 { sm, md, lg, xl }
}

/// A complete theme.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Token schema version; must equal [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Theme id (file stem).
    pub name: String,
    /// Color tokens.
    pub colors: Colors,
    /// Spacing scale.
    pub spacing: Spacing,
    /// Corner radii.
    pub radii: Radii,
    /// Text sizes.
    pub text_size: TextSize,
}

/// A theme file that cannot be used.
#[derive(Debug)]
pub enum ThemeError {
    /// Not valid JSON, or a token is missing, unknown or malformed.
    Parse(serde_json::Error),
    /// Written for a schema version this build cannot read.
    Schema(u32),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Parse(e) => write!(f, "invalid theme: {e}"),
            ThemeError::Schema(v) => {
                write!(
                    f,
                    "theme schema {v} is not supported (expected {SCHEMA_VERSION})"
                )
            }
        }
    }
}

impl std::error::Error for ThemeError {}

/// Minimum WCAG contrast for a text token over a background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    /// Body text: AA requires 4.5:1.
    Normal,
    /// Large text (≥ 18.66 px bold or ≥ 24 px), e.g. card ranks: AA requires 3:1.
    Large,
}

impl TextRole {
    /// The AA threshold for this role.
    pub fn min_contrast(self) -> f64 {
        match self {
            TextRole::Normal => 4.5,
            TextRole::Large => 3.0,
        }
    }
}

/// A text token drawn over a background token, checked for contrast.
#[derive(Debug, Clone, Copy)]
pub struct TextPair {
    /// `"text_token on background_token"`.
    pub name: &'static str,
    /// Foreground.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// Which AA threshold applies.
    pub role: TextRole,
}

impl Theme {
    /// Parses a theme file.
    pub fn from_json(json: &str) -> Result<Self, ThemeError> {
        // Read the version first: a newer schema may add tokens that the
        // strict parse below would reject as unknown fields.
        #[derive(Deserialize)]
        struct Probe {
            schema: u32,
        }
        let probe: Probe = serde_json::from_str(json).map_err(ThemeError::Parse)?;
        if probe.schema != SCHEMA_VERSION {
            return Err(ThemeError::Schema(probe.schema));
        }
        serde_json::from_str(json).map_err(ThemeError::Parse)
    }

    /// The built-in default theme.
    pub fn builtin_default() -> Self {
        Self::from_json(DEFAULT_THEME_JSON).expect("built-in theme is valid (tested)")
    }

    /// Every text-over-background combination the UI draws. A widget that
    /// draws text on a new background adds its pair here.
    pub fn text_pairs(&self) -> Vec<TextPair> {
        let c = &self.colors;
        let pair = |name, fg, bg, role| TextPair { name, fg, bg, role };
        use TextRole::{Large, Normal};
        vec![
            pair("text on background", c.text, c.background, Normal),
            pair("text on surface", c.text, c.surface, Normal),
            pair("text on surface_raised", c.text, c.surface_raised, Normal),
            pair(
                "text_muted on background",
                c.text_muted,
                c.background,
                Normal,
            ),
            pair("text_muted on surface", c.text_muted, c.surface, Normal),
            pair(
                "text_muted on surface_raised",
                c.text_muted,
                c.surface_raised,
                Normal,
            ),
            pair("accent on surface", c.accent, c.surface, Normal),
            pair("on_accent on accent", c.on_accent, c.accent, Normal),
            pair("text on felt", c.text, c.felt, Normal),
            pair("text on seat", c.text, c.seat, Normal),
            pair("text on seat_active", c.text, c.seat_active, Normal),
            pair("text_muted on seat", c.text_muted, c.seat, Normal),
            pair(
                "suit_spades on card_face",
                c.suit_spades,
                c.card_face,
                Large,
            ),
            pair(
                "suit_hearts on card_face",
                c.suit_hearts,
                c.card_face,
                Large,
            ),
            pair(
                "suit_diamonds on card_face",
                c.suit_diamonds,
                c.card_face,
                Large,
            ),
            pair("suit_clubs on card_face", c.suit_clubs, c.card_face, Large),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Every theme file shipped under `content/themes/`.
    fn all_themes() -> Vec<(String, Theme)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/themes");
        let mut themes = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "json") {
                let json = std::fs::read_to_string(&path).unwrap();
                let theme =
                    Theme::from_json(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                assert_eq!(theme.name, stem, "{}: name must match file", path.display());
                themes.push((stem, theme));
            }
        }
        assert!(!themes.is_empty());
        themes
    }

    #[test]
    fn builtin_default_parses() {
        let t = Theme::builtin_default();
        assert_eq!(t.name, "default");
        assert_eq!(t.schema, SCHEMA_VERSION);
    }

    /// Spec §15.4: WCAG AA for text tokens over their backgrounds.
    #[test]
    fn text_tokens_meet_wcag_aa() {
        for (name, theme) in all_themes() {
            let failures: Vec<String> = theme
                .text_pairs()
                .into_iter()
                .filter(|p| p.fg.contrast(p.bg) < p.role.min_contrast())
                .map(|p| {
                    format!(
                        "{}: {:.2} < {} ({} on {})",
                        p.name,
                        p.fg.contrast(p.bg),
                        p.role.min_contrast(),
                        p.fg,
                        p.bg
                    )
                })
                .collect();
            assert!(
                failures.is_empty(),
                "theme {name}:\n{}",
                failures.join("\n")
            );
        }
    }

    /// D7 / spec §15.4: no red anywhere.
    #[test]
    fn no_red_tokens() {
        for (name, theme) in all_themes() {
            let red: Vec<String> = theme
                .colors
                .entries()
                .into_iter()
                .filter(|(_, c)| c.is_red())
                .map(|(token, c)| format!("{token} = {c}"))
                .collect();
            assert!(red.is_empty(), "theme {name} has red tokens: {red:?}");
        }
    }

    #[test]
    fn red_detector() {
        let c = |s: &str| s.parse::<Color>().unwrap();
        assert!(c("#ff0000").is_red());
        assert!(c("#c0392b").is_red());
        assert!(c("#dc143c").is_red()); // crimson, hue ≈ 348°
        assert!(!c("#c4621c").is_red()); // burnt orange hearts
        assert!(!c("#b08200").is_red()); // gold
        assert!(!c("#6b4fa0").is_red()); // violet
        assert!(!c("#f3ead3").is_red()); // cream: low saturation
        assert!(!c("#3a0000").is_red()); // too dark to read as red
    }

    #[test]
    fn color_parsing() {
        let c: Color = "#0F5257".parse().unwrap();
        assert_eq!((c.r, c.g, c.b, c.a), (0x0f, 0x52, 0x57, 0xff));
        assert_eq!(c.to_string(), "#0f5257");
        let c: Color = "#00000080".parse().unwrap();
        assert_eq!(c.a, 0x80);
        assert_eq!(c.to_string(), "#00000080");
        for bad in ["0f5257", "#0f525", "#0f52571", "#gg0000", "#ü00000"] {
            assert!(bad.parse::<Color>().is_err(), "{bad}");
        }
    }

    #[test]
    fn contrast_reference_values() {
        let black: Color = "#000000".parse().unwrap();
        let white: Color = "#ffffff".parse().unwrap();
        assert!((black.contrast(white) - 21.0).abs() < 1e-9);
        assert!((white.contrast(white) - 1.0).abs() < 1e-9);
        // Translucent text is composited first: 25% white over black is dark.
        let faint: Color = "#ffffff40".parse().unwrap();
        assert!(faint.contrast(black) < 4.5);
        assert_eq!(faint.over(black).to_string(), "#404040");
    }

    #[test]
    fn unknown_and_missing_tokens_are_rejected() {
        let mut v: serde_json::Value = serde_json::from_str(DEFAULT_THEME_JSON).unwrap();
        v["colors"]["shiny"] = "#ffffff".into();
        assert!(Theme::from_json(&v.to_string()).is_err());

        let mut v: serde_json::Value = serde_json::from_str(DEFAULT_THEME_JSON).unwrap();
        v["colors"].as_object_mut().unwrap().remove("text");
        assert!(Theme::from_json(&v.to_string()).is_err());

        let mut v: serde_json::Value = serde_json::from_str(DEFAULT_THEME_JSON).unwrap();
        v["schema"] = 2.into();
        v["colors"]["focus_ring"] = "#ffffff".into();
        assert!(matches!(
            Theme::from_json(&v.to_string()),
            Err(ThemeError::Schema(2))
        ));
    }
}

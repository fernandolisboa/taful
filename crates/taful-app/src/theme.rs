//! Design tokens (spec §15.4, D7).
//!
//! A theme is a JSON token file under `content/themes/`. No widget hard-codes
//! a color: the desktop app converts these tokens to GPUI colors in a single
//! module, and a test there rejects color literals anywhere else.
//!
//! [`Theme::warnings`] runs the theme rules: WCAG AA contrast for text and
//! grade tokens over their backgrounds, no red anywhere, and grade colors
//! told apart. Tests fail on any warning in a built-in theme; the in-app
//! editor shows the same warnings for user themes but lets them be saved
//! (D7 amendment "Theme editor validation").

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Version of the token schema this build reads. A bump ships a migration
/// (missing tokens get defaults), like the layout schema.
///
/// - 1: first slice tokens.
/// - 2 (M6): the five grade colors. A schema 1 file gets them from the
///   built-in default theme.
pub const SCHEMA_VERSION: u32 = 2;

/// The built-in default theme, embedded in the binary.
pub const DEFAULT_THEME_JSON: &str = include_str!("../../../content/themes/default.json");

/// Id of the default theme.
pub const DEFAULT_THEME: &str = "default";

/// Every built-in theme as `(id, JSON)`, in the order settings lists them.
/// A test checks that this is exactly the set of files in `content/themes/`.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    (DEFAULT_THEME, DEFAULT_THEME_JSON),
    (
        "midnight",
        include_str!("../../../content/themes/midnight.json"),
    ),
    (
        "daylight",
        include_str!("../../../content/themes/daylight.json"),
    ),
];

/// Tokens added in schema 2.
const GRADE_TOKENS: [&str; 5] = [
    "grade_best",
    "grade_correct",
    "grade_inaccuracy",
    "grade_mistake",
    "grade_blunder",
];

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

    /// An opaque-or-`a` color from hue in degrees (wrapped to 0–360),
    /// saturation and lightness (clamped to 0–1), HSL model.
    pub fn from_hsl(h: f64, s: f64, l: f64, a: u8) -> Color {
        let h = h.rem_euclid(360.0);
        let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let byte = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Color {
            r: byte(r),
            g: byte(g),
            b: byte(b),
            a,
        }
    }

    /// Hue a gray takes when it gains saturation: a warm amber, so that
    /// tinting a gray never turns it red (a gray's HSL hue is 0°, pure red).
    pub const GRAY_HUE: f64 = 40.0;

    /// This color with its hue turned by `dh` degrees and its saturation
    /// and lightness moved by `ds` and `dl` (0–1 scale, clamped). Alpha is
    /// kept. The editor's color steppers. A gray has no hue: it starts
    /// from [`Color::GRAY_HUE`].
    pub fn adjust(self, dh: f64, ds: f64, dl: f64) -> Color {
        let (h, s, l) = self.hsl();
        let h = if s < 1e-3 { Self::GRAY_HUE } else { h };
        Color::from_hsl(h + dh, s + ds, l + dl, self.a)
    }

    /// Euclidean distance in RGB, 0–441 (alpha ignored).
    pub fn distance(self, other: Color) -> f64 {
        [(self.r, other.r), (self.g, other.g), (self.b, other.b)]
            .iter()
            .map(|&(x, y)| (f64::from(x) - f64::from(y)).powi(2))
            .sum::<f64>()
            .sqrt()
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

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
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
        #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
        }

        impl $name {
            /// Every token name, in declaration order.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($field)),*];

            /// Every token as `(name, value)`, in declaration order.
            pub fn entries(&self) -> Vec<(&'static str, $ty)> {
                vec![$((stringify!($field), self.$field)),*]
            }

            /// A token by name.
            pub fn get(&self, token: &str) -> Option<$ty> {
                match token {
                    $(stringify!($field) => Some(self.$field),)*
                    _ => None,
                }
            }

            /// Sets a token by name; `false` for an unknown name.
            pub fn set(&mut self, token: &str, value: $ty) -> bool {
                match token {
                    $(stringify!($field) => self.$field = value,)*
                    _ => return false,
                }
                true
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
        /// Grade label: best action (spec §8).
        grade_best,
        /// Grade label: correct.
        grade_correct,
        /// Grade label: inaccuracy.
        grade_inaccuracy,
        /// Grade label: mistake (amber, never red).
        grade_mistake,
        /// Grade label: blunder (violet, never red).
        grade_blunder,
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
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Token schema version; must equal [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Built-in themes: the id (file stem). User themes: the user's name
    /// for it.
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

/// Minimum RGB distance between any two grade colors.
pub const MIN_GRADE_DISTANCE: f64 = 60.0;

/// A theme rule a theme breaks. Built-in themes must have none (tests);
/// user themes may be saved with them (the editor shows them live).
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// A text pair (see [`Theme::text_pairs`]) below its AA threshold.
    Contrast {
        /// `"fg_token on bg_token"`.
        pair: &'static str,
        /// The pair's contrast ratio.
        ratio: f64,
        /// The AA threshold it misses.
        min: f64,
    },
    /// A color token a person would call red (D7: no red anywhere).
    Red {
        /// The token.
        token: &'static str,
    },
    /// Two grade colors closer than [`MIN_GRADE_DISTANCE`] in RGB.
    GradesClose {
        /// Grade id (`best`, …, `blunder`).
        a: &'static str,
        /// Grade id.
        b: &'static str,
        /// Their RGB distance.
        distance: f64,
    },
}

impl fmt::Display for Warning {
    /// English, for test failures and logs; the UI localizes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::Contrast { pair, ratio, min } => {
                write!(f, "{pair}: contrast {ratio:.2} < {min}")
            }
            Warning::Red { token } => write!(f, "{token} is red"),
            Warning::GradesClose { a, b, distance } => {
                write!(f, "grades {a} and {b} are too close ({distance:.0})")
            }
        }
    }
}

impl Theme {
    /// Parses a theme file, migrating older schema versions.
    pub fn from_json(json: &str) -> Result<Self, ThemeError> {
        // Read the version first: a newer schema may add tokens that the
        // strict parse below would reject as unknown fields.
        let mut v: serde_json::Value = serde_json::from_str(json).map_err(ThemeError::Parse)?;
        let schema = v
            .get("schema")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ThemeError::Parse(serde::de::Error::missing_field("schema")))?;
        let schema = u32::try_from(schema).unwrap_or(u32::MAX);
        match schema {
            SCHEMA_VERSION => {}
            1 => migrate_v1(&mut v),
            other => return Err(ThemeError::Schema(other)),
        }
        serde_json::from_value(v).map_err(ThemeError::Parse)
    }

    /// The theme as a token file (current schema).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a theme always serializes")
    }

    /// Every rule this theme breaks, in a stable order: contrast (in
    /// [`Theme::text_pairs`] order), red tokens, close grade pairs.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out: Vec<Warning> = self
            .text_pairs()
            .into_iter()
            .filter_map(|p| {
                let ratio = p.fg.contrast(p.bg);
                let min = p.role.min_contrast();
                (ratio < min).then_some(Warning::Contrast {
                    pair: p.name,
                    ratio,
                    min,
                })
            })
            .collect();
        out.extend(
            self.colors
                .entries()
                .into_iter()
                .filter(|(_, c)| c.is_red())
                .map(|(token, _)| Warning::Red { token }),
        );
        let g = self.grade_colors();
        for i in 0..g.len() {
            for j in i + 1..g.len() {
                let distance = g[i].1.distance(g[j].1);
                if distance < MIN_GRADE_DISTANCE {
                    out.push(Warning::GradesClose {
                        a: g[i].0,
                        b: g[j].0,
                        distance,
                    });
                }
            }
        }
        out
    }

    /// The built-in default theme.
    pub fn builtin_default() -> Self {
        Self::from_json(DEFAULT_THEME_JSON).expect("built-in theme is valid (tested)")
    }

    /// A built-in theme by id.
    pub fn builtin(id: &str) -> Option<Self> {
        BUILTIN_THEMES
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, json)| Self::from_json(json).expect("built-in themes are valid (tested)"))
    }

    /// A built-in theme by id, or the default one for an unknown id.
    pub fn builtin_or_default(id: Option<&str>) -> Self {
        id.and_then(Self::builtin)
            .unwrap_or_else(Self::builtin_default)
    }

    /// The color of a grade, by its id (`best`, `correct`, `inaccuracy`,
    /// `mistake`, `blunder`).
    pub fn grade_colors(&self) -> [(&'static str, Color); 5] {
        let c = &self.colors;
        [
            ("best", c.grade_best),
            ("correct", c.grade_correct),
            ("inaccuracy", c.grade_inaccuracy),
            ("mistake", c.grade_mistake),
            ("blunder", c.grade_blunder),
        ]
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
            pair("accent on background", c.accent, c.background, Normal),
            pair("accent on surface", c.accent, c.surface, Normal),
            pair(
                "accent on surface_raised",
                c.accent,
                c.surface_raised,
                Normal,
            ),
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
            pair("accent on seat", c.accent, c.seat, Normal),
            pair("accent on rail", c.accent, c.rail, Normal),
            pair(
                "grade_best on background",
                c.grade_best,
                c.background,
                Normal,
            ),
            pair("grade_best on surface", c.grade_best, c.surface, Normal),
            pair(
                "grade_best on surface_raised",
                c.grade_best,
                c.surface_raised,
                Normal,
            ),
            pair(
                "grade_correct on background",
                c.grade_correct,
                c.background,
                Normal,
            ),
            pair(
                "grade_correct on surface",
                c.grade_correct,
                c.surface,
                Normal,
            ),
            pair(
                "grade_correct on surface_raised",
                c.grade_correct,
                c.surface_raised,
                Normal,
            ),
            pair(
                "grade_inaccuracy on background",
                c.grade_inaccuracy,
                c.background,
                Normal,
            ),
            pair(
                "grade_inaccuracy on surface",
                c.grade_inaccuracy,
                c.surface,
                Normal,
            ),
            pair(
                "grade_inaccuracy on surface_raised",
                c.grade_inaccuracy,
                c.surface_raised,
                Normal,
            ),
            pair(
                "grade_mistake on background",
                c.grade_mistake,
                c.background,
                Normal,
            ),
            pair(
                "grade_mistake on surface",
                c.grade_mistake,
                c.surface,
                Normal,
            ),
            pair(
                "grade_mistake on surface_raised",
                c.grade_mistake,
                c.surface_raised,
                Normal,
            ),
            pair(
                "grade_blunder on background",
                c.grade_blunder,
                c.background,
                Normal,
            ),
            pair(
                "grade_blunder on surface",
                c.grade_blunder,
                c.surface,
                Normal,
            ),
            pair(
                "grade_blunder on surface_raised",
                c.grade_blunder,
                c.surface_raised,
                Normal,
            ),
        ]
    }
}

/// Schema 1 → 2: the grade colors, taken from the built-in default theme.
fn migrate_v1(v: &mut serde_json::Value) {
    let defaults: serde_json::Value =
        serde_json::from_str(DEFAULT_THEME_JSON).expect("built-in theme is JSON");
    if let Some(colors) = v.get_mut("colors").and_then(|c| c.as_object_mut()) {
        for token in GRADE_TOKENS {
            if !colors.contains_key(token) {
                colors.insert(token.to_owned(), defaults["colors"][token].clone());
            }
        }
    }
    v["schema"] = SCHEMA_VERSION.into();
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

    /// The warnings of every shipped theme that `keep` selects, as text.
    fn shipped_warnings(keep: fn(&Warning) -> bool) -> Vec<String> {
        all_themes()
            .into_iter()
            .flat_map(|(name, theme)| {
                theme
                    .warnings()
                    .into_iter()
                    .filter(keep)
                    .map(move |w| format!("theme {name}: {w}"))
            })
            .collect()
    }

    /// Spec §15.4: WCAG AA for text tokens over their backgrounds. Built-in
    /// themes fail CI on any violation (D7 amendment).
    #[test]
    fn text_tokens_meet_wcag_aa() {
        let failures = shipped_warnings(|w| matches!(w, Warning::Contrast { .. }));
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// D7 / spec §15.4: no red anywhere.
    #[test]
    fn no_red_tokens() {
        let failures = shipped_warnings(|w| matches!(w, Warning::Red { .. }));
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// No built-in theme breaks any rule (covers kinds added later).
    #[test]
    fn builtin_themes_have_no_warnings() {
        let failures = shipped_warnings(|_| true);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn builtin_list_matches_the_theme_files() {
        let mut files: Vec<String> = all_themes().into_iter().map(|(n, _)| n).collect();
        let mut listed: Vec<String> = BUILTIN_THEMES.iter().map(|(n, _)| n.to_string()).collect();
        files.sort();
        listed.sort();
        assert_eq!(files, listed);
        for (id, _) in BUILTIN_THEMES {
            assert_eq!(Theme::builtin(id).unwrap().name, *id);
        }
        assert_eq!(Theme::builtin_or_default(Some("nope")).name, DEFAULT_THEME);
        assert_eq!(Theme::builtin_or_default(None).name, DEFAULT_THEME);
    }

    /// Spec §15.4 (M6): the five grades must be told apart at a glance. Any
    /// two grade colors differ clearly in RGB, and their lightness against
    /// the surface is in the AA range (checked in `text_pairs`).
    #[test]
    fn grade_colors_are_distinct() {
        let failures = shipped_warnings(|w| matches!(w, Warning::GradesClose { .. }));
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The editor's demo: a red blunder color in a copy of `midnight` is
    /// reported as red, and as too close to `mistake` when it is.
    #[test]
    fn warnings_report_each_rule() {
        let mut t = Theme::builtin("midnight").unwrap();
        assert!(t.warnings().is_empty());
        assert!(t.colors.set("grade_blunder", "#e02020".parse().unwrap()));
        let w = t.warnings();
        assert!(w.contains(&Warning::Red {
            token: "grade_blunder"
        }));
        assert!(w.iter().all(
            |w| !matches!(w, Warning::Contrast { pair, .. } if !pair.starts_with("grade_blunder"))
        ));

        let mut t = Theme::builtin_default();
        t.colors.grade_blunder = t.colors.grade_mistake;
        assert!(t.warnings().iter().any(|w| matches!(
            w,
            Warning::GradesClose { a: "mistake", b: "blunder", distance } if *distance == 0.0
        )));

        let mut t = Theme::builtin_default();
        t.colors.text_muted = t.colors.surface;
        let w = t.warnings();
        assert!(w.iter().any(|w| matches!(
            w,
            Warning::Contrast { pair: "text_muted on surface", ratio, min } if *ratio < 1.01 && *min == 4.5
        )));
        assert_eq!(
            w.iter()
                .filter(|w| matches!(w, Warning::Contrast { .. }))
                .count(),
            w.len()
        );
    }

    #[test]
    fn json_round_trip() {
        for (id, _) in BUILTIN_THEMES {
            let t = Theme::builtin(id).unwrap();
            assert_eq!(Theme::from_json(&t.to_json()).unwrap(), t, "{id}");
        }
        let mut t = Theme::builtin_default();
        t.colors.felt = "#11223380".parse().unwrap();
        t.spacing.md = 13.5;
        assert_eq!(Theme::from_json(&t.to_json()).unwrap(), t);
    }

    #[test]
    fn tokens_by_name() {
        let mut t = Theme::builtin_default();
        assert_eq!(Colors::NAMES.len(), t.colors.entries().len());
        assert_eq!(Colors::NAMES[0], "background");
        let c: Color = "#123456".parse().unwrap();
        assert!(t.colors.set("rail", c));
        assert_eq!(t.colors.get("rail"), Some(c));
        assert!(!t.colors.set("shiny", c));
        assert_eq!(t.colors.get("shiny"), None);
        assert!(t.radii.set("pill", 3.0));
        assert_eq!(t.radii.get("pill"), Some(3.0));
        assert_eq!(Spacing::NAMES, &["xs", "sm", "md", "lg", "xl", "xxl"]);
    }

    #[test]
    fn hsl_round_trip_and_adjust() {
        for (_, theme) in all_themes() {
            for (token, c) in theme.colors.entries() {
                let (h, s, l) = c.hsl();
                let back = Color::from_hsl(h, s, l, c.a);
                assert!(back.distance(c) <= 2.0, "{token}: {c} -> {back}");
                assert_eq!(back.a, c.a);
            }
        }
        let c = |s: &str| s.parse::<Color>().unwrap();
        assert_eq!(Color::from_hsl(0.0, 1.0, 0.5, 255), c("#ff0000"));
        assert_eq!(Color::from_hsl(480.0, 1.0, 0.5, 255), c("#00ff00"));
        assert_eq!(Color::from_hsl(240.0, 1.0, 0.5, 255), c("#0000ff"));
        assert_eq!(c("#808080").adjust(90.0, 0.0, 1.0), c("#ffffff"));
        assert_eq!(c("#ff000080").adjust(120.0, 0.0, 0.0), c("#00ff0080"));
        assert_eq!(c("#ffffff").adjust(0.0, 0.0, -2.0), c("#000000"));
        // Saturating a gray tints it amber, never red.
        let tinted = c("#808080").adjust(0.0, 0.5, 0.0);
        assert!(!tinted.is_red(), "{tinted}");
        assert!((tinted.hsl().0 - Color::GRAY_HUE).abs() < 2.0);
    }

    #[test]
    fn schema_1_themes_migrate() {
        let mut v: serde_json::Value = serde_json::from_str(DEFAULT_THEME_JSON).unwrap();
        v["schema"] = 1.into();
        v["name"] = "old".into();
        v["colors"]["text"] = "#ffffff".into();
        for t in GRADE_TOKENS {
            v["colors"].as_object_mut().unwrap().remove(t);
        }
        let t = Theme::from_json(&v.to_string()).unwrap();
        assert_eq!(t.schema, SCHEMA_VERSION);
        assert_eq!(t.colors.text.to_string(), "#ffffff");
        let d = Theme::builtin_default();
        assert_eq!(t.colors.grade_blunder, d.colors.grade_blunder);
        assert_eq!(t.colors.grade_best, d.colors.grade_best);
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
        v["schema"] = (SCHEMA_VERSION + 1).into();
        v["colors"]["focus_ring"] = "#ffffff".into();
        assert!(matches!(
            Theme::from_json(&v.to_string()),
            Err(ThemeError::Schema(v)) if v == SCHEMA_VERSION + 1
        ));
    }
}

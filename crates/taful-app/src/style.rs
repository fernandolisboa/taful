//! Visual styles (D19).
//!
//! A style is a whole look of the desktop app: its chrome (navigation),
//! components, fonts and art. `Salao` is the one a new install opens in
//! (D21). `Classic` is the look the app first shipped with, and the only
//! one that follows the user's theme (D7). Every other style has fixed
//! colors, read from `content/styles/<id>.json`:
//!
//! ```json
//! { "theme": { ...a theme token file... }, "extra": { "gold_hi": "#f2c869" } }
//! ```
//!
//! `theme` holds the standard tokens (so every screen has colors in that
//! style) and `extra` the colors the theme schema has no token for. Tested
//! for every file: no red in either part, four different suit colors and
//! five different grade colors. A style's colors are an approved design, so
//! the contrast and grade-distance warnings of user themes are not applied.

use std::collections::BTreeMap;

use serde_json::json;

use crate::theme::{Color, Theme, ThemeError};
use crate::{App, Result, keys};

/// A visual style of the desktop app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    /// The original look; colors come from the user's theme.
    Classic,
    /// Same structure as Classic, with light and material: serif display
    /// type, gold with volume, a lamp-lit felt with a wooden rail. The
    /// style of a new install (D21).
    #[default]
    Salao,
    /// Dense and data-first: icon rail, dashboard home, range matrix.
    Cockpit,
    /// The interface made of playing cards on a felt background.
    Baralho,
}

impl Style {
    /// Every style, in the order settings lists them: the default first,
    /// Classic last.
    pub const ALL: [Style; 4] = [Style::Salao, Style::Cockpit, Style::Baralho, Style::Classic];

    /// Stable id: the `ui.style` setting, `TAFUL_STYLE`, the file stem
    /// under `content/styles/`.
    pub fn id(self) -> &'static str {
        match self {
            Style::Classic => "classic",
            Style::Salao => "salao",
            Style::Cockpit => "cockpit",
            Style::Baralho => "baralho",
        }
    }

    /// The style with this id.
    pub fn from_id(id: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Fluent id of the style's name.
    pub fn label_key(self) -> &'static str {
        match self {
            Style::Classic => "style-classic",
            Style::Salao => "style-salao",
            Style::Cockpit => "style-cockpit",
            Style::Baralho => "style-baralho",
        }
    }

    /// The style's fixed colors: `None` for Classic (it follows the user's
    /// theme) and for a style whose token file is not there or is broken
    /// (it then falls back to the user's theme as well).
    pub fn tokens(self) -> Option<StyleTokens> {
        let path = format!("styles/{}.json", self.id());
        let (_, json) = crate::content::CONTENT_FILES
            .iter()
            .find(|(p, _)| *p == path)?;
        match StyleTokens::from_json(json) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("taful: {path} is unusable: {e}");
                None
            }
        }
    }
}

/// A style's fixed colors.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleTokens {
    /// The standard tokens.
    pub theme: Theme,
    /// Colors with no token in the theme schema, by name.
    pub extra: BTreeMap<String, Color>,
}

impl StyleTokens {
    /// Parses a style token file.
    pub fn from_json(text: &str) -> std::result::Result<Self, ThemeError> {
        /// The file's top level; a misspelled key is an error, not an
        /// empty `extra`.
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            theme: serde_json::Value,
            #[serde(default)]
            extra: BTreeMap<String, Color>,
        }
        let file: File = serde_json::from_str(text).map_err(ThemeError::Parse)?;
        let theme = Theme::from_json(&file.theme.to_string())?;
        Ok(Self {
            theme,
            extra: file.extra,
        })
    }

    /// Names of the tokens (standard and extra) a person would call red.
    pub fn red_tokens(&self) -> Vec<String> {
        let standard = self.theme.colors.entries();
        standard
            .iter()
            .map(|(name, c)| (*name, *c))
            .chain(self.extra.iter().map(|(name, c)| (name.as_str(), *c)))
            .filter(|(_, c)| c.is_red())
            .map(|(name, _)| name.to_owned())
            .collect()
    }
}

impl App {
    /// The visual style: the one the user picked, the default otherwise.
    ///
    /// A user who never picked a style but did pick a theme stays on
    /// Classic, the one style that shows it (themes can only be picked
    /// there, so this is someone from before the default changed, D21).
    pub fn style(&self) -> Style {
        match self
            .string_setting(keys::UI_STYLE)
            .and_then(|id| Style::from_id(&id))
        {
            Some(style) => style,
            None if self.string_setting(keys::THEME).is_some() => Style::Classic,
            None => Style::default(),
        }
    }

    /// Stores the visual style.
    pub fn set_style(&self, style: Style) -> Result<()> {
        Ok(self.store.set_setting(keys::UI_STYLE, &json!(style.id()))?)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn ids_round_trip() {
        for s in Style::ALL {
            assert_eq!(Style::from_id(s.id()), Some(s));
            assert_eq!(s.label_key(), format!("style-{}", s.id()));
        }
        assert_eq!(Style::from_id("nope"), None);
        assert_eq!(Style::default(), Style::Salao);
        assert_eq!(Style::ALL.first(), Some(&Style::default()));
        assert_eq!(Style::ALL.last(), Some(&Style::Classic));
    }

    /// Every file under `content/styles/` belongs to a style, parses, has
    /// no red (D7), four suit colors and five grade colors; Classic has no
    /// file.
    #[test]
    fn style_files_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/styles");
        let mut seen = 0;
        // No style has a file yet: the directory may not exist.
        for entry in std::fs::read_dir(dir).into_iter().flatten() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let style = Style::from_id(&stem).unwrap_or_else(|| panic!("{stem}: not a style id"));
            assert_ne!(style, Style::Classic, "classic follows the user's theme");
            let text = std::fs::read_to_string(&path).unwrap();
            let tokens = StyleTokens::from_json(&text).unwrap_or_else(|e| panic!("{stem}: {e}"));
            assert_eq!(tokens.red_tokens(), Vec::<String>::new(), "{stem}");
            let c = &tokens.theme.colors;
            let suits = [c.suit_spades, c.suit_hearts, c.suit_diamonds, c.suit_clubs];
            let grades = tokens.theme.grade_colors().map(|(_, color)| color);
            for set in [&suits[..], &grades[..]] {
                for (i, a) in set.iter().enumerate() {
                    assert!(!set[i + 1..].contains(a), "{stem}: a color is used twice");
                }
            }
            assert_eq!(style.tokens(), Some(tokens), "{stem}: not embedded");
            seen += 1;
        }
        assert_eq!(Style::Classic.tokens(), None);
        let with_file = Style::ALL.iter().filter(|s| s.tokens().is_some()).count();
        assert_eq!(with_file, seen);
    }

    #[test]
    fn tokens_parse_and_flag_red() {
        let theme = Theme::builtin_default().to_json();
        let text = format!(
            r##"{{"theme": {theme}, "extra": {{"glow": "#f2c869", "alarm": "#e02020"}}}}"##
        );
        let t = StyleTokens::from_json(&text).unwrap();
        assert_eq!(t.theme, Theme::builtin_default());
        assert_eq!(t.extra.len(), 2);
        assert_eq!(t.red_tokens(), vec!["alarm".to_owned()]);
        // `extra` is optional; `theme` is not.
        let t = StyleTokens::from_json(&format!(r#"{{"theme": {theme}}}"#)).unwrap();
        assert!(t.extra.is_empty());
        assert!(StyleTokens::from_json(r#"{"extra": {}}"#).is_err());
        // A misspelled key is an error, not an empty `extra`.
        assert!(
            StyleTokens::from_json(&format!(r#"{{"theme": {theme}, "extras": {{}}}}"#)).is_err()
        );
    }

    #[test]
    fn style_setting_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path()).unwrap();
        assert_eq!(app.style(), Style::Salao);
        app.set_style(Style::Cockpit).unwrap();
        assert_eq!(app.style(), Style::Cockpit);
        app.set_style(Style::Classic).unwrap();
        assert_eq!(app.style(), Style::Classic);
        app.set_string_setting(keys::UI_STYLE, "gone").unwrap();
        assert_eq!(app.style(), Style::Salao);
    }

    /// The default changed from Classic to Salão (D21): someone who picked
    /// a theme and never a style keeps the style that shows the theme.
    #[test]
    fn a_picked_theme_keeps_classic() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path()).unwrap();
        app.set_theme("midnight").unwrap();
        assert_eq!(app.style(), Style::Classic);
        app.set_style(Style::Baralho).unwrap();
        assert_eq!(app.style(), Style::Baralho);
    }
}

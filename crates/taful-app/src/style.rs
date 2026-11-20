//! Visual styles (D19).
//!
//! A style is a whole look of the desktop app: its chrome (navigation),
//! components, fonts and art. `Classic` is the look the app shipped with,
//! and the only one that follows the user's theme (D7). Every other style
//! has fixed colors, read from `content/styles/<id>.json`:
//!
//! ```json
//! { "theme": { ...a theme token file... }, "extra": { "gold_hi": "#f2c869" } }
//! ```
//!
//! `theme` holds the standard tokens (so every screen has colors in that
//! style) and `extra` the colors the theme schema has no token for. The
//! house rule "no red" holds for both (tested).

use std::collections::BTreeMap;

use serde_json::json;

use crate::theme::{Color, Theme, ThemeError};
use crate::{App, Result, keys};

/// A visual style of the desktop app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    /// The original look; colors come from the user's theme.
    #[default]
    Classic,
    /// Same structure as Classic, with light and material: serif display
    /// type, gold with volume, a lamp-lit felt with a wooden rail.
    Salao,
    /// Dense and data-first: icon rail, dashboard home, range matrix.
    Cockpit,
    /// The interface made of playing cards on a felt background.
    Baralho,
}

impl Style {
    /// Every style, in the order settings lists them.
    pub const ALL: [Style; 4] = [Style::Classic, Style::Salao, Style::Cockpit, Style::Baralho];

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
        let mut v: serde_json::Value = serde_json::from_str(text).map_err(ThemeError::Parse)?;
        let theme = v
            .get("theme")
            .ok_or_else(|| ThemeError::Parse(serde::de::Error::missing_field("theme")))?;
        let theme = Theme::from_json(&theme.to_string())?;
        let extra = match v.get_mut("extra") {
            Some(e) => serde_json::from_value(e.take()).map_err(ThemeError::Parse)?,
            None => BTreeMap::new(),
        };
        Ok(Self { theme, extra })
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
    /// The visual style (Classic unless the user picked another).
    pub fn style(&self) -> Style {
        self.string_setting(keys::UI_STYLE)
            .and_then(|id| Style::from_id(&id))
            .unwrap_or_default()
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
        assert_eq!(Style::default(), Style::Classic);
    }

    /// Every file under `content/styles/` belongs to a style, parses, and
    /// has no red (D7); Classic has no file.
    #[test]
    fn style_files_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/styles");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).into_iter().flatten() {
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
    }

    #[test]
    fn style_setting_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path()).unwrap();
        assert_eq!(app.style(), Style::Classic);
        app.set_style(Style::Salao).unwrap();
        assert_eq!(app.style(), Style::Salao);
        app.set_string_setting(keys::UI_STYLE, "gone").unwrap();
        assert_eq!(app.style(), Style::Classic);
    }
}

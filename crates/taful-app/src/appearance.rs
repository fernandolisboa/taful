//! Theme and layout choices (spec §15.3, §15.4; settings screen, §15.1 #8).
//!
//! Themes are the built-in token files plus the user's themes from the theme
//! editor (`themes` table). Layouts are the built-in presets plus the user's
//! saved copies (`layouts` table). The active choice of each is a setting;
//! an unknown or broken choice falls back to the default.
//!
//! A user theme may break the theme rules: the editor shows
//! [`Theme::warnings`] live but saves anyway (D7 amendment).

use serde_json::json;

use crate::layout::{BUILTIN_LAYOUTS, DEFAULT_LAYOUT, Layout};
use crate::theme::{BUILTIN_THEMES, DEFAULT_THEME, SCHEMA_VERSION, Theme};
use crate::{App, Error, Result, keys};

/// Prefix of a built-in layout's key.
const BUILTIN: &str = "builtin:";
/// Prefix of a saved layout's key.
const USER: &str = "user:";

/// One entry of the layout picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutChoice {
    /// `builtin:<preset id>` or `user:<row id>`: what [`App::set_layout`] takes.
    pub key: String,
    /// Preset id (localize with `layout-<id>`) or the user's name.
    pub name: String,
    pub builtin: bool,
}

/// Longest user theme name, in characters.
pub const THEME_NAME_MAX: usize = 40;

/// One entry of the theme picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeChoice {
    /// The built-in id, or `user:<row id>`: what [`App::set_theme`] takes.
    pub key: String,
    /// Built-in id (localize with `theme-<id>`) or the user's name.
    pub name: String,
    pub builtin: bool,
}

/// Size token groups the editor shows, as `(group id, token names)`. The
/// group id is part of the label key ([`size_label_key`]).
pub const SIZE_GROUPS: [(&str, &[&str]); 3] = [
    ("spacing", crate::theme::Spacing::NAMES),
    ("radii", crate::theme::Radii::NAMES),
    ("text", crate::theme::TextSize::NAMES),
];

/// Fluent id of a color token's label in the theme editor.
pub fn color_label_key(token: &str) -> String {
    format!("theme-color-{}", token.replace('_', "-"))
}

/// Fluent id of a size token's label (`group` from [`SIZE_GROUPS`]).
pub fn size_label_key(group: &str, token: &str) -> String {
    format!("theme-size-{group}-{token}")
}

/// Whether `key` names a user theme.
pub fn is_user_theme(key: &str) -> bool {
    key.starts_with(USER)
}

/// A trimmed, non-empty name of at most [`THEME_NAME_MAX`] characters.
fn theme_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("theme name is empty".into()));
    }
    Ok(name.chars().take(THEME_NAME_MAX).collect())
}

/// A fixed flop the theme editor draws so every suit, the felt, the seats
/// and a pot are on screen while tokens change: folded to the button, who
/// raises with A♠K♥, the big blind calls, flop Q♦ J♣ 2♥. Returns the state
/// and the hero's seat.
pub fn theme_preview_state() -> (crate::preflop::GameState, crate::preflop::Seat) {
    use taful_core::action::Action;
    use taful_core::config::GameConfig;
    use taful_core::engine::Phase;
    use taful_core::position::Position;

    let config = GameConfig::cash(6, 100);
    let start = crate::preflop::GameState::new(config.clone(), 0, 7).expect("valid config");
    let hero = (0..config.table_size as usize)
        .find(|&s| start.position(s) == Position::Btn)
        .expect("6-max has a button");
    let mut holes = vec![None; config.table_size as usize];
    holes[hero] = Some("AsKh".parse().expect("valid hand"));
    let mut s = crate::preflop::GameState::new_with_holes(config, 0, 7, &holes).expect("valid");
    while let Phase::ToAct(seat) = s.phase() {
        let a = match s.position(seat) {
            Position::Btn => Action::Raise(250),
            Position::Bb => Action::Call,
            _ => Action::Fold,
        };
        s = s.apply(a).expect("legal preview line");
    }
    let flop = taful_core::cards::parse_cards("QdJc2h").expect("valid board");
    (s.deal_board(&flop).expect("flop is due"), hero)
}

/// Key of a built-in preset.
pub fn builtin_layout_key(id: &str) -> String {
    format!("{BUILTIN}{id}")
}

impl App {
    /// Built-in themes first, then the user's (oldest first).
    pub fn theme_choices(&self) -> Result<Vec<ThemeChoice>> {
        let mut out: Vec<ThemeChoice> = BUILTIN_THEMES
            .iter()
            .map(|(id, _)| ThemeChoice {
                key: (*id).to_owned(),
                name: (*id).to_owned(),
                builtin: true,
            })
            .collect();
        for row in self.store.themes()? {
            // A theme this build cannot read (corrupt row, newer schema
            // from an import) is not offered: it could not be selected.
            if Theme::from_json(&row.json).is_err() {
                continue;
            }
            out.push(ThemeChoice {
                key: format!("{USER}{}", row.id),
                name: row.name,
                builtin: false,
            });
        }
        Ok(out)
    }

    /// A theme by key, if it exists and is valid. A user theme carries the
    /// row's name.
    pub fn load_theme(&self, key: &str) -> Option<Theme> {
        let Some(id) = key.strip_prefix(USER) else {
            return Theme::builtin(key);
        };
        let row = self.store.theme(id).ok()??;
        match Theme::from_json(&row.json) {
            Ok(mut t) => {
                t.name = row.name;
                Some(t)
            }
            Err(e) => {
                eprintln!("taful: theme {:?} is unusable: {e}", row.name);
                None
            }
        }
    }

    /// The active theme's key: a built-in id or `user:<id>`; the default
    /// theme if the saved choice is gone or broken.
    pub fn theme_key(&self) -> String {
        self.string_setting(keys::THEME)
            .filter(|k| self.load_theme(k).is_some())
            .unwrap_or_else(|| DEFAULT_THEME.to_owned())
    }

    /// The active theme's tokens.
    pub fn theme(&self) -> Theme {
        self.string_setting(keys::THEME)
            .and_then(|k| self.load_theme(&k))
            .unwrap_or_else(Theme::builtin_default)
    }

    /// Makes a theme active.
    pub fn set_theme(&self, key: &str) -> Result<Theme> {
        let theme = self
            .load_theme(key)
            .ok_or_else(|| Error::NotFound(format!("theme {key}")))?;
        self.store.set_setting(keys::THEME, &json!(key))?;
        Ok(theme)
    }

    /// Copies a theme (built-in or user) into a new user theme named
    /// `name` and makes it active. The theme editor's starting point.
    pub fn duplicate_theme(&self, source: &str, name: &str) -> Result<ThemeChoice> {
        let mut theme = self
            .load_theme(source)
            .ok_or_else(|| Error::NotFound(format!("theme {source}")))?;
        let name = theme_name(name)?;
        theme.name = name.clone();
        let id = self
            .store
            .insert_theme(&name, SCHEMA_VERSION, &theme.to_json())?;
        let key = format!("{USER}{id}");
        self.store.set_setting(keys::THEME, &json!(key))?;
        Ok(ThemeChoice {
            key,
            name,
            builtin: false,
        })
    }

    /// The row of a live user theme.
    fn user_theme_row(&self, key: &str) -> Result<taful_store::ThemeRow> {
        let id = key
            .strip_prefix(USER)
            .ok_or_else(|| Error::Invalid(format!("theme {key} is built in")))?;
        self.store
            .theme(id)?
            .ok_or_else(|| Error::NotFound(format!("theme {key}")))
    }

    fn update_theme_row(&self, key: &str, id: &str, theme: &Theme) -> Result<()> {
        if self
            .store
            .update_theme(id, &theme.name, SCHEMA_VERSION, &theme.to_json())?
        {
            Ok(())
        } else {
            Err(Error::NotFound(format!("theme {key}")))
        }
    }

    /// Stores the editor's tokens for a user theme. Warnings do not block
    /// the save; the theme keeps its stored name. Returns the saved theme.
    pub fn save_theme(&self, key: &str, theme: &Theme) -> Result<Theme> {
        let row = self.user_theme_row(key)?;
        let mut theme = theme.clone();
        theme.name = row.name;
        theme.schema = SCHEMA_VERSION;
        self.update_theme_row(key, &row.id, &theme)?;
        Ok(theme)
    }

    /// Renames a user theme. Returns the stored name (trimmed, capped).
    pub fn rename_theme(&self, key: &str, name: &str) -> Result<String> {
        let row = self.user_theme_row(key)?;
        let name = theme_name(name)?;
        let mut theme =
            Theme::from_json(&row.json).map_err(|e| Error::Invalid(format!("theme {key}: {e}")))?;
        theme.name = name.clone();
        self.update_theme_row(key, &row.id, &theme)?;
        Ok(name)
    }

    /// Deletes a user theme; the default theme becomes active if it was.
    pub fn delete_theme(&self, key: &str) -> Result<()> {
        let row = self.user_theme_row(key)?;
        self.store.delete_theme(&row.id)?;
        if self.string_setting(keys::THEME).as_deref() == Some(key) {
            self.store.set_setting(keys::THEME, &json!(DEFAULT_THEME))?;
        }
        Ok(())
    }

    /// Built-in presets first, then saved copies (oldest first).
    pub fn layout_choices(&self) -> Result<Vec<LayoutChoice>> {
        let mut out: Vec<LayoutChoice> = BUILTIN_LAYOUTS
            .iter()
            .map(|(id, _)| LayoutChoice {
                key: builtin_layout_key(id),
                name: (*id).to_owned(),
                builtin: true,
            })
            .collect();
        for row in self.store.layouts()? {
            out.push(LayoutChoice {
                key: format!("{USER}{}", row.id),
                name: row.name,
                builtin: false,
            });
        }
        Ok(out)
    }

    /// A layout by key, if it exists and is valid.
    fn load_layout(&self, key: &str) -> Option<Layout> {
        if let Some(id) = key.strip_prefix(BUILTIN) {
            return Layout::builtin(id);
        }
        let id = key.strip_prefix(USER)?;
        let row = self
            .store
            .layouts()
            .ok()?
            .into_iter()
            .find(|r| r.id == id)?;
        match Layout::from_json(&row.json) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("taful: saved layout {:?} is unusable: {e}", row.name);
                None
            }
        }
    }

    /// The active layout and its key; the default preset if the saved
    /// choice is gone or broken.
    pub fn active_layout(&self) -> (String, Layout) {
        if let Some(key) = self.string_setting(keys::LAYOUT)
            && let Some(l) = self.load_layout(&key)
        {
            return (key, l);
        }
        (
            builtin_layout_key(DEFAULT_LAYOUT),
            Layout::builtin_default(),
        )
    }

    /// Makes a layout active.
    pub fn set_layout(&self, key: &str) -> Result<Layout> {
        let layout = self
            .load_layout(key)
            .ok_or_else(|| Error::NotFound(format!("layout {key}")))?;
        self.store.set_setting(keys::LAYOUT, &json!(key))?;
        Ok(layout)
    }

    /// Saves a copy of the active layout under `name` and makes it active.
    pub fn save_layout_copy(&self, name: &str) -> Result<LayoutChoice> {
        let (_, mut layout) = self.active_layout();
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::NotFound("layout name".into()));
        }
        layout.name = name.to_owned();
        let id = self
            .store
            .insert_layout(name, layout.schema, &layout.to_json())?;
        let key = format!("{USER}{id}");
        self.store.set_setting(keys::LAYOUT, &json!(key))?;
        Ok(LayoutChoice {
            key,
            name: name.to_owned(),
            builtin: false,
        })
    }

    /// Deletes a saved layout; the default preset becomes active if it was.
    pub fn delete_layout(&self, key: &str) -> Result<()> {
        let id = key
            .strip_prefix(USER)
            .ok_or_else(|| Error::NotFound(format!("saved layout {key}")))?;
        self.store.delete_layout(id)?;
        if self.string_setting(keys::LAYOUT).as_deref() == Some(key) {
            self.reset_layout()?;
        }
        Ok(())
    }

    /// Back to the default preset.
    pub fn reset_layout(&self) -> Result<()> {
        Ok(self
            .store
            .set_setting(keys::LAYOUT, &json!(builtin_layout_key(DEFAULT_LAYOUT)))?)
    }
}

#[cfg(test)]
mod tests {
    use crate::services::tests::app_at;

    use super::*;

    #[test]
    fn themes() {
        let (app, _) = app_at(1);
        assert_eq!(app.theme_key(), "default");
        assert_eq!(app.set_theme("midnight").unwrap().name, "midnight");
        assert_eq!(app.theme().name, "midnight");
        assert!(app.set_theme("neon").is_err());
        assert!(app.set_theme("user:nope").is_err());
        assert_eq!(app.theme_key(), "midnight");
        app.set_string_setting(keys::THEME, "gone").unwrap();
        assert_eq!(app.theme_key(), "default");
        assert_eq!(app.theme().name, "default");
        let choices = app.theme_choices().unwrap();
        assert_eq!(choices.len(), BUILTIN_THEMES.len());
        assert!(choices.iter().all(|c| c.builtin && c.key == c.name));
    }

    /// Issue #23 demo: duplicate `midnight`, set the blunder color to red,
    /// see the warning, save anyway, switch to the new theme.
    #[test]
    fn theme_editor_flow() {
        use crate::theme::Warning;

        let (app, t) = app_at(1);
        app.set_theme("midnight").unwrap();
        let copy = app.duplicate_theme("midnight", "  Night copy  ").unwrap();
        assert_eq!(copy.name, "Night copy");
        assert!(!copy.builtin && is_user_theme(&copy.key));
        assert_eq!(app.theme_key(), copy.key, "the copy becomes active");
        let mut draft = app.theme();
        assert_eq!(draft.name, "Night copy");
        assert_eq!(draft.colors, Theme::builtin("midnight").unwrap().colors);
        assert!(draft.warnings().is_empty());

        assert!(
            draft
                .colors
                .set("grade_blunder", "#e02020".parse().unwrap())
        );
        assert!(draft.warnings().contains(&Warning::Red {
            token: "grade_blunder"
        }));
        let saved = app.save_theme(&copy.key, &draft).unwrap();
        assert_eq!(saved.colors.grade_blunder, draft.colors.grade_blunder);
        assert!(!saved.warnings().is_empty(), "saved with warnings");

        // Switch away and back: the stored tokens come back.
        app.set_theme("default").unwrap();
        assert_eq!(app.theme().name, "default");
        let back = app.set_theme(&copy.key).unwrap();
        assert_eq!(back.colors.grade_blunder.to_string(), "#e02020");
        assert_eq!(app.theme(), back);

        // Rename keeps the tokens; the name is trimmed and capped.
        let long = "x".repeat(THEME_NAME_MAX + 10);
        assert_eq!(
            app.rename_theme(&copy.key, &long).unwrap().len(),
            THEME_NAME_MAX
        );
        assert_eq!(
            app.rename_theme(&copy.key, " Blue night ").unwrap(),
            "Blue night"
        );
        assert!(app.rename_theme(&copy.key, "   ").is_err());
        assert_eq!(app.theme().name, "Blue night");
        assert_eq!(app.theme().colors, back.colors);

        // Duplicate a user theme: same tokens, new key.
        t.store(2, std::sync::atomic::Ordering::SeqCst);
        let dup = app.duplicate_theme(&copy.key, "Blue night 2").unwrap();
        assert_ne!(dup.key, copy.key);
        assert_eq!(app.theme().colors, back.colors);
        let names: Vec<String> = app
            .theme_choices()
            .unwrap()
            .into_iter()
            .filter(|c| !c.builtin)
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["Blue night", "Blue night 2"]);

        // Built-ins cannot be saved over, renamed or deleted.
        assert!(matches!(
            app.save_theme("midnight", &draft),
            Err(Error::Invalid(_))
        ));
        assert!(app.rename_theme("default", "x").is_err());
        assert!(app.delete_theme("daylight").is_err());
        assert!(app.duplicate_theme("neon", "x").is_err());
        assert!(app.duplicate_theme("midnight", " ").is_err());

        // Deleting the active theme falls back to the default.
        app.delete_theme(&dup.key).unwrap();
        assert_eq!(app.theme_key(), "default");
        assert!(app.set_theme(&dup.key).is_err());
        assert!(app.save_theme(&dup.key, &draft).is_err());
        // Deleting an inactive one keeps the choice.
        app.set_theme("midnight").unwrap();
        app.delete_theme(&copy.key).unwrap();
        assert_eq!(app.theme_key(), "midnight");
        assert_eq!(app.theme_choices().unwrap().len(), BUILTIN_THEMES.len());
    }

    #[test]
    fn preview_state_is_a_flop() {
        let (s, hero) = theme_preview_state();
        assert_eq!(s.board().len(), 3);
        assert_eq!(s.players()[hero].hole.to_string(), "AsKh");
        assert!(!s.players()[hero].folded);
        assert!(matches!(s.phase(), taful_core::engine::Phase::ToAct(_)));
    }

    #[test]
    fn broken_user_theme_falls_back() {
        let (app, _) = app_at(1);
        let id = app.store.insert_theme("bad", 2, "{\"schema\":2}").unwrap();
        let key = format!("user:{id}");
        app.set_string_setting(keys::THEME, &key).unwrap();
        assert_eq!(app.theme_key(), "default");
        assert_eq!(app.theme().name, "default");
        assert!(app.set_theme(&key).is_err());
        assert_eq!(
            app.theme_choices().unwrap().len(),
            BUILTIN_THEMES.len(),
            "a broken theme is not offered"
        );
        assert!(
            app.rename_theme(&key, "x").is_err(),
            "no panic on a broken row"
        );
    }

    /// User themes are user data: they survive an export/import round trip.
    #[test]
    fn user_themes_survive_backup_restore() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = crate::App::open(dir.path()).unwrap();
        let copy = app.duplicate_theme("midnight", "Mine").unwrap();
        let mut draft = app.theme();
        draft.colors.felt = "#123456".parse().unwrap();
        app.save_theme(&copy.key, &draft).unwrap();
        let export = dir.path().join("export.sqlite");
        app.export_to(&export).unwrap();

        app.delete_theme(&copy.key).unwrap();
        assert_eq!(app.theme_key(), "default");
        app.import_from(&export).unwrap();
        assert_eq!(app.theme_key(), copy.key);
        let t = app.theme();
        assert_eq!(t.name, "Mine");
        assert_eq!(t.colors.felt.to_string(), "#123456");
    }

    #[test]
    fn layouts_save_load_reset() {
        let (app, t) = app_at(1);
        let (key, l) = app.active_layout();
        assert_eq!(key, "builtin:default");
        assert_eq!(l.name, "default");
        assert_eq!(app.layout_choices().unwrap().len(), BUILTIN_LAYOUTS.len());

        app.set_layout("builtin:wide-panel").unwrap();
        assert_eq!(app.active_layout().1.name, "wide-panel");
        assert!(app.set_layout("builtin:nope").is_err());
        assert!(app.set_layout("user:nope").is_err());

        t.store(2, std::sync::atomic::Ordering::SeqCst);
        let saved = app.save_layout_copy(" Mine ").unwrap();
        assert_eq!(saved.name, "Mine");
        let (key, l) = app.active_layout();
        assert_eq!(key, saved.key);
        assert_eq!(l.name, "Mine");
        assert_eq!(
            l.panels,
            Layout::builtin("wide-panel").unwrap().panels,
            "a copy of the active layout"
        );
        let choices = app.layout_choices().unwrap();
        assert_eq!(choices.last().unwrap(), &saved);
        assert!(app.save_layout_copy("  ").is_err());

        assert!(app.delete_layout("builtin:default").is_err());
        app.delete_layout(&saved.key).unwrap();
        assert_eq!(app.active_layout().0, "builtin:default");
        assert_eq!(app.layout_choices().unwrap().len(), BUILTIN_LAYOUTS.len());

        app.set_layout("builtin:panel-left").unwrap();
        app.reset_layout().unwrap();
        assert_eq!(app.active_layout().1.name, "default");
    }

    /// Settings labels built-ins as `theme-<id>` and `layout-<id>`.
    #[test]
    fn every_builtin_has_a_label() {
        for loc in [crate::i18n::Locale::PtBr, crate::i18n::Locale::EnUs] {
            let i18n = crate::i18n::Localizer::new(loc).unwrap();
            for (id, _) in BUILTIN_THEMES {
                assert!(i18n.has(&format!("theme-{id}")), "{loc:?}: theme-{id}");
            }
            for (id, _) in BUILTIN_LAYOUTS {
                assert!(i18n.has(&format!("layout-{id}")), "{loc:?}: layout-{id}");
            }
        }
    }

    /// The editor labels every token (dynamic Fluent ids).
    #[test]
    fn every_token_has_a_label() {
        for loc in [crate::i18n::Locale::PtBr, crate::i18n::Locale::EnUs] {
            let i18n = crate::i18n::Localizer::new(loc).unwrap();
            for token in crate::theme::Colors::NAMES {
                let key = color_label_key(token);
                assert!(i18n.has(&key), "{loc:?}: {key}");
            }
            for (group, names) in SIZE_GROUPS {
                for token in names {
                    let key = size_label_key(group, token);
                    assert!(i18n.has(&key), "{loc:?}: {key}");
                }
            }
        }
    }

    #[test]
    fn broken_saved_layout_falls_back() {
        let (app, _) = app_at(1);
        let id = app.store.insert_layout("bad", 1, "{\"schema\":1}").unwrap();
        app.set_string_setting(keys::LAYOUT, &format!("user:{id}"))
            .unwrap();
        assert_eq!(app.active_layout().0, "builtin:default");
    }
}

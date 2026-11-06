//! Theme and layout choices (spec §15.3, §15.4; settings screen, §15.1 #8).
//!
//! Themes are the built-in token files. Layouts are the built-in presets
//! plus the user's saved copies (`layouts` table). The active choice of each
//! is a setting; an unknown or broken choice falls back to the default.

use serde_json::json;

use crate::layout::{BUILTIN_LAYOUTS, DEFAULT_LAYOUT, Layout};
use crate::layout_edit::LayoutEditor;
use crate::theme::{BUILTIN_THEMES, DEFAULT_THEME, Theme};
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

/// Key of a built-in preset.
pub fn builtin_layout_key(id: &str) -> String {
    format!("{BUILTIN}{id}")
}

impl App {
    /// The active theme's id (a built-in theme).
    pub fn theme_id(&self) -> String {
        self.string_setting(keys::THEME)
            .filter(|id| BUILTIN_THEMES.iter().any(|(t, _)| t == id))
            .unwrap_or_else(|| DEFAULT_THEME.to_owned())
    }

    /// The active theme's tokens.
    pub fn theme(&self) -> Theme {
        Theme::builtin_or_default(Some(&self.theme_id()))
    }

    /// Picks a built-in theme.
    pub fn set_theme(&self, id: &str) -> Result<Theme> {
        let theme = Theme::builtin(id).ok_or_else(|| Error::NotFound(format!("theme {id}")))?;
        self.store.set_setting(keys::THEME, &json!(id))?;
        Ok(theme)
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
        let (_, layout) = self.active_layout();
        self.insert_layout_copy(layout, name)
    }

    /// Starts editing the active layout (layout edit mode, #22).
    pub fn edit_layout(&self) -> LayoutEditor {
        LayoutEditor::new(self.active_layout().1)
    }

    /// Saves an edited layout as a new user copy named `name` and makes it
    /// active; built-in presets are never written. The draft goes through
    /// the same parse-and-migrate step as a saved copy on load, and is
    /// refused (nothing stored) if it breaks a rule.
    pub fn save_edited_layout(&self, draft: &Layout, name: &str) -> Result<LayoutChoice> {
        let layout = Layout::from_json(&draft.to_json()).map_err(Error::Layout)?;
        self.insert_layout_copy(layout, name)
    }

    fn insert_layout_copy(&self, mut layout: Layout, name: &str) -> Result<LayoutChoice> {
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
        assert_eq!(app.theme_id(), "default");
        assert_eq!(app.set_theme("midnight").unwrap().name, "midnight");
        assert_eq!(app.theme().name, "midnight");
        assert!(app.set_theme("neon").is_err());
        assert_eq!(app.theme_id(), "midnight");
        app.set_string_setting(keys::THEME, "gone").unwrap();
        assert_eq!(app.theme_id(), "default");
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

    #[test]
    fn broken_saved_layout_falls_back() {
        let (app, _) = app_at(1);
        let id = app.store.insert_layout("bad", 1, "{\"schema\":1}").unwrap();
        app.set_string_setting(keys::LAYOUT, &format!("user:{id}"))
            .unwrap();
        assert_eq!(app.active_layout().0, "builtin:default");
    }
}

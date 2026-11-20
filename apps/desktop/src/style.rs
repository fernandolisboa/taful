//! Visual styles (D19): which module draws the app.
//!
//! Each style is a module with the same set of items (hooks): `classic` is
//! the original look and the reference for the set; every other style
//! starts as `pub use crate::classic::*;` and replaces what it draws
//! differently by defining an item with the same name (an explicit item
//! shadows the glob import). Shared code reaches the active style through
//! [`by_style!`], so a style never edits shared code: it lives in its own
//! module, `content/styles/<id>.json`, `i18n/<locale>/<id>.ftl` and
//! `apps/desktop/assets/<id>/`. `docs/development.md` lists what to mind
//! when writing one.
//!
//! The hooks:
//!
//! - Constants: `READY` (offered in settings), `ASSETS` and `FONTS`
//!   (embedded files), `FONT_FAMILY` (the window's base font), `WINDOW`
//!   (preferred size of a new window), `FELT_SCALE` (the size of a table
//!   that stands alone, against Classic's) and `FELT_WIDTH` (that table's
//!   width, so a narrow window can draw it smaller).
//! - State: `ShellState` and `TableState`, one slot each on the shell and
//!   on the table screen, owned by the style.
//! - Views: `frame` (chrome and content, under the tour and update
//!   overlays), `key` (shortcuts), `home`, `lessons`, `table`, `story` (the
//!   table's "how the hand got here").
//! - Felt: `felt_table`, `felt_card`, `explanation`, `felt_panel`.
//! - Widgets: the ones the top of `widgets.rs` dispatches (the lesson text
//!   renderer is shared).

pub use taful_app::style::Style;

/// Evaluates `<style module>::<rest>` for the given style, e.g.
/// `by_style!(ui.style, home(shell, cx))` or `by_style!(style, READY)`.
macro_rules! by_style {
    ($style:expr, $($rest:tt)+) => {
        match $style {
            $crate::style::Style::Classic => $crate::classic::$($rest)+,
            $crate::style::Style::Salao => $crate::salao::$($rest)+,
            $crate::style::Style::Cockpit => $crate::cockpit::$($rest)+,
            $crate::style::Style::Baralho => $crate::baralho::$($rest)+,
        }
    };
}
pub(crate) use by_style;

/// Whether settings offers the style (a style under construction is not).
pub fn ready(style: Style) -> bool {
    by_style!(style, READY)
}

/// The styles settings offers: the default first, Classic last.
pub fn choices() -> Vec<Style> {
    Style::ALL.into_iter().filter(|s| ready(*s)).collect()
}

/// The saved style, unless `TAFUL_STYLE=<id>` (screenshots, development)
/// names another. A saved style that is not ready falls back to the
/// default.
pub fn startup(saved: Style) -> Style {
    match std::env::var("TAFUL_STYLE")
        .ok()
        .and_then(|id| Style::from_id(&id))
    {
        Some(forced) => forced,
        None if ready(saved) => saved,
        None => Style::default(),
    }
}

/// The window's base font family in this style (`None`: the system font).
pub fn font_family(style: Style) -> Option<&'static str> {
    by_style!(style, FONT_FAMILY)
}

/// The size a new window opens at in this style, before it is capped to
/// the display (`None`: the app's default).
pub fn window_size(style: Style) -> Option<(f32, f32)> {
    by_style!(style, WINDOW)
}

/// Every style's embedded files as `(path, bytes)`.
pub fn assets() -> impl Iterator<Item = &'static (&'static str, &'static [u8])> {
    Style::ALL
        .into_iter()
        .flat_map(|s| by_style!(s, ASSETS).iter())
}

/// Every style's fonts, added to the text system at startup.
pub fn fonts() -> impl Iterator<Item = &'static [u8]> {
    Style::ALL
        .into_iter()
        .flat_map(|s| by_style!(s, FONTS).iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default style is ready and listed first; Classic stays on
    /// offer, last.
    #[test]
    fn the_default_is_offered_first_and_classic_last() {
        assert!(ready(Style::default()));
        assert_eq!(choices().first(), Some(&Style::default()));
        assert!(ready(Style::Classic));
        assert_eq!(choices().last(), Some(&Style::Classic));
    }

    /// Asset paths are unique and prefixed with their style's id, so two
    /// styles never serve each other's files.
    #[test]
    fn assets_are_namespaced() {
        let mut seen = std::collections::BTreeSet::new();
        for s in Style::ALL {
            for (path, bytes) in by_style!(s, ASSETS) {
                assert!(
                    path.starts_with(&format!("{}/", s.id())),
                    "{path}: not under {}/",
                    s.id()
                );
                assert!(!bytes.is_empty(), "{path} is empty");
                assert!(seen.insert(*path), "{path} is listed twice");
            }
        }
    }

    /// A style that sets colors of its own ships its token file.
    #[test]
    fn ready_styles_have_tokens() {
        for s in choices() {
            assert_eq!(s.tokens().is_some(), s != Style::Classic, "{s:?}");
        }
    }
}

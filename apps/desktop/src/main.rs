//! Taful desktop app (spec §15): window, navigation, and the first-slice
//! screens built so far (home, skill tree, lesson reader + quiz, calc
//! trainer, review session, settings) and the table screen (preflop spot mode
//! and full hands).

// No console window on Windows release builds; debug builds keep it for logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod felt;
mod screen;
mod screens;
mod shell;
mod theme;
mod tour;
mod widgets;

use std::rc::Rc;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use taful_app::i18n::{Locale, Localizer};
use taful_app::theme::Theme;

/// Everything views read but never change: strings and tokens. Replaced as
/// a whole when the locale or the theme changes.
pub struct Ui {
    /// Localized strings (shared: the theme editor swaps tokens on every
    /// edit without parsing the strings again).
    pub i18n: Rc<Localizer>,
    /// Theme tokens.
    pub theme: Theme,
}

impl Ui {
    pub fn new(locale: Locale, theme: Theme) -> Self {
        Self {
            i18n: Rc::new(Localizer::new(locale).expect("embedded strings are valid (tested)")),
            theme,
        }
    }

    /// The same strings with other tokens (live theme preview).
    pub fn with_theme(&self, theme: Theme) -> Self {
        Self {
            i18n: self.i18n.clone(),
            theme,
        }
    }
}

fn main() {
    let data_dir = taful_app::default_data_dir();
    let app = match taful_app::App::open(&data_dir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("taful: cannot open {}: {e}", data_dir.display());
            std::process::exit(1);
        }
    };
    // A dev override beats the saved setting.
    let locale = std::env::var("TAFUL_LOCALE")
        .ok()
        .and_then(|tag| Locale::from_tag(&tag))
        .unwrap_or_else(|| app.locale());
    // TAFUL_THEME=<id> (screenshots) beats the saved theme.
    let theme = std::env::var("TAFUL_THEME")
        .ok()
        .and_then(|id| app.load_theme(&id))
        .unwrap_or_else(|| app.theme());
    let ui = Rc::new(Ui::new(locale, theme));
    let mut app = Some(app);
    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        // Fit small screens (CI runners default to 1024×768).
        let (w, h) = cx
            .primary_display()
            .map(|d| d.bounds().size)
            .map(|s| (f32::from(s.width) * 0.95, f32::from(s.height) * 0.9))
            .map_or((1280., 800.), |(w, h)| (w.min(1280.), h.min(800.)));
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        let title = ui.i18n.tr("window-title");
        let app = app.take().expect("run callback runs once");
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(800.), px(520.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| shell::Shell::new(ui.clone(), app, window, cx)),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}

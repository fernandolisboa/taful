//! Taful desktop app (spec §15). M0: the app shell — window, navigation over
//! the first-slice screens, localized strings and theme tokens. M3: the
//! table screen (preflop spot mode and full hands).

// No console window on Windows release builds; debug builds keep it for logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod screen;
mod screens;
mod shell;
mod theme;

use std::rc::Rc;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use taful_app::i18n::{Locale, Localizer};
use taful_app::theme::Theme;

/// Everything views read but never change: strings and tokens.
pub struct Ui {
    /// Localized strings.
    pub i18n: Localizer,
    /// Theme tokens.
    pub theme: Theme,
}

impl Ui {
    fn load() -> Self {
        // Settings land in M2; until then a dev override picks the locale.
        let locale = std::env::var("TAFUL_LOCALE")
            .ok()
            .and_then(|tag| Locale::from_tag(&tag))
            .unwrap_or(Locale::DEFAULT);
        Self {
            i18n: Localizer::new(locale).expect("embedded strings are valid (tested)"),
            theme: Theme::builtin_default(),
        }
    }
}

fn main() {
    let ui = Rc::new(Ui::load());
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
            |_, cx| cx.new(|cx| shell::Shell::new(ui.clone(), cx)),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}

//! Taful desktop app (spec §15): window, navigation, and the first-slice
//! screens built so far (home, skill tree, lesson reader + quiz, calc
//! trainer, review session, settings) and the table screen (preflop spot mode
//! and full hands).

// No console window on Windows release builds; debug builds keep it for logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod baralho;
mod caption;
mod classic;
mod cockpit;
mod felt;
mod motion;
mod salao;
mod screen;
mod screens;
mod shell;
mod style;
mod theme;
mod tour;
mod tray;
mod updater;
mod widgets;

use std::rc::Rc;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use taful_app::i18n::{Locale, Localizer};
use taful_app::style::Style;
use taful_app::theme::Theme;

/// Everything views read but never change: strings, tokens and the visual
/// style. Replaced as a whole when the locale, the theme or the style
/// changes.
pub struct Ui {
    /// Localized strings (shared: the theme editor swaps tokens on every
    /// edit without parsing the strings again).
    pub i18n: Rc<Localizer>,
    /// Theme tokens: the user's theme in the Classic style, the style's own
    /// in every other (D19).
    pub theme: Theme,
    /// The visual style drawing the app.
    pub style: Style,
    /// The style's colors that have no theme token, by name.
    pub extra: Rc<theme::Extra>,
    /// The style has colors of its own: `theme` is the style's, not the
    /// user's.
    pub own_colors: bool,
}

impl Ui {
    /// `theme` is the user's theme; a style with colors of its own
    /// replaces it.
    pub fn new(locale: Locale, theme: Theme, style: Style) -> Self {
        let (theme, extra, own_colors) = match style.tokens() {
            Some(t) => (t.theme, theme::Extra::new(&t.extra), true),
            None => (theme, theme::Extra::default(), false),
        };
        Self {
            i18n: Rc::new(Localizer::new(locale).expect("embedded strings are valid (tested)")),
            theme,
            style,
            extra: Rc::new(extra),
            own_colors,
        }
    }

    /// The same strings and style with other tokens (live theme preview).
    pub fn with_theme(&self, theme: Theme) -> Self {
        Self {
            i18n: self.i18n.clone(),
            theme,
            style: self.style,
            extra: self.extra.clone(),
            own_colors: self.own_colors,
        }
    }
}

/// The smallest the window gets (D21). Every style lays every screen out
/// from this size up; 940 is what half of a Full HD screen leaves a snapped
/// window.
pub const MIN_WINDOW: (f32, f32) = (940., 600.);

/// `TAFUL_WINDOW=<width>x<height>` (screenshots at a mockup's size).
fn window_override() -> Option<(f32, f32)> {
    let v = std::env::var("TAFUL_WINDOW").ok()?;
    let (w, h) = v.split_once('x')?;
    let (w, h): (f32, f32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    // Sizes a window can have.
    let ok = |x: f32| (200. ..=16000.).contains(&x);
    (ok(w) && ok(h)).then_some((w, h))
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
    let style = style::startup(app.style());
    let ui = Rc::new(Ui::new(locale, theme, style));
    let mut app = Some(app);
    let gpui_app = Application::new().with_assets(assets::Assets);
    gpui_app.run(move |cx: &mut App| {
        let fonts: Vec<_> = style::fonts().map(std::borrow::Cow::Borrowed).collect();
        if !fonts.is_empty()
            && let Err(e) = cx.text_system().add_fonts(fonts)
        {
            eprintln!("taful: loading the embedded fonts failed: {e}");
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        // Fit small screens (CI runners default to 1024×768). An explicit
        // TAFUL_WINDOW is taken as given.
        let (want_w, want_h) = style::window_size(ui.style).unwrap_or((1280., 800.));
        // Sizes are the room the styles draw in; the app's own title bar
        // comes on top (D22). The display cap holds for the whole window.
        let (w, h) = window_override()
            .map(|(w, h)| (w, h + caption::HEIGHT))
            .unwrap_or_else(|| {
                let want_h = want_h + caption::HEIGHT;
                cx.primary_display()
                    .map(|d| d.bounds().size)
                    .map(|s| (f32::from(s.width) * 0.95, f32::from(s.height) * 0.9))
                    .map_or((want_w, want_h), |(w, h)| (w.min(want_w), h.min(want_h)))
            });
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        let title = ui.i18n.tr("window-title");
        let app = app.take().expect("run callback runs once");
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(MIN_WINDOW.0), px(MIN_WINDOW.1 + caption::HEIGHT))),
                // No system title bar: the app draws its own (D22). The
                // title still names the window in the taskbar and Alt+Tab.
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    appears_transparent: true,
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

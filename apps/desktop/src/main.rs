//! Spike §9.1 (docs/brief.md): render a poker table with GPUI on Windows and
//! validate a draggable, grid-snapping panel. Throwaway code: the real app
//! structure comes from the spec, not from here.

mod table;
mod theme;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};

fn main() {
    Application::new().run(|cx: &mut App| {
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
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Taful — spike da mesa (GPUI)".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| table::TableView::new()),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}

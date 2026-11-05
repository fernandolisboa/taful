//! App shell: navigation rail + the current screen.

use std::rc::Rc;

use gpui::{Context, Entity, FontWeight, SharedString, Window, div, prelude::*, px};

use crate::Ui;
use crate::screen::Screen;
use crate::screens::table::TableScreen;
use crate::theme::Palette;

const NAV_WIDTH: f32 = 232.;
const CONTENT_MAX_WIDTH: f32 = 720.;
const SELECTION_BAR: f32 = 3.;

pub struct Shell {
    ui: Rc<Ui>,
    palette: Palette,
    screen: Screen,
    table: Entity<TableScreen>,
}

impl Shell {
    pub fn new(ui: Rc<Ui>, cx: &mut Context<Self>) -> Self {
        let palette = Palette::new(&ui.theme.colors);
        let table = cx.new(|_| TableScreen::new(ui.clone()));
        // Dev override (and CI screenshots): TAFUL_SCREEN=table opens a screen.
        let screen = std::env::var("TAFUL_SCREEN")
            .ok()
            .and_then(|id| Screen::from_id(&id))
            .unwrap_or(Screen::Home);
        Self {
            ui,
            palette,
            screen,
            table,
        }
    }

    fn select(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen != screen {
            self.screen = screen;
            cx.notify();
        }
    }

    fn nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let items = Screen::ALL.into_iter().enumerate().map(|(i, screen)| {
            let selected = screen == self.screen;
            let label: SharedString = i18n.tr(screen.nav_key()).into();
            div()
                .id(("nav", i))
                .flex()
                .items_center()
                .px(px(t.spacing.md))
                .py(px(t.spacing.sm))
                .rounded(px(t.radii.md))
                .border_l(px(SELECTION_BAR))
                .border_color(if selected { p.accent } else { p.surface })
                .cursor_pointer()
                .text_size(px(t.text_size.md))
                .when(selected, |d| {
                    d.bg(p.surface_raised)
                        .text_color(p.text)
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!selected, |d| {
                    d.text_color(p.text_muted)
                        .hover(|s| s.bg(p.surface_raised).border_color(p.surface_raised))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.select(screen, cx)))
                .child(label)
        });

        div()
            .w(px(NAV_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(t.spacing.xl))
            .p(px(t.spacing.lg))
            .bg(p.surface)
            .border_r_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.xs))
                    .px(px(t.spacing.md))
                    .child(
                        div()
                            .text_size(px(t.text_size.xl))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.accent)
                            .child(i18n.tr("app-name")),
                    )
                    .child(
                        div()
                            .text_size(px(t.text_size.sm))
                            .text_color(p.text_muted)
                            .child(i18n.tr("app-tagline")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.xs))
                    .children(items),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.xs))
                    .px(px(t.spacing.md))
                    .text_size(px(t.text_size.sm))
                    .text_color(p.text_muted)
                    .child(i18n.tr("footer-offline"))
                    .child(i18n.tr_args(
                        "footer-version",
                        &[("version", env!("CARGO_PKG_VERSION").into())],
                    )),
            )
    }

    fn content(&self) -> impl IntoElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let screen = self.screen;
        if screen == Screen::Table {
            return div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(t.spacing.lg))
                .p(px(t.spacing.xxl))
                .child(
                    div()
                        .text_size(px(t.text_size.xl))
                        .font_weight(FontWeight::BOLD)
                        .child(i18n.tr(screen.nav_key())),
                )
                .child(self.table.clone());
        }
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(t.spacing.xl))
            .p(px(t.spacing.xxl))
            .child(
                div()
                    .text_size(px(t.text_size.xl))
                    .font_weight(FontWeight::BOLD)
                    .child(i18n.tr(screen.nav_key())),
            )
            .child(
                div()
                    .max_w(px(CONTENT_MAX_WIDTH))
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.md))
                    .p(px(t.spacing.xl))
                    .rounded(px(t.radii.lg))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .text_size(px(t.text_size.lg))
                            .child(i18n.tr(screen.body_key())),
                    )
                    .child(
                        div()
                            .text_size(px(t.text_size.md))
                            .text_color(p.accent)
                            .child(i18n.tr_args(
                                "screen-coming-in",
                                &[("milestone", screen.milestone().into())],
                            )),
                    ),
            )
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(self.palette.background)
            .text_color(self.palette.text)
            .child(self.nav(cx))
            .child(self.content())
    }
}

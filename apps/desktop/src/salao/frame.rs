//! Salão's frame: the lit background, the sidebar and the content column.

use gpui::{
    AnyElement, Context, Div, FontWeight, SharedString, Window, div, prelude::*, px, relative,
};

use super::colors::colors;
use super::kit;
use crate::screen::{NAV_BOTTOM, NAV_GROUPS, Screen};
use crate::shell::Shell;
use crate::theme::alpha;

/// The sidebar's width.
pub const SIDE_W: f32 = 224.;
/// The content column's width (home, lessons, reading screens).
const COLUMN_W: f32 = 1080.;
/// The wide screens' side padding (the table and the screens with one).
pub const WIDE_PAD: f32 = 32.;

/// Chrome and content: everything under the tour and update overlays.
pub fn frame(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    let vp = window.viewport_size();
    let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
    let main = main(shell, vw, window, cx);
    let side = sidebar(shell, cx);
    div()
        .relative()
        .size_full()
        .flex()
        // The sidebar is drawn last (its shadow falls on the content) and
        // sits first.
        .flex_row_reverse()
        .text_size(px(14.))
        .line_height(relative(kit::LEADING))
        // Two pixels past every edge: stretched this far, the picture's
        // outermost rows pick up its neighbors in the sprite atlas.
        .child(
            kit::image(kit::IMG_BG, vw + 4., vh + 4.)
                .absolute()
                .top(px(-2.))
                .left(px(-2.)),
        )
        .child(main)
        .child(side)
        .into_any_element()
}

/// The Taful mark ("Gravata") and the wordmark.
fn brand(shell: &Shell) -> Div {
    let c = colors();
    const MARK: f32 = 34.;
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(MARK))
                .child(
                    kit::icon(kit::SVG_LOGO_WINGS, MARK, c.logo_wings)
                        .absolute()
                        .top_0()
                        .left_0(),
                )
                .child(
                    kit::icon(kit::SVG_LOGO_KNOT, MARK, c.logo_knot)
                        .absolute()
                        .top_0()
                        .left_0(),
                ),
        )
        .child(
            kit::BRAND
                .text(27., kit::LEADING)
                .text_color(c.gold2)
                .child(shell.ui.i18n.tr("app-name")),
        )
}

fn sidebar(shell: &mut Shell, cx: &mut Context<Shell>) -> Div {
    let (c, p) = (colors(), shell.palette);
    let ui = shell.ui.clone();
    let i18n = &ui.i18n;
    let due = shell.app.due_count().unwrap_or(0);
    // The lesson reader lives under the lessons item.
    let current = if shell.screen == Screen::Lesson {
        Screen::SkillTree
    } else {
        shell.screen
    };
    let item = |i: usize, screen: Screen, cx: &mut Context<Shell>| {
        let on = screen == current;
        let label: SharedString = i18n.tr(screen.nav_key()).into();
        div()
            .id(("salao_nav", i))
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(9.))
            .rounded(px(8.))
            .cursor_pointer()
            .when(on, |d| {
                d.text_color(p.text)
                    .bg(kit::hgrad(
                        alpha(p.accent, 0.16),
                        alpha(p.accent, 0.03),
                        0.7,
                    ))
                    .child(kit::ring(8., alpha(p.accent, 0.14)))
                    .child(
                        div()
                            .absolute()
                            .left(px(-16.))
                            .top(px(8.))
                            .bottom(px(8.))
                            .w(px(3.))
                            .rounded_r(px(3.))
                            .bg(p.accent)
                            .shadow(vec![kit::shadow(alpha(p.accent, 0.7), 0., 0., 12., 0.)]),
                    )
            })
            .when(!on, |d| {
                d.text_color(p.text_muted).hover(|s| s.text_color(p.text))
            })
            .on_click(cx.listener(move |this, _, window, cx| this.select(screen, window, cx)))
            .child(kit::sans_text(14., FontWeight::NORMAL).child(label))
            .when(screen == Screen::Review && due > 0, |d| {
                d.child(
                    div()
                        .px(px(7.))
                        .py(px(1.))
                        .rounded(px(99.))
                        .bg(p.accent)
                        .text_color(p.on_accent)
                        .child(kit::sans_text(11., FontWeight::SEMIBOLD).child(due.to_string())),
                )
            })
    };
    let mut n = 0;
    let mut nav = div().mt(px(26.)).flex().flex_col().gap(px(2.));
    for (heading, screens) in NAV_GROUPS {
        if let Some(h) = heading {
            nav = nav.child(div().pt(px(16.)).px(px(12.)).pb(px(6.)).child(kit::label(
                kit::SANS_R16,
                10.5,
                c.dim,
                i18n.tr(h),
            )));
        }
        for &screen in screens {
            nav = nav.child(item(n, screen, cx));
            n += 1;
        }
    }
    let settings_on = current == NAV_BOTTOM;
    let settings = div()
        .id("salao_nav_settings")
        .mb(px(14.))
        .cursor_pointer()
        .text_color(if settings_on { p.text } else { p.text_muted })
        .hover(|s| s.text_color(p.text))
        .on_click(cx.listener(|this, _, window, cx| this.select(NAV_BOTTOM, window, cx)))
        .child(
            kit::SANS
                .text(14., 1.5)
                .child(i18n.tr(NAV_BOTTOM.nav_key())),
        );
    let version = crate::updater::current_version().to_string();
    div()
        .relative()
        .w(px(SIDE_W))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .pt(px(26.))
        .px(px(16.))
        .pb(px(20.))
        .bg(kit::vgrad(c.side_top, c.side_bottom))
        .border_r_1()
        .border_color(alpha(p.accent, 0.16))
        .shadow(vec![kit::shadow(alpha(c.black, 0.35), 8., 0., 30., 0.)])
        .child(brand(shell))
        .child(
            div().pt(px(6.)).px(px(10.)).child(
                kit::SANS_R02
                    .lines(11.5, kit::LEADING)
                    .text_color(c.dim)
                    .child(i18n.tr("app-tagline")),
            ),
        )
        .child(
            div()
                .relative()
                .child(crate::tour::anchor("nav"))
                .child(nav),
        )
        .child(div().flex_1())
        .child(
            div().px(px(12.)).child(settings).child(
                kit::SANS
                    .lines(11.5, 1.5)
                    .text_color(c.dim)
                    .child(div().child(i18n.tr("footer-offline")))
                    .child(
                        div().child(i18n.tr_args("footer-version", &[("version", version.into())])),
                    ),
            ),
        )
}

/// A screen's serif title, `size` pixels.
pub fn h1(shell: &Shell, size: f32) -> Div {
    kit::H1
        .text(size, kit::LEADING)
        .child(shell.ui.i18n.tr(shell.screen.nav_key()))
}

/// A screen's title and its one-line description, as the mockup's pages
/// open.
pub fn page_title(shell: &Shell) -> Div {
    let p = shell.palette;
    div()
        .relative()
        .flex()
        .flex_col()
        .child(crate::tour::anchor("screen_title"))
        .child(h1(shell, 38.).mt(px(4.)).mb(px(2.)))
        .when(
            matches!(
                shell.screen,
                Screen::Home
                    | Screen::SkillTree
                    | Screen::Calc
                    | Screen::HandReview
                    | Screen::Review
                    | Screen::Settings
            ),
            |d| {
                d.child(
                    kit::sans_text(14.5, FontWeight::NORMAL)
                        .text_color(p.text_muted)
                        .child(shell.ui.i18n.tr(shell.screen.body_key())),
                )
            },
        )
}

fn main(shell: &mut Shell, vw: f32, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    let main_w = (vw - SIDE_W).max(0.);
    let visit = shell.screen_visit;
    match shell.screen {
        // The table lays itself out over the whole pane.
        Screen::Table => div()
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(shell.screen_body(window, cx))
            .into_any_element(),
        // Screens with a table take the pane's width.
        Screen::HandReview | Screen::Review => {
            let title = h1(shell, 34.);
            let body = shell.screen_body(window, cx);
            div()
                .id("salao_wide")
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .track_scroll(&shell.scroll)
                .child(crate::motion::fade_in(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .gap(px(14.))
                        .pt(px(26.))
                        .px(px(WIDE_PAD))
                        .pb(px(28.))
                        .child(
                            div()
                                .relative()
                                .child(crate::tour::anchor("screen_title"))
                                .child(title),
                        )
                        .child(body),
                    ("screen_fade", visit),
                ))
                .into_any_element()
        }
        screen => {
            let column = COLUMN_W.min(main_w - 2. * WIDE_PAD).max(0.);
            // Home and the lessons open with their own header.
            let header =
                (!matches!(screen, Screen::Home | Screen::SkillTree)).then(|| page_title(shell));
            let body = shell.screen_body(window, cx);
            div()
                .id("salao_main")
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .overflow_y_scroll()
                .track_scroll(&shell.scroll)
                .child(crate::motion::fade_in(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        // A definite width: text measured under a relative
                        // one wraps at the wrong place.
                        .w(px(column))
                        .pt(px(34.))
                        .pb(px(34.))
                        .when(header.is_some(), |d| d.gap(px(22.)))
                        .children(header)
                        .child(body),
                    ("screen_fade", visit),
                ))
                .into_any_element()
        }
    }
}

//! Cockpit's chrome: the icon rail, the status bar with today's numbers,
//! and the dotted backdrop the screens sit on.

use gpui::{AnyElement, Context, Div, FontWeight, Window, div, prelude::*, px, relative};
use taful_app::dashboard::Day;

use super::kit::{self, EDGE, MONO, SANS, STRONG, TEXT, icon, label};
use super::pal::Pal;
use crate::Ui;
use crate::screen::Screen;
use crate::shell::{CONTENT_MAX_WIDTH, Shell};

/// The rail's width and the status bar's height.
pub const RAIL: f32 = 76.;
pub const BAR: f32 = 48.;
/// The smallest room a Cockpit screen is laid out in; a smaller window
/// scrolls. The width is what a 1280 px window leaves.
const MIN_ROOM: (f32, f32) = (1176., 824.);

/// The rail's items, top to bottom: screen, icon, short label.
const ITEMS: [(Screen, &str, &str); 6] = [
    (Screen::Home, "home", "nav-home"),
    (Screen::SkillTree, "book", "nav-skill-tree"),
    (Screen::Table, "table", "nav-table"),
    (Screen::Calc, "calc", "cockpit-nav-calc"),
    (Screen::Review, "review", "cockpit-nav-review"),
    (Screen::HandReview, "hands", "cockpit-nav-hands"),
];
const BOTTOM: (Screen, &str, &str) = (Screen::Settings, "settings", "cockpit-nav-settings");

/// Chrome and content.
pub fn frame(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    super::state::sync(shell);
    let pal = Pal::new(&shell.ui);
    let view = view(shell, &pal, window, cx);
    let table_crumb = (shell.screen == Screen::Table)
        .then(|| super::table::crumb(shell.table.read(cx)))
        .flatten();
    div()
        .size_full()
        .flex()
        .font_family(SANS)
        .text_size(px(TEXT))
        .line_height(relative(1.4))
        .child(rail(shell, &pal, cx))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .flex()
                .flex_col()
                .child(bar(shell, table_crumb, &pal))
                .child(view),
        )
        .into_any_element()
}

fn rail(shell: &Shell, pal: &Pal, cx: &mut Context<Shell>) -> Div {
    let i18n = &shell.ui.i18n;
    // The lesson reader lives under the lessons item.
    let current = if shell.screen == Screen::Lesson {
        Screen::SkillTree
    } else {
        shell.screen
    };
    let due = shell.cockpit.due();
    let item = |(screen, glyph, key): (Screen, &'static str, &'static str),
                cx: &mut Context<Shell>| {
        let on = screen == current;
        let color = if on { pal.amber } else { pal.muted };
        let (hover_bg, hover_fg) = (pal.panel2, if on { pal.amber } else { pal.text });
        div()
            .id(("cockpit_nav", screen as usize))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .w(px(64.))
            .h(px(54.))
            .mb(px(2.))
            .rounded(px(8.))
            .text_size(px(10.))
            .text_color(color)
            .cursor_pointer()
            .when(on, |d| d.bg(pal.panel2))
            .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
            .on_click(cx.listener(move |this, _, window, cx| this.select(screen, window, cx)))
            .when(on, |d| {
                // The marker sits on the rail's edge, outside the item.
                d.child(
                    div()
                        .absolute()
                        .left(px(-6.))
                        .top(px(12.))
                        .bottom(px(12.))
                        .w(px(3.))
                        .rounded_tr(px(2.))
                        .rounded_br(px(2.))
                        .bg(pal.amber),
                )
            })
            .child(icon(glyph, 20., color))
            .child(i18n.tr(key))
            .when(screen == Screen::Review && due > 0, |d| {
                d.child(
                    div()
                        .absolute()
                        .right(px(9.))
                        .top(px(5.))
                        .px(px(4.))
                        .rounded(px(8.))
                        .bg(pal.amber)
                        .text_color(pal.on_amber)
                        .font_family(MONO)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(9.5))
                        .line_height(px(14.))
                        .child(due.to_string()),
                )
            })
    };
    let mut rail = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .w(px(RAIL))
        .h_full()
        .pt(px(10.))
        .pb(px(12.))
        .bg(pal.rail_bg)
        .border_r_1()
        .border_color(pal.line)
        .child(crate::tour::anchor("nav"))
        // The mark (a bow tie with a diamond knot), in its two colors.
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(36.))
                .mb(px(14.))
                .child(
                    icon("logo_wings", 34., pal.amber)
                        .absolute()
                        .top(px(1.))
                        .left(px(1.)),
                )
                .child(
                    icon("logo_knot", 34., pal.logo_knot)
                        .absolute()
                        .top(px(1.))
                        .left(px(1.)),
                ),
        );
    for it in ITEMS {
        rail = rail.child(item(it, cx));
    }
    rail.child(div().flex_1())
        .child(
            div()
                .flex_none()
                .w(px(32.))
                .h(px(1.))
                .mt(px(6.))
                .mb(px(8.))
                .bg(pal.line),
        )
        .child(item(BOTTOM, cx))
}

/// "sexta, 20 nov".
pub fn date(ui: &Ui, day: Day) -> String {
    let i18n = &ui.i18n;
    i18n.tr_args(
        "cockpit-date",
        &[
            (
                "weekday",
                i18n.tr(&format!("cockpit-weekday-{}", day.weekday)).into(),
            ),
            ("day", day.day.to_string().into()),
            (
                "month",
                i18n.tr(&format!("cockpit-month-{}", day.month)).into(),
            ),
        ],
    )
}

/// A percentage of a 0–1 share, whole.
pub fn percent(x: f64) -> String {
    format!("{:.0}", x * 100.)
}

fn bar(shell: &Shell, table_crumb: Option<String>, pal: &Pal) -> Div {
    let (ui, i18n) = (&shell.ui, &shell.ui.i18n);
    let today = shell.cockpit.today();
    let crumb = match shell.screen {
        Screen::Home => today.as_ref().map(|t| date(ui, t.day)),
        Screen::SkillTree => shell.cockpit.lessons().map(|l| {
            i18n.tr_args(
                "cockpit-lessons-crumb",
                &[
                    ("tracks", l.summary.tracks.into()),
                    ("lessons", l.summary.total.into()),
                ],
            )
        }),
        Screen::Table => table_crumb,
        _ => None,
    };
    let metric = || {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(7.))
            .h_full()
            .px(px(14.))
            .border_l_1()
            .border_color(pal.line)
    };
    let number = |text: String| {
        div()
            .font_family(MONO)
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(13.))
            .child(text)
    };
    let none = || i18n.tr("cockpit-none");
    let decisions = today.as_ref().map_or(0, |t| t.decisions);
    let accuracy = today.as_ref().and_then(|t| t.accuracy);
    let dots = today.as_ref().map(|t| t.last.clone()).unwrap_or_default();
    let right = div()
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .child(
            metric()
                .child(label(&i18n.tr("cockpit-bar-today"), pal))
                .child(number(decisions.to_string()))
                .child(label(
                    &i18n.tr_args("cockpit-bar-decisions", &[("count", decisions.into())]),
                    pal,
                )),
        )
        .child(
            metric()
                .child(label(&i18n.tr("cockpit-bar-accuracy"), pal))
                .child(match accuracy {
                    Some(a) => {
                        number(i18n.tr_args("cockpit-percent", &[("value", percent(a).into())]))
                            .text_color(pal.grades[0])
                    }
                    None => number(none()).text_color(pal.muted),
                }),
        )
        .child(
            metric()
                .child(label(&i18n.tr("cockpit-bar-ev-lost"), pal))
                .child(number(i18n.tr_args(
                    "cockpit-bb",
                    &[(
                        "value",
                        crate::felt::num(ui, today.as_ref().map_or(0., |t| t.ev_lost_bb), 1).into(),
                    )],
                ))),
        )
        .child(
            metric()
                .child(label(&i18n.tr("cockpit-bar-last"), pal))
                .child(
                    div().flex().gap(px(4.)).children(
                        dots.into_iter()
                            .map(|g| div().size(px(8.)).rounded(px(2.)).bg(pal.grade(g))),
                    ),
                )
                .when(today.as_ref().is_none_or(|t| t.last.is_empty()), |d| {
                    d.child(number(none()).text_color(pal.muted))
                }),
        );
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(14.))
        .h(px(BAR))
        .pl(px(16.))
        .border_b_1()
        .border_color(pal.line)
        .bg(pal.bar_bg)
        .overflow_hidden()
        .child(
            div()
                .relative()
                .flex_none()
                .text_size(px(15.))
                .font_weight(STRONG)
                .child(crate::tour::anchor("screen_title"))
                .child(i18n.tr(shell.screen.nav_key())),
        )
        .children(crumb.map(|c| {
            div()
                .flex_none()
                .text_size(px(12.5))
                .text_color(pal.muted)
                .child(c)
        }))
        .child(div().flex_1())
        .child(right)
}

/// The current screen on the dotted backdrop.
fn view(shell: &mut Shell, pal: &Pal, window: &mut Window, cx: &mut Context<Shell>) -> Div {
    let t = shell.ui.theme.clone();
    let vp = window.viewport_size();
    let (w, h) = (
        (f32::from(vp.width) - RAIL).max(0.),
        (f32::from(vp.height) - BAR).max(0.),
    );
    shell.cockpit.room = (
        (w - 2. * EDGE).max(MIN_ROOM.0),
        (h - 2. * EDGE).max(MIN_ROOM.1),
    );
    let body = shell.screen_body(window, cx);
    let fade = ("screen_fade", shell.screen_visit);
    let scroll = div()
        .id("cockpit_view")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .track_scroll(&shell.scroll);
    let content = match shell.screen {
        // Cockpit's own screens lay themselves out on the whole pane.
        Screen::Home | Screen::SkillTree => scroll.overflow_scroll().child(crate::motion::fade_in(
            div().flex_none().p(px(EDGE)).child(body),
            fade,
        )),
        // The table fills the pane, edge to edge.
        Screen::Table => scroll
            .overflow_scroll()
            .child(crate::motion::fade_in(div().flex_none().child(body), fade)),
        Screen::HandReview | Screen::Review => {
            scroll.overflow_y_scroll().child(crate::motion::fade_in(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(w))
                    .p(px(t.spacing.xxl))
                    .child(body),
                fade,
            ))
        }
        // A reading column, centered.
        _ => scroll
            .flex()
            .flex_col()
            .items_center()
            .overflow_y_scroll()
            .child(crate::motion::fade_in(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(w.min(CONTENT_MAX_WIDTH + 2. * t.spacing.xxl)))
                    .p(px(t.spacing.xxl))
                    .child(body),
                fade,
            )),
    };
    div()
        .relative()
        .flex_1()
        .min_h(px(0.))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(kit::dots(pal)),
        )
        .child(content)
}

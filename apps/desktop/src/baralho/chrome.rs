//! Baralho's frame: the felt behind everything, the navigation bar at the
//! top of the canvas, and the page under it. Home, lessons and the table
//! are drawn on the felt at the mockups' coordinates; every other screen
//! is a cream sheet under a heading.

use std::rc::Rc;

use gpui::{AnyElement, Context, Div, SharedString, Stateful, Window, div, prelude::*, px};

use super::kit::{self, Raised};
use super::{Bx, CANVAS_W, bx, fit, paint, typo, u};
use crate::Ui;
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::alpha;

/// Height of the navigation bar.
pub const NAV_H: f32 = 70.;
/// The canvas' side margin (headings and panels start here).
pub const MARGIN: f32 = 64.;
/// Width of a sheet that holds a reading column, and a sheet's padding.
const SHEET_W: f32 = 1040.;
const SHEET_PAD: f32 = 30.;

/// The navigation's screens, left to right; settings sits at the right.
const NAV: [Screen; 6] = [
    Screen::Home,
    Screen::SkillTree,
    Screen::Table,
    Screen::Calc,
    Screen::Review,
    Screen::HandReview,
];

pub fn frame(shell: &mut Shell, window: &mut Window, cx: &mut Context<Shell>) -> AnyElement {
    let s = fit(window);
    paint::begin_frame(window, cx);
    let bx = bx(&shell.ui);
    let vp = crate::caption::viewport(window);
    let left = ((f32::from(vp.width) - CANVAS_W * s) / 2.).max(0.);

    let page = match shell.screen {
        Screen::Home => super::home(shell, cx),
        Screen::SkillTree => super::lessons(shell, cx),
        Screen::Table => shell.table.clone().into_any_element(),
        _ => sheet_page(shell, &bx, window, cx),
    };
    let nav = nav(shell, &bx, cx);

    div()
        .size_full()
        .relative()
        .overflow_hidden()
        .text_color(bx.on_felt)
        .child(
            paint::felt(bx, px(left))
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
        .child(
            typo::text(15., 400)
                .on(div())
                .absolute()
                .top_0()
                .left(px(left))
                .w(u(CANVAS_W))
                .h_full()
                .child(crate::motion::fade_in(
                    div().absolute().top_0().left_0().size_full().child(page),
                    ("screen_fade", shell.screen_visit),
                ))
                .child(nav),
        )
        .into_any_element()
}

/// The pill that plays the screen's tour again.
fn tour_pill(shell: &Shell, bx: &Bx, cx: &mut Context<Shell>) -> Stateful<Div> {
    typo::text(13., 700)
        .on(div())
        .id("tour_replay")
        .flex_none()
        .flex()
        .items_center()
        .h(u(32.))
        .px(u(14.))
        .rounded_full()
        .bg(alpha(bx.well, 0.5))
        .border(u(2.))
        .border_color(bx.deep)
        .cursor_pointer()
        .opacity(0.86)
        .hover(|s| s.opacity(1.))
        .on_click(cx.listener(|this, _, _, cx| {
            this.tour_start();
            cx.notify();
        }))
        .child(format!("? {}", shell.ui.i18n.tr("tour-replay")))
}

fn nav(shell: &mut Shell, bx: &Bx, cx: &mut Context<Shell>) -> Div {
    let due = shell.app.due_count().unwrap_or(0);
    let shell = &*shell;
    let i18n = &shell.ui.i18n;
    // The lesson reader lives under the lessons item.
    let current = if shell.screen == Screen::Lesson {
        Screen::SkillTree
    } else {
        shell.screen
    };
    let item = |i: usize, screen: Screen, cx: &mut Context<Shell>| {
        let label: SharedString = i18n.tr(screen.nav_key()).into();
        let badge = (screen == Screen::Review && due > 0).then(|| {
            typo::text(12., 800)
                .lh(20. / 12.)
                .on(div())
                .px(u(7.))
                .rounded_full()
                .bg(shell.palette.suit_hearts)
                .text_color(bx.paper)
                .child(due.to_string())
        });
        let content = typo::text(15., 600)
            .on(div())
            .flex()
            .items_center()
            .gap(u(8.))
            .whitespace_nowrap();
        let link = if screen == current {
            Raised {
                face: bx.cream,
                edge: bx.deep,
                radius: 21.,
                border: 2.,
                drop: 4.,
            }
            .wrap(
                content
                    .px(u(16.))
                    .text_color(bx.ink)
                    .child(label)
                    .children(badge),
            )
            .h(u(46.))
        } else {
            content
                .h(u(42.))
                .px(u(18.))
                .rounded_full()
                .opacity(0.86)
                .hover(|s| s.opacity(1.))
                .child(label)
                .children(badge)
        };
        link.id(("nav", i))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| this.select(screen, window, cx)))
    };

    let logo = div()
        .flex()
        .items_center()
        .gap(u(10.))
        .mr(u(36.))
        .child(
            div()
                .relative()
                .flex_none()
                .size(u(38.))
                .child(kit::icon(kit::LOGO_WINGS, 38., bx.logo_wings).absolute())
                .child(kit::icon(kit::LOGO_KNOT, 38., bx.logo_knot).absolute()),
        )
        .child(
            typo::disp(30., 800, 40)
                .on(div())
                .child(i18n.tr("app-name")),
        );

    let mut bar = div()
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(u(NAV_H))
        .flex()
        .items_center()
        .gap(u(8.))
        .px(u(40.))
        .child(crate::tour::anchor("nav"))
        .child(logo);
    for (i, screen) in NAV.into_iter().enumerate() {
        bar = bar.child(item(i, screen, cx));
    }
    // The screens drawn as mocked have no heading to carry the tour's
    // pill: it sits here, before the settings.
    let tour = matches!(
        shell.screen,
        Screen::Home | Screen::SkillTree | Screen::Table
    )
    .then(|| tour_pill(shell, bx, cx).mr(u(4.)));
    bar.child(div().flex_1())
        .children(tour)
        .child(item(NAV.len(), crate::screen::NAV_BOTTOM, cx))
}

/// `ui` with the theme's sizes scaled `s` times, for the shared code that
/// lays itself out from them. `cache` keeps the last one made.
pub fn scaled(cache: &mut Option<(f32, Rc<Ui>, Rc<Ui>)>, ui: &Rc<Ui>, s: f32) -> Rc<Ui> {
    if let Some((k, base, scaled)) = cache
        && *k == s
        && Rc::ptr_eq(base, ui)
    {
        return scaled.clone();
    }
    let mut theme = ui.theme.clone();
    let sp = &mut theme.spacing;
    for v in [
        &mut sp.xs,
        &mut sp.sm,
        &mut sp.md,
        &mut sp.lg,
        &mut sp.xl,
        &mut sp.xxl,
    ] {
        *v *= s;
    }
    let r = &mut theme.radii;
    for v in [&mut r.sm, &mut r.md, &mut r.lg] {
        *v *= s;
    }
    let t = &mut theme.text_size;
    for v in [&mut t.sm, &mut t.md, &mut t.lg, &mut t.xl] {
        *v *= s;
    }
    let scaled = Rc::new(ui.with_theme(theme));
    *cache = Some((s, ui.clone(), scaled.clone()));
    scaled
}

/// A heading on the felt: the screen's name and what it is for.
pub fn heading(title: impl Into<SharedString>, body: Option<SharedString>, bx: &Bx) -> Div {
    div()
        .relative()
        .flex()
        .items_end()
        .gap(u(22.))
        .child(
            typo::disp_at(96, 46., 800, 35)
                .on(div())
                .flex_none()
                .child(title.into()),
        )
        .children(body.map(|b| {
            typo::text(16., 400)
                .on(div())
                .flex_1()
                .min_w_0()
                .mb(u(5.))
                .text_color(bx.on_felt_mut)
                .child(b)
        }))
}

/// A screen without a mockup of its own: its heading on the felt and its
/// content on a cream sheet, scrolling under the navigation.
fn sheet_page(
    shell: &mut Shell,
    bx: &Bx,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let wide = matches!(shell.screen, Screen::HandReview | Screen::Review);
    let width = if wide {
        CANVAS_W - 2. * MARGIN
    } else {
        SHEET_W
    };
    let i18n = shell.ui.i18n.clone();
    let subtitle = matches!(
        shell.screen,
        Screen::Calc | Screen::HandReview | Screen::Review | Screen::Settings
    )
    .then(|| SharedString::from(i18n.tr(shell.screen.body_key())));
    let replay = tour_pill(shell, bx, cx).mb(u(4.));
    let head = heading(i18n.tr(shell.screen.nav_key()), subtitle, bx)
        .child(crate::tour::anchor("screen_title"))
        .when(shell.screen == Screen::Lesson, |d| d.child(div().flex_1()))
        .child(replay);

    // What the sheet's padding leaves the screen, in window pixels.
    shell.room_w = (width - 2. * SHEET_PAD) * super::scale();
    // The screens read their sizes from the theme: hand them a scaled one.
    let plain = shell.ui.clone();
    shell.ui = scaled(&mut shell.baralho.scaled, &plain, super::scale());
    let body = shell.screen_body(window, cx);
    shell.ui = plain;

    let sheet = kit::page_sheet(
        bx,
        typo::text(15., 400)
            .on(div())
            .flex()
            .flex_col()
            .p(u(SHEET_PAD))
            .text_color(bx.ink)
            .child(body),
    )
    .w(u(width));

    div()
        .id("page")
        .absolute()
        .top(u(NAV_H))
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .flex_col()
        .items_center()
        .overflow_y_scroll()
        .track_scroll(&shell.scroll)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(u(width))
                .pt(u(20.))
                // The sheet keeps its drop's room below it.
                .pb(u(48. - kit::SHEET_DROP))
                .gap(u(24.))
                .child(head)
                .child(sheet),
        )
        .into_any_element()
}

//! Study mode (D24): the gear on the lessons screen and its popover, the
//! same choice in Settings, and what the styles share to show level tests.
//!
//! Each style draws its own gear button with [`Shell::study_gear`] (its own
//! size and colors) next to "Ver por". The popover is drawn once over the
//! whole window by [`Shell::study_popover`], under the gear's last painted
//! bounds, so it is never clipped or scaled by a style's canvas.

use gpui::{
    AnyElement, Context, Div, FontWeight, Rgba, SharedString, Stateful, Window, div, prelude::*,
    px, svg,
};
use taful_app::fmt;
use taful_app::lessons::{LessonGroup, TestState, UnlockMode};

use crate::Ui;
use crate::screen::Screen;
use crate::shell::Shell;
use crate::theme::alpha;
use crate::widgets::TermTip;

pub const GEAR: &str = "study/gear.svg";
pub const INFO: &str = "study/info.svg";

/// Files shared by every style.
pub const ASSETS: &[(&str, &[u8])] = &[
    (GEAR, include_bytes!("../assets/study/gear.svg")),
    (INFO, include_bytes!("../assets/study/info.svg")),
];

/// The tour anchor that records where the gear is.
const ANCHOR: &str = "study_gear";
/// The popover's width, and its distance below the gear.
const POPOVER_W: f32 = 340.;
const POPOVER_GAP: f32 = 6.;
/// Room kept from the window's edges.
const EDGE: f32 = 12.;

/// How a style draws its gear button.
pub struct GearLook {
    /// The button's side.
    pub size: f32,
    pub radius: f32,
    pub icon: Rgba,
    /// The icon on hover and while the popover is open.
    pub icon_on: Rgba,
    /// Its outline (transparent for none).
    pub ring: Rgba,
    /// Its fill on hover and while open.
    pub fill_on: Rgba,
}

impl Shell {
    /// Changes how lessons unlock and remembers it.
    pub(crate) fn pick_unlock_mode(&mut self, mode: UnlockMode) {
        if let Err(e) = self.app.set_unlock_mode(mode) {
            eprintln!("taful: saving the study mode failed: {e}");
        }
    }

    /// The gear that opens the study-mode popover.
    pub(crate) fn study_gear(&mut self, look: GearLook, cx: &mut Context<Self>) -> Stateful<Div> {
        let open = self.lesson.gear_open;
        let tip = self.ui.i18n.tr("study-mode-gear");
        let (ui, p) = (self.ui.clone(), self.palette);
        div()
            .id("study_gear")
            .group("study_gear")
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(look.size))
            .rounded(px(look.radius))
            .border_1()
            .border_color(look.ring)
            .cursor_pointer()
            .when(open, |d| d.bg(look.fill_on))
            .hover(move |s| s.bg(look.fill_on))
            .when(!open, |d| {
                d.tooltip(move |_, cx| cx.new(|_| TermTip::new(tip.clone(), "", &ui, p)).into())
            })
            .child(crate::tour::anchor(ANCHOR))
            .child(
                svg()
                    .path(GEAR)
                    .size(px((look.size * 0.56).round()))
                    .flex_none()
                    .text_color(if open { look.icon_on } else { look.icon })
                    .group_hover("study_gear", move |s| s.text_color(look.icon_on)),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.lesson.gear_open = !this.lesson.gear_open;
                this.lesson.gear_waits = 0;
                cx.notify();
            }))
    }

    /// The study modes as a list of choices: the popover's body and the
    /// Settings card's. Each row is the mode's name over its one-line
    /// label, with an (i) whose tooltip explains it.
    pub(crate) fn study_mode_rows(&mut self, id: &'static str, cx: &mut Context<Self>) -> Div {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let current = self.app.unlock_mode();
        let mut col = div().flex().flex_col().gap(px(t.spacing.xs));
        for (k, mode) in UnlockMode::ALL.into_iter().enumerate() {
            let on = mode == current;
            let key = mode.label_key();
            let (name, about) = (ui.i18n.tr(key), ui.i18n.tr(&format!("{key}-about")));
            let tip_ui = ui.clone();
            let info = div()
                .id((id, 100 + k))
                .group("study_info")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded(px(12.))
                .tooltip(move |_, cx| {
                    cx.new(|_| TermTip::new(name.clone(), about.clone(), &tip_ui, p))
                        .into()
                })
                .child(
                    svg()
                        .path(INFO)
                        .size(px(15.))
                        .flex_none()
                        .text_color(p.text_muted)
                        .group_hover("study_info", |s| s.text_color(p.accent)),
                );
            let radio = div()
                .flex_none()
                .mt(px(3.))
                .size(px(16.))
                .rounded(px(8.))
                .border(px(1.5))
                .border_color(if on { p.accent } else { p.text_muted })
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| {
                    d.child(div().size(px(8.)).rounded(px(4.)).bg(p.accent))
                });
            col = col.child(
                div()
                    .id((id, k))
                    .flex()
                    .items_start()
                    .gap(px(t.spacing.sm))
                    .px(px(t.spacing.sm))
                    .py(px(t.spacing.sm))
                    .rounded(px(t.radii.md))
                    .border_1()
                    .border_color(if on {
                        alpha(p.accent, 0.5)
                    } else {
                        alpha(p.text, 0.0)
                    })
                    .when(on, |d| d.bg(alpha(p.accent, 0.08)))
                    .when(!on, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(alpha(p.text, 0.05)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.pick_unlock_mode(mode);
                                cx.notify();
                            }))
                    })
                    .child(radio)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(t.text_size.md))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if on { p.accent } else { p.text })
                                    .child(ui.i18n.tr(key)),
                            )
                            .child(
                                div()
                                    .text_size(px(t.text_size.sm))
                                    .line_height(px(t.text_size.sm * 1.4))
                                    .text_color(p.text_muted)
                                    .child(ui.i18n.tr(&format!("{key}-line"))),
                            ),
                    )
                    .child(info),
            );
        }
        col
    }

    /// The study-mode popover over the window, under the gear, while it is
    /// open. A click outside it closes it.
    pub(crate) fn study_popover(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.lesson.gear_open {
            return None;
        }
        if self.screen != Screen::SkillTree {
            self.lesson.gear_open = false;
            return None;
        }
        let Some(gear) = crate::tour::anchor_bounds(ANCHOR) else {
            // The gear may paint this frame; place the popover on the next.
            // If it still does not (the list failed to load), close rather
            // than redraw every frame.
            if self.lesson.gear_waits < 2 {
                self.lesson.gear_waits += 1;
                window.request_animation_frame();
            } else {
                self.lesson.gear_open = false;
            }
            return None;
        };
        self.lesson.gear_waits = 0;
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let w = POPOVER_W.min(vw - 2. * EDGE);
        let right = f32::from(gear.origin.x + gear.size.width);
        let left = (right - w).clamp(EDGE, (vw - w - EDGE).max(EDGE));
        let top = f32::from(gear.origin.y + gear.size.height) + POPOVER_GAP;
        let rows = self.study_mode_rows("study_mode", cx);
        let level = self.start_level_row("study_level_change", cx);
        let card = div()
            .id("study_popover")
            .occlude()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(w))
            .max_h(px((vh - top - EDGE).max(120.)))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(t.spacing.sm))
            .p(px(t.spacing.md))
            .rounded(px(t.radii.lg))
            .bg(p.surface_raised)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .text_color(p.text)
            .child(
                div()
                    .px(px(t.spacing.sm))
                    .text_size(px(t.text_size.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text_muted)
                    .child(ui.i18n.tr("study-mode-title")),
            )
            .child(rows)
            .child(
                div()
                    .px(px(t.spacing.sm))
                    .text_size(px(t.text_size.sm))
                    .line_height(px(t.text_size.sm * 1.4))
                    .text_color(p.text_muted)
                    .child(ui.i18n.tr("study-mode-note")),
            )
            // Where the Jornada starts (D25), changed with the level question.
            .child(div().h(px(1.)).mx(px(t.spacing.sm)).bg(p.border))
            .child(
                div()
                    .px(px(t.spacing.sm))
                    .text_size(px(t.text_size.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text_muted)
                    .child(ui.i18n.tr("study-level-title")),
            )
            .child(level);
        let scrim = div()
            .id("study_scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.lesson.gear_open = false;
                    cx.notify();
                }),
            );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(scrim)
                .child(card)
                .into_any_element(),
        )
    }

    /// Opens a level's test in the lesson reader.
    pub(crate) fn open_level_test(
        &mut self,
        level: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.lesson.current = None;
        self.lesson.open = None;
        self.lesson.quiz = None;
        self.lesson.test = Some(crate::screens::lesson::TestRun::new(level));
        self.input.clear();
        self.scroll_to_top();
        self.select(Screen::Lesson, window, cx);
    }
}

/// What a level group shows about its test, for the styles: the button's
/// label (or the passed tag) and the line under it.
pub struct TestLine {
    pub state: TestState,
    /// The button label (`tree-test-take`) or the passed tag
    /// (`tree-test-passed`).
    pub action: String,
    /// Questions and pass mark (`tree-test-meta`).
    pub meta: String,
    /// The test is the next thing to do (nothing left to read in Prova).
    pub suggested: bool,
}

/// The test line of a level group, if it has a test.
pub fn test_line(ui: &Ui, group: &LessonGroup, next_test: Option<&str>) -> Option<TestLine> {
    let test = group.test.as_ref()?;
    let i18n = &ui.i18n;
    let pass = format!("{}%", fmt::number(test.pass * 100.0, 0, i18n.locale()));
    Some(TestLine {
        state: test.state,
        action: i18n.tr(match test.state {
            TestState::Passed => "tree-test-passed",
            _ => "tree-test-take",
        }),
        meta: i18n.tr_args(
            "tree-test-meta",
            &[("questions", test.questions.into()), ("pass", pass.into())],
        ),
        suggested: next_test == Some(group.id.as_str()),
    })
}

/// The test that opens a level Prova keeps closed (`tree-opens-with`).
pub fn opens_with_text(ui: &Ui, group: &LessonGroup) -> Option<SharedString> {
    let level = group.opens_with.as_ref()?;
    let name = ui.i18n.tr(&format!("level-{level}"));
    Some(
        ui.i18n
            .tr_args("tree-opens-with", &[("level", name.into())])
            .into(),
    )
}

/// The test to take when Prova has nothing left to read
/// (`tree-next-test`).
pub fn next_test_text(ui: &Ui, level: &str) -> String {
    let name = ui.i18n.tr(&format!("level-{level}"));
    ui.i18n.tr_args("tree-next-test", &[("level", name.into())])
}

/// What a home says with no lesson to take: the test that opens the next
/// level, or that every lesson is done.
pub fn idle_text(ui: &Ui, next_test: Option<&str>) -> String {
    match next_test {
        Some(level) => {
            let name = ui.i18n.tr(&format!("level-{level}"));
            ui.i18n
                .tr_args("home-lesson-test", &[("level", name.into())])
        }
        None => ui.i18n.tr("home-lesson-none"),
    }
}

/// The test of `level` opens on click (for the styles' buttons).
pub fn on_test_click(
    level: String,
    cx: &mut Context<Shell>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(move |this: &mut Shell, _: &gpui::ClickEvent, window, cx| {
        cx.stop_propagation();
        this.open_level_test(&level, window, cx);
    })
}

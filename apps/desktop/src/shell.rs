//! App shell: navigation rail + the current screen, the window-wide key
//! handler (one focus target feeds the answer box of whichever screen is
//! up), and the background ticks (live timer, backup-by-use).

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, Context, Entity, FocusHandle, FontWeight, KeyDownEvent, ScrollHandle, SharedString,
    Subscription, Task, Window, div, prelude::*, px,
};
use taful_app::App;

use crate::Ui;
use crate::screen::Screen;
use crate::screens::hand_review::HandReviewView;
use crate::screens::table::{TableEvent, TableScreen};
use crate::screens::{
    calc::CalcView, lesson::LessonView, review::ReviewView, settings::SettingsView,
};
use crate::theme::Palette;

pub(crate) const NAV_WIDTH: f32 = 232.;
pub const CONTENT_MAX_WIDTH: f32 = 960.;
const SELECTION_BAR: f32 = 3.;
/// How often the live timer repaints while an exercise is running.
const TICK: Duration = Duration::from_millis(250);
/// App use is added to the backup clock in steps of this size.
const USE_STEP: Duration = Duration::from_secs(60);

/// What a key did to the answer box.
pub enum KeyResult {
    Edited,
    Submit,
    Escape,
    Ignored,
}

pub struct Shell {
    pub(crate) ui: Rc<Ui>,
    pub(crate) palette: Palette,
    pub(crate) app: App,
    pub(crate) screen: Screen,
    pub(crate) focus: FocusHandle,
    /// The content pane's scroll position (reset on every screen change).
    scroll: ScrollHandle,
    /// The answer being typed (calc, numeric quiz questions, reviews).
    pub(crate) input: String,
    pub(crate) lesson: LessonView,
    pub(crate) calc: CalcView,
    pub(crate) review: ReviewView,
    pub(crate) settings: SettingsView,
    pub(crate) hand_review: HandReviewView,
    pub(crate) table: Entity<TableScreen>,
    /// The home screen's leaks, read again each time home is opened.
    pub(crate) leaks: Option<Vec<taful_app::model::Leak>>,
    /// The guided tour on screen: which screen's, and the step shown.
    tour: Option<(Screen, usize)>,
    /// Screens whose tour was finished or skipped (`tour.seen`).
    pub(crate) tour_seen: String,
    /// Set by Settings' "show tours again" until the screen changes.
    pub(crate) tour_reset_done: bool,
    _ticker: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(ui: Rc<Ui>, mut app: App, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = Palette::new(&ui.theme.colors);
        let focus = cx.focus_handle();
        let trainers = app.trainers().map_err(|e| e.to_string());
        // TAFUL_LAYOUT=<preset> (screenshots) beats the saved layout.
        let layout = std::env::var("TAFUL_LAYOUT")
            .ok()
            .and_then(|id| taful_app::layout::Layout::builtin(&id))
            .unwrap_or_else(|| app.active_layout().1);
        let auto_replay =
            app.string_setting(taful_app::keys::TABLE_REPLAY).as_deref() != Some("off");
        let table = cx.new(|cx| TableScreen::new(ui.clone(), trainers, layout, auto_replay, cx));
        window.focus(&focus);
        let ticker = cx.spawn(async move |this, cx| {
            let mut since_use = Duration::ZERO;
            loop {
                cx.background_executor().timer(TICK).await;
                since_use += TICK;
                let add_use = since_use >= USE_STEP;
                if add_use {
                    since_use = Duration::ZERO;
                }
                let alive = this.update(cx, |shell, cx| {
                    if add_use && let Err(e) = shell.app.note_use(USE_STEP.as_millis() as i64) {
                        eprintln!("taful: backup failed: {e}");
                    }
                    if shell.timer_running() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let subscriptions = vec![
            cx.subscribe_in(&table, window, Self::on_table_event),
            cx.on_release(|shell, _| shell.exit_backup()),
            cx.on_app_quit(|shell, _| {
                shell.exit_backup();
                async {}
            }),
        ];
        let mut shell = Self {
            ui,
            palette,
            app,
            // Dev override (and CI screenshots): TAFUL_SCREEN=table opens a screen.
            screen: std::env::var("TAFUL_SCREEN")
                .ok()
                .and_then(|id| Screen::from_id(&id))
                .unwrap_or(Screen::Home),
            focus,
            scroll: ScrollHandle::new(),
            input: String::new(),
            lesson: LessonView::default(),
            calc: CalcView::default(),
            review: ReviewView::default(),
            settings: SettingsView::default(),
            hand_review: HandReviewView::default(),
            leaks: None,
            tour: None,
            tour_seen: String::new(),
            tour_reset_done: false,
            table,
            _ticker: ticker,
            _subscriptions: subscriptions,
        };
        shell.tour_seen = shell
            .app
            .string_setting(taful_app::keys::TOUR_SEEN)
            .unwrap_or_default();
        shell.maybe_start_tour();
        // Dev override (screenshots): TAFUL_REVIEW_START=1 deals the first review.
        if shell.screen == Screen::Review && std::env::var_os("TAFUL_REVIEW_START").is_some() {
            shell.review_load_next(cx);
        }
        shell
    }

    /// Stores what the table graded (spec §14) and opens the hand review.
    fn on_table_event(
        &mut self,
        _: &Entity<TableScreen>,
        ev: &TableEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ev {
            TableEvent::Answered(d, ctx) => {
                if let Err(e) = self.app.record_spot_decision(d, ctx, None) {
                    eprintln!("taful: storing a decision failed: {e}");
                }
            }
            TableEvent::HandOver(h) => match self.app.record_hand(h) {
                Ok(id) => {
                    self.hand_review.last_stored = Some(id);
                    self.hand_review.loaded = false;
                }
                Err(e) => eprintln!("taful: storing a hand failed: {e}"),
            },
            TableEvent::SaveLayout(draft) => {
                let i18n = &self.ui.i18n;
                let name = self.app.new_layout_copy_name(|n| {
                    i18n.tr_args("settings-layout-copy-name", &[("n", n.into())])
                });
                let r = match self.app.save_edited_layout(draft, &name) {
                    Ok((_, layout)) => Ok(layout),
                    Err(e) => {
                        eprintln!("taful: saving the edited layout failed: {e}");
                        Err(i18n.tr("layout-edit-save-failed"))
                    }
                };
                self.table.update(cx, |t, cx| t.layout_saved(r, cx));
            }
            TableEvent::AutoReplay(on) => {
                let value = if *on { "on" } else { "off" };
                if let Err(e) = self
                    .app
                    .set_string_setting(taful_app::keys::TABLE_REPLAY, value)
                {
                    eprintln!("taful: storing the replay setting failed: {e}");
                }
            }
            TableEvent::ReviewHand => {
                let id = self.hand_review.last_stored.clone();
                self.select(Screen::HandReview, window, cx);
                if let Some(id) = id {
                    self.open_hand(&id);
                }
            }
        }
        cx.notify();
    }

    fn exit_backup(&mut self) {
        if let Err(e) = self.app.on_exit() {
            eprintln!("taful: backup on exit failed: {e}");
        }
    }

    /// Whether a timed exercise is on screen (the timer needs repaints).
    fn timer_running(&self) -> bool {
        match self.screen {
            Screen::Calc => self.calc.running(),
            Screen::Review => self.review.calc_running(),
            _ => false,
        }
    }

    pub(crate) fn select(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen != screen {
            self.tour_reset_done = false;
            // Leaving mid-tour counts as skipping it (before calc_leave,
            // which would otherwise be undone by the tour's resume).
            self.tour_end();
            if self.screen == Screen::Calc {
                self.calc_leave();
            }
            self.screen = screen;
            crate::tour::clear_anchors();
            self.input.clear();
            self.scroll_to_top();
            if screen == Screen::Review {
                self.review_enter();
            }
            if screen == Screen::Calc {
                self.calc_enter();
            }
            if screen == Screen::HandReview {
                self.hand_review_enter();
            }
            self.maybe_start_tour();
            if screen == Screen::Home {
                self.leaks = None;
            }
        }
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn scroll_to_top(&self) {
        self.scroll.set_offset(gpui::point(px(0.), px(0.)));
    }

    /// Edits the shared answer box.
    fn edit_input(&mut self, ev: &KeyDownEvent) -> KeyResult {
        let ks = &ev.keystroke;
        if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
            return KeyResult::Ignored;
        }
        match ks.key.as_str() {
            "enter" => return KeyResult::Submit,
            "escape" => return KeyResult::Escape,
            "backspace" => {
                self.input.pop();
                return KeyResult::Edited;
            }
            _ => {}
        }
        let Some(ch) = ks.key_char.as_deref().and_then(|s| s.chars().next()) else {
            return KeyResult::Ignored;
        };
        let allowed = ch.is_ascii_digit()
            || (matches!(ch, '.' | ',') && !self.input.contains(['.', ',']))
            || (ch == '-' && self.input.is_empty());
        if allowed && self.input.len() < 12 {
            self.input.push(ch);
            KeyResult::Edited
        } else {
            KeyResult::Ignored
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.tour.is_some() {
            // The tour owns the keyboard: no answer is typed behind it.
            match ev.keystroke.key.as_str() {
                "escape" => self.tour_end(),
                "enter" => self.tour_next(cx),
                _ => {}
            }
            cx.notify();
            return;
        }
        if self.screen == Screen::Settings {
            // The theme editor's hex and name fields.
            self.theme_editor_key(ev, cx);
            cx.notify();
            return;
        }
        let result = self.edit_input(ev);
        match self.screen {
            Screen::Calc => self.calc_key(result, cx),
            Screen::Review => self.review_key(result, cx),
            Screen::Lesson => self.quiz_key(result, cx),
            _ => {}
        }
        let _ = window;
        cx.notify();
    }

    /// Re-reads strings and tokens after a locale or theme change.
    pub(crate) fn set_ui(&mut self, ui: Rc<Ui>, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = Palette::new(&ui.theme.colors);
        window.set_window_title(&ui.i18n.tr("window-title"));
        let table_ui = ui.clone();
        self.table.update(cx, |table, cx| {
            table.set_ui(table_ui);
            cx.notify();
        });
        self.ui = ui;
        // Rendered lesson text and quiz prompts are per locale.
        self.lesson = LessonView::default();
        self.review = ReviewView::default();
        // A status line from before the switch is in the old language.
        self.settings.message = None;
    }

    fn nav(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let due = self.app.due_count().unwrap_or(0);
        // The lesson reader lives under the lessons item.
        let current = if self.screen == Screen::Lesson {
            Screen::SkillTree
        } else {
            self.screen
        };
        let item = |i: usize, screen: Screen, cx: &mut Context<Self>| {
            let selected = screen == current;
            let label: SharedString = i18n.tr(screen.nav_key()).into();
            div()
                .id(("nav", i))
                .flex()
                .items_center()
                .justify_between()
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
                .on_click(cx.listener(move |this, _, window, cx| this.select(screen, window, cx)))
                .child(label)
                .when(screen == Screen::Review && due > 0, |d| {
                    d.child(
                        div()
                            .px(px(t.spacing.sm))
                            .rounded(px(t.radii.pill))
                            .bg(p.accent)
                            .text_color(p.on_accent)
                            .text_size(px(t.text_size.sm))
                            .child(due.to_string()),
                    )
                })
        };
        let mut n = 0;
        let mut items = div().flex().flex_col().gap(px(t.spacing.lg));
        for (heading, screens) in crate::screen::NAV_GROUPS {
            let mut g = div().flex().flex_col().gap(px(t.spacing.xs));
            if let Some(h) = heading {
                g = g.child(
                    div()
                        .px(px(t.spacing.md))
                        .pb(px(t.spacing.xs))
                        .text_size(px(t.text_size.sm))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text_muted)
                        .child(i18n.tr(h).to_uppercase()),
                );
            }
            for &screen in screens {
                g = g.child(item(n, screen, cx));
                n += 1;
            }
            items = items.child(g);
        }
        let settings = item(n, crate::screen::NAV_BOTTOM, cx);

        div()
            .relative()
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
            .child(crate::tour::anchor("nav"))
            .child(items)
            .child(div().flex_1())
            .child(settings)
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
            .into_any_element()
    }

    /// The "coming in Mx" card for screens later milestones build.
    pub(crate) fn placeholder(&self, screen: Screen) -> AnyElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        crate::widgets::card(&self.ui, p)
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
            )
            .into_any_element()
    }

    /// Opens the current screen's tour on its first visit.
    /// `TAFUL_TOUR=0` (screenshots, CI) never opens one on its own.
    fn maybe_start_tour(&mut self) {
        if std::env::var("TAFUL_TOUR").as_deref() == Ok("0") {
            return;
        }
        if !crate::tour::is_seen(&self.tour_seen, self.screen) {
            self.tour_start();
        }
    }

    /// Opens the current screen's tour at its first step. A timed
    /// question's clock is paused while the tour is open.
    fn tour_start(&mut self) {
        self.tour = Some((self.screen, 0));
        if self.screen == Screen::Calc {
            self.calc_leave();
        }
    }

    fn tour_next(&mut self, cx: &mut Context<Self>) {
        if let Some((screen, i)) = self.tour {
            if i + 1 < crate::tour::steps(screen).len() {
                self.tour = Some((screen, i + 1));
            } else {
                self.tour_end();
            }
        }
        cx.notify();
    }

    /// Closes the tour and remembers the screen as seen.
    fn tour_end(&mut self) {
        let Some((screen, _)) = self.tour.take() else {
            return;
        };
        if screen == Screen::Calc && self.screen == Screen::Calc {
            self.calc_enter();
        }
        self.tour_seen = crate::tour::with_seen(&self.tour_seen, screen);
        if let Err(e) = self
            .app
            .set_string_setting(taful_app::keys::TOUR_SEEN, &self.tour_seen)
        {
            eprintln!("taful: saving the tour state failed: {e}");
        }
    }

    /// Forgets every seen tour; each screen shows its tour again.
    pub(crate) fn tour_reset(&mut self) {
        self.tour_seen.clear();
        self.tour_reset_done = true;
        if let Err(e) = self.app.set_string_setting(taful_app::keys::TOUR_SEEN, "") {
            eprintln!("taful: resetting the tours failed: {e}");
        }
    }

    /// The coach mark over the anchored element (or centered when the
    /// anchor is not on screen, e.g. a quiz that is already open).
    fn tour_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        const CARD_W: f32 = 360.;
        const GAP: f32 = 12.;
        // Room kept for the card (its height is known only after layout).
        const CARD_H: f32 = 260.;
        let (screen, i) = self.tour?;
        let steps = crate::tour::steps(screen);
        let step = steps.get(i)?;
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let bounds = crate::tour::anchor_bounds(step.anchor);
        if bounds.is_none() && crate::tour::retry_once(screen, i) {
            // The anchor may paint this frame; point at it on the next one.
            window.request_animation_frame();
        }
        let last = i + 1 == steps.len();
        let card = div()
            .id("tour_card")
            .occlude()
            .absolute()
            .w(px(CARD_W))
            .flex()
            .flex_col()
            .gap(px(t.spacing.sm))
            .p(px(t.spacing.lg))
            .rounded(px(t.radii.lg))
            .bg(p.surface_raised)
            .border_1()
            .border_color(p.accent)
            .shadow_lg()
            .child(crate::widgets::eyebrow(
                i18n.tr_args(
                    "tour-step",
                    &[("n", (i + 1).into()), ("total", steps.len().into())],
                ),
                &self.ui,
                p,
            ))
            .child(crate::widgets::title(i18n.tr(step.title), &self.ui))
            .child(
                div()
                    .text_size(px(t.text_size.md))
                    .line_height(px(t.text_size.md * 1.5))
                    .child(i18n.tr(step.body)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(t.spacing.sm))
                    .child(
                        div()
                            .id("tour_skip")
                            .text_size(px(t.text_size.sm))
                            .text_color(p.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.text))
                            .when(!last, |d| d.child(i18n.tr("tour-skip")))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tour_end();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::widgets::button(
                            "tour_next",
                            i18n.tr(if last { "tour-done" } else { "tour-next" }),
                            true,
                            &self.ui,
                            p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.tour_next(cx))),
                    ),
            );
        let Some(b) = bounds else {
            return Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(card.relative())
                    .into_any_element(),
            );
        };
        let (bx, by) = (f32::from(b.origin.x), f32::from(b.origin.y));
        let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
        let ring = div()
            .absolute()
            .left(px(bx - 4.))
            .top(px(by - 4.))
            .w(px(bw + 8.))
            .h(px(bh + 8.))
            .rounded(px(t.radii.lg))
            .border_2()
            .border_color(p.accent);
        let clamp_x = |x: f32| x.clamp(GAP, (vw - CARD_W - GAP).max(GAP));
        // The card always stays inside the window, even when the anchor
        // is scrolled out of view (e.g. the quiz button under a long text).
        let clamp_y = |y: f32| y.clamp(GAP, (vh - CARD_H - GAP).max(GAP));
        // Tall anchors (nav, felt): beside them; others: below, or above
        // when there is no room below.
        // `None`: above the anchor, the card's bottom edge on its top.
        let (x, top) = if bh > vh * 0.5 && bx + bw + GAP + CARD_W < vw {
            (bx + bw + GAP, Some(by + 40.))
        } else if bh > vh * 0.5 && bx > CARD_W + GAP {
            (bx - CARD_W - GAP, Some(by + 40.))
        } else if by + bh + GAP + CARD_H < vh {
            (clamp_x(bx), Some(by + bh + GAP))
        } else if by - GAP - CARD_H >= GAP && by <= vh {
            (clamp_x(bx), None)
        } else {
            (clamp_x(bx), Some(by - GAP - CARD_H))
        };
        let card = match top {
            Some(y) => card.left(px(x)).top(px(clamp_y(y))),
            None => card.left(px(x)).bottom(px(vh - by + GAP)),
        };
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(ring)
                .child(card)
                .into_any_element(),
        )
    }

    /// Screen title plus a one-line "what is this screen" subtitle.
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let (p, t, i18n) = (&self.palette, &self.ui.theme, &self.ui.i18n);
        let subtitle = matches!(
            self.screen,
            Screen::Home
                | Screen::SkillTree
                | Screen::Calc
                | Screen::HandReview
                | Screen::Review
                | Screen::Settings
        );
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(t.spacing.xs))
            .child(crate::tour::anchor("screen_title"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(t.spacing.md))
                    .child(
                        div()
                            .text_size(px(t.text_size.xl))
                            .font_weight(FontWeight::BOLD)
                            .child(i18n.tr(self.screen.nav_key())),
                    )
                    .child(
                        div()
                            .id("tour_replay")
                            .px(px(t.spacing.sm))
                            .rounded(px(t.radii.pill))
                            .border_1()
                            .border_color(p.border)
                            .text_size(px(t.text_size.sm))
                            .text_color(p.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.border_color(p.accent).text_color(p.text))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tour_start();
                                cx.notify();
                            }))
                            .child(format!("? {}", i18n.tr("tour-replay"))),
                    ),
            )
            .when(subtitle, |d| {
                d.child(
                    div()
                        .text_size(px(t.text_size.md))
                        .text_color(p.text_muted)
                        .child(i18n.tr(self.screen.body_key())),
                )
            })
            .into_any_element()
    }

    fn content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = &self.ui.theme;
        let (xl, xxl) = (t.spacing.xl, t.spacing.xxl);
        if matches!(
            self.screen,
            Screen::Table | Screen::HandReview | Screen::Review
        ) {
            let gap = t.spacing.lg;
            // Screens with a table take the whole pane (no reading column).
            let body = match self.screen {
                Screen::Table => self.table.clone().into_any_element(),
                Screen::HandReview => self.hand_review_view(cx),
                _ => self.review_view(cx),
            };
            return div()
                .id("content_wide")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .gap(px(gap))
                        .p(px(xxl))
                        .child(self.header(cx))
                        .child(body),
                )
                .into_any_element();
        }
        let viewport = f32::from(window.viewport_size().width);
        let column_width = (viewport - NAV_WIDTH).clamp(0., CONTENT_MAX_WIDTH + 2. * xxl);
        let body = match self.screen {
            Screen::Home => self.home_view(cx),
            Screen::SkillTree => self.tree_view(cx),
            Screen::Lesson => self.lesson_view(cx),
            Screen::Calc => self.calc_view(cx),
            Screen::Settings => self.settings_view(window, cx),
            Screen::Table | Screen::HandReview | Screen::Review => self.placeholder(self.screen),
        };
        div()
            .id("content")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(xl))
                    // A definite width: text measured under `w_full` + `max_w`
                    // wraps at the unclamped width and the column comes out
                    // too short to scroll to its end.
                    .w(px(column_width))
                    .p(px(xxl))
                    .child(self.header(cx))
                    .child(body),
            )
            .into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::tour::begin_frame();
        let content = self.content(window, cx);
        let nav = self.nav(cx);
        let tour = self.tour_overlay(window, cx);
        div()
            .relative()
            .size_full()
            .flex()
            .bg(self.palette.background)
            .text_color(self.palette.text)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(nav)
            .child(content)
            .children(tour)
    }
}

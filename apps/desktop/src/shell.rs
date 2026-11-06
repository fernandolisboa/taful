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
pub const CONTENT_MAX_WIDTH: f32 = 760.;
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
        let table = cx.new(|cx| TableScreen::new(ui.clone(), trainers, layout, cx));
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
            table,
            _ticker: ticker,
            _subscriptions: subscriptions,
        };
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
            self.screen = screen;
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
        let items = Screen::ALL.into_iter().enumerate().map(|(i, screen)| {
            let selected = screen == self.screen;
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

    fn content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = &self.ui.theme;
        let (xl, xxl) = (t.spacing.xl, t.spacing.xxl);
        let title = self.ui.i18n.tr(self.screen.nav_key());
        let title_size = t.text_size.xl;
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
                        .child(
                            div()
                                .text_size(px(title_size))
                                .font_weight(FontWeight::BOLD)
                                .child(title),
                        )
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
                    .child(
                        div()
                            .text_size(px(title_size))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(body),
            )
            .into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.content(window, cx);
        let nav = self.nav(cx);
        div()
            .size_full()
            .flex()
            .bg(self.palette.background)
            .text_color(self.palette.text)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(nav)
            .child(content)
    }
}

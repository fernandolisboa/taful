//! The level question (D25): "Qual é o seu nível no poker?".
//!
//! On a first run (no level stored, no lesson progress) the app asks the
//! user's level before anything else. Each level of the Jornada is one row
//! with a plain line ("Nunca joguei", "Sei as regras"…); picking one opens
//! every level up to it, and "Pular" starts at the first level. Nothing is
//! marked done or passed, and no test is shown. The same card opens later
//! from the lessons screen's gear (and Settings › Estudo) to change the
//! level, with "Cancelar" in place of "Pular".
//!
//! The card is laid out here, once, and floats over the style's frame like
//! the tour. Each style dresses it through its `level_look` hook (see
//! [`Look`]): its panel, type, row colors and the mark it gives levels
//! elsewhere (Salão's medallions, Baralho's suits, Cockpit's key caps).

use gpui::{AnyElement, Context, Div, FontWeight, Rgba, SharedString, Window, div, prelude::*, px};
use taful_app::lessons::LevelChoice;

use crate::shell::Shell;
use crate::style::by_style;
use crate::theme::alpha;

/// The card's width, before the window narrows it.
const WIDTH: f32 = 560.;
/// Room kept from the window's edges.
const EDGE: f32 = 16.;
/// Below this window height the rows tighten so the card fits at the
/// minimum window (940 × 600) without scrolling.
const SHORT: f32 = 760.;

/// The question on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelQuestion {
    /// Asked on the first run ("Pular"), or opened to change the level
    /// ("Cancelar", the current level marked).
    pub first_run: bool,
}

/// How a style dresses the card. Text builders get the text and return
/// the styled box; the layout (gaps, rows, order) is shared.
pub struct Look {
    /// The layer that dims the app behind the card.
    pub scrim: Rgba,
    /// The card: the style's panel around `content` (which brings its own
    /// padding and layout).
    pub card: fn(&Shell, Div) -> Div,
    /// The card's inner padding.
    pub pad: f32,
    /// The small label above the title.
    pub eyebrow: fn(&Shell, SharedString) -> Div,
    pub title: fn(&Shell, SharedString) -> Div,
    /// The lines under the title and at the foot (they wrap).
    pub text: fn(&Shell, SharedString) -> Div,
    /// A level's name and its plain line.
    pub name: fn(&Shell, SharedString) -> Div,
    pub about: fn(&Shell, SharedString) -> Div,
    /// The tag at a row's end: "Em breve", or "Atual" (`strong`).
    pub tag: fn(&Shell, SharedString, bool) -> Div,
    /// The mark left of a row, as the style draws levels elsewhere: `n` is
    /// the level's place (0-based), `k` its place among the levels that
    /// have lessons (`None`: none yet, "em breve").
    pub mark: fn(&Shell, usize, Option<usize>) -> AnyElement,
    /// "Pular" / "Cancelar": its text color, and on hover.
    pub link: (Rgba, Rgba),
    pub row: RowLook,
}

/// The rows' shape and colors.
pub struct RowLook {
    pub radius: f32,
    /// Outline and fill at rest, under the pointer, and on the current
    /// level (when changing it).
    pub ring: Rgba,
    pub fill: Rgba,
    pub ring_hot: Rgba,
    pub fill_hot: Rgba,
    pub ring_on: Rgba,
    pub fill_on: Rgba,
}

impl Shell {
    /// Asks the level on the first run (D25). Called once at startup; a
    /// data dir with lesson progress from before is never asked (its level
    /// is stored instead). `TAFUL_LEVEL_QUESTION=0` (CI screenshots) never
    /// asks, `=1` always does (screenshots of the question).
    pub(crate) fn maybe_ask_level(&mut self) {
        let due = match std::env::var("TAFUL_LEVEL_QUESTION").as_deref() {
            Ok("0") => false,
            Ok("1") => true,
            _ => self.app.level_question_due().unwrap_or_else(|e| {
                eprintln!("taful: reading the starting level failed: {e}");
                false
            }),
        };
        if due {
            self.level_question = Some(LevelQuestion { first_run: true });
        }
    }

    /// Opens the question to change the level (the gear, Settings).
    pub(crate) fn change_level(&mut self) {
        self.lesson.gear_open = false;
        // A tour still open on this screen would sit over the card and lose
        // its keys to it: leaving it for the question counts as skipping.
        self.tour_end();
        self.level_question = Some(LevelQuestion { first_run: false });
    }

    /// Closes the question with the user's answer: a level, or `None` for
    /// "Pular" (first run: start at the first level) or "Cancelar".
    pub(crate) fn answer_level(&mut self, level: Option<&str>, cx: &mut Context<Self>) {
        let Some(q) = self.level_question.take() else {
            return;
        };
        let saved = match (level, q.first_run) {
            (Some(level), _) => self.app.set_start_level(level),
            (None, true) => self.app.skip_level_question(),
            (None, false) => Ok(()),
        };
        if let Err(e) = saved {
            eprintln!("taful: saving the starting level failed: {e}");
        }
        // Screens that keep their numbers for a visit (Salão's and Baralho's
        // homes) read the new next lesson; the screen fades in again.
        if level.is_some() || q.first_run {
            self.screen_visit += 1;
        }
        // The screen's tour waited for the answer.
        if q.first_run {
            self.maybe_start_tour();
        }
        cx.notify();
    }

    /// The question's keys: 1–6 pick a level, Escape skips or cancels.
    /// `true` when the question is open (it owns the keyboard).
    pub(crate) fn level_question_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.level_question.is_none() {
            return false;
        }
        if key == "escape" {
            self.answer_level(None, cx);
        } else if let Some(n) = key.parse::<usize>().ok().filter(|n| *n >= 1)
            && let Some(choice) = self.app.level_choices().get(n - 1)
        {
            let id = choice.id.clone();
            self.answer_level(Some(&id), cx);
        }
        true
    }

    /// "Seu nível": where the Jornada starts, with a "Mudar" link that
    /// opens the question. The gear popover's and Settings' row.
    pub(crate) fn start_level_row(&mut self, id: &'static str, cx: &mut Context<Self>) -> Div {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let level = self.app.start_level().unwrap_or_default();
        let name = ui.i18n.tr(&format!("level-{level}"));
        div()
            .flex()
            .items_center()
            .gap(px(t.spacing.sm))
            .px(px(t.spacing.sm))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(t.text_size.sm))
                    .line_height(px(t.text_size.sm * 1.4))
                    .text_color(p.text_muted)
                    .child(
                        ui.i18n
                            .tr_args("study-level-line", &[("level", name.into())]),
                    ),
            )
            .child(
                div()
                    .id(id)
                    .flex_none()
                    .px(px(t.spacing.sm))
                    .py(px(2.))
                    .rounded(px(t.radii.sm))
                    .text_size(px(t.text_size.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.accent)
                    .cursor_pointer()
                    .hover(|s| s.bg(alpha(p.accent, 0.1)))
                    .child(ui.i18n.tr("study-level-change"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.change_level();
                        cx.notify();
                    })),
            )
    }

    /// The question over the window, while it is open.
    pub(crate) fn level_question_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let q = self.level_question?;
        let look: Look = by_style!(self.ui.style, level_look(self));
        let ui = self.ui.clone();
        let i18n = &ui.i18n;
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let short = vh < SHORT;
        let width = WIDTH.min(vw - 2. * EDGE);
        let current = if q.first_run {
            None
        } else {
            self.app.start_level().ok()
        };
        let choices = self.app.level_choices();

        let mut rows = div().flex().flex_col().gap(px(if short { 4. } else { 6. }));
        let mut k = 0;
        for (n, choice) in choices.iter().enumerate() {
            let place = (choice.lessons > 0).then(|| {
                k += 1;
                k - 1
            });
            rows =
                rows.child(self.level_row(&look, n, place, choice, current.as_deref(), short, cx));
        }

        let head = div()
            .flex()
            .flex_col()
            .gap(px(if short { 4. } else { 6. }))
            .child((look.eyebrow)(
                self,
                i18n.tr(if q.first_run {
                    "level-question-welcome"
                } else {
                    "level-question-change"
                })
                .into(),
            ))
            .child((look.title)(self, i18n.tr("level-question-title").into()))
            .child((look.text)(self, i18n.tr("level-question-lead").into()));

        let (link, link_hot) = look.link;
        let leave = div()
            .id("level_question_leave")
            .flex_none()
            .px(px(10.))
            .py(px(6.))
            .text_color(link)
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(move |s| s.text_color(link_hot))
            .child(i18n.tr(if q.first_run {
                "level-question-skip"
            } else {
                "level-question-cancel"
            }))
            .on_click(cx.listener(|this, _, _, cx| this.answer_level(None, cx)));
        let foot = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                (look.text)(
                    self,
                    if q.first_run {
                        let first = choices.first().map(|c| c.id.as_str()).unwrap_or_default();
                        let name = i18n.tr(&format!("level-{first}"));
                        i18n.tr_args("level-question-note", &[("level", name.into())])
                    } else {
                        i18n.tr("level-question-change-note")
                    }
                    .into(),
                )
                .flex_1()
                .min_w_0(),
            )
            .child(leave);

        let pad = if short { look.pad * 0.75 } else { look.pad };
        let gap = if short { 12. } else { 18. };
        let content = div()
            .flex()
            .flex_col()
            .gap(px(gap))
            .p(px(pad))
            .child(head)
            .child(rows)
            .child(foot);
        let card = (look.card)(self, content)
            .id("level_question")
            .occlude()
            .w(px(width))
            .flex_shrink()
            .min_h_0()
            .max_h(px(vh - crate::caption::HEIGHT - 2. * EDGE))
            .overflow_y_scroll();
        let scrim = div()
            .id("level_question_scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(look.scrim);
        Some(
            div()
                .absolute()
                .top(px(crate::caption::HEIGHT))
                .left_0()
                .right_0()
                .bottom_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(scrim)
                .child(crate::motion::fade_in(card, "level_question_fade"))
                .into_any_element(),
        )
    }

    /// One level: its mark, name and plain line, and a tag at the end.
    #[allow(clippy::too_many_arguments)]
    fn level_row(
        &self,
        look: &Look,
        n: usize,
        place: Option<usize>,
        choice: &LevelChoice,
        current: Option<&str>,
        short: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = &self.ui.i18n;
        let soon = choice.lessons == 0;
        let on = current == Some(choice.id.as_str());
        let r = &look.row;
        let (ring, fill) = if on {
            (r.ring_on, r.fill_on)
        } else {
            (r.ring, r.fill)
        };
        let (ring_hot, fill_hot) = (r.ring_hot, r.fill_hot);
        let tag = if on {
            Some((look.tag)(
                self,
                i18n.tr("level-question-current").into(),
                true,
            ))
        } else if soon {
            Some((look.tag)(self, i18n.tr("tree-soon").into(), false))
        } else {
            None
        };
        let id = choice.id.clone();
        div()
            .id(("level_choice", n))
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(14.))
            .py(px(if short { 7. } else { 10. }))
            .rounded(px(r.radius))
            .border_1()
            .border_color(ring)
            .bg(fill)
            .cursor_pointer()
            .hover(move |s| s.border_color(ring_hot).bg(fill_hot))
            .child((look.mark)(self, n, place))
            // A short window puts the plain line beside the name.
            .child(
                div()
                    .flex()
                    .when(short, |d| d.items_center().gap(px(10.)))
                    .when(!short, |d| d.flex_col())
                    .flex_1()
                    .min_w_0()
                    .child(
                        (look.name)(self, i18n.tr(&format!("level-{}", choice.id)).into())
                            .flex_none(),
                    )
                    .child(
                        (look.about)(self, i18n.tr(&format!("level-{}-about", choice.id)).into())
                            .when(short, |d| d.flex_1().min_w_0().truncate()),
                    ),
            )
            .children(tag)
            .on_click(cx.listener(move |this, _, _, cx| this.answer_level(Some(&id), cx)))
            .into_any_element()
    }
}

// ---- Classic ---------------------------------------------------------------

/// Classic's look: the theme's raised surface, its accent and type scale.
pub fn classic_look(shell: &Shell) -> Look {
    let p = shell.palette;
    Look {
        scrim: alpha(p.background, 0.82),
        card: |s, content| {
            let (p, t) = (s.palette, &s.ui.theme);
            content
                .rounded(px(t.radii.lg))
                .bg(p.surface_raised)
                .border_1()
                .border_color(p.border)
                .shadow_lg()
                .text_color(p.text)
        },
        pad: shell.ui.theme.spacing.xl,
        eyebrow: |s, text| crate::widgets::eyebrow(text, &s.ui, &s.palette),
        title: |s, text| {
            div()
                .text_size(px(s.ui.theme.text_size.xl))
                .font_weight(FontWeight::BOLD)
                .child(text)
        },
        text: |s, text| {
            let t = &s.ui.theme;
            div()
                .text_size(px(t.text_size.md))
                .line_height(px(t.text_size.md * 1.45))
                .text_color(s.palette.text_muted)
                .child(text)
        },
        name: |s, text| {
            div()
                .text_size(px(s.ui.theme.text_size.md))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        },
        about: |s, text| {
            div()
                .text_size(px(s.ui.theme.text_size.sm))
                .text_color(s.palette.text_muted)
                .child(text)
        },
        tag: |s, text, strong| {
            let p = s.palette;
            div()
                .flex_none()
                .px(px(8.))
                .py(px(2.))
                .rounded(px(s.ui.theme.radii.sm))
                .text_size(px(s.ui.theme.text_size.sm))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if strong { p.on_accent } else { p.text_muted })
                .bg(if strong {
                    p.accent
                } else {
                    alpha(p.text, 0.08)
                })
                .child(text)
        },
        mark: |s, n, place| {
            let (p, soon) = (s.palette, place.is_none());
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(32.))
                .rounded(px(16.))
                .border(px(1.5))
                .border_color(if soon { alpha(p.text, 0.2) } else { p.accent })
                .text_color(if soon { p.text_muted } else { p.accent })
                .text_size(px(s.ui.theme.text_size.sm))
                .font_weight(FontWeight::BOLD)
                .child((n + 1).to_string())
                .into_any_element()
        },
        link: (p.text_muted, p.text),
        row: RowLook {
            radius: shell.ui.theme.radii.md,
            ring: alpha(p.text, 0.0),
            fill: alpha(p.text, 0.03),
            ring_hot: alpha(p.accent, 0.6),
            fill_hot: alpha(p.accent, 0.08),
            ring_on: alpha(p.accent, 0.5),
            fill_on: alpha(p.accent, 0.12),
        },
    }
}

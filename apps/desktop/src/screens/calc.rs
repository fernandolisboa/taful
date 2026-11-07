//! Calc trainer (spec §11, §15.1 item 6): exercise picker, timed prompt,
//! result with the shortcut next to the exact value.

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::i18n::Locale;
use taful_app::model::{CalcOutcome, CalcRound, Exercise, ExerciseKind, Prompt, Rating, Unit};

use crate::Ui;
use crate::shell::{KeyResult, Shell};
use crate::theme::Palette;
use crate::widgets::{answer_box, button, card, chip, muted};

#[derive(Default)]
pub struct CalcView {
    /// `None` = mixed.
    pub kind: Option<ExerciseKind>,
    pub round: Option<CalcRound>,
    pub outcome: Option<CalcOutcome>,
    /// When the user left the screen mid-question: the clock is paused
    /// until they come back.
    pub left_at: Option<i64>,
}

impl CalcView {
    pub fn running(&self) -> bool {
        self.round.is_some() && self.outcome.is_none()
    }
}

/// Decimals shown for an answer in `unit`.
fn decimals(unit: Unit) -> u8 {
    match unit {
        Unit::Count => 0,
        Unit::Percent | Unit::Ratio => 1,
    }
}

/// A value in the exercise's unit, formatted.
pub fn value_text(x: f64, unit: Unit, locale: Locale) -> String {
    let n = fmt::number(x, decimals(unit), locale);
    match unit {
        Unit::Percent => format!("{n}%"),
        _ => n,
    }
}

/// The prompt sentence of an exercise.
pub fn prompt_text(e: &Exercise, ui: &Ui) -> String {
    let loc = ui.i18n.locale();
    let n = |x: f64| fmt::number_trim(x, 1, loc);
    let key = format!("calc-prompt-{}", e.kind.id());
    match &e.prompt {
        Prompt::Outs {
            hero,
            villain,
            board,
        } => ui.i18n.tr_args(
            &key,
            &[
                ("hero", fmt::cards(&hero.to_string()).into()),
                ("villain", fmt::cards(&villain.to_string()).into()),
                ("board", fmt::cards(board).into()),
            ],
        ),
        Prompt::RuleOf2And4 {
            outs,
            cards_to_come,
        } => ui.i18n.tr_args(
            &key,
            &[("outs", (*outs).into()), ("cards", (*cards_to_come).into())],
        ),
        Prompt::PotOdds { pot_bb, bet_bb } => ui.i18n.tr_args(
            &key,
            &[("pot", n(*pot_bb).into()), ("bet", n(*bet_bb).into())],
        ),
        Prompt::Mdf { bet_pct } | Prompt::Alpha { bet_pct } => ui
            .i18n
            .tr_args(&key, &[("pct", bet_pct.to_string().into())]),
        Prompt::Spr { stack_bb, pot_bb } => ui.i18n.tr_args(
            &key,
            &[("stack", n(*stack_bb).into()), ("pot", n(*pot_bb).into())],
        ),
        Prompt::EquityVsRange {
            hero,
            board,
            villain,
        } => ui.i18n.tr_args(
            &key,
            &[
                ("hero", fmt::cards(&hero.to_string()).into()),
                ("board", fmt::cards(board).into()),
                ("range", fmt::range(villain).into()),
            ],
        ),
    }
}

pub fn rating_key(r: Rating) -> &'static str {
    match r {
        Rating::Again => "rating-again",
        Rating::Hard => "rating-hard",
        Rating::Good => "rating-good",
        Rating::Easy => "rating-easy",
    }
}

/// The timed prompt with the answer box (shared with the review session).
pub fn round_view(
    round: &CalcRound,
    input: &str,
    now: i64,
    target: f64,
    ui: &Ui,
    p: &Palette,
) -> AnyElement {
    let t = &ui.theme;
    let loc = ui.i18n.locale();
    let e = &round.exercise;
    let secs = (now - round.started_at).max(0) as f64 / 1000.0;
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.lg))
        .child(
            div()
                .text_size(px(t.text_size.lg))
                .line_height(px(t.text_size.lg * 1.5))
                .child(prompt_text(e, ui)),
        )
        .child(answer_box(input, ui.i18n.tr("calc-type"), ui, p))
        .child(
            div()
                .text_size(px(t.text_size.sm))
                .text_color(if secs > target {
                    p.accent
                } else {
                    p.text_muted
                })
                .child(ui.i18n.tr_args(
                    "calc-elapsed",
                    &[
                        ("secs", fmt::number(secs, 1, loc).into()),
                        ("target", fmt::number_trim(target, 1, loc).into()),
                    ],
                )),
        )
        .into_any_element()
}

/// The graded result (shared with the review session).
pub fn outcome_view(round: &CalcRound, o: &CalcOutcome, ui: &Ui, p: &Palette) -> AnyElement {
    let t = &ui.theme;
    let loc = ui.i18n.locale();
    let e = &round.exercise;
    let unit = e.unit;
    let headline = match (o.given, o.correct) {
        (None, _) => ui.i18n.tr("calc-result-skipped"),
        (_, true) => ui.i18n.tr("calc-result-right"),
        (_, false) => ui.i18n.tr("calc-result-wrong"),
    };
    let mut lines = vec![];
    if let Some(g) = o.given {
        lines.push(ui.i18n.tr_args(
            "calc-your-answer",
            &[("value", value_text(g, unit, loc).into())],
        ));
    }
    lines.push(ui.i18n.tr_args(
        "calc-exact",
        &[("value", value_text(o.exact, unit, loc).into())],
    ));
    lines.push(ui.i18n.tr_args(
        "calc-shortcut-value",
        &[("value", value_text(o.shortcut, unit, loc).into())],
    ));
    lines.push(
        ui.i18n.tr_args(
            "calc-tolerance",
            &[
                ("value", fmt::number_trim(o.tolerance, 1, loc).into()),
                (
                    "unit",
                    if unit == Unit::Percent {
                        "percent"
                    } else {
                        "other"
                    }
                    .into(),
                ),
            ],
        ),
    );
    lines.push(ui.i18n.tr_args(
        "calc-elapsed",
        &[
            ("secs", fmt::number(o.elapsed.as_secs_f64(), 1, loc).into()),
            (
                "target",
                fmt::number_trim(o.target.as_secs_f64(), 1, loc).into(),
            ),
        ],
    ));
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.sm))
        .child(
            div()
                .text_size(px(t.text_size.md))
                .text_color(p.text_muted)
                .child(prompt_text(e, ui)),
        )
        .child(
            div()
                .flex()
                .gap(px(t.spacing.md))
                .items_center()
                .child(
                    div()
                        .text_size(px(t.text_size.xl))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if o.correct { p.accent } else { p.text })
                        .child(headline),
                )
                .child(
                    div()
                        .px(px(t.spacing.sm))
                        .rounded(px(t.radii.pill))
                        .bg(p.surface_raised)
                        .text_size(px(t.text_size.sm))
                        .child(ui.i18n.tr_args(
                            "calc-rating",
                            &[("rating", ui.i18n.tr(rating_key(o.rating)).into())],
                        )),
                ),
        )
        .children(
            lines
                .into_iter()
                .map(|l| div().text_size(px(t.text_size.md)).child(l)),
        )
        .child(muted(
            ui.i18n.tr(&format!("calc-shortcut-{}", e.kind.id())),
            ui,
            p,
        ))
        .when(o.queued, |d| {
            d.child(muted(ui.i18n.tr("calc-queued"), ui, p))
        })
        .into_any_element()
}

impl Shell {
    pub(crate) fn calc_enter(&mut self) {
        if let Some(left) = self.calc.left_at.take()
            && self.calc.running()
            && let Some(round) = &mut self.calc.round
        {
            // Time spent on other screens does not count.
            round.started_at += (self.app.now() - left).max(0);
        }
        if self.calc.round.is_none() {
            self.calc_new_round();
        }
    }

    /// Pauses a running question's clock when the user leaves the screen.
    pub(crate) fn calc_leave(&mut self) {
        if self.calc.running() {
            self.calc.left_at = Some(self.app.now());
        }
    }

    fn calc_new_round(&mut self) {
        self.calc.round = Some(self.app.calc_start(self.calc.kind));
        self.calc.outcome = None;
        self.input.clear();
    }

    fn calc_answer(&mut self, given: Option<f64>) {
        let Some(round) = &self.calc.round else {
            return;
        };
        match self.app.calc_submit(round, given) {
            Ok(o) => self.calc.outcome = Some(o),
            Err(e) => eprintln!("taful: calc submit failed: {e}"),
        }
        self.input.clear();
    }

    pub(crate) fn calc_key(&mut self, key: KeyResult, _cx: &mut Context<Self>) {
        match key {
            KeyResult::Submit if self.calc.outcome.is_some() => self.calc_new_round(),
            KeyResult::Submit => {
                if let Some(x) = fmt::parse_number(&self.input, self.ui.i18n.locale()) {
                    self.calc_answer(Some(x));
                }
            }
            KeyResult::Escape if self.calc.running() => self.calc_answer(None),
            _ => {}
        }
    }

    pub(crate) fn calc_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let kinds = std::iter::once(None).chain(ExerciseKind::ALL.into_iter().map(Some));
        let picker =
            div()
                .flex()
                .flex_wrap()
                .gap(px(t.spacing.sm))
                .children(kinds.enumerate().map(|(i, k)| {
                    let label = match k {
                        None => ui.i18n.tr("calc-mixed"),
                        Some(k) => ui.i18n.tr(&format!("calc-kind-{}", k.id())),
                    };
                    chip(("calc_kind", i), label, self.calc.kind == k, &ui, &p).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.calc.kind = k;
                            this.calc_new_round();
                            window.focus(&this.focus);
                            cx.notify();
                        }),
                    )
                }));
        let body: AnyElement = match (&self.calc.round, &self.calc.outcome) {
            (Some(round), Some(o)) => div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.lg))
                .child(outcome_view(round, o, &ui, &p))
                .child(
                    button("calc-next", ui.i18n.tr("calc-next"), true, &ui, &p).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.calc_new_round();
                            window.focus(&this.focus);
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element(),
            (Some(round), None) => div()
                .flex()
                .flex_col()
                .gap(px(t.spacing.lg))
                .child(round_view(
                    round,
                    &self.input,
                    self.app.now(),
                    round
                        .exercise
                        .target(self.app.calc_settings())
                        .as_secs_f64(),
                    &ui,
                    &p,
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(t.spacing.sm))
                        .child(
                            button("calc-submit", ui.i18n.tr("calc-submit"), true, &ui, &p)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(x) =
                                        fmt::parse_number(&this.input, this.ui.i18n.locale())
                                    {
                                        this.calc_answer(Some(x));
                                    }
                                    window.focus(&this.focus);
                                    cx.notify();
                                })),
                        )
                        .child(
                            button("calc-skip", ui.i18n.tr("calc-skip"), false, &ui, &p).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.calc_answer(None);
                                    window.focus(&this.focus);
                                    cx.notify();
                                }),
                            ),
                        ),
                )
                .into_any_element(),
            _ => button("calc-start", ui.i18n.tr("calc-start"), true, &ui, &p)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.calc_new_round();
                    window.focus(&this.focus);
                    cx.notify();
                }))
                .into_any_element(),
        };
        let stats = self.calc.kind.and_then(|k| self.app.calc_stats(k).ok());
        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.lg))
            .child(muted(ui.i18n.tr("calc-intro"), &ui, &p))
            .child(picker)
            .child(card(&ui, &p).child(body))
            .when_some(stats.filter(|s| s.attempts > 0), |d, s| {
                d.child(muted(
                    ui.i18n.tr_args(
                        "calc-stats",
                        &[
                            ("correct", s.correct.into()),
                            ("attempts", s.attempts.into()),
                            (
                                "secs",
                                fmt::number(s.mean_duration_ms / 1000.0, 1, loc).into(),
                            ),
                        ],
                    ),
                    &ui,
                    &p,
                ))
            })
            .into_any_element()
    }
}

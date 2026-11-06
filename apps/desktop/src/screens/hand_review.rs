//! Hand review (spec §15.1 item 5, M5): the stored full hands, each
//! decision with its grade and explanation, and a replay that steps through
//! every action and dealt street.

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::model::{HandReview, HandSummary};
use taful_app::template::street_id;

use crate::felt::{self, Explain};
use crate::shell::Shell;
use crate::widgets::{button, card, muted, title};

#[derive(Default)]
pub struct HandReviewView {
    /// The hand the table stored last (the table's "review hand").
    pub last_stored: Option<String>,
    list: Vec<HandSummary>,
    open: Option<HandReview>,
    /// Index into the open hand's states.
    step: usize,
    /// The list was read since the last stored hand.
    pub loaded: bool,
}

impl Shell {
    pub(crate) fn hand_review_enter(&mut self) {
        self.hand_review.loaded = true;
        self.hand_review.list = self.app.recent_hands().unwrap_or_else(|e| {
            eprintln!("taful: hand list failed: {e}");
            Vec::new()
        });
        let current = self.hand_review.open.as_ref().map(|h| h.summary.id.clone());
        let keep = current.filter(|id| self.hand_review.list.iter().any(|h| &h.id == id));
        match keep.or_else(|| self.hand_review.list.first().map(|h| h.id.clone())) {
            Some(id) => self.open_hand(&id),
            None => self.hand_review.open = None,
        }
    }

    /// Open a stored hand at its first decision.
    pub(crate) fn open_hand(&mut self, id: &str) {
        if !self.hand_review.list.iter().any(|h| h.id == id) {
            self.hand_review.list = self.app.recent_hands().unwrap_or_default();
        }
        match self.app.hand_review(id) {
            Ok(h) => {
                self.hand_review.step = h.decisions.first().map_or(0, |d| d.step);
                self.hand_review.open = Some(h);
            }
            Err(e) => eprintln!("taful: hand review failed: {e}"),
        }
    }

    fn hand_step(&mut self, step: usize, cx: &mut Context<Self>) {
        if let Some(h) = &self.hand_review.open {
            self.hand_review.step = step.min(h.states.len().saturating_sub(1));
        }
        cx.notify();
    }

    fn summary_line(&self, h: &HandSummary) -> String {
        let i18n = &self.ui.i18n;
        let hand = h.hero_hand.map_or("?".into(), |x| x.to_string());
        let net = h
            .hero_net_bb
            .map_or("–".into(), |n| felt::num(&self.ui, n, 1));
        i18n.tr_args(
            "hand-review-summary",
            &[
                ("hand", hand.into()),
                ("net", net.into()),
                ("loss", felt::num(&self.ui, h.total_ev_loss_bb, 2).into()),
            ],
        )
    }

    pub(crate) fn hand_review_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        if !self.hand_review.loaded {
            self.hand_review_enter();
        }
        let Some(h) = &self.hand_review.open else {
            return card(&ui, &p)
                .child(ui.i18n.tr("hand-review-empty"))
                .child(
                    button("hr_play", ui.i18n.tr("hand-review-play"), true, &ui, &p).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.select(crate::screen::Screen::Table, window, cx)
                        }),
                    ),
                )
                .into_any_element();
        };
        let step = self.hand_review.step.min(h.states.len() - 1);
        let last = h.states.len() - 1;
        let state = &h.states[step];

        // Replay controls.
        let ctrl = |id: &'static str, key: &str, to: usize, enabled: bool| {
            button(id, ui.i18n.tr(key), false, &ui, &p)
                .when(!enabled, |b| b.opacity(0.4))
                .on_click(cx.listener(move |this, _, _, cx| this.hand_step(to, cx)))
        };
        let controls = div()
            .flex()
            .items_center()
            .gap(px(t.spacing.sm))
            .child(ctrl("hr_first", "hand-review-first", 0, step > 0))
            .child(ctrl(
                "hr_prev",
                "hand-review-prev",
                step.saturating_sub(1),
                step > 0,
            ))
            .child(ctrl(
                "hr_next",
                "hand-review-next",
                (step + 1).min(last),
                step < last,
            ))
            .child(ctrl("hr_last", "hand-review-last", last, step < last))
            .child(muted(
                ui.i18n.tr_args(
                    "hand-review-step",
                    &[("step", (step + 1).into()), ("total", (last + 1).into())],
                ),
                &ui,
                &p,
            ));

        // Decisions: click jumps to the table before the decision.
        let rows = h.decisions.iter().enumerate().map(|(i, d)| {
            let at = d.step;
            let street = d
                .street
                .map(|s| ui.i18n.tr(&format!("street-{}", street_id(s))))
                .unwrap_or_default();
            div()
                .id(("hr_dec", i))
                .flex()
                .justify_between()
                .px(px(t.spacing.sm))
                .py(px(t.spacing.xs))
                .rounded(px(t.radii.sm))
                .cursor_pointer()
                .text_size(px(t.text_size.sm))
                .when(at == step, |r| r.bg(p.surface_raised))
                .hover(|s| s.bg(p.surface_raised))
                .on_click(cx.listener(move |this, _, _, cx| this.hand_step(at, cx)))
                .child(format!(
                    "{street} · {}",
                    felt::action_short(&ui, d.actions[d.chosen])
                ))
                .child(felt::grade_label(&ui, &p, d.grade))
        });
        let mut panel = felt::panel(&ui, &p)
            .child(title(self.summary_line(&h.summary), &ui))
            .child(muted(fmt::datetime(h.summary.played_at, loc), &ui, &p))
            .child(
                div()
                    .text_size(px(t.text_size.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(ui.i18n.tr("table-decisions")),
            )
            .children(rows);
        if let Some(d) = h.decisions.iter().find(|d| d.step == step) {
            panel = panel.child(felt::explanation(
                &ui,
                &p,
                &Explain {
                    grade: d.grade,
                    ev_loss_bb: d.ev_loss_bb,
                    context: Some((state.position(h.hero), d.hero_combo.clone())),
                    actions: &d.actions,
                    chosen: d.chosen,
                    hand_freqs: &d.hand_freqs,
                    hand_evs: &d.hand_evs,
                    range_freqs: &d.range_freqs,
                    vs_profile: None,
                },
            ));
        }
        if h.decisions.is_empty() {
            panel = panel.child(muted(ui.i18n.tr("hand-review-no-decisions"), &ui, &p));
        }

        let replay = div()
            .flex()
            .gap(px(t.spacing.xl))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.lg))
                    .child(felt::table(&ui, &p, state, h.hero, step == last))
                    .child(controls),
            )
            .child(panel);

        // The hand list, newest first.
        let list = self.hand_review.list.iter().enumerate().map(|(i, s)| {
            let id = s.id.clone();
            let open = s.id == h.summary.id;
            let worst = s.worst.map(|g| felt::grade_label(&ui, &p, g));
            div()
                .id(("hr_hand", i))
                .flex()
                .justify_between()
                .gap(px(t.spacing.lg))
                .px(px(t.spacing.md))
                .py(px(t.spacing.sm))
                .rounded(px(t.radii.md))
                .border_1()
                .border_color(if open { p.accent } else { p.border })
                .bg(p.surface)
                .cursor_pointer()
                .hover(|s| s.border_color(p.accent))
                .text_size(px(t.text_size.sm))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_hand(&id);
                    cx.notify();
                }))
                .child(self.summary_line(s))
                .child(
                    div().flex().gap(px(t.spacing.md)).children(worst).child(
                        div()
                            .text_color(p.text_muted)
                            .child(fmt::datetime(s.played_at, loc)),
                    ),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.xl))
            .child(replay)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.sm))
                    .w(px(felt::FELT_W + felt::PANEL_W + t.spacing.xl))
                    .child(title(ui.i18n.tr("hand-review-recent"), &ui))
                    .children(list),
            )
            .into_any_element()
    }
}

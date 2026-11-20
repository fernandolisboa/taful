//! A graded decision as Baralho shows it: the grade as a stamp, then every
//! action with the reference's frequency as a bar and its EV.

use gpui::{Div, div, prelude::*, relative};
use taful_app::model::Grade;
use taful_app::preflop::Card;

use super::kit::{self, Pip, Raised};
use super::{Bx, Tone, typo, u};
use crate::Ui;
use crate::felt::{Explain, action_short, grade_key, num};
use crate::theme::{Palette, alpha};

/// The round seal at the left of the card's head.
pub fn seal(bx: &Bx, tone: Tone, inner: impl IntoElement) -> Div {
    Raised {
        face: tone,
        edge: bx.deep,
        radius: 33.,
        border: 2.5,
        drop: 4.,
    }
    .sized(
        66.,
        66.,
        div().flex().items_center().justify_center().child(inner),
    )
}

/// What a grade's seal carries: a check for a good answer, else the mark
/// chess annotation gives a move that bad.
fn grade_mark(bx: &Bx, grade: Grade) -> Div {
    let mark = match grade {
        Grade::Best | Grade::Correct => {
            return div().child(kit::icon(kit::CHECK, 34., bx.paper));
        }
        Grade::Inaccuracy => "?!",
        Grade::Mistake => "?",
        Grade::Blunder => "??",
    };
    typo::disp(30., 800, 30)
        .on(div())
        .text_color(bx.paper)
        .child(mark)
}

/// The head of the card: a seal, a title with a line under it and, at the
/// right, a figure.
pub fn head(
    seal: Div,
    title: String,
    tone: Tone,
    size: f32,
    line: Option<Div>,
    figure: Option<Div>,
) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(u(16.))
        // Room for the seal's drop.
        .pb(u(4.))
        .child(seal)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    typo::disp_at(96, size, 800, 40)
                        .on(div())
                        .text_color(tone)
                        .child(title),
                )
                .children(line.map(|l| l.mt(u(4.)))),
        )
        .children(figure)
}

/// The line under a head's title.
pub fn head_line(bx: &Bx, text: impl IntoElement) -> Div {
    typo::text(14.5, 600)
        .on(div())
        .text_color(bx.ink2)
        .child(text)
}

/// A section's label, with an optional note at the right.
pub fn section(bx: &Bx, title: String, note: Option<String>) -> Div {
    typo::caps(11.5, 800, 120)
        .on(div())
        .flex()
        .flex_none()
        .items_baseline()
        .justify_between()
        .gap(u(10.))
        .mt(u(22.))
        .mb(u(8.))
        .text_color(bx.mut_ink)
        .child(div().flex_none().child(title.to_uppercase()))
        .children(note.map(|n| typo::text(12.5, 600).on(div()).child(n)))
}

/// A paragraph of the card.
pub fn note(bx: &Bx, text: String) -> Div {
    typo::text(13.5, 600)
        .on(div())
        .mt(u(10.))
        .text_color(bx.ink2)
        .child(text)
}

/// The cards of a hand written like `QsJs`.
fn hand_cards(hand: &str) -> Vec<Card> {
    let chars: Vec<char> = hand.chars().collect();
    chars
        .chunks(2)
        .filter_map(|c| c.iter().collect::<String>().parse().ok())
        .collect()
}

/// Cards inline in a line of text: the rank, then the suit's glyph.
fn inline_cards(p: &Palette, cards: &[Card], size: f32) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(u(size * 0.3))
        .children(cards.iter().map(|c| {
            div()
                .flex()
                .items_center()
                .child(c.rank().to_char().to_string())
                .child(kit::icon(Pip::of(c.suit()).path(), size * 0.86, p.suit(c.suit())).ml(u(1.)))
        }))
}

/// One action: its name, the reference's frequency as a bar and a figure,
/// and its EV. `tag` is a small label under the name; `picked` frames
/// the row in that color.
fn option_row(
    bx: &Bx,
    name: String,
    tag: Option<(String, Tone)>,
    freq: Option<f32>,
    freq_text: String,
    ev: String,
    picked: Option<(Tone, Tone)>,
) -> Div {
    let fill = freq.unwrap_or(0.).clamp(0., 1.);
    let bar = div()
        .flex_1()
        .min_w_0()
        .h(u(16.))
        .rounded(u(8.))
        .bg(bx.deep)
        .p(u(2.))
        .child(
            div()
                .size_full()
                .rounded(u(6.))
                .bg(bx.cream2)
                .overflow_hidden()
                .when(fill > 0.004, |d| {
                    d.child(
                        div()
                            .h_full()
                            .w(relative(fill))
                            .min_w(u(4.))
                            .bg(bx.amber)
                            .border_r(u(2.))
                            .border_color(bx.deep),
                    )
                }),
        );
    typo::text(14.5, 700)
        .on(div())
        .flex()
        .flex_none()
        .items_center()
        .gap(u(10.))
        .py(u(9.))
        .px(u(12.))
        .rounded(u(13.))
        .border(u(2.))
        .border_color(alpha(bx.cream, 0.))
        .when_some(picked, |d, (line, wash)| d.bg(wash).border_color(line))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(u(112.))
                .child(div().whitespace_nowrap().child(name))
                .children(tag.map(|(text, tone)| {
                    typo::caps(11., 800, 60)
                        .on(div())
                        .text_color(tone)
                        .child(text.to_uppercase())
                })),
        )
        .child(bar)
        .child(
            div()
                .flex()
                .flex_none()
                .justify_end()
                .w(u(42.))
                .text_color(bx.ink2)
                .child(freq_text),
        )
        .child(
            typo::disp(17., 800, 20)
                .lh(1.2)
                .on(div())
                .flex()
                .flex_none()
                .justify_end()
                .w(u(52.))
                .child(ev),
        )
}

/// A graded decision: the stamp, then the actions. `labels` names each
/// action (the table knows the size of a call; without it they are the
/// short names).
pub fn graded(ui: &Ui, p: &Palette, bx: &Bx, d: &Explain<'_>, labels: Option<&[String]>) -> Div {
    let i18n = &ui.i18n;
    let tone = p.grade(d.grade);
    let label = |i: usize| {
        labels
            .and_then(|l| l.get(i).cloned())
            .or_else(|| d.actions.get(i).map(|&a| action_short(ui, a)))
            .unwrap_or_default()
    };
    let line = d.context.as_ref().map(|(_, hand)| {
        head_line(
            bx,
            div()
                .flex()
                .items_center()
                .gap(u(5.))
                .child(i18n.tr_args("baralho-result-with", &[("action", label(d.chosen).into())]))
                .child(inline_cards(p, &hand_cards(hand), 14.5)),
        )
    });
    let figure = div()
        .flex()
        .flex_col()
        .flex_none()
        .items_end()
        .child(
            typo::caps(10.5, 800, 120)
                .on(div())
                .text_color(bx.mut_ink)
                .child(i18n.tr("baralho-ev-loss").to_uppercase()),
        )
        .child(
            typo::disp(24., 800, 30)
                .lh(1.2)
                .on(div())
                .child(format!("{} bb", num(ui, d.ev_loss_bb, 2))),
        );
    let best = d.best();
    let vs = d.vs_profile;
    let note = match vs {
        Some(v) => i18n.tr_args(
            "baralho-actions-note-profile",
            &[("profile", i18n.opponent(&v.profile).into())],
        ),
        None => i18n.tr("baralho-actions-note"),
    };
    let good = matches!(d.grade, Grade::Best | Grade::Correct);
    let rows = (0..d.actions.len()).map(|i| {
        let freq = d.hand_freqs.get(i).copied();
        let tag = if i == d.chosen {
            Some((i18n.tr("baralho-your-pick"), tone))
        } else if i == best && d.grade != Grade::Best {
            Some((i18n.tr("table-grade-best"), p.grade_best))
        } else {
            None
        };
        option_row(
            bx,
            label(i),
            tag,
            freq,
            freq.map_or("–".into(), |f| {
                format!("{}%", num(ui, f64::from(f) * 100., 0))
            }),
            d.hand_evs
                .get(i)
                .map_or("–".into(), |e| num(ui, f64::from(*e), 2)),
            (i == d.chosen).then(|| (tone, if good { bx.pick } else { alpha(tone, 0.14) })),
        )
    });
    // Against a profile: the GTO line, the exploitative line and their gap.
    let lines = vs.map(|v| {
        let line = |key: &str, a: usize| i18n.tr_args(key, &[("action", label(a).into())]);
        typo::text(12.5, 600)
            .on(div())
            .flex()
            .flex_col()
            .flex_none()
            .mt(u(8.))
            .px(u(12.))
            .text_color(bx.mut_ink)
            .child(line("table-exploit-line", v.exploit))
            .child(line("table-gto-line", v.gto))
            .child(i18n.tr_args(
                "table-ev-gap",
                &[("amount", num(ui, v.ev_gap_bb, 2).into())],
            ))
    });
    div()
        .flex()
        .flex_col()
        .flex_none()
        .text_color(bx.ink)
        .child(head(
            seal(bx, tone, grade_mark(bx, d.grade)),
            i18n.tr(&grade_key(d.grade)),
            tone,
            40.,
            line,
            Some(figure),
        ))
        .child(section(bx, i18n.tr("baralho-actions-here"), Some(note)))
        .children(rows)
        .children(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hands_read_as_cards() {
        let cards = hand_cards("QsJs");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].to_string(), "Qs");
        assert_eq!(cards[1].to_string(), "Js");
        assert!(hand_cards("AKs").len() < 2);
    }
}

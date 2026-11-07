//! Table drawing shared by every screen that shows a poker table (spec
//! §15.2): the table screen, the review session and the hand review. No
//! state and no listeners: plain elements from a table state.

use gpui::{Div, FontWeight, div, prelude::*, px};
use taful_app::fmt;
use taful_app::i18n::Locale;
use taful_app::model::Grade;
use taful_app::preflop::{Action, BB_CHIPS, Card, Chips, GameState, Position, Seat};

use crate::Ui;
use crate::motion;
use crate::theme::Palette;

pub const FELT_W: f32 = 640.;
pub const FELT_H: f32 = 300.;
const SEAT_W: f32 = 120.;
const SEAT_H: f32 = 72.;
/// Room above and below the felt for the top and bottom seats.
const PAD: f32 = 20.;
const CARD_W: f32 = 26.;
const CARD_H: f32 = 36.;
pub const PANEL_W: f32 = 340.;

/// A number in the UI locale.
pub fn num(ui: &Ui, x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    match ui.i18n.locale() {
        Locale::PtBr => s.replace('.', ","),
        Locale::EnUs => s,
    }
}

/// Chips as big blinds.
pub fn bb(ui: &Ui, chips: Chips) -> String {
    let v = chips as f64 / BB_CHIPS as f64;
    num(ui, v, if chips.is_multiple_of(BB_CHIPS) { 0 } else { 1 })
}

/// Button label for hero's action at `state`.
pub fn action_label(ui: &Ui, state: &GameState, a: Action) -> String {
    let i18n = &ui.i18n;
    let amount = |c: Chips| bb(ui, c).into();
    match a {
        Action::Fold => i18n.tr("table-action-fold"),
        Action::Check => i18n.tr("table-action-check"),
        Action::Call => {
            let to_call = state.bounds().map_or(0, |b| b.to_call);
            i18n.tr_args("table-action-call", &[("amount", amount(to_call))])
        }
        Action::Bet(to) => i18n.tr_args("table-action-bet", &[("amount", amount(to))]),
        Action::Raise(to) => i18n.tr_args("table-action-raise", &[("amount", amount(to))]),
        Action::AllIn => {
            let to = state.bounds().and_then(|b| b.max_to).unwrap_or(0);
            i18n.tr_args("table-action-allin", &[("amount", amount(to))])
        }
    }
}

/// Short label for logs and the decision table (no table state needed).
pub fn action_short(ui: &Ui, a: Action) -> String {
    let i18n = &ui.i18n;
    match a {
        Action::Fold => i18n.tr("table-action-fold"),
        Action::Check => i18n.tr("table-action-check"),
        Action::Call => i18n.tr("table-log-call"),
        Action::Bet(to) => i18n.tr_args("table-action-bet", &[("amount", bb(ui, to).into())]),
        Action::Raise(to) => i18n.tr_args("table-action-raise", &[("amount", bb(ui, to).into())]),
        Action::AllIn => i18n.tr("table-log-allin"),
    }
}

/// One card face (or a back for `None`), `s` times the normal size.
pub fn card(ui: &Ui, p: &Palette, c: Option<Card>, s: f32) -> Div {
    let t = &ui.theme;
    let base = div()
        .w(px(CARD_W * s))
        .h(px(CARD_H * s))
        .rounded(px(t.radii.sm * s))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .text_size(px(t.text_size.sm * s))
        .font_weight(FontWeight::BOLD);
    match c {
        Some(c) => base
            .bg(p.card_face)
            .text_color(p.suit(c.suit()))
            .child(c.rank().to_char().to_string())
            .child(fmt::suit_symbol(c.suit()).to_string()),
        None => base.bg(p.card_back).border_1().border_color(p.rail_edge),
    }
}

/// Width of [`table`] at scale 1.
pub const TABLE_W: f32 = FELT_W;
/// Height of [`table`] at scale 1 (felt plus room for the top and bottom seats).
pub const TABLE_H: f32 = FELT_H + 2. * PAD;

/// The scale at which [`table`] fits a `w` × `h` box, within the range
/// that stays readable.
pub fn fit_scale(w: f32, h: f32) -> f32 {
    (w / TABLE_W).min(h / TABLE_H).clamp(0.7, 1.8)
}

/// Felt, seats, pot and board, at scale 1. `reveal` shows every live hand
/// (end of hand).
pub fn table(ui: &Ui, p: &Palette, state: &GameState, hero: Seat, reveal: bool) -> Div {
    table_scaled(ui, p, state, hero, reveal, 1.)
}

/// [`table`] drawn `s` times its normal size (layout panels, spec §15.3).
pub fn table_scaled(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    s: f32,
) -> Div {
    table_tagged(ui, p, state, hero, reveal, s, SeatExtras::default())
}

/// Seat tags for [`table_tagged`]: each bot profile's name (none for hero
/// and reference bots).
pub fn seat_tags(ui: &Ui, profiles: &[Option<String>]) -> Vec<Option<String>> {
    profiles
        .iter()
        .map(|p| {
            p.as_ref()
                .map(|id| ui.i18n.tr(&format!("bot-profile-{id}")))
        })
        .collect()
}

/// What [`table_tagged`] adds to the seats.
#[derive(Default, Clone, Copy)]
pub struct SeatExtras<'a> {
    /// A tag per seat (`None` = no tag).
    pub tags: &'a [Option<String>],
    /// Chips each winner took, slid from the pot once the hand is over.
    pub wins: &'a [(Seat, Chips)],
}

/// [`table_scaled`] with a tag over each seat that has one (the bot
/// profile a seat plays, #28).
pub fn table_tagged(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    s: f32,
    extra: SeatExtras<'_>,
) -> Div {
    let SeatExtras { tags, wins } = extra;
    let (t, i18n) = (&ui.theme, &ui.i18n);
    let n = state.players().len();
    let (felt_w, felt_h, pad) = (FELT_W * s, FELT_H * s, PAD * s);
    let (seat_w, seat_h) = (SEAT_W * s, SEAT_H * s);
    let (sm, md) = (t.text_size.sm * s, t.text_size.md * s);
    let (cx0, cy0) = (felt_w / 2., felt_h / 2. + pad);
    let (rx, ry) = (
        felt_w / 2. - seat_w / 2. + 8. * s,
        felt_h / 2. - seat_h / 2. + 18. * s,
    );
    let to_act = state.to_act();
    let seats = (0..n).map(|seat| {
        // Hero sits at the bottom; the others clockwise from hero's left.
        let k = (seat + n - hero) % n;
        let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
        let (x, y) = (cx0 + rx * angle.cos(), cy0 + ry * angle.sin());
        let pl = &state.players()[seat];
        let pos = state.position(seat);
        let active = to_act == Some(seat);
        let show = seat == hero || (reveal && !pl.folded);
        let cards: Vec<Option<Card>> = if pl.folded {
            vec![]
        } else if show {
            pl.hole.cards().map(Some).to_vec()
        } else {
            vec![None, None]
        };
        let last = state
            .actions()
            .last()
            .into_iter()
            .flatten()
            .rev()
            .find(|(s, _)| *s == seat)
            .map(|(_, a)| action_short(ui, *a));
        div()
            .absolute()
            .left(px(x - seat_w / 2.))
            .top(px(y - seat_h / 2.))
            .w(px(seat_w))
            .h(px(seat_h))
            .rounded(px(t.radii.lg * s))
            .border_1()
            .border_color(if active { p.accent } else { p.rail_edge })
            .bg(if active { p.seat_active } else { p.seat })
            .when(pl.folded, |d| d.opacity(0.5))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(2. * s))
            .child(
                div()
                    .flex()
                    .gap(px(t.spacing.xs * s))
                    .text_size(px(sm))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if seat == hero { p.accent } else { p.text })
                            .child(pos.label()),
                    )
                    .child(div().text_color(p.text_muted).child(
                        i18n.tr_args("table-stack", &[("amount", bb(ui, pl.stack).into())]),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(2. * s))
                    .children(cards.into_iter().enumerate().map(|(i, c)| {
                        // Keyed by the real card: a new hand deals again,
                        // turning hero's or a showdown hand face up does not.
                        let real = pl.hole.cards()[i];
                        motion::deal_in(
                            card(ui, p, c, s),
                            anim_id(format!("deal_{seat}_{i}_{real}")),
                            14. * s,
                        )
                    })),
            )
            .when_some(last, |d, l| {
                d.child(div().text_size(px(sm)).text_color(p.text_muted).child(l))
            })
    });
    // Seat tags, just outside each seat on the side facing the pot, so they
    // cover neither the seat nor anything outside the felt.
    let tag_h = sm * 0.85 + 6. * s;
    let tag_els = (0..n).filter_map(|seat| {
        let tag = tags.get(seat)?.as_ref()?;
        let k = (seat + n - hero) % n;
        let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
        let (x, y) = (cx0 + rx * angle.cos(), cy0 + ry * angle.sin());
        let top = if angle.sin() >= 0. {
            y - seat_h / 2. - tag_h - 2. * s
        } else {
            y + seat_h / 2. + 2. * s
        };
        Some(
            div()
                .absolute()
                .left(px(x - seat_w / 2.))
                .top(px(top))
                .w(px(seat_w))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(t.spacing.sm * s))
                        .rounded(px(t.radii.pill))
                        .bg(p.rail)
                        .border_1()
                        .border_color(p.rail_edge)
                        .text_size(px(sm * 0.85))
                        .text_color(p.accent)
                        .child(tag.clone()),
                ),
        )
    });
    // Chips in front of each seat on this street, between seat and pot.
    let bets = (0..n).filter_map(|seat| {
        let c = state.players()[seat].street_commit;
        if c == 0 {
            return None;
        }
        let k = (seat + n - hero) % n;
        let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
        let (x, y) = (cx0 + rx * 0.55 * angle.cos(), cy0 + ry * 0.5 * angle.sin());
        let (sx, sy) = (cx0 + rx * angle.cos(), cy0 + ry * angle.sin());
        let street = state.street();
        Some(motion::slide(
            div()
                .absolute()
                .w(px(56. * s))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(t.spacing.sm * s))
                        .rounded(px(t.radii.pill))
                        .bg(p.rail)
                        .text_size(px(sm))
                        .text_color(p.accent)
                        .child(bb(ui, c)),
                ),
            // From the seat toward the pot, again on every new amount.
            anim_id(format!("bet_{seat}_{street:?}_{c}")),
            (sx - 28. * s, sy - 10. * s),
            (x - 28. * s, y - 10. * s),
        ))
    });
    // What each winner took, slid from the pot to the seat.
    let win_els = wins.iter().filter(|(_, c)| *c > 0).map(|&(seat, c)| {
        let k = (seat + n - hero) % n;
        let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
        let (x, y) = (cx0 + rx * 0.62 * angle.cos(), cy0 + ry * 0.58 * angle.sin());
        motion::slide(
            div()
                .absolute()
                .w(px(72. * s))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(t.spacing.sm * s))
                        .rounded(px(t.radii.pill))
                        .bg(p.accent)
                        .text_size(px(sm))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.on_accent)
                        .child(format!("+{}", bb(ui, c))),
                ),
            anim_id(format!("win_{seat}_{c}")),
            (cx0 - 36. * s, cy0 - 10. * s),
            (x - 36. * s, y - 10. * s),
        )
    });
    let board = state.board().to_vec();
    div()
        .relative()
        .w(px(felt_w))
        .h(px(felt_h + 2. * pad))
        .flex_none()
        .child(
            div()
                .absolute()
                .left(px(seat_w / 2.))
                .top(px(seat_h / 2. - 6. * s + pad))
                .w(px(felt_w - seat_w))
                .h(px(felt_h - seat_h + 12. * s))
                .rounded(px(t.radii.pill))
                .bg(p.felt)
                .border(px(6. * s))
                .border_color(p.rail)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(t.spacing.sm * s))
                .child(motion::fade_in(
                    div()
                        .text_size(px(md))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child(
                            i18n.tr_args("table-pot", &[("amount", bb(ui, state.pot()).into())]),
                        ),
                    anim_id(format!("pot_{}", state.pot())),
                ))
                .when(!board.is_empty(), |d| {
                    d.child(div().flex().gap(px(t.spacing.xs * s)).children(
                        board.iter().enumerate().map(|(i, &c)| {
                            motion::deal_in(
                                card(ui, p, Some(c), s),
                                anim_id(format!("board_{i}_{c}")),
                                14. * s,
                            )
                        }),
                    ))
                }),
        )
        .children(bets)
        .children(seats)
        .children(tag_els)
        .children(win_els)
}

fn anim_id(s: String) -> gpui::ElementId {
    gpui::ElementId::Name(s.into())
}

/// The numbers of one graded decision (spec §8 explanation payload).
pub struct Explain<'a> {
    pub grade: Grade,
    pub ev_loss_bb: f64,
    /// `(position, hand)` line under the grade.
    pub context: Option<(Position, String)>,
    pub actions: &'a [Action],
    pub chosen: usize,
    pub hand_freqs: &'a [f32],
    pub hand_evs: &'a [f32],
    pub range_freqs: &'a [f32],
    /// Graded against a bot profile: `hand_evs` are against the profile.
    pub vs_profile: Option<&'a taful_app::preflop::VsProfile>,
}

impl<'a> Explain<'a> {
    pub fn of(d: &'a taful_app::preflop::GradedDecision) -> Explain<'a> {
        Explain {
            grade: d.grade,
            ev_loss_bb: d.ev_loss_bb,
            context: Some((d.position, d.hand.to_string())),
            actions: &d.actions,
            chosen: d.chosen,
            hand_freqs: &d.hand_freqs,
            hand_evs: &d.hand_evs,
            range_freqs: &d.range_freqs,
            vs_profile: d.vs_profile.as_ref(),
        }
    }

    fn best(&self) -> usize {
        (0..self.hand_evs.len())
            .max_by(|&a, &b| self.hand_evs[a].total_cmp(&self.hand_evs[b]))
            .unwrap_or(0)
    }
}

/// Fluent id of a grade's label.
pub fn grade_key(g: Grade) -> String {
    format!("table-grade-{}", g.id())
}

/// Grade, EV loss and the per-action table for one decision (spec §8).
pub fn explanation(ui: &Ui, p: &Palette, d: &Explain<'_>) -> Div {
    let (t, i18n) = (&ui.theme, &ui.i18n);
    let best = d.best();
    let header = div()
        .flex()
        .items_baseline()
        .gap(px(t.spacing.md))
        .child(
            div()
                .text_size(px(t.text_size.lg))
                .font_weight(FontWeight::BOLD)
                .text_color(p.grade(d.grade))
                .child(i18n.tr(&grade_key(d.grade))),
        )
        .child(
            div()
                .text_size(px(t.text_size.sm))
                .text_color(p.text_muted)
                .child(i18n.tr_args(
                    "table-ev-loss",
                    &[("amount", num(ui, d.ev_loss_bb, 2).into())],
                )),
        );
    // The action column takes what the panel has left (layouts change the
    // panel's width); the numbers keep fixed columns.
    let first = |s: String| div().flex_1().min_w(px(0.)).child(s);
    let cell = |s: String, w: f32| div().w(px(w)).flex_none().child(s);
    let vs = d.vs_profile;
    let head = div()
        .flex()
        .text_size(px(t.text_size.sm))
        .text_color(p.text_muted)
        .child(first(i18n.tr("table-col-action")))
        .child(cell(i18n.tr("table-col-hand-freq"), 64.))
        .child(cell(
            i18n.tr(if vs.is_some() {
                "table-col-ev-profile"
            } else {
                "table-col-ev"
            }),
            56.,
        ))
        .when(vs.is_some(), |h| {
            h.child(cell(i18n.tr("table-col-ev-gto"), 56.))
        })
        .child(cell(i18n.tr("table-col-range-freq"), 56.));
    let pct = |v: Option<&f32>| match v {
        Some(x) => format!("{}%", num(ui, f64::from(*x) * 100., 0)),
        None => "–".into(),
    };
    let rows = d.actions.iter().enumerate().map(|(i, &a)| {
        let mark = if i == d.chosen { "▸ " } else { "" };
        div()
            .flex()
            .text_size(px(t.text_size.sm))
            .when(i == best, |r| {
                r.text_color(p.accent).font_weight(FontWeight::SEMIBOLD)
            })
            .child(first(format!("{mark}{}", action_short(ui, a))))
            .child(cell(pct(d.hand_freqs.get(i)), 64.))
            .child(cell(
                d.hand_evs
                    .get(i)
                    .map_or("–".into(), |e| num(ui, f64::from(*e), 2)),
                56.,
            ))
            .when_some(vs, |r, v| {
                r.child(cell(
                    v.reference_evs
                        .get(i)
                        .map_or("–".into(), |e| num(ui, f64::from(*e), 2)),
                    56.,
                ))
            })
            .child(cell(pct(d.range_freqs.get(i)), 56.))
    });
    // GTO line vs exploitative line and their EV gap (spec §8, D8).
    let lines = vs.map(|v| {
        let line = |key: &str, a: usize| {
            i18n.tr_args(
                key,
                &[(
                    "action",
                    d.actions
                        .get(a)
                        .map_or("–".into(), |&x| action_short(ui, x))
                        .into(),
                )],
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.xs))
            .text_size(px(t.text_size.sm))
            .child(div().text_color(p.text_muted).child(
                if taful_app::mixed::is_table_key(&v.profile) {
                    i18n.tr_args(
                        "table-vs-table",
                        &[("table", i18n.opponent(&v.profile).into())],
                    )
                } else {
                    i18n.tr_args(
                        "table-vs-profile",
                        &[("profile", i18n.opponent(&v.profile).into())],
                    )
                },
            ))
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
        .gap(px(t.spacing.sm))
        .child(header)
        .when_some(d.context.as_ref(), |c, (pos, hand)| {
            c.child(
                div()
                    .text_size(px(t.text_size.sm))
                    .text_color(p.text_muted)
                    .child(i18n.tr_args(
                        "table-decision-context",
                        &[
                            ("position", pos.label().into()),
                            ("hand", hand.clone().into()),
                        ],
                    )),
            )
        })
        .children(lines)
        .child(head)
        .children(rows)
}

/// The side panel next to a table.
pub fn panel(ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    div()
        .w(px(PANEL_W))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(t.spacing.lg))
        .p(px(t.spacing.lg))
        .rounded(px(t.radii.lg))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
}

/// A grade label in its color (decision lists).
pub fn grade_label(ui: &Ui, p: &Palette, g: Grade) -> Div {
    div()
        .text_color(p.grade(g))
        .child(ui.i18n.tr(&grade_key(g)))
}

/// Readable name of a spot configuration (`preflop` or a library id).
pub fn spot_label(ui: &Ui, id: &str) -> String {
    let key = format!("table-postflop-{id}");
    if ui.i18n.has(&key) {
        ui.i18n.tr(&key)
    } else {
        id.to_string()
    }
}

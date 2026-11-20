//! The table on Cockpit's table screen: the felt, a pod per seat, the pot,
//! the board and the bets. Everything sits at the design's coordinates on a
//! 900 × 516 stage, scaled as a whole to the panel.

use gpui::{
    AnyElement, Div, FontWeight, SharedString, div, linear_color_stop, linear_gradient, prelude::*,
    px,
};
use taful_app::preflop::{Action, Card, Seat};

use super::kit::{self, MONO, Tile, tile, tracked, tracked_wrap};
use super::table::{Ev, Scene};
use crate::felt::{anim_id, bb};
use crate::motion;
use crate::theme::alpha;

/// The stage at scale 1.
const STAGE: (f32, f32) = (900., 516.);
/// The scales the stage is drawn at, and its height at the smallest and at
/// its natural size.
const SCALES: (f32, f32) = (0.55, 1.6);
pub(super) const STAGE_MIN_H: f32 = STAGE.1 * SCALES.0;
pub(super) const STAGE_H: f32 = STAGE.1;
const CENTER: (f32, f32) = (450., 249.);
const POD_W: f32 = 128.;
/// Where the pot, the board and hero's hand are centered.
const POT: (f32, f32) = (450., 164.);
const BOARD: (f32, f32) = (450., 234.);
const HERO_HAND: (f32, f32) = (450., 358.);

/// Center of the pod of the `k`-th seat clockwise from hero at a table of
/// `n` (hero is 0, at the bottom).
fn pod_at(k: usize, n: usize) -> (f32, f32) {
    const SIX: [(f32, f32); 6] = [
        (450., 436.),
        (104., 322.),
        (104., 162.),
        (450., 62.),
        (796., 162.),
        (796., 322.),
    ];
    if n == 6 {
        return SIX[k];
    }
    let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
    (
        (CENTER.0 + 380. * angle.cos()).clamp(104., 796.),
        CENTER.1 + 187. * angle.sin(),
    )
}

/// Which side of the table a pod is on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Hero,
    Top,
    Left,
    Right,
}

fn side(k: usize, (x, y): (f32, f32)) -> Side {
    if k == 0 {
        Side::Hero
    } else if (x - CENTER.0).abs() < 60. && y < CENTER.1 {
        Side::Top
    } else if x < CENTER.0 {
        Side::Left
    } else {
        Side::Right
    }
}

/// Toward the middle of the table from a pod at height `y`.
fn inward(y: f32) -> f32 {
    if y > CENTER.1 { -1. } else { 1. }
}

/// Where a seat's face-down cards, bet and dealer button go.
fn spots(k: usize, at: (f32, f32)) -> ((f32, f32), (f32, f32), (f32, f32)) {
    let (x, y) = at;
    match side(k, at) {
        Side::Hero => (HERO_HAND, (358., 318.), (560., 338.)),
        // Face-down cards sit over the pod's top edge, as designed for the
        // side seats; the top seat's go beside it, clear of the pot.
        Side::Top => ((x + 106., y), (x + 102., y + 64.), (x - 90., y + 42.)),
        Side::Left => (
            (x, y - 60.),
            (x + 132., y + 38. * inward(y)),
            (x + 100., y - 38. * inward(y)),
        ),
        Side::Right => (
            (x, y - 60.),
            (x - 132., y + 38. * inward(y)),
            (x - 100., y - 38. * inward(y)),
        ),
    }
}

/// A box `w` wide whose content is centered on `(x, y)`.
fn centered(x: f32, y: f32, w: f32, s: f32) -> Div {
    const ROOM: f32 = 240.;
    div()
        .absolute()
        .left(px((x - w / 2.) * s))
        .top(px((y - ROOM / 2.) * s))
        .w(px(w * s))
        .h(px(ROOM * s))
        .flex()
        .items_center()
        .justify_center()
}

/// A seat: position, stack, how much of the starting stack is left, and
/// the seat's last action.
fn pod(sc: &Scene<'_>, seat: Seat, last: Option<&Ev>, s: f32) -> Div {
    let (ui, pal, tp) = (sc.ui, &sc.pal, &sc.tp);
    let i18n = &ui.i18n;
    let state = sc.state();
    let pl = &state.players()[seat];
    let hero = seat == sc.hero;
    let start = state
        .initial_stacks()
        .get(seat)
        .copied()
        .unwrap_or(pl.stack);
    let left = if start > 0 {
        (pl.stack as f32 / start as f32).clamp(0., 1.)
    } else {
        0.
    };
    let you = || i18n.tr("cockpit-you");
    // The action line: the last action, hero's marked as such; before
    // hero acts, that it is hero's turn.
    let (text, color) = match last {
        Some(e) => {
            let mut text = e.what.clone();
            if let Some(share) = e.pot_share {
                text = format!(
                    "{text} · {}",
                    i18n.tr_args(
                        "cockpit-act-pot-share",
                        &[("percent", super::frame::percent(share).into())],
                    )
                );
            }
            if hero {
                text = format!("{text} · {}", you());
            }
            let color = match e.action {
                Action::Fold => pal.muted,
                Action::Check | Action::Call => tp.call,
                Action::Bet(_) | Action::Raise(_) | Action::AllIn => tp.raise,
            };
            (text, color)
        }
        None if hero && sc.hero_turn => (i18n.tr("cockpit-pod-turn"), pal.amber),
        None => (String::new(), pal.muted),
    };
    let body = div()
        .w(px(POD_W * s))
        .flex_none()
        .rounded(px(5. * s))
        .border_1()
        .pt(px(7. * s))
        .px(px(9. * s))
        .pb(px(8. * s))
        .map(|d| {
            if pl.folded {
                d.bg(tp.pod_fold).border_color(pal.line)
            } else {
                d.bg(tp.pod)
                    .border_color(if hero { pal.amber } else { pal.line2 })
                    .shadow(kit::shadow(tp.shadow, 8. * s, 18. * s))
            }
        })
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .child(
                    tracked(state.position(seat).label(), 12.5 * s, 0.02)
                        .font_weight(FontWeight::BOLD)
                        .text_color(if pl.folded {
                            pal.dim
                        } else if hero {
                            pal.amber
                        } else {
                            pal.text
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_end()
                        .font_family(MONO)
                        .text_size(px(12.5 * s))
                        .text_color(if pl.folded { pal.dim } else { pal.text })
                        .child(SharedString::from(bb(ui, pl.stack)))
                        .child(
                            div()
                                .ml(px(2. * s))
                                .text_size(px(10. * s))
                                .text_color(pal.muted)
                                .child(i18n.tr("cockpit-unit-bb")),
                        ),
                ),
        )
        .child(
            div()
                .mt(px(6. * s))
                .h(px(3. * s))
                .rounded(px(1.))
                .overflow_hidden()
                .bg(tp.pod_bar)
                .child(div().h_full().w(gpui::relative(left)).bg(if pl.folded {
                    pal.line2
                } else {
                    pal.teal
                })),
        )
        .child(
            tracked_wrap(&text.to_uppercase(), 10.5 * s, 0.06)
                .mt(px(5. * s))
                .min_h(px(15. * s))
                .line_height(px(15. * s))
                .text_color(color),
        );
    if hero && !pl.folded {
        // The ring around hero's pod.
        div()
            .flex_none()
            .p(px(3. * s))
            .rounded(px(8. * s))
            .bg(alpha(pal.amber, 0.14))
            .child(body)
    } else {
        body
    }
}

/// Chips in front of a seat.
fn bet(text: String, chip: super::pal::Tint, sc: &Scene<'_>, s: f32) -> Div {
    let tp = &sc.tp;
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6. * s))
        .py(px(2. * s))
        .pl(px(3. * s))
        .pr(px(9. * s))
        .rounded_full()
        .border_1()
        .border_color(tp.bet_edge)
        .bg(tp.bet_bg)
        .font_family(MONO)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13. * s))
        .whitespace_nowrap()
        .child(
            div()
                .flex_none()
                .size(px(16. * s))
                .rounded_full()
                .bg(chip)
                .border(px(3. * s))
                .border_color(tp.bet_shade),
        )
        .child(SharedString::from(text))
}

/// The table, fitted to a `w` × `h` box.
pub(super) fn stage(sc: &Scene<'_>, w: f32, h: f32) -> Div {
    let (ui, pal, tp) = (sc.ui, &sc.pal, &sc.tp);
    let i18n = &ui.i18n;
    let s = (w / STAGE.0).min(h / STAGE.1).clamp(SCALES.0, SCALES.1);
    let state = sc.state();
    let n = state.players().len();
    // A new deal animates even where it looks like the last one.
    let deal = {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for pl in state.players() {
            pl.hole.hash(&mut hash);
        }
        hash.finish()
    };
    let street = state.street();
    let mut layers: Vec<AnyElement> = Vec::new();

    // The felt.
    layers.push(
        div()
            .absolute()
            .left(px(92. * s))
            .top(px(70. * s))
            .w(px(716. * s))
            .h(px(330. * s))
            .rounded(px(165. * s))
            .p(px(9. * s))
            .bg(tp.oval_rim)
            .border_1()
            .border_color(pal.line2)
            .shadow(kit::shadow(tp.shadow, 24. * s, 50. * s))
            .child(
                div()
                    .relative()
                    .size_full()
                    .rounded(px(156. * s))
                    .border_2()
                    .border_color(tp.felt_line)
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(tp.felt[0], 0.),
                        linear_color_stop(tp.felt[1], 1.),
                    ))
                    .child(
                        div()
                            .absolute()
                            .inset(px(32. * s))
                            .rounded(px(122. * s))
                            .border_1()
                            .border_dashed()
                            .border_color(tp.felt_dash),
                    ),
            )
            .into_any_element(),
    );

    // The pot and the board.
    layers.push(
        centered(POT.0, POT.1, 200., s)
            .child(motion::fade_in(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        tracked(&i18n.tr("cockpit-pot").to_uppercase(), 10.5 * s, 0.08)
                            .text_color(tp.pot_label),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .child(
                                tracked(&bb(ui, state.pot()), 24. * s, -0.02)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .line_height(px(31. * s)),
                            )
                            .child(
                                div()
                                    .ml(px(3. * s))
                                    .pb(px(4. * s))
                                    .font_family(MONO)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(12. * s))
                                    .text_color(tp.pot_unit)
                                    .child(i18n.tr("cockpit-unit-bb")),
                            ),
                    ),
                anim_id(format!("ck_pot_{deal}_{}", state.pot())),
            ))
            .into_any_element(),
    );
    let board = state.board();
    layers.push(
        centered(BOARD.0, BOARD.1, 320., s)
            .gap(px(6. * s))
            .children((0..5).map(|i| {
                match board.get(i) {
                    Some(&c) => motion::deal_in(
                        tile(Some(c), Tile::Large, s, tp),
                        anim_id(format!("ck_board_{deal}_{i}_{c}")),
                        14. * s,
                    ),
                    None => div()
                        .flex_none()
                        .w(px(56. * s))
                        .h(px(76. * s))
                        .rounded(px(6. * s))
                        .border_1()
                        .border_dashed()
                        .border_color(tp.slot)
                        .into_any_element(),
                }
            }))
            .into_any_element(),
    );

    for seat in 0..n {
        let k = (seat + n - sc.hero) % n;
        let at = pod_at(k, n);
        let (hand_at, bet_at, dealer_at) = spots(k, at);
        let pl = &state.players()[seat];
        let hero = seat == sc.hero;

        // The seat's last action so far: a fold from any street, anything
        // else from this street.
        let last = sc
            .evs
            .iter()
            .rev()
            .filter(|e| e.step <= sc.at && e.seat == seat)
            .find(|e| e.action == Action::Fold || e.street == street);

        // Cards: hero's and shown hands face up, the rest face down.
        if !pl.folded {
            let show = hero || sc.reveal;
            let size = if hero { Tile::Large } else { Tile::Small };
            let cards: Vec<Option<Card>> = pl.hole.cards().map(|c| show.then_some(c)).to_vec();
            layers.push(
                centered(hand_at.0, hand_at.1, 140., s)
                    .gap(px(5. * s))
                    .children(cards.into_iter().enumerate().map(|(i, c)| {
                        let real = pl.hole.cards()[i];
                        motion::deal_in(
                            tile(c, size, s, tp),
                            anim_id(format!("ck_deal_{deal}_{seat}_{i}_{real}")),
                            14. * s,
                        )
                    }))
                    .into_any_element(),
            );
        }

        // What the seat has in front of it on this street, or what it won.
        let won = sc.wins.iter().find(|(w, _)| *w == seat).map(|(_, c)| *c);
        if let Some(c) = won {
            layers.push(
                centered(bet_at.0, bet_at.1, 120., s)
                    .child(motion::fade_in(
                        bet(format!("+{}", bb(ui, c)), pal.amber, sc, s).text_color(pal.amber),
                        anim_id(format!("ck_win_{deal}_{seat}_{c}")),
                    ))
                    .into_any_element(),
            );
        } else if pl.street_commit > 0 && sc.wins.is_empty() {
            let chip = match last.map(|e| e.action) {
                Some(Action::Call) => tp.call,
                _ => pal.amber,
            };
            let c = pl.street_commit;
            layers.push(
                centered(bet_at.0, bet_at.1, 120., s)
                    .child(motion::fade_in(
                        bet(bb(ui, c), chip, sc, s),
                        anim_id(format!("ck_bet_{deal}_{seat}_{street:?}_{c}")),
                    ))
                    .into_any_element(),
            );
        }

        if seat == state.button() {
            layers.push(
                centered(dealer_at.0, dealer_at.1, 40., s)
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(20. * s))
                            .rounded_full()
                            .bg(tp.dealer)
                            .text_color(tp.dealer_text)
                            .font_family(MONO)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11. * s))
                            .child("D"),
                    )
                    .into_any_element(),
            );
        }

        layers.push(
            centered(at.0, at.1, POD_W + 8., s)
                .child(pod(sc, seat, last, s))
                .into_any_element(),
        );

        // The profile a bot seat plays: over the top seat, under the
        // others (the side seats' cards sit over their pods).
        let tag_dy = if side(k, at) == Side::Top { -46. } else { 46. };
        if let Some(tag) = sc.tags.get(seat).and_then(Option::as_ref) {
            layers.push(
                centered(at.0, at.1 + tag_dy, POD_W, s)
                    .child(
                        div()
                            .flex_none()
                            .px(px(6. * s))
                            .rounded(px(3. * s))
                            .border_1()
                            .border_color(pal.line2)
                            .bg(pal.panel2)
                            .font_family(MONO)
                            .text_size(px(10. * s))
                            .text_color(pal.amber)
                            .whitespace_nowrap()
                            .child(SharedString::from(tag.clone())),
                    )
                    .into_any_element(),
            );
        }
    }

    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(STAGE.0 * s))
                .h(px(STAGE.1 * s))
                .children(layers),
        )
}

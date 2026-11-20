//! Salão's felt: the table under the lamp (rail, seats, chips, cards) and
//! the verdict on a decision. Every position and size is the mockup's
//! (`design-directions/src/salao-mesa.html`), in the coordinates of its
//! "zone", times a scale.

use gpui::{AnyElement, Div, FontWeight, div, prelude::*, px};
use taful_app::model::Grade;
use taful_app::preflop::{Action, BB_CHIPS, Card, Chips, GameState, Position, Seat};

use super::colors::colors;
use super::kit::{self, CardSize, Face};
use crate::Ui;
use crate::felt::{Explain, SeatExtras, TABLE_H, action_short, anim_id, bb, grade_key, num};
use crate::motion;
use crate::theme::{Palette, alpha};

/// The drawing's box in the mockup's pixels: its zone from just above the
/// top seat to just below hero's.
pub const W: f32 = 900.;
pub const H: f32 = 538.;
/// Where that box starts in the zone.
pub const TOP: f32 = 28.;
/// The side panel's width.
pub const PANEL_W: f32 = 404.;
/// A table that stands alone (the review screens) is drawn this many
/// times the size Classic gives it: the pills and their type need it.
pub const FELT_SCALE: f32 = 1.4;

/// Where a seat and its things sit: centers, in zone coordinates.
#[derive(Clone, Copy)]
struct Place {
    seat: (f32, f32),
    hole: (f32, f32),
    /// The chips bet on this street, and a winner's take.
    bet: (f32, f32),
    /// The dealer button.
    dealer: (f32, f32),
}

/// The pot's and the board's centers.
const POT: (f32, f32) = (450., 170.);
const BOARD: (f32, f32) = (450., 268.);
/// The rail's center.
const CENTER: (f32, f32) = (450., 276.);

/// Six seats, hero first and then clockwise from hero's left: the mockup's
/// (it shows hero's and the bottom left one's chips and cards; the others
/// mirror them).
const SIX: [Place; 6] = [
    Place {
        seat: (450., 528.),
        hole: (450., 448.),
        bet: (352., 372.),
        dealer: (556., 396.),
    },
    Place {
        seat: (88., 384.),
        hole: (88., 330.),
        bet: (232., 336.),
        dealer: (196., 392.),
    },
    Place {
        seat: (88., 168.),
        hole: (88., 222.),
        bet: (232., 204.),
        dealer: (196., 160.),
    },
    Place {
        seat: (450., 62.),
        hole: (450., 116.),
        bet: (540., 116.),
        dealer: (366., 116.),
    },
    Place {
        seat: (812., 168.),
        hole: (812., 222.),
        bet: (668., 204.),
        dealer: (704., 160.),
    },
    Place {
        seat: (812., 384.),
        hole: (812., 330.),
        bet: (668., 336.),
        dealer: (704., 392.),
    },
];

/// The seats of a table of `n`, hero first and then clockwise from hero's
/// left. Other sizes than six go around the rail.
fn places(n: usize) -> Vec<Place> {
    if n == SIX.len() {
        return SIX.to_vec();
    }
    (0..n)
        .map(|i| {
            if i == 0 {
                return SIX[0];
            }
            let angle = std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
            let (dx, dy) = (362. * angle.cos(), 214. * angle.sin());
            let seat = (CENTER.0 + dx, CENTER.1 + dy);
            // Cards on the side facing the middle of the table.
            let inward = if dy > 0. { -54. } else { 54. };
            Place {
                seat,
                hole: (seat.0, seat.1 + inward),
                bet: (CENTER.0 + 0.6 * dx, CENTER.1 + 0.62 * dy),
                dealer: (CENTER.0 + 0.7 * dx, seat.1 - inward * 0.15),
            }
        })
        .collect()
}

/// `el` centered on a point: GPUI has no `translate(-50%, -50%)`, so a box
/// of no size centers what overflows it. `el` must not shrink.
fn centered((x, y): (f32, f32), el: impl IntoElement) -> Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(0.))
        .h(px(0.))
        .flex()
        .items_center()
        .justify_center()
        .child(el)
}

/// The rail and the felt in the drawing's box, `k` times the mockup's
/// size, with the drop shadows the image does not carry.
pub fn base(k: f32) -> Div {
    let c = colors();
    let (x, y) = (60. * k, (66. - TOP) * k);
    div()
        .relative()
        .flex_none()
        .w(px(W * k))
        .h(px(H * k))
        .child(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(780. * k))
                .h(px(420. * k))
                .rounded(px(210. * k))
                .shadow(vec![
                    kit::shadow(alpha(c.black, 0.6), 0., 34. * k, 60. * k, 0.),
                    kit::shadow(alpha(c.black, 0.5), 0., 6. * k, 14. * k, 0.),
                ]),
        )
        .child(
            kit::image(kit::IMG_TABLE, 780. * k, 420. * k)
                .absolute()
                .left(px(x))
                .top(px(y)),
        )
}

/// What each seat last did on this street, a call with what it paid.
fn last_actions(ui: &Ui, state: &GameState) -> Vec<Option<String>> {
    let i18n = &ui.i18n;
    let n = state.players().len();
    let mut commit: Vec<Chips> = vec![0; n];
    let mut bet: Chips = 0;
    if state.actions().len() == 1 {
        // Preflop the blinds are in front before anyone acts.
        let blinds = &state.config().blinds;
        for (seat, c) in commit.iter_mut().enumerate() {
            *c = match state.position(seat) {
                Position::Sb => blinds.sb,
                Position::Bb => blinds.bb,
                _ => 0,
            };
        }
        bet = blinds.bb;
    }
    let preflop = state.actions().len() == 1;
    let mut raises = 0;
    let mut out = vec![None; n];
    for &(seat, a) in state.actions().last().into_iter().flatten() {
        // What the seat has in front in the end: its last action's size
        // when that was an all-in (or a call for less).
        let front = state.players()[seat].street_commit;
        let label = match a {
            Action::Call => {
                let paid = bet.min(front.max(commit[seat])) - commit[seat];
                commit[seat] += paid;
                if paid > 0 {
                    i18n.tr_args("table-action-call", &[("amount", bb(ui, paid).into())])
                } else {
                    action_short(ui, a)
                }
            }
            Action::Bet(to) | Action::Raise(to) => {
                commit[seat] = to;
                bet = to;
                raises += 1;
                // A preflop raise is an open or an n-bet, as in the story.
                match a {
                    Action::Raise(_) if preflop && raises == 1 => {
                        i18n.tr_args("table-tl-open", &[("amount", bb(ui, to).into())])
                    }
                    Action::Raise(_) if preflop => i18n.tr_args(
                        "table-tl-nbet",
                        &[("n", (raises + 1).into()), ("amount", bb(ui, to).into())],
                    ),
                    _ => action_short(ui, a),
                }
            }
            Action::AllIn => {
                commit[seat] = front;
                bet = bet.max(front);
                i18n.tr_args("table-action-allin", &[("amount", bb(ui, front).into())])
            }
            Action::Fold | Action::Check => action_short(ui, a),
        };
        out[seat] = Some(label);
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum SeatKind {
    Hero,
    Live,
    Folded,
}

/// What a seat's pill says.
struct SeatText {
    position: &'static str,
    stack: String,
    unit: String,
    /// The last action, or who sits there.
    line: String,
    /// Whether `line` is an action of this street.
    acted: bool,
}

/// A seat: the position on a disc, the stack and the last action.
fn seat_pill(p: &Palette, k: f32, kind: SeatKind, text: SeatText) -> Div {
    let SeatText {
        position,
        stack,
        unit,
        line,
        acted,
    } = text;
    let c = colors();
    let folded = kind == SeatKind::Folded;
    let small = if folded {
        c.seat_fold_muted
    } else {
        p.text_muted
    };
    let avatar = div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(40. * k))
        .rounded(px(20. * k));
    let avatar = match kind {
        SeatKind::Hero => avatar
            .text_color(p.on_accent)
            .bg(p.accent)
            .shadow(vec![kit::shadow(
                alpha(c.black, 0.4),
                0.,
                2. * k,
                4. * k,
                0.,
            )])
            .child(
                kit::image(kit::IMG_AVATAR_HERO, 40. * k, 40. * k)
                    .absolute()
                    .top_0()
                    .left_0(),
            ),
        SeatKind::Live => avatar
            .text_color(p.text)
            .bg(kit::vgrad(c.avatar_top, c.avatar_bottom))
            .shadow(vec![kit::shadow(
                alpha(c.black, 0.4),
                0.,
                2. * k,
                4. * k,
                0.,
            )])
            .child(kit::top_light(20. * k, alpha(c.white, 0.12))),
        SeatKind::Folded => avatar.text_color(c.avatar_fold_text).bg(c.avatar_fold),
    }
    .child(
        kit::SANS_B02
            .text(12.5 * k, kit::LEADING)
            .relative()
            .child(position),
    );
    let stack_face = (kit::STACK, 18., 1.1);
    let unit_face = (kit::SANS, 11.5, 1.1);
    let text = div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_end()
                .child(kit::STACK.text(18. * k, 1.1).child(stack))
                .child(
                    kit::SANS
                        .text(11.5 * k, 1.1)
                        .ml(px(3. * k))
                        .mb(px(Face::lift(stack_face, unit_face) * k))
                        .text_color(small)
                        .child(unit),
                ),
        )
        .child(
            kit::sans_text(12. * k, FontWeight::NORMAL)
                .min_h(px(12. * kit::LEADING * k))
                .text_color(if folded {
                    small
                } else if acted {
                    c.gold2
                } else {
                    p.text_muted
                })
                .child(line),
        );
    let pill = div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10. * k))
        .h(px(54. * k))
        .min_w(px(150. * k))
        .pl(px(7. * k))
        .pr(px(18. * k))
        .rounded(px(30. * k))
        .border_1()
        .whitespace_nowrap();
    match kind {
        SeatKind::Hero => pill
            .text_color(p.text)
            .bg(kit::vgrad(c.seat_top, c.seat_bottom))
            .border_color(alpha(p.accent, 0.75))
            .shadow(vec![
                kit::shadow(alpha(c.black, 0.55), 0., 10. * k, 24. * k, 0.),
                kit::shadow(alpha(p.accent, 0.25), 0., 0., 26. * k, 0.),
            ])
            // The mockup's three-pixel ring around the border.
            .child(
                div()
                    .absolute()
                    .top(px(-3. * k - 1.))
                    .left(px(-3. * k - 1.))
                    .right(px(-3. * k - 1.))
                    .bottom(px(-3. * k - 1.))
                    .rounded(px(33. * k))
                    .border(px(3. * k))
                    .border_color(alpha(p.accent, 0.14)),
            ),
        SeatKind::Live => pill
            .text_color(p.text)
            .bg(kit::vgrad(c.seat_top, c.seat_bottom))
            .border_color(alpha(p.text, 0.1))
            .shadow(vec![kit::shadow(
                alpha(c.black, 0.55),
                0.,
                10. * k,
                22. * k,
                0.,
            )])
            .child(kit::top_light(30. * k, alpha(c.white, 0.07))),
        SeatKind::Folded => pill
            .text_color(c.dim)
            .bg(kit::vgrad(c.seat_fold_top, c.seat_fold_bottom))
            .border_color(alpha(p.text, 0.05))
            .shadow(vec![kit::shadow(
                alpha(c.black, 0.4),
                0.,
                6. * k,
                14. * k,
                0.,
            )]),
    }
    .child(avatar)
    .child(text)
}

/// Chips and their amount, as a bet in front of a seat.
fn chips_and_amount(p: &Palette, k: f32, chips: Chips, amount: String, win: bool) -> Div {
    let c = colors();
    div()
        .flex()
        .flex_none()
        .items_end()
        .gap(px(7. * k))
        .children(
            kit::chip_stacks(chips as f64 / BB_CHIPS as f64)
                .into_iter()
                .map(|(value, n)| kit::chip_stack(value, n, k)),
        )
        .child(
            div()
                .flex_none()
                .px(px(10. * k))
                .pt(px(k))
                .pb(px(2. * k))
                .rounded(px(99.))
                .bg(alpha(c.bet_bg, 0.6))
                .whitespace_nowrap()
                .child(
                    kit::BET
                        .text(17. * k, kit::LEADING)
                        .text_color(if win { c.gold2 } else { p.text })
                        .child(amount),
                ),
        )
}

/// Changes with every deal, so a new hand's blinds, pot and cards animate
/// even where they equal the last hand's.
fn deal_key(state: &GameState) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for pl in state.players() {
        pl.hole.hash(&mut h);
    }
    h.finish()
}

/// The table: rail, pot, board, seats with their cards and chips, `k`
/// times the mockup's size in a [`W`] × [`H`] box. `reveal` shows every
/// live hand (end of hand).
pub fn draw(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    k: f32,
    extra: SeatExtras<'_>,
) -> Div {
    let show = Show {
        reveal,
        hero_folded: false,
    };
    draw_with(ui, p, state, hero, show, k, extra)
}

/// [`draw`] with hero shown as having folded at `state`. (The state after
/// a fold that ends the hand has the pot already paid and the uncalled
/// bet returned: not what the table looked like when hero gave up.)
pub fn draw_folded(ui: &Ui, p: &Palette, state: &GameState, hero: Seat, k: f32) -> Div {
    let show = Show {
        reveal: false,
        hero_folded: true,
    };
    draw_with(ui, p, state, hero, show, k, SeatExtras::default())
}

#[derive(Clone, Copy)]
struct Show {
    reveal: bool,
    hero_folded: bool,
}

fn draw_with(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    show: Show,
    k: f32,
    extra: SeatExtras<'_>,
) -> Div {
    let SeatExtras { tags, wins } = extra;
    let Show {
        reveal,
        hero_folded,
    } = show;
    let folded = |seat: Seat| state.players()[seat].folded || (hero_folded && seat == hero);
    let c = colors();
    let i18n = &ui.i18n;
    let n = state.players().len();
    let places = places(n);
    let place = |seat: Seat| places[(seat + n - hero) % n];
    // Zone coordinates to the box's pixels.
    let at = |(x, y): (f32, f32)| (x * k, (y - TOP) * k);
    let deal = deal_key(state);
    let last = last_actions(ui, state);
    let unit = i18n.tr("salao-bb");

    let pot = {
        let (big, small) = ((kit::POT, 30., 1.), (kit::SANS, 13., 1.));
        let el = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10. * k))
            .whitespace_nowrap()
            .children(
                kit::chip_stacks(state.pot() as f64 / BB_CHIPS as f64)
                    .into_iter()
                    .map(|(value, count)| kit::chip_stack(value, count, k)),
            )
            .child(
                div()
                    .ml(px(6. * k))
                    .child(kit::label(
                        kit::SANS_R18,
                        10.5 * k,
                        alpha(p.text, 0.7),
                        i18n.tr("salao-pot"),
                    ))
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .child(
                                kit::POT
                                    .text(30. * k, 1.)
                                    .text_color(p.text)
                                    .child(bb(ui, state.pot())),
                            )
                            .child(
                                kit::SANS
                                    .text(13. * k, 1.)
                                    .ml(px(4. * k))
                                    .mb(px(Face::lift(big, small) * k))
                                    .text_color(alpha(p.text, 0.75))
                                    .child(unit.clone()),
                            ),
                    ),
            );
        motion::fade_in(
            centered(at(POT), el),
            anim_id(format!("pot_{deal}_{}", state.pot())),
        )
    };

    let board = centered(
        at(BOARD),
        div()
            .flex()
            .flex_none()
            .gap(px(8. * k))
            .children((0..5).map(|i| {
                match state.board().get(i) {
                    Some(&card) => motion::deal_in(
                        kit::playing_card(p, Some(card), CardSize::Lg, k),
                        anim_id(format!("board_{deal}_{i}_{card}")),
                        14. * k,
                    ),
                    None => div()
                        .flex_none()
                        .w(px(CardSize::Lg.width() * k))
                        .h(px(CardSize::Lg.height() * k))
                        .rounded(px(8. * k))
                        .border(px(1.5 * k))
                        .border_dashed()
                        .border_color(alpha(p.text, 0.16))
                        .into_any_element(),
                }
            })),
    );

    let dealer = {
        let (x, y) = at(place(state.button()).dealer);
        // The disc sits at (10, 5) in the image, among its shadows.
        kit::image(kit::IMG_DEALER, 46. * k, 44. * k)
            .absolute()
            .left(px(x - 23. * k))
            .top(px(y - 18. * k))
    };

    // Hole cards. Keyed by the real card: a new hand deals again, turning
    // a showdown hand face up does not.
    let hole = |seat: Seat| -> Option<Div> {
        let pl = &state.players()[seat];
        let is_hero = seat == hero;
        if folded(seat) && !is_hero {
            return None;
        }
        let dealt = |i: usize, el: Div| {
            let real = pl.hole.cards()[i];
            motion::deal_in(
                el,
                anim_id(format!("deal_{deal}_{seat}_{i}_{real}")),
                14. * k,
            )
        };
        let cards: Vec<Card> = pl.hole.cards().to_vec();
        let row = div().flex().flex_none();
        let row =
            if is_hero {
                row.gap(px(6. * k))
                    .when(folded(seat), |d| d.opacity(0.45))
                    .children(cards.iter().enumerate().map(|(i, &card)| {
                        dealt(i, kit::playing_card(p, Some(card), CardSize::Lg, k))
                    }))
            } else if reveal {
                row.gap(px(2. * k)).children(
                    cards.iter().enumerate().map(|(i, &card)| {
                        dealt(i, kit::playing_card(p, Some(card), CardSize::Sm, k))
                    }),
                )
            } else {
                // Two backs, the second over the first's last 14 pixels (the
                // first's slot is that much narrower than the card).
                row.children((0..2).map(|i| {
                    let lap = if i == 0 { 14. } else { 0. };
                    div()
                        .flex_none()
                        .w(px((CardSize::Sm.width() - lap) * k))
                        .child(dealt(i, kit::playing_card(p, None, CardSize::Sm, k)))
                }))
            };
        Some(centered(at(place(seat).hole), row))
    };

    let seat_el = |seat: Seat| -> Div {
        let pl = &state.players()[seat];
        let is_hero = seat == hero;
        let action = if folded(seat) {
            Some(i18n.tr("table-action-fold"))
        } else {
            last[seat].clone()
        };
        let acted = action.is_some() && !folded(seat);
        let line = match (action, is_hero) {
            (Some(a), true) => i18n.tr_args("salao-seat-you", &[("action", a.into())]),
            (None, true) => i18n.tr("salao-you"),
            (Some(a), false) => a,
            (None, false) => String::new(),
        };
        let kind = if is_hero {
            SeatKind::Hero
        } else if folded(seat) {
            SeatKind::Folded
        } else {
            SeatKind::Live
        };
        centered(
            at(place(seat).seat),
            seat_pill(
                p,
                k,
                kind,
                SeatText {
                    position: state.position(seat).label(),
                    stack: bb(ui, pl.stack),
                    unit: unit.clone(),
                    line,
                    acted,
                },
            ),
        )
    };

    // The bot profile a seat plays: a tab on the seat's outer edge.
    let tag_el = |seat: Seat| -> Option<Div> {
        let tag = tags.get(seat)?.as_ref()?;
        let (x, y) = place(seat).seat;
        let y = if y < CENTER.1 { y - 27. } else { y + 27. };
        Some(centered(
            at((x, y)),
            div()
                .flex_none()
                .px(px(9. * k))
                .py(px(k))
                .rounded(px(99.))
                .bg(c.bet_bg)
                .border_1()
                .border_color(alpha(p.accent, 0.3))
                .whitespace_nowrap()
                .child(
                    kit::sans_text(11. * k, FontWeight::MEDIUM)
                        .text_color(c.gold2)
                        .child(tag.clone()),
                ),
        ))
    };

    // Chips in front of each seat on this street, slid from the seat on
    // every new amount. (Not once the winners' takes are shown: those sit
    // in the same places.)
    let street = state.street();
    let bets = (0..n).filter_map(|seat| {
        let chips = state.players()[seat].street_commit;
        if chips == 0 || !wins.is_empty() {
            return None;
        }
        let spot = place(seat);
        Some(motion::slide(
            centered(
                (0., 0.),
                chips_and_amount(p, k, chips, bb(ui, chips), false),
            ),
            anim_id(format!("bet_{deal}_{seat}_{street:?}_{chips}")),
            at(spot.seat),
            at(spot.bet),
        ))
    });
    // What each winner took, slid from the pot.
    let win_els = wins.iter().map(|&(seat, chips)| {
        motion::slide(
            centered(
                (0., 0.),
                chips_and_amount(p, k, chips, format!("+{}", bb(ui, chips)), true),
            ),
            anim_id(format!("win_{deal}_{seat}_{chips}")),
            at(POT),
            at(place(seat).bet),
        )
    });

    let others = (0..n).filter(|seat| *seat != hero);
    base(k)
        .child(pot)
        .child(board)
        .child(dealer)
        .children(others.clone().filter_map(hole))
        .children(others.clone().map(seat_el))
        .children(others.filter_map(tag_el))
        .children(bets.collect::<Vec<AnyElement>>())
        .children(hole(hero))
        .child(seat_el(hero))
        .children(win_els.collect::<Vec<AnyElement>>())
}

// ---- hooks ---------------------------------------------------------------------

/// The table in the box Classic's takes at scale `s`.
pub fn felt_table(
    ui: &Ui,
    p: &Palette,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    s: f32,
    extra: SeatExtras<'_>,
) -> Div {
    draw(ui, p, state, hero, reveal, s * TABLE_H / H, extra)
}

/// A card as wide as Classic's at scale `s`.
pub fn felt_card(_ui: &Ui, p: &Palette, c: Option<Card>, s: f32) -> Div {
    kit::playing_card(p, c, CardSize::Sm, s * 26. / CardSize::Sm.width())
}

/// The side panel next to a table.
pub fn felt_panel(ui: &Ui, p: &Palette) -> Div {
    kit::panel(p)
        .w(px(PANEL_W))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(ui.theme.spacing.lg))
        .px(px(22.))
        .pt(px(22.))
        .pb(px(18.))
}

/// A section's small heading inside the side panel, with an optional note
/// at its right.
pub fn section(title: String, note: Option<String>) -> Div {
    let c = colors();
    div()
        .flex()
        .items_start()
        .justify_between()
        .child(kit::label(kit::SANS_R16, 10.5, c.dim, title))
        .children(note.map(|n| {
            kit::SANS_R04
                .text(12., kit::LEADING)
                .text_color(c.dim)
                .child(n)
        }))
}

fn seal(grade: Grade) -> &'static str {
    match grade {
        Grade::Best => "salao/img/seal_best.png",
        Grade::Correct => "salao/img/seal_correct.png",
        Grade::Inaccuracy => "salao/img/seal_inaccuracy.png",
        Grade::Mistake => "salao/img/seal_mistake.png",
        Grade::Blunder => "salao/img/seal_blunder.png",
    }
}

/// An action's name; a call with its size when the table is at hand.
pub fn action_name(ui: &Ui, a: Action, to_call: Option<Chips>) -> String {
    match (a, to_call) {
        (Action::Call, Some(c)) if c > 0 => ui
            .i18n
            .tr_args("table-action-call", &[("amount", bb(ui, c).into())]),
        _ => action_short(ui, a),
    }
}

/// The verdict on one decision: the grade on its seal, what was played,
/// the EV it lost, and every action with the reference's frequency and
/// its EV.
pub fn explanation(ui: &Ui, p: &Palette, d: &Explain<'_>) -> Div {
    let c = colors();
    let i18n = &ui.i18n;
    let grade = p.grade(d.grade);
    let best = d.best();
    let name = |i: usize| {
        d.actions
            .get(i)
            .map_or("–".into(), |&a| action_name(ui, a, d.to_call))
    };

    // "Call 10 with Q♠ J♠": the suits as marks in their own colors.
    let played = i18n.tr_args("salao-verdict-with", &[("action", name(d.chosen).into())]);
    let cards: Option<Vec<Card>> = d.context.as_ref().and_then(|(_, hand)| {
        // Two cards of two ASCII characters each.
        (hand.len() == 4)
            .then(|| [hand.get(..2), hand.get(2..)])
            .and_then(|v| v.into_iter().map(|c| c?.parse().ok()).collect())
    });
    let what = div()
        .mt(px(5.))
        .flex()
        .items_start()
        .text_color(p.text_muted)
        .whitespace_nowrap();
    let piece = |s: String| kit::sans_text(13., FontWeight::NORMAL).child(s);
    let what = match (&cards, &d.context) {
        (Some(cards), _) => cards.iter().enumerate().fold(what, |row, (i, card)| {
            let rank = card.rank().to_char();
            row.child(piece(if i == 0 {
                format!("{played} {rank}")
            } else {
                format!(" {rank}")
            }))
            .child(
                kit::icon(
                    kit::suit_path(card.suit()),
                    11.17,
                    kit::suit_light(c, card.suit()),
                )
                .ml(px(0.5))
                .mt(px(2.86)),
            )
        }),
        (None, Some((_, hand))) => what.child(piece(format!("{played} {hand}"))),
        (None, None) => what.child(piece(name(d.chosen))),
    };

    let verdict = div()
        .flex()
        .items_center()
        .pb(px(18.))
        .border_b_1()
        .border_color(c.line)
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(60.))
                .rounded(px(30.))
                .shadow(vec![kit::shadow(alpha(grade, 0.35), 0., 0., 30., 0.)])
                .child(
                    div()
                        .absolute()
                        .top(px(-4.))
                        .left(px(-4.))
                        .size(px(68.))
                        .rounded(px(34.))
                        .border(px(4.))
                        .border_color(alpha(grade, 0.16)),
                )
                .child(
                    kit::image(seal(d.grade), 60., 60.)
                        .absolute()
                        .top_0()
                        .left_0(),
                ),
        )
        .child(
            div()
                .ml(px(16.))
                .flex_none()
                .child(
                    kit::H1
                        .text(34., 1.)
                        .text_color(grade)
                        .whitespace_nowrap()
                        .child(i18n.tr(&grade_key(d.grade))),
                )
                .child(what),
        )
        .child(div().flex_1().min_w(px(16.)))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .items_end()
                .whitespace_nowrap()
                // The label sits on the baseline of a body line: 14 px
                // down, which this leading gives it.
                .child(
                    kit::SANS_R14
                        .text(10.5, 2.)
                        .text_color(c.dim)
                        .child(i18n.tr("salao-ev-loss").to_uppercase()),
                )
                .child(kit::LOSS.text(24., kit::LEADING).child(format!(
                    "{} {}",
                    num(ui, d.ev_loss_bb, 2),
                    i18n.tr("salao-bb")
                ))),
        );

    let rows = d.actions.iter().enumerate().map(|(i, _)| {
        let freq = d.hand_freqs.get(i).copied();
        // After a wrong answer, the action it should have been.
        let wrong = !matches!(d.grade, Grade::Best | Grade::Correct);
        let mark = if i == d.chosen {
            Some((i18n.tr("salao-option-yours"), grade))
        } else if i == best && wrong {
            Some((i18n.tr("salao-option-best"), p.grade(Grade::Best)))
        } else {
            None
        };
        div()
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .when(i == d.chosen, |r| {
                r.bg(alpha(grade, 0.09))
                    .child(kit::ring(8., alpha(grade, 0.35)))
            })
            .child(
                div()
                    .relative()
                    .w(px(104.))
                    .flex_none()
                    .whitespace_nowrap()
                    .child(kit::sans_text(13.5, FontWeight::NORMAL).child(name(i)))
                    .children(mark.map(|(text, color)| {
                        kit::sans_text(11., FontWeight::NORMAL)
                            .mt(px(-1.))
                            .text_color(color)
                            .child(text)
                    })),
            )
            .child(
                kit::bar(
                    freq.unwrap_or(0.),
                    8.,
                    alpha(p.text, 0.07),
                    c.bar_start,
                    c.gold2,
                )
                .relative()
                .flex_1(),
            )
            .child(
                div()
                    .relative()
                    .w(px(44.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .child(
                        kit::sans_text(13.5, FontWeight::NORMAL)
                            .text_color(p.text_muted)
                            .child(freq.map_or("–".into(), |f| {
                                format!("{}%", num(ui, f64::from(f) * 100., 0))
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w(px(52.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .child(
                        kit::TEXT.text(16., kit::LEADING).child(
                            d.hand_evs
                                .get(i)
                                .map_or("–".into(), |e| num(ui, f64::from(*e), 2)),
                        ),
                    ),
            )
    });

    // Against a bot profile the EVs are against that bot: say so, with
    // the GTO line, the exploitative line and the gap between them (D8).
    let versus = d.vs_profile.map(|v| {
        let line = |key: &str, a: usize| i18n.tr_args(key, &[("action", name(a).into())]);
        let note = |s: String| {
            kit::sans_lines(12.5, FontWeight::NORMAL)
                .text_color(p.text_muted)
                .child(s)
        };
        div()
            .mt(px(10.))
            .px(px(10.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(note(if taful_app::mixed::is_table_key(&v.profile) {
                i18n.tr_args(
                    "table-vs-table",
                    &[("table", i18n.opponent(&v.profile).into())],
                )
            } else {
                i18n.tr_args(
                    "table-vs-profile",
                    &[("profile", i18n.opponent(&v.profile).into())],
                )
            }))
            .child(note(line("table-exploit-line", v.exploit)))
            .child(note(line("table-gto-line", v.gto)))
            .child(note(i18n.tr_args(
                "table-ev-gap",
                &[("amount", num(ui, v.ev_gap_bb, 2).into())],
            )))
    });

    div()
        .flex()
        .flex_col()
        .child(verdict)
        .child(
            section(
                i18n.tr("salao-options"),
                Some(i18n.tr(if d.vs_profile.is_some() {
                    "salao-options-note-profile"
                } else {
                    "salao-options-note"
                })),
            )
            .mt(px(18.))
            .mb(px(8.)),
        )
        .children(rows)
        .children(versus)
}

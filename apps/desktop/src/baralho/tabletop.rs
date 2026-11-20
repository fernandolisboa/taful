//! The table as Baralho draws it: the rail and its top, cream seat plates,
//! chips, the pot and the board, at the mockup's coordinates. Everything
//! sits in a [`W`] × [`H`] box; a caller with another box draws it inside
//! [`zoomed`].

use gpui::{Div, div, prelude::*};
use taful_app::model::Street;
use taful_app::preflop::{Action, BB_CHIPS, Card, Chips, GameState, Seat};
use taful_app::story::{Story, StoryAction, StoryEvent};
use taful_app::template::street_id;

use super::kit::{self, CardSize, Raised};
use super::paint::{self, Tex};
use super::{Bx, Tone, typo, u, zoomed};
use crate::Ui;
use crate::felt::{SeatExtras, anim_id, bb};
use crate::motion;
use crate::theme::{Palette, alpha, token_color};

/// The table's own canvas.
pub const W: f32 = 1010.;
pub const H: f32 = 584.;
/// The rail: left, top, width, height.
const RAIL: (f32, f32, f32, f32) = (84., 70., 842., 430.);
/// The middle of the table top.
const CENTER: (f32, f32) = (505., 285.);
const POT: (f32, f32) = (505., 172.);
const BOARD: (f32, f32) = (505., 274.);
/// Six seats, hero first, then clockwise from hero's left.
const SIX: [(f32, f32); 6] = [
    (505., 548.),
    (96., 396.),
    (96., 176.),
    (505., 66.),
    (914., 176.),
    (914., 396.),
];
/// A seat this high sits on the rail's top edge.
const TOP_ROW: f32 = 110.;

/// One action of a hand's story, in words.
pub struct Line {
    pub seat: Seat,
    pub who: String,
    pub what: String,
    pub street: Street,
    /// The story state right after it.
    pub step: usize,
    pub hero: bool,
    pub fold: bool,
    pub aggressive: bool,
}

/// One street of the story: its board cards, the pot it started with and
/// its actions.
pub struct Block {
    pub street: Street,
    pub cards: Vec<Card>,
    pub pot: Chips,
    pub lines: Vec<Line>,
}

/// An action in words: opens and 3-bets by name before the flop.
fn words(ui: &Ui, a: &StoryAction, raises: usize) -> String {
    let i18n = &ui.i18n;
    let amount = |c: Chips| bb(ui, c).into();
    match a.action {
        Action::Fold => i18n.tr("table-action-fold"),
        Action::Check => i18n.tr("table-action-check"),
        Action::Call => i18n.tr_args("table-action-call", &[("amount", amount(a.paid))]),
        Action::Bet(_) => i18n.tr_args("table-action-bet", &[("amount", amount(a.to))]),
        Action::Raise(_) if a.street == Street::Preflop && raises == 1 => {
            i18n.tr_args("table-tl-open", &[("amount", amount(a.to))])
        }
        Action::Raise(_) if a.street == Street::Preflop => i18n.tr_args(
            "table-tl-nbet",
            &[("n", (raises + 1).into()), ("amount", amount(a.to))],
        ),
        Action::Raise(_) => i18n.tr_args("table-action-raise", &[("amount", amount(a.to))]),
        Action::AllIn => i18n.tr_args("table-action-allin", &[("amount", amount(a.to))]),
    }
}

/// The story street by street. With `merge`, the folds before the first
/// voluntary action share one line (the log); without, every action is a
/// line of its own (the seats).
pub fn blocks(ui: &Ui, story: &Story, hero: Seat, merge: bool) -> Vec<Block> {
    let i18n = &ui.i18n;
    let mut out = vec![Block {
        street: Street::Preflop,
        cards: Vec::new(),
        pot: story.states[0].pot(),
        lines: Vec::new(),
    }];
    let mut raises = 0usize;
    let mut openers: Vec<&str> = Vec::new();
    let mut opened = false;
    for e in &story.events {
        let a = match e {
            StoryEvent::Dealt {
                street,
                cards,
                step,
            } => {
                out.push(Block {
                    street: *street,
                    cards: cards.clone(),
                    pot: story.states[*step].pot(),
                    lines: Vec::new(),
                });
                continue;
            }
            StoryEvent::Action(a) => a,
        };
        let Some(block) = out.last_mut() else {
            continue;
        };
        let is_hero = a.seat == hero;
        if merge && block.street == Street::Preflop && !opened && a.action == Action::Fold {
            openers.push(a.position.label());
            if let Some(l) = block.lines.last_mut().filter(|_| openers.len() > 1) {
                l.who = openers.join(", ");
                l.step = a.step;
                continue;
            }
        } else {
            opened = true;
        }
        if a.street == Street::Preflop && matches!(a.action, Action::Raise(_)) {
            raises += 1;
        }
        let who = if is_hero {
            i18n.tr_args(
                "table-story-you",
                &[("position", a.position.label().into())],
            )
        } else {
            a.position.label().to_string()
        };
        block.lines.push(Line {
            seat: a.seat,
            who,
            what: words(ui, a, raises),
            street: a.street,
            step: a.step,
            hero: is_hero,
            fold: a.action == Action::Fold,
            aggressive: a.action.is_aggressive(),
        });
    }
    out
}

/// What each seat's plate says: a fold from any street, else the seat's
/// last action on the street being played.
fn seat_lines(ui: &Ui, state: &GameState, hero: Seat) -> Vec<Option<Line>> {
    let mut out: Vec<Option<Line>> = state.players().iter().map(|_| None).collect();
    if let Ok(story) = Story::of(state) {
        for line in blocks(ui, &story, hero, false)
            .into_iter()
            .flat_map(|b| b.lines)
        {
            let seat = line.seat;
            if let Some(slot) = out.get_mut(seat) {
                *slot = Some(line);
            }
        }
    }
    for line in &mut out {
        if line
            .as_ref()
            .is_some_and(|l| !l.fold && l.street != state.street())
        {
            *line = None;
        }
    }
    out
}

/// Where seat `k` (0 = hero, then clockwise from hero's left) of `n` sits.
fn seat_at(k: usize, n: usize) -> (f32, f32) {
    if n == SIX.len() {
        return SIX[k % n];
    }
    if k == 0 {
        return SIX[0];
    }
    let angle = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32;
    let (c, s) = (angle.cos(), angle.sin());
    // The seats ride the rail: flat sides, so the cosine saturates.
    let x = CENTER.0 + 409. * (c * 1.25).clamp(-1., 1.);
    let y = CENTER.1 + if s >= 0. { 263. * s } else { 219. * s };
    (x, y)
}

/// Whether a point is left (-1) or right (1) of the table's middle.
fn side(x: f32) -> f32 {
    if x < CENTER.0 { -1. } else { 1. }
}

/// Where a seat's face-down cards go: on top of its plate, or beside it
/// for a seat on the rail's top edge.
fn hole_at((x, y): (f32, f32)) -> (f32, f32) {
    if y < TOP_ROW {
        (x + 117. * if x < CENTER.0 - 1. { -1. } else { 1. }, y - 4.)
    } else {
        (x, y - 56.)
    }
}

/// Where a seat's chips go, between the seat and the pot.
fn bet_at(k: usize, (x, y): (f32, f32)) -> (f32, f32) {
    if k == 0 {
        (x - 113., y - 170.)
    } else if y < TOP_ROW {
        (x, y + 58.)
    } else {
        (x + (CENTER.0 - x) * 0.38, y + (CENTER.1 - y) * 0.47)
    }
}

/// Where the dealer button goes for a seat.
fn dealer_at(k: usize, (x, y): (f32, f32)) -> (f32, f32) {
    if k == 0 {
        (x + 119., y - 148.)
    } else if y < TOP_ROW {
        (x - 105., y + 46.)
    } else {
        (
            x - 128. * side(x),
            y + if y < CENTER.1 { -44. } else { 44. },
        )
    }
}

/// A box centered on a point of the table: the CSS
/// `translate(-50%, -50%)`. [`center`] makes it, this places it.
fn at((x, y): (f32, f32), boxed: Div) -> Div {
    boxed.left(u(x - BOX.0 / 2.)).top(u(y - BOX.1 / 2.))
}

/// The box [`at`] places, bigger than anything it centers.
const BOX: (f32, f32) = (480., 240.);

fn center(child: impl IntoElement) -> Div {
    div()
        .absolute()
        .w(u(BOX.0))
        .h(u(BOX.1))
        .flex()
        .items_center()
        .justify_center()
        .child(child)
}

/// The left/top of [`at`]'s box in window pixels (for slides).
fn corner((x, y): (f32, f32)) -> (f32, f32) {
    (f32::from(u(x - BOX.0 / 2.)), f32::from(u(y - BOX.1 / 2.)))
}

/// The chips that make `amount`, largest first, with each chip's value in
/// big blinds.
fn chip_tones(ui: &Ui, amount: Chips) -> Vec<(u32, Tone)> {
    let c = &ui.theme.colors;
    let values = [
        (500, c.chip_500),
        (100, c.chip_100),
        (25, c.chip_25),
        (5, c.chip_5),
        (1, c.chip_1),
    ];
    // Whole big blinds; anything smaller is still one chip.
    let mut left = ((amount as f64 / BB_CHIPS as f64).round() as u32).max(1);
    let mut out = Vec::new();
    for (value, color) in values {
        while left >= value && out.len() < 24 {
            out.push((value, token_color(color)));
            left -= value;
        }
    }
    out
}

/// The most chips one stack shows.
const STACK: usize = 5;

/// One stack for a bet: the largest chips at the bottom.
fn bet_stack(ui: &Ui, bx: &Bx, amount: Chips) -> Div {
    let tones: Vec<Tone> = chip_tones(ui, amount)
        .into_iter()
        .take(STACK)
        .map(|(_, t)| t)
        .collect();
    kit::chips(bx, &tones)
}

/// The pot's chips: a stack per chip value, the largest first.
fn pot_stacks(ui: &Ui, bx: &Bx, amount: Chips) -> Vec<Div> {
    let mut stacks: Vec<(u32, Vec<Tone>)> = Vec::new();
    for (value, tone) in chip_tones(ui, amount) {
        match stacks.last_mut() {
            Some((v, s)) if *v == value => {
                if s.len() < STACK {
                    s.push(tone);
                }
            }
            _ => stacks.push((value, vec![tone])),
        }
    }
    stacks
        .iter()
        .take(3)
        .map(|(_, s)| kit::chips(bx, s))
        .collect()
}

/// An amount on a dark pill, next to chips.
fn amount_pill(bx: &Bx, text: String) -> Div {
    typo::disp(18., 800, 20)
        .lh(1.2)
        .on(div())
        .px(u(11.))
        .pt(u(1.))
        .pb(u(2.))
        .rounded_full()
        .bg(bx.deep)
        .text_color(bx.on_felt)
        .child(text)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Plate {
    Live,
    Folded,
    Hero,
}

/// A seat's plate: the position badge, the stack and the last action.
fn plate(
    bx: &Bx,
    label: &'static str,
    stack: String,
    line: Option<(String, Tone)>,
    kind: Plate,
    turn: bool,
) -> Div {
    let (face, edge, drop, ink, soft) = match kind {
        Plate::Live => (bx.cream, bx.deep, 5., bx.ink, bx.mut_ink),
        Plate::Hero => (bx.cream_hero, bx.deep, 5., bx.ink, bx.mut_ink),
        Plate::Folded => (
            bx.seat_fold,
            bx.seat_fold_edge,
            4.,
            bx.seat_fold_text,
            bx.seat_fold_mut,
        ),
    };
    let badge = typo::disp(14.5, 800, 20)
        .on(div())
        .flex_none()
        .size(u(42.))
        .rounded(u(11.))
        .flex()
        .items_center()
        .justify_center()
        .child(label);
    let badge = match kind {
        Plate::Live => badge.bg(bx.ink).text_color(bx.cream),
        Plate::Hero => badge
            .bg(bx.amber)
            .text_color(bx.ink)
            .border(u(2.))
            .border_color(bx.deep),
        Plate::Folded => badge.bg(bx.seat_fold_badge).text_color(bx.seat_fold_text),
    };
    let body = div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_baseline()
                .child(typo::disp(19., 800, 20).lh(1.05).on(div()).child(stack))
                .child(
                    typo::text(12., 600)
                        .lh(1.05)
                        .on(div())
                        .ml(u(3.))
                        .text_color(soft)
                        .child("bb"),
                ),
        )
        .children(line.map(|(text, tone)| {
            typo::text(12.5, 700)
                .on(div())
                .whitespace_nowrap()
                .text_color(if kind == Plate::Folded { soft } else { tone })
                .child(text)
        }));
    let seat = Raised {
        face,
        edge,
        radius: 16.,
        border: 2.5,
        drop,
    }
    .wrap(
        div()
            .flex()
            .items_center()
            .gap(u(10.))
            .pl(u(5.5))
            .pr(u(13.5))
            .text_color(ink)
            .child(badge)
            .child(body),
    )
    .min_w(u(146.))
    .h(u(58. + drop));
    if kind != Plate::Hero {
        if !turn {
            return seat;
        }
        // The seat to act: a thin amber ring around the plate.
        return div().relative().child(seat.relative()).child(
            div()
                .absolute()
                .top(u(-5.))
                .left(u(-5.))
                .right(u(-5.))
                .bottom(u(-5.))
                .rounded(u(21.))
                .border(u(2.))
                .border_color(bx.amber),
        );
    }
    // Hero's amber halo, around the plate and down to its drop.
    div()
        .relative()
        .child(
            div()
                .absolute()
                .top(u(-5.))
                .left(u(-5.))
                .right(u(-5.))
                .bottom(u(-5.))
                .rounded(u(21.))
                .bg(alpha(bx.amber, 0.5)),
        )
        .child(seat.relative())
}

/// The rail, the table top and the dashed line on it.
fn rail(bx: &Bx) -> Div {
    let (x, y, w, h) = RAIL;
    div()
        .absolute()
        .left(u(x))
        .top(u(y))
        .w(u(w))
        .h(u(h + 12.))
        .child(
            // The rail's hard drop, and the soft shadow under it.
            div()
                .absolute()
                .left_0()
                .top(u(12.))
                .w(u(w))
                .h(u(h))
                .rounded(u(215.))
                .bg(bx.rail_drop)
                .shadow(kit::soft_shadow(bx, 22., 40., 0.3)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .w(u(w))
                .h(u(h))
                .rounded(u(215.))
                .bg(bx.deep)
                .child(
                    div()
                        .absolute()
                        .inset(u(6.))
                        .rounded(u(209.))
                        .border(u(2.5))
                        .border_color(bx.amber),
                )
                .child(
                    paint::fill(Tex::TableTop, *bx, 201.)
                        .absolute()
                        .inset(u(14.)),
                )
                .child(
                    div()
                        .absolute()
                        .inset(u(58.))
                        .rounded(u(160.))
                        .border(u(2.5))
                        .border_dashed()
                        .border_color(alpha(bx.cream, 0.2)),
                ),
        )
}

/// The table with nothing on it and a line of text in the middle (no spot
/// to show, or one being dealt).
pub fn empty(bx: &Bx, text: Option<String>) -> Div {
    div()
        .relative()
        .flex_none()
        .w(u(W))
        .h(u(H))
        .child(rail(bx))
        .children(text.map(|t| {
            at(
                CENTER,
                center(
                    typo::text(16., 600)
                        .on(div())
                        .max_w(u(440.))
                        .text_center()
                        .text_color(bx.on_felt)
                        .child(t),
                ),
            )
        }))
}

/// The table at `state`, seen from `hero`'s seat. `reveal` shows every
/// live hand (the end of a hand); `said` is what hero's plate says when it
/// is not the last action of `state`; `turn` rings the seat to act.
#[allow(clippy::too_many_arguments)]
pub fn table(
    ui: &Ui,
    p: &Palette,
    bx: &Bx,
    state: &GameState,
    hero: Seat,
    reveal: bool,
    extra: SeatExtras<'_>,
    said: Option<String>,
    turn: bool,
) -> Div {
    let SeatExtras { tags, wins } = extra;
    let i18n = &ui.i18n;
    let n = state.players().len();
    let slot = |seat: Seat| (seat + n - hero) % n;
    // Changes with every deal, so a new hand's cards and chips animate even
    // where they equal the last hand's.
    let deal = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for pl in state.players() {
            pl.hole.hash(&mut h);
        }
        h.finish()
    };
    let lines = seat_lines(ui, state, hero);
    let drop_in = f32::from(u(14.));
    let mut out = div().relative().flex_none().w(u(W)).h(u(H)).child(rail(bx));

    // The pot: its chips, its name and its size.
    let pot = bb(ui, state.pot());
    let value = |tone: Tone| {
        div()
            .flex()
            .items_baseline()
            .text_color(tone)
            .child(typo::disp(34., 800, 30).on(div()).child(pot.clone()))
            .child(
                typo::text(15., 700)
                    .lh(1.)
                    .on(div())
                    .ml(u(4.))
                    .opacity(0.85)
                    .child("bb"),
            )
    };
    out = out.child(at(
        POT,
        center(motion::fade_in(
            div()
                .flex()
                .items_center()
                .gap(u(12.))
                .children(pot_stacks(ui, bx, state.pot()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            typo::caps(11.5, 800, 160)
                                .on(div())
                                .opacity(0.8)
                                .child(i18n.tr("baralho-pot").to_uppercase()),
                        )
                        .child(
                            div()
                                .relative()
                                // The hard shadow under the figure.
                                .child(value(alpha(bx.well, 0.6)).absolute().left_0().top(u(3.)))
                                .child(value(bx.on_felt).relative()),
                        ),
                ),
            anim_id(format!("pot_{deal}_{}", state.pot())),
        )),
    ));

    // The board: dealt cards, then a dashed slot for each card to come.
    let board = state.board();
    out = out.child(at(
        BOARD,
        center(
            div()
                .flex()
                .gap(u(9.))
                .children(board.iter().enumerate().map(|(i, &c)| {
                    motion::deal_in(
                        kit::playing_card(bx, p, Some(c), CardSize::LG),
                        anim_id(format!("board_{deal}_{i}_{c}")),
                        drop_in,
                    )
                }))
                .children((board.len()..5).map(|_| {
                    div()
                        .flex_none()
                        .w(u(74.))
                        .h(u(102.))
                        .rounded(u(11.))
                        .border(u(2.5))
                        .border_dashed()
                        .border_color(alpha(bx.cream, 0.28))
                })),
        ),
    ));

    // Chips in front of each seat on this street (not once the winners'
    // takes are shown: those sit in the same places).
    for seat in 0..n {
        let commit = state.players()[seat].street_commit;
        if commit == 0 || !wins.is_empty() {
            continue;
        }
        let from = seat_at(slot(seat), n);
        let to = bet_at(slot(seat), from);
        let street = state.street();
        out = out.child(motion::slide(
            center(
                div()
                    .flex()
                    .items_end()
                    .gap(u(7.))
                    .child(bet_stack(ui, bx, commit))
                    .child(amount_pill(bx, bb(ui, commit))),
            ),
            anim_id(format!("bet_{deal}_{seat}_{street:?}_{commit}")),
            corner(from),
            corner(to),
        ));
    }

    // The dealer button.
    let button = state.button();
    out = out.child(at(
        dealer_at(slot(button), seat_at(slot(button), n)),
        center(
            Raised {
                face: bx.cream,
                edge: bx.deep,
                radius: 15.,
                border: 2.5,
                drop: 3.,
            }
            .sized(
                30.,
                30.,
                typo::disp(14., 800, 20)
                    .on(div())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(bx.ink)
                    .child("D"),
            ),
        ),
    ));

    // Hole cards: hero's large and face up, the others' small.
    for seat in 0..n {
        let pl = &state.players()[seat];
        if pl.folded {
            continue;
        }
        let k = slot(seat);
        let mine = seat == hero;
        let shown = mine || reveal;
        let size = if mine { CardSize::XL } else { CardSize::SM };
        let spot = seat_at(k, n);
        let hole = pl.hole.cards();
        let cards = hole.iter().enumerate().map(|(i, &c)| {
            let card = kit::playing_card(bx, p, shown.then_some(c), size);
            let gap = match (i, mine, shown) {
                (0, _, _) => 0.,
                (_, true, _) => 8.,
                (_, false, true) => 3.,
                // Face-down cards overlap.
                (_, false, false) => -12.,
            };
            // Keyed by the real card: a new hand deals again, turning a
            // hand face up at showdown does not.
            motion::deal_in(
                card.ml(u(gap)),
                anim_id(format!("deal_{deal}_{seat}_{i}_{c}")),
                drop_in,
            )
        });
        let place = if mine {
            (spot.0, spot.1 - 92.)
        } else {
            hole_at(spot)
        };
        out = out.child(at(place, center(div().flex().children(cards))));
    }

    // The plates.
    for seat in 0..n {
        let pl = &state.players()[seat];
        let mine = seat == hero;
        let kind = if pl.folded {
            Plate::Folded
        } else if mine {
            Plate::Hero
        } else {
            Plate::Live
        };
        let line = lines.get(seat).and_then(Option::as_ref);
        let what = match (&said, mine) {
            (Some(said), true) => Some(said.clone()),
            _ => line.map(|l| l.what.clone()),
        };
        let text = match (what, mine) {
            (Some(what), true) => {
                Some(i18n.tr_args("baralho-seat-you", &[("action", what.into())]))
            }
            (Some(what), false) => Some(what),
            (None, true) => Some(i18n.tr("baralho-you")),
            (None, false) => None,
        };
        let tone = if mine {
            p.suit_clubs
        } else if line.is_some_and(|l| l.aggressive) {
            p.suit_hearts
        } else {
            bx.mut_ink
        };
        out = out.child(at(
            seat_at(slot(seat), n),
            center(plate(
                bx,
                state.position(seat).label(),
                bb(ui, pl.stack),
                text.map(|t| (t, tone)),
                kind,
                turn && state.to_act() == Some(seat),
            )),
        ));
    }

    // Seat tags (the bot profile a seat plays): under the plate, or over
    // it for a seat on the rail's top edge.
    for seat in 0..n {
        let Some(tag) = tags.get(seat).and_then(Option::as_ref) else {
            continue;
        };
        let (x, y) = seat_at(slot(seat), n);
        let dy = if y < TOP_ROW { -43. } else { 49. };
        out = out.child(at(
            (x, y + dy),
            center(
                typo::text(11.5, 700)
                    .on(div())
                    .px(u(9.))
                    .rounded_full()
                    .bg(bx.deep)
                    .text_color(bx.amber)
                    .whitespace_nowrap()
                    .child(tag.clone()),
            ),
        ));
    }

    // What each winner took, slid from the pot to the seat.
    for &(seat, chips) in wins {
        let to = bet_at(slot(seat), seat_at(slot(seat), n));
        out = out.child(motion::slide(
            center(
                typo::disp(18., 800, 20)
                    .lh(1.2)
                    .on(div())
                    .px(u(11.))
                    .pt(u(1.))
                    .pb(u(2.))
                    .rounded_full()
                    .bg(bx.amber)
                    .border(u(2.))
                    .border_color(bx.deep)
                    .text_color(bx.ink)
                    .child(format!("+{}", bb(ui, chips))),
            ),
            anim_id(format!("win_{deal}_{seat}_{chips}")),
            corner(POT),
            corner(to),
        ));
    }
    out
}

/// A street's name.
pub fn street_name(ui: &Ui, street: Street) -> String {
    ui.i18n.tr(&format!("street-{}", street_id(street)))
}

/// [`table`] fitted to a `w` × `h` box (design pixels), centered in it.
pub fn fitted(w: f32, h: f32, draw: impl FnOnce() -> Div) -> Div {
    let k = (w / W).min(h / H);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(u(w))
        .h(u(h))
        .child(zoomed(k, draw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_seats_sit_where_the_mockup_has_them() {
        for (k, at) in SIX.iter().enumerate() {
            assert_eq!(seat_at(k, 6), *at);
        }
        // Hero's chips and the button, as drawn.
        assert_eq!(bet_at(0, SIX[0]), (392., 378.));
        assert_eq!(dealer_at(0, SIX[0]), (624., 400.));
        let (x, y) = bet_at(1, SIX[1]);
        assert!((x - 252.).abs() < 1.5 && (y - 344.).abs() < 1.5);
        assert_eq!(hole_at(SIX[1]), (96., 340.));
    }

    #[test]
    fn other_table_sizes_keep_hero_at_the_bottom() {
        for n in [2, 3, 9] {
            assert_eq!(seat_at(0, n), SIX[0]);
            for k in 0..n {
                let (x, y) = seat_at(k, n);
                assert!((96. ..=914.).contains(&x) && (66. ..=548.).contains(&y));
            }
        }
    }
}

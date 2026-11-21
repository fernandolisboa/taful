//! Pictures in lessons (D26): rows of cards and a table seen from above,
//! drawn in the active style from a lesson's `cards` and `seats` blocks
//! (format: `taful_content::picture`).

use gpui::{AnyElement, Div, FontWeight, div, prelude::*, px};
use taful_app::model::{CardRow, CardSlot, Picture, SeatMap};
use taful_app::preflop::Position;

use crate::Ui;
use crate::felt;
use crate::theme::Palette;

/// Cards are drawn this many times the table's card size.
const CARD_SCALE: f32 = 1.5;
/// The table picture's box.
const SEATS_W: f32 = 380.;
const SEATS_H: f32 = 210.;
const SEAT_W: f32 = 58.;
const SEAT_H: f32 = 28.;

/// A lesson picture.
pub fn picture(pic: &Picture, ui: &Ui, p: &Palette) -> AnyElement {
    match pic {
        Picture::Cards(rows) => cards(rows, ui, p).into_any_element(),
        Picture::Seats(map) => seats(map, ui, p).into_any_element(),
    }
}

fn cards(rows: &[CardRow], ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    let captions = rows.iter().any(|r| r.caption.is_some());
    div()
        .flex()
        .flex_col()
        .gap(px(t.spacing.sm))
        .py(px(t.spacing.xs))
        .children(rows.iter().map(|r| {
            let slots = r.cards.iter().map(|c| match *c {
                CardSlot::Face { card, dim } => {
                    let face = felt::card(ui, p, Some(card), CARD_SCALE);
                    if dim { face.opacity(0.35) } else { face }
                }
                CardSlot::Back => felt::card(ui, p, None, CARD_SCALE),
                CardSlot::Gap => div().w(px(t.spacing.md)),
            });
            div()
                .flex()
                .items_center()
                .gap(px(t.spacing.md))
                .when(captions, |d| {
                    d.child(
                        div()
                            .w(px(128.))
                            .flex_none()
                            .text_size(px(t.text_size.sm))
                            .text_color(p.text_muted)
                            .child(r.caption.clone().unwrap_or_default()),
                    )
                })
                .child(div().flex().items_center().gap(px(4.)).children(slots))
        }))
}

fn seats(map: &SeatMap, ui: &Ui, p: &Palette) -> Div {
    let t = &ui.theme;
    let n = usize::from(map.players);
    // The reader sits at the bottom (else the button), the others clockwise
    // from their left, as on the table screen.
    let bottom = map
        .you
        .or(Some(Position::Btn))
        .and_then(|pos| map.seat_of(pos))
        .unwrap_or(0);
    let angle = |seat: usize| {
        let k = (seat + n - bottom) % n;
        std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / n as f32
    };
    let (cx, cy) = (SEATS_W / 2., SEATS_H / 2.);
    let (felt_w, felt_h) = (SEATS_W - 2. * SEAT_W, SEATS_H - 2. * SEAT_H);
    // Seats sit on the rail; chips and the button between the seat and the
    // middle.
    let (rx, ry) = (felt_w / 2. + 8., felt_h / 2. + 4.);
    let at = |seat: usize, k: f32| {
        let a = angle(seat);
        (cx + rx * k * a.cos(), cy + ry * k * a.sin())
    };
    // Heads-up the button posts the small blind.
    let sb = map.seat_of(Position::Sb).or_else(|| (n == 2).then_some(0));
    let bb = map.seat_of(Position::Bb);
    let chip = |text: String, seat: usize| {
        let (x, y) = at(seat, 0.42);
        div()
            .absolute()
            .left(px(x - 23.))
            .top(px(y - 9.))
            .w(px(46.))
            .h(px(18.))
            .rounded(px(9.))
            .bg(p.surface)
            .border_1()
            .border_color(p.accent_muted)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(t.text_size.sm * 0.85))
            .text_color(p.text)
            .child(text)
    };
    let mut out = div()
        .relative()
        .w(px(SEATS_W))
        .h(px(SEATS_H))
        .flex_none()
        .my(px(t.spacing.xs))
        .child(
            div()
                .absolute()
                .left(px(SEAT_W))
                .top(px(SEAT_H))
                .w(px(felt_w))
                .h(px(felt_h))
                .rounded(px(felt_h / 2.))
                .bg(p.felt)
                .border(px(5.))
                .border_color(p.rail),
        );
    for seat in 0..n {
        let pos = map.position(seat);
        let (x, y) = at(seat, 1.);
        let you = map.you == Some(pos);
        let marked = map.mark.contains(&pos);
        let folded = map.out.contains(&pos);
        let mut s = div()
            .absolute()
            .left(px(x - SEAT_W / 2.))
            .top(px(y - SEAT_H / 2.))
            .w(px(SEAT_W))
            .h(px(SEAT_H))
            .rounded(px(SEAT_H / 2.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(t.text_size.sm))
            .font_weight(FontWeight::BOLD)
            .border_2()
            .border_color(if you { p.accent } else { p.rail_edge })
            .bg(if marked { p.accent } else { p.seat })
            .text_color(if marked { p.on_accent } else { p.text })
            .child(pos.label());
        if folded {
            s = s.opacity(0.4);
        }
        out = out.child(s);
        if let Some(k) = map.order_of(pos) {
            out = out.child(
                div()
                    .absolute()
                    .left(px(x + SEAT_W / 2. - 10.))
                    .top(px(y - SEAT_H / 2. - 8.))
                    .size(px(18.))
                    .rounded(px(9.))
                    .bg(p.accent)
                    .border_1()
                    .border_color(p.surface)
                    .text_color(p.on_accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(t.text_size.sm * 0.8))
                    .font_weight(FontWeight::BOLD)
                    .child(k.to_string()),
            );
        }
        if you {
            let below = y < cy;
            out = out.child(
                div()
                    .absolute()
                    .left(px(x - SEAT_W / 2.))
                    .w(px(SEAT_W))
                    .top(px(if below {
                        y + SEAT_H / 2. + 1.
                    } else {
                        y - SEAT_H / 2. - 16.
                    }))
                    .flex()
                    .justify_center()
                    .text_size(px(t.text_size.sm * 0.85))
                    .text_color(p.accent)
                    .child(ui.i18n.tr("lesson-picture-you")),
            );
        }
    }
    // The dealer button, in front of the button's seat, a little towards
    // the small blind.
    let (bx, by) = {
        let a = angle(0) + 0.45;
        (cx + rx * 0.62 * a.cos(), cy + ry * 0.62 * a.sin())
    };
    out = out.child(
        div()
            .absolute()
            .left(px(bx - 10.))
            .top(px(by - 10.))
            .size(px(20.))
            .rounded(px(10.))
            .bg(p.card_face)
            .border_1()
            .border_color(p.rail_edge)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(t.text_size.sm * 0.8))
            .font_weight(FontWeight::BOLD)
            .text_color(p.suit_spades)
            .child("D"),
    );
    if map.blinds {
        if let Some(s) = sb {
            out = out.child(chip(format!("{} bb", felt::num(ui, 0.5, 1)), s));
        }
        if let Some(s) = bb {
            out = out.child(chip(format!("{} bb", felt::num(ui, 1., 0)), s));
        }
    }
    out
}

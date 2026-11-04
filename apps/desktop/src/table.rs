use std::f32::consts::PI;

use gpui::{
    AnyElement, Context, Div, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Rgba, SharedString, Window, div, prelude::*, px,
};

use crate::theme::{Theme, default_theme};

const TABLE_MAX_W: f32 = 860.;
const TABLE_MIN_W: f32 = 560.;
const TABLE_RATIO: f32 = 0.49;
const RAIL: f32 = 22.;
const SEAT_W: f32 = 168.;
const SEAT_H: f32 = 62.;
const CARD_W: f32 = 52.;
const CARD_H: f32 = 74.;
const GRID: f32 = 24.;
const PANEL_W: f32 = 288.;
const PANEL_H: f32 = 168.;

#[derive(Clone, Copy)]
enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    fn symbol(self) -> &'static str {
        match self {
            Suit::Spades => "♠",
            Suit::Hearts => "♥",
            Suit::Diamonds => "♦",
            Suit::Clubs => "♣",
        }
    }

    fn color(self, theme: &Theme) -> Rgba {
        match self {
            Suit::Spades => theme.suit_spades,
            Suit::Hearts => theme.suit_hearts,
            Suit::Diamonds => theme.suit_diamonds,
            Suit::Clubs => theme.suit_clubs,
        }
    }
}

#[derive(Clone, Copy)]
struct Card(&'static str, Suit);

struct Seat {
    position: &'static str,
    name: &'static str,
    stack_bb: f32,
    bet_bb: f32,
    in_hand: bool,
    hero: bool,
}

#[derive(Clone, Copy)]
struct PanelDrag {
    grab_x: f32,
    grab_y: f32,
}

pub struct TableView {
    theme: Theme,
    seats: Vec<Seat>,
    board: Vec<Card>,
    hero_cards: [Card; 2],
    pot_bb: f32,
    dealer_seat: usize,
    panel_x: f32,
    panel_y: f32,
    drag: Option<PanelDrag>,
    last_action: Option<SharedString>,
}

impl TableView {
    pub fn new() -> Self {
        // BTN vs BB single-raised pot, turn. Seats run clockwise from hero.
        let seat = |position, name, stack_bb, bet_bb, in_hand, hero| Seat {
            position,
            name,
            stack_bb,
            bet_bb,
            in_hand,
            hero,
        };
        Self {
            theme: default_theme(),
            seats: vec![
                seat("BB", "Você", 94.5, 0., true, true),
                seat("UTG", "Nit Nilo", 100., 0., false, false),
                seat("HJ", "Tita TAG", 112.3, 0., false, false),
                seat("CO", "Calu Station", 87., 0., false, false),
                seat("BTN", "Mano Maníaco", 91., 3.5, true, false),
                seat("SB", "Bia Blinds", 99.5, 0., false, false),
            ],
            board: vec![
                Card("K", Suit::Diamonds),
                Card("9", Suit::Clubs),
                Card("4", Suit::Hearts),
                Card("2", Suit::Spades),
            ],
            hero_cards: [Card("A", Suit::Spades), Card("K", Suit::Hearts)],
            pot_bb: 5.5,
            dealer_seat: 4,
            panel_x: GRID,
            panel_y: GRID,
            drag: None,
            last_action: None,
        }
    }

    fn on_panel_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag = Some(PanelDrag {
            grab_x: f32::from(event.position.x) - self.panel_x,
            grab_y: f32::from(event.position.y) - self.panel_y,
        });
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.drag else { return };
        let viewport = window.viewport_size();
        let max_x = (f32::from(viewport.width) - PANEL_W).max(0.);
        let max_y = (f32::from(viewport.height) - PANEL_H).max(0.);
        let snap = |v: f32, max: f32| ((v / GRID).round() * GRID).clamp(0., max);
        let x = snap(f32::from(event.position.x) - drag.grab_x, max_x);
        let y = snap(f32::from(event.position.y) - drag.grab_y, max_y);
        if x != self.panel_x || y != self.panel_y {
            self.panel_x = x;
            self.panel_y = y;
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    fn card(&self, card: Card, scale: f32) -> Div {
        let t = &self.theme;
        let color = card.1.color(t);
        div()
            .w(px(CARD_W * scale))
            .h(px(CARD_H * scale))
            .rounded(px(6. * scale))
            .bg(t.card_face)
            .border_1()
            .border_color(t.accent_dark)
            .shadow_md()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_color(color)
            .font_weight(FontWeight::BOLD)
            .child(
                div()
                    .text_size(px(24. * scale))
                    .line_height(px(26. * scale))
                    .child(card.0),
            )
            .child(
                div()
                    .text_size(px(22. * scale))
                    .line_height(px(24. * scale))
                    .child(card.1.symbol()),
            )
    }

    fn card_back(&self, scale: f32) -> Div {
        let t = &self.theme;
        div()
            .w(px(CARD_W * scale))
            .h(px(CARD_H * scale))
            .rounded(px(6. * scale))
            .bg(t.card_back)
            .border_2()
            .border_color(t.accent)
            .shadow_sm()
            .p(px(4. * scale))
            .child(
                div()
                    .size_full()
                    .rounded(px(3. * scale))
                    .border_1()
                    .border_color(t.accent_dark),
            )
    }

    fn chip_stack(&self, chips: usize) -> Div {
        let t = &self.theme;
        div()
            .relative()
            .w(px(22.))
            .h(px(22. + 4. * chips as f32))
            .children((0..chips).map(|i| {
                div()
                    .absolute()
                    .left_0()
                    .bottom(px(4. * i as f32))
                    .size(px(22.))
                    .rounded_full()
                    .bg(t.accent)
                    .border_2()
                    .border_dashed()
                    .border_color(t.text)
            }))
    }

    fn seat_view(&self, seat: &Seat, is_dealer: bool) -> Div {
        let t = &self.theme;
        let initials: String = seat
            .name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .collect();
        div()
            .w(px(SEAT_W))
            .h(px(SEAT_H))
            .rounded_lg()
            .bg(if seat.hero { t.seat_active } else { t.seat })
            .border_2()
            .border_color(if seat.hero { t.accent } else { t.rail_edge })
            .when(!seat.in_hand, |d| d.opacity(0.45))
            .shadow_lg()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .child(
                div()
                    .size(px(40.))
                    .flex_none()
                    .rounded_full()
                    .bg(t.felt_line)
                    .border_2()
                    .border_color(if is_dealer { t.accent } else { t.text_muted })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.text)
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child(initials),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.text)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(seat.name),
                    )
                    .child(div().text_size(px(12.)).text_color(t.accent).child(format!(
                        "{} · {} bb",
                        seat.position,
                        fmt_bb(seat.stack_bb)
                    )))
                    .when(!seat.in_hand, |d| {
                        d.child(
                            div()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child("fora da mão"),
                        )
                    }),
            )
    }

    fn dealer_button(&self) -> Div {
        let t = &self.theme;
        div()
            .size(px(26.))
            .rounded_full()
            .bg(t.text)
            .border_2()
            .border_color(t.accent)
            .shadow_md()
            .flex()
            .items_center()
            .justify_center()
            .text_color(t.background)
            .text_size(px(12.))
            .font_weight(FontWeight::BOLD)
            .child("D")
    }

    fn action_button(
        &self,
        id: &'static str,
        label: String,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let (bg, fg, hover) = if primary {
            (t.accent, t.background, t.text)
        } else {
            (t.panel_header, t.text, t.rail_edge)
        };
        let action: SharedString = label.clone().into();
        div()
            .id(id)
            .px_5()
            .py_3()
            .min_w(px(120.))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .text_size(px(15.))
            .font_weight(FontWeight::BOLD)
            .flex()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.last_action = Some(action.clone());
                cx.notify();
            }))
            .child(label)
    }

    fn grid_overlay(&self, width: f32, height: f32) -> impl IntoElement {
        let t = &self.theme;
        let cols = (width / GRID) as usize + 1;
        let rows = (height / GRID) as usize + 1;
        div()
            .absolute()
            .inset_0()
            .children((0..rows).flat_map(move |r| {
                (0..cols).map(move |c| {
                    div()
                        .absolute()
                        .left(px(c as f32 * GRID - 1.))
                        .top(px(r as f32 * GRID - 1.))
                        .size(px(2.))
                        .bg(t.grid_dot)
                })
            }))
    }

    fn hint_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let to_call = self.seats.iter().map(|s| s.bet_bb).fold(0., f32::max);
        let final_pot = self.pot_bb + to_call * 2.;
        let odds = to_call / final_pot * 100.;
        let dragging = self.drag.is_some();
        div()
            .absolute()
            .left(px(self.panel_x))
            .top(px(self.panel_y))
            .w(px(PANEL_W))
            .h(px(PANEL_H))
            .rounded_lg()
            .bg(t.panel)
            .border_2()
            .border_color(if dragging { t.accent } else { t.rail_edge })
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(t.panel_header)
                    .text_color(t.text)
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_grab()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_panel_mouse_down))
                    .child("⠿  Calculadora — arraste aqui"),
            )
            .child(
                div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_size(px(13.))
                    .text_color(t.text_muted)
                    .child(format!("Pote: {} bb", fmt_bb(self.pot_bb)))
                    .child(format!("Para pagar: {} bb", fmt_bb(to_call)))
                    .child(
                        div()
                            .text_color(t.accent)
                            .font_weight(FontWeight::BOLD)
                            .child(format!(
                                "Pot odds: {} / {} = {:.0}%",
                                fmt_bb(to_call),
                                fmt_bb(final_pot),
                                odds
                            )),
                    )
                    .child(format!(
                        "Posição {}, {} · grade de {} px",
                        self.panel_x, self.panel_y, GRID
                    )),
            )
    }

    fn table(&self, width: f32, cx_x: f32, cx_y: f32) -> AnyElement {
        let t = &self.theme;
        let table_w = (width - SEAT_W * 2. - 80.).clamp(TABLE_MIN_W, TABLE_MAX_W);
        let table_h = table_w * TABLE_RATIO;
        let left = cx_x - table_w / 2.;
        let top = cx_y - table_h / 2.;
        let mut layer = div().absolute().inset_0();

        layer = layer.child(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(table_w))
                .h(px(table_h))
                .rounded_full()
                .bg(t.rail)
                .border_2()
                .border_color(t.rail_edge)
                .shadow_lg()
                .p(px(RAIL))
                .child(
                    div()
                        .size_full()
                        .rounded_full()
                        .bg(t.felt)
                        .border_2()
                        .border_color(t.felt_line),
                ),
        );

        let board_w = self.board.len() as f32 * (CARD_W + 8.);
        layer = layer.child(
            div()
                .absolute()
                .left(px(cx_x - board_w / 2.))
                .top(px(cx_y - CARD_H / 2. + 10.))
                .flex()
                .gap_2()
                .children(self.board.iter().map(|c| self.card(*c, 1.))),
        );
        layer = layer.child(
            div()
                .absolute()
                .left(px(cx_x - 80.))
                .top(px(cx_y - CARD_H / 2. - 44.))
                .w(px(160.))
                .flex()
                .justify_center()
                .items_center()
                .gap_2()
                .child(self.chip_stack(4))
                .child(
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .bg(t.background)
                        .text_color(t.text)
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .child(format!("Pote {} bb", fmt_bb(self.pot_bb))),
                ),
        );

        let rx = table_w / 2. + 30.;
        let ry = table_h / 2. + 34.;
        for (i, seat) in self.seats.iter().enumerate() {
            let angle = PI / 2. + i as f32 * PI / 3.;
            let (dx, dy) = (angle.cos(), angle.sin());
            let sx = cx_x + dx * rx;
            let sy = cx_y + dy * ry;
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(sx - SEAT_W / 2.))
                    .top(px(sy - SEAT_H / 2.))
                    .child(self.seat_view(seat, i == self.dealer_seat)),
            );
            if i == self.dealer_seat {
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(sx + SEAT_W / 2. - 16.))
                        .top(px(sy - SEAT_H / 2. - 12.))
                        .child(self.dealer_button()),
                );
            }

            if seat.in_hand {
                let (cards, scale): (Vec<AnyElement>, f32) = if seat.hero {
                    (
                        self.hero_cards
                            .iter()
                            .map(|c| self.card(*c, 1.1).into_any_element())
                            .collect(),
                        1.1,
                    )
                } else {
                    (
                        (0..2)
                            .map(|_| self.card_back(0.7).into_any_element())
                            .collect(),
                        0.7,
                    )
                };
                let w = 2. * CARD_W * scale + 6.;
                let h = CARD_H * scale;
                // Hero's cards sit above the seat; villains' sit between seat and center.
                let (card_cx, card_cy) = if seat.hero {
                    (sx, sy - SEAT_H / 2. - h / 2. - 6.)
                } else {
                    (sx - dx * rx * 0.26, sy - dy * ry * 0.37)
                };
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(card_cx - w / 2.))
                        .top(px(card_cy - h / 2.))
                        .flex()
                        .gap(px(6.))
                        .children(cards),
                );
            }

            if seat.bet_bb > 0. {
                let bx = cx_x + dx * rx * 0.62;
                let by = cx_y + dy * ry * 0.58;
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(bx - 40.))
                        .top(px(by - 14.))
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(self.chip_stack(2))
                        .child(
                            div()
                                .text_color(t.text)
                                .text_size(px(13.))
                                .font_weight(FontWeight::BOLD)
                                .child(format!("{} bb", fmt_bb(seat.bet_bb))),
                        ),
                );
            }
        }

        layer.into_any_element()
    }
}

impl Render for TableView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));
        let to_call = self.seats.iter().map(|s| s.bet_bb).fold(0., f32::max);
        let t = &self.theme;

        div()
            .size_full()
            .relative()
            .bg(t.background)
            .text_color(t.text)
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .when(self.drag.is_some(), |d| d.child(self.grid_overlay(vw, vh)))
            .child(self.table(vw, vw / 2., vh / 2. - 40.))
            .child(
                div()
                    .absolute()
                    .right(px(24.))
                    .bottom(px(24.))
                    .flex()
                    .gap_3()
                    .child(self.action_button("fold", "Desistir".into(), false, cx))
                    .child(self.action_button(
                        "call",
                        format!("Pagar {}", fmt_bb(to_call)),
                        false,
                        cx,
                    ))
                    .child(self.action_button(
                        "raise",
                        format!("Aumentar {}", fmt_bb(to_call * 3.)),
                        true,
                        cx,
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left(px(24.))
                    .bottom(px(24.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child(format!(
                        "Spike §9.1 · GPUI 0.2.2 · janela {}×{}",
                        vw.round(),
                        vh.round()
                    ))
                    .child(match &self.last_action {
                        Some(a) => format!("Última ação: {a}"),
                        None => "Clique numa ação ou arraste o painel".into(),
                    }),
            )
            .child(self.hint_panel(cx))
    }
}

fn fmt_bb(v: f32) -> String {
    let s = if v.fract() == 0. {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    };
    s.replace('.', ",")
}

//! Cockpit's building blocks: type, panels, buttons, key caps, icons and
//! small charts.

use gpui::{
    AnyElement, Bounds, BoxShadow, Div, ElementId, FontWeight, PathBuilder, Pixels, SharedString,
    Stateful, Svg, canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px,
    size, svg,
};
use taful_app::preflop::{Card, Suit};

use super::pal::{Pal, TablePal, Tint};
use crate::theme::alpha;

/// The interface font.
pub const SANS: &str = "Inter";
/// The font of numbers, labels and codes.
pub const MONO: &str = "JetBrains Mono";
/// Advance of every glyph of [`MONO`], in ems.
const MONO_ADVANCE: f32 = 0.6;
/// The base text size.
pub const TEXT: f32 = 13.;
/// The gap between panels and around them.
pub const GAP: f32 = 12.;
pub const EDGE: f32 = 14.;
/// A panel header's height.
pub const HEAD: f32 = 34.;

/// Weight of the design's strongest sans text.
pub const STRONG: FontWeight = FontWeight::BOLD;

/// An icon of the Cockpit set, by file name.
pub fn icon(name: &'static str, size_px: f32, color: Tint) -> Svg {
    let path: SharedString = match name {
        "home" => "cockpit/icons/home.svg",
        "book" => "cockpit/icons/book.svg",
        "table" => "cockpit/icons/table.svg",
        "calc" => "cockpit/icons/calc.svg",
        "review" => "cockpit/icons/review.svg",
        "hands" => "cockpit/icons/hands.svg",
        "settings" => "cockpit/icons/settings.svg",
        "logo_wings" => "cockpit/icons/logo-wings.svg",
        "logo_knot" => "cockpit/icons/logo-knot.svg",
        "lock" => "cockpit/icons/lock.svg",
        "spade" => "cockpit/icons/spade.svg",
        "heart" => "cockpit/icons/heart.svg",
        "diamond" => "cockpit/icons/diamond.svg",
        "club" => "cockpit/icons/club.svg",
        "caret" => "cockpit/icons/caret.svg",
        _ => "cockpit/icons/check.svg",
    }
    .into();
    svg()
        .path(path)
        .size(px(size_px))
        .flex_none()
        .text_color(color)
}

/// Text in the mono font.
pub fn mono(text: impl Into<SharedString>) -> Div {
    div().font_family(MONO).child(text.into())
}

/// Mono text with letter spacing (`em` of the font size, like CSS; GPUI
/// has none): one cell per character. Every glyph of the font has the same
/// advance, so the cells line up exactly as a spaced run would.
pub fn tracked(text: &str, size_px: f32, em: f32) -> Div {
    let cell = size_px * (MONO_ADVANCE + em);
    let mut row = div()
        .flex()
        .flex_none()
        .font_family(MONO)
        .text_size(px(size_px))
        .whitespace_nowrap();
    for ch in text.chars() {
        let c = div().w(px(cell)).flex_none();
        row = row.child(if ch == ' ' {
            c
        } else {
            c.child(SharedString::from(ch.to_string()))
        });
    }
    row
}

/// [`tracked`] text that wraps between words.
pub fn tracked_wrap(text: &str, size_px: f32, em: f32) -> Div {
    let space = size_px * (MONO_ADVANCE + em);
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(space))
        .children(text.split(' ').map(|word| tracked(word, size_px, em)))
}

/// A drop shadow straight down.
pub fn shadow(color: Tint, y: f32, blur: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    }]
}

/// The size of a card [`tile`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    /// The board and hero's hand on the felt.
    Large,
    /// Opponents' hands and the hand beside a grade.
    Small,
    /// Inline, in a line of text.
    Tiny,
}

/// The icon of a suit (the fonts have no suit symbols).
pub fn suit_icon(suit: Suit) -> &'static str {
    match suit {
        Suit::SPADES => "spade",
        Suit::HEARTS => "heart",
        Suit::DIAMONDS => "diamond",
        _ => "club",
    }
}

/// A card as a colored tile, one color per suit (`None`: face down),
/// drawn `s` times its size.
pub fn tile(card: Option<Card>, size: Tile, s: f32, tp: &TablePal) -> Div {
    let (w, h, radius, rank, pip) = match size {
        Tile::Large => (56., 76., 6., 31., 21.),
        Tile::Small => (30., 40., 4., 16., 11.),
        Tile::Tiny => (20., 24., 3., 12., 8.),
    };
    let base = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(w * s))
        .h(px(h * s))
        .rounded(px(radius * s))
        .overflow_hidden();
    let Some(card) = card else {
        let [a, b] = tp.back;
        return base
            .bg(a)
            .border_1()
            .border_color(tp.back_edge)
            .child(stripes(b, 4. * s));
    };
    let suit = card.suit();
    let ink = if suit == Suit::DIAMONDS {
        tp.tile_diamonds_text
    } else {
        tp.tile_text
    };
    let name = suit_icon(suit);
    base.bg(tp.tiles[usize::from(suit.index())])
        .border_1()
        .border_color(tp.tile_edge)
        .when(size != Tile::Tiny, |d| {
            d.flex_col()
                .gap(px(s))
                .shadow(shadow(tp.shadow, 4. * s, 10. * s))
        })
        .child(
            div()
                .font_family(MONO)
                .font_weight(FontWeight::BOLD)
                .text_size(px(rank * s))
                .line_height(px(rank * s))
                .text_color(ink)
                .child(SharedString::from(card.rank().to_char().to_string())),
        )
        .child(icon(name, pip * s, ink))
}

/// Diagonal stripes of `color`, `band` wide with the same gap, over the
/// whole box.
fn stripes(color: Tint, band: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            // A 45° band's horizontal footprint.
            let step = band * std::f32::consts::SQRT_2;
            let mut x = -h;
            while x < w {
                let mut b = PathBuilder::fill();
                let at =
                    |dx: f32, dy: f32| point(bounds.origin.x + px(dx), bounds.origin.y + px(dy));
                b.move_to(at(x, h));
                b.line_to(at(x + h, 0.));
                b.line_to(at(x + h + step, 0.));
                b.line_to(at(x + step, h));
                b.close();
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
                x += 2. * step;
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A small upper-case caption.
pub fn label(text: &str, pal: &Pal) -> Div {
    tracked(&text.to_uppercase(), 10.5, 0.08).text_color(pal.muted)
}

/// A key cap.
pub fn kbd(key: &str, pal: &Pal) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_b_2()
        .border_color(pal.line2)
        .bg(pal.panel2)
        .font_family(MONO)
        .text_size(px(10.5))
        .line_height(px(16.))
        .text_color(pal.muted)
        .child(SharedString::from(key.to_owned()))
}

/// How a button is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Plain,
    /// The screen's main action.
    Amber,
    /// A small button inside a row.
    Small,
    SmallAmber,
}

/// A button, with the key that also triggers it when there is one.
pub fn btn(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    key: Option<&str>,
    kind: Btn,
    pal: &Pal,
) -> Stateful<Div> {
    let amber = matches!(kind, Btn::Amber | Btn::SmallAmber);
    let small = matches!(kind, Btn::Small | Btn::SmallAmber);
    let (bg, edge, fg) = if amber {
        (pal.amber, pal.amber, pal.on_amber)
    } else {
        (pal.raised, pal.line2, pal.text)
    };
    let hover = if amber {
        alpha(pal.amber, 0.9)
    } else {
        pal.line2
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(if small { 24. } else { 30. }))
        .px(px(if small { 9. } else { 12. }))
        .rounded(px(5.))
        .border_1()
        .border_color(edge)
        .bg(bg)
        .text_color(fg)
        .text_size(px(if small { 11.5 } else { 12.5 }))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(text.into())
        .when_some(key, |d, key| {
            d.child(if amber {
                kbd(key, pal)
                    .bg(pal.kbd_on_amber)
                    .border_color(pal.kbd_on_amber_line)
                    .text_color(pal.on_amber)
            } else {
                kbd(key, pal)
            })
        })
}

/// A panel: a bordered box whose children stack.
pub fn panel(pal: &Pal) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .overflow_hidden()
        .rounded(px(6.))
        .border_1()
        .border_color(pal.line)
        .bg(pal.panel)
}

/// A panel's header: its title after a small square, and a note on the
/// right. A `lit` header has the amber square; an unlit one is for content
/// that is not reachable yet.
pub fn head(title: &str, note: Option<AnyElement>, lit: bool, pal: &Pal) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(HEAD))
        .px(px(12.))
        .border_b_1()
        .border_color(pal.line)
        .bg(linear_gradient(
            180.,
            linear_color_stop(pal.head_top, 0.),
            linear_color_stop(pal.panel, 1.),
        ))
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .mr(px(8.))
                .rounded(px(1.))
                .bg(if lit { pal.amber } else { pal.line2 }),
        )
        .child(
            tracked(&title.to_uppercase(), 10.5, 0.09)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if lit { pal.text } else { pal.muted })
                .flex_shrink()
                .overflow_hidden(),
        )
        .child(div().flex_1())
        .children(note.map(|n| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(12.))
                .pl(px(10.))
                .text_size(px(11.5))
                .text_color(pal.muted)
                .child(n)
        }))
}

/// A big mono number with a small unit after it.
pub fn figure(value: &str, unit: Option<String>, size_px: f32, pal: &Pal) -> Div {
    div()
        .flex()
        .items_end()
        .child(
            tracked(value, size_px, -0.03)
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(px(size_px)),
        )
        .children(unit.map(|u| {
            let small = (size_px * 0.44).round().max(12.);
            div()
                .ml(px(4.))
                .font_family(MONO)
                .font_weight(FontWeight::MEDIUM)
                .text_size(px(small))
                .line_height(px(small * 1.15))
                .text_color(pal.muted)
                .child(u)
        }))
}

/// Progress as one block per step: done, the current one, and the rest.
pub fn steps(
    lessons: &[taful_app::lessons::LessonCard],
    next: Option<&str>,
    pal: &Pal,
) -> Vec<Div> {
    lessons
        .iter()
        .map(|l| {
            div().bg(if l.state == taful_app::model::LessonState::Done {
                pal.teal
            } else if next == Some(l.id.as_str()) {
                pal.amber
            } else {
                pal.track
            })
        })
        .collect()
}

/// A line chart without axes over its own box: `series` oldest first
/// (`None`: no point that day), drawn in `color` with a dot on the last
/// point. `area` fills under the line; `guide` is a dashed level line.
pub fn sparkline(
    series: Vec<Option<f64>>,
    guide: Option<f64>,
    color: Tint,
    dot: Tint,
    area: Option<Tint>,
    guide_color: Tint,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let known: Vec<(usize, f64)> = series
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|v| (i, v)))
                .collect();
            if known.is_empty() {
                return;
            }
            let values = known.iter().map(|(_, v)| *v).chain(guide);
            let (lo, hi) = values.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            // Room for the stroke and the end dot.
            let (pad_x, pad_y) = (3., 5.);
            let last = (series.len().max(2) - 1) as f32;
            let at = |i: usize, v: f64| {
                let t = if hi > lo {
                    ((v - lo) / (hi - lo)) as f32
                } else {
                    0.5
                };
                point(
                    bounds.origin.x + px(pad_x + (w - 2. * pad_x) * i as f32 / last),
                    bounds.origin.y + px(pad_y + (h - 2. * pad_y) * (1. - t)),
                )
            };
            let points: Vec<_> = known.iter().map(|(i, v)| at(*i, *v)).collect();
            let bottom = bounds.origin.y + bounds.size.height;
            if let (Some(tint), [first, .., end]) = (area, points.as_slice()) {
                let mut b = PathBuilder::fill();
                b.move_to(point(first.x, bottom));
                for p in &points {
                    b.line_to(*p);
                }
                b.line_to(point(end.x, bottom));
                b.close();
                if let Ok(path) = b.build() {
                    window.paint_path(path, tint);
                }
            }
            if let Some(level) = guide {
                let y = at(0, level).y;
                let mut b = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(4.)]);
                b.move_to(point(bounds.origin.x, y));
                b.line_to(point(bounds.origin.x + bounds.size.width, y));
                if let Ok(path) = b.build() {
                    window.paint_path(path, guide_color);
                }
            }
            if points.len() > 1 {
                let mut b = PathBuilder::stroke(px(1.8));
                b.move_to(points[0]);
                for p in &points[1..] {
                    b.line_to(*p);
                }
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
            if let Some(end) = points.last() {
                let r = px(3.);
                window.paint_quad(
                    fill(
                        Bounds::new(point(end.x - r, end.y - r), size(r * 2., r * 2.)),
                        dot,
                    )
                    .corner_radii(r),
                );
            }
        },
    )
    .size_full()
}

/// The backdrop: a dot every 16 px, aligned to the window.
pub fn dots(pal: &Pal) -> impl IntoElement {
    const STEP: f32 = 16.;
    let color = pal.dot;
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let (x1, y1) = (
                x0 + f32::from(bounds.size.width),
                y0 + f32::from(bounds.size.height),
            );
            let first = |from: f32| ((from - STEP / 2.) / STEP).ceil() * STEP + STEP / 2.;
            let r = px(1.1);
            let mut y = first(y0);
            while y < y1 {
                let mut x = first(x0);
                while x < x1 {
                    window.paint_quad(
                        fill(
                            Bounds::new(point(px(x) - r, px(y) - r), size(r * 2., r * 2.)),
                            color,
                        )
                        .corner_radii(r),
                    );
                    x += STEP;
                }
                y += STEP;
            }
        },
    )
    .size_full()
}

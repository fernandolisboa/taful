//! Baralho's building blocks: raised shapes with a thick outline and a
//! hard drop, cream sheets, buttons, playing cards, chips and suit glyphs.

use gpui::{
    BoxShadow, Div, ElementId, SharedString, Stateful, Svg, div, point, prelude::*, px, svg,
};
use taful_app::preflop::{Card, Suit};

use super::paint::{self, Tex};
use super::{Bx, Tone, typo, u};
use crate::theme::{Palette, alpha};

/// The four suits and the star of the last track.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pip {
    Spade,
    Heart,
    Diamond,
    Club,
    Star,
    /// The sixth group's mark (the deck has five).
    Crown,
}

impl Pip {
    /// The marks in the order lesson groups take them.
    pub const TRACKS: [Pip; 6] = [
        Pip::Spade,
        Pip::Diamond,
        Pip::Club,
        Pip::Heart,
        Pip::Star,
        Pip::Crown,
    ];

    pub fn of(s: Suit) -> Pip {
        match s {
            Suit::CLUBS => Pip::Club,
            Suit::DIAMONDS => Pip::Diamond,
            Suit::HEARTS => Pip::Heart,
            _ => Pip::Spade,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Pip::Spade => "baralho/icons/spade.svg",
            Pip::Heart => "baralho/icons/heart.svg",
            Pip::Diamond => "baralho/icons/diamond.svg",
            Pip::Club => "baralho/icons/club.svg",
            Pip::Star => "baralho/icons/star.svg",
            Pip::Crown => "baralho/icons/crown.svg",
        }
    }

    pub fn tone(self, bx: &Bx, p: &Palette) -> Tone {
        match self {
            Pip::Spade => p.suit_spades,
            Pip::Heart => p.suit_hearts,
            Pip::Diamond => p.suit_diamonds,
            Pip::Club => p.suit_clubs,
            Pip::Star => bx.joker,
            Pip::Crown => bx.amber,
        }
    }
}

pub const LOCK: &str = "baralho/icons/lock.svg";
pub const CHECK: &str = "baralho/icons/check.svg";
pub const CARET: &str = "baralho/icons/caret.svg";
/// The logo: a bow tie (wings) with a diamond knot.
pub const LOGO_WINGS: &str = "baralho/icons/logo-wings.svg";
pub const LOGO_KNOT: &str = "baralho/icons/logo-knot.svg";

/// A glyph `size` design pixels square, in `tone`.
pub fn icon(path: &'static str, size: f32, tone: Tone) -> Svg {
    svg().path(path).size(u(size)).flex_none().text_color(tone)
}

/// A shape with a thick outline that sits `drop` above its hard shadow.
#[derive(Clone, Copy)]
pub struct Raised {
    pub face: Tone,
    pub edge: Tone,
    pub radius: f32,
    pub border: f32,
    pub drop: f32,
}

impl Raised {
    /// Wraps `face`, the box that holds the content. The result is `drop`
    /// taller than the outlined box and gives that room back as a
    /// negative margin, so it lays out like the box alone.
    pub fn wrap(self, face: Div) -> Div {
        self.hug(face.flex_1()).mb(u(-self.drop))
    }

    /// A raised box as tall as what it holds, for a parent that takes its
    /// height from its content (a page that scrolls). Unlike
    /// [`Raised::wrap`] it has no negative margin: the layout leaves a box
    /// with one out of that height, so the page came out too short to
    /// scroll. The drop's room stays below the box, and the face hugs its
    /// content rather than filling a fixed height.
    pub fn hug(self, face: Div) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .rounded(u(self.radius))
            .bg(self.edge)
            .pt(u(self.border))
            .px(u(self.border))
            .pb(u(self.border + self.drop))
            .child(face.rounded(u(self.radius - self.border)).bg(self.face))
    }

    /// [`Raised::wrap`] at a fixed outlined-box size.
    pub fn sized(self, w: f32, h: f32, face: Div) -> Div {
        self.wrap(face).w(u(w)).h(u(h + self.drop))
    }
}

/// A soft shadow under a raised panel (`y` offset, CSS blur, opacity).
pub fn soft_shadow(bx: &Bx, y: f32, blur: f32, opacity: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(bx.shade, opacity).into(),
        offset: point(px(0.), u(y)),
        // GPUI's blur radius is the Gaussian's sigma: half the CSS blur.
        blur_radius: u(blur / 2.),
        spread_radius: px(0.),
    }]
}

/// A cream card panel: thick outline, hard drop, soft shadow and the
/// thin line just inside the edge. `face` holds the content.
pub fn sheet(bx: &Bx, face: Div) -> Div {
    sheet_with(bx, face, Raised::wrap)
}

/// A [`sheet`] laid out with [`Raised::hug`]: [`SHEET_DROP`] of room stays
/// below it.
pub fn page_sheet(bx: &Bx, face: Div) -> Div {
    sheet_with(bx, face, Raised::hug)
}

/// How far a sheet's hard drop reaches below its outline, in design pixels.
pub const SHEET_DROP: f32 = 7.;

fn sheet_with(bx: &Bx, face: Div, raise: fn(Raised, Div) -> Div) -> Div {
    let sheet = Raised {
        face: bx.cream,
        edge: bx.deep,
        radius: 20.,
        border: 2.5,
        drop: SHEET_DROP,
    };
    raise(
        sheet,
        face.relative().child(
            div()
                .absolute()
                .inset(u(7.))
                .rounded(u(13.))
                .border(u(1.5))
                .border_color(alpha(bx.ink, 0.13)),
        ),
    )
    .shadow(soft_shadow(bx, 22., 30., 0.25))
}

/// How a button is filled.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Amber,
    Ink,
    Cream,
}

/// A button: display type on a raised pill. `small` is the compact size.
pub fn btn(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    fill: Fill,
    small: bool,
    bx: &Bx,
) -> Stateful<Div> {
    btn_padded(id, label, fill, small, if small { 14. } else { 20. }, bx)
}

/// [`btn`] with its own side padding (the narrow step buttons).
pub fn btn_padded(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    fill: Fill,
    small: bool,
    pad: f32,
    bx: &Bx,
) -> Stateful<Div> {
    let (face, ink) = match fill {
        Fill::Amber => (bx.amber, bx.ink),
        Fill::Ink => (bx.ink, bx.cream),
        Fill::Cream => (bx.cream, bx.ink),
    };
    let (h, radius, border, drop, ty) = if small {
        (36., 10., 2., 3., typo::disp(14., 700, 10))
    } else {
        (46., 13., 2.5, 4., typo::disp(16.5, 700, 10))
    };
    let face_box = ty
        .on(div())
        .flex()
        .items_center()
        .justify_center()
        .gap(u(8.))
        .px(u(pad - border))
        .whitespace_nowrap()
        .text_color(ink)
        .group_hover("btn", |s| s.opacity(0.9))
        .child(label.into());
    Raised {
        face,
        edge: bx.deep,
        radius,
        border,
        drop,
    }
    .wrap(face_box)
    .h(u(h + drop))
    .group("btn")
    .cursor_pointer()
    .id(id.into())
}

/// A playing card's measures in design pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardSize {
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub border: f32,
    pub drop: f32,
    pub rank: f32,
    pub pip: f32,
    pub gap: f32,
}

impl CardSize {
    pub const XS: CardSize = CardSize::new(24., 31., 5., 1.5, 2., 13., 9., 0.);
    pub const SM: CardSize = CardSize::new(40., 54., 7., 2., 3., 21., 15., 1.);
    pub const MD: CardSize = CardSize::new(58., 80., 9., 2.5, 4., 32., 22., 1.);
    pub const LG: CardSize = CardSize::new(74., 102., 11., 2.5, 4., 42., 28., 1.);
    pub const XL: CardSize = CardSize::new(88., 120., 13., 2.5, 6., 50., 34., 1.);

    #[allow(clippy::too_many_arguments)]
    const fn new(
        w: f32,
        h: f32,
        radius: f32,
        border: f32,
        drop: f32,
        rank: f32,
        pip: f32,
        gap: f32,
    ) -> Self {
        Self {
            w,
            h,
            radius,
            border,
            drop,
            rank,
            pip,
            gap,
        }
    }

    /// The medium card `k` times as large.
    pub fn scaled(k: f32) -> CardSize {
        let m = Self::MD;
        CardSize {
            w: m.w * k,
            h: m.h * k,
            radius: m.radius * k,
            border: (m.border * k).clamp(1.5, 2.5),
            drop: (m.drop * k).max(2.),
            rank: m.rank * k,
            pip: m.pip * k,
            gap: k.min(1.),
        }
    }
}

/// A playing card: rank over suit on cream, or its hatched back.
pub fn playing_card(bx: &Bx, p: &Palette, c: Option<Card>, size: CardSize) -> Div {
    let raised = Raised {
        face: bx.cream,
        edge: bx.deep,
        radius: size.radius,
        border: size.border,
        drop: size.drop,
    };
    let face = div().flex().flex_col().items_center().justify_center();
    let face = match c {
        Some(c) => {
            let tone = p.suit(c.suit());
            face.gap(u(size.gap))
                .child(
                    typo::disp(size.rank, 800, 40)
                        .on(div())
                        .text_color(tone)
                        .child(c.rank().to_char().to_string()),
                )
                .child(icon(Pip::of(c.suit()).path(), size.pip, tone))
        }
        None => face.relative().child(
            paint::fill(
                // Only the medium and large backs carry the cream ring.
                if size.drop == CardSize::MD.drop {
                    Tex::Back
                } else {
                    Tex::BackPlain
                },
                *bx,
                size.radius - size.border,
            )
            .absolute()
            .inset_0(),
        ),
    };
    raised.sized(size.w, size.h, face)
}

/// One chip: a disc with a cream ring.
fn chip(bx: &Bx, tone: Tone) -> Div {
    let disc = |d: f32, c: Tone| {
        div()
            .absolute()
            .left(u((25. - d) / 2.))
            .top(u((25. - d) / 2.))
            .size(u(d))
            .rounded_full()
            .bg(c)
    };
    Raised {
        face: tone,
        edge: bx.deep,
        radius: 15.,
        border: 2.5,
        drop: 3.,
    }
    .sized(
        30.,
        30.,
        div()
            .relative()
            .child(disc(17.7, bx.cream))
            .child(disc(13.4, tone)),
    )
}

/// A stack of chips, bottom first. Each is one of the theme's chip colors.
pub fn chips(bx: &Bx, stack: &[Tone]) -> Div {
    let n = stack.len().max(1) as f32;
    let mut out = div()
        .relative()
        .flex_none()
        .w(u(30.))
        .h(u(30. + (n - 1.) * 5.));
    for (i, tone) in stack.iter().enumerate() {
        out = out.child(
            chip(bx, *tone)
                .absolute()
                .left_0()
                .bottom(u(i as f32 * 5. - 3.)),
        );
    }
    out
}

/// A grade dot: 13 px, outlined.
pub fn dot(bx: &Bx, tone: Tone) -> Div {
    div()
        .flex_none()
        .size(u(13.))
        .rounded_full()
        .bg(bx.deep)
        .p(u(2.))
        .child(div().size_full().rounded_full().bg(tone))
}

/// A small outlined label.
pub fn pill(bx: &Bx, label: impl Into<SharedString>) -> Div {
    typo::text(13.5, 700)
        .on(div())
        .flex()
        .flex_none()
        .items_center()
        .gap(u(8.))
        .px(u(12.))
        .py(u(5.))
        .rounded_full()
        .border(u(2.))
        .border_color(bx.deep)
        .bg(bx.cream_hi)
        .text_color(bx.ink)
        .whitespace_nowrap()
        .child(label.into())
}

/// Text that wraps like a browser wraps it: at spaces and after hyphens,
/// never inside a word (GPUI's own wrapping keeps `a-b` whole and then
/// cuts it anywhere when it does not fit). One element per word.
pub fn words(text: &str, ty: typo::Ty) -> Div {
    // Figtree's space, in ems.
    const SPACE: f32 = 0.24;
    let space = ty.size * SPACE;
    // Each word carries its trailing space; the box is that much wider (plus
    // a hair for rounding), so a line's last word may hang its space over
    // the edge, as browsers do.
    let mut out = ty.on(div()).flex().flex_wrap().mr(u(-space - 2.));
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let parts = hyphen_parts(word);
        let last = parts.len().saturating_sub(1);
        for (i, part) in parts.into_iter().enumerate() {
            out = out.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .when(i == last, |d| d.mr(u(space)))
                    .child(part.to_owned()),
            );
        }
    }
    out
}

/// Where a word may break: after a hyphen with at least three characters
/// on each side (`check-|raise`, but `6-max` and `c-bet` stay whole).
fn hyphen_parts(word: &str) -> Vec<&str> {
    const MIN: usize = 3;
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, _) in word.match_indices('-') {
        let end = i + 1;
        if word[start..i].chars().count() >= MIN && word[end..].chars().count() >= MIN {
            parts.push(&word[start..end]);
            start = end;
        }
    }
    parts.push(&word[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_break_after_long_hyphenated_parts_only() {
        // Capitals keep these from reading as Fluent ids to the source lint.
        assert_eq!(hyphen_parts("Check-Raise"), ["Check-", "Raise"]);
        assert_eq!(hyphen_parts("(bluff-catching)"), ["(bluff-", "catching)"]);
        assert_eq!(hyphen_parts("6-max"), ["6-max"]);
        assert_eq!(hyphen_parts("C-bet"), ["C-bet"]);
        assert_eq!(hyphen_parts("Stack-To-Pot"), ["Stack-", "To-Pot"]);
        assert_eq!(hyphen_parts("plain"), ["plain"]);
    }
}

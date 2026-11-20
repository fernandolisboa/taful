//! Salão's building blocks: type faces, shadows and gradients, panels,
//! buttons, playing cards and chips. Every number is the mockup's
//! (`design-directions/src/salao.css`).

use gpui::{
    Background, BoxShadow, Div, ElementId, FontWeight, Img, ObjectFit, Rgba, SharedString,
    Stateful, StyledImage, Svg, div, img, linear_color_stop, linear_gradient, point, prelude::*,
    px, relative, svg,
};
use taful_app::preflop::{Card, Suit};

use super::colors::{Colors, colors};
use crate::theme::{Palette, alpha};

// ---- images -------------------------------------------------------------------

pub const IMG_BG: &str = "salao/img/bg.webp";
pub const IMG_HERO: &str = "salao/img/hero.webp";
pub const IMG_TABLE: &str = "salao/img/table.webp";
pub const IMG_DEALER: &str = "salao/img/dealer.png";
pub const IMG_MEDAL_DONE: &str = "salao/img/medal_done.png";
pub const IMG_AVATAR_HERO: &str = "salao/img/avatar_hero.png";
pub const SVG_CHECK: &str = "salao/img/check.svg";
pub const SVG_LOCK: &str = "salao/img/lock.svg";
pub const SVG_LOGO_WINGS: &str = "salao/img/logo_wings.svg";
pub const SVG_LOGO_KNOT: &str = "salao/img/logo_knot.svg";

/// Every image the style draws (a test checks each one is embedded).
#[cfg(test)]
pub const IMAGES: &[&str] = &[
    IMG_BG,
    IMG_HERO,
    IMG_TABLE,
    IMG_DEALER,
    IMG_MEDAL_DONE,
    IMG_AVATAR_HERO,
    SVG_CHECK,
    SVG_LOCK,
    SVG_LOGO_WINGS,
    SVG_LOGO_KNOT,
    "salao/img/card_xs.png",
    "salao/img/card_sm.png",
    "salao/img/card_lg.png",
    "salao/img/back_sm.png",
    "salao/img/back_lg.png",
    "salao/img/chip_1.png",
    "salao/img/chip_5.png",
    "salao/img/chip_25.png",
    "salao/img/chip_100.png",
    "salao/img/chip_500.png",
    "salao/img/seal_best.png",
    "salao/img/seal_correct.png",
    "salao/img/seal_inaccuracy.png",
    "salao/img/seal_mistake.png",
    "salao/img/seal_blunder.png",
    "salao/img/suit_s.svg",
    "salao/img/suit_h.svg",
    "salao/img/suit_d.svg",
    "salao/img/suit_c.svg",
];

/// An embedded image stretched over a `w` × `h` box.
pub fn image(path: &'static str, w: f32, h: f32) -> Img {
    img(path)
        .w(px(w))
        .h(px(h))
        .flex_none()
        .object_fit(ObjectFit::Fill)
}

/// An embedded one-color icon, `size` pixels square.
pub fn icon(path: &'static str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

// ---- type ---------------------------------------------------------------------

/// A font family of the style. GPUI has no letter-spacing and no variable
/// font axes, so each use of the serif (its optical size, weight and
/// tracking) and each tracked label is a family of its own with one
/// regular face; `assets/salao/build/fonts.py` lists what each one is.
#[derive(Clone, Copy)]
pub struct Face {
    pub family: &'static str,
    /// Ascent and descent, as fractions of the font size.
    ascent: f32,
    descent: f32,
    /// Whether the mockup drew this family a pixel higher at the sizes
    /// where its descent rounds down (it did for the sans, not the serif).
    raised: bool,
}

const fn serif(family: &'static str) -> Face {
    Face {
        family,
        ascent: 0.978,
        descent: 0.255,
        raised: false,
    }
}

const fn sans(family: &'static str) -> Face {
    Face {
        family,
        ascent: 0.968_75,
        descent: 0.241_2,
        raised: true,
    }
}

/// The italic wordmark.
pub const BRAND: Face = serif("Taful Salao Brand");
/// Screen titles and the verdict's grade.
pub const H1: Face = serif("Taful Salao H1");
/// The home hero's lesson title.
pub const HERO: Face = serif("Taful Salao Hero");
/// The big numbers of the home cards, and their unit.
pub const STAT: Face = serif("Taful Salao Stat");
pub const STAT_UNIT: Face = serif("Taful Salao Stat Unit");
/// The lessons' overall count.
pub const OVERALL: Face = serif("Taful Salao Overall");
/// Track titles.
pub const TRACK: Face = serif("Taful Salao Track");
/// The pot on the felt.
pub const POT: Face = serif("Taful Salao Pot");
/// Card ranks.
pub const RANK: Face = serif("Taful Salao Rank");
/// Panel titles.
pub const TITLE: Face = serif("Taful Salao Title");
/// The leaks' italic ordinals.
pub const ORDINAL: Face = serif("Taful Salao Ordinal");
/// EV amounts in lists.
pub const AMOUNT: Face = serif("Taful Salao Amount");
/// Track medallions.
pub const MEDAL: Face = serif("Taful Salao Medal");
/// A track's lesson count.
pub const COUNT: Face = serif("Taful Salao Count");
/// A seat's stack.
pub const STACK: Face = serif("Taful Salao Stack");
/// A bet's amount on the felt.
pub const BET: Face = serif("Taful Salao Bet");
/// The verdict's EV loss.
pub const LOSS: Face = serif("Taful Salao Loss");
/// Serif at text sizes (street names, EVs in tables).
pub const TEXT: Face = serif("Taful Salao Text");
/// Inter, with real weights 400 to 800.
pub const SANS: Face = sans("Taful Salao Sans");
/// Tracked labels: regular at +0.02, +0.04, +0.14, +0.16 and +0.18 em,
/// semibold at +0.14 and +0.18 em, bold at +0.02 em.
pub const SANS_R02: Face = sans("Taful Salao Sans R02");
pub const SANS_R04: Face = sans("Taful Salao Sans R04");
pub const SANS_R14: Face = sans("Taful Salao Sans R14");
pub const SANS_R16: Face = sans("Taful Salao Sans R16");
pub const SANS_R18: Face = sans("Taful Salao Sans R18");
pub const SANS_S14: Face = sans("Taful Salao Sans S14");
pub const SANS_S18: Face = sans("Taful Salao Sans S18");
pub const SANS_B02: Face = sans("Taful Salao Sans B02");

/// Every face (a test checks each has its font file).
#[cfg(test)]
pub const FACES: &[Face] = &[
    BRAND, H1, HERO, STAT, STAT_UNIT, OVERALL, TRACK, POT, RANK, TITLE, ORDINAL, AMOUNT, MEDAL,
    COUNT, STACK, BET, LOSS, TEXT, SANS, SANS_R02, SANS_R04, SANS_R14, SANS_R16, SANS_R18,
    SANS_S14, SANS_S18, SANS_B02,
];

/// The mockup's body line height.
pub const LEADING: f32 = 1.45;

impl Face {
    /// One line of text in this face: `size` pixels in a line box `leading`
    /// times as tall, with the baseline on the pixel row the mockup has it.
    /// The box's vertical padding is part of that: space it with margins.
    pub fn text(self, size: f32, leading: f32) -> Div {
        let line = size * leading;
        let want = self.mock_baseline(size, line);
        // GPUI rounds line heights to whole pixels and centers the text in
        // them: take the tallest whole line that fits and pad it.
        let mut inner = line.floor();
        let mut top = want - self.drawn_baseline(size, inner);
        while inner > 1. && (top < 0. || inner + top > line + 0.01) {
            inner -= 1.;
            top = want - self.drawn_baseline(size, inner);
        }
        let top = top.max(0.);
        self.style(size)
            .line_height(px(inner))
            .pt(px(top))
            .pb(px((line - inner - top).max(0.)))
    }

    /// Text that may wrap, or a block of several lines: the first baseline
    /// is the mockup's, and the lines are a whole number of pixels apart.
    pub fn lines(self, size: f32, leading: f32) -> Div {
        let line = (size * leading).round();
        let shift = self.mock_baseline(size, size * leading) - self.drawn_baseline(size, line);
        self.style(size)
            .line_height(px(line))
            .relative()
            .top(px(shift))
    }

    fn style(self, size: f32) -> Div {
        div()
            .font_family(self.family)
            .font_weight(FontWeight::NORMAL)
            .text_size(px(size))
    }

    /// The whole-pixel ascent and descent the mockup gave this face.
    fn rounded(self, size: f32) -> (f32, f32) {
        let (ascent, descent) = (self.ascent * size, self.descent * size);
        if self.raised && descent.round() < descent {
            (ascent.round() - 1., descent.round() + 1.)
        } else {
            (ascent.round(), descent.round())
        }
    }

    /// From the top of a `line`-pixel line box down to the baseline, in the
    /// mockup.
    fn mock_baseline(self, size: f32, line: f32) -> f32 {
        let (ascent, descent) = self.rounded(size);
        ((line - ascent - descent) / 2.).floor() + ascent
    }

    /// The same distance as GPUI draws it (on the pixel row above).
    fn drawn_baseline(self, size: f32, line: f32) -> f32 {
        ((line - (self.ascent + self.descent) * size) / 2. + self.ascent * size + 0.001).floor()
    }

    /// The bottom margin that puts a `small` text's baseline on this one's
    /// when both sit at the bottom of a row (GPUI has no baseline
    /// alignment). Each is `(face, size, leading)`.
    pub fn lift(big: (Face, f32, f32), small: (Face, f32, f32)) -> f32 {
        let below = |(face, size, leading): (Face, f32, f32)| {
            size * leading - face.mock_baseline(size, size * leading)
        };
        below(big) - below(small)
    }
}

/// One line of body text in the base family: `size` pixels at `weight`.
pub fn sans_text(size: f32, weight: FontWeight) -> Div {
    SANS.text(size, LEADING).font_weight(weight)
}

/// Body text that may wrap (see [`Face::lines`]).
pub fn sans_lines(size: f32, weight: FontWeight) -> Div {
    SANS.lines(size, LEADING).font_weight(weight)
}

/// An uppercase tracked label.
pub fn label(face: Face, size: f32, color: Rgba, text: impl Into<SharedString>) -> Div {
    face.text(size, LEADING)
        .text_color(color)
        .child(text.into().to_uppercase())
}

/// The small gold label above a panel's content.
pub fn eyebrow(text: impl Into<SharedString>, color: Rgba) -> Div {
    label(SANS_S18, 11., color, text)
}

// ---- light --------------------------------------------------------------------

/// A CSS box shadow: `blur` is the CSS blur radius (GPUI takes the
/// Gaussian's deviation, half of it).
pub fn shadow(color: Rgba, x: f32, y: f32, blur: f32, spread: f32) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(x), px(y)),
        blur_radius: px(blur / 2.),
        spread_radius: px(spread),
    }
}

/// Top to bottom.
pub fn vgrad(top: Rgba, bottom: Rgba) -> Background {
    linear_gradient(
        180.,
        linear_color_stop(top, 0.),
        linear_color_stop(bottom, 1.),
    )
}

/// Left to right, `to` reached at `at` (0–1) of the width.
pub fn hgrad(from: Rgba, to: Rgba, at: f32) -> Background {
    linear_gradient(90., linear_color_stop(from, 0.), linear_color_stop(to, at))
}

/// A one-pixel ring just inside an element's edge (CSS `inset 0 0 0 1px`),
/// as an overlay so it takes no room.
pub fn ring(radius: f32, color: Rgba) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .border_1()
        .border_color(color)
}

/// A one-pixel highlight along an element's top edge (CSS
/// `inset 0 1px 0`).
pub fn top_light(radius: f32, color: Rgba) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .border_t_1()
        .border_color(color)
}

/// A three-stop vertical gradient behind an element's content: `top` to
/// `mid` at `at` (0–1) of the height, then to `bottom`. Two overlays (GPUI
/// gradients have two stops); the element is `relative` and its content
/// comes after them.
pub fn vgrad3(radius: f32, top: Rgba, mid: Rgba, bottom: Rgba, at: f32) -> [Div; 2] {
    [
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(relative(at))
            .rounded_t(px(radius))
            .bg(vgrad(top, mid)),
        div()
            .absolute()
            .top(relative(at))
            .bottom_0()
            .left_0()
            .w_full()
            .rounded_b(px(radius))
            .bg(vgrad(mid, bottom)),
    ]
}

/// The gold fill of buttons, the selected segment and hero's avatar.
pub fn gold_fill(radius: f32, at: f32, p: &Palette) -> [Div; 2] {
    let c = colors();
    vgrad3(radius, c.gold_hi, p.accent, c.gold_lo, at)
}

// ---- panels and controls ------------------------------------------------------

/// The mockup's `.panel`: a lit surface with a soft drop shadow.
pub fn panel(p: &Palette) -> Div {
    let c = colors();
    div()
        .relative()
        .rounded(px(14.))
        .border_1()
        .border_color(c.line)
        .bg(vgrad(c.surf2, c.surf))
        .shadow(vec![
            shadow(alpha(c.black, 0.42), 0., 18., 40., 0.),
            shadow(alpha(c.black, 0.3), 0., 2., 6., 0.),
        ])
        .text_color(p.text)
        .child(top_light(13., alpha(c.white, 0.05)))
}

/// A button: gold (the main action) or ghost, 40 pixels tall or 32 (`small`).
pub fn btn(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    gold: bool,
    small: bool,
    p: &Palette,
) -> Stateful<Div> {
    let c = colors();
    let (h, pad, size, radius) = if small {
        (32., 13., 13., 8.)
    } else {
        (40., 18., 14., 9.)
    };
    let base = div()
        .id(id.into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(h))
        .px(px(pad))
        .rounded(px(radius))
        .whitespace_nowrap()
        .cursor_pointer();
    let text = sans_text(size, FontWeight::SEMIBOLD)
        .relative()
        .child(text.into());
    if gold {
        base.text_color(p.on_accent)
            .bg(p.accent)
            .shadow(vec![
                shadow(alpha(p.accent, 0.28), 0., 8., 20., 0.),
                shadow(alpha(c.black, 0.4), 0., 2., 4., 0.),
            ])
            .children(gold_fill(radius, 0.55, p))
            .child(top_light(radius, alpha(c.white, 0.45)))
            .hover(|s| s.opacity(0.94))
            .child(text)
    } else {
        base.text_color(p.text)
            .bg(vgrad(c.ghost_top, c.ghost_bottom))
            .border_1()
            .border_color(c.line2)
            .shadow(vec![shadow(alpha(c.black, 0.3), 0., 4., 10., 0.)])
            .child(top_light(radius - 1., alpha(c.white, 0.06)))
            .hover(|s| s.border_color(alpha(p.accent, 0.45)))
            .child(text)
    }
}

/// A selectable pill (spot and opponent pickers).
pub fn pill(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    on: bool,
    p: &Palette,
) -> Stateful<Div> {
    let c = colors();
    let base = div()
        .id(id.into())
        .flex_none()
        .px(px(13.))
        .py(px(6.))
        .rounded(px(99.))
        .border_1()
        .whitespace_nowrap()
        .cursor_pointer()
        .child(sans_text(13., FontWeight::NORMAL).child(text.into()));
    if on {
        base.text_color(c.gold2)
            .border_color(alpha(p.accent, 0.55))
            .bg(alpha(p.accent, 0.12))
    } else {
        base.text_color(p.text_muted)
            .border_color(c.line)
            .bg(vgrad(c.pill_top, c.pill_bottom))
            .hover(|s| s.text_color(p.text))
    }
}

/// The track of a segmented control.
pub fn seg(_p: &Palette) -> Div {
    let c = colors();
    div()
        .flex()
        .flex_none()
        .p(px(3.))
        .rounded(px(10.))
        .bg(c.well)
        .border_1()
        .border_color(c.line)
}

/// One option of a [`seg`].
pub fn seg_item(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    on: bool,
    p: &Palette,
) -> Stateful<Div> {
    let c = colors();
    let base = div()
        .id(id.into())
        .relative()
        .px(px(14.))
        .py(px(6.))
        .rounded(px(7.))
        .whitespace_nowrap()
        .cursor_pointer();
    let text = sans_text(
        13.5,
        if on {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::MEDIUM
        },
    )
    .relative()
    .child(text.into());
    if on {
        base.text_color(p.on_accent)
            .bg(p.accent)
            .shadow(vec![shadow(alpha(p.accent, 0.3), 0., 3., 8., 0.)])
            .children(gold_fill(7., 0.6, p))
            .child(top_light(7., alpha(c.white, 0.4)))
            .child(text)
    } else {
        base.text_color(p.text_muted)
            .hover(|s| s.text_color(p.text))
            .child(text)
    }
}

/// A thin bar filled to `frac` (0–1) with a gradient from `from` to `to`.
pub fn bar(frac: f32, h: f32, track: Rgba, from: Rgba, to: Rgba) -> Div {
    let frac = frac.clamp(0., 1.);
    div().h(px(h)).rounded(px(h / 2.)).bg(track).child(
        div()
            .h_full()
            .w(relative(frac))
            .rounded(px(h / 2.))
            .bg(hgrad(from, to, 1.)),
    )
}

// ---- cards and chips ----------------------------------------------------------

/// The mockup's card sizes.
#[derive(Clone, Copy, PartialEq)]
pub enum CardSize {
    Xs,
    Sm,
    Lg,
}

impl CardSize {
    /// Width, height, corner radius, rank size, pip size, gap.
    fn dims(self) -> (f32, f32, f32, f32, f32, f32) {
        match self {
            CardSize::Xs => (22., 30., 3.5, 12.5, 9., 0.),
            CardSize::Sm => (38., 52., 5., 20., 15., 0.),
            CardSize::Lg => (68., 94., 8., 36., 26., 2.),
        }
    }

    pub fn width(self) -> f32 {
        self.dims().0
    }

    pub fn height(self) -> f32 {
        self.dims().1
    }

    fn face(self) -> &'static str {
        match self {
            CardSize::Xs => "salao/img/card_xs.png",
            CardSize::Sm => "salao/img/card_sm.png",
            CardSize::Lg => "salao/img/card_lg.png",
        }
    }

    /// The back's image (drawn at two sizes, stretched to the others).
    fn back(self) -> &'static str {
        match self {
            CardSize::Xs | CardSize::Sm => "salao/img/back_sm.png",
            CardSize::Lg => "salao/img/back_lg.png",
        }
    }
}

/// A suit's icon file.
pub fn suit_path(s: Suit) -> &'static str {
    match s {
        Suit::CLUBS => "salao/img/suit_c.svg",
        Suit::DIAMONDS => "salao/img/suit_d.svg",
        Suit::HEARTS => "salao/img/suit_h.svg",
        _ => "salao/img/suit_s.svg",
    }
}

/// A suit's color inside text on a dark panel.
pub fn suit_light(c: &Colors, s: Suit) -> Rgba {
    match s {
        Suit::CLUBS => c.suit_light_c,
        Suit::DIAMONDS => c.suit_light_d,
        Suit::HEARTS => c.suit_light_h,
        _ => c.suit_light_s,
    }
}

/// A playing card (`None`: its back), `k` times the mockup's size.
pub fn playing_card(p: &Palette, card: Option<Card>, size: CardSize, k: f32) -> Div {
    let c = colors();
    let (w, h, radius, rank, pip, gap) = size.dims();
    let (w, h) = (w * k, h * k);
    let shadows = match (size, card) {
        (CardSize::Xs, _) => vec![shadow(alpha(c.black, 0.4), 0., 2. * k, 4. * k, 0.)],
        (_, None) => vec![shadow(alpha(c.black, 0.45), 0., 8. * k, 16. * k, 0.)],
        _ => vec![
            shadow(alpha(c.black, 0.45), 0., 8. * k, 16. * k, 0.),
            shadow(alpha(c.black, 0.5), 0., k, 2. * k, 0.),
        ],
    };
    let base = div()
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .rounded(px(radius * k))
        .shadow(shadows);
    let Some(card) = card else {
        return base
            .bg(p.card_back)
            .child(image(size.back(), w, h).absolute().top_0().left_0());
    };
    let ink = p.suit(card.suit());
    base.bg(p.card_face)
        .child(image(size.face(), w, h).absolute().top_0().left_0())
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(gap * k))
                .child(
                    RANK.text(rank * k, 1.)
                        .text_color(ink)
                        .child(card.rank().to_char().to_string()),
                )
                .child(icon(suit_path(card.suit()), pip * k, ink)),
        )
}

/// A chip's image by denomination (1, 5, 25, 100 or 500).
fn chip_path(value: u16) -> &'static str {
    match value {
        500 => "salao/img/chip_500.png",
        100 => "salao/img/chip_100.png",
        25 => "salao/img/chip_25.png",
        5 => "salao/img/chip_5.png",
        _ => "salao/img/chip_1.png",
    }
}

/// A stack of `n` chips of one denomination, `k` times the mockup's size:
/// 26-pixel discs, each 4 pixels above the one below.
pub fn chip_stack(value: u16, n: usize, k: f32) -> Div {
    let n = n.max(1);
    let h = 26. + 4. * (n as f32 - 1.);
    let mut stack = div().relative().flex_none().w(px(26. * k)).h(px(h * k));
    for i in 0..n {
        // The image is a 38 × 40 box with the disc at (6, 3) and its
        // shadows around it.
        let top = h - 26. - 4. * i as f32 - 3.;
        stack = stack.child(
            image(chip_path(value), 38. * k, 40. * k)
                .absolute()
                .left(px(-6. * k))
                .top(px(top * k)),
        );
    }
    stack
}

/// The chip stacks that stand for `bb` big blinds: the fewest chips, at
/// most three stacks of at most seven.
pub fn chip_stacks(bb: f64) -> Vec<(u16, usize)> {
    let mut left = bb.round().max(1.) as u64;
    let mut out = Vec::new();
    for value in [500u16, 100, 25, 5, 1] {
        let n = (left / u64::from(value)).min(7) as usize;
        if n > 0 && out.len() < 3 {
            out.push((value, n));
            left -= n as u64 * u64::from(value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_for_an_amount() {
        assert_eq!(chip_stacks(10.), vec![(5, 2)]);
        assert_eq!(chip_stacks(40.), vec![(25, 1), (5, 3)]);
        assert_eq!(chip_stacks(0.4), vec![(1, 1)]);
        assert_eq!(chip_stacks(2.5), vec![(1, 3)]);
        assert_eq!(chip_stacks(131.), vec![(100, 1), (25, 1), (5, 1)]);
        // Never more than three stacks of seven.
        let big = chip_stacks(10_000.);
        assert!(big.len() <= 3 && big.iter().all(|(_, n)| *n <= 7));
    }

    #[test]
    fn baselines_line_up() {
        // A small text beside a big one is lifted, never pushed down.
        let lift = Face::lift((STAT, 56., 1.), (STAT_UNIT, 24., 1.));
        assert!(lift > 0. && lift < 10., "{lift}");
        assert_eq!(Face::lift((SANS, 14., LEADING), (SANS, 14., LEADING)), 0.);
    }

    /// The line boxes measured in the mockup (ascent, descent).
    #[test]
    fn line_boxes_are_the_mockups() {
        for (size, up, down) in [
            (10.5, 10., 3.),
            (11., 11., 3.),
            (11.5, 11., 3.),
            (12., 12., 3.),
            (12.5, 11., 4.),
            (13., 12., 4.),
            (13.5, 12., 4.),
            (14., 13., 4.),
            (14.5, 13., 4.),
        ] {
            assert_eq!(SANS.rounded(size), (up, down), "sans {size}");
        }
        for (size, up, down) in [
            (12.5, 12., 3.),
            (16., 16., 4.),
            (17., 17., 4.),
            (21., 21., 5.),
            (24., 23., 6.),
            (38., 37., 10.),
            (56., 55., 14.),
        ] {
            assert_eq!(H1.rounded(size), (up, down), "serif {size}");
        }
        // The nav's 14 px in a 20.3 px line: one pixel of leading above.
        assert_eq!(SANS.mock_baseline(14., 20.3), 14.);
    }
}

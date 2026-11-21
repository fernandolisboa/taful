//! Baralho typography. GPUI has neither letter-spacing nor variable-font
//! axes, so each (optical size, weight, tracking) the mockups use is a
//! static font file with its own family name
//! (`apps/desktop/assets/baralho/fonts/build.py`). Sizes are in design
//! pixels (the 1600×900 mockup canvas) and scale with the window.

use gpui::{FontWeight, SharedString, Styled};

use super::u;

macro_rules! fonts {
    (
        display: [$(($o:literal, $w:literal, $t:literal)),* $(,)?],
        caps: [$(($cw:literal, $ct:literal)),* $(,)?],
        text: [$($file:literal),* $(,)?] $(,)?
    ) => {
        /// Display faces as (optical size, weight, tracking in -1/1000 em, family).
        pub const DISPLAY_FACES: &[(u16, u16, u16, &str)] = &[
            $(($o, $w, $t, concat!("Baralho D", $o, " W", $w, " T", $t))),*
        ];
        /// Uppercase-label faces as (weight, tracking in 1/1000 em, family).
        pub const CAPS_FACES: &[(u16, u16, &str)] = &[
            $(($cw, $ct, concat!("Baralho Caps W", $cw, " T", $ct))),*
        ];
        /// Every font file of the style.
        pub const FONTS: &[&[u8]] = &[
            $(include_bytes!(concat!(
                "../../assets/baralho/fonts/display-", $o, "-", $w, "-", $t, ".ttf"
            ))),*,
            $(include_bytes!(concat!(
                "../../assets/baralho/fonts/caps-", $cw, "-", $ct, ".ttf"
            ))),*,
            $(include_bytes!(concat!("../../assets/baralho/fonts/", $file))),*
        ];
    };
}

fonts! {
    display: [
        (14, 700, 10), (14, 700, 70), (14, 800, 0), (14, 800, 20), (14, 800, 40),
        (17, 700, 10), (17, 700, 20), (17, 800, 0),
        (21, 800, 20), (21, 800, 25), (21, 800, 30), (21, 800, 40),
        (26, 800, 30), (26, 800, 40),
        (32, 800, 30), (32, 800, 40),
        (46, 800, 40),
        (96, 800, 35), (96, 800, 40), (96, 800, 50),
    ],
    caps: [(700, 120), (800, 60), (800, 120), (800, 160)],
    text: [
        "figtree-400.ttf", "figtree-500.ttf", "figtree-600.ttf", "figtree-700.ttf",
        "figtree-800.ttf", "figtree-400-italic.ttf", "figtree-700-italic.ttf",
    ],
}

/// The body family (plain Figtree, real weights).
pub const TEXT_FAMILY: &str = "Figtree";

/// One text style: family, weight, size and line height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ty {
    family: &'static str,
    weight: u16,
    /// Font size in design pixels.
    pub size: f32,
    /// Line height as a multiple of the size.
    pub lh: f32,
}

/// The optical-size bucket a design size falls in.
fn opsz_for(size: f32) -> u16 {
    match size {
        s if s < 15.75 => 14,
        s if s < 18.5 => 17,
        s if s < 23. => 21,
        s if s < 28. => 26,
        s if s < 38. => 32,
        s if s < 60. => 46,
        _ => 96,
    }
}

/// The display family for an optical size, weight and tracking: the exact
/// face when it exists, else the nearest tracking at that size and weight,
/// else the nearest face of that size.
fn display_family(opsz: u16, weight: u16, track: u16) -> &'static str {
    let near = |faces: &mut dyn Iterator<Item = &'static (u16, u16, u16, &'static str)>| {
        faces
            .min_by_key(|(_, w, t, _)| (w.abs_diff(weight), t.abs_diff(track)))
            .map(|f| f.3)
    };
    near(&mut DISPLAY_FACES.iter().filter(|f| f.0 == opsz))
        .or_else(|| near(&mut DISPLAY_FACES.iter()))
        .unwrap_or(TEXT_FAMILY)
}

/// Display type (Bricolage Grotesque) at `size` design pixels with the
/// optical size the mockups get from the browser (the size itself).
/// `track` is the negative tracking in thousandths of an em. Line height 1.
pub fn disp(size: f32, weight: u16, track: u16) -> Ty {
    disp_at(opsz_for(size), size, weight, track)
}

/// Display type at a fixed optical size (headings set to `opsz 96`).
pub fn disp_at(opsz: u16, size: f32, weight: u16, track: u16) -> Ty {
    Ty {
        family: display_family(opsz, weight, track),
        weight,
        size,
        lh: 1.,
    }
}

/// Body type (Figtree) at `size` design pixels; line height 1.4.
pub fn text(size: f32, weight: u16) -> Ty {
    Ty {
        family: TEXT_FAMILY,
        weight,
        size,
        lh: 1.4,
    }
}

/// Tracked uppercase label type; `track` in thousandths of an em.
pub fn caps(size: f32, weight: u16, track: u16) -> Ty {
    let family = CAPS_FACES
        .iter()
        .min_by_key(|(w, t, _)| (w.abs_diff(weight), t.abs_diff(track)))
        .map_or(TEXT_FAMILY, |f| f.2);
    Ty {
        family,
        weight,
        size,
        lh: 1.4,
    }
}

impl Ty {
    /// The same style with another line height (multiple of the size).
    pub fn lh(mut self, lh: f32) -> Self {
        self.lh = lh;
        self
    }

    /// Applies the style at its size in window pixels, unscaled: text over
    /// the canvas (the level question) that must stay readable in a small
    /// window.
    pub fn on_px<E: Styled>(self, e: E) -> E {
        e.font_family(SharedString::new_static(self.family))
            .font_weight(FontWeight(f32::from(self.weight)))
            .text_size(gpui::px(self.size))
            .line_height(gpui::px((self.size * self.lh).round()))
    }

    /// Applies the style to an element.
    pub fn on<E: Styled>(self, e: E) -> E {
        e.font_family(SharedString::new_static(self.family))
            .font_weight(FontWeight(f32::from(self.weight)))
            .text_size(u(self.size))
            .line_height(u(self.size * self.lh))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mockup_style_has_its_exact_face() {
        // (size, weight, tracking) of every display style in the mockups.
        for (size, w, t) in [
            (30., 800, 40),
            (16.5, 700, 10),
            (14., 700, 10),
            (13., 800, 40),
            (21., 800, 40),
            (32., 800, 40),
            (42., 800, 40),
            (50., 800, 40),
            (31., 800, 30),
            (22., 800, 20),
            (17.5, 700, 20),
            (17., 800, 0),
            (26., 800, 30),
            (20., 800, 25),
            (20., 800, 30),
            (26., 800, 40),
            (14., 800, 20),
            (14.5, 800, 20),
            (19., 800, 20),
            (18., 800, 0),
            (34., 800, 30),
            (15., 700, 70),
            (14., 800, 0),
            (24., 800, 30),
            (16., 800, 0),
        ] {
            let family = disp(size, w, t).family;
            let want = format!("Baralho D{} W{w} T{t}", opsz_for(size));
            assert_eq!(family, want, "{size}px");
        }
        for t in [35, 40, 50] {
            assert_eq!(
                disp_at(96, 46., 800, t).family,
                format!("Baralho D96 W800 T{t}")
            );
        }
        for (w, t) in [(700, 120), (800, 60), (800, 120), (800, 160)] {
            assert_eq!(caps(12., w, t).family, format!("Baralho Caps W{w} T{t}"));
        }
    }

    #[test]
    fn font_files_are_embedded() {
        assert_eq!(FONTS.len(), DISPLAY_FACES.len() + CAPS_FACES.len() + 7);
        assert!(FONTS.iter().all(|f| f.len() > 10_000));
    }
}

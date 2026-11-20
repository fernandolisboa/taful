//! The Taful mark: a bow tie whose knot is a diamond (D20).
//!
//! The drawn sources are the SVGs in `apps/desktop/assets/icon/`. This module
//! holds the small cut of the mark as geometry, so a client can paint it at
//! any pixel size without an SVG renderer (the notification-area icon).

use crate::theme::Color;

/// The wings' color (amber).
pub const WINGS: Color = Color {
    r: 0xf0,
    g: 0xb0,
    b: 0x40,
    a: 0xff,
};

/// The knot's color (cream).
pub const KNOT: Color = Color {
    r: 0xf6,
    g: 0xef,
    b: 0xdc,
    a: 0xff,
};

/// The whole mark on a light background, where amber and cream do not show
/// (dark ink).
pub const INK: Color = Color {
    r: 0x16,
    g: 0x24,
    b: 0x2b,
    a: 0xff,
};

/// What the mark is painted over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    /// A dark surface: amber wings, cream knot.
    Dark,
    /// A light surface: the whole mark in dark ink.
    Light,
}

/// The mark is drawn on a square box of this many units.
const BOX: f32 = 24.;

/// Outline of the left wing in box units: the path of `mark-small.svg` with
/// its curves flattened. The right wing is its mirror image.
// One coordinate happens to read like τ.
#[allow(clippy::approx_constant)]
const WING: [(f32, f32); 23] = [
    (9.583, 8.700),
    (8.540, 8.367),
    (7.432, 7.933),
    (6.291, 7.425),
    (5.151, 6.867),
    (4.043, 6.283),
    (3.000, 5.700),
    (2.266, 5.463),
    (1.625, 5.606),
    (1.172, 6.059),
    (1.000, 6.750),
    (1.000, 17.250),
    (1.172, 17.941),
    (1.625, 18.394),
    (2.266, 18.537),
    (3.000, 18.300),
    (4.043, 17.717),
    (5.151, 17.133),
    (6.291, 16.575),
    (7.432, 16.067),
    (8.540, 15.633),
    (9.583, 15.300),
    (7.044, 12.000),
];

/// Half width and half height of the knot, a diamond centered on the box.
const KNOT_HALF: (f32, f32) = (3.0, 3.9);

/// Samples per pixel side for the antialiased edges.
const SAMPLES: u32 = 4;

fn in_wing(x: f32, y: f32) -> bool {
    // The right wing mirrors the left one.
    let x = if x > BOX / 2. { BOX - x } else { x };
    let mut inside = false;
    let mut prev = WING[WING.len() - 1];
    for &cur in &WING {
        let ((x0, y0), (x1, y1)) = (prev, cur);
        if (y0 > y) != (y1 > y) && x < x0 + (y - y0) / (y1 - y0) * (x1 - x0) {
            inside = !inside;
        }
        prev = cur;
    }
    inside
}

fn in_knot(x: f32, y: f32) -> bool {
    let (dx, dy) = ((x - BOX / 2.).abs(), (y - BOX / 2.).abs());
    dx / KNOT_HALF.0 + dy / KNOT_HALF.1 <= 1.
}

/// The mark on a transparent background, in the colors for `ground`:
/// `size`×`size` pixels, four bytes each (red, green, blue, alpha; alpha not
/// premultiplied), rows top to bottom.
pub fn pixels(size: u32, ground: Ground) -> Vec<u8> {
    let (wing_color, knot_color) = match ground {
        Ground::Dark => (WINGS, KNOT),
        Ground::Light => (INK, INK),
    };
    let unit = BOX / size as f32;
    let step = unit / SAMPLES as f32;
    let total = (SAMPLES * SAMPLES) as f32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut wings, mut knot) = (0u32, 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = px as f32 * unit + (sx as f32 + 0.5) * step;
                    let y = py as f32 * unit + (sy as f32 + 0.5) * step;
                    if in_knot(x, y) {
                        knot += 1;
                    } else if in_wing(x, y) {
                        wings += 1;
                    }
                }
            }
            let covered = wings + knot;
            if covered == 0 {
                out.extend([0, 0, 0, 0]);
                continue;
            }
            // The pixel's color is the mix of what covers it; how much of the
            // pixel is covered goes to alpha.
            let mix = |w: u8, k: u8| {
                ((u32::from(w) * wings + u32::from(k) * knot) as f32 / covered as f32).round() as u8
            };
            out.extend([
                mix(wing_color.r, knot_color.r),
                mix(wing_color.g, knot_color.g),
                mix(wing_color.b, knot_color.b),
                (covered as f32 / total * 255.).round() as u8,
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn draws_a_bow_tie_with_a_knot() {
        for size in [16, 20, 24, 32, 48] {
            let px = pixels(size, Ground::Dark);
            assert_eq!(px.len(), (size * size * 4) as usize);
            let mid = size / 2;
            // The knot in the middle, a wing on each side of it.
            assert_eq!(at(&px, size, mid, mid), [KNOT.r, KNOT.g, KNOT.b, 255]);
            let wing = [WINGS.r, WINGS.g, WINGS.b, 255];
            assert_eq!(at(&px, size, size / 8, mid), wing, "size {size}");
            assert_eq!(at(&px, size, size - 1 - size / 8, mid), wing, "size {size}");
            // Nothing in the corners, nor above and below the knot.
            for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
                assert_eq!(at(&px, size, x, y)[3], 0);
            }
            assert_eq!(at(&px, size, mid, size / 8)[3], 0);
            assert_eq!(at(&px, size, mid, size - 1 - size / 8)[3], 0);
        }
    }

    #[test]
    fn the_two_wings_mirror_each_other() {
        let size = 32;
        let px = pixels(size, Ground::Dark);
        for y in 0..size {
            for x in 0..size {
                assert_eq!(at(&px, size, x, y), at(&px, size, size - 1 - x, y));
            }
        }
    }

    #[test]
    fn a_gap_separates_the_knot_from_the_wings() {
        // Along the middle row, left to center: wing, clear pixels, knot.
        let size = 32;
        let px = pixels(size, Ground::Dark);
        let alpha: Vec<u8> = (0..=size / 2)
            .map(|x| at(&px, size, x, size / 2)[3])
            .collect();
        let wing = alpha.iter().position(|&a| a == 255).unwrap();
        let clear = alpha[wing..].iter().filter(|&&a| a == 0).count();
        assert!(clear >= 2, "{alpha:?}");
        assert_eq!(alpha[alpha.len() - 1], 255);
    }

    #[test]
    fn on_a_light_ground_the_mark_is_one_dark_color() {
        let size = 32;
        let (dark, light) = (pixels(size, Ground::Dark), pixels(size, Ground::Light));
        for (d, l) in dark.chunks(4).zip(light.chunks(4)) {
            // Same shape, one color.
            assert_eq!(d[3], l[3]);
            if l[3] > 0 {
                assert_eq!(&l[..3], &[INK.r, INK.g, INK.b]);
            }
        }
    }

    /// The numbers of one SVG path, in order.
    fn numbers(d: &str) -> Vec<f32> {
        d.split(|c: char| c.is_ascii_alphabetic() || c == ' ' || c == ',')
            .filter(|t| !t.is_empty())
            .map(|t| t.parse().unwrap())
            .collect()
    }

    /// The outline here is a copy of `mark-small.svg`: a change to the
    /// drawing has to be made in both.
    #[test]
    fn the_outline_follows_the_drawn_mark() {
        let svg = include_str!("../../../apps/desktop/assets/icon/mark-small.svg");
        let paths: Vec<&str> = svg
            .split(" d=\"")
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        assert_eq!(paths.len(), 2, "wings, then knot");

        // Wings: every point where the left wing's path changes segment
        // (the first sub-path) is a point of `WING`.
        let left = paths[0].split('Z').next().unwrap();
        let mut ends = 0;
        for coords in left.split(|c: char| c.is_ascii_alphabetic()) {
            let n = numbers(coords);
            if n.len() < 2 {
                continue;
            }
            let (x, y) = (n[n.len() - 2], n[n.len() - 1]);
            assert!(
                WING.iter()
                    .any(|&(wx, wy)| (wx - x).abs() < 0.01 && (wy - y).abs() < 0.01),
                "({x}, {y}) of mark-small.svg is not on WING"
            );
            ends += 1;
        }
        assert!(ends >= 7, "{ends} segment ends");

        // Knot: a diamond around the center of the box.
        let k = numbers(paths[1]);
        assert_eq!(k.len(), 8);
        let (xs, ys): (Vec<f32>, Vec<f32>) = k.chunks(2).map(|p| (p[0], p[1])).unzip();
        let span = |v: &[f32]| {
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!((span(&xs) / 2. - KNOT_HALF.0).abs() < 0.01);
        assert!((span(&ys) / 2. - KNOT_HALF.1).abs() < 0.01);
    }
}

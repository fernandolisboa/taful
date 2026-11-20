//! Generated textures. GPUI has no radial or repeating gradients, so the
//! felt's glow and dots, the table top and the card backs are small
//! images computed from the style's colors, cached by size, and painted
//! with rounded corners.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Bounds, Canvas, Corners, Pixels, RenderImage, Window, canvas, point, px, size};
use image::{Frame, RgbaImage};

use super::{Bx, CANVAS_W, Tone, scale};

/// A texture that fills an element.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tex {
    /// A card back: cross-hatch with a cream ring inside the edge.
    Back,
    /// A card back without the ring (the mockup's small and extra-large
    /// cards have none).
    BackPlain,
    /// A locked lesson's back: a fainter hatch and ring.
    Locked,
    /// Diagonal stripes (tiny face-down cards).
    Stripes,
    /// The table top: a lit oval with a shaded upper edge.
    TableTop,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Fill {
        tex: Tex,
        w: u32,
        h: u32,
        /// Corner radius and design scale, in thousandths.
        radius: u32,
        k: u32,
    },
    Glow {
        w: u32,
        h: u32,
        k: u32,
        left: i32,
    },
    Dots {
        side: u32,
    },
}

/// Frames a texture may go unused before it is freed.
const KEEP_FRAMES: u64 = 240;
/// The window glow is computed at one pixel in this many (it is smooth).
const GLOW_STEP: f32 = 4.;
/// Dot rows and columns per tile, and the dots' pitch in design pixels.
const DOT_TILE: u32 = 24;
const DOT_PITCH: f32 = 9.;

thread_local! {
    static STORE: RefCell<HashMap<Key, (Arc<RenderImage>, u64)>> = RefCell::new(HashMap::new());
    static FRAME: Cell<u64> = const { Cell::new(0) };
}

/// Starts a frame and frees the textures that went unused for a while
/// (sizes change as the window is resized).
pub fn begin_frame(window: &mut Window, cx: &mut App) {
    let now = FRAME.with(|f| {
        f.set(f.get() + 1);
        f.get()
    });
    let stale: Vec<Arc<RenderImage>> = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let old: Vec<Key> = s
            .iter()
            .filter(|(_, (_, used))| used + KEEP_FRAMES < now)
            .map(|(k, _)| *k)
            .collect();
        old.iter()
            .filter_map(|k| s.remove(k))
            .map(|(image, _)| image)
            .collect()
    });
    for image in stale {
        cx.drop_image(image, Some(window));
    }
}

fn cached(key: Key, make: impl FnOnce() -> RgbaImage) -> Arc<RenderImage> {
    let now = FRAME.with(Cell::get);
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let entry = s
            .entry(key)
            .or_insert_with(|| (Arc::new(RenderImage::new(vec![Frame::new(make())])), now));
        entry.1 = now;
        entry.0.clone()
    })
}

/// A color as the bytes GPUI's images hold (BGRA, straight alpha).
fn bytes(c: Tone) -> [f32; 4] {
    [c.b * 255., c.g * 255., c.r * 255., c.a * 255.]
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0., 1.);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// `top` at `alpha` over an opaque `base`.
fn over(base: [f32; 4], top: [f32; 4], alpha: f32) -> [f32; 4] {
    let mut out = mix(base, top, alpha);
    out[3] = base[3];
    out
}

fn put(img: &mut RgbaImage, x: u32, y: u32, c: [f32; 4]) {
    img.put_pixel(
        x,
        y,
        image::Rgba(c.map(|v| v.round().clamp(0., 255.) as u8)),
    );
}

/// Three stops of a radial gradient at `t` (0 centre, 1 edge), the middle
/// stop at 55% like the mockups.
fn radial(stops: [[f32; 4]; 3], t: f32) -> [f32; 4] {
    if t < 0.55 {
        mix(stops[0], stops[1], t / 0.55)
    } else {
        mix(stops[1], stops[2], (t - 0.55) / 0.45)
    }
}

/// Signed distance to a rounded rectangle of size `w`×`h` (negative
/// inside).
fn rounded_distance(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let r = r.min(w / 2.).min(h / 2.);
    let (qx, qy) = (
        (x - w / 2.).abs() - (w / 2. - r),
        (y - h / 2.).abs() - (h / 2. - r),
    );
    qx.max(qy).min(0.) + (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt() - r
}

/// Coverage of the stripe `[0, width)` of a pattern repeating every
/// `period`, at position `t`, for a sample `blur` wide.
fn stripe(t: f32, width: f32, period: f32, blur: f32) -> f32 {
    let t = t.rem_euclid(period);
    let rise = (t / blur + 0.5).clamp(0., 1.);
    let fall = ((width - t) / blur + 0.5).clamp(0., 1.);
    let wrap = ((t - period) / blur + 0.5).clamp(0., 1.);
    (rise.min(fall)).max(wrap)
}

/// Positions along the two 45° diagonals of a `w`×`h` box, measured as
/// CSS measures a gradient line (from the corner the line starts at).
fn diagonals(x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let half = (w + h) * D / 2.;
    let (cx, cy) = (x - w / 2., y - h / 2.);
    ((cx - cy) * D + half, (-cx - cy) * D + half)
}

fn fill_image(tex: Tex, bx: &Bx, w: u32, h: u32, radius: f32, k: f32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    let (dw, dh) = (w as f32 / k, h as f32 / k);
    // One device pixel in design pixels.
    let blur = 1. / k;
    let cream = bytes(bx.cream);
    for py in 0..h {
        for px_ in 0..w {
            let (x, y) = ((px_ as f32 + 0.5) / k, (py as f32 + 0.5) / k);
            let c = match tex {
                Tex::Back | Tex::BackPlain | Tex::Locked => {
                    let (base, line, ring) = match tex {
                        Tex::Back => (bx.back, 0.2, 1.),
                        Tex::BackPlain => (bx.back, 0.2, 0.),
                        _ => (bx.back_locked, 0.13, 0.55),
                    };
                    let (a, b) = diagonals(x, y, dw, dh);
                    let mut c = bytes(base);
                    c = over(c, cream, line * stripe(b, 2., 8., blur));
                    c = over(c, cream, line * stripe(a, 2., 8., blur));
                    let d = rounded_distance(x, y, dw, dh, radius);
                    over(c, cream, ring * ((d + 3.) / blur + 0.5).clamp(0., 1.))
                }
                Tex::Stripes => {
                    let (a, _) = diagonals(x, y, dw, dh);
                    mix(bytes(bx.back_hi), bytes(bx.back), stripe(a, 3., 6., blur))
                }
                Tex::TableTop => {
                    let (ex, ey) = ((x - dw * 0.5) / 460., (y - dh * 0.45) / 250.);
                    let c = radial(
                        [bytes(bx.table_hi), bytes(bx.table_mid), bytes(bx.table_lo)],
                        (ex * ex + ey * ey).sqrt(),
                    );
                    // The rail's shade on the upper edge: what a copy of
                    // the shape moved 6 px down leaves uncovered.
                    let d = rounded_distance(x, y - 6., dw, dh, radius);
                    over(c, bytes(bx.shade), 0.18 * (d / blur + 0.5).clamp(0., 1.))
                }
            };
            put(&mut img, px_, py, c);
        }
    }
    img
}

/// An element that paints `tex` over its bounds with rounded corners
/// (`radius` in design pixels). Size and position it like any element.
pub fn fill(tex: Tex, bx: Bx, radius: f32) -> Canvas<()> {
    // The scale now, not when painting: the caller may be zoomed.
    let scale = scale();
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let dpr = window.scale_factor();
            let k = scale * dpr;
            let w = (f32::from(bounds.size.width) * dpr).round().max(1.) as u32;
            let h = (f32::from(bounds.size.height) * dpr).round().max(1.) as u32;
            let key = Key::Fill {
                tex,
                w,
                h,
                radius: (radius * 1000.) as u32,
                k: (k * 1000.) as u32,
            };
            let image = cached(key, || fill_image(tex, &bx, w, h, radius, k));
            let _ = window.paint_image(bounds, Corners::all(px(radius * scale)), image, 0, false);
        },
    )
}

fn glow_image(bx: &Bx, w: u32, h: u32, k: f32, left: f32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    let stops = [bytes(bx.felt_hi), bytes(bx.felt), bytes(bx.felt_lo)];
    for py in 0..h {
        for px_ in 0..w {
            // Design coordinates of the sample, the canvas starting at `left`.
            let x = (px_ as f32 + 0.5) * GLOW_STEP / k - left;
            let y = (py as f32 + 0.5) * GLOW_STEP / k;
            let (ex, ey) = (
                (x - CANVAS_W * 0.5) / 1100.,
                (y - super::CANVAS_H * 0.46) / 620.,
            );
            put(&mut img, px_, py, radial(stops, (ex * ex + ey * ey).sqrt()));
        }
    }
    img
}

fn dots_image(bx: &Bx, side: u32) -> RgbaImage {
    let mut img = RgbaImage::new(side, side);
    // Device pixels per design pixel inside the tile.
    let k = side as f32 / (DOT_TILE as f32 * DOT_PITCH);
    let dot = bytes(bx.paper);
    for py in 0..side {
        for px_ in 0..side {
            // 2×2 samples: the dots are about a pixel wide.
            let mut a = 0.;
            for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let x = ((px_ as f32 + sx) / k).rem_euclid(DOT_PITCH) - DOT_PITCH / 2.;
                let y = ((py as f32 + sy) / k).rem_euclid(DOT_PITCH) - DOT_PITCH / 2.;
                a += ((1.3 - (x * x + y * y).sqrt()) / 0.3).clamp(0., 1.);
            }
            put(
                &mut img,
                px_,
                py,
                [dot[0], dot[1], dot[2], 255. * 0.035 * a / 4.],
            );
        }
    }
    img
}

/// The window's felt: a soft glow centred on the canvas (which starts
/// `left` window pixels in) under a fine grid of dots.
pub fn felt(bx: Bx, left: Pixels) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds: Bounds<Pixels>, (), window, _| {
            let dpr = window.scale_factor();
            let k = scale() * dpr;
            let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let w = (bw * dpr / GLOW_STEP).ceil().max(1.) as u32;
            let h = (bh * dpr / GLOW_STEP).ceil().max(1.) as u32;
            let left_design = f32::from(left) / scale();
            let key = Key::Glow {
                w,
                h,
                k: (k * 1000.) as u32,
                left: left_design.round() as i32,
            };
            let glow = cached(key, || glow_image(&bx, w, h, k, left_design));
            let _ = window.paint_image(bounds, Corners::default(), glow, 0, false);

            let tile_design = DOT_TILE as f32 * DOT_PITCH;
            let side = (tile_design * k).round().max(1.) as u32;
            let dots = cached(Key::Dots { side }, || dots_image(&bx, side));
            let tile = tile_design * scale();
            let (mut y, mut row) = (0., 0);
            while y < bh && row < 64 {
                let (mut x, mut col) = (0., 0);
                while x < bw && col < 64 {
                    let at = Bounds::new(
                        bounds.origin + point(px(x), px(y)),
                        size(px(tile), px(tile)),
                    );
                    let _ = window.paint_image(at, Corners::default(), dots.clone(), 0, false);
                    x += tile;
                    col += 1;
                }
                y += tile;
                row += 1;
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripes_repeat() {
        assert!(stripe(1., 2., 8., 0.1) > 0.99);
        assert!(stripe(5., 2., 8., 0.1) < 0.01);
        assert!(stripe(9., 2., 8., 0.1) > 0.99);
        assert!((stripe(0., 2., 8., 1.) - 0.5).abs() < 0.01);
    }

    #[test]
    fn rounded_distance_is_signed() {
        assert!(rounded_distance(50., 20., 100., 40., 20.) < 0.);
        assert!(rounded_distance(0., 0., 100., 40., 20.) > 0.);
        assert!(rounded_distance(50., 0., 100., 40., 20.).abs() < 0.001);
    }
}

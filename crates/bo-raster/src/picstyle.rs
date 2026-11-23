//! Pixel operations of picture styles (BlueDoc round 19): artistic effects, soft edges,
//! bevel, 3-D rotation, inner shadow, and the separate images of outer shadow, glow and
//! reflection — named by the tokens of [`bo_model::picture_style::style_ops`].
//!
//! These are BlueOffice's own approximations of the effects described by ECMA-376 Part 1
//! (§20.1.8.26 effectLst children, §20.1.5 3-D) and [MS-ODRAWXML] (`a14:imgEffect`
//! artistic effects): box blurs (three passes ≈ Gaussian), max filters, Sobel edges,
//! posterising, a hashed grain and a projective warp.

use crate::codecs::Rgba;
use bo_model::picture_style::{style_ops, StyleOp};

/// Largest side the blurred effect images (shadow, glow) are computed at.
const EFFECT_MAX: u32 = 640;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn px(v: f64) -> usize {
    v.round().max(0.0) as usize
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u8(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn new_img(w: usize, h: usize) -> Rgba {
    Rgba {
        width: u32::try_from(w.max(1)).unwrap_or(1),
        height: u32::try_from(h.max(1)).unwrap_or(1),
        pixels: vec![0; w.max(1) * h.max(1) * 4],
    }
}

/// A copy of a picture (Rgba is not Clone).
fn dup(img: &Rgba) -> Rgba {
    Rgba {
        width: img.width,
        height: img.height,
        pixels: img.pixels.clone(),
    }
}

fn dims(img: &Rgba) -> (usize, usize) {
    (img.width as usize, img.height as usize)
}

/// One box-blur pass of a single-channel plane, horizontally then vertically.
fn box_pass(v: &mut [f32], w: usize, h: usize, rx: usize, ry: usize) {
    let mut tmp = vec![0f32; v.len()];
    if rx > 0 {
        let n = (2 * rx + 1) as f32;
        for y in 0..h {
            let row = &v[y * w..(y + 1) * w];
            let mut acc: f32 = (0..=rx).map(|i| row.get(i).copied().unwrap_or(0.0)).sum();
            for x in 0..w {
                tmp[y * w + x] = acc / n;
                let add = x + rx + 1;
                if add < w {
                    acc += row[add];
                }
                if x >= rx {
                    acc -= row[x - rx];
                }
            }
        }
        v.copy_from_slice(&tmp);
    }
    if ry > 0 {
        let n = (2 * ry + 1) as f32;
        for x in 0..w {
            let at = |y: usize| v[y * w + x];
            let mut acc: f32 = (0..=ry).filter(|i| *i < h).map(at).sum();
            for y in 0..h {
                tmp[y * w + x] = acc / n;
                let add = y + ry + 1;
                if add < h {
                    acc += at(add);
                }
                if y >= ry {
                    acc -= at(y - ry);
                }
            }
        }
        v.copy_from_slice(&tmp);
    }
}

/// Gaussian-like blur (three box passes) of a plane with radii in pixels.
fn blur_plane(v: &mut [f32], w: usize, h: usize, rx: f64, ry: f64) {
    let (bx, by) = (px(rx / 2.0), px(ry / 2.0));
    if bx == 0 && by == 0 {
        return;
    }
    for _ in 0..3 {
        box_pass(v, w, h, bx, by);
    }
}

/// Max filter (dilation) of a plane.
fn dilate(v: &mut [f32], w: usize, h: usize, rx: usize, ry: usize) {
    let mut tmp = v.to_vec();
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(rx), (x + rx).min(w - 1));
            tmp[y * w + x] = v[y * w + a..=y * w + b].iter().copied().fold(0.0, f32::max);
        }
    }
    for x in 0..w {
        for y in 0..h {
            let (a, b) = (y.saturating_sub(ry), (y + ry).min(h - 1));
            v[y * w + x] = (a..=b).map(|yy| tmp[yy * w + x]).fold(0.0, f32::max);
        }
    }
}

fn alpha_plane(img: &Rgba) -> Vec<f32> {
    img.pixels
        .chunks_exact(4)
        .map(|p| f32::from(p[3]) / 255.0)
        .collect()
}

/// Blur all four channels (premultiplied while blurring).
fn blur_rgba(img: &mut Rgba, rx: f64, ry: f64) {
    let (w, h) = dims(img);
    let a = alpha_plane(img);
    let mut planes: Vec<Vec<f32>> = (0..3)
        .map(|c| {
            img.pixels
                .chunks_exact(4)
                .zip(&a)
                .map(|(p, a)| f32::from(p[c]) * a)
                .collect()
        })
        .collect();
    let mut ab = a;
    for p in &mut planes {
        blur_plane(p, w, h, rx, ry);
    }
    blur_plane(&mut ab, w, h, rx, ry);
    for (i, p) in img.pixels.chunks_exact_mut(4).enumerate() {
        let al = ab[i];
        for c in 0..3 {
            p[c] = if al > 0.0 {
                to_u8(f64::from(planes[c][i] / al))
            } else {
                0
            };
        }
        p[3] = to_u8(f64::from(al) * 255.0);
    }
}

/// A smaller copy for blurred effect images (they lose nothing at a lower resolution).
fn shrink_for_effect(img: &Rgba) -> Rgba {
    let m = img.width.max(img.height);
    if m <= EFFECT_MAX {
        return dup(img);
    }
    let k = f64::from(EFFECT_MAX) / f64::from(m);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (w, h) = (
        ((f64::from(img.width) * k).round() as u32).max(1),
        ((f64::from(img.height) * k).round() as u32).max(1),
    );
    crate::picfx::resample(img, w, h)
}

/// The picture's alpha in `color` with opacity `a`, on a canvas padded by `pw`/`ph`.
fn silhouette(img: &Rgba, color: u32, a: f64, pw: usize, ph: usize) -> (Rgba, Vec<f32>) {
    let (w, h) = dims(img);
    let (ow, oh) = (w + 2 * pw, h + 2 * ph);
    let mut plane = vec![0f32; ow * oh];
    for y in 0..h {
        for x in 0..w {
            plane[(y + ph) * ow + x + pw] = f32::from(img.pixels[(y * w + x) * 4 + 3]) / 255.0;
        }
    }
    let mut out = new_img(ow, oh);
    let [r, g, b] = [(color >> 16) & 255, (color >> 8) & 255, color & 255];
    for p in out.pixels.chunks_exact_mut(4) {
        p[0] = u8::try_from(r).unwrap_or(0);
        p[1] = u8::try_from(g).unwrap_or(0);
        p[2] = u8::try_from(b).unwrap_or(0);
    }
    let _ = a;
    (out, plane)
}

fn put_alpha(out: &mut Rgba, plane: &[f32], a: f64) {
    for (p, v) in out.pixels.chunks_exact_mut(4).zip(plane) {
        p[3] = to_u8(f64::from(*v) * a * 255.0);
    }
}

fn luma(p: &[u8]) -> f64 {
    0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])
}

/// Sobel gradient magnitude of the luminance (0..=1 roughly).
fn edges(img: &Rgba) -> Vec<f32> {
    let (w, h) = dims(img);
    let l: Vec<f32> = img
        .pixels
        .chunks_exact(4)
        .map(|p| (luma(p) / 255.0) as f32)
        .collect();
    let at = |x: isize, y: isize| {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, h as isize - 1) as usize;
        l[y * w + x]
    };
    let mut out = vec![0f32; w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1);
            let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1);
            out[y as usize * w + x as usize] = (gx * gx + gy * gy).sqrt() / 4.0;
        }
    }
    out
}

/// A repeatable grain value in -1..=1 for a pixel.
fn grain(x: usize, y: usize) -> f64 {
    let mut v = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    v ^= v >> 31;
    v = v.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    v ^= v >> 29;
    ((v & 0xFFFF) as f64 / 32_767.5) - 1.0
}

fn posterize(img: &mut Rgba, levels: u32) {
    let n = f64::from(levels.max(2) - 1);
    for p in img.pixels.chunks_exact_mut(4) {
        for c in 0..3 {
            p[c] = to_u8((f64::from(p[c]) / 255.0 * n).round() / n * 255.0);
        }
    }
}

fn gray(img: &mut Rgba) {
    for p in img.pixels.chunks_exact_mut(4) {
        let l = to_u8(luma(p));
        p[0] = l;
        p[1] = l;
        p[2] = l;
    }
}

/// Artistic effect `name` with `amount` (the element's main attribute).
fn artistic(img: &mut Rgba, name: &str, amount: i32) {
    let (w, h) = dims(img);
    let side = w.min(h).max(1) as f64;
    let amt = f64::from(amount.max(0));
    match name {
        "artisticBlur" => {
            let r = (amt / 100.0 * side / 8.0).max(1.0);
            blur_rgba(img, r, r);
        }
        "artisticPencilGrayscale"
        | "artisticPencilSketch"
        | "artisticLineDrawing"
        | "artisticPhotocopy" => {
            let e = edges(img);
            let k = 2.0 + amt / 25.0;
            for (p, v) in img.pixels.chunks_exact_mut(4).zip(&e) {
                let ink = (f64::from(*v) * k).min(1.0);
                let base = if name == "artisticPhotocopy" {
                    if luma(p) < 96.0 {
                        0.0
                    } else {
                        255.0
                    }
                } else {
                    255.0 - (255.0 - luma(p)) * 0.15
                };
                let l = to_u8(base * (1.0 - ink));
                p[0] = l;
                p[1] = l;
                p[2] = l;
            }
        }
        "artisticChalkSketch" | "artisticGlowEdges" => {
            let e = edges(img);
            let k = 3.0 + amt / 5.0;
            for (p, v) in img.pixels.chunks_exact_mut(4).zip(&e) {
                let ink = (f64::from(*v) * k).min(1.0);
                for c in 0..3 {
                    let lit = if name == "artisticGlowEdges" {
                        f64::from(p[c])
                    } else {
                        235.0
                    };
                    p[c] = to_u8(30.0 + (lit - 30.0) * ink);
                }
            }
        }
        "artisticMarker"
        | "artisticPaintBrush"
        | "artisticPaintStrokes"
        | "artisticWatercolorSponge"
        | "artisticPastelsSmooth" => {
            let r = (side / 160.0 * (1.0 + amt / 20.0)).clamp(1.0, side / 6.0);
            blur_rgba(img, r, r);
            posterize(img, 6);
        }
        "artisticCutout" => posterize(img, u32::try_from(amount.clamp(2, 6)).unwrap_or(4) + 1),
        "artisticGlowDiffused" => {
            let mut soft = dup(img);
            let r = side / 40.0;
            blur_rgba(&mut soft, r, r);
            for (p, s) in img
                .pixels
                .chunks_exact_mut(4)
                .zip(soft.pixels.chunks_exact(4))
            {
                for c in 0..3 {
                    p[c] = to_u8(
                        255.0 - (255.0 - f64::from(p[c])) * (255.0 - f64::from(s[c])) / 255.0 * 0.8,
                    );
                }
            }
        }
        "artisticMosaicBubbles" | "artisticGlass" | "artisticCement" => {
            let cell = (side / (20.0 + 60.0 * (1.0 - amt.min(100.0) / 100.0))).max(2.0) as usize;
            let src = dup(img);
            for y in 0..h {
                for x in 0..w {
                    let (cx, cy) = ((x / cell) * cell + cell / 2, (y / cell) * cell + cell / 2);
                    let (cx, cy) = (cx.min(w - 1), cy.min(h - 1));
                    let o = (y * w + x) * 4;
                    let s = (cy * w + cx) * 4;
                    let fx = (x % cell) as f64 / cell as f64 - 0.5;
                    let fy = (y % cell) as f64 / cell as f64 - 0.5;
                    let shade = 1.0 - (fx * fx + fy * fy) * 0.6;
                    for c in 0..3 {
                        img.pixels[o + c] = to_u8(f64::from(src.pixels[s + c]) * shade);
                    }
                }
            }
        }
        "artisticFilmGrain" | "artisticTexturizer" | "artisticPlasticWrap" => {
            let k = 10.0 + amt / 2.0;
            for y in 0..h {
                for x in 0..w {
                    let o = (y * w + x) * 4;
                    let g = grain(x, y) * k;
                    for c in 0..3 {
                        img.pixels[o + c] = to_u8(f64::from(img.pixels[o + c]) + g);
                    }
                }
            }
        }
        "artisticLightScreen" | "artisticCrisscrossEtching" => {
            let step = (side / (30.0 + amt * 4.0)).max(3.0) as usize;
            gray(img);
            for y in 0..h {
                for x in 0..w {
                    let line = if name == "artisticLightScreen" {
                        y % step == 0
                    } else {
                        (x + y) % step == 0 || (x + h - y) % step == 0
                    };
                    if line {
                        let o = (y * w + x) * 4;
                        for c in 0..3 {
                            img.pixels[o + c] = to_u8(f64::from(img.pixels[o + c]) * 0.55);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Soft edges: the alpha fades to nothing over the radius inside the outline.
fn soft(img: &mut Rgba, rx: f64, ry: f64) {
    let (w, h) = dims(img);
    let (rx, ry) = (rx * w as f64, ry * h as f64);
    // Pad so that the canvas edge counts as outside.
    let (pw, ph) = (px(rx) + 1, px(ry) + 1);
    let (ow, oh) = (w + 2 * pw, h + 2 * ph);
    let mut plane = vec![0f32; ow * oh];
    for y in 0..h {
        for x in 0..w {
            plane[(y + ph) * ow + x + pw] = f32::from(img.pixels[(y * w + x) * 4 + 3]) / 255.0;
        }
    }
    blur_plane(&mut plane, ow, oh, rx * 1.4, ry * 1.4);
    for y in 0..h {
        for x in 0..w {
            let m = f64::from(plane[(y + ph) * ow + x + pw]);
            let t = ((m - 0.5) / 0.5).clamp(0.0, 1.0);
            let o = (y * w + x) * 4 + 3;
            img.pixels[o] = to_u8(f64::from(img.pixels[o]) * t * t * (3.0 - 2.0 * t));
        }
    }
}

/// Bevel: edges lit from the top left, shaded at the bottom right.
fn bevel(img: &mut Rgba, bx: f64, by: f64) {
    let (w, h) = dims(img);
    let (rx, ry) = ((bx * w as f64).max(1.0), (by * h as f64).max(1.0));
    let (pw, ph) = (px(rx) + 1, px(ry) + 1);
    let (ow, oh) = (w + 2 * pw, h + 2 * ph);
    let mut plane = vec![0f32; ow * oh];
    for y in 0..h {
        for x in 0..w {
            plane[(y + ph) * ow + x + pw] = f32::from(img.pixels[(y * w + x) * 4 + 3]) / 255.0;
        }
    }
    blur_plane(&mut plane, ow, oh, rx * 2.0, ry * 2.0);
    let at = |x: usize, y: usize| f64::from(plane[y * ow + x]);
    for y in 0..h {
        for x in 0..w {
            let (cx, cy) = (x + pw, y + ph);
            let gx = (at(cx + 1, cy) - at(cx - 1, cy)) * rx;
            let gy = (at(cx, cy + 1) - at(cx, cy - 1)) * ry;
            // Light from the top left: an edge rising to the right / downwards faces it.
            let shade = ((gx + gy) * 0.9).clamp(-1.0, 1.0);
            let o = (y * w + x) * 4;
            for c in 0..3 {
                let v = f64::from(img.pixels[o + c]);
                img.pixels[o + c] = to_u8(if shade > 0.0 {
                    v + (255.0 - v) * shade * 0.7
                } else {
                    v * (1.0 + shade * 0.6)
                });
            }
        }
    }
}

/// Inner shadow: the colour along the inside of the outline away from the light.
fn inner_shadow(img: &mut Rgba, color: u32, a: f64, bx: f64, by: f64, dx: f64, dy: f64) {
    let (w, h) = dims(img);
    let (rx, ry) = (bx * w as f64, by * h as f64);
    let (ox, oy) = (dx * w as f64, dy * h as f64);
    let (pw, ph) = (px(rx + ox.abs()) + 2, px(ry + oy.abs()) + 2);
    let (ow, oh) = (w + 2 * pw, h + 2 * ph);
    // Outside of the picture, shifted by the offset, blurred.
    let mut plane = vec![1f32; ow * oh];
    for y in 0..h {
        for x in 0..w {
            let tx = (x as f64 + pw as f64 + ox).round();
            let ty = (y as f64 + ph as f64 + oy).round();
            if tx >= 0.0 && ty >= 0.0 && (tx as usize) < ow && (ty as usize) < oh {
                plane[ty as usize * ow + tx as usize] =
                    1.0 - f32::from(img.pixels[(y * w + x) * 4 + 3]) / 255.0;
            }
        }
    }
    blur_plane(&mut plane, ow, oh, rx, ry);
    let rgb = [(color >> 16) & 255, (color >> 8) & 255, color & 255].map(f64::from);
    for y in 0..h {
        for x in 0..w {
            let s = f64::from(plane[(y + ph) * ow + x + pw]) * a;
            let o = (y * w + x) * 4;
            for c in 0..3 {
                let v = f64::from(img.pixels[o + c]);
                img.pixels[o + c] = to_u8(v + (rgb[c] - v) * s);
            }
        }
    }
}

/// Rotation angles (x tilt, y turn, z roll in degrees) and perspective strength of a
/// §20.1.10.47 camera preset — BlueOffice's own values for the families of names.
pub fn camera_angles(name: &str) -> (f64, f64, f64, f64) {
    let persp = if name.starts_with("perspective") {
        0.35
    } else {
        0.0
    };
    let n = name.to_ascii_lowercase();
    let mut rx = 0.0;
    let mut ry = 0.0;
    let mut rz = 0.0;
    if n.starts_with("isometric") {
        rx = 30.0;
        ry = if n.contains("left") { 40.0 } else { -40.0 };
        if n.contains("offaxis1") {
            rx = 15.0;
        } else if n.contains("offaxis2") {
            rx = -15.0;
        } else if n.contains("topup") || n.contains("topdown") {
            rx = 45.0;
            ry = 0.0;
        } else if n.contains("bottom") {
            rx = -45.0;
            ry = 0.0;
        }
        if n.contains("down") && !n.contains("top") && !n.contains("bottom") {
            rz = if ry > 0.0 { -8.0 } else { 8.0 };
        }
    } else if n.starts_with("oblique") {
        rx = if n.contains("top") {
            25.0
        } else if n.contains("bottom") {
            -25.0
        } else {
            0.0
        };
        ry = if n.contains("left") {
            25.0
        } else if n.contains("right") {
            -25.0
        } else {
            0.0
        };
    } else {
        if n.contains("left") {
            ry = 20.0;
        }
        if n.contains("right") {
            ry = -20.0;
        }
        if n.contains("above") {
            rx = 25.0;
        }
        if n.contains("below") {
            rx = -25.0;
        }
        if n.contains("relaxed") {
            rx = if n.contains("moderately") { 30.0 } else { 50.0 };
        }
        if n.contains("contrasting") {
            rx = 10.0;
            ry *= 1.5;
        }
        if n.contains("heroic") {
            rx = -15.0;
            ry *= if n.contains("extreme") { 1.8 } else { 1.3 };
        }
    }
    (rx, ry, rz, persp)
}

/// Solve the homography taking the four `from` points to the four `to` points.
fn homography(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Option<[f64; 8]> {
    let mut m = [[0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = from[i];
        let (u, v) = to[i];
        m[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        m[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for c in 0..8 {
        let p = (c..8).max_by(|a, b| m[*a][c].abs().total_cmp(&m[*b][c].abs()))?;
        if m[p][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, p);
        for r in 0..8 {
            if r != c {
                let k = m[r][c] / m[c][c];
                for k2 in c..9 {
                    m[r][k2] -= k * m[c][k2];
                }
            }
        }
    }
    let mut out = [0f64; 8];
    for (i, o) in out.iter_mut().enumerate() {
        *o = m[i][8] / m[i][i];
    }
    Some(out)
}

/// 3-D rotation: the picture as a plane turned by the camera preset, projected and fitted
/// into the same box.
fn camera(img: &mut Rgba, name: &str) {
    let (w, h) = dims(img);
    let (rx, ry, rz, persp) = camera_angles(name);
    let (sx, cx) = rx.to_radians().sin_cos();
    let (sy, cy) = ry.to_radians().sin_cos();
    let (sz, cz) = rz.to_radians().sin_cos();
    let (hw, hh) = (w as f64 / 2.0, h as f64 / 2.0);
    let dist = (w.max(h) as f64) * if persp > 0.0 { 1.0 / persp } else { 1e9 };
    let project = |x: f64, y: f64| {
        let (x, y) = (x - hw, y - hh);
        // z roll, then y turn, then x tilt.
        let (x, y) = (x * cz - y * sz, x * sz + y * cz);
        let (x, z) = (x * cy, -x * sy);
        let (y, z) = (y * cx - z * sx, y * sx + z * cx);
        let k = dist / (dist + z);
        (x * k, y * k)
    };
    let corners = [
        (0.0, 0.0),
        (w as f64, 0.0),
        (w as f64, h as f64),
        (0.0, h as f64),
    ];
    let pts = corners.map(|(x, y)| project(x, y));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let k = (w as f64 / (x1 - x0).max(1e-6)).min(h as f64 / (y1 - y0).max(1e-6));
    let (ox, oy) = (
        (w as f64 - (x1 - x0) * k) / 2.0,
        (h as f64 - (y1 - y0) * k) / 2.0,
    );
    let quad = pts.map(|(x, y)| ((x - x0) * k + ox, (y - y0) * k + oy));
    // Inverse map: output quad → source rectangle.
    let Some(hm) = homography(quad, corners) else {
        return;
    };
    let src = dup(img);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let d = hm[6] * fx + hm[7] * fy + 1.0;
            let u = (hm[0] * fx + hm[1] * fy + hm[2]) / d;
            let v = (hm[3] * fx + hm[4] * fy + hm[5]) / d;
            let o = (y * w + x) * 4;
            if u < 0.0 || v < 0.0 || u >= w as f64 || v >= h as f64 {
                img.pixels[o..o + 4].copy_from_slice(&[0, 0, 0, 0]);
            } else {
                let s = ((v as usize) * w + u as usize) * 4;
                img.pixels[o..o + 4].copy_from_slice(&src.pixels[s..s + 4]);
            }
        }
    }
}

/// Apply the style operations of derived image `id` to `img` (the picture after its
/// colour adjustments, crop shape and mirroring); an effect image (shadow, glow,
/// reflection) replaces it.
#[must_use]
pub fn apply_style(img: Rgba, id: &str) -> Rgba {
    let ops = style_ops(id);
    if ops.is_empty() {
        return img;
    }
    let mut img = img;
    for op in ops {
        match op {
            StyleOp::Crop(c) => {
                if let Some(c2) = crate::picfx::cut(&img, c) {
                    img = c2;
                }
            }
            StyleOp::Artistic(name, amount) => artistic(&mut img, &name, amount),
            StyleOp::Soft(rx, ry) => soft(&mut img, rx, ry),
            StyleOp::Bevel(bx, by) => bevel(&mut img, bx, by),
            StyleOp::Camera(name) => camera(&mut img, &name),
            StyleOp::InnerShadow(color, a, bx, by, dx, dy) => {
                inner_shadow(&mut img, color, a, bx, by, dx, dy);
            }
            StyleOp::Silhouette(color, a, bx, by, pxf, pyf) => {
                let small = shrink_for_effect(&img);
                let (w, h) = dims(&small);
                let (pw, ph) = (px(pxf * w as f64), px(pyf * h as f64));
                let (mut out, mut plane) = silhouette(&small, color, a, pw, ph);
                let (ow, oh) = dims(&out);
                blur_plane(&mut plane, ow, oh, bx * w as f64, by * h as f64);
                put_alpha(&mut out, &plane, a);
                img = out;
            }
            StyleOp::Glow(color, a, rxf, ryf) => {
                let small = shrink_for_effect(&img);
                let (w, h) = dims(&small);
                let (rx, ry) = (rxf * w as f64, ryf * h as f64);
                let (mut out, mut plane) = silhouette(&small, color, a, px(rx), px(ry));
                let (ow, oh) = dims(&out);
                dilate(&mut plane, ow, oh, px(rx / 2.0), px(ry / 2.0));
                blur_plane(&mut plane, ow, oh, rx, ry);
                put_alpha(&mut out, &plane, a);
                img = out;
            }
            StyleOp::Reflection(st, end, pos, bx, by) => {
                crate::picfx::mirror(&mut img, (false, true));
                let (w, h) = dims(&img);
                if bx > 0.0 || by > 0.0 {
                    blur_rgba(&mut img, bx * w as f64, by * h as f64);
                }
                let pos = pos.max(1e-3);
                for y in 0..h {
                    let t = y as f64 / h.max(1) as f64;
                    let k = if t > pos {
                        0.0
                    } else {
                        st + (end - st) * t / pos
                    };
                    for x in 0..w {
                        let o = (y * w + x) * 4 + 3;
                        img.pixels[o] = to_u8(f64::from(img.pixels[o]) * k);
                    }
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(n: usize) -> Rgba {
        let mut i = new_img(n, n);
        for p in i.pixels.chunks_exact_mut(4) {
            p.copy_from_slice(&[200, 100, 50, 255]);
        }
        i
    }

    #[test]
    fn shadow_is_padded_and_blurred_in_its_colour() {
        let out = apply_style(
            square(40),
            "m#fx:shadow=FF0000,50000,10000,10000,20000,20000",
        );
        assert_eq!((out.width, out.height), (56, 56));
        let at = |x: usize, y: usize| &out.pixels[(y * 56 + x) * 4..(y * 56 + x) * 4 + 4];
        // Centre: the full opacity times alpha; corner: nothing; colour red.
        assert_eq!(at(28, 28)[..3], [255, 0, 0]);
        assert!(
            (i32::from(at(28, 28)[3]) - 127).abs() <= 2,
            "{:?}",
            at(28, 28)
        );
        assert_eq!(at(0, 0)[3], 0);
        // The edge is soft: half way.
        assert!(at(8, 28)[3] > 20 && at(8, 28)[3] < 110, "{:?}", at(8, 28));
    }

    #[test]
    fn soft_edges_fade_the_border_only() {
        let out = apply_style(square(60), "m#fx:soft=10000,10000");
        let a = |x: usize, y: usize| out.pixels[(y * 60 + x) * 4 + 3];
        assert_eq!(a(30, 30), 255);
        assert!(a(0, 30) < 40, "{}", a(0, 30));
        assert!(a(3, 30) < a(8, 30));
    }

    #[test]
    fn reflection_is_upside_down_and_fades() {
        let mut img = square(10);
        // Top row blue.
        for x in 0..10 {
            img.pixels[x * 4..x * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        }
        let out = apply_style(img, "m#fx:refl=50000,0,50000,0,0");
        // The blue row is now at the bottom, past the fade end: transparent.
        assert_eq!(out.pixels[(9 * 10) * 4 + 3], 0);
        assert_eq!(out.pixels[2], 50);
        assert_eq!(out.pixels[3], 128);
    }

    #[test]
    fn camera_keeps_the_box_and_clears_the_corners() {
        let out = apply_style(square(50), "m#fx:cam=perspectiveLeft");
        assert_eq!((out.width, out.height), (50, 50));
        assert!(out.pixels.chunks_exact(4).any(|p| p[3] == 0));
        assert_eq!(out.pixels[(25 * 50 + 25) * 4 + 3], 255);
    }

    #[test]
    fn artistic_effects_change_pixels() {
        for (name, _, amount) in bo_model::picture_style::ARTISTIC {
            let mut img = square(24);
            for (i, p) in img.pixels.chunks_exact_mut(4).enumerate() {
                if i % 24 > 12 {
                    p.copy_from_slice(&[10, 20, 30, 255]);
                }
            }
            let before = img.pixels.clone();
            let out = apply_style(img, &format!("m#fx:art={name}@{amount}"));
            assert_ne!(out.pixels, before, "{name}");
        }
    }

    #[test]
    fn glow_and_bevel_and_inner_shadow_run() {
        let g = apply_style(square(20), "m#fx:glow=00FF00,60000,10000,10000");
        assert_eq!((g.width, g.height), (24, 24));
        assert!(g.pixels[3] < g.pixels[(12 * 24 + 12) * 4 + 3]);
        let b = apply_style(square(30), "m#fx:bevel=10000,10000");
        assert!(b.pixels[(2 * 30 + 2) * 4] > 200, "lit top left");
        let i = apply_style(square(30), "m#fx:ish=000000,80000,10000,10000,5000,5000");
        assert!(i.pixels[(2 * 30 + 2) * 4] < 150, "shaded top left");
    }
}

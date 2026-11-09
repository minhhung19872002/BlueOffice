//! Picture adjustments applied to decoded pixels (CORE-PIC-001): the colour
//! transforms of a picture's `a:blip` (ECMA-376 Part 1, §20.1.8) and the
//! [MS-ODRAWXML] picture effects, plus "Compress pictures" (downsampling, dropping
//! cropped areas, re-encoding as PNG or JPEG).
//!
//! The specification names each transform but leaves the exact arithmetic open;
//! the formulas here are BlueOffice's own choices, stated per function. Colour
//! values are straight 8-bit sRGB taken as 0…1; luminance is the Rec. ITU-R BT.601
//! luma `0.299 R + 0.587 G + 0.114 B`.

use crate::codecs::{self, Rgba};
use bo_model::picture::{clip_of_id, split_fx_id, ClipShape, PictureFx, PCT};
use bo_render::ImageSource;
use std::borrow::Cow;

/// Per-channel tolerance of §20.1.8.16 clrChange (JPEG noise around the colour).
pub const COLOR_CHANGE_TOLERANCE: i32 = 6;

fn luma(p: &[u8]) -> f64 {
    (0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2])) / 255.0
}

fn to_u8(v: f64) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

fn rgb(c: u32) -> [f64; 3] {
    [
        f64::from((c >> 16) & 0xFF) / 255.0,
        f64::from((c >> 8) & 0xFF) / 255.0,
        f64::from(c & 0xFF) / 255.0,
    ]
}

/// §20.1.8.42 lum: contrast scales each channel about mid-grey, then brightness is
/// added: `v' = (v − ½)·k + ½ + bright`, with `k = 1 + contrast` for a decrease and
/// `k = 1 / (1 − contrast)` for an increase (100 % gives a hard threshold).
pub fn lum(v: u8, bright: i32, contrast: i32) -> u8 {
    let c = f64::from(contrast) / PCT;
    let k = if c <= 0.0 {
        1.0 + c
    } else {
        1.0 / (1.0 - c).max(1e-3)
    };
    let x = f64::from(v) / 255.0;
    to_u8((x - 0.5) * k + 0.5 + f64::from(bright) / PCT)
}

/// 3×3 box average of channel `c` around (x, y), edges clamped.
fn box3(px: &[u8], w: usize, h: usize, x: usize, y: usize, c: usize) -> f64 {
    let mut s = 0u32;
    for dy in [-1i64, 0, 1] {
        for dx in [-1i64, 0, 1] {
            let xx = (x as i64 + dx).clamp(0, w as i64 - 1) as usize;
            let yy = (y as i64 + dy).clamp(0, h as i64 - 1) as usize;
            s += u32::from(px[(yy * w + xx) * 4 + c]);
        }
    }
    f64::from(s) / 9.0
}

/// [MS-ODRAWXML] sharpenSoften: `amount` > 0 adds `amount × (v − blur)` (unsharp
/// mask), `amount` < 0 moves `|amount|` of the way to the 3×3 box blur.
fn sharpen(img: &mut Rgba, amount: i32) {
    let (w, h) = (img.width as usize, img.height as usize);
    let s = f64::from(amount) / PCT;
    let src = img.pixels.clone();
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let i = (y * w + x) * 4 + c;
                let v = f64::from(src[i]);
                let b = box3(&src, w, h, x, y, c);
                let out = if s >= 0.0 {
                    v + s * (v - b)
                } else {
                    v + -s * (b - v)
                };
                img.pixels[i] = out.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Apply `fx` to straight RGBA pixels, in this order: colour change, saturation,
/// sharpen/soften, brightness/contrast, greyscale, black-and-white, duotone, opacity.
pub fn apply(img: &mut Rgba, fx: &PictureFx) {
    let fx = fx.clamped();
    if let Some(cc) = fx.color_change {
        // §20.1.8.16 clrChange: pixels of `from` become `to` (with its opacity).
        let from = [
            (cc.from >> 16) & 0xFF,
            (cc.from >> 8) & 0xFF,
            cc.from & 0xFF,
        ];
        let to = [(cc.to >> 16) as u8, (cc.to >> 8) as u8, cc.to as u8];
        let a = f64::from(cc.alpha) / PCT;
        for p in img.pixels.chunks_exact_mut(4) {
            let near =
                (0..3).all(|c| (i32::from(p[c]) - from[c] as i32).abs() <= COLOR_CHANGE_TOLERANCE);
            if near {
                p[..3].copy_from_slice(&to);
                p[3] = (f64::from(p[3]) * a).round() as u8;
            }
        }
    }
    if let Some(sat) = fx.saturation.filter(|s| *s != 100_000) {
        // Saturation: each channel moves away from (or towards) the pixel's grey.
        let k = f64::from(sat) / PCT;
        for p in img.pixels.chunks_exact_mut(4) {
            let g = luma(p);
            for c in &mut p[..3] {
                *c = to_u8(g + (f64::from(*c) / 255.0 - g) * k);
            }
        }
    }
    if fx.sharpen != 0 && img.width > 0 && img.height > 0 {
        sharpen(img, fx.sharpen);
    }
    if fx.bright != 0 || fx.contrast != 0 {
        let table: Vec<u8> = (0..=255u8)
            .map(|v| lum(v, fx.bright, fx.contrast))
            .collect();
        for p in img.pixels.chunks_exact_mut(4) {
            for c in &mut p[..3] {
                *c = table[usize::from(*c)];
            }
        }
    }
    if fx.grayscale {
        // §20.1.8.34 grayscl.
        for p in img.pixels.chunks_exact_mut(4) {
            let g = to_u8(luma(p));
            p[..3].fill(g);
        }
    }
    if let Some(t) = fx.bilevel {
        // §20.1.8.11 biLevel: luminance below the threshold → black, else white.
        let t = f64::from(t) / PCT;
        for p in img.pixels.chunks_exact_mut(4) {
            let v = if luma(p) < t { 0 } else { 255 };
            p[..3].fill(v);
        }
    }
    if let Some([a, b]) = fx.duotone {
        // §20.1.8.23 duotone: linear interpolation of the two colours by luminance.
        let (a, b) = (rgb(a), rgb(b));
        for p in img.pixels.chunks_exact_mut(4) {
            let l = luma(p);
            for c in 0..3 {
                p[c] = to_u8(a[c] + (b[c] - a[c]) * l);
            }
        }
    }
    if let Some(amt) = fx.alpha.filter(|a| *a < 100_000) {
        // §20.1.8.4 alphaModFix: opacity multiplied by `amt`.
        let k = f64::from(amt) / PCT;
        for p in img.pixels.chunks_exact_mut(4) {
            p[3] = (f64::from(p[3]) * k).round() as u8;
        }
    }
}

/// Decode PNG, JPEG, GIF, BMP or TIFF bytes to straight RGBA.
pub fn decode_rgba(bytes: &[u8]) -> Option<Rgba> {
    if bytes.starts_with(b"\x89PNG") {
        let pm = tiny_skia::Pixmap::decode_png(bytes).ok()?;
        let mut pixels = pm.data().to_vec();
        for p in pixels.chunks_exact_mut(4) {
            let a = u16::from(p[3]);
            if a > 0 && a < 255 {
                for c in &mut p[..3] {
                    *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        return Some(Rgba {
            width: pm.width(),
            height: pm.height(),
            pixels,
        });
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
        let pixels = dec.decode().ok()?;
        let (w, h) = dec.dimensions()?;
        return Some(Rgba {
            width: u32::try_from(w).ok()?,
            height: u32::try_from(h).ok()?,
            pixels,
        });
    }
    codecs::decode(bytes)
}

/// Mask the visible (cropped) area of `img` with a preset shape (§20.1.9.18
/// prstGeom): pixels outside the shape become transparent. The shape spans the
/// part left by the crop; what the crop removes is not drawn anyway.
pub fn mask_shape(img: &mut Rgba, clip: &ClipShape) {
    let (w, h) = (f64::from(img.width), f64::from(img.height));
    let [l, t, r, b] = clip.crop.map(|v| f64::from(v) / PCT);
    let (x0, y0) = (l * w, t * h);
    let (rw, rh) = ((1.0 - l - r) * w, (1.0 - t - b) * h);
    if rw <= 0.0 || rh <= 0.0 {
        return;
    }
    let Some(Ok(geom)) = bo_drawingml::geom::eval_preset(&clip.name, &[], rw, rh) else {
        return;
    };
    let mut pb = tiny_skia::PathBuilder::new();
    for p in geom
        .paths
        .iter()
        .filter(|p| p.fill != bo_drawingml::FillMode::None)
    {
        for s in &p.segs {
            let f = |v: f64| v as f32;
            match *s {
                bo_drawingml::Seg::Move(x, y) => pb.move_to(f(x0 + x), f(y0 + y)),
                bo_drawingml::Seg::Line(x, y) => pb.line_to(f(x0 + x), f(y0 + y)),
                bo_drawingml::Seg::Quad(a, c, x, y) => {
                    pb.quad_to(f(x0 + a), f(y0 + c), f(x0 + x), f(y0 + y));
                }
                bo_drawingml::Seg::Cubic(a, c, d, e, x, y) => pb.cubic_to(
                    f(x0 + a),
                    f(y0 + c),
                    f(x0 + d),
                    f(y0 + e),
                    f(x0 + x),
                    f(y0 + y),
                ),
                bo_drawingml::Seg::Close => pb.close(),
            }
        }
    }
    let Some(path) = pb.finish() else { return };
    let Some(mut mask) = tiny_skia::Mask::new(img.width, img.height) else {
        return;
    };
    mask.fill_path(
        &path,
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::identity(),
    );
    for (p, m) in img.pixels.chunks_exact_mut(4).zip(mask.data()) {
        p[3] = ((u16::from(p[3]) * u16::from(*m) + 127) / 255) as u8;
    }
}

/// PNG of `bytes` with `fx` applied.
pub fn derive_png(bytes: &[u8], fx: &PictureFx) -> Option<Vec<u8>> {
    let mut img = decode_rgba(bytes)?;
    apply(&mut img, fx);
    crate::rgba_to_png(img)
}

/// PNG of a derived image id's picture (`bytes` = the media picture).
pub fn derive_id_png(bytes: &[u8], id: &str) -> Option<Vec<u8>> {
    let (_, fx) = split_fx_id(id)?;
    let mut img = decode_rgba(bytes)?;
    apply(&mut img, &fx);
    if let Some(c) = clip_of_id(id) {
        mask_shape(&mut img, &c);
    }
    crate::rgba_to_png(img)
}

/// Encoded bytes of image `id`: the source's bytes, or for a derived id
/// (`media#fx:…`, [`PictureFx::derived_id`]) the media picture with the
/// adjustments applied, as PNG.
pub fn resolve<'a>(src: &'a dyn ImageSource, id: &str) -> Option<Cow<'a, [u8]>> {
    if let Some(b) = src.image(id) {
        return Some(Cow::Borrowed(b));
    }
    let (base, _) = split_fx_id(id)?;
    let bytes = src.image(base)?;
    derive_id_png(bytes, id).map(Cow::Owned)
}

/// Decoded pixels of image `id` (derived ids included).
pub fn resolve_rgba(src: &dyn ImageSource, id: &str) -> Option<Rgba> {
    if let Some(b) = src.image(id) {
        return decode_rgba(b);
    }
    let (base, fx) = split_fx_id(id)?;
    let mut img = decode_rgba(src.image(base)?)?;
    apply(&mut img, &fx);
    if let Some(c) = clip_of_id(id) {
        mask_shape(&mut img, &c);
    }
    Some(img)
}

/// Area-average resampling to `w` × `h` (downsampling; also works upwards).
pub fn resample(img: &Rgba, w: u32, h: u32) -> Rgba {
    let (sw, sh) = (img.width as usize, img.height as usize);
    let (dw, dh) = (w.max(1) as usize, h.max(1) as usize);
    let mut out = vec![0u8; dw * dh * 4];
    if sw == 0 || sh == 0 {
        return Rgba {
            width: dw as u32,
            height: dh as u32,
            pixels: out,
        };
    }
    let fx = sw as f64 / dw as f64;
    let fy = sh as f64 / dh as f64;
    for y in 0..dh {
        let y0 = y as f64 * fy;
        let y1 = (y0 + fy).min(sh as f64);
        for x in 0..dw {
            let x0 = x as f64 * fx;
            let x1 = (x0 + fx).min(sw as f64);
            let mut acc = [0f64; 4];
            let mut wsum = 0.0;
            let mut sy = y0.floor() as usize;
            while (sy as f64) < y1 && sy < sh {
                let wy = (y1.min(sy as f64 + 1.0) - y0.max(sy as f64)).max(0.0);
                let mut sx = x0.floor() as usize;
                while (sx as f64) < x1 && sx < sw {
                    let wx = (x1.min(sx as f64 + 1.0) - x0.max(sx as f64)).max(0.0);
                    let wgt = wx * wy;
                    let i = (sy * sw + sx) * 4;
                    let a = f64::from(img.pixels[i + 3]) * wgt;
                    // Colours weighted by alpha so transparent pixels do not bleed.
                    for (sum, v) in acc.iter_mut().zip(&img.pixels[i..i + 3]) {
                        *sum += f64::from(*v) * a;
                    }
                    acc[3] += a;
                    wsum += wgt;
                    sx += 1;
                }
                sy += 1;
            }
            let o = (y * dw + x) * 4;
            if acc[3] > 0.0 {
                for c in 0..3 {
                    out[o + c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
            }
            out[o + 3] = if wsum > 0.0 {
                (acc[3] / wsum).round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
    }
    Rgba {
        width: dw as u32,
        height: dh as u32,
        pixels: out,
    }
}

/// Pixels of `img` inside the §20.1.8.55 srcRect `crop` (l, t, r, b in 1/1000 %).
pub fn cut(img: &Rgba, crop: [i32; 4]) -> Option<Rgba> {
    let (w, h) = (f64::from(img.width), f64::from(img.height));
    let [l, t, r, b] = crop.map(|v| f64::from(v.max(0)) / PCT);
    let x0 = (l * w).round() as u32;
    let y0 = (t * h).round() as u32;
    let x1 = ((1.0 - r) * w).round().min(w) as u32;
    let y1 = ((1.0 - b) * h).round().min(h) as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (nw, nh) = (x1 - x0, y1 - y0);
    let mut pixels = Vec::with_capacity((nw * nh * 4) as usize);
    for y in y0..y1 {
        let s = ((y * img.width + x0) * 4) as usize;
        pixels.extend_from_slice(img.pixels.get(s..s + (nw * 4) as usize)?);
    }
    Some(Rgba {
        width: nw,
        height: nh,
        pixels,
    })
}

/// Thumbnail PNG (longest side `max_side` px) of `bytes` with `fx` applied
/// (previews of the picture tools galleries).
pub fn preview_png(bytes: &[u8], fx: &PictureFx, max_side: u32) -> Option<Vec<u8>> {
    let img = decode_rgba(bytes)?;
    let k = (f64::from(max_side) / f64::from(img.width.max(img.height))).min(1.0);
    let mut small = if k < 1.0 {
        resample(
            &img,
            (f64::from(img.width) * k).round().max(1.0) as u32,
            (f64::from(img.height) * k).round().max(1.0) as u32,
        )
    } else {
        img
    };
    apply(&mut small, fx);
    crate::rgba_to_png(small)
}

/// "Compress pictures" options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressOptions {
    /// Target resolution in pixels per inch of the displayed size (`None` keeps the pixels).
    pub ppi: Option<f64>,
    /// Remove the cropped-away parts of the picture.
    pub drop_cropped: bool,
    /// JPEG quality 1–100 for opaque pictures (`None`: JPEG pictures at 85, others PNG).
    pub jpeg_quality: Option<u8>,
}

/// Result of [`compress`].
#[derive(Debug, Clone, PartialEq)]
pub struct Compressed {
    /// New encoded picture.
    pub bytes: Vec<u8>,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Encoded as JPEG (else PNG).
    pub jpeg: bool,
    /// The crop was applied to the pixels (the picture's srcRect must be cleared).
    pub crop_applied: bool,
}

/// Default JPEG quality when a JPEG picture is re-encoded.
pub const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Compress a picture shown at `shown_w` × `shown_h` points through `crop`.
/// Returns `None` when nothing would get smaller (the picture is kept as it is).
pub fn compress(
    bytes: &[u8],
    crop: Option<[i32; 4]>,
    shown_w: f64,
    shown_h: f64,
    opts: &CompressOptions,
) -> Option<Compressed> {
    let mut img = decode_rgba(bytes)?;
    let crop = crop.filter(|c| c.iter().any(|v| *v != 0));
    let mut crop_applied = false;
    // Displayed size of the whole picture (the crop shows only part of it).
    let [l, t, r, b] = crop.unwrap_or([0; 4]).map(|v| f64::from(v) / PCT);
    let (mut full_w, mut full_h) = (
        shown_w / (1.0 - l - r).max(1e-3),
        shown_h / (1.0 - t - b).max(1e-3),
    );
    if opts.drop_cropped {
        if let Some(c) = crop {
            img = cut(&img, c)?;
            crop_applied = true;
            (full_w, full_h) = (shown_w, shown_h);
        }
    }
    if let Some(ppi) = opts.ppi.filter(|p| *p > 0.0) {
        let tw = (full_w / 72.0 * ppi).ceil().max(1.0);
        let th = (full_h / 72.0 * ppi).ceil().max(1.0);
        let k = (tw / f64::from(img.width)).min(th / f64::from(img.height));
        if k < 1.0 {
            let nw = (f64::from(img.width) * k).round().max(1.0) as u32;
            let nh = (f64::from(img.height) * k).round().max(1.0) as u32;
            img = resample(&img, nw, nh);
        }
    }
    let opaque = img.pixels.chunks_exact(4).all(|p| p[3] == 255);
    let was_jpeg = bytes.starts_with(&[0xFF, 0xD8]);
    let quality = opts
        .jpeg_quality
        .or(was_jpeg.then_some(DEFAULT_JPEG_QUALITY));
    let (out, jpeg) = match quality {
        Some(q) if opaque => (crate::jpeg::encode(&img, q)?, true),
        _ => (
            crate::rgba_to_png(Rgba {
                width: img.width,
                height: img.height,
                pixels: img.pixels.clone(),
            })?,
            false,
        ),
    };
    if out.len() >= bytes.len() && !crop_applied {
        return None;
    }
    Some(Compressed {
        width: img.width,
        height: img.height,
        bytes: out,
        jpeg,
        crop_applied,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::picture::ColorChange;

    fn one(rgb: [u8; 3]) -> Rgba {
        Rgba {
            width: 1,
            height: 1,
            pixels: vec![rgb[0], rgb[1], rgb[2], 255],
        }
    }

    fn fx_of(f: impl FnOnce(&mut PictureFx)) -> PictureFx {
        let mut fx = PictureFx::default();
        f(&mut fx);
        fx
    }

    #[test]
    fn brightness_and_contrast_by_hand() {
        // bright +20 %: 100/255 + 0.2 → ·255 = 100 + 51 = 151.
        assert_eq!(lum(100, 20_000, 0), 151);
        // bright −20 %: 100 − 51 = 49; 0 stays 0 (clamped), 255 + 51 clamps to 255.
        assert_eq!(lum(100, -20_000, 0), 49);
        assert_eq!(lum(0, -20_000, 0), 0);
        assert_eq!(lum(255, 20_000, 0), 255);
        // contrast +40 %: k = 1/0.6; (100 − 127.5)/0.6 + 127.5 = 81.67 → 82;
        // (200 − 127.5)/0.6 + 127.5 = 248.33 → 248.
        assert_eq!(lum(100, 0, 40_000), 82);
        assert_eq!(lum(200, 0, 40_000), 248);
        // contrast −40 %: k = 0.6; (100 − 127.5)·0.6 + 127.5 = 111; (200 − 127.5)·0.6 + 127.5 = 171.
        assert_eq!(lum(100, 0, -40_000), 111);
        assert_eq!(lum(200, 0, -40_000), 171);
        // Both: (100 − 127.5)·0.6 + 127.5 + 51 = 162.
        let mut img = one([100, 200, 0]);
        apply(
            &mut img,
            &fx_of(|f| {
                f.bright = 20_000;
                f.contrast = -40_000;
            }),
        );
        // 200 → 72.5·0.6 + 178.5 = 222; 0 → (−127.5)·0.6 + 178.5 = 102.
        assert_eq!(img.pixels, vec![162, 222, 102, 255]);
    }

    #[test]
    fn greyscale_duotone_bilevel_by_hand() {
        // BT.601: red 0.299·255 = 76.245 → 76; green 149.685 → 150; blue 29.07 → 29.
        let mut img = Rgba {
            width: 3,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        };
        apply(&mut img, &fx_of(|f| f.grayscale = true));
        assert_eq!(
            img.pixels,
            vec![76, 76, 76, 255, 150, 150, 150, 255, 29, 29, 29, 255]
        );
        // Duotone navy (000080) → yellow (FFFF00) on grey 128: L = 128/255.
        // R = 255·L = 128; G = 128; B = 128 + (0 − 128)·L = 63.75 → 64.
        let mut g = one([128, 128, 128]);
        apply(&mut g, &fx_of(|f| f.duotone = Some([0x00_0080, 0xFF_FF00])));
        assert_eq!(g.pixels, vec![128, 128, 64, 255]);
        // biLevel 50 %: L(red) = 0.299 < 0.5 → black; L(green) = 0.587 → white.
        let mut b = Rgba {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };
        apply(&mut b, &fx_of(|f| f.bilevel = Some(50_000)));
        assert_eq!(b.pixels, vec![0, 0, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn color_change_saturation_alpha() {
        let mut img = Rgba {
            width: 2,
            height: 1,
            pixels: vec![250, 252, 255, 255, 10, 20, 30, 255],
        };
        // White (within the tolerance) becomes transparent white; the other pixel stays.
        apply(
            &mut img,
            &fx_of(|f| {
                f.color_change = Some(ColorChange {
                    from: 0xFF_FFFF,
                    to: 0xFF_FFFF,
                    alpha: 0,
                });
            }),
        );
        assert_eq!(img.pixels, vec![255, 255, 255, 0, 10, 20, 30, 255]);
        // Saturation 0 = grey: L(255,0,0) = 76.245 → 76.
        let mut s = one([255, 0, 0]);
        apply(&mut s, &fx_of(|f| f.saturation = Some(0)));
        assert_eq!(s.pixels, vec![76, 76, 76, 255]);
        // alphaModFix 50 %: 255·0.5 = 127.5 → 128.
        let mut a = one([1, 2, 3]);
        apply(&mut a, &fx_of(|f| f.alpha = Some(50_000)));
        assert_eq!(a.pixels, vec![1, 2, 3, 128]);
    }

    #[test]
    fn sharpen_and_soften_by_hand() {
        // 3×1: 0, 90, 0. Box blur at the centre (edges clamped): columns 0,90,0 for
        // each of three rows → 90·3/9 = 30. Sharpen 50 %: 90 + 0.5·(90 − 30) = 120.
        // Soften 50 %: 90 + 0.5·(30 − 90) = 60.
        let row = |v: u8| vec![0, 0, 0, 255, v, v, v, 255, 0, 0, 0, 255];
        let mut img = Rgba {
            width: 3,
            height: 1,
            pixels: row(90),
        };
        apply(&mut img, &fx_of(|f| f.sharpen = 50_000));
        assert_eq!(img.pixels[4], 120);
        let mut img = Rgba {
            width: 3,
            height: 1,
            pixels: row(90),
        };
        apply(&mut img, &fx_of(|f| f.sharpen = -50_000));
        assert_eq!(img.pixels[4], 60);
    }

    #[test]
    fn resample_and_cut() {
        // 4×2 → 2×1 averages 2×2 blocks.
        let img = Rgba {
            width: 4,
            height: 2,
            pixels: [
                [0u8, 0, 0, 255],
                [100, 100, 100, 255],
                [200, 0, 0, 255],
                [200, 0, 0, 255],
                [0, 0, 0, 255],
                [100, 100, 100, 255],
                [0, 200, 0, 255],
                [0, 200, 0, 255],
            ]
            .concat(),
        };
        let r = resample(&img, 2, 1);
        assert_eq!(r.pixels, vec![50, 50, 50, 255, 100, 100, 0, 255]);
        // Crop 25 % left and right: columns 1..3.
        let c = cut(&img, [25_000, 0, 25_000, 0]).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(&c.pixels[..8], &[100, 100, 100, 255, 200, 0, 0, 255]);
    }

    #[test]
    fn compress_downsamples_and_drops_crop() {
        // 400×300 noisy opaque picture shown at 2 × 1.5 inches (144 × 108 pt).
        let (w, h) = (400u32, 300u32);
        let mut pixels = Vec::new();
        let mut seed = 7u32;
        for _ in 0..w * h {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let v = (seed >> 16) as u8;
            pixels.extend_from_slice(&[v, v / 2, 255 - v, 255]);
        }
        let png = crate::rgba_to_png(Rgba {
            width: w,
            height: h,
            pixels,
        })
        .unwrap();
        // 96 ppi of 2 × 1.5 in = 192 × 144 px; PNG kept (not a JPEG, no quality given).
        let r = compress(
            &png,
            None,
            144.0,
            108.0,
            &CompressOptions {
                ppi: Some(96.0),
                drop_cropped: false,
                jpeg_quality: None,
            },
        )
        .unwrap();
        assert_eq!((r.width, r.height), (192, 144));
        assert!(!r.jpeg && !r.crop_applied);
        assert!(r.bytes.len() < png.len());
        // Crop 25 % left/right: the visible half (200×300 px) shows at 144 × 108 pt;
        // dropping the crop then 96 ppi → 192 × 144 px, as JPEG at quality 70.
        let r = compress(
            &png,
            Some([25_000, 0, 25_000, 0]),
            144.0,
            108.0,
            &CompressOptions {
                ppi: Some(96.0),
                drop_cropped: true,
                jpeg_quality: Some(70),
            },
        )
        .unwrap();
        assert!(r.crop_applied && r.jpeg);
        // 200×300 scaled by min(192/200, 144/300) = 0.48 → 96 × 144.
        assert_eq!((r.width, r.height), (96, 144));
        assert!(r.bytes.len() * 4 < png.len());
    }

    #[test]
    fn derived_ids_resolve() {
        let png = crate::rgba_to_png(one([255, 0, 0])).unwrap();
        let mut src = std::collections::BTreeMap::new();
        src.insert("/m/a.png".to_owned(), png);
        let id = fx_of(|f| f.grayscale = true).derived_id("/m/a.png");
        let img = resolve_rgba(&src, &id).unwrap();
        assert_eq!(img.pixels, vec![76, 76, 76, 255]);
        assert!(matches!(resolve(&src, &id), Some(Cow::Owned(_))));
        assert!(matches!(resolve(&src, "/m/a.png"), Some(Cow::Borrowed(_))));
    }

    #[test]
    fn ellipse_mask_clears_corners() {
        let mut img = Rgba {
            width: 20,
            height: 10,
            pixels: vec![200; 20 * 10 * 4],
        };
        mask_shape(
            &mut img,
            &ClipShape {
                name: "ellipse".into(),
                crop: [0; 4],
            },
        );
        let a = |x: usize, y: usize| img.pixels[(y * 20 + x) * 4 + 3];
        // Corners fall outside the ellipse, the centre inside.
        assert_eq!(a(0, 0), 0);
        assert_eq!(a(19, 9), 0);
        assert_eq!(a(10, 5), 200);
    }
}

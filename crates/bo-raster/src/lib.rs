//! Display list → raster image (PNG) using `tiny-skia`.
//!
//! Used for headless rendering (`bo-cli render`, thumbnails) and visual
//! regression tests.
#![forbid(unsafe_code)]

pub mod codecs;
pub mod compose;
pub mod gif;
pub mod jpeg;
pub mod picfx;

use bo_render::{Command, GradientStop, ImageSource, PageList, PathSeg, Rgba};
use bo_text::{FaceId, FontManager, PathCmd};
use std::collections::{BTreeMap, HashMap};
pub use tiny_skia::Pixmap as RasterPixmap;
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, LinearGradient, Mask, Paint, PathBuilder, Pattern,
    Pixmap, Point, RadialGradient, Rect, Shader, SpreadMode, Stroke, StrokeDash, Transform,
};

/// Rendering errors.
#[derive(Debug)]
pub enum RasterError {
    /// Requested image would be empty or unreasonably large.
    InvalidSize(u32, u32),
    /// PNG encoding failed.
    Encode(String),
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RasterError::InvalidSize(w, h) => write!(f, "invalid image size {w}x{h}"),
            RasterError::Encode(e) => write!(f, "PNG encoding failed: {e}"),
        }
    }
}

impl std::error::Error for RasterError {}

/// Maximum width/height of a rendered page in pixels.
pub const MAX_DIMENSION: u32 = 16_384;

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

/// Encoded images (PNG or JPEG) referenced by `Image` commands, keyed by id.
pub type Images = BTreeMap<String, Vec<u8>>;

/// Render a page at `scale` pixels per point (1.0 = 72 dpi, 96/72 = 96 dpi).
pub fn render_page(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
) -> Result<Pixmap, RasterError> {
    render_page_with(page, fonts, scale, &Images::new())
}

/// Clip / transform state saved by `Push*` commands.
enum Saved {
    Clip(Option<Mask>),
    Transform(Transform),
    Opacity(f32),
}

/// Colour with its alpha multiplied by `k` (group opacity).
fn fade(c: Rgba, k: f32) -> Rgba {
    if k >= 1.0 {
        return c;
    }
    Rgba(c.0, c.1, c.2, (f32::from(c.3) * k).round() as u8)
}

/// Render a page, resolving `Image` commands from `images`.
pub fn render_page_with(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &dyn ImageSource,
) -> Result<Pixmap, RasterError> {
    render_onto(page, fonts, scale, images, true)
}

/// Render a page on a transparent canvas (a layer composited later, e.g. one shape
/// layer of the slide show), resolving `Image` commands from `images`.
pub fn render_layer(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &dyn ImageSource,
) -> Result<Pixmap, RasterError> {
    render_onto(page, fonts, scale, images, false)
}

fn render_onto(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &dyn ImageSource,
    opaque: bool,
) -> Result<Pixmap, RasterError> {
    let w = (page.width * scale).ceil() as u32;
    let h = (page.height * scale).ceil() as u32;
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(RasterError::InvalidSize(w, h));
    }
    let mut pm = Pixmap::new(w, h).ok_or(RasterError::InvalidSize(w, h))?;
    if opaque {
        pm.fill(tiny_skia::Color::WHITE);
    }
    let mut t = Transform::from_scale(scale, scale);
    let mut glyph_cache: HashMap<(u32, u16), Option<tiny_skia::Path>> = HashMap::new();
    let mut decoded: HashMap<&str, Option<Pixmap>> = HashMap::new();
    let mut mask: Option<Mask> = None;
    let mut stack: Vec<Saved> = Vec::new();
    let mut alpha = 1.0_f32;

    for cmd in &page.commands {
        match cmd {
            Command::FillRect { x, y, w, h, color } => {
                if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                    pm.fill_rect(r, &paint(fade(*color, alpha)), t, mask.as_ref());
                }
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                dash,
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(*x2, *y2);
                if let Some(path) = pb.finish() {
                    let stroke = Stroke {
                        width: *width,
                        dash: dash
                            .and_then(|[on, off]| tiny_skia::StrokeDash::new(vec![on, off], 0.0)),
                        ..Stroke::default()
                    };
                    pm.stroke_path(
                        &path,
                        &paint(fade(*color, alpha)),
                        &stroke,
                        t,
                        mask.as_ref(),
                    );
                }
            }
            Command::Glyphs {
                face,
                size,
                x,
                y,
                color,
                glyphs,
                ..
            } => {
                let upem = f32::from(fonts.info(FaceId(*face)).units_per_em);
                let s = size / upem;
                let p = paint(fade(*color, alpha));
                for g in glyphs {
                    let path = glyph_cache
                        .entry((*face, g.0))
                        .or_insert_with(|| glyph_path(fonts, FaceId(*face), g.0));
                    if let Some(path) = path {
                        // Font units are y-up: flip with a negative y scale.
                        let gt = t.pre_translate(x + g.1, y + g.2).pre_scale(s, -s);
                        pm.fill_path(path, &p, FillRule::Winding, gt, mask.as_ref());
                    }
                }
            }
            Command::PushClip { x, y, w, h } => {
                stack.push(Saved::Clip(mask.clone()));
                let path = Rect::from_xywh(*x, *y, *w, *h).map(PathBuilder::from_rect);
                mask = intersect(mask.take(), path.as_ref(), t, w_h(&pm));
            }
            Command::PushClipPath { segs } => {
                stack.push(Saved::Clip(mask.clone()));
                mask = intersect(mask.take(), build_path(segs).as_ref(), t, w_h(&pm));
            }
            Command::PopClip => {
                if let Some(Saved::Clip(m)) = stack.pop() {
                    mask = m;
                }
            }
            Command::PushTransform { m } => {
                stack.push(Saved::Transform(t));
                let [a, b, c, d, e, f] = *m;
                t = t.pre_concat(Transform::from_row(a, b, c, d, e, f));
            }
            Command::PopTransform => {
                if let Some(Saved::Transform(saved)) = stack.pop() {
                    t = saved;
                }
            }
            Command::Path { segs, fill, stroke } => {
                let Some(path) = build_path(segs) else {
                    continue;
                };
                if let Some(mut p) = fill.as_ref().and_then(make_paint) {
                    if alpha < 1.0 {
                        if let bo_render::Paint::Solid { color } =
                            fill.as_ref().unwrap_or(&bo_render::Paint::Solid {
                                color: Rgba(0, 0, 0, 0),
                            })
                        {
                            p = paint(fade(*color, alpha));
                        }
                    }
                    pm.fill_path(&path, &p, FillRule::Winding, t, mask.as_ref());
                }
                if let Some(s) = stroke {
                    let st = make_stroke(s);
                    pm.stroke_path(&path, &paint(fade(s.color, alpha)), &st, t, mask.as_ref());
                }
            }
            Command::PushOpacity { alpha: a } => {
                stack.push(Saved::Opacity(alpha));
                alpha *= a.clamp(0.0, 1.0);
            }
            Command::PopOpacity => {
                if let Some(Saved::Opacity(a)) = stack.pop() {
                    alpha = a;
                }
            }
            Command::BlurPath {
                segs,
                color,
                radius,
                spread,
            } => {
                if let Some(path) = build_path(segs) {
                    blur_fill(
                        &mut pm,
                        &path,
                        fade(*color, alpha),
                        (*radius, *spread),
                        t,
                        mask.as_ref(),
                    );
                }
            }
            Command::Image {
                x,
                y,
                w,
                h,
                image,
                crop,
                rotation,
            } => {
                let img =
                    decoded
                        .entry(image.as_str())
                        .or_insert_with(|| match images.image(image) {
                            Some(b) => decode_image(b),
                            // Derived id: the picture with its adjustments (CORE-PIC-001).
                            None => picfx::resolve_rgba(images, image).and_then(pixmap_of),
                        });
                let Some(img) = img.as_ref() else {
                    // Missing or undecodable image: light grey placeholder.
                    if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                        pm.fill_rect(r, &paint(Rgba(220, 220, 220, 255)), t, mask.as_ref());
                    }
                    continue;
                };
                draw_image(
                    &mut pm,
                    img,
                    [*x, *y, *w, *h],
                    (*crop, *rotation, alpha),
                    t,
                    mask.as_ref(),
                );
            }
        }
    }
    Ok(pm)
}

/// Fill `path` (grown by `spread` points: outline stroked `2 × spread` wide) into a
/// layer, blur it (three box blurs ≈ Gaussian, radius in points under `t`) and
/// composite it (soft shadows, §20.1.8.49; glow, §20.1.8.32).
fn blur_fill(
    pm: &mut Pixmap,
    path: &tiny_skia::Path,
    color: Rgba,
    (radius, spread): (f32, f32),
    t: Transform,
    mask: Option<&Mask>,
) {
    let scale = (t.sx * t.sx + t.ky * t.ky).sqrt().max(1e-3);
    let r = (radius * scale / 2.0).round().max(0.0) as i32;
    let Some(b) = path.bounds().transform(t) else {
        return;
    };
    let pad = (r * 3 + 2) as f32 + spread.max(0.0) * scale;
    let x0 = (b.left() - pad).floor().max(0.0) as i32;
    let y0 = (b.top() - pad).floor().max(0.0) as i32;
    let x1 = (b.right() + pad).ceil().min(pm.width() as f32) as i32;
    let y1 = (b.bottom() + pad).ceil().min(pm.height() as f32) as i32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let Some(mut layer) = Pixmap::new(w, h) else {
        return;
    };
    let lt = Transform::from_translate(-x0 as f32, -y0 as f32).pre_concat(t);
    layer.fill_path(path, &paint(color), FillRule::Winding, lt, None);
    if spread > 0.0 {
        let stroke = tiny_skia::Stroke {
            width: spread * 2.0,
            line_join: tiny_skia::LineJoin::Round,
            line_cap: tiny_skia::LineCap::Round,
            ..tiny_skia::Stroke::default()
        };
        layer.stroke_path(path, &paint(color), &stroke, lt, None);
    }
    if r > 0 {
        let data = layer.data_mut();
        for _ in 0..3 {
            box_blur(data, w as usize, h as usize, r as usize);
        }
    }
    pm.draw_pixmap(
        x0,
        y0,
        layer.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        mask,
    );
}

/// Soft mask of a blurred (and grown) filled path: coverage 0..=255 per pixel over a
/// box in the path's own coordinates (used for PDF soft shadows and glow, which
/// PDF cannot blur itself).
#[derive(Debug, Clone, PartialEq)]
pub struct BlurMask {
    /// Box covered by the mask, in path coordinates (points).
    pub x: f32,
    /// Top of the box, in points.
    pub y: f32,
    /// Box width, in points.
    pub w: f32,
    /// Box height, in points.
    pub h: f32,
    /// Pixel size.
    pub width: u32,
    /// Mask height, in pixels.
    pub height: u32,
    /// Row-major coverage, `width × height` bytes.
    pub alpha: Vec<u8>,
}

/// [`BlurMask`] of `segs` blurred by `radius` and grown by `spread` points,
/// rendered at `px_per_pt` pixels per point (capped to 1024 × 1024 pixels).
pub fn blur_mask(segs: &[PathSeg], radius: f32, spread: f32, px_per_pt: f32) -> Option<BlurMask> {
    let path = build_path(segs)?;
    let b = path.bounds();
    let pad = radius * 1.5 + spread.max(0.0) + 1.0;
    let (x, y) = (b.left() - pad, b.top() - pad);
    let (w, h) = (b.width() + 2.0 * pad, b.height() + 2.0 * pad);
    let s = px_per_pt
        .min(1024.0 / w.max(1.0))
        .min(1024.0 / h.max(1.0))
        .max(0.05);
    let (pw, ph) = (
        (w * s).ceil().max(1.0) as u32,
        (h * s).ceil().max(1.0) as u32,
    );
    let mut pm = Pixmap::new(pw, ph)?;
    let t = Transform::from_scale(s, s).pre_translate(-x, -y);
    blur_fill(
        &mut pm,
        &path,
        Rgba(0, 0, 0, 255),
        (radius, spread),
        t,
        None,
    );
    let alpha = pm.data().chunks_exact(4).map(|p| p[3]).collect();
    Some(BlurMask {
        x,
        y,
        w: pw as f32 / s,
        h: ph as f32 / s,
        width: pw,
        height: ph,
        alpha,
    })
}

/// One horizontal and one vertical box blur pass over premultiplied RGBA.
fn box_blur(data: &mut [u8], w: usize, h: usize, r: usize) {
    let mut tmp = vec![0u8; data.len()];
    blur_lines(data, &mut tmp, w, 4, h, w * 4, r);
    blur_lines(&tmp, data, h, w * 4, w, 4, r);
}

/// Box blur of `lines` lines of `len` pixels (`stride` bytes apart, lines `step` apart).
fn blur_lines(
    from: &[u8],
    to: &mut [u8],
    len: usize,
    stride: usize,
    lines: usize,
    step: usize,
    r: usize,
) {
    let n = (2 * r + 1) as u32;
    let last = len as isize - 1;
    for line in 0..lines {
        let base = line * step;
        for ch in 0..4 {
            let at = |i: isize| u32::from(from[base + i.clamp(0, last) as usize * stride + ch]);
            let ri = r as isize;
            let mut sum: u32 = (-ri..=ri).map(at).sum();
            for i in 0..len {
                to[base + i * stride + ch] = (sum / n) as u8;
                let ii = i as isize;
                sum = sum + at(ii + ri + 1) - at(ii - ri);
            }
        }
    }
}
fn w_h(pm: &Pixmap) -> (u32, u32) {
    (pm.width(), pm.height())
}

/// Current clip ∩ `path` (under `t`). A missing path clips everything.
fn intersect(
    current: Option<Mask>,
    path: Option<&tiny_skia::Path>,
    t: Transform,
    (w, h): (u32, u32),
) -> Option<Mask> {
    let mut m = Mask::new(w, h)?;
    if let Some(p) = path {
        m.fill_path(p, FillRule::Winding, true, t);
    }
    if let Some(cur) = current {
        for (a, b) in m.data_mut().iter_mut().zip(cur.data()) {
            *a = ((u16::from(*a) * u16::from(*b)) / 255) as u8;
        }
    }
    Some(m)
}

fn build_path(segs: &[PathSeg]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for s in segs {
        match *s {
            PathSeg::M(x, y) => pb.move_to(x, y),
            PathSeg::L(x, y) => pb.line_to(x, y),
            PathSeg::Q(a, b, x, y) => pb.quad_to(a, b, x, y),
            PathSeg::C(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
            PathSeg::Z => pb.close(),
        }
    }
    pb.finish()
}

fn color(c: Rgba) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3)
}

fn stops(s: &[GradientStop]) -> Vec<tiny_skia::GradientStop> {
    s.iter()
        .map(|g| tiny_skia::GradientStop::new(g.offset, color(g.color)))
        .collect()
}

fn shader_paint(shader: Shader<'_>) -> Paint<'_> {
    Paint {
        shader,
        anti_alias: true,
        ..Paint::default()
    }
}

fn make_paint(p: &bo_render::Paint) -> Option<Paint<'static>> {
    let fallback = |s: &[GradientStop]| s.last().map(|l| paint(l.color));
    match p {
        bo_render::Paint::Solid { color } => Some(paint(*color)),
        bo_render::Paint::Linear {
            x1,
            y1,
            x2,
            y2,
            stops: s,
        } => {
            if s.len() < 2 {
                return fallback(s);
            }
            LinearGradient::new(
                Point::from_xy(*x1, *y1),
                Point::from_xy(*x2, *y2),
                stops(s),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .map(shader_paint)
            .or_else(|| fallback(s))
        }
        bo_render::Paint::Radial {
            cx,
            cy,
            r,
            stops: s,
        } => {
            if s.len() < 2 {
                return fallback(s);
            }
            let c = Point::from_xy(*cx, *cy);
            RadialGradient::new(
                c,
                0.0,
                c,
                *r,
                stops(s),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .map(shader_paint)
            .or_else(|| fallback(s))
        }
    }
}

fn make_stroke(s: &bo_render::Stroke) -> Stroke {
    Stroke {
        width: s.width.max(0.01),
        line_cap: match s.cap.as_str() {
            "round" => LineCap::Round,
            "square" => LineCap::Square,
            _ => LineCap::Butt,
        },
        line_join: match s.join.as_str() {
            "miter" => LineJoin::Miter,
            "bevel" => LineJoin::Bevel,
            _ => LineJoin::Round,
        },
        dash: if s.dash.len() >= 2 && s.dash.len() & 1 == 0 {
            StrokeDash::new(s.dash.clone(), 0.0)
        } else {
            None
        },
        ..Stroke::default()
    }
}

/// Decode PNG (tiny-skia) or JPEG (zune-jpeg) bytes.
fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    if bytes.starts_with(b"\x89PNG") {
        return Pixmap::decode_png(bytes).ok();
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
        let data = dec.decode().ok()?;
        let (w, h) = dec.dimensions()?;
        let size = tiny_skia::IntSize::from_wh(u32::try_from(w).ok()?, u32::try_from(h).ok()?)?;
        // JPEG pixels are opaque, hence already premultiplied.
        return Pixmap::from_vec(data, size);
    }
    codecs::decode(bytes).and_then(pixmap_of)
}

/// Pixmap (premultiplied) of straight RGBA pixels.
fn pixmap_of(img: codecs::Rgba) -> Option<Pixmap> {
    let mut data = img.pixels;
    for p in data.chunks_exact_mut(4) {
        let a = u16::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(img.width, img.height)?)
}

/// Re-encode a GIF, BMP or TIFF picture as PNG (`None` for other or broken input),
/// so that every display list backend can draw it.
pub fn transcode_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    codecs::decode(bytes)
        .and_then(pixmap_of)
        .and_then(|p| p.encode_png().ok())
}

/// Encode straight RGBA pixels as PNG.
pub fn rgba_to_png(img: codecs::Rgba) -> Option<Vec<u8>> {
    pixmap_of(img).and_then(|p| p.encode_png().ok())
}

fn draw_image(
    pm: &mut Pixmap,
    img: &Pixmap,
    [x, y, w, h]: [f32; 4],
    (crop, rotation, opacity): (Option<[f32; 4]>, f32, f32),
    t: Transform,
    mask: Option<&Mask>,
) {
    let Some(dest) = Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let [cl, ct, cr, cb] = crop.unwrap_or([0.0; 4]);
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let vis_w = (iw * (1.0 - cl - cr)).max(1e-3);
    let vis_h = (ih * (1.0 - ct - cb)).max(1e-3);
    let (sx, sy) = (w / vis_w, h / vis_h);
    // Image pixel (u, v) → user space.
    let pattern_t = Transform::from_row(sx, 0.0, 0.0, sy, x - cl * iw * sx, y - ct * ih * sy);
    let shader = Pattern::new(
        img.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Bilinear,
        opacity.clamp(0.0, 1.0),
        pattern_t,
    );
    let t = if rotation == 0.0 {
        t
    } else {
        t.pre_concat(Transform::from_rotate_at(
            rotation,
            x + w / 2.0,
            y + h / 2.0,
        ))
    };
    pm.fill_rect(dest, &shader_paint(shader), t, mask);
}

fn glyph_path(fonts: &FontManager, face: FaceId, gid: u16) -> Option<tiny_skia::Path> {
    let cmds = fonts.outline(face, gid);
    if cmds.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    for c in cmds {
        match c {
            PathCmd::MoveTo(x, y) => pb.move_to(x, y),
            PathCmd::LineTo(x, y) => pb.line_to(x, y),
            PathCmd::QuadTo(a, b, x, y) => pb.quad_to(a, b, x, y),
            PathCmd::CubicTo(a, b, c2, d, x, y) => pb.cubic_to(a, b, c2, d, x, y),
            PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

/// Render a page and encode it as PNG.
pub fn render_page_png(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
) -> Result<Vec<u8>, RasterError> {
    render_page_png_with(page, fonts, scale, &Images::new())
}

/// Render a page with images and encode it as PNG.
pub fn render_page_png_with(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &dyn ImageSource,
) -> Result<Vec<u8>, RasterError> {
    render_page_with(page, fonts, scale, images)?
        .encode_png()
        .map_err(|e| RasterError::Encode(e.to_string()))
}

/// Render a page with images and encode it as a baseline JPEG (`quality` 1..=100).
pub fn render_page_jpeg_with(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &dyn ImageSource,
    quality: u8,
) -> Result<Vec<u8>, RasterError> {
    let pm = render_page_with(page, fonts, scale, images)?;
    // The page is painted on opaque white, so premultiplied = straight RGBA.
    jpeg::encode_jpeg(pm.data(), pm.width(), pm.height(), quality)
        .ok_or_else(|| RasterError::Encode("JPEG".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blurred_path_is_soft_and_opacity_fades() {
        let square = vec![
            PathSeg::M(20.0, 20.0),
            PathSeg::L(60.0, 20.0),
            PathSeg::L(60.0, 60.0),
            PathSeg::L(20.0, 60.0),
            PathSeg::Z,
        ];
        let page = PageList {
            width: 80.0,
            height: 80.0,
            commands: vec![
                Command::BlurPath {
                    segs: square.clone(),
                    color: Rgba(0, 0, 0, 255),
                    radius: 8.0,
                    spread: 0.0,
                },
                Command::PushOpacity { alpha: 0.5 },
                Command::FillRect {
                    x: 0.0,
                    y: 70.0,
                    w: 10.0,
                    h: 10.0,
                    color: Rgba(255, 0, 0, 255),
                },
                Command::PopOpacity,
            ],
        };
        let pm = render_page(&page, &FontManager::new(), 1.0).unwrap();
        let px = |x: u32, y: u32| pm.pixel(x, y).unwrap();
        // Centre dark, edge half-tone, outside the hard edge still shaded.
        assert!(px(40, 40).red() < 20);
        let edge = px(20, 40).red();
        assert!(edge > 60 && edge < 200, "{edge}");
        assert!(px(16, 40).red() < 250 && px(16, 40).red() > edge);
        // Half-transparent red over white.
        let p = px(5, 75);
        assert!(p.red() > 250 && p.green() > 100 && p.green() < 150, "{p:?}");
    }

    #[test]
    fn glow_grows_the_outline_and_mask_is_soft() {
        let square = vec![
            PathSeg::M(30.0, 30.0),
            PathSeg::L(50.0, 30.0),
            PathSeg::L(50.0, 50.0),
            PathSeg::L(30.0, 50.0),
            PathSeg::Z,
        ];
        let page = |spread: f32| PageList {
            width: 80.0,
            height: 80.0,
            commands: vec![Command::BlurPath {
                segs: square.clone(),
                color: Rgba(0, 0, 0, 255),
                radius: 4.0,
                spread,
            }],
        };
        let plain = render_page(&page(0.0), &FontManager::new(), 1.0).unwrap();
        let glow = render_page(&page(8.0), &FontManager::new(), 1.0).unwrap();
        // 6 pt outside the square: the glow (outline grown by 8 pt) is still dark.
        let at = |pm: &Pixmap| pm.pixel(24, 40).unwrap().red();
        assert!(at(&glow) < 120, "{}", at(&glow));
        assert!(at(&plain) > 200, "{}", at(&plain));
        // The PDF mask: opaque inside, fading out, covering the blur.
        let m = blur_mask(&square, 4.0, 0.0, 2.0).unwrap();
        assert!(m.x < 30.0 - 4.0 && m.w > 28.0);
        assert_eq!(m.alpha.len(), (m.width * m.height) as usize);
        let px = |x: f32, y: f32| {
            let (i, j) = (((x - m.x) * 2.0) as u32, ((y - m.y) * 2.0) as u32);
            m.alpha[(j * m.width + i) as usize]
        };
        assert!(px(40.0, 40.0) > 240);
        let edge = px(30.0, 40.0);
        assert!(edge > 60 && edge < 200, "{edge}");
        assert!(px(m.x + 0.5, 40.0) < 10);
    }

    #[test]
    fn renders_rect_and_size() {
        let page = PageList {
            width: 100.0,
            height: 50.0,
            commands: vec![Command::FillRect {
                x: 10.0,
                y: 10.0,
                w: 20.0,
                h: 20.0,
                color: Rgba(255, 0, 0, 255),
            }],
        };
        let pm = render_page(&page, &FontManager::new(), 2.0).unwrap();
        assert_eq!((pm.width(), pm.height()), (200, 100));
        let px = pm.pixel(40, 40).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 0, 0));
        let px = pm.pixel(5, 5).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 255, 255));
        let png = render_page_png(&page, &FontManager::new(), 1.0).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn clip_limits_painting() {
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Command::PushClip {
                    x: 0.0,
                    y: 0.0,
                    w: 50.0,
                    h: 100.0,
                },
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                    color: Rgba(0, 0, 255, 255),
                },
                Command::PopClip,
            ],
        };
        let pm = render_page(&page, &FontManager::new(), 1.0).unwrap();
        assert_eq!(pm.pixel(25, 50).unwrap().blue(), 255);
        assert_eq!(pm.pixel(75, 50).unwrap().red(), 255);
    }

    #[test]
    fn rejects_huge() {
        let page = PageList {
            width: 1e6,
            height: 10.0,
            commands: vec![],
        };
        assert!(render_page(&page, &FontManager::new(), 1.0).is_err());
    }
    fn solid(c: Rgba) -> bo_render::Paint {
        bo_render::Paint::Solid { color: c }
    }

    #[test]
    fn transform_path_and_clip_path() {
        let tri = vec![
            PathSeg::M(0.0, 0.0),
            PathSeg::L(40.0, 0.0),
            PathSeg::L(0.0, 40.0),
            PathSeg::Z,
        ];
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Command::PushTransform {
                    m: [1.0, 0.0, 0.0, 1.0, 50.0, 50.0],
                },
                Command::Path {
                    segs: tri.clone(),
                    fill: Some(solid(Rgba(255, 0, 0, 255))),
                    stroke: None,
                },
                Command::PopTransform,
                Command::PushClipPath { segs: tri },
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                    color: Rgba(0, 0, 255, 255),
                },
                Command::PopClip,
            ],
        };
        let pm = render_page(&page, &FontManager::new(), 1.0).unwrap();
        let px = pm.pixel(55, 55).unwrap();
        assert_eq!((px.red(), px.blue()), (255, 0));
        let px = pm.pixel(5, 5).unwrap();
        assert_eq!((px.red(), px.blue()), (0, 255));
        // Outside the clip triangle: untouched.
        let px = pm.pixel(35, 35).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 255, 255));
    }

    #[test]
    fn gradient_and_image() {
        let mut src = Pixmap::new(2, 1).unwrap();
        src.pixels_mut()[0] = tiny_skia::ColorU8::from_rgba(0, 255, 0, 255).premultiply();
        src.pixels_mut()[1] = tiny_skia::ColorU8::from_rgba(0, 255, 0, 255).premultiply();
        let mut images = Images::new();
        images.insert("img".into(), src.encode_png().unwrap());
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Command::Path {
                    segs: vec![
                        PathSeg::M(0.0, 0.0),
                        PathSeg::L(100.0, 0.0),
                        PathSeg::L(100.0, 50.0),
                        PathSeg::L(0.0, 50.0),
                        PathSeg::Z,
                    ],
                    fill: Some(bo_render::Paint::Linear {
                        x1: 0.0,
                        y1: 0.0,
                        x2: 100.0,
                        y2: 0.0,
                        stops: vec![
                            GradientStop {
                                offset: 0.0,
                                color: Rgba(0, 0, 0, 255),
                            },
                            GradientStop {
                                offset: 1.0,
                                color: Rgba(255, 255, 255, 255),
                            },
                        ],
                    }),
                    stroke: None,
                },
                Command::Image {
                    x: 0.0,
                    y: 60.0,
                    w: 50.0,
                    h: 40.0,
                    image: "img".into(),
                    crop: None,
                    rotation: 0.0,
                },
            ],
        };
        let pm = render_page_with(&page, &FontManager::new(), 1.0, &images).unwrap();
        assert!(pm.pixel(2, 10).unwrap().red() < 20);
        assert!(pm.pixel(97, 10).unwrap().red() > 235);
        let px = pm.pixel(25, 80).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (0, 255, 0));
        assert_eq!(pm.pixel(75, 80).unwrap().red(), 255);
    }
}

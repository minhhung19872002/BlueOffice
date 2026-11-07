//! Display list → raster image (PNG) using `tiny-skia`.
//!
//! Used for headless rendering (`bo-cli render`, thumbnails) and visual
//! regression tests.
#![forbid(unsafe_code)]

use bo_render::{Command, GradientStop, PageList, PathSeg, Rgba};
use bo_text::{FaceId, FontManager, PathCmd};
use std::collections::{BTreeMap, HashMap};
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
}

/// Render a page, resolving `Image` commands from `images`.
pub fn render_page_with(
    page: &PageList,
    fonts: &FontManager,
    scale: f32,
    images: &Images,
) -> Result<Pixmap, RasterError> {
    let w = (page.width * scale).ceil() as u32;
    let h = (page.height * scale).ceil() as u32;
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(RasterError::InvalidSize(w, h));
    }
    let mut pm = Pixmap::new(w, h).ok_or(RasterError::InvalidSize(w, h))?;
    pm.fill(tiny_skia::Color::WHITE);
    let mut t = Transform::from_scale(scale, scale);
    let mut glyph_cache: HashMap<(u32, u16), Option<tiny_skia::Path>> = HashMap::new();
    let mut decoded: HashMap<&str, Option<Pixmap>> = HashMap::new();
    let mut mask: Option<Mask> = None;
    let mut stack: Vec<Saved> = Vec::new();

    for cmd in &page.commands {
        match cmd {
            Command::FillRect { x, y, w, h, color } => {
                if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                    pm.fill_rect(r, &paint(*color), t, mask.as_ref());
                }
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(*x2, *y2);
                if let Some(path) = pb.finish() {
                    let stroke = Stroke {
                        width: *width,
                        ..Stroke::default()
                    };
                    pm.stroke_path(&path, &paint(*color), &stroke, t, mask.as_ref());
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
                let p = paint(*color);
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
                if let Some(p) = fill.as_ref().and_then(make_paint) {
                    pm.fill_path(&path, &p, FillRule::Winding, t, mask.as_ref());
                }
                if let Some(s) = stroke {
                    let st = make_stroke(s);
                    pm.stroke_path(&path, &paint(s.color), &st, t, mask.as_ref());
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
                let img = decoded
                    .entry(image.as_str())
                    .or_insert_with(|| images.get(image).and_then(|b| decode_image(b)));
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
                    *crop,
                    *rotation,
                    t,
                    mask.as_ref(),
                );
            }
        }
    }
    Ok(pm)
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
    None
}

fn draw_image(
    pm: &mut Pixmap,
    img: &Pixmap,
    [x, y, w, h]: [f32; 4],
    crop: Option<[f32; 4]>,
    rotation: f32,
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
        1.0,
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
    images: &Images,
) -> Result<Vec<u8>, RasterError> {
    render_page_with(page, fonts, scale, images)?
        .encode_png()
        .map_err(|e| RasterError::Encode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

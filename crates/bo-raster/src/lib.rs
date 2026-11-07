//! Display list → raster image (PNG) using `tiny-skia`.
//!
//! Used for headless rendering (`bo-cli render`, thumbnails) and visual
//! regression tests.
#![forbid(unsafe_code)]

use bo_render::{image, Command, Media, PageList, Rgba};
use bo_text::{FaceId, FontManager, PathCmd};
use std::collections::HashMap;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

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

/// Render a page at `scale` pixels per point (1.0 = 72 dpi, 96/72 = 96 dpi).
pub fn render_page(
    page: &PageList,
    fonts: &FontManager,
    media: &Media,
    scale: f32,
) -> Result<Pixmap, RasterError> {
    let w = (page.width * scale).ceil() as u32;
    let h = (page.height * scale).ceil() as u32;
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(RasterError::InvalidSize(w, h));
    }
    let mut pm = Pixmap::new(w, h).ok_or(RasterError::InvalidSize(w, h))?;
    pm.fill(tiny_skia::Color::WHITE);
    let base = Transform::from_scale(scale, scale);
    let mut glyph_cache: HashMap<(u32, u16), Option<tiny_skia::Path>> = HashMap::new();
    let mut images: HashMap<&str, Option<Pixmap>> = HashMap::new();
    let mut clips: Vec<Rect> = Vec::new();
    let mut mask: Option<tiny_skia::Mask> = None;

    for cmd in &page.commands {
        match cmd {
            Command::FillRect { x, y, w, h, color } => {
                if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                    pm.fill_rect(r, &paint(*color), base, mask.as_ref());
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
                    pm.stroke_path(&path, &paint(*color), &stroke, base, mask.as_ref());
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
                        let t = base.pre_translate(x + g.1, y + g.2).pre_scale(s, -s);
                        pm.fill_path(path, &p, FillRule::Winding, t, mask.as_ref());
                    }
                }
            }
            Command::Image {
                x,
                y,
                w,
                h,
                media: name,
                crop,
                rotation,
            } => {
                let pix = images
                    .entry(name.as_str())
                    .or_insert_with(|| media.get(name).and_then(|b| to_pixmap(b, *crop)));
                draw_image(
                    &mut pm,
                    pix.as_ref(),
                    [*x, *y, *w, *h],
                    *rotation,
                    base,
                    mask.as_ref(),
                );
            }
            Command::PushClip { x, y, w, h } => {
                if let Some(r) = Rect::from_xywh(*x, *y, *w, *h) {
                    clips.push(r);
                    mask = build_mask(&clips, w_h(&pm), base);
                }
            }
            Command::PopClip => {
                clips.pop();
                mask = build_mask(&clips, w_h(&pm), base);
            }
        }
    }
    Ok(pm)
}

/// Decode a picture and apply its crop (fractions l, t, r, b of the source).
fn to_pixmap(bytes: &[u8], crop: Option<[f32; 4]>) -> Option<Pixmap> {
    let d = image::decode(bytes)?;
    let mut pm = Pixmap::new(d.width, d.height)?;
    for (dst, src) in pm.pixels_mut().iter_mut().zip(d.rgba.chunks_exact(4)) {
        *dst = tiny_skia::ColorU8::from_rgba(src[0], src[1], src[2], src[3]).premultiply();
    }
    let Some([l, t, r, b]) = crop else {
        return Some(pm);
    };
    let (w, h) = (d.width as f32, d.height as f32);
    let x0 = (l.clamp(0.0, 1.0) * w) as i32;
    let y0 = (t.clamp(0.0, 1.0) * h) as i32;
    let cw = ((1.0 - l - r).clamp(0.0, 1.0) * w).round() as u32;
    let ch = ((1.0 - t - b).clamp(0.0, 1.0) * h).round() as u32;
    let rect = tiny_skia::IntRect::from_xywh(x0, y0, cw.max(1), ch.max(1))?;
    pm.clone_rect(rect).or(Some(pm))
}

fn draw_image(
    pm: &mut Pixmap,
    img: Option<&Pixmap>,
    [x, y, w, h]: [f32; 4],
    rotation: f32,
    base: Transform,
    mask: Option<&tiny_skia::Mask>,
) {
    let t = base.pre_concat(Transform::from_rotate_at(
        rotation,
        x + w / 2.0,
        y + h / 2.0,
    ));
    let Some(img) = img else {
        // Unsupported format (EMF/WMF/…): grey placeholder.
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            pm.fill_rect(r, &paint(Rgba(224, 224, 224, 255)), t, mask);
        }
        return;
    };
    let sx = w / img.width() as f32;
    let sy = h / img.height() as f32;
    let paint = tiny_skia::PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..tiny_skia::PixmapPaint::default()
    };
    pm.draw_pixmap(
        0,
        0,
        img.as_ref(),
        &paint,
        t.pre_translate(x, y).pre_scale(sx, sy),
        mask,
    );
}

fn w_h(pm: &Pixmap) -> (u32, u32) {
    (pm.width(), pm.height())
}

fn build_mask(clips: &[Rect], (w, h): (u32, u32), t: Transform) -> Option<tiny_skia::Mask> {
    if clips.is_empty() {
        return None;
    }
    let mut mask = tiny_skia::Mask::new(w, h)?;
    let first = clips[0];
    mask.fill_path(&PathBuilder::from_rect(first), FillRule::Winding, true, t);
    for r in &clips[1..] {
        let mut m2 = tiny_skia::Mask::new(w, h)?;
        m2.fill_path(&PathBuilder::from_rect(*r), FillRule::Winding, true, t);
        let d1 = mask.data_mut();
        for (a, b) in d1.iter_mut().zip(m2.data()) {
            *a = ((u16::from(*a) * u16::from(*b)) / 255) as u8;
        }
    }
    Some(mask)
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
    media: &Media,
    scale: f32,
) -> Result<Vec<u8>, RasterError> {
    render_page(page, fonts, media, scale)?
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
        let pm = render_page(&page, &FontManager::new(), &Media::new(), 2.0).unwrap();
        assert_eq!((pm.width(), pm.height()), (200, 100));
        let px = pm.pixel(40, 40).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 0, 0));
        let px = pm.pixel(5, 5).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 255, 255));
        let png = render_page_png(&page, &FontManager::new(), &Media::new(), 1.0).unwrap();
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
        let pm = render_page(&page, &FontManager::new(), &Media::new(), 1.0).unwrap();
        assert_eq!(pm.pixel(25, 50).unwrap().blue(), 255);
        assert_eq!(pm.pixel(75, 50).unwrap().red(), 255);
    }

    #[test]
    fn draws_png_image_and_placeholder() {
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, 1, 1);
            enc.set_color(png::ColorType::Rgb);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[0, 200, 0]).unwrap();
        }
        let mut media = Media::new();
        media.insert("/word/media/g.png".into(), std::sync::Arc::new(png));
        media.insert(
            "/word/media/x.emf".into(),
            std::sync::Arc::new(b"\x01\0\0\0".to_vec()),
        );
        let img = |name: &str, x: f32| Command::Image {
            x,
            y: 0.0,
            w: 40.0,
            h: 40.0,
            media: name.into(),
            crop: None,
            rotation: 0.0,
        };
        let page = PageList {
            width: 100.0,
            height: 50.0,
            commands: vec![
                img("/word/media/g.png", 0.0),
                img("/word/media/x.emf", 50.0),
            ],
        };
        let pm = render_page(&page, &FontManager::new(), &media, 1.0).unwrap();
        let px = pm.pixel(20, 20).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (0, 200, 0));
        let px = pm.pixel(70, 20).unwrap();
        assert_eq!(px.red(), 224);
    }

    #[test]
    fn rejects_huge() {
        let page = PageList {
            width: 1e6,
            height: 10.0,
            commands: vec![],
        };
        assert!(render_page(&page, &FontManager::new(), &Media::new(), 1.0).is_err());
    }
}

//! Compositing of rendered layers into video frames (slide show export): each layer
//! is drawn with an opacity, an offset, a scale about the frame centre and an optional
//! clip, in that order, over an opaque background.

use tiny_skia::{
    BlendMode, FillRule, FilterQuality, Mask, PathBuilder, Pixmap, PixmapPaint, Rect, Transform,
};

/// Clip of a placed layer, in fractions of the frame (0..1).
#[derive(Debug, Clone, PartialEq)]
pub enum Clip {
    /// Union of rectangles `[x0, y0, x1, y1]`.
    Rects(Vec<[f32; 4]>),
    /// Disc around `(cx, cy)`; `r` is a fraction of the frame diagonal.
    Circle {
        /// Centre x.
        cx: f32,
        /// Centre y.
        cy: f32,
        /// Radius (fraction of the diagonal).
        r: f32,
    },
}

/// How one layer is drawn on the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// 0 = invisible, 1 = opaque.
    pub opacity: f32,
    /// Offset, in fractions of the frame width / height.
    pub dx: f32,
    /// Vertical offset (fraction of the height).
    pub dy: f32,
    /// Scale about the frame centre (1 = none).
    pub scale: f32,
    /// Clip in frame coordinates (`None` = the whole frame).
    pub clip: Option<Clip>,
}

impl Default for Place {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            dx: 0.0,
            dy: 0.0,
            scale: 1.0,
            clip: None,
        }
    }
}

/// A frame being composited.
pub struct Canvas {
    pm: Pixmap,
}

impl Canvas {
    /// An opaque frame of `w × h` pixels filled with `rgb` (`None` if the size is invalid).
    pub fn new(w: u32, h: u32, rgb: [u8; 3]) -> Option<Self> {
        let mut pm = Pixmap::new(w, h)?;
        pm.fill(tiny_skia::Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255));
        Some(Self { pm })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.pm.width()
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.pm.height()
    }

    /// Draw `src` (top-left at the frame origin) as placed by `p`.
    pub fn draw(&mut self, src: &Pixmap, p: &Place) {
        let opacity = p.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 || p.scale <= 0.0 {
            return;
        }
        let (w, h) = (self.pm.width() as f32, self.pm.height() as f32);
        let mask = p
            .clip
            .as_ref()
            .and_then(|c| clip_mask(c, self.pm.width(), self.pm.height()));
        if p.clip.is_some() && mask.is_none() {
            // An empty clip shows nothing.
            return;
        }
        let t = Transform::from_translate(p.dx * w, p.dy * h).pre_concat(Transform::from_row(
            p.scale,
            0.0,
            0.0,
            p.scale,
            w / 2.0 * (1.0 - p.scale),
            h / 2.0 * (1.0 - p.scale),
        ));
        let whole = t.is_identity()
            || (p.scale == 1.0 && (p.dx * w).fract() == 0.0 && (p.dy * h).fract() == 0.0);
        let paint = PixmapPaint {
            opacity,
            blend_mode: BlendMode::SourceOver,
            quality: if whole {
                FilterQuality::Nearest
            } else {
                FilterQuality::Bilinear
            },
        };
        if whole {
            self.pm.draw_pixmap(
                (p.dx * w) as i32,
                (p.dy * h) as i32,
                src.as_ref(),
                &paint,
                Transform::identity(),
                mask.as_ref(),
            );
        } else {
            self.pm
                .draw_pixmap(0, 0, src.as_ref(), &paint, t, mask.as_ref());
        }
    }

    /// The frame as a pixmap (premultiplied; opaque, so equal to straight RGBA).
    pub fn pixmap(&self) -> &Pixmap {
        &self.pm
    }

    /// Consume the canvas into a pixmap.
    pub fn into_pixmap(self) -> Pixmap {
        self.pm
    }

    /// Straight RGBA bytes, `width × height × 4`.
    pub fn rgba(&self) -> Vec<u8> {
        to_rgba(&self.pm)
    }
}

/// Straight RGBA bytes of a pixmap.
pub fn to_rgba(pm: &Pixmap) -> Vec<u8> {
    pm.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

fn clip_mask(c: &Clip, w: u32, h: u32) -> Option<Mask> {
    let (fw, fh) = (w as f32, h as f32);
    let mut pb = PathBuilder::new();
    match c {
        Clip::Rects(rects) => {
            for &[x0, y0, x1, y1] in rects {
                if let Some(r) = Rect::from_ltrb(x0 * fw, y0 * fh, x1 * fw, y1 * fh) {
                    pb.push_rect(r);
                }
            }
        }
        Clip::Circle { cx, cy, r } => {
            let rad = r * fw.hypot(fh);
            if rad > 0.0 {
                pb.push_circle(cx * fw, cy * fh, rad);
            }
        }
    }
    let path = pb.finish()?;
    let mut m = Mask::new(w, h)?;
    m.fill_path(&path, FillRule::Winding, true, Transform::identity());
    Some(m)
}

/// A solid `rgb` pixmap (used by tests and as a black layer).
pub fn solid(w: u32, h: u32, rgb: [u8; 3]) -> Option<Pixmap> {
    let mut pm = Pixmap::new(w, h)?;
    pm.fill(tiny_skia::Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255));
    Some(pm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(c: &Canvas, x: u32, y: u32) -> [u8; 4] {
        let rgba = c.rgba();
        let o = ((y * c.width() + x) * 4) as usize;
        [rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3]]
    }

    #[test]
    fn opacity_offset_and_clip() {
        let red = solid(20, 10, [255, 0, 0]).unwrap();
        let mut c = Canvas::new(20, 10, [0, 0, 0]).unwrap();
        c.draw(
            &red,
            &Place {
                opacity: 0.5,
                ..Place::default()
            },
        );
        let p = px(&c, 5, 5);
        assert!((127..=129).contains(&p[0]) && p[3] == 255, "{p:?}");

        let mut c = Canvas::new(20, 10, [0, 0, 0]).unwrap();
        c.draw(
            &red,
            &Place {
                dx: 0.5,
                ..Place::default()
            },
        );
        assert_eq!(px(&c, 5, 5), [0, 0, 0, 255]);
        assert_eq!(px(&c, 15, 5), [255, 0, 0, 255]);

        let mut c = Canvas::new(20, 10, [0, 0, 0]).unwrap();
        c.draw(
            &red,
            &Place {
                clip: Some(Clip::Rects(vec![[0.0, 0.0, 0.25, 1.0]])),
                ..Place::default()
            },
        );
        assert_eq!(px(&c, 2, 5), [255, 0, 0, 255]);
        assert_eq!(px(&c, 10, 5), [0, 0, 0, 255]);

        let mut c = Canvas::new(20, 10, [0, 0, 0]).unwrap();
        c.draw(
            &red,
            &Place {
                clip: Some(Clip::Rects(vec![])),
                ..Place::default()
            },
        );
        assert_eq!(px(&c, 10, 5), [0, 0, 0, 255]);
    }
}

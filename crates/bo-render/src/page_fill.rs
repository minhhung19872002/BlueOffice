//! Drawing of page background fill effects (ECMA-376 Part 4, §19.1.2.5 fill of the §17.2.1
//! page background): gradients as gradient paths (rectangular ones as nested bands), textures
//! as tiled pictures, patterns as the screen pattern over its mixed colour, pictures
//! stretched over the page.

use bo_layout::PageFillPaint;
use bo_model::Color;

use crate::{Command, GradientStop, Paint, PathSeg, Rgba};

/// Most picture tiles drawn on a page.
const MAX_TILES: f32 = 4096.0;

/// Bands of a rectangular gradient (enough for 8-bit steps on a page).
const BOX_BANDS: usize = 96;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<PathSeg> {
    vec![
        PathSeg::M(x, y),
        PathSeg::L(x + w, y),
        PathSeg::L(x + w, y + h),
        PathSeg::L(x, y + h),
        PathSeg::Z,
    ]
}

/// Colour at offset `t` of sorted stops (linear between them, clamped at the ends).
fn at(stops: &[(f32, Color)], t: f32) -> Color {
    let Some(first) = stops.first() else {
        return Color::WHITE;
    };
    if t <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let k = if t1 - t0 > f32::EPSILON {
                (t - t0) / (t1 - t0)
            } else {
                1.0
            };
            let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * k).round() as u8;
            return Color {
                r: m(a.r, b.r),
                g: m(a.g, b.g),
                b: m(a.b, b.b),
            };
        }
    }
    stops.last().map_or(Color::WHITE, |s| s.1)
}

/// Commands drawing `paint` over a `w` × `h` page.
pub(crate) fn commands(paint: &PageFillPaint, w: f32, h: f32, out: &mut Vec<Command>) {
    match paint {
        PageFillPaint::Linear { angle, stops } => {
            // VML angle (Part 4 §19.1.2.5): 0 runs top to bottom, -90 left to right; the
            // gradient spans the page's extent along that direction.
            let a = angle.to_radians();
            let (dx, dy) = (-a.sin(), a.cos());
            let (cx, cy) = (w / 2.0, h / 2.0);
            let half = f32::midpoint(w * dx.abs(), h * dy.abs());
            out.push(Command::Path {
                segs: rect(0.0, 0.0, w, h),
                fill: Some(Paint::Linear {
                    x1: cx - dx * half,
                    y1: cy - dy * half,
                    x2: cx + dx * half,
                    y2: cy + dy * half,
                    stops: stops
                        .iter()
                        .map(|(t, c)| GradientStop {
                            offset: t.clamp(0.0, 1.0),
                            color: (*c).into(),
                        })
                        .collect(),
                }),
                stroke: None,
            });
        }
        PageFillPaint::Box { fx, fy, stops } => {
            // Concentric rectangles around the focus, from the outer edge inwards.
            let (px, py) = (fx * w, fy * h);
            let (ex, ey) = (px.max(w - px), py.max(h - py));
            out.push(Command::PushClip {
                x: 0.0,
                y: 0.0,
                w,
                h,
            });
            for i in (1..=BOX_BANDS).rev() {
                let t = i as f32 / BOX_BANDS as f32;
                let c = at(stops, t - 0.5 / BOX_BANDS as f32);
                let (hw, hh) = (ex * t, ey * t);
                out.push(Command::FillRect {
                    x: px - hw,
                    y: py - hh,
                    w: 2.0 * hw,
                    h: 2.0 * hh,
                    color: c.into(),
                });
            }
            out.push(Command::PopClip);
        }
        PageFillPaint::Tile {
            media,
            w: tw,
            h: th,
        } => {
            if *tw < 0.1 || *th < 0.1 {
                return;
            }
            // Tiny pictures: grow the tile (by whole multiples) to keep the list bounded.
            let (mut tw, mut th) = (*tw, *th);
            while (w / tw).ceil() * (h / th).ceil() > MAX_TILES {
                tw *= 2.0;
                th *= 2.0;
            }
            out.push(Command::PushClip {
                x: 0.0,
                y: 0.0,
                w,
                h,
            });
            let mut y = 0.0;
            while y < h {
                let mut x = 0.0;
                while x < w {
                    out.push(Command::Image {
                        x,
                        y,
                        w: tw,
                        h: th,
                        image: media.clone(),
                        crop: None,
                        rotation: 0.0,
                    });
                    x += tw;
                }
                y += th;
            }
            out.push(Command::PopClip);
        }
        PageFillPaint::Pattern { tile, fg, bg, mix } => {
            out.push(Command::FillRect {
                x: 0.0,
                y: 0.0,
                w,
                h,
                color: (*mix).into(),
            });
            out.push(Command::FillPattern {
                x: 0.0,
                y: 0.0,
                w,
                h,
                tile: *tile,
                fg: Rgba::from(*fg),
                bg: Rgba::from(*bg),
            });
        }
        PageFillPaint::Picture { media } => out.push(Command::Image {
            x: 0.0,
            y: 0.0,
            w,
            h,
            image: media.clone(),
            crop: None,
            rotation: 0.0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_gradient_runs_left_to_right() {
        let mut out = Vec::new();
        let stops = vec![(0.0, Color::BLACK), (1.0, Color::WHITE)];
        commands(
            &PageFillPaint::Linear {
                angle: -90.0,
                stops,
            },
            600.0,
            800.0,
            &mut out,
        );
        let Some(Command::Path {
            fill: Some(Paint::Linear { x1, y1, x2, y2, .. }),
            ..
        }) = out.first()
        else {
            panic!("gradient path");
        };
        assert!((x1 - 0.0).abs() < 0.01 && (x2 - 600.0).abs() < 0.01);
        assert!((y1 - 400.0).abs() < 0.01 && (y2 - 400.0).abs() < 0.01);
    }

    #[test]
    fn box_gradient_bands_and_tiles() {
        let mut out = Vec::new();
        let stops = vec![(0.0, Color::BLACK), (1.0, Color::WHITE)];
        commands(
            &PageFillPaint::Box {
                fx: 0.5,
                fy: 0.5,
                stops,
            },
            100.0,
            100.0,
            &mut out,
        );
        assert_eq!(out.len(), BOX_BANDS + 2);
        // Outermost band (light) first, innermost (dark) last.
        let Command::FillRect { color, w, .. } = &out[1] else {
            panic!("band")
        };
        assert!(color.0 > 250 && (*w - 100.0).abs() < 0.01);
        let mut out = Vec::new();
        commands(
            &PageFillPaint::Tile {
                media: "/word/media/t.jpeg".into(),
                w: 40.0,
                h: 30.0,
            },
            100.0,
            100.0,
            &mut out,
        );
        // 3 × 4 tiles inside a clip.
        assert_eq!(out.len(), 12 + 2);
    }
}

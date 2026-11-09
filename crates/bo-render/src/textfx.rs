//! Drawing of character effects (see `bo_layout::TextFx`): effects behind the glyphs
//! (reflection, glow, shadows, emboss/imprint relief) and over them (gradient fill,
//! hollow outline, text outline), built from the glyph outlines so that every backend
//! (canvas, PNG, PDF) draws them with the ordinary path commands. The glyph run itself
//! is still emitted (transparent when its fill is replaced) so that text stays
//! selectable and extractable.

use crate::{Command, GradientStop, Paint, PathSeg, Rgba, Stroke};
use bo_layout::{Fragment, FxPaint, FxRgba, TextFx};
use bo_text::FontManager;
use bo_text::PathCmd as Font;

fn rgba(p: FxRgba) -> Rgba {
    Rgba(
        p.color.r,
        p.color.g,
        p.color.b,
        (p.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Glyph outlines of a fragment in page points, moved by (`dx`, `dy`); with `mirror`,
/// reflected about the horizontal line y = `mirror`.
pub(crate) fn glyph_segs(
    f: &Fragment,
    fonts: &FontManager,
    dx: f32,
    dy: f32,
    mirror: Option<f32>,
) -> Vec<PathSeg> {
    let k = f.size / f32::from(fonts.info(f.face).units_per_em);
    let sx = f.fx.as_ref().map_or(1.0, |x| x.scale_x);
    let mut segs = Vec::new();
    for g in f.glyphs.iter() {
        let (ox, oy) = (f.x + dx + g.x, f.y + dy + g.y);
        let pt = |x: f32, y: f32| {
            let py = oy - y * k;
            (ox + x * k * sx, mirror.map_or(py, |m| 2.0 * m - py))
        };
        for c in fonts.outline(f.face, g.id) {
            segs.push(match c {
                Font::MoveTo(x, y) => {
                    let (a, b) = pt(x, y);
                    PathSeg::M(a, b)
                }
                Font::LineTo(x, y) => {
                    let (a, b) = pt(x, y);
                    PathSeg::L(a, b)
                }
                Font::QuadTo(x1, y1, x, y) => {
                    let (a, b) = pt(x1, y1);
                    let (c, d) = pt(x, y);
                    PathSeg::Q(a, b, c, d)
                }
                Font::CubicTo(x1, y1, x2, y2, x, y) => {
                    let (a, b) = pt(x1, y1);
                    let (c, d) = pt(x2, y2);
                    let (e, h) = pt(x, y);
                    PathSeg::C(a, b, c, d, e, h)
                }
                Font::Close => PathSeg::Z,
            });
        }
    }
    segs
}

/// Ascent and descent of a fragment's face at its size (points).
fn extent(f: &Fragment, fonts: &FontManager) -> (f32, f32) {
    let info = fonts.info(f.face);
    let k = f.size / f32::from(info.units_per_em);
    (f32::from(info.ascender) * k, -f32::from(info.descender) * k)
}

fn stops(v: &[(f32, FxRgba)]) -> Vec<GradientStop> {
    v.iter()
        .map(|(o, c)| GradientStop {
            offset: o.clamp(0.0, 1.0),
            color: rgba(*c),
        })
        .collect()
}

/// A paint over the fragment box (gradients span the box).
fn paint(p: &FxPaint, f: &Fragment, fonts: &FontManager) -> Option<Paint> {
    let (asc, desc) = extent(f, fonts);
    let (x0, y0, x1, y1) = (f.x, f.y - asc, f.x + f.width, f.y + desc);
    match p {
        FxPaint::None => None,
        FxPaint::Solid(c) => Some(Paint::Solid { color: rgba(*c) }),
        FxPaint::Linear { angle, stops: s } => {
            // The gradient line through the box centre in direction `angle` (clockwise
            // from left→right, y down), long enough to cover the box.
            let (cx, cy) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
            let (sin, cos) = angle.to_radians().sin_cos();
            let half = f32::midpoint((x1 - x0) * cos.abs(), (y1 - y0) * sin.abs());
            Some(Paint::Linear {
                x1: cx - cos * half,
                y1: cy - sin * half,
                x2: cx + cos * half,
                y2: cy + sin * half,
                stops: stops(s),
            })
        }
        FxPaint::Radial { stops: s } => Some(Paint::Radial {
            cx: f32::midpoint(x0, x1),
            cy: f32::midpoint(y0, y1),
            r: ((x1 - x0).hypot(y1 - y0) / 2.0).max(0.1),
            stops: stops(s),
        }),
    }
}

fn fill(segs: Vec<PathSeg>, color: Rgba) -> Command {
    Command::Path {
        segs,
        fill: Some(Paint::Solid { color }),
        stroke: None,
    }
}

fn stroke(width: f32, color: Rgba) -> Stroke {
    Stroke {
        width,
        color,
        dash: Vec::new(),
        cap: "round".into(),
        join: "round".into(),
    }
}

/// Offset of the relief copies of emboss / imprint / shadow (BlueOffice rendering
/// choice: a twentieth of the font size, at least a quarter point).
fn relief(f: &Fragment) -> f32 {
    (f.size / 20.0).max(0.25)
}

/// Effects painted before the glyph run.
pub(crate) fn behind(
    f: &Fragment,
    fx: &TextFx,
    color: Rgba,
    fonts: &FontManager,
    out: &mut Vec<Command>,
) {
    // [MS-DOCX] w14:reflection: the glyphs mirrored under the descent, fading out.
    if let Some(r) = &fx.reflection {
        let (asc, desc) = extent(f, fonts);
        let m = f.y + desc + r.dist / 2.0;
        let segs = glyph_segs(f, fonts, 0.0, 0.0, Some(m));
        let len = (asc + desc) * r.end_pos.max(0.01);
        let at = |a: f32| Rgba(color.0, color.1, color.2, (a.clamp(0.0, 1.0) * 255.0) as u8);
        out.push(Command::Path {
            segs,
            fill: Some(Paint::Linear {
                x1: f.x,
                y1: m,
                x2: f.x,
                y2: m + len,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: at(r.start_alpha),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: at(r.end_alpha),
                    },
                ],
            }),
            stroke: None,
        });
    }
    // [MS-DOCX] w14:glow: the outline grown by half the radius and blurred by it.
    if let Some((rad, p)) = fx.glow {
        out.push(Command::BlurPath {
            segs: glyph_segs(f, fonts, 0.0, 0.0, None),
            color: rgba(p),
            radius: rad,
            spread: rad / 2.0,
        });
    }
    // [MS-DOCX] w14:shadow: offset copy, blurred.
    if let Some(s) = fx.soft_shadow {
        let segs = glyph_segs(f, fonts, s.dx, s.dy, None);
        out.push(if s.blur > 0.0 {
            Command::BlurPath {
                segs,
                color: rgba(s.paint),
                radius: s.blur,
                spread: 0.0,
            }
        } else {
            fill(segs, rgba(s.paint))
        });
    }
    let d = relief(f);
    // §17.3.2.31 shadow: a grey copy down and to the right.
    if fx.shadow {
        out.push(fill(
            glyph_segs(f, fonts, d, d, None),
            Rgba(0x80, 0x80, 0x80, 255),
        ));
    }
    // §17.3.2.13 emboss (raised): light upper-left edge, dark lower-right edge;
    // §17.3.2.18 imprint (engraved): the reverse.
    if fx.emboss || fx.imprint {
        let (light, dark) = (Rgba(0xFF, 0xFF, 0xFF, 255), Rgba(0x60, 0x60, 0x60, 255));
        let (tl, br) = if fx.emboss {
            (light, dark)
        } else {
            (dark, light)
        };
        out.push(fill(glyph_segs(f, fonts, -d / 2.0, -d / 2.0, None), tl));
        out.push(fill(glyph_segs(f, fonts, d / 2.0, d / 2.0, None), br));
    }
}

/// Colour of the glyph run itself: transparent when the fill is replaced by a
/// gradient or a hollow outline, the solid w14 fill when there is one.
pub(crate) fn glyph_color(fx: &TextFx, color: Rgba) -> Rgba {
    if fx.replaces_fill() {
        return Rgba(color.0, color.1, color.2, 0);
    }
    match &fx.fill {
        Some(FxPaint::Solid(c)) => rgba(*c),
        _ => color,
    }
}

/// Effects painted over the glyph run.
pub(crate) fn front(
    f: &Fragment,
    fx: &TextFx,
    color: Rgba,
    fonts: &FontManager,
    out: &mut Vec<Command>,
) {
    let needs_outline = fx.hollow
        || fx.stroke.is_some()
        || matches!(
            fx.fill,
            Some(FxPaint::Linear { .. } | FxPaint::Radial { .. })
        );
    if !needs_outline {
        return;
    }
    let segs = glyph_segs(f, fonts, 0.0, 0.0, None);
    if segs.is_empty() {
        return;
    }
    // w14:textFill gradient.
    if let Some(p @ (FxPaint::Linear { .. } | FxPaint::Radial { .. })) = &fx.fill {
        out.push(Command::Path {
            segs: segs.clone(),
            fill: paint(p, f, fonts),
            stroke: None,
        });
    }
    // §17.3.2.23 outline: the glyph outlines in the text colour, interiors empty.
    if fx.hollow && fx.stroke.is_none() {
        out.push(Command::Path {
            segs: segs.clone(),
            fill: None,
            stroke: Some(stroke((f.size / 30.0).max(0.4), color)),
        });
    }
    // w14:textOutline (a gradient stroke is drawn in its first colour).
    if let Some((w, p)) = &fx.stroke {
        if let Some(c) = p.first() {
            out.push(Command::Path {
                segs,
                fill: None,
                stroke: Some(stroke(*w, rgba(c))),
            });
        }
    }
}

//! Shape Format fills and effects of a shape (BlueDoc round 20, `bo_model::shape_style`):
//! the paths of a shape gain a gradient (ECMA-376 Part 1, §20.1.8.33 gradFill) or picture
//! (§20.1.8.14 blipFill) paint and transparency, and its effects (§20.1.8.26 effectLst)
//! become extra paths around it — an outer shadow and a glow as soft silhouettes, a
//! reflection mirrored below it with a fading paint, soft edges as copies clipped to inset
//! outlines, bevel and inner shadow as gradient overlays, a 3-D rotation as the outline
//! turned by the camera preset (§20.1.5.5). BlueOffice's own approximations, drawn with the
//! display list's existing commands so that every backend draws them.

use crate::types::{PaintStop, PathCmd, PathPaint, PathStyle, ShapePath};
use bo_model::picture_style::PicEffects;
use bo_model::shape_style::{camera_projection, opacity, ShapeGradient, ShapeStyle};
use bo_model::{Color, Document};

const EMU_PT: f64 = 12_700.0;

#[allow(clippy::cast_possible_truncation)]
fn pt(emu: i64) -> f32 {
    (emu as f64 / EMU_PT) as f32
}

fn rgb(c: u32) -> Color {
    Color {
        r: ((c >> 16) & 255) as u8,
        g: ((c >> 8) & 255) as u8,
        b: (c & 255) as u8,
    }
}

/// `segs` with every point mapped by `f`.
pub(crate) fn map_segs(segs: &[PathCmd], f: &dyn Fn(f32, f32) -> (f32, f32)) -> Vec<PathCmd> {
    segs.iter()
        .map(|s| match *s {
            PathCmd::M(x, y) => {
                let (x, y) = f(x, y);
                PathCmd::M(x, y)
            }
            PathCmd::L(x, y) => {
                let (x, y) = f(x, y);
                PathCmd::L(x, y)
            }
            PathCmd::Q(a, b, x, y) => {
                let ((a, b), (x, y)) = (f(a, b), f(x, y));
                PathCmd::Q(a, b, x, y)
            }
            PathCmd::C(a, b, c, d, x, y) => {
                let ((a, b), (c, d), (x, y)) = (f(a, b), f(c, d), f(x, y));
                PathCmd::C(a, b, c, d, x, y)
            }
            PathCmd::Z => PathCmd::Z,
        })
        .collect()
}

/// The paint of gradient `g` over a `w` × `h` box.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn gradient_paint(g: &ShapeGradient, w: f32, h: f32) -> PathPaint {
    let stops: Vec<PaintStop> = g
        .stops
        .iter()
        .map(|s| {
            (
                (s.pos.clamp(0, 100_000) as f32) / 100_000.0,
                rgb(s.color),
                opacity(s.alpha),
            )
        })
        .collect();
    if g.kind == "linear" || g.kind.is_empty() {
        // §20.1.8.41 lin: the gradient line through the box centre at `ang`, spanning the
        // box's extent along it.
        let a = (f64::from(g.angle) / 60_000.0).to_radians();
        let (s, c) = (a.sin() as f32, a.cos() as f32);
        let half = ((w * c).abs() + (h * s).abs()) / 2.0;
        let (cx, cy) = (w / 2.0, h / 2.0);
        PathPaint::Linear {
            x1: cx - half * c,
            y1: cy - half * s,
            x2: cx + half * c,
            y2: cy + half * s,
            stops,
        }
    } else {
        // §20.1.8.46 path: from the focus rectangle (fillToRect) outwards; rectangular and
        // shape paths are drawn as circles reaching the farthest corner.
        let [l, t, r, b] = g
            .focus
            .map(|v| (v.clamp(-100_000, 200_000) as f32) / 100_000.0);
        let (fx, fy) = ((l + (1.0 - r)) / 2.0 * w, (t + (1.0 - b)) / 2.0 * h);
        let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .iter()
            .map(|(x, y)| (x - fx).hypot(y - fy))
            .fold(0.0f32, f32::max);
        let r = if g.kind == "circle" { far } else { far * 0.85 };
        PathPaint::Radial {
            cx: fx,
            cy: fy,
            r: r.max(0.5),
            stops,
        }
    }
}

/// The paths of a `w` × `h` shape with style `style` (see the module docs). `paths` are the
/// shape's own paths with their solid fill and stroke.
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
pub(crate) fn styled(
    doc: &Document,
    style: &ShapeStyle,
    mut paths: Vec<ShapePath>,
    w: f32,
    h: f32,
) -> Vec<ShapePath> {
    let fx: Option<&PicEffects> = style.effects.as_ref().filter(|e| !e.is_empty());
    // 3-D rotation: the outline turned by the camera.
    if let Some(cam) = fx.and_then(|e| e.camera.as_deref()) {
        let proj = camera_projection(cam, f64::from(w), f64::from(h));
        let f = |x: f32, y: f32| {
            let (x, y) = proj(f64::from(x), f64::from(y));
            (x as f32, y as f32)
        };
        for p in &mut paths {
            p.segs = map_segs(&p.segs, &f);
        }
    }
    let fill_alpha = style.fill_alpha.map_or(1.0, opacity);
    let line_alpha = style.line_alpha.map_or(1.0, opacity);
    let paint = if let Some(g) = &style.gradient {
        Some(gradient_paint(g, w, h))
    } else {
        style.picture.as_ref().map(|p| {
            let tile = p.tile.then(|| {
                let (tw, th) = doc
                    .media
                    .get(&p.media)
                    .and_then(|b| crate::page_fill::image_points(b))
                    .unwrap_or((64.0, 64.0));
                let k = if p.scale > 0 {
                    p.scale as f32 / 100_000.0
                } else {
                    1.0
                };
                ((tw * k).max(2.0), (th * k).max(2.0))
            });
            PathPaint::Picture {
                media: p.media.clone(),
                rect: [0.0, 0.0, w, h],
                tile,
                alpha: if p.alpha > 0 { opacity(p.alpha) } else { 1.0 },
            }
        })
    };
    // The silhouette of the shape: its filled paths (else every path).
    let mut outline: Vec<PathCmd> = paths
        .iter()
        .filter(|p| p.fill.is_some())
        .flat_map(|p| p.segs.iter().copied())
        .collect();
    if outline.is_empty() {
        outline = paths.iter().flat_map(|p| p.segs.iter().copied()).collect();
    }
    let base_color = paths.iter().find_map(|p| p.fill).or_else(|| {
        style
            .gradient
            .as_ref()
            .and_then(|g| g.stops.first())
            .map(|s| rgb(s.color))
    });
    let filled = base_color.is_some() || paint.is_some();
    let mut out: Vec<ShapePath> = Vec::new();
    let silhouette =
        |segs: Vec<PathCmd>, color: Color, alpha: f32, blur: Option<(f32, f32)>| ShapePath {
            segs,
            fill: Some(color),
            stroke: None,
            style: Some(Box::new(PathStyle {
                fill_alpha: alpha,
                blur,
                ..PathStyle::default()
            })),
        };
    if let Some(e) = fx {
        // §20.1.8.49 outerShdw: the silhouette scaled about the bottom edge (algn b), moved
        // by dist along dir, blurred by blurRad.
        if let Some(s) = e.shadow.as_ref().filter(|s| !s.inner) {
            let (sin, cos) = (f64::from(s.dir) / 60_000.0).to_radians().sin_cos();
            let (dx, dy) = (
                (f64::from(pt(s.dist)) * cos) as f32,
                (f64::from(pt(s.dist)) * sin) as f32,
            );
            let k = |v: i32| if v == 0 { 1.0 } else { v as f32 / 100_000.0 };
            let (sx, sy) = (k(s.sx), k(s.sy));
            let segs = map_segs(&outline, &|x, y| {
                (w / 2.0 + (x - w / 2.0) * sx + dx, h + (y - h) * sy + dy)
            });
            let blur = pt(s.blur);
            out.push(silhouette(
                segs,
                rgb(s.color),
                opacity(s.alpha) * fill_alpha.max(0.25),
                (blur > 0.0).then_some((blur, 0.0)),
            ));
        }
        // §20.1.8.32 glow: the outline grown by half the radius, blurred by it.
        if let Some(g) = &e.glow {
            let r = pt(g.rad);
            if r > 0.0 {
                out.push(silhouette(
                    outline.clone(),
                    rgb(g.color),
                    opacity(g.alpha),
                    Some((r, r / 2.0)),
                ));
            }
        }
        // §20.1.8.50 reflection: mirrored below the shape, fading from stA to endA over
        // endPos of its height.
        if let Some(r) = &e.reflection {
            let dist = pt(r.dist);
            let top = h + dist;
            let mirror = |segs: &[PathCmd]| map_segs(segs, &|x, y| (x, top + (h - y)));
            let (st, end) = (opacity(r.st_a), opacity(r.end_a));
            let span = (h * (r.end_pos.clamp(1, 100_000) as f32) / 100_000.0).max(1.0);
            for p in &paths {
                let fade = |c: Color| PathPaint::Linear {
                    x1: 0.0,
                    y1: top,
                    x2: 0.0,
                    y2: top + span,
                    stops: vec![(0.0, c, st * fill_alpha), (1.0, c, end * fill_alpha)],
                };
                let color = p.fill.or(base_color);
                if p.fill.is_none() && p.stroke.is_none() {
                    continue;
                }
                out.push(ShapePath {
                    segs: mirror(&p.segs),
                    fill: p.fill.and(color),
                    stroke: p.stroke.clone(),
                    style: Some(Box::new(PathStyle {
                        paint: p.fill.and(color).map(fade),
                        line_alpha: line_alpha * (st + end) / 2.0,
                        clip: Some(vec![
                            PathCmd::M(-w, top),
                            PathCmd::L(2.0 * w, top),
                            PathCmd::L(2.0 * w, top + span),
                            PathCmd::L(-w, top + span),
                            PathCmd::Z,
                        ]),
                        ..PathStyle::default()
                    })),
                });
            }
        }
    }
    // The shape itself, with its paint and transparency.
    let main: Vec<ShapePath> = paths
        .into_iter()
        .map(|mut p| {
            let has_fill = p.fill.is_some();
            let st = PathStyle {
                paint: if has_fill { paint.clone() } else { None },
                fill_alpha,
                line_alpha,
                ..PathStyle::default()
            };
            if st != PathStyle::default() {
                p.style = Some(Box::new(st));
            }
            p
        })
        .collect();
    // §20.1.8.53 softEdge: copies clipped to inset outlines, each faint.
    match fx.and_then(|e| e.soft_edge).filter(|r| *r > 0) {
        Some(rad) if filled => {
            let steps = bo_drawingml::effects::soft_edge_steps(
                f64::from(pt(rad)),
                f64::from(w),
                f64::from(h),
            );
            for (_, m, alpha) in steps {
                let clip = map_segs(&outline, &|x, y| {
                    let (x, y) = (f64::from(x), f64::from(y));
                    (
                        (m[0] * x + m[2] * y + m[4]) as f32,
                        (m[1] * x + m[3] * y + m[5]) as f32,
                    )
                });
                for p in &main {
                    let mut p = p.clone();
                    let mut st = p.style.take().map_or_else(PathStyle::default, |b| *b);
                    st.clip = Some(clip.clone());
                    st.opacity = alpha as f32;
                    p.style = Some(Box::new(st));
                    out.push(p);
                }
            }
        }
        _ => out.extend(main),
    }
    if let Some(e) = fx {
        let overlay = |paint: PathPaint| ShapePath {
            segs: outline.clone(),
            fill: Some(Color::WHITE),
            stroke: None,
            style: Some(Box::new(PathStyle {
                paint: Some(paint),
                ..PathStyle::default()
            })),
        };
        // §20.1.8.40 innerShdw: a band of the shadow colour inside the edge the shadow
        // comes from, fading over the blur.
        if let Some(s) = e.shadow.as_ref().filter(|s| s.inner) {
            let a = (f64::from(s.dir) / 60_000.0).to_radians();
            let (sin, cos) = (a.sin() as f32, a.cos() as f32);
            let half = ((w * cos).abs() + (h * sin).abs()) / 2.0;
            let reach = (pt(s.blur) + pt(s.dist)).max(1.0);
            let f = (reach / (2.0 * half).max(1.0)).min(0.5);
            let c = rgb(s.color);
            let (cx, cy) = (w / 2.0, h / 2.0);
            out.push(overlay(PathPaint::Linear {
                x1: cx - half * cos,
                y1: cy - half * sin,
                x2: cx + half * cos,
                y2: cy + half * sin,
                stops: vec![(0.0, c, opacity(s.alpha)), (f, c, 0.0), (1.0, c, 0.0)],
            }));
        }
        // §20.1.5.12 sp3d bevelT: lit top and left edges, shaded bottom and right ones.
        if let Some(b) = e.bevel.as_ref().filter(|_| filled) {
            let bw = pt(b.w).max(1.0);
            let (fy, fx2) = ((bw / h.max(1.0)).min(0.45), (bw / w.max(1.0)).min(0.45));
            let (white, black) = (Color::WHITE, Color::BLACK);
            out.push(overlay(PathPaint::Linear {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: h,
                stops: vec![
                    (0.0, white, 0.55),
                    (fy, white, 0.0),
                    (1.0 - fy, black, 0.0),
                    (1.0, black, 0.35),
                ],
            }));
            out.push(overlay(PathPaint::Linear {
                x1: 0.0,
                y1: 0.0,
                x2: w,
                y2: 0.0,
                stops: vec![
                    (0.0, white, 0.3),
                    (fx2, white, 0.0),
                    (1.0 - fx2, black, 0.0),
                    (1.0, black, 0.25),
                ],
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::picture_style::{PicGlow, PicReflection, PicShadow};
    use bo_model::shape_style::ShapeStop;

    fn rect(w: f32, h: f32) -> ShapePath {
        ShapePath {
            segs: vec![
                PathCmd::M(0.0, 0.0),
                PathCmd::L(w, 0.0),
                PathCmd::L(w, h),
                PathCmd::L(0.0, h),
                PathCmd::Z,
            ],
            fill: Some(Color::BLACK),
            stroke: Some((1.0, Color::BLACK, None)),
            style: None,
        }
    }

    #[test]
    fn gradient_and_transparency_go_on_the_shape() {
        let doc = Document::default();
        let style = ShapeStyle {
            gradient: Some(ShapeGradient {
                kind: "linear".into(),
                angle: 5_400_000,
                focus: [0; 4],
                stops: vec![
                    ShapeStop {
                        pos: 0,
                        color: 0xFF_0000,
                        alpha: 100_000,
                    },
                    ShapeStop {
                        pos: 100_000,
                        color: 0x00_00FF,
                        alpha: 100_000,
                    },
                ],
            }),
            fill_alpha: Some(50_000),
            ..ShapeStyle::default()
        };
        let out = styled(&doc, &style, vec![rect(100.0, 50.0)], 100.0, 50.0);
        assert_eq!(out.len(), 1);
        let st = out[0].style.as_deref().expect("style");
        assert!((st.fill_alpha - 0.5).abs() < 1e-6);
        // 90°: top to bottom through the centre.
        let Some(PathPaint::Linear { x1, y1, x2, y2, .. }) = &st.paint else {
            panic!("linear paint");
        };
        assert!((x1 - 50.0).abs() < 1e-3 && (x2 - 50.0).abs() < 1e-3);
        assert!(y1.abs() < 1e-3 && (y2 - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effects_add_paths_around_the_shape() {
        let doc = Document::default();
        let style = ShapeStyle {
            effects: Some(PicEffects {
                shadow: Some(PicShadow {
                    blur: 50_800,
                    dist: 38_100,
                    dir: 2_700_000,
                    color: 0,
                    alpha: 40_000,
                    ..PicShadow::default()
                }),
                glow: Some(PicGlow {
                    rad: 101_600,
                    color: 0xFF_C000,
                    alpha: 60_000,
                }),
                reflection: Some(PicReflection {
                    st_a: 50_000,
                    end_a: 300,
                    end_pos: 55_000,
                    ..PicReflection::default()
                }),
                soft_edge: Some(63_500),
                bevel: Some(bo_model::picture_style::PicBevel {
                    prst: "circle".into(),
                    w: 76_200,
                    h: 76_200,
                }),
                camera: None,
            }),
            ..ShapeStyle::default()
        };
        let out = styled(&doc, &style, vec![rect(100.0, 50.0)], 100.0, 50.0);
        // Shadow, glow, reflection, soft-edge copies, two bevel overlays.
        let blurred = out
            .iter()
            .filter(|p| p.style.as_ref().is_some_and(|s| s.blur.is_some()))
            .count();
        assert_eq!(blurred, 2);
        assert!(out.len() > 6, "{}", out.len());
        // The reflection lies below the shape.
        let refl = &out[2];
        let PathCmd::M(_, y) = refl.segs[0] else {
            panic!("move");
        };
        assert!(y >= 50.0);
        // No style: the paths are untouched.
        let plain = styled(
            &doc,
            &ShapeStyle::default(),
            vec![rect(10.0, 10.0)],
            10.0,
            10.0,
        );
        assert_eq!(plain, vec![rect(10.0, 10.0)]);
    }

    #[test]
    fn camera_turns_the_outline_inside_the_box() {
        let doc = Document::default();
        let style = ShapeStyle {
            effects: Some(PicEffects {
                camera: Some("isometricLeftDown".into()),
                ..PicEffects::default()
            }),
            ..ShapeStyle::default()
        };
        let out = styled(&doc, &style, vec![rect(100.0, 50.0)], 100.0, 50.0);
        assert_ne!(out[0].segs, rect(100.0, 50.0).segs);
        for s in &out[0].segs {
            if let PathCmd::L(x, y) | PathCmd::M(x, y) = *s {
                assert!((-0.01..=100.01).contains(&x) && (-0.01..=50.01).contains(&y));
            }
        }
    }
}

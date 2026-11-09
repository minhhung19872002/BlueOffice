//! Slide → display list (prompt §5.5). Coordinates in points, y down.

use crate::inherit::{
    chain, clr_map_of, effective_body, effective_props, show_master_shapes, theme_of, Level,
    TextStyleStack,
};
use crate::text_layout::{draw_text, TextEnv};
use crate::{PptxError, Result};
use bo_drawingml::color::{resolve, ColorContext};
use bo_drawingml::geom::{evaluate, rect_geometry, Evaluated, FillMode, Seg};
use bo_drawingml::props::parse_effects;
use bo_model::slide::{
    Background, BlipFill, Color, CommonSlideData, Effect, Fill, GraphicContent, GraphicFrame, Line,
    OuterShadow, Presentation, Rel, ShapeNode, ShapeStyle, Table, TextBody, Theme, Xfrm,
};
use bo_render::{Command, GradientStop, PageList, Paint, PathSeg, Rgba, Stroke};
use bo_text::FontManager;
use std::collections::BTreeMap;

const EMU_PT: f64 = 12_700.0;

/// Options for [`render_slide`].
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Skip text (thumbnail fast path).
    pub skip_text: bool,
    /// Draw only these top-level shapes of the slide (`cNvPr@id`), without
    /// background, master and layout shapes (clipboard pictures).
    pub only: Option<Vec<u32>>,
    /// Draw one layer of the slide show: the slide's top-level shapes
    /// `start..end` and, with `backdrop`, the background and the master and
    /// layout shapes under them (animated shapes get a layer of their own).
    pub layer: Option<ShapeRange>,
}

/// Top-level shapes `start..end` of a slide, optionally over its backdrop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShapeRange {
    /// Index of the first top-level shape drawn.
    pub start: usize,
    /// Index one past the last top-level shape drawn.
    pub end: usize,
    /// Also draw the backdrop (background, master and layout shapes).
    pub backdrop: bool,
}

/// 2D affine transform `[a, b, c, d, e, f]`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Aff([f64; 6]);

impl Aff {
    const ID: Aff = Aff([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fn translate(x: f64, y: f64) -> Aff {
        Aff([1.0, 0.0, 0.0, 1.0, x, y])
    }
    /// `self ∘ o` (apply `o` first).
    fn then(self, o: Aff) -> Aff {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = o.0;
        Aff([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }
    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
    fn is_translation(self) -> bool {
        let [a, b, c, d, _, _] = self.0;
        (a - 1.0).abs() < 1e-9 && b.abs() < 1e-9 && c.abs() < 1e-9 && (d - 1.0).abs() < 1e-9
    }
    /// Rotation (clockwise, 60000ths °) and flips about the centre of a w × h box.
    fn rot_flip(w: f64, h: f64, rot: i32, flip_h: bool, flip_v: bool) -> Aff {
        let ang = f64::from(rot) / 60_000.0 * std::f64::consts::PI / 180.0;
        let (s, c) = ang.sin_cos();
        let sx = if flip_h { -1.0 } else { 1.0 };
        let sy = if flip_v { -1.0 } else { 1.0 };
        // translate(center) · rotate · scale(flip) · translate(−center)
        Aff::translate(w / 2.0, h / 2.0)
            .then(Aff([c, s, -s, c, 0.0, 0.0]))
            .then(Aff([sx, 0.0, 0.0, sy, 0.0, 0.0]))
            .then(Aff::translate(-w / 2.0, -h / 2.0))
    }
    fn to_f32(self) -> [f32; 6] {
        self.0.map(|v| v as f32)
    }
}

/// Coordinate context for nested groups (§20.1.7.5 chOff/chExt).
#[derive(Clone, Copy)]
struct Frame {
    aff: Aff,
    /// child (x, y) → (off_x + (x − ch_x)·sx, off_y + (y − ch_y)·sy), in points.
    off: (f64, f64),
    ch: (f64, f64),
    scale: (f64, f64),
}

impl Frame {
    fn root() -> Frame {
        Frame {
            aff: Aff::ID,
            off: (0.0, 0.0),
            ch: (0.0, 0.0),
            scale: (1.0, 1.0),
        }
    }
    /// Map an xfrm to (x, y, w, h) in the frame's affine input space.
    fn rect(&self, x: &Xfrm) -> Option<(f64, f64, f64, f64)> {
        let (ox, oy) = x.off?;
        let (cx, cy) = x.ext?;
        let px = self.off.0 + (ox as f64 / EMU_PT - self.ch.0) * self.scale.0;
        let py = self.off.1 + (oy as f64 / EMU_PT - self.ch.1) * self.scale.1;
        Some((
            px,
            py,
            cx as f64 / EMU_PT * self.scale.0,
            cy as f64 / EMU_PT * self.scale.1,
        ))
    }
    /// Full transform of a shape box (local 0..w × 0..h) given its xfrm.
    fn shape_aff(&self, x: &Xfrm, r: (f64, f64, f64, f64)) -> Aff {
        self.aff.then(Aff::translate(r.0, r.1)).then(Aff::rot_flip(
            r.2,
            r.3,
            x.rot.unwrap_or(0),
            x.flip_h.unwrap_or(false),
            x.flip_v.unwrap_or(false),
        ))
    }
}

struct Ctx<'a> {
    pres: &'a Presentation,
    level: Level,
    theme: Option<&'a Theme>,
    clr_map: Vec<(String, String)>,
    rels: &'a [Rel],
    part: &'a str,
    slide_number: usize,
    opts: &'a RenderOptions,
}

impl Ctx<'_> {
    fn colors(&self, ph: Option<bo_drawingml::Rgba>) -> ColorContext<'_> {
        ColorContext {
            theme: self.theme,
            clr_map: &self.clr_map,
            ph_clr: ph,
        }
    }
    fn color(&self, c: &Color, ph: Option<bo_drawingml::Rgba>) -> Rgba {
        let [r, g, b, a] = resolve(c, &self.colors(ph)).to_u8();
        Rgba(r, g, b, a)
    }
    fn image_part(&self, rid: &str) -> Option<String> {
        self.rels
            .iter()
            .find(|r| r.id == rid && !r.external)
            .map(|r| bo_opc::resolve_target(self.part, &r.target))
    }
}

fn level_data(pres: &Presentation, level: Level) -> Option<(&CommonSlideData, &[Rel], &str)> {
    match level {
        Level::Slide(i) => pres
            .slides
            .get(i)
            .map(|s| (&s.data, s.info.rels.as_slice(), s.info.part_name.as_str())),
        Level::Layout(i) => pres
            .layouts
            .get(i)
            .map(|s| (&s.data, s.info.rels.as_slice(), s.info.part_name.as_str())),
        Level::Master(i) => pres
            .masters
            .get(i)
            .map(|s| (&s.data, s.info.rels.as_slice(), s.info.part_name.as_str())),
    }
}

/// Render slide `index` to a display list. Images referenced by the slide are
/// added to `images` (key = part name, used as the `image` id of `Image` commands).
pub fn render_slide(
    pres: &Presentation,
    index: usize,
    fonts: &mut FontManager,
    images: &mut BTreeMap<String, Vec<u8>>,
) -> Result<PageList> {
    render_with(pres, index, fonts, images, &RenderOptions::default())
}

/// Render with options.
pub fn render_with(
    pres: &Presentation,
    index: usize,
    fonts: &mut FontManager,
    images: &mut BTreeMap<String, Vec<u8>>,
    opts: &RenderOptions,
) -> Result<PageList> {
    let slide = pres
        .slides
        .get(index)
        .ok_or(PptxError::NoSuchSlide(index))?;
    let width = pres.slide_size.cx as f64 / EMU_PT;
    let height = pres.slide_size.cy as f64 / EMU_PT;
    let mut out = Vec::new();
    let layout = pres.layouts.get(slide.layout);
    let master_idx = layout.map(|l| l.master);
    let (show_layout, show_master) = show_master_shapes(pres, slide);

    let mut levels = Vec::new();
    if show_master {
        if let Some(m) = master_idx {
            levels.push(Level::Master(m));
        }
    }
    if show_layout && layout.is_some() {
        levels.push(Level::Layout(slide.layout));
    }
    levels.push(Level::Slide(index));

    // Background: nearest level that defines one (§19.3.1.1 bg).
    let mut bg_levels = vec![Level::Slide(index), Level::Layout(slide.layout)];
    if let Some(m) = master_idx {
        bg_levels.push(Level::Master(m));
    }
    let only = opts.only.as_deref();
    if only.is_some() || opts.layer.is_some_and(|l| !l.backdrop) {
        bg_levels.clear();
        levels.retain(|l| matches!(l, Level::Slide(_)));
    }
    for lv in bg_levels {
        let Some((data, rels, part)) = level_data(pres, lv) else {
            continue;
        };
        if let Some(bg) = &data.background {
            let cx = make_ctx(pres, Level::Slide(index), rels, part, index, opts);
            draw_background(&cx, bg, width, height, images, &mut out);
            break;
        }
    }

    for lv in levels {
        let Some((data, rels, part)) = level_data(pres, lv) else {
            continue;
        };
        let mut cx = make_ctx(pres, lv, rels, part, index, opts);
        // Master/layout shapes are drawn with the slide's colour mapping.
        cx.clr_map = clr_map_of(pres, Level::Slide(index));
        let is_slide = matches!(lv, Level::Slide(_));
        for (i, n) in data.shapes.iter().enumerate() {
            if !is_slide && is_placeholder(n) {
                continue;
            }
            if let (true, Some(l)) = (is_slide, opts.layer) {
                if !(l.start..l.end).contains(&i) {
                    continue;
                }
            }
            if let Some(ids) = only {
                if !n.nv().is_some_and(|nv| ids.contains(&nv.id)) {
                    continue;
                }
            }
            draw_node(&cx, n, Frame::root(), fonts, images, &mut out);
        }
    }
    Ok(PageList {
        width: width as f32,
        height: height as f32,
        commands: out,
    })
}

/// Render a slide scaled to `width` points (thumbnail).
pub fn render_thumbnail(
    pres: &Presentation,
    index: usize,
    fonts: &mut FontManager,
    images: &mut BTreeMap<String, Vec<u8>>,
    width: f32,
) -> Result<PageList> {
    let page = render_slide(pres, index, fonts, images)?;
    let k = if page.width > 0.0 {
        width / page.width
    } else {
        1.0
    };
    let mut commands = Vec::with_capacity(page.commands.len() + 4);
    commands.push(Command::PushClip {
        x: 0.0,
        y: 0.0,
        w: width,
        h: page.height * k,
    });
    commands.push(Command::PushTransform {
        m: [k, 0.0, 0.0, k, 0.0, 0.0],
    });
    commands.extend(page.commands);
    commands.push(Command::PopTransform);
    commands.push(Command::PopClip);
    Ok(PageList {
        width,
        height: page.height * k,
        commands,
    })
}

fn make_ctx<'a>(
    pres: &'a Presentation,
    level: Level,
    rels: &'a [Rel],
    part: &'a str,
    index: usize,
    opts: &'a RenderOptions,
) -> Ctx<'a> {
    Ctx {
        pres,
        level,
        theme: theme_of(pres, level),
        clr_map: clr_map_of(pres, level),
        rels,
        part,
        slide_number: index + 1,
        opts,
    }
}

fn is_placeholder(n: &ShapeNode) -> bool {
    n.nv().is_some_and(|nv| nv.placeholder.is_some())
}

fn draw_background(
    cx: &Ctx<'_>,
    bg: &Background,
    w: f64,
    h: f64,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let (fill, ph) = match bg {
        Background::Props { fill, .. } => (Some(fill.clone()), None),
        // §19.3.1.4 bgRef: idx 1001+ → bgFillStyleLst, 1..999 → fillStyleLst.
        Background::Ref { idx, color } => {
            let ph = color.as_ref().map(|c| resolve(c, &cx.colors(None)));
            let f = cx.theme.and_then(|t| {
                if *idx >= 1001 {
                    t.formats.bg_fill_styles.get(*idx as usize - 1001).cloned()
                } else if *idx >= 1 {
                    t.formats.fill_styles.get(*idx as usize - 1).cloned()
                } else {
                    None
                }
            });
            (f, ph)
        }
    };
    let Some(fill) = fill else {
        return;
    };
    let segs = rect_segs(Aff::ID, w, h);
    paint_fill(
        cx,
        &fill,
        ph,
        &segs,
        Aff::ID,
        (w, h),
        FillMode::Norm,
        images,
        out,
    );
}

fn rect_segs(aff: Aff, w: f64, h: f64) -> Vec<PathSeg> {
    let g = rect_geometry(w, h);
    to_segs(&g.paths[0].segs, aff)
}

fn to_segs(segs: &[Seg], aff: Aff) -> Vec<PathSeg> {
    segs.iter()
        .map(|s| match *s {
            Seg::Move(x, y) => {
                let (x, y) = aff.apply(x, y);
                PathSeg::M(x as f32, y as f32)
            }
            Seg::Line(x, y) => {
                let (x, y) = aff.apply(x, y);
                PathSeg::L(x as f32, y as f32)
            }
            Seg::Quad(a, b, x, y) => {
                let (a, b) = aff.apply(a, b);
                let (x, y) = aff.apply(x, y);
                PathSeg::Q(a as f32, b as f32, x as f32, y as f32)
            }
            Seg::Cubic(a, b, c, d, x, y) => {
                let (a, b) = aff.apply(a, b);
                let (c, d) = aff.apply(c, d);
                let (x, y) = aff.apply(x, y);
                PathSeg::C(a as f32, b as f32, c as f32, d as f32, x as f32, y as f32)
            }
            Seg::Close => PathSeg::Z,
        })
        .collect()
}

fn shade(c: Rgba, mode: FillMode) -> Rgba {
    // §20.1.10.37 ST_PathFillMode: lighten/darken the shape fill.
    let (k, toward): (f32, f32) = match mode {
        FillMode::Lighten => (0.4, 255.0),
        FillMode::LightenLess => (0.2, 255.0),
        FillMode::Darken => (0.4, 0.0),
        FillMode::DarkenLess => (0.2, 0.0),
        _ => return c,
    };
    let m = |v: u8| (f32::from(v) + (toward - f32::from(v)) * k).round() as u8;
    Rgba(m(c.0), m(c.1), m(c.2), c.3)
}

#[allow(clippy::too_many_arguments)]
fn paint_fill(
    cx: &Ctx<'_>,
    fill: &Fill,
    ph: Option<bo_drawingml::Rgba>,
    segs: &[PathSeg],
    aff: Aff,
    (w, h): (f64, f64),
    mode: FillMode,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let paint = match fill {
        Fill::None | Fill::Group => None,
        Fill::Solid(c) => Some(Paint::Solid {
            color: shade(cx.color(c, ph), mode),
        }),
        Fill::Gradient(g) => {
            let stops: Vec<GradientStop> = g
                .stops
                .iter()
                .map(|s| GradientStop {
                    offset: (f64::from(s.pos) / 100_000.0).clamp(0.0, 1.0) as f32,
                    color: shade(cx.color(&s.color, ph), mode),
                })
                .collect();
            let mut stops = stops;
            stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            match &g.shade {
                // §20.1.8.46 path gradient: approximated by a radial gradient.
                Some(bo_model::slide::GradientShade::Path { fill_to, .. }) => {
                    let [l, t, r, b] = fill_to.unwrap_or([50_000, 50_000, 50_000, 50_000]);
                    let fx = (f64::from(l) + (100_000.0 - f64::from(r))) / 200_000.0;
                    let fy = (f64::from(t) + (100_000.0 - f64::from(b))) / 200_000.0;
                    let (cxp, cyp) = aff.apply(w * fx, h * fy);
                    let rad = (w.max(h) / 2.0) * std::f64::consts::SQRT_2;
                    Some(Paint::Radial {
                        cx: cxp as f32,
                        cy: cyp as f32,
                        r: rad as f32,
                        stops,
                    })
                }
                // §20.1.8.41 lin: angle clockwise from the x axis.
                other => {
                    let ang = match other {
                        Some(bo_model::slide::GradientShade::Linear { ang, .. }) => *ang,
                        _ => 5_400_000,
                    };
                    let a = f64::from(ang) / 60_000.0 * std::f64::consts::PI / 180.0;
                    let (s, c) = a.sin_cos();
                    let half = f64::midpoint(w * c.abs(), h * s.abs());
                    let (x1, y1) = aff.apply(w / 2.0 - c * half, h / 2.0 - s * half);
                    let (x2, y2) = aff.apply(w / 2.0 + c * half, h / 2.0 + s * half);
                    Some(Paint::Linear {
                        x1: x1 as f32,
                        y1: y1 as f32,
                        x2: x2 as f32,
                        y2: y2 as f32,
                        stops,
                    })
                }
            }
        }
        // §20.1.8.47 pattFill: drawn as the blend of foreground and background.
        // §20.1.8.47 pattFill / §20.1.10.51 ST_PresetPatternVal: hatch presets are
        // drawn as clipped lines over the background; dot/percent presets as a blend.
        Fill::Pattern(p) => {
            let fg =
                p.fg.as_ref()
                    .map_or(Rgba(0, 0, 0, 255), |c| cx.color(c, ph));
            let bg =
                p.bg.as_ref()
                    .map_or(Rgba(255, 255, 255, 255), |c| cx.color(c, ph));
            pattern_fill(p.preset.as_deref().unwrap_or("pct50"), fg, bg, segs, out);
            None
        }
        Fill::Picture(b) => {
            out.push(Command::PushClipPath {
                segs: segs.to_vec(),
            });
            draw_image(cx, b, aff, w, h, images, out);
            out.push(Command::PopClip);
            None
        }
    };
    if let Some(p) = paint {
        out.push(Command::Path {
            segs: segs.to_vec(),
            fill: Some(p),
            stroke: None,
        });
    }
}

/// Blend `fg` over `bg` with coverage `k` (0..1).
fn blend(fg: Rgba, bg: Rgba, k: f64) -> Rgba {
    let m = |a: u8, b: u8| (f64::from(b) + (f64::from(a) - f64::from(b)) * k).round() as u8;
    Rgba(m(fg.0, bg.0), m(fg.1, bg.1), m(fg.2, bg.2), fg.3.max(bg.3))
}

fn pattern_fill(preset: &str, fg: Rgba, bg: Rgba, segs: &[PathSeg], out: &mut Vec<Command>) {
    // (horizontal, vertical, down-diagonal, up-diagonal, spacing pt, width pt)
    let hatch = match preset {
        "horz" => Some((true, false, false, false, 4.0, 0.75)),
        "ltHorz" | "narHorz" => Some((true, false, false, false, 3.0, 0.5)),
        "dkHorz" => Some((true, false, false, false, 4.0, 1.5)),
        "wdHorz" | "dashHorz" => Some((true, false, false, false, 6.0, 0.75)),
        "vert" => Some((false, true, false, false, 4.0, 0.75)),
        "ltVert" | "narVert" => Some((false, true, false, false, 3.0, 0.5)),
        "dkVert" => Some((false, true, false, false, 4.0, 1.5)),
        "wdVert" | "dashVert" => Some((false, true, false, false, 6.0, 0.75)),
        "dnDiag" | "ltDnDiag" | "dashDnDiag" => Some((false, false, true, false, 4.0, 0.6)),
        "dkDnDiag" | "wdDnDiag" => Some((false, false, true, false, 4.0, 1.5)),
        "upDiag" | "ltUpDiag" | "dashUpDiag" => Some((false, false, false, true, 4.0, 0.6)),
        "dkUpDiag" | "wdUpDiag" => Some((false, false, false, true, 4.0, 1.5)),
        "cross" | "smGrid" | "lgGrid" => Some((true, true, false, false, 4.0, 0.6)),
        "diagCross" | "openDmnd" => Some((false, false, true, true, 4.0, 0.6)),
        _ => None,
    };
    let Some((h, v, dn, up, gap, width)) = hatch else {
        // Percent / dotted presets: average coverage.
        let k = preset
            .strip_prefix("pct")
            .and_then(|n| n.parse::<f64>().ok())
            .map_or(0.5, |n| n / 100.0);
        out.push(Command::Path {
            segs: segs.to_vec(),
            fill: Some(Paint::Solid {
                color: blend(fg, bg, k),
            }),
            stroke: None,
        });
        return;
    };
    out.push(Command::Path {
        segs: segs.to_vec(),
        fill: Some(Paint::Solid { color: bg }),
        stroke: None,
    });
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for s in segs {
        let pts: &[(f32, f32)] = &match *s {
            PathSeg::M(x, y) | PathSeg::L(x, y) => [(x, y), (x, y), (x, y)],
            PathSeg::Q(a, c, x, y) => [(a, c), (x, y), (x, y)],
            PathSeg::C(a, c, d, e, x, y) => [(a, c), (d, e), (x, y)],
            PathSeg::Z => continue,
        };
        for (x, y) in pts {
            b = [b[0].min(*x), b[1].min(*y), b[2].max(*x), b[3].max(*y)];
        }
    }
    if b[0] > b[2] {
        return;
    }
    let (x0, y0, x1, y1) = (
        f64::from(b[0]),
        f64::from(b[1]),
        f64::from(b[2]),
        f64::from(b[3]),
    );
    let mut lines: Vec<PathSeg> = Vec::new();
    let mut seg = |ax: f64, ay: f64, bx: f64, by: f64| {
        lines.push(PathSeg::M(ax as f32, ay as f32));
        lines.push(PathSeg::L(bx as f32, by as f32));
    };
    let span = (x1 - x0) + (y1 - y0);
    let n = ((span / gap).ceil() as usize).min(2000);
    for i in 0..=n {
        let d = i as f64 * gap;
        if h && y0 + d <= y1 {
            seg(x0, y0 + d, x1, y0 + d);
        }
        if v && x0 + d <= x1 {
            seg(x0 + d, y0, x0 + d, y1);
        }
        if dn {
            seg(x0 + d - (y1 - y0), y0, x0 + d, y1);
        }
        if up {
            seg(x0 + d - (y1 - y0), y1, x0 + d, y0);
        }
    }
    out.push(Command::PushClipPath {
        segs: segs.to_vec(),
    });
    out.push(Command::Path {
        segs: lines,
        fill: None,
        stroke: Some(Stroke {
            width: width as f32,
            color: fg,
            dash: vec![],
            cap: "butt".into(),
            join: "miter".into(),
        }),
    });
    out.push(Command::PopClip);
}

fn draw_image(
    cx: &Ctx<'_>,
    b: &BlipFill,
    aff: Aff,
    w: f64,
    h: f64,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let Some(part) = b.embed.as_deref().and_then(|r| cx.image_part(r)) else {
        return;
    };
    if !images.contains_key(&part) {
        if let Some(data) = cx.pres.part_data(&part) {
            images.insert(part.clone(), data.to_vec());
        }
    }
    // §20.1.8.55 srcRect: fractions of the image cropped from each edge.
    let crop = b
        .src_rect
        .map(|r| r.map(|v| (f64::from(v) / 100_000.0) as f32))
        .filter(|r| r.iter().any(|v| *v != 0.0));
    // §20.1.8.4 alphaModFix@amt: constant opacity of the picture.
    let alpha = b
        .blip_children
        .iter()
        .find(|e| e.local() == "alphaModFix")
        .map(|e| {
            e.attr("amt")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(100_000.0)
        })
        .map(|amt| (amt / 100_000.0).clamp(0.0, 1.0) as f32)
        .filter(|a| *a < 1.0);
    if let Some(alpha) = alpha {
        out.push(Command::PushOpacity { alpha });
    }
    let push = !aff.is_translation();
    let (x, y) = if push {
        out.push(Command::PushTransform { m: aff.to_f32() });
        (0.0, 0.0)
    } else {
        aff.apply(0.0, 0.0)
    };
    out.push(Command::Image {
        x: x as f32,
        y: y as f32,
        w: w as f32,
        h: h as f32,
        image: part,
        crop,
        rotation: 0.0,
    });
    if push {
        out.push(Command::PopTransform);
    }
    if alpha.is_some() {
        out.push(Command::PopOpacity);
    }
}

/// Theme style matrix lookups (§20.1.4.2 style references).
fn style_fill(cx: &Ctx<'_>, st: Option<&ShapeStyle>) -> (Option<Fill>, Option<bo_drawingml::Rgba>) {
    let Some(st) = st else {
        return (None, None);
    };
    let ph = st
        .fill_ref
        .color
        .as_ref()
        .map(|c| resolve(c, &cx.colors(None)));
    let idx = st.fill_ref.idx as usize;
    let f = cx.theme.and_then(|t| {
        if idx >= 1001 {
            t.formats.bg_fill_styles.get(idx - 1001).cloned()
        } else if idx >= 1 {
            t.formats.fill_styles.get(idx - 1).cloned()
        } else {
            None
        }
    });
    (f, ph)
}

fn effective_line(
    cx: &Ctx<'_>,
    own: Option<&Line>,
    st: Option<&ShapeStyle>,
) -> (Option<Line>, Option<bo_drawingml::Rgba>) {
    let mut base: Option<Line> = None;
    let mut ph = None;
    if let Some(st) = st {
        if st.ln_ref.idx >= 1 {
            base = cx.theme.and_then(|t| {
                t.formats
                    .line_styles
                    .get(st.ln_ref.idx as usize - 1)
                    .cloned()
            });
            ph = st
                .ln_ref
                .color
                .as_ref()
                .map(|c| resolve(c, &cx.colors(None)));
        }
    }
    match (base, own) {
        (None, None) => (None, ph),
        (Some(b), None) => (Some(b), ph),
        (None, Some(o)) => (Some(o.clone()), ph),
        (Some(mut b), Some(o)) => {
            if o.w.is_some() {
                b.w = o.w;
            }
            if o.fill.is_some() {
                b.fill.clone_from(&o.fill);
            }
            if o.dash.is_some() {
                b.dash.clone_from(&o.dash);
            }
            if o.cap.is_some() {
                b.cap.clone_from(&o.cap);
            }
            if o.join.is_some() {
                b.join.clone_from(&o.join);
            }
            if o.head.is_some() {
                b.head.clone_from(&o.head);
            }
            if o.tail.is_some() {
                b.tail.clone_from(&o.tail);
            }
            (Some(b), ph)
        }
    }
}

fn make_stroke(
    cx: &Ctx<'_>,
    l: &Line,
    ph: Option<bo_drawingml::Rgba>,
    scale: f64,
) -> Option<Stroke> {
    let color = match &l.fill {
        Some(Fill::Solid(c)) => cx.color(c, ph),
        Some(Fill::Gradient(g)) => g.stops.first().map(|s| cx.color(&s.color, ph))?,
        Some(Fill::Pattern(p)) => p.fg.as_ref().map(|c| cx.color(c, ph))?,
        _ => return None,
    };
    let width = (l.w.unwrap_or(9_525) as f64 / EMU_PT * scale).max(0.25);
    // §20.1.10.48 ST_PresetLineDashVal, lengths in multiples of the line width.
    let pattern: &[f64] = match l.dash.as_deref() {
        Some("dot") => &[1.0, 3.0],
        Some("dash") => &[4.0, 3.0],
        Some("lgDash") => &[8.0, 3.0],
        Some("dashDot") => &[4.0, 3.0, 1.0, 3.0],
        Some("lgDashDot") => &[8.0, 3.0, 1.0, 3.0],
        Some("lgDashDotDot") => &[8.0, 3.0, 1.0, 3.0, 1.0, 3.0],
        Some("sysDash") => &[3.0, 1.0],
        Some("sysDot") => &[1.0, 1.0],
        Some("sysDashDot") => &[3.0, 1.0, 1.0, 1.0],
        Some("sysDashDotDot") => &[3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        _ => &[],
    };
    Some(Stroke {
        width: width as f32,
        color,
        dash: pattern.iter().map(|v| (v * width) as f32).collect(),
        cap: match l.cap.as_deref() {
            Some("rnd") => "round",
            Some("sq") => "square",
            _ => "butt",
        }
        .into(),
        join: l.join.clone().unwrap_or_else(|| "round".into()),
    })
}

fn shadow_of(
    cx: &Ctx<'_>,
    effects: Option<&Vec<Effect>>,
    st: Option<&ShapeStyle>,
) -> Option<OuterShadow> {
    let from_list = |v: &[Effect]| {
        v.iter().find_map(|e| match e {
            Effect::OuterShadow(s) => Some(s.clone()),
            Effect::Glow(_) | Effect::Other(_) => None,
        })
    };
    if let Some(e) = effects {
        return from_list(e);
    }
    let st = st?;
    if st.effect_ref.idx == 0 {
        return None;
    }
    let es = cx
        .theme?
        .formats
        .effect_styles
        .get(st.effect_ref.idx as usize - 1)?;
    let lst = es.child("a:effectLst")?;
    from_list(&parse_effects(lst))
}

/// §20.1.8.32 glow of a shape (its effect list, else its style's effect).
fn glow_of(
    cx: &Ctx<'_>,
    effects: Option<&Vec<Effect>>,
    st: Option<&ShapeStyle>,
) -> Option<bo_model::slide::Glow> {
    let from_list = |v: &[Effect]| {
        v.iter().find_map(|e| match e {
            Effect::Glow(g) => Some(g.clone()),
            _ => None,
        })
    };
    if let Some(e) = effects {
        return from_list(e);
    }
    let st = st?;
    if st.effect_ref.idx == 0 {
        return None;
    }
    let es = cx
        .theme?
        .formats
        .effect_styles
        .get(st.effect_ref.idx as usize - 1)?;
    from_list(&parse_effects(es.child("a:effectLst")?))
}

/// §20.1.8.32 glow: a soft band of the glow colour `rad` wide around the shape's
/// outline, behind the shape (the outline grown by half the radius, blurred by it).
fn draw_glow(
    cx: &Ctx<'_>,
    glow: Option<&bo_model::slide::Glow>,
    geom: &Evaluated,
    aff: Aff,
    out: &mut Vec<Command>,
) {
    let Some(g) = glow else { return };
    let rad = g.rad.unwrap_or(0) as f64 / EMU_PT;
    if rad <= 0.0 {
        return;
    }
    let color = g
        .color
        .as_ref()
        .map_or(Rgba(255, 192, 0, 160), |c| cx.color(c, None));
    for p in geom
        .paths
        .iter()
        .filter(|p| p.fill != FillMode::None || p.stroke)
    {
        out.push(Command::BlurPath {
            segs: to_segs(&p.segs, aff),
            color,
            radius: rad as f32,
            spread: (rad / 2.0) as f32,
        });
    }
}

/// §20.1.8.49 outerShdw: offset copy, blurred by blurRad.
fn draw_shadow(cx: &Ctx<'_>, s: &OuterShadow, geom: &Evaluated, aff: Aff, out: &mut Vec<Command>) {
    let dist = s.dist.unwrap_or(0) as f64 / EMU_PT;
    let dir = f64::from(s.dir.unwrap_or(0)) / 60_000.0 * std::f64::consts::PI / 180.0;
    let off = Aff::translate(dist * dir.cos(), dist * dir.sin()).then(aff);
    let color = s
        .color
        .as_ref()
        .map_or(Rgba(0, 0, 0, 96), |c| cx.color(c, None));
    let blur = s.blur_rad.unwrap_or(0) as f64 / EMU_PT;
    for p in geom.paths.iter().filter(|p| p.fill != FillMode::None) {
        let segs = to_segs(&p.segs, off);
        out.push(if blur > 0.0 {
            Command::BlurPath {
                segs,
                color,
                radius: blur as f32,
                spread: 0.0,
            }
        } else {
            Command::Path {
                segs,
                fill: Some(Paint::Solid { color }),
                stroke: None,
            }
        });
    }
}

/// A 3-D shape (ECMA-376 Part 1, §20.1.5) as a real solid ([`crate::mesh3d`]): the outline polygons
/// extruded by `extrusionH`, bevelled by `bevelT`, turned and projected by the camera and shaded by the
/// light rig and material. Returns the layer under the face and the layer over it, and the transform
/// of the (turned) front face. A solid fill is drawn by the mesh itself (the shaded front face); other
/// fills are painted by the caller on the front face between the two layers.
fn draw_solid(
    cx: &Ctx<'_>,
    solid: &crate::three_d::Solid,
    geom: &Evaluated,
    aff: Aff,
    size: (f64, f64),
    fill: Option<(&Fill, Option<bo_drawingml::Rgba>)>,
) -> (Vec<Command>, Vec<Command>, Aff) {
    let (w, h) = size;
    let [a, b, c, d] = solid.face();
    let face = aff
        .then(Aff::translate(w / 2.0, h / 2.0))
        .then(Aff([a, b, c, d, 0.0, 0.0]))
        .then(Aff::translate(-w / 2.0, -h / 2.0));
    let base = match fill {
        Some((Fill::Solid(c), ph)) => Some(cx.color(c, ph)),
        Some((Fill::Gradient(g), ph)) => g.stops.first().map(|s| cx.color(&s.color, ph)),
        _ => None,
    };
    let solid_fill = matches!(fill, Some((Fill::Solid(_), _)));
    let side = solid
        .extrusion_color
        .as_ref()
        .map(|c| cx.color(c, None))
        .or(base)
        .unwrap_or(Rgba(96, 96, 96, 255));
    let polys: Vec<Vec<(f64, f64)>> = geom
        .paths
        .iter()
        .filter(|p| p.fill != FillMode::None)
        .flat_map(|p| crate::mesh3d::polygons(&p.segs))
        .collect();
    let model = crate::mesh3d::Model {
        depth: solid.depth,
        bevel: solid.bevel.clone(),
        bevel_bottom: solid.bevel_bottom.clone(),
        face: base.unwrap_or(side),
        side,
        contour: (solid.contour > 0.0).then(|| {
            (
                solid.contour,
                solid
                    .contour_color
                    .as_ref()
                    .map_or(Rgba(255, 255, 255, 255), |c| cx.color(c, None)),
            )
        }),
        light: solid.light,
        specular: solid.specular,
        front: solid_fill,
    };
    let (behind, over) =
        crate::mesh3d::render(&polys, size, &solid.view_for(w, h), &model, &|x, y| {
            aff.apply(x, y)
        });
    (behind, over, face)
}

#[allow(clippy::too_many_arguments)]
fn draw_geometry(
    cx: &Ctx<'_>,
    geom: &Evaluated,
    aff: Aff,
    size: (f64, f64),
    fill: Option<(&Fill, Option<bo_drawingml::Rgba>)>,
    line: Option<(&Line, Option<bo_drawingml::Rgba>)>,
    shadow: Option<&OuterShadow>,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let has_fill = fill.is_some_and(|(f, _)| !matches!(f, Fill::None | Fill::Group));
    if let (Some(s), true) = (shadow, has_fill) {
        draw_shadow(cx, s, geom, aff, out);
    }
    let stroke = line.and_then(|(l, ph)| make_stroke(cx, l, ph, 1.0));
    for p in &geom.paths {
        let segs = to_segs(&p.segs, aff);
        if let (Some((f, ph)), true) = (fill, p.fill != FillMode::None) {
            paint_fill(cx, f, ph, &segs, aff, size, p.fill, images, out);
        }
        if p.stroke {
            if let Some(s) = &stroke {
                out.push(Command::Path {
                    segs: segs.clone(),
                    fill: None,
                    stroke: Some(s.clone()),
                });
            }
        }
    }
    if let (Some((l, _)), Some(s)) = (line, &stroke) {
        if let Some(first) = geom.paths.iter().find(|p| p.stroke) {
            arrowheads(l, s, &first.segs, aff, out);
        }
    }
}

/// §20.1.8.38 headEnd / §20.1.8.57 tailEnd as filled triangles.
fn arrowheads(l: &Line, s: &Stroke, segs: &[Seg], aff: Aff, out: &mut Vec<Command>) {
    let pts: Vec<(f64, f64)> = segs
        .iter()
        .filter_map(|s| match *s {
            Seg::Move(x, y)
            | Seg::Line(x, y)
            | Seg::Quad(_, _, x, y)
            | Seg::Cubic(_, _, _, _, x, y) => Some(aff.apply(x, y)),
            Seg::Close => None,
        })
        .collect();
    if pts.len() < 2 {
        return;
    }
    let size = |e: &bo_model::slide::LineEnd| {
        let f = |v: Option<&str>| match v {
            Some("sm") => 2.0,
            Some("lg") => 5.0,
            _ => 3.0,
        };
        (
            f(e.w.as_deref()) * f64::from(s.width).max(1.0),
            f(e.len.as_deref()) * f64::from(s.width).max(1.0),
        )
    };
    let mut draw = |e: &bo_model::slide::LineEnd, tip: (f64, f64), from: (f64, f64)| {
        if matches!(e.kind.as_deref(), None | Some("none")) {
            return;
        }
        let (w, len) = size(e);
        let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 1e-6 {
            return;
        }
        let (ux, uy) = (dx / d, dy / d);
        let base = (tip.0 - ux * len, tip.1 - uy * len);
        let (nx, ny) = (-uy * w / 2.0, ux * w / 2.0);
        out.push(Command::Path {
            segs: vec![
                PathSeg::M(tip.0 as f32, tip.1 as f32),
                PathSeg::L((base.0 + nx) as f32, (base.1 + ny) as f32),
                PathSeg::L((base.0 - nx) as f32, (base.1 - ny) as f32),
                PathSeg::Z,
            ],
            fill: Some(Paint::Solid { color: s.color }),
            stroke: None,
        });
    };
    if let Some(t) = &l.tail {
        draw(t, pts[pts.len() - 1], pts[pts.len() - 2]);
    }
    if let Some(h) = &l.head {
        draw(h, pts[0], pts[1]);
    }
}

fn draw_node(
    cx: &Ctx<'_>,
    node: &ShapeNode,
    frame: Frame,
    fonts: &mut FontManager,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let mut warnings = Vec::new();
    match node {
        ShapeNode::Shape(s) => {
            if s.nv.hidden == Some(true) {
                return;
            }
            let ch = chain(cx.pres, cx.level, s);
            let props = effective_props(&ch);
            let Some(x) = props.xfrm.clone() else {
                return;
            };
            let Some(r) = frame.rect(&x) else {
                return;
            };
            let aff = frame.shape_aff(&x, r);
            let geom = evaluate(props.geometry.as_ref(), r.2, r.3, &mut warnings);
            let style = ch.iter().find_map(|s| s.style.as_ref());
            let (sfill, fph) = style_fill(cx, style);
            let fill = props.fill.clone().or(sfill);
            let (line, lph) = effective_line(cx, props.line.as_ref(), style);
            let shadow = shadow_of(cx, props.effects.as_ref(), style);
            let glow = glow_of(cx, props.effects.as_ref(), style);
            draw_glow(cx, glow.as_ref(), &geom, aff, out);
            // §20.1.5 3-D: shadow, then the solid (sides, face, bevel, contour).
            let solid = crate::three_d::solid_of(&props.extra);
            match &solid {
                Some(sd) => {
                    let has_fill = fill
                        .as_ref()
                        .is_some_and(|f| !matches!(f, Fill::None | Fill::Group));
                    if let (Some(sh), true) = (shadow.as_ref(), has_fill) {
                        draw_shadow(cx, sh, &geom, aff, out);
                    }
                    let fill = fill.as_ref().map(|f| (f, fph));
                    let (behind, over, face) = draw_solid(cx, sd, &geom, aff, (r.2, r.3), fill);
                    out.extend(behind);
                    let solid_fill = matches!(fill, Some((Fill::Solid(_), _)));
                    if !solid_fill {
                        draw_geometry(cx, &geom, face, (r.2, r.3), fill, None, None, images, out);
                    }
                    out.extend(over);
                    draw_geometry(
                        cx,
                        &geom,
                        face,
                        (r.2, r.3),
                        None,
                        line.as_ref().map(|l| (l, lph)),
                        None,
                        images,
                        out,
                    );
                }
                None => draw_geometry(
                    cx,
                    &geom,
                    aff,
                    (r.2, r.3),
                    fill.as_ref().map(|f| (f, fph)),
                    line.as_ref().map(|l| (l, lph)),
                    shadow.as_ref(),
                    images,
                    out,
                ),
            }
            if !cx.opts.skip_text {
                if let Some(tb) = &s.text {
                    // Text follows the shape rotation but is never mirrored: a vertical
                    // flip turns it upside down, a horizontal flip leaves it as is.
                    let text_rot = x.rot.unwrap_or(0)
                        + if x.flip_v == Some(true) {
                            10_800_000
                        } else {
                            0
                        };
                    let text_aff = frame
                        .aff
                        .then(Aff::translate(r.0, r.1))
                        .then(Aff::rot_flip(r.2, r.3, text_rot, false, false));
                    // The text lies on the (turned) front face of a 3-D shape.
                    let text_aff = match &solid {
                        Some(sd) => {
                            let [a, b, c, d] = sd.face();
                            text_aff
                                .then(Aff::translate(r.2 / 2.0, r.3 / 2.0))
                                .then(Aff([a, b, c, d, 0.0, 0.0]))
                                .then(Aff::translate(-r.2 / 2.0, -r.3 / 2.0))
                        }
                        None => text_aff,
                    };
                    draw_shape_text(cx, &ch, tb, &geom, text_aff, fonts, out);
                }
            }
        }
        ShapeNode::Connector(c) => {
            let Some(x) = c.props.xfrm.clone() else {
                return;
            };
            let Some(r) = frame.rect(&x) else {
                return;
            };
            let aff = frame.shape_aff(&x, r);
            let geom = evaluate(c.props.geometry.as_ref(), r.2, r.3, &mut warnings);
            let (line, lph) = effective_line(cx, c.props.line.as_ref(), c.style.as_ref());
            draw_geometry(
                cx,
                &geom,
                aff,
                (r.2, r.3),
                None,
                line.as_ref().map(|l| (l, lph)),
                None,
                images,
                out,
            );
        }
        ShapeNode::Picture(p) => {
            let Some(x) = p.props.xfrm.clone() else {
                return;
            };
            let Some(r) = frame.rect(&x) else {
                return;
            };
            let aff = frame.shape_aff(&x, r);
            let geom = evaluate(p.props.geometry.as_ref(), r.2, r.3, &mut warnings);
            let is_rect = match &p.props.geometry {
                None => true,
                Some(bo_model::slide::Geometry::Preset { name, .. }) => name == "rect",
                Some(_) => false,
            };
            let glow = glow_of(cx, p.props.effects.as_ref(), p.style.as_ref());
            draw_glow(cx, glow.as_ref(), &geom, aff, out);
            if is_rect {
                draw_image(cx, &p.blip, aff, r.2, r.3, images, out);
            } else if let Some(path) = geom.paths.first() {
                out.push(Command::PushClipPath {
                    segs: to_segs(&path.segs, aff),
                });
                draw_image(cx, &p.blip, aff, r.2, r.3, images, out);
                out.push(Command::PopClip);
            }
            let (line, lph) = effective_line(cx, p.props.line.as_ref(), p.style.as_ref());
            draw_geometry(
                cx,
                &geom,
                aff,
                (r.2, r.3),
                None,
                line.as_ref().map(|l| (l, lph)),
                None,
                images,
                out,
            );
        }
        ShapeNode::Group(g) => {
            let Some(x) = g.props.xfrm.as_ref() else {
                for c in &g.children {
                    draw_node(cx, c, frame, fonts, images, out);
                }
                return;
            };
            let Some(r) = frame.rect(x) else {
                return;
            };
            let (chx, chy) = x.ch_off.unwrap_or(x.off.unwrap_or((0, 0)));
            let (chw, chh) = x.ch_ext.unwrap_or(x.ext.unwrap_or((0, 0)));
            let sx = if chw > 0 {
                r.2 / (chw as f64 / EMU_PT)
            } else {
                1.0
            };
            let sy = if chh > 0 {
                r.3 / (chh as f64 / EMU_PT)
            } else {
                1.0
            };
            let inner = Frame {
                aff: frame.aff.then(Aff::translate(r.0, r.1)).then(Aff::rot_flip(
                    r.2,
                    r.3,
                    x.rot.unwrap_or(0),
                    x.flip_h.unwrap_or(false),
                    x.flip_v.unwrap_or(false),
                )),
                off: (0.0, 0.0),
                ch: (chx as f64 / EMU_PT, chy as f64 / EMU_PT),
                scale: (sx, sy),
            };
            for c in &g.children {
                draw_node(cx, c, inner, fonts, images, out);
            }
        }
        ShapeNode::Frame(f) => draw_frame(cx, f, frame, fonts, images, out),
        // mc:AlternateContent (ECMA-376 Part 3, §10.2.1): a chartex choice is drawn
        // ([MS-ODRAWXML] 2.24), otherwise the fallback content.
        ShapeNode::Other(el) if el.local() == "AlternateContent" => {
            let chartex = |e: &bo_model::slide::XmlElement| {
                e.local() == "graphicFrame"
                    && e.elements()
                        .find(|g| g.local() == "graphic")
                        .and_then(|g| g.elements().find(|d| d.local() == "graphicData"))
                        .and_then(|d| d.attr("uri"))
                        == Some(bo_xlsx::chartex::CX_NS)
            };
            let choice = el
                .elements()
                .filter(|c| c.local() == "Choice")
                .find_map(|c| c.elements().find(|e| chartex(e)));
            // A shape whose text holds equations (a14:m): drawn with the math layout.
            let math_choice: Option<Vec<&bo_model::slide::XmlElement>> = el
                .elements()
                .filter(|c| c.local() == "Choice")
                .find(|c| c.elements().any(has_math))
                .map(|c| c.elements().collect());
            let nodes: Vec<&bo_model::slide::XmlElement> = match (choice, math_choice) {
                (Some(frame), _) => vec![frame],
                (None, Some(nodes)) => nodes,
                (None, None) => el
                    .elements()
                    .find(|c| c.local() == "Fallback")
                    .map(|f| f.elements().collect())
                    .unwrap_or_default(),
            };
            for e in nodes {
                draw_node(cx, &crate::parse_node(e), frame, fonts, images, out);
            }
        }
        ShapeNode::Other(_) => {}
    }
}

/// An element holding a paragraph with an equation (`a:p/a14:m`, [MS-ODRAWXML]).
fn has_math(e: &bo_model::slide::XmlElement) -> bool {
    (e.local() == "p" && e.elements().any(|c| c.local() == "m")) || e.elements().any(has_math)
}

fn draw_shape_text(
    cx: &Ctx<'_>,
    ch: &[&bo_model::slide::Shape],
    tb: &TextBody,
    geom: &Evaluated,
    aff: Aff,
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
) {
    let bp = effective_body(ch);
    let stack = TextStyleStack::new(cx.pres, cx.level, ch);
    let env = TextEnv {
        theme: cx.theme,
        colors: cx.colors(None),
        slide_number: cx.slide_number,
    };
    let mut cmds = Vec::new();
    // Text rotation follows the shape unless bodyPr@upright is set.
    draw_text(tb, &bp, &stack, geom.text_rect, &env, fonts, &mut cmds);
    wrap_transform(aff, cmds, out);
}

/// Caret geometry of a slide shape's text laid out in a `w × h` box (local
/// points, origin at the box's top-left corner, before rotation) — exactly as
/// [`render_slide`] draws it (§21.1.2 text body, inherited styles and body properties).
pub fn shape_text_geometry(
    pres: &Presentation,
    slide: usize,
    shape: &bo_model::slide::Shape,
    w: f64,
    h: f64,
    fonts: &mut FontManager,
) -> Option<crate::text_layout::TextGeometry> {
    let level = Level::Slide(slide);
    let empty = TextBody::default();
    let tb = shape.text.as_ref().unwrap_or(&empty);
    let ch = chain(pres, level, shape);
    let props = effective_props(&ch);
    let mut warnings = Vec::new();
    let geom = evaluate(props.geometry.as_ref(), w, h, &mut warnings);
    let bp = effective_body(&ch);
    let stack = TextStyleStack::new(pres, level, &ch);
    let clr_map = clr_map_of(pres, level);
    let env = TextEnv {
        theme: theme_of(pres, level),
        colors: ColorContext {
            theme: theme_of(pres, level),
            clr_map: &clr_map,
            ph_clr: None,
        },
        slide_number: slide + 1,
    };
    Some(crate::text_layout::text_geometry(
        tb,
        &bp,
        &stack,
        geom.text_rect,
        &env,
        fonts,
    ))
}

/// Text style layers of table cells: presentation defaults and the master's
/// `p:otherStyle` (§19.3.1.35), as drawn by [`draw_table`].
fn table_text_stack(pres: &Presentation, level: Level) -> TextStyleStack {
    TextStyleStack {
        layers: pres
            .default_text_style
            .iter()
            .cloned()
            .chain(
                crate::inherit::master_of(pres, level)
                    .and_then(|m| pres.masters.get(m))
                    .and_then(|m| m.text_styles.other.clone()),
            )
            .collect(),
        font_ref: None,
        master_layers: 0,
    }
}

/// Cell text with the table style's text colour/bold/italic and its body properties
/// with the cell margins and anchor (§21.1.3.17 tcPr), as drawn by [`draw_table`].
fn cell_text_body(
    pres: &Presentation,
    t: &Table,
    ri: usize,
    ci: usize,
) -> Option<(TextBody, bo_model::slide::BodyProps)> {
    let rows = t.rows.len();
    let cols = t.grid.len();
    let cell = t.rows.get(ri)?.cells.get(ci)?;
    let mut props = cell.props.clone().unwrap_or_default();
    let (flags, style_id) = crate::table_style::table_props(t.props.as_ref());
    let style = style_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(|id| crate::table_style::resolve(pres, id));
    let mut text_style = (None, None, None);
    if let Some(st) = &style {
        let (fill, color, bold, italic, _) =
            crate::table_style::cell_style(st, flags, ri, ci, rows, cols);
        if props.fill.is_none() {
            props.fill = fill;
        }
        text_style = (color, bold, italic);
    }
    let empty = TextBody::default();
    let tb = cell.text.as_ref().unwrap_or(&empty);
    let tb = if text_style == (None, None, None) {
        tb.clone()
    } else {
        styled_text(tb, &text_style)
    };
    let mut bp = tb.body_pr.clone();
    bp.l_ins = Some(props.mar_l.unwrap_or(91_440));
    bp.r_ins = Some(props.mar_r.unwrap_or(91_440));
    bp.t_ins = Some(props.mar_t.unwrap_or(45_720));
    bp.b_ins = Some(props.mar_b.unwrap_or(45_720));
    bp.anchor.clone_from(&props.anchor);
    Some((tb, bp))
}

/// Caret geometry of the text of table cell (`row`, `col`) (its merge anchor) and
/// the cell's box `[x, y, w, h]` in points from the table's top-left corner; the
/// geometry is relative to that box (same layout as the rendered cell).
pub fn table_cell_text_geometry(
    pres: &Presentation,
    slide: usize,
    t: &Table,
    row: usize,
    col: usize,
    fonts: &mut FontManager,
) -> Option<(crate::text_layout::TextGeometry, [f64; 4])> {
    let level = Level::Slide(slide);
    let cols: Vec<f64> = t.grid.iter().map(|c| c.w as f64 / EMU_PT).collect();
    let mut col_x = vec![0.0];
    for w in &cols {
        col_x.push(col_x.last().copied().unwrap_or(0.0) + w);
    }
    let rows: Vec<f64> = t.rows.iter().map(|r| r.h as f64 / EMU_PT).collect();
    let mut row_y = vec![0.0];
    for h in &rows {
        row_y.push(row_y.last().copied().unwrap_or(0.0) + h);
    }
    let cell = t.rows.get(row)?.cells.get(col)?;
    if col >= cols.len() {
        return None;
    }
    let span = cell.grid_span.unwrap_or(1).max(1) as usize;
    let rspan = cell.row_span.unwrap_or(1).max(1) as usize;
    let (x0, x1) = (col_x[col], col_x[(col + span).min(cols.len())]);
    let (y0, y1) = (row_y[row], row_y[(row + rspan).min(rows.len())]);
    let (w, h) = (x1 - x0, y1 - y0);
    let (tb, bp) = cell_text_body(pres, t, row, col)?;
    let stack = table_text_stack(pres, level);
    let clr_map = clr_map_of(pres, level);
    let env = TextEnv {
        theme: theme_of(pres, level),
        colors: ColorContext {
            theme: theme_of(pres, level),
            clr_map: &clr_map,
            ph_clr: None,
        },
        slide_number: slide + 1,
    };
    let g = crate::text_layout::text_geometry(&tb, &bp, &stack, [0.0, 0.0, w, h], &env, fonts);
    Some((g, [x0, y0, w, h]))
}

fn wrap_transform(aff: Aff, cmds: Vec<Command>, out: &mut Vec<Command>) {
    if cmds.is_empty() {
        return;
    }
    out.push(Command::PushTransform { m: aff.to_f32() });
    out.extend(cmds);
    out.push(Command::PopTransform);
}

fn draw_frame(
    cx: &Ctx<'_>,
    f: &GraphicFrame,
    frame: Frame,
    fonts: &mut FontManager,
    images: &mut BTreeMap<String, Vec<u8>>,
    out: &mut Vec<Command>,
) {
    let Some(r) = frame.rect(&f.xfrm) else {
        return;
    };
    let aff = frame.shape_aff(&f.xfrm, r);
    let mut cmds = Vec::new();
    match &f.content {
        GraphicContent::Table(t) => draw_table(cx, t, fonts, &mut cmds),
        // §21.2.2.27 chart reference → chart part (cached values).
        GraphicContent::Other(gd)
            if f.uri == bo_drawingml::chart::URI_CHART || f.uri == bo_xlsx::chartex::CX_NS =>
        {
            let chart = bo_drawingml::chart::chart_rel_id(gd)
                .and_then(|rid| cx.image_part(rid))
                .and_then(|part| {
                    let data = cx.pres.part_data(&part)?;
                    bo_xlsx::chart_write::parse_chart_part(data, &part)
                });
            if let Some(chart) = chart {
                draw_chart_frame(cx, &chart, r.2, r.3, fonts, &mut cmds);
            }
        }
        // SmartArt (§21.4.2.6 dgm:relIds): the pre-rendered drawing part
        // ([MS-ODRAWXML] dsp:drawing), shapes positioned inside the frame.
        GraphicContent::Other(gd) if f.uri == crate::diagram::URI_DIAGRAM => {
            let ext = f.xfrm.ext.unwrap_or((0, 0));
            let shapes = diagram_fallback(cx, gd).or_else(|| diagram_from_data(cx, gd, ext));
            for node in shapes.unwrap_or_default() {
                draw_node(cx, &node, Frame::root(), fonts, images, &mut cmds);
            }
        }
        GraphicContent::Other(_) => {}
    }
    wrap_transform(aff, cmds, out);
}

/// Shapes of the drawing fallback of a SmartArt frame: data part (`r:dm`) →
/// `dsp:dataModelExt@relId`, or else the part's only diagramDrawing relationship.
fn diagram_fallback(cx: &Ctx<'_>, gd: &bo_model::slide::XmlElement) -> Option<Vec<ShapeNode>> {
    let data_part = crate::diagram::data_rel_id(gd).and_then(|rid| cx.image_part(rid));
    let by_ext = data_part.as_deref().and_then(|p| {
        let rid = crate::diagram::drawing_rel_id(cx.pres.part_data(p)?, p)?;
        cx.image_part(&rid)
    });
    let part = by_ext.or_else(|| {
        let mut it = cx
            .rels
            .iter()
            .filter(|r| r.rel_type == crate::diagram::REL_DIAGRAM_DRAWING && !r.external);
        match (it.next(), it.next()) {
            (Some(r), None) => Some(bo_opc::resolve_target(cx.part, &r.target)),
            _ => None,
        }
    })?;
    crate::diagram::drawing_shapes(cx.pres.part_data(&part)?, &part)
}

/// A SmartArt without a drawing part laid out from its data model (`crate::diagram_layout`), in the
/// frame's `ext` (EMU).
fn diagram_from_data(
    cx: &Ctx<'_>,
    gd: &bo_model::slide::XmlElement,
    ext: (i64, i64),
) -> Option<Vec<ShapeNode>> {
    let data_part = crate::diagram::data_rel_id(gd).and_then(|rid| cx.image_part(rid))?;
    let layout = crate::diagram::layout_rel_id(gd)
        .and_then(|rid| cx.image_part(rid))
        .and_then(|p| cx.pres.part_data(&p));
    crate::diagram_layout::synth(
        cx.pres.part_data(&data_part)?,
        &data_part,
        layout,
        ext.0,
        ext.1,
    )
}

/// A chart part drawn by the shared chart renderer (`bo_sheet::chart`, the same as BlueSheet's) from
/// its cached values (an empty workbook: no reference resolves), series colours from the theme
/// (accent1…accent6 by default, ECMA-376 Part 1, §21.2.2.150 ser spPr).
fn draw_chart_frame(
    cx: &Ctx<'_>,
    chart: &bo_model::sheet::Chart,
    w: f64,
    h: f64,
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
) {
    use bo_model::sheet::ChartColor;
    use bo_model::slide::{ParaProps, Paragraph, RunProps, TextRun};
    let env = TextEnv {
        theme: cx.theme,
        colors: cx.colors(None),
        slide_number: cx.slide_number,
    };
    let stack = TextStyleStack::default();
    let color = |c: Option<&ChartColor>, i: usize| -> Rgba {
        match c {
            Some(ChartColor::Rgb(v)) => cx.color(&Color::srgb(*v), None),
            Some(ChartColor::Scheme(name)) => cx.color(&Color::scheme(name), None),
            // Default series colours cycle through accent1..accent6 (§20.1.6.2 clrScheme).
            None => cx.color(&Color::scheme(&format!("accent{}", i % 6 + 1)), None),
        }
    };
    let mut label = |text: &str,
                     rect: [f64; 4],
                     align: bo_sheet::chart::Align,
                     size: f64,
                     ink: Rgba,
                     out: &mut Vec<Command>| {
        let algn = match align {
            bo_sheet::chart::Align::Left => "l",
            bo_sheet::chart::Align::Right => "r",
            bo_sheet::chart::Align::Center => "ctr",
        };
        let Rgba(r, g, b, _) = ink;
        let tb = TextBody {
            body_pr: bo_model::slide::BodyProps {
                l_ins: Some(0),
                t_ins: Some(0),
                r_ins: Some(0),
                b_ins: Some(0),
                anchor: Some("ctr".into()),
                wrap: Some("none".into()),
                ..Default::default()
            },
            list_style: None,
            paragraphs: vec![Paragraph {
                props: Some(ParaProps {
                    algn: Some(algn.into()),
                    ..ParaProps::default()
                }),
                runs: vec![TextRun::Run {
                    props: RunProps {
                        sz: Some((size * 100.0) as i32),
                        fill: Some(Fill::Solid(Color::srgb(
                            (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
                        ))),
                        ..RunProps::default()
                    },
                    text: text.to_owned(),
                }],
                end_props: None,
            }],
        };
        let bp = tb.body_pr.clone();
        draw_text(&tb, &bp, &stack, rect, &env, fonts, out);
    };
    // No sheets: references never resolve, the cached values are drawn.
    let wb = bo_model::sheet::Workbook::default();
    bo_sheet::chart::draw_chart_with(
        &wb,
        0,
        chart,
        w,
        h,
        &bo_sheet::numfmt::Locale::default(),
        &color,
        false,
        &mut label,
        out,
    );
}

/// §21.1.3 table: cell fills, borders and text, with the table style (§20.1.4.2.26).
fn draw_table(cx: &Ctx<'_>, t: &Table, fonts: &mut FontManager, out: &mut Vec<Command>) {
    let cols: Vec<f64> = t.grid.iter().map(|c| c.w as f64 / EMU_PT).collect();
    let mut col_x = vec![0.0];
    for w in &cols {
        col_x.push(col_x.last().copied().unwrap_or(0.0) + w);
    }
    let rows: Vec<f64> = t.rows.iter().map(|r| r.h as f64 / EMU_PT).collect();
    let mut row_y = vec![0.0];
    for h in &rows {
        row_y.push(row_y.last().copied().unwrap_or(0.0) + h);
    }
    let env = TextEnv {
        theme: cx.theme,
        colors: cx.colors(None),
        slide_number: cx.slide_number,
    };
    let stack = TextStyleStack {
        layers: cx
            .pres
            .default_text_style
            .iter()
            .cloned()
            .chain(
                crate::inherit::master_of(cx.pres, cx.level)
                    .and_then(|m| cx.pres.masters.get(m))
                    .and_then(|m| m.text_styles.other.clone()),
            )
            .collect(),
        font_ref: None,
        master_layers: 0,
    };
    let mut images = BTreeMap::new();
    // §21.1.3.15 tblPr: table style (tableStyleId) and banding flags.
    let (flags, style_id) = crate::table_style::table_props(t.props.as_ref());
    let style = style_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(|id| crate::table_style::resolve(cx.pres, id));
    for (ri, row) in t.rows.iter().enumerate() {
        for (this, cell) in row.cells.iter().enumerate() {
            let span = cell.grid_span.unwrap_or(1).max(1) as usize;
            let rspan = cell.row_span.unwrap_or(1).max(1) as usize;
            if cell.h_merge == Some(true) || cell.v_merge == Some(true) || this >= cols.len() {
                continue;
            }
            let x0 = col_x[this];
            let x1 = col_x[(this + span).min(cols.len())];
            let y0 = row_y[ri];
            let y1 = row_y[(ri + rspan).min(rows.len())];
            let (w, h) = (x1 - x0, y1 - y0);
            let aff = Aff::translate(x0, y0);
            let mut props = cell.props.clone().unwrap_or_default();
            // Style values fill in what the cell does not set itself.
            let mut text_style = (None, None, None);
            if let Some(st) = &style {
                let (fill, color, bold, italic, b) =
                    crate::table_style::cell_style(st, flags, ri, this, rows.len(), cols.len());
                if props.fill.is_none() {
                    props.fill = fill;
                }
                let [l, r, tp, bt] = b;
                props.ln_l = props.ln_l.or(l);
                props.ln_r = props.ln_r.or(r);
                props.ln_t = props.ln_t.or(tp);
                props.ln_b = props.ln_b.or(bt);
                text_style = (color, bold, italic);
            }
            if let Some(fill) = &props.fill {
                let segs = rect_segs(aff, w, h);
                paint_fill(
                    cx,
                    fill,
                    None,
                    &segs,
                    aff,
                    (w, h),
                    FillMode::Norm,
                    &mut images,
                    out,
                );
            }
            let edges = [
                (&props.ln_l, (0.0, 0.0), (0.0, h)),
                (&props.ln_r, (w, 0.0), (w, h)),
                (&props.ln_t, (0.0, 0.0), (w, 0.0)),
                (&props.ln_b, (0.0, h), (w, h)),
            ];
            for (ln, a, b) in edges {
                if let Some(s) = ln.as_ref().and_then(|l| make_stroke(cx, l, None, 1.0)) {
                    let (ax, ay) = aff.apply(a.0, a.1);
                    let (bx, by) = aff.apply(b.0, b.1);
                    out.push(Command::Path {
                        segs: vec![
                            PathSeg::M(ax as f32, ay as f32),
                            PathSeg::L(bx as f32, by as f32),
                        ],
                        fill: None,
                        stroke: Some(s),
                    });
                }
            }
            if let Some(tb) = &cell.text {
                let styled;
                let tb = if text_style == (None, None, None) {
                    tb
                } else {
                    styled = styled_text(tb, &text_style);
                    &styled
                };
                // §21.1.3.17 tcPr margins (defaults 0.1" / 0.05").
                let mut bp = tb.body_pr.clone();
                bp.l_ins = Some(props.mar_l.unwrap_or(91_440));
                bp.r_ins = Some(props.mar_r.unwrap_or(91_440));
                bp.t_ins = Some(props.mar_t.unwrap_or(45_720));
                bp.b_ins = Some(props.mar_b.unwrap_or(45_720));
                bp.anchor.clone_from(&props.anchor);
                let mut cmds = Vec::new();
                draw_text(tb, &bp, &stack, [0.0, 0.0, w, h], &env, fonts, &mut cmds);
                wrap_transform(aff, cmds, out);
            }
        }
    }
}

/// Apply a table style's text colour / bold / italic (tcTxStyle) to runs that
/// do not set them.
fn styled_text(
    tb: &TextBody,
    (color, bold, italic): &(Option<Color>, Option<bool>, Option<bool>),
) -> TextBody {
    use bo_model::slide::{RunProps, TextRun};
    let apply = |p: &mut RunProps| {
        if p.fill.is_none() {
            if let Some(c) = color {
                p.fill = Some(Fill::Solid(c.clone()));
            }
        }
        if p.b.is_none() {
            p.b = *bold;
        }
        if p.i.is_none() {
            p.i = *italic;
        }
    };
    let mut out = tb.clone();
    for para in &mut out.paragraphs {
        for r in &mut para.runs {
            match r {
                TextRun::Run { props, .. }
                | TextRun::Break { props }
                | TextRun::Field { props, .. } => apply(props),
                TextRun::Other(_) => {}
            }
        }
        if let Some(p) = &mut para.end_props {
            apply(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::Aff;

    #[test]
    fn rotation_about_centre() {
        // 90° clockwise about the centre of a 100 × 50 box.
        let a = Aff::rot_flip(100.0, 50.0, 5_400_000, false, false);
        let (x, y) = a.apply(50.0, 25.0);
        assert!((x - 50.0).abs() < 1e-9 && (y - 25.0).abs() < 1e-9);
        // The top-left corner maps to (75, −25) (clockwise in y-down space).
        let (x, y) = a.apply(0.0, 0.0);
        assert!(
            (x - 75.0).abs() < 1e-9 && (y + 25.0).abs() < 1e-9,
            "{x} {y}"
        );
        let f = Aff::rot_flip(100.0, 50.0, 0, true, false);
        let (x, _) = f.apply(10.0, 0.0);
        assert!((x - 90.0).abs() < 1e-9);
    }
}

//! Shapes and text boxes (ECMA-376 Part 1, §20.1.9 shape geometry; §17.17.1
//! txbxContent): the outline comes from the preset geometry engine of `bo-drawingml`,
//! the text is stacked like a table cell inside the text rectangle minus the body
//! insets (§21.1.2.1.1 bodyPr lIns/tIns/rIns/bIns, anchor).

use crate::blocks::{stack_blocks, Ctx};
use crate::types::{Layer, Line, LineSource, PageItem, PathCmd, ShapePath};
use crate::LayoutError;
use bo_drawingml::geom::{eval_preset, rect_geometry, FillMode, Seg};
use bo_model::slide::Guide;
use bo_model::{Image, Shape, ShapeArt, TextAnchor, TextPath};
use bo_text::PathCmd as OutlineCmd;
use std::sync::Arc;

/// What a shape draws, relative to the top-left corner of its box.
#[derive(Debug, Clone, Default)]
pub(crate) struct Drawing {
    pub items: Vec<PageItem>,
    /// Text box lines ([`LineSource::TextBox`] with `para` 0: relative to the paragraph
    /// holding the shape, rebased like body lines).
    pub lines: Vec<Line>,
}

impl Drawing {
    /// Items and lines moved to (`x`, `y`); text box lines get body paragraph `para`
    /// (or [`LineSource::NO_PARA`]).
    pub fn placed(&self, x: f32, y: f32, para: Option<u32>) -> (Vec<PageItem>, Vec<Line>) {
        let items = self
            .items
            .iter()
            .cloned()
            .map(|mut i| {
                i.translate(x, y);
                if let Some(LineSource::TextBox { para: p, .. }) = i.source_mut() {
                    *p = para.unwrap_or(LineSource::NO_PARA);
                }
                i
            })
            .collect();
        let lines = self
            .lines
            .iter()
            .cloned()
            .map(|mut l| {
                l.translate(x, y);
                if let LineSource::TextBox { para: p, .. } = &mut l.source {
                    *p = para.unwrap_or(LineSource::NO_PARA);
                }
                l
            })
            .collect();
        (items, lines)
    }
}

/// An anchored (floating) drawing object of a paragraph.
#[derive(Debug, Clone)]
pub(crate) struct Float {
    pub img: Image,
    /// Its shape / text box, when it is not a picture.
    pub drawing: Option<Arc<Drawing>>,
    /// Relative body paragraph id of the anchor paragraph (text box lines).
    pub para: Option<u32>,
}

/// Paint layer of a drawing object (`behindDoc`, §20.4.2.3).
pub(crate) fn layer_of(img: &Image) -> Layer {
    if img.anchor.as_ref().is_some_and(|a| a.behind) {
        Layer::Back
    } else {
        Layer::Front
    }
}

/// Lay out a shape of `img` (anchored at index offset `at` of its paragraph).
pub(crate) fn shape_drawing(
    ctx: &mut Ctx<'_>,
    img: &Image,
    shape: &Shape,
    at: u32,
) -> Result<Drawing, LayoutError> {
    let (w, h) = (
        img.width.to_pt().max(0.0) as f64,
        img.height.to_pt().max(0.0) as f64,
    );
    let av: Vec<Guide> = shape
        .adjust
        .iter()
        .map(|(n, v)| Guide {
            name: n.clone(),
            fmla: format!("val {v}"),
        })
        .collect();
    // §20.1.9.18 prstGeom (unknown presets and errors: the bounding rectangle).
    let geom = eval_preset(&shape.preset, &av, w, h)
        .and_then(Result::ok)
        .unwrap_or_else(|| rect_geometry(w, h));
    // §20.1.7.6 xfrm: flips, then rotation about the box centre.
    let (cx, cy) = (w / 2.0, h / 2.0);
    let rot = f64::from(img.rotation) / 60_000.0 * std::f64::consts::PI / 180.0;
    let (sin, cos) = rot.sin_cos();
    let tf = |x: f64, y: f64| -> (f32, f32) {
        let x = if shape.flip_h { w - x } else { x };
        let y = if shape.flip_v { h - y } else { y };
        let (dx, dy) = (x - cx, y - cy);
        (
            (cx + dx * cos - dy * sin) as f32,
            (cy + dx * sin + dy * cos) as f32,
        )
    };
    if let Some(line) = shape.art.as_ref().and_then(|a| a.signature_line.as_deref()) {
        return Ok(crate::sigline::signature_line_drawing(
            ctx.fonts, img, line, at,
        ));
    }
    if let Some(art) = &shape.art {
        return Ok(art_drawing(ctx, img, shape, art, at, &tf));
    }
    let mut paths = Vec::new();
    for p in &geom.paths {
        let segs: Vec<PathCmd> = p
            .segs
            .iter()
            .map(|s| match *s {
                Seg::Move(x, y) => {
                    let (x, y) = tf(x, y);
                    PathCmd::M(x, y)
                }
                Seg::Line(x, y) => {
                    let (x, y) = tf(x, y);
                    PathCmd::L(x, y)
                }
                Seg::Quad(a, b, x, y) => {
                    let ((a, b), (x, y)) = (tf(a, b), tf(x, y));
                    PathCmd::Q(a, b, x, y)
                }
                Seg::Cubic(a, b, c, d, x, y) => {
                    let ((a, b), (c, d), (x, y)) = (tf(a, b), tf(c, d), tf(x, y));
                    PathCmd::C(a, b, c, d, x, y)
                }
                Seg::Close => PathCmd::Z,
            })
            .collect();
        let fill = if p.fill == FillMode::None {
            None
        } else {
            shape.fill
        };
        let stroke = if p.stroke {
            shape
                .line
                .as_ref()
                .map(|l| (l.width.to_pt().max(0.25), l.color, l.dash.clone()))
        } else {
            None
        };
        if fill.is_some() || stroke.is_some() {
            paths.push(ShapePath { segs, fill, stroke });
        }
    }
    let mut out = Drawing::default();
    if !paths.is_empty() {
        out.items.push(PageItem::Shape {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
            paths,
            source: LineSource::TextBox {
                para: 0,
                at,
                sub: 0,
            },
            layer: layer_of(img),
            alpha: 1.0,
        });
    }
    let Some(blocks) = &shape.text else {
        return Ok(out);
    };
    // Text rectangle of the geometry (§20.1.9.22 rect) minus the insets.
    let [l, t, r, b] = geom.text_rect.map(|v| v as f32);
    let [il, it, ir, ib] = shape.insets.map(bo_model::Emu::to_pt);
    let (x0, y0) = (l.min(r) + il, t.min(b) + it);
    let inner_w = ((r - l).abs() - il - ir).max(1.0);
    let inner_h = ((b - t).abs() - it - ib).max(0.0);
    let saved = (ctx.textbox.replace((at, 0)), ctx.other_lists.clone());
    let st = stack_blocks(ctx, blocks, inner_w, false, None);
    ctx.textbox = saved.0;
    ctx.other_lists = saved.1;
    let st = st?;
    let dy = match shape.anchor {
        TextAnchor::Top => 0.0,
        TextAnchor::Center => ((inner_h - st.h) / 2.0).max(0.0),
        TextAnchor::Bottom => (inner_h - st.h).max(0.0),
    };
    for (top, band) in &st.bands {
        let band = band.translated(x0, y0 + dy + top);
        out.lines.extend(band.lines);
        out.items.extend(band.items);
    }
    Ok(out)
}

/// Shape with decorative content (VML watermarks): a text path stretched over the
/// box (ECMA-376 Part 4, §19.1.2.23 textpath `fitshape`) drawn as glyph outlines
/// filled with the shape fill, or a picture fill (§19.1.2.11 imagedata) drawn as a
/// picture; both with the fill opacity (§19.1.2.5 fill `opacity`).
fn art_drawing(
    ctx: &mut Ctx<'_>,
    img: &Image,
    shape: &Shape,
    art: &ShapeArt,
    at: u32,
    tf: &dyn Fn(f64, f64) -> (f32, f32),
) -> Drawing {
    let (w, h) = (img.width.to_pt().max(0.0), img.height.to_pt().max(0.0));
    let mut out = Drawing::default();
    let mut alpha = f32::from(art.opacity.min(1000)) / 1000.0;
    if let Some(media) = &art.picture {
        // A washed-out picture is drawn faintly over the page.
        if art.washout {
            alpha *= WASHOUT_ALPHA;
        }
        out.items.push(PageItem::Image {
            x: 0.0,
            y: 0.0,
            w,
            h,
            media: media.clone(),
            crop: None,
            rotation: crate::blocks::rotation_of(img),
            layer: layer_of(img),
            alpha,
        });
        return out;
    }
    let (Some(t), Some(fill)) = (&art.text, shape.fill) else {
        return out;
    };
    let segs = text_path_outline(ctx, t, f64::from(w), f64::from(h), tf);
    if segs.is_empty() {
        return out;
    }
    out.items.push(PageItem::Shape {
        x: 0.0,
        y: 0.0,
        w,
        h,
        paths: vec![ShapePath {
            segs,
            fill: Some(fill),
            stroke: None,
        }],
        source: LineSource::TextBox {
            para: 0,
            at,
            sub: 0,
        },
        layer: layer_of(img),
        alpha,
    });
    out
}

/// Opacity of a washed-out picture (VML imagedata gain / blacklevel).
pub(crate) const WASHOUT_ALPHA: f32 = 0.3;

/// Glyph outlines of a text path stretched to fill the `w`×`h` box: the advance of
/// the text spans the width and ascender-to-descender spans the height; `tf` maps
/// box coordinates (flips, rotation).
pub(crate) fn text_path_outline(
    ctx: &mut Ctx<'_>,
    t: &TextPath,
    w: f64,
    h: f64,
    tf: &dyn Fn(f64, f64) -> (f32, f32),
) -> Vec<PathCmd> {
    let family = if t.font.is_empty() {
        "Calibri"
    } else {
        t.font.as_str()
    };
    let Some(face) = ctx
        .fonts
        .resolve(family, t.bold, t.italic)
        .or_else(|| ctx.fonts.resolve("Calibri", t.bold, t.italic))
    else {
        return Vec::new();
    };
    let text = t.text.replace(['\r', '\n'], " ");
    let glyphs = ctx.fonts.shape(face, &text);
    let info = ctx.fonts.info(face);
    let (asc, desc) = (f64::from(info.ascender), f64::from(info.descender));
    let advance: f64 = glyphs.iter().map(|g| f64::from(g.x_advance)).sum();
    if advance <= 0.0 || asc - desc <= 0.0 {
        return Vec::new();
    }
    let (sx, sy) = (w / advance, h / (asc - desc));
    let mut segs = Vec::new();
    let mut pen = 0.0f64;
    for g in glyphs.iter() {
        let ox = pen + f64::from(g.x_offset);
        let oy = f64::from(g.y_offset);
        // Font units, y up → box points, y down from the top of the box.
        let p = |x: f32, y: f32| tf((ox + f64::from(x)) * sx, (asc - oy - f64::from(y)) * sy);
        for c in ctx.fonts.outline(face, g.glyph_id) {
            segs.push(match c {
                OutlineCmd::MoveTo(x, y) => {
                    let (x, y) = p(x, y);
                    PathCmd::M(x, y)
                }
                OutlineCmd::LineTo(x, y) => {
                    let (x, y) = p(x, y);
                    PathCmd::L(x, y)
                }
                OutlineCmd::QuadTo(a, b, x, y) => {
                    let ((a, b), (x, y)) = (p(a, b), p(x, y));
                    PathCmd::Q(a, b, x, y)
                }
                OutlineCmd::CubicTo(a, b, c, d, x, y) => {
                    let ((a, b), (c, d), (x, y)) = (p(a, b), p(c, d), p(x, y));
                    PathCmd::C(a, b, c, d, x, y)
                }
                OutlineCmd::Close => PathCmd::Z,
            });
        }
        pen += f64::from(g.x_advance);
    }
    segs
}

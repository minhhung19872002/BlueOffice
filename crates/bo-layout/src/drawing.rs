//! Shapes and text boxes (ECMA-376 Part 1, §20.1.9 shape geometry; §17.17.1
//! txbxContent): the outline comes from the preset geometry engine of `bo-drawingml`,
//! the text is stacked like a table cell inside the text rectangle minus the body
//! insets (§21.1.2.1.1 bodyPr lIns/tIns/rIns/bIns, anchor).

use crate::blocks::{stack_blocks, Ctx};
use crate::types::{Layer, Line, LineSource, PageItem, PathCmd, ShapePath};
use crate::LayoutError;
use bo_drawingml::geom::{eval_preset, rect_geometry, FillMode, Seg};
use bo_model::slide::Guide;
use bo_model::{Image, Shape, TextAnchor};
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
            layer: layer_of(img),
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

//! Text editing inside a shape on the canvas: the text selection, caret and
//! selection geometry (from the renderer's own layout, `bo_pptx::shape_text_geometry`)
//! and auto-fit while typing (ECMA-376 Part 1, §21.1.2.1.2 spAutoFit,
//! §21.1.2.1.3 normAutofit).

use super::geom::{BoxR, Placed};
use super::text;
use bo_model::slide::{Autofit, Presentation, Shape, ShapeNode};
use bo_pptx::inherit::{chain, effective_body, Level};
use bo_pptx::TextGeometry;
use bo_text::FontManager;
use serde::Serialize;

/// Text selection of the shape being edited (character offsets of the editing text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSel {
    pub id: u32,
    pub start: usize,
    pub end: usize,
    /// Table cell (row, grid column) whose text is edited, in table `id`.
    pub cell: Option<(usize, usize)>,
}

/// Text editing state sent to the UI (offsets in UTF-16 code units).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEditOut {
    pub id: u32,
    pub start: usize,
    pub end: usize,
    /// Caret line `[x1, y1, x2, y2]` in slide points (collapsed selections).
    pub caret: Option<[f64; 4]>,
    /// Selection highlight, one box per line (slide points).
    pub rects: Vec<BoxR>,
    /// Box of the shape.
    pub rect: BoxR,
    /// Text in the editing format.
    pub text: String,
    /// Table cell `[row, col]` being edited (the rect is the cell's box).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<[usize; 2]>,
}

/// Box in which the text is laid out: the shape box turned upside down when
/// the shape is flipped vertically (text is never mirrored, §20.1.7.6 xfrm).
pub fn text_box(p: &Placed, node: &ShapeNode) -> BoxR {
    let flip_v = match node {
        ShapeNode::Shape(s) => s
            .props
            .xfrm
            .as_ref()
            .and_then(|x| x.flip_v)
            .unwrap_or(false),
        _ => false,
    };
    BoxR {
        rot: if flip_v { p.b.rot + 180.0 } else { p.b.rot },
        ..p.b
    }
}

/// Caret geometry of a shape's text in its box.
pub fn geometry(
    pres: &Presentation,
    slide: usize,
    s: &Shape,
    b: &BoxR,
    fonts: &mut FontManager,
) -> Option<TextGeometry> {
    bo_pptx::shape_text_geometry(pres, slide, s, b.w, b.h, fonts)
}

fn local_to_slide(b: &BoxR, x: f64, y: f64) -> (f64, f64) {
    b.to_slide(x - b.w / 2.0, y - b.h / 2.0)
}

/// Caret and selection boxes for `[start, end)` (character offsets).
pub fn selection_shapes(
    g: &TextGeometry,
    b: &BoxR,
    start: usize,
    end: usize,
) -> (Option<[f64; 4]>, Vec<BoxR>) {
    let (s, e) = (start.min(end), start.max(end));
    if s == e {
        let caret = g.caret(s).and_then(|(li, x)| {
            let l = g.lines.get(li)?;
            let (x1, y1) = local_to_slide(b, x, l.top);
            let (x2, y2) = local_to_slide(b, x, l.top + l.height);
            Some([x1, y1, x2, y2])
        });
        return (caret, Vec::new());
    }
    let mut rects = Vec::new();
    for l in &g.lines {
        if e <= l.start || s > l.end || (s == l.end && e > l.end) {
            continue;
        }
        let a = s.max(l.start) - l.start;
        let z = e.min(l.end) - l.start;
        let x0 = l.xs.get(a).copied().unwrap_or(0.0);
        let mut x1 = l.xs.get(z).copied().unwrap_or(x0);
        if e > l.end {
            // The selection continues on the next line: show the line end.
            x1 += 4.0;
        }
        if x1 <= x0 {
            continue;
        }
        let (cx, cy) = local_to_slide(b, f64::midpoint(x0, x1), l.top + l.height / 2.0);
        rects.push(BoxR {
            cx,
            cy,
            w: x1 - x0,
            h: l.height,
            rot: b.rot,
        });
    }
    (None, rects)
}

/// Character offset nearest to slide point (x, y).
pub fn offset_at(g: &TextGeometry, b: &BoxR, x: f64, y: f64) -> usize {
    let (lx, ly) = b.to_local(x, y);
    g.offset_at(lx + b.w / 2.0, ly + b.h / 2.0)
}

/// Auto-fit a shape after its text changed: `spAutoFit` sets the height to the
/// text (top edge fixed), `normAutofit` stores the font scale and line spacing
/// reduction the text needs. Returns the new box when it changed.
pub fn autofit(
    pres: &Presentation,
    slide: usize,
    s: &mut Shape,
    b: &BoxR,
    fonts: &mut FontManager,
) -> Option<BoxR> {
    let ch = chain(pres, Level::Slide(slide), s);
    let body = effective_body(&ch);
    match body.autofit {
        Some(Autofit::Shape) => {
            let g = bo_pptx::shape_text_geometry(pres, slide, s, b.w, b.h, fonts)?;
            let h = g.content_height.max(1.0);
            if (h - b.h).abs() < 0.5 {
                return None;
            }
            let (cx, cy) = b.to_slide(0.0, (h - b.h) / 2.0);
            Some(BoxR { cx, cy, h, ..*b })
        }
        Some(Autofit::Normal { .. }) => {
            let g = bo_pptx::shape_text_geometry(pres, slide, s, b.w, b.h, fonts)?;
            let pct = |v: f64| -> Option<i32> {
                let p = (v * 100_000.0).round() as i32;
                (p != 100_000 && p != 0).then_some(p)
            };
            let font_scale = pct(g.font_scale);
            let red = (g.ln_spc_reduction * 100_000.0).round() as i32;
            let fit = Autofit::Normal {
                font_scale,
                ln_spc_reduction: (red > 0).then_some(red),
            };
            if let Some(tb) = &mut s.text {
                if tb.body_pr.autofit.as_ref() != Some(&fit) {
                    tb.body_pr.autofit = Some(fit);
                }
            }
            None
        }
        _ => None,
    }
}

/// UTF-16 offsets of a text selection.
pub fn to_utf16(text_s: &str, sel: TextSel) -> (usize, usize) {
    (
        text::utf16_offset(text_s, sel.start),
        text::utf16_offset(text_s, sel.end),
    )
}

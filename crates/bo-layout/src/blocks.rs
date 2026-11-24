//! Block building: paragraphs → line bands, tables → row bands, and unpaged
//! stacking of block lists (table cells, headers/footers, notes).
//!
//! A [`Band`] is a vertical slice of content that is never split across pages
//! (one line of a paragraph, one row of a nested table). Coordinates inside a
//! band are relative to its top-left corner (x = 0 is the left edge of the
//! content box the band was laid out for).

use crate::drawing::Float;
use crate::paragraph::{layout_paragraph, FieldContext, LayoutCache, ParaInput, RelLine, WrapZone};
use crate::style::{para_props, ListCounters};
use crate::types::{Layer, Line, LineRotation, LineSource, NoteKey, PageItem};
use crate::{LayoutError, RevisionView};
use bo_model::{
    Block, Border, Borders, CellPlace, CellVAlign, Color, Document, HeightRule, Image, ParaProps,
    Paragraph, RunContent, Table, TableGeometry, TableView, TableWidth, Twips, VMerge,
};
use bo_text::FontManager;
use std::collections::HashMap;

/// Default left/right cell margin (ECMA-376 Part 1, §17.4.42 tblCellMar; application default 0.08").
pub(crate) const DEFAULT_CELL_MARGIN: f32 = 5.4;

/// Shared layout state.
pub(crate) struct Ctx<'a> {
    pub doc: &'a Document,
    pub fonts: &'a mut FontManager,
    pub cache: &'a mut LayoutCache,
    /// List counters of the main story.
    pub lists: ListCounters,
    /// List counters of headers, footers and notes.
    pub other_lists: ListCounters,
    pub notes: HashMap<NoteKey, String>,
    pub fields: FieldContext,
    pub default_tab: f32,
    /// Next body paragraph id (document order, see `bo_model::index::body_paragraphs`).
    pub next_id: u32,
    /// Id of the first paragraph of the top-level block being built: body lines carry
    /// ids relative to it so a built block can be placed again under another id.
    pub id_origin: u32,
    /// Endnote ids in order of first reference.
    pub endnote_order: Vec<i32>,
    /// How tracked changes are shown.
    pub view: RevisionView,
    /// Markup filter of the all-markup view (see [`crate::MarkFilter`]).
    pub marks: crate::MarkFilter,
    /// Note being stacked and its next paragraph index (note lines carry
    /// [`LineSource::Note`]).
    pub note: Option<(NoteKey, u32)>,
    /// Text box being stacked: (index offset of the shape in its paragraph, next
    /// paragraph index); its lines carry [`LineSource::TextBox`].
    pub textbox: Option<(u32, u32)>,
}

/// Break requested after a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Brk {
    None,
    Page,
    Column,
}

/// Unsplittable slice of content.
#[derive(Debug, Clone)]
pub(crate) struct Band {
    pub h: f32,
    /// Height that must stay clear of top-and-bottom wrapped objects (§20.4.2.19
    /// wrapTopAndBottom): `h` but for lines whose extra line spacing may overlap them.
    pub text_h: f32,
    pub lines: Vec<Line>,
    pub items: Vec<PageItem>,
    pub notes: Vec<NoteKey>,
    pub brk: Brk,
    /// A text line of a paragraph with widow control (§17.3.1.44 widowControl): a
    /// table row split keeps at least two of its lines on each side.
    pub widow_control: bool,
}

impl Band {
    pub fn empty(h: f32) -> Band {
        Band {
            h,
            text_h: h,
            lines: Vec::new(),
            items: Vec::new(),
            notes: Vec::new(),
            brk: Brk::None,
            widow_control: false,
        }
    }

    /// Copy moved by (`dx`, `dy`).
    pub fn translated(&self, dx: f32, dy: f32) -> Band {
        self.translated_ids(dx, dy, 0)
    }

    /// Copy moved by (`dx`, `dy`) whose relative body ids are rebased on `base`.
    pub fn translated_ids(&self, dx: f32, dy: f32, base: u32) -> Band {
        let mut b = self.clone();
        for l in &mut b.lines {
            l.translate(dx, dy);
            l.source.rebase(base);
        }
        for i in &mut b.items {
            i.translate(dx, dy);
            if let Some(s) = i.source_mut() {
                s.rebase(base);
            }
        }
        b
    }

    /// Mark all body lines as non-body (repeated header rows, notes, headers); note
    /// lines keep their note source.
    pub fn set_other(&mut self) {
        for l in &mut self.lines {
            l.source.detach();
        }
        for i in &mut self.items {
            if let Some(s) = i.source_mut() {
                s.detach();
            }
        }
    }

    /// Append another band's content (already positioned in this band's space).
    pub fn absorb(&mut self, other: Band) {
        self.lines.extend(other.lines);
        self.items.extend(other.items);
        self.notes.extend(other.notes);
    }
}

/// Paragraph decoration (ECMA-376 Part 1, §17.3.1.24 pBdr, §17.3.1.31 shd).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Deco {
    pub shading: Option<Color>,
    /// Screen pattern of the shading (§17.18.78).
    pub pattern: Option<crate::ShadePattern>,
    pub borders: Option<Borders>,
    pub x0: f32,
    pub x1: f32,
}

/// A laid-out paragraph ready for placement.
#[derive(Clone)]
pub(crate) struct ParaBlock {
    pub lines: Vec<Band>,
    pub space_before: f32,
    pub space_after: f32,
    pub page_break_before: bool,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub widow_control: bool,
    pub contextual: bool,
    pub style: Option<String>,
    pub deco: Option<Deco>,
    pub floats: Vec<Float>,
    /// Depends only on the paragraph and the width: no list label, fields or note
    /// references (so it can be reused by the incremental layout).
    pub pure: bool,
    /// List label (kept to lay the paragraph out again around wrapped pictures).
    pub label: Option<(String, bo_model::RunProps, String)>,
    /// Drop cap frame (ECMA-376 Part 1, §17.3.1.11 framePr `w:dropCap`).
    pub frame: Option<bo_model::FrameProps>,
    /// Lines counted by line numbering (§17.3.1.35 suppressLineNumbers off).
    pub numbered: bool,
    /// Width the lines were laid out for (columns of unequal width lay it out again).
    pub width: f32,
}

impl ParaBlock {
    pub fn height(&self) -> f32 {
        self.lines.iter().map(|b| b.h).sum()
    }
}

/// Crop fractions l,t,r,b from `a:srcRect` (1/1000 of a percent, DrawingML §20.1.8.55).
pub(crate) fn crop_of(img: &Image) -> Option<[f32; 4]> {
    // The display image of a flipped picture is the mirrored media (§20.1.7.6 xfrm
    // flipH / flipV): the crop is applied to the mirrored picture, so its sides swap.
    let (fh, fv) = img
        .format
        .as_deref()
        .map_or((false, false), |f| (f.flip_h, f.flip_v));
    img.crop.map(|c| {
        let [l, t, r, b] = c.map(|v| v as f32 / 100_000.0);
        let (l, r) = if fh { (r, l) } else { (l, r) };
        let (t, b) = if fv { (b, t) } else { (t, b) };
        [l, t, r, b]
    })
}

/// Rotation in degrees from `a:xfrm/@rot` (60 000ths of a degree, §20.1.7.6).
pub(crate) fn rotation_of(img: &Image) -> f32 {
    img.rotation as f32 / 60_000.0
}

fn rel_line_band(rl: &RelLine, source: LineSource) -> Band {
    let fragments = rl
        .fragments
        .iter()
        .cloned()
        .map(|mut f| {
            f.y = rl.baseline;
            f
        })
        .collect();
    let mut items = Vec::new();
    for &(x, w, color, pattern) in &rl.highlights {
        items.push(PageItem::Rect {
            x,
            y: rl.baseline - rl.ascent,
            w,
            h: rl.ascent + rl.descent,
            color,
            layer: Layer::Back,
            pattern,
        });
    }
    // §17.3.2.4 bdr: a box around the run, `space` points away from the text.
    for (x, w, b) in &rl.borders {
        let pad = b.space as f32;
        let (x0, x1) = (x - pad, x + w + pad);
        let (y0, y1) = (rl.baseline - rl.ascent, rl.baseline + rl.descent);
        let width = (b.size as f32 / 8.0).max(0.25);
        let color = b.color.unwrap_or(bo_model::Color::BLACK);
        for (a, bb, c, d) in [
            (x0, y0, x1, y0),
            (x0, y1, x1, y1),
            (x0, y0, x0, y1),
            (x1, y0, x1, y1),
        ] {
            items.push(PageItem::Rule {
                x1: a,
                y1: bb,
                x2: c,
                y2: d,
                width,
                color,
                style: b.style.clone(),
            });
        }
    }
    let mut boxed = Vec::new();
    for o in &rl.objects {
        let y = rl.baseline - o.h;
        if let Some(d) = &o.drawing {
            // Inline shape / text box (its text lines follow the paragraph line).
            let para = match source {
                LineSource::Body(id) => Some(id),
                _ => None,
            };
            let (its, lines) = d.placed(o.x, y, para);
            items.extend(its);
            boxed.extend(lines);
        } else {
            // Round 19: with its picture style (border, effects).
            // Inside the room its effects take (§20.4.2.6 effectExtent).
            let [el, et, er, eb] = crate::picstyle::extent_pt(&o.image);
            items.extend(crate::picstyle::picture_items(
                &o.image,
                o.x + el,
                y + et,
                (o.w - el - er).max(0.1),
                (o.h - et - eb).max(0.1),
                Layer::Front,
                1.0,
                0,
            ));
        }
        // A tracked picture is framed in its author's colour, a deleted one crossed out.
        if let Some((color, deleted)) = o.revision {
            let (x0, y0, x1, y1) = (o.x, y, o.x + o.w, rl.baseline);
            let mut seg = |a: f32, b: f32, c: f32, d: f32| {
                items.push(PageItem::Rule {
                    x1: a,
                    y1: b,
                    x2: c,
                    y2: d,
                    width: 1.5,
                    color,
                    style: "single".into(),
                });
            };
            seg(x0, y0, x1, y0);
            seg(x0, y1, x1, y1);
            seg(x0, y0, x0, y1);
            seg(x1, y0, x1, y1);
            if deleted {
                seg(x0, y0, x1, y1);
                seg(x0, y1, x1, y0);
            }
        }
    }
    let mut lines = vec![Line {
        top: 0.0,
        height: rl.height,
        baseline: rl.baseline,
        fragments,
        source,
        start: rl.start,
        end: rl.end,
        carets: rl.carets.clone(),
        rotation: None,
    }];
    lines.extend(boxed);
    Band {
        h: rl.height,
        text_h: rl.text_h,
        lines,
        items,
        notes: rl.notes.clone(),
        brk: if rl.page_break_after {
            Brk::Page
        } else if rl.column_break_after {
            Brk::Column
        } else {
            Brk::None
        },
        widow_control: false,
    }
}

/// Lay out one paragraph for a content box of `width` points.
pub(crate) fn build_para(
    ctx: &mut Ctx<'_>,
    p: &Paragraph,
    width: f32,
    body: bool,
    note_mark: Option<&str>,
) -> Result<ParaBlock, LayoutError> {
    let props = para_props(ctx.doc, p);
    let label = {
        let lists = if body {
            &mut ctx.lists
        } else {
            &mut ctx.other_lists
        };
        props.num.and_then(|n| lists.next(ctx.doc, n))
    };
    let pure = props.num.is_none()
        && !p.runs.iter().any(|r| {
            r.content.iter().any(|c| {
                matches!(
                    c,
                    RunContent::Field { .. }
                        | RunContent::FootnoteRef(_)
                        | RunContent::EndnoteRef(_)
                )
            })
        });
    let source = if body {
        let id = ctx.next_id - ctx.id_origin;
        ctx.next_id += 1;
        LineSource::Body(id)
    } else if let Some(((endnote, id), next)) = ctx.note.as_mut() {
        let para = *next;
        *next += 1;
        LineSource::Note {
            endnote: *endnote,
            id: *id,
            para,
        }
    } else if let Some((at, next)) = ctx.textbox.as_mut() {
        let sub = *next;
        *next += 1;
        LineSource::TextBox {
            para: 0,
            at: *at,
            sub,
        }
    } else {
        LineSource::Other
    };
    para_block(ctx, p, props, label, source, width, note_mark, pure, &[])
}

/// Lay a top-level body paragraph out again so that its lines flow around wrapped
/// pictures (same list label and id as `pb`, built by [`build_para`]).
pub(crate) fn rewrap_para(
    ctx: &mut Ctx<'_>,
    p: &Paragraph,
    pb: &ParaBlock,
    width: f32,
    zones: &[WrapZone],
) -> Result<ParaBlock, LayoutError> {
    let props = para_props(ctx.doc, p);
    let source = LineSource::Body(0);
    para_block(
        ctx,
        p,
        props,
        pb.label.clone(),
        source,
        width,
        None,
        pb.pure,
        zones,
    )
}

#[allow(clippy::too_many_arguments)]
fn para_block(
    ctx: &mut Ctx<'_>,
    p: &Paragraph,
    props: bo_model::ParaProps,
    label: Option<(String, bo_model::RunProps, String)>,
    source: LineSource,
    width: f32,
    note_mark: Option<&str>,
    pure: bool,
    wrap: &[WrapZone],
) -> Result<ParaBlock, LayoutError> {
    let inp = ParaInput {
        doc: ctx.doc,
        para: p,
        props,
        label: label.clone(),
        width,
        fields: ctx.fields,
        default_tab: ctx.default_tab,
        note_numbers: &ctx.notes,
        note_mark,
        view: ctx.view,
        marks: ctx.marks,
        wrap,
    };
    let pl = layout_paragraph(&inp, ctx.fonts, ctx.cache)?;
    let mut lines: Vec<Band> = pl.lines.iter().map(|l| rel_line_band(l, source)).collect();
    // The box takes room above the first line and below the last (§17.3.1.24 pBdr):
    // the line and its `w:space` (and the shadow's band), measured black-box in the
    // reference word processor's PDF export (docs/ui/REPORT.md "Dàn trang vòng 6").
    if let Some(b) = &pl.borders {
        let (top, bottom) = box_room(b);
        if top > 0.0 {
            if let Some(first) = lines.first_mut() {
                let mut moved = first.translated(0.0, top);
                moved.h += top;
                moved.text_h += top;
                *first = moved;
            }
        }
        if let Some(last) = lines.last_mut().filter(|_| bottom > 0.0) {
            last.h += bottom;
            last.text_h += bottom;
        }
    }
    let visible_borders = pl.borders.as_ref().is_some_and(|b| {
        [&b.top, &b.left, &b.bottom, &b.right]
            .iter()
            .any(|e| e.as_ref().is_some_and(Border::is_visible))
    });
    let deco = (pl.shading.is_some() || visible_borders).then(|| Deco {
        shading: pl.shading,
        pattern: pl.shading_pattern,
        borders: pl.borders.clone(),
        x0: pl.indent_left,
        x1: width - pl.indent_right,
    });
    Ok(ParaBlock {
        lines,
        space_before: pl.space_before,
        space_after: pl.space_after,
        page_break_before: pl.page_break_before,
        keep_next: pl.keep_next,
        keep_lines: pl.keep_lines,
        widow_control: pl.widow_control,
        contextual: pl.contextual_spacing,
        style: pl.style.clone(),
        deco,
        floats: pl
            .floats
            .iter()
            .cloned()
            .map(|mut f| {
                f.para = match source {
                    LineSource::Body(id) => Some(id),
                    _ => None,
                };
                f
            })
            .collect(),
        pure,
        label,
        frame: pl.frame,
        width,
        numbered: pl.numbered,
    })
}

/// Width in points of a border (`w:sz` is in eighths of a point, §17.3.4).
pub(crate) fn border_width(b: &Border) -> f32 {
    (b.size as f32 / 8.0).max(0.25)
}

fn rule(b: &Border, x1: f32, y1: f32, x2: f32, y2: f32) -> PageItem {
    PageItem::Rule {
        x1,
        y1,
        x2,
        y2,
        width: border_width(b),
        color: b.color.unwrap_or(Color::BLACK),
        style: b.style.clone(),
    }
}

impl Deco {
    /// Visible box lines (top, left, bottom or right).
    fn has_box(&self) -> bool {
        self.borders.as_ref().is_some_and(|b| {
            [&b.top, &b.left, &b.bottom, &b.right]
                .iter()
                .any(|e| e.as_ref().is_some_and(Border::is_visible))
        })
    }

    /// Whether the next paragraph shares this one's box (§17.3.1.24 pBdr: consecutive
    /// paragraphs with identical borders and indents form one box, the `w:between` line
    /// separating them) or, without a box, its shading (§17.3.1.31) — measured black-box
    /// in the reference word processor's PDF export (docs/ui/REPORT.md "Dàn trang vòng 7"):
    /// another indent or border size starts a new box.
    pub(crate) fn groups_with(&self, next: &Deco) -> bool {
        (self.x0 - next.x0).abs() < 0.01
            && (self.x1 - next.x1).abs() < 0.01
            && self.borders == next.borders
            && (self.has_box()
                || (self.shading.is_some()
                    && self.shading == next.shading
                    && self.pattern == next.pattern))
    }

    /// Horizontal extent of the shading: the text column grown to the inner edges of the
    /// box lines (`w:space` + 1.4 pt left, + 1.5 pt right; 1.44 / 1.57 pt without lines in
    /// the reference export).
    fn shade_x(&self) -> (f32, f32) {
        let side =
            |e: Option<&Border>| e.filter(|e| e.is_visible()).map_or(0.0, |e| e.space as f32);
        let b = self.borders.as_ref();
        (
            self.x0 - side(b.and_then(|b| b.left.as_ref())) - 1.4,
            self.x1 + side(b.and_then(|b| b.right.as_ref())) + 1.5,
        )
    }
}

/// Place of a paragraph segment in a group of paragraphs sharing a box (see
/// [`Deco::groups_with`]).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Joined {
    /// The paragraph above shares the box: no top line (the `between` line instead);
    /// sides and shading start at `from` (the end of the paragraph above, so they run
    /// through the spacing between the two).
    pub above: bool,
    /// Top of the sides / shading when `above` (≤ the segment top).
    pub from: f32,
    /// The shading runs from `from` too (same shading as the paragraph above).
    pub shade_from: bool,
    /// The paragraph below shares the box: no bottom line.
    pub below: bool,
}

/// Shading and border items of a paragraph segment spanning `y0..y1` in a group of
/// paragraphs sharing a box.
pub(crate) fn deco_items_joined(d: &Deco, y0: f32, y1: f32, j: Joined, out: &mut Vec<PageItem>) {
    if let Some(color) = d.shading {
        let (x0, x1) = d.shade_x();
        let top = if j.above && j.shade_from { j.from } else { y0 };
        out.push(PageItem::Rect {
            x: x0,
            y: top,
            w: (x1 - x0).max(0.0),
            h: (y1 - top).max(0.0),
            color,
            layer: Layer::Back,
            pattern: d.pattern,
        });
    }
    if let Some(b) = &d.borders {
        box_items(b, d, y0, y1, j, out);
    }
}

/// Room of the `w:between` line (§17.3.1.24): above a paragraph joined to the one
/// above, the line and its `w:space`; below a paragraph joined to the one below, the
/// `w:space` — the line sits after the spacing between the two (measured black-box,
/// docs/ui/REPORT.md "Dàn trang vòng 7").
fn between_room(b: &Borders) -> (f32, f32) {
    edge(b.inside_h.as_ref()).map_or((0.0, 0.0), |(e, x)| {
        let space = e.space as f32;
        (x + space, space)
    })
}

/// `pb` with its box room changed for a group of paragraphs sharing a box: a paragraph
/// joined above loses the top line's room for the `between` line's, one joined below
/// the bottom line's for the `between` space.
pub(crate) fn regroup(pb: &ParaBlock, above: bool, below: bool) -> ParaBlock {
    let mut out = pb.clone();
    let Some(b) = pb.deco.as_ref().and_then(|d| d.borders.as_ref()) else {
        return out;
    };
    let (top, bottom) = box_room(b);
    let (b_top, b_bottom) = between_room(b);
    if above {
        if let Some(first) = out.lines.first_mut() {
            let dy = b_top - top;
            let mut moved = first.translated(0.0, dy);
            moved.h += dy;
            moved.text_h += dy;
            *first = moved;
        }
    }
    if below {
        if let Some(last) = out.lines.last_mut() {
            let dy = b_bottom - bottom;
            last.h += dy;
            last.text_h += dy;
        }
    }
    out
}

/// Distance across a border line as drawn (`double`: two strokes and the gap between).
fn border_extent(b: &Border) -> f32 {
    let w = border_width(b);
    match b.style.as_str() {
        "double" => 3.0 * w,
        "thick" => 1.5 * w,
        _ => w,
    }
}

/// Visible edge with its drawn extent.
fn edge(e: Option<&Border>) -> Option<(Border, f32)> {
    e.cloned().filter(Border::is_visible).map(|b| {
        let x = border_extent(&b);
        (b, x)
    })
}

/// Whether a paragraph box has a shadow (§17.18.2 CT_Border `w:shadow` on its right or
/// bottom edge).
fn box_shadow(b: &Borders) -> bool {
    [&b.right, &b.bottom]
        .iter()
        .any(|e| e.as_ref().is_some_and(|e| e.shadow && e.is_visible()))
}

/// Room a paragraph box takes above its first line and below its last: line extent +
/// `w:space`, and the line extent again for a shadowed box.
pub(crate) fn box_room(b: &Borders) -> (f32, f32) {
    let shadow = box_shadow(b);
    let room = |e: Option<&Border>| {
        edge(e).map_or(0.0, |(b, x)| {
            x + b.space as f32 + if shadow { x } else { 0.0 }
        })
    };
    (room(b.top.as_ref()), room(b.bottom.as_ref()))
}

/// Lines of a paragraph box (§17.3.1.24 pBdr) around a segment `y0..y1` (its room
/// above / below included, see [`box_room`]), as the reference word processor exports
/// them: the top line's outer edge on the segment top (one line extent lower with a
/// shadow), the bottom one's on the segment bottom (one extent higher with a shadow);
/// the left line's inner edge `w:space` + 1.4 pt left of the text, the right one's
/// `w:space` + 1.5 pt right of it; top and bottom lines run between the outer edges of
/// the side lines, side lines between the outer edges of the top and bottom ones. A
/// shadow is a black band as wide as the line right outside the right / bottom line —
/// the right one from the top line's inner edge down past the bottom band.
fn box_items(b: &Borders, d: &Deco, y0: f32, y1: f32, j: Joined, out: &mut Vec<PageItem>) {
    let shadow = box_shadow(b) && !j.above && !j.below;
    let (top, bottom, left, right) = (
        edge(b.top.as_ref()),
        edge(b.bottom.as_ref()),
        edge(b.left.as_ref()),
        edge(b.right.as_ref()),
    );
    let lift = |e: &Option<(Border, f32)>| {
        e.as_ref()
            .map_or(0.0, |(_, x)| if shadow { *x } else { 0.0 })
    };
    // Outer edges of the box.
    let yt = y0 + lift(&top);
    let yb = y1 - lift(&bottom);
    let xl = d.x0 - left.as_ref().map_or(0.0, |(b, x)| b.space as f32 + 1.4 + x);
    let xr = d.x1
        + right
            .as_ref()
            .map_or(0.0, |(b, x)| b.space as f32 + 1.5 + x);
    match &top {
        Some((e, x)) if !j.above => {
            let y = yt + x / 2.0;
            out.push(rule(e, xl, y, xr, y));
        }
        _ => {}
    }
    if j.above {
        // The `between` line on the segment top, across the box like a top line.
        if let Some((e, x)) = edge(b.inside_h.as_ref()) {
            let y = y0 + x / 2.0;
            out.push(rule(&e, xl, y, xr, y));
        }
    }
    match &bottom {
        Some((e, x)) if !j.below => {
            let y = yb - x / 2.0;
            out.push(rule(e, xl, y, xr, y));
        }
        _ => {}
    }
    // Sides run on through the spacing to the paragraph below's sides.
    let side_top = if j.above { j.from.min(yt) } else { yt };
    if let Some((e, x)) = &left {
        let xc = xl + x / 2.0;
        out.push(rule(e, xc, side_top, xc, yb));
    }
    if let Some((e, x)) = &right {
        let xc = xr - x / 2.0;
        out.push(rule(e, xc, side_top, xc, yb));
    }
    if !shadow {
        return;
    }
    let shade = |x: f32, y: f32, w: f32, h: f32| PageItem::Rect {
        x,
        y,
        w: w.max(0.0),
        h: h.max(0.0),
        color: Color::BLACK,
        layer: Layer::Front,
        pattern: None,
    };
    let rw = right
        .as_ref()
        .filter(|(e, _)| e.shadow)
        .map_or(0.0, |(_, x)| *x);
    let bw = bottom
        .as_ref()
        .filter(|(e, _)| e.shadow)
        .map_or(0.0, |(_, x)| *x);
    if rw > 0.0 {
        let from = yt + top.as_ref().map_or(0.0, |(_, x)| *x);
        out.push(shade(xr, from, rw, yb + bw - from));
    }
    if bw > 0.0 {
        let from = xl + left.as_ref().map_or(0.0, |(_, x)| *x);
        out.push(shade(from, yb, xr + rw - from, bw));
    }
}

/// Content stacked without pagination.
#[derive(Debug, Default, Clone)]
pub(crate) struct Stack {
    /// (top, band) pairs, top relative to the stack top.
    pub bands: Vec<(f32, Band)>,
    pub h: f32,
    /// Anchored pictures with the top of their anchor paragraph.
    pub floats: Vec<(f32, Float)>,
}

/// Stack the blocks of note `key` (its lines carry [`LineSource::Note`]).
pub(crate) fn stack_note(
    ctx: &mut Ctx<'_>,
    key: NoteKey,
    blocks: &[Block],
    width: f32,
    note_mark: Option<&str>,
) -> Result<Stack, LayoutError> {
    let saved = ctx.note.replace((key, 0));
    let st = stack_blocks(ctx, blocks, width, false, note_mark);
    ctx.note = saved;
    st
}

/// Stack `blocks` in a box `width` points wide.
pub(crate) fn stack_blocks(
    ctx: &mut Ctx<'_>,
    blocks: &[Block],
    width: f32,
    body: bool,
    note_mark: Option<&str>,
) -> Result<Stack, LayoutError> {
    let mut st = Stack::default();
    let mut y = 0.0f32;
    let mut prev: Option<(Option<String>, bool, f32)> = None;
    let mut first_para = true;
    // §17.3.1.24: the paragraph built ahead (to know whether it shares the box of the one
    // before) and the box left open by the previous paragraph (its deco, its bottom).
    let mut ahead: Option<ParaBlock> = None;
    let mut open: Option<(Deco, f32)> = None;
    for (k, b) in blocks.iter().enumerate() {
        match b {
            Block::Paragraph(p) => {
                // The note number mark belongs to the first paragraph of a note only.
                let mark = if first_para { note_mark } else { None };
                first_para = false;
                let pb = match ahead.take() {
                    Some(pb) => pb,
                    None => build_para(ctx, p, width, body, mark)?,
                };
                let joined_above = open
                    .as_ref()
                    .filter(|(g, _)| pb.deco.as_ref().is_some_and(|d| g.groups_with(d)))
                    .cloned();
                let mut joined_below = false;
                if let (Some(d), Some(Block::Paragraph(np))) = (&pb.deco, blocks.get(k + 1)) {
                    let n = build_para(ctx, np, width, body, None)?;
                    joined_below =
                        !n.page_break_before && n.deco.as_ref().is_some_and(|nd| d.groups_with(nd));
                    ahead = Some(n);
                }
                let pb = if joined_above.is_some() || joined_below {
                    regroup(&pb, joined_above.is_some(), joined_below)
                } else {
                    pb
                };
                let mut before = pb.space_before;
                if let Some((ps, pc, pa)) = &prev {
                    let mut after = *pa;
                    // §17.3.1.9 contextualSpacing: ignore spacing between paragraphs of the same style.
                    if *ps == pb.style {
                        if pb.contextual {
                            before = 0.0;
                        }
                        if *pc {
                            after = 0.0;
                        }
                    }
                    before = crate::flow::collapse_spacing(&mut y, *pa, after, before);
                }
                y += before;
                let y0 = y;
                let first_idx = st.bands.len();
                for mut band in pb.lines {
                    let h = band.h;
                    band.widow_control = pb.widow_control && !band.lines.is_empty();
                    st.bands.push((y, band));
                    y += h;
                }
                open = None;
                if let Some(d) = &pb.deco {
                    if let Some((top, band)) = st.bands.get_mut(first_idx) {
                        let mut j = Joined {
                            above: joined_above.is_some(),
                            from: 0.0,
                            shade_from: false,
                            below: joined_below,
                        };
                        // Sides and shading run through the spacing from the paragraph above.
                        if let Some((g, end)) = &joined_above {
                            j.from = (end - *top).min(0.0);
                            j.shade_from = g.shading == d.shading && g.pattern == d.pattern;
                        }
                        let mut items = Vec::new();
                        deco_items_joined(d, 0.0, y - *top, j, &mut items);
                        band.items.extend(items);
                    }
                    if joined_below {
                        open = Some((d.clone(), y));
                    }
                }
                let _ = y0;
                for f in pb.floats {
                    st.floats.push((y0, f));
                }
                y += pb.space_after;
                prev = Some((pb.style, pb.contextual, pb.space_after));
            }
            Block::Table(t) => {
                let tb = layout_table(ctx, t, width, body)?;
                for row in &tb.rows {
                    let band = row_band(row, row.h, tb.x, false, false);
                    let h = band.h;
                    st.bands.push((y, band));
                    y += h;
                }
                prev = None;
                open = None;
            }
            Block::Opaque(_) => {}
        }
    }
    st.h = y.max(0.0);
    Ok(st)
}

// ------------------------------------------------------------------ tables

thread_local! {
    /// Lay out table gridlines ([`GRID_STYLE`] rules on cell edges without a border) for a
    /// view that shows them (BlueDoc's View Gridlines); off by default, never drawn by
    /// `bo-render` (the view draws them over the page). Per thread: the engine lays out on
    /// the thread that owns it.
    static TABLE_GRID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Style name of a table gridline rule (view only, not part of the document).
pub const GRID_STYLE: &str = "viewGrid";

/// Turn [`GRID_STYLE`] rules on or off for later layouts on this thread.
pub fn set_table_grid(on: bool) {
    TABLE_GRID.with(|g| g.set(on));
}

/// A gridline rule from (`x1`, `y1`) to (`x2`, `y2`).
fn grid_rule(x1: f32, y1: f32, x2: f32, y2: f32) -> PageItem {
    PageItem::Rule {
        x1,
        y1,
        x2,
        y2,
        width: 0.5,
        color: Color {
            r: 0x93,
            g: 0x93,
            b: 0x93,
        },
        style: GRID_STYLE.into(),
    }
}

/// A laid-out cell.
#[derive(Debug, Clone)]
pub(crate) struct CellBox {
    /// Left edge relative to the table's left edge.
    pub x: f32,
    pub w: f32,
    pub content: Vec<(f32, Band)>,
    pub content_h: f32,
    /// left, right, top, bottom
    pub margins: [f32; 4],
    pub shading: Option<Color>,
    /// Screen pattern of the shading (§17.18.78).
    pub pattern: Option<crate::ShadePattern>,
    /// top, right, bottom, left (visible borders only)
    pub edges: [Option<Border>; 4],
    /// Diagonal borders (visible only): top-left to bottom-right (§17.4.80 tl2br) and
    /// top-right to bottom-left (§17.4.79 tr2bl).
    pub diagonals: [Option<Border>; 2],
    pub v_align: CellVAlign,
    /// `vMerge` continuation (content drawn by the restart cell above).
    pub continued: bool,
    /// The cell below (same grid column) continues this cell's merge.
    pub merged_below: bool,
    /// Rows a vMerge restart cell spans (1 otherwise) and the height of the rows below
    /// this one that it spans, set once all rows are measured: §17.4.84 vAlign centres /
    /// bottom-aligns its content in the whole merged cell, as the reference word processor
    /// does (measured black-box, docs/ui/REPORT.md "Dàn trang vòng 3", office-17).
    pub rows_spanned: usize,
    pub span_below: f32,
    /// Tracked cell insertion/deletion in markup (§17.13.5.4 cellIns, §17.13.5.3
    /// cellDel): (author colour, deleted).
    pub revision: Option<(Color, bool)>,
    /// Table with cell spacing (§17.4.44 tblCellSpacing): room above and below the
    /// cell's own box inside the row band (0 otherwise); the cell then draws all four of
    /// its edges around that box.
    pub inset: [f32; 2],
}

/// A laid-out row.
#[derive(Debug, Clone)]
pub(crate) struct RowBox {
    pub h: f32,
    pub cells: Vec<CellBox>,
    pub header: bool,
    pub cant_split: bool,
    /// Tracked row insertion/deletion in markup (§17.13.5.19, §17.13.5.12): (author
    /// colour, deleted).
    pub revision: Option<(Color, bool)>,
    /// Colour of a change bar for tracked cell/table property changes (§17.13.5.36,
    /// §17.13.5.34).
    pub changed: Option<Color>,
    /// Table with cell spacing (§17.4.44 tblCellSpacing): the table's own border frame.
    pub frame: Option<Box<SpacedFrame>>,
}

/// Border frame of a table with cell spacing around one row's band: side lines at
/// `x0` / `x1` (relative to the table's x), the top line on the first row, the bottom
/// line on the last.
#[derive(Debug, Clone)]
pub(crate) struct SpacedFrame {
    pub x0: f32,
    pub x1: f32,
    /// top, right, bottom, left (visible only)
    pub edges: [Option<Border>; 4],
    pub first: bool,
    pub last: bool,
}

/// A laid-out table.
#[derive(Debug, Clone)]
pub(crate) struct TableBox {
    /// Offset of the table's left edge from the content box left.
    pub x: f32,
    pub rows: Vec<RowBox>,
    /// Floating position (§17.4.58 tblpPr) of a table the text wraps around.
    pub float: Option<bo_model::TableFloat>,
    /// §17.4.56 tblOverlap `never`: no other floating table may overlap this one.
    pub overlap_never: bool,
}

/// Paragraph properties of the table's style chain (ECMA-376 Part 1, §17.7.6 table
/// styles: a table style's `w:pPr` applies to the paragraphs of its cells), `None` when
/// it sets none.
fn table_style_ppr(doc: &Document, t: &Table) -> Option<ParaProps> {
    let id = t.props.style.as_ref()?;
    let mut p = ParaProps::default();
    for s in doc.styles.chain(id) {
        p = s.ppr.inherit(&p);
    }
    (p != ParaProps::default()).then_some(p)
}

/// The paragraphs of a cell with the table style's paragraph properties `tp` under
/// their own (§17.7.2 style hierarchy: document defaults, then table style, then
/// paragraph style, then direct formatting). Values the paragraph style sets win over
/// the table style; nested tables apply their own style.
fn with_table_ppr(doc: &Document, blocks: &[Block], tp: &ParaProps) -> Vec<Block> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => {
                let style_id = p.props.style.clone().or_else(|| {
                    doc.styles
                        .default_of(bo_model::StyleKind::Paragraph)
                        .map(|s| s.id.clone())
                });
                let style = style_id
                    .as_deref()
                    .map(|id| doc.styles.resolve_paragraph(id).0)
                    .unwrap_or_default();
                let mut props = p.props.inherit(&style.inherit(tp));
                props.style.clone_from(&p.props.style);
                props.num = p.props.num;
                let mut q = p.clone();
                q.props = props;
                Block::Paragraph(q)
            }
            other => other.clone(),
        })
        .collect()
}

fn table_borders(doc: &Document, t: &Table) -> Borders {
    if let Some(b) = &t.props.borders {
        return b.clone();
    }
    // Table style chain (§17.7.6 table styles): nearest style that defines borders.
    if let Some(id) = &t.props.style {
        for s in doc.styles.chain(id).iter().rev() {
            if let Some(b) = s.tblpr.as_ref().and_then(|p| p.borders.clone()) {
                return b;
            }
        }
    }
    Borders::default()
}

fn grid_widths(t: &Table, geo: &TableGeometry, avail: f32) -> Vec<f32> {
    let mut grid: Vec<f32> = geo.grid.iter().map(|w| w.to_pt().max(0.0)).collect();
    let cols_in_rows = geo
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.start + c.span).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    if grid.iter().sum::<f32>() <= 0.5 || grid.len() < cols_in_rows {
        // No usable tblGrid (§17.4.49): derive from the first row's cell widths or split evenly.
        let n = cols_in_rows.max(1);
        let first: Vec<f32> = t
            .rows
            .first()
            .map(|r| {
                r.cells
                    .iter()
                    .flat_map(|c| {
                        let span = c.props.grid_span.max(1) as usize;
                        let w = match c.props.width {
                            Some(TableWidth::Dxa(w)) => w.to_pt() / span as f32,
                            Some(TableWidth::Pct(p)) => avail * p as f32 / 5000.0 / span as f32,
                            _ => 0.0,
                        };
                        std::iter::repeat_n(w, span)
                    })
                    .collect()
            })
            .unwrap_or_default();
        grid = if first.len() == n && first.iter().all(|w| *w > 0.5) {
            first
        } else {
            vec![avail / n as f32; n]
        };
    }
    // Percentage table width (§17.4.64 tblW type="pct"): scale the grid.
    let total: f32 = grid.iter().sum();
    let target = match t.props.width {
        Some(TableWidth::Pct(p)) if p > 0 => Some(avail * p as f32 / 5000.0),
        Some(TableWidth::Dxa(w)) if w.0 > 0 && t.grid.is_empty() => Some(w.to_pt()),
        _ => None,
    };
    if let Some(target) = target {
        if total > 0.0 {
            let k = target / total;
            for g in &mut grid {
                *g *= k;
            }
        }
    }
    grid
}

/// AutoFit to contents (ECMA-376 Part 1, §17.4.53 tblLayout `autofit`, §17.4.72 tcW
/// `auto`): columns whose cells give no preferred width are sized by their content as
/// the reference word processor does — each takes its widest unwrapped line when all
/// fit beside the other columns, else its longest word plus a share of the room left in
/// proportion to how much more its lines want (cell margins included). Columns with a
/// preferred width keep their grid width.
fn autofit_columns(
    ctx: &mut Ctx<'_>,
    t: &Table,
    geo: &TableGeometry,
    grid: &mut [f32],
    avail: f32,
    body: bool,
) -> Result<(), LayoutError> {
    if t.props.fixed_layout
        || matches!(t.props.width, Some(TableWidth::Pct(_)))
        || matches!(t.props.width, Some(TableWidth::Dxa(w)) if w.0 > 0)
    {
        return Ok(());
    }
    let n = grid.len();
    let (ml, mr) = t
        .props
        .cell_margin
        .map_or((DEFAULT_CELL_MARGIN, DEFAULT_CELL_MARGIN), |(l, r)| {
            (l.to_pt(), r.to_pt())
        });
    // A column is sized by content when every single-column cell in it has no width.
    let mut auto = vec![None::<bool>; n];
    for (ri, places) in geo.rows.iter().enumerate() {
        for (k, pl) in places.iter().enumerate() {
            if pl.span != 1 || pl.hidden || pl.start >= n {
                continue;
            }
            let Some(cell) = t.rows.get(ri).and_then(|r| r.cells.get(k)) else {
                continue;
            };
            let free = match cell.props.width {
                None | Some(TableWidth::Auto) => true,
                Some(TableWidth::Dxa(w)) => w.0 <= 0,
                Some(TableWidth::Pct(p)) => p == 0,
            };
            let a = &mut auto[pl.start];
            *a = Some(a.unwrap_or(true) && free);
        }
    }
    if !auto.contains(&Some(true)) {
        return Ok(());
    }
    let table_ppr = table_style_ppr(ctx.doc, t);
    let mut min = vec![0.0f32; n];
    let mut max = vec![0.0f32; n];
    for (ri, places) in geo.rows.iter().enumerate() {
        for (k, pl) in places.iter().enumerate() {
            if pl.span != 1 || pl.hidden || pl.start >= n || auto[pl.start] != Some(true) {
                continue;
            }
            let Some(cell) = t.rows.get(ri).and_then(|r| r.cells.get(k)) else {
                continue;
            };
            let styled;
            let blocks: &[Block] = match &table_ppr {
                Some(tp) => {
                    styled = with_table_ppr(ctx.doc, &cell.blocks, tp);
                    &styled
                }
                None => &cell.blocks,
            };
            let st = measure_stack(ctx, blocks, 20_000.0, body)?;
            let (lo, hi) = (longest_word(&st.bands), widest_line(&st.bands));
            min[pl.start] = min[pl.start].max(lo + ml + mr);
            max[pl.start] = max[pl.start].max(hi + ml + mr);
        }
    }
    let fixed: f32 = (0..n)
        .filter(|&c| auto[c] != Some(true))
        .map(|c| grid[c])
        .sum();
    let room = (avail - fixed).max(0.0);
    let cols: Vec<usize> = (0..n).filter(|&c| auto[c] == Some(true)).collect();
    let sum_max: f32 = cols.iter().map(|&c| max[c]).sum();
    let sum_min: f32 = cols.iter().map(|&c| min[c]).sum();
    for &c in &cols {
        grid[c] = if sum_max <= room {
            max[c]
        } else if sum_min >= room || sum_max - sum_min < 0.01 {
            min[c]
        } else {
            min[c] + (max[c] - min[c]) * (room - sum_min) / (sum_max - sum_min)
        }
        .max(ml + mr + 1.0);
    }
    Ok(())
}

/// Right edge of the widest line in `bands` (points from the box's left edge).
fn widest_line(bands: &[(f32, Band)]) -> f32 {
    bands
        .iter()
        .flat_map(|(_, b)| &b.lines)
        .flat_map(|l| &l.fragments)
        .map(|f| f.x + f.width)
        .fold(0.0f32, f32::max)
}

/// Width of the widest word (run of glyphs between spaces) in `bands`: the narrowest a
/// column can be without breaking a word.
fn longest_word(bands: &[(f32, Band)]) -> f32 {
    let mut best = 0.0f32;
    for f in bands
        .iter()
        .flat_map(|(_, b)| &b.lines)
        .flat_map(|l| &l.fragments)
    {
        let mut start: Option<f32> = None;
        for g in f.glyphs.iter() {
            let space = f
                .text
                .get(g.cluster as usize..)
                .and_then(|t| t.chars().next())
                .is_some_and(char::is_whitespace);
            if space {
                if let Some(s) = start.take() {
                    best = best.max(g.x - s);
                }
            } else if start.is_none() {
                start = Some(g.x);
            }
        }
        if let Some(s) = start {
            best = best.max(f.width - s);
        }
    }
    best
}

/// Stack `blocks` at `width` without side effects on `ctx` (ids, list counters).
fn measure_stack(
    ctx: &mut Ctx<'_>,
    blocks: &[Block],
    width: f32,
    body: bool,
) -> Result<Stack, LayoutError> {
    let saved = (
        ctx.next_id,
        ctx.lists.clone(),
        ctx.other_lists.clone(),
        ctx.endnote_order.len(),
        ctx.note,
        ctx.textbox,
    );
    let st = stack_blocks(ctx, blocks, width, body, None);
    ctx.next_id = saved.0;
    ctx.lists = saved.1;
    ctx.other_lists = saved.2;
    ctx.endnote_order.truncate(saved.3);
    ctx.note = saved.4;
    ctx.textbox = saved.5;
    st
}

/// Stack `blocks` at `width` without side effects on `ctx` (ids, list counters):
/// the widest line and the height.
fn measure_blocks(
    ctx: &mut Ctx<'_>,
    blocks: &[Block],
    width: f32,
    body: bool,
) -> Result<(f32, f32), LayoutError> {
    let saved = (
        ctx.next_id,
        ctx.lists.clone(),
        ctx.other_lists.clone(),
        ctx.endnote_order.len(),
        ctx.note,
        ctx.textbox,
    );
    let st = stack_blocks(ctx, blocks, width, body, None);
    ctx.next_id = saved.0;
    ctx.lists = saved.1;
    ctx.other_lists = saved.2;
    ctx.endnote_order.truncate(saved.3);
    ctx.note = saved.4;
    ctx.textbox = saved.5;
    let st = st?;
    Ok((widest_line(&st.bands), st.h))
}

/// Line length of a vertical cell whose row has no set height: its longest line,
/// at least as long as the stack is thick (an empty cell keeps a square shape).
fn natural_length(ctx: &mut Ctx<'_>, blocks: &[Block], body: bool) -> Result<f32, LayoutError> {
    let (w, h) = measure_blocks(ctx, blocks, 2000.0, body)?;
    Ok((w + 0.5).max(h).max(6.0))
}

/// Where a rotation takes a page item (rectangles and rules exactly; pictures and
/// shapes turn about their centre).
pub(crate) fn rotate_item(it: &mut PageItem, r: &LineRotation) {
    match it {
        PageItem::Rect { x, y, w, h, .. } => {
            let (ax, ay) = r.apply(*x, *y);
            let (bx, by) = r.apply(*x + *w, *y + *h);
            (*x, *y, *w, *h) = (ax.min(bx), ay.min(by), (ax - bx).abs(), (ay - by).abs());
        }
        PageItem::Rule { x1, y1, x2, y2, .. } => {
            (*x1, *y1) = r.apply(*x1, *y1);
            (*x2, *y2) = r.apply(*x2, *y2);
        }
        PageItem::Image {
            x,
            y,
            w,
            h,
            rotation,
            ..
        } => {
            let (cx, cy) = r.apply(*x + *w / 2.0, *y + *h / 2.0);
            (*x, *y) = (cx - *w / 2.0, cy - *h / 2.0);
            *rotation += r.degrees;
        }
        PageItem::Shape { x, y, w, h, .. } => {
            let (cx, cy) = r.apply(*x + *w / 2.0, *y + *h / 2.0);
            (*x, *y) = (cx - *w / 2.0, cy - *h / 2.0);
        }
    }
}

/// Turn the content of a vertical cell (ECMA-376 Part 1, §17.4.72 textDirection):
/// lines `len` long stacked `thick` deep become columns; `rot` 90 = top to bottom
/// (first line on the right), -90 = bottom to top (first line on the left). The
/// result is one band `len` tall, moved `shift` points right.
pub(crate) fn rotate_content(
    bands: Vec<(f32, Band)>,
    rot: i32,
    thick: f32,
    len: f32,
    shift: f32,
) -> Band {
    let mut all = Band::empty(len);
    for (top, b) in bands {
        all.absorb(b.translated(0.0, top));
    }
    // A quarter turn about (p, p) takes the box [0, len] × [0, thick] to
    // [0, thick] × [0, len]: p = thick / 2 clockwise, len / 2 anticlockwise.
    let r = if rot > 0 {
        LineRotation {
            degrees: 90.0,
            cx: thick / 2.0,
            cy: thick / 2.0,
        }
    } else {
        LineRotation {
            degrees: -90.0,
            cx: len / 2.0,
            cy: len / 2.0,
        }
    };
    for l in &mut all.lines {
        l.rotation = Some(r);
    }
    for it in &mut all.items {
        rotate_item(it, &r);
    }
    all.translated(shift, 0.0)
}

/// Measurements of a table for the table tools (points).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableMeasure {
    /// Laid-out height of every row.
    pub row_heights: Vec<f32>,
    /// Width of each grid column with no line wrapped (plus the cell margins).
    pub max_widths: Vec<f32>,
}

/// Measure `t` laid out in a box `avail` points wide: row heights and the content
/// widths of each grid column (cells spanning several columns widen the columns they
/// cover when they need more room).
pub(crate) fn measure_table(
    ctx: &mut Ctx<'_>,
    t: &Table,
    avail: f32,
) -> Result<TableMeasure, LayoutError> {
    let tb = layout_table(ctx, t, avail, false)?;
    let (ml, mr) = t
        .props
        .cell_margin
        .map_or((DEFAULT_CELL_MARGIN, DEFAULT_CELL_MARGIN), |(l, r)| {
            (l.to_pt(), r.to_pt())
        });
    let cols = t.grid.len().max(
        t.rows
            .iter()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| c.props.grid_span.max(1) as usize)
                    .sum()
            })
            .max()
            .unwrap_or(0),
    );
    let mut max_widths = vec![ml + mr; cols];
    // Cells spanning several grid columns (§17.4.17 gridSpan): (first column, span, width).
    let mut spans: Vec<(usize, usize, f32)> = Vec::new();
    for row in &t.rows {
        let mut g = 0usize;
        for c in &row.cells {
            let n = c.props.grid_span.max(1) as usize;
            if c.props.v_merge != Some(VMerge::Continue) && g + n <= cols {
                let (wide, _) = measure_blocks(ctx, &c.blocks, 20_000.0, false)?;
                if n == 1 {
                    max_widths[g] = max_widths[g].max(wide + ml + mr);
                } else {
                    spans.push((g, n, wide + ml + mr));
                }
            }
            g += n;
        }
    }
    // Then the spanning cells, narrowest spans first: what a cell needs beyond the
    // columns it covers is shared among them in proportion to their widths.
    spans.sort_by_key(|s| s.1);
    for (g, n, need) in spans {
        let cover = &mut max_widths[g..g + n];
        let have: f32 = cover.iter().sum();
        if need > have {
            let extra = need - have;
            for w in cover.iter_mut() {
                *w += if have > 0.0 {
                    extra * *w / have
                } else {
                    extra / n as f32
                };
            }
        }
    }
    Ok(TableMeasure {
        row_heights: tb.rows.iter().map(|r| r.h).collect(),
        max_widths,
    })
}

/// Lay out a table (ECMA-376 Part 1, §17.4.38 tbl) for a content box `avail` points wide.
pub(crate) fn layout_table(
    ctx: &mut Ctx<'_>,
    t: &Table,
    avail: f32,
    body: bool,
) -> Result<TableBox, LayoutError> {
    // Tracked structural changes (§17.13.5): the cells and grid of the view.
    let geo = t.geometry(match ctx.view {
        RevisionView::Markup => TableView::Markup,
        RevisionView::Final => TableView::Final,
        RevisionView::Original => TableView::Original,
    });
    let mut grid = grid_widths(t, &geo, avail);
    autofit_columns(ctx, t, &geo, &mut grid, avail, body)?;
    let ncols = grid.len();
    let total: f32 = grid.iter().sum();
    let indent = t.props.indent.map_or(0.0, Twips::to_pt);
    let (ml, mr) = t
        .props
        .cell_margin
        .map_or((DEFAULT_CELL_MARGIN, DEFAULT_CELL_MARGIN), |(l, r)| {
            (l.to_pt(), r.to_pt())
        });
    // §17.4.29 jc on tables. §17.4.50 tblInd: before compatibility mode 15 ([MS-DOCX]
    // §2.3.1 compatSetting) the indent places the first cell's text, so the table edge
    // sits one left cell margin (§17.4.43 tblCellMar) left of it — the cell text lines up
    // with the body text, as the desktop word processor draws such files; from mode 15 on
    // it places the table edge.
    let tb = table_borders(ctx.doc, t);
    // From mode 15 the indent places the outer edge of the table's left border: grid line,
    // border centre and cell text all sit half that border's width further right (measured
    // on the reference's PDFs, office-09 / office-36: a 0.5 pt left border spans 85.1–85.58
    // pt from an 85.05 pt margin, cell text 0.24 pt right of grid + cell margin).
    let half_left = tb
        .left
        .as_ref()
        .filter(|b| !matches!(b.style.as_str(), "nil" | "none"))
        .map_or(0.0, |b| b.size as f32 / 16.0);
    let x = match t.props.align {
        Some(bo_model::Alignment::Center) => (avail - total) / 2.0,
        Some(bo_model::Alignment::Right) => avail - total,
        _ if !ctx.doc.modern_layout() => indent - ml,
        _ => indent + half_left,
    };
    let table_ppr = table_style_ppr(ctx.doc, t);
    let (cell_top, cell_bottom) = t
        .props
        .cell_margin_vertical
        .map_or((0.0, 0.0), |(a, b)| (a.to_pt(), b.to_pt()));
    let nrows = t.rows.len();
    // §17.4.44 tblCellSpacing: cells drawn apart inside the table's own border frame,
    // measured black-box in the reference word processor's PDF export (docs/ui/REPORT.md
    // "Dàn trang vòng 7", cs-a … cs-f): twice the spacing between the frame and the cells
    // and between cells (both ways), the cells sharing what is left in proportion to
    // their grid columns; each cell draws its own four edges.
    let spacing = cell_spacing(&t.props).filter(|s| *s > 0.0);
    let visible = |b: &Option<Border>| b.clone().filter(Border::is_visible);
    let frame_edges = [
        visible(&tb.top),
        visible(&tb.right),
        visible(&tb.bottom),
        visible(&tb.left),
    ];
    let half_cell = [&tb.inside_v, &tb.left]
        .iter()
        .find_map(|b| b.as_ref().filter(|b| b.is_visible()))
        .map_or(0.0, |b| border_width(b) / 2.0);
    let spaced_x = |s: f32, g: usize, span: usize| -> (f32, f32) {
        let step = 2.0 * s + 2.0 * half_cell;
        let room = total - 2.0 * (2.0 * s + half_cell) - ncols.saturating_sub(1) as f32 * step;
        let k = room.max(1.0) / total.max(1.0);
        let x = 2.0 * s + half_cell + grid.iter().take(g).map(|w| w * k + step).sum::<f32>();
        let end = (g + span).min(ncols);
        let w = grid[g.min(end)..end].iter().map(|w| w * k).sum::<f32>()
            + span.saturating_sub(1) as f32 * step;
        (x, w)
    };

    // Placement of each cell in the view, per row.
    let place_at = |ri: usize, g: usize| -> Option<&CellPlace> {
        geo.rows.get(ri)?.iter().find(|c| c.start == g && !c.hidden)
    };
    // Number of rows a vMerge restart cell spans (§17.4.85).
    let span_rows = |ri: usize, g: usize| -> usize {
        let mut k = 1;
        while ri + k < nrows
            && place_at(ri + k, g).is_some_and(|c| c.v_merge == Some(VMerge::Continue))
        {
            k += 1;
        }
        k
    };
    let markup = ctx.view == RevisionView::Markup;

    let mut rows: Vec<RowBox> = Vec::with_capacity(nrows);
    let mut tall: Vec<(usize, usize, f32)> = Vec::new(); // (first row, span, needed height)
    for (ri, row) in t.rows.iter().enumerate() {
        let mut cells = Vec::with_capacity(row.cells.len());
        let mut h = 0.0f32;
        for (ci, cell) in row.cells.iter().enumerate() {
            let pl = geo.rows[ri].get(ci).copied().unwrap_or(CellPlace {
                start: 0,
                span: 1,
                hidden: false,
                v_merge: cell.props.v_merge,
            });
            let (g, span) = (pl.start, pl.span);
            let cx: f32 = grid.iter().take(g).sum();
            let w: f32 = if pl.hidden {
                0.0
            } else if g < ncols {
                grid[g..(g + span).min(ncols)].iter().sum()
            } else {
                avail / ncols.max(1) as f32
            };
            let (cx, w) = match spacing {
                Some(s) if !pl.hidden && g < ncols => spaced_x(s, g, span),
                _ => (cx, w),
            };
            let continued = pl.v_merge == Some(VMerge::Continue);
            // ECMA-376 Part 1, §17.4.72 textDirection: vertical lines as long as the
            // row's height (§17.4.81 trHeight), else as long as the longest line.
            let rot = match cell.props.text_direction {
                Some(d) if !continued && !pl.hidden => d.rotation(),
                _ => 0,
            };
            let mut vlen = 0.0f32;
            let styled;
            let cell_blocks: &[Block] = match &table_ppr {
                Some(tp) => {
                    styled = with_table_ppr(ctx.doc, &cell.blocks, tp);
                    &styled
                }
                None => &cell.blocks,
            };
            let mut stack = if continued {
                Stack::default()
            } else if rot != 0 {
                vlen = match row.props.height {
                    Some((v, _)) if v.to_pt() > 1.0 => v.to_pt(),
                    _ => natural_length(ctx, cell_blocks, body)?,
                };
                stack_blocks(ctx, cell_blocks, vlen.max(1.0), body, None)?
            } else {
                // Hidden cells are laid out too (at an ordinary width): their
                // paragraphs keep their ids.
                let lw = if pl.hidden {
                    avail / ncols.max(1) as f32
                } else {
                    w
                };
                stack_blocks(ctx, cell_blocks, (lw - ml - mr).max(1.0), body, None)?
            };
            if pl.hidden {
                stack = Stack::default();
            }
            let mut content = stack.bands;
            // Floating pictures inside cells are positioned relative to the cell (approximation).
            for (top, f) in stack.floats {
                let img = &f.img;
                let (ix, iy) = img
                    .anchor
                    .as_ref()
                    .map_or((0.0, 0.0), |a| (a.x.to_pt(), a.y.to_pt()));
                let mut band = Band::empty(0.0);
                if let Some(d) = &f.drawing {
                    let (items, lines) = d.placed(ix, 0.0, f.para);
                    band.items.extend(items);
                    band.lines.extend(lines);
                    content.push((top + iy, band));
                    continue;
                }
                band.items.extend(crate::picstyle::picture_items(
                    img,
                    ix,
                    0.0,
                    img.width.to_pt(),
                    img.height.to_pt(),
                    if img.anchor.as_ref().is_some_and(|a| a.behind) {
                        Layer::Back
                    } else {
                        Layer::Front
                    },
                    1.0,
                    0,
                ));
                content.push((top + iy, band));
            }
            let mut content_h = stack.h;
            let mut v_align = cell.props.v_align.unwrap_or(CellVAlign::Top);
            if rot != 0 {
                // The stacked lines turn into columns `stack.h` wide; §17.4.84 vAlign
                // then places them across the cell (top = where the lines begin).
                let free = (w - ml - mr - stack.h).max(0.0);
                let shift = match (v_align, rot > 0) {
                    (CellVAlign::Center, _) => free / 2.0,
                    (CellVAlign::Top, true) | (CellVAlign::Bottom, false) => free,
                    _ => 0.0,
                };
                content = vec![(0.0, rotate_content(content, rot, stack.h, vlen, shift))];
                content_h = vlen;
                // Bottom-to-top lines start at the bottom of the cell.
                v_align = if rot > 0 {
                    CellVAlign::Top
                } else {
                    CellVAlign::Bottom
                };
            }
            let first_row = ri == 0;
            let last_row = ri + 1 == nrows;
            let first_col = g == 0;
            let last_col = g + span >= ncols;
            let own = cell.props.borders.as_ref();
            let pick = |own_e: Option<&Option<Border>>,
                        outer: &Option<Border>,
                        inner: &Option<Border>,
                        is_outer: bool| {
                own_e
                    .and_then(Clone::clone)
                    .or_else(|| {
                        if is_outer {
                            outer.clone()
                        } else {
                            inner.clone()
                        }
                    })
                    .filter(Border::is_visible)
            };
            let edges = [
                pick(own.map(|b| &b.top), &tb.top, &tb.inside_h, first_row),
                pick(own.map(|b| &b.right), &tb.right, &tb.inside_v, last_col),
                pick(own.map(|b| &b.bottom), &tb.bottom, &tb.inside_h, last_row),
                pick(own.map(|b| &b.left), &tb.left, &tb.inside_v, first_col),
            ];
            let merged_below = !pl.hidden
                && ri + 1 < nrows
                && place_at(ri + 1, g).is_some_and(|c| c.v_merge == Some(VMerge::Continue));
            let rows_spanned = if continued || pl.hidden {
                1
            } else {
                span_rows(ri, g)
            };
            // The row's content starts below its top border and the table's bottom
            // border closes the last row: the reference word processor makes a single-line
            // Table Grid row 0.5 pt (sz 4) taller than its line (measured black-box,
            // docs/ui/REPORT.md Fidelity round 1; §17.4.38 tcBorders, §17.4.39 tblBorders).
            let bw = |b: &Option<Border>| b.as_ref().map_or(0.0, |b| b.size as f32 / 8.0);
            // §17.4.42 tblCellMar top / bottom add to the borders (measured black-box:
            // 7.2 / 3.6 pt margins make a Table Grid row 0.48 + 10.8 pt taller than its line).
            let inset = match spacing {
                Some(s) => [
                    2.0 * s + if first_row { bw(&frame_edges[0]) } else { 0.0 },
                    if last_row {
                        2.0 * s + bw(&frame_edges[2])
                    } else {
                        0.0
                    },
                ],
                None => [0.0, 0.0],
            };
            let mt = inset[0] + bw(&edges[0]) + cell_top;
            let mb = if spacing.is_some() {
                bw(&edges[2]) + inset[1]
            } else if last_row {
                bw(&edges[2])
            } else {
                0.0
            } + cell_bottom;
            let needed = content_h + mt + mb;
            if rows_spanned > 1 {
                tall.push((ri, rows_spanned, needed));
            } else if !continued {
                h = h.max(needed);
            }
            // §17.13.5.4 cellIns / §17.13.5.3 cellDel shown in markup.
            let revision = match (&cell.props.del, &cell.props.ins) {
                (Some(m), _) if markup => Some((crate::style::author_color(&m.info.author), true)),
                (None, Some(m)) if markup => {
                    Some((crate::style::author_color(&m.info.author), false))
                }
                _ => None,
            };
            cells.push(CellBox {
                x: cx,
                w,
                content,
                content_h,
                margins: [ml, mr, mt, mb],
                shading: if pl.hidden {
                    None
                } else {
                    crate::charfx::mixed_shading(
                        cell.props.shading,
                        cell.props.shading_pattern.as_ref(),
                    )
                },
                pattern: if pl.hidden {
                    None
                } else {
                    crate::charfx::shade_pattern(
                        cell.props.shading,
                        cell.props.shading_pattern.as_ref(),
                    )
                },
                edges: if pl.hidden {
                    [None, None, None, None]
                } else {
                    edges
                },
                diagonals: if pl.hidden || continued {
                    [None, None]
                } else {
                    cell.props
                        .diagonals
                        .clone()
                        .map(|d| d.filter(Border::is_visible))
                },
                v_align,
                continued,
                merged_below,
                rows_spanned,
                span_below: 0.0,
                revision,
                inset,
            });
        }
        // Row height rule (§17.4.81 trHeight).
        match row.props.height {
            Some((v, HeightRule::Exact)) => h = v.to_pt(),
            Some((v, HeightRule::AtLeast)) => h = h.max(v.to_pt()),
            None => {}
        }
        // Tracked rows (§17.13.5.19 ins / §17.13.5.12 del in trPr): hidden in the final
        // / original views (their paragraphs keep their ids), tinted in markup.
        let hidden = match ctx.view {
            RevisionView::Markup => false,
            RevisionView::Final => row.props.del.is_some(),
            RevisionView::Original => row.props.ins.is_some(),
        };
        if hidden {
            for c in &mut cells {
                c.content.clear();
                c.content_h = 0.0;
                c.shading = None;
                c.edges = [None, None, None, None];
                c.diagonals = [None, None];
            }
            h = 0.0;
        }
        let revision = match (&row.props.del, &row.props.ins) {
            (Some(m), _) if markup => Some((crate::style::author_color(&m.info.author), true)),
            (None, Some(m)) if markup => Some((crate::style::author_color(&m.info.author), false)),
            _ => None,
        };
        // Change bar: cell/row/table property changes, merges, grid changes, cells.
        let changed = row
            .cells
            .iter()
            .find_map(|c| {
                let p = &c.props;
                p.change
                    .as_ref()
                    .map(|ch| &ch.info)
                    .or(p.merge.as_ref().map(|m| &m.info))
                    .or(p.ins.as_ref().map(|m| &m.info))
                    .or(p.del.as_ref().map(|m| &m.info))
            })
            .or(row.props.change.as_ref().map(|ch| &ch.info))
            .or(t.props.change.as_ref().map(|ch| &ch.info))
            .filter(|_| markup)
            .map(|i| crate::style::author_color(&i.author));
        // §17.13.5.37 trPrChange: the original view repeats the previous header rows.
        let header = match (&row.props.change, ctx.view) {
            (Some(ch), RevisionView::Original) => ch.header,
            _ => row.props.header,
        };
        rows.push(RowBox {
            h: if hidden { 0.0 } else { h.max(1.0) },
            cells,
            header,
            cant_split: row.props.cant_split,
            revision,
            changed,
            frame: spacing.map(|_| {
                Box::new(SpacedFrame {
                    x0: 0.0,
                    x1: total,
                    edges: frame_edges.clone(),
                    first: ri == 0,
                    last: ri + 1 == nrows,
                })
            }),
        });
    }
    for (ri, span, need) in tall {
        let end = (ri + span).min(rows.len());
        let have: f32 = rows[ri..end].iter().map(|r| r.h).sum();
        if need > have {
            if let Some(last) = rows.get_mut(end - 1) {
                last.h += need - have;
            }
        }
    }
    for ri in 0..rows.len() {
        for ci in 0..rows[ri].cells.len() {
            let span = rows[ri].cells[ci].rows_spanned;
            if span > 1 {
                let end = (ri + span).min(rows.len());
                let below: f32 = rows[ri + 1..end].iter().map(|r| r.h).sum();
                rows[ri].cells[ci].span_below = below;
            }
        }
    }
    Ok(TableBox {
        x,
        rows,
        float: t.props.float.clone(),
        overlap_never: t.props.overlap.as_deref() == Some("never"),
    })
}

/// Render a row (or a split part of it) `h` tall as a band. `force_top` / `force_bottom`
/// draw the edge even where a merge or page split would normally hide it.
pub(crate) fn row_band(
    row: &RowBox,
    h: f32,
    table_x: f32,
    force_top: bool,
    force_bottom: bool,
) -> Band {
    let mut band = Band::empty(h);
    let mut row_end = None;
    if let (Some(first), Some(last)) = (row.cells.first(), row.cells.last()) {
        let (rx0, rx1) = (table_x + first.x, table_x + last.x + last.w);
        // The end-of-row mark's place: right of the row, as tall as the first line of
        // its last cell.
        let mt = last.margins[2];
        let (top, lh, base) = last
            .content
            .iter()
            .find_map(|(t, b)| {
                b.lines
                    .first()
                    .map(|l| (mt + t + l.top, l.height, mt + t + l.baseline))
            })
            .unwrap_or((mt, h.min(14.0), mt + h.min(14.0) * 0.8));
        row_end = Some(Line {
            top,
            height: lh,
            baseline: base,
            fragments: Vec::new(),
            source: LineSource::RowEnd,
            start: 0,
            end: 0,
            carets: vec![(0, rx1)].into(),
            rotation: None,
        });
        if let Some((color, deleted)) = row.revision {
            // Light tint of the author's colour behind the row; deleted rows struck out.
            let tint = |c: u8| (f32::from(c) * 0.2 + 255.0 * 0.8) as u8;
            band.items.push(PageItem::Rect {
                x: rx0,
                y: 0.0,
                w: rx1 - rx0,
                h,
                color: Color {
                    r: tint(color.r),
                    g: tint(color.g),
                    b: tint(color.b),
                },
                layer: Layer::Back,
                pattern: None,
            });
            if deleted {
                band.items.push(PageItem::Rule {
                    x1: rx0,
                    y1: h / 2.0,
                    x2: rx1,
                    y2: h / 2.0,
                    width: 1.0,
                    color,
                    style: "single".into(),
                });
            }
        }
        if let Some(color) = row.changed.or(row.revision.map(|r| r.0)) {
            // Change bar left of the table.
            band.items.push(PageItem::Rule {
                x1: rx0 - 6.0,
                y1: 0.0,
                x2: rx0 - 6.0,
                y2: h,
                width: 1.5,
                color,
                style: "single".into(),
            });
        }
    }
    for c in &row.cells {
        let x0 = table_x + c.x;
        let x1 = x0 + c.w;
        if let Some((color, deleted)) = c.revision {
            // Tracked cell: tint of the author's colour; deleted cells crossed out.
            let tint = |v: u8| (f32::from(v) * 0.25 + 255.0 * 0.75) as u8;
            band.items.push(PageItem::Rect {
                x: x0,
                y: 0.0,
                w: c.w,
                h,
                color: Color {
                    r: tint(color.r),
                    g: tint(color.g),
                    b: tint(color.b),
                },
                layer: Layer::Back,
                pattern: None,
            });
            if deleted {
                band.items.push(PageItem::Rule {
                    x1: x0,
                    y1: 0.0,
                    x2: x1,
                    y2: h,
                    width: 0.75,
                    color,
                    style: "single".into(),
                });
            }
        }
        let spaced = row.frame.is_some();
        let (cy0, cy1) = if spaced {
            (c.inset[0], (h - c.inset[1]).max(c.inset[0]))
        } else {
            (0.0, h)
        };
        if let Some(color) = c.shading {
            band.items.push(PageItem::Rect {
                x: x0,
                y: cy0,
                w: c.w,
                h: cy1 - cy0,
                color,
                layer: Layer::Back,
                pattern: c.pattern,
            });
        }
        if !c.continued {
            let [ml, _, mt, mb] = c.margins;
            let free = (h + c.span_below - mt - mb - c.content_h).max(0.0);
            // §17.4.84 vAlign
            let dy = mt
                + match c.v_align {
                    CellVAlign::Top => 0.0,
                    CellVAlign::Center => free / 2.0,
                    CellVAlign::Bottom => free,
                };
            for (top, b) in &c.content {
                band.absorb(b.translated(x0 + ml, dy + top));
            }
        }
        let [top, right, bottom, left] = &c.edges;
        if spaced {
            // §17.4.44: every cell is boxed on its own.
            if let Some(b) = top {
                band.items.push(rule(b, x0, cy0, x1, cy0));
            }
            if let Some(b) = bottom {
                band.items.push(rule(b, x0, cy1, x1, cy1));
            }
            if let Some(b) = left {
                band.items.push(rule(b, x0, cy0, x0, cy1));
            }
            if let Some(b) = right {
                band.items.push(rule(b, x1, cy0, x1, cy1));
            }
            continue;
        }
        if let Some(b) = top.as_ref().filter(|_| force_top || !c.continued) {
            band.items.push(rule(b, x0, 0.0, x1, 0.0));
        }
        if let Some(b) = bottom.as_ref().filter(|_| force_bottom || !c.merged_below) {
            band.items.push(rule(b, x0, h, x1, h));
        }
        if let Some(b) = left {
            band.items.push(rule(b, x0, 0.0, x0, h));
        }
        if let Some(b) = right {
            band.items.push(rule(b, x1, 0.0, x1, h));
        }
        // Diagonal borders run corner to corner across the whole (merged) cell box
        // (§17.4.80 tl2br, §17.4.79 tr2bl); a vMerge continuation has none of its own.
        // The reference word processor joins the inner corners of the cell's borders
        // (measured in its PDF export: the line starts where the top and left border
        // strokes end).
        if c.diagonals.iter().any(Option::is_some) {
            let half = |e: &Option<Border>| e.as_ref().map_or(0.0, |b| border_width(b) / 2.0);
            let bottom = if c.edges[2].is_some() {
                &c.edges[2]
            } else {
                &c.edges[0]
            };
            let (l, r) = (x0 + half(left), x1 - half(right));
            let (t, b) = (half(top), h + c.span_below - half(bottom));
            if let Some(d) = &c.diagonals[0] {
                band.items.push(rule(d, l, t, r, b));
            }
            if let Some(d) = &c.diagonals[1] {
                band.items.push(rule(d, r, t, l, b));
            }
        }
        // Gridlines where a cell edge has no border (View Gridlines).
        if TABLE_GRID.with(std::cell::Cell::get) {
            if top.is_none() && (force_top || !c.continued) {
                band.items.push(grid_rule(x0, 0.0, x1, 0.0));
            }
            if bottom.is_none() && (force_bottom || !c.merged_below) {
                band.items.push(grid_rule(x0, h, x1, h));
            }
            if left.is_none() {
                band.items.push(grid_rule(x0, 0.0, x0, h));
            }
            if right.is_none() {
                band.items.push(grid_rule(x1, 0.0, x1, h));
            }
        }
    }
    // The table's own frame around cells set apart (§17.4.44 tblCellSpacing).
    if let Some(f) = &row.frame {
        let (x0, x1) = (table_x + f.x0, table_x + f.x1);
        let [top, right, bottom, left] = &f.edges;
        if let Some(b) = top.as_ref().filter(|_| f.first || force_top) {
            band.items.push(rule(b, x0, 0.0, x1, 0.0));
        }
        if let Some(b) = bottom.as_ref().filter(|_| f.last || force_bottom) {
            band.items.push(rule(b, x0, h, x1, h));
        }
        if let Some(b) = left {
            band.items.push(rule(b, x0, 0.0, x0, h));
        }
        if let Some(b) = right {
            band.items.push(rule(b, x1, 0.0, x1, h));
        }
    }
    // After the cells' lines (the first line of a row stays its first cell's).
    band.lines.extend(row_end);
    band
}

/// Cell spacing of a table in points (§17.4.44 tblCellSpacing, kept verbatim among the
/// unknown `w:tblPr` children; `dxa` widths only).
fn cell_spacing(p: &bo_model::TableProps) -> Option<f32> {
    p.unknown.iter().find_map(|r| {
        let x: &str = &r.xml;
        let at = x.find("tblCellSpacing")?;
        let el = &x[at..];
        let el = &el[..el.find('>').unwrap_or(el.len())];
        if el.contains("w:type=\"pct\"") || el.contains("w:type=\"nil\"") {
            return None;
        }
        let v = el.split("w:w=\"").nth(1)?;
        let v = &v[..v.find('"')?];
        v.parse::<f32>().ok().map(|tw| tw / 20.0)
    })
}

/// Notes referenced anywhere in a row.
pub(crate) fn row_notes(row: &RowBox) -> Vec<NoteKey> {
    row.cells
        .iter()
        .flat_map(|c| c.content.iter().flat_map(|(_, b)| b.notes.iter().copied()))
        .collect()
}

/// Bands of a cell with their tops.
type Bands = Vec<(f32, Band)>;

/// Widow / orphan control of the paragraph a row split cuts through (ECMA-376 Part 1,
/// §17.3.1.44 widowControl), as the body flow does: one line left after the break
/// takes another with it, and a paragraph's first line alone before the break moves
/// on. `a` / `b` are a cell's bands before / after the break, in content order.
fn keep_widow_lines(mut a: Bands, mut b: Bands) -> (Bands, Bands) {
    let para = |band: &Band| {
        band.widow_control
            .then(|| band.lines.first().map(|l| l.source))
            .flatten()
    };
    let Some(src) = b.first().and_then(|(_, band)| para(band)) else {
        return (a, b);
    };
    let before = a
        .iter()
        .rev()
        .take_while(|(_, band)| para(band) == Some(src))
        .count();
    if before == 0 {
        return (a, b);
    }
    let after = b
        .iter()
        .take_while(|(_, band)| para(band) == Some(src))
        .count();
    let mut take = before;
    if after == 1 && take >= 2 {
        take -= 1; // widow: never leave the last line alone
    }
    let starts_here = a[a.len() - before]
        .1
        .lines
        .first()
        .is_some_and(|l| l.start == 0);
    if take == 1 && starts_here {
        take = 0; // orphan: never leave the first line alone
    }
    let moved = a.split_off(a.len() - (before - take));
    b.splice(0..0, moved);
    (a, b)
}

/// Split a row so that the first part is `avail` tall (ECMA-376 Part 1, §17.4.6 cantSplit
/// unset: rows may break across pages). Returns `None` when nothing fits or nothing remains.
pub(crate) fn split_row(row: &RowBox, avail: f32) -> Option<(RowBox, RowBox)> {
    let mut first = row.clone();
    let mut rest = row.clone();
    let mut any_first = false;
    let mut any_rest = false;
    let mut rest_h = 0.0f32;
    for (ci, c) in row.cells.iter().enumerate() {
        let [_, _, mt, mb] = c.margins;
        let limit = avail - mt - mb;
        let (a, b): (Vec<_>, Vec<_>) = c
            .content
            .iter()
            .cloned()
            .partition(|(top, band)| top + band.h <= limit + 0.01);
        let had_text = a.iter().any(|(_, band)| !band.lines.is_empty());
        let (a, b) = keep_widow_lines(a, b);
        if had_text && a.iter().all(|(_, band)| band.lines.is_empty()) {
            // Widow control left this cell nothing before the break: as measured in the
            // reference word processor, the whole row moves on rather than splitting.
            return None;
        }
        let base = b.iter().map(|(t, _)| *t).fold(f32::INFINITY, f32::min);
        let b: Vec<(f32, Band)> = b.into_iter().map(|(t, band)| (t - base, band)).collect();
        let bh = b.iter().map(|(t, band)| t + band.h).fold(0.0f32, f32::max);
        any_first |= !a.is_empty();
        any_rest |= !b.is_empty();
        rest_h = rest_h.max(bh + mt + mb);
        let fc = &mut first.cells[ci];
        fc.content_h = a.iter().map(|(t, band)| t + band.h).fold(0.0f32, f32::max);
        fc.content = a;
        fc.v_align = CellVAlign::Top;
        fc.span_below = 0.0;
        let rc = &mut rest.cells[ci];
        rc.content_h = bh;
        rc.content = b;
        rc.v_align = CellVAlign::Top;
        rc.span_below = 0.0;
    }
    if !any_first || !any_rest {
        return None;
    }
    // The part before the break ends below its last line (its bottom border drawn
    // there, as in the reference word processor), not at the page bottom.
    first.h = first
        .cells
        .iter()
        .map(|c| c.content_h + c.margins[2] + c.margins[3])
        .fold(0.0f32, f32::max)
        .min(avail);
    rest.h = rest_h.max(1.0);
    Some((first, rest))
}

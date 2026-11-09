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
use crate::types::{Layer, Line, LineSource, NoteKey, PageItem};
use crate::{LayoutError, RevisionView};
use bo_model::{
    Block, Border, Borders, CellPlace, CellVAlign, Color, Document, HeightRule, Image, Paragraph,
    RunContent, Table, TableGeometry, TableView, TableWidth, Twips, VMerge,
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
    pub lines: Vec<Line>,
    pub items: Vec<PageItem>,
    pub notes: Vec<NoteKey>,
    pub brk: Brk,
}

impl Band {
    pub fn empty(h: f32) -> Band {
        Band {
            h,
            lines: Vec::new(),
            items: Vec::new(),
            notes: Vec::new(),
            brk: Brk::None,
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
#[derive(Debug, Clone)]
pub(crate) struct Deco {
    pub shading: Option<Color>,
    pub borders: Option<Borders>,
    pub x0: f32,
    pub x1: f32,
}

/// A laid-out paragraph ready for placement.
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
}

impl ParaBlock {
    pub fn height(&self) -> f32 {
        self.lines.iter().map(|b| b.h).sum()
    }
}

/// Crop fractions l,t,r,b from `a:srcRect` (1/1000 of a percent, DrawingML §20.1.8.55).
pub(crate) fn crop_of(img: &Image) -> Option<[f32; 4]> {
    img.crop.map(|c| c.map(|v| v as f32 / 100_000.0))
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
    for &(x, w, color) in &rl.highlights {
        items.push(PageItem::Rect {
            x,
            y: rl.baseline - rl.ascent,
            w,
            h: rl.ascent + rl.descent,
            color,
            layer: Layer::Back,
        });
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
            items.push(PageItem::Image {
                x: o.x,
                y,
                w: o.w,
                h: o.h,
                media: o.image.media.clone(),
                crop: crop_of(&o.image),
                rotation: rotation_of(&o.image),
                layer: Layer::Front,
            });
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
    }];
    lines.extend(boxed);
    Band {
        h: rl.height,
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
    let lines = pl.lines.iter().map(|l| rel_line_band(l, source)).collect();
    let visible_borders = pl.borders.as_ref().is_some_and(|b| {
        [&b.top, &b.left, &b.bottom, &b.right]
            .iter()
            .any(|e| e.as_ref().is_some_and(Border::is_visible))
    });
    let deco = (pl.shading.is_some() || visible_borders).then(|| Deco {
        shading: pl.shading,
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

/// Shading and border items of a paragraph segment spanning `y0..y1`.
pub(crate) fn deco_items(d: &Deco, y0: f32, y1: f32, out: &mut Vec<PageItem>) {
    if let Some(color) = d.shading {
        out.push(PageItem::Rect {
            x: d.x0,
            y: y0,
            w: (d.x1 - d.x0).max(0.0),
            h: (y1 - y0).max(0.0),
            color,
            layer: Layer::Back,
        });
    }
    if let Some(b) = &d.borders {
        let vis = |e: &Option<Border>| e.clone().filter(Border::is_visible);
        let pad = |e: &Border| e.space as f32;
        if let Some(e) = vis(&b.top) {
            let y = y0 - pad(&e);
            out.push(rule(&e, d.x0, y, d.x1, y));
        }
        if let Some(e) = vis(&b.bottom) {
            let y = y1 + pad(&e);
            out.push(rule(&e, d.x0, y, d.x1, y));
        }
        if let Some(e) = vis(&b.left) {
            let x = d.x0 - pad(&e);
            out.push(rule(&e, x, y0, x, y1));
        }
        if let Some(e) = vis(&b.right) {
            let x = d.x1 + pad(&e);
            out.push(rule(&e, x, y0, x, y1));
        }
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
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                // The note number mark belongs to the first paragraph of a note only.
                let mark = if first_para { note_mark } else { None };
                first_para = false;
                let pb = build_para(ctx, p, width, body, mark)?;
                let mut before = pb.space_before;
                if let Some((ps, pc, pa)) = &prev {
                    // §17.3.1.9 contextualSpacing: ignore spacing between paragraphs of the same style.
                    if *ps == pb.style {
                        if pb.contextual {
                            before = 0.0;
                        }
                        if *pc {
                            y -= pa;
                        }
                    }
                }
                y += before;
                let y0 = y;
                let first_idx = st.bands.len();
                for band in pb.lines {
                    let h = band.h;
                    st.bands.push((y, band));
                    y += h;
                }
                if let Some(d) = &pb.deco {
                    if let Some((top, band)) = st.bands.get_mut(first_idx) {
                        let mut items = Vec::new();
                        deco_items(d, 0.0, y - *top, &mut items);
                        let _ = y0;
                        band.items.extend(items);
                    }
                }
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
            }
            Block::Opaque(_) => {}
        }
    }
    st.h = y.max(0.0);
    Ok(st)
}

// ------------------------------------------------------------------ tables

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
    /// top, right, bottom, left (visible borders only)
    pub edges: [Option<Border>; 4],
    pub v_align: CellVAlign,
    /// `vMerge` continuation (content drawn by the restart cell above).
    pub continued: bool,
    /// The cell below (same grid column) continues this cell's merge.
    pub merged_below: bool,
    /// Tracked cell insertion/deletion in markup (§17.13.5.4 cellIns, §17.13.5.3
    /// cellDel): (author colour, deleted).
    pub revision: Option<(Color, bool)>,
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
}

/// A laid-out table.
#[derive(Debug, Clone)]
pub(crate) struct TableBox {
    /// Offset of the table's left edge from the content box left.
    pub x: f32,
    pub rows: Vec<RowBox>,
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
    let grid = grid_widths(t, &geo, avail);
    let ncols = grid.len();
    let total: f32 = grid.iter().sum();
    let indent = t.props.indent.map_or(0.0, Twips::to_pt);
    // §17.4.29 jc on tables.
    let x = match t.props.align {
        Some(bo_model::Alignment::Center) => (avail - total) / 2.0,
        Some(bo_model::Alignment::Right) => avail - total,
        _ => indent,
    };
    let tb = table_borders(ctx.doc, t);
    let (ml, mr) = t
        .props
        .cell_margin
        .map_or((DEFAULT_CELL_MARGIN, DEFAULT_CELL_MARGIN), |(l, r)| {
            (l.to_pt(), r.to_pt())
        });
    let nrows = t.rows.len();

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
            let continued = pl.v_merge == Some(VMerge::Continue);
            let mut stack = if continued {
                Stack::default()
            } else {
                // Hidden cells are laid out too (at an ordinary width): their
                // paragraphs keep their ids.
                let lw = if pl.hidden {
                    avail / ncols.max(1) as f32
                } else {
                    w
                };
                stack_blocks(ctx, &cell.blocks, (lw - ml - mr).max(1.0), body, None)?
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
                band.items.push(PageItem::Image {
                    x: ix,
                    y: 0.0,
                    w: img.width.to_pt(),
                    h: img.height.to_pt(),
                    media: img.media.clone(),
                    crop: crop_of(img),
                    rotation: rotation_of(img),
                    layer: if img.anchor.as_ref().is_some_and(|a| a.behind) {
                        Layer::Back
                    } else {
                        Layer::Front
                    },
                });
                content.push((top + iy, band));
            }
            let content_h = stack.h;
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
            let needed = content_h;
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
                margins: [ml, mr, 0.0, 0.0],
                shading: if pl.hidden { None } else { cell.props.shading },
                edges: if pl.hidden {
                    [None, None, None, None]
                } else {
                    edges
                },
                v_align: cell.props.v_align.unwrap_or(CellVAlign::Top),
                continued,
                merged_below,
                revision,
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
    Ok(TableBox { x, rows })
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
    if let (Some(first), Some(last)) = (row.cells.first(), row.cells.last()) {
        let (rx0, rx1) = (table_x + first.x, table_x + last.x + last.w);
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
        if let Some(color) = c.shading {
            band.items.push(PageItem::Rect {
                x: x0,
                y: 0.0,
                w: c.w,
                h,
                color,
                layer: Layer::Back,
            });
        }
        if !c.continued {
            let [ml, _, mt, mb] = c.margins;
            let free = (h - mt - mb - c.content_h).max(0.0);
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
    }
    band
}

/// Notes referenced anywhere in a row.
pub(crate) fn row_notes(row: &RowBox) -> Vec<NoteKey> {
    row.cells
        .iter()
        .flat_map(|c| c.content.iter().flat_map(|(_, b)| b.notes.iter().copied()))
        .collect()
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
        let rc = &mut rest.cells[ci];
        rc.content_h = bh;
        rc.content = b;
        rc.v_align = CellVAlign::Top;
    }
    if !any_first || !any_rest {
        return None;
    }
    first.h = avail;
    rest.h = rest_h.max(1.0);
    Some((first, rest))
}

//! Pagination of the main story: sections, columns, keep rules, widow/orphan
//! control, tables split across pages, anchored pictures and footnotes.

use crate::blocks::{
    build_para, deco_items, layout_table, rewrap_para, row_band, row_notes, split_row,
    stack_blocks, stack_note, Band, Brk, Ctx, ParaBlock, Stack, TableBox,
};
use crate::drawing::Float;
use crate::paragraph::{WrapZone, MIN_SEGMENT};
use crate::style::ListCounters;
use crate::types::{Layer, Line, LineRotation, LineSource, NoteKey, Page, PageItem};
use crate::LayoutError;
use bo_model::{
    Block, Color, DropCap, Emu, FrameProps, LineNumberRestart, LineNumbering, NotePos, PageSetup,
    Paragraph, Section, SectionBreak, Twips, Wrap, WrapText,
};
use std::collections::HashMap;
use std::sync::Arc;

const EPS: f32 = 0.01;
/// Space reserved above the footnote area for the separator (ECMA-376 Part 1, §17.11.23 separator).
const NOTE_SEPARATOR: f32 = 12.0;

#[derive(Debug, Clone, PartialEq)]
struct Prev {
    style: Option<String>,
    contextual: bool,
    after: f32,
}

/// Lay paragraph `p` out again for a column of width `w` continuing it after its
/// first `placed` lines (ECMA-376 Part 1, §17.6.3 `col/@w`: unequal columns): the
/// placed `parts` (top, bottom, width) keep their widths, the rest takes `w`. `None`
/// when the placed lines would not come out the same.
fn rewrap_across_columns(
    ctx: &mut Ctx<'_>,
    p: &Paragraph,
    pb: &ParaBlock,
    parts: &[(f32, f32, f32)],
    from: f32,
    w: f32,
    placed: usize,
) -> Result<Option<ParaBlock>, LayoutError> {
    let wide = parts.iter().map(|q| q.2).fold(w, f32::max);
    let zone = |top: f32, bottom: f32, width: f32| WrapZone {
        top,
        bottom,
        left: width,
        right: wide + 1.0,
        side: WrapText::Left,
        outline: None,
    };
    let mut zones: Vec<WrapZone> = parts
        .iter()
        .filter(|q| q.2 < wide - 0.01)
        .map(|q| zone(q.0, q.1 - 0.01, q.2))
        .collect();
    if w < wide - 0.01 {
        zones.push(zone(from, f32::MAX / 4.0, w));
    }
    let again = rewrap_para(ctx, p, pb, wide, &zones)?;
    let end = |b: &ParaBlock| {
        b.lines
            .get(placed.wrapping_sub(1))
            .and_then(|l| l.lines.first())
            .map(|l| l.end)
    };
    Ok((again.lines.len() > placed && end(&again) == end(pb)).then_some(again))
}

/// Spacing between two paragraphs (ECMA-376 Part 1, §17.3.1.33 spacing `w:before` /
/// `w:after`). The reference word processor keeps the larger of the previous paragraph's
/// space after and the next one's space before rather than their sum (measured black-box
/// with 12/6, 6/12 and 12/12 pt: 12 pt each time, docs/ui/REPORT.md Fidelity round 1).
/// `y` already holds `added_after` (the previous paragraph's own space after); it is taken
/// back, and the space still to add above the next paragraph is returned.
pub(crate) fn collapse_spacing(y: &mut f32, added_after: f32, after: f32, before: f32) -> f32 {
    *y -= added_after;
    after.max(before)
}

/// (header height, footer height) of a section's default, first and even variants.
pub(crate) type HfHeights = [(f32, f32); 3];

/// Pagination state between two top-level blocks, without the pages themselves.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Snap {
    setup: PageSetup,
    section: usize,
    cols: Vec<(f32, f32)>,
    col: usize,
    top: f32,
    bottom: f32,
    y: f32,
    col_has_content: bool,
    page_has_content: bool,
    notes_on_page: Vec<NoteKey>,
    foot_h: f32,
    foot_slack: f32,
    carry: Vec<(NoteKey, usize)>,
    text_bottom: Option<f32>,
    excl: Vec<(f32, f32)>,
    sides: Vec<SideZone>,
    prev: Option<Prev>,
    lists: ListCounters,
    other_lists: ListCounters,
    /// Length of [`Flow::foot_order`].
    pub foot_keys: usize,
    cols_top: f32,
    cols_low: f32,
    line_no: u32,
}

/// Where the flow stood before a top-level block (incremental layout, prompt §5.4).
#[derive(Debug, Clone)]
pub(crate) struct Checkpoint {
    pub snap: Snap,
    /// Displayed number of the current page.
    pub number: u32,
    /// Index of the current page and its line / item counts at that moment.
    pub page: usize,
    pub lines: usize,
    pub items: usize,
    /// Next body paragraph id.
    pub next_id: u32,
}

/// A pure paragraph built by an earlier layout, reusable at the same width.
pub(crate) struct CachedPara {
    pub width: f32,
    pub block: Arc<ParaBlock>,
}

/// The new flow reached a state of the previous layout: everything after is reused.
#[derive(Debug)]
pub(crate) struct Resync {
    pub section: usize,
    pub block: usize,
    /// The previous layout's checkpoint at that boundary.
    pub old: Checkpoint,
    /// The new flow's checkpoint at the same boundary.
    pub new: Checkpoint,
}

/// Pagination state.
pub(crate) struct Flow {
    pub pages: Vec<Page>,
    setup: PageSetup,
    section: usize,
    number: u32,
    /// (x, width) of each text column.
    cols: Vec<(f32, f32)>,
    col: usize,
    top: f32,
    bottom: f32,
    y: f32,
    col_has_content: bool,
    page_has_content: bool,
    notes_on_page: Vec<NoteKey>,
    foot_h: f32,
    /// Part of the footnote area that may continue on the next page: the last note
    /// beyond its first line (a note starts on the page of its reference).
    foot_slack: f32,
    /// Footnotes continued from the previous page: (note, first band shown here).
    carry: Vec<(NoteKey, usize)>,
    /// Bottom of the lowest body line on the current page (any column); part of the
    /// checkpoints because the footnote area depends on it.
    text_bottom: Option<f32>,
    /// Footnote continuation notice (`w:footnote w:type="continuationNotice"`).
    notice: Option<Stack>,
    /// Vertical ranges on the current page occupied by top-and-bottom pictures.
    excl: Vec<(f32, f32)>,
    /// Areas of square/tight-wrapped pictures on the current page (text flows beside them).
    sides: Vec<SideZone>,
    pub foot_cache: HashMap<NoteKey, Stack>,
    prev: Option<Prev>,
    /// Page index at which each section started on a fresh page.
    pub section_start: Vec<Option<usize>>,
    /// (header height, footer height) per section and variant, used to push the body.
    pub hf_heights: Vec<HfHeights>,
    /// §17.10.1 evenAndOddHeaders: even pages show the even header/footer.
    pub even_odd: bool,
    /// §17.10.6 titlePg of the current section.
    title_page: bool,
    /// A column break (§17.3.3.1 `w:br w:type="column"`) placed the current columns'
    /// content: they keep their layout (no balancing).
    col_break: bool,
    /// Body id of the first paragraph of the block being placed (lines carry relative ids).
    id_base: u32,
    /// Notes in the order their stacks were computed (`foot_cache` keys).
    pub foot_order: Vec<NoteKey>,
    /// Pure paragraphs built by the previous layout, per section and block.
    pub built: Vec<Vec<Option<CachedPara>>>,
    /// Checkpoint before each top-level block (`None` while a keepNext lookahead is
    /// pending). Slots at or after `resync_from` still hold the previous layout's.
    pub cps: Vec<Vec<Option<Checkpoint>>>,
    /// Per section, the first block boundary where the flow may re-synchronise with
    /// the previous layout (`usize::MAX`: never; empty: full layout).
    pub resync_from: Vec<usize>,
    /// Note order of the previous layout (re-synchronisation needs the same prefix).
    pub old_foot_order: Vec<NoteKey>,
    /// Re-synchronisation may change the displayed numbers of reused pages.
    pub allow_renumber: bool,
    /// Set when the flow stopped because it re-synchronised.
    pub resynced: Option<Resync>,
    /// Footnotes of the current section go right below the text (§17.11.21 pos beneathText).
    beneath: bool,
    /// §17.6.4 `w:sep` of the current section: a line between the columns.
    separator: bool,
    /// Top of the current section's columns on this page.
    cols_top: f32,
    /// Bottom of the lowest content of the current columns on this page.
    cols_low: f32,
    /// §17.6.8 lnNumType of the current section.
    line_numbers: Option<LineNumbering>,
    /// Number of the last numbered line.
    line_no: u32,
    /// Shaped line numbers by value.
    number_cache: HashMap<u32, Band>,
    /// A drop cap waiting for its paragraph (§17.3.1.11 framePr `w:dropCap`).
    drop: Option<PendingDrop>,
    /// Vertical section (§17.6.20 textDirection): the physical page and the turn
    /// that takes the pages laid out with swapped axes to it.
    vertical: Option<(PageSetup, LineRotation)>,
}

/// A drop cap paragraph laid out, placed beside the next paragraph's first lines.
struct PendingDrop {
    /// Its lines stacked (relative body ids).
    band: Band,
    /// Body id of the drop cap paragraph.
    base: u32,
    /// Width of its text.
    width: f32,
    frame: FrameProps,
}

/// Line numbers are this far left of the text when the distance is automatic.
const LINE_NUMBER_GAP: f32 = 18.0;

/// Geometry of a section's body area.
fn body_area(setup: &PageSetup, hf: (f32, f32)) -> (f32, f32) {
    let page_h = setup.height.to_pt();
    let mt = setup.margin_top.to_pt();
    let mb = setup.margin_bottom.to_pt();
    // §17.6.11 pgMar: a negative top/bottom margin is exact (the header does not push the body).
    let top = if mt < 0.0 {
        -mt
    } else {
        mt.max(setup.header.to_pt() + hf.0)
    };
    let bottom = if mb < 0.0 {
        page_h + mb
    } else {
        page_h - mb.max(setup.footer.to_pt() + hf.1)
    };
    (top, bottom.max(top + 36.0))
}

/// Column boxes (§17.6.4 cols): equal widths separated by `space`, or the widths
/// and spaces of each §17.6.3 col (`w:equalWidth` false), as given.
pub(crate) fn columns(setup: &PageSetup, sec: &Section) -> Vec<(f32, f32)> {
    let left = setup.margin_left.to_pt();
    let width =
        (setup.width.to_pt() - setup.margin_left.to_pt() - setup.margin_right.to_pt()).max(36.0);
    let c = &sec.columns;
    if c.unequal() {
        let defs = &c.cols[..c.len()];
        // Widths wider than the text area are kept as given: the reference word
        // processor lets the last column run into the right margin (measured black-box,
        // gen-06: 2000 / 3000 / 3646 twips + 2 × 360 on a 9026-twip line).
        let mut x = left;
        let mut out = Vec::with_capacity(defs.len());
        for d in defs {
            let w = d.width.to_pt().max(18.0);
            out.push((x, w));
            x += w + d.space.to_pt().max(0.0);
        }
        return out;
    }
    let n = c.len();
    let space = c.space.to_pt().max(0.0);
    let w = ((width - space * (n - 1) as f32) / n as f32).max(18.0);
    (0..n).map(|i| (left + i as f32 * (w + space), w)).collect()
}

/// Page setup with swapped axes for a vertical section (ECMA-376 Part 1, §17.6.20
/// textDirection) and the turn back to the page: lines run along the virtual
/// width (`tbRl`: top to bottom, stacked right to left; `btLr`: bottom to top,
/// stacked left to right). The body area is that of the physical page (`top`,
/// `bottom` from its header and footer).
pub(crate) fn vertical_setup(
    phys: &PageSetup,
    top: f32,
    bottom: f32,
    rot: i32,
) -> (PageSetup, LineRotation) {
    let (w, h) = (phys.width, phys.height);
    let tw = |pt: f32| Twips((pt * 20.0).round() as i32);
    let (ml, mr) = (phys.margin_left, phys.margin_right);
    let (start, end) = if rot > 0 {
        (tw(top), Twips((h.0 - tw(bottom).0).max(0)))
    } else {
        (Twips((h.0 - tw(bottom).0).max(0)), tw(top))
    };
    let (first, last) = if rot > 0 { (mr, ml) } else { (ml, mr) };
    let v = PageSetup {
        width: h,
        height: w,
        margin_left: start,
        margin_right: end,
        margin_top: first,
        margin_bottom: last,
        header: Twips(0),
        footer: Twips(0),
    };
    // Clockwise about (W/2, W/2) takes (x, y) to (W - y, x); anticlockwise about
    // (H/2, H/2) takes it to (y, H - x).
    let r = if rot > 0 {
        let c = w.to_pt() / 2.0;
        LineRotation {
            degrees: 90.0,
            cx: c,
            cy: c,
        }
    } else {
        let c = h.to_pt() / 2.0;
        LineRotation {
            degrees: -90.0,
            cx: c,
            cy: c,
        }
    };
    (v, r)
}

/// Position of an anchored picture (§20.4.2.3 anchor, §20.4.2.10 positionH, §20.4.2.11 positionV).
pub(crate) fn float_item(
    f: &Float,
    setup: &PageSetup,
    col_x: f32,
    para_top: f32,
) -> (Vec<PageItem>, Vec<Line>, Option<Excl>) {
    let img = &f.img;
    let w = img.width.to_pt();
    let h = img.height.to_pt();
    let draw = |x: f32, y: f32, layer: Layer| -> (Vec<PageItem>, Vec<Line>) {
        match &f.drawing {
            Some(d) => d.placed(x, y, f.para),
            None => (
                vec![PageItem::Image {
                    x,
                    y,
                    w,
                    h,
                    media: img.display_id(),
                    crop: crate::blocks::crop_of(img),
                    rotation: crate::blocks::rotation_of(img),
                    layer,
                    alpha: 1.0,
                    z: 0,
                }],
                Vec::new(),
            ),
        }
    };
    let Some(a) = &img.anchor else {
        let (items, lines) = draw(col_x, para_top, Layer::Front);
        return (items, lines, None);
    };
    // Reference area of positionH / positionV (§20.4.3.4 ST_RelFromH, §20.4.3.5
    // ST_RelFromV): (start, extent).
    let page_w = setup.width.to_pt();
    let page_h = setup.height.to_pt();
    let (ml, mr) = (setup.margin_left.to_pt(), setup.margin_right.to_pt());
    let (mt, mb) = (
        setup.margin_top.to_pt().abs(),
        setup.margin_bottom.to_pt().abs(),
    );
    let href = match a.h_relative.as_str() {
        "page" => (0.0, page_w),
        "leftMargin" | "insideMargin" => (0.0, ml),
        "rightMargin" | "outsideMargin" => (page_w - mr, mr),
        "margin" => (ml, page_w - ml - mr),
        _ => (col_x, page_w - ml - mr),
    };
    let vref = match a.v_relative.as_str() {
        "page" => (0.0, page_h),
        "topMargin" | "insideMargin" => (0.0, mt),
        "margin" => (mt, page_h - mt - mb),
        "bottomMargin" | "outsideMargin" => (page_h - mb, mb),
        _ => (para_top, 0.0),
    };
    // §20.4.2.1 / §20.4.2.2 align replaces the offset.
    let aligned = |al: &str, (start, ext): (f32, f32), size: f32| match al {
        "center" => start + (ext - size) / 2.0,
        "right" | "bottom" | "outside" => start + ext - size,
        _ => start,
    };
    let x = match &a.h_align {
        Some(al) => aligned(al, href, w),
        None => href.0 + a.x.to_pt(),
    };
    let y = match &a.v_align {
        Some(al) => aligned(al, vref, h),
        None => vref.0 + a.y.to_pt(),
    };
    let (mut items, lines) = draw(x, y, if a.behind { Layer::Back } else { Layer::Front });
    // §20.4.2.3 relativeHeight: the stacking order among floating objects.
    let z = a.relative_height.saturating_add(1);
    for i in &mut items {
        i.set_z(z);
    }
    let [dt, db, dl, dr] = a.dist.map(Emu::to_pt);
    let excl = match a.wrap {
        Wrap::None => None,
        // §20.4.2.19 wrapTopAndBottom: no text beside the picture.
        Wrap::TopAndBottom => Some(Excl::Band(y - dt, y + h + db)),
        // §20.4.2.17 wrapSquare: the picture's box plus distT/B/L/R.
        Wrap::Square => Some(Excl::Side(SideZone {
            y0: y - dt,
            y1: y + h + db,
            x0: x - dl,
            x1: x + w + dr,
            side: a.wrap_text,
            outline: None,
        })),
        // §20.4.2.20 wrapTight: text follows the edges of the wrap polygon
        // (§20.4.2.16, in 21 600ths of the extent) plus distL/R line by line;
        // distT/B do not apply. The zone is the polygon's bounding box.
        Wrap::Tight | Wrap::Through => {
            let [px0, py0, px1, py1] = a
                .polygon
                .map_or([0.0, 0.0, 1.0, 1.0], |p| p.map(|v| v as f32 / 21_600.0));
            let outline = (a.polygon_points.len() >= 3).then(|| Outline {
                points: a
                    .polygon_points
                    .iter()
                    .map(|&(px, py)| (x + w * px as f32 / 21_600.0, y + h * py as f32 / 21_600.0))
                    .collect(),
                pad: (dl, dr),
            });
            Some(Excl::Side(SideZone {
                y0: y + h * py0.min(py1),
                y1: y + h * py0.max(py1),
                x0: x + w * px0.min(px1) - dl,
                x1: x + w * px0.max(px1) + dr,
                side: a.wrap_text,
                outline,
            }))
        }
    };
    (items, lines, excl)
}

/// Revision of the run holding floating picture `img` in `p`: (author colour,
/// deleted).
fn float_revision(p: &Paragraph, img: &bo_model::Image) -> Option<(Color, bool)> {
    let r = p.runs.iter().find(|r| {
        r.content
            .iter()
            .any(|c| matches!(c, bo_model::RunContent::Image(i) if **i == *img))
    })?;
    match (&r.props.del, &r.props.ins) {
        (Some(d), _) => Some((crate::style::author_color(&d.info.author), true)),
        (None, Some(i)) => Some((crate::style::author_color(&i.info.author), false)),
        _ => None,
    }
}

/// Frame (and, for a deleted picture, cross) around a tracked floating picture.
fn float_frame(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    (color, deleted): (Color, bool),
    out: &mut Vec<PageItem>,
) {
    let mut seg = |x1: f32, y1: f32, x2: f32, y2: f32| {
        out.push(PageItem::Rule {
            x1,
            y1,
            x2,
            y2,
            width: 1.5,
            color,
            style: if deleted { "dashed" } else { "single" }.into(),
        });
    };
    seg(x, y, x + w, y);
    seg(x, y + h, x + w, y + h);
    seg(x, y, x, y + h);
    seg(x + w, y, x + w, y + h);
    if deleted {
        seg(x, y, x + w, y + h);
        seg(x, y + h, x + w, y);
    }
}

/// Area of a square/tight-wrapped picture on the page (points, distances included).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SideZone {
    pub y0: f32,
    pub y1: f32,
    pub x0: f32,
    pub x1: f32,
    pub side: WrapText,
    /// Tight wrap: the wrap polygon on the page (points) and `distL`/`distR`;
    /// lines are kept clear of the polygon's extent at their height only.
    pub outline: Option<Outline>,
}

/// Wrap polygon of a tight-wrapped picture (ECMA-376 Part 1, §20.4.2.16) with
/// the left / right distances from text (§20.4.2.20 `distL` `distR`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Outline {
    pub points: std::sync::Arc<[(f32, f32)]>,
    pub pad: (f32, f32),
}

impl Outline {
    /// The same outline moved by `(-dx, -dy)`.
    pub(crate) fn offset(&self, dx: f32, dy: f32) -> Outline {
        Outline {
            points: self.points.iter().map(|&(x, y)| (x - dx, y - dy)).collect(),
            pad: self.pad,
        }
    }

    /// Horizontal extent `(left, right)` of the polygon between heights `y0`
    /// and `y1`, distances included; `None` when the polygon does not reach
    /// that band.
    pub(crate) fn extent(&self, y0: f32, y1: f32) -> Option<(f32, f32)> {
        let pts = &self.points;
        let n = pts.len();
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for i in 0..n {
            let (ax, ay) = pts[i];
            let (bx, by) = pts[(i + 1) % n];
            // The part of edge a→b inside the band.
            let (t0, t1) = if (by - ay).abs() < f32::EPSILON {
                if ay < y0 || ay > y1 {
                    continue;
                }
                (0.0, 1.0)
            } else {
                let ta = ((y0 - ay) / (by - ay)).clamp(0.0, 1.0);
                let tb = ((y1 - ay) / (by - ay)).clamp(0.0, 1.0);
                let (t0, t1) = (ta.min(tb), ta.max(tb));
                let ym = ay + (by - ay) * (t0 + t1) / 2.0;
                if ym < y0 - 0.001 || ym > y1 + 0.001 {
                    continue;
                }
                (t0, t1)
            };
            for t in [t0, t1] {
                let x = ax + (bx - ax) * t;
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        (lo <= hi).then_some((lo - self.pad.0, hi + self.pad.1))
    }
}

/// Text exclusion of an anchored picture.
pub(crate) enum Excl {
    /// Lines skip this vertical range (top and bottom).
    Band(f32, f32),
    /// Lines flow beside this area.
    Side(SideZone),
}

enum Built {
    Para(Arc<ParaBlock>),
    Table(TableBox),
    Skip,
}

impl Flow {
    pub fn new(hf_heights: Vec<HfHeights>, sections: usize) -> Flow {
        Flow {
            pages: Vec::new(),
            setup: PageSetup::default(),
            section: 0,
            number: 0,
            cols: vec![(0.0, 100.0)],
            col: 0,
            top: 0.0,
            bottom: 0.0,
            y: 0.0,
            col_has_content: false,
            page_has_content: false,
            notes_on_page: Vec::new(),
            foot_h: 0.0,
            foot_slack: 0.0,
            carry: Vec::new(),
            text_bottom: None,
            notice: None,
            excl: Vec::new(),
            sides: Vec::new(),
            foot_cache: HashMap::new(),
            prev: None,
            section_start: vec![None; sections],
            hf_heights,
            even_odd: false,
            title_page: false,
            col_break: false,
            id_base: 0,
            foot_order: Vec::new(),
            built: Vec::new(),
            cps: Vec::new(),
            resync_from: Vec::new(),
            old_foot_order: Vec::new(),
            allow_renumber: false,
            resynced: None,
            beneath: false,
            separator: false,
            cols_top: 0.0,
            cols_low: 0.0,
            line_numbers: None,
            line_no: 0,
            number_cache: HashMap::new(),
            drop: None,
            vertical: None,
        }
    }

    /// The current state.
    fn checkpoint(&self, ctx: &Ctx<'_>) -> Checkpoint {
        let (lines, items) = self
            .pages
            .last()
            .map_or((0, 0), |p| (p.lines.len(), p.items.len()));
        Checkpoint {
            snap: Snap {
                setup: self.setup,
                section: self.section,
                cols: self.cols.clone(),
                col: self.col,
                top: self.top,
                bottom: self.bottom,
                y: self.y,
                col_has_content: self.col_has_content,
                page_has_content: self.page_has_content,
                notes_on_page: self.notes_on_page.clone(),
                foot_h: self.foot_h,
                foot_slack: self.foot_slack,
                carry: self.carry.clone(),
                text_bottom: self.text_bottom,
                excl: self.excl.clone(),
                sides: self.sides.clone(),
                prev: self.prev.clone(),
                lists: ctx.lists.clone(),
                other_lists: ctx.other_lists.clone(),
                foot_keys: self.foot_order.len(),
                cols_top: self.cols_top,
                cols_low: self.cols_low,
                line_no: self.line_no,
            },
            number: self.number,
            page: self.pages.len().saturating_sub(1),
            lines,
            items,
            next_id: ctx.next_id,
        }
    }

    /// Continue from `cp` (the caller has restored the pages up to `cp.page`).
    pub fn restore(&mut self, ctx: &mut Ctx<'_>, cp: &Checkpoint) {
        let s = &cp.snap;
        self.setup = s.setup;
        self.section = s.section;
        self.cols.clone_from(&s.cols);
        self.col = s.col;
        self.top = s.top;
        self.bottom = s.bottom;
        self.y = s.y;
        self.col_has_content = s.col_has_content;
        self.page_has_content = s.page_has_content;
        self.notes_on_page.clone_from(&s.notes_on_page);
        self.foot_h = s.foot_h;
        self.foot_slack = s.foot_slack;
        self.carry.clone_from(&s.carry);
        self.text_bottom = s.text_bottom;
        self.excl.clone_from(&s.excl);
        self.sides.clone_from(&s.sides);
        self.prev.clone_from(&s.prev);
        self.cols_top = s.cols_top;
        self.cols_low = s.cols_low;
        self.line_no = s.line_no;
        if let Some(sec) = ctx.doc.sections.get(s.section) {
            self.separator = sec.columns.separator;
            self.line_numbers = sec.line_numbers;
        }
        self.drop = None;
        self.number = cp.number;
        ctx.lists.clone_from(&s.lists);
        ctx.other_lists.clone_from(&s.other_lists);
        ctx.next_id = cp.next_id;
    }

    /// Record the checkpoint before block `k` of section `si`; returns `true` (and
    /// stops the flow) when it matches the previous layout's checkpoint there.
    fn boundary(&mut self, ctx: &Ctx<'_>, si: usize, k: usize, pending: bool) -> bool {
        if self.cps.get(si).is_none_or(|v| k >= v.len()) {
            return false;
        }
        if pending {
            self.cps[si][k] = None;
            return false;
        }
        let new = self.checkpoint(ctx);
        let eligible = self.resync_from.get(si).is_some_and(|&from| k >= from);
        let slot = &mut self.cps[si][k];
        if eligible {
            if let Some(old) = slot.take() {
                let n = new.snap.foot_keys;
                let same = old.snap == new.snap
                    && self.old_foot_order.get(..n) == Some(&self.foot_order[..n])
                    && (old.number == new.number || self.allow_renumber);
                if same {
                    *slot = Some(new.clone());
                    self.resynced = Some(Resync {
                        section: si,
                        block: k,
                        old,
                        new,
                    });
                    return true;
                }
            }
        }
        *slot = Some(new);
        false
    }

    fn col_x(&self) -> f32 {
        self.cols[self.col].0
    }

    fn col_w(&self) -> f32 {
        self.cols[self.col].1
    }

    fn push_page(&mut self) {
        let phys = self.vertical.map_or(self.setup, |v| v.0);
        self.pages.push(Page {
            width: phys.width.to_pt(),
            height: phys.height.to_pt(),
            lines: Vec::new(),
            items: Vec::new(),
            number: self.number,
            section: self.section,
            decor: crate::types::PageDecor::default(),
        });
        self.col = 0;
        self.y = self.top;
        self.cols_top = self.top;
        self.cols_low = self.top;
        // §17.18.47 newPage: numbering starts again on each page.
        if let Some(ln) = self.line_numbers {
            if ln.restart == LineNumberRestart::NewPage {
                self.line_no = ln.start;
            }
        }
        self.col_has_content = false;
        self.page_has_content = false;
        self.col_break = false;
        self.notes_on_page.clear();
        self.foot_h = 0.0;
        self.foot_slack = 0.0;
        self.text_bottom = None;
        // Footnotes continued from the previous page (ECMA-376 Part 1, §17.11.1
        // continuationSeparator) open the footnote area.
        if !self.carry.is_empty() {
            self.foot_h = NOTE_SEPARATOR;
            for &(key, from) in &self.carry {
                let (rem, first) = self.remaining(key, from);
                self.foot_h += rem;
                self.foot_slack = rem - first;
            }
        }
        self.excl.clear();
        self.sides.clear();
        self.prev = None;
    }

    /// Height of note `key` from band `from` on, and of that first band (with the
    /// space above it).
    fn remaining(&self, key: NoteKey, from: usize) -> (f32, f32) {
        let Some(st) = self.foot_cache.get(&key) else {
            return (0.0, 0.0);
        };
        let top = st.bands.get(from).map_or(st.h, |b| b.0);
        let first = st.bands.get(from).map_or(0.0, |(t, b)| t - top + b.h);
        ((st.h - top).max(0.0), first)
    }

    /// Place the footnotes of the current page at its bottom. When the text took the
    /// space of the last note beyond its first line, the notes are placed below the
    /// text and what does not fit continues on the next page (ECMA-376 Part 1,
    /// §17.11.1 continuationSeparator; the continuation notice, §17.18.33
    /// `continuationNotice`, ends the part shown here).
    fn finish_page(&mut self) {
        self.close_columns();
        self.place_page_notes();
        self.turn_page();
    }

    /// Balance the columns of a section ended by a continuous section break on this page
    /// (ECMA-376 Part 1, §17.18.77 `continuous`): the reference word processor evens out
    /// the columns' heights there instead of filling the first column (measured
    /// black-box, docs/ui/REPORT.md Fidelity round 1). The section's lines on this page
    /// are poured again, in order, into columns of the smallest height that holds them;
    /// shading and borders move with their lines. Pages with tables or floating objects
    /// in the columns keep their layout.
    fn balance_columns(&mut self) {
        let n = self.cols.len();
        if n < 2 || self.vertical.is_some() || std::mem::take(&mut self.col_break) {
            return;
        }
        let top = self.cols_top;
        let cols = self.cols.clone();
        let Some(page) = self.pages.last_mut() else {
            return;
        };
        let col_of = |x: f32| cols.iter().rposition(|&(cx, _)| x >= cx - 1.0).unwrap_or(0);
        // Lines of this section's columns: (line index, column).
        let mut mine: Vec<(usize, usize)> = Vec::new();
        for (i, l) in page.lines.iter().enumerate() {
            if l.top < top - EPS || l.rotation.is_some() {
                continue;
            }
            if l.source == LineSource::RowEnd {
                return;
            }
            let x = l
                .fragments
                .iter()
                .map(|f| f.x)
                .fold(f32::MAX, f32::min)
                .min(l.carets.first().map_or(f32::MAX, |c| c.1));
            let c = if x < f32::MAX {
                col_of(x)
            } else {
                mine.last().map_or(0, |m| m.1)
            };
            mine.push((i, c));
        }
        let in_region = |y: f32| y >= top - EPS;
        if page.items.iter().any(|it| {
            matches!(it, PageItem::Image { y, .. } | PageItem::Shape { y, .. } if in_region(*y))
        }) {
            return;
        }
        if mine.len() < 2 {
            return;
        }
        mine.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then(page.lines[a.0].top.total_cmp(&page.lines[b.0].top))
        });
        // Units: (line, space above it within its column, height).
        let mut units: Vec<(usize, usize, f32, f32)> = Vec::new();
        let mut prev: Option<(usize, f32)> = None;
        for &(i, c) in &mine {
            let l = &page.lines[i];
            let gap = match prev {
                Some((pc, bottom)) if pc == c => (l.top - bottom).max(0.0),
                _ => 0.0,
            };
            units.push((i, c, gap, l.height));
            prev = Some((c, l.top + l.height));
        }
        let lead = mine
            .first()
            .map_or(0.0, |m| (page.lines[m.0].top - top).max(0.0));
        let fill = |limit: f32| -> Option<Vec<(usize, f32)>> {
            let mut out = Vec::with_capacity(units.len());
            let (mut col, mut y) = (0usize, lead);
            for (k, &(_, _, gap, h)) in units.iter().enumerate() {
                let start = if k == 0 {
                    lead
                } else if y <= EPS {
                    0.0
                } else {
                    y + gap
                };
                if start + h > limit + EPS && y > EPS {
                    col += 1;
                    if col >= n {
                        return None;
                    }
                    y = 0.0;
                    out.push((col, 0.0));
                    y += h;
                    continue;
                }
                out.push((col, start));
                y = start + h;
            }
            Some(out)
        };
        let total: f32 = lead + units.iter().map(|u| u.2 + u.3).sum::<f32>();
        let tallest = units.iter().map(|u| u.3).fold(0.0f32, f32::max);
        let mut limit = (total / n as f32).max(tallest);
        let high = self.cols_low - top;
        let plan = loop {
            if limit > high + EPS {
                return;
            }
            if let Some(p) = fill(limit) {
                break p;
            }
            limit += 0.5;
        };
        // Move the lines, and the shading / border items beside them.
        let mut moves: Vec<(f32, f32, f32, f32, usize, f32, f32)> = Vec::new();
        for (&(i, c, _, _), &(nc, ny)) in units.iter().zip(&plan) {
            let l = &mut page.lines[i];
            let dx = cols[nc].0 - cols[c].0;
            let dy = top + ny - l.top;
            moves.push((
                l.top,
                l.top + l.height,
                cols[c].0,
                cols[c].0 + cols[c].1,
                c,
                dx,
                dy,
            ));
            if dx.abs() > EPS || dy.abs() > EPS {
                l.translate(dx, dy);
            }
        }
        for it in &mut page.items {
            let (x, y) = match it {
                PageItem::Rect { x, y, .. } => (*x, *y),
                PageItem::Rule { x1, y1, .. } => (*x1, *y1),
                _ => continue,
            };
            if !in_region(y) {
                continue;
            }
            if let Some(m) = moves
                .iter()
                .find(|m| y >= m.0 - EPS && y < m.1 - EPS && x >= m.2 - 1.0 && x <= m.3 + 1.0)
            {
                it.translate(m.5, m.6);
            }
        }
        let low = plan
            .iter()
            .zip(&units)
            .map(|(&(_, ny), u)| top + ny + u.3)
            .fold(top, f32::max);
        self.cols_low = low;
        // The last paragraph's space after stays below it (taken back when the next
        // paragraph collapses its spacing).
        self.y = low + self.prev.as_ref().map_or(0.0, |p| p.after);
    }

    /// Balance columns of unequal widths (ECMA-376 Part 1, §17.6.3 `col/@w`) before a
    /// continuous section break: lines cannot move between columns of other widths, so
    /// the section — when it began on this page and holds only paragraphs — is laid out
    /// again with the columns' bottom raised to the lowest height that still holds it
    /// on this page (the reference word processor's balanced columns, measured
    /// black-box, docs/ui/REPORT.md "Dàn trang vòng 2").
    fn balance_unequal_columns(
        &mut self,
        ctx: &mut Ctx<'_>,
        si: usize,
        start: &Checkpoint,
    ) -> Result<(), LayoutError> {
        let doc = ctx.doc;
        let (Some(sec), Some(next)) = (doc.sections.get(si), doc.sections.get(si + 1)) else {
            return Ok(());
        };
        let continuous = matches!(
            next.break_type,
            SectionBreak::Continuous | SectionBreak::NextColumn
        ) && next.page.width == sec.page.width
            && next.page.height == sec.page.height
            && next.text_direction == sec.text_direction;
        let n = self.cols.len();
        let unequal = self.cols.iter().any(|c| (c.1 - self.cols[0].1).abs() > EPS);
        if !continuous
            || !unequal
            || n < 2
            || self.vertical.is_some()
            || self.col_break
            || self.drop.is_some()
            || self.pages.len() != start.page + 1
            || self.notes_on_page != start.snap.notes_on_page
            || !sec.blocks.iter().all(|b| matches!(b, Block::Paragraph(_)))
        {
            return Ok(());
        }
        // A document's first section starts before its first page exists.
        let top = start.snap.cols_top.max(self.top);
        let tallest = self.pages.last().map_or(0.0, |p| {
            p.lines[start.lines.min(p.lines.len())..]
                .iter()
                .fold(0.0f32, |m, l| m.max(l.height))
        });
        let full = self.bottom;
        let used = self.cols_low - top;
        // The lowest column bottom that keeps the section on this page (bisection to
        // half a point; the columns' widths make the line count depend on it).
        let (mut lo, mut hi) = (tallest, used);
        while hi - lo > 0.5 {
            let mid = f32::midpoint(lo, hi);
            self.rewind(ctx, start);
            self.bottom = top + mid;
            self.place_section_paragraphs(ctx, si)?;
            if self.pages.len() == start.page + 1 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        self.rewind(ctx, start);
        self.bottom = if hi < used - EPS { top + hi } else { full };
        let mark = self.place_section_paragraphs(ctx, si)?;
        self.bottom = full;
        // The empty paragraph carrying the section break does not lower the columns
        // (measured black-box: the next section follows the lowest text line).
        if let (Some(m), Some(page)) = (mark, self.pages.last()) {
            if self.pages.len() == start.page + 1 {
                self.cols_low = page.lines[start.lines.min(m)..m]
                    .iter()
                    .filter(|l| l.top >= top - EPS)
                    .map(|l| l.top + l.height)
                    .fold(top, f32::max);
            }
        }
        // As after moving lines ([`Flow::balance_columns`]): the next section starts
        // below the lowest column, the last paragraph's space after kept below it.
        self.y = self.cols_low + self.prev.as_ref().map_or(0.0, |p| p.after);
        // Balanced already: the section break leaves the columns as they are.
        self.col_break = true;
        // The trial layouts recorded no reusable checkpoints in this section.
        if let Some(v) = self.cps.get_mut(si) {
            for c in v.iter_mut() {
                *c = None;
            }
        }
        Ok(())
    }

    /// Back to `start` on its page (pages and content placed since are dropped).
    fn rewind(&mut self, ctx: &mut Ctx<'_>, start: &Checkpoint) {
        self.pages.truncate(start.page + 1);
        if let Some(page) = self.pages.last_mut() {
            page.lines.truncate(start.lines);
            page.items.truncate(start.items);
        }
        self.foot_order.truncate(start.snap.foot_keys);
        self.restore(ctx, start);
    }

    /// Place the paragraphs of section `si` from its start (see
    /// [`Flow::balance_unequal_columns`]). Returns the page's line count before the
    /// section's last paragraph when that paragraph is empty (it only carries the
    /// section break).
    fn place_section_paragraphs(
        &mut self,
        ctx: &mut Ctx<'_>,
        si: usize,
    ) -> Result<Option<usize>, LayoutError> {
        let doc = ctx.doc;
        let Some(sec) = doc.sections.get(si) else {
            return Ok(None);
        };
        let mut mark = None;
        for (k, b) in sec.blocks.iter().enumerate() {
            let (built, base) = self.build(ctx, si, k, b)?;
            self.id_base = base;
            if let (Built::Para(pb), Block::Paragraph(p)) = (&built, b) {
                if k + 1 == sec.blocks.len() && p.plain_text().is_empty() {
                    mark = self.pages.last().map(|pg| pg.lines.len());
                }
                self.place_para(ctx, pb, 0.0, Some(p))?;
            }
        }
        Ok(mark)
    }

    /// Place the empty paragraph that only carries a section break before a continuous
    /// section (ECMA-376 Part 1, §17.6.17 sectPr in pPr, §17.6.22 type continuous): the
    /// reference word processor gives it no height and no spacing — the next section's
    /// first line follows the previous text line with the spacing between those two
    /// paragraphs (measured black-box, docs/ui/REPORT.md "Dàn trang vòng 3", cal-32:
    /// one → one, one → two and two → one columns, with and without space after).
    fn place_section_mark(
        &mut self,
        ctx: &mut Ctx<'_>,
        pb: &ParaBlock,
        src: Option<&Paragraph>,
    ) -> Result<(), LayoutError> {
        let mut mark = pb.clone();
        mark.space_before = 0.0;
        mark.space_after = 0.0;
        mark.page_break_before = false;
        mark.deco = None;
        for l in &mut mark.lines {
            l.h = 0.0;
            l.text_h = 0.0;
        }
        let prev = self.prev.clone();
        let y = self.y;
        self.place_para(ctx, &mark, 0.0, src)?;
        // The spacing between the paragraphs around the mark collapses as if it were not there.
        self.y = self.y.max(y);
        self.prev = prev;
        Ok(())
    }

    /// Separator lines between the current columns (ECMA-376 Part 1, §17.6.4
    /// `w:sep`), as tall as the longest column on this page.
    fn close_columns(&mut self) {
        if self.separator && self.cols.len() > 1 && self.cols_low > self.cols_top + EPS {
            let (y1, y2) = (self.cols_top, self.cols_low);
            let xs: Vec<f32> = self
                .cols
                .windows(2)
                .map(|w| (w[0].0 + w[0].1 + w[1].0) / 2.0)
                .collect();
            if let Some(page) = self.pages.last_mut() {
                for x in xs {
                    page.items.push(PageItem::Rule {
                        x1: x,
                        y1,
                        x2: x,
                        y2,
                        width: 0.5,
                        color: Color::BLACK,
                        style: "single".into(),
                    });
                }
            }
        }
        self.cols_top = self.cols_low.max(self.y);
    }

    /// Turn the page of a vertical section (§17.6.20 textDirection) from its
    /// swapped axes to the physical page.
    fn turn_page(&mut self) {
        let Some((_, r)) = self.vertical else {
            return;
        };
        let Some(page) = self.pages.last_mut() else {
            return;
        };
        for l in &mut page.lines {
            if l.rotation.is_none() {
                l.rotation = Some(r);
            }
        }
        for it in &mut page.items {
            crate::blocks::rotate_item(it, &r);
        }
    }

    fn place_page_notes(&mut self) {
        if self.notes_on_page.is_empty() && self.carry.is_empty() {
            return;
        }
        let x = self.cols[0].0;
        let full_w = (self.setup.width.to_pt()
            - self.setup.margin_left.to_pt()
            - self.setup.margin_right.to_pt())
        .max(36.0);
        let mut y = self.bottom - self.foot_h;
        let Some(page) = self.pages.last_mut() else {
            return;
        };
        let text = self.text_bottom;
        let split = text.is_some_and(|t| t > y + EPS);
        if split {
            y = text.unwrap_or(y);
        } else if self.beneath {
            // §17.11.21 pos beneathText: below the last text line of the page.
            if let Some(t) = text {
                y = y.min(t);
            }
        }
        let notice_h = self.notice.as_ref().map_or(0.0, |n| n.h);
        let limit = self.bottom - if split { notice_h } else { 0.0 };
        let sep = |page: &mut Page, y: f32, w: f32| {
            page.items.push(PageItem::Rule {
                x1: x,
                y1: y + NOTE_SEPARATOR / 2.0,
                x2: x + w,
                y2: y + NOTE_SEPARATOR / 2.0,
                width: 0.5,
                color: Color::BLACK,
                style: "single".into(),
            });
        };
        // §17.11.1 continuationSeparator spans the text width; §17.11.23 separator.
        sep(page, y, if self.carry.is_empty() { 144.0 } else { full_w });
        y += NOTE_SEPARATOR;
        let entries: Vec<(NoteKey, usize)> = self
            .carry
            .iter()
            .copied()
            .chain(self.notes_on_page.iter().map(|k| (*k, 0)))
            .collect();
        let mut carry: Vec<(NoteKey, usize)> = Vec::new();
        let mut placed_any = false;
        for (key, from) in entries {
            if !carry.is_empty() {
                carry.push((key, from));
                continue;
            }
            let Some(st) = self.foot_cache.get(&key) else {
                continue;
            };
            let top0 = st.bands.get(from).map_or(st.h, |b| b.0);
            for (i, (top, band)) in st.bands.iter().enumerate().skip(from) {
                let by = y + top - top0;
                if split && placed_any && by + band.h > limit + EPS {
                    carry.push((key, i));
                    break;
                }
                let mut b = band.translated(x, by);
                b.set_other();
                page.lines.extend(b.lines);
                page.items.extend(b.items);
                placed_any = true;
            }
            if carry.is_empty() {
                y += st.h - top0;
            } else if let Some(&(_, i)) = carry.first() {
                y += st.bands.get(i).map_or(0.0, |b| b.0) - top0;
            }
        }
        if !carry.is_empty() {
            if let Some(n) = &self.notice {
                for (top, band) in &n.bands {
                    let mut b = band.translated(x, y + top);
                    b.set_other();
                    page.lines.extend(b.lines);
                    page.items.extend(b.items);
                }
            }
        }
        self.carry = carry;
    }

    fn new_page(&mut self) {
        self.finish_page();
        self.number += 1;
        self.push_page();
        self.page_area();
    }

    fn next_column(&mut self) {
        if self.col + 1 < self.cols.len() {
            self.col += 1;
            // Columns of a section starting on this page begin below the previous one.
            self.y = self.cols_top;
            self.col_has_content = false;
            self.prev = None;
        } else {
            self.new_page();
        }
    }

    /// Height added to the footnote area by placing content that references `notes`,
    /// and the footnote slack after it (see [`Flow::foot_slack`]).
    fn notes_need(
        &mut self,
        ctx: &mut Ctx<'_>,
        notes: &[NoteKey],
    ) -> Result<(f32, f32), LayoutError> {
        let mut add = 0.0;
        let mut slack = self.foot_slack;
        let mut seen: Vec<NoteKey> = Vec::new();
        for key in notes {
            if key.0 || self.on_page(*key) || seen.contains(key) {
                continue;
            }
            seen.push(*key);
            let h = self.note_stack(ctx, *key)?;
            add += h;
            slack = h - self.remaining(*key, 0).1;
        }
        if add > 0.0 && self.foot_h <= 0.0 {
            add += NOTE_SEPARATOR;
        }
        Ok((add, slack))
    }

    /// The note is already in this page's footnote area.
    fn on_page(&self, key: NoteKey) -> bool {
        self.notes_on_page.contains(&key) || self.carry.iter().any(|c| c.0 == key)
    }

    fn note_stack(&mut self, ctx: &mut Ctx<'_>, key: NoteKey) -> Result<f32, LayoutError> {
        if let Some(s) = self.foot_cache.get(&key) {
            return Ok(s.h);
        }
        let blocks = if key.0 {
            ctx.doc.endnotes.get(&key.1)
        } else {
            ctx.doc.footnotes.get(&key.1)
        };
        let mark = ctx.notes.get(&key).cloned();
        self.foot_order.push(key);
        let st = match blocks {
            Some(b) => {
                let width = (self.setup.width.to_pt()
                    - self.setup.margin_left.to_pt()
                    - self.setup.margin_right.to_pt())
                .max(36.0);
                stack_note(ctx, key, b, width, mark.as_deref())?
            }
            None => Stack::default(),
        };
        let h = st.h;
        self.foot_cache.insert(key, st);
        Ok(h)
    }

    fn register_notes(&mut self, ctx: &mut Ctx<'_>, notes: &[NoteKey]) -> Result<(), LayoutError> {
        for key in notes {
            if key.0 || self.on_page(*key) {
                continue;
            }
            let h = self.note_stack(ctx, *key)?;
            if self.foot_h <= 0.0 {
                self.foot_h += NOTE_SEPARATOR;
            }
            self.foot_h += h;
            self.foot_slack = self.later_slack(h, self.remaining(*key, 0).1);
            self.notes_on_page.push(*key);
        }
        Ok(())
    }

    fn skip_excl(&self, mut y: f32, h: f32) -> f32 {
        for _ in 0..16 {
            let mut moved = false;
            for &(a, b) in &self.excl {
                if y < b - EPS && y + h > a + EPS {
                    y = b;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        y
    }

    /// Emit a band at (current column x, `y`).
    fn put(&mut self, ctx: &mut Ctx<'_>, band: &Band, y: f32) -> Result<(), LayoutError> {
        self.register_notes(ctx, &band.notes)?;
        let b = band.translated_ids(self.col_x(), y, self.id_base);
        if let Some(t) = b
            .lines
            .iter()
            .filter(|l| matches!(l.source, LineSource::Body(_)))
            .map(|l| l.top + l.height)
            .reduce(f32::max)
        {
            self.text_bottom = Some(self.text_bottom.map_or(t, |x| x.max(t)));
        }
        if let Some(page) = self.pages.last_mut() {
            page.lines.extend(b.lines);
            page.items.extend(b.items);
        }
        self.y = y + band.h;
        self.cols_low = self.cols_low.max(self.y);
        self.col_has_content = true;
        self.page_has_content = true;
        Ok(())
    }

    /// Number body line `line` just placed at `y` (ECMA-376 Part 1, §17.6.8
    /// lnNumType): every counted line advances the number, multiples of
    /// `countBy` are drawn right-aligned `distance` left of the column.
    fn number_line(&mut self, ctx: &mut Ctx<'_>, line: &Band, y: f32) -> Result<(), LayoutError> {
        let Some(ln) = self.line_numbers else {
            return Ok(());
        };
        let Some(base) = line
            .lines
            .iter()
            .find(|l| matches!(l.source, LineSource::Body(_)))
            .map(|l| l.baseline)
        else {
            return Ok(());
        };
        self.line_no += 1;
        let n = self.line_no;
        if !n.is_multiple_of(u32::from(ln.count_by.max(1))) {
            return Ok(());
        }
        let band = if let Some(b) = self.number_cache.get(&n) {
            b.clone()
        } else {
            let b = number_band(ctx, n)?;
            self.number_cache.insert(n, b.clone());
            b
        };
        let Some(first) = band.lines.first() else {
            return Ok(());
        };
        let right = first
            .fragments
            .iter()
            .map(|f| f.x + f.width)
            .fold(0.0f32, f32::max);
        let gap = ln.distance.map_or(LINE_NUMBER_GAP, Twips::to_pt);
        let dx = self.col_x() - gap - right;
        let dy = y + base - first.baseline;
        let b = band.translated(dx, dy);
        if let Some(page) = self.pages.last_mut() {
            page.lines.extend(b.lines);
            page.items.extend(b.items);
        }
        Ok(())
    }

    /// Place a waiting drop cap beside paragraph `pb` starting at the current
    /// position (ECMA-376 Part 1, §17.3.1.11 framePr): `lines` lines of `pb` tall,
    /// its baseline on the last of them; `drop` keeps those lines `hSpace` clear of
    /// it, `margin` puts it in the left margin.
    fn place_drop(&mut self, d: &PendingDrop, pb: &ParaBlock) {
        let Some(first) = pb.lines.first() else {
            return;
        };
        let lh = first.h.max(1.0);
        let n = f32::from(d.frame.lines.max(1));
        let first_base = first
            .lines
            .iter()
            .find(|l| matches!(l.source, LineSource::Body(_)))
            .map_or(lh * 0.8, |l| l.baseline);
        let cap_base = d
            .band
            .lines
            .iter()
            .find(|l| matches!(l.source, LineSource::Body(_)))
            .map_or(d.band.h, |l| l.baseline);
        let gap = d.frame.h_space.to_pt().max(0.0);
        let x = match d.frame.drop_cap {
            DropCap::Drop => self.col_x(),
            DropCap::Margin => self.col_x() - d.width - gap,
        };
        let dy = self.y + (n - 1.0) * lh + first_base - cap_base;
        let b = d.band.translated_ids(x, dy, d.base);
        if let Some(page) = self.pages.last_mut() {
            page.lines.extend(b.lines);
            page.items.extend(b.items);
        }
        self.page_has_content = true;
        if d.frame.drop_cap == DropCap::Drop {
            let cx = self.col_x();
            self.sides.push(SideZone {
                y0: self.y,
                y1: self.y + n * lh - 0.5,
                x0: cx - 1.0,
                x1: cx + d.width + gap,
                side: WrapText::Right,
                outline: None,
            });
        }
    }

    /// Content `h` tall fits at `y` with `notes_add` more footnote height, of which
    /// `slack` may continue on the next page.
    fn fits(&self, y: f32, h: f32, notes_add: f32, slack: f32) -> bool {
        y + h <= self.bottom - self.foot_h - notes_add + slack + EPS
    }

    /// Place an unsplittable band, moving to the next column/page when needed.
    fn place_band(&mut self, ctx: &mut Ctx<'_>, band: &Band) -> Result<(), LayoutError> {
        loop {
            let (add, slack) = self.notes_need(ctx, &band.notes)?;
            let y = self.skip_excl(self.y, band.h);
            if self.fits(y, band.h, add, slack) || !self.col_has_content {
                self.put(ctx, band, y)?;
                break;
            }
            self.next_column();
        }
        match band.brk {
            Brk::Page => self.new_page(),
            Brk::Column => {
                self.col_break = true;
                self.next_column();
            }
            Brk::None => {}
        }
        Ok(())
    }

    /// How many of `lines` fit from the current position.
    fn fit_count(&mut self, ctx: &mut Ctx<'_>, lines: &[Band]) -> Result<usize, LayoutError> {
        let mut y = self.y;
        let mut foot = self.foot_h;
        let mut slack = self.foot_slack;
        let mut seen: Vec<NoteKey> = self.notes_on_page.clone();
        seen.extend(self.carry.iter().map(|c| c.0));
        for (i, l) in lines.iter().enumerate() {
            // The line holding a note's reference may leave the note's rest to the next
            // page; the lines after it only when the note could not fit on one page.
            let mut here = slack;
            for key in &l.notes {
                if !key.0 && !seen.contains(key) {
                    if foot <= 0.0 {
                        foot += NOTE_SEPARATOR;
                    }
                    let h = self.note_stack(ctx, *key)?;
                    foot += h;
                    let first = self.remaining(*key, 0).1;
                    here = h - first;
                    slack = self.later_slack(h, first);
                    seen.push(*key);
                }
            }
            let ly = self.skip_excl(y, l.text_h);
            // The extra space a multiple of single spacing adds below the text may pass the
            // bottom margin (only the text must fit), but not into a footnote area on the
            // page: measured black-box, docs/ui/REPORT.md "Dàn trang vòng 3" (cal-31:
            // Calibri 11 pt at 1.15 / 1.5 lines, with and without footnotes).
            let need = if foot > 0.0 { l.h } else { l.text_h };
            if ly + need > self.bottom - foot + here + EPS {
                return Ok(i);
            }
            y = ly + l.h;
            if l.brk != Brk::None {
                return Ok(i + 1);
            }
        }
        Ok(lines.len())
    }

    /// Part of a footnote `h` tall (first band `first`) that body lines placed after its
    /// reference may push to the next page. The reference word processor keeps a note
    /// that fits on a page whole with its reference and moves the following body lines
    /// to the next page instead (measured black-box, notes-05: 24-paragraph note); only
    /// a note too tall for that continues on the next page (§17.11.1
    /// continuationSeparator).
    fn later_slack(&self, h: f32, first: f32) -> f32 {
        if h + NOTE_SEPARATOR + 2.0 * first < self.full_height() {
            0.0
        } else {
            h - first
        }
    }

    fn full_height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Place a paragraph applying spacing, keep and widow rules
    /// (ECMA-376 Part 1, §17.3.1.14 keepNext, §17.3.1.15 keepLines, §17.3.1.44 widowControl).
    fn place_para(
        &mut self,
        ctx: &mut Ctx<'_>,
        pb: &ParaBlock,
        keep_with: f32,
        src: Option<&Paragraph>,
    ) -> Result<(), LayoutError> {
        if pb.page_break_before && self.page_has_content {
            self.new_page();
        }
        let mut before = pb.space_before;
        if let Some(p) = &self.prev {
            let mut after = p.after;
            if p.style == pb.style {
                // §17.3.1.9 contextualSpacing: no spacing between paragraphs of the same style.
                if pb.contextual {
                    before = 0.0;
                }
                if p.contextual {
                    after = 0.0;
                }
            }
            if self.col_has_content {
                before = collapse_spacing(&mut self.y, p.after, after, before);
            }
        }
        // Space before is dropped at the top of a page or column, except at the start of a
        // section's first page (the document start, a new-page section break): measured
        // black-box in the reference word processor (docs/ui/REPORT.md, Fidelity round 1) —
        // natural, hard and §17.3.1.23 pageBreakBefore page breaks all drop it.
        let section_top = !self.page_has_content
            && self.col == 0
            && self.section_start.get(self.section).copied().flatten()
                == self.pages.len().checked_sub(1);
        if self.col_has_content || section_top {
            self.y += before;
        }
        let total = pb.height();
        let full = self.full_height();
        let keep_h = if pb.keep_next {
            total + keep_with
        } else {
            total
        };
        if (pb.keep_lines || pb.keep_next)
            && self.col_has_content
            && keep_h <= full
            && !self.fits(self.y, keep_h, 0.0, self.foot_slack)
        {
            self.next_column();
        }

        if let Some(d) = self.drop.take() {
            self.place_drop(&d, pb);
        }
        // Anchored pictures are positioned relative to the paragraph's first line.
        let mut float_items = Vec::new();
        let mut float_lines = Vec::new();
        for img in &pb.floats {
            let (items, lines, excl) = float_item(img, &self.setup, self.col_x(), self.y);
            // A tracked floating picture (inserted / deleted, or both sides of a move
            // §17.13.5.22 moveFrom / §17.13.5.25 moveTo) is framed in markup.
            if ctx.view == crate::RevisionView::Markup {
                if let (Some(rev), Some(PageItem::Image { x, y, w, h, .. })) = (
                    src.and_then(|p| float_revision(p, &img.img)),
                    items.iter().find(|i| matches!(i, PageItem::Image { .. })),
                ) {
                    float_frame(*x, *y, *w, *h, rev, &mut float_items);
                }
            }
            float_items.extend(items.into_iter().map(|mut i| {
                if let Some(s) = i.source_mut() {
                    s.rebase(self.id_base);
                }
                i
            }));
            float_lines.extend(lines.into_iter().map(|mut l| {
                l.source.rebase(self.id_base);
                l
            }));
            match excl {
                Some(Excl::Band(a, b)) => self.excl.push((a, b)),
                Some(Excl::Side(z)) => {
                    // Too narrow on the allowed side(s) for text: behave as top and bottom.
                    let room_l = z.x0 - self.col_x();
                    let room_r = self.col_x() + self.col_w() - z.x1;
                    let room = match z.side {
                        WrapText::Left => room_l,
                        WrapText::Right => room_r,
                        WrapText::BothSides | WrapText::Largest => room_l.max(room_r),
                    };
                    if room < MIN_SEGMENT {
                        self.excl.push((z.y0, z.y1));
                    } else {
                        self.sides.push(z);
                    }
                }
                None => {}
            }
        }
        if let Some(page) = self.pages.last_mut() {
            page.items.extend(float_items);
            page.lines.extend(float_lines);
        }
        // Lines beside square/tight-wrapped pictures are laid out again around them.
        let rewrapped;
        let pb = match src {
            Some(p)
                if self.sides.iter().any(|z| z.y1 > self.y)
                    || (self.col_w() - pb.width).abs() > 0.01 =>
            {
                let (top, cx) = (self.y, self.col_x());
                let zones: Vec<WrapZone> = self
                    .sides
                    .iter()
                    .filter(|z| z.y1 > top)
                    .map(|z| WrapZone {
                        top: z.y0 - top,
                        bottom: z.y1 - top,
                        left: z.x0 - cx,
                        right: z.x1 - cx,
                        side: z.side,
                        outline: z.outline.as_ref().map(|o| o.offset(cx, top)),
                    })
                    .collect();
                rewrapped = rewrap_para(ctx, p, pb, self.col_w(), &zones)?;
                &rewrapped
            }
            _ => pb,
        };

        let first = pb;
        // Laid out again when it continues in a column of another width.
        let mut owned: Option<ParaBlock> = None;
        // Placed parts of the paragraph: (top, bottom, width) from its first line's top.
        let mut parts: Vec<(f32, f32, f32)> = Vec::new();
        let mut part_w = self.col_w();
        let mut n = pb.lines.len();
        let mut i = 0;
        let mut segments: Vec<(usize, f32, f32, f32)> = Vec::new(); // (page, col x, y0, y1)
        while i < n {
            if (self.col_w() - part_w).abs() > 0.01 && self.sides.is_empty() {
                if let Some(p) = src {
                    let cur = owned.as_ref().unwrap_or(first);
                    let from: f32 = cur.lines[..i].iter().map(|b| b.h).sum();
                    if i > 0 {
                        let start = parts.last().map_or(0.0, |q| q.1);
                        parts.push((start, from, part_w));
                    }
                    part_w = self.col_w();
                    if let Some(again) =
                        rewrap_across_columns(ctx, p, cur, &parts, from, part_w, i)?
                    {
                        n = again.lines.len();
                        owned = Some(again);
                    }
                }
            }
            let pb = owned.as_ref().unwrap_or(first);
            let fit = self.fit_count(ctx, &pb.lines[i..])?;
            let remaining = n - i;
            let mut take = fit.min(remaining);
            let breaks_inside = pb.lines[i..i + take].iter().any(|l| l.brk != Brk::None);
            if take < remaining && !breaks_inside {
                if pb.widow_control && remaining >= 2 {
                    if remaining - take == 1 && take >= 1 {
                        take -= 1; // widow: never leave the last line alone
                    }
                    if take == 1 && i == 0 {
                        take = 0; // orphan: never leave the first line alone
                    }
                }
                if take == 0 && self.col_has_content {
                    self.next_column();
                    continue;
                }
                take = take.max(1);
            }
            let page_idx = self.pages.len() - 1;
            let col_x = self.col_x();
            let mut y0 = None;
            for line in &pb.lines[i..i + take] {
                let y = self.skip_excl(self.y, line.text_h);
                y0.get_or_insert(y);
                self.put(ctx, line, y)?;
                if pb.numbered {
                    self.number_line(ctx, line, y)?;
                }
                i += 1;
                if line.brk != Brk::None {
                    break;
                }
            }
            if let Some(y0) = y0 {
                segments.push((page_idx, col_x, y0, self.y));
            }
            let last = &pb.lines[i - 1];
            match last.brk {
                Brk::Page => self.new_page(),
                Brk::Column => {
                    self.col_break = true;
                    self.next_column();
                }
                Brk::None if i < n => self.next_column(),
                Brk::None => {}
            }
        }
        let pb = first;
        if let Some(d) = &pb.deco {
            for (pi, cx, y0, y1) in segments {
                let mut items = Vec::new();
                deco_items(d, y0, y1, &mut items);
                if let Some(page) = self.pages.get_mut(pi) {
                    for mut it in items {
                        it.translate(cx, 0.0);
                        page.items.push(it);
                    }
                }
            }
        }
        self.y += pb.space_after;
        self.prev = Some(Prev {
            style: pb.style.clone(),
            contextual: pb.contextual,
            after: pb.space_after,
        });
        Ok(())
    }

    fn place_row(
        &mut self,
        ctx: &mut Ctx<'_>,
        mut band: Band,
        repeat: bool,
    ) -> Result<(), LayoutError> {
        if repeat {
            band.set_other();
            band.notes.clear();
        }
        let y = self.y;
        self.put(ctx, &band, y)
    }

    /// Place a table row by row; rows split across pages unless `cantSplit`, and
    /// `tblHeader` rows repeat at the top of each page (§17.4.49 tblHeader).
    fn place_table(&mut self, ctx: &mut Ctx<'_>, tb: &TableBox) -> Result<(), LayoutError> {
        if let Some(f) = &tb.float {
            return self.place_floating_table(ctx, tb, f);
        }
        let header_count = tb.rows.iter().take_while(|r| r.header).count();
        let mut idx = 0;
        while idx < tb.rows.len() {
            let mut row = tb.rows[idx].clone();
            let mut continuation = false;
            loop {
                let notes = row_notes(&row);
                let (add, slack) = self.notes_need(ctx, &notes)?;
                self.y = self.skip_excl(self.y, row.h);
                if self.fits(self.y, row.h, add, slack) {
                    self.place_row(ctx, row_band(&row, row.h, tb.x, continuation, false), false)?;
                    break;
                }
                let avail = self.bottom - self.foot_h - add + slack - self.y;
                if !row.cant_split && avail > 6.0 {
                    if let Some((a, b)) = split_row(&row, avail) {
                        self.place_row(ctx, row_band(&a, a.h, tb.x, continuation, true), false)?;
                        self.next_column();
                        self.repeat_headers(ctx, tb, header_count, idx)?;
                        row = b;
                        continuation = true;
                        continue;
                    }
                }
                if !self.col_has_content {
                    self.place_row(ctx, row_band(&row, row.h, tb.x, continuation, false), false)?;
                    break;
                }
                self.next_column();
                self.repeat_headers(ctx, tb, header_count, idx)?;
                continuation = continuation || idx > 0;
            }
            idx += 1;
        }
        self.prev = None;
        Ok(())
    }

    /// Place a floating table (ECMA-376 Part 1, §17.4.58 tblpPr) at its anchor
    /// position, whole on the current page, and let the following text wrap around
    /// it on both sides, `leftFromText` / `rightFromText` / `topFromText` /
    /// `bottomFromText` away. The flow position does not move.
    fn place_floating_table(
        &mut self,
        ctx: &mut Ctx<'_>,
        tb: &TableBox,
        f: &bo_model::TableFloat,
    ) -> Result<(), LayoutError> {
        let w = tb
            .rows
            .iter()
            .filter_map(|r| r.cells.last().map(|c| c.x + c.w))
            .fold(0.0f32, f32::max);
        let h: f32 = tb.rows.iter().map(|r| r.h).sum();
        let s = &self.setup;
        let (page_w, page_h) = (s.width.to_pt(), s.height.to_pt());
        let (ml, mr) = (s.margin_left.to_pt(), s.margin_right.to_pt());
        let (mt, mb) = (s.margin_top.to_pt().abs(), s.margin_bottom.to_pt().abs());
        // §17.18.35 ST_HAnchor / §17.18.100 ST_VAnchor: (start, extent) of the reference.
        let href = match f.horz_anchor.as_deref() {
            Some("page") => (0.0, page_w),
            Some("margin") => (ml, page_w - ml - mr),
            _ => (self.col_x(), self.col_w()),
        };
        let vref = match f.vert_anchor.as_deref() {
            Some("page") => (0.0, page_h),
            Some("margin") => (mt, page_h - mt - mb),
            _ => (self.y, 0.0),
        };
        // §17.18.106 ST_XAlign / §17.18.109 ST_YAlign replace the offsets.
        let x = match f.x_align.as_deref() {
            Some("center") => href.0 + (href.1 - w) / 2.0,
            Some("right" | "outside") => href.0 + href.1 - w,
            Some("left" | "inside") => href.0,
            _ => href.0 + f.x.to_pt(),
        };
        let y = match f.y_align.as_deref() {
            Some("top" | "inside") => vref.0,
            Some("center") => vref.0 + (vref.1 - h) / 2.0,
            Some("bottom" | "outside") => vref.0 + vref.1 - h,
            _ => vref.0 + f.y.to_pt(),
        };
        let mut top = y;
        for row in &tb.rows {
            let band = row_band(row, row.h, 0.0, false, false);
            self.register_notes(ctx, &band.notes)?;
            let b = band.translated_ids(x, top, self.id_base);
            if let Some(page) = self.pages.last_mut() {
                page.lines.extend(b.lines);
                page.items.extend(b.items);
            }
            top += row.h;
        }
        self.page_has_content = true;
        let z = SideZone {
            y0: y - f.top_from_text.to_pt(),
            y1: y + h + f.bottom_from_text.to_pt(),
            x0: x - f.left_from_text.to_pt(),
            x1: x + w + f.right_from_text.to_pt(),
            side: WrapText::BothSides,
            outline: None,
        };
        // No room for text beside the table: text continues below it.
        let room = (z.x0 - self.col_x()).max(self.col_x() + self.col_w() - z.x1);
        if room < MIN_SEGMENT {
            self.excl.push((z.y0, z.y1));
        } else {
            self.sides.push(z);
        }
        self.prev = None;
        Ok(())
    }

    fn repeat_headers(
        &mut self,
        ctx: &mut Ctx<'_>,
        tb: &TableBox,
        header_count: usize,
        idx: usize,
    ) -> Result<(), LayoutError> {
        if idx < header_count || header_count == 0 {
            return Ok(());
        }
        for r in &tb.rows[..header_count] {
            self.place_row(ctx, row_band(r, r.h, tb.x, false, false), true)?;
        }
        Ok(())
    }

    /// Build block `k` of section `si`; returns it with the body id of its first paragraph.
    fn build(
        &mut self,
        ctx: &mut Ctx<'_>,
        si: usize,
        k: usize,
        b: &Block,
    ) -> Result<(Built, u32), LayoutError> {
        ctx.fields.page = self.number;
        let base = ctx.next_id;
        ctx.id_origin = base;
        let width = self.col_w();
        let built = match b {
            Block::Paragraph(p) => {
                let cached = self
                    .built
                    .get(si)
                    .and_then(|v| v.get(k))
                    .and_then(Option::as_ref)
                    .filter(|c| c.width == width)
                    .map(|c| c.block.clone());
                if let Some(pb) = cached {
                    ctx.next_id += 1;
                    Built::Para(pb)
                } else {
                    let pb = Arc::new(build_para(ctx, p, width, true, None)?);
                    if pb.pure {
                        if let Some(slot) = self.built.get_mut(si).and_then(|v| v.get_mut(k)) {
                            *slot = Some(CachedPara {
                                width,
                                block: pb.clone(),
                            });
                        }
                    }
                    Built::Para(pb)
                }
            }
            Block::Table(t) => Built::Table(layout_table(ctx, t, width, true)?),
            Block::Opaque(_) => Built::Skip,
        };
        Ok((built, base))
    }

    /// Start section `si`.
    fn start_section(&mut self, si: usize, sec: &Section, first: bool) {
        let phys = self.vertical.map_or(self.setup, |v| v.0);
        let same_size = phys.width == sec.page.width && phys.height == sec.page.height;
        let rot = sec
            .text_direction
            .map_or(0, bo_model::TextDirection::rotation);
        let same_dir = self.vertical.map_or(0, |v| v.1.degrees as i32) == rot;
        let continuous = !first
            && matches!(
                sec.break_type,
                SectionBreak::Continuous | SectionBreak::NextColumn
            )
            && same_size
            && same_dir;
        if !first && !continuous {
            self.finish_page();
        } else if continuous {
            self.balance_columns();
            // The new columns start below the lowest text of the previous ones.
            self.close_columns();
            self.y = self.cols_top;
        }
        let hf = self.hf_heights.get(si).map_or((0.0, 0.0), |h| h[0]);
        let (top, bottom) = body_area(&sec.page, hf);
        if rot == 0 {
            self.vertical = None;
            self.setup = sec.page;
            self.top = top;
            self.bottom = bottom;
        } else {
            let (v, r) = vertical_setup(&sec.page, top, bottom, rot);
            self.vertical = Some((sec.page, r));
            self.setup = v;
            let (vt, vb) = body_area(&v, (0.0, 0.0));
            self.top = vt;
            self.bottom = vb;
        }
        self.cols = columns(&self.setup, sec);
        self.separator = sec.columns.separator;
        // §17.6.8 lnNumType: numbering restarts with the section unless continuous.
        let prev_numbered = self.line_numbers.is_some();
        self.line_numbers = sec.line_numbers;
        if let Some(ln) = sec.line_numbers {
            if first || !prev_numbered || ln.restart == LineNumberRestart::NewSection {
                self.line_no = ln.start;
            }
        }
        if continuous {
            self.section = si;
            self.col = 0;
            self.cols_top = self.y;
            self.cols_low = self.y;
            if sec.break_type == SectionBreak::NextColumn && self.col_has_content {
                self.next_column();
            }
            return;
        }
        self.section = si;
        self.number = match sec.page_number_start {
            Some(n) => n,
            None => self.number + 1,
        };
        // §17.18.77 oddPage / evenPage: insert a blank page to reach the right parity.
        let want_odd = match sec.break_type {
            SectionBreak::OddPage => Some(true),
            SectionBreak::EvenPage => Some(false),
            _ => None,
        };
        if let Some(odd) = want_odd {
            if !first && (self.number % 2 == 1) != odd {
                self.push_page();
                self.number += 1;
            }
        }
        self.title_page = sec.title_page;
        self.push_page();
        self.section_start[si] = Some(self.pages.len() - 1);
        self.page_area();
    }

    /// Body area of the fresh current page from the header and footer it shows (default,
    /// first or even variant, §17.10.5 headerReference): a taller first-page header
    /// pushes only the first page's body down, as in the reference word processor.
    fn page_area(&mut self) {
        if self.vertical.is_some() || self.page_has_content {
            return;
        }
        let Some(hf) = self.hf_heights.get(self.section) else {
            return;
        };
        let pi = self.pages.len().checked_sub(1);
        let v = if self.title_page && self.section_start.get(self.section).copied().flatten() == pi
        {
            1
        } else if self.even_odd && self.number.is_multiple_of(2) {
            2
        } else {
            0
        };
        let (top, bottom) = body_area(&self.setup, hf[v]);
        self.top = top;
        self.bottom = bottom;
        self.y = top;
        self.cols_top = top;
        self.cols_low = top;
    }

    /// Paginate all sections, or continue from block `from = (section, block)` after
    /// [`Flow::restore`]. Stops early when the flow re-synchronises with the previous
    /// layout ([`Flow::resynced`]).
    pub fn run(
        &mut self,
        ctx: &mut Ctx<'_>,
        from: Option<(usize, usize)>,
    ) -> Result<(), LayoutError> {
        let doc = ctx.doc;
        if let (Some(blocks), Some(sec)) = (&doc.continuation_notice, doc.sections.first()) {
            let w = (sec.page.width.to_pt()
                - sec.page.margin_left.to_pt()
                - sec.page.margin_right.to_pt())
            .max(36.0);
            self.notice = Some(stack_blocks(ctx, blocks, w, false, None)?);
        }
        let (s0, k0) = from.unwrap_or((0, 0));
        for (si, sec) in doc.sections.iter().enumerate().skip(s0) {
            let mut k = 0;
            let mut section_start = None;
            if from.is_some() && si == s0 {
                k = k0;
            } else {
                self.start_section(si, sec, si == 0);
                section_start = Some(self.checkpoint(ctx));
            }
            self.beneath = doc.footnote_props(si).pos == Some(NotePos::BeneathText);
            let mut next: Option<(Built, u32)> = None;
            while k < sec.blocks.len() {
                if self.boundary(ctx, si, k, next.is_some() || self.drop.is_some()) {
                    return Ok(());
                }
                let (cur, base) = match next.take() {
                    Some(b) => b,
                    None => self.build(ctx, si, k, &sec.blocks[k])?,
                };
                // One-block lookahead for keepNext.
                let mut keep_with = 0.0;
                if let Built::Para(pb) = &cur {
                    if pb.keep_next && k + 1 < sec.blocks.len() {
                        let nb = self.build(ctx, si, k + 1, &sec.blocks[k + 1])?;
                        keep_with = match &nb.0 {
                            Built::Para(n) => n.space_before + n.lines.first().map_or(0.0, |l| l.h),
                            Built::Table(t) => t.rows.first().map_or(0.0, |r| r.h),
                            Built::Skip => 0.0,
                        };
                        next = Some(nb);
                    }
                }
                self.id_base = base;
                // §17.3.1.11 framePr dropCap: the paragraph is set beside the next one.
                let drop_next = matches!(sec.blocks.get(k + 1), Some(Block::Paragraph(_)));
                match &cur {
                    Built::Para(pb) if pb.frame.is_some() && drop_next && self.drop.is_none() => {
                        self.drop = Some(pending_drop(pb, base));
                    }
                    Built::Para(pb) => {
                        let src = match &sec.blocks[k] {
                            Block::Paragraph(p) => Some(p),
                            _ => None,
                        };
                        let continuous_next = doc
                            .sections
                            .get(si + 1)
                            .is_some_and(|n| matches!(n.break_type, SectionBreak::Continuous));
                        if k + 1 == sec.blocks.len() && continuous_next && is_mark_only(pb, src) {
                            self.place_section_mark(ctx, pb, src)?;
                        } else {
                            self.place_para(ctx, pb, keep_with, src)?;
                        }
                    }
                    Built::Table(tb) => self.place_table(ctx, tb)?,
                    Built::Skip => {}
                }
                k += 1;
            }
            if let Some(start) = &section_start {
                self.balance_unequal_columns(ctx, si, start)?;
            }
            // §17.11.22 pos sectEnd: the section's endnotes at its end.
            if doc.endnote_props(si).pos == Some(NotePos::SectEnd) {
                let ids = crate::endnotes_in(&sec.blocks);
                self.place_endnotes(ctx, &ids)?;
            }
        }
        if self.pages.is_empty() {
            self.number = 1;
            self.push_page();
        }
        // Endnotes of the other sections at the end of the document (docEnd, default).
        let rest: Vec<i32> = {
            let placed: std::collections::HashSet<i32> = doc
                .sections
                .iter()
                .enumerate()
                .filter(|(si, _)| doc.endnote_props(*si).pos == Some(NotePos::SectEnd))
                .flat_map(|(_, s)| crate::endnotes_in(&s.blocks))
                .collect();
            ctx.endnote_order
                .iter()
                .copied()
                .filter(|id| !placed.contains(id))
                .collect()
        };
        self.place_endnotes(ctx, &rest)?;
        self.finish_page();
        // Footnotes still continuing after the last page get pages of their own.
        let mut guard = 0;
        while !self.carry.is_empty() && guard < 10_000 {
            guard += 1;
            self.number += 1;
            self.push_page();
            self.finish_page();
        }
        Ok(())
    }

    /// Endnotes at the end of the document (§17.11.4 endnotePr default position docEnd).
    fn place_endnotes(&mut self, ctx: &mut Ctx<'_>, ids: &[i32]) -> Result<(), LayoutError> {
        let keys: Vec<NoteKey> = ids
            .iter()
            .filter(|id| ctx.doc.endnotes.contains_key(id))
            .map(|id| (true, *id))
            .collect();
        if keys.is_empty() {
            return Ok(());
        }
        let mut sep = Band::empty(NOTE_SEPARATOR);
        sep.items.push(PageItem::Rule {
            x1: 0.0,
            y1: NOTE_SEPARATOR / 2.0,
            x2: 144.0,
            y2: NOTE_SEPARATOR / 2.0,
            width: 0.5,
            color: Color::BLACK,
            style: "single".into(),
        });
        self.place_band(ctx, &sep)?;
        for key in keys {
            let blocks = ctx.doc.endnotes.get(&key.1).cloned().unwrap_or_default();
            let mark = ctx.notes.get(&key).cloned();
            let st = stack_note(ctx, key, &blocks, self.col_w(), mark.as_deref())?;
            let mut last_bottom = 0.0f32;
            for (top, band) in &st.bands {
                // Keep the vertical gaps between bands (paragraph spacing).
                self.y += (top - last_bottom).max(0.0);
                let mut b = band.clone();
                b.set_other();
                self.place_band(ctx, &b)?;
                last_bottom = top + band.h;
            }
            self.y += (st.h - last_bottom).max(0.0);
        }
        Ok(())
    }
}

/// A drop cap paragraph's lines stacked into one band, with its text width.
fn pending_drop(pb: &ParaBlock, base: u32) -> PendingDrop {
    let mut band = Band::empty(pb.height());
    let mut top = 0.0;
    let mut width = 0.0f32;
    for b in &pb.lines {
        for l in &b.lines {
            for f in &l.fragments {
                width = width.max(f.x + f.width);
            }
        }
        band.absorb(b.translated(0.0, top));
        top += b.h;
    }
    PendingDrop {
        band,
        base,
        width,
        frame: pb.frame.unwrap_or(FrameProps {
            drop_cap: DropCap::Drop,
            lines: 3,
            h_space: Twips(0),
        }),
    }
}

/// The shaped number `n` of line numbering, in the "Line Number" character style
/// when the document defines it (ECMA-376 Part 1, §17.6.8).
fn number_band(ctx: &mut Ctx<'_>, n: u32) -> Result<Band, LayoutError> {
    let style = ctx
        .doc
        .style("LineNumber")
        .is_some()
        .then(|| "LineNumber".to_owned());
    let p = Paragraph::new(
        bo_model::ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            indent_left: Some(Twips(0)),
            indent_right: Some(Twips(0)),
            indent_first_line: Some(Twips(0)),
            ..bo_model::ParaProps::default()
        },
        vec![bo_model::Run::text(
            n.to_string(),
            bo_model::RunProps {
                style,
                ..bo_model::RunProps::default()
            },
        )],
    );
    let saved = (ctx.note.take(), ctx.textbox.take());
    let pb = build_para(ctx, &p, 200.0, false, None);
    (ctx.note, ctx.textbox) = saved;
    let mut band = pb?
        .lines
        .into_iter()
        .next()
        .unwrap_or_else(|| Band::empty(0.0));
    band.set_other();
    Ok(band)
}

/// Header/footer lookup with inheritance from previous sections (§17.10.5 headerReference).
pub(crate) fn hf_blocks(
    doc: &bo_model::Document,
    si: usize,
    header: bool,
    variant: u8,
) -> Option<&Vec<Block>> {
    for s in doc.sections[..=si.min(doc.sections.len().saturating_sub(1))]
        .iter()
        .rev()
    {
        let set = if header { &s.headers } else { &s.footers };
        let v = match variant {
            1 => &set.first,
            2 => &set.even,
            _ => &set.default,
        };
        if let Some(b) = v {
            return Some(b);
        }
    }
    None
}

/// An empty paragraph (no text, objects or floats) that only carries its section's break.
fn is_mark_only(pb: &ParaBlock, src: Option<&Paragraph>) -> bool {
    src.is_some_and(|p| p.plain_text().is_empty())
        && pb.floats.is_empty()
        && pb.deco.is_none()
        && pb.lines.len() == 1
        && pb
            .lines
            .iter()
            .all(|l| l.items.is_empty() && l.brk == Brk::None)
}

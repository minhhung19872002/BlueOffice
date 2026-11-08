//! Pagination of the main story: sections, columns, keep rules, widow/orphan
//! control, tables split across pages, anchored pictures and footnotes.

use crate::blocks::{
    build_para, deco_items, layout_table, rewrap_para, row_band, row_notes, split_row,
    stack_blocks, stack_note, Band, Brk, Ctx, ParaBlock, Stack, TableBox,
};
use crate::drawing::Float;
use crate::paragraph::{WrapZone, MIN_SEGMENT};
use crate::style::ListCounters;
use crate::types::{Layer, Line, LineSource, NoteKey, Page, PageItem};
use crate::LayoutError;
use bo_model::{
    Block, Color, Emu, NotePos, PageSetup, Paragraph, Section, SectionBreak, Wrap, WrapText,
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
    /// (header height, footer height) per section, used to push the body.
    pub hf_heights: Vec<(f32, f32)>,
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
}

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

/// Column boxes (§17.6.4 cols): equal widths separated by `space`.
pub(crate) fn columns(setup: &PageSetup, sec: &Section) -> Vec<(f32, f32)> {
    let left = setup.margin_left.to_pt();
    let width =
        (setup.width.to_pt() - setup.margin_left.to_pt() - setup.margin_right.to_pt()).max(36.0);
    let n = usize::from(sec.columns.count.clamp(1, 45));
    let space = sec.columns.space.to_pt().max(0.0);
    let w = ((width - space * (n - 1) as f32) / n as f32).max(18.0);
    (0..n).map(|i| (left + i as f32 * (w + space), w)).collect()
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
                    media: img.media.clone(),
                    crop: crate::blocks::crop_of(img),
                    rotation: crate::blocks::rotation_of(img),
                    layer,
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
    let (items, lines) = draw(x, y, if a.behind { Layer::Back } else { Layer::Front });
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
        Wrap::Tight => {
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
    pub fn new(hf_heights: Vec<(f32, f32)>, sections: usize) -> Flow {
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
            id_base: 0,
            foot_order: Vec::new(),
            built: Vec::new(),
            cps: Vec::new(),
            resync_from: Vec::new(),
            old_foot_order: Vec::new(),
            allow_renumber: false,
            resynced: None,
            beneath: false,
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
        self.pages.push(Page {
            width: self.setup.width.to_pt(),
            height: self.setup.height.to_pt(),
            lines: Vec::new(),
            items: Vec::new(),
            number: self.number,
            section: self.section,
        });
        self.col = 0;
        self.y = self.top;
        self.col_has_content = false;
        self.page_has_content = false;
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
    }

    fn next_column(&mut self) {
        if self.col + 1 < self.cols.len() {
            self.col += 1;
            self.y = self.top;
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
            self.foot_slack = h - self.remaining(*key, 0).1;
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
        self.col_has_content = true;
        self.page_has_content = true;
        Ok(())
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
            Brk::Column => self.next_column(),
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
            for key in &l.notes {
                if !key.0 && !seen.contains(key) {
                    if foot <= 0.0 {
                        foot += NOTE_SEPARATOR;
                    }
                    let h = self.note_stack(ctx, *key)?;
                    foot += h;
                    slack = h - self.remaining(*key, 0).1;
                    seen.push(*key);
                }
            }
            let ly = self.skip_excl(y, l.h);
            if ly + l.h > self.bottom - foot + slack + EPS {
                return Ok(i);
            }
            y = ly + l.h;
            if l.brk != Brk::None {
                return Ok(i + 1);
            }
        }
        Ok(lines.len())
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
            if p.style == pb.style {
                if pb.contextual {
                    before = 0.0;
                }
                if p.contextual {
                    self.y -= p.after;
                }
            }
        }
        if self.col_has_content {
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
            Some(p) if self.sides.iter().any(|z| z.y1 > self.y) => {
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

        let n = pb.lines.len();
        let mut i = 0;
        let mut segments: Vec<(usize, f32, f32, f32)> = Vec::new(); // (page, col x, y0, y1)
        while i < n {
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
                let y = self.skip_excl(self.y, line.h);
                y0.get_or_insert(y);
                self.put(ctx, line, y)?;
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
                Brk::Column => self.next_column(),
                Brk::None if i < n => self.next_column(),
                Brk::None => {}
            }
        }
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
        let same_size = self.setup.width == sec.page.width && self.setup.height == sec.page.height;
        let continuous = !first
            && matches!(
                sec.break_type,
                SectionBreak::Continuous | SectionBreak::NextColumn
            )
            && same_size;
        if !first && !continuous {
            self.finish_page();
        }
        self.setup = sec.page;
        let hf = self.hf_heights.get(si).copied().unwrap_or((0.0, 0.0));
        let (top, bottom) = body_area(&sec.page, hf);
        self.cols = columns(&sec.page, sec);
        self.top = top;
        self.bottom = bottom;
        if continuous {
            self.section = si;
            self.col = 0;
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
        self.push_page();
        self.section_start[si] = Some(self.pages.len() - 1);
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
            if from.is_some() && si == s0 {
                k = k0;
            } else {
                self.start_section(si, sec, si == 0);
            }
            self.beneath = doc.footnote_props(si).pos == Some(NotePos::BeneathText);
            let mut next: Option<(Built, u32)> = None;
            while k < sec.blocks.len() {
                if self.boundary(ctx, si, k, next.is_some()) {
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
                match &cur {
                    Built::Para(pb) => {
                        let src = match &sec.blocks[k] {
                            Block::Paragraph(p) => Some(p),
                            _ => None,
                        };
                        self.place_para(ctx, pb, keep_with, src)?;
                    }
                    Built::Table(tb) => self.place_table(ctx, tb)?,
                    Built::Skip => {}
                }
                k += 1;
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

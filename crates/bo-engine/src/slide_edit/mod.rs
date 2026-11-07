//! BlueSlide editing (prompt §6 Phase 4) over the JSON bridge (ADR-0008).
//!
//! The presentation model stays the single source of truth inside the
//! engine. Hosts send a [`SlideCommand`] and receive a [`SlideEditResult`]:
//! the selection with its geometry and handles, ribbon state, the slides whose
//! display list changed and — for drag previews — the snapped boxes and smart
//! guide lines.
//!
//! Every change is a transaction of invertible [`ops::SlideOp`]s: undo/redo is
//! unlimited and exact. Edited shapes are regenerated from the model, which
//! keeps their unknown children (`Extra`) in place, so saving stays lossless for
//! everything that was not touched.

pub mod build;
pub mod collab;
pub mod geom;
pub mod guides;
pub mod ops;
pub mod rich;
pub mod text;

use crate::{DocInfo, Engine, EngineError};
use bo_model::slide::{Fill, Presentation, Rel, ShapeNode, TextBody, TextRun, Xfrm};
use bo_pptx::inherit::{chain, clr_map_of, theme_of, Level, TextStyleStack};
use geom::{
    find, handles, hit_test, map_box, node_xfrm, place_slide, pointer_angle, resize_box, BoxR,
    Handle, HandleKind, Placed, Rect, EMU_PT,
};
use guides::{Edges, Guide};
use ops::{OpError, SlideOp, Tx};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use text::{Align, FormatState, TextFormat};

/// Hit-test tolerance in points (thin lines stay clickable).
const HIT_TOLERANCE: f64 = 3.0;
/// Offset of duplicated shapes, in points.
const DUPLICATE_OFFSET: f64 = 12.0;

/// The selection: current slide and selected shape ids (`cNvPr@id`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideSel {
    pub slide: usize,
    pub ids: Vec<u32>,
}

/// Interactive transform of the selection.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Transform {
    /// Move by (dx, dy) points.
    Move { dx: f64, dy: f64 },
    /// Drag a resize handle by (dx, dy) points.
    Resize {
        handle: HandleKind,
        dx: f64,
        dy: f64,
        #[serde(default)]
        keep_aspect: bool,
    },
    /// Drag the rotation handle to slide point (x, y); `step` snaps to 15°.
    Rotate {
        x: f64,
        y: f64,
        #[serde(default)]
        step: bool,
    },
}

/// Z-order changes (§19.3.1.45 spTree: later siblings are drawn on top).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZOrder {
    Front,
    Back,
    Forward,
    Backward,
}

/// Alignment of shapes (to the slide for one shape, to the selection otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlignTo {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

/// Distribution axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

fn yes() -> bool {
    true
}
fn default_tolerance() -> f64 {
    4.0
}

/// A slide editing command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SlideCommand {
    /// Current state, nothing changes.
    State,
    /// Make `slide` current (clears the selection).
    GoTo {
        slide: usize,
    },
    Select {
        ids: Vec<u32>,
    },
    SelectAll,
    /// Select the topmost shape at a slide point; `extend` (Shift) toggles it;
    /// `deep` selects inside groups.
    Click {
        x: f64,
        y: f64,
        #[serde(default)]
        extend: bool,
        #[serde(default)]
        deep: bool,
    },
    /// Shapes at a point (topmost first), selection unchanged.
    HitTest {
        x: f64,
        y: f64,
        #[serde(default)]
        deep: bool,
    },
    /// Move / resize / rotate the selection; `preview` only computes the
    /// result (boxes and smart guides) without changing the deck.
    Transform {
        transform: Transform,
        #[serde(default = "yes")]
        snap: bool,
        #[serde(default = "default_tolerance")]
        tolerance: f64,
        #[serde(default)]
        preview: bool,
    },
    Delete,
    Duplicate,
    ZOrder {
        to: ZOrder,
    },
    Group,
    Ungroup,
    Align {
        to: AlignTo,
    },
    Distribute {
        axis: Axis,
    },
    /// Preset shape (§20.1.10.56 ST_ShapeType, e.g. `rect`, `ellipse`, `rightArrow`).
    InsertShape {
        preset: String,
        #[serde(default)]
        rect: Option<Rect>,
    },
    InsertTextBox {
        #[serde(default)]
        text: String,
        #[serde(default)]
        rect: Option<Rect>,
    },
    InsertTable {
        rows: usize,
        cols: usize,
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Replace the text of a shape (`\n` paragraphs, `\u{b}` line breaks).
    /// `merge` joins the change with the previous typing in the same shape
    /// (one undo step); `sel` is the text selection afterwards (UTF-16).
    SetText {
        #[serde(default)]
        id: Option<u32>,
        text: String,
        #[serde(default)]
        merge: bool,
        #[serde(default)]
        sel: Option<[usize; 2]>,
    },
    /// Format the text selection (`range`, UTF-16, or the current text
    /// selection) or — outside text editing — the whole text of the selection.
    FormatText {
        format: TextFormat,
        #[serde(default)]
        range: Option<[usize; 2]>,
    },
    /// Start / move the text selection inside a shape (UTF-16 offsets).
    TextSelect {
        #[serde(default)]
        id: Option<u32>,
        start: usize,
        end: usize,
    },
    /// Place the caret at a slide point of the edited shape (`extend` = Shift).
    TextHit {
        x: f64,
        y: f64,
        #[serde(default)]
        extend: bool,
    },
    /// Leave text editing.
    TextExit,
    /// `RRGGBB` or `None` for no fill.
    SetFill {
        color: Option<String>,
    },
    /// `RRGGBB` or `None` for no line; width in points.
    SetLine {
        color: Option<String>,
        #[serde(default)]
        width: Option<f64>,
    },
    /// New slide after `after` (default: current) from `layout` (default: like the current one).
    AddSlide {
        #[serde(default)]
        after: Option<usize>,
        #[serde(default)]
        layout: Option<usize>,
    },
    DeleteSlide {
        #[serde(default)]
        index: Option<usize>,
    },
    DuplicateSlide {
        #[serde(default)]
        index: Option<usize>,
    },
    MoveSlide {
        from: usize,
        to: usize,
    },
    SetNotes {
        #[serde(default)]
        slide: Option<usize>,
        text: String,
    },
    Undo,
    Redo,
}

impl SlideCommand {
    /// Commands that keep the text selection of the shape being edited.
    fn keeps_text_selection(&self) -> bool {
        matches!(
            self,
            SlideCommand::State
                | SlideCommand::SetText { .. }
                | SlideCommand::FormatText { .. }
                | SlideCommand::TextSelect { .. }
                | SlideCommand::TextHit { .. }
                | SlideCommand::Undo
                | SlideCommand::Redo
        )
    }
}

/// The only selected id.
fn single(ids: &[u32]) -> Option<u32> {
    match ids {
        [one] => Some(*one),
        _ => None,
    }
}

/// Text of a shape in the editing format.
fn shape_text(pres: &Presentation, slide: usize, id: u32) -> Option<String> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    match ops::node_at(pres, slide, &p.path)? {
        ShapeNode::Shape(s) => Some(s.text.as_ref().map(text::body_text).unwrap_or_default()),
        _ => None,
    }
}

/// A selected shape as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeInfo {
    pub id: u32,
    pub name: String,
    /// `shape` | `picture` | `group` | `connector` | `table` | `graphic`.
    pub kind: &'static str,
    pub rect: BoxR,
    /// Text in the editing format (shapes only).
    pub text: Option<String>,
    /// Placeholder type (`title`, `body`, … ; `obj` when untyped).
    pub placeholder: Option<String>,
    /// Resize keeps the aspect ratio by default (pictures).
    pub lock_aspect: bool,
    pub editable_text: bool,
}

/// A slide layout new slides can be based on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutInfo {
    pub index: usize,
    pub name: String,
    pub kind: Option<String>,
}

/// Result of a transform preview.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub boxes: Vec<BoxR>,
    pub bounds: Option<BoxR>,
    pub guides: Vec<Guide>,
    /// Applied move delta after snapping.
    pub dx: f64,
    pub dy: f64,
    /// Rotation angle (rotate only), degrees.
    pub angle: Option<f64>,
}

/// Result of a slide command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideEditResult {
    pub info: DocInfo,
    /// Current slide.
    pub slide: usize,
    pub selection: Vec<ShapeInfo>,
    /// Box around the selection (the shape's own box for one shape).
    pub bounds: Option<BoxR>,
    pub handles: Vec<Handle>,
    /// Empty placeholders of the current slide (drawn as dashed outlines).
    pub placeholders: Vec<BoxR>,
    /// Slides whose display list changed.
    pub dirty: Vec<usize>,
    /// The slide list changed (count or order).
    pub slides_changed: bool,
    pub modified: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub format: FormatState,
    /// Notes of the current slide in the editing format.
    pub notes: String,
    pub layouts: Vec<LayoutInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hits: Option<Vec<u32>>,
    /// Message of a command that could not be applied (the deck is unchanged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Change to share and the selected object while collaborating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collab: Option<collab::SlideCollabOut>,
    /// Text being edited inside a shape: selection, caret and highlight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<rich::TextEditOut>,
}

/// Why a command was refused (codes the UI can translate).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error(transparent)]
    Op(#[from] OpError),
    #[error("{0}")]
    Refused(&'static str),
}

type R<T> = std::result::Result<T, EditError>;

fn refuse<T>(code: &'static str) -> R<T> {
    Err(EditError::Refused(code))
}

struct Txn {
    id: u64,
    ops: Vec<SlideOp>,
    before: SlideSel,
    after: SlideSel,
    /// Typing in (slide, shape): later typing there joins this transaction.
    typing: Option<(usize, u32)>,
}

/// Selection and history of the open presentation.
#[derive(Default)]
pub struct SlideEditor {
    sel: SlideSel,
    undo: Vec<Txn>,
    redo: Vec<Txn>,
    next_txn: u64,
    saved: u64,
    /// Text selection while a shape's text is edited.
    text: Option<rich::TextSel>,
}

#[derive(Default)]
struct Outcome {
    touched: BTreeSet<usize>,
    all: bool,
    preview: Option<Preview>,
    hits: Option<Vec<u32>>,
    error: Option<String>,
}

impl Outcome {
    fn note(&mut self, ops: &[SlideOp]) {
        for op in ops {
            match op {
                SlideOp::Slides { .. } => self.all = true,
                SlideOp::Parts { .. } | SlideOp::Pres { .. } => {}
                _ => {
                    if let Some(s) = op.touched_slide() {
                        self.touched.insert(s);
                    }
                }
            }
        }
    }
}

impl SlideEditor {
    fn state_id(&self) -> u64 {
        self.undo.last().map_or(0, |t| t.id)
    }

    /// Mark the current state as saved.
    pub fn mark_saved(&mut self) {
        self.saved = self.state_id();
    }

    /// Keep the selection valid for the deck.
    fn clamp(&mut self, pres: &Presentation) {
        let n = pres.slides.len();
        if self.sel.slide >= n {
            self.sel.slide = n.saturating_sub(1);
            self.sel.ids.clear();
        }
        let placed = place_slide(pres, self.sel.slide);
        self.sel.ids.retain(|id| find(&placed, *id).is_some());
        if let Some(t) = self.text {
            self.text = shape_text(pres, self.sel.slide, t.id).map(|s| {
                let n = s.chars().count();
                rich::TextSel {
                    start: t.start.min(n),
                    end: t.end.min(n),
                    ..t
                }
            });
        }
    }

    /// Run a change as one undoable transaction.
    fn mutate(
        &mut self,
        pres: &mut Presentation,
        out: &mut Outcome,
        f: impl FnOnce(&mut Tx<'_>, &mut SlideSel) -> R<()>,
    ) {
        self.mutate_as(pres, out, None, f);
    }

    /// [`Self::mutate`]; typing (`Some(slide, shape)`) joins the previous
    /// typing transaction in the same shape when nothing happened in between.
    fn mutate_as(
        &mut self,
        pres: &mut Presentation,
        out: &mut Outcome,
        typing: Option<(usize, u32)>,
        f: impl FnOnce(&mut Tx<'_>, &mut SlideSel) -> R<()>,
    ) {
        let before = self.sel.clone();
        let mut sel = self.sel.clone();
        let mut tx = Tx::new(pres);
        match f(&mut tx, &mut sel) {
            Ok(()) => {
                let ops = std::mem::take(&mut tx.ops);
                out.note(&ops);
                if !ops.is_empty() {
                    let join = typing.is_some()
                        && self.redo.is_empty()
                        && self.state_id() != self.saved
                        && self.undo.last().is_some_and(|t| t.typing == typing);
                    if let (true, Some(last)) = (join, self.undo.last_mut()) {
                        last.ops.extend(ops);
                        last.after = sel.clone();
                    } else {
                        self.next_txn += 1;
                        self.undo.push(Txn {
                            id: self.next_txn,
                            ops,
                            before,
                            after: sel.clone(),
                            typing,
                        });
                    }
                    self.redo.clear();
                }
                self.sel = sel;
            }
            Err(e) => {
                tx.rollback();
                out.error = Some(e.to_string());
            }
        }
    }

    fn undo(&mut self, pres: &mut Presentation, out: &mut Outcome) {
        let Some(t) = self.undo.pop() else { return };
        let mut done = Vec::new();
        for op in t.ops.iter().rev() {
            let inv = op.inverse();
            if let Err(e) = inv.apply(pres) {
                // Should not happen: restore and report.
                for d in done.iter().rev() {
                    let _ = SlideOp::inverse(d).apply(pres);
                }
                out.error = Some(e.to_string());
                self.undo.push(t);
                return;
            }
            done.push(inv);
        }
        out.note(&t.ops);
        self.sel = t.before.clone();
        self.redo.push(t);
    }

    fn redo(&mut self, pres: &mut Presentation, out: &mut Outcome) {
        let Some(t) = self.redo.pop() else { return };
        let mut done: Vec<&SlideOp> = Vec::new();
        for op in &t.ops {
            if let Err(e) = op.apply(pres) {
                for d in done.iter().rev() {
                    let _ = d.inverse().apply(pres);
                }
                out.error = Some(e.to_string());
                self.redo.push(t);
                return;
            }
            done.push(op);
        }
        out.note(&t.ops);
        self.sel = t.after.clone();
        self.undo.push(t);
    }
}

// ---------------------------------------------------------------------------
// Helpers on the model
// ---------------------------------------------------------------------------

fn slide_size(pres: &Presentation) -> (f64, f64) {
    (
        pres.slide_size.cx as f64 / EMU_PT,
        pres.slide_size.cy as f64 / EMU_PT,
    )
}

fn kind_of(n: &ShapeNode) -> &'static str {
    match n {
        ShapeNode::Shape(_) => "shape",
        ShapeNode::Picture(_) => "picture",
        ShapeNode::Group(_) => "group",
        ShapeNode::Connector(_) => "connector",
        ShapeNode::Frame(f) if matches!(f.content, bo_model::slide::GraphicContent::Table(_)) => {
            "table"
        }
        ShapeNode::Frame(_) | ShapeNode::Other(_) => "graphic",
    }
}

/// Largest `cNvPr@id` in a slide + 1.
fn next_shape_id(pres: &Presentation, slide: usize) -> u32 {
    let Some(s) = pres.slides.get(slide) else {
        return 2;
    };
    let mut max = s.data.tree_nv.id;
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if let Some(nv) = n.nv() {
            max = max.max(nv.id);
        }
    });
    max + 1
}

/// Selected shapes, without descendants of other selected groups.
fn selected<'a>(placed: &'a [Placed], ids: &[u32]) -> Vec<&'a Placed> {
    let mut v: Vec<&Placed> = ids.iter().filter_map(|id| find(placed, *id)).collect();
    let paths: Vec<Vec<usize>> = v.iter().map(|p| p.path.clone()).collect();
    v.retain(|p| {
        !paths
            .iter()
            .any(|q| q.len() < p.path.len() && p.path.starts_with(q))
    });
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v.dedup_by(|a, b| a.id == b.id);
    v
}

/// Placed shape at an index path (any depth).
fn placed_at<'a>(placed: &'a [Placed], path: &[usize]) -> Option<&'a Placed> {
    for p in placed {
        if p.path == path {
            return Some(p);
        }
        if path.starts_with(&p.path) {
            if let Some(c) = placed_at(&p.children, path) {
                return Some(c);
            }
        }
    }
    None
}

fn node_clone(pres: &Presentation, slide: usize, path: &[usize]) -> R<ShapeNode> {
    ops::node_at(pres, slide, path)
        .cloned()
        .ok_or(EditError::Op(OpError::Range))
}

/// Give a node (and a group's children) fresh ids.
fn renumber(n: &mut ShapeNode, next: &mut u32) {
    let set = |nv: &mut bo_model::slide::NonVisual, next: &mut u32| {
        nv.id = *next;
        *next += 1;
    };
    match n {
        ShapeNode::Shape(s) => set(&mut s.nv, next),
        ShapeNode::Picture(p) => set(&mut p.nv, next),
        ShapeNode::Connector(c) => set(&mut c.nv, next),
        ShapeNode::Frame(f) => set(&mut f.nv, next),
        ShapeNode::Group(g) => {
            set(&mut g.nv, next);
            for c in &mut g.children {
                renumber(c, next);
            }
        }
        ShapeNode::Other(_) => {}
    }
}

fn all_ids(n: &ShapeNode, out: &mut Vec<u32>) {
    if let Some(nv) = n.nv() {
        out.push(nv.id);
    }
    if let ShapeNode::Group(g) = n {
        for c in &g.children {
            all_ids(c, out);
        }
    }
}

/// Set the transform of a node from a new visual box.
fn apply_box(pres: &Presentation, slide: usize, node: &mut ShapeNode, p: &Placed, b: &BoxR) {
    let base = node_xfrm(pres, slide, node).unwrap_or_default();
    let mut x = p.frame.to_xfrm(b, &base);
    match node {
        ShapeNode::Shape(s) => s.props.xfrm = Some(x),
        ShapeNode::Picture(pic) => pic.props.xfrm = Some(x),
        ShapeNode::Connector(c) => c.props.xfrm = Some(x),
        ShapeNode::Group(g) => {
            // §20.1.7.5: without chOff/chExt the child space follows off/ext;
            // pin it so the children move and scale with the group.
            if x.ch_off.is_none() {
                x.ch_off = base.off;
            }
            if x.ch_ext.is_none() {
                x.ch_ext = base.ext;
            }
            g.props.xfrm = Some(x);
        }
        ShapeNode::Frame(f) => {
            // Tables are sized by their grid (§21.1.3.2 gridCol, §21.1.3.18 tr@h).
            if let (bo_model::slide::GraphicContent::Table(t), Some((ow, oh)), Some((nw, nh))) =
                (&mut f.content, base.ext, x.ext)
            {
                if ow > 0 && oh > 0 && (ow, oh) != (nw, nh) {
                    let kx = nw as f64 / ow as f64;
                    let ky = nh as f64 / oh as f64;
                    for c in &mut t.grid {
                        c.w = (c.w as f64 * kx).round() as i64;
                    }
                    for r in &mut t.rows {
                        r.h = (r.h as f64 * ky).round() as i64;
                    }
                }
            }
            f.xfrm = x;
        }
        ShapeNode::Other(_) => {}
    }
}

/// Default box for a new object: centred, `w × h` points (or as requested).
fn place_new(pres: &Presentation, rect: Option<Rect>, w: f64, h: f64) -> Rect {
    if let Some(r) = rect.filter(|r| r.w > 0.0 && r.h > 0.0) {
        return r;
    }
    let (sw, sh) = slide_size(pres);
    Rect {
        x: (sw - w) / 2.0,
        y: (sh - h) / 2.0,
        w,
        h,
    }
}

/// Effective text formatting at the start of a shape's text.
fn format_state(pres: &Presentation, slide: usize, s: &bo_model::slide::Shape) -> FormatState {
    format_state_at(pres, slide, s, None)
}

/// Effective text formatting at a text selection `(start, end)` (character
/// offsets) or, without one, at the start of the text.
fn format_state_at(
    pres: &Presentation,
    slide: usize,
    s: &bo_model::slide::Shape,
    at: Option<(usize, usize)>,
) -> FormatState {
    let Some(tb) = &s.text else {
        return FormatState::default();
    };
    let ch = chain(pres, Level::Slide(slide), s);
    let stack = TextStyleStack::new(pres, Level::Slide(slide), &ch);
    let empty = bo_model::slide::Paragraph::default();
    let (p, rpr) = if let Some((a, b)) = at {
        let (pi, rpr) = text::props_at_offset(tb, a.min(b), a != b);
        (tb.paragraphs.get(pi).unwrap_or(&empty), rpr)
    } else {
        let p = tb.paragraphs.first().unwrap_or(&empty);
        let rpr = p
            .runs
            .iter()
            .find_map(|r| match r {
                TextRun::Run { props, .. } => Some(props.clone()),
                _ => None,
            })
            .or_else(|| p.end_props.clone())
            .unwrap_or_default();
        (p, rpr)
    };
    let ppr = stack.paragraph(p);
    let r = TextStyleStack::run(&ppr, &rpr);
    let map = clr_map_of(pres, Level::Slide(slide));
    let ctx = bo_drawingml::ColorContext {
        theme: theme_of(pres, Level::Slide(slide)),
        clr_map: &map,
        ph_clr: None,
    };
    let hex = |c: &bo_model::slide::Color| {
        format!(
            "{:06X}",
            bo_drawingml::resolve_color(c, &ctx).to_hex() & 0x00FF_FFFF
        )
    };
    let color = match &r.fill {
        Some(Fill::Solid(c)) => Some(hex(c)),
        _ => None,
    };
    let font = r.latin.as_ref().map(|f| {
        bo_drawingml::theme::resolve_typeface(theme_of(pres, Level::Slide(slide)), &f.typeface)
            .to_owned()
    });
    FormatState {
        has_text: true,
        bold: r.b == Some(true),
        italic: r.i == Some(true),
        underline: r.u.as_deref().is_some_and(|u| u != "none"),
        strike: r.strike.as_deref().is_some_and(|v| v != "noStrike"),
        size: r.sz.map(|s| f64::from(s) / 100.0),
        color,
        highlight: r.highlight.as_ref().map(hex),
        font: font.filter(|f| !f.is_empty()),
        align: Some(Align::of(ppr.algn.as_deref())),
        bullets: matches!(
            ppr.bullet,
            Some(bo_model::slide::Bullet::Char(_) | bo_model::slide::Bullet::Blip(_))
        ),
        numbering: matches!(ppr.bullet, Some(bo_model::slide::Bullet::AutoNum { .. })),
        level: ppr.lvl.unwrap_or(0),
        line_spacing: match ppr.ln_spc {
            Some(bo_model::slide::Spacing::Percent(p)) => Some(f64::from(p) / 100_000.0),
            _ => None,
        },
    }
}

/// Path of the notes body placeholder (§19.3.1.36 ph type="body").
fn notes_body_index(n: &bo_model::slide::Notes) -> Option<usize> {
    n.data.shapes.iter().position(|s| match s {
        ShapeNode::Shape(sh) => sh
            .nv
            .placeholder
            .as_ref()
            .is_some_and(|p| p.kind.as_deref() == Some("body")),
        _ => false,
    })
}

fn notes_text(pres: &Presentation, slide: usize) -> String {
    let Some(n) = pres.slides.get(slide).and_then(|s| s.notes.as_ref()) else {
        return String::new();
    };
    match notes_body_index(n).and_then(|i| n.data.shapes.get(i)) {
        Some(ShapeNode::Shape(s)) => s.text.as_ref().map(text::body_text).unwrap_or_default(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Transform planning (shared by preview and commit)
// ---------------------------------------------------------------------------

fn edges(sign: f64) -> Edges {
    if sign < 0.0 {
        Edges::Start
    } else if sign > 0.0 {
        Edges::End
    } else {
        Edges::None
    }
}

fn is_axis_aligned(rot: f64) -> bool {
    let r = rot.rem_euclid(90.0);
    r < 0.01 || r > 89.99
}

/// New boxes of the selected shapes for a transform.
fn plan(
    pres: &Presentation,
    placed: &[Placed],
    items: &[&Placed],
    t: Transform,
    snap: bool,
    tol: f64,
) -> Preview {
    let slide = slide_size(pres);
    let ids: Vec<u32> = items.iter().map(|p| p.id).collect();
    let others: Vec<Rect> = placed
        .iter()
        .filter(|p| !ids.contains(&p.id) && !items.iter().any(|i| i.path.starts_with(&p.path)))
        .map(|p| p.b.bounds())
        .collect();
    let union = Rect::union_all(items.iter().map(|p| p.b.bounds())).unwrap_or_default();
    let mut pv = Preview::default();
    match t {
        Transform::Move { dx, dy } => {
            let mut dx = dx;
            let mut dy = dy;
            if snap {
                let moved = Rect {
                    x: union.x + dx,
                    y: union.y + dy,
                    ..union
                };
                let s = guides::snap(&moved, &others, slide, tol, Edges::All, Edges::All);
                dx += s.dx;
                dy += s.dy;
                pv.guides = s.guides;
            }
            pv.dx = dx;
            pv.dy = dy;
            pv.boxes = items.iter().map(|p| p.b.translated(dx, dy)).collect();
        }
        Transform::Resize {
            handle,
            dx,
            dy,
            keep_aspect,
        } => {
            let base = if items.len() == 1 {
                items[0].b
            } else {
                BoxR::from_rect(union)
            };
            let mut nb = resize_box(&base, handle, dx, dy, keep_aspect);
            if snap && is_axis_aligned(nb.rot) && nb.rot.abs() < 0.01 {
                let (sx, sy) = handle.signs();
                let s = guides::snap(&nb.bounds(), &others, slide, tol, edges(sx), edges(sy));
                if s.dx != 0.0 || s.dy != 0.0 {
                    nb = resize_box(&base, handle, dx + s.dx, dy + s.dy, keep_aspect);
                }
                pv.guides =
                    guides::snap(&nb.bounds(), &others, slide, 0.01, edges(sx), edges(sy)).guides;
            }
            pv.boxes = if items.len() == 1 {
                vec![nb]
            } else {
                let to = nb.bounds();
                items.iter().map(|p| map_box(&p.b, &union, &to)).collect()
            };
        }
        Transform::Rotate { x, y, step } => {
            let (cx, cy) = if items.len() == 1 {
                (items[0].b.cx, items[0].b.cy)
            } else {
                (union.x + union.w / 2.0, union.y + union.h / 2.0)
            };
            let mut angle = pointer_angle(cx, cy, x, y);
            if step {
                angle = geom::normalize_deg((angle / 15.0).round() * 15.0);
            }
            pv.angle = Some(angle);
            pv.boxes = if items.len() == 1 {
                vec![BoxR {
                    rot: angle,
                    ..items[0].b
                }]
            } else {
                let (s, c) = angle.to_radians().sin_cos();
                items
                    .iter()
                    .map(|p| {
                        let (ox, oy) = (p.b.cx - cx, p.b.cy - cy);
                        BoxR {
                            cx: cx + ox * c - oy * s,
                            cy: cy + ox * s + oy * c,
                            rot: geom::normalize_deg(p.b.rot + angle),
                            ..p.b
                        }
                    })
                    .collect()
            };
        }
    }
    pv.bounds = if pv.boxes.len() == 1 {
        pv.boxes.first().copied()
    } else {
        Rect::union_all(pv.boxes.iter().map(BoxR::bounds)).map(BoxR::from_rect)
    };
    pv
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_transform(tx: &mut Tx<'_>, sel: &SlideSel, t: Transform, snap: bool, tol: f64) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items = selected(&placed, &sel.ids);
    if items.is_empty() {
        return refuse("noSelection");
    }
    let pv = plan(tx.pres, &placed, &items, t, snap, tol);
    for (p, b) in items.iter().zip(&pv.boxes) {
        let mut n = node_clone(tx.pres, sel.slide, &p.path)?;
        apply_box(tx.pres, sel.slide, &mut n, p, b);
        tx.replace_node(sel.slide, &p.path, n)?;
    }
    Ok(())
}

fn remove_animations(tx: &mut Tx<'_>, slide: usize, ids: &[u32]) -> R<()> {
    tx.fields(slide, |f| {
        if let Some(t) = &mut f.timing {
            t.animations.retain(|a| !ids.contains(&a.shape_id));
        }
    })?;
    Ok(())
}

fn cmd_delete(tx: &mut Tx<'_>, sel: &mut SlideSel) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let mut paths: Vec<Vec<usize>> = selected(&placed, &sel.ids)
        .iter()
        .map(|p| p.path.clone())
        .collect();
    if paths.is_empty() {
        return refuse("noSelection");
    }
    paths.sort();
    let mut removed = Vec::new();
    for path in paths.iter().rev() {
        let n = tx.remove_node(sel.slide, path)?;
        all_ids(&n, &mut removed);
        // A group left without children goes too.
        if path.len() > 1 {
            let parent = &path[..path.len() - 1];
            if ops::container(tx.pres, sel.slide, parent).is_some_and(Vec::is_empty) {
                let g = tx.remove_node(sel.slide, parent)?;
                all_ids(&g, &mut removed);
            }
        }
    }
    let has_timing = tx
        .pres
        .slides
        .get(sel.slide)
        .is_some_and(|s| s.timing.as_ref().is_some_and(|t| !t.animations.is_empty()));
    if has_timing {
        remove_animations(tx, sel.slide, &removed)?;
    }
    sel.ids.clear();
    Ok(())
}

fn cmd_duplicate(tx: &mut Tx<'_>, sel: &mut SlideSel) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items: Vec<Placed> = selected(&placed, &sel.ids).into_iter().cloned().collect();
    if items.is_empty() {
        return refuse("noSelection");
    }
    let mut next = next_shape_id(tx.pres, sel.slide);
    let mut new_ids = Vec::new();
    for p in items.iter().rev() {
        let mut n = node_clone(tx.pres, sel.slide, &p.path)?;
        apply_box(
            tx.pres,
            sel.slide,
            &mut n,
            p,
            &p.b.translated(DUPLICATE_OFFSET, DUPLICATE_OFFSET),
        );
        renumber(&mut n, &mut next);
        if let Some(nv) = n.nv() {
            new_ids.push(nv.id);
        }
        let (last, parent) = p.path.split_last().ok_or(OpError::NoContainer)?;
        tx.insert_nodes(sel.slide, parent, last + 1, vec![n])?;
    }
    new_ids.reverse();
    sel.ids = new_ids;
    Ok(())
}

fn cmd_zorder(tx: &mut Tx<'_>, sel: &SlideSel, to: ZOrder) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items = selected(&placed, &sel.ids);
    if items.is_empty() {
        return refuse("noSelection");
    }
    let mut parents: Vec<Vec<usize>> = items
        .iter()
        .map(|p| p.path[..p.path.len() - 1].to_vec())
        .collect();
    parents.sort();
    parents.dedup();
    for parent in parents {
        let old = ops::container(tx.pres, sel.slide, &parent)
            .cloned()
            .ok_or(OpError::NoContainer)?;
        let chosen: Vec<bool> = (0..old.len())
            .map(|i| {
                items.iter().any(|p| {
                    p.path.len() == parent.len() + 1
                        && p.path.starts_with(&parent)
                        && p.path[parent.len()] == i
                })
            })
            .collect();
        let mut idx: Vec<usize> = (0..old.len()).collect();
        match to {
            ZOrder::Front => idx.sort_by_key(|&i| chosen[i]),
            ZOrder::Back => idx.sort_by_key(|&i| !chosen[i]),
            ZOrder::Forward => {
                for i in (0..idx.len().saturating_sub(1)).rev() {
                    if chosen[idx[i]] && !chosen[idx[i + 1]] {
                        idx.swap(i, i + 1);
                    }
                }
            }
            ZOrder::Backward => {
                for i in 1..idx.len() {
                    if chosen[idx[i]] && !chosen[idx[i - 1]] {
                        idx.swap(i, i - 1);
                    }
                }
            }
        }
        let new: Vec<ShapeNode> = idx.iter().map(|&i| old[i].clone()).collect();
        let Some(first) = (0..old.len()).find(|&i| old[i] != new[i]) else {
            continue;
        };
        let last = (0..old.len())
            .rev()
            .find(|&i| old[i] != new[i])
            .unwrap_or(first);
        tx.op(SlideOp::Nodes {
            slide: sel.slide,
            parent,
            index: first,
            remove: old[first..=last].to_vec(),
            insert: new[first..=last].to_vec(),
        })?;
    }
    Ok(())
}

/// Bounding box (EMU) of a child transform, rotation included.
fn xfrm_bounds(x: &Xfrm) -> Option<(f64, f64, f64, f64)> {
    let (ox, oy) = x.off?;
    let (cx, cy) = x.ext?;
    let b = BoxR {
        cx: ox as f64 + cx as f64 / 2.0,
        cy: oy as f64 + cy as f64 / 2.0,
        w: cx as f64,
        h: cy as f64,
        rot: f64::from(x.rot.unwrap_or(0)) / 60_000.0,
    }
    .bounds();
    Some((b.x, b.y, b.right(), b.bottom()))
}

fn cmd_group(tx: &mut Tx<'_>, sel: &mut SlideSel) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items: Vec<Placed> = selected(&placed, &sel.ids).into_iter().cloned().collect();
    if items.len() < 2 {
        return refuse("groupNeedsTwo");
    }
    let parent = items[0].path[..items[0].path.len() - 1].to_vec();
    if items
        .iter()
        .any(|p| p.path.len() != parent.len() + 1 || !p.path.starts_with(&parent))
    {
        return refuse("groupSameLevel");
    }
    let mut children = Vec::new();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &items {
        let mut n = node_clone(tx.pres, sel.slide, &p.path)?;
        if let ShapeNode::Shape(s) = &n {
            if s.nv.placeholder.is_some() {
                return refuse("groupPlaceholder");
            }
        }
        // Pin inherited transforms before the shape moves into the group.
        apply_box(tx.pres, sel.slide, &mut n, p, &p.b);
        let x = node_xfrm(tx.pres, sel.slide, &n).ok_or(EditError::Refused("noTransform"))?;
        let (a, b, c, d) = xfrm_bounds(&x).ok_or(EditError::Refused("noTransform"))?;
        x0 = x0.min(a);
        y0 = y0.min(b);
        x1 = x1.max(c);
        y1 = y1.max(d);
        children.push(n);
    }
    let id = next_shape_id(tx.pres, sel.slide);
    let off = (x0.round() as i64, y0.round() as i64);
    let ext = ((x1 - x0).round() as i64, (y1 - y0).round() as i64);
    let group = build::group(id, off, ext, children).ok_or(EditError::Refused("build"))?;
    let indices: Vec<usize> = items.iter().map(|p| p.path[parent.len()]).collect();
    for p in items.iter().rev() {
        tx.remove_node(sel.slide, &p.path)?;
    }
    let max = indices.iter().copied().max().unwrap_or(0);
    let at = max + 1 - indices.len();
    tx.insert_nodes(sel.slide, &parent, at, vec![group])?;
    sel.ids = vec![id];
    Ok(())
}

fn cmd_ungroup(tx: &mut Tx<'_>, sel: &mut SlideSel) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let groups: Vec<Placed> = selected(&placed, &sel.ids)
        .into_iter()
        .filter(|p| {
            matches!(
                ops::node_at(tx.pres, sel.slide, &p.path),
                Some(ShapeNode::Group(_))
            )
        })
        .cloned()
        .collect();
    if groups.is_empty() {
        return refuse("noGroup");
    }
    let mut new_sel = Vec::new();
    for g in groups.iter().rev() {
        let Some(ShapeNode::Group(node)) = ops::node_at(tx.pres, sel.slide, &g.path).cloned()
        else {
            continue;
        };
        let mut kids = Vec::new();
        for (i, mut c) in node.children.into_iter().enumerate() {
            // Re-express each child in the group's parent space.
            if let Some(cp) = g.children.iter().find(|p| p.path.last() == Some(&i)) {
                let as_parent = Placed {
                    frame: g.frame,
                    ..cp.clone()
                };
                apply_box(tx.pres, sel.slide, &mut c, &as_parent, &cp.b);
            }
            if let Some(nv) = c.nv() {
                new_sel.push(nv.id);
            }
            kids.push(c);
        }
        let (last, parent) = g.path.split_last().ok_or(OpError::NoContainer)?;
        tx.remove_node(sel.slide, &g.path)?;
        tx.insert_nodes(sel.slide, parent, *last, kids)?;
    }
    sel.ids = new_sel;
    Ok(())
}

fn move_items(tx: &mut Tx<'_>, slide: usize, moves: &[(Placed, f64, f64)]) -> R<()> {
    for (p, dx, dy) in moves {
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            continue;
        }
        let mut n = node_clone(tx.pres, slide, &p.path)?;
        apply_box(tx.pres, slide, &mut n, p, &p.b.translated(*dx, *dy));
        tx.replace_node(slide, &p.path, n)?;
    }
    Ok(())
}

fn cmd_align(tx: &mut Tx<'_>, sel: &SlideSel, to: AlignTo) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items: Vec<Placed> = selected(&placed, &sel.ids).into_iter().cloned().collect();
    if items.is_empty() {
        return refuse("noSelection");
    }
    let (sw, sh) = slide_size(tx.pres);
    let target = if items.len() == 1 {
        Rect {
            x: 0.0,
            y: 0.0,
            w: sw,
            h: sh,
        }
    } else {
        Rect::union_all(items.iter().map(|p| p.b.bounds())).unwrap_or_default()
    };
    let moves: Vec<(Placed, f64, f64)> = items
        .into_iter()
        .map(|p| {
            let r = p.b.bounds();
            let (dx, dy) = match to {
                AlignTo::Left => (target.x - r.x, 0.0),
                AlignTo::Center => (target.x + target.w / 2.0 - (r.x + r.w / 2.0), 0.0),
                AlignTo::Right => (target.right() - r.right(), 0.0),
                AlignTo::Top => (0.0, target.y - r.y),
                AlignTo::Middle => (0.0, target.y + target.h / 2.0 - (r.y + r.h / 2.0)),
                AlignTo::Bottom => (0.0, target.bottom() - r.bottom()),
            };
            (p, dx, dy)
        })
        .collect();
    move_items(tx, sel.slide, &moves)
}

fn cmd_distribute(tx: &mut Tx<'_>, sel: &SlideSel, axis: Axis) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let mut items: Vec<(Placed, Rect)> = selected(&placed, &sel.ids)
        .into_iter()
        .map(|p| (p.clone(), p.b.bounds()))
        .collect();
    if items.is_empty() {
        return refuse("noSelection");
    }
    let (sw, sh) = slide_size(tx.pres);
    let horizontal = axis == Axis::Horizontal;
    let key = |r: &Rect| if horizontal { r.x } else { r.y };
    let size = |r: &Rect| if horizontal { r.w } else { r.h };
    items.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
    // Three or more shapes spread between the outer ones; fewer across the slide.
    let (start, end) = if items.len() >= 3 {
        let u = Rect::union_all(items.iter().map(|i| i.1)).unwrap_or_default();
        if horizontal {
            (u.x, u.right())
        } else {
            (u.y, u.bottom())
        }
    } else if horizontal {
        (0.0, sw)
    } else {
        (0.0, sh)
    };
    let total: f64 = items.iter().map(|i| size(&i.1)).sum();
    let n = items.len();
    let gap = if n > 1 {
        (end - start - total) / (n - 1) as f64
    } else {
        0.0
    };
    let mut pos = if n == 1 {
        start + (end - start - total) / 2.0
    } else {
        start
    };
    let mut moves = Vec::new();
    for (p, r) in items {
        let d = pos - key(&r);
        pos += size(&r) + gap;
        moves.push(if horizontal { (p, d, 0.0) } else { (p, 0.0, d) });
    }
    move_items(tx, sel.slide, &moves)
}

fn insert_top(tx: &mut Tx<'_>, sel: &mut SlideSel, node: Option<ShapeNode>) -> R<()> {
    let node = node.ok_or(EditError::Refused("build"))?;
    let id = node.nv().map_or(0, |nv| nv.id);
    let len = tx
        .pres
        .slides
        .get(sel.slide)
        .ok_or(OpError::NoSlide(sel.slide))?
        .data
        .shapes
        .len();
    tx.insert_nodes(sel.slide, &[], len, vec![node])?;
    sel.ids = vec![id];
    Ok(())
}

fn text_target(tx: &Tx<'_>, sel: &SlideSel, id: Option<u32>) -> R<Vec<usize>> {
    let placed = place_slide(tx.pres, sel.slide);
    let id = match id {
        Some(i) => i,
        None => match sel.ids.as_slice() {
            [one] => *one,
            _ => return refuse("noSelection"),
        },
    };
    find(&placed, id)
        .map(|p| p.path.clone())
        .ok_or(EditError::Refused("noShape"))
}

fn cmd_set_text(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    value: &str,
    fonts: &mut bo_text::FontManager,
) -> R<()> {
    let path = text_target(tx, sel, id)?;
    let placed = place_slide(tx.pres, sel.slide);
    let p = placed_at(&placed, &path).cloned();
    let mut n = node_clone(tx.pres, sel.slide, &path)?;
    let ShapeNode::Shape(s) = &mut n else {
        return refuse("noText");
    };
    let tb = s.text.get_or_insert_with(|| TextBody {
        list_style: Some(bo_model::slide::ListStyle::default()),
        ..TextBody::default()
    });
    text::set_text(tb, value);
    // §21.1.2.1.2 spAutoFit / §21.1.2.1.3 normAutofit while typing.
    let fitted = p
        .as_ref()
        .and_then(|p| rich::autofit(tx.pres, sel.slide, s, &p.b, fonts));
    if let (Some(p), Some(b)) = (p.as_ref(), fitted) {
        apply_box(tx.pres, sel.slide, &mut n, p, &b);
    }
    tx.replace_node(sel.slide, &path, n)?;
    Ok(())
}

fn cmd_format(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    f: &TextFormat,
    text_sel: Option<rich::TextSel>,
) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    if let Some(ts) = text_sel {
        // Inside the text: format the selected characters / paragraphs.
        let path = find(&placed, ts.id)
            .map(|p| p.path.clone())
            .ok_or(EditError::Refused("noShape"))?;
        let mut n = node_clone(tx.pres, sel.slide, &path)?;
        let state = match &n {
            ShapeNode::Shape(s) => format_state_at(tx.pres, sel.slide, s, Some((ts.start, ts.end))),
            _ => return refuse("noText"),
        };
        let ShapeNode::Shape(s) = &mut n else {
            return refuse("noText");
        };
        let Some(tb) = &mut s.text else {
            return refuse("noText");
        };
        let (mut a, mut b) = (ts.start.min(ts.end), ts.start.max(ts.end));
        if a == b && !f.is_paragraph() {
            // A collapsed caret formats the word around it.
            (a, b) = text::word_at(&text::body_text(tb), a);
        }
        let default_size = state.size.unwrap_or(18.0);
        text::apply_format_range(tb, f, &state, default_size, a, b);
        tx.replace_node(sel.slide, &path, n)?;
        return Ok(());
    }
    let paths: Vec<Vec<usize>> = selected(&placed, &sel.ids)
        .iter()
        .map(|p| p.path.clone())
        .collect();
    // Toggle state comes from the first shape with text.
    let mut state = None;
    for p in &paths {
        if let Some(ShapeNode::Shape(s)) = ops::node_at(tx.pres, sel.slide, p) {
            if s.text.is_some() {
                state = Some(format_state(tx.pres, sel.slide, s));
                break;
            }
        }
    }
    let Some(state) = state else {
        return refuse("noText");
    };
    let default_size = state.size.unwrap_or(18.0);
    for p in paths {
        let mut n = node_clone(tx.pres, sel.slide, &p)?;
        if let ShapeNode::Shape(s) = &mut n {
            if let Some(tb) = &mut s.text {
                text::apply_format(tb, f, &state, default_size);
                tx.replace_node(sel.slide, &p, n)?;
            }
        }
    }
    Ok(())
}

fn fill_of(hex: Option<&str>) -> R<Fill> {
    match hex {
        None => Ok(Fill::None),
        Some(h) => text::parse_hex(h)
            .map(|v| Fill::Solid(bo_model::slide::Color::srgb(v)))
            .ok_or(EditError::Refused("badColor")),
    }
}

fn cmd_fill(tx: &mut Tx<'_>, sel: &SlideSel, hex: Option<&str>) -> R<()> {
    let fill = fill_of(hex)?;
    let placed = place_slide(tx.pres, sel.slide);
    let paths: Vec<Vec<usize>> = selected(&placed, &sel.ids)
        .iter()
        .map(|p| p.path.clone())
        .collect();
    if paths.is_empty() {
        return refuse("noSelection");
    }
    for p in paths {
        let mut n = node_clone(tx.pres, sel.slide, &p)?;
        // §20.1.8 fill of spPr.
        if let ShapeNode::Shape(s) = &mut n {
            s.props.fill = Some(fill.clone());
            tx.replace_node(sel.slide, &p, n)?;
        }
    }
    Ok(())
}

fn cmd_line(tx: &mut Tx<'_>, sel: &SlideSel, hex: Option<&str>, width: Option<f64>) -> R<()> {
    let fill = fill_of(hex)?;
    let placed = place_slide(tx.pres, sel.slide);
    let paths: Vec<Vec<usize>> = selected(&placed, &sel.ids)
        .iter()
        .map(|p| p.path.clone())
        .collect();
    if paths.is_empty() {
        return refuse("noSelection");
    }
    for p in paths {
        let mut n = node_clone(tx.pres, sel.slide, &p)?;
        let props = match &mut n {
            ShapeNode::Shape(s) => &mut s.props,
            ShapeNode::Picture(pic) => &mut pic.props,
            ShapeNode::Connector(c) => &mut c.props,
            _ => continue,
        };
        // §20.1.2.2.24 ln: explicit line overrides the style's lnRef.
        let line = props.line.get_or_insert_with(Default::default);
        line.fill = Some(fill.clone());
        if let Some(w) = width {
            line.w = Some((w.clamp(0.0, 1584.0) * EMU_PT).round() as i64);
        }
        tx.replace_node(sel.slide, &p, n)?;
    }
    Ok(())
}

/// Layout for a new slide after the current one: the same layout, or the
/// master's content layout when the current slide is a title slide.
fn layout_for_new(pres: &Presentation, after: usize) -> Option<usize> {
    let cur = pres
        .slides
        .get(after)
        .map(|s| s.layout)
        .or(if pres.layouts.is_empty() {
            None
        } else {
            Some(0)
        })?;
    let l = pres.layouts.get(cur)?;
    if l.kind.as_deref() == Some("title") {
        if let Some(i) = pres
            .layouts
            .iter()
            .position(|o| o.master == l.master && o.kind.as_deref() == Some("obj"))
        {
            return Some(i);
        }
    }
    Some(cur)
}

fn cmd_add_slide(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    after: Option<usize>,
    layout: Option<usize>,
) -> R<()> {
    let n = tx.pres.slides.len();
    let after = after.unwrap_or(sel.slide).min(n.saturating_sub(1));
    let layout = match layout {
        Some(l) if l < tx.pres.layouts.len() => l,
        Some(_) => return refuse("noLayout"),
        None => layout_for_new(tx.pres, after).ok_or(EditError::Refused("noLayout"))?,
    };
    let slide = build::slide_from_layout(tx.pres, layout);
    let at = if n == 0 { 0 } else { after + 1 };
    tx.op(SlideOp::Slides {
        index: at,
        remove: vec![],
        insert: vec![slide],
    })?;
    *sel = SlideSel {
        slide: at,
        ids: vec![],
    };
    Ok(())
}

fn cmd_delete_slide(tx: &mut Tx<'_>, sel: &mut SlideSel, index: Option<usize>) -> R<()> {
    let i = index.unwrap_or(sel.slide);
    let s = tx.pres.slides.get(i).cloned().ok_or(OpError::NoSlide(i))?;
    tx.op(SlideOp::Slides {
        index: i,
        remove: vec![s],
        insert: vec![],
    })?;
    let n = tx.pres.slides.len();
    *sel = SlideSel {
        slide: if i >= n { n.saturating_sub(1) } else { i },
        ids: vec![],
    };
    Ok(())
}

fn cmd_duplicate_slide(tx: &mut Tx<'_>, sel: &mut SlideSel, index: Option<usize>) -> R<()> {
    let i = index.unwrap_or(sel.slide);
    let s = tx.pres.slides.get(i).ok_or(OpError::NoSlide(i))?;
    let copy = build::duplicate_slide(tx.pres, s);
    tx.op(SlideOp::Slides {
        index: i + 1,
        remove: vec![],
        insert: vec![copy],
    })?;
    *sel = SlideSel {
        slide: i + 1,
        ids: vec![],
    };
    Ok(())
}

fn cmd_move_slide(tx: &mut Tx<'_>, sel: &mut SlideSel, from: usize, to: usize) -> R<()> {
    let n = tx.pres.slides.len();
    if from >= n || to >= n {
        return refuse("noSlide");
    }
    if from == to {
        return Ok(());
    }
    let s = tx.pres.slides[from].clone();
    tx.op(SlideOp::Slides {
        index: from,
        remove: vec![s.clone()],
        insert: vec![],
    })?;
    tx.op(SlideOp::Slides {
        index: to,
        remove: vec![],
        insert: vec![s],
    })?;
    *sel = SlideSel {
        slide: to,
        ids: vec![],
    };
    Ok(())
}

fn cmd_set_notes(tx: &mut Tx<'_>, slide: usize, value: &str) -> R<()> {
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    if let Some(notes) = &s.notes {
        let mut notes = notes.clone();
        if let Some(i) = notes_body_index(&notes) {
            if let Some(ShapeNode::Shape(sh)) = notes.data.shapes.get_mut(i) {
                let tb = sh.text.get_or_insert_with(TextBody::default);
                text::set_text(tb, value);
            }
        } else {
            // No body placeholder: add one (§19.3.1.36 ph type="body").
            let fresh = build::notes(tx.pres, "/ppt/notesMasters/notesMaster1.xml", value)
                .ok_or(EditError::Refused("build"))?;
            if let Some(body) = fresh.data.shapes.into_iter().nth(1) {
                notes.data.shapes.push(body);
            }
        }
        tx.fields(slide, |f| f.notes = Some(notes))?;
        return Ok(());
    }
    if value.is_empty() {
        return Ok(());
    }
    let master = if let Some(m) = build::notes_master(tx.pres) {
        m
    } else {
        let nm = build::new_notes_master(tx.pres).ok_or(EditError::Refused("build"))?;
        let at = tx.pres.parts.len();
        tx.op(SlideOp::Parts {
            index: at,
            remove: vec![],
            insert: nm.parts,
        })?;
        let before = ops::PresFields::of(tx.pres);
        let mut after = before.clone();
        after.rels.push(nm.rel);
        after.extra.push(bo_pptx::presentation_extra(nm.id_list));
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
        nm.master
    };
    let notes = build::notes(tx.pres, &master, value).ok_or(EditError::Refused("build"))?;
    tx.fields(slide, |f| f.notes = Some(notes))?;
    Ok(())
}

/// Insert a picture part + relationship + `p:pic` (sized at 96 dpi, fitted to 80 % of the slide).
fn cmd_picture(tx: &mut Tx<'_>, sel: &mut SlideSel, bytes: Vec<u8>, name: &str) -> R<()> {
    let (pw, ph) = crate::edit::image_size(&bytes).ok_or(EditError::Refused("badImage"))?;
    let (ext, mime) = match bo_render::image::sniff(&bytes) {
        bo_render::image::Format::Jpeg => ("jpeg", "image/jpeg"),
        _ => ("png", "image/png"),
    };
    let (sw, sh) = slide_size(tx.pres);
    let mut w = f64::from(pw) * 0.75;
    let mut h = f64::from(ph) * 0.75;
    let k = (sw * 0.8 / w).min(sh * 0.8 / h).min(1.0);
    w *= k;
    h *= k;
    let slide = sel.slide;
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let rid = (1..=s.info.rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| s.info.rels.iter().all(|r| &r.id != id))
        .unwrap_or_default();
    let part = build::unique_part(tx.pres, "/ppt/media/image", ext);
    let target = format!("../media/{}", part.rsplit('/').next().unwrap_or_default());
    let id = next_shape_id(tx.pres, slide);
    let node = build::picture(id, &rid, name, (sw - w) / 2.0, (sh - h) / 2.0, w, h);
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![bo_model::slide::RawPart {
            name: part,
            content_type: Some(mime.into()),
            data: bytes,
        }],
    })?;
    tx.fields(slide, |f| {
        f.rels.push(Rel {
            id: rid.clone(),
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target,
            external: false,
        });
    })?;
    insert_top(tx, sel, node)
}

// ---------------------------------------------------------------------------
// Engine entry points
// ---------------------------------------------------------------------------

impl Engine {
    /// Run a JSON [`SlideCommand`]; returns the [`SlideEditResult`] as JSON.
    pub fn slide_command_json(&mut self, json: &str) -> crate::Result<String> {
        let cmd: SlideCommand =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        let r = self.slide_command(cmd)?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Run a slide editing command.
    pub fn slide_command(&mut self, cmd: SlideCommand) -> crate::Result<SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            let fonts = &mut self.fonts;
            ed.clamp(pres);
            if !cmd.keeps_text_selection() {
                ed.text = None;
            }
            match cmd {
                SlideCommand::State => {}
                SlideCommand::GoTo { slide } => {
                    if slide < pres.slides.len() {
                        ed.sel = SlideSel { slide, ids: vec![] };
                    } else {
                        out.error = Some("noSlide".into());
                    }
                }
                SlideCommand::Select { ids } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    ed.sel.ids = ids
                        .into_iter()
                        .filter(|i| find(&placed, *i).is_some())
                        .collect();
                }
                SlideCommand::SelectAll => {
                    ed.sel.ids = place_slide(pres, ed.sel.slide)
                        .iter()
                        .map(|p| p.id)
                        .collect();
                }
                SlideCommand::Click { x, y, extend, deep } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let hit = hit_test(&placed, x, y, HIT_TOLERANCE, deep);
                    match (hit, extend) {
                        (Some(id), true) => {
                            if let Some(i) = ed.sel.ids.iter().position(|v| *v == id) {
                                ed.sel.ids.remove(i);
                            } else {
                                ed.sel.ids.push(id);
                            }
                        }
                        (Some(id), false) => {
                            if !ed.sel.ids.contains(&id) {
                                ed.sel.ids = vec![id];
                            }
                        }
                        (None, true) => {}
                        (None, false) => ed.sel.ids.clear(),
                    }
                }
                SlideCommand::HitTest { x, y, deep } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    out.hits = Some(
                        hit_test(&placed, x, y, HIT_TOLERANCE, deep)
                            .into_iter()
                            .collect(),
                    );
                }
                SlideCommand::Transform {
                    transform,
                    snap,
                    tolerance,
                    preview,
                } => {
                    if preview {
                        let placed = place_slide(pres, ed.sel.slide);
                        let items = selected(&placed, &ed.sel.ids);
                        if items.is_empty() {
                            out.error = Some("noSelection".into());
                        } else {
                            out.preview =
                                Some(plan(pres, &placed, &items, transform, snap, tolerance));
                        }
                    } else {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            cmd_transform(tx, sel, transform, snap, tolerance)
                        });
                    }
                }
                SlideCommand::Delete => ed.mutate(pres, &mut out, cmd_delete),
                SlideCommand::Duplicate => ed.mutate(pres, &mut out, cmd_duplicate),
                SlideCommand::ZOrder { to } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_zorder(tx, sel, to));
                }
                SlideCommand::Group => ed.mutate(pres, &mut out, cmd_group),
                SlideCommand::Ungroup => ed.mutate(pres, &mut out, cmd_ungroup),
                SlideCommand::Align { to } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_align(tx, sel, to));
                }
                SlideCommand::Distribute { axis } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_distribute(tx, sel, axis));
                }
                SlideCommand::InsertShape { preset, rect } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        if bo_drawingml::preset(&preset).is_none() {
                            return refuse("unknownPreset");
                        }
                        let (sw, sh) = slide_size(tx.pres);
                        let r = place_new(tx.pres, rect, sw * 0.25, sh * 0.25);
                        let id = next_shape_id(tx.pres, sel.slide);
                        let n = build::preset_shape(id, &preset, r.x, r.y, r.w, r.h);
                        insert_top(tx, sel, n)
                    });
                }
                SlideCommand::InsertTextBox { text, rect } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let (sw, _) = slide_size(tx.pres);
                        let r = place_new(tx.pres, rect, sw * 0.4, 40.0);
                        let id = next_shape_id(tx.pres, sel.slide);
                        let n = build::text_box(id, &text, r.x, r.y, r.w, r.h);
                        insert_top(tx, sel, n)
                    });
                }
                SlideCommand::InsertTable { rows, cols, rect } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let (sw, _) = slide_size(tx.pres);
                        let r = place_new(tx.pres, rect, sw * 0.6, 120.0);
                        let id = next_shape_id(tx.pres, sel.slide);
                        let n = build::table(id, rows, cols, r.x, r.y, r.w);
                        insert_top(tx, sel, n)
                    });
                }
                SlideCommand::SetText {
                    id,
                    text,
                    merge,
                    sel: text_sel,
                } => {
                    let target = id.or(single(&ed.sel.ids));

                    let typing = if merge {
                        target.map(|t| (ed.sel.slide, t))
                    } else {
                        None
                    };
                    ed.mutate_as(pres, &mut out, typing, |tx, sel| {
                        cmd_set_text(tx, sel, id, &text, fonts)
                    });
                    if let (Some(t), Some([a, b]), None) = (target, text_sel, &out.error) {
                        let plain = text.replace("\r\n", "\n").replace('\r', "\n");
                        ed.text = Some(rich::TextSel {
                            id: t,
                            start: text::char_offset(&plain, a),
                            end: text::char_offset(&plain, b),
                        });
                    }
                }
                SlideCommand::FormatText { format, range } => {
                    let ts = match (range, ed.text) {
                        (Some([a, b]), cur) => {
                            let id = cur.map(|t| t.id).or(single(&ed.sel.ids));

                            id.and_then(|id| {
                                let s = shape_text(pres, ed.sel.slide, id)?;
                                Some(rich::TextSel {
                                    id,
                                    start: text::char_offset(&s, a),
                                    end: text::char_offset(&s, b),
                                })
                            })
                        }
                        (None, cur) => cur,
                    };
                    ed.mutate(pres, &mut out, |tx, sel| cmd_format(tx, sel, &format, ts));
                }
                SlideCommand::TextSelect { id, start, end } => {
                    let id = id.or(single(&ed.sel.ids));

                    match id.and_then(|id| Some((id, shape_text(pres, ed.sel.slide, id)?))) {
                        Some((id, s)) => {
                            if !ed.sel.ids.contains(&id) {
                                ed.sel.ids = vec![id];
                            }
                            ed.text = Some(rich::TextSel {
                                id,
                                start: text::char_offset(&s, start),
                                end: text::char_offset(&s, end),
                            });
                        }
                        None => out.error = Some("noText".into()),
                    }
                }
                SlideCommand::TextHit { x, y, extend } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let hit = ed.text.and_then(|t| {
                        let p = find(&placed, t.id)?;
                        let Some(ShapeNode::Shape(s)) = ops::node_at(pres, ed.sel.slide, &p.path)
                        else {
                            return None;
                        };
                        let n = ShapeNode::Shape(s.clone());
                        let b = rich::text_box(p, &n);
                        let g = rich::geometry(pres, ed.sel.slide, s, &b, fonts)?;
                        Some((t, rich::offset_at(&g, &b, x, y)))
                    });
                    match hit {
                        Some((t, off)) => {
                            ed.text = Some(rich::TextSel {
                                id: t.id,
                                start: if extend { t.start } else { off },
                                end: off,
                            });
                        }
                        None => out.error = Some("noText".into()),
                    }
                }
                SlideCommand::TextExit => ed.text = None,
                SlideCommand::SetFill { color } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_fill(tx, sel, color.as_deref())
                    });
                }
                SlideCommand::SetLine { color, width } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_line(tx, sel, color.as_deref(), width)
                    });
                }
                SlideCommand::AddSlide { after, layout } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_add_slide(tx, sel, after, layout)
                    });
                }
                SlideCommand::DeleteSlide { index } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_delete_slide(tx, sel, index));
                }
                SlideCommand::DuplicateSlide { index } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_duplicate_slide(tx, sel, index)
                    });
                }
                SlideCommand::MoveSlide { from, to } => {
                    ed.mutate(pres, &mut out, |tx, sel| cmd_move_slide(tx, sel, from, to));
                }
                SlideCommand::SetNotes { slide, text } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_set_notes(tx, slide.unwrap_or(sel.slide), &text)
                    });
                }
                SlideCommand::Undo => ed.undo(pres, &mut out),
                SlideCommand::Redo => ed.redo(pres, &mut out),
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// Insert a PNG/JPEG picture on the current slide.
    pub fn slide_insert_picture(
        &mut self,
        bytes: Vec<u8>,
        name: &str,
    ) -> crate::Result<SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.mutate(pres, &mut out, |tx, sel| cmd_picture(tx, sel, bytes, name));
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// Re-render every slide (after the slide list changed).
    fn rerender_slides(&mut self) -> crate::Result<()> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let mut pages = Vec::with_capacity(pres.slides.len());
        for i in 0..pres.slides.len() {
            pages.push(bo_pptx::render_slide(
                pres,
                i,
                &mut self.fonts,
                &mut self.images,
            )?);
        }
        self.display = Some(bo_render::DisplayList { pages });
        Ok(())
    }

    fn finish_slide_command(&mut self, out: Outcome) -> crate::Result<SlideEditResult> {
        let count = self.slide_count();
        let dirty: Vec<usize> = if out.all {
            self.rerender_slides()?;
            (0..count).collect()
        } else {
            let d: Vec<usize> = out.touched.iter().copied().filter(|&i| i < count).collect();
            for &i in &d {
                self.refresh_slide(i)?;
            }
            d
        };
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let ed = &self.slides;
        let slide = ed.sel.slide;
        let placed = place_slide(pres, slide);
        let items = selected(&placed, &ed.sel.ids);
        let mut selection = Vec::new();
        let mut format = None;
        let mut text_edit = None;
        if let Some(t) = ed.text {
            if let Some(p) = find(&placed, t.id) {
                if let Some(ShapeNode::Shape(s)) = ops::node_at(pres, slide, &p.path) {
                    format = Some(format_state_at(pres, slide, s, Some((t.start, t.end))));
                    let n = ShapeNode::Shape(s.clone());
                    let b = rich::text_box(p, &n);
                    let body = s.text.as_ref().map(text::body_text).unwrap_or_default();
                    let (caret, rects) = rich::geometry(pres, slide, s, &b, &mut self.fonts)
                        .map(|g| rich::selection_shapes(&g, &b, t.start, t.end))
                        .unwrap_or_default();
                    let (start, end) = rich::to_utf16(&body, t);
                    text_edit = Some(rich::TextEditOut {
                        id: t.id,
                        start,
                        end,
                        caret,
                        rects,
                        rect: p.b,
                        text: body,
                    });
                }
            }
        }
        for p in &items {
            let Some(n) = ops::node_at(pres, slide, &p.path) else {
                continue;
            };
            let nv = n.nv().cloned().unwrap_or_default();
            let (text, editable) = match n {
                ShapeNode::Shape(s) => {
                    if format.is_none() && s.text.is_some() {
                        format = Some(format_state(pres, slide, s));
                    }
                    (s.text.as_ref().map(text::body_text), true)
                }
                _ => (None, false),
            };
            let lock_aspect = match n {
                ShapeNode::Picture(pic) => pic
                    .nv
                    .drawing_props
                    .as_ref()
                    .and_then(|d| d.child("a:picLocks"))
                    .and_then(|l| l.attr("noChangeAspect"))
                    .is_none_or(|v| v == "1" || v == "true"),
                _ => false,
            };
            selection.push(ShapeInfo {
                id: nv.id,
                name: nv.name.clone(),
                kind: kind_of(n),
                rect: p.b,
                text,
                placeholder: nv
                    .placeholder
                    .as_ref()
                    .map(|ph| ph.kind.clone().unwrap_or_else(|| "obj".into())),
                lock_aspect,
                editable_text: editable,
            });
        }
        let bounds = match items.as_slice() {
            [] => None,
            [one] => Some(one.b),
            many => Rect::union_all(many.iter().map(|p| p.b.bounds())).map(BoxR::from_rect),
        };
        let placeholders = placed
            .iter()
            .filter(|p| {
                matches!(ops::node_at(pres, slide, &p.path), Some(ShapeNode::Shape(s))
                    if s.nv.placeholder.is_some()
                        && s.text.as_ref().is_none_or(|t| t.text().trim().is_empty()))
            })
            .map(|p| p.b)
            .collect();
        Ok(SlideEditResult {
            info: self.info()?,
            slide,
            handles: bounds.as_ref().map(handles).unwrap_or_default(),
            bounds,
            selection,
            placeholders,
            dirty,
            slides_changed: out.all,
            modified: ed.state_id() != ed.saved,
            can_undo: !ed.undo.is_empty(),
            can_redo: !ed.redo.is_empty(),
            format: format.unwrap_or_default(),
            notes: notes_text(pres, slide),
            layouts: pres
                .layouts
                .iter()
                .enumerate()
                .map(|(i, l)| LayoutInfo {
                    index: i,
                    name: l.data.name.clone().unwrap_or_default(),
                    kind: l.kind.clone(),
                })
                .collect(),
            preview: out.preview,
            hits: out.hits,
            error: out.error,
            collab: None,
            text_edit,
        })
    }
}

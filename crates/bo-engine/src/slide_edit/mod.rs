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
pub mod chart;
pub mod clip;
pub mod collab;
pub mod comments;
pub mod connect;
pub mod geom;
pub mod guides;
pub mod media;
pub mod morph;
pub mod ops;
pub mod rich;
pub mod show;
pub mod smartart;
pub mod table;
pub mod text;
pub mod video;

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
    /// 0-based index of the current slide.
    pub slide: usize,
    /// Selected shape ids (`cNvPr@id`), in selection order.
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
    Move {
        /// Horizontal distance in points.
        dx: f64,
        /// Vertical distance in points.
        dy: f64,
    },
    /// Drag a resize handle by (dx, dy) points.
    Resize {
        /// Which handle is dragged.
        handle: HandleKind,
        /// Horizontal drag distance in points.
        dx: f64,
        /// Vertical drag distance in points.
        dy: f64,
        /// Keep the aspect ratio (Shift held).
        #[serde(default)]
        keep_aspect: bool,
    },
    /// Drag the rotation handle to slide point (x, y); `step` snaps to 15°.
    Rotate {
        /// Pointer x in slide points.
        x: f64,
        /// Pointer y in slide points.
        y: f64,
        /// Snap the angle to 15° steps (Shift held).
        #[serde(default)]
        step: bool,
    },
}

/// Z-order changes (§19.3.1.45 spTree: later siblings are drawn on top).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZOrder {
    /// Bring to front (topmost).
    Front,
    /// Send to back (bottommost).
    Back,
    /// Bring forward one step.
    Forward,
    /// Send backward one step.
    Backward,
}

/// Alignment of shapes (to the slide for one shape, to the selection otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlignTo {
    /// Align left edges.
    Left,
    /// Align horizontal centres.
    Center,
    /// Align right edges.
    Right,
    /// Align top edges.
    Top,
    /// Align vertical middles.
    Middle,
    /// Align bottom edges.
    Bottom,
}

/// Distribution axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    /// Equal horizontal gaps.
    Horizontal,
    /// Equal vertical gaps.
    Vertical,
}

fn yes() -> bool {
    true
}
fn default_tolerance() -> f64 {
    4.0
}
fn default_site_tolerance() -> f64 {
    10.0
}

/// A slide editing command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SlideCommand {
    /// Current state, nothing changes.
    State,
    /// Password for the next `.pptx` saves ([MS-OFFCRYPTO] Agile Encryption);
    /// empty or absent removes it.
    SetSavePassword {
        /// The password; `None` or empty saves unencrypted.
        #[serde(default)]
        password: Option<String>,
    },
    /// Make `slide` current (clears the selection).
    GoTo {
        /// 0-based slide index.
        slide: usize,
    },
    /// Select shapes by id on the current slide (replaces the selection).
    Select {
        /// Shape ids (`cNvPr@id`).
        ids: Vec<u32>,
    },
    /// Select every top-level shape of the current slide.
    SelectAll,
    /// Marquee: select the top-level shapes lying entirely inside `rect`
    /// (`extend` adds them to the selection).
    SelectRect {
        /// Marquee rectangle in slide points.
        rect: Rect,
        /// Add to the current selection instead of replacing it.
        #[serde(default)]
        extend: bool,
    },
    /// Select the next (or previous) shape in z-order (Tab / Shift+Tab).
    SelectNext {
        /// Select the previous shape instead (Shift+Tab).
        #[serde(default)]
        back: bool,
    },
    /// Select the topmost shape at a slide point; `extend` (Shift) toggles it;
    /// `deep` selects inside groups.
    Click {
        /// Point x in slide points.
        x: f64,
        /// Point y in slide points.
        y: f64,
        /// Toggle the hit shape in the selection (Shift held).
        #[serde(default)]
        extend: bool,
        /// Select inside groups instead of the whole group.
        #[serde(default)]
        deep: bool,
    },
    /// Shapes at a point (topmost first), selection unchanged.
    HitTest {
        /// Point x in slide points.
        x: f64,
        /// Point y in slide points.
        y: f64,
        /// Hit shapes inside groups.
        #[serde(default)]
        deep: bool,
    },
    /// Move / resize / rotate the selection; `preview` only computes the
    /// result (boxes and smart guides) without changing the deck.
    Transform {
        /// Move, resize or rotate.
        transform: Transform,
        /// Snap to smart guides (other shapes, slide edges and centre).
        #[serde(default = "yes")]
        snap: bool,
        /// Snap distance in points.
        #[serde(default = "default_tolerance")]
        tolerance: f64,
        /// Only compute the preview; the deck is not changed.
        #[serde(default)]
        preview: bool,
    },
    /// Delete the selected shapes.
    Delete,
    /// Duplicate the selected shapes (offset by 12 points).
    Duplicate,
    /// Change the z-order of the selected shapes.
    ZOrder {
        /// Where the shapes move in the stacking order.
        to: ZOrder,
    },
    /// Group the selected shapes.
    Group,
    /// Ungroup the selected groups.
    Ungroup,
    /// Align the selected shapes.
    Align {
        /// Which edge or centre to align.
        to: AlignTo,
    },
    /// Distribute the selected shapes evenly.
    Distribute {
        /// Distribution direction.
        axis: Axis,
    },
    /// Preset shape (§20.1.10.56 ST_ShapeType, e.g. `rect`, `ellipse`, `rightArrow`).
    InsertShape {
        /// Preset geometry name.
        preset: String,
        /// Box in slide points; `None` = a default size centred on the slide.
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Insert a text box.
    InsertTextBox {
        /// Initial text.
        #[serde(default)]
        text: String,
        /// Box in slide points; `None` = a default size centred on the slide.
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Insert a table.
    InsertTable {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
        /// Box in slide points; `None` = a default size centred on the slide.
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Replace the text of a shape (`\n` paragraphs, `\u{b}` line breaks).
    /// `merge` joins the change with the previous typing in the same shape
    /// (one undo step); `sel` is the text selection afterwards (UTF-16).
    SetText {
        /// Shape id; `None` = the selected shape.
        #[serde(default)]
        id: Option<u32>,
        /// New text in the editing format.
        text: String,
        /// Join with the previous typing in the same shape (one undo step).
        #[serde(default)]
        merge: bool,
        /// Text selection `[start, end]` afterwards, UTF-16 offsets.
        #[serde(default)]
        sel: Option<[usize; 2]>,
        /// Text of table cell `[row, col]` of table `id` instead of the shape's.
        #[serde(default)]
        cell: Option<[usize; 2]>,
    },
    /// Format the text selection (`range`, UTF-16, or the current text
    /// selection) or — outside text editing — the whole text of the selection.
    FormatText {
        /// Character / paragraph properties to set; absent fields are kept.
        format: TextFormat,
        /// `[start, end]` UTF-16 offsets; `None` = the current text selection.
        #[serde(default)]
        range: Option<[usize; 2]>,
    },
    /// Start / move the text selection inside a shape (UTF-16 offsets).
    TextSelect {
        /// Shape id; `None` = the selected shape.
        #[serde(default)]
        id: Option<u32>,
        /// Selection anchor, UTF-16 offset into the shape text.
        start: usize,
        /// Selection focus, UTF-16 offset into the shape text.
        end: usize,
        /// Table cell `[row, col]` of table `id`.
        #[serde(default)]
        cell: Option<[usize; 2]>,
    },
    /// Place the caret at a slide point of the edited shape (`extend` = Shift).
    TextHit {
        /// Point x in slide points.
        x: f64,
        /// Point y in slide points.
        y: f64,
        /// Extend the text selection (Shift held).
        #[serde(default)]
        extend: bool,
    },
    /// Edit the text of the table cell at a slide point on the canvas (caret at the
    /// point; the table is the selected one).
    CellTextHit {
        /// Point x in slide points.
        x: f64,
        /// Point y in slide points.
        y: f64,
    },
    /// Edit the next (or previous, `back`) cell of the table whose cell text is edited,
    /// its text selected (Tab / Shift+Tab).
    CellNext {
        /// Go to the previous cell instead (Shift+Tab).
        #[serde(default)]
        back: bool,
    },
    /// Leave text editing.
    TextExit,
    /// Select cells `(r0, c0)..=(r1, c1)` of a table (default: the selected one).
    TableSelect {
        /// Table shape id; `None` = the selected table.
        #[serde(default)]
        id: Option<u32>,
        /// First row (0-based).
        r0: usize,
        /// First column (0-based).
        c0: usize,
        /// Last row (0-based, inclusive); `None` = `r0`.
        #[serde(default)]
        r1: Option<usize>,
        /// Last column (0-based, inclusive); `None` = `c0`.
        #[serde(default)]
        c1: Option<usize>,
    },
    /// Select the cell at a slide point (`extend` = Shift grows the range).
    TableHit {
        /// Point x in slide points.
        x: f64,
        /// Point y in slide points.
        y: f64,
        /// Grow the cell range (Shift held).
        #[serde(default)]
        extend: bool,
    },
    /// Text of a cell (default: the first selected cell); `merge` joins typing.
    SetCellText {
        /// 0-based row; `None` = the first selected cell's row.
        #[serde(default)]
        row: Option<usize>,
        /// 0-based column; `None` = the first selected cell's column.
        #[serde(default)]
        col: Option<usize>,
        /// New cell text.
        text: String,
        /// Join with the previous typing in the same cell (one undo step).
        #[serde(default)]
        merge: bool,
    },
    /// Insert a row above (or `below`) the selected cells.
    TableInsertRow {
        /// Insert below instead of above.
        #[serde(default)]
        below: bool,
    },
    /// Insert a column left (or `right`) of the selected cells.
    TableInsertCol {
        /// Insert to the right instead of the left.
        #[serde(default)]
        right: bool,
    },
    /// Delete the rows of the selected cells.
    TableDeleteRows,
    /// Delete the columns of the selected cells.
    TableDeleteCols,
    /// Merge the selected cells.
    TableMerge,
    /// Split the selected merged cell.
    TableSplit,
    /// Fill of the selected cells: `RRGGBB` or `None` (back to the style).
    TableFill {
        /// Fill colour `RRGGBB`; `None` uses the table style.
        color: Option<String>,
    },
    /// Table style options (§21.1.3.15 tblPr firstRow / bandRow).
    TableOptions {
        /// Emphasize the header row; `None` keeps it.
        #[serde(default)]
        first_row: Option<bool>,
        /// Banded rows; `None` keeps them.
        #[serde(default)]
        band_row: Option<bool>,
    },
    /// Leave table cell selection.
    TableExit,
    /// Drag adjust handle `index` of the selected preset shape to slide point
    /// (x, y) (§20.1.9.5 avLst); `merge` joins the previous drag step.
    Adjust {
        /// 0-based adjust handle index.
        index: usize,
        /// Pointer x in slide points.
        x: f64,
        /// Pointer y in slide points.
        y: f64,
        /// Join with the previous drag step (one undo step).
        #[serde(default)]
        merge: bool,
    },
    /// Drag end `end` (0 = start, 1 = end) of the selected connector to (x, y):
    /// it attaches to the nearest connection site within `tolerance` points.
    ConnectorEnd {
        /// Which end: 0 = start, 1 = end.
        end: usize,
        /// Pointer x in slide points.
        x: f64,
        /// Pointer y in slide points.
        y: f64,
        /// Join with the previous drag step (one undo step).
        #[serde(default)]
        merge: bool,
        /// Snap distance to connection sites, in points.
        #[serde(default = "default_site_tolerance")]
        tolerance: f64,
    },
    /// Connect the two selected shapes with a connector between their
    /// nearest connection sites (`preset` default `straightConnector1`).
    InsertConnector {
        /// Connector preset (`straightConnector1`, `bentConnector3`, …).
        #[serde(default)]
        preset: Option<String>,
    },
    /// Copy the selected shapes to the internal clipboard (full shapes and
    /// the pictures they use); the result carries text and a PNG for the
    /// system clipboard.
    Copy,
    /// Copy, then delete the selection.
    Cut,
    /// Paste the clipboard: shapes onto the current slide (offset when pasted
    /// where they came from), slides after the current one.
    Paste,
    /// Copy slides (default: the current one).
    CopySlides {
        /// 0-based slide indices; `None` = the current slide.
        #[serde(default)]
        indices: Option<Vec<usize>>,
    },
    /// Cut slides (default: the current one): copy, then delete them.
    CutSlides {
        /// 0-based slide indices; `None` = the current slide.
        #[serde(default)]
        indices: Option<Vec<usize>>,
    },
    /// `RRGGBB` or `None` for no fill.
    SetFill {
        /// Fill colour of the selected shapes.
        color: Option<String>,
    },
    /// `RRGGBB` or `None` for no line; width in points.
    SetLine {
        /// Line colour of the selected shapes.
        color: Option<String>,
        /// Line width in points; `None` keeps it.
        #[serde(default)]
        width: Option<f64>,
    },
    /// New slide after `after` (default: current) from `layout` (default: like the current one).
    AddSlide {
        /// 0-based slide index to insert after; `None` = the current slide.
        #[serde(default)]
        after: Option<usize>,
        /// 0-based layout index (see `layouts`); `None` = the current slide's layout.
        #[serde(default)]
        layout: Option<usize>,
    },
    /// Delete a slide (default: the current one).
    DeleteSlide {
        /// 0-based slide index; `None` = the current slide.
        #[serde(default)]
        index: Option<usize>,
    },
    /// Duplicate a slide (default: the current one) after it.
    DuplicateSlide {
        /// 0-based slide index; `None` = the current slide.
        #[serde(default)]
        index: Option<usize>,
    },
    /// Move a slide to another position.
    MoveSlide {
        /// 0-based index of the slide to move.
        from: usize,
        /// 0-based index it moves to.
        to: usize,
    },
    /// Replace the speaker notes of a slide.
    SetNotes {
        /// 0-based slide index; `None` = the current slide.
        #[serde(default)]
        slide: Option<usize>,
        /// Notes text in the editing format (`\n` paragraphs).
        text: String,
    },
    /// Transition of `slides` (default: the current one; `all`: every slide).
    SetTransition {
        /// 0-based slide indices; `None` = the current slide.
        #[serde(default)]
        slides: Option<Vec<usize>>,
        /// Apply to every slide of the presentation.
        #[serde(default)]
        all: bool,
        /// The transition to set (§19.3.1.50 transition).
        transition: show::TransitionSpec,
    },
    /// Add an effect for each selected shape (main sequence of the current slide).
    AddAnimation {
        /// Effect class, preset, timing and options of the new effect.
        #[serde(flatten)]
        spec: show::AnimSpec,
    },
    /// Change effect `index` of the current slide.
    SetAnimation {
        /// 0-based index of the effect in the main sequence.
        index: usize,
        /// New effect settings.
        #[serde(flatten)]
        spec: show::AnimSpec,
    },
    /// Remove effect `index` of the current slide.
    RemoveAnimation {
        /// 0-based index of the effect in the main sequence.
        index: usize,
    },
    /// Move effect `from` to position `to` of the sequence.
    MoveAnimation {
        /// Current 0-based position of the effect.
        from: usize,
        /// New 0-based position of the effect.
        to: usize,
    },
    /// Animation painter: give each selected shape the effects of shape `from` (appended).
    CopyAnimations {
        /// Shape id (`cNvPr@id`) whose effects are copied.
        from: u32,
    },
    /// What the slide show needs to play `slide` (layers, effects, media); read-only.
    Show {
        /// 0-based slide index.
        slide: usize,
    },
    /// Insert a chart (`column`, `bar`, `line`, `area`, `pie`, `doughnut`).
    InsertChart {
        /// Chart type (see above).
        kind: String,
        /// Chart title; `None` = no title.
        #[serde(default)]
        title: Option<String>,
        /// Category/series grid; `None` = a sample grid.
        #[serde(default)]
        data: Option<chart::ChartData>,
        /// `r` | `l` | `t` | `b` | `tr` | `none` (default: bottom, right for pies).
        #[serde(default)]
        legend: Option<String>,
        /// Frame of the chart in points; `None` = a default frame on the slide.
        #[serde(default)]
        rect: Option<Rect>,
        /// Chart template to format the new chart with (CORE-CHART-007).
        #[serde(default)]
        template: Option<bo_sheet::chart_style::ChartTemplate>,
        /// Titles the template adds (localised by the UI).
        #[serde(default)]
        texts: bo_sheet::chart_style::LayoutTexts,
    },
    /// Chart Design gallery of chart `id` (default: the selected one): previews of the chart
    /// styles and quick layouts, colour variations from the theme, and the chart's template
    /// (in the result's `chartGallery`; read-only).
    ChartGallery {
        /// Shape id of the chart's graphic frame; `None` = the selected chart.
        #[serde(default)]
        id: Option<u32>,
        /// Preview width in points (default 96).
        #[serde(default)]
        width: Option<f64>,
        /// Preview height in points (default 64).
        #[serde(default)]
        height: Option<f64>,
    },
    /// Insert a SmartArt from the BlueOffice layout library (CORE-SMART-003).
    InsertSmartArt {
        /// Layout category: `list` | `process` | `cycle` | `hierarchy` | `matrix` | `pyramid` |
        /// `relationship`.
        layout: String,
        /// Text outline (one node per line, `level` 0 = top).
        items: Vec<smartart::SmartItem>,
        /// Colours: `colorful` (default) | `blue` | `green` | `orange` | `purple` | `gray`.
        #[serde(default)]
        colors: Option<String>,
        /// Look: `fill` (default) | `outline`.
        #[serde(default)]
        style: Option<String>,
        /// Frame in points; `None` = a default frame on the slide.
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Change the text outline and/or layout of SmartArt `id` (default: the selected one).
    SetSmartArt {
        /// Shape id of the frame; `None` = the selected SmartArt.
        #[serde(default)]
        id: Option<u32>,
        /// New layout category.
        #[serde(default)]
        layout: Option<String>,
        /// New text outline.
        #[serde(default)]
        items: Option<Vec<smartart::SmartItem>>,
        /// New colours.
        #[serde(default)]
        colors: Option<String>,
        /// New look.
        #[serde(default)]
        style: Option<String>,
    },
    /// Replace SmartArt `id` (default: the selected one) by a group of ordinary shapes.
    SmartArtToShapes {
        /// Shape id of the frame; `None` = the selected SmartArt.
        #[serde(default)]
        id: Option<u32>,
    },
    /// Change type, title, legend or data of chart `id` (default: the selected one).
    SetChart {
        /// Shape id of the chart's graphic frame; `None` = the selected chart.
        #[serde(default)]
        id: Option<u32>,
        /// Fields to change (absent = unchanged).
        #[serde(flatten)]
        spec: chart::ChartSpec,
    },
    /// New comment thread on the current slide, or a reply to thread `parent`
    /// ([MS-PPTX] modern comments).
    AddComment {
        /// Comment text.
        text: String,
        /// Author display name.
        #[serde(default)]
        author: String,
        /// Creation time (ISO 8601); now when omitted.
        #[serde(default)]
        date: Option<String>,
        /// Thread replied to.
        #[serde(default)]
        parent: Option<String>,
    },
    /// Resolve (`done`) or reopen a thread of the current slide.
    ResolveComment {
        /// Thread id.
        id: String,
        /// Resolved.
        done: bool,
    },
    /// Delete a thread (with its replies) or one reply of the current slide.
    DeleteComment {
        /// Thread or reply id.
        id: String,
    },
    /// Undo the last change.
    Undo,
    /// Redo the last undone change.
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
                | SlideCommand::CellTextHit { .. }
                | SlideCommand::CellNext { .. }
                | SlideCommand::Undo
                | SlideCommand::Redo
        )
    }

    /// Commands that keep the table cell selection.
    fn keeps_table_selection(&self) -> bool {
        matches!(
            self,
            SlideCommand::State
                | SlideCommand::TableSelect { .. }
                | SlideCommand::TableHit { .. }
                | SlideCommand::SetCellText { .. }
                | SlideCommand::TableInsertRow { .. }
                | SlideCommand::TableInsertCol { .. }
                | SlideCommand::TableDeleteRows
                | SlideCommand::TableDeleteCols
                | SlideCommand::TableMerge
                | SlideCommand::TableSplit
                | SlideCommand::TableFill { .. }
                | SlideCommand::TableOptions { .. }
                | SlideCommand::FormatText { .. }
                | SlideCommand::SetText { .. }
                | SlideCommand::TextSelect { .. }
                | SlideCommand::TextHit { .. }
                | SlideCommand::CellTextHit { .. }
                | SlideCommand::CellNext { .. }
                | SlideCommand::TextExit
                | SlideCommand::Undo
                | SlideCommand::Redo
        )
    }
}

/// Add or replace a package part; a new table styles part is also related
/// from the presentation part (ECMA-376 Part 1, §13.3.11).
fn ensure_part(tx: &mut Tx<'_>, part: bo_model::slide::RawPart) -> R<()> {
    if let Some(i) = tx
        .pres
        .parts
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(&part.name))
    {
        let old = tx.pres.parts[i].clone();
        tx.op(SlideOp::Parts {
            index: i,
            remove: vec![old],
            insert: vec![part],
        })?;
        return Ok(());
    }
    let target = part.name.trim_start_matches("/ppt/").to_owned();
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![part],
    })?;
    let before = ops::PresFields::of(tx.pres);
    if !before.rels.iter().any(|r| r.target == target) {
        let mut after = before.clone();
        let mut n = 1;
        let id = loop {
            let id = format!("rId{n}");
            let taken = tx.pres.info.rels.iter().any(|r| r.id == id)
                || tx.pres.slides.iter().any(|s| s.pres_rel_id == id)
                || tx.pres.masters.iter().any(|m| m.pres_rel_id == id);
            if !taken {
                break id;
            }
            n += 1;
        };
        after.rels.push(Rel {
            id,
            rel_type: format!("{}tableStyles", bo_pptx::RELATIONSHIP_NS),
            target,
            external: false,
        });
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
    }
    Ok(())
}

/// Table of a graphic frame on a slide.
fn table_of(pres: &Presentation, slide: usize, id: u32) -> Option<bo_model::slide::Table> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    match ops::node_at(pres, slide, &p.path)? {
        ShapeNode::Frame(f) => match &f.content {
            bo_model::slide::GraphicContent::Table(t) => Some(t.clone()),
            bo_model::slide::GraphicContent::Other(_) => None,
        },
        _ => None,
    }
}

/// Change the table of frame `id`; the frame keeps the grid's size.
fn edit_table(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: u32,
    f: impl FnOnce(&mut bo_model::slide::Table) -> bool,
) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let p = find(&placed, id).ok_or(EditError::Refused("noTable"))?;
    let mut n = node_clone(tx.pres, sel.slide, &p.path)?;
    let ShapeNode::Frame(fr) = &mut n else {
        return refuse("noTable");
    };
    let bo_model::slide::GraphicContent::Table(t) = &mut fr.content else {
        return refuse("noTable");
    };
    if !f(t) {
        return refuse("tableEdit");
    }
    // §21.1.3.2 gridCol / §21.1.3.18 tr@h define the frame size.
    let (w, h) = table::size(t);
    fr.xfrm.ext = Some((w, h));
    tx.replace_node(sel.slide, &p.path, n)?;
    Ok(())
}

/// The only selected id.
fn single(ids: &[u32]) -> Option<u32> {
    match ids {
        [one] => Some(*one),
        _ => None,
    }
}

/// Text being edited: of the shape or of its table cell, in the editing format.
fn edit_text(pres: &Presentation, slide: usize, t: rich::TextSel) -> Option<String> {
    match t.cell {
        Some(rc) => cell_text(pres, slide, t.id, rc),
        None => shape_text(pres, slide, t.id),
    }
}

/// Text of a table cell in the editing format (§21.1.3.16 tc txBody).
fn cell_text(pres: &Presentation, slide: usize, id: u32, (r, c): (usize, usize)) -> Option<String> {
    let t = table_of(pres, slide, id)?;
    let cell = t.rows.get(r)?.cells.get(c)?;
    Some(cell.text.as_ref().map(text::body_text).unwrap_or_default())
}

/// A shape holding a cell's text (format state and formatting reuse the shape code).
fn cell_shape(t: &bo_model::slide::Table, (r, c): (usize, usize)) -> bo_model::slide::Shape {
    bo_model::slide::Shape {
        text: t
            .rows
            .get(r)
            .and_then(|row| row.cells.get(c))
            .and_then(|cell| cell.text.clone()),
        ..bo_model::slide::Shape::default()
    }
}

/// Caret geometry of a table cell's text and the cell's box on the slide.
fn cell_geometry(
    pres: &Presentation,
    slide: usize,
    frame: &BoxR,
    t: &bo_model::slide::Table,
    (r, c): (usize, usize),
    fonts: &mut bo_text::FontManager,
) -> Option<(bo_pptx::TextGeometry, BoxR)> {
    let (g, [x, y, w, h]) = bo_pptx::table_cell_text_geometry(pres, slide, t, r, c, fonts)?;
    let (cx, cy) = frame.to_slide(x + w / 2.0 - frame.w / 2.0, y + h / 2.0 - frame.h / 2.0);
    Some((
        g,
        BoxR {
            cx,
            cy,
            w,
            h,
            rot: frame.rot,
        },
    ))
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
    /// Shape id (`cNvPr@id`).
    pub id: u32,
    /// Shape name (`cNvPr@name`).
    pub name: String,
    /// `shape` | `picture` | `group` | `connector` | `table` | `graphic`.
    pub kind: &'static str,
    /// Box in slide points.
    pub rect: BoxR,
    /// Text in the editing format (shapes only).
    pub text: Option<String>,
    /// Placeholder type (`title`, `body`, … ; `obj` when untyped).
    pub placeholder: Option<String>,
    /// Resize keeps the aspect ratio by default (pictures).
    pub lock_aspect: bool,
    /// Its text can be edited in place.
    pub editable_text: bool,
}

/// A slide layout new slides can be based on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutInfo {
    /// 0-based layout index (for `addSlide`).
    pub index: usize,
    /// Layout name.
    pub name: String,
    /// Layout type (§19.7.15 `ST_SlideLayoutType`, e.g. `titleOnly`).
    pub kind: Option<String>,
}

/// Result of a transform preview.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// Snapped box of each selected shape.
    pub boxes: Vec<BoxR>,
    /// Box around all of them.
    pub bounds: Option<BoxR>,
    /// Smart guide lines to draw.
    pub guides: Vec<Guide>,
    /// Applied move delta after snapping.
    pub dx: f64,
    /// Applied vertical move delta after snapping.
    pub dy: f64,
    /// Rotation angle (rotate only), degrees.
    pub angle: Option<f64>,
}

/// Result of a slide command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideEditResult {
    /// Document summary (name, kind, slide count, …).
    pub info: DocInfo,
    /// Current slide.
    pub slide: usize,
    /// Selected shapes.
    pub selection: Vec<ShapeInfo>,
    /// Box around the selection (the shape's own box for one shape).
    pub bounds: Option<BoxR>,
    /// Resize and rotate handles of the selection.
    pub handles: Vec<Handle>,
    /// Empty placeholders of the current slide (drawn as dashed outlines).
    pub placeholders: Vec<BoxR>,
    /// Slides whose display list changed.
    pub dirty: Vec<usize>,
    /// The slide list changed (count or order).
    pub slides_changed: bool,
    /// Changed since opened or last saved.
    pub modified: bool,
    /// Undo is available.
    pub can_undo: bool,
    /// Redo is available.
    pub can_redo: bool,
    /// Ribbon formatting state of the selection.
    pub format: FormatState,
    /// Notes of the current slide in the editing format.
    pub notes: String,
    /// Slide layouts of the deck (for `addSlide`).
    pub layouts: Vec<LayoutInfo>,
    /// Result of a `transform` preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
    /// Shape ids at the point of `hitTest`, topmost first.
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
    /// Table being edited: cells and selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<table::TableOut>,
    /// Adjust handles of the selected preset shape (slide points).
    pub adjust: Vec<connect::AdjustOut>,
    /// The selected connector's ends and attachment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector: Option<connect::ConnectorOut>,
    /// Connection sites of the slide's shapes while a connector is selected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sites: Vec<[f64; 2]>,
    /// What the internal clipboard holds: `shapes` | `slides`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<&'static str>,
    /// After copy / cut: content for the system clipboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<ClipboardOut>,
    /// Transition of the current slide.
    pub transition: Option<show::TransitionOut>,
    /// Main animation sequence of the current slide.
    pub animations: Vec<show::AnimOut>,
    /// Top-level shapes of the current slide (id, name): what a trigger can be.
    pub objects: Vec<show::ObjectOut>,
    /// Video and audio clips of the current slide.
    pub media: Vec<show::MediaOut>,
    /// The selected chart (type, title, legend, data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<chart::ChartOut>,
    /// Result of `chartGallery`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_gallery: Option<serde_json::Value>,
    /// The selected SmartArt (layout and text outline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart: Option<smartart::SmartArtOut>,
    /// Answer of the `show` command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<show::ShowOut>,
    /// Comment threads of the current slide ([MS-PPTX] modern comments).
    pub comments: Vec<comments::ThreadOut>,
}

/// System clipboard content of a copy: plain text and a PNG (base64).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardOut {
    /// Plain text of the copied shapes.
    pub text: String,
    /// PNG image of the copied shapes, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
}

/// Why a command was refused (codes the UI can translate).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// An operation failed.
    #[error(transparent)]
    Op(#[from] OpError),
    /// The command does not apply to the current selection (message code).
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
    typing: Option<(usize, u32, u8)>,
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
    /// Selected cells while a table is edited.
    table: Option<table::TableSel>,
}

#[derive(Default)]
struct Outcome {
    touched: BTreeSet<usize>,
    all: bool,
    preview: Option<Preview>,
    hits: Option<Vec<u32>>,
    error: Option<String>,
    /// Shapes just copied: (slide, top-level ids, bounds, text) for the system clipboard.
    copied: Option<(usize, Vec<u32>, Rect, String)>,
    /// Slides just copied (text only).
    copied_text: Option<String>,
    /// Slide asked for by the `show` command.
    show: Option<usize>,
    /// Gallery asked for by `chartGallery`.
    chart_gallery: Option<serde_json::Value>,
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
            self.text = edit_text(pres, self.sel.slide, t).map(|s| {
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
        typing: Option<(usize, u32, u8)>,
        f: impl FnOnce(&mut Tx<'_>, &mut SlideSel) -> R<()>,
    ) {
        let before = self.sel.clone();
        let mut sel = self.sel.clone();
        let mut tx = Tx::new(pres);
        let res = f(&mut tx, &mut sel).and_then(|()| reroute_connectors(&mut tx));
        match res {
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
        n if chart::is_chart(n) => "chart",
        n if smartart::is_smartart(n) => "smartart",
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
    !(0.01..=89.99).contains(&r)
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
        // The group's flips move onto its children (§20.1.7.6 xfrm flipH/flipV).
        let gx = node.props.xfrm.clone().unwrap_or_default();
        let (fh, fv) = (gx.flip_h.unwrap_or(false), gx.flip_v.unwrap_or(false));
        let mut kids = Vec::new();
        for (i, mut c) in node.children.into_iter().enumerate() {
            if fh || fv {
                toggle_flips(&mut c, fh, fv);
            }
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

/// Toggle the flips of a node's own transform.
fn toggle_flips(n: &mut ShapeNode, fh: bool, fv: bool) {
    let x = match n {
        ShapeNode::Shape(s) => s.props.xfrm.as_mut(),
        ShapeNode::Picture(p) => p.props.xfrm.as_mut(),
        ShapeNode::Connector(c) => c.props.xfrm.as_mut(),
        ShapeNode::Group(g) => g.props.xfrm.as_mut(),
        ShapeNode::Frame(f) => Some(&mut f.xfrm),
        ShapeNode::Other(_) => None,
    };
    if let Some(x) = x {
        let t = |v: Option<bool>, on: bool| {
            let r = v.unwrap_or(false) ^ on;
            r.then_some(true)
        };
        x.flip_h = t(x.flip_h, fh);
        x.flip_v = t(x.flip_v, fv);
    }
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
    if let Some((ts, rc)) = text_sel.and_then(|t| Some((t, t.cell?))) {
        // Inside a table cell's text: same as a shape's, on the cell's text body.
        let tbl = table_of(tx.pres, sel.slide, ts.id).ok_or(EditError::Refused("noTable"))?;
        let state = format_state_at(
            tx.pres,
            sel.slide,
            &cell_shape(&tbl, rc),
            Some((ts.start, ts.end)),
        );
        return edit_table(tx, sel, ts.id, |t| {
            let Some(cell) = t.rows.get_mut(rc.0).and_then(|row| row.cells.get_mut(rc.1)) else {
                return false;
            };
            let tb = cell.text.get_or_insert_with(TextBody::default);
            let (mut a, mut b) = (ts.start.min(ts.end), ts.start.max(ts.end));
            if a == b && !f.is_paragraph() {
                (a, b) = text::word_at(&text::body_text(tb), a);
            }
            let default_size = state.size.unwrap_or(18.0);
            text::apply_format_range(tb, f, &state, default_size, a, b);
            true
        });
    }
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
// Adjust handles and connectors
// ---------------------------------------------------------------------------

/// Re-route the attached connectors of every slide the transaction changed
/// (a connector follows the connection sites of the shapes it joins).
fn reroute_connectors(tx: &mut Tx<'_>) -> R<()> {
    // Shapes changed by the transaction, per slide.
    let mut changed: Vec<(usize, Vec<u32>)> = Vec::new();
    for op in &tx.ops {
        if let SlideOp::Nodes { slide, insert, .. } = op {
            let mut ids = Vec::new();
            for n in insert {
                all_ids(n, &mut ids);
            }
            match changed.iter_mut().find(|(s, _)| s == slide) {
                Some((_, v)) => v.extend(ids),
                None => changed.push((*slide, ids)),
            }
        }
    }
    for (slide, ids) in changed {
        let placed = place_slide(tx.pres, slide);
        let mut changes = Vec::new();
        // Connectors at any depth (inside groups too).
        let mut all: Vec<&Placed> = Vec::new();
        connect::walk(&placed, &mut |p| all.push(p));
        for p in all {
            if let Some(ShapeNode::Connector(c)) = ops::node_at(tx.pres, slide, &p.path) {
                let joins_changed = [0, 1]
                    .iter()
                    .any(|&e| connect::cxn_ref(c, e).is_some_and(|(id, _)| ids.contains(&id)));
                if !joins_changed {
                    continue;
                }
                if let Some(n) = connect::reroute(tx.pres, slide, &placed, p, c) {
                    changes.push((p.path.clone(), ShapeNode::Connector(Box::new(n))));
                }
            }
        }
        for (path, n) in changes {
            tx.replace_node(slide, &path, n)?;
        }
    }
    Ok(())
}

fn the_one(tx: &Tx<'_>, sel: &SlideSel) -> R<(Placed, ShapeNode)> {
    let placed = place_slide(tx.pres, sel.slide);
    let [id] = sel.ids.as_slice() else {
        return refuse("noSelection");
    };
    let p = find(&placed, *id)
        .cloned()
        .ok_or(EditError::Refused("noShape"))?;
    let n = node_clone(tx.pres, sel.slide, &p.path)?;
    Ok((p, n))
}

fn cmd_adjust(tx: &mut Tx<'_>, sel: &SlideSel, index: usize, x: f64, y: f64) -> R<()> {
    let (p, n) = the_one(tx, sel)?;
    let new = connect::adjust(tx.pres, sel.slide, &p, &n, index, x, y)
        .ok_or(EditError::Refused("noHandle"))?;
    tx.replace_node(sel.slide, &p.path, new)?;
    Ok(())
}

fn cmd_connector_end(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    end: usize,
    x: f64,
    y: f64,
    tol: f64,
) -> R<()> {
    let (p, n) = the_one(tx, sel)?;
    let ShapeNode::Connector(mut c) = n else {
        return refuse("noConnector");
    };
    if end > 1 {
        return refuse("noConnector");
    }
    let placed = place_slide(tx.pres, sel.slide);
    let x0 = c
        .props
        .xfrm
        .clone()
        .ok_or(EditError::Refused("noTransform"))?;
    let [a, b] = connect::ends(&p, &x0);
    let hit = connect::nearest_site(tx.pres, sel.slide, &placed, x, y, tol, c.nv.id);
    let to = hit.map_or((x, y), |h| h.2);
    connect::set_cxn_ref(&mut c, end, hit.map(|h| (h.0, h.1)));
    let (a, b) = if end == 0 { (to, b) } else { (a, to) };
    let routed = connect::route(&p, &c, a, b);
    tx.replace_node(sel.slide, &p.path, ShapeNode::Connector(Box::new(routed)))?;
    Ok(())
}

fn cmd_insert_connector(tx: &mut Tx<'_>, sel: &mut SlideSel, preset: Option<&str>) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items = selected(&placed, &sel.ids);
    let [p0, p1] = items.as_slice() else {
        return refuse("connectNeedsTwo");
    };
    let node = |p: &Placed| ops::node_at(tx.pres, sel.slide, &p.path);
    let (Some(n0), Some(n1)) = (node(p0), node(p1)) else {
        return refuse("noShape");
    };
    let s0 = connect::sites(tx.pres, sel.slide, p0, n0);
    let s1 = connect::sites(tx.pres, sel.slide, p1, n1);
    let mut best: Option<(usize, usize, f64)> = None;
    for (i, a) in s0.iter().enumerate() {
        for (j, b) in s1.iter().enumerate() {
            let d = (a.0 - b.0).hypot(a.1 - b.1);
            if best.is_none_or(|x| d < x.2) {
                best = Some((i, j, d));
            }
        }
    }
    let Some((i, j, _)) = best else {
        return refuse("noSites");
    };
    let id = next_shape_id(tx.pres, sel.slide);
    let Some(ShapeNode::Connector(mut c)) =
        build::connector(id, preset.unwrap_or("straightConnector1"))
    else {
        return refuse("build");
    };
    connect::set_cxn_ref(&mut c, 0, Some((p0.id, i as u32)));
    connect::set_cxn_ref(&mut c, 1, Some((p1.id, j as u32)));
    let base = c.props.xfrm.clone().unwrap_or_default();
    c.props.xfrm = Some(connect::xfrm_between(&base, s0[i], s1[j]));
    insert_top(tx, sel, Some(ShapeNode::Connector(c)))
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

/// Copy the selected shapes (top-level space, inherited transforms pinned).
fn copy_shapes(pres: &Presentation, sel: &SlideSel, deck: u64) -> R<(clip::Clip, Vec<u32>)> {
    let placed = place_slide(pres, sel.slide);
    let items: Vec<Placed> = selected(&placed, &sel.ids).into_iter().cloned().collect();
    if items.is_empty() {
        return refuse("noSelection");
    }
    let mut nodes = Vec::new();
    let mut boxes = Vec::new();
    let mut top = Vec::new();
    for p in &items {
        let mut n = node_clone(pres, sel.slide, &p.path)?;
        let as_root = Placed {
            frame: geom::Frame::root(),
            ..p.clone()
        };
        apply_box(pres, sel.slide, &mut n, &as_root, &p.b);
        boxes.push(p.b);
        top.push(p.path[0]);
        nodes.push(n);
    }
    let rels = clip::collect_rels(pres, sel.slide, &nodes);
    // Top-level shapes to draw for the clipboard picture.
    top.sort_unstable();
    top.dedup();
    let ids = top
        .iter()
        .filter_map(|&i| {
            pres.slides
                .get(sel.slide)
                .and_then(|s| s.data.shapes.get(i))
                .and_then(ShapeNode::nv)
                .map(|nv| nv.id)
        })
        .collect();
    Ok((
        clip::Clip {
            deck,
            source: None,
            kind: clip::ClipKind::Shapes {
                slide: sel.slide,
                nodes,
                rels,
                bounds: clip::bounds_of(&boxes),
            },
            pastes: 0,
        },
        ids,
    ))
}

/// Paste clipboard shapes onto the current slide.
fn paste_shapes(tx: &mut Tx<'_>, sel: &mut SlideSel, c: &mut clip::Clip, deck: u64) -> R<()> {
    let clip::ClipKind::Shapes {
        slide: from,
        nodes,
        rels,
        ..
    } = &c.kind
    else {
        return refuse("noClip");
    };
    let slide = sel.slide;
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let part_name = s.info.part_name.clone();
    let mut slide_rels = s.info.rels.clone();
    let mut map = Vec::new();
    let mut new_parts = Vec::new();
    for cr in rels {
        let mut rel = cr.rel.clone();
        if !rel.external {
            let same_deck = c.deck == deck;
            let exists = clip::rel_part(&part_name, &rel).is_some_and(|name| {
                tx.pres
                    .parts
                    .iter()
                    .any(|p| p.name.eq_ignore_ascii_case(&name))
            });
            if !(same_deck && exists) {
                // Another deck: bring the part along under a new name.
                let Some(part) = &cr.part else {
                    return refuse("clipMissingPart");
                };
                let ext = part.name.rsplit('.').next().unwrap_or("bin");
                let stem = part
                    .name
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.split('.').next())
                    .unwrap_or("part")
                    .trim_end_matches(|ch: char| ch.is_ascii_digit());
                let dir = part.name.rsplit_once('/').map_or("/ppt/media", |(d, _)| d);
                let mut name = build::unique_part(tx.pres, &format!("{dir}/{stem}"), ext);
                while new_parts
                    .iter()
                    .any(|p: &bo_model::slide::RawPart| p.name == name)
                {
                    name = format!("{name}1");
                }
                let rel_dir = dir.trim_start_matches("/ppt/");
                rel.target = format!("../{rel_dir}/{}", name.rsplit('/').next().unwrap_or(""));
                new_parts.push(bo_model::slide::RawPart {
                    name,
                    content_type: part.content_type.clone(),
                    data: part.data.clone(),
                });
            }
        }
        // Reuse an equal relationship of the target slide, else add one.
        let id = if let Some(r) = slide_rels.iter().find(|r| {
            r.rel_type == rel.rel_type && r.target == rel.target && r.external == rel.external
        }) {
            r.id.clone()
        } else {
            let id = clip::free_rel_id(&slide_rels);
            slide_rels.push(Rel {
                id: id.clone(),
                ..rel.clone()
            });
            id
        };
        map.push((cr.rel.id.clone(), id));
    }
    if !new_parts.is_empty() {
        let at = tx.pres.parts.len();
        tx.op(SlideOp::Parts {
            index: at,
            remove: vec![],
            insert: new_parts,
        })?;
    }
    tx.fields(slide, |f| f.rels = slide_rels)?;
    // Pasted where they came from: offset so the copies are visible.
    let offset = if c.deck == deck && *from == slide {
        c.pastes += 1;
        DUPLICATE_OFFSET * c.pastes as f64
    } else {
        0.0
    };
    let mut next = next_shape_id(tx.pres, slide);
    let mut ids = Vec::new();
    let mut out = Vec::new();
    for n in nodes {
        let mut n = clip::remap_rels(n, &map);
        clip::shift(&mut n, offset, offset);
        renumber(&mut n, &mut next);
        if let Some(nv) = n.nv() {
            ids.push(nv.id);
        }
        out.push(n);
    }
    let len = tx
        .pres
        .slides
        .get(slide)
        .ok_or(OpError::NoSlide(slide))?
        .data
        .shapes
        .len();
    tx.insert_nodes(slide, &[], len, out)?;
    sel.ids = ids;
    Ok(())
}

fn slide_indices(pres: &Presentation, sel: &SlideSel, indices: Option<Vec<usize>>) -> Vec<usize> {
    let mut v = indices.unwrap_or_else(|| vec![sel.slide]);
    v.retain(|&i| i < pres.slides.len());
    v.sort_unstable();
    v.dedup();
    v
}

/// Paste clipboard slides after the current slide (same deck only: their
/// layouts and media belong to it).
fn paste_slides(tx: &mut Tx<'_>, sel: &mut SlideSel, c: &clip::Clip, deck: u64) -> R<()> {
    let clip::ClipKind::Slides(slides) = &c.kind else {
        return refuse("noClip");
    };
    if c.deck != deck {
        let src = c
            .source
            .as_deref()
            .ok_or(EditError::Refused("clipOtherDeck"))?;
        return paste_slides_from(tx, sel, slides, src);
    }
    let first = if tx.pres.slides.is_empty() {
        0
    } else {
        sel.slide + 1
    };
    for (at, s) in (first..).zip(slides) {
        if s.layout >= tx.pres.layouts.len() {
            return refuse("noLayout");
        }
        let copy = build::duplicate_slide(tx.pres, s);
        tx.op(SlideOp::Slides {
            index: at,
            remove: vec![],
            insert: vec![copy],
        })?;
    }
    *sel = SlideSel {
        slide: first,
        ids: vec![],
    };
    Ok(())
}

/// Placeholder types of shapes (§19.3.1.36 ph@type; absent = `obj`), `body` and
/// `obj` counted as one.
fn ph_types(nodes: &[ShapeNode]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    bo_pptx::visit(nodes, &mut |n| {
        let nv = match n {
            ShapeNode::Shape(s) => Some(&s.nv),
            ShapeNode::Picture(p) => Some(&p.nv),
            ShapeNode::Frame(f) => Some(&f.nv),
            _ => None,
        };
        if let Some(ph) = nv.and_then(|nv| nv.placeholder.as_ref()) {
            let k = match ph.kind.as_deref() {
                None | Some("obj" | "body") => "body",
                Some("ctrTitle" | "title") => "title",
                Some("subTitle") => "subTitle",
                Some(o) => o,
            };
            if !v.iter().any(|x| x == k) {
                v.push(k.to_owned());
            }
        }
    });
    v
}

/// Layout of the target deck for a slide pasted from another deck (§19.3.1.39):
/// the layout with the same name, else the one whose placeholder types cover the
/// slide's best; `None` when none covers them (the source layout is imported).
fn match_layout(
    pres: &Presentation,
    src_layout: &bo_model::slide::SlideLayout,
    slide: &bo_model::slide::Slide,
) -> Option<usize> {
    if let Some(name) = src_layout.data.name.as_deref().filter(|n| !n.is_empty()) {
        if let Some(i) = pres.layouts.iter().position(|l| {
            l.data
                .name
                .as_deref()
                .is_some_and(|m| m.eq_ignore_ascii_case(name))
        }) {
            return Some(i);
        }
    }
    let need = ph_types(&slide.data.shapes);
    let want = ph_types(&src_layout.data.shapes);
    let mut best: Option<(usize, i64)> = None;
    for (i, l) in pres.layouts.iter().enumerate() {
        let have = ph_types(&l.data.shapes);
        if !need.iter().all(|t| have.contains(t)) {
            continue;
        }
        // Same kind first, then the most shared placeholder types, the fewest extra.
        let same_kind = i64::from(l.kind.is_some() && l.kind == src_layout.kind);
        let shared = want.iter().filter(|t| have.contains(t)).count() as i64;
        let extra = have.iter().filter(|t| !want.contains(t)).count() as i64;
        let score = same_kind * 1000 + shared * 10 - extra;
        if best.is_none_or(|b| score > b.1) {
            best = Some((i, score));
        }
    }
    best.map(|b| b.0)
}

/// Copies parts targeted by the relationships of imported parts under new names.
struct PartCarrier<'a> {
    src: &'a clip::SlideSource,
    /// Source part name → new part name.
    map: Vec<(String, String)>,
    taken: Vec<String>,
    new_parts: Vec<bo_model::slide::RawPart>,
}

impl PartCarrier<'_> {
    fn unique(&mut self, pres: &Presentation, name: &str) -> String {
        let (dir, file) = name.rsplit_once('/').unwrap_or(("/ppt/media", name));
        let (stem, ext) = file.rsplit_once('.').unwrap_or((file, "bin"));
        let stem = stem.trim_end_matches(|c: char| c.is_ascii_digit());
        let n = build::unique_part_except(pres, &format!("{dir}/{stem}"), ext, &self.taken);
        self.taken.push(n.clone());
        n
    }

    /// Copy source part `old` under a new name (once), with the parts its own
    /// relationships target — a chart's embedded workbook, colour and style
    /// parts — and a `.rels` part pointing at the copies (ECMA-376 Part 2, §9.3).
    fn copy_part(&mut self, pres: &Presentation, old: &str) -> Option<String> {
        if let Some((_, n)) = self.map.iter().find(|(o, _)| o.eq_ignore_ascii_case(old)) {
            return Some(n.clone());
        }
        let p = self
            .src
            .parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(old))
            .cloned()?;
        let n = self.unique(pres, &p.name);
        self.map.push((old.to_owned(), n.clone()));
        self.new_parts.push(bo_model::slide::RawPart {
            name: n.clone(),
            ..p
        });
        let rels_name = bo_opc::rels_part_name(old);
        if let Some(rp) = self
            .src
            .parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&rels_name))
            .cloned()
        {
            let mut out = Vec::new();
            for (id, t, target, ext) in clip::rels_of(&rp.data) {
                if ext {
                    out.push((id, t, target, ext));
                    continue;
                }
                let child = bo_opc::resolve_target(old, &target);
                if let Some(cn) = self.copy_part(pres, &child) {
                    out.push((id, t, clip::relative_target(&n, &cn), false));
                }
            }
            self.new_parts.push(bo_model::slide::RawPart {
                name: bo_opc::rels_part_name(&n),
                content_type: rp.content_type.clone(),
                data: clip::rels_xml(&out),
            });
        }
        Some(n)
    }

    /// Relationships of a part moved from `old_owner` to `new_owner`: structural ones
    /// are kept (the writer re-targets them), the others get a copy of their part;
    /// relationships to parts the source does not have are dropped.
    fn rels(
        &mut self,
        pres: &Presentation,
        old_owner: &str,
        new_owner: &str,
        rels: &[Rel],
    ) -> Vec<Rel> {
        let mut out = Vec::new();
        for r in rels {
            if r.external || clip::is_structural(&r.rel_type) {
                out.push(r.clone());
                continue;
            }
            let Some(old) = clip::rel_part(old_owner, r) else {
                continue;
            };
            let Some(new) = self.copy_part(pres, &old) else {
                continue;
            };
            out.push(Rel {
                target: clip::relative_target(new_owner, &new),
                ..r.clone()
            });
        }
        out
    }
}

/// Fitting slides of one size into a deck of another (document approach, as
/// when the slide size changes, §19.2.1.39 sldSz): positions and sizes scale
/// uniformly by the smaller ratio and the content is centred; text sizes,
/// line widths and child coordinates of groups are kept.
struct SizeFit {
    scale: f64,
    dx: f64,
    dy: f64,
}

impl SizeFit {
    fn new(from: &bo_model::slide::SlideSize, to: &bo_model::slide::SlideSize) -> SizeFit {
        if from.cx <= 0 || from.cy <= 0 || (from.cx == to.cx && from.cy == to.cy) {
            return SizeFit {
                scale: 1.0,
                dx: 0.0,
                dy: 0.0,
            };
        }
        let (fx, fy) = (from.cx as f64, from.cy as f64);
        let (tx, ty) = (to.cx as f64, to.cy as f64);
        let scale = (tx / fx).min(ty / fy);
        SizeFit {
            scale,
            dx: (tx - fx * scale) / 2.0,
            dy: (ty - fy * scale) / 2.0,
        }
    }

    /// Scale the transforms of top-level shapes (§20.1.7.6 xfrm off / ext).
    fn apply(&self, nodes: &mut [ShapeNode]) {
        if (self.scale - 1.0).abs() < 1e-12 && self.dx == 0.0 && self.dy == 0.0 {
            return;
        }
        let r = |v: f64| v.round() as i64;
        for n in nodes {
            let x = match n {
                ShapeNode::Shape(s) => s.props.xfrm.as_mut(),
                ShapeNode::Picture(p) => p.props.xfrm.as_mut(),
                ShapeNode::Connector(c) => c.props.xfrm.as_mut(),
                ShapeNode::Group(g) => g.props.xfrm.as_mut(),
                ShapeNode::Frame(f) => Some(&mut f.xfrm),
                ShapeNode::Other(_) => None,
            };
            let Some(x) = x else { continue };
            if let Some((ox, oy)) = x.off {
                x.off = Some((
                    r(ox as f64 * self.scale + self.dx),
                    r(oy as f64 * self.scale + self.dy),
                ));
            }
            if let Some((cx, cy)) = x.ext {
                x.ext = Some((r(cx as f64 * self.scale), r(cy as f64 * self.scale)));
            }
        }
    }
}

/// Paste slides copied from another deck: each slide gets the target deck's
/// layout of the same name, else the closest by placeholder types, else its own
/// layout imported with its master and theme (§19.3.1.39, §19.3.1.42, §20.1.6.9);
/// pictures and media come along; relationships stay valid.
fn paste_slides_from(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    slides: &[bo_model::slide::Slide],
    src: &clip::SlideSource,
) -> R<()> {
    let before = ops::PresFields::of(tx.pres);
    let mut after = before.clone();
    let mut carrier = PartCarrier {
        src,
        map: Vec::new(),
        taken: Vec::new(),
        new_parts: Vec::new(),
    };
    // Source index → target index of imported layouts / masters / themes.
    let mut layouts: Vec<(usize, usize)> = Vec::new();
    let mut masters: Vec<(usize, usize)> = Vec::new();
    let mut themes: Vec<(usize, usize)> = Vec::new();
    let notes_master = build::notes_master(tx.pres);
    // New part names of the pasted slides (slide → slide hyperlinks among them).
    let slide_names: Vec<(String, String)> = slides
        .iter()
        .map(|s| {
            (
                s.info.part_name.clone(),
                carrier.unique(tx.pres, "/ppt/slides/slide1.xml"),
            )
        })
        .collect();
    let fit = SizeFit::new(&src.slide_size, &tx.pres.slide_size);
    let mut out_slides = Vec::new();
    for (k, s) in slides.iter().enumerate() {
        let sl = src
            .layouts
            .get(s.layout)
            .ok_or(EditError::Refused("noLayout"))?;
        let layout = if let Some(&(_, t)) = layouts.iter().find(|(o, _)| *o == s.layout) {
            t
        } else if let Some(t) = match_layout(tx.pres, sl, s) {
            t
        } else {
            // Import the layout, its master and the master's theme.
            let sm = src
                .masters
                .get(sl.master)
                .ok_or(EditError::Refused("noMaster"))?;
            let master = if let Some(&(_, t)) = masters.iter().find(|(o, _)| *o == sl.master) {
                t
            } else {
                let theme = if let Some(&(_, t)) = themes.iter().find(|(o, _)| *o == sm.theme) {
                    t
                } else {
                    let st = src
                        .themes
                        .get(sm.theme)
                        .ok_or(EditError::Refused("noTheme"))?;
                    let mut t = st.clone();
                    let name = carrier.unique(tx.pres, "/ppt/theme/theme1.xml");
                    t.info.rels = carrier.rels(tx.pres, &st.info.part_name, &name, &st.info.rels);
                    t.info.part_name = name;
                    after.themes.push(t);
                    themes.push((sm.theme, after.themes.len() - 1));
                    after.themes.len() - 1
                };
                let mut m = sm.clone();
                fit.apply(&mut m.data.shapes);
                let name = carrier.unique(tx.pres, "/ppt/slideMasters/slideMaster1.xml");
                m.info.rels = carrier.rels(tx.pres, &sm.info.part_name, &name, &sm.info.rels);
                m.info.part_name = name;
                m.theme = theme;
                // The writer lists the layouts pointing to it (§19.3.1.41 sldLayoutId).
                m.layouts = Vec::new();
                m.master_id = 0;
                m.pres_rel_id = String::new();
                after.masters.push(m);
                masters.push((sl.master, after.masters.len() - 1));
                after.masters.len() - 1
            };
            let mut l = sl.clone();
            fit.apply(&mut l.data.shapes);
            let name = carrier.unique(tx.pres, "/ppt/slideLayouts/slideLayout1.xml");
            l.info.rels = carrier.rels(tx.pres, &sl.info.part_name, &name, &sl.info.rels);
            l.info.part_name = name;
            l.master = master;
            after.layouts.push(l);
            after.layouts.len() - 1
        };
        layouts.push((s.layout, layout));
        let mut d = s.clone();
        let name = slide_names[k].1.clone();
        d.info.rels = carrier.rels(tx.pres, &s.info.part_name, &name, &s.info.rels);
        // Slide → slide hyperlinks (§21.1.2.3.5 hlinkClick, `ppaction://hlinksldjump`):
        // to a slide pasted along → its copy; to any other → the link is removed.
        let mut dropped: Vec<String> = Vec::new();
        d.info.rels.retain_mut(|r| {
            if r.external || !r.rel_type.ends_with("/slide") {
                return true;
            }
            let old = bo_opc::resolve_target(&s.info.part_name, &r.target);
            if let Some((_, n)) = slide_names
                .iter()
                .find(|(o, _)| !o.is_empty() && o.eq_ignore_ascii_case(&old))
            {
                r.target = clip::relative_target(&name, n);
                true
            } else {
                dropped.push(r.id.clone());
                false
            }
        });
        if !dropped.is_empty() {
            d.data.shapes = d
                .data
                .shapes
                .iter()
                .map(|n| clip::strip_rel_refs(n, &dropped))
                .collect();
        }
        fit.apply(&mut d.data.shapes);
        d.info.part_name = name;
        d.layout = layout;
        d.slide_id = 0;
        d.pres_rel_id = String::new();
        // Notes: kept when the deck has a notes master (§19.3.1.28), linked to it.
        d.notes = match (&d.notes, &notes_master) {
            (Some(n), Some(nm)) => {
                let mut n = n.clone();
                let nn = carrier.unique(tx.pres, "/ppt/notesSlides/notesSlide1.xml");
                n.info.rels = n
                    .info
                    .rels
                    .iter()
                    .filter(|r| clip::is_structural(&r.rel_type))
                    .map(|r| {
                        if r.rel_type.ends_with("/notesMaster") {
                            Rel {
                                target: clip::relative_target(&nn, nm),
                                ..r.clone()
                            }
                        } else {
                            r.clone()
                        }
                    })
                    .collect();
                n.info.part_name = nn;
                Some(n)
            }
            _ => None,
        };
        out_slides.push(d);
    }
    if !carrier.new_parts.is_empty() {
        let at = tx.pres.parts.len();
        tx.op(SlideOp::Parts {
            index: at,
            remove: vec![],
            insert: std::mem::take(&mut carrier.new_parts),
        })?;
    }
    if after != before {
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
    }
    let first = if tx.pres.slides.is_empty() {
        0
    } else {
        sel.slide + 1
    };
    tx.op(SlideOp::Slides {
        index: first,
        remove: vec![],
        insert: out_slides,
    })?;
    *sel = SlideSel {
        slide: first,
        ids: vec![],
    };
    Ok(())
}

fn slides_text(pres: &Presentation, indices: &[usize]) -> String {
    indices
        .iter()
        .filter_map(|&i| pres.slides.get(i))
        .map(|s| clip::plain_text(&s.data.shapes))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
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
        if let SlideCommand::SetSavePassword { password } = &cmd {
            self.set_save_password(password.as_deref());
            return self.slide_command(SlideCommand::State);
        }
        // New part names of a collaborating replica never clash with another's.
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            let fonts = &mut self.fonts;
            let clipboard = &mut self.slide_clip;
            let deck = self.deck_seq;
            ed.clamp(pres);
            if !cmd.keeps_text_selection() {
                ed.text = None;
            }
            if !cmd.keeps_table_selection() {
                ed.table = None;
            }
            match cmd {
                SlideCommand::State | SlideCommand::SetSavePassword { .. } => {}
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
                SlideCommand::SelectRect { rect, extend } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let inside: Vec<u32> = placed
                        .iter()
                        .filter(|p| {
                            let b = p.b.bounds();
                            b.x >= rect.x - 0.01
                                && b.y >= rect.y - 0.01
                                && b.right() <= rect.right() + 0.01
                                && b.bottom() <= rect.bottom() + 0.01
                        })
                        .map(|p| p.id)
                        .collect();
                    if extend {
                        for id in inside {
                            if !ed.sel.ids.contains(&id) {
                                ed.sel.ids.push(id);
                            }
                        }
                    } else {
                        ed.sel.ids = inside;
                    }
                }
                SlideCommand::SelectNext { back } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let n = placed.len();
                    if n > 0 {
                        let cur = ed
                            .sel
                            .ids
                            .last()
                            .and_then(|id| placed.iter().position(|p| p.id == *id));
                        let i = match (cur, back) {
                            (None, false) => 0,
                            (None, true) => n - 1,
                            (Some(i), false) => (i + 1) % n,
                            (Some(i), true) => (i + n - 1) % n,
                        };
                        ed.sel.ids = vec![placed[i].id];
                    }
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
                        // The built-in style goes into the deck's table styles part.
                        if let Some(part) = build::table_styles_with(tx.pres, 0) {
                            ensure_part(tx, part)?;
                        }
                        let style = bo_pptx::table_style::BUILTIN_IDS[0];
                        let n = build::table(id, rows, cols, r.x, r.y, r.w, style);
                        insert_top(tx, sel, n)
                    });
                }
                SlideCommand::SetText {
                    id,
                    text,
                    merge,
                    sel: text_sel,
                    cell: Some([r, c]),
                } => {
                    // Text of a table cell edited on the canvas (formatting kept).
                    let target = id.or(ed.text.map(|t| t.id)).or(single(&ed.sel.ids));
                    match target {
                        Some(tid) => {
                            let key = merge.then_some((ed.sel.slide, tid, 4));
                            ed.mutate_as(pres, &mut out, key, |tx, sel| {
                                edit_table(tx, sel, tid, |t| {
                                    let Some(cell) =
                                        t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c))
                                    else {
                                        return false;
                                    };
                                    let tb = cell.text.get_or_insert_with(TextBody::default);
                                    text::set_text(tb, &text);
                                    true
                                })
                            });
                            if out.error.is_none() {
                                let plain = text.replace("\r\n", "\n").replace('\r', "\n");
                                let [a, b] = text_sel.unwrap_or([0, 0]);
                                ed.text = Some(rich::TextSel {
                                    id: tid,
                                    start: text::char_offset(&plain, a),
                                    end: text::char_offset(&plain, b),
                                    cell: Some((r, c)),
                                });
                                ed.table = Some(table::TableSel {
                                    id: tid,
                                    r0: r,
                                    c0: c,
                                    r1: r,
                                    c1: c,
                                });
                            }
                        }
                        None => out.error = Some("noTable".into()),
                    }
                }
                SlideCommand::SetText {
                    id,
                    text,
                    merge,
                    sel: text_sel,
                    cell: None,
                } => {
                    let target = id.or(single(&ed.sel.ids));

                    let typing = if merge {
                        target.map(|t| (ed.sel.slide, t, 0))
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
                            cell: None,
                        });
                    }
                }
                SlideCommand::FormatText { format, range } => {
                    let ts = match (range, ed.text) {
                        (Some([a, b]), cur) => {
                            let id = cur.map(|t| t.id).or(single(&ed.sel.ids));

                            id.and_then(|id| {
                                let cell = cur.filter(|t| t.id == id).and_then(|t| t.cell);
                                let probe = rich::TextSel {
                                    id,
                                    start: 0,
                                    end: 0,
                                    cell,
                                };
                                let s = edit_text(pres, ed.sel.slide, probe)?;
                                Some(rich::TextSel {
                                    start: text::char_offset(&s, a),
                                    end: text::char_offset(&s, b),
                                    ..probe
                                })
                            })
                        }
                        (None, cur) => cur,
                    };
                    ed.mutate(pres, &mut out, |tx, sel| cmd_format(tx, sel, &format, ts));
                }
                SlideCommand::TextSelect {
                    id,
                    start,
                    end,
                    cell,
                } => {
                    let id = id.or(single(&ed.sel.ids));
                    let cell = cell.map(|[r, c]| (r, c));
                    let probe = id.map(|id| rich::TextSel {
                        id,
                        start: 0,
                        end: 0,
                        cell,
                    });
                    match probe.and_then(|t| Some((t, edit_text(pres, ed.sel.slide, t)?))) {
                        Some((t, s)) => {
                            if !ed.sel.ids.contains(&t.id) {
                                ed.sel.ids = vec![t.id];
                            }
                            ed.text = Some(rich::TextSel {
                                start: text::char_offset(&s, start),
                                end: text::char_offset(&s, end),
                                ..t
                            });
                            if let Some((r, c)) = cell {
                                ed.table = Some(table::TableSel {
                                    id: t.id,
                                    r0: r,
                                    c0: c,
                                    r1: r,
                                    c1: c,
                                });
                            }
                        }
                        None => out.error = Some("noText".into()),
                    }
                }
                SlideCommand::CellTextHit { x, y } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let id = ed.table.map(|t| t.id).or(single(&ed.sel.ids));
                    let hit = id.and_then(|id| {
                        let p = find(&placed, id)?;
                        let t = table_of(pres, ed.sel.slide, id)?;
                        let (lx, ly) = p.b.to_local(x, y);
                        let (r, c) = table::cell_at(&t, lx + p.b.w / 2.0, ly + p.b.h / 2.0)?;
                        let (r, c) = table::anchor_of(&t, r, c);
                        let (g, b) = cell_geometry(pres, ed.sel.slide, &p.b, &t, (r, c), fonts)?;
                        Some((id, r, c, rich::offset_at(&g, &b, x, y)))
                    });
                    match hit {
                        Some((id, r, c, off)) => {
                            ed.sel.ids = vec![id];
                            ed.table = Some(table::TableSel {
                                id,
                                r0: r,
                                c0: c,
                                r1: r,
                                c1: c,
                            });
                            ed.text = Some(rich::TextSel {
                                id,
                                start: off,
                                end: off,
                                cell: Some((r, c)),
                            });
                        }
                        None => out.error = Some("noCell".into()),
                    }
                }
                SlideCommand::CellNext { back } => {
                    let cur = ed.text.filter(|t| t.cell.is_some());
                    let next = cur.and_then(|t| {
                        let tbl = table_of(pres, ed.sel.slide, t.id)?;
                        let mut order: Vec<(usize, usize)> = table::anchors(&tbl)
                            .into_iter()
                            .map(|(r, c, _, _)| (r, c))
                            .collect();
                        order.sort_unstable();
                        let i = order.iter().position(|&rc| Some(rc) == t.cell)?;
                        let n = order.len();
                        let j = if back { (i + n - 1) % n } else { (i + 1) % n };
                        let rc = order[j];
                        let len = cell_text(pres, ed.sel.slide, t.id, rc)?.chars().count();
                        Some((t.id, rc, len))
                    });
                    match next {
                        Some((id, (r, c), len)) => {
                            ed.table = Some(table::TableSel {
                                id,
                                r0: r,
                                c0: c,
                                r1: r,
                                c1: c,
                            });
                            ed.text = Some(rich::TextSel {
                                id,
                                start: 0,
                                end: len,
                                cell: Some((r, c)),
                            });
                        }
                        None => out.error = Some("noCell".into()),
                    }
                }
                SlideCommand::TextHit { x, y, extend } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let hit = ed.text.and_then(|t| {
                        let p = find(&placed, t.id)?;
                        if let Some(rc) = t.cell {
                            let tbl = table_of(pres, ed.sel.slide, t.id)?;
                            let (g, b) = cell_geometry(pres, ed.sel.slide, &p.b, &tbl, rc, fonts)?;
                            return Some((t, rich::offset_at(&g, &b, x, y)));
                        }
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
                                cell: t.cell,
                            });
                        }
                        None => out.error = Some("noText".into()),
                    }
                }
                SlideCommand::TextExit => ed.text = None,
                SlideCommand::TableExit => ed.table = None,
                SlideCommand::TableSelect { id, r0, c0, r1, c1 } => {
                    let id = id.or(single(&ed.sel.ids)).or(ed.table.map(|t| t.id));
                    match id.and_then(|id| Some((id, table_of(pres, ed.sel.slide, id)?))) {
                        Some((id, t)) => {
                            let (rows, cols) = table::dims(&t);
                            if r0 >= rows || c0 >= cols {
                                out.error = Some("noCell".into());
                            } else {
                                ed.sel.ids = vec![id];
                                let s = table::TableSel {
                                    id,
                                    r0,
                                    c0,
                                    r1: r1.unwrap_or(r0).min(rows - 1),
                                    c1: c1.unwrap_or(c0).min(cols - 1),
                                };
                                ed.table = Some(table::expand(&t, s));
                            }
                        }
                        None => out.error = Some("noTable".into()),
                    }
                }
                SlideCommand::TableHit { x, y, extend } => {
                    let placed = place_slide(pres, ed.sel.slide);
                    let id = ed.table.map(|t| t.id).or(single(&ed.sel.ids));
                    let hit = id.and_then(|id| {
                        let p = find(&placed, id)?;
                        let t = table_of(pres, ed.sel.slide, id)?;
                        let (lx, ly) = p.b.to_local(x, y);
                        let (r, c) = table::cell_at(&t, lx + p.b.w / 2.0, ly + p.b.h / 2.0)?;
                        Some((id, t, r, c))
                    });
                    match hit {
                        Some((id, t, r, c)) => {
                            let s = match ed.table {
                                Some(cur) if extend && cur.id == id => table::TableSel {
                                    r1: r,
                                    c1: c,
                                    ..cur
                                },
                                _ => table::TableSel {
                                    id,
                                    r0: r,
                                    c0: c,
                                    r1: r,
                                    c1: c,
                                },
                            };
                            ed.sel.ids = vec![id];
                            ed.table = Some(table::expand(&t, s));
                        }
                        None => out.error = Some("noCell".into()),
                    }
                }
                SlideCommand::SetCellText {
                    row,
                    col,
                    ref text,
                    merge,
                } => match ed.table {
                    Some(ts) => {
                        let (r, c) = (row.unwrap_or(ts.r0), col.unwrap_or(ts.c0));
                        let key = merge.then_some((ed.sel.slide, ts.id, 3));
                        ed.mutate_as(pres, &mut out, key, |tx, sel| {
                            edit_table(tx, sel, ts.id, |t| table::set_cell_text(t, r, c, text))
                        });
                    }
                    None => out.error = Some("noTable".into()),
                },
                SlideCommand::TableInsertRow { .. }
                | SlideCommand::TableInsertCol { .. }
                | SlideCommand::TableDeleteRows
                | SlideCommand::TableDeleteCols
                | SlideCommand::TableMerge
                | SlideCommand::TableSplit
                | SlideCommand::TableFill { .. }
                | SlideCommand::TableOptions { .. } => match ed.table.map(table::TableSel::norm) {
                    Some(ts) => {
                        let fill = match &cmd {
                            SlideCommand::TableFill { color } => {
                                match color.as_deref().map(|h| fill_of(Some(h))) {
                                    Some(Ok(f)) => Some(Some(f)),
                                    Some(Err(_)) => None,
                                    None => Some(None),
                                }
                            }
                            _ => Some(None),
                        };
                        let mut next = ts;
                        ed.mutate(pres, &mut out, |tx, sel| {
                            edit_table(tx, sel, ts.id, |t| match &cmd {
                                SlideCommand::TableInsertRow { below } => {
                                    let at = if *below { ts.r1 + 1 } else { ts.r0 };
                                    table::insert_row(t, at);
                                    next = table::TableSel {
                                        r0: at,
                                        r1: at,
                                        ..ts
                                    };
                                    true
                                }
                                SlideCommand::TableInsertCol { right } => {
                                    let at = if *right { ts.c1 + 1 } else { ts.c0 };
                                    table::insert_col(t, at);
                                    next = table::TableSel {
                                        c0: at,
                                        c1: at,
                                        ..ts
                                    };
                                    true
                                }
                                SlideCommand::TableDeleteRows => {
                                    let ok = table::delete_rows(t, ts.r0, ts.r1);
                                    let r = ts.r0.min(t.rows.len().saturating_sub(1));
                                    next = table::TableSel { r0: r, r1: r, ..ts };
                                    ok
                                }
                                SlideCommand::TableDeleteCols => {
                                    let ok = table::delete_cols(t, ts.c0, ts.c1);
                                    let c = ts.c0.min(t.grid.len().saturating_sub(1));
                                    next = table::TableSel { c0: c, c1: c, ..ts };
                                    ok
                                }
                                SlideCommand::TableMerge => table::merge(t, ts),
                                SlideCommand::TableSplit => table::split(t, ts),
                                SlideCommand::TableFill { .. } => match &fill {
                                    Some(f) => {
                                        table::set_fill(t, ts, f.as_ref());
                                        true
                                    }
                                    None => false,
                                },
                                SlideCommand::TableOptions {
                                    first_row,
                                    band_row,
                                } => {
                                    if let Some(v) = first_row {
                                        table::set_flag(t, "firstRow", *v);
                                    }
                                    if let Some(v) = band_row {
                                        table::set_flag(t, "bandRow", *v);
                                    }
                                    true
                                }
                                _ => false,
                            })
                        });
                        if out.error.is_none() {
                            ed.table = Some(next);
                            if let Some(t) = table_of(pres, ed.sel.slide, ts.id) {
                                let (rows, cols) = table::dims(&t);
                                let n = next.norm();
                                ed.table = Some(table::expand(
                                    &t,
                                    table::TableSel {
                                        r0: n.r0.min(rows - 1),
                                        r1: n.r1.min(rows - 1),
                                        c0: n.c0.min(cols - 1),
                                        c1: n.c1.min(cols - 1),
                                        ..n
                                    },
                                ));
                            }
                        }
                    }
                    None => out.error = Some("noTable".into()),
                },
                SlideCommand::Adjust { index, x, y, merge } => {
                    let key = single(&ed.sel.ids)
                        .filter(|_| merge)
                        .map(|id| (ed.sel.slide, id, 1));
                    ed.mutate_as(pres, &mut out, key, |tx, sel| {
                        cmd_adjust(tx, sel, index, x, y)
                    });
                }
                SlideCommand::ConnectorEnd {
                    end,
                    x,
                    y,
                    merge,
                    tolerance,
                } => {
                    let key = single(&ed.sel.ids)
                        .filter(|_| merge)
                        .map(|id| (ed.sel.slide, id, 2));
                    ed.mutate_as(pres, &mut out, key, |tx, sel| {
                        cmd_connector_end(tx, sel, end, x, y, tolerance)
                    });
                }
                SlideCommand::InsertConnector { ref preset } => {
                    let preset = preset.clone();
                    ed.mutate(pres, &mut out, |tx, sel| {
                        cmd_insert_connector(tx, sel, preset.as_deref())
                    });
                }
                SlideCommand::Copy | SlideCommand::Cut => match copy_shapes(pres, &ed.sel, deck) {
                    Ok((c, ids)) => {
                        if let clip::ClipKind::Shapes { nodes, bounds, .. } = &c.kind {
                            out.copied =
                                Some((ed.sel.slide, ids, *bounds, clip::plain_text(nodes)));
                        }
                        *clipboard = Some(c);
                        if matches!(cmd, SlideCommand::Cut) {
                            ed.mutate(pres, &mut out, cmd_delete);
                        }
                    }
                    Err(e) => out.error = Some(e.to_string()),
                },
                SlideCommand::Paste => match clipboard.as_mut() {
                    Some(c) if matches!(c.kind, clip::ClipKind::Slides(_)) => {
                        let c = c.clone();
                        ed.mutate(pres, &mut out, |tx, sel| paste_slides(tx, sel, &c, deck));
                    }
                    Some(c) => {
                        let mut work = c.clone();
                        ed.mutate(pres, &mut out, |tx, sel| {
                            paste_shapes(tx, sel, &mut work, deck)
                        });
                        if out.error.is_none() {
                            *c = work;
                        }
                    }
                    None => out.error = Some("noClip".into()),
                },
                SlideCommand::CopySlides { ref indices }
                | SlideCommand::CutSlides { ref indices } => {
                    let v = slide_indices(pres, &ed.sel, indices.clone());
                    if v.is_empty() {
                        out.error = Some("noSlide".into());
                    } else {
                        out.copied_text = Some(slides_text(pres, &v));
                        *clipboard = Some(clip::Clip {
                            deck,
                            kind: clip::ClipKind::Slides(
                                v.iter().map(|&i| pres.slides[i].clone()).collect(),
                            ),
                            pastes: 0,
                            source: Some(Box::new(clip::slide_source(pres, &v))),
                        });
                        if matches!(cmd, SlideCommand::CutSlides { .. }) {
                            ed.mutate(pres, &mut out, |tx, sel| {
                                for &i in v.iter().rev() {
                                    if tx.pres.slides.len() <= 1 {
                                        return refuse("lastSlide");
                                    }
                                    cmd_delete_slide(tx, sel, Some(i))?;
                                }
                                Ok(())
                            });
                        }
                    }
                }
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
                SlideCommand::SetTransition {
                    slides,
                    all,
                    transition,
                } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_set_transition(tx, sel, slides, all, &transition)
                    });
                }
                SlideCommand::AddAnimation { spec } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_add_animation(tx, sel, &spec)
                    });
                }
                SlideCommand::SetAnimation { index, spec } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_set_animation(tx, sel, index, &spec)
                    });
                }
                SlideCommand::RemoveAnimation { index } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_remove_animation(tx, sel, index)
                    });
                }
                SlideCommand::MoveAnimation { from, to } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_move_animation(tx, sel, from, to)
                    });
                }
                SlideCommand::CopyAnimations { from } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        show::cmd_copy_animations(tx, sel, from)
                    });
                }
                SlideCommand::Show { slide } => {
                    if slide < pres.slides.len() {
                        out.show = Some(slide);
                    } else {
                        out.error = Some("noSlide".into());
                    }
                }
                SlideCommand::InsertChart {
                    kind,
                    title,
                    data,
                    legend,
                    rect,
                    template,
                    texts,
                } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let spec = chart::NewChart {
                            kind: &kind,
                            title,
                            data,
                            legend,
                            rect,
                            template,
                            texts,
                        };
                        chart::cmd_insert_chart(tx, sel, spec)
                    });
                }
                SlideCommand::ChartGallery { id, width, height } => {
                    let id = id.or(match ed.sel.ids.as_slice() {
                        [one] => Some(*one),
                        _ => None,
                    });
                    match id.and_then(|id| {
                        chart::chart_gallery(
                            pres,
                            ed.sel.slide,
                            id,
                            width.unwrap_or(96.0),
                            height.unwrap_or(64.0),
                        )
                    }) {
                        Some(g) => out.chart_gallery = Some(g),
                        None => out.error = Some("noChart".into()),
                    }
                }
                SlideCommand::AddComment {
                    text,
                    author,
                    date,
                    parent,
                } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        comments::cmd_add(
                            tx,
                            sel.slide,
                            &text,
                            &author,
                            date.as_deref(),
                            parent.as_deref(),
                        )
                    });
                }
                SlideCommand::ResolveComment { id, done } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        comments::cmd_resolve(tx, sel.slide, &id, done)
                    });
                }
                SlideCommand::DeleteComment { id } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        comments::cmd_delete(tx, sel.slide, &id)
                    });
                }
                SlideCommand::InsertSmartArt {
                    layout,
                    items,
                    colors,
                    style,
                    rect,
                } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        smartart::cmd_insert_smartart(
                            tx,
                            sel,
                            &layout,
                            &items,
                            (colors.as_deref(), style.as_deref()),
                            rect,
                        )
                    });
                }
                SlideCommand::SetSmartArt {
                    id,
                    layout,
                    items,
                    colors,
                    style,
                } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        smartart::cmd_set_smartart(
                            tx,
                            sel,
                            id,
                            layout.as_deref(),
                            items.as_deref(),
                            (colors.as_deref(), style.as_deref()),
                        )
                    });
                }
                SlideCommand::SmartArtToShapes { id } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        smartart::cmd_smartart_to_shapes(tx, sel, id)
                    });
                }
                SlideCommand::SetChart { id, spec } => {
                    ed.mutate(pres, &mut out, |tx, sel| {
                        chart::cmd_set_chart(tx, sel, id, &spec)
                    });
                }
                SlideCommand::Undo => ed.undo(pres, &mut out),
                SlideCommand::Redo => ed.redo(pres, &mut out),
            }
            ed.clamp(pres);
        }
        let copied = out.copied.take();
        let copied_text = out.copied_text.take();
        let show_slide = out.show.take();
        let mut r = self.finish_slide_command(out)?;
        if let Some(i) = show_slide {
            r.show = self.slide_show(i);
        }
        r.clipboard = match (copied, copied_text) {
            (Some((slide, ids, bounds, text)), _) => Some(ClipboardOut {
                text,
                png: self.selection_png(slide, ids, bounds),
            }),
            (None, Some(text)) => Some(ClipboardOut { text, png: None }),
            _ => None,
        };
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// PNG (base64) of some top-level shapes of a slide, cropped to `bounds`,
    /// at 2 px per point (system clipboard picture).
    fn selection_png(&mut self, slide: usize, ids: Vec<u32>, bounds: Rect) -> Option<String> {
        use base64::Engine as _;
        const SCALE: f64 = 2.0;
        let pres = self.pres.as_ref()?;
        let (w, h) = (bounds.w.max(1.0), bounds.h.max(1.0));
        if w * SCALE > 4096.0 || h * SCALE > 4096.0 {
            return None;
        }
        let opts = bo_pptx::RenderOptions {
            only: Some(ids),
            ..bo_pptx::RenderOptions::default()
        };
        let page =
            bo_pptx::render_with(pres, slide, &mut self.fonts, &mut self.images, &opts).ok()?;
        let mut commands = Vec::with_capacity(page.commands.len() + 2);
        commands.push(bo_render::Command::PushTransform {
            m: [1.0, 0.0, 0.0, 1.0, -bounds.x as f32, -bounds.y as f32],
        });
        commands.extend(page.commands);
        commands.push(bo_render::Command::PopTransform);
        let cropped = bo_render::PageList {
            width: w as f32,
            height: h as f32,
            commands,
        };
        let png =
            bo_raster::render_page_png_with(&cropped, &self.fonts, SCALE as f32, &self.images)
                .ok()?;
        Some(base64::engine::general_purpose::STANDARD.encode(png))
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
            if let Some((kind, _)) = media::media_kind(name) {
                let poster =
                    bo_raster::render_page_png(&media::poster_page(kind), &self.fonts, 2.0)
                        .unwrap_or_default();
                ed.mutate(pres, &mut out, |tx, sel| {
                    media::cmd_insert_media(tx, sel, bytes, name, poster)
                });
            } else {
                ed.mutate(pres, &mut out, |tx, sel| cmd_picture(tx, sel, bytes, name));
            }
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// Layers, effects and clips of a slide for the slide show.
    fn slide_show(&mut self, slide: usize) -> Option<show::ShowOut> {
        let pres = self.pres.as_ref()?;
        let ranges = show::layers(pres, slide);
        let mut layers = Vec::with_capacity(ranges.len());
        for r in &ranges {
            let opts = bo_pptx::RenderOptions {
                layer: Some(*r),
                ..bo_pptx::RenderOptions::default()
            };
            let page =
                bo_pptx::render_with(pres, slide, &mut self.fonts, &mut self.images, &opts).ok()?;
            layers.push(show::LayerOut {
                ids: show::layer_ids(pres, slide, *r),
                backdrop: r.backdrop,
                page,
            });
        }
        let mut animations = show::animations_out(pres, slide);
        for a in &mut animations {
            a.layer = show::layer_of(pres, slide, &ranges, a.shape_id);
        }
        let (width, height) = slide_size(pres);
        Some(show::ShowOut {
            slide,
            width: width as f32,
            height: height as f32,
            transition: show::transition_out(pres, slide),
            clicks: pres
                .slides
                .get(slide)
                .and_then(|s| s.timing.as_ref())
                .map_or(0, bo_model::slide::Timing::click_count),
            layers,
            animations,
            triggers: show::triggers_out(pres, slide),
            media: media::media_out(pres, slide),
            morph: self.morph_out(slide),
        })
    }

    /// Morph into `slide` from the slide before it: matched shapes drawn on both slides, the
    /// others and both backdrops ([MS-PPTX] p159:morph).
    fn morph_out(&mut self, slide: usize) -> Option<morph::MorphOut> {
        let pres = self.pres.as_ref()?;
        let t = show::transition_out(pres, slide)?;
        if t.kind != "morph" || slide == 0 {
            return None;
        }
        let prev = slide - 1;
        let (a, b) = (morph::states(pres, prev), morph::states(pres, slide));
        let pairs = morph::match_shapes(&a, &b);
        let mut draw = |s: usize, range: bo_pptx::ShapeRange| {
            let opts = bo_pptx::RenderOptions {
                layer: Some(range),
                ..bo_pptx::RenderOptions::default()
            };
            bo_pptx::render_with(pres, s, &mut self.fonts, &mut self.images, &opts).ok()
        };
        let one = |i: usize| bo_pptx::ShapeRange {
            start: i,
            end: i + 1,
            backdrop: false,
        };
        let backdrop = bo_pptx::ShapeRange {
            start: 0,
            end: 0,
            backdrop: true,
        };
        let mut out = morph::MorphOut {
            from_backdrop: draw(prev, backdrop)?,
            to_backdrop: draw(slide, backdrop)?,
            pairs: Vec::new(),
            gone: Vec::new(),
            new: Vec::new(),
            option: t.dir.unwrap_or_else(|| "byObject".into()),
        };
        for &(i, j) in &pairs {
            out.pairs.push(morph::MorphPair {
                from: morph::box5(&a[i].b),
                to: morph::box5(&b[j].b),
                from_page: draw(prev, one(a[i].index))?,
                to_page: draw(slide, one(b[j].index))?,
            });
        }
        let count = |s: usize| pres.slides.get(s).map_or(0, |x| x.data.shapes.len());
        for k in 0..count(prev) {
            if !pairs.iter().any(|p| a[p.0].index == k) {
                out.gone.push(draw(prev, one(k))?);
            }
        }
        for k in 0..count(slide) {
            if !pairs.iter().any(|p| b[p.1].index == k) {
                out.new.push(draw(slide, one(k))?);
            }
        }
        Some(out)
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
        if let Some((t, rc)) = ed.text.and_then(|t| Some((t, t.cell?))) {
            if let (Some(p), Some(tbl)) = (find(&placed, t.id), table_of(pres, slide, t.id)) {
                let s = cell_shape(&tbl, rc);
                format = Some(format_state_at(pres, slide, &s, Some((t.start, t.end))));
                let body = s.text.as_ref().map(text::body_text).unwrap_or_default();
                if let Some((g, b)) = cell_geometry(pres, slide, &p.b, &tbl, rc, &mut self.fonts) {
                    let (caret, rects) = rich::selection_shapes(&g, &b, t.start, t.end);
                    let (start, end) = rich::to_utf16(&body, t);
                    text_edit = Some(rich::TextEditOut {
                        id: t.id,
                        start,
                        end,
                        caret,
                        rects,
                        rect: b,
                        text: body,
                        cell: Some([rc.0, rc.1]),
                    });
                }
            }
        } else if let Some(t) = ed.text {
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
                        cell: None,
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
        let table_state = ed.table.and_then(|ts| {
            let p = find(&placed, ts.id)?;
            match ops::node_at(pres, slide, &p.path)? {
                ShapeNode::Frame(f) => match &f.content {
                    bo_model::slide::GraphicContent::Table(t) => {
                        Some(table::table_out(t, &p.b, ts))
                    }
                    bo_model::slide::GraphicContent::Other(_) => None,
                },
                _ => None,
            }
        });
        let mut adjust = Vec::new();
        let mut connector = None;
        let mut sites = Vec::new();
        if let [one] = items.as_slice() {
            if let Some(n) = ops::node_at(pres, slide, &one.path) {
                adjust = connect::adjust_handles(pres, slide, one, n);
                if let ShapeNode::Connector(c) = n {
                    if let Some(x) = &c.props.xfrm {
                        let [a, b] = connect::ends(one, x);
                        connector = Some(connect::ConnectorOut {
                            id: one.id,
                            ends: [[a.0, a.1], [b.0, b.1]],
                            attached: [
                                connect::cxn_ref(c, 0).is_some(),
                                connect::cxn_ref(c, 1).is_some(),
                            ],
                        });
                        let mut flat = Vec::new();
                        connect::walk(&placed, &mut |p| flat.push(p));
                        for p in flat.into_iter().filter(|p| p.id != one.id) {
                            if let Some(pn) = ops::node_at(pres, slide, &p.path) {
                                sites.extend(
                                    connect::sites(pres, slide, p, pn)
                                        .into_iter()
                                        .map(|(x, y)| [x, y]),
                                );
                            }
                        }
                    }
                }
            }
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
            table: table_state,
            adjust,
            connector,
            sites,
            clip: match self.slide_clip.as_ref().map(|c| &c.kind) {
                Some(clip::ClipKind::Shapes { .. }) => Some("shapes"),
                Some(clip::ClipKind::Slides(_)) => Some("slides"),
                None => None,
            },
            clipboard: None,
            transition: show::transition_out(pres, slide),
            animations: show::animations_out(pres, slide),
            objects: show::objects_out(pres, slide),
            media: media::media_out(pres, slide),
            chart: match items.as_slice() {
                [one] => chart::chart_out(pres, slide, one.id),
                _ => None,
            },
            chart_gallery: out.chart_gallery,
            smartart: match items.as_slice() {
                [one] => smartart::smartart_out(pres, slide, one.id),
                _ => None,
            },
            show: None,
            comments: comments::comments_out(pres, slide),
        })
    }
}

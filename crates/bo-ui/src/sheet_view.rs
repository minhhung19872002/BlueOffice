//! BlueSheet grid (ADR-0018, plan N6): column letters and row numbers, the cells drawn by the
//! in-process [`SheetEngine`] into one grid image, the selection and the in-cell editor.
//!
//! Colours and sizes were measured black-box on a desktop spreadsheet at 125 % display scale
//! (docs/ui/REPORT-sheet.md): grey #D4D4D4 gridlines, #E6E6E6 headers with #999999 edges,
//! #D2D2D2 headers of the selection with green labels, a two-pixel green frame around the
//! selection straddling the cell's gridlines, and a fill handle at its bottom-right corner.
use crate::{Font, Input, Key, Mods, Painter, TextMeasure};
use bo_engine::sheet::SheetCommand;
use bo_engine::SheetEngine;
use bo_model::sheet::{col_to_letters, CellRange, CellRef, MAX_COLS, MAX_ROWS};
use bo_render::{PageList, PathSeg, Rgba};
use bo_sheet::viewport::{PaneKind, Viewport};
use serde_json::{json, Value};

#[path = "sheet_pages.rs"]
mod pages_impl;

/// Keeps the grid image drawn from the engine's display list (a GPU bitmap on Windows).
pub trait GridStore {
    /// Draw `list` (points) at `scale` device pixels per point as the new grid image; the
    /// glyph runs refer to `engine`'s fonts.
    fn update(&mut self, list: &PageList, engine: &SheetEngine, scale: f32) -> Result<(), String>;

    /// The view scrolled by whole device pixels: keep the still-visible part of the last
    /// image by applying moves (old image to new image), then draw list into ands
    /// only (left, top, width, height in device pixels). Stores that cannot move pixels
    /// redraw the whole image.
    fn scroll(
        &mut self,
        list: &PageList,
        engine: &SheetEngine,
        scale: f32,
        moves: &[GridMove],
        bands: &[[u32; 4]],
    ) -> Result<(), String> {
        let _ = (moves, bands);
        self.update(list, engine, scale)
    }

    /// The view scrolled by whole device pixels with nothing frozen along that axis: a store
    /// keeping its image as a ring ([`GridRing`]) turns it by `shift` (the new image at
    /// (x, y) is the old one at (x + dx, y + dy)) and draws only `bands`, without copying the
    /// kept pixels. `Ok(false)`: not supported, [`GridStore::scroll`] is used instead.
    fn turn(
        &mut self,
        list: &PageList,
        engine: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[[u32; 4]],
    ) -> Result<bool, String> {
        let _ = (list, engine, scale, shift, bands);
        Ok(false)
    }

    /// [`GridStore::turn`] with a display list laid out for each newly exposed band only
    /// (round 27: the rest of the viewport is not laid out again). `size` is the whole
    /// image's size in device pixels. `Ok(false)`: not supported (the caller lays out the
    /// whole viewport and uses [`GridStore::turn`]).
    fn turn_bands(
        &mut self,
        engine: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[GridBand],
        size: (u32, u32),
    ) -> Result<bool, String> {
        let _ = (engine, scale, shift, bands, size);
        Ok(false)
    }
}

/// A newly exposed band of the grid image with its own display list.
#[derive(Debug, Clone)]
pub struct GridBand {
    /// The band in image pixels (left, top, width, height).
    pub rect: [u32; 4],
    /// Image pixel of the list's origin (the list covers the band and a small margin).
    pub origin: [u32; 2],
    /// Display list of the band (points, origin at `origin`).
    pub list: PageList,
}

/// A grid image kept as a ring: image pixel (x, y) is stored at ((x + px) mod w,
/// (y + py) mod h), so scrolling only changes the phase (px, py) and draws the newly
/// exposed band, and showing the image takes up to four pieces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridRing {
    phase: [u32; 2],
    size: [u32; 2],
}

impl GridRing {
    /// A ring of `width` × `height` pixels at phase zero (a freshly drawn image).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            phase: [0, 0],
            size: [width, height],
        }
    }

    /// Where image pixel (0, 0) is stored.
    pub fn phase(&self) -> [u32; 2] {
        self.phase
    }

    /// Scroll: the new image at (x, y) is the old one at (x + dx, y + dy).
    pub fn turn(&mut self, shift: [i64; 2]) {
        for (i, d) in shift.iter().enumerate() {
            if self.size[i] > 0 {
                let n = i64::from(self.size[i]);
                self.phase[i] = (i64::from(self.phase[i]) + d).rem_euclid(n) as u32;
            }
        }
    }

    /// The pieces of image rectangle `r` (left, top, width, height): each is the image
    /// rectangle and the top-left corner where the ring stores it.
    pub fn split(&self, r: [u32; 4]) -> Vec<([u32; 4], [u32; 2])> {
        let axis = |a: u32, len: u32, p: u32, n: u32| -> Vec<(u32, u32, u32)> {
            let a = a.min(n);
            let len = len.min(n - a);
            if len == 0 {
                return Vec::new();
            }
            let s = (a + p) % n;
            let first = len.min(n - s);
            let mut v = vec![(a, first, s)];
            if first < len {
                v.push((a + first, len - first, 0));
            }
            v
        };
        let xs = axis(r[0], r[2], self.phase[0], self.size[0]);
        let ys = axis(r[1], r[3], self.phase[1], self.size[1]);
        let mut out = Vec::with_capacity(4);
        for &(ly, lh, py) in &ys {
            for &(lx, lw, px) in &xs {
                out.push(([lx, ly, lw, lh], [px, py]));
            }
        }
        out
    }

    /// The pieces of the whole image (to show it).
    pub fn pieces(&self) -> Vec<([u32; 4], [u32; 2])> {
        self.split([0, 0, self.size[0], self.size[1]])
    }
}

/// Scroll position, extent and visible size of both axes (vertical first).
type BarFigures = [(f64, f64, f64); 2];

/// What the cached scroll bar figures were computed for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BarKey {
    scroll: (f64, f64),
    frame: [f32; 4],
    sheet: usize,
    zoom: f32,
    scale: f32,
    content_gen: u64,
}

/// A rectangle of the previous grid image kept at a new place when scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridMove {
    /// Left, top, width, height in the previous image (device pixels).
    pub src: [u32; 4],
    /// Top-left corner in the new image.
    pub dst: [u32; 2],
}

/// Gridlines.
pub const GRID_LINE: Rgba = Rgba(0xd4, 0xd4, 0xd4, 255);
/// Header background.
const HDR_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
/// Header background of selected rows / columns.
const HDR_SEL_BG: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
/// Lines between column headers, row headers, and next to a selected header.
const HDR_COL_LINE: Rgba = Rgba(0xc9, 0xc9, 0xc9, 255);
const HDR_ROW_LINE: Rgba = Rgba(0xd0, 0xd0, 0xd0, 255);
const HDR_SEL_LINE: Rgba = Rgba(0xb6, 0xb6, 0xb6, 255);
/// Edge between the headers and the cells.
const HDR_EDGE: Rgba = Rgba(0x99, 0x99, 0x99, 255);
/// Header labels.
const HDR_TEXT: Rgba = Rgba(0x00, 0x00, 0x00, 255);
/// Selection frame, labels of selected headers.
pub const SEL_GREEN: Rgba = Rgba(0x21, 0x73, 0x46, 255);
/// Tint over the selected cells other than the active one (white #FFFFFF shows as #C6C6C6,
/// gridlines #A5A5A5, as measured); a one-pixel ring inside the frame stays untinted.
const SEL_TINT: Rgba = Rgba(0x00, 0x00, 0x00, 57);
/// Select-all triangle in the corner.
const CORNER_MARK: Rgba = Rgba(0xb4, 0xb4, 0xb4, 255);
/// Line under frozen rows / right of frozen columns.
const FREEZE_LINE: Rgba = Rgba(0x9b, 0x9b, 0x9b, 255);

/// Formatting of the active cell, as the ribbon shows it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellFormat {
    /// Font family.
    pub font: String,
    /// Font size in points.
    pub size: f64,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Struck through.
    pub strike: bool,
    /// Font colour `RRGGBB` (empty: automatic).
    pub color: String,
    /// Horizontal alignment (empty: general).
    pub h_align: String,
    /// Vertical alignment (empty: bottom).
    pub v_align: String,
    /// Wrap text.
    pub wrap: bool,
    /// Number format code.
    pub num_fmt: String,
}

/// The in-cell editor.
#[derive(Debug, Clone, PartialEq)]
struct Editor {
    text: String,
    /// Caret as a byte offset into `text`.
    caret: usize,
    /// Entered by typing over the cell ("Enter" mode: arrows commit and move); `false` after
    /// F2 or a double click ("Edit" mode: arrows move the caret).
    typed: bool,
    /// Width of the text before the caret, in device pixels.
    caret_x: f32,
    /// Width of the whole text.
    text_w: f32,
    /// Text being composed by an input method.
    preedit: String,
    /// The reference inserted by pointing at cells (byte range in `text`).
    point: Option<std::ops::Range<usize>>,
    /// Cells of the pointed reference: anchor and moving end (arrow keys move the end).
    point_cells: Option<((u32, u32), (u32, u32))>,
    /// Pieces of a formula's text, references in their frame colour (`None`: the cell's
    /// colour): x offset from the text start, byte range of `text`, colour.
    runs: Vec<(f32, std::ops::Range<usize>, Option<Rgba>)>,
    /// AutoComplete: the text from this byte offset is the suggested rest of an entry of the
    /// column (shown selected; typing replaces it, Backspace removes it, Enter keeps it).
    auto: Option<usize>,
    /// The entry suggested (its letter case is kept when the suggestion is accepted).
    auto_entry: String,
    /// Width of the text before the suggestion.
    auto_x: f32,
}

/// Placed column or row: index, first pixel (window), pixel of its closing gridline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    index: u32,
    start: f32,
    line: f32,
}

/// What a pointer drag does.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Cells,
    Cols,
    Rows,
    /// Resizing column / row `index` from the edge at window pixel `origin`.
    ColEdge(u32, f32),
    RowEdge(u32, f32),
    /// Dragging the fill handle of the selection (its bounds when the drag began).
    Fill(u32, u32, u32, u32),
    /// Moving the selection by its border (grabbed at a cell): the cells go where the grey
    /// frame (`fill_to`) is on release, as the reference's drag-and-drop.
    Move(u32, u32),
    /// Pointing at cells for a formula being typed: the cell the drag began on.
    Point(u32, u32),
    /// Dragging a page break in Page Break Preview: (print area, rows, its position).
    Break(usize, bool, u32),
}

/// Colours of the reference frames drawn around the cells a formula refers to while it is
/// edited, in the order the references appear (measured on the reference spreadsheet, which
/// repeats its seven colours; the reference text in the cell editor takes the same colour).
pub const REF_COLORS: [Rgba; 7] = [
    Rgba(0x32, 0x6a, 0xc7, 255),
    Rgba(0xc0, 0x35, 0x3e, 255),
    Rgba(0x81, 0x57, 0xb7, 255),
    Rgba(0x00, 0x7c, 0x20, 255),
    Rgba(0xb0, 0x3e, 0x84, 255),
    Rgba(0xb6, 0x49, 0x00, 255),
    Rgba(0x26, 0x73, 0x92, 255),
];

/// Opacity of the tint inside a reference frame (10 % of its colour over white, measured).
const REF_TINT_ALPHA: u8 = 26;

/// A reference of a formula: byte range in the text and cell bounds (first row, first column,
/// last row, last column).
pub type FormulaRef = (std::ops::Range<usize>, (u32, u32, u32, u32));

/// A1 references in a formula (`=A1+SUM(B2:C3)`): byte range and cell bounds
/// (first row, first column, last row, last column), outside string literals.
pub fn formula_refs(text: &str) -> Vec<FormulaRef> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    if b.first() != Some(&b'=') {
        return out;
    }
    // One cell reference at `i`: ($)letters($)digits → (end, row, col).
    let cell = |mut i: usize| -> Option<(usize, u32, u32)> {
        if b.get(i) == Some(&b'$') {
            i += 1;
        }
        let ls = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i == ls || i - ls > 3 {
            return None;
        }
        let letters = &text[ls..i];
        if b.get(i) == Some(&b'$') {
            i += 1;
        }
        let ds = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == ds || i - ds > 7 {
            return None;
        }
        let row: u32 = text[ds..i].parse().ok()?;
        let col = letters.bytes().try_fold(0_u32, |a, c| {
            Some(a * 26 + u32::from(c.to_ascii_uppercase() - b'A') + 1)
        })?;
        ((1..=MAX_ROWS).contains(&row) && (1..=MAX_COLS).contains(&col)).then_some((
            i,
            row - 1,
            col - 1,
        ))
    };
    let mut i = 1;
    let mut quoted = false;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            quoted = !quoted;
            i += 1;
            continue;
        }
        let starts = !quoted
            && (c == b'$' || c.is_ascii_alphabetic())
            && !b[i - 1].is_ascii_alphanumeric()
            && !matches!(b[i - 1], b'_' | b'.' | b'!' | b'\'' | b'$');
        if starts {
            if let Some((end, r, col)) = cell(i) {
                // A name or function continues with letters, digits, `(` or `_`.
                let next = b.get(end).copied();
                if !next.is_some_and(|n| n.is_ascii_alphanumeric() || n == b'(' || n == b'_') {
                    let (mut stop, mut bounds) = (end, (r, col, r, col));
                    if next == Some(b':') {
                        if let Some((e2, r2, c2)) = cell(end + 1) {
                            stop = e2;
                            bounds = (r.min(r2), col.min(c2), r.max(r2), col.max(c2));
                        }
                    }
                    out.push((i..stop, bounds));
                    i = stop;
                    continue;
                }
            }
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    out
}

/// Statistics of the selection for the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    /// Sum of the numbers.
    pub sum: f64,
    /// Non-empty cells.
    pub count: u64,
    /// Numbers.
    pub numbers: u64,
    /// Average of the numbers.
    pub average: Option<f64>,
    /// Smallest number.
    pub min: Option<f64>,
    /// Largest number.
    pub max: Option<f64>,
}

/// The grid of one workbook window.
pub struct SheetView {
    engine: SheetEngine,
    /// A drawing (chart) is selected: the cell selection is not drawn.
    object_selected: bool,
    sheet: usize,
    /// Window rectangle of the grid with its headers.
    frame: [f32; 4],
    scale: f32,
    zoom: f32,
    /// Scroll of the main pane from the first unfrozen row / column, in points.
    scroll: (f64, f64),
    anchor: (u32, u32),
    extent: (u32, u32),
    active: (u32, u32),
    edit: Option<Editor>,
    grid_dirty: bool,
    /// The grid image must be drawn whole (cells, size or scale changed), not only scrolled.
    full_redraw: bool,
    /// Scrolling lays out only the exposed rows (set_band_lists).
    band_lists: bool,
    /// Scroll bar figures of both axes and what they were computed for: the sheet's content
    /// size scans every cell, so it is only redone after edits, not per scroll step.
    bar_cache: Option<(BarKey, BarFigures)>,
    /// The sheet's content size (points) for (sheet, content generation): scanning every
    /// cell is the costly part of the scroll bars, and scrolling does not change it.
    content_cache: Option<((usize, u64), (f64, f64))>,
    /// Counts edits that may change the content size or row / column geometry.
    content_gen: u64,
    /// Scroll of the last drawn grid image.
    drawn: Option<(f64, f64)>,
    drag: Option<Drag>,
    cols: Vec<Span>,
    rows: Vec<Span>,
    /// Frozen boundary lines (window pixels) when panes are frozen.
    freeze: (Option<f32>, Option<f32>),
    /// Last placed (grid) pixel position in each direction: cells beyond are not shown.
    content_end: (f32, f32),
    /// Selection and active cell rectangles (outer frame: left, top, right, bottom pixels of
    /// the gridlines around them), when visible.
    sel_rect: Option<[f32; 4]>,
    active_rect: Option<[f32; 4]>,
    /// Header font (the workbook's default font).
    header_font: (String, f64),
    format: CellFormat,
    details: (String, String),
    stats: Stats,
    modified: bool,
    /// Copied range (dashed frame) while the clipboard holds it.
    copied: Option<(usize, CellRange)>,
    /// Text clipboard form of the last copy (recognises our own data when pasting).
    clip_text: String,
    error: Option<String>,
    /// Measured header label widths at `label_size` pixels.
    label_widths: std::collections::HashMap<String, f32>,
    label_size: f32,
    /// View settings: gridlines, row / column headings.
    show_grid: bool,
    /// Formulas are shown in their cells instead of their results (Formulas tab).
    show_formulas: bool,
    show_headings: bool,
    /// Cells the fill handle drag would fill (bounds), while dragging.
    fill_to: Option<(u32, u32, u32, u32)>,
    /// Ctrl held when the mouse button is released (set by the host).
    mouse_ctrl: bool,
    /// Page layout view (pages instead of the grid), when shown.
    layout: Option<pages_impl::PageLayoutView>,
    /// Page Break Preview: print areas and breaks, when shown.
    breaks: Option<Vec<pages_impl::BreakArea>>,
    /// "Page {n}" (Page Break Preview) and "Add header" (page layout view) in the UI language.
    page_labels: (String, String),
}

fn rgba_hex(s: &str) -> Option<Rgba> {
    let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    (s.len() == 6).then_some(())?;
    Some(Rgba(v(0)?, v(2)?, v(4)?, 255))
}

fn prev_char(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

fn next_char(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

impl SheetView {
    /// A view of `engine`'s workbook (one must be open), first sheet, cell A1 selected.
    pub fn new(engine: SheetEngine) -> Self {
        let header_font = engine
            .workbook()
            .and_then(|w| w.styles.fonts.first())
            .map_or(("Calibri".to_owned(), 11.0), |f| {
                (
                    f.name.clone().unwrap_or_else(|| "Calibri".into()),
                    f.size.unwrap_or(11.0),
                )
            });
        let mut v = Self {
            engine,
            object_selected: false,
            sheet: 0,
            frame: [0.0, 0.0, 800.0, 600.0],
            scale: 1.0,
            zoom: 1.0,
            scroll: (0.0, 0.0),
            anchor: (0, 0),
            extent: (0, 0),
            active: (0, 0),
            edit: None,
            grid_dirty: true,
            full_redraw: true,
            band_lists: true,
            bar_cache: None,
            content_cache: None,
            content_gen: 0,
            drawn: None,
            drag: None,
            cols: Vec::new(),
            rows: Vec::new(),
            freeze: (None, None),
            content_end: (0.0, 0.0),
            sel_rect: None,
            active_rect: None,
            header_font,
            format: CellFormat::default(),
            details: (String::new(), String::new()),
            stats: Stats::default(),
            modified: false,
            copied: None,
            clip_text: String::new(),
            error: None,
            label_widths: std::collections::HashMap::new(),
            label_size: 0.0,
            show_grid: true,
            show_formulas: false,
            show_headings: true,
            fill_to: None,
            mouse_ctrl: false,
            layout: None,
            breaks: None,
            page_labels: ("Page {n}".into(), "Add header".into()),
        };
        // Start on the sheet the workbook was saved on, at its first unfrozen cell.
        let active = v
            .engine
            .workbook()
            .and_then(|w| {
                w.sheets
                    .iter()
                    .position(|s| s.views.first().is_some_and(|v| v.tab_selected))
            })
            .unwrap_or(0);
        v.sheet = active;
        v.refresh();
        v
    }

    /// The engine (saving, statistics).
    pub fn engine(&self) -> &SheetEngine {
        &self.engine
    }

    /// The engine, for commands of the window.
    pub fn engine_mut(&mut self) -> &mut SheetEngine {
        &mut self.engine
    }

    /// Current sheet index.
    pub fn sheet(&self) -> usize {
        self.sheet
    }

    /// Unsaved changes.
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// The workbook was saved.
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Zoom factor (1.0 = 100 %).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Formatting of the active cell.
    pub fn format(&self) -> &CellFormat {
        &self.format
    }

    /// Statistics of the selection.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Last error of an edit (shown by the window), taken.
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    /// Whether a cell is being edited.
    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }

    /// Address of the active cell, or the size of the selection while dragging (name box).
    pub fn name_box(&self) -> String {
        if self.drag == Some(Drag::Cells) && self.anchor != self.extent {
            let (r0, c0, r1, c1) = self.range_bounds();
            return format!("{}R x {}C", r1 - r0 + 1, c1 - c0 + 1);
        }
        self.details.0.clone()
    }

    /// Formula bar text: the editor's text while editing, else the active cell's input.
    pub fn formula_text(&self) -> &str {
        self.edit.as_ref().map_or(&self.details.1, |e| &e.text)
    }

    /// Caret byte offset in the formula bar text while editing.
    pub fn formula_caret(&self) -> Option<usize> {
        self.edit.as_ref().map(|e| e.caret)
    }

    /// Device pixels per point of the grid.
    fn k(&self) -> f32 {
        self.zoom * self.scale * 4.0 / 3.0
    }

    /// Height of the column header band (24 px at 125 %, as measured).
    fn hdr_h(&self) -> f32 {
        if !self.show_headings {
            return 0.0;
        }
        (19.2 * self.scale * self.zoom).round()
    }

    /// Width of the row header band: room for the largest row number shown (34 px at
    /// 125 % for up to two digits, as measured), its last pixel being the edge line.
    fn hdr_w(&self) -> f32 {
        if !self.show_headings {
            return 0.0;
        }
        let last = self.rows.last().map_or(1, |r| r.index + 1);
        let digits = last.to_string().len().max(2) as f32;
        ((digits * 7.0 + 13.0) * self.scale * self.zoom).round()
    }

    /// Window x / y of the grid image's top-left corner.
    fn grid_origin(&self) -> (f32, f32) {
        (self.frame[0] + self.hdr_w(), self.frame[1] + self.hdr_h())
    }

    /// Place the grid in the window (x, y, width, height in device pixels).
    pub fn set_frame(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let f = [x.round(), y.round(), w.round().max(1.0), h.round().max(1.0)];
        if f != self.frame {
            self.frame = f;
            self.redraw_all();
            self.refresh();
        }
    }

    /// Display scale (device pixels per logical pixel).
    pub fn set_scale(&mut self, scale: f32) {
        if scale > 0.0 && scale != self.scale {
            self.scale = scale;
            self.redraw_all();
            self.refresh();
        }
    }

    /// Set the zoom (0.1–4.0); `true` when it changed.
    pub fn set_zoom(&mut self, zoom: f32) -> bool {
        let z = zoom.clamp(0.1, 4.0);
        if (z - self.zoom).abs() < 1e-4 {
            return false;
        }
        self.zoom = z;
        self.redraw_all();
        self.refresh();
        true
    }

    /// Viewport of the grid image, in points (no headers: the window draws them).
    pub(crate) fn viewport(&self) -> Viewport {
        let k = f64::from(self.k());
        let (gx, gy) = self.grid_origin();
        let w = f64::from((self.frame[0] + self.frame[2] - gx).max(1.0));
        let h = f64::from((self.frame[1] + self.frame[3] - gy).max(1.0));
        Viewport {
            scroll_x: self.scroll.0,
            scroll_y: self.scroll.1,
            width: w / k,
            height: h / k,
            header_width: 0.0,
            header_height: 0.0,
        }
    }

    /// Lay out the visible headers, selection rectangles and the active cell's details.
    fn refresh(&mut self) {
        self.refresh_with(true);
    }

    /// [`Self::refresh`]; scrolling (`details` false) keeps the active cell's details, whose
    /// reading costs more than the layout (1.3 ms of a 1.5 ms wheel notch on a 10 000-row
    /// sheet, round 27).
    fn refresh_with(&mut self, details: bool) {
        // Row heights in whole pixels at this scale, as the reference shows them.
        self.engine.set_display_scale(Some(f64::from(self.k())));
        let vp = self.viewport();
        let (gx, gy) = self.grid_origin();
        let k = self.k();
        let sheet = self.sheet;
        let range = self.range();
        let (ar, ac) = self.active;
        // Pixel of the gridline the engine draws for a boundary at screen point `pt`: it
        // centres a 0.75 pt line just before the boundary, snapped to whole pixels.
        let line_px = |pt: f64| -> f32 {
            let w = (0.75 * k).round().max(1.0);
            let c = (pt as f32 - 0.375) * k;
            if w % 2.0 == 1.0 {
                (c - 0.5).round()
            } else {
                (c - w / 2.0).round()
            }
        };
        let Ok((layout, geom)) = self.engine.view_layout(sheet, &vp) else {
            return;
        };
        let mut cols = Vec::new();
        let mut rows = Vec::new();
        for kind in [PaneKind::Left, PaneKind::Main] {
            if let Some(p) = layout.panes.iter().find(|p| p.kind == kind) {
                for c in p.cols.clone() {
                    if geom.col_width(c) <= 0.0 {
                        continue;
                    }
                    let a = p.to_screen_x(geom.col_x(c));
                    let b = p.to_screen_x(geom.col_x(c + 1));
                    if a >= p.screen.x + p.screen.w {
                        continue;
                    }
                    cols.push(Span {
                        index: c,
                        start: gx + line_px(a) + 1.0,
                        line: gx + line_px(b),
                    });
                }
            }
        }
        for kind in [PaneKind::Top, PaneKind::Main] {
            if let Some(p) = layout.panes.iter().find(|p| p.kind == kind) {
                for r in p.rows.clone() {
                    if geom.row_height(r) <= 0.0 {
                        continue;
                    }
                    let a = p.to_screen_y(geom.row_y(r));
                    let b = p.to_screen_y(geom.row_y(r + 1));
                    if a >= p.screen.y + p.screen.h {
                        continue;
                    }
                    rows.push(Span {
                        index: r,
                        start: gy + line_px(a) + 1.0,
                        line: gy + line_px(b),
                    });
                }
            }
        }
        let freeze = (
            (layout.frozen_cols > 0).then(|| gx + line_px(geom.col_x(layout.frozen_cols))),
            (layout.frozen_rows > 0).then(|| gy + line_px(geom.row_y(layout.frozen_rows))),
        );
        // Outer frame of a range on screen: the pixels of the gridlines around it.
        let rect_of = |r: CellRange| -> Option<[f32; 4]> {
            let a = layout.cell_screen_rect(geom, r.first.row, r.first.col)?;
            let b = layout.cell_screen_rect(geom, r.last.row, r.last.col)?;
            let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
            let (x1, y1) = ((a.x + a.w).max(b.x + b.w), (a.y + a.h).max(b.y + b.h));
            Some([
                gx + line_px(x0),
                gy + line_px(y0),
                gx + line_px(x1),
                gy + line_px(y1),
            ])
        };
        let single = |r: u32, c: u32| {
            let at = CellRef::new(r, c);
            geom.merge_at(at).unwrap_or(CellRange::single(at))
        };
        // Whole rows / columns reach far off-screen; drawing clips them.
        let sel = rect_of(range);
        let act = rect_of(single(ar, ac));
        self.cols = cols;
        self.rows = rows;
        self.freeze = freeze;
        self.content_end = (
            self.cols.last().map_or(gx, |c| c.line),
            self.rows.last().map_or(gy, |r| r.line),
        );
        self.sel_rect = sel;
        self.active_rect = act;
        if details {
            self.load_details();
        }
    }

    fn load_details(&mut self) {
        let (r, c) = self.active;
        let d = self.engine.cell(self.sheet, r, c).ok();
        self.details = d.map_or_else(
            || (CellRef::new(r, c).to_string(), String::new()),
            |d| (d.address, d.input),
        );
        let st = self
            .engine
            .exec(SheetCommand::FormatState {
                sheet: self.sheet,
                row: r,
                col: c,
            })
            .unwrap_or(Value::Null);
        let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
        self.format = CellFormat {
            font: s("fontName"),
            size: st.get("fontSize").and_then(Value::as_f64).unwrap_or(11.0),
            bold: b("bold"),
            italic: b("italic"),
            underline: !matches!(s("underline").as_str(), "" | "none"),
            strike: b("strike"),
            color: s("fontColor"),
            h_align: s("hAlign"),
            v_align: s("vAlign"),
            wrap: b("wrap"),
            num_fmt: s("numFmt"),
        };
        if self.format.font.is_empty() {
            self.format.font.clone_from(&self.header_font.0);
        }
        self.stats = self.compute_stats();
    }

    fn compute_stats(&mut self) -> Stats {
        let (r0, c0, r1, c1) = self.range_bounds();
        if r0 == r1 && c0 == c1 {
            return Stats::default();
        }
        // Only the used part of whole rows / columns is visited.
        let (lr, lc) = self
            .engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
            .map_or((0, 0), |ws| {
                (
                    ws.cells.keys().next_back().map_or(0, |k| k.0),
                    ws.cells.keys().map(|k| k.1).max().unwrap_or(0),
                )
            });
        let (r1, c1) = (r1.min(lr), c1.min(lc));
        if r0 > r1 || c0 > c1 {
            return Stats::default();
        }
        let v = self
            .engine
            .exec(
                serde_json::from_value(json!({
                    "cmd": "rangeStats", "sheet": self.sheet,
                    "ranges": [{ "r0": r0, "c0": c0, "r1": r1, "c1": c1 }],
                }))
                .unwrap_or(SheetCommand::Presets),
            )
            .unwrap_or(Value::Null);
        Stats {
            sum: v.get("sum").and_then(Value::as_f64).unwrap_or(0.0),
            count: v.get("count").and_then(Value::as_u64).unwrap_or(0),
            numbers: v.get("numbers").and_then(Value::as_u64).unwrap_or(0),
            average: v.get("average").and_then(Value::as_f64),
            min: v.get("min").and_then(Value::as_f64),
            max: v.get("max").and_then(Value::as_f64),
        }
    }

    /// The selected range (any corner order normalised).
    fn range(&self) -> CellRange {
        let (r0, c0, r1, c1) = self.range_bounds();
        CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1))
    }

    /// First row, first column, last row, last column of the selection.
    pub fn range_bounds(&self) -> (u32, u32, u32, u32) {
        let (a, e) = (self.anchor, self.extent);
        (a.0.min(e.0), a.1.min(e.1), a.0.max(e.0), a.1.max(e.1))
    }

    /// The selection as an engine range.
    pub fn rng(&self) -> Value {
        let (r0, c0, r1, c1) = self.range_bounds();
        json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
    }

    /// Run a sheet command given as JSON (`sheet` is filled in); `true` when it ran.
    pub fn run(&mut self, mut cmd: Value) -> bool {
        if let Some(o) = cmd.as_object_mut() {
            o.entry("sheet").or_insert(json!(self.sheet));
        }
        let parsed: Result<SheetCommand, _> = serde_json::from_value(cmd);
        match parsed {
            Ok(c) => self.exec(c),
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// Run any engine command given as JSON (`sheet` filled in when absent) and return its
    /// result; nothing is marked as changed (queries, plans).
    pub fn query(&mut self, mut cmd: Value) -> Option<Value> {
        if let Some(o) = cmd.as_object_mut() {
            o.entry("sheet").or_insert(json!(self.sheet));
        }
        match self.engine.exec_json(&cmd.to_string()) {
            Ok(out) => serde_json::from_str(&out).ok().or(Some(Value::Null)),
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        }
    }

    /// Open the cell editor on the active cell with `text` (caret at the end), in Enter
    /// mode (AutoSum, Insert Function).
    pub fn edit_with(&mut self, text: &str, m: &dyn TextMeasure) {
        self.begin_edit(text.to_owned(), true, m);
    }

    /// The cells with content in the selection, for clearing: `(row, col, "")`.
    fn filled_cells(&self) -> Vec<(u32, u32, String)> {
        let (r0, c0, r1, c1) = self.range_bounds();
        self.engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
            .map(|ws| {
                (r0..=r1.min(ws.cells.keys().next_back().map_or(0, |k| k.0)))
                    .flat_map(|row| {
                        ws.cells
                            .range((row, c0)..=(row, c1))
                            .map(|(&(r, c), _)| (r, c, String::new()))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clear the selection's contents and / or formats as one undo step.
    pub fn clear(&mut self, contents: bool, formats: bool) -> bool {
        let mut cmds = Vec::new();
        if formats {
            cmds.push(json!({ "cmd": "clearFormats", "sheet": self.sheet, "range": self.rng() }));
        }
        if contents {
            let cells = self.filled_cells();
            if !cells.is_empty() {
                cmds.push(json!({ "cmd": "setInputs", "sheet": self.sheet, "cells": cells }));
            }
        }
        if cmds.is_empty() {
            return false;
        }
        self.run(json!({ "cmd": "batch", "cmds": cmds }))
    }

    /// Active cell (row, column).
    pub fn active(&self) -> (u32, u32) {
        self.active
    }

    /// First row and column shown in the main pane (frozen rows / columns included).
    pub fn top_left(&self) -> (u32, u32) {
        (
            self.rows.first().map_or(0, |r| r.index),
            self.cols.first().map_or(0, |c| c.index),
        )
    }

    /// Size of the selection in points (zoom to selection).
    pub fn selection_size_pt(&mut self) -> Option<(f64, f64)> {
        let vp = self.viewport();
        let (r0, c0, r1, c1) = self.range_bounds();
        let (_, geom) = self.engine.view_layout(self.sheet, &vp).ok()?;
        let w = geom.col_x(c1.min(MAX_COLS - 1) + 1) - geom.col_x(c0);
        let h = geom.row_y(r1.min(MAX_ROWS - 1) + 1) - geom.row_y(r0);
        Some((w, h))
    }

    /// Size of the grid area in points at 100 % zoom.
    pub fn view_size_pt(&self) -> (f64, f64) {
        let vp = self.viewport();
        let z = f64::from(self.zoom);
        (vp.width * z, vp.height * z)
    }

    /// Scroll so that cell (`r`, `c`) is the first one shown in the main pane.
    pub fn scroll_to_cell(&mut self, r: u32, c: u32) {
        let vp = self.viewport();
        let Ok((layout, geom)) = self.engine.view_layout(self.sheet, &vp) else {
            return;
        };
        let (fr, fc) = (layout.frozen_rows, layout.frozen_cols);
        let s = (
            (geom.col_x(c.max(fc)) - geom.col_x(fc)).max(0.0),
            (geom.row_y(r.max(fr)) - geom.row_y(fr)).max(0.0),
        );
        if s != self.scroll {
            self.scroll = s;
            self.grid_dirty = true;
        }
        self.refresh();
    }

    /// Lay out again after the engine changed outside a command (recalculation).
    pub fn reload(&mut self) {
        self.redraw_all();
        self.refresh();
    }

    /// Show formulas in their cells instead of their results (Formulas tab).
    pub fn set_show_formulas(&mut self, on: bool) {
        if self.show_formulas != on {
            self.show_formulas = on;
            self.redraw_all();
        }
    }

    /// Formulas shown in cells.
    pub fn show_formulas(&self) -> bool {
        self.show_formulas
    }

    /// Window rectangle of cell (`r`, `c`) when its row and column are on screen.
    pub(crate) fn cell_rect(&self, r: u32, c: u32) -> Option<[f32; 4]> {
        let row = self.rows.iter().find(|s| s.index == r)?;
        let col = self.cols.iter().find(|s| s.index == c)?;
        Some([
            col.start,
            row.start,
            col.line - col.start,
            row.line - row.start,
        ])
    }

    /// The cell under window point (x, y) when it is over the cells.
    pub(crate) fn cell_under(&self, x: f32, y: f32) -> Option<(u32, u32)> {
        let (gx, gy) = self.grid_origin();
        let [fx, fy, fw, fh] = self.frame;
        (x >= gx && y >= gy && x < fx + fw && y < fy + fh).then(|| self.cell_at(x, y))
    }

    /// The grid's rectangle in the window (cells and headers).
    pub(crate) fn grid_frame(&self) -> [f32; 4] {
        self.frame
    }

    /// Show or hide the gridlines (view setting).
    pub fn set_gridlines(&mut self, on: bool) {
        if self.show_grid != on {
            self.show_grid = on;
            self.redraw_all();
        }
    }

    /// A drawing (chart) is selected instead of cells: the cell selection is not drawn.
    pub fn set_object_selected(&mut self, on: bool) {
        self.object_selected = on;
    }

    /// Gridlines shown.
    pub fn gridlines(&self) -> bool {
        self.show_grid
    }

    /// Show or hide the row and column headings (view setting).
    pub fn set_headings(&mut self, on: bool) {
        if self.show_headings != on {
            self.show_headings = on;
            self.redraw_all();
            self.refresh();
        }
    }

    /// Row and column headings shown.
    pub fn headings(&self) -> bool {
        self.show_headings
    }

    /// Run a typed sheet command; `true` when it ran (the grid is redrawn).
    pub fn exec(&mut self, cmd: SheetCommand) -> bool {
        match self.engine.exec(cmd) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// The workbook changed: redraw the grid and reload the active cell.
    pub fn changed(&mut self) {
        self.modified = true;
        self.reload_breaks();
        self.reload_layout();
        self.redraw_all();
        self.refresh();
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> bool {
        self.edit = None;
        let done = self.engine.undo().unwrap_or(false);
        if done {
            self.changed();
        }
        done
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        let done = self.engine.redo().unwrap_or(false);
        if done {
            self.changed();
        }
        done
    }

    /// Show sheet `index`.
    pub fn set_sheet(&mut self, index: usize) -> bool {
        let n = self.engine.workbook().map_or(0, |w| w.sheets.len());
        if index >= n || index == self.sheet {
            return false;
        }
        self.commit();
        self.sheet = index;
        self.scroll = (0.0, 0.0);
        self.anchor = (0, 0);
        self.extent = (0, 0);
        self.active = (0, 0);
        self.copied = None;
        if let Some(pl) = self.layout.as_mut() {
            pl.scroll = (0.0, 0.0);
        }
        self.reload_breaks();
        self.reload_layout();
        self.redraw_all();
        self.refresh();
        true
    }

    /// Show sheet `index` even when it is the current index (after a sheet was deleted
    /// or hidden).
    pub fn set_sheet_index(&mut self, index: usize) {
        if index == self.sheet {
            self.sheet = usize::MAX;
        }
        if !self.set_sheet(index) {
            self.sheet = index.min(
                self.engine
                    .workbook()
                    .map_or(0, |w| w.sheets.len().saturating_sub(1)),
            );
            self.redraw_all();
            self.refresh();
        }
    }

    /// Sheet names, in tab order (hidden sheets as `None`).
    pub fn sheet_names(&self) -> Vec<Option<String>> {
        self.engine.workbook().map_or_else(Vec::new, |w| {
            w.sheets
                .iter()
                .map(|s| {
                    matches!(s.state, bo_model::sheet::SheetState::Visible).then(|| s.name.clone())
                })
                .collect()
        })
    }

    /// Add a sheet after the others and show it.
    pub fn add_sheet(&mut self) -> bool {
        match self.engine.add_sheet(None) {
            Ok(i) => {
                self.modified = true;
                self.set_sheet(i);
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// Select a range with `active` as the active cell and bring it into view.
    pub fn select(&mut self, anchor: (u32, u32), extent: (u32, u32), active: (u32, u32)) {
        let clamp = |(r, c): (u32, u32)| (r.min(MAX_ROWS - 1), c.min(MAX_COLS - 1));
        self.anchor = clamp(anchor);
        self.extent = clamp(extent);
        self.active = clamp(active);
        self.reveal(self.extent);
        self.refresh();
    }

    /// Scroll so that cell (`r`, `c`) is in the main pane (whole rows / columns, as the
    /// reference does).
    fn reveal(&mut self, (r, c): (u32, u32)) {
        let vp = self.viewport();
        let Ok((layout, geom)) = self.engine.view_layout(self.sheet, &vp) else {
            return;
        };
        let (fr, fc) = (layout.frozen_rows, layout.frozen_cols);
        let Some(main) = layout.panes.iter().find(|p| p.kind == PaneKind::Main) else {
            return;
        };
        let (base_x, base_y) = (geom.col_x(fc), geom.row_y(fr));
        let (mut sx, mut sy) = self.scroll;
        if r >= fr {
            let top = geom.row_y(r) - base_y;
            let bottom = geom.row_y(r + 1) - base_y;
            if top < sy {
                sy = top;
            } else if bottom > sy + main.screen.h {
                // The first row whose top lets the cell's bottom fit.
                let want = bottom - main.screen.h;
                let i = geom.rows.index_at(want + base_y);
                let at = geom.row_y(i) - base_y;
                sy = if at < want {
                    geom.row_y(i + 1) - base_y
                } else {
                    at
                };
            }
        }
        if c >= fc {
            let left = geom.col_x(c) - base_x;
            let right = geom.col_x(c + 1) - base_x;
            if left < sx {
                sx = left;
            } else if right > sx + main.screen.w {
                let want = right - main.screen.w;
                let i = geom.cols.index_at(want + base_x);
                let at = geom.col_x(i) - base_x;
                sx = if at < want {
                    geom.col_x(i + 1) - base_x
                } else {
                    at
                };
                sx = sx.min(left);
            }
        }
        if (sx, sy) != self.scroll {
            self.scroll = (sx.max(0.0), sy.max(0.0));
            self.grid_dirty = true;
        }
    }

    /// Scroll by whole rows (positive: down).
    pub fn scroll_rows(&mut self, rows: i64) -> bool {
        self.scroll_by(rows, 0)
    }

    /// Scroll by whole columns (positive: right).
    pub fn scroll_cols(&mut self, cols: i64) -> bool {
        self.scroll_by(0, cols)
    }

    fn scroll_by(&mut self, rows: i64, cols: i64) -> bool {
        let vp = self.viewport();
        let Ok((layout, geom)) = self.engine.view_layout(self.sheet, &vp) else {
            return false;
        };
        let (fr, fc) = (layout.frozen_rows, layout.frozen_cols);
        let (base_x, base_y) = (geom.col_x(fc), geom.row_y(fr));
        let step = |axis: &bo_sheet::geometry::Axis, base: f64, at: f64, n: i64, first: u32| {
            let mut i = i64::from(axis.index_at(at + base).max(first));
            let mut left = n;
            while left != 0 {
                i = (i + left.signum()).clamp(i64::from(first), i64::from(axis.count()) - 1);
                if axis.size(i as u32) > 0.0 || i == i64::from(first) {
                    left -= left.signum();
                }
                if i == i64::from(first) && left < 0 {
                    break;
                }
            }
            axis.offset(i as u32) - base
        };
        let sy = if rows == 0 {
            self.scroll.1
        } else {
            step(&geom.rows, base_y, self.scroll.1, rows, fr)
        };
        let sx = if cols == 0 {
            self.scroll.0
        } else {
            step(&geom.cols, base_x, self.scroll.0, cols, fc)
        };
        let s = (sx.max(0.0), sy.max(0.0));
        if s == self.scroll {
            return false;
        }
        self.scroll = s;
        self.grid_dirty = true;
        self.refresh_with(false);
        true
    }

    /// Scroll position, scrollable extent and visible size of the main pane, in points
    /// (`vertical` or horizontal), for scroll bars.
    pub fn scroll_info(&mut self, vertical: bool) -> (f64, f64, f64) {
        if self.layout.is_some() {
            return self.layout_scroll_info(vertical);
        }
        let key = BarKey {
            scroll: self.scroll,
            frame: self.frame,
            sheet: self.sheet,
            zoom: self.zoom,
            scale: self.scale,
            content_gen: self.content_gen,
        };
        if let Some((k, v)) = &self.bar_cache {
            if *k == key {
                return v[usize::from(!vertical)];
            }
        }
        let both = [self.scroll_info_now(true), self.scroll_info_now(false)];
        self.bar_cache = Some((key, both));
        both[usize::from(!vertical)]
    }

    fn scroll_info_now(&mut self, vertical: bool) -> (f64, f64, f64) {
        let vp = self.viewport();
        let key = (self.sheet, self.content_gen);
        let (content_w, content_h) = match self.content_cache {
            Some((k, v)) if k == key => v,
            _ => {
                let v = self.engine.content_size(self.sheet).unwrap_or((0.0, 0.0));
                self.content_cache = Some((key, v));
                v
            }
        };
        let Ok((layout, geom)) = self.engine.view_layout(self.sheet, &vp) else {
            return (0.0, 1.0, 1.0);
        };
        let main = layout.panes.iter().find(|p| p.kind == PaneKind::Main);
        let (fr, fc) = (layout.frozen_rows, layout.frozen_cols);
        if vertical {
            let view = main.map_or(1.0, |p| p.screen.h);
            let content = (content_h - geom.row_y(fr)).max(self.scroll.1 + view);
            (self.scroll.1, content, view)
        } else {
            let view = main.map_or(1.0, |p| p.screen.w);
            let content = (content_w - geom.col_x(fc)).max(self.scroll.0 + view);
            (self.scroll.0, content, view)
        }
    }

    /// Scroll the main pane to `pos` points (snapped to a row / column start).
    pub fn scroll_to(&mut self, vertical: bool, pos: f64) -> bool {
        if self.layout.is_some() {
            return self.layout_scroll_to(vertical, pos);
        }
        let vp = self.viewport();
        let Ok((layout, geom)) = self.engine.view_layout(self.sheet, &vp) else {
            return false;
        };
        let (fr, fc) = (layout.frozen_rows, layout.frozen_cols);
        let mut s = self.scroll;
        if vertical {
            let base = geom.row_y(fr);
            let i = geom.rows.index_at(pos.max(0.0) + base).max(fr);
            s.1 = geom.row_y(i) - base;
        } else {
            let base = geom.col_x(fc);
            let i = geom.cols.index_at(pos.max(0.0) + base).max(fc);
            s.0 = geom.col_x(i) - base;
        }
        if s == self.scroll {
            return false;
        }
        self.scroll = s;
        self.grid_dirty = true;
        self.refresh_with(false);
        true
    }

    // ------------------------------------------------------------------ editing

    fn begin_edit(&mut self, text: String, typed: bool, m: &dyn TextMeasure) {
        let caret = text.len();
        self.edit = Some(Editor {
            text,
            caret,
            typed,
            caret_x: 0.0,
            text_w: 0.0,
            preedit: String::new(),
            point: None,
            point_cells: None,
            runs: Vec::new(),
            auto: None,
            auto_entry: String::new(),
            auto_x: 0.0,
        });
        self.measure_edit(m);
    }

    /// Whether a click on a cell now inserts its reference in the formula being typed (the
    /// caret follows an operator, a parenthesis or a separator, or a pointed reference).
    fn pointing(&self) -> bool {
        let Some(e) = self.edit.as_ref() else {
            return false;
        };
        if !e.text.starts_with('=') || !e.preedit.is_empty() {
            return false;
        }
        if e.point.as_ref().is_some_and(|p| p.end == e.caret) {
            return true;
        }
        e.text[..e.caret]
            .trim_end()
            .chars()
            .next_back()
            .is_some_and(|c| "=+-*/^&(,;:<>".contains(c))
    }

    /// Put the reference of cells `a`..`b` at the caret (replacing the pointed one).
    fn point_ref(&mut self, a: (u32, u32), b: (u32, u32), m: &dyn TextMeasure) {
        let r = if a == b {
            CellRef::new(a.0, a.1).to_string()
        } else {
            let (r0, c0, r1, c1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
            format!("{}:{}", CellRef::new(r0, c0), CellRef::new(r1, c1))
        };
        if let Some(e) = self.edit.as_mut() {
            let at = match e.point.take() {
                Some(p) if p.end == e.caret => {
                    e.text.replace_range(p.clone(), "");
                    p.start
                }
                _ => e.caret,
            };
            e.text.insert_str(at, &r);
            e.caret = at + r.len();
            e.point = Some(at..e.caret);
            e.point_cells = Some((a, b));
        }
        self.measure_edit(m);
    }

    /// Arrow key `k` while typing a formula where a reference may go ("Point" mode): the
    /// first arrow points at the cell next to the active one, later arrows move the pointed
    /// cell, Shift extends it to a range (as the reference spreadsheet does).
    fn point_key(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> bool {
        let (dr, dc) = match k {
            Key::Up => (-1, 0),
            Key::Down => (1, 0),
            Key::Left => (0, -1),
            _ => (0, 1),
        };
        let Some(e) = self.edit.as_ref() else {
            return false;
        };
        let (anchor, end) = match (&e.point, e.point_cells) {
            (Some(p), Some(cells)) if p.end == e.caret => cells,
            _ => (self.active, self.active),
        };
        let step =
            |v: u32, d: i64, max: u32| (i64::from(v) + d).clamp(0, i64::from(max) - 1) as u32;
        let next = (step(end.0, dr, MAX_ROWS), step(end.1, dc, MAX_COLS));
        let next = self.skip_hidden(next.0, next.1, dr, dc);
        let anchor = if mods.shift { anchor } else { next };
        self.point_ref(anchor, next, m);
        self.reveal(next);
        true
    }

    /// Mode of the cell editor for the status bar: `"point"` (a reference was just pointed
    /// at), `"enter"` (typing over a cell) or `"edit"` (F2 / double click); `None` when not
    /// editing.
    pub fn edit_mode(&self) -> Option<&'static str> {
        let e = self.edit.as_ref()?;
        Some(if e.point.as_ref().is_some_and(|p| p.end == e.caret) {
            "point"
        } else if e.typed {
            "enter"
        } else {
            "edit"
        })
    }

    /// Reference frames of the formula being edited: colour and window rectangle (the
    /// outer pixels of the gridlines around the cells), for references on screen.
    fn ref_frames(&self) -> Vec<(Rgba, [f32; 4])> {
        let Some(e) = self.edit.as_ref() else {
            return Vec::new();
        };
        let find = |spans: &[Span], i: u32, first: bool| -> Option<f32> {
            spans
                .iter()
                .find(|x| x.index == i)
                .map(|x| if first { x.start - 1.0 } else { x.line })
        };
        let mut out = Vec::new();
        for (n, (_, (r0, c0, r1, c1))) in formula_refs(&e.text).into_iter().enumerate() {
            let color = REF_COLORS[n % REF_COLORS.len()];
            let visible = |spans: &[Span], lo: u32, hi: u32| {
                spans.iter().any(|s| s.index >= lo && s.index <= hi)
            };
            if !visible(&self.cols, c0, c1) || !visible(&self.rows, r0, r1) {
                continue;
            }
            let (gx, gy) = self.grid_origin();
            let l = find(&self.cols, c0, true).unwrap_or(gx - 1.0);
            let t = find(&self.rows, r0, true).unwrap_or(gy - 1.0);
            let r = find(&self.cols, c1, false).unwrap_or(self.frame[0] + self.frame[2]);
            let b = find(&self.rows, r1, false).unwrap_or(self.frame[1] + self.frame[3]);
            out.push((color, [l, t, r, b]));
        }
        out
    }

    /// Start editing the active cell with its content, caret at the end (F2).
    pub fn edit_cell(&mut self, m: &dyn TextMeasure) {
        let t = self.details.1.clone();
        self.begin_edit(t, false, m);
    }

    fn edit_font(&self) -> Font<'_> {
        Font {
            family: &self.format.font,
            size: (self.format.size as f32 * self.k()).max(1.0),
            weight: if self.format.bold { 700 } else { 400 },
            italic: self.format.italic,
            underline: false,
        }
    }

    fn measure_edit(&mut self, m: &dyn TextMeasure) {
        let family = self.format.font.clone();
        let font = Font {
            family: &family,
            ..self.edit_font()
        };
        if let Some(e) = self.edit.as_ref() {
            let shown = format!("{}{}{}", &e.text[..e.caret], e.preedit, &e.text[e.caret..]);
            let before = format!("{}{}", &e.text[..e.caret], e.preedit);
            let caret_x = m.measure_font(&before, &font).0;
            let text_w = m.measure_font(&shown, &font).0;
            // A formula's references in their frame colours (not while composing).
            let mut runs = Vec::new();
            if e.preedit.is_empty() {
                let mut at = 0;
                for (n, (range, _)) in formula_refs(&e.text).into_iter().enumerate() {
                    if range.start > at {
                        let x = m.measure_font(&e.text[..at], &font).0;
                        runs.push((x, at..range.start, None));
                    }
                    let x = m.measure_font(&e.text[..range.start], &font).0;
                    at = range.end;
                    runs.push((x, range, Some(REF_COLORS[n % REF_COLORS.len()])));
                }
                if !runs.is_empty() && at < e.text.len() {
                    let x = m.measure_font(&e.text[..at], &font).0;
                    runs.push((x, at..e.text.len(), None));
                }
            }
            let auto_x = e
                .auto
                .and_then(|a| e.text.get(..a))
                .map_or(0.0, |s| m.measure_font(s, &font).0);
            if let Some(e) = self.edit.as_mut() {
                e.caret_x = caret_x;
                e.text_w = text_w;
                e.runs = runs;
                e.auto_x = auto_x;
            }
        }
    }

    /// AutoComplete as the reference: when the typed text (not a formula or a number) starts
    /// exactly one longer text entry of the column's block of data, the rest of that entry is
    /// suggested after the caret.
    fn autocomplete(&mut self, m: &dyn TextMeasure) {
        let Some(e) = self.edit.as_ref() else { return };
        if !e.typed
            || e.caret != e.text.len()
            || e.text.is_empty()
            || e.text.starts_with('=')
            || e.text.contains('\n')
            || e.text.trim().parse::<f64>().is_ok()
        {
            return;
        }
        let typed = e.text.to_lowercase();
        let (row, col) = self.active;
        let Some(ws) = self
            .engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
        else {
            return;
        };
        let text_at = |r: u32| -> Option<String> {
            let c = ws.cell(r, col)?;
            match &c.value {
                bo_model::sheet::CellValue::Text(s) => Some(s.clone()),
                bo_model::sheet::CellValue::Empty => None,
                other => Some(other.as_text().unwrap_or_default()),
            }
        };
        // The column's contiguous entries above and below the active cell.
        let mut entries = Vec::new();
        let mut r = row;
        while r > 0 {
            r -= 1;
            match text_at(r) {
                Some(t) => entries.push(t),
                None => break,
            }
        }
        let mut r = row + 1;
        while let Some(t) = text_at(r) {
            entries.push(t);
            r += 1;
            if entries.len() > 10_000 {
                break;
            }
        }
        let mut found: Option<String> = None;
        for t in entries {
            let low = t.to_lowercase();
            if low.len() > typed.len() && low.starts_with(&typed) {
                match &found {
                    Some(f) if f.to_lowercase() != low => return,
                    Some(_) => {}
                    None => found = Some(t),
                }
            }
        }
        let Some(full) = found else { return };
        // The typed text, then the rest of the entry (accepted in the entry's letter case).
        let Some(rest) = full
            .char_indices()
            .nth(e.text.chars().count())
            .map(|(i, _)| full[i..].to_owned())
        else {
            return;
        };
        if let Some(e) = self.edit.as_mut() {
            let a = e.text.len();
            e.text.push_str(&rest);
            e.auto = Some(a);
            e.auto_entry = full;
            e.caret = a;
        }
        self.measure_edit(m);
    }

    /// Commit the editor's text to the active cell; `true` when something was written.
    pub fn commit(&mut self) -> bool {
        let Some(mut e) = self.edit.take() else {
            return false;
        };
        // An accepted AutoComplete suggestion enters the entry as the column has it.
        if e.auto.is_some() && !e.auto_entry.is_empty() {
            e.text = std::mem::take(&mut e.auto_entry);
        }
        let (r, c) = self.active;
        if e.text == self.details.1 && !e.typed {
            return false;
        }
        match self.engine.set_input(self.sheet, r, c, &e.text) {
            Ok(()) => {
                self.changed();
                true
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.refresh();
                false
            }
        }
    }

    /// Drop the editor's text.
    pub fn cancel_edit(&mut self) -> bool {
        self.edit.take().is_some()
    }

    /// Insert text in the editor (the formula bar types here too).
    pub fn edit_insert(&mut self, s: &str, m: &dyn TextMeasure) {
        if let Some(e) = self.edit.as_mut() {
            e.text.insert_str(e.caret, s);
            e.caret += s.len();
            e.point = None;
        }
        self.measure_edit(m);
    }

    /// Clear the contents (not formats) of the selection, as one undo step.
    pub fn clear_contents(&mut self) -> bool {
        let (r0, c0, r1, c1) = self.range_bounds();
        let cells: Vec<(u32, u32, String)> = self
            .engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
            .map(|ws| {
                (r0..=r1.min(ws.cells.keys().next_back().map_or(0, |k| k.0)))
                    .flat_map(|row| {
                        ws.cells
                            .range((row, c0)..=(row, c1))
                            .map(|(&(r, c), _)| (r, c, String::new()))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .unwrap_or_default();
        if cells.is_empty() {
            return false;
        }
        match self.engine.set_inputs(self.sheet, &cells) {
            Ok(()) => {
                self.changed();
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// Copy (or cut) the selection: engine clipboard plus the text form for other
    /// applications (tab-separated displayed values), returned.
    pub fn copy(&mut self, cut: bool) -> String {
        let (r0, c0, r1, c1) = self.range_bounds();
        let _ = self.run(json!({ "cmd": "copy", "range": self.rng(), "cut": cut }));
        self.copied = Some((self.sheet, self.range()));
        let (lr, lc) = self
            .engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
            .map_or((0, 0), |ws| {
                (
                    ws.cells.keys().next_back().map_or(0, |k| k.0),
                    ws.cells.keys().map(|k| k.1).max().unwrap_or(0),
                )
            });
        let mut out = String::new();
        for r in r0..=r1.min(lr.max(r0)) {
            let line: Vec<String> = (c0..=c1.min(lc.max(c0)))
                .map(|c| {
                    self.engine
                        .cell(self.sheet, r, c)
                        .map(|d| d.display)
                        .unwrap_or_default()
                })
                .collect();
            out.push_str(&line.join("\t"));
            out.push_str("\r\n");
        }
        // Copying is not a change of the workbook; cutting is.
        if cut {
            self.modified = true;
        }
        self.clip_text.clone_from(&out);
        out
    }

    /// Paste: the engine's own copy when the system clipboard still holds its text, else the
    /// clipboard text (tab-separated rows) as typed input from the active cell.
    pub fn paste(&mut self, text: Option<String>) -> bool {
        let (r, c) = self.active;
        let own = self.copied.is_some()
            && text
                .as_deref()
                .is_none_or(|t| t.replace("\r\n", "\n") == self.clip_text.replace("\r\n", "\n"));
        if own {
            let ok = self.run(json!({ "cmd": "paste", "row": r, "col": c, "range": self.rng() }));
            if ok {
                // A cut moves once.
                if self.engine.clipboard().is_none() {
                    self.copied = None;
                }
            }
            return ok;
        }
        let Some(t) = text else { return false };
        let t = t.replace("\r\n", "\n");
        let t = t.strip_suffix('\n').unwrap_or(&t);
        let mut cells = Vec::new();
        let mut rows = 0;
        let mut cols = 0;
        for (i, line) in t.split('\n').enumerate() {
            for (j, v) in line.split('\t').enumerate() {
                cells.push((r + i as u32, c + j as u32, v.to_owned()));
                cols = cols.max(j as u32 + 1);
            }
            rows = i as u32 + 1;
        }
        if cells.is_empty() {
            return false;
        }
        match self.engine.set_inputs(self.sheet, &cells) {
            Ok(()) => {
                self.anchor = (r, c);
                self.extent = (r + rows.max(1) - 1, c + cols.max(1) - 1);
                self.changed();
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    // ------------------------------------------------------------------ input

    fn move_active(&mut self, dr: i64, dc: i64, extend: bool) {
        let (r, c) = if extend { self.extent } else { self.active };
        let nr = (i64::from(r) + dr).clamp(0, i64::from(MAX_ROWS) - 1) as u32;
        let nc = (i64::from(c) + dc).clamp(0, i64::from(MAX_COLS) - 1) as u32;
        // Skip hidden rows / columns.
        let (nr, nc) = self.skip_hidden(nr, nc, dr.signum(), dc.signum());
        if extend {
            self.select(self.anchor, (nr, nc), self.active);
        } else {
            self.select((nr, nc), (nr, nc), (nr, nc));
        }
    }

    fn skip_hidden(&mut self, mut r: u32, mut c: u32, dr: i64, dc: i64) -> (u32, u32) {
        let vp = self.viewport();
        if let Ok((_, geom)) = self.engine.view_layout(self.sheet, &vp) {
            while dr != 0 && geom.row_height(r) <= 0.0 && r > 0 && r < MAX_ROWS - 1 {
                r = (i64::from(r) + dr) as u32;
            }
            while dc != 0 && geom.col_width(c) <= 0.0 && c > 0 && c < MAX_COLS - 1 {
                c = (i64::from(c) + dc) as u32;
            }
        }
        (r, c)
    }

    /// Ctrl+arrow: to the edge of the data block (the engine's rule).
    fn jump(&mut self, dir: &str, extend: bool) {
        let (r, c) = if extend { self.extent } else { self.active };
        let v = self
            .engine
            .exec(SheetCommand::DataEdge {
                sheet: self.sheet,
                row: r,
                col: c,
                dir: dir.to_owned(),
            })
            .unwrap_or(Value::Null);
        let to = (
            v.get("row").and_then(Value::as_u64).unwrap_or(u64::from(r)) as u32,
            v.get("col").and_then(Value::as_u64).unwrap_or(u64::from(c)) as u32,
        );
        if extend {
            self.select(self.anchor, to, self.active);
        } else {
            self.select(to, to, to);
        }
    }

    /// Rows of the main pane that are fully visible (Page Up / Page Down step).
    fn page_rows(&self) -> i64 {
        let n = self
            .rows
            .iter()
            .filter(|r| r.line < self.frame[1] + self.frame[3])
            .count();
        (n as i64 - 1).max(1)
    }

    fn last_used(&self) -> (u32, u32) {
        self.engine
            .workbook()
            .and_then(|w| w.sheets.get(self.sheet))
            .map_or((0, 0), |ws| {
                (
                    ws.cells.keys().next_back().map_or(0, |k| k.0),
                    ws.cells.keys().map(|k| k.1).max().unwrap_or(0),
                )
            })
    }

    /// Key while the editor is open; `None` lets the key through (commit + navigate).
    fn edit_key(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> Option<bool> {
        let typed = self.edit.as_ref()?.typed;
        let pointing = self.pointing();
        let e = self.edit.as_mut()?;
        // Backspace removes an AutoComplete suggestion; Enter / Tab accept it (commit);
        // other keys keep it as typed text.
        if let Some(a) = e.auto {
            if k == Key::Backspace {
                e.auto = None;
                e.text.truncate(a);
                e.caret = a;
                self.measure_edit(m);
                return Some(true);
            }
            if !matches!(k, Key::Enter | Key::Tab) {
                e.auto = None;
                e.caret = e.text.len();
            }
        }
        match k {
            Key::Escape => {
                self.edit = None;
                Some(true)
            }
            Key::Backspace => {
                if e.caret > 0 {
                    let p = prev_char(&e.text, e.caret);
                    e.text.replace_range(p..e.caret, "");
                    e.caret = p;
                }
                self.measure_edit(m);
                Some(true)
            }
            Key::Delete => {
                if e.caret < e.text.len() {
                    let n = next_char(&e.text, e.caret);
                    e.text.replace_range(e.caret..n, "");
                }
                self.measure_edit(m);
                Some(true)
            }
            Key::Left | Key::Right if !typed => {
                e.caret = if k == Key::Left {
                    prev_char(&e.text, e.caret)
                } else {
                    next_char(&e.text, e.caret)
                };
                self.measure_edit(m);
                Some(true)
            }
            Key::Home if !typed || !mods.ctrl => {
                e.caret = 0;
                self.measure_edit(m);
                Some(true)
            }
            Key::End if !typed || !mods.ctrl => {
                e.caret = e.text.len();
                self.measure_edit(m);
                Some(true)
            }
            Key::Up | Key::Down if !typed => Some(false),
            Key::Up | Key::Down | Key::Left | Key::Right if typed && !mods.ctrl && pointing => {
                Some(self.point_key(k, mods, m))
            }
            // F4 cycles the reference at the caret: $A$1 → A$1 → $A1 → A1 (the reference's
            // order); the caret goes after it.
            Key::F(4) if e.text.starts_with('=') => {
                if let Some((text, caret)) = cycle_reference(&e.text, e.caret) {
                    e.text = text;
                    e.caret = caret;
                    self.measure_edit(m);
                }
                Some(true)
            }
            Key::Enter if mods.alt => {
                e.text.insert(e.caret, '\n');
                e.caret += 1;
                self.measure_edit(m);
                Some(true)
            }
            Key::Letter(_) if mods.ctrl => Some(false),
            _ => None,
        }
    }

    /// Handle an input event over the grid; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        match input {
            Input::Text(s) => {
                if self.edit.is_none() {
                    self.begin_edit(String::new(), true, m);
                }
                if let Some(e) = self.edit.as_mut() {
                    e.preedit.clear();
                    // Typing replaces the suggested rest of an entry.
                    if let Some(a) = e.auto.take() {
                        e.text.truncate(a);
                        e.caret = a;
                    }
                }
                self.edit_insert(&s, m);
                self.autocomplete(m);
                true
            }
            Input::Preedit(s) => {
                if self.edit.is_none() {
                    if s.is_empty() {
                        return false;
                    }
                    self.begin_edit(String::new(), true, m);
                }
                if let Some(e) = self.edit.as_mut() {
                    e.preedit = s;
                }
                self.measure_edit(m);
                true
            }
            Input::Key(k, mods) => {
                let done = self.key(k, mods, m);
                if self.layout.is_some() {
                    self.layout_reveal();
                }
                done
            }
            Input::Wheel(px) if self.layout.is_some() => {
                let (pos, _, _) = self.layout_scroll_info(true);
                self.layout_scroll_to(true, pos + f64::from(px / self.k()))
            }
            Input::Press { x, y, shift, .. } if self.layout.is_some() => {
                self.layout_press(x, y, shift)
            }
            Input::Drag { .. } if self.layout.is_some() => false,
            Input::Press {
                x,
                y,
                count: 1,
                shift: false,
            } if self.breaks.is_some() && self.edit.is_none() => {
                if let Some((a, rows, at)) = self.break_at(x, y) {
                    self.drag = Some(Drag::Break(a, rows, at));
                    true
                } else {
                    self.press(x, y, 1, false, m)
                }
            }
            Input::Wheel(px) => {
                // Three rows per notch (the host sends ~20 logical px per wheel line).
                let lines = (px / (20.0 * self.scale)).round() as i64;
                let n = if lines == 0 {
                    px.signum() as i64
                } else {
                    lines
                };
                self.scroll_rows(n)
            }
            Input::Press { x, y, count, shift } => self.press(x, y, count, shift, m),
            Input::Drag { x, y } => self.drag_to(x, y, m),
            Input::Release { x, y } => {
                let d = self.drag.take();
                match d {
                    Some(Drag::ColEdge(c, origin)) => self.resize_edge(false, c, origin, x),
                    Some(Drag::RowEdge(r, origin)) => self.resize_edge(true, r, origin, y),
                    Some(Drag::Fill(r0, c0, r1, c1)) => self.finish_fill((r0, c0, r1, c1)),
                    Some(Drag::Move(..)) => self.finish_move(),
                    Some(Drag::Break(a, rows, at)) => self.drop_break(a, rows, at, x, y),
                    Some(Drag::Point(..)) => {
                        self.measure_edit(m);
                        true
                    }
                    Some(_) => {
                        self.load_details();
                        true
                    }
                    None => false,
                }
            }
            Input::Scale(s) => {
                self.set_scale(s);
                true
            }
            _ => false,
        }
    }

    fn key(&mut self, k: Key, mods: Mods, m: &dyn TextMeasure) -> bool {
        if self.edit.is_some() {
            if let Some(r) = self.edit_key(k, mods, m) {
                return r;
            }
            // Keys that end editing: commit, then move as without the editor.
            if matches!(
                k,
                Key::Enter | Key::Tab | Key::Up | Key::Down | Key::Left | Key::Right
            ) {
                self.commit();
            } else {
                return false;
            }
        }
        let ext = mods.shift;
        match k {
            Key::Up if mods.ctrl => self.jump("up", ext),
            Key::Down if mods.ctrl => self.jump("down", ext),
            Key::Left if mods.ctrl => self.jump("left", ext),
            Key::Right if mods.ctrl => self.jump("right", ext),
            Key::Up => self.move_active(-1, 0, ext),
            Key::Down => self.move_active(1, 0, ext),
            Key::Left => self.move_active(0, -1, ext),
            Key::Right => self.move_active(0, 1, ext),
            Key::Enter | Key::Tab => {
                let (dr, dc) = match (k, mods.shift) {
                    (Key::Enter, false) => (1, 0),
                    (Key::Enter, true) => (-1, 0),
                    (_, false) => (0, 1),
                    (_, true) => (0, -1),
                };
                let (r0, c0, r1, c1) = self.range_bounds();
                if (r0, c0) == (r1, c1) {
                    self.move_active(dr, dc, false);
                } else {
                    // Inside a selected range the active cell cycles through it.
                    let (mut r, mut c) = self.active;
                    if dr == 0 {
                        c = (i64::from(c) + dc) as u32;
                        if c > c1 || c < c0 || c == u32::MAX {
                            c = if dc > 0 { c0 } else { c1 };
                            r = if r >= r1 { r0 } else { r + 1 };
                        }
                    } else {
                        r = (i64::from(r) + dr) as u32;
                        if r > r1 || (dr < 0 && r < r0) || r == u32::MAX {
                            r = if dr > 0 { r0 } else { r1 };
                            c = if c >= c1 { c0 } else { c + 1 };
                        }
                    }
                    self.active = (r, c);
                    self.refresh();
                }
            }
            Key::Home if mods.ctrl => self.select((0, 0), (0, 0), (0, 0)),
            Key::Home => {
                let r = self.active.0;
                self.select((r, 0), (r, 0), (r, 0));
            }
            Key::End if mods.ctrl => {
                let to = self.last_used();
                self.select(to, to, to);
            }
            Key::PageDown | Key::PageUp => {
                let n = self.page_rows() * if k == Key::PageDown { 1 } else { -1 };
                self.scroll_rows(n);
                self.move_active(n, 0, ext);
            }
            Key::Delete => return self.clear_contents(),
            Key::Backspace => {
                self.begin_edit(String::new(), true, m);
            }
            Key::F(2) => self.edit_cell(m),
            Key::Letter('a') if mods.ctrl => {
                self.select((0, 0), (MAX_ROWS - 1, MAX_COLS - 1), self.active);
            }
            Key::Letter('z') if mods.ctrl => return self.undo(),
            Key::Letter('y') if mods.ctrl => return self.redo(),
            Key::Escape => {
                let had = self.copied.take().is_some();
                return had;
            }
            _ => return false,
        }
        true
    }

    /// Cell, column or row under a window point (clamped to the visible grid).
    fn cell_at(&self, x: f32, y: f32) -> (u32, u32) {
        let pick = |spans: &[Span], v: f32| -> u32 {
            spans
                .iter()
                .find(|s| v <= s.line)
                .or(spans.last())
                .map_or(0, |s| s.index)
        };
        let r = pick(&self.rows, y);
        let c = pick(&self.cols, x);
        (r, c)
    }

    /// Edge of a column / row header within 3 px of the point: (column or row, edge pixel).
    fn header_edge(&self, x: f32, y: f32) -> Option<(bool, u32, f32)> {
        let (gx, gy) = self.grid_origin();
        let near = (3.0 * self.scale).round();
        if y >= self.frame[1] && y < gy && x >= gx {
            return self
                .cols
                .iter()
                .find(|c| (x - c.line).abs() <= near)
                .map(|c| (false, c.index, c.line));
        }
        if x >= self.frame[0] && x < gx && y >= gy {
            return self
                .rows
                .iter()
                .find(|r| (y - r.line).abs() <= near)
                .map(|r| (true, r.index, r.line));
        }
        None
    }

    /// Whether a window point is on the fill handle of the selection.
    fn on_fill_handle(&self, x: f32, y: f32) -> bool {
        let Some([_, _, r, b]) = self.sel_rect else {
            return false;
        };
        let (r0, c0, r1, c1) = self.range_bounds();
        if (r0 == 0 && r1 == MAX_ROWS - 1) || (c0 == 0 && c1 == MAX_COLS - 1) {
            return false;
        }
        let near = (4.0 * self.scale).round();
        (x - r).abs() <= near && (y - b).abs() <= near
    }

    /// Within a few pixels of the selection's border (not its fill handle).
    fn on_sel_border(&self, x: f32, y: f32) -> bool {
        let Some([l, t, r, b]) = self.sel_rect else {
            return false;
        };
        if self.object_selected || self.on_fill_handle(x, y) || self.whole_lines().is_some() {
            return false;
        }
        let near = (3.0 * self.scale / 1.25).round().max(2.0);
        let inside_x = x >= l - near && x <= r + near;
        let inside_y = y >= t - near && y <= b + near;
        let on_v = inside_y && ((x - l).abs() <= near || (x - r).abs() <= near);
        let on_h = inside_x && ((y - t).abs() <= near || (y - b).abs() <= near);
        on_v || on_h
    }

    /// Drag-and-drop of the selection ended: move the cells to the grey frame (cut and
    /// paste: references to them follow).
    fn finish_move(&mut self) -> bool {
        let Some(to) = self.fill_to.take() else {
            return false;
        };
        let from = self.range_bounds();
        if to == from {
            return true;
        }
        let rng = self.rng();
        if !self.run(json!({ "cmd": "copy", "range": rng, "cut": true })) {
            return false;
        }
        let ok = self.run(json!({ "cmd": "paste", "row": to.0, "col": to.1 }));
        if ok {
            let off = (
                i64::from(to.0) - i64::from(from.0),
                i64::from(to.1) - i64::from(from.1),
            );
            let mv = |v: u32, d: i64| (i64::from(v) + d).max(0) as u32;
            self.anchor = (to.0, to.1);
            self.extent = (to.2, to.3);
            self.active = (mv(self.active.0, off.0), mv(self.active.1, off.1));
            self.copied = None;
            self.refresh();
        }
        ok
    }

    /// Double click on the fill handle: AutoFill down to the last row of the data in the
    /// column left of the selection (else right of it), as the reference does.
    fn fill_to_neighbour(&mut self) -> bool {
        let (r0, c0, r1, c1) = self.range_bounds();
        let end = {
            let Some(ws) = self
                .engine
                .workbook()
                .and_then(|w| w.sheets.get(self.sheet))
            else {
                return false;
            };
            let filled = |r: u32, c: u32| {
                ws.cell(r, c)
                    .is_some_and(|x| !matches!(x.value, bo_model::sheet::CellValue::Empty))
            };
            let end_of = |c: u32| {
                let mut r = r1;
                while r + 1 < MAX_ROWS && filled(r + 1, c) {
                    r += 1;
                }
                r
            };
            if c0 > 0 && filled(r1 + 1, c0 - 1) {
                end_of(c0 - 1)
            } else if c1 + 1 < MAX_COLS && filled(r1 + 1, c1 + 1) {
                end_of(c1 + 1)
            } else {
                r1
            }
        };
        if end == r1 {
            return false;
        }
        self.fill_to = Some((r0, c0, end, c1));
        self.finish_fill((r0, c0, r1, c1))
    }

    /// The fill handle drag ended over `to`: AutoFill the selection into it.
    fn finish_fill(&mut self, from: (u32, u32, u32, u32)) -> bool {
        let Some(to) = self.fill_to.take() else {
            return false;
        };
        if to == from {
            return true;
        }
        let rng = |(r0, c0, r1, c1): (u32, u32, u32, u32)| json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 });
        // Ctrl while dragging toggles between copying and a series, as the reference.
        let mode = if self.mouse_ctrl { "toggle" } else { "auto" };
        let ok = self.run(
            json!({ "cmd": "autofill", "source": rng(from), "target": rng(to), "mode": mode }),
        );
        if ok {
            self.anchor = (to.0, to.1);
            self.extent = (to.2, to.3);
            self.refresh();
        }
        ok
    }

    /// The cell under a window point, selected unless it is already in the selection (a
    /// right click before the context menu); `false` outside the cells.
    pub fn context_at(&mut self, x: f32, y: f32) -> Option<bool> {
        let (gx, gy) = self.grid_origin();
        let [fx, fy, fw, fh] = self.frame;
        if x < fx || y < fy || x >= fx + fw || y >= fy + fh {
            return None;
        }
        self.commit();
        let (r, c) = self.cell_at(x.max(gx), y.max(gy));
        let (r0, c0, r1, c1) = self.range_bounds();
        let header_col = y < gy && x >= gx;
        let header_row = x < gx && y >= gy;
        if header_col {
            if !(c >= c0 && c <= c1 && r0 == 0 && r1 == MAX_ROWS - 1) {
                self.anchor = (0, c);
                self.extent = (MAX_ROWS - 1, c);
                self.active = (self.rows.first().map_or(0, |s| s.index), c);
                self.refresh();
            }
            return Some(false);
        }
        if header_row {
            if !(r >= r0 && r <= r1 && c0 == 0 && c1 == MAX_COLS - 1) {
                self.anchor = (r, 0);
                self.extent = (r, MAX_COLS - 1);
                self.active = (r, self.cols.first().map_or(0, |s| s.index));
                self.refresh();
            }
            return Some(true);
        }
        if !(r >= r0 && r <= r1 && c >= c0 && c <= c1) {
            self.select((r, c), (r, c), (r, c));
        }
        Some(true)
    }

    /// Whole rows or whole columns are selected: `Some(true)` rows, `Some(false)` columns.
    pub fn whole_lines(&self) -> Option<bool> {
        let (r0, c0, r1, c1) = self.range_bounds();
        if c0 == 0 && c1 == MAX_COLS - 1 {
            Some(true)
        } else if r0 == 0 && r1 == MAX_ROWS - 1 {
            Some(false)
        } else {
            None
        }
    }

    /// Pointer cursor kind at a window point: `0` cell cross, `1` column resize, `2` row resize,
    /// `3` arrow (headers).
    pub fn cursor_at(&self, x: f32, y: f32) -> u8 {
        match self.header_edge(x, y) {
            Some((false, ..)) => 1,
            Some((true, ..)) => 2,
            // The selection's border moves it (four-way arrow).
            None if self.edit.is_none() && self.on_sel_border(x, y) => 6,
            None => {
                let (gx, gy) = self.grid_origin();
                u8::from(x < gx || y < gy) * 3
            }
        }
    }

    fn press(&mut self, x: f32, y: f32, count: u8, shift: bool, m: &dyn TextMeasure) -> bool {
        let (gx, gy) = self.grid_origin();
        let [fx, fy, fw, fh] = self.frame;
        if x < fx || y < fy || x >= fx + fw || y >= fy + fh {
            return false;
        }
        // Pointing at a cell while typing a formula inserts its reference.
        if x >= gx && y >= gy && self.pointing() {
            let at = self.cell_at(x, y);
            self.point_ref(at, at, m);
            self.drag = Some(Drag::Point(at.0, at.1));
            return true;
        }
        if self.edit.is_some() {
            self.commit();
        }
        // The fill handle drags a series / copy of the selection; a double click fills down
        // as far as the column beside it has data.
        if !shift && self.on_fill_handle(x, y) {
            let (r0, c0, r1, c1) = self.range_bounds();
            if count >= 2 {
                self.drag = None;
                return self.fill_to_neighbour();
            }
            self.drag = Some(Drag::Fill(r0, c0, r1, c1));
            self.fill_to = Some((r0, c0, r1, c1));
            return true;
        }
        if let Some((rows, index, line)) = self.header_edge(x, y) {
            // A double click on the edge fits the column / row to its contents.
            if count >= 2 {
                self.drag = None;
                return self.run(json!({
                    "cmd": "autofit", "rows": rows, "first": index, "last": index,
                }));
            }
            self.drag = Some(if rows {
                Drag::RowEdge(index, line)
            } else {
                Drag::ColEdge(index, line)
            });
            return true;
        }
        // The selection's border drags the cells somewhere else.
        if !shift && self.on_sel_border(x, y) {
            let (r0, c0, r1, c1) = self.range_bounds();
            let at = self.cell_at(x.max(gx), y.max(gy));
            self.drag = Some(Drag::Move(at.0, at.1));
            self.fill_to = Some((r0, c0, r1, c1));
            return true;
        }
        let (r, c) = self.cell_at(x, y);
        if x < gx && y < gy {
            self.select((0, 0), (MAX_ROWS - 1, MAX_COLS - 1), self.active);
            return true;
        }
        if y < gy {
            self.drag = Some(Drag::Cols);
            let first = self.rows.first().map_or(0, |s| s.index);
            if shift {
                let a = self.anchor;
                self.anchor = (0, a.1);
                self.extent = (MAX_ROWS - 1, c);
            } else {
                self.anchor = (0, c);
                self.extent = (MAX_ROWS - 1, c);
                self.active = (first, c);
            }
            self.refresh();
            return true;
        }
        if x < gx {
            self.drag = Some(Drag::Rows);
            let first = self.cols.first().map_or(0, |s| s.index);
            if shift {
                let a = self.anchor;
                self.anchor = (a.0, 0);
                self.extent = (r, MAX_COLS - 1);
            } else {
                self.anchor = (r, 0);
                self.extent = (r, MAX_COLS - 1);
                self.active = (r, first);
            }
            self.refresh();
            return true;
        }
        self.drag = Some(Drag::Cells);
        if shift {
            self.extent = (r, c);
        } else {
            self.anchor = (r, c);
            self.extent = (r, c);
            self.active = (r, c);
        }
        self.refresh();
        if count >= 2 && !shift {
            self.drag = None;
            self.edit_cell(m);
        }
        true
    }

    fn drag_to(&mut self, x: f32, y: f32, m: &dyn TextMeasure) -> bool {
        let Some(d) = self.drag else { return false };
        let (gx, gy) = self.grid_origin();
        // Past the edge: scroll one row / column per move.
        let below = y > self.content_end.1;
        let right = x > self.content_end.0;
        let mut scrolled = false;
        if matches!(d, Drag::Cells | Drag::Rows) && (below || y < gy) {
            scrolled |= self.scroll_by(if below { 1 } else { -1 }, 0);
        }
        if matches!(d, Drag::Cells | Drag::Cols) && (right || x < gx) {
            scrolled |= self.scroll_by(0, if right { 1 } else { -1 });
        }
        let (r, c) = self.cell_at(x.max(gx), y.max(gy));
        let before = self.extent;
        match d {
            Drag::Cells => self.extent = (r, c),
            Drag::Cols => self.extent = (MAX_ROWS - 1, c),
            Drag::Rows => self.extent = (r, MAX_COLS - 1),
            Drag::ColEdge(..) | Drag::RowEdge(..) | Drag::Break(..) => return true,
            Drag::Fill(r0, c0, r1, c1) => {
                // Along the axis the pointer left the selection by the most cells.
                let down = r.saturating_sub(r1);
                let up = r0.saturating_sub(r);
                let right = c.saturating_sub(c1);
                let left = c0.saturating_sub(c);
                let rows = down.max(up);
                let cols = right.max(left);
                let to = if rows == 0 && cols == 0 {
                    (r0, c0, r1, c1)
                } else if rows >= cols {
                    if down >= up {
                        (r0, c0, r, c1)
                    } else {
                        (r, c0, r1, c1)
                    }
                } else if right >= left {
                    (r0, c0, r1, c)
                } else {
                    (r0, c, r1, c1)
                };
                let changed = self.fill_to != Some(to);
                self.fill_to = Some(to);
                return changed || scrolled;
            }
            Drag::Move(gr, gc) => {
                let (r0, c0, r1, c1) = self.range_bounds();
                let dr = (i64::from(r) - i64::from(gr)).max(-i64::from(r0));
                let dc = (i64::from(c) - i64::from(gc)).max(-i64::from(c0));
                let mv = |v: u32, d: i64| (i64::from(v) + d) as u32;
                let to = (mv(r0, dr), mv(c0, dc), mv(r1, dr), mv(c1, dc));
                let changed = self.fill_to != Some(to);
                self.fill_to = Some(to);
                return changed || scrolled;
            }
            Drag::Point(ar, ac) => {
                let before = self.edit.as_ref().map(|e| e.text.clone());
                self.point_ref((ar, ac), (r, c), m);
                return before != self.edit.as_ref().map(|e| e.text.clone()) || scrolled;
            }
        }
        if before != self.extent || scrolled {
            self.refresh();
            return true;
        }
        false
    }

    fn resize_edge(&mut self, rows: bool, index: u32, origin: f32, to: f32) -> bool {
        let start = if rows {
            self.rows.iter().find(|s| s.index == index).map(|s| s.start)
        } else {
            self.cols.iter().find(|s| s.index == index).map(|s| s.start)
        };
        let Some(start) = start else { return false };
        if (to - origin).abs() < 1.0 {
            return false;
        }
        let size = (f64::from(to - start + 1.0) / f64::from(self.k())).max(0.0);
        self.run(json!({
            "cmd": "resize", "rows": rows, "first": index, "last": index, "size": size,
        }))
    }

    // ------------------------------------------------------------------ drawing

    /// The whole grid image must be drawn again.
    fn redraw_all(&mut self) {
        self.grid_dirty = true;
        self.full_redraw = true;
        self.content_gen += 1;
    }

    /// Redraw the grid image when the cells, scroll or size changed. A pure scroll by whole
    /// pixels in one direction keeps the still-visible part and draws only the newly exposed
    /// rows / columns ([GridStore::scroll]).
    pub fn prepare(&mut self, store: &mut dyn GridStore) -> Result<(), String> {
        if !self.grid_dirty {
            return Ok(());
        }
        if self.layout.is_some() {
            let list = self.layout_list()?;
            store.update(&list, &self.engine, self.k())?;
            self.drawn = None;
            self.grid_dirty = false;
            self.full_redraw = false;
            return Ok(());
        }
        let outside = self.breaks.as_ref().map(|areas| {
            (
                areas.iter().map(|a| a.range).collect::<Vec<_>>(),
                pages_impl::OUTSIDE_GREY,
            )
        });
        let vp = self.viewport();
        let show_grid = self.show_grid;
        let show_formulas = self.show_formulas;
        let k = self.k();
        // A vertical scroll that turns the ring: lay out only the exposed rows (with two
        // pixels around them for the lines on their edges).
        if self.band_lists && !self.full_redraw {
            if let Some((_, bands, Some(shift))) = self
                .drawn
                .and_then(|from| self.blit_plan(from, (vp.width as f32, vp.height as f32), k))
            {
                if shift[0] == 0 && !bands.is_empty() {
                    let w = (vp.width as f32 * k).ceil() as u32;
                    let h = (vp.height as f32 * k).ceil() as u32;
                    let mut lists = Vec::new();
                    for b in &bands {
                        let y0 = b[1].saturating_sub(2);
                        let y1 = (b[1] + b[3] + 2).min(h);
                        let band_vp = Viewport {
                            scroll_y: vp.scroll_y + f64::from(y0) / f64::from(k),
                            height: f64::from(y1 - y0) / f64::from(k),
                            ..vp
                        };
                        let list = self
                            .engine
                            .render_viewport_with(self.sheet, &band_vp, |o| {
                                o.headers = Some(false);
                                o.gridline_color = GRID_LINE;
                                o.outside.clone_from(&outside);
                                if !show_grid {
                                    o.gridlines = Some(false);
                                }
                                if show_formulas {
                                    o.show_formulas = Some(true);
                                }
                            })
                            .map_err(|e| e.to_string())?;
                        lists.push(GridBand {
                            rect: *b,
                            origin: [0, y0],
                            list,
                        });
                    }
                    if store.turn_bands(&self.engine, k, shift, &lists, (w, h))? {
                        self.drawn = Some(self.scroll);
                        self.grid_dirty = false;
                        return Ok(());
                    }
                }
            }
        }
        let list = self
            .engine
            .render_viewport_with(self.sheet, &vp, |o| {
                o.headers = Some(false);
                o.gridline_color = GRID_LINE;
                o.outside.clone_from(&outside);
                if !show_grid {
                    o.gridlines = Some(false);
                }
                if show_formulas {
                    o.show_formulas = Some(true);
                }
            })
            .map_err(|e| e.to_string())?;
        let plan = if self.full_redraw {
            None
        } else {
            self.drawn
                .and_then(|from| self.blit_plan(from, (list.width, list.height), k))
        };
        match plan {
            Some((moves, bands, _)) if moves.is_empty() && bands.is_empty() => {}
            Some((moves, bands, shift)) => {
                let turned = match shift {
                    Some(s) => store.turn(&list, &self.engine, k, s, &bands)?,
                    None => false,
                };
                if !turned {
                    store.scroll(&list, &self.engine, k, &moves, &bands)?;
                }
            }
            None => store.update(&list, &self.engine, k)?,
        }
        self.drawn = Some(self.scroll);
        self.grid_dirty = false;
        self.full_redraw = false;
        Ok(())
    }

    /// Moves and bands that turn the image drawn at scroll rom into the current one, or
    /// None when it must be drawn whole (both directions, fractional pixels, a jump
    /// larger than the scrolling part); with the shift of the whole image when nothing is
    /// frozen along the scroll (a ring store turns instead of moving pixels).
    #[allow(clippy::type_complexity)]
    fn blit_plan(
        &self,
        from: (f64, f64),
        (lw, lh): (f32, f32),
        k: f32,
    ) -> Option<(Vec<GridMove>, Vec<[u32; 4]>, Option<[i64; 2]>)> {
        let w = (lw * k).ceil() as i64;
        let h = (lh * k).ceil() as i64;
        let whole = |v: f64| {
            let r = v.round();
            ((v - r).abs() < 0.01).then_some(r as i64)
        };
        let dx = whole((self.scroll.0 - from.0) * f64::from(k))?;
        let dy = whole((self.scroll.1 - from.1) * f64::from(k))?;
        if dx == 0 && dy == 0 {
            return Some((Vec::new(), Vec::new(), None));
        }
        if dx != 0 && dy != 0 {
            return None;
        }
        let (gx, gy) = self.grid_origin();
        // Frozen rows / columns stay; the scrolling part starts after their boundary line.
        let fixed = if dy == 0 {
            self.freeze.0.map_or(0, |x| (x - gx + 1.0) as i64)
        } else {
            self.freeze.1.map_or(0, |y| (y - gy + 1.0) as i64)
        }
        .clamp(0, if dy == 0 { w } else { h });
        let (len, d) = if dy == 0 { (w, dx) } else { (h, dy) };
        let room = len - fixed;
        let n = d.abs();
        if n >= room {
            return None;
        }
        let u = |v: i64| v.max(0) as u32;
        // Along the scrolling axis: (source start, destination start, kept length, band start).
        let (src, dst, band) = if d > 0 {
            (fixed + n, fixed, len - n)
        } else {
            (fixed, fixed + n, fixed)
        };
        let keep = room - n;
        let mut moves = Vec::new();
        let mut bands = Vec::new();
        let rect = |a: i64, l: i64| -> [u32; 4] {
            if dy == 0 {
                [u(a), 0, u(l), u(h)]
            } else {
                [0, u(a), u(w), u(l)]
            }
        };
        if fixed > 0 {
            let r = rect(0, fixed);
            moves.push(GridMove {
                src: r,
                dst: [0, 0],
            });
            // Redraw around the frozen boundary line itself.
            bands.push(rect((fixed - 2).max(0), 4.min(len - (fixed - 2).max(0))));
        }
        let s = rect(src, keep);
        let at = if dy == 0 { [u(dst), 0] } else { [0, u(dst)] };
        moves.push(GridMove { src: s, dst: at });
        bands.push(rect(band, n));
        let shift = (fixed == 0).then_some([dx, dy]);
        Some((moves, bands, shift))
    }

    /// Whether Ctrl is held with the next mouse release (the host reads the key state).
    pub fn set_mouse_ctrl(&mut self, on: bool) {
        self.mouse_ctrl = on;
    }

    /// Lay out only the newly exposed rows when the grid image turns (default on; off
    /// lays out the whole viewport, for measurements).
    pub fn set_band_lists(&mut self, on: bool) {
        self.band_lists = on;
    }

    /// The grid image must be drawn again (text rendering or device changed).
    pub fn invalidate(&mut self) {
        self.redraw_all();
    }

    /// Caret rectangle of the in-cell editor (window pixels), for input method windows.
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        let e = self.edit.as_ref()?;
        let [l, t, _, b] = self.active_rect?;
        Some([l + 1.0 + self.pad() + e.caret_x, t + 1.0, 1.0, b - t - 1.0])
    }

    fn pad(&self) -> f32 {
        (2.0 * self.scale * self.zoom).round()
    }

    /// Paint headers, the grid image (`Painter::page(0)`), the selection and the editor.
    pub fn paint(&self, p: &mut dyn Painter) {
        if self.layout.is_some() {
            self.paint_layout(p);
            return;
        }
        let [fx, fy, fw, fh] = self.frame;
        let (gx, gy) = self.grid_origin();
        let (right, bottom) = (fx + fw, fy + fh);
        p.push_clip(fx, fy, fw, fh);
        p.fill_rect(gx, gy, right - gx, bottom - gy, Rgba(255, 255, 255, 255));
        p.page(0, gx as i32, gy as i32);
        self.paint_breaks(p);
        // A selected drawing (chart) hides the cell selection: no highlighted headers, tint
        // or frame, as on the reference.
        let (r0, c0, r1, c1) = if self.object_selected {
            (u32::MAX, u32::MAX, u32::MAX, u32::MAX)
        } else {
            self.range_bounds()
        };
        let full_cols = r0 == 0 && r1 == MAX_ROWS - 1;
        let full_rows = c0 == 0 && c1 == MAX_COLS - 1;
        // Headers first: the selection frame covers their edge lines, as on the reference.
        self.paint_headers(p, (r0, c0, r1, c1), full_rows, full_cols);
        let sel_rect = if self.object_selected {
            None
        } else {
            self.sel_rect
        };
        // Selection tint (all cells but the active one), then the frame.
        if let Some([l0, t0, r0_, b0]) = sel_rect {
            // Inside the frame's one-pixel white ring.
            let (l, t, r, b) = (l0 + 1.0, t0 + 1.0, r0_ - 1.0, b0 - 1.0);
            if (r0, c0) != (r1, c1) {
                p.push_clip(gx, gy, right - gx, bottom - gy);
                let rr = r.min(right);
                let bb = b.min(bottom);
                if let Some([al, at, ar, ab]) = self.active_rect {
                    // Tint around the active cell: above, below, left, right bands.
                    p.fill_rect(l + 1.0, t + 1.0, rr - l - 1.0, (at - t).max(0.0), SEL_TINT);
                    p.fill_rect(
                        l + 1.0,
                        ab + 1.0,
                        rr - l - 1.0,
                        (bb - ab - 1.0).max(0.0),
                        SEL_TINT,
                    );
                    p.fill_rect(l + 1.0, at + 1.0, (al - l).max(0.0), ab - at, SEL_TINT);
                    p.fill_rect(
                        ar + 1.0,
                        at + 1.0,
                        (rr - ar - 1.0).max(0.0),
                        ab - at,
                        SEL_TINT,
                    );
                } else {
                    p.fill_rect(l + 1.0, t + 1.0, rr - l - 1.0, bb - t - 1.0, SEL_TINT);
                }
                p.pop_clip();
            }
        }
        // Copied range: dashed frame.
        self.paint_copied(p, right, bottom);
        // Formula references: one coloured frame per reference.
        let w2 = (2.0 * self.scale / 1.25).round().max(2.0);
        p.push_clip(gx, gy, right - gx, bottom - gy);
        // Measured: a 2 px frame over the surrounding gridlines and one pixel outside them, a
        // 10 % tint (over gridlines and text) inside a one-pixel white ring, and a 5 px square framed in white on
        // each corner.
        let hs = (5.0 * self.scale / 1.25).round().max(5.0);
        let frames = self.ref_frames();
        let white = Rgba(255, 255, 255, 255);
        for &(color, [l, t, r, b]) in &frames {
            p.fill_rect(l + 1.0, t + 1.0, r - l - 1.0, 1.0, white);
            p.fill_rect(l + 1.0, b - 1.0, r - l - 1.0, 1.0, white);
            p.fill_rect(l + 1.0, t + 1.0, 1.0, b - t - 1.0, white);
            p.fill_rect(r - 1.0, t + 1.0, 1.0, b - t - 1.0, white);
            let tint = Rgba(color.0, color.1, color.2, REF_TINT_ALPHA);
            p.fill_rect(l + 2.0, t + 2.0, r - l - 3.0, b - t - 3.0, tint);
        }
        for &(color, [l, t, r, b]) in &frames {
            p.fill_rect(l - 1.0, t - 1.0, r - l + 3.0, w2, color);
            p.fill_rect(l - 1.0, b - w2 + 2.0, r - l + 3.0, w2, color);
            p.fill_rect(l - 1.0, t, w2, b - t + 1.0, color);
            p.fill_rect(r - w2 + 2.0, t, w2, b - t + 1.0, color);
            for (cx, cy) in [(l, t), (r, t), (l, b), (r, b)] {
                let (x, y) = (cx - (hs / 2.0).floor(), cy - (hs / 2.0).floor());
                p.fill_rect(x - 1.0, y - 1.0, hs + 2.0, hs + 2.0, white);
                p.fill_rect(x, y, hs, hs, color);
            }
        }
        // Fill handle drag: grey frame around the cells being filled.
        if let Some((r0, c0, r1, c1)) = self.fill_to {
            let find = |spans: &[Span], i: u32, first: bool| -> Option<f32> {
                spans
                    .iter()
                    .find(|x| x.index == i)
                    .map(|x| if first { x.start - 1.0 } else { x.line })
            };
            let l = find(&self.cols, c0, true).unwrap_or(gx - 1.0);
            let t = find(&self.rows, r0, true).unwrap_or(gy - 1.0);
            let r = find(&self.cols, c1, false).unwrap_or(right);
            let b = find(&self.rows, r1, false).unwrap_or(bottom);
            let grey = Rgba(0x80, 0x80, 0x80, 255);
            p.fill_rect(l, t, r - l + 1.0, 1.0, grey);
            p.fill_rect(l, b, r - l + 1.0, 1.0, grey);
            p.fill_rect(l, t, 1.0, b - t + 1.0, grey);
            p.fill_rect(r, t, 1.0, b - t + 1.0, grey);
        }
        p.pop_clip();
        if let Some([l, t, r, b]) = sel_rect {
            p.push_clip(gx - 2.0, gy - 2.0, right - gx + 2.0, bottom - gy + 2.0);
            let w = (2.0 * self.scale / 1.25).round().max(2.0);
            let (x0, y0, x1, y1) = (l - w + 1.0, t - w + 1.0, r + w, b + w);
            p.fill_rect(x0, y0, x1 - x0, w, SEL_GREEN);
            p.fill_rect(x0, y1 - w, x1 - x0, w, SEL_GREEN);
            p.fill_rect(x0, y0, w, y1 - y0, SEL_GREEN);
            p.fill_rect(x1 - w, y0, w, y1 - y0, SEL_GREEN);
            // Fill handle: a square on the bottom-right corner with a white edge.
            if !full_cols && !full_rows && self.edit.is_none() {
                let h = (5.0 * self.scale / 1.25).round().max(5.0);
                let (hx, hy) = (x1 - w / 2.0 - h / 2.0, y1 - w / 2.0 - h / 2.0);
                p.fill_rect(
                    hx - 1.0,
                    hy - 1.0,
                    h + 2.0,
                    h + 2.0,
                    Rgba(255, 255, 255, 255),
                );
                p.fill_rect(hx, hy, h, h, SEL_GREEN);
            }
            p.pop_clip();
        }
        self.paint_editor(p, right);
        p.pop_clip();
        // Frozen pane lines.
        if let Some(x) = self.freeze.0 {
            p.fill_rect(x, fy, 1.0, fh, FREEZE_LINE);
        }
        if let Some(y) = self.freeze.1 {
            p.fill_rect(fx, y, fw, 1.0, FREEZE_LINE);
        }
    }

    fn paint_copied(&self, p: &mut dyn Painter, right: f32, bottom: f32) {
        let Some((s, range)) = self.copied else {
            return;
        };
        if s != self.sheet {
            return;
        }
        let (gx, gy) = self.grid_origin();
        let find = |spans: &[Span], i: u32, first: bool| -> Option<f32> {
            spans
                .iter()
                .find(|x| x.index == i)
                .map(|x| if first { x.start - 1.0 } else { x.line })
        };
        let l = find(&self.cols, range.first.col, true).unwrap_or(gx - 1.0);
        let t = find(&self.rows, range.first.row, true).unwrap_or(gy - 1.0);
        let r = find(&self.cols, range.last.col, false).unwrap_or(right);
        let b = find(&self.rows, range.last.row, false).unwrap_or(bottom);
        let dash = (3.0 * self.scale).round();
        let w = (2.0 * self.scale / 1.25).round().max(2.0);
        p.push_clip(gx, gy, right - gx, bottom - gy);
        let mut x = l;
        while x < r {
            let len = dash.min(r - x);
            p.fill_rect(x, t - 1.0, len, w, SEL_GREEN);
            p.fill_rect(x, b, len, w, SEL_GREEN);
            x += 2.0 * dash;
        }
        let mut y = t;
        while y < b {
            let len = dash.min(b - y);
            p.fill_rect(l - 1.0, y, w, len, SEL_GREEN);
            p.fill_rect(r, y, w, len, SEL_GREEN);
            y += 2.0 * dash;
        }
        p.pop_clip();
    }

    fn paint_editor(&self, p: &mut dyn Painter, right: f32) {
        let (Some(e), Some([l, t, r, b])) = (self.edit.as_ref(), self.active_rect) else {
            return;
        };
        let pad = self.pad();
        let font = self.edit_font();
        let w = (e.text_w + 2.0 * pad + (4.0 * self.scale).round()).max(r - l - 1.0);
        let w = w.min(right - l - 1.0);
        p.fill_rect(l + 1.0, t + 1.0, w, b - t - 1.0, Rgba(255, 255, 255, 255));
        let color = rgba_hex(&self.format.color).unwrap_or(Rgba(0, 0, 0, 255));
        let line_h = font.size * 1.22;
        // Bottom aligned like the cell's default vertical alignment.
        let ty = (b - pad / 2.0 - line_h).round().max(t + 1.0);
        let shown = format!("{}{}{}", &e.text[..e.caret], e.preedit, &e.text[e.caret..]);
        p.push_clip(l + 1.0, t + 1.0, w, b - t - 1.0);
        if e.runs.is_empty() {
            p.text_font(&shown, l + 1.0 + pad, ty, &font, color);
        } else {
            for (x, range, c) in &e.runs {
                if let Some(part) = e.text.get(range.clone()) {
                    p.text_font(part, l + 1.0 + pad + x, ty, &font, c.unwrap_or(color));
                }
            }
        }

        // An AutoComplete suggestion is shown selected.
        if let Some(a) = e.auto {
            let x0 = (l + 1.0 + pad + e.auto_x).round();
            let x1 = (l + 1.0 + pad + e.text_w).round();
            p.fill_rect(
                x0,
                (t + 3.0).round(),
                x1 - x0,
                (b - t - 5.0).max(1.0),
                Rgba(0x00, 0x78, 0xd7, 255),
            );
            if let Some(rest) = e.text.get(a..) {
                p.text_font(rest, x0, ty, &font, Rgba(255, 255, 255, 255));
            }
        }
        let cx = (l + 1.0 + pad + e.caret_x).round();
        p.fill_rect(
            cx,
            (t + 3.0).round(),
            1.0,
            (b - t - 5.0).max(1.0),
            Rgba(0, 0, 0, 255),
        );
        p.pop_clip();
    }

    fn paint_headers(
        &self,
        p: &mut dyn Painter,
        (r0, c0, r1, c1): (u32, u32, u32, u32),
        full_rows: bool,
        full_cols: bool,
    ) {
        let [fx, fy, fw, fh] = self.frame;
        let (gx, gy) = self.grid_origin();
        let (right, bottom) = (fx + fw, fy + fh);
        let font = Font {
            family: &self.header_font.0,
            size: (self.header_font.1 as f32 * self.k()).max(1.0),
            weight: 400,
            italic: false,
            underline: false,
        };
        let line_h = (font.size * 1.22).round();
        // Column headers.
        p.push_clip(gx, fy, right - gx, gy - fy);
        p.fill_rect(gx, fy, right - gx, gy - fy, HDR_BG);
        for c in &self.cols {
            let sel = c.index >= c0 && c.index <= c1;
            let (x0, x1) = (c.start, c.line);
            if sel {
                p.fill_rect(x0, fy, x1 - x0, gy - fy, HDR_SEL_BG);
            }
            let line = if sel || full_rows {
                HDR_SEL_LINE
            } else {
                HDR_COL_LINE
            };
            p.fill_rect(x1, fy, 1.0, gy - fy - 1.0, line);
            let label = col_to_letters(c.index);
            let ink = if sel { SEL_GREEN } else { HDR_TEXT };
            // Measured: column letters sit one pixel higher than row numbers in their band.
            self.centred(
                p,
                &label,
                &font,
                (x0, x1),
                (fy - 1.0, gy - 2.0),
                line_h,
                ink,
            );
        }
        p.fill_rect(gx, gy - 1.0, right - gx, 1.0, HDR_EDGE);
        p.pop_clip();
        // Row headers.
        p.push_clip(fx, gy, gx - fx, bottom - gy);
        p.fill_rect(fx, gy, gx - fx, bottom - gy, HDR_BG);
        for r in &self.rows {
            let sel = r.index >= r0 && r.index <= r1;
            let (y0, y1) = (r.start, r.line);
            if sel {
                p.fill_rect(fx, y0, gx - fx, y1 - y0, HDR_SEL_BG);
            }
            let line = if sel || full_cols {
                HDR_SEL_LINE
            } else {
                HDR_ROW_LINE
            };
            p.fill_rect(fx, y1, gx - fx - 1.0, 1.0, line);
            let ink = if sel { SEL_GREEN } else { HDR_TEXT };
            self.centred(
                p,
                &(r.index + 1).to_string(),
                &font,
                (fx, gx - 1.0),
                (y0, y1),
                line_h,
                ink,
            );
        }
        p.fill_rect(gx - 1.0, gy, 1.0, bottom - gy, HDR_EDGE);
        p.pop_clip();
        // Corner: select-all triangle.
        p.fill_rect(fx, fy, gx - fx, gy - fy, HDR_BG);
        p.fill_rect(gx - 1.0, fy, 1.0, gy - fy, HDR_EDGE);
        p.fill_rect(fx, gy - 1.0, gx - fx, 1.0, HDR_EDGE);
        let s = (gy - fy - (8.0 * self.scale).round()).max(4.0);
        let (ex, ey) = (
            gx - (4.0 * self.scale).round(),
            gy - (4.0 * self.scale).round(),
        );
        p.path(
            &[
                PathSeg::M(ex, ey - s * 0.75),
                PathSeg::L(ex, ey),
                PathSeg::L(ex - s * 0.75, ey),
                PathSeg::Z,
            ],
            Some(CORNER_MARK),
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn centred(
        &self,
        p: &mut dyn Painter,
        s: &str,
        font: &Font<'_>,
        (x0, x1): (f32, f32),
        (y0, y1): (f32, f32),
        line_h: f32,
        ink: Rgba,
    ) {
        let w = self.label_w(s, font);
        p.text_font(
            s,
            ((x0 + x1 - w) / 2.0).round(),
            ((y0 + y1 - line_h) / 2.0).round(),
            font,
            ink,
        );
    }

    /// Measured width of a header label ([`SheetView::measure_labels`]); an estimate of
    /// half an em per character until then.
    fn label_w(&self, s: &str, font: &Font<'_>) -> f32 {
        self.label_widths
            .get(s)
            .copied()
            .unwrap_or_else(|| s.chars().count() as f32 * font.size * 0.52)
    }

    /// Window box of a visible cell: left and top gridline, right and bottom gridline.
    pub(crate) fn cell_box(&self, r: u32, c: u32) -> Option<[f32; 4]> {
        let row = self.rows.iter().find(|s| s.index == r)?;
        let col = self.cols.iter().find(|s| s.index == c)?;
        Some([col.start - 1.0, row.start - 1.0, col.line, row.line])
    }

    /// Placed rows: index, first pixel, closing gridline (tests).
    #[cfg(test)]
    pub(crate) fn rows_for_test(&self) -> Vec<(u32, f32, f32)> {
        self.rows
            .iter()
            .map(|s| (s.index, s.start, s.line))
            .collect()
    }

    /// Placed columns (tests).
    #[cfg(test)]
    pub(crate) fn cols_for_test(&self) -> Vec<(u32, f32, f32)> {
        self.cols
            .iter()
            .map(|s| (s.index, s.start, s.line))
            .collect()
    }

    /// Selection frame (tests).
    #[cfg(test)]
    pub(crate) fn sel_rect_for_test(&self) -> Option<[f32; 4]> {
        self.sel_rect
    }

    /// Grid frame (tests).
    #[cfg(test)]
    pub(crate) fn frame_for_test(&self) -> [f32; 4] {
        self.frame
    }

    /// Measure the header labels now shown (call after input, with the host's text measure).
    pub fn measure_labels(&mut self, m: &dyn TextMeasure) {
        let size = (self.header_font.1 as f32 * self.k()).max(1.0);
        if size != self.label_size {
            self.label_widths.clear();
            self.label_size = size;
        }
        let font = Font {
            family: &self.header_font.0,
            size,
            weight: 400,
            italic: false,
            underline: false,
        };
        let labels: Vec<String> = self
            .cols
            .iter()
            .map(|c| col_to_letters(c.index))
            .chain(self.rows.iter().map(|r| (r.index + 1).to_string()))
            .filter(|l| !self.label_widths.contains_key(l))
            .collect();
        for l in labels {
            let w = m.measure_font(&l, &font).0;
            self.label_widths.insert(l, w);
        }
        if self.label_widths.len() > 20_000 {
            self.label_widths.clear();
        }
    }
}

/// The next absolute / relative form of the cell reference (or range) touching byte `caret` of
/// formula `text`, and the caret after it: `$A$1` → `A$1` → `$A1` → `A1` → `$A$1`.
pub fn cycle_reference(text: &str, caret: usize) -> Option<(String, usize)> {
    let b = text.as_bytes();
    let part = |c: u8| c.is_ascii_alphanumeric() || c == b'$' || c == b':';
    let mut s = caret.min(b.len());
    while s > 0 && part(b[s - 1]) {
        s -= 1;
    }
    let mut e = caret.min(b.len());
    while e < b.len() && part(b[e]) {
        e += 1;
    }
    let token = text.get(s..e)?;
    // Each cell of a range: optional $, letters, optional $, digits.
    let parse = |cell: &str| -> Option<(bool, String, bool, String)> {
        let mut rest = cell;
        let abs_c = rest.starts_with('$');
        rest = rest.trim_start_matches('$');
        let letters: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
        rest = &rest[letters.len()..];
        let abs_r = rest.starts_with('$');
        rest = rest.trim_start_matches('$');
        let ok = !letters.is_empty()
            && letters.len() <= 3
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit());
        ok.then(|| (abs_c, letters, abs_r, rest.to_owned()))
    };
    let cells: Vec<_> = token.split(':').map(parse).collect::<Option<Vec<_>>>()?;
    let (ac, _, ar, _) = cells.first()?.clone();
    // Next state from the first cell's state.
    let (nc, nr) = match (ac, ar) {
        (true, true) => (false, true),
        (false, true) => (true, false),
        (true, false) => (false, false),
        (false, false) => (true, true),
    };
    let out: Vec<String> = cells
        .iter()
        .map(|(_, l, _, d)| {
            format!(
                "{}{l}{}{d}",
                if nc { "$" } else { "" },
                if nr { "$" } else { "" }
            )
        })
        .collect();
    let new = out.join(":");
    let mut text = text.to_owned();
    text.replace_range(s..e, &new);
    Some((text, s + new.len()))
}

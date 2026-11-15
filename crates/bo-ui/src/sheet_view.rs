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

/// Keeps the grid image drawn from the engine's display list (a GPU bitmap on Windows).
pub trait GridStore {
    /// Draw `list` (points) at `scale` device pixels per point as the new grid image; the
    /// glyph runs refer to `engine`'s fonts.
    fn update(&mut self, list: &PageList, engine: &SheetEngine, scale: f32) -> Result<(), String>;
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
}

/// The grid of one workbook window.
pub struct SheetView {
    engine: SheetEngine,
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
        (19.2 * self.scale * self.zoom).round()
    }

    /// Width of the row header band: room for the largest row number shown (34 px at
    /// 125 % for up to two digits, as measured), its last pixel being the edge line.
    fn hdr_w(&self) -> f32 {
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
            self.grid_dirty = true;
            self.refresh();
        }
    }

    /// Display scale (device pixels per logical pixel).
    pub fn set_scale(&mut self, scale: f32) {
        if scale > 0.0 && scale != self.scale {
            self.scale = scale;
            self.grid_dirty = true;
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
        self.grid_dirty = true;
        self.refresh();
        true
    }

    /// Viewport of the grid image, in points (no headers: the window draws them).
    fn viewport(&self) -> Viewport {
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
        self.load_details();
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
        self.grid_dirty = true;
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
        self.grid_dirty = true;
        self.refresh();
        true
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
        self.refresh();
        true
    }

    /// Scroll position, scrollable extent and visible size of the main pane, in points
    /// (`vertical` or horizontal), for scroll bars.
    pub fn scroll_info(&mut self, vertical: bool) -> (f64, f64, f64) {
        let vp = self.viewport();
        let (content_w, content_h) = self.engine.content_size(self.sheet).unwrap_or((0.0, 0.0));
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
        self.refresh();
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
        });
        self.measure_edit(m);
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
            if let Some(e) = self.edit.as_mut() {
                e.caret_x = caret_x;
                e.text_w = text_w;
            }
        }
    }

    /// Commit the editor's text to the active cell; `true` when something was written.
    pub fn commit(&mut self) -> bool {
        let Some(e) = self.edit.take() else {
            return false;
        };
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
        let e = self.edit.as_mut()?;
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
                }
                self.edit_insert(&s, m);
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
            Input::Key(k, mods) => self.key(k, mods, m),
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
            Input::Drag { x, y } => self.drag_to(x, y),
            Input::Release { x, y } => {
                let d = self.drag.take();
                match d {
                    Some(Drag::ColEdge(c, origin)) => self.resize_edge(false, c, origin, x),
                    Some(Drag::RowEdge(r, origin)) => self.resize_edge(true, r, origin, y),
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

    /// Pointer cursor kind at a window point: `0` cell cross, `1` column resize, `2` row resize,
    /// `3` arrow (headers).
    pub fn cursor_at(&self, x: f32, y: f32) -> u8 {
        match self.header_edge(x, y) {
            Some((false, ..)) => 1,
            Some((true, ..)) => 2,
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
        if self.edit.is_some() {
            self.commit();
        }
        if let Some((rows, index, line)) = self.header_edge(x, y) {
            self.drag = Some(if rows {
                Drag::RowEdge(index, line)
            } else {
                Drag::ColEdge(index, line)
            });
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

    fn drag_to(&mut self, x: f32, y: f32) -> bool {
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
            Drag::ColEdge(..) | Drag::RowEdge(..) => return true,
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

    /// Redraw the grid image when the cells, scroll or size changed.
    pub fn prepare(&mut self, store: &mut dyn GridStore) -> Result<(), String> {
        if !self.grid_dirty {
            return Ok(());
        }
        let vp = self.viewport();
        let list = self
            .engine
            .render_viewport_with(self.sheet, &vp, |o| {
                o.headers = Some(false);
                o.gridline_color = GRID_LINE;
            })
            .map_err(|e| e.to_string())?;
        store.update(&list, &self.engine, self.k())?;
        self.grid_dirty = false;
        Ok(())
    }

    /// The grid image must be drawn again (text rendering or device changed).
    pub fn invalidate(&mut self) {
        self.grid_dirty = true;
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
        let [fx, fy, fw, fh] = self.frame;
        let (gx, gy) = self.grid_origin();
        let (right, bottom) = (fx + fw, fy + fh);
        p.push_clip(fx, fy, fw, fh);
        p.fill_rect(gx, gy, right - gx, bottom - gy, Rgba(255, 255, 255, 255));
        p.page(0, gx as i32, gy as i32);
        let (r0, c0, r1, c1) = self.range_bounds();
        let full_cols = r0 == 0 && r1 == MAX_ROWS - 1;
        let full_rows = c0 == 0 && c1 == MAX_COLS - 1;
        // Headers first: the selection frame covers their edge lines, as on the reference.
        self.paint_headers(p, (r0, c0, r1, c1), full_rows, full_cols);
        // Selection tint (all cells but the active one), then the frame.
        if let Some([l0, t0, r0_, b0]) = self.sel_rect {
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
        if let Some([l, t, r, b]) = self.sel_rect {
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
        p.text_font(&shown, l + 1.0 + pad, ty, &font, color);

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

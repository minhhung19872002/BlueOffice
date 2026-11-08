//! BlueSheet engine facade: one workbook, rendered viewport by viewport.
//!
//! Same contract as the document [`Engine`](crate::Engine): every host (WASM
//! worker, Tauri, CLI) calls these functions, results cross the boundary as
//! JSON, and the canvas draws the `bo-render` page list it gets back. Sheet
//! coordinates and viewport sizes are in points.

use crate::{EngineError, GlyphOutline, Result};
use bo_model::sheet::{Cell, CellRef, CellValue, Formula, Workbook, Worksheet};
use bo_render::PageList;
use bo_sheet::calc::FormulaAdjuster;
use bo_sheet::formula::BoFormula;
use bo_sheet::geometry::SheetGeometry;
use bo_sheet::live::LiveCalc;
use bo_sheet::numfmt::Locale;
use bo_sheet::render::{format_cell, render_viewport, DrawingOverlay, RenderOptions};
use bo_sheet::viewport::{Hit, ViewLayout, Viewport};
use bo_text::FontManager;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

mod cmd;
pub(crate) mod collab;
mod pivot_cmd;
mod png;
mod print;

pub use cmd::{Clip, PasteMode, SheetCommand};

/// Hard limits of the grid (ECMA-376 Part 1, §18.3.1.73 `row/@r` ≤ 1 048 576; 16 384 columns).
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

/// SmartArt fallbacks ([MS-ODRAWXML] `dsp:drawing`) of every sheet, drawn as
/// vector commands for the size of their anchor when the workbook is opened.
fn diagram_overlays(wb: &Workbook, fonts: &mut FontManager) -> BTreeMap<String, DrawingOverlay> {
    const EMU_PER_PT: f64 = 12_700.0;
    let mut out = BTreeMap::new();
    for ws in wb.sheets.iter().filter(|ws| !ws.diagrams.is_empty()) {
        let g = SheetGeometry::new(ws, &wb.styles, fonts);
        for d in &ws.diagrams {
            if out.contains_key(&d.part) {
                continue;
            }
            let Some(xml) = wb
                .opaque
                .parts
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(&d.part))
            else {
                continue;
            };
            let to = d.to.unwrap_or(CellRef::new(
                d.from.row.saturating_add(10),
                d.from.col.saturating_add(6),
            ));
            let w = g.col_x(to.col) - g.col_x(d.from.col);
            let h = g.row_y(to.row) - g.row_y(d.from.row);
            #[allow(clippy::cast_possible_truncation)]
            let (we, he) = ((w * EMU_PER_PT) as i64, (h * EMU_PER_PT) as i64);
            if let Some(commands) =
                bo_pptx::diagram::render_drawing(&xml.data, &d.part, we, he, fonts)
            {
                out.insert(d.part.clone(), DrawingOverlay { w, h, commands });
            }
        }
    }
    out
}

/// Summary of an open workbook.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookInfo {
    pub name: String,
    pub sheets: Vec<SheetInfo>,
    /// The file contains a VBA project ([MS-OVBA]) or XLM macro sheets; kept
    /// when saving, never run.
    pub macros: bool,
    /// Cells converted from TCVN3 / VNI fonts to Unicode on open (0 = none);
    /// the conversion is the first undo step.
    pub legacy_converted: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    /// Used range: last row / column with content (0-based), `None` when empty.
    pub last_row: Option<u32>,
    pub last_col: Option<u32>,
    pub frozen_rows: u32,
    pub frozen_cols: u32,
    pub hidden: bool,
    /// Hidden row spans `[first, last]` (filtered or hidden rows), for navigation.
    pub hidden_rows: Vec<(u32, u32)>,
    pub hidden_cols: Vec<(u32, u32)>,
    /// AutoFilter range `[r0, c0, r1, c1]`.
    pub auto_filter: Option<[u32; 4]>,
    /// Merged ranges `[r0, c0, r1, c1]`.
    pub merges: Vec<[u32; 4]>,
    pub charts: usize,
    /// Number of pivot tables on the sheet.
    pub pivots: usize,
}

/// Where a screen point falls (viewport-relative, points).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HitInfo {
    Cell {
        row: u32,
        col: u32,
    },
    ColumnHeader {
        col: u32,
    },
    RowHeader {
        row: u32,
    },
    /// Near the right edge of column `col` in the header (resize / autofit).
    ColumnEdge {
        col: u32,
    },
    /// Near the bottom edge of row `row` in the header.
    RowEdge {
        row: u32,
    },
    /// The AutoFilter button of column `col`.
    FilterButton {
        row: u32,
        col: u32,
    },
    /// A chart placed on the sheet, and the cell under the point.
    Chart {
        index: usize,
        row: u32,
        col: u32,
    },
    Corner,
    None,
}

/// Screen rectangle (points) of a cell or range in the current viewport.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What the formula bar / in-cell editor shows for a cell.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CellDetails {
    pub address: String,
    /// Editable input: `=FORMULA`, a plain number, or text.
    pub input: String,
    /// Formatted display text.
    pub display: String,
}

/// Viewport request from the UI.
#[derive(Debug, Clone, Copy)]
pub struct ViewRequest {
    pub sheet: usize,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewRequest {
    fn viewport(&self) -> Viewport {
        let mut vp = Viewport::new(self.width, self.height);
        vp.scroll_x = self.scroll_x.max(0.0);
        vp.scroll_y = self.scroll_y.max(0.0);
        vp
    }
}

enum UndoEntry {
    /// Previous content of individual cells.
    Cells {
        sheet: usize,
        cells: Vec<(u32, u32, Option<Cell>)>,
    },
    /// Whole workbook before a structural change (insert/delete rows, sort…).
    Workbook(Box<Workbook>),
}

/// The sheet engine.
pub struct SheetEngine {
    fonts: FontManager,
    name: String,
    wb: Option<Workbook>,
    geometry: Vec<Option<SheetGeometry>>,
    locale: Locale,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Persistent formula engine: edits recalculate only their dependents.
    live: LiveCalc,
    adjuster: BoFormula,
    /// Internal clipboard (cell records) for paste / paste special.
    clip: Option<Clip>,
    /// Live collaboration binding (`bo-collab`).
    collab: Option<collab::SheetCollab>,
    /// The open file has macros (warning bar; preserved, never executed).
    macros: bool,
    /// Cells the last legacy-font conversion changed.
    legacy_converted: usize,
    /// SmartArt drawing fallbacks of the open workbook, drawn once on load.
    drawings: Arc<BTreeMap<String, DrawingOverlay>>,
}

impl Default for SheetEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_number(s: &str, locale: &Locale) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (t, scale) = match t.strip_suffix('%') {
        Some(p) => (p.trim_end(), 0.01),
        None => (t, 1.0),
    };
    // Locale form first: "1.234,5" (vi-VN) / "1,234.5" (en-US).
    let dec = locale.decimal;
    let group = locale.thousands;
    let normalized: String = t
        .chars()
        .filter(|&c| c != group)
        .map(|c| if c == dec { '.' } else { c })
        .collect();
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
    };
    let v = if valid(&normalized) {
        normalized.parse::<f64>().ok()
    } else {
        None
    }
    .or_else(|| t.parse::<f64>().ok().filter(|_| valid(t)))?;
    v.is_finite().then_some(v * scale)
}

/// Legacy `.xls` / `.xlsb` workbook ([MS-XLS], [MS-XLSB] via `bo-binary`) →
/// sheet model. Number formats become custom `numFmt`s + `cellXfs`.
fn from_legacy(src: &bo_binary::Workbook) -> Workbook {
    use bo_binary::CellValue as L;
    let mut wb = Workbook::new();
    wb.date1904 = src.date1904;
    wb.sheets.clear();
    let base_xf = wb.styles.cell_xfs.first().cloned().unwrap_or_default();
    if wb.styles.cell_xfs.is_empty() {
        wb.styles.cell_xfs.push(base_xf.clone());
    }
    let mut style_of = std::collections::HashMap::<String, u32>::new();
    for (i, ls) in src.sheets.iter().enumerate() {
        let mut ws = Worksheet::new(&ls.name);
        ws.sheet_id = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if ls.hidden {
            ws.state = bo_model::sheet::SheetState::Hidden;
        }
        for c in &ls.cells {
            let value = match &c.value {
                L::Empty => CellValue::Empty,
                L::Number(n) => CellValue::Number(*n),
                L::Text(t) => CellValue::Text(t.clone()),
                L::Bool(b) => CellValue::Bool(*b),
                L::Error(e) => CellValue::Error(bo_model::sheet::CellError::parse(e)),
            };
            let style = match &c.number_format {
                Some(code) => *style_of.entry(code.clone()).or_insert_with(|| {
                    let id = 164 + u32::try_from(wb.styles.num_fmts.len()).unwrap_or(0);
                    wb.styles.num_fmts.push(bo_model::sheet::NumFmt {
                        id,
                        code: code.clone(),
                    });
                    let mut xf = base_xf.clone();
                    xf.num_fmt_id = id;
                    xf.apply_number_format = Some(true);
                    wb.styles.cell_xfs.push(xf);
                    u32::try_from(wb.styles.cell_xfs.len() - 1).unwrap_or(0)
                }),
                None => 0,
            };
            let mut cell = Cell::new(value);
            cell.style = style;
            cell.formula = c.formula.as_ref().map(|f| Formula {
                text: f.clone(),
                ..Formula::default()
            });
            ws.cells.insert((c.row, c.col), cell);
        }
        wb.sheets.push(ws);
    }
    if wb.sheets.is_empty() {
        wb.sheets.push(Worksheet::new("Sheet1"));
    }
    wb
}

/// OpenDocument spreadsheet (ODF 1.3 Part 3 §9, via `bo-odf`) → sheet model.
/// Values are kept; `of:` formulas are kept as cached values only (the
/// OpenFormula → A1 translation is not implemented yet).
fn from_ods(src: &bo_odf::SpreadsheetDoc) -> Workbook {
    use bo_odf::CellValue as O;
    let mut wb = Workbook::new();
    wb.sheets.clear();
    for (i, os) in src.sheets.iter().enumerate() {
        let mut ws = Worksheet::new(&os.name);
        ws.sheet_id = u32::try_from(i + 1).unwrap_or(u32::MAX);
        for c in &os.cells {
            let value = match &c.value {
                O::Float(v) | O::Percentage(v) | O::Currency { value: v, .. } => {
                    CellValue::Number(*v)
                }
                O::Date(t) | O::Time(t) | O::String(t) => CellValue::Text(t.clone()),
                O::Boolean(b) => CellValue::Bool(*b),
            };
            ws.cells.insert((c.row, c.col), Cell::new(value));
        }
        wb.sheets.push(ws);
    }
    if wb.sheets.is_empty() {
        wb.sheets.push(Worksheet::new("Sheet1"));
    }
    wb
}

/// Turns what the user typed into a cell (keeping the cell's style). Typed
/// dates and times become serial numbers (§18.17.4); the built-in date number
/// format to give the cell is returned with it (applied by the caller when the
/// cell's format is General, see [`date_style`]).
fn input_to_cell_typed(
    input: &str,
    old: Option<&Cell>,
    locale: &Locale,
    date1904: bool,
) -> (Option<Cell>, Option<u32>) {
    if !input.starts_with(['=', '\'']) {
        if let Some(t) = bo_sheet::numfmt::parse_typed_date(input, locale, date1904) {
            let mut cell = old.cloned().unwrap_or_default();
            cell.formula = None;
            cell.value = CellValue::Number(t.serial);
            return (Some(cell), Some(t.num_fmt_id));
        }
    }
    (input_to_cell(input, old, locale), None)
}

/// Style index giving `style` the built-in number format `fmt_id`, when its
/// current format is General (§18.8.30 id 0); any other format (a date format
/// chosen by the user, text `@`, …) is kept.
fn date_style(styles: &mut bo_model::sheet::Stylesheet, style: u32, fmt_id: u32) -> u32 {
    let mut xf = styles.xf(style).cloned().unwrap_or_default();
    if xf.num_fmt_id != 0 {
        return style;
    }
    xf.num_fmt_id = fmt_id;
    xf.apply_number_format = Some(true);
    styles.add_xf(xf)
}

/// Turns what the user typed into a cell (keeping the cell's style).
fn input_to_cell(input: &str, old: Option<&Cell>, locale: &Locale) -> Option<Cell> {
    let mut cell = old.cloned().unwrap_or_default();
    cell.formula = None;
    if input.is_empty() {
        if cell.style == 0 {
            return None;
        }
        cell.value = CellValue::Empty;
        return Some(cell);
    }
    if let Some(f) = input.strip_prefix('=').filter(|f| !f.trim().is_empty()) {
        cell.formula = Some(Formula {
            text: f.to_owned(),
            ..Formula::default()
        });
        cell.value = CellValue::Empty;
        return Some(cell);
    }
    let upper = input.trim().to_uppercase();
    cell.value = match upper.as_str() {
        "TRUE" | "ĐÚNG" => CellValue::Bool(true),
        "FALSE" | "SAI" => CellValue::Bool(false),
        _ => match parse_number(input, locale) {
            Some(n) => CellValue::Number(n),
            None => CellValue::Text(input.strip_prefix('\'').unwrap_or(input).to_owned()),
        },
    };
    Some(cell)
}

/// Formula-bar text of a number in a cell with a built-in date / time format
/// (§18.8.30 ids 14, 18–22): the date in the locale's short date form (plus
/// the time), so that editing and confirming keeps the value. `None` for
/// other formats.
fn date_input(n: f64, fmt_id: u32, locale: &Locale, date1904: bool) -> Option<String> {
    let code = match fmt_id {
        14 => locale.short_date.to_owned(),
        22 => format!("{} h:mm:ss", locale.short_date),
        18..=21 if (0.0..1.0).contains(&n) => "h:mm:ss".to_owned(),
        _ => return None,
    };
    let opts = bo_sheet::numfmt::FormatOptions {
        locale: *locale,
        date1904,
    };
    Some(bo_sheet::numfmt::format_number(n, &code, &opts).text)
}

fn number_input(n: f64, locale: &Locale) -> String {
    let s = format!("{n}");
    if locale.decimal == '.' {
        s
    } else {
        s.replace('.', &locale.decimal.to_string())
    }
}

impl SheetEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: FontManager::new(),
            name: String::new(),
            wb: None,
            geometry: Vec::new(),
            locale: Locale::VI_VN,
            undo: Vec::new(),
            redo: Vec::new(),
            live: LiveCalc::new(),
            adjuster: BoFormula::new(),
            clip: None,
            collab: None,
            macros: false,
            legacy_converted: 0,
            drawings: Arc::default(),
        }
    }

    /// Engine with the bundled fonts loaded (native hosts).
    #[cfg(feature = "bundled-fonts")]
    #[must_use]
    pub fn with_bundled_fonts() -> Self {
        let mut e = Self::new();
        for f in crate::bundled::FONTS {
            e.add_font(f.to_vec());
        }
        e
    }

    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        self.fonts.add_font(bytes).len()
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.fonts.len()
    }

    /// Sets the clock used by `NOW()`/`TODAY()` (Unix milliseconds, UTC).
    /// Browser hosts call it with `Date.now()` since WASM has no clock.
    #[allow(clippy::cast_precision_loss)]
    pub fn set_now_ms(&mut self, unix_ms: u64) {
        // 1970-01-01 is serial 25569 in the 1900 date system.
        let mut serial = unix_ms as f64 / 86_400_000.0 + 25_569.0;
        if self.wb.as_ref().is_some_and(|w| w.date1904) {
            serial -= 1_462.0;
        }
        self.adjuster.now = Some(serial);
        self.live.set_now(Some(serial));
    }

    /// Locale for input parsing and display (`vi-VN` default).
    pub fn set_locale(&mut self, tag: &str) {
        self.locale = if tag.eq_ignore_ascii_case("en-US") {
            Locale::EN_US
        } else {
            Locale::VI_VN
        };
    }

    /// Opens `.xlsx` / `.xlsm` / `.xlsb` / `.xls` / `.ods` / `.csv` bytes
    /// (legacy and ODF files are converted; saving writes `.xlsx`).
    pub fn open(&mut self, bytes: &[u8], name: &str) -> Result<WorkbookInfo> {
        let lower = name.to_ascii_lowercase();
        let wb = if lower.ends_with(".csv") {
            let opts = if self.locale.decimal == ',' {
                bo_xlsx::CsvOptions::vi_vn()
            } else {
                bo_xlsx::CsvOptions::default()
            };
            let ws =
                bo_xlsx::read_csv(bytes, &opts).map_err(|e| EngineError::Sheet(e.to_string()))?;
            let mut wb = Workbook::new();
            wb.sheets = vec![ws];
            wb
        } else if matches!(bo_binary::detect(bytes), Some(bo_binary::LegacyKind::Xls)) {
            from_legacy(&bo_binary::read_xls(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?)
        } else if bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODS) {
            from_ods(&bo_odf::read_ods(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?)
        } else if lower.ends_with(".xlsb") {
            from_legacy(
                &bo_binary::read_xlsb(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?,
            )
        } else if bytes.starts_with(b"PK\x03\x04") {
            bo_xlsx::read_xlsx(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?
        } else {
            return Err(EngineError::Unsupported(name.to_owned()));
        };
        let macros = !lower.ends_with(".csv") && bo_binary::has_macros(bytes);
        self.load(wb, name)?;
        self.macros = macros;
        self.convert_legacy_text()?;
        self.info()
    }

    /// Converts cells shown in legacy Vietnamese fonts (TCVN3 `.Vn*`, `VNI-*`)
    /// to Unicode and switches those fonts to a Unicode font
    /// ([`bo_encoding::convert_workbook`]); one undo step. Returns the number
    /// of cells converted (also reported as `legacyConverted`).
    pub fn convert_legacy_text(&mut self) -> Result<usize> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let before = Box::new(wb.clone());
        let n = bo_encoding::convert_workbook(wb);
        self.legacy_converted = n;
        if n > 0 {
            self.live.rebuild(wb);
            self.invalidate(None);
            self.push_undo(UndoEntry::Workbook(before));
        }
        Ok(n)
    }

    /// New empty workbook with one sheet.
    pub fn new_workbook(&mut self, name: &str) -> Result<WorkbookInfo> {
        let mut wb = Workbook::new();
        if wb.sheets.is_empty() {
            wb.sheets.push(Worksheet::new("Sheet1"));
        }
        self.load(wb, name)
    }

    pub fn load(&mut self, mut wb: Workbook, name: &str) -> Result<WorkbookInfo> {
        self.live.rebuild(&mut wb);
        // One-cell / absolute chart anchors (§20.5.2.24, §20.5.2.1) → cells.
        for si in 0..wb.sheets.len() {
            if wb.sheets[si]
                .charts
                .iter()
                .any(|c| c.anchor != bo_model::sheet::ChartAnchor::TwoCell)
            {
                let g = SheetGeometry::new(&wb.sheets[si], &wb.styles, &mut self.fonts);
                for c in &mut wb.sheets[si].charts {
                    bo_sheet::chart::resolve_anchor(c, &g);
                }
            }
        }
        self.drawings = Arc::new(diagram_overlays(&wb, &mut self.fonts));
        self.clip = None;
        self.collab = None;
        self.geometry = (0..wb.sheets.len()).map(|_| None).collect();
        self.wb = Some(wb);
        self.name = name.to_owned();
        self.undo.clear();
        self.redo.clear();
        self.macros = false;
        self.legacy_converted = 0;
        self.info()
    }

    #[must_use]
    pub fn workbook(&self) -> Option<&Workbook> {
        self.wb.as_ref()
    }

    fn wb(&self) -> Result<&Workbook> {
        self.wb.as_ref().ok_or(EngineError::NoDocument)
    }

    fn sheet(&self, i: usize) -> Result<&Worksheet> {
        self.wb()?
            .sheets
            .get(i)
            .ok_or(EngineError::PageOutOfRange(i))
    }

    pub fn info(&self) -> Result<WorkbookInfo> {
        let wb = self.wb()?;
        Ok(WorkbookInfo {
            name: self.name.clone(),
            sheets: wb
                .sheets
                .iter()
                .map(|ws| {
                    let (fr, fc) = bo_sheet::viewport::frozen_counts(ws);
                    let quad = |r: &bo_model::sheet::CellRange| {
                        [r.first.row, r.first.col, r.last.row, r.last.col]
                    };
                    SheetInfo {
                        name: ws.name.clone(),
                        last_row: ws.cells.keys().next_back().map(|k| k.0),
                        last_col: ws.cells.keys().map(|k| k.1).max(),
                        frozen_rows: fr,
                        frozen_cols: fc,
                        hidden: !matches!(ws.state, bo_model::sheet::SheetState::Visible),
                        hidden_rows: bo_sheet::rowcol::hidden_spans(ws, true),
                        hidden_cols: bo_sheet::rowcol::hidden_spans(ws, false),
                        auto_filter: ws.auto_filter.as_ref().map(|a| quad(&a.range)),
                        merges: ws.merges.iter().take(10_000).map(quad).collect(),
                        charts: ws.charts.len(),
                        pivots: ws.pivots.len(),
                    }
                })
                .collect(),
            macros: self.macros,
            legacy_converted: self.legacy_converted,
        })
    }

    /// Geometry of a sheet, cached until the sheet changes.
    fn geometry(&mut self, sheet: usize) -> Result<&SheetGeometry> {
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let ws = wb
            .sheets
            .get(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let slot = self
            .geometry
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        if slot.is_none() {
            *slot = Some(SheetGeometry::new(ws, &wb.styles, &mut self.fonts));
        }
        slot.as_ref().ok_or(EngineError::NoDocument)
    }

    fn invalidate(&mut self, sheet: Option<usize>) {
        if let Some(i) = sheet {
            if let Some(g) = self.geometry.get_mut(i) {
                *g = None;
            }
        } else {
            let n = self.wb.as_ref().map_or(0, |w| w.sheets.len());
            self.geometry = (0..n).map(|_| None).collect();
        }
    }

    /// Display list of the visible part of a sheet.
    pub fn render(&mut self, req: &ViewRequest) -> Result<PageList> {
        self.geometry(req.sheet)?;
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let geom = self
            .geometry
            .get(req.sheet)
            .and_then(Option::as_ref)
            .ok_or(EngineError::NoDocument)?;
        let opts = RenderOptions {
            locale: self.locale,
            now: self.live.now(),
            drawings: self.drawings.clone(),
            ..RenderOptions::default()
        };
        Ok(render_viewport(
            wb,
            req.sheet,
            geom,
            &req.viewport(),
            &opts,
            &mut self.fonts,
        ))
    }

    pub fn render_json(&mut self, req: &ViewRequest) -> Result<String> {
        Ok(serde_json::to_string(&self.render(req)?).unwrap_or_default())
    }

    fn layout(&mut self, req: &ViewRequest) -> Result<(ViewLayout, &SheetGeometry)> {
        self.geometry(req.sheet)?;
        let ws = self
            .wb
            .as_ref()
            .and_then(|w| w.sheets.get(req.sheet))
            .ok_or(EngineError::NoDocument)?;
        let geom = self
            .geometry
            .get(req.sheet)
            .and_then(Option::as_ref)
            .ok_or(EngineError::NoDocument)?;
        Ok((ViewLayout::for_sheet(geom, ws, &req.viewport()), geom))
    }

    /// What is under a viewport point (header edges within 3 pt of a border,
    /// AutoFilter buttons and charts included).
    pub fn hit_test(&mut self, req: &ViewRequest, x: f64, y: f64) -> Result<HitInfo> {
        const EDGE: f64 = 3.0;
        let (charts, filter) = {
            let ws = self.sheet(req.sheet)?;
            (
                ws.charts.iter().map(|c| (c.from, c.to)).collect::<Vec<_>>(),
                ws.auto_filter.as_ref().map(|a| a.range),
            )
        };
        let (layout, geom) = self.layout(req)?;
        let hit = layout.hit_test(geom, x, y);
        // Charts are drawn above the cells.
        if matches!(hit, Hit::Cell(_)) {
            for (i, (from, to)) in charts.iter().enumerate().rev() {
                let to = to.unwrap_or(bo_model::sheet::CellRef::new(
                    from.row.saturating_add(15),
                    from.col.saturating_add(8),
                ));
                let (Some(a), Some(b)) = (
                    layout.cell_screen_rect(geom, from.row, from.col),
                    layout.cell_screen_rect(geom, to.row, to.col),
                ) else {
                    continue;
                };
                if x >= a.x && x < b.x && y >= a.y && y < b.y {
                    let Hit::Cell(under) = hit else { break };
                    return Ok(HitInfo::Chart {
                        index: i,
                        row: under.row,
                        col: under.col,
                    });
                }
            }
        }
        Ok(match hit {
            Hit::Cell(r) => {
                let button = filter.is_some_and(|f| {
                    f.first.row == r.row && (f.first.col..=f.last.col).contains(&r.col)
                });
                let rect = layout.cell_screen_rect(geom, r.row, r.col);
                match rect {
                    Some(rc)
                        if button && x >= rc.x + rc.w - (rc.h - 3.0).clamp(6.0, 12.0) - 2.0 =>
                    {
                        HitInfo::FilterButton {
                            row: r.row,
                            col: r.col,
                        }
                    }
                    _ => HitInfo::Cell {
                        row: r.row,
                        col: r.col,
                    },
                }
            }
            Hit::ColumnHeader(c) => {
                let rc = layout.cell_screen_rect(geom, layout.frozen_rows, c);
                match rc {
                    Some(rc) if (x - (rc.x + rc.w)).abs() <= EDGE => HitInfo::ColumnEdge { col: c },
                    Some(rc) if (x - rc.x).abs() <= EDGE && c > 0 => {
                        let mut p = c - 1;
                        while p > 0 && geom.col_width(p) <= 0.0 {
                            p -= 1;
                        }
                        HitInfo::ColumnEdge { col: p }
                    }
                    _ => HitInfo::ColumnHeader { col: c },
                }
            }
            Hit::RowHeader(r) => {
                let rc = layout.cell_screen_rect(geom, r, layout.frozen_cols);
                match rc {
                    Some(rc) if (y - (rc.y + rc.h)).abs() <= EDGE => HitInfo::RowEdge { row: r },
                    Some(rc) if (y - rc.y).abs() <= EDGE && r > 0 => {
                        let mut p = r - 1;
                        while p > 0 && geom.row_height(p) <= 0.0 {
                            p -= 1;
                        }
                        HitInfo::RowEdge { row: p }
                    }
                    _ => HitInfo::RowHeader { row: r },
                }
            }
            Hit::Corner => HitInfo::Corner,
            Hit::None => HitInfo::None,
        })
    }

    /// Screen rectangle of a range (for the selection overlay); `None` when off-screen.
    pub fn range_rect(
        &mut self,
        req: &ViewRequest,
        (r0, c0): (u32, u32),
        (r1, c1): (u32, u32),
    ) -> Result<Option<ScreenRect>> {
        let (layout, geom) = self.layout(req)?;
        let a = layout.cell_screen_rect(geom, r0.min(r1), c0.min(c1));
        let b = layout.cell_screen_rect(geom, r0.max(r1), c0.max(c1));
        Ok(match (a, b) {
            (Some(a), Some(b)) => Some(ScreenRect {
                x: a.x,
                y: a.y,
                w: (b.x + b.w - a.x).max(0.0),
                h: (b.y + b.h - a.y).max(0.0),
            }),
            (Some(r), None) | (None, Some(r)) => Some(ScreenRect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            (None, None) => None,
        })
    }

    /// Sheet-space size of the used range plus a margin (for scrollbars).
    pub fn content_size(&mut self, sheet: usize) -> Result<(f64, f64)> {
        let ws = self.sheet(sheet)?;
        let last_row = ws.cells.keys().map(|k| k.0).max().unwrap_or(0);
        let last_col = ws.cells.keys().map(|k| k.1).max().unwrap_or(0);
        let geom = self.geometry(sheet)?;
        let r = (last_row + 100).min(MAX_ROWS - 1);
        let c = (last_col + 30).min(MAX_COLS - 1);
        Ok((
            geom.col_x(c) + geom.col_width(c),
            geom.row_y(r) + geom.row_height(r),
        ))
    }

    /// Formula-bar contents of a cell.
    pub fn cell(&self, sheet: usize, row: u32, col: u32) -> Result<CellDetails> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let address = CellRef { row, col }.to_string();
        let Some(cell) = ws.cell(row, col) else {
            return Ok(CellDetails {
                address,
                input: String::new(),
                display: String::new(),
            });
        };
        let fmt_id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
        let input = match (&cell.formula, &cell.value) {
            (Some(f), _) if !f.text.is_empty() => format!("={}", f.text),
            (_, CellValue::Number(n)) => date_input(*n, fmt_id, &self.locale, wb.date1904)
                .unwrap_or_else(|| number_input(*n, &self.locale)),
            (_, CellValue::Bool(b)) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
            (_, CellValue::Text(t)) => t.clone(),
            (_, CellValue::RichText(runs)) => runs.iter().map(|r| r.text.as_str()).collect(),
            (_, CellValue::Error(e)) => e.as_str().to_owned(),
            (_, _) => String::new(),
        };
        Ok(CellDetails {
            address,
            input,
            display: format_cell(wb, cell, &self.locale).text,
        })
    }

    /// Sets what the user typed into a cell (one undo step).
    pub fn set_input(&mut self, sheet: usize, row: u32, col: u32, input: &str) -> Result<()> {
        self.set_inputs(sheet, &[(row, col, input.to_owned())])
    }

    /// Sets several cells as one undo step (paste, fill).
    pub fn set_inputs(&mut self, sheet: usize, cells: &[(u32, u32, String)]) -> Result<()> {
        self.set_inputs_checked(sheet, cells, false).map(|_| ())
    }

    /// [`SheetEngine::set_inputs`] that first checks data validation
    /// (§18.3.1.32) when `validate` is set: a `stop` rule refuses the whole
    /// edit (returned, nothing changes); `warning` / `information` rules are
    /// returned after the edit is applied.
    pub fn set_inputs_checked(
        &mut self,
        sheet: usize,
        cells: &[(u32, u32, String)],
        validate: bool,
    ) -> Result<Option<bo_sheet::validation::Rejection>> {
        let locale = self.locale;
        let mut notice = None;
        if validate {
            let wb = self.wb()?;
            let ws = self.sheet(sheet)?;
            for (row, col, input) in cells {
                let cand = input_to_cell_typed(input, ws.cell(*row, *col), &locale, wb.date1904).0;
                let value = cand.map_or(CellValue::Empty, |c| {
                    if c.formula.is_some() {
                        CellValue::Empty
                    } else {
                        c.value
                    }
                });
                if input.starts_with('=') {
                    continue;
                }
                if let Err(r) = bo_sheet::validation::check(
                    wb,
                    sheet,
                    *row,
                    *col,
                    &value,
                    &locale,
                    self.live.now(),
                ) {
                    if r.blocks() {
                        return Ok(Some(r));
                    }
                    notice = Some(r);
                }
            }
        }
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if sheet >= wb.sheets.len() {
            return Err(EngineError::PageOutOfRange(sheet));
        }
        let date1904 = wb.date1904;
        let Workbook { sheets, styles, .. } = &mut *wb;
        let ws = &mut sheets[sheet];
        let mut old = Vec::with_capacity(cells.len());
        for (row, col, input) in cells {
            if *row >= MAX_ROWS || *col >= MAX_COLS {
                continue;
            }
            let prev = ws.cells.get(&(*row, *col)).cloned();
            // Text-formatted cells (§18.8.30 id 49 `@`) keep what was typed.
            let text_fmt = prev
                .as_ref()
                .and_then(|c| styles.xf(c.style))
                .is_some_and(|x| x.num_fmt_id == 49);
            let (cell, date_fmt) = if text_fmt {
                (input_to_cell(input, prev.as_ref(), &locale), None)
            } else {
                input_to_cell_typed(input, prev.as_ref(), &locale, date1904)
            };
            match cell {
                Some(mut c) => {
                    if let Some(id) = date_fmt {
                        c.style = date_style(styles, c.style, id);
                    }
                    ws.cells.insert((*row, *col), c);
                }
                None => {
                    ws.cells.remove(&(*row, *col));
                }
            }
            old.push((*row, *col, prev));
        }
        let coords: Vec<(u32, u32)> = old.iter().map(|(r, c, _)| (*r, *c)).collect();
        self.live.update(wb, sheet, &coords);
        self.push_undo(UndoEntry::Cells { sheet, cells: old });
        Ok(notice)
    }

    fn push_undo(&mut self, e: UndoEntry) {
        self.undo.push(e);
        self.redo.clear();
    }

    /// Inserts (`count > 0`) rows or columns at `at`, or deletes them (`delete = true`).
    pub fn structural(
        &mut self,
        sheet: usize,
        rows: bool,
        at: u32,
        count: u32,
        delete: bool,
    ) -> Result<()> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if sheet >= wb.sheets.len() || count == 0 {
            return Ok(());
        }
        let before = Box::new(wb.clone());
        let adj: &dyn FormulaAdjuster = &self.adjuster;
        match (rows, delete) {
            (true, false) => bo_sheet::ops::insert_rows(wb, sheet, at, count, adj),
            (true, true) => bo_sheet::ops::delete_rows(wb, sheet, at, count, adj),
            (false, false) => bo_sheet::ops::insert_cols(wb, sheet, at, count, adj),
            (false, true) => bo_sheet::ops::delete_cols(wb, sheet, at, count, adj),
        }
        .map_err(|e| EngineError::Sheet(e.to_string()))?;
        self.live.rebuild(wb);
        self.invalidate(Some(sheet));
        self.push_undo(UndoEntry::Workbook(before));
        if let Some(c) = self.collab.as_mut() {
            c.pending.push(collab::LocalStruct {
                sheet,
                rows,
                at,
                count,
                delete,
            });
        }
        Ok(())
    }

    fn apply(&mut self, e: UndoEntry) -> Result<UndoEntry> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let mut touched: Option<(usize, Vec<(u32, u32)>)> = None;
        let inverse = match e {
            UndoEntry::Cells { sheet, cells } => {
                touched = Some((sheet, cells.iter().map(|(r, c, _)| (*r, *c)).collect()));
                let ws = wb
                    .sheets
                    .get_mut(sheet)
                    .ok_or(EngineError::PageOutOfRange(sheet))?;
                let mut inv = Vec::with_capacity(cells.len());
                for (r, c, cell) in cells.into_iter().rev() {
                    let cur = match cell {
                        Some(cell) => ws.cells.insert((r, c), cell),
                        None => ws.cells.remove(&(r, c)),
                    };
                    inv.push((r, c, cur));
                }
                UndoEntry::Cells { sheet, cells: inv }
            }
            UndoEntry::Workbook(prev) => {
                let cur = std::mem::replace(wb, *prev);
                self.invalidate(None);
                UndoEntry::Workbook(Box::new(cur))
            }
        };
        if let Some(wb) = self.wb.as_mut() {
            match touched {
                Some((sheet, cells)) => self.live.update(wb, sheet, &cells),
                None => self.live.rebuild(wb),
            }
        }
        Ok(inverse)
    }

    pub fn undo(&mut self) -> Result<bool> {
        let Some(e) = self.undo.pop() else {
            return Ok(false);
        };
        let inv = self.apply(e)?;
        self.redo.push(inv);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool> {
        let Some(e) = self.redo.pop() else {
            return Ok(false);
        };
        let inv = self.apply(e)?;
        self.undo.push(inv);
        Ok(true)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Adds a sheet at the end; returns its index.
    pub fn add_sheet(&mut self, name: Option<&str>) -> Result<usize> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let before = Box::new(wb.clone());
        let mut n = wb.sheets.len() + 1;
        let name = match name {
            Some(s) if !s.trim().is_empty() => s.trim().to_owned(),
            _ => loop {
                let cand = format!("Sheet{n}");
                if wb
                    .sheets
                    .iter()
                    .all(|s| !s.name.eq_ignore_ascii_case(&cand))
                {
                    break cand;
                }
                n += 1;
            },
        };
        if wb.sheets.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
            return Err(EngineError::Sheet(format!("sheet '{name}' already exists")));
        }
        let mut ws = Worksheet::new(&name);
        ws.sheet_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
        wb.sheets.push(ws);
        let idx = wb.sheets.len() - 1;
        self.live.rebuild(wb);
        self.geometry.push(None);
        self.push_undo(UndoEntry::Workbook(before));
        Ok(idx)
    }

    /// Renames a sheet (ECMA-376 §18.2.19: ≤ 31 chars, no `[]:*?/\`).
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 31
            || name.contains(['[', ']', ':', '*', '?', '/', '\\'])
        {
            return Err(EngineError::Sheet(format!("invalid sheet name '{name}'")));
        }
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if wb
            .sheets
            .iter()
            .enumerate()
            .any(|(i, s)| i != sheet && s.name.eq_ignore_ascii_case(name))
        {
            return Err(EngineError::Sheet(format!("sheet '{name}' already exists")));
        }
        let before = Box::new(wb.clone());
        let ws = wb
            .sheets
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let old = std::mem::replace(&mut ws.name, name.to_owned());
        // Pivot caches follow their source sheet (§18.10.1 worksheetSource/@sheet).
        for c in &mut wb.pivot_caches {
            if c.source_sheet
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(&old))
            {
                c.source_sheet = Some(name.to_owned());
            }
        }
        self.live.rebuild(wb);
        self.push_undo(UndoEntry::Workbook(before));
        Ok(())
    }

    /// Deletes a sheet (a workbook keeps at least one).
    pub fn delete_sheet(&mut self, sheet: usize) -> Result<()> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if wb.sheets.len() <= 1 || sheet >= wb.sheets.len() {
            return Err(EngineError::Sheet("cannot delete the last sheet".into()));
        }
        let before = Box::new(wb.clone());
        wb.sheets.remove(sheet);
        self.live.rebuild(wb);
        self.invalidate(None);
        self.push_undo(UndoEntry::Workbook(before));
        Ok(())
    }

    /// Serialises the workbook (`xlsx` or `csv` of `sheet`). Chart caches are
    /// refreshed from their ranges first (§21.2.2.120 `numCache`).
    pub fn save(&mut self, format: &str, sheet: usize) -> Result<Vec<u8>> {
        let locale = self.locale;
        if format == "xlsx" {
            if let Some(wb) = self.wb.as_mut() {
                if wb
                    .sheets
                    .iter()
                    .any(|s| s.charts.iter().any(|c| c.dirty || c.part.is_empty()))
                {
                    bo_sheet::chart::refresh_caches(wb, &locale);
                }
            }
        }
        let wb = self.wb()?;
        match format {
            "xlsx" => bo_xlsx::write_xlsx(wb).map_err(|e| EngineError::Sheet(e.to_string())),
            "csv" => {
                let opts = if self.locale.decimal == ',' {
                    bo_xlsx::CsvOptions::vi_vn()
                } else {
                    bo_xlsx::CsvOptions::default()
                };
                Ok(bo_xlsx::write_csv(self.sheet(sheet)?, &opts))
            }
            other => Err(EngineError::Unsupported(other.to_owned())),
        }
    }

    /// Glyph outlines for the canvas (same as the document engine).
    #[must_use]
    pub fn glyph_outlines_json(&self, face: u32, glyphs: &[u16]) -> String {
        serde_json::to_string(&self.glyph_outlines(face, glyphs)).unwrap_or_else(|_| "[]".into())
    }

    /// Outlines as structs (tests, native hosts).
    #[must_use]
    pub fn glyph_outlines(&self, face: u32, glyphs: &[u16]) -> Vec<GlyphOutline> {
        if face as usize >= self.fonts.len() {
            return Vec::new();
        }
        let id = bo_text::FaceId(face);
        let upem = self.fonts.info(id).units_per_em;
        glyphs
            .iter()
            .map(|&g| GlyphOutline {
                face,
                glyph: g,
                units_per_em: upem,
                path: bo_text::outline_svg_path(&self.fonts.outline(id, g)),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SheetEngine {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Book1.xlsx").unwrap();
        e
    }

    #[test]
    fn parses_vietnamese_and_english_numbers() {
        let vi = Locale::VI_VN;
        assert_eq!(parse_number("1.234,5", &vi), Some(1234.5));
        assert_eq!(parse_number("12,5%", &vi), Some(0.125));
        assert_eq!(parse_number("-3", &vi), Some(-3.0));
        assert_eq!(parse_number("abc", &vi), None);
        assert_eq!(parse_number("1,234.5", &Locale::EN_US), Some(1234.5));
    }

    #[test]
    fn input_round_trips_through_formula_bar() {
        let mut e = engine();
        e.set_input(0, 0, 0, "1.234,5").unwrap();
        e.set_input(0, 1, 0, "=SUM(A1:A1)").unwrap();
        e.set_input(0, 2, 0, "Hà Nội").unwrap();
        e.set_input(0, 3, 0, "đúng").unwrap();
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "1234,5");
        assert_eq!(e.cell(0, 1, 0).unwrap().input, "=SUM(A1:A1)");
        assert_eq!(e.cell(0, 2, 0).unwrap().display, "Hà Nội");
        assert_eq!(e.cell(0, 3, 0).unwrap().input, "TRUE");
        assert_eq!(e.cell(0, 2, 0).unwrap().address, "A3");
        e.set_input(0, 2, 0, "").unwrap();
        assert_eq!(e.info().unwrap().sheets[0].last_row, Some(3));
        assert!(e.workbook().unwrap().sheets[0].cell(2, 0).is_none());
    }

    #[test]
    fn formulas_recalculate_and_follow_structure() {
        let mut e = engine();
        e.set_input(0, 0, 0, "2").unwrap();
        e.set_input(0, 1, 0, "3").unwrap();
        e.set_input(0, 2, 0, "=SUM(A1:A2)*2").unwrap();
        assert_eq!(e.cell(0, 2, 0).unwrap().display, "10");
        e.set_input(0, 0, 0, "5").unwrap();
        assert_eq!(e.cell(0, 2, 0).unwrap().display, "16");
        e.structural(0, true, 0, 1, false).unwrap();
        assert_eq!(e.cell(0, 3, 0).unwrap().input, "=SUM(A2:A3)*2");
        assert_eq!(e.cell(0, 3, 0).unwrap().display, "16");
        e.set_now_ms(1_793_000_000_000);
        e.set_input(0, 5, 0, "=YEAR(TODAY())").unwrap();
        assert_eq!(e.cell(0, 5, 0).unwrap().display, "2026");
    }

    #[test]
    fn undo_redo_cells_and_structure() {
        let mut e = engine();
        e.set_input(0, 0, 0, "1").unwrap();
        e.set_input(0, 0, 0, "2").unwrap();
        e.structural(0, true, 0, 2, false).unwrap();
        assert_eq!(e.cell(0, 2, 0).unwrap().input, "2");
        assert!(e.undo().unwrap());
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "2");
        assert!(e.undo().unwrap());
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "1");
        assert!(e.redo().unwrap());
        assert!(e.redo().unwrap());
        assert_eq!(e.cell(0, 2, 0).unwrap().input, "2");
        assert!(!e.redo().unwrap());
    }

    #[test]
    fn renders_hit_tests_and_saves() {
        let mut e = engine();
        e.set_input(0, 1, 1, "Xin chào").unwrap();
        let req = ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 400.0,
            height: 300.0,
        };
        let page = e.render(&req).unwrap();
        assert_ne!(page.commands.len(), 0);
        let r = e.range_rect(&req, (1, 1), (1, 1)).unwrap().unwrap();
        assert_eq!(
            e.hit_test(&req, r.x + 1.0, r.y + 1.0).unwrap(),
            HitInfo::Cell { row: 1, col: 1 }
        );
        assert_eq!(e.hit_test(&req, 1.0, 1.0).unwrap(), HitInfo::Corner);
        let bytes = e.save("xlsx", 0).unwrap();
        let mut back = SheetEngine::with_bundled_fonts();
        back.open(&bytes, "x.xlsx").unwrap();
        assert_eq!(back.cell(0, 1, 1).unwrap().input, "Xin chào");
        let csv = e.save("csv", 0).unwrap();
        assert!(String::from_utf8_lossy(&csv).contains("Xin chào"));
        assert!(e.export_pdf().unwrap().starts_with(b"%PDF"));
    }

    #[test]
    fn opens_ods_and_rejects_garbage() {
        let mut e = engine();
        e.set_input(0, 0, 0, "Hà Nội").unwrap();
        e.set_input(0, 0, 1, "1,5").unwrap();
        let ods = bo_odf::write_ods(&bo_odf::SpreadsheetDoc {
            sheets: vec![bo_odf::Sheet {
                name: "Bảng 1".into(),
                cells: vec![
                    bo_odf::Cell {
                        row: 0,
                        col: 0,
                        value: bo_odf::CellValue::String("Hà Nội".into()),
                        formula: None,
                    },
                    bo_odf::Cell {
                        row: 1,
                        col: 1,
                        value: bo_odf::CellValue::Float(2.5),
                        formula: None,
                    },
                ],
            }],
        })
        .unwrap();
        let info = e.open(&ods, "a.ods").unwrap();
        assert_eq!(info.sheets[0].name, "Bảng 1");
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "Hà Nội");
        assert_eq!(e.cell(0, 1, 1).unwrap().input, "2,5");
        assert!(e.open(b"not a workbook", "x.xls").is_err());
    }

    #[test]
    fn sheets_add_rename_delete() {
        let mut e = engine();
        assert_eq!(e.add_sheet(None).unwrap(), 1);
        e.rename_sheet(1, "Doanh thu").unwrap();
        assert!(e.rename_sheet(0, "doanh thu").is_err());
        assert!(e.rename_sheet(0, "a/b").is_err());
        e.delete_sheet(0).unwrap();
        assert_eq!(e.info().unwrap().sheets[0].name, "Doanh thu");
        assert!(e.delete_sheet(0).is_err());
        assert!(e.undo().unwrap());
        assert_eq!(e.info().unwrap().sheets.len(), 2);
    }
}

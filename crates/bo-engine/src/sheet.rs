//! BlueSheet engine facade: one workbook, rendered viewport by viewport.
//!
//! Same contract as the document [`Engine`](crate::Engine): every host (WASM
//! worker, Tauri, CLI) calls these functions, results cross the boundary as
//! JSON, and the canvas draws the `bo-render` page list it gets back. Sheet
//! coordinates and viewport sizes are in points.

use crate::{EngineError, GlyphOutline, Result};
use bo_model::sheet::{Cell, CellRef, CellValue, Formula, Workbook, Worksheet};
use bo_render::{Command, DisplayList, PageList};
use bo_sheet::calc::{FormulaAdjuster, FormulaEngine};
use bo_sheet::formula::BoFormula;
use bo_sheet::geometry::SheetGeometry;
use bo_sheet::numfmt::Locale;
use bo_sheet::render::{format_cell, render_viewport, RenderOptions};
use bo_sheet::viewport::{Hit, ViewLayout, Viewport};
use bo_text::FontManager;
use serde::Serialize;

/// Hard limits of the grid (ECMA-376 Part 1, §18.3.1.73 `row/@r` ≤ 1 048 576; 16 384 columns).
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

/// Summary of an open workbook.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookInfo {
    pub name: String,
    pub sheets: Vec<SheetInfo>,
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
}

/// Where a screen point falls (viewport-relative, points).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HitInfo {
    Cell { row: u32, col: u32 },
    ColumnHeader { col: u32 },
    RowHeader { row: u32 },
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

/// Moves a viewport page onto a larger page at `(dx, dy)`, clipped to its own size.
fn translate_page(page: &mut PageList, dx: f32, dy: f32, width: f32, height: f32) {
    // Clip to the viewport, then shift everything with one transform.
    let mut commands = Vec::with_capacity(page.commands.len() + 4);
    commands.push(Command::PushClip {
        x: dx,
        y: dy,
        w: page.width,
        h: page.height,
    });
    commands.push(Command::PushTransform {
        m: [1.0, 0.0, 0.0, 1.0, dx, dy],
    });
    commands.append(&mut page.commands);
    commands.push(Command::PopTransform);
    commands.push(Command::PopClip);
    page.commands = commands;
    page.width = width;
    page.height = height;
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
    calc: Box<dyn FormulaEngine + Send>,
    adjuster: Box<dyn FormulaAdjuster + Send>,
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
            calc: Box::new(BoFormula::new()),
            adjuster: Box::new(BoFormula::new()),
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

    /// Plugs in a formula engine (recalculation + reference adjustment).
    pub fn set_formulas(
        &mut self,
        calc: Box<dyn FormulaEngine + Send>,
        adjuster: Box<dyn FormulaAdjuster + Send>,
    ) {
        self.calc = calc;
        self.adjuster = adjuster;
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
        self.calc = Box::new(BoFormula { now: Some(serial) });
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
        self.load(wb, name)
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
        self.calc.recalculate(&mut wb);
        self.geometry = (0..wb.sheets.len()).map(|_| None).collect();
        self.wb = Some(wb);
        self.name = name.to_owned();
        self.undo.clear();
        self.redo.clear();
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
                    SheetInfo {
                        name: ws.name.clone(),
                        last_row: ws.cells.keys().map(|k| k.0).max(),
                        last_col: ws.cells.keys().map(|k| k.1).max(),
                        frozen_rows: fr,
                        frozen_cols: fc,
                        hidden: !matches!(ws.state, bo_model::sheet::SheetState::Visible),
                    }
                })
                .collect(),
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

    /// What is under a viewport point.
    pub fn hit_test(&mut self, req: &ViewRequest, x: f64, y: f64) -> Result<HitInfo> {
        let (layout, geom) = self.layout(req)?;
        Ok(match layout.hit_test(geom, x, y) {
            Hit::Cell(r) => HitInfo::Cell {
                row: r.row,
                col: r.col,
            },
            Hit::ColumnHeader(c) => HitInfo::ColumnHeader { col: c },
            Hit::RowHeader(r) => HitInfo::RowHeader { row: r },
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
        let input = match (&cell.formula, &cell.value) {
            (Some(f), _) if !f.text.is_empty() => format!("={}", f.text),
            (_, CellValue::Number(n)) => number_input(*n, &self.locale),
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
        let locale = self.locale;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let ws = wb
            .sheets
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let mut old = Vec::with_capacity(cells.len());
        for (row, col, input) in cells {
            if *row >= MAX_ROWS || *col >= MAX_COLS {
                continue;
            }
            let prev = ws.cells.get(&(*row, *col)).cloned();
            match input_to_cell(input, prev.as_ref(), &locale) {
                Some(c) => {
                    ws.cells.insert((*row, *col), c);
                }
                None => {
                    ws.cells.remove(&(*row, *col));
                }
            }
            old.push((*row, *col, prev));
        }
        self.calc.recalculate(wb);
        self.push_undo(UndoEntry::Cells { sheet, cells: old });
        Ok(())
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
        let adj: &dyn FormulaAdjuster = self.adjuster.as_ref();
        match (rows, delete) {
            (true, false) => bo_sheet::ops::insert_rows(wb, sheet, at, count, adj),
            (true, true) => bo_sheet::ops::delete_rows(wb, sheet, at, count, adj),
            (false, false) => bo_sheet::ops::insert_cols(wb, sheet, at, count, adj),
            (false, true) => bo_sheet::ops::delete_cols(wb, sheet, at, count, adj),
        }
        .map_err(|e| EngineError::Sheet(e.to_string()))?;
        self.calc.recalculate(wb);
        self.invalidate(Some(sheet));
        self.push_undo(UndoEntry::Workbook(before));
        Ok(())
    }

    fn apply(&mut self, e: UndoEntry) -> Result<UndoEntry> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let inverse = match e {
            UndoEntry::Cells { sheet, cells } => {
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
            self.calc.recalculate(wb);
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
        wb.sheets
            .get_mut(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?
            .name = name.to_owned();
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
        self.invalidate(None);
        self.push_undo(UndoEntry::Workbook(before));
        Ok(())
    }

    /// Serialises the workbook (`xlsx` or `csv` of `sheet`).
    pub fn save(&self, format: &str, sheet: usize) -> Result<Vec<u8>> {
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

    /// PDF of the used range of every visible sheet, A4 portrait pages.
    pub fn export_pdf(&mut self) -> Result<Vec<u8>> {
        const PAGE_W: f64 = 595.0;
        const PAGE_H: f64 = 842.0;
        const MARGIN: f64 = 36.0;
        let n = self.wb()?.sheets.len();
        let mut pages = Vec::new();
        for sheet in 0..n {
            if self.info()?.sheets.get(sheet).is_some_and(|s| s.hidden) {
                continue;
            }
            let (w, h) = {
                let ws = self.sheet(sheet)?;
                let (Some(lr), Some(lc)) = (
                    ws.cells.keys().map(|k| k.0).max(),
                    ws.cells.keys().map(|k| k.1).max(),
                ) else {
                    continue;
                };
                let g = self.geometry(sheet)?;
                (
                    g.col_x(lc) + g.col_width(lc),
                    g.row_y(lr) + g.row_height(lr),
                )
            };
            let (cw, ch) = (PAGE_W - 2.0 * MARGIN, PAGE_H - 2.0 * MARGIN);
            let mut y = 0.0;
            while y < h {
                let mut x = 0.0;
                while x < w {
                    let mut page = self.render(&ViewRequest {
                        sheet,
                        scroll_x: x,
                        scroll_y: y,
                        width: cw,
                        height: ch,
                    })?;
                    #[allow(clippy::cast_possible_truncation)]
                    translate_page(
                        &mut page,
                        MARGIN as f32,
                        MARGIN as f32,
                        PAGE_W as f32,
                        PAGE_H as f32,
                    );
                    pages.push(page);
                    x += cw;
                }
                y += ch;
            }
        }
        let list = DisplayList { pages };
        bo_pdf::write_pdf(
            &list,
            &self.fonts,
            &bo_pdf::PdfMeta {
                title: Some(self.name.clone()),
            },
        )
        .map_err(EngineError::from)
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

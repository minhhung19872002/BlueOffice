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

mod analysis_cmd;
mod audit_cmd;
mod cell_media_cmd;
mod chart_design_cmd;
mod cmd;
pub(crate) mod collab;
mod comments;
mod compare_cmd;
mod datatools_cmd;
mod formctl_cmd;
mod inspect_cmd;
mod light_undo;
pub mod model_cmd;
mod ole_cmd;
mod paste_opts;
mod pivot_cmd;
mod png;
mod print;
mod print_cmd;
mod print_job;
pub mod protect;
pub mod query_cmd;
mod scenario_cmd;
mod security_cmd;
mod smartart_cmd;
mod solver_cmd;
mod sortfilter_cmd;
mod sparkline_cmd;
mod spell_cmd;
mod table_cmd;
mod theme_file;
mod view_cmd;

pub use cmd::{Clip, PasteMode, SheetCommand};

/// Hard limits of the grid (ECMA-376 Part 1, §18.3.1.73 `row/@r` ≤ 1 048 576; 16 384 columns).
pub const MAX_ROWS: u32 = 1_048_576;
/// Maximum number of columns (`XFD`).
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
            // A new or edited diagram (CORE-SMART-005) is drawn from its own parts.
            let Some(xml) = d.xml.as_ref().map(|x| x.drawing.as_slice()).or_else(|| {
                wb.opaque
                    .parts
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&d.part))
                    .map(|p| p.data.as_slice())
            }) else {
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
            if let Some(commands) = bo_pptx::diagram::render_drawing(xml, &d.part, we, he, fonts) {
                out.insert(d.part.clone(), DrawingOverlay { w, h, commands });
            }
        }
    }
    out
}

/// Fingerprint of the SmartArt diagrams of a workbook (anchors, parts and new / edited drawings).
fn diagrams_key(wb: &Workbook) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (i, ws) in wb.sheets.iter().enumerate() {
        for d in &ws.diagrams {
            (i, &d.part, d.from.row, d.from.col).hash(&mut h);
            d.to.map(|t| (t.row, t.col)).hash(&mut h);
            d.xml.as_ref().map(|x| &x.drawing).hash(&mut h);
        }
    }
    h.finish()
}

/// Summary of an open workbook.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookInfo {
    /// File name of the workbook.
    pub name: String,
    /// Every sheet, in tab order.
    pub sheets: Vec<SheetInfo>,
    /// The file is saved encrypted with a password ([MS-OFFCRYPTO]).
    pub encrypted: bool,
    /// The file carries a VBA project or macro sheets (kept, never run).
    pub macros: bool,
    /// `workbookProtection/@lockStructure` (ECMA-376 Part 1, §18.2.29).
    pub structure_locked: bool,
    /// Cells converted from TCVN3 / VNI fonts to Unicode on open (0 = none);
    /// the conversion is the first undo step.
    pub legacy_converted: usize,
    /// Package variant (`xlsx`, `xlsm`, `xltx`, `xltm`) the next `.xlsx` save produces
    /// — the opened file's unless `saveFormat` changed it (CORE-FMT-001).
    pub variant: Option<String>,
}

/// Summary of one sheet (tab bar, navigation and frozen panes).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    /// Sheet (tab) name.
    pub name: String,
    /// Used range: last row / column with content (0-based), `None` when empty.
    pub last_row: Option<u32>,
    /// Last used column (0-based), `None` when empty.
    pub last_col: Option<u32>,
    /// Number of frozen rows at the top.
    pub frozen_rows: u32,
    /// Number of frozen columns at the left.
    pub frozen_cols: u32,
    /// The sheet is hidden or very hidden.
    pub hidden: bool,
    /// Hidden row spans `[first, last]` (filtered or hidden rows), for navigation.
    pub hidden_rows: Vec<(u32, u32)>,
    /// Hidden column spans `[first, last]`, for navigation.
    pub hidden_cols: Vec<(u32, u32)>,
    /// AutoFilter range `[r0, c0, r1, c1]`.
    pub auto_filter: Option<[u32; 4]>,
    /// Merged ranges `[r0, c0, r1, c1]`.
    pub merges: Vec<[u32; 4]>,
    /// Number of charts on the sheet.
    pub charts: usize,
    /// Number of pivot tables on the sheet.
    pub pivots: usize,
    /// `sheetProtection/@sheet` (§18.3.1.85).
    pub protected: bool,
    /// `sheetPr/tabColor` as `RRGGBB` (§18.3.1.93).
    pub tab_color: Option<String>,
}

/// Where a screen point falls (viewport-relative, points).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HitInfo {
    /// A grid cell.
    Cell {
        /// 0-based row.
        row: u32,
        /// 0-based column.
        col: u32,
    },
    /// A column header.
    ColumnHeader {
        /// 0-based column.
        col: u32,
    },
    /// A row header.
    RowHeader {
        /// 0-based row.
        row: u32,
    },
    /// Near the right edge of column `col` in the header (resize / autofit).
    ColumnEdge {
        /// 0-based column.
        col: u32,
    },
    /// Near the bottom edge of row `row` in the header.
    RowEdge {
        /// 0-based row.
        row: u32,
    },
    /// The AutoFilter button of column `col`.
    FilterButton {
        /// 0-based row of the AutoFilter header cell.
        row: u32,
        /// 0-based column.
        col: u32,
    },
    /// A chart placed on the sheet, and the cell under the point.
    Chart {
        /// 0-based index of the chart in the sheet.
        index: usize,
        /// 0-based row of the cell under the point.
        row: u32,
        /// 0-based column of the cell under the point.
        col: u32,
    },
    /// A SmartArt diagram on the sheet (CORE-SMART-005), and the cell under the point.
    SmartArt {
        /// 0-based index of the diagram in the sheet.
        index: usize,
        /// 0-based row of the cell under the point.
        row: u32,
        /// 0-based column of the cell under the point.
        col: u32,
    },
    /// The top-left corner (select all).
    Corner,
    /// Outside the grid.
    None,
}

/// Screen rectangle (points) of a cell or range in the current viewport.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct ScreenRect {
    /// Left edge in points.
    pub x: f64,
    /// Top edge in points.
    pub y: f64,
    /// Width in points.
    pub w: f64,
    /// Height in points.
    pub h: f64,
}

/// What the formula bar / in-cell editor shows for a cell.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CellDetails {
    /// A1-style address (`B7`).
    pub address: String,
    /// Editable input: `=FORMULA`, a plain number, or text.
    pub input: String,
    /// Formatted display text.
    pub display: String,
}

/// Viewport request from the UI.
#[derive(Debug, Clone, Copy)]
pub struct ViewRequest {
    /// 0-based sheet index.
    pub sheet: usize,
    /// Horizontal scroll offset of the grid, in points.
    pub scroll_x: f64,
    /// Vertical scroll offset of the grid, in points.
    pub scroll_y: f64,
    /// Viewport width in points.
    pub width: f64,
    /// Viewport height in points.
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
    /// Whole workbook before a structural change (insert/delete rows…).
    Workbook(Box<Workbook>),
    /// A sort, AutoFilter or pivot step without a copy of the cells (SHEET-PERF-002).
    Light(Box<light_undo::Light>),
    /// Several entries undone as one step, in the order they were recorded
    /// (a BlueScript run, CORE-SCRIPT-001).
    Group(Vec<UndoEntry>),
}

/// The sheet engine.
pub struct SheetEngine {
    pub(crate) fonts: FontManager,
    pub(crate) name: String,
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
    /// Password the next `.xlsx` saves are encrypted with ([MS-OFFCRYPTO]).
    save_password: Option<String>,
    /// Live collaboration binding (`bo-collab`).
    collab: Option<collab::SheetCollab>,
    /// The open file has macros (warning bar; preserved, never executed).
    macros: bool,
    /// Package variant of the open file (what the writer reproduces, CORE-FMT-001).
    file_variant: Option<crate::variant::Variant>,
    /// Variant the next saves produce when it differs (`saveFormat`).
    save_variant: Option<crate::variant::Variant>,
    /// Cells the last legacy-font conversion changed.
    legacy_converted: usize,
    /// SmartArt drawing fallbacks of the open workbook, drawn on load and when a diagram changes.
    drawings: Arc<BTreeMap<String, DrawingOverlay>>,
    /// Fingerprint of the diagrams `drawings` were drawn for.
    drawings_key: u64,
    /// Package parts the document inspector drops on the next save.
    scrub: crate::inspect::package::Scrub,
    /// Core / extended document properties as read and edited (CORE-UI-002).
    pub(crate) props: crate::props::PropsState,
    /// Final mark, labels and read-only state (CORE-SEC-001).
    pub(crate) sec: crate::security::FileSecurity,
    /// Protected ranges unlocked with their password: (sheet name, range name).
    pub(crate) open_ranges: Vec<(String, String)>,
    /// Names of the current user (password-to-modify user name).
    pub(crate) identity: Vec<String>,
    /// Automatic table behaviour after typing (SHEET-TABLE-001).
    table_auto: table_cmd::TableAuto,
    /// Spelling and grammar checking of text cells (CORE-SPELL-006).
    pub(crate) spell: crate::spell::SpellState,
    /// `IMAGE` pictures fetched by the host (SHEET-CELL-001).
    web_images: cell_media_cmd::WebImages,
    /// Sheets of the last print preview (CORE-PRINT-001).
    print_cache: crate::print::PreviewCache,
    /// PDF/A part, tagged PDF and bookmarks of `export_pdf` (CORE-FMT-002).
    pub(crate) pdf_export: bo_pdf::ExportChoices,
    /// Bytes of Get & Transform sources provided by the host (SHEET-PQ-001).
    query_sources: query_cmd::QuerySources,
}

impl Default for SheetEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// `calcPr/@fullCalcOnLoad` (ECMA-376 Part 1, §18.2.2) of the workbook part.
fn full_calc_on_load(wb: &Workbook) -> bool {
    wb.unknown.iter().any(|x| {
        x.xml.contains("calcPr")
            && (x.xml.contains("fullCalcOnLoad=\"1\"") || x.xml.contains("fullCalcOnLoad=\"true\""))
    })
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

/// A workbook read from file bytes of any spreadsheet format BlueSheet opens (.csv, .xls,
/// .xlsb, .ods, .xlsx/.xlsm); `name` gives the extension where the bytes do not.
pub(crate) fn read_workbook_bytes(bytes: &[u8], name: &str, locale: &Locale) -> Result<Workbook> {
    let lower = name.to_ascii_lowercase();
    let wb = if lower.ends_with(".csv") {
        let opts = if locale.decimal == ',' {
            bo_xlsx::CsvOptions::vi_vn()
        } else {
            bo_xlsx::CsvOptions::default()
        };
        let ws = bo_xlsx::read_csv(bytes, &opts).map_err(|e| EngineError::Sheet(e.to_string()))?;
        let mut wb = Workbook::new();
        wb.sheets = vec![ws];
        wb
    } else if matches!(bo_binary::detect(bytes), Some(bo_binary::LegacyKind::Xls)) {
        from_legacy(&bo_binary::read_xls(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?)
    } else if bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODS) {
        from_ods(&bo_odf::read_ods(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?)
    } else if lower.ends_with(".xlsb") {
        from_legacy(&bo_binary::read_xlsb(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?)
    } else if bytes.starts_with(b"PK\x03\x04") {
        bo_xlsx::read_xlsx(bytes).map_err(|e| EngineError::Sheet(e.to_string()))?
    } else {
        // XML Spreadsheet 2003, HTML tables, MHTML (CORE-FMT-003).
        use crate::web_formats::WebKind;
        match crate::web_formats::detect(bytes, name) {
            Some(WebKind::XmlSpreadsheet) => bo_xlsx::read_xml_spreadsheet(bytes)
                .map_err(|e| EngineError::Sheet(e.to_string()))?,
            Some(WebKind::Html) => bo_html::read_workbook_html(bytes),
            Some(WebKind::Mht) => {
                let archive = bo_html::mht::parse(bytes);
                let root = archive
                    .root
                    .and_then(|i| archive.parts.get(i))
                    .ok_or_else(|| EngineError::Sheet("the MHTML file has no HTML page".into()))?;
                bo_html::sheet::read_workbook_html_str(&bo_encoding::web::decode_html(
                    &root.data,
                    root.charset.as_deref(),
                ))
            }
            Some(WebKind::FlatOpc) => {
                let zip = crate::web_formats::flat_package(bytes)?
                    .ok_or_else(|| EngineError::Unsupported(name.to_owned()))?;
                bo_xlsx::read_xlsx(&zip).map_err(|e| EngineError::Sheet(e.to_string()))?
            }
            None => return Err(EngineError::Unsupported(name.to_owned())),
        }
    };
    Ok(wb)
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
    // Automatic cell styles (ODF 1.3 §16.2) → cell formats (§18.8.45 xf).
    let mut xfs: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for cs in &src.styles {
        let patch = bo_sheet::style::StylePatch {
            bold: cs.bold.then_some(true),
            italic: cs.italic.then_some(true),
            underline: cs.underline.then(|| "single".to_owned()),
            font_name: cs.font.clone(),
            font_size: cs.size,
            font_color: cs.color.clone(),
            fill_color: cs.background.clone(),
            h_align: cs.align.as_deref().map(|a| {
                match a {
                    "center" => "center",
                    "end" | "right" => "right",
                    "justify" => "justify",
                    _ => "left",
                }
                .to_owned()
            }),
            wrap: cs.wrap.then_some(true),
            ..bo_sheet::style::StylePatch::default()
        };
        let xf = bo_sheet::style::patched_xf(&mut wb.styles, 0, &patch);
        xfs.insert(cs.name.as_str(), xf);
    }
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
            let mut cell = Cell::new(value);
            // §19.643 table:formula in OpenFormula (ODF 1.3 Part 4) → A1.
            cell.formula = c
                .formula
                .as_deref()
                .and_then(bo_odf::from_openformula)
                .map(|text| Formula {
                    text,
                    ..Formula::default()
                });
            if let Some(&xf) = c.style.as_deref().and_then(|n| xfs.get(n)) {
                cell.style = xf;
            }
            ws.cells.insert((c.row, c.col), cell);
        }
        wb.sheets.push(ws);
    }
    if wb.sheets.is_empty() {
        wb.sheets.push(Worksheet::new("Sheet1"));
    }
    wb
}

/// Maps a workbook to the ODF spreadsheet model: values, formulas in
/// OpenFormula (ODF 1.3 Part 4, `of:` namespace) and one automatic cell style
/// per cell format in use (font, colours, fill, alignment, wrapping).
pub fn workbook_to_ods(wb: &Workbook, locale: &Locale) -> bo_odf::SpreadsheetDoc {
    use bo_odf::CellValue as O;
    let mut doc = bo_odf::SpreadsheetDoc::default();
    let mut names: std::collections::HashMap<u32, Option<String>> =
        std::collections::HashMap::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        let mut sheet = bo_odf::Sheet {
            name: ws.name.clone(),
            cells: Vec::new(),
        };
        for (&(row, col), c) in &ws.cells {
            let value = match &c.value {
                CellValue::Number(n) => O::Float(*n),
                CellValue::Bool(b) => O::Boolean(*b),
                CellValue::Empty if c.formula.is_none() && c.style == 0 => continue,
                CellValue::Empty => O::String(String::new()),
                CellValue::Text(t) => O::String(t.clone()),
                CellValue::RichText(_) => O::String(c.value.as_text().unwrap_or_default()),
                // Dates and errors as displayed.
                _ => O::String(format_cell(wb, c, locale).text),
            };
            let style = names
                .entry(c.style)
                .or_insert_with(|| {
                    if c.style == 0 {
                        return None;
                    }
                    let st = bo_sheet::style::format_state(wb, si, row, col, locale);
                    let def = bo_sheet::style::format_state(&Workbook::new(), 0, 0, 0, locale);
                    let cs = bo_odf::CellStyle {
                        name: format!("ce{}", c.style),
                        bold: st.bold,
                        italic: st.italic,
                        underline: !st.underline.is_empty() && st.underline != "none",
                        font: (st.font_name != def.font_name).then(|| st.font_name.clone()),
                        size: ((st.font_size - def.font_size).abs() > f64::EPSILON)
                            .then_some(st.font_size),
                        color: (!st.font_color.is_empty()).then(|| st.font_color.clone()),
                        background: (!st.fill_color.is_empty()).then(|| st.fill_color.clone()),
                        align: match st.h_align.as_str() {
                            "center" | "centerContinuous" => Some("center".into()),
                            "right" => Some("end".into()),
                            "justify" | "distributed" => Some("justify".into()),
                            "left" => Some("start".into()),
                            _ => None,
                        },
                        wrap: st.wrap,
                    };
                    let name = cs.name.clone();
                    doc.styles.push(cs);
                    Some(name)
                })
                .clone();
            sheet.cells.push(bo_odf::Cell {
                row,
                col,
                value,
                formula: c
                    .formula
                    .as_ref()
                    .filter(|f| !f.text.is_empty())
                    .map(|f| bo_odf::to_openformula(&f.text)),
                style,
            });
        }
        doc.sheets.push(sheet);
    }
    doc
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
            cell.vm = None;
            cell.image = None;
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
    // Typing replaces a picture / rich value ([MS-XLSX] §2.3.6) with the new value.
    cell.vm = None;
    cell.image = None;
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

/// Formula-bar text of a number in a cell with a date / time format — built-in
/// (§18.8.30 ids 14–22) or custom (§18.8.31 with date/time tokens): the date in
/// the locale's short date form, plus the time when the value has one (always
/// for id 22), or the time alone below 1; so that editing and confirming keeps
/// the value. `None` for other formats.
fn date_input(
    n: f64,
    styles: &bo_model::sheet::Stylesheet,
    fmt_id: u32,
    locale: &Locale,
    date1904: bool,
) -> Option<String> {
    if n < 0.0 || !n.is_finite() {
        return None;
    }
    let fmt_code = bo_sheet::numfmt::format_code(styles, fmt_id, locale);
    if !bo_sheet::numfmt::NumberFormat::parse_with(&fmt_code, locale).is_date() {
        return None;
    }
    let code = if n < 1.0 && fmt_id != 14 {
        "h:mm:ss".to_owned()
    } else if fmt_id == 22 || n.fract() != 0.0 {
        format!("{} h:mm:ss", locale.short_date)
    } else {
        locale.short_date.to_owned()
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
    /// An engine with no workbook and no fonts.
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
            save_password: None,
            collab: None,
            macros: false,
            file_variant: None,
            save_variant: None,
            legacy_converted: 0,
            drawings: Arc::default(),
            drawings_key: 0,
            scrub: crate::inspect::package::Scrub::default(),
            props: crate::props::PropsState::default(),
            sec: crate::security::FileSecurity::default(),
            open_ranges: Vec::new(),
            identity: Vec::new(),
            table_auto: table_cmd::TableAuto::default(),
            spell: crate::spell::SpellState::default(),
            web_images: cell_media_cmd::WebImages::default(),
            print_cache: crate::print::PreviewCache::default(),
            pdf_export: bo_pdf::ExportChoices::default(),
            query_sources: query_cmd::QuerySources::default(),
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
        // English spelling (SCOWL en_US, assets/dictionaries); Vietnamese needs no data.
        let _ = e.load_dictionary(crate::bundled::EN_US_AFF, crate::bundled::EN_US_DIC);
        e
    }

    /// Registers a font file (TTF/OTF/TTC); returns the number of faces added.
    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        self.fonts.add_font(bytes).len()
    }

    /// Number of registered font faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.fonts.len()
    }

    /// Installed OS fonts for families that are not bundled (desktop, see `Engine::set_system_fonts`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_system_fonts(&mut self, index: bo_text::SystemFontIndex) {
        self.fonts.set_system_fonts(index);
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

    /// Enables `WEBSERVICE` with a host fetcher, or disables it again with `None`.
    ///
    /// Web access is **off by default** and only an administrator enables it (deployment
    /// policy); while it is off `WEBSERVICE` returns `#VALUE!`. The web and desktop hosts do
    /// not install a fetcher yet; `bo-formula` itself never opens network connections.
    /// Takes effect at the next recalculation (the function is volatile).
    pub fn set_web_provider(&mut self, provider: Option<Arc<dyn bo_formula::WebProvider>>) {
        self.live.set_web_provider(provider);
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
        self.open_with_password(bytes, name, None)
    }

    /// [`SheetEngine::open`] for files that may be encrypted ([MS-OFFCRYPTO]
    /// §2.3.4.10 Agile / §2.3.4.5 Standard Encryption, via `bo-crypto`): errors
    /// `password required` / `wrong password` ask the host for a password. An
    /// encrypted workbook is saved again with the same password.
    pub fn open_with_password(
        &mut self,
        bytes: &[u8],
        name: &str,
        password: Option<&str>,
    ) -> Result<WorkbookInfo> {
        let plain = crate::decrypt_package(bytes, password)?;
        let encrypted = plain.is_some();
        let bytes = plain.as_deref().unwrap_or(bytes);
        // Flat OPC (CORE-FMT-003) opens as the package it describes.
        let flat = crate::web_formats::flat_package(bytes)?;
        let bytes = flat.as_deref().unwrap_or(bytes);
        let info = self.open_plain(bytes, name)?;
        self.macros = bo_binary::has_macros(bytes);
        self.file_variant = crate::variant::detect(bytes).or(self.file_variant);
        self.open_security(bytes);
        self.props = crate::props::PropsState::read(bytes);
        self.save_password = if encrypted {
            password.map(str::to_owned)
        } else {
            None
        };
        self.info().or(Ok(info))
    }

    /// Package variant of the next `.xlsx` saves by file extension (`xlsx`, `xlsm`,
    /// `xltx`, `xltm`; CORE-FMT-001): a macro-free variant drops the VBA project; the
    /// workbook name takes the new extension.
    pub fn set_save_format(&mut self, ext: &str) -> Result<()> {
        self.wb()?;
        let v = crate::variant::Variant::from_ext(ext)
            .filter(|v| v.app() == "sheet")
            .ok_or_else(|| EngineError::Unsupported(ext.to_owned()))?;
        self.save_variant = (Some(v) != self.file_variant).then_some(v);
        if !self.name.is_empty() {
            self.name = crate::with_ext(&self.name, v.ext());
        }
        Ok(())
    }

    /// Password for the next `.xlsx` saves (`None` or empty: not encrypted).
    pub fn set_save_password(&mut self, password: Option<&str>) {
        self.save_password = password.filter(|p| !p.is_empty()).map(str::to_owned);
    }

    fn open_plain(&mut self, bytes: &[u8], name: &str) -> Result<WorkbookInfo> {
        let wb = read_workbook_bytes(bytes, name, &self.locale)?;
        self.load_opened(wb, name)?;
        self.convert_legacy_text()?;
        self.info()
    }

    /// Converts cells shown in legacy Vietnamese fonts (TCVN3 `.Vn*`, `VNI-*`)
    /// to Unicode and switches those fonts to a Unicode font
    /// ([`bo_encoding::convert_workbook`]); one undo step. Returns the number
    /// of cells converted (also reported as `legacyConverted`).
    pub fn convert_legacy_text(&mut self) -> Result<usize> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if !bo_encoding::workbook_has_legacy_fonts(wb) {
            // Nothing to convert: no undo snapshot (a full copy of the workbook).
            self.legacy_converted = 0;
            return Ok(0);
        }
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

    /// [`SheetEngine::load`] for a workbook read from a file: saved formula
    /// results are kept unless the file asks for a full calculation on load
    /// (ECMA-376 Part 1, §18.2.2 `calcPr/@fullCalcOnLoad`).
    fn load_opened(&mut self, mut wb: Workbook, name: &str) -> Result<WorkbookInfo> {
        if full_calc_on_load(&wb) {
            return self.load(wb, name);
        }
        self.live.rebuild_from_file(&mut wb);
        self.install(wb, name)
    }

    /// New empty workbook with one sheet.
    pub fn new_workbook(&mut self, name: &str) -> Result<WorkbookInfo> {
        let mut wb = Workbook::new();
        if wb.sheets.is_empty() {
            wb.sheets.push(Worksheet::new("Sheet1"));
        }
        self.load(wb, name)
    }

    /// Opens a workbook model as the current document (clears undo, clipboard and collaboration).
    pub fn load(&mut self, mut wb: Workbook, name: &str) -> Result<WorkbookInfo> {
        self.live.rebuild(&mut wb);
        self.install(wb, name)
    }

    /// Makes `wb` (already calculated) the open workbook.
    fn install(&mut self, mut wb: Workbook, name: &str) -> Result<WorkbookInfo> {
        self.save_password = None;
        self.scrub = crate::inspect::package::Scrub::default();
        self.props = crate::props::PropsState::default();
        self.macros = false;
        self.file_variant = Some(crate::variant::Variant::Xlsx);
        self.save_variant = None;
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
        self.drawings_key = diagrams_key(&wb);
        self.clip = None;
        self.collab = None;
        self.geometry = (0..wb.sheets.len()).map(|_| None).collect();
        self.wb = Some(wb);
        self.name = name.to_owned();
        self.undo.clear();
        self.redo.clear();
        self.legacy_converted = 0;
        self.sec = crate::security::FileSecurity::default();
        self.open_ranges.clear();
        self.info()
    }

    /// The open workbook, if any.
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

    /// Summary of the open workbook.
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
                        protected: protect::sheet_info(ws).protected,
                        tab_color: ws
                            .props
                            .tab_color
                            .as_ref()
                            .and_then(|c| bo_sheet::color::resolve_color(c, wb.theme.as_ref()))
                            .map(|c| format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)),
                    }
                })
                .collect(),
            encrypted: self.save_password.is_some(),
            macros: self.macros,
            structure_locked: protect::workbook_info(wb).structure,
            legacy_converted: self.legacy_converted,
            variant: self
                .save_variant
                .or(self.file_variant)
                .map(|v| v.ext().to_owned()),
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

    /// Draw the SmartArt diagrams again when one was inserted, edited, moved or deleted (or such
    /// an edit was undone).
    fn refresh_drawings(&mut self) {
        let Some(wb) = self.wb.as_ref() else { return };
        let key = diagrams_key(wb);
        if key != self.drawings_key {
            self.drawings = Arc::new(diagram_overlays(wb, &mut self.fonts));
            self.drawings_key = key;
        }
    }

    /// Display list of the visible part of a sheet.
    pub fn render(&mut self, req: &ViewRequest) -> Result<PageList> {
        self.refresh_drawings();
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
            images: self.web_images.states(),
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

    /// [`Self::render`] as JSON.
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
        let (charts, filter, diagrams) = {
            let ws = self.sheet(req.sheet)?;
            (
                ws.charts.iter().map(|c| (c.from, c.to)).collect::<Vec<_>>(),
                ws.auto_filter.as_ref().map(|a| a.range),
                ws.diagrams
                    .iter()
                    .map(|d| (d.from, d.to))
                    .collect::<Vec<_>>(),
            )
        };
        let (layout, geom) = self.layout(req)?;
        let hit = layout.hit_test(geom, x, y);
        // Charts and SmartArt are drawn above the cells.
        if let Hit::Cell(under) = hit {
            for (i, (from, to)) in diagrams.iter().enumerate().rev() {
                let to = to.unwrap_or(bo_model::sheet::CellRef::new(
                    from.row.saturating_add(10),
                    from.col.saturating_add(6),
                ));
                let (Some(a), Some(b)) = (
                    layout.cell_screen_rect(geom, from.row, from.col),
                    layout.cell_screen_rect(geom, to.row, to.col),
                ) else {
                    continue;
                };
                if x >= a.x && x < b.x && y >= a.y && y < b.y {
                    return Ok(HitInfo::SmartArt {
                        index: i,
                        row: under.row,
                        col: under.col,
                    });
                }
            }
        }
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
            (_, CellValue::Number(n)) => {
                date_input(*n, &wb.styles, fmt_id, &self.locale, wb.date1904)
                    .unwrap_or_else(|| number_input(*n, &self.locale))
            }
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
        self.writable()?;
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
        {
            // §18.3.1.85: locked cells of a protected sheet cannot be edited.
            let wb = self.wb()?;
            let ws = self.sheet(sheet)?;
            let open = self.open_ranges(sheet);
            protect::check_cells(wb, ws, &open, cells.iter().map(|(r, c, _)| (*r, *c)))?;
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
        // Tables: header names, automatic expansion, calculated columns (SHEET-TABLE-001).
        self.tables_after_input(sheet, &coords);
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
        self.writable()?;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if sheet >= wb.sheets.len() || count == 0 {
            return Ok(());
        }
        let action = match (rows, delete) {
            (true, false) => protect::Action::InsertRows,
            (true, true) => protect::Action::DeleteRows,
            (false, false) => protect::Action::InsertColumns,
            (false, true) => protect::Action::DeleteColumns,
        };
        protect::check(&wb.sheets[sheet], action)?;
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
        let e = match e {
            // Undone newest first; the inverses replay oldest first.
            UndoEntry::Group(list) => {
                let mut inv = Vec::with_capacity(list.len());
                for e in list.into_iter().rev() {
                    inv.push(self.apply(e)?);
                }
                return Ok(UndoEntry::Group(inv));
            }
            UndoEntry::Light(step) => {
                let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
                let before = wb.sheets.len();
                let inv = step.apply(wb, &self.adjuster);
                self.invalidate(None);
                self.sync_light(&inv, before);
                return Ok(UndoEntry::Light(Box::new(inv)));
            }
            e => e,
        };
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
            UndoEntry::Group(_) | UndoEntry::Light(_) => {
                return Err(EngineError::Sheet("nested undo group".into()))
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

    /// Undoes the last edit; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool> {
        self.writable()?;
        let Some(e) = self.undo.pop() else {
            return Ok(false);
        };
        let inv = self.apply(e)?;
        self.redo.push(inv);
        Ok(true)
    }

    /// Redoes the last undone edit; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool> {
        self.writable()?;
        let Some(e) = self.redo.pop() else {
            return Ok(false);
        };
        let inv = self.apply(e)?;
        self.undo.push(inv);
        Ok(true)
    }

    /// Number of undo steps.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.undo.len()
    }

    /// Joins the undo steps after the first `depth` into one; returns the length afterwards.
    pub fn merge_history_since(&mut self, depth: usize) -> usize {
        if self.undo.len() > depth + 1 {
            let tail = self.undo.split_off(depth);
            self.undo.push(UndoEntry::Group(tail));
        }
        self.undo.len()
    }

    /// Undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Adds a sheet at the end; returns its index.
    pub fn add_sheet(&mut self, name: Option<&str>) -> Result<usize> {
        self.writable()?;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        protect::check_structure(wb)?;
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
        self.writable()?;
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 31
            || name.contains(['[', ']', ':', '*', '?', '/', '\\'])
        {
            return Err(EngineError::Sheet(format!("invalid sheet name '{name}'")));
        }
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        protect::check_structure(wb)?;
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
        self.writable()?;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        protect::check_structure(wb)?;
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

    /// Serialises the workbook: `xlsx`, `csv` of `sheet`, `ods`, `html` (every sheet
    /// as an HTML table), `html-sheet` (only `sheet`) or `xml` (Flat OPC of the
    /// `.xlsx` package, not encrypted; CORE-FMT-003). Chart caches are refreshed
    /// from their ranges first (§21.2.2.120 `numCache`).
    pub fn save(&mut self, format: &str, sheet: usize) -> Result<Vec<u8>> {
        let locale = self.locale;
        if matches!(format, "xlsx" | "xml") {
            if let Some(wb) = self.wb.as_mut() {
                if wb
                    .sheets
                    .iter()
                    .any(|s| s.charts.iter().any(|c| c.dirty || c.part.is_empty()))
                {
                    bo_sheet::chart::refresh_caches(wb, &locale);
                }
                // Form controls agree with their linked cells in the saved file.
                bo_sheet::formctl::sync_from_links(wb);
                // Connections and query tables of the Get & Transform queries (§18.12–13).
                query_cmd::sync_ecma_parts(wb);
                // Manual calculation with calcPr/@calcOnSave (§18.2.2): calculate first.
                let calc = bo_sheet::calcopts::CalcSettings::read(wb);
                if calc.calc_on_save && self.live.needs_calculation() {
                    self.live.calculate(wb, None);
                    self.invalidate(None);
                }
            }
        }
        let wb = self.wb()?;
        match format {
            "xlsx" | "xml" => {
                let bytes =
                    bo_xlsx::write_xlsx(wb).map_err(|e| EngineError::Sheet(e.to_string()))?;
                let bytes = match self.save_variant {
                    Some(v) if Some(v) != self.file_variant => {
                        crate::variant::convert(&bytes, v)?.bytes
                    }
                    _ => bytes,
                };
                let bytes = self.props.apply(bytes)?;
                let bytes = crate::inspect::package::scrub(&bytes, &self.scrub)?;
                let bytes = self.sec.apply(bytes)?;
                if format == "xml" {
                    return bo_opc::flat::zip_to_flat(&bytes)
                        .map_err(|e| EngineError::Format(e.to_string()));
                }
                match &self.save_password {
                    // [MS-OFFCRYPTO] §2.3.4.10 Agile Encryption (AES-256 / SHA-512).
                    Some(pw) => bo_crypto::encrypt(&bytes, pw)
                        .map_err(|e| EngineError::Crypto(e.to_string())),
                    None => Ok(bytes),
                }
            }
            "csv" => {
                let opts = if self.locale.decimal == ',' {
                    bo_xlsx::CsvOptions::vi_vn()
                } else {
                    bo_xlsx::CsvOptions::default()
                };
                Ok(bo_xlsx::write_csv(self.sheet(sheet)?, &opts))
            }
            // HTML tables: displayed text, cell formatting, merged cells.
            "html" | "html-sheet" => {
                let only = (format == "html-sheet").then_some(sheet);
                if only.is_some() {
                    self.sheet(sheet)?;
                }
                let title = self
                    .name
                    .rsplit_once('.')
                    .map_or(self.name.as_str(), |(b, _)| b);
                Ok(bo_html::write_workbook_html(wb, only, &self.locale, title).into_bytes())
            }
            // ODF 1.3 spreadsheet: values, OpenFormula formulas, cell styles.
            "ods" => bo_odf::write_ods(&workbook_to_ods(wb, &self.locale))
                .map_err(|e| EngineError::Sheet(e.to_string())),
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
                        style: None,
                    },
                    bo_odf::Cell {
                        row: 1,
                        col: 1,
                        value: bo_odf::CellValue::Float(2.5),
                        formula: None,
                        style: None,
                    },
                ],
            }],
            styles: Vec::new(),
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

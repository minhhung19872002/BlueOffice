//! The [`crate::calc`] boundary implemented with the `bo-formula` engine.
//!
//! [`BoFormula`] is both a [`FormulaAdjuster`] (reference rewriting for
//! insert/delete rows and columns, AutoFill and sort) and a [`FormulaEngine`]
//! (recalculation of a [`bo_model::sheet::Workbook`]):
//!
//! * cells, formulas (normal, shared — followers are expanded from the master,
//!   ECMA-376 Part 1, §18.3.1.40 `f` with `t="shared"` — and array formulas),
//!   defined names (§18.2.5), tables (§18.5.1.2), hidden rows (§18.3.1.73) and
//!   the 1904 date system (§18.2.28 `date1904`) are loaded into a
//!   [`bo_formula::Workbook`];
//! * after calculation every formula cell gets its new cached value; dynamic
//!   array results are written into the spilled cells and the anchor's
//!   array range is updated.

use bo_formula::{
    adjust_for_change, format, offset_relative, parse, DateSystem, ErrorKind, TableInfo, Value,
};
use bo_model::sheet::{Cell, CellError, CellRange, CellValue, FormulaKind, Workbook};

use crate::calc::{Dimension, FormulaAdjuster, FormulaEngine, StructuralChange};

/// Sheet name that matches no real sheet: unqualified references in
/// workbook-scope names have no home sheet and are left alone.
const NO_SHEET: &str = "\u{0}";

/// Formula adjuster and calculation engine backed by `bo-formula`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoFormula {
    /// Fixed serial for `NOW()`/`TODAY()` (tests, reproducible output);
    /// `None` uses the system clock.
    pub now: Option<f64>,
}

impl BoFormula {
    /// An engine using the system clock.
    pub fn new() -> Self {
        BoFormula::default()
    }
}

fn to_change(c: &StructuralChange) -> bo_formula::StructuralChange {
    use bo_formula::StructuralChange as S;
    let (at, count) = (c.at, c.count);
    match (c.dimension, c.insert) {
        (Dimension::Rows, true) => S::InsertRows { at, count },
        (Dimension::Rows, false) => S::DeleteRows { at, count },
        (Dimension::Cols, true) => S::InsertCols { at, count },
        (Dimension::Cols, false) => S::DeleteCols { at, count },
    }
}

impl FormulaAdjuster for BoFormula {
    fn adjust(
        &self,
        formula: &str,
        host_sheet: Option<&str>,
        change: &StructuralChange,
    ) -> Option<String> {
        let expr = parse(formula).ok()?;
        let adjusted = adjust_for_change(
            &expr,
            to_change(change),
            host_sheet.unwrap_or(NO_SHEET),
            &change.sheet,
        );
        (adjusted != expr).then(|| format(&adjusted))
    }

    fn adjust_band(
        &self,
        formula: &str,
        host_sheet: Option<&str>,
        change: &StructuralChange,
        band: (u32, u32),
    ) -> Option<String> {
        let expr = parse(formula).ok()?;
        let adjusted = bo_formula::adjust_for_band_change(
            &expr,
            to_change(change),
            host_sheet.unwrap_or(NO_SHEET),
            &change.sheet,
            band,
        );
        (adjusted != expr).then(|| format(&adjusted))
    }

    fn translate(&self, formula: &str, d_rows: i64, d_cols: i64) -> Option<String> {
        let expr = parse(formula).ok()?;
        let moved = offset_relative(&expr, d_rows, d_cols);
        (moved != expr).then(|| format(&moved))
    }

    fn moved(&self, formula: &str, host_sheet: &str, mv: &crate::calc::CellMove) -> Option<String> {
        let expr = parse(formula).ok()?;
        let block = bo_formula::MovedBlock {
            from_sheet: mv.from_sheet.clone(),
            to_sheet: mv.to_sheet.clone(),
            first_row: mv.first_row,
            first_col: mv.first_col,
            last_row: mv.last_row,
            last_col: mv.last_col,
            d_row: mv.d_rows,
            d_col: mv.d_cols,
        };
        let out = bo_formula::adjust_for_move(&expr, host_sheet, &block);
        (out != expr).then(|| format(&out))
    }
}

pub(crate) fn to_value(v: &CellValue, system: DateSystem) -> Value {
    match v {
        CellValue::Empty => Value::Empty,
        CellValue::Number(n) => Value::Number(*n),
        CellValue::Bool(b) => Value::Bool(*b),
        CellValue::Error(e) => Value::Error(ErrorKind::parse(e.as_str()).unwrap_or(ErrorKind::NA)),
        CellValue::Date(s) => bo_formula::date::parse_datetime_text(s, system)
            .map_or_else(|| Value::Text(s.clone()), Value::Number),
        CellValue::Text(_) | CellValue::RichText(_) => Value::Text(v.as_text().unwrap_or_default()),
    }
}

pub(crate) fn to_cell_value(v: &Value) -> CellValue {
    match v.first() {
        Value::Number(n) => CellValue::Number(*n),
        Value::Text(s) => CellValue::Text(s.clone()),
        Value::Bool(b) => CellValue::Bool(*b),
        Value::Error(e) => CellValue::Error(CellError::parse(e.as_str())),
        Value::Empty => CellValue::Number(0.0),
        _ => CellValue::Error(CellError::Value),
    }
}

/// Formula text of a cell, expanding shared-formula followers from their master.
fn formula_text<'f>(
    masters: &std::collections::HashMap<u32, (u32, u32, bo_formula::Expr)>,
    row: u32,
    col: u32,
    f: &'f bo_model::sheet::Formula,
) -> Option<std::borrow::Cow<'f, str>> {
    if !f.text.is_empty() {
        return Some(std::borrow::Cow::Borrowed(&f.text));
    }
    let FormulaKind::Shared { index, .. } = f.kind else {
        return None;
    };
    let (mr, mc, expr) = masters.get(&index)?;
    let moved = offset_relative(
        expr,
        i64::from(row) - i64::from(*mr),
        i64::from(col) - i64::from(*mc),
    );
    Some(std::borrow::Cow::Owned(format(&moved)))
}

/// Whether formula `text` calls a function whose data lives outside the workbook:
/// the cube functions and `RTD` (ECMA-376 Part 1, §18.17.7). BlueSheet has no OLAP
/// connection or real-time data server, so such a cell keeps the result saved in
/// the file (`<v>`, §18.3.1.96) instead of turning into an error on recalculation.
pub(crate) fn external_data_formula(text: &str) -> bool {
    const NAMES: [&str; 8] = [
        "CUBEKPIMEMBER",
        "CUBEMEMBER",
        "CUBEMEMBERPROPERTY",
        "CUBERANKEDMEMBER",
        "CUBESET",
        "CUBESETCOUNT",
        "CUBEVALUE",
        "RTD",
    ];
    // Cheap rejection first: this runs for every formula of the workbook.
    let upper = |needle: &[u8]| {
        text.as_bytes()
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
    };
    if !upper(b"CUBE") && !upper(b"RTD") {
        return false;
    }
    let Ok(expr) = parse(text) else {
        return false;
    };
    let mut found = false;
    expr.walk(&mut |e| {
        if let bo_formula::Expr::Call { name, .. } = e {
            found |= NAMES.contains(&name.as_str());
        }
    });
    found
}

/// Whether `cell` is an external-data formula with a saved result to keep.
pub(crate) fn keeps_saved_result(cell: &Cell, text: &str) -> bool {
    !matches!(cell.value, CellValue::Empty) && external_data_formula(text)
}

pub(crate) fn array_range(f: &bo_model::sheet::Formula) -> Option<CellRange> {
    match &f.kind {
        FormulaKind::Array { range } => CellRange::parse(range),
        _ => None,
    }
}

/// The range of a legacy array formula (entered over a fixed range, not spilling;
/// ECMA-376 Part 1, §18.17.2.4), `None` for every other formula.
pub(crate) fn legacy_array_range(f: &bo_model::sheet::Formula) -> Option<CellRange> {
    if f.dynamic {
        return None;
    }
    array_range(f)
}

/// Sets formula `f` (text `text`) of a cell into the engine: a legacy array formula over
/// its range, any other as an ordinary (possibly spilling) formula.
pub(crate) fn set_engine_formula(
    fw: &mut bo_formula::Workbook,
    sheet: usize,
    row: u32,
    col: u32,
    f: &bo_model::sheet::Formula,
    text: &str,
) -> Result<(), bo_formula::ParseError> {
    match legacy_array_range(f) {
        Some(r) => fw.set_array_formula(sheet, row, col, text, r.rows(), r.cols()),
        None => fw.set_formula(sheet, row, col, text),
    }
}

pub(crate) fn load(wb: &Workbook, now: Option<f64>) -> bo_formula::Workbook {
    load_with(wb, now, false)
}

/// Whether the cached value of a formula cell can stand for its result when a
/// file is opened. Array formulas and data tables (whose results live in other
/// cells), `ca` "calculate always" cells (§18.3.1.40) and cells without a saved
/// value are calculated.
fn cached_result(cell: &Cell, system: DateSystem) -> Option<Value> {
    let f = cell.formula.as_ref()?;
    if f.calc_always
        || matches!(
            f.kind,
            FormulaKind::Array { .. } | FormulaKind::DataTable { .. }
        )
        || matches!(cell.value, CellValue::Empty)
    {
        return None;
    }
    Some(to_value(&cell.value, system))
}

/// Loads `wb` into a formula engine. With `trust_cache` formula cells keep
/// their saved values (see [`cached_result`]) and only the others are marked
/// for calculation; otherwise every formula is marked.
pub(crate) fn load_with(
    wb: &Workbook,
    now: Option<f64>,
    trust_cache: bool,
) -> bo_formula::Workbook {
    let system = if wb.date1904 {
        DateSystem::Date1904
    } else {
        DateSystem::Date1900
    };
    let names: Vec<&str> = wb.sheets.iter().map(|s| s.name.as_str()).collect();
    let mut fw = bo_formula::Workbook::with_sheets(&names);
    // Sheets of linked workbooks (§18.14), after the workbook's own: `[n]Sheet` holds the
    // values last read from link `n` (`sheetDataSet`), so `[n]Sheet!A1` evaluates to them.
    for (li, link) in wb.external_links.iter().enumerate() {
        for (si, name) in link.sheets.iter().enumerate() {
            let s = fw.add_sheet(&format!("[{}]{name}", li + 1));
            for (r, c, v) in link.cells.get(si).map_or(&[][..], Vec::as_slice) {
                fw.load_value(s, *r, *c, to_value(v, system));
            }
        }
    }
    let calc = crate::calcopts::CalcSettings::read(wb);
    // Before any formula is loaded (it marks every formula for calculation).
    fw.set_iterative(calc.iterative());
    fw.set_date_system(system);
    fw.set_now(now);
    // TEXT() renders with the full number-format engine (§18.8.31).
    fw.set_text_formatter(Some(crate::numfmt::text_function_format));
    // Names and tables first: defining one rebuilds the dependency graph of
    // the formulas already loaded, which is empty at this point.
    for dn in &wb.defined_names {
        let scope = dn.local_sheet.map(|s| s as usize);
        // Names that do not parse (external links, macros) are skipped.
        let _ = fw.define_name(&dn.name, scope, &dn.formula);
    }
    for (si, ws) in wb.sheets.iter().enumerate() {
        for t in &ws.tables {
            let first_row = t.range.first.row;
            fw.add_table(TableInfo {
                name: t.name.clone(),
                sheet: si,
                first_row,
                last_row: t.range.last.row,
                first_col: t.range.first.col,
                columns: t.columns.iter().map(|c| c.name.clone()).collect(),
                header_row: t.header_row_count.unwrap_or(1) > 0,
                totals_row: t.totals_row_count.unwrap_or(0) > 0,
            });
        }
    }
    if crate::pivot::formula_info::workbook_reads_pivots(wb) {
        fw.set_pivots(crate::pivot::formula_info::formula_pivots(wb));
    }
    for (si, ws) in wb.sheets.iter().enumerate() {
        fw.reserve_cells(si, ws.cells.len());
        // Cells covered by an array formula hold its previous results, not inputs.
        let mut covered: Vec<CellRange> = Vec::new();
        let mut masters = std::collections::HashMap::new();
        for (&(r, c), cell) in &ws.cells {
            if let Some(f) = &cell.formula {
                if let Some(range) = array_range(f) {
                    covered.push(range);
                }
                if let (
                    FormulaKind::Shared {
                        index,
                        range: Some(_),
                    },
                    false,
                ) = (&f.kind, f.text.is_empty())
                {
                    if let Ok(e) = parse(&f.text) {
                        masters.entry(*index).or_insert((r, c, e));
                    }
                }
            }
        }
        for (&(r, c), cell) in &ws.cells {
            let here = bo_model::sheet::CellRef { row: r, col: c };
            match &cell.formula {
                Some(f) => {
                    let text = formula_text(&masters, r, c, f);
                    let cached = if trust_cache {
                        cached_result(cell, system)
                    } else {
                        None
                    };
                    let ok = text.is_some_and(|t| {
                        !keeps_saved_result(cell, &t)
                            && if legacy_array_range(f).is_some() {
                                set_engine_formula(&mut fw, si, r, c, f, &t).is_ok()
                            } else {
                                fw.load_formula(si, r, c, &t, cached).is_ok()
                            }
                    });
                    if !ok {
                        // Unparsable or external-data formula: keep its cached value
                        // as a constant.
                        fw.set_value(si, r, c, to_value(&cell.value, system));
                    }
                }
                None => {
                    if covered.is_empty()
                        || !covered.iter().any(|a| a.contains(here) && a.first != here)
                    {
                        fw.load_value(si, r, c, to_value(&cell.value, system));
                    }
                }
            }
        }
        // Bulk: `set_row_hidden` would mark every formula once per hidden row (a
        // filtered list of a million rows). Without a trusted cache every formula
        // is already marked by `load_formula`; with one, cached `SUBTOTAL`
        // results already account for the hidden rows.
        for (&r, info) in &ws.rows {
            if info.hidden {
                fw.load_row_hidden(si, r);
            }
        }
        fw.load_filter_spans(si, crate::filter::filter_spans(ws));
    }
    if !calc.full_precision {
        crate::calcopts::apply_display_precision(&mut fw, wb);
    }
    fw
}

impl FormulaEngine for BoFormula {
    fn recalculate(&mut self, wb: &mut Workbook) {
        let mut fw = load(wb, self.now);
        fw.recalculate();
        for si in 0..wb.sheets.len() {
            let anchors: Vec<(u32, u32)> = wb.sheets[si]
                .cells
                .iter()
                .filter(|(_, c)| c.formula.is_some())
                .map(|(&k, _)| k)
                .collect();
            write_back(&fw, wb, si, &anchors);
        }
        crate::whatif::calculate_data_tables(&mut fw, wb);
    }
}

/// Copies calculated values of formula cells `anchors` of sheet `si` from the
/// engine into the model (spilled results included).
pub(crate) fn write_back(
    fw: &bo_formula::Workbook,
    wb: &mut Workbook,
    si: usize,
    anchors: &[(u32, u32)],
) {
    let Some(ws) = wb.sheets.get_mut(si) else {
        return;
    };
    for &(r, c) in anchors {
        if ws
            .cells
            .get(&(r, c))
            .is_none_or(|cell| cell.formula.is_none())
        {
            continue;
        }
        let value = fw.value(si, r, c);
        let spill = fw.spill_range(si, r, c);
        let old = ws
            .cells
            .get(&(r, c))
            .and_then(|cell| cell.formula.as_ref())
            .and_then(array_range);
        // Clear results of the previous array that are no longer covered.
        if let Some(old) = old {
            for rr in old.first.row..=old.last.row {
                for cc in old.first.col..=old.last.col {
                    let inside = spill.is_some_and(|a| a.contains(si, rr, cc));
                    if (rr, cc) != (r, c) && !inside {
                        if let Some(cell) = ws.cells.get_mut(&(rr, cc)) {
                            if cell.formula.is_none() {
                                cell.value = CellValue::Empty;
                            }
                        }
                    }
                }
            }
        }
        if let Some(cell) = ws.cells.get_mut(&(r, c)) {
            cell.value = to_cell_value(&value);
            // `IMAGE` results ([MS-XLSX] §2.3.6.1.11): the cell shows the picture.
            cell.image = crate::cell_image::formula_image(&value);
            if let (Some(a), Some(f)) = (spill, cell.formula.as_mut()) {
                // A legacy array formula keeps the range it was entered in.
                if !f.text.is_empty() && legacy_array_range(f).is_none() {
                    f.dynamic = true;
                    let range = CellRange::new(
                        bo_model::sheet::CellRef {
                            row: a.r1,
                            col: a.c1,
                        },
                        bo_model::sheet::CellRef {
                            row: a.r2,
                            col: a.c2,
                        },
                    );
                    f.kind = FormulaKind::Array {
                        range: range.to_string(),
                    };
                }
            }
        }
        if let Some(a) = spill {
            for rr in a.r1..=a.r2 {
                for cc in a.c1..=a.c2 {
                    if (rr, cc) == (r, c) {
                        continue;
                    }
                    let v = to_cell_value(&fw.value(si, rr, cc));
                    let cell = ws.cells.entry((rr, cc)).or_insert_with(Cell::default);
                    if cell.formula.is_none() {
                        cell.value = v;
                    }
                }
            }
        }
    }
}

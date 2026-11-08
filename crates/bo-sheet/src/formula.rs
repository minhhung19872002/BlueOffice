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
fn formula_text(
    masters: &[(u32, u32, u32, bo_formula::Expr)],
    row: u32,
    col: u32,
    f: &bo_model::sheet::Formula,
) -> Option<String> {
    if !f.text.is_empty() {
        return Some(f.text.clone());
    }
    let FormulaKind::Shared { index, .. } = f.kind else {
        return None;
    };
    let (_, mr, mc, expr) = masters.iter().find(|(i, ..)| *i == index)?;
    let moved = offset_relative(
        expr,
        i64::from(row) - i64::from(*mr),
        i64::from(col) - i64::from(*mc),
    );
    Some(format(&moved))
}

pub(crate) fn array_range(f: &bo_model::sheet::Formula) -> Option<CellRange> {
    match &f.kind {
        FormulaKind::Array { range } => CellRange::parse(range),
        _ => None,
    }
}

pub(crate) fn load(wb: &Workbook, now: Option<f64>) -> bo_formula::Workbook {
    let system = if wb.date1904 {
        DateSystem::Date1904
    } else {
        DateSystem::Date1900
    };
    let names: Vec<&str> = wb.sheets.iter().map(|s| s.name.as_str()).collect();
    let mut fw = bo_formula::Workbook::with_sheets(&names);
    fw.set_date_system(system);
    fw.set_now(now);
    for (si, ws) in wb.sheets.iter().enumerate() {
        // Cells covered by an array formula hold its previous results, not inputs.
        let mut covered: Vec<CellRange> = Vec::new();
        let mut masters = Vec::new();
        fw.reserve_cells(si, ws.cells.len());
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
                        masters.push((*index, r, c, e));
                    }
                }
            }
        }
        for (&(r, c), cell) in &ws.cells {
            let here = bo_model::sheet::CellRef { row: r, col: c };
            match &cell.formula {
                Some(f) => {
                    let text = formula_text(&masters, r, c, f);
                    let ok = text.is_some_and(|t| fw.set_formula(si, r, c, &t).is_ok());
                    if !ok {
                        // Unparsable formula: keep its cached value as a constant.
                        fw.set_value(si, r, c, to_value(&cell.value, system));
                    }
                }
                None => {
                    if !covered.iter().any(|a| a.contains(here) && a.first != here) {
                        fw.set_value(si, r, c, to_value(&cell.value, system));
                    }
                }
            }
        }
        for (&r, info) in &ws.rows {
            if info.hidden {
                fw.set_row_hidden(si, r, true);
            }
        }
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
    for dn in &wb.defined_names {
        let scope = dn.local_sheet.map(|s| s as usize);
        // Names that do not parse (external links, macros) are skipped.
        let _ = fw.define_name(&dn.name, scope, &dn.formula);
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
            if let (Some(a), Some(f)) = (spill, cell.formula.as_mut()) {
                if !f.text.is_empty() {
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

//! Read-only formula evaluation and reference resolution over the sheet model,
//! used by conditional formatting (ECMA-376 Part 1, §18.3.1.10 `cfRule/formula`),
//! data validation (§18.3.1.32) and charts (§21.2.2.66 `c:f`).
//!
//! Cells are read from their cached values, which [`crate::formula::BoFormula`]
//! keeps current, so nothing here recalculates the workbook.

use bo_formula::{
    evaluate, offset_relative, parse, CellProvider, DateSystem, EvalConfig, Expr, Value,
};
use bo_model::sheet::{CellRange, CellValue, Workbook};

/// [`CellProvider`] over a model workbook (cached values).
pub struct ModelCells<'a> {
    /// Workbook whose cached cell values are read.
    pub wb: &'a Workbook,
    config: EvalConfig,
}

impl<'a> ModelCells<'a> {
    /// Provider for `wb`; `now` is the serial used by `NOW()`/`TODAY()` (hosts
    /// without a clock, e.g. WASM, must pass one).
    pub fn new(wb: &'a Workbook, now: Option<f64>) -> Self {
        let date_system = if wb.date1904 {
            DateSystem::Date1904
        } else {
            DateSystem::Date1900
        };
        ModelCells {
            wb,
            config: EvalConfig {
                date_system,
                // Never read the system clock (unavailable on wasm32).
                now: Some(now.unwrap_or(45_658.0)),
                text_formatter: Some(crate::numfmt::text_function_format),
            },
        }
    }

    /// Evaluates `formula` (without `=`) written for `origin` as if it were in
    /// (`row`, `col`): relative references move by the distance (§18.3.1.10).
    pub fn eval_at(
        &self,
        sheet: usize,
        formula: &str,
        origin: (u32, u32),
        row: u32,
        col: u32,
    ) -> Value {
        let Ok(expr) = parse(formula) else {
            return Value::Error(bo_formula::ErrorKind::Name);
        };
        self.eval_expr(sheet, &expr, origin, row, col)
    }

    /// [`ModelCells::eval_at`] for a parsed expression.
    pub fn eval_expr(
        &self,
        sheet: usize,
        expr: &Expr,
        origin: (u32, u32),
        row: u32,
        col: u32,
    ) -> Value {
        let dr = i64::from(row) - i64::from(origin.0);
        let dc = i64::from(col) - i64::from(origin.1);
        let moved;
        let e = if dr == 0 && dc == 0 {
            expr
        } else {
            moved = offset_relative(expr, dr, dc);
            &moved
        };
        evaluate(e, self, sheet, row, col, &self.config)
    }
}

/// Model value → formula value.
pub fn to_formula_value(v: &CellValue) -> Value {
    match v {
        CellValue::Empty => Value::Empty,
        CellValue::Number(n) => Value::Number(*n),
        CellValue::Bool(b) => Value::Bool(*b),
        CellValue::Error(e) => Value::Error(
            bo_formula::ErrorKind::parse(e.as_str()).unwrap_or(bo_formula::ErrorKind::NA),
        ),
        CellValue::Date(s) => Value::Text(s.clone()),
        CellValue::Text(_) | CellValue::RichText(_) => Value::Text(v.as_text().unwrap_or_default()),
    }
}

impl CellProvider for ModelCells<'_> {
    fn sheet_count(&self) -> usize {
        self.wb.sheets.len()
    }

    fn sheet_name(&self, sheet: usize) -> Option<&str> {
        self.wb.sheets.get(sheet).map(|s| s.name.as_str())
    }

    fn cell_value(&self, sheet: usize, row: u32, col: u32) -> Value {
        self.wb
            .sheets
            .get(sheet)
            .and_then(|s| s.cell(row, col))
            .map_or(Value::Empty, |c| to_formula_value(&c.value))
    }

    fn cell_formula(&self, sheet: usize, row: u32, col: u32) -> Option<String> {
        self.wb
            .sheets
            .get(sheet)?
            .cell(row, col)?
            .formula
            .as_ref()
            .map(|f| format!("={}", f.text))
    }

    fn defined_name(&self, sheet: usize, name: &str) -> Option<Expr> {
        let names = &self.wb.defined_names;
        let scoped = names.iter().find(|d| {
            d.local_sheet == u32::try_from(sheet).ok() && d.name.eq_ignore_ascii_case(name)
        });
        let global = || {
            names
                .iter()
                .find(|d| d.local_sheet.is_none() && d.name.eq_ignore_ascii_case(name))
        };
        parse(&scoped.or_else(global)?.formula).ok()
    }

    fn used_extent(&self, sheet: usize) -> (u32, u32) {
        self.wb
            .sheets
            .get(sheet)
            .and_then(bo_model::sheet::Worksheet::used_range)
            .map_or((0, 0), |r| (r.last.row + 1, r.last.col + 1))
    }

    fn is_row_hidden(&self, sheet: usize, row: u32) -> bool {
        self.wb
            .sheets
            .get(sheet)
            .and_then(|s| s.rows.get(&row))
            .is_some_and(|r| r.hidden)
    }
}

/// Resolves a reference such as `Sheet1!$A$1:$B$3`, `'Bảng 1'!A1`, `A1:A5`
/// (on `default_sheet`) or a defined name to (sheet, range).
pub fn resolve_ref(wb: &Workbook, default_sheet: usize, text: &str) -> Option<(usize, CellRange)> {
    resolve_ref_depth(wb, default_sheet, text, 0)
}

fn resolve_ref_depth(
    wb: &Workbook,
    default_sheet: usize,
    text: &str,
    depth: u8,
) -> Option<(usize, CellRange)> {
    let t = text.trim().trim_start_matches('=');
    let (sheet, addr) = match t.rsplit_once('!') {
        Some((s, a)) => {
            let name = s.trim_matches('\'').replace("''", "'");
            let i = wb
                .sheets
                .iter()
                .position(|w| w.name.eq_ignore_ascii_case(&name))?;
            (i, a)
        }
        None => (default_sheet, t),
    };
    if let Some(r) = CellRange::parse(addr) {
        return Some((sheet, r));
    }
    if depth > 2 {
        return None;
    }
    // Defined name (§18.2.5): sheet scope first, then workbook scope.
    let dn = wb
        .defined_names
        .iter()
        .find(|d| {
            d.local_sheet == u32::try_from(default_sheet).ok() && d.name.eq_ignore_ascii_case(t)
        })
        .or_else(|| {
            wb.defined_names
                .iter()
                .find(|d| d.local_sheet.is_none() && d.name.eq_ignore_ascii_case(t))
        })?;
    resolve_ref_depth(wb, default_sheet, &dn.formula, depth + 1)
}

/// Quoted sheet name for formulas (`'Bảng 1'`), plain when it is a simple identifier.
pub fn quote_sheet(name: &str) -> String {
    let simple = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if simple {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// Absolute reference text `Sheet!$A$1:$B$2` of a range.
pub fn abs_ref(sheet_name: &str, r: CellRange) -> String {
    let cell = |c: bo_model::sheet::CellRef| {
        format!("${}${}", bo_model::sheet::col_to_letters(c.col), c.row + 1)
    };
    let body = if r.first == r.last {
        cell(r.first)
    } else {
        format!("{}:{}", cell(r.first), cell(r.last))
    };
    format!("{}!{body}", quote_sheet(sheet_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{DefinedName, Worksheet};

    #[test]
    fn evaluates_relative_formulas_and_resolves_refs() {
        let mut wb = Workbook::new();
        wb.sheets[0].set_a1("A1", CellValue::Number(5.0));
        wb.sheets[0].set_a1("A2", CellValue::Number(15.0));
        wb.sheets.push(Worksheet::new("Bảng 1"));
        wb.defined_names.push(DefinedName {
            name: "Ds".into(),
            formula: "'Bảng 1'!$B$1:$B$3".into(),
            ..DefinedName::default()
        });
        let m = ModelCells::new(&wb, None);
        assert_eq!(m.eval_at(0, "A1>10", (0, 0), 0, 0), Value::Bool(false));
        assert_eq!(m.eval_at(0, "A1>10", (0, 0), 1, 0), Value::Bool(true));
        assert_eq!(m.eval_at(0, "$A$1*2", (0, 0), 1, 0), Value::Number(10.0));
        assert_eq!(
            resolve_ref(&wb, 0, "'Bảng 1'!$B$1:$B$3"),
            Some((1, CellRange::parse("B1:B3").unwrap()))
        );
        assert_eq!(resolve_ref(&wb, 0, "ds").map(|r| r.0), Some(1));
        assert_eq!(resolve_ref(&wb, 0, "A1:A2").map(|r| r.0), Some(0));
        assert!(resolve_ref(&wb, 0, "Nope!A1").is_none());
        assert_eq!(quote_sheet("Sheet1"), "Sheet1");
        assert_eq!(quote_sheet("Bảng 1"), "'Bảng 1'");
        assert_eq!(
            abs_ref("Sheet1", CellRange::parse("A2:B4").unwrap()),
            "Sheet1!$A$2:$B$4"
        );
    }
}

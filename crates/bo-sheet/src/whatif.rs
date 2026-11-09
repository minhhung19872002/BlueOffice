//! What-if analysis: Goal Seek and data tables.
//!
//! Goal Seek changes one input cell (a constant) until a formula cell reaches a goal. The
//! search works on the formula engine's copy of the workbook and recalculates incrementally;
//! the caller applies the found input as an ordinary edit. Steps: secant steps from the current
//! input, falling back to bisection once the goal is bracketed, and widening the search while
//! it is not; stop when the result is within `tolerance` of the goal or after `max_iterations`.

use bo_formula::Value;
use bo_model::sheet::{CellRange, CellRef, Formula, FormulaKind, Workbook};
use serde::Serialize;

use crate::formula::{load, to_cell_value, write_back};

/// Outcome of a Goal Seek.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalSeek {
    /// Whether a solution within the tolerance was found.
    pub found: bool,
    /// The input value tried last (the solution when `found`).
    pub input: f64,
    /// The formula's value for that input.
    pub result: f64,
    /// Number of evaluations.
    pub iterations: u32,
}

/// Why a Goal Seek could not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalSeekError {
    /// The target cell holds no formula.
    TargetNotFormula,
    /// The changing cell holds a formula (it must be a constant).
    ChangingIsFormula,
}

/// Search for the value of `changing` that makes `target` equal `goal`.
///
/// # Errors
/// The target must be a formula and the changing cell a constant.
pub fn goal_seek(
    wb: &Workbook,
    sheet: usize,
    target: (u32, u32),
    goal: f64,
    changing: (u32, u32),
    max_iterations: u32,
    tolerance: f64,
) -> Result<GoalSeek, GoalSeekError> {
    let ws = &wb.sheets[sheet];
    if ws.cells.get(&target).is_none_or(|c| c.formula.is_none()) {
        return Err(GoalSeekError::TargetNotFormula);
    }
    let start_cell = ws.cells.get(&changing);
    if start_cell.is_some_and(|c| c.formula.is_some()) {
        return Err(GoalSeekError::ChangingIsFormula);
    }
    let start = match start_cell.map(|c| &c.value) {
        Some(bo_model::sheet::CellValue::Number(n)) => *n,
        _ => 0.0,
    };
    let mut fw = load(wb, None);
    fw.recalculate_all();
    let mut iterations = 0;
    // f(x) = target(x) − goal, NaN when the formula gives an error or no number.
    let mut eval = |x: f64| -> f64 {
        iterations += 1;
        fw.set_value(sheet, changing.0, changing.1, Value::Number(x));
        fw.recalculate();
        match fw.value(sheet, target.0, target.1) {
            Value::Number(v) => v - goal,
            Value::Bool(b) => f64::from(u8::from(b)) - goal,
            _ => f64::NAN,
        }
    };
    let mut x0 = start;
    let mut f0 = eval(x0);
    let done = |f: f64| f.is_finite() && f.abs() <= tolerance;
    let finish = |x: f64, f: f64, found: bool, it: u32| GoalSeek {
        found,
        // 15 significant digits, as a cell shows a number (3.9999999999999996 → 4).
        input: format!("{x:.14e}").parse().unwrap_or(x),
        result: f + goal,
        iterations: it,
    };
    if done(f0) {
        return Ok(finish(x0, f0, true, 1));
    }
    let mut step = if x0 == 0.0 { 1.0 } else { x0.abs() * 0.01 };
    let mut x1 = x0 + step;
    let mut f1 = eval(x1);
    // A bracket [a, b] with f(a), f(b) of opposite signs, once known.
    let mut bracket: Option<(f64, f64, f64, f64)> = None;
    let max = max_iterations.max(2);
    let mut count = 2;
    while count < max {
        if done(f1) {
            return Ok(finish(x1, f1, true, count));
        }
        if f0.is_finite() && f1.is_finite() && f0.signum() != f1.signum() {
            bracket = Some(if x0 < x1 {
                (x0, f0, x1, f1)
            } else {
                (x1, f1, x0, f0)
            });
        }
        let next = if let Some((a, fa, b, fb)) = bracket {
            // Secant inside the bracket, else bisection.
            let s = b - fb * (b - a) / (fb - fa);
            if s.is_finite() && s > a && s < b {
                s
            } else {
                f64::midpoint(a, b)
            }
        } else if f0.is_finite() && f1.is_finite() && f1 != f0 {
            let s = x1 - f1 * (x1 - x0) / (f1 - f0);
            // Limit wild jumps; keep widening while no bracket is known.
            let limit = (x1 - x0).abs().max(step) * 100.0;
            if s.is_finite() {
                x1 + (s - x1).clamp(-limit, limit)
            } else {
                x1 + step
            }
        } else {
            step *= 2.0;
            x1 + if f1.is_finite() { step } else { -step }
        };
        let fnext = eval(next);
        count += 1;
        if let Some((a, fa, b, fb)) = bracket {
            bracket = Some(if fnext.is_finite() && fnext.signum() == fa.signum() {
                (next, fnext, b, fb)
            } else {
                (a, fa, next, fnext)
            });
        }
        x0 = x1;
        f0 = f1;
        x1 = next;
        f1 = fnext;
    }
    let _ = (x0, f0);
    Ok(finish(x1, f1, done(f1), count))
}

// ------------------------------------------------------------------ data tables

/// A data table ({=TABLE(r1, r2)}, ECMA-376 Part 1, §18.3.1.40 `f` with `t="dataTable"`):
/// the result cells, the input cells and the orientation.
#[derive(Debug, Clone, PartialEq)]
pub struct DataTable {
    /// The result cells (`ref`).
    pub range: CellRange,
    /// Two input cells (`dt2D`): `r1` takes the top row's values, `r2` the left column's.
    pub two_d: bool,
    /// One input cell fed from the top row (`dtr`); otherwise from the left column.
    pub row_input: bool,
    /// First input cell (`r1`).
    pub r1: CellRef,
    /// Second input cell (`r2`, two-variable tables).
    pub r2: Option<CellRef>,
}

fn attr<'a>(f: &'a Formula, name: &str) -> Option<&'a str> {
    f.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn on(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true"))
}

impl DataTable {
    /// The data table a formula stands for.
    #[must_use]
    pub fn of(f: &Formula) -> Option<DataTable> {
        let FormulaKind::DataTable { range } = &f.kind else {
            return None;
        };
        Some(DataTable {
            range: CellRange::parse(range)?,
            two_d: on(attr(f, "dt2D")),
            row_input: on(attr(f, "dtr")),
            r1: CellRef::parse(&attr(f, "r1")?.replace('$', ""))?,
            r2: attr(f, "r2").and_then(|r| CellRef::parse(&r.replace('$', ""))),
        })
    }

    /// The formula a new data table is stored as (on the first result cell).
    #[must_use]
    pub fn formula(&self) -> Formula {
        let mut attrs = vec![
            (
                "dt2D".to_owned(),
                if self.two_d { "1" } else { "0" }.to_owned(),
            ),
            (
                "dtr".to_owned(),
                if self.row_input { "1" } else { "0" }.to_owned(),
            ),
            ("r1".to_owned(), self.r1.to_string()),
        ];
        if let Some(r2) = self.r2 {
            attrs.push(("r2".to_owned(), r2.to_string()));
        }
        Formula {
            text: String::new(),
            kind: FormulaKind::DataTable {
                range: self.range.to_string(),
            },
            calc_always: false,
            attrs,
        }
    }
}

/// Every data table of the workbook: (sheet, table).
fn tables(wb: &Workbook) -> Vec<(usize, DataTable)> {
    let mut out = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for cell in ws.cells.values() {
            if let Some(t) = cell.formula.as_ref().and_then(DataTable::of) {
                out.push((si, t));
            }
        }
    }
    out
}

/// Calculate the data tables: for each result cell the input cells take the values of its row
/// / column headers, the formula in the table's corner row or column is recalculated and its
/// value stored; the inputs are restored afterwards. Formulas that use the results are then
/// recalculated.
pub fn calculate_data_tables(fw: &mut bo_formula::Workbook, wb: &mut Workbook) {
    let list = tables(wb);
    if list.is_empty() {
        return;
    }
    let mut results: Vec<(usize, u32, u32, Value)> = Vec::new();
    for (si, t) in &list {
        let si = *si;
        let (r0, c0) = (t.range.first.row, t.range.first.col);
        if r0 == 0 || c0 == 0 {
            continue;
        }
        let inputs: Vec<CellRef> = std::iter::once(t.r1).chain(t.r2).collect();
        let saved: Vec<Value> = inputs.iter().map(|c| fw.value(si, c.row, c.col)).collect();
        for row in r0..=t.range.last.row {
            for col in c0..=t.range.last.col {
                let top = fw.value(si, r0 - 1, col);
                let left = fw.value(si, row, c0 - 1);
                let formula_cell = if t.two_d {
                    fw.set_value(si, t.r1.row, t.r1.col, top);
                    if let Some(r2) = t.r2 {
                        fw.set_value(si, r2.row, r2.col, left);
                    }
                    (r0 - 1, c0 - 1)
                } else if t.row_input {
                    fw.set_value(si, t.r1.row, t.r1.col, top);
                    (row, c0 - 1)
                } else {
                    fw.set_value(si, t.r1.row, t.r1.col, left);
                    (r0 - 1, col)
                };
                fw.recalculate();
                results.push((si, row, col, fw.value(si, formula_cell.0, formula_cell.1)));
            }
        }
        for (c, v) in inputs.iter().zip(saved) {
            fw.set_value(si, c.row, c.col, v);
        }
        fw.recalculate();
    }
    for (si, row, col, v) in results {
        let ws = &mut wb.sheets[si];
        let cell = ws.cells.entry((row, col)).or_default();
        cell.value = to_cell_value(&v);
        fw.set_value(si, row, col, v);
    }
    fw.recalculate();
    let mut by_sheet: Vec<Vec<(u32, u32)>> = vec![Vec::new(); wb.sheets.len()];
    for id in fw.last_recalculated() {
        if let Some(v) = by_sheet.get_mut(id.sheet) {
            // The anchor of a data table keeps its own result.
            let anchor = wb.sheets[id.sheet]
                .cells
                .get(&(id.row, id.col))
                .and_then(|c| c.formula.as_ref())
                .is_some_and(|f| matches!(f.kind, FormulaKind::DataTable { .. }));
            if !anchor {
                v.push((id.row, id.col));
            }
        }
    }
    for (si, anchors) in by_sheet.iter().enumerate() {
        if !anchors.is_empty() {
            write_back(fw, wb, si, anchors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellValue, Worksheet};

    fn book(cells: &[(&str, &str)]) -> Workbook {
        let mut ws = Worksheet::new("S");
        for (a1, input) in cells {
            if let Some(f) = input.strip_prefix('=') {
                let r = bo_model::sheet::CellRef::parse(a1).unwrap_or_default();
                ws.set_cell(r.row, r.col, Cell::default().with_formula(f));
            } else {
                ws.set_a1(a1, CellValue::Number(input.parse().unwrap_or(0.0)));
            }
        }
        let mut wb = Workbook::default();
        wb.sheets.push(ws);
        wb
    }

    #[test]
    fn data_tables_one_and_two_inputs() {
        use crate::calc::FormulaEngine;
        // Loan payment for rates in A3:A5 (column input B1) and terms in B2:C2 (row input B2… )
        // One-variable, column-oriented: B2 = A1*10, inputs A3:A5 → A1, results B3:B5.
        let mut wb = book(&[
            ("A1", "1"),
            ("B2", "=A1*10"),
            ("A3", "2"),
            ("A4", "3"),
            ("A5", "4"),
        ]);
        let t = DataTable {
            range: CellRange::parse("B3:B5").unwrap(),
            two_d: false,
            row_input: false,
            r1: CellRef::parse("A1").unwrap(),
            r2: None,
        };
        wb.sheets[0].set_cell(
            2,
            1,
            Cell {
                formula: Some(t.formula()),
                ..Cell::default()
            },
        );
        crate::formula::BoFormula::new().recalculate(&mut wb);
        let n = |wb: &Workbook, a1: &str| match wb.sheets[0].cell_a1(a1).map(|c| &c.value) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("{a1}: {other:?}"),
        };
        assert_eq!(n(&wb, "B3"), 20.0);
        assert_eq!(n(&wb, "B4"), 30.0);
        assert_eq!(n(&wb, "B5"), 40.0);
        // The input keeps its value and the formula its result.
        assert_eq!(n(&wb, "A1"), 1.0);
        assert_eq!(n(&wb, "B2"), 10.0);
        // Two inputs: D1 * E1 with D1 from the top row (G6:H6) and E1 from the left column
        // (F7:F8); corner F6 holds the formula.
        let mut wb = book(&[
            ("D1", "1"),
            ("E1", "1"),
            ("F6", "=D1*E1"),
            ("G6", "2"),
            ("H6", "3"),
            ("F7", "10"),
            ("F8", "20"),
        ]);
        let t = DataTable {
            range: CellRange::parse("G7:H8").unwrap(),
            two_d: true,
            row_input: false,
            r1: CellRef::parse("D1").unwrap(),
            r2: Some(CellRef::parse("E1").unwrap()),
        };
        assert_eq!(DataTable::of(&t.formula()), Some(t.clone()));
        wb.sheets[0].set_cell(
            6,
            6,
            Cell {
                formula: Some(t.formula()),
                ..Cell::default()
            },
        );
        crate::formula::BoFormula::new().recalculate(&mut wb);
        assert_eq!(n(&wb, "G7"), 20.0);
        assert_eq!(n(&wb, "H7"), 30.0);
        assert_eq!(n(&wb, "G8"), 40.0);
        assert_eq!(n(&wb, "H8"), 60.0);
        assert_eq!(n(&wb, "F6"), 1.0);
    }

    #[test]
    fn solves_linear_and_nonlinear_targets() {
        // B1 = 2*A1 + 3 = 11 → A1 = 4.
        let wb = book(&[("A1", "1"), ("B1", "=2*A1+3")]);
        let r = goal_seek(&wb, 0, (0, 1), 11.0, (0, 0), 100, 1e-3).unwrap();
        assert!(r.found);
        assert!((r.input - 4.0).abs() < 1e-3, "{r:?}");
        // Loan payment: PMT(A1/12, 360, 200000) = -1000 → yearly rate about 4.2 %.
        let wb = book(&[("A1", "0.05"), ("B1", "=PMT(A1/12,360,200000)")]);
        let r = goal_seek(&wb, 0, (0, 1), -1000.0, (0, 0), 100, 1e-6).unwrap();
        assert!(r.found, "{r:?}");
        assert!((r.result + 1000.0).abs() <= 1e-6);
        // x² = 2 from 1 → √2.
        let wb = book(&[("A1", "1"), ("B1", "=A1^2")]);
        let r = goal_seek(&wb, 0, (0, 1), 2.0, (0, 0), 100, 1e-9).unwrap();
        assert!((r.input - 2f64.sqrt()).abs() < 1e-6, "{r:?}");
        // No solution: x² = −1.
        let r = goal_seek(&wb, 0, (0, 1), -1.0, (0, 0), 100, 1e-6).unwrap();
        assert!(!r.found);
        // Argument checks.
        assert_eq!(
            goal_seek(&wb, 0, (0, 0), 1.0, (0, 1), 100, 1e-3),
            Err(GoalSeekError::TargetNotFormula)
        );
        assert_eq!(
            goal_seek(&wb, 0, (0, 1), 1.0, (0, 1), 100, 1e-3),
            Err(GoalSeekError::ChangingIsFormula)
        );
    }
}

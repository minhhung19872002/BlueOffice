//! What-if analysis: Goal Seek.
//!
//! Goal Seek changes one input cell (a constant) until a formula cell reaches a goal. The
//! search works on the formula engine's copy of the workbook and recalculates incrementally;
//! the caller applies the found input as an ordinary edit. Steps: secant steps from the current
//! input, falling back to bisection once the goal is bracketed, and widening the search while
//! it is not; stop when the result is within `tolerance` of the goal or after `max_iterations`.

use bo_formula::Value;
use bo_model::sheet::Workbook;
use serde::Serialize;

use crate::formula::load;

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

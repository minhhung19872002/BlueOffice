//! The `solve` command's work: runs the Solver (`bo_sheet::solver`, any method), enters the
//! solution and writes the requested reports (`bo_sheet::solver_report`) on new sheets — all
//! in one undo step.

use super::analysis_cmd::free_name;
use super::cmd::Recalc;
use super::SheetEngine;
use crate::Result;
use bo_model::sheet::{CellRange, CellRef, CellValue, Worksheet};
use bo_sheet::solver::{Model, Status};
use bo_sheet::solver_report::{self, RCell, ReportKind};
use bo_sheet::style::StylePatch;
use serde_json::{json, Value};

impl SheetEngine {
    /// Solves `model` on `sheet`; with `apply` a successful solution is entered, and every
    /// kind in `reports` is written on a new sheet. Returns the solution
    /// (`{ status, objective, values }`) plus `reports: [{ kind, sheet }]` when any were made.
    pub(super) fn solve_model(
        &mut self,
        sheet: usize,
        model: &Model,
        reports: &[ReportKind],
        apply: bool,
    ) -> Result<Value> {
        self.sheet(sheet)?;
        let vi = self.locale == bo_sheet::numfmt::Locale::VI_VN;
        let wb = self.wb()?;
        let sol = bo_sheet::solver::solve(wb, sheet, model);
        let ok = matches!(sol.status, Status::Optimal | Status::Limit) && !sol.values.is_empty();
        let tables = if ok && !reports.is_empty() {
            solver_report::build(wb, sheet, model, &sol, reports, vi)
        } else {
            Vec::new()
        };
        let mut out = serde_json::to_value(&sol).unwrap_or(Value::Null);
        if !ok || (!apply && tables.is_empty()) {
            return Ok(out);
        }
        let values = sol.values.clone();
        let made = self.edit(sheet, Recalc::All, |wb, _| {
            if apply {
                let ws = &mut wb.sheets[sheet];
                for &((r, c), v) in &values {
                    let cell = ws.cells.entry((r, c)).or_default();
                    cell.formula = None;
                    cell.value = CellValue::Number(v);
                }
            }
            let mut made = Vec::new();
            for table in &tables {
                super::protect::check_structure(wb)?;
                let mut ws = Worksheet::new(&free_name(wb, &table.name));
                ws.sheet_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
                for (r, row) in (0u32..).zip(&table.rows) {
                    for (c, cell) in (0u32..).zip(row) {
                        let value = match cell {
                            RCell::Empty => continue,
                            RCell::Text(t) => CellValue::Text(t.clone()),
                            RCell::Num(n) => CellValue::Number(*n),
                        };
                        ws.cells.entry((r, c)).or_default().value = value;
                    }
                }
                wb.sheets.push(ws);
                let si = wb.sheets.len() - 1;
                let bold = StylePatch {
                    bold: Some(true),
                    ..StylePatch::default()
                };
                for &r in &table.strong {
                    let width = table.rows.get(r).map_or(1, Vec::len).max(1);
                    let r = u32::try_from(r).unwrap_or(0);
                    let last = u32::try_from(width - 1).unwrap_or(0);
                    let range = CellRange::new(CellRef::new(r, 0), CellRef::new(r, last));
                    bo_sheet::style::apply_patch(wb, si, range, &bold);
                }
                made.push(json!({ "kind": table.kind, "sheet": si }));
            }
            Ok(made)
        })?;
        if !tables.is_empty() {
            // New sheets need their own geometry slots.
            self.invalidate(None);
            out["reports"] = Value::Array(made);
        }
        Ok(out)
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

    #[allow(clippy::needless_pass_by_value)]
    fn run(e: &mut SheetEngine, v: Value) -> Value {
        serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
    }

    fn text(e: &SheetEngine, sheet: usize, row: u32, col: u32) -> CellValue {
        e.wb().unwrap().sheets[sheet]
            .cells
            .get(&(row, col))
            .map(|c| c.value.clone())
            .unwrap_or_default()
    }

    #[test]
    fn nonlinear_solve_with_reports_is_one_undo_step() {
        let mut e = engine();
        e.set_locale("en-US");
        // max xy s.t. x + y ≤ 10 → (5, 5), value 25.
        e.set_inputs(
            0,
            &[
                (0, 0, "1".into()),
                (0, 1, "1".into()),
                (0, 2, "=A1*B1".into()),
                (1, 2, "=A1+B1".into()),
            ],
        )
        .unwrap();
        let r = run(
            &mut e,
            json!({
                "cmd": "solve", "sheet": 0, "objective": "C1", "goal": "max",
                "variables": ["A1:B1"],
                "constraints": [{"lhs": "C2", "relation": "le", "rhs": "10"}],
                "method": "nonlinear", "options": {"convergence": 1e-7},
                "reports": ["answer", "sensitivity", "limits"], "apply": true
            }),
        );
        assert_eq!(r["status"], "optimal");
        assert!(
            (r["objective"].as_f64().unwrap() - 25.0).abs() < 1e-3,
            "{r}"
        );
        let reports = r["reports"].as_array().unwrap();
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0]["kind"], "answer");
        assert_eq!(reports[1]["sheet"], 2);
        let names: Vec<String> = e
            .wb()
            .unwrap()
            .sheets
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(
            names[1..],
            ["Solver result", "Solver sensitivity", "Solver limits"]
        );
        assert_eq!(
            text(&e, 1, 0, 0),
            CellValue::Text("Optimisation result".into())
        );
        let x = e.cell(0, 0, 0).unwrap().display;
        assert!(x.starts_with('5') || x.starts_with("4.9"), "{x}");
        // One undo removes the reports and restores the start values.
        assert!(e.undo().unwrap());
        assert_eq!(e.wb().unwrap().sheets.len(), 1);
        assert_eq!(e.cell(0, 0, 0).unwrap().display, "1");
    }

    #[test]
    fn evolutionary_alldifferent_and_old_json() {
        let mut e = engine();
        // Order three jobs: cost = 3·pos(A) + 2·pos(B) + pos(C) is least with A first:
        // (1, 2, 3) → 3 + 4 + 3 = 10 (the rearrangement inequality).
        e.set_inputs(
            0,
            &[
                (0, 0, "3".into()),
                (0, 1, "2".into()),
                (0, 2, "1".into()),
                (0, 3, "=3*A1+2*B1+C1".into()),
            ],
        )
        .unwrap();
        let r = run(
            &mut e,
            json!({
                "cmd": "solve", "sheet": 0, "objective": "D1", "goal": "min",
                "variables": ["A1:C1"], "constraints": [{"lhs": "A1:C1", "relation": "dif"}],
                "method": "evolutionary", "options": {"seed": 4, "population": 12}
            }),
        );
        assert_eq!(r["objective"], 10.0, "{r}");
        assert_eq!(r["values"][0][1], 1.0);
        // Missing bounds.
        let r = run(
            &mut e,
            json!({
                "cmd": "solve", "sheet": 0, "objective": "D1", "goal": "min",
                "variables": ["A1:C1"], "method": "evolutionary"
            }),
        );
        assert_eq!(r["status"], "missingBounds");
        // The simplex default and its JSON are unchanged; a report without apply leaves
        // the model sheet alone.
        let r = run(
            &mut e,
            json!({
                "cmd": "solve", "sheet": 0, "objective": "D1", "goal": "min",
                "variables": ["A1:C1"], "constraints": [{"lhs": "A1:C1", "relation": "dif"}],
                "reports": ["answer"]
            }),
        );
        assert_eq!(r["status"], "optimal");
        assert_eq!(r["objective"], 10.0);
        assert!(r.get("detail").is_none());
        assert_eq!(r["reports"][0]["sheet"], 1);
        assert_eq!(e.cell(0, 0, 0).unwrap().display, "3");
        // vi-VN is the default locale: Vietnamese report labels.
        assert_eq!(e.wb().unwrap().sheets[1].name, "Kết quả tối ưu");
    }
}

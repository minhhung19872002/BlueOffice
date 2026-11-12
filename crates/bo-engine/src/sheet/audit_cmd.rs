//! BlueSheet formula auditing and calculation commands (SHEET-AUDIT-001):
//!
//! * `traceArrows` — precedent / dependent arrows of a cell, `levels` deep;
//! * `evaluateFormula` — step-by-step evaluation ([`bo_formula::trace_formula`]);
//! * `errorCheck`, `ignoreError`, `resetIgnoredErrors` — error checking
//!   ([`bo_sheet::errcheck`], ECMA-376 Part 1, §18.3.1.51 `ignoredErrors`);
//! * `watchValues` — rows of the watch window; `cellWatches` / `setCellWatches`
//!   — the watched cells saved with each sheet (CT_Worksheet `cellWatches`; a
//!   change is not an undo step, as adding a watch is not an edit of the cells);
//! * `checkFormula` — does a formula text parse, and are its functions known (used
//!   to vet a formula suggested from outside before it is offered; CORE-AI-001);
//! * `calcOptions`, `setCalcOptions`, `calculate` — calculation mode, iteration
//!   and precision (§18.2.2 `calcPr`), calculate now / calculate sheet.
//!
//! Commands come as JSON like every sheet command (`{"cmd": "traceArrows", …}`)
//! and are parsed before [`super::cmd::SheetCommand`].

use super::cmd::Recalc;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_formula::{Area, CellId, Expr, Value as FValue};
use bo_model::sheet::{CellRef, CellValue, Workbook};
use bo_sheet::calcopts::CalcSettings;
use bo_sheet::errcheck::{self, ErrorRule, ErrorRules};
use serde::Deserialize;
use serde_json::{json, Value};

/// Most arrows returned by one `traceArrows`.
const MAX_ARROWS: usize = 2000;
/// Areas larger than this are searched for formula cells through the sheet's cells.
const SCAN_LIMIT: u64 = 4096;

/// A cell of the watch window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct WatchCell {
    /// 0-based sheet index.
    pub sheet: usize,
    /// Row (0-based).
    pub row: u32,
    /// Column (0-based).
    pub col: u32,
}

/// An auditing / calculation command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AuditCommand {
    /// Arrows from precedents to the cell (`dependents: false`) or from the
    /// cell to its dependents, `levels` deep: `{ arrows: [{ from: { sheet, r0,
    /// c0, r1, c1 }, to: { sheet, row, col }, level, error }], more }`.
    TraceArrows {
        sheet: usize,
        row: u32,
        col: u32,
        #[serde(default)]
        dependents: bool,
        #[serde(default = "one")]
        levels: u32,
    },
    /// Step-by-step evaluation of a cell's formula, `null` without one.
    EvaluateFormula { sheet: usize, row: u32, col: u32 },
    /// Syntax check of a formula text, nothing is changed or evaluated:
    /// `{ ok, error: { message, pos } | null, functions, unknownFunctions }`.
    CheckFormula { formula: String },
    /// Error-checking findings of a sheet (row-major): `[{ row, col, rule, … }]`.
    ErrorCheck {
        sheet: usize,
        #[serde(default)]
        rules: ErrorRules,
    },
    /// Ignore one finding (saved with the sheet). One undo step.
    IgnoreError {
        sheet: usize,
        row: u32,
        col: u32,
        rule: ErrorRule,
    },
    /// Forget the ignored findings of every sheet. One undo step.
    ResetIgnoredErrors,
    /// Watch-window rows: `[{ book, sheet, name, cell, value, formula }]`.
    WatchValues { cells: Vec<WatchCell> },
    /// The watched cells saved in the workbook: `[{ sheet, row, col }]`.
    CellWatches,
    /// Replaces the watched cells saved in the workbook (no undo step).
    SetCellWatches { cells: Vec<WatchCell> },
    /// Calculation settings and state: `{ options, needsCalc, circular }`.
    CalcOptions,
    /// Change the calculation settings (§18.2.2 `calcPr`). One undo step.
    SetCalcOptions { options: CalcSettings },
    /// Calculate now (`sheet` absent), the sheet only, or everything (`full`).
    Calculate {
        #[serde(default)]
        sheet: Option<usize>,
        #[serde(default)]
        full: bool,
    },
}

fn one() -> u32 {
    1
}

impl AuditCommand {
    /// Parses a JSON command when it is an auditing command.
    pub fn parse(v: &Value) -> Option<AuditCommand> {
        const NAMES: &[&str] = &[
            "traceArrows",
            "evaluateFormula",
            "checkFormula",
            "errorCheck",
            "ignoreError",
            "resetIgnoredErrors",
            "watchValues",
            "cellWatches",
            "setCellWatches",
            "calcOptions",
            "setCalcOptions",
            "calculate",
        ];
        let name = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&name) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// The command changes the workbook file.
    fn edits(&self) -> bool {
        matches!(
            self,
            AuditCommand::IgnoreError { .. }
                | AuditCommand::ResetIgnoredErrors
                | AuditCommand::SetCalcOptions { .. }
        )
    }
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn area_json(a: &Area) -> Value {
    json!({ "sheet": a.sheet, "r0": a.r1, "c0": a.c1, "r1": a.r2, "c1": a.c2 })
}

/// `Sheet1!A1` (quoted sheet name when needed).
fn cell_label(wb: &Workbook, sheet: usize, row: u32, col: u32) -> String {
    let name = wb.sheets.get(sheet).map_or("", |s| s.name.as_str());
    let quoted = if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    };
    format!("{quoted}!{}", CellRef::new(row, col))
}

/// Formula cells of the model inside an area.
fn formula_cells(wb: &Workbook, a: &Area) -> Vec<CellId> {
    let Some(ws) = wb.sheets.get(a.sheet) else {
        return Vec::new();
    };
    let is_formula = |r: u32, c: u32| ws.cell(r, c).is_some_and(|x| x.formula.is_some());
    if a.size() <= SCAN_LIMIT {
        let mut out = Vec::new();
        for r in a.r1..=a.r2 {
            for c in a.c1..=a.c2 {
                if is_formula(r, c) {
                    out.push(CellId::new(a.sheet, r, c));
                }
            }
        }
        out
    } else {
        let mut out: Vec<CellId> = ws
            .cells
            .iter()
            .filter(|((r, c), x)| x.formula.is_some() && a.contains(a.sheet, *r, *c))
            .map(|(&(r, c), _)| CellId::new(a.sheet, r, c))
            .collect();
        out.sort_unstable();
        out
    }
}

/// `checkFormula`: parses `formula` and lists the functions it calls; a function is
/// unknown when it is neither built in nor a defined name of the workbook.
fn check_formula(wb: &Workbook, formula: &str) -> Value {
    match bo_formula::parse(formula) {
        Err(e) => json!({
            "ok": false,
            "error": { "message": e.message, "pos": e.pos },
            "functions": [],
            "unknownFunctions": [],
        }),
        Ok(expr) => {
            let mut called: Vec<String> = Vec::new();
            expr.walk(&mut |e| {
                if let Expr::Call { name, .. } = e {
                    if !called.contains(name) {
                        called.push(name.clone());
                    }
                }
            });
            let builtin = bo_formula::function_names();
            let unknown: Vec<&String> = called
                .iter()
                .filter(|n| {
                    builtin.binary_search(&n.as_str()).is_err()
                        && !wb
                            .defined_names
                            .iter()
                            .any(|d| d.name.eq_ignore_ascii_case(n))
                })
                .collect();
            json!({
                "ok": unknown.is_empty(),
                "error": null,
                "functions": called,
                "unknownFunctions": unknown,
            })
        }
    }
}

fn has_error(wb: &Workbook, a: &Area) -> bool {
    a.is_single()
        && wb
            .sheets
            .get(a.sheet)
            .and_then(|s| s.cell(a.r1, a.c1))
            .is_some_and(|c| matches!(c.value, CellValue::Error(_)))
}

impl SheetEngine {
    /// Runs an auditing command.
    pub fn audit(&mut self, cmd: AuditCommand) -> Result<Value> {
        if cmd.edits() {
            self.writable()?;
        }
        if self.live.engine().is_none() {
            if let Some(wb) = self.wb.as_mut() {
                self.live.rebuild(wb);
            }
        }
        match cmd {
            AuditCommand::TraceArrows {
                sheet,
                row,
                col,
                dependents,
                levels,
            } => self.trace_arrows(sheet, row, col, dependents, levels.max(1)),
            AuditCommand::EvaluateFormula { sheet, row, col } => {
                self.evaluate_formula(sheet, row, col)
            }
            AuditCommand::CheckFormula { formula } => {
                // What typing it would store: `;` separators, decimal commas and
                // parentheses left open are read as a vi-VN user means them.
                let fixed = bo_formula::localized_input(&formula, self.locale.decimal);
                let mut v = check_formula(self.wb()?, fixed.as_deref().unwrap_or(&formula));
                v["fixed"] = fixed.map_or(Value::Null, Value::String);
                Ok(v)
            }
            AuditCommand::ErrorCheck { sheet, rules } => {
                let wb = self.wb()?;
                self.sheet(sheet)?;
                let locale = self.locale;
                let number = |s: &str| super::parse_number(s, &locale);
                let found = errcheck::check(wb, sheet, &rules, self.live.engine(), &number);
                serde_json::to_value(found).map_err(|e| bad(e.to_string()))
            }
            AuditCommand::IgnoreError {
                sheet,
                row,
                col,
                rule,
            } => {
                self.edit(sheet, Recalc::No, |wb, _| {
                    errcheck::ignore(&mut wb.sheets[sheet], row, col, rule);
                    Ok(())
                })?;
                Ok(json!({ "ok": true }))
            }
            AuditCommand::ResetIgnoredErrors => {
                self.edit(0, Recalc::No, |wb, _| {
                    for ws in &mut wb.sheets {
                        errcheck::reset_ignored(ws);
                    }
                    Ok(())
                })?;
                Ok(json!({ "ok": true }))
            }
            AuditCommand::WatchValues { cells } => self.watch_values(&cells),
            AuditCommand::CellWatches => {
                let wb = self.wb()?;
                let mut out = Vec::new();
                for (i, ws) in wb.sheets.iter().enumerate() {
                    for c in errcheck::cell_watches(ws) {
                        out.push(json!({ "sheet": i, "row": c.row, "col": c.col }));
                    }
                }
                Ok(Value::Array(out))
            }
            AuditCommand::SetCellWatches { cells } => {
                let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
                let mut changed = false;
                for (i, ws) in wb.sheets.iter_mut().enumerate() {
                    let mine: Vec<CellRef> = cells
                        .iter()
                        .filter(|w| w.sheet == i)
                        .map(|w| CellRef::new(w.row, w.col))
                        .collect();
                    changed |= errcheck::set_cell_watches(ws, &mine);
                }
                Ok(json!({ "changed": changed }))
            }
            AuditCommand::CalcOptions => self.calc_state(),
            AuditCommand::SetCalcOptions { options } => {
                if !options.iterate_delta.is_finite() || options.iterate_delta < 0.0 {
                    return Err(bad("invalid maximum change"));
                }
                let old = CalcSettings::read(self.wb()?);
                if old != options {
                    // Turning precision as displayed on rounds the stored constants
                    // for good, in the same undo step.
                    let round = old.full_precision && !options.full_precision;
                    self.edit(0, Recalc::All, |wb, _| {
                        options.write(wb);
                        if round {
                            bo_sheet::calcopts::round_constants_to_display(wb);
                        }
                        Ok(())
                    })?;
                    self.invalidate(None);
                }
                self.calc_state()
            }
            AuditCommand::Calculate { sheet, full } => {
                let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
                if full {
                    self.live.rebuild(wb);
                } else {
                    self.live.calculate(wb, sheet);
                }
                self.invalidate(None);
                self.calc_state()
            }
        }
    }

    fn calc_state(&self) -> Result<Value> {
        let wb = self.wb()?;
        let circular: Vec<Value> = self
            .live
            .circular_references()
            .into_iter()
            .map(|c| {
                json!({
                    "sheet": c.sheet,
                    "row": c.row,
                    "col": c.col,
                    "label": cell_label(wb, c.sheet, c.row, c.col),
                })
            })
            .collect();
        Ok(json!({
            "options": CalcSettings::read(wb),
            "needsCalc": self.live.needs_calculation(),
            "circular": circular,
        }))
    }

    fn trace_arrows(
        &self,
        sheet: usize,
        row: u32,
        col: u32,
        dependents: bool,
        levels: u32,
    ) -> Result<Value> {
        let wb = self.wb()?;
        self.sheet(sheet)?;
        let Some(fw) = self.live.engine() else {
            return Ok(json!({ "arrows": [], "more": false }));
        };
        let start = CellId::new(sheet, row, col);
        let mut frontier = vec![start];
        let mut seen: std::collections::HashSet<CellId> = std::iter::once(start).collect();
        let mut arrows = Vec::new();
        let mut more = false;
        'levels: for level in 1..=levels.saturating_add(1) {
            let mut next = Vec::new();
            for c in &frontier {
                let links: Vec<(Area, CellId)> = if dependents {
                    fw.direct_dependents(c.sheet, c.row, c.col)
                        .into_iter()
                        .map(|d| (Area::cell(c.sheet, c.row, c.col), d))
                        .collect()
                } else {
                    fw.direct_precedents(c.sheet, c.row, c.col)
                        .into_iter()
                        .map(|a| (a, *c))
                        .collect()
                };
                for (from, to) in links {
                    if level > levels {
                        more = true;
                        break 'levels;
                    }
                    if arrows.len() >= MAX_ARROWS {
                        more = true;
                        break 'levels;
                    }
                    arrows.push(json!({
                        "from": area_json(&from),
                        "to": { "sheet": to.sheet, "row": to.row, "col": to.col },
                        "level": level,
                        "error": has_error(wb, &from),
                    }));
                    let reached = if dependents {
                        vec![to]
                    } else {
                        formula_cells(wb, &from)
                    };
                    for f in reached {
                        if seen.insert(f) {
                            next.push(f);
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        Ok(json!({ "arrows": arrows, "more": more }))
    }

    fn evaluate_formula(&self, sheet: usize, row: u32, col: u32) -> Result<Value> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let Some(fw) = self.live.engine() else {
            return Ok(Value::Null);
        };
        let Some(t) = fw.trace(sheet, row, col) else {
            return Ok(Value::Null);
        };
        let formula = ws
            .cell(row, col)
            .and_then(|c| c.formula.as_ref())
            .map(|f| format!("={}", f.text))
            .unwrap_or_default();
        let steps: Vec<Value> = t
            .steps
            .iter()
            .map(|s| {
                json!({
                    "before": s.before,
                    "part": s.part,
                    "after": s.after,
                    "shown": s.shown,
                    "error": matches!(s.value, FValue::Error(_)),
                    "cell": s.cell.map(|c| json!({
                        "sheet": c.sheet,
                        "row": c.row,
                        "col": c.col,
                        "label": cell_label(wb, c.sheet, c.row, c.col),
                    })),
                })
            })
            .collect();
        Ok(json!({
            "cell": cell_label(wb, sheet, row, col),
            "formula": formula,
            "steps": steps,
            "result": t.shown,
            "truncated": t.truncated,
        }))
    }

    fn watch_values(&self, cells: &[WatchCell]) -> Result<Value> {
        let wb = self.wb()?;
        let mut rows = Vec::with_capacity(cells.len());
        for w in cells {
            let Some(ws) = wb.sheets.get(w.sheet) else {
                // The sheet was deleted: the watch shows a reference error.
                rows.push(json!({
                    "book": self.name, "sheet": null, "name": null,
                    "cell": CellRef::new(w.row, w.col).to_string(),
                    "value": "#REF!", "formula": null,
                }));
                continue;
            };
            let d = self.cell(w.sheet, w.row, w.col)?;
            let formula = d.input.starts_with('=').then_some(d.input.clone());
            rows.push(json!({
                "book": self.name,
                "sheet": ws.name,
                "name": defined_name_of(wb, w.sheet, w.row, w.col),
                "cell": d.address,
                "value": d.display,
                "formula": formula,
            }));
        }
        Ok(Value::Array(rows))
    }
}

/// A defined name (§18.2.5) designating exactly this cell.
fn defined_name_of(wb: &Workbook, sheet: usize, row: u32, col: u32) -> Option<String> {
    let name = &wb.sheets.get(sheet)?.name;
    wb.defined_names
        .iter()
        .filter(|d| !d.hidden && !d.name.starts_with("_xlnm."))
        .filter(|d| d.local_sheet.is_none_or(|s| s as usize == sheet))
        .find(|d| {
            let Ok(Expr::Ref(r)) = bo_formula::parse(&d.formula) else {
                return false;
            };
            let on_sheet = r
                .sheet
                .as_ref()
                .is_some_and(|s| s.last.is_none() && s.first.eq_ignore_ascii_case(name));
            on_sheet && r.bounds() == (row, col, row, col)
        })
        .map(|d| d.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(cells: &[(usize, &str, &str)], sheets: &[&str]) -> SheetEngine {
        let mut e = SheetEngine::new();
        let mut wb = Workbook::new();
        wb.sheets[0].name = sheets[0].to_owned();
        for s in &sheets[1..] {
            wb.sheets.push(bo_model::sheet::Worksheet::new(s));
        }
        e.load(wb, "t.xlsx").unwrap();
        for (s, a1, input) in cells {
            let c = CellRef::parse(a1).unwrap();
            e.set_input(*s, c.row, c.col, input).unwrap();
        }
        e
    }

    fn run(e: &mut SheetEngine, v: &Value) -> Value {
        let cmd = AuditCommand::parse(v).expect("audit command");
        e.audit(cmd).unwrap()
    }

    #[test]
    fn trace_arrows_through_indirect() {
        // B1 = INDIRECT(C1) where C1 holds "A1": arrows from C1 and from A1.
        let mut e = engine(
            &[
                (0, "A1", "4"),
                (0, "C1", "A1"),
                (0, "B1", "=INDIRECT(C1)+1"),
            ],
            &["Sheet1"],
        );
        let p = run(
            &mut e,
            &json!({"cmd": "traceArrows", "sheet": 0, "row": 0, "col": 1}),
        );
        let froms: Vec<Value> = p["arrows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["from"].clone())
            .collect();
        assert_eq!(
            froms,
            vec![
                json!({"sheet": 0, "r0": 0, "c0": 2, "r1": 0, "c1": 2}),
                json!({"sheet": 0, "r0": 0, "c0": 0, "r1": 0, "c1": 0}),
            ]
        );
        let d = run(
            &mut e,
            &json!({"cmd": "traceArrows", "sheet": 0, "row": 0, "col": 0, "dependents": true}),
        );
        assert_eq!(
            d["arrows"][0]["to"],
            json!({"sheet": 0, "row": 0, "col": 1})
        );
    }

    #[test]
    fn trace_arrows_by_level() {
        // A1 → B1 → C1 → Two!A1; C1 also reads D1:D2.
        let mut e = engine(
            &[
                (0, "A1", "1"),
                (0, "B1", "=A1*2"),
                (0, "D1", "5"),
                (0, "C1", "=B1+SUM(D1:D2)"),
                (1, "A1", "=Sheet1!C1"),
            ],
            &["Sheet1", "Two"],
        );
        let p1 = run(
            &mut e,
            &json!({"cmd": "traceArrows", "sheet": 0, "row": 0, "col": 2}),
        );
        let froms: Vec<Value> = p1["arrows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["from"].clone())
            .collect();
        assert_eq!(
            froms,
            vec![
                json!({"sheet": 0, "r0": 0, "c0": 1, "r1": 0, "c1": 1}),
                json!({"sheet": 0, "r0": 0, "c0": 3, "r1": 1, "c1": 3}),
            ]
        );
        // B1 holds a formula: a second level exists (A1 → B1).
        assert_eq!(p1["more"], json!(true));
        let p2 = run(
            &mut e,
            &json!({"cmd": "traceArrows", "sheet": 0, "row": 0, "col": 2, "levels": 2}),
        );
        let arrows = p2["arrows"].as_array().unwrap();
        assert_eq!(arrows.len(), 3);
        assert_eq!(arrows[2]["level"], json!(2));
        assert_eq!(arrows[2]["to"], json!({"sheet": 0, "row": 0, "col": 1}));
        assert_eq!(p2["more"], json!(false));
        // Dependents of A1: B1, then C1, then Two!A1 (another sheet).
        let d = run(
            &mut e,
            &json!({"cmd": "traceArrows", "sheet": 0, "row": 0, "col": 0, "dependents": true, "levels": 3}),
        );
        let to: Vec<Value> = d["arrows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["to"].clone())
            .collect();
        assert_eq!(
            to,
            vec![
                json!({"sheet": 0, "row": 0, "col": 1}),
                json!({"sheet": 0, "row": 0, "col": 2}),
                json!({"sheet": 1, "row": 0, "col": 0}),
            ]
        );
    }

    #[test]
    fn check_formula_reports_syntax_and_unknown_functions() {
        let mut e = engine(&[(0, "A1", "5")], &["Sheet1"]);
        let ok = run(
            &mut e,
            &json!({"cmd": "checkFormula", "formula": "=SUM(A1:A3)/COUNT(A1:A3)"}),
        );
        assert_eq!(ok["ok"], json!(true));
        assert_eq!(ok["functions"], json!(["SUM", "COUNT"]));
        assert!(ok["error"].is_null());
        let syntax = run(
            &mut e,
            &json!({"cmd": "checkFormula", "formula": "=SUM(A1:A3))"}),
        );
        assert_eq!(syntax["ok"], json!(false));
        assert!(syntax["error"]["message"].is_string());
        // A parenthesis left open at the end is closed (what typing it stores).
        let open = run(
            &mut e,
            &json!({"cmd": "checkFormula", "formula": "=SUM(A1:A3"}),
        );
        assert_eq!(open["ok"], json!(true));
        assert_eq!(open["fixed"], json!("=SUM(A1:A3)"));
        let unknown = run(
            &mut e,
            &json!({"cmd": "checkFormula", "formula": "=NOSUCHFUNCTION(A1)+SUM(A1)"}),
        );
        assert_eq!(unknown["ok"], json!(false));
        assert_eq!(unknown["unknownFunctions"], json!(["NOSUCHFUNCTION"]));
        // Typed with `;` separators (vi-VN): read as the stored `,` form.
        let semi = run(
            &mut e,
            &json!({"cmd": "checkFormula", "formula": "=IF(A1>1;A1;0)"}),
        );
        assert_eq!(semi["ok"], json!(true));
        assert_eq!(semi["fixed"], json!("=IF(A1>1,A1,0)"));
        // Nothing was written or evaluated.
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "5");
    }

    #[test]
    fn evaluate_watch_and_error_check() {
        let mut e = engine(
            &[
                (0, "A1", "5"),
                (0, "B1", "3"),
                (0, "C1", "=IF(A1>B1,A1*2,0)"),
                (0, "D1", "=1/0"),
            ],
            &["Sheet1"],
        );
        let t = run(
            &mut e,
            &json!({"cmd": "evaluateFormula", "sheet": 0, "row": 0, "col": 2}),
        );
        assert_eq!(t["cell"], json!("Sheet1!C1"));
        assert_eq!(t["formula"], json!("=IF(A1>B1,A1*2,0)"));
        // A1, B1, 5>3, A1 (in the branch), 5*2, IF.
        assert_eq!(t["steps"].as_array().unwrap().len(), 6);
        assert_eq!(t["result"], json!("10"));
        let none = run(
            &mut e,
            &json!({"cmd": "evaluateFormula", "sheet": 0, "row": 0, "col": 0}),
        );
        assert!(none.is_null());
        let w = run(
            &mut e,
            &json!({"cmd": "watchValues", "cells": [{"sheet": 0, "row": 0, "col": 2}]}),
        );
        assert_eq!(w[0]["cell"], json!("C1"));
        assert_eq!(w[0]["value"], json!("10"));
        assert_eq!(w[0]["formula"], json!("=IF(A1>B1,A1*2,0)"));
        // Live update: B1 = 9 makes C1 = 0.
        e.set_input(0, 0, 1, "9").unwrap();
        let w = run(
            &mut e,
            &json!({"cmd": "watchValues", "cells": [{"sheet": 0, "row": 0, "col": 2}]}),
        );
        assert_eq!(w[0]["value"], json!("0"));
        let f = run(&mut e, &json!({"cmd": "errorCheck", "sheet": 0}));
        assert_eq!(f, json!([{ "row": 0, "col": 3, "rule": "evalError" }]));
        run(
            &mut e,
            &json!({"cmd": "ignoreError", "sheet": 0, "row": 0, "col": 3, "rule": "evalError"}),
        );
        assert_eq!(
            run(&mut e, &json!({"cmd": "errorCheck", "sheet": 0})),
            json!([])
        );
        // Ignoring is one undo step.
        assert!(e.undo().unwrap());
        assert_eq!(
            run(&mut e, &json!({"cmd": "errorCheck", "sheet": 0}))
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn calculation_options_and_manual_mode() {
        let mut e = engine(&[(0, "A1", "1"), (0, "B1", "=A1+1")], &["Sheet1"]);
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["options"]["mode"], json!("auto"));
        assert_eq!(s["needsCalc"], json!(false));
        let mut opts = s["options"].clone();
        opts["mode"] = json!("manual");
        run(&mut e, &json!({"cmd": "setCalcOptions", "options": opts}));
        e.set_input(0, 0, 0, "10").unwrap();
        assert_eq!(e.cell(0, 0, 1).unwrap().display, "2");
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["needsCalc"], json!(true));
        run(&mut e, &json!({"cmd": "calculate"}));
        assert_eq!(e.cell(0, 0, 1).unwrap().display, "11");
        // The setting is saved in calcPr and read back.
        let bytes = e.save("xlsx", 0).unwrap();
        let wb = bo_xlsx::read_xlsx(&bytes).unwrap();
        assert_eq!(
            CalcSettings::read(&wb).mode,
            bo_sheet::calcopts::CalcMode::Manual
        );
        // Circular reference reported with its label.
        e.set_input(0, 0, 2, "=C1+1").unwrap();
        run(&mut e, &json!({"cmd": "calculate"}));
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["circular"][0]["label"], json!("Sheet1!C1"));
    }

    /// Shift+F9 ("calculate sheet") in manual mode calculates the active sheet only; rows
    /// inserted in manual mode are not calculated until F9 (§18.2.2 `calcMode="manual"`).
    #[test]
    fn calculate_sheet_and_structural_edits_in_manual_mode() {
        let mut e = engine(
            &[
                (0, "A1", "1"),
                (0, "B1", "=A1*2"),
                (1, "A1", "=Sheet1!A1+1"),
            ],
            &["Sheet1", "Sheet2"],
        );
        let mut opts = run(&mut e, &json!({"cmd": "calcOptions"}))["options"].clone();
        opts["mode"] = json!("manual");
        run(&mut e, &json!({"cmd": "setCalcOptions", "options": opts}));
        // Changing the options reloads the engine; one calculation brings it up to date.
        run(&mut e, &json!({"cmd": "calculate"}));
        e.set_input(0, 0, 0, "5").unwrap();
        assert_eq!(e.cell(0, 0, 1).unwrap().display, "2");
        run(&mut e, &json!({"cmd": "calculate", "sheet": 0}));
        // hand: Sheet1!B1 = 5 × 2; Sheet2!A1 waits (still 1 + 1).
        assert_eq!(e.cell(0, 0, 1).unwrap().display, "10");
        assert_eq!(e.cell(1, 0, 0).unwrap().display, "2");
        assert_eq!(e.live.last_evaluations(), 1, "only Sheet1!B1 evaluated");
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["needsCalc"], json!(true));
        run(&mut e, &json!({"cmd": "calculate"}));
        assert_eq!(e.cell(1, 0, 0).unwrap().display, "6", "hand: 5 + 1");
        // Inserting a row: the cells move with their values, nothing is calculated.
        e.set_input(0, 0, 0, "7").unwrap();
        e.structural(0, true, 0, 1, false).unwrap();
        assert_eq!(e.cell(0, 1, 0).unwrap().display, "7");
        assert_eq!(
            e.cell(0, 1, 1).unwrap().display,
            "10",
            "not recalculated yet"
        );
        assert_eq!(e.cell(1, 0, 0).unwrap().display, "6");
        assert_eq!(e.live.last_evaluations(), 0);
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["needsCalc"], json!(true));
        run(&mut e, &json!({"cmd": "calculate"}));
        assert_eq!(e.cell(0, 1, 1).unwrap().display, "14", "hand: 7 × 2");
        assert_eq!(e.cell(1, 0, 0).unwrap().display, "8", "hand: Sheet1!A2 + 1");
        assert_eq!(
            e.wb().unwrap().sheets[1]
                .cell(0, 0)
                .unwrap()
                .formula
                .as_ref()
                .map(|f| f.text.as_str()),
            Some("Sheet1!A2+1")
        );
        let s = run(&mut e, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["needsCalc"], json!(false));
    }

    #[test]
    fn calc_pr_and_ignored_errors_round_trip() {
        let mut e = engine(&[(0, "A1", "'12"), (0, "B1", "=A1")], &["Sheet1"]);
        let mut opts = run(&mut e, &json!({"cmd": "calcOptions"}))["options"].clone();
        opts["mode"] = json!("autoNoTable");
        opts["iterate"] = json!(true);
        opts["iterateCount"] = json!(50);
        opts["iterateDelta"] = json!(0.01);
        opts["fullPrecision"] = json!(false);
        let wanted: CalcSettings = serde_json::from_value(opts.clone()).unwrap();
        run(&mut e, &json!({"cmd": "setCalcOptions", "options": opts}));
        run(
            &mut e,
            &json!({"cmd": "ignoreError", "sheet": 0, "row": 0, "col": 0, "rule": "numberAsText"}),
        );
        let bytes = e.save("xlsx", 0).unwrap();
        if let Ok(path) = std::env::var("BO_AUDIT_SAVE") {
            // Hand-run check against the OpenXML validator (`bo-cli validate`).
            std::fs::write(path, &bytes).unwrap();
        }
        let wb = bo_xlsx::read_xlsx(&bytes).unwrap();
        assert_eq!(CalcSettings::read(&wb), wanted);
        assert_eq!(
            errcheck::ignored(&wb.sheets[0]),
            vec![(
                bo_model::sheet::CellRange::single(CellRef::new(0, 0)),
                vec![ErrorRule::NumberAsText]
            )]
        );
        // Reopened, the same settings drive the engine.
        let mut r = SheetEngine::new();
        r.open(&bytes, "t.xlsx").unwrap();
        let s = run(&mut r, &json!({"cmd": "calcOptions"}));
        assert_eq!(s["options"]["iterateCount"], json!(50));
        assert_eq!(
            run(&mut r, &json!({"cmd": "errorCheck", "sheet": 0})),
            json!([])
        );
    }

    #[test]
    fn manual_mode_insert_does_not_calculate() {
        let mut e = engine(
            &[(0, "A1", "1"), (0, "A2", "2"), (0, "A3", "=SUM(A1:A2)")],
            &["Sheet1"],
        );
        let mut opts = run(&mut e, &json!({"cmd": "calcOptions"}))["options"].clone();
        opts["mode"] = json!("manual");
        run(&mut e, &json!({"cmd": "setCalcOptions", "options": opts}));
        e.set_input(0, 0, 0, "10").unwrap();
        assert_eq!(e.cell(0, 2, 0).unwrap().display, "3");
        // Insert a cell at A2 (shift down): the sum moves to A4 as SUM(A1:A3) and
        // still shows 3 — nothing is calculated before F9.
        let ins = json!({ "cmd": "shiftCells", "sheet": 0,
            "range": { "r0": 1, "c0": 0, "r1": 1, "c1": 0 }, "down": true, "insert": true });
        e.exec_json(&ins.to_string()).unwrap();
        assert_eq!(e.cell(0, 3, 0).unwrap().input, "=SUM(A1:A3)");
        assert_eq!(e.cell(0, 3, 0).unwrap().display, "3");
        assert_eq!(
            run(&mut e, &json!({"cmd": "calcOptions"}))["needsCalc"],
            json!(true)
        );
        // F9: 10 + 2.
        run(&mut e, &json!({"cmd": "calculate"}));
        assert_eq!(e.cell(0, 3, 0).unwrap().display, "12");
    }

    #[test]
    fn precision_as_displayed_rounds_stored_constants() {
        // A1 = 1.236 and A2 = 2.5 shown as "0.00" and "0", A3 = 0.123456 in General,
        // A4 = A1 + A2 (+ A3).
        let mut e = engine(&[(0, "A4", "=A1+A2+A3")], &["Sheet1"]);
        {
            let wb = e.wb.as_mut().unwrap();
            for (r, v) in [(0, 1.236), (1, 2.5), (2, 0.123_456)] {
                wb.sheets[0].set_cell(r, 0, bo_model::sheet::Cell::new(CellValue::Number(v)));
            }
            let base = wb.styles.xf(0).cloned().unwrap_or_default();
            for id in [2, 1] {
                // Built-in formats 2 = "0.00", 1 = "0" (§18.8.30 numFmt).
                let mut xf = base.clone();
                xf.num_fmt_id = id;
                wb.styles.cell_xfs.push(xf);
            }
            let n = u32::try_from(wb.styles.cell_xfs.len()).unwrap();
            wb.sheets[0].cells.get_mut(&(0, 0)).unwrap().style = n - 2;
            wb.sheets[0].cells.get_mut(&(1, 0)).unwrap().style = n - 1;
            e.live.rebuild(wb);
        }
        let mut opts = run(&mut e, &json!({"cmd": "calcOptions"}))["options"].clone();
        opts["fullPrecision"] = json!(false);
        run(&mut e, &json!({"cmd": "setCalcOptions", "options": opts}));
        // Stored for good: 1.24 and 3 (half away from zero); General is left alone.
        let num = |e: &SheetEngine, r: u32| e.sheet(0).unwrap().cells[&(r, 0)].value.clone();
        assert_eq!(num(&e, 0), CellValue::Number(1.24));
        assert_eq!(num(&e, 1), CellValue::Number(3.0));
        assert_eq!(num(&e, 2), CellValue::Number(0.123_456));
        // 1.24 + 3 + 0.123456 = 4.363456.
        let CellValue::Number(sum) = num(&e, 3) else {
            panic!("A4 is not a number");
        };
        assert!((sum - 4.363_456).abs() < 1e-12, "{sum}");
        // One undo step brings the setting and the digits back.
        assert!(e.undo().unwrap());
        assert_eq!(num(&e, 0), CellValue::Number(1.236));
        assert!(CalcSettings::read(e.wb().unwrap()).full_precision);
    }

    #[test]
    fn cell_watches_are_saved_with_the_workbook() {
        let mut e = engine(&[(0, "A1", "5"), (1, "B2", "=1+1")], &["Sheet1", "Hai"]);
        assert_eq!(run(&mut e, &json!({"cmd": "cellWatches"})), json!([]));
        // A whole-workbook undo step before the watches are added.
        let group = json!({"cmd": "outlineGroup", "sheet": 0, "rows": true, "first": 1, "last": 2});
        e.exec_json(&group.to_string()).unwrap();
        let cells = json!([
            {"sheet": 1, "row": 1, "col": 1},
            {"sheet": 0, "row": 0, "col": 0},
        ]);
        let r = run(&mut e, &json!({"cmd": "setCellWatches", "cells": cells}));
        assert_eq!(r["changed"], json!(true));
        // Not an undo step: undo takes back the grouping, the watches stay (and
        // after redo too).
        assert!(e.undo().unwrap());
        assert!(e.sheet(0).unwrap().rows.is_empty());
        let w = run(&mut e, &json!({"cmd": "cellWatches"}));
        assert_eq!(w.as_array().map(Vec::len), Some(2));
        assert!(e.redo().unwrap());
        let w = run(&mut e, &json!({"cmd": "cellWatches"}));
        assert_eq!(w.as_array().map(Vec::len), Some(2));
        let r = run(&mut e, &json!({"cmd": "setCellWatches", "cells": cells}));
        assert_eq!(r["changed"], json!(false));
        let bytes = e.save("xlsx", 0).unwrap();
        if let Ok(path) = std::env::var("BO_WATCH_SAVE") {
            // Hand-run check against the OpenXML validator (`bo-cli validate`).
            std::fs::write(path, &bytes).unwrap();
        }
        let back = bo_xlsx::read_xlsx(&bytes).unwrap();
        let mut f = SheetEngine::new();
        f.load(back, "t.xlsx").unwrap();
        // Listed sheet by sheet, in file order.
        assert_eq!(
            run(&mut f, &json!({"cmd": "cellWatches"})),
            json!([
                {"sheet": 0, "row": 0, "col": 0},
                {"sheet": 1, "row": 1, "col": 1},
            ])
        );
        let xml = bo_opc::Package::read(&bytes, &bo_opc::Limits::default())
            .unwrap()
            .part("/xl/worksheets/sheet2.xml")
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap();
        assert!(
            xml.contains("<cellWatches><cellWatch r=\"B2\"/></cellWatches>"),
            "{xml}"
        );
        // Removing every watch drops the element.
        run(&mut f, &json!({"cmd": "setCellWatches", "cells": []}));
        assert_eq!(run(&mut f, &json!({"cmd": "cellWatches"})), json!([]));
        assert!(f
            .wb()
            .unwrap()
            .sheets
            .iter()
            .all(|ws| !ws.unknown.iter().any(|x| x.xml.contains("cellWatches"))));
    }
}

//! A persistent `bo-formula` engine kept in step with the model, so editing a
//! cell recalculates only its dependents (ECMA-376 Part 1, §18.17 formulas;
//! the dependency graph lives in [`bo_formula::Workbook`]).
//!
//! [`crate::formula::BoFormula`] rebuilds an engine from the model on every
//! call, which is right for opening a file or after structural edits. Interactive
//! editing uses [`LiveCalc::update`] instead: the edited cells are pushed into the
//! live engine, which recalculates the dirty cells, and only those values are
//! written back into the model.
//!
//! The workbook's calculation mode (ECMA-376 Part 1, §18.2.2 `calcPr/@calcMode`,
//! see [`crate::calcopts`]) is honoured by [`LiveCalc::update`]: in manual mode
//! an edited formula shows its own result but nothing else is recalculated until
//! [`LiveCalc::calculate`]; in "automatic except tables" mode data tables wait
//! for it. Rebuilds (opening, structural edits) always calculate.

use crate::calcopts::{CalcMode, CalcSettings};
use crate::formula::{keeps_saved_result, load, load_with, to_cell_value, to_value, write_back};
use bo_formula::{CellId, DateSystem, WebProvider};
use bo_model::sheet::{FormulaKind, Workbook};
use std::sync::Arc;

/// Live calculation state for one workbook.
#[derive(Default)]
pub struct LiveCalc {
    engine: Option<bo_formula::Workbook>,
    now: Option<f64>,
    /// Fetcher for `WEBSERVICE`; `None` (default) keeps web access disabled.
    web: Option<Arc<dyn WebProvider>>,
    /// Manual mode: edits not calculated yet.
    pending: bool,
    /// "Automatic except tables": data tables not calculated yet.
    tables_pending: bool,
    /// Results calculated but not written into the model yet (other sheets
    /// after [`LiveCalc::calculate`] for one sheet).
    unwritten: Vec<CellId>,
}

impl LiveCalc {
    /// Empty state (the first [`LiveCalc::rebuild`] loads the workbook).
    pub fn new() -> Self {
        LiveCalc::default()
    }

    /// Installs (or removes) the `WEBSERVICE` fetcher; takes effect at the next
    /// recalculation (the function is volatile).
    pub fn set_web_provider(&mut self, provider: Option<Arc<dyn WebProvider>>) {
        self.web.clone_from(&provider);
        if let Some(e) = self.engine.as_mut() {
            e.set_web_provider(provider);
        }
    }

    /// Serial date-time used by `NOW()`/`TODAY()`.
    pub fn set_now(&mut self, now: Option<f64>) {
        self.now = now;
        if let Some(e) = self.engine.as_mut() {
            e.set_now(now);
        }
    }

    /// Current clock serial.
    pub fn now(&self) -> Option<f64> {
        self.now
    }

    /// Loads the whole workbook, recalculates everything and writes every
    /// formula result back.
    pub fn rebuild(&mut self, wb: &mut Workbook) {
        let mut fw = load(wb, self.now);
        fw.set_web_provider(self.web.clone());
        fw.recalculate_all();
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
        // The previous engine holds a copy of every cell: freed off the edit path.
        crate::drop_in_background(self.engine.replace(fw));
        self.pending = false;
        self.tables_pending = false;
        self.unwritten.clear();
    }

    /// Loads a workbook just read from a file, keeping the saved results of
    /// formula cells (ECMA-376 Part 1, §18.3.1.96 `v`): only formulas without
    /// a cached value, array formulas, `ca` cells, volatile functions and their
    /// dependents are calculated, the way spreadsheet applications open a file
    /// unless `calcPr/@fullCalcOnLoad` (§18.2.2) asks for a full calculation.
    /// Opening the 1 000 000-cell benchmark workbook drops its whole
    /// recalculation this way (the dependency graph is still built so edits
    /// recalculate their dependents).
    pub fn rebuild_from_file(&mut self, wb: &mut Workbook) {
        let mut fw = load_with(wb, self.now, true);
        fw.set_web_provider(self.web.clone());
        fw.recalculate();
        let mut by_sheet: Vec<Vec<(u32, u32)>> = vec![Vec::new(); wb.sheets.len()];
        for id in fw.last_recalculated() {
            if let Some(v) = by_sheet.get_mut(id.sheet) {
                v.push((id.row, id.col));
            }
        }
        for (si, anchors) in by_sheet.iter().enumerate() {
            if !anchors.is_empty() {
                write_back(&fw, wb, si, anchors);
            }
        }
        // The previous engine holds a copy of every cell: freed off the edit path.
        crate::drop_in_background(self.engine.replace(fw));
        self.pending = false;
        self.tables_pending = false;
        self.unwritten.clear();
    }

    /// Pushes the current content of `cells` of sheet `sheet` into the live
    /// engine, recalculates dependents and writes their values back.
    /// Falls back to [`LiveCalc::rebuild`] when the engine is not in step.
    pub fn update(&mut self, wb: &mut Workbook, sheet: usize, cells: &[(u32, u32)]) {
        let Some(fw) = self.engine.as_mut() else {
            self.rebuild(wb);
            return;
        };
        let Some(ws) = wb.sheets.get(sheet) else {
            return;
        };
        let system = if wb.date1904 {
            DateSystem::Date1904
        } else {
            DateSystem::Date1900
        };
        // A new `GETPIVOTDATA` formula reads the current pivot table reports.
        let reads_pivots = cells.iter().any(|&(r, c)| {
            ws.cell(r, c)
                .and_then(|cell| cell.formula.as_ref())
                .is_some_and(|f| f.text.to_ascii_uppercase().contains("GETPIVOTDATA"))
        });
        if reads_pivots {
            fw.set_pivots(crate::pivot::formula_info::formula_pivots(wb));
        }
        for &(r, c) in cells {
            match ws.cell(r, c) {
                Some(cell) => match &cell.formula {
                    Some(f) if !f.text.is_empty() => {
                        if fw.set_formula(sheet, r, c, &f.text).is_err() {
                            fw.set_value(sheet, r, c, to_value(&cell.value, system));
                        }
                    }
                    // Shared-formula followers / data tables: rebuild from the model.
                    Some(_) => {
                        self.rebuild(wb);
                        return;
                    }
                    None => fw.set_value(sheet, r, c, to_value(&cell.value, system)),
                },
                None => fw.clear(sheet, r, c),
            }
        }
        let mode = CalcSettings::read(wb).mode;
        if mode == CalcMode::Manual {
            // Only the edited formulas show a result (against current values).
            let ws = &mut wb.sheets[sheet];
            for &(r, c) in cells {
                let Some(cell) = ws.cells.get_mut(&(r, c)) else {
                    continue;
                };
                let Some(text) = cell.formula.as_ref().map(|f| f.text.clone()) else {
                    continue;
                };
                if let Ok(v) = fw.evaluate(sheet, r, c, &text) {
                    cell.value = to_cell_value(&v);
                }
            }
            self.pending = true;
            return;
        }
        fw.recalculate();
        let ids = fw.last_recalculated().to_vec();
        write_ids(fw, wb, &ids, None);
        if mode == CalcMode::AutoNoTable {
            self.tables_pending = true;
        } else {
            crate::whatif::calculate_data_tables(fw, wb);
        }
    }

    /// After rows `start..start + perm.len()` of columns `c0..=c1` of sheet `sheet`
    /// were permuted in the model (a sort: row `start + n` received row
    /// `start + perm[n]`), moves the same cells in the live engine, sets the moved
    /// formulas again from the model and recalculates their dependents only —
    /// instead of reloading the workbook. Falls back to [`LiveCalc::rebuild`] in
    /// manual calculation, with "precision as displayed", shared / array formulas in
    /// the moved rows or spill ranges over them.
    pub fn permute_rows(
        &mut self,
        wb: &mut Workbook,
        sheet: usize,
        start: u32,
        c0: u32,
        c1: u32,
        perm: &[u32],
    ) {
        if perm.is_empty() || c1 < c0 {
            return;
        }
        let calc = CalcSettings::read(wb);
        let Some(ws) = wb.sheets.get(sheet) else {
            return;
        };
        let end = start + (perm.len() as u32 - 1);
        let mut formulas: Vec<(u32, u32)> = Vec::new();
        let mut plain = true;
        for (&(r, c), cell) in ws.cells.range((start, c0)..=(end, c1)) {
            if c < c0 || c > c1 {
                continue;
            }
            if let Some(f) = &cell.formula {
                if f.text.is_empty() || f.kind != FormulaKind::Normal {
                    plain = false;
                    break;
                }
                if perm[(r - start) as usize] != r - start {
                    formulas.push((r, c));
                }
            }
        }
        let fits = plain && calc.mode != CalcMode::Manual && calc.full_precision;
        let Some(fw) = self.engine.as_mut().filter(|_| fits) else {
            self.rebuild(wb);
            return;
        };
        if !fw.permute_rows(sheet, start, c0, c1, perm) {
            self.rebuild(wb);
            return;
        }
        let system = if wb.date1904 {
            DateSystem::Date1904
        } else {
            DateSystem::Date1900
        };
        for (r, c) in formulas {
            if let Some(cell) = ws.cell(r, c) {
                let text = cell.formula.as_ref().map_or("", |f| f.text.as_str());
                if keeps_saved_result(cell, text) || fw.set_formula(sheet, r, c, text).is_err() {
                    fw.set_value(sheet, r, c, to_value(&cell.value, system));
                }
            }
        }
        fw.recalculate();
        let ids = fw.last_recalculated().to_vec();
        write_ids(fw, wb, &ids, None);
        if calc.mode == CalcMode::AutoNoTable {
            self.tables_pending = true;
        } else {
            crate::whatif::calculate_data_tables(fw, wb);
        }
    }

    /// Pushes the hidden rows of sheet `sheet` (AutoFilter, outline) into the live
    /// engine in one step and recalculates (`SUBTOTAL` / `AGGREGATE` skip hidden
    /// rows), without reloading the cells. Falls back to [`LiveCalc::rebuild`]
    /// when no engine is loaded.
    pub fn sync_hidden_rows(&mut self, wb: &mut Workbook, sheet: usize) {
        let Some(fw) = self.engine.as_mut() else {
            self.rebuild(wb);
            return;
        };
        let Some(ws) = wb.sheets.get(sheet) else {
            return;
        };
        fw.set_filter_spans(sheet, crate::filter::filter_spans(ws));
        fw.set_hidden_rows(
            sheet,
            ws.rows.iter().filter(|(_, i)| i.hidden).map(|(r, _)| *r),
        );
        let mode = CalcSettings::read(wb).mode;
        if mode == CalcMode::Manual {
            self.pending = true;
            return;
        }
        fw.recalculate();
        let ids = fw.last_recalculated().to_vec();
        write_ids(fw, wb, &ids, None);
        if mode == CalcMode::AutoNoTable {
            self.tables_pending = true;
        } else {
            crate::whatif::calculate_data_tables(fw, wb);
        }
    }

    /// Calculates what is pending (manual mode, "automatic except tables"):
    /// cells edited since the last calculation, their dependents, volatile
    /// functions and data tables. With `sheet`, only that sheet's results are
    /// shown; the others wait for the next calculation of the workbook.
    pub fn calculate(&mut self, wb: &mut Workbook, sheet: Option<usize>) {
        let Some(fw) = self.engine.as_mut() else {
            self.rebuild(wb);
            return;
        };
        fw.recalculate();
        let mut ids = std::mem::take(&mut self.unwritten);
        ids.extend_from_slice(fw.last_recalculated());
        ids.sort_unstable();
        ids.dedup();
        self.unwritten = write_ids(fw, wb, &ids, sheet);
        crate::whatif::calculate_data_tables(fw, wb);
        self.pending = false;
        self.tables_pending = false;
    }

    /// Whether results are out of date (manual mode or data tables waiting).
    pub fn needs_calculation(&self) -> bool {
        self.pending || self.tables_pending || !self.unwritten.is_empty()
    }

    /// Cells in circular references (found by the last calculations), sorted.
    pub fn circular_references(&self) -> Vec<CellId> {
        self.engine
            .as_ref()
            .map(bo_formula::Workbook::circular_references)
            .unwrap_or_default()
    }

    /// The formula engine (dependency graph, current values), when loaded.
    pub fn engine(&self) -> Option<&bo_formula::Workbook> {
        self.engine.as_ref()
    }

    /// Number of formula evaluations of the last recalculation (diagnostics).
    pub fn last_evaluations(&self) -> usize {
        self.engine
            .as_ref()
            .map_or(0, bo_formula::Workbook::last_recalc_evaluations)
    }

    /// Drops the engine (the next update rebuilds).
    pub fn invalidate(&mut self) {
        self.engine = None;
    }
}

/// Writes the engine results of formula cells `ids` into the model (only those
/// of sheet `only`, when given); returns the cells left out.
fn write_ids(
    fw: &bo_formula::Workbook,
    wb: &mut Workbook,
    ids: &[CellId],
    only: Option<usize>,
) -> Vec<CellId> {
    let mut by_sheet: Vec<Vec<(u32, u32)>> = vec![Vec::new(); wb.sheets.len()];
    let mut left = Vec::new();
    for id in ids {
        if only.is_some_and(|s| s != id.sheet) {
            left.push(*id);
        } else if let Some(v) = by_sheet.get_mut(id.sheet) {
            v.push((id.row, id.col));
        }
    }
    for (si, anchors) in by_sheet.iter().enumerate() {
        if !anchors.is_empty() {
            write_back(fw, wb, si, anchors);
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellValue};

    #[test]
    fn opening_keeps_cached_results_and_calculates_the_rest() {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.set_cell(0, 0, Cell::new(CellValue::Number(1.0)));
        // Cached (deliberately different) result: kept as saved.
        let mut b1 = Cell::default().with_formula("A1*2");
        b1.value = CellValue::Number(99.0);
        ws.set_cell(0, 1, b1);
        // No cached value: calculated.
        ws.set_cell(0, 2, Cell::default().with_formula("A1+10"));
        // Depends on a calculated cell: recalculated with it.
        let mut d1 = Cell::default().with_formula("C1+1");
        d1.value = CellValue::Number(-1.0);
        ws.set_cell(0, 3, d1);
        let mut live = LiveCalc::new();
        live.rebuild_from_file(&mut wb);
        let v = |wb: &Workbook, c| wb.sheets[0].cell(0, c).unwrap().value.clone();
        assert_eq!(v(&wb, 1), CellValue::Number(99.0));
        assert_eq!(v(&wb, 2), CellValue::Number(11.0));
        assert_eq!(v(&wb, 3), CellValue::Number(12.0));
        // The dependency graph is complete: editing A1 recalculates B1.
        wb.sheets[0].cells.get_mut(&(0, 0)).unwrap().value = CellValue::Number(5.0);
        live.update(&mut wb, 0, &[(0, 0)]);
        assert_eq!(v(&wb, 1), CellValue::Number(10.0));
        assert_eq!(v(&wb, 3), CellValue::Number(16.0));
    }

    #[test]
    fn edits_recalculate_only_dependents() {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        for r in 0..200 {
            ws.set_cell(r, 0, Cell::new(CellValue::Number(f64::from(r))));
            ws.set_cell(r, 1, Cell::default().with_formula(&format!("A{}*2", r + 1)));
        }
        ws.set_cell(0, 2, Cell::default().with_formula("SUM(B1:B200)"));
        let mut live = LiveCalc::new();
        live.rebuild(&mut wb);
        assert_eq!(
            wb.sheets[0].cell(0, 2).unwrap().value,
            CellValue::Number(39_800.0)
        );
        wb.sheets[0].cells.get_mut(&(5, 0)).unwrap().value = CellValue::Number(105.0);
        live.update(&mut wb, 0, &[(5, 0)]);
        assert_eq!(live.last_evaluations(), 2, "B6 and C1 only");
        assert_eq!(
            wb.sheets[0].cell(5, 1).unwrap().value,
            CellValue::Number(210.0)
        );
        assert_eq!(
            wb.sheets[0].cell(0, 2).unwrap().value,
            CellValue::Number(40_000.0)
        );
        // New formula and cleared cell.
        wb.sheets[0].set_cell(0, 3, Cell::default().with_formula("C1+1"));
        wb.sheets[0].cells.remove(&(0, 0));
        live.update(&mut wb, 0, &[(0, 3), (0, 0)]);
        assert_eq!(
            wb.sheets[0].cell(0, 3).unwrap().value,
            CellValue::Number(40_001.0)
        );
        live.invalidate();
        live.update(&mut wb, 0, &[]);
        assert_eq!(
            wb.sheets[0].cell(0, 3).unwrap().value,
            CellValue::Number(40_001.0)
        );
    }

    fn num(wb: &Workbook, s: usize, r: u32, c: u32) -> CellValue {
        wb.sheets[s].cell(r, c).unwrap().value.clone()
    }

    // source: ECMA-376 Part 1, §18.17.2.3 relative references follow moved cells
    #[test]
    fn permuted_rows_recalculate_like_a_full_reload() {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        for r in 0..300u32 {
            let v = f64::from((r * 7919) % 301);
            ws.set_cell(r, 0, Cell::new(CellValue::Number(v)));
            ws.set_cell(r, 1, Cell::new(CellValue::Text(format!("t{}", r % 13))));
            if r % 3 == 0 {
                ws.set_cell(r, 2, Cell::default().with_formula(&format!("A{}*2", r + 1)));
            }
        }
        ws.set_cell(0, 5, Cell::default().with_formula("SUM(C1:C300)+A1"));
        ws.set_cell(1, 5, Cell::default().with_formula("INDEX(B1:B300,5)&A7"));
        ws.set_cell(2, 5, Cell::default().with_formula("F1+1"));
        for r in 400..600u32 {
            ws.set_cell(r, 9, Cell::default().with_formula("1+1"));
        }
        let mut live = LiveCalc::new();
        live.rebuild(&mut wb);
        // Reverse rows 1..299 of A:C, the way a sort moves them (formulas translated).
        let start = 1u32;
        let perm: Vec<u32> = (0..299u32).rev().collect();
        let ws = &mut wb.sheets[0];
        let mut rows: Vec<Vec<(u32, Cell)>> = (0..299u32)
            .map(|n| {
                (0..3u32)
                    .filter_map(|c| ws.cells.remove(&(start + n, c)).map(|cell| (c, cell)))
                    .collect()
            })
            .collect();
        for (n, &from) in perm.iter().enumerate() {
            let to = start + n as u32;
            for (c, mut cell) in std::mem::take(&mut rows[from as usize]) {
                if let Some(f) = cell.formula.as_mut() {
                    f.text = format!("A{}*2", to + 1);
                }
                ws.cells.insert((to, c), cell);
            }
        }
        live.permute_rows(&mut wb, 0, start, 0, 2, &perm);
        assert!(
            live.last_evaluations() < 200,
            "only the moved formulas and their readers: {}",
            live.last_evaluations()
        );
        let mut full = wb.clone();
        LiveCalc::new().rebuild(&mut full);
        assert_eq!(wb, full);
    }

    #[test]
    fn manual_mode_waits_for_calculate() {
        let mut wb = Workbook::new();
        wb.sheets.push(bo_model::sheet::Worksheet::new("Two"));
        wb.sheets[0].set_cell(0, 0, Cell::new(CellValue::Number(1.0)));
        wb.sheets[0].set_cell(0, 1, Cell::default().with_formula("A1*2"));
        wb.sheets[1].set_cell(0, 0, Cell::default().with_formula("Sheet1!A1+100"));
        crate::calcopts::CalcSettings {
            mode: CalcMode::Manual,
            ..CalcSettings::default()
        }
        .write(&mut wb);
        let mut live = LiveCalc::new();
        live.rebuild(&mut wb);
        assert_eq!(num(&wb, 0, 0, 1), CellValue::Number(2.0));
        assert!(!live.needs_calculation());
        // Editing A1 does not recalculate B1 or Two!A1.
        wb.sheets[0].cells.get_mut(&(0, 0)).unwrap().value = CellValue::Number(5.0);
        live.update(&mut wb, 0, &[(0, 0)]);
        assert_eq!(num(&wb, 0, 0, 1), CellValue::Number(2.0));
        assert!(live.needs_calculation());
        // A formula typed in manual mode shows its own result: A1*3 = 15.
        wb.sheets[0].set_cell(0, 2, Cell::default().with_formula("A1*3"));
        live.update(&mut wb, 0, &[(0, 2)]);
        assert_eq!(num(&wb, 0, 0, 2), CellValue::Number(15.0));
        assert_eq!(num(&wb, 0, 0, 1), CellValue::Number(2.0));
        // Calculating the first sheet only: B1 = 10, Two!A1 still 101.
        live.calculate(&mut wb, Some(0));
        assert_eq!(num(&wb, 0, 0, 1), CellValue::Number(10.0));
        assert_eq!(num(&wb, 1, 0, 0), CellValue::Number(101.0));
        assert!(live.needs_calculation());
        // Calculating the workbook: Two!A1 = 105.
        live.calculate(&mut wb, None);
        assert_eq!(num(&wb, 1, 0, 0), CellValue::Number(105.0));
        assert!(!live.needs_calculation());
    }

    #[test]
    fn calc_pr_drives_iteration_and_precision() {
        let mut wb = Workbook::new();
        wb.sheets[0].set_cell(0, 0, Cell::default().with_formula("(A1+10)/2"));
        let mut live = LiveCalc::new();
        live.rebuild(&mut wb);
        // No iteration: a circular reference is an error, reported as such.
        assert!(matches!(num(&wb, 0, 0, 0), CellValue::Error(_)));
        assert_eq!(live.circular_references(), vec![CellId::new(0, 0, 0)]);
        // iterate="1" iterateCount="3": 5, 7.5, 8.75.
        crate::calcopts::CalcSettings {
            iterate: true,
            iterate_count: 3,
            ..CalcSettings::default()
        }
        .write(&mut wb);
        wb.sheets[0].set_cell(0, 0, Cell::default().with_formula("(A1+10)/2"));
        live.rebuild(&mut wb);
        assert_eq!(num(&wb, 0, 0, 0), CellValue::Number(8.75));
        // fullPrecision="0": B1 = 1.234 shown as 0.00 is used as 1.23.
        let mut wb = Workbook::new();
        let two_dp = crate::style::patched_xf(
            &mut wb.styles,
            0,
            &crate::style::StylePatch {
                num_fmt: Some("0.00".into()),
                ..crate::style::StylePatch::default()
            },
        );
        let mut b1 = Cell::new(CellValue::Number(1.234));
        b1.style = two_dp;
        wb.sheets[0].set_cell(0, 1, b1);
        wb.sheets[0].set_cell(0, 2, Cell::default().with_formula("B1*100"));
        live.rebuild(&mut wb);
        assert_eq!(num(&wb, 0, 0, 2), CellValue::Number(123.4));
        crate::calcopts::CalcSettings {
            full_precision: false,
            ..CalcSettings::default()
        }
        .write(&mut wb);
        live.rebuild(&mut wb);
        assert_eq!(num(&wb, 0, 0, 2), CellValue::Number(123.0));
    }
}

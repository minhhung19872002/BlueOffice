//! A persistent `bo-formula` engine kept in step with the model, so editing a
//! cell recalculates only its dependents (ECMA-376 Part 1, §18.17 formulas;
//! the dependency graph lives in [`bo_formula::Workbook`]).
//!
//! [`crate::formula::BoFormula`] rebuilds an engine from the model on every
//! call, which is right for opening a file or after structural edits. Interactive
//! editing uses [`LiveCalc::update`] instead: the edited cells are pushed into the
//! live engine, which recalculates the dirty cells, and only those values are
//! written back into the model.

use crate::formula::{load, to_value, write_back};
use bo_formula::DateSystem;
use bo_model::sheet::Workbook;

/// Live calculation state for one workbook.
#[derive(Default)]
pub struct LiveCalc {
    engine: Option<bo_formula::Workbook>,
    now: Option<f64>,
}

impl LiveCalc {
    /// Empty state (the first [`LiveCalc::rebuild`] loads the workbook).
    pub fn new() -> Self {
        LiveCalc::default()
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
        self.engine = Some(fw);
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
        fw.recalculate();
        let mut by_sheet: Vec<Vec<(u32, u32)>> = vec![Vec::new(); wb.sheets.len()];
        for id in fw.last_recalculated() {
            if let Some(v) = by_sheet.get_mut(id.sheet) {
                v.push((id.row, id.col));
            }
        }
        for (si, anchors) in by_sheet.iter().enumerate() {
            if !anchors.is_empty() {
                write_back(fw, wb, si, anchors);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellValue};

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
}

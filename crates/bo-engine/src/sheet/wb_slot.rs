//! The open workbook with an index of its legacy array formulas.
//!
//! Reading the formula-bar contents of a cell must know whether the cell lies in a legacy
//! array formula (ECMA-376 Part 1, §18.17.2.4): scanning every cell of the sheet cost
//! 1.3–1.8 ms per arrow key on a large sheet. [`WbSlot`] keeps the anchors of those arrays
//! per sheet, built on first use and dropped by any mutable access to the workbook
//! (`DerefMut`), so the index can never be stale.

use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

use bo_model::sheet::{CellRange, FormulaKind, Workbook, Worksheet};

/// Legacy array formulas of one sheet: (anchor, range).
pub(crate) type SheetArrays = Vec<((u32, u32), CellRange)>;

/// The open workbook plus a lazily built legacy-array index.
pub(crate) struct WbSlot {
    wb: Workbook,
    /// Per-sheet array anchors; `None` until read after the last mutable access.
    arrays: Mutex<Option<Vec<SheetArrays>>>,
}

impl WbSlot {
    pub(crate) fn new(wb: Workbook) -> Self {
        Self {
            wb,
            arrays: Mutex::new(None),
        }
    }

    /// The legacy array formula (anchor, range) of sheet `sheet` covering `(row, col)`.
    pub(crate) fn legacy_array_at(
        &self,
        sheet: usize,
        row: u32,
        col: u32,
    ) -> Option<((u32, u32), CellRange)> {
        let here = bo_model::sheet::CellRef { row, col };
        let mut guard = self
            .arrays
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = guard.get_or_insert_with(|| self.wb.sheets.iter().map(scan).collect());
        index
            .get(sheet)?
            .iter()
            .find(|(_, r)| r.contains(here))
            .copied()
    }
}

impl WbSlot {
    /// The legacy array formulas of sheet `sheet` (index built on first use).
    pub(crate) fn arrays_of(&self, sheet: usize) -> SheetArrays {
        let mut guard = self
            .arrays
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = guard.get_or_insert_with(|| self.wb.sheets.iter().map(scan).collect());
        index.get(sheet).cloned().unwrap_or_default()
    }

    /// The index, taken out before an edit of known cells (see [`WbSlot::restore_index`]).
    pub(crate) fn take_index(&mut self) -> Option<Vec<SheetArrays>> {
        self.arrays
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// After an edit that changed only cells `cells` of sheet `sheet` (values and
    /// formulas typed or pasted, with recalculated results elsewhere): puts back the index
    /// taken before, updated for those cells, instead of scanning every cell again on the
    /// next look-up (a million-cell sheet cost ~15 ms per Enter, round 59).
    pub(crate) fn restore_index(
        &mut self,
        index: Option<Vec<SheetArrays>>,
        sheet: usize,
        cells: &[(u32, u32)],
    ) {
        let Some(mut index) = index else {
            return;
        };
        if index.len() != self.wb.sheets.len() {
            return;
        }
        let (Some(list), Some(ws)) = (index.get_mut(sheet), self.wb.sheets.get(sheet)) else {
            return;
        };
        if cells.len() > 64 {
            let set: std::collections::HashSet<(u32, u32)> = cells.iter().copied().collect();
            list.retain(|(k, _)| !set.contains(k));
        } else {
            list.retain(|(k, _)| !cells.contains(k));
        }
        for &k in cells {
            let Some(f) = ws.cells.get(&k).and_then(|c| c.formula.as_ref()) else {
                continue;
            };
            if let (false, FormulaKind::Array { range }) = (f.dynamic, &f.kind) {
                if let Some(r) = CellRange::parse(range) {
                    list.push((k, r));
                }
            }
        }
        *self
            .arrays
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(index);
    }
}

/// Every legacy (non-dynamic) array formula of `ws`.
pub(crate) fn scan(ws: &Worksheet) -> SheetArrays {
    ws.cells
        .iter()
        .filter_map(|(&k, c)| {
            let f = c.formula.as_ref()?;
            if f.dynamic {
                return None;
            }
            match &f.kind {
                FormulaKind::Array { range } => CellRange::parse(range).map(|r| (k, r)),
                _ => None,
            }
        })
        .collect()
}

impl From<Workbook> for WbSlot {
    fn from(wb: Workbook) -> Self {
        Self::new(wb)
    }
}

impl Deref for WbSlot {
    type Target = Workbook;
    fn deref(&self) -> &Workbook {
        &self.wb
    }
}

impl DerefMut for WbSlot {
    fn deref_mut(&mut self) -> &mut Workbook {
        // Any change may add, move or remove an array formula.
        *self
            .arrays
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        &mut self.wb
    }
}

impl Clone for WbSlot {
    fn clone(&self) -> Self {
        Self::new(self.wb.clone())
    }
}

impl std::fmt::Debug for WbSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.wb.fmt(f)
    }
}

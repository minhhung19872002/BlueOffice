//! Undo steps of large edits that do not copy the cells (SHEET-PERF-002).
//!
//! A sort, an AutoFilter or a pivot table over a million rows used to snapshot the
//! whole workbook (`wb.clone()`, every cell). A [`Light`] step instead keeps:
//!
//! * the workbook **without cells and pivot caches** (`shell`: styles, sheet
//!   settings, hidden rows, AutoFilter state, pivot definitions, charts …);
//! * the previous content of the **cell regions** the edit wrote (a pivot report);
//! * the **row permutation** of a sort ([`RowPerm`]) and the original text of the
//!   formulas it moved (references are translated when rows move, which is not
//!   always reversible: a reference pushed off the grid becomes `#REF!`);
//! * how the **pivot caches** changed ([`CacheUndo`]: caches added, or replaced by a
//!   refresh — moved, never cloned).
//!
//! Applying a step swaps the current state with the saved one and returns the
//! inverse step, so undo / redo stay exact. Only edits that append sheets (never
//! insert or remove them) and that change cells only through the recorded regions
//! and permutation may use it.

use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use bo_model::sheet::{Cell, CellRange, CellRef, PivotCache, Workbook};
use bo_sheet::calc::FormulaAdjuster;
use std::collections::BTreeMap;

/// A row permutation inside columns `c0..=c1`: row `start + n` received the cells
/// of row `start + perm[n]` (formulas translated by the distance moved).
pub(crate) struct RowPerm {
    pub sheet: usize,
    pub start: u32,
    pub c0: u32,
    pub c1: u32,
    pub perm: Vec<u32>,
    /// Formula text of the moved cells before the permutation, at their original
    /// positions.
    pub formulas: Vec<((u32, u32), String)>,
    /// `true`: applying performs the permutation (redo); `false`: applying moves
    /// the rows back and restores `formulas` (undo).
    pub forward: bool,
}

/// `inv[perm[n]] = n`.
pub(crate) fn inverse(perm: &[u32]) -> Vec<u32> {
    let mut inv = vec![0u32; perm.len()];
    for (n, &from) in perm.iter().enumerate() {
        inv[from as usize] = n as u32;
    }
    inv
}

/// How the pivot caches of the workbook changed.
#[derive(Default)]
pub(crate) enum CacheUndo {
    /// Unchanged.
    #[default]
    Same,
    /// Applying keeps the first `n` caches (the edit appended the others).
    Truncate(usize),
    /// Applying appends these caches.
    Append(Vec<PivotCache>),
    /// Applying swaps these caches in at their index.
    Swap(Vec<(usize, PivotCache)>),
}

/// Previous content of a rectangle of one sheet.
pub(crate) struct Region {
    pub sheet: usize,
    pub range: CellRange,
    pub cells: Vec<((u32, u32), Cell)>,
}

/// What a light edit reports besides its cell regions.
#[derive(Default)]
pub(crate) struct LightFx {
    /// Rows permuted by a sort.
    pub perm: Option<RowPerm>,
    /// Pivot caches added or replaced.
    pub caches: CacheUndo,
    /// Sheets whose hidden rows (AutoFilter) changed.
    pub hidden: Vec<usize>,
}

/// Cells by position, in key order.
pub(crate) type Cells = Vec<((u32, u32), Cell)>;

/// One undo step without a copy of the cells.
pub(crate) struct Light {
    shell: Box<Workbook>,
    regions: Vec<Region>,
    fx: LightFx,
}

/// The workbook without cells and pivot caches.
pub(crate) fn shell(wb: &mut Workbook) -> Box<Workbook> {
    let cells: Vec<BTreeMap<(u32, u32), Cell>> = wb
        .sheets
        .iter_mut()
        .map(|s| std::mem::take(&mut s.cells))
        .collect();
    let caches = std::mem::take(&mut wb.pivot_caches);
    let out = Box::new(wb.clone());
    for (s, c) in wb.sheets.iter_mut().zip(cells) {
        s.cells = c;
    }
    wb.pivot_caches = caches;
    out
}

/// Cells of `sheet` at or below-right of `at`.
pub(crate) fn quadrant(wb: &Workbook, sheet: usize, at: CellRef) -> Vec<((u32, u32), Cell)> {
    wb.sheets.get(sheet).map_or_else(Vec::new, |ws| {
        ws.cells
            .range((at.row, at.col)..)
            .filter(|((_, c), _)| *c >= at.col)
            .map(|(k, c)| (*k, c.clone()))
            .collect()
    })
}

/// The smallest rectangle holding every cell of the quadrant at `at` that the edit
/// changed, with the previous content of that rectangle (`None`: nothing changed).
pub(crate) fn changed_region(
    wb: &Workbook,
    sheet: usize,
    at: CellRef,
    before: Vec<((u32, u32), Cell)>,
) -> Option<Region> {
    let ws = wb.sheets.get(sheet)?;
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    let mut grow = |r: u32, c: u32| {
        bbox = Some(match bbox {
            None => (r, c, r, c),
            Some((r0, c0, r1, c1)) => (r0.min(r), c0.min(c), r1.max(r), c1.max(c)),
        });
    };
    let old: BTreeMap<(u32, u32), &Cell> = before.iter().map(|(k, c)| (*k, c)).collect();
    for ((r, c), cell) in ws.cells.range((at.row, at.col)..) {
        if *c < at.col {
            continue;
        }
        if old.get(&(*r, *c)).is_none_or(|o| *o != cell) {
            grow(*r, *c);
        }
    }
    for (r, c) in old.keys() {
        if !ws.cells.contains_key(&(*r, *c)) {
            grow(*r, *c);
        }
    }
    drop(old);
    let (r0, c0, r1, c1) = bbox?;
    let range = CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1));
    let cells = before
        .into_iter()
        .filter(|((r, c), _)| range.contains(CellRef::new(*r, *c)))
        .collect();
    Some(Region {
        sheet,
        range,
        cells,
    })
}

/// The region of the whole quadrant at `at` (restores it after a failed edit).
pub(crate) fn whole_quadrant(sheet: usize, at: CellRef, cells: Vec<((u32, u32), Cell)>) -> Region {
    Region {
        sheet,
        range: CellRange::new(at, CellRef::new(MAX_ROWS - 1, MAX_COLS - 1)),
        cells,
    }
}

/// Cells of `range` (taken out of `ws`).
fn take_range(cells: &mut BTreeMap<(u32, u32), Cell>, range: CellRange) -> Vec<((u32, u32), Cell)> {
    let keys: Vec<(u32, u32)> = cells
        .range((range.first.row, range.first.col)..=(range.last.row, range.last.col))
        .map(|(k, _)| *k)
        .filter(|(_, c)| *c >= range.first.col && *c <= range.last.col)
        .collect();
    keys.into_iter()
        .filter_map(|k| cells.remove(&k).map(|c| (k, c)))
        .collect()
}

/// Permute rows of `ws` as `p` says; returns the inverse step.
fn apply_perm(wb: &mut Workbook, p: RowPerm, adj: &dyn FormulaAdjuster) -> RowPerm {
    let Some(ws) = wb.sheets.get_mut(p.sheet) else {
        return p;
    };
    let range = CellRange::new(
        CellRef::new(p.start, p.c0),
        CellRef::new(p.start + (p.perm.len() as u32).saturating_sub(1), p.c1),
    );
    if p.forward {
        bo_sheet::sort::permute_rows(ws, range, p.start, &p.perm, Some(adj));
    } else {
        bo_sheet::sort::permute_rows(ws, range, p.start, &inverse(&p.perm), None);
        for ((r, c), text) in &p.formulas {
            if let Some(f) = ws.cells.get_mut(&(*r, *c)).and_then(|c| c.formula.as_mut()) {
                f.text.clone_from(text);
            }
        }
    }
    RowPerm {
        forward: !p.forward,
        ..p
    }
}

impl Light {
    pub(crate) fn new(shell: Box<Workbook>, regions: Vec<Region>, fx: LightFx) -> Light {
        Light { shell, regions, fx }
    }

    /// Restores the state the step holds; returns the inverse step.
    pub(crate) fn apply(self, wb: &mut Workbook, adj: &dyn FormulaAdjuster) -> Light {
        let Light { shell, regions, fx } = self;
        let mut cur = std::mem::replace(wb, *shell);
        // Sheets of both states keep their cells (the shell has none); sheets only in
        // one state carry their cells with it.
        for (a, b) in wb.sheets.iter_mut().zip(cur.sheets.iter_mut()) {
            std::mem::swap(&mut a.cells, &mut b.cells);
        }
        let mut caches = std::mem::take(&mut cur.pivot_caches);
        let inv_caches = match fx.caches {
            CacheUndo::Same => CacheUndo::Same,
            CacheUndo::Truncate(n) => {
                let added = caches.split_off(n.min(caches.len()));
                CacheUndo::Append(added)
            }
            CacheUndo::Append(added) => {
                let n = caches.len();
                caches.extend(added);
                CacheUndo::Truncate(n)
            }
            CacheUndo::Swap(list) => {
                let mut inv = Vec::with_capacity(list.len());
                for (i, mut c) in list.into_iter().rev() {
                    if let Some(slot) = caches.get_mut(i) {
                        std::mem::swap(slot, &mut c);
                    }
                    inv.push((i, c));
                }
                CacheUndo::Swap(inv)
            }
        };
        wb.pivot_caches = caches;
        let mut inv_regions = Vec::with_capacity(regions.len());
        for r in regions.into_iter().rev() {
            let Some(ws) = wb.sheets.get_mut(r.sheet) else {
                continue;
            };
            let now = take_range(&mut ws.cells, r.range);
            ws.cells.extend(r.cells);
            inv_regions.push(Region {
                sheet: r.sheet,
                range: r.range,
                cells: now,
            });
        }
        let perm = fx.perm.map(|p| apply_perm(wb, p, adj));
        Light {
            shell: Box::new(cur),
            regions: inv_regions,
            fx: LightFx {
                perm,
                caches: inv_caches,
                hidden: fx.hidden,
            },
        }
    }

    /// The step holds no cell copies (only a permutation and settings).
    #[cfg(test)]
    pub(crate) fn holds_no_cells(&self) -> bool {
        self.regions.is_empty() && self.shell.sheets.iter().all(|s| s.cells.is_empty())
    }
}

#[cfg(test)]
#[path = "light_undo_tests.rs"]
mod tests;

impl SheetEngine {
    /// Brings the formula engine in step after `step` was applied (`sheets_before`:
    /// sheet count before): the permuted rows, the written regions and the hidden
    /// rows only; a full reload when sheets were added or removed.
    pub(crate) fn sync_light(&mut self, step: &Light, sheets_before: usize) {
        let Some(wb) = self.wb.as_mut() else {
            return;
        };
        if wb.sheets.len() != sheets_before {
            self.live.rebuild(wb);
            return;
        }
        if let Some(p) = &step.fx.perm {
            // `step` is the inverse: the permutation just performed is its opposite.
            let done = if p.forward {
                inverse(&p.perm)
            } else {
                p.perm.clone()
            };
            self.live
                .permute_rows(wb, p.sheet, p.start, p.c0, p.c1, &done);
        }
        for r in &step.regions {
            // Cells written now (in the sheet) and before (in the step).
            let mut keys: Vec<(u32, u32)> = r.cells.iter().map(|(k, _)| *k).collect();
            if let Some(ws) = wb.sheets.get(r.sheet) {
                keys.extend(
                    ws.cells
                        .range(
                            (r.range.first.row, r.range.first.col)
                                ..=(r.range.last.row, r.range.last.col),
                        )
                        .map(|(k, _)| *k)
                        .filter(|(_, c)| *c >= r.range.first.col && *c <= r.range.last.col),
                );
            }
            keys.sort_unstable();
            keys.dedup();
            self.live.update(wb, r.sheet, &keys);
        }
        for &s in &step.fx.hidden {
            self.live.sync_hidden_rows(wb, s);
        }
    }
}

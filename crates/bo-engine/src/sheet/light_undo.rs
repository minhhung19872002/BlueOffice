//! Undo steps of large edits that do not copy the whole workbook (SHEET-PERF-002).
//!
//! A sort, an AutoFilter, a pivot table, an Advanced Filter or a Subtotal over a
//! million rows used to snapshot the whole workbook (`wb.clone()`, every cell). A
//! [`Light`] step instead keeps:
//!
//! * the workbook **without cells and pivot caches** (`shell`: styles, names, sheet
//!   settings, hidden rows, AutoFilter state, pivot definitions, charts …);
//! * the previous content of the **cell areas** the edit wrote (a pivot report,
//!   the rows a left-to-right sort or a Subtotal changed, an Advanced Filter
//!   extract);
//! * the **row permutation** of a sort ([`RowPerm`]) and the original text of the
//!   formulas it moved (references are translated when rows move, which is not
//!   always reversible: a reference pushed off the grid becomes `#REF!`);
//! * the previous **text of formulas outside those areas** the edit rewrote
//!   (references adjusted by the rows a Subtotal inserts);
//! * how the **pivot caches** changed ([`CacheUndo`]: caches added, replaced by a
//!   refresh — moved, never cloned — or the whole list, whose records are shared).
//!
//! Applying a step swaps the current state with the saved one and returns the
//! inverse step, so undo / redo stay exact. Only edits that append sheets (never
//! insert or remove them) and that change cells only through the recorded areas,
//! permutation and formula texts may use it.

use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use bo_model::sheet::{Cell, CellRange, CellRef, CellValue, PivotCache, Workbook};
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
    /// Applying swaps in this whole list (a copy taken before the edit: the
    /// records, the large part, are shared with the caches it was copied from).
    Whole(Vec<PivotCache>),
}

/// Previous content of a rectangle of one sheet.
pub(crate) struct Region {
    pub sheet: usize,
    pub range: CellRange,
    pub cells: Vec<((u32, u32), Cell)>,
}

/// What a light edit reports besides its cell areas.
#[derive(Default)]
pub(crate) struct LightFx {
    /// Rows permuted by a sort.
    pub perm: Option<RowPerm>,
    /// Pivot caches added or replaced.
    pub caches: CacheUndo,
    /// Sheets whose hidden rows (AutoFilter) changed.
    pub hidden: Vec<usize>,
    /// Formula texts outside the areas the edit rewrote: (sheet, cell, text the
    /// step restores).
    pub texts: Vec<(usize, (u32, u32), String)>,
    /// The formula engine is reloaded after applying (rows inserted or deleted).
    pub reload: bool,
}

/// Cells by position, in key order.
pub(crate) type Cells = Vec<((u32, u32), Cell)>;

/// One undo step without a copy of the whole workbook.
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

/// The rectangle from `at` to the bottom-right corner of the grid.
pub(crate) fn quadrant(at: CellRef) -> CellRange {
    CellRange::new(at, CellRef::new(MAX_ROWS - 1, MAX_COLS - 1))
}

/// Cells of `range` in `sheet`.
pub(crate) fn cells_in(wb: &Workbook, sheet: usize, range: CellRange) -> Cells {
    wb.sheets.get(sheet).map_or_else(Vec::new, |ws| {
        ws.cells
            .range((range.first.row, range.first.col)..=(range.last.row, range.last.col))
            .filter(|((_, c), _)| *c >= range.first.col && *c <= range.last.col)
            .map(|(k, c)| (*k, c.clone()))
            .collect()
    })
}

/// The smallest rectangle holding every cell of `area` that the edit changed,
/// with the previous content of that rectangle (`None`: nothing changed).
pub(crate) fn changed_region(
    wb: &Workbook,
    sheet: usize,
    area: CellRange,
    before: Cells,
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
    for ((r, c), cell) in ws
        .cells
        .range((area.first.row, area.first.col)..=(area.last.row, area.last.col))
    {
        if *c < area.first.col || *c > area.last.col {
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

/// The region of the whole `area` (restores it after a failed edit).
pub(crate) fn whole_area(sheet: usize, area: CellRange, cells: Cells) -> Region {
    Region {
        sheet,
        range: area,
        cells,
    }
}

/// Text of every formula of the workbook outside `areas` (sheet, area), for an edit
/// that may rewrite references anywhere (rows inserted or deleted).
pub(crate) fn formula_texts(
    wb: &Workbook,
    areas: &[(usize, CellRange)],
) -> Vec<(usize, (u32, u32), String)> {
    let mut out = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        let mine: Vec<CellRange> = areas
            .iter()
            .filter(|(s, _)| *s == si)
            .map(|(_, a)| *a)
            .collect();
        for (&(r, c), cell) in &ws.cells {
            let Some(f) = &cell.formula else {
                continue;
            };
            if mine.iter().any(|a| a.contains(CellRef::new(r, c))) {
                continue;
            }
            out.push((si, (r, c), f.text.clone()));
        }
    }
    out
}

/// The entries of `texts` whose formula text the edit changed.
pub(crate) fn changed_texts(
    wb: &Workbook,
    texts: Vec<(usize, (u32, u32), String)>,
) -> Vec<(usize, (u32, u32), String)> {
    texts
        .into_iter()
        .filter(|(s, k, t)| {
            wb.sheets
                .get(*s)
                .and_then(|ws| ws.cells.get(k))
                .and_then(|c| c.formula.as_ref())
                .is_none_or(|f| f.text != *t)
        })
        .collect()
}

/// Cells of `range` (taken out of `ws`).
fn take_range(cells: &mut BTreeMap<(u32, u32), Cell>, range: CellRange) -> Cells {
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

/// Same content as far as sheet protection is concerned (§18.3.1.85: locked cells
/// keep their values and formulas).
fn same_content(a: Option<&Cell>, b: Option<&Cell>, a_text: Option<&str>) -> bool {
    let empty = |c: &Cell| matches!(c.value, CellValue::Empty) && c.formula.is_none();
    match (a, b) {
        (Some(a), Some(b)) => {
            a.value == b.value
                && match (
                    a_text.or(a.formula.as_ref().map(|f| f.text.as_str())),
                    &b.formula,
                ) {
                    (Some(x), Some(f)) => x == f.text,
                    (None, None) => true,
                    _ => false,
                }
        }
        (None, None) => true,
        (Some(c), None) | (None, Some(c)) => empty(c),
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
            CacheUndo::Whole(list) => CacheUndo::Whole(std::mem::replace(&mut caches, list)),
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
        let mut texts = Vec::with_capacity(fx.texts.len());
        for (s, k, mut text) in fx.texts {
            if let Some(f) = wb
                .sheets
                .get_mut(s)
                .and_then(|ws| ws.cells.get_mut(&k))
                .and_then(|c| c.formula.as_mut())
            {
                std::mem::swap(&mut f.text, &mut text);
                texts.push((s, k, text));
            }
        }
        Light {
            shell: Box::new(cur),
            regions: inv_regions,
            fx: LightFx {
                perm,
                caches: inv_caches,
                hidden: fx.hidden,
                texts,
                reload: fx.reload,
            },
        }
    }

    /// Whether the edit this step undoes changed a locked cell of a sheet that was
    /// protected before it (`wb`: the workbook after the edit; `open`: protected
    /// ranges unlocked in this session, by sheet name). The cells compared are the
    /// recorded areas, the permuted rows and the rewritten formulas only — the
    /// edit changed nothing else.
    pub(crate) fn touches_locked(&self, wb: &Workbook, open: &[(String, String)]) -> bool {
        use super::protect;
        let before = &*self.shell;
        let guarded = |si: usize| {
            before
                .sheets
                .get(si)
                .filter(|ws| protect::sheet_info(ws).protected)
                .and_then(|old| Some((old, wb.sheets.get(si)?)))
        };
        let names_of = |old: &bo_model::sheet::Worksheet| -> Vec<String> {
            open.iter()
                .filter(|(s, _)| *s == old.name)
                .map(|(_, n)| n.clone())
                .collect()
        };
        let locked = |old: &bo_model::sheet::Worksheet,
                      names: &[String],
                      ranges: &[protect::ProtectedRange],
                      cell: Option<&Cell>,
                      r: u32,
                      c: u32| {
            protect::locked_with(before, old, cell, r, c)
                && !protect::in_open_range(ranges, names, r, c)
        };
        for reg in &self.regions {
            let Some((old, new)) = guarded(reg.sheet) else {
                continue;
            };
            let names = names_of(old);
            let ranges = protect::protected_ranges(old);
            let prev: BTreeMap<(u32, u32), &Cell> =
                reg.cells.iter().map(|(k, c)| (*k, c)).collect();
            let a = reg.range;
            let now = new
                .cells
                .range((a.first.row, a.first.col)..=(a.last.row, a.last.col))
                .filter(|((_, c), _)| *c >= a.first.col && *c <= a.last.col);
            for (&(r, c), cell) in now {
                let p = prev.get(&(r, c)).copied();
                if !same_content(p, Some(cell), None) && locked(old, &names, &ranges, p, r, c) {
                    return true;
                }
            }
            for (&(r, c), p) in &prev {
                if !new.cells.contains_key(&(r, c))
                    && !same_content(Some(p), None, None)
                    && locked(old, &names, &ranges, Some(p), r, c)
                {
                    return true;
                }
            }
        }
        for (s, (r, c), _) in &self.fx.texts {
            let Some((old, new)) = guarded(*s) else {
                continue;
            };
            let names = names_of(old);
            let ranges = protect::protected_ranges(old);
            if locked(old, &names, &ranges, new.cells.get(&(*r, *c)), *r, *c) {
                return true;
            }
        }
        if let Some(p) = &self.fx.perm {
            if let Some((old, new)) = guarded(p.sheet) {
                let names = names_of(old);
                let ranges = protect::protected_ranges(old);
                let inv = inverse(&p.perm);
                let texts: BTreeMap<(u32, u32), &str> =
                    p.formulas.iter().map(|(k, t)| (*k, t.as_str())).collect();
                for (n, &from) in p.perm.iter().enumerate() {
                    if from as usize == n {
                        continue;
                    }
                    let row = p.start + n as u32;
                    // The cell that was at `row` is now at `start + inv[n]`.
                    let was = p.start + inv[n];
                    let mut cols: Vec<u32> = new
                        .cells
                        .range((row, p.c0)..=(row, p.c1))
                        .chain(new.cells.range((was, p.c0)..=(was, p.c1)))
                        .map(|((_, c), _)| *c)
                        .collect();
                    cols.sort_unstable();
                    cols.dedup();
                    for c in cols {
                        let prev = new.cells.get(&(was, c));
                        let text = texts.get(&(row, c)).copied();
                        if !same_content(prev, new.cells.get(&(row, c)), text)
                            && locked(old, &names, &ranges, prev, row, c)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
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
    /// sheet count before): the permuted rows, the written areas, the rewritten
    /// formulas, the hidden rows, the defined names and the sheets added or removed
    /// at the end only; a reload when the step says so (rows inserted / deleted).
    pub(crate) fn sync_light(&mut self, step: &Light, sheets_before: usize) {
        let Some(wb) = self.wb.as_mut() else {
            return;
        };
        if step.fx.reload {
            self.live.reload(wb);
            return;
        }
        let now = wb.sheets.len();
        if now < sheets_before {
            self.live.sheets_removed(wb, sheets_before - now);
        }
        // `step` is the inverse: its shell holds the state before this change.
        if step.shell.defined_names != wb.defined_names {
            self.live.sync_names(wb);
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
        let kept = now.min(sheets_before);
        for r in step.regions.iter().filter(|r| r.sheet < kept) {
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
        let mut by_sheet: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
        for (s, k, _) in &step.fx.texts {
            by_sheet.entry(*s).or_default().push(*k);
        }
        for (s, keys) in by_sheet {
            self.live.update(wb, s, &keys);
        }
        for &s in step.fx.hidden.iter().filter(|s| **s < kept) {
            self.live.sync_hidden_rows(wb, s);
        }
        if now > sheets_before {
            self.live.sheets_appended(wb, sheets_before);
        }
    }
}

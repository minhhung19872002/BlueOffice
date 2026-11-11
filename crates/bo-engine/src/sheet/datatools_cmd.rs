//! Data tools commands: Text to Columns, Remove Duplicates, Consolidate, Subtotal,
//! outline groups / Auto Outline and the invalid-data list (`bo_sheet::datatools`,
//! `bo_sheet::outline`). Every change is one undo step.

use super::cmd::{current_region, At, Recalc, Rng};
use super::{parse_number, SheetEngine, MAX_COLS, MAX_ROWS};
use crate::{EngineError, Result};
use bo_model::sheet::{Cell, CellRange, CellRef, CellValue, Formula, Workbook, Worksheet};
use bo_sheet::datatools::{
    self, parse_date_order, split_text, ColumnFormat, ConsolidateFn, ConsolidateSource, SplitMode,
};
use bo_sheet::numfmt::Locale;
use bo_sheet::outline::{self, SubtotalSpec};
use serde_json::{json, Value};

/// Rows shown by the Text to Columns preview.
const PREVIEW_ROWS: u32 = 20;
/// Most cells listed by `invalidCells`.
const MAX_INVALID: usize = 10_000;

/// Index of the Consolidate source holding cell `(sheet, row, col)`.
fn source_of(sources: &[ConsolidateSource], sheet: usize, row: u32, col: u32) -> Option<usize> {
    sources
        .iter()
        .position(|x| x.sheet == sheet && x.range.contains(CellRef::new(row, col)))
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// Built-in number formats (§18.8.30): 14 short date, 49 text `@`.
const FMT_DATE: u32 = 14;
const FMT_TEXT: u32 = 49;

/// The text a cell offers to Text to Columns: its text, or what it displays.
fn cell_text(wb: &Workbook, cell: &Cell, locale: &Locale) -> Option<String> {
    match &cell.value {
        CellValue::Empty => None,
        CellValue::Text(_) | CellValue::RichText(_) => cell.value.as_text(),
        _ => Some(bo_sheet::render::format_cell(wb, cell, locale).text),
    }
}

/// Value (and number format to give the cell) of one field.
fn convert(
    field: &str,
    fmt: ColumnFormat,
    locale: &Locale,
    date1904: bool,
) -> (CellValue, Option<u32>) {
    if field.is_empty() {
        return (CellValue::Empty, None);
    }
    match fmt {
        ColumnFormat::Text => (CellValue::Text(field.to_owned()), Some(FMT_TEXT)),
        ColumnFormat::Date(order) => match parse_date_order(field, order, date1904) {
            Some(serial) => (CellValue::Number(serial), Some(FMT_DATE)),
            None => (CellValue::Text(field.to_owned()), None),
        },
        ColumnFormat::General | ColumnFormat::Skip => {
            if let Some(n) = parse_number(field, locale) {
                return (CellValue::Number(n), None);
            }
            match field.trim().to_uppercase().as_str() {
                "TRUE" | "ĐÚNG" => return (CellValue::Bool(true), None),
                "FALSE" | "SAI" => return (CellValue::Bool(false), None),
                _ => {}
            }
            if let Some(t) = bo_sheet::numfmt::parse_typed_date(field, locale, date1904) {
                return (CellValue::Number(t.serial), Some(t.num_fmt_id));
            }
            // A field that looks like a formula stays text (our choice: splitting
            // text never creates formulas).
            (CellValue::Text(field.to_owned()), None)
        }
    }
}

/// Style index giving `style` the built-in format `fmt_id` when its format is General.
fn with_format(styles: &mut bo_model::sheet::Stylesheet, style: u32, fmt_id: u32) -> u32 {
    let mut xf = styles.xf(style).cloned().unwrap_or_default();
    if xf.num_fmt_id != 0 {
        return style;
    }
    xf.num_fmt_id = fmt_id;
    xf.apply_number_format = Some(true);
    styles.add_xf(xf)
}

fn overlaps(a: CellRange, b: CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

/// Clears the value and formula of `range` (styles stay).
fn clear_values(ws: &mut Worksheet, range: CellRange) {
    for r in range.first.row..=range.last.row {
        let keys: Vec<(u32, u32)> = ws
            .cells
            .range((r, range.first.col)..=(r, range.last.col))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            let keep = ws.cells.get_mut(&k).is_some_and(|c| {
                c.value = CellValue::Empty;
                c.formula = None;
                c.style != 0
            });
            if !keep {
                ws.cells.remove(&k);
            }
        }
    }
}

/// Options of the `textToColumns` command.
pub(super) struct TextToColumns {
    pub range: Rng,
    pub split: SplitMode,
    pub formats: Vec<ColumnFormat>,
    pub dest: Option<At>,
    pub preview: bool,
}

/// Options of the `consolidate` command.
pub(super) struct Consolidate {
    pub sources: Vec<String>,
    pub function: ConsolidateFn,
    pub top_row: bool,
    pub left_col: bool,
    pub links: bool,
    pub dest: At,
}

impl SheetEngine {
    /// The list a data tool works on: the range, or the block of data around a single cell.
    fn list_range(&self, sheet: usize, range: Rng) -> Result<CellRange> {
        Ok(current_region(self.sheet(sheet)?, range.range()))
    }

    /// Text to Columns: splits the cells of a one-column range into the columns
    /// starting at `dest` (default: the range itself). With `preview` nothing changes
    /// and `{ rows: [[field]], columns }` is returned for the first rows.
    pub(super) fn text_to_columns(&mut self, sheet: usize, o: &TextToColumns) -> Result<Value> {
        let locale = self.locale;
        let range = o.range.range();
        if range.first.col != range.last.col {
            return Err(bad("textToColumns: oneColumn"));
        }
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let lines: Vec<(u32, Vec<String>)> = (range.first.row..=range.last.row)
            .filter_map(|r| {
                let t = cell_text(wb, ws.cell(r, range.first.col)?, &locale)?;
                Some((r - range.first.row, split_text(&t, &o.split)))
            })
            .collect();
        if o.preview {
            let rows: Vec<&Vec<String>> = lines
                .iter()
                .filter(|(i, _)| *i < PREVIEW_ROWS)
                .map(|(_, f)| f)
                .collect();
            let columns = lines.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
            return Ok(json!({ "rows": rows, "columns": columns }));
        }
        let dest = o.dest.unwrap_or(At {
            sheet,
            row: range.first.row,
            col: range.first.col,
        });
        if dest.sheet >= wb.sheets.len() {
            return Err(bad("no such sheet"));
        }
        let kept = |n: usize| {
            (0..n)
                .filter(|j| o.formats.get(*j).copied().unwrap_or_default() != ColumnFormat::Skip)
                .count() as u32
        };
        let width = lines.iter().map(|(_, f)| kept(f.len())).max().unwrap_or(0);
        if dest.col.saturating_add(width) > MAX_COLS
            || dest.row.saturating_add(range.rows()) > MAX_ROWS
        {
            return Err(bad("textToColumns: tooLarge"));
        }
        let date1904 = wb.date1904;
        self.edit(dest.sheet, Recalc::All, |wb, _| {
            if dest.sheet == sheet && dest.col == range.first.col {
                clear_values(&mut wb.sheets[sheet], range);
            }
            for (i, fields) in &lines {
                let row = dest.row + i;
                let mut k = 0;
                for (j, field) in fields.iter().enumerate() {
                    let fmt = o.formats.get(j).copied().unwrap_or_default();
                    if fmt == ColumnFormat::Skip {
                        continue;
                    }
                    let col = dest.col + k;
                    k += 1;
                    let (value, fmt_id) = convert(field, fmt, &locale, date1904);
                    let old = wb.sheets[dest.sheet].cell(row, col).cloned();
                    if value == CellValue::Empty && old.is_none() {
                        continue;
                    }
                    let mut cell = old.unwrap_or_default();
                    cell.formula = None;
                    cell.value = value;
                    if let Some(id) = fmt_id {
                        cell.style = with_format(&mut wb.styles, cell.style, id);
                    }
                    wb.sheets[dest.sheet].set_cell(row, col, cell);
                }
            }
            Ok(json!({ "rows": lines.len(), "columns": width }))
        })
    }

    /// Remove Duplicates over `columns` of the list; `{ removed, remaining }`.
    pub(super) fn remove_duplicates(
        &mut self,
        sheet: usize,
        range: Rng,
        columns: &[u32],
        header: bool,
    ) -> Result<Value> {
        let range = self.list_range(sheet, range)?;
        self.edit(sheet, Recalc::All, |wb, adj| {
            let r =
                datatools::remove_duplicates(&mut wb.sheets[sheet], range, columns, header, adj);
            Ok(json!(r))
        })
    }

    /// Consolidate: writes the summary table at `dest`; `{ sheet, range }`.
    pub(super) fn consolidate(&mut self, sheet: usize, o: &Consolidate) -> Result<Value> {
        let locale = self.locale;
        let wb = self.wb()?;
        let mut sources = Vec::new();
        for s in o.sources.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let (si, range) = bo_sheet::cells::resolve_ref(wb, sheet, &s.replace('$', ""))
                .ok_or_else(|| bad("consolidate: badRange"))?;
            sources.push(ConsolidateSource { sheet: si, range });
        }
        if sources.is_empty() {
            return Err(bad("consolidate: noSources"));
        }
        if o.dest.sheet >= wb.sheets.len() {
            return Err(bad("no such sheet"));
        }
        let table =
            datatools::consolidate(wb, &sources, o.function, o.top_row, o.left_col, &locale);
        let body_rows = table.cells.len() as u32;
        let body_cols = table.cells.first().map_or(0, Vec::len) as u32;
        if body_rows == 0 || body_cols == 0 {
            return Err(bad("consolidate: noData"));
        }
        let (top, left) = (u32::from(o.top_row), u32::from(o.left_col));
        // With links, each summary row comes after one detail row per source that
        // feeds it (the link formulas), grouped and collapsed.
        let details: Vec<Vec<usize>> = if o.links {
            table
                .cells
                .iter()
                .map(|row| {
                    let mut ks = std::collections::BTreeSet::new();
                    for cell in row.iter().flatten() {
                        for &(s, r, c) in &cell.sources {
                            if let Some(k) = source_of(&sources, s, r, c) {
                                ks.insert(k);
                            }
                        }
                    }
                    ks.into_iter().collect()
                })
                .collect()
        } else {
            vec![Vec::new(); table.cells.len()]
        };
        let body_rows = body_rows + details.iter().map(|d| d.len() as u32).sum::<u32>();
        let (h, w) = (body_rows + top, body_cols + left);
        if o.dest.row.saturating_add(h) > MAX_ROWS || o.dest.col.saturating_add(w) > MAX_COLS {
            return Err(bad("consolidate: tooLarge"));
        }
        let out = CellRange::new(
            CellRef::new(o.dest.row, o.dest.col),
            CellRef::new(o.dest.row + h - 1, o.dest.col + w - 1),
        );
        if sources
            .iter()
            .any(|s| s.sheet == o.dest.sheet && overlaps(s.range, out))
        {
            return Err(bad("consolidate: overlap"));
        }
        let names: Vec<String> = wb.sheets.iter().map(|s| s.name.clone()).collect();
        // Current values of the linked source cells.
        let wb_values: std::collections::HashMap<(usize, u32, u32), CellValue> = table
            .cells
            .iter()
            .flatten()
            .flatten()
            .flat_map(|c| c.sources.iter().copied())
            .filter(|_| o.links)
            .map(|(s, r, c)| {
                let v = wb.sheets[s]
                    .cell(r, c)
                    .map(|x| x.value.clone())
                    .unwrap_or_default();
                ((s, r, c), v)
            })
            .collect();
        let (dr, dc, ds) = (o.dest.row, o.dest.col, o.dest.sheet);
        self.edit(ds, Recalc::All, |wb, _| {
            let ws = &mut wb.sheets[ds];
            clear_values(ws, out);
            let put = |ws: &mut Worksheet, r: u32, c: u32, v: CellValue, f: Option<Formula>| {
                let cell = ws.cells.entry((r, c)).or_default();
                cell.value = v;
                cell.formula = f;
            };
            if let Some(labels) = &table.col_labels {
                for (j, v) in labels.iter().enumerate() {
                    put(ws, dr, dc + left + j as u32, v.clone(), None);
                }
            }
            // Row by row: the detail rows (links), then the labelled summary row.
            let mut r = dr + top;
            for (i, row) in table.cells.iter().enumerate() {
                let first = r;
                for &k in &details[i] {
                    // One detail row per source: a link to each of its cells.
                    let src = sources[k];
                    if left > 0 {
                        put(ws, r, dc, CellValue::Text(names[src.sheet].clone()), None);
                    }
                    for (j, cell) in row.iter().enumerate() {
                        let Some(cell) = cell else { continue };
                        for &(s, sr, sc) in &cell.sources {
                            if source_of(&sources, s, sr, sc) != Some(k) {
                                continue;
                            }
                            let at = CellRef::new(sr, sc);
                            let link = bo_sheet::cells::abs_ref(&names[s], CellRange::new(at, at));
                            let v = wb_values.get(&(s, sr, sc)).cloned().unwrap_or_default();
                            put(ws, r, dc + left + j as u32, v, Some(Formula::normal(&link)));
                        }
                    }
                    r += 1;
                }
                if let Some(label) = table.row_labels.as_ref().and_then(|l| l.get(i)) {
                    put(ws, r, dc, label.clone(), None);
                }
                for (j, cell) in row.iter().enumerate() {
                    let Some(cell) = cell else { continue };
                    let c = dc + left + j as u32;
                    let f = if o.links && r > first {
                        // The summary of its detail rows (§18.3.1.73 outline).
                        let span = CellRange::new(CellRef::new(first, c), CellRef::new(r - 1, c));
                        Some(Formula::normal(&format!(
                            "{}({})",
                            o.function.function_name(),
                            span
                        )))
                    } else {
                        None
                    };
                    put(ws, r, c, cell.value.clone(), f);
                }
                if o.links && r > first {
                    outline::group(ws, true, first, r - 1, false);
                    outline::set_detail(ws, true, r, false, true);
                }
                r += 1;
            }
            Ok(json!({ "sheet": ds, "range": Rng::of(out) }))
        })
    }

    /// Subtotal over the list; `{ groups, range, levels }`.
    pub(super) fn subtotal(
        &mut self,
        sheet: usize,
        range: Rng,
        mut spec: SubtotalSpec,
    ) -> Result<Value> {
        let locale = self.locale;
        spec.range = self.list_range(sheet, range)?;
        self.edit(sheet, Recalc::All, |wb, adj| {
            let r = outline::subtotal(wb, sheet, &spec, adj, &locale)
                .map_err(|e| bad(format!("subtotal: {e}")))?;
            Ok(json!({ "groups": r.groups, "range": Rng::of(r.range), "levels": r.levels }))
        })
    }

    /// Removes the subtotal rows of the list; `{ range }`.
    pub(super) fn remove_subtotals(&mut self, sheet: usize, range: Rng) -> Result<Value> {
        let range = self.list_range(sheet, range)?;
        self.edit(sheet, Recalc::All, |wb, adj| {
            let r = outline::remove_subtotals(wb, sheet, range, adj).map_err(bad)?;
            Ok(json!({ "range": Rng::of(r) }))
        })
    }

    /// Auto Outline of `range` (default: the used range); `{ rowLevels, colLevels }`.
    /// Nothing found is the error `autoOutline: none` (the outline is left as it was).
    pub(super) fn auto_outline(&mut self, sheet: usize, range: Option<Rng>) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let range = match range {
            Some(r) => current_region(ws, r.range()),
            None => ws.used_range().ok_or_else(|| bad("autoOutline: none"))?,
        };
        self.edit(sheet, Recalc::No, |wb, _| {
            let o = outline::auto_outline(&mut wb.sheets[sheet], range, true, true);
            if o.row_levels == 0 && o.col_levels == 0 {
                return Err(bad("autoOutline: none"));
            }
            Ok(json!(o))
        })
    }

    /// Cells violating their data validation: `{ cells: [[row, col]], truncated }`.
    pub(super) fn invalid_cells(&self, sheet: usize) -> Result<Value> {
        let wb = self.wb()?;
        self.sheet(sheet)?;
        let mut cells =
            datatools::invalid_cells(wb, sheet, &self.locale, self.live.now(), MAX_INVALID + 1);
        let truncated = cells.len() > MAX_INVALID;
        cells.truncate(MAX_INVALID);
        Ok(json!({ "cells": cells, "truncated": truncated }))
    }
}

#[cfg(test)]
mod tests;

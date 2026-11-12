//! Table commands of BlueSheet (SHEET-TABLE-001, ECMA-376 Part 1, §18.5): list the tables of a
//! sheet, change name / header and totals rows / style options, totals functions, resize,
//! convert to range, and the automatic behaviour after typing ([`SheetEngine::tables_after_input`]):
//! header cells rename their column, values typed next to a table expand it, a formula typed into
//! an empty column becomes a calculated column. Each change is one undo step; the expansion and
//! the calculated-column fill are steps of their own after the typing, so undo takes them back
//! first.

use super::cmd::{Recalc, Rng};
use super::{SheetEngine, UndoEntry};
use crate::{EngineError, Result};
use bo_model::sheet::Table;
use bo_sheet::table::{self as tb, TableStylePatch};
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// Automatic table behaviour after typing (on by default).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableAuto {
    /// Values typed next to a table expand it.
    pub expand: bool,
    /// A formula typed into an empty table column fills the column.
    pub calc: bool,
    /// What the last input did automatically: `expand` / `calc`, with a sequence number.
    pub last: Option<(&'static str, u64)>,
    seq: u64,
}

impl Default for TableAuto {
    fn default() -> Self {
        TableAuto {
            expand: true,
            calc: true,
            last: None,
            seq: 0,
        }
    }
}

/// Changes of one table (`tableUpdate`).
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableUpdate {
    /// New name (`table/@name`, `@displayName`).
    #[serde(default)]
    pub name: Option<String>,
    /// Show the header row.
    #[serde(default)]
    pub header_row: Option<bool>,
    /// Show the totals row.
    #[serde(default)]
    pub totals_row: Option<bool>,
    /// Label written in the first column when the totals row is first shown.
    #[serde(default)]
    pub totals_label: Option<String>,
    /// Style name and options.
    #[serde(flatten)]
    pub style: TableStylePatch,
}

fn table_json(index: usize, t: &Table) -> Value {
    let s = t.style.clone().unwrap_or_default();
    json!({
        "index": index,
        "id": t.id,
        "name": t.name,
        "range": Rng::of(t.range),
        "headerRow": tb::has_header(t),
        "totalsRow": tb::has_totals(t),
        "filterButton": t.auto_filter.is_some(),
        "style": s.name,
        "bandedRows": s.show_row_stripes,
        "bandedColumns": s.show_column_stripes,
        "firstColumn": s.show_first_column,
        "lastColumn": s.show_last_column,
        "columns": t.columns.iter().map(|c| json!({
            "name": c.name,
            "totalsFunction": c.totals_row_function.as_deref().unwrap_or("none"),
            "totalsFormula": c.totals_row_formula,
            "totalsLabel": c.totals_row_label,
            "calculated": c.calculated_formula,
        })).collect::<Vec<_>>(),
    })
}

impl SheetEngine {
    /// The tables of `sheet` and the automatic-behaviour options.
    pub(super) fn table_list(&self, sheet: usize) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        let list: Vec<Value> = ws
            .tables
            .iter()
            .enumerate()
            .map(|(i, t)| table_json(i, t))
            .collect();
        Ok(json!({
            "tables": list,
            "autoExpand": self.table_auto.expand,
            "autoCalc": self.table_auto.calc,
            "lastAuto": self.table_auto.last.map(|(k, seq)| json!({ "kind": k, "seq": seq })),
        }))
    }

    /// The table style gallery.
    pub(super) fn table_styles() -> Value {
        json!(bo_sheet::table_style::gallery())
    }

    /// Applies `u` to table `table` of `sheet` (one undo step).
    pub(super) fn table_update(
        &mut self,
        sheet: usize,
        table: usize,
        u: &TableUpdate,
    ) -> Result<Value> {
        self.table_exists(sheet, table)?;
        self.edit(sheet, Recalc::All, |wb, _| {
            if let Some(n) = &u.name {
                let old = wb.sheets[sheet].tables[table].name.clone();
                tb::rename_table(wb, sheet, table, n).map_err(bad)?;
                // A query loading into the table keeps its link.
                super::query_cmd::table_renamed(
                    wb,
                    &old,
                    &wb.sheets[sheet].tables[table].name.clone(),
                );
            }
            if let Some(h) = u.header_row {
                tb::set_header_row(wb, sheet, table, h).map_err(bad)?;
            }
            if let Some(t) = u.totals_row {
                let label = u.totals_label.as_deref().unwrap_or("Tổng");
                tb::set_totals_row(wb, sheet, table, t, label).map_err(bad)?;
            }
            tb::set_style(wb, sheet, table, &u.style).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }

    /// Sets the totals function of column `column` (one undo step).
    pub(super) fn table_total(
        &mut self,
        sheet: usize,
        table: usize,
        column: usize,
        function: &str,
        formula: Option<&str>,
    ) -> Result<Value> {
        self.table_exists(sheet, table)?;
        self.edit(sheet, Recalc::All, |wb, _| {
            tb::set_total_function(wb, sheet, table, column, function, formula).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }

    /// Resizes table `table` to `range` (one undo step).
    pub(super) fn table_resize(&mut self, sheet: usize, table: usize, range: Rng) -> Result<Value> {
        self.table_exists(sheet, table)?;
        let locale = self.locale;
        self.edit(sheet, Recalc::All, |wb, _| {
            tb::resize_table(wb, sheet, table, range.range(), &locale).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }

    /// Converts table `table` to a plain range (one undo step).
    pub(super) fn table_convert(&mut self, sheet: usize, table: usize) -> Result<Value> {
        self.table_exists(sheet, table)?;
        self.edit(sheet, Recalc::All, |wb, _| {
            tb::convert_to_range(wb, sheet, table).map_err(bad)?;
            Ok(json!({ "ok": true }))
        })
    }

    /// Sets the automatic-behaviour options; returns them.
    pub(super) fn table_options(&mut self, expand: Option<bool>, calc: Option<bool>) -> Value {
        if let Some(e) = expand {
            self.table_auto.expand = e;
        }
        if let Some(c) = calc {
            self.table_auto.calc = c;
        }
        json!({ "autoExpand": self.table_auto.expand, "autoCalc": self.table_auto.calc })
    }

    fn table_exists(&self, sheet: usize, table: usize) -> Result<()> {
        if self.sheet(sheet)?.tables.get(table).is_none() {
            return Err(bad("no such table"));
        }
        Ok(())
    }

    /// Table behaviour after the input of `cells` (whose undo entry is the last one): in-table
    /// references get the table name, header cells rename their column (in the same undo
    /// step), then the table expansion and the calculated-column fill, each one undo step.
    pub(super) fn tables_after_input(&mut self, sheet: usize, cells: &[(u32, u32)]) {
        self.table_auto.last = None;
        let locale = self.locale;
        let Some(wb) = self.wb.as_mut() else { return };
        let Some(ws) = wb.sheets.get(sheet) else {
            return;
        };
        if ws.tables.is_empty() {
            return;
        }
        tb::qualify(wb, sheet, cells);
        // Header cells: rename the columns as part of the input's undo step.
        let ws = &wb.sheets[sheet];
        let in_header = cells.iter().any(|&(r, c)| {
            tb::table_at(ws, r, c).is_some_and(|ti| {
                let t = &ws.tables[ti];
                tb::has_header(t) && r == t.range.first.row
            })
        });
        let mut renamed = false;
        if in_header {
            match self.undo.last() {
                // A whole-workbook step (paste) already holds the state before: rename in place.
                Some(UndoEntry::Workbook(_)) => {
                    if tb::sync_headers(wb, sheet, cells, &locale) {
                        self.live.rebuild(wb);
                        renamed = true;
                    }
                }
                Some(UndoEntry::Cells {
                    sheet: s,
                    cells: old,
                }) if *s == sheet => {
                    let mut before = wb.clone();
                    let bws = &mut before.sheets[sheet];
                    for (r, c, cell) in old {
                        match cell {
                            Some(cell) => {
                                bws.cells.insert((*r, *c), cell.clone());
                            }
                            None => {
                                bws.cells.remove(&(*r, *c));
                            }
                        }
                    }
                    if tb::sync_headers(wb, sheet, cells, &locale) {
                        self.undo.pop();
                        self.undo.push(UndoEntry::Workbook(Box::new(before)));
                        self.live.rebuild(wb);
                        renamed = true;
                    }
                }
                _ => {}
            }
        }
        if renamed {
            self.invalidate(Some(sheet));
        }
        let Some(wb) = self.wb.as_ref() else { return };
        if self.table_auto.expand {
            if let Some((ti, range)) = tb::expansion_for(wb, sheet, cells) {
                let done = self.edit(sheet, Recalc::All, |wb, _| {
                    tb::resize_table(wb, sheet, ti, range, &locale).map_err(bad)
                });
                if done.is_ok() {
                    self.note_auto("expand");
                }
            }
        }
        let Some(wb) = self.wb.as_ref() else { return };
        if self.table_auto.calc && cells.len() == 1 {
            if let Some((ti, k, base)) = tb::calculated_for(wb, sheet, cells[0]) {
                let done = self.edit(sheet, Recalc::All, |wb, _| {
                    tb::set_calculated(wb, sheet, ti, k, &base);
                    Ok(())
                });
                if done.is_ok() {
                    self.note_auto("calc");
                }
            }
        }
    }

    fn note_auto(&mut self, kind: &'static str) {
        self.table_auto.seq += 1;
        self.table_auto.last = Some((kind, self.table_auto.seq));
    }
}

#[cfg(test)]
#[path = "table_cmd_tests.rs"]
mod tests;

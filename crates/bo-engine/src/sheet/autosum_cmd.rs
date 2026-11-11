//! AutoSum of BlueSheet (Σ, Alt+=): where a total goes and what it adds up. The command only
//! reads the workbook; the UI applies the plan through the normal commands (`setInputs`,
//! `format`, `tableUpdate`, `tableTotal`) in one `batch`, i.e. one undo step that also
//! travels through a collaboration session.
//!
//! `autoSum { sheet, range, function? }` → one of
//!
//! * `{ mode: "edit", row, col, text, selStart, selEnd }` — one cell is selected: the formula
//!   proposed for it, to be opened in the cell editor with `selStart..selEnd` (the proposed
//!   range) selected so that the user can adjust it. The numbers directly above win over the
//!   numbers directly on the left; a gap of empty cells is bridged when no number touches the
//!   cell; with nothing to add up the text is `=SUM()` with the caret between the parentheses.
//! * `{ mode: "write", cells: [[row, col, formula]], formats: [{ row, col, numFmt }], select }`
//!   — several cells are selected: the totals are written at once. A column of numbers gets
//!   its total in the first empty cell below, a row in the first empty cell on the right, a
//!   block one total under each column; empty cells at the end of the selection (last row,
//!   last column or both, then with a grand total in the corner) receive the totals instead.
//!   A selection of empty cells only gets one total per cell, each looking up / left as a
//!   single cell does. `select` is the selection with the totals included.
//! * `{ mode: "table", table, show, columns, clear, function, select }` — the totals belong
//!   to a table: its totals row is shown (`show`) and `columns` get the function.
//! * `{ mode: "none" }` — nothing to add up.
//!
//! `functionList` → `{ names }`: the worksheet functions, for the "More functions" list.
//!
//! A range with rows hidden by a filter is added up with `SUBTOTAL` (function numbers 9, 1,
//! 2, 4, 5: ISO/IEC 29500-1 §18.17.7 SUBTOTAL), which skips the rows the filter hides.

use super::cmd::Rng;
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{col_to_letters, CellRange, CellRef, CellValue, Workbook, Worksheet};
use bo_sheet::table as tb;
use serde::Deserialize;
use serde_json::{json, Value};

/// The summary function of an AutoSum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoFn {
    /// `SUM`.
    #[default]
    Sum,
    /// `AVERAGE`.
    Average,
    /// `COUNT` (numbers only).
    Count,
    /// `MAX`.
    Max,
    /// `MIN`.
    Min,
}

impl AutoFn {
    fn name(self) -> &'static str {
        match self {
            AutoFn::Sum => "SUM",
            AutoFn::Average => "AVERAGE",
            AutoFn::Count => "COUNT",
            AutoFn::Max => "MAX",
            AutoFn::Min => "MIN",
        }
    }

    /// `SUBTOTAL` function number (§18.17.7 SUBTOTAL: 1 AVERAGE, 2 COUNT, 4 MAX, 5 MIN, 9 SUM).
    fn subtotal(self) -> u32 {
        match self {
            AutoFn::Sum => 9,
            AutoFn::Average => 1,
            AutoFn::Count => 2,
            AutoFn::Max => 4,
            AutoFn::Min => 5,
        }
    }

    /// Totals row function of a table column (§18.18.86 `ST_TotalsRowFunction`).
    fn table(self) -> &'static str {
        match self {
            AutoFn::Sum => "sum",
            AutoFn::Average => "average",
            AutoFn::Count => "countNums",
            AutoFn::Max => "max",
            AutoFn::Min => "min",
        }
    }
}

/// The AutoSum command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AutoSumCommand {
    /// The plan of an AutoSum over the selection `range`.
    AutoSum {
        /// 0-based sheet index.
        sheet: usize,
        /// The selected cells.
        range: Rng,
        /// Summary function (default `sum`).
        #[serde(default)]
        function: AutoFn,
    },
    /// The names of the worksheet functions, sorted (the "More functions" list).
    FunctionList,
}

impl AutoSumCommand {
    /// Parses a JSON command when it is the AutoSum command.
    pub fn parse(v: &Value) -> Option<Result<AutoSumCommand>> {
        if !matches!(v.get("cmd")?.as_str()?, "autoSum" | "functionList") {
            return None;
        }
        Some(
            serde_json::from_value(v.clone())
                .map_err(|e| EngineError::Sheet(format!("bad command: {e}"))),
        )
    }
}

fn a1(row: u32, col: u32) -> String {
    format!("{}{}", col_to_letters(col), row + 1)
}

fn a1_range(r: CellRange) -> String {
    if r.first == r.last {
        a1(r.first.row, r.first.col)
    } else {
        format!(
            "{}:{}",
            a1(r.first.row, r.first.col),
            a1(r.last.row, r.last.col)
        )
    }
}

fn span(r0: u32, c0: u32, r1: u32, c1: u32) -> CellRange {
    CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1))
}

struct Planner<'a> {
    wb: &'a Workbook,
    ws: &'a Worksheet,
    func: AutoFn,
}

impl Planner<'_> {
    fn number(&self, row: u32, col: u32) -> bool {
        matches!(
            self.ws.cell(row, col).map(|c| &c.value),
            Some(CellValue::Number(_))
        )
    }

    fn empty(&self, row: u32, col: u32) -> bool {
        self.ws
            .cell(row, col)
            .is_none_or(|c| matches!(c.value, CellValue::Empty) && c.formula.is_none())
    }

    fn any_number(&self, r: CellRange) -> bool {
        (r.first.row..=r.last.row).any(|row| {
            self.ws
                .cells
                .range((row, r.first.col)..=(row, r.last.col))
                .any(|(_, c)| matches!(c.value, CellValue::Number(_)))
        })
    }

    fn all_empty(&self, r: CellRange) -> bool {
        (r.first.row..=r.last.row)
            .all(|row| (r.first.col..=r.last.col).all(|col| self.empty(row, col)))
    }

    /// Rows of `r` are hidden by a filter (the sheet's or a table's).
    fn filtered(&self, r: CellRange) -> bool {
        bo_sheet::filter::filter_spans(self.ws)
            .iter()
            .any(|&(a, b)| {
                let (lo, hi) = (r.first.row.max(a), r.last.row.min(b));
                lo <= hi && self.ws.rows.range(lo..=hi).any(|(_, i)| i.hidden)
            })
    }

    /// The formula adding up `r`, and the byte span of the range inside it.
    fn formula(&self, r: CellRange) -> (String, usize, usize) {
        let refs = a1_range(r);
        let head = if r.first.col == r.last.col && self.filtered(r) {
            format!("=SUBTOTAL({},", self.func.subtotal())
        } else {
            format!("={}(", self.func.name())
        };
        let start = head.len();
        (format!("{head}{refs})"), start, start + refs.len())
    }

    /// The numbers a total in (`row`, `col`) adds up: the run directly above, else the run
    /// directly on the left, else the nearest run above / left across empty cells.
    fn guess(&self, row: u32, col: u32) -> Option<CellRange> {
        let up = |from: u32| {
            let mut top = from;
            while top > 0 && self.number(top - 1, col) {
                top -= 1;
            }
            top
        };
        let left = |from: u32| {
            let mut first = from;
            while first > 0 && self.number(row, first - 1) {
                first -= 1;
            }
            first
        };
        if row > 0 && self.number(row - 1, col) {
            return Some(span(up(row - 1), col, row - 1, col));
        }
        if col > 0 && self.number(row, col - 1) {
            return Some(span(row, left(col - 1), row, col - 1));
        }
        // Across a gap: the nearest filled cell above (then on the left) must be a number.
        let above = self
            .ws
            .cells
            .range((0, 0)..(row, 0))
            .rev()
            .find(|(&(_, c), cell)| c == col && !self.is_blank(cell))
            .map(|(&(r, _), _)| r);
        if let Some(r) = above.filter(|&r| self.number(r, col)) {
            return Some(span(up(r), col, row - 1, col));
        }
        let before = self
            .ws
            .cells
            .range((row, 0)..(row, col))
            .rev()
            .find(|(_, cell)| !self.is_blank(cell))
            .map(|(&(_, c), _)| c);
        if let Some(c) = before.filter(|&c| self.number(row, c)) {
            return Some(span(row, left(c), row, col - 1));
        }
        None
    }

    fn is_blank(&self, c: &bo_model::sheet::Cell) -> bool {
        let _ = self;
        matches!(c.value, CellValue::Empty) && c.formula.is_none()
    }

    /// Number format code of the first number of `r` when it is not General.
    fn num_fmt(&self, r: CellRange) -> Option<String> {
        if self.func == AutoFn::Count {
            return None;
        }
        for row in r.first.row..=r.last.row {
            for (_, c) in self.ws.cells.range((row, r.first.col)..=(row, r.last.col)) {
                if matches!(c.value, CellValue::Number(_)) {
                    let id = self.wb.styles.xf(c.style).map_or(0, |x| x.num_fmt_id);
                    return (id != 0).then(|| {
                        bo_sheet::numfmt::format_code(
                            &self.wb.styles,
                            id,
                            &bo_sheet::numfmt::Locale::EN_US,
                        )
                    });
                }
            }
        }
        None
    }

    /// The cell (`row`, `col`) shows General (so a total may take the format of its numbers).
    fn general(&self, row: u32, col: u32) -> bool {
        self.ws
            .cell(row, col)
            .is_none_or(|c| self.wb.styles.xf(c.style).is_none_or(|x| x.num_fmt_id == 0))
    }

    /// The table whose totals row holds, or would hold, (`row`, `col`): its index and
    /// whether the totals row has to be shown first.
    fn table_total_at(&self, row: u32, col: u32) -> Option<(usize, bool)> {
        self.ws.tables.iter().enumerate().find_map(|(i, t)| {
            if col < t.range.first.col || col > t.range.last.col {
                return None;
            }
            if tb::has_totals(t) {
                (row == t.range.last.row).then_some((i, false))
            } else {
                (row == t.range.last.row + 1 && tb::data_rows(t).is_some()).then_some((i, true))
            }
        })
    }

    /// The plan giving the totals row of table `ti` the function in `cols` (absolute columns).
    fn table_plan(&self, ti: usize, show: bool, cols: &[u32], sel: CellRange) -> Value {
        let t = &self.ws.tables[ti];
        let columns: Vec<u32> = cols.iter().map(|c| c - t.range.first.col).collect();
        // A totals row shown for the first time adds up the last column by itself.
        let fresh = t
            .columns
            .iter()
            .all(|c| c.totals_row_function.is_none() && c.totals_row_label.is_none());
        let last = t.range.last.col - t.range.first.col;
        let clear: Vec<u32> = if show && fresh && !columns.contains(&last) {
            vec![last]
        } else {
            Vec::new()
        };
        let total_row = t.range.last.row + u32::from(show);
        let select = span(
            sel.first.row.min(total_row),
            sel.first.col,
            sel.last.row.max(total_row),
            sel.last.col,
        );
        json!({
            "mode": "table", "table": ti, "show": show, "columns": columns, "clear": clear,
            "function": self.func.table(), "select": Rng::of(select),
        })
    }

    /// One selected cell.
    fn single(&self, row: u32, col: u32) -> Value {
        if let Some((ti, show)) = self.table_total_at(row, col) {
            return self.table_plan(ti, show, &[col], span(row, col, row, col));
        }
        // A total written before is opened again with its range selected.
        if let Some(f) = self.ws.cell(row, col).and_then(|c| c.formula.as_ref()) {
            let text = f.text.trim();
            let plain = format!("{}(", self.func.name());
            let sub = format!("SUBTOTAL({},", self.func.subtotal());
            let head = [plain, sub].into_iter().find(|h| {
                text.len() > h.len() && text.is_char_boundary(h.len()) && {
                    text[..h.len()].eq_ignore_ascii_case(h)
                }
            });
            if let Some(h) = head.filter(|_| text.ends_with(')')) {
                let inner = &text[h.len()..text.len() - 1];
                if !inner.contains(['(', ')']) {
                    let start = 1 + h.len();
                    return json!({
                        "mode": "edit", "row": row, "col": col, "text": format!("={text}"),
                        "selStart": start, "selEnd": start + inner.len(),
                    });
                }
            }
        }
        if let Some(r) = self.guess(row, col) {
            let (text, start, end) = self.formula(r);
            json!({
                "mode": "edit", "row": row, "col": col, "text": text,
                "selStart": start, "selEnd": end,
                "numFmt": self.num_fmt(r).filter(|_| self.general(row, col)),
            })
        } else {
            let text = format!("={}()", self.func.name());
            let at = text.len() - 1;
            json!({
                "mode": "edit", "row": row, "col": col, "text": text,
                "selStart": at, "selEnd": at,
            })
        }
    }

    /// First row at or below `from` whose cells in `cols` are all empty.
    fn free_row(&self, from: u32, cols: &[u32]) -> Option<u32> {
        (from..super::MAX_ROWS).find(|&row| cols.iter().all(|&c| self.empty(row, c)))
    }

    /// First column at or right of `from` whose cells in `rows` are all empty.
    fn free_col(&self, from: u32, rows: &[u32]) -> Option<u32> {
        (from..super::MAX_COLS).find(|&col| rows.iter().all(|&r| self.empty(r, col)))
    }

    /// Several selected cells.
    #[allow(clippy::too_many_lines)]
    fn multi(&self, r: CellRange) -> Value {
        let (r0, c0, r1, c1) = (r.first.row, r.first.col, r.last.row, r.last.col);
        let mut totals: Vec<(u32, u32, CellRange)> = Vec::new();
        if self.all_empty(r) {
            // Empty cells below / right of the data: each one looks up, then left.
            let mut table: Option<(usize, bool, Vec<u32>)> = None;
            for row in r0..=r1 {
                for col in c0..=c1 {
                    if let Some((ti, show)) = self.table_total_at(row, col) {
                        match &mut table {
                            Some((t, _, cols)) if *t == ti => cols.push(col),
                            Some(_) => {}
                            None => table = Some((ti, show, vec![col])),
                        }
                    } else if let Some(g) = self.guess(row, col) {
                        totals.push((row, col, g));
                    }
                }
            }
            if let Some((ti, show, cols)) = table {
                let t = &self.ws.tables[ti];
                let numeric: Vec<u32> = cols
                    .into_iter()
                    .filter(|&c| {
                        tb::data_rows(t).is_some_and(|(a, b)| self.any_number(span(a, c, b, c)))
                    })
                    .collect();
                if numeric.is_empty() {
                    return json!({ "mode": "none" });
                }
                return self.table_plan(ti, show, &numeric, r);
            }
        } else {
            // A selection inside the data of one table: the table's totals row.
            if let Some(ti) = tb::table_at(self.ws, r0, c0) {
                let t = &self.ws.tables[ti];
                if let Some((a, b)) = tb::data_rows(t) {
                    if r0 >= a && r1 <= b && c1 <= t.range.last.col {
                        let cols: Vec<u32> = (c0..=c1)
                            .filter(|&c| self.any_number(span(a, c, b, c)))
                            .collect();
                        if cols.is_empty() {
                            return json!({ "mode": "none" });
                        }
                        return self.table_plan(ti, !tb::has_totals(t), &cols, r);
                    }
                }
            }
            let one_col = c0 == c1;
            let one_row = r0 == r1;
            // Empty cells at the end of the selection receive the totals.
            let last_row_blank = !one_row && self.all_empty(span(r1, c0, r1, c1));
            let data_r1 = if last_row_blank { r1 - 1 } else { r1 };
            let last_col_blank = !one_col && self.all_empty(span(r0, c1, data_r1, c1));
            let data_c1 = if last_col_blank { c1 - 1 } else { c1 };
            let num_cols: Vec<u32> = (c0..=data_c1)
                .filter(|&c| self.any_number(span(r0, c, data_r1, c)))
                .collect();
            let num_rows: Vec<u32> = (r0..=data_r1)
                .filter(|&row| self.any_number(span(row, c0, row, data_c1)))
                .collect();
            let below = |cols: &[u32]| {
                if last_row_blank {
                    Some(r1)
                } else {
                    self.free_row(r1 + 1, cols)
                }
            };
            let right = |rows: &[u32]| {
                if last_col_blank {
                    Some(c1)
                } else {
                    self.free_col(c1 + 1, rows)
                }
            };
            if (one_row && !last_col_blank) || (!one_row && last_col_blank && !last_row_blank) {
                // A row of numbers, or a block with an empty last column: totals on the right.
                if let Some(col) = right(&num_rows) {
                    for &row in &num_rows {
                        totals.push((row, col, span(row, c0, row, data_c1)));
                    }
                }
            } else if one_row {
                // One row ending in an empty cell.
                if !num_rows.is_empty() {
                    totals.push((r0, c1, span(r0, c0, r0, data_c1)));
                }
            } else if let Some(row) = below(&num_cols) {
                for &col in &num_cols {
                    totals.push((row, col, span(r0, col, data_r1, col)));
                }
                if last_col_blank && !num_cols.is_empty() {
                    // Empty last row and last column: row totals and the grand total.
                    for &rr in &num_rows {
                        totals.push((rr, c1, span(rr, c0, rr, data_c1)));
                    }
                    totals.push((row, c1, span(r0, c1, data_r1, c1)));
                }
            }
        }
        if totals.is_empty() {
            return json!({ "mode": "none" });
        }
        let (mut s0, mut t0, mut s1, mut t1) = (r0, c0, r1, c1);
        let mut cells = Vec::with_capacity(totals.len());
        let mut formats = Vec::new();
        for &(row, col, src) in &totals {
            s0 = s0.min(row);
            s1 = s1.max(row);
            t0 = t0.min(col);
            t1 = t1.max(col);
            cells.push(json!([row, col, self.formula(src).0]));
            if self.general(row, col) {
                // The grand total adds up totals that are still empty: take their numbers.
                let from = if self.any_number(src) { src } else { r };
                if let Some(code) = self.num_fmt(from) {
                    formats.push(json!({ "row": row, "col": col, "numFmt": code }));
                }
            }
        }
        json!({
            "mode": "write", "cells": cells, "formats": formats,
            "select": Rng::of(span(s0, t0, s1, t1)),
        })
    }
}

impl SheetEngine {
    /// The plan of an AutoSum (nothing changes).
    pub(super) fn auto_sum_command(&self, cmd: &AutoSumCommand) -> Result<Value> {
        let (sheet, range, function) = match *cmd {
            AutoSumCommand::AutoSum {
                sheet,
                range,
                function,
            } => (sheet, range, function),
            AutoSumCommand::FunctionList => {
                let mut names = bo_formula::function_names();
                names.sort_unstable();
                names.dedup();
                return Ok(json!({ "names": names }));
            }
        };
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let p = Planner {
            wb,
            ws,
            func: function,
        };
        let r = range.range();
        // One cell, or one merged cell.
        let single = r.first == r.last || ws.merge_at(r.first).is_some_and(|m| *m == r);
        Ok(if single {
            p.single(r.first.row, r.first.col)
        } else {
            p.multi(r)
        })
    }
}

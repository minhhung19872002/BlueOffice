//! Structural edits on the workbook model: insert / delete rows and columns.
//!
//! Everything addressed by cell position moves with the cells: cell data, row
//! and column properties (§18.3.1.73 / §18.3.1.13), merges, hyperlinks, comments,
//! data validations, conditional formats, tables, the AutoFilter and manual page
//! breaks. Formula text is rewritten through a [`FormulaAdjuster`]
//! (see [`crate::calc`]); with [`crate::calc::NoFormulas`] it is left unchanged.

use crate::calc::{Dimension, FormulaAdjuster, StructuralChange};
use bo_model::sheet::{
    CellRange, CellRef, ChartAnchor, ColInfo, TableColumn, Workbook, Worksheet, MAX_COLS, MAX_ROWS,
};
use std::collections::BTreeMap;

/// Errors of structural edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The sheet index (or name) does not exist in the workbook.
    NoSuchSheet,
    /// Index or count outside the sheet limits.
    OutOfRange,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NoSuchSheet => f.write_str("no such sheet"),
            EditError::OutOfRange => f.write_str("row/column index out of range"),
        }
    }
}

impl std::error::Error for EditError {}

/// Maps indexes along one axis through an insertion or deletion.
#[derive(Debug, Clone, Copy)]
struct Shift {
    at: u32,
    count: u32,
    insert: bool,
    max: u32,
}

impl Shift {
    /// New index of `i`, or `None` if it is deleted / pushed past the limit.
    fn index(self, i: u32) -> Option<u32> {
        if i < self.at {
            return Some(i);
        }
        if self.insert {
            let n = i.checked_add(self.count)?;
            (n < self.max).then_some(n)
        } else if i < self.at + self.count {
            None
        } else {
            Some(i - self.count)
        }
    }

    /// New inclusive span `a..=b`: insertions inside a span grow it; deletions
    /// shrink it; `None` when the span disappears entirely.
    fn span(self, a: u32, b: u32) -> Option<(u32, u32)> {
        if self.insert {
            let na = self.index(a)?;
            let nb = if b >= self.at {
                b.saturating_add(self.count).min(self.max - 1)
            } else {
                b
            };
            Some((na, nb))
        } else {
            let end = self.at + self.count; // exclusive
            if a >= self.at && b < end {
                return None;
            }
            let na = if a < self.at {
                a
            } else if a < end {
                self.at
            } else {
                a - self.count
            };
            let nb = if b < self.at {
                b
            } else if b < end {
                self.at - 1
            } else {
                b - self.count
            };
            Some((na, nb))
        }
    }
}

struct Mapper {
    shift: Shift,
    dim: Dimension,
}

impl Mapper {
    fn cell(&self, c: CellRef) -> Option<CellRef> {
        Some(match self.dim {
            Dimension::Rows => CellRef::new(self.shift.index(c.row)?, c.col),
            Dimension::Cols => CellRef::new(c.row, self.shift.index(c.col)?),
        })
    }

    fn range(&self, r: CellRange) -> Option<CellRange> {
        Some(match self.dim {
            Dimension::Rows => {
                let (a, b) = self.shift.span(r.first.row, r.last.row)?;
                CellRange::new(CellRef::new(a, r.first.col), CellRef::new(b, r.last.col))
            }
            Dimension::Cols => {
                let (a, b) = self.shift.span(r.first.col, r.last.col)?;
                CellRange::new(CellRef::new(r.first.row, a), CellRef::new(r.last.row, b))
            }
        })
    }

    fn ranges(&self, v: &mut Vec<CellRange>) {
        *v = v.iter().filter_map(|r| self.range(*r)).collect();
    }
}

/// Insert `count` empty rows before zero-based row `at` of sheet `sheet`.
pub fn insert_rows(
    wb: &mut Workbook,
    sheet: usize,
    at: u32,
    count: u32,
    adj: &dyn FormulaAdjuster,
) -> Result<(), EditError> {
    structural(wb, sheet, Dimension::Rows, at, count, true, adj)
}

/// Delete `count` rows starting at zero-based row `at`.
pub fn delete_rows(
    wb: &mut Workbook,
    sheet: usize,
    at: u32,
    count: u32,
    adj: &dyn FormulaAdjuster,
) -> Result<(), EditError> {
    structural(wb, sheet, Dimension::Rows, at, count, false, adj)
}

/// Insert `count` empty columns before zero-based column `at`.
pub fn insert_cols(
    wb: &mut Workbook,
    sheet: usize,
    at: u32,
    count: u32,
    adj: &dyn FormulaAdjuster,
) -> Result<(), EditError> {
    structural(wb, sheet, Dimension::Cols, at, count, true, adj)
}

/// Delete `count` columns starting at zero-based column `at`.
pub fn delete_cols(
    wb: &mut Workbook,
    sheet: usize,
    at: u32,
    count: u32,
    adj: &dyn FormulaAdjuster,
) -> Result<(), EditError> {
    structural(wb, sheet, Dimension::Cols, at, count, false, adj)
}

fn structural(
    wb: &mut Workbook,
    sheet: usize,
    dim: Dimension,
    at: u32,
    count: u32,
    insert: bool,
    adj: &dyn FormulaAdjuster,
) -> Result<(), EditError> {
    let max = match dim {
        Dimension::Rows => MAX_ROWS,
        Dimension::Cols => MAX_COLS,
    };
    if count == 0 {
        return Ok(());
    }
    if at >= max || count > max || (!insert && at + count > max) {
        return Err(EditError::OutOfRange);
    }
    let name = wb
        .sheets
        .get(sheet)
        .ok_or(EditError::NoSuchSheet)?
        .name
        .clone();
    let m = Mapper {
        shift: Shift {
            at,
            count,
            insert,
            max,
        },
        dim,
    };
    if let Some(ws) = wb.sheets.get_mut(sheet) {
        move_sheet_content(ws, &m);
    }
    let change = StructuralChange {
        sheet: name,
        dimension: dim,
        at,
        count,
        insert,
    };
    adjust_formulas(wb, &change, adj);
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn move_sheet_content(ws: &mut Worksheet, m: &Mapper) {
    // Cells.
    let cells = std::mem::take(&mut ws.cells);
    ws.cells = cells
        .into_iter()
        .filter_map(|((r, c), cell)| {
            let n = m.cell(CellRef::new(r, c))?;
            Some(((n.row, n.col), cell))
        })
        .collect();
    // Row / column properties.
    match m.dim {
        Dimension::Rows => {
            let rows = std::mem::take(&mut ws.rows);
            ws.rows = rows
                .into_iter()
                .filter_map(|(r, info)| Some((m.shift.index(r)?, info)))
                .collect::<BTreeMap<_, _>>();
        }
        Dimension::Cols => {
            let cols = std::mem::take(&mut ws.cols);
            ws.cols = cols
                .into_iter()
                .filter_map(|c| {
                    let (min, max) = m.shift.span(c.min, c.max)?;
                    Some(ColInfo { min, max, ..c })
                })
                .collect();
        }
    }
    m.ranges(&mut ws.merges);
    ws.merges.retain(|r| r.first != r.last);
    ws.hyperlinks = std::mem::take(&mut ws.hyperlinks)
        .into_iter()
        .filter_map(|mut h| {
            h.range = m.range(h.range)?;
            Some(h)
        })
        .collect();
    ws.comments = std::mem::take(&mut ws.comments)
        .into_iter()
        .filter_map(|mut c| {
            c.cell = m.cell(c.cell)?;
            Some(c)
        })
        .collect();
    for dv in &mut ws.data_validations {
        m.ranges(&mut dv.sqref);
    }
    ws.data_validations.retain(|d| !d.sqref.is_empty());
    for cf in &mut ws.conditional_formats {
        m.ranges(&mut cf.sqref);
    }
    ws.conditional_formats.retain(|c| !c.sqref.is_empty());
    // Tables: columns inside the table are added / removed with the sheet columns.
    let tables = std::mem::take(&mut ws.tables);
    for mut t in tables {
        let old = t.range;
        let Some(new) = m.range(old) else { continue };
        if m.dim == Dimension::Cols {
            let s = m.shift;
            let mut cols = Vec::with_capacity(t.columns.len());
            let mut next_id = t.columns.iter().map(|c| c.id).max().unwrap_or(0) + 1;
            let originals = t.columns.clone();
            for (i, col) in t.columns.into_iter().enumerate() {
                let abs = old.first.col + i as u32;
                if s.insert && abs == s.at && abs > old.first.col {
                    for _ in 0..s.count {
                        cols.push(new_table_column(next_id, &originals));
                        next_id += 1;
                    }
                }
                if !s.insert && abs >= s.at && abs < s.at + s.count {
                    continue;
                }
                cols.push(col);
            }
            t.columns = cols;
        }
        t.range = new;
        ws.tables.push(t);
    }
    if let Some(af) = &mut ws.auto_filter {
        match m.range(af.range) {
            Some(new) => {
                if m.dim == Dimension::Cols {
                    let base_old = af.range.first.col;
                    let base_new = new.first.col;
                    af.columns = std::mem::take(&mut af.columns)
                        .into_iter()
                        .filter_map(|mut fc| {
                            let abs = m.shift.index(base_old + fc.col_id)?;
                            fc.col_id = abs.checked_sub(base_new)?;
                            Some(fc)
                        })
                        .collect();
                }
                af.range = new;
            }
            None => ws.auto_filter = None,
        }
    }
    // Pivot table reports (§18.10.1.49 location) move with their cells.
    for pt in &mut ws.pivots {
        if let Some(r) = m.range(pt.location) {
            pt.location = r;
        }
    }
    // Chart anchors (§20.5.2.33 twoCellAnchor: moves and sizes with the cells;
    // §20.5.2.24 oneCellAnchor: its top-left cell moves; an absolute anchor stays).
    for ch in &mut ws.charts {
        match ch.anchor {
            ChartAnchor::TwoCell => {
                let to = ch.to.unwrap_or(ch.from);
                if let Some(r) = m.range(CellRange::new(ch.from, to)) {
                    ch.from = r.first;
                    if ch.to.is_some() {
                        ch.to = Some(r.last);
                    }
                }
            }
            ChartAnchor::OneCell => {
                if let Some(c) = m.range(CellRange::new(ch.from, ch.from)) {
                    ch.from = c.first;
                }
            }
            ChartAnchor::Absolute => {}
        }
    }
    let breaks = match m.dim {
        Dimension::Rows => &mut ws.page_setup.row_breaks,
        Dimension::Cols => &mut ws.page_setup.col_breaks,
    };
    *breaks = breaks.iter().filter_map(|&b| m.shift.index(b)).collect();
}

fn new_table_column(id: u32, existing: &[TableColumn]) -> TableColumn {
    let mut n = id as usize;
    let mut name = format!("Column{n}");
    while existing.iter().any(|c| c.name.eq_ignore_ascii_case(&name)) {
        n += 1;
        name = format!("Column{n}");
    }
    TableColumn {
        id,
        name,
        ..TableColumn::default()
    }
}

fn adjust_formulas(wb: &mut Workbook, change: &StructuralChange, adj: &dyn FormulaAdjuster) {
    let apply = |text: &mut String, host: Option<&str>| {
        if let Some(n) = adj.adjust(text, host, change) {
            *text = n;
        }
    };
    let sheet_names: Vec<String> = wb.sheets.iter().map(|s| s.name.clone()).collect();
    // Pivot cache sources on the changed sheet (§18.10.1.98 worksheetSource ref).
    for c in &mut wb.pivot_caches {
        if let (Some(sheet), Some(r)) = (c.source_sheet.as_deref(), c.source_ref.as_mut()) {
            if sheet.eq_ignore_ascii_case(&change.sheet) {
                apply(r, Some(sheet));
            }
        }
    }
    for dn in &mut wb.defined_names {
        let host = dn
            .local_sheet
            .and_then(|i| sheet_names.get(i as usize))
            .map(String::as_str);
        apply(&mut dn.formula, host);
    }
    for ws in &mut wb.sheets {
        let host = ws.name.clone();
        let host = Some(host.as_str());
        for cell in ws.cells.values_mut() {
            if let Some(f) = &mut cell.formula {
                if !f.text.is_empty() {
                    apply(&mut f.text, host);
                }
            }
        }
        for s in [&mut ws.print_area, &mut ws.print_titles]
            .into_iter()
            .flatten()
        {
            apply(s, host);
        }
        for dv in &mut ws.data_validations {
            for s in [&mut dv.formula1, &mut dv.formula2].into_iter().flatten() {
                apply(s, host);
            }
        }
        // Chart series references (§21.2.2.215 f of tx / cat / val).
        for s in ws
            .charts
            .iter_mut()
            .flat_map(|c| &mut c.plots)
            .flat_map(|p| &mut p.series)
        {
            for r in [&mut s.name_ref, &mut s.cat_ref, &mut s.val_ref]
                .into_iter()
                .flatten()
            {
                apply(r, host);
            }
        }
        for cf in &mut ws.conditional_formats {
            for r in &mut cf.rules {
                for s in &mut r.formulas {
                    apply(s, host);
                }
            }
        }
        for t in &mut ws.tables {
            for c in &mut t.columns {
                for s in [&mut c.calculated_formula, &mut c.totals_row_formula]
                    .into_iter()
                    .flatten()
                {
                    apply(s, host);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::NoFormulas;
    use bo_model::sheet::{
        AutoFilter, Cell, CellValue, Comment, DataValidation, FilterColumn, Hyperlink, RowInfo,
        Table,
    };

    fn r(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    fn wb() -> Workbook {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        for row in 0..5 {
            for col in 0..4 {
                ws.set_cell(
                    row,
                    col,
                    Cell::new(CellValue::Number(f64::from(row * 10 + col))),
                );
            }
        }
        ws.rows.insert(
            2,
            RowInfo {
                height: Some(30.0),
                ..RowInfo::default()
            },
        );
        ws.cols.push(ColInfo {
            min: 1,
            max: 2,
            width: Some(20.0),
            ..ColInfo::default()
        });
        ws.merges.push(r("B2:C3"));
        ws.hyperlinks.push(Hyperlink {
            range: r("D5"),
            location: Some("A1".into()),
            ..Hyperlink::default()
        });
        ws.comments.push(Comment {
            cell: CellRef::new(3, 0),
            ..Comment::default()
        });
        ws.data_validations.push(DataValidation {
            sqref: vec![r("A1:A5"), r("C3")],
            ..DataValidation::default()
        });
        ws.tables.push(Table {
            name: "T".into(),
            range: r("A1:D5"),
            columns: (0..4)
                .map(|i| TableColumn {
                    id: i + 1,
                    name: format!("C{i}"),
                    ..TableColumn::default()
                })
                .collect(),
            ..Table::default()
        });
        ws.auto_filter = Some(AutoFilter {
            range: r("A1:D5"),
            columns: vec![FilterColumn {
                col_id: 3,
                ..FilterColumn::default()
            }],
            ..AutoFilter::default()
        });
        ws.page_setup.row_breaks = vec![3];
        wb
    }

    fn num(wb: &Workbook, a1: &str) -> Option<f64> {
        match wb.sheets[0].cell_a1(a1)?.value {
            CellValue::Number(n) => Some(n),
            _ => None,
        }
    }

    #[test]
    fn insert_rows_moves_everything_below() {
        let mut wb = wb();
        insert_rows(&mut wb, 0, 1, 2, &NoFormulas).unwrap();
        let ws = &wb.sheets[0];
        assert_eq!(num(&wb, "A1"), Some(0.0));
        assert_eq!(num(&wb, "A2"), None);
        assert_eq!(num(&wb, "A4"), Some(10.0));
        assert_eq!(num(&wb, "D7"), Some(43.0));
        assert!(ws.rows.contains_key(&4), "row 3 properties moved to row 5");
        assert_eq!(ws.merges, vec![r("B4:C5")]);
        assert_eq!(ws.hyperlinks[0].range, r("D7"));
        assert_eq!(ws.comments[0].cell, CellRef::new(5, 0));
        assert_eq!(ws.data_validations[0].sqref, vec![r("A1:A7"), r("C5")]);
        assert_eq!(
            ws.tables[0].range,
            r("A1:D7"),
            "insertion inside a table grows it"
        );
        assert_eq!(ws.auto_filter.as_ref().unwrap().range, r("A1:D7"));
        assert_eq!(ws.page_setup.row_breaks, vec![5]);
    }

    #[test]
    fn delete_rows_shrinks_and_drops() {
        let mut wb = wb();
        delete_rows(&mut wb, 0, 1, 2, &NoFormulas).unwrap();
        let ws = &wb.sheets[0];
        assert_eq!(num(&wb, "A2"), Some(30.0));
        assert_eq!(ws.cells.len(), 12);
        // B2:C3 lies entirely inside the deleted rows.
        assert_eq!(ws.merges.len(), 0);
        assert_eq!(ws.data_validations[0].sqref, vec![r("A1:A3")], "C3 deleted");
        assert_eq!(ws.comments[0].cell, CellRef::new(1, 0));
        assert_eq!(ws.tables[0].range, r("A1:D3"));
        assert!(!ws.rows.contains_key(&2));
    }

    #[test]
    fn insert_and_delete_cols_update_tables_and_filters() {
        let mut wb = wb();
        insert_cols(&mut wb, 0, 2, 1, &NoFormulas).unwrap();
        {
            let ws = &wb.sheets[0];
            assert_eq!(num(&wb, "D1"), Some(2.0));
            assert_eq!(ws.cols[0].min, 1);
            assert_eq!(ws.cols[0].max, 3, "column span B:C grows to B:D");
            let t = &ws.tables[0];
            assert_eq!(t.range, r("A1:E5"));
            assert_eq!(t.columns.len(), 5);
            assert_eq!(t.columns[2].name, "Column5");
            assert_eq!(ws.auto_filter.as_ref().unwrap().columns[0].col_id, 4);
        }
        delete_cols(&mut wb, 0, 0, 2, &NoFormulas).unwrap();
        let ws = &wb.sheets[0];
        assert_eq!(num(&wb, "B1"), Some(2.0));
        let t = &ws.tables[0];
        assert_eq!(t.range, r("A1:C5"));
        assert_eq!(
            t.columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Column5", "C2", "C3"]
        );
        assert_eq!(ws.auto_filter.as_ref().unwrap().columns[0].col_id, 2);
        assert_eq!((ws.cols[0].min, ws.cols[0].max), (0, 1));
    }

    #[test]
    fn adjuster_is_called_for_formulas() {
        struct Upper;
        impl FormulaAdjuster for Upper {
            fn adjust(&self, f: &str, host: Option<&str>, c: &StructuralChange) -> Option<String> {
                assert!(c.insert);
                Some(format!("{}@{}", f.to_uppercase(), host.unwrap_or("-")))
            }
        }
        let mut wb = wb();
        wb.sheets[0].set_cell(
            9,
            0,
            Cell::new(CellValue::Number(1.0)).with_formula("sum(a1:a5)"),
        );
        insert_rows(&mut wb, 0, 0, 1, &Upper).unwrap();
        let f = wb.sheets[0].cell(10, 0).unwrap().formula.as_ref().unwrap();
        assert_eq!(f.text, "SUM(A1:A5)@Sheet1");
        // The default keeps formulas.
        insert_rows(&mut wb, 0, 0, 1, &NoFormulas).unwrap();
        let f = wb.sheets[0].cell(11, 0).unwrap().formula.as_ref().unwrap();
        assert_eq!(f.text, "SUM(A1:A5)@Sheet1");
    }

    #[test]
    fn limits() {
        let mut wb = wb();
        assert_eq!(
            insert_rows(&mut wb, 3, 0, 1, &NoFormulas),
            Err(EditError::NoSuchSheet)
        );
        assert_eq!(
            delete_cols(&mut wb, 0, MAX_COLS - 1, 2, &NoFormulas),
            Err(EditError::OutOfRange)
        );
        // Cells pushed past the last row are dropped.
        wb.sheets[0].set_cell(MAX_ROWS - 1, 0, Cell::new(CellValue::Bool(true)));
        insert_rows(&mut wb, 0, 0, 1, &NoFormulas).unwrap();
        assert!(wb.sheets[0].cell(MAX_ROWS - 1, 0).is_none());
        assert!(insert_rows(&mut wb, 0, 0, 0, &NoFormulas).is_ok());
    }
}

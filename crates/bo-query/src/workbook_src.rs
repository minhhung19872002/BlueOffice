//! Workbook sources: a sheet (its used range, `Column1…`) or a table (§18.5.1.2, with its
//! column names) of a workbook read by the host (.xlsx, .xls, .xlsb, .ods, .csv).

use crate::def::ItemKind;
use crate::text::cell_val;
use crate::value::{QTable, Val};
use crate::QueryError;
use bo_model::sheet::{SheetKind, Workbook};

/// A sheet or table offered by the navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbookItem {
    /// Sheet or table name.
    pub name: String,
    /// What it is.
    pub kind: ItemKind,
}

/// The sheets and tables of a workbook.
pub fn items(wb: &Workbook) -> Vec<WorkbookItem> {
    let mut out = Vec::new();
    for ws in &wb.sheets {
        if !matches!(ws.kind, SheetKind::Worksheet) {
            continue;
        }
        for t in &ws.tables {
            out.push(WorkbookItem {
                name: t.name.clone(),
                kind: ItemKind::Table,
            });
        }
        out.push(WorkbookItem {
            name: ws.name.clone(),
            kind: ItemKind::Sheet,
        });
    }
    out
}

/// Read a sheet or table of `wb`.
pub fn read_item(wb: &Workbook, item: &str, kind: ItemKind) -> Result<QTable, QueryError> {
    match kind {
        ItemKind::Sheet => {
            let ws = wb
                .sheet(item)
                .ok_or_else(|| QueryError::Source(format!("no sheet {item}")))?;
            let (mut r0, mut c0, mut r1, mut c1) = (u32::MAX, u32::MAX, 0, 0);
            for (&(r, c), cell) in &ws.cells {
                if matches!(cell.value, bo_model::sheet::CellValue::Empty) {
                    continue;
                }
                r0 = r0.min(r);
                c0 = c0.min(c);
                r1 = r1.max(r);
                c1 = c1.max(c);
            }
            if r0 == u32::MAX {
                return Ok(QTable::default());
            }
            let rows = (r0..=r1)
                .map(|r| {
                    (c0..=c1)
                        .map(|c| {
                            ws.cells
                                .get(&(r, c))
                                .map_or(Val::Null, |x| cell_val(&x.value))
                        })
                        .collect()
                })
                .collect();
            Ok(QTable::from_rows(rows))
        }
        ItemKind::Table => {
            let lower = item.to_lowercase();
            for ws in &wb.sheets {
                if let Some(t) = ws.tables.iter().find(|t| t.name.to_lowercase() == lower) {
                    let first = t.range.first.row + t.header_row_count.unwrap_or(1);
                    let last = t
                        .range
                        .last
                        .row
                        .saturating_sub(t.totals_row_count.unwrap_or(0));
                    let cols: Vec<u32> = (t.range.first.col..=t.range.last.col).collect();
                    let names = cols
                        .iter()
                        .enumerate()
                        .map(|(k, &c)| {
                            t.columns.get(k).map_or_else(
                                || {
                                    ws.cells
                                        .get(&(t.range.first.row, c))
                                        .and_then(|x| x.value.as_text())
                                        .unwrap_or_default()
                                },
                                |tc| tc.name.clone(),
                            )
                        })
                        .collect();
                    let rows = if first > last {
                        Vec::new()
                    } else {
                        (first..=last)
                            .map(|r| {
                                cols.iter()
                                    .map(|&c| {
                                        ws.cells
                                            .get(&(r, c))
                                            .map_or(Val::Null, |x| cell_val(&x.value))
                                    })
                                    .collect()
                            })
                            .collect()
                    };
                    return Ok(QTable::new(names, rows));
                }
            }
            Err(QueryError::Source(format!("no table {item}")))
        }
    }
}

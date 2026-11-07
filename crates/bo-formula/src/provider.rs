//! The data source an evaluation reads from.

use crate::ast::Expr;
use crate::reference::{MAX_COLS, MAX_ROWS};
use std::sync::Arc;

use crate::value::{Area, Array, Value};

/// Geometry of a table (ListObject) used to resolve structured references
/// (ECMA-376 Part 1, §18.5 tables).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableInfo {
    /// Table name.
    pub name: String,
    /// Sheet index.
    pub sheet: usize,
    /// First row of the table including the header row (0-based).
    pub first_row: u32,
    /// Last row of the table including any totals row.
    pub last_row: u32,
    /// First column.
    pub first_col: u32,
    /// Column names, left to right.
    pub columns: Vec<String>,
    /// Whether the first row is a header row.
    pub header_row: bool,
    /// Whether the last row is a totals row.
    pub totals_row: bool,
}

impl TableInfo {
    /// First data row.
    pub fn data_first_row(&self) -> u32 {
        self.first_row + u32::from(self.header_row)
    }

    /// Last data row.
    pub fn data_last_row(&self) -> u32 {
        self.last_row.saturating_sub(u32::from(self.totals_row))
    }

    /// Absolute column index of a column name (case-insensitive).
    pub fn column_index(&self, name: &str) -> Option<u32> {
        self.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name) || c.to_lowercase() == name.to_lowercase())
            .map(|i| self.first_col + i as u32)
    }

    /// Last column index.
    pub fn last_col(&self) -> u32 {
        self.first_col + self.columns.len().max(1) as u32 - 1
    }
}

/// Read access to workbook data needed during evaluation.
///
/// Sheet indices are 0-based; rows/columns are 0-based.
pub trait CellProvider {
    /// Number of sheets.
    fn sheet_count(&self) -> usize;

    /// Name of a sheet.
    fn sheet_name(&self, sheet: usize) -> Option<&str>;

    /// Find a sheet by name (case-insensitive).
    fn sheet_index(&self, name: &str) -> Option<usize> {
        (0..self.sheet_count()).find(|&i| {
            self.sheet_name(i).is_some_and(|n| {
                n.eq_ignore_ascii_case(name) || n.to_lowercase() == name.to_lowercase()
            })
        })
    }

    /// Current (calculated) value of a cell; [`Value::Empty`] when blank.
    fn cell_value(&self, sheet: usize, row: u32, col: u32) -> Value;

    /// Formula text of a cell, if it has one.
    fn cell_formula(&self, _sheet: usize, _row: u32, _col: u32) -> Option<String> {
        None
    }

    /// Definition of a defined name visible from `sheet` (sheet scope first, then
    /// workbook scope). `name` is matched case-insensitively.
    fn defined_name(&self, _sheet: usize, _name: &str) -> Option<Expr> {
        None
    }

    /// A table by name.
    fn table(&self, _name: &str) -> Option<TableInfo> {
        None
    }

    /// The table containing a cell, for `[@Col]` references written inside a table.
    fn table_at(&self, _sheet: usize, _row: u32, _col: u32) -> Option<TableInfo> {
        None
    }

    /// Used extent `(rows, cols)` of a sheet; whole-row/column references are
    /// clipped to it when materialized.
    fn used_extent(&self, _sheet: usize) -> (u32, u32) {
        (MAX_ROWS, MAX_COLS)
    }

    /// The area spilled by the dynamic-array formula anchored at a cell, if any
    /// (target of the `A1#` spill-range operator).
    fn spill_area(&self, _sheet: usize, _row: u32, _col: u32) -> Option<Area> {
        None
    }

    /// Values of a multi-cell area if the provider has them materialized
    /// (an optimization for repeated lookups into the same table); `None`
    /// means the evaluator reads the cells one by one.
    fn area_array(&self, _area: &Area) -> Option<Arc<Array>> {
        None
    }

    /// Whether a row is hidden (for `SUBTOTAL` 101-111 and `AGGREGATE`).
    fn is_row_hidden(&self, _sheet: usize, _row: u32) -> bool {
        false
    }
}

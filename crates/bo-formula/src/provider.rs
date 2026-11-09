//! The data source an evaluation reads from.

use crate::ast::Expr;
use crate::lookupindex::ExactIndex;
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

    /// The cells `(first row, first col, last row, last col)` the specifiers `items` of a
    /// structured reference select when written in row `row` (ECMA-376 Part 1, §18.5.1 tables;
    /// `[#This Row]` / `@` selects the formula's row, no item means `[#Data]`).
    ///
    /// # Errors
    /// `#REF!` for an unknown column or a missing header / totals row, `#VALUE!` for
    /// `[#This Row]` outside the data rows.
    pub fn resolve(
        &self,
        items: &[crate::ast::StructItem],
        row: u32,
    ) -> Result<(u32, u32, u32, u32), crate::value::ErrorKind> {
        use crate::ast::StructItem;
        use crate::value::ErrorKind;
        let mut col_lo = self.first_col;
        let mut col_hi = self.last_col();
        let mut cols_given = false;
        let mut rows: Option<(u32, u32)> = None;
        let mut add_rows = |lo: u32, hi: u32| {
            rows = Some(match rows {
                Some((a, b)) => (a.min(lo), b.max(hi)),
                None => (lo, hi),
            });
        };
        let mut add_cols = |lo: u32, hi: u32| {
            if cols_given {
                col_lo = col_lo.min(lo);
                col_hi = col_hi.max(hi);
            } else {
                (col_lo, col_hi) = (lo, hi);
                cols_given = true;
            }
        };
        for item in items {
            match item {
                StructItem::All => add_rows(self.first_row, self.last_row),
                StructItem::Data => add_rows(self.data_first_row(), self.data_last_row()),
                StructItem::Headers => {
                    if !self.header_row {
                        return Err(ErrorKind::Ref);
                    }
                    add_rows(self.first_row, self.first_row);
                }
                StructItem::Totals => {
                    if !self.totals_row {
                        return Err(ErrorKind::Ref);
                    }
                    add_rows(self.last_row, self.last_row);
                }
                StructItem::ThisRow => {
                    if row < self.data_first_row() || row > self.data_last_row() {
                        return Err(ErrorKind::Value);
                    }
                    add_rows(row, row);
                }
                StructItem::Column(c) => {
                    let i = self.column_index(c).ok_or(ErrorKind::Ref)?;
                    add_cols(i, i);
                }
                StructItem::ColumnRange(a, b) => {
                    let i = self.column_index(a).ok_or(ErrorKind::Ref)?;
                    let j = self.column_index(b).ok_or(ErrorKind::Ref)?;
                    add_cols(i.min(j), i.max(j));
                }
            }
        }
        let (r1, r2) = rows.unwrap_or((self.data_first_row(), self.data_last_row()));
        if r1 > r2 {
            return Err(ErrorKind::Ref);
        }
        Ok((r1, col_lo, r2, col_hi))
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

    /// Hashed exact-match index of a one-row or one-column area whose values
    /// are final (an optimization for many exact lookups into the same table);
    /// `None` means the lookup scans the values.
    fn exact_index(&self, _area: &Area) -> Option<Arc<ExactIndex>> {
        None
    }

    /// Whether a row is hidden (for `SUBTOTAL` 101-111 and `AGGREGATE`).
    fn is_row_hidden(&self, _sheet: usize, _row: u32) -> bool {
        false
    }
}

/// Why a [`WebProvider`] could not return a response for `WEBSERVICE`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebError {
    /// The host does not allow web access (an administrator enables it).
    Disabled,
    /// The URL is not allowed by the host's policy (allow-list, private address, ...).
    Blocked,
    /// The request failed (network error, HTTP error status, timeout, ...).
    Failed(String),
}

/// Pluggable fetcher behind `WEBSERVICE` ([MS-XLSX] future function `_xlfn.WEBSERVICE`).
///
/// `bo-formula` never opens network connections itself: without a provider installed
/// (see [`crate::EvalConfig::web_provider`] / [`crate::Workbook::set_web_provider`]) the
/// function is disabled and returns `#VALUE!`. Hosts install one only when an administrator
/// has enabled web access; the provider decides timeouts, proxies and allow-lists.
///
/// The formula engine has already checked that the URL is `http`/`https` and at most
/// 2048 characters long; the provider returns the response body as text.
pub trait WebProvider: Send + Sync {
    /// Fetch `url` (HTTP GET) and return the response body.
    ///
    /// # Errors
    ///
    /// Any [`WebError`]; `WEBSERVICE` reports every failure as `#VALUE!`.
    fn fetch(&self, url: &str) -> Result<String, WebError>;
}

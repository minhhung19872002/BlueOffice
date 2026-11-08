//! Reference adjustment for structural edits (insert/delete rows and columns)
//! and for copy/paste of formulas with relative references
//! (ECMA-376 Part 1, §18.17.2.3: relative references are offsets from the
//! formula cell; absolute `$` components are fixed).

use crate::ast::Expr;
use crate::reference::{CellAddr, RefKind, Reference, SheetRef, MAX_COLS, MAX_ROWS};
use crate::value::ErrorKind;

/// A structural change to one worksheet. `at` is 0-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralChange {
    /// `count` rows inserted before row `at`.
    InsertRows {
        /// Insertion index.
        at: u32,
        /// Number of rows.
        count: u32,
    },
    /// Rows `at..at+count` deleted.
    DeleteRows {
        /// First deleted row.
        at: u32,
        /// Number of rows.
        count: u32,
    },
    /// `count` columns inserted before column `at`.
    InsertCols {
        /// Insertion index.
        at: u32,
        /// Number of columns.
        count: u32,
    },
    /// Columns `at..at+count` deleted.
    DeleteCols {
        /// First deleted column.
        at: u32,
        /// Number of columns.
        count: u32,
    },
}

/// Axis-specific view of a change: (is_row_axis, insert?, at, count).
fn axis(change: StructuralChange) -> (bool, bool, u32, u32) {
    match change {
        StructuralChange::InsertRows { at, count } => (true, true, at, count),
        StructuralChange::DeleteRows { at, count } => (true, false, at, count),
        StructuralChange::InsertCols { at, count } => (false, true, at, count),
        StructuralChange::DeleteCols { at, count } => (false, false, at, count),
    }
}

/// Adjust an interval `[lo, hi]` on one axis. Returns `None` when it is deleted entirely.
fn adjust_interval(
    lo: u32,
    hi: u32,
    insert: bool,
    at: u32,
    count: u32,
    max: u32,
) -> Option<(u32, u32)> {
    if count == 0 {
        return Some((lo, hi));
    }
    if insert {
        let shift = |v: u32| v.checked_add(count).filter(|&n| n < max);
        if lo >= at {
            Some((shift(lo)?, shift(hi)?))
        } else if hi >= at {
            // Insertion inside the range expands it.
            Some((lo, shift(hi).unwrap_or(max - 1)))
        } else {
            Some((lo, hi))
        }
    } else {
        let end = at.saturating_add(count) - 1;
        if lo >= at && hi <= end {
            return None;
        }
        let nlo = if lo < at {
            lo
        } else if lo <= end {
            at
        } else {
            lo - count
        };
        let nhi = if hi < at {
            hi
        } else if hi <= end {
            at - 1
        } else {
            hi - count
        };
        Some((nlo, nhi))
    }
}

/// Adjust the span `[lo, hi]` on the axis affected by `change`.
/// Returns `None` when the span is deleted entirely.
pub(crate) fn adjust_span(lo: u32, hi: u32, change: StructuralChange) -> Option<(u32, u32)> {
    let (rows_axis, insert, at, count) = axis(change);
    let max = if rows_axis { MAX_ROWS } else { MAX_COLS };
    adjust_interval(lo, hi, insert, at, count, max)
}

/// Whether the change inserts/deletes rows (as opposed to columns).
pub(crate) fn is_row_change(change: StructuralChange) -> bool {
    axis(change).0
}

fn adjust_kind(kind: RefKind, change: StructuralChange) -> Option<RefKind> {
    let (rows_axis, insert, at, count) = axis(change);
    let max = if rows_axis { MAX_ROWS } else { MAX_COLS };
    match kind {
        RefKind::Cell(a) => {
            let v = if rows_axis { a.row } else { a.col };
            let (n, _) = adjust_interval(v, v, insert, at, count, max)?;
            let mut out = a;
            if rows_axis {
                out.row = n;
            } else {
                out.col = n;
            }
            Some(RefKind::Cell(out))
        }
        RefKind::Range(a, b) => {
            let (mut lo, mut hi) = if rows_axis {
                (a.row.min(b.row), a.row.max(b.row))
            } else {
                (a.col.min(b.col), a.col.max(b.col))
            };
            (lo, hi) = adjust_interval(lo, hi, insert, at, count, max)?;
            let (mut x, mut y) = (a, b);
            let a_is_lo = if rows_axis {
                a.row <= b.row
            } else {
                a.col <= b.col
            };
            let (lo_ref, hi_ref) = if a_is_lo {
                (&mut x, &mut y)
            } else {
                (&mut y, &mut x)
            };
            if rows_axis {
                lo_ref.row = lo;
                hi_ref.row = hi;
            } else {
                lo_ref.col = lo;
                hi_ref.col = hi;
            }
            Some(RefKind::Range(x, y))
        }
        RefKind::Rows {
            first,
            last,
            first_abs,
            last_abs,
        } => {
            if !rows_axis {
                return Some(kind);
            }
            let (lo, hi) =
                adjust_interval(first.min(last), first.max(last), insert, at, count, max)?;
            Some(RefKind::Rows {
                first: lo,
                last: hi,
                first_abs,
                last_abs,
            })
        }
        RefKind::Cols {
            first,
            last,
            first_abs,
            last_abs,
        } => {
            if rows_axis {
                return Some(kind);
            }
            let (lo, hi) =
                adjust_interval(first.min(last), first.max(last), insert, at, count, max)?;
            Some(RefKind::Cols {
                first: lo,
                last: hi,
                first_abs,
                last_abs,
            })
        }
    }
}

/// Adjust all references in `expr` after a structural change on sheet `changed_sheet`.
///
/// `formula_sheet` is the sheet holding the formula (unqualified references point there).
/// References into deleted cells become `#REF!`; ranges shrink or grow as needed.
pub fn adjust_for_change(
    expr: &Expr,
    change: StructuralChange,
    formula_sheet: &str,
    changed_sheet: &str,
) -> Expr {
    expr.map(&mut |e| match e {
        Expr::Ref(r) => {
            let affected = match &r.sheet {
                None => formula_sheet.eq_ignore_ascii_case(changed_sheet),
                Some(s) => {
                    s.first.eq_ignore_ascii_case(changed_sheet)
                        || s.last
                            .as_deref()
                            .is_some_and(|l| l.eq_ignore_ascii_case(changed_sheet))
                }
            };
            if !affected {
                return Expr::Ref(r);
            }
            match adjust_kind(r.kind, change) {
                Some(kind) => Expr::Ref(Reference {
                    sheet: r.sheet,
                    kind,
                }),
                None => Expr::Error(ErrorKind::Ref),
            }
        }
        other => other,
    })
}

fn offset_value(v: u32, abs: bool, d: i64, max: u32) -> Option<u32> {
    if abs {
        return Some(v);
    }
    let n = i64::from(v) + d;
    (0..i64::from(max)).contains(&n).then_some(n as u32)
}

fn offset_cell(a: CellAddr, dr: i64, dc: i64) -> Option<CellAddr> {
    Some(CellAddr {
        row: offset_value(a.row, a.row_abs, dr, MAX_ROWS)?,
        col: offset_value(a.col, a.col_abs, dc, MAX_COLS)?,
        ..a
    })
}

/// Shift the relative parts of all references by (`d_row`, `d_col`), as when a
/// formula is copied from one cell to another. References pushed off the grid
/// become `#REF!`.
pub fn offset_relative(expr: &Expr, d_row: i64, d_col: i64) -> Expr {
    expr.map(&mut |e| match e {
        Expr::Ref(r) => {
            let kind = match r.kind {
                RefKind::Cell(a) => offset_cell(a, d_row, d_col).map(RefKind::Cell),
                RefKind::Range(a, b) => offset_cell(a, d_row, d_col)
                    .zip(offset_cell(b, d_row, d_col))
                    .map(|(a, b)| RefKind::Range(a, b)),
                RefKind::Rows {
                    first,
                    last,
                    first_abs,
                    last_abs,
                } => offset_value(first, first_abs, d_row, MAX_ROWS)
                    .zip(offset_value(last, last_abs, d_row, MAX_ROWS))
                    .map(|(first, last)| RefKind::Rows {
                        first,
                        last,
                        first_abs,
                        last_abs,
                    }),
                RefKind::Cols {
                    first,
                    last,
                    first_abs,
                    last_abs,
                } => offset_value(first, first_abs, d_col, MAX_COLS)
                    .zip(offset_value(last, last_abs, d_col, MAX_COLS))
                    .map(|(first, last)| RefKind::Cols {
                        first,
                        last,
                        first_abs,
                        last_abs,
                    }),
            };
            match kind {
                Some(kind) => Expr::Ref(Reference {
                    sheet: r.sheet,
                    kind,
                }),
                None => Expr::Error(ErrorKind::Ref),
            }
        }
        other => other,
    })
}

/// A block of cells moved by cut and paste: the rectangle
/// (`first_row`, `first_col`)–(`last_row`, `last_col`) of sheet `from_sheet`,
/// now at an offset of (`d_row`, `d_col`) on sheet `to_sheet`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovedBlock {
    /// Name of the sheet the block was cut from.
    pub from_sheet: String,
    /// Name of the sheet the block was pasted to.
    pub to_sheet: String,
    /// First row of the source block (0-based).
    pub first_row: u32,
    /// First column of the source block (0-based).
    pub first_col: u32,
    /// Last row of the source block (0-based, inclusive).
    pub last_row: u32,
    /// Last column of the source block (0-based, inclusive).
    pub last_col: u32,
    /// Row offset from the source to the destination.
    pub d_row: i64,
    /// Column offset from the source to the destination.
    pub d_col: i64,
}

impl MovedBlock {
    fn contains(&self, a: CellAddr) -> bool {
        (self.first_row..=self.last_row).contains(&a.row)
            && (self.first_col..=self.last_col).contains(&a.col)
    }

    fn shift(&self, a: CellAddr) -> Option<CellAddr> {
        let r = i64::from(a.row) + self.d_row;
        let c = i64::from(a.col) + self.d_col;
        ((0..i64::from(MAX_ROWS)).contains(&r) && (0..i64::from(MAX_COLS)).contains(&c)).then_some(
            CellAddr {
                row: r as u32,
                col: c as u32,
                ..a
            },
        )
    }
}

/// Re-point references to cells moved by cut and paste (the moved cells keep
/// their identity): a cell reference inside the block, or a range whose both
/// corners are inside it, follows the block — absolute or relative alike —
/// and gets the destination sheet name when the block changed sheets.
/// Other references (and 3-D references) are unchanged.
pub fn adjust_for_move(expr: &Expr, formula_sheet: &str, block: &MovedBlock) -> Expr {
    let target_sheet = |r: &Reference| -> Option<SheetRef> {
        if block.to_sheet.eq_ignore_ascii_case(&block.from_sheet) {
            r.sheet.clone()
        } else if block.to_sheet.eq_ignore_ascii_case(formula_sheet) {
            None
        } else {
            Some(SheetRef::single(block.to_sheet.clone()))
        }
    };
    expr.map(&mut |e| match e {
        Expr::Ref(r) => {
            let sheet = match &r.sheet {
                None => formula_sheet,
                Some(s) if s.last.is_none() => s.first.as_str(),
                Some(_) => return Expr::Ref(r),
            };
            if !sheet.eq_ignore_ascii_case(&block.from_sheet) {
                return Expr::Ref(r);
            }
            let kind = match r.kind {
                RefKind::Cell(a) if block.contains(a) => block.shift(a).map(RefKind::Cell),
                RefKind::Range(a, b) if block.contains(a) && block.contains(b) => block
                    .shift(a)
                    .zip(block.shift(b))
                    .map(|(a, b)| RefKind::Range(a, b)),
                _ => return Expr::Ref(r),
            };
            match kind {
                Some(kind) => Expr::Ref(Reference {
                    sheet: target_sheet(&r),
                    kind,
                }),
                None => Expr::Error(ErrorKind::Ref),
            }
        }
        other => other,
    })
}

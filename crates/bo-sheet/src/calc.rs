//! Thin boundary to the formula engine (`bo-formula`, developed separately).
//!
//! `bo-sheet` never evaluates formulas: cells keep their formula text and the
//! cached value read from the file ([`bo_model::sheet::Cell::value`]). Editing
//! operations that move cells call a [`FormulaAdjuster`] so references can be
//! rewritten, and callers that need fresh values call a [`FormulaEngine`]. The
//! defaults ([`NoFormulas`]) keep formula text and cached values unchanged.

use bo_model::sheet::Workbook;

/// Which axis a structural change applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Row insert/delete (cell references change row numbers).
    Rows,
    /// Column insert/delete (cell references change column letters).
    Cols,
}

/// Rows or columns inserted / deleted on one sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralChange {
    /// Name of the sheet whose rows/columns change.
    pub sheet: String,
    /// Whether rows or columns are inserted / deleted.
    pub dimension: Dimension,
    /// First zero-based index affected.
    pub at: u32,
    /// Number of rows or columns inserted / deleted.
    pub count: u32,
    /// `true` = insertion, `false` = deletion.
    pub insert: bool,
}

/// Cells moved by cut and paste: rows `first_row..=last_row` × columns
/// `first_col..=last_col` of sheet `from_sheet`, now `d_rows` / `d_cols` away on `to_sheet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellMove {
    /// Sheet the cells were cut from.
    pub from_sheet: String,
    /// Sheet the cells were pasted on (may equal `from_sheet`).
    pub to_sheet: String,
    /// First zero-based row of the cut block.
    pub first_row: u32,
    /// First zero-based column of the cut block.
    pub first_col: u32,
    /// Last zero-based row of the cut block (inclusive).
    pub last_row: u32,
    /// Last zero-based column of the cut block (inclusive).
    pub last_col: u32,
    /// Row offset from the cut block to its pasted position.
    pub d_rows: i64,
    /// Column offset from the cut block to its pasted position.
    pub d_cols: i64,
}

/// Rewrites formula references after a structural change.
pub trait FormulaAdjuster {
    /// Return the new text of `formula` (without `=`) located on sheet
    /// `host_sheet` (`None` for workbook-scope names), or `None` to keep it.
    fn adjust(
        &self,
        formula: &str,
        host_sheet: Option<&str>,
        change: &StructuralChange,
    ) -> Option<String>;

    /// [`FormulaAdjuster::adjust`] for cells inserted / deleted in a band only (insert or
    /// delete cells, shifting the others): `band` is the first / last column (for a row
    /// change) or row (for a column change) that moves; references reaching outside it
    /// are kept. `None` keeps the text.
    fn adjust_band(
        &self,
        formula: &str,
        host_sheet: Option<&str>,
        change: &StructuralChange,
        band: (u32, u32),
    ) -> Option<String> {
        let _ = (formula, host_sheet, change, band);
        None
    }

    /// Text of `formula` copied `d_rows` / `d_cols` cells away (relative references
    /// move, absolute ones stay), or `None` to copy it unchanged. Used by AutoFill.
    fn translate(&self, formula: &str, d_rows: i64, d_cols: i64) -> Option<String> {
        let _ = (formula, d_rows, d_cols);
        None
    }

    /// Text of `formula` (on sheet `host_sheet`) after the cells of `mv` were
    /// cut and pasted: references to them follow, or `None` to keep it.
    fn moved(&self, formula: &str, host_sheet: &str, mv: &CellMove) -> Option<String> {
        let _ = (formula, host_sheet, mv);
        None
    }
}

/// Recalculates formula results.
pub trait FormulaEngine {
    /// Recompute every formula cell of the workbook, updating cached values.
    fn recalculate(&mut self, wb: &mut Workbook);
}

/// Default: formulas are left untouched and cached values are kept.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFormulas;

impl FormulaAdjuster for NoFormulas {
    fn adjust(&self, _: &str, _: Option<&str>, _: &StructuralChange) -> Option<String> {
        None
    }
}

impl FormulaEngine for NoFormulas {
    fn recalculate(&mut self, _: &mut Workbook) {}
}

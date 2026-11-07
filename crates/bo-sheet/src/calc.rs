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
    Rows,
    Cols,
}

/// Rows or columns inserted / deleted on one sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralChange {
    /// Name of the sheet whose rows/columns change.
    pub sheet: String,
    pub dimension: Dimension,
    /// First zero-based index affected.
    pub at: u32,
    pub count: u32,
    /// `true` = insertion, `false` = deletion.
    pub insert: bool,
}

/// Cells moved by cut and paste: rows `first_row..=last_row` × columns
/// `first_col..=last_col` of sheet `from_sheet`, now `d_rows` / `d_cols` away on `to_sheet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellMove {
    pub from_sheet: String,
    pub to_sheet: String,
    pub first_row: u32,
    pub first_col: u32,
    pub last_row: u32,
    pub last_col: u32,
    pub d_rows: i64,
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

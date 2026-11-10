//! The one reference-shifting primitive of structural edits (insert / delete rows and
//! columns, ECMA-376 Part 1, §18.3.1.73 `row`, §18.3.1.13 `col`).
//!
//! [`RefShift`] describes `count` rows (or columns) inserted before, or deleted from,
//! index `at` of one sheet, and maps everything addressed by position through it:
//!
//! * an index or an inclusive span on the changed axis ([`RefShift::index`],
//!   [`RefShift::span`]);
//! * a cell and a range ([`RefShift::cell`], [`RefShift::range`]): a deleted cell is gone,
//!   a range shrinks (or disappears when every row / column of it is deleted) and grows
//!   when rows / columns are inserted inside it;
//! * a drawing anchor ([`RefShift::anchor`], ECMA-376 Part 1, §20.5.2.15 `from` /
//!   §20.5.2.32 `to` markers with their offsets, and the [MS-XLSX] `controlPr/anchor` of form
//!   controls): its markers move with their cells per [`AnchorMode`] (§20.5.3.2
//!   `ST_EditAs`); a marker whose row / column is deleted lands on the first surviving
//!   row / column at offset 0, so an object whose cells are all deleted collapses there;
//! * formula text, through the workbook's [`FormulaAdjuster`] ([`RefShift::formula`]):
//!   references to deleted cells become `#REF!` (§18.17.2.3 relative / absolute
//!   references move alike for structural edits).
//!
//! [`crate::ops`] applies it to the whole workbook: cells, merges, rules, tables, charts,
//! form controls (anchor, linked cell, input range), sparklines (location, data and date
//! ranges), slicers, timelines and SmartArt frames.

use crate::calc::{Dimension, FormulaAdjuster, StructuralChange};
use bo_model::sheet::{CellRange, CellRef, ControlAnchor, MAX_COLS, MAX_ROWS};

/// How a drawing object follows its cells (ECMA-376 Part 1, §20.5.3.2 `ST_EditAs`; the
/// [MS-XLSX] `moveWithCells` / `sizeWithCells` of a control anchor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorMode {
    /// `twoCell`: both markers follow their cells (moves and resizes).
    MoveAndSize,
    /// `oneCell`: the top-left marker follows its cell, the size is kept.
    Move,
    /// `absolute`: never moves.
    Fixed,
}

/// `count` rows / columns inserted before (or deleted from) zero-based index `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefShift {
    /// The changed axis.
    pub dim: Dimension,
    /// First inserted / deleted index.
    pub at: u32,
    /// How many rows / columns.
    pub count: u32,
    /// `true` = insertion, `false` = deletion.
    pub insert: bool,
}

impl RefShift {
    /// `count` rows inserted before row `at`.
    #[must_use]
    pub fn insert_rows(at: u32, count: u32) -> Self {
        Self::new(Dimension::Rows, at, count, true)
    }

    /// Rows `at..at + count` deleted.
    #[must_use]
    pub fn delete_rows(at: u32, count: u32) -> Self {
        Self::new(Dimension::Rows, at, count, false)
    }

    /// `count` columns inserted before column `at`.
    #[must_use]
    pub fn insert_cols(at: u32, count: u32) -> Self {
        Self::new(Dimension::Cols, at, count, true)
    }

    /// Columns `at..at + count` deleted.
    #[must_use]
    pub fn delete_cols(at: u32, count: u32) -> Self {
        Self::new(Dimension::Cols, at, count, false)
    }

    /// A shift on `dim`.
    #[must_use]
    pub fn new(dim: Dimension, at: u32, count: u32, insert: bool) -> Self {
        RefShift {
            dim,
            at,
            count,
            insert,
        }
    }

    /// Number of rows / columns of a sheet on the changed axis.
    #[must_use]
    pub fn max(self) -> u32 {
        match self.dim {
            Dimension::Rows => MAX_ROWS,
            Dimension::Cols => MAX_COLS,
        }
    }

    /// One past the last deleted index.
    fn end(self) -> u32 {
        self.at.saturating_add(self.count)
    }

    /// The change as handed to a [`FormulaAdjuster`] for sheet `sheet`.
    #[must_use]
    pub fn change(self, sheet: &str) -> StructuralChange {
        StructuralChange {
            sheet: sheet.to_owned(),
            dimension: self.dim,
            at: self.at,
            count: self.count,
            insert: self.insert,
        }
    }

    /// New index of `i`, or `None` if it is deleted / pushed past the sheet limit.
    #[must_use]
    pub fn index(self, i: u32) -> Option<u32> {
        if i < self.at || self.count == 0 {
            return Some(i);
        }
        if self.insert {
            let n = i.checked_add(self.count)?;
            (n < self.max()).then_some(n)
        } else if i < self.end() {
            None
        } else {
            Some(i - self.count)
        }
    }

    /// New inclusive span `a..=b`: insertions inside a span grow it, deletions shrink it;
    /// `None` when the span disappears entirely (or is pushed past the limit).
    #[must_use]
    pub fn span(self, a: u32, b: u32) -> Option<(u32, u32)> {
        if self.count == 0 {
            return Some((a, b));
        }
        if self.insert {
            let na = self.index(a)?;
            let nb = if b >= self.at {
                b.saturating_add(self.count).min(self.max() - 1)
            } else {
                b
            };
            Some((na, nb))
        } else {
            let end = self.end();
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

    /// The coordinate of `c` on the changed axis.
    fn axis(self, c: CellRef) -> u32 {
        match self.dim {
            Dimension::Rows => c.row,
            Dimension::Cols => c.col,
        }
    }

    /// `c` with its coordinate on the changed axis set to `v`.
    fn with_axis(self, c: CellRef, v: u32) -> CellRef {
        match self.dim {
            Dimension::Rows => CellRef::new(v, c.col),
            Dimension::Cols => CellRef::new(c.row, v),
        }
    }

    /// New position of cell `c`; `None` when its row / column is deleted.
    #[must_use]
    pub fn cell(self, c: CellRef) -> Option<CellRef> {
        Some(self.with_axis(c, self.index(self.axis(c))?))
    }

    /// New extent of range `r`; `None` when all its rows / columns are deleted.
    #[must_use]
    pub fn range(self, r: CellRange) -> Option<CellRange> {
        let (a, b) = self.span(self.axis(r.first), self.axis(r.last))?;
        Some(CellRange::new(
            self.with_axis(r.first, a),
            self.with_axis(r.last, b),
        ))
    }

    /// Shifts every range of a list (`sqref`), dropping the deleted ones.
    pub fn ranges(self, v: &mut Vec<CellRange>) {
        *v = v.iter().filter_map(|r| self.range(*r)).collect();
    }

    /// A drawing marker (cell + offset into it, §20.5.2.15 `CT_Marker`) moved with its
    /// cell: a deleted row / column puts it at the first surviving one, offset 0 (on the
    /// changed axis; offsets are `(column, row)`).
    fn marker(self, c: CellRef, off: (i64, i64)) -> (CellRef, (i64, i64)) {
        let v = self.axis(c);
        match self.index(v) {
            Some(n) => (self.with_axis(c, n), off),
            None if self.insert => (self.with_axis(c, self.max() - 1), off),
            None => {
                let off = match self.dim {
                    Dimension::Rows => (off.0, 0),
                    Dimension::Cols => (0, off.1),
                };
                (self.with_axis(c, self.at.min(self.max() - 1)), off)
            }
        }
    }

    /// The anchor of a drawing object or form control after the change (see the module
    /// documentation). The anchor is never dropped: deleting all its cells collapses it.
    #[must_use]
    pub fn anchor(self, a: &ControlAnchor, mode: AnchorMode) -> ControlAnchor {
        if self.count == 0 {
            return *a;
        }
        match mode {
            AnchorMode::Fixed => *a,
            AnchorMode::MoveAndSize => {
                let (from, from_off) = self.marker(a.from, a.from_off);
                let (to, to_off) = self.marker(a.to, a.to_off);
                ControlAnchor {
                    from,
                    from_off,
                    to,
                    to_off,
                }
            }
            AnchorMode::Move => {
                let (from, from_off) = self.marker(a.from, a.from_off);
                let delta = i64::from(self.axis(from)) - i64::from(self.axis(a.from));
                let to_v = (i64::from(self.axis(a.to)) + delta)
                    .clamp(i64::from(self.axis(from)), i64::from(self.max() - 1));
                ControlAnchor {
                    from,
                    from_off,
                    to: self.with_axis(a.to, u32::try_from(to_v).unwrap_or(0)),
                    to_off: a.to_off,
                }
            }
        }
    }

    /// The cell anchor of an object that keeps only cells (charts, slicers, timelines,
    /// SmartArt frames: `from`, and `to` for a two-cell anchor). `None` for `to` keeps the
    /// one-cell form.
    #[must_use]
    pub fn cells_anchor(
        self,
        from: CellRef,
        to: Option<CellRef>,
        mode: AnchorMode,
    ) -> (CellRef, Option<CellRef>) {
        let a = ControlAnchor {
            from,
            to: to.unwrap_or(from),
            ..ControlAnchor::default()
        };
        let n = self.anchor(&a, mode);
        (n.from, to.map(|_| n.to))
    }

    /// Formula text (without `=`) rewritten for the change on sheet `sheet`; `host` is the
    /// sheet unqualified references point at. `None` when nothing changes.
    #[must_use]
    pub fn formula(
        self,
        text: &str,
        host: Option<&str>,
        sheet: &str,
        adj: &dyn FormulaAdjuster,
    ) -> Option<String> {
        adj.adjust(text, host, &self.change(sheet))
    }
}

/// Whether adjusted formula text lost a reference (it now holds `#REF!`).
#[must_use]
pub fn lost_ref(text: &str) -> bool {
    text.to_ascii_uppercase().contains("#REF!")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::BoFormula;

    fn r(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    fn c(s: &str) -> CellRef {
        CellRef::parse(s).unwrap()
    }

    fn range_str(s: RefShift, a1: &str) -> Option<String> {
        s.range(r(a1)).map(|x| x.to_string())
    }

    #[test]
    fn insert_before_inside_after() {
        let s = RefShift::insert_rows(4, 2); // before row 5
        assert_eq!(s.cell(c("B3")), Some(c("B3")));
        assert_eq!(s.cell(c("B5")), Some(c("B7")));
        assert_eq!(range_str(s, "A2:C3").as_deref(), Some("A2:C3"));
        assert_eq!(range_str(s, "A6:C8").as_deref(), Some("A8:C10"));
        // Inside: the range grows.
        assert_eq!(range_str(s, "A3:C6").as_deref(), Some("A3:C8"));
        // Columns.
        let s = RefShift::insert_cols(1, 1);
        assert_eq!(s.cell(c("A1")), Some(c("A1")));
        assert_eq!(s.cell(c("B1")), Some(c("C1")));
        assert_eq!(range_str(s, "A1:B2").as_deref(), Some("A1:C2"));
    }

    #[test]
    fn insert_pushing_past_the_limit_drops() {
        let s = RefShift::insert_rows(0, 1);
        assert_eq!(s.cell(CellRef::new(MAX_ROWS - 1, 0)), None);
        // A range reaching the last row is clipped there.
        let full = CellRange::new(CellRef::new(0, 0), CellRef::new(MAX_ROWS - 1, 0));
        let s = RefShift::insert_rows(1, 3);
        assert_eq!(s.range(full).unwrap().last.row, MAX_ROWS - 1);
    }

    #[test]
    fn delete_overlapping_start_end_and_whole() {
        let s = RefShift::delete_rows(2, 3); // rows 3..5
        assert_eq!(s.cell(c("A2")), Some(c("A2")));
        assert_eq!(s.cell(c("A3")), None);
        assert_eq!(s.cell(c("A5")), None);
        assert_eq!(s.cell(c("A6")), Some(c("A3")));
        // Overlapping the start of the range.
        assert_eq!(range_str(s, "A4:A8").as_deref(), Some("A3:A5"));
        // Overlapping its end.
        assert_eq!(range_str(s, "A1:A4").as_deref(), Some("A1:A2"));
        // Containing the deletion.
        assert_eq!(range_str(s, "A1:A9").as_deref(), Some("A1:A6"));
        // Wholly deleted.
        assert_eq!(range_str(s, "A3:B5"), None);
        let mut v = vec![r("A3:A4"), r("A1:A10")];
        s.ranges(&mut v);
        assert_eq!(v, vec![r("A1:A7")]);
        // Columns.
        let s = RefShift::delete_cols(1, 2);
        assert_eq!(range_str(s, "B1:C1"), None);
        assert_eq!(range_str(s, "A1:E1").as_deref(), Some("A1:C1"));
    }

    fn anchor(from: &str, to: &str) -> ControlAnchor {
        ControlAnchor {
            from: c(from),
            from_off: (10, 20),
            to: c(to),
            to_off: (30, 40),
        }
    }

    #[test]
    fn anchors_move_and_size() {
        let a = anchor("B3", "D6");
        let m = AnchorMode::MoveAndSize;
        // Insert above: both markers move.
        let n = RefShift::insert_rows(0, 2).anchor(&a, m);
        assert_eq!((n.from, n.to), (c("B5"), c("D8")));
        assert_eq!((n.from_off, n.to_off), ((10, 20), (30, 40)));
        // Insert inside: the object grows.
        let n = RefShift::insert_rows(4, 1).anchor(&a, m);
        assert_eq!((n.from, n.to), (c("B3"), c("D7")));
        // Insert below / right: unchanged.
        assert_eq!(RefShift::insert_rows(10, 1).anchor(&a, m), a);
        assert_eq!(RefShift::insert_cols(5, 1).anchor(&a, m), a);
        // Delete rows over the top marker: it lands on the first surviving row, offset 0.
        let n = RefShift::delete_rows(1, 3).anchor(&a, m);
        assert_eq!((n.from, n.from_off), (c("B2"), (10, 0)));
        assert_eq!((n.to, n.to_off), (c("D3"), (30, 40)));
        // Delete over the bottom marker.
        let n = RefShift::delete_rows(4, 5).anchor(&a, m);
        assert_eq!((n.from, n.to, n.to_off), (c("B3"), c("D5"), (30, 0)));
        // Everything deleted: collapsed at the deletion point.
        let n = RefShift::delete_rows(0, 10).anchor(&a, m);
        assert_eq!((n.from, n.to), (c("B1"), c("D1")));
        assert_eq!((n.from_off.1, n.to_off.1), (0, 0));
        // Columns: the column offset resets.
        let n = RefShift::delete_cols(1, 1).anchor(&a, m);
        assert_eq!((n.from, n.from_off), (c("B3"), (0, 20)));
        assert_eq!(n.to, c("C6"));
    }

    #[test]
    fn anchors_move_without_sizing_and_fixed() {
        let a = anchor("B3", "D4");
        let m = AnchorMode::Move;
        let n = RefShift::insert_rows(0, 3).anchor(&a, m);
        assert_eq!((n.from, n.to), (c("B6"), c("D7")));
        // Inserting inside keeps the size (the bottom marker stays put).
        let n = RefShift::insert_rows(3, 5).anchor(&a, m);
        assert_eq!((n.from, n.to), (c("B3"), c("D4")));
        // The top row deleted: the object moves up and keeps its height in rows.
        let n = RefShift::delete_rows(1, 2).anchor(&a, m);
        assert_eq!((n.from, n.to, n.to_off), (c("B2"), c("D3"), (30, 40)));
        assert_eq!(RefShift::delete_rows(0, 5).anchor(&a, AnchorMode::Fixed), a);
        // Cells-only anchors (charts and frames).
        let s = RefShift::insert_cols(0, 1);
        assert_eq!(
            s.cells_anchor(c("B2"), Some(c("C3")), AnchorMode::MoveAndSize),
            (c("C2"), Some(c("D3")))
        );
        assert_eq!(
            s.cells_anchor(c("B2"), None, AnchorMode::Move),
            (c("C2"), None)
        );
    }

    #[test]
    fn formulas_go_through_the_adjuster() {
        let adj = BoFormula::default();
        let s = RefShift::insert_rows(0, 1);
        assert_eq!(
            s.formula("$B$2", Some("S"), "S", &adj).as_deref(),
            Some("$B$3")
        );
        // Another sheet's unqualified reference does not move.
        assert_eq!(s.formula("$B$2", Some("T"), "S", &adj), None);
        assert_eq!(
            s.formula("S!$A$1:$A$3", Some("T"), "S", &adj).as_deref(),
            Some("S!$A$2:$A$4")
        );
        let d = RefShift::delete_rows(1, 1);
        let out = d.formula("$B$2", Some("S"), "S", &adj).unwrap();
        assert!(lost_ref(&out), "{out}");
        assert_eq!(
            d.formula("$A$1:$A$3", Some("S"), "S", &adj).as_deref(),
            Some("$A$1:$A$2")
        );
    }
}

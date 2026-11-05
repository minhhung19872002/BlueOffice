//! Cell reference types and A1 notation helpers.
//!
//! ECMA-376 Part 1, §18.17.2.3 (cell references: A1 and R1C1 styles, absolute
//! `$` markers, ranges, whole rows/columns and sheet-qualified references).

/// Number of rows in a worksheet (grid size used by current spreadsheet applications).
pub const MAX_ROWS: u32 = 1_048_576;
/// Number of columns in a worksheet (`XFD`).
pub const MAX_COLS: u32 = 16_384;

/// A single cell position with absolute/relative markers (0-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellAddr {
    /// Row index (0-based).
    pub row: u32,
    /// Column index (0-based).
    pub col: u32,
    /// `$` before the row.
    pub row_abs: bool,
    /// `$` before the column.
    pub col_abs: bool,
}

impl CellAddr {
    /// A relative address.
    pub fn new(row: u32, col: u32) -> Self {
        CellAddr {
            row,
            col,
            row_abs: false,
            col_abs: false,
        }
    }

    /// An absolute address (`$A$1`).
    pub fn absolute(row: u32, col: u32) -> Self {
        CellAddr {
            row,
            col,
            row_abs: true,
            col_abs: true,
        }
    }

    /// Format in A1 notation with `$` markers.
    pub fn to_a1(&self) -> String {
        let mut s = String::new();
        if self.col_abs {
            s.push('$');
        }
        s.push_str(&col_to_letters(self.col));
        if self.row_abs {
            s.push('$');
        }
        s.push_str(&(self.row + 1).to_string());
        s
    }
}

/// The kind of area a reference designates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// A single cell, e.g. `B3`.
    Cell(CellAddr),
    /// A rectangular range, e.g. `A1:B5` (corners kept as written).
    Range(CellAddr, CellAddr),
    /// Whole columns, e.g. `A:C`.
    Cols {
        /// First column.
        first: u32,
        /// Last column.
        last: u32,
        /// `$` on the first column.
        first_abs: bool,
        /// `$` on the last column.
        last_abs: bool,
    },
    /// Whole rows, e.g. `1:3`.
    Rows {
        /// First row.
        first: u32,
        /// Last row.
        last: u32,
        /// `$` on the first row.
        first_abs: bool,
        /// `$` on the last row.
        last_abs: bool,
    },
}

/// Sheet qualifier of a reference: `Sheet1!` or the 3-D form `Sheet1:Sheet3!`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SheetRef {
    /// First (or only) sheet name.
    pub first: String,
    /// Last sheet of a 3-D reference.
    pub last: Option<String>,
}

impl SheetRef {
    /// A single-sheet qualifier.
    pub fn single(name: impl Into<String>) -> Self {
        SheetRef {
            first: name.into(),
            last: None,
        }
    }
}

/// A (possibly sheet-qualified) cell reference.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    /// Optional sheet qualifier; `None` means the formula's own sheet.
    pub sheet: Option<SheetRef>,
    /// The referenced area.
    pub kind: RefKind,
}

impl Reference {
    /// An unqualified single-cell reference.
    pub fn cell(addr: CellAddr) -> Self {
        Reference {
            sheet: None,
            kind: RefKind::Cell(addr),
        }
    }

    /// Bounds `(r1, c1, r2, c2)` of the referenced area (0-based, inclusive).
    pub fn bounds(&self) -> (u32, u32, u32, u32) {
        match self.kind {
            RefKind::Cell(a) => (a.row, a.col, a.row, a.col),
            RefKind::Range(a, b) => (
                a.row.min(b.row),
                a.col.min(b.col),
                a.row.max(b.row),
                a.col.max(b.col),
            ),
            RefKind::Cols { first, last, .. } => {
                (0, first.min(last), MAX_ROWS - 1, first.max(last))
            }
            RefKind::Rows { first, last, .. } => {
                (first.min(last), 0, first.max(last), MAX_COLS - 1)
            }
        }
    }
}

/// Convert a 0-based column index to letters (`0` → `A`, `26` → `AA`).
pub fn col_to_letters(col: u32) -> String {
    let mut n = col + 1;
    let mut buf = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        buf.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

/// Convert column letters to a 0-based index; `None` if invalid or beyond `XFD`.
pub fn letters_to_col(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for c in s.bytes() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + u32::from(c.to_ascii_uppercase() - b'A' + 1);
    }
    (n >= 1 && n <= MAX_COLS).then(|| n - 1)
}

/// Parse an A1 cell address like `$B$3` occupying the whole string.
pub fn parse_a1_cell(s: &str) -> Option<CellAddr> {
    let (addr, len) = scan_a1_cell(s.as_bytes())?;
    (len == s.len()).then_some(addr)
}

/// Scan an A1 cell address at the start of `b`, returning it and its byte length.
pub(crate) fn scan_a1_cell(b: &[u8]) -> Option<(CellAddr, usize)> {
    let mut i = 0;
    let col_abs = b.first() == Some(&b'$');
    if col_abs {
        i += 1;
    }
    let ls = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let letters = std::str::from_utf8(&b[ls..i]).ok()?;
    let col = letters_to_col(letters)?;
    let row_abs = b.get(i) == Some(&b'$');
    if row_abs {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let row = parse_row_number(&b[ds..i])?;
    Some((
        CellAddr {
            row,
            col,
            row_abs,
            col_abs,
        },
        i,
    ))
}

/// Parse a 1-based row number into a 0-based index.
pub(crate) fn parse_row_number(digits: &[u8]) -> Option<u32> {
    if digits.is_empty() || digits.len() > 7 || digits[0] == b'0' {
        return None;
    }
    let n: u32 = std::str::from_utf8(digits).ok()?.parse().ok()?;
    (1..=MAX_ROWS).contains(&n).then(|| n - 1)
}

/// Parse an A1-style area string such as `A1`, `$A$1:B5`, `A:C` or `2:4`
/// (without sheet qualifier).
pub fn parse_a1_area(s: &str) -> Option<RefKind> {
    let b = s.as_bytes();
    if let Some((a, n)) = scan_a1_cell(b) {
        if n == b.len() {
            return Some(RefKind::Cell(a));
        }
        if b[n] == b':' {
            let (c, m) = scan_a1_cell(&b[n + 1..])?;
            return (n + 1 + m == b.len()).then_some(RefKind::Range(a, c));
        }
        return None;
    }
    let (first, l1) = scan_col_only(b)?;
    if b.get(l1) == Some(&b':') {
        let (last, l2) = scan_col_only(&b[l1 + 1..])?;
        if l1 + 1 + l2 == b.len() {
            return Some(RefKind::Cols {
                first: first.0,
                last: last.0,
                first_abs: first.1,
                last_abs: last.1,
            });
        }
    }
    None
}

/// Scan `$?LETTERS` returning (col, abs) and length.
pub(crate) fn scan_col_only(b: &[u8]) -> Option<((u32, bool), usize)> {
    let mut i = 0;
    let abs = b.first() == Some(&b'$');
    if abs {
        i += 1;
    }
    let s = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col = letters_to_col(std::str::from_utf8(&b[s..i]).ok()?)?;
    Some(((col, abs), i))
}

/// Scan `$?DIGITS` returning (row, abs) and length.
pub(crate) fn scan_row_only(b: &[u8]) -> Option<((u32, bool), usize)> {
    let mut i = 0;
    let abs = b.first() == Some(&b'$');
    if abs {
        i += 1;
    }
    let s = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let row = parse_row_number(&b[s..i])?;
    Some(((row, abs), i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_roundtrip() {
        for (c, s) in [(0, "A"), (25, "Z"), (26, "AA"), (701, "ZZ"), (702, "AAA"), (16_383, "XFD")] {
            assert_eq!(col_to_letters(c), s);
            assert_eq!(letters_to_col(s), Some(c));
        }
        assert_eq!(letters_to_col("XFE"), None);
    }

    #[test]
    fn a1_parsing() {
        assert_eq!(parse_a1_cell("$B$3"), Some(CellAddr::absolute(2, 1)));
        assert_eq!(parse_a1_cell("A0"), None);
        assert_eq!(parse_a1_cell("A1048577"), None);
        assert!(matches!(parse_a1_area("A:C"), Some(RefKind::Cols { first: 0, last: 2, .. })));
    }
}

//! Multi-level sort of a cell range (rows reordered top-to-bottom).
//!
//! Ascending order follows the usual spreadsheet convention: numbers, then text
//! (case-insensitive unless requested), then logical values (FALSE < TRUE), then
//! errors; blank cells always sort last, in both directions. The sort is stable.
//! Only the cells inside the range move (values, formulas, styles); formula
//! references are not rewritten here (see [`crate::calc`]).

use crate::calc::{FormulaAdjuster, NoFormulas};
use bo_model::sheet::{Cell, CellRange, CellValue, Worksheet};
use std::cmp::Ordering;

/// One sort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    /// Absolute zero-based column of the key.
    pub col: u32,
    pub descending: bool,
    pub case_sensitive: bool,
}

impl SortKey {
    /// Ascending, case-insensitive key on `col`.
    pub fn asc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: false,
            case_sensitive: false,
        }
    }

    /// Descending, case-insensitive key on `col`.
    pub fn desc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: true,
            case_sensitive: false,
        }
    }
}

fn rank(v: &CellValue) -> u8 {
    match v {
        CellValue::Number(_) | CellValue::Date(_) => 0,
        CellValue::Text(_) | CellValue::RichText(_) => 1,
        CellValue::Bool(_) => 2,
        CellValue::Error(_) => 3,
        CellValue::Empty => 4,
    }
}

fn is_blank(v: Option<&CellValue>) -> bool {
    match v {
        None | Some(CellValue::Empty) => true,
        Some(CellValue::Text(s)) => s.is_empty(),
        _ => false,
    }
}

/// Compare two non-blank values in ascending order.
pub fn compare_values(a: &CellValue, b: &CellValue, case_sensitive: bool) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => x.total_cmp(y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x.cmp(y),
        (CellValue::Date(x), CellValue::Date(y)) => x.cmp(y),
        _ => {
            let (sa, sb) = (
                a.as_text().unwrap_or_default(),
                b.as_text().unwrap_or_default(),
            );
            if case_sensitive {
                // Lower case before upper case for otherwise equal text.
                text_key(&sa)
                    .cmp(&text_key(&sb))
                    .then_with(|| sa.to_lowercase().cmp(&sb.to_lowercase()))
                    .then_with(|| sb.cmp(&sa))
            } else {
                text_key(&sa)
                    .cmp(&text_key(&sb))
                    .then_with(|| sa.to_lowercase().cmp(&sb.to_lowercase()))
            }
        }
    }
}

/// Collation key for text: lower case with diacritics removed (NFD, combining marks
/// dropped) and `đ` folded to `d`, so Vietnamese words sort by their base letters.
pub fn text_key(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F))
        .map(|c| match c {
            'đ' | 'Đ' => 'd',
            c => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn value_at(row: &[(u32, Cell)], col: u32) -> Option<&CellValue> {
    row.iter()
        .find(|(c, _)| *c == col)
        .map(|(_, cell)| &cell.value)
}

fn key_cmp(a: Option<&CellValue>, b: Option<&CellValue>, k: SortKey) -> Ordering {
    match (is_blank(a), is_blank(b)) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            let (Some(a), Some(b)) = (a, b) else {
                return Ordering::Equal;
            };
            let o = compare_values(a, b, k.case_sensitive);
            if k.descending {
                o.reverse()
            } else {
                o
            }
        }
    }
}

/// Sort the rows of `range` by `keys` (first key most significant). With
/// `has_header` the first row stays in place. Keys outside the range are ignored.
/// Formula text moves unchanged; see [`sort_range_with`].
pub fn sort_range(ws: &mut Worksheet, range: CellRange, keys: &[SortKey], has_header: bool) {
    sort_range_with(ws, range, keys, has_header, &NoFormulas);
}

/// [`sort_range`] that rewrites the formulas of moved rows through `adj`
/// ([`FormulaAdjuster::translate`] by the row distance), so relative
/// references keep pointing at the same row, as when the row is cut and pasted.
pub fn sort_range_with(
    ws: &mut Worksheet,
    range: CellRange,
    keys: &[SortKey],
    has_header: bool,
    adj: &dyn FormulaAdjuster,
) {
    let keys: Vec<SortKey> = keys
        .iter()
        .copied()
        .filter(|k| k.col >= range.first.col && k.col <= range.last.col)
        .collect();
    let first = range.first.row + u32::from(has_header);
    if keys.is_empty() || first >= range.last.row {
        return;
    }
    let (c0, c1) = (range.first.col, range.last.col);
    // Take the rows out.
    let mut rows: Vec<(u32, Vec<(u32, Cell)>)> =
        Vec::with_capacity((range.last.row - first + 1) as usize);
    for r in first..=range.last.row {
        let cols: Vec<u32> = ws
            .cells
            .range((r, c0)..=(r, c1))
            .map(|(&(_, c), _)| c)
            .collect();
        let row = cols
            .into_iter()
            .filter_map(|c| ws.cells.remove(&(r, c)).map(|cell| (c, cell)))
            .collect();
        rows.push((r, row));
    }
    rows.sort_by(|(_, a), (_, b)| {
        keys.iter()
            .map(|k| key_cmp(value_at(a, k.col), value_at(b, k.col), *k))
            .find(|o| *o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    for (i, (from, row)) in rows.into_iter().enumerate() {
        let r = first + i as u32;
        let d = i64::from(r) - i64::from(from);
        for (c, mut cell) in row {
            if let Some(f) = &mut cell.formula {
                if d != 0 && !f.text.is_empty() {
                    if let Some(t) = adj.translate(&f.text, d, 0) {
                        f.text = t;
                    }
                }
            }
            ws.cells.insert((r, c), cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::CellError;

    fn sheet(rows: &[(&str, CellValue)]) -> Worksheet {
        let mut ws = Worksheet::new("S");
        ws.set_a1("A1", CellValue::Text("Name".into()));
        ws.set_a1("B1", CellValue::Text("Score".into()));
        for (i, (name, v)) in rows.iter().enumerate() {
            let r = i as u32 + 1;
            ws.set_cell(r, 0, Cell::new(CellValue::Text((*name).into())));
            ws.set_cell(r, 1, Cell::new(v.clone()).with_style(r));
        }
        ws
    }

    fn col(ws: &Worksheet, c: u32) -> Vec<String> {
        (1..=ws.used_range().unwrap().last.row)
            .map(|r| match ws.cell(r, c).map(|c| &c.value) {
                Some(CellValue::Number(n)) => n.to_string(),
                Some(v) => v.as_text().unwrap_or_else(|| format!("{v:?}")),
                None => "-".into(),
            })
            .collect()
    }

    #[test]
    fn multi_level_with_header_and_blanks() {
        let mut ws = sheet(&[
            ("Bình", CellValue::Number(7.0)),
            ("an", CellValue::Number(9.0)),
            ("Chi", CellValue::Empty),
            ("An", CellValue::Number(7.0)),
            ("Dũng", CellValue::Text("x".into())),
        ]);
        let range = CellRange::parse("A1:B6").unwrap();
        sort_range(&mut ws, range, &[SortKey::desc(1), SortKey::asc(0)], true);
        assert_eq!(
            ws.cell_a1("A1").unwrap().value,
            CellValue::Text("Name".into())
        );
        // Descending: text ("x") ranks above numbers; blanks still last.
        assert_eq!(col(&ws, 0), ["Dũng", "an", "An", "Bình", "Chi"]);
        // Styles travel with the cells.
        assert_eq!(ws.cell(2, 1).unwrap().style, 2);
        sort_range(&mut ws, range, &[SortKey::asc(1), SortKey::asc(0)], true);
        assert_eq!(col(&ws, 0), ["An", "Bình", "an", "Dũng", "Chi"]);
    }

    #[test]
    fn type_order() {
        use CellValue::*;
        let mut v = [
            Error(CellError::NA),
            Bool(true),
            Text("b".into()),
            Number(2.0),
            Bool(false),
            Text("A".into()),
            Number(-1.0),
        ];
        v.sort_by(|a, b| compare_values(a, b, false));
        assert_eq!(
            v,
            [
                Number(-1.0),
                Number(2.0),
                Text("A".into()),
                Text("b".into()),
                Bool(false),
                Bool(true),
                Error(CellError::NA)
            ]
        );
        assert_eq!(
            compare_values(&Text("a".into()), &Text("A".into()), true),
            Ordering::Less
        );
    }
}

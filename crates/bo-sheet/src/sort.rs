//! Multi-level sort of a cell range: rows reordered top-to-bottom, or columns
//! left-to-right (ECMA-376 Part 1, §18.3.1.92 `sortState/@columnSort`), by
//! value or by cell / font colour (§18.3.1.91 `sortCondition/@sortBy`).
//!
//! Ascending order follows the usual spreadsheet convention: numbers, then text
//! (case-insensitive unless requested), then logical values (FALSE < TRUE), then
//! errors; blank cells always sort last, in both directions. The sort is stable.
//! Only the cells inside the range move (values, formulas, styles); formula
//! references are not rewritten here (see [`crate::calc`]).

use crate::calc::{FormulaAdjuster, NoFormulas};
use bo_model::sheet::{Cell, CellRange, CellValue, Workbook, Worksheet};
use std::cmp::Ordering;

/// What a sort level compares (§18.18.79 `ST_SortBy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    /// Compare cell values (numbers before text, blanks last).
    #[default]
    Value,
    /// Cells filled with this colour first (last when descending).
    CellColor((u8, u8, u8)),
    /// Cells whose font has this colour first (last when descending).
    FontColor((u8, u8, u8)),
}

/// One sort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    /// Absolute zero-based column of the key (the key row when sorting left to right).
    pub col: u32,
    /// Sort from largest to smallest (`descending`).
    pub descending: bool,
    /// Text comparison distinguishes case (`caseSensitive`).
    pub case_sensitive: bool,
    /// What the level compares.
    pub by: SortBy,
}

impl SortKey {
    /// Ascending, case-insensitive key on `col`.
    pub fn asc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: false,
            case_sensitive: false,
            by: SortBy::Value,
        }
    }

    /// Descending, case-insensitive key on `col`.
    pub fn desc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: true,
            case_sensitive: false,
            by: SortBy::Value,
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

fn cell_at(line: &[(u32, Cell)], pos: u32) -> Option<&Cell> {
    line.iter().find(|(c, _)| *c == pos).map(|(_, cell)| cell)
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
/// Colour keys are ignored (they need the stylesheet, see [`sort_lines`]).
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
        .filter(|k| k.by == SortBy::Value)
        .collect();
    let mut wb = Workbook {
        sheets: vec![std::mem::take(ws)],
        ..Workbook::default()
    };
    sort_lines(&mut wb, 0, range, &keys, has_header, false, adj);
    *ws = wb.sheets.pop().unwrap_or_default();
}

/// Sort `range` of sheet `sheet`: rows by key columns, or with `by_columns`
/// the columns left to right by key rows. With `has_header` the first row
/// (first column) stays in place. Colour keys use the workbook's styles.
pub fn sort_lines(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    keys: &[SortKey],
    has_header: bool,
    by_columns: bool,
    adj: &dyn FormulaAdjuster,
) {
    let (lo, hi) = if by_columns {
        (range.first.row, range.last.row)
    } else {
        (range.first.col, range.last.col)
    };
    let keys: Vec<SortKey> = keys
        .iter()
        .copied()
        .filter(|k| k.col >= lo && k.col <= hi)
        .collect();
    let (start, end) = if by_columns {
        (range.first.col + u32::from(has_header), range.last.col)
    } else {
        (range.first.row + u32::from(has_header), range.last.row)
    };
    if keys.is_empty() || start >= end || sheet >= wb.sheets.len() {
        return;
    }
    // Take the lines out: (original index, [(position across the line, cell)]).
    let mut lines: Vec<(u32, Vec<(u32, Cell)>)> = Vec::with_capacity((end - start + 1) as usize);
    {
        let ws = &mut wb.sheets[sheet];
        for i in start..=end {
            let keys_in_line: Vec<(u32, u32)> = if by_columns {
                (range.first.row..=range.last.row)
                    .filter(|r| ws.cells.contains_key(&(*r, i)))
                    .map(|r| (r, i))
                    .collect()
            } else {
                ws.cells
                    .range((i, range.first.col)..=(i, range.last.col))
                    .map(|(&k, _)| k)
                    .collect()
            };
            let line = keys_in_line
                .into_iter()
                .filter_map(|k| {
                    let pos = if by_columns { k.0 } else { k.1 };
                    ws.cells.remove(&k).map(|cell| (pos, cell))
                })
                .collect();
            lines.push((i, line));
        }
    }
    let colour_of = |cell: Option<&Cell>, by: SortBy| -> bool {
        let Some(cell) = cell else { return false };
        match by {
            SortBy::Value => false,
            SortBy::CellColor(c) => crate::color::cell_fill_rgb(wb, cell) == Some(c),
            SortBy::FontColor(c) => crate::color::cell_font_rgb(wb, cell).unwrap_or((0, 0, 0)) == c,
        }
    };
    let cmp = |a: &[(u32, Cell)], b: &[(u32, Cell)]| -> Ordering {
        keys.iter()
            .map(|k| {
                let (ca, cb) = (cell_at(a, k.col), cell_at(b, k.col));
                match k.by {
                    SortBy::Value => key_cmp(ca.map(|c| &c.value), cb.map(|c| &c.value), *k),
                    by => {
                        // Matching cells on top (descending: at the bottom).
                        let o = colour_of(cb, by).cmp(&colour_of(ca, by));
                        if k.descending {
                            o.reverse()
                        } else {
                            o
                        }
                    }
                }
            })
            .find(|o| *o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    };
    lines.sort_by(|(_, a), (_, b)| cmp(a, b));
    let ws = &mut wb.sheets[sheet];
    for (n, (from, line)) in lines.into_iter().enumerate() {
        let to = start + n as u32;
        let d = i64::from(to) - i64::from(from);
        for (pos, mut cell) in line {
            if let Some(f) = &mut cell.formula {
                if d != 0 && !f.text.is_empty() {
                    let (dr, dc) = if by_columns { (0, d) } else { (d, 0) };
                    if let Some(t) = adj.translate(&f.text, dr, dc) {
                        f.text = t;
                    }
                }
            }
            let key = if by_columns { (pos, to) } else { (to, pos) };
            ws.cells.insert(key, cell);
        }
    }
}

/// Distinct fill and font colours (`RRGGBB`) in one column (row when
/// `by_columns`) of `range`, for the "sort by colour" choices.
pub fn line_colors(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    key: u32,
    by_columns: bool,
) -> (Vec<String>, Vec<String>) {
    let mut fills = Vec::new();
    let mut fonts = Vec::new();
    let Some(ws) = wb.sheets.get(sheet) else {
        return (fills, fonts);
    };
    let cells: Vec<&Cell> = if by_columns {
        (range.first.col..=range.last.col)
            .filter_map(|c| ws.cell(key, c))
            .collect()
    } else {
        (range.first.row..=range.last.row)
            .filter_map(|r| ws.cell(r, key))
            .collect()
    };
    for cell in cells.into_iter().take(100_000) {
        if let Some(c) = crate::color::cell_fill_rgb(wb, cell) {
            let h = crate::color::hex(c);
            if !fills.contains(&h) {
                fills.push(h);
            }
        }
        if !matches!(cell.value, CellValue::Empty) {
            let h = crate::color::hex(crate::color::cell_font_rgb(wb, cell).unwrap_or((0, 0, 0)));
            if !fonts.contains(&h) {
                fonts.push(h);
            }
        }
    }
    (fills, fonts)
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

    #[test]
    fn left_to_right_and_by_colour() {
        use bo_model::sheet::{CellXf, Fill, Font, SheetColor};
        let mut wb = Workbook::new();
        let red = wb.styles.add_fill(Fill::solid("FF0000"));
        let red_xf = wb.styles.add_xf(CellXf {
            fill_id: red,
            ..CellXf::default()
        });
        let blue_font = wb.styles.add_font(Font {
            color: Some(SheetColor::rgb("0000FF")),
            ..Font::default()
        });
        let blue_xf = wb.styles.add_xf(CellXf {
            font_id: blue_font,
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        // Header column A, then three columns sorted by row 2 (left to right).
        for (c, (name, v)) in [("Năm", 0.0), ("c", 3.0), ("a", 1.0), ("b", 2.0)]
            .iter()
            .enumerate()
        {
            ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*name).into())));
            ws.set_cell(1, c as u32, Cell::new(CellValue::Number(*v)));
        }
        let range = CellRange::parse("A1:D2").unwrap();
        let by_row2 = SortKey::asc(1);
        sort_lines(&mut wb, 0, range, &[by_row2], true, true, &NoFormulas);
        let ws = &wb.sheets[0];
        let row0: Vec<String> = (0..4)
            .map(|c| {
                ws.cell(0, c)
                    .and_then(|x| x.value.as_text())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(row0, ["Năm", "a", "b", "c"]);
        // Colour sort: red-filled rows first, then blue text.
        let ws = &mut wb.sheets[0];
        ws.cells.clear();
        for (r, (v, style)) in [(1.0, 0), (2.0, red_xf), (3.0, blue_xf), (4.0, red_xf)]
            .iter()
            .enumerate()
        {
            ws.set_cell(
                r as u32,
                0,
                Cell::new(CellValue::Number(*v)).with_style(*style),
            );
        }
        let range = CellRange::parse("A1:A4").unwrap();
        let red_key = SortKey {
            by: SortBy::CellColor((255, 0, 0)),
            ..SortKey::asc(0)
        };
        let blue_key = SortKey {
            by: SortBy::FontColor((0, 0, 255)),
            ..SortKey::asc(0)
        };
        sort_lines(
            &mut wb,
            0,
            range,
            &[red_key, blue_key],
            false,
            false,
            &NoFormulas,
        );
        assert_eq!(col_vals(&wb.sheets[0]), [2.0, 4.0, 3.0, 1.0]);
        let (fills, fonts) = line_colors(&wb, 0, range, 0, false);
        assert_eq!(fills, ["FF0000"]);
        assert_eq!(fonts, ["000000", "0000FF"]);
        // Descending colour key: red at the bottom.
        let red_desc = SortKey {
            descending: true,
            ..red_key
        };
        sort_lines(&mut wb, 0, range, &[red_desc], false, false, &NoFormulas);
        assert_eq!(col_vals(&wb.sheets[0])[2..], [2.0, 4.0]);
    }

    fn col_vals(ws: &Worksheet) -> Vec<f64> {
        (0..4)
            .map(|r| match ws.cell(r, 0).map(|c| &c.value) {
                Some(CellValue::Number(n)) => *n,
                _ => f64::NAN,
            })
            .collect()
    }
}

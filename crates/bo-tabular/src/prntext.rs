//! Formatted text, space delimited (`.prn`): what the sheet shows, in fixed-width
//! columns.
//!
//! Writing puts the displayed text of every cell (number formats applied) into
//! columns padded with spaces: a column is as wide as the sheet column in
//! characters, widened to its longest text so that nothing is cut off, numbers
//! are right-aligned and everything else left-aligned, and one space separates
//! neighbouring columns.
//!
//! Reading is a best-effort guess, since the format does not say where columns
//! are: a character position that is blank on every line separates two columns.
//! Texts with spaces inside therefore stay in one column only where another line
//! has a character at that position. A run holding only texts directly followed
//! by a run holding only numbers, never both on one line, is one column (a
//! left-aligned heading over right-aligned numbers). Fields that look like
//! numbers or booleans become such values.

use crate::text::{chars_to_width, detect_value, lines, single_sheet, width_to_chars};
use crate::{Result, TabularError};
use bo_model::sheet::{Cell, CellValue, ColInfo, Workbook, Worksheet, MAX_COLS, MAX_ROWS};
use bo_sheet::numfmt::Locale;

/// Default column width in characters when the sheet gives none (§18.3.1.81
/// `sheetFormatPr/@baseColWidth`).
const DEFAULT_CHARS: u32 = 8;

/// The bytes are text: no control characters other than white space in the first
/// kilobytes (a `.prn` file can also be raw printer output, which is not opened).
#[must_use]
pub fn is_text(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    bytes
        .iter()
        .take(8192)
        .all(|b| *b >= 0x20 || matches!(b, b'\t' | b'\r' | b'\n' | 0x0C | 0x1A))
}

/// A run of character positions `[start, end)` and its field on every line that
/// has one: (row, value).
struct Span {
    start: usize,
    end: usize,
    values: Vec<(u32, CellValue)>,
}

impl Span {
    fn all(&self, f: impl Fn(&CellValue) -> bool) -> bool {
        self.values.iter().all(|(_, v)| f(v))
    }
}

/// `a` holds a left-aligned heading and `b` the right-aligned numbers below it.
fn heading_over_numbers(a: &Span, b: &Span) -> bool {
    if a.values.is_empty() || b.values.is_empty() {
        return false;
    }
    let rows: std::collections::HashSet<u32> = a.values.iter().map(|(r, _)| *r).collect();
    a.all(|v| matches!(v, CellValue::Text(_)))
        && b.all(|v| matches!(v, CellValue::Number(_)))
        && b.values.iter().all(|(r, _)| !rows.contains(r))
}

/// Read space-delimited text into a workbook with one sheet. `decimal` is the
/// decimal separator of numbers.
pub fn read_prn(bytes: &[u8], decimal: char) -> Result<Workbook> {
    let text = bo_encoding::codepage::decode_text(bytes);
    let mut rows: Vec<Vec<char>> = lines(&text)
        .map(|l| {
            l.chars()
                .filter(|c| !matches!(c, '\u{0C}' | '\u{1A}'))
                .map(|c| if c == '\t' { ' ' } else { c })
                .collect()
        })
        .collect();
    while rows.last().is_some_and(|r| r.iter().all(|c| *c == ' ')) {
        rows.pop();
    }
    if rows.len() > MAX_ROWS as usize {
        return Err(TabularError::Limit(format!("{} rows", rows.len())));
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    // Positions holding a character on some line.
    let mut used = vec![false; width];
    for r in &rows {
        for (u, c) in used.iter_mut().zip(r) {
            *u |= *c != ' ';
        }
    }
    // Fields: maximal runs of used positions.
    let mut spans: Vec<Span> = Vec::new();
    let mut start = None;
    for (i, u) in used.iter().enumerate() {
        match (u, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push(Span {
                    start: s,
                    end: i,
                    values: Vec::new(),
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push(Span {
            start: s,
            end: width,
            values: Vec::new(),
        });
    }
    for (row, line) in (0u32..).zip(&rows) {
        for span in &mut spans {
            if span.start >= line.len() {
                break;
            }
            let field: String = line[span.start..span.end.min(line.len())].iter().collect();
            let field = field.trim();
            if !field.is_empty() {
                span.values.push((row, detect_value(field, decimal)));
            }
        }
    }
    // A heading and the numbers under it are one column.
    let mut columns: Vec<Span> = Vec::new();
    for span in spans {
        match columns.last_mut() {
            Some(prev) if heading_over_numbers(prev, &span) => {
                prev.end = span.end;
                prev.values.extend(span.values);
            }
            _ => columns.push(span),
        }
    }
    if columns.len() > MAX_COLS as usize {
        return Err(TabularError::Limit(format!("{} columns", columns.len())));
    }
    let mut ws = Worksheet::new("Sheet1");
    for (col, span) in (0u32..).zip(columns) {
        // Columns as wide as their fields.
        let chars = u32::try_from(span.end - span.start)
            .unwrap_or(u32::MAX)
            .min(255);
        if chars != DEFAULT_CHARS {
            ws.cols.push(ColInfo {
                min: col,
                max: col,
                width: Some(chars_to_width(f64::from(chars))),
                custom_width: true,
                ..ColInfo::default()
            });
        }
        for (row, value) in span.values {
            ws.cells.insert((row, col), Cell::new(value));
        }
    }
    Ok(single_sheet(ws))
}

/// Displayed text of a cell on one line.
fn shown(wb: &Workbook, cell: &Cell, locale: &Locale) -> String {
    bo_sheet::render::format_cell(wb, cell, locale)
        .text
        .chars()
        .filter(|c| *c != '\r')
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Largest text written, in bytes: padding makes a few cells far to the right on
/// many rows into a very large file.
pub const MAX_BYTES: usize = 128 * 1024 * 1024;

/// Serialise worksheet `ws` of `wb` as formatted text: the displayed text of the
/// cells from `A1` to the bottom-right used cell, in space-padded columns. A text
/// of more than [`MAX_BYTES`] is refused.
pub fn write_prn(wb: &Workbook, ws: &Worksheet, locale: &Locale) -> Result<Vec<u8>> {
    let Some(range) = ws.used_range() else {
        return Ok(Vec::new());
    };
    let cols = range.last.col as usize + 1;
    let default = ws
        .format
        .default_col_width
        .map_or(DEFAULT_CHARS, width_to_chars);
    let mut widths = vec![default as usize; cols];
    for c in &ws.cols {
        if let Some(w) = c.width {
            for col in c.min..=c.max.min(range.last.col) {
                if let Some(slot) = widths.get_mut(col as usize) {
                    *slot = width_to_chars(w) as usize;
                }
            }
        }
    }
    for ((_, col), cell) in &ws.cells {
        let n = shown(wb, cell, locale).chars().count();
        if let Some(w) = widths.get_mut(*col as usize) {
            *w = (*w).max(n);
        }
    }
    // Where each column starts: one space between neighbours.
    let mut starts = Vec::with_capacity(cols);
    let mut x = 0usize;
    for w in &widths {
        starts.push(x);
        x += w + 1;
    }
    let mut out = String::new();
    for row in 0..=range.last.row {
        let mut line = String::new();
        let mut at = 0usize; // characters on the line so far
        for ((_, col), cell) in ws.cells.range((row, 0)..=(row, range.last.col)) {
            let text = shown(wb, cell, locale);
            let (Some(begin), Some(w)) = (starts.get(*col as usize), widths.get(*col as usize))
            else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            let n = text.chars().count();
            let pad = if matches!(cell.value, CellValue::Number(_)) {
                w.saturating_sub(n)
            } else {
                0
            };
            line.extend(std::iter::repeat_n(' ', (begin + pad).saturating_sub(at)));
            line.push_str(&text);
            at = begin + pad + n;
        }
        if out.len() + line.len() > MAX_BYTES {
            return Err(TabularError::Limit(format!(
                "the formatted text is longer than {MAX_BYTES} bytes"
            )));
        }
        out.push_str(&line);
        out.push_str("\r\n");
    }
    Ok(bo_encoding::codepage::encode_text(&out))
}

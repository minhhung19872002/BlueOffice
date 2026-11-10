//! Data Interchange Format (`.dif`): a table of numbers and strings as text.
//!
//! Written from the published description of the format (Software Arts, "DIF: a
//! format for data exchange between application programs"). A file has a header
//! and a data part:
//!
//! * header items are three lines — a topic (`TABLE`, `VECTORS`, `TUPLES`,
//!   `DATA`, …), a line `vector,value` and a quoted string. `DATA` ends the header;
//! * data values are two lines — `type,number` and a string:
//!   `-1,0` + `BOT` starts a tuple (a row), `-1,0` + `EOD` ends the data;
//!   `0,n` + a value indicator is a number (`V` valid, `NA` not available,
//!   `ERROR`, `TRUE`, `FALSE`); `1,0` + `"text"` is a string.
//!
//! The format holds values only: no formulas and no formatting. The `VECTORS` and
//! `TUPLES` counts are written (columns and rows) but never relied on when reading
//! — spreadsheet applications disagree about which is which — and a row is simply
//! what follows a `BOT`.

use crate::text::{lines, number_text, single_sheet};
use crate::{Result, TabularError};
use bo_model::sheet::{Cell, CellError, CellValue, Workbook, Worksheet, MAX_COLS, MAX_ROWS};
use std::fmt::Write as _;

/// The bytes start with the `TABLE` header item of a DIF file.
#[must_use]
pub fn is_dif(bytes: &[u8]) -> bool {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let Some(rest) = b.strip_prefix(b"TABLE") else {
        return false;
    };
    let rest = rest.strip_prefix(b"\r").unwrap_or(rest);
    let rest = rest.strip_prefix(b"\n").unwrap_or(rest);
    rest.len() < b.len() - 5 && rest.starts_with(b"0,")
}

/// The text of a quoted string line.
fn unquote(s: &str) -> String {
    let t = s.trim();
    match t.strip_prefix('"') {
        Some(inner) => inner
            .strip_suffix('"')
            .unwrap_or(inner)
            .replace("\"\"", "\""),
        None => t.to_owned(),
    }
}

/// Read a DIF file into a workbook with one sheet.
pub fn read_dif(bytes: &[u8]) -> Result<Workbook> {
    if !is_dif(bytes) {
        return Err(TabularError::NotDif);
    }
    let text = bo_encoding::codepage::decode_text(bytes);
    let mut it = lines(&text);
    // Header: items of three lines up to and including DATA.
    loop {
        let Some(topic) = it.next() else {
            return Ok(single_sheet(Worksheet::new("Sheet1")));
        };
        let _numbers = it.next();
        let _string = it.next();
        if topic.trim() == "DATA" {
            break;
        }
    }
    let mut ws = Worksheet::new("Sheet1");
    let (mut row, mut col) = (0u32, 0u32);
    let mut started = false;
    while let Some(head) = it.next() {
        let Some(string) = it.next() else {
            break;
        };
        let Some((kind, number)) = head.split_once(',') else {
            continue;
        };
        let value = match kind.trim() {
            "-1" => {
                match string.trim() {
                    "BOT" => {
                        if started {
                            row += 1;
                        }
                        started = true;
                        col = 0;
                    }
                    "EOD" => break,
                    _ => {}
                }
                continue;
            }
            "0" => match string.trim() {
                "TRUE" => CellValue::Bool(true),
                "FALSE" => CellValue::Bool(false),
                "NA" => CellValue::Error(CellError::NA),
                "ERROR" => CellValue::Error(CellError::Value),
                _ => match number.trim().parse::<f64>() {
                    Ok(n) if n.is_finite() => CellValue::Number(n),
                    _ => CellValue::Empty,
                },
            },
            "1" => {
                let s = unquote(string);
                if s.is_empty() {
                    CellValue::Empty
                } else {
                    CellValue::Text(s)
                }
            }
            _ => continue,
        };
        if row >= MAX_ROWS || col >= MAX_COLS {
            return Err(TabularError::Limit(format!("cell ({row}, {col})")));
        }
        if value != CellValue::Empty {
            ws.cells.insert((row, col), Cell::new(value));
        }
        col += 1;
    }
    Ok(single_sheet(ws))
}

/// Most values written: every tuple holds a value for every vector, so a few
/// cells far apart would otherwise make a file of gigabytes.
pub const MAX_VALUES: u64 = 8_000_000;

/// Serialise the values of a worksheet (cached results, not formulas) as DIF,
/// from `A1` to the bottom-right used cell. A line break inside a text becomes a
/// space: a DIF string is one line. A used range of more than [`MAX_VALUES`]
/// cells is refused.
pub fn write_dif(ws: &Worksheet) -> Result<Vec<u8>> {
    let (rows, cols) = ws
        .used_range()
        .map_or((0, 0), |r| (r.last.row + 1, r.last.col + 1));
    if u64::from(rows) * u64::from(cols) > MAX_VALUES {
        return Err(TabularError::Limit(format!(
            "{rows} rows of {cols} columns (DIF holds at most {MAX_VALUES} values)"
        )));
    }
    let mut out = String::new();
    let title: String = ws
        .name
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .collect();
    let _ = write!(out, "TABLE\r\n0,1\r\n\"{title}\"\r\n");
    let _ = write!(out, "VECTORS\r\n0,{cols}\r\n\"\"\r\n");
    let _ = write!(out, "TUPLES\r\n0,{rows}\r\n\"\"\r\n");
    out.push_str("DATA\r\n0,0\r\n\"\"\r\n");
    for row in 0..rows {
        out.push_str("-1,0\r\nBOT\r\n");
        for col in 0..cols {
            match ws.cell(row, col).map(|c| &c.value) {
                Some(CellValue::Number(n)) if n.is_finite() => {
                    let _ = write!(out, "0,{}\r\nV\r\n", number_text(*n));
                }
                Some(CellValue::Bool(true)) => out.push_str("0,1\r\nTRUE\r\n"),
                Some(CellValue::Bool(false)) => out.push_str("0,0\r\nFALSE\r\n"),
                Some(CellValue::Error(CellError::NA)) => out.push_str("0,0\r\nNA\r\n"),
                // Other errors, and numbers that are not finite.
                Some(CellValue::Number(_) | CellValue::Error(_)) => {
                    out.push_str("0,0\r\nERROR\r\n");
                }
                Some(v @ (CellValue::Text(_) | CellValue::RichText(_))) => {
                    string(&v.as_text().unwrap_or_default(), &mut out);
                }
                Some(CellValue::Date(d)) => string(d, &mut out),
                Some(CellValue::Empty) | None => out.push_str("1,0\r\n\"\"\r\n"),
            }
        }
    }
    out.push_str("-1,0\r\nEOD\r\n");
    Ok(bo_encoding::codepage::encode_text(&out))
}

fn string(text: &str, out: &mut String) {
    out.push_str("1,0\r\n\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '\r' => {}
            '\n' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push_str("\"\r\n");
}

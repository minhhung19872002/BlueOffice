//! CSV import/export (RFC 4180) for a single worksheet.
//!
//! * Fields are separated by [`CsvOptions::delimiter`] (`,` by default, `;` for
//!   locales such as vi-VN whose decimal separator is `,`).
//! * Fields containing the delimiter, `"`, CR or LF are quoted; `"` is doubled.
//! * A UTF-8 byte order mark is skipped on read and optionally written.
//! * Records end with CRLF on write; CRLF, LF and CR are accepted on read.

use crate::{Result, XlsxError};
use bo_model::sheet::{Cell, CellValue, Worksheet, MAX_COLS, MAX_ROWS};
use std::fmt::Write as _;

const BOM: &str = "\u{FEFF}";

/// CSV dialect options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvOptions {
    /// Field delimiter.
    pub delimiter: char,
    /// Decimal separator used for numbers (read and write).
    pub decimal: char,
    /// Write a UTF-8 BOM (helps spreadsheet applications detect UTF-8).
    pub write_bom: bool,
    /// Convert numeric / boolean looking fields to numbers / booleans on read.
    pub detect_types: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            delimiter: ',',
            decimal: '.',
            write_bom: false,
            detect_types: true,
        }
    }
}

impl CsvOptions {
    /// Vietnamese convention: `;` delimiter, `,` decimal separator, UTF-8 BOM.
    pub fn vi_vn() -> Self {
        CsvOptions {
            delimiter: ';',
            decimal: ',',
            write_bom: true,
            detect_types: true,
        }
    }
}

/// Parse CSV bytes (UTF-8, optional BOM) into a worksheet named `Sheet1`.
pub fn read_csv(bytes: &[u8], opts: &CsvOptions) -> Result<Worksheet> {
    let text = std::str::from_utf8(bytes).map_err(|_| XlsxError::CsvEncoding)?;
    let text = text.strip_prefix(BOM).unwrap_or(text);
    let mut ws = Worksheet::new("Sheet1");
    let (mut row, mut col) = (0u32, 0u32);
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut pending = false; // a field (possibly empty) has started on this record

    let commit =
        |ws: &mut Worksheet, row: u32, col: u32, field: &mut String, quoted: bool| -> Result<()> {
            if row >= MAX_ROWS || col >= MAX_COLS {
                return Err(XlsxError::CsvLimit(format!("cell ({row}, {col})")));
            }
            if !field.is_empty() {
                let value = if quoted || !opts.detect_types {
                    CellValue::Text(std::mem::take(field))
                } else {
                    detect(field, opts)
                };
                ws.cells.insert((row, col), Cell::new(value));
            }
            field.clear();
            Ok(())
        };

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
                pending = true;
            }
            c if c == opts.delimiter => {
                commit(&mut ws, row, col, &mut field, quoted)?;
                quoted = false;
                col += 1;
                pending = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                commit(&mut ws, row, col, &mut field, quoted)?;
                quoted = false;
                row += 1;
                col = 0;
                pending = false;
            }
            _ => {
                field.push(c);
                pending = true;
            }
        }
    }
    if pending || !field.is_empty() {
        commit(&mut ws, row, col, &mut field, quoted)?;
    }
    Ok(ws)
}

fn detect(field: &str, opts: &CsvOptions) -> CellValue {
    match field {
        "TRUE" | "true" => return CellValue::Bool(true),
        "FALSE" | "false" => return CellValue::Bool(false),
        _ => {}
    }
    let t = field.trim();
    let looks_numeric = !t.is_empty()
        && t.chars().all(|c| {
            c.is_ascii_digit() || c == opts.decimal || matches!(c, '+' | '-' | 'e' | 'E')
        })
        && t.chars().any(|c| c.is_ascii_digit())
        // Keep identifiers with leading zeros ("007", phone numbers) as text.
        && !(t.len() > 1 && t.starts_with('0') && !t[1..].starts_with(opts.decimal));
    if looks_numeric {
        let norm: String = t
            .chars()
            .map(|c| if c == opts.decimal { '.' } else { c })
            .collect();
        if let Ok(n) = norm.parse::<f64>() {
            if n.is_finite() {
                return CellValue::Number(n);
            }
        }
    }
    CellValue::Text(field.to_owned())
}

/// Serialize the cells of a worksheet (cached values, not formulas) as CSV.
/// The output covers rows/columns from `A1` to the bottom-right used cell.
pub fn write_csv(ws: &Worksheet, opts: &CsvOptions) -> Vec<u8> {
    let mut out = String::new();
    if opts.write_bom {
        out.push_str(BOM);
    }
    let Some(range) = ws.used_range() else {
        return out.into_bytes();
    };
    for row in 0..=range.last.row {
        for col in 0..=range.last.col {
            if col > 0 {
                out.push(opts.delimiter);
            }
            if let Some(cell) = ws.cell(row, col) {
                let s = match &cell.value {
                    CellValue::Empty => String::new(),
                    CellValue::Number(n) => {
                        let s = number_text(*n);
                        if opts.decimal == '.' {
                            s
                        } else {
                            s.replace('.', &opts.decimal.to_string())
                        }
                    }
                    CellValue::Text(_) | CellValue::RichText(_) => {
                        cell.value.as_text().unwrap_or_default()
                    }
                    CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
                    CellValue::Error(e) => e.as_str().to_owned(),
                    CellValue::Date(d) => d.clone(),
                };
                write_field(&mut out, &s, opts.delimiter);
            }
        }
        out.push_str("\r\n");
    }
    out.into_bytes()
}

fn number_text(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn write_field(out: &mut String, s: &str, delim: char) {
    if s.contains(delim) || s.contains(['"', '\r', '\n']) || s.starts_with(' ') || s.ends_with(' ')
    {
        out.push('"');
        let _ = write!(out, "{}", s.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4180_quoting() {
        let src = "a,\"b,c\",\"d\"\"e\"\r\n1,2.5,\"line\nbreak\"\r\n";
        let ws = read_csv(src.as_bytes(), &CsvOptions::default()).unwrap();
        assert_eq!(ws.cell(0, 1).unwrap().value, CellValue::Text("b,c".into()));
        assert_eq!(ws.cell(0, 2).unwrap().value, CellValue::Text("d\"e".into()));
        assert_eq!(ws.cell(1, 0).unwrap().value, CellValue::Number(1.0));
        assert_eq!(ws.cell(1, 1).unwrap().value, CellValue::Number(2.5));
        assert_eq!(
            ws.cell(1, 2).unwrap().value,
            CellValue::Text("line\nbreak".into())
        );
        let back = String::from_utf8(write_csv(&ws, &CsvOptions::default())).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn vi_vn_dialect_with_bom() {
        let opts = CsvOptions::vi_vn();
        let src = "\u{FEFF}Tên;Số tiền\r\nNguyễn Văn A;1234,5\r\nMã;007\r\n";
        let ws = read_csv(src.as_bytes(), &opts).unwrap();
        assert_eq!(
            ws.cell(1, 0).unwrap().value,
            CellValue::Text("Nguyễn Văn A".into())
        );
        assert_eq!(ws.cell(1, 1).unwrap().value, CellValue::Number(1234.5));
        // Leading zeros stay text.
        assert_eq!(ws.cell(2, 1).unwrap().value, CellValue::Text("007".into()));
        let out = String::from_utf8(write_csv(&ws, &opts)).unwrap();
        assert_eq!(out, src);
    }

    #[test]
    fn empty_fields_and_no_trailing_newline() {
        let ws = read_csv(b"a,,c\n,x", &CsvOptions::default()).unwrap();
        assert!(ws.cell(0, 1).is_none());
        assert_eq!(ws.cell(1, 1).unwrap().value, CellValue::Text("x".into()));
        assert_eq!(ws.cells.len(), 3);
        let ws = read_csv(b"TRUE,false,-1e3", &CsvOptions::default()).unwrap();
        assert_eq!(ws.cell(0, 0).unwrap().value, CellValue::Bool(true));
        assert_eq!(ws.cell(0, 2).unwrap().value, CellValue::Number(-1000.0));
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        assert!(matches!(
            read_csv(&[0xff, 0xfe, 0x00], &CsvOptions::default()),
            Err(XlsxError::CsvEncoding)
        ));
    }
}

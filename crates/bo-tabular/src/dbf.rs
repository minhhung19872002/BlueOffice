//! dBASE tables (`.dbf`), read only: dBASE III / IV, FoxBASE+, FoxPro and Visual
//! FoxPro files, from the publicly documented table file structure.
//!
//! * Header (32 bytes): byte 0 the file type, bytes 4–7 the number of records,
//!   8–9 the length of the header, 10–11 the length of a record, byte 29 the
//!   language driver id (the code page of the text).
//! * Field descriptors (32 bytes each) up to a `0x0D` terminator: an 11-byte
//!   name, the type at byte 11, the length at 16 and the decimal count at 17.
//! * Records of fixed length: a deletion flag (`*` = deleted, skipped here)
//!   followed by the fields as text, or as binary for the Visual FoxPro types.
//!
//! Field types: `C` character, `N` / `F` number, `D` date (`YYYYMMDD`), `L`
//! logical; of Visual FoxPro tables also `I` integer, `B` double, `Y` currency
//! and `T` date-time. Memo, general and picture fields (`M`, `G`, `P`, whose data
//! is in a separate memo file) are left empty.
//!
//! The table becomes one sheet: field names in the first row, one row per record.

use crate::text::{chars_to_width, single_sheet};
use crate::{Result, TabularError};
use bo_model::sheet::{Cell, CellValue, CellXf, ColInfo, Workbook, Worksheet, MAX_COLS, MAX_ROWS};

/// File types read (byte 0): dBASE III / FoxBASE+ (`0x03`), with a memo file
/// (`0x83`), dBASE IV with a memo file (`0x8B`, `0xCB`), dBASE IV SQL tables
/// (`0x43`, `0x63`), FoxPro 2 with a memo file (`0xF5`), FoxBASE (`0xFB`) and
/// Visual FoxPro (`0x30`–`0x32`).
const TYPES: [u8; 11] = [
    0x03, 0x30, 0x31, 0x32, 0x43, 0x63, 0x83, 0x8B, 0xCB, 0xF5, 0xFB,
];

/// Windows / DOS code page of a language driver id (header byte 29), from the
/// published table of the dBASE and FoxPro file structure.
#[must_use]
pub fn code_page(language_driver: u8) -> Option<u16> {
    Some(match language_driver {
        0x01 | 0x09 | 0x0B | 0x0D | 0x0F | 0x11 | 0x15 | 0x18 | 0x19 | 0x1B => 437,
        0x02 | 0x0A | 0x0E | 0x10 | 0x12 | 0x14 | 0x16 | 0x1A | 0x1D | 0x25 | 0x37 => 850,
        0x03 | 0x57 | 0x58 | 0x59 => 1252,
        0x08 | 0x17 | 0x66 => 865,
        0x1C | 0x6C => 863,
        0x1F | 0x22 | 0x23 | 0x40 | 0x64 | 0x87 => 852,
        0x24 => 860,
        0x26 | 0x65 => 866,
        0x67 => 861,
        0x6A | 0x86 => 737,
        0x6B | 0x88 => 857,
        0x50 | 0x7C => 874,
        0x13 | 0x7B => 932,
        0x4D | 0x7A => 936,
        0x4E | 0x79 => 949,
        0x4F | 0x78 => 950,
        0x7D => 1255,
        0x7E => 1256,
        0xC8 => 1250,
        0xC9 => 1251,
        0xCA => 1254,
        0xCB => 1253,
        0xCC => 1257,
        _ => return None,
    })
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
    ])))
}

/// The bytes carry a plausible dBASE table header.
#[must_use]
pub fn is_dbf(bytes: &[u8]) -> bool {
    let (Some(kind), Some(header), Some(record)) =
        (bytes.first(), u16_at(bytes, 8), u16_at(bytes, 10))
    else {
        return false;
    };
    TYPES.contains(kind)
        && header >= 33
        && header <= bytes.len()
        && record >= 1
        // The field descriptors end with 0x0D inside the header.
        && (32..header)
            .step_by(32)
            .any(|at| bytes.get(at) == Some(&0x0D))
}

struct Field {
    name: String,
    kind: u8,
    /// Offset in the record (after the deletion flag) and length.
    at: usize,
    len: usize,
    decimals: u8,
    /// Sheet column; `None` for fields that are not shown (null flags).
    col: Option<u32>,
}

/// Text of the table's code page; without one, UTF-8 when valid and Windows-1252
/// otherwise.
fn text(bytes: &[u8], cp: Option<u16>) -> String {
    cp.and_then(|cp| bo_encoding::codepage::decode(bytes, cp))
        .unwrap_or_else(|| bo_encoding::codepage::decode_text(bytes))
}

/// Days from 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Serial date of the 1900 date system for a day count from 1970-01-01: day 1 is
/// 1900-01-01 and serial 60 is the 29 February 1900 that never was (ECMA-376
/// Part 1, §18.17.4.1), so later days are one further.
fn serial(days: i64) -> Option<f64> {
    let n = days + 25_568; // 1899-12-31 is 25 568 days before 1970-01-01
    let n = if n >= 60 { n + 1 } else { n };
    // Serial dates end at 9999-12-31 (2 958 465).
    #[allow(clippy::cast_precision_loss)]
    (1..=2_958_465).contains(&n).then_some(n as f64)
}

/// `YYYYMMDD` → serial date.
fn date(raw: &[u8]) -> Option<f64> {
    if raw.len() != 8 || !raw.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let s = std::str::from_utf8(raw).ok()?;
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: i64 = s.get(4..6)?.parse().ok()?;
    let d: i64 = s.get(6..8)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    serial(days_from_civil(y, m, d))
}

fn le<const N: usize>(raw: &[u8]) -> Option<[u8; N]> {
    raw.get(..N)?.try_into().ok()
}

/// Read a dBASE table into a workbook with one sheet.
#[allow(clippy::too_many_lines)]
pub fn read_dbf(bytes: &[u8]) -> Result<Workbook> {
    let bad = |m: &str| TabularError::Dbf(m.to_owned());
    if !is_dbf(bytes) {
        return Err(bad("no table header"));
    }
    let kind = bytes[0];
    let visual = matches!(kind, 0x30..=0x32);
    let header = u16_at(bytes, 8).ok_or_else(|| bad("truncated header"))?;
    let record = u16_at(bytes, 10).ok_or_else(|| bad("truncated header"))?;
    let claimed = bytes
        .get(4..8)
        .and_then(|b| b.try_into().ok())
        .map_or(0, u32::from_le_bytes) as usize;
    let cp = bytes.get(29).copied().and_then(code_page);

    // Field descriptors.
    let mut fields: Vec<Field> = Vec::new();
    let mut at = 1usize; // the deletion flag comes first
    let mut cols = 0u32;
    let mut off = 32usize;
    while off + 32 <= header && bytes.get(off) != Some(&0x0D) {
        let Some(d) = bytes.get(off..off + 32) else {
            break;
        };
        let raw_name = d[..11].split(|b| *b == 0).next().unwrap_or(&[]);
        let name = text(raw_name, cp).trim().to_owned();
        let kind = d[11].to_ascii_uppercase();
        // A character field longer than 255 keeps the high byte of its length
        // where other types keep the decimal count.
        let (len, decimals) = if kind == b'C' {
            (usize::from(d[16]) | usize::from(d[17]) << 8, 0)
        } else {
            (usize::from(d[16]), d[17])
        };
        if at + len > record {
            return Err(bad("the fields are longer than a record"));
        }
        // `0`: the null flags of Visual FoxPro, not data.
        let col = (kind != b'0').then_some(cols);
        if col.is_some() {
            cols += 1;
            if cols > MAX_COLS {
                return Err(TabularError::Limit(format!("{cols} fields")));
            }
        }
        fields.push(Field {
            name,
            kind,
            at,
            len,
            decimals,
            col,
        });
        at += len;
        off += 32;
    }
    if fields.is_empty() {
        return Err(bad("no fields"));
    }

    let mut wb = single_sheet(Worksheet::new("Sheet1"));
    // Formats: dates (built-in 14, the short date), date-times, fixed decimals.
    let xf = |wb: &mut Workbook, num_fmt_id: u32| {
        wb.styles.add_xf(CellXf {
            num_fmt_id,
            xf_id: Some(0),
            apply_number_format: Some(true),
            ..CellXf::default()
        })
    };
    let date_xf = xf(&mut wb, 14);
    let stamp = wb.styles.add_num_fmt("yyyy-mm-dd hh:mm:ss");
    let stamp_xf = xf(&mut wb, stamp);
    let mut decimal_xf = std::collections::HashMap::new();
    for f in &fields {
        let n = match f.kind {
            b'N' | b'F' | b'B' => f.decimals.min(15),
            b'Y' => 4,
            _ => 0,
        };
        if n > 0 && !decimal_xf.contains_key(&n) {
            let id = if n == 2 {
                2 // built-in `0.00`
            } else {
                wb.styles
                    .add_num_fmt(&format!("0.{}", "0".repeat(usize::from(n))))
            };
            decimal_xf.insert(n, xf(&mut wb, id));
        }
    }

    let available = bytes.len().saturating_sub(header) / record;
    let count = claimed.min(available);
    if count >= MAX_ROWS as usize {
        return Err(TabularError::Limit(format!("{count} records")));
    }
    let mut ws = Worksheet::new("Sheet1");
    for f in &fields {
        let Some(col) = f.col else { continue };
        if !f.name.is_empty() {
            ws.cells
                .insert((0, col), Cell::new(CellValue::Text(f.name.clone())));
        }
        let chars = f.len.max(f.name.chars().count()).clamp(4, 60);
        ws.cols.push(ColInfo {
            min: col,
            max: col,
            #[allow(clippy::cast_precision_loss)]
            width: Some(chars_to_width(chars as f64)),
            custom_width: true,
            ..ColInfo::default()
        });
    }
    let mut row = 1u32;
    for i in 0..count {
        let Some(rec) = bytes.get(header + i * record..header + (i + 1) * record) else {
            break;
        };
        match rec.first() {
            Some(b'*') => continue, // deleted
            Some(0x1A) => break,    // end of file
            _ => {}
        }
        for f in &fields {
            let (Some(col), Some(raw)) = (f.col, rec.get(f.at..f.at + f.len)) else {
                continue;
            };
            let ascii = || std::str::from_utf8(raw).unwrap_or("").trim();
            let fixed = |n: u8| decimal_xf.get(&n).copied().unwrap_or(0);
            let (value, style) = match f.kind {
                b'C' => {
                    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
                    let s = text(&raw[..end], cp);
                    let s = s.trim_end();
                    if s.is_empty() {
                        continue;
                    }
                    (CellValue::Text(s.to_owned()), 0)
                }
                b'N' | b'F' => match ascii().parse::<f64>() {
                    Ok(n) if n.is_finite() => (CellValue::Number(n), fixed(f.decimals.min(15))),
                    _ => continue,
                },
                b'D' => match date(raw) {
                    Some(n) => (CellValue::Number(n), date_xf),
                    None => continue,
                },
                b'L' => match raw.first() {
                    Some(b'T' | b't' | b'Y' | b'y') => (CellValue::Bool(true), 0),
                    Some(b'F' | b'f' | b'N' | b'n') => (CellValue::Bool(false), 0),
                    _ => continue,
                },
                b'I' if f.len == 4 => match le::<4>(raw) {
                    Some(b) => (CellValue::Number(f64::from(i32::from_le_bytes(b))), 0),
                    None => continue,
                },
                b'B' if visual && f.len == 8 => match le::<8>(raw).map(f64::from_le_bytes) {
                    Some(n) if n.is_finite() => (CellValue::Number(n), fixed(f.decimals.min(15))),
                    _ => continue,
                },
                // Currency: a 64-bit integer of 1/10 000 units.
                b'Y' if f.len == 8 => match le::<8>(raw) {
                    #[allow(clippy::cast_precision_loss)]
                    Some(b) => (
                        CellValue::Number(i64::from_le_bytes(b) as f64 / 10_000.0),
                        fixed(4),
                    ),
                    None => continue,
                },
                // Date-time: the Julian day number, then milliseconds since midnight.
                b'T' if f.len == 8 => {
                    let (Some(day), Some(ms)) = (le::<4>(raw), le::<4>(&raw[4..])) else {
                        continue;
                    };
                    let (day, ms) = (u32::from_le_bytes(day), u32::from_le_bytes(ms));
                    // Julian day 2 440 588 is 1970-01-01.
                    match serial(i64::from(day) - 2_440_588) {
                        Some(n) if ms < 86_400_000 => (
                            CellValue::Number(n + f64::from(ms) / 86_400_000.0),
                            stamp_xf,
                        ),
                        _ => continue,
                    }
                }
                // Memo, general, picture, binary and unknown types.
                _ => continue,
            };
            ws.cells
                .insert((row, col), Cell::new(value).with_style(style));
        }
        row += 1;
    }
    ws.sheet_id = 1;
    wb.sheets = vec![ws];
    Ok(wb)
}

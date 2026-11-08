//! Intermediate spreadsheet types shared by the `.xls` and `.xlsb` readers.
//!
//! These are deliberately simple; mapping onto the BlueSheet model is done by
//! the integration layer.

/// A workbook.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Workbook {
    /// Worksheets in tab order.
    pub sheets: Vec<Worksheet>,
    /// Dates use the 1904 system ([MS-XLS] §2.4.77 Date1904 / [MS-XLSB] BrtWbProp).
    pub date1904: bool,
    /// Defined names (workbook and sheet scope).
    pub names: Vec<DefinedName>,
    /// The file carries VBA or XLM macros (preserved, never executed).
    pub has_macros: bool,
}

/// A defined name ([MS-XLS] §2.4.150 Lbl / [MS-XLSB] BrtName).
#[derive(Debug, Clone, PartialEq)]
pub struct DefinedName {
    /// Name as written in the file.
    pub name: String,
    /// Formula text without `=`, when it could be decoded.
    pub formula: Option<String>,
    /// 0-based sheet index for sheet-scoped names.
    pub sheet: Option<u32>,
}

/// A worksheet (chart sheets and macro sheets are skipped).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Worksheet {
    /// Sheet tab name.
    pub name: String,
    /// Whether the sheet is hidden (`hsState` ≠ visible).
    pub hidden: bool,
    /// Cells in file order (row-major in practice).
    pub cells: Vec<Cell>,
}

impl Worksheet {
    /// The sheet as RFC 4180 CSV of the cached values (`\n` line ends). Each
    /// row ends at its last non-empty cell; missing rows are empty lines.
    pub fn to_csv(&self) -> String {
        let mut cells: Vec<&Cell> = self
            .cells
            .iter()
            .filter(|c| c.value != CellValue::Empty)
            .collect();
        cells.sort_by_key(|c| (c.row, c.col));
        let mut out = String::new();
        let mut row = 0u32;
        let mut col = 0u32;
        for c in cells {
            while row < c.row {
                out.push('\n');
                row += 1;
                col = 0;
            }
            if c.col < col {
                continue; // duplicate cell
            }
            // `col` fields are already written on this row.
            let commas = if col == 0 { c.col } else { c.col - col + 1 };
            for _ in 0..commas {
                out.push(',');
            }
            let text = match &c.value {
                CellValue::Empty => String::new(),
                CellValue::Number(v) => crate::xls_formula::fmt_number(*v),
                CellValue::Text(t) => t.clone(),
                CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
                CellValue::Error(e) => e.clone(),
            };
            if text.contains([',', '"', '\n', '\r']) {
                out.push('"');
                out.push_str(&text.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(&text);
            }
            col = c.col + 1;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }
}

/// A cell (0-based row/column).
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// 0-based row index.
    pub row: u32,
    /// 0-based column index.
    pub col: u32,
    /// Value, or the cached result for formula cells.
    pub value: CellValue,
    /// Formula text without the leading `=` (A1 style), when available.
    pub formula: Option<String>,
    /// Number format code when not "General".
    pub number_format: Option<String>,
}

/// A cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// No value.
    Empty,
    /// Numeric value (dates are serial numbers).
    Number(f64),
    /// String value.
    Text(String),
    /// Boolean value.
    Bool(bool),
    /// Error literal such as `#DIV/0!`.
    Error(String),
}

/// Error literal for a BErr / error code ([MS-XLS] §2.5.97.2 BErr).
pub fn error_text(code: u8) -> String {
    match code {
        0x00 => "#NULL!",
        0x07 => "#DIV/0!",
        0x0F => "#VALUE!",
        0x17 => "#REF!",
        0x1D => "#NAME?",
        0x24 => "#NUM!",
        0x2A => "#N/A",
        0x2B => "#GETTING_DATA",
        _ => "#ERR!",
    }
    .to_owned()
}

/// Decode an RkNumber ([MS-XLS] §2.5.217, [MS-XLSB] §2.5.122).
pub fn rk_to_f64(rk: u32) -> f64 {
    let x100 = rk & 1 != 0;
    let v = if rk & 2 != 0 {
        f64::from((rk as i32) >> 2)
    } else {
        f64::from_bits(u64::from(rk & 0xFFFF_FFFC) << 32)
    };
    if x100 {
        v / 100.0
    } else {
        v
    }
}

/// Built-in number formats (ECMA-376 Part 1, §18.8.30 numFmt; [MS-XLS] §2.4.126).
pub fn builtin_format(id: u16) -> Option<&'static str> {
    Some(match id {
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "mm-dd-yy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yy h:mm",
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mmss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

/// Column letters (0 → `A`).
pub fn col_name(mut col: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (col % 26) as u8);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    s.reverse();
    String::from_utf8_lossy(&s).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rk_numbers() {
        // [MS-XLS] §2.5.217 examples: integer 1 with fInt, 1.0 as IEEE high bits.
        assert_eq!(rk_to_f64((1 << 2) | 2), 1.0);
        assert_eq!(rk_to_f64(((-5i32 << 2) as u32) | 2), -5.0);
        assert_eq!(rk_to_f64((1234 << 2) | 3), 12.34);
        let hi = (1.5f64.to_bits() >> 32) as u32;
        assert_eq!(rk_to_f64(hi), 1.5);
        assert_eq!(rk_to_f64(hi | 1), 0.015);
    }

    #[test]
    fn columns() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(255), "IV");
        assert_eq!(col_name(16_383), "XFD");
    }
}

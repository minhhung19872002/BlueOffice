//! Helpers shared by the text formats.

use bo_model::sheet::{CellValue, Workbook, Worksheet};

/// Lines of `text`, ending at CR LF, LF or CR.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .flat_map(|l| l.strip_suffix('\r').unwrap_or(l).split('\r'))
}

/// A workbook holding just `ws`.
pub(crate) fn single_sheet(ws: Worksheet) -> Workbook {
    let mut wb = Workbook::new();
    let mut ws = ws;
    ws.sheet_id = 1;
    wb.sheets = vec![ws];
    wb
}

/// Shortest text of a number that reads back as the same number: integers
/// without a fraction, everything else as Rust prints it (never an exponent).
pub(crate) fn number_text(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        // In range: the cast is exact.
        #[allow(clippy::cast_possible_truncation)]
        let i = n as i64;
        i.to_string()
    } else {
        format!("{n}")
    }
}

/// A number written in a text file: digits with an optional sign, one decimal
/// separator and an exponent. Identifiers with leading zeros (`007`) are not
/// numbers.
pub(crate) fn parse_number(field: &str, decimal: char) -> Option<f64> {
    let t = field.trim();
    let digits = t.chars().any(|c| c.is_ascii_digit());
    let shape = t
        .chars()
        .all(|c| c.is_ascii_digit() || c == decimal || matches!(c, '+' | '-' | 'e' | 'E'));
    let unsigned = t.trim_start_matches(['+', '-']);
    let leading_zero =
        unsigned.len() > 1 && unsigned.starts_with('0') && !unsigned[1..].starts_with(decimal);
    if !digits || !shape || leading_zero {
        return None;
    }
    let norm: String = t
        .chars()
        .map(|c| if c == decimal { '.' } else { c })
        .collect();
    norm.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The typed value of a field of plain text: a number, a boolean, or the text.
pub(crate) fn detect_value(field: &str, decimal: char) -> CellValue {
    match field {
        "TRUE" | "true" => return CellValue::Bool(true),
        "FALSE" | "false" => return CellValue::Bool(false),
        _ => {}
    }
    match parse_number(field, decimal) {
        Some(n) => CellValue::Number(n),
        None => CellValue::Text(field.to_owned()),
    }
}

/// Column width of `n` characters as the model stores it: with the padding of the
/// default font (ECMA-376 Part 1, §18.3.1.13 `col`, attribute `width`).
pub(crate) fn chars_to_width(n: f64) -> f64 {
    ((n * 7.0 + 5.0) / 7.0 * 256.0).trunc() / 256.0
}

/// Whole characters of a stored column width (inverse of [`chars_to_width`]).
pub(crate) fn width_to_chars(width: f64) -> u32 {
    let n = (width - 5.0 / 7.0).round().clamp(0.0, 255.0);
    // Clamped to 0..=255 above.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = n as u32;
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings() {
        let v: Vec<&str> = lines("a\r\nb\nc\rd").collect();
        assert_eq!(v, ["a", "b", "c", "d"]);
        assert_eq!(lines("").collect::<Vec<_>>(), [""]);
    }

    #[test]
    fn numbers_and_text() {
        assert_eq!(parse_number("1.5", '.'), Some(1.5));
        assert_eq!(parse_number(" -2,25 ", ','), Some(-2.25));
        assert_eq!(parse_number("1e3", '.'), Some(1000.0));
        assert_eq!(parse_number("007", '.'), None);
        assert_eq!(parse_number("-007", '.'), None);
        assert_eq!(parse_number("0.5", '.'), Some(0.5));
        assert_eq!(parse_number("1.2.3", '.'), None);
        assert_eq!(parse_number("e", '.'), None);
        assert_eq!(parse_number("12a", '.'), None);
        assert_eq!(detect_value("TRUE", '.'), CellValue::Bool(true));
        assert_eq!(detect_value("x 1", '.'), CellValue::Text("x 1".into()));
        assert_eq!(number_text(3.0), "3");
        assert_eq!(number_text(-0.25), "-0.25");
        assert_eq!(number_text(1e20), "100000000000000000000");
    }
}

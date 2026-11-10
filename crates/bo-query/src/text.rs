//! Text decoding (encoding detection) and delimited text (CSV) sources.
//!
//! * Encodings: a byte order mark (UTF-8, UTF-16LE/BE, Unicode Standard §23.8) wins; valid
//!   UTF-8 is UTF-8; text with a zero byte in every other position is UTF-16 without BOM;
//!   anything else is Windows-1252, then checked for the legacy Vietnamese encodings TCVN3
//!   and VNI-Windows (`bo_encoding`).
//! * Delimiters: the candidate (`,` `;` tab `|`) found the same number of times (outside
//!   quotes) on most of the first lines.
//! * Fields are split by the RFC 4180 reader of `bo_xlsx` (quotes, doubled quotes, CR/LF).

use crate::value::{QTable, Val};
use crate::QueryError;
use bo_model::sheet::CellValue;

/// Decoded text and the encoding it was read with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text.
    pub text: String,
    /// Encoding name (`utf-8`, `utf-16le`, `utf-16be`, `windows-1252`, `tcvn3`, `vni`).
    pub encoding: String,
}

/// Decode `bytes` with `encoding` (absent: detected).
pub fn decode(bytes: &[u8], encoding: Option<&str>) -> Decoded {
    let enc = encoding.map_or_else(|| detect_encoding(bytes), str::to_ascii_lowercase);
    let text = match enc.as_str() {
        "utf-16le" | "utf-16" => utf16(bytes, true),
        "utf-16be" => utf16(bytes, false),
        "windows-1252" | "latin1" | "cp1252" => cp1252(bytes),
        "tcvn3" => bo_encoding::tcvn3_to_unicode(&cp1252(bytes), false),
        "vni" => bo_encoding::vni_to_unicode(&cp1252(bytes)),
        _ => {
            let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            String::from_utf8_lossy(b).into_owned()
        }
    };
    Decoded {
        text: text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_owned(),
        encoding: enc,
    }
}

/// The encoding of `bytes` (see the module documentation).
pub fn detect_encoding(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return "utf-8".into();
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return "utf-16le".into();
    }
    if bytes.starts_with(b"\xFE\xFF") {
        return "utf-16be".into();
    }
    if bytes.len() >= 4 {
        let sample = &bytes[..bytes.len().min(512) & !1];
        let even = sample.iter().step_by(2).filter(|&&b| b == 0).count();
        let odd = sample
            .iter()
            .skip(1)
            .step_by(2)
            .filter(|&&b| b == 0)
            .count();
        let half = sample.len() / 2;
        if odd * 10 >= half * 3 && even == 0 {
            return "utf-16le".into();
        }
        if even * 10 >= half * 3 && odd == 0 {
            return "utf-16be".into();
        }
    }
    if std::str::from_utf8(bytes).is_ok() {
        return "utf-8".into();
    }
    let latin = cp1252(bytes);
    match bo_encoding::detect_legacy_vietnamese(&latin) {
        Some(bo_encoding::Legacy::Tcvn3) => "tcvn3".into(),
        Some(bo_encoding::Legacy::Vni) => "vni".into(),
        None => "windows-1252".into(),
    }
}

fn cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| bo_encoding::cp1252::decode(b))
        .collect()
}

fn utf16(bytes: &[u8], le: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// The delimiter of delimited `text` (`,` when nothing fits better).
pub fn detect_delimiter(text: &str) -> char {
    const CANDIDATES: [char; 4] = [',', ';', '\t', '|'];
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(20)
        .collect();
    let mut best = (',', 0usize, 0usize);
    for d in CANDIDATES {
        let counts: Vec<usize> = lines.iter().map(|l| count_outside_quotes(l, d)).collect();
        let Some(&first) = counts.first() else {
            continue;
        };
        if first == 0 {
            continue;
        }
        let same = counts.iter().filter(|&&c| c == first).count();
        if (same, first) > (best.1, best.2) {
            best = (d, same, first);
        }
    }
    best.0
}

fn count_outside_quotes(line: &str, d: char) -> usize {
    let mut quoted = false;
    let mut n = 0;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if c == d && !quoted {
            n += 1;
        }
    }
    n
}

/// What a CSV source was read with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvInfo {
    /// Encoding used.
    pub encoding: String,
    /// Delimiter used.
    pub delimiter: char,
}

/// Read delimited text as a table of text values (`Column1`, `Column2`, …).
pub fn read_csv(
    bytes: &[u8],
    delimiter: Option<&str>,
    encoding: Option<&str>,
) -> Result<(QTable, CsvInfo), QueryError> {
    let decoded = decode(bytes, encoding);
    let delimiter = match delimiter.map(|d| if d == "\\t" { "\t" } else { d }) {
        Some(d) if d.chars().count() == 1 => d.chars().next().unwrap_or(','),
        Some(d) if d.eq_ignore_ascii_case("tab") => '\t',
        _ => detect_delimiter(&decoded.text),
    };
    let opts = bo_xlsx::CsvOptions {
        delimiter,
        decimal: '.',
        write_bom: false,
        detect_types: false,
    };
    let ws = bo_xlsx::read_csv(decoded.text.as_bytes(), &opts)
        .map_err(|e| QueryError::Source(e.to_string()))?;
    let (mut nrows, mut ncols) = (0u32, 0u32);
    for &(r, c) in ws.cells.keys() {
        nrows = nrows.max(r + 1);
        ncols = ncols.max(c + 1);
    }
    let mut rows = vec![vec![Val::Null; ncols as usize]; nrows as usize];
    for (&(r, c), cell) in &ws.cells {
        rows[r as usize][c as usize] = cell_val(&cell.value);
    }
    Ok((
        QTable::from_rows(rows),
        CsvInfo {
            encoding: decoded.encoding,
            delimiter,
        },
    ))
}

/// A worksheet cell value as a query value.
pub fn cell_val(v: &CellValue) -> Val {
    match v {
        CellValue::Empty => Val::Null,
        CellValue::Number(n) => Val::Num(*n),
        CellValue::Text(_) | CellValue::RichText(_) => Val::Text(v.as_text().unwrap_or_default()),
        CellValue::Bool(b) => Val::Bool(*b),
        CellValue::Error(e) => Val::Err(error_text(e).into()),
        CellValue::Date(s) => {
            crate::value::parse_date(s, false).map_or_else(|| Val::Text(s.clone()), Val::Date)
        }
    }
}

fn error_text(e: &bo_model::sheet::CellError) -> &'static str {
    use bo_model::sheet::CellError as E;
    match e {
        E::Null => "#NULL!",
        E::Div0 => "#DIV/0!",
        E::Ref => "#REF!",
        E::Name => "#NAME?",
        E::Num => "#NUM!",
        E::NA => "#N/A",
        _ => "#VALUE!",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        assert_eq!(detect_encoding("Hà Nội".as_bytes()), "utf-8");
        let mut le = vec![0xFF, 0xFE];
        for u in "Hà".encode_utf16() {
            le.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode(&le, None).text, "Hà");
        let be: Vec<u8> = "ab,c".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(detect_encoding(&be), "utf-16be");
        assert_eq!(decode(&be, None).text, "ab,c");
        // "café" in Windows-1252.
        assert_eq!(decode(b"caf\xE9", None).text, "café");
        assert_eq!(decode(b"caf\xE9", None).encoding, "windows-1252");
    }

    #[test]
    fn delimiters() {
        assert_eq!(detect_delimiter("a;b;c\n1;2,5;3\n"), ';');
        assert_eq!(detect_delimiter("a,b\n\"x;y\",2\n"), ',');
        assert_eq!(detect_delimiter("a\tb\n1\t2\n"), '\t');
        let (t, info) = read_csv(b"a;b\n1;\"x;y\"\n", None, None).unwrap();
        assert_eq!(info.delimiter, ';');
        assert_eq!(t.columns, vec!["Column1", "Column2"]);
        assert_eq!(t.rows[1][1], Val::Text("x;y".into()));
    }
}

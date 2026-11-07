//! Mail-merge data sources: CSV (RFC 4180, `,` `;` or tab separated, UTF-8) and
//! the first visible sheet of an `.xlsx` workbook (ECMA-376 Part 1, §18), the
//! first row naming the fields.

use bo_edit::merge::MergeData;

/// Maximum number of records read from a data source.
pub const MAX_RECORDS: usize = 100_000;

/// Mail-merge state reported to hosts.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeState {
    pub name: String,
    pub fields: Vec<String>,
    pub count: usize,
    /// Record shown in the preview (0-based), if any.
    pub preview: Option<usize>,
}

/// Decode standard base64 (RFC 4648 §4; whitespace ignored, padding optional).
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in s.as_bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn table(rows: Vec<Vec<String>>, name: &str) -> MergeData {
    let mut it = rows.into_iter();
    let fields: Vec<String> = it
        .next()
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.trim().to_owned())
        .collect();
    let rows = it
        .filter(|r| r.iter().any(|v| !v.trim().is_empty()))
        .take(MAX_RECORDS)
        .collect();
    MergeData {
        name: name.to_owned(),
        fields,
        rows,
    }
}

/// Read a data source from bytes; the format comes from the content, then the name.
pub fn read_data(bytes: &[u8], name: &str) -> crate::Result<MergeData> {
    if bytes.starts_with(b"PK\x03\x04") {
        let wb = bo_xlsx::read_xlsx(bytes).map_err(|e| crate::EngineError::Sheet(e.to_string()))?;
        let ws = wb
            .sheets
            .iter()
            .find(|s| s.state == bo_model::sheet::SheetState::Visible)
            .or_else(|| wb.sheets.first())
            .ok_or_else(|| crate::EngineError::Sheet("empty workbook".into()))?;
        let max_col = ws.cells.keys().map(|k| k.1).max().unwrap_or(0);
        let max_row = ws
            .cells
            .keys()
            .map(|k| k.0)
            .max()
            .unwrap_or(0)
            .min(MAX_RECORDS as u32);
        let locale = bo_sheet::numfmt::Locale::VI_VN;
        let rows = (0..=max_row)
            .map(|r| {
                (0..=max_col)
                    .map(|c| {
                        ws.cell(r, c)
                            .map(|cell| bo_sheet::render::format_cell(&wb, cell, &locale).text)
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        return Ok(table(rows, name));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| crate::EngineError::Unsupported(format!("{name}: not UTF-8 text")))?;
    let first = text.lines().next().unwrap_or("");
    let delimiter = [',', ';', '\t']
        .into_iter()
        .max_by_key(|d| first.matches(*d).count())
        .unwrap_or(',');
    let ws = bo_xlsx::read_csv(
        text.as_bytes(),
        &bo_xlsx::CsvOptions {
            delimiter,
            detect_types: false,
            ..bo_xlsx::CsvOptions::default()
        },
    )
    .map_err(|e| crate::EngineError::Sheet(e.to_string()))?;
    let max_col = ws.cells.keys().map(|k| k.1).max().unwrap_or(0);
    let max_row = ws.cells.keys().map(|k| k.0).max().unwrap_or(0);
    let rows = (0..=max_row.min(MAX_RECORDS as u32))
        .map(|r| {
            (0..=max_col)
                .map(|c| {
                    ws.cell(r, c)
                        .map(|cell| match &cell.value {
                            bo_model::sheet::CellValue::Number(n) => n.to_string(),
                            v => v.as_text().unwrap_or_default(),
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    Ok(table(rows, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_and_csv() {
        assert_eq!(base64_decode("SMOgIE7hu5lp").unwrap(), "Hà Nội".as_bytes());
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
        assert!(base64_decode("a$").is_none());
        let d = read_data(
            "Họ tên;Số tiền\n\"Nguyễn Văn An\";1500000\n;\nTrần Thị Bình;2,5\n".as_bytes(),
            "ds.csv",
        )
        .unwrap();
        assert_eq!(d.fields, ["Họ tên", "Số tiền"]);
        assert_eq!(d.rows.len(), 2);
        assert_eq!(d.value(0, "họ tên"), Some("Nguyễn Văn An"));
        assert_eq!(d.value(1, "Số tiền"), Some("2,5"));
    }
}

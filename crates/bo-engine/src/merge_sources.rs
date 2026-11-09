//! Mail-merge data sources beyond CSV / XLSX: vCard address books (RFC 6350, and the
//! vCard 2.1 / 3.0 forms with `CHARSET` / `ENCODING=QUOTED-PRINTABLE` parameters,
//! RFC 2045 §6.7), JSON arrays (RFC 8259) and tables handed over by a host data
//! provider (`MergeDataProvider`: SQL runs on the server, never in the client).

use crate::merge::MAX_RECORDS;
use bo_edit::merge::MergeData;

// ---------------------------------------------------------------- vCard

/// Columns of a vCard data source (one record per `BEGIN:VCARD … END:VCARD`).
pub const VCARD_FIELDS: [&str; 18] = [
    "FullName",
    "LastName",
    "FirstName",
    "MiddleName",
    "Prefix",
    "Suffix",
    "Organization",
    "Title",
    "Email",
    "Phone",
    "Mobile",
    "Street",
    "City",
    "Region",
    "PostalCode",
    "Country",
    "Address",
    "Birthday",
];

/// Quoted-printable decoding (RFC 2045 §6.7): `=XX` octets, `=` soft line breaks.
fn quoted_printable(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let hex = |c: u8| (c as char).to_digit(16);
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            match (b.get(i + 1).copied(), b.get(i + 2).copied()) {
                (Some(h), Some(l)) if hex(h).is_some() && hex(l).is_some() => {
                    let v = hex(h).unwrap_or(0) * 16 + hex(l).unwrap_or(0);
                    out.push(u8::try_from(v).unwrap_or(b'?'));
                    i += 3;
                    continue;
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Value escapes of RFC 6350 §3.4: `\\` `\,` `\;` `\n`.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(x) => out.push(x),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split a structured value at unescaped `;` (RFC 6350 §3.3), unescaping each part.
fn components(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for c in s.chars() {
        if esc {
            cur.push('\\');
            cur.push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ';' {
            parts.push(unescape(&cur));
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    parts.push(unescape(&cur));
    parts
}

/// One content line: upper-case name (group removed), upper-case parameters, value.
fn content_line(line: &str) -> Option<(String, Vec<String>, String)> {
    // The value starts after the first `:` outside a quoted parameter value.
    let mut quoted = false;
    let mut colon = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts
        .next()?
        .rsplit('.')
        .next()?
        .trim()
        .to_ascii_uppercase();
    let params = parts.map(|p| p.trim().to_ascii_uppercase()).collect();
    Some((name, params, value.to_owned()))
}

/// Read a vCard address book; unknown properties are ignored.
pub fn read_vcard(bytes: &[u8], name: &str) -> MergeData {
    let text = String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    // Unfold (RFC 6350 §3.2): a line starting with a space or tab continues the previous.
    let mut lines: Vec<String> = Vec::new();
    for l in text.trim_start_matches('\u{feff}').split('\n') {
        match (l.chars().next(), lines.last_mut()) {
            (Some(' ' | '\t'), Some(prev)) => prev.push_str(&l[1..]),
            _ => lines.push(l.to_owned()),
        }
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut card: Option<Vec<String>> = None;
    let col = |f: &str| VCARD_FIELDS.iter().position(|x| *x == f).unwrap_or(0);
    let mut i = 0;
    while i < lines.len() {
        let mut line = lines[i].clone();
        i += 1;
        let Some((prop, params, mut value)) = content_line(&line) else {
            continue;
        };
        let qp = params
            .iter()
            .any(|p| p == "ENCODING=QUOTED-PRINTABLE" || p == "QUOTED-PRINTABLE");
        if qp {
            // vCard 2.1: a value ending in `=` continues on the next line.
            while value.ends_with('=') && i < lines.len() {
                value.pop();
                value.push_str(&lines[i]);
                line.push_str(&lines[i]);
                i += 1;
            }
            value = String::from_utf8_lossy(&quoted_printable(&value)).into_owned();
        }
        let types: Vec<&str> = params
            .iter()
            .flat_map(|p| {
                p.strip_prefix("TYPE=")
                    .unwrap_or(p)
                    .split(',')
                    .collect::<Vec<_>>()
            })
            .collect();
        match prop.as_str() {
            "BEGIN" if value.trim().eq_ignore_ascii_case("VCARD") => {
                card = Some(vec![String::new(); VCARD_FIELDS.len()]);
            }
            "END" if value.trim().eq_ignore_ascii_case("VCARD") => {
                if let Some(mut c) = card.take() {
                    let full = col("FullName");
                    if c[full].is_empty() {
                        // No FN: family, middle and given names (Vietnamese order).
                        c[full] = [
                            &c[col("LastName")],
                            &c[col("MiddleName")],
                            &c[col("FirstName")],
                        ]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    }
                    if c.iter().any(|v| !v.is_empty()) && rows.len() < MAX_RECORDS {
                        rows.push(c);
                    }
                }
            }
            _ => {
                let Some(c) = card.as_mut() else { continue };
                let mut set = |f: &str, v: &str| {
                    let k = col(f);
                    if c[k].is_empty() {
                        c[k] = v.trim().to_owned();
                    }
                };
                match prop.as_str() {
                    "FN" => set("FullName", &unescape(&value)),
                    // §6.2.2 N: family; given; additional; prefixes; suffixes.
                    "N" => {
                        let n = components(&value);
                        let get = |k: usize| n.get(k).cloned().unwrap_or_default();
                        set("LastName", &get(0));
                        set("FirstName", &get(1));
                        set("MiddleName", &get(2));
                        set("Prefix", &get(3));
                        set("Suffix", &get(4));
                    }
                    "ORG" => set(
                        "Organization",
                        &components(&value).first().cloned().unwrap_or_default(),
                    ),
                    "TITLE" => set("Title", &unescape(&value)),
                    "EMAIL" => set("Email", &unescape(&value)),
                    "TEL" => {
                        let v = unescape(&value);
                        let v = v.strip_prefix("tel:").unwrap_or(&v).to_owned();
                        if types.contains(&"CELL") {
                            set("Mobile", &v);
                        }
                        set("Phone", &v);
                    }
                    // §6.3.1 ADR: PO box; extended; street; locality; region; code; country.
                    "ADR" => {
                        let a = components(&value);
                        let get = |k: usize| a.get(k).cloned().unwrap_or_default();
                        let street = [get(0), get(1), get(2)]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(", ");
                        set("Street", &street);
                        set("City", &get(3));
                        set("Region", &get(4));
                        set("PostalCode", &get(5));
                        set("Country", &get(6));
                        let full = [street, get(3), get(4), get(5), get(6)]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(", ");
                        set("Address", &full);
                    }
                    "BDAY" => set("Birthday", &unescape(&value)),
                    _ => {}
                }
            }
        }
    }
    MergeData {
        name: name.to_owned(),
        fields: VCARD_FIELDS.iter().map(|s| (*s).to_owned()).collect(),
        rows,
    }
}

// ---------------------------------------------------------------- JSON

/// A JSON value keeping object members in source order.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> serde::Deserialize<'de> for Json {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_none<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Number(v.to_string()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Number(v.to_string()))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(Json::Number(v.to_string()))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::Text(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<Json, E> {
                Ok(Json::Text(v))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(Json::Array(v))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v = Vec::new();
                while let Some((k, x)) = a.next_entry::<String, Json>()? {
                    v.push((k, x));
                }
                Ok(Json::Object(v))
            }
        }
        d.deserialize_any(V)
    }
}

impl Json {
    /// Display text of a scalar; arrays of scalars join with `, `.
    fn text(&self) -> String {
        match self {
            Json::Null => String::new(),
            Json::Bool(b) => b.to_string(),
            Json::Number(n) | Json::Text(n) => n.clone(),
            Json::Array(v) => v
                .iter()
                .map(Json::text)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
            Json::Object(m) => m
                .iter()
                .map(|(_, v)| v.text())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Members of an object with nested objects flattened as `parent.child`.
    fn flatten(&self, prefix: &str, out: &mut Vec<(String, String)>) {
        if let Json::Object(m) = self {
            for (k, v) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match v {
                    Json::Object(_) => v.flatten(&key, out),
                    _ => out.push((key, v.text())),
                }
            }
        }
    }
}

/// Read a JSON data source: an array of objects (fields = member names in order of
/// first appearance, nested objects as `a.b`), an array of arrays (first row names
/// the fields), an array of scalars (field `Value`), or an object holding one such
/// array (its first array member).
pub fn read_json(bytes: &[u8], name: &str) -> crate::Result<MergeData> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| crate::EngineError::Unsupported(format!("{name}: not UTF-8 text")))?;
    let v: Json = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| crate::EngineError::Format(format!("{name}: {e}")))?;
    let items = match v {
        Json::Array(a) => a,
        Json::Object(m) => m
            .into_iter()
            .find_map(|(_, x)| match x {
                Json::Array(a) => Some(a),
                _ => None,
            })
            .ok_or_else(|| crate::EngineError::Format(format!("{name}: no array of records")))?,
        _ => {
            return Err(crate::EngineError::Format(format!(
                "{name}: no array of records"
            )))
        }
    };
    let mut fields: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    if items.iter().all(|i| matches!(i, Json::Array(_))) && !items.is_empty() {
        let mut it = items.into_iter();
        if let Some(Json::Array(h)) = it.next() {
            fields = h.iter().map(|x| x.text().trim().to_owned()).collect();
        }
        for i in it.take(MAX_RECORDS) {
            if let Json::Array(r) = i {
                let mut row: Vec<String> = r.iter().map(Json::text).collect();
                row.resize(fields.len(), String::new());
                rows.push(row);
            }
        }
    } else if items.iter().any(|i| matches!(i, Json::Object(_))) {
        let mut records: Vec<Vec<(String, String)>> = Vec::new();
        for i in items.iter().take(MAX_RECORDS) {
            let mut kv = Vec::new();
            i.flatten("", &mut kv);
            for (k, _) in &kv {
                if !fields.contains(k) {
                    fields.push(k.clone());
                }
            }
            records.push(kv);
        }
        for kv in records {
            let mut row = vec![String::new(); fields.len()];
            for (k, v) in kv {
                if let Some(c) = fields.iter().position(|f| *f == k) {
                    row[c] = v;
                }
            }
            rows.push(row);
        }
    } else {
        fields.push("Value".into());
        rows = items
            .iter()
            .take(MAX_RECORDS)
            .map(|i| vec![i.text()])
            .collect();
    }
    Ok(MergeData {
        name: name.to_owned(),
        fields,
        rows,
    })
}

/// A table from a host data provider (e.g. a SQL query run by the server): field
/// names and rows of display text; short rows are padded, long rows cut.
pub fn from_table(name: &str, fields: &[String], rows: &[Vec<String>]) -> MergeData {
    let fields: Vec<String> = fields.iter().map(|f| f.trim().to_owned()).collect();
    let rows = rows
        .iter()
        .take(MAX_RECORDS)
        .map(|r| {
            let mut r = r.clone();
            r.resize(fields.len(), String::new());
            r
        })
        .collect();
    MergeData {
        name: name.to_owned(),
        fields,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcard_records_with_vietnamese_names() {
        // vCard 4.0 with folding and escapes, then a 2.1 card with quoted-printable UTF-8
        // ("Trần Thị Bình": T r =E1=BA=A7 n …), and a card without FN.
        let vcf = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Nguyễn Văn An\r\nN:Nguyễn;An;Văn;Ông;\r\n\
ORG:Công ty BlueStar\\, chi nhánh Hà Nội;Phòng Kế toán\r\nEMAIL;TYPE=work:an@bluestar.vn\r\n\
TEL;TYPE=cell:+84 912 345 678\r\nADR;TYPE=work:;;12 Láng Hạ;Đống Đa;Hà Nội;100000;Việt\r\n  Nam\r\n\
END:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Tr=E1=BA=A7n;B=C3=AC=\r\nnh;Th=E1=BB=8B;;\r\n\
FN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Tr=E1=BA=A7n Th=E1=BB=8B B=C3=ACnh\r\nTEL;HOME:024 3826 1234\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nN:Lê;Cường;Văn;;\r\nitem1.EMAIL;TYPE=INTERNET:cuong@vd.vn\r\nEND:VCARD\r\n";
        let d = read_vcard(vcf.as_bytes(), "danh-ba.vcf");
        assert_eq!(d.rows.len(), 3);
        assert_eq!(d.value(0, "FullName"), Some("Nguyễn Văn An"));
        assert_eq!(d.value(0, "LastName"), Some("Nguyễn"));
        assert_eq!(d.value(0, "MiddleName"), Some("Văn"));
        assert_eq!(d.value(0, "Prefix"), Some("Ông"));
        assert_eq!(
            d.value(0, "Organization"),
            Some("Công ty BlueStar, chi nhánh Hà Nội")
        );
        assert_eq!(d.value(0, "Mobile"), Some("+84 912 345 678"));
        assert_eq!(d.value(0, "City"), Some("Đống Đa"));
        assert_eq!(d.value(0, "Country"), Some("Việt Nam"));
        assert_eq!(
            d.value(0, "Address"),
            Some("12 Láng Hạ, Đống Đa, Hà Nội, 100000, Việt Nam")
        );
        assert_eq!(d.value(1, "FullName"), Some("Trần Thị Bình"));
        assert_eq!(d.value(1, "FirstName"), Some("Bình"));
        assert_eq!(d.value(1, "Phone"), Some("024 3826 1234"));
        assert_eq!(d.value(1, "Mobile"), Some(""));
        // No FN: family + middle + given.
        assert_eq!(d.value(2, "FullName"), Some("Lê Văn Cường"));
        assert_eq!(d.value(2, "Email"), Some("cuong@vd.vn"));
        // Detected by content too.
        let d2 = crate::merge::read_data(vcf.as_bytes(), "contacts").unwrap();
        assert_eq!(d2.rows.len(), 3);
    }

    #[test]
    fn json_arrays() {
        let j = r#"[
          {"Họ tên": "Nguyễn Văn An", "SoTien": 1500000, "DiaChi": {"Tinh": "Hà Nội"}, "VIP": true},
          {"Họ tên": "Trần Thị Bình", "SoTien": 2.5, "Ghi chú": null, "The": ["a", "b"]}
        ]"#;
        let d = read_json(j.as_bytes(), "ds.json").unwrap();
        assert_eq!(
            d.fields,
            ["Họ tên", "SoTien", "DiaChi.Tinh", "VIP", "Ghi chú", "The"]
        );
        assert_eq!(d.rows.len(), 2);
        assert_eq!(d.value(0, "SoTien"), Some("1500000"));
        assert_eq!(d.value(0, "DiaChi.Tinh"), Some("Hà Nội"));
        assert_eq!(d.value(0, "VIP"), Some("true"));
        assert_eq!(d.value(0, "The"), Some(""));
        assert_eq!(d.value(1, "SoTien"), Some("2.5"));
        assert_eq!(d.value(1, "The"), Some("a, b"));
        // An object holding the array; arrays of arrays with a header row.
        let w = r#"{"meta": 1, "records": [["Họ tên", "Tỉnh"], ["Lê Văn Cường", "Huế"], ["Phạm Thị Dung"]]}"#;
        let d = crate::merge::read_data(w.as_bytes(), "x").unwrap();
        assert_eq!(d.fields, ["Họ tên", "Tỉnh"]);
        assert_eq!(
            d.rows,
            [vec!["Lê Văn Cường", "Huế"], vec!["Phạm Thị Dung", ""]]
        );
        assert!(read_json(b"{\"a\": 1}", "x.json").is_err());
        assert!(read_json(b"[1,", "x.json").is_err());
        let s = read_json(b"[\"a\", \"b\"]", "x.json").unwrap();
        assert_eq!(s.fields, ["Value"]);
        assert_eq!(s.rows.len(), 2);
    }

    #[test]
    fn provider_table() {
        let d = from_table(
            "sql: khach_hang",
            &["Họ tên".into(), " Email ".into()],
            &[
                vec!["Nguyễn Văn An".into()],
                vec!["a".into(), "b".into(), "c".into()],
            ],
        );
        assert_eq!(d.fields, ["Họ tên", "Email"]);
        assert_eq!(d.rows[0], ["Nguyễn Văn An", ""]);
        assert_eq!(d.rows[1], ["a", "b"]);
    }
}

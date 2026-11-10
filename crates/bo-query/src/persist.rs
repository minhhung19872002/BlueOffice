//! How queries are kept in a workbook (`docs/spec-notes/sheet-queries.md`):
//!
//! * the definitions, in a custom XML data storage part (ECMA-376 Part 1, §22.5 custom XML
//!   data properties; the part is kept by other software and means nothing to it) whose
//!   root is `<queries xmlns="http://schemas.bluestar.vn/blueoffice/2026/queries">` with one
//!   `<query name="…">` per query holding its JSON definition;
//! * where the source has a standard form, a connection (§18.13 `xl/connections.xml`: text
//!   files §18.13.12 `textPr`, web pages §18.13.15 `webPr`) and a query table part (§18.12)
//!   linked to the loaded table (`table/@tableType="queryTable"`, `tableColumn/@queryTableFieldId`,
//!   §18.5.1.2–3), so other software sees a table refreshed from a connection. Both carry an
//!   `extLst` extension ([`EXT_URI`]) naming the query, which is how BlueOffice finds (and
//!   rewrites) its own elements; everything else in those parts is kept as it was.

use crate::def::{Query, Source};
use bo_opc::xml::{escape, unescape, XmlReader};
use quick_xml::events::Event;
use std::fmt::Write as _;

/// Namespace of the BlueOffice query definitions part.
pub const NS: &str = "http://schemas.bluestar.vn/blueoffice/2026/queries";
/// `ext/@uri` of the extension marking BlueOffice connections and query tables.
pub const EXT_URI: &str = "{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A41}";
/// Relationship type of a connections part (§18.13).
pub const REL_CONNECTIONS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections";
/// Content type of a connections part.
pub const CT_CONNECTIONS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml";
/// Relationship type of a query table part (§18.12).
pub const REL_QUERY_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/queryTable";
/// Content type of a query table part.
pub const CT_QUERY_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.queryTable+xml";
/// Relationship type of a custom XML data storage part (Part 1, §15.2.4).
pub const REL_CUSTOM_XML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
/// Relationship type of a custom XML data storage properties part (§15.2.5).
pub const REL_CUSTOM_XML_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
/// Content type of a custom XML data storage properties part.
pub const CT_CUSTOM_XML_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

/// The definitions part for `queries`.
pub fn queries_xml(queries: &[Query]) -> String {
    let mut out = String::from(DECL);
    let _ = write!(out, "<queries xmlns=\"{NS}\" version=\"1\">");
    for q in queries {
        let json = serde_json::to_string(q).unwrap_or_default();
        let _ = write!(
            out,
            "<query name=\"{}\">{}</query>",
            escape(&q.name),
            escape(&json)
        );
    }
    out.push_str("</queries>");
    out
}

/// Is `data` a BlueOffice query definitions part?
pub fn is_queries_part(data: &[u8]) -> bool {
    let head = &data[..data.len().min(400)];
    let head = String::from_utf8_lossy(head);
    head.contains("<queries") && head.contains(NS)
}

/// The definitions of a definitions part (queries that do not read are skipped).
pub fn parse_queries(data: &[u8]) -> Vec<Query> {
    let mut out = Vec::new();
    let mut r = XmlReader::new(data, "queries");
    let mut text: Option<String> = None;
    loop {
        match r.read() {
            Ok(Event::Start(e)) => {
                let n = e.local_name();
                let s: &str = n.as_ref();
                if s == "query" {
                    text = Some(String::new());
                }
            }
            Ok(Event::Text(t)) => {
                let t: &str = &t;
                if let Some(s) = text.as_mut() {
                    s.push_str(&unescape(t));
                }
            }
            Ok(Event::GeneralRef(g)) => {
                let t: &str = &g;
                if let Some(s) = text.as_mut() {
                    s.push_str(&unescape(&format!("&{t};")));
                }
            }
            Ok(Event::CData(c)) => {
                let t: &str = &c;
                if let Some(s) = text.as_mut() {
                    s.push_str(t);
                }
            }
            Ok(Event::End(e)) => {
                let n = e.local_name();
                let s: &str = n.as_ref();
                if s == "query" {
                    if let Some(json) = text.take() {
                        if let Ok(q) = serde_json::from_str::<Query>(&json) {
                            out.push(q);
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// The custom XML properties part of the definitions part (§22.5.2.2 `datastoreItem`).
pub fn item_props_xml(item_id: &str) -> String {
    format!(
        "{DECL}<ds:datastoreItem ds:itemID=\"{item_id}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs><ds:schemaRef ds:uri=\"{NS}\"/></ds:schemaRefs></ds:datastoreItem>"
    )
}

fn ext(name: &str, p: &str) -> String {
    format!(
        "<{p}extLst><{p}ext uri=\"{EXT_URI}\"><bq:query xmlns:bq=\"{NS}\" name=\"{}\"/></{p}ext></{p}extLst>",
        escape(name)
    )
}

/// Does the query's source have a standard connection form (text file or web page)?
pub fn has_connection(q: &Query) -> bool {
    matches!(q.source, Source::Csv { .. } | Source::Web { .. })
}

/// A `connection` element (§18.13.1) for `q`, with id `id` and name `name`; `None` when the
/// source has no standard form. `prefix` is the namespace prefix of the part (`""` or `x:`).
pub fn connection_xml(q: &Query, id: u32, name: &str, prefix: &str) -> Option<String> {
    let p = prefix;
    let head = |ty: u32| {
        format!(
            "<{p}connection id=\"{id}\" name=\"{}\" type=\"{ty}\" refreshedVersion=\"0\" background=\"1\" saveData=\"1\">",
            escape(name)
        )
    };
    let body = match &q.source {
        // §18.13.12 textPr; ST_Connection type 6 = text.
        Source::Csv {
            file,
            delimiter,
            encoding,
        } => {
            let code_page = match encoding.as_deref() {
                Some("utf-16le" | "utf-16") => 1200,
                Some("utf-16be") => 1201,
                Some("windows-1252" | "tcvn3" | "vni") => 1252,
                _ => 65001,
            };
            let d = delimiter.as_deref().unwrap_or(",");
            let flags = match d {
                "\t" | "\\t" | "tab" => String::new(),
                "," => " tab=\"0\" comma=\"1\"".into(),
                ";" => " tab=\"0\" semicolon=\"1\"".into(),
                " " => " tab=\"0\" space=\"1\"".into(),
                other => format!(" tab=\"0\" delimiter=\"{}\"", escape(other)),
            };
            format!(
                "{}<{p}textPr codePage=\"{code_page}\" sourceFile=\"{}\" delimited=\"1\"{flags}/>",
                head(6),
                escape(file)
            )
        }
        // §18.13.15 webPr; type 4 = web query; tables §18.13.11 (1-based index).
        Source::Web { url, table } => format!(
            "{}<{p}webPr url=\"{}\" htmlTables=\"1\"><{p}tables count=\"1\"><{p}x v=\"{}\"/></{p}tables></{p}webPr>",
            head(4),
            escape(url),
            table + 1
        ),
        _ => return None,
    };
    Some(format!("{body}{}</{p}connection>", ext(&q.name, p)))
}

/// A query table part (§18.12.3 `queryTable`) for the table columns `fields` (table column
/// id, name) of `q`'s table, fed by connection `connection_id`.
pub fn query_table_xml(q: &Query, connection_id: u32, fields: &[(u32, String)]) -> String {
    let mut out = String::from(DECL);
    let _ = write!(
        out,
        "<queryTable xmlns=\"{MAIN_NS}\" name=\"{}\" connectionId=\"{connection_id}\" autoFormatId=\"16\" applyNumberFormats=\"0\" applyBorderFormats=\"0\" applyFontFormats=\"0\" applyPatternFormats=\"0\" applyAlignmentFormats=\"0\" applyWidthHeightFormats=\"0\">",
        escape(&q.name)
    );
    let _ = write!(
        out,
        "<queryTableRefresh nextId=\"{}\"><queryTableFields count=\"{}\">",
        fields.len() + 1,
        fields.len()
    );
    for (k, (col_id, name)) in fields.iter().enumerate() {
        let _ = write!(
            out,
            "<queryTableField id=\"{}\" name=\"{}\" tableColumnId=\"{col_id}\"/>",
            k + 1,
            escape(name)
        );
    }
    out.push_str("</queryTableFields></queryTableRefresh>");
    out.push_str(&ext(&q.name, ""));
    out.push_str("</queryTable>");
    out
}

/// Is `data` a query table part written by BlueOffice?
pub fn is_own_query_table(data: &[u8]) -> bool {
    let s = String::from_utf8_lossy(data);
    s.contains(EXT_URI) && s.contains("queryTable")
}

/// A connections part split around BlueOffice's own connections: the part without them,
/// the root element's prefix (`""` or `x:`) and the largest connection id left.
pub struct Connections {
    /// Part text without BlueOffice connections.
    pub xml: String,
    /// Namespace prefix of the elements (with its colon).
    pub prefix: String,
    /// Largest `connection/@id` left (0 when none).
    pub max_id: u32,
    /// Names of the connections left.
    pub names: Vec<String>,
}

/// Take BlueOffice's connections out of a connections part (other connections stay as
/// their bytes were).
pub fn strip_connections(xml: &str) -> Connections {
    let prefix = xml
        .find("connections")
        .and_then(|i| {
            let before = &xml[..i];
            let lt = before.rfind('<')?;
            Some(before[lt + 1..].to_owned())
        })
        .filter(|p| p.is_empty() || p.ends_with(':'))
        .unwrap_or_default();
    let open = format!("<{prefix}connection");
    let close = format!("</{prefix}connection>");
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    let mut max_id = 0;
    let mut names = Vec::new();
    while let Some(i) = find_element(rest, &open) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let start_end = tail.find('>').map_or(tail.len(), |e| e + 1);
        let end = if tail[..start_end].ends_with("/>") {
            start_end
        } else {
            tail.find(&close).map_or(tail.len(), |e| e + close.len())
        };
        let elem = &tail[..end];
        if !elem.contains(EXT_URI) {
            let start = &tail[..start_end];
            if let Some(id) = attr_of(start, "id").and_then(|v| v.parse::<u32>().ok()) {
                max_id = max_id.max(id);
            }
            if let Some(n) = attr_of(start, "name") {
                names.push(n);
            }
            out.push_str(elem);
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    Connections {
        xml: out,
        prefix,
        max_id,
        names,
    }
}

/// Position of an element start `<name` (not `<names…`).
fn find_element(s: &str, open: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find(open) {
        let at = from + i;
        let next = s[at + open.len()..].chars().next();
        if matches!(next, Some(c) if c.is_whitespace() || c == '>' || c == '/') {
            return Some(at);
        }
        from = at + open.len();
    }
    None
}

fn attr_of(start: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let i = start.find(&pat)? + pat.len();
    let end = start[i..].find('"')?;
    Some(unescape(&start[i..i + end]))
}

/// A connections part holding the connections of `stripped` plus `ours`; `None` when
/// both are empty.
pub fn connections_part(stripped: Option<&Connections>, ours: &[String]) -> Option<String> {
    match stripped {
        Some(c) => {
            if ours.is_empty() && c.max_id == 0 && c.names.is_empty() {
                return None;
            }
            let close = format!("</{}connections>", c.prefix);
            let mut xml = c.xml.clone();
            if let Some(i) = xml.rfind(&close) {
                xml.insert_str(i, &ours.concat());
            } else {
                // `<connections/>`: open it.
                let empty = format!("<{}connections", c.prefix);
                if let Some(s) = xml.find(&empty) {
                    if let Some(e) = xml[s..].find("/>") {
                        let at = s + e;
                        xml.replace_range(at..at + 2, &format!(">{}{close}", ours.concat()));
                    }
                }
            }
            Some(xml)
        }
        None if ours.is_empty() => None,
        None => Some(format!(
            "{DECL}<connections xmlns=\"{MAIN_NS}\">{}</connections>",
            ours.concat()
        )),
    }
}

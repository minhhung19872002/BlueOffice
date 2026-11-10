//! The workbook data model of BlueSheet (`docs/parity/BACKLOG.md` SHEET-DM-001,
//! `docs/spec-notes/sheet-datamodel.md`).
//!
//! * a model is a list of **tables** (sheet tables, SHEET-TABLE-001, which includes the tables
//!   that Get & Transform queries load), **relationships** (one-to-many on a key column,
//!   active or inactive), **calculated columns** and **measures** written in the BlueOffice
//!   model expression language ([`expr`], evaluated by [`eval`]);
//! * pivot tables can take the model as their source ([`pivot`]): fields come from several
//!   related tables and measures are evaluated in the filter context of every report cell;
//! * definitions live in a custom XML part (ECMA-376 Part 1, §15.2.4 / §22.5) of the
//!   workbook, so undo and save follow them like any other part. The binary data model of
//!   other software (`xl/model/item.data`, [MS-XLDM]) is never read or written: it stays an
//!   opaque part, byte for byte, and is only listed ([`foreign_parts`]).

pub mod eval;
pub mod expr;
pub mod pivot;

#[cfg(test)]
mod tests;

use bo_model::sheet::{CellValue, OpaquePart, OpaqueRel, Workbook};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Namespace of the definitions part.
pub const NS: &str = "http://schemas.bluestar.vn/blueoffice/2026/datamodel";
/// Fixed item id of the definitions part (`ds:itemID`, one per workbook).
pub const ITEM_ID: &str = "{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A43}";
/// Relationship type of a custom XML part (§15.2.4).
const REL_CUSTOM_XML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
/// Relationship type of custom XML properties (§15.2.5).
const REL_CUSTOM_XML_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
/// Content type of custom XML properties (§15.2.5).
const CT_CUSTOM_XML_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
/// Workbook relationship id of the definitions part.
const REL_ID: &str = "rIdBoDataModel";

/// A table of the model: a sheet table, by name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelTable {
    /// Sheet table name (§18.5.1.2 `name`).
    pub name: String,
}

/// A relationship: every row of `from_table` (the many side) finds at most one row of
/// `to_table` (the one side) whose `to_column` equals its `from_column`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Relationship {
    /// Many side table.
    pub from_table: String,
    /// Foreign key column.
    pub from_column: String,
    /// One side table.
    pub to_table: String,
    /// Key column (distinct values).
    pub to_column: String,
    /// Filters flow through the relationship (one active path between two tables).
    pub active: bool,
}

/// A measure: an expression evaluated in the filter context of a report cell.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Measure {
    /// Name, unique in the model.
    pub name: String,
    /// Home table (where the field list shows it).
    pub table: String,
    /// Expression.
    pub expression: String,
    /// Number format code of its values (`None`: General).
    pub format: Option<String>,
}

/// A calculated column: an expression evaluated for every row of its table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalcColumn {
    /// Table.
    pub table: String,
    /// Column name, unique in the table.
    pub name: String,
    /// Expression (row context of the table; `RELATED` follows relationships).
    pub expression: String,
}

/// The definitions of a workbook's data model.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DataModel {
    /// Tables, in field list order.
    pub tables: Vec<ModelTable>,
    /// Relationships.
    pub relationships: Vec<Relationship>,
    /// Calculated columns, in evaluation order.
    pub columns: Vec<CalcColumn>,
    /// Measures.
    pub measures: Vec<Measure>,
    /// `cacheId` of every pivot cache built from the model.
    pub pivots: Vec<u32>,
}

impl DataModel {
    /// The model of `wb` (empty when the workbook has none).
    #[must_use]
    pub fn read(wb: &Workbook) -> DataModel {
        defs_part(wb).map_or_else(DataModel::default, |i| parse(&wb.opaque.parts[i].data))
    }

    /// Write the model into `wb` (the part goes away when the model is empty).
    pub fn write(&self, wb: &mut Workbook) {
        write_part(wb, self);
    }

    /// Nothing defined.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
            && self.relationships.is_empty()
            && self.columns.is_empty()
            && self.measures.is_empty()
            && self.pivots.is_empty()
    }

    /// Index of table `name` (case-insensitive).
    #[must_use]
    pub fn table(&self, name: &str) -> Option<usize> {
        let l = name.to_lowercase();
        self.tables.iter().position(|t| t.name.to_lowercase() == l)
    }

    /// Index of measure `name` (case-insensitive).
    #[must_use]
    pub fn measure(&self, name: &str) -> Option<usize> {
        let l = name.to_lowercase();
        self.measures
            .iter()
            .position(|m| m.name.to_lowercase() == l)
    }
}

// ---------------------------------------------------------------------------
// The definitions part
// ---------------------------------------------------------------------------

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

/// The XML of the definitions part.
#[must_use]
pub fn to_xml(m: &DataModel) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    s.push_str(&format!("<dataModel xmlns=\"{NS}\" version=\"1\">"));
    for t in &m.tables {
        s.push_str(&format!("<table name=\"{}\"/>", escape(&t.name)));
    }
    for r in &m.relationships {
        s.push_str(&format!(
            "<relationship fromTable=\"{}\" fromColumn=\"{}\" toTable=\"{}\" toColumn=\"{}\" active=\"{}\"/>",
            escape(&r.from_table),
            escape(&r.from_column),
            escape(&r.to_table),
            escape(&r.to_column),
            u8::from(r.active)
        ));
    }
    for c in &m.columns {
        s.push_str(&format!(
            "<column table=\"{}\" name=\"{}\">{}</column>",
            escape(&c.table),
            escape(&c.name),
            escape(&c.expression)
        ));
    }
    for x in &m.measures {
        s.push_str(&format!(
            "<measure name=\"{}\" table=\"{}\"",
            escape(&x.name),
            escape(&x.table)
        ));
        if let Some(f) = &x.format {
            s.push_str(&format!(" format=\"{}\"", escape(f)));
        }
        s.push_str(&format!(">{}</measure>", escape(&x.expression)));
    }
    for p in &m.pivots {
        s.push_str(&format!("<pivot cacheId=\"{p}\"/>"));
    }
    s.push_str("</dataModel>");
    s
}

/// `data` is a definitions part (its root element is in [`NS`]).
#[must_use]
pub fn is_defs_part(data: &[u8]) -> bool {
    let head = &data[..data.len().min(512)];
    let head = String::from_utf8_lossy(head);
    head.contains("<dataModel") && head.contains(NS)
}

/// Resolve the five predefined entities and numeric character references.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let ent = &tail[1..j];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=j]),
        }
        rest = &tail[j + 1..];
    }
    out.push_str(rest);
    out
}

/// Attributes of a start tag body (` a="1" b='2'`), by local name.
fn tag_attrs(body: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = body;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim();
        let key = key.rsplit(char::is_whitespace).next().unwrap_or(key);
        let key = key.rsplit(':').next().unwrap_or(key).to_owned();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(close) = after[1..].find(q) else {
            break;
        };
        out.insert(key, unescape(&after[1..=close]));
        rest = &after[close + 2..];
    }
    out
}

/// Parse a definitions part (unknown elements are ignored).
#[must_use]
pub fn parse(data: &[u8]) -> DataModel {
    let text = String::from_utf8_lossy(data);
    let mut m = DataModel::default();
    let get = |a: &HashMap<String, String>, k: &str| a.get(k).cloned().unwrap_or_default();
    let mut rest: &str = &text;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        if rest.starts_with('?') || rest.starts_with('!') || rest.starts_with('/') {
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let raw = &rest[..gt];
        let empty = raw.ends_with('/');
        let raw = raw.trim_end_matches('/');
        let name = raw.split_whitespace().next().unwrap_or("");
        let name = name.rsplit(':').next().unwrap_or(name);
        let a = tag_attrs(&raw[raw.find(char::is_whitespace).unwrap_or(raw.len())..]);
        rest = &rest[gt + 1..];
        // Text content of `column` / `measure`.
        let mut body = String::new();
        if !empty && (name == "column" || name == "measure") {
            if let Some(end) = rest.find("</") {
                body = unescape(&rest[..end]);
                rest = &rest[end..];
            }
        }
        match name {
            "table" => m.tables.push(ModelTable {
                name: get(&a, "name"),
            }),
            "relationship" => m.relationships.push(Relationship {
                from_table: get(&a, "fromTable"),
                from_column: get(&a, "fromColumn"),
                to_table: get(&a, "toTable"),
                to_column: get(&a, "toColumn"),
                active: a
                    .get("active")
                    .is_none_or(|v| matches!(v.as_str(), "1" | "true")),
            }),
            "column" => m.columns.push(CalcColumn {
                table: get(&a, "table"),
                name: get(&a, "name"),
                expression: body,
            }),
            "measure" => m.measures.push(Measure {
                name: get(&a, "name"),
                table: get(&a, "table"),
                expression: body,
                format: a.get("format").cloned(),
            }),
            "pivot" => {
                if let Some(id) = a.get("cacheId").and_then(|v| v.parse().ok()) {
                    m.pivots.push(id);
                }
            }
            _ => {}
        }
    }
    m
}

fn defs_part(wb: &Workbook) -> Option<usize> {
    wb.opaque.parts.iter().position(|p| {
        let n = p.name.to_ascii_lowercase();
        n.starts_with("/customxml/item") && !n.contains("props") && is_defs_part(&p.data)
    })
}

fn part_rels_name(part: &str) -> String {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    format!("{dir}/_rels/{file}.rels")
}

fn item_props_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<ds:datastoreItem ds:itemID=\"{ITEM_ID}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs><ds:schemaRef ds:uri=\"{NS}\"/></ds:schemaRefs></ds:datastoreItem>"
    )
}

fn write_part(wb: &mut Workbook, m: &DataModel) {
    let existing = defs_part(wb);
    if m.is_empty() {
        if let Some(i) = existing {
            let name = wb.opaque.parts.remove(i).name;
            let rels = part_rels_name(&name);
            let props = name.replace("/item", "/itemProps");
            wb.opaque.parts.retain(|p| {
                !p.name.eq_ignore_ascii_case(&rels) && !p.name.eq_ignore_ascii_case(&props)
            });
            wb.opaque
                .workbook_rels
                .retain(|r| !r.target.eq_ignore_ascii_case(&name));
        }
        return;
    }
    let data = to_xml(m).into_bytes();
    if let Some(i) = existing {
        wb.opaque.parts[i].data = data;
        return;
    }
    let used = |n: &str, wb: &Workbook| {
        wb.opaque
            .parts
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(n))
    };
    let mut k = 1;
    while used(&format!("/customXml/item{k}.xml"), wb)
        || used(&format!("/customXml/itemProps{k}.xml"), wb)
    {
        k += 1;
    }
    let item = format!("/customXml/item{k}.xml");
    let props = format!("/customXml/itemProps{k}.xml");
    wb.opaque.parts.push(OpaquePart {
        name: item.clone(),
        content_type: None,
        data,
    });
    wb.opaque.parts.push(OpaquePart {
        name: props,
        content_type: Some(CT_CUSTOM_XML_PROPS.into()),
        data: item_props_xml().into_bytes(),
    });
    wb.opaque.parts.push(OpaquePart {
        name: part_rels_name(&item),
        content_type: None,
        data: format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL_CUSTOM_XML_PROPS}\" Target=\"itemProps{k}.xml\"/></Relationships>"
        )
        .into_bytes(),
    });
    let mut id = REL_ID.to_owned();
    while wb.opaque.workbook_rels.iter().any(|r| r.id == id) {
        id.push('x');
    }
    wb.opaque.workbook_rels.push(OpaqueRel {
        id,
        rel_type: REL_CUSTOM_XML.into(),
        target: item,
        external: false,
    });
}

// ---------------------------------------------------------------------------
// Data model parts of other software (read only)
// ---------------------------------------------------------------------------

/// A data model part written by other software, kept byte for byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignPart {
    /// Part name.
    pub name: String,
    /// `model` (the binary model, `xl/model/item.data`), `customXml` (an item of the
    /// model's custom XML) or `connection` (a connection to the workbook model).
    pub kind: String,
    /// Size in bytes (connections: 0).
    pub size: usize,
}

/// The data model content of other software in `wb`: the binary model part, custom XML
/// items that belong to it and the `ThisWorkbookDataModel` connection (all left untouched).
#[must_use]
pub fn foreign_parts(wb: &Workbook) -> Vec<ForeignPart> {
    let mut out = Vec::new();
    for p in &wb.opaque.parts {
        let n = p.name.to_ascii_lowercase();
        if n.starts_with("/xl/model/") && !n.contains("/_rels/") {
            out.push(ForeignPart {
                name: p.name.clone(),
                kind: "model".into(),
                size: p.data.len(),
            });
        } else if n.starts_with("/customxml/item")
            && !n.contains("props")
            && !n.contains("/_rels/")
            && !is_defs_part(&p.data)
        {
            let head = String::from_utf8_lossy(&p.data[..p.data.len().min(2048)]).to_lowercase();
            // Items written with the model: the model's own schemas (Gemini/PowerPivot
            // client data items), not other custom XML such as query mashups.
            let utf16: String = p
                .data
                .iter()
                .take(4096)
                .filter(|b| **b != 0)
                .map(|b| char::from(*b))
                .collect::<String>()
                .to_lowercase();
            if head.contains("gemini") || utf16.contains("gemini") {
                out.push(ForeignPart {
                    name: p.name.clone(),
                    kind: "customXml".into(),
                    size: p.data.len(),
                });
            }
        } else if n == "/xl/connections.xml" {
            let s = String::from_utf8_lossy(&p.data);
            for chunk in s.split("<connection ").skip(1) {
                if let Some(name) = chunk
                    .split("name=\"")
                    .nth(1)
                    .and_then(|x| x.split('"').next())
                {
                    if name.eq_ignore_ascii_case("ThisWorkbookDataModel")
                        || chunk.contains("model=\"1\"")
                    {
                        out.push(ForeignPart {
                            name: name.to_owned(),
                            kind: "connection".into(),
                            size: 0,
                        });
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Model data
// ---------------------------------------------------------------------------

/// A value key for matching (relationships, filters, distinct counts): numbers by value
/// (`-0` = `0`), texts case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// Blank.
    Empty,
    /// A number (bits of the `f64`).
    Num(u64),
    /// Lower-cased text.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// An error literal.
    Err(String),
}

impl Key {
    /// The key of a value.
    #[must_use]
    pub fn of(v: &CellValue) -> Key {
        match v {
            CellValue::Empty => Key::Empty,
            CellValue::Number(n) => Key::Num(if *n == 0.0 { 0.0f64 } else { *n }.to_bits()),
            CellValue::Bool(b) => Key::Bool(*b),
            CellValue::Error(e) => Key::Err(e.as_str().to_owned()),
            CellValue::Text(s) | CellValue::Date(s) => Key::Text(s.to_lowercase()),
            other @ CellValue::RichText(_) => {
                Key::Text(other.as_text().unwrap_or_default().to_lowercase())
            }
        }
    }
}

/// The rows of one model table.
#[derive(Debug, Clone, Default)]
pub struct TableData {
    /// Table name.
    pub name: String,
    /// Column names (source columns, then calculated columns).
    pub columns: Vec<String>,
    /// Number of source columns (the others are calculated).
    pub source_columns: usize,
    /// Rows, one value per column.
    pub rows: Vec<Vec<CellValue>>,
    /// Sheet and table index of the source.
    pub source: Option<(usize, usize)>,
    /// Number format id of every source column (its first number; `0` = General).
    pub formats: Vec<u32>,
}

impl TableData {
    /// Index of column `name` (case-insensitive).
    #[must_use]
    pub fn column(&self, name: &str) -> Option<usize> {
        let l = name.to_lowercase();
        self.columns.iter().position(|c| c.to_lowercase() == l)
    }
}

/// A relationship between loaded tables, with the row map of its many side.
#[derive(Debug, Clone)]
pub struct RelData {
    /// Many side (table, column).
    pub from: (usize, usize),
    /// One side (table, column).
    pub to: (usize, usize),
    /// Active.
    pub active: bool,
    /// For every row of the many side, the row of the one side with the same key.
    pub map: Vec<Option<u32>>,
}

/// A relationship that could not be loaded, by index in [`DataModel::relationships`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProblem {
    /// What it concerns (`relationship`, `column`, `measure`, `table`).
    pub kind: &'static str,
    /// Index in its list of the definitions.
    pub index: usize,
    /// Message.
    pub message: String,
}

/// The loaded model: table rows, relationship maps and parsed measures.
#[derive(Debug, Clone, Default)]
pub struct ModelData {
    /// The definitions.
    pub def: DataModel,
    /// Tables in [`DataModel::tables`] order (tables not found are empty).
    pub tables: Vec<TableData>,
    /// Loaded relationships (by index in the definitions; `None` when invalid).
    pub rels: Vec<Option<RelData>>,
    /// Parsed measures (by index in the definitions).
    pub measures: Vec<Result<expr::Expr, expr::ParseError>>,
    /// Problems found while loading.
    pub problems: Vec<ModelProblem>,
}

/// Find a sheet table by name: (sheet, table index).
#[must_use]
pub fn find_sheet_table(wb: &Workbook, name: &str) -> Option<(usize, usize)> {
    let l = name.to_lowercase();
    wb.sheets.iter().enumerate().find_map(|(si, ws)| {
        ws.tables
            .iter()
            .position(|t| t.name.to_lowercase() == l)
            .map(|ti| (si, ti))
    })
}

/// Rows of a sheet table (header and totals rows left out).
fn read_table(wb: &Workbook, si: usize, ti: usize) -> TableData {
    let ws = &wb.sheets[si];
    let t = &ws.tables[ti];
    let header = t.header_row_count.unwrap_or(1);
    let totals = t.totals_row_count.unwrap_or(0);
    let first = t.range.first.row + header;
    let last = t.range.last.row.saturating_sub(totals);
    let c0 = t.range.first.col;
    let width = t.columns.len().max(t.range.cols() as usize);
    let mut columns: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
    while columns.len() < width {
        columns.push(format!("Column{}", columns.len() + 1));
    }
    let mut rows: Vec<Vec<CellValue>> = Vec::new();
    if first <= last && t.range.last.row >= first {
        rows = vec![vec![CellValue::Empty; width]; (last - first + 1) as usize];
        for (&(r, c), cell) in ws.cells.range((first, c0)..=(last, t.range.last.col)) {
            if c < c0 || c > t.range.last.col {
                continue;
            }
            let v = match &cell.value {
                CellValue::RichText(_) => CellValue::Text(cell.value.as_text().unwrap_or_default()),
                v => v.clone(),
            };
            rows[(r - first) as usize][(c - c0) as usize] = v;
        }
    }
    let formats = crate::pivot::source_formats(
        wb,
        si,
        bo_model::sheet::CellRange::new(
            bo_model::sheet::CellRef::new(first.saturating_sub(1), c0),
            bo_model::sheet::CellRef::new(last.max(first), t.range.last.col),
        ),
    );
    TableData {
        name: t.name.clone(),
        source_columns: columns.len(),
        columns,
        rows,
        source: Some((si, ti)),
        formats,
    }
}

impl ModelData {
    /// Load the model of `wb`: table rows, relationship maps, calculated columns and
    /// measures (problems are collected, not fatal).
    #[must_use]
    pub fn load(wb: &Workbook) -> ModelData {
        Self::load_def(wb, DataModel::read(wb))
    }

    /// Load the definitions `def` against the tables of `wb`.
    #[must_use]
    pub fn load_def(wb: &Workbook, def: DataModel) -> ModelData {
        let mut md = ModelData {
            tables: Vec::with_capacity(def.tables.len()),
            ..ModelData::default()
        };
        for (i, t) in def.tables.iter().enumerate() {
            if let Some((si, ti)) = find_sheet_table(wb, &t.name) {
                md.tables.push(read_table(wb, si, ti));
            } else {
                md.problems.push(ModelProblem {
                    kind: "table",
                    index: i,
                    message: format!("table {} not found", t.name),
                });
                md.tables.push(TableData {
                    name: t.name.clone(),
                    ..TableData::default()
                });
            }
        }
        md.def = def;
        md.measures = md
            .def
            .measures
            .iter()
            .map(|m| expr::parse(&m.expression))
            .collect();
        md.link();
        // Calculated columns, in order (each may use the previous ones).
        let cols = md.def.columns.clone();
        for (ci, c) in cols.iter().enumerate() {
            let Some(t) = md.def.table(&c.table) else {
                md.problems.push(ModelProblem {
                    kind: "column",
                    index: ci,
                    message: format!("table {} not in the model", c.table),
                });
                continue;
            };
            if md.tables[t].column(&c.name).is_some() {
                md.problems.push(ModelProblem {
                    kind: "column",
                    index: ci,
                    message: format!("column {} already exists", c.name),
                });
                continue;
            }
            let values: Vec<CellValue> = match expr::parse(&c.expression) {
                Ok(e) => {
                    let n = md.tables[t].rows.len();
                    let ev = eval::Evaluator::new(&md);
                    (0..n).map(|r| ev.row_value(&e, t, r).into_cell()).collect()
                }
                Err(e) => {
                    md.problems.push(ModelProblem {
                        kind: "column",
                        index: ci,
                        message: e.to_string(),
                    });
                    vec![
                        CellValue::Error(bo_model::sheet::CellError::Name);
                        md.tables[t].rows.len()
                    ]
                }
            };
            let td = &mut md.tables[t];
            td.columns.push(c.name.clone());
            for (row, v) in td.rows.iter_mut().zip(values) {
                row.push(v);
            }
            // A relationship on the new column can now be linked.
            md.link();
        }
        md
    }

    /// Build the row maps of the relationships.
    fn link(&mut self) {
        self.problems.retain(|p| p.kind != "relationship");
        let mut rels = Vec::with_capacity(self.def.relationships.len());
        for (i, r) in self.def.relationships.iter().enumerate() {
            let find = |t: &str, c: &str| -> Option<(usize, usize)> {
                let ti = self.def.table(t)?;
                Some((ti, self.tables[ti].column(c)?))
            };
            let (Some(from), Some(to)) = (
                find(&r.from_table, &r.from_column),
                find(&r.to_table, &r.to_column),
            ) else {
                self.problems.push(ModelProblem {
                    kind: "relationship",
                    index: i,
                    message: "column not found".into(),
                });
                rels.push(None);
                continue;
            };
            let mut index: HashMap<Key, u32> = HashMap::new();
            let mut dup = false;
            for (ri, row) in self.tables[to.0].rows.iter().enumerate() {
                let k = Key::of(&row[to.1]);
                if k == Key::Empty {
                    continue;
                }
                if index.insert(k, ri as u32).is_some() {
                    dup = true;
                }
            }
            if dup {
                self.problems.push(ModelProblem {
                    kind: "relationship",
                    index: i,
                    message: format!("{}[{}] has duplicate values", r.to_table, r.to_column),
                });
            }
            let map = self.tables[from.0]
                .rows
                .iter()
                .map(|row| index.get(&Key::of(&row[from.1])).copied())
                .collect();
            rels.push(Some(RelData {
                from,
                to,
                active: r.active,
                map,
            }));
        }
        self.rels = rels;
    }

    /// Index of table `name`.
    #[must_use]
    pub fn table(&self, name: &str) -> Option<usize> {
        self.def.table(name)
    }

    /// (table, column) of `Table[Column]`.
    #[must_use]
    pub fn column(&self, table: &str, column: &str) -> Option<(usize, usize)> {
        let t = self.table(table)?;
        Some((t, self.tables[t].column(column)?))
    }

    /// Tables reachable from `t` through active many-to-one relationships, with the
    /// relationship path to each (breadth first; `t` itself first with no path).
    #[must_use]
    pub fn reachable(&self, t: usize, use_rels: &[usize]) -> Vec<(usize, Vec<usize>)> {
        let mut out: Vec<(usize, Vec<usize>)> = vec![(t, Vec::new())];
        let mut i = 0;
        while i < out.len() {
            let (cur, path) = out[i].clone();
            for (ri, r) in self.rels.iter().enumerate() {
                let Some(r) = r else { continue };
                if r.from.0 != cur || !self.rel_active(ri, use_rels) {
                    continue;
                }
                if out.iter().any(|(x, _)| *x == r.to.0) {
                    continue;
                }
                let mut p = path.clone();
                p.push(ri);
                out.push((r.to.0, p));
            }
            i += 1;
        }
        out
    }

    /// Relationship `ri` filters in a context that activates `use_rels`
    /// (`USERELATIONSHIP`: the named relationship replaces the others between its tables).
    #[must_use]
    pub fn rel_active(&self, ri: usize, use_rels: &[usize]) -> bool {
        let Some(Some(r)) = self.rels.get(ri) else {
            return false;
        };
        if use_rels.contains(&ri) {
            return true;
        }
        let pair = |x: &RelData| {
            let (a, b) = (x.from.0, x.to.0);
            (a.min(b), a.max(b))
        };
        let overridden = use_rels.iter().any(|u| {
            self.rels
                .get(*u)
                .and_then(Option::as_ref)
                .is_some_and(|x| pair(x) == pair(r))
        });
        r.active && !overridden
    }
}

/// How two columns relate, from the distinct values of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cardinality {
    /// The first column repeats, the second is unique.
    ManyToOne,
    /// The first column is unique, the second repeats.
    OneToMany,
    /// Both unique.
    OneToOne,
    /// Neither unique (not supported).
    ManyToMany,
}

/// Cardinality of `a` → `b` and the number of non-blank values of `a` with no match in `b`.
#[must_use]
pub fn detect(a: &[CellValue], b: &[CellValue]) -> (Cardinality, usize) {
    let unique = |vals: &[CellValue]| {
        let mut seen = std::collections::HashSet::new();
        vals.iter()
            .map(Key::of)
            .filter(|k| *k != Key::Empty)
            .all(|k| seen.insert(k))
    };
    let (ua, ub) = (unique(a), unique(b));
    let card = match (ua, ub) {
        (false, true) => Cardinality::ManyToOne,
        (true, false) => Cardinality::OneToMany,
        (true, true) => Cardinality::OneToOne,
        (false, false) => Cardinality::ManyToMany,
    };
    let keys: std::collections::HashSet<Key> = b.iter().map(Key::of).collect();
    let unmatched = a
        .iter()
        .map(Key::of)
        .filter(|k| *k != Key::Empty && !keys.contains(k))
        .count();
    (card, unmatched)
}

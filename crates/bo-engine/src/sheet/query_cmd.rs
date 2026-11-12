//! Get & Transform data in BlueSheet (`docs/parity/BACKLOG.md` SHEET-PQ-001): the query
//! commands of the JSON bridge over `bo-query`.
//!
//! * `querySource` — the host hands over the bytes of a source (a file, the files of a
//!   folder, a web page) under its key; the engine never reads files or the network;
//! * `queryInspect` — what the navigator shows for a source (sheets and tables, JSON arrays,
//!   repeating XML elements, web tables, CSV encoding and delimiter) and the steps proposed
//!   for a new query;
//! * `queryPreview` — the table after the first steps of an unsaved query;
//! * `querySave` (Close & Load), `queryRefresh`, `queryDelete` — one undo step each; the
//!   result is written as a table (SHEET-TABLE-001) whose name the query keeps;
//! * `queryList`, `queryGet`, `queryMashup` (M formulas of a [MS-QDEFF] DataMashup part
//!   written by other software, read only).
//!
//! Definitions live in a custom XML part of the workbook (`bo_query::persist`) so undo and
//! save follow them like any other part; [`sync_ecma_parts`] writes the standard connection
//! and query table parts just before an `.xlsx` is written.

use super::cmd::Recalc;
use super::SheetEngine;
use crate::{EngineError, Result};
use base64::Engine as _;
use bo_model::sheet::{
    Cell, CellRange, CellRef, CellValue, OpaquePart, OpaqueRel, Workbook, Worksheet,
};
use bo_query::persist;
use bo_query::{Evaluator, QTable, Query, Resolver, Source, Val};
use bo_sheet::numfmt::Locale;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Bytes of query sources provided by the host this session (not saved in the file).
#[derive(Debug, Default, Clone)]
pub(crate) struct QuerySources {
    files: HashMap<String, Vec<u8>>,
    folders: HashMap<String, Vec<(String, Vec<u8>)>>,
}

/// A file sent by the host.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceFile {
    /// File name.
    pub name: String,
    /// Bytes, base64.
    pub data: String,
}

/// Where Close & Load puts the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoadMode {
    /// A new sheet named after the query.
    #[default]
    NewSheet,
    /// The given cell of an existing sheet.
    Existing,
    /// Keep the definition only (connection only).
    None,
}

/// A Get & Transform command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum QueryCommand {
    /// Provide the bytes of a source: `{ ok, files }`.
    QuerySource {
        /// Source key (file name or path, folder, URL).
        key: String,
        /// `file`, `folder` or `web`.
        #[serde(default)]
        access: String,
        /// The file (one) or the folder's files.
        files: Vec<SourceFile>,
    },
    /// Navigator content of a source and the proposed steps.
    QueryInspect {
        /// The source.
        source: Source,
    },
    /// The table after `upto` steps of `query` (all when absent): `{ columns, types, rows,
    /// rowCount, error? }`.
    QueryPreview {
        /// The query (saved or not).
        query: Query,
        /// Steps applied.
        #[serde(default)]
        upto: Option<usize>,
        /// Rows returned (default 200).
        #[serde(default)]
        limit: Option<usize>,
    },
    /// The saved queries.
    QueryList,
    /// One saved query.
    QueryGet {
        /// Name.
        name: String,
    },
    /// Save a query and load its result (one undo step): `{ name, table?, sheet?, rows }`.
    QuerySave {
        /// The query.
        query: Query,
        /// Name of the query it replaces (edit), if any.
        #[serde(default)]
        original: Option<String>,
        /// Where the result goes (a query already loaded stays where it is).
        #[serde(default)]
        load: LoadMode,
        /// Sheet of `existing`.
        #[serde(default)]
        sheet: usize,
        /// Top-left row of `existing`.
        #[serde(default)]
        row: u32,
        /// Top-left column of `existing`.
        #[serde(default)]
        col: u32,
    },
    /// Re-run queries and rewrite their tables (one undo step): `{ refreshed, errors }`.
    QueryRefresh {
        /// Queries (absent: all).
        #[serde(default)]
        names: Option<Vec<String>>,
    },
    /// Delete a query (one undo step); its table stays as a plain table unless `deleteTable`.
    QueryDelete {
        /// Name.
        name: String,
        /// Also clear the loaded table.
        #[serde(default)]
        delete_table: bool,
    },
    /// M formulas of a [MS-QDEFF] DataMashup part of the file (read only): `{ present, formulas }`.
    QueryMashup,
}

impl QueryCommand {
    /// Parses a query command (`None`: another command).
    pub fn parse(v: &Value) -> Option<Result<QueryCommand>> {
        let name = v.get("cmd")?.as_str()?;
        if !name.starts_with("query") {
            return None;
        }
        Some(
            serde_json::from_value(v.clone())
                .map_err(|e| EngineError::Sheet(format!("bad command: {e}"))),
        )
    }

    fn edits(&self) -> bool {
        matches!(
            self,
            QueryCommand::QuerySave { .. }
                | QueryCommand::QueryRefresh { .. }
                | QueryCommand::QueryDelete { .. }
        )
    }
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

struct EngineResolver<'a> {
    sources: &'a QuerySources,
    defs: &'a [Query],
    locale: Locale,
}

impl Resolver for EngineResolver<'_> {
    fn file(&self, key: &str) -> Option<&[u8]> {
        self.sources.files.get(key).map(Vec::as_slice)
    }

    fn folder(&self, key: &str) -> Option<Vec<(&str, &[u8])>> {
        self.sources
            .folders
            .get(key)
            .map(|v| v.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect())
    }

    fn query(&self, name: &str) -> Option<Query> {
        let l = name.to_lowercase();
        self.defs
            .iter()
            .find(|q| q.name.to_lowercase() == l)
            .cloned()
    }

    fn workbook(&self, bytes: &[u8], name: &str) -> std::result::Result<Workbook, String> {
        super::read_workbook_bytes(bytes, name, &self.locale).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// The definitions part
// ---------------------------------------------------------------------------

/// Fixed item id of the definitions part (one per workbook).
const ITEM_ID: &str = "{6E1B3C2A-9D84-4F5B-A7C1-3B0E5D9F2A42}";
/// Workbook relationship id of the definitions part.
const REL_ID: &str = "rIdBoQueries";

fn defs_part(wb: &Workbook) -> Option<usize> {
    wb.opaque.parts.iter().position(|p| {
        p.name.to_ascii_lowercase().starts_with("/customxml/item")
            && !p.name.to_ascii_lowercase().contains("props")
            && persist::is_queries_part(&p.data)
    })
}

/// The saved queries of `wb`.
pub(crate) fn read_defs(wb: &Workbook) -> Vec<Query> {
    defs_part(wb).map_or_else(Vec::new, |i| {
        persist::parse_queries(&wb.opaque.parts[i].data)
    })
}

fn rels_xml(rels: &[(&str, &str, &str)]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (id, ty, target) in rels {
        s.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{target}\"/>"
        ));
    }
    s.push_str("</Relationships>");
    s
}

fn part_rels_name(part: &str) -> String {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    format!("{dir}/_rels/{file}.rels")
}

/// Write `defs` into `wb` (the part goes away with the last query).
pub(crate) fn write_defs(wb: &mut Workbook, defs: &[Query]) {
    let existing = defs_part(wb);
    if defs.is_empty() {
        if let Some(i) = existing {
            let name = wb.opaque.parts.remove(i).name;
            let rels = part_rels_name(&name);
            let props = wb
                .opaque
                .parts
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(&rels))
                .and_then(|p| {
                    let s = String::from_utf8_lossy(&p.data);
                    let t = s.split("Target=\"").nth(1)?.split('"').next()?.to_owned();
                    Some(resolve(&name, &t))
                });
            wb.opaque.parts.retain(|p| {
                !p.name.eq_ignore_ascii_case(&rels)
                    && props
                        .as_ref()
                        .is_none_or(|pp| !p.name.eq_ignore_ascii_case(pp))
            });
            wb.opaque
                .workbook_rels
                .retain(|r| !r.target.eq_ignore_ascii_case(&name));
        }
        return;
    }
    let data = persist::queries_xml(defs).into_bytes();
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
        name: props.clone(),
        content_type: Some(persist::CT_CUSTOM_XML_PROPS.into()),
        data: persist::item_props_xml(ITEM_ID).into_bytes(),
    });
    wb.opaque.parts.push(OpaquePart {
        name: part_rels_name(&item),
        content_type: None,
        data: rels_xml(&[(
            "rId1",
            persist::REL_CUSTOM_XML_PROPS,
            &format!("itemProps{k}.xml"),
        )])
        .into_bytes(),
    });
    let mut id = REL_ID.to_owned();
    while wb.opaque.workbook_rels.iter().any(|r| r.id == id) {
        id.push('x');
    }
    wb.opaque.workbook_rels.push(OpaqueRel {
        id,
        rel_type: persist::REL_CUSTOM_XML.into(),
        target: item,
        external: false,
    });
    // `xml` parts are application/xml (a Default every package has).
}

/// Absolute part name of `target` relative to `source`'s folder.
fn resolve(source: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.to_owned();
    }
    let mut parts: Vec<&str> = source
        .rsplit_once('/')
        .map_or("", |(d, _)| d)
        .split('/')
        .collect();
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    let joined = parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{joined}")
}

// ---------------------------------------------------------------------------
// Loading a result as a table
// ---------------------------------------------------------------------------

fn find_table(wb: &Workbook, name: &str) -> Option<(usize, usize)> {
    let l = name.to_lowercase();
    wb.sheets.iter().enumerate().find_map(|(si, ws)| {
        ws.tables
            .iter()
            .position(|t| t.name.to_lowercase() == l)
            .map(|ti| (si, ti))
    })
}

/// A table name made from a query name (§18.5.1.2 `name` rules).
fn table_name_for(wb: &Workbook, query: &str) -> String {
    let mut base: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !base
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        base.insert(0, '_');
    }
    if !bo_sheet::table::valid_table_name(&base) {
        base = format!("_{base}");
    }
    let taken = |n: &str| {
        let l = n.to_lowercase();
        wb.sheets
            .iter()
            .flat_map(|s| &s.tables)
            .any(|t| t.name.to_lowercase() == l)
            || wb.defined_names.iter().any(|d| d.name.to_lowercase() == l)
    };
    let mut name = base.clone();
    let mut k = 2;
    while taken(&name) || !bo_sheet::table::valid_table_name(&name) {
        name = format!("{base}_{k}");
        k += 1;
    }
    name
}

/// A sheet name made from a query name (§18.2.19 rules: ≤ 31 characters, none of `:\/?*[]`).
fn sheet_name_for(wb: &Workbook, query: &str) -> String {
    let base: String = query
        .chars()
        .filter(|c| !matches!(c, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
        .take(28)
        .collect();
    let base = if base.trim().is_empty() {
        "Query".to_owned()
    } else {
        base.trim().trim_matches('\'').to_owned()
    };
    let mut name = base.clone();
    let mut k = 2;
    while wb.sheet(&name).is_some() {
        name = format!("{base} ({k})");
        k += 1;
    }
    name
}

fn cell_of(v: &Val, date_style: u32) -> Option<Cell> {
    let (value, style) = match v {
        Val::Null => return None,
        Val::Num(n) => (CellValue::Number(*n), 0),
        Val::Text(s) => (CellValue::Text(s.clone()), 0),
        Val::Bool(b) => (CellValue::Bool(*b), 0),
        Val::Date(d) => (CellValue::Number(*d), date_style),
        Val::Err(e) => (CellValue::Error(bo_model::sheet::CellError::parse(e)), 0),
    };
    Some(Cell {
        value,
        style,
        ..Cell::default()
    })
}

fn date_style(wb: &mut Workbook, locale: &Locale) -> u32 {
    let code = if locale.decimal == ',' {
        "dd/mm/yyyy"
    } else {
        "yyyy-mm-dd"
    };
    bo_sheet::style::patched_xf(
        &mut wb.styles,
        0,
        &bo_sheet::style::StylePatch {
            num_fmt: Some(code.into()),
            ..bo_sheet::style::StylePatch::default()
        },
    )
}

fn write_cells(ws: &mut Worksheet, at: CellRef, t: &QTable, date_style: u32) {
    for (c, name) in t.columns.iter().enumerate() {
        ws.cells.insert(
            (at.row, at.col + c as u32),
            Cell {
                value: CellValue::Text(name.clone()),
                ..Cell::default()
            },
        );
    }
    for (r, row) in t.rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            let key = (at.row + 1 + r as u32, at.col + c as u32);
            match cell_of(v, date_style) {
                Some(cell) => {
                    ws.cells.insert(key, cell);
                }
                None => {
                    ws.cells.remove(&key);
                }
            }
        }
    }
}

fn clear_range(ws: &mut Worksheet, r: CellRange) {
    let keys: Vec<(u32, u32)> = ws
        .cells
        .range((r.first.row, 0)..=(r.last.row, u32::MAX))
        .map(|(k, _)| *k)
        .filter(|&(_, c)| c >= r.first.col && c <= r.last.col)
        .collect();
    for k in keys {
        ws.cells.remove(&k);
    }
}

fn overlaps(a: &CellRange, b: &CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
}

/// Range of a table of `t` at `at` (header row, at least one data row).
fn range_for(at: CellRef, t: &QTable) -> Result<CellRange> {
    let cols = t.columns.len().max(1) as u32;
    let rows = t.rows.len().max(1) as u32;
    let last = CellRef::new(at.row + rows, at.col + cols - 1);
    if last.row >= super::MAX_ROWS || last.col >= super::MAX_COLS {
        return Err(bad("query: the result does not fit on the sheet"));
    }
    Ok(CellRange::new(at, last))
}

/// Load `t` as a new table at (`sheet`, `at`); its name.
fn load_new(
    wb: &mut Workbook,
    sheet: usize,
    at: CellRef,
    t: &QTable,
    query: &str,
    locale: &Locale,
) -> Result<String> {
    if t.columns.is_empty() {
        return Err(bad("query: the result has no columns"));
    }
    let range = range_for(at, t)?;
    let ws = wb
        .sheets
        .get(sheet)
        .ok_or(EngineError::PageOutOfRange(sheet))?;
    if ws.tables.iter().any(|x| overlaps(&x.range, &range)) {
        return Err(bad("query: the result would overlap a table"));
    }
    let ds = date_style(wb, locale);
    write_cells(&mut wb.sheets[sheet], at, t, ds);
    let ti = bo_sheet::table::create_table(wb, sheet, range, locale).map_err(bad)?;
    let name = table_name_for(wb, query);
    bo_sheet::table::rename_table(wb, sheet, ti, &name).map_err(bad)?;
    Ok(name)
}

/// A column of a reloaded query table: column `i` of the query result, or the user's column
/// `k` of the old table.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Query(usize),
    User(usize),
}

/// The columns of the reloaded table: the query result's columns in their new order, each
/// column the user added (an old column that is not a query column) kept after the query
/// column it followed (or first when none did), like a spreadsheet application keeps the
/// columns added to a query table. `query_cols` are the result's columns of the last load;
/// empty (a definition saved before they were kept), the calculated columns not in the
/// result are taken as the user's.
fn slots(old: &bo_model::sheet::Table, t: &QTable, query_cols: &[String]) -> Vec<Slot> {
    let in_result = |n: &str| t.columns.iter().position(|c| c.eq_ignore_ascii_case(n));
    let is_user = |c: &bo_model::sheet::TableColumn| {
        in_result(&c.name).is_none()
            && if query_cols.is_empty() {
                c.calculated_formula.is_some()
            } else {
                !query_cols.iter().any(|q| q.eq_ignore_ascii_case(&c.name))
            }
    };
    let mut out: Vec<Slot> = (0..t.columns.len()).map(Slot::Query).collect();
    for (k, c) in old.columns.iter().enumerate() {
        if !is_user(c) {
            continue;
        }
        // The nearest query column before it that is still in the result.
        let anchor = old.columns[..k].iter().rev().find_map(|p| {
            if is_user(p) {
                None
            } else {
                in_result(&p.name)
            }
        });
        let mut pos = anchor.map_or(0, |i| {
            out.iter()
                .position(|s| *s == Slot::Query(i))
                .map_or(out.len(), |p| p + 1)
        });
        while matches!(out.get(pos), Some(Slot::User(_))) {
            pos += 1;
        }
        out.insert(pos, Slot::User(k));
    }
    out
}

/// Rewrite the table (`sheet`, `ti`) with `t`: same top-left cell and style, columns kept by
/// name (their ids and settings), the totals row (if any) moved below the new rows. Columns
/// the user added to the table stay: a calculated column (§18.5.1.3 `calculatedColumnFormula`)
/// is filled down over the new rows, other cells keep their values row by row.
fn reload(
    wb: &mut Workbook,
    sheet: usize,
    ti: usize,
    t: &QTable,
    query_cols: &[String],
    locale: &Locale,
) -> Result<()> {
    if t.columns.is_empty() {
        return Err(bad("query: the result has no columns"));
    }
    let old = wb.sheets[sheet].tables[ti].clone();
    let totals = old.totals_row_count.unwrap_or(0) > 0;
    let at = old.range.first;
    let layout = slots(&old, t, query_cols);
    let mut range = range_for(at, t)?;
    range.last.col = at.col + layout.len() as u32 - 1;
    if range.last.col >= super::MAX_COLS {
        return Err(bad("query: the result does not fit on the sheet"));
    }
    if totals {
        range.last.row += 1;
    }
    if wb.sheets[sheet]
        .tables
        .iter()
        .enumerate()
        .any(|(k, x)| k != ti && overlaps(&x.range, &range))
    {
        return Err(bad("query: the result would overlap a table"));
    }
    // Totals row cells by column name.
    let ws = &wb.sheets[sheet];
    let total_cells: HashMap<String, Cell> = if totals {
        old.columns
            .iter()
            .enumerate()
            .filter_map(|(k, c)| {
                ws.cells
                    .get(&(old.range.last.row, old.range.first.col + k as u32))
                    .map(|cell| (c.name.to_lowercase(), cell.clone()))
            })
            .collect()
    } else {
        HashMap::new()
    };
    // Cells of the user's plain columns, by data row (header included at index 0).
    let old_data_last = old.range.last.row - u32::from(totals);
    let user_cells: HashMap<usize, Vec<Option<Cell>>> = layout
        .iter()
        .filter_map(|s| match s {
            Slot::User(k) if old.columns[*k].calculated_formula.is_none() => Some(*k),
            _ => None,
        })
        .map(|k| {
            let col = old.range.first.col + k as u32;
            let cells = (old.range.first.row..=old_data_last)
                .map(|r| ws.cells.get(&(r, col)).cloned())
                .collect();
            (k, cells)
        })
        .collect();
    let ds = date_style(wb, locale);
    let ws = &mut wb.sheets[sheet];
    clear_range(ws, old.range);
    // The query's columns in their places, blank where the user's columns are.
    let shown = QTable::new(
        layout
            .iter()
            .map(|s| match s {
                Slot::Query(i) => t.columns[*i].clone(),
                Slot::User(k) => old.columns[*k].name.clone(),
            })
            .collect(),
        t.rows
            .iter()
            .map(|row| {
                layout
                    .iter()
                    .map(|s| match s {
                        Slot::Query(i) => row.get(*i).cloned().unwrap_or(Val::Null),
                        Slot::User(_) => Val::Null,
                    })
                    .collect()
            })
            .collect(),
    );
    write_cells(ws, at, &shown, ds);
    let data_rows = t.rows.len().max(1) as u32;
    for (p, s) in layout.iter().enumerate() {
        let Slot::User(k) = s else { continue };
        let Some(cells) = user_cells.get(k) else {
            continue;
        };
        let col = at.col + p as u32;
        for (r, cell) in cells.iter().enumerate().take(data_rows as usize + 1) {
            if let Some(cell) = cell {
                ws.cells.insert((at.row + r as u32, col), cell.clone());
            }
        }
    }
    let mut next_id = old.columns.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let columns: Vec<_> = layout
        .iter()
        .map(|s| match s {
            Slot::User(k) => old.columns[*k].clone(),
            Slot::Query(i) => {
                let name = &t.columns[*i];
                if let Some(c) = old
                    .columns
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(name))
                {
                    let mut c = c.clone();
                    c.name.clone_from(name);
                    c
                } else {
                    let id = next_id;
                    next_id += 1;
                    bo_model::sheet::TableColumn {
                        id,
                        name: name.clone(),
                        ..Default::default()
                    }
                }
            }
        })
        .collect();
    if totals {
        for (k, c) in columns.iter().enumerate() {
            if let Some(cell) = total_cells.get(&c.name.to_lowercase()) {
                ws.cells
                    .insert((range.last.row, at.col + k as u32), cell.clone());
            }
        }
    }
    let tab = &mut ws.tables[ti];
    tab.range = range;
    tab.columns = columns;
    if tab.auto_filter.is_some() {
        let data_last = if totals {
            range.last.row - 1
        } else {
            range.last.row
        };
        let af = CellRange::new(range.first, CellRef::new(data_last, range.last.col));
        tab.auto_filter = Some(format!("<autoFilter ref=\"{af}\"/>"));
    }
    // Calculated columns of the user's fill the new rows.
    let first_data = range.first.row + 1;
    bo_sheet::table::fill_calculated_rows(ws, ti, first_data, first_data + data_rows - 1);
    Ok(())
}

/// The load target of a query after loading `t` into table `table`.
fn load_target(table: String, t: &QTable) -> bo_query::LoadTarget {
    bo_query::LoadTarget {
        table,
        columns: t.columns.clone(),
    }
}

/// A table was renamed from `old` to `new`: the queries loading into it follow (the link
/// between a query and its table is the table name).
pub(crate) fn table_renamed(wb: &mut Workbook, old: &str, new: &str) {
    let mut defs = read_defs(wb);
    let mut changed = false;
    for q in &mut defs {
        if let Some(l) = q
            .load
            .as_mut()
            .filter(|l| l.table.eq_ignore_ascii_case(old))
        {
            new.clone_into(&mut l.table);
            changed = true;
        }
    }
    if changed {
        write_defs(wb, &defs);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn error_json(e: &bo_query::StepError) -> Value {
    json!({
        "step": e.step,
        "code": e.error.code(),
        "message": e.error.to_string(),
        "missing": e.error.missing(),
    })
}

fn table_json(t: &QTable, limit: usize) -> Value {
    json!({
        "columns": t.columns,
        "types": t.types,
        "rows": t.rows.iter().take(limit).map(|r| r.iter().map(Val::to_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "rowCount": t.rows.len(),
    })
}

impl SheetEngine {
    /// Runs a query command (read-only workbooks refuse the editing ones).
    pub(crate) fn query_command_checked(&mut self, cmd: QueryCommand) -> Result<Value> {
        if self.sec.read_only && cmd.edits() {
            return Err(EngineError::Sheet(crate::security::READ_ONLY.into()));
        }
        self.query_command(cmd)
    }

    fn resolver<'a>(&'a self, defs: &'a [Query]) -> EngineResolver<'a> {
        EngineResolver {
            sources: &self.query_sources,
            defs,
            locale: self.locale,
        }
    }

    fn defs(&self) -> Result<Vec<Query>> {
        Ok(read_defs(self.wb.as_ref().ok_or(EngineError::NoDocument)?))
    }

    /// Runs a query command.
    pub fn query_command(&mut self, cmd: QueryCommand) -> Result<Value> {
        match cmd {
            QueryCommand::QuerySource { key, access, files } => {
                let decode = |f: &SourceFile| {
                    base64::engine::general_purpose::STANDARD
                        .decode(f.data.as_bytes())
                        .map(|b| (f.name.clone(), b))
                        .map_err(|_| bad("querySource: bad base64"))
                };
                let files: Vec<(String, Vec<u8>)> =
                    files.iter().map(decode).collect::<Result<_>>()?;
                let n = files.len();
                if access == "folder" {
                    self.query_sources.folders.insert(key, files);
                } else {
                    let bytes = files
                        .into_iter()
                        .next()
                        .map(|(_, b)| b)
                        .ok_or_else(|| bad("querySource: no file"))?;
                    self.query_sources.files.insert(key, bytes);
                }
                Ok(json!({ "ok": true, "files": n }))
            }
            QueryCommand::QueryInspect { source } => self.query_inspect(&source),
            QueryCommand::QueryPreview { query, upto, limit } => {
                let defs = self.defs()?;
                let r = self.resolver(&defs);
                let ev = Evaluator::new(&r);
                let limit = limit.unwrap_or(200);
                Ok(match ev.run(&query, upto) {
                    Ok(t) => table_json(&t, limit),
                    Err(e) => {
                        // The table before the failing step, to show with the error.
                        let partial = e
                            .step
                            .and_then(|s| Evaluator::new(&r).run(&query, Some(s)).ok());
                        let mut v = partial.map_or_else(
                            || table_json(&QTable::default(), 0),
                            |t| table_json(&t, limit),
                        );
                        v["error"] = error_json(&e);
                        v
                    }
                })
            }
            QueryCommand::QueryList => {
                let wb = self.wb()?;
                let defs = read_defs(wb);
                let list: Vec<Value> = defs
                    .iter()
                    .map(|q| {
                        let loaded = q.load.as_ref().and_then(|l| find_table(wb, &l.table));
                        let key = q.source.key();
                        json!({
                            "name": q.name,
                            "kind": source_kind(&q.source),
                            "key": key,
                            "access": q.source.access(),
                            "steps": q.steps.len(),
                            "table": q.load.as_ref().map(|l| &l.table),
                            "sheet": loaded.map(|(s, _)| s),
                            "sheetName": loaded.map(|(s, _)| wb.sheets[s].name.clone()),
                            "rows": loaded.map(|(s, t)| {
                                let t = &wb.sheets[s].tables[t];
                                t.range.last.row - t.range.first.row
                                    - t.totals_row_count.unwrap_or(0)
                            }),
                            "available": key.is_none_or(|k| self.query_sources.files.contains_key(k) || self.query_sources.folders.contains_key(k)),
                        })
                    })
                    .collect();
                Ok(json!({ "queries": list }))
            }
            QueryCommand::QueryGet { name } => {
                let defs = self.defs()?;
                let q = defs
                    .into_iter()
                    .find(|q| q.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| bad(format!("query: {name}")))?;
                Ok(serde_json::to_value(q).unwrap_or(Value::Null))
            }
            QueryCommand::QuerySave {
                query,
                original,
                load,
                sheet,
                row,
                col,
            } => self.query_save(
                query,
                original.as_deref(),
                load,
                sheet,
                CellRef::new(row, col),
            ),
            QueryCommand::QueryRefresh { names } => self.query_refresh(names.as_deref()),
            QueryCommand::QueryDelete { name, delete_table } => {
                let mut defs = self.defs()?;
                let i = defs
                    .iter()
                    .position(|q| q.name.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| bad(format!("query: {name}")))?;
                let q = defs.remove(i);
                let wb = self.wb()?;
                let target = q.load.as_ref().and_then(|l| find_table(wb, &l.table));
                let sheet = target.map_or(0, |(s, _)| s);
                self.edit(sheet, Recalc::All, |wb, _| {
                    if let (true, Some((s, ti))) = (delete_table, target) {
                        let range = wb.sheets[s].tables[ti].range;
                        clear_range(&mut wb.sheets[s], range);
                        wb.sheets[s].tables.remove(ti);
                    }
                    write_defs(wb, &defs);
                    Ok(())
                })?;
                Ok(json!({ "deleted": q.name }))
            }
            QueryCommand::QueryMashup => {
                let wb = self.wb()?;
                Ok(mashup_formulas(wb).map_or_else(
                    || json!({ "present": false }),
                    |f| json!({ "present": true, "formulas": f }),
                ))
            }
        }
    }

    fn query_inspect(&self, source: &Source) -> Result<Value> {
        let defs = self.defs()?;
        let r = self.resolver(&defs);
        let mut out = json!({});
        let bytes = source.key().and_then(|k| r.file(k));
        match source {
            Source::Csv { .. } => {
                if let Some(b) = bytes {
                    let d = bo_query::text::decode(b, None);
                    out["csv"] = json!({
                        "encoding": d.encoding,
                        "delimiter": bo_query::text::detect_delimiter(&d.text).to_string(),
                    });
                }
            }
            Source::Json { .. } => {
                if let Some(b) = bytes {
                    out["jsonPaths"] = json!(bo_query::json_src::array_paths(b));
                }
            }
            Source::Xml { .. } => {
                if let Some(b) = bytes {
                    out["xmlElements"] = json!(bo_query::xml_src::repeating_elements(b)
                        .into_iter()
                        .map(|(name, count)| json!({ "name": name, "count": count }))
                        .collect::<Vec<_>>());
                }
            }
            Source::Workbook { file, .. } => {
                if let Some(b) = bytes {
                    let wb = r.workbook(b, file).map_err(bad)?;
                    out["items"] = json!(bo_query::workbook_src::items(&wb)
                        .into_iter()
                        .map(|i| json!({ "name": i.name, "kind": i.kind }))
                        .collect::<Vec<_>>());
                }
            }
            Source::Web { .. } => {
                if let Some(b) = bytes {
                    let html = bo_query::text::decode(b, None).text;
                    out["tables"] = json!(bo_query::html::tables(&html)
                        .iter()
                        .enumerate()
                        .map(|(i, t)| json!({
                            "index": i,
                            "caption": t.caption,
                            "header": t.header,
                            "rows": t.rows.len(),
                            "cols": t.rows.iter().map(Vec::len).max().unwrap_or(0),
                            "preview": t.rows.iter().take(3).collect::<Vec<_>>(),
                        }))
                        .collect::<Vec<_>>());
                }
            }
            Source::Folder {
                folder, file_kind, ..
            } => {
                if let Some(files) = r.folder(folder) {
                    out["files"] = json!(files
                        .iter()
                        .map(|(n, _)| (*n).to_owned())
                        .filter(|n| bo_query::eval::kind_matches(file_kind, n))
                        .collect::<Vec<_>>());
                }
            }
            Source::Query { .. } => {}
        }
        let ev = Evaluator::new(&r);
        match ev.source(source) {
            Ok(t) => {
                let promote = matches!(
                    source,
                    Source::Csv { .. }
                        | Source::Web { .. }
                        | Source::Workbook {
                            item_kind: bo_query::ItemKind::Sheet,
                            ..
                        }
                );
                out["suggested"] = json!(bo_query::suggest_steps(&t, promote));
                out["preview"] = table_json(&t, 50);
            }
            Err(e) => {
                out["error"] = error_json(&bo_query::StepError {
                    step: None,
                    error: e,
                });
            }
        }
        Ok(out)
    }

    fn query_save(
        &mut self,
        mut query: Query,
        original: Option<&str>,
        load: LoadMode,
        sheet: usize,
        at: CellRef,
    ) -> Result<Value> {
        let name = query.name.trim().to_owned();
        if name.is_empty() {
            return Err(bad("query: empty name"));
        }
        query.name = name.clone();
        // The detected delimiter and encoding become part of the definition (the connection
        // part names them; a later file keeps being read the same way).
        if let Source::Csv {
            file,
            delimiter,
            encoding,
        } = &mut query.source
        {
            if let Some(b) = self.query_sources.files.get(file.as_str()) {
                let d = bo_query::text::decode(b, encoding.as_deref());
                if delimiter.is_none() {
                    *delimiter = Some(bo_query::text::detect_delimiter(&d.text).to_string());
                }
                if encoding.is_none() {
                    *encoding = Some(d.encoding);
                }
            }
        }
        let mut defs = self.defs()?;
        let prev = original.and_then(|o| defs.iter().position(|q| q.name.eq_ignore_ascii_case(o)));
        if defs
            .iter()
            .enumerate()
            .any(|(i, q)| Some(i) != prev && q.name.eq_ignore_ascii_case(&name))
        {
            return Err(bad(format!("query: duplicate name {name}")));
        }
        // An edited query keeps its table.
        let kept = prev.and_then(|i| defs[i].load.clone());
        query.load = kept.clone().or(query.load);
        match prev {
            Some(i) => defs[i] = query.clone(),
            None => defs.push(query.clone()),
        }
        let result = {
            let r = self.resolver(&defs);
            Evaluator::new(&r).run(&query, None)
        };
        let wb = self.wb()?;
        let table_target = query.load.as_ref().and_then(|l| find_table(wb, &l.table));
        if load == LoadMode::NewSheet && table_target.is_none() {
            super::protect::check_structure(wb)?;
        }
        let locale = self.locale;
        let edit_sheet = table_target.map_or(sheet, |(s, _)| s);
        let edit_sheet = if load == LoadMode::NewSheet && table_target.is_none() {
            0
        } else {
            edit_sheet
        };
        let (table, sheet_out, rows) = self.edit(edit_sheet, Recalc::All, |wb, _| {
            let t = match (&result, load, table_target) {
                (_, LoadMode::None, None) => None,
                (Ok(t), _, _) => Some(t),
                (Err(e), _, _) => {
                    return Err(bad(format!("query: {}", e.error)));
                }
            };
            let mut out = (None, None, 0usize);
            if let Some(t) = t {
                if let Some((s, ti)) = table_target {
                    let cols = query.load.as_ref().map_or(&[][..], |l| &l.columns[..]);
                    reload(wb, s, ti, t, cols, &locale)?;
                    let tname = wb.sheets[s].tables[ti].name.clone();
                    if let Some(q) = defs.iter_mut().find(|q| q.name == name) {
                        q.load = Some(load_target(tname, t));
                    }
                    out = (
                        Some(wb.sheets[s].tables[ti].name.clone()),
                        Some(s),
                        t.rows.len(),
                    );
                } else {
                    let (s, at) = if load == LoadMode::NewSheet {
                        let sname = sheet_name_for(wb, &name);
                        let mut ws = Worksheet::new(&sname);
                        ws.sheet_id = wb.sheets.iter().map(|w| w.sheet_id).max().unwrap_or(0) + 1;
                        wb.sheets.push(ws);
                        (wb.sheets.len() - 1, CellRef::new(0, 0))
                    } else {
                        (sheet, at)
                    };
                    let tname = load_new(wb, s, at, t, &name, &locale)?;
                    if let Some(q) = defs.iter_mut().find(|q| q.name == name) {
                        q.load = Some(load_target(tname.clone(), t));
                    }
                    out = (Some(tname), Some(s), t.rows.len());
                }
            }
            write_defs(wb, &defs);
            Ok(out)
        })?;
        // A new sheet needs its geometry.
        self.invalidate(None);
        Ok(json!({ "name": name, "table": table, "sheet": sheet_out, "rows": rows }))
    }

    fn query_refresh(&mut self, names: Option<&[String]>) -> Result<Value> {
        let defs = self.defs()?;
        let chosen: Vec<&Query> = defs
            .iter()
            .filter(|q| {
                names
                    .as_ref()
                    .is_none_or(|n| n.iter().any(|x| x.eq_ignore_ascii_case(&q.name)))
            })
            .collect();
        if let Some(n) = names {
            if let Some(missing) = n
                .iter()
                .find(|x| !defs.iter().any(|q| q.name.eq_ignore_ascii_case(x)))
            {
                return Err(bad(format!("query: {missing}")));
            }
        }
        let mut results = Vec::new();
        let mut errors = Vec::new();
        {
            let r = self.resolver(&defs);
            let ev = Evaluator::new(&r);
            let wb = self.wb()?;
            for q in &chosen {
                let Some(target) = q.load.as_ref().and_then(|l| find_table(wb, &l.table)) else {
                    continue;
                };
                match ev.run(q, None) {
                    Ok(t) => results.push((q.name.clone(), target, t)),
                    Err(e) => {
                        let mut v = error_json(&e);
                        v["name"] = json!(q.name);
                        errors.push(v);
                    }
                }
            }
        }
        let mut refreshed = Vec::new();
        if !results.is_empty() {
            let locale = self.locale;
            let first = results[0].1 .0;
            let mut defs = defs.clone();
            self.edit(first, Recalc::All, |wb, _| {
                let mut columns_changed = false;
                for (name, (s, ti), t) in &results {
                    let q = defs.iter_mut().find(|q| &q.name == name);
                    let cols = q
                        .as_ref()
                        .and_then(|q| q.load.as_ref())
                        .map(|l| l.columns.clone())
                        .unwrap_or_default();
                    reload(wb, *s, *ti, t, &cols, &locale)?;
                    if let Some(l) = q.and_then(|q| q.load.as_mut()) {
                        if l.columns != t.columns {
                            l.columns.clone_from(&t.columns);
                            columns_changed = true;
                        }
                    }
                    refreshed.push(json!({ "name": name, "rows": t.rows.len() }));
                }
                if columns_changed {
                    write_defs(wb, &defs);
                }
                Ok(())
            })?;
            self.invalidate(None);
        }
        Ok(json!({ "refreshed": refreshed, "errors": errors }))
    }
}

fn source_kind(s: &Source) -> &'static str {
    match s {
        Source::Csv { .. } => "csv",
        Source::Json { .. } => "json",
        Source::Xml { .. } => "xml",
        Source::Workbook { .. } => "workbook",
        Source::Folder { .. } => "folder",
        Source::Web { .. } => "web",
        Source::Query { .. } => "query",
    }
}

// ---------------------------------------------------------------------------
// Standard parts on save
// ---------------------------------------------------------------------------

/// Write the connections part (§18.13) and the query table parts (§18.12) of the queries
/// with a standard connection form, replacing the ones BlueOffice wrote before and keeping
/// those of other software.
pub(crate) fn sync_ecma_parts(wb: &mut Workbook) {
    let defs = read_defs(wb);
    let conn_rel = wb
        .opaque
        .workbook_rels
        .iter()
        .find(|r| r.rel_type == persist::REL_CONNECTIONS && !r.external)
        .map(|r| r.target.clone());
    let had_ours = wb.sheets.iter().flat_map(|s| &s.tables).any(|t| {
        t.rels
            .iter()
            .any(|r| r.rel_type == persist::REL_QUERY_TABLE)
    });
    if defs.is_empty() && !had_ours && conn_rel.is_none() {
        return;
    }
    // 1. Take out the query tables BlueOffice wrote.
    let mut dropped: Vec<String> = Vec::new();
    for ws in &mut wb.sheets {
        for t in &mut ws.tables {
            let ours: Vec<String> = t
                .rels
                .iter()
                .filter(|r| r.rel_type == persist::REL_QUERY_TABLE)
                .filter(|r| {
                    wb.opaque.parts.iter().any(|p| {
                        p.name.eq_ignore_ascii_case(&r.target)
                            && persist::is_own_query_table(&p.data)
                    })
                })
                .map(|r| r.target.clone())
                .collect();
            if ours.is_empty() {
                continue;
            }
            t.rels.retain(|r| !ours.contains(&r.target));
            t.attrs.retain(|(k, _)| k != "tableType");
            for c in &mut t.columns {
                c.attrs.retain(|(k, _)| k != "queryTableFieldId");
            }
            dropped.extend(ours);
        }
    }
    wb.opaque
        .parts
        .retain(|p| !dropped.iter().any(|d| d.eq_ignore_ascii_case(&p.name)));
    // 2. The connections part without BlueOffice's connections.
    let stripped = conn_rel.as_ref().and_then(|name| {
        wb.opaque
            .parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map(|p| persist::strip_connections(&String::from_utf8_lossy(&p.data)))
    });
    let prefix = stripped
        .as_ref()
        .map_or("", |c| c.prefix.as_str())
        .to_owned();
    let mut next_id = stripped.as_ref().map_or(0, |c| c.max_id) + 1;
    let mut names: Vec<String> = stripped.as_ref().map_or_else(Vec::new, |c| c.names.clone());
    let mut ours = Vec::new();
    let mut qt_no = 1;
    for q in &defs {
        let Some((s, ti)) = q.load.as_ref().and_then(|l| find_table(wb, &l.table)) else {
            continue;
        };
        // A table already linked by other software stays as it is.
        if wb.sheets[s].tables[ti]
            .rels
            .iter()
            .any(|r| r.rel_type == persist::REL_QUERY_TABLE)
        {
            continue;
        }
        let mut cname = q.name.clone();
        let mut k = 2;
        while names.iter().any(|n| n.eq_ignore_ascii_case(&cname)) {
            cname = format!("{} ({k})", q.name);
            k += 1;
        }
        let id = next_id;
        let Some(xml) = persist::connection_xml(q, id, &cname, &prefix) else {
            continue;
        };
        next_id += 1;
        names.push(cname);
        ours.push(xml);
        let mut part = format!("/xl/queryTables/queryTable{qt_no}.xml");
        while wb
            .opaque
            .parts
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&part))
        {
            qt_no += 1;
            part = format!("/xl/queryTables/queryTable{qt_no}.xml");
        }
        qt_no += 1;
        let t = &mut wb.sheets[s].tables[ti];
        // Columns the user added are not bound to the query (§18.12.4 dataBound).
        let bound = q.load.as_ref().map_or(&[][..], |l| &l.columns[..]);
        let fields: Vec<persist::QueryField> = t
            .columns
            .iter()
            .map(|c| {
                let b = bound.is_empty() || bound.iter().any(|n| n.eq_ignore_ascii_case(&c.name));
                (c.id, c.name.clone(), b)
            })
            .collect();
        t.attrs.retain(|(k, _)| k != "tableType");
        t.attrs.push(("tableType".into(), "queryTable".into()));
        for (k, c) in t.columns.iter_mut().enumerate() {
            c.attrs.retain(|(a, _)| a != "queryTableFieldId");
            c.attrs
                .push(("queryTableFieldId".into(), (k + 1).to_string()));
        }
        let mut rid = 1;
        while t.rels.iter().any(|r| r.id == format!("rId{rid}")) {
            rid += 1;
        }
        t.rels.push(OpaqueRel {
            id: format!("rId{rid}"),
            rel_type: persist::REL_QUERY_TABLE.into(),
            target: part.clone(),
            external: false,
        });
        wb.opaque.parts.push(OpaquePart {
            name: part,
            content_type: Some(persist::CT_QUERY_TABLE.into()),
            data: persist::query_table_xml(q, id, &fields).into_bytes(),
        });
    }
    // 3. Write (or drop) the connections part.
    match (
        persist::connections_part(stripped.as_ref(), &ours),
        conn_rel,
    ) {
        (Some(xml), Some(name)) => {
            if let Some(p) = wb
                .opaque
                .parts
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(&name))
            {
                if p.data != xml.as_bytes() {
                    p.data = xml.into_bytes();
                }
            }
        }
        (Some(xml), None) => {
            let name = "/xl/connections.xml".to_owned();
            wb.opaque.parts.push(OpaquePart {
                name: name.clone(),
                content_type: Some(persist::CT_CONNECTIONS.into()),
                data: xml.into_bytes(),
            });
            let mut id = "rIdBoConnections".to_owned();
            while wb.opaque.workbook_rels.iter().any(|r| r.id == id) {
                id.push('x');
            }
            wb.opaque.workbook_rels.push(OpaqueRel {
                id,
                rel_type: persist::REL_CONNECTIONS.into(),
                target: name,
                external: false,
            });
        }
        (None, Some(name)) => {
            wb.opaque
                .parts
                .retain(|p| !p.name.eq_ignore_ascii_case(&name));
            wb.opaque.workbook_rels.retain(|r| r.target != name);
        }
        (None, None) => {}
    }
}

// ---------------------------------------------------------------------------
// [MS-QDEFF] DataMashup (read only)
// ---------------------------------------------------------------------------

/// The M formulas (`Formulas/Section1.m`) of a DataMashup custom XML part ([MS-QDEFF]
/// §2.2: base64 of Version, PackagePartsLength, PackageParts = an OPC package, …).
pub(crate) fn mashup_formulas(wb: &Workbook) -> Option<String> {
    let part = wb.opaque.parts.iter().find(|p| {
        p.name.to_ascii_lowercase().starts_with("/customxml/item")
            && bo_query::text::decode(&p.data[..p.data.len().min(600) & !1], None)
                .text
                .contains("<DataMashup")
    })?;
    let text = decode_xml_text(&part.data)?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(text.split_whitespace().collect::<String>())
        .ok()?;
    if raw.len() < 8 {
        return None;
    }
    let len = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
    let zip = raw.get(8..8 + len)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).ok()?;
    let mut f = archive.by_name("Formulas/Section1.m").ok()?;
    if f.size() > 4 * 1024 * 1024 {
        return None;
    }
    let mut s = String::new();
    std::io::Read::read_to_string(&mut f, &mut s).ok()?;
    Some(s)
}

/// Text content of the root element of an XML part (UTF-8 or UTF-16 with BOM).
fn decode_xml_text(data: &[u8]) -> Option<String> {
    let text = bo_query::text::decode(data, None).text;
    let start = text.find("<DataMashup")?;
    let open_end = start + text[start..].find('>')? + 1;
    let close = text.rfind("</")?;
    (close >= open_end).then(|| text[open_end..close].to_owned())
}

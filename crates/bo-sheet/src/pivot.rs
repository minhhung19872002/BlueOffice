//! PivotTables (ECMA-376 Part 1, §18.10): a pivot cache built from a source
//! range, and the report written into the sheet as ordinary cells.
//!
//! The report uses a tabular layout without subtotals: report filters (page
//! fields, §18.10.1 `pageFields`) above the body, one column per row field,
//! one header row per column field (plus one for value names when there are
//! several values), grand totals on both axes. The [`PivotTable`] summary keeps
//! `location` and the `firstHeaderRow` / `firstDataRow` / `firstDataCol`
//! offsets (§18.10.1.49) in step with the cells written.

use crate::cells::resolve_ref;
use bo_model::sheet::{
    pivot_item_cmp, Cell, CellRange, CellRef, CellValue, CellXf, PivotCache, PivotCacheField,
    PivotDataField, PivotFilter, PivotFunction, PivotItemKey, PivotTable, Workbook, Worksheet,
    MAX_COLS, MAX_ROWS,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Maximum number of source cells read into a cache.
const MAX_SOURCE_CELLS: u64 = 4_000_000;
/// Maximum number of report cells.
const MAX_REPORT_CELLS: u64 = 2_000_000;

/// Captions written into report cells (localized by the host).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PivotLabels {
    pub grand_total: String,
    /// `dataCaption` and the header of several value fields.
    pub values: String,
    /// Report filter showing every item.
    pub all: String,
    /// Report filter showing several items.
    pub multiple: String,
    /// Label of the blank item.
    pub blank: String,
    /// Header of an unnamed source column (followed by its number).
    pub column: String,
    /// Prefix of new pivot table names (followed by a number).
    pub name: String,
}

impl Default for PivotLabels {
    fn default() -> Self {
        PivotLabels {
            grand_total: "Grand Total".into(),
            values: "Values".into(),
            all: "(All)".into(),
            multiple: "(Multiple Items)".into(),
            blank: "(blank)".into(),
            column: "Column".into(),
            name: "Pivot".into(),
        }
    }
}

/// Fields placed in the areas of a pivot table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PivotSpec {
    pub rows: Vec<u32>,
    pub cols: Vec<u32>,
    pub values: Vec<PivotDataField>,
    pub filters: Vec<PivotFilter>,
}

impl PivotSpec {
    /// The areas of an existing pivot table.
    pub fn of(pt: &PivotTable) -> PivotSpec {
        PivotSpec {
            rows: pt.rows.clone(),
            cols: pt.cols.clone(),
            values: pt.values.clone(),
            filters: pt.filters.clone(),
        }
    }
}

/// Plain text of a value (headers, item labels in the field list).
pub fn value_text(v: &CellValue) -> String {
    match v {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{n:.0}")
            } else {
                format!("{n}")
            }
        }
        CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        CellValue::Error(e) => e.as_str().to_owned(),
        CellValue::Date(d) => d.clone(),
        other => other.as_text().unwrap_or_default(),
    }
}

/// Field names (header row) and records of a source range.
pub fn source_data(
    ws: &Worksheet,
    range: CellRange,
    labels: &PivotLabels,
) -> Result<(Vec<String>, Vec<Vec<CellValue>>), String> {
    if u64::from(range.rows()) * u64::from(range.cols()) > MAX_SOURCE_CELLS {
        return Err("source range too large".into());
    }
    let mut names: Vec<String> = Vec::with_capacity(range.cols() as usize);
    for c in range.first.col..=range.last.col {
        let raw = ws
            .cell(range.first.row, c)
            .map(|cell| value_text(&cell.value))
            .unwrap_or_default();
        let base = if raw.trim().is_empty() {
            format!("{}{}", labels.column, c - range.first.col + 1)
        } else {
            raw.trim().to_owned()
        };
        // Cache field names are unique (§18.10.1.3).
        let mut name = base.clone();
        let mut n = 2;
        while names
            .iter()
            .any(|x| x.to_lowercase() == name.to_lowercase())
        {
            name = format!("{base}{n}");
            n += 1;
        }
        names.push(name);
    }
    let mut records = Vec::new();
    for r in range.first.row + 1..=range.last.row {
        let rec: Vec<CellValue> = (range.first.col..=range.last.col)
            .map(|c| match ws.cell(r, c).map(|cell| &cell.value) {
                Some(CellValue::RichText(_)) => CellValue::Text(
                    ws.cell(r, c)
                        .and_then(|x| x.value.as_text())
                        .unwrap_or_default(),
                ),
                Some(v) => v.clone(),
                None => CellValue::Empty,
            })
            .collect();
        records.push(rec);
    }
    // Trailing blank rows of a whole-column selection are not records.
    while records
        .last()
        .is_some_and(|r| r.iter().all(|v| matches!(v, CellValue::Empty)))
    {
        records.pop();
    }
    Ok((names, records))
}

/// Cache fields with their distinct items in display order.
pub fn cache_fields(names: &[String], records: &[Vec<CellValue>]) -> Vec<PivotCacheField> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut seen: HashSet<PivotItemKey> = HashSet::new();
            let mut items = Vec::new();
            for rec in records {
                let v = rec.get(i).cloned().unwrap_or_default();
                if seen.insert(PivotItemKey::of(&v)) {
                    items.push(v);
                }
            }
            items.sort_by(pivot_item_cmp);
            PivotCacheField {
                name: name.clone(),
                items,
            }
        })
        .collect()
}

/// Running aggregate of one data cell.
#[derive(Debug, Clone, Copy)]
struct Acc {
    count: u64,
    nums: u64,
    sum: f64,
    sumsq: f64,
    prod: f64,
    min: f64,
    max: f64,
}

impl Default for Acc {
    fn default() -> Self {
        Acc {
            count: 0,
            nums: 0,
            sum: 0.0,
            sumsq: 0.0,
            prod: 1.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }
}

impl Acc {
    fn add(&mut self, v: &CellValue) {
        if matches!(v, CellValue::Empty) {
            return;
        }
        self.count += 1;
        if let CellValue::Number(n) = v {
            self.nums += 1;
            self.sum += n;
            self.sumsq += n * n;
            self.prod *= n;
            self.min = self.min.min(*n);
            self.max = self.max.max(*n);
        }
    }

    /// Result of `f` (§18.18.17); a group without values is blank.
    fn result(&self, f: PivotFunction) -> CellValue {
        use bo_model::sheet::CellError;
        if self.count == 0 {
            return CellValue::Empty;
        }
        let n = self.nums as f64;
        let var = |pop: bool| {
            let d = if pop { n } else { n - 1.0 };
            if d <= 0.0 {
                return None;
            }
            Some(((self.sumsq - self.sum * self.sum / n) / d).max(0.0))
        };
        let num = |x: Option<f64>| x.map_or(CellValue::Error(CellError::Div0), CellValue::Number);
        match f {
            PivotFunction::Sum => CellValue::Number(self.sum),
            PivotFunction::Count => CellValue::Number(self.count as f64),
            PivotFunction::CountNums => CellValue::Number(n),
            PivotFunction::Average => num((self.nums > 0).then(|| self.sum / n)),
            PivotFunction::Max => CellValue::Number(if self.nums > 0 { self.max } else { 0.0 }),
            PivotFunction::Min => CellValue::Number(if self.nums > 0 { self.min } else { 0.0 }),
            PivotFunction::Product => {
                CellValue::Number(if self.nums > 0 { self.prod } else { 0.0 })
            }
            PivotFunction::Var => num(var(false)),
            PivotFunction::Varp => num(var(true)),
            PivotFunction::StdDev => num(var(false).map(f64::sqrt)),
            PivotFunction::StdDevp => num(var(true).map(f64::sqrt)),
        }
    }
}

/// A computed report: cells relative to the top-left of the page-field area.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// `(row, col, value)`.
    pub cells: Vec<(u32, u32, CellValue)>,
    /// Rows above the body taken by report filters (filters + one blank row).
    pub page_rows: u32,
    /// Body size.
    pub height: u32,
    pub width: u32,
    pub first_header_row: u32,
    pub first_data_row: u32,
    pub first_data_col: u32,
    /// Body rows drawn bold (headers, grand total).
    pub bold_rows: Vec<u32>,
    /// Body columns holding values: `(col, value index)`.
    pub data_cols: Vec<(u32, usize)>,
}

const TOTAL: u32 = u32::MAX;

/// Caption of a data field.
pub fn value_name(cache: &PivotCache, d: &PivotDataField) -> String {
    d.name.clone().unwrap_or_else(|| {
        let field = cache
            .fields
            .get(d.field as usize)
            .map_or("", |f| f.name.as_str());
        let f = d.function.as_str();
        let mut cap = f.chars();
        let f = cap
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + cap.as_str())
            .unwrap_or_default();
        format!("{f} of {field}")
    })
}

/// Compute the report of `spec` over `cache`.
pub fn layout(
    cache: &PivotCache,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<Layout, String> {
    let nf = cache.fields.len() as u32;
    let all_fields = spec
        .rows
        .iter()
        .chain(&spec.cols)
        .copied()
        .chain(spec.values.iter().map(|d| d.field))
        .chain(spec.filters.iter().map(|f| f.field));
    if all_fields.into_iter().any(|f| f >= nf) {
        return Err("unknown pivot field".into());
    }
    // Item index of every record value, per field.
    let index: Vec<HashMap<PivotItemKey, u32>> = cache
        .fields
        .iter()
        .map(|f| {
            f.items
                .iter()
                .enumerate()
                .map(|(i, v)| (PivotItemKey::of(v), i as u32))
                .collect()
        })
        .collect();
    let item_of = |rec: &[CellValue], f: u32| -> u32 {
        let v = rec.get(f as usize).unwrap_or(&CellValue::Empty);
        index[f as usize]
            .get(&PivotItemKey::of(v))
            .copied()
            .unwrap_or(0)
    };
    let filters: Vec<(u32, Vec<PivotItemKey>)> = spec
        .filters
        .iter()
        .filter(|f| !f.selected.is_empty())
        .map(|f| (f.field, f.selected.iter().map(PivotItemKey::of).collect()))
        .collect();
    let k = spec.values.len();
    let mut row_keys: BTreeMap<Vec<u32>, u32> = BTreeMap::new();
    let mut col_keys: BTreeMap<Vec<u32>, u32> = BTreeMap::new();
    let mut groups: Vec<(Vec<u32>, Vec<u32>, usize)> = Vec::new();
    for (ri, rec) in cache.records.iter().enumerate() {
        let shown = filters.iter().all(|(f, keys)| {
            let v = rec.get(*f as usize).unwrap_or(&CellValue::Empty);
            keys.contains(&PivotItemKey::of(v))
        });
        if !shown {
            continue;
        }
        let rk: Vec<u32> = spec.rows.iter().map(|f| item_of(rec, *f)).collect();
        let ck: Vec<u32> = spec.cols.iter().map(|f| item_of(rec, *f)).collect();
        row_keys.entry(rk.clone()).or_insert(0);
        col_keys.entry(ck.clone()).or_insert(0);
        groups.push((rk, ck, ri));
    }
    for (i, v) in row_keys.values_mut().enumerate() {
        *v = i as u32;
    }
    for (i, v) in col_keys.values_mut().enumerate() {
        *v = i as u32;
    }
    let mut acc: HashMap<(u32, u32), Vec<Acc>> = HashMap::new();
    for (rk, ck, ri) in &groups {
        let (r, c) = (row_keys[rk], col_keys[ck]);
        let rec = &cache.records[*ri];
        for key in [(r, c), (r, TOTAL), (TOTAL, c), (TOTAL, TOTAL)] {
            let slot = acc.entry(key).or_insert_with(|| vec![Acc::default(); k]);
            for (vi, d) in spec.values.iter().enumerate() {
                slot[vi].add(rec.get(d.field as usize).unwrap_or(&CellValue::Empty));
            }
        }
    }
    let m = spec.rows.len() as u32;
    let n = spec.cols.len() as u32;
    let item_value = |f: u32, x: u32| -> CellValue {
        match cache.fields[f as usize].items.get(x as usize) {
            None | Some(CellValue::Empty) => CellValue::Text(labels.blank.clone()),
            Some(v) => v.clone(),
        }
    };
    let names: Vec<String> = spec.values.iter().map(|d| value_name(cache, d)).collect();
    let caption = if k == 1 {
        names[0].clone()
    } else {
        labels.values.clone()
    };
    let lc = if m == 0 { u32::from(n > 0) } else { m };
    // Data columns: (column key or TOTAL, value index).
    let col_list: Vec<(Vec<u32>, u32)> = col_keys.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let mut columns: Vec<(u32, usize)> = Vec::new();
    if k > 0 {
        if n == 0 {
            columns.extend((0..k).map(|v| (0, v)));
        } else {
            for (_, ci) in &col_list {
                columns.extend((0..k).map(|v| (*ci, v)));
            }
            columns.extend((0..k).map(|v| (TOTAL, v)));
        }
    }
    let hr = if n == 0 { 1 } else { 1 + n + u32::from(k > 1) };
    let row_list: Vec<(Vec<u32>, u32)> = if m == 0 {
        vec![(Vec::new(), 0)]
    } else {
        row_keys.iter().map(|(k, v)| (k.clone(), *v)).collect()
    };
    let total_row = m > 0;
    let height = hr + row_list.len() as u32 + u32::from(total_row);
    let width = (lc + columns.len() as u32).max(1);
    if u64::from(height) * u64::from(width) > MAX_REPORT_CELLS {
        return Err("pivot table too large".into());
    }
    let p = spec.filters.len() as u32;
    let page_rows = if p > 0 { p + 1 } else { 0 };
    let mut out = Layout {
        page_rows,
        height,
        width,
        first_header_row: u32::from(n > 0),
        first_data_row: hr,
        first_data_col: lc,
        ..Layout::default()
    };
    let put = |out: &mut Layout, r: u32, c: u32, v: CellValue| {
        if !matches!(v, CellValue::Empty) {
            out.cells.push((page_rows + r, c, v));
        }
    };
    // Report filters (page fields).
    for (i, f) in spec.filters.iter().enumerate() {
        let i = i as u32;
        out.cells.push((
            i,
            0,
            CellValue::Text(cache.fields[f.field as usize].name.clone()),
        ));
        let sel = match f.selected.as_slice() {
            [] => CellValue::Text(labels.all.clone()),
            [CellValue::Empty] => CellValue::Text(labels.blank.clone()),
            [one] => one.clone(),
            _ => CellValue::Text(labels.multiple.clone()),
        };
        out.cells.push((i, 1, sel));
    }
    // Header rows.
    let field_name = |f: u32| CellValue::Text(cache.fields[f as usize].name.clone());
    if n > 0 {
        put(&mut out, 0, 0, CellValue::Text(caption.clone()));
        put(&mut out, 0, lc, field_name(spec.cols[0]));
        for (j, _) in spec.cols.iter().enumerate() {
            let row = 1 + j as u32;
            for (x, (ck, vi)) in columns.iter().enumerate() {
                let col = lc + x as u32;
                if *vi != 0 {
                    continue;
                }
                let v = if *ck == TOTAL {
                    if j == 0 {
                        CellValue::Text(labels.grand_total.clone())
                    } else {
                        CellValue::Empty
                    }
                } else {
                    let key = &col_list[*ck as usize].0;
                    item_value(spec.cols[j], key[j])
                };
                put(&mut out, row, col, v);
            }
        }
        if k > 1 {
            for (x, (_, vi)) in columns.iter().enumerate() {
                put(
                    &mut out,
                    n + 1,
                    lc + x as u32,
                    CellValue::Text(names[*vi].clone()),
                );
            }
        }
    } else {
        for (x, (_, vi)) in columns.iter().enumerate() {
            put(
                &mut out,
                0,
                lc + x as u32,
                CellValue::Text(names[*vi].clone()),
            );
        }
    }
    for (l, f) in spec.rows.iter().enumerate() {
        put(&mut out, hr - 1, l as u32, field_name(*f));
    }
    out.bold_rows.extend(0..hr);
    // Body.
    for (i, (key, ri)) in row_list.iter().enumerate() {
        let row = hr + i as u32;
        if m == 0 {
            if n > 0 {
                put(&mut out, row, 0, CellValue::Text(caption.clone()));
            }
        } else {
            for (l, f) in spec.rows.iter().enumerate() {
                put(&mut out, row, l as u32, item_value(*f, key[l]));
            }
        }
        for (x, (ck, vi)) in columns.iter().enumerate() {
            let rk = if m == 0 { TOTAL } else { *ri };
            let ck = if n == 0 { TOTAL } else { *ck };
            let v = acc.get(&(rk, ck)).map_or(CellValue::Empty, |a| {
                a[*vi].result(spec.values[*vi].function)
            });
            put(&mut out, row, lc + x as u32, v);
        }
    }
    if total_row {
        let row = height - 1;
        put(
            &mut out,
            row,
            0,
            CellValue::Text(labels.grand_total.clone()),
        );
        for (x, (ck, vi)) in columns.iter().enumerate() {
            let ck = if n == 0 { TOTAL } else { *ck };
            let v = acc.get(&(TOTAL, ck)).map_or(CellValue::Empty, |a| {
                a[*vi].result(spec.values[*vi].function)
            });
            put(&mut out, row, lc + x as u32, v);
        }
        out.bold_rows.push(row);
    }
    out.data_cols = columns
        .iter()
        .enumerate()
        .map(|(x, (_, vi))| (lc + x as u32, *vi))
        .collect();
    Ok(out)
}

/// The sheet area a pivot table's cells occupy (body plus report filters).
pub fn occupied(pt: &PivotTable) -> CellRange {
    let p = pt.filters.len() as u32;
    let top = if p > 0 {
        pt.location.first.row.saturating_sub(p + 1)
    } else {
        pt.location.first.row
    };
    let right = if p > 0 {
        pt.location.last.col.max(pt.location.first.col + 1)
    } else {
        pt.location.last.col
    };
    CellRange::new(
        CellRef::new(top, pt.location.first.col),
        CellRef::new(pt.location.last.row, right.min(MAX_COLS - 1)),
    )
}

/// Source sheet and range of a cache.
pub fn cache_source(wb: &Workbook, cache: &PivotCache) -> Option<(usize, CellRange)> {
    if let Some(name) = &cache.source_name {
        if let Some(t) = wb.sheets.iter().enumerate().find_map(|(i, s)| {
            s.tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(name))
                .map(|t| (i, t.range))
        }) {
            return Some(t);
        }
        return resolve_ref(wb, 0, name);
    }
    let sheet = cache.source_sheet.as_deref()?;
    let si = wb
        .sheets
        .iter()
        .position(|s| s.name.to_lowercase() == sheet.to_lowercase())?;
    Some((si, CellRange::parse(cache.source_ref.as_deref()?)?))
}

/// Write the report of pivot `index` of `sheet` into its cells (clearing the
/// area it occupied before) and update its location.
fn render(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    labels: &PivotLabels,
) -> Result<(), String> {
    let pt = wb.sheets[sheet]
        .pivots
        .get(index)
        .cloned()
        .ok_or("no such pivot table")?;
    let cache = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pt.cache_id)
        .ok_or("pivot cache missing")?;
    let lay = layout(cache, &PivotSpec::of(&pt), labels)?;
    // Number formats of the value fields come from the first number of their column.
    let source = cache_source(wb, cache);
    let num_fmt: Vec<Option<u32>> = pt
        .values
        .iter()
        .map(|d| {
            let (si, r) = source?;
            let ws = wb.sheets.get(si)?;
            let col = r.first.col + d.field;
            (r.first.row + 1..=r.last.row).find_map(|row| {
                let c = ws.cell(row, col)?;
                matches!(c.value, CellValue::Number(_))
                    .then(|| wb.styles.xf(c.style).map(|x| x.num_fmt_id))
                    .flatten()
            })
        })
        .collect();
    let old = occupied(&pt);
    // The report starts at the old page-field area's top-left.
    let top = old.first;
    let body_top = top.row + lay.page_rows;
    if u64::from(body_top) + u64::from(lay.height) > u64::from(MAX_ROWS)
        || u64::from(top.col) + u64::from(lay.width) > u64::from(MAX_COLS)
    {
        return Err("pivot table does not fit in the sheet".into());
    }
    let styles: Vec<Option<u32>> = num_fmt
        .iter()
        .map(|f| {
            f.filter(|id| *id != 0).map(|id| {
                wb.styles.add_xf(CellXf {
                    num_fmt_id: id,
                    xf_id: Some(0),
                    apply_number_format: Some(true),
                    ..CellXf::default()
                })
            })
        })
        .collect();
    let ws = &mut wb.sheets[sheet];
    let new_area = CellRange::new(
        top,
        CellRef::new(
            body_top + lay.height - 1,
            top.col + lay.width.max(if lay.page_rows > 0 { 2 } else { 1 }) - 1,
        ),
    );
    for area in [old, new_area] {
        clear_area(ws, area);
    }
    let data_style: HashMap<u32, Option<u32>> = lay
        .data_cols
        .iter()
        .map(|(c, vi)| (*c, styles.get(*vi).copied().flatten()))
        .collect();
    for (r, c, v) in &lay.cells {
        let style = if *r >= lay.page_rows + lay.first_data_row {
            data_style.get(c).copied().flatten().unwrap_or(0)
        } else {
            0
        };
        ws.cells.insert(
            (top.row + r, top.col + c),
            Cell::new(v.clone()).with_style(style),
        );
    }
    let pt = &mut ws.pivots[index];
    pt.location = CellRange::new(
        CellRef::new(body_top, top.col),
        CellRef::new(body_top + lay.height - 1, top.col + lay.width - 1),
    );
    pt.first_header_row = lay.first_header_row;
    pt.first_data_row = lay.first_data_row;
    pt.first_data_col = lay.first_data_col;
    pt.data_on_rows = false;
    pt.data_caption.clone_from(&labels.values);
    pt.dirty = true;
    // Headers and the grand total row in bold; report filter names too.
    for r in &lay.bold_rows {
        let row = body_top + r;
        crate::style::apply_patch(
            wb,
            sheet,
            CellRange::new(
                CellRef::new(row, top.col),
                CellRef::new(row, top.col + lay.width - 1),
            ),
            &crate::style::StylePatch {
                bold: Some(true),
                ..crate::style::StylePatch::default()
            },
        );
    }
    if lay.page_rows > 0 {
        crate::style::apply_patch(
            wb,
            sheet,
            CellRange::new(top, CellRef::new(top.row + lay.page_rows - 2, top.col)),
            &crate::style::StylePatch {
                bold: Some(true),
                ..crate::style::StylePatch::default()
            },
        );
    }
    Ok(())
}

/// Create a pivot table over `source` (header row included) of `src_sheet`,
/// with its top-left (report filters included) at `dest` of `dest_sheet`.
/// Returns the index of the pivot table in the destination sheet.
#[allow(clippy::too_many_arguments)]
pub fn create_pivot(
    wb: &mut Workbook,
    src_sheet: usize,
    source: CellRange,
    dest_sheet: usize,
    dest: CellRef,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<usize, String> {
    let ws = wb.sheets.get(src_sheet).ok_or("no such sheet")?;
    if dest_sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    if source.rows() < 1 {
        return Err("empty source".into());
    }
    if dest_sheet == src_sheet && source.contains(dest) {
        return Err("the pivot table would overlap its source".into());
    }
    let (names, records) = source_data(ws, source, labels)?;
    let source_name = ws.name.clone();
    let cache_id = wb
        .pivot_caches
        .iter()
        .map(|c| c.cache_id + 1)
        .max()
        .unwrap_or(1)
        .max(1);
    wb.pivot_caches.push(PivotCache {
        cache_id,
        source_sheet: Some(source_name),
        source_ref: Some(source.to_string()),
        fields: cache_fields(&names, &records),
        records,
        dirty: true,
        ..PivotCache::default()
    });
    let existing: Vec<String> = wb
        .sheets
        .iter()
        .flat_map(|s| s.pivots.iter().map(|p| p.name.to_lowercase()))
        .collect();
    let mut n = 1;
    let name = loop {
        let name = format!("{}{n}", labels.name);
        if !existing.contains(&name.to_lowercase()) {
            break name;
        }
        n += 1;
    };
    let ws = &mut wb.sheets[dest_sheet];
    ws.pivots.push(PivotTable {
        name,
        cache_id,
        location: CellRange::single(dest),
        rows: spec.rows.clone(),
        cols: spec.cols.clone(),
        values: spec.values.clone(),
        filters: Vec::new(),
        data_caption: labels.values.clone(),
        dirty: true,
        ..PivotTable::default()
    });
    let index = ws.pivots.len() - 1;
    // Filters are set after placement so the page-field rows go below `dest`.
    let p = spec.filters.len() as u32;
    if p > 0 {
        let pt = &mut ws.pivots[index];
        pt.filters = spec.filters.clone();
        let body = CellRef::new(dest.row + p + 1, dest.col);
        pt.location = CellRange::single(body);
    }
    if let Err(e) = render(wb, dest_sheet, index, labels) {
        wb.sheets[dest_sheet].pivots.pop();
        wb.pivot_caches.retain(|c| c.cache_id != cache_id);
        return Err(e);
    }
    Ok(index)
}

/// Rebuild the cache of pivot `index` from its source and re-render every
/// pivot table using it (fields are matched by name).
pub fn refresh_pivot(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.pivots.get(index))
        .map(|p| p.cache_id)
        .ok_or("no such pivot table")?;
    let ci = wb
        .pivot_caches
        .iter()
        .position(|c| c.cache_id == cache_id)
        .ok_or("pivot cache missing")?;
    let (si, range) = cache_source(wb, &wb.pivot_caches[ci]).ok_or("pivot source not found")?;
    let (names, records) = source_data(&wb.sheets[si], range, labels)?;
    let old_names: Vec<String> = wb.pivot_caches[ci]
        .fields
        .iter()
        .map(|f| f.name.to_lowercase())
        .collect();
    let remap = |f: u32| -> Option<u32> {
        let old = old_names.get(f as usize)?;
        names
            .iter()
            .position(|n| n.to_lowercase() == *old)
            .map(|i| i as u32)
            .or_else(|| (old_names.is_empty() && (f as usize) < names.len()).then_some(f))
    };
    // Re-point every pivot table using the cache.
    let mut users = Vec::new();
    for (s, ws) in wb.sheets.iter_mut().enumerate() {
        for (i, pt) in ws.pivots.iter_mut().enumerate() {
            if pt.cache_id != cache_id {
                continue;
            }
            pt.rows = pt.rows.iter().filter_map(|f| remap(*f)).collect();
            pt.cols = pt.cols.iter().filter_map(|f| remap(*f)).collect();
            pt.values = pt
                .values
                .iter()
                .filter_map(|d| remap(d.field).map(|field| PivotDataField { field, ..d.clone() }))
                .collect();
            pt.filters = pt
                .filters
                .iter()
                .filter_map(|p| remap(p.field).map(|field| PivotFilter { field, ..p.clone() }))
                .collect();
            users.push((s, i));
        }
    }
    let cache = &mut wb.pivot_caches[ci];
    cache.fields = cache_fields(&names, &records);
    cache.records = records;
    cache.dirty = true;
    for (s, i) in users {
        render(wb, s, i, labels)?;
    }
    Ok(())
}

/// Change the areas of pivot `index` and re-render it. A cache read from a
/// file (records not loaded) is refreshed from its source first.
pub fn update_pivot(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.pivots.get(index))
        .map(|p| p.cache_id)
        .ok_or("no such pivot table")?;
    let needs_data = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == cache_id)
        .is_none_or(|c| !c.dirty);
    if needs_data {
        refresh_pivot(wb, sheet, index, labels)?;
    }
    let nf = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == cache_id)
        .map_or(0, |c| c.fields.len() as u32);
    let ok = spec
        .rows
        .iter()
        .chain(&spec.cols)
        .chain(spec.values.iter().map(|d| &d.field))
        .chain(spec.filters.iter().map(|f| &f.field))
        .all(|f| *f < nf);
    if !ok {
        return Err("unknown pivot field".into());
    }
    let old = wb.sheets[sheet].pivots[index].clone();
    // The body stays in place when report filters are added or removed; it
    // moves down only when the filters do not fit above it.
    let body = old.location.first;
    let p = spec.filters.len() as u32;
    let new_body_row = if p > 0 { body.row.max(p + 1) } else { body.row };
    // Clear the old area first (the new one may be smaller).
    let area = occupied(&old);
    {
        let ws = &mut wb.sheets[sheet];
        clear_area(ws, area);
        let pt = &mut ws.pivots[index];
        pt.rows = spec.rows.clone();
        pt.cols = spec.cols.clone();
        pt.values = spec.values.clone();
        pt.filters = spec.filters.clone();
        pt.location = CellRange::single(CellRef::new(new_body_row, body.col));
    }
    render(wb, sheet, index, labels)
}

/// Remove every cell of `area`.
fn clear_area(ws: &mut Worksheet, area: CellRange) {
    let keys: Vec<(u32, u32)> = ws
        .cells
        .range((area.first.row, 0)..=(area.last.row, MAX_COLS))
        .map(|(k, _)| *k)
        .filter(|(_, c)| *c >= area.first.col && *c <= area.last.col)
        .collect();
    for k in keys {
        ws.cells.remove(&k);
    }
}

/// The pivot table whose body or report filters contain a cell.
pub fn pivot_at(ws: &Worksheet, row: u32, col: u32) -> Option<usize> {
    ws.pivots
        .iter()
        .position(|p| occupied(p).contains(CellRef::new(row, col)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sales() -> Workbook {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.name = "Data".into();
        let rows: [(&str, &str, f64); 6] = [
            ("Bắc", "A", 10.0),
            ("Nam", "B", 5.0),
            ("Bắc", "B", 7.0),
            ("Trung", "A", 2.0),
            ("Nam", "A", 1.0),
            ("bắc", "A", 3.0),
        ];
        for (c, h) in ["Vùng", "Loại", "Doanh thu"].iter().enumerate() {
            ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
        }
        for (i, (a, b, v)) in rows.iter().enumerate() {
            let r = i as u32 + 1;
            ws.set_cell(r, 0, Cell::new(CellValue::Text((*a).into())));
            ws.set_cell(r, 1, Cell::new(CellValue::Text((*b).into())));
            ws.set_cell(r, 2, Cell::new(CellValue::Number(*v)));
        }
        wb.sheets.push(Worksheet::new("Pivot"));
        wb
    }

    fn src() -> CellRange {
        CellRange::parse("A1:C7").unwrap()
    }

    fn text(ws: &Worksheet, a1: &str) -> String {
        ws.cell_a1(a1)
            .map(|c| value_text(&c.value))
            .unwrap_or_default()
    }

    fn sum_of(field: u32, function: PivotFunction) -> PivotDataField {
        PivotDataField {
            field,
            function,
            name: None,
        }
    }

    #[test]
    fn rows_and_values() {
        let mut wb = sales();
        let spec = PivotSpec {
            rows: vec![0],
            values: vec![sum_of(2, PivotFunction::Sum)],
            ..PivotSpec::default()
        };
        let i = create_pivot(
            &mut wb,
            0,
            src(),
            1,
            CellRef::new(2, 0),
            &spec,
            &PivotLabels::default(),
        )
        .unwrap();
        let ws = &wb.sheets[1];
        let pt = &ws.pivots[i];
        assert_eq!(pt.location.to_string(), "A3:B7");
        assert_eq!(
            (pt.first_header_row, pt.first_data_row, pt.first_data_col),
            (0, 1, 1)
        );
        assert_eq!(text(ws, "A3"), "Vùng");
        assert_eq!(text(ws, "B3"), "Sum of Doanh thu");
        // Case-insensitive grouping: "Bắc" and "bắc" are one item (first seen kept).
        assert_eq!(
            (text(ws, "A4"), text(ws, "B4")),
            ("Bắc".into(), "20".into())
        );
        assert_eq!((text(ws, "A5"), text(ws, "B5")), ("Nam".into(), "6".into()));
        assert_eq!(
            (text(ws, "A6"), text(ws, "B6")),
            ("Trung".into(), "2".into())
        );
        assert_eq!(
            (text(ws, "A7"), text(ws, "B7")),
            ("Grand Total".into(), "28".into())
        );
        let cache = &wb.pivot_caches[0];
        assert_eq!(cache.source_ref.as_deref(), Some("A1:C7"));
        assert_eq!(cache.fields[0].items.len(), 3);
        assert_eq!(cache.records.len(), 6);
    }

    #[test]
    fn columns_filters_and_functions() {
        let mut wb = sales();
        let spec = PivotSpec {
            rows: vec![0],
            cols: vec![1],
            values: vec![sum_of(2, PivotFunction::Average)],
            filters: vec![PivotFilter {
                field: 1,
                selected: vec![CellValue::Text("A".into())],
            }],
        };
        create_pivot(
            &mut wb,
            0,
            src(),
            1,
            CellRef::new(0, 0),
            &spec,
            &PivotLabels::default(),
        )
        .unwrap();
        let ws = &wb.sheets[1];
        let pt = &ws.pivots[0];
        // Filter row, blank row, then the body.
        assert_eq!(
            (text(ws, "A1"), text(ws, "B1")),
            ("Loại".into(), "A".into())
        );
        assert_eq!(pt.location.first, CellRef::new(2, 0));
        assert_eq!(text(ws, "A3"), "Average of Doanh thu");
        assert_eq!(text(ws, "B3"), "Loại");
        assert_eq!(
            (text(ws, "B4"), text(ws, "C4")),
            ("A".into(), "Grand Total".into())
        );
        assert_eq!(text(ws, "A4"), "Vùng");
        assert_eq!(pt.location.to_string(), "A3:C8");
        // Bắc: A = (10 + 3) / 2.
        assert_eq!(
            (text(ws, "A5"), text(ws, "B5")),
            ("Bắc".into(), "6.5".into())
        );
        assert_eq!(text(ws, "A8"), "Grand Total");
        assert_eq!(text(ws, "B8"), "4");
        // Change to Count with no filter: refresh in place.
        let spec = PivotSpec {
            rows: vec![0],
            cols: vec![1],
            values: vec![sum_of(2, PivotFunction::Count)],
            filters: Vec::new(),
        };
        update_pivot(&mut wb, 1, 0, &spec, &PivotLabels::default()).unwrap();
        let ws = &wb.sheets[1];
        assert_eq!(
            ws.pivots[0].location.first,
            CellRef::new(2, 0),
            "body stays"
        );
        assert!(ws.cell_a1("A1").is_none(), "filter area cleared");
        assert_eq!(text(ws, "A3"), "Count of Doanh thu");
        assert_eq!(
            (text(ws, "C4"), text(ws, "D4")),
            ("B".into(), "Grand Total".into())
        );
        assert_eq!(text(ws, "C7"), "", "Trung has no B");
        assert_eq!(text(ws, "D8"), "6");
    }
    #[test]
    fn refresh_follows_source_changes() {
        let mut wb = sales();
        let spec = PivotSpec {
            rows: vec![1],
            values: vec![sum_of(2, PivotFunction::Max)],
            ..PivotSpec::default()
        };
        create_pivot(
            &mut wb,
            0,
            src(),
            1,
            CellRef::new(0, 0),
            &spec,
            &PivotLabels::default(),
        )
        .unwrap();
        wb.sheets[0].set_cell(1, 2, Cell::new(CellValue::Number(100.0)));
        refresh_pivot(&mut wb, 1, 0, &PivotLabels::default()).unwrap();
        let ws = &wb.sheets[1];
        assert_eq!((text(ws, "A2"), text(ws, "B2")), ("A".into(), "100".into()));
        assert_eq!(pivot_at(ws, 1, 1), Some(0));
        assert_eq!(pivot_at(ws, 10, 10), None);
    }

    #[test]
    fn several_values_without_rows() {
        let mut wb = sales();
        let spec = PivotSpec {
            values: vec![
                sum_of(2, PivotFunction::Sum),
                sum_of(0, PivotFunction::Count),
            ],
            ..PivotSpec::default()
        };
        create_pivot(
            &mut wb,
            0,
            src(),
            1,
            CellRef::new(0, 0),
            &spec,
            &PivotLabels::default(),
        )
        .unwrap();
        let ws = &wb.sheets[1];
        assert_eq!(ws.pivots[0].location.to_string(), "A1:B2");
        assert_eq!((text(ws, "A2"), text(ws, "B2")), ("28".into(), "6".into()));
    }
}

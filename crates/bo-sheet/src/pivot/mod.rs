//! PivotTables (ECMA-376 Part 1, §18.10): a pivot cache built from a source
//! range, and the report written into the sheet as ordinary cells.
//!
//! Report filters (page fields, §18.10.1 `pageFields`) sit above the body. The
//! body is computed by [`compute`] (subtotals, grand totals, compact / outline
//! / tabular layouts, grouping, calculated fields, "show values as", sorting
//! and filters) and formatted with the pivot style ([`style`]). The
//! [`PivotTable`] summary keeps `location`, the `firstHeaderRow` /
//! `firstDataRow` / `firstDataCol` offsets (§18.10.1.49) and the `rowItems` /
//! `colItems` lines in step with the cells written.

mod calc;
mod compute;
mod group;
pub mod style;

pub use compute::{ColKind, Layout, RowKind};

use crate::cells::resolve_ref;
use bo_formula::DateSystem;
use bo_model::sheet::{
    pivot_item_cmp, Cell, CellRange, CellRef, CellValue, CellXf, PivotCache, PivotCacheField,
    PivotDataField, PivotFieldFilter, PivotFieldGroup, PivotFieldOptions, PivotFilter,
    PivotFunction, PivotGroupBy, PivotItemKey, PivotLayout, PivotRangeGroup, PivotShowAs,
    PivotStyleInfo, PivotTable, Workbook, Worksheet, MAX_COLS, MAX_ROWS,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Maximum number of source cells read into a cache.
const MAX_SOURCE_CELLS: u64 = 4_000_000;

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
    /// Subtotal caption; `{item}` is replaced by the item.
    pub subtotal: String,
    /// Header of the row label column (compact layout).
    pub row_labels: String,
    /// Header above the column items (compact layout).
    pub col_labels: String,
    /// Short month names (date grouping), January first.
    pub months: Vec<String>,
    /// Quarter caption; `{n}` is replaced by 1–4.
    pub quarter: String,
    /// Names of the grouping fields added for years / quarters / months.
    pub years_field: String,
    pub quarters_field: String,
    pub months_field: String,
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
            subtotal: "{item} Total".into(),
            row_labels: "Row Labels".into(),
            col_labels: "Column Labels".into(),
            months: [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
            quarter: "Qtr{n}".into(),
            years_field: "Years".into(),
            quarters_field: "Quarters".into(),
            months_field: "Months".into(),
        }
    }
}

/// Fields placed in the areas of a pivot table and its report settings.
#[derive(Debug, Clone, PartialEq)]
pub struct PivotSpec {
    pub rows: Vec<u32>,
    pub cols: Vec<u32>,
    pub values: Vec<PivotDataField>,
    pub filters: Vec<PivotFilter>,
    /// Per cache field settings (subtotals, sorting, blank rows, …).
    pub fields: Vec<PivotFieldOptions>,
    pub layout: PivotLayout,
    pub row_grand_totals: bool,
    pub col_grand_totals: bool,
    pub field_filters: Vec<PivotFieldFilter>,
    pub style: Option<PivotStyleInfo>,
}

impl Default for PivotSpec {
    fn default() -> Self {
        PivotSpec {
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
            filters: Vec::new(),
            fields: Vec::new(),
            layout: PivotLayout::Tabular,
            row_grand_totals: true,
            col_grand_totals: true,
            field_filters: Vec::new(),
            style: None,
        }
    }
}

impl PivotSpec {
    /// The areas and settings of an existing pivot table.
    pub fn of(pt: &PivotTable) -> PivotSpec {
        PivotSpec {
            rows: pt.rows.clone(),
            cols: pt.cols.clone(),
            values: pt.values.clone(),
            filters: pt.filters.clone(),
            fields: pt.fields.clone(),
            layout: pt.layout,
            row_grand_totals: pt.row_grand_totals,
            col_grand_totals: pt.col_grand_totals,
            field_filters: pt.field_filters.clone(),
            style: pt.style.clone(),
        }
    }

    /// Copy the areas and settings into `pt`.
    pub fn apply(&self, pt: &mut PivotTable) {
        pt.rows.clone_from(&self.rows);
        pt.cols.clone_from(&self.cols);
        pt.values.clone_from(&self.values);
        pt.filters.clone_from(&self.filters);
        pt.fields.clone_from(&self.fields);
        pt.layout = self.layout;
        pt.row_grand_totals = self.row_grand_totals;
        pt.col_grand_totals = self.col_grand_totals;
        pt.field_filters.clone_from(&self.field_filters);
        pt.style.clone_from(&self.style);
    }

    /// Settings of cache field `f`.
    pub fn field(&self, f: u32) -> PivotFieldOptions {
        self.fields.get(f as usize).cloned().unwrap_or_default()
    }
}

/// The style of new pivot tables.
pub fn default_style() -> PivotStyleInfo {
    PivotStyleInfo {
        name: style::DEFAULT_STYLE.into(),
        show_row_headers: true,
        show_col_headers: true,
        show_row_stripes: false,
        show_col_stripes: false,
        show_last_column: true,
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

/// Number format of every source column: that of its first number (0 = General).
pub fn source_formats(wb: &Workbook, sheet: usize, range: CellRange) -> Vec<u32> {
    let Some(ws) = wb.sheets.get(sheet) else {
        return vec![0; range.cols() as usize];
    };
    (range.first.col..=range.last.col)
        .map(|col| {
            (range.first.row + 1..=range.last.row)
                .find_map(|row| {
                    let c = ws.cell(row, col)?;
                    matches!(c.value, CellValue::Number(_))
                        .then(|| wb.styles.xf(c.style).map(|x| x.num_fmt_id))
                        .flatten()
                })
                .unwrap_or(0)
        })
        .collect()
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
                ..PivotCacheField::default()
            }
        })
        .collect()
}

/// Items of a cache field as an axis shows them: its group captions when
/// grouped (§18.10.1.37 `groupItems`), otherwise its shared items.
pub fn axis_items(f: &PivotCacheField) -> &[CellValue] {
    match &f.group {
        Some(g) if g.range.is_some() || !g.discrete.is_empty() => &g.items,
        _ => &f.items,
    }
}

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

/// Compute the report of `spec` over `cache` (1900 date system).
pub fn layout(
    cache: &PivotCache,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<Layout, String> {
    layout_in(cache, spec, labels, DateSystem::Date1900)
}

/// Compute the report of `spec` over `cache` with the workbook's date system.
pub fn layout_in(
    cache: &PivotCache,
    spec: &PivotSpec,
    labels: &PivotLabels,
    sys: DateSystem,
) -> Result<Layout, String> {
    compute::Engine::new(cache, spec, labels, sys)?.layout()
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

fn date_system(wb: &Workbook) -> DateSystem {
    if wb.date1904 {
        DateSystem::Date1904
    } else {
        DateSystem::Date1900
    }
}

/// Number format of the value cells of `d` (§18.10.1.21 `numFmtId`): its own,
/// a percentage for ratio calculations, otherwise the source field's.
pub fn value_format(cache: &PivotCache, d: &PivotDataField) -> u32 {
    if let Some(id) = d.num_fmt_id {
        return id;
    }
    if d.show_as.is_percentage() {
        return 10; // 0.00% (§18.8.30 built-in id 10)
    }
    let counts = matches!(d.function, PivotFunction::Count | PivotFunction::CountNums);
    let plain = matches!(
        d.show_as,
        PivotShowAs::Index | PivotShowAs::RankAscending | PivotShowAs::RankDescending
    );
    if counts || plain {
        return 0;
    }
    cache
        .fields
        .get(d.field as usize)
        .map_or(0, |f| f.num_fmt_id)
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
    let lay = layout_in(cache, &PivotSpec::of(&pt), labels, date_system(wb))?;
    let value_fmt: Vec<u32> = pt.values.iter().map(|d| value_format(cache, d)).collect();
    let label_fmt: Vec<u32> = cache
        .fields
        .iter()
        .map(|f| {
            let grouped = f
                .group
                .as_ref()
                .is_some_and(|g| g.range.is_some() || !g.discrete.is_empty());
            if grouped {
                0
            } else {
                f.num_fmt_id
            }
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
    let mut xf_of: HashMap<u32, u32> = HashMap::new();
    let mut style_for = |wb: &mut Workbook, id: u32| -> u32 {
        if id == 0 {
            return 0;
        }
        *xf_of.entry(id).or_insert_with(|| {
            wb.styles.add_xf(CellXf {
                num_fmt_id: id,
                xf_id: Some(0),
                apply_number_format: Some(true),
                ..CellXf::default()
            })
        })
    };
    let data_style: HashMap<u32, u32> = lay
        .data_cols
        .iter()
        .map(|(c, vi)| (*c, style_for(wb, value_fmt.get(*vi).copied().unwrap_or(0))))
        .collect();
    let mut label_style: HashMap<(u32, u32), u32> = HashMap::new();
    for (r, c, f) in &lay.label_fields {
        let id = label_fmt.get(*f as usize).copied().unwrap_or(0);
        if id != 0 {
            let s = style_for(wb, id);
            label_style.insert((lay.page_rows + r, *c), s);
        }
    }
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
    for (r, c, v) in &lay.cells {
        let style = if *r >= lay.page_rows + lay.first_data_row && *c >= lay.first_data_col {
            data_style.get(c).copied().unwrap_or(0)
        } else {
            label_style.get(&(*r, *c)).copied().unwrap_or(0)
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
    pt.row_items.clone_from(&lay.row_items);
    pt.col_items.clone_from(&lay.col_items);
    pt.dirty = true;
    let style_info = pt.style.clone();
    let body = CellRef::new(body_top, top.col);
    style::apply(wb, sheet, body, &lay, style_info.as_ref());
    // Compact layout: inner item labels are indented (§18.10.1.73 `indent`).
    for (r, c, level) in &lay.indents {
        let at = CellRef::new(body_top + r, top.col + c);
        crate::style::apply_patch(
            wb,
            sheet,
            CellRange::single(at),
            &crate::style::StylePatch {
                indent: Some(*level),
                h_align: Some("left".into()),
                ..crate::style::StylePatch::default()
            },
        );
    }
    // Report filter names in bold.
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
    let formats = source_formats(wb, src_sheet, source);
    let cache_id = wb
        .pivot_caches
        .iter()
        .map(|c| c.cache_id + 1)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut fields = cache_fields(&names, &records);
    for (f, id) in fields.iter_mut().zip(formats) {
        f.num_fmt_id = id;
    }
    wb.pivot_caches.push(PivotCache {
        cache_id,
        source_sheet: Some(source_name),
        source_ref: Some(source.to_string()),
        fields,
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
    let mut pt = PivotTable {
        name,
        cache_id,
        location: CellRange::single(dest),
        data_caption: labels.values.clone(),
        dirty: true,
        ..PivotTable::default()
    };
    spec.apply(&mut pt);
    if pt.style.is_none() {
        pt.style = Some(default_style());
    }
    // Filters are set after placement so the page-field rows go below `dest`.
    let p = spec.filters.len() as u32;
    if p > 0 {
        pt.location = CellRange::single(CellRef::new(dest.row + p + 1, dest.col));
    }
    let ws = &mut wb.sheets[dest_sheet];
    ws.pivots.push(pt);
    let index = ws.pivots.len() - 1;
    if let Err(e) = render(wb, dest_sheet, index, labels) {
        wb.sheets[dest_sheet].pivots.pop();
        wb.pivot_caches.retain(|c| c.cache_id != cache_id);
        return Err(e);
    }
    Ok(index)
}

/// Every pivot table using cache `cache_id`: `(sheet, index)`.
fn users(wb: &Workbook, cache_id: u32) -> Vec<(usize, usize)> {
    wb.sheets
        .iter()
        .enumerate()
        .flat_map(|(s, ws)| {
            ws.pivots
                .iter()
                .enumerate()
                .filter(move |(_, p)| p.cache_id == cache_id)
                .map(move |(i, _)| (s, i))
        })
        .collect()
}

/// Re-point a pivot table after the cache fields changed (`remap`: old → new).
fn remap_pivot(pt: &mut PivotTable, remap: &dyn Fn(u32) -> Option<u32>, new_len: usize) {
    pt.rows = pt.rows.iter().filter_map(|f| remap(*f)).collect();
    pt.cols = pt.cols.iter().filter_map(|f| remap(*f)).collect();
    pt.values = pt
        .values
        .iter()
        .filter_map(|d| {
            remap(d.field).map(|field| PivotDataField {
                field,
                base_field: remap(d.base_field).unwrap_or(0),
                ..d.clone()
            })
        })
        .collect();
    pt.filters = pt
        .filters
        .iter()
        .filter_map(|p| remap(p.field).map(|field| PivotFilter { field, ..p.clone() }))
        .collect();
    pt.field_filters = pt
        .field_filters
        .iter()
        .filter_map(|p| remap(p.field).map(|field| PivotFieldFilter { field, ..p.clone() }))
        .collect();
    let mut fields = vec![PivotFieldOptions::default(); new_len];
    for (i, o) in pt.fields.iter().enumerate() {
        if let Some(n) = remap(i as u32) {
            if let Some(slot) = fields.get_mut(n as usize) {
                *slot = o.clone();
            }
        }
    }
    while fields.last() == Some(&PivotFieldOptions::default()) {
        fields.pop();
    }
    pt.fields = fields;
    let nv = pt.values.len() as u32;
    for o in &mut pt.fields {
        o.sort_by = o.sort_by.filter(|v| *v < nv);
    }
    pt.dirty = true;
}

/// Recompute the bounds (when automatic) and captions of every range group.
fn regroup(
    fields: &mut [PivotCacheField],
    records: &[Vec<CellValue>],
    labels: &PivotLabels,
    sys: DateSystem,
) {
    for (i, field) in fields.iter_mut().enumerate() {
        let Some(g) = field.group.clone() else {
            continue;
        };
        let Some(mut r) = g.range.clone() else {
            continue;
        };
        let base = g.base.unwrap_or(i as u32) as usize;
        let Some((lo, hi, ints, blank)) = group::value_bounds(
            records
                .iter()
                .map(|rec| rec.get(base).unwrap_or(&CellValue::Empty)),
            sys,
        ) else {
            continue;
        };
        let (lo, hi) = if r.dates {
            group::date_bounds(lo, hi)
        } else {
            (lo, hi)
        };
        if r.auto_start {
            r.start = lo;
        }
        if r.auto_end {
            r.end = hi;
        }
        let items = group::range_items(&r, ints, blank, labels, sys);
        field.group = Some(PivotFieldGroup {
            range: Some(r),
            items,
            ..g
        });
    }
}

/// Rebuild the cache of pivot `index` from its source and re-render every
/// pivot table using it (fields are matched by name; groups and calculated
/// fields are kept).
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
    refresh_cache(wb, cache_id, labels)
}

fn refresh_cache(wb: &mut Workbook, cache_id: u32, labels: &PivotLabels) -> Result<(), String> {
    let ci = wb
        .pivot_caches
        .iter()
        .position(|c| c.cache_id == cache_id)
        .ok_or("pivot cache missing")?;
    let (si, range) = cache_source(wb, &wb.pivot_caches[ci]).ok_or("pivot source not found")?;
    let (names, records) = source_data(&wb.sheets[si], range, labels)?;
    let formats = source_formats(wb, si, range);
    let sys = date_system(wb);
    let old = wb.pivot_caches[ci].fields.clone();
    let mut fields = cache_fields(&names, &records);
    for (f, id) in fields.iter_mut().zip(formats) {
        f.num_fmt_id = id;
    }
    // Old field index → new index: database fields by name, derived fields appended.
    let mut map: Vec<Option<u32>> = vec![None; old.len()];
    for (oi, of) in old.iter().enumerate().filter(|(_, f)| !f.derived) {
        let lower = of.name.to_lowercase();
        map[oi] = fields
            .iter()
            .position(|f| f.name.to_lowercase() == lower)
            .map(|i| i as u32);
    }
    for (oi, of) in old.iter().enumerate().filter(|(_, f)| f.derived) {
        map[oi] = Some(fields.len() as u32);
        fields.push(PivotCacheField {
            items: Vec::new(),
            ..of.clone()
        });
    }
    for (oi, of) in old.iter().enumerate().filter(|(_, f)| !f.derived) {
        if let Some(ni) = map[oi] {
            fields[ni as usize].group.clone_from(&of.group);
        }
    }
    for f in &mut fields {
        if let Some(g) = &mut f.group {
            g.base = g.base.and_then(|b| map.get(b as usize).copied().flatten());
            g.par = g.par.and_then(|b| map.get(b as usize).copied().flatten());
        }
    }
    regroup(&mut fields, &records, labels, sys);
    let identity = old.is_empty();
    let n_new = fields.len();
    let remap = |f: u32| -> Option<u32> {
        if identity {
            return ((f as usize) < n_new).then_some(f);
        }
        map.get(f as usize).copied().flatten()
    };
    let list = users(wb, cache_id);
    for (s, i) in &list {
        remap_pivot(&mut wb.sheets[*s].pivots[*i], &remap, n_new);
    }
    let cache = &mut wb.pivot_caches[ci];
    cache.fields = fields;
    cache.records = records;
    cache.dirty = true;
    for (s, i) in list {
        render(wb, s, i, labels)?;
    }
    Ok(())
}

/// Refresh every pivot cache in use (each once).
pub fn refresh_all(wb: &mut Workbook, labels: &PivotLabels) -> Result<usize, String> {
    let mut ids: Vec<u32> = wb
        .sheets
        .iter()
        .flat_map(|s| s.pivots.iter().map(|p| p.cache_id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    for id in &ids {
        refresh_cache(wb, *id, labels)?;
    }
    Ok(ids.len())
}

/// Load the records of a cache read from a file (its records part is opaque).
fn ensure_records(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    labels: &PivotLabels,
) -> Result<u32, String> {
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
        refresh_cache(wb, cache_id, labels)?;
    }
    Ok(cache_id)
}

fn cache_mut(wb: &mut Workbook, cache_id: u32) -> Result<&mut PivotCache, String> {
    wb.pivot_caches
        .iter_mut()
        .find(|c| c.cache_id == cache_id)
        .ok_or_else(|| "pivot cache missing".to_owned())
}

/// Change the areas and settings of pivot `index` and re-render it. A cache
/// read from a file (records not loaded) is refreshed from its source first.
pub fn update_pivot(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = ensure_records(wb, sheet, index, labels)?;
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
        .chain(spec.field_filters.iter().map(|f| &f.field))
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
        spec.apply(pt);
        pt.location = CellRange::single(CellRef::new(new_body_row, body.col));
    }
    render(wb, sheet, index, labels)
}

/// How to group the items of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupRequest {
    /// Remove the grouping (and the grouping fields built on the field).
    None,
    /// Date parts; the finest selected part groups the field itself, coarser
    /// parts become new grouping fields placed before it.
    Dates {
        years: bool,
        quarters: bool,
        months: bool,
        days: bool,
    },
    /// Numeric ranges; missing bounds follow the data.
    Numbers {
        start: Option<f64>,
        end: Option<f64>,
        interval: f64,
    },
}

/// Remove cache fields `gone` of cache `cache_id`, re-pointing its pivot tables.
fn remove_cache_fields(wb: &mut Workbook, cache_id: u32, gone: &[u32]) -> Result<(), String> {
    if gone.is_empty() {
        return Ok(());
    }
    let cache = cache_mut(wb, cache_id)?;
    let old_len = cache.fields.len();
    let mut map: Vec<Option<u32>> = Vec::with_capacity(old_len);
    let mut next = 0u32;
    for i in 0..old_len as u32 {
        if gone.contains(&i) {
            map.push(None);
        } else {
            map.push(Some(next));
            next += 1;
        }
    }
    let mut i = 0u32;
    cache.fields.retain(|_| {
        let keep = !gone.contains(&i);
        i += 1;
        keep
    });
    for f in &mut cache.fields {
        if let Some(g) = &mut f.group {
            g.base = g.base.and_then(|b| map.get(b as usize).copied().flatten());
            g.par = g.par.and_then(|b| map.get(b as usize).copied().flatten());
        }
    }
    // Records keep one value per database field; derived fields come last.
    cache.dirty = true;
    let n_new = next as usize;
    let remap = |f: u32| map.get(f as usize).copied().flatten();
    for (s, i) in users(wb, cache_id) {
        remap_pivot(&mut wb.sheets[s].pivots[i], &remap, n_new);
    }
    Ok(())
}

/// A field name not yet used by the cache.
fn unique_name(cache: &PivotCache, base: &str) -> String {
    let taken = |n: &str| {
        cache
            .fields
            .iter()
            .any(|f| f.name.to_lowercase() == n.to_lowercase())
    };
    if !taken(base) {
        return base.to_owned();
    }
    let mut n = 2;
    loop {
        let cand = format!("{base}{n}");
        if !taken(&cand) {
            return cand;
        }
        n += 1;
    }
}

/// Group (or ungroup) the items of cache field `field` of pivot `index`
/// (§18.10.1.27 `fieldGroup`). The grouping belongs to the cache, so every
/// pivot table sharing it is re-rendered.
pub fn group_field(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    field: u32,
    req: &GroupRequest,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = ensure_records(wb, sheet, index, labels)?;
    let sys = date_system(wb);
    {
        let cache = cache_mut(wb, cache_id)?;
        let f = cache
            .fields
            .get(field as usize)
            .ok_or("unknown pivot field")?;
        if f.derived || f.formula.is_some() {
            return Err("only a source field can be grouped".into());
        }
    }
    // Drop the grouping fields built on this field.
    let gone: Vec<u32> = {
        let cache = cache_mut(wb, cache_id)?;
        cache
            .fields
            .iter()
            .enumerate()
            .filter(|(i, f)| {
                *i as u32 != field
                    && f.derived
                    && f.formula.is_none()
                    && f.group.as_ref().is_some_and(|g| g.base == Some(field))
            })
            .map(|(i, _)| i as u32)
            .collect()
    };
    remove_cache_fields(wb, cache_id, &gone)?;
    let cache = cache_mut(wb, cache_id)?;
    let values: Vec<CellValue> = cache
        .records
        .iter()
        .map(|r| r.get(field as usize).cloned().unwrap_or_default())
        .collect();
    let mut added: Vec<u32> = Vec::new();
    match req {
        GroupRequest::None => {
            cache.fields[field as usize].group = None;
        }
        GroupRequest::Numbers {
            start,
            end,
            interval,
        } => {
            if interval.is_nan() || *interval <= 0.0 {
                return Err("the interval must be positive".into());
            }
            let (lo, hi, ints, blank) = group::value_bounds(values.iter(), sys)
                .ok_or("the field has non-numeric values")?;
            let r = PivotRangeGroup {
                group_by: PivotGroupBy::Range,
                auto_start: start.is_none(),
                auto_end: end.is_none(),
                start: start.unwrap_or(lo),
                end: end.unwrap_or(hi),
                interval: *interval,
                dates: false,
            };
            if r.end < r.start {
                return Err("the end is before the start".into());
            }
            let items = group::range_items(&r, ints, blank, labels, sys);
            cache.fields[field as usize].group = Some(PivotFieldGroup {
                base: Some(field),
                range: Some(r),
                items,
                ..PivotFieldGroup::default()
            });
        }
        GroupRequest::Dates {
            years,
            quarters,
            months,
            days,
        } => {
            let parts: Vec<PivotGroupBy> = [
                (PivotGroupBy::Years, *years),
                (PivotGroupBy::Quarters, *quarters),
                (PivotGroupBy::Months, *months),
                (PivotGroupBy::Days, *days),
            ]
            .into_iter()
            .filter(|(_, on)| *on)
            .map(|(p, _)| p)
            .collect();
            let Some((&finest, coarser)) = parts.split_last() else {
                return Err("choose at least one date part".into());
            };
            let (lo, hi, _, blank) = group::value_bounds(values.iter(), sys)
                .ok_or("the field has values that are not dates")?;
            let (lo, hi) = group::date_bounds(lo, hi);
            let range = |by: PivotGroupBy| PivotRangeGroup {
                group_by: by,
                auto_start: true,
                auto_end: true,
                start: lo,
                end: hi,
                interval: 1.0,
                dates: true,
            };
            // Coarser parts: new grouping fields (§18.10.1.27 `base`), each the
            // `par` of the next finer one.
            let first_new = cache.fields.len() as u32;
            for (j, by) in coarser.iter().enumerate() {
                let name = match by {
                    PivotGroupBy::Years => &labels.years_field,
                    PivotGroupBy::Quarters => &labels.quarters_field,
                    _ => &labels.months_field,
                };
                let name = unique_name(cache, name);
                let r = range(*by);
                let items = group::range_items(&r, true, blank, labels, sys);
                let par = (j > 0).then(|| first_new + j as u32 - 1);
                cache.fields.push(PivotCacheField {
                    name,
                    derived: true,
                    group: Some(PivotFieldGroup {
                        par,
                        base: Some(field),
                        range: Some(r),
                        items,
                        ..PivotFieldGroup::default()
                    }),
                    ..PivotCacheField::default()
                });
                added.push(first_new + j as u32);
            }
            let r = range(finest);
            let items = group::range_items(&r, true, blank, labels, sys);
            cache.fields[field as usize].group = Some(PivotFieldGroup {
                par: added.last().copied(),
                base: Some(field),
                range: Some(r),
                items,
                ..PivotFieldGroup::default()
            });
        }
    }
    cache.dirty = true;
    // Selections by value no longer match the new items; new grouping fields
    // go before the field on its axis.
    let list = users(wb, cache_id);
    for (s, i) in &list {
        let pt = &mut wb.sheets[*s].pivots[*i];
        for p in &mut pt.filters {
            if p.field == field {
                p.selected.clear();
            }
        }
        if let Some(o) = pt.fields.get_mut(field as usize) {
            o.hidden.clear();
        }
        pt.field_filters
            .retain(|f| f.field != field || !f.kind.is_caption());
        for axis in [&mut pt.rows, &mut pt.cols] {
            if let Some(pos) = axis.iter().position(|f| *f == field) {
                for (j, a) in added.iter().enumerate() {
                    axis.insert(pos + j, *a);
                }
            }
        }
        pt.dirty = true;
    }
    for (s, i) in list {
        let area = occupied(&wb.sheets[s].pivots[i]);
        clear_area(&mut wb.sheets[s], area);
        render(wb, s, i, labels)?;
    }
    Ok(())
}

/// Add (`field` = `None`) or change a calculated field (§18.10.1.3
/// `cacheField/@formula`). A new field is added to the values of pivot
/// `index`. Returns the cache field index.
pub fn set_calculated_field(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    field: Option<u32>,
    name: &str,
    formula: &str,
    labels: &PivotLabels,
) -> Result<u32, String> {
    let cache_id = ensure_records(wb, sheet, index, labels)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("the field needs a name".into());
    }
    let cache = cache_mut(wb, cache_id)?;
    if cache
        .fields
        .iter()
        .enumerate()
        .any(|(i, f)| f.name.to_lowercase() == name.to_lowercase() && Some(i as u32) != field)
    {
        return Err("a field with this name exists".into());
    }
    calc::compile(formula, &cache.fields)?;
    let text = formula.trim();
    let text = text.strip_prefix('=').unwrap_or(text).trim().to_owned();
    let fi = if let Some(f) = field {
        let fld = cache
            .fields
            .get_mut(f as usize)
            .filter(|x| x.formula.is_some())
            .ok_or("not a calculated field")?;
        fld.name = name.to_owned();
        fld.formula = Some(text);
        f
    } else {
        cache.fields.push(PivotCacheField {
            name: name.to_owned(),
            formula: Some(text),
            derived: true,
            ..PivotCacheField::default()
        });
        let f = cache.fields.len() as u32 - 1;
        let pt = &mut wb.sheets[sheet].pivots[index];
        pt.values.push(PivotDataField {
            field: f,
            ..PivotDataField::default()
        });
        f
    };
    cache_mut(wb, cache_id)?.dirty = true;
    for (s, i) in users(wb, cache_id) {
        let area = occupied(&wb.sheets[s].pivots[i]);
        clear_area(&mut wb.sheets[s], area);
        render(wb, s, i, labels)?;
    }
    Ok(fi)
}

/// Remove a calculated field from the cache (and every pivot table using it).
pub fn remove_calculated_field(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    field: u32,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = ensure_records(wb, sheet, index, labels)?;
    let ok = cache_mut(wb, cache_id)?
        .fields
        .get(field as usize)
        .is_some_and(|f| f.formula.is_some());
    if !ok {
        return Err("not a calculated field".into());
    }
    let areas: Vec<(usize, CellRange)> = users(wb, cache_id)
        .into_iter()
        .map(|(s, i)| (s, occupied(&wb.sheets[s].pivots[i])))
        .collect();
    for (s, a) in areas {
        clear_area(&mut wb.sheets[s], a);
    }
    remove_cache_fields(wb, cache_id, &[field])?;
    for (s, i) in users(wb, cache_id) {
        render(wb, s, i, labels)?;
    }
    Ok(())
}

/// The relationships part name of a part (OPC §9.3).
fn rels_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// Delete pivot `index` of `sheet`: its cells, its part (when read from a
/// file) and its cache when no other pivot table uses it.
pub fn delete_pivot(wb: &mut Workbook, sheet: usize, index: usize) -> Result<(), String> {
    let pt = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.pivots.get(index))
        .cloned()
        .ok_or("no such pivot table")?;
    let ws = &mut wb.sheets[sheet];
    clear_area(ws, occupied(&pt));
    ws.pivots.remove(index);
    if !pt.part.is_empty() {
        ws.rels.retain(|r| r.target != pt.part);
        let rels = rels_name(&pt.part);
        wb.opaque
            .parts
            .retain(|p| p.name != pt.part && p.name != rels);
    }
    if users(wb, pt.cache_id).is_empty() {
        if let Some(ci) = wb
            .pivot_caches
            .iter()
            .position(|c| c.cache_id == pt.cache_id)
        {
            let c = wb.pivot_caches.remove(ci);
            if !c.part.is_empty() {
                wb.opaque.workbook_rels.retain(|r| r.id != c.rel_id);
                let rels = rels_name(&c.part);
                wb.opaque.parts.retain(|p| {
                    p.name != c.part && p.name != rels && Some(&p.name) != c.records_part.as_ref()
                });
            }
        }
    }
    Ok(())
}

/// Move pivot `index` of `sheet` so that its top-left (report filters
/// included) is `dest` of `dest_sheet`. Returns its new index.
pub fn move_pivot(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    dest_sheet: usize,
    dest: CellRef,
    labels: &PivotLabels,
) -> Result<usize, String> {
    let pt = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.pivots.get(index))
        .cloned()
        .ok_or("no such pivot table")?;
    if dest_sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    let cache = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pt.cache_id)
        .ok_or("pivot cache missing")?;
    if let Some((si, r)) = cache_source(wb, cache) {
        if si == dest_sheet && r.contains(dest) {
            return Err("the pivot table would overlap its source".into());
        }
    }
    let area = occupied(&pt);
    clear_area(&mut wb.sheets[sheet], area);
    let mut moved = wb.sheets[sheet].pivots.remove(index);
    let p = moved.filters.len() as u32;
    let body = CellRef::new(dest.row + if p > 0 { p + 1 } else { 0 }, dest.col);
    moved.location = CellRange::single(body);
    moved.dirty = true;
    // A pivot table read from a file keeps its part; its relationship follows it.
    if dest_sheet != sheet && !moved.part.is_empty() {
        if let Some(pos) = wb.sheets[sheet]
            .rels
            .iter()
            .position(|r| r.target == moved.part)
        {
            let mut rel = wb.sheets[sheet].rels.remove(pos);
            let ids: HashSet<String> = wb.sheets[dest_sheet]
                .rels
                .iter()
                .map(|r| r.id.clone())
                .collect();
            let mut n = 1;
            while ids.contains(&format!("rIdPivot{n}")) {
                n += 1;
            }
            rel.id = format!("rIdPivot{n}");
            wb.sheets[dest_sheet].rels.push(rel);
        }
    }
    let ws = &mut wb.sheets[dest_sheet];
    ws.pivots.push(moved);
    let at = ws.pivots.len() - 1;
    render(wb, dest_sheet, at, labels)?;
    Ok(at)
}

/// Point the cache of pivot `index` at a new source range and refresh it.
pub fn change_source(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    src_sheet: usize,
    range: CellRange,
    labels: &PivotLabels,
) -> Result<(), String> {
    let cache_id = wb
        .sheets
        .get(sheet)
        .and_then(|s| s.pivots.get(index))
        .map(|p| p.cache_id)
        .ok_or("no such pivot table")?;
    let name = wb
        .sheets
        .get(src_sheet)
        .ok_or("no such sheet")?
        .name
        .clone();
    if range.rows() < 2 {
        return Err("empty source".into());
    }
    for (s, i) in users(wb, cache_id) {
        let p = &wb.sheets[s].pivots[i];
        if s == src_sheet && overlaps(occupied(p), range) {
            return Err("the pivot table would overlap its source".into());
        }
    }
    let cache = cache_mut(wb, cache_id)?;
    cache.source_sheet = Some(name);
    cache.source_ref = Some(range.to_string());
    cache.source_name = None;
    for (s, i) in users(wb, cache_id) {
        let area = occupied(&wb.sheets[s].pivots[i]);
        clear_area(&mut wb.sheets[s], area);
    }
    refresh_cache(wb, cache_id, labels)
}

fn overlaps(a: CellRange, b: CellRange) -> bool {
    a.first.row <= b.last.row
        && b.first.row <= a.last.row
        && a.first.col <= b.last.col
        && b.first.col <= a.last.col
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
mod tests;

//! Pivot tables over the data model.
//!
//! The cache of such a pivot table has one field per model column (`Table[Column]`, every
//! table in model order, calculated columns included) and one per measure (`[Measure]`). Its
//! records are rebuilt on every render: the rows of a **base table** — the table from which
//! every table the report uses is reached through active many-to-one relationships (a fact
//! table) — joined with the rows they relate to. Row, column and filter fields are therefore
//! ordinary cache fields for the pivot engine (subtotals, layouts, sorting, styles), plain
//! value fields aggregate the joined records, and a value field over a measure takes the
//! measure evaluated in the filter context of each report cell
//! ([`crate::pivot::compute::MeasureSource`]).
//!
//! The records are written as an ordinary pivot cache (§18.10.1.67–68, without
//! `refreshOnLoad`), so other software shows the report's values.

use super::eval::{Context, Evaluator};
use super::{DataModel, ModelData};
use crate::pivot::compute::{FieldFilter, MeasureSource};
use crate::pivot::{PivotLabels, PivotSpec};
use bo_model::sheet::{
    CellRange, CellRef, CellValue, PivotCache, PivotCacheField, PivotTable, SlicerSource, Workbook,
};
use std::collections::HashSet;

/// What a cache field of a model pivot table is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Column `1` of table `0`.
    Column(usize, usize),
    /// Measure (index in the definitions).
    Measure(usize),
}

/// Field names and kinds of a model pivot cache.
#[must_use]
pub fn field_list(md: &ModelData) -> Vec<(String, FieldKind)> {
    let mut out = Vec::new();
    for (t, td) in md.tables.iter().enumerate() {
        for (c, name) in td.columns.iter().enumerate() {
            out.push((format!("{}[{}]", td.name, name), FieldKind::Column(t, c)));
        }
    }
    for (mi, m) in md.def.measures.iter().enumerate() {
        out.push((format!("[{}]", m.name), FieldKind::Measure(mi)));
    }
    out
}

/// Cache `cache_id` is built from the data model.
#[must_use]
pub fn is_model_cache(wb: &Workbook, cache_id: u32) -> bool {
    super::defs_part(wb).is_some() && DataModel::read(wb).pivots.contains(&cache_id)
}

/// Forget cache `cache_id` (its last pivot table was deleted).
pub fn forget_cache(wb: &mut Workbook, cache_id: u32) {
    if super::defs_part(wb).is_none() {
        return;
    }
    let mut dm = DataModel::read(wb);
    if dm.pivots.contains(&cache_id) {
        dm.pivots.retain(|p| *p != cache_id);
        dm.write(wb);
    }
}

/// The loaded model of a pivot table and the kind of every cache field.
pub struct ModelPivot {
    /// Loaded model.
    pub md: ModelData,
    /// Kind of every cache field.
    pub kinds: Vec<FieldKind>,
}

impl MeasureSource for ModelPivot {
    fn is_measure(&self, field: u32) -> bool {
        matches!(self.kinds.get(field as usize), Some(FieldKind::Measure(_)))
    }

    fn measure(&self, field: u32, filters: &[FieldFilter]) -> CellValue {
        let Some(FieldKind::Measure(mi)) = self.kinds.get(field as usize) else {
            return CellValue::Empty;
        };
        let ctx = self.context(filters);
        Evaluator::new(&self.md).measure(*mi, &ctx).into_cell()
    }
}

impl ModelPivot {
    /// The filter context of report constraints on cache fields.
    #[must_use]
    pub(crate) fn context(&self, filters: &[FieldFilter]) -> Context {
        let mut ctx = Context::default();
        for f in filters {
            if let Some(FieldKind::Column(t, c)) = self.kinds.get(f.field as usize) {
                if f.exclude {
                    ctx.drop_values(*t, *c, &f.values);
                } else {
                    ctx.keep(*t, *c, &f.values);
                }
            }
        }
        ctx
    }

    /// Add the filters of table slicers on model tables to `spec` (as hidden items).
    pub fn effective_spec(&self, wb: &Workbook, spec: &mut PivotSpec, labels: &PivotLabels) {
        let locale = labels.locale.unwrap_or_default();
        for ws in &wb.sheets {
            for sl in &ws.slicers {
                let SlicerSource::Table {
                    table_id,
                    column,
                    shown: Some(shown),
                } = &sl.source
                else {
                    continue;
                };
                let Some((si, ti)) = wb.sheets.iter().enumerate().find_map(|(si, s)| {
                    s.tables
                        .iter()
                        .position(|t| t.id == *table_id)
                        .map(|ti| (si, ti))
                }) else {
                    continue;
                };
                let st = &wb.sheets[si].tables[ti];
                let Some(t) = self.md.table(&st.name) else {
                    continue;
                };
                let Some(ci) = st.columns.iter().position(|c| c.id == *column) else {
                    continue;
                };
                let Some(c) = self.md.tables[t].column(&st.columns[ci].name) else {
                    continue;
                };
                let Some(field) = self
                    .kinds
                    .iter()
                    .position(|k| *k == FieldKind::Column(t, c))
                else {
                    continue;
                };
                // Keys of the values whose shown text the slicer keeps.
                let header = st.header_row_count.unwrap_or(1);
                let totals = st.totals_row_count.unwrap_or(0);
                let col = st.range.first.col + ci as u32;
                let mut keep: HashSet<super::Key> = HashSet::new();
                let last = st.range.last.row.saturating_sub(totals);
                for r in st.range.first.row + header..=last {
                    let cell = wb.sheets[si].cells.get(&(r, col));
                    let text = cell
                        .map(|c| crate::render::format_cell(wb, c, &locale).text)
                        .unwrap_or_default();
                    if shown.contains(&text) {
                        keep.insert(cell.map_or(super::Key::Empty, |c| super::Key::of(&c.value)));
                    }
                }
                let Some(cache_field) = self.items_of(field) else {
                    continue;
                };
                if spec.fields.len() <= field {
                    spec.fields
                        .resize(field + 1, bo_model::sheet::PivotFieldOptions::default());
                }
                let hidden = &mut spec.fields[field].hidden;
                for v in cache_field {
                    if !keep.contains(&super::Key::of(&v))
                        && !hidden
                            .iter()
                            .any(|h| super::Key::of(h) == super::Key::of(&v))
                    {
                        hidden.push(v);
                    }
                }
            }
        }
    }

    /// Items of a cache field: the distinct values of its column.
    fn items_of(&self, field: usize) -> Option<Vec<CellValue>> {
        match self.kinds.get(field)? {
            FieldKind::Column(t, c) => {
                let mut items = crate::pivot::distinct_items(&self.md.tables[*t].rows, *c);
                if !items.iter().any(|v| matches!(v, CellValue::Empty)) {
                    items.push(CellValue::Empty);
                }
                Some(items)
            }
            FieldKind::Measure(_) => None,
        }
    }
}

/// Tables a measure reads (directly or through the measures it names).
fn measure_tables(md: &ModelData, mi: usize, out: &mut HashSet<usize>, seen: &mut HashSet<usize>) {
    if !seen.insert(mi) {
        return;
    }
    if let Some(Ok(e)) = md.measures.get(mi) {
        for t in e.tables() {
            if let Some(x) = md.table(t) {
                out.insert(x);
            }
        }
        for b in e.brackets() {
            if let Some(x) = md.def.measure(b) {
                measure_tables(md, x, out, seen);
            }
        }
    }
    if let Some(x) = md.def.measures.get(mi).and_then(|m| md.table(&m.table)) {
        if out.is_empty() {
            out.insert(x);
        }
    }
}

/// The base table of a report using `used` tables: the one reaching most of them through
/// active relationships, then a used table, then the one with most rows.
#[must_use]
pub fn base_table<S: std::hash::BuildHasher>(md: &ModelData, used: &HashSet<usize, S>) -> usize {
    let mut best = (0usize, false, 0usize, usize::MAX);
    for t in 0..md.tables.len() {
        let reach: HashSet<usize> = md.reachable(t, &[]).into_iter().map(|(u, _)| u).collect();
        let score = (
            used.iter().filter(|u| reach.contains(u)).count(),
            used.contains(&t),
            md.tables[t].rows.len(),
            usize::MAX - t,
        );
        if best.3 == usize::MAX || score > best {
            best = score;
        }
    }
    usize::MAX - best.3
}

/// Records of the base table joined with the rows it relates to (one value per field).
#[must_use]
pub fn join(md: &ModelData, base: usize, kinds: &[FieldKind]) -> Vec<Vec<CellValue>> {
    let Some(bt) = md.tables.get(base) else {
        return Vec::new();
    };
    let reach = md.reachable(base, &[]);
    // Row of every reached table for every base row.
    let maps: Vec<(usize, Vec<Option<u32>>)> = reach
        .iter()
        .map(|(u, path)| {
            let m = (0..bt.rows.len())
                .map(|r| {
                    let mut cur = Some(r as u32);
                    for ri in path {
                        cur = cur.and_then(|x| {
                            md.rels[*ri]
                                .as_ref()
                                .and_then(|rel| rel.map.get(x as usize).copied().flatten())
                        });
                    }
                    cur
                })
                .collect();
            (*u, m)
        })
        .collect();
    let cols: Vec<(usize, usize, usize)> = kinds
        .iter()
        .enumerate()
        .filter_map(|(fi, k)| match k {
            FieldKind::Column(t, c) => {
                let mi = maps.iter().position(|(u, _)| u == t)?;
                Some((fi, mi, *c))
            }
            FieldKind::Measure(_) => None,
        })
        .collect();
    (0..bt.rows.len())
        .map(|r| {
            let mut rec = vec![CellValue::Empty; kinds.len()];
            for &(fi, mi, c) in &cols {
                let (u, map) = &maps[mi];
                if let Some(x) = map[r] {
                    if let Some(v) = md.tables[*u]
                        .rows
                        .get(x as usize)
                        .and_then(|row| row.get(c))
                    {
                        rec[fi].clone_from(v);
                    }
                }
            }
            rec
        })
        .collect()
}

/// Fields of a pivot table's areas (and hidden items).
fn used_fields(pt: &PivotTable) -> Vec<u32> {
    let mut out: Vec<u32> = pt.rows.iter().chain(&pt.cols).copied().collect();
    out.extend(pt.values.iter().map(|d| d.field));
    out.extend(pt.filters.iter().map(|f| f.field));
    out.extend(pt.field_filters.iter().map(|f| f.field));
    out.extend(
        pt.fields
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.hidden.is_empty())
            .map(|(i, _)| i as u32),
    );
    out
}

/// Rebuild cache `cache_id` from the model for the pivot tables using it: fields (pivot
/// tables are re-pointed by field name when the model changed), base table, records,
/// items and number formats. Returns the loaded model.
///
/// # Errors
/// The cache is missing or the model has no tables.
pub fn sync(wb: &mut Workbook, cache_id: u32) -> Result<ModelPivot, String> {
    let md = ModelData::load(wb);
    if md.tables.is_empty() {
        return Err("the data model has no tables".into());
    }
    let list = field_list(&md);
    let names: Vec<String> = list.iter().map(|(n, _)| n.clone()).collect();
    let kinds: Vec<FieldKind> = list.iter().map(|(_, k)| *k).collect();
    let ci = wb
        .pivot_caches
        .iter()
        .position(|c| c.cache_id == cache_id)
        .ok_or("pivot cache missing")?;
    let old: Vec<String> = wb.pivot_caches[ci]
        .fields
        .iter()
        .map(|f| f.name.to_lowercase())
        .collect();
    let users = crate::pivot::users(wb, cache_id);
    if !old.is_empty() && old != names.iter().map(|n| n.to_lowercase()).collect::<Vec<_>>() {
        let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
        let map: Vec<Option<u32>> = old
            .iter()
            .map(|o| lower.iter().position(|n| n == o).map(|i| i as u32))
            .collect();
        let remap = |f: u32| map.get(f as usize).copied().flatten();
        for (s, i) in &users {
            crate::pivot::remap_pivot(&mut wb.sheets[*s].pivots[*i], &remap, names.len());
        }
    }
    // Tables the reports use.
    let mut used: HashSet<usize> = HashSet::new();
    let mut seen = HashSet::new();
    for (s, i) in &users {
        for f in used_fields(&wb.sheets[*s].pivots[*i]) {
            match kinds.get(f as usize) {
                Some(FieldKind::Column(t, _)) => {
                    used.insert(*t);
                }
                Some(FieldKind::Measure(mi)) => measure_tables(&md, *mi, &mut used, &mut seen),
                None => {}
            }
        }
    }
    let base = base_table(&md, &used);
    let records = join(&md, base, &kinds);
    let reached: HashSet<usize> = md
        .reachable(base, &[])
        .into_iter()
        .map(|(u, _)| u)
        .collect();
    let mut fields = Vec::with_capacity(kinds.len());
    for (fi, (name, kind)) in list.iter().enumerate() {
        let (items, fmt) = match kind {
            FieldKind::Column(t, c) => {
                let mut items = crate::pivot::distinct_items(&md.tables[*t].rows, *c);
                let blank = reached.contains(t)
                    && records.iter().any(|r| matches!(r[fi], CellValue::Empty));
                if blank && !items.iter().any(|v| matches!(v, CellValue::Empty)) {
                    items.push(CellValue::Empty);
                }
                (items, md.tables[*t].formats.get(*c).copied().unwrap_or(0))
            }
            FieldKind::Measure(mi) => {
                let fmt = md.def.measures[*mi]
                    .format
                    .as_deref()
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map_or(0, |code| crate::style::num_fmt_id(&mut wb.styles, code));
                (Vec::new(), fmt)
            }
        };
        fields.push(PivotCacheField {
            name: name.clone(),
            items,
            num_fmt_id: fmt,
            ..PivotCacheField::default()
        });
    }
    let base_name = md.tables[base].name.clone();
    let cache = &mut wb.pivot_caches[ci];
    cache.fields = fields;
    cache.records = records.into();
    cache.dirty = true;
    cache.model = true;
    cache.source_name = Some(base_name);
    cache.source_sheet = None;
    cache.source_ref = None;
    cache.calculated_items.clear();
    // Value fields over a measure are captioned with the measure's name.
    for (s, i) in &users {
        let pt = &mut wb.sheets[*s].pivots[*i];
        for d in &mut pt.values {
            if let (None, Some(FieldKind::Measure(mi))) = (&d.name, kinds.get(d.field as usize)) {
                d.name = Some(md.def.measures[*mi].name.clone());
            }
        }
    }
    Ok(ModelPivot { md, kinds })
}

/// Create a pivot table over the data model with its top-left at `dest` of `dest_sheet`
/// (empty areas). Returns its index on the sheet.
///
/// # Errors
/// No such sheet, or the model has no tables.
pub fn create_model_pivot(
    wb: &mut Workbook,
    dest_sheet: usize,
    dest: CellRef,
    spec: &PivotSpec,
    labels: &PivotLabels,
) -> Result<usize, String> {
    if dest_sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    let mut dm = DataModel::read(wb);
    if dm.tables.is_empty() {
        return Err("the data model has no tables".into());
    }
    let before = dm.clone();
    let cache_id = wb
        .pivot_caches
        .iter()
        .map(|c| c.cache_id + 1)
        .max()
        .unwrap_or(1)
        .max(1);
    wb.pivot_caches.push(PivotCache {
        cache_id,
        dirty: true,
        model: true,
        ..PivotCache::default()
    });
    dm.pivots.push(cache_id);
    dm.write(wb);
    let name = crate::pivot::new_pivot_name(wb, labels);
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
        pt.style = Some(crate::pivot::default_style());
    }
    let p = spec.filters.len() as u32;
    if p > 0 {
        pt.location = CellRange::single(CellRef::new(dest.row + p + 1, dest.col));
    }
    wb.sheets[dest_sheet].pivots.push(pt);
    let index = wb.sheets[dest_sheet].pivots.len() - 1;
    if let Err(e) = crate::pivot::render_pivot(wb, dest_sheet, index, labels) {
        wb.sheets[dest_sheet].pivots.pop();
        wb.pivot_caches.retain(|c| c.cache_id != cache_id);
        before.write(wb);
        return Err(e);
    }
    Ok(index)
}

/// Re-render every pivot table over the data model (after the model changed).
///
/// # Errors
/// A report that cannot be rendered.
pub fn render_all(wb: &mut Workbook, labels: &PivotLabels) -> Result<usize, String> {
    if super::defs_part(wb).is_none() {
        return Ok(0);
    }
    let ids = DataModel::read(wb).pivots;
    let mut n = 0;
    for id in ids {
        for (s, i) in crate::pivot::users(wb, id) {
            crate::pivot::render_pivot(wb, s, i, labels)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Re-render the model pivot tables when table `name` is in the model (a table slicer
/// changed its filter).
///
/// # Errors
/// A report that cannot be rendered.
pub fn render_for_table(wb: &mut Workbook, name: &str, labels: &PivotLabels) -> Result<(), String> {
    if super::defs_part(wb).is_none() || DataModel::read(wb).table(name).is_none() {
        return Ok(());
    }
    render_all(wb, labels).map(|_| ())
}

//! Slicers: item buttons that filter a pivot table field or a table column ([MS-XLSX] slicer
//! parts, read into [`bo_model::sheet::Slicer`]).
//!
//! A pivot slicer's items are its pivot cache field's shared items; choosing items hides the
//! others in every connected pivot table (`pivotField/items/item@h`) and re-renders them. A
//! table slicer's items are the distinct shown texts of its column; choosing items writes the
//! table's `autoFilter` (§18.3.1.2, `filterColumn/filters`) and hides the other data rows.

use std::collections::BTreeSet;

use bo_model::sheet::{CellValue, SlicerSource, Workbook};
use serde::Serialize;

use crate::numfmt::Locale;
use crate::pivot::{update_pivot, value_text, PivotLabels, PivotSpec};
use crate::render::format_cell;

/// One button of a slicer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlicerItem {
    /// The label shown.
    pub label: String,
    /// The item passes the filter.
    pub selected: bool,
}

/// Where a table slicer's table lives: (sheet, table index).
fn find_table(wb: &Workbook, id: u32) -> Option<(usize, usize)> {
    wb.sheets
        .iter()
        .enumerate()
        .find_map(|(si, ws)| ws.tables.iter().position(|t| t.id == id).map(|ti| (si, ti)))
}

/// The data rows and the sheet column of a table's column `column` (1-based id).
fn table_column(wb: &Workbook, id: u32, column: u32) -> Option<(usize, usize, u32, Vec<u32>)> {
    let (si, ti) = find_table(wb, id)?;
    let t = &wb.sheets[si].tables[ti];
    let index = t.columns.iter().position(|c| c.id == column)?;
    let header = t.header_row_count.unwrap_or(1);
    let totals = t.totals_row_count.unwrap_or(0);
    let first = t.range.first.row + header;
    let last = t.range.last.row.checked_sub(totals)?;
    let rows = (first..=last).collect();
    Some((si, ti, t.range.first.col + index as u32, rows))
}

fn shown_text(wb: &Workbook, si: usize, row: u32, col: u32, locale: &Locale) -> String {
    wb.sheets[si]
        .cells
        .get(&(row, col))
        .map(|c| format_cell(wb, c, locale).text)
        .unwrap_or_default()
}

/// The pivot cache a pivot slicer filters: the cache of its first connected pivot table (the
/// slicer's own `pivotCacheId` numbers caches in the x14 extension, not by `cacheId`).
fn pivot_cache_of(wb: &Workbook, pivots: &[String], fallback: u32) -> u32 {
    wb.sheets
        .iter()
        .flat_map(|s| &s.pivots)
        .find(|p| pivots.iter().any(|n| n == &p.name))
        .map_or(fallback, |p| p.cache_id)
}

/// The buttons of slicer `index` of sheet `sheet`.
#[must_use]
pub fn items(wb: &Workbook, sheet: usize, index: usize, locale: &Locale) -> Vec<SlicerItem> {
    let Some(s) = wb.sheets.get(sheet).and_then(|ws| ws.slicers.get(index)) else {
        return Vec::new();
    };
    match &s.source {
        SlicerSource::Pivot {
            field,
            cache_id,
            items,
            pivots,
        } => {
            let cache_id = pivot_cache_of(wb, pivots, *cache_id);
            let Some(f) = wb
                .pivot_caches
                .iter()
                .find(|c| c.cache_id == cache_id)
                .and_then(|c| c.fields.iter().find(|f| f.name.eq_ignore_ascii_case(field)))
            else {
                return Vec::new();
            };
            let mut out: Vec<SlicerItem> = items
                .iter()
                .filter_map(|(x, sel)| {
                    f.items.get(*x as usize).map(|v| SlicerItem {
                        label: value_text(v),
                        selected: *sel,
                    })
                })
                .collect();
            // A cache read without the slicer's item list: every item, all selected.
            if out.is_empty() {
                out = f
                    .items
                    .iter()
                    .map(|v| SlicerItem {
                        label: value_text(v),
                        selected: true,
                    })
                    .collect();
            }
            // Nothing selected is stored as "no filter".
            if out.iter().all(|i| !i.selected) {
                for i in &mut out {
                    i.selected = true;
                }
            }
            out
        }
        SlicerSource::Table {
            table_id,
            column,
            shown,
        } => {
            let Some((si, _, col, rows)) = table_column(wb, *table_id, *column) else {
                return Vec::new();
            };
            let labels: BTreeSet<String> = rows
                .iter()
                .map(|&r| shown_text(wb, si, r, col, locale))
                .collect();
            labels
                .into_iter()
                .map(|label| SlicerItem {
                    selected: shown.as_ref().is_none_or(|s| s.contains(&label)),
                    label,
                })
                .collect()
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Choose the items of slicer `index` that pass (an empty choice clears the filter). Pivot
/// tables connected to a pivot slicer are re-rendered; a table slicer filters the table rows.
///
/// # Errors
/// An unknown slicer or a pivot table that cannot be re-rendered.
pub fn select(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    chosen: &[String],
    locale: &Locale,
    labels: &PivotLabels,
) -> Result<(), String> {
    let slicer = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.slicers.get(index))
        .cloned()
        .ok_or("no such slicer")?;
    let all = items(wb, sheet, index, locale);
    let keep = |label: &str| chosen.is_empty() || chosen.iter().any(|c| c == label);
    match slicer.source {
        SlicerSource::Pivot {
            field,
            pivots,
            cache_id,
            ..
        } => {
            let cache_id = pivot_cache_of(wb, &pivots, cache_id);
            let (fi, values) = {
                let cache = wb
                    .pivot_caches
                    .iter()
                    .find(|c| c.cache_id == cache_id)
                    .ok_or("pivot cache missing")?;
                let fi = cache
                    .fields
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(&field))
                    .ok_or("slicer field missing")?;
                (fi, cache.fields[fi].items.clone())
            };
            let hidden: Vec<CellValue> = values
                .iter()
                .filter(|v| !keep(&value_text(v)))
                .cloned()
                .collect();
            // Every connected pivot table: hide the unchosen items and re-render.
            let targets: Vec<(usize, usize)> = wb
                .sheets
                .iter()
                .enumerate()
                .flat_map(|(si, ws)| {
                    ws.pivots
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| pivots.iter().any(|n| n == &p.name))
                        .map(move |(pi, _)| (si, pi))
                })
                .collect();
            for (si, pi) in targets {
                let mut spec = PivotSpec::of(&wb.sheets[si].pivots[pi]);
                if spec.fields.len() <= fi {
                    spec.fields
                        .resize(fi + 1, bo_model::sheet::PivotFieldOptions::default());
                }
                spec.fields[fi].hidden.clone_from(&hidden);
                update_pivot(wb, si, pi, &spec, labels)?;
            }
            // The cache may have been refreshed: index the items afresh.
            let values = wb
                .pivot_caches
                .iter()
                .find(|c| c.cache_id == cache_id)
                .and_then(|c| c.fields.get(fi))
                .map(|f| f.items.clone())
                .unwrap_or(values);
            let s = &mut wb.sheets[sheet].slicers[index];
            if let SlicerSource::Pivot { items, .. } = &mut s.source {
                *items = values
                    .iter()
                    .enumerate()
                    .map(|(x, v)| (x as u32, keep(&value_text(v))))
                    .collect();
            }
            s.dirty = true;
        }
        SlicerSource::Table {
            table_id, column, ..
        } => {
            table_column(wb, table_id, column).ok_or("slicer table missing")?;
            let filtered = !chosen.is_empty() && all.iter().any(|i| !keep(&i.label));
            let shown: Option<Vec<String>> = filtered.then(|| {
                all.iter()
                    .filter(|i| keep(&i.label))
                    .map(|i| i.label.clone())
                    .collect()
            });
            if let SlicerSource::Table { shown: s, .. } =
                &mut wb.sheets[sheet].slicers[index].source
            {
                *s = shown;
            }
            wb.sheets[sheet].slicers[index].dirty = true;
            apply_table_filters(wb, table_id, locale);
            // Pivot tables over the data model follow the slicers of model tables.
            if let Some((si, ti)) = find_table(wb, table_id) {
                let name = wb.sheets[si].tables[ti].name.clone();
                let mut l = labels.clone();
                l.locale = Some(*locale);
                crate::datamodel::pivot::render_for_table(wb, &name, &l)?;
            }
        }
    }
    Ok(())
}

/// Write the table's `autoFilter` from every slicer on it (one `filterColumn` per filtered
/// column) and hide the data rows any of them leaves out.
fn apply_table_filters(wb: &mut Workbook, table_id: u32, locale: &Locale) {
    let Some((si, ti)) = find_table(wb, table_id) else {
        return;
    };
    // (sheet column, shown texts) of every filtering slicer of the table.
    let mut filters: Vec<(u32, Vec<String>)> = Vec::new();
    for ws in &wb.sheets {
        for sl in &ws.slicers {
            if let SlicerSource::Table {
                table_id: t,
                column,
                shown: Some(vals),
            } = &sl.source
            {
                if *t == table_id {
                    if let Some((_, _, col, _)) = table_column(wb, table_id, *column) {
                        filters.retain(|(c, _)| *c != col);
                        filters.push((col, vals.clone()));
                    }
                }
            }
        }
    }
    filters.sort_by_key(|(c, _)| *c);
    let t = &wb.sheets[si].tables[ti];
    let first_col = t.range.first.col;
    let range = t.range.to_string();
    let header = t.header_row_count.unwrap_or(1);
    let totals = t.totals_row_count.unwrap_or(0);
    let rows: Vec<u32> = match t.range.last.row.checked_sub(totals) {
        Some(last) => (t.range.first.row + header..=last).collect(),
        None => Vec::new(),
    };
    let xml = if filters.is_empty() {
        format!("<autoFilter ref=\"{range}\"/>")
    } else {
        let mut cols = String::new();
        for (col, vals) in &filters {
            cols.push_str("<filterColumn colId=\"");
            cols.push_str(&(col - first_col).to_string());
            cols.push_str("\"><filters>");
            for v in vals {
                cols.push_str("<filter val=\"");
                cols.push_str(&escape(v));
                cols.push_str("\"/>");
            }
            cols.push_str("</filters></filterColumn>");
        }
        format!("<autoFilter ref=\"{range}\">{cols}</autoFilter>")
    };
    let hidden: Vec<(u32, bool)> = rows
        .iter()
        .map(|&r| {
            let out = filters
                .iter()
                .any(|(col, vals)| !vals.contains(&shown_text(wb, si, r, *col, locale)));
            (r, out)
        })
        .collect();
    wb.sheets[si].tables[ti].auto_filter = Some(xml);
    let ws = &mut wb.sheets[si];
    for (r, hide) in hidden {
        ws.rows.entry(r).or_default().hidden = hide;
    }
}

/// A valid, unused slicer cache name for `base` (`Slicer_<base>`, digits added when taken).
fn cache_name(wb: &Workbook, base: &str) -> String {
    let clean: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = format!("Slicer_{clean}");
    let taken = |n: &str| {
        wb.defined_names
            .iter()
            .any(|d| d.name.eq_ignore_ascii_case(n))
            || wb
                .sheets
                .iter()
                .flat_map(|s| &s.slicers)
                .any(|s| s.cache.eq_ignore_ascii_case(n))
    };
    if !taken(&stem) {
        return stem;
    }
    (1..10_000)
        .map(|i| format!("{stem}{i}"))
        .find(|n| !taken(n))
        .unwrap_or(stem)
}

/// A slicer's caption, column count and style (`None`: unchanged). The caller writes them into
/// the parts of a slicer read from a file.
///
/// # Errors
/// An unknown slicer or a column count of zero.
pub fn set_options(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    caption: Option<String>,
    columns: Option<u32>,
    style: Option<String>,
) -> Result<(), String> {
    let s = wb
        .sheets
        .get_mut(sheet)
        .and_then(|ws| ws.slicers.get_mut(index))
        .ok_or("no such slicer")?;
    if let Some(c) = caption {
        s.caption = c;
    }
    if let Some(n) = columns {
        if n == 0 || n > 20_000 {
            return Err("bad column count".into());
        }
        s.columns = n;
    }
    if let Some(st) = style {
        s.style = (!st.is_empty()).then_some(st);
    }
    Ok(())
}

/// Insert a slicer for column `column` (1-based id) of table `table_id`, placed at `at`
/// (top-left cell) on sheet `sheet`. Returns the slicer's index on the sheet.
///
/// # Errors
/// An unknown table or column.
pub fn insert_table_slicer(
    wb: &mut Workbook,
    sheet: usize,
    table_id: u32,
    column: u32,
    at: bo_model::sheet::CellRef,
) -> Result<usize, String> {
    let (ti_sheet, ti) = find_table(wb, table_id).ok_or("no such table")?;
    let caption = wb.sheets[ti_sheet].tables[ti]
        .columns
        .iter()
        .find(|c| c.id == column)
        .map(|c| c.name.clone())
        .ok_or("no such column")?;
    if sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    let cache = cache_name(wb, &caption);
    // Slicer names are unique in the workbook.
    let name = frame_name(wb, &caption);
    wb.defined_names.push(bo_model::sheet::DefinedName {
        name: cache.clone(),
        formula: "#N/A".into(),
        ..bo_model::sheet::DefinedName::default()
    });
    let slicers = &mut wb.sheets[sheet].slicers;
    slicers.push(bo_model::sheet::Slicer {
        name,
        caption,
        cache,
        source: SlicerSource::Table {
            table_id,
            column,
            shown: None,
        },
        columns: 1,
        style: Some("SlicerStyleLight1".into()),
        row_height: Some(241_300),
        from: at,
        to: Some(bo_model::sheet::CellRef::new(at.row + 12, at.col + 2)),
        ..bo_model::sheet::Slicer::default()
    });
    Ok(slicers.len() - 1)
}

// ------------------------------------------------------------------ pivot table slicers

/// The pivot cache fields the workbook's slicers filter in pivot table `pt`: their hidden
/// items filter the report wherever the field is (also outside the row and column areas).
#[must_use]
pub fn fields_of(wb: &Workbook, pt: &bo_model::sheet::PivotTable) -> Vec<u32> {
    let Some(cache) = wb.pivot_caches.iter().find(|c| c.cache_id == pt.cache_id) else {
        return Vec::new();
    };
    let mut out: Vec<u32> = wb
        .sheets
        .iter()
        .flat_map(|s| &s.slicers)
        .filter_map(|s| match &s.source {
            SlicerSource::Pivot { field, pivots, .. } if pivots.iter().any(|n| n == &pt.name) => {
                cache
                    .fields
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(field))
                    .map(|i| i as u32)
            }
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The unselected item values of every pivot slicer on cache `cache_id`, by slicer
/// `(sheet, index)`: taken before the cache is refreshed, given back to
/// [`restore_choices`] afterwards (the items are indexed afresh).
pub(crate) fn choices(wb: &Workbook, cache_id: u32) -> Vec<(usize, usize, Vec<CellValue>)> {
    let mut out = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for (i, s) in ws.slicers.iter().enumerate() {
            let SlicerSource::Pivot {
                field,
                pivots,
                cache_id: own,
                items,
            } = &s.source
            else {
                continue;
            };
            if pivot_cache_of(wb, pivots, *own) != cache_id || pivots.is_empty() {
                continue;
            }
            let Some(f) = wb
                .pivot_caches
                .iter()
                .find(|c| c.cache_id == cache_id)
                .and_then(|c| c.fields.iter().find(|f| f.name.eq_ignore_ascii_case(field)))
            else {
                continue;
            };
            // Nothing selected is stored as "no filter".
            let filtered = items.iter().any(|(_, sel)| *sel);
            let hidden = items
                .iter()
                .filter(|(_, sel)| filtered && !*sel)
                .filter_map(|(x, _)| f.items.get(*x as usize).cloned())
                .collect();
            out.push((si, i, hidden));
        }
    }
    out
}

/// Index the items of the slicers of [`choices`] over the refreshed cache `cache_id`: the
/// values that were unselected stay so, new values are selected.
pub(crate) fn restore_choices(
    wb: &mut Workbook,
    cache_id: u32,
    saved: Vec<(usize, usize, Vec<CellValue>)>,
) {
    for (si, i, hidden) in saved {
        let Some(SlicerSource::Pivot { field, .. }) = wb
            .sheets
            .get(si)
            .and_then(|ws| ws.slicers.get(i))
            .map(|s| s.source.clone())
        else {
            continue;
        };
        let Some(values) = wb
            .pivot_caches
            .iter()
            .find(|c| c.cache_id == cache_id)
            .and_then(|c| {
                c.fields
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(&field))
            })
            .map(|f| f.items.clone())
        else {
            continue;
        };
        let keys: Vec<bo_model::sheet::PivotItemKey> = hidden
            .iter()
            .map(bo_model::sheet::PivotItemKey::of)
            .collect();
        let s = &mut wb.sheets[si].slicers[i];
        if let SlicerSource::Pivot { items, .. } = &mut s.source {
            let next: Vec<(u32, bool)> = values
                .iter()
                .enumerate()
                .map(|(x, v)| {
                    (
                        x as u32,
                        !keys.contains(&bo_model::sheet::PivotItemKey::of(v)),
                    )
                })
                .collect();
            if *items != next {
                *items = next;
                s.dirty = true;
            }
        }
    }
}

/// The [MS-XLSX] `x14:pivotCacheDefinition/@pivotCacheId` of cache `cache_id`, given now when
/// the cache has none: slicer and timeline caches name the pivot cache by it.
pub(crate) fn ensure_ext_id(wb: &mut Workbook, cache_id: u32) -> Result<u32, String> {
    let next = wb
        .pivot_caches
        .iter()
        .filter_map(|c| c.ext_id)
        .max()
        .map_or(1, |m| m.saturating_add(1));
    let cache = wb
        .pivot_caches
        .iter_mut()
        .find(|c| c.cache_id == cache_id)
        .ok_or("pivot cache missing")?;
    Ok(*cache.ext_id.get_or_insert(next))
}

/// An unused name for a slicer or timeline with caption `caption` (names are unique in the
/// workbook).
pub(crate) fn frame_name(wb: &Workbook, caption: &str) -> String {
    let taken = |n: &str| {
        wb.sheets.iter().any(|s| {
            s.slicers.iter().any(|x| x.name == n) || s.timelines.iter().any(|x| x.name == n)
        })
    };
    let mut name = caption.to_owned();
    let mut k = 1;
    while taken(&name) {
        k += 1;
        name = format!("{caption} {k}");
    }
    name
}

/// Insert a slicer for cache field `field` of pivot table `pivot` of sheet `pivot_sheet`,
/// placed at `at` (top-left cell) on sheet `sheet`. Returns the slicer's index on the sheet.
///
/// # Errors
/// An unknown pivot table or field, a calculated or grouping field, or a pivot table over the
/// data model (its tables take table slicers).
pub fn insert_pivot_slicer(
    wb: &mut Workbook,
    sheet: usize,
    pivot_sheet: usize,
    pivot: usize,
    field: u32,
    at: bo_model::sheet::CellRef,
    labels: &PivotLabels,
) -> Result<usize, String> {
    if sheet >= wb.sheets.len() {
        return Err("no such sheet".into());
    }
    let pt = wb
        .sheets
        .get(pivot_sheet)
        .and_then(|s| s.pivots.get(pivot))
        .ok_or("no such pivot table")?;
    let (cache_id, pivot_name) = (pt.cache_id, pt.name.clone());
    if crate::datamodel::pivot::is_model_cache(wb, cache_id) {
        return Err("not available for a pivot table over the data model".into());
    }
    let field_of = |wb: &Workbook| {
        wb.pivot_caches
            .iter()
            .find(|c| c.cache_id == cache_id)
            .and_then(|c| c.fields.get(field as usize))
            .cloned()
    };
    let mut f = field_of(wb).ok_or("unknown pivot field")?;
    if f.formula.is_some() || f.derived {
        return Err("a slicer needs a source field".into());
    }
    if f.group
        .as_ref()
        .is_some_and(|g| g.range.is_some() || !g.discrete.is_empty())
    {
        return Err("ungroup the field before adding a slicer".into());
    }
    if f.items.is_empty() {
        // A cache read without the items of this field: load them from the source.
        crate::pivot::ensure_records(wb, pivot_sheet, pivot, labels)?;
        f = field_of(wb).ok_or("unknown pivot field")?;
    }
    let ext_id = ensure_ext_id(wb, cache_id)?;
    // The items the pivot table hides already are unselected.
    let hidden: Vec<bo_model::sheet::PivotItemKey> = wb.sheets[pivot_sheet].pivots[pivot]
        .field(field)
        .hidden
        .iter()
        .map(bo_model::sheet::PivotItemKey::of)
        .collect();
    let items = f
        .items
        .iter()
        .enumerate()
        .map(|(x, v)| {
            (
                x as u32,
                !hidden.contains(&bo_model::sheet::PivotItemKey::of(v)),
            )
        })
        .collect();
    let cache = cache_name(wb, &f.name);
    let name = frame_name(wb, &f.name);
    wb.defined_names.push(bo_model::sheet::DefinedName {
        name: cache.clone(),
        formula: "#N/A".into(),
        ..bo_model::sheet::DefinedName::default()
    });
    let slicers = &mut wb.sheets[sheet].slicers;
    slicers.push(bo_model::sheet::Slicer {
        name,
        caption: f.name.clone(),
        cache,
        source: SlicerSource::Pivot {
            field: f.name,
            pivots: vec![pivot_name],
            cache_id: ext_id,
            items,
        },
        columns: 1,
        style: Some("SlicerStyleLight1".into()),
        row_height: Some(241_300),
        from: at,
        to: Some(bo_model::sheet::CellRef::new(at.row + 12, at.col + 2)),
        ..bo_model::sheet::Slicer::default()
    });
    Ok(slicers.len() - 1)
}

/// The pivot tables a slicer on cache field `field` of the cache of `pivots` may filter:
/// every pivot table on that cache, as `(sheet, index, name, connected)`.
#[must_use]
pub fn connectable(wb: &Workbook, pivots: &[String]) -> Vec<(usize, usize, String, bool)> {
    let Some(cache_id) = wb
        .sheets
        .iter()
        .flat_map(|s| &s.pivots)
        .find(|p| pivots.iter().any(|n| n == &p.name))
        .map(|p| p.cache_id)
    else {
        return Vec::new();
    };
    crate::pivot::users(wb, cache_id)
        .into_iter()
        .map(|(s, i)| {
            let name = wb.sheets[s].pivots[i].name.clone();
            let on = pivots.contains(&name);
            (s, i, name, on)
        })
        .collect()
}

/// Connect pivot slicer `index` of sheet `sheet` to the pivot tables named `names` (all on
/// the slicer's pivot cache; at least one). Newly connected pivot tables take the slicer's
/// filter, disconnected ones show every item of the field again.
///
/// # Errors
/// A table slicer, an empty list, or a pivot table on another cache.
pub fn connect(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    names: &[String],
    labels: &PivotLabels,
) -> Result<(), String> {
    let s = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.slicers.get(index))
        .cloned()
        .ok_or("no such slicer")?;
    let SlicerSource::Pivot {
        field,
        pivots,
        cache_id,
        items,
    } = s.source
    else {
        return Err("a table slicer filters its table only".into());
    };
    if names.is_empty() {
        return Err("a slicer filters at least one pivot table".into());
    }
    let all = connectable(wb, &pivots);
    if names.iter().any(|n| !all.iter().any(|(_, _, x, _)| x == n)) {
        return Err("the pivot table uses other data".into());
    }
    let cache = pivot_cache_of(wb, &pivots, cache_id);
    let (fi, values) = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == cache)
        .and_then(|c| {
            let fi = c
                .fields
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(&field))?;
            Some((fi, c.fields[fi].items.clone()))
        })
        .ok_or("slicer field missing")?;
    let filtered = items.iter().any(|(_, sel)| *sel) && items.iter().any(|(_, sel)| !*sel);
    let hidden: Vec<CellValue> = items
        .iter()
        .filter(|(_, sel)| filtered && !*sel)
        .filter_map(|(x, _)| values.get(*x as usize).cloned())
        .collect();
    if let SlicerSource::Pivot { pivots: p, .. } = &mut wb.sheets[sheet].slicers[index].source {
        *p = names.to_vec();
    }
    for (si, pi, name, was) in all {
        let now = names.contains(&name);
        if now == was {
            continue;
        }
        let mut spec = PivotSpec::of(&wb.sheets[si].pivots[pi]);
        if spec.fields.len() <= fi {
            spec.fields
                .resize(fi + 1, bo_model::sheet::PivotFieldOptions::default());
        }
        spec.fields[fi].hidden = if now { hidden.clone() } else { Vec::new() };
        update_pivot(wb, si, pi, &spec, labels)?;
    }
    Ok(())
}

/// Colours of a slicer or timeline style for drawing (BlueOffice's own palette over the
/// theme accents): the built-in style names end in a variant (`Light`, `Dark`, `Other`) and a
/// number choosing the accent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameColors {
    /// Fill of a selected item / period (`#RRGGBB`).
    pub selected: String,
    /// Text on a selected item.
    pub selected_text: String,
    /// Fill of the header.
    pub header: String,
    /// Text of the header.
    pub header_text: String,
    /// Frame border.
    pub border: String,
    /// Fill of the frame (a custom style's `wholeTable`); `None` = the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Fill of an item / period that is not selected (a custom style); `None` = the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unselected: Option<String>,
    /// Text of an item that is not selected (a custom style); `None` = the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unselected_text: Option<String>,
}

/// The colour of a differential fill: a solid fill keeps its colour in `bgColor` (§18.8.3;
/// some writers use `fgColor`), a gradient its first stop.
fn dxf_fill(d: &bo_model::sheet::Dxf, wb: &Workbook) -> Option<String> {
    use bo_model::sheet::Fill;
    let c = match d.fill.as_ref()? {
        Fill::Pattern(p) => p.bg.as_ref().or(p.fg.as_ref())?,
        Fill::Gradient(g) => &g.stops.first()?.1,
    };
    let rgb = crate::color::resolve_color(c, wb.theme.as_ref())?;
    Some(format!("#{}", crate::color::hex((rgb.0, rgb.1, rgb.2))))
}

/// The font colour of a differential format.
fn dxf_text(d: &bo_model::sheet::Dxf, wb: &Workbook) -> Option<String> {
    let c = d.font.as_ref()?.color.as_ref()?;
    let rgb = crate::color::resolve_color(c, wb.theme.as_ref())?;
    Some(format!("#{}", crate::color::hex((rgb.0, rgb.1, rgb.2))))
}

/// The colour of the first edge of a differential border that has one.
fn dxf_border(d: &bo_model::sheet::Dxf, wb: &Workbook) -> Option<String> {
    let b = d.border.as_ref()?;
    [&b.top, &b.left, &b.bottom, &b.right]
        .into_iter()
        .chain(b.horizontal.as_ref())
        .chain(b.vertical.as_ref())
        .find_map(|e| e.color.as_ref())
        .and_then(|c| crate::color::resolve_color(c, wb.theme.as_ref()))
        .map(|rgb| format!("#{}", crate::color::hex((rgb.0, rgb.1, rgb.2))))
}

/// The colours of a custom slicer or timeline style (the workbook's own `x14:slicerStyle` /
/// `x15:timelineStyle` and its `tableStyle`): what it sets over the first light style.
#[must_use]
pub fn custom_frame_colors(wb: &Workbook, parts: &bo_model::sheet::FrameStyleParts) -> FrameColors {
    let mut c = builtin_frame_colors(
        wb,
        if parts.timeline {
            "TimeSlicerStyleLight1"
        } else {
            "SlicerStyleLight1"
        },
    );
    if let Some(d) = &parts.whole {
        c.background = dxf_fill(d, wb);
        if let Some(b) = dxf_border(d, wb) {
            c.border = b;
        }
        if let Some(t) = dxf_text(d, wb) {
            c.header_text.clone_from(&t);
            c.unselected_text = Some(t);
        }
    }
    if let Some(d) = &parts.header {
        if let Some(f) = dxf_fill(d, wb) {
            c.header = f;
        }
        if let Some(t) = dxf_text(d, wb) {
            c.header_text = t;
        }
    }
    if let Some(d) = &parts.selected {
        if let Some(f) = dxf_fill(d, wb) {
            c.selected = f;
        }
        if let Some(t) = dxf_text(d, wb) {
            c.selected_text = t;
        }
    }
    if let Some(d) = &parts.unselected {
        if let Some(f) = dxf_fill(d, wb) {
            c.unselected = Some(f);
        }
        if let Some(t) = dxf_text(d, wb) {
            c.unselected_text = Some(t);
        }
    }
    c
}

/// The colours of style `name` (`SlicerStyleLight1`, `SlicerStyleDark3`, `SlicerStyleOther2`,
/// `TimeSlicerStyleLight4`, …); an unknown or missing name draws as the first light style.
#[must_use]
pub fn frame_colors(wb: &Workbook, name: Option<&str>) -> FrameColors {
    let name = name.unwrap_or_default();
    // A custom style of the workbook ([MS-XLSX] `x14:slicerStyle` / `x15:timelineStyle`).
    if let Some(parts) = wb
        .styles
        .frame_styles
        .iter()
        .find(|s| !name.is_empty() && s.name == name)
    {
        return custom_frame_colors(wb, parts);
    }
    builtin_frame_colors(wb, name)
}

/// The colours of built-in style `name` (see [`frame_colors`]).
fn builtin_frame_colors(wb: &Workbook, name: &str) -> FrameColors {
    let digits: String = name
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let n: usize = digits.parse().unwrap_or(1);
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let dark = stem.ends_with("Dark");
    let other = stem.ends_with("Other");
    // Accents 1 to 6 are theme colour indexes 4 to 9 (§18.8.3 `theme` colour index).
    let accent = if other {
        if n % 2 == 1 {
            "#595959".to_owned()
        } else {
            "#7F7F7F".to_owned()
        }
    } else {
        let (r, g, b) = crate::color::theme_rgb(4 + ((n.max(1) - 1) % 6) as u32, wb.theme.as_ref());
        format!("#{r:02X}{g:02X}{b:02X}")
    };
    if dark {
        FrameColors {
            selected: accent.clone(),
            selected_text: "#FFFFFF".into(),
            header: accent.clone(),
            header_text: "#FFFFFF".into(),
            border: accent,
            background: None,
            unselected: None,
            unselected_text: None,
        }
    } else {
        FrameColors {
            selected: accent.clone(),
            selected_text: "#FFFFFF".into(),
            header: "#FFFFFF".into(),
            header_text: "#262626".into(),
            border: accent,
            background: None,
            unselected: None,
            unselected_text: None,
        }
    }
}

/// The built-in slicer style names offered for new slicers.
pub const SLICER_STYLES: [&str; 14] = [
    "SlicerStyleLight1",
    "SlicerStyleLight2",
    "SlicerStyleLight3",
    "SlicerStyleLight4",
    "SlicerStyleLight5",
    "SlicerStyleLight6",
    "SlicerStyleOther1",
    "SlicerStyleOther2",
    "SlicerStyleDark1",
    "SlicerStyleDark2",
    "SlicerStyleDark3",
    "SlicerStyleDark4",
    "SlicerStyleDark5",
    "SlicerStyleDark6",
];

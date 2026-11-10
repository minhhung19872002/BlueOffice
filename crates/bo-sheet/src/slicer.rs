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
    let mut name = caption.clone();
    let mut k = 1;
    while wb
        .sheets
        .iter()
        .flat_map(|s| &s.slicers)
        .any(|s| s.name == name)
    {
        k += 1;
        name = format!("{caption} {k}");
    }
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

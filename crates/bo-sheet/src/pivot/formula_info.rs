//! Pivot table reports as the formula engine reads them for `GETPIVOTDATA`
//! (ECMA-376 Part 1, §18.17.7 GETPIVOTDATA).

use bo_formula::{Area, PivotAxisField, PivotInfo, PivotValueCell, Value};
use bo_model::sheet::{CellValue, PivotCache, PivotTable, Workbook};

use super::{
    axis_items, cache_source, date_system, layout_in, occupied, refresh_cache, value_name,
    PivotLabels, PivotSpec,
};

/// Whether any formula of the workbook calls `GETPIVOTDATA` (the reports are only
/// computed for workbooks that read them).
pub(crate) fn workbook_reads_pivots(wb: &Workbook) -> bool {
    const NAME: &[u8] = b"GETPIVOTDATA";
    wb.sheets.iter().any(|ws| !ws.pivots.is_empty())
        && wb.sheets.iter().any(|ws| {
            ws.cells.values().any(|c| {
                c.formula.as_ref().is_some_and(|f| {
                    f.text
                        .as_bytes()
                        .windows(NAME.len())
                        .any(|w| w.eq_ignore_ascii_case(NAME))
                })
            })
        })
}

/// The cache of `pt` with its records: a cache read from a file keeps its records part
/// opaque, so they are read from the source again into a scratch copy (the workbook
/// itself is not changed).
fn cache_with_records(wb: &Workbook, pt: &PivotTable, labels: &PivotLabels) -> Option<PivotCache> {
    let cache = wb.pivot_caches.iter().find(|c| c.cache_id == pt.cache_id)?;
    if cache.dirty || !cache.records.is_empty() {
        return Some(cache.clone());
    }
    let (si, _) = cache_source(wb, cache)?;
    let mut scratch = Workbook {
        sheets: vec![wb.sheets.get(si)?.clone()],
        defined_names: wb.defined_names.clone(),
        date1904: wb.date1904,
        pivot_caches: vec![cache.clone()],
        ..Workbook::default()
    };
    refresh_cache(&mut scratch, cache.cache_id, labels, None).ok()?;
    scratch.pivot_caches.pop()
}

fn caption(v: &CellValue, labels: &PivotLabels, wb: &Workbook) -> Value {
    match v {
        CellValue::Empty => Value::Text(labels.blank.clone()),
        v => crate::formula::to_value(v, date_system(wb)),
    }
}

/// One report, `None` when it cannot be computed (over the data model, source missing).
fn report(wb: &Workbook, sheet: usize, pt: &PivotTable, labels: &PivotLabels) -> Option<PivotInfo> {
    if crate::datamodel::pivot::is_model_cache(wb, pt.cache_id) {
        return None;
    }
    let cache = cache_with_records(wb, pt, labels)?;
    let spec = PivotSpec::of_in(wb, pt);
    let lay = layout_in(&cache, &spec, labels, date_system(wb)).ok()?;
    let axis = |fields: &[u32]| -> Vec<PivotAxisField> {
        fields
            .iter()
            .filter_map(|&f| cache.fields.get(f as usize))
            .map(|f| PivotAxisField {
                name: f.name.clone(),
                items: axis_items(f)
                    .iter()
                    .map(|v| caption(v, labels, wb))
                    .collect(),
            })
            .collect()
    };
    let data_fields = spec
        .values
        .iter()
        .map(|d| {
            let mut names = vec![value_name(&cache, d)];
            if let Some(f) = cache.fields.get(d.field as usize) {
                names.push(f.name.clone());
            }
            names
        })
        .collect();
    let body = pt.location.first;
    let mut cells = Vec::new();
    for (row, row_path) in &lay.value_rows {
        let on_rows = lay
            .data_rows
            .iter()
            .find(|(r, _)| r == row)
            .map(|(_, vi)| *vi);
        for ((col, vi), col_path) in lay.data_cols.iter().zip(&lay.col_keys) {
            cells.push(PivotValueCell {
                row: body.row + row,
                col: body.col + col,
                data: on_rows.unwrap_or(*vi),
                row_path: row_path.clone(),
                col_path: col_path.clone(),
            });
        }
    }
    let area = occupied(pt);
    Some(PivotInfo {
        area: Area::new(
            sheet,
            area.first.row,
            area.first.col,
            area.last.row,
            area.last.col,
        ),
        data_fields,
        row_fields: axis(&spec.rows),
        col_fields: axis(&spec.cols),
        cells,
    })
}

/// Every pivot table report of the workbook that can be computed.
pub(crate) fn formula_pivots(wb: &Workbook) -> Vec<PivotInfo> {
    let labels = PivotLabels::default();
    let mut out = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for pt in &ws.pivots {
            if let Some(info) = report(wb, si, pt, &labels) {
                out.push(info);
            }
        }
    }
    out
}

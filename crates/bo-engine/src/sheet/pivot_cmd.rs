//! Pivot table commands over the JSON bridge (ECMA-376 Part 1, §18.10): the
//! field list of the side panel, report settings, grouping, calculated
//! fields, delete / move / change source / refresh all.

use super::cmd::{At, Rng};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{
    CellRef, CellValue, PivotCache, PivotDataField, PivotFieldFilter, PivotFieldOptions,
    PivotFunction, PivotGroupBy, PivotItemKey, PivotShowAs, PivotSort, PivotStyleInfo, PivotTable,
    Stylesheet, Workbook,
};
use bo_sheet::numfmt::Locale;
use bo_sheet::pivot::{GroupRequest, PivotLabels, PivotSpec};
use serde::Deserialize;
use serde_json::{json, Value};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn yes() -> bool {
    true
}

/// A value field of a pivot table from the UI.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PivotValueSpec {
    pub field: u32,
    #[serde(default)]
    pub function: PivotFunction,
    /// Caption; `None` = "<Function> of <field>".
    #[serde(default)]
    pub name: Option<String>,
    /// "Show values as" (§18.18.70 `ST_ShowDataAs`, x14 additions).
    #[serde(default)]
    pub show_as: PivotShowAs,
    #[serde(default)]
    pub base_field: u32,
    /// Item index of the base field, or 1048828 / 1048829 (previous / next).
    #[serde(default)]
    pub base_item: u32,
    /// Number format code; `None` / empty = from the source.
    #[serde(default)]
    pub num_fmt: Option<String>,
}

/// A report filter from the UI: shown items by index into the field's items
/// (as listed by `pivots`); empty = all items.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PivotFilterSpec {
    pub field: u32,
    #[serde(default)]
    pub items: Vec<usize>,
}

/// Settings of one row / column field from the field settings dialog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PivotFieldSpec {
    pub field: u32,
    #[serde(default = "yes")]
    pub subtotal: bool,
    #[serde(default = "yes")]
    pub subtotal_top: bool,
    #[serde(default)]
    pub blank_row: bool,
    #[serde(default)]
    pub repeat_labels: bool,
    #[serde(default)]
    pub sort: PivotSort,
    /// Sort by this value field (index into the values) instead of by label.
    #[serde(default)]
    pub sort_by: Option<u32>,
    /// Hidden items by index into the field's items.
    #[serde(default)]
    pub hidden: Vec<usize>,
}

/// `pivotTableStyleInfo` from the UI.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PivotStyleSpec {
    pub name: String,
    #[serde(default = "yes")]
    pub row_headers: bool,
    #[serde(default = "yes")]
    pub col_headers: bool,
    #[serde(default)]
    pub row_stripes: bool,
    #[serde(default)]
    pub col_stripes: bool,
    #[serde(default = "yes")]
    pub last_column: bool,
}

/// Optional report settings of `pivotUpdate` (absent = unchanged).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PivotSettings {
    pub layout: Option<bo_model::sheet::PivotLayout>,
    pub row_grand_totals: Option<bool>,
    pub col_grand_totals: Option<bool>,
    pub field_options: Option<Vec<PivotFieldSpec>>,
    pub field_filters: Option<Vec<PivotFieldFilter>>,
    pub style: Option<PivotStyleSpec>,
    /// Several value fields listed down the rows (`dataOnRows`).
    pub data_on_rows: Option<bool>,
}

/// Items of a field as the report shows them (group captions when grouped).
fn axis_values(cache: &PivotCache, field: u32) -> &[CellValue] {
    cache
        .fields
        .get(field as usize)
        .map_or(&[], bo_sheet::pivot::axis_items)
}

/// Data field of a UI value spec (number format codes become ids).
pub(super) fn data_field(styles: &mut Stylesheet, v: PivotValueSpec) -> PivotDataField {
    let num_fmt_id = v
        .num_fmt
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| bo_sheet::style::num_fmt_id(styles, c));
    PivotDataField {
        field: v.field,
        function: v.function,
        name: v.name.filter(|n| !n.trim().is_empty()),
        show_as: v.show_as,
        base_field: v.base_field,
        base_item: v.base_item,
        num_fmt_id,
    }
}

/// The spec of `pt` with the areas and settings of a `pivotUpdate`.
pub(super) fn updated_spec(
    wb: &mut Workbook,
    pt: &PivotTable,
    areas: (
        Vec<u32>,
        Vec<u32>,
        Vec<PivotValueSpec>,
        Vec<PivotFilterSpec>,
    ),
    settings: PivotSettings,
) -> PivotSpec {
    let (rows, cols, values, filters) = areas;
    let cache = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pt.cache_id)
        .cloned()
        .unwrap_or_default();
    let mut spec = PivotSpec::of(pt);
    spec.rows = rows;
    spec.cols = cols;
    spec.values = values
        .into_iter()
        .map(|v| data_field(&mut wb.styles, v))
        .collect();
    spec.filters = filters
        .iter()
        .map(|f| bo_model::sheet::PivotFilter {
            field: f.field,
            selected: f
                .items
                .iter()
                .filter_map(|i| axis_values(&cache, f.field).get(*i).cloned())
                .collect(),
        })
        .collect();
    if let Some(l) = settings.layout {
        spec.layout = l;
    }
    if let Some(v) = settings.row_grand_totals {
        spec.row_grand_totals = v;
    }
    if let Some(v) = settings.col_grand_totals {
        spec.col_grand_totals = v;
    }
    if let Some(list) = settings.field_options {
        for o in list {
            let i = o.field as usize;
            if i >= cache.fields.len() {
                continue;
            }
            if spec.fields.len() <= i {
                spec.fields.resize(i + 1, PivotFieldOptions::default());
            }
            let items = axis_values(&cache, o.field);
            spec.fields[i] = PivotFieldOptions {
                subtotal: o.subtotal,
                subtotal_top: o.subtotal_top,
                blank_row: o.blank_row,
                repeat_labels: o.repeat_labels,
                sort: o.sort,
                sort_by: o.sort_by,
                hidden: o
                    .hidden
                    .iter()
                    .filter_map(|x| items.get(*x).cloned())
                    .collect(),
            };
        }
    }
    if let Some(f) = settings.field_filters {
        spec.field_filters = f;
    }
    if let Some(d) = settings.data_on_rows {
        spec.data_on_rows = d;
    }
    if let Some(s) = settings.style {
        spec.style = Some(PivotStyleInfo {
            name: s.name,
            show_row_headers: s.row_headers,
            show_col_headers: s.col_headers,
            show_row_stripes: s.row_stripes,
            show_col_stripes: s.col_stripes,
            show_last_column: s.last_column,
        });
    }
    spec
}

/// Grouping of a field for the field settings dialog.
fn group_json(cache: &PivotCache, field: usize) -> Value {
    let Some(f) = cache.fields.get(field) else {
        return Value::Null;
    };
    let Some(g) = &f.group else {
        return Value::Null;
    };
    if f.derived {
        return json!({ "by": "derived", "base": g.base });
    }
    let Some(r) = &g.range else {
        return json!({ "by": "discrete" });
    };
    if !r.dates {
        return json!({ "by": "numbers", "start": r.start, "end": r.end, "interval": r.interval });
    }
    let mut parts = vec![r.group_by];
    parts.extend(cache.fields.iter().filter_map(|o| {
        let og = o.group.as_ref()?;
        (o.derived && og.base == Some(field as u32))
            .then(|| og.range.as_ref().map(|x| x.group_by))
            .flatten()
    }));
    let has = |p: PivotGroupBy| parts.contains(&p);
    json!({
        "by": "dates",
        "years": has(PivotGroupBy::Years),
        "quarters": has(PivotGroupBy::Quarters),
        "months": has(PivotGroupBy::Months),
        "days": has(PivotGroupBy::Days),
        "start": r.start,
        "end": r.end,
    })
}

/// Field list, areas and settings of a pivot table for the side panel.
pub(super) fn pivot_json(wb: &Workbook, index: usize, p: &PivotTable, locale: &Locale) -> Value {
    let cache = wb.pivot_caches.iter().find(|c| c.cache_id == p.cache_id);
    let fields: Vec<Value> = cache
        .map(|c| {
            c.fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let grouped = f
                        .group
                        .as_ref()
                        .is_some_and(|g| g.range.is_some() || !g.discrete.is_empty());
                    // Item labels in the source column's number format (dates).
                    let fmt = (f.num_fmt_id != 0 && !grouped).then(|| {
                        let code = bo_sheet::numfmt::format_code(&wb.styles, f.num_fmt_id, locale);
                        bo_sheet::numfmt::NumberFormat::parse_with(&code, locale)
                    });
                    let opts = bo_sheet::numfmt::FormatOptions {
                        locale: *locale,
                        date1904: wb.date1904,
                    };
                    let items: Vec<String> = axis_values(c, i as u32)
                        .iter()
                        .map(|v| match (&fmt, v) {
                            (Some(nf), CellValue::Number(_)) => nf.format(v, &opts).text,
                            _ => bo_sheet::pivot::value_text(v),
                        })
                        .collect();
                    let date = fmt
                        .as_ref()
                        .is_some_and(bo_sheet::numfmt::NumberFormat::is_date)
                        || f.group
                            .as_ref()
                            .and_then(|g| g.range.as_ref())
                            .is_some_and(|r| r.dates);
                    json!({
                        "name": f.name,
                        "items": items,
                        "formula": f.formula,
                        "derived": f.derived,
                        "date": date,
                        "group": group_json(c, i),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let item_index = |field: u32, v: &CellValue| -> Option<usize> {
        let key = PivotItemKey::of(v);
        axis_values(cache?, field)
            .iter()
            .position(|x| PivotItemKey::of(x) == key)
    };
    let source = cache.and_then(|c| {
        c.source_name.clone().or_else(|| {
            Some(format!(
                "{}!{}",
                bo_sheet::cells::quote_sheet(c.source_sheet.as_deref()?),
                c.source_ref.as_deref()?
            ))
        })
    });
    let n = cache.map_or(0, |c| c.fields.len());
    let options: Vec<Value> = (0..n as u32)
        .map(|f| {
            let o = p.field(f);
            json!({
                "subtotal": o.subtotal,
                "subtotalTop": o.subtotal_top,
                "blankRow": o.blank_row,
                "repeatLabels": o.repeat_labels,
                "sort": o.sort,
                "sortBy": o.sort_by,
                "hidden": o.hidden.iter().filter_map(|v| item_index(f, v)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "index": index,
        "name": p.name,
        "location": Rng::of(p.location),
        "area": Rng::of(bo_sheet::pivot::occupied(p)),
        "source": source,
        "fields": fields,
        "rows": p.rows,
        "cols": p.cols,
        "values": p.values.iter().map(|d| json!({
            "field": d.field,
            "function": d.function,
            "name": cache.map(|c| bo_sheet::pivot::value_name(c, d)),
            "showAs": d.show_as,
            "baseField": d.base_field,
            "baseItem": d.base_item,
            "numFmt": d.num_fmt_id.map(|id| bo_sheet::numfmt::format_code(&wb.styles, id, locale)),
        })).collect::<Vec<_>>(),
        "filters": p.filters.iter().map(|f| json!({
            "field": f.field,
            "items": f.selected.iter().filter_map(|v| item_index(f.field, v)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "layout": p.layout,
        "dataOnRows": p.data_on_rows,
        "calcItems": cache.map(|c| c.calculated_items.iter().map(|ci| json!({
            "field": ci.field,
            "name": ci.name,
            "formula": ci.formula,
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "rowGrandTotals": p.row_grand_totals,
        "colGrandTotals": p.col_grand_totals,
        "fieldOptions": options,
        "fieldFilters": p.field_filters,
        "style": p.style.as_ref().map(|s| json!({
            "name": s.name,
            "rowHeaders": s.show_row_headers,
            "colHeaders": s.show_col_headers,
            "rowStripes": s.show_row_stripes,
            "colStripes": s.show_col_stripes,
            "lastColumn": s.show_last_column,
        })),
    })
}

/// `pivotGroup` request from the UI.
pub(super) fn group_request(
    by: &str,
    parts: [bool; 4],
    start: Option<f64>,
    end: Option<f64>,
    interval: Option<f64>,
) -> Result<GroupRequest> {
    Ok(match by {
        "none" => GroupRequest::None,
        "dates" => GroupRequest::Dates {
            years: parts[0],
            quarters: parts[1],
            months: parts[2],
            days: parts[3],
        },
        "numbers" => GroupRequest::Numbers {
            start,
            end,
            interval: interval.unwrap_or(1.0),
        },
        other => return Err(bad(format!("unknown grouping '{other}'"))),
    })
}

impl SheetEngine {
    /// Captions from the host with today's date for relative date filters
    /// (the clock serial is in the 1900 system; §18.17.4.1).
    pub(super) fn pivot_labels(&self, labels: Option<PivotLabels>) -> PivotLabels {
        let mut l = labels.unwrap_or_default();
        let d1904 = self.wb.as_ref().is_some_and(|w| w.date1904);
        l.today = self.live.now().map(|t| if d1904 { t - 1462.0 } else { t });
        l
    }

    /// Run a pivot operation as one undoable edit that may touch every sheet.
    pub(super) fn pivot_edit<R>(
        &mut self,
        sheet: usize,
        f: impl FnOnce(&mut Workbook) -> std::result::Result<R, String>,
    ) -> Result<R> {
        let out = self.edit(sheet, super::cmd::Recalc::All, |wb, _| f(wb).map_err(bad))?;
        self.invalidate(None);
        Ok(out)
    }

    /// `pivotMove`: returns the new sheet and index.
    pub(super) fn pivot_move(
        &mut self,
        sheet: usize,
        index: usize,
        dest: At,
        labels: &PivotLabels,
    ) -> Result<Value> {
        self.pivot_edit(sheet, |wb| {
            let i = bo_sheet::pivot::move_pivot(
                wb,
                sheet,
                index,
                dest.sheet,
                CellRef::new(dest.row, dest.col),
                labels,
            )?;
            Ok(json!({ "sheet": dest.sheet, "index": i }))
        })
    }

    /// `pivotChangeSource`: `source` is an A1 range, optionally sheet-qualified.
    pub(super) fn pivot_change_source(
        &mut self,
        sheet: usize,
        index: usize,
        source: &str,
        labels: &PivotLabels,
    ) -> Result<Value> {
        let (si, range) = bo_sheet::cells::resolve_ref(self.wb()?, sheet, source.trim())
            .ok_or_else(|| bad("bad source range"))?;
        self.pivot_edit(sheet, |wb| {
            bo_sheet::pivot::change_source(wb, sheet, index, si, range, labels)?;
            Ok(json!({ "ok": true }))
        })
    }
}

//! Timelines: a strip of periods (years, quarters, months or days) over a pivot table's date
//! field; choosing a range of periods sets a `dateBetween` filter (§18.10.1.37) on the field in
//! every connected pivot table.

use bo_formula::date::{serial_to_ymd, ymd_to_serial, DateSystem};
use bo_model::sheet::{CellValue, PivotFieldFilter, PivotFilterType, Workbook};
use serde::Serialize;

use crate::pivot::{update_pivot, PivotLabels, PivotSpec};

/// One period of a timeline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    /// The label shown (`2024`, `Q1 2024`, `01/2024`, `15/01/2024`).
    pub label: String,
    /// First day (serial).
    pub start: f64,
    /// Last day (serial).
    pub end: f64,
    /// Inside the selection (every period when nothing is filtered).
    pub selected: bool,
}

fn system(wb: &Workbook) -> DateSystem {
    if wb.date1904 {
        DateSystem::Date1904
    } else {
        DateSystem::Date1900
    }
}

/// `2024-01-31T00:00:00` → serial.
fn iso_serial(s: &str, sys: DateSystem) -> Option<f64> {
    let d = s.get(..10)?;
    let mut it = d.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let day: u32 = it.next()?.parse().ok()?;
    ymd_to_serial(y, m, day, sys)
}

fn serial_iso(n: f64, sys: DateSystem) -> Option<String> {
    let (y, m, d) = serial_to_ymd(n.floor(), sys)?;
    Some(format!("{y:04}-{m:02}-{d:02}T00:00:00"))
}

/// The dates of the timeline's pivot cache field (serials).
fn dates(wb: &Workbook, sheet: usize, index: usize) -> Option<(usize, u32, Vec<f64>)> {
    let t = wb.sheets.get(sheet)?.timelines.get(index)?;
    let (si, pi) = wb.sheets.iter().enumerate().find_map(|(si, ws)| {
        ws.pivots
            .iter()
            .position(|p| t.pivots.iter().any(|n| n == &p.name))
            .map(|pi| (si, pi))
    })?;
    let cache_id = wb.sheets[si].pivots[pi].cache_id;
    let cache = wb.pivot_caches.iter().find(|c| c.cache_id == cache_id)?;
    let fi = cache
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(&t.field))?;
    let sys = system(wb);
    let mut out: Vec<f64> = cache.fields[fi]
        .items
        .iter()
        .filter_map(|v| match v {
            CellValue::Number(n) => Some(n.floor()),
            CellValue::Date(d) => iso_serial(d, sys),
            _ => None,
        })
        .collect();
    // A cache read without its items: the records' values.
    if out.is_empty() {
        out = cache
            .records
            .iter()
            .filter_map(|r| match r.get(fi) {
                Some(CellValue::Number(n)) => Some(n.floor()),
                _ => None,
            })
            .collect();
    }
    out.sort_by(f64::total_cmp);
    out.dedup();
    Some((si, fi as u32, out))
}

/// The periods of timeline `index` at its level (0 years, 1 quarters, 2 months, 3 days).
#[must_use]
pub fn periods(wb: &Workbook, sheet: usize, index: usize, vi: bool) -> Vec<Period> {
    let Some(t) = wb.sheets.get(sheet).and_then(|ws| ws.timelines.get(index)) else {
        return Vec::new();
    };
    let Some((_, _, days)) = dates(wb, sheet, index) else {
        return Vec::new();
    };
    let (Some(&first), Some(&last)) = (days.first(), days.last()) else {
        return Vec::new();
    };
    let sys = system(wb);
    let (Some((y0, m0, _)), Some((y1, m1, _))) =
        (serial_to_ymd(first, sys), serial_to_ymd(last, sys))
    else {
        return Vec::new();
    };
    let sel = match (&t.start, &t.end) {
        (Some(a), Some(b)) => iso_serial(a, sys).zip(iso_serial(b, sys)),
        _ => None,
    };
    let mut out = Vec::new();
    let mut push = |label: String, a: Option<f64>, b: Option<f64>| {
        if let (Some(a), Some(b)) = (a, b) {
            let selected = sel.is_none_or(|(s, e)| a <= e && b >= s);
            out.push(Period {
                label,
                start: a,
                end: b,
                selected,
            });
        }
    };
    let month_end = |y: i64, m: u32| {
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        ymd_to_serial(ny, nm, 1, sys).map(|n| n - 1.0)
    };
    match t.level {
        0 => {
            for y in y0..=y1 {
                push(
                    y.to_string(),
                    ymd_to_serial(y, 1, 1, sys),
                    ymd_to_serial(y, 12, 31, sys),
                );
            }
        }
        1 => {
            let (q0, q1) = ((m0 - 1) / 3, (m1 - 1) / 3);
            let mut y = y0;
            let mut q = q0;
            while (y, q) <= (y1, q1) {
                let label = if vi {
                    format!("Quý {} {y}", q + 1)
                } else {
                    format!("Q{} {y}", q + 1)
                };
                push(
                    label,
                    ymd_to_serial(y, q * 3 + 1, 1, sys),
                    month_end(y, q * 3 + 3),
                );
                q += 1;
                if q == 4 {
                    q = 0;
                    y += 1;
                }
            }
        }
        3 => {
            let mut d = first;
            // At most ten years of days.
            let stop = last.min(first + 3660.0);
            while d <= stop {
                if let Some((y, m, day)) = serial_to_ymd(d, sys) {
                    push(format!("{day:02}/{m:02}/{y}"), Some(d), Some(d));
                }
                d += 1.0;
            }
        }
        _ => {
            let mut y = y0;
            let mut m = m0;
            while (y, m) <= (y1, m1) {
                push(
                    format!("{m:02}/{y}"),
                    ymd_to_serial(y, m, 1, sys),
                    month_end(y, m),
                );
                m += 1;
                if m == 13 {
                    m = 1;
                    y += 1;
                }
            }
        }
    }
    out
}

/// Set the time level of timeline `index` (0 years, 1 quarters, 2 months, 3 days) and its
/// caption and style (`None`: unchanged). The caller writes them into the parts of a timeline
/// read from a file.
///
/// # Errors
/// An unknown timeline or level.
pub fn set_options(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    level: Option<u32>,
    caption: Option<String>,
    style: Option<String>,
) -> Result<(), String> {
    let t = wb
        .sheets
        .get_mut(sheet)
        .and_then(|ws| ws.timelines.get_mut(index))
        .ok_or("no such timeline")?;
    if let Some(l) = level {
        if l > 3 {
            return Err("unknown time level".into());
        }
        t.level = l;
    }
    if let Some(c) = caption {
        t.caption = c;
    }
    if let Some(st) = style {
        t.style = (!st.is_empty()).then_some(st);
    }
    Ok(())
}

/// Filter timeline `index` to the days `start..=end` (serials), or clear it with `None`.
///
/// # Errors
/// An unknown timeline or pivot table.
pub fn select(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    range: Option<(f64, f64)>,
    labels: &PivotLabels,
) -> Result<(), String> {
    let (_, field, _) = dates(wb, sheet, index).ok_or("timeline field missing")?;
    let sys = system(wb);
    let pivots = wb.sheets[sheet].timelines[index].pivots.clone();
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
        spec.field_filters.retain(|f| f.field != field);
        if let Some((a, b)) = range {
            spec.field_filters.push(between(field, a, b));
        }
        update_pivot(wb, si, pi, &spec, labels)?;
    }
    let t = &mut wb.sheets[sheet].timelines[index];
    if let Some((a, b)) = range {
        t.start = serial_iso(a.min(b), sys);
        t.end = serial_iso(a.max(b), sys);
    } else {
        t.start = None;
        t.end = None;
    }
    t.dirty = true;
    Ok(())
}

/// Whether cache field `f` holds dates: its number format is a date format, or every item
/// that is not empty is a date value.
fn is_date_field(wb: &Workbook, f: &bo_model::sheet::PivotCacheField) -> bool {
    if f.derived || f.formula.is_some() {
        return false;
    }
    if f.items.iter().any(|v| matches!(v, CellValue::Date(_))) {
        return true;
    }
    let locale = crate::numfmt::Locale::EN_US;
    let code = crate::numfmt::format_code(&wb.styles, f.num_fmt_id, &locale);
    f.num_fmt_id != 0 && crate::numfmt::NumberFormat::parse_with(&code, &locale).is_date()
}

/// Insert a timeline over date field `field` of pivot table `pivot` of sheet `pivot_sheet`,
/// placed at `at` (top-left cell) on sheet `sheet`, showing months. Returns the timeline's
/// index on the sheet.
///
/// # Errors
/// An unknown pivot table or field, a field without dates, or a pivot table over the data
/// model.
pub fn insert_timeline(
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
    let f = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == cache_id)
        .and_then(|c| c.fields.get(field as usize))
        .cloned()
        .ok_or("unknown pivot field")?;
    if !is_date_field(wb, &f) {
        return Err("a timeline needs a date field".into());
    }
    // The dates come from the cache records when the field keeps no items.
    if f.items.is_empty() {
        crate::pivot::ensure_records(wb, pivot_sheet, pivot, labels)?;
    }
    crate::slicer::ensure_ext_id(wb, cache_id)?;
    if let Some(c) = wb.pivot_caches.iter_mut().find(|c| c.cache_id == cache_id) {
        c.timeline_data = true;
    }
    let clean: String = f
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = format!("NativeTimeline_{clean}");
    let taken = |n: &str| {
        wb.defined_names
            .iter()
            .any(|d| d.name.eq_ignore_ascii_case(n))
            || wb
                .sheets
                .iter()
                .flat_map(|s| &s.timelines)
                .any(|t| t.cache.eq_ignore_ascii_case(n))
    };
    let cache = if taken(&stem) {
        (1..10_000)
            .map(|i| format!("{stem}{i}"))
            .find(|n| !taken(n))
            .unwrap_or(stem)
    } else {
        stem
    };
    let name = crate::slicer::frame_name(wb, &f.name);
    wb.defined_names.push(bo_model::sheet::DefinedName {
        name: cache.clone(),
        formula: "#N/A".into(),
        ..bo_model::sheet::DefinedName::default()
    });
    let list = &mut wb.sheets[sheet].timelines;
    list.push(bo_model::sheet::Timeline {
        name,
        caption: f.name.clone(),
        cache,
        field: f.name,
        pivots: vec![pivot_name],
        level: 2,
        from: at,
        to: Some(bo_model::sheet::CellRef::new(at.row + 7, at.col + 6)),
        style: Some("TimeSlicerStyleLight1".into()),
        ..bo_model::sheet::Timeline::default()
    });
    Ok(list.len() - 1)
}

/// The date fields of pivot table `pivot` of sheet `sheet` a timeline can filter (cache field
/// indexes).
#[must_use]
pub fn date_fields(wb: &Workbook, sheet: usize, pivot: usize) -> Vec<u32> {
    let Some(pt) = wb.sheets.get(sheet).and_then(|s| s.pivots.get(pivot)) else {
        return Vec::new();
    };
    wb.pivot_caches
        .iter()
        .find(|c| c.cache_id == pt.cache_id)
        .map(|c| {
            c.fields
                .iter()
                .enumerate()
                .filter(|(_, f)| is_date_field(wb, f))
                .map(|(i, _)| i as u32)
                .collect()
        })
        .unwrap_or_default()
}

/// Connect timeline `index` of sheet `sheet` to the pivot tables named `names` (all on the
/// timeline's pivot cache; at least one). Newly connected pivot tables take the timeline's
/// date range, disconnected ones lose it.
///
/// # Errors
/// An empty list or a pivot table on another cache.
pub fn connect(
    wb: &mut Workbook,
    sheet: usize,
    index: usize,
    names: &[String],
    labels: &PivotLabels,
) -> Result<(), String> {
    let t = wb
        .sheets
        .get(sheet)
        .and_then(|ws| ws.timelines.get(index))
        .cloned()
        .ok_or("no such timeline")?;
    if names.is_empty() {
        return Err("a timeline filters at least one pivot table".into());
    }
    let all = crate::slicer::connectable(wb, &t.pivots);
    if names.iter().any(|n| !all.iter().any(|(_, _, x, _)| x == n)) {
        return Err("the pivot table uses other data".into());
    }
    let (_, field, _) = dates(wb, sheet, index).ok_or("timeline field missing")?;
    let sys = system(wb);
    let range = t
        .start
        .as_deref()
        .and_then(|a| iso_serial(a, sys))
        .zip(t.end.as_deref().and_then(|b| iso_serial(b, sys)));
    wb.sheets[sheet].timelines[index].pivots = names.to_vec();
    for (si, pi, name, was) in all {
        let now = names.contains(&name);
        if now == was {
            continue;
        }
        let mut spec = PivotSpec::of(&wb.sheets[si].pivots[pi]);
        spec.field_filters
            .retain(|f| !(f.field == field && f.kind == PivotFilterType::DateBetween));
        if let (true, Some((a, b))) = (now, range) {
            spec.field_filters.push(between(field, a, b));
        }
        update_pivot(wb, si, pi, &spec, labels)?;
    }
    Ok(())
}

fn between(field: u32, a: f64, b: f64) -> PivotFieldFilter {
    PivotFieldFilter {
        field,
        kind: PivotFilterType::DateBetween,
        measure: 0,
        value1: a.min(b).to_string(),
        value2: a.max(b).to_string(),
        top: true,
        count: 0.0,
    }
}

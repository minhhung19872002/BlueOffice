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
            spec.field_filters.push(PivotFieldFilter {
                field,
                kind: PivotFilterType::DateBetween,
                measure: 0,
                value1: a.min(b).to_string(),
                value2: a.max(b).to_string(),
                top: true,
                count: 0.0,
            });
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

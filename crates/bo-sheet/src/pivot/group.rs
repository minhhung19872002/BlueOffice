//! Grouping of pivot items (ECMA-376 Part 1, §18.10.1.27 `fieldGroup`):
//! date parts and numeric ranges (`rangePr`, §18.10.1.77) and manual groups
//! (`discretePr`, §18.10.1.26).
//!
//! The group captions (`groupItems`, §18.10.1.37) of a range group are, in
//! order: the "before start" item (`<start`), one item per range / date part,
//! the "after end" item (`>end`), and a trailing blank item when the base field
//! has blanks. Values map to an item index with [`bucket`].

use super::PivotLabels;
use bo_formula::date::{
    days_in_month, parse_datetime_text, serial_to_hms, serial_to_ymd, ymd_to_serial,
};
use bo_formula::DateSystem;
use bo_model::sheet::{CellValue, PivotGroupBy, PivotRangeGroup};

/// Numeric value of a record value for grouping (dates by their serial).
pub(crate) fn number_of(v: &CellValue, sys: DateSystem) -> Option<f64> {
    match v {
        CellValue::Number(n) => Some(*n),
        CellValue::Date(s) => parse_datetime_text(&s.replacen('T', " ", 1), sys)
            .or_else(|| parse_datetime_text(s.split('T').next().unwrap_or(s), sys)),
        _ => None,
    }
}

/// Number of items between the "before" and "after" items.
pub(crate) fn middle_count(r: &PivotRangeGroup, sys: DateSystem) -> usize {
    match r.group_by {
        PivotGroupBy::Range => {
            if r.interval <= 0.0 || r.end < r.start {
                1
            } else {
                (((r.end - r.start) / r.interval).floor() as usize).saturating_add(1)
            }
        }
        PivotGroupBy::Years => {
            let y0 = serial_to_ymd(r.start, sys).map_or(0, |d| d.0);
            let y1 = serial_to_ymd(r.end, sys).map_or(y0, |d| d.0);
            usize::try_from(y1 - y0 + 1).unwrap_or(1).max(1)
        }
        PivotGroupBy::Quarters => 4,
        PivotGroupBy::Months => 12,
        PivotGroupBy::Days => 366,
        PivotGroupBy::Hours => 24,
        PivotGroupBy::Minutes | PivotGroupBy::Seconds => 60,
    }
}

/// Day of a leap year (0-based) of a month / day, the calendar of `days` groups.
fn leap_day_index(m: u32, d: u32) -> usize {
    let before: u32 = (1..m).map(|mm| days_in_month(2000, mm)).sum();
    (before + d - 1) as usize
}

/// Index into the middle items of a value inside the range.
fn middle_index(r: &PivotRangeGroup, v: f64, sys: DateSystem) -> Option<usize> {
    Some(match r.group_by {
        PivotGroupBy::Range => {
            if r.interval <= 0.0 {
                0
            } else {
                ((v - r.start) / r.interval).floor().max(0.0) as usize
            }
        }
        PivotGroupBy::Years => {
            let y0 = serial_to_ymd(r.start, sys)?.0;
            usize::try_from(serial_to_ymd(v, sys)?.0 - y0).ok()?
        }
        PivotGroupBy::Quarters => ((serial_to_ymd(v, sys)?.1 - 1) / 3) as usize,
        PivotGroupBy::Months => (serial_to_ymd(v, sys)?.1 - 1) as usize,
        PivotGroupBy::Days => {
            let (_, m, d) = serial_to_ymd(v, sys)?;
            leap_day_index(m, d)
        }
        PivotGroupBy::Hours => serial_to_hms(v).0 as usize,
        PivotGroupBy::Minutes => serial_to_hms(v).1 as usize,
        PivotGroupBy::Seconds => serial_to_hms(v).2 as usize,
    })
}

/// Group item index of a value; `items` is the number of group items.
pub(crate) fn bucket(r: &PivotRangeGroup, v: &CellValue, items: usize, sys: DateSystem) -> u32 {
    let last = items.saturating_sub(1) as u32;
    let mid = middle_count(r, sys);
    // A trailing blank item exists when there is one more item than the range needs.
    let has_blank = items == mid + 3;
    let Some(x) = number_of(v, sys) else {
        return if has_blank { last } else { 0 };
    };
    // Dates compare by day: the end bound covers its whole day.
    let (start, end) = if r.dates {
        (r.start.floor(), r.end.floor() + 1.0)
    } else {
        (r.start, r.end)
    };
    let after = (mid + 1) as u32;
    if x < start {
        return 0;
    }
    if (r.dates && x >= end) || (!r.dates && x > end) {
        return after.min(last);
    }
    middle_index(r, x, sys).map_or(0, |i| (1 + i.min(mid - 1)) as u32)
}

/// Short text of a number for range captions.
fn num_text(n: f64) -> String {
    super::value_text(&CellValue::Number((n * 1e9).round() / 1e9))
}

/// `dd/mm/yyyy` text of a serial (captions of the "before" / "after" items).
fn date_text(serial: f64, sys: DateSystem) -> String {
    serial_to_ymd(serial, sys).map_or_else(
        || num_text(serial),
        |(y, m, d)| format!("{d:02}/{m:02}/{y:04}"),
    )
}

/// The group captions of a range group (see the module documentation).
pub(crate) fn range_items(
    r: &PivotRangeGroup,
    integers: bool,
    blank: bool,
    labels: &PivotLabels,
    sys: DateSystem,
) -> Vec<CellValue> {
    let text = |s: String| CellValue::Text(s);
    let bound = |v: f64| {
        if r.dates {
            date_text(v, sys)
        } else {
            num_text(v)
        }
    };
    let mut out = vec![text(format!("<{}", bound(r.start)))];
    let month = |m: usize| {
        labels
            .months
            .get(m)
            .cloned()
            .unwrap_or_else(|| (m + 1).to_string())
    };
    match r.group_by {
        PivotGroupBy::Range => {
            for i in 0..middle_count(r, sys) {
                let a = r.start + r.interval * i as f64;
                // Whole numbers: "0-9"; otherwise the bounds touch: "0-0.5".
                let b = if integers && r.interval.fract() == 0.0 {
                    a + r.interval - 1.0
                } else {
                    a + r.interval
                };
                out.push(text(format!("{}-{}", num_text(a), num_text(b))));
            }
        }
        PivotGroupBy::Years => {
            let y0 = serial_to_ymd(r.start, sys).map_or(0, |d| d.0);
            for i in 0..middle_count(r, sys) {
                out.push(text((y0 + i as i64).to_string()));
            }
        }
        PivotGroupBy::Quarters => {
            for q in 1..=4 {
                out.push(text(labels.quarter.replace("{n}", &q.to_string())));
            }
        }
        PivotGroupBy::Months => {
            for m in 0..12 {
                out.push(text(month(m)));
            }
        }
        PivotGroupBy::Days => {
            for m in 1..=12u32 {
                for d in 1..=days_in_month(2000, m) {
                    out.push(text(format!("{d}-{}", month(m as usize - 1))));
                }
            }
        }
        PivotGroupBy::Hours | PivotGroupBy::Minutes | PivotGroupBy::Seconds => {
            let n = middle_count(r, sys);
            for i in 0..n {
                out.push(text(format!("{i:02}")));
            }
        }
    }
    out.push(text(format!(">{}", bound(r.end))));
    if blank {
        out.push(CellValue::Empty);
    }
    out
}

/// Range bounds of the values of a field: `(min, max, all integers, has blanks)`.
/// `None` when a value is neither a number nor a date (text cannot be grouped
/// by range) or there are no numbers.
pub(crate) fn value_bounds<'a>(
    values: impl Iterator<Item = &'a CellValue>,
    sys: DateSystem,
) -> Option<(f64, f64, bool, bool)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut integers = true;
    let mut blank = false;
    for v in values {
        if matches!(v, CellValue::Empty) {
            blank = true;
            continue;
        }
        let x = number_of(v, sys)?;
        integers &= x.fract() == 0.0;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    (lo <= hi).then_some((lo, hi, integers, blank))
}

/// Date range bounds for `rangePr` with dates: from the first day to the last day.
pub(crate) fn date_bounds(lo: f64, hi: f64) -> (f64, f64) {
    (lo.floor(), hi.floor())
}

/// First serial of the year of `serial` (for tests and year captions).
#[allow(dead_code)]
pub(crate) fn year_start(serial: f64, sys: DateSystem) -> Option<f64> {
    let (y, _, _) = serial_to_ymd(serial, sys)?;
    ymd_to_serial(y, 1, 1, sys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> PivotLabels {
        PivotLabels::default()
    }

    fn date(y: i64, m: u32, d: u32) -> f64 {
        ymd_to_serial(y, m, d, DateSystem::Date1900).unwrap()
    }

    #[test]
    fn months_and_years() {
        let sys = DateSystem::Date1900;
        let r = PivotRangeGroup {
            group_by: PivotGroupBy::Months,
            auto_start: true,
            auto_end: true,
            start: date(2024, 1, 5),
            end: date(2024, 3, 20),
            interval: 1.0,
            dates: true,
        };
        let items = range_items(&r, true, false, &labels(), sys);
        // "<05/01/2024", Jan … Dec, ">20/03/2024".
        assert_eq!(items.len(), 14);
        assert_eq!(items[0], CellValue::Text("<05/01/2024".into()));
        assert_eq!(items[2], CellValue::Text("Feb".into()));
        let b = |y, m, d| bucket(&r, &CellValue::Number(date(y, m, d)), items.len(), sys);
        assert_eq!(b(2024, 2, 29), 2);
        assert_eq!(b(2024, 3, 20), 3, "the end day is inside");
        assert_eq!(b(2024, 3, 21), 13, "after the end");
        assert_eq!(b(2024, 1, 4), 0, "before the start");
        let y = PivotRangeGroup {
            group_by: PivotGroupBy::Years,
            start: date(2023, 6, 1),
            end: date(2025, 1, 1),
            ..r.clone()
        };
        let items = range_items(&y, true, true, &labels(), sys);
        assert_eq!(items.len(), 3 + 3);
        assert_eq!(items[1], CellValue::Text("2023".into()));
        assert_eq!(items[5], CellValue::Empty);
        assert_eq!(bucket(&y, &CellValue::Number(date(2024, 7, 1)), 6, sys), 2);
        assert_eq!(bucket(&y, &CellValue::Empty, 6, sys), 5, "blank item");
        // ISO date text (cache items of type `d`) groups like its serial.
        assert_eq!(
            bucket(&y, &CellValue::Date("2025-01-01T00:00:00".into()), 6, sys),
            3
        );
    }

    #[test]
    fn numeric_ranges() {
        let sys = DateSystem::Date1900;
        let r = PivotRangeGroup {
            group_by: PivotGroupBy::Range,
            start: 0.0,
            end: 100.0,
            interval: 25.0,
            auto_start: false,
            auto_end: false,
            dates: false,
        };
        // 0-24, 25-49, 50-74, 75-99, 100-124.
        let items = range_items(&r, true, false, &labels(), sys);
        assert_eq!(items.len(), 7);
        assert_eq!(items[1], CellValue::Text("0-24".into()));
        assert_eq!(items[5], CellValue::Text("100-124".into()));
        let b = |v: f64| bucket(&r, &CellValue::Number(v), 7, sys);
        assert_eq!(
            (b(-1.0), b(0.0), b(24.9), b(25.0), b(100.0), b(101.0)),
            (0, 1, 1, 2, 5, 6)
        );
        let items = range_items(
            &PivotRangeGroup {
                interval: 0.5,
                end: 1.0,
                ..r
            },
            false,
            false,
            &labels(),
            sys,
        );
        assert_eq!(items[1], CellValue::Text("0-0.5".into()));
    }

    #[test]
    fn days_quarters_and_bounds() {
        let sys = DateSystem::Date1900;
        let r = PivotRangeGroup {
            group_by: PivotGroupBy::Days,
            start: date(2023, 1, 1),
            end: date(2024, 12, 31),
            interval: 1.0,
            dates: true,
            ..PivotRangeGroup::default()
        };
        let items = range_items(&r, true, false, &labels(), sys);
        assert_eq!(items.len(), 368);
        assert_eq!(items[60], CellValue::Text("29-Feb".into()));
        // 1 March is day 61 of the leap calendar in every year.
        assert_eq!(
            bucket(&r, &CellValue::Number(date(2023, 3, 1)), 368, sys),
            61
        );
        assert_eq!(
            bucket(&r, &CellValue::Number(date(2024, 3, 1)), 368, sys),
            61
        );
        let q = PivotRangeGroup {
            group_by: PivotGroupBy::Quarters,
            ..r
        };
        assert_eq!(bucket(&q, &CellValue::Number(date(2024, 8, 1)), 6, sys), 3);
        assert_eq!(
            value_bounds(
                [
                    CellValue::Number(3.5),
                    CellValue::Empty,
                    CellValue::Number(-1.0)
                ]
                .iter(),
                sys
            ),
            Some((-1.0, 3.5, false, true))
        );
        assert_eq!(
            value_bounds([CellValue::Text("x".into())].iter(), sys),
            None
        );
        assert_eq!(year_start(date(2024, 5, 5), sys), Some(date(2024, 1, 1)));
    }
}

//! AutoFill: extend a selection by dragging the fill handle.
//!
//! Series recognized from the source cells:
//! * numbers — one value is copied (or incremented with `increment_single`), two
//!   or more continue their least-squares linear trend;
//! * dates (numbers with a date format) — by day, or by month / year when the
//!   source dates share the day (and month);
//! * text with a trailing integer (`Quý 1`, `Item 07`) — the number steps;
//! * weekday and month names (English and Vietnamese, full and short forms:
//!   `Thứ Hai`, `T2`, `Tháng Một`, `Tháng 1`, `Thg 1`, `Monday`, `Jan`…) and
//!   user lists;
//! * anything else is repeated cyclically.

use crate::calc::FormulaAdjuster;
use crate::numfmt::{civil_from_days, days_from_civil, format_code, Locale, NumberFormat};
use bo_model::sheet::{CellRange, CellValue, Workbook};

/// Built-in lists (each list is cyclic). Vietnamese first.
pub const BUILTIN_LISTS: &[&[&str]] = &[
    &[
        "Thứ Hai",
        "Thứ Ba",
        "Thứ Tư",
        "Thứ Năm",
        "Thứ Sáu",
        "Thứ Bảy",
        "Chủ Nhật",
    ],
    &["T2", "T3", "T4", "T5", "T6", "T7", "CN"],
    &[
        "Tháng Một",
        "Tháng Hai",
        "Tháng Ba",
        "Tháng Tư",
        "Tháng Năm",
        "Tháng Sáu",
        "Tháng Bảy",
        "Tháng Tám",
        "Tháng Chín",
        "Tháng Mười",
        "Tháng Mười Một",
        "Tháng Mười Hai",
    ],
    &[
        "Tháng 1",
        "Tháng 2",
        "Tháng 3",
        "Tháng 4",
        "Tháng 5",
        "Tháng 6",
        "Tháng 7",
        "Tháng 8",
        "Tháng 9",
        "Tháng 10",
        "Tháng 11",
        "Tháng 12",
    ],
    &[
        "Thg 1", "Thg 2", "Thg 3", "Thg 4", "Thg 5", "Thg 6", "Thg 7", "Thg 8", "Thg 9", "Thg 10",
        "Thg 11", "Thg 12",
    ],
    &[
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
    &["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    &[
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    &[
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    &["Quý 1", "Quý 2", "Quý 3", "Quý 4"],
];

/// Options for series detection.
#[derive(Debug, Clone, Copy, Default)]
pub struct FillOptions<'a> {
    /// The source numbers are dates (their format is a date format).
    pub dates: bool,
    /// Increment a single number by 1 instead of copying it.
    pub increment_single: bool,
    /// Additional user-defined lists.
    pub custom_lists: &'a [Vec<String>],
}

fn list_position(s: &str, lists: &[Vec<String>]) -> Option<(Vec<String>, usize)> {
    let low = s.to_lowercase();
    for l in lists {
        if let Some(i) = l.iter().position(|x| x.to_lowercase() == low) {
            return Some((l.clone(), i));
        }
    }
    for l in BUILTIN_LISTS {
        if let Some(i) = l.iter().position(|x| x.to_lowercase() == low) {
            return Some((l.iter().map(|s| (*s).to_owned()).collect(), i));
        }
    }
    None
}

/// Re-apply the letter case of `like` (all upper / all lower) to `s`.
fn match_case(s: &str, like: &str) -> String {
    let has_alpha = like.chars().any(char::is_alphabetic);
    if has_alpha
        && like
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_uppercase)
    {
        s.to_uppercase()
    } else if has_alpha
        && like
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_lowercase)
    {
        s.to_lowercase()
    } else {
        s.to_owned()
    }
}

/// Split text into (prefix, number, digits) when it ends with an integer.
fn trailing_number(s: &str) -> Option<(&str, i64, usize)> {
    let digits = s.chars().rev().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 15 {
        return None;
    }
    let split = s.len() - digits;
    let n = s[split..].parse().ok()?;
    Some((&s[..split], n, digits))
}

fn linear_trend(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mx = (n - 1.0) / 2.0;
    let my = v.iter().sum::<f64>() / n;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for (i, y) in v.iter().enumerate() {
        let dx = i as f64 - mx;
        sxy += dx * (y - my);
        sxx += dx * dx;
    }
    let slope = if sxx == 0.0 { 0.0 } else { sxy / sxx };
    (my - slope * mx, slope)
}

fn add_months(serial: f64, months: i64) -> f64 {
    // Serial ↔ civil in the 1900 system: days since 1899-12-30 (valid from 1900-03-01).
    let day0 = days_from_civil(1899, 12, 30);
    let whole = serial.floor();
    let frac = serial - whole;
    let (y, m, d) = civil_from_days(day0 + whole as i64);
    let total = i64::from(y) * 12 + i64::from(m) - 1 + months;
    let (ny, nm) = (
        total.div_euclid(12) as i32,
        (total.rem_euclid(12) + 1) as u32,
    );
    let last = match nm {
        2 if (ny % 4 == 0 && ny % 100 != 0) || ny % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (days_from_civil(ny, nm, d.min(last)) - day0) as f64 + frac
}

/// Produce `count` values continuing `source` (an empty source yields empties).
pub fn fill_series(source: &[CellValue], count: usize, opts: &FillOptions<'_>) -> Vec<CellValue> {
    if source.is_empty() {
        return vec![CellValue::Empty; count];
    }
    let n = source.len();
    let cyclic = |i: usize| source[i % n].clone();
    // Numbers / dates.
    let nums: Option<Vec<f64>> = source
        .iter()
        .map(|v| match v {
            CellValue::Number(x) => Some(*x),
            _ => None,
        })
        .collect();
    if let Some(nums) = nums {
        if opts.dates {
            let day0 = days_from_civil(1899, 12, 30);
            let civil = |s: f64| civil_from_days(day0 + s.floor() as i64);
            let step_months = if n >= 2 {
                let (y0, m0, d0) = civil(nums[0]);
                let (y1, m1, d1) = civil(nums[1]);
                let months = (i64::from(y1) - i64::from(y0)) * 12 + i64::from(m1) - i64::from(m0);
                (d0 == d1 && months != 0).then_some(months)
            } else {
                None
            };
            let last = nums[n - 1];
            let step = if n >= 2 {
                nums[n - 1] - nums[n - 2]
            } else {
                1.0
            };
            return (1..=count)
                .map(|k| {
                    CellValue::Number(
                        step_months
                            .map_or(last + step * k as f64, |m| add_months(last, m * k as i64)),
                    )
                })
                .collect();
        }
        if n == 1 && !opts.increment_single {
            return (0..count).map(cyclic).collect();
        }
        let (a, b) = if n == 1 {
            (nums[0], 1.0)
        } else {
            linear_trend(&nums)
        };
        return (0..count)
            .map(|k| {
                let x = (n + k) as f64;
                let v = a + b * x;
                // Avoid binary noise like 0.30000000000000004.
                CellValue::Number((v * 1e12).round() / 1e12)
            })
            .collect();
    }
    let texts: Option<Vec<String>> = source.iter().map(CellValue::as_text).collect();
    if let Some(texts) = texts {
        // Lists (weekdays, months, custom).
        let pos: Option<Vec<(Vec<String>, usize)>> = texts
            .iter()
            .map(|t| list_position(t, opts.custom_lists))
            .collect();
        if let Some(pos) = pos {
            if pos.iter().all(|(l, _)| *l == pos[0].0) {
                let list = &pos[0].0;
                let len = list.len() as i64;
                let step = if n >= 2 {
                    pos[n - 1].1 as i64 - pos[n - 2].1 as i64
                } else {
                    1
                };
                let last = pos[n - 1].1 as i64;
                let like = &texts[n - 1];
                return (1..=count)
                    .map(|k| {
                        let i = (last + step * k as i64).rem_euclid(len) as usize;
                        CellValue::Text(match_case(&list[i], like))
                    })
                    .collect();
            }
        }
        // Text with a trailing number and a common prefix.
        let parts: Option<Vec<(&str, i64, usize)>> =
            texts.iter().map(|t| trailing_number(t)).collect();
        if let Some(parts) = parts {
            if parts.iter().all(|p| p.0 == parts[0].0) {
                let step = if n >= 2 {
                    parts[n - 1].1 - parts[n - 2].1
                } else {
                    1
                };
                let (prefix, last, width) = parts[n - 1];
                return (1..=count)
                    .map(|k| {
                        let v = last + step * k as i64;
                        let digits = if v < 0 {
                            format!("-{:0width$}", -v, width = width)
                        } else {
                            format!("{v:0width$}")
                        };
                        CellValue::Text(format!("{prefix}{digits}"))
                    })
                    .collect();
            }
        }
    }
    (0..count).map(cyclic).collect()
}

/// AutoFill `target` from `source` on sheet `sheet`. `target` must contain
/// `source` and extend it in one direction (down, up, right or left). Styles
/// repeat cyclically; formulas are moved through `adj` (see
/// [`FormulaAdjuster::translate`]) or copied unchanged.
pub fn autofill(
    wb: &mut Workbook,
    sheet: usize,
    source: CellRange,
    target: CellRange,
    locale: &Locale,
    adj: &dyn FormulaAdjuster,
) -> bool {
    let Some(ws) = wb.sheets.get(sheet) else {
        return false;
    };
    let (s, t) = (source, target);
    let vertical = t.first.col == s.first.col && t.last.col == s.last.col;
    let horizontal = t.first.row == s.first.row && t.last.row == s.last.row;
    let forward = t.first == s.first;
    let backward = t.last == s.last;
    if !(vertical || horizontal) || !(forward || backward) || t == s {
        return false;
    }
    let along_rows = vertical; // series run along rows (down/up)
    let lanes = if along_rows {
        s.first.col..=s.last.col
    } else {
        s.first.row..=s.last.row
    };
    let src_len = if along_rows { s.rows() } else { s.cols() } as usize;
    let tgt_len = if along_rows { t.rows() } else { t.cols() } as usize;
    let extra = tgt_len - src_len;
    let mut writes = Vec::new();
    for lane in lanes {
        let at = |i: usize| -> (u32, u32) {
            let i = i as u32;
            if along_rows {
                (s.first.row + i, lane)
            } else {
                (lane, s.first.col + i)
            }
        };
        let mut cells: Vec<_> = (0..src_len)
            .map(|i| ws.cell(at(i).0, at(i).1).cloned())
            .collect();
        if !forward {
            cells.reverse();
        }
        let values: Vec<CellValue> = cells
            .iter()
            .map(|c| c.as_ref().map_or(CellValue::Empty, |c| c.value.clone()))
            .collect();
        let is_date = cells.iter().flatten().any(|c| {
            let id = wb.styles.xf(c.style).map_or(0, |x| x.num_fmt_id);
            NumberFormat::parse_with(&format_code(&wb.styles, id, locale), locale).is_date()
        });
        let series = fill_series(
            &values,
            extra,
            &FillOptions {
                dates: is_date,
                ..FillOptions::default()
            },
        );
        for (k, v) in series.into_iter().enumerate() {
            let src = cells[k % src_len].clone();
            let offset = (k + src_len) as i64;
            let (r, c) = if forward {
                let (r, c) = at(0);
                if along_rows {
                    (r + offset as u32, c)
                } else {
                    (r, c + offset as u32)
                }
            } else {
                let (r, c) = at(src_len - 1);
                if along_rows {
                    (r - offset as u32, c)
                } else {
                    (r, c - offset as u32)
                }
            };
            let mut cell = src.unwrap_or_default();
            if let Some(f) = &mut cell.formula {
                // Distance from the source cell the formula is copied from.
                let (sr, sc) = if forward {
                    at(k % src_len)
                } else {
                    at(src_len - 1 - k % src_len)
                };
                let (dr, dc) = (i64::from(r) - i64::from(sr), i64::from(c) - i64::from(sc));
                if let Some(t) = adj.translate(&f.text, dr, dc) {
                    f.text = t;
                }
            } else {
                cell.value = v;
            }
            writes.push(((r, c), cell));
        }
    }
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    for ((r, c), cell) in writes {
        if cell.value == CellValue::Empty && cell.style == 0 && cell.formula.is_none() {
            ws.cells.remove(&(r, c));
        } else {
            ws.cells.insert((r, c), cell);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::NoFormulas;
    use bo_model::sheet::{Cell, CellXf};

    fn t(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }
    fn n(v: f64) -> CellValue {
        CellValue::Number(v)
    }
    fn series(src: &[CellValue], count: usize) -> Vec<CellValue> {
        fill_series(src, count, &FillOptions::default())
    }

    #[test]
    fn numbers() {
        assert_eq!(series(&[n(5.0)], 2), [n(5.0), n(5.0)]);
        let inc = FillOptions {
            increment_single: true,
            ..FillOptions::default()
        };
        assert_eq!(fill_series(&[n(5.0)], 2, &inc), [n(6.0), n(7.0)]);
        assert_eq!(series(&[n(1.0), n(3.0)], 3), [n(5.0), n(7.0), n(9.0)]);
        assert_eq!(series(&[n(0.1), n(0.2)], 2), [n(0.3), n(0.4)]);
        // Least-squares trend of 1, 2, 4 → slope 1.5, intercept 0.8333…
        assert_eq!(series(&[n(1.0), n(2.0), n(4.0)], 1), [n(5.333_333_333_333)]);
    }

    #[test]
    fn dates() {
        let d = FillOptions {
            dates: true,
            ..FillOptions::default()
        };
        // 45000 = 2023-03-15.
        assert_eq!(
            fill_series(&[n(45_000.0)], 2, &d),
            [n(45_001.0), n(45_002.0)]
        );
        assert_eq!(
            fill_series(&[n(45_000.0), n(45_007.0)], 1, &d),
            [n(45_014.0)]
        );
        // 2023-01-31, 2023-02-28 differ in day → day step; month step from same day:
        // 2023-01-15 (44941) → 2023-02-15 (44972) → 2023-03-15 (45000), 2023-04-15 (45031).
        assert_eq!(
            fill_series(&[n(44_941.0), n(44_972.0)], 2, &d),
            [n(45_000.0), n(45_031.0)]
        );
        // Month end clamps: 2024-01-31 (45322) + 1 month → 2024-02-29 (45351).
        assert_eq!(add_months(45_322.0, 1), 45_351.0);
        // Yearly: 2023-03-15 → 2024-03-15 (45366).
        assert_eq!(
            fill_series(&[n(44_635.0), n(45_000.0)], 1, &d),
            [n(45_366.0)]
        );
    }

    #[test]
    fn lists_and_text() {
        assert_eq!(
            series(&[t("Thứ Hai")], 3),
            [t("Thứ Ba"), t("Thứ Tư"), t("Thứ Năm")]
        );
        assert_eq!(series(&[t("Thứ Bảy")], 2), [t("Chủ Nhật"), t("Thứ Hai")]);
        assert_eq!(series(&[t("T6")], 3), [t("T7"), t("CN"), t("T2")]);
        assert_eq!(
            series(&[t("Tháng Mười Một")], 2),
            [t("Tháng Mười Hai"), t("Tháng Một")]
        );
        assert_eq!(series(&[t("Tháng 11")], 2), [t("Tháng 12"), t("Tháng 1")]);
        assert_eq!(series(&[t("Thg 12")], 1), [t("Thg 1")]);
        assert_eq!(series(&[t("jan"), t("mar")], 2), [t("may"), t("jul")]);
        assert_eq!(series(&[t("MONDAY")], 1), [t("TUESDAY")]);
        assert_eq!(series(&[t("Sun")], 1), [t("Mon")]);
        assert_eq!(series(&[t("Quý 3")], 2), [t("Quý 4"), t("Quý 1")]);
        assert_eq!(series(&[t("Item 1")], 2), [t("Item 2"), t("Item 3")]);
        assert_eq!(series(&[t("HĐ-007"), t("HĐ-010")], 1), [t("HĐ-013")]);
        assert_eq!(series(&[t("a"), t("b")], 3), [t("a"), t("b"), t("a")]);
        let custom = vec![vec!["Bắc".to_owned(), "Trung".to_owned(), "Nam".to_owned()]];
        let o = FillOptions {
            custom_lists: &custom,
            ..FillOptions::default()
        };
        assert_eq!(fill_series(&[t("Trung")], 2, &o), [t("Nam"), t("Bắc")]);
        assert_eq!(series(&[], 1), [CellValue::Empty]);
        assert_eq!(series(&[CellValue::Bool(true)], 1), [CellValue::Bool(true)]);
    }

    #[test]
    #[allow(clippy::items_after_statements)]
    fn autofill_down_right_and_up() {
        let mut wb = Workbook::new();
        let date_fmt = wb.styles.add_xf(CellXf {
            num_fmt_id: 14,
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        ws.set_cell(0, 0, Cell::new(n(1.0)));
        ws.set_cell(1, 0, Cell::new(n(2.0)));
        ws.set_cell(0, 1, Cell::new(n(45_000.0)).with_style(date_fmt));
        ws.set_cell(0, 2, Cell::new(n(10.0)).with_formula("A1*2"));
        let src = CellRange::parse("A1:A2").unwrap();
        assert!(autofill(
            &mut wb,
            0,
            src,
            CellRange::parse("A1:A5").unwrap(),
            &Locale::VI_VN,
            &NoFormulas
        ));
        let v = |wb: &Workbook, a1: &str| wb.sheets[0].cell_a1(a1).map(|c| c.value.clone());
        assert_eq!(v(&wb, "A5"), Some(n(5.0)));
        // Date fills by day to the right.
        assert!(autofill(
            &mut wb,
            0,
            CellRange::parse("B1").unwrap(),
            CellRange::parse("B1:B3").unwrap(),
            &Locale::VI_VN,
            &NoFormulas
        ));
        assert_eq!(v(&wb, "B3"), Some(n(45_002.0)));
        assert_eq!(wb.sheets[0].cell_a1("B3").unwrap().style, date_fmt);
        // Formula cells are copied with their cached value; the adjuster translates.
        struct Tr;
        impl FormulaAdjuster for Tr {
            fn adjust(
                &self,
                _: &str,
                _: Option<&str>,
                _: &crate::calc::StructuralChange,
            ) -> Option<String> {
                None
            }
            fn translate(&self, f: &str, dr: i64, dc: i64) -> Option<String> {
                Some(format!("{f}+{dr},{dc}"))
            }
        }
        assert!(autofill(
            &mut wb,
            0,
            CellRange::parse("C1").unwrap(),
            CellRange::parse("C1:C2").unwrap(),
            &Locale::VI_VN,
            &Tr
        ));
        assert_eq!(
            wb.sheets[0]
                .cell_a1("C2")
                .unwrap()
                .formula
                .as_ref()
                .unwrap()
                .text,
            "A1*2+1,0"
        );
        // Upward fill continues backwards; invalid shapes are rejected.
        wb.sheets[0].set_cell(10, 4, Cell::new(n(10.0)));
        wb.sheets[0].set_cell(11, 4, Cell::new(n(20.0)));
        assert!(autofill(
            &mut wb,
            0,
            CellRange::parse("E11:E12").unwrap(),
            CellRange::parse("E9:E12").unwrap(),
            &Locale::VI_VN,
            &NoFormulas
        ));
        assert_eq!(v(&wb, "E9"), Some(n(-10.0)));
        assert_eq!(v(&wb, "E10"), Some(n(0.0)));
        assert!(!autofill(
            &mut wb,
            0,
            src,
            CellRange::parse("B2:C9").unwrap(),
            &Locale::VI_VN,
            &NoFormulas
        ));
    }
}

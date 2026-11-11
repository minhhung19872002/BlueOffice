//! AutoFilter evaluation (ECMA-376 Part 1, §18.3.1.2 `autoFilter`, §18.3.2).
//!
//! A row of the filter range (header excluded) is shown when it satisfies the
//! criterion of every filter column. Value lists compare the cell's *displayed*
//! text case-insensitively (§18.3.2.6 `filter/@val`); custom filters
//! (§18.3.2.2) compare numerically when both sides are numbers and as text with
//! `*` / `?` wildcards otherwise; `top10` (§18.3.2.10) keeps the N largest /
//! smallest numbers (or N percent); date groups (§18.3.2.4 `dateGroupItem`),
//! dynamic filters (§18.3.2.5, relative dates and above/below average) and
//! colour filters (§18.3.2.1) are evaluated too. Criteria kept as raw XML are
//! not evaluated (rows stay visible).

use crate::numfmt::{format_code, FormatOptions, Locale, NumberFormat};
use crate::render::format_cell;
use bo_model::sheet::{
    AutoFilter, Cell, CellValue, ColumnFilter, CustomFilter, DateGroup, Fill, RowInfo, Workbook,
    Worksheet,
};

/// `*` / `?` wildcard match (`~` escapes), case-insensitive.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    // Iterative matcher with backtracking on the last `*`.
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    star = Some((pi, ti));
                    pi += 1;
                    continue;
                }
                '?' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '~' if pi + 1 < p.len() && p[pi + 1] == t[ti] => {
                    pi += 2;
                    ti += 1;
                    continue;
                }
                c if c == t[ti] => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                _ => {}
            }
        }
        match star {
            Some((sp, st)) => {
                pi = sp + 1;
                ti = st + 1;
                star = Some((sp, st + 1));
            }
            None => return false,
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

fn custom_match(c: &CustomFilter, value: &CellValue, text: &str) -> bool {
    let op = c.operator.as_deref().unwrap_or("equal");
    let num = match value {
        CellValue::Number(n) => Some(*n),
        _ => None,
    };
    if let (Some(n), Ok(v)) = (num, c.val.trim().parse::<f64>()) {
        return match op {
            "lessThan" => n < v,
            "lessThanOrEqual" => n <= v,
            "notEqual" => n != v,
            "greaterThanOrEqual" => n >= v,
            "greaterThan" => n > v,
            _ => n == v,
        };
    }
    let a = text.to_lowercase();
    let b = c.val.to_lowercase();
    match op {
        "notEqual" => !wildcard_match(&c.val, text),
        "lessThan" => a < b,
        "lessThanOrEqual" => a <= b,
        "greaterThan" => a > b,
        "greaterThanOrEqual" => a >= b,
        _ => wildcard_match(&c.val, text),
    }
}

/// Serial date of a cell value (numbers and ISO `t="d"` dates).
fn date_of(wb: &Workbook, v: &CellValue) -> Option<f64> {
    match v {
        CellValue::Number(n) => Some(*n),
        CellValue::Date(iso) => {
            let (d, t) = iso.split_once('T').unwrap_or((iso, ""));
            let mut p = d.split('-').map(|x| x.parse::<u32>().ok());
            let (y, m, dd) = (p.next()??, p.next()??, p.next()??);
            let mut tp = t
                .split(':')
                .map(|x| x.trim_end_matches('Z').parse::<f64>().ok());
            let h = tp.next().flatten().unwrap_or(0.0);
            let mi = tp.next().flatten().unwrap_or(0.0);
            let s = tp.next().flatten().unwrap_or(0.0);
            crate::numfmt::datetime_to_serial(y as i32, m, dd, h as u32, mi as u32, s, wb.date1904)
        }
        _ => None,
    }
}

/// §18.3.2.4: does the date fall into the group (fields up to its grouping match)?
fn in_group(wb: &Workbook, serial: f64, g: &DateGroup) -> bool {
    let Some(d) = crate::numfmt::serial_to_datetime(serial, wb.date1904, 0) else {
        return false;
    };
    let depth = match g.grouping.as_str() {
        "year" => 1,
        "month" => 2,
        "day" => 3,
        "hour" => 4,
        "minute" => 5,
        _ => 6,
    };
    let parts = [
        (Some(g.year), d.year as u32),
        (g.month, d.month),
        (g.day, d.day),
        (g.hour, d.hour),
        (g.minute, d.minute),
        (g.second, d.second),
    ];
    parts
        .iter()
        .take(depth)
        .all(|(want, got)| want.is_none_or(|w| w == *got))
}

/// Serial-day range `[from, to)` of a relative date filter (§18.18.26
/// `ST_DynamicFilterType`), relative to `now`; weeks start on Sunday.
fn dynamic_range(kind: &str, now: f64, date1904: bool) -> Option<(f64, f64)> {
    let today = now.floor();
    let d = crate::numfmt::serial_to_datetime(today, date1904, 0)?;
    // Month numbers may overflow (0 = previous December, 13 = next January).
    let serial = |y: i32, m: i32, day: u32| {
        let (y, m) = (
            y + (m - 1).div_euclid(12),
            (m - 1).rem_euclid(12) as u32 + 1,
        );
        crate::numfmt::datetime_to_serial(y, m, day, 0, 0, 0.0, date1904)
    };
    let month = d.month as i32;
    let months = |from: i32, n: i32| -> Option<(f64, f64)> {
        Some((serial(d.year, from, 1)?, serial(d.year, from + n, 1)?))
    };
    let quarter_start = (month - 1) / 3 * 3 + 1;
    let year = |off: i32| Some((serial(d.year + off, 1, 1)?, serial(d.year + off + 1, 1, 1)?));
    let week_start = today - f64::from(d.weekday);
    Some(match kind {
        "today" => (today, today + 1.0),
        "yesterday" => (today - 1.0, today),
        "tomorrow" => (today + 1.0, today + 2.0),
        "thisWeek" => (week_start, week_start + 7.0),
        "lastWeek" => (week_start - 7.0, week_start),
        "nextWeek" => (week_start + 7.0, week_start + 14.0),
        "thisMonth" => months(month, 1)?,
        "lastMonth" => months(month - 1, 1)?,
        "nextMonth" => months(month + 1, 1)?,
        "thisQuarter" => months(quarter_start, 3)?,
        "lastQuarter" => months(quarter_start - 3, 3)?,
        "nextQuarter" => months(quarter_start + 3, 3)?,
        "thisYear" => year(0)?,
        "lastYear" => year(-1)?,
        "nextYear" => year(1)?,
        "yearToDate" => (serial(d.year, 1, 1)?, today + 1.0),
        _ => return None,
    })
}

/// Month (`M1`–`M12`) or quarter (`Q1`–`Q4`) of any year.
fn period_match(kind: &str, serial: f64, date1904: bool) -> Option<bool> {
    let d = crate::numfmt::serial_to_datetime(serial, date1904, 0)?;
    if let Some(m) = kind.strip_prefix('M').and_then(|m| m.parse::<u32>().ok()) {
        return Some(d.month == m);
    }
    if let Some(q) = kind.strip_prefix('Q').and_then(|q| q.parse::<u32>().ok()) {
        return Some((d.month - 1) / 3 + 1 == q);
    }
    None
}

/// Colour a colour filter looks for: the dxf's fill (`cell_color`) or font colour.
fn filter_color(wb: &Workbook, dxf_id: Option<u32>, cell_color: bool) -> Option<(u8, u8, u8)> {
    let dxf = wb.styles.dxfs.get(dxf_id? as usize)?;
    let theme = wb.theme.as_ref();
    let c = if cell_color {
        match dxf.fill.as_ref()? {
            // Differential solid fills carry their colour in bgColor (§18.8.14).
            Fill::Pattern(p) => p.bg.as_ref().or(p.fg.as_ref())?,
            Fill::Gradient(g) => &g.stops.first()?.1,
        }
    } else {
        dxf.font.as_ref()?.color.as_ref()?
    };
    let c = crate::color::resolve_color(c, theme)?;
    Some((c.0, c.1, c.2))
}

/// Row spans (first, last data row, zero-based) of the filters of `ws` that have a criterion:
/// the sheet's AutoFilter (§18.3.1.2) and the tables' filters. A hidden row inside such a
/// span is filtered out (`SUBTOTAL` skips it with every function number).
#[must_use]
pub fn filter_spans(ws: &Worksheet) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if let Some(af) = ws.auto_filter.as_ref().filter(|af| !af.columns.is_empty()) {
        out.push((af.range.first.row + 1, af.range.last.row));
    }
    for t in &ws.tables {
        if t.auto_filter
            .as_deref()
            .is_some_and(|x| x.contains("filterColumn"))
        {
            out.push((t.range.first.row + 1, t.range.last.row));
        }
    }
    out.retain(|&(a, b)| a <= b);
    out
}

/// Zero-based rows (absolute) that the AutoFilter `af` hides.
pub fn hidden_rows(wb: &Workbook, ws: &Worksheet, af: &AutoFilter, locale: &Locale) -> Vec<u32> {
    hidden_rows_at(wb, ws, af, locale, None)
}

/// [`hidden_rows`] with an explicit "now" (serial) for relative date filters;
/// `None` reads the system clock.
#[allow(clippy::too_many_lines)]
pub fn hidden_rows_at(
    wb: &Workbook,
    ws: &Worksheet,
    af: &AutoFilter,
    locale: &Locale,
    now: Option<f64>,
) -> Vec<u32> {
    let now = now.unwrap_or_else(|| {
        bo_formula::date::now_serial(if wb.date1904 {
            bo_formula::DateSystem::Date1904
        } else {
            bo_formula::DateSystem::Date1900
        })
    });
    let first = af.range.first.row + 1;
    let last = af.range.last.row;
    if first > last {
        return Vec::new();
    }
    // Pre-compute top-10 thresholds per column.
    let thresholds: Vec<Option<(bool, f64)>> = af
        .columns
        .iter()
        .map(|fc| match fc.filter {
            ColumnFilter::Top10 {
                top, percent, val, ..
            } => {
                let col = af.range.first.col + fc.col_id;
                let mut nums: Vec<f64> = (first..=last)
                    .filter_map(|r| match ws.cell(r, col).map(|c| &c.value) {
                        Some(CellValue::Number(n)) => Some(*n),
                        _ => None,
                    })
                    .collect();
                if nums.is_empty() {
                    return None;
                }
                nums.sort_by(f64::total_cmp);
                if top {
                    nums.reverse();
                }
                let n = if percent {
                    ((nums.len() as f64) * val / 100.0).floor().max(1.0) as usize
                } else {
                    val.max(1.0) as usize
                };
                let cut = nums[n.min(nums.len()) - 1];
                Some((top, cut))
            }
            ColumnFilter::Dynamic { ref kind, .. }
                if kind == "aboveAverage" || kind == "belowAverage" =>
            {
                // §18.3.2.5: the average of the column's numbers.
                let col = af.range.first.col + fc.col_id;
                let nums: Vec<f64> = (first..=last)
                    .filter_map(|r| match ws.cell(r, col).map(|c| &c.value) {
                        Some(CellValue::Number(n)) => Some(*n),
                        _ => None,
                    })
                    .collect();
                (!nums.is_empty()).then(|| {
                    (
                        kind == "aboveAverage",
                        nums.iter().sum::<f64>() / nums.len() as f64,
                    )
                })
            }
            _ => None,
        })
        .collect();
    let colors: Vec<Option<(u8, u8, u8)>> = af
        .columns
        .iter()
        .map(|fc| match fc.filter {
            ColumnFilter::Color { dxf_id, cell_color } => filter_color(wb, dxf_id, cell_color),
            _ => None,
        })
        .collect();
    let cell_rgb = |cell: Option<&Cell>, fill: bool| -> Option<(u8, u8, u8)> {
        let cell = cell?;
        if fill {
            crate::color::cell_fill_rgb(wb, cell)
        } else {
            Some(crate::color::cell_font_rgb(wb, cell).unwrap_or((0, 0, 0)))
        }
    };
    // §18.3.2.9 iconFilter: icons the sheet's conditional formats show in the range.
    let icons: std::collections::HashMap<(u32, u32), (String, usize)> = if af
        .columns
        .iter()
        .any(|fc| matches!(fc.filter, ColumnFilter::Icon { .. }))
    {
        wb.sheets
            .iter()
            .position(|s| std::ptr::eq(s, ws))
            .map(|sheet| {
                crate::cf::evaluate(
                    wb,
                    sheet,
                    first..last + 1,
                    af.range.first.col..af.range.last.col + 1,
                    Some(now),
                )
                .into_iter()
                .filter_map(|(k, f)| f.icon.map(|i| (k, (i.set, i.index))))
                .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    // Displayed text: number formats parsed once per style (not once per cell).
    let mut formats: std::collections::HashMap<u32, NumberFormat> =
        std::collections::HashMap::new();
    let fopts = FormatOptions {
        locale: *locale,
        date1904: wb.date1904,
    };
    let mut display = |cell: Option<&Cell>| -> String {
        let Some(cell) = cell else {
            return String::new();
        };
        formats
            .entry(cell.style)
            .or_insert_with(|| {
                let id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
                NumberFormat::parse_with(&format_code(&wb.styles, id, locale), locale)
            })
            .format(&cell.value, &fopts)
            .text
    };
    // Value lists, lower-cased once.
    let value_sets: Vec<std::collections::HashSet<String>> = af
        .columns
        .iter()
        .map(|fc| match &fc.filter {
            ColumnFilter::Values { values, .. } | ColumnFilter::DateGroups { values, .. } => {
                values.iter().map(|v| v.to_lowercase()).collect()
            }
            _ => std::collections::HashSet::new(),
        })
        .collect();
    let empty = CellValue::Empty;
    let mut hidden = Vec::new();
    for r in first..=last {
        let mut visible = true;
        for (((fc, th), want), set) in af
            .columns
            .iter()
            .zip(&thresholds)
            .zip(&colors)
            .zip(&value_sets)
        {
            let col = af.range.first.col + fc.col_id;
            let cell = ws.cell(r, col);
            let value = cell.map_or(&empty, |c| &c.value);
            let ok = match &fc.filter {
                ColumnFilter::None | ColumnFilter::Raw(_) => true,
                ColumnFilter::Values {
                    blank: show_blank, ..
                } => {
                    let text = display(cell);
                    if text.is_empty() {
                        *show_blank
                    } else {
                        set.contains(&text.to_lowercase())
                    }
                }
                ColumnFilter::Custom { and, conditions } => {
                    // The text is only needed by conditions not compared as numbers.
                    let numeric = |c: &CustomFilter| {
                        matches!(value, CellValue::Number(_)) && c.val.trim().parse::<f64>().is_ok()
                    };
                    let text = if conditions.iter().all(numeric) {
                        String::new()
                    } else {
                        display(cell)
                    };
                    let mut it = conditions.iter().map(|c| custom_match(c, value, &text));
                    if *and {
                        it.all(|b| b)
                    } else {
                        conditions.is_empty() || it.any(|b| b)
                    }
                }
                ColumnFilter::Top10 { .. } => match (th, value) {
                    (Some((true, cut)), CellValue::Number(n)) => n >= cut,
                    (Some((false, cut)), CellValue::Number(n)) => n <= cut,
                    _ => false,
                },
                ColumnFilter::DateGroups {
                    blank: show_blank,
                    groups,
                    ..
                } => {
                    let text = display(cell);
                    if text.is_empty() {
                        *show_blank
                    } else {
                        set.contains(&text.to_lowercase())
                            || date_of(wb, value)
                                .is_some_and(|d| groups.iter().any(|g| in_group(wb, d, g)))
                    }
                }
                ColumnFilter::Dynamic { kind, .. } => match (kind.as_str(), value) {
                    ("aboveAverage" | "belowAverage", CellValue::Number(n)) => match th {
                        Some((true, avg)) => n > avg,
                        Some((false, avg)) => n < avg,
                        None => false,
                    },
                    ("null", _) => true,
                    (k, v) => match date_of(wb, v) {
                        Some(d) => match dynamic_range(k, now, wb.date1904) {
                            Some((a, b)) => d >= a && d < b,
                            None => period_match(k, d, wb.date1904).unwrap_or(true),
                        },
                        None => false,
                    },
                },
                ColumnFilter::Color { cell_color, .. } => {
                    want.is_some_and(|w| cell_rgb(cell, *cell_color) == Some(w))
                }
                ColumnFilter::Icon { icon_set, icon_id } => {
                    match (icons.get(&(r, col)), icon_id) {
                        (Some((set, i)), Some(id)) => set == icon_set && *i == *id as usize,
                        // No `iconId`: the cells showing no icon.
                        (None, None) => true,
                        _ => false,
                    }
                }
            };
            if !ok {
                visible = false;
                break;
            }
        }
        if !visible {
            hidden.push(r);
        }
    }
    hidden
}

/// Apply the sheet's AutoFilter: rows inside the range are hidden or shown.
/// Returns the number of hidden rows.
pub fn apply_auto_filter(wb: &mut Workbook, sheet: usize, locale: &Locale) -> usize {
    apply_auto_filter_at(wb, sheet, locale, None)
}

/// [`apply_auto_filter`] with an explicit "now" (serial) for relative date filters.
pub fn apply_auto_filter_at(
    wb: &mut Workbook,
    sheet: usize,
    locale: &Locale,
    now: Option<f64>,
) -> usize {
    let Some(ws) = wb.sheets.get(sheet) else {
        return 0;
    };
    let Some(af) = ws.auto_filter.clone() else {
        return 0;
    };
    let hidden = hidden_rows_at(wb, ws, &af, locale, now);
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return 0;
    };
    for r in af.range.first.row + 1..=af.range.last.row {
        let hide = hidden.binary_search(&r).is_ok();
        let info = ws.rows.entry(r).or_default();
        info.hidden = hide;
        if *info == RowInfo::default() {
            ws.rows.remove(&r);
        }
    }
    hidden.len()
}

/// Distinct displayed values of filter column `col_id` (for the dropdown), sorted
/// case-insensitively; the empty string stands for blanks.
pub fn column_values(
    wb: &Workbook,
    ws: &Worksheet,
    af: &AutoFilter,
    col_id: u32,
    locale: &Locale,
) -> Vec<String> {
    let col = af.range.first.col + col_id;
    let mut v: Vec<String> = (af.range.first.row + 1..=af.range.last.row)
        .map(|r| {
            ws.cell(r, col)
                .map(|c| format_cell(wb, c, locale).text)
                .unwrap_or_default()
        })
        .collect();
    v.sort_by(|a, b| {
        crate::sort::text_key(a)
            .cmp(&crate::sort::text_key(b))
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    v.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
    v
}

/// Years and months of the date-formatted numbers of filter column `col_id`
/// (for the date tree of the dropdown), ascending.
pub fn column_dates(
    wb: &Workbook,
    ws: &Worksheet,
    af: &AutoFilter,
    col_id: u32,
) -> Vec<(i32, Vec<u32>)> {
    let col = af.range.first.col + col_id;
    let mut out: std::collections::BTreeMap<i32, std::collections::BTreeSet<u32>> =
        std::collections::BTreeMap::new();
    for r in af.range.first.row + 1..=af.range.last.row {
        let Some(cell) = ws.cell(r, col) else {
            continue;
        };
        let serial = match &cell.value {
            CellValue::Number(n) => {
                let id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
                let code = crate::numfmt::format_code(&wb.styles, id, &Locale::VI_VN);
                if !crate::numfmt::NumberFormat::parse(&code).is_date() {
                    continue;
                }
                *n
            }
            v @ CellValue::Date(_) => match date_of(wb, v) {
                Some(s) => s,
                None => continue,
            },
            _ => continue,
        };
        if let Some(d) = crate::numfmt::serial_to_datetime(serial, wb.date1904, 0) {
            out.entry(d.year).or_default().insert(d.month);
        }
    }
    out.into_iter()
        .map(|(y, m)| (y, m.into_iter().collect()))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{CellRange, CellRef, FilterColumn};

    fn wb() -> Workbook {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        let data: [(&str, f64); 6] = [
            ("Hà Nội", 10.0),
            ("Huế", 25.0),
            ("hà nội", 5.0),
            ("Đà Nẵng", 40.0),
            ("", 15.0),
            ("Cần Thơ", 30.0),
        ];
        ws.set_a1("A1", CellValue::Text("City".into()));
        ws.set_a1("B1", CellValue::Text("Sales".into()));
        for (i, (c, n)) in data.iter().enumerate() {
            let r = i as u32 + 1;
            if !c.is_empty() {
                ws.set_cell(
                    r,
                    0,
                    bo_model::sheet::Cell::new(CellValue::Text((*c).into())),
                );
            }
            ws.set_cell(r, 1, bo_model::sheet::Cell::new(CellValue::Number(*n)));
        }
        wb
    }

    fn af(col_id: u32, filter: ColumnFilter) -> AutoFilter {
        AutoFilter {
            range: CellRange::parse("A1:B7").unwrap(),
            columns: vec![FilterColumn {
                col_id,
                filter,
                ..FilterColumn::default()
            }],
            ..AutoFilter::default()
        }
    }

    fn hidden(f: &AutoFilter) -> Vec<u32> {
        let wb = wb();
        hidden_rows(&wb, &wb.sheets[0], f, &Locale::VI_VN)
    }

    #[test]
    fn wildcards() {
        assert!(wildcard_match("h*", "Hà Nội"));
        assert!(wildcard_match("*nội", "Hà Nội"));
        assert!(wildcard_match("h?ế", "Huế"));
        assert!(!wildcard_match("h?", "Huế"));
        assert!(wildcard_match("a~*b", "a*b"));
        assert!(!wildcard_match("a~*b", "axb"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("*a*b*", "xxaYYb"));
    }

    #[test]
    fn value_lists() {
        let f = af(
            0,
            ColumnFilter::Values {
                values: vec!["HÀ NỘI".into()],
                blank: true,
            },
        );
        // Rows 1 and 3 match case-insensitively, row 5 is blank; others hidden.
        assert_eq!(hidden(&f), vec![2, 4, 6]);
    }

    #[test]
    fn custom_and_top10() {
        let f = af(
            1,
            ColumnFilter::Custom {
                and: true,
                conditions: vec![
                    CustomFilter {
                        operator: Some("greaterThanOrEqual".into()),
                        val: "10".into(),
                    },
                    CustomFilter {
                        operator: Some("lessThan".into()),
                        val: "30".into(),
                    },
                ],
            },
        );
        assert_eq!(hidden(&f), vec![3, 4, 6]);
        let f = af(
            0,
            ColumnFilter::Custom {
                and: false,
                conditions: vec![CustomFilter {
                    operator: None,
                    val: "h*".into(),
                }],
            },
        );
        assert_eq!(hidden(&f), vec![4, 5, 6]);
        let top2 = af(
            1,
            ColumnFilter::Top10 {
                top: true,
                percent: false,
                val: 2.0,
                filter_val: None,
            },
        );
        assert_eq!(hidden(&top2), vec![1, 2, 3, 5]);
        let bottom50 = af(
            1,
            ColumnFilter::Top10 {
                top: false,
                percent: true,
                val: 50.0,
                filter_val: None,
            },
        );
        assert_eq!(hidden(&bottom50), vec![2, 4, 6]);
    }

    #[test]
    fn apply_sets_row_flags_and_lists_values() {
        let mut wb = wb();
        wb.sheets[0].auto_filter = Some(af(
            0,
            ColumnFilter::Values {
                values: vec!["Huế".into()],
                blank: false,
            },
        ));
        assert_eq!(apply_auto_filter(&mut wb, 0, &Locale::VI_VN), 5);
        assert!(wb.sheets[0].rows[&1].hidden);
        assert!(!wb.sheets[0].rows.contains_key(&2));
        // Clearing the criterion shows everything again and drops empty row records.
        wb.sheets[0].auto_filter.as_mut().unwrap().columns.clear();
        assert_eq!(apply_auto_filter(&mut wb, 0, &Locale::VI_VN), 0);
        assert!(wb.sheets[0].rows.is_empty());
        let a = wb.sheets[0].auto_filter.clone().unwrap();
        let vals = column_values(&wb, &wb.sheets[0], &a, 0, &Locale::VI_VN);
        assert_eq!(vals, ["", "Cần Thơ", "Đà Nẵng", "Hà Nội", "Huế"]);
    }

    #[test]
    fn date_groups_dynamic_and_colour() {
        use crate::numfmt::datetime_to_serial;
        use bo_model::sheet::{Cell, CellXf, DateGroup, Dxf, Fill};
        let mut wb = Workbook::new();
        let red = wb.styles.add_fill(Fill::solid("FF0000"));
        let red_xf = wb.styles.add_xf(CellXf {
            fill_id: red,
            ..CellXf::default()
        });
        wb.styles.dxfs.push(Dxf {
            fill: Some(Fill::Pattern(bo_model::sheet::PatternFill {
                pattern: Some("solid".into()),
                fg: None,
                bg: Some(bo_model::sheet::SheetColor::rgb("FF0000")),
            })),
            ..Dxf::default()
        });
        let ws = &mut wb.sheets[0];
        ws.set_a1("A1", CellValue::Text("Ngày".into()));
        let dates = [(2024, 3, 5), (2024, 7, 10), (2025, 1, 2), (2025, 1, 20)];
        for (i, (y, m, d)) in dates.iter().enumerate() {
            let s = datetime_to_serial(*y, *m, *d, 0, 0, 0.0, false).unwrap();
            let style = if i % 2 == 0 { red_xf } else { 0 };
            ws.set_cell(
                i as u32 + 1,
                0,
                Cell::new(CellValue::Number(s)).with_style(style),
            );
        }
        let mk = |filter: ColumnFilter| AutoFilter {
            range: bo_model::sheet::CellRange::parse("A1:A5").unwrap(),
            columns: vec![bo_model::sheet::FilterColumn {
                col_id: 0,
                filter,
                ..Default::default()
            }],
            ..AutoFilter::default()
        };
        let now = datetime_to_serial(2025, 1, 15, 10, 0, 0.0, false);
        let hid = |f: ColumnFilter| hidden_rows_at(&wb, &wb.sheets[0], &mk(f), &Locale::VI_VN, now);
        // Year 2024 or March 2024 (month grouping).
        let groups = vec![
            DateGroup {
                year: 2024,
                month: Some(7),
                grouping: "month".into(),
                ..DateGroup::default()
            },
            DateGroup {
                year: 2025,
                grouping: "year".into(),
                ..DateGroup::default()
            },
        ];
        assert_eq!(
            hid(ColumnFilter::DateGroups {
                values: Vec::new(),
                blank: false,
                groups
            }),
            vec![1]
        );
        let dynamic = |kind: &str| ColumnFilter::Dynamic {
            kind: kind.into(),
            val: None,
            max_val: None,
        };
        assert_eq!(hid(dynamic("thisMonth")), vec![1, 2]);
        assert_eq!(hid(dynamic("lastYear")), vec![3, 4]);
        assert_eq!(hid(dynamic("thisWeek")), vec![1, 2, 3, 4]);
        // Weeks start on Sunday: last week 2025-01-05..11, next week 01-19..25.
        assert_eq!(hid(dynamic("lastWeek")), vec![1, 2, 3, 4]);
        assert_eq!(hid(dynamic("nextWeek")), vec![1, 2, 3]);
        assert_eq!(hid(dynamic("M7")), vec![1, 3, 4]);
        assert_eq!(hid(dynamic("Q1")), vec![2]);
        assert_eq!(hid(dynamic("aboveAverage")), vec![1, 2]);
        // Colour filter: red cells (rows 1 and 3) stay.
        assert_eq!(
            hid(ColumnFilter::Color {
                dxf_id: Some(0),
                cell_color: true
            }),
            vec![2, 4]
        );
    }

    // source: ECMA-376 Part 1, §18.3.2.9 iconFilter
    #[test]
    fn icon_filter() {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.set_a1("A1", CellValue::Text("Điểm".into()));
        for (i, v) in [5.0, 25.0, 12.0, 30.0, 8.0, 15.0].iter().enumerate() {
            ws.set_cell(i as u32 + 1, 0, Cell::new(CellValue::Number(*v)));
        }
        ws.set_a1("A8", CellValue::Text("x".into()));
        crate::sort::tests::arrows(&mut wb, "A2:A8");
        let mk = |icon_id: Option<u32>| AutoFilter {
            range: CellRange::parse("A1:A8").unwrap(),
            columns: vec![FilterColumn {
                col_id: 0,
                filter: ColumnFilter::Icon {
                    icon_set: "3Arrows".into(),
                    icon_id,
                },
                ..FilterColumn::default()
            }],
            ..AutoFilter::default()
        };
        let hid = |f: &AutoFilter| hidden_rows(&wb, &wb.sheets[0], f, &Locale::VI_VN);
        // Icon 0 (below 10): 5 (row 1) and 8 (row 5) stay.
        assert_eq!(hid(&mk(Some(0))), vec![2, 3, 4, 6, 7]);
        // Icon 2 (20 and up): 25 and 30.
        assert_eq!(hid(&mk(Some(2))), vec![1, 3, 5, 6, 7]);
        // No icon: only the text cell.
        assert_eq!(hid(&mk(None)), vec![1, 2, 3, 4, 5, 6]);
    }

    /// The cached number formats and lazily built texts give the rows a cell-by-cell
    /// evaluation with `format_cell` gives: value lists on formatted numbers and on
    /// text, custom conditions on numbers and with wildcards, AND / OR, blanks.
    #[test]
    fn hidden_rows_match_naive_evaluation() {
        let mut wb = Workbook::new();
        // Style 1: two decimals (built-in 2, `0.00`); style 2: percent (built-in 10).
        for id in [2, 10] {
            wb.styles.cell_xfs.push(bo_model::sheet::CellXf {
                num_fmt_id: id,
                ..bo_model::sheet::CellXf::default()
            });
        }
        let words = [
            "Hà Nội",
            "hà nội",
            "Huế",
            "Cần Thơ",
            "Đà Nẵng",
            "x1",
            "X2",
            "",
        ];
        let ws = &mut wb.sheets[0];
        let n = 4000u32;
        let mut s = 11u64;
        let mut rnd = |m: u64| {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            (s >> 33) % m
        };
        for c in 0..3 {
            ws.set_cell(0, c, Cell::new(CellValue::Text(format!("H{c}"))));
        }
        for r in 1..=n {
            let w = words[rnd(8) as usize];
            if !w.is_empty() {
                ws.set_cell(r, 0, Cell::new(CellValue::Text(w.into())));
            }
            let v = rnd(400) as f64 / 4.0;
            ws.set_cell(
                r,
                1,
                Cell::new(CellValue::Number(v)).with_style(rnd(3) as u32),
            );
            if rnd(5) > 0 {
                ws.set_cell(r, 2, Cell::new(CellValue::Number(rnd(100) as f64)));
            } else {
                ws.set_cell(
                    r,
                    2,
                    Cell::new(CellValue::Text(words[rnd(8) as usize].into())),
                );
            }
        }
        let cond = |op: &str, val: &str| CustomFilter {
            operator: Some(op.into()),
            val: val.into(),
        };
        let filters = [
            (
                0,
                ColumnFilter::Values {
                    values: vec!["HÀ NỘI".into(), "Huế".into()],
                    blank: true,
                },
            ),
            (
                1,
                ColumnFilter::Values {
                    values: vec!["12.50".into(), "25".into(), "5000.00%".into()],
                    blank: false,
                },
            ),
            (
                1,
                ColumnFilter::Custom {
                    and: true,
                    conditions: vec![cond("greaterThan", "20"), cond("lessThanOrEqual", "70.5")],
                },
            ),
            (
                2,
                ColumnFilter::Custom {
                    and: false,
                    conditions: vec![cond("lessThan", "10"), cond("equal", "x*")],
                },
            ),
            (
                0,
                ColumnFilter::Custom {
                    and: true,
                    conditions: vec![cond("notEqual", "h*")],
                },
            ),
        ];
        let range = CellRange::new(CellRef::new(0, 0), CellRef::new(n, 2));
        // Single columns, then all of them together.
        let mut cases: Vec<Vec<(u32, ColumnFilter)>> =
            filters.iter().map(|f| vec![f.clone()]).collect();
        cases.push(vec![
            filters[0].clone(),
            filters[2].clone(),
            filters[3].clone(),
        ]);
        for case in cases {
            let af = AutoFilter {
                range,
                columns: case
                    .iter()
                    .map(|(col_id, filter)| FilterColumn {
                        col_id: *col_id,
                        filter: filter.clone(),
                        ..FilterColumn::default()
                    })
                    .collect(),
                ..AutoFilter::default()
            };
            let ws = &wb.sheets[0];
            let naive: Vec<u32> = (1..=n)
                .filter(|&r| {
                    !case.iter().all(|(col, f)| {
                        let cell = ws.cell(r, *col);
                        let value = cell.map_or(CellValue::Empty, |c| c.value.clone());
                        let text = cell
                            .map(|c| format_cell(&wb, c, &Locale::VI_VN).text)
                            .unwrap_or_default();
                        match f {
                            ColumnFilter::Values { values, blank } => {
                                if text.is_empty() {
                                    *blank
                                } else {
                                    values
                                        .iter()
                                        .any(|v| v.to_lowercase() == text.to_lowercase())
                                }
                            }
                            ColumnFilter::Custom { and, conditions } => {
                                let mut it =
                                    conditions.iter().map(|c| custom_match(c, &value, &text));
                                if *and {
                                    it.all(|b| b)
                                } else {
                                    it.any(|b| b)
                                }
                            }
                            _ => true,
                        }
                    })
                })
                .collect();
            let got = hidden_rows_at(&wb, ws, &af, &Locale::VI_VN, Some(45_000.0));
            assert_eq!(got, naive, "{case:?}");
            assert!(!got.is_empty() && got.len() < n as usize, "{case:?}");
        }
    }
}

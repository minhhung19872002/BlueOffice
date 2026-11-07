//! AutoFilter evaluation (ECMA-376 Part 1, §18.3.1.2 `autoFilter`, §18.3.2).
//!
//! A row of the filter range (header excluded) is shown when it satisfies the
//! criterion of every filter column. Value lists compare the cell's *displayed*
//! text case-insensitively (§18.3.2.6 `filter/@val`); custom filters
//! (§18.3.2.2) compare numerically when both sides are numbers and as text with
//! `*` / `?` wildcards otherwise; `top10` (§18.3.2.10) keeps the N largest /
//! smallest numbers (or N percent). Criteria kept as raw XML are not evaluated
//! (rows stay visible).

use crate::numfmt::Locale;
use crate::render::format_cell;
use bo_model::sheet::{
    AutoFilter, CellValue, ColumnFilter, CustomFilter, RowInfo, Workbook, Worksheet,
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

/// Zero-based rows (absolute) that the AutoFilter `af` hides.
pub fn hidden_rows(wb: &Workbook, ws: &Worksheet, af: &AutoFilter, locale: &Locale) -> Vec<u32> {
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
            _ => None,
        })
        .collect();
    let mut hidden = Vec::new();
    for r in first..=last {
        let visible = af.columns.iter().zip(&thresholds).all(|(fc, th)| {
            let col = af.range.first.col + fc.col_id;
            let cell = ws.cell(r, col);
            let value = cell.map_or(CellValue::Empty, |c| c.value.clone());
            let text = cell
                .map(|c| format_cell(wb, c, locale).text)
                .unwrap_or_default();
            let blank = text.is_empty();
            match &fc.filter {
                ColumnFilter::None | ColumnFilter::Raw(_) => true,
                ColumnFilter::Values {
                    values,
                    blank: show_blank,
                } => {
                    if blank {
                        *show_blank
                    } else {
                        values
                            .iter()
                            .any(|v| v.to_lowercase() == text.to_lowercase())
                    }
                }
                ColumnFilter::Custom { and, conditions } => {
                    let mut it = conditions.iter().map(|c| custom_match(c, &value, &text));
                    if *and {
                        it.all(|b| b)
                    } else {
                        conditions.is_empty() || it.any(|b| b)
                    }
                }
                ColumnFilter::Top10 { .. } => match (th, &value) {
                    (Some((true, cut)), CellValue::Number(n)) => n >= cut,
                    (Some((false, cut)), CellValue::Number(n)) => n <= cut,
                    _ => false,
                },
            }
        });
        if !visible {
            hidden.push(r);
        }
    }
    hidden
}

/// Apply the sheet's AutoFilter: rows inside the range are hidden or shown.
/// Returns the number of hidden rows.
pub fn apply_auto_filter(wb: &mut Workbook, sheet: usize, locale: &Locale) -> usize {
    let Some(ws) = wb.sheets.get(sheet) else {
        return 0;
    };
    let Some(af) = ws.auto_filter.clone() else {
        return 0;
    };
    let hidden = hidden_rows(wb, ws, &af, locale);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{CellRange, FilterColumn};

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
}

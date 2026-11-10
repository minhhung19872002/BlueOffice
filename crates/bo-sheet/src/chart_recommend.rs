//! Recommended charts (CORE-DESIGN-001): rules on the shape of the selected data rank the
//! chart types of [`crate::chart::CHART_TYPES`] that suit it.
//!
//! The data are read like [`crate::chart::new_chart`] reads them: a header row when the first
//! row holds text, leading label columns (text or dates), then one value series per numeric
//! column. The rules:
//! - several label columns (a hierarchy) → treemap, sunburst;
//! - dates or years as labels (a time series) → line, then column / area;
//! - numbers only, two or more columns → scatter (first column = X);
//! - one series of positive parts in a few categories → pie / doughnut;
//! - many categories or long labels → horizontal bars;
//! - otherwise a few categories → columns (stacked columns for several series).
//!
//! The rules and their wording are BlueOffice's own.

use crate::numfmt::{format_code, Locale, NumberFormat};
use bo_model::sheet::{CellRange, CellValue, Workbook, Worksheet};
use serde::Serialize;

/// What the labels of the data are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LabelKind {
    /// No label column.
    #[default]
    None,
    /// Text labels (categories).
    Text,
    /// Dates or years (a time axis).
    Time,
}

/// The shape of a data range.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DataShape {
    /// The first row holds series names.
    pub header: bool,
    /// Data rows (categories / points).
    pub rows: u32,
    /// Leading label columns.
    pub label_cols: u32,
    /// What the (innermost) labels are.
    pub labels: LabelKind,
    /// Numeric value columns after the labels.
    pub series: u32,
    /// Every value is ≥ 0 and at least one is > 0.
    pub positive: bool,
    /// Longest label, in characters.
    pub label_len: usize,
}

/// One recommended chart type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// Chart type name of [`crate::chart::CHART_TYPES`].
    pub kind: &'static str,
    /// Rank, 0–100 (higher first).
    pub score: u32,
    /// Why (a key the UI words): `trend`, `compare`, `parts`, `relationship`,
    /// `manyCategories`, `hierarchy`, `composition`, `default`.
    pub reason: &'static str,
}

fn is_text(v: &CellValue) -> bool {
    matches!(v, CellValue::Text(_) | CellValue::RichText(_))
}

/// A date: an ISO date value or a number shown with a date format.
fn is_date(wb: &Workbook, ws: &Worksheet, r: u32, c: u32) -> bool {
    let Some(cell) = ws.cell(r, c) else {
        return false;
    };
    match cell.value {
        CellValue::Date(_) => true,
        CellValue::Number(_) => {
            let id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
            NumberFormat::parse(&format_code(&wb.styles, id, &Locale::default())).is_date()
        }
        _ => false,
    }
}

/// A label cell of a chart's category column: text or a date (empty cells allowed).
pub fn is_label(wb: &Workbook, ws: &Worksheet, r: u32, c: u32) -> bool {
    ws.cell(r, c)
        .is_none_or(|cell| is_text(&cell.value) || matches!(cell.value, CellValue::Empty))
        || is_date(wb, ws, r, c)
}

fn text_of(v: &CellValue) -> String {
    v.as_text().unwrap_or_default()
}

/// A four-digit year written as text ("2025").
fn is_year_text(s: &str) -> bool {
    let s = s.trim();
    s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) && ("1900".."2101").contains(&s)
}

/// The shape of `range` on sheet `sheet`; `None` without numbers to plot.
pub fn shape_of(wb: &Workbook, sheet: usize, range: CellRange) -> Option<DataShape> {
    let ws = wb.sheets.get(sheet)?;
    let text_at = |r: u32, c: u32| ws.cell(r, c).is_some_and(|c| is_text(&c.value));
    let header =
        range.rows() > 1 && (range.first.col..=range.last.col).any(|c| text_at(range.first.row, c));
    let r0 = range.first.row + u32::from(header);
    if r0 > range.last.row {
        return None;
    }
    let label_col = |c: u32| (r0..=range.last.row).all(|r| is_label(wb, ws, r, c));
    let mut label_cols = 0;
    while label_cols + 1 < range.cols() && label_col(range.first.col + label_cols) {
        label_cols += 1;
    }
    let c0 = range.first.col + label_cols;
    let mut series = 0;
    let mut positive = true;
    let mut any_positive = false;
    for c in c0..=range.last.col {
        let mut numbers = 0;
        for r in r0..=range.last.row {
            if let Some(CellValue::Number(n)) = ws.cell(r, c).map(|x| &x.value) {
                numbers += 1;
                positive &= *n >= 0.0;
                any_positive |= *n > 0.0;
            }
        }
        if numbers > 0 {
            series += 1;
        }
    }
    if series == 0 {
        return None;
    }
    let labels = if label_cols == 0 {
        LabelKind::None
    } else {
        let lc = c0 - 1;
        let rows: Vec<u32> = (r0..=range.last.row).collect();
        let time = rows.iter().all(|&r| {
            ws.cell(r, lc).is_none_or(|cell| {
                matches!(cell.value, CellValue::Empty)
                    || is_date(wb, ws, r, lc)
                    || is_year_text(&text_of(&cell.value))
            })
        });
        if time {
            LabelKind::Time
        } else {
            LabelKind::Text
        }
    };
    let label_len = (r0..=range.last.row)
        .filter_map(|r| ws.cell(r, c0.saturating_sub(1)).filter(|_| label_cols > 0))
        .map(|cell| text_of(&cell.value).chars().count())
        .max()
        .unwrap_or(0);
    Some(DataShape {
        header,
        rows: range.last.row - r0 + 1,
        label_cols,
        labels,
        series,
        positive: positive && any_positive,
        label_len,
    })
}

/// The chart types for data of shape `s`, best first.
pub fn rules(s: &DataShape) -> Vec<Recommendation> {
    let mut v: Vec<Recommendation> = Vec::new();
    let mut add = |kind: &'static str, score: u32, reason: &'static str| {
        v.push(Recommendation {
            kind,
            score,
            reason,
        });
    };
    if s.label_cols >= 2 && s.positive {
        add("treemap", 95, "hierarchy");
        add("sunburst", 90, "hierarchy");
    }
    if s.labels == LabelKind::Time && s.rows >= 2 {
        add("line", 92, "trend");
        add("column", 74, "compare");
        add(
            if s.series > 1 { "stackedArea" } else { "area" },
            60,
            "trend",
        );
    }
    if s.labels == LabelKind::None && s.series >= 2 {
        add("scatter", 90, "relationship");
        add("line", 58, "trend");
    }
    if s.series == 1 && s.positive && (2..=6).contains(&s.rows) && s.labels != LabelKind::Time {
        add("pie", 88, "parts");
        add("doughnut", 72, "parts");
    }
    if s.rows > 10 || s.label_len > 15 {
        add("bar", 88, "manyCategories");
        add("column", 62, "compare");
    } else {
        add("column", 86, "compare");
        add("bar", 70, "compare");
    }
    if s.series >= 2 {
        add("stackedColumn", 66, "composition");
        if s.positive {
            add("percentBar", 52, "composition");
        }
    }
    if s.rows >= 4 && s.labels == LabelKind::Text {
        add("line", 50, "trend");
    }
    add("column", 40, "default");
    // Best score of each kind, best first; ties keep the rule order.
    let mut out: Vec<Recommendation> = Vec::new();
    for r in v {
        match out.iter_mut().find(|o| o.kind == r.kind) {
            Some(o) if o.score >= r.score => {}
            Some(o) => *o = r,
            None => out.push(r),
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.score));
    out
}

/// Recommended chart types for `range` of `sheet`, best first, at most `limit`.
pub fn recommend(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    limit: usize,
) -> Vec<Recommendation> {
    shape_of(wb, sheet, range).map_or_else(Vec::new, |s| {
        let mut v = rules(&s);
        v.truncate(limit.max(1));
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellRef};

    fn shape(
        labels: LabelKind,
        label_cols: u32,
        rows: u32,
        series: u32,
        positive: bool,
    ) -> DataShape {
        DataShape {
            header: true,
            rows,
            label_cols,
            labels,
            series,
            positive,
            label_len: 6,
        }
    }

    fn first(s: &DataShape) -> &'static str {
        rules(s)[0].kind
    }

    #[test]
    fn rules_pick_the_chart_for_the_data() {
        assert_eq!(first(&shape(LabelKind::Time, 1, 12, 1, true)), "line");
        assert_eq!(first(&shape(LabelKind::None, 0, 20, 2, false)), "scatter");
        assert_eq!(first(&shape(LabelKind::Text, 1, 4, 1, true)), "pie");
        assert_eq!(first(&shape(LabelKind::Text, 1, 4, 1, false)), "column");
        assert_eq!(first(&shape(LabelKind::Text, 1, 25, 1, true)), "bar");
        assert_eq!(first(&shape(LabelKind::Text, 1, 5, 3, true)), "column");
        assert_eq!(first(&shape(LabelKind::Text, 2, 8, 1, true)), "treemap");
        let long = DataShape {
            label_len: 30,
            ..shape(LabelKind::Text, 1, 5, 2, true)
        };
        assert_eq!(first(&long), "bar");
    }

    #[test]
    fn rules_rank_without_duplicates() {
        for s in [
            shape(LabelKind::Time, 1, 12, 2, true),
            shape(LabelKind::Text, 1, 4, 1, true),
            shape(LabelKind::None, 0, 3, 3, false),
        ] {
            let r = rules(&s);
            let mut kinds: Vec<&str> = r.iter().map(|x| x.kind).collect();
            kinds.sort_unstable();
            kinds.dedup();
            assert_eq!(kinds.len(), r.len());
            assert!(r.windows(2).all(|w| w[0].score >= w[1].score));
            assert!(r
                .iter()
                .all(|x| crate::chart::CHART_TYPES.contains(&x.kind)));
        }
    }

    fn book(rows: &[&[CellValue]]) -> Workbook {
        let mut wb = Workbook::default();
        let mut ws = Worksheet::default();
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                ws.cells.insert(
                    (r as u32, c as u32),
                    Cell {
                        value: v.clone(),
                        ..Cell::default()
                    },
                );
            }
        }
        wb.sheets.push(ws);
        wb
    }

    fn t(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    fn n(v: f64) -> CellValue {
        CellValue::Number(v)
    }

    fn all(rows: usize, cols: u32) -> CellRange {
        CellRange::new(CellRef::new(0, 0), CellRef::new(rows as u32 - 1, cols - 1))
    }

    #[test]
    fn shape_of_reads_header_labels_and_series() {
        let wb = book(&[
            &[t("Tỉnh"), t("2024"), t("2025")],
            &[t("Hà Nội"), n(10.0), n(12.0)],
            &[t("Huế"), n(5.0), n(-4.0)],
        ]);
        let s = shape_of(&wb, 0, all(3, 3)).unwrap();
        assert!(s.header);
        assert_eq!((s.rows, s.label_cols, s.series), (2, 1, 2));
        assert_eq!(s.labels, LabelKind::Text);
        assert!(!s.positive);
        assert_eq!(s.label_len, 6);
    }

    #[test]
    fn years_and_hierarchies_are_recognised() {
        let years = book(&[
            &[t("Năm"), t("Doanh thu")],
            &[t("2023"), n(1.0)],
            &[t("2024"), n(2.0)],
            &[t("2025"), n(3.0)],
        ]);
        assert_eq!(recommend(&years, 0, all(4, 2), 3)[0].kind, "line");
        let tree = book(&[
            &[t("Miền"), t("Tỉnh"), t("Dân số")],
            &[t("Bắc"), t("Hà Nội"), n(8.0)],
            &[t("Bắc"), t("Hải Phòng"), n(2.0)],
            &[t("Nam"), t("Cần Thơ"), n(1.2)],
        ]);
        let r = recommend(&tree, 0, all(4, 3), 6);
        assert_eq!(r[0].kind, "treemap");
        assert_eq!(r[1].kind, "sunburst");
        let xy = book(&[
            &[t("X"), t("Y")],
            &[n(1.0), n(3.0)],
            &[n(2.0), n(1.0)],
            &[n(5.0), n(4.0)],
        ]);
        assert_eq!(recommend(&xy, 0, all(4, 2), 6)[0].kind, "scatter");
        // Text only: nothing to plot.
        let words = book(&[&[t("a"), t("b")], &[t("c"), t("d")]]);
        assert_eq!(recommend(&words, 0, all(2, 2), 6), Vec::new());
    }
}

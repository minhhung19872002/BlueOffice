//! Pivot table tests. Expected reports are worked out by hand from the source
//! data below, following the definitions of ECMA-376 Part 1, §18.10.1.73
//! (layout, subtotals, grand totals), §18.10.1.21 / §18.18.70 (`showDataAs`),
//! §18.10.1.27 / §18.10.1.77 (grouping), §18.10.1.35 (filters) and
//! §18.10.1.3 (calculated fields).

use super::*;
use bo_model::sheet::{PivotFilterType, PivotSort, PIVOT_BASE_NEXT, PIVOT_BASE_PREVIOUS};

// ------------------------------------------------------------- the small table

fn sales() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.name = "Data".into();
    let rows: [(&str, &str, f64); 6] = [
        ("Bắc", "A", 10.0),
        ("Nam", "B", 5.0),
        ("Bắc", "B", 7.0),
        ("Trung", "A", 2.0),
        ("Nam", "A", 1.0),
        ("bắc", "A", 3.0),
    ];
    for (c, h) in ["Vùng", "Loại", "Doanh thu"].iter().enumerate() {
        ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for (i, (a, b, v)) in rows.iter().enumerate() {
        let r = i as u32 + 1;
        ws.set_cell(r, 0, Cell::new(CellValue::Text((*a).into())));
        ws.set_cell(r, 1, Cell::new(CellValue::Text((*b).into())));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(*v)));
    }
    wb.sheets.push(Worksheet::new("Pivot"));
    wb
}

fn src() -> CellRange {
    CellRange::parse("A1:C7").unwrap()
}

fn text(ws: &Worksheet, a1: &str) -> String {
    ws.cell_a1(a1)
        .map(|c| value_text(&c.value))
        .unwrap_or_default()
}

fn sum_of(field: u32, function: PivotFunction) -> PivotDataField {
    PivotDataField {
        field,
        function,
        ..PivotDataField::default()
    }
}

#[test]
fn rows_and_values() {
    let mut wb = sales();
    let spec = PivotSpec {
        rows: vec![0],
        values: vec![sum_of(2, PivotFunction::Sum)],
        ..PivotSpec::default()
    };
    let i = create_pivot(
        &mut wb,
        0,
        src(),
        1,
        CellRef::new(2, 0),
        &spec,
        &PivotLabels::default(),
    )
    .unwrap();
    let ws = &wb.sheets[1];
    let pt = &ws.pivots[i];
    assert_eq!(pt.location.to_string(), "A3:B7");
    assert_eq!(
        (pt.first_header_row, pt.first_data_row, pt.first_data_col),
        (0, 1, 1)
    );
    assert_eq!(text(ws, "A3"), "Vùng");
    assert_eq!(text(ws, "B3"), "Sum of Doanh thu");
    // Case-insensitive grouping: "Bắc" and "bắc" are one item (first seen kept).
    assert_eq!(
        (text(ws, "A4"), text(ws, "B4")),
        ("Bắc".into(), "20".into())
    );
    assert_eq!((text(ws, "A5"), text(ws, "B5")), ("Nam".into(), "6".into()));
    assert_eq!(
        (text(ws, "A6"), text(ws, "B6")),
        ("Trung".into(), "2".into())
    );
    assert_eq!(
        (text(ws, "A7"), text(ws, "B7")),
        ("Grand Total".into(), "28".into())
    );
    let cache = &wb.pivot_caches[0];
    assert_eq!(cache.source_ref.as_deref(), Some("A1:C7"));
    assert_eq!(cache.fields[0].items.len(), 3);
    assert_eq!(cache.records.len(), 6);
    assert_eq!(
        pt.style.as_ref().map(|s| s.name.as_str()),
        Some("PivotStyleLight16")
    );
    assert_eq!(pt.row_items.len(), 4, "3 items + grand total");
    assert_eq!(pt.row_items[3].kind, "grand");
}

#[test]
fn columns_filters_and_functions() {
    let mut wb = sales();
    let spec = PivotSpec {
        rows: vec![0],
        cols: vec![1],
        values: vec![sum_of(2, PivotFunction::Average)],
        filters: vec![PivotFilter {
            field: 1,
            selected: vec![CellValue::Text("A".into())],
        }],
        ..PivotSpec::default()
    };
    create_pivot(
        &mut wb,
        0,
        src(),
        1,
        CellRef::new(0, 0),
        &spec,
        &PivotLabels::default(),
    )
    .unwrap();
    let ws = &wb.sheets[1];
    let pt = &ws.pivots[0];
    // Filter row, blank row, then the body.
    assert_eq!(
        (text(ws, "A1"), text(ws, "B1")),
        ("Loại".into(), "A".into())
    );
    assert_eq!(pt.location.first, CellRef::new(2, 0));
    assert_eq!(text(ws, "A3"), "Average of Doanh thu");
    assert_eq!(text(ws, "B3"), "Loại");
    assert_eq!(
        (text(ws, "B4"), text(ws, "C4")),
        ("A".into(), "Grand Total".into())
    );
    assert_eq!(text(ws, "A4"), "Vùng");
    assert_eq!(pt.location.to_string(), "A3:C8");
    // Bắc: A = (10 + 3) / 2.
    assert_eq!(
        (text(ws, "A5"), text(ws, "B5")),
        ("Bắc".into(), "6.5".into())
    );
    assert_eq!(text(ws, "A8"), "Grand Total");
    assert_eq!(text(ws, "B8"), "4");
    // Change to Count with no filter: refresh in place.
    let spec = PivotSpec {
        rows: vec![0],
        cols: vec![1],
        values: vec![sum_of(2, PivotFunction::Count)],
        ..PivotSpec::default()
    };
    update_pivot(&mut wb, 1, 0, &spec, &PivotLabels::default()).unwrap();
    let ws = &wb.sheets[1];
    assert_eq!(
        ws.pivots[0].location.first,
        CellRef::new(2, 0),
        "body stays"
    );
    assert!(ws.cell_a1("A1").is_none(), "filter area cleared");
    assert_eq!(text(ws, "A3"), "Count of Doanh thu");
    assert_eq!(
        (text(ws, "C4"), text(ws, "D4")),
        ("B".into(), "Grand Total".into())
    );
    assert_eq!(text(ws, "C7"), "", "Trung has no B");
    assert_eq!(text(ws, "D8"), "6");
}

#[test]
fn refresh_follows_source_changes() {
    let mut wb = sales();
    let spec = PivotSpec {
        rows: vec![1],
        values: vec![sum_of(2, PivotFunction::Max)],
        ..PivotSpec::default()
    };
    create_pivot(
        &mut wb,
        0,
        src(),
        1,
        CellRef::new(0, 0),
        &spec,
        &PivotLabels::default(),
    )
    .unwrap();
    wb.sheets[0].set_cell(1, 2, Cell::new(CellValue::Number(100.0)));
    refresh_pivot(&mut wb, 1, 0, &PivotLabels::default()).unwrap();
    let ws = &wb.sheets[1];
    assert_eq!((text(ws, "A2"), text(ws, "B2")), ("A".into(), "100".into()));
    assert_eq!(pivot_at(ws, 1, 1), Some(0));
    assert_eq!(pivot_at(ws, 10, 10), None);
}

#[test]
fn several_values_without_rows() {
    let mut wb = sales();
    let spec = PivotSpec {
        values: vec![
            sum_of(2, PivotFunction::Sum),
            sum_of(0, PivotFunction::Count),
        ],
        ..PivotSpec::default()
    };
    create_pivot(
        &mut wb,
        0,
        src(),
        1,
        CellRef::new(0, 0),
        &spec,
        &PivotLabels::default(),
    )
    .unwrap();
    let ws = &wb.sheets[1];
    assert_eq!(ws.pivots[0].location.to_string(), "A1:B2");
    assert_eq!((text(ws, "A2"), text(ws, "B2")), ("28".into(), "6".into()));
}

// ------------------------------------------------------------- the reference table

/// Eight sales records (Region, Product, Date, Amount, Qty). By hand:
///
/// | Region | Chè | Gạo | Total |   Qty |
/// |--------|-----|-----|-------|-------|
/// | Bắc    |  60 | 170 |   230 |  9    |
/// | Nam    |  30 | 100 |   130 |  9    |
/// | Trung  |  40 |   – |    40 |  2    |
/// | Total  | 130 | 270 |   400 | 20    |
///
/// By month: 2023-11 10, 2023-12 20, 2024-01 180, 2024-02 90, 2024-03 100.
type Record = (&'static str, &'static str, (i64, u32, u32), f64, f64);

const RECORDS: [Record; 8] = [
    ("Bắc", "Gạo", (2024, 1, 15), 100.0, 2.0),
    ("Bắc", "Chè", (2024, 2, 10), 50.0, 1.0),
    ("Nam", "Gạo", (2024, 1, 20), 80.0, 4.0),
    ("Nam", "Chè", (2024, 3, 5), 30.0, 3.0),
    ("Bắc", "Gạo", (2024, 3, 25), 70.0, 1.0),
    ("Nam", "Gạo", (2023, 12, 31), 20.0, 2.0),
    ("Trung", "Chè", (2024, 2, 28), 40.0, 2.0),
    ("Bắc", "Chè", (2023, 11, 11), 10.0, 5.0),
];

const REGION: u32 = 0;
const PRODUCT: u32 = 1;
const DATE: u32 = 2;
const AMOUNT: u32 = 3;
const QTY: u32 = 4;

fn serial(d: (i64, u32, u32)) -> f64 {
    bo_formula::date::ymd_to_serial(d.0, d.1, d.2, DateSystem::Date1900).unwrap()
}

/// The reference workbook: data on "Data" (A1:E9, dates formatted), an empty "Report".
fn book() -> Workbook {
    let mut wb = Workbook::new();
    wb.sheets[0].name = "Data".into();
    let date_xf = wb.styles.add_xf(CellXf {
        num_fmt_id: 14,
        xf_id: Some(0),
        apply_number_format: Some(true),
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Region", "Product", "Date", "Amount", "Qty"]
        .iter()
        .enumerate()
    {
        ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for (i, (r, p, d, a, q)) in RECORDS.iter().enumerate() {
        let row = i as u32 + 1;
        ws.set_cell(row, 0, Cell::new(CellValue::Text((*r).into())));
        ws.set_cell(row, 1, Cell::new(CellValue::Text((*p).into())));
        ws.set_cell(
            row,
            2,
            Cell::new(CellValue::Number(serial(*d))).with_style(date_xf),
        );
        ws.set_cell(row, 3, Cell::new(CellValue::Number(*a)));
        ws.set_cell(row, 4, Cell::new(CellValue::Number(*q)));
    }
    wb.sheets.push(Worksheet::new("Report"));
    wb
}

fn source() -> CellRange {
    CellRange::parse("A1:E9").unwrap()
}

fn amount() -> PivotDataField {
    PivotDataField {
        field: AMOUNT,
        name: Some("Sum".into()),
        ..PivotDataField::default()
    }
}

fn opts(f: impl FnOnce(&mut PivotFieldOptions)) -> PivotFieldOptions {
    let mut o = PivotFieldOptions::default();
    f(&mut o);
    o
}

/// Field options for `Region` only.
fn region_opts(o: PivotFieldOptions) -> Vec<PivotFieldOptions> {
    vec![o]
}

/// Text of a report value: numbers with at most 4 decimals.
fn show(v: &CellValue) -> String {
    match v {
        CellValue::Number(n) => {
            let s = format!("{n:.4}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s == "-0" {
                "0".into()
            } else {
                s.to_owned()
            }
        }
        other => value_text(other),
    }
}

/// The report body (page-field rows excluded) as `|`-joined rows, trailing
/// empty cells trimmed.
fn body(lay: &Layout) -> Vec<String> {
    let mut g = vec![vec![String::new(); lay.width as usize]; lay.height as usize];
    for (r, c, v) in &lay.cells {
        if *r >= lay.page_rows {
            g[(*r - lay.page_rows) as usize][*c as usize] = show(v);
        }
    }
    g.into_iter()
        .map(|row| {
            let mut s = row.join("|");
            while s.ends_with('|') {
                s.pop();
            }
            s
        })
        .collect()
}

/// Build the cache of the reference data and compute `spec`.
fn report(spec: &PivotSpec) -> Vec<String> {
    let wb = book();
    let (names, records) = source_data(&wb.sheets[0], source(), &PivotLabels::default()).unwrap();
    let cache = PivotCache {
        fields: cache_fields(&names, &records),
        records,
        ..PivotCache::default()
    };
    body(&layout(&cache, spec, &PivotLabels::default()).unwrap())
}

/// Create the pivot on "Report"!A1 and return the workbook.
fn created(spec: &PivotSpec) -> Workbook {
    let mut wb = book();
    create_pivot(
        &mut wb,
        0,
        source(),
        1,
        CellRef::new(0, 0),
        spec,
        &PivotLabels::default(),
    )
    .unwrap();
    wb
}

fn rows2() -> PivotSpec {
    PivotSpec {
        rows: vec![REGION, PRODUCT],
        values: vec![amount()],
        ..PivotSpec::default()
    }
}

// ------------------------------------------------------------- 1. subtotals, grand totals

#[test]
fn subtotals_and_grand_totals_table() {
    struct Case {
        what: &'static str,
        spec: PivotSpec,
        expect: &'static [&'static str],
    }
    let cases = [
        Case {
            what: "tabular, subtotals at the bottom (tabular never shows them on top)",
            spec: rows2(),
            expect: &[
                "Region|Product|Sum",
                "Bắc|Chè|60",
                "|Gạo|170",
                "Bắc Total||230",
                "Nam|Chè|30",
                "|Gạo|100",
                "Nam Total||130",
                "Trung|Chè|40",
                "Trung Total||40",
                "Grand Total||400",
            ],
        },
        Case {
            what: "no Region subtotal, no grand total row",
            spec: PivotSpec {
                fields: region_opts(opts(|o| o.subtotal = false)),
                col_grand_totals: false,
                ..rows2()
            },
            expect: &[
                "Region|Product|Sum",
                "Bắc|Chè|60",
                "|Gạo|170",
                "Nam|Chè|30",
                "|Gạo|100",
                "Trung|Chè|40",
            ],
        },
        Case {
            what: "column subtotals and the grand total column",
            spec: PivotSpec {
                cols: vec![REGION, PRODUCT],
                values: vec![amount()],
                ..PivotSpec::default()
            },
            expect: &[
                "Sum|Region",
                "|Bắc||Bắc Total|Nam||Nam Total|Trung|Trung Total|Grand Total",
                "|Chè|Gạo||Chè|Gạo||Chè",
                "Sum|60|170|230|30|100|130|40|40|400",
            ],
        },
        Case {
            what: "rows × columns without the grand total column (rowGrandTotals=0)",
            spec: PivotSpec {
                rows: vec![REGION],
                cols: vec![PRODUCT],
                values: vec![amount()],
                row_grand_totals: false,
                ..PivotSpec::default()
            },
            expect: &[
                "Sum|Product",
                "Region|Chè|Gạo",
                "Bắc|60|170",
                "Nam|30|100",
                "Trung|40",
                "Grand Total|130|270",
            ],
        },
    ];
    for c in &cases {
        assert_eq!(report(&c.spec), c.expect, "{}", c.what);
    }
}

#[test]
fn row_and_column_items() {
    let wb = created(&rows2());
    let pt = &wb.sheets[1].pivots[0];
    let kinds: Vec<&str> = pt.row_items.iter().map(|i| i.kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["", "", "default", "", "", "default", "", "default", "grand"]
    );
    let items: Vec<Vec<u32>> = pt.row_items.iter().map(|i| i.items.clone()).collect();
    assert_eq!(
        items,
        [
            vec![0, 0],
            vec![0, 1],
            vec![0],
            vec![1, 0],
            vec![1, 1],
            vec![1],
            vec![2, 0],
            vec![2],
            vec![0]
        ]
    );
    assert_eq!(pt.col_items.len(), 1);
    assert_eq!(pt.location.to_string(), "A1:C10");
}

// ------------------------------------------------------------- 2. layouts

#[test]
fn layouts_table() {
    let cases: [(&str, PivotSpec, &[&str]); 5] = [
        (
            "outline, subtotals on the group line",
            PivotSpec {
                layout: PivotLayout::Outline,
                ..rows2()
            },
            &[
                "Region|Product|Sum",
                "Bắc||230",
                "|Chè|60",
                "|Gạo|170",
                "Nam||130",
                "|Chè|30",
                "|Gạo|100",
                "Trung||40",
                "|Chè|40",
                "Grand Total||400",
            ],
        ),
        (
            "outline, subtotals at the bottom",
            PivotSpec {
                layout: PivotLayout::Outline,
                fields: region_opts(opts(|o| o.subtotal_top = false)),
                ..rows2()
            },
            &[
                "Region|Product|Sum",
                "Bắc",
                "|Chè|60",
                "|Gạo|170",
                "Bắc Total||230",
                "Nam",
                "|Chè|30",
                "|Gạo|100",
                "Nam Total||130",
                "Trung",
                "|Chè|40",
                "Trung Total||40",
                "Grand Total||400",
            ],
        ),
        (
            "compact: one label column",
            PivotSpec {
                layout: PivotLayout::Compact,
                ..rows2()
            },
            &[
                "Row Labels|Sum",
                "Bắc|230",
                "Chè|60",
                "Gạo|170",
                "Nam|130",
                "Chè|30",
                "Gạo|100",
                "Trung|40",
                "Chè|40",
                "Grand Total|400",
            ],
        ),
        (
            "tabular, repeated labels and a blank row after each region",
            PivotSpec {
                fields: region_opts(opts(|o| {
                    o.repeat_labels = true;
                    o.blank_row = true;
                })),
                ..rows2()
            },
            &[
                "Region|Product|Sum",
                "Bắc|Chè|60",
                "Bắc|Gạo|170",
                "Bắc Total||230",
                "",
                "Nam|Chè|30",
                "Nam|Gạo|100",
                "Nam Total||130",
                "",
                "Trung|Chè|40",
                "Trung Total||40",
                "",
                "Grand Total||400",
            ],
        ),
        (
            "compact with columns: row / column label captions",
            PivotSpec {
                layout: PivotLayout::Compact,
                rows: vec![REGION],
                cols: vec![PRODUCT],
                values: vec![amount()],
                ..PivotSpec::default()
            },
            &[
                "Sum|Column Labels",
                "Row Labels|Chè|Gạo|Grand Total",
                "Bắc|60|170|230",
                "Nam|30|100|130",
                "Trung|40||40",
                "Grand Total|130|270|400",
            ],
        ),
    ];
    for (what, spec, expect) in &cases {
        assert_eq!(report(spec), *expect, "{what}");
    }
}

#[test]
fn compact_labels_are_indented() {
    let wb = created(&PivotSpec {
        layout: PivotLayout::Compact,
        ..rows2()
    });
    let ws = &wb.sheets[1];
    let indent = |a1: &str| {
        let c = ws.cell_a1(a1).unwrap();
        wb.styles
            .xf(c.style)
            .and_then(|x| x.alignment.as_ref())
            .and_then(|a| a.indent)
            .unwrap_or(0)
    };
    assert_eq!(text(ws, "A3"), "Chè");
    assert_eq!((indent("A2"), indent("A3")), (0, 1));
}

// ------------------------------------------------------------- 3. grouping

#[test]
fn date_grouping_years_and_months() {
    let mut wb = created(&PivotSpec {
        rows: vec![DATE],
        values: vec![amount()],
        ..PivotSpec::default()
    });
    let labels = PivotLabels::default();
    group_field(
        &mut wb,
        1,
        0,
        DATE,
        &GroupRequest::Dates {
            years: true,
            quarters: false,
            months: true,
            days: false,
        },
        &labels,
    )
    .unwrap();
    let cache = &wb.pivot_caches[0];
    assert_eq!(cache.fields.len(), 6, "a Years field is added");
    let years = &cache.fields[5];
    assert_eq!((years.name.as_str(), years.derived), ("Years", true));
    let g = cache.fields[DATE as usize].group.as_ref().unwrap();
    assert_eq!(g.par, Some(5));
    assert_eq!(g.range.as_ref().unwrap().group_by, PivotGroupBy::Months);
    // 2023-11-11 … 2024-03-25.
    assert_eq!(g.range.as_ref().unwrap().start, serial((2023, 11, 11)));
    let pt = &wb.sheets[1].pivots[0];
    assert_eq!(pt.rows, vec![5, DATE]);
    let spec = PivotSpec::of(pt);
    let lay = layout(cache, &spec, &labels).unwrap();
    assert_eq!(
        body(&lay),
        [
            "Years|Date|Sum",
            "2023|Nov|10",
            "|Dec|20",
            "2023 Total||30",
            "2024|Jan|180",
            "|Feb|90",
            "|Mar|100",
            "2024 Total||370",
            "Grand Total||400",
        ]
    );
    // Months alone merge the years; ungrouping restores the dates.
    group_field(
        &mut wb,
        1,
        0,
        DATE,
        &GroupRequest::Dates {
            years: false,
            quarters: false,
            months: true,
            days: false,
        },
        &labels,
    )
    .unwrap();
    assert_eq!(wb.pivot_caches[0].fields.len(), 5);
    let ws = &wb.sheets[1];
    let col: Vec<String> = (2..=7).map(|r| text(ws, &format!("A{r}"))).collect();
    assert_eq!(col, ["Jan", "Feb", "Mar", "Nov", "Dec", "Grand Total"]);
    assert_eq!(text(ws, "B2"), "180");
    group_field(&mut wb, 1, 0, DATE, &GroupRequest::None, &labels).unwrap();
    let ws = &wb.sheets[1];
    assert_eq!(text(ws, "A2"), serial((2023, 11, 11)).to_string());
    // Item labels keep the date format of the source column.
    let style = ws.cell_a1("A2").unwrap().style;
    assert_eq!(wb.styles.xf(style).map(|x| x.num_fmt_id), Some(14));
}

#[test]
fn quarter_grouping_and_numeric_ranges() {
    let labels = PivotLabels::default();
    let mut wb = created(&PivotSpec {
        rows: vec![DATE],
        values: vec![amount()],
        ..PivotSpec::default()
    });
    group_field(
        &mut wb,
        1,
        0,
        DATE,
        &GroupRequest::Dates {
            years: false,
            quarters: true,
            months: false,
            days: false,
        },
        &labels,
    )
    .unwrap();
    // Q1 = Jan–Mar 2024 (370), Q4 = Nov–Dec 2023 (30).
    let ws = &wb.sheets[1];
    assert_eq!(
        (text(ws, "A2"), text(ws, "B2")),
        ("Qtr1".into(), "370".into())
    );
    assert_eq!(
        (text(ws, "A3"), text(ws, "B3")),
        ("Qtr4".into(), "30".into())
    );
    // Amounts in ranges of 50: 0-49 → 10, 20, 30, 40; 50-99 → 50, 70, 80; 100-149 → 100.
    let mut wb = created(&PivotSpec {
        rows: vec![AMOUNT],
        values: vec![PivotDataField {
            field: QTY,
            function: PivotFunction::Count,
            name: Some("N".into()),
            ..PivotDataField::default()
        }],
        ..PivotSpec::default()
    });
    group_field(
        &mut wb,
        1,
        0,
        AMOUNT,
        &GroupRequest::Numbers {
            start: Some(0.0),
            end: Some(100.0),
            interval: 50.0,
        },
        &labels,
    )
    .unwrap();
    let ws = &wb.sheets[1];
    let rows: Vec<(String, String)> = (2..=5)
        .map(|r| (text(ws, &format!("A{r}")), text(ws, &format!("B{r}"))))
        .collect();
    assert_eq!(
        rows,
        [
            ("0-49".into(), "4".into()),
            ("50-99".into(), "3".into()),
            ("100-149".into(), "1".into()),
            ("Grand Total".into(), "8".into())
        ]
    );
    // A refresh keeps the grouping.
    wb.sheets[0].set_cell(1, 3, Cell::new(CellValue::Number(5.0)));
    refresh_pivot(&mut wb, 1, 0, &labels).unwrap();
    let ws = &wb.sheets[1];
    assert_eq!(
        (text(ws, "A2"), text(ws, "B2")),
        ("0-49".into(), "5".into())
    );
    // Text cannot be grouped by range.
    assert!(group_field(
        &mut wb,
        1,
        0,
        REGION,
        &GroupRequest::Numbers {
            start: None,
            end: None,
            interval: 1.0
        },
        &labels
    )
    .is_err());
}

// ------------------------------------------------------------- 4. calculated fields

#[test]
fn calculated_field_uses_sums() {
    let labels = PivotLabels::default();
    let mut wb = created(&PivotSpec {
        rows: vec![REGION],
        values: vec![amount()],
        ..PivotSpec::default()
    });
    let f = set_calculated_field(&mut wb, 1, 0, None, "Price", "=Amount/Qty", &labels).unwrap();
    assert_eq!(f, 5);
    let cache = &wb.pivot_caches[0];
    assert_eq!(cache.fields[5].formula.as_deref(), Some("Amount/Qty"));
    // Bắc 230/9, Nam 130/9, Trung 40/2, total 400/20 (sums, not record averages).
    let lay = layout(cache, &PivotSpec::of(&wb.sheets[1].pivots[0]), &labels).unwrap();
    assert_eq!(
        body(&lay),
        [
            "Region|Sum|Sum of Price",
            "Bắc|230|25.5556",
            "Nam|130|14.4444",
            "Trung|40|20",
            "Grand Total|400|20",
        ]
    );
    // Rename + new formula; quoted names; then remove.
    set_calculated_field(
        &mut wb,
        1,
        0,
        Some(5),
        "Bonus",
        "'Amount'*0.1+IF(Qty>5,1,0)",
        &labels,
    )
    .unwrap();
    let ws = &wb.sheets[1];
    assert_eq!(text(ws, "C1"), "Sum of Bonus");
    assert_eq!(text(ws, "C2"), "24");
    assert!(set_calculated_field(&mut wb, 1, 0, None, "Bad", "Price*2", &labels).is_err());
    assert!(set_calculated_field(&mut wb, 1, 0, None, "Region", "Qty", &labels).is_err());
    remove_calculated_field(&mut wb, 1, 0, 5, &labels).unwrap();
    assert_eq!(wb.pivot_caches[0].fields.len(), 5);
    assert_eq!(wb.sheets[1].pivots[0].values.len(), 1);
    assert_eq!(text(&wb.sheets[1], "C1"), "");
}

// ------------------------------------------------------------- 5. show values as

#[test]
fn show_values_as_table() {
    let matrix = |show_as: PivotShowAs, base_field: u32, base_item: u32| PivotSpec {
        rows: vec![REGION],
        cols: vec![PRODUCT],
        values: vec![PivotDataField {
            show_as,
            base_field,
            base_item,
            ..amount()
        }],
        ..PivotSpec::default()
    };
    let by_region = |show_as: PivotShowAs, base_item: u32| PivotSpec {
        rows: vec![REGION],
        values: vec![PivotDataField {
            show_as,
            base_field: REGION,
            base_item,
            ..amount()
        }],
        ..PivotSpec::default()
    };
    let nested = |show_as: PivotShowAs| PivotSpec {
        values: vec![PivotDataField {
            show_as,
            base_field: REGION,
            ..amount()
        }],
        ..rows2()
    };
    let cases: Vec<(&str, PivotSpec, Vec<&str>)> = vec![
        (
            "% of grand total: 60/400, 170/400, 230/400 …",
            matrix(PivotShowAs::PercentOfTotal, 0, 0),
            vec![
                "Sum|Product",
                "Region|Chè|Gạo|Grand Total",
                "Bắc|0.15|0.425|0.575",
                "Nam|0.075|0.25|0.325",
                "Trung|0.1||0.1",
                "Grand Total|0.325|0.675|1",
            ],
        ),
        (
            "% of row total: 60/230, 170/230 …",
            matrix(PivotShowAs::PercentOfRow, 0, 0),
            vec![
                "Sum|Product",
                "Region|Chè|Gạo|Grand Total",
                "Bắc|0.2609|0.7391|1",
                "Nam|0.2308|0.7692|1",
                "Trung|1||1",
                "Grand Total|0.325|0.675|1",
            ],
        ),
        (
            "% of column total: 60/130, 170/270 …",
            matrix(PivotShowAs::PercentOfCol, 0, 0),
            vec![
                "Sum|Product",
                "Region|Chè|Gạo|Grand Total",
                "Bắc|0.4615|0.6296|0.575",
                "Nam|0.2308|0.3704|0.325",
                "Trung|0.3077||0.1",
                "Grand Total|1|1|1",
            ],
        ),
        (
            "index: v·400/(row·col), e.g. 60·400/(230·130) = 0.8027",
            matrix(PivotShowAs::Index, 0, 0),
            vec![
                "Sum|Product",
                "Region|Chè|Gạo|Grand Total",
                "Bắc|0.8027|1.095|1",
                "Nam|0.7101|1.1396|1",
                "Trung|3.0769||1",
                "Grand Total|1|1|1",
            ],
        ),
        (
            "difference from Bắc: 130 − 230, 40 − 230",
            by_region(PivotShowAs::Difference, 0),
            vec!["Region|Sum", "Bắc", "Nam|-100", "Trung|-190", "Grand Total"],
        ),
        (
            "difference from the previous item",
            by_region(PivotShowAs::Difference, PIVOT_BASE_PREVIOUS),
            vec!["Region|Sum", "Bắc", "Nam|-100", "Trung|-90", "Grand Total"],
        ),
        (
            "% of the next item: 230/130, 130/40",
            by_region(PivotShowAs::Percent, PIVOT_BASE_NEXT),
            vec![
                "Region|Sum",
                "Bắc|1.7692",
                "Nam|3.25",
                "Trung",
                "Grand Total",
            ],
        ),
        (
            "% difference from the previous item: (130 − 230)/230",
            by_region(PivotShowAs::PercentDiff, PIVOT_BASE_PREVIOUS),
            vec![
                "Region|Sum",
                "Bắc",
                "Nam|-0.4348",
                "Trung|-0.6923",
                "Grand Total",
            ],
        ),
        (
            "running total in Region",
            by_region(PivotShowAs::RunTotal, 0),
            vec![
                "Region|Sum",
                "Bắc|230",
                "Nam|360",
                "Trung|400",
                "Grand Total",
            ],
        ),
        (
            "% running total in Region",
            by_region(PivotShowAs::PercentOfRunningTotal, 0),
            vec![
                "Region|Sum",
                "Bắc|0.575",
                "Nam|0.9",
                "Trung|1",
                "Grand Total",
            ],
        ),
        (
            "rank largest to smallest",
            by_region(PivotShowAs::RankDescending, 0),
            vec!["Region|Sum", "Bắc|1", "Nam|2", "Trung|3", "Grand Total"],
        ),
        (
            "rank smallest to largest",
            by_region(PivotShowAs::RankAscending, 0),
            vec!["Region|Sum", "Bắc|3", "Nam|2", "Trung|1", "Grand Total"],
        ),
        (
            "% of parent row: 60/230, 230/400",
            nested(PivotShowAs::PercentOfParentRow),
            vec![
                "Region|Product|Sum",
                "Bắc|Chè|0.2609",
                "|Gạo|0.7391",
                "Bắc Total||0.575",
                "Nam|Chè|0.2308",
                "|Gạo|0.7692",
                "Nam Total||0.325",
                "Trung|Chè|1",
                "Trung Total||0.1",
                "Grand Total||1",
            ],
        ),
        (
            "% of parent (base Region): the region line is 100 %, the total blank",
            nested(PivotShowAs::PercentOfParent),
            vec![
                "Region|Product|Sum",
                "Bắc|Chè|0.2609",
                "|Gạo|0.7391",
                "Bắc Total||1",
                "Nam|Chè|0.2308",
                "|Gạo|0.7692",
                "Nam Total||1",
                "Trung|Chè|1",
                "Trung Total||1",
                "Grand Total",
            ],
        ),
    ];
    for (what, spec, expect) in &cases {
        assert_eq!(report(spec), *expect, "{what}");
    }
}

#[test]
fn percentages_get_a_percent_format() {
    let wb = created(&PivotSpec {
        rows: vec![REGION],
        values: vec![PivotDataField {
            show_as: PivotShowAs::PercentOfTotal,
            ..amount()
        }],
        ..PivotSpec::default()
    });
    let ws = &wb.sheets[1];
    let fmt = |a1: &str| {
        wb.styles
            .xf(ws.cell_a1(a1).unwrap().style)
            .map(|x| x.num_fmt_id)
    };
    assert_eq!(fmt("B2"), Some(10));
    // An explicit format wins.
    let mut wb = created(&PivotSpec {
        rows: vec![REGION],
        values: vec![PivotDataField {
            num_fmt_id: Some(4),
            ..amount()
        }],
        ..PivotSpec::default()
    });
    assert_eq!(
        wb.styles
            .xf(wb.sheets[1].cell_a1("B2").unwrap().style)
            .map(|x| x.num_fmt_id),
        Some(4)
    );
    wb.sheets[1].pivots[0].values[0].num_fmt_id = None;
    assert_eq!(
        value_format(&wb.pivot_caches[0], &wb.sheets[1].pivots[0].values[0]),
        0
    );
}

// ------------------------------------------------------------- 6. sorting and filters

#[test]
fn sorting_table() {
    let base = || PivotSpec {
        rows: vec![REGION],
        values: vec![amount()],
        ..PivotSpec::default()
    };
    let cases: [(&str, PivotFieldOptions, &[&str]); 4] = [
        (
            "descending by label",
            opts(|o| o.sort = PivotSort::Descending),
            &[
                "Region|Sum",
                "Trung|40",
                "Nam|130",
                "Bắc|230",
                "Grand Total|400",
            ],
        ),
        (
            "ascending by label",
            opts(|o| o.sort = PivotSort::Ascending),
            &[
                "Region|Sum",
                "Bắc|230",
                "Nam|130",
                "Trung|40",
                "Grand Total|400",
            ],
        ),
        (
            "ascending by Sum",
            opts(|o| {
                o.sort = PivotSort::Ascending;
                o.sort_by = Some(0);
            }),
            &[
                "Region|Sum",
                "Trung|40",
                "Nam|130",
                "Bắc|230",
                "Grand Total|400",
            ],
        ),
        (
            "descending by Sum",
            opts(|o| {
                o.sort = PivotSort::Descending;
                o.sort_by = Some(0);
            }),
            &[
                "Region|Sum",
                "Bắc|230",
                "Nam|130",
                "Trung|40",
                "Grand Total|400",
            ],
        ),
    ];
    for (what, o, expect) in &cases {
        let spec = PivotSpec {
            fields: region_opts(o.clone()),
            ..base()
        };
        assert_eq!(report(&spec), *expect, "{what}");
    }
    // Inner items sort within their parent: products by Sum, descending.
    let spec = PivotSpec {
        fields: vec![
            PivotFieldOptions::default(),
            opts(|o| {
                o.sort = PivotSort::Descending;
                o.sort_by = Some(0);
            }),
        ],
        ..rows2()
    };
    assert_eq!(&report(&spec)[1..3], ["Bắc|Gạo|170", "|Chè|60"]);
}

#[test]
fn filters_table() {
    let filter =
        |kind: PivotFilterType, v1: &str, v2: &str, count: f64, top: bool| PivotFieldFilter {
            field: REGION,
            kind,
            measure: 0,
            value1: v1.into(),
            value2: v2.into(),
            top,
            count,
        };
    let base = |f: PivotFieldFilter| PivotSpec {
        rows: vec![REGION],
        values: vec![amount()],
        field_filters: vec![f],
        ..PivotSpec::default()
    };
    let cases: [(&str, PivotFieldFilter, &[&str]); 9] = [
        (
            "top 2 by Sum (grand total of the visible items)",
            filter(PivotFilterType::Count, "", "", 2.0, true),
            &["Region|Sum", "Bắc|230", "Nam|130", "Grand Total|360"],
        ),
        (
            "bottom 1",
            filter(PivotFilterType::Count, "", "", 1.0, false),
            &["Region|Sum", "Trung|40", "Grand Total|40"],
        ),
        (
            "top 50 %: Bắc alone reaches 200 of 400",
            filter(PivotFilterType::Percent, "", "", 50.0, true),
            &["Region|Sum", "Bắc|230", "Grand Total|230"],
        ),
        (
            "top items summing to 300",
            filter(PivotFilterType::Sum, "", "", 300.0, true),
            &["Region|Sum", "Bắc|230", "Nam|130", "Grand Total|360"],
        ),
        (
            "label begins with \"n\" (case-insensitive)",
            filter(PivotFilterType::CaptionBeginsWith, "n", "", 0.0, true),
            &["Region|Sum", "Nam|130", "Grand Total|130"],
        ),
        (
            "label does not equal Trung",
            filter(PivotFilterType::CaptionNotEqual, "trung", "", 0.0, true),
            &["Region|Sum", "Bắc|230", "Nam|130", "Grand Total|360"],
        ),
        (
            "label contains \"a\"",
            filter(PivotFilterType::CaptionContains, "a", "", 0.0, true),
            &["Region|Sum", "Nam|130", "Grand Total|130"],
        ),
        (
            "value greater than 100",
            filter(PivotFilterType::ValueGreaterThan, "100", "", 0.0, true),
            &["Region|Sum", "Bắc|230", "Nam|130", "Grand Total|360"],
        ),
        (
            "value between 30 and 150",
            filter(PivotFilterType::ValueBetween, "30", "150", 0.0, true),
            &["Region|Sum", "Nam|130", "Trung|40", "Grand Total|170"],
        ),
    ];
    for (what, f, expect) in &cases {
        assert_eq!(report(&base(f.clone())), *expect, "{what}");
    }
    // Top 1 product per region: Gạo 170, Gạo 100, Chè 40; subtotals follow.
    let spec = PivotSpec {
        field_filters: vec![PivotFieldFilter {
            field: PRODUCT,
            ..filter(PivotFilterType::Count, "", "", 1.0, true)
        }],
        ..rows2()
    };
    assert_eq!(
        report(&spec),
        [
            "Region|Product|Sum",
            "Bắc|Gạo|170",
            "Bắc Total||170",
            "Nam|Gạo|100",
            "Nam Total||100",
            "Trung|Chè|40",
            "Trung Total||40",
            "Grand Total||310",
        ]
    );
    // Hidden items (item/@h) are left out of totals too.
    let spec = PivotSpec {
        rows: vec![REGION],
        values: vec![amount()],
        fields: region_opts(opts(|o| o.hidden = vec![CellValue::Text("nam".into())])),
        ..PivotSpec::default()
    };
    assert_eq!(
        report(&spec),
        ["Region|Sum", "Bắc|230", "Trung|40", "Grand Total|270"]
    );
}

// ------------------------------------------------------------- 7. delete, move, source, refresh all, style

#[test]
fn delete_move_and_change_source() {
    let labels = PivotLabels::default();
    let mut wb = created(&PivotSpec {
        rows: vec![REGION],
        values: vec![amount()],
        ..PivotSpec::default()
    });
    // Move to Report!D5.
    let i = move_pivot(&mut wb, 1, 0, 1, CellRef::new(4, 3), &labels).unwrap();
    let ws = &wb.sheets[1];
    assert!(ws.cell_a1("A1").is_none());
    assert_eq!(text(ws, "D5"), "Region");
    assert_eq!(text(ws, "E6"), "230");
    assert_eq!(ws.pivots[i].location.to_string(), "D5:E9");
    // Onto its own source: refused.
    assert!(move_pivot(&mut wb, 1, i, 0, CellRef::new(1, 1), &labels).is_err());
    // To another sheet.
    wb.sheets.push(Worksheet::new("Other"));
    let j = move_pivot(&mut wb, 1, i, 2, CellRef::new(0, 0), &labels).unwrap();
    assert_eq!(wb.sheets[1].pivots.len(), 0);
    assert!(wb.sheets[1].cell_a1("D5").is_none());
    assert_eq!(text(&wb.sheets[2], "B2"), "230");
    // Change the source to the first five records only: Bắc 220, Nam 110.
    change_source(
        &mut wb,
        2,
        j,
        0,
        CellRange::parse("A1:E6").unwrap(),
        &labels,
    )
    .unwrap();
    let ws = &wb.sheets[2];
    assert_eq!(wb.pivot_caches[0].source_ref.as_deref(), Some("A1:E6"));
    assert_eq!(
        (text(ws, "B2"), text(ws, "B3"), text(ws, "A4")),
        ("220".into(), "110".into(), "Grand Total".into())
    );
    assert!(ws.cell_a1("A5").is_none(), "the old Trung row is cleared");
    // Refresh all after an edit.
    wb.sheets[0].set_cell(1, 3, Cell::new(CellValue::Number(0.0)));
    assert_eq!(refresh_all(&mut wb, &labels).unwrap(), 1);
    assert_eq!(text(&wb.sheets[2], "B2"), "120");
    // Delete: cells and cache go.
    delete_pivot(&mut wb, 2, j).unwrap();
    assert_eq!(wb.sheets[2].pivots.len(), 0);
    assert!(wb.sheets[2].cells.is_empty());
    assert_eq!(wb.pivot_caches.len(), 0);
}

#[test]
fn pivot_style_formats_headers_and_totals() {
    let mut wb = created(&rows2());
    let fill = |wb: &Workbook, a1: &str| {
        let ws = &wb.sheets[1];
        let xf = wb.styles.xf(ws.cell_a1(a1).unwrap().style).unwrap();
        wb.styles.fills.get(xf.fill_id as usize).cloned()
    };
    let bold = |wb: &Workbook, a1: &str| {
        let ws = &wb.sheets[1];
        let xf = wb.styles.xf(ws.cell_a1(a1).unwrap().style).unwrap();
        wb.styles
            .fonts
            .get(xf.font_id as usize)
            .is_some_and(|f| f.bold)
    };
    // Header row filled, subtotal and grand total rows bold, data plain.
    assert_ne!(fill(&wb, "A1"), fill(&wb, "B2"));
    assert!(bold(&wb, "A4"), "Bắc Total");
    assert!(bold(&wb, "C10"), "grand total");
    assert!(!bold(&wb, "C2"));
    // A medium style fills the header with the accent and white text.
    let mut spec = PivotSpec::of(&wb.sheets[1].pivots[0]);
    spec.style = Some(PivotStyleInfo {
        name: "PivotStyleMedium9".into(),
        show_row_stripes: true,
        ..default_style()
    });
    update_pivot(&mut wb, 1, 0, &spec, &PivotLabels::default()).unwrap();
    let ws = &wb.sheets[1];
    let xf = wb.styles.xf(ws.cell_a1("A1").unwrap().style).unwrap();
    let font = &wb.styles.fonts[xf.font_id as usize];
    assert_eq!(
        font.color.as_ref().and_then(|c| c.rgb.clone()).as_deref(),
        Some("FFFFFFFF")
    );
    // Banded rows: the second data line (A3) differs from the first (A2).
    assert_ne!(fill(&wb, "B2"), fill(&wb, "B3"));
}

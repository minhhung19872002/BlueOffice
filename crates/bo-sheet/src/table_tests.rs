//! Tests of tables: expected values are worked out by hand in the comments.

use super::*;
use crate::calc::FormulaEngine;
use crate::formula::BoFormula;
use bo_model::sheet::{CellValue, RowInfo, Worksheet};

fn recalc(wb: &mut Workbook) {
    BoFormula::new().recalculate(wb);
}

fn num(wb: &Workbook, a1: &str) -> f64 {
    let r = CellRef::parse(a1).unwrap();
    match wb.sheets[0].cell(r.row, r.col).map(|c| &c.value) {
        Some(CellValue::Number(n)) => *n,
        other => panic!("{a1}: {other:?}"),
    }
}

fn formula(wb: &Workbook, a1: &str) -> String {
    let r = CellRef::parse(a1).unwrap();
    wb.sheets[0]
        .cell(r.row, r.col)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.clone())
        .unwrap_or_default()
}

fn set_formula(wb: &mut Workbook, a1: &str, f: &str) {
    let r = CellRef::parse(a1).unwrap();
    wb.sheets[0]
        .cells
        .insert((r.row, r.col), Cell::default().with_formula(f));
}

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn r(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}

/// Sheet `S`: A1:B6 = Tên / Số with Số = 4, 8, "x", 15, 1 (rows 2–6); table `Bảng1` over it.
fn sample() -> Workbook {
    let mut ws = Worksheet::new("S");
    ws.set_a1("A1", text("Tên"));
    ws.set_a1("B1", text("Số"));
    for (i, (n, v)) in [
        ("a", CellValue::Number(4.0)),
        ("b", CellValue::Number(8.0)),
        ("c", text("x")),
        ("d", CellValue::Number(15.0)),
        ("e", CellValue::Number(1.0)),
    ]
    .into_iter()
    .enumerate()
    {
        ws.set_a1(&format!("A{}", i + 2), text(n));
        ws.set_a1(&format!("B{}", i + 2), v);
    }
    let mut wb = Workbook::new();
    wb.sheets = vec![ws];
    create_table(&mut wb, 0, r("A1:B6"), &Locale::VI_VN).unwrap();
    wb
}

#[test]
fn creates_a_table_with_unique_names() {
    let mut ws = Worksheet::new("S");
    ws.set_a1("A1", text("Tên"));
    ws.set_a1("B1", text("Tên"));
    ws.set_a1("A2", CellValue::Number(1.0));
    let mut wb = Workbook::new();
    wb.sheets = vec![ws];
    let i = create_table(&mut wb, 0, r("A1:C3"), &Locale::VI_VN).unwrap();
    let t = &wb.sheets[0].tables[i];
    assert_eq!(t.name, "Bảng1");
    let cols: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cols, ["Tên", "Tên2", "Cột 3"]);
    // The empty header cell shows its generated name.
    assert_eq!(
        wb.sheets[0].cell(0, 2).map(|c| &c.value),
        Some(&text("Cột 3"))
    );
    assert!(create_table(&mut wb, 0, r("B2:D4"), &Locale::VI_VN).is_err());
    let j = create_table(&mut wb, 0, r("F1:G2"), &Locale::VI_VN).unwrap();
    assert_eq!(wb.sheets[0].tables[j].id, 2);
    assert_eq!(wb.sheets[0].tables[j].name, "Bảng2");
}

#[test]
fn totals_row_functions_skip_hidden_rows() {
    let mut wb = sample();
    // Row 5 (B5 = 15) hidden: SUBTOTAL 10x leaves it out.
    wb.sheets[0].rows.insert(
        4,
        RowInfo {
            hidden: true,
            ..RowInfo::default()
        },
    );
    set_totals_row(&mut wb, 0, 0, true, "Tổng").unwrap();
    let t = &wb.sheets[0].tables[0];
    assert_eq!(t.range, r("A1:B7"));
    assert_eq!(t.totals_row_count, Some(1));
    // First time: label in the first column, a sum in the (numeric) last column.
    assert_eq!(
        wb.sheets[0].cell(6, 0).map(|c| &c.value),
        Some(&text("Tổng"))
    );
    assert_eq!(formula(&wb, "B7"), "SUBTOTAL(109,Bảng1[Số])");
    // The filter stops above the totals row.
    assert_eq!(
        t.auto_filter.as_deref(),
        Some("<autoFilter ref=\"A1:B6\"/>")
    );
    // Visible numbers 4, 8, 1 (B4 is text "x"): sum 13, mean 13/3,
    // sample variance ((16+64+1) − 13²/3) / 2 = 37/3.
    let cases: [(&str, f64); 7] = [
        ("sum", 13.0),
        ("average", 13.0 / 3.0),
        ("countNums", 3.0),
        ("count", 4.0),
        ("max", 8.0),
        ("min", 1.0),
        ("var", 37.0 / 3.0),
    ];
    for (f, want) in cases {
        set_total_function(&mut wb, 0, 0, 1, f, None).unwrap();
        recalc(&mut wb);
        assert!(
            (num(&wb, "B7") - want).abs() < 1e-9,
            "{f}: {}",
            num(&wb, "B7")
        );
    }
    set_total_function(&mut wb, 0, 0, 1, "stdDev", None).unwrap();
    assert_eq!(formula(&wb, "B7"), "SUBTOTAL(107,Bảng1[Số])");
    recalc(&mut wb);
    assert!((num(&wb, "B7") - (37.0f64 / 3.0).sqrt()).abs() < 1e-9);
    // A custom formula is kept in totalsRowFormula; SUM counts the hidden row:
    // (4 + 8 + 15 + 1) × 2 = 56.
    set_total_function(&mut wb, 0, 0, 1, "custom", Some("=SUM(Bảng1[Số])*2")).unwrap();
    recalc(&mut wb);
    assert_eq!(num(&wb, "B7"), 56.0);
    let c = &wb.sheets[0].tables[0].columns[1];
    assert_eq!(c.totals_row_function.as_deref(), Some("custom"));
    assert_eq!(c.totals_row_formula.as_deref(), Some("SUM(Bảng1[Số])*2"));
    // None clears the cell.
    set_total_function(&mut wb, 0, 0, 1, "none", None).unwrap();
    assert!(wb.sheets[0].cell(6, 1).is_none());
    assert!(set_total_function(&mut wb, 0, 0, 1, "median", None).is_err());
    // Hide the totals row again: its cells go, the range shrinks.
    set_totals_row(&mut wb, 0, 0, false, "Tổng").unwrap();
    assert_eq!(wb.sheets[0].tables[0].range, r("A1:B6"));
    assert!(wb.sheets[0].cell(6, 0).is_none());
}

#[test]
fn totals_row_refused_when_the_row_below_is_used() {
    let mut wb = sample();
    wb.sheets[0].set_a1("B7", CellValue::Number(1.0));
    assert!(set_totals_row(&mut wb, 0, 0, true, "Tổng").is_err());
    assert_eq!(wb.sheets[0].tables[0].range, r("A1:B6"));
}

#[test]
fn structured_references_evaluate() {
    let mut wb = sample();
    set_totals_row(&mut wb, 0, 0, true, "Tổng").unwrap();
    // C column outside the table: references to it.
    set_formula(&mut wb, "D2", "SUM(Bảng1[Số])"); // 4+8+15+1 = 28
    set_formula(&mut wb, "D3", "Bảng1[[#Totals],[Số]]"); // the totals cell = 28
    set_formula(&mut wb, "D4", "ROWS(Bảng1[#All])"); // header + 5 + totals = 7
    set_formula(&mut wb, "D5", "COUNTA(Bảng1[#Headers])"); // 2
    set_formula(&mut wb, "D6", "ROWS(Bảng1)"); // data rows = 5
    set_formula(&mut wb, "D7", "COLUMNS(Bảng1[[Tên]:[Số]])"); // 2
    set_formula(&mut wb, "D8", "Bảng1[[#Totals],[Tên]]"); // the label
    recalc(&mut wb);
    assert_eq!(num(&wb, "D2"), 28.0);
    assert_eq!(num(&wb, "D3"), 28.0);
    assert_eq!(num(&wb, "D4"), 7.0);
    assert_eq!(num(&wb, "D5"), 2.0);
    assert_eq!(num(&wb, "D6"), 5.0);
    assert_eq!(num(&wb, "D7"), 2.0);
    assert_eq!(
        wb.sheets[0].cell(7, 3).map(|c| &c.value),
        Some(&text("Tổng"))
    );
}

#[test]
fn calculated_column_fills_and_keeps_exceptions() {
    let mut wb = sample();
    // Extend to a third column C "Đôi"; C3 typed with [@Số]*2.
    wb.sheets[0].set_a1("C1", text("Đôi"));
    resize_table(&mut wb, 0, 0, r("A1:C6"), &Locale::VI_VN).unwrap();
    assert_eq!(wb.sheets[0].tables[0].columns[2].name, "Đôi");
    set_formula(&mut wb, "C3", "[@Số]*2");
    let (ti, k, base) = calculated_for(&wb, 0, (2, 2)).unwrap();
    assert_eq!((ti, k), (0, 2));
    set_calculated(&mut wb, 0, ti, k, &base);
    assert_eq!(base, "Bảng1[[#This Row],[Số]]*2");
    assert_eq!(
        wb.sheets[0].tables[0].columns[2]
            .calculated_formula
            .as_deref(),
        Some("Bảng1[[#This Row],[Số]]*2")
    );
    recalc(&mut wb);
    // 4×2, 8×2, "x"×2 = #VALUE!, 15×2, 1×2.
    assert_eq!(num(&wb, "C2"), 8.0);
    assert_eq!(num(&wb, "C5"), 30.0);
    assert_eq!(num(&wb, "C6"), 2.0);
    // An exception (a typed value) stays; a new formula elsewhere does not refill the column.
    wb.sheets[0].set_a1("C4", CellValue::Number(0.0));
    wb.sheets[0].cells.get_mut(&(3, 2)).unwrap().formula = None;
    set_formula(&mut wb, "C5", "[@Số]*3");
    assert!(calculated_for(&wb, 0, (4, 2)).is_none());
    // A relative A1 formula moves with the rows: D column of a fresh table.
    let mut wb = sample();
    wb.sheets[0].set_a1("C1", text("Ba"));
    resize_table(&mut wb, 0, 0, r("A1:C6"), &Locale::VI_VN).unwrap();
    set_formula(&mut wb, "C2", "B2*3");
    let (ti, k, base) = calculated_for(&wb, 0, (1, 2)).unwrap();
    set_calculated(&mut wb, 0, ti, k, &base);
    assert_eq!(formula(&wb, "C6"), "B6*3");
    recalc(&mut wb);
    assert_eq!(num(&wb, "C6"), 3.0);
    // A formula typed in a column holding values is not spread.
    set_formula(&mut wb, "B2", "1+1");
    assert!(calculated_for(&wb, 0, (1, 1)).is_none());
}

#[test]
fn calculated_column_follows_a_new_formula_without_exceptions() {
    let mut wb = sample();
    wb.sheets[0].set_a1("C1", text("K"));
    resize_table(&mut wb, 0, 0, r("A1:C6"), &Locale::VI_VN).unwrap();
    set_formula(&mut wb, "C2", "B2+1");
    let (ti, k, base) = calculated_for(&wb, 0, (1, 2)).unwrap();
    set_calculated(&mut wb, 0, ti, k, &base);
    // Typing B4*10 in C4: no exceptions, so the column takes the new formula.
    set_formula(&mut wb, "C4", "B4*10");
    let (_, _, base) = calculated_for(&wb, 0, (3, 2)).unwrap();
    assert_eq!(base, "B2*10");
    set_calculated(&mut wb, 0, 0, 2, &base);
    assert_eq!(formula(&wb, "C6"), "B6*10");
}

#[test]
fn typing_next_to_a_table_expands_it() {
    let mut wb = sample();
    wb.sheets[0].set_a1("C1", text("K"));
    resize_table(&mut wb, 0, 0, r("A1:C6"), &Locale::VI_VN).unwrap();
    set_formula(&mut wb, "C2", "[@Số]+1");
    let (ti, k, base) = calculated_for(&wb, 0, (1, 2)).unwrap();
    set_calculated(&mut wb, 0, ti, k, &base);
    // A7 typed below: one more row; the calculated column fills C7.
    wb.sheets[0].set_a1("A7", text("f"));
    let (ti, range) = expansion_for(&wb, 0, &[(6, 0)]).unwrap();
    assert_eq!(range, r("A1:C7"));
    resize_table(&mut wb, 0, ti, range, &Locale::VI_VN).unwrap();
    assert_eq!(formula(&wb, "C7"), "Bảng1[[#This Row],[Số]]+1");
    // D3 typed to the right: one more column, named after its (empty) header → "Cột 4".
    wb.sheets[0].set_a1("D3", CellValue::Number(5.0));
    let (_, range) = expansion_for(&wb, 0, &[(2, 3)]).unwrap();
    assert_eq!(range, r("A1:D7"));
    resize_table(&mut wb, 0, 0, range, &Locale::VI_VN).unwrap();
    assert_eq!(wb.sheets[0].tables[0].columns[3].name, "Cột 4");
    assert_eq!(
        wb.sheets[0].cell(0, 3).map(|c| &c.value),
        Some(&text("Cột 4"))
    );
    assert_eq!(
        wb.sheets[0].tables[0].auto_filter.as_deref(),
        Some("<autoFilter ref=\"A1:D7\"/>")
    );
    // Not adjacent / empty: no expansion.
    wb.sheets[0].set_a1("A9", text("far"));
    assert!(expansion_for(&wb, 0, &[(8, 0)]).is_none());
    assert!(expansion_for(&wb, 0, &[(7, 1)]).is_none());
    // Pasting two rows below at once adds both.
    wb.sheets[0].set_a1("A8", text("g"));
    let (_, range) = expansion_for(&wb, 0, &[(7, 0), (8, 0)]).unwrap();
    assert_eq!(range, r("A1:D9"));
    // With a totals row, rows below do not join.
    let mut wb = sample();
    set_totals_row(&mut wb, 0, 0, true, "Tổng").unwrap();
    wb.sheets[0].set_a1("A8", text("z"));
    assert!(expansion_for(&wb, 0, &[(7, 0)]).is_none());
}

#[test]
fn resize_rules() {
    let mut wb = sample();
    // The header row cannot move; the range must keep a data row and overlap the table.
    assert!(resize_table(&mut wb, 0, 0, r("A2:B6"), &Locale::VI_VN).is_err());
    assert!(resize_table(&mut wb, 0, 0, r("A1:B1"), &Locale::VI_VN).is_err());
    assert!(resize_table(&mut wb, 0, 0, r("D1:E3"), &Locale::VI_VN).is_err());
    // Shrinking drops the column (its cells stay).
    resize_table(&mut wb, 0, 0, r("A1:A4"), &Locale::VI_VN).unwrap();
    let t = &wb.sheets[0].tables[0];
    assert_eq!(t.columns.len(), 1);
    assert_eq!(t.range, r("A1:A4"));
    assert_eq!(num(&wb, "B2"), 4.0);
    // With a totals row: it moves to the new last row.
    let mut wb = sample();
    set_totals_row(&mut wb, 0, 0, true, "Tổng").unwrap();
    resize_table(&mut wb, 0, 0, r("A1:B9"), &Locale::VI_VN).unwrap();
    assert!(wb.sheets[0].cell(6, 1).is_none());
    assert_eq!(formula(&wb, "B9"), "SUBTOTAL(109,Bảng1[Số])");
}

#[test]
fn header_edits_rename_columns_and_formulas() {
    let mut wb = sample();
    set_formula(&mut wb, "E1", "SUM(Bảng1[Số])");
    wb.sheets[0].set_a1("B1", text("Số tiền"));
    assert!(sync_headers(&mut wb, 0, &[(0, 1)], &Locale::VI_VN));
    assert_eq!(wb.sheets[0].tables[0].columns[1].name, "Số tiền");
    assert_eq!(formula(&wb, "E1"), "SUM(Bảng1[Số tiền])");
    recalc(&mut wb);
    // 4 + 8 + 15 + 1.
    assert_eq!(num(&wb, "E1"), 28.0);
    // A repeated name gets a suffix written back into the cell.
    wb.sheets[0].set_a1("B1", text("tên"));
    sync_headers(&mut wb, 0, &[(0, 1)], &Locale::VI_VN);
    assert_eq!(wb.sheets[0].tables[0].columns[1].name, "tên2");
    assert_eq!(
        wb.sheets[0].cell(0, 1).map(|c| &c.value),
        Some(&text("tên2"))
    );
    // Rename the table: references follow.
    rename_table(&mut wb, 0, 0, "DoanhThu").unwrap();
    assert_eq!(formula(&wb, "E1"), "SUM(DoanhThu[tên2])");
    assert!(rename_table(&mut wb, 0, 0, "A1").is_err());
    assert!(rename_table(&mut wb, 0, 0, "có cách").is_err());
    assert!(rename_table(&mut wb, 0, 0, "1abc").is_err());
}

#[test]
fn convert_to_range_rewrites_structured_references() {
    let mut wb = sample();
    set_totals_row(&mut wb, 0, 0, true, "Tổng").unwrap();
    wb.sheets[0].set_a1("C1", text("Đôi"));
    resize_table(&mut wb, 0, 0, r("A1:C7"), &Locale::VI_VN).unwrap();
    set_formula(&mut wb, "C2", "[@Số]*2");
    let (ti, k, base) = calculated_for(&wb, 0, (1, 2)).unwrap();
    set_calculated(&mut wb, 0, ti, k, &base);
    set_formula(&mut wb, "E1", "SUM(Bảng1[Số])");
    set_formula(&mut wb, "E2", "Bảng1[[#Totals],[Số]]");
    let mut other = Worksheet::new("Khác");
    other.cells.insert(
        (0, 0),
        Cell::default().with_formula("COUNT(Bảng1[[Tên]:[Số]])"),
    );
    wb.sheets.push(other);
    convert_to_range(&mut wb, 0, 0).unwrap();
    assert_eq!(wb.sheets[0].tables.len(), 0);
    // Data rows 2–6 of B; totals row 7.
    assert_eq!(formula(&wb, "E1"), "SUM($B$2:$B$6)");
    assert_eq!(formula(&wb, "E2"), "$B$7");
    assert_eq!(formula(&wb, "B7"), "SUBTOTAL(109,$B$2:$B$6)");
    // [#This Row] keeps the row relative.
    assert_eq!(formula(&wb, "C4"), "$B4*2");
    assert_eq!(
        wb.sheets[1]
            .cell(0, 0)
            .and_then(|c| c.formula.as_ref())
            .unwrap()
            .text,
        "COUNT(S!$A$2:$B$6)"
    );
    recalc(&mut wb);
    assert_eq!(num(&wb, "E1"), 28.0);
    assert_eq!(num(&wb, "C6"), 2.0);
    // The table look is now direct formatting: the header has a fill.
    let h = wb.sheets[0].cell(0, 0).unwrap();
    assert!(crate::color::cell_fill_rgb(&wb, h).is_some());
}

#[test]
fn header_row_and_style_options() {
    let mut wb = sample();
    set_header_row(&mut wb, 0, 0, false).unwrap();
    let t = &wb.sheets[0].tables[0];
    assert_eq!(t.range, r("A2:B6"));
    assert_eq!(t.header_row_count, Some(0));
    assert!(t.auto_filter.is_none());
    assert!(wb.sheets[0].cell(0, 0).is_none());
    // Back: the names return to row 1 with the filter.
    set_header_row(&mut wb, 0, 0, true).unwrap();
    let t = &wb.sheets[0].tables[0];
    assert_eq!(t.range, r("A1:B6"));
    assert_eq!(t.header_row_count, None);
    assert_eq!(
        t.auto_filter.as_deref(),
        Some("<autoFilter ref=\"A1:B6\"/>")
    );
    assert_eq!(wb.sheets[0].cell(0, 1).map(|c| &c.value), Some(&text("Số")));
    set_style(
        &mut wb,
        0,
        0,
        &TableStylePatch {
            style: Some("TableStyleDark3".into()),
            banded_rows: Some(false),
            banded_columns: Some(true),
            first_column: Some(true),
            last_column: Some(true),
            filter_button: Some(false),
        },
    )
    .unwrap();
    let t = &wb.sheets[0].tables[0];
    let s = t.style.as_ref().unwrap();
    assert_eq!(s.name.as_deref(), Some("TableStyleDark3"));
    assert!(
        !s.show_row_stripes && s.show_column_stripes && s.show_first_column && s.show_last_column
    );
    assert!(t.auto_filter.is_none());
}

#[test]
fn table_names_follow_the_defined_name_rules() {
    assert!(valid_table_name("Bảng_1"));
    assert!(valid_table_name("_x.y"));
    assert!(!valid_table_name("A1"));
    assert!(!valid_table_name("R1C1"));
    assert!(!valid_table_name("R"));
    assert!(!valid_table_name("a b"));
    assert!(!valid_table_name(""));
}

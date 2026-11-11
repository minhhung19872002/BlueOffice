//! Data tools through the JSON bridge. Expected values are worked out by hand.

use super::super::SheetEngine;
use bo_model::sheet::CellValue;
use serde_json::{json, Value};

fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[allow(clippy::needless_pass_by_value)]
fn fail(e: &mut SheetEngine, v: Value) -> String {
    e.exec_json(&v.to_string()).unwrap_err().to_string()
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn val(e: &SheetEngine, s: usize, r: u32, c: u32) -> CellValue {
    e.workbook().unwrap().sheets[s]
        .cell(r, c)
        .map(|c| c.value.clone())
        .unwrap_or_default()
}

fn fmt_id(e: &SheetEngine, r: u32, c: u32) -> u32 {
    let wb = e.workbook().unwrap();
    let cell = wb.sheets[0].cell(r, c).unwrap();
    wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id)
}

fn n(x: f64) -> CellValue {
    CellValue::Number(x)
}

fn t(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

#[test]
fn text_to_columns_with_formats_and_preview() {
    let mut e = engine();
    e.set_input(
        0,
        0,
        0,
        "Nguyễn Văn A;\"Hà Nội; Việt Nam\";15/03/2024;007;x",
    )
    .unwrap();
    e.set_input(0, 1, 0, "Trần Thị B;Huế;01/12/2023;12,5;y")
        .unwrap();
    let split = json!({ "mode": "delimited", "delimiters": [";"], "qualifier": "\"" });
    let p = run(
        &mut e,
        json!({ "cmd": "textToColumns", "sheet": 0, "range": rng(0, 0, 1, 0),
                "split": split, "preview": true }),
    );
    assert_eq!(p["columns"], 5);
    assert_eq!(p["rows"][0][1], "Hà Nội; Việt Nam");
    // Nothing changed by the preview.
    assert!(e.workbook().unwrap().sheets[0].cell(0, 1).is_none());
    let r = run(
        &mut e,
        json!({ "cmd": "textToColumns", "sheet": 0, "range": rng(0, 0, 1, 0), "split": split,
                "formats": ["general", "general", { "date": "dmy" }, "text", "skip"] }),
    );
    assert_eq!(r, json!({ "rows": 2, "columns": 4 }));
    assert_eq!(val(&e, 0, 0, 0), t("Nguyễn Văn A"));
    assert_eq!(val(&e, 0, 0, 1), t("Hà Nội; Việt Nam"));
    // 15/03/2024 = serial 45366 (2024-01-01 is 45292, + 31 + 29 + 14).
    assert_eq!(val(&e, 0, 0, 2), n(45_366.0));
    assert_eq!(fmt_id(&e, 0, 2), 14);
    // 2023-12-01: 2024-01-01 (45292) − 31.
    assert_eq!(val(&e, 0, 1, 2), n(45_261.0));
    // Text column keeps the leading zeros and gets format `@` (id 49).
    assert_eq!(val(&e, 0, 0, 3), t("007"));
    assert_eq!(fmt_id(&e, 0, 3), 49);
    // The skipped column is not written.
    assert!(e.workbook().unwrap().sheets[0].cell(0, 4).is_none());
    assert!(e.undo().unwrap());
    assert_eq!(
        val(&e, 0, 0, 0),
        t("Nguyễn Văn A;\"Hà Nội; Việt Nam\";15/03/2024;007;x")
    );
}

#[test]
fn text_to_columns_fixed_width_to_other_cell() {
    let mut e = engine();
    e.set_input(0, 0, 0, "Lê  Thị Hoa 25").unwrap();
    // vi-VN: "12,5" is a number in general columns.
    e.set_input(0, 1, 0, "AB  CD      12,5").unwrap();
    run(
        &mut e,
        json!({ "cmd": "textToColumns", "sheet": 0, "range": rng(0, 0, 1, 0),
                "split": { "mode": "fixed", "breaks": [4, 12] },
                "dest": { "sheet": 0, "row": 0, "col": 2 } }),
    );
    // Source kept when the result goes elsewhere.
    assert_eq!(val(&e, 0, 0, 0), t("Lê  Thị Hoa 25"));
    assert_eq!(val(&e, 0, 0, 2), t("Lê"));
    assert_eq!(val(&e, 0, 0, 3), t("Thị Hoa"));
    assert_eq!(val(&e, 0, 0, 4), n(25.0));
    assert_eq!(val(&e, 0, 1, 3), t("CD"));
    assert_eq!(val(&e, 0, 1, 4), n(12.5));
    let err = fail(
        &mut e,
        json!({ "cmd": "textToColumns", "sheet": 0, "range": rng(0, 0, 1, 1),
                "split": { "mode": "fixed", "breaks": [] } }),
    );
    assert!(err.contains("oneColumn"), "{err}");
}

#[test]
fn remove_duplicates_reports_counts() {
    let mut e = engine();
    for (r, (a, b)) in [
        ("Tên", "Số"),
        ("An", "1"),
        ("an", "1"),
        ("Bình", "2"),
        ("An", "3"),
    ]
    .iter()
    .enumerate()
    {
        e.set_input(0, r as u32, 0, a).unwrap();
        e.set_input(0, r as u32, 1, b).unwrap();
    }
    // A single cell: the list around it.
    let r = run(
        &mut e,
        json!({ "cmd": "removeDuplicates", "sheet": 0, "range": rng(1, 0, 1, 0),
                "columns": [0, 1], "header": true }),
    );
    assert_eq!(r, json!({ "removed": 1, "remaining": 3 }));
    assert_eq!(val(&e, 0, 2, 0), t("Bình"));
    assert_eq!(val(&e, 0, 3, 1), n(3.0));
    assert_eq!(val(&e, 0, 4, 0), CellValue::Empty);
    let r = run(
        &mut e,
        json!({ "cmd": "removeDuplicates", "sheet": 0, "range": rng(0, 0, 3, 1),
                "columns": [0], "header": true }),
    );
    // "An" again in row 4.
    assert_eq!(r, json!({ "removed": 1, "remaining": 2 }));
}

#[test]
fn consolidate_values_and_links() {
    let mut e = engine();
    e.add_sheet(Some("Quý 2")).unwrap();
    for (s, rows) in [
        (
            0,
            [["", "Q1", "Q2"], ["Táo", "10", "20"], ["Cam", "5", "7"]],
        ),
        (1, [["", "Q2", "Q3"], ["Cam", "1", "2"], ["Lê", "3", "4"]]),
    ] {
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                e.set_input(s, r as u32, c as u32, v).unwrap();
            }
        }
    }
    let r = run(
        &mut e,
        json!({ "cmd": "consolidate", "sheet": 0, "sources": ["A1:C3", "'Quý 2'!$A$1:$C$3"],
                "function": "sum", "topRow": true, "leftCol": true,
                "dest": { "sheet": 0, "row": 5, "col": 0 } }),
    );
    assert_eq!(r, json!({ "sheet": 0, "range": rng(5, 0, 8, 3) }));
    assert_eq!(val(&e, 0, 5, 1), t("Q1"));
    assert_eq!(val(&e, 0, 5, 3), t("Q3"));
    assert_eq!(val(&e, 0, 7, 0), t("Cam"));
    assert_eq!(val(&e, 0, 7, 2), n(8.0));
    assert_eq!(val(&e, 0, 8, 3), n(4.0));
    assert_eq!(val(&e, 0, 6, 3), CellValue::Empty);
    // Linked: formulas over the source cells, recalculated.
    run(
        &mut e,
        json!({ "cmd": "consolidate", "sheet": 0, "sources": ["A1:C3", "'Quý 2'!A1:C3"],
                "function": "average", "topRow": true, "leftCol": true, "links": true,
                "dest": { "sheet": 1, "row": 5, "col": 0 } }),
    );
    // Linked: each summary row follows one detail row per source feeding it, with
    // links to the source cells; the details are grouped and collapsed.
    //   row 6  Q1 Q2 Q3          (labels)
    //   row 7  Sheet1  =B2 =C2   (detail, hidden)
    //   row 8  Táo  AVERAGE(B7:B7) …
    //   row 9  Sheet1  =B3 =C3   / row 10 'Quý 2' =B2 =C2 (Q2, Q3)
    //   row 11 Cam  AVERAGE(B9:B10) → 5, (7 + 1) / 2 = 4, 2
    //   row 12 'Quý 2' =B3 =C3   / row 13 Lê
    let wb = e.workbook().unwrap();
    let ws = &wb.sheets[1];
    let formula = |r: u32, c: u32| ws.cell(r, c).and_then(|x| x.formula.clone()).map(|f| f.text);
    assert_eq!(formula(6, 1).as_deref(), Some("Sheet1!$B$2"));
    assert_eq!(val(&e, 1, 6, 0), t("Sheet1"));
    assert_eq!(val(&e, 1, 7, 0), t("Táo"));
    assert_eq!(formula(7, 1).as_deref(), Some("AVERAGE(B7)"));
    assert_eq!(formula(9, 2).as_deref(), Some("'Quý 2'!$B$2"));
    assert_eq!(formula(10, 2).as_deref(), Some("AVERAGE(C9:C10)"));
    assert_eq!(val(&e, 1, 10, 0), t("Cam"));
    assert_eq!(val(&e, 1, 10, 1), n(5.0));
    assert_eq!(val(&e, 1, 10, 2), n(4.0));
    assert_eq!(val(&e, 1, 10, 3), n(2.0));
    assert_eq!(val(&e, 1, 12, 0), t("Lê"));
    assert_eq!(val(&e, 1, 12, 3), n(4.0));
    // Outline: detail rows level 1 and hidden, summaries marked collapsed.
    let hidden: Vec<u32> = (5..13)
        .filter(|r| ws.rows.get(r).is_some_and(|i| i.hidden && i.outline_level == 1))
        .collect();
    assert_eq!(hidden, vec![6, 8, 9, 11]);
    assert!([7, 10, 12]
        .iter()
        .all(|r| ws.rows.get(r).is_some_and(|i| i.collapsed && i.outline_level == 0)));
    // A source changes: the links follow.
    e.set_input(0, 2, 1, "9").unwrap();
    assert_eq!(val(&e, 1, 10, 1), n(9.0));
    // The output may not cover a source.
    let err = fail(
        &mut e,
        json!({ "cmd": "consolidate", "sheet": 0, "sources": ["A1:C3"],
                "dest": { "sheet": 0, "row": 1, "col": 1 } }),
    );
    assert!(err.contains("overlap"), "{err}");
}

fn sales(e: &mut SheetEngine) {
    let rows = [
        ["Khu vực", "Tháng", "Doanh thu"],
        ["Bắc", "1", "10"],
        ["Bắc", "2", "20"],
        ["Nam", "1", "5"],
        ["Nam", "2", "7"],
        ["Nam", "3", "8"],
        ["Trung", "1", "4"],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            e.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
}

#[test]
fn subtotal_computes_and_outlines_and_round_trips() {
    let mut e = engine();
    sales(&mut e);
    let r = run(
        &mut e,
        json!({ "cmd": "subtotal", "sheet": 0, "range": rng(0, 0, 0, 0), "groupBy": 0,
                "function": "sum", "columns": [2], "pageBreaks": true }),
    );
    assert_eq!(
        r,
        json!({ "groups": 3, "range": rng(0, 0, 10, 2), "levels": 2 })
    );
    // Bắc 10 + 20, Nam 5 + 7 + 8, Trung 4; grand total 54 (subtotals not counted twice).
    assert_eq!(val(&e, 0, 3, 2), n(30.0));
    assert_eq!(val(&e, 0, 7, 2), n(20.0));
    assert_eq!(val(&e, 0, 9, 2), n(4.0));
    assert_eq!(val(&e, 0, 10, 2), n(54.0));
    assert_eq!(val(&e, 0, 10, 0), t("Tổng cộng"));
    // Show level 2: only subtotals and the grand total.
    run(
        &mut e,
        json!({ "cmd": "outlineShowLevel", "sheet": 0, "rows": true, "level": 2 }),
    );
    let hidden = e.info().unwrap().sheets[0].hidden_rows.clone();
    assert_eq!(hidden, vec![(1, 2), (4, 6), (8, 8)]);

    // Saved and reopened: levels, collapsed flags, sheetFormatPr and breaks survive.
    let bytes = e.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_WRITE_DATATOOLS") {
        std::fs::write(format!("{dir}/subtotal-outline.xlsx"), &bytes).unwrap();
    }
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "x.xlsx").unwrap();
    let ws = &back.workbook().unwrap().sheets[0];
    let levels: Vec<u8> = (0..11)
        .map(|r| ws.rows.get(&r).map_or(0, |i| i.outline_level))
        .collect();
    assert_eq!(levels, vec![0, 2, 2, 1, 2, 2, 2, 1, 2, 1, 0]);
    assert_eq!(ws.format.outline_level_row, 2);
    assert!(ws.rows.get(&3).is_some_and(|i| i.collapsed && !i.hidden));
    assert_eq!(ws.page_setup.row_breaks, vec![3, 7]);
    let f = ws.cell(10, 2).unwrap().formula.clone().unwrap();
    assert_eq!(f.text, "SUBTOTAL(9,C2:C10)");

    // Remove all: back to seven rows, one undo step each.
    let r = run(
        &mut e,
        json!({ "cmd": "removeSubtotals", "sheet": 0, "range": rng(2, 1, 2, 1) }),
    );
    assert_eq!(r, json!({ "range": rng(0, 0, 6, 2) }));
    assert_eq!(val(&e, 0, 6, 0), t("Trung"));
    assert!(e.undo().unwrap());
    assert_eq!(val(&e, 0, 10, 2), n(54.0));
}

#[test]
fn subtotal_above_with_count() {
    let mut e = engine();
    sales(&mut e);
    run(
        &mut e,
        json!({ "cmd": "subtotal", "sheet": 0, "range": rng(0, 0, 6, 2), "groupBy": 0,
                "function": "count", "columns": [2], "summaryBelow": false,
                "label": "{0} (số dòng)", "grandLabel": "Tất cả" }),
    );
    // Header; "Tất cả" 6; Bắc 2; …; Nam 3; …; Trung 1.
    assert_eq!(val(&e, 0, 1, 0), t("Tất cả"));
    assert_eq!(val(&e, 0, 1, 2), n(6.0));
    assert_eq!(val(&e, 0, 2, 0), t("Bắc (số dòng)"));
    assert_eq!(val(&e, 0, 2, 2), n(2.0));
    assert_eq!(val(&e, 0, 5, 2), n(3.0));
    assert_eq!(val(&e, 0, 9, 2), n(1.0));
    assert_eq!(
        e.workbook().unwrap().sheets[0].props.summary_below,
        Some(false)
    );
    let err = fail(
        &mut e,
        json!({ "cmd": "subtotal", "sheet": 0, "range": rng(0, 0, 6, 2), "groupBy": 7,
                "columns": [2] }),
    );
    assert!(err.contains("subtotal: badColumn"), "{err}");
}

#[test]
fn outline_group_auto_and_clear() {
    let mut e = engine();
    for (r, v) in ["1", "2", "=SUM(A1:A2)", "4", "=SUM(A4:A4)"]
        .iter()
        .enumerate()
    {
        e.set_input(0, r as u32, 0, v).unwrap();
    }
    e.set_input(0, 0, 3, "=SUM(A1:C1)").unwrap();
    let r = run(&mut e, json!({ "cmd": "autoOutline", "sheet": 0 }));
    assert_eq!(r, json!({ "rowLevels": 1, "colLevels": 1 }));
    let ws = &e.workbook().unwrap().sheets[0];
    let levels: Vec<u8> = (0..5)
        .map(|r| ws.rows.get(&r).map_or(0, |i| i.outline_level))
        .collect();
    assert_eq!(levels, vec![1, 1, 0, 1, 0]);
    assert_eq!(ws.col_info(1).map(|c| c.outline_level), Some(1));
    run(
        &mut e,
        json!({ "cmd": "outlineGroup", "sheet": 0, "rows": true, "first": 0, "last": 1 }),
    );
    assert_eq!(e.workbook().unwrap().sheets[0].format.outline_level_row, 2);
    run(&mut e, json!({ "cmd": "clearOutline", "sheet": 0 }));
    let ws = &e.workbook().unwrap().sheets[0];
    assert_eq!(ws.format.outline_level_row, 0);
    assert_eq!(ws.format.outline_level_col, 0);
    // No formulas: nothing to outline.
    let mut e = engine();
    e.set_input(0, 0, 0, "1").unwrap();
    let err = fail(&mut e, json!({ "cmd": "autoOutline", "sheet": 0 }));
    assert!(err.contains("autoOutline: none"), "{err}");
}

#[test]
fn invalid_cells_listed() {
    let mut e = engine();
    run(
        &mut e,
        json!({ "cmd": "setValidation", "sheet": 0, "range": rng(0, 0, 4, 0),
                "spec": { "kind": "decimal", "operator": "greaterThan", "formula1": "0" } }),
    );
    for (r, v) in ["1", "-2", "abc", "", "0"].iter().enumerate() {
        e.set_input(0, r as u32, 0, v).unwrap();
    }
    let r = run(&mut e, json!({ "cmd": "invalidCells", "sheet": 0 }));
    assert_eq!(
        r,
        json!({ "cells": [[1, 0], [2, 0], [4, 0]], "truncated": false })
    );
}

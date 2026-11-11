//! BlueSheet AutoSum (Σ, Alt+=): the read-only `autoSum` plan and the plan applied through the
//! normal commands in one `batch` (one undo step).
//!
//! Reference behaviour (the spreadsheet convention the project follows, written down before
//! the implementation; the expected formulas below are that convention applied by hand):
//!
//! * one empty cell → the formula is *proposed* in the cell editor: the run of numbers
//!   directly above (a text header ends the run and is not included); none above → the run
//!   directly on the left; both → above wins; nothing → `=SUM()`;
//! * a column of numbers → the total goes into the first empty cell below, at once;
//! * a row of numbers → the first empty cell on the right;
//! * a block → one total under each column; with an empty last row and last column in the
//!   selection → both are filled, and the corner gets the grand total;
//! * an empty cell at the end of the selected column receives the total of the cells above;
//! * the selection afterwards includes the totals;
//! * a filtered list → `SUBTOTAL(9, range)`; a table → its totals row.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn book(rows: &[&[&str]]) -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            if !v.is_empty() {
                e.set_input(0, r as u32, c as u32, v).unwrap();
            }
        }
    }
    e
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn plan(e: &mut SheetEngine, r: &Value, function: &str) -> Value {
    exec(
        e,
        &json!({ "cmd": "autoSum", "sheet": 0, "range": r, "function": function }),
    )
}

/// Applies a `write` plan the way the editor does: one batch.
fn apply(e: &mut SheetEngine, p: &Value) {
    assert_eq!(p["mode"], "write", "{p}");
    let mut cmds = vec![json!({ "cmd": "setInputs", "sheet": 0, "cells": p["cells"] })];
    for f in p["formats"].as_array().unwrap() {
        cmds.push(json!({
            "cmd": "format", "sheet": 0,
            "range": rng(
                f["row"].as_u64().unwrap() as u32, f["col"].as_u64().unwrap() as u32,
                f["row"].as_u64().unwrap() as u32, f["col"].as_u64().unwrap() as u32),
            "patch": { "numFmt": f["numFmt"] },
        }));
    }
    exec(e, &json!({ "cmd": "batch", "cmds": cmds }));
}

fn input(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().input
}

fn shown(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().display
}

/// The owner's sheet: C1 header, C2:C4 = 12.5, 25, 37.5 (decimal comma: the engine's
/// default locale is vi-VN).
fn column() -> SheetEngine {
    book(&[
        &["", "", "Số tiền"],
        &["", "", "12,5"],
        &["", "", "25"],
        &["", "", "37,5"],
    ])
}

#[test]
fn a_column_of_numbers_gets_its_total_below() {
    let mut e = column();
    let p = plan(&mut e, &rng(1, 2, 3, 2), "sum");
    assert_eq!(p["mode"], "write");
    assert_eq!(p["cells"], json!([[4, 2, "=SUM(C2:C4)"]]));
    // The selection grows over the total: C2:C5.
    assert_eq!(p["select"], rng(1, 2, 4, 2));
    apply(&mut e, &p);
    assert_eq!(input(&e, 4, 2), "=SUM(C2:C4)");
    // 12.5 + 25 + 37.5 = 75
    assert_eq!(shown(&e, 4, 2), "75");
    // One undo step takes the total back.
    assert!(e.undo().unwrap());
    assert_eq!(input(&e, 4, 2), "");
    assert_eq!(input(&e, 3, 2), "37,5");
}

#[test]
fn the_other_functions_use_the_same_cells() {
    // AVERAGE 25, COUNT 3, MAX 37.5, MIN 12.5 of 12.5, 25, 37.5.
    for (function, formula, value) in [
        ("average", "=AVERAGE(C2:C4)", "25"),
        ("count", "=COUNT(C2:C4)", "3"),
        ("max", "=MAX(C2:C4)", "37,5"),
        ("min", "=MIN(C2:C4)", "12,5"),
    ] {
        let mut e = column();
        let p = plan(&mut e, &rng(1, 2, 3, 2), function);
        assert_eq!(p["cells"], json!([[4, 2, formula]]), "{function}");
        apply(&mut e, &p);
        assert_eq!(shown(&e, 4, 2), value, "{function}");
    }
}

#[test]
fn the_total_skips_filled_cells_below_the_column() {
    let mut e = column();
    e.set_input(0, 4, 2, "ghi chú").unwrap();
    let p = plan(&mut e, &rng(1, 2, 3, 2), "sum");
    // C5 is taken: the first empty cell below is C6.
    assert_eq!(p["cells"], json!([[5, 2, "=SUM(C2:C4)"]]));
    assert_eq!(p["select"], rng(1, 2, 5, 2));
}

#[test]
fn an_empty_cell_ending_the_selected_column_receives_the_total() {
    let mut e = column();
    // C2:C5 with C5 empty.
    let p = plan(&mut e, &rng(1, 2, 4, 2), "sum");
    assert_eq!(p["cells"], json!([[4, 2, "=SUM(C2:C4)"]]));
    assert_eq!(p["select"], rng(1, 2, 4, 2));
}

#[test]
fn a_row_of_numbers_gets_its_total_on_the_right() {
    let mut e = book(&[&["Quý", "1", "2", "3"]]);
    let p = plan(&mut e, &rng(0, 1, 0, 3), "sum");
    assert_eq!(p["cells"], json!([[0, 4, "=SUM(B1:D1)"]]));
    assert_eq!(p["select"], rng(0, 1, 0, 4));
    apply(&mut e, &p);
    assert_eq!(shown(&e, 0, 4), "6");
    // B1:E1 with E1 empty: the total goes into E1.
    let mut e = book(&[&["Quý", "1", "2", "3"]]);
    let p = plan(&mut e, &rng(0, 1, 0, 4), "sum");
    assert_eq!(p["cells"], json!([[0, 4, "=SUM(B1:D1)"]]));
}

fn block() -> SheetEngine {
    // A1:C3 = 1 2 3 / 4 5 6 / 7 8 9
    book(&[&["1", "2", "3"], &["4", "5", "6"], &["7", "8", "9"]])
}

#[test]
fn a_block_gets_one_total_under_each_column() {
    let mut e = block();
    let p = plan(&mut e, &rng(0, 0, 2, 2), "sum");
    assert_eq!(
        p["cells"],
        json!([
            [3, 0, "=SUM(A1:A3)"],
            [3, 1, "=SUM(B1:B3)"],
            [3, 2, "=SUM(C1:C3)"]
        ])
    );
    assert_eq!(p["select"], rng(0, 0, 3, 2));
    apply(&mut e, &p);
    // 1+4+7, 2+5+8, 3+6+9
    assert_eq!(
        [shown(&e, 3, 0), shown(&e, 3, 1), shown(&e, 3, 2)],
        ["12", "15", "18"]
    );
}

#[test]
fn an_empty_last_row_and_column_get_totals_and_the_grand_total() {
    let mut e = block();
    // A1:D4: row 4 and column D are empty.
    let p = plan(&mut e, &rng(0, 0, 3, 3), "sum");
    assert_eq!(
        p["cells"],
        json!([
            [3, 0, "=SUM(A1:A3)"],
            [3, 1, "=SUM(B1:B3)"],
            [3, 2, "=SUM(C1:C3)"],
            [0, 3, "=SUM(A1:C1)"],
            [1, 3, "=SUM(A2:C2)"],
            [2, 3, "=SUM(A3:C3)"],
            [3, 3, "=SUM(D1:D3)"]
        ])
    );
    assert_eq!(p["select"], rng(0, 0, 3, 3));
    apply(&mut e, &p);
    // Rows: 6, 15, 24; grand total 45 = 1 + … + 9.
    assert_eq!(
        [shown(&e, 0, 3), shown(&e, 1, 3), shown(&e, 2, 3)],
        ["6", "15", "24"]
    );
    assert_eq!(shown(&e, 3, 3), "45");
    // One undo step for all seven totals.
    assert!(e.undo().unwrap());
    assert_eq!(input(&e, 3, 3), "");
    assert_eq!(input(&e, 0, 3), "");
}

#[test]
fn an_empty_last_column_alone_gets_row_totals() {
    let mut e = block();
    let p = plan(&mut e, &rng(0, 0, 2, 3), "sum");
    assert_eq!(
        p["cells"],
        json!([
            [0, 3, "=SUM(A1:C1)"],
            [1, 3, "=SUM(A2:C2)"],
            [2, 3, "=SUM(A3:C3)"]
        ])
    );
}

#[test]
fn label_columns_get_no_total() {
    let mut e = book(&[
        &["Tỉnh", "Quý 1", "Quý 2"],
        &["Hà Nội", "10", "12"],
        &["Huế", "5", "4"],
    ]);
    let p = plan(&mut e, &rng(0, 0, 2, 2), "sum");
    assert_eq!(
        p["cells"],
        json!([[3, 1, "=SUM(B1:B3)"], [3, 2, "=SUM(C1:C3)"]])
    );
    apply(&mut e, &p);
    assert_eq!([shown(&e, 3, 1), shown(&e, 3, 2)], ["15", "16"]);
}

#[test]
fn one_empty_cell_proposes_the_numbers_above() {
    let mut e = column();
    let p = plan(&mut e, &rng(4, 2, 4, 2), "sum");
    assert_eq!(p["mode"], "edit");
    assert_eq!((p["row"].as_u64(), p["col"].as_u64()), (Some(4), Some(2)));
    // The header "Số tiền" in C1 ends the run and is not part of it.
    assert_eq!(p["text"], "=SUM(C2:C4)");
    // The range is selected inside the formula: "=SUM(" is 5 bytes, "C2:C4" another 5.
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(5), Some(10))
    );
    // Nothing was written.
    assert_eq!(input(&e, 4, 2), "");
    let p = plan(&mut e, &rng(4, 2, 4, 2), "average");
    assert_eq!(p["text"], "=AVERAGE(C2:C4)");
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(9), Some(14))
    );
}

#[test]
fn one_empty_cell_looks_left_when_nothing_is_above() {
    let mut e = book(&[&["Quý", "1", "2", "3"]]);
    let p = plan(&mut e, &rng(0, 4, 0, 4), "sum");
    assert_eq!(p["text"], "=SUM(B1:D1)");
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(5), Some(10))
    );
}

#[test]
fn above_wins_over_left() {
    // C3 has 1, 2 above (C1:C2) and 5, 6 on its left (A3:B3).
    let mut e = book(&[&["", "", "1"], &["", "", "2"], &["5", "6", ""]]);
    let p = plan(&mut e, &rng(2, 2, 2, 2), "sum");
    assert_eq!(p["text"], "=SUM(C1:C2)");
}

#[test]
fn text_directly_above_makes_it_look_left() {
    // B2: text above (B1), a number on the left (A2).
    let mut e = book(&[&["", "chữ"], &["4", ""]]);
    let p = plan(&mut e, &rng(1, 1, 1, 1), "sum");
    assert_eq!(p["text"], "=SUM(A2)");
}

#[test]
fn nothing_to_add_up_proposes_an_empty_call() {
    let mut e = book(&[&["chữ"]]);
    let p = plan(&mut e, &rng(5, 5, 5, 5), "sum");
    assert_eq!(p["mode"], "edit");
    assert_eq!(p["text"], "=SUM()");
    // The caret sits between the parentheses.
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(5), Some(5))
    );
}

#[test]
fn a_gap_of_empty_cells_is_bridged() {
    // A1:A3 numbers, A4 empty, the total in A5 covers A1:A4.
    let mut e = book(&[&["1"], &["2"], &["3"]]);
    let p = plan(&mut e, &rng(4, 0, 4, 0), "sum");
    assert_eq!(p["text"], "=SUM(A1:A4)");
}

#[test]
fn a_total_written_before_is_opened_again() {
    let mut e = column();
    e.set_input(0, 4, 2, "=SUM(C2:C3)").unwrap();
    let p = plan(&mut e, &rng(4, 2, 4, 2), "sum");
    assert_eq!(p["mode"], "edit");
    // The formula in the cell, not a new guess.
    assert_eq!(p["text"], "=SUM(C2:C3)");
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(5), Some(10))
    );
}

#[test]
fn empty_cells_under_the_data_each_get_a_total() {
    let mut e = block();
    // A4:C4 (all empty) below the block.
    let p = plan(&mut e, &rng(3, 0, 3, 2), "sum");
    assert_eq!(
        p["cells"],
        json!([
            [3, 0, "=SUM(A1:A3)"],
            [3, 1, "=SUM(B1:B3)"],
            [3, 2, "=SUM(C1:C3)"]
        ])
    );
    assert_eq!(p["select"], rng(3, 0, 3, 2));
}

#[test]
fn the_total_takes_the_number_format_of_its_numbers() {
    let mut e = column();
    exec(
        &mut e,
        &json!({ "cmd": "format", "sheet": 0, "range": rng(1, 2, 3, 2), "patch": { "numFmt": "#,##0.00" } }),
    );
    let p = plan(&mut e, &rng(1, 2, 3, 2), "sum");
    assert_eq!(
        p["formats"],
        json!([{ "row": 4, "col": 2, "numFmt": "#,##0.00" }])
    );
    apply(&mut e, &p);
    assert_eq!(shown(&e, 4, 2), "75,00");
    // A count is not an amount: no format.
    let mut e = column();
    exec(
        &mut e,
        &json!({ "cmd": "format", "sheet": 0, "range": rng(1, 2, 3, 2), "patch": { "numFmt": "#,##0.00" } }),
    );
    let p = plan(&mut e, &rng(1, 2, 3, 2), "count");
    assert_eq!(p["formats"], json!([]));
}

#[test]
fn a_filtered_list_is_added_up_with_subtotal() {
    let mut e = book(&[&["Tên", "Số"], &["a", "10"], &["b", "20"], &["a", "30"]]);
    exec(
        &mut e,
        &json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
    );
    exec(
        &mut e,
        &json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": { "values": ["a"] } }),
    );
    assert_eq!(e.info().unwrap().sheets[0].hidden_rows, vec![(2, 2)]);
    // One cell below the list: SUBTOTAL(9, …), the range selected after "=SUBTOTAL(9,".
    let p = plan(&mut e, &rng(4, 1, 4, 1), "sum");
    assert_eq!(p["text"], "=SUBTOTAL(9,B2:B4)");
    assert_eq!(
        (p["selStart"].as_u64(), p["selEnd"].as_u64()),
        (Some(12), Some(17))
    );
    // The selected column: written at once; the hidden 20 is left out → 10 + 30 = 40.
    let p = plan(&mut e, &rng(1, 1, 3, 1), "sum");
    assert_eq!(p["cells"], json!([[4, 1, "=SUBTOTAL(9,B2:B4)"]]));
    apply(&mut e, &p);
    assert_eq!(shown(&e, 4, 1), "40");
    // AVERAGE is function number 1.
    let p = plan(&mut e, &rng(5, 1, 5, 1), "average");
    assert!(
        p["text"].as_str().unwrap().starts_with("=SUBTOTAL(1,"),
        "{p}"
    );
}

/// A1:B4 = Mặt hàng / Giá with 10, 20, 30 as a table.
fn table() -> SheetEngine {
    let mut e = book(&[
        &["Mặt hàng", "Giá", "SL"],
        &["bút", "10", "1"],
        &["vở", "20", "2"],
        &["thước", "30", "3"],
    ]);
    exec(
        &mut e,
        &json!({ "cmd": "createTable", "sheet": 0, "range": rng(0, 0, 3, 2) }),
    );
    e
}

/// Applies a `table` plan the way the editor does.
fn apply_table(e: &mut SheetEngine, p: &Value) {
    assert_eq!(p["mode"], "table", "{p}");
    let mut cmds = Vec::new();
    if p["show"] == true {
        cmds.push(
            json!({ "cmd": "tableUpdate", "sheet": 0, "table": p["table"], "totalsRow": true }),
        );
    }
    for c in p["clear"].as_array().unwrap() {
        cmds.push(json!({ "cmd": "tableTotal", "sheet": 0, "table": p["table"], "column": c, "function": "none" }));
    }
    for c in p["columns"].as_array().unwrap() {
        cmds.push(json!({ "cmd": "tableTotal", "sheet": 0, "table": p["table"], "column": c, "function": p["function"] }));
    }
    exec(e, &json!({ "cmd": "batch", "cmds": cmds }));
}

#[test]
fn a_table_column_gets_the_tables_totals_row() {
    let mut e = table();
    // B2:B4 inside the table.
    let p = plan(&mut e, &rng(1, 1, 3, 1), "sum");
    assert_eq!(p["mode"], "table");
    assert_eq!(p["show"], true);
    assert_eq!(p["columns"], json!([1]));
    // The totals row would add up the last column by itself: not asked for.
    assert_eq!(p["clear"], json!([2]));
    assert_eq!(p["select"], rng(1, 1, 4, 1));
    apply_table(&mut e, &p);
    let t = exec(&mut e, &json!({ "cmd": "tables", "sheet": 0 }));
    assert_eq!(t["tables"][0]["totalsRow"], true);
    assert_eq!(t["tables"][0]["columns"][1]["totalsFunction"], "sum");
    assert_eq!(t["tables"][0]["columns"][2]["totalsFunction"], "none");
    // 10 + 20 + 30
    assert_eq!(shown(&e, 4, 1), "60");
    assert_eq!(input(&e, 4, 2), "");
    // One undo step removes the totals row again.
    assert!(e.undo().unwrap());
    let t = exec(&mut e, &json!({ "cmd": "tables", "sheet": 0 }));
    assert_eq!(t["tables"][0]["totalsRow"], false);
    // The cell directly below a table column does the same; the average of 1, 2, 3 is 2.
    let p = plan(&mut e, &rng(4, 2, 4, 2), "average");
    assert_eq!(p["mode"], "table");
    assert_eq!(
        (p["columns"].clone(), p["clear"].clone()),
        (json!([2]), json!([]))
    );
    apply_table(&mut e, &p);
    assert_eq!(shown(&e, 4, 2), "2");
    // With the totals row shown, a cell of it only changes its function.
    let p = plan(&mut e, &rng(4, 1, 4, 1), "max");
    assert_eq!(p["show"], false);
    apply_table(&mut e, &p);
    assert_eq!(shown(&e, 4, 1), "30");
}

#[test]
fn nothing_to_add_up_in_a_text_selection() {
    let mut e = book(&[&["a", "b"], &["c", "d"]]);
    let p = plan(&mut e, &rng(0, 0, 1, 1), "sum");
    assert_eq!(p["mode"], "none");
}

#[test]
fn the_function_list_is_sorted_and_has_the_summary_functions() {
    let mut e = column();
    let l = exec(&mut e, &json!({ "cmd": "functionList" }));
    let names: Vec<&str> = l["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    for f in ["SUM", "AVERAGE", "COUNT", "MAX", "MIN", "SUBTOTAL"] {
        assert!(names.contains(&f), "{f}");
    }
    assert!(names.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn the_status_bar_figures_leave_out_filtered_rows() {
    let mut e = book(&[&["Tên", "Số"], &["a", "10"], &["b", "20"], &["a", "30"]]);
    let stats = |e: &mut SheetEngine| {
        exec(
            e,
            &json!({ "cmd": "rangeStats", "sheet": 0, "ranges": [rng(1, 1, 3, 1)] }),
        )
    };
    let all = stats(&mut e);
    assert_eq!(
        (all["sum"].as_f64(), all["count"].as_u64()),
        (Some(60.0), Some(3))
    );
    exec(
        &mut e,
        &json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
    );
    exec(
        &mut e,
        &json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": { "values": ["a"] } }),
    );
    // The row of "b" (20) is filtered out: 10 + 30, two cells, mean 20.
    let shown = stats(&mut e);
    assert_eq!(
        (shown["sum"].as_f64(), shown["count"].as_u64()),
        (Some(40.0), Some(2))
    );
    assert_eq!(shown["average"].as_f64(), Some(20.0));
    // A row hidden by hand still counts.
    exec(
        &mut e,
        &json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": null }),
    );
    exec(
        &mut e,
        &json!({ "cmd": "hide", "sheet": 0, "rows": true, "first": 2, "last": 2, "hidden": true }),
    );
    let by_hand = stats(&mut e);
    assert_eq!(by_hand["sum"].as_f64(), Some(60.0));
}

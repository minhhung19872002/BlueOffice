//! Engine tests of SHEET-TABLE-001: totals row, calculated columns, automatic expansion and their
//! undo steps, header renames, convert to range and the `.xlsx` round trip. Expected values are
//! worked out by hand in the comments.

use crate::sheet::SheetEngine;
use serde_json::{json, Value};

/// A1:B4 = Mặt hàng / Giá with 10, 20, 30; table over A1:B4.
fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    for (r, (a, b)) in [
        ("Mặt hàng", "Giá"),
        ("bút", "10"),
        ("vở", "20"),
        ("thước", "30"),
    ]
    .iter()
    .enumerate()
    {
        e.set_input(0, r as u32, 0, a).unwrap();
        e.set_input(0, r as u32, 1, b).unwrap();
    }
    run(
        &mut e,
        json!({ "cmd": "createTable", "sheet": 0, "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 1 } }),
    );
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
}

fn table(e: &mut SheetEngine) -> Value {
    run(e, json!({ "cmd": "tables", "sheet": 0 }))
}

fn shown(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().display
}

fn input(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().input
}

#[test]
fn typing_below_expands_and_undo_takes_back_the_expansion_first() {
    let mut e = engine();
    e.set_input(0, 4, 0, "tẩy").unwrap();
    let t = table(&mut e);
    assert_eq!(
        t["tables"][0]["range"],
        json!({ "r0": 0, "c0": 0, "r1": 4, "c1": 1 })
    );
    assert_eq!(t["lastAuto"]["kind"], "expand");
    // Undo 1: the expansion only; the typed text stays.
    assert!(e.undo().unwrap());
    let t = table(&mut e);
    assert_eq!(t["tables"][0]["range"]["r1"], 3);
    assert_eq!(shown(&e, 4, 0), "tẩy");
    // Undo 2: the typing.
    assert!(e.undo().unwrap());
    assert_eq!(shown(&e, 4, 0), "");
    // Turned off, typing below leaves the table alone.
    run(
        &mut e,
        json!({ "cmd": "tableOptions", "autoExpand": false }),
    );
    e.set_input(0, 4, 0, "tẩy").unwrap();
    assert_eq!(table(&mut e)["tables"][0]["range"]["r1"], 3);
}

#[test]
fn pasting_below_expands() {
    let mut e = engine();
    // Copy row 2 (bút, 10) and paste it at row 5 and 6 (two tiles): the table takes both rows.
    run(
        &mut e,
        json!({ "cmd": "copy", "sheet": 0, "range": { "r0": 1, "c0": 0, "r1": 1, "c1": 1 } }),
    );
    run(
        &mut e,
        json!({ "cmd": "paste", "sheet": 0, "row": 4, "col": 0,
                "range": { "r0": 4, "c0": 0, "r1": 5, "c1": 1 } }),
    );
    assert_eq!(table(&mut e)["tables"][0]["range"]["r1"], 5);
    // Undo the expansion only: the pasted cells stay.
    e.undo().unwrap();
    assert_eq!(table(&mut e)["tables"][0]["range"]["r1"], 3);
    assert_eq!(shown(&e, 5, 1), "10");
}

#[test]
fn typing_right_adds_a_named_column() {
    let mut e = engine();
    e.set_input(0, 0, 2, "Số lượng").unwrap();
    let t = table(&mut e);
    assert_eq!(t["tables"][0]["range"]["c1"], 2);
    assert_eq!(t["tables"][0]["columns"][2]["name"], "Số lượng");
}

#[test]
fn calculated_column_is_its_own_undo_step() {
    let mut e = engine();
    e.set_input(0, 0, 2, "Thuế").unwrap(); // expands to C
    e.set_input(0, 1, 2, "=[@Giá]*0.1").unwrap();
    // C2..C4 = 1, 2, 3.
    assert_eq!(shown(&e, 2, 2), "2");
    assert_eq!(shown(&e, 3, 2), "3");
    assert_eq!(input(&e, 3, 2), "=Bảng1[[#This Row],[Giá]]*0.1");
    let t = table(&mut e);
    assert_eq!(t["lastAuto"]["kind"], "calc");
    assert_eq!(
        t["tables"][0]["columns"][2]["calculated"],
        "Bảng1[[#This Row],[Giá]]*0.1"
    );
    // Undo: the fill goes, the typed formula stays.
    e.undo().unwrap();
    assert_eq!(shown(&e, 3, 2), "");
    assert_eq!(shown(&e, 1, 2), "1");
    e.redo().unwrap();
    // An exception typed in C3 is kept; a new row gets the column formula.
    e.set_input(0, 2, 2, "99").unwrap();
    e.set_input(0, 4, 0, "tẩy").unwrap();
    e.set_input(0, 4, 1, "50").unwrap();
    assert_eq!(shown(&e, 2, 2), "99");
    assert_eq!(shown(&e, 4, 2), "5");
}

#[test]
fn header_edit_renames_the_column_and_its_references() {
    let mut e = engine();
    e.set_input(0, 0, 4, "=SUM(Bảng1[Giá])").unwrap();
    assert_eq!(shown(&e, 0, 4), "60");
    e.set_input(0, 0, 1, "Đơn giá").unwrap();
    assert_eq!(input(&e, 0, 4), "=SUM(Bảng1[Đơn giá])");
    assert_eq!(shown(&e, 0, 4), "60");
    // One undo step takes back the header and the rename.
    e.undo().unwrap();
    assert_eq!(shown(&e, 0, 1), "Giá");
    assert_eq!(table(&mut e)["tables"][0]["columns"][1]["name"], "Giá");
    assert_eq!(input(&e, 0, 4), "=SUM(Bảng1[Giá])");
}

#[test]
fn totals_row_functions_and_round_trip() {
    let mut e = engine();
    run(
        &mut e,
        json!({ "cmd": "tableUpdate", "sheet": 0, "table": 0, "totalsRow": true, "totalsLabel": "Cộng",
                "style": "TableStyleLight3", "bandedColumns": true, "name": "BanHang" }),
    );
    // 10 + 20 + 30 = 60 in B5, label in A5.
    assert_eq!(shown(&e, 4, 0), "Cộng");
    assert_eq!(shown(&e, 4, 1), "60");
    run(
        &mut e,
        json!({ "cmd": "tableTotal", "sheet": 0, "table": 0, "column": 1, "function": "average" }),
    );
    assert_eq!(shown(&e, 4, 1), "20");
    // Hide row 3 (20): average of 10 and 30 = 20; max = 30; countNums = 2.
    run(
        &mut e,
        json!({ "cmd": "hide", "sheet": 0, "rows": true, "first": 2, "last": 2, "hidden": true }),
    );
    assert_eq!(shown(&e, 4, 1), "20");
    run(
        &mut e,
        json!({ "cmd": "tableTotal", "sheet": 0, "table": 0, "column": 1, "function": "max" }),
    );
    assert_eq!(shown(&e, 4, 1), "30");
    run(
        &mut e,
        json!({ "cmd": "tableTotal", "sheet": 0, "table": 0, "column": 1, "function": "sum" }),
    );
    // 10 + 30 with row 3 hidden.
    assert_eq!(shown(&e, 4, 1), "40");
    run(
        &mut e,
        json!({ "cmd": "tableTotal", "sheet": 0, "table": 0, "column": 1, "function": "countNums" }),
    );
    assert_eq!(shown(&e, 4, 1), "2");
    run(
        &mut e,
        json!({ "cmd": "tableTotal", "sheet": 0, "table": 0, "column": 0, "function": "count" }),
    );
    let bytes = e.save("xlsx", 0).unwrap();
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "x.xlsx").unwrap();
    let t = table(&mut back);
    let t0 = &t["tables"][0];
    assert_eq!(t0["name"], "BanHang");
    assert_eq!(t0["totalsRow"], true);
    assert_eq!(t0["style"], "TableStyleLight3");
    assert_eq!(t0["bandedColumns"], true);
    assert_eq!(t0["columns"][0]["totalsFunction"], "count");
    assert_eq!(t0["columns"][1]["totalsFunction"], "countNums");
    assert_eq!(input(&back, 4, 1), "=SUBTOTAL(102,BanHang[Giá])");
    // Saved again unchanged, the table part is the same.
    let again = back.save("xlsx", 0).unwrap();
    assert_eq!(table_part(&bytes), table_part(&again));
}

fn table_part(xlsx: &[u8]) -> Vec<u8> {
    let pkg = bo_opc::Package::read(xlsx, &bo_opc::Limits::default()).unwrap();
    pkg.part("/xl/tables/table1.xml").unwrap().data.clone()
}

#[test]
fn resize_and_convert_to_range() {
    let mut e = engine();
    e.set_input(0, 0, 3, "=SUM(Bảng1[Giá])").unwrap();
    run(
        &mut e,
        json!({ "cmd": "tableResize", "sheet": 0, "table": 0, "range": { "r0": 0, "c0": 0, "r1": 2, "c1": 1 } }),
    );
    // Data rows 2–3: 10 + 20 = 30.
    assert_eq!(shown(&e, 0, 3), "30");
    assert!(e
        .exec_json(
            &json!({ "cmd": "tableResize", "sheet": 0, "table": 0, "range": { "r0": 1, "c0": 0, "r1": 2, "c1": 1 } })
                .to_string()
        )
        .is_err());
    run(
        &mut e,
        json!({ "cmd": "tableConvert", "sheet": 0, "table": 0 }),
    );
    assert_eq!(input(&e, 0, 3), "=SUM($B$2:$B$3)");
    assert_eq!(table(&mut e)["tables"], json!([]));
    e.undo().unwrap();
    assert_eq!(input(&e, 0, 3), "=SUM(Bảng1[Giá])");
    let styles = run(&mut e, json!({ "cmd": "tableStyles" }));
    assert_eq!(styles.as_array().map(Vec::len), Some(21));
}

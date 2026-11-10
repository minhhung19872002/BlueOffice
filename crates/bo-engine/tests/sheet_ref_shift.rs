//! Cell-anchored objects follow inserted / deleted rows and columns (CORE-MISC-005): form
//! controls (anchor, linked cell, input range — [MS-XLSX] `controlPr/anchor`, `fmlaLink`,
//! `fmlaRange`) and sparklines (`xm:sqref` location, `xm:f` data). Saving keeps the moved
//! placement, undo restores it exactly.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn controls(e: &mut SheetEngine) -> Vec<Value> {
    run(e, json!({"cmd": "formControls", "sheet": 0}))
        .as_array()
        .unwrap()
        .clone()
}

fn sparklines(e: &mut SheetEngine) -> Vec<(String, [u32; 2])> {
    let wb = e.workbook().unwrap();
    wb.sheets[0]
        .sparkline_groups
        .iter()
        .flat_map(|g| &g.sparklines)
        .map(|s| {
            (
                s.formula.clone().unwrap_or_default(),
                [s.location.row, s.location.col],
            )
        })
        .collect()
}

fn reopen(e: &mut SheetEngine) -> SheetEngine {
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    f
}

fn engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e
}

/// A check box at B3 linked to $D$5 and a combo box at B6 listing $F$1:$F$4.
fn with_controls() -> SheetEngine {
    let mut e = engine();
    for (i, v) in ["a", "b", "c", "d"].iter().enumerate() {
        e.set_input(0, i as u32, 5, v).unwrap();
    }
    let cb = run(
        &mut e,
        json!({"cmd": "insertFormControl", "sheet": 0, "kind": "checkBox", "row": 2, "col": 1, "text": "Chọn"}),
    )["index"]
        .as_u64()
        .unwrap();
    run(
        &mut e,
        json!({"cmd": "formatFormControl", "sheet": 0, "index": cb, "spec": {"link": "$D$5"}}),
    );
    let combo = run(
        &mut e,
        json!({"cmd": "insertFormControl", "sheet": 0, "kind": "comboBox", "row": 5, "col": 1}),
    )["index"]
        .as_u64()
        .unwrap();
    run(
        &mut e,
        json!({"cmd": "formatFormControl", "sheet": 0, "index": combo, "spec": {"inputRange": "$F$1:$F$4", "link": "$H$1"}}),
    );
    e
}

#[test]
fn form_controls_follow_inserted_rows_and_columns() {
    let mut e = with_controls();
    let before = controls(&mut e);
    assert_eq!(before[0]["from"], json!([2, 1]));
    // Two rows inserted above everything: anchors and references move down.
    e.structural(0, true, 0, 2, false).unwrap();
    let l = controls(&mut e);
    assert_eq!(l[0]["from"], json!([4, 1]));
    assert_eq!(l[0]["link"], "$D$7");
    assert_eq!(l[1]["from"], json!([7, 1]));
    assert_eq!(l[1]["inputRange"], "$F$3:$F$6");
    // The linked cell moved with its value.
    assert_eq!(e.cell(0, 6, 3).unwrap().display, "FALSE");
    // A column inserted before B moves the controls right and the references with them.
    e.structural(0, false, 0, 1, false).unwrap();
    let l = controls(&mut e);
    assert_eq!(l[0]["from"], json!([4, 2]));
    assert_eq!(l[0]["link"], "$E$7");
    assert_eq!(l[1]["inputRange"], "$G$3:$G$6");
    // The size is kept (move, don't size).
    assert_eq!(
        l[0]["to"][0].as_u64().unwrap() - l[0]["from"][0].as_u64().unwrap(),
        before[0]["to"][0].as_u64().unwrap() - before[0]["from"][0].as_u64().unwrap()
    );
    // Undo restores exactly, step by step.
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(controls(&mut e), before);
    e.redo().unwrap();
    assert_eq!(controls(&mut e)[0]["link"], "$D$7");
}

#[test]
fn deleting_the_linked_cell_unlinks_and_input_ranges_shrink() {
    let mut e = with_controls();
    let before = controls(&mut e);
    // Delete row 5 (the linked cell D5) and row 2 (inside the input range).
    e.structural(0, true, 4, 1, true).unwrap();
    e.structural(0, true, 1, 1, true).unwrap();
    let l = controls(&mut e);
    assert_eq!(l[0]["link"], Value::Null);
    assert_eq!(l[1]["inputRange"], "$F$1:$F$3");
    assert_eq!(l[1]["link"], "$H$1");
    // The check box (row 3) moved up one row; the combo box (row 6) two.
    assert_eq!(l[0]["from"], json!([1, 1]));
    assert_eq!(l[1]["from"], json!([3, 1]));
    // Deleting the column of the whole input range unlinks it too.
    e.structural(0, false, 5, 1, true).unwrap();
    assert_eq!(controls(&mut e)[1]["inputRange"], Value::Null);
    for _ in 0..3 {
        e.undo().unwrap();
    }
    assert_eq!(controls(&mut e), before);
}

#[test]
fn moved_controls_survive_saving_new_and_from_the_file() {
    let mut e = with_controls();
    e.structural(0, true, 0, 3, false).unwrap();
    // New controls: written fresh from the model.
    let mut f = reopen(&mut e);
    let l = controls(&mut f);
    assert_eq!(l[0]["from"], json!([5, 1]));
    assert_eq!(l[0]["link"], "$D$8");
    assert_eq!(l[1]["inputRange"], "$F$4:$F$7");
    assert_eq!(l[0]["isNew"], false);
    // Controls read from the file: their parts are patched in place.
    f.structural(0, false, 0, 2, false).unwrap();
    f.structural(0, true, 0, 1, true).unwrap();
    let moved = controls(&mut f);
    assert_eq!(moved[0]["from"], json!([4, 3]));
    assert_eq!(moved[0]["link"], "$F$7");
    let mut g = reopen(&mut f);
    let again = controls(&mut g);
    for (a, b) in again.iter().zip(&moved) {
        for k in [
            "from",
            "to",
            "fromOff",
            "toOff",
            "link",
            "inputRange",
            "kind",
        ] {
            assert_eq!(a[k], b[k], "{k}");
        }
    }
    // The VML shape anchor agrees with the controls entry.
    let bytes = f.save("xlsx", 0).unwrap();
    let wb = bo_xlsx::read_xlsx(&bytes).unwrap();
    let vml = wb
        .opaque
        .parts
        .iter()
        .find(|p| p.name.contains("vmlDrawing"))
        .unwrap();
    let text = String::from_utf8_lossy(&vml.data);
    assert!(text.contains("<x:Anchor>3, "), "{text}");
}

#[test]
fn sparklines_follow_rows_and_columns() {
    let mut e = engine();
    for (c, v) in ["1", "3", "2"].iter().enumerate() {
        e.set_input(0, 0, c as u32, v).unwrap();
        e.set_input(0, 1, c as u32, v).unwrap();
    }
    run(
        &mut e,
        json!({"cmd": "sparklineInsert", "sheet": 0, "type": "line", "data": "A1:C2", "location": {"r0": 0, "c0": 3, "r1": 1, "c1": 3}}),
    );
    let before = sparklines(&mut e);
    assert_eq!(
        before,
        [
            ("Sheet1!A1:C1".to_owned(), [0, 3]),
            ("Sheet1!A2:C2".to_owned(), [1, 3])
        ]
    );
    e.structural(0, true, 0, 1, false).unwrap();
    e.structural(0, false, 1, 1, false).unwrap();
    let moved = sparklines(&mut e);
    assert_eq!(
        moved,
        [
            ("Sheet1!A2:D2".to_owned(), [1, 4]),
            ("Sheet1!A3:D3".to_owned(), [2, 4])
        ]
    );
    // Saved and reopened in place.
    let mut f = reopen(&mut e);
    assert_eq!(sparklines(&mut f), moved);
    // Deleting a sparkline's row removes it; deleting data columns shrinks the ranges.
    e.structural(0, true, 1, 1, true).unwrap();
    e.structural(0, false, 0, 1, true).unwrap();
    assert_eq!(sparklines(&mut e), [("Sheet1!A2:C2".to_owned(), [1, 3])]);
    // Its data deleted entirely: the sparkline stays and draws nothing.
    e.structural(0, false, 0, 3, true).unwrap();
    assert_eq!(sparklines(&mut e), [(String::new(), [1, 0])]);
    for _ in 0..5 {
        e.undo().unwrap();
    }
    assert_eq!(sparklines(&mut e), before);
}

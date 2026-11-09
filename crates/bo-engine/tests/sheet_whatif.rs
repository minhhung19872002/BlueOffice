//! BlueSheet what-if analysis: Goal Seek through the command bridge.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

#[test]
fn goal_seek_finds_and_enters_the_input() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    // B1 = 2*A1 + 3; reach 11 by changing A1 (answer 4).
    e.set_inputs(0, &[(0, 0, "1".into()), (0, 1, "=2*A1+3".into())])
        .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "goalSeek", "sheet": 0, "target": "B1", "goal": 11, "changing": "A1"}),
    )
    .unwrap();
    assert_eq!(r["found"], true);
    assert!((r["input"].as_f64().unwrap() - 4.0).abs() < 1e-3);
    // Not applied yet.
    let v = |e: &mut SheetEngine, a1: &str| {
        let (row, col) = match a1 {
            "A1" => (0, 0),
            _ => (0, 1),
        };
        e.cell(0, row, col).unwrap().display.parse::<f64>().unwrap()
    };
    assert!((v(&mut e, "A1") - 1.0).abs() < 1e-12);
    exec(
        &mut e,
        &json!({"cmd": "goalSeek", "sheet": 0, "target": "$B$1", "goal": 11, "changing": "$A$1", "apply": true}),
    )
    .unwrap();
    assert!((v(&mut e, "A1") - 4.0).abs() < 1e-3);
    assert!((v(&mut e, "B1") - 11.0).abs() < 1e-3);
    // One undo step restores the input.
    e.undo().unwrap();
    assert!((v(&mut e, "A1") - 1.0).abs() < 1e-12);
    // A constant target is refused.
    assert!(exec(
        &mut e,
        &json!({"cmd": "goalSeek", "sheet": 0, "target": "A1", "goal": 1, "changing": "B1"}),
    )
    .is_err());
}

#[test]
fn data_table_fills_results_and_survives_saving() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    // A1 = rate input; B2 = A1*100 (formula row); A3:A5 = rates; results B3:B5.
    e.set_inputs(
        0,
        &[
            (0, 0, "1".into()),
            (1, 1, "=A1*100".into()),
            (2, 0, "2".into()),
            (3, 0, "3".into()),
            (4, 0, "5".into()),
        ],
    )
    .unwrap();
    exec(
        &mut e,
        &json!({"cmd": "dataTable", "sheet": 0, "range": {"r0": 1, "c0": 0, "r1": 4, "c1": 1}, "colInput": "A1"}),
    )
    .unwrap();
    let shown = |e: &SheetEngine, row: u32, col: u32| e.cell(0, row, col).unwrap().display;
    assert_eq!(shown(&e, 2, 1), "200");
    assert_eq!(shown(&e, 3, 1), "300");
    assert_eq!(shown(&e, 4, 1), "500");
    assert_eq!(shown(&e, 0, 0), "1");
    // Changing the formula recalculates the table.
    e.set_inputs(0, &[(1, 1, "=A1*10".into())]).unwrap();
    assert_eq!(shown(&e, 4, 1), "50");
    // Saved as t="dataTable" and read back with its results.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(shown(&f, 3, 1), "30");
    f.set_inputs(0, &[(1, 1, "=A1+1".into())]).unwrap();
    assert_eq!(shown(&f, 3, 1), "4");
    // A table needs an input cell.
    assert!(exec(
        &mut e,
        &json!({"cmd": "dataTable", "sheet": 0, "range": {"r0": 1, "c0": 0, "r1": 4, "c1": 1}}),
    )
    .is_err());
}

#[test]
fn solver_optimises_and_applies() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    // Maximise 3x + 5y s.t. x ≤ 4, 2y ≤ 12, 3x + 2y ≤ 18 (x, y in A1:B1) → 36 at (2, 6).
    e.set_inputs(
        0,
        &[
            (0, 2, "=3*A1+5*B1".into()),
            (1, 2, "=2*B1".into()),
            (2, 2, "=3*A1+2*B1".into()),
            (3, 2, "18".into()),
        ],
    )
    .unwrap();
    let spec = json!({
        "cmd": "solve", "sheet": 0, "objective": "C1", "goal": "max", "variables": ["A1:B1"],
        "constraints": [
            {"lhs": "A1", "relation": "le", "rhs": "4"},
            {"lhs": "C2", "relation": "le", "rhs": "12"},
            {"lhs": "C3", "relation": "le", "rhs": "C4"}
        ]
    });
    let r = exec(&mut e, &spec).unwrap();
    assert_eq!(r["status"], "optimal");
    assert!((r["objective"].as_f64().unwrap() - 36.0).abs() < 1e-9);
    let mut apply = spec.clone();
    apply["apply"] = json!(true);
    exec(&mut e, &apply).unwrap();
    assert_eq!(e.cell(0, 0, 0).unwrap().display, "2");
    assert_eq!(e.cell(0, 0, 1).unwrap().display, "6");
    assert_eq!(e.cell(0, 0, 2).unwrap().display, "36");
    e.undo().unwrap();
    assert_eq!(e.cell(0, 0, 2).unwrap().display, "0");
    // A value goal with an integer constraint.
    let r = exec(
        &mut e,
        &json!({"cmd": "solve", "sheet": 0, "objective": "C2", "goal": {"value": 7},
                "variables": ["B1"], "constraints": [{"lhs": "B1", "relation": "int"}]}),
    )
    .unwrap();
    assert_eq!(r["status"], "infeasible");
}

#[test]
fn table_slicer_commands() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "Vùng".into()),
            (0, 1, "Số".into()),
            (1, 0, "Bắc".into()),
            (1, 1, "1".into()),
            (2, 0, "Nam".into()),
            (2, 1, "2".into()),
            (3, 0, "Bắc".into()),
            (3, 1, "3".into()),
        ],
    )
    .unwrap();
    // A slicer needs a table.
    assert!(exec(
        &mut e,
        &json!({"cmd": "insertSlicer", "sheet": 0, "row": 1, "col": 0})
    )
    .is_err());
    exec(
        &mut e,
        &json!({"cmd": "createTable", "sheet": 0, "range": {"r0": 0, "c0": 0, "r1": 3, "c1": 1}}),
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "insertSlicer", "sheet": 0, "row": 1, "col": 0}),
    )
    .unwrap();
    assert_eq!(r["index"], 0);
    let list = exec(&mut e, &json!({"cmd": "slicers", "sheet": 0})).unwrap();
    assert_eq!(list[0]["caption"], "Vùng");
    assert_eq!(list[0]["items"].as_array().unwrap().len(), 2);
    exec(
        &mut e,
        &json!({"cmd": "slicerSelect", "sheet": 0, "index": 0, "items": ["Nam"]}),
    )
    .unwrap();
    let list = exec(&mut e, &json!({"cmd": "slicers", "sheet": 0})).unwrap();
    assert_eq!(list[0]["items"][0]["selected"], false);
    assert_eq!(list[0]["items"][1]["selected"], true);
    // Undo restores the unfiltered table; delete removes the new slicer.
    e.undo().unwrap();
    let list = exec(&mut e, &json!({"cmd": "slicers", "sheet": 0})).unwrap();
    assert_eq!(list[0]["items"][0]["selected"], true);
    exec(
        &mut e,
        &json!({"cmd": "deleteSlicer", "sheet": 0, "index": 0}),
    )
    .unwrap();
    let list = exec(&mut e, &json!({"cmd": "slicers", "sheet": 0})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

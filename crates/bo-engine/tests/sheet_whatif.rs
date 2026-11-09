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

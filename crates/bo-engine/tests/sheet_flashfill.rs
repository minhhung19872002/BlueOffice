//! BlueSheet Flash Fill through the command bridge.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn text(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().display
}

#[test]
fn flash_fill_learns_initials_below_a_header() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "Họ và tên".into()),
            (0, 1, "Viết tắt".into()),
            (1, 0, "Nguyễn Văn An".into()),
            (1, 1, "NVA".into()),
            (2, 0, "Trần Thị Bích".into()),
            (3, 0, "Lê Minh Tuấn".into()),
        ],
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "flashFill", "sheet": 0, "row": 2, "col": 1}),
    )
    .unwrap();
    assert_eq!(r, json!({"found": true, "filled": 2}));
    assert_eq!(text(&e, 2, 1), "TTB");
    assert_eq!(text(&e, 3, 1), "LMT");
    // The header and the example are untouched.
    assert_eq!(text(&e, 0, 1), "Viết tắt");
    assert_eq!(text(&e, 1, 1), "NVA");
    // One undo step.
    e.undo().unwrap();
    assert_eq!(text(&e, 2, 1), "");
    assert_eq!(text(&e, 3, 1), "");
}

#[test]
fn flash_fill_combines_two_columns() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "John".into()),
            (0, 1, "Smith".into()),
            (0, 2, "john.smith@example.com".into()),
            (1, 0, "Mary".into()),
            (1, 1, "Jones".into()),
            (1, 2, "mary.jones@example.com".into()),
            (2, 0, "Peter".into()),
            (2, 1, "Brown".into()),
        ],
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "flashFill", "sheet": 0, "row": 2, "col": 2}),
    )
    .unwrap();
    assert_eq!(r["filled"], 1);
    assert_eq!(text(&e, 2, 2), "peter.brown@example.com");
}

#[test]
fn flash_fill_without_a_pattern_changes_nothing() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "a".into()),
            (0, 1, "1".into()),
            (1, 0, "b".into()),
            (1, 1, "2".into()),
            (2, 0, "c".into()),
        ],
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "flashFill", "sheet": 0, "row": 2, "col": 1}),
    )
    .unwrap();
    assert_eq!(r, json!({"found": false, "filled": 0}));
    assert_eq!(text(&e, 2, 1), "");
    // Nothing to undo from Flash Fill: undo takes back the typing instead.
    assert!(e.undo().unwrap());
    assert_eq!(text(&e, 0, 0), "");
}

#[test]
fn flash_fill_suggests_while_the_second_example_is_typed() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "Nguyễn Văn An".into()),
            (0, 1, "Nguyễn".into()),
            (1, 0, "Trần Thị Bích".into()),
            (2, 0, "Lê Minh Tuấn".into()),
            (3, 0, "Phạm Hùng".into()),
        ],
    )
    .unwrap();
    let preview = |e: &mut SheetEngine, text: &str| {
        exec(
            e,
            &json!({"cmd": "flashFillPreview", "sheet": 0, "row": 1, "col": 1, "text": text}),
        )
        .unwrap()["cells"]
            .clone()
    };
    // "Tr" begins the family name of row 2: the rest of the column is suggested.
    assert_eq!(preview(&mut e, "Tr"), json!([[2, "Lê"], [3, "Phạm"]]));
    assert_eq!(preview(&mut e, "trần"), json!([[2, "Lê"], [3, "Phạm"]]));
    // Text that does not follow the pattern, or a formula: no suggestion.
    assert_eq!(preview(&mut e, "Bích"), json!([]));
    assert_eq!(preview(&mut e, "=A2"), json!([]));
    // Nothing was written and nothing can be undone by the preview.
    assert_eq!(text(&e, 2, 1), "");
    assert!(e.undo().unwrap());
    assert_eq!(text(&e, 0, 0), "");
}

#[test]
fn flash_fill_writes_month_names() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "'15/03/2024".into()),
            (0, 1, "March".into()),
            (1, 0, "'02/11/2023".into()),
        ],
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "flashFill", "sheet": 0, "row": 1, "col": 1}),
    )
    .unwrap();
    assert_eq!(r["filled"], 1);
    assert_eq!(text(&e, 1, 1), "November");
}

//! Paste Special operations (add / subtract / multiply / divide), skip blanks and transpose,
//! as measured on the reference through its object model (docs/ui/REPORT-sheet.md round 36).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use bo_model::sheet::{CellError, CellValue};
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, cmd: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&cmd.to_string()).unwrap()).unwrap_or(Value::Null)
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn value(e: &SheetEngine, a1: &str) -> CellValue {
    e.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .map_or(CellValue::Empty, |c| c.value.clone())
}

fn formula(e: &SheetEngine, a1: &str) -> String {
    e.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.clone())
        .unwrap_or_default()
}

/// Column A = source, column B = destination, one case per row.
fn engine(cases: &[(&str, &str)]) -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("a.xlsx").unwrap();
    let mut inputs = vec![(0, 5, "4".to_owned())];
    for (i, (dest, src)) in cases.iter().enumerate() {
        let r = u32::try_from(i).unwrap();
        if !src.is_empty() {
            inputs.push((r, 0, (*src).to_owned()));
        }
        if !dest.is_empty() {
            inputs.push((r, 1, (*dest).to_owned()));
        }
    }
    e.set_inputs(0, &inputs).unwrap();
    e
}

const CASES: &[(&str, &str)] = &[
    ("5", "3"),
    ("5", "x"),
    ("y", "3"),
    ("", "3"),
    ("5", ""),
    ("=1+1", "3"),
    ("5", "=$F$1*2"),
    ("=1+1", "=$F$1*2"),
    ("", ""),
];

fn paste(e: &mut SheetEngine, mode: &str, op: &str, skip: bool) {
    let n = u32::try_from(CASES.len()).unwrap() - 1;
    exec(
        e,
        &json!({"cmd": "copy", "sheet": 0, "range": rng(0, 0, n, 0)}),
    );
    exec(
        e,
        &json!({"cmd": "paste", "sheet": 0, "row": 0, "col": 1, "mode": mode,
                "operation": op, "skipBlanks": skip}),
    );
}

#[test]
fn add_all_as_the_reference() {
    let mut e = engine(CASES);
    paste(&mut e, "all", "add", false);
    assert_eq!(value(&e, "B1"), CellValue::Number(8.0));
    assert_eq!(value(&e, "B2"), CellValue::Number(5.0));
    assert_eq!(value(&e, "B3"), CellValue::Text("y".into()));
    assert_eq!(value(&e, "B4"), CellValue::Number(3.0));
    assert_eq!(value(&e, "B5"), CellValue::Number(5.0));
    assert_eq!(formula(&e, "B6"), "(1+1)+3");
    assert_eq!(value(&e, "B6"), CellValue::Number(5.0));
    assert_eq!(formula(&e, "B7"), "5+($F$1*2)");
    assert_eq!(value(&e, "B7"), CellValue::Number(13.0));
    assert_eq!(formula(&e, "B8"), "(1+1)+($F$1*2)");
    assert_eq!(value(&e, "B8"), CellValue::Number(10.0));
    assert_eq!(value(&e, "B9"), CellValue::Empty);
    // One undo step restores every destination.
    e.undo().unwrap();
    assert_eq!(value(&e, "B1"), CellValue::Number(5.0));
    assert_eq!(formula(&e, "B6"), "1+1");
}

#[test]
fn values_use_the_source_results() {
    let mut e = engine(CASES);
    paste(&mut e, "values", "add", false);
    assert_eq!(formula(&e, "B7"), "");
    assert_eq!(value(&e, "B7"), CellValue::Number(13.0));
    assert_eq!(formula(&e, "B8"), "(1+1)+8");
    assert_eq!(value(&e, "B8"), CellValue::Number(10.0));
}

#[test]
fn multiply_and_divide_treat_blanks_as_zero() {
    let mut e = engine(CASES);
    paste(&mut e, "all", "multiply", false);
    assert_eq!(value(&e, "B1"), CellValue::Number(15.0));
    assert_eq!(value(&e, "B4"), CellValue::Number(0.0));
    assert_eq!(value(&e, "B5"), CellValue::Number(0.0));
    let mut e = engine(CASES);
    paste(&mut e, "all", "divide", false);
    assert_eq!(value(&e, "B5"), CellValue::Error(CellError::Div0));
    let mut e = engine(CASES);
    // Skip blanks keeps the destination of a blank source.
    paste(&mut e, "all", "divide", true);
    assert_eq!(value(&e, "B5"), CellValue::Number(5.0));
    let mut e = engine(CASES);
    paste(&mut e, "all", "subtract", false);
    assert_eq!(value(&e, "B4"), CellValue::Number(-3.0));
    assert_eq!(formula(&e, "B6"), "(1+1)-3");
}

#[test]
fn transpose_with_an_operation() {
    let mut e = SheetEngine::new();
    e.new_workbook("a.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "1".into()),
            (0, 1, "2".into()),
            (0, 3, "10".into()),
            (1, 3, "20".into()),
        ],
    )
    .unwrap();
    exec(
        &mut e,
        &json!({"cmd": "copy", "sheet": 0, "range": rng(0, 0, 0, 1)}),
    );
    exec(
        &mut e,
        &json!({"cmd": "paste", "sheet": 0, "row": 0, "col": 3, "operation": "add", "transpose": true}),
    );
    assert_eq!(value(&e, "D1"), CellValue::Number(11.0));
    assert_eq!(value(&e, "D2"), CellValue::Number(22.0));
    assert_eq!(value(&e, "E1"), CellValue::Empty);
}

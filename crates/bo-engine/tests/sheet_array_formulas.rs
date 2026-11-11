//! Legacy array formulas (ECMA-376 Part 1, §18.17.2.4; Ctrl+Shift+Enter) next to dynamic
//! arrays (SHEET-FORMULA-001). Reference behaviour (public documentation of array formulas
//! in spreadsheet applications): the formula is entered once for the selected range and
//! shown in braces in every cell of it; its result fills the range exactly (one value
//! repeats, a one-row / one-column result repeats, the rest is `#N/A`); changing or
//! clearing only some cells of the range is refused ("part of an array"); the whole range
//! can be cleared; a single cell of the array re-enters the whole array; undo restores it.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use bo_model::sheet::{CellError, CellValue, FormulaKind};
use serde_json::{json, Value};

fn value(e: &SheetEngine, a1: &str) -> CellValue {
    e.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .map_or(CellValue::Empty, |c| c.value.clone())
}

fn exec(e: &mut SheetEngine, cmd: &Value) -> Result<Value, String> {
    e.exec_json(&cmd.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
        .map_err(|err| err.to_string())
}

fn engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("a.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "1".into()),
            (1, 0, "2".into()),
            (2, 0, "3".into()),
            (0, 1, "10".into()),
            (1, 1, "20".into()),
            (2, 1, "30".into()),
        ],
    )
    .unwrap();
    e
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

#[test]
fn entered_over_a_range_fills_it_exactly() {
    let mut e = engine();
    // D1:D4 = {=A1:A3*B1:B3}: 10, 40, 90 and #N/A below the result.
    let res = exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(0, 3, 3, 3), "formula": "=A1:A3*B1:B3"}),
    )
    .unwrap();
    assert_eq!(res["range"], rng(0, 3, 3, 3));
    assert_eq!(value(&e, "D1"), CellValue::Number(10.0));
    assert_eq!(value(&e, "D3"), CellValue::Number(90.0));
    assert_eq!(value(&e, "D4"), CellValue::Error(CellError::NA));
    let ws = &e.workbook().unwrap().sheets[0];
    let f = ws.cell_a1("D1").unwrap().formula.clone().unwrap();
    assert_eq!(
        (f.text.as_str(), f.kind, f.dynamic),
        (
            "A1:A3*B1:B3",
            FormulaKind::Array {
                range: "D1:D4".into()
            },
            false
        )
    );
    // Every cell of the range shows the formula in braces.
    let d = e.cell(0, 2, 3).unwrap();
    assert_eq!(d.input, "{=A1:A3*B1:B3}");
    assert_eq!(d.array.as_deref(), Some("D1:D4"));
    // A scalar entered over E1:E3 repeats.
    exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(0, 4, 2, 4), "formula": "=SUM(A1:A3*B1:B3)"}),
    )
    .unwrap();
    assert_eq!(value(&e, "E1"), CellValue::Number(140.0));
    assert_eq!(value(&e, "E3"), CellValue::Number(140.0));
    // An input change recalculates the array.
    e.set_inputs(0, &[(0, 0, "2".into())]).unwrap();
    assert_eq!(value(&e, "D1"), CellValue::Number(20.0));
    assert_eq!(value(&e, "E2"), CellValue::Number(150.0));
}

#[test]
fn part_of_an_array_cannot_change() {
    let mut e = engine();
    exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(0, 3, 2, 3), "formula": "=A1:A3*2"}),
    )
    .unwrap();
    // Typing into one cell, or clearing two of three, is refused.
    let typed = e.set_inputs(0, &[(1, 3, "5".into())]).unwrap_err();
    assert!(
        typed.to_string().contains(bo_engine::sheet::ARRAY_PART),
        "{typed}"
    );
    let cleared = e
        .set_inputs(0, &[(0, 3, String::new()), (1, 3, String::new())])
        .unwrap_err();
    assert!(cleared.to_string().contains(bo_engine::sheet::ARRAY_PART));
    assert_eq!(value(&e, "D2"), CellValue::Number(4.0));
    // An array formula over a range cutting across it is refused too.
    let across = exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(2, 3, 3, 3), "formula": "=1"}),
    )
    .unwrap_err();
    assert!(across.contains(bo_engine::sheet::ARRAY_PART));
    // A single cell of the array re-enters the whole array.
    let res = exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(1, 3, 1, 3), "formula": "{=A1:A3*3}"}),
    )
    .unwrap();
    assert_eq!(res["range"], rng(0, 3, 2, 3));
    assert_eq!(value(&e, "D3"), CellValue::Number(9.0));
    // Undo goes back to the previous formula.
    e.undo().unwrap();
    assert_eq!(value(&e, "D3"), CellValue::Number(6.0));
    // Clearing the whole range removes the array.
    e.set_inputs(
        0,
        &[
            (0, 3, String::new()),
            (1, 3, String::new()),
            (2, 3, String::new()),
        ],
    )
    .unwrap();
    assert_eq!(value(&e, "D1"), CellValue::Empty);
    assert_eq!(value(&e, "D2"), CellValue::Empty);
    e.set_inputs(0, &[(1, 3, "5".into())]).unwrap();
}

#[test]
fn legacy_and_dynamic_arrays_survive_saving() {
    let mut e = engine();
    exec(
        &mut e,
        &json!({"cmd": "setArrayFormula", "sheet": 0, "range": rng(0, 3, 3, 3), "formula": "=A1:A3*2"}),
    )
    .unwrap();
    // Typed normally, the same formula spills (a dynamic array).
    e.set_inputs(0, &[(0, 5, "=A1:A3*2".into())]).unwrap();
    assert_eq!(value(&e, "F3"), CellValue::Number(6.0));
    let bytes = e.save("xlsx", 0).unwrap();
    let mut again = SheetEngine::new();
    again.open(&bytes, "a.xlsx").unwrap();
    let ws = &again.workbook().unwrap().sheets[0];
    let f = |a1: &str| ws.cell_a1(a1).unwrap().formula.clone().unwrap();
    assert!(!f("D1").dynamic);
    assert!(f("F1").dynamic);
    assert_eq!(value(&again, "D4"), CellValue::Error(CellError::NA));
    assert_eq!(value(&again, "F3"), CellValue::Number(6.0));
    // After a full recalculation the legacy array still fills its range only.
    exec(&mut again, &json!({"cmd": "calculate"})).unwrap();
    assert_eq!(value(&again, "D4"), CellValue::Error(CellError::NA));
    assert_eq!(value(&again, "D3"), CellValue::Number(6.0));
}

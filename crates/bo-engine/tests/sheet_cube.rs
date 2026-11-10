//! Cube functions and RTD (ECMA-376 Part 1, §18.17.7) in a workbook file (SHEET-FN-CUBE):
//! BlueSheet has no OLAP connection, so the formula and the result saved in the file are
//! kept through opening, editing, recalculating and saving; dependents use the saved result.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use bo_model::sheet::{Cell, CellError, CellValue, Workbook, Worksheet};
use serde_json::json;

const CUBE: &str = r#"CUBEVALUE("Sales","[Measures].[Amount]","[Date].[2025]")"#;
const MEMBER: &str = r#"CUBEMEMBER("Sales","[Product].[All]")"#;

fn file() -> Vec<u8> {
    let mut wb = Workbook::new();
    wb.sheets.clear();
    let mut ws = Worksheet::new("Sheet1");
    ws.set_cell(
        0,
        0,
        Cell::new(CellValue::Number(1250.5)).with_formula(CUBE),
    );
    ws.set_cell(
        1,
        0,
        Cell::new(CellValue::Text("All".into())).with_formula(MEMBER),
    );
    ws.set_cell(
        0,
        1,
        Cell::new(CellValue::Number(2501.0)).with_formula("A1*2"),
    );
    ws.set_cell(0, 2, Cell::new(CellValue::Number(1.0)));
    wb.sheets.push(ws);
    bo_xlsx::write_xlsx(&wb).unwrap()
}

fn value(e: &SheetEngine, row: u32, col: u32) -> CellValue {
    e.workbook().unwrap().sheets[0]
        .cell(row, col)
        .map_or(CellValue::Empty, |c| c.value.clone())
}

fn formula(e: &SheetEngine, row: u32, col: u32) -> String {
    e.workbook().unwrap().sheets[0]
        .cell(row, col)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.clone())
        .unwrap_or_default()
}

fn check_kept(e: &SheetEngine) {
    assert_eq!(formula(e, 0, 0), CUBE);
    assert_eq!(value(e, 0, 0), CellValue::Number(1250.5));
    assert_eq!(formula(e, 1, 0), MEMBER);
    assert_eq!(value(e, 1, 0), CellValue::Text("All".into()));
}

#[test]
fn cube_formulas_keep_formula_and_saved_result() {
    let mut e = SheetEngine::new();
    e.open(&file(), "cube.xlsx").unwrap();
    check_kept(&e);

    // Editing cells that depend on the cube cell uses its saved result.
    e.set_inputs(0, &[(0, 2, "5".into()), (0, 3, "=A1+C1".into())])
        .unwrap();
    check_kept(&e);
    assert_eq!(value(&e, 0, 3), CellValue::Number(1255.5));
    assert_eq!(value(&e, 0, 1), CellValue::Number(2501.0));

    // A full recalculation does not turn it into an error either.
    e.exec_json(&json!({"cmd": "calculate"}).to_string())
        .unwrap();
    check_kept(&e);
    assert_eq!(value(&e, 0, 1), CellValue::Number(2501.0));

    // Saved and opened again: formula and result are still there.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut again = SheetEngine::new();
    again.open(&bytes, "cube.xlsx").unwrap();
    check_kept(&again);
    assert_eq!(value(&again, 0, 3), CellValue::Number(1255.5));
}

#[test]
fn a_cube_formula_typed_here_has_no_connection() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(0, &[(0, 0, format!("={CUBE}"))]).unwrap();
    assert_eq!(formula(&e, 0, 0), CUBE);
    // No workbook connection of that name: #NAME?.
    assert_eq!(value(&e, 0, 0), CellValue::Error(CellError::Name));
}

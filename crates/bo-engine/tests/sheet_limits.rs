//! BlueSheet at the limits of the grid (SHEET-PERF-001): the last cell
//! XFD1048576 (1 048 576 rows × 16 384 columns, ECMA-376 Part 1, §18.3.1.4 `c/@r`
//! and §18.3.1.73 `row/@r`) in references, ranges, Go To, selection regions,
//! rendering and row / column inserts at the edge; a 10-million-cell workbook
//! opens (ignored in debug builds: run with `--release -- --ignored`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::{SheetEngine, ViewRequest};
use bo_model::sheet::{Cell, CellValue, Workbook, Worksheet, MAX_COLS, MAX_ROWS};
use serde_json::{json, Value};

const LAST_ROW: u32 = MAX_ROWS - 1;
const LAST_COL: u32 = MAX_COLS - 1;

fn engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.set_locale("en-US");
    e.new_workbook("edge.xlsx").unwrap();
    e
}

fn run(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn display(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().display
}

#[test]
fn last_cell_in_references_and_ranges() {
    let mut e = engine();
    e.set_input(0, LAST_ROW, LAST_COL, "5").unwrap();
    e.set_input(0, LAST_ROW, 0, "2").unwrap();
    e.set_input(0, 0, LAST_COL, "1").unwrap();
    assert_eq!(e.cell(0, LAST_ROW, LAST_COL).unwrap().address, "XFD1048576");
    e.set_input(0, 0, 0, "=XFD1048576*2").unwrap();
    e.set_input(0, 1, 0, "=SUM(XFD1:XFD1048576)").unwrap();
    e.set_input(0, 2, 0, "=SUM(A1048576:XFD1048576)").unwrap();
    e.set_input(0, 3, 0, "=COUNT(XFD:XFD)").unwrap();
    e.set_input(0, 4, 0, "=ROW(XFD1048576)+COLUMN(XFD1048576)")
        .unwrap();
    assert_eq!(display(&e, 0, 0), "10");
    assert_eq!(display(&e, 1, 0), "6");
    assert_eq!(display(&e, 2, 0), "7");
    assert_eq!(display(&e, 3, 0), "2");
    assert_eq!(display(&e, 4, 0), (1_048_576 + 16_384).to_string());
    // Beyond the grid is not a reference.
    e.set_input(0, 5, 0, "=XFE1").unwrap();
    assert!(display(&e, 5, 0).starts_with('#'), "{}", display(&e, 5, 0));
    // Go To the last cell and the whole sheet.
    let r = run(
        &mut e,
        &json!({ "cmd": "resolve", "sheet": 0, "text": "XFD1048576" }),
    );
    assert_eq!(
        r["range"],
        json!({ "r0": LAST_ROW, "c0": LAST_COL, "r1": LAST_ROW, "c1": LAST_COL })
    );
    let r = run(
        &mut e,
        &json!({ "cmd": "resolve", "sheet": 0, "text": "A1:XFD1048576" }),
    );
    assert_eq!(r["range"]["r1"], json!(LAST_ROW));
    assert_eq!(r["range"]["c1"], json!(LAST_COL));
}

#[test]
fn region_and_rendering_at_the_corner() {
    let mut e = engine();
    let cells: Vec<(u32, u32, String)> = (LAST_ROW - 2..=LAST_ROW)
        .flat_map(|r| (LAST_COL - 1..=LAST_COL).map(move |c| (r, c, format!("{r}-{c}"))))
        .collect();
    e.set_inputs(0, &cells).unwrap();
    // The current region (Ctrl+A) of the corner cell stops at the grid's edge.
    let r = run(
        &mut e,
        &json!({ "cmd": "region", "sheet": 0,
            "range": { "r0": LAST_ROW, "c0": LAST_COL, "r1": LAST_ROW, "c1": LAST_COL } }),
    );
    assert_eq!(
        r,
        json!({ "r0": LAST_ROW - 2, "c0": LAST_COL - 1, "r1": LAST_ROW, "c1": LAST_COL })
    );
    // Sorting it (a range ending on the last row and column).
    run(
        &mut e,
        &json!({ "cmd": "sort", "sheet": 0,
            "range": { "r0": LAST_ROW, "c0": LAST_COL, "r1": LAST_ROW, "c1": LAST_COL },
            "keys": [{ "col": LAST_COL, "descending": true }] }),
    );
    assert_eq!(
        display(&e, LAST_ROW - 2, LAST_COL),
        format!("{LAST_ROW}-{LAST_COL}")
    );
    // Scrolled to (and past) the end of the grid.
    for (x, y) in [(1e9, 1e9), (0.0, 1e12)] {
        e.render(&ViewRequest {
            sheet: 0,
            scroll_x: x,
            scroll_y: y,
            width: 800.0,
            height: 600.0,
        })
        .unwrap();
    }
}

#[test]
fn insert_at_the_edge() {
    let mut e = engine();
    e.set_input(0, LAST_ROW, 0, "last row").unwrap();
    e.set_input(0, 0, LAST_COL, "last col").unwrap();
    e.set_input(0, LAST_ROW - 1, 0, "row before").unwrap();
    e.set_input(0, 0, 0, "=A1048575").unwrap();
    // Inserting a row above the last one pushes the last row off the grid.
    e.structural(0, true, LAST_ROW, 1, false).unwrap();
    assert_eq!(display(&e, LAST_ROW, 0), "");
    assert_eq!(display(&e, LAST_ROW - 1, 0), "row before");
    // Before the second-last row: "row before" moves to the last row, the reference follows.
    e.structural(0, true, LAST_ROW - 1, 1, false).unwrap();
    assert_eq!(display(&e, LAST_ROW, 0), "row before");
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "=A1048576");
    // Columns: the last column is pushed off.
    e.structural(0, false, LAST_COL, 1, false).unwrap();
    assert_eq!(display(&e, 0, LAST_COL), "");
    // Deleting the last row / column at the edge.
    e.structural(0, true, LAST_ROW, 1, true).unwrap();
    assert_eq!(display(&e, LAST_ROW, 0), "");
    e.structural(0, false, LAST_COL, 1, true).unwrap();
    assert!(e.undo().unwrap());
}

/// A generated 1 048 576 × 10 workbook (10 485 760 cells, numbers and shared
/// strings) saved as .xlsx and opened again.
#[test]
#[ignore = "10 million cells: run in release (`cargo test --release -p bo-engine --test sheet_limits -- --ignored`)"]
fn ten_million_cell_workbook_opens() {
    let cells = (0..MAX_ROWS).flat_map(|r| {
        (0..10u32).map(move |c| {
            let v = if c < 3 {
                CellValue::Text(format!("T{}", (r + c) % 997))
            } else {
                CellValue::Number(f64::from(r % 1000 + c))
            };
            ((r, c), Cell::new(v))
        })
    });
    let mut wb = Workbook::new();
    wb.sheets = vec![Worksheet {
        name: "Sheet1".into(),
        cells: cells.collect(),
        ..Worksheet::default()
    }];
    let bytes = bo_xlsx::write_xlsx(&wb).unwrap();
    drop(wb);
    let mut e = SheetEngine::new();
    e.set_locale("en-US");
    let t = std::time::Instant::now();
    e.open(&bytes, "ten-million.xlsx").unwrap();
    println!(
        "opened 10 485 760 cells in {:.2} s",
        t.elapsed().as_secs_f64()
    );
    assert_eq!(e.workbook().unwrap().sheets[0].cells.len(), 10_485_760);
    assert_eq!(display(&e, LAST_ROW, 9), (LAST_ROW % 1000 + 9).to_string());
    assert_eq!(display(&e, LAST_ROW, 0), format!("T{}", LAST_ROW % 997));
    // Editing still recalculates.
    e.set_input(0, 0, 10, "=SUM(D1:D1048576)").unwrap();
    let want: u64 = (0..MAX_ROWS).map(|r| u64::from(r % 1000 + 3)).sum();
    assert_eq!(display(&e, 0, 10).replace(',', ""), want.to_string());
}

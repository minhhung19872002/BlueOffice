//! BlueSheet Quick Analysis and recommended charts (CORE-DESIGN-001): the read-only
//! `quickAnalysis` / `recommendCharts` commands, and a Quick Analysis choice applied through
//! the normal commands in one `batch` (one undo step) that saves and reopens.
//!
//! With `BO_QUICK_OUT=<dir>` the saved workbook is kept there (for OpenXmlValidator).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use serde_json::{json, Value};
use std::path::PathBuf;

fn exec(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn book(rows: &[&[&str]]) -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            e.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    e
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn sales() -> SheetEngine {
    book(&[
        &["Tỉnh", "Quý 1", "Quý 2"],
        &["Hà Nội", "10", "12"],
        &["Huế", "5", "4"],
        &["Đà Nẵng", "7", "9"],
    ])
}

fn kinds(r: &Value) -> Vec<String> {
    r["charts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn quick_analysis_reports_the_data_shape() {
    let mut e = sales();
    let q = exec(
        &mut e,
        &json!({ "cmd": "quickAnalysis", "sheet": 0, "range": rng(0, 0, 3, 2) }),
    );
    assert_eq!(q["header"], true);
    assert_eq!(q["labelCols"], 1);
    assert_eq!(q["numericCols"], json!([1, 2]));
    assert_eq!(q["numericRows"], json!([1, 2, 3]));
    assert_eq!(q["belowFree"], true);
    assert_eq!(q["rightFree"], true);
    e.set_input(0, 4, 1, "x").unwrap();
    let q = exec(
        &mut e,
        &json!({ "cmd": "quickAnalysis", "sheet": 0, "range": rng(0, 0, 3, 2) }),
    );
    assert_eq!(q["belowFree"], false);
}

#[test]
fn recommended_charts_follow_the_data() {
    // Categories with two series: columns first, with previews.
    let mut e = sales();
    let r = exec(
        &mut e,
        &json!({ "cmd": "recommendCharts", "sheet": 0, "range": rng(0, 0, 3, 2), "width": 120, "height": 80 }),
    );
    let k = kinds(&r);
    assert_eq!(k[0], "column");
    assert!(k.contains(&"stackedColumn".to_owned()));
    for c in r["charts"].as_array().unwrap() {
        let svg = c["svg"].as_str().unwrap();
        assert!(svg.starts_with("<svg") && svg.contains("120"), "{svg}");
    }
    // One series of positive parts: a pie.
    let mut e = book(&[&["Loại", "Số"], &["A", "3"], &["B", "5"], &["C", "2"]]);
    let r = exec(
        &mut e,
        &json!({ "cmd": "recommendCharts", "sheet": 0, "range": rng(0, 0, 3, 1) }),
    );
    assert_eq!(kinds(&r)[0], "pie");
    // Dates as labels: a line, plotted on the dates (categories, one series).
    let mut e = book(&[
        &["Ngày", "Giá"],
        &["2025-01-01", "3"],
        &["2025-02-01", "5"],
        &["2025-03-01", "4"],
    ]);
    let r = exec(
        &mut e,
        &json!({ "cmd": "recommendCharts", "sheet": 0, "range": rng(0, 0, 3, 1) }),
    );
    assert_eq!(kinds(&r)[0], "line", "{}", r["shape"]);
    let c = exec(
        &mut e,
        &json!({ "cmd": "insertChart", "sheet": 0, "range": rng(0, 0, 3, 1), "kind": "line" }),
    );
    let charts = exec(&mut e, &json!({ "cmd": "charts", "sheet": 0 }));
    assert_eq!(c["index"], 0);
    assert_eq!(charts[0]["series"], 1, "{charts}");
    // Two numeric columns: a scatter.
    let mut e = book(&[&["X", "Y"], &["1", "3"], &["2", "1"], &["5", "4"]]);
    let r = exec(
        &mut e,
        &json!({ "cmd": "recommendCharts", "sheet": 0, "range": rng(0, 0, 3, 1) }),
    );
    assert_eq!(kinds(&r)[0], "scatter");
    // Many categories: horizontal bars.
    let rows: Vec<Vec<String>> = (0..14)
        .map(|i| vec![format!("Mục {i}"), format!("{}", i * 3 + 1)])
        .collect();
    let refs: Vec<Vec<&str>> = rows
        .iter()
        .map(|r| r.iter().map(String::as_str).collect())
        .collect();
    let slices: Vec<&[&str]> = refs.iter().map(Vec::as_slice).collect();
    let mut e = book(&slices);
    let r = exec(
        &mut e,
        &json!({ "cmd": "recommendCharts", "sheet": 0, "range": rng(0, 0, 13, 1) }),
    );
    assert_eq!(kinds(&r)[0], "bar");
}

#[test]
fn quick_analysis_choices_are_one_undo_step_and_round_trip() {
    let mut e = sales();
    // Data bars, a total row and the recommended chart, as the UI sends them.
    exec(
        &mut e,
        &json!({ "cmd": "batch", "cmds": [
            { "cmd": "insertChart", "sheet": 0, "range": rng(0, 0, 3, 2), "kind": "column" },
            { "cmd": "addCf", "sheet": 0, "range": rng(1, 1, 3, 2),
              "rule": { "kind": "dataBar", "color": "#4472C4" } },
            { "cmd": "setInputs", "sheet": 0,
              "cells": [[4, 0, "Tổng"], [4, 1, "=SUM(B2:B4)"], [4, 2, "=SUM(C2:C4)"]] },
            { "cmd": "format", "sheet": 0, "range": rng(4, 0, 4, 2), "patch": { "bold": true } },
        ] }),
    );
    assert_eq!(e.cell(0, 4, 1).unwrap().display, "22");
    assert_eq!(e.cell(0, 4, 2).unwrap().display, "25");
    let cf = exec(&mut e, &json!({ "cmd": "listCf", "sheet": 0 }));
    assert_eq!(cf["rules"].as_array().unwrap().len(), 1);
    let bytes = e.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_QUICK_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join("quick-analysis.xlsx"), &bytes).unwrap();
    }
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "quick.xlsx").unwrap();
    assert_eq!(back.cell(0, 4, 2).unwrap().display, "25");
    assert_eq!(
        exec(&mut back, &json!({ "cmd": "charts", "sheet": 0 }))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        exec(&mut back, &json!({ "cmd": "listCf", "sheet": 0 }))["rules"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // One undo takes all of it back.
    assert!(e.undo().unwrap());
    assert_eq!(e.cell(0, 4, 1).unwrap().display, "");
    assert_eq!(
        exec(&mut e, &json!({ "cmd": "charts", "sheet": 0 })),
        json!([])
    );
    assert_eq!(
        exec(&mut e, &json!({ "cmd": "listCf", "sheet": 0 }))["rules"],
        json!([])
    );
}

//! BlueSheet what-if scenarios through the command bridge: add / edit / delete, show (one
//! undo step), merge, summary report on a new sheet, protection and saving.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn shown(e: &SheetEngine, sheet: usize, a1: &str) -> String {
    let c = bo_model::sheet::CellRef::parse(a1).unwrap();
    e.cell(sheet, c.row, c.col).unwrap().display
}

/// Price B1 = 10, quantity B2 = 100, revenue B3 = B1*B2 with scenarios "Low" (8, 50) and
/// "High" (12, 200).
fn engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 1, "10".into()),
            (1, 1, "100".into()),
            (2, 1, "=B1*B2".into()),
        ],
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "Low", "cells": "$B$1:$B$2",
                "values": ["8", "50"], "comment": "bad year", "user": "An"}),
    )
    .unwrap();
    assert_eq!(r["index"], 0);
    let r = exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "High", "cells": "B1, B2",
                "values": ["12", "200"]}),
    )
    .unwrap();
    assert_eq!(r["index"], 1);
    e
}

#[test]
fn add_edit_delete_and_list() {
    let mut e = engine();
    // A clashing name (any case) is refused, as is a value count that does not match.
    assert!(exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "low", "cells": "B1", "values": ["1"]}),
    )
    .is_err());
    assert!(exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "Mid", "cells": "B1:B2", "values": ["1"]}),
    )
    .is_err());
    // Without values the cells' current values are taken.
    exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "Now", "cells": "B1:B2"}),
    )
    .unwrap();
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["items"].as_array().unwrap().len(), 3);
    assert_eq!(l["items"][0]["cells"], json!(["B1", "B2"]));
    assert_eq!(l["items"][0]["values"], json!(["8", "50"]));
    assert_eq!(l["items"][0]["comment"], "bad year");
    assert_eq!(l["items"][0]["user"], "An");
    assert_eq!(l["items"][0]["locked"], true);
    assert_eq!(l["items"][2]["values"], json!(["10", "100"]));
    assert_eq!(l["protected"], false);
    // Edit "Now": one more cell; kept cells keep their stored values, the new one is read.
    exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "index": 2, "name": "Now", "cells": "B1 B2 C1"}),
    )
    .unwrap();
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["items"][2]["values"], json!(["10", "100", ""]));
    exec(
        &mut e,
        &json!({"cmd": "scenarioDelete", "sheet": 0, "index": 2}),
    )
    .unwrap();
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["items"].as_array().unwrap().len(), 2);
    e.undo().unwrap();
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["items"].as_array().unwrap().len(), 3);
    assert!(exec(
        &mut e,
        &json!({"cmd": "scenarioDelete", "sheet": 0, "index": 9})
    )
    .is_err());
}

#[test]
fn show_is_one_undo_step() {
    let mut e = engine();
    assert_eq!(shown(&e, 0, "B3"), "1000");
    exec(
        &mut e,
        &json!({"cmd": "scenarioShow", "sheet": 0, "index": 0}),
    )
    .unwrap();
    // 8 × 50 = 400.
    assert_eq!(shown(&e, 0, "B1"), "8");
    assert_eq!(shown(&e, 0, "B3"), "400");
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["current"], 0);
    exec(
        &mut e,
        &json!({"cmd": "scenarioShow", "sheet": 0, "index": 1}),
    )
    .unwrap();
    // 12 × 200 = 2400.
    assert_eq!(shown(&e, 0, "B3"), "2400");
    e.undo().unwrap();
    assert_eq!(shown(&e, 0, "B3"), "400");
    e.undo().unwrap();
    assert_eq!(shown(&e, 0, "B1"), "10");
    assert_eq!(shown(&e, 0, "B2"), "100");
    assert_eq!(shown(&e, 0, "B3"), "1000");
}

#[test]
fn summary_report_and_merge() {
    let mut e = engine();
    let r = exec(
        &mut e,
        &json!({"cmd": "scenarioSummary", "sheet": 0, "results": "B3",
                "labels": {"sheet": "Báo cáo", "title": "Kịch bản – {sheet}", "current": "Hiện tại",
                           "changing": "Ô thay đổi", "results": "Ô kết quả", "comment": "Ghi chú",
                           "note": ""}}),
    )
    .unwrap();
    assert_eq!(r["sheet"], 1);
    let info = e.info().unwrap();
    assert_eq!(info.sheets[1].name, "Báo cáo");
    // The source sheet is untouched.
    assert_eq!(shown(&e, 0, "B1"), "10");
    assert_eq!(shown(&e, 0, "B3"), "1000");
    assert_eq!(shown(&e, 1, "A1"), "Kịch bản – Sheet1");
    assert_eq!(shown(&e, 1, "C3"), "Hiện tại");
    assert_eq!(shown(&e, 1, "D3"), "Low");
    assert_eq!(shown(&e, 1, "E3"), "High");
    // "Low" has a comment, so row 4 holds the comments.
    assert_eq!(shown(&e, 1, "A4"), "Ghi chú");
    assert_eq!(shown(&e, 1, "D4"), "bad year");
    assert_eq!(shown(&e, 1, "A5"), "Ô thay đổi");
    assert_eq!(shown(&e, 1, "B6"), "$B$1");
    assert_eq!(shown(&e, 1, "C6"), "10");
    assert_eq!(shown(&e, 1, "D6"), "8");
    assert_eq!(shown(&e, 1, "E6"), "12");
    assert_eq!(shown(&e, 1, "B7"), "$B$2");
    assert_eq!(shown(&e, 1, "E7"), "200");
    assert_eq!(shown(&e, 1, "A8"), "Ô kết quả");
    assert_eq!(shown(&e, 1, "B9"), "$B$3");
    assert_eq!(shown(&e, 1, "C9"), "1000");
    assert_eq!(shown(&e, 1, "D9"), "400");
    assert_eq!(shown(&e, 1, "E9"), "2400");
    // The result cells are remembered.
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["results"], "B3");
    // One undo step removes the report.
    e.undo().unwrap();
    assert_eq!(e.info().unwrap().sheets.len(), 1);
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["results"], "");

    // Merge into the report sheet: both copied; merging again renames the clashes.
    exec(
        &mut e,
        &json!({"cmd": "scenarioSummary", "sheet": 0, "results": "B3"}),
    )
    .unwrap();
    let r = exec(
        &mut e,
        &json!({"cmd": "scenarioMerge", "sheet": 1, "from": 0}),
    )
    .unwrap();
    assert_eq!(r["merged"], 2);
    exec(
        &mut e,
        &json!({"cmd": "scenarioMerge", "sheet": 1, "from": 0}),
    )
    .unwrap();
    let l = exec(&mut e, &json!({"cmd": "scenarios", "sheet": 1})).unwrap();
    let names: Vec<&str> = l["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Low", "High", "Low (2)", "High (2)"]);
    assert!(exec(
        &mut e,
        &json!({"cmd": "scenarioMerge", "sheet": 1, "from": 1})
    )
    .is_err());
}

#[test]
fn protection_and_saving() {
    let mut e = engine();
    exec(
        &mut e,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "Secret", "cells": "B1",
                "values": ["99"], "hidden": true, "locked": false}),
    )
    .unwrap();
    exec(
        &mut e,
        &json!({"cmd": "scenarioShow", "sheet": 0, "index": 1}),
    )
    .unwrap();
    // Saved and read back with the flags, comment, author and current index.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    let l = exec(&mut f, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["items"].as_array().unwrap().len(), 3);
    assert_eq!(l["items"][0]["comment"], "bad year");
    assert_eq!(l["items"][2]["hidden"], true);
    assert_eq!(l["items"][2]["locked"], false);
    assert_eq!(l["current"], 1);
    assert_eq!(shown(&f, 0, "B3"), "2400");

    // Protected sheet (scenarios protected): hidden ones are not listed, locked ones
    // cannot change, nothing can be added; showing still works on unlocked cells only.
    exec(
        &mut f,
        &json!({"cmd": "protectSheet", "sheet": 0, "password": ""}),
    )
    .unwrap();
    let l = exec(&mut f, &json!({"cmd": "scenarios", "sheet": 0})).unwrap();
    assert_eq!(l["protected"], true);
    let idx: Vec<u64> = l["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["index"].as_u64().unwrap())
        .collect();
    assert_eq!(idx, [0, 1]);
    assert!(exec(
        &mut f,
        &json!({"cmd": "scenarioDelete", "sheet": 0, "index": 0})
    )
    .is_err());
    assert!(exec(
        &mut f,
        &json!({"cmd": "scenarioSave", "sheet": 0, "name": "New", "cells": "B1", "values": ["1"]}),
    )
    .is_err());
    // The unlocked hidden scenario may still be deleted.
    exec(
        &mut f,
        &json!({"cmd": "scenarioDelete", "sheet": 0, "index": 2}),
    )
    .unwrap();
    // B1 is a locked cell: entering a scenario is refused.
    assert!(exec(
        &mut f,
        &json!({"cmd": "scenarioShow", "sheet": 0, "index": 0})
    )
    .is_err());
    assert_eq!(shown(&f, 0, "B1"), "12");
}

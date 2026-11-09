//! Conditional formatting rules manager over the JSON bridge (SHEET-CF-001):
//! `addCf` with data bar / icon set options, `listCf` in priority order, `editCf`
//! (move, Stop If True, replace, delete) and undo.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn run(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn rng() -> Value {
    json!({ "r0": 0, "c0": 0, "r1": 4, "c1": 0 })
}

fn kinds(e: &mut SheetEngine) -> Vec<(String, i64, bool)> {
    run(e, &json!({ "cmd": "listCf", "sheet": 0 }))["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["kind"].as_str().unwrap().to_owned(),
                r["priority"].as_i64().unwrap(),
                r["stopIfTrue"].as_bool().unwrap(),
            )
        })
        .collect()
}

#[test]
fn manager_commands() {
    let mut e = SheetEngine::new();
    e.new_workbook("cf.xlsx").unwrap();
    let cells: Vec<(u32, u32, String)> = (0..5)
        .map(|r| (r, 0, format!("{}", i64::from(r) * 10 - 20)))
        .collect();
    e.set_inputs(0, &cells).unwrap();
    run(
        &mut e,
        &json!({ "cmd": "addCf", "sheet": 0, "range": rng(), "rule": {
            "kind": "dataBar", "color": "4A7BB7",
            "options": { "min": { "kind": "autoMin" }, "max": { "kind": "autoMax" },
                         "axisPosition": "middle", "negativeColor": "8E3B8E" } } }),
    );
    run(
        &mut e,
        &json!({ "cmd": "addCf", "sheet": 0, "range": rng(), "rule": {
            "kind": "iconSet", "set": "5Boxes", "options": {
                "thresholds": [ { "kind": "num", "value": "-10" }, { "kind": "num", "value": "0" },
                                { "kind": "num", "value": "10", "gte": false }, { "kind": "percent", "value": "90" } ],
                "showValue": false } } }),
    );
    assert_eq!(
        kinds(&mut e),
        vec![("iconSet".into(), 1, false), ("dataBar".into(), 2, false)]
    );
    let list = run(&mut e, &json!({ "cmd": "listCf", "sheet": 0 }));
    let bar = &list["rules"][1];
    assert_eq!(bar["spec"]["options"]["axisPosition"], "middle");
    assert_eq!(bar["spec"]["options"]["negativeColor"], "8E3B8E");
    let (b, r) = (bar["block"].clone(), bar["rule"].clone());
    // Move the data bar first, then stop there.
    run(
        &mut e,
        &json!({ "cmd": "editCf", "sheet": 0, "block": b, "rule": r, "op": { "op": "moveUp" } }),
    );
    run(
        &mut e,
        &json!({ "cmd": "editCf", "sheet": 0, "block": b, "rule": r,
                 "op": { "op": "stopIfTrue", "value": true } }),
    );
    assert_eq!(
        kinds(&mut e),
        vec![("dataBar".into(), 1, true), ("iconSet".into(), 2, false)]
    );
    // Replace the data bar with a colour scale; undo brings the bar back.
    run(
        &mut e,
        &json!({ "cmd": "editCf", "sheet": 0, "block": b, "rule": r, "op": { "op": "replace",
                 "rule": { "kind": "colorScale", "minColor": "FFFFFF", "maxColor": "2E6DB4" } } }),
    );
    assert_eq!(kinds(&mut e)[0], ("colorScale".into(), 1, true));
    assert!(e.undo().unwrap());
    assert_eq!(kinds(&mut e)[0].0, "dataBar");
    // Delete everything; a bad index is an error.
    run(
        &mut e,
        &json!({ "cmd": "editCf", "sheet": 0, "block": b, "rule": r, "op": { "op": "delete" } }),
    );
    assert_eq!(kinds(&mut e).len(), 1);
    assert!(e
        .exec_json(
            &json!({ "cmd": "editCf", "sheet": 0, "block": 9, "rule": 0, "op": { "op": "delete" } })
                .to_string()
        )
        .is_err());
    // Rules on pivot values need a pivot table under the selection.
    assert!(e
        .exec_json(
            &json!({ "cmd": "addCf", "sheet": 0, "range": rng(), "pivot": true,
                     "rule": { "kind": "dataBar", "color": "4A7BB7" } })
            .to_string()
        )
        .is_err());
}

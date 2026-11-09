//! Engine tests of SHEET-DATA-001: sort by icon / custom list with the remembered
//! `sortState`, top-N / icon / relative-date AutoFilters against an injected clock,
//! and the Advanced Filter. Expected rows are worked out by hand from the data.

use crate::sheet::SheetEngine;
use bo_model::sheet::ColumnFilter;
use serde_json::{json, Value};

/// Region (A), name (B), sales (C); labels on row 0.
const DATA: [(&str, &str, &str); 8] = [
    ("Bắc", "An", "120"),    // 1
    ("Nam", "Bình", "80"),   // 2
    ("Bắc", "Bảo", "200"),   // 3
    ("Trung", "Chi", "150"), // 4
    ("Nam", "Bình", "80"),   // 5
    ("Bắc", "an", "50"),     // 6
    ("Nam", "Dũng", "300"),  // 7
    ("Trung", "Bích", "95"), // 8
];

fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    for (c, l) in ["Khu vực", "Nhân viên", "Doanh số"].iter().enumerate() {
        e.set_input(0, 0, c as u32, l).unwrap();
    }
    for (i, (a, b, c)) in DATA.iter().enumerate() {
        let r = i as u32 + 1;
        e.set_input(0, r, 0, a).unwrap();
        e.set_input(0, r, 1, b).unwrap();
        e.set_input(0, r, 2, c).unwrap();
    }
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn hidden(e: &SheetEngine, sheet: usize) -> Vec<u32> {
    e.workbook().unwrap().sheets[sheet]
        .rows
        .iter()
        .filter(|(_, i)| i.hidden)
        .map(|(r, _)| *r)
        .collect()
}

fn text(e: &SheetEngine, row: u32, col: u32) -> String {
    e.workbook().unwrap().sheets[0]
        .cell(row, col)
        .and_then(|c| c.value.as_text())
        .unwrap_or_default()
}

fn reopen(e: &mut SheetEngine, name: &str) -> SheetEngine {
    let bytes = e.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_SORTFILTER_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), &bytes).unwrap();
    }
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "x.xlsx").unwrap();
    back
}

// source: ECMA-376 Part 1, §18.3.1.91 sortCondition (customList, sortBy="icon"), §18.3.1.92 sortState
#[test]
fn sort_by_custom_list_and_icon_is_remembered() {
    let mut e = engine();
    run(
        &mut e,
        json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
    );
    // Region in the order Trung, Nam, Bắc; then sales descending.
    run(
        &mut e,
        json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 8, 2), "hasHeader": true,
            "keys": [
                { "col": 0, "customList": ["Trung", "Nam", "Bắc"] },
                { "col": 2, "descending": true }
            ] }),
    );
    let names: Vec<String> = (1..=8).map(|r| text(&e, r, 1)).collect();
    assert_eq!(
        names,
        ["Chi", "Bích", "Dũng", "Bình", "Bình", "Bảo", "An", "an"]
    );
    let ss = e.workbook().unwrap().sheets[0]
        .auto_filter
        .as_ref()
        .unwrap()
        .sort_state
        .clone()
        .unwrap();
    assert_eq!(ss.range.to_string(), "A2:C9");
    assert_eq!(
        ss.conditions[0].custom_list.as_deref(),
        Some("Trung,Nam,Bắc")
    );
    assert_eq!(ss.conditions[1].range.to_string(), "C2:C9");
    assert!(ss.conditions[1].descending);
    // Icon sort: 3 arrows on percent thresholds of 50..300 (33 % = 132.5, 66 % = 215):
    // 300 → icon 2; 200, 150 → icon 1; the rest icon 0.
    run(
        &mut e,
        json!({ "cmd": "addCf", "sheet": 0, "range": rng(1, 2, 8, 2),
            "rule": { "kind": "iconSet", "set": "3Arrows" } }),
    );
    let colors = run(
        &mut e,
        json!({ "cmd": "sortColors", "sheet": 0, "range": rng(0, 0, 8, 2), "key": 2 }),
    );
    let mut ids: Vec<u64> = colors["icons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_u64().unwrap())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, [0, 1, 2]);
    // Icon 1 on top, then by name ascending.
    run(
        &mut e,
        json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 8, 2), "hasHeader": true,
            "keys": [
                { "col": 2, "by": "icon", "iconSet": "3Arrows", "iconId": 1 },
                { "col": 1 }
            ] }),
    );
    let names: Vec<String> = (1..=8).map(|r| text(&e, r, 1)).collect();
    assert_eq!(
        names,
        ["Bảo", "Chi", "An", "an", "Bích", "Bình", "Bình", "Dũng"]
    );
    // Undo restores the previous order.
    assert!(e.undo().unwrap());
    assert_eq!(text(&e, 1, 1), "Chi");
    // The remembered state survives a save.
    let back = reopen(&mut e, "sort-state.xlsx");
    let ss = back.workbook().unwrap().sheets[0]
        .auto_filter
        .as_ref()
        .unwrap()
        .sort_state
        .clone()
        .unwrap();
    assert_eq!(ss.conditions.len(), 2);
    assert_eq!(
        ss.conditions[0].custom_list.as_deref(),
        Some("Trung,Nam,Bắc")
    );
}

// source: ECMA-376 Part 1, §18.3.2.10 top10, §18.3.2.9 iconFilter
#[test]
fn top_n_and_icon_filters() {
    let mut e = engine();
    run(
        &mut e,
        json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
    );
    // Top 2 items: 300 (row 7) and 200 (row 3).
    let set = |e: &mut SheetEngine, filter: Value| {
        run(
            e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 2, "filter": filter }),
        );
    };
    set(&mut e, json!({ "top10": { "val": 2 } }));
    assert_eq!(hidden(&e, 0), [1, 2, 4, 5, 6, 8]);
    let fv = run(
        &mut e,
        json!({ "cmd": "filterValues", "sheet": 0, "col": 2 }),
    );
    assert_eq!(fv["kind"], "top10");
    assert_eq!(fv["top10"]["val"], 2.0);
    // Bottom 25 % of 8 items = 2 items: cut-off is the 2nd smallest (80), ties kept:
    // 50 (row 6) and both 80s (rows 2, 5).
    set(
        &mut e,
        json!({ "top10": { "top": false, "percent": true, "val": 25 } }),
    );
    assert_eq!(hidden(&e, 0), [1, 3, 4, 7, 8]);
    // Above average (1075 / 8 = 134.375): 200, 150, 300.
    set(&mut e, json!({ "dynamic": "aboveAverage" }));
    assert_eq!(hidden(&e, 0), [1, 2, 5, 6, 8]);
    // Icon filter (3 arrows, percent thresholds 132.5 / 215): icon 1 = 200, 150.
    run(
        &mut e,
        json!({ "cmd": "addCf", "sheet": 0, "range": rng(1, 2, 8, 2),
            "rule": { "kind": "iconSet", "set": "3Arrows" } }),
    );
    set(&mut e, json!({ "icon": { "set": "3Arrows", "id": 1 } }));
    assert_eq!(hidden(&e, 0), [1, 2, 5, 6, 7, 8]);
    let fv = run(
        &mut e,
        json!({ "cmd": "filterValues", "sheet": 0, "col": 2 }),
    );
    assert_eq!(fv["kind"], "icon");
    assert_eq!(fv["icons"].as_array().unwrap().len(), 3);
    // Saved as iconFilter and read back.
    let back = reopen(&mut e, "icon-filter.xlsx");
    let af = back.workbook().unwrap().sheets[0]
        .auto_filter
        .clone()
        .unwrap();
    assert_eq!(
        af.columns[0].filter,
        ColumnFilter::Icon {
            icon_set: "3Arrows".into(),
            icon_id: Some(1)
        }
    );
}

// source: ECMA-376 Part 1, §18.3.2.5 dynamicFilter, §18.18.26 ST_DynamicFilterType
#[test]
fn relative_date_filters_use_the_engine_clock() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    e.set_input(0, 0, 0, "Ngày").unwrap();
    let dates = [
        "05/03/2024", // 1
        "20/11/2024", // 2
        "02/01/2025", // 3
        "15/01/2025", // 4
        "20/02/2025", // 5
        "10/05/2025", // 6
        "01/01/2026", // 7
    ];
    for (i, d) in dates.iter().enumerate() {
        e.set_input(0, i as u32 + 1, 0, d).unwrap();
    }
    // Today is Wednesday 2025-01-15 (10:00 UTC).
    e.set_now_ms(1_736_935_200_000);
    run(
        &mut e,
        json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
    );
    let all: Vec<u32> = (1..=7).collect();
    let visible = |e: &mut SheetEngine, kind: &str| -> Vec<u32> {
        run(
            e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": { "dynamic": kind } }),
        );
        let h = hidden(e, 0);
        all.iter().copied().filter(|r| !h.contains(r)).collect()
    };
    assert_eq!(visible(&mut e, "today"), [4]);
    // Week Sunday 12 – Saturday 18 January 2025.
    assert_eq!(visible(&mut e, "thisWeek"), [4]);
    assert_eq!(visible(&mut e, "thisMonth"), [3, 4]);
    assert_eq!(visible(&mut e, "lastQuarter"), [2]);
    assert_eq!(visible(&mut e, "nextQuarter"), [6]);
    assert_eq!(visible(&mut e, "thisYear"), [3, 4, 5, 6]);
    assert_eq!(visible(&mut e, "nextYear"), [7]);
    assert_eq!(visible(&mut e, "yearToDate"), [3, 4]);
    assert_eq!(visible(&mut e, "Q1"), [1, 3, 4, 5, 7]);
    assert_eq!(visible(&mut e, "M11"), [2]);
}

// source: ECMA-376 Part 1, §18.2.5 definedName (_xlnm._FilterDatabase, _xlnm.Criteria, _xlnm.Extract)
#[test]
fn advanced_filter_in_place_copy_and_show_all() {
    let mut e = engine();
    // Criteria F1:G3: (Bắc AND > 100) OR Nam.
    for (r, c, v) in [
        (0, 5, "Khu vực"),
        (0, 6, "Doanh số"),
        (1, 5, "Bắc"),
        (1, 6, ">100"),
        (2, 5, "Nam"),
    ] {
        e.set_input(0, r, c, v).unwrap();
    }
    let out = run(
        &mut e,
        json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, 0, 0),
            "criteria": rng(0, 5, 2, 6), "unique": true }),
    );
    // Rows 1, 2, 3, 7 (row 5 repeats row 2).
    assert_eq!(
        (out["matched"].as_u64(), out["hidden"].as_u64()),
        (Some(4), Some(4))
    );
    assert_eq!(out["list"], rng(0, 0, 8, 2));
    assert_eq!(hidden(&e, 0), [4, 5, 6, 8]);
    let names: Vec<String> = e
        .workbook()
        .unwrap()
        .defined_names
        .iter()
        .map(|d| d.name.clone())
        .collect();
    assert!(names.contains(&"_xlnm._FilterDatabase".to_owned()));
    assert!(names.contains(&"_xlnm.Criteria".to_owned()));
    // Saved: names and hidden rows survive.
    let back = reopen(&mut e, "advanced-filter.xlsx");
    assert_eq!(hidden(&back, 0), [4, 5, 6, 8]);
    // Undo shows the rows again; redo-free re-run then "show all".
    assert!(e.undo().unwrap());
    assert_eq!(hidden(&e, 0), Vec::<u32>::new());
    run(
        &mut e,
        json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, 8, 2),
            "criteria": rng(0, 5, 2, 6) }),
    );
    assert_eq!(hidden(&e, 0), [4, 6, 8]);
    let shown = run(&mut e, json!({ "cmd": "showAllData", "sheet": 0 }));
    assert_eq!(shown["shown"], 3);
    assert_eq!(hidden(&e, 0), Vec::<u32>::new());
    // Copy names only (label "Nhân viên" in I1) of rows matching; unique.
    e.set_input(0, 0, 8, "Nhân viên").unwrap();
    let out = run(
        &mut e,
        json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, 8, 2),
            "criteria": rng(0, 5, 2, 6), "copyTo": rng(0, 8, 0, 8), "unique": true }),
    );
    assert_eq!(out["extract"], rng(0, 8, 4, 8));
    let copied: Vec<String> = (1..=4).map(|r| text(&e, r, 8)).collect();
    // Names of rows 1, 2, 3, 7 (the second "Bình" is a duplicate on this column).
    assert_eq!(copied, ["An", "Bình", "Bảo", "Dũng"]);
    assert_eq!(hidden(&e, 0), Vec::<u32>::new());
    // Bad destination label → error, nothing changes.
    e.set_input(0, 0, 8, "Không có").unwrap();
    let err = e.exec_json(
        &json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, 8, 2),
            "criteria": rng(0, 5, 2, 6), "copyTo": rng(0, 8, 0, 8) })
        .to_string(),
    );
    assert!(err.is_err());
    assert_eq!(text(&e, 1, 8), "An");
}

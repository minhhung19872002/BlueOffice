//! BlueSheet pivot table family through the command bridge: pivot tables sharing a cache,
//! drill-down, pivot table slicers, timelines, the item filter of pivot chart buttons, and
//! slicers / timelines on printed pages (SHEET-PIVOT-001/002, SHEET-SLICER-001,
//! SHEET-TIMELINE-001). Expected values are worked out by hand from the six source rows below.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use bo_model::sheet::{Cell, CellValue, CellXf, Workbook};
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    exec(e, &v).unwrap()
}

/// Region, kind, date (serial, §18.17.4.1), amount:
/// 2024-01-15 = 45306, 2024-02-10 = 45332, 2024-02-25 = 45347, 2024-03-05 = 45356,
/// 2025-01-20 = 45677, 2025-01-21 = 45678.
const ROWS: [(&str, &str, f64, f64); 6] = [
    ("Bắc", "A", 45306.0, 10.0),
    ("Nam", "B", 45332.0, 5.0),
    ("Bắc", "B", 45347.0, 7.0),
    ("Trung", "A", 45356.0, 2.0),
    ("Nam", "A", 45677.0, 1.0),
    ("Bắc", "A", 45678.0, 3.0),
];

fn engine() -> SheetEngine {
    let mut wb = Workbook::new();
    let date = wb.styles.add_xf(CellXf {
        num_fmt_id: 14,
        xf_id: Some(0),
        apply_number_format: Some(true),
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Vùng", "Loại", "Ngày", "Tiền"].iter().enumerate() {
        ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for (i, (region, kind, day, amount)) in ROWS.iter().enumerate() {
        let r = i as u32 + 1;
        ws.set_cell(r, 0, Cell::new(CellValue::Text((*region).into())));
        ws.set_cell(r, 1, Cell::new(CellValue::Text((*kind).into())));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(*day)).with_style(date));
        ws.set_cell(r, 3, Cell::new(CellValue::Number(*amount)));
    }
    // Fonts are needed to draw the printed pages.
    let mut e = SheetEngine::with_bundled_fonts();
    e.load(wb, "t.xlsx").unwrap();
    e
}

fn rng() -> Value {
    json!({ "r0": 0, "c0": 0, "r1": 0, "c1": 0 })
}

fn shown(e: &SheetEngine, sheet: usize, row: u32, col: u32) -> String {
    e.cell(sheet, row, col).unwrap().display
}

/// Two pivot tables by kind on sheet 1: the first at A3 (A 16, B 12, total 28 in B6), the
/// second at A13 (total in B16).
fn with_pivots() -> SheetEngine {
    let mut e = engine();
    let made = run(
        &mut e,
        json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(),
            "rows": [1], "values": [{ "field": 3, "function": "sum" }] }),
    );
    assert_eq!(made["sheet"], 1);
    run(
        &mut e,
        json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(),
            "dest": { "sheet": 1, "row": 12, "col": 0 },
            "rows": [1], "values": [{ "field": 3, "function": "sum" }] }),
    );
    assert_eq!(shown(&e, 1, 3, 0), "A");
    assert_eq!(shown(&e, 1, 3, 1), "16");
    assert_eq!(shown(&e, 1, 5, 1), "28");
    assert_eq!(shown(&e, 1, 15, 1), "28");
    e
}

#[test]
fn pivots_share_a_cache_and_drill_down_lists_records() {
    let mut e = with_pivots();
    // One cache for both pivot tables; each lists the other.
    assert_eq!(e.workbook().unwrap().pivot_caches.len(), 1);
    let list = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
    assert_eq!(list[0]["sharedWith"][0]["name"], list[1]["name"]);
    assert_eq!(list[1]["sharedWith"][0]["sheet"], 1);
    assert_eq!(
        list[0]["dataArea"],
        json!({ "r0": 3, "c0": 1, "r1": 5, "c1": 1 })
    );
    // The grand total: all six records on a new sheet; B (row 5 of the sheet): two.
    let d = run(
        &mut e,
        json!({ "cmd": "pivotDrillDown", "sheet": 1, "row": 5, "col": 1, "name": "Chi tiết" }),
    );
    assert_eq!(d, json!({ "sheet": 2, "name": "Chi tiết1", "rows": 6 }));
    assert_eq!(shown(&e, 2, 0, 0), "Vùng");
    assert_eq!(shown(&e, 2, 0, 3), "Tiền");
    assert_eq!(shown(&e, 2, 1, 0), "Bắc");
    assert_eq!(shown(&e, 2, 6, 3), "3");
    // The dates keep their date format (not the bare serial).
    assert_ne!(shown(&e, 2, 1, 2), "45306");
    let b = run(
        &mut e,
        json!({ "cmd": "pivotDrillDown", "sheet": 1, "row": 4, "col": 1 }),
    );
    assert_eq!(b["name"], "Detail1");
    assert_eq!(b["rows"], 2);
    assert_eq!(
        (shown(&e, 3, 1, 0), shown(&e, 3, 1, 3)),
        ("Nam".into(), "5".into())
    );
    assert_eq!(
        (shown(&e, 3, 2, 0), shown(&e, 3, 2, 3)),
        ("Bắc".into(), "7".into())
    );
    // One undo step each; a label cell has no detail.
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.info().unwrap().sheets.len(), 2);
    assert!(exec(
        &mut e,
        &json!({ "cmd": "pivotDrillDown", "sheet": 1, "row": 3, "col": 0 })
    )
    .is_err());
}

#[test]
fn pivot_slicer_commands() {
    let mut e = with_pivots();
    let names: Vec<String> = e.workbook().unwrap().sheets[1]
        .pivots
        .iter()
        .map(|p| p.name.clone())
        .collect();
    // A slicer over "Vùng" (not in the report), placed right of the pivot table.
    let made = run(
        &mut e,
        json!({ "cmd": "insertPivotSlicer", "sheet": 1, "pivot": 0, "field": 0 }),
    );
    assert_eq!(made["index"], 0);
    let list = run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }));
    let s = &list[0];
    assert_eq!(s["kind"], "pivot");
    assert_eq!(s["caption"], "Vùng");
    assert_eq!(s["from"], json!([2, 3]));
    assert_eq!(s["style"], "SlicerStyleLight1");
    assert_eq!(s["colors"]["header"], "#FFFFFF");
    let labels: Vec<&str> = s["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Bắc", "Nam", "Trung"]);
    let conn: Vec<(String, bool)> = s["pivots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap().to_owned(),
                p["connected"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(conn, [(names[0].clone(), true), (names[1].clone(), false)]);
    // Nam: (Nam, B, 5) and (Nam, A, 1) → total 6 in the connected pivot table only.
    run(
        &mut e,
        json!({ "cmd": "slicerSelect", "sheet": 1, "index": 0, "items": ["Nam"] }),
    );
    assert_eq!(shown(&e, 1, 5, 1), "6");
    assert_eq!(shown(&e, 1, 15, 1), "28");
    run(
        &mut e,
        json!({ "cmd": "slicerConnect", "sheet": 1, "index": 0, "pivots": names }),
    );
    assert_eq!(shown(&e, 1, 15, 1), "6");
    e.undo().unwrap();
    assert_eq!(shown(&e, 1, 15, 1), "28");
    // Options and moving the frame; each is one undo step.
    run(
        &mut e,
        json!({ "cmd": "slicerOptions", "sheet": 1, "index": 0,
            "caption": "Khu vực", "columns": 3, "style": "SlicerStyleDark2" }),
    );
    run(
        &mut e,
        json!({ "cmd": "moveSlicer", "sheet": 1, "index": 0, "from": [1, 6], "to": [9, 10] }),
    );
    let list = run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }));
    let s = &list[0];
    assert_eq!(
        (
            s["caption"].clone(),
            s["columns"].clone(),
            s["style"].clone()
        ),
        (json!("Khu vực"), json!(3), json!("SlicerStyleDark2"))
    );
    assert_eq!(s["colors"]["header"], s["colors"]["selected"]);
    assert_eq!(
        (s["from"].clone(), s["to"].clone()),
        (json!([1, 6]), json!([9, 10]))
    );
    assert!(exec(
        &mut e,
        &json!({ "cmd": "moveSlicer", "sheet": 1, "index": 0, "from": [5, 5], "to": [1, 1] })
    )
    .is_err());
    assert!(exec(
        &mut e,
        &json!({ "cmd": "slicerOptions", "sheet": 1, "index": 0, "columns": 0 })
    )
    .is_err());
    e.undo().unwrap();
    let list = run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(list[0]["from"], json!([2, 3]));

    // Printing draws the slicer: "Trung" appears nowhere else on the sheet.
    let page = run(
        &mut e,
        json!({ "cmd": "printPreview", "sheet": 1, "page": 0 }),
    );
    assert!(
        page.to_string().contains("Trung"),
        "slicer items are printed"
    );
    assert!(e.export_pdf().unwrap().starts_with(b"%PDF"));

    // Saved and opened again: the slicer comes back from its parts with the same choice,
    // still filters, and can be deleted.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut e2 = SheetEngine::new();
    e2.open(&bytes, "t.xlsx").unwrap();
    let list = run(&mut e2, json!({ "cmd": "slicers", "sheet": 1 }));
    let chosen: Vec<bool> = list[0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["selected"].as_bool().unwrap())
        .collect();
    assert_eq!(chosen, [false, true, false]);
    assert_eq!(list[0]["caption"], "Khu vực");
    assert_eq!(list[0]["columns"], 3);
    run(
        &mut e2,
        json!({ "cmd": "slicerSelect", "sheet": 1, "index": 0, "items": ["Bắc", "Trung"] }),
    );
    // Bắc 10 + 7 + 3, Trung 2.
    assert_eq!(shown(&e2, 1, 5, 1), "22");
    run(
        &mut e2,
        json!({ "cmd": "deleteSlicer", "sheet": 1, "index": 0 }),
    );
    assert_eq!(shown(&e2, 1, 5, 1), "28", "the filter goes with the slicer");
    let list = run(&mut e2, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(list.as_array().unwrap().len(), 0);
    e2.undo().unwrap();
    let list = run(&mut e2, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(shown(&e2, 1, 5, 1), "22");
    e2.redo().unwrap();
    let bytes = e2.save("xlsx", 0).unwrap();
    let mut e3 = SheetEngine::new();
    e3.open(&bytes, "t.xlsx").unwrap();
    let list = run(&mut e3, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(list.as_array().unwrap().len(), 0);
    assert_eq!(e3.info().unwrap().sheets[1].pivots, 2);
}

#[test]
fn timeline_commands() {
    let mut e = with_pivots();
    let names: Vec<String> = e.workbook().unwrap().sheets[1]
        .pivots
        .iter()
        .map(|p| p.name.clone())
        .collect();
    // "Tiền" holds no dates.
    assert!(exec(
        &mut e,
        &json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 3 })
    )
    .is_err());
    let made = run(
        &mut e,
        json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 2 }),
    );
    assert_eq!(made["index"], 0);
    let list = run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }));
    let t = &list[0];
    assert_eq!(
        (t["caption"].clone(), t["level"].clone()),
        (json!("Ngày"), json!(2))
    );
    assert_eq!(t["filtered"], false);
    // Months from January 2024 to January 2025.
    assert_eq!(t["periods"].as_array().unwrap().len(), 13);
    assert_eq!(t["style"], "TimeSlicerStyleLight1");
    // Years: 2024 and 2025.
    run(
        &mut e,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "level": 0 }),
    );
    let list = run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }));
    let periods = list[0]["periods"].as_array().unwrap();
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0]["label"], "2024");
    // 2024 (45292 … 45657): 10 + 5 + 7 + 2 = 24 in the connected pivot table.
    assert_eq!(
        (periods[0]["start"].clone(), periods[0]["end"].clone()),
        (json!(45292.0), json!(45657.0))
    );
    run(
        &mut e,
        json!({ "cmd": "timelineSelect", "sheet": 1, "index": 0, "start": 45292, "end": 45657 }),
    );
    assert_eq!(shown(&e, 1, 5, 1), "24");
    assert_eq!(shown(&e, 1, 15, 1), "28");
    run(
        &mut e,
        json!({ "cmd": "timelineConnect", "sheet": 1, "index": 0, "pivots": names }),
    );
    assert_eq!(shown(&e, 1, 15, 1), "24");
    run(
        &mut e,
        json!({ "cmd": "moveTimeline", "sheet": 1, "index": 0, "from": [20, 0], "to": [27, 6] }),
    );
    assert!(exec(
        &mut e,
        &json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "level": 7 })
    )
    .is_err());
    // Printing draws the period labels of the timeline.
    let page = run(
        &mut e,
        json!({ "cmd": "printPreview", "sheet": 1, "page": 0 }),
    );
    assert!(
        page.to_string().contains("2025"),
        "timeline periods are printed"
    );

    // Saved and opened again: level, selection, anchor and connections come back.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut e2 = SheetEngine::new();
    e2.open(&bytes, "t.xlsx").unwrap();
    let list = run(&mut e2, json!({ "cmd": "timelines", "sheet": 1 }));
    let t = &list[0];
    assert_eq!(
        (t["level"].clone(), t["filtered"].clone()),
        (json!(0), json!(true))
    );
    assert_eq!(t["from"], json!([20, 0]));
    assert_eq!(t["periods"][0]["selected"], true);
    assert_eq!(t["periods"][1]["selected"], false);
    assert!(t["pivots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["connected"] == true));
    // Quarters in the file timeline, then 2025 only: (Nam, A, 1) + (Bắc, A, 3) = 4 in both
    // pivot tables.
    run(
        &mut e2,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "level": 1 }),
    );
    let list = run(&mut e2, json!({ "cmd": "timelines", "sheet": 1 }));
    assert_eq!(list[0]["periods"].as_array().unwrap().len(), 5);
    run(
        &mut e2,
        json!({ "cmd": "timelineSelect", "sheet": 1, "index": 0, "start": 45658, "end": 46022 }),
    );
    // Only kind A has rows in 2025: the reports shrink to A and the total line.
    assert_eq!(
        (shown(&e2, 1, 3, 0), shown(&e2, 1, 3, 1)),
        ("A".into(), "4".into())
    );
    assert_eq!(shown(&e2, 1, 4, 1), "4");
    assert_eq!(shown(&e2, 1, 14, 1), "4");
    run(
        &mut e2,
        json!({ "cmd": "deleteTimeline", "sheet": 1, "index": 0 }),
    );
    assert_eq!(shown(&e2, 1, 5, 1), "28");
    assert_eq!(shown(&e2, 1, 15, 1), "28");
    let bytes = e2.save("xlsx", 0).unwrap();
    let mut e3 = SheetEngine::new();
    e3.open(&bytes, "t.xlsx").unwrap();
    let list = run(&mut e3, json!({ "cmd": "timelines", "sheet": 1 }));
    assert_eq!(list.as_array().unwrap().len(), 0);
}

/// Timeline Tools › Show (header, selection label, time level, scroll bar) and the scroll
/// position of the strip: shown by default; hiding one is one undo step; scrolling is saved with
/// the file but adds no undo step; both survive a save, for a new timeline and for one read
/// from a file ([MS-XLSX] `CT_Timeline` attributes).
#[test]
fn timeline_show_options_and_scroll_position_round_trip() {
    let mut e = with_pivots();
    run(
        &mut e,
        json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 2 }),
    );
    let t = &run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    for k in [
        "showHeader",
        "showSelectionLabel",
        "showTimeLevel",
        "showScrollbar",
    ] {
        assert_eq!(t[k], true, "{k} shown by default");
    }
    assert_eq!(t["scrollDay"], Value::Null);
    run(
        &mut e,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0,
            "showTimeLevel": false, "showScrollbar": false }),
    );
    // 2024-06-01 = 45444: scrolled to June 2024.
    run(
        &mut e,
        json!({ "cmd": "timelineScroll", "sheet": 1, "index": 0, "day": 45444 }),
    );
    let t = &run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    assert_eq!(
        (t["showTimeLevel"].clone(), t["showScrollbar"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(t["showHeader"], true);
    assert_eq!(t["scrollDay"], 45444.0);
    // Scrolling added no undo step: one undo takes back the options (the workbook goes back
    // to how it was before them, like after a change of the sheet zoom).
    let depth = e.history_len();
    run(
        &mut e,
        json!({ "cmd": "timelineScroll", "sheet": 1, "index": 0, "day": 45444 }),
    );
    assert_eq!(e.history_len(), depth);
    assert!(e.undo().unwrap());
    let t = &run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    assert_eq!(t["showTimeLevel"], true);
    assert!(e.redo().unwrap());
    run(
        &mut e,
        json!({ "cmd": "timelineScroll", "sheet": 1, "index": 0, "day": 45444 }),
    );

    let bytes = e.save("xlsx", 0).unwrap();
    let mut e2 = SheetEngine::new();
    e2.open(&bytes, "t.xlsx").unwrap();
    let t = &run(&mut e2, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    assert_eq!(
        (t["showTimeLevel"].clone(), t["showScrollbar"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(t["scrollDay"], 45444.0);
    // Changed in the file's own part: header hidden, level shown again, scrolled back.
    run(
        &mut e2,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0,
            "showHeader": false, "showTimeLevel": true }),
    );
    run(
        &mut e2,
        json!({ "cmd": "timelineScroll", "sheet": 1, "index": 0 }),
    );
    let bytes = e2.save("xlsx", 0).unwrap();
    let mut e3 = SheetEngine::new();
    e3.open(&bytes, "t.xlsx").unwrap();
    let t = &run(&mut e3, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    assert_eq!(
        (
            t["showHeader"].clone(),
            t["showTimeLevel"].clone(),
            t["showScrollbar"].clone()
        ),
        (json!(false), json!(true), json!(false))
    );
    assert_eq!(t["scrollDay"], Value::Null);
}

/// New Slicer Style / New Timeline Style: a custom style with a format per element is one undo
/// step, is offered with the frames (`customStyles`), draws them with its colours once chosen,
/// and comes back from the saved file; a name in use is refused.
#[test]
fn new_slicer_and_timeline_styles() {
    let mut e = with_pivots();
    run(
        &mut e,
        json!({ "cmd": "insertPivotSlicer", "sheet": 1, "pivot": 0, "field": 0 }),
    );
    run(
        &mut e,
        json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 2 }),
    );
    let depth = e.history_len();
    run(
        &mut e,
        json!({ "cmd": "createFrameStyle", "sheet": 1, "name": "Bộ lọc xanh",
            "whole": { "fill": "#F2F7FC", "border": "#1F4E79" },
            "header": { "text": "#1F4E79", "bold": true },
            "selected": { "fill": "#2E75B6", "text": "#FFFFFF" },
            "unselected": { "fill": "#DDEBF7", "text": "#595959" } }),
    );
    run(
        &mut e,
        json!({ "cmd": "createFrameStyle", "sheet": 1, "name": "Dòng cam", "timeline": true,
            "selected": { "fill": "#C55A11" }, "whole": { "border": "#BF8F00" } }),
    );
    assert_eq!(e.history_len(), depth + 2, "one undo step each");
    // Same name again (either kind), a built-in name: refused.
    for bad in [
        json!({ "cmd": "createFrameStyle", "sheet": 1, "name": "Bộ lọc xanh" }),
        json!({ "cmd": "createFrameStyle", "sheet": 1, "name": "Dòng cam" }),
        json!({ "cmd": "createFrameStyle", "sheet": 1, "name": "SlicerStyleLight3" }),
    ] {
        assert!(exec(&mut e, &bad).is_err(), "{bad}");
    }
    let s = &run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }))[0];
    assert_eq!(s["customStyles"], json!(["Bộ lọc xanh"]));
    let t = &run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    assert_eq!(t["customStyles"], json!(["Dòng cam"]));
    // Not applied until chosen; then the frame takes its colours.
    assert_eq!(s["style"], "SlicerStyleLight1");
    run(
        &mut e,
        json!({ "cmd": "slicerOptions", "sheet": 1, "index": 0, "style": "Bộ lọc xanh" }),
    );
    run(
        &mut e,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "style": "Dòng cam" }),
    );
    let check = |e: &mut SheetEngine| {
        let s = &run(e, json!({ "cmd": "slicers", "sheet": 1 }))[0];
        let c = &s["colors"];
        assert_eq!(s["style"], "Bộ lọc xanh");
        assert_eq!(
            (c["selected"].clone(), c["selectedText"].clone()),
            (json!("#2E75B6"), json!("#FFFFFF"))
        );
        assert_eq!(
            (c["unselected"].clone(), c["unselectedText"].clone()),
            (json!("#DDEBF7"), json!("#595959"))
        );
        assert_eq!(
            (
                c["background"].clone(),
                c["border"].clone(),
                c["headerText"].clone()
            ),
            (json!("#F2F7FC"), json!("#1F4E79"), json!("#1F4E79"))
        );
        let t = &run(e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
        assert_eq!(t["style"], "Dòng cam");
        assert_eq!(
            (
                t["colors"]["selected"].clone(),
                t["colors"]["border"].clone()
            ),
            (json!("#C55A11"), json!("#BF8F00"))
        );
    };
    check(&mut e);
    let bytes = e.save("xlsx", 0).unwrap();
    // For the validator (`bo-cli validate`): BO_FRAME_STYLES_OUT=<file>.
    if let Ok(out) = std::env::var("BO_FRAME_STYLES_OUT") {
        std::fs::write(out, &bytes).unwrap();
    }
    let mut e2 = SheetEngine::new();
    e2.open(&bytes, "t.xlsx").unwrap();
    check(&mut e2);
    let s = &run(&mut e2, json!({ "cmd": "slicers", "sheet": 1 }))[0];
    assert_eq!(s["customStyles"], json!(["Bộ lọc xanh"]));
    // Undo takes the styles away again (the choice first).
    e.undo().unwrap();
    e.undo().unwrap();
    e.undo().unwrap();
    let s = &run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }))[0];
    assert_eq!(s["customStyles"], json!(["Bộ lọc xanh"]));
    e.undo().unwrap();
    let s = &run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }))[0];
    assert_eq!(s["customStyles"], json!([]));
}

/// Changing the time level keeps the chosen dates: the first quarter of 2024 chosen, then months
/// → January to March 2024 are selected and the pivot table keeps the quarter's total.
#[test]
fn time_level_change_keeps_the_selection() {
    let mut e = with_pivots();
    run(
        &mut e,
        json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 2 }),
    );
    run(
        &mut e,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "level": 1 }),
    );
    // Q1 2024: 2024-01-01 = 45292 … 2024-03-31 = 45382: 10 + 5 + 7 + 2 = 24.
    run(
        &mut e,
        json!({ "cmd": "timelineSelect", "sheet": 1, "index": 0, "start": 45292, "end": 45382 }),
    );
    assert_eq!(shown(&e, 1, 5, 1), "24");
    run(
        &mut e,
        json!({ "cmd": "timelineOptions", "sheet": 1, "index": 0, "level": 2 }),
    );
    let t = &run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }))[0];
    let on: Vec<&str> = t["periods"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["selected"] == true)
        .map(|p| p["label"].as_str().unwrap())
        .collect();
    assert_eq!(on, ["01/2024", "02/2024", "03/2024"]);
    assert_eq!(t["filtered"], true);
    assert_eq!(shown(&e, 1, 5, 1), "24");
}

#[test]
fn deleting_a_pivot_table_detaches_its_frames() {
    let mut e = with_pivots();
    let names: Vec<String> = e.workbook().unwrap().sheets[1]
        .pivots
        .iter()
        .map(|p| p.name.clone())
        .collect();
    // A slicer on both pivot tables, a timeline on the first one only.
    run(
        &mut e,
        json!({ "cmd": "insertPivotSlicer", "sheet": 1, "pivot": 0, "field": 0 }),
    );
    run(
        &mut e,
        json!({ "cmd": "slicerConnect", "sheet": 1, "index": 0, "pivots": names }),
    );
    run(
        &mut e,
        json!({ "cmd": "insertTimeline", "sheet": 1, "pivot": 0, "field": 2 }),
    );
    // Placed below each other, right of the report.
    let slicers = run(&mut e, json!({ "cmd": "slicers", "sheet": 1 }));
    let timelines = run(&mut e, json!({ "cmd": "timelines", "sheet": 1 }));
    assert_eq!(slicers[0]["from"], json!([2, 3]));
    assert_eq!(timelines[0]["from"], json!([15, 3]));
    // Read from a file, then the first pivot table is deleted.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut e2 = SheetEngine::with_bundled_fonts();
    e2.open(&bytes, "t.xlsx").unwrap();
    run(
        &mut e2,
        json!({ "cmd": "pivotDelete", "sheet": 1, "index": 0 }),
    );
    // The slicer stays with the second pivot table; the timeline had no other one.
    let slicers = run(&mut e2, json!({ "cmd": "slicers", "sheet": 1 }));
    let conn = slicers[0]["pivots"].as_array().unwrap();
    assert_eq!(conn.len(), 1);
    assert_eq!(conn[0]["name"], names[1].as_str());
    assert_eq!(conn[0]["connected"], true);
    let timelines = run(&mut e2, json!({ "cmd": "timelines", "sheet": 1 }));
    assert_eq!(timelines.as_array().unwrap().len(), 0);
    // The slicer still filters the remaining pivot table (now the only one, at A13).
    run(
        &mut e2,
        json!({ "cmd": "slicerSelect", "sheet": 1, "index": 0, "items": ["Trung"] }),
    );
    assert_eq!(shown(&e2, 1, 13, 1), "2");
    // Saved again and reopened: one slicer on one pivot table, no timeline parts left.
    let bytes = e2.save("xlsx", 0).unwrap();
    let mut e3 = SheetEngine::new();
    e3.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(e3.info().unwrap().sheets[1].pivots, 1);
    let slicers = run(&mut e3, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(slicers.as_array().unwrap().len(), 1);
    assert_eq!(slicers[0]["pivots"].as_array().unwrap().len(), 1);
    let timelines = run(&mut e3, json!({ "cmd": "timelines", "sheet": 1 }));
    assert_eq!(timelines.as_array().unwrap().len(), 0);
    // Deleting the last pivot table removes the slicer too.
    run(
        &mut e3,
        json!({ "cmd": "pivotDelete", "sheet": 1, "index": 0 }),
    );
    let slicers = run(&mut e3, json!({ "cmd": "slicers", "sheet": 1 }));
    assert_eq!(slicers.as_array().unwrap().len(), 0);
}

#[test]
fn pivot_chart_filter_buttons_hide_items() {
    let mut e = with_pivots();
    let c = run(
        &mut e,
        json!({ "cmd": "pivotChart", "sheet": 1, "index": 0, "kind": "column" }),
    );
    let charts = run(&mut e, json!({ "cmd": "charts", "sheet": 1 }));
    assert_eq!(
        charts[c["index"].as_u64().unwrap() as usize]["pivotAt"],
        json!([1, 0])
    );
    let cats = |e: &SheetEngine| -> Vec<String> {
        e.workbook().unwrap().sheets[1].charts[0].plots[0].series[0]
            .categories
            .clone()
    };
    assert_eq!(cats(&e), ["A", "B"]);
    // Hiding item 0 (A) of the row field: the report shows B 12 and the chart follows.
    run(
        &mut e,
        json!({ "cmd": "pivotFieldItems", "sheet": 1, "index": 0, "field": 1, "hidden": [0] }),
    );
    assert_eq!(
        (shown(&e, 1, 3, 0), shown(&e, 1, 3, 1)),
        ("B".into(), "12".into())
    );
    assert_eq!(shown(&e, 1, 4, 1), "12");
    assert_eq!(cats(&e), ["B"]);
    let list = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
    assert_eq!(list[0]["fieldOptions"][1]["hidden"], json!([0]));
    // Hiding every item is refused; an empty list shows all again.
    assert!(exec(
        &mut e,
        &json!({ "cmd": "pivotFieldItems", "sheet": 1, "index": 0, "field": 1, "hidden": [0, 1] })
    )
    .is_err());
    run(
        &mut e,
        json!({ "cmd": "pivotFieldItems", "sheet": 1, "index": 0, "field": 1, "hidden": [] }),
    );
    assert_eq!(shown(&e, 1, 5, 1), "28");
    assert_eq!(cats(&e), ["A", "B"]);
    // A report filter keeps the ticked items instead.
    run(
        &mut e,
        json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [1], "cols": [],
            "values": [{ "field": 3, "function": "sum" }], "filters": [{ "field": 0 }] }),
    );
    run(
        &mut e,
        json!({ "cmd": "pivotFieldItems", "sheet": 1, "index": 0, "field": 0, "hidden": [0, 2] }),
    );
    let list = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
    assert_eq!(list[0]["filters"][0]["items"], json!([1]));
    // Nam only: A 1, B 5.
    let body = e.workbook().unwrap().sheets[1].pivots[0].location;
    assert_eq!(shown(&e, 1, body.last.row, body.last.col), "6");
}

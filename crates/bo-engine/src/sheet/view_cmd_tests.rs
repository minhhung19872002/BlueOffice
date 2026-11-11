//! SHEET-VIEW-001: view type / show formulas / zoom, page break preview and
//! dragged breaks, page layout hit-testing, custom views. Expected values are
//! derived by hand from the inputs.

use super::*;
use bo_model::sheet::CellValue;
use serde_json::{json, Value};

fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
}

/// A1:A100 filled, every row 20 pt high, A4 portrait with the default margins.
/// Printable height = 842 − 2 × 0.75 in × 72 = 734 pt → 36 rows of 20 pt per page
/// (720 ≤ 734 < 740): rows 1–36, 37–72, 73–100.
fn hundred_rows() -> SheetEngine {
    let mut e = engine();
    let wb = e.wb.as_mut().unwrap();
    let ws = &mut wb.sheets[0];
    ws.format.default_row_height = Some(20.0);
    for r in 0..100 {
        ws.set_cell(
            r,
            0,
            bo_model::sheet::Cell::new(CellValue::Number(f64::from(r))),
        );
    }
    e.invalidate(None);
    e
}

fn spans(v: &Value) -> Vec<[u32; 2]> {
    serde_json::from_value(v.clone()).unwrap()
}

#[test]
fn breaks_and_fit_scale_by_hand() {
    // Spans 0–35, 36–71, 72–99 with a manual break after row index 35.
    let b = breaks_of(&[(0, 35), (36, 71), (72, 99)], &[35]);
    assert_eq!(
        b,
        vec![
            Break {
                at: 36,
                manual: true
            },
            Break {
                at: 72,
                manual: false
            }
        ]
    );
    // 50 rows × 20 pt = 1000 pt on 734 pt at 100 % → 73 % (734 / 1000 = 0.734).
    assert_eq!(fit_scale(1000.0, 0.0, 734.0, 1.0), Some(73));
    // 14 pt of headings: 734 / 1014 = 0.7238… → 72 %.
    assert_eq!(fit_scale(1000.0, 14.0, 734.0, 1.0), Some(72));
    // At 70 % the 1000 pt print as 700 pt ≤ 734 pt: it fits.
    assert_eq!(fit_scale(1000.0, 0.0, 734.0, 0.7), None);
    assert_eq!(fit_scale(700.0, 0.0, 734.0, 1.0), None);
    // Never below 10 %.
    assert_eq!(fit_scale(100_000.0, 0.0, 734.0, 1.0), Some(10));
}

#[test]
fn automatic_breaks_follow_page_height() {
    let mut e = hundred_rows();
    let r = run(&mut e, json!({ "cmd": "pageBreakPreview", "sheet": 0 }));
    let a = &r["areas"][0];
    assert_eq!((a["r0"].clone(), a["r1"].clone()), (json!(0), json!(99)));
    assert_eq!(spans(&a["rows"]), vec![[0, 35], [36, 71], [72, 99]]);
    assert_eq!(spans(&a["cols"]), vec![[0, 0]]);
    assert_eq!(
        a["rowBreaks"],
        json!([{ "at": 36, "manual": false }, { "at": 72, "manual": false }])
    );
    let numbers: Vec<u64> = a["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["number"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, vec![1, 2, 3]);
}

#[test]
fn dragging_breaks_moves_manual_breaks_and_rescales() {
    let mut e = hundred_rows();
    // Automatic break before row 37 (index 36) dragged up to row 31: a manual
    // break after index 29; then 36 rows per page again: 30–65, 66–99.
    let r = run(
        &mut e,
        json!({ "cmd": "movePageBreak", "sheet": 0, "rows": true, "from": 36, "to": 30 }),
    );
    assert_eq!(r["scale"], Value::Null);
    assert_eq!(e.sheet(0).unwrap().page_setup.row_breaks, vec![29]);
    let p = run(&mut e, json!({ "cmd": "pageBreakPreview", "sheet": 0 }));
    assert_eq!(
        spans(&p["areas"][0]["rows"]),
        vec![[0, 29], [30, 65], [66, 99]]
    );
    assert_eq!(
        p["areas"][0]["rowBreaks"][0],
        json!({ "at": 30, "manual": true })
    );

    // Dragging it down to row 51: the first page holds 50 rows = 1000 pt >
    // 734 pt, so the scale becomes ⌊734 / 1000 × 100⌋ = 73 %. At 73 % a page
    // holds 734 / 0.73 = 1005.5 pt = 50 rows: 0–49 (manual), 50–99.
    let r = run(
        &mut e,
        json!({ "cmd": "movePageBreak", "sheet": 0, "rows": true, "from": 30, "to": 50 }),
    );
    assert_eq!(r["scale"], json!(73));
    let ws = e.sheet(0).unwrap();
    assert_eq!(ws.page_setup.row_breaks, vec![49]);
    assert_eq!(ws.page_setup.scale, Some(73));
    let p = run(&mut e, json!({ "cmd": "pageBreakPreview", "sheet": 0 }));
    assert_eq!(spans(&p["areas"][0]["rows"]), vec![[0, 49], [50, 99]]);

    // One undo step each.
    e.undo().unwrap();
    assert_eq!(e.sheet(0).unwrap().page_setup.row_breaks, vec![29]);
    assert_eq!(e.sheet(0).unwrap().page_setup.scale, None);

    // Dragged out of the print area: removed.
    run(
        &mut e,
        json!({ "cmd": "movePageBreak", "sheet": 0, "rows": true, "from": 30, "to": 0 }),
    );
    assert_eq!(e.sheet(0).unwrap().page_setup.row_breaks.len(), 0);
    // A new break (no `from`) before row 11.
    run(
        &mut e,
        json!({ "cmd": "movePageBreak", "sheet": 0, "rows": true, "to": 10 }),
    );
    assert_eq!(e.sheet(0).unwrap().page_setup.row_breaks, vec![9]);
}

#[test]
fn view_type_show_formulas_and_zoom() {
    let mut e = engine();
    let s = run(&mut e, json!({ "cmd": "sheetViewState", "sheet": 0 }));
    assert_eq!(s["view"], "normal");
    assert_eq!(s["showFormulas"], false);
    assert_eq!(s["zoom"], 100);
    // Zoom 80 % in normal view, then page break preview at 60 %.
    run(
        &mut e,
        json!({ "cmd": "setSheetView", "sheet": 0, "zoom": 80 }),
    );
    run(
        &mut e,
        json!({ "cmd": "setSheetView", "sheet": 0, "view": "pageBreakPreview" }),
    );
    let s = run(
        &mut e,
        json!({ "cmd": "setSheetView", "sheet": 0, "zoom": 60 }),
    );
    assert_eq!(s["view"], "pageBreakPreview");
    assert_eq!(s["zoom"], 60);
    assert_eq!(s["zooms"]["normal"], 80);
    let v = &e.sheet(0).unwrap().views[0];
    assert_eq!(v.attr("view"), Some("pageBreakPreview"));
    assert_eq!(v.attr("zoomScaleNormal"), Some("80"));
    assert_eq!(v.attr("zoomScaleSheetLayoutView"), Some("60"));
    assert_eq!(v.zoom_scale, Some(60));
    // Back to normal: the normal zoom comes back.
    let s = run(
        &mut e,
        json!({ "cmd": "setSheetView", "sheet": 0, "view": "normal", "showFormulas": true }),
    );
    assert_eq!(s["zoom"], 80);
    assert_eq!(s["showFormulas"], true);
    assert!(e.sheet(0).unwrap().views[0].show_formulas());
    // Not an undo step.
    assert!(!e.can_undo());
    // Show formulas doubles the column widths of the geometry (64 px → 128 px).
    let w = e.geometry(0).unwrap().col_width(0);
    let plain = {
        let mut f = engine();
        f.geometry(0).unwrap().col_width(0)
    };
    assert!((w - 2.0 * plain).abs() < 1e-9);
    // Dragging column B to 150 pt while formulas are shown keeps it 150 pt on screen
    // and stores the normal width 75 pt (100 px): back in normal view it is 75 pt.
    run(
        &mut e,
        json!({ "cmd": "resize", "sheet": 0, "rows": false, "first": 1, "last": 1, "size": 150.0 }),
    );
    assert!((e.geometry(0).unwrap().col_width(1) - 150.0).abs() < 0.5);
    // The width dialog shows the normal width: (100 px − 5 px padding) / digit width.
    let s = run(
        &mut e,
        json!({ "cmd": "sizeOf", "sheet": 0, "rows": false, "index": 1 }),
    );
    let mdw = e.geometry(0).unwrap().metrics.mdw_px;
    let want = ((100.0 - 5.0) / mdw * 100.0_f64).round() / 100.0;
    assert!(
        (s["size"].as_f64().unwrap() - want).abs() < 1e-9,
        "{s} {want}"
    );
    run(
        &mut e,
        json!({ "cmd": "setSheetView", "sheet": 0, "showFormulas": false }),
    );
    assert!((e.geometry(0).unwrap().col_width(1) - 75.0).abs() < 0.5);
}

#[test]
fn page_cell_rect_inverts_page_hit() {
    let mut e = hundred_rows();
    // A4 portrait, default margins: A1 at the left / top margin (0.7 in, 0.75 in).
    let r = run(
        &mut e,
        json!({ "cmd": "pageCellRect", "sheet": 0, "page": 0, "row": 0, "col": 0 }),
    );
    assert!((r["x"].as_f64().unwrap() - 50.4).abs() < 1e-6, "{r}");
    assert!((r["y"].as_f64().unwrap() - 54.0).abs() < 1e-6, "{r}");
    assert!((r["h"].as_f64().unwrap() - 20.0).abs() < 1e-6, "{r}");
    // Row 38 (index 37) is the second row of page 2: 20 pt below its top margin.
    let r = run(
        &mut e,
        json!({ "cmd": "pageCellRect", "sheet": 0, "page": 1, "row": 37, "col": 0 }),
    );
    assert!((r["y"].as_f64().unwrap() - 74.0).abs() < 1e-6, "{r}");
    // The middle of that rectangle hits the same cell.
    let (x, y) = (
        r["x"].as_f64().unwrap() + r["w"].as_f64().unwrap() / 2.0,
        r["y"].as_f64().unwrap() + 10.0,
    );
    let hit = run(
        &mut e,
        json!({ "cmd": "pageHit", "sheet": 0, "page": 1, "x": x, "y": y }),
    );
    assert_eq!(hit, json!({ "row": 37, "col": 0 }));
    // Not on that page.
    let none = run(
        &mut e,
        json!({ "cmd": "pageCellRect", "sheet": 0, "page": 0, "row": 37, "col": 0 }),
    );
    assert!(none.is_null());
}

#[test]
fn page_layout_and_hit_test() {
    let mut e = hundred_rows();
    let r = run(&mut e, json!({ "cmd": "pageLayout", "sheet": 0 }));
    let pages = r["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);
    // A4: 595 × 842 pt; left / top margins 0.7 in = 50.4 pt and 0.75 in = 54 pt.
    assert_eq!(pages[0]["width"], 595.0);
    let c = &pages[1]["cells"];
    assert_eq!((c["r0"].clone(), c["r1"].clone()), (json!(36), json!(71)));
    let (x, y) = (c["x"].as_f64().unwrap(), c["y"].as_f64().unwrap());
    assert!((x - 50.4).abs() < 1e-9 && (y - 54.0).abs() < 1e-9);
    // 45 pt below the top of the cells of page 2 is the third row (20 pt rows): index 38.
    let h = run(
        &mut e,
        json!({ "cmd": "pageHit", "sheet": 0, "page": 1, "x": x + 5.0, "y": y + 45.0 }),
    );
    assert_eq!(h, json!({ "row": 38, "col": 0 }));
    // In the margin: nothing.
    let h = run(
        &mut e,
        json!({ "cmd": "pageHit", "sheet": 0, "page": 1, "x": 10.0, "y": 10.0 }),
    );
    assert_eq!(h, Value::Null);
}

#[test]
fn page_layout_fills_the_page_with_cells() {
    let mut e = hundred_rows();
    let r = run(&mut e, json!({ "cmd": "pageLayout", "sheet": 0 }));
    let pages = r["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);
    // The last page holds rows 73–100, then empty rows down to the bottom margin:
    // (842 − 2 × 54) pt / 20 pt = 36 rows a page → rows 72..=107.
    let c = &pages[2]["cells"];
    assert_eq!((c["r0"].clone(), c["r1"].clone()), (json!(72), json!(107)));
    // Across: as many columns as fit in 595 − 2 × 50.4 = 494.2 pt.
    let g = e.geometry(0).unwrap().clone();
    let (mut used, mut last) = (0.0, 0);
    for col in 0..100 {
        let w = g.col_width(col);
        if used + w > 494.2 + 0.01 {
            break;
        }
        used += w;
        last = col;
    }
    assert!(last > 0);
    assert_eq!(c["c1"], json!(last));
    // Typing below the data on the last page: the hit lands on an empty row.
    let (x, y) = (c["x"].as_f64().unwrap(), c["y"].as_f64().unwrap());
    let h = run(
        &mut e,
        json!({ "cmd": "pageHit", "sheet": 0, "page": 2, "x": x + 5.0, "y": y + 30.0 * 20.0 + 1.0 }),
    );
    assert_eq!(h, json!({ "row": 102, "col": 0 }));
    // An empty sheet: nothing to print, one page of empty cells in the layout view.
    let mut e = engine();
    let r = run(&mut e, json!({ "cmd": "pageLayout", "sheet": 0 }));
    assert_eq!(r["pages"].as_array().unwrap().len(), 1);
    assert_eq!(r["pages"][0]["cells"]["r0"], json!(0));
    assert!(r["pages"][0]["cells"]["r1"].as_u64().unwrap() > 20);
    let p = run(&mut e, json!({ "cmd": "printPreview", "sheet": 0 }));
    assert_eq!(p["pages"], json!(0));
    let p = run(
        &mut e,
        json!({ "cmd": "printPreview", "sheet": 0, "layout": true }),
    );
    assert_eq!(p["pages"], json!(1));
    // Data only in C3: the layout page still starts at A1.
    e.set_input(0, 2, 2, "x").unwrap();
    let r = run(&mut e, json!({ "cmd": "pageLayout", "sheet": 0 }));
    let c = &r["pages"][0]["cells"];
    assert_eq!((c["r0"].clone(), c["c0"].clone()), (json!(0), json!(0)));
    e.undo().unwrap();
    // The sheet's own gridlines are drawn on the layout page (printing them is off).
    let lines = p["page"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["op"] == "line")
        .count();
    assert!(lines > 20, "gridlines on the layout page: {lines}");
}

#[test]
fn custom_views_save_apply_delete() {
    let mut e = hundred_rows();
    // Hidden row 5 (index 4), landscape, zoom 75 % with C4 active.
    run(
        &mut e,
        json!({ "cmd": "hide", "sheet": 0, "rows": true, "first": 4, "last": 4, "hidden": true }),
    );
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "orientation": "landscape" }),
    );
    let r = run(
        &mut e,
        json!({
            "cmd": "addCustomView", "name": "In ngang", "sheet": 0,
            "zoom": 75, "active": [3, 2], "sqref": "C4:D5", "topLeft": [2, 0]
        }),
    );
    let guid = r["guid"].as_str().unwrap().to_owned();
    assert!(guid.starts_with('{') && guid.len() == 38);
    let list = run(&mut e, json!({ "cmd": "customViews" }));
    assert_eq!(list[0]["name"], "In ngang");
    let cv = &e.sheet(0).unwrap().custom_views[0];
    assert_eq!(cv.scale, Some(75));
    assert!(cv.hidden_rows);
    assert_eq!(cv.top_left_cell.as_deref(), Some("A3"));

    // Change everything, then apply the view.
    run(
        &mut e,
        json!({ "cmd": "hide", "sheet": 0, "rows": true, "first": 4, "last": 4, "hidden": false }),
    );
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "orientation": "portrait" }),
    );
    let a = run(&mut e, json!({ "cmd": "applyCustomView", "guid": guid }));
    assert_eq!(a["sheet"], 0);
    assert_eq!(a["zoom"], 75);
    assert_eq!(a["active"], json!([3, 2]));
    assert_eq!(a["topLeft"], json!([2, 0]));
    let ws = e.sheet(0).unwrap();
    assert_eq!(bo_sheet::rowcol::hidden_spans(ws, true), vec![(4, 4)]);
    assert_eq!(ws.page_setup.orientation.as_deref(), Some("landscape"));
    // Saving under the same name replaces the view.
    run(
        &mut e,
        json!({ "cmd": "addCustomView", "name": "in NGANG", "sheet": 0 }),
    );
    assert_eq!(e.wb().unwrap().custom_views.len(), 1);
    assert_eq!(e.sheet(0).unwrap().custom_views.len(), 1);
    let guid = e.wb().unwrap().custom_views[0].guid.clone();
    run(&mut e, json!({ "cmd": "deleteCustomView", "guid": guid }));
    assert_eq!(e.wb().unwrap().custom_views.len(), 0);
    assert_eq!(e.sheet(0).unwrap().custom_views.len(), 0);
    // Saved, reopened, still there (§18.2.3 / §18.3.1.26 round trip).
    run(
        &mut e,
        json!({ "cmd": "addCustomView", "name": "V", "sheet": 0 }),
    );
    let bytes = e.save("xlsx", 0).unwrap();
    let back = bo_xlsx::read_xlsx(&bytes).unwrap();
    assert_eq!(back.custom_views[0].name, "V");
    assert_eq!(back.sheets[0].custom_views.len(), 1);
    // The exact hidden rows travel as `Z_<guid>_.wvu.Rows` (row 5 still hidden).
    assert_eq!(
        back.sheets[0].custom_views[0].session_hidden_rows,
        Some(vec![(4, 4)])
    );
}

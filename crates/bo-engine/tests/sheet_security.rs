//! BlueSheet: password-protected files ([MS-OFFCRYPTO]), sheet / workbook
//! protection (ECMA-376 Part 1, §18.3.1.85, §18.2.29), sheet tabs (move,
//! `tabColor` §18.3.1.93), multi-range commands and opening with cached results.

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn workbook() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "1".into()),
            (1, 0, "2".into()),
            (2, 0, "=A1+A2".into()),
        ],
    )
    .unwrap();
    e
}

#[test]
fn encrypted_workbook_needs_its_password_and_keeps_it() {
    let mut e = workbook();
    exec(
        &mut e,
        &json!({"cmd": "setSavePassword", "password": "Bí mật"}),
    )
    .unwrap();
    let bytes = e.save("xlsx", 0).unwrap();
    assert!(
        bo_crypto::is_encrypted(&bytes),
        "saved as an encrypted package"
    );

    let mut r = SheetEngine::new();
    let err = r.open(&bytes, "t.xlsx").unwrap_err().to_string();
    assert_eq!(err, "password required");
    let err = r
        .open_with_password(&bytes, "t.xlsx", Some("sai"))
        .unwrap_err()
        .to_string();
    assert_eq!(err, "wrong password");
    let info = r
        .open_with_password(&bytes, "t.xlsx", Some("Bí mật"))
        .unwrap();
    assert!(info.encrypted);
    assert_eq!(r.cell(0, 2, 0).unwrap().display, "3");
    // Saved again with the same password.
    let again = r.save("xlsx", 0).unwrap();
    let mut r2 = SheetEngine::new();
    assert!(r2
        .open_with_password(&again, "t.xlsx", Some("Bí mật"))
        .is_ok());
    // Removing the password saves a plain package.
    exec(&mut r2, &json!({"cmd": "setSavePassword"})).unwrap();
    assert!(r2.save("xlsx", 0).unwrap().starts_with(b"PK"));
}

#[test]
fn protected_sheet_refuses_locked_cells_and_keeps_unlocked_ones_editable() {
    let mut e = workbook();
    // B1:B3 unlocked (§18.8.33 protection/@locked = 0).
    exec(
        &mut e,
        &json!({"cmd": "format", "sheet": 0, "range": {"r0": 0, "c0": 1, "r1": 2, "c1": 1},
                "patch": {"locked": false}}),
    )
    .unwrap();
    exec(
        &mut e,
        &json!({"cmd": "protectSheet", "sheet": 0, "password": "khoá",
                "allow": {"formatCells": false, "sort": true}}),
    )
    .unwrap();
    assert!(e.info().unwrap().sheets[0].protected);
    let err = e.set_input(0, 0, 0, "5").unwrap_err().to_string();
    assert!(
        err.ends_with(bo_engine::sheet::protect::SHEET_PROTECTED),
        "{err}"
    );
    e.set_input(0, 1, 1, "ok").unwrap();
    assert_eq!(e.cell(0, 1, 1).unwrap().display, "ok");
    // Formatting and structure are refused, the value of A1 is unchanged.
    let fmt = json!({"cmd": "format", "sheet": 0, "range": {"r0": 0, "c0": 0, "r1": 0, "c1": 0},
                     "patch": {"bold": true}});
    assert!(exec(&mut e, &fmt).is_err());
    assert!(e.structural(0, true, 0, 1, false).is_err());
    assert_eq!(e.cell(0, 0, 0).unwrap().display, "1");
    // Paste into a locked cell is refused as a whole (content check in `edit`).
    exec(
        &mut e,
        &json!({"cmd": "copy", "sheet": 0, "range": {"r0": 1, "c0": 1, "r1": 1, "c1": 1}}),
    )
    .unwrap();
    assert!(exec(
        &mut e,
        &json!({"cmd": "paste", "sheet": 0, "row": 0, "col": 0})
    )
    .is_err());
    assert_eq!(e.cell(0, 0, 0).unwrap().display, "1");
    // State for the UI.
    let p = exec(&mut e, &json!({"cmd": "protection", "sheet": 0})).unwrap();
    assert_eq!(p["sheet"]["protected"], true);
    assert_eq!(p["sheet"]["hasPassword"], true);
    assert_eq!(p["sheet"]["allow"]["sort"], true);

    // Survives save / reopen (sheetProtection written at its schema slot).
    let bytes = e.save("xlsx", 0).unwrap();
    let mut r = SheetEngine::new();
    r.open(&bytes, "t.xlsx").unwrap();
    assert!(r.set_input(0, 0, 0, "5").is_err());
    assert!(exec(
        &mut r,
        &json!({"cmd": "unprotectSheet", "sheet": 0, "password": "sai"})
    )
    .is_err());
    exec(
        &mut r,
        &json!({"cmd": "unprotectSheet", "sheet": 0, "password": "khoá"}),
    )
    .unwrap();
    r.set_input(0, 0, 0, "5").unwrap();
    assert_eq!(r.cell(0, 2, 0).unwrap().display, "7");
}

#[test]
fn workbook_structure_protection_blocks_sheet_operations() {
    let mut e = workbook();
    e.add_sheet(Some("Hai")).unwrap();
    exec(&mut e, &json!({"cmd": "protectWorkbook", "password": "wb"})).unwrap();
    assert!(e.info().unwrap().structure_locked);
    assert!(e.add_sheet(None).is_err());
    assert!(e.rename_sheet(1, "Ba").is_err());
    assert!(e.delete_sheet(1).is_err());
    assert!(exec(&mut e, &json!({"cmd": "moveSheet", "from": 1, "to": 0})).is_err());
    let bytes = e.save("xlsx", 0).unwrap();
    let mut r = SheetEngine::new();
    assert!(r.open(&bytes, "t.xlsx").unwrap().structure_locked);
    exec(
        &mut r,
        &json!({"cmd": "unprotectWorkbook", "password": "wb"}),
    )
    .unwrap();
    r.add_sheet(None).unwrap();
}

#[test]
fn sheet_tabs_move_and_colour() {
    let mut e = workbook();
    e.add_sheet(Some("Hai")).unwrap();
    e.add_sheet(Some("Ba")).unwrap();
    e.set_input(2, 0, 0, "=Sheet1!A3*10").unwrap();
    exec(&mut e, &json!({"cmd": "moveSheet", "from": 2, "to": 0})).unwrap();
    let names: Vec<String> = e
        .info()
        .unwrap()
        .sheets
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["Ba", "Sheet1", "Hai"]);
    assert_eq!(e.cell(0, 0, 0).unwrap().display, "30");
    exec(
        &mut e,
        &json!({"cmd": "setTabColor", "sheet": 1, "color": "#c00000"}),
    )
    .unwrap();
    assert_eq!(
        e.info().unwrap().sheets[1].tab_color.as_deref(),
        Some("C00000")
    );
    assert!(exec(
        &mut e,
        &json!({"cmd": "setTabColor", "sheet": 1, "color": "red"})
    )
    .is_err());
    let bytes = e.save("xlsx", 0).unwrap();
    let mut r = SheetEngine::new();
    let info = r.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(info.sheets[1].tab_color.as_deref(), Some("C00000"));
    assert_eq!(info.sheets[0].name, "Ba");
    // Undo restores the order.
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.info().unwrap().sheets[0].name, "Sheet1");
}

#[test]
fn batch_formats_several_ranges_in_one_undo_step_and_stats_count_once() {
    let mut e = workbook();
    e.set_inputs(0, &[(0, 3, "10".into()), (5, 5, "x".into())])
        .unwrap();
    let rng = |r0: u32, c0: u32, r1: u32, c1: u32| json!({"r0": r0, "c0": c0, "r1": r1, "c1": c1});
    exec(
        &mut e,
        &json!({"cmd": "batch", "cmds": [
            {"cmd": "format", "sheet": 0, "range": rng(0, 0, 1, 0), "patch": {"bold": true}},
            {"cmd": "format", "sheet": 0, "range": rng(0, 3, 0, 3), "patch": {"bold": true}},
        ]}),
    )
    .unwrap();
    let bold = |e: &mut SheetEngine, r: u32, c: u32| {
        exec(
            e,
            &json!({"cmd": "formatState", "sheet": 0, "row": r, "col": c}),
        )
        .unwrap()["bold"]
            == true
    };
    assert!(bold(&mut e, 0, 0) && bold(&mut e, 1, 0) && bold(&mut e, 0, 3));
    assert!(!bold(&mut e, 0, 1));
    e.undo().unwrap();
    assert!(!bold(&mut e, 0, 0) && !bold(&mut e, 0, 3), "one undo step");
    // A1:A3 and A2:D2 overlap on A2; D1 separate; text counts but is not summed.
    let s = exec(
        &mut e,
        &json!({"cmd": "rangeStats", "sheet": 0,
                "ranges": [rng(0, 0, 2, 0), rng(1, 0, 1, 3), rng(0, 3, 0, 3), rng(5, 5, 5, 5)]}),
    )
    .unwrap();
    assert_eq!(s["sum"], 16.0);
    assert_eq!(s["numbers"], 4);
    assert_eq!(s["count"], 5);
    // A failing command rolls the batch back.
    let err = exec(
        &mut e,
        &json!({"cmd": "batch", "cmds": [
            {"cmd": "format", "sheet": 0, "range": rng(0, 0, 0, 0), "patch": {"italic": true}},
            {"cmd": "setTabColor", "sheet": 0, "color": "nope"},
        ]}),
    );
    assert!(err.is_err());
    let st = exec(
        &mut e,
        &json!({"cmd": "formatState", "sheet": 0, "row": 0, "col": 0}),
    )
    .unwrap();
    assert_eq!(st["italic"], false);
}

#[test]
fn opening_keeps_saved_results_unless_full_calc_on_load() {
    // A formula whose saved result differs from its calculation (as written
    // by another application): kept as saved, like spreadsheet applications do.
    let mut e = workbook();
    let bytes = e.save("xlsx", 0).unwrap();
    let mut wb = bo_xlsx::read_xlsx(&bytes).unwrap();
    wb.sheets[0].cells.get_mut(&(2, 0)).unwrap().value = bo_model::sheet::CellValue::Number(42.0);
    let stale = bo_xlsx::write_xlsx(&wb).unwrap();
    let mut r = SheetEngine::new();
    r.open(&stale, "t.xlsx").unwrap();
    assert_eq!(r.cell(0, 2, 0).unwrap().display, "42");
    r.set_input(0, 0, 0, "10").unwrap();
    assert_eq!(r.cell(0, 2, 0).unwrap().display, "12", "edits recalculate");
    // calcPr/@fullCalcOnLoad (§18.2.2) recalculates everything on open.
    wb.unknown.push(bo_model::sheet::RawXml {
        after: "calcPr".into(),
        xml: r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#.into(),
    });
    let full = bo_xlsx::write_xlsx(&wb).unwrap();
    r.open(&full, "t.xlsx").unwrap();
    assert_eq!(r.cell(0, 2, 0).unwrap().display, "3");
}

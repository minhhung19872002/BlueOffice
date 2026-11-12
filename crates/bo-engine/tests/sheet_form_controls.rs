//! BlueSheet form controls through the command bridge: each control writes its linked cell
//! (values derived by hand below), the linked cell drives the control, every change is one
//! undo step, and the controls survive saving.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]

use bo_engine::SheetEngine;
use bo_model::sheet::{CellRef, ControlAnchor, FormControl, FormControlKind, OpaquePart};
use serde_json::{json, Value};

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn run(e: &mut SheetEngine, v: Value) -> Value {
    exec(e, &v).unwrap()
}

fn list(e: &mut SheetEngine) -> Vec<Value> {
    run(e, json!({"cmd": "formControls", "sheet": 0}))
        .as_array()
        .unwrap()
        .clone()
}

fn shown(e: &SheetEngine, a1: &str) -> String {
    let at = CellRef::parse(a1).unwrap();
    e.cell(0, at.row, at.col).unwrap().display
}

fn insert(e: &mut SheetEngine, kind: &str, row: u32, col: u32) -> usize {
    let r = run(
        e,
        json!({"cmd": "insertFormControl", "sheet": 0, "kind": kind, "row": row, "col": col, "text": "Mục"}),
    );
    r["index"].as_u64().unwrap() as usize
}

fn format(e: &mut SheetEngine, index: usize, spec: Value) {
    run(
        e,
        json!({"cmd": "formatFormControl", "sheet": 0, "index": index, "spec": spec}),
    );
}

fn new_engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e
}

#[test]
fn check_box_writes_true_false_and_follows_its_cell() {
    let mut e = new_engine();
    let i = insert(&mut e, "checkBox", 1, 1);
    format(&mut e, i, json!({"link": "$D$1"}));
    // Linking writes the current state (unchecked) to D1.
    assert_eq!(shown(&e, "D1"), "FALSE");
    e.set_input(0, 0, 4, "=IF(D1,\"Có\",\"Không\")").unwrap();
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": i}),
    );
    assert_eq!(shown(&e, "D1"), "TRUE");
    assert_eq!(shown(&e, "E1"), "Có");
    assert_eq!(list(&mut e)[i]["checked"], "checked");
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": i}),
    );
    assert_eq!(shown(&e, "D1"), "FALSE");
    assert_eq!(shown(&e, "E1"), "Không");
    // One undo step per click.
    e.undo().unwrap();
    assert_eq!(shown(&e, "D1"), "TRUE");
    assert_eq!(list(&mut e)[i]["checked"], "checked");
    // Typing into the linked cell changes the control.
    e.set_input(0, 0, 3, "FALSE").unwrap();
    assert_eq!(list(&mut e)[i]["checked"], "unchecked");
    // Mixed writes #N/A.
    format(&mut e, i, json!({"checked": "mixed"}));
    assert_eq!(shown(&e, "D1"), "#N/A");
    assert_eq!(list(&mut e)[i]["checked"], "mixed");
}

#[test]
fn option_buttons_write_their_position_in_the_group() {
    let mut e = new_engine();
    let g = insert(&mut e, "groupBox", 0, 0); // rows 0..6, cols 0..3
    let a = insert(&mut e, "optionButton", 1, 0);
    let b = insert(&mut e, "optionButton", 3, 0);
    let c = insert(&mut e, "optionButton", 10, 0); // outside: its own group
    let d = insert(&mut e, "optionButton", 12, 0);
    format(&mut e, a, json!({"link": "$F$1"}));
    format(&mut e, c, json!({"link": "$F$2"}));
    let l = list(&mut e);
    assert_eq!(l[a]["group"], l[b]["group"]);
    assert_ne!(l[a]["group"], l[c]["group"]);
    assert_eq!(l[c]["group"], l[d]["group"]);
    assert!(l[g]["group"].is_null());
    // B is the 2nd button of the box.
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": b}),
    );
    assert_eq!(shown(&e, "F1"), "2");
    let l = list(&mut e);
    assert_eq!(
        (l[a]["checked"].as_str(), l[b]["checked"].as_str()),
        (Some("unchecked"), Some("checked"))
    );
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": a}),
    );
    assert_eq!(shown(&e, "F1"), "1");
    // D is the 2nd button of the sheet group; F1 is untouched.
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": d}),
    );
    assert_eq!(shown(&e, "F2"), "2");
    assert_eq!(shown(&e, "F1"), "1");
    // The cell drives the group.
    e.set_input(0, 0, 5, "2").unwrap();
    let l = list(&mut e);
    assert_eq!(
        (l[a]["checked"].as_str(), l[b]["checked"].as_str()),
        (Some("unchecked"), Some("checked"))
    );
}

#[test]
fn combo_and_list_boxes_write_the_chosen_index() {
    let mut e = new_engine();
    for (r, t) in ["Hà Nội", "Đà Nẵng", "Cần Thơ"].iter().enumerate() {
        e.set_input(0, 9 + r as u32, 0, t).unwrap();
    }
    let i = insert(&mut e, "comboBox", 0, 2);
    format(
        &mut e,
        i,
        json!({"inputRange": "$A$10:$A$12", "link": "$G$1", "dropLines": 4}),
    );
    e.set_input(0, 0, 7, "=INDEX(A10:A12,G1)").unwrap();
    let l = list(&mut e);
    assert_eq!(l[i]["items"], json!(["Hà Nội", "Đà Nẵng", "Cần Thơ"]));
    assert_eq!(l[i]["dropLines"], 4);
    run(
        &mut e,
        json!({"cmd": "formControlSelect", "sheet": 0, "index": i, "sel": 3}),
    );
    assert_eq!(shown(&e, "G1"), "3");
    assert_eq!(shown(&e, "H1"), "Cần Thơ");
    // Past the end: the last item.
    run(
        &mut e,
        json!({"cmd": "formControlSelect", "sheet": 0, "index": i, "sel": 9}),
    );
    assert_eq!(list(&mut e)[i]["sel"], 3);
    e.set_input(0, 0, 6, "1").unwrap();
    assert_eq!(list(&mut e)[i]["sel"], 1);
    assert_eq!(shown(&e, "H1"), "Hà Nội");
    // A list box with a sheet-qualified range.
    let j = insert(&mut e, "listBox", 4, 2);
    format(
        &mut e,
        j,
        json!({"inputRange": "Sheet1!$A$10:$A$11", "link": "$G$2"}),
    );
    assert_eq!(list(&mut e)[j]["items"], json!(["Hà Nội", "Đà Nẵng"]));
    run(
        &mut e,
        json!({"cmd": "formControlSelect", "sheet": 0, "index": j, "sel": 2}),
    );
    assert_eq!(shown(&e, "G2"), "2");
    // A bad reference is refused and changes nothing.
    assert!(exec(
        &mut e,
        &json!({"cmd": "formatFormControl", "sheet": 0, "index": j, "spec": {"link": "Không có!A1"}})
    )
    .is_err());
}

#[test]
fn scroll_bar_and_spin_button_step_between_limits() {
    let mut e = new_engine();
    let s = insert(&mut e, "scrollBar", 0, 0);
    format(
        &mut e,
        s,
        json!({"link": "$I$1", "min": 10, "max": 50, "inc": 5, "page": 20, "val": 10}),
    );
    e.set_input(0, 0, 9, "=I1*2").unwrap();
    assert_eq!(shown(&e, "I1"), "10");
    let step = |e: &mut SheetEngine, n: i32, page: bool| {
        run(
            e,
            json!({"cmd": "formControlValue", "sheet": 0, "index": s, "step": n, "page": page}),
        )["val"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(step(&mut e, 1, false), 15); // 10 + 5
    assert_eq!(step(&mut e, 2, true), 50); // 15 + 2·20 = 55 → max 50
    assert_eq!(step(&mut e, -100, false), 10); // → min 10
    run(
        &mut e,
        json!({"cmd": "formControlValue", "sheet": 0, "index": s, "value": 33}),
    );
    assert_eq!(shown(&e, "I1"), "33");
    assert_eq!(shown(&e, "J1"), "66");
    e.undo().unwrap();
    assert_eq!(shown(&e, "I1"), "10");

    let p = insert(&mut e, "spinner", 0, 3);
    format(
        &mut e,
        p,
        json!({"link": "$I$2", "min": 1, "max": 12, "val": 11}),
    );
    assert_eq!(
        run(
            &mut e,
            json!({"cmd": "formControlValue", "sheet": 0, "index": p, "step": 1})
        )["val"],
        12
    );
    assert_eq!(
        run(
            &mut e,
            json!({"cmd": "formControlValue", "sheet": 0, "index": p, "step": 1})
        )["val"],
        12
    );
    // The cell drives the value (limited to the range).
    e.set_input(0, 1, 8, "999").unwrap();
    assert_eq!(list(&mut e)[p]["val"], 12);
    e.set_input(0, 1, 8, "4").unwrap();
    assert_eq!(list(&mut e)[p]["val"], 4);
}

#[test]
fn button_reports_its_macro_without_running_it() {
    let mut e = new_engine();
    let b = insert(&mut e, "button", 2, 2);
    format(
        &mut e,
        b,
        json!({"macroName": "ChayBaoCao", "text": "Báo cáo"}),
    );
    let undo = e.can_undo();
    let r = run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": b}),
    );
    assert_eq!(r["macro"], "ChayBaoCao");
    assert_eq!(r["changed"], false);
    assert_eq!(e.can_undo(), undo);
    let l = list(&mut e);
    assert_eq!(l[b]["text"], "Báo cáo");
    assert_eq!(l[b]["kind"], "button");
    // Labels and group boxes do nothing on click.
    let lb = insert(&mut e, "label", 6, 0);
    assert_eq!(
        run(
            &mut e,
            json!({"cmd": "formControlClick", "sheet": 0, "index": lb})
        )["changed"],
        false
    );
}

#[test]
fn controls_survive_saving_with_their_linked_state() {
    let mut e = new_engine();
    let c = insert(&mut e, "checkBox", 0, 0);
    let s = insert(&mut e, "scrollBar", 0, 3);
    format(&mut e, c, json!({"link": "$H$1"}));
    format(&mut e, s, json!({"link": "$H$2", "max": 40}));
    run(
        &mut e,
        json!({"cmd": "formControlClick", "sheet": 0, "index": c}),
    );
    // Typed values reach the file through the controls' state.
    e.set_input(0, 1, 7, "25").unwrap();
    run(
        &mut e,
        json!({"cmd": "moveFormControl", "sheet": 0, "index": c, "row": 5, "col": 1, "offset": [3.0, 1.5]}),
    );
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    let l = list(&mut f);
    assert_eq!(l.len(), 2);
    assert_eq!(l[c]["checked"], "checked");
    assert_eq!(l[c]["from"], json!([5, 1]));
    assert_eq!(l[c]["fromOff"], json!([3.0, 1.5]));
    assert_eq!(l[s]["val"], 25);
    assert_eq!(l[s]["max"], 40);
    let wb = f.workbook().unwrap();
    let fc = &wb.sheets[0].form_controls;
    assert_eq!(fc[s].val, 25);
    assert_ne!(fc[s].ctrl_prop_part, "");
    // Delete one, undo brings it back.
    run(
        &mut f,
        json!({"cmd": "deleteFormControl", "sheet": 0, "index": c}),
    );
    assert_eq!(list(&mut f).len(), 1);
    f.undo().unwrap();
    assert_eq!(list(&mut f).len(), 2);
}

#[test]
fn activex_controls_are_kept_with_their_picture() {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";
    let mut wb = bo_model::sheet::Workbook::new();
    wb.sheets[0].form_controls.push(FormControl {
        kind: FormControlKind::ActiveX,
        name: "CommandButton1".into(),
        text: "CommandButton1".into(),
        shape_id: 1025,
        activex_part: "/xl/activeX/activeX1.xml".into(),
        image_part: Some("/xl/media/image1.png".into()),
        anchor: ControlAnchor {
            from: CellRef::new(1, 1),
            to: CellRef::new(3, 3),
            ..ControlAnchor::default()
        },
        ..FormControl::default()
    });
    wb.opaque.parts.push(OpaquePart {
        name: "/xl/media/image1.png".into(),
        content_type: None,
        data: PNG.to_vec(),
    });
    let mut e = SheetEngine::new();
    e.load(wb, "ax.xlsx").unwrap();
    let l = list(&mut e);
    assert_eq!(l[0]["kind"], "activeX");
    assert!(l[0]["image"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert!(exec(
        &mut e,
        &json!({"cmd": "deleteFormControl", "sheet": 0, "index": 0})
    )
    .is_err());
    assert!(exec(
        &mut e,
        &json!({"cmd": "formatFormControl", "sheet": 0, "index": 0, "spec": {"text": "x"}})
    )
    .is_err());
    assert!(exec(
        &mut e,
        &json!({"cmd": "insertFormControl", "sheet": 0, "kind": "activeX", "row": 0, "col": 0})
    )
    .is_err());
}

/// [MS-XLSX] `controlPr/anchor` `moveWithCells` / `sizeWithCells`: a "move and size"
/// control stretches over rows inserted inside it, a "fixed" one stays, the default one
/// moves without sizing; the placement and size survive saving and reopening.
#[test]
fn placement_decides_how_controls_follow_their_cells() {
    let mut e = new_engine();
    // Three list boxes at B2 (rows 1..=5 zero-based after the default size).
    let a = insert(&mut e, "listBox", 1, 1);
    let b = insert(&mut e, "listBox", 1, 4);
    let c = insert(&mut e, "listBox", 1, 7);
    format(&mut e, b, json!({"placement": "moveAndSize"}));
    format(&mut e, c, json!({"placement": "fixed"}));
    let before = list(&mut e);
    assert_eq!(before[a]["placement"], "move");
    assert_eq!(before[b]["placement"], "moveAndSize");
    let from = |l: &[Value], i: usize| l[i]["from"][0].as_u64().unwrap();
    let to = |l: &[Value], i: usize| l[i]["to"][0].as_u64().unwrap();
    let h = to(&before, a) - from(&before, a);
    // Two rows inserted at row 3 (inside every control).
    e.structural(0, true, 2, 2, false).unwrap();
    let l = list(&mut e);
    assert_eq!(
        (from(&l, a), to(&l, a)),
        (1, 1 + h),
        "hand: starts above row 3, size kept"
    );
    assert_eq!(
        (from(&l, b), to(&l, b)),
        (1, 1 + h + 2),
        "hand: stretched by 2 rows"
    );
    assert_eq!((from(&l, c), to(&l, c)), (1, 1 + h), "hand: fixed");
    // Two rows inserted above: move and move-and-size shift by 2, fixed stays.
    e.structural(0, true, 0, 2, false).unwrap();
    let l = list(&mut e);
    assert_eq!(from(&l, a), 3);
    assert_eq!(from(&l, b), 3);
    assert_eq!(from(&l, c), 1);
    // Saved and reopened: the attributes come back.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    let m = list(&mut f);
    assert_eq!(m[a]["placement"], "move");
    assert_eq!(m[b]["placement"], "moveAndSize");
    assert_eq!(m[c]["placement"], "fixed");
    assert_eq!((from(&m, b), to(&m, b)), (3, 3 + h + 2));
    // Changing the placement of a control read from the file patches its entry in place.
    format(&mut f, a, json!({"placement": "moveAndSize"}));
    let bytes = f.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_VALIDATE_DIR") {
        // Hand-run check against the OpenXML validator (`bo-cli validate`).
        std::fs::write(format!("{dir}/fc-placement.xlsx"), &bytes).unwrap();
    }
    let mut g = SheetEngine::new();
    g.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(list(&mut g)[a]["placement"], "moveAndSize");
    g.structural(0, true, 4, 1, false).unwrap();
    let n = list(&mut g);
    assert_eq!(to(&n, a) - from(&n, a), h + 1);
}

#[test]
fn resize_moves_the_bottom_right_corner() {
    let mut e = new_engine();
    let i = insert(&mut e, "button", 2, 2);
    run(
        &mut e,
        json!({"cmd": "resizeFormControl", "sheet": 0, "index": i, "row": 6, "col": 5, "offset": [10.0, 4.0]}),
    );
    let l = list(&mut e);
    assert_eq!(l[i]["from"], json!([2, 2]));
    assert_eq!(l[i]["to"], json!([6, 5]));
    assert_eq!(l[i]["toOff"], json!([10.0, 4.0]));
    // Never above / left of the top-left corner.
    run(
        &mut e,
        json!({"cmd": "resizeFormControl", "sheet": 0, "index": i, "row": 0, "col": 0}),
    );
    let l = list(&mut e);
    assert_eq!(l[i]["to"], json!([2, 2]));
    assert_eq!(l[i]["toOff"], json!([1.0, 1.0]));
    e.undo().unwrap();
    assert_eq!(list(&mut e)[i]["to"], json!([6, 5]));
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(list(&mut f)[i]["to"], json!([6, 5]));
}

/// [MS-XLSX] `selType` `multi` / `extend` with `multiSel`: clicks combine with the
/// selection, the linked cell is left alone, and the selection survives saving.
#[test]
fn multi_select_list_boxes_keep_their_selection() {
    let mut e = new_engine();
    for (r, t) in ["Một", "Hai", "Ba", "Bốn", "Năm"].iter().enumerate() {
        e.set_input(0, r as u32, 0, t).unwrap();
    }
    let i = insert(&mut e, "listBox", 0, 2);
    format(
        &mut e,
        i,
        json!({"inputRange": "$A$1:$A$5", "link": "$G$1", "selType": "multi"}),
    );
    let click = |e: &mut SheetEngine, sel: u32, ctrl: bool, shift: bool| {
        run(
            e,
            json!({"cmd": "formControlSelect", "sheet": 0, "index": i, "sel": sel, "ctrl": ctrl, "shift": shift}),
        );
    };
    click(&mut e, 2, false, false);
    click(&mut e, 4, false, false);
    assert_eq!(list(&mut e)[i]["multiSel"], json!([2, 4]));
    click(&mut e, 2, false, false);
    assert_eq!(
        list(&mut e)[i]["multiSel"],
        json!([4]),
        "hand: second click toggles off"
    );
    assert_eq!(
        shown(&e, "G1"),
        "",
        "a multi-select list box leaves its cell alone"
    );
    e.undo().unwrap();
    assert_eq!(list(&mut e)[i]["multiSel"], json!([2, 4]));
    // Extend: plain click replaces, Ctrl toggles, Shift selects the run.
    format(&mut e, i, json!({"selType": "extend"}));
    click(&mut e, 1, false, false);
    assert_eq!(list(&mut e)[i]["multiSel"], json!([1]));
    click(&mut e, 3, false, true);
    assert_eq!(list(&mut e)[i]["multiSel"], json!([1, 2, 3]), "hand: 1..=3");
    click(&mut e, 5, true, false);
    assert_eq!(list(&mut e)[i]["multiSel"], json!([1, 2, 3, 5]));
    click(&mut e, 2, true, false);
    assert_eq!(list(&mut e)[i]["multiSel"], json!([1, 3, 5]));
    let bytes = e.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_VALIDATE_DIR") {
        // Hand-run check against the OpenXML validator (`bo-cli validate`).
        std::fs::write(format!("{dir}/fc-multisel.xlsx"), &bytes).unwrap();
    }
    let mut f = SheetEngine::new();
    f.open(&bytes, "t.xlsx").unwrap();
    let l = list(&mut f);
    assert_eq!(l[i]["selType"], "extend");
    assert_eq!(l[i]["multiSel"], json!([1, 3, 5]));
    // Back to single selection: the cell is written again.
    format(&mut f, i, json!({"selType": "single"}));
    run(
        &mut f,
        json!({"cmd": "formControlSelect", "sheet": 0, "index": i, "sel": 4}),
    );
    assert_eq!(shown(&f, "G1"), "4");
    assert_eq!(list(&mut f)[i]["multiSel"], json!([]));
}

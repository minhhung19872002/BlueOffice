//! Paste options (CORE-PASTE-001) through the JSON command bridge: the copied
//! picture, link / merge / text-only paste in BlueDoc, and the extra BlueSheet
//! paste options (column widths, no borders).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn copy_carries_a_picture_of_the_selection() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        &json!({"cmd":"insertText","text":"Ảnh của đoạn này"}),
    );
    // A caret copies no picture.
    let r = cmd(&mut e, &json!({"cmd":"copy"}));
    assert!(r["clip"].is_null() || r["clip"]["png"].is_null());
    cmd(&mut e, &json!({"cmd":"selectAll"}));
    let r = cmd(&mut e, &json!({"cmd":"copy"}));
    let png = base64::engine::general_purpose::STANDARD
        .decode(r["clip"]["png"].as_str().unwrap())
        .unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // Pasted as a picture: one inline picture, one undo step.
    cmd(&mut e, &json!({"cmd":"move","motion":"docEnd"}));
    let r = e.insert_image(png, "clipboard.png").unwrap();
    assert!(r.error.is_none());
    assert_eq!(e.document().unwrap().media.len(), 1);
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(e.document().unwrap().plain_text(), "Ảnh của đoạn này");
}

#[test]
fn paste_modes_switch_with_one_undo_step() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, &json!({"cmd":"insertText","text":"x "}));
    let html = "<p><b>đậm</b> chữ</p>";
    let r = cmd(&mut e, &json!({"cmd":"paste","html":html,"text":"đậm chữ"}));
    assert!(r["error"].is_null());
    // Switch to text only: undo the paste, paste again.
    cmd(&mut e, &json!({"cmd":"undo"}));
    let r = cmd(
        &mut e,
        &json!({"cmd":"paste","html":html,"text":"đậm chữ","mode":"text"}),
    );
    assert!(r["error"].is_null());
    assert_eq!(e.document().unwrap().plain_text(), "x đậm chữ");
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":2},"focus":{"p":0,"o":4}}}),
    );
    let r = cmd(&mut e, &json!({"cmd":"state"}));
    assert!(!r["format"]["bold"].as_bool().unwrap());
    // One undo goes back to the text before the paste.
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(e.document().unwrap().plain_text(), "x ");
}

#[test]
fn link_mode_pastes_a_hyperlink() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(
        &mut e,
        &json!({"cmd":"paste","text":"https://bluestar.vn","mode":"link"}),
    );
    assert!(r["error"].is_null());
    let doc = e.document().unwrap();
    let p = doc.paragraphs().next().unwrap();
    assert_eq!(p.runs[0].props.link.as_deref(), Some("https://bluestar.vn"));
    // Plain words are refused (the state is unchanged).
    let r = cmd(
        &mut e,
        &json!({"cmd":"paste","text":"hai từ","mode":"link"}),
    );
    assert!(r["error"].is_string());
    assert_eq!(e.document().unwrap().plain_text(), "https://bluestar.vn");
}

fn sheet_exec(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap_or(Value::Null)
}

/// Width (characters) of column `c` of the first sheet, when set.
fn col_width(e: &SheetEngine, c: u32) -> Option<f64> {
    e.workbook().unwrap().sheets[0]
        .cols
        .iter()
        .find(|x| x.min <= c && c <= x.max)
        .and_then(|x| x.width)
}

#[test]
fn sheet_paste_keeps_column_widths_and_drops_borders() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(0, &[(0, 0, "1".into()), (0, 1, "2".into())])
        .unwrap();
    sheet_exec(
        &mut e,
        &json!({"cmd":"resize","sheet":0,"rows":false,"first":0,"last":0,"size":120.0}),
    );
    sheet_exec(
        &mut e,
        &json!({"cmd":"borders","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":1},
                "spec":{"preset":"all"}}),
    );
    sheet_exec(
        &mut e,
        &json!({"cmd":"copy","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":1}}),
    );
    let w0 = col_width(&e, 0).unwrap();
    assert_eq!(col_width(&e, 4), None);
    // Column widths: column E takes column A's width, F keeps the default (B has none).
    let r = sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":3,"col":4,"colWidths":true}),
    );
    assert!(!r.is_null());
    assert!((col_width(&e, 4).unwrap() - w0).abs() < 0.01);
    assert_eq!(col_width(&e, 5), None);
    assert_eq!(e.cell(0, 3, 4).unwrap().input, "1");
    e.undo().unwrap();
    assert_eq!(col_width(&e, 4), None);
    assert_eq!(e.cell(0, 3, 4).unwrap().input, "");
    // No borders: values and formats without the borders.
    sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":5,"col":0,"noBorders":true}),
    );
    assert_eq!(e.cell(0, 5, 1).unwrap().input, "2");
    let src = sheet_exec(
        &mut e,
        &json!({"cmd":"formatState","sheet":0,"row":0,"col":0}),
    );
    let st = sheet_exec(
        &mut e,
        &json!({"cmd":"formatState","sheet":0,"row":5,"col":0}),
    );
    assert_eq!(src["borders"][0], "thin");
    assert_eq!(st["borders"], json!(["", "", "", ""]));
}

fn slide_run(e: &mut Engine, v: &Value) -> bo_engine::slide_edit::SlideEditResult {
    let r = e
        .slide_command(serde_json::from_value(v.clone()).unwrap())
        .unwrap();
    assert!(r.error.is_none(), "{v}: {:?}", r.error);
    r
}

/// JSON of the top-level shape `id` of slide 0.
fn shape_json(e: &Engine, id: u32) -> String {
    let p = e.presentation().unwrap();
    let n = p.slides[0]
        .data
        .shapes
        .iter()
        .find(|n| n.nv().is_some_and(|nv| nv.id == id))
        .unwrap();
    serde_json::to_string(n).unwrap()
}

#[test]
fn slide_shapes_paste_with_destination_theme_or_source_formatting() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-edit-clipboard.pptx"),
    )
    .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "a.pptx").unwrap();
    let r = slide_run(&mut e, &json!({"cmd":"insertShape","preset":"rect"}));
    let id = r.selection[0].id;
    assert!(shape_json(&e, id).contains("\"Scheme\":\"accent1\""));
    slide_run(&mut e, &json!({"cmd":"copy"}));
    // Default: theme references are kept (they follow the destination's theme).
    let r = slide_run(&mut e, &json!({"cmd":"paste"}));
    assert!(shape_json(&e, r.selection[0].id).contains("\"Scheme\":\"accent1\""));
    // Keep source formatting: literal colours; one undo step removes the copy.
    slide_run(&mut e, &json!({"cmd":"undo"}));
    let r = slide_run(&mut e, &json!({"cmd":"paste","keepSource":true}));
    let pasted = shape_json(&e, r.selection[0].id);
    assert!(!pasted.contains("\"Scheme\":\"accent1\""), "{pasted}");
    assert!(pasted.contains("\"Srgb\""));
    let n = e.presentation().unwrap().slides[0].data.shapes.len();
    slide_run(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(e.presentation().unwrap().slides[0].data.shapes.len(), n - 1);
}

/// First run properties of the shape `id` of slide 0.
fn first_run(e: &Engine, id: u32) -> bo_model::slide::RunProps {
    let p = e.presentation().unwrap();
    let n = p.slides[0]
        .data
        .shapes
        .iter()
        .find(|n| n.nv().is_some_and(|nv| nv.id == id))
        .unwrap();
    let bo_model::slide::ShapeNode::Shape(s) = n else {
        panic!("shape")
    };
    s.text.as_ref().unwrap().paragraphs[0]
        .runs
        .iter()
        .find_map(|r| match r {
            bo_model::slide::TextRun::Run { props, .. } => Some(props.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn keep_source_formatting_pins_what_placeholders_inherit() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-action-links.pptx"),
    )
    .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "a.pptx").unwrap();
    // A placeholder with text whose runs take their look from the layout / master.
    let id = {
        let p = e.presentation().unwrap();
        p.slides[0]
            .data
            .shapes
            .iter()
            .find_map(|n| match n {
                bo_model::slide::ShapeNode::Shape(s)
                    if s.nv.placeholder.is_some()
                        && s.text.as_ref().is_some_and(|t| !t.text().trim().is_empty()) =>
                {
                    Some(s.nv.id)
                }
                _ => None,
            })
            .expect("a placeholder with text on slide 0")
    };
    let own = first_run(&e, id);
    assert!(own.sz.is_none() && own.fill.is_none(), "{own:?}");
    slide_run(&mut e, &json!({"cmd":"select","ids":[id]}));
    slide_run(&mut e, &json!({"cmd":"copy"}));
    // Destination theme: the copy still inherits.
    let r = slide_run(&mut e, &json!({"cmd":"paste"}));
    let dest = first_run(&e, r.selection[0].id);
    assert!(dest.sz.is_none() && dest.fill.is_none(), "{dest:?}");
    slide_run(&mut e, &json!({"cmd":"undo"}));
    // Keep source: size and a literal colour written from the master's text styles.
    let r = slide_run(&mut e, &json!({"cmd":"paste","keepSource":true}));
    let kept = first_run(&e, r.selection[0].id);
    assert!(kept.sz.is_some(), "{kept:?}");
    let fill = serde_json::to_string(&kept.fill).unwrap();
    assert!(fill.contains("Srgb"), "{fill}");
    assert!(!fill.contains("Scheme"), "{fill}");
}

/// Style index of a cell of the first sheet.
fn style_of(e: &SheetEngine, r: u32, c: u32) -> u32 {
    e.workbook().unwrap().sheets[0]
        .cells
        .get(&(r, c))
        .map_or(0, |x| x.style)
}

#[test]
fn clipboard_history_items_paste_with_formats() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(
        0,
        &[
            (0, 0, "5".into()),
            (0, 1, "=A1*2".into()),
            (1, 0, "chữ".into()),
        ],
    )
    .unwrap();
    sheet_exec(
        &mut e,
        &json!({"cmd":"format","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":1},"patch":{"bold":true}}),
    );
    let first = sheet_exec(
        &mut e,
        &json!({"cmd":"copy","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":1}}),
    );
    let id = first["clipId"].as_u64().unwrap();
    // A later copy (even a cut) does not lose the earlier one.
    let second = sheet_exec(
        &mut e,
        &json!({"cmd":"copy","sheet":0,"range":{"r0":1,"c0":0,"r1":1,"c1":0},"cut":true}),
    );
    assert!(second["clipId"].as_u64().unwrap() > id);
    let r = sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":5,"col":2,"clip":id}),
    );
    assert_eq!(r["range"]["r0"], 5);
    // Formats and the formula (shifted) came with it.
    assert_eq!(e.cell(0, 5, 2).unwrap().input, "5");
    assert_eq!(e.cell(0, 5, 3).unwrap().input, "=C6*2");
    assert_eq!(style_of(&e, 5, 2), style_of(&e, 0, 0));
    assert_ne!(style_of(&e, 5, 2), 0);
    // The last copy is still the cut: pasting it moves the cell.
    sheet_exec(&mut e, &json!({"cmd":"paste","sheet":0,"row":8,"col":0}));
    assert_eq!(e.cell(0, 8, 0).unwrap().input, "chữ");
    assert_eq!(e.cell(0, 1, 0).unwrap().input, "");
    // The cut is kept in the history as a copy.
    let cut_id = second["clipId"].as_u64().unwrap();
    sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":9,"col":0,"clip":cut_id}),
    );
    assert_eq!(e.cell(0, 9, 0).unwrap().input, "chữ");
    assert_eq!(e.cell(0, 8, 0).unwrap().input, "chữ");
    // An unknown copy pastes nothing.
    let r = sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":0,"col":9,"clip":9999}),
    );
    assert!(r.is_null());
    // A new workbook forgets the history.
    e.new_workbook("u.xlsx").unwrap();
    let r = sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":0,"col":0,"clip":id}),
    );
    assert!(r.is_null());
}

#[test]
fn paste_special_column_widths_only() {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e.set_inputs(0, &[(0, 0, "1".into()), (0, 4, "giữ".into())])
        .unwrap();
    sheet_exec(
        &mut e,
        &json!({"cmd":"resize","sheet":0,"rows":false,"first":0,"last":0,"size":120.0}),
    );
    sheet_exec(
        &mut e,
        &json!({"cmd":"copy","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":0},"cut":true}),
    );
    let w0 = col_width(&e, 0).unwrap();
    let r = sheet_exec(
        &mut e,
        &json!({"cmd":"paste","sheet":0,"row":0,"col":4,"mode":"columnWidths"}),
    );
    assert!(!r.is_null());
    assert!((col_width(&e, 4).unwrap() - w0).abs() < 0.01);
    // The cells stay: the destination keeps its text, the cut source is not moved.
    assert_eq!(e.cell(0, 0, 4).unwrap().input, "giữ");
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "1");
    e.undo().unwrap();
    assert_eq!(col_width(&e, 4), None);
}

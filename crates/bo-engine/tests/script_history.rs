//! `historyLen` / `mergeHistory` (CORE-SCRIPT-001): a BlueScript run becomes one
//! undo step in BlueDoc, BlueSheet and BlueSlide.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use bo_engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;

fn doc(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn slide(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&v.to_string()).unwrap()).unwrap()
}

fn sheet(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn doc_steps_join_into_one() {
    let mut e = Engine::with_bundled_fonts();
    e.open(b"", "a.txt").unwrap();
    let since = doc(&mut e, &json!({ "cmd": "historyLen" }))["count"]
        .as_u64()
        .unwrap();
    for t in ["Một ", "hai ", "ba"] {
        doc(&mut e, &json!({ "cmd": "insertText", "text": t }));
        doc(&mut e, &json!({ "cmd": "splitParagraph" }));
    }
    let before = doc(&mut e, &json!({ "cmd": "historyLen" }))["count"]
        .as_u64()
        .unwrap();
    assert!(before >= since + 3);
    let r = doc(&mut e, &json!({ "cmd": "mergeHistory", "since": since }));
    assert_eq!(r["count"], json!(since + 1));
    doc(&mut e, &json!({ "cmd": "undo" }));
    let text = doc(&mut e, &json!({ "cmd": "speechText" }));
    let paras = text["speech"]["paras"].as_array().unwrap();
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0]["text"], json!(""));
    doc(&mut e, &json!({ "cmd": "redo" }));
    let text = doc(&mut e, &json!({ "cmd": "speechText" }));
    assert_eq!(text["speech"]["paras"].as_array().unwrap().len(), 4);
}

#[test]
fn sheet_steps_join_into_one() {
    let mut e = SheetEngine::new();
    e.new_workbook("s.xlsx").unwrap();
    let since = sheet(&mut e, &json!({ "cmd": "historyLen" }))["history"]
        .as_u64()
        .unwrap();
    e.set_inputs(0, &[(0, 0, "1".into())]).unwrap();
    e.structural(0, true, 0, 1, false).unwrap();
    e.set_inputs(0, &[(0, 0, "x".into())]).unwrap();
    let r = sheet(&mut e, &json!({ "cmd": "mergeHistory", "since": since }));
    assert_eq!(r["history"], json!(since + 1));
    assert!(e.undo().unwrap());
    assert!(!e.can_undo());
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "");
    assert!(e.redo().unwrap());
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "x");
    assert_eq!(e.cell(0, 1, 0).unwrap().input, "1");
}

#[test]
fn slide_steps_join_into_one() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-edit-arrange.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "deck.pptx").unwrap();
    let n = e.presentation().unwrap().slides.len();
    let since = slide(&mut e, &json!({ "cmd": "historyLen" }))["history"]
        .as_u64()
        .unwrap();
    slide(&mut e, &json!({ "cmd": "addSlide" }));
    slide(&mut e, &json!({ "cmd": "insertTextBox", "text": "A" }));
    slide(&mut e, &json!({ "cmd": "addSlide" }));
    let r = slide(&mut e, &json!({ "cmd": "mergeHistory", "since": since }));
    assert_eq!(r["history"], json!(since + 1));
    let u = slide(&mut e, &json!({ "cmd": "undo" }));
    assert_eq!(u["info"]["pageCount"], json!(n));
    assert_eq!(u["canUndo"], json!(since > 0));
    let r = slide(&mut e, &json!({ "cmd": "redo" }));
    assert_eq!(r["info"]["pageCount"], json!(n + 2));
}

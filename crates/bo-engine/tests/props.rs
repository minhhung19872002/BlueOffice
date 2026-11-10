//! Document properties of the File › Info page (CORE-UI-002): read, edit, save,
//! reopen for `.docx`, `.xlsx` and `.pptx` (ECMA-376 Part 2 §11 core properties,
//! Part 1 §22.2 extended properties).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{Engine, SheetEngine};
use bo_opc::{Limits, Package};
use serde_json::{json, Value};

fn corpus(rel: &str) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    std::fs::read(root.join(rel)).unwrap()
}

fn edits() -> Value {
    json!({
        "title": "Báo cáo quý <3> & tổng kết",
        "subject": "Chủ đề",
        "author": "Nguyễn Văn A",
        "keywords": "ngân sách; quý 3",
        "comments": "Ghi chú",
        "category": "Nội bộ",
        "status": "Bản nháp",
        "company": "Bluestar",
        "manager": "Trần B",
    })
}

fn check(props: &Value) {
    for (k, v) in edits().as_object().unwrap() {
        assert_eq!(&props[k], v, "{k}");
    }
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

#[test]
fn docx_properties_round_trip() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("doc/phase1-sample.docx"), "phase1-sample.docx")
        .unwrap();
    let r: Value =
        serde_json::from_str(&e.command_json(r#"{"cmd":"docProperties"}"#).unwrap()).unwrap();
    assert!(r["docProperties"].is_object());
    // Unchanged properties keep the saved package's properties part as it was.
    let before = e.save_docx().unwrap();
    let cmd = json!({ "cmd": "setDocProperties", "props": edits() }).to_string();
    let r: Value = serde_json::from_str(&e.command_json(&cmd).unwrap()).unwrap();
    check(&r["docProperties"]);
    let saved = e.save_docx().unwrap();
    assert_ne!(before, saved);
    let core = part(&saved, "/docProps/core.xml");
    assert!(
        core.contains("Báo cáo quý &lt;3&gt; &amp; tổng kết"),
        "{core}"
    );
    let mut again = Engine::with_bundled_fonts();
    again.open(&saved, "again.docx").unwrap();
    let r: Value =
        serde_json::from_str(&again.command_json(r#"{"cmd":"docProperties"}"#).unwrap()).unwrap();
    check(&r["docProperties"]);
    // Clearing a property removes it; the others stay.
    let cmd = json!({ "cmd": "setDocProperties", "props": { "title": "" } }).to_string();
    again.command_json(&cmd).unwrap();
    let cleared = again.save_docx().unwrap();
    assert!(!part(&cleared, "/docProps/core.xml").contains("title>"));
    let mut third = Engine::with_bundled_fonts();
    third.open(&cleared, "third.docx").unwrap();
    assert_eq!(third.doc_properties().title, "");
    assert_eq!(third.doc_properties().author, "Nguyễn Văn A");
}

#[test]
fn xlsx_properties_round_trip() {
    let mut e = SheetEngine::new();
    e.open(&corpus("sheet/cf-1.xlsx"), "cf-1.xlsx").unwrap();
    let r: Value =
        serde_json::from_str(&e.exec_json(r#"{"cmd":"docProperties"}"#).unwrap()).unwrap();
    assert!(r.is_object());
    let cmd = json!({ "cmd": "setDocProperties", "props": edits() }).to_string();
    let r: Value = serde_json::from_str(&e.exec_json(&cmd).unwrap()).unwrap();
    check(&r);
    let saved = e.save("xlsx", 0).unwrap();
    let app = part(&saved, "/docProps/app.xml");
    assert!(app.contains("<Company>Bluestar</Company>"), "{app}");
    let mut again = SheetEngine::new();
    again.open(&saved, "again.xlsx").unwrap();
    let r: Value =
        serde_json::from_str(&again.exec_json(r#"{"cmd":"docProperties"}"#).unwrap()).unwrap();
    check(&r);
}

#[test]
fn pptx_properties_round_trip() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("slide/slide-anim-fade-appear.pptx"), "deck.pptx")
        .unwrap();
    let cmd = json!({ "cmd": "setDocProperties", "props": edits() }).to_string();
    let r: Value = serde_json::from_str(&e.slide_command_json(&cmd).unwrap()).unwrap();
    check(&r["docProperties"]);
    let saved = e.save_pptx().unwrap();
    let mut again = Engine::with_bundled_fonts();
    again.open(&saved, "again.pptx").unwrap();
    let r: Value = serde_json::from_str(
        &again
            .slide_command_json(r#"{"cmd":"docProperties"}"#)
            .unwrap(),
    )
    .unwrap();
    check(&r["docProperties"]);
}

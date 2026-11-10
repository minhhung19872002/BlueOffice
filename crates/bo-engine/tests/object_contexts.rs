//! The object kind of the selection that drives the contextual ribbon tabs (CORE-UI-008):
//! `format.ole` in BlueDoc (embedded object, ECMA-376 Part 1 §17.3.3.19 object) and the
//! `equation` flag (`a14:m`, [MS-ODRAWXML]) of a selected BlueSlide shape.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn doc_cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(v.get("error").is_none(), "{json}: {v}");
    v
}

fn slide_cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap();
    assert!(v.get("error").is_none(), "{json}: {v}");
    v
}

#[test]
fn doc_format_flags_the_embedded_object_at_the_selection() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let v = doc_cmd(&mut e, r#"{"cmd":"insertText","text":"Bảng: "}"#);
    assert_eq!(v["format"]["ole"], false);
    doc_cmd(&mut e, r#"{"cmd":"oleInsert"}"#);
    // The caret follows the new object: it is the object at the selection.
    let v = doc_cmd(&mut e, r#"{"cmd":"state"}"#);
    assert_eq!(v["format"]["ole"], true, "{}", v["format"]);
    // A new paragraph leaves it.
    let v = doc_cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    assert_eq!(v["format"]["ole"], false);
}

#[test]
fn slide_selection_flags_an_equation_shape() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck("slide-equation.pptx"), "e.pptx").unwrap();
    let v = slide_cmd(&mut e, r#"{"cmd":"select","ids":[5]}"#);
    let sel = &v["selection"][0];
    assert_eq!(sel["id"], 5, "{v}");
    assert_eq!(sel["kind"], "shape", "{sel}");
    assert_eq!(sel["equation"], true, "{sel}");
    // The title text box holds no equation: the flag is left out.
    let v = slide_cmd(&mut e, r#"{"cmd":"select","ids":[20]}"#);
    assert_eq!(v["selection"][0]["id"], 20, "{v}");
    assert!(v["selection"][0].get("equation").is_none(), "{v}");
    // Saved and reopened: still an equation shape.
    let saved = e.save_pptx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&saved, "e.pptx").unwrap();
    let v = slide_cmd(&mut back, r#"{"cmd":"select","ids":[5]}"#);
    assert_eq!(v["selection"][0]["equation"], true, "{v}");
}

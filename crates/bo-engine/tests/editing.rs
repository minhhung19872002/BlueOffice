//! Editing through the JSON command bridge (the same path the web and desktop UIs use).

use bo_engine::{EditResult, Engine};

fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

#[test]
fn type_vietnamese_bold_save_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("Văn bản mới.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":"Xin chào Việt Nam"}"#);
    assert_eq!(r["selection"]["focus"]["o"], "Xin chào Việt Nam".len());
    assert!(r["modified"].as_bool().unwrap());
    assert_eq!(r["dirty"], serde_json::json!([0]));
    let caret = &r["caret"];
    assert!(caret["x"].as_f64().unwrap() > 85.0);
    // Select "chào" and bold it.
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":4},"focus":{"p":0,"o":9}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert!(r["format"]["bold"].as_bool().unwrap());
    assert_eq!(r["rects"].as_array().unwrap().len(), 1);
    let saved = e.save_docx().unwrap();
    e.mark_saved();
    assert!(!cmd(&mut e, r#"{"cmd":"state"}"#)["modified"]
        .as_bool()
        .unwrap());

    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "Văn bản mới.docx").unwrap();
    let doc = f.document().unwrap();
    assert_eq!(doc.plain_text(), "Xin chào Việt Nam");
    let p = doc.paragraphs().next().unwrap();
    assert!(p
        .runs
        .iter()
        .any(|r| r.props.bold == Some(true) && r.plain_text() == "chào"));
    assert!(f.page_json(0).unwrap().contains("chào"));
}

#[test]
fn mouse_keyboard_undo_find_clipboard() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Dòng một\nDòng hai".as_bytes(), "a.txt").unwrap();
    // Click at the start of the second line.
    let r: EditResult = e
        .command(serde_json::from_str(r#"{"cmd":"click","page":0,"x":0,"y":95}"#).unwrap())
        .unwrap();
    assert_eq!((r.selection.focus.para, r.selection.focus.offset), (1, 0));
    let r = cmd(&mut e, r#"{"cmd":"move","motion":"up"}"#);
    assert_eq!(r["selection"]["focus"]["p"], 0);
    let r = cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd","extend":true}"#);
    assert_eq!(r["selection"]["focus"]["o"], "Dòng một".len());
    let r = cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":135,"y":95,"count":2}"#,
    );
    assert_eq!(r["selection"]["anchor"]["p"], 1);
    let r = cmd(&mut e, r#"{"cmd":"copy"}"#);
    assert_eq!(r["clip"]["text"], "hai");
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let r = cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"paste","internal":{}}}"#,
            serde_json::to_string(r["clip"]["internal"].as_str().unwrap()).unwrap()
        ),
    );
    assert!(r["error"].is_null());
    assert_eq!(e.document().unwrap().plain_text(), "Dòng một\nDòng haihai");
    let r = cmd(&mut e, r#"{"cmd":"find","query":"dòng"}"#);
    assert_eq!(r["matches"].as_array().unwrap().len(), 2);
    let r = cmd(
        &mut e,
        r#"{"cmd":"replaceAll","query":"Dòng","with":"Hàng","options":{"matchCase":true}}"#,
    );
    assert_eq!(r["count"], 2);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Dòng một\nDòng hai");
    let r = cmd(&mut e, r#"{"cmd":"redo"}"#);
    assert!(r["format"]["canUndo"].as_bool().unwrap());
    let r = cmd(&mut e, r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    assert!(r["error"].is_null());
    assert!(e.command_json(r#"{"cmd":"bogus"}"#).is_err());
}

#[test]
fn bad_commands_and_images() {
    let mut e = Engine::with_bundled_fonts();
    assert!(e.command_json(r#"{"cmd":"state"}"#).is_err()); // no document
    e.new_document("x.docx").unwrap();
    assert!(e.command_json("{").is_err());
    let r = cmd(&mut e, r#"{"cmd":"applyStyle","id":"Nope"}"#);
    assert!(r["error"].is_string());
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 96, 48);
        enc.set_color(png::ColorType::Rgb);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&vec![200; 96 * 48 * 3]).unwrap();
    }
    let r = e.insert_image(png, "a.png").unwrap();
    assert!(r.error.is_none());
    assert!(e.page_json(0).unwrap().contains(r#""op":"image""#));
    assert!(e.media("/word/media/image1.png").is_some());
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "x.docx").unwrap();
    assert_eq!(f.document().unwrap().media.len(), 1);
    assert!(e.insert_image(vec![1, 2, 3], "x.gif").is_err());
}

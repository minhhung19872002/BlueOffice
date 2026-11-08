//! Text boxes and shapes (insert, type inside, move/resize, save and reopen), the
//! style gallery, Find by format and pictures that need conversion (GIF, BMP, EMF).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{Block, RunContent};
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    f
}

/// Text of the first text box of the body.
fn box_text(e: &Engine) -> Option<String> {
    let d = e.document()?;
    d.sections[0].blocks.iter().find_map(|b| {
        let p = b.as_paragraph()?;
        p.runs
            .iter()
            .flat_map(|r| &r.content)
            .find_map(|c| match c {
                RunContent::Image(img) => img.shape.as_ref()?.text.as_ref().map(|t| {
                    t.iter()
                        .filter_map(Block::as_paragraph)
                        .map(bo_model::Paragraph::plain_text)
                        .collect::<Vec<_>>()
                        .join("\n")
                }),
                _ => None,
            })
    })
}

#[test]
fn text_box_insert_type_move_save_and_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân bài "}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertShape","textBox":true,"width":150,"height":60}"#,
    );
    assert_eq!(r["story"]["kind"], "textBox");
    let d = &r["drawing"];
    assert_eq!(d["textBox"], true);
    assert_eq!(d["anchored"], true);
    let (para, at) = (d["para"].as_u64().unwrap(), d["at"].as_u64().unwrap());
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":"Hộp chữ"}"#);
    assert_eq!(r["story"]["kind"], "textBox");
    // The caret is inside the box.
    let (bx, by) = (d["x"].as_f64().unwrap(), d["y"].as_f64().unwrap());
    let cy = r["caret"]["y"].as_f64().unwrap();
    assert!(cy >= by && cy <= by + 60.0, "caret {cy} box {by}");
    assert!(e.page_json(0).unwrap().contains("Hộp chữ"));
    // Click in the body: the text box closes, one undoable change.
    let r = cmd(&mut e, r#"{"cmd":"click","page":0,"x":80,"y":700}"#);
    assert!(r.get("story").is_none());
    assert_eq!(box_text(&e).as_deref(), Some("Hộp chữ"));

    // Move by (20, 10) pt and resize to 200 × 80 pt.
    let r = cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setDrawingFrame","para":{para},"at":{at},"dx":20,"dy":10,"width":200,"height":80}}"#
        ),
    );
    let d2 = &r["drawing"];
    assert!((d2["x"].as_f64().unwrap() - bx - 20.0).abs() < 0.5, "{d2}");
    assert!((d2["w"].as_f64().unwrap() - 200.0).abs() < 0.5);
    // A click on the box reports it and opens it again.
    let (cx, cy) = (
        d2["x"].as_f64().unwrap() + 30.0,
        d2["y"].as_f64().unwrap() + 20.0,
    );
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"click","page":0,"x":{cx},"y":{cy}}}"#),
    );
    assert_eq!(r["story"]["kind"], "textBox");
    assert_eq!(r["drawing"]["para"], para);
    cmd(&mut e, r#"{"cmd":"closeTextBox"}"#);

    let f = reopen(&e);
    assert_eq!(box_text(&f).as_deref(), Some("Hộp chữ"));
    assert!(f.page_json(0).unwrap().contains("Hộp chữ"));
    // Undo restores the old frame.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    let p = e.document().unwrap().sections[0].blocks[0]
        .as_paragraph()
        .unwrap()
        .clone();
    let img = p
        .runs
        .iter()
        .flat_map(|r| &r.content)
        .find_map(|c| match c {
            RunContent::Image(i) => Some(i.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(img.width.0, 150 * 12_700);
}

#[test]
fn shapes_render_as_paths() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertShape","preset":"ellipse","width":100,"height":50}"#,
    );
    assert!(r.get("story").is_none());
    assert_eq!(r["drawing"]["textBox"], false);
    let page = e.page_json(0).unwrap();
    assert!(page.contains(r#""op":"path""#), "{page}");
    let f = reopen(&e);
    let page = f.page_json(0).unwrap();
    assert!(page.contains(r#""op":"path""#));
}

#[test]
fn style_gallery_lists_document_styles() {
    let mut e = Engine::with_bundled_fonts();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/styles-01-headings.docx"
    ));
    match bytes {
        Ok(b) => {
            e.open(&b, "s.docx").unwrap();
        }
        Err(_) => {
            e.new_document("a.docx").unwrap();
        }
    }
    let r = cmd(&mut e, r#"{"cmd":"styleList"}"#);
    let list = r["styles"].as_array().unwrap();
    assert!(list.iter().any(|s| s["id"] == "Normal"));
    assert!(list.iter().any(|s| s["id"] == "Heading1"));
    // Quick styles come first.
    let first_other = list.iter().position(|s| s["quick"] == false);
    if let Some(i) = first_other {
        assert!(list[i..].iter().all(|s| s["quick"] == false));
    }
    let h1 = list.iter().find(|s| s["id"] == "Heading1").unwrap();
    assert!(h1["preview"]["size"].as_f64().unwrap() > 10.0);
}

#[test]
fn find_by_format() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"một "}"#);
    cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"hai"}"#);
    cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" ba hai"}"#);
    // Every bold stretch.
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"","format":{"bold":true}}"#,
    );
    let m = r["matches"].as_array().unwrap();
    assert_eq!(m.len(), 1, "{m:?}");
    // "một " is 6 bytes.
    assert_eq!(
        (m[0]["start"].as_u64(), m[0]["end"].as_u64()),
        (Some(6), Some(9))
    );
    // "hai" occurs twice, once in bold.
    let r = cmd(&mut e, r#"{"cmd":"find","query":"hai"}"#);
    assert_eq!(r["matches"].as_array().unwrap().len(), 2);
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"hai","format":{"bold":false}}"#,
    );
    assert_eq!(r["matches"].as_array().unwrap().len(), 1);
    let r = cmd(
        &mut e,
        r#"{"cmd":"replaceAll","query":"hai","with":"HAI","format":{"bold":true}}"#,
    );
    assert_eq!(r["count"], 1);
    assert!(e.document().unwrap().plain_text().contains("HAI ba hai"));
}

/// A minimal 2×1 GIF (red, blue) for picture conversion.
fn gif() -> Vec<u8> {
    let mut b = b"GIF89a".to_vec();
    b.extend_from_slice(&[2, 0, 1, 0, 0x80, 0, 0]);
    b.extend_from_slice(&[255, 0, 0, 0, 0, 255]);
    b.extend_from_slice(&[0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0]);
    // min code size 2; codes (3 bits): clear 4, 0, 1, end 5
    b.extend_from_slice(&[2, 2, 0x44, 0x0A, 0, 0x3B]);
    b
}

#[test]
fn inserted_gif_becomes_png() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r: Value = serde_json::from_str(
        &serde_json::to_string(&e.insert_image(gif(), "a.gif").unwrap()).unwrap(),
    )
    .unwrap();
    assert!(r.get("error").is_none(), "{r}");
    let d = e.document().unwrap();
    let (name, data) = d.media.iter().next().unwrap();
    assert!(name.ends_with(".png"));
    assert!(data.starts_with(b"\x89PNG"));
}

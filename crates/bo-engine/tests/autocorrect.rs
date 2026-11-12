//! AutoCorrect as you type through the JSON command bridge (CORE-AUTOCORR-001).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::{json, Value};

fn cmd(e: &mut Engine, json: &str) -> Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn type_text(e: &mut Engine, text: &str) -> Vec<Value> {
    text.chars()
        .map(|c| {
            cmd(
                e,
                &json!({"cmd": "insertText", "text": c.to_string()}).to_string(),
            )
        })
        .collect()
}

fn para0(e: &mut Engine) -> String {
    let r = cmd(e, r#"{"cmd":"selectAll"}"#);
    let _ = r;
    let c = cmd(e, r#"{"cmd":"copy"}"#);
    c["clip"]["text"]
        .as_str()
        .unwrap_or_default()
        .trim_end()
        .to_owned()
}

fn engine() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(b"", "a.txt").unwrap();
    e
}

#[test]
fn off_until_the_host_sets_options() {
    let mut e = engine();
    let r = cmd(&mut e, r#"{"cmd":"autoCorrect"}"#);
    assert_eq!(r["autocorrect"]["enabled"], json!(false));
    assert!(r["autocorrect"]["entries"].as_array().unwrap().len() > 10);
    let langs = &r["autocorrect"]["languageEntries"];
    assert!(langs["vi-VN"].as_array().unwrap().len() > 15);
    assert!(langs["en-US"].as_array().unwrap().len() > 20);
    type_text(&mut e, "teh ");
    assert_eq!(para0(&mut e), "teh");
}

#[test]
fn corrects_as_you_type_with_its_own_undo_step() {
    let mut e = engine();
    let r = cmd(&mut e, r#"{"cmd":"autoCorrect","reset":true}"#);
    assert_eq!(r["autocorrect"]["enabled"], json!(true));
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    let rs = type_text(&mut e, "đây là teh ");
    // "đây " got its capital; "teh " was replaced.
    assert_eq!(rs[3]["corrected"], json!("capitalize"));
    assert_eq!(rs.last().unwrap()["corrected"], json!("replace"));
    assert_eq!(para0(&mut e), "Đây là the");
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(para0(&mut e), "Đây là teh");
    // Enter corrects the paragraph it ends.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    type_text(&mut e, "(c)");
    let r = cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    assert!(r["corrected"].is_null(), "symbol replaced on ')': {r}");
    assert!(para0(&mut e).contains("teh ©"));
}

#[test]
fn options_from_the_host() {
    let mut e = engine();
    let opts = json!({"cmd": "autoCorrect", "options": {
        "capitalizeSentences": false,
        "entries": [{"from": "bxd", "to": "Bộ Xây dựng"}],
        "languageEntries": {},
    }});
    let r = cmd(&mut e, &opts.to_string());
    assert_eq!(r["autocorrect"]["enabled"], json!(true));
    assert_eq!(r["autocorrect"]["capitalizeSentences"], json!(false));
    assert_eq!(r["autocorrect"]["entries"].as_array().unwrap().len(), 1);
    type_text(&mut e, "theo bxd teh ");
    assert_eq!(para0(&mut e), "theo Bộ Xây dựng teh");
}

/// Text of the story being edited (select all + copy inside it).
fn story_text(e: &mut Engine) -> String {
    cmd(e, r#"{"cmd":"selectAll"}"#);
    let c = cmd(e, r#"{"cmd":"copy"}"#);
    c["clip"]["text"]
        .as_str()
        .unwrap_or_default()
        .trim_end()
        .to_owned()
}

#[test]
fn corrects_in_headers_notes_and_text_boxes() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"autoCorrect","reset":true}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân bài"}"#);
    // Header (§17.10).
    let r = cmd(
        &mut e,
        r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#,
    );
    assert_eq!(r["story"]["kind"], "header");
    let rs = type_text(&mut e, "đầu trang teh (c)");
    assert_eq!(rs.last().unwrap()["story"]["kind"], "header");
    assert_eq!(story_text(&mut e), "Đầu trang the ©");
    // Undo inside the header reverts only the last correction.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(story_text(&mut e), "Đầu trang the (c)");
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    // Footnote (§17.11).
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertNote","endnote":false,"text":""}"#);
    let id = *e.document().unwrap().footnotes.keys().next().unwrap();
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"editNote","endnote":false,"id":{id}}}"#),
    );
    assert!(r["story"].is_object(), "{r}");
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    type_text(&mut e, " xem thêm teh ");
    assert!(
        story_text(&mut e).ends_with("Xem thêm the"),
        "{}",
        story_text(&mut e)
    );
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    // Text box (§20.4 wps:txbx): Enter after "1. " ends the paragraph it corrects.
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":80,"y":700}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertShape","textBox":true,"width":150,"height":60}"#,
    );
    assert_eq!(r["story"]["kind"], "textBox");
    type_text(&mut e, "hộp teh");
    let r = cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    assert_eq!(r["story"]["kind"], "textBox");
    assert_eq!(story_text(&mut e), "Hộp the");
    cmd(&mut e, r#"{"cmd":"closeTextBox"}"#);
    // Header and note text are in the document.
    let d = e.document().unwrap();
    let text = |b: &Vec<bo_model::Block>| {
        b.iter()
            .filter_map(bo_model::Block::as_paragraph)
            .map(bo_model::Paragraph::plain_text)
            .collect::<String>()
    };
    let header = d.sections[0].headers.default.as_ref().map(text);
    assert_eq!(header.as_deref(), Some("Đầu trang the (c)"));
    assert!(d
        .footnotes
        .values()
        .any(|b| text(b).ends_with("Xem thêm the ")));
}

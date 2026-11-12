//! Review display modes over the command bridge: simple / all / no markup /
//! original, markup options and the page markup sent with each page.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn page(e: &Engine) -> Value {
    serde_json::from_str(&e.page_json(0).unwrap()).unwrap()
}

/// "Hợp đồng số 1" with "số 1" replaced by "mới" as tracked changes, and a comment.
fn reviewed() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"An"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Hợp đồng số 1"}"#);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    let (a, b) = ("Hợp đồng ".len(), "Hợp đồng số 1".len());
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{a}}},"focus":{{"p":0,"o":{b}}}}}}}"#
        ),
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"mới"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":3}}}"#,
    );
    cmd(&mut e, r#"{"cmd":"addComment","text":"Kiểm tra"}"#);
    e
}

#[test]
fn all_markup_draws_bars_and_comment_balloons() {
    let e = reviewed();
    let p = page(&e);
    let m = &p["markup"];
    assert_eq!(m["width"], 180.0);
    assert_eq!(m["bars"].as_array().unwrap().len(), 1);
    let b = m["balloons"].as_array().unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0]["kind"], "comment");
    assert_eq!(b[0]["text"], "Kiểm tra");
    assert!(b[0]["x"].as_f64().unwrap() >= p["width"].as_f64().unwrap());
    // The display list itself keeps the page size (printing / PDF are unaffected).
    assert!(p["width"].as_f64().unwrap() < 600.0);
}

#[test]
fn modes_switch_text_and_markup() {
    let mut e = reviewed();
    // Simple markup: accepted text, red bar, comment as an icon.
    let r = cmd(&mut e, r#"{"cmd":"setMarkup","mode":"simple"}"#);
    assert_eq!(r["review"]["markup"]["mode"], "simple");
    assert_eq!(r["review"]["view"], "final");
    assert_eq!(r["dirty"], serde_json::json!([0]));
    let p = page(&e);
    let s = p.to_string();
    assert!(s.contains("mới") && !s.contains("số 1"));
    assert_eq!(p["markup"]["bars"][0]["color"], "#d02121");
    assert_eq!(p["markup"]["balloons"][0]["icon"], true);
    // No markup: accepted text, nothing drawn beside the page.
    cmd(&mut e, r#"{"cmd":"setMarkup","mode":"none"}"#);
    let p = page(&e);
    assert!(p.get("markup").is_none());
    assert!(!p.to_string().contains("số 1"));
    // Original: rejected text.
    cmd(&mut e, r#"{"cmd":"setMarkup","mode":"original"}"#);
    let s = page(&e).to_string();
    assert!(s.contains("số 1") && !s.contains("mới"));
    // Legacy view command maps onto the modes.
    let r = cmd(&mut e, r#"{"cmd":"setRevisionView","view":"markup"}"#);
    assert_eq!(r["review"]["markup"]["mode"], "all");
}

#[test]
fn markup_options_filter_marks() {
    let mut e = reviewed();
    // Comments hidden: no balloons, so no markup area; bars stay.
    cmd(&mut e, r#"{"cmd":"setMarkup","comments":false}"#);
    let p = page(&e);
    assert_eq!(p["markup"]["width"], 0.0);
    assert_eq!(p["markup"]["bars"].as_array().unwrap().len(), 1);
    // Deletions in balloons: the deleted text leaves the line for a balloon.
    let r = cmd(&mut e, r#"{"cmd":"setMarkup","deletionBalloons":true}"#);
    assert_eq!(r["review"]["markup"]["deletionBalloons"], true);
    let p = page(&e);
    let b = p["markup"]["balloons"].as_array().unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0]["kind"], "delete");
    assert_eq!(b[0]["text"], "số 1");
    // Hiding the only reviewer: nothing marked, the accepted text inline.
    cmd(
        &mut e,
        r#"{"cmd":"setMarkup","deletionBalloons":false,"hiddenReviewers":["An"]}"#,
    );
    let p = page(&e);
    assert!(p.get("markup").is_none());
    assert!(!p.to_string().contains("số 1"));
    // The document itself still holds both revisions.
    let r = cmd(&mut e, r#"{"cmd":"review"}"#);
    assert_eq!(r["review"]["revisions"].as_array().unwrap().len(), 2);
}

/// Changes in a header or a footnote get balloons too: beside the header's line on the
/// page, beside the note's line at the page bottom (§17.10, §17.11; all markup only).
#[test]
fn header_and_footnote_changes_get_balloons() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"An"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân"}"#);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    cmd(
        &mut e,
        r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đầu trang mới"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    cmd(&mut e, r#"{"cmd":"insertNote","text":"Ghi chú"}"#);
    let p = page(&e);
    let b = p["markup"]["balloons"].as_array().unwrap();
    let header = b
        .iter()
        .find(|x| x["text"].as_str().unwrap_or("").contains("Đầu trang"))
        .unwrap_or_else(|| panic!("{b:?}"));
    let note = b
        .iter()
        .find(|x| x["text"].as_str().unwrap_or("").contains("Ghi chú"))
        .unwrap_or_else(|| panic!("{b:?}"));
    let h = p["height"].as_f64().unwrap();
    assert!(header["anchorY"].as_f64().unwrap() < h / 4.0, "{header}");
    assert!(note["anchorY"].as_f64().unwrap() > h / 2.0, "{note}");
    // No markup or simple markup: no balloons for them.
    cmd(&mut e, r#"{"cmd":"setMarkup","mode":"simple"}"#);
    let p = page(&e);
    let none = p["markup"]["balloons"]
        .as_array()
        .is_none_or(|b| b.iter().all(|x| x["kind"] == "comment"));
    assert!(none, "{p}");
}

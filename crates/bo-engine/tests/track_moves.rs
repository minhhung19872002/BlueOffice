//! Tracked moves, formatting change details and locked tracking over the command
//! bridge (ECMA-376 Part 1, §17.13.5.22–27 moveFrom / moveTo and their ranges,
//! §17.13.5.31 rPrChange, §17.15.1.29 documentProtection `w:edit="trackedChanges"`,
//! §17.15.1.89 trackRevisions), with `.docx` round trips.
//!
//! Set `BO_TRACK_MOVES_OUT=<dir>` to keep the written files (OpenXmlValidator:
//! `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn texts(e: &Engine) -> Vec<String> {
    let s = e.session().unwrap();
    (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect()
}

fn kinds(r: &Value) -> Vec<String> {
    r["review"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_TRACK_MOVES_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

/// "Một", "Hai", "Ba" as three paragraphs, tracking on, author "An".
fn three() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"An"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Một"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Hai"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Ba"}"#);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    e
}

fn caret(e: &mut Engine, p: usize) {
    cmd(
        e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":0}},"focus":{{"p":{p},"o":0}}}}}}"#
        ),
    );
}

#[test]
fn corpus_move_is_accepted_as_a_unit() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/review-01-insert-delete.docx"
    );
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), "review-01.docx")
        .unwrap();
    let r = cmd(&mut e, r#"{"cmd":"review"}"#);
    let revs = r["review"]["revisions"].as_array().unwrap().clone();
    // moveTo w:id="110" inside moveToRangeStart w:name="move1".
    let to = revs.iter().find(|x| x["kind"] == "moveTo").unwrap();
    assert_eq!(to["id"], 110);
    let r = cmd(&mut e, r#"{"cmd":"acceptRevision","id":110}"#);
    let k = kinds(&r);
    assert!(!k.iter().any(|k| k == "moveFrom" || k == "moveTo"), "{k:?}");
    assert_eq!(texts(&e)[2], " Câu giữ nguyên.");
    assert_eq!(texts(&e)[3], "Phụ lục. Câu được di chuyển.");
    // The range markers of the resolved move are gone from the saved file.
    let saved = e.save_docx().unwrap();
    let xml = bo_opc::Package::read(&saved, &bo_opc::Limits::default())
        .unwrap()
        .part("/word/document.xml")
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        .unwrap();
    assert!(!xml.contains("moveFromRange") && !xml.contains("moveToRange"));
}

#[test]
fn tracked_paragraph_move_round_trips() {
    let mut e = three();
    caret(&mut e, 0);
    let r = cmd(&mut e, r#"{"cmd":"moveParagraphs","up":false}"#);
    assert_eq!(kinds(&r), ["moveFrom", "moveTo"]);
    assert_eq!(texts(&e), ["Một", "Hai", "Một", "Ba"]);
    let saved = e.save_docx().unwrap();
    keep("track-move-paragraph.docx", &saved);
    let xml = bo_opc::Package::read(&saved, &bo_opc::Limits::default())
        .unwrap()
        .part("/word/document.xml")
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        .unwrap();
    for tag in [
        "<w:moveFromRangeStart",
        "<w:moveFromRangeEnd",
        "<w:moveToRangeStart",
        "<w:moveToRangeEnd",
        "<w:moveFrom ",
        "<w:moveTo ",
    ] {
        assert!(xml.contains(tag), "{tag}");
    }
    // Reopened: the halves pair by range name; rejecting the moveFrom rejects both.
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "moved.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    // trackRevisions was saved: tracking is on again.
    assert_eq!(r["review"]["trackChanges"], true);
    assert_eq!(kinds(&r), ["moveFrom", "moveTo"]);
    let from = r["review"]["revisions"][0]["id"].as_i64().unwrap();
    let r = cmd(
        &mut f,
        &format!(r#"{{"cmd":"rejectRevision","id":{from}}}"#),
    );
    assert_eq!(kinds(&r).len(), 0);
    assert_eq!(texts(&f), ["Một", "Hai", "Ba"]);
}

#[test]
fn formatting_change_lists_its_properties() {
    let mut e = three();
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":1,"o":0},"focus":{"p":1,"o":3}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    let rev = &r["review"]["revisions"][0];
    assert_eq!(rev["kind"], "format");
    assert_eq!(rev["detail"], serde_json::json!(["bold"]));
    let saved = e.save_docx().unwrap();
    keep("track-format.docx", &saved);
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "f.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    assert_eq!(
        r["review"]["revisions"][0]["detail"],
        serde_json::json!(["bold"])
    );
}

#[test]
fn locked_tracking_needs_the_password() {
    let mut e = three();
    caret(&mut e, 1);
    cmd(&mut e, r#"{"cmd":"insertText","text":"x"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"protectDocument","edit":"trackedChanges","password":"mật khẩu"}"#,
    );
    assert_eq!(r["review"]["trackingLocked"], true);
    assert_eq!(r["review"]["lockPassword"], true);
    let r = cmd(&mut e, r#"{"cmd":"setTrackChanges","on":false}"#);
    assert_eq!(r["error"], "review:locked");
    assert_eq!(r["review"]["trackChanges"], true);
    let r = cmd(&mut e, r#"{"cmd":"acceptAllRevisions"}"#);
    assert_eq!(r["error"], "review:locked");
    let r = cmd(&mut e, r#"{"cmd":"review"}"#);
    assert_eq!(kinds(&r), ["insert"]);
    let saved = e.save_docx().unwrap();
    keep("track-locked.docx", &saved);
    let settings = bo_opc::Package::read(&saved, &bo_opc::Limits::default())
        .unwrap()
        .part("/word/settings.xml")
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        .unwrap();
    assert!(settings.contains("<w:trackRevisions/>"), "{settings}");
    assert!(settings.contains("w:edit=\"trackedChanges\""), "{settings}");
    assert!(
        settings.contains("w:algorithmName=\"SHA-512\""),
        "{settings}"
    );
    // §17.15.1.89 trackRevisions precedes §17.15.1.29 documentProtection.
    assert!(settings.find("trackRevisions") < settings.find("documentProtection"));
    // Reopened: still locked and tracking.
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "locked.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    assert_eq!(r["review"]["trackingLocked"], true);
    assert_eq!(r["review"]["trackChanges"], true);
    let r = cmd(&mut f, r#"{"cmd":"unprotectDocument","password":"sai"}"#);
    assert_eq!(r["error"], "forms:password");
    let r = cmd(
        &mut f,
        r#"{"cmd":"unprotectDocument","password":"mật khẩu"}"#,
    );
    assert_eq!(r["review"]["trackingLocked"], false);
    let r = cmd(&mut f, r#"{"cmd":"acceptAllRevisions"}"#);
    assert!(r.get("error").is_none_or(Value::is_null), "{r}");
    assert_eq!(kinds(&r).len(), 0);
    assert_eq!(texts(&f)[1], "xHai");
}

/// Locked tracking holds in headers, footers and notes too: what is typed there is
/// tracked, and while the header is open its changes cannot be accepted or rejected,
/// nor tracking turned off.
#[test]
fn locked_tracking_covers_headers_and_notes() {
    let mut e = three();
    cmd(
        &mut e,
        r#"{"cmd":"protectDocument","edit":"trackedChanges","password":"pw"}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đầu trang"}"#);
    for c in [
        r#"{"cmd":"acceptAllRevisions"}"#,
        r#"{"cmd":"rejectAllRevisions"}"#,
        r#"{"cmd":"setTrackChanges","on":false}"#,
    ] {
        let r = cmd(&mut e, c);
        assert_eq!(r["error"], "review:locked", "{c}");
    }
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let r = cmd(&mut e, r#"{"cmd":"review"}"#);
    let revs = r["review"]["revisions"].as_array().unwrap();
    assert!(
        revs.iter()
            .any(|x| x["kind"] == "insert" && x["story"]["kind"] == "header"),
        "{revs:?}"
    );
    // A footnote typed while locked is tracked as well; accepting it is refused.
    caret(&mut e, 0);
    cmd(&mut e, r#"{"cmd":"insertNote","text":"chú"}"#);
    let r = cmd(&mut e, r#"{"cmd":"review"}"#);
    let revs = r["review"]["revisions"].as_array().unwrap();
    assert!(
        revs.iter().any(|x| x["story"]["kind"] == "footnote"),
        "{revs:?}"
    );
    let r = cmd(&mut e, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(r["error"], "review:locked");
}

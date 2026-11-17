//! Restrict Editing (ECMA-376 Part 1, §17.15.1.29 documentProtection) started from the
//! pane: `readOnly` refuses changes, `comments` lets only comments in, Stop Protection
//! with the password lifts it.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;

fn run(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn doc() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    run(&mut e, r#"{"cmd":"insertText","text":"Hello world"}"#);
    e
}

fn text(e: &Engine) -> String {
    format!("{:?}", e.document().unwrap().sections[0].blocks)
}

#[test]
fn read_only_refuses_changes_until_stopped() {
    let mut e = doc();
    let v = run(
        &mut e,
        r#"{"cmd":"protectDocument","edit":"readOnly","password":"pw"}"#,
    );
    assert!(v.get("error").is_none(), "{v}");
    let before = text(&e);
    let v = run(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
    assert!(v.get("error").is_some(), "{v}");
    assert_eq!(text(&e), before);
    let v = run(&mut e, r#"{"cmd":"deleteBackward"}"#);
    assert!(v.get("error").is_some(), "{v}");
    // Comments are changes too in read-only mode.
    run(&mut e, r#"{"cmd":"selectAll"}"#);
    let v = run(&mut e, r#"{"cmd":"addComment","text":"note"}"#);
    assert!(v.get("error").is_some(), "{v}");
    // A wrong password keeps it; the right one stops it.
    run(&mut e, r#"{"cmd":"unprotectDocument","password":"nope"}"#);
    let v = run(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
    assert!(v.get("error").is_some(), "{v}");
    run(&mut e, r#"{"cmd":"unprotectDocument","password":"pw"}"#);
    let v = run(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
    assert!(v.get("error").is_none(), "{v}");
    assert_ne!(text(&e), before);
}

#[test]
fn comments_mode_lets_only_comments_in() {
    let mut e = doc();
    run(&mut e, r#"{"cmd":"protectDocument","edit":"comments"}"#);
    let before = text(&e);
    let v = run(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
    assert!(v.get("error").is_some(), "{v}");
    assert_eq!(text(&e), before);
    run(&mut e, r#"{"cmd":"selectAll"}"#);
    let v = run(&mut e, r#"{"cmd":"addComment","text":"note"}"#);
    assert!(v.get("error").is_none(), "{v}");
    assert_eq!(e.document().unwrap().comments.len(), 1);
}

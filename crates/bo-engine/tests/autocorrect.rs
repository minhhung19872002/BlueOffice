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
    assert!(r["autocorrect"]["entries"].as_array().unwrap().len() > 20);
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
    }});
    let r = cmd(&mut e, &opts.to_string());
    assert_eq!(r["autocorrect"]["enabled"], json!(true));
    assert_eq!(r["autocorrect"]["capitalizeSentences"], json!(false));
    assert_eq!(r["autocorrect"]["entries"].as_array().unwrap().len(), 1);
    type_text(&mut e, "theo bxd teh ");
    assert_eq!(para0(&mut e), "theo Bộ Xây dựng teh");
}

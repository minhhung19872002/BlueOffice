//! CORE-MISC-003 (3): `openSetup` runs the BlueDoc document-open steps (`setAuthor`,
//! `setIdentity`, `autoCorrect`, `state`, `fontList`) in one call. Its answer must be what the
//! separate commands give, and the separate commands keep working.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::{json, Value};
use std::time::Instant;

fn cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

/// A saved document marked as final with an editable region, opened in a new engine.
fn bytes() -> Vec<u8> {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        &json!({"cmd": "insertText", "text": "Đầu giữa cuối"}),
    );
    cmd(&mut e, &json!({"cmd": "markFinal", "on": true}));
    e.save_docx().unwrap()
}

fn open(b: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "a.docx").unwrap();
    e
}

const NAMES: [&str; 2] = ["Nguyễn An", "an@bluestar.vn"];

fn options() -> Value {
    // The engine defaults with one change, as a user's kept options would be.
    let mut e = open(&bytes());
    let mut o = cmd(&mut e, &json!({"cmd": "autoCorrect", "reset": true}))["autocorrect"].clone();
    o["capitalizeSentences"] = json!(false);
    o
}

/// The old sequence of five commands; the answer the combined command must equal.
fn sequential(e: &mut Engine, ac: &Value) -> Value {
    let a = cmd(e, &json!({"cmd": "setAuthor", "name": "Trần Bình"}));
    let s = cmd(e, &json!({"cmd": "setIdentity", "names": NAMES}));
    let c = cmd(e, &json!({"cmd": "autoCorrect", "options": ac}));
    let mut st = cmd(e, &json!({"cmd": "state"}));
    let f = cmd(e, &json!({"cmd": "fontList"}));
    for r in [&a, &s, &c, &f] {
        assert!(r.get("error").is_none(), "{r}");
        assert_eq!(r["dirty"], json!([]));
    }
    st["security"] = s["security"].clone();
    st["autocorrect"] = c["autocorrect"].clone();
    st["fonts"] = f["fonts"].clone();
    st
}

fn combined(e: &mut Engine, ac: &Value) -> Value {
    cmd(
        e,
        &json!({"cmd": "openSetup", "author": "Trần Bình", "names": NAMES,
                "autoCorrect": ac, "fonts": true, "now": 1_700_000_000_000_i64}),
    )
}

#[test]
fn open_setup_answers_like_the_separate_commands() {
    let b = bytes();
    let ac = options();
    let expected = sequential(&mut open(&b), &ac);
    let got = combined(&mut open(&b), &ac);
    assert_eq!(got, expected);
    // What the host reads from it.
    assert_eq!(got["security"]["markedFinal"], true);
    assert_eq!(got["autocorrect"]["capitalizeSentences"], false);
    assert_ne!(got["fonts"], json!([]));
    assert!(got["fonts"].is_array());

    // Without fonts or AutoCorrect options: defaults, no font list.
    let mut e = open(&b);
    let r = cmd(
        &mut e,
        &json!({"cmd": "openSetup", "resetAutoCorrect": true}),
    );
    assert!(r.get("fonts").is_none(), "{r}");
    let defaults = cmd(&mut open(&b), &json!({"cmd": "autoCorrect", "reset": true}));
    assert_eq!(r["autocorrect"], defaults["autocorrect"]);
}

#[test]
fn open_setup_sets_the_author_and_identity() {
    // An editable region for one user in a restricted document.
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, &json!({"cmd": "insertText", "text": "Đầu giữa"}));
    cmd(&mut e, &json!({"cmd": "selectAll"}));
    cmd(
        &mut e,
        &json!({"cmd": "addEditableRegion", "editor": "an@bluestar.vn"}),
    );
    cmd(&mut e, &json!({"cmd": "restrictEditing"}));
    let b = e.save_docx().unwrap();

    let mut f = open(&b);
    let r = cmd(&mut f, &json!({"cmd": "openSetup", "names": NAMES}));
    assert!(r.get("error").is_none(), "{r}");
    // The region is An's: typing in it is allowed.
    cmd(&mut f, &json!({"cmd": "selectAll"}));
    let r = cmd(&mut f, &json!({"cmd": "insertText", "text": "x"}));
    assert!(r.get("error").is_none(), "{r}");
    // Without the identity the same edit is refused.
    let mut g = open(&b);
    cmd(&mut g, &json!({"cmd": "openSetup", "names": []}));
    cmd(&mut g, &json!({"cmd": "selectAll"}));
    let r = cmd(&mut g, &json!({"cmd": "insertText", "text": "x"}));
    assert_eq!(r["error"], "document protected");

    // New revisions carry the author.
    let mut h = Engine::with_bundled_fonts();
    h.new_document("a.docx").unwrap();
    cmd(&mut h, &json!({"cmd": "openSetup", "author": "Trần Bình"}));
    cmd(&mut h, &json!({"cmd": "setTrackChanges", "on": true}));
    cmd(&mut h, &json!({"cmd": "insertText", "text": "mới"}));
    let xml = String::from_utf8(
        bo_opc::Package::read(&h.save_docx().unwrap(), &bo_opc::Limits::default())
            .unwrap()
            .part("/word/document.xml")
            .unwrap()
            .data
            .clone(),
    )
    .unwrap();
    assert!(xml.contains(r#"w:author="Trần Bình""#), "{xml}");
}

/// Time of the set-up both ways (engine only, no worker round trips): printed for the
/// record, `cargo test -p bo-engine --release --test open_setup -- --nocapture`.
#[test]
fn open_setup_timing() {
    let b = bytes();
    let ac = options();
    let runs = 20;
    let mut seq = 0f64;
    let mut one = 0f64;
    for _ in 0..runs {
        let mut e = open(&b);
        let t = Instant::now();
        sequential(&mut e, &ac);
        seq += t.elapsed().as_secs_f64();
        let mut e = open(&b);
        let t = Instant::now();
        combined(&mut e, &ac);
        one += t.elapsed().as_secs_f64();
    }
    let runs_f = f64::from(runs);
    println!(
        "open set-up: 5 commands {:.3} ms, openSetup {:.3} ms (mean of {runs})",
        seq * 1000.0 / runs_f,
        one * 1000.0 / runs_f
    );
}

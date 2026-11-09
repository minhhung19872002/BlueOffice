//! Advanced find / replace through the JSON command bridge (CORE-FIND-001): special
//! characters, wildcards and highlight-all rectangles.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::json;

fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

#[test]
fn highlight_all_returns_one_rect_per_line_of_each_match() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Hà Nội và hà nội\nThủ đô hà nội".as_bytes(), "a.txt")
        .unwrap();
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"hà nội","highlight":true}"#,
    );
    let m = r["matches"].as_array().unwrap();
    assert_eq!(m.len(), 3);
    // "Hà Nội và " = H(1) à(2) space(1) N(1) ộ(3) i(1) space(1) v(1) à(2) space(1) = 14.
    assert_eq!(m[1], json!({"p":0,"start":14,"end":23}));
    let h = r["highlights"].as_array().unwrap();
    assert_eq!(h.len(), 3);
    // Rectangles of the first line come left to right; the third is on line two.
    let x = |i: usize| h[i]["x"].as_f64().unwrap();
    let y = |i: usize| h[i]["y"].as_f64().unwrap();
    assert!(x(0) < x(1));
    assert_eq!(y(0), y(1));
    assert!(y(2) > y(0));
    assert!(h.iter().all(|r| r["w"].as_f64().unwrap() > 0.0));
    // Without the option there are no rectangles; the selection is unchanged.
    let r = cmd(&mut e, r#"{"cmd":"find","query":"hà nội"}"#);
    assert!(r["highlights"].is_null());
    assert_eq!(r["rects"].as_array().unwrap().len(), 0);
}

#[test]
fn paragraph_mark_match_select_replace_and_undo() {
    let mut e = Engine::with_bundled_fonts();
    e.open("một\nhai\nba".as_bytes(), "a.txt").unwrap();
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"một^phai","highlight":true}"#,
    );
    let m = r["matches"][0].clone();
    // Ends at byte 3 of paragraph 1 ("hai").
    assert_eq!(m, json!({"p":0,"start":0,"end":3,"ep":1}));
    // Two lines are highlighted.
    assert_eq!(r["highlights"].as_array().unwrap().len(), 2);
    let r = cmd(&mut e, &json!({"cmd":"selectMatch","match":m}).to_string());
    assert_eq!(r["selection"]["focus"], json!({"p":1,"o":3}));
    let r = cmd(
        &mut e,
        &json!({"cmd":"replace","match":m,"with":"^&!"}).to_string(),
    );
    assert!(r["error"].is_null());
    assert_eq!(e.document().unwrap().plain_text(), "một\nhai!\nba");
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "một\nhai\nba");
    let r = cmd(&mut e, r#"{"cmd":"replaceAll","query":"^p","with":", "}"#);
    assert_eq!(r["count"], 2);
    assert_eq!(e.document().unwrap().plain_text(), "một, hai, ba");
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "một\nhai\nba");
}

#[test]
fn wildcard_replace_with_groups_and_single_replace() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Nguyễn Văn An\nTrần Thị Bình".as_bytes(), "a.txt")
        .unwrap();
    // Swap the first two words of each line.
    let r = cmd(
        &mut e,
        r#"{"cmd":"replaceAll","query":"(<*>) (<*>)","with":"\\2 \\1","options":{"wildcards":true}}"#,
    );
    assert_eq!(r["count"], 2);
    assert_eq!(
        e.document().unwrap().plain_text(),
        "Văn Nguyễn An\nThị Trần Bình"
    );
    // A single replace with the query expands groups against the match.
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"(B)(ình)","options":{"wildcards":true}}"#,
    );
    let m = r["matches"][0].clone();
    let r = cmd(
        &mut e,
        &json!({"cmd":"replace","match":m,"with":"\\2\\1","query":"(B)(ình)","options":{"wildcards":true}})
            .to_string(),
    );
    assert!(r["error"].is_null());
    assert_eq!(
        e.document().unwrap().plain_text(),
        "Văn Nguyễn An\nThị Trần ìnhB"
    );
    // A bad wildcard pattern is reported, nothing changes.
    let r = cmd(
        &mut e,
        r#"{"cmd":"find","query":"[z-a]","options":{"wildcards":true}}"#,
    );
    assert!(r["error"].is_string());
}

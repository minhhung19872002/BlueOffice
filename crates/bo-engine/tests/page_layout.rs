//! Layout tab over the JSON bridge: columns with a separator, line numbers,
//! hyphenation, vertical sections and drop caps — laid out, one undo step each,
//! saved and reopened (ECMA-376 Part 1, §17.6.4, §17.6.8, §17.15.1.10, §17.6.20,
//! §17.3.1.11).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_layout::{LineSource, PageItem};
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

fn engine(n: usize) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    for i in 0..n {
        cmd(
            &mut e,
            &format!(r#"{{"cmd":"insertText","text":"Đoạn {i} của văn bản bố cục trang."}}"#),
        );
        if i + 1 < n {
            cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
        }
    }
    e
}

fn vertical_rules(e: &Engine) -> usize {
    e.layout().unwrap().pages[0]
        .items
        .iter()
        .filter(|i| matches!(i, PageItem::Rule { x1, x2, y1, y2, .. } if (x1 - x2).abs() < 0.01 && (y2 - y1) > 50.0))
        .count()
}

#[test]
fn columns_with_separator_are_laid_out_and_saved() {
    let mut e = engine(80);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setColumns","columns":{"count":3,"space":360,"separator":true},"scope":"section"}"#,
    );
    assert_eq!(r["pageLayout"]["columns"]["count"], 3);
    assert_eq!(r["pageLayout"]["columns"]["separator"], true);
    assert_eq!(vertical_rules(&e), 2);
    let f = reopen(&e);
    let c = &f.document().unwrap().sections[0].columns;
    assert_eq!(c.len(), 3);
    assert!(c.separator);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(vertical_rules(&e), 0);
}

#[test]
fn line_numbers_and_paragraph_suppression() {
    let mut e = engine(6);
    cmd(
        &mut e,
        r#"{"cmd":"setLineNumbering","numbering":{"countBy":1,"start":0,"distance":null,"restart":"newPage"}}"#,
    );
    let numbers = |e: &Engine| -> Vec<String> {
        e.layout().unwrap().pages[0]
            .lines
            .iter()
            .filter(|l| l.source == LineSource::Other)
            .map(|l| l.fragments.iter().map(|f| f.text.as_str()).collect())
            .collect()
    };
    assert_eq!(numbers(&e), ["1", "2", "3", "4", "5", "6"]);
    // The caret is in the last paragraph: leave it out.
    cmd(
        &mut e,
        r#"{"cmd":"formatParagraph","format":{"kind":"suppressLineNumbers","on":true}}"#,
    );
    assert_eq!(numbers(&e).len(), 5);
    let r = cmd(&mut e, r#"{"cmd":"pageLayout"}"#);
    assert_eq!(r["pageLayout"]["suppressLineNumbers"], true);
    let f = reopen(&e);
    assert!(f.document().unwrap().sections[0].line_numbers.is_some());
    cmd(&mut e, r#"{"cmd":"setLineNumbering","numbering":null}"#);
    assert_eq!(numbers(&e), Vec::<String>::new());
}

#[test]
fn hyphenation_setting_round_trips() {
    let mut e = engine(1);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setHyphenation","hyphenation":{"auto":true,"zone":360,"consecutiveLimit":2,"noCaps":true}}"#,
    );
    assert_eq!(r["pageLayout"]["hyphenation"]["auto"], true);
    let f = reopen(&e);
    let h = f.document().unwrap().hyphenation;
    assert!(h.auto && h.no_caps);
    assert_eq!(h.consecutive_limit, Some(2));
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!e.document().unwrap().hyphenation.auto);
}

#[test]
fn vertical_section_turns_the_lines() {
    let mut e = engine(2);
    cmd(&mut e, r#"{"cmd":"setSectionTextDirection","dir":"tbRl"}"#);
    let l = e.layout().unwrap();
    assert!(l.pages[0].lines.iter().all(|x| x.rotation.is_some()));
    let f = reopen(&e);
    assert_eq!(
        f.document().unwrap().sections[0].text_direction,
        Some(bo_model::TextDirection::TbRl)
    );
    cmd(&mut e, r#"{"cmd":"setSectionTextDirection","dir":null}"#);
    assert!(e.layout().unwrap().pages[0]
        .lines
        .iter()
        .all(|x| x.rotation.is_none()));
}

#[test]
fn drop_cap_is_made_changed_and_removed() {
    let mut e = engine(2);
    // Caret into the first paragraph.
    cmd(
        &mut e,
        r#"{"cmd":"move","motion":"docStart","extend":false}"#,
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"setDropCap","dropCap":{"position":"drop","lines":3,"distance":0}}"#,
    );
    assert_eq!(r["pageLayout"]["dropCap"]["lines"], 3);
    let d = e.document().unwrap();
    assert_eq!(d.sections[0].blocks.len(), 3);
    assert_eq!(d.paragraphs().next().unwrap().plain_text(), "Đ");
    let f = reopen(&e);
    let p0 = f
        .document()
        .unwrap()
        .paragraphs()
        .next()
        .unwrap()
        .props
        .frame;
    assert_eq!(p0.map(|f| f.lines), Some(3));
    cmd(&mut e, r#"{"cmd":"setDropCap","dropCap":null}"#);
    assert_eq!(
        e.document()
            .unwrap()
            .paragraphs()
            .next()
            .unwrap()
            .plain_text(),
        "Đoạn 0 của văn bản bố cục trang."
    );
}

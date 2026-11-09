//! Multilevel lists through the JSON bridge (DOC-LIST-001): define a new list, link its
//! levels to heading styles, set the numbering value; labels on the page, undo and the
//! saved .docx (ECMA-376 Part 1, §17.9).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

/// Type paragraphs, each with a paragraph style (`None` = Normal).
fn write(e: &mut Engine, paras: &[(&str, &str)]) {
    for (i, (text, style)) in paras.iter().enumerate() {
        if i > 0 {
            cmd(e, r#"{"cmd":"splitParagraph"}"#);
        }
        cmd(e, &format!(r#"{{"cmd":"applyStyle","id":"{style}"}}"#));
        cmd(
            e,
            &serde_json::json!({ "cmd": "insertText", "text": text }).to_string(),
        );
    }
}

/// Page lines: the list label fragment, then the text (the label's tab is a gap, not text).
fn lines(e: &Engine) -> Vec<String> {
    let l = e.layout().unwrap();
    (0..l.pages.len())
        .flat_map(|p| {
            l.page_text(p)
                .lines()
                .map(|s| s.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    // BO_LISTS_OUT=<dir>: keep the saved files (to check with `bo-cli validate --openxml require`).
    if let Ok(dir) = std::env::var("BO_LISTS_OUT") {
        let n = std::fs::read_dir(&dir).map_or(0, Iterator::count);
        std::fs::write(
            std::path::Path::new(&dir).join(format!("lists-{n}.docx")),
            &bytes,
        )
        .unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    f
}

const HEADING_LIST: &str = r#"{"cmd":"defineList","apply":false,"levels":[
    {"format":"decimal","text":"%1.","start":1,"indent":432,"hanging":432,"style":"Heading1"},
    {"format":"decimal","text":"%1.%2.","start":1,"indent":720,"hanging":576,"style":"Heading2"},
    {"format":"lowerLetter","text":"%3)","start":1,"indent":1080,"hanging":360,"style":"Heading3"}
]}"#;

#[test]
fn heading_linked_list_numbers_headings() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    write(
        &mut e,
        &[
            ("Intro", "Heading1"),
            ("Scope", "Heading2"),
            ("Terms", "Heading2"),
            ("Body text", "Normal"),
            ("Design", "Heading1"),
            ("Model", "Heading2"),
            ("Detail", "Heading3"),
        ],
    );
    cmd(&mut e, HEADING_LIST);
    // Level 1 counts headings 1, level 2 restarts under each level 1 (§17.9.10
    // default lvlRestart), level 3 uses its own letter format.
    let want = [
        "1.Intro",
        "1.1.Scope",
        "1.2.Terms",
        "Body text",
        "2.Design",
        "2.1.Model",
        "a)Detail",
    ];
    assert_eq!(lines(&e), want);

    // Saved and read back: same labels, links kept (§17.9.23 pStyle + style numPr).
    let f = reopen(&e);
    assert_eq!(lines(&f), want);
    let d = f.document().unwrap();
    let abs = d.numbering.abstracts.last().unwrap();
    assert_eq!(abs.levels.len(), 9);
    assert_eq!(abs.levels[1].pstyle.as_deref(), Some("Heading2"));
    assert!(d.style("Heading2").unwrap().ppr.num.is_some());

    // One undo removes the numbering again.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(lines(&e)[0], "Intro");
}

#[test]
fn set_numbering_value_restarts_and_continues() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    write(
        &mut e,
        &[
            ("one", "Normal"),
            ("two", "Normal"),
            ("three", "Normal"),
            ("four", "Normal"),
        ],
    );
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"defineList","levels":[{"format":"upperRoman","text":"%1.","start":1,"indent":720,"hanging":360}]}"#,
    );
    assert_eq!(lines(&e), ["I.one", "II.two", "III.three", "IV.four"]);
    // Caret in "three": set numbering value 10 (a new num with startOverride).
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":2,"o":0},"focus":{"p":2,"o":0}}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"setListNumbering","numbering":{"kind":"restart","value":10}}"#,
    );
    let want = ["I.one", "II.two", "X.three", "XI.four"];
    assert_eq!(lines(&e), want);
    let f = reopen(&e);
    assert_eq!(lines(&f), want);
    assert!(f
        .document()
        .unwrap()
        .numbering
        .nums
        .iter()
        .any(|n| n.start_overrides == [(0, 10)]));
    // Continue numbering joins the list above again.
    cmd(
        &mut e,
        r#"{"cmd":"setListNumbering","numbering":{"kind":"continue"}}"#,
    );
    assert_eq!(lines(&e), ["I.one", "II.two", "III.three", "IV.four"]);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(lines(&e), want);
    // Outside a list the command reports an error and changes nothing.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    let r: Value = serde_json::from_str(
        &e.command_json(r#"{"cmd":"setListNumbering","numbering":{"kind":"restart","value":2}}"#)
            .unwrap(),
    )
    .unwrap();
    assert!(r.get("error").is_some(), "{r}");
}

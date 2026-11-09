//! References tab through the JSON command bridge with the real layout: index (XE / INDEX),
//! table of figures (TOC `\c`), table of authorities (TA / TOA) and footnote ↔ endnote
//! conversion (ECMA-376 Part 1, §17.16.5.28, §17.16.5.62, §17.16.5.67, §17.16.5.68,
//! §17.16.5.72, §17.11). Page breaks put each marked paragraph on a known page; the
//! expected entries are worked out by hand.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;

fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    let r: serde_json::Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r["error"].is_null(), "{json}: {r}");
    r
}

fn sel(e: &mut Engine, p: usize, a: usize, b: usize) {
    cmd(
        e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{a}}},"focus":{{"p":{p},"o":{b}}}}}}}"#
        ),
    );
}

/// Id of the first body paragraph whose text starts with `text`.
fn para_of(e: &Engine, text: &str) -> usize {
    let s = e.session().unwrap();
    (0..s.paragraph_count())
        .find(|i| s.para_text(*i).is_some_and(|t| t.starts_with(text)))
        .unwrap()
}

/// A page break before the paragraph starting with `text`.
fn new_page_before(e: &mut Engine, text: &str) {
    let p = para_of(e, text);
    sel(e, p, 0, 0);
    cmd(e, r#"{"cmd":"insertPageBreak"}"#);
}

fn lines(e: &Engine) -> Vec<String> {
    e.document()
        .unwrap()
        .plain_text()
        .lines()
        .map(|l| l.trim_matches('\u{c}').to_owned())
        .collect()
}

/// Entries after the line `first` (inclusive), `n` of them.
fn block(e: &Engine, first: &str, n: usize) -> Vec<String> {
    let l = lines(e);
    let i = l
        .iter()
        .position(|x| x == first)
        .unwrap_or_else(|| panic!("{l:?}"));
    l[i..i + n].to_vec()
}

#[test]
fn index_marked_on_pages_sorted_in_vietnamese_and_saved() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Đà Nẵng\nHà Nội\nDừa".as_bytes(), "a.txt").unwrap();
    for (text, mark) in [
        ("Đà Nẵng", r#"{"main":"Đà Nẵng"}"#),
        ("Hà Nội", r#"{"main":"Hà Nội","bold":true}"#),
        ("Dừa", r#"{"main":"Dừa"}"#),
    ] {
        let p = para_of(&e, text);
        sel(&mut e, p, 0, text.len());
        let r = cmd(
            &mut e,
            &format!(r#"{{"cmd":"markIndexEntry","mark":{mark}}}"#),
        );
        assert_eq!(r["count"], 1);
    }
    // Hà Nội on page 2, Dừa on page 3.
    new_page_before(&mut e, "Hà Nội");
    new_page_before(&mut e, "Dừa");
    let last = para_of(&e, "Dừa");
    sel(&mut e, last, "Dừa".len() + 3, "Dừa".len() + 3);
    cmd(
        &mut e,
        "{\"cmd\":\"insertReferenceTable\",\"instr\":\"INDEX \\\\e \\\"\\t\\\" \\\\h \\\"A\\\" \\\\z \\\"1066\\\"\",\"leader\":\"dot\",\"columns\":2}",
    );
    // The index has its own two-column continuous section (§17.6.4 cols).
    let doc = e.document().unwrap();
    assert_eq!(doc.sections.len(), 3);
    assert_eq!(doc.sections[1].columns.count, 2);
    let want = ["D", "Dừa\t3", "Đ", "Đà Nẵng\t1", "H", "Hà Nội\t2"];
    assert_eq!(block(&e, "D", 6), want);
    // Undo removes the index in one step; F9 rebuilds nothing new.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!lines(&e).contains(&"Đà Nẵng\t1".to_owned()));
    cmd(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(block(&e, "D", 6), want);

    // Saved and reopened: the XE and INDEX fields come back and update the same way.
    let bytes = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_REFS_OUT") {
        std::fs::write(format!("{dir}/index.docx"), &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "index.docx").unwrap();
    assert_eq!(block(&f, "D", 6), want);
    cmd(&mut f, r#"{"cmd":"updateFields"}"#);
    assert_eq!(block(&f, "D", 6), want);
}

#[test]
fn table_of_figures_and_authorities_on_pages() {
    let mut e = Engine::with_bundled_fonts();
    e.open(
        "Mở đầu\nTheo Bộ luật Dân sự 2015 thì\nMột\nHai\nBLDS cũng vậy".as_bytes(),
        "a.txt",
    )
    .unwrap();
    // Captions after "Một" (page 2) and "Hai" (page 3).
    let p = para_of(&e, "Một");
    sel(&mut e, p, 0, 0);
    cmd(
        &mut e,
        r#"{"cmd":"insertCaption","label":"Hình","text":"Sơ đồ"}"#,
    );
    let p = para_of(&e, "Hai");
    sel(&mut e, p, 0, 0);
    cmd(
        &mut e,
        r#"{"cmd":"insertCaption","label":"Hình","text":"Quy trình"}"#,
    );
    // A citation marked everywhere: page 1 (long), page 3 (short).
    let p = para_of(&e, "Theo");
    let start = "Theo ".len();
    sel(&mut e, p, start, start + "Bộ luật Dân sự 2015".len());
    let r = cmd(
        &mut e,
        r#"{"cmd":"markCitation","mark":{"long":"Bộ luật Dân sự 2015","short":"BLDS","category":2},"all":true}"#,
    );
    assert_eq!(r["count"], 2);
    new_page_before(&mut e, "Một");
    new_page_before(&mut e, "Hai");

    sel(&mut e, 0, 0, 0);
    cmd(
        &mut e,
        r#"{"cmd":"insertReferenceTable","instr":"TOC \\h \\z \\c \"Hình\""}"#,
    );
    assert_eq!(
        block(&e, "Hình 1: Sơ đồ\t2", 2),
        ["Hình 1: Sơ đồ\t2", "Hình 2: Quy trình\t3"]
    );
    let end = para_of(&e, "BLDS");
    sel(&mut e, end, 0, 0);
    cmd(
        &mut e,
        r#"{"cmd":"insertReferenceTable","instr":"TOA \\h \\c \"0\""}"#,
    );
    // The figures table stays on page 1: the pages do not move.
    assert_eq!(
        block(&e, "Luật và bộ luật", 2),
        ["Luật và bộ luật", "Bộ luật Dân sự 2015, 1, 3"]
    );
    let bytes = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_REFS_OUT") {
        std::fs::write(format!("{dir}/figures-toa.docx"), &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "figures-toa.docx").unwrap();
    cmd(&mut f, r#"{"cmd":"updateFields"}"#);
    assert_eq!(
        block(&f, "Luật và bộ luật", 2),
        ["Luật và bộ luật", "Bộ luật Dân sự 2015, 1, 3"]
    );
    assert_eq!(
        block(&f, "Hình 1: Sơ đồ\t2", 2),
        ["Hình 1: Sơ đồ\t2", "Hình 2: Quy trình\t3"]
    );
}

#[test]
fn footnotes_converted_to_endnotes_and_saved() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Một\nHai".as_bytes(), "a.txt").unwrap();
    sel(&mut e, 0, "Một".len(), "Một".len());
    cmd(
        &mut e,
        r#"{"cmd":"insertNote","endnote":false,"text":"Ghi chú một"}"#,
    );
    sel(&mut e, 1, "Hai".len(), "Hai".len());
    cmd(
        &mut e,
        r#"{"cmd":"insertNote","endnote":true,"text":"Ghi chú hai"}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"convertNotes","mode":"toEndnotes"}"#);
    assert_eq!(r["count"], 1);
    let notes: Vec<(bool, String)> = e
        .session()
        .unwrap()
        .notes()
        .into_iter()
        .map(|n| (n.endnote, n.text))
        .collect();
    assert_eq!(
        notes,
        [
            (true, "Ghi chú một".to_owned()),
            (true, "Ghi chú hai".to_owned())
        ]
    );
    let bytes = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_REFS_OUT") {
        std::fs::write(format!("{dir}/notes.docx"), &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "notes.docx").unwrap();
    let doc = f.document().unwrap();
    assert!(doc.footnotes.keys().all(|k| *k <= 0));
    assert_eq!(doc.endnotes.keys().filter(|k| **k > 0).count(), 2);
    // Swap them back: both footnotes, the first one numbered 1.
    let r = cmd(&mut f, r#"{"cmd":"convertNotes","mode":"swap"}"#);
    assert_eq!(r["count"], 2);
    assert!(f.session().unwrap().notes().iter().all(|n| !n.endnote));
}

//! Template filling (CORE-API-001, `bo_engine::fill`): content controls by tag / title,
//! bookmarks, placeholders, document properties and workbook cells, on corpus files.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::fill::{parse_edits, FillOutcome};
use bo_engine::{Engine, SheetEngine};

fn corpus(rel: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    std::fs::read(path).unwrap()
}

fn open(rel: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus(rel), "template.docx").unwrap();
    e
}

fn reopen(e: &Engine) -> Engine {
    let mut f = Engine::with_bundled_fonts();
    f.open(&e.save_docx().unwrap(), "out.docx").unwrap();
    f
}

fn counts(r: &[FillOutcome]) -> Vec<usize> {
    r.iter().map(|o| o.count).collect()
}

#[test]
fn content_controls_are_filled_by_tag_and_title() {
    let mut e = open("doc/controls-01-forms.docx");
    let edits = parse_edits(
        r#"[
            {"op":"setControl","tag":"ho_ten","value":"Nguyễn Văn An"},
            {"op":"setControl","title":"Phòng ban","value":"Kế toán"},
            {"op":"setControl","title":"Phòng ban","value":"Không có"},
            {"op":"setControl","tag":"khong_co","value":"x"}
        ]"#,
    )
    .unwrap();
    let report = e.fill(&edits).unwrap();
    // A list control only takes one of its items; an unknown tag matches nothing.
    assert_eq!(counts(&report), [1, 1, 0, 0]);
    assert_eq!(report[0].op, "setControl");
    assert_eq!(report[0].target, "ho_ten");

    let f = reopen(&e);
    let text = f.document().unwrap().plain_text();
    assert!(text.contains("Nguyễn Văn An"), "{text}");
    assert!(!text.contains("Nhập họ tên"), "{text}");
    assert!(text.contains("Kế toán"), "{text}");
    assert!(!text.contains("Kỹ thuật"), "{text}");
}

#[test]
fn bookmark_text_is_replaced_inside_its_marks() {
    let mut e = open("doc/links-02-bookmarks.docx");
    let bookmark = |e: &Engine| {
        bo_edit::fields::bookmarks(e.document().unwrap())
            .into_iter()
            .find(|b| b.name == "_Toc1")
            .unwrap()
            .text
    };
    assert_eq!(bookmark(&e), "Mục tiêu");
    let fill = |e: &mut Engine, text: &str| {
        let json = serde_json::json!([{"op": "setBookmark", "name": "_Toc1", "text": text}]);
        counts(&e.fill(&parse_edits(&json.to_string()).unwrap()).unwrap())
    };
    assert_eq!(fill(&mut e, "Mục đích của đề án"), [1]);
    assert_eq!(bookmark(&e), "Mục đích của đề án");
    // The bookmark still encloses its text: it can be filled again, also after a save.
    let mut f = reopen(&e);
    assert_eq!(bookmark(&f), "Mục đích của đề án");
    assert_eq!(fill(&mut f, "X"), [1]);
    assert_eq!(bookmark(&f), "X");
    assert_eq!(fill(&mut f, ""), [1]);
    assert_eq!(bookmark(&f), "");

    let missing = parse_edits(r#"[{"op":"setBookmark","name":"KhongCo","text":"a"}]"#).unwrap();
    assert_eq!(counts(&f.fill(&missing).unwrap()), [0]);
}

#[test]
fn placeholders_are_literal_and_properties_are_saved() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("Mẫu.docx").unwrap();
    e.command_json(r#"{"cmd":"insertText","text":"Kính gửi {{ten}}, mã ^p: {{ten}}"}"#)
        .unwrap();
    let edits = parse_edits(
        r#"[
            {"op":"replaceText","find":"{{ten}}","with":"ông Bình ^t&"},
            {"op":"replaceText","find":"^p","with":"số"},
            {"op":"replaceText","find":"{{khong}}","with":"x"},
            {"op":"setProperties","props":{"title":"Thư mời","author":"Văn phòng"}}
        ]"#,
    )
    .unwrap();
    assert_eq!(counts(&e.fill(&edits).unwrap()), [2, 1, 0, 1]);
    let f = reopen(&e);
    assert_eq!(
        f.document().unwrap().plain_text(),
        "Kính gửi ông Bình ^t&, mã số: ông Bình ^t&"
    );
    assert_eq!(f.doc_properties().title, "Thư mời");
    assert_eq!(f.doc_properties().author, "Văn phòng");
}

#[test]
fn workbook_cells_are_set_by_reference_and_defined_name() {
    let mut e = SheetEngine::new();
    e.open(&corpus("sheet/names-1.xlsx"), "names-1.xlsx")
        .unwrap();
    let edits = parse_edits(
        r#"[
            {"op":"setCell","ref":"TyGia","value":25000},
            {"op":"setCell","ref":"A2","value":"Tổng cộng"},
            {"op":"setCell","ref":"C2","value":"=TyGia*2"},
            {"op":"setCell","ref":"KhongCoTen","value":1}
        ]"#,
    )
    .unwrap();
    assert_eq!(counts(&e.fill(&edits).unwrap()), [1, 1, 1, 0]);
    let saved = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&saved, "out.xlsx").unwrap();
    // TyGia is 'Sổ 1'!$B$1.
    assert_eq!(f.cell(0, 0, 1).unwrap().input, "25000");
    assert_eq!(f.cell(0, 1, 0).unwrap().input, "Tổng cộng");
    assert_eq!(f.cell(0, 1, 2).unwrap().input, "=TyGia*2");
    assert_eq!(f.cell(0, 1, 2).unwrap().display, "50000");
}

#[test]
fn edits_of_the_wrong_kind_and_unknown_ops_are_errors() {
    assert!(parse_edits(r#"[{"op":"deleteEverything"}]"#).is_err());
    assert!(parse_edits(r#"{"op":"setCell"}"#).is_err());
    assert!(parse_edits(r#"[{"op":"setBookmark","name":"a","text":"b","extra":1}]"#).is_err());

    let mut e = open("doc/phase1-sample.docx");
    let cell = parse_edits(r#"[{"op":"setCell","ref":"A1","value":1}]"#).unwrap();
    assert!(e.fill(&cell).is_err());
    let no_target = parse_edits(r#"[{"op":"setControl","value":"x"}]"#).unwrap();
    assert!(e.fill(&no_target).is_err());

    let mut s = SheetEngine::new();
    s.open(&corpus("sheet/names-1.xlsx"), "names-1.xlsx")
        .unwrap();
    let text = parse_edits(r#"[{"op":"replaceText","find":"a","with":"b"}]"#).unwrap();
    assert!(s.fill(&text).is_err());
}

#[test]
fn one_docx_per_merged_record() {
    let mut e = open("doc/merge-01-letter.docx");
    assert_eq!(
        e.set_merge_data(&corpus("doc/merge-data.csv"), "merge-data.csv")
            .unwrap(),
        3
    );
    let files = e.merge_docx_files("Thư «Họ tên»").unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "Thư Nguyễn Văn An.docx",
            "Thư Trần Thị Bình.docx",
            "Thư Lê Văn Cường.docx"
        ]
    );
    let mut f = Engine::with_bundled_fonts();
    f.open(&files[1].1, "b.docx").unwrap();
    let text = f.document().unwrap().plain_text();
    assert!(text.contains("Trần Thị Bình"), "{text}");
    assert!(!text.contains("Nguyễn Văn An"), "{text}");

    // Zipped, as PDF or .docx.
    assert!(e.merge_zip("«Họ tên»", "docx").unwrap().starts_with(b"PK"));
    assert!(e.merge_zip("«Họ tên»", "pdf").unwrap().starts_with(b"PK"));
    assert!(e.merge_zip("«Họ tên»", "odt").is_err());
}

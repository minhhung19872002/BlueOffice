//! BlueDoc table tools through the JSON bridge (ECMA-376 Part 1, §17.4): text → table,
//! sort, distribute rows (measured from the layout), AutoFit to the contents
//! (measured), vertical text, floating tables and table → text; saved as .docx and
//! read back. `BO_TABLE_TOOLS_OUT=<file.docx>` also writes the saved file (for the
//! OpenXmlValidator check: `bo-cli validate <file> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{Block, Document, HeightRule, Table, TableWidth, TextDirection};
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn table(d: &Document) -> &Table {
    d.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .unwrap()
}

fn cell(t: &Table, r: usize, c: usize) -> String {
    t.rows[r].cells[c]
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(bo_model::Paragraph::plain_text)
        .collect()
}

#[test]
fn table_tools_through_the_bridge() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        r#"{"cmd":"insertText","text":"Họ và tên đầy đủ\tSố\nNguyễn Văn Đức\t10\nTrần An\t9"}"#,
    );
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"fromText","separator":"\t"}}"#,
    );
    assert_eq!(r["format"]["table"]["rows"], 3);
    assert_eq!(r["format"]["table"]["cols"], 2);
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"sort","header":true,"keys":[{"column":1,"kind":"number"}]}}"#,
    );
    let t = table(e.document().unwrap());
    assert_eq!(
        (cell(t, 1, 0), cell(t, 2, 0)),
        ("Trần An".to_owned(), "Nguyễn Văn Đức".to_owned())
    );
    // Rows measured by the layout: equal minimum heights.
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"distributeRows"}}"#,
    );
    let t = table(e.document().unwrap());
    let h = t.rows[0].props.height.unwrap();
    assert_eq!(h.1, HeightRule::AtLeast);
    assert!(h.0 .0 > 100, "{h:?}");
    assert!(t.rows.iter().all(|r| r.props.height == Some(h)));
    // AutoFit to the contents: the name column is wider than the number column.
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"autoFit","mode":"contents"}}"#,
    );
    let t = table(e.document().unwrap());
    assert_eq!(t.props.width, Some(TableWidth::Auto));
    assert!(t.grid[0].0 > t.grid[1].0 * 2, "{:?}", t.grid);
    let total: i32 = t.grid.iter().map(|g| g.0).sum();
    assert!(total < 9000, "fits its content: {total}");
    // Vertical text: the display list turns the line.
    let r = cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"textDirection","dir":"btLr"}}"#,
    );
    assert_eq!(r["format"]["table"]["direction"], "btLr");
    assert!(e.page_json(0).unwrap().contains("pushTransform"));
    let r = cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"wrap","around":true}}"#,
    );
    assert_eq!(r["format"]["table"]["wrap"], true);
    let bytes = e.save_docx().unwrap();
    if let Ok(path) = std::env::var("BO_TABLE_TOOLS_OUT") {
        std::fs::write(path, &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    let t = table(f.document().unwrap());
    assert!(t.props.float.is_some());
    assert_eq!(
        t.rows[0].cells[0].props.text_direction,
        Some(TextDirection::BtLr)
    );
    assert_eq!(cell(t, 2, 1), "10");
    // Table → text, then undo back to the table.
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"toText","separator":","}}"#,
    );
    let d = e.document().unwrap();
    assert!(!d.sections[0]
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Table(_))));
    assert_eq!(
        d.sections[0].blocks[1].as_paragraph().unwrap().plain_text(),
        "Trần An,9"
    );
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(table(e.document().unwrap()).props.float.is_some());
}

#[test]
fn formula_field_in_a_cell() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"1200\n3400\n"}"#);
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"fromText","separator":"\n","cols":1}}"#,
    );
    // The caret is in the first cell; go to the last (empty) one.
    cmd(&mut e, r#"{"cmd":"tab"}"#);
    cmd(&mut e, r#"{"cmd":"tab"}"#);
    cmd(
        &mut e,
        r##"{"cmd":"insertField","instr":"=SUM(ABOVE) \\# \"#,##0\""}"##,
    );
    let t = table(e.document().unwrap());
    // 1200 + 3400 = 4600, shown with a thousands separator.
    assert_eq!(cell(t, 2, 0), "4,600");
}

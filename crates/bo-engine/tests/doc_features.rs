//! BlueDoc editing features through the JSON bridge: headers/footers, page setup,
//! notes, tables with Tab, fonts, Format Painter, Paste Special and templates, each
//! saved as .docx and read back.

use bo_engine::Engine;
use bo_model::{Block, Document, RunContent};
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

fn hf_text(blocks: Option<&Vec<Block>>) -> String {
    blocks
        .map(|b| {
            b.iter()
                .filter_map(Block::as_paragraph)
                .map(bo_model::Paragraph::plain_text)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn doc(e: &Engine) -> &Document {
    e.document().unwrap()
}

#[test]
fn header_editing_with_page_number_and_undo() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân bài"}"#);
    // Double-click in the top margin opens the header.
    let r = cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":100,"y":30,"count":2}"#,
    );
    assert_eq!(r["story"]["kind"], "header");
    let caret_y = r["caret"]["y"].as_f64().unwrap();
    assert!(caret_y < 56.7, "caret in the top margin: {caret_y}");
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":"Trang "}"#);
    assert_eq!(r["story"]["kind"], "header");
    cmd(&mut e, r#"{"cmd":"insertField","instr":"PAGE"}"#);
    // Shown live on the page.
    assert!(e.page_json(0).unwrap().contains("Trang"));
    // A click in the body closes the header: one undoable change.
    let r = cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":100}"#);
    assert!(r.get("story").is_none());
    assert_eq!(
        hf_text(doc(&e).sections[0].headers.default.as_ref()),
        "Trang 1"
    );
    let f = reopen(&e);
    let h = doc(&f).sections[0].headers.default.clone().unwrap();
    assert!(h
        .iter()
        .filter_map(Block::as_paragraph)
        .any(|p| p.runs.iter().any(|r| r
            .content
            .iter()
            .any(|c| matches!(c, RunContent::Field { instr, .. } if instr.trim() == "PAGE")))));
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(doc(&e).sections[0].headers.default.is_none());
    assert_eq!(doc(&e).plain_text(), "Thân bài");
}

#[test]
fn footer_first_page_and_odd_even_in_a_read_document() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"x"}"#);
    let mut e = reopen(&e);
    let r = cmd(
        &mut e,
        r#"{"cmd":"editHeaderFooter","page":0,"header":false}"#,
    );
    assert_eq!(r["story"]["kind"], "footer");
    assert!(r["caret"]["y"].as_f64().unwrap() > 780.0);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setHeaderOptions","titlePage":true,"evenAndOdd":true}"#,
    );
    // Still editing, now the first-page footer.
    assert_eq!(r["story"]["variant"], "first");
    assert!(r["story"]["titlePage"].as_bool().unwrap());
    cmd(&mut e, r#"{"cmd":"insertText","text":"Chân trang đầu"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let f = reopen(&e);
    let s = &doc(&f).sections[0];
    assert!(s.title_page);
    assert!(doc(&f).even_and_odd_headers);
    assert_eq!(hf_text(s.footers.first.as_ref()), "Chân trang đầu");
}

#[test]
fn page_setup_and_margins_persist() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(
        &mut e,
        r#"{"cmd":"setPageSetup","setup":{"width":16838,"height":11906,"marginTop":1134,"marginBottom":1134,"marginLeft":1701,"marginRight":850,"header":567,"footer":567,"columns":2,"columnSpace":425},"scope":"document"}"#,
    );
    assert_eq!(r["format"]["section"]["width"], 16838);
    assert_eq!(
        r["info"]["pages"][0]["width"].as_f64().unwrap().round(),
        842.0
    );
    let f = reopen(&e);
    let s = &doc(&f).sections[0];
    assert!(s.page.is_landscape());
    assert_eq!(s.columns.count, 2);
    assert_eq!(s.page.margin_left.0, 1701);
}

#[test]
fn ruler_indents_and_tabs_persist() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"a\tb"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"formatParagraph","format":{"kind":"indents","left":567,"right":0,"firstLine":-283}}"#,
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"formatParagraph","format":{"kind":"tabs","tabs":[{"pos":2835,"align":"center"}]}}"#,
    );
    assert_eq!(r["format"]["indentLeft"], 567);
    assert_eq!(r["format"]["firstLine"], -283);
    assert_eq!(r["format"]["tabs"][0]["align"], "center");
    let mut f = reopen(&e);
    let r = cmd(&mut f, r#"{"cmd":"state"}"#);
    assert_eq!(r["format"]["indentLeft"], 567);
    assert_eq!(r["format"]["tabs"][0]["pos"], 2835);
}

#[test]
fn footnotes_in_a_read_document() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Văn bản"}"#);
    let mut e = reopen(&e);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let r = cmd(&mut e, r#"{"cmd":"insertNote","text":"Ghi chú"}"#);
    let id = r["notes"][0]["id"].as_i64().unwrap();
    cmd(
        &mut e,
        &format!(r#"{{"cmd":"setNoteText","id":{id},"text":"Ghi chú đã sửa"}}"#),
    );
    let mut f = reopen(&e);
    let r = cmd(&mut f, r#"{"cmd":"notes"}"#);
    assert_eq!(r["notes"][0]["text"], "Ghi chú đã sửa");
    assert!(f.page_json(0).unwrap().contains("Ghi"));
}

#[test]
fn table_tab_adds_rows_and_commands() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertTable","rows":1,"cols":2}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"A"}"#);
    cmd(&mut e, r#"{"cmd":"tab"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"B"}"#);
    let r = cmd(&mut e, r#"{"cmd":"tab"}"#);
    assert_eq!(r["format"]["table"]["rows"], 2);
    let r = cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"insertColumn","right":true}}"#,
    );
    assert_eq!(r["format"]["table"]["cols"], 3);
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"headerRow","on":true}}"#,
    );
    let f = reopen(&e);
    let t = doc(&f).sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!((t.rows.len(), t.grid.len()), (2, 3));
    assert!(t.rows[0].props.header);
}

#[test]
fn fonts_painter_and_paste_special() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"fontList"}"#);
    let fonts = r["fonts"].as_array().unwrap();
    assert!(fonts
        .iter()
        .any(|f| f["name"] == "Tinos" && f["available"] == true));
    let tnr = fonts
        .iter()
        .find(|f| f["name"] == "Times New Roman")
        .unwrap();
    assert_eq!(tnr["substitute"], "Tinos");
    cmd(&mut e, r#"{"cmd":"insertText","text":"một hai"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":4}}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"font","name":"Carlito"}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"size","halfPoints":36}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"pickFormat"}"#);
    assert!(r["painter"].as_bool().unwrap());
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":5},"focus":{"p":0,"o":5}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"applyFormat"}"#);
    assert_eq!(r["format"]["font"], "Carlito");
    assert_eq!(r["format"]["sizeHalfPoints"], 36);
    let r = cmd(&mut e, r#"{"cmd":"cancelFormat"}"#);
    assert!(!r["painter"].as_bool().unwrap());
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"paste","html":"<b>đậm</b>","text":"đậm","mode":"text"}"#,
    );
    assert!(!r["format"]["bold"].as_bool().unwrap());
    assert_eq!(doc(&e).plain_text(), "một haiđậm");
    let r = cmd(&mut e, r#"{"cmd":"clearFormatting"}"#);
    assert!(r["format"]["canUndo"].as_bool().unwrap());
}

#[test]
fn new_from_template_nd30() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(
        &mut e,
        r#"{"cmd":"newFromTemplate","id":"nd30","name":"Công văn.docx"}"#,
    );
    assert_eq!(r["info"]["name"], "Công văn.docx");
    assert_eq!(r["format"]["section"]["marginLeft"], 1701);
    let text = e.page_json(0).unwrap();
    assert!(text.contains("CỘNG"));
    let f = reopen(&e);
    let s = &doc(&f).sections[0];
    assert!(s.title_page);
    assert_eq!(s.page.margin_right.0, 851);
    assert!(doc(&f).plain_text().contains("Độc lập - Tự do - Hạnh phúc"));
}

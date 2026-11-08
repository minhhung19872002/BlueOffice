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
    let r = cmd(&mut e, r#"{"cmd":"insertField","instr":"PAGE"}"#);
    assert!(r["caret"]["y"].as_f64().unwrap() < 56.7, "{}", r["caret"]);
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

fn note_runs(e: &Engine, id: i32) -> Vec<bo_model::Run> {
    doc(e).footnotes[&id]
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|p| p.runs.clone())
        .collect()
}

#[test]
fn footnote_edited_in_place_on_the_page() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Văn bản"}"#);
    // Inserting a footnote puts the caret into it at the page bottom.
    let r = cmd(&mut e, r#"{"cmd":"insertNote","endnote":false,"text":""}"#);
    assert_eq!(r["story"]["kind"], "footnote");
    let id = r["story"]["note"].as_i64().unwrap() as i32;
    let caret_y = r["caret"]["y"].as_f64().unwrap();
    assert!(caret_y > 600.0, "caret in the footnote area: {caret_y}");
    cmd(&mut e, r#"{"cmd":"insertText","text":"Chú thích đậm"}"#);
    // Rich text: bold the last word (selection inside the note).
    cmd(
        &mut e,
        r#"{"cmd":"move","motion":"wordLeft","extend":true}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert_ne!(r["rects"], serde_json::json!([]));
    assert!(e.page_json(0).unwrap().contains("thích"));
    // A click in the body closes the note: one undoable change.
    let r = cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":80}"#);
    assert!(r.get("story").is_none());
    let runs = note_runs(&e, id);
    assert!(runs
        .iter()
        .any(|r| r.props.bold == Some(true) && r.plain_text() == "đậm"));
    // A single click on the footnote line edits it again.
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"click","page":0,"x":200,"y":{caret_y}}}"#),
    );
    assert_eq!(r["story"]["note"], id);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"!"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let f = reopen(&e);
    let text: String = note_runs(&f, *doc(&f).footnotes.keys().max().unwrap())
        .iter()
        .map(bo_model::Run::plain_text)
        .collect();
    assert!(text.ends_with("Chú thích đậm!"), "{text}");
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!note_runs(&e, id)
        .iter()
        .any(|r| r.plain_text().contains('!')));
    // Deleting the reference mark deletes the note; undo brings it back.
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":80}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"deleteBackward"}"#);
    assert!(!doc(&e).footnotes.contains_key(&id));
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(doc(&e).footnotes.contains_key(&id));
}

#[test]
fn footnote_options_and_copy_paste() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Một"}"#);
    cmd(&mut e, r#"{"cmd":"insertNote","text":"Ghi chú"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setNoteOptions","options":{"restart":"eachPage","pos":"beneathText","fmt":"lowerLetter"}}"#,
    );
    assert_eq!(r["format"]["section"]["footnoteRestart"], "eachPage");
    let f = reopen(&e);
    let np = &doc(&f).sections[0].footnote_pr;
    assert_eq!(np.restart, Some(bo_model::NoteRestart::EachPage));
    assert_eq!(np.pos, Some(bo_model::NotePos::BeneathText));
    assert_eq!(np.fmt, Some(bo_model::NumFormat::LowerLetter));
    // Copying the reference carries the note: pasting creates a second note.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"left","extend":true}"#);
    let r = cmd(&mut e, r#"{"cmd":"copy"}"#);
    let internal = r["clip"]["internal"].as_str().unwrap().to_owned();
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let paste = serde_json::json!({"cmd": "paste", "internal": internal}).to_string();
    cmd(&mut e, &paste);
    assert_eq!(doc(&e).footnotes.len(), 2);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(doc(&e).footnotes.len(), 1);
}

#[test]
fn tracked_table_and_page_setup_saved_and_rejected() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"A"}"#);
    let original = reopen(&e);
    let mut e = reopen(&e);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":80}"#);
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"insertRow","below":true}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"shading","hex":"FFEE00"}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"properties","widthPct":80,"align":"Center"}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"setPageSetup","setup":{"width":16838,"height":11906,"marginTop":1134,"marginBottom":1134,"marginLeft":1134,"marginRight":1134,"header":567,"footer":567},"scope":"section"}"#,
    );
    // Saved as trPr/ins, tcPrChange, tblPrChange and sectPrChange, read back.
    let mut f = reopen(&e);
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    let kinds: Vec<String> = r["review"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kind"].as_str().unwrap().to_owned())
        .collect();
    for k in ["rowInsert", "cellFormat", "tableFormat", "sectionFormat"] {
        assert!(kinds.iter().any(|x| x == k), "{k} in {kinds:?}");
    }
    cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    let (a, b) = (doc(&f), doc(&original));
    assert_eq!(a.sections[0].page, b.sections[0].page);
    assert!(a.sections[0].change.is_none());
    let (Block::Table(x), Block::Table(y)) = (&a.sections[0].blocks[0], &b.sections[0].blocks[0])
    else {
        panic!("tables");
    };
    assert_eq!((&x.props, &x.rows), (&y.props, &y.rows));
}

#[test]
fn header_link_to_previous_per_section() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Một"}"#);
    cmd(&mut e, r#"{"cmd":"insertSectionBreak","kind":"nextPage"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Hai"}"#);
    // Header of section 1 (page 0).
    cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":100,"y":30,"count":2}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đầu A"}"#);
    // Section 2 is linked: same header, editing it changes section 1's (§17.10.5).
    let r = cmd(
        &mut e,
        r#"{"cmd":"click","page":1,"x":100,"y":30,"count":2}"#,
    );
    assert_eq!(r["story"]["section"], 1);
    assert_eq!(r["story"]["linked"], true);
    assert_eq!(r["story"]["canLink"], true);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" chung"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    assert_eq!(
        hf_text(doc(&e).sections[0].headers.default.as_ref()),
        "Đầu A chung"
    );
    assert!(doc(&e).sections[1].headers.default.is_none());
    // Unlink section 2 and give it its own header.
    cmd(
        &mut e,
        r#"{"cmd":"click","page":1,"x":100,"y":30,"count":2}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"setHeaderLink","linked":false}"#);
    assert_eq!(r["story"]["linked"], false);
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đầu B"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    assert_eq!(
        hf_text(doc(&e).sections[0].headers.default.as_ref()),
        "Đầu A chung"
    );
    assert_eq!(
        hf_text(doc(&e).sections[1].headers.default.as_ref()),
        "Đầu B"
    );
    assert!(e.page_json(1).unwrap().contains("Đầu B"));
    let f = reopen(&e);
    assert_eq!(
        hf_text(doc(&f).sections[1].headers.default.as_ref()),
        "Đầu B"
    );
    // Link again: section 2 shows section 1's header; undo restores its own.
    cmd(
        &mut e,
        r#"{"cmd":"click","page":1,"x":100,"y":30,"count":2}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"setHeaderLink","linked":true}"#);
    assert_eq!(r["story"]["linked"], true);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    assert!(doc(&e).sections[1].headers.default.is_none());
    assert!(e.page_json(1).unwrap().contains("chung"));
    let f = reopen(&e);
    assert!(doc(&f).sections[1].headers.default.is_none());
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        hf_text(doc(&e).sections[1].headers.default.as_ref()),
        "Đầu B"
    );
    // The first section cannot be linked.
    let r = cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":100,"y":30,"count":2}"#,
    );
    assert_eq!(r["story"]["canLink"], false);
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

fn kinds(e: &mut Engine) -> Vec<(String, Option<String>)> {
    let r = cmd(e, r#"{"cmd":"review"}"#);
    r["review"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["kind"].as_str().unwrap().to_owned(),
                x["story"]["kind"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

fn corpus(name: &str) -> Engine {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc")
        .join(name);
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), name).unwrap();
    e
}

#[test]
fn tracked_columns_merge_and_header_row_saved_and_rejected() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertTable","rows":3,"cols":2}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"A"}"#);
    let original = reopen(&e);
    let mut e = reopen(&e);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":80}"#);
    for edit in [
        r#"{"kind":"insertColumn","right":true}"#,
        r#"{"kind":"headerRow","on":true}"#,
        r#"{"kind":"splitCell"}"#,
    ] {
        cmd(&mut e, &format!(r#"{{"cmd":"tableEdit","edit":{edit}}}"#));
    }
    // An original cell of the last row, deleted (shift left).
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd","extend":false}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"up","extend":false}"#);
    cmd(
        &mut e,
        r#"{"cmd":"tableEdit","edit":{"kind":"deleteCell"}}"#,
    );
    // cellIns + tblGridChange, trPrChange, cellDel: written and read back.
    let mut f = reopen(&e);
    let k = kinds(&mut f);
    for want in ["cellInsert", "rowFormat", "cellDelete"] {
        assert!(k.iter().any(|x| x.0 == want), "{want} in {k:?}");
    }
    let Block::Table(t) = &doc(&f).sections[0].blocks[0] else {
        panic!("table")
    };
    assert!(t.grid_change.is_some());
    cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    let (Block::Table(x), Block::Table(y)) = (
        &doc(&f).sections[0].blocks[0],
        &doc(&original).sections[0].blocks[0],
    ) else {
        panic!("tables");
    };
    assert_eq!((&x.props, &x.grid, &x.rows), (&y.props, &y.grid, &y.rows));
}

#[test]
fn tracked_header_and_footnote_edits_through_the_bridge() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân bài"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertNote","endnote":false,"text":"Ghi chú"}"#,
    );
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":100}"#);
    // Header with some text, untracked.
    cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":100,"y":30,"count":2}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đầu trang"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let original = reopen(&e);
    let mut e = reopen(&e);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":100,"y":30,"count":2}"#,
    );
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd","extend":false}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" mới"}"#);
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let id = *doc(&e).footnotes.keys().next().unwrap();
    cmd(
        &mut e,
        &format!(r#"{{"cmd":"editNote","endnote":false,"id":{id}}}"#),
    );
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd","extend":false}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" thêm"}"#);
    cmd(&mut e, r#"{"cmd":"click","page":0,"x":100,"y":100}"#);
    let mut f = reopen(&e);
    let k = kinds(&mut f);
    assert!(k.iter().any(|x| x.1.as_deref() == Some("header")), "{k:?}");
    assert!(
        k.iter().any(|x| x.1.as_deref() == Some("footnote")),
        "{k:?}"
    );
    cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(kinds(&mut f).len(), 0);
    assert_eq!(
        hf_text(doc(&f).sections[0].headers.default.as_ref()),
        hf_text(doc(&original).sections[0].headers.default.as_ref())
    );
    let text = |d: &Document| {
        d.footnotes
            .values()
            .map(|b| hf_text(Some(b)))
            .collect::<Vec<_>>()
    };
    assert_eq!(text(doc(&f)), text(doc(&original)));
}

#[test]
fn tracked_picture_move_saved_and_rejected() {
    let mut e = corpus("images-04-wrap-square-sides.docx");
    let original = reopen(&e);
    cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    // The first paragraph with a floating picture.
    let n = e.document().unwrap().sections[0].blocks.len();
    let mut moved = false;
    for i in 0..n {
        let r = cmd(
            &mut e,
            &format!(
                r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{i},"o":0}},"focus":{{"p":{i},"o":0}}}}}}"#
            ),
        );
        if !r["format"]["picture"].is_null() {
            cmd(&mut e, r#"{"cmd":"setPicturePosition","x":120,"y":10}"#);
            moved = true;
            break;
        }
    }
    assert!(moved, "no floating picture");
    let mut f = reopen(&e);
    let k = kinds(&mut f);
    assert!(
        k.iter().any(|x| x.0 == "moveFrom") && k.iter().any(|x| x.0 == "moveTo"),
        "{k:?}"
    );
    cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(doc(&f).plain_text(), doc(&original).plain_text());
}

#[test]
fn corpus_structure_and_story_revisions() {
    for (name, want) in [
        (
            "review-05-columns-cells.docx",
            vec!["cellInsert", "cellDelete", "rowFormat"],
        ),
        (
            "review-06-merge-split.docx",
            vec!["cellMerge", "cellDelete", "cellInsert", "rowFormat"],
        ),
        (
            "review-07-row-properties.docx",
            vec!["rowFormat", "cellMerge", "cellInsert", "cellDelete"],
        ),
        ("review-11-picture-move.docx", vec!["moveFrom", "moveTo"]),
        (
            "review-12-picture-move-paragraph.docx",
            vec!["moveFrom", "moveTo"],
        ),
        (
            "review-13-picture-insert-delete-move.docx",
            vec!["insert", "delete", "moveFrom", "moveTo"],
        ),
    ] {
        let mut e = corpus(name);
        let k = kinds(&mut e);
        for w in want {
            assert!(k.iter().any(|x| x.0 == w), "{name}: {w} in {k:?}");
        }
        // Accept all leaves no revision, reject all neither; both lay out.
        let mut a = reopen(&e);
        cmd(&mut a, r#"{"cmd":"acceptAllRevisions"}"#);
        assert_eq!(kinds(&mut a).len(), 0, "{name}");
        cmd(&mut e, r#"{"cmd":"rejectAllRevisions"}"#);
        assert_eq!(kinds(&mut e).len(), 0, "{name}");
    }
    for (name, stories) in [
        (
            "review-08-header-footer.docx",
            vec!["header", "footer", "footnote"],
        ),
        (
            "review-09-notes.docx",
            vec!["header", "footnote", "endnote"],
        ),
        (
            "review-10-stories-mixed.docx",
            vec!["header", "footer", "footnote"],
        ),
    ] {
        let mut e = corpus(name);
        let k = kinds(&mut e);
        for s in stories {
            assert!(
                k.iter().any(|x| x.1.as_deref() == Some(s)),
                "{name}: {s} in {k:?}"
            );
        }
        cmd(&mut e, r#"{"cmd":"acceptAllRevisions"}"#);
        assert_eq!(kinds(&mut e).len(), 0, "{name}");
    }
}

#[test]
fn corpus_long_footnotes_continue() {
    for name in [
        "notes-05-long-footnote.docx",
        "notes-06-continuation-notice.docx",
        "notes-07-document-note-settings.docx",
    ] {
        let e = corpus(name);
        let l = e.layout().unwrap();
        // Some footnote has lines on two pages.
        let mut pages: std::collections::HashMap<i32, Vec<usize>> =
            std::collections::HashMap::default();
        for (pi, p) in l.pages.iter().enumerate() {
            for x in &p.lines {
                if let bo_layout::LineSource::Note {
                    endnote: false, id, ..
                } = x.source
                {
                    let v = pages.entry(id).or_default();
                    if !v.contains(&pi) {
                        v.push(pi);
                    }
                }
            }
        }
        assert!(pages.values().any(|v| v.len() > 1), "{name}: {pages:?}");
    }
    // §17.11.11 document-wide numFmt upperRoman, numStart 3.
    let e = corpus("notes-05-long-footnote.docx");
    assert!(e.layout().unwrap().pages.iter().any(|p| p
        .lines
        .iter()
        .any(|l| l.fragments.iter().any(|f| f.text == "III"))));
    let e = corpus("notes-06-continuation-notice.docx");
    let all: String = e
        .layout()
        .unwrap()
        .pages
        .iter()
        .flat_map(|p| p.lines.iter())
        .flat_map(|l| l.fragments.iter().map(|f| f.text.to_string()))
        .collect();
    assert!(all.contains("tiếp"), "continuation notice shown");
}

#[test]
fn continued_footnote_is_edited_on_its_second_page() {
    let mut e = corpus("notes-05-long-footnote.docx");
    let l = e.layout().unwrap();
    let pi = l.pages.len() - 1;
    let line = l.pages[pi]
        .lines
        .iter()
        .find(|x| matches!(x.source, bo_layout::LineSource::Note { .. }))
        .expect("continued note lines on the last page")
        .clone();
    let y = line.top + line.height / 2.0;
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"click","page":{pi},"x":200,"y":{y}}}"#),
    );
    assert_eq!(r["story"]["kind"], "footnote", "{}", r["story"]);
    assert_eq!(r["caret"]["page"], pi, "{}", r["caret"]);
}

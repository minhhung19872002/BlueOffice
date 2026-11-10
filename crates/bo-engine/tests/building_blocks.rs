//! Building blocks (DOC-BB-001): saving the selection as a block, inserting it with
//! identical content (formatting, table, numbering), the document's glossary part
//! (ECMA-376 Part 1, §17.12) across save and reopen, the user's store, AutoText (F3),
//! header / footer / cover page galleries, the shipped library and previews.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{
    AbstractNum, Block, Document, HalfPoints, Num, NumRef, Paragraph, Run, RunContent, RunProps,
    Style, StyleKind, Table, Twips,
};
use serde_json::{json, Value};

fn cmd(e: &mut Engine, v: &Value) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{v}: {:?}", r.get("error"));
    r
}

fn cmd_err(e: &mut Engine, v: &Value) -> String {
    let r: Value = serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap();
    r["error"].as_str().unwrap_or_default().to_owned()
}

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        bo_model::ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

/// "Mở đầu" · a styled, numbered, partly bold paragraph · a table · "Kết".
fn source_doc() -> Document {
    let mut d = bo_engine::plain_text_document("x");
    let mut st = Style::new("GhiChu", "Ghi chú", StyleKind::Paragraph);
    st.rpr.italic = Some(true);
    d.styles.styles.push(st);
    d.numbering.abstracts.push(AbstractNum {
        id: 2,
        levels: Vec::new(),
    });
    d.numbering.nums.push(Num {
        id: 5,
        abstract_id: 2,
        start_overrides: Vec::new(),
    });
    let mut styled = Paragraph::default();
    styled.props.style = Some("GhiChu".into());
    styled.props.num = Some(NumRef { num_id: 5, ilvl: 0 });
    styled.runs.push(Run::text("Căn cứ ", RunProps::default()));
    styled.runs.push(Run::text(
        "Luật Tổ chức",
        RunProps {
            bold: Some(true),
            size: Some(HalfPoints(28)),
            ..RunProps::default()
        },
    ));
    let mut table = Table::simple(2, 2, Twips(9000));
    table.rows[0].cells[0].blocks = vec![para("Ô một")];
    d.sections[0].blocks = vec![
        para("Mở đầu"),
        Block::Paragraph(styled),
        Block::Table(table),
        para("Kết"),
    ];
    d
}

/// Keep a file for `bo-cli validate` when `BO_BB_DUMP` names a directory.
fn dump(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_BB_DUMP") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

fn engine(doc: Document) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.load(doc, "n.docx").unwrap();
    e
}

/// Select from the start of paragraph `a` to the end of paragraph `b`.
fn select(e: &mut Engine, a: usize, b: usize, end: usize) {
    cmd(
        e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":a,"o":0},"focus":{"p":b,"o":end}}}),
    );
}

fn spec(name: &str, gallery: &str, behavior: &str) -> Value {
    json!({"name":name,"gallery":gallery,"category":"Thử","description":"Mô tả","behavior":behavior})
}

fn list(r: &Value) -> Vec<Value> {
    r["blocks"]["list"].as_array().cloned().unwrap_or_default()
}

fn key_of(r: &Value, name: &str) -> String {
    list(r)
        .iter()
        .find(|b| b["name"] == name)
        .unwrap_or_else(|| panic!("no block {name}"))["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Content without source XML, for comparing blocks.
fn plain(blocks: &[Block]) -> Vec<Block> {
    bo_edit::blocks::detached(blocks)
}

#[test]
fn save_selection_then_insert_gives_identical_content() {
    let mut e = engine(source_doc());
    // Paragraph ids: 0 "Mở đầu", 1 styled, 2..=5 table cells, 6 "Kết".
    select(&mut e, 1, 6, "Kết".len());
    let r = cmd(
        &mut e,
        &json!({"cmd":"saveBuildingBlock","spec":spec("Khối thử","docParts","p"),"store":"document"}),
    );
    let saved = e.document().unwrap().sections[0].blocks[1..].to_vec();
    let key = key_of(&r, "Khối thử");
    assert!(key.starts_with("doc:"));
    // The glossary got the style and the list the content uses.
    let g = e.document().unwrap().glossary.clone().unwrap();
    assert!(g.styles.styles.iter().any(|s| s.id == "GhiChu"));
    assert_eq!(g.numbering.nums.len(), 1);
    assert_eq!(plain(&g.parts[0].blocks).len(), 3);

    // Save, reopen, insert at the end of the first paragraph.
    let bytes = e.save_docx().unwrap();
    dump("saved-block.docx", &bytes);
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "n.docx").unwrap();
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    let key = key_of(&r, "Khối thử");
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":"Mở đầu".len()},"focus":{"p":0,"o":"Mở đầu".len()}}}),
    );
    let r = cmd(&mut e, &json!({"cmd":"insertBuildingBlock","key":key}));
    assert_eq!(r["blocks"]["inserted"], "Khối thử");
    let d = e.document().unwrap();
    let blocks = &d.sections[0].blocks;
    // "Mở đầu" · [styled · table · "Kết"] · the original three.
    assert_eq!(blocks.len(), 7);
    let mut inserted = plain(&blocks[1..4]);
    let mut original = plain(&saved);
    // Only the list instance differs: the insert makes a new one (§17.9.15 num).
    let num_of = |b: &mut Block| match b {
        Block::Paragraph(p) => p.props.num.take(),
        _ => None,
    };
    let (new_num, old_num) = (num_of(&mut inserted[0]), num_of(&mut original[0]));
    assert_eq!(inserted, original);
    let (new_num, old_num) = (new_num.unwrap(), old_num.unwrap());
    assert_ne!(new_num.num_id, old_num.num_id);
    let abstract_of = |id: i32| {
        d.numbering
            .nums
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.abstract_id)
            .unwrap()
    };
    assert!(d
        .numbering
        .abstracts
        .iter()
        .any(|a| a.id == abstract_of(new_num.num_id)));
    // One undo step.
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(e.document().unwrap().sections[0].blocks.len(), 4);
}

#[test]
fn document_blocks_survive_save_and_can_be_edited_and_deleted() {
    let mut e = engine(source_doc());
    select(&mut e, 0, 0, "Mở đầu".len());
    cmd(
        &mut e,
        &json!({"cmd":"saveBuildingBlock","spec":spec("md","autoTxt","content"),"store":"document"}),
    );
    select(&mut e, 6, 6, "Kết".len());
    let r = cmd(
        &mut e,
        &json!({"cmd":"saveBuildingBlock","spec":spec("kt","autoTxt","content"),"store":"document"}),
    );
    let doc_blocks = |r: &Value| {
        list(r)
            .into_iter()
            .filter(|b| b["store"] == "document")
            .collect::<Vec<_>>()
    };
    assert_eq!(doc_blocks(&r).len(), 2);
    assert_eq!(doc_blocks(&r)[0]["text"], "Mở đầu");
    // Saving the same name again redefines the entry.
    let r = cmd(
        &mut e,
        &json!({"cmd":"saveBuildingBlock","spec":spec("KT","autoTxt","content"),"store":"document"}),
    );
    assert_eq!(doc_blocks(&r).len(), 2);

    let first = e.save_docx().unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&first, "n.docx").unwrap();
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    assert_eq!(doc_blocks(&r).len(), 2);
    // Untouched glossary: its part is byte-identical after another save.
    let part = |bytes: &[u8]| {
        let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
        pkg.part("/word/glossary/document.xml")
            .unwrap()
            .data
            .clone()
    };
    assert_eq!(part(&e.save_docx().unwrap()), part(&first));

    let key = key_of(&r, "md");
    let r = cmd(
        &mut e,
        &json!({"cmd":"editBuildingBlock","key":key,"spec":{"name":"Mở đầu văn bản","gallery":"docParts","category":"Chung","description":"","behavior":"p"}}),
    );
    let b = doc_blocks(&r)
        .into_iter()
        .find(|b| b["name"] == "Mở đầu văn bản")
        .unwrap();
    assert_eq!(b["gallery"], "docParts");
    assert_eq!(b["behavior"], "p");
    assert_eq!(b["category"], "Chung");
    let key = key_of(&r, "KT");
    let r = cmd(&mut e, &json!({"cmd":"deleteBuildingBlock","key":key}));
    assert_eq!(doc_blocks(&r).len(), 1);
    // Undo brings the deleted block back.
    let _ = cmd(&mut e, &json!({"cmd":"undo"}));
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    assert_eq!(doc_blocks(&r).len(), 2);

    // Errors.
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"insertBuildingBlock","key":"doc:99"})),
        "noBlock"
    );
    assert_eq!(
        cmd_err(
            &mut e,
            &json!({"cmd":"deleteBuildingBlock","key":"builtin:0"})
        ),
        "builtinBlock"
    );
    select(&mut e, 0, 0, 3);
    assert_eq!(
        cmd_err(
            &mut e,
            &json!({"cmd":"saveBuildingBlock","spec":spec(" ","autoTxt","content"),"store":"document"})
        ),
        "badName"
    );
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":0}}}),
    );
    assert_eq!(
        cmd_err(
            &mut e,
            &json!({"cmd":"saveBuildingBlock","spec":spec("x","autoTxt","content"),"store":"document"})
        ),
        "emptySelection"
    );
}

#[test]
fn user_store_is_a_dotx_shared_between_documents() {
    let mut e = engine(source_doc());
    select(&mut e, 1, 1, "Căn cứ Luật Tổ chức".len());
    let r = cmd(
        &mut e,
        &json!({"cmd":"saveBuildingBlock","spec":spec("cc","autoTxt","content"),"store":"user"}),
    );
    let store = r["blocks"]["store"].as_str().unwrap().to_owned();
    assert!(key_of(&r, "cc").starts_with("user:"));
    // The open document did not change.
    assert!(e.document().unwrap().glossary.is_none());
    assert_eq!(r["modified"], false);
    // The store is a template package (§11.3.10) with a glossary part.
    let bytes = bo_engine::merge::base64_decode(&store).unwrap();
    dump("user-store.dotx", &bytes);
    assert_eq!(
        bo_engine::variant::detect(&bytes),
        Some(bo_engine::variant::Variant::Dotx)
    );
    assert_eq!(e.block_store().unwrap(), bytes);

    // Another document, another engine: load the store and press F3 after "cc".
    let mut e = engine(bo_engine::plain_text_document("Xét cc"));
    let r = cmd(&mut e, &json!({"cmd":"loadBlockStore","file":store}));
    let key = key_of(&r, "cc");
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":"Xét cc".len()},"focus":{"p":0,"o":"Xét cc".len()}}}),
    );
    let r = cmd(&mut e, &json!({"cmd":"expandAutoText"}));
    assert_eq!(r["blocks"]["inserted"], "cc");
    let d = e.document().unwrap();
    let p = d.sections[0].blocks[0].as_paragraph().unwrap();
    assert_eq!(p.plain_text(), "Xét Căn cứ Luật Tổ chức");
    // Formatting came along: the bold 14 pt run and the paragraph style's definition.
    let bold = p
        .runs
        .iter()
        .find(|r| matches!(&r.content[0], RunContent::Text(t) if t == "Luật Tổ chức"))
        .unwrap();
    assert_eq!(bold.props.bold, Some(true));
    assert_eq!(bold.props.size, Some(HalfPoints(28)));
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(
        e.document().unwrap().sections[0].blocks[0]
            .as_paragraph()
            .unwrap()
            .plain_text(),
        "Xét cc"
    );
    // No name before the caret: nothing happens.
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":3},"focus":{"p":0,"o":3}}}),
    );
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"expandAutoText"})),
        "noAutoText"
    );

    // Deleting from the store returns the changed file; an empty store has no glossary.
    let r = cmd(&mut e, &json!({"cmd":"deleteBuildingBlock","key":key}));
    let after = bo_engine::merge::base64_decode(r["blocks"]["store"].as_str().unwrap()).unwrap();
    assert!(bo_docx::read_docx(&after).unwrap().glossary.is_none());
    assert!(list(&r).iter().all(|b| b["store"] != "user"));
}

#[test]
fn f3_expands_library_snippets_by_name() {
    let mut e = engine(bo_engine::plain_text_document("nơi nhận"));
    cmd(
        &mut e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":"nơi nhận".len()},"focus":{"p":0,"o":"nơi nhận".len()}}}),
    );
    let r = cmd(&mut e, &json!({"cmd":"expandAutoText"}));
    assert_eq!(r["blocks"]["inserted"], "Nơi nhận");
    let text: Vec<String> = e
        .document()
        .unwrap()
        .paragraphs()
        .map(Paragraph::plain_text)
        .collect();
    assert_eq!(text[0], "Nơi nhận:");
    assert!(text.iter().any(|t| t.starts_with("- Lưu: VT")));
}

#[test]
fn header_footer_page_number_and_cover_galleries() {
    let mut e = engine(bo_engine::plain_text_document("Thân văn bản"));
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    let first = |gallery: &str| {
        list(&r)
            .iter()
            .find(|b| b["gallery"] == gallery)
            .unwrap_or_else(|| panic!("no {gallery}"))["key"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    cmd(
        &mut e,
        &json!({"cmd":"insertBuildingBlock","key":first("hdrs")}),
    );
    cmd(
        &mut e,
        &json!({"cmd":"insertBuildingBlock","key":first("pgNumB"),"page":0}),
    );
    let d = e.document().unwrap();
    let header = d.sections[0].headers.default.clone().unwrap();
    assert_ne!(header.len(), 0);
    let footer = d.sections[0].footers.default.as_ref().unwrap();
    let has_page_field = footer.iter().filter_map(Block::as_paragraph).any(|p| {
        p.runs.iter().any(|r| {
            r.content
                .iter()
                .any(|c| matches!(c, RunContent::Field { instr, .. } if instr.trim() == "PAGE"))
        })
    });
    assert!(has_page_field);
    // A top page number replaces the header.
    cmd(
        &mut e,
        &json!({"cmd":"insertBuildingBlock","key":first("pgNumT")}),
    );
    let d = e.document().unwrap();
    assert_ne!(d.sections[0].headers.default.as_ref().unwrap(), &header);
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(
        e.document().unwrap().sections[0]
            .headers
            .default
            .as_ref()
            .unwrap(),
        &header
    );
    // A cover page goes before the text, on a page of its own.
    let r = cmd(
        &mut e,
        &json!({"cmd":"insertBuildingBlock","key":first("coverPg")}),
    );
    assert!(r["info"]["pages"].as_array().unwrap().len() >= 2);
    let d = e.document().unwrap();
    let last = d.sections[0]
        .blocks
        .last()
        .and_then(Block::as_paragraph)
        .unwrap();
    assert_eq!(last.plain_text(), "Thân văn bản");
    // The result is a valid package that reads back.
    let bytes = e.save_docx().unwrap();
    let again = bo_docx::read_docx(&bytes).unwrap();
    assert!(again.sections[0].headers.default.is_some());
}

#[test]
fn every_library_block_inserts_and_has_a_preview() {
    let mut e = engine(bo_engine::plain_text_document("Thân"));
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    let blocks = list(&r);
    assert!(blocks.len() >= 26);
    for b in &blocks {
        let key = b["key"].as_str().unwrap();
        let r = cmd(&mut e, &json!({"cmd":"buildingBlockPreview","key":key}));
        let png =
            bo_engine::merge::base64_decode(r["blocks"]["preview"].as_str().unwrap()).unwrap();
        assert_eq!(&png[1..4], b"PNG", "{}", b["name"]);
        dump(&format!("preview-{}.png", key.replace(':', "-")), &png);
        cmd(&mut e, &json!({"cmd":"insertBuildingBlock","key":key}));
    }
    let bytes = e.save_docx().unwrap();
    dump("library-inserted.docx", &bytes);
    let d = bo_docx::read_docx(&bytes).unwrap();
    // Tables, text boxes and administrative snippets are in the text.
    let tables = d.sections[0]
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::Table(_)))
        .count();
    assert!(tables >= 4, "{tables}");
    let boxes = d
        .paragraphs()
        .flat_map(|p| p.runs.iter())
        .flat_map(|r| r.content.iter())
        .filter(|c| matches!(c, RunContent::Image(i) if i.shape.as_ref().is_some_and(|s| s.text.is_some())))
        .count();
    assert_eq!(boxes, 3);
}

#[test]
fn read_only_documents_refuse_edits_but_list_blocks() {
    let mut e = engine(source_doc());
    let r = cmd(&mut e, &json!({"cmd":"buildingBlocks"}));
    assert_ne!(list(&r).len(), 0);
    // A block command is not mistaken for another command family.
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"buildingBlockPreview","key":"nope"})),
        "noBlock"
    );
}

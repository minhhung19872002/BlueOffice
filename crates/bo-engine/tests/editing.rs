//! Editing through the JSON command bridge (the same path the web and desktop UIs use).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{EditResult, Engine};

fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

#[test]
fn type_vietnamese_bold_save_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("Văn bản mới.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":"Xin chào Việt Nam"}"#);
    assert_eq!(r["selection"]["focus"]["o"], "Xin chào Việt Nam".len());
    assert!(r["modified"].as_bool().unwrap());
    assert_eq!(r["dirty"], serde_json::json!([0]));
    let caret = &r["caret"];
    assert!(caret["x"].as_f64().unwrap() > 85.0);
    // Select "chào" and bold it.
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":4},"focus":{"p":0,"o":9}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert!(r["format"]["bold"].as_bool().unwrap());
    assert_eq!(r["rects"].as_array().unwrap().len(), 1);
    let saved = e.save_docx().unwrap();
    e.mark_saved();
    assert!(!cmd(&mut e, r#"{"cmd":"state"}"#)["modified"]
        .as_bool()
        .unwrap());

    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "Văn bản mới.docx").unwrap();
    let doc = f.document().unwrap();
    assert_eq!(doc.plain_text(), "Xin chào Việt Nam");
    let p = doc.paragraphs().next().unwrap();
    assert!(p
        .runs
        .iter()
        .any(|r| r.props.bold == Some(true) && r.plain_text() == "chào"));
    assert!(f.page_json(0).unwrap().contains("chào"));
}

#[test]
fn mouse_keyboard_undo_find_clipboard() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Dòng một\nDòng hai".as_bytes(), "a.txt").unwrap();
    // Click at the start of the second line.
    let r: EditResult = e
        .command(serde_json::from_str(r#"{"cmd":"click","page":0,"x":0,"y":95}"#).unwrap())
        .unwrap();
    assert_eq!((r.selection.focus.para, r.selection.focus.offset), (1, 0));
    let r = cmd(&mut e, r#"{"cmd":"move","motion":"up"}"#);
    assert_eq!(r["selection"]["focus"]["p"], 0);
    let r = cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd","extend":true}"#);
    assert_eq!(r["selection"]["focus"]["o"], "Dòng một".len());
    let r = cmd(
        &mut e,
        r#"{"cmd":"click","page":0,"x":135,"y":95,"count":2}"#,
    );
    assert_eq!(r["selection"]["anchor"]["p"], 1);
    let r = cmd(&mut e, r#"{"cmd":"copy"}"#);
    assert_eq!(r["clip"]["text"], "hai");
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    let r = cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"paste","internal":{}}}"#,
            serde_json::to_string(r["clip"]["internal"].as_str().unwrap()).unwrap()
        ),
    );
    assert!(r["error"].is_null());
    assert_eq!(e.document().unwrap().plain_text(), "Dòng một\nDòng haihai");
    let r = cmd(&mut e, r#"{"cmd":"find","query":"dòng"}"#);
    assert_eq!(r["matches"].as_array().unwrap().len(), 2);
    let r = cmd(
        &mut e,
        r#"{"cmd":"replaceAll","query":"Dòng","with":"Hàng","options":{"matchCase":true}}"#,
    );
    assert_eq!(r["count"], 2);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Dòng một\nDòng hai");
    let r = cmd(&mut e, r#"{"cmd":"redo"}"#);
    assert!(r["format"]["canUndo"].as_bool().unwrap());
    let r = cmd(&mut e, r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    assert!(r["error"].is_null());
    assert!(e.command_json(r#"{"cmd":"bogus"}"#).is_err());
}

#[test]
fn bad_commands_and_images() {
    let mut e = Engine::with_bundled_fonts();
    assert!(e.command_json(r#"{"cmd":"state"}"#).is_err()); // no document
    e.new_document("x.docx").unwrap();
    assert!(e.command_json("{").is_err());
    let r = cmd(&mut e, r#"{"cmd":"applyStyle","id":"Nope"}"#);
    assert!(r["error"].is_string());
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 96, 48);
        enc.set_color(png::ColorType::Rgb);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&vec![200; 96 * 48 * 3]).unwrap();
    }
    let r = e.insert_image(png, "a.png").unwrap();
    assert!(r.error.is_none());
    assert!(e.page_json(0).unwrap().contains(r#""op":"image""#));
    assert!(e.media("/word/media/image1.png").is_some());
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "x.docx").unwrap();
    assert_eq!(f.document().unwrap().media.len(), 1);
    assert!(e.insert_image(vec![1, 2, 3], "x.gif").is_err());
}

#[test]
fn track_changes_over_the_bridge_and_docx() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Hợp đồng số 1".as_bytes(), "a.txt").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Trần Thị B"}"#);
    let r = cmd(&mut e, r#"{"cmd":"setTrackChanges","on":true}"#);
    assert_eq!(r["review"]["trackChanges"], true);
    assert_eq!(r["review"]["author"], "Trần Thị B");
    // Select "số 1" and type over it: a deletion plus an insertion.
    let start = "Hợp đồng ".len();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{start}}},"focus":{{"p":0,"o":{}}}}}}}"#,
            "Hợp đồng số 1".len()
        ),
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertText","text":"mới","now":1794130215000}"#,
    );
    let revs = r["review"]["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 2);
    assert_eq!(revs[0]["kind"], "delete");
    assert_eq!(revs[1]["kind"], "insert");
    assert_eq!(revs[1]["author"], "Trần Thị B");
    assert_eq!(revs[1]["date"], "2026-11-08T09:30:15Z");
    // Markup view shows both; the final view hides the deletion.
    let page = e.page_json(0).unwrap();
    assert!(page.contains("số 1") && page.contains("mới"));
    cmd(&mut e, r#"{"cmd":"setRevisionView","view":"final"}"#);
    assert!(!e.page_json(0).unwrap().contains("số 1"));
    cmd(&mut e, r#"{"cmd":"setRevisionView","view":"markup"}"#);
    // Saved as w:del / w:ins (ECMA-376 Part 1, §17.13.5) and read back.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    assert_eq!(r["review"]["revisions"].as_array().unwrap().len(), 2);
    let r = cmd(&mut f, r#"{"cmd":"acceptAllRevisions"}"#);
    assert_eq!(r["count"], 2);
    assert_eq!(f.document().unwrap().plain_text(), "Hợp đồng mới");
    cmd(&mut f, r#"{"cmd":"undo"}"#);
    let id = cmd(&mut f, r#"{"cmd":"review"}"#)["review"]["revisions"][1]["id"].clone();
    let r = cmd(&mut f, &format!(r#"{{"cmd":"rejectRevision","id":{id}}}"#));
    assert_eq!(r["count"], 1);
    let r = cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(f.document().unwrap().plain_text(), "Hợp đồng số 1");
}

#[test]
fn toc_captions_and_cross_references() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Mở đầu\nChương I\nNội dung\nChương II".as_bytes(), "a.txt")
        .unwrap();
    // Headings: paragraphs 1 and 3; a page break puts "Chương II" on page 2.
    let sel = |p: usize, o: usize| {
        format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{o}}},"focus":{{"p":{p},"o":{o}}}}}}}"#
        )
    };
    cmd(&mut e, &sel(1, 0));
    cmd(&mut e, r#"{"cmd":"applyStyle","id":"Heading1"}"#);
    cmd(&mut e, &sel(3, 0));
    cmd(&mut e, r#"{"cmd":"applyStyle","id":"Heading1"}"#);
    cmd(&mut e, &sel(2, "Nội dung".len()));
    cmd(&mut e, r#"{"cmd":"insertPageBreak"}"#);
    // TOC at the very start (ECMA-376 Part 1, §17.16.5.68).
    cmd(&mut e, &sel(0, 0));
    let r = cmd(&mut e, r#"{"cmd":"insertToc"}"#);
    assert!(r["error"].is_null(), "{r}");
    let text = e.document().unwrap().plain_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Chương I\t1");
    // The TOC pushes nothing to a new page here: Chương II stays on page 2.
    assert_eq!(lines[1], "Chương II\t2");
    // Captions number themselves (SEQ) and can be cross-referenced (REF / PAGEREF).
    let last = e.session().unwrap().paragraph_count() - 1;
    cmd(&mut e, &sel(last, 0));
    cmd(
        &mut e,
        r#"{"cmd":"insertCaption","label":"Hình","text":"Sơ đồ"}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"insertCaption","label":"Hình","text":"Bảng giá"}"#,
    );
    let text = e.document().unwrap().plain_text();
    assert!(text.contains("Hình 1: Sơ đồ\nHình 2: Bảng giá"), "{text}");
    let r = cmd(&mut e, r#"{"cmd":"crossReferenceTargets"}"#);
    let targets = r["targets"].as_array().unwrap();
    let t = targets
        .iter()
        .find(|t| t["label"] == "Hình 2: Bảng giá")
        .unwrap();
    cmd(&mut e, &sel(2, 0));
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"insertCrossReference","target":{}}}"#, t["id"]),
    );
    assert!(r["error"].is_null(), "{r}");
    let r = cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"insertCrossReference","target":{},"page":true}}"#,
            t["id"]
        ),
    );
    assert!(r["error"].is_null(), "{r}");
    let p2 = e
        .document()
        .unwrap()
        .paragraphs()
        .nth(2)
        .unwrap()
        .plain_text();
    assert!(p2.starts_with("Hình 22"), "{p2}");
    // A caption inserted before renumbers after "update fields".
    cmd(&mut e, &sel(2, 0));
    cmd(
        &mut e,
        r#"{"cmd":"insertCaption","label":"Hình","text":"Mới"}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    assert!(r["count"].as_u64().unwrap() >= 3, "{r}");
    let text = e.document().unwrap().plain_text();
    assert!(text.contains("Hình 2: Sơ đồ\nHình 3: Bảng giá"), "{text}");
    let p2 = e
        .document()
        .unwrap()
        .paragraphs()
        .nth(2)
        .unwrap()
        .plain_text();
    assert!(p2.starts_with("Hình 32"), "{p2}");
    // Saved and read back with the TOC as a complex field and SEQ/REF as fields.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(
        f.document().unwrap().plain_text(),
        e.document().unwrap().plain_text()
    );
    let r = cmd(&mut f, r#"{"cmd":"updateFields"}"#);
    assert_eq!(r["count"], 0);
}

fn corpus(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc")
        .join(name);
    std::fs::read(p).unwrap()
}

#[test]
fn update_fields_on_corpus_files() {
    let lines = |e: &Engine| -> Vec<String> {
        e.document()
            .unwrap()
            .plain_text()
            .lines()
            .map(str::to_owned)
            .collect()
    };
    // Stale TOC → regenerated from headings with page numbers from layout.
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("fields-04-toc-stale.docx"), "toc.docx")
        .unwrap();
    let r = cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(
        lines(&e)[..4],
        [
            "Chương I. Quy định chung\t1",
            "1. Phạm vi điều chỉnh\t1",
            "1.1. Đối tượng áp dụng\t2",
            "Chương II. Tổ chức thực hiện\t2"
        ]
    );
    // Undo restores the stale table in one step.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(lines(&e)[0], "Chương cũ\t9");
    // SEQ: ARABIC / ROMAN numbering, \c repeats, \r restarts.
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("fields-05-captions-seq.docx"), "seq.docx")
        .unwrap();
    cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    let l = lines(&e);
    assert_eq!(l[0], "Hình 1: Sơ đồ tổ chức");
    assert_eq!(l[1], "Bảng I: Dự toán");
    assert_eq!(l[3], "Hình 2: Quy trình");
    assert_eq!(l[4], "(nhắc lại số hình: 2)");
    assert_eq!(l[5], "Bảng 10: Phụ lục, đánh lại từ 10");
    // REF / PAGEREF follow their bookmarks.
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("fields-06-crossref.docx"), "ref.docx")
        .unwrap();
    cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    assert_eq!(
        lines(&e)[0],
        "Xem Hình 1 ở trang 2; điều khoản: Điều 5 – Hiệu lực thi hành."
    );
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for ch in bytes.chunks(3) {
        let n = ch.iter().fold(0u32, |a, &b| (a << 8) | u32::from(b)) << (8 * (3 - ch.len()));
        for i in 0..=ch.len() {
            s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    s
}

#[test]
fn mail_merge_csv_and_xlsx() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Kính gửi: \nSố tiền: ".as_bytes(), "thu.txt")
        .unwrap();
    let csv = "Họ tên,Số tiền\nNguyễn Văn An,1.500.000\nTrần Thị Bình,2.000.000\n";
    let r = cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"mergeData","name":"ds.csv","base64":"{}"}}"#,
            b64(csv.as_bytes())
        ),
    );
    assert_eq!(r["merge"]["count"], 2);
    assert_eq!(r["merge"]["fields"][0], "Họ tên");
    cmd(&mut e, r#"{"cmd":"move","motion":"docStart"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    cmd(&mut e, r#"{"cmd":"mergeInsertField","name":"Họ tên"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"mergeInsertField","name":"Số tiền"}"#);
    assert_eq!(
        e.document().unwrap().plain_text(),
        "Kính gửi: «Họ tên»\nSố tiền: «Số tiền»"
    );
    // Preview record 2 (ECMA-376 Part 1, §17.16.5.35 MERGEFIELD).
    let r = cmd(&mut e, r#"{"cmd":"mergePreview","record":1}"#);
    assert_eq!(r["merge"]["preview"], 1);
    assert!(e.layout().unwrap().page_text(0).contains("Trần Thị Bình"));
    cmd(&mut e, r#"{"cmd":"mergePreview"}"#);
    assert!(e.layout().unwrap().page_text(0).contains("«Họ tên»"));
    // The template saves with MERGEFIELD fields and reads them back.
    let template = e.save_docx().unwrap();
    // Merge: one section per record, then .docx and PDF output.
    let r = cmd(&mut e, r#"{"cmd":"mergeToDocument"}"#);
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["info"]["name"], "thu-merged.docx");
    assert_eq!(r["info"]["pageCount"], 2);
    let doc = e.document().unwrap();
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(
        doc.plain_text(),
        "Kính gửi: Nguyễn Văn An\nSố tiền: 1.500.000\nKính gửi: Trần Thị Bình\nSố tiền: 2.000.000"
    );
    let merged = e.save_docx().unwrap();
    assert!(e.export_pdf().unwrap().starts_with(b"%PDF"));
    let mut f = Engine::with_bundled_fonts();
    f.open(&merged, "m.docx").unwrap();
    assert_eq!(f.document().unwrap().sections.len(), 2);
    // XLSX data source with the reopened template.
    let mut wb = bo_model::sheet::Workbook::new();
    let ws = &mut wb.sheets[0];
    for (r, row) in [["Họ tên", "Số tiền"], ["Lê Văn Cường", "3"]]
        .iter()
        .enumerate()
    {
        for (c, v) in row.iter().enumerate() {
            ws.cells.insert(
                (r as u32, c as u32),
                bo_model::sheet::Cell {
                    value: bo_model::sheet::CellValue::Text((*v).into()),
                    ..Default::default()
                },
            );
        }
    }
    let xlsx = bo_xlsx::write_xlsx(&wb).unwrap();
    let mut g = Engine::with_bundled_fonts();
    g.open(&template, "thu.docx").unwrap();
    cmd(
        &mut g,
        &format!(
            r#"{{"cmd":"mergeData","name":"ds.xlsx","base64":"{}"}}"#,
            b64(&xlsx)
        ),
    );
    cmd(&mut g, r#"{"cmd":"mergeToDocument"}"#);
    assert_eq!(
        g.document().unwrap().plain_text(),
        "Kính gửi: Lê Văn Cường\nSố tiền: 3"
    );
}

#[test]
fn mail_merge_corpus_templates() {
    for (file, expect) in [
        ("merge-01-letter.docx", "Đơn vị: VĂN PHÒNG UBND"),
        ("merge-02-table.docx", "Nguyễn Văn An, Giám đốc"),
        ("merge-03-header-sections.docx", "Hồ sơ của Lê Văn Cường"),
    ] {
        let mut e = Engine::with_bundled_fonts();
        e.open(&corpus(file), file).unwrap();
        for data in ["merge-data.csv", "merge-data.xlsx"] {
            assert_eq!(e.set_merge_data(&corpus(data), data).unwrap(), 3);
        }
        cmd(&mut e, r#"{"cmd":"mergePreview","record":2}"#);
        let r = cmd(&mut e, r#"{"cmd":"mergeToDocument"}"#);
        assert!(r["error"].is_null(), "{file}: {r}");
        let text = e.document().unwrap().plain_text();
        assert!(text.contains(expect), "{file}: {text}");
        // "Trần Thị Bình" has no position: the \b ", " prefix disappears.
        assert!(!text.contains("Bình, "), "{file}: {text}");
    }
}

#[test]
fn comment_threads_save_and_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Quyết định số 12".as_bytes(), "a.txt").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Nguyễn Văn An"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":3}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"addComment","text":"Viết hoa?"}"#);
    let id = r["count"].as_i64().unwrap();
    assert_eq!(r["review"]["comments"][0]["quote"], "Quy");
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Trần Thị Bình"}"#);
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"replyComment","id":{id},"text":"Giữ nguyên."}}"#),
    );
    assert_eq!(r["review"]["comments"][1]["parent"], id);
    cmd(&mut e, &format!(r#"{{"cmd":"resolveComment","id":{id}}}"#));
    // comments.xml + commentsExtended.xml ([MS-DOCX] paraIdParent, done).
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"review"}"#);
    let list = r["review"]["comments"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["author"], "Nguyễn Văn An");
    assert_eq!(list[0]["done"], true);
    assert_eq!(list[1]["text"], "Giữ nguyên.");
    assert_eq!(list[1]["parent"], list[0]["id"]);
    assert_eq!(list[1]["quote"], "Quy");
    let r = cmd(&mut f, &format!(r#"{{"cmd":"selectComment","id":{id}}}"#));
    assert_eq!(r["selection"]["focus"]["o"], 3);
    let r = cmd(&mut f, &format!(r#"{{"cmd":"deleteComment","id":{id}}}"#));
    assert_eq!(r["review"]["comments"].as_array().unwrap().len(), 0);
    assert_eq!(f.document().unwrap().plain_text(), "Quyết định số 12");
}

// source: prompt §7.1 "So sánh" — test pair: the edited document compared with the original it
// came from; ECMA-376 Part 1, §17.13.5: rejecting every revision gives the original back,
// accepting them the edited text; one undo step removes the comparison.
#[test]
fn compare_with_the_original_document() {
    let mut e = Engine::with_bundled_fonts();
    e.open(
        "Hợp đồng số 1 ký ngày 5.\nĐiều 1: Giá trị.\nĐiều 2: Thời hạn.".as_bytes(),
        "a.txt",
    )
    .unwrap();
    let original = e.save_docx().unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Người so sánh"}"#);
    // An edit without tracking: the date "5" becomes "7".
    let o = "Hợp đồng số 1 ký ngày ".len();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{o}}},"focus":{{"p":0,"o":{}}}}}}}"#,
            o + 1
        ),
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"7"}"#);
    let edited = e.document().unwrap().plain_text();
    assert!(edited.contains("ngày 7"));
    let b64 = b64(&original);
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"compareWith","name":"goc.docx","base64":"{b64}"}}"#),
    );
    assert!(r["count"].as_u64().unwrap() >= 2, "{}", r["count"]);
    let revs = r["review"]["revisions"].as_array().unwrap();
    assert!(
        revs.iter().any(|x| x["kind"] == "delete") && revs.iter().any(|x| x["kind"] == "insert")
    );
    assert!(revs.iter().all(|x| x["author"] == "Người so sánh"));
    // Saved with the revisions, the comparison reads back; reject all → the original.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "so-sanh.docx").unwrap();
    cmd(&mut f, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(
        f.document().unwrap().plain_text(),
        "Hợp đồng số 1 ký ngày 5.\nĐiều 1: Giá trị.\nĐiều 2: Thời hạn."
    );
    cmd(&mut f, r#"{"cmd":"undo"}"#);
    cmd(&mut f, r#"{"cmd":"acceptAllRevisions"}"#);
    assert_eq!(f.document().unwrap().plain_text(), edited);
    // Undo in the comparing editor: the edited document without revisions.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(
        cmd(&mut e, r#"{"cmd":"review"}"#)["review"]["revisions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"compareWith","name":"x.docx","base64":"bm9wZQ=="}"#,
    );
    assert!(r["error"].is_string());
}

/// Select bytes `a..b` of paragraph `p` and type `text` (tracked when tracking is on).
fn replace(e: &mut Engine, p: usize, a: usize, b: usize, text: &str) {
    cmd(
        e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{a}}},"focus":{{"p":{p},"o":{b}}}}}}}"#
        ),
    );
    cmd(e, &format!(r#"{{"cmd":"insertText","text":"{text}"}}"#));
}

// source: prompt §7.1 "Kết hợp" — two reviewers track their edits in copies of one original; the
// combined document holds both, by author; rejecting all gives the original, accepting all both.
#[test]
fn combine_two_reviewers_copies() {
    let text = "Giá 100 triệu.\nThời hạn một năm.";
    let mut lan = Engine::with_bundled_fonts();
    lan.open(text.as_bytes(), "a.txt").unwrap();
    let mut minh = Engine::with_bundled_fonts();
    minh.open(&lan.save_docx().unwrap(), "b.docx").unwrap();
    cmd(&mut lan, r#"{"cmd":"setAuthor","name":"Lan"}"#);
    cmd(&mut lan, r#"{"cmd":"setTrackChanges","on":true}"#);
    replace(&mut lan, 0, "Giá ".len(), "Giá 100".len(), "120");
    cmd(&mut minh, r#"{"cmd":"setAuthor","name":"Minh"}"#);
    cmd(&mut minh, r#"{"cmd":"setTrackChanges","on":true}"#);
    replace(&mut minh, 1, "Thời hạn ".len(), "Thời hạn một".len(), "hai");
    let theirs = minh.save_docx().unwrap();
    let r = cmd(
        &mut lan,
        &format!(
            r#"{{"cmd":"combineWith","name":"ban-cua-Minh.docx","base64":"{}"}}"#,
            b64(&theirs)
        ),
    );
    let revs = r["review"]["revisions"].as_array().unwrap();
    let authors: std::collections::BTreeSet<&str> =
        revs.iter().map(|x| x["author"].as_str().unwrap()).collect();
    assert_eq!(authors.into_iter().collect::<Vec<_>>(), vec!["Lan", "Minh"]);
    cmd(&mut lan, r#"{"cmd":"acceptAllRevisions"}"#);
    assert_eq!(
        lan.document().unwrap().plain_text(),
        "Giá 120 triệu.\nThời hạn hai năm."
    );
    cmd(&mut lan, r#"{"cmd":"undo"}"#);
    cmd(&mut lan, r#"{"cmd":"rejectAllRevisions"}"#);
    assert_eq!(lan.document().unwrap().plain_text(), text);
}

// source: ECMA-376 Part 1, §17.16.5 fields over the bridge: inserted fields show their result,
// read the document properties and the clock, and keep their switches through save / reopen.
#[test]
fn fields_show_their_results() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("Báo cáo.docx").unwrap();
    e.set_clock_ms(1_794_204_300_000); // 2026-11-09T05:25:00Z
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Lê Văn Cường"}"#);
    for f in [
        r"USERNAME",
        r"USERINITIALS",
        r"FILENAME",
        r#"DATE \\@ \"dd/MM/yyyy\""#,
        r"= 21 * 2 \\* CardText",
        r"PAGE \\* ROMAN",
    ] {
        cmd(&mut e, &format!(r#"{{"cmd":"insertField","instr":"{f}"}}"#));
        cmd(&mut e, r#"{"cmd":"insertText","text":" | "}"#);
    }
    let text = e.document().unwrap().plain_text();
    assert_eq!(
        text,
        "Lê Văn Cường | LVC | Báo cáo.docx | 09/11/2026 | bốn mươi hai | 1 | "
    );
    // The layout shows the page number with its switch.
    let page = e.page_json(0).unwrap();
    assert!(
        page.contains("hai | I | "),
        "PAGE with a ROMAN format shows I"
    );
}

// source: ECMA-376 Part 1, §17.16.18 fldLock — a locked field keeps its result through updates
// and saving; unlocked it updates again; field codes show instead of results; unlinking leaves
// the result as text.
#[test]
fn lock_unlink_and_show_field_codes() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Lan"}"#);
    cmd(&mut e, r#"{"cmd":"insertField","instr":"USERNAME"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Lan");
    // The caret is right after the field: lock it, change the user, update.
    let r = cmd(&mut e, r#"{"cmd":"lockFields","lock":true}"#);
    assert_eq!(r["count"], 1);
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Minh"}"#);
    cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Lan");
    // Saved locked (w:fldLock) and read back locked.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    cmd(&mut f, r#"{"cmd":"setAuthor","name":"Minh"}"#);
    cmd(&mut f, r#"{"cmd":"updateFields"}"#);
    assert_eq!(f.document().unwrap().plain_text(), "Lan");
    // Unlocked: the update shows the new user.
    cmd(&mut e, r#"{"cmd":"lockFields","lock":false}"#);
    cmd(&mut e, r#"{"cmd":"updateFields"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Minh");
    // Field codes on the page instead of the result.
    cmd(&mut e, r#"{"cmd":"showFieldCodes","on":true}"#);
    assert!(e.page_json(0).unwrap().contains("{ USERNAME }"));
    cmd(&mut e, r#"{"cmd":"showFieldCodes","on":false}"#);
    assert!(!e.page_json(0).unwrap().contains("{ USERNAME }"));
    // Unlinked: plain text, no field left.
    let r = cmd(&mut e, r#"{"cmd":"unlinkFields"}"#);
    assert_eq!(r["count"], 1);
    let fields = e.document().unwrap().sections[0]
        .blocks
        .iter()
        .filter_map(|b| b.as_paragraph())
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.content)
        .filter(|c| matches!(c, bo_model::RunContent::Field { .. }))
        .count();
    assert_eq!(fields, 0);
    assert_eq!(e.document().unwrap().plain_text(), "Minh");
}

// source: ECMA-376 Part 1, §17.5.2 content controls (sdt) — inserted, filled (checkbox, list,
// date), saved and read back with their kind and value.
#[test]
fn repeating_sections_pictures_and_control_properties() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thành viên: "}"#);
    // A repeating section around the paragraph; the caret is in its only item.
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"repeatingSection","placeholder":"Danh sách"}"#,
    );
    assert_eq!(r["control"]["kind"], "repeatingSection");
    assert_eq!(r["control"]["title"], "Danh sách");
    assert_eq!(r["control"]["block"], true);
    assert_eq!(r["control"]["repeating"], true);
    assert_ne!(r["control"]["frame"], serde_json::json!([]));
    // Its "+": a second item, the caret at its start.
    let r = cmd(&mut e, r#"{"cmd":"addRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(r["selection"]["focus"]["p"], 1);
    assert_eq!(r["control"]["kind"], "repeatingSection");
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    // A picture control in the new item, then its properties.
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"picture","placeholder":"Ảnh chân dung"}"#,
    );
    assert_eq!(r["control"]["kind"], "picture");
    assert_eq!(r["control"]["placeholder"], true);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setControlProps","props":{"title":"Ảnh","tag":"anh","deleteLocked":true,"contentLocked":false}}"#,
    );
    assert_eq!(r["count"], 1);
    assert_eq!(r["control"]["title"], "Ảnh");
    assert_eq!(r["control"]["tag"], "anh");
    assert_eq!(r["control"]["deleteLocked"], true);
    assert_eq!(r["control"]["locked"], false);
    // Saved with w15 repeating sections (two items) and read back the same.
    let saved = e.save_docx().unwrap();
    let pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert_eq!(xml.matches("w15:repeatingSectionItem").count(), 2);
    assert_eq!(xml.matches("<w15:repeatingSection ").count(), 1);
    assert!(xml.contains("<w:alias w:val=\"Ảnh\"/><w:tag w:val=\"anh\"/>"));
    assert!(xml.contains("<w:lock w:val=\"sdtLocked\"/>"));
    assert!(xml.contains("<w:picture/>"));
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(
        &mut f,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":2},"focus":{"p":0,"o":2}}}"#,
    );
    assert_eq!(r["control"]["kind"], "repeatingSection");
    assert_eq!(r["control"]["repeating"], true);
    let r = cmd(&mut f, r#"{"cmd":"addRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(
        f.document()
            .unwrap()
            .plain_text()
            .matches("Thành viên")
            .count(),
        3
    );
}

#[test]
fn fields_next_prompts_and_checkbox_by_keyboard_and_mouse() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Tên: "}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertField","instr":"FILLIN \"Tên khách?\" \\d \"Lan\""}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":" trang "}"#);
    cmd(&mut e, r#"{"cmd":"insertField","instr":"PAGE"}"#);
    let r = cmd(&mut e, r#"{"cmd":"fieldPrompts"}"#);
    assert_eq!(r["fieldPrompts"][0]["key"], "FILLIN:Tên khách?");
    assert_eq!(r["fieldPrompts"][0]["default"], "Lan");
    let r = cmd(
        &mut e,
        r#"{"cmd":"updateFields","answers":{"FILLIN:Tên khách?":"Minh"}}"#,
    );
    assert!(r.get("error").is_none());
    assert_eq!(e.document().unwrap().plain_text(), "Tên: Minh trang 1");
    // F11 from the start selects the first field, then the next; Shift+F11 goes back.
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":0}}}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"nextField","forward":true}"#);
    assert_eq!(r["selection"]["anchor"]["o"], "Tên: ".len());
    let r = cmd(&mut e, r#"{"cmd":"nextField","forward":true}"#);
    let page_at = r["selection"]["anchor"]["o"].as_u64().unwrap();
    assert!(page_at > "Tên: ".len() as u64);
    let r = cmd(&mut e, r#"{"cmd":"nextField","forward":false}"#);
    assert_eq!(r["selection"]["anchor"]["o"], "Tên: ".len());
    // A check box: Space toggles it when selected; a click on its mark toggles it too.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" Đồng ý "}"#);
    let r = cmd(&mut e, r#"{"cmd":"insertControl","kind":"checkbox"}"#);
    assert_eq!(r["control"]["checked"], false);
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":" "}"#);
    assert_eq!(r["control"]["checked"], true);
    let mark = r["rects"][0].clone();
    let (page, x, y) = (
        mark["page"].as_u64().unwrap(),
        mark["x"].as_f64().unwrap() + mark["w"].as_f64().unwrap() / 2.0,
        mark["y"].as_f64().unwrap() + mark["h"].as_f64().unwrap() / 2.0,
    );
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"click","page":{page},"x":{x},"y":{y}}}"#),
    );
    assert_eq!(r["control"]["checked"], false);
    // A click in a placeholder selects all of it.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" "}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"text","placeholder":"Nhập ghi chú."}"#,
    );
    let ph = r["rects"][0].clone();
    let x = ph["x"].as_f64().unwrap() + ph["w"].as_f64().unwrap() / 3.0;
    let y = ph["y"].as_f64().unwrap() + ph["h"].as_f64().unwrap() / 2.0;
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"click","page":0,"x":{x},"y":{y}}}"#),
    );
    let (a, b) = (
        r["selection"]["anchor"]["o"].as_u64().unwrap(),
        r["selection"]["focus"]["o"].as_u64().unwrap(),
    );
    assert_eq!(b - a, "Nhập ghi chú.".len() as u64);
}

#[test]
fn corpus_repeating_section_round_trips_and_grows() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/controls-04-repeating-picture.docx");
    let bytes = std::fs::read(path).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "r.docx").unwrap();
    // Untouched, the file saves its controls as read.
    let again = e.save_docx().unwrap();
    let xml = |b: &[u8]| {
        let pkg = bo_opc::Package::read(b, &bo_opc::Limits::default()).unwrap();
        String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap()
    };
    assert_eq!(xml(&again).matches("w15:repeatingSectionItem").count(), 2);
    // In the second item: "+" adds a third after it.
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":1,"o":3},"focus":{"p":1,"o":3}}}"#,
    );
    assert_eq!(r["control"]["kind"], "repeatingSection");
    assert_eq!(r["control"]["title"], "Danh sách");
    let r = cmd(&mut e, r#"{"cmd":"addRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    let saved = e.save_docx().unwrap();
    assert_eq!(xml(&saved).matches("w15:repeatingSectionItem").count(), 3);
    // Still one section (the other matches are its items).
    assert_eq!(xml(&saved).matches("<w15:repeatingSection").count(), 4);
    let texts = e.document().unwrap().plain_text();
    assert!(
        texts.contains("Thành viên: Bình\nThành viên: Bình\nẢnh"),
        "{texts}"
    );
    // The picture control shows its placeholder; the text control cannot be deleted.
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":3,"o":8},"focus":{"p":3,"o":8}}}"#,
    );
    assert_eq!(r["control"]["kind"], "picture");
    assert_eq!(r["control"]["placeholder"], true);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":4,"o":15},"focus":{"p":4,"o":15}}}"#,
    );
    assert_eq!(r["control"]["deleteLocked"], true);
    assert_eq!(r["control"]["tag"], "ma_ho_so");
}

/// Corpus `controls-06-repeating-rows.docx`: a repeating section of table rows (§17.5.2.32 sdt
/// around w:tr, [MS-DOCX] w15:repeatingSection). The caret in a row of an item reports the
/// section; "+" adds a row after it and "−" deletes one, one undo step each; saved, the rows
/// stay wrapped in their controls.
#[test]
fn corpus_repeating_rows_grow_and_shrink() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/controls-06-repeating-rows.docx");
    let bytes = std::fs::read(path).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "r.docx").unwrap();
    let xml = |b: &[u8]| {
        let pkg = bo_opc::Package::read(b, &bo_opc::Limits::default()).unwrap();
        String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap()
    };
    assert_eq!(
        xml(&e.save_docx().unwrap())
            .matches("<w15:repeatingSectionItem")
            .count(),
        2
    );
    let text = |e: &Engine| e.document().unwrap().plain_text();
    // Paragraph 4 = "Bình" (header 0-1, An 2-3).
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":4,"o":1},"focus":{"p":4,"o":1}}}"#,
    );
    assert_eq!(r["control"]["kind"], "repeatingSection");
    assert_eq!(r["control"]["title"], "Thành viên");
    assert_eq!(r["control"]["repeating"], true);
    let r = cmd(&mut e, r#"{"cmd":"addRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(r["selection"]["focus"]["p"], 6);
    assert!(
        text(&e).contains("Bình\tThành viên\nBình\tThành viên\nTổng"),
        "{}",
        text(&e)
    );
    let saved = e.save_docx().unwrap();
    let x = xml(&saved);
    assert_eq!(x.matches("<w15:repeatingSectionItem").count(), 3, "{x}");
    assert_eq!(x.matches("<w15:repeatingSection/>").count(), 1);
    // The total row stays after the section.
    let at = |s: &str| x.find(s).unwrap();
    assert!(x.rfind("</w:sdtContent></w:sdt>").unwrap() < at(">Tổng<"));
    // Read back: still three items, "−" on the first one leaves two.
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "r.docx").unwrap();
    cmd(
        &mut f,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":2,"o":0},"focus":{"p":2,"o":0}}}"#,
    );
    let r = cmd(&mut f, r#"{"cmd":"deleteRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    assert!(!text(&f).contains("An"), "{}", text(&f));
    let x = xml(&f.save_docx().unwrap());
    assert_eq!(x.matches("<w15:repeatingSectionItem").count(), 2);
    assert_eq!(x.matches("<w15:repeatingSection/>").count(), 1);
    // Undo brings the row back.
    cmd(&mut f, r#"{"cmd":"undo"}"#);
    assert!(text(&f).contains("An\tTrưởng nhóm"));
    // BO_CC_OUT=<dir>: keep the edited file (to check with `bo-cli validate --openxml require`).
    if let Ok(dir) = std::env::var("BO_CC_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("repeating-rows.docx"),
            &saved,
        )
        .unwrap();
    }
}

#[test]
fn ctrl_f9_turns_the_selected_text_into_a_field() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Mục SECTION"}"#);
    // Nothing selected: nothing to wrap.
    let r = cmd(&mut e, r#"{"cmd":"insertFieldFromSelection"}"#);
    assert_eq!(r["count"], 0);
    let from = "Mục ".len();
    let to = "Mục SECTION".len();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{from}}},"focus":{{"p":0,"o":{to}}}}}}}"#
        ),
    );
    let r = cmd(&mut e, r#"{"cmd":"insertFieldFromSelection"}"#);
    assert_eq!(r["count"], 1);
    // The field shows its result at once (SECTION → 1, ECMA-376 Part 1 SECTION field).
    assert_eq!(e.document().unwrap().plain_text(), "Mục 1");
}

#[test]
fn content_controls_insert_fill_and_save() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Đồng ý: "}"#);
    let r = cmd(&mut e, r#"{"cmd":"insertControl","kind":"checkbox"}"#);
    assert_eq!(r["control"]["kind"], "checkbox");
    assert_eq!(r["control"]["checked"], false);
    let r = cmd(&mut e, r#"{"cmd":"setControlValue"}"#);
    assert_eq!(r["control"]["checked"], true);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    // Typed after the check box, outside it.
    let r = cmd(&mut e, r#"{"cmd":"insertText","text":" Loại: "}"#);
    assert!(r.get("control").is_none());
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"dropDown","placeholder":"Chọn một mục","items":["Hợp đồng","Phụ lục"]}"#,
    );
    assert_eq!(r["control"]["placeholder"], true);
    assert_eq!(
        r["control"]["items"],
        serde_json::json!(["Hợp đồng", "Phụ lục"])
    );
    cmd(&mut e, r#"{"cmd":"setControlValue","value":"Phụ lục"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" Ngày: "}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"date","placeholder":"Chọn ngày"}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"setControlValue","value":"2026-11-09"}"#);
    assert_eq!(r["control"]["date"], "2026-11-09T00:00:00Z");
    assert_eq!(
        e.document().unwrap().plain_text(),
        "Đồng ý: ☒ Loại: Phụ lục Ngày: 09/11/2026"
    );
    // Saved as w:sdt and read back as controls.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(
        f.document().unwrap().plain_text(),
        "Đồng ý: ☒ Loại: Phụ lục Ngày: 09/11/2026"
    );
    let n = "Đồng ý: ".len() + 1;
    let r = cmd(
        &mut f,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{n}}},"focus":{{"p":0,"o":{n}}}}}}}"#
        ),
    );
    assert_eq!(r["control"]["kind"], "checkbox");
    assert_eq!(r["control"]["checked"], true);
    // Untouched controls keep their properties as read.
    let saved2 = f.save_docx().unwrap();
    let mut g = Engine::with_bundled_fonts();
    g.open(&saved2, "b.docx").unwrap();
    assert_eq!(
        g.document().unwrap().plain_text(),
        f.document().unwrap().plain_text()
    );
}

#[test]
fn content_control_split_by_enter_still_saves_well_formed() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"richText","placeholder":"Nội dung"}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":"đầu cuối"}"#);
    let n = "đầu".len();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{n}}},"focus":{{"p":0,"o":{n}}}}}}}"#
        ),
    );
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(
        f.document().unwrap().plain_text(),
        "đầu
 cuối"
    );
}

#[test]
fn equation_templates_and_numbered_equations() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"equationTemplates"}"#);
    let list = r["templates"].as_array().unwrap();
    assert!(list.len() >= 10);
    assert!(list.iter().any(|t| t["id"] == "quadratic"));
    cmd(&mut e, r#"{"cmd":"insertText","text":"Định lý:"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertEquation","text":"a^2+b^2=c^2","numbered":"Phương_trình"}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"insertEquation","text":"e^(iπ)+1=0","numbered":"Phương_trình"}"#,
    );
    let doc = e.document().unwrap();
    let text = doc.plain_text();
    // Each equation is its own paragraph, numbered (1) and (2) at the right margin.
    assert!(text.contains("\t(1)"), "{text:?}");
    assert!(text.contains("\t(2)"), "{text:?}");
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let back = f.document().unwrap().plain_text();
    assert!(back.contains("(1)") && back.contains("(2)"), "{back:?}");
    // A label with a space is refused.
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertEquation","text":"x","numbered":"Phương trình"}"#,
    );
    assert!(r.get("error").is_some_and(|v| !v.is_null()), "{r}");
}

fn caret_at(e: &mut Engine, p: usize, o: usize) -> serde_json::Value {
    cmd(
        e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{o}}},"focus":{{"p":{p},"o":{o}}}}}}}"#
        ),
    )
}

// Reference behaviour (word processor): F9 updates the field at the caret only; Shift+F9 shows
// that field's code; Ctrl+F9 puts empty braces where the code is typed, F9 then shows the result.
/// Shift+F9 on one field, then on the other one at the end of the line: each toggles alone.
#[test]
fn shift_f9_toggles_each_field_alone() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    // Made as Ctrl+F9 makes them: the typed code selected, then wrapped.
    let field = |e: &mut Engine, code: &str| {
        let r = cmd(
            e,
            &serde_json::json!({ "cmd": "insertText", "text": code }).to_string(),
        );
        let o = r["selection"]["focus"]["o"].as_u64().unwrap();
        let from = o - code.len() as u64;
        cmd(
            e,
            &format!(
                r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":0,"o":{from}}},"focus":{{"p":0,"o":{o}}}}}}}"#
            ),
        );
        cmd(e, r#"{"cmd":"insertFieldFromSelection"}"#);
        cmd(e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    };
    field(&mut e, "= 6*7 + 0");
    cmd(&mut e, r#"{"cmd":"insertText","text":" và "}"#);
    field(&mut e, "= 2+3 + 0");
    assert_eq!(e.document().unwrap().plain_text(), "42 và 5");
    cmd(&mut e, r#"{"cmd":"move","motion":"docStart"}"#);
    let r = cmd(&mut e, r#"{"cmd":"toggleFieldCodes"}"#);
    assert_eq!(r["count"], 1);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    let r = cmd(&mut e, r#"{"cmd":"toggleFieldCodes"}"#);
    assert_eq!(r["count"], 1, "{r}");
    let page = e.page_json(0).unwrap();
    assert!(page.contains("{ = 6*7 + 0 }") && page.contains("{ = 2+3 + 0 }"));
}

/// The caret right after a field showing its code (the last thing of its paragraph, or
/// before more text) is after the closing brace, not after the code's first word.
#[test]
fn caret_after_a_field_showing_its_code() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"ab = 2+3 + 0"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":3},"focus":{"p":0,"o":12}}}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertFieldFromSelection"}"#);
    let end = |e: &mut Engine| {
        let r = cmd(e, r#"{"cmd":"move","motion":"lineEnd"}"#);
        r["caret"]["x"].as_f64().unwrap()
    };
    let result = end(&mut e);
    let r = cmd(&mut e, r#"{"cmd":"toggleFieldCodes"}"#);
    assert_eq!(r["count"], 1);
    let code = end(&mut e);
    // "{ = 2+3 + 0 }" is much wider than "5": the caret follows the whole code.
    assert!(code > result + 40.0, "{result} → {code}");
    // Text after the field: the caret just after the field is at the same place.
    cmd(&mut e, r#"{"cmd":"insertText","text":" xy"}"#);
    // The field is one object (U+FFFC, 3 bytes of the index text) at 3..6.
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":6},"focus":{"p":0,"o":6}}}"#,
    );
    let after = r["caret"]["x"].as_f64().unwrap();
    assert!((after - code).abs() < 0.5, "{after} vs {code}");
    // The caret before the field stays before its opening brace.
    let r = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":3},"focus":{"p":0,"o":3}}}"#,
    );
    assert!(r["caret"]["x"].as_f64().unwrap() < result - 2.0);
}

#[test]
fn f9_shift_f9_and_ctrl_f9_in_place() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Lan"}"#);
    cmd(&mut e, r#"{"cmd":"insertField","instr":"USERNAME"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" và "}"#);
    cmd(&mut e, r#"{"cmd":"insertField","instr":"USERINITIALS"}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Lan và L");
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"Minh Anh"}"#);
    // F9 with the caret just after the second field: only it changes.
    let r = cmd(&mut e, r#"{"cmd":"updateFields","selection":true}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(e.document().unwrap().plain_text(), "Lan và MA");
    // Everything selected: both.
    cmd(&mut e, r#"{"cmd":"selectAll"}"#);
    cmd(&mut e, r#"{"cmd":"updateFields","selection":true}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Minh Anh và MA");
    // Shift+F9 at the first field: its code on the page, the other field's result stays.
    let r = caret_at(&mut e, 0, 0);
    assert_eq!(r["field"]["code"], "USERNAME");
    assert_eq!(r["field"]["codes"], false);
    let r = cmd(&mut e, r#"{"cmd":"toggleFieldCodes"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(r["field"]["codes"], true);
    let page = e.page_json(0).unwrap();
    assert!(page.contains("{ USERNAME }"));
    assert!(!page.contains("{ USERINITIALS }"));
    cmd(&mut e, r#"{"cmd":"toggleFieldCodes"}"#);
    assert!(!e.page_json(0).unwrap().contains("{ USERNAME }"));
    // Ctrl+F9 at the end: empty braces, selected, showing "{  }".
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" = "}"#);
    let r = cmd(&mut e, r#"{"cmd":"insertEmptyField"}"#);
    assert_eq!(r["field"]["code"], "");
    assert_eq!(r["field"]["codes"], true);
    assert_eq!(r["rects"].as_array().unwrap().len(), 1);
    assert!(e.page_json(0).unwrap().contains("{  }"));
    let r = cmd(&mut e, r#"{"cmd":"setFieldCode","instr":"= 6*7"}"#);
    assert_eq!(r["count"], 1);
    assert!(e.page_json(0).unwrap().contains("{ = 6*7 }"));
    // F9: its result instead of its code.
    cmd(&mut e, r#"{"cmd":"updateFields","selection":true}"#);
    assert_eq!(e.document().unwrap().plain_text(), "Minh Anh và MA = 42");
    assert!(!e.page_json(0).unwrap().contains("{ = 6*7 }"));
    // Saved as an ordinary field.
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(f.document().unwrap().plain_text(), "Minh Anh và MA = 42");
}

// ECMA-376 Part 1, §17.16.5.4 ASK / §17.16.5.20 FILLIN in a header: the question is asked for the
// header's own fields, its answer shows there.
#[test]
fn fillin_in_a_header_asks_and_updates() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Thân"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"insertField","instr":"FILLIN \"Đơn vị?\" \\d \"Phòng A\""}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"fieldPrompts","selection":true}"#);
    assert_eq!(r["fieldPrompts"][0]["prompt"], "Đơn vị?");
    assert_eq!(r["fieldPrompts"][0]["default"], "Phòng A");
    let r = cmd(
        &mut e,
        r#"{"cmd":"updateFields","selection":true,"answers":{"FILLIN:Đơn vị?":"Phòng Kế toán"}}"#,
    );
    assert!(r.get("error").is_none());
    assert!(r.get("story").is_some(), "still editing the header");
    cmd(&mut e, r#"{"cmd":"closeHeaderFooter"}"#);
    let saved = e.save_docx().unwrap();
    let pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let header = pkg
        .parts()
        .iter()
        .find(|p| p.name.starts_with("/word/header"))
        .map(|p| String::from_utf8(p.data.clone()).unwrap())
        .unwrap();
    assert!(header.contains("Phòng Kế toán"));
}

// Content control properties over the bridge: show as, colour, temporary, list items, date
// format and language, a building block gallery and a picture placeholder — written as
// w15:appearance / w15:color / w:temporary / w:lid / w:docPartList and read back the same.
#[test]
fn control_properties_gallery_and_picture_round_trip() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Tỉnh: "}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"dropDown","placeholder":"Chọn một mục.","items":["Huế"]}"#,
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"setControlProps","props":{"title":"Tỉnh","tag":"","deleteLocked":false,"contentLocked":false,"appearance":"tags","color":"C00000","temporary":true,"items":[{"displayText":"Hà Nội","value":"HN"},{"displayText":"Huế","value":"Huế"}]}}"#,
    );
    assert_eq!(r["count"], 1);
    assert_eq!(r["control"]["appearance"], "tags");
    assert_eq!(r["control"]["color"], "C00000");
    assert_eq!(r["control"]["temporary"], true);
    assert_eq!(r["control"]["items"], serde_json::json!(["Hà Nội", "Huế"]));
    assert_eq!(r["control"]["values"], serde_json::json!(["HN", "Huế"]));
    cmd(&mut e, r#"{"cmd":"move","motion":"lineEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":" Ngày: "}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"date","placeholder":"Chọn ngày."}"#,
    );
    cmd(&mut e, r#"{"cmd":"setControlValue","value":"2026-11-12"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"setControlProps","props":{"title":"","tag":"","deleteLocked":false,"contentLocked":false,"dateFormat":"MMMM d, yyyy","dateLid":"en-US"}}"#,
    );
    assert_eq!(r["control"]["lid"], "en-US");
    assert!(e
        .document()
        .unwrap()
        .plain_text()
        .ends_with("November 12, 2026"));
    // Each next control at the start of a paragraph of its own.
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"A"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineStart"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"buildingBlockGallery","placeholder":"Chọn một khối dựng sẵn."}"#,
    );
    assert_eq!(r["control"]["kind"], "buildingBlockGallery");
    assert_eq!(r["control"]["gallery"], "Quick Parts");
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"B"}"#);
    cmd(&mut e, r#"{"cmd":"move","motion":"lineStart"}"#);
    let r = cmd(&mut e, r#"{"cmd":"insertControl","kind":"picture"}"#);
    assert_eq!(r["control"]["kind"], "picture");
    assert_eq!(r["control"]["placeholder"], true);
    assert_eq!(e.document().unwrap().media.len(), 1);
    // One undo removes the picture control and its picture.
    let r = cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert!(r.get("control").is_none(), "{}", r["control"]);
    assert_eq!(e.document().unwrap().media.len(), 0);
    cmd(&mut e, r#"{"cmd":"redo"}"#);
    let saved = e.save_docx().unwrap();
    let pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert!(xml.contains("<w:temporary/>"));
    assert!(xml.contains("w15:appearance") && xml.contains("w15:val=\"tags\""));
    assert!(xml.contains("<w15:color"));
    assert!(xml.contains("<w:listItem w:displayText=\"Hà Nội\" w:value=\"HN\"/>"));
    assert!(xml.contains("<w:lid w:val=\"en-US\"/>"));
    assert!(
        xml.contains("<w:docPartList><w:docPartGallery w:val=\"Quick Parts\"/></w:docPartList>")
    );
    assert!(xml.contains("<w:picture/>") && xml.contains("<w:drawing>"));
    // Read back with the same properties.
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = caret_at(&mut f, 0, "Tỉnh: ".len() + 1);
    assert_eq!(r["control"]["appearance"], "tags");
    assert_eq!(r["control"]["color"], "C00000");
    assert_eq!(r["control"]["temporary"], true);
    assert_eq!(r["control"]["values"], serde_json::json!(["HN", "Huế"]));
    // Saved again unchanged: byte-identical document part.
    let again = f.save_docx().unwrap();
    let pkg2 = bo_opc::Package::read(&again, &bo_opc::Limits::default()).unwrap();
    let xml2 = String::from_utf8(pkg2.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert_eq!(xml, xml2);
}

// A repeating section item is deleted (not the last one), one undo step.
#[test]
fn delete_repeating_item_over_the_bridge() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Mục"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"insertControl","kind":"repeatingSection","placeholder":"DS"}"#,
    );
    let r = cmd(&mut e, r#"{"cmd":"deleteRepeatingItem"}"#);
    assert_eq!(r["count"], 0);
    cmd(&mut e, r#"{"cmd":"addRepeatingItem"}"#);
    assert_eq!(e.document().unwrap().plain_text().matches("Mục").count(), 2);
    let r = cmd(&mut e, r#"{"cmd":"deleteRepeatingItem"}"#);
    assert_eq!(r["count"], 1);
    assert_eq!(e.document().unwrap().plain_text().matches("Mục").count(), 1);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(e.document().unwrap().plain_text().matches("Mục").count(), 2);
}

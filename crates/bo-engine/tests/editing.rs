//! Editing through the JSON command bridge (the same path the web and desktop UIs use).

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

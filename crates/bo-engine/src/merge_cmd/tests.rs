//! DOC-MERGE-001 engine commands. Expected names, counts and texts are derived by
//! hand from the data below.
#![allow(clippy::unwrap_used)]

use super::*;
use crate::edit::Command;
use std::io::Read as _;

fn cmd(e: &mut Engine, json: &str) -> EditResult {
    let c: Command = serde_json::from_str(json).unwrap();
    e.command(c).unwrap()
}

/// "Kính gửi «Họ tên»" then (optionally) a page break and a second page.
fn engine(two_pages: bool) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    let text = if two_pages {
        "Kính gửi \nTrang hai"
    } else {
        "Kính gửi "
    };
    e.load(crate::plain_text_document(text), "thu-moi.docx")
        .unwrap();
    cmd(
        &mut e,
        r#"{"cmd":"move","motion":"docStart","extend":false}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"move","motion":"lineEnd","extend":false}"#,
    );
    cmd(&mut e, r#"{"cmd":"mergeInsertField","name":"Họ tên"}"#);
    if two_pages {
        cmd(&mut e, r#"{"cmd":"move","motion":"docEnd","extend":false}"#);
        cmd(
            &mut e,
            r#"{"cmd":"move","motion":"lineStart","extend":false}"#,
        );
        cmd(&mut e, r#"{"cmd":"insertPageBreak"}"#);
    }
    e
}

const TABLE: &str = r#"{"cmd":"mergeTable","name":"sql: khach_hang",
  "fields":["Họ tên","Email","TinhThanh"],
  "rows":[["Nguyễn Văn An","an@vd.vn","Hà Nội"],
          ["Trần Thị Bình","","Đà Nẵng"],
          ["Lê Văn Cường","cuong@vd.vn","Hà Nội"],
          ["Nguyễn Văn An","an2@vd.vn","Huế"]]}"#;

/// With `BO_MERGE_OUT=<dir>`, keep an output for `bo-cli validate --openxml require`.
fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_MERGE_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

fn unzip(b64: &str) -> Vec<(String, Vec<u8>)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut v = Vec::new();
            f.read_to_end(&mut v).unwrap();
            (f.name().unwrap().into_owned(), v)
        })
        .collect()
}

#[test]
fn file_names_from_patterns() {
    let d = crate::merge_sources::from_table(
        "t",
        &["Họ tên".into()],
        &[vec!["Nguyễn/Văn: An?".into()]],
    );
    assert_eq!(
        fill_pattern("Thư - «Họ tên»", &d, 0),
        "Thư - Nguyễn/Văn: An?"
    );
    assert_eq!(
        safe_file_name(" Thư - Nguyễn/Văn: An?. "),
        "Thư - Nguyễn_Văn_ An_"
    );
    let mut used = std::collections::HashSet::new();
    assert_eq!(unique_name("a", "pdf", 1, &mut used), "a.pdf");
    assert_eq!(unique_name("A", "pdf", 2, &mut used), "A (2).pdf");
    assert_eq!(unique_name("", "pdf", 3, &mut used), "3.pdf");
}

#[test]
fn table_provider_recipients_and_pdf_zip() {
    let mut e = engine(true);
    let r = cmd(&mut e, TABLE);
    let st = r.merge.unwrap();
    assert_eq!((st.count, st.active), (4, 4));
    // Hà Nội only, sorted by name descending: Nguyễn Văn An (row 0), Lê Văn Cường (row 2).
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeSetRecipients","options":{
            "filter":[{"field":"TinhThanh","op":"eq","value":"Hà Nội"}],
            "sort":[{"field":"Họ tên","descending":true}]}}"#,
    );
    assert_eq!(r.merge.unwrap().active, 2);
    let r = cmd(&mut e, r#"{"cmd":"mergeRecords"}"#);
    let rec = r.merge_out.unwrap().records.unwrap();
    assert_eq!(rec.total, 4);
    assert_eq!(rec.rows[0].position, Some(0));
    assert_eq!(rec.rows[1].position, None);
    assert_eq!(rec.rows[2].position, Some(1));
    // Every record again; duplicates by name: rows 0 and 3.
    cmd(&mut e, r#"{"cmd":"mergeSetRecipients","options":{}}"#);
    let r = cmd(&mut e, r#"{"cmd":"mergeDuplicates","fields":["Họ tên"]}"#);
    assert_eq!(r.merge_out.unwrap().duplicates.unwrap(), [vec![0, 3]]);
    // Exclude Bình: three PDFs of two pages each; the repeated name gets "(2)".
    cmd(
        &mut e,
        r#"{"cmd":"mergeSetRecipients","options":{"excluded":[1]}}"#,
    );
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeToPdfs","pattern":"Thư mời - «Họ tên»"}"#,
    );
    assert!(r.error.is_none(), "{:?}", r.error);
    let out = r.merge_out.unwrap();
    let files = out.files.unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Thư mời - Nguyễn Văn An.pdf",
            "Thư mời - Lê Văn Cường.pdf",
            "Thư mời - Nguyễn Văn An (2).pdf"
        ]
    );
    assert!(files.iter().all(|f| f.pages == 2));
    let zipped = unzip(&out.zip.unwrap());
    assert_eq!(zipped.len(), 3);
    assert_eq!(zipped[1].0, "Thư mời - Lê Văn Cường.pdf");
    assert!(zipped.iter().all(|(_, b)| b.starts_with(b"%PDF")));
    // The template is unchanged.
    assert!(e.document().unwrap().plain_text().starts_with("Kính gửi "));
}

#[test]
fn emails_for_the_mail_provider() {
    let mut e = engine(false);
    cmd(&mut e, TABLE);
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeToEmails","to":"Email","subject":"Thư mời gửi «Họ tên»","attach":"pdf","attachmentName":"Thư mời «Họ tên»"}"#,
    );
    let out = r.merge_out.unwrap();
    let mails = out.mails.unwrap();
    // Bình has no address.
    assert_eq!(out.skipped, Some(1));
    let to: Vec<&str> = mails.iter().map(|m| m.to.as_str()).collect();
    assert_eq!(to, ["an@vd.vn", "cuong@vd.vn", "an2@vd.vn"]);
    assert_eq!(mails[1].subject, "Thư mời gửi Lê Văn Cường");
    assert!(mails[1].html.contains("Kính gửi Lê Văn Cường"));
    let a = mails[2].attachment.as_ref().unwrap();
    assert_eq!(a.name, "Thư mời Nguyễn Văn An (2).pdf");
    assert_eq!(a.content_type, "application/pdf");
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeToEmails","to":"Email","subject":"S","attach":"docx"}"#,
    );
    let m = r.merge_out.unwrap().mails.unwrap();
    assert!(m[0].attachment.as_ref().unwrap().name.ends_with(".docx"));
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeToEmails","to":"Email","subject":"S"}"#,
    );
    assert!(r.merge_out.unwrap().mails.unwrap()[0].attachment.is_none());
}

#[test]
fn rules_prompts_and_merged_docx_round_trip() {
    let mut e = engine(false);
    cmd(&mut e, TABLE);
    cmd(
        &mut e,
        r#"{"cmd":"move","motion":"docStart","extend":false}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"mergeInsertRule","rule":{"kind":"skipIf","field":"Email","op":"=","value":""}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"mergeInsertRule","rule":{"kind":"if","field":"TinhThanh","op":"=","value":"Hà Nội","then":"[HN] ","else":""}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"mergeInsertRule","rule":{"kind":"fillIn","prompt":"Lời chào?","default":"Xin chào","once":true}}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":" "}"#);
    cmd(
        &mut e,
        r#"{"cmd":"mergeInsertRule","rule":{"kind":"mergeSeq"}}"#,
    );
    cmd(&mut e, r#"{"cmd":"insertText","text":". "}"#);
    let r = cmd(&mut e, r#"{"cmd":"mergePrompts"}"#);
    let p = r.merge_out.unwrap().prompts.unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].key, "FILLIN:Lời chào?");
    assert!(p[0].once);
    cmd(
        &mut e,
        r#"{"cmd":"mergeSetAnswers","answers":{"FILLIN:Lời chào?":["Thân gửi"]}}"#,
    );
    // The template round-trips through .docx with its rule fields.
    let tpl = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&tpl, "tpl.docx").unwrap();
    let r = cmd(&mut back, TABLE);
    assert!(r.error.is_none());
    cmd(
        &mut back,
        r#"{"cmd":"mergeSetAnswers","answers":{"FILLIN:Lời chào?":["Thân gửi"]}}"#,
    );
    for eng in [&mut e, &mut back] {
        let r = cmd(eng, r#"{"cmd":"mergeToDocument"}"#);
        assert!(r.error.is_none(), "{:?}", r.error);
        let text = eng.document().unwrap().plain_text();
        assert_eq!(
            text,
            "[HN] Thân gửi 1. Kính gửi Nguyễn Văn An\n[HN] Thân gửi 2. Kính gửi Lê Văn Cường\nThân gửi 3. Kính gửi Nguyễn Văn An"
        );
    }
    // The merged document is a valid package that reads back.
    let merged = e.save_docx().unwrap();
    keep("merge-rules-template.docx", &tpl);
    keep("merge-rules-merged.docx", &merged);
    let doc = bo_docx::read_docx(&merged).unwrap();
    assert_eq!(doc.sections.len(), 3);
}

#[test]
fn labels_and_envelopes_from_presets() {
    let mut e = engine(false);
    let r = cmd(&mut e, r#"{"cmd":"mergePresets"}"#);
    let presets = r.merge_out.unwrap().presets.unwrap();
    let sheet = presets.labels.iter().find(|l| l.id == "a4-2x7").unwrap();
    let json = format!(
        r#"{{"cmd":"mergeLabels","sheet":{},"text":"«Họ tên»\n«TinhThanh»","mode":"perRecord"}}"#,
        serde_json::to_string(sheet).unwrap()
    );
    cmd(&mut e, TABLE);
    let r = cmd(&mut e, &json);
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.info.name.starts_with("labels-a4-2x7"));
    // The data source stays: 4 records on one sheet of 14 labels.
    let r = cmd(&mut e, r#"{"cmd":"mergeToDocument"}"#);
    assert!(r.error.is_none());
    assert_eq!(e.document().unwrap().sections.len(), 1);
    let text = e.document().unwrap().plain_text();
    assert!(text.starts_with("Nguyễn Văn An Hà Nội\t\tTrần Thị Bình Đà Nẵng"));
    keep("merge-labels-merged.docx", &e.save_docx().unwrap());
    let env = presets.envelopes.iter().find(|s| s.id == "c5").unwrap();
    let json = format!(
        r#"{{"cmd":"mergeEnvelope","size":{},"delivery":"«Họ tên»","returnAddress":"BlueStar"}}"#,
        serde_json::to_string(env).unwrap()
    );
    let r = cmd(&mut e, &json);
    assert!(r.error.is_none());
    // C5 landscape: 229 × 162 mm = 12983 × 9184 twips.
    let page = &e.document().unwrap().sections[0].page;
    assert_eq!((page.width.0, page.height.0), (12_983, 9_184));
    keep("merge-envelope-template.docx", &e.save_docx().unwrap());
    let r = cmd(&mut e, r#"{"cmd":"mergeToPdfs","pattern":"«Họ tên»"}"#);
    let files = r.merge_out.unwrap().files.unwrap();
    assert_eq!(files.len(), 4);
    assert!(files.iter().all(|f| f.pages == 1));
    // Invalid geometry is refused without touching the document.
    let r = cmd(
        &mut e,
        r#"{"cmd":"mergeLabels","sheet":{"id":"x","pageWidth":100,"pageHeight":100,"rows":2,"cols":2,"top":0,"left":0,"labelWidth":60,"labelHeight":40,"hPitch":60,"vPitch":40},"text":"a","mode":"same"}"#,
    );
    assert!(r.error.is_some());
}

//! Write path round trips: open → edit → save → reopen → render the same where unchanged.
use super::*;
use crate::tests::build_pdf;
use bo_render::Command;

/// `n` pages of 200 × 100 pt, page `i` showing "Page i".
fn pages_pdf(n: usize) -> Vec<u8> {
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 200 100] \
             /Resources << /Font << /F1 3 0 R >> >> >>",
            kids.join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    for i in 0..n {
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /Contents {} 0 R >>",
            5 + 2 * i
        ));
        let body = format!(
            "BT /F1 12 Tf 20 50 Td (Page {}) Tj ET 0 0 1 rg 10 10 30 20 re f",
            i + 1
        );
        objs.push(format!(
            "<< /Length {} >>\nstream\n{body}\nendstream",
            body.len()
        ));
    }
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn text_of(doc: &mut PdfDocument, i: usize) -> String {
    let p = doc.render(i, RenderOptions::default()).expect("renders");
    TextLayer::new(p.chars).text()
}

fn texts(doc: &mut PdfDocument) -> Vec<String> {
    (0..doc.page_count()).map(|i| text_of(doc, i)).collect()
}

/// Display list without image ids (they carry the page index).
fn commands(doc: &mut PdfDocument, i: usize) -> Vec<Command> {
    doc.render(i, RenderOptions::default())
        .expect("renders")
        .list
        .commands
}

#[test]
fn delete_move_rotate_save_reopen() {
    let mut doc = open(pages_pdf(4));
    assert_eq!(texts(&mut doc), ["Page 1", "Page 2", "Page 3", "Page 4"]);
    let page3 = commands(&mut doc, 2);
    doc.delete_pages(&[1]).expect("deletes");
    assert_eq!(texts(&mut doc), ["Page 1", "Page 3", "Page 4"]);
    // Move the last page before the first.
    doc.move_pages(&[2], 0).expect("moves");
    assert_eq!(texts(&mut doc), ["Page 4", "Page 1", "Page 3"]);
    doc.rotate_pages(&[0], 1).expect("rotates");
    assert_eq!(
        doc.page_info(0).map(|p| (p.rotate, p.width)),
        Some((90, 100.0))
    );
    assert!(doc.modified() && doc.has_structural_edits());
    let saved = doc.save().expect("saves");
    assert!(!doc.has_structural_edits());
    // A full rewrite: one revision only.
    assert_eq!(saved.windows(5).filter(|w| w == b"%%EOF").count(), 1);
    let mut again = open(saved);
    assert_eq!(texts(&mut again), ["Page 4", "Page 1", "Page 3"]);
    assert_eq!(again.page_info(0).map(|p| p.rotate), Some(90));
    // Unchanged page draws exactly as before.
    assert_eq!(commands(&mut again, 2), page3);
    // Inherited MediaBox and Resources were copied onto the pages.
    assert_eq!(
        again.page_info(1).map(|p| (p.width, p.height)),
        Some((200.0, 100.0))
    );
}

#[test]
fn undo_and_redo_restore_the_page_tree() {
    let mut doc = open(pages_pdf(3));
    doc.delete_pages(&[0, 2]).expect("deletes");
    assert_eq!(texts(&mut doc), ["Page 2"]);
    assert!(doc.delete_pages(&[0]).is_err(), "the last page stays");
    doc.insert_blank_page(1, 300.0, 400.0).expect("inserts");
    assert_eq!(doc.page_count(), 2);
    assert_eq!(
        doc.page_info(1).map(|p| (p.width, p.height)),
        Some((300.0, 400.0))
    );
    assert!(doc.undo());
    assert_eq!(doc.page_count(), 1);
    assert!(doc.undo());
    assert_eq!(texts(&mut doc), ["Page 1", "Page 2", "Page 3"]);
    assert!(!doc.undo());
    assert!(doc.redo());
    assert_eq!(texts(&mut doc), ["Page 2"]);
    assert!(doc.redo());
    assert_eq!(doc.page_count(), 2);
    // A new edit clears the redo list.
    assert!(doc.undo());
    doc.rotate_pages(&[0], -1).expect("rotates");
    assert!(!doc.can_redo());
    assert_eq!(doc.page_info(0).map(|p| p.rotate), Some(270));
    let mut again = open(doc.save().expect("saves"));
    assert_eq!(texts(&mut again), ["Page 2"]);
}

#[test]
fn merge_extract_and_split() {
    let mut a = open(pages_pdf(2));
    let b = pages_pdf(3);
    // Pages 3 and 1 of the other file after page 1.
    assert_eq!(a.insert_pdf(1, &b, Some(&[2, 0])).expect("inserts"), 2);
    assert_eq!(texts(&mut a), ["Page 1", "Page 3", "Page 1", "Page 2"]);
    assert_eq!(a.append_pdf(&b).expect("appends"), 3);
    assert_eq!(a.page_count(), 7);
    let saved = a.save().expect("saves");
    let mut merged = open(saved);
    assert_eq!(
        texts(&mut merged),
        ["Page 1", "Page 3", "Page 1", "Page 2", "Page 1", "Page 2", "Page 3"]
    );
    let part = merged
        .extract_pages(&[6, 0], SaveOptions::default())
        .expect("extracts");
    let mut part = open(part);
    assert_eq!(texts(&mut part), ["Page 3", "Page 1"]);
    let parts = merged.split(3, SaveOptions::default()).expect("splits");
    assert_eq!(parts.len(), 3);
    let counts: Vec<usize> = parts.into_iter().map(|p| open(p).page_count()).collect();
    assert_eq!(counts, [3, 3, 1]);
    // The source document is not changed by extracting.
    assert_eq!(merged.page_count(), 7);
    assert!(!merged.modified());
}

#[test]
fn compression_and_object_streams() {
    let mut doc = open(pages_pdf(30));
    let before = commands(&mut doc, 5);
    let plain = doc.bytes().len();
    let small = doc
        .save_full(SaveOptions {
            object_streams: true,
            ..SaveOptions::default()
        })
        .expect("saves");
    assert!(small.len() < plain, "{} < {plain}", small.len());
    assert!(small.windows(6).any(|w| w == b"ObjStm"));
    let mut again = open(small);
    assert_eq!(again.page_count(), 30);
    assert_eq!(commands(&mut again, 5), before);
    assert_eq!(text_of(&mut again, 29), "Page 30");
}

fn encrypted(bytes: &[u8], aes: bool) -> Vec<u8> {
    encrypted_with(bytes, aes, "")
}

fn encrypted_with(bytes: &[u8], aes: bool, user: &str) -> Vec<u8> {
    use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
    use lopdf::{EncryptionState, EncryptionVersion, Permissions};
    let mut doc = lopdf::Document::load_mem(bytes).expect("loads");
    let id = Object::String(
        b"0123456789abcdef".to_vec(),
        lopdf::StringFormat::Hexadecimal,
    );
    doc.trailer.set("ID", vec![id.clone(), id]);
    let state = if aes {
        let mut filters: BTreeMap<Vec<u8>, std::sync::Arc<dyn CryptFilter>> = BTreeMap::new();
        filters.insert(b"StdCF".to_vec(), std::sync::Arc::new(Aes128CryptFilter));
        EncryptionState::try_from(EncryptionVersion::V4 {
            document: &doc,
            encrypt_metadata: true,
            crypt_filters: filters,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: "owner",
            user_password: user,
            permissions: Permissions::all(),
        })
    } else {
        EncryptionState::try_from(EncryptionVersion::V2 {
            document: &doc,
            owner_password: "owner",
            user_password: user,
            key_length: 128,
            permissions: Permissions::PRINTABLE | Permissions::COPYABLE,
        })
    }
    .expect("state");
    doc.encrypt(&state).expect("encrypts");
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("writes");
    out
}

use std::collections::BTreeMap;

#[test]
fn encrypted_files_are_written_back_encrypted() {
    for aes in [false, true] {
        let bytes = encrypted(&pages_pdf(3), aes);
        // The page text is not readable in the encrypted bytes.
        assert!(!bytes.windows(6).any(|w| w == b"Page 2"));
        let mut doc = open(bytes);
        assert!(doc.is_encrypted());
        assert_eq!(texts(&mut doc), ["Page 1", "Page 2", "Page 3"]);
        // Incremental save of a comment, encrypted with the same key.
        doc.add_annotation(&NewAnnot {
            kind: AnnotKind::Note,
            page: 0,
            rects: vec![[10.0, 10.0, 30.0, 30.0]],
            color: [255, 214, 0],
            author: "Tester".into(),
            contents: "Bí mật".into(),
        })
        .expect("comments");
        let inc = doc.save().expect("saves");
        assert!(!inc.windows(6).any(|w| w == b"Tester"));
        let again = open(inc.clone());
        assert_eq!(again.annotations()[0].contents, "Bí mật");
        // Page edits: full rewrite, still encrypted with the original handler.
        let mut doc = open(inc);
        doc.delete_pages(&[1]).expect("deletes");
        let full = doc.save().expect("saves");
        assert!(full.windows(8).any(|w| w == b"/Encrypt"));
        assert!(!full.windows(6).any(|w| w == b"Page 3"));
        let mut again = open(full);
        assert!(again.is_encrypted());
        assert_eq!(texts(&mut again), ["Page 1", "Page 3"]);
        assert_eq!(again.annotations().len(), 1);
    }
}

#[test]
fn comments_undo_and_hostile_edits() {
    let mut doc = open(pages_pdf(2));
    doc.add_annotation(&NewAnnot {
        kind: AnnotKind::Square,
        page: 1,
        rects: vec![[10.0, 10.0, 60.0, 40.0]],
        color: [229, 57, 53],
        author: String::new(),
        contents: String::new(),
    })
    .expect("adds");
    assert_eq!(doc.annotations().len(), 1);
    assert!(doc.undo());
    assert_eq!(doc.annotations().len(), 0);
    // The undone comment is not written.
    let mut again = open(doc.save().expect("saves"));
    assert_eq!(again.annotations().len(), 0);
    assert_eq!(texts(&mut again), ["Page 1", "Page 2"]);
    // Out-of-range indices are ignored or refused, never a panic.
    assert!(doc.rotate_pages(&[99], 1).is_err());
    doc.move_pages(&[99], 0).expect("nothing to move");
    assert!(doc.extract_pages(&[], SaveOptions::default()).is_err());
    assert!(doc.insert_pdf(0, b"not a pdf", None).is_err());
    let before = object_map_len(&doc);
    assert!(doc.insert_pdf(5, &pages_pdf(1), Some(&[3])).is_err());
    assert_eq!(object_map_len(&doc), before);
}

fn object_map_len(doc: &PdfDocument) -> usize {
    edit::object_map(doc).len()
}

#[test]
fn password_protected_files() {
    let bytes = encrypted_with(&pages_pdf(2), true, "secret");
    assert!(PdfDocument::needs_password(&bytes));
    assert!(!PdfDocument::needs_password(&pages_pdf(1)));
    assert!(PdfDocument::open(bytes.clone(), standard_fonts()).is_err());
    assert!(PdfDocument::open_with_password(bytes.clone(), standard_fonts(), "wrong").is_err());
    let mut doc =
        PdfDocument::open_with_password(bytes, standard_fonts(), "secret").expect("opens");
    assert_eq!(texts(&mut doc), ["Page 1", "Page 2"]);
    doc.rotate_pages(&[1], 2).expect("rotates");
    let full = doc.save().expect("saves");
    // Still protected by the same password.
    assert!(PdfDocument::needs_password(&full));
    let again =
        PdfDocument::open_with_password(full.clone(), standard_fonts(), "secret").expect("opens");
    assert_eq!(again.page_info(1).map(|p| p.rotate), Some(180));
    // The owner password opens it too; a comment saved incrementally stays encrypted.
    let mut owner =
        PdfDocument::open_with_password(full, standard_fonts(), "owner").expect("opens");
    owner
        .add_annotation(&NewAnnot {
            kind: AnnotKind::Note,
            page: 0,
            rects: vec![[10.0, 10.0, 30.0, 30.0]],
            color: [255, 214, 0],
            author: "Tester".into(),
            contents: "x".into(),
        })
        .expect("comments");
    let inc = owner.save().expect("saves");
    assert!(PdfDocument::needs_password(&inc));
    let again = PdfDocument::open_with_password(inc, standard_fonts(), "secret").expect("opens");
    assert_eq!(again.annotations().len(), 1);
    assert_eq!(text_of(&mut open_pw(&again), 0), "Page 1");
}

fn open_pw(d: &PdfDocument) -> PdfDocument {
    PdfDocument::open_with_password(d.bytes().to_vec(), standard_fonts(), "secret").expect("opens")
}

#[test]
fn bluedoc_export_rewrite_renders_the_same() {
    let mut engine = bo_engine::Engine::with_bundled_fonts();
    let docx = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/phase1-sample.docx"
    ))
    .expect("corpus");
    engine
        .open(&docx, "phase1-sample.docx")
        .expect("opens docx");
    let pdf = engine.export_pdf().expect("exports");
    let mut doc = open(pdf);
    let n = doc.page_count();
    let before: Vec<Vec<Command>> = (0..n).map(|i| commands(&mut doc, i)).collect();
    for opts in [
        SaveOptions::default(),
        SaveOptions {
            object_streams: true,
            ..SaveOptions::default()
        },
        SaveOptions {
            compress: false,
            prune: false,
            object_streams: false,
        },
    ] {
        let mut d = open(doc.bytes().to_vec());
        let bytes = d.save_full(opts).expect("saves");
        let mut again = open(bytes);
        assert_eq!(again.page_count(), n);
        for (i, b) in before.iter().enumerate() {
            assert_eq!(&commands(&mut again, i), b, "page {i} with {opts:?}");
        }
    }
    // Moving the last page to the front n times gives the original order.
    for _ in 0..n {
        doc.move_pages(&[n - 1], 0).expect("moves");
    }
    let mut again = open(doc.save().expect("saves"));
    for (i, b) in before.iter().enumerate() {
        assert_eq!(&commands(&mut again, i), b, "page {i} after moves");
    }
}

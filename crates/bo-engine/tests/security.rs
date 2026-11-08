//! Opening protected and legacy documents: password (MS-OFFCRYPTO), macros
//! (MS-OVBA, kept but never run), .odt and SmartArt drawing fallbacks.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;

fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn sample_docx(text: &str) -> Vec<u8> {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"insertText","text":{}}}"#,
            serde_json::json!(text)
        ),
    );
    e.save_docx().unwrap()
}

const CFB: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

#[test]
fn password_protected_open_and_save() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&sample_docx("Bí mật"), "a.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"setSavePassword","password":"mk-123"}"#);
    assert_eq!(r["info"]["protected"], true);
    let enc = e.save_docx().unwrap();
    assert!(enc.starts_with(CFB));
    let mut f = Engine::with_bundled_fonts();
    let err = f.open(&enc, "a.docx").unwrap_err().to_string();
    assert_eq!(err, "password required");
    let err = f
        .open_with_password(&enc, "a.docx", Some("sai"))
        .unwrap_err()
        .to_string();
    assert_eq!(err, "wrong password");
    let info = f
        .open_with_password(&enc, "a.docx", Some("mk-123"))
        .unwrap();
    assert!(info.protected);
    assert_eq!(f.document().unwrap().plain_text(), "Bí mật");
    // Saving keeps the password; removing it saves a plain package.
    assert!(f.save_docx().unwrap().starts_with(CFB));
    cmd(&mut f, r#"{"cmd":"setSavePassword"}"#);
    assert!(f.save_docx().unwrap().starts_with(b"PK"));
}

#[test]
fn macros_are_reported_and_kept() {
    let docx = sample_docx("Có macro");
    let mut pkg = bo_opc::Package::read(&docx, &bo_opc::Limits::default()).unwrap();
    // [MS-OVBA] project storage inside an OOXML package (opaque here, never run).
    pkg.put_part(
        "/word/vbaProject.bin",
        vec![0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3],
    );
    let docm = pkg.write().unwrap();
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&docm, "macro.docm").unwrap();
    assert!(info.macros);
    cmd(&mut e, r#"{"cmd":"insertText","text":" và sửa"}"#);
    let saved = e.save_docx().unwrap();
    let back = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    assert_eq!(
        back.part("/word/vbaProject.bin").map(|p| p.data.clone()),
        Some(vec![0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3])
    );
    let mut f = Engine::with_bundled_fonts();
    assert!(f.open(&saved, "macro.docm").unwrap().macros);
    assert!(!f.open(&docx, "plain.docx").unwrap().macros);
}

fn corpus(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/doc")
            .join(name),
    )
    .unwrap()
}

#[test]
fn smartart_shows_its_drawing_fallback() {
    // mc:Fallback picture (ECMA-376 Part 3, §10.2.1).
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("smartart-01-fallback-picture.docx"), "a.docx")
        .unwrap();
    assert_eq!(
        e.page_json(0).unwrap().matches(r#""op":"image""#).count(),
        2
    );
    // Diagram drawing part ([MS-ODRAWXML] dsp:drawing) rendered as a picture.
    let src = corpus("smartart-02-process.docx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "b.docx").unwrap();
    let media = e.media("/word/diagrams/drawing1.xml").unwrap();
    assert!(media.starts_with(b"\x89PNG"));
    assert!(e
        .page_json(0)
        .unwrap()
        .contains("/word/diagrams/drawing1.xml"));
    assert!(!cmd(&mut e, r#"{"cmd":"state"}"#)["modified"]
        .as_bool()
        .unwrap());
    // Saving keeps the SmartArt parts verbatim (the rendering only lives in memory).
    cmd(&mut e, r#"{"cmd":"insertText","text":"Sửa "}"#);
    let saved = e.save_docx().unwrap();
    let a = bo_opc::Package::read(&src, &bo_opc::Limits::default()).unwrap();
    let b = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    for part in ["/word/diagrams/drawing1.xml", "/word/diagrams/data1.xml"] {
        assert_eq!(
            a.part(part).map(|p| p.data.clone()),
            b.part(part).map(|p| p.data.clone()),
            "{part}"
        );
    }
    let xml = String::from_utf8(b.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert!(xml.contains("dgm:relIds"));
}

#[test]
fn opens_odt_as_a_document() {
    let doc = bo_engine::plain_text_document("Xin chào ODF\nDòng hai");
    let odt = bo_odf::write_odt(&doc).unwrap();
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&odt, "van-ban.odt").unwrap();
    assert_eq!(info.name, "van-ban.docx");
    assert_eq!(e.document().unwrap().plain_text(), "Xin chào ODF\nDòng hai");
}

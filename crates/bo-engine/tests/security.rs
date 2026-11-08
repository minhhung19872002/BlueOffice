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

/// The same warning bar for BlueSheet (.xlsm) and BlueSlide (.pptm): the
/// [MS-OVBA] project part is reported on open and a plain file is not.
#[test]
fn sheet_and_slide_macros_are_reported() {
    let with_vba = |bytes: &[u8], part: &str| {
        let mut pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
        pkg.put_part(part, vec![0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3]);
        pkg.write().unwrap()
    };
    let mut s = bo_engine::SheetEngine::new();
    s.new_workbook("a.xlsx").unwrap();
    let xlsx = s.save("xlsx", 0).unwrap();
    let xlsm = with_vba(&xlsx, "/xl/vbaProject.bin");
    assert!(s.open(&xlsm, "macro.xlsm").unwrap().macros);
    assert!(!s.open(&xlsx, "plain.xlsx").unwrap().macros);
    // A new workbook clears the flag.
    s.open(&xlsm, "macro.xlsm").unwrap();
    assert!(!s.new_workbook("b.xlsx").unwrap().macros);

    let pptx = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-text-bullets.pptx"),
    )
    .unwrap();
    let pptm = with_vba(&pptx, "/ppt/vbaProject.bin");
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&pptm, "macro.pptm").unwrap();
    assert_eq!(info.kind, "slide");
    assert!(info.macros);
    assert!(!e.open(&pptx, "plain.pptx").unwrap().macros);
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

/// SmartArt drawing fallback ([MS-ODRAWXML] `dsp:drawing`) in BlueSlide and
/// BlueSheet: the pre-drawn shapes and their text (one glyph run per word)
/// show up in the display list.
#[test]
fn smartart_fallback_in_slides_and_sheets() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let mut e = Engine::with_bundled_fonts();
    for (file, words) in [
        (
            "slide-smartart-process.pptx",
            ["\"Tiếp \"", "\"Thẩm \"", "\"quả\""],
        ),
        (
            "slide-smartart-cycle.pptx",
            ["\"Lập \"", "\"Thực \"", "\"Cải \""],
        ),
    ] {
        let bytes = std::fs::read(root.join("slide").join(file)).unwrap();
        e.open(&bytes, file).unwrap();
        let page = e.page_json(0).unwrap();
        for w in words {
            assert!(page.contains(w), "{file}: {w}");
        }
        // Saving keeps the diagram parts verbatim.
        let saved = e.save_pptx().unwrap();
        let a = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let b = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
        for part in ["/ppt/diagrams/drawing1.xml", "/ppt/diagrams/data1.xml"] {
            assert_eq!(
                a.part(part).map(|p| p.data.clone()),
                b.part(part).map(|p| p.data.clone()),
                "{file} {part}"
            );
        }
    }

    let mut s = bo_engine::SheetEngine::with_bundled_fonts();
    let bytes = std::fs::read(root.join("sheet/smartart-3.xlsx")).unwrap();
    s.open(&bytes, "smartart-3.xlsx").unwrap();
    let view = bo_engine::sheet::ViewRequest {
        sheet: 0,
        scroll_x: 0.0,
        scroll_y: 0.0,
        width: 800.0,
        height: 400.0,
    };
    let json = s.render_json(&view).unwrap();
    for w in ["\"Lập \"", "\"duyệt\"", "\"Giải \"", "\"Quyết \""] {
        assert!(json.contains(w), "sheet: {w}");
    }
    // Scrolled away: nothing of the diagram is drawn.
    let far = bo_engine::sheet::ViewRequest {
        scroll_y: 5000.0,
        ..view
    };
    assert!(!s.render_json(&far).unwrap().contains("\"duyệt\""));
}

/// TCVN3 / VNI text in a self-generated `.docx`: converted to Unicode on open
/// with a Unicode font, reported in `DocInfo`, and the first undo restores it.
#[test]
fn legacy_vietnamese_docx_is_converted_and_undoable() {
    let tcvn = "Céng hoµ x· héi chñ nghÜa ViÖt Nam";
    let vni = "Ñoäc laäp - Töï do - Haïnh phuùc";
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"insertText","text":{}}}"#,
            serde_json::json!(tcvn)
        ),
    );
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"insertText","text":{}}}"#,
            serde_json::json!(vni)
        ),
    );
    let font = |e: &mut Engine, p: usize, len: usize, name: &str| {
        cmd(
            e,
            &format!(
                r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":0}},"focus":{{"p":{p},"o":{len}}}}}}}"#
            ),
        );
        cmd(
            e,
            &format!(r#"{{"cmd":"formatText","format":{{"kind":"font","name":"{name}"}}}}"#),
        );
    };
    font(&mut e, 0, tcvn.len(), ".VnTime");
    font(&mut e, 1, vni.len(), "VNI-Times");
    let legacy = e.save_docx().unwrap();
    assert_eq!(
        e.open(&sample_docx("Không có phông cũ"), "plain.docx")
            .unwrap()
            .legacy_converted,
        0
    );

    let mut f = Engine::with_bundled_fonts();
    let info = f.open(&legacy, "cu.docx").unwrap();
    assert_eq!(info.legacy_converted, 2);
    let text = f.document().unwrap().plain_text();
    assert!(
        text.contains("Cộng hoà xã hội chủ nghĩa Việt Nam"),
        "{text}"
    );
    assert!(text.contains("Độc lập - Tự do - Hạnh phúc"), "{text}");
    let bo_model::Block::Paragraph(p) = &f.document().unwrap().sections[0].blocks[0] else {
        panic!("paragraph")
    };
    assert_eq!(p.runs[0].props.font.as_deref(), Some(bo_encoding_font()));
    // Saved as Unicode; the first undo gives the legacy text back.
    let saved = f.save_docx().unwrap();
    let mut g = Engine::with_bundled_fonts();
    assert_eq!(g.open(&saved, "moi.docx").unwrap().legacy_converted, 0);
    cmd(&mut f, r#"{"cmd":"undo"}"#);
    let back = f.document().unwrap().plain_text();
    assert!(back.contains(tcvn) && back.contains(vni), "{back}");
}

fn bo_encoding_font() -> &'static str {
    "Times New Roman"
}

/// The same for BlueSheet: cells formatted with a `.VnTime` / `VNI-*` font.
#[test]
fn legacy_vietnamese_xlsx_is_converted_and_undoable() {
    use bo_model::sheet::{CellValue, CellXf, Font, Workbook};
    let mut wb = Workbook::new();
    let tcvn_font = wb.styles.add_font(Font {
        name: Some(".VnTime".into()),
        size: Some(12.0),
        ..Font::default()
    });
    let vni_font = wb.styles.add_font(Font {
        name: Some("VNI-Times".into()),
        size: Some(12.0),
        ..Font::default()
    });
    let x1 = wb.styles.add_xf(CellXf {
        font_id: tcvn_font,
        ..CellXf::default()
    });
    let x2 = wb.styles.add_xf(CellXf {
        font_id: vni_font,
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_a1("A1", CellValue::Text("Hµ Néi".into()));
    ws.set_a1("A2", CellValue::Text("Thaønh phoá Hoà Chí Minh".into()));
    ws.set_a1("A3", CellValue::Text("Plain ASCII".into()));
    ws.cells.get_mut(&(0, 0)).unwrap().style = x1;
    ws.cells.get_mut(&(1, 0)).unwrap().style = x2;
    let bytes = bo_xlsx::write_xlsx(&wb).unwrap();

    let mut s = bo_engine::SheetEngine::with_bundled_fonts();
    let info = s.open(&bytes, "cu.xlsx").unwrap();
    assert_eq!(info.legacy_converted, 2);
    let input = |s: &mut bo_engine::SheetEngine, r: u32| {
        serde_json::to_value(s.cell(0, r, 0).unwrap()).unwrap()["input"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(input(&mut s, 0), "Hà Nội");
    assert_eq!(input(&mut s, 1), "Thành phố Hồ Chí Minh");
    assert_eq!(input(&mut s, 2), "Plain ASCII");
    let r = s
        .exec_json(r#"{"cmd":"formatState","sheet":0,"row":0,"col":0}"#)
        .unwrap();
    assert!(r.contains("Times New Roman"), "{r}");
    assert!(s.undo().unwrap());
    assert_eq!(input(&mut s, 0), "Hµ Néi");
    // Nothing left to convert after the explicit command on Unicode text.
    assert!(s.redo().unwrap());
    let again = s.exec_json(r#"{"cmd":"convertLegacyText"}"#).unwrap();
    assert!(again.contains(r#""converted":0"#), "{again}");
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

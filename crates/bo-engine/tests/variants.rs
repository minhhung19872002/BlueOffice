//! Templates, macro-enabled variants, slide shows and theme files (CORE-FMT-001):
//! detection by main part content type, round trips, save-as between variants with
//! the VBA project kept byte for byte or dropped, new document from a template, and
//! `.thmx` theme files applied to / saved from documents, workbooks and decks.
//!
//! With `BO_VARIANT_OUT=<dir>` the written files are kept there (for
//! OpenXmlValidator: `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::variant::{self, Variant};
use bo_engine::{Engine, SheetEngine};
use bo_opc::{Limits, Package, Relationship, TargetMode};
use bo_pptx::build::{build_pptx, DeckSpec, LayoutKind, ParaSpec, ShapeSpec, SlideSpec};
use serde_json::{json, Value};
use std::path::PathBuf;

const REL_VBA: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
const REL_VBA_DATA: &str = "http://schemas.microsoft.com/office/2006/relationships/wordVbaData";
const REL_UI: &str = "http://schemas.microsoft.com/office/2006/relationships/ui/extensibility";
/// Stand-in for a VBA project: the bytes only have to survive unchanged.
const VBA: &[u8] = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1 BlueOffice test VBA project \x00\x01\x02\xFF";
const VBA_DATA: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<wne:vbaSuppData xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"/>"#;
/// A ribbon customization whose button calls a macro ([MS-CUSTOMUI] `onAction`).
const CUSTOM_UI: &str = r#"<customUI xmlns="http://schemas.microsoft.com/office/2006/01/customui"><ribbon><tabs><tab id="bo" label="BO"><group id="g" label="G"><button id="b" label="Chạy" onAction="Hello"/></group></tab></tabs></ribbon></customUI>"#;

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_VARIANT_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn pkg(bytes: &[u8]) -> Package {
    Package::read(bytes, &Limits::default()).unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    pkg(bytes).part(name).map(|p| p.data.clone())
}

fn main_part(p: &Package) -> String {
    p.related_part("/", bo_opc::REL_OFFICE_DOCUMENT)
        .unwrap()
        .unwrap()
}

/// `bytes` with a VBA project next to the main part (plus `vbaData.xml` for Word),
/// a macro-calling ribbon customization, and the macro-enabled main content type.
fn with_macros(bytes: &[u8]) -> Vec<u8> {
    let mut p = pkg(bytes);
    let main = main_part(&p);
    let dir = main.rsplit_once('/').unwrap().0.to_owned();
    let word = dir == "/word";
    let mut ct = p.content_types().unwrap();
    let vba = format!("{dir}/vbaProject.bin");
    p.put_part(&vba, VBA.to_vec());
    ct.set_override(&vba, "application/vnd.ms-office.vbaProject");
    if word {
        p.put_part(&format!("{dir}/vbaData.xml"), VBA_DATA.as_bytes().to_vec());
        ct.set_override(
            &format!("{dir}/vbaData.xml"),
            "application/vnd.ms-word.vbaData+xml",
        );
        p.set_relationships(
            &vba,
            &[Relationship {
                id: "rId1".into(),
                rel_type: REL_VBA_DATA.into(),
                target: "vbaData.xml".into(),
                mode: TargetMode::Internal,
            }],
        );
    }
    p.put_part("/customUI/customUI.xml", CUSTOM_UI.as_bytes().to_vec());
    let mut root = p.relationships("/").unwrap();
    root.push(Relationship {
        id: "rIdUi".into(),
        rel_type: REL_UI.into(),
        target: "customUI/customUI.xml".into(),
        mode: TargetMode::Internal,
    });
    p.set_relationships("/", &root);
    let mut rels = p.relationships(&main).unwrap();
    rels.push(Relationship {
        id: "rIdVba".into(),
        rel_type: REL_VBA.into(),
        target: "vbaProject.bin".into(),
        mode: TargetMode::Internal,
    });
    p.set_relationships(&main, &rels);
    let enabled = Variant::from_content_type(ct.get(&main).unwrap())
        .unwrap()
        .with_macros(true);
    ct.set_override(&main, &enabled.content_type());
    p.set_content_types(&ct);
    p.write().unwrap()
}

/// The VBA project survived byte for byte (and `vbaData.xml` for Word).
fn assert_macros_kept(bytes: &[u8], dir: &str) {
    assert_eq!(part(bytes, &format!("{dir}/vbaProject.bin")).unwrap(), VBA);
    if dir == "/word" {
        assert_eq!(
            part(bytes, "/word/vbaData.xml").unwrap(),
            VBA_DATA.as_bytes()
        );
    }
    assert!(part(bytes, "/customUI/customUI.xml").is_some());
}

/// No macro part, no relationship or content type override pointing at one.
fn assert_macros_gone(bytes: &[u8]) {
    let p = pkg(bytes);
    for part in p.parts() {
        let n = part.name.to_ascii_lowercase();
        assert!(
            !n.ends_with("vbaproject.bin")
                && !n.ends_with("vbadata.xml")
                && !n.contains("customui"),
            "{n} left behind"
        );
        if n.ends_with(".rels") {
            let x = String::from_utf8_lossy(&part.data);
            assert!(!x.contains(REL_VBA) && !x.contains(REL_UI) && !x.contains("vbaData"));
        }
    }
    let ct = String::from_utf8(part_ct(bytes)).unwrap();
    assert!(!ct.contains("vbaProject") && !ct.contains("vbaData") && !ct.contains("macroEnabled"));
    assert!(!bo_binary::has_macros(bytes));
}

fn part_ct(bytes: &[u8]) -> Vec<u8> {
    part(bytes, "/[Content_Types].xml").unwrap()
}

fn doc_bytes() -> Vec<u8> {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    e.command_json(r#"{"cmd":"insertText","text":"Mẫu văn bản"}"#)
        .unwrap();
    e.save_docx().unwrap()
}

fn sheet_bytes() -> Vec<u8> {
    let mut e = SheetEngine::new();
    e.new_workbook("a.xlsx").unwrap();
    e.set_inputs(0, &[(0, 0, "Mẫu".into()), (1, 0, "=1+2".into())])
        .unwrap();
    e.save("xlsx", 0).unwrap()
}

fn deck_bytes() -> Vec<u8> {
    build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        title: None,
        slides: vec![SlideSpec {
            layout: LayoutKind::Title,
            shapes: vec![ShapeSpec {
                name: "Title".into(),
                x: 838_200,
                y: 365_125,
                cx: 10_515_600,
                cy: 1_325_563,
                placeholder: Some(bo_pptx::build::Placeholder::CenterTitle),
                text: vec![ParaSpec::plain("Trình chiếu")],
                ..ShapeSpec::default()
            }],
            ..SlideSpec::default()
        }],
    })
    .unwrap()
}

fn run(e: &mut Engine, cmd: &Value) -> Value {
    serde_json::from_str(&e.command_json(&cmd.to_string()).unwrap()).unwrap()
}

fn run_slide(e: &mut Engine, cmd: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&cmd.to_string()).unwrap()).unwrap()
}

fn exec(e: &mut SheetEngine, cmd: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&cmd.to_string()).unwrap()).unwrap()
}

#[test]
fn content_types_are_the_published_ones() {
    // ECMA-376 Part 1 §11.3.10, §12.3.23, §13.3.6 and [MS-OFFMACRO2].
    let expected = [
        (
            "docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        (
            "dotx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
        ),
        (
            "docm",
            "application/vnd.ms-word.document.macroEnabled.main+xml",
        ),
        (
            "dotm",
            "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
        ),
        (
            "xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        ),
        (
            "xltx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
        ),
        (
            "xlsm",
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        ),
        (
            "xltm",
            "application/vnd.ms-excel.template.macroEnabled.main+xml",
        ),
        (
            "pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        ),
        (
            "potx",
            "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
        ),
        (
            "ppsx",
            "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
        ),
        (
            "pptm",
            "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
        ),
        (
            "potm",
            "application/vnd.ms-powerpoint.template.macroEnabled.main+xml",
        ),
        (
            "ppsm",
            "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml",
        ),
    ];
    assert_eq!(expected.len(), Variant::ALL.len());
    for (ext, ct) in expected {
        assert_eq!(Variant::from_ext(ext).unwrap().content_type(), ct, "{ext}");
    }
}

/// Every variant of each family, written from one source with macros (for the validator).
#[test]
fn converts_between_every_variant() {
    for (app, base, dir) in [
        ("doc", doc_bytes(), "/word"),
        ("sheet", sheet_bytes(), "/xl"),
        ("slide", deck_bytes(), "/ppt"),
    ] {
        let src = with_macros(&base);
        for v in Variant::family(app) {
            let out = variant::convert(&src, v).unwrap();
            assert_eq!(variant::detect(&out.bytes), Some(v));
            assert_eq!(out.macros_dropped, !v.macros());
            if v.macros() {
                assert_macros_kept(&out.bytes, dir);
            } else {
                assert_macros_gone(&out.bytes);
            }
            keep(&format!("variant.{}", v.ext()), &out.bytes);
        }
        // Another document type is refused.
        let other = if app == "doc" {
            Variant::Xlsx
        } else {
            Variant::Docx
        };
        assert!(variant::convert(&src, other).is_err());
    }
}

#[test]
fn document_variants_round_trip_and_save_as() {
    let dotm = variant::convert(&with_macros(&doc_bytes()), Variant::Dotm)
        .unwrap()
        .bytes;
    let mut e = Engine::with_bundled_fonts();
    // The name says .docx: the main part's content type decides.
    let info = e.open(&dotm, "đặt tên sai.docx").unwrap();
    assert_eq!(info.variant.as_deref(), Some("dotm"));
    assert!(info.macros);
    // Saving again keeps the variant and the macros.
    let again = e.save_docx().unwrap();
    assert_eq!(variant::detect(&again), Some(Variant::Dotm));
    assert_macros_kept(&again, "/word");
    // Save as .docm: macros kept byte for byte.
    let r = run(&mut e, &json!({"cmd":"saveFormat","ext":"docm"}));
    assert_eq!(r["info"]["variant"], "docm");
    // The document name follows its type.
    assert_eq!(r["info"]["name"], "đặt tên sai.docm");
    let docm = e.save_docx().unwrap();
    assert_eq!(variant::detect(&docm), Some(Variant::Docm));
    assert_macros_kept(&docm, "/word");
    keep("saved.docm", &docm);
    // Save as .dotx: the macros, their data part and the ribbon callbacks go.
    run(&mut e, &json!({"cmd":"saveFormat","ext":"dotx"}));
    let dotx = e.save_docx().unwrap();
    assert_eq!(variant::detect(&dotx), Some(Variant::Dotx));
    assert_macros_gone(&dotx);
    keep("saved.dotx", &dotx);
    // Text survives every step.
    let mut back = Engine::with_bundled_fonts();
    back.open(&dotx, "x.dotx").unwrap();
    assert!(back
        .document()
        .unwrap()
        .plain_text()
        .contains("Mẫu văn bản"));
    // A workbook type is refused.
    let r = run(&mut e, &json!({"cmd":"saveFormat","ext":"xlsx"}));
    assert_eq!(r["error"], "saveFormat");
    assert_eq!(r["info"]["variant"], "dotx");
}

#[test]
fn new_document_from_template() {
    let dotx = variant::convert(&doc_bytes(), Variant::Dotx).unwrap().bytes;
    let doc = variant::new_from_template(&dotx).unwrap();
    assert_eq!(variant::detect(&doc), Some(Variant::Docx));
    // A macro-enabled template gives a macro-enabled document with the same project.
    let dotm = variant::convert(&with_macros(&doc_bytes()), Variant::Dotm)
        .unwrap()
        .bytes;
    let docm = variant::new_from_template(&dotm).unwrap();
    assert_eq!(variant::detect(&docm), Some(Variant::Docm));
    assert_macros_kept(&docm, "/word");
    // Same through the engine: open the template, save as the document type.
    let mut e = Engine::with_bundled_fonts();
    e.open(&dotx, "Mẫu.dotx").unwrap();
    run(&mut e, &json!({"cmd":"saveFormat","ext":"docx"}));
    assert_eq!(
        variant::detect(&e.save_docx().unwrap()),
        Some(Variant::Docx)
    );
    // Documents and other files are left alone.
    let docx = doc_bytes();
    assert_eq!(variant::new_from_template(&docx).unwrap(), docx);
    // Templates of every type.
    for (t, d) in [
        (Variant::Xltx, Variant::Xlsx),
        (Variant::Xltm, Variant::Xlsm),
        (Variant::Potx, Variant::Pptx),
        (Variant::Potm, Variant::Pptm),
    ] {
        let base = if t.app() == "sheet" {
            sheet_bytes()
        } else {
            deck_bytes()
        };
        let tpl = variant::convert(&base, t).unwrap().bytes;
        assert_eq!(
            variant::detect(&variant::new_from_template(&tpl).unwrap()),
            Some(d)
        );
    }
}

#[test]
fn workbook_variants_round_trip_and_save_as() {
    let xltm = variant::convert(&with_macros(&sheet_bytes()), Variant::Xltm)
        .unwrap()
        .bytes;
    let mut e = SheetEngine::new();
    let info = e.open(&xltm, "book.xlsx").unwrap();
    assert_eq!(info.variant.as_deref(), Some("xltm"));
    assert!(info.macros);
    let again = e.save("xlsx", 0).unwrap();
    assert_eq!(variant::detect(&again), Some(Variant::Xltm));
    assert_macros_kept(&again, "/xl");
    let r = exec(&mut e, &json!({"cmd":"saveFormat","ext":"xlsm"}));
    assert_eq!(
        (r["ok"].as_bool(), r["variant"].as_str()),
        (Some(true), Some("xlsm"))
    );
    let xlsm = e.save("xlsx", 0).unwrap();
    assert_eq!(variant::detect(&xlsm), Some(Variant::Xlsm));
    assert_macros_kept(&xlsm, "/xl");
    exec(&mut e, &json!({"cmd":"saveFormat","ext":"xlsx"}));
    let xlsx = e.save("xlsx", 0).unwrap();
    assert_eq!(variant::detect(&xlsx), Some(Variant::Xlsx));
    assert_macros_gone(&xlsx);
    keep("saved.xlsx", &xlsx);
    let r = exec(&mut e, &json!({"cmd":"saveFormat","ext":"pptx"}));
    assert_eq!(r["error"], "saveFormat");
    // The cells survive.
    let mut back = SheetEngine::new();
    back.open(&xlsx, "x.xlsx").unwrap();
    let ws = &back.workbook().unwrap().sheets[0];
    assert_eq!(ws.cells.len(), 2);
}

#[test]
fn slide_show_variants_round_trip_and_save_as() {
    let ppsm = variant::convert(&with_macros(&deck_bytes()), Variant::Ppsm)
        .unwrap()
        .bytes;
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&ppsm, "show.pptx").unwrap();
    assert_eq!(info.variant.as_deref(), Some("ppsm"));
    assert_eq!(info.kind, "slide");
    assert!(info.macros);
    let again = e.save_pptx().unwrap();
    assert_eq!(variant::detect(&again), Some(Variant::Ppsm));
    assert_macros_kept(&again, "/ppt");
    let r = run_slide(&mut e, &json!({"cmd":"saveFormat","ext":"potm"}));
    assert_eq!(r["info"]["variant"], "potm");
    let potm = e.save_pptx().unwrap();
    assert_eq!(variant::detect(&potm), Some(Variant::Potm));
    assert_macros_kept(&potm, "/ppt");
    run_slide(&mut e, &json!({"cmd":"saveFormat","ext":"ppsx"}));
    let ppsx = e.save_pptx().unwrap();
    assert_eq!(variant::detect(&ppsx), Some(Variant::Ppsx));
    assert_macros_gone(&ppsx);
    keep("saved.ppsx", &ppsx);
    let r = run_slide(&mut e, &json!({"cmd":"saveFormat","ext":"dotx"}));
    assert_eq!(r["error"], "saveFormat");
}

#[test]
fn encrypted_saves_convert_before_encrypting() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&with_macros(&doc_bytes()), "a.docm").unwrap();
    run(&mut e, &json!({"cmd":"saveFormat","ext":"docx"}));
    run(
        &mut e,
        &json!({"cmd":"setSavePassword","password":"mật khẩu"}),
    );
    let enc = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    let info = back
        .open_with_password(&enc, "a.docx", Some("mật khẩu"))
        .unwrap();
    assert_eq!(info.variant.as_deref(), Some("docx"));
    assert!(!info.macros);
}

/// The theme of a deck saved as `.thmx`, then applied to a document and a workbook.
#[test]
fn theme_files() {
    let mut deck = Engine::with_bundled_fonts();
    deck.open(&deck_bytes(), "deck.pptx").unwrap();
    // A built-in theme with recognisable fonts.
    let r = run_slide(
        &mut deck,
        &json!({"cmd":"applyTheme","builtin":"bamboo","all":true}),
    );
    assert!(r["error"].is_null(), "{r}");
    let name = r["design"]["theme"].clone();
    assert!(name.is_string());
    let r = run_slide(&mut deck, &json!({"cmd":"saveThemeFile"}));
    let b64 = r["design"]["themeFile"].as_str().unwrap().to_owned();
    let thmx = base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .unwrap();
    keep("deck.thmx", &thmx);
    // Theme manager main part → theme part (§20.1.6.11 → §14.2.7).
    let p = pkg(&thmx);
    let main = main_part(&p);
    assert_eq!(main, "/theme/theme/themeManager.xml");
    let ct = p.content_types().unwrap();
    assert_eq!(ct.get(&main), Some(variant::CT_THEME_MANAGER));
    assert_eq!(ct.get("/theme/theme/theme1.xml"), Some(variant::CT_THEME));
    let theme = variant::theme_of_thmx(&thmx).unwrap();
    let deck_theme = variant::theme_of_package(&deck.save_pptx().unwrap()).unwrap();
    assert_eq!(theme, deck_theme);
    let fonts = bo_engine::theme_file::theme_fonts(&theme).unwrap();
    assert_ne!(fonts.major_latin, "");

    // Applied to a document: theme part and theme fonts, one undo step.
    let mut doc = Engine::with_bundled_fonts();
    doc.open(&doc_bytes(), "a.docx").unwrap();
    let r = run(&mut doc, &json!({"cmd":"applyThemeFile","file":b64}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(doc.document().unwrap().theme_fonts, fonts);
    let saved = doc.save_docx().unwrap();
    assert_eq!(variant::theme_of_package(&saved).unwrap(), theme);
    keep("themed.docx", &saved);
    // Saved back as a theme file.
    let r = run(&mut doc, &json!({"cmd":"saveThemeFile"}));
    let again = base64::engine::general_purpose::STANDARD
        .decode(r["design"]["themeFile"].as_str().unwrap())
        .unwrap();
    assert_eq!(variant::theme_of_thmx(&again).unwrap(), theme);
    run(&mut doc, &json!({"cmd":"undo"}));
    assert_ne!(doc.document().unwrap().theme_fonts, fonts);
    // Not a theme.
    let junk = base64::engine::general_purpose::STANDARD.encode(b"not a theme");
    let r = run(&mut doc, &json!({"cmd":"applyThemeFile","file":junk}));
    assert_eq!(r["error"], "noTheme");

    // Applied to a new workbook (which has no theme part yet): the part is added.
    let mut wb = SheetEngine::new();
    wb.new_workbook("b.xlsx").unwrap();
    let r = exec(&mut wb, &json!({"cmd":"saveThemeFile"}));
    assert_eq!(r["error"], "noTheme");
    let r = exec(&mut wb, &json!({"cmd":"applyThemeFile","file":b64}));
    assert_eq!(r["ok"], true);
    let saved = wb.save("xlsx", 0).unwrap();
    assert_eq!(variant::theme_of_package(&saved).unwrap(), theme);
    keep("themed.xlsx", &saved);
    let r = exec(&mut wb, &json!({"cmd":"saveThemeFile"}));
    let again = base64::engine::general_purpose::STANDARD
        .decode(r["file"].as_str().unwrap())
        .unwrap();
    assert_eq!(variant::theme_of_thmx(&again).unwrap(), theme);
    // The colour scheme is the theme's: accent1 of the theme part.
    let colors = &wb.workbook().unwrap().theme.as_ref().unwrap().colors;
    let accent1 = &colors[4];
    let xml = String::from_utf8(theme.clone()).unwrap();
    assert!(xml.contains(&format!("val=\"{accent1}\"")), "{accent1}");
    // And to a deck straight from the theme file (themeManager package).
    let mut other = Engine::with_bundled_fonts();
    other.open(&deck_bytes(), "o.pptx").unwrap();
    let r = run_slide(
        &mut other,
        &json!({"cmd":"applyTheme","file":b64,"all":true}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["design"]["theme"], name);
}

//! CORE-FMT-003 through the engines: HTML / MHTML save and open (BlueDoc), HTML
//! tables and XML Spreadsheet 2003 (BlueSheet), Flat OPC for the three apps.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::edit::Command;
use bo_engine::slide_edit::SlideCommand;
use bo_engine::{Engine, SheetEngine};
use bo_model::sheet::CellValue;
use bo_opc::{Limits, Package};
use std::path::PathBuf;

fn corpus(rel: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// An XML part without its declaration and surrounding white space.
fn xml_body(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    let s = s.trim_start_matches('\u{feff}').trim();
    match s.strip_prefix("<?xml") {
        Some(rest) => rest[rest.find("?>").unwrap() + 2..].trim().to_owned(),
        None => s.to_owned(),
    }
}

/// Flat OPC is lossless at part level: the same parts with the same content types;
/// binary parts byte for byte, XML parts up to the XML declaration.
fn assert_same_parts(zip: &[u8], flat: &[u8]) {
    assert!(bo_opc::flat::is_flat_opc(flat));
    let a = Package::read(zip, &Limits::default()).unwrap();
    let b = Package::read(flat, &Limits::default()).unwrap();
    let (cta, ctb) = (a.content_types().unwrap(), b.content_types().unwrap());
    let names = |p: &Package| {
        let mut v: Vec<String> = p
            .parts()
            .iter()
            .map(|x| x.name.clone())
            .filter(|n| n != "/[Content_Types].xml")
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&a), names(&b));
    for part in a
        .parts()
        .iter()
        .filter(|p| p.name != "/[Content_Types].xml")
    {
        let other = b.part(&part.name).unwrap();
        assert_eq!(cta.get(&part.name), ctb.get(&part.name), "{}", part.name);
        let ty = cta.get(&part.name).unwrap_or("");
        if ty.ends_with("xml") && xml_body(&part.data).starts_with('<') {
            assert_eq!(xml_body(&other.data), xml_body(&part.data), "{}", part.name);
        } else {
            assert_eq!(other.data, part.data, "{}", part.name);
        }
    }
}

#[test]
fn document_saves_and_opens_as_html_and_mhtml() {
    let docx = corpus("doc/phase1-sample.docx");
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&docx, "Mẫu.docx").unwrap();
    let text = e.document().unwrap().plain_text();
    let tables = |e: &Engine| {
        e.document().unwrap().sections[0]
            .blocks
            .iter()
            .filter(|b| matches!(b, bo_model::Block::Table(_)))
            .count()
    };
    for format in ["html", "mht"] {
        let bytes = e.export_as(format).unwrap();
        // Exporting changes nothing in the open document.
        assert_eq!(e.info().unwrap().page_count, info.page_count);
        let mut back = Engine::with_bundled_fonts();
        let opened = back.open(&bytes, &format!("Mẫu.{format}")).unwrap();
        assert_eq!(opened.name, "Mẫu.docx", "saved as a new .docx");
        assert!(opened.page_count >= 1);
        assert_eq!(
            words(&back.document().unwrap().plain_text()),
            words(&text),
            "{format}"
        );
        assert_eq!(tables(&back), tables(&e), "{format}");
        assert!(back.page_json(0).is_ok());
        // …and saves as a package again.
        let saved = back.save_docx().unwrap();
        assert_eq!(
            words(&bo_docx::read_docx(&saved).unwrap().plain_text()),
            words(&text)
        );
        if let Ok(dir) = std::env::var("BO_WEB_OUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                PathBuf::from(dir).join(format!("from-{format}.docx")),
                saved,
            )
            .unwrap();
        }
    }
    // Detected by content even under a text name.
    let html = e.export_as("html").unwrap();
    let mut by_content = Engine::with_bundled_fonts();
    by_content.open(&html, "page.txt").unwrap();
    assert_eq!(
        words(&by_content.document().unwrap().plain_text()),
        words(&text)
    );
}

#[test]
fn export_file_command_returns_base64_and_keeps_the_document() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("doc/phase1-sample.docx"), "a.docx").unwrap();
    let cmd = |json: &str| serde_json::from_str::<Command>(json).unwrap();
    let r = e
        .command(cmd(r#"{"cmd":"exportFile","format":"html"}"#))
        .unwrap();
    assert!(!r.modified);
    let html = base64::engine::general_purpose::STANDARD
        .decode(r.file.unwrap())
        .unwrap();
    assert!(html.starts_with(b"<!DOCTYPE html>"));
    let r = e
        .command(cmd(r#"{"cmd":"exportFile","format":"mht"}"#))
        .unwrap();
    let mht = base64::engine::general_purpose::STANDARD
        .decode(r.file.unwrap())
        .unwrap();
    assert!(bo_html::is_mht(&mht));
    let r = e
        .command(cmd(r#"{"cmd":"exportFile","format":"xml"}"#))
        .unwrap();
    let flat = base64::engine::general_purpose::STANDARD
        .decode(r.file.unwrap())
        .unwrap();
    assert_same_parts(&e.save_docx().unwrap(), &flat);
    let r = e
        .command(cmd(r#"{"cmd":"exportFile","format":"rtf"}"#))
        .unwrap();
    assert!(r.file.is_none() && r.error.is_some());
    // The serialised result carries the file under `file`.
    let json = serde_json::to_string(
        &e.command(cmd(r#"{"cmd":"exportFile","format":"html"}"#))
            .unwrap(),
    )
    .unwrap();
    assert!(json.contains("\"file\":\""));
}

#[test]
fn html_with_pictures_scripts_and_styles_opens_in_the_engine() {
    // A real PNG so the page renders.
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 8, 4);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[0x80; 8 * 4 * 3]).unwrap();
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    let html = format!(
        "<html><head><meta charset=\"utf-8\"><style>h1{{color:#C00000}}</style>\
         <script>alert('x')</script></head><body><h1>Tiêu đề</h1>\
         <p>Ảnh: <img src=\"data:image/png;base64,{b64}\" width=\"80\" height=\"40\" onerror=\"evil()\"></p>\
         <ul><li>một<li>hai</ul><table border=1><tr><td>a<td>b</table></body></html>"
    );
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(html.as_bytes(), "trang.htm").unwrap();
    assert_eq!(info.name, "trang.docx");
    let doc = e.document().unwrap();
    assert_eq!(doc.media.len(), 1);
    assert_eq!(
        doc.media.values().next().unwrap().as_slice(),
        png.as_slice()
    );
    assert!(!doc.plain_text().contains("alert"));
    assert!(e.page_json(0).unwrap().contains("Image") || e.page_json(0).unwrap().contains("image"));
    let saved = e.save_docx().unwrap();
    let pkg = Package::read(&saved, &Limits::default()).unwrap();
    assert!(pkg
        .parts()
        .iter()
        .any(|p| p.name.starts_with("/word/media/")));
    assert!(!pkg
        .parts()
        .iter()
        .any(|p| String::from_utf8_lossy(&p.data).contains("evil()")));
    if let Ok(dir) = std::env::var("BO_WEB_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join("from-foreign-html.docx"), saved).unwrap();
    }
}

#[test]
fn flat_opc_round_trips_for_documents_workbooks_and_presentations() {
    // BlueDoc.
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(&corpus("doc/phase1-sample.docx"), "a.docx").unwrap();
    let flat = e.export_as("xml").unwrap();
    assert_same_parts(&e.save_docx().unwrap(), &flat);
    let mut back = Engine::with_bundled_fonts();
    let opened = back.open(&flat, "a.xml").unwrap();
    assert_eq!(opened.name, "a.docx");
    assert_eq!(opened.page_count, info.page_count);
    assert_eq!(
        back.document().unwrap().plain_text(),
        e.document().unwrap().plain_text()
    );
    // Opening and exporting again gives the same file.
    assert_eq!(back.export_as("xml").unwrap(), flat);

    // BlueSlide.
    let deck_path = std::fs::read_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide"),
    )
    .unwrap()
    .filter_map(|e| e.ok().map(|e| e.path()))
    .filter(|p| p.extension().is_some_and(|x| x == "pptx"))
    .min()
    .unwrap();
    let pptx = std::fs::read(&deck_path).unwrap();
    let mut d = Engine::with_bundled_fonts();
    let info = d.open(&pptx, "deck.pptx").unwrap();
    let r = d
        .slide_command(
            serde_json::from_str::<SlideCommand>(r#"{"cmd":"exportFile","format":"xml"}"#).unwrap(),
        )
        .unwrap();
    let flat = base64::engine::general_purpose::STANDARD
        .decode(r.file.unwrap())
        .unwrap();
    assert_same_parts(&d.save_pptx().unwrap(), &flat);
    let mut back = Engine::with_bundled_fonts();
    let opened = back.open(&flat, "deck.xml").unwrap();
    assert_eq!(opened.name, "deck.pptx");
    assert_eq!(opened.page_count, info.page_count);
    assert_eq!(back.slide_count(), d.slide_count());
    // The original file as Flat OPC, without going through the writer.
    assert_same_parts(&pptx, &bo_opc::flat::zip_to_flat(&pptx).unwrap());

    // BlueSheet.
    let xlsx = corpus("sheet/cf-1.xlsx");
    let mut s = SheetEngine::new();
    s.open(&xlsx, "book.xlsx").unwrap();
    let flat = s.save("xml", 0).unwrap();
    assert_same_parts(&s.save("xlsx", 0).unwrap(), &flat);
    let mut back = SheetEngine::new();
    back.open(&flat, "book.xml").unwrap();
    let (a, b) = (s.workbook().unwrap(), back.workbook().unwrap());
    assert_eq!(a.sheets.len(), b.sheets.len());
    for (x, y) in a.sheets.iter().zip(&b.sheets) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cells.len(), y.cells.len());
        for (k, c) in &x.cells {
            assert_eq!(c.value, y.cells[k].value, "{} {k:?}", x.name);
        }
    }
    assert_same_parts(&xlsx, &bo_opc::flat::zip_to_flat(&xlsx).unwrap());
    // A password does not apply to Flat OPC: the file stays readable.
    s.set_save_password(Some("bí mật"));
    assert!(bo_opc::flat::is_flat_opc(&s.save("xml", 0).unwrap()));
}

#[test]
fn workbook_opens_excel_xml_2003_and_html_tables() {
    let mut s = SheetEngine::new();
    let info = s
        .open(&corpus("sheet/xml2003-1-budget.xml"), "budget.xml")
        .unwrap();
    assert_eq!(
        info.sheets
            .iter()
            .map(|x| x.name.as_str())
            .collect::<Vec<_>>(),
        ["Ngân sách", "Tham_so", "Ẩn"]
    );
    // Formulas converted from R1C1 calculate.
    let total = s.cell(0, 4, 1).unwrap();
    assert_eq!(total.input, "=SUM(B2:B4)");
    assert_eq!(
        s.workbook().unwrap().sheets[0].cell_a1("B5").unwrap().value,
        CellValue::Number(165_000_000.0)
    );
    assert_eq!(s.cell(0, 1, 3).unwrap().display, "98,75%");
    // The named range resolves across sheets: D8 = Thue_suat * B5.
    assert_eq!(
        s.workbook().unwrap().sheets[0].cell_a1("D8").unwrap().value,
        CellValue::Number(16_500_000.0)
    );
    let xlsx = s.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_WEB_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join("from-xml2003.xlsx"), &xlsx).unwrap();
    }

    // Save every sheet as HTML tables, open them again.
    let html = s.save("html", 0).unwrap();
    let text = String::from_utf8(html.clone()).unwrap();
    assert!(text.contains("<caption>Ngân sách</caption>"));
    assert!(text.contains(">98,75%</td>"), "{text}");
    let mut back = SheetEngine::new();
    let info = back.open(&html, "budget.html").unwrap();
    assert_eq!(info.sheets.len(), 3);
    let wb = back.workbook().unwrap();
    assert_eq!(
        wb.sheets[0].cell_a1("B2").unwrap().value,
        CellValue::Number(120_000_000.0)
    );
    assert_eq!(
        wb.sheets[0].cell_a1("A2").unwrap().value,
        CellValue::Text("Lương & thưởng".into())
    );
    assert_eq!(
        wb.sheets[0]
            .cell_a1("B5")
            .unwrap()
            .formula
            .as_ref()
            .unwrap()
            .text,
        "SUM(B2:B4)"
    );
    assert_eq!(wb.sheets[0].merges.len(), 1);
    let from_html = back.save("xlsx", 0).unwrap();
    if let Ok(dir) = std::env::var("BO_WEB_OUT") {
        std::fs::write(PathBuf::from(dir).join("from-html-tables.xlsx"), &from_html).unwrap();
    }
    // One sheet only.
    let one = s.save("html-sheet", 1).unwrap();
    let mut single = SheetEngine::new();
    assert_eq!(single.open(&one, "one.html").unwrap().sheets.len(), 1);
    assert!(s.save("html-sheet", 99).is_err());

    // A foreign page with a table.
    let mut f = SheetEngine::new();
    f.open(
        "<html><body><table><tr><th>Mặt hàng</th><th>SL</th></tr><tr><td>Bút</td><td>12</td></tr></table></body></html>".as_bytes(),
        "bang.htm",
    )
    .unwrap();
    let ws = &f.workbook().unwrap().sheets[0];
    assert_eq!(
        ws.cell_a1("A1").unwrap().value,
        CellValue::Text("Mặt hàng".into())
    );
    assert_eq!(ws.cell_a1("B2").unwrap().value, CellValue::Number(12.0));
    // The document engine does not take a workbook.
    let mut e = Engine::with_bundled_fonts();
    assert!(e
        .open(&corpus("sheet/xml2003-1-budget.xml"), "budget.xml")
        .is_err());
}

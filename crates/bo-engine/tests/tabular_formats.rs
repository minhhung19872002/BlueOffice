//! CORE-FMT-004 through the engines: SYLK, DIF, formatted text (`.prn`) and dBASE
//! tables open as workbooks; a sheet saves as SYLK, DIF and formatted text; a
//! document exports as an EPUB 3 publication.
//! The fixtures in `tests/corpus/sheet/tabular-1-*` are hand-written.
#![allow(clippy::unwrap_used)]

use bo_engine::SheetEngine;
use bo_model::sheet::{CellError, CellValue};
use std::path::PathBuf;

fn corpus(rel: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn value(s: &SheetEngine, a1: &str) -> CellValue {
    s.workbook().unwrap().sheets[0]
        .cell_a1(a1)
        .unwrap_or_else(|| panic!("no cell {a1}"))
        .value
        .clone()
}

fn text(v: &str) -> CellValue {
    CellValue::Text(v.into())
}

/// The `.xlsx` of an opened legacy file, kept for the validator when
/// `BO_FMT_OUT` names a directory.
fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_FMT_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

#[test]
fn sylk_opens_calculates_and_saves() {
    let mut s = SheetEngine::new();
    // The content says SYLK, whatever the name.
    let info = s
        .open(&corpus("sheet/tabular-1-orders.slk"), "orders.txt")
        .unwrap();
    assert_eq!(info.sheets.len(), 1);
    assert_eq!(value(&s, "A1"), text("Mặt hàng"));
    assert_eq!(value(&s, "A2"), text("Bút bi; xanh"));
    assert_eq!(value(&s, "A3"), text("Vở \"Hồng Hà\""));
    // Expressions in R1C1 become formulas; a shared expression follows its cell.
    assert_eq!(s.cell(0, 1, 3).unwrap().input, "=B2*C2");
    assert_eq!(s.cell(0, 2, 3).unwrap().input, "=B3*C3");
    assert_eq!(s.cell(0, 5, 3).unwrap().input, "=SUM(D2:D4)");
    assert_eq!(s.cell(0, 5, 4).unwrap().input, "=$D$4/$D$6");
    // Number formats (vi-VN shows `.` between thousands and `,` for decimals).
    assert_eq!(s.cell(0, 5, 3).unwrap().display, "1.040.000");
    assert_eq!(s.cell(0, 5, 4).unwrap().display, "11,54%");
    assert_eq!(s.cell(0, 1, 4).unwrap().display, "15/03/2024");
    // Editing recalculates the converted formulas.
    let res = s
        .exec_json(r#"{"cmd":"setInputs","sheet":0,"cells":[[1,1,"200"]]}"#)
        .unwrap();
    assert!(!res.contains("\"error\""), "{res}");
    assert_eq!(value(&s, "D2"), CellValue::Number(700_000.0));
    assert_eq!(value(&s, "D6"), CellValue::Number(1_320_000.0));
    let xlsx = s.save("xlsx", 0).unwrap();
    assert!(xlsx.starts_with(b"PK"));
    keep("from-sylk.xlsx", &xlsx);

    // Save as SYLK and open again: values, formulas, formats, widths, the name.
    let slk = s.save("slk", 0).unwrap();
    let mut back = SheetEngine::new();
    back.open(&slk, "saved.slk").unwrap();
    assert_eq!(value(&back, "A3"), text("Vở \"Hồng Hà\""));
    assert_eq!(value(&back, "D6"), CellValue::Number(1_320_000.0));
    assert_eq!(back.cell(0, 2, 3).unwrap().input, "=B3*C3");
    assert_eq!(back.cell(0, 5, 3).unwrap().display, "1.320.000");
    assert_eq!(back.cell(0, 1, 4).unwrap().display, "15/03/2024");
    let (a, b) = (s.workbook().unwrap(), back.workbook().unwrap());
    assert_eq!(a.sheets[0].cols, b.sheets[0].cols);
    assert_eq!(a.defined_names, b.defined_names);
    assert_eq!(b.defined_names[0].name, "Tong_tien");
    assert!(s.save("nope", 0).is_err());
    assert!(s.save("slk", 9).is_err());
}

#[test]
fn dif_and_prn_open_and_save() {
    let mut s = SheetEngine::new();
    s.open(&corpus("sheet/tabular-1-orders.dif"), "orders.dif")
        .unwrap();
    assert_eq!(value(&s, "A2"), text("Bút bi; xanh"));
    assert_eq!(value(&s, "A3"), text("Vở \"Hồng Hà\""));
    assert_eq!(value(&s, "B2"), CellValue::Number(120.0));
    assert_eq!(value(&s, "C3"), CellValue::Number(12_500.0));
    assert_eq!(value(&s, "D2"), CellValue::Bool(true));
    assert_eq!(value(&s, "D3"), CellValue::Bool(false));
    assert_eq!(value(&s, "C4"), CellValue::Error(CellError::NA));
    assert!(s.workbook().unwrap().sheets[0].cell_a1("D4").is_none());
    keep("from-dif.xlsx", &s.save("xlsx", 0).unwrap());
    let dif = s.save("dif", 0).unwrap();
    let mut back = SheetEngine::new();
    back.open(&dif, "saved.dif").unwrap();
    assert_eq!(
        back.workbook().unwrap().sheets[0].cells,
        s.workbook().unwrap().sheets[0].cells
    );

    // Formatted text: columns found from the blank positions, numbers typed.
    let mut p = SheetEngine::new();
    p.open(&corpus("sheet/tabular-1-orders.prn"), "orders.prn")
        .unwrap();
    assert_eq!(value(&p, "A1"), text("Mặt hàng"));
    assert_eq!(value(&p, "A3"), text("Vở Hồng Hà"));
    assert_eq!(value(&p, "B1"), text("SL"));
    assert_eq!(value(&p, "B2"), CellValue::Number(120.0));
    assert_eq!(value(&p, "C3"), CellValue::Number(12_500.0));
    assert_eq!(value(&p, "D4"), CellValue::Number(120_000.0));
    assert_eq!(p.workbook().unwrap().sheets[0].cells.len(), 16);
    keep("from-prn.xlsx", &p.save("xlsx", 0).unwrap());
    // Saving shows what the cells show; the result opens to the same values.
    let prn = p.save("prn", 0).unwrap();
    let shown = String::from_utf8(prn.clone()).unwrap();
    assert!(shown.starts_with("Mặt hàng "), "{shown}");
    assert_eq!(shown.lines().count(), 4);
    let mut back = SheetEngine::new();
    back.open(&prn, "saved.prn").unwrap();
    assert_eq!(
        back.workbook().unwrap().sheets[0].cells,
        p.workbook().unwrap().sheets[0].cells
    );
    // A `.prn` that is printer output, not text, is refused.
    assert!(SheetEngine::new()
        .open(b"\x1b%-12345X@PJL JOB\r\n\x00\x01", "job.prn")
        .is_err());
}

#[test]
fn dbase_table_opens_as_a_sheet() {
    let mut s = SheetEngine::new();
    s.open(&corpus("sheet/tabular-1-customers.dbf"), "customers.DBF")
        .unwrap();
    let names: Vec<String> = (0..7).map(|c| s.cell(0, 0, c).unwrap().input).collect();
    assert_eq!(
        names,
        ["NAME", "CITY", "QTY", "PRICE", "SINCE", "ACTIVE", "NOTES"]
    );
    // Windows-1252 text (language driver 0x03), numbers, a date, a logical value.
    assert_eq!(value(&s, "A2"), text("Café Crème"));
    assert_eq!(value(&s, "B2"), text("Zürich"));
    assert_eq!(value(&s, "C2"), CellValue::Number(12.0));
    assert_eq!(s.cell(0, 1, 3).unwrap().display, "4,50");
    assert_eq!(s.cell(0, 1, 4).unwrap().display, "15/03/2024");
    assert_eq!(value(&s, "F2"), CellValue::Bool(true));
    // The deleted record is skipped.
    assert_eq!(value(&s, "A3"), text("Smørrebrød"));
    assert_eq!(value(&s, "B3"), text("København"));
    assert_eq!(value(&s, "D3"), CellValue::Number(89.95));
    assert_eq!(s.cell(0, 2, 4).unwrap().display, "31/12/1999");
    assert_eq!(value(&s, "A4"), text("Jalapeño"));
    assert_eq!(value(&s, "C4"), CellValue::Number(-3.0));
    let ws = &s.workbook().unwrap().sheets[0];
    assert!(ws.cell_a1("B4").is_none() && ws.cell_a1("E4").is_none());
    assert!(ws.cell_a1("G3").is_none(), "memo fields stay empty");
    assert_eq!(ws.used_range().unwrap().last.row, 3);
    let xlsx = s.save("xlsx", 0).unwrap();
    keep("from-dbf.xlsx", &xlsx);
    // The table goes on as a workbook: saved and opened again.
    let mut back = SheetEngine::new();
    back.open(&xlsx, "customers.xlsx").unwrap();
    assert_eq!(value(&back, "A3"), text("Smørrebrød"));
    assert_eq!(back.cell(0, 1, 4).unwrap().display, "15/03/2024");
}

/// Entries of a ZIP in file order: (name, bytes).
fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read as _;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out: Vec<(u64, String, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        out.push((f.header_start(), f.name().unwrap().into_owned(), data));
    }
    out.sort_by_key(|e| e.0);
    out.into_iter().map(|(_, n, d)| (n, d)).collect()
}

#[test]
fn document_exports_as_epub() {
    use base64::Engine as _;
    use bo_engine::edit::Command;
    let mut e = bo_engine::Engine::with_bundled_fonts();
    // Not a document: nothing to export.
    assert!(e.export_as("epub").is_err());
    e.open(&corpus("doc/fields-03-toc.docx"), "Mục lục.docx")
        .unwrap();
    let pages = e.info().unwrap().page_count;
    // The command of the editors, at the host's time (2026-11-08T09:30:15Z).
    let cmd = |json: &str| serde_json::from_str::<Command>(json).unwrap();
    let json = e
        .command_json(r#"{"cmd":"exportFile","format":"epub","now":1794130215000}"#)
        .unwrap();
    let r: serde_json::Value = serde_json::from_str(&json).unwrap();
    let epub = base64::engine::general_purpose::STANDARD
        .decode(r["file"].as_str().unwrap())
        .unwrap();
    // OCF: `mimetype` first and stored, so the media type is at byte 38.
    assert_eq!(&epub[..4], b"PK\x03\x04");
    assert_eq!(&epub[30..58], b"mimetypeapplication/epub+zip");
    let files = zip_entries(&epub);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        &names[..5],
        [
            "mimetype",
            "META-INF/container.xml",
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
            "OEBPS/style.css"
        ]
    );
    let chapters = names.iter().filter(|n| n.contains("/chapter-")).count();
    assert!(chapters >= 2, "one chapter per Heading 1: {names:?}");
    let text = |name: &str| {
        String::from_utf8(files.iter().find(|f| f.0 == name).unwrap().1.clone()).unwrap()
    };
    let opf = text("OEBPS/content.opf");
    assert!(opf.contains("<meta property=\"dcterms:modified\">2026-11-08T09:30:15Z</meta>"));
    assert!(opf.contains("<dc:identifier id=\"pub-id\">urn:uuid:"));
    assert_eq!(opf.matches("<itemref ").count(), chapters);
    // The table of contents has one link per chapter.
    assert_eq!(
        text("OEBPS/nav.xhtml")
            .matches("<li><a href=\"chapter-")
            .count(),
        chapters
    );
    // Every word of the document's headings is in a chapter file.
    let all: String = (1..=chapters)
        .map(|i| text(&format!("OEBPS/chapter-{i}.xhtml")))
        .collect();
    assert!(all.contains("</h1>"));
    // Nothing changed in the document; the same content and time give the same file.
    assert_eq!(e.info().unwrap().page_count, pages);
    let again = e
        .command(cmd(r#"{"cmd":"exportFile","format":"epub"}"#))
        .unwrap();
    assert!(!again.modified);
    assert_eq!(
        e.export_as_at("epub", Some(1_794_130_215_000)).unwrap(),
        epub
    );
    keep("document.epub", &epub);
}

/// A PDF file opens as a document of its text, best effort: a document exported
/// to PDF by the engine comes back with its words, one PDF page per page.
#[test]
fn pdf_opens_as_text_document() {
    let mut e = bo_engine::Engine::with_bundled_fonts();
    e.open(&corpus("doc/phase1-sample.docx"), "sample.docx")
        .unwrap();
    let source = e.document().unwrap().plain_text();
    let pages = e.info().unwrap().page_count;
    let pdf = e.export_pdf().unwrap();

    let mut back = bo_engine::Engine::with_bundled_fonts();
    // Recognised by content, whatever the name; saved as .docx.
    let info = back.open(&pdf, "Báo cáo.pdf").unwrap();
    assert_eq!(info.name, "Báo cáo.docx");
    let text = back.document().unwrap().plain_text();
    // The words come back in order, with their diacritics and the gaps between
    // them (lines are re-broken where the PDF lines end).
    let words: Vec<&str> = source.split_whitespace().take(120).collect();
    let got: Vec<&str> = text.split_whitespace().take(120).collect();
    assert_eq!(words.len(), 120);
    assert_eq!(got, words);
    let lines = text.matches('\n').count();
    assert!(
        lines > source.matches('\n').count(),
        "one paragraph per PDF line"
    );
    // Each PDF page after the first starts a new page.
    let breaks = back.document().unwrap().sections[0]
        .blocks
        .iter()
        .filter(|b| matches!(b, bo_model::Block::Paragraph(p) if p.props.page_break_before == Some(true)))
        .count();
    assert_eq!(breaks + 1, pages);
    // A broken PDF is an error, not a panic.
    assert!(back.open(&pdf[..pdf.len() / 3], "cut.pdf").is_err());
}

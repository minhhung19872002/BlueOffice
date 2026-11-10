//! PDF/A, tagged PDF and bookmarks of the engine exports (CORE-FMT-002): the
//! structure and outline come from the document model (headings, paragraphs),
//! slide titles and sheet names. Checked structurally with `bo_pdf::probe`
//! (ISO 19005 / ISO 32000-1 rules); veraPDF is not run here.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{Engine, SheetEngine};
use bo_pdf::probe::{dict_get, refs, text, Probe};
use bo_pdf::{ExportChoices, PdfA};
use bo_pptx::build::{build_pptx, DeckSpec, ParaSpec, ShapeSpec, SlideSpec};

fn cmd(e: &mut Engine, json: &str) {
    let r: serde_json::Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r["error"].is_null(), "{json}: {r}");
}

fn sel(e: &mut Engine, p: usize, o: usize) {
    cmd(
        e,
        &format!(
            r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{o}}},"focus":{{"p":{p},"o":{o}}}}}}}"#
        ),
    );
}

/// "Chương I" (H1), "Mục 1" (H2), body text, page break, "Chương II" (H1).
fn doc() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(
        "Chương I\nMục 1\nNội dung thứ nhất\nChương II\nKết thúc".as_bytes(),
        "báo cáo.txt",
    )
    .unwrap();
    for (p, style) in [(0, "Heading1"), (1, "Heading2"), (3, "Heading1")] {
        sel(&mut e, p, 0);
        cmd(&mut e, &format!(r#"{{"cmd":"applyStyle","id":"{style}"}}"#));
    }
    sel(&mut e, 2, "Nội dung thứ nhất".len());
    cmd(&mut e, r#"{"cmd":"insertPageBreak"}"#);
    e
}

fn choices(pdfa: Option<PdfA>, tagged: bool, bookmarks: bool) -> ExportChoices {
    ExportChoices {
        pdfa,
        tagged,
        bookmarks,
    }
}

/// Outline item titles, depth first, with their level (1 = top).
fn outline(p: &Probe) -> Vec<(usize, String)> {
    fn walk(p: &Probe, first: Option<String>, level: usize, out: &mut Vec<(usize, String)>) {
        let mut cur = first.and_then(|r| p.deref(&r));
        while let Some(o) = cur {
            out.push((level, text(&dict_get(&o.dict, "Title").unwrap()).unwrap()));
            walk(p, dict_get(&o.dict, "First"), level + 1, out);
            cur = dict_get(&o.dict, "Next").and_then(|r| p.deref(&r));
        }
    }
    let cat = p.catalog().unwrap();
    let Some(root) = dict_get(&cat.dict, "Outlines").and_then(|r| p.deref(&r)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(p, dict_get(&root.dict, "First"), 1, &mut out);
    out
}

/// Structure types of the tree below `StructTreeRoot`, depth first.
fn roles(p: &Probe) -> Vec<String> {
    fn walk(p: &Probe, r: u32, out: &mut Vec<String>) {
        let Some(o) = p.get(r) else { return };
        if let Some(s) = dict_get(&o.dict, "S") {
            out.push(s.trim().trim_start_matches('/').to_owned());
        }
        for k in refs(&dict_get(&o.dict, "K").unwrap_or_default()) {
            walk(p, k, out);
        }
    }
    let cat = p.catalog().unwrap();
    let root = bo_pdf::probe::parse_ref(&dict_get(&cat.dict, "StructTreeRoot").unwrap()).unwrap();
    let mut out = Vec::new();
    walk(p, root, &mut out);
    out
}

fn xmp(p: &Probe) -> String {
    let cat = p.catalog().unwrap();
    let md = p.deref(&dict_get(&cat.dict, "Metadata").unwrap()).unwrap();
    String::from_utf8(md.stream.clone().unwrap()).unwrap()
}

#[test]
fn doc_pdfa_2b_tagged_with_bookmarks_from_headings() {
    let e = doc();
    let bytes = e
        .export_pdf_with(&choices(Some(PdfA::A2b), true, true))
        .unwrap();
    let p = Probe::parse(&bytes).unwrap();
    let cat = p.catalog().unwrap();
    // §12.3.3 outline from the heading paragraphs, nested by outline level.
    assert_eq!(
        outline(&p),
        [
            (1, "Chương I".to_owned()),
            (2, "Mục 1".to_owned()),
            (1, "Chương II".to_owned()),
        ]
    );
    assert_eq!(
        dict_get(&cat.dict, "PageMode").as_deref(),
        Some("/UseOutlines")
    );
    // §14.7.1 MarkInfo + §14.7.2 StructTreeRoot; headings H1/H2, paragraphs P.
    assert!(dict_get(&cat.dict, "MarkInfo")
        .unwrap()
        .contains("/Marked true"));
    let r = roles(&p);
    let heads: Vec<&str> = r
        .iter()
        .map(String::as_str)
        .filter(|s| s.starts_with('H'))
        .collect();
    assert_eq!(heads, ["H1", "H2", "H1"], "{r:?}");
    assert_eq!(r.iter().filter(|s| *s == "P").count(), 2, "{r:?}");
    // Each page with tagged content has a parent-tree key (§14.7.4.4).
    assert!(p
        .pages()
        .iter()
        .all(|pg| dict_get(&pg.dict, "StructParents").is_some()));
    // ISO 19005-2 §6.6.2: XMP identification; §6.2.3: PDF/A output intent.
    let x = xmp(&p);
    assert!(x.contains("<pdfaid:part>2</pdfaid:part>"), "{x}");
    assert!(x.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
    assert!(x.contains("báo cáo"), "title from the file name: {x}");
    assert!(dict_get(&cat.dict, "OutputIntents")
        .unwrap()
        .contains("/GTS_PDFA1"));
    // ISO 19005-2 §6.1.3: no encryption, a file identifier.
    assert!(dict_get(&p.trailer, "Encrypt").is_none());
    assert!(dict_get(&p.trailer, "ID").is_some());
    // No source file outside PDF/A-3.
    assert!(!String::from_utf8_lossy(&bytes).contains("/EmbeddedFiles"));
}

#[test]
fn doc_pdfa_1b_has_no_transparency_and_3b_embeds_the_docx() {
    let e = doc();
    let one = e
        .export_pdf_with(&choices(Some(PdfA::A1b), false, false))
        .unwrap();
    let p = Probe::parse(&one).unwrap();
    assert_eq!(p.header, "%PDF-1.4");
    let s = String::from_utf8_lossy(&one);
    assert!(!s.contains("/SMask") && !s.contains("/Group") && !s.contains("/ca "));
    assert!(xmp(&p).contains("<pdfaid:part>1</pdfaid:part>"));
    // Neither outline nor structure when not asked for.
    assert_eq!(outline(&p), Vec::<(usize, String)>::new());
    assert!(dict_get(&p.catalog().unwrap().dict, "StructTreeRoot").is_none());

    let three = e
        .export_pdf_with(&choices(Some(PdfA::A3b), false, false))
        .unwrap();
    let p = Probe::parse(&three).unwrap();
    assert!(xmp(&p).contains("<pdfaid:part>3</pdfaid:part>"));
    // ISO 19005-3 §6.8: the source as an associated file (/AF, /AFRelationship).
    let cat = p.catalog().unwrap();
    let af = refs(&dict_get(&cat.dict, "AF").unwrap());
    assert_eq!(af.len(), 1);
    let spec = p.get(af[0]).unwrap();
    assert_eq!(
        text(&dict_get(&spec.dict, "UF").unwrap()).as_deref(),
        Some("báo cáo.docx")
    );
    assert_eq!(
        dict_get(&spec.dict, "AFRelationship").as_deref(),
        Some("/Source")
    );
    let ef = dict_get(&spec.dict, "EF").unwrap();
    let stream = p
        .deref(&dict_get(&ef, "UF").or_else(|| dict_get(&ef, "F")).unwrap())
        .unwrap();
    let docx = Probe::stream_data(stream).unwrap();
    assert!(docx.starts_with(b"PK"), "a zip package");
}

#[test]
fn plain_choices_through_the_slide_export_bridge() {
    // `{"kind":"pdf"}` is the WASM / Tauri path (exportSlides) for BlueDoc too.
    let mut e = doc();
    let bytes = e
        .export_slides_json(r#"{"kind":"pdf","pdfa":"2b","bookmarks":true}"#)
        .unwrap();
    let p = Probe::parse(&bytes).unwrap();
    assert_eq!(outline(&p).len(), 3);
    assert!(dict_get(&p.catalog().unwrap().dict, "StructTreeRoot").is_none());
}

#[test]
fn slides_tagged_with_bookmarks_per_slide() {
    let slides = (0..2)
        .map(|i| SlideSpec {
            shapes: vec![ShapeSpec {
                name: "Text".into(),
                x: 914_400,
                y: 914_400,
                cx: 6_096_000,
                cy: 914_400,
                text: vec![ParaSpec::plain(&format!("Trang {}", i + 1))],
                ..ShapeSpec::default()
            }],
            ..SlideSpec::default()
        })
        .collect();
    let deck = build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        slides,
        title: None,
    })
    .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open_pptx(&deck, "deck.pptx").unwrap();
    let bytes = e
        .export_pdf_with(&choices(Some(PdfA::A2b), true, true))
        .unwrap();
    let p = Probe::parse(&bytes).unwrap();
    // One bookmark per slide (no title placeholder → the slide number).
    assert_eq!(outline(&p), [(1, "1".to_owned()), (1, "2".to_owned())]);
    let r = roles(&p);
    assert_eq!(r.iter().filter(|s| *s == "Sect").count(), 2, "{r:?}");
    assert!(r.iter().any(|s| s == "P"), "{r:?}");
}

#[test]
fn sheet_set_pdf_export_then_export_pdf() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/page-1.xlsx");
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), "page-1.xlsx")
        .unwrap();
    let r: serde_json::Value = serde_json::from_str(
        &e.exec_json(r#"{"cmd":"setPdfExport","pdfa":"1b","tagged":true,"bookmarks":true}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(r["pdfa"], "1b", "{r}");
    let names: Vec<String> = e
        .workbook()
        .unwrap()
        .sheets
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let bytes = e.export_pdf().unwrap();
    let p = Probe::parse(&bytes).unwrap();
    assert_eq!(p.header, "%PDF-1.4");
    assert!(xmp(&p).contains("<pdfaid:part>1</pdfaid:part>"));
    let o = outline(&p);
    assert_eq!(o[0].1, names[0]);
    let r = roles(&p);
    assert!(
        r.iter().any(|s| s == "Sect") && r.iter().any(|s| s == "Table"),
        "{r:?}"
    );
}

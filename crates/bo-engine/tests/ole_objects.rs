//! Embedded and linked workbooks in BlueDoc and BlueSlide (CORE-EMBED-001): insert,
//! preview geometry, edit and re-embed, links, and byte-for-byte round trip of objects
//! read from hand-built files.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{Block, RunContent};
use bo_opc::{Limits, Package};
use std::io::Write as _;
use std::path::PathBuf;

fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn doc_cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(v.get("error").is_none(), "{json}: {v}");
    v
}

fn slide_cmd(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap();
    assert!(v.get("error").is_none(), "{json}: {v}");
    v
}

/// Keep written files for `bo-cli validate --openxml require` when `BO_OLE_OUT` is set.
fn keep(bytes: &[u8], name: &str) {
    if let Ok(dir) = std::env::var("BO_OLE_OUT") {
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn objects(doc: &bo_model::Document) -> Vec<bo_model::Image> {
    let mut out = Vec::new();
    for s in &doc.sections {
        for b in &s.blocks {
            if let Block::Paragraph(p) = b {
                for r in &p.runs {
                    for c in &r.content {
                        if let RunContent::Image(i) = c {
                            if i.ole.is_some() {
                                out.push((**i).clone());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name).map(|p| p.data.clone())
}

fn main_xml(bytes: &[u8]) -> String {
    String::from_utf8(part(bytes, "/word/document.xml").unwrap()).unwrap()
}

/// A small workbook built with BlueSheet: Sheet1!A1:B2.
fn workbook() -> Vec<u8> {
    let mut s = bo_engine::sheet::SheetEngine::with_bundled_fonts();
    s.new_workbook("b.xlsx").unwrap();
    s.set_input(0, 0, 0, "Năm").unwrap();
    s.set_input(0, 0, 1, "Doanh thu").unwrap();
    s.set_input(0, 1, 0, "2026").unwrap();
    s.set_input(0, 1, 1, "=10*12").unwrap();
    s.set_input(0, 5, 3, "outside").unwrap();
    s.save("xlsx", 0).unwrap()
}

#[test]
fn doc_embeds_a_range_edits_and_round_trips() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let book = workbook();
    let v = doc_cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"oleInsert","data":"{}","range":"Sheet1!A1:B2"}}"#,
            b64(&book)
        ),
    );
    let info = &v["ole"];
    assert_eq!(info["workbook"], true);
    assert_eq!(info["linked"], false);
    assert_eq!(info["range"], "A1:B2");
    assert_eq!(info["sheet"], "Sheet1");
    // Preview geometry: the shown size is the size of A1:B2 on the sheet.
    let mut s = bo_engine::sheet::SheetEngine::with_bundled_fonts();
    s.open(&book, "b.xlsx").unwrap();
    let (w, h) = s
        .area_size(0, bo_model::sheet::CellRange::parse("A1:B2").unwrap())
        .unwrap();
    assert!((info["width"].as_f64().unwrap() - w).abs() < 0.01, "{info}");
    assert!(
        (info["height"].as_f64().unwrap() - h).abs() < 0.01,
        "{info}"
    );

    let saved = e.save_docx().unwrap();
    keep(&saved, "doc-embed.docx");
    let xml = main_xml(&saved);
    assert!(xml.contains("<w:object"), "{xml}");
    assert!(xml.contains("ProgID=\"Excel.Sheet.12\""));
    assert!(xml.contains("Type=\"Embed\""));
    let ct = String::from_utf8(part(&saved, "/[Content_Types].xml").unwrap()).unwrap();
    assert!(ct.contains("spreadsheetml.sheet"), "{ct}");

    // Reopen: the object is read back with its package.
    let mut back = Engine::with_bundled_fonts();
    back.open(&saved, "a.docx").unwrap();
    let objs = objects(back.document().unwrap());
    assert_eq!(objs.len(), 1);
    let pkg = objs[0].ole.as_ref().unwrap().package.clone();
    assert!(pkg.starts_with("/word/embeddings/"), "{pkg}");
    let embedded = part(&saved, &pkg).unwrap();
    // The print area marks the shown range; the cells are the workbook's.
    let wb = bo_xlsx::read_xlsx(&embedded).unwrap();
    assert_eq!(
        wb.sheets[0].print_area.as_deref(),
        Some("'Sheet1'!$A$1:$B$2")
    );
    assert!(wb.sheets[0].cell(5, 3).is_some());

    // An unchanged object is written back byte-for-byte.
    let again = back.save_docx().unwrap();
    assert_eq!(part(&again, &pkg), Some(embedded.clone()));
    let x1 = main_xml(&saved);
    let x2 = main_xml(&again);
    let obj = |x: &str| {
        let a = x.find("<w:object").unwrap();
        let b = x.find("</w:object>").unwrap();
        x[a..b].to_owned()
    };
    assert_eq!(obj(&x1), obj(&x2));

    // Edit: the caret is after the object, so it is the selected one.
    doc_cmd(
        &mut back,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":0}}}"#,
    );
    let v = doc_cmd(&mut back, r#"{"cmd":"oleInfo","cells":true}"#);
    assert_eq!(
        v["ole"]["cells"],
        serde_json::json!([["Năm", "Doanh thu"], ["2026", "=10*12"]])
    );
    let v = doc_cmd(
        &mut back,
        r#"{"cmd":"oleEdit","cells":[["Năm","Doanh thu","Lãi"],["2026","=10*12","=B2/4"]]}"#,
    );
    assert_eq!(v["ole"]["range"], "A1:C2");
    let edited = back.save_docx().unwrap();
    keep(&edited, "doc-embed-edited.docx");
    let wb = bo_xlsx::read_xlsx(&part(&edited, &pkg).unwrap()).unwrap();
    assert_eq!(
        wb.sheets[0].print_area.as_deref(),
        Some("'Sheet1'!$A$1:$C$2")
    );
    // B2/4 = 120/4 = 30, cached in the re-embedded workbook.
    assert_eq!(
        wb.sheets[0].cell(1, 2).map(|c| c.value.clone()),
        Some(bo_model::sheet::CellValue::Number(30.0))
    );
    // Wider area, same scale: the object grew by column C.
    let w2 = back
        .document()
        .map(|d| objects(d)[0].width.0 as f64 / 12_700.0)
        .unwrap();
    assert!(w2 > w + 1.0, "{w2} vs {w}");
    // Undo brings the old workbook back.
    doc_cmd(&mut back, r#"{"cmd":"undo"}"#);
    let undone = back.save_docx().unwrap();
    assert_eq!(part(&undone, &pkg), Some(embedded));
}

#[test]
fn doc_new_workbook_object_and_link() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    // New empty workbook: A1:D3.
    let v = doc_cmd(&mut e, r#"{"cmd":"oleInsert"}"#);
    assert_eq!(v["ole"]["range"], "A1:D3");
    // Linked object: the file's bytes now, the path for later updates.
    let book = workbook();
    let v = doc_cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"oleInsert","data":"{}","link":"C:\\data\\sales book.xlsx","range":"A1:B2"}}"#,
            b64(&book)
        ),
    );
    assert_eq!(v["ole"]["linked"], true);
    assert_eq!(v["ole"]["source"], r"C:\data\sales book.xlsx");
    let v = doc_cmd(&mut e, r#"{"cmd":"oleLinks"}"#);
    assert_eq!(v["links"][0]["count"], 1);
    let saved = e.save_docx().unwrap();
    keep(&saved, "doc-link.docx");
    let xml = main_xml(&saved);
    assert!(xml.contains("Type=\"Link\""), "{xml}");
    assert!(xml.contains("LINK Excel.Sheet.12"), "{xml}");
    let rels = String::from_utf8(part(&saved, "/word/_rels/document.xml.rels").unwrap()).unwrap();
    assert!(rels.contains("file:///C:/data/sales%20book.xlsx"), "{rels}");
    assert!(rels.contains("TargetMode=\"External\""));

    // Reopen: the link is found with its item from the LINK field.
    let mut back = Engine::with_bundled_fonts();
    back.open(&saved, "a.docx").unwrap();
    let v = doc_cmd(&mut back, r#"{"cmd":"oleLinks"}"#);
    assert_eq!(v["links"][0]["path"], "C:/data/sales book.xlsx");
    assert_eq!(v["links"][0]["item"], "Sheet1!A1:B2");
    // Update links: a bigger source workbook.
    let mut s = bo_engine::sheet::SheetEngine::with_bundled_fonts();
    s.open(&book, "b.xlsx").unwrap();
    s.set_input(0, 0, 0, "Năm tài chính rất dài").unwrap();
    let newer = s.save("xlsx", 0).unwrap();
    let source = v["links"][0]["source"].as_str().unwrap().to_owned();
    let v = doc_cmd(
        &mut back,
        &format!(
            r#"{{"cmd":"oleUpdateLinks","sources":[{{"source":"{source}","data":"{}"}}]}}"#,
            b64(&newer)
        ),
    );
    assert_eq!(v["count"], 1);
    let after = back.save_docx().unwrap();
    keep(&after, "doc-link-updated.docx");
    assert!(main_xml(&after).contains("Type=\"Link\""));
}

/// A hand-built `.docx` with an embedded object (EMF-less: PNG preview) and an
/// opaque `oleObject1.bin`, as other writers store objects.
fn fixture_docx() -> Vec<u8> {
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: 4,
        height: 2,
        pixels: vec![200; 4 * 2 * 4],
    })
    .unwrap();
    let doc = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body><w:p><w:r><w:t>Before </w:t></w:r><w:r><w:object w:dxaOrig="2880" w:dyaOrig="1440"><v:shape id="_x0000_i1025" style="width:144pt;height:72pt" o:ole=""><v:imagedata r:id="rId2" o:title=""/></v:shape><o:OLEObject Type="Embed" ProgID="Package" ShapeID="_x0000_i1025" DrawAspect="Content" ObjectID="_1700000000" r:id="rId3"/></w:object></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#;
    let ct = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.oleObject"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let root = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject" Target="embeddings/oleObject1.bin"/></Relationships>"#;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default();
    for (n, d) in [
        ("[Content_Types].xml", ct.as_bytes()),
        ("_rels/.rels", root.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", rels.as_bytes()),
        ("word/media/image1.png", png.as_slice()),
        (
            "word/embeddings/oleObject1.bin",
            b"\xD0\xCF\x11\xE0opaque-ole-bytes",
        ),
    ] {
        zip.start_file(n, opt).unwrap();
        zip.write_all(d).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn doc_reads_foreign_object_shows_preview_and_keeps_it() {
    let src = fixture_docx();
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "f.docx").unwrap();
    let objs = objects(e.document().unwrap());
    assert_eq!(objs.len(), 1);
    // 144 pt × 72 pt.
    assert_eq!((objs[0].width.0, objs[0].height.0), (1_828_800, 914_400));
    assert_eq!(objs[0].media, "/word/media/image1.png");
    let o = objs[0].ole.as_ref().unwrap();
    assert_eq!(
        (o.prog_id.as_str(), o.package.as_str()),
        ("Package", "/word/embeddings/oleObject1.bin")
    );
    // The preview is drawn on the page.
    let page = e.page(0).unwrap().clone();
    assert!(
        page.commands
            .iter()
            .any(|c| matches!(c, bo_render::Command::Image { .. })),
        "no picture drawn"
    );
    // Edit the text around it: the object and its part stay byte-for-byte.
    doc_cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":0}}}"#,
    );
    doc_cmd(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
    let out = e.save_docx().unwrap();
    let a = main_xml(&src);
    let b = main_xml(&out);
    let obj = |x: &str| {
        let i = x.find("<w:object").unwrap();
        let j = x.find("</w:object>").unwrap();
        x[i..j].to_owned()
    };
    assert_eq!(obj(&a), obj(&b));
    assert_eq!(
        part(&out, "/word/embeddings/oleObject1.bin"),
        part(&src, "/word/embeddings/oleObject1.bin")
    );
    // A non-workbook object cannot be edited as one.
    doc_cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":8},"focus":{"p":0,"o":8}}}"#,
    );
    let v: serde_json::Value = serde_json::from_str(
        &e.command_json(r#"{"cmd":"oleEdit","cells":[["1"]]}"#)
            .unwrap(),
    )
    .unwrap();
    assert!(v.get("error").is_some(), "{v}");
}

fn deck() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-shapes-groups.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "d.pptx").unwrap();
    e
}

#[test]
fn slide_embeds_edits_links_and_round_trips() {
    let mut e = deck();
    let book = workbook();
    let v = slide_cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"oleInsert","data":"{}","range":"A1:B2"}}"#,
            b64(&book)
        ),
    );
    assert_eq!(v["selection"][0]["kind"], "object", "{v}");
    assert_eq!(v["ole"]["range"], "A1:B2");
    let mut s = bo_engine::sheet::SheetEngine::with_bundled_fonts();
    s.open(&book, "b.xlsx").unwrap();
    let (w, h) = s
        .area_size(0, bo_model::sheet::CellRange::parse("A1:B2").unwrap())
        .unwrap();
    assert!((v["ole"]["width"].as_f64().unwrap() - w).abs() < 0.01);
    assert!((v["ole"]["height"].as_f64().unwrap() - h).abs() < 0.01);
    let saved = e.save_pptx().unwrap();
    keep(&saved, "slide-embed.pptx");
    let pkg = Package::read(&saved, &Limits::default()).unwrap();
    let book_part = pkg
        .parts()
        .iter()
        .find(|p| p.name.starts_with("/ppt/embeddings/Workbook"))
        .map(|p| p.name.clone())
        .unwrap();
    // Reopen: the frame shows its preview picture.
    let mut back = Engine::with_bundled_fonts();
    back.open(&saved, "d.pptx").unwrap();
    let list = back.page(0).unwrap().clone();
    assert!(list
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::Image { .. })));
    // Unchanged: byte-for-byte.
    let again = back.save_pptx().unwrap();
    assert_eq!(part(&again, &book_part), part(&saved, &book_part));
    // Edit the selected object.
    let id = v["selection"][0]["id"].as_u64().unwrap();
    slide_cmd(&mut back, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    let v = slide_cmd(
        &mut back,
        r#"{"cmd":"oleEdit","cells":[["a","b","c"],["1","2","=A2+B2"],["4","5","6"]]}"#,
    );
    assert_eq!(v["ole"]["range"], "A1:C3", "{v}");
    let edited = back.save_pptx().unwrap();
    keep(&edited, "slide-embed-edited.pptx");
    let wb = bo_xlsx::read_xlsx(&part(&edited, &book_part).unwrap()).unwrap();
    assert_eq!(
        wb.sheets[0].cell(1, 2).map(|c| c.value.clone()),
        Some(bo_model::sheet::CellValue::Number(3.0))
    );
    // Linked object.
    let v = slide_cmd(
        &mut back,
        &format!(
            r#"{{"cmd":"oleInsert","data":"{}","link":"/home/u/b.xlsx","autoUpdate":true}}"#,
            b64(&book)
        ),
    );
    assert_eq!(v["ole"]["linked"], true);
    assert_eq!(v["ole"]["autoUpdate"], true);
    let v = slide_cmd(&mut back, r#"{"cmd":"oleLinks"}"#);
    assert_eq!(v["links"][0]["path"], "/home/u/b.xlsx");
    let v = slide_cmd(
        &mut back,
        &format!(
            r#"{{"cmd":"oleUpdateLinks","sources":[{{"source":"/home/u/b.xlsx","data":"{}"}}]}}"#,
            b64(&book)
        ),
    );
    assert_eq!(v["count"], 1);
    let linked = back.save_pptx().unwrap();
    keep(&linked, "slide-link.pptx");
    let slide = String::from_utf8(part(&linked, "/ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(slide.contains("<p:link updateAutomatic=\"1\"/>"), "{slide}");
}

/// Rewrite one part of a package.
fn with_part(bytes: &[u8], name: &str, data: &[u8]) -> Vec<u8> {
    let mut pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.put_part(name, data.to_vec());
    pkg.write().unwrap()
}

#[test]
fn slide_alternate_content_object_is_drawn_and_kept() {
    // An object frame as other writers store it: a VML choice and a fallback with the
    // preview picture (ECMA-376 Part 3, §10.2.1).
    let mut e = deck();
    slide_cmd(
        &mut e,
        &format!(r#"{{"cmd":"oleInsert","data":"{}"}}"#, b64(&workbook())),
    );
    let saved = e.save_pptx().unwrap();
    let xml = String::from_utf8(part(&saved, "/ppt/slides/slide1.xml").unwrap()).unwrap();
    let o = xml.find("<p:oleObj").unwrap();
    let a = xml[..o].rfind("<p:graphicFrame>").unwrap();
    let b = o + xml[o..].find("</p:graphicFrame>").unwrap() + "</p:graphicFrame>".len();
    let frame = &xml[a..b];
    let cut = frame.find("<p:embed/>").unwrap() + "<p:embed/>".len();
    let choice = format!(
        "{}</p:oleObj></a:graphicData></a:graphic></p:graphicFrame>",
        &frame[..cut]
    )
    .replace("<p:oleObj ", "<p:oleObj spid=\"_x0000_s1026\" ");
    let alt = format!(
        r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:v="urn:schemas-microsoft-com:vml" Requires="v">{choice}</mc:Choice><mc:Fallback>{frame}</mc:Fallback></mc:AlternateContent>"#
    );
    let fixture = with_part(
        &saved,
        "/ppt/slides/slide1.xml",
        xml.replace(frame, &alt).as_bytes(),
    );
    let images = |e: &Engine| {
        e.page(0)
            .unwrap()
            .commands
            .iter()
            .filter(|c| matches!(c, bo_render::Command::Image { .. }))
            .count()
    };
    let mut f = Engine::with_bundled_fonts();
    f.open(&fixture, "alt.pptx").unwrap();
    let mut plain = Engine::with_bundled_fonts();
    plain.open(&saved, "p.pptx").unwrap();
    assert!(images(&plain) > 0);
    assert_eq!(images(&f), images(&plain));
    // Saved again unchanged: the AlternateContent and the workbook stay as they were.
    let out = f.save_pptx().unwrap();
    let back = String::from_utf8(part(&out, "/ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(back.contains("Requires=\"v\""), "{back}");
    assert!(back.contains("spid=\"_x0000_s1026\""));
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    let mut n = 0;
    for p in pkg
        .parts()
        .iter()
        .filter(|p| p.name.contains("/embeddings/"))
    {
        assert_eq!(part(&fixture, &p.name).as_deref(), Some(p.data.as_slice()));
        n += 1;
    }
    assert_eq!(n, 1);
}

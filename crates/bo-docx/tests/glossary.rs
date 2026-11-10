//! Glossary document part (ECMA-376 Part 1, §17.12; part §11.3.8): reading the
//! building blocks, byte-for-byte round trip of an untouched glossary, rewriting
//! when entries are added, edited or removed, and a glossary in a new document.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT, CT_GLOSSARY, CT_STYLES};
use bo_model::glossary::{DocPart, DocPartProps, Glossary};
use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_GLOSSARY: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument";
const GLOSSARY: &str = "/word/glossary/document.xml";

/// The glossary part of the fixture: an AutoText entry, a header block with a style
/// of the glossary's own styles part, and a content-control placeholder.
fn glossary_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:glossaryDocument xmlns:w=\"{W}\"><w:docParts>\
<w:docPart><w:docPartPr><w:name w:val=\"kg\"/><w:category><w:name w:val=\"Hành chính\"/><w:gallery w:val=\"autoTxt\"/></w:category><w:types><w:type w:val=\"normal\"/></w:types><w:behaviors><w:behavior w:val=\"content\"/></w:behaviors><w:description w:val=\"Lời kính gửi\"/><w:guid w:val=\"{{6F1C2D3E-0001-4A5B-8C7D-9E0F1A2B3C4D}}\"/></w:docPartPr><w:docPartBody><w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Kính gửi: </w:t></w:r></w:p></w:docPartBody></w:docPart>\
<w:docPart><w:docPartPr><w:name w:val=\"Đầu trang đơn giản\"/><w:style w:val=\"BlockHeader\"/><w:category><w:name w:val=\"General\"/><w:gallery w:val=\"hdrs\"/></w:category><w:behaviors><w:behavior w:val=\"p\"/></w:behaviors><w:guid w:val=\"{{6F1C2D3E-0002-4A5B-8C7D-9E0F1A2B3C4D}}\"/></w:docPartPr><w:docPartBody><w:p><w:pPr><w:pStyle w:val=\"BlockHeader\"/></w:pPr><w:r><w:t>[Tên tài liệu]</w:t></w:r></w:p><w:p><w:pPr><w:jc w:val=\"right\"/></w:pPr><w:r><w:t>Trang </w:t></w:r><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p></w:docPartBody></w:docPart>\
<w:docPart><w:docPartPr><w:name w:val=\"DefaultPlaceholder_1\"/><w:category><w:name w:val=\"General\"/><w:gallery w:val=\"placeholder\"/></w:category><w:types><w:type w:val=\"bbPlcHdr\"/></w:types><w:behaviors><w:behavior w:val=\"content\"/></w:behaviors><w:guid w:val=\"{{6F1C2D3E-0003-4A5B-8C7D-9E0F1A2B3C4D}}\"/></w:docPartPr><w:docPartBody><w:p><w:r><w:t>Bấm để nhập</w:t></w:r></w:p></w:docPartBody></w:docPart>\
</w:docParts></w:glossaryDocument>"
    )
}

fn glossary_styles() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W}\"><w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val=\"22\"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"BlockHeader\"><w:name w:val=\"Block Header\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:i/></w:rPr></w:style></w:styles>"
    )
}

/// A package with a main part and a glossary document with its own styles.
fn fixture() -> Vec<u8> {
    let mut p = Package::new();
    let main = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{W}\"><w:body><w:p><w:r><w:t>Thân văn bản</w:t></w:r></w:p><w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1134\" w:right=\"851\" w:bottom=\"1134\" w:left=\"1701\" w:header=\"567\" w:footer=\"567\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, main.into_bytes())
        .unwrap();
    p.add_part(GLOSSARY, CT_GLOSSARY, glossary_xml().into_bytes())
        .unwrap();
    p.add_part(
        "/word/glossary/styles.xml",
        CT_STYLES,
        glossary_styles().into_bytes(),
    )
    .unwrap();
    let rel = |id: &str, ty: &str, target: &str| Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    };
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "word/document.xml")],
    );
    p.set_relationships(
        "/word/document.xml",
        &[rel("rId1", REL_GLOSSARY, "glossary/document.xml")],
    );
    p.set_relationships(GLOSSARY, &[rel("rId1", REL_STYLES, "styles.xml")]);
    p.write().unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name).map(|p| p.data.clone())
}

fn text(p: &DocPart) -> String {
    p.blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(Paragraph::plain_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn block(name: &str, gallery: &str, body: &str) -> DocPart {
    DocPart {
        props: DocPartProps {
            name: name.into(),
            gallery: gallery.into(),
            category: "General".into(),
            description: "Mô tả & ghi chú".into(),
            types: vec!["normal".into()],
            behaviors: vec!["p".into()],
            guid: Some("{6F1C2D3E-0009-4A5B-8C7D-9E0F1A2B3C4D}".into()),
            style: None,
        },
        blocks: vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(body, RunProps::default())],
        ))],
        src: None,
    }
}

#[test]
fn reads_the_building_blocks() {
    let doc = read_docx(&fixture()).unwrap();
    let g = doc.glossary.as_deref().expect("glossary");
    assert_eq!(g.parts.len(), 3);
    let kg = &g.parts[0];
    assert_eq!(kg.props.name, "kg");
    assert_eq!(kg.props.gallery, "autoTxt");
    assert_eq!(kg.props.category, "Hành chính");
    assert_eq!(kg.props.description, "Lời kính gửi");
    assert_eq!(kg.props.types, ["normal"]);
    assert_eq!(kg.props.behavior(), "content");
    assert_eq!(
        kg.props.guid.as_deref(),
        Some("{6F1C2D3E-0001-4A5B-8C7D-9E0F1A2B3C4D}")
    );
    assert_eq!(text(kg), "Kính gửi: ");
    let hdr = &g.parts[1];
    assert_eq!(hdr.props.gallery, "hdrs");
    assert_eq!(hdr.props.behavior(), "p");
    assert_eq!(hdr.props.style.as_deref(), Some("BlockHeader"));
    assert_eq!(hdr.blocks.len(), 2);
    // The glossary's own styles part.
    assert!(g.styles.styles.iter().any(|s| s.id == "BlockHeader"));
    assert!(doc.style("BlockHeader").is_none());
    // Placeholders of content controls are not gallery entries.
    assert!(g.parts[2].props.is_placeholder());
    assert_eq!(g.listed().count(), 2);
    assert!(g.unchanged());
}

#[test]
fn untouched_glossary_round_trips_byte_for_byte() {
    let src = fixture();
    let mut doc = read_docx(&src).unwrap();
    // Editing the body does not touch the glossary parts.
    if let Some(Block::Paragraph(p)) = doc.sections[0].blocks.first_mut() {
        p.runs = vec![Run::text("Đã sửa", RunProps::default())];
    }
    let out = write_docx(&doc).unwrap();
    for name in [
        GLOSSARY,
        "/word/glossary/styles.xml",
        "/word/glossary/_rels/document.xml.rels",
    ] {
        assert_eq!(part(&out, name), part(&src, name), "{name}");
    }
    let again = read_docx(&out).unwrap();
    assert_eq!(again.glossary.as_deref().map(|g| g.parts.len()), Some(3));
}

#[test]
fn added_block_is_written_and_other_entries_stay_verbatim() {
    let src = fixture();
    let mut doc = read_docx(&src).unwrap();
    let mut g: Glossary = doc.glossary.as_deref().cloned().unwrap();
    g.parts
        .push(block("Nơi nhận", "autoTxt", "Nơi nhận: <Lưu VT>"));
    doc.glossary = Some(Arc::new(g));
    let out = write_docx(&doc).unwrap();
    let xml = String::from_utf8(part(&out, GLOSSARY).unwrap()).unwrap();
    // The entries read from the file are byte-identical; the new one follows them.
    let old = glossary_xml();
    let old_parts = &old[old.find("<w:docPart>").unwrap()..old.rfind("</w:docParts>").unwrap()];
    assert!(xml.contains(old_parts));
    assert!(xml.contains("<w:name w:val=\"Nơi nhận\"/>"));
    assert!(xml.contains("<w:description w:val=\"Mô tả &amp; ghi chú\"/>"));
    // Styles of the glossary did not change: the part is kept.
    assert_eq!(
        part(&out, "/word/glossary/styles.xml"),
        part(&src, "/word/glossary/styles.xml")
    );
    let again = read_docx(&out).unwrap();
    let g = again.glossary.as_deref().unwrap();
    assert_eq!(g.parts.len(), 4);
    assert_eq!(g.parts[3].props.name, "Nơi nhận");
    assert_eq!(g.parts[3].props.behavior(), "p");
    assert_eq!(text(&g.parts[3]), "Nơi nhận: <Lưu VT>");
    assert!(g.unchanged());
}

#[test]
fn edited_and_removed_entries() {
    let mut doc = read_docx(&fixture()).unwrap();
    let mut g: Glossary = doc.glossary.as_deref().cloned().unwrap();
    g.parts.remove(0);
    g.parts[0].props.name = "Đầu trang mới".into();
    g.parts[0].props.category = "Báo cáo".into();
    doc.glossary = Some(Arc::new(g));
    let again = read_docx(&write_docx(&doc).unwrap()).unwrap();
    let g = again.glossary.as_deref().unwrap();
    assert_eq!(g.parts.len(), 2);
    assert_eq!(g.parts[0].props.name, "Đầu trang mới");
    assert_eq!(g.parts[0].props.category, "Báo cáo");
    assert_eq!(g.parts[0].props.style.as_deref(), Some("BlockHeader"));
    assert_eq!(text(&g.parts[0]), "[Tên tài liệu]\nTrang 1");
    assert!(g.parts[1].props.is_placeholder());
}

#[test]
fn removing_every_block_removes_the_glossary_parts() {
    let mut doc = read_docx(&fixture()).unwrap();
    doc.glossary = None;
    let out = write_docx(&doc).unwrap();
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    assert!(pkg.parts().iter().all(|p| !p.name.contains("/glossary/")));
    assert!(pkg
        .relationships("/word/document.xml")
        .unwrap()
        .iter()
        .all(|r| r.rel_type != REL_GLOSSARY));
    assert!(read_docx(&out).unwrap().glossary.is_none());
}

#[test]
fn new_document_with_a_glossary() {
    let mut doc = Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::default())])],
        ..Document::default()
    };
    let mut style =
        bo_model::Style::new("BlockTitle", "Block Title", bo_model::StyleKind::Paragraph);
    style.rpr.bold = Some(true);
    let mut entry = block("Tiêu đề", "docParts", "Tiêu đề khối");
    if let Some(Block::Paragraph(p)) = entry.blocks.first_mut() {
        p.props.style = Some("BlockTitle".into());
    }
    doc.glossary = Some(Arc::new(Glossary {
        parts: vec![entry],
        styles: bo_model::StyleSheet {
            styles: vec![style],
            ..bo_model::StyleSheet::default()
        },
        ..Glossary::default()
    }));
    let out = write_docx(&doc).unwrap();
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    assert_eq!(
        pkg.content_types().unwrap().get(GLOSSARY),
        Some(CT_GLOSSARY)
    );
    assert_eq!(
        pkg.related_part(GLOSSARY, REL_STYLES).unwrap().as_deref(),
        Some("/word/glossary/styles.xml")
    );
    let again = read_docx(&out).unwrap();
    let g = again.glossary.as_deref().unwrap();
    assert_eq!(g.parts.len(), 1);
    assert_eq!(g.parts[0].props.name, "Tiêu đề");
    assert!(g.styles.styles.iter().any(|s| s.id == "BlockTitle"));
    // Writing it again keeps the glossary parts as they are.
    let twice = write_docx(&again).unwrap();
    assert_eq!(part(&twice, GLOSSARY), part(&out, GLOSSARY));
}

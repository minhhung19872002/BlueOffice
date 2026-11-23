//! Page background fill effects (ECMA-376 Part 1, §17.2.1 background with the VML
//! `v:background`, Part 4 §19.1.2.2, and its `v:fill`, §19.1.2.5): reading, lossless
//! writing, writing after an edit (with the picture relationship).
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::{Color, PageFill, PageFillKind};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const V: &str = "urn:schemas-microsoft-com:vml";
const O: &str = "urn:schemas-microsoft-com:office:office";
const IMG: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn rel(id: &str, ty: &str, target: &str) -> Relationship {
    Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    }
}

fn sample(fill: &str) -> Vec<u8> {
    let mut p = Package::new();
    let doc = format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:v="{V}" xmlns:o="{O}"><w:background w:color="2070C0"><v:background id="_x0000_s1025" o:bwmode="white" fillcolor="#2070c0" o:targetscreensize="1024,768">{fill}</v:background></w:background><w:body><w:p><w:r><w:t>Nền trang</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"##
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    p.add_part("/word/media/image1.png", "image/png", PNG.to_vec())
        .unwrap();
    p.set_relationships(
        "/word/document.xml",
        &[rel("rId4", IMG, "media/image1.png")],
    );
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "word/document.xml")],
    );
    p.write().unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

#[test]
fn reads_gradient_and_keeps_it_verbatim() {
    let src = sample(r##"<v:fill color2="#ffe060" focus="-50%" angle="-45" type="gradient"/>"##);
    let doc = read_docx(&src).unwrap();
    assert_eq!(doc.design.background, Color::from_hex("2070C0"));
    let f = doc.design.background_fill.as_ref().unwrap();
    assert_eq!(f.kind, PageFillKind::Gradient);
    assert_eq!((f.angle, f.focus), (-45, -50));
    assert_eq!(f.color2, Color::from_hex("FFE060"));
    // Unchanged: the main part's background stays byte for byte.
    let out = write_docx(&doc).unwrap();
    assert!(part(&out, "/word/document.xml")
        .contains(r##"<v:fill color2="#ffe060" focus="-50%" angle="-45" type="gradient"/>"##));
}

#[test]
fn picture_fill_reads_media_and_writes_relationship() {
    let src = sample(r#"<v:fill r:id="rId4" o:title="Canvas" type="tile"/>"#);
    let doc = read_docx(&src).unwrap();
    let f = doc.design.background_fill.clone().unwrap();
    assert_eq!(f.kind, PageFillKind::Tile);
    assert_eq!(f.image.as_deref(), Some("/word/media/image1.png"));
    assert!(doc.media.contains_key("/word/media/image1.png"));
    // Edited: a picture (frame) fill written anew, its relationship pointing at the media.
    let mut doc = doc;
    doc.design.background_fill = Some(PageFill {
        kind: PageFillKind::Frame,
        title: Some("Ảnh".into()),
        lock_aspect: true,
        ..f
    });
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    assert_eq!(xml.matches("<w:background").count(), 1, "{xml}");
    assert!(xml.contains("type=\"frame\""), "{xml}");
    let back = read_docx(&out).unwrap();
    let g = back.design.background_fill.unwrap();
    assert_eq!(g.kind, PageFillKind::Frame);
    assert_eq!(g.image.as_deref(), Some("/word/media/image1.png"));
    assert!(g.lock_aspect);
    assert_eq!(back.design.background, doc.design.background);
}

#[test]
fn new_document_gets_a_fill_and_removing_it_leaves_the_colour() {
    let mut doc = read_docx(&sample(r#"<v:fill type="gradient" focus="100%"/>"#)).unwrap();
    doc.design.background_fill = None;
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    assert!(xml.contains("<w:background w:color=\"2070C0\"/>"), "{xml}");
    assert!(!xml.contains("v:background"));
    // A document created here: the fill with its picture as a new media part.
    let mut doc = bo_model::Document::default();
    doc.sections.push(bo_model::Section::default());
    doc.media
        .insert("/word/media/fill1.png".into(), Arc::new(PNG.to_vec()));
    doc.design.background = Color::from_hex("FFFFFF");
    doc.design.background_fill = Some(PageFill {
        kind: PageFillKind::Pattern,
        color2: Color::from_hex("E0F0FF"),
        image: Some("/word/media/fill1.png".into()),
        title: Some("5%".into()),
        ..PageFill::default()
    });
    let out = write_docx(&doc).unwrap();
    let back = read_docx(&out).unwrap();
    let f = back.design.background_fill.unwrap();
    assert_eq!(f.kind, PageFillKind::Pattern);
    assert_eq!(f.title.as_deref(), Some("5%"));
    assert!(back.media.contains_key(f.image.as_deref().unwrap()));
}

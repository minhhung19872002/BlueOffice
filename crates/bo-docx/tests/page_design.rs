//! Page borders (ECMA-376 Part 1, §17.6.10 pgBorders), page colour (§17.2.1
//! background, §17.15.1.32 displayBackgroundShape), VML watermarks in headers
//! (Part 4, §19.1.2.23 textpath, §19.1.2.11 imagedata) and an authored theme part:
//! reading, lossless writing and writing after edits.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::{Block, Color, Document, PageBorderDisplay, RunContent};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const V: &str = "urn:schemas-microsoft-com:vml";
const O: &str = "urn:schemas-microsoft-com:office:office";
const W10: &str = "urn:schemas-microsoft-com:office:word";

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

/// Package with a background, page borders, a settings part and two headers: a
/// text watermark (default) and a picture watermark (first page).
fn sample() -> Vec<u8> {
    let mut p = Package::new();
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:r="{R}"><w:background w:color="FFF2CC"/><w:body><w:p><w:r><w:t>Nội dung</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId10"/><w:headerReference w:type="first" r:id="rId11"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:pgBorders w:zOrder="back" w:display="notFirstPage" w:offsetFrom="page"><w:top w:val="basicBlackDots" w:sz="12" w:space="24" w:color="1F6FB2"/><w:left w:val="double" w:sz="6" w:space="20" w:color="auto" w:shadow="1"/><w:bottom w:val="apples" w:sz="14" w:space="24" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="20" w:color="FF0000"/></w:pgBorders><w:titlePg/></w:sectPr></w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="{W}"><w:zoom w:percent="100"/><w:displayBackgroundShape/><w:defaultTabStop w:val="720"/></w:settings>"#
    );
    p.add_part(
        "/word/settings.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml",
        settings.into_bytes(),
    )
    .unwrap();
    let hdr = |inner: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="{W}" xmlns:r="{R}" xmlns:v="{V}" xmlns:o="{O}" xmlns:w10="{W10}"><w:p><w:r>{inner}</w:r><w:r><w:t>Tiêu đề trang</w:t></w:r></w:p></w:hdr>"#
        )
    };
    let text = r##"<w:pict><v:shape id="SampleWaterMarkText" style="position:absolute;margin-left:0;margin-top:0;width:400pt;height:100pt;rotation:315;z-index:-1;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" fillcolor="#c0c0c0" stroked="f"><v:fill opacity=".5"/><v:textpath style="font-family:&quot;Cambria&quot;;font-size:1pt;font-weight:bold" string="MẬT"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape></w:pict>"##;
    let pic = r#"<w:pict><v:shape id="SamplePictureWatermark" style="position:absolute;margin-left:0;margin-top:0;width:200pt;height:150pt;z-index:-2;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" stroked="f"><v:imagedata r:id="rId1" o:title="" gain="16384f" blacklevel="26214f"/></v:shape></w:pict>"#;
    let ct_hdr = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
    p.add_part("/word/header1.xml", ct_hdr, hdr(text).into_bytes())
        .unwrap();
    p.add_part("/word/header2.xml", ct_hdr, hdr(pic).into_bytes())
        .unwrap();
    p.add_part("/word/media/image1.png", "image/png", PNG.to_vec())
        .unwrap();
    let img = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
    p.set_relationships("/word/header2.xml", &[rel("rId1", img, "media/image1.png")]);
    let hr = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
    p.set_relationships(
        "/word/document.xml",
        &[
            rel(
                "rId1",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings",
                "settings.xml",
            ),
            rel("rId10", hr, "header1.xml"),
            rel("rId11", hr, "header2.xml"),
        ],
    );
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "word/document.xml")],
    );
    p.write().unwrap()
}

fn header_image(doc: &Document, first: bool) -> bo_model::Image {
    let h = &doc.sections[0].headers;
    let blocks = if first { &h.first } else { &h.default };
    let Some(Block::Paragraph(p)) = blocks.as_ref().unwrap().first() else {
        panic!("header paragraph")
    };
    p.runs
        .iter()
        .flat_map(|r| &r.content)
        .find_map(|c| match c {
            RunContent::Image(i) => Some((**i).clone()),
            _ => None,
        })
        .unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

#[test]
fn reads_page_borders_background_and_watermarks() {
    let doc = read_docx(&sample()).unwrap();
    assert_eq!(doc.design.background, Color::from_hex("FFF2CC"));
    assert!(doc.design.display_background_shape);
    let pb = doc.sections[0].page_borders.as_ref().unwrap();
    assert!(pb.behind && pb.offset_from_page);
    assert_eq!(pb.display, PageBorderDisplay::NotFirstPage);
    let top = pb.top.as_ref().unwrap();
    assert_eq!(
        (top.line.style.as_str(), top.line.size, top.line.space),
        ("basicBlackDots", 12, 24)
    );
    assert!(top.line.is_art());
    assert_eq!(top.line.color, Color::from_hex("1F6FB2"));
    let left = pb.left.as_ref().unwrap();
    assert!(left.shadow && !left.line.is_art());
    assert_eq!(left.line.color, None);
    assert_eq!(pb.bottom.as_ref().unwrap().line.style, "apples");
    // Text watermark: a VML text path, 400 × 100 pt, rotated 315°, centred on the
    // margins behind the text, half transparent.
    let t = header_image(&doc, false);
    let s = t.shape.as_ref().unwrap();
    let art = s.art.as_ref().unwrap();
    let tp = art.text.as_ref().unwrap();
    assert_eq!(
        (tp.text.as_str(), tp.font.as_str(), tp.bold),
        ("MẬT", "Cambria", true)
    );
    assert_eq!(art.opacity, 500);
    assert!(art.watermark);
    assert_eq!(s.fill, Color::from_hex("C0C0C0"));
    assert!(s.line.is_none());
    assert_eq!((t.width.0, t.height.0), (400 * 12_700, 100 * 12_700));
    assert_eq!(t.rotation, 315 * 60_000);
    let a = t.anchor.as_ref().unwrap();
    assert!(a.behind);
    assert_eq!(a.h_align.as_deref(), Some("center"));
    assert_eq!(a.v_relative, "margin");
    // Picture watermark: the media part, washed out (gain 0.25, black level 0.4 in 16.16 fixed point).
    let p = header_image(&doc, true);
    let art = p.shape.as_ref().unwrap().art.as_ref().unwrap();
    assert_eq!(art.picture.as_deref(), Some("/word/media/image1.png"));
    assert!(art.washout && art.watermark);
    assert!(doc.media.contains_key("/word/media/image1.png"));
}

#[test]
fn unchanged_document_keeps_its_parts() {
    let src = sample();
    let doc = read_docx(&src).unwrap();
    let out = write_docx(&doc).unwrap();
    assert_eq!(
        part(&out, "/word/document.xml"),
        part(&src, "/word/document.xml")
    );
    assert_eq!(
        part(&out, "/word/settings.xml"),
        part(&src, "/word/settings.xml")
    );
    let back = read_docx(&out).unwrap();
    assert_eq!(back.sections[0].page_borders, doc.sections[0].page_borders);
    assert_eq!(back.design, doc.design);
}

#[test]
fn edited_design_is_written_and_read_back() {
    let mut doc = read_docx(&sample()).unwrap();
    doc.design.background = Color::from_hex("E7EEF5");
    doc.design.display_background_shape = false;
    let pb = doc.sections[0].page_borders.as_mut().unwrap();
    pb.display = PageBorderDisplay::FirstPage;
    pb.behind = false;
    pb.right = None;
    // Edit the text watermark (forces the header to be regenerated).
    let h = doc.sections[0].headers.default.as_mut().unwrap();
    if let Some(Block::Paragraph(p)) = h.first_mut() {
        for c in p.runs.iter_mut().flat_map(|r| r.content.iter_mut()) {
            if let RunContent::Image(i) = c {
                let art = i.shape.as_mut().unwrap().art.as_mut().unwrap();
                art.text.as_mut().unwrap().text = "BẢN SAO".into();
            }
        }
    }
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    // §17.2.1 background is the first child of w:document.
    assert!(
        xml.contains("<w:background w:color=\"E7EEF5\"/><w:body>"),
        "{xml}"
    );
    assert_eq!(xml.matches("<w:background").count(), 1);
    // pgBorders after pgMar, before titlePg (§17.6.17), defaults not written.
    let at = |s: &str| xml.find(s).unwrap();
    assert!(at("<w:pgMar") < at("<w:pgBorders") && at("<w:pgBorders") < at("<w:titlePg"));
    assert!(xml.contains("<w:pgBorders w:display=\"firstPage\" w:offsetFrom=\"page\">"));
    assert!(!xml.contains("w:zOrder"));
    assert!(!part(&out, "/word/settings.xml").contains("displayBackgroundShape"));
    let back = read_docx(&out).unwrap();
    assert_eq!(back.design.background, doc.design.background);
    assert_eq!(back.sections[0].page_borders, doc.sections[0].page_borders);
    // The edited watermark comes back as a watermark with the new text; the picture
    // watermark of the first-page header still resolves its picture.
    let t = header_image(&back, false);
    let art = t.shape.as_ref().unwrap().art.as_ref().unwrap();
    assert_eq!(art.text.as_ref().unwrap().text, "BẢN SAO");
    assert!(art.watermark);
    assert_eq!(art.opacity, 500);
    assert_eq!(t.rotation, 315 * 60_000);
    let p = header_image(&back, true);
    let pic = p.shape.as_ref().unwrap().art.as_ref().unwrap();
    assert_eq!(
        back.media
            .get(pic.picture.as_deref().unwrap())
            .map(|b| b.len()),
        Some(PNG.len())
    );
}

#[test]
fn new_document_with_design() {
    let mut doc = Document::default();
    doc.sections.push(bo_model::Section::default());
    doc.design.background = Color::from_hex("FDF6E3");
    doc.design.display_background_shape = true;
    let theme = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements/></a:theme>"#;
    doc.design.theme_xml = Some(Arc::from(theme));
    let out = write_docx(&doc).unwrap();
    assert!(part(&out, "/word/document.xml").contains("<w:background w:color=\"FDF6E3\"/>"));
    let settings = part(&out, "/word/settings.xml");
    let bg = settings.find("<w:displayBackgroundShape/>").unwrap();
    assert!(bg < settings.find("<w:compat>").unwrap());
    assert_eq!(part(&out, "/word/theme/theme1.xml"), theme);
    let back = read_docx(&out).unwrap();
    assert_eq!(back.design.background, doc.design.background);
    assert!(back.design.display_background_shape);
}

//! BlueDoc round 14: Collapsed by default ([MS-DOCX] 2.6.1.1 w15:collapsed in pPr, the
//! reference writes it after outlineLvl) and the default tab stop interval (§17.15.1.25
//! defaultTabStop) patched into a read document's settings in CT_Settings order.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section, Twips};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn first_props(doc: &Document) -> &ParaProps {
    match &doc.sections[0].blocks[0] {
        Block::Paragraph(p) => &p.props,
        _ => panic!("not a paragraph"),
    }
}

/// A document whose heading is collapsed by default and whose settings hold a 0.4" default
/// tab stop before the hyphenation settings (as the reference saves them).
fn sample() -> Vec<u8> {
    let mut p = Package::new();
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:w15="{W15}"><w:body><w:p><w:pPr><w:outlineLvl w:val="0"/><w15:collapsed/></w:pPr><w:r><w:t>Head</w:t></w:r></w:p><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="{W}"><w:zoom w:percent="100"/><w:defaultTabStop w:val="576"/><w:autoHyphenation/><w:characterSpacingControl w:val="doNotCompress"/></w:settings>"#
    );
    p.add_part(
        "/word/settings.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml",
        settings.into_bytes(),
    )
    .unwrap();
    p.set_relationships(
        "/word/document.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type:
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings"
                    .into(),
            target: "settings.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.set_relationships(
        "",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.write().unwrap()
}

#[test]
fn reads_collapsed_and_default_tab_stop() {
    let doc = read_docx(&sample()).unwrap();
    assert_eq!(first_props(&doc).collapsed, Some(true));
    assert_eq!(doc.default_tab_stop, Some(Twips(576)));
}

#[test]
fn collapsed_written_after_outline_level_with_its_namespace() {
    let mut doc = Document::default();
    let props = ParaProps {
        outline_level: Some(1),
        collapsed: Some(true),
        ..ParaProps::default()
    };
    doc.sections = vec![Section {
        blocks: vec![Block::Paragraph(Paragraph::new(
            props,
            vec![Run::text("Mục", RunProps::default())],
        ))],
        ..Section::default()
    }];
    let bytes = write_docx(&doc).unwrap();
    // BO_ROUND14_OUT=<file.docx> keeps the output for OpenXmlValidator (bo-cli validate).
    if let Ok(out) = std::env::var("BO_ROUND14_OUT") {
        std::fs::write(out, &bytes).unwrap();
    }
    let main = part(&bytes, "/word/document.xml");
    let lvl = main.find("<w:outlineLvl").unwrap();
    let c = main.find("<w15:collapsed").unwrap();
    assert!(lvl < c, "{main}");
    assert!(main.contains(&format!(r#"<w15:collapsed xmlns:w15="{W15}"/>"#)));
    let back = read_docx(&bytes).unwrap();
    assert_eq!(first_props(&back).collapsed, Some(true));
}

#[test]
fn default_tab_stop_patched_before_hyphenation() {
    let mut doc = read_docx(&sample()).unwrap();
    doc.default_tab_stop = Some(Twips(851));
    let bytes = write_docx(&doc).unwrap();
    let settings = part(&bytes, "/word/settings.xml");
    assert_eq!(
        settings.matches("<w:defaultTabStop").count(),
        1,
        "{settings}"
    );
    let t = settings.find(r#"<w:defaultTabStop w:val="851"/>"#).unwrap();
    assert!(settings.find("<w:zoom").unwrap() < t);
    assert!(
        t < settings.find("<w:autoHyphenation/>").unwrap(),
        "{settings}"
    );
    assert_eq!(
        read_docx(&bytes).unwrap().default_tab_stop,
        Some(Twips(851))
    );
    // Back to the default: the element goes (absent = 720).
    doc.default_tab_stop = None;
    let settings = part(&write_docx(&doc).unwrap(), "/word/settings.xml");
    assert!(!settings.contains("<w:defaultTabStop"), "{settings}");
    // Collapsed kept on the unchanged heading.
    assert!(part(&write_docx(&doc).unwrap(), "/word/document.xml").contains("collapsed"));
}

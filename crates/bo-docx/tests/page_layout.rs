//! Page layout settings: drop caps (ECMA-376 Part 1, §17.3.1.11 framePr), columns
//! (§17.6.4 cols, §17.6.3 col), line numbering (§17.6.8 lnNumType, §17.3.1.35
//! suppressLineNumbers), hyphenation (§17.15.1.10 / .21 / .40 / .52, §17.3.1.34
//! suppressAutoHyphens) and section text direction (§17.6.20): reading, writing a
//! new document, and patching the settings of a read one.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::{
    Block, ColumnDef, Columns, Document, DropCap, FrameProps, Hyphenation, LineNumberRestart,
    LineNumbering, ParaProps, Paragraph, Run, RunProps, Section, TextDirection, Twips,
};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn para(props: ParaProps, text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        props,
        vec![Run::text(text, RunProps::default())],
    ))
}

fn sample() -> Vec<u8> {
    sample_with(
        r#"<w:lnNumType w:countBy="5" w:start="9" w:distance="283" w:restart="newSection"/>"#,
    )
}

/// The sample with another first-section `w:lnNumType`.
fn sample_with(ln_num: &str) -> Vec<u8> {
    let mut p = Package::new();
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}"><w:body><w:p><w:pPr><w:keepNext/><w:framePr w:dropCap="drop" w:lines="3" w:hSpace="113" w:wrap="around" w:vAnchor="text" w:hAnchor="text"/></w:pPr><w:r><w:t>C</w:t></w:r></w:p><w:p><w:pPr><w:suppressLineNumbers/><w:suppressAutoHyphens/></w:pPr><w:r><w:t>hữ hoa thả.</w:t></w:r></w:p><w:p><w:pPr><w:framePr w:w="2000" w:h="400" w:hRule="exact" w:wrap="around"/></w:pPr><w:r><w:t>Khung</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/>{ln_num}<w:cols w:num="2" w:space="567" w:sep="1" w:equalWidth="0"><w:col w:w="3000" w:space="567"/><w:col w:w="5788"/></w:cols></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>Dọc</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="720" w:sep="true"/><w:textDirection w:val="tbRl"/></w:sectPr></w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="{W}"><w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:autoHyphenation/><w:consecutiveHyphenLimit w:val="2"/><w:hyphenationZone w:val="425"/><w:doNotHyphenateCaps/><w:characterSpacingControl w:val="doNotCompress"/></w:settings>"#
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

fn props(doc: &Document, i: usize) -> &ParaProps {
    let blocks: Vec<&Block> = doc.sections.iter().flat_map(|s| &s.blocks).collect();
    match blocks[i] {
        Block::Paragraph(p) => &p.props,
        _ => panic!("not a paragraph"),
    }
}

#[test]
fn reads_layout_settings() {
    let doc = read_docx(&sample()).unwrap();
    assert_eq!(
        props(&doc, 0).frame,
        Some(FrameProps {
            drop_cap: DropCap::Drop,
            lines: 3,
            h_space: Twips(113),
        })
    );
    assert_eq!(props(&doc, 1).suppress_line_numbers, Some(true));
    assert_eq!(props(&doc, 1).suppress_auto_hyphens, Some(true));
    // A frame that is not a drop cap stays verbatim.
    assert_eq!(props(&doc, 2).frame, None);
    assert!(props(&doc, 2)
        .unknown
        .iter()
        .any(|r| r.xml.contains("w:framePr")));
    let s0 = &doc.sections[0];
    assert_eq!(
        s0.line_numbers,
        Some(LineNumbering {
            count_by: 5,
            start: 9,
            distance: Some(Twips(283)),
            restart: LineNumberRestart::NewSection,
        })
    );
    assert!(s0.columns.separator);
    assert_eq!(
        s0.columns.cols,
        [
            ColumnDef {
                width: Twips(3000),
                space: Twips(567)
            },
            ColumnDef {
                width: Twips(5788),
                space: Twips(0)
            }
        ]
    );
    let s1 = &doc.sections[1];
    assert!(s1.columns.separator && !s1.columns.unequal());
    assert_eq!(s1.text_direction, Some(TextDirection::TbRl));
    assert_eq!(s0.text_direction, None);
    assert_eq!(
        doc.hyphenation,
        Hyphenation {
            auto: true,
            zone: Some(Twips(425)),
            consecutive_limit: Some(2),
            no_caps: true,
        }
    );
}

/// [MS-OI29500] 2.1.215 (Part 1 §17.6.8 lnNumType): `start` within 0..=32767 (numbers
/// skipped, the first one is start + 1), `countBy` within 1..=100; absent start = 0.
#[test]
fn line_numbering_values_are_kept_in_range() {
    let ln = |xml: &str| read_docx(&sample_with(xml)).unwrap().sections[0].line_numbers;
    let got = ln(r#"<w:lnNumType w:countBy="500" w:start="40000"/>"#).unwrap();
    assert_eq!((got.count_by, got.start), (100, 32_767));
    let got = ln(r#"<w:lnNumType w:countBy="0" w:start="-2"/>"#).unwrap();
    assert_eq!((got.count_by, got.start), (1, 0));
    let got = ln(r#"<w:lnNumType w:countBy="1"/>"#).unwrap();
    assert_eq!(got.start, 0);
    // Written back as read: start 3 stays 3 (shown as 4).
    let mut doc = read_docx(&sample_with(r#"<w:lnNumType w:countBy="1" w:start="3"/>"#)).unwrap();
    doc.sections[0]
        .blocks
        .push(para(ParaProps::default(), "mới"));
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    assert!(xml.contains(r#"w:start="3""#), "{xml}");
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
}

#[test]
fn edited_settings_are_patched_in_schema_order() {
    let mut doc = read_docx(&sample()).unwrap();
    doc.hyphenation = Hyphenation {
        auto: true,
        zone: Some(Twips(360)),
        consecutive_limit: None,
        no_caps: false,
    };
    let out = write_docx(&doc).unwrap();
    let s = part(&out, "/word/settings.xml");
    assert!(s.contains(
        r#"<w:defaultTabStop w:val="720"/><w:autoHyphenation/><w:hyphenationZone w:val="360"/><w:characterSpacingControl"#
    ), "{s}");
    assert_eq!(read_docx(&out).unwrap().hyphenation, doc.hyphenation);
    // Hyphenation off: the elements go.
    doc.hyphenation = Hyphenation::default();
    let s = part(&write_docx(&doc).unwrap(), "/word/settings.xml");
    assert!(!s.contains("Hyphen"), "{s}");
}

#[test]
fn edited_sections_and_paragraphs_round_trip() {
    let mut doc = read_docx(&sample()).unwrap();
    let s0 = &mut doc.sections[0];
    s0.line_numbers = Some(LineNumbering {
        count_by: 1,
        start: 0,
        distance: None,
        restart: LineNumberRestart::Continuous,
    });
    s0.columns.cols[0].width = Twips(2500);
    doc.sections[1].text_direction = Some(TextDirection::BtLr);
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.props.frame = Some(FrameProps {
            drop_cap: DropCap::Margin,
            lines: 2,
            h_space: Twips(0),
        });
    }
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    assert!(
        xml.contains(r#"<w:lnNumType w:countBy="1" w:restart="continuous"/>"#),
        "{xml}"
    );
    assert!(xml.contains(r#"<w:cols w:num="2" w:space="567" w:sep="1" w:equalWidth="0"><w:col w:w="2500" w:space="567"/><w:col w:w="5788"/></w:cols>"#), "{xml}");
    assert!(xml.contains(r#"<w:textDirection w:val="btLr"/>"#), "{xml}");
    assert!(
        xml.contains(r#"<w:framePr w:dropCap="margin" w:lines="2""#),
        "{xml}"
    );
    let back = read_docx(&out).unwrap();
    for (a, b) in back.sections.iter().zip(&doc.sections) {
        assert_eq!(a.line_numbers, b.line_numbers);
        assert_eq!(a.columns, b.columns);
        assert_eq!(a.text_direction, b.text_direction);
    }
    assert_eq!(props(&back, 0).frame, props(&doc, 0).frame);
}

#[test]
fn new_document_writes_everything() {
    let mut s0 = Section::new(vec![
        para(
            ParaProps {
                frame: Some(FrameProps {
                    drop_cap: DropCap::Drop,
                    lines: 3,
                    h_space: Twips(0),
                }),
                ..ParaProps::default()
            },
            "A",
        ),
        para(
            ParaProps {
                suppress_line_numbers: Some(true),
                suppress_auto_hyphens: Some(false),
                ..ParaProps::default()
            },
            "nh sáng",
        ),
    ]);
    s0.columns = Columns {
        separator: true,
        ..Columns::equal(3, Twips(360))
    };
    s0.line_numbers = Some(LineNumbering::default());
    let mut s1 = Section::new(vec![para(ParaProps::default(), "Dọc")]);
    s1.text_direction = Some(TextDirection::TbRl);
    let doc = Document {
        sections: vec![s0, s1],
        hyphenation: Hyphenation {
            auto: true,
            zone: None,
            consecutive_limit: Some(3),
            no_caps: true,
        },
        ..Document::default()
    };
    let out = write_docx(&doc).unwrap();
    let s = part(&out, "/word/settings.xml");
    assert!(
        s.contains(
            r#"<w:autoHyphenation/><w:consecutiveHyphenLimit w:val="3"/><w:doNotHyphenateCaps/>"#
        ),
        "{s}"
    );
    let back = read_docx(&out).unwrap();
    assert_eq!(back.hyphenation, doc.hyphenation);
    assert_eq!(back.sections[0].columns, doc.sections[0].columns);
    assert_eq!(back.sections[0].line_numbers, doc.sections[0].line_numbers);
    assert_eq!(back.sections[1].text_direction, Some(TextDirection::TbRl));
    assert_eq!(props(&back, 0).frame, props(&doc, 0).frame);
    assert_eq!(props(&back, 1).suppress_line_numbers, Some(true));
    assert_eq!(props(&back, 1).suppress_auto_hyphens, Some(false));
}

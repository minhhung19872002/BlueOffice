//! Reader tests on hand-written WordprocessingML.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, CT_DOCUMENT};
use bo_model::{Alignment, Block, LineSpacing, RunContent, Twips, Underline};
use bo_opc::{Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

fn docx(body: &str) -> Vec<u8> {
    let mut p = Package::new();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w:body>{body}</w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, xml.into_bytes())
        .unwrap();
    p.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "/word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.write().unwrap()
}

#[test]
fn paragraph_and_run_properties() {
    let body = r#"<w:p w14:paraId="1"><w:pPr><w:jc w:val="both"/><w:ind w:left="720" w:hanging="360"/><w:spacing w:before="240" w:after="0" w:line="276" w:lineRule="auto"/></w:pPr>
<w:r><w:rPr><w:rFonts w:ascii="Arial"/><w:b/><w:i w:val="0"/><w:u w:val="double"/><w:sz w:val="32"/><w:color w:val="FF0000"/></w:rPr><w:t>Xin chào</w:t><w:tab/><w:t xml:space="preserve"> thế giới &amp; bạn</w:t></w:r>
<w:del><w:r><w:delText>gone</w:delText></w:r></w:del>
<w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p>
<w:sectPr><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>"#;
    let doc = read_docx(&docx(body)).unwrap();
    assert_eq!(doc.sections.len(), 1);
    let sec = &doc.sections[0];
    assert_eq!(sec.page.width, Twips(16_838));
    assert_eq!(sec.page.margin_left, Twips(1440));
    let Block::Paragraph(p) = &sec.blocks[0] else {
        panic!("expected paragraph")
    };
    assert_eq!(p.props.align, Some(Alignment::Justify));
    assert_eq!(p.props.indent_left, Some(Twips(720)));
    assert_eq!(p.props.indent_first_line, Some(Twips(-360)));
    assert_eq!(p.props.space_before, Some(Twips(240)));
    assert_eq!(p.props.line_spacing, Some(LineSpacing::Auto(276)));
    let r = &p.runs[0];
    assert_eq!(r.props.font.as_deref(), Some("Arial"));
    assert_eq!(r.props.bold, Some(true));
    assert_eq!(r.props.italic, Some(false));
    assert_eq!(r.props.underline, Some(Underline::Double));
    assert_eq!(r.props.size.unwrap().to_pt(), 16.0);
    assert_eq!(r.content[1], RunContent::Tab);
    assert_eq!(p.plain_text(), "Xin chào\t thế giới & bạnlink");
}

#[test]
fn section_break_in_paragraph() {
    let body = r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:pPr><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>B</w:t></w:r></w:p><w:sectPr/>"#;
    let doc = read_docx(&docx(body)).unwrap();
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(doc.sections[0].page.width, Twips(12_240));
    assert_eq!(doc.plain_text(), "A\nB");
}

#[test]
fn unknown_elements_are_ignored() {
    let body = r"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:drawing><x/></w:drawing><w:t>after</w:t></w:r></w:p>";
    let doc = read_docx(&docx(body)).unwrap();
    // The unknown drawing is kept as an object placeholder (U+FFFC).
    assert_eq!(doc.plain_text(), "cell\n\u{FFFC}after");
}

#[test]
fn not_a_docx() {
    assert!(read_docx(b"PK garbage").is_err());
    let mut p = Package::new();
    p.put_part("/a.xml", b"<a/>".to_vec());
    assert!(read_docx(&p.write().unwrap()).is_err());
}

#[test]
fn content_controls_inline_and_block() {
    use bo_model::ControlKind;
    let body = r#"<w:p><w:sdt><w:sdtPr><w:alias w:val="Tên"/><w:id w:val="5"/><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>Nhập tên</w:t></w:r></w:sdtContent></w:sdt><w:sdt><w:sdtPr><w14:checkbox><w14:checked w14:val="1"/><w14:checkedState w14:val="2612"/><w14:uncheckedState w14:val="2610"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>☒</w:t></w:r></w:sdtContent></w:sdt></w:p>
<w:sdt><w:sdtPr><w:tag w:val="k"/><w:lock w:val="sdtContentLocked"/><w:dropDownList><w:listItem w:displayText="A" w:value="a"/></w:dropDownList></w:sdtPr><w:sdtContent><w:p><w:r><w:t>một</w:t></w:r></w:p><w:p><w:r><w:t>hai</w:t></w:r></w:p></w:sdtContent></w:sdt>"#;
    let doc = read_docx(&docx(body)).unwrap();
    let blocks = &doc.sections[0].blocks;
    assert_eq!(blocks.len(), 3);
    let Block::Paragraph(p) = &blocks[0] else {
        panic!("paragraph")
    };
    assert_eq!(p.plain_text(), "Nhập tên☒");
    let starts: Vec<_> = p
        .runs
        .iter()
        .flat_map(|r| &r.content)
        .filter_map(|c| match c {
            RunContent::ControlStart(cc) => Some(cc),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0].alias.as_deref(), Some("Tên"));
    assert_eq!(starts[0].id, Some(5));
    assert!(starts[0].placeholder);
    assert_eq!(starts[0].kind, ControlKind::Text { multi_line: false });
    assert!(matches!(
        starts[1].kind,
        ControlKind::Checkbox {
            checked: true,
            on: '\u{2612}',
            off: '\u{2610}'
        }
    ));
    // A block control's markers sit at the start of its first and the end of its last paragraph.
    let (Block::Paragraph(a), Block::Paragraph(b)) = (&blocks[1], &blocks[2]) else {
        panic!("paragraphs")
    };
    let Some(RunContent::ControlStart(cc)) = a.runs[0].content.first() else {
        panic!("start marker")
    };
    assert!(cc.block);
    assert_eq!(cc.tag.as_deref(), Some("k"));
    assert_eq!(cc.lock.as_deref(), Some("sdtContentLocked"));
    assert_eq!(
        cc.kind,
        ControlKind::DropDown(vec![("A".into(), "a".into())])
    );
    assert!(matches!(
        b.runs.last().and_then(|r| r.content.last()),
        Some(RunContent::ControlEnd(true))
    ));
    // Written back as one sdt around both paragraphs, its properties as read.
    let out = bo_docx::write_docx(&doc).unwrap();
    let again = read_docx(&out).unwrap();
    assert_eq!(again.sections[0].blocks.len(), 3);
    let pkg = Package::read(&out, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert_eq!(xml.matches("<w:sdt>").count(), 3);
    assert!(xml.contains(r#"<w:lock w:val="sdtContentLocked"/>"#));
}

/// ECMA-376 Part 1, §17.5.2.32 sdt around table rows ([MS-DOCX] w15:repeatingSection of
/// w15:repeatingSectionItem rows): the controls sit on the rows and a changed table writes them
/// back around the same rows.
#[test]
fn repeating_section_of_table_rows_round_trips() {
    use bo_model::ControlKind;
    let tr = |t: &str| format!("<w:tr><w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc></w:tr>");
    let item = |id: u32, t: &str| {
        format!(
            r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>"#,
            tr(t)
        )
    };
    let body = format!(
        r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>{}<w:sdt><w:sdtPr><w:alias w:val="Thành viên"/><w:id w:val="10"/><w15:repeatingSection/></w:sdtPr><w:sdtContent>{}{}</w:sdtContent></w:sdt>{}</w:tbl><w:p/>"#,
        tr("Đầu"),
        item(11, "An"),
        item(12, "Bình"),
        tr("Cuối")
    );
    let doc = read_docx(&docx(&body)).unwrap();
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("table")
    };
    assert_eq!(t.rows.len(), 4);
    let kinds: Vec<Vec<ControlKind>> = t
        .rows
        .iter()
        .map(|r| r.props.controls.iter().map(|c| c.kind.clone()).collect())
        .collect();
    assert_eq!(
        kinds,
        vec![
            vec![],
            vec![
                ControlKind::RepeatingSection,
                ControlKind::RepeatingSectionItem
            ],
            vec![ControlKind::RepeatingSectionItem],
            vec![],
        ]
    );
    let ends: Vec<u8> = t.rows.iter().map(|r| r.props.controls_end).collect();
    assert_eq!(ends, [0, 1, 2, 0]);
    assert_eq!(
        t.rows[1].props.controls[0].alias.as_deref(),
        Some("Thành viên")
    );

    // A changed table (the source XML no longer stands for it) writes the controls again.
    let mut changed = doc.clone();
    if let Block::Table(t) = &mut changed.sections[0].blocks[0] {
        t.src = None;
        t.grid[0] = Twips(2400);
    }
    let out = bo_docx::write_docx(&changed).unwrap();
    let pkg = Package::read(&out, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert_eq!(xml.matches("<w:sdt>").count(), 3);
    let at = |s: &str| xml.find(s).unwrap();
    assert!(at("Đầu") < at("<w15:repeatingSection/>"));
    assert!(at("<w15:repeatingSection/>") < at(">An<"));
    assert!(at(">Bình<") < at("Cuối"));
    // `</w:sdtContent></w:sdt>` twice after Bình's row: its item and the section.
    let tail = &xml[at(">Bình<")..at("Cuối")];
    assert_eq!(tail.matches("</w:sdtContent></w:sdt>").count(), 2);
    let again = read_docx(&out).unwrap();
    let Block::Table(t2) = &again.sections[0].blocks[0] else {
        panic!("table")
    };
    let ends: Vec<u8> = t2.rows.iter().map(|r| r.props.controls_end).collect();
    assert_eq!(ends, [0, 1, 2, 0]);
}

/// ECMA-376 Part 1, §17.4.58 tblpPr and §17.4.72 textDirection are read into the model,
/// survive a write of a changed table, and strict values (`lr`) read like transitional.
#[test]
fn floating_table_and_vertical_text_round_trip() {
    let body = r#"<w:tbl><w:tblPr><w:tblpPr w:leftFromText="180" w:rightFromText="200" w:topFromText="10" w:bottomFromText="20" w:vertAnchor="page" w:horzAnchor="margin" w:tblpXSpec="center" w:tblpY="1440"/><w:tblW w:w="4000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:textDirection w:val="btLr"/></w:tcPr><w:p/></w:tc><w:tc><w:tcPr><w:textDirection w:val="lr"/></w:tcPr><w:p/></w:tc></w:tr></w:tbl><w:p/>"#;
    let doc = read_docx(&docx(body)).unwrap();
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("table")
    };
    let f = t.props.float.clone().unwrap();
    assert_eq!(
        (
            f.left_from_text,
            f.right_from_text,
            f.top_from_text,
            f.bottom_from_text
        ),
        (Twips(180), Twips(200), Twips(10), Twips(20))
    );
    assert_eq!(f.vert_anchor.as_deref(), Some("page"));
    assert_eq!(f.horz_anchor.as_deref(), Some("margin"));
    assert_eq!(f.x_align.as_deref(), Some("center"));
    assert_eq!((f.y, f.y_align.as_deref()), (Twips(1440), None));
    assert_eq!(t.props.unknown, Vec::new());
    let dirs: Vec<_> = t.rows[0]
        .cells
        .iter()
        .map(|c| c.props.text_direction)
        .collect();
    assert_eq!(
        dirs,
        vec![
            Some(bo_model::TextDirection::BtLr),
            Some(bo_model::TextDirection::BtLr)
        ]
    );
    // Change the table so it is written from the model, not from its source XML.
    let mut doc = doc;
    if let Block::Table(t) = &mut doc.sections[0].blocks[0] {
        t.rows[0].cells[1].props.text_direction = Some(bo_model::TextDirection::TbRl);
    }
    let out = bo_docx::write_docx(&doc).unwrap();
    let pkg = Package::read(&out, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert!(xml.contains(r#"<w:textDirection w:val="tbRl"/>"#), "{xml}");
    assert!(xml.contains(r#"w:tblpXSpec="center""#));
    let back = read_docx(&out).unwrap();
    let Block::Table(t2) = &back.sections[0].blocks[0] else {
        panic!("table")
    };
    assert_eq!(t2.props.float, Some(f));
    assert_eq!(
        t2.rows[0].cells[1].props.text_direction,
        Some(bo_model::TextDirection::TbRl)
    );
}

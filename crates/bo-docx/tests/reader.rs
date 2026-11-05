//! Reader tests on hand-written WordprocessingML.

use bo_docx::{read_docx, CT_DOCUMENT};
use bo_model::{Alignment, Block, LineSpacing, RunContent, Twips, Underline};
use bo_opc::{Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

fn docx(body: &str) -> Vec<u8> {
    let mut p = Package::new();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:body>{body}</w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, xml.into_bytes()).unwrap();
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
    let Block::Paragraph(p) = &sec.blocks[0];
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
    let body = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:drawing><x/></w:drawing><w:t>after</w:t></w:r></w:p>"#;
    let doc = read_docx(&docx(body)).unwrap();
    assert_eq!(doc.plain_text(), "after");
}

#[test]
fn not_a_docx() {
    assert!(read_docx(b"PK garbage").is_err());
    let mut p = Package::new();
    p.put_part("/a.xml", b"<a/>".to_vec());
    assert!(read_docx(&p.write().unwrap()).is_err());
}

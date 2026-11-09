//! Proofing language (§17.3.2.20 lang) and noProof (§17.3.2.21) of runs, paragraph marks, styles
//! and docDefaults (CORE-SPELL-005), on hand-written WordprocessingML.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT, CT_STYLES};
use bo_model::{Block, Document, Lang, Paragraph, RunProps, StyleKind};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

const STYLES: &str = r#"<w:docDefaults><w:rPrDefault><w:rPr><w:lang w:val="en-US" w:eastAsia="ja-JP" w:bidi="ar-SA"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Viet"><w:name w:val="Viet"/><w:basedOn w:val="Normal"/><w:rPr><w:lang w:val="vi-VN"/></w:rPr></w:style><w:style w:type="character" w:styleId="Code"><w:name w:val="Code"/><w:rPr><w:noProof/><w:lang w:val="en-GB"/></w:rPr></w:style>"#;

fn docx(body: &str) -> Vec<u8> {
    let mut p = Package::new();
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document {NS}><w:body>{body}</w:body></w:document>"#
    );
    let styles =
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:styles {NS}>{STYLES}</w:styles>"#);
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    p.add_part("/word/styles.xml", CT_STYLES, styles.into_bytes())
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
    p.set_relationships(
        "/word/document.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_STYLES.into(),
            target: "styles.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.write().unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

/// Run properties through the style hierarchy (§17.7.2): docDefaults, paragraph style (or the
/// default one), character style, direct formatting.
fn effective(doc: &Document, p: &Paragraph, direct: &RunProps) -> RunProps {
    let mut r = doc.default_run.clone();
    let style = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    if let Some(id) = style {
        r = doc.styles.resolve_paragraph(&id).1.inherit(&r);
    }
    if let Some(id) = &direct.style {
        r = doc.styles.resolve_character(id).inherit(&r);
    }
    direct.inherit(&r)
}

fn para(doc: &Document, i: usize) -> &Paragraph {
    match &doc.sections[0].blocks[i] {
        Block::Paragraph(p) => p,
        Block::Table(_) | Block::Opaque(_) => panic!("paragraph {i}"),
    }
}

const BODY: &str = r#"<w:p><w:pPr><w:pStyle w:val="Viet"/><w:rPr><w:noProof/><w:lang w:val="fr-FR"/></w:rPr></w:pPr><w:r><w:t>Xin chào </w:t></w:r><w:r><w:rPr><w:lang w:val="en-US" w:eastAsia="zh-CN"/></w:rPr><w:t>hello </w:t></w:r><w:r><w:rPr><w:rStyle w:val="Code"/></w:rPr><w:t>x_y</w:t></w:r><w:r><w:rPr><w:noProof w:val="0"/></w:rPr><w:t> z</w:t></w:r></w:p><w:p><w:pPr><w:rPr><w:b/><w:lang w:val="de-DE"/></w:rPr></w:pPr><w:r><w:t>plain</w:t></w:r></w:p>"#;

#[test]
fn lang_and_no_proof_are_modelled() {
    let doc = read_docx(&docx(BODY)).unwrap();
    // docDefaults: three languages.
    assert_eq!(
        doc.default_run.lang,
        Some(Lang {
            val: Some("en-US".into()),
            east_asia: Some("ja-JP".into()),
            bidi: Some("ar-SA".into()),
        })
    );
    let viet = doc.styles.styles.iter().find(|s| s.id == "Viet").unwrap();
    assert_eq!(viet.rpr.lang, Some(Lang::latin("vi-VN")));
    let code = doc.styles.styles.iter().find(|s| s.id == "Code").unwrap();
    assert_eq!(code.rpr.no_proof, Some(true));
    assert!(code.rpr.unknown.is_empty(), "{:?}", code.rpr.unknown);
    let p = para(&doc, 0);
    // Paragraph mark: modelled, nothing left verbatim.
    assert_eq!(p.props.mark_lang, Some(Lang::latin("fr-FR")));
    assert_eq!(p.props.mark_no_proof, Some(true));
    assert!(p.props.unknown.is_empty(), "{:?}", p.props.unknown);
    assert_eq!(p.runs[0].props.lang, None);
    assert_eq!(
        p.runs[1].props.lang,
        Some(Lang {
            val: Some("en-US".into()),
            east_asia: Some("zh-CN".into()),
            bidi: None,
        })
    );
    assert_eq!(p.runs[1].props.unknown, Vec::new());
    assert_eq!(p.runs[3].props.no_proof, Some(false));
    // A paragraph mark rPr with other children keeps them verbatim.
    let q = para(&doc, 1);
    assert_eq!(q.props.mark_lang, Some(Lang::latin("de-DE")));
    assert_eq!(q.props.unknown.len(), 1);
}

#[test]
fn languages_resolve_through_the_style_hierarchy() {
    let doc = read_docx(&docx(BODY)).unwrap();
    let p = para(&doc, 0);
    let eff = |i: usize| effective(&doc, p, &p.runs[i].props);
    // Paragraph style Viet over docDefaults: vi-VN for Latin text, East Asian and bidi inherited.
    assert_eq!(
        eff(0).lang,
        Some(Lang {
            val: Some("vi-VN".into()),
            east_asia: Some("ja-JP".into()),
            bidi: Some("ar-SA".into()),
        })
    );
    // Direct formatting wins attribute by attribute.
    let l1 = eff(1).lang.unwrap();
    assert_eq!(
        (
            l1.val.as_deref(),
            l1.east_asia.as_deref(),
            l1.bidi.as_deref()
        ),
        (Some("en-US"), Some("zh-CN"), Some("ar-SA"))
    );
    // Character style Code: en-GB and noProof.
    let e2 = eff(2);
    assert_eq!(e2.lang.unwrap().val.as_deref(), Some("en-GB"));
    assert_eq!(e2.no_proof, Some(true));
    // noProof off directly; language from the paragraph style.
    assert_eq!(eff(3).no_proof, Some(false));
    // The second paragraph has no style: Normal + docDefaults.
    let q = para(&doc, 1);
    let e = effective(&doc, q, &q.runs[0].props);
    assert_eq!(e.lang.unwrap().val.as_deref(), Some("en-US"));
}

#[test]
fn untouched_paragraphs_are_byte_stable_and_edits_are_written() {
    let bytes = docx(BODY);
    let doc = read_docx(&bytes).unwrap();
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    // Both paragraphs come back exactly as written.
    for piece in BODY.split("</w:p>").filter(|s| !s.is_empty()) {
        assert!(xml.contains(&format!("{piece}</w:p>")), "{piece}\n{xml}");
    }
    let styles = part(&out, "/word/styles.xml");
    assert!(styles.contains(r#"<w:lang w:val="en-US" w:eastAsia="ja-JP" w:bidi="ar-SA"/>"#));
    // Change the languages so the paragraphs are written from the model.
    let mut doc = doc;
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.runs[0].props.lang = Some(Lang::latin("vi-VN"));
        p.runs[0].props.no_proof = Some(true);
        p.props.mark_lang = Some(Lang::latin("en-GB"));
    }
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[1] {
        p.props.mark_no_proof = Some(true);
    }
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    // CT_RPrBase order: noProof before lang (§17.3.2.28).
    assert!(
        xml.contains(r#"<w:r><w:rPr><w:noProof/><w:lang w:val="vi-VN"/></w:rPr><w:t xml:space="preserve">Xin chào </w:t></w:r>"#)
            || xml.contains(r#"<w:r><w:rPr><w:noProof/><w:lang w:val="vi-VN"/></w:rPr><w:t>Xin chào </w:t></w:r>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<w:rPr><w:noProof/><w:lang w:val="en-GB"/></w:rPr></w:pPr>"#),
        "{xml}"
    );
    // The kept rPr child (bold) stays, noProof goes after it and lang keeps its place.
    assert!(
        xml.contains(r#"<w:rPr><w:b/><w:noProof/><w:lang w:val="de-DE"/></w:rPr>"#),
        "{xml}"
    );
    // And reads back the same.
    let back = read_docx(&out).unwrap();
    let p = para(&back, 0);
    assert_eq!(p.runs[0].props.lang, Some(Lang::latin("vi-VN")));
    assert_eq!(p.runs[0].props.no_proof, Some(true));
    assert_eq!(p.props.mark_lang, Some(Lang::latin("en-GB")));
    assert_eq!(para(&back, 1).props.mark_no_proof, Some(true));
}

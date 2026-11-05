//! Minimal `bo_model::Document` → `.docx` writer.
//!
//! Phase 1 uses this to generate corpus/test documents. It writes only the
//! properties the model knows; full lossless writing comes in Phase 2.

use crate::{Result, CT_DOCUMENT, CT_STYLES};
use bo_model::{
    Alignment, Block, Document, LineSpacing, PageSetup, ParaProps, RunContent, RunProps, Underline,
};
use bo_opc::xml::escape;
use bo_opc::{Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use std::fmt::Write as _;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Serialize a document to `.docx` bytes.
pub fn write_docx(doc: &Document) -> Result<Vec<u8>> {
    let mut pkg = Package::new();
    pkg.add_part(
        "/word/document.xml",
        CT_DOCUMENT,
        document_xml(doc).into_bytes(),
    )?;
    pkg.add_part("/word/styles.xml", CT_STYLES, styles_xml(doc).into_bytes())?;
    pkg.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    pkg.set_relationships(
        "/word/document.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_STYLES.into(),
            target: "styles.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    Ok(pkg.write()?)
}

fn styles_xml(doc: &Document) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W_NS}\"><w:docDefaults><w:rPrDefault><w:rPr>{}</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{}</w:pPr></w:pPrDefault></w:docDefaults></w:styles>",
        rpr(&doc.default_run),
        ppr(&doc.default_para)
    )
}

fn document_xml(doc: &Document) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{W_NS}\"><w:body>"
    );
    let n = doc.sections.len();
    for (si, sec) in doc.sections.iter().enumerate() {
        let last = si + 1 == n;
        let count = sec.blocks.len();
        for (bi, b) in sec.blocks.iter().enumerate() {
            let Block::Paragraph(p) = b;
            s.push_str("<w:p>");
            let mut props = ppr(&p.props);
            // Non-final sections store sectPr in the last paragraph (§17.6.17).
            if !last && bi + 1 == count {
                props.push_str(&sect_pr(&sec.page));
            }
            if !props.is_empty() {
                let _ = write!(s, "<w:pPr>{props}</w:pPr>");
            }
            for r in &p.runs {
                s.push_str("<w:r>");
                let rp = rpr(&r.props);
                if !rp.is_empty() {
                    let _ = write!(s, "<w:rPr>{rp}</w:rPr>");
                }
                for c in &r.content {
                    match c {
                        RunContent::Text(t) => {
                            let _ = write!(s, "<w:t xml:space=\"preserve\">{}</w:t>", escape(t));
                        }
                        RunContent::Tab => s.push_str("<w:tab/>"),
                        RunContent::LineBreak => s.push_str("<w:br/>"),
                        RunContent::PageBreak => s.push_str("<w:br w:type=\"page\"/>"),
                    }
                }
                s.push_str("</w:r>");
            }
            s.push_str("</w:p>");
        }
        if last {
            s.push_str(&sect_pr(&sec.page));
        }
    }
    s.push_str("</w:body></w:document>");
    s
}

fn sect_pr(p: &PageSetup) -> String {
    format!(
        "<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr>",
        p.width.0, p.height.0, p.margin_top.0, p.margin_right.0, p.margin_bottom.0, p.margin_left.0
    )
}

fn ppr(p: &ParaProps) -> String {
    let mut s = String::new();
    if p.page_break_before == Some(true) {
        s.push_str("<w:pageBreakBefore/>");
    }
    if p.space_before.is_some() || p.space_after.is_some() || p.line_spacing.is_some() {
        s.push_str("<w:spacing");
        if let Some(v) = p.space_before {
            let _ = write!(s, " w:before=\"{}\"", v.0);
        }
        if let Some(v) = p.space_after {
            let _ = write!(s, " w:after=\"{}\"", v.0);
        }
        match p.line_spacing {
            Some(LineSpacing::Auto(v)) => {
                let _ = write!(s, " w:line=\"{v}\" w:lineRule=\"auto\"");
            }
            Some(LineSpacing::Exact(v)) => {
                let _ = write!(s, " w:line=\"{}\" w:lineRule=\"exact\"", v.0);
            }
            Some(LineSpacing::AtLeast(v)) => {
                let _ = write!(s, " w:line=\"{}\" w:lineRule=\"atLeast\"", v.0);
            }
            None => {}
        }
        s.push_str("/>");
    }
    if p.indent_left.is_some() || p.indent_right.is_some() || p.indent_first_line.is_some() {
        s.push_str("<w:ind");
        if let Some(v) = p.indent_left {
            let _ = write!(s, " w:left=\"{}\"", v.0);
        }
        if let Some(v) = p.indent_right {
            let _ = write!(s, " w:right=\"{}\"", v.0);
        }
        match p.indent_first_line {
            Some(v) if v.0 < 0 => {
                let _ = write!(s, " w:hanging=\"{}\"", -v.0);
            }
            Some(v) => {
                let _ = write!(s, " w:firstLine=\"{}\"", v.0);
            }
            None => {}
        }
        s.push_str("/>");
    }
    if let Some(a) = p.align {
        let v = match a {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justify => "both",
        };
        let _ = write!(s, "<w:jc w:val=\"{v}\"/>");
    }
    s
}

fn rpr(p: &RunProps) -> String {
    let mut s = String::new();
    if let Some(f) = &p.font {
        let f = escape(f);
        let _ = write!(
            s,
            "<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>"
        );
    }
    let flag = |s: &mut String, name: &str, v: Option<bool>| match v {
        Some(true) => {
            let _ = write!(s, "<w:{name}/>");
        }
        Some(false) => {
            let _ = write!(s, "<w:{name} w:val=\"0\"/>");
        }
        None => {}
    };
    flag(&mut s, "b", p.bold);
    flag(&mut s, "i", p.italic);
    flag(&mut s, "strike", p.strike);
    if let Some(c) = p.color {
        let _ = write!(s, "<w:color w:val=\"{:02X}{:02X}{:02X}\"/>", c.r, c.g, c.b);
    }
    if let Some(sz) = p.size {
        let _ = write!(s, "<w:sz w:val=\"{}\"/>", sz.0);
    }
    if let Some(u) = p.underline {
        let v = match u {
            Underline::None => "none",
            Underline::Single => "single",
            Underline::Double => "double",
        };
        let _ = write!(s, "<w:u w:val=\"{v}\"/>");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_docx;
    use bo_model::{Color, HalfPoints, Paragraph, Run, Section, Twips};

    fn sample() -> Document {
        let bold = RunProps {
            bold: Some(true),
            size: Some(HalfPoints(28)),
            color: Some(Color {
                r: 0x1F,
                g: 0x4E,
                b: 0x99,
            }),
            ..Default::default()
        };
        Document {
            default_run: RunProps {
                font: Some("Times New Roman".into()),
                size: Some(HalfPoints(26)),
                ..Default::default()
            },
            default_para: ParaProps {
                space_after: Some(Twips(120)),
                ..Default::default()
            },
            sections: vec![
                Section {
                    page: PageSetup::default(),
                    blocks: vec![Block::Paragraph(Paragraph {
                        props: ParaProps {
                            align: Some(Alignment::Center),
                            ..Default::default()
                        },
                        runs: vec![Run::text("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", bold)],
                    })],
                },
                Section {
                    page: PageSetup {
                        width: Twips(16_838),
                        height: Twips(11_906),
                        ..PageSetup::default()
                    },
                    blocks: vec![Block::Paragraph(Paragraph {
                        props: ParaProps {
                            align: Some(Alignment::Justify),
                            indent_first_line: Some(Twips(567)),
                            line_spacing: Some(LineSpacing::Auto(360)),
                            page_break_before: Some(true),
                            ..Default::default()
                        },
                        runs: vec![Run {
                            props: RunProps {
                                italic: Some(true),
                                underline: Some(Underline::Single),
                                ..Default::default()
                            },
                            content: vec![
                                RunContent::Text("Độc lập & <Tự do>".into()),
                                RunContent::Tab,
                                RunContent::LineBreak,
                                RunContent::PageBreak,
                            ],
                        }],
                    })],
                },
            ],
        }
    }

    #[test]
    fn write_then_read_is_identity() {
        let doc = sample();
        let bytes = write_docx(&doc).unwrap();
        let back = read_docx(&bytes).unwrap();
        assert_eq!(back, doc);
    }
}

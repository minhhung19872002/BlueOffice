//! `.odt` writer from [`bo_model::Document`].

use crate::dom::escape;
use crate::package::{meta_xml, OdfPackage};
use crate::units::{fmt_num, fmt_pt};
use crate::{Result, MIME_ODT};
use bo_model::{
    Alignment, Block, Document, LineSpacing, PageSetup, ParaProps, RunContent, RunProps, Underline,
};
use std::collections::HashMap;

/// Namespace declarations shared by the XML streams we write.
pub(crate) const XMLNS: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" "#,
    r#"office:version="1.3""#
);

/// Attributes of `style:paragraph-properties` (ODF 1.3 Part 3, §17.6).
fn para_attrs(p: &ParaProps) -> String {
    let mut a = String::new();
    if let Some(al) = p.align {
        // §20.216 fo:text-align
        let v = match al {
            Alignment::Left => "start",
            Alignment::Center => "center",
            Alignment::Right => "end",
            Alignment::Justify => "justify",
        };
        a.push_str(&format!(r#" fo:text-align="{v}""#));
    }
    let mut len = |name: &str, v: Option<bo_model::Twips>| {
        if let Some(t) = v {
            a.push_str(&format!(r#" {name}="{}""#, fmt_pt(t)));
        }
    };
    len("fo:margin-left", p.indent_left);
    len("fo:margin-right", p.indent_right);
    len("fo:text-indent", p.indent_first_line);
    len("fo:margin-top", p.space_before);
    len("fo:margin-bottom", p.space_after);
    match p.line_spacing {
        // §20.204 fo:line-height (percentage of single spacing)
        Some(LineSpacing::Auto(n)) => a.push_str(&format!(
            r#" fo:line-height="{}%""#,
            fmt_num(f64::from(n) / 2.4, 4)
        )),
        Some(LineSpacing::Exact(t)) => a.push_str(&format!(r#" fo:line-height="{}""#, fmt_pt(t))),
        // §20.317 style:line-height-at-least
        Some(LineSpacing::AtLeast(t)) => {
            a.push_str(&format!(r#" style:line-height-at-least="{}""#, fmt_pt(t)));
        }
        None => {}
    }
    if let Some(b) = p.page_break_before {
        // §20.186 fo:break-before
        a.push_str(if b {
            r#" fo:break-before="page""#
        } else {
            r#" fo:break-before="auto""#
        });
    }
    a
}

/// Attributes of `style:text-properties` (ODF 1.3 Part 3, §17.10).
fn text_attrs(r: &RunProps) -> String {
    let mut a = String::new();
    if let Some(f) = &r.font {
        // §20.282 style:font-name → style:font-face in office:font-face-decls
        a.push_str(&format!(
            r#" style:font-name="{0}" style:font-name-asian="{0}" style:font-name-complex="{0}""#,
            escape(f)
        ));
    }
    if let Some(s) = r.size {
        // §20.187 fo:font-size
        let pt = fmt_num(f64::from(s.0) / 2.0, 1);
        a.push_str(&format!(
            r#" fo:font-size="{pt}pt" style:font-size-asian="{pt}pt" style:font-size-complex="{pt}pt""#
        ));
    }
    if let Some(b) = r.bold {
        // §20.190 fo:font-weight
        let v = if b { "bold" } else { "normal" };
        a.push_str(&format!(
            r#" fo:font-weight="{v}" style:font-weight-asian="{v}" style:font-weight-complex="{v}""#
        ));
    }
    if let Some(i) = r.italic {
        // §20.188 fo:font-style
        let v = if i { "italic" } else { "normal" };
        a.push_str(&format!(
            r#" fo:font-style="{v}" style:font-style-asian="{v}" style:font-style-complex="{v}""#
        ));
    }
    match r.underline {
        // §20.389 / §20.390
        Some(Underline::None) => a.push_str(r#" style:text-underline-style="none""#),
        Some(Underline::Single) => a.push_str(
            r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#,
        ),
        Some(Underline::Double) => a.push_str(
            r#" style:text-underline-style="solid" style:text-underline-type="double" style:text-underline-width="auto" style:text-underline-color="font-color""#,
        ),
        None => {}
    }
    if let Some(s) = r.strike {
        // §20.375 style:text-line-through-style
        a.push_str(if s {
            r#" style:text-line-through-style="solid" style:text-line-through-type="single""#
        } else {
            r#" style:text-line-through-style="none""#
        });
    }
    if let Some(c) = r.color {
        // §20.180 fo:color
        a.push_str(&format!(
            r##" fo:color="#{:02x}{:02x}{:02x}""##,
            c.r, c.g, c.b
        ));
    }
    a
}

/// `style:page-layout-properties` (§17.2).
fn page_layout(name: &str, p: &PageSetup) -> String {
    let orientation = if p.width > p.height {
        "landscape"
    } else {
        "portrait"
    };
    format!(
        r#"<style:page-layout style:name="{name}"><style:page-layout-properties fo:page-width="{}" fo:page-height="{}" style:print-orientation="{orientation}" fo:margin-top="{}" fo:margin-bottom="{}" fo:margin-left="{}" fo:margin-right="{}"/></style:page-layout>"#,
        fmt_pt(p.width),
        fmt_pt(p.height),
        fmt_pt(p.margin_top),
        fmt_pt(p.margin_bottom),
        fmt_pt(p.margin_left),
        fmt_pt(p.margin_right),
    )
}

fn font_decls(fonts: &[String]) -> String {
    let mut s = String::from("<office:font-face-decls>");
    for f in fonts {
        s.push_str(&format!(
            r#"<style:font-face style:name="{0}" svg:font-family="&apos;{0}&apos;"/>"#,
            escape(f)
        ));
    }
    s.push_str("</office:font-face-decls>");
    s
}

/// One inline atom of a paragraph.
enum Atom<'a> {
    Char(char),
    Tab,
    LineBreak,
    Run(&'a RunProps),
}

/// Write a paragraph's runs, encoding white space so that a reader applying
/// the collapsing rules of ODF 1.3 Part 3, §6.1.2 recovers it exactly.
fn write_inline(out: &mut String, runs: &[bo_model::Run], text_styles: &HashMap<RunProps, String>) {
    let mut atoms = Vec::new();
    for r in runs {
        atoms.push(Atom::Run(&r.props));
        for c in &r.content {
            match c {
                RunContent::Text(t) => atoms.extend(t.chars().map(Atom::Char)),
                RunContent::Tab => atoms.push(Atom::Tab),
                RunContent::LineBreak => atoms.push(Atom::LineBreak),
                RunContent::PageBreak => {}
            }
        }
    }
    let last_char = atoms
        .iter()
        .rposition(|a| !matches!(a, Atom::Run(_)))
        .unwrap_or(0);
    let mut ws = true; // collapse state: at paragraph start
    let mut spaces = 0usize; // pending <text:s/>
    let mut open_span = false;
    let flush = |out: &mut String, spaces: &mut usize| {
        if *spaces == 1 {
            out.push_str("<text:s/>");
        } else if *spaces > 1 {
            out.push_str(&format!(r#"<text:s text:c="{spaces}"/>"#));
        }
        *spaces = 0;
    };
    for (i, a) in atoms.iter().enumerate() {
        match a {
            Atom::Run(props) => {
                flush(out, &mut spaces);
                if open_span {
                    out.push_str("</text:span>");
                    open_span = false;
                }
                if let Some(name) = text_styles.get(*props) {
                    out.push_str(&format!(r#"<text:span text:style-name="{name}">"#));
                    open_span = true;
                }
            }
            Atom::Char(' ') => {
                let next_is_space = matches!(atoms.get(i + 1), Some(Atom::Char(' ')));
                if ws || next_is_space || i == last_char {
                    spaces += 1;
                } else {
                    out.push(' ');
                    ws = true;
                }
            }
            Atom::Char(c) => {
                flush(out, &mut spaces);
                if matches!(c, '\t' | '\n' | '\r') {
                    // Other white space would collapse; write it as a space element.
                    out.push_str("<text:s/>");
                } else {
                    out.push_str(&escape(&c.to_string()));
                }
                ws = false;
            }
            Atom::Tab => {
                flush(out, &mut spaces);
                out.push_str("<text:tab/>");
                ws = false;
            }
            Atom::LineBreak => {
                flush(out, &mut spaces);
                out.push_str("<text:line-break/>");
                ws = true;
            }
        }
    }
    flush(out, &mut spaces);
    if open_span {
        out.push_str("</text:span>");
    }
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ParaKey {
    props: ParaProps,
    master: Option<String>,
    break_after: bool,
}

/// Serialize `doc` as an `.odt` package.
pub fn write_odt(doc: &Document) -> Result<Vec<u8>> {
    // Collect automatic styles.
    let mut para_styles: Vec<ParaKey> = Vec::new();
    let mut para_index: HashMap<ParaKey, usize> = HashMap::new();
    let mut text_styles: HashMap<RunProps, String> = HashMap::new();
    let mut text_order: Vec<RunProps> = Vec::new();
    let mut fonts: Vec<String> = Vec::new();
    let add_font = |f: &Option<String>, fonts: &mut Vec<String>| {
        if let Some(f) = f {
            if !fonts.contains(f) {
                fonts.push(f.clone());
            }
        }
    };
    add_font(&doc.default_run.font, &mut fonts);

    let mut body = String::new();
    for (si, section) in doc.sections.iter().enumerate() {
        for (bi, block) in section.blocks.iter().enumerate() {
            let Block::Paragraph(p) = block;
            let key = ParaKey {
                props: p.props,
                master: (si > 0 && bi == 0).then(|| format!("Section{}", si + 1)),
                break_after: p
                    .runs
                    .iter()
                    .any(|r| r.content.iter().any(|c| matches!(c, RunContent::PageBreak))),
            };
            let idx = *para_index.entry(key.clone()).or_insert_with(|| {
                para_styles.push(key);
                para_styles.len() - 1
            });
            for r in &p.runs {
                add_font(&r.props.font, &mut fonts);
                if r.props != RunProps::default() && !text_styles.contains_key(&r.props) {
                    text_styles.insert(r.props.clone(), format!("T{}", text_order.len() + 1));
                    text_order.push(r.props.clone());
                }
            }
            body.push_str(&format!(r#"<text:p text:style-name="P{}">"#, idx + 1));
            write_inline(&mut body, &p.runs, &text_styles);
            body.push_str("</text:p>");
        }
    }

    // content.xml (ODF 1.3 Part 3, §3.1.3.2 office:document-content)
    let mut content =
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {XMLNS}>");
    content.push_str(&font_decls(&fonts));
    content.push_str("<office:automatic-styles>");
    for (i, k) in para_styles.iter().enumerate() {
        let master = k
            .master
            .as_ref()
            .map(|m| format!(r#" style:master-page-name="{m}""#))
            .unwrap_or_default();
        let mut attrs = para_attrs(&k.props);
        if k.break_after {
            // §20.183 fo:break-after
            attrs.push_str(r#" fo:break-after="page""#);
        }
        content.push_str(&format!(
            r#"<style:style style:name="P{}" style:family="paragraph" style:parent-style-name="Standard"{master}><style:paragraph-properties{attrs}/></style:style>"#,
            i + 1
        ));
    }
    for (i, r) in text_order.iter().enumerate() {
        content.push_str(&format!(
            r#"<style:style style:name="T{}" style:family="text"><style:text-properties{}/></style:style>"#,
            i + 1,
            text_attrs(r)
        ));
    }
    content.push_str("</office:automatic-styles><office:body><office:text>");
    content.push_str(&body);
    content.push_str("</office:text></office:body></office:document-content>");

    // styles.xml (§3.1.3.3 office:document-styles)
    let mut styles =
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-styles {XMLNS}>");
    styles.push_str(&font_decls(&fonts));
    styles.push_str(&format!(
        r#"<office:styles><style:default-style style:family="paragraph"><style:paragraph-properties{}/><style:text-properties{}/></style:default-style><style:style style:name="Standard" style:family="paragraph" style:class="text"/></office:styles>"#,
        para_attrs(&doc.default_para),
        text_attrs(&doc.default_run)
    ));
    styles.push_str("<office:automatic-styles>");
    let pages: Vec<PageSetup> = if doc.sections.is_empty() {
        vec![PageSetup::default()]
    } else {
        doc.sections.iter().map(|s| s.page).collect()
    };
    for (i, p) in pages.iter().enumerate() {
        styles.push_str(&page_layout(&format!("pm{}", i + 1), p));
    }
    styles.push_str("</office:automatic-styles><office:master-styles>");
    for i in 0..pages.len() {
        let name = if i == 0 {
            "Standard".to_owned()
        } else {
            format!("Section{}", i + 1)
        };
        styles.push_str(&format!(
            r#"<style:master-page style:name="{name}" style:page-layout-name="pm{}"/>"#,
            i + 1
        ));
    }
    styles.push_str("</office:master-styles></office:document-styles>");

    OdfPackage::write(
        MIME_ODT,
        &[
            ("content.xml", "text/xml", content.into_bytes()),
            ("styles.xml", "text/xml", styles.into_bytes()),
            ("meta.xml", "text/xml", meta_xml()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_odt;
    use bo_model::{Color, HalfPoints, Paragraph, Run, Section, Twips};

    fn rp(f: impl FnOnce(&mut RunProps)) -> RunProps {
        let mut r = RunProps::default();
        f(&mut r);
        r
    }

    pub(crate) fn sample() -> Document {
        let para = |props: ParaProps, runs: Vec<Run>| Block::Paragraph(Paragraph { props, runs });
        Document {
            default_run: rp(|r| {
                r.font = Some("Times New Roman".into());
                r.size = Some(HalfPoints(26));
            }),
            default_para: ParaProps {
                space_after: Some(Twips(120)),
                ..Default::default()
            },
            sections: vec![
                Section {
                    page: PageSetup::default(),
                    blocks: vec![
                        para(
                            ParaProps {
                                align: Some(Alignment::Center),
                                ..Default::default()
                            },
                            vec![Run::text(
                                "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
                                rp(|r| r.bold = Some(true)),
                            )],
                        ),
                        para(
                            ParaProps {
                                align: Some(Alignment::Justify),
                                indent_first_line: Some(Twips(567)),
                                indent_left: Some(Twips(-120)),
                                indent_right: Some(Twips(15)),
                                space_before: Some(Twips(60)),
                                line_spacing: Some(LineSpacing::Auto(360)),
                                ..Default::default()
                            },
                            vec![
                                Run::text("  Đậm ", rp(|r| r.bold = Some(true))),
                                Run::text("nghiêng", rp(|r| r.italic = Some(true))),
                                Run {
                                    props: RunProps::default(),
                                    content: vec![
                                        RunContent::Text(" a  b ".into()),
                                        RunContent::Tab,
                                        RunContent::Text(" c".into()),
                                        RunContent::LineBreak,
                                        RunContent::Text(" d <&> ".into()),
                                    ],
                                },
                                Run::text(
                                    "gạch",
                                    rp(|r| {
                                        r.underline = Some(Underline::Double);
                                        r.strike = Some(true);
                                        r.color = Some(Color {
                                            r: 0xC0,
                                            g: 0,
                                            b: 0x10,
                                        });
                                        r.size = Some(HalfPoints(29));
                                        r.font = Some("Arial".into());
                                    }),
                                ),
                                Run::text("đơn", rp(|r| r.underline = Some(Underline::Single))),
                                Run {
                                    props: RunProps::default(),
                                    content: vec![RunContent::PageBreak],
                                },
                            ],
                        ),
                        para(
                            ParaProps {
                                line_spacing: Some(LineSpacing::Exact(Twips(300))),
                                page_break_before: Some(true),
                                ..Default::default()
                            },
                            vec![Run::text("x", RunProps::default())],
                        ),
                        para(ParaProps::default(), vec![]),
                    ],
                },
                Section {
                    page: PageSetup {
                        width: Twips(16_838),
                        height: Twips(11_906),
                        ..PageSetup::default()
                    },
                    blocks: vec![para(
                        ParaProps {
                            line_spacing: Some(LineSpacing::AtLeast(Twips(280))),
                            align: Some(Alignment::Right),
                            ..Default::default()
                        },
                        vec![Run::text("Ngang", rp(|r| r.italic = Some(false)))],
                    )],
                },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let doc = sample();
        let bytes = write_odt(&doc).unwrap();
        let back = read_odt(&bytes).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn whitespace_encoding() {
        let mut out = String::new();
        let runs = vec![Run::text("  a  b ", RunProps::default())];
        write_inline(&mut out, &runs, &HashMap::new());
        assert_eq!(out, r#"<text:s text:c="2"/>a <text:s/>b<text:s/>"#);
    }
}

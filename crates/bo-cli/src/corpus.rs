//! Self-generated round-trip corpus for BlueDoc (`tests/corpus/doc`).
//!
//! Every file is assembled from hand-written WordprocessingML (ECMA-376 Part 1)
//! so that it exercises one feature group, including markup the editor does not
//! model (rsid attributes, `w14:*`, `mc:AlternateContent`, content controls…),
//! which must survive a round trip. All text is invented.
#![allow(clippy::needless_raw_string_hashes, clippy::format_collect)] // uniform r#"…"# literals for XML

use bo_opc::{Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use std::path::Path;

const NS: &str = concat!(
    r#"xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" "#,
    r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" "#,
    r#"xmlns:o="urn:schemas-microsoft-com:office:office" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" "#,
    r#"xmlns:v="urn:schemas-microsoft-com:vml" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" "#,
    r#"xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
    // ECMA-376 Part 3, §10.2.1 Ignorable: extension namespaces a consumer may skip
    r#"mc:Ignorable="w14 w15""#
);

const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
/// ECMA-376 Part 1, §11.3.8 Glossary Document part.
const REL_GLOSSARY: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument";
const REL_CORE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

const CT_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
const CT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const CT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
// SmartArt namespaces (ECMA-376 Part 1, §21.4; [MS-ODRAWXML] dsp).
const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const REL_COMMENTS_EXT: &str =
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
const REL_COMMENTS_IDS: &str =
    "http://schemas.microsoft.com/office/2016/09/relationships/commentsIds";
const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const CT_COMMENTS_EXT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml";
const CT_COMMENTS_IDS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsIds+xml";

/// Package parts of one corpus file.
#[derive(Default)]
struct Doc {
    body: String,
    sect: String,
    styles: Option<String>,
    numbering: Option<String>,
    /// (rId, kind "header"/"footer", content)
    hf: Vec<(String, &'static str, String)>,
    footnotes: Option<String>,
    endnotes: Option<String>,
    settings: Option<String>,
    images: Vec<(String, Vec<u8>)>,
    links: Vec<(String, String)>,
    /// Further parts related to the main document: (file name in /word, content type,
    /// relationship type, complete XML).
    extra: Vec<(&'static str, &'static str, &'static str, String)>,
    /// `w:background` element before the body (ECMA-376 Part 1, §17.2.1).
    background: Option<String>,
    /// Pictures of header/footer parts: (header/footer rId, picture rId in that part,
    /// index into `images`).
    hf_images: Vec<(String, String, usize)>,
    /// Custom XML data storage parts (ECMA-376 Part 1, §15.2.4): (item XML, `ds:itemID`),
    /// written as `/customXml/itemN.xml` with an `itemPropsN.xml` properties part (§15.2.5).
    custom_xml: Vec<(String, String)>,
    /// Glossary document (ECMA-376 Part 1, §17.12): the building blocks of the file.
    glossary: Option<Gloss>,
}

/// Parts of a glossary document: `w:docPart` elements, the glossary's own styles
/// (default: the corpus styles) and numbering, and pictures of `Doc::images` it
/// refers to as (relationship id, index).
#[derive(Default)]
struct Gloss {
    parts: String,
    styles: Option<String>,
    numbering: Option<String>,
    images: Vec<(&'static str, usize)>,
}

fn wrap(root: &str, inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:{root} {NS}>{inner}</w:{root}>"#
    )
}

impl Doc {
    fn build(&self) -> Result<Vec<u8>, bo_opc::OpcError> {
        let mut p = Package::new();
        let doc = wrap(
            "document",
            &format!(
                "{}<w:body>{}{}</w:body>",
                self.background.as_deref().unwrap_or(""),
                self.body,
                self.sect
            ),
        );
        p.add_part("/word/document.xml", CT_MAIN, doc.into_bytes())?;
        let mut rels = Vec::new();
        let mut rel = |id: &str, ty: &str, target: &str, external: bool| {
            rels.push(Relationship {
                id: id.into(),
                rel_type: ty.into(),
                target: target.into(),
                mode: if external {
                    TargetMode::External
                } else {
                    TargetMode::Internal
                },
            });
        };
        let styles = self.styles.clone().unwrap_or_else(default_styles);
        p.add_part(
            "/word/styles.xml",
            CT_STYLES,
            wrap("styles", &styles).into_bytes(),
        )?;
        rel("rId1", REL_STYLES, "styles.xml", false);
        if let Some(n) = &self.numbering {
            p.add_part(
                "/word/numbering.xml",
                CT_NUMBERING,
                wrap("numbering", n).into_bytes(),
            )?;
            rel("rId2", REL_NUMBERING, "numbering.xml", false);
        }
        let settings = self.settings.clone().unwrap_or_else(|| {
            r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()
        });
        p.add_part(
            "/word/settings.xml",
            CT_SETTINGS,
            wrap("settings", &settings).into_bytes(),
        )?;
        rel("rId3", REL_SETTINGS, "settings.xml", false);
        if let Some(f) = &self.footnotes {
            p.add_part(
                "/word/footnotes.xml",
                CT_FOOTNOTES,
                wrap("footnotes", &notes("footnote", f)).into_bytes(),
            )?;
            rel("rId4", REL_FOOTNOTES, "footnotes.xml", false);
        }
        if let Some(e) = &self.endnotes {
            p.add_part(
                "/word/endnotes.xml",
                CT_ENDNOTES,
                wrap("endnotes", &notes("endnote", e)).into_bytes(),
            )?;
            rel("rId5", REL_ENDNOTES, "endnotes.xml", false);
        }
        for (i, (id, kind, content)) in self.hf.iter().enumerate() {
            let name = format!("{kind}{}.xml", i + 1);
            let (ct, ty, root) = if *kind == "header" {
                (CT_HEADER, REL_HEADER, "hdr")
            } else {
                (CT_FOOTER, REL_FOOTER, "ftr")
            };
            p.add_part(
                &format!("/word/{name}"),
                ct,
                wrap(root, content).into_bytes(),
            )?;
            rel(id, ty, &name, false);
            // Pictures referenced from this part (relationships are per part).
            let own: Vec<Relationship> = self
                .hf_images
                .iter()
                .filter(|(hf, _, _)| hf == id)
                .map(|(_, rid, k)| Relationship {
                    id: rid.clone(),
                    rel_type: REL_IMAGE.into(),
                    target: format!("media/image{}.png", k + 1),
                    mode: TargetMode::Internal,
                })
                .collect();
            if !own.is_empty() {
                p.set_relationships(&format!("/word/{name}"), &own);
            }
        }
        for (i, (id, bytes)) in self.images.iter().enumerate() {
            let name = format!("media/image{}.png", i + 1);
            p.add_part(&format!("/word/{name}"), "image/png", bytes.clone())?;
            rel(id, REL_IMAGE, &name, false);
        }
        for (id, url) in &self.links {
            rel(id, REL_HYPERLINK, url, true);
        }
        for (i, (name, ct, ty, xml)) in self.extra.iter().enumerate() {
            p.add_part(&format!("/word/{name}"), ct, xml.clone().into_bytes())?;
            rel(&format!("rIdX{}", i + 1), ty, name, false);
        }
        for (i, (xml, id)) in self.custom_xml.iter().enumerate() {
            let k = i + 1;
            let item = format!("/customXml/item{k}.xml");
            let props = bo_model::custom_xml::CustomXmlPart {
                part_name: item.clone(),
                xml: xml.clone(),
                item_id: Some(id.clone()),
                schema_refs: Vec::new(),
                props_part: Some(format!("/customXml/itemProps{k}.xml")),
            }
            .props_xml();
            p.add_part(&item, "application/xml", xml.clone().into_bytes())?;
            p.add_part(
                &format!("/customXml/itemProps{k}.xml"),
                "application/vnd.openxmlformats-officedocument.customXmlProperties+xml",
                props.into_bytes(),
            )?;
            p.set_relationships(
                &item,
                &[Relationship {
                    id: "rId1".into(),
                    rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps".into(),
                    target: format!("itemProps{k}.xml"),
                    mode: TargetMode::Internal,
                }],
            );
            rel(
                &format!("rIdC{k}"),
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml",
                &format!("../customXml/item{k}.xml"),
                false,
            );
        }
        if let Some(g) = &self.glossary {
            // §11.3.8 Glossary Document part with relationships of its own (Part 2, §9.3).
            p.add_part(
                "/word/glossary/document.xml",
                bo_docx::CT_GLOSSARY,
                wrap(
                    "glossaryDocument",
                    &format!("<w:docParts>{}</w:docParts>", g.parts),
                )
                .into_bytes(),
            )?;
            let own = |id: &str, ty: &str, target: &str| Relationship {
                id: id.into(),
                rel_type: ty.into(),
                target: target.into(),
                mode: TargetMode::Internal,
            };
            let mut grels = vec![own("rId1", REL_STYLES, "styles.xml")];
            p.add_part(
                "/word/glossary/styles.xml",
                CT_STYLES,
                wrap("styles", &g.styles.clone().unwrap_or_else(default_styles)).into_bytes(),
            )?;
            if let Some(n) = &g.numbering {
                p.add_part(
                    "/word/glossary/numbering.xml",
                    CT_NUMBERING,
                    wrap("numbering", n).into_bytes(),
                )?;
                grels.push(own("rId2", REL_NUMBERING, "numbering.xml"));
            }
            for (id, k) in &g.images {
                grels.push(own(id, REL_IMAGE, &format!("../media/image{}.png", k + 1)));
            }
            p.set_relationships("/word/glossary/document.xml", &grels);
            rel("rIdG1", REL_GLOSSARY, "glossary/document.xml", false);
        }
        p.set_relationships("/word/document.xml", &rels);
        let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>BlueOffice corpus</dc:title><dc:creator>bo-cli corpus</dc:creator><dcterms:created xsi:type="dcterms:W3CDTF">2026-11-07T00:00:00Z</dcterms:created></cp:coreProperties>"#;
        p.add_part("/docProps/core.xml", CT_CORE, core.as_bytes().to_vec())?;
        p.set_relationships(
            "/",
            &[
                Relationship {
                    id: "rId1".into(),
                    rel_type: REL_OFFICE_DOCUMENT.into(),
                    target: "word/document.xml".into(),
                    mode: TargetMode::Internal,
                },
                Relationship {
                    id: "rId2".into(),
                    rel_type: REL_CORE.into(),
                    target: "docProps/core.xml".into(),
                    mode: TargetMode::Internal,
                },
            ],
        );
        p.write()
    }
}

/// Initials of an author name (w:initials).
fn initials(author: &str) -> String {
    author
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

fn notes(kind: &str, inner: &str) -> String {
    format!(
        r#"<w:{kind} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:{kind}><w:{kind} w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{kind}>{inner}"#
    )
}

fn default_styles() -> String {
    r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsia="Times New Roman" w:cs="Times New Roman"/><w:sz w:val="26"/><w:szCs w:val="26"/><w:lang w:val="vi-VN" w:eastAsia="en-US" w:bidi="ar-SA"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:latentStyles w:defLockedState="0" w:defUIPriority="99" w:defSemiHidden="0" w:defUnhideWhenUsed="0" w:defQFormat="0" w:count="376"><w:lsdException w:name="Normal" w:uiPriority="0" w:qFormat="1"/><w:lsdException w:name="heading 1" w:uiPriority="9" w:qFormat="1"/></w:latentStyles><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:link w:val="Heading1Char"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:bCs/><w:color w:val="1F3A68"/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="character" w:customStyle="1" w:styleId="Heading1Char"><w:name w:val="Heading 1 Char"/><w:basedOn w:val="DefaultParagraphFont"/><w:link w:val="Heading1"/><w:rPr><w:b/><w:color w:val="1F3A68"/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#.to_owned()
}

fn sect(extra: &str) -> String {
    format!(
        r#"<w:sectPr w:rsidR="00A11B22">{extra}<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/><w:docGrid w:linePitch="360"/></w:sectPr>"#
    )
}

fn p(text: &str) -> String {
    format!(
        r#"<w:p w14:paraId="1A2B3C4D" w14:textId="77777777" w:rsidR="00A11B22" w:rsidRDefault="00A11B22"><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#
    )
}

fn pr(ppr: &str, rpr: &str, text: &str) -> String {
    format!(
        r#"<w:p><w:pPr>{ppr}</w:pPr><w:r><w:rPr>{rpr}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#
    )
}

/// A paragraph of runs `(rPr children, text)`.
fn rpara(runs: &[(&str, &str)]) -> String {
    let mut s = String::from("<w:p>");
    for (rpr, text) in runs {
        s.push_str("<w:r>");
        if !rpr.is_empty() {
            s.push_str(&format!("<w:rPr>{rpr}</w:rPr>"));
        }
        s.push_str(&format!(r#"<w:t xml:space="preserve">{text}</w:t></w:r>"#));
    }
    s.push_str("</w:p>");
    s
}

const VI: &[&str] = &[
    "Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015;",
    "Theo đề nghị của Giám đốc Trung tâm Công nghệ thông tin (giả định);",
    "Quyết định này có hiệu lực kể từ ngày ký ban hành.",
    "Các đơn vị, cá nhân có liên quan chịu trách nhiệm thi hành.",
];

fn lorem(n: usize) -> String {
    (0..n)
        .map(|i| p(&format!("{} {}", i + 1, VI[i % VI.len()])))
        .collect()
}

/// Tiny RGB PNG (solid colour) for picture tests.
fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        if let Ok(mut wr) = enc.write_header() {
            let data: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
            let _ = wr.write_image_data(&data);
        }
    }
    out
}

/// `id` is `wp:docPr/@id`, which must be unique within the document
/// (ECMA-376 Part 1, §20.4.2.5 docPr).
fn inline_pic(id: u32, rid: &str, cx: i64, cy: i64, extra_xfrm: &str, src_rect: &str) -> String {
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Ảnh {id}" descr="Ảnh minh hoạ"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="image1.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/>{src_rect}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm{extra_xfrm}><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

#[allow(clippy::too_many_arguments)]
fn anchor_pic(
    id: u32,
    rid: &str,
    cx: i64,
    cy: i64,
    wrap: &str,
    behind: bool,
    rel_h: &str,
    rel_v: &str,
) -> String {
    let b = u8::from(behind);
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="{b}" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="{rel_h}"><wp:posOffset>457200</wp:posOffset></wp:positionH><wp:positionV relativeFrom="{rel_v}"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="Ảnh nổi {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="image.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

/// Anchored picture at (`x`, `y`) EMU from the margin / paragraph with `dist` = T, B, L, R
/// (ECMA-376 Part 1, §20.4.2.3 anchor) and the wrap element `wrap` (§20.4.2.17, §20.4.2.20).
fn wrap_pic(
    id: u32,
    rid: &str,
    size: (i64, i64),
    at: (i64, i64),
    dist: [i64; 4],
    wrap: &str,
) -> String {
    let (cx, cy) = size;
    let (x, y) = at;
    let [t, b, l, r] = dist;
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="{t}" distB="{b}" distL="{l}" distR="{r}" simplePos="0" relativeHeight="{id}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="margin"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="Ảnh bao chữ {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="image.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

/// `wp:wrapTight` with a polygon given in Word's 21 600-unit square (§20.4.2.16).
fn tight(side: &str, pts: &[(i64, i64)]) -> String {
    let mut poly = String::new();
    for (i, (x, y)) in pts.iter().chain(pts.first()).enumerate() {
        let tag = if i == 0 { "start" } else { "lineTo" };
        poly.push_str(&format!(r#"<wp:{tag} x="{x}" y="{y}"/>"#));
    }
    format!(
        r#"<wp:wrapTight wrapText="{side}"><wp:wrapPolygon edited="1">{poly}</wp:wrapPolygon></wp:wrapTight>"#
    )
}

fn cell(w: i32, extra: &str, text: &str) -> String {
    format!(
        r#"<w:tc><w:tcPr><w:tcW w:w="{w}" w:type="dxa"/>{extra}</w:tcPr>{}</w:tc>"#,
        p(text)
    )
}

fn table(grid: &[i32], rows: &[String], tblpr: &str) -> String {
    let g: String = grid
        .iter()
        .map(|w| format!(r#"<w:gridCol w:w="{w}"/>"#))
        .collect();
    format!(
        r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/>{tblpr}<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid>{g}</w:tblGrid>{}</w:tbl>"#,
        rows.concat()
    )
}

fn row(trpr: &str, cells: &[String]) -> String {
    format!(
        r#"<w:tr w:rsidR="00A11B22"><w:trPr>{trpr}</w:trPr>{}</w:tr>"#,
        cells.concat()
    )
}

fn numbering(abstracts: &[(i32, &[(&str, &str)])], nums: &str) -> String {
    let mut s = String::new();
    for (id, levels) in abstracts {
        s.push_str(&format!(r#"<w:abstractNum w:abstractNumId="{id}" w15:restartNumberingAfterBreak="0"><w:nsid w:val="1A2B3C{id:02}"/><w:multiLevelType w:val="hybridMultilevel"/>"#));
        for (lvl, (fmt, text)) in levels.iter().enumerate() {
            let ind = 720 * (lvl + 1);
            s.push_str(&format!(r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{ind}" w:hanging="360"/></w:pPr></w:lvl>"#));
        }
        s.push_str("</w:abstractNum>");
    }
    s.push_str(nums);
    s
}

fn li(num: i32, lvl: u8, text: &str) -> String {
    pr(
        &format!(
            r#"<w:pStyle w:val="Normal"/><w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{num}"/></w:numPr>"#
        ),
        "",
        text,
    )
}

fn hdr_ref(kind: &str, ty: &str, rid: &str) -> String {
    format!(r#"<w:{kind}Reference w:type="{ty}" r:id="{rid}"/>"#)
}

fn field_simple(instr: &str, cached: &str) -> String {
    // w:instr is an attribute (ECMA-376 Part 1, §17.16.19 fldSimple): quotes in
    // field switches such as `\@ "dd/MM/yyyy"` must be escaped.
    let instr = instr
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;");
    format!(r#"<w:fldSimple w:instr=" {instr} "><w:r><w:t>{cached}</w:t></w:r></w:fldSimple>"#)
}

fn field_complex(instr: &str, cached: &str) -> String {
    format!(
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{cached}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
    )
}

/// A CITATION field (ECMA-376 Part 1, §17.16.5.8) as a complex field inside its inline
/// content control (§17.5.2.3 citation). `shown` is XML-escaped text.
fn cite_sdt(id: i32, instr: &str, shown: &str) -> String {
    format!(
        r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w:citation/></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>"#,
        field_complex(instr, shown)
    )
}

/// A CITATION field as a simple field (§17.16.19 fldSimple).
fn cite_simple(instr: &str, shown: &str) -> String {
    format!(r#"<w:fldSimple w:instr=" {instr} "><w:r><w:t>{shown}</w:t></w:r></w:fldSimple>"#)
}

/// A BIBLIOGRAPHY field (§17.16.5.7) spanning one paragraph per entry, inside its block
/// content control (§17.5.2.2 bibliography). An entry is XML-escaped text with italic
/// stretches between asterisks and a tab after the number label of numeric styles.
fn bibliography_sdt(id: i32, instr: &str, entries: &[&str]) -> String {
    let mut out =
        format!(r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w:bibliography/></w:sdtPr><w:sdtContent>"#);
    for (i, e) in entries.iter().enumerate() {
        out.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Bibliography"/></w:pPr>"#);
        if i == 0 {
            out.push_str(&format!(
                r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#
            ));
        }
        let (label, text) = match e.split_once('\t') {
            Some((l, t)) => (Some(l), t),
            None => (None, *e),
        };
        if let Some(l) = label {
            out.push_str(&format!("<w:r><w:t>{l}</w:t><w:tab/></w:r>"));
        }
        for (k, part) in text.split('*').enumerate() {
            if part.is_empty() {
                continue;
            }
            let rpr = if k % 2 == 1 {
                "<w:rPr><w:i/></w:rPr>"
            } else {
                ""
            };
            out.push_str(&format!(
                r#"<w:r>{rpr}<w:t xml:space="preserve">{part}</w:t></w:r>"#
            ));
        }
        if i + 1 == entries.len() {
            out.push_str(r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#);
        }
        out.push_str("</w:p>");
    }
    out.push_str("</w:sdtContent></w:sdt>");
    out
}

/// One `b:Source` (§22.6.2.59): people as (family, given) or a corporate author, then the
/// other elements. Values are XML-escaped text.
fn cite_source(
    tag: &str,
    kind: &str,
    people: &[(&str, &str)],
    corporate: &str,
    fields: &[(&str, &str)],
) -> String {
    let mut out = format!("<b:Source><b:Tag>{tag}</b:Tag><b:SourceType>{kind}</b:SourceType>");
    if !corporate.is_empty() {
        out.push_str(&format!(
            "<b:Author><b:Author><b:Corporate>{corporate}</b:Corporate></b:Author></b:Author>"
        ));
    } else if !people.is_empty() {
        out.push_str("<b:Author><b:Author><b:NameList>");
        for (last, first) in people {
            out.push_str(&format!(
                "<b:Person><b:Last>{last}</b:Last><b:First>{first}</b:First></b:Person>"
            ));
        }
        out.push_str("</b:NameList></b:Author></b:Author>");
    }
    for (k, v) in fields {
        out.push_str(&format!("<b:{k}>{v}</b:{k}>"));
    }
    out.push_str("</b:Source>");
    out
}

/// The bibliography part (§22.6.2.60 Sources) with the selected style's name.
fn cite_sources(style: &str, sources: &[String]) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<b:Sources StyleName=\"{style}\" xmlns:b=\"http://schemas.openxmlformats.org/officeDocument/2006/bibliography\" xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/bibliography\">{}</b:Sources>",
        sources.concat()
    )
}

/// §17.12.5 docPart: properties (§17.12.7 docPartPr; `n` makes the §17.12.11 guid)
/// and body (§17.12.6 docPartBody).
fn doc_part(n: u32, name: &str, gallery: &str, category: &str, extra: &str, body: &str) -> String {
    format!(
        r#"<w:docPart><w:docPartPr><w:name w:val="{name}"/><w:category><w:name w:val="{category}"/><w:gallery w:val="{gallery}"/></w:category>{extra}<w:guid w:val="{{C0A5B10C-{n:04}-4D2E-9F3A-5B6C7D8E9F00}}"/></w:docPartPr><w:docPartBody>{body}</w:docPartBody></w:docPart>"#
    )
}

/// §17.12.15 types and §17.12.2 behaviors of a docPart.
fn part_kind(ty: &str, behavior: &str) -> String {
    format!(
        r#"<w:types><w:type w:val="{ty}"/></w:types><w:behaviors><w:behavior w:val="{behavior}"/></w:behaviors>"#
    )
}

/// All corpus documents: (file name, feature, description, package).
fn documents() -> Vec<(&'static str, &'static str, &'static str, Doc)> {
    let mut v = Vec::new();
    let s = sect("");

    // ---- text: run properties (§17.3.2)
    v.push(("text-01-run-props.docx", "text", "Đậm/nghiêng/gạch chân các kiểu, gạch ngang đơn/kép, màu, cỡ chữ, phông", Doc {
        body: [
            pr("", r#"<w:b/><w:bCs/>"#, "Chữ đậm"),
            pr("", r#"<w:i/>"#, "Chữ nghiêng"),
            pr("", r#"<w:u w:val="double"/>"#, "Gạch chân kép"),
            pr("", r#"<w:u w:val="dotted"/>"#, "Gạch chân chấm"),
            pr("", r#"<w:u w:val="wave"/>"#, "Gạch chân lượn sóng"),
            pr("", r#"<w:strike/>"#, "Gạch ngang"),
            pr("", r#"<w:dstrike/>"#, "Gạch ngang kép"),
            pr("", r#"<w:color w:val="C00000"/><w:sz w:val="36"/>"#, "Màu đỏ cỡ 18"),
            pr("", r#"<w:rFonts w:ascii="Arial" w:hAnsi="Arial"/>"#, "Phông Arial"),
            pr("", r#"<w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/>"#, "Phông Calibri 11"),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("text-02-highlight-sup-caps.docx", "text", "Tô sáng, nền, chỉ số trên/dưới, chữ hoa, chữ hoa nhỏ, giãn chữ, kerning", Doc {
        body: [
            pr("", r#"<w:highlight w:val="yellow"/>"#, "Tô sáng vàng"),
            pr("", r#"<w:shd w:val="clear" w:color="auto" w:fill="D9E2F3"/>"#, "Nền xanh nhạt"),
            r#"<w:p><w:r><w:t>H</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>2</w:t></w:r><w:r><w:t>O và x</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>2</w:t></w:r></w:p>"#.to_owned(),
            pr("", r#"<w:caps/>"#, "chữ hoa toàn bộ"),
            pr("", r#"<w:smallCaps/>"#, "Chữ Hoa Nhỏ"),
            pr("", r#"<w:spacing w:val="40"/><w:kern w:val="28"/>"#, "Giãn chữ 2pt"),
            pr("", r#"<w:vanish/>"#, "Văn bản ẩn"),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("text-03-vietnamese-rsid.docx", "text", "Tiếng Việt nhiều run, rsid, proofErr, lastRenderedPageBreak, w14:paraId, ký tự đặc biệt", Doc {
        body: [
            r#"<w:p w14:paraId="0F0E0D0C" w14:textId="1A1B1C1D" w:rsidR="00B22C33" w:rsidRPr="00C33D44" w:rsidRDefault="00B22C33" w:rsidP="00D44E55"><w:pPr><w:jc w:val="both"/></w:pPr><w:r w:rsidRPr="00C33D44"><w:t xml:space="preserve">Cộng hòa </w:t></w:r><w:proofErr w:type="spellStart"/><w:r w:rsidR="00E55F66"><w:t>xã</w:t></w:r><w:proofErr w:type="spellEnd"/><w:r><w:lastRenderedPageBreak/><w:t xml:space="preserve"> hội chủ nghĩa Việt Nam</w:t></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:t>Tab:</w:t><w:tab/><w:t>ngắt dòng</w:t><w:br/><w:t>gạch nối</w:t><w:noBreakHyphen/><w:t>mềm</w:t><w:softHyphen/><w:t>©®™ — “ngoặc” ‘đơn’ … € ₫</w:t></w:r></w:p>"#.to_owned(),
            lorem(3),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- paragraphs (§17.3.1)
    v.push((
        "para-01-align-indent.docx",
        "paragraph",
        "Căn trái/giữa/phải/đều, thụt lề trái/phải/dòng đầu/treo",
        Doc {
            body: [
                pr(r#"<w:jc w:val="left"/>"#, "", "Căn trái"),
                pr(r#"<w:jc w:val="center"/>"#, "", "Căn giữa"),
                pr(r#"<w:jc w:val="right"/>"#, "", "Căn phải"),
                pr(
                    // CT_PPrBase order: ind before jc (ECMA-376 Part 1, §17.3.1.26)
                    r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
                    "",
                    &VI.concat(),
                ),
                pr(
                    r#"<w:ind w:left="1440" w:right="720" w:hanging="360"/>"#,
                    "",
                    &VI.concat(),
                ),
            ]
            .concat(),
            sect: s.clone(),
            ..Doc::default()
        },
    ));
    v.push(("para-02-spacing-keep.docx", "paragraph", "Khoảng cách trước/sau, giãn dòng auto/exact/atLeast, keepNext, keepLines, widowControl, pageBreakBefore, contextualSpacing", Doc {
        body: [
            pr(r#"<w:spacing w:before="240" w:after="240" w:line="360" w:lineRule="auto"/>"#, "", &VI.concat()),
            pr(r#"<w:spacing w:line="400" w:lineRule="exact"/>"#, "", &VI.concat()),
            pr(r#"<w:spacing w:line="200" w:lineRule="atLeast"/>"#, "", &VI.concat()),
            pr(r#"<w:keepNext/><w:keepLines/>"#, "", "Giữ với đoạn sau"),
            pr(r#"<w:widowControl w:val="0"/><w:contextualSpacing/>"#, "", "Không kiểm soát dòng mồ côi"),
            pr(r#"<w:pageBreakBefore/>"#, "", "Sang trang trước đoạn"),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("para-03-tabs-borders.docx", "paragraph", "Tab stop tuỳ biến (trái/giữa/phải/thập phân, dẫn chấm), viền và nền đoạn", Doc {
        body: [
            r#"<w:p><w:pPr><w:tabs><w:tab w:val="center" w:pos="4680"/><w:tab w:val="right" w:leader="dot" w:pos="9355"/></w:tabs></w:pPr><w:r><w:t>Mục lục</w:t><w:tab/><w:t>giữa</w:t><w:tab/><w:t>12</w:t></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:pPr><w:tabs><w:tab w:val="decimal" w:pos="3000"/></w:tabs></w:pPr><w:r><w:tab/><w:t>1234,56</w:t></w:r></w:p>"#.to_owned(),
            pr(r#"<w:pBdr><w:top w:val="single" w:sz="8" w:space="1" w:color="1F3A68"/><w:left w:val="single" w:sz="8" w:space="4" w:color="1F3A68"/><w:bottom w:val="double" w:sz="6" w:space="1" w:color="auto"/><w:right w:val="dashed" w:sz="8" w:space="4" w:color="auto"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/>"#, "", "Đoạn có viền và nền"),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- styles (§17.7)
    let heading =
        |lvl: u8, text: &str| pr(&format!(r#"<w:pStyle w:val="Heading{lvl}"/>"#), "", text);
    let mut styles2 = default_styles();
    styles2.push_str(r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Heading1"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:i/><w:sz w:val="28"/></w:rPr></w:style><w:style w:type="paragraph" w:customStyle="1" w:styleId="TrichYeu"><w:name w:val="Trích yếu"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:b/></w:rPr></w:style><w:style w:type="character" w:customStyle="1" w:styleId="NhanManh"><w:name w:val="Nhấn mạnh"/><w:rPr><w:i/><w:color w:val="C00000"/></w:rPr></w:style>"#);
    v.push((
        "styles-01-headings.docx",
        "styles",
        "docDefaults, latentStyles, Heading1 kế thừa Normal, next style",
        Doc {
            body: [
                heading(1, "Chương I"),
                lorem(2),
                heading(1, "Chương II"),
                lorem(2),
            ]
            .concat(),
            sect: s.clone(),
            ..Doc::default()
        },
    ));
    v.push((
        "styles-02-based-on-chain.docx",
        "styles",
        "Chuỗi basedOn Heading2 → Heading1 → Normal, style đoạn tuỳ biến",
        Doc {
            body: [
                heading(1, "Phần A"),
                heading(2, "Mục A.1"),
                pr(
                    r#"<w:pStyle w:val="TrichYeu"/>"#,
                    "",
                    "Về việc kiểm thử kiểu",
                ),
                lorem(2),
            ]
            .concat(),
            sect: s.clone(),
            styles: Some(styles2.clone()),
            ..Doc::default()
        },
    ));
    v.push(("styles-03-character-style.docx", "styles", "Kiểu ký tự (rStyle) cùng định dạng trực tiếp, kiểu bảng", Doc {
        body: [
            r#"<w:p><w:r><w:t xml:space="preserve">Bình thường </w:t></w:r><w:r><w:rPr><w:rStyle w:val="NhanManh"/></w:rPr><w:t>nhấn mạnh</w:t></w:r><w:r><w:rPr><w:rStyle w:val="NhanManh"/><w:b/></w:rPr><w:t xml:space="preserve"> và đậm</w:t></w:r></w:p>"#.to_owned(),
            table(&[4677, 4678], &[row("", &[cell(4677, "", "A"), cell(4678, "", "B")])], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            p(""),
        ].concat(),
        sect: s.clone(),
        styles: Some(styles2),
        ..Doc::default()
    }));

    // ---- lists (§17.9)
    let multi: &[(&str, &str)] = &[
        ("decimal", "%1."),
        ("lowerLetter", "%2)"),
        ("lowerRoman", "%3."),
        ("upperRoman", "%4."),
    ];
    let bullets: &[(&str, &str)] = &[("bullet", "•"), ("bullet", "o"), ("bullet", "▪")];
    let nums_basic = r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#;
    v.push((
        "lists-01-multilevel.docx",
        "lists",
        "Danh sách số nhiều cấp (decimal, lowerLetter, lowerRoman, upperRoman)",
        Doc {
            body: [
                li(1, 0, "Một"),
                li(1, 1, "Một a"),
                li(1, 1, "Một b"),
                li(1, 2, "La mã"),
                li(1, 0, "Hai"),
                li(1, 3, "Cấp 4"),
            ]
            .concat(),
            sect: s.clone(),
            numbering: Some(numbering(&[(0, multi), (1, bullets)], nums_basic)),
            ..Doc::default()
        },
    ));
    v.push((
        "lists-02-bullets.docx",
        "lists",
        "Danh sách gạch đầu dòng các cấp xen đoạn thường",
        Doc {
            body: [
                li(2, 0, "Ý thứ nhất"),
                li(2, 1, "Ý con"),
                p("Đoạn chen giữa"),
                li(2, 0, "Ý thứ hai"),
                li(2, 2, "Cấp ba"),
            ]
            .concat(),
            sect: s.clone(),
            numbering: Some(numbering(&[(0, multi), (1, bullets)], nums_basic)),
            ..Doc::default()
        },
    ));
    v.push(("lists-03-restart-override.docx", "lists", "Hai num cùng abstractNum, lvlOverride/startOverride khởi động lại", Doc {
        body: [li(1, 0, "A1"), li(1, 0, "A2"), li(3, 0, "B1 (bắt đầu lại từ 5)"), li(3, 0, "B2")].concat(),
        sect: s.clone(),
        numbering: Some(numbering(&[(0, multi), (1, bullets)], &format!(r#"{nums_basic}<w:num w:numId="3"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="5"/></w:lvlOverride></w:num>"#))),
        ..Doc::default()
    }));
    // Heading styles numbered through their numPr; the level comes from lvl/pStyle
    // (§17.9.23): Heading1's numPr has no ilvl.
    let linked_styles = default_styles().replacen(
        r#"<w:keepLines/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/>"#,
        r#"<w:keepLines/><w:numPr><w:numId w:val="1"/></w:numPr><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/>"#,
        1,
    ) + r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Heading1"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:sz w:val="28"/></w:rPr></w:style>"#;
    let linked_lvl = |ilvl: u8, fmt: &str, text: &str, style: &str| {
        let ind = 432 * (u32::from(ilvl) + 1);
        format!(
            r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:pStyle w:val="{style}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{ind}" w:hanging="432"/></w:pPr></w:lvl>"#
        )
    };
    v.push(("lists-04-heading-linked.docx", "lists", "Danh sách đa cấp liên kết Heading1/Heading2 qua lvl/pStyle và numPr của style (1., 1.1.)", Doc {
        body: [
            heading(1, "Tổng quan"),
            heading(2, "Phạm vi"),
            heading(2, "Thuật ngữ"),
            p("Nội dung"),
            heading(1, "Thiết kế"),
            heading(2, "Mô hình"),
        ]
        .concat(),
        sect: s.clone(),
        styles: Some(linked_styles),
        numbering: Some(format!(
            r#"<w:abstractNum w:abstractNumId="0"><w:nsid w:val="1A2B3C40"/><w:multiLevelType w:val="multilevel"/>{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
            linked_lvl(0, "decimal", "%1.", "Heading1"),
            linked_lvl(1, "decimal", "%1.%2.", "Heading2"),
        )),
        ..Doc::default()
    }));

    // §17.9.10 lvlRestart, §17.9.4 isLgl, and a restart at level 2 (§17.9.26 startOverride on
    // ilvl 1) that keeps the level-1 counter.
    let lvl = |ilvl: u8, fmt: &str, extra: &str, text: &str| {
        let ind = 720 * (u32::from(ilvl) + 1);
        format!(
            r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{extra}<w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{ind}" w:hanging="720"/></w:pPr></w:lvl>"#
        )
    };
    v.push(("lists-05-restart-legal.docx", "lists", "Danh sách đa cấp: cấp 3 chỉ bắt đầu lại sau cấp 1 (lvlRestart), kiểu pháp lý isLgl (I → 1), bắt đầu lại ở cấp 2 (startOverride ilvl 1) không đặt lại cấp 1", Doc {
        body: [
            li(1, 0, "Chương"),
            li(1, 1, "Mục"),
            li(1, 2, "Điều"),
            li(1, 1, "Mục"),
            li(1, 2, "Điều nối tiếp"),
            li(2, 1, "Mục bắt đầu lại từ e"),
            li(1, 0, "Chương"),
            li(1, 2, "Điều"),
        ]
        .concat(),
        sect: s.clone(),
        numbering: Some(format!(
            r#"<w:abstractNum w:abstractNumId="0"><w:nsid w:val="1A2B3C50"/><w:multiLevelType w:val="multilevel"/>{}{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="1"><w:startOverride w:val="5"/></w:lvlOverride></w:num>"#,
            lvl(0, "upperRoman", "", "%1."),
            lvl(1, "lowerLetter", "", "%2)"),
            lvl(2, "decimal", r#"<w:lvlRestart w:val="1"/><w:isLgl/>"#, "%1.%2.%3."),
        )),
        ..Doc::default()
    }));

    // ---- sections, headers/footers (§17.6, §17.10)
    let hdr = |t: &str| p(t);
    v.push(("sections-01-landscape-columns.docx", "sections", "Section dọc → ngang (nextPage) → 2 cột (continuous)", Doc {
        body: [
            lorem(2),
            r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 1</w:t></w:r></w:p>"#.to_owned(),
            lorem(2),
            r#"<w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section ngang</w:t></w:r></w:p>"#.to_owned(),
            lorem(12),
        ].concat(),
        sect: r#"<w:sectPr><w:type w:val="continuous"/><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:num="2" w:space="720"/></w:sectPr>"#.to_owned(),
        ..Doc::default()
    }));
    v.push(("sections-02-header-footer.docx", "sections", "Header/footer default + first (titlePg), số trang PAGE/NUMPAGES", Doc {
        body: lorem(80),
        sect: sect(&[hdr_ref("header", "default", "rId10"), hdr_ref("header", "first", "rId11"), hdr_ref("footer", "default", "rId12"), "<w:titlePg/>".to_owned()].concat()).replace("<w:titlePg/><w:pgSz", "<w:pgSz").replace("<w:docGrid", "<w:titlePg/><w:docGrid"),
        hf: vec![
            ("rId10".into(), "header", hdr("CÔNG TY GIẢ ĐỊNH – Báo cáo")),
            ("rId11".into(), "header", hdr("Trang bìa")),
            ("rId12".into(), "footer", format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Trang </w:t></w:r>{}<w:r><w:t>/</w:t></w:r>{}</w:p>"#, field_complex("PAGE", "1"), field_simple("NUMPAGES", "1"))),
        ],
        ..Doc::default()
    }));
    v.push(("sections-04-link-to-previous.docx", "sections", "Ba section: section 2 liên kết header/footer với section 1 (không có headerReference, §17.10.5), section 3 có header riêng", Doc {
        body: [
            lorem(1),
            r#"<w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rId10"/><w:footerReference w:type="default" r:id="rId11"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 1</w:t></w:r></w:p>"#.to_owned(),
            lorem(1),
            r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 2 (liên kết)</w:t></w:r></w:p>"#.to_owned(),
            lorem(1),
        ].concat(),
        sect: sect(&hdr_ref("header", "default", "rId12")),
        hf: vec![
            ("rId10".into(), "header", hdr("Đầu trang chung (section 1, 2)")),
            ("rId11".into(), "footer", hdr("Chân trang chung")),
            ("rId12".into(), "header", hdr("Đầu trang riêng section 3")),
        ],
        ..Doc::default()
    }));
    v.push(("sections-03-even-odd-restart.docx", "sections", "Header chẵn/lẻ (evenAndOddHeaders), section oddPage, đánh số lại pgNumType", Doc {
        body: [
            lorem(60),
            r#"<w:p><w:pPr><w:sectPr><w:headerReference w:type="default" r:id="rId10"/><w:headerReference w:type="even" r:id="rId11"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:pPr></w:p>"#.to_owned(),
            lorem(30),
        ].concat(),
        sect: r#"<w:sectPr><w:type w:val="oddPage"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:pgNumType w:start="1"/></w:sectPr>"#.to_owned(),
        hf: vec![
            ("rId10".into(), "header", format!("<w:p><w:pPr><w:jc w:val=\"right\"/></w:pPr>{}</w:p>", field_complex("PAGE", "1"))),
            ("rId11".into(), "header", format!("<w:p>{}</w:p>", field_complex("PAGE", "2"))),
        ],
        // CT_Settings order: defaultTabStop before evenAndOddHeaders (ECMA-376 Part 1, §17.15.1.78)
        settings: Some(r#"<w:defaultTabStop w:val="720"/><w:evenAndOddHeaders/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()),
        ..Doc::default()
    }));

    // ---- tables (§17.4)
    let rows: Vec<String> = (0..60)
        .map(|i| {
            row(
                "",
                &[
                    cell(1000, "", &i.to_string()),
                    cell(5000, "", "Nguyễn Văn A"),
                    cell(3355, "", "Hà Nội"),
                ],
            )
        })
        .collect();
    let mut long = vec![row(
        r#"<w:tblHeader/>"#,
        &[
            cell(
                1000,
                r#"<w:shd w:val="clear" w:color="auto" w:fill="1F3A68"/>"#,
                "STT",
            ),
            cell(5000, "", "Họ và tên"),
            cell(3355, "", "Địa chỉ"),
        ],
    )];
    long.extend(rows);
    v.push((
        "tables-01-header-rows-long.docx",
        "tables",
        "Bảng dài qua nhiều trang, lặp hàng tiêu đề, nền ô",
        Doc {
            body: [
                p("Danh sách"),
                table(
                    &[1000, 5000, 3355],
                    &long,
                    r#"<w:tblW w:w="9355" w:type="dxa"/>"#,
                ),
                p(""),
            ]
            .concat(),
            sect: s.clone(),
            ..Doc::default()
        },
    ));
    v.push((
        "tables-02-merge.docx",
        "tables",
        "Gộp ô ngang (gridSpan) và dọc (vMerge), vAlign, độ rộng pct",
        Doc {
            body: [
                table(
                    &[3118, 3118, 3119],
                    &[
                        row(
                            "",
                            &[
                                cell(
                                    6236,
                                    r#"<w:gridSpan w:val="2"/><w:vAlign w:val="center"/>"#,
                                    "Gộp ngang",
                                ),
                                cell(3119, r#"<w:vMerge w:val="restart"/>"#, "Gộp dọc"),
                            ],
                        ),
                        row(
                            r#"<w:trHeight w:val="567" w:hRule="atLeast"/>"#,
                            &[
                                cell(3118, "", "a"),
                                cell(3118, "", "b"),
                                cell(3119, r#"<w:vMerge/>"#, ""),
                            ],
                        ),
                        row(
                            r#"<w:cantSplit/>"#,
                            &[
                                cell(3118, "", "c"),
                                cell(3118, "", "d"),
                                cell(3119, "", "e"),
                            ],
                        ),
                    ],
                    r#"<w:tblW w:w="5000" w:type="pct"/><w:jc w:val="center"/>"#,
                ),
                p(""),
            ]
            .concat(),
            sect: s.clone(),
            ..Doc::default()
        },
    ));
    let inner = table(
        &[1500, 1500],
        &[row(
            "",
            &[cell(1500, "", "trong 1"), cell(1500, "", "trong 2")],
        )],
        "",
    );
    v.push(("tables-03-nested-borders.docx", "tables", "Bảng lồng nhau, viền ô riêng, lề ô, bảng không viền", Doc {
        body: [
            format!(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders><w:tblCellMar><w:left w:w="57" w:type="dxa"/><w:right w:w="57" w:type="dxa"/></w:tblCellMar><w:tblLook w:val="0000"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:tcBorders><w:right w:val="single" w:sz="12" w:space="0" w:color="FF0000"/></w:tcBorders></w:tcPr>{}{inner}{}</w:tc>{}</w:tr></w:tbl>"#, p("Ngoài"), p(""), cell(4000, "", "Ô phải")),
            p(""),
            table(&[4677, 4678], &[row("", &[cell(4677, "", "Không viền"), cell(4678, "", "x")])], r#"<w:tblBorders><w:top w:val="nil"/><w:left w:val="nil"/><w:bottom w:val="nil"/><w:right w:val="nil"/><w:insideH w:val="nil"/><w:insideV w:val="nil"/></w:tblBorders>"#),
            p(""),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- pictures (§20.4)
    let emu = 914_400 / 2;
    v.push(("images-01-inline.docx", "images", "Ảnh PNG inline trong dòng chữ, hai ảnh dùng chung part", Doc {
        body: format!("<w:p><w:r><w:t xml:space=\"preserve\">Ảnh: </w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> và </w:t></w:r>{}</w:p>{}", inline_pic(1, "rId20", emu, emu, "", ""), inline_pic(2, "rId20", emu * 2, emu, "", ""), lorem(1)),
        sect: s.clone(),
        images: vec![("rId20".into(), png(4, 4, [30, 120, 200]))],
        ..Doc::default()
    }));
    v.push((
        "images-02-anchored-wrap.docx",
        "images",
        "Ảnh nổi wrapTopAndBottom, wrapSquare, behindDoc (wrapNone)",
        Doc {
            body: [
                format!(
                    "<w:p>{}<w:r><w:t>Chữ dưới ảnh</w:t></w:r></w:p>",
                    anchor_pic(
                        1,
                        "rId20",
                        emu * 2,
                        emu,
                        r#"<wp:wrapTopAndBottom/>"#,
                        false,
                        "column",
                        "paragraph"
                    )
                ),
                lorem(3),
                format!(
                    "<w:p>{}<w:r><w:t>Bao quanh</w:t></w:r></w:p>",
                    anchor_pic(
                        2,
                        "rId21",
                        emu,
                        emu,
                        r#"<wp:wrapSquare wrapText="bothSides"/>"#,
                        false,
                        "margin",
                        "paragraph"
                    )
                ),
                format!(
                    "<w:p>{}<w:r><w:t>Sau chữ</w:t></w:r></w:p>",
                    anchor_pic(
                        3,
                        "rId20",
                        emu * 4,
                        emu * 2,
                        r#"<wp:wrapNone/>"#,
                        true,
                        "page",
                        "page"
                    )
                ),
            ]
            .concat(),
            sect: s.clone(),
            images: vec![
                ("rId20".into(), png(2, 2, [200, 50, 50])),
                ("rId21".into(), png(3, 2, [50, 160, 60])),
            ],
            ..Doc::default()
        },
    ));
    v.push((
        "images-03-crop-rotate.docx",
        "images",
        "Ảnh cắt (srcRect) và xoay (xfrm rot)",
        Doc {
            body: format!(
                "<w:p>{}</w:p><w:p>{}</w:p>",
                inline_pic(
                    1,
                    "rId20",
                    emu,
                    emu,
                    "",
                    r#"<a:srcRect l="10000" t="5000" r="20000" b="0"/>"#
                ),
                inline_pic(2, "rId20", emu, emu, r#" rot="5400000""#, "")
            ),
            sect: s.clone(),
            images: vec![("rId20".into(), png(8, 8, [240, 200, 0]))],
            ..Doc::default()
        },
    ));

    // ---- text wrapping around anchored pictures (§20.4.2.17 wrapSquare, §20.4.2.20 wrapTight)
    let inch = 914_400;
    let d = [0, 0, 114_300, 114_300];
    let para = |pic: String, n: usize| {
        format!(
            "<w:p>{pic}<w:r><w:t xml:space=\"preserve\">{} </w:t></w:r></w:p>",
            VI[n % VI.len()].repeat(3)
        )
    };
    v.push((
        "images-04-wrap-square-sides.docx",
        "images",
        "wrapSquare: ảnh bên trái (bothSides), giữa (bothSides — chữ hai bên), giữa wrapText=left, right, largest; distT/B/L/R",
        Doc {
            body: [
                para(wrap_pic(1, "rId20", (inch, inch), (0, 0), d, r#"<wp:wrapSquare wrapText="bothSides"/>"#), 0),
                para(wrap_pic(2, "rId21", (inch, inch), (2 * inch, 0), [57_150, 57_150, 114_300, 114_300], r#"<wp:wrapSquare wrapText="bothSides"/>"#), 1),
                para(wrap_pic(3, "rId20", (inch, inch * 3 / 4), (2 * inch, 0), d, r#"<wp:wrapSquare wrapText="left"/>"#), 2),
                para(wrap_pic(4, "rId21", (inch, inch * 3 / 4), (2 * inch, 0), d, r#"<wp:wrapSquare wrapText="right"/>"#), 3),
                para(wrap_pic(5, "rId20", (inch, inch * 3 / 4), (inch * 3 / 2, 0), d, r#"<wp:wrapSquare wrapText="largest"/>"#), 4),
            ]
            .concat(),
            sect: s.clone(),
            images: vec![
                ("rId20".into(), png(4, 4, [30, 120, 200])),
                ("rId21".into(), png(4, 4, [200, 90, 30])),
            ],
            ..Doc::default()
        },
    ));
    v.push((
        "images-05-wrap-tight.docx",
        "images",
        "wrapTight có wrapPolygon (đa giác cắt góc; đa giác rộng hơn ảnh, wrapText=largest), wrapThrough, distL/distR",
        Doc {
            body: [
                para(wrap_pic(1, "rId20", (inch * 3 / 2, inch), (0, 0), d, &tight("bothSides", &[(0, 0), (21_600, 0), (21_600, 10_800), (10_800, 21_600), (0, 21_600)])), 5),
                para(wrap_pic(2, "rId21", (inch, inch), (inch * 5 / 2, 0), [0, 0, 57_150, 57_150], &tight("largest", &[(-1_200, -600), (22_800, -600), (22_800, 22_200), (-1_200, 22_200)])), 6),
                para(wrap_pic(3, "rId20", (inch, inch), (inch * 4, 0), d, r#"<wp:wrapThrough wrapText="bothSides"><wp:wrapPolygon edited="0"><wp:start x="0" y="0"/><wp:lineTo x="0" y="21600"/><wp:lineTo x="21600" y="21600"/><wp:lineTo x="21600" y="0"/><wp:lineTo x="0" y="0"/></wp:wrapPolygon></wp:wrapThrough>"#), 7),
            ]
            .concat(),
            sect: s.clone(),
            images: vec![
                ("rId20".into(), png(6, 4, [60, 160, 90])),
                ("rId21".into(), png(4, 4, [160, 60, 160])),
            ],
            ..Doc::default()
        },
    ));
    v.push((
        "images-06-wrap-flow.docx",
        "images",
        "Ảnh wrapSquare cao trải qua nhiều đoạn, ảnh lệch xuống (posOffset dọc), hai ảnh hai bên cùng đoạn",
        Doc {
            body: [
                para(wrap_pic(1, "rId20", (inch * 5 / 4, inch * 3), (0, 0), d, r#"<wp:wrapSquare wrapText="bothSides"/>"#), 8),
                lorem(3),
                para(
                    [
                        wrap_pic(2, "rId21", (inch, inch), (0, inch / 2), d, r#"<wp:wrapSquare wrapText="bothSides"/>"#),
                        wrap_pic(3, "rId21", (inch, inch), (inch * 5, inch / 2), d, r#"<wp:wrapSquare wrapText="bothSides"/>"#),
                    ]
                    .concat(),
                    9,
                ),
                lorem(2),
            ]
            .concat(),
            sect: s.clone(),
            images: vec![
                ("rId20".into(), png(4, 8, [220, 180, 40])),
                ("rId21".into(), png(4, 4, [40, 120, 220])),
            ],
            ..Doc::default()
        },
    ));

    // ---- hyperlinks & bookmarks (§17.16.22, §17.13.6)
    v.push(("links-01-external.docx", "links", "Hyperlink ngoài (quan hệ External), kiểu Hyperlink", Doc {
        body: r#"<w:p><w:r><w:t xml:space="preserve">Xem </w:t></w:r><w:hyperlink r:id="rId30" w:history="1"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>trang thông tin</w:t></w:r></w:hyperlink><w:r><w:t>.</w:t></w:r></w:p>"#.to_owned(),
        sect: s.clone(),
        links: vec![("rId30".into(), "https://example.com/van-ban".into())],
        ..Doc::default()
    }));
    v.push(("links-02-bookmarks.docx", "links", "Bookmark và hyperlink nội bộ (w:anchor)", Doc {
        body: [
            r#"<w:p><w:bookmarkStart w:id="0" w:name="_Toc1"/><w:r><w:t>Mục tiêu</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>"#.to_owned(),
            lorem(2),
            r#"<w:p><w:hyperlink w:anchor="_Toc1"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>Về đầu mục</w:t></w:r></w:hyperlink></w:p>"#.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("links-03-ranges-permissions.docx", "links", "Bookmark lồng qua nhiều đoạn, permStart/permEnd, commentRange không có phần chú thích", Doc {
        body: [
            r#"<w:p><w:bookmarkStart w:id="1" w:name="VungA"/><w:permStart w:id="5" w:edGrp="everyone"/><w:r><w:t>Vùng được phép sửa</w:t></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:t>Tiếp tục vùng</w:t></w:r><w:permEnd w:id="5"/><w:bookmarkEnd w:id="1"/></w:p>"#.to_owned(),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- notes (§17.11)
    let fref = |id: i32| {
        format!(
            r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{id}"/></w:r>"#
        )
    };
    let fnote = |id: i32, t: &str| {
        format!(
            r#"<w:footnote w:id="{id}"><w:p><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> {t}</w:t></w:r></w:p></w:footnote>"#
        )
    };
    v.push(("notes-01-footnotes.docx", "notes", "Chú thích cuối trang (footnote) với dấu phân cách", Doc {
        body: format!("<w:p><w:r><w:t>Theo Luật</w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> và Nghị định</w:t></w:r>{}</w:p>{}", fref(1), fref(2), lorem(3)),
        sect: s.clone(),
        footnotes: Some(fnote(1, "Luật giả định số 01/2026.") + &fnote(2, "Nghị định giả định.")),
        ..Doc::default()
    }));
    v.push(("notes-04-numbering-options.docx", "notes", "footnotePr của section: numRestart eachSect, numFmt lowerRoman, numStart, pos beneathText (§17.11.11); hai section", Doc {
        body: format!(
            r#"<w:p><w:r><w:t>Phần một</w:t></w:r>{}<w:r><w:t xml:space="preserve"> và </w:t></w:r>{}</w:p><w:p><w:pPr><w:sectPr><w:footnotePr><w:numFmt w:val="decimal"/></w:footnotePr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết phần một</w:t></w:r></w:p><w:p><w:r><w:t>Phần hai</w:t></w:r>{}</w:p>"#,
            fref(1), fref(2), fref(3)
        ),
        sect: r#"<w:sectPr><w:footnotePr><w:pos w:val="beneathText"/><w:numFmt w:val="lowerRoman"/><w:numStart w:val="1"/><w:numRestart w:val="eachSect"/></w:footnotePr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr>"#.to_owned(),
        footnotes: Some(fnote(1, "Ghi chú một.") + &fnote(2, "Ghi chú hai.") + &fnote(3, "Ghi chú của phần hai (đánh số lại: i).")),
        ..Doc::default()
    }));
    v.push(("notes-02-endnotes.docx", "notes", "Chú thích cuối văn bản (endnote)", Doc {
        body: format!(r#"<w:p><w:r><w:t>Tài liệu tham khảo</w:t></w:r><w:r><w:endnoteReference w:id="1"/></w:r></w:p>{}"#, lorem(2)),
        sect: s.clone(),
        endnotes: Some(r#"<w:endnote w:id="1"><w:p><w:r><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> Nguồn giả định.</w:t></w:r></w:p></w:endnote>"#.to_owned()),
        ..Doc::default()
    }));
    v.push((
        "notes-03-many-footnotes.docx",
        "notes",
        "Nhiều footnote dồn trang, footnote nhiều đoạn",
        Doc {
            body: (1..=12)
                .map(|i| {
                    format!(
                        "<w:p><w:r><w:t>{} {}</w:t></w:r>{}</w:p>",
                        i,
                        VI[i as usize % 4],
                        fref(i)
                    )
                })
                .collect(),
            sect: s.clone(),
            footnotes: Some(
                (1..=12)
                    .map(|i| fnote(i, &format!("Ghi chú số {i}.")))
                    .collect::<String>()
                    + "",
            ),
            ..Doc::default()
        },
    ));

    // ---- fields (§17.16)
    v.push(("fields-01-simple.docx", "fields", "fldSimple PAGE, NUMPAGES, DATE", Doc {
        body: format!("<w:p><w:r><w:t xml:space=\"preserve\">Trang </w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> / </w:t></w:r>{}</w:p><w:p>{}</w:p>", field_simple("PAGE", "1"), field_simple("NUMPAGES", "1"), field_simple("DATE \\@ \"dd/MM/yyyy\"", "05/11/2026")),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("fields-02-complex.docx", "fields", "Field phức (fldChar begin/separate/end, instrText) PAGE, DATE, REF", Doc {
        body: format!("<w:p>{}</w:p><w:p>{}</w:p><w:p><w:bookmarkStart w:id=\"3\" w:name=\"Ten\"/><w:r><w:t>BlueDoc</w:t></w:r><w:bookmarkEnd w:id=\"3\"/>{}</w:p>", field_complex("PAGE \\* MERGEFORMAT", "1"), field_complex("DATE \\@ \"dddd, d MMMM yyyy\"", "Thứ Bảy, 7 tháng 11 2026"), field_complex("REF Ten \\h", "BlueDoc")),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("fields-03-toc.docx", "fields", "Mục lục TOC với kết quả cache, sdt docPartGallery", Doc {
        body: format!(r#"<w:sdt><w:sdtPr><w:id w:val="-123456"/><w:docPartObj><w:docPartGallery w:val="Table of Contents"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent><w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9355"/></w:tabs></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h \z \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Chương I</w:t></w:r><w:r><w:tab/><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:sdtContent></w:sdt>{}{}"#, heading(1, "Chương I"), lorem(2)),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- unknown / extension markup (prompt §5.1)
    v.push(("unknown-01-sdt-customxml.docx", "unknown", "Content control (w:sdt) cấp khối và cấp run, customXml, smartTag", Doc {
        body: [
            r#"<w:sdt><w:sdtPr><w:alias w:val="Tiêu đề"/><w:tag w:val="title"/><w:id w:val="1001"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Tiêu đề trong content control</w:t></w:r></w:p></w:sdtContent></w:sdt>"#.to_owned(),
            r#"<w:p><w:r><w:t xml:space="preserve">Ngày: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="1002"/><w:date><w:dateFormat w:val="dd/MM/yyyy"/><w:lid w:val="vi-VN"/></w:date></w:sdtPr><w:sdtContent><w:r><w:t>07/11/2026</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            r#"<w:customXml w:uri="urn:bluestar:test" w:element="khoi"><w:p><w:r><w:t>Trong customXml</w:t></w:r></w:p></w:customXml>"#.to_owned(),
            r#"<w:p><w:smartTag w:uri="urn:schemas-microsoft-com:office:smarttags" w:element="place"><w:r><w:t>Hà Nội</w:t></w:r></w:smartTag></w:p>"#.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // Form content controls (§17.5.2; [MS-DOCX] w14:checkbox, w15:repeatingSection).
    v.push(("controls-01-forms.docx", "controls", "Content control biểu mẫu: văn bản thuần (placeholder), hộp kiểm w14 đã/chưa chọn, danh sách thả xuống, hộp kết hợp, chọn ngày, nhóm nhiều đoạn có khoá", Doc {
        body: [
            r#"<w:p><w:r><w:t xml:space="preserve">Họ tên: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Họ tên"/><w:tag w:val="ho_ten"/><w:id w:val="2001"/><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:color w:val="808080"/></w:rPr><w:t>Nhập họ tên</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            r#"<w:p><w:sdt><w:sdtPr><w:id w:val="2002"/><w14:checkbox><w14:checked w14:val="1"/><w14:checkedState w14:val="2612" w14:font="MS Gothic"/><w14:uncheckedState w14:val="2610" w14:font="MS Gothic"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>☒</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> Đồng ý điều khoản   </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="2003"/><w14:checkbox><w14:checked w14:val="0"/><w14:checkedState w14:val="2612" w14:font="MS Gothic"/><w14:uncheckedState w14:val="2610" w14:font="MS Gothic"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>☐</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> Nhận bản tin</w:t></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:t xml:space="preserve">Phòng ban: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Phòng ban"/><w:id w:val="2004"/><w:dropDownList><w:listItem w:displayText="Kế toán" w:value="KT"/><w:listItem w:displayText="Kỹ thuật" w:value="KTh"/><w:listItem w:displayText="Kinh doanh" w:value="KD"/></w:dropDownList></w:sdtPr><w:sdtContent><w:r><w:t>Kỹ thuật</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve">   Tỉnh: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="2005"/><w:comboBox><w:listItem w:displayText="Hà Nội" w:value="HN"/><w:listItem w:displayText="Đà Nẵng" w:value="DN"/></w:comboBox></w:sdtPr><w:sdtContent><w:r><w:t>Huế</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:t xml:space="preserve">Ngày ký: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="2006"/><w:date w:fullDate="2026-11-09T00:00:00Z"><w:dateFormat w:val="dd/MM/yyyy"/><w:lid w:val="vi-VN"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent><w:r><w:t>09/11/2026</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            r#"<w:sdt><w:sdtPr><w:alias w:val="Điều khoản"/><w:tag w:val="dieu_khoan"/><w:id w:val="2007"/><w:lock w:val="sdtContentLocked"/><w:richText/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Điều 1. Hai bên cam kết thực hiện đúng hợp đồng.</w:t></w:r></w:p><w:p><w:r><w:t>Điều 2. Tranh chấp được giải quyết bằng thương lượng.</w:t></w:r></w:p></w:sdtContent></w:sdt>"#.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // Repeating sections ([MS-DOCX] w15:repeatingSection / w15:repeatingSectionItem), a picture
    // control showing its placeholder (§17.5.2.26 picture, §17.5.2.39 showingPlcHdr) and a
    // locked, titled text control (§17.5.2.1 alias, §17.5.2.42 tag, §17.5.2.23 lock).
    let item = |id: u32, name: &str| {
        format!(
            r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent><w:p><w:r><w:t xml:space="preserve">Thành viên: {name}</w:t></w:r></w:p></w:sdtContent></w:sdt>"#
        )
    };
    v.push(("controls-04-repeating-picture.docx", "controls", "Phần lặp lại (w15:repeatingSection) có hai mục, điều khiển ảnh đang hiện chỗ giữ, điều khiển văn bản có tiêu đề/thẻ/khoá không cho xoá", Doc {
        body: [
            format!(r#"<w:sdt><w:sdtPr><w:alias w:val="Danh sách"/><w:tag w:val="thanh_vien"/><w:id w:val="4001"/><w15:repeatingSection/></w:sdtPr><w:sdtContent>{}{}</w:sdtContent></w:sdt>"#, item(4002, "An"), item(4003, "Bình")),
            r#"<w:p><w:r><w:t xml:space="preserve">Ảnh: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Ảnh chân dung"/><w:id w:val="4004"/><w:showingPlcHdr/><w:picture/></w:sdtPr><w:sdtContent><w:r><w:t>Bấm vào đây để chèn ảnh.</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:t xml:space="preserve">Mã hồ sơ: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Mã hồ sơ"/><w:tag w:val="ma_ho_so"/><w:id w:val="4005"/><w:lock w:val="sdtLocked"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>HS-2026-07</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("controls-05-appearance-gallery-eq.docx", "controls", "Điều khiển hiện dạng thẻ / không hiện (w15:appearance) có màu (w15:color), điều khiển tạm (w:temporary), danh sách có giá trị riêng, ngày tiếng Anh (w:lid en-US), thư viện khối dựng sẵn (w:docPartList), trường EQ (phân số, căn, mũ, khung, ngoặc, mảng)", Doc {
        body: [
            r#"<w:p><w:r><w:t xml:space="preserve">Tỉnh: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Tỉnh"/><w:id w:val="5001"/><w15:color w:val="C00000"/><w15:appearance w15:val="tags"/><w:dropDownList><w:listItem w:displayText="Hà Nội" w:value="HN"/><w:listItem w:displayText="Huế" w:value="HUE"/></w:dropDownList></w:sdtPr><w:sdtContent><w:r><w:t>Huế</w:t></w:r></w:sdtContent></w:sdt></w:p>"#,
            r#"<w:p><w:r><w:t xml:space="preserve">Ghi chú: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="5002"/><w:temporary/><w:showingPlcHdr/><w15:appearance w15:val="hidden"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>Bấm vào đây để nhập văn bản.</w:t></w:r></w:sdtContent></w:sdt></w:p>"#,
            r#"<w:p><w:r><w:t xml:space="preserve">Date: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="5003"/><w:date w:fullDate="2026-11-12T00:00:00Z"><w:dateFormat w:val="MMMM d, yyyy"/><w:lid w:val="en-US"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent><w:r><w:t>November 12, 2026</w:t></w:r></w:sdtContent></w:sdt></w:p>"#,
            r#"<w:sdt><w:sdtPr><w:alias w:val="Khối"/><w:id w:val="5004"/><w:showingPlcHdr/><w:docPartList><w:docPartGallery w:val="Quick Parts"/><w:docPartCategory w:val="General"/></w:docPartList></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Chọn một khối dựng sẵn.</w:t></w:r></w:p></w:sdtContent></w:sdt>"#,
            r#"<w:p><w:r><w:t xml:space="preserve">EQ: </w:t></w:r><w:fldSimple w:instr=" EQ \f(1,2) "><w:r><w:t>1/2</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> </w:t></w:r><w:fldSimple w:instr=" EQ \r(3,x) "><w:r><w:t>x</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> </w:t></w:r><w:fldSimple w:instr=" EQ x\s\up8(2) "><w:r><w:t>x2</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> </w:t></w:r><w:fldSimple w:instr=" EQ \x(A) "><w:r><w:t>A</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> </w:t></w:r><w:fldSimple w:instr=" EQ \b\bc\{(\a\co2(1,2,3,4)) "><w:r><w:t>1 2 3 4</w:t></w:r></w:fldSimple></w:p>"#,
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // A repeating section of table rows (§17.5.2.32 sdt around w:tr, CT_SdtContentRow; [MS-DOCX]
    // w15:repeatingSection / w15:repeatingSectionItem): a header row, two items of one row each,
    // a total row.
    let row_item = |id: u32, name: &str, role: &str| {
        format!(
            r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>"#,
            row("", &[cell(4677, "", name), cell(4678, "", role)])
        )
    };
    v.push(("controls-06-repeating-rows.docx", "controls", "Phần lặp lại theo dòng bảng (w:sdt bao w:tr, w15:repeatingSection) có hai mục, giữa dòng tiêu đề và dòng tổng", Doc {
        body: [
            table(
                &[4677, 4678],
                &[
                    row("<w:tblHeader/>", &[cell(4677, "", "Họ tên"), cell(4678, "", "Vai trò")]),
                    format!(r#"<w:sdt><w:sdtPr><w:alias w:val="Thành viên"/><w:tag w:val="thanh_vien"/><w:id w:val="6001"/><w15:repeatingSection/></w:sdtPr><w:sdtContent>{}{}</w:sdtContent></w:sdt>"#, row_item(6002, "An", "Trưởng nhóm"), row_item(6003, "Bình", "Thành viên")),
                    row("", &[cell(4677, "", "Tổng"), cell(4678, "", "2 người")]),
                ],
                r#"<w:tblW w:w="0" w:type="auto"/>"#,
            ),
            p(""),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // EQ field spacing options (ECMA-376 Part 1, §17.16.5.17): \a\vs \hs, \s\ai \di, \d\li.
    v.push(("fields-07-eq-spacing.docx", "fields", "Trường EQ với tuỳ chọn khoảng cách: mảng \\a\\vs\\hs, xếp chồng \\s\\ai\\di, dịch chuyển \\d\\fo\\li (gạch dưới)", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Mảng: </w:t></w:r>{}</w:p>"#, field_simple(r"EQ \a\co2\vs6\hs12(1,2,3,4)", "1 2 3 4")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Xếp chồng: </w:t></w:r>{}</w:p>"#, field_simple(r"EQ \s\ai4\di4(a,b)", "a b")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Họ tên: </w:t></w:r>{}</w:p>"#, field_simple(r"EQ \d\fo60\li()", " ")),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // ---- legacy form fields (§17.16.17 ffData) and forms protection (§17.15.1.29, §17.6.6)
    let ff = |name: &str, id: u32, ff: &str, instr: &str, result: &str| -> String {
        let sep = if result.is_empty() && instr != "FORMTEXT" {
            String::new()
        } else {
            r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r>"#.to_owned()
        };
        let res = if result.is_empty() {
            String::new()
        } else {
            format!(r#"<w:r><w:t xml:space="preserve">{result}</w:t></w:r>"#)
        };
        format!(
            r#"<w:bookmarkStart w:id="{id}" w:name="{name}"/><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="{name}"/>{ff}</w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r>{sep}{res}<w:r><w:fldChar w:fldCharType="end"/></w:r><w:bookmarkEnd w:id="{id}"/>"#
        )
    };
    let run = |t: &str| format!(r#"<w:r><w:t xml:space="preserve">{t}</w:t></w:r>"#);
    v.push(("controls-02-legacy-forms.docx", "controls", "Trường biểu mẫu kiểu cũ (FORMTEXT thường/số/ngày/tính toán, FORMCHECKBOX tự động/cỡ cố định, FORMDROPDOWN), macro và trợ giúp giữ nguyên, REF tới bookmark trường, bảo vệ “chỉ điền biểu mẫu” với một section không bảo vệ (formProt)", Doc {
        body: [
            format!("<w:p>{}{}</w:p>", run("Họ và tên: "), ff("HoTen", 1, r#"<w:enabled/><w:calcOnExit w:val="0"/><w:entryMacro w:val="VaoHoTen"/><w:helpText w:type="text" w:val="Ghi đầy đủ họ tên"/><w:statusText w:type="text" w:val="Họ tên"/><w:textInput><w:maxLength w:val="40"/><w:format w:val="TITLE CASE"/></w:textInput>"#, "FORMTEXT", "Nguyễn Văn An")),
            format!("<w:p>{}{}</w:p>", run("Ngày sinh: "), ff("NgaySinh", 2, r#"<w:enabled/><w:calcOnExit w:val="0"/><w:textInput><w:type w:val="date"/><w:format w:val="dd/MM/yyyy"/></w:textInput>"#, "FORMTEXT", "01/02/1990")),
            format!("<w:p>{}{}{}{}</w:p>", run("Số lượng: "), ff("SoLuong", 3, r##"<w:enabled/><w:calcOnExit/><w:textInput><w:type w:val="number"/><w:default w:val="1"/><w:format w:val="#,##0"/></w:textInput>"##, "FORMTEXT", "3"), run("   Thành tiền: "), ff("ThanhTien", 4, r##"<w:enabled w:val="0"/><w:calcOnExit w:val="0"/><w:textInput><w:type w:val="calculated"/><w:default w:val="=SoLuong*150000"/><w:format w:val="#,##0"/></w:textInput>"##, "FORMTEXT", "450,000")),
            format!("<w:p>{}{}{}{}</w:p>", ff("DongY", 5, r#"<w:enabled/><w:calcOnExit w:val="0"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/><w:checked/></w:checkBox>"#, "FORMCHECKBOX", ""), run(" Đồng ý điều khoản   "), ff("NhanThu", 6, r#"<w:enabled/><w:calcOnExit w:val="0"/><w:exitMacro w:val="RaNhanThu"/><w:checkBox><w:size w:val="28"/><w:default w:val="0"/></w:checkBox>"#, "FORMCHECKBOX", ""), run(" Nhận thư")),
            format!("<w:p>{}{}</w:p>", run("Tỉnh/thành: "), ff("Tinh", 7, r#"<w:enabled/><w:calcOnExit w:val="0"/><w:ddList><w:result w:val="1"/><w:listEntry w:val="Hà Nội"/><w:listEntry w:val="Huế"/><w:listEntry w:val="Đà Nẵng"/><w:listEntry w:val="Cần Thơ"/></w:ddList>"#, "FORMDROPDOWN", "")),
            format!(r#"<w:p><w:pPr>{}</w:pPr>{}{}</w:p>"#, sect(""), run("Xác nhận: "), field_complex("REF HoTen \\h", "Nguyễn Văn An")),
            p("Ghi chú (phần này không bị bảo vệ, sửa tự do):"),
        ].concat(),
        sect: sect(r#"<w:type w:val="continuous"/>"#).replace("<w:cols w:space=\"708\"/>", "<w:cols w:space=\"708\"/><w:formProt w:val=\"0\"/>"),
        settings: Some(r#"<w:zoom w:percent="100"/><w:documentProtection w:edit="forms" w:enforcement="1"/><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()),
        ..Doc::default()
    }));
    // Content controls bound to a custom XML part (§17.5.2.6 dataBinding, §15.2.4).
    let bind = |xpath: &str| {
        format!(
            r#"<w:dataBinding w:prefixMappings="xmlns:ns0='urn:bluestar:corpus:don-hang'" w:xpath="{xpath}" w:storeItemID="{{3B0F6A52-1C7D-4E8A-9B21-6D4C0E5F7A13}}"/>"#
        )
    };
    v.push(("controls-03-data-binding.docx", "controls", "Content control gắn dữ liệu XML tuỳ chỉnh (customXml/item1.xml + itemProps1.xml): hai ô văn bản cùng một nút, hộp kiểm, ngày (storeMappedDataAs date), danh sách theo giá trị, thuộc tính @ma, một nút không tồn tại giữ nội dung cũ", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Khách hàng: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Khách hàng"/><w:id w:val="3001"/>{}<w:text/></w:sdtPr><w:sdtContent><w:r><w:t>Công ty Sao Xanh</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve">   Mã đơn: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="3002"/>{}<w:text/></w:sdtPr><w:sdtContent><w:r><w:t>DH-0042</w:t></w:r></w:sdtContent></w:sdt></w:p>"#, bind("/ns0:donHang[1]/ns0:khachHang[1]"), bind("/ns0:donHang[1]/@ma")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Kính gửi </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="3003"/>{}<w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:b/></w:rPr><w:t>Công ty Sao Xanh</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t>,</w:t></w:r></w:p>"#, bind("/ns0:donHang[1]/ns0:khachHang[1]")),
            format!(r#"<w:p><w:sdt><w:sdtPr><w:id w:val="3004"/>{}<w14:checkbox><w14:checked w14:val="1"/><w14:checkedState w14:val="2612" w14:font="MS Gothic"/><w14:uncheckedState w14:val="2610" w14:font="MS Gothic"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>☒</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> Giao hàng tận nơi</w:t></w:r></w:p>"#, bind("/ns0:donHang[1]/ns0:giaoTanNoi[1]")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Ngày giao: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="3005"/>{}<w:date w:fullDate="2026-12-15T00:00:00Z"><w:dateFormat w:val="dd/MM/yyyy"/><w:lid w:val="vi-VN"/><w:storeMappedDataAs w:val="date"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent><w:r><w:t>15/12/2026</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve">   Hình thức: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="3006"/>{}<w:dropDownList><w:listItem w:displayText="Chuyển khoản" w:value="CK"/><w:listItem w:displayText="Tiền mặt" w:value="TM"/></w:dropDownList></w:sdtPr><w:sdtContent><w:r><w:t>Chuyển khoản</w:t></w:r></w:sdtContent></w:sdt></w:p>"#, bind("/ns0:donHang[1]/ns0:ngayGiao[1]"), bind("/ns0:donHang[1]/ns0:thanhToan[1]")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Ghi chú: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="3007"/>{}<w:text/></w:sdtPr><w:sdtContent><w:r><w:t>(không có nút này trong dữ liệu)</w:t></w:r></w:sdtContent></w:sdt></w:p>"#, bind("/ns0:donHang[1]/ns0:ghiChu[1]")),
        ].concat(),
        sect: s.clone(),
        custom_xml: vec![(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>
<dh:donHang xmlns:dh=\"urn:bluestar:corpus:don-hang\" ma=\"DH-0042\">
  <dh:khachHang>Công ty Sao Xanh</dh:khachHang>
  <dh:giaoTanNoi>true</dh:giaoTanNoi>
  <dh:ngayGiao>2026-12-15</dh:ngayGiao>
  <dh:thanhToan>CK</dh:thanhToan>
</dh:donHang>".to_owned(),
            "{3B0F6A52-1C7D-4E8A-9B21-6D4C0E5F7A13}".to_owned(),
        )],
        ..Doc::default()
    }));
    // ---- citations and bibliography (§22.6 b:Sources, §17.16.5.8 CITATION, §17.16.5.7
    // BIBLIOGRAPHY); the cached results are the ones BlueDoc's own style formatters give.
    let cite_styles = format!(
        r#"{}<w:style w:type="paragraph" w:styleId="Bibliography"><w:name w:val="bibliography"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:ind w:left="720" w:hanging="720"/></w:pPr></w:style>"#,
        default_styles()
    );
    v.push(("cite-01-apa-author-date.docx", "cite", "Nguồn trích dẫn (customXml b:Sources, StyleName APA): sách hai tác giả, bài báo ba tác giả có DOI, báo cáo của tổ chức, nguồn giữ chỗ; trường CITATION phức trong sdt w:citation (một nguồn, nhiều nguồn \\m kèm trang \\p, nguồn giữ chỗ) và BIBLIOGRAPHY trong sdt w:bibliography", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Phương pháp nghiên cứu được trình bày trong </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_sdt(4101, "CITATION Smi20 \\l 1033", "(Smith &amp; Doe, 2020)")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Kết quả gần đây </w:t></w:r>{}<w:r><w:t xml:space="preserve"> cho thấy xu hướng tương tự.</w:t></w:r></w:p>"#, cite_sdt(4102, "CITATION Ngu21 \\l 1033 \\p 105 \\m WHO22", "(Nguyen et al., 2021, p. 105; World Health Organization, 2022)")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Nguồn sẽ bổ sung sau </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_sdt(4103, "CITATION Placeholder1 \\l 1033", "(Placeholder1)")),
            heading(1, "Tài liệu tham khảo"),
            bibliography_sdt(4104, "BIBLIOGRAPHY", &[
                "Nguyen, M. T., Smith, J. K., &amp; Tran, L. (2021). Deep learning for text. *Journal of Data*, *12*(3), 100–120. https://doi.org/10.1000/jd.2021.5",
                "Smith, J. K., &amp; Doe, J. (2020). *Research methods* (2nd ed.). Blue Press.",
                "World Health Organization. (2022). *Global report*. WHO Press.",
            ]),
            p("Hết."),
        ].concat(),
        sect: s.clone(),
        styles: Some(cite_styles.clone()),
        custom_xml: vec![(
            cite_sources("APA", &[
                cite_source("Smi20", "Book", &[("Smith", "John K."), ("Doe", "Jane")], "", &[("Title", "Research methods"), ("Year", "2020"), ("Edition", "2"), ("Publisher", "Blue Press"), ("City", "Hanoi"), ("LCID", "1033")]),
                cite_source("Ngu21", "JournalArticle", &[("Nguyen", "Minh T."), ("Smith", "John K."), ("Tran", "Lan")], "", &[("Title", "Deep learning for text"), ("JournalName", "Journal of Data"), ("Year", "2021"), ("Volume", "12"), ("Issue", "3"), ("Pages", "100-120"), ("DOI", "10.1000/jd.2021.5")]),
                cite_source("WHO22", "Report", &[], "World Health Organization", &[("Title", "Global report"), ("Year", "2022"), ("Publisher", "WHO Press"), ("City", "Geneva")]),
                cite_source("Placeholder1", "Book", &[], "", &[]),
            ]),
            "{7C1E9A40-52B3-4D6F-8A17-3E5B9C0D2F61}".to_owned(),
        )],
        ..Doc::default()
    }));
    v.push(("cite-02-ieee-numeric.docx", "cite", "Trích dẫn kiểu số theo thứ tự xuất hiện (StyleName IEEE): chương sách có chủ biên, bài hội nghị, trang web có ngày truy cập; trường CITATION dạng fldSimple (kèm trang, hai nguồn) và danh mục có nhãn [n] + tab", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Thuật toán phân tích cú pháp nhanh </w:t></w:r>{}<w:r><w:t xml:space="preserve"> dựa trên kỹ thuật lấy mẫu </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_simple("CITATION Gar18", "[1]"), cite_simple("CITATION Lee19 \\p 50-52", "[2, pp. 50–52]")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Xem thêm </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_simple("CITATION Gar18 \\m Blu23", "[1], [3]")),
            heading(1, "References"),
            bibliography_sdt(4201, "BIBLIOGRAPHY", &[
                "[1]\tL. Garcia, “Fast parsing,” in *Proceedings of the International Conference on Parsing*, Paris, 2018, pp. 10–15.",
                "[2]\tA. Lee, “Sampling,” in *Handbook of statistics*, M. Brown, Ed. London: Blue Press, 2019, pp. 45–67.",
                "[3]\tBlue Docs Team. “Getting started.” Blue Docs. Accessed: Jan. 15, 2024. [Online]. Available: https://example.org/start",
            ]),
        ].concat(),
        sect: s.clone(),
        styles: Some(cite_styles.clone()),
        custom_xml: vec![(
            cite_sources("IEEE", &[
                r#"<b:Source><b:Tag>Lee19</b:Tag><b:SourceType>BookSection</b:SourceType><b:Author><b:Author><b:NameList><b:Person><b:Last>Lee</b:Last><b:First>Anna</b:First></b:Person></b:NameList></b:Author><b:Editor><b:NameList><b:Person><b:Last>Brown</b:Last><b:First>Mark</b:First></b:Person></b:NameList></b:Editor></b:Author><b:Title>Sampling</b:Title><b:BookTitle>Handbook of statistics</b:BookTitle><b:Year>2019</b:Year><b:Pages>45-67</b:Pages><b:Publisher>Blue Press</b:Publisher><b:City>London</b:City></b:Source>"#.to_owned(),
                cite_source("Gar18", "ConferenceProceedings", &[("Garcia", "Luis")], "", &[("Title", "Fast parsing"), ("ConferenceName", "Proceedings of the International Conference on Parsing"), ("Year", "2018"), ("Pages", "10-15"), ("Publisher", "ACM"), ("City", "Paris")]),
                cite_source("Blu23", "InternetSite", &[], "Blue Docs Team", &[("Title", "Getting started"), ("InternetSiteTitle", "Blue Docs"), ("Year", "2023"), ("Month", "5"), ("Day", "4"), ("URL", "https://example.org/start"), ("YearAccessed", "2024"), ("MonthAccessed", "1"), ("DayAccessed", "15")]),
            ]),
            "{2A6D4F18-9B7C-4E35-A1D0-6F8E3C5B7A92}".to_owned(),
        )],
        ..Doc::default()
    }));
    v.push(("cite-03-vn-thesis.docx", "cite", "Danh mục tài liệu tham khảo kiểu luận văn Việt Nam (StyleName VNThesis): tài liệu tiếng Việt trước (xếp theo tên tác giả, không có tác giả thì theo cơ quan ban hành), tài liệu nước ngoài sau (xếp theo họ); văn bản pháp luật; trích dẫn [số, tr. x-y]; BIBLIOGRAPHY \\l 1066", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Lúa lai đã được nghiên cứu từ sớm </w:t></w:r>{}<w:r><w:t xml:space="preserve">, </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_sdt(4301, "CITATION Qua92 \\l 1066 \\p 10-16", "[2, tr. 10-16]"), cite_sdt(4302, "CITATION And85 \\l 1066 \\m Bor97", "[5], [6]")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Việc xử lý dữ liệu cá nhân tuân theo quy định hiện hành </w:t></w:r>{}<w:r><w:t xml:space="preserve"> và giáo trình </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cite_sdt(4303, "CITATION ND13 \\l 1066", "[4]"), cite_sdt(4304, "CITATION Ngu15 \\l 1066", "[1]")),
            heading(1, "TÀI LIỆU THAM KHẢO"),
            bibliography_sdt(4305, "BIBLIOGRAPHY \\l 1066", &[
                "[1]\tNguyễn Văn An, Trần Thị Bình (2015), *Giáo trình thống kê*, Nhà xuất bản Giáo dục, Hà Nội.",
                "[2]\tQuách Ngọc Ân (1992), “Nhìn lại hai năm phát triển lúa lai”, *Di truyền học ứng dụng*, 98(1), tr. 10-16.",
                "[3]\tBộ Nông nghiệp &amp; PTNT (1996), *Báo cáo tổng kết 5 năm (1992-1996) phát triển lúa lai*, Hà Nội.",
                "[4]\tChính phủ (2023), *Nghị định quy định về bảo vệ dữ liệu cá nhân*, số 13/2023/NĐ-CP, ngày 17/04/2023.",
                "[5]\tAnderson J.E. (1985), “The Relative Inefficiency of Quota”, *American Economic Review*, 75(1), pp. 178-90.",
                "[6]\tBorkakati R.P., Virmani S.S. (1997), “Genetics of thermosensitive genic male sterility in Rice”, *Euphytica*, 88, pp. 1-7.",
            ]),
        ].concat(),
        sect: s.clone(),
        styles: Some(cite_styles),
        custom_xml: vec![(
            cite_sources("VNThesis", &[
                cite_source("And85", "JournalArticle", &[("Anderson", "J. E.")], "", &[("Title", "The Relative Inefficiency of Quota"), ("JournalName", "American Economic Review"), ("Year", "1985"), ("Volume", "75"), ("Issue", "1"), ("Pages", "178-90"), ("LCID", "1033")]),
                cite_source("Qua92", "JournalArticle", &[("Quách", "Ngọc Ân")], "", &[("Title", "Nhìn lại hai năm phát triển lúa lai"), ("JournalName", "Di truyền học ứng dụng"), ("Year", "1992"), ("Volume", "98"), ("Issue", "1"), ("Pages", "10-16"), ("LCID", "1066")]),
                cite_source("Bo96", "Report", &[], "Bộ Nông nghiệp &amp; PTNT", &[("Title", "Báo cáo tổng kết 5 năm (1992-1996) phát triển lúa lai"), ("Year", "1996"), ("City", "Hà Nội"), ("LCID", "1066")]),
                cite_source("Ngu15", "Book", &[("Nguyễn", "Văn An"), ("Trần", "Thị Bình")], "", &[("Title", "Giáo trình thống kê"), ("Year", "2015"), ("Publisher", "Nhà xuất bản Giáo dục"), ("City", "Hà Nội"), ("LCID", "1066")]),
                cite_source("Bor97", "JournalArticle", &[("Borkakati", "R. P."), ("Virmani", "S. S.")], "", &[("Title", "Genetics of thermosensitive genic male sterility in Rice"), ("JournalName", "Euphytica"), ("Year", "1997"), ("Volume", "88"), ("Pages", "1-7"), ("LCID", "1033")]),
                cite_source("ND13", "Case", &[], "", &[("Title", "Nghị định quy định về bảo vệ dữ liệu cá nhân"), ("CaseNumber", "13/2023/NĐ-CP"), ("Court", "Chính phủ"), ("Year", "2023"), ("Month", "4"), ("Day", "17"), ("LCID", "1066")]),
            ]),
            "{5E3B7D29-0A4C-4F81-B6E2-9D1C8A7F4B03}".to_owned(),
        )],
        ..Doc::default()
    }));
    v.push(("unknown-02-alternate-content.docx", "unknown", "mc:AlternateContent (Choice wps / Fallback VML), w14 trong rPr, ký hiệu w:sym", Doc {
        body: [
            r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="w14"><w14:dummy/><w:t>Choice</w:t></mc:Choice><mc:Fallback><w:t>Fallback</w:t></mc:Fallback></mc:AlternateContent></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:rPr><w14:textOutline w14:w="9525" w14:cap="flat" w14:cmpd="sng" w14:algn="ctr"><w14:solidFill><w14:srgbClr w14:val="000000"/></w14:solidFill><w14:prstDash w14:val="solid"/><w14:round/></w14:textOutline><w14:ligatures w14:val="standardContextual"/></w:rPr><w:t>Chữ có hiệu ứng w14</w:t></w:r></w:p>"#.to_owned(),
            r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F0FC"/><w:t xml:space="preserve"> đã kiểm</w:t></w:r></w:p>"#.to_owned(),
            r##"<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:object w:dxaOrig="1440" w:dyaOrig="1440"><v:shape id="_x0000_i1025" type="#_x0000_t75" style="width:72pt;height:72pt"/></w:object></w:r></w:p>"##.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("unknown-03-tracked-changes.docx", "unknown", "Theo dõi thay đổi w:ins / w:del, rPrChange, pPrChange", Doc {
        body: [
            r#"<w:p><w:r><w:t xml:space="preserve">Văn bản </w:t></w:r><w:ins w:id="10" w:author="Người dùng A" w:date="2026-11-07T08:00:00Z"><w:r><w:t>được chèn</w:t></w:r></w:ins><w:del w:id="11" w:author="Người dùng A" w:date="2026-11-07T08:01:00Z"><w:r><w:delText xml:space="preserve"> bị xoá</w:delText></w:r></w:del></w:p>"#.to_owned(),
            r#"<w:p><w:pPr><w:jc w:val="center"/><w:pPrChange w:id="12" w:author="B" w:date="2026-11-07T09:00:00Z"><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrChange></w:pPr><w:r><w:rPr><w:b/><w:rPrChange w:id="13" w:author="B" w:date="2026-11-07T09:00:00Z"><w:rPr/></w:rPrChange></w:rPr><w:t>Đổi định dạng</w:t></w:r></w:p>"#.to_owned(),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- review: tracked changes (ECMA-376 Part 1, §17.13.5)
    let d1 = r#"w:author="Nguyễn Văn An" w:date="2026-11-07T08:00:00Z""#;
    let d2 = r#"w:author="Trần Thị Bình" w:date="2026-11-07T09:15:00Z""#;
    // ---- fields to update: TOC (§17.16.5.68), SEQ (§17.16.5.56), REF/PAGEREF (§17.16.5.51/45)
    let fld = |instr: &str, cached: &str| field_complex(instr, cached);
    let toc_entry = |style: &str, bm: &str, text: &str, page: &str, head: &str, tail: &str| {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="{style}"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9345"/></w:tabs></w:pPr>{head}<w:hyperlink w:anchor="{bm}" w:history="1"><w:r><w:t>{text}</w:t></w:r><w:r><w:tab/></w:r>{}</w:hyperlink>{tail}</w:p>"#,
            fld(&format!("PAGEREF {bm} \\h"), page)
        )
    };
    let toc_head = r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h \z \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#;
    let toc_tail = r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#;
    let bm_heading = |ppr: &str, id: i32, bm: &str, text: &str| {
        format!(
            r#"<w:p><w:pPr>{ppr}</w:pPr><w:bookmarkStart w:id="{id}" w:name="{bm}"/><w:r><w:t>{text}</w:t></w:r><w:bookmarkEnd w:id="{id}"/></w:p>"#
        )
    };
    let h1 = r#"<w:pStyle w:val="Heading1"/>"#;
    let h2 = r#"<w:pStyle w:val="Heading1"/><w:outlineLvl w:val="1"/>"#;
    let h3 = r#"<w:outlineLvl w:val="2"/>"#;
    let pb = r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#;
    v.push(("fields-04-toc-stale.docx", "fields", "Mục lục TOC (fldChar nhiều đoạn, hyperlink + PAGEREF lồng) với mục cũ/sai trang, tiêu đề cấp 1–3 (style + outlineLvl) có bookmark _Toc — cần “Cập nhật trường”", Doc {
        body: [
            toc_entry("TOC1", "_Toc100000001", "Chương cũ", "9", toc_head, ""),
            toc_entry("TOC2", "_Toc100000002", "Mục đã xoá", "9", "", toc_tail),
            bm_heading(h1, 1, "_Toc100000001", "Chương I. Quy định chung"),
            lorem(2),
            bm_heading(h2, 2, "_Toc100000002", "1. Phạm vi điều chỉnh"),
            pb.to_owned(),
            pr(h3, "", "1.1. Đối tượng áp dụng"),
            pr(h1, "", "Chương II. Tổ chức thực hiện"),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("fields-05-captions-seq.docx", "fields", "Tiêu đề hình/bảng: SEQ Hình/Bảng (field phức và fldSimple) có số cũ, \\* ROMAN, \\r đặt lại, \\c lặp", Doc {
        body: [
            format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Hình </w:t></w:r>{}<w:r><w:t>: Sơ đồ tổ chức</w:t></w:r></w:p>"#, fld("SEQ Hình \\* ARABIC", "7")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Bảng </w:t></w:r>{}<w:r><w:t>: Dự toán</w:t></w:r></w:p>"#, field_simple("SEQ Bảng \\* ROMAN", "V")),
            lorem(1),
            format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Hình </w:t></w:r>{}<w:r><w:t>: Quy trình</w:t></w:r></w:p>"#, fld("SEQ Hình \\* ARABIC", "1")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">(nhắc lại số hình: </w:t></w:r>{}<w:r><w:t>)</w:t></w:r></w:p>"#, fld("SEQ Hình \\c", "0")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Bảng </w:t></w:r>{}<w:r><w:t>: Phụ lục, đánh lại từ 10</w:t></w:r></w:p>"#, fld("SEQ Bảng \\r 10", "2")),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("fields-06-crossref.docx", "fields", "Tham chiếu chéo REF/PAGEREF tới bookmark _Ref của tiêu đề hình và bookmark người dùng, kết quả cache cũ", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Xem </w:t></w:r>{}<w:r><w:t xml:space="preserve"> ở trang </w:t></w:r>{}<w:r><w:t xml:space="preserve">; điều khoản: </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, fld("REF _Ref200000001 \\h", "Hình 9"), fld("PAGEREF _Ref200000001 \\h", "9"), fld("REF DieuKhoan \\h", "cũ")),
            lorem(1),
            pb.to_owned(),
            format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:bookmarkStart w:id="11" w:name="_Ref200000001"/><w:r><w:t xml:space="preserve">Hình </w:t></w:r>{}<w:bookmarkEnd w:id="11"/><w:r><w:t>: Mặt bằng</w:t></w:r></w:p>"#, fld("SEQ Hình \\* ARABIC", "3")),
            r#"<w:p><w:bookmarkStart w:id="12" w:name="DieuKhoan"/><w:r><w:t>Điều 5 – Hiệu lực thi hành</w:t></w:r><w:bookmarkEnd w:id="12"/></w:p>"#.to_owned(),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- mail merge (§17.16.5.35 MERGEFIELD); data: merge-data.csv / merge-data.xlsx
    let mf = |name: &str, extra: &str| {
        let shown = name.trim_matches('"');
        field_simple(
            &format!("MERGEFIELD {name}{extra} \\* MERGEFORMAT"),
            &format!("«{shown}»"),
        )
    };
    v.push(("merge-01-letter.docx", "merge", "Thư trộn: MERGEFIELD fldSimple và field phức, tên trường có dấu cách (\"Họ tên\"), \\* Upper", Doc {
        body: [
            pr(r#"<w:jc w:val="center"/>"#, "<w:b/>", "THƯ MỜI"),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Kính gửi: </w:t></w:r>{}</w:p>"#, mf("\"Họ tên\"", "")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Đơn vị: </w:t></w:r>{}</w:p>"#, field_complex("MERGEFIELD DonVi \\* Upper", "«DonVi»")),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("merge-02-table.docx", "merge", "Trường trộn trong bảng, \\b / \\f (chữ trước/sau khi có giá trị), số tiền", Doc {
        body: [
            table(&[3000, 6355], &[
                row("", &[cell(3000, "", "Người nhận"), format!(r#"<w:tc><w:tcPr><w:tcW w:w="6355" w:type="dxa"/></w:tcPr><w:p>{}{}</w:p></w:tc>"#, mf("\"Họ tên\"", ""), mf("ChucVu", " \\b \", \""))]),
                row("", &[cell(3000, "", "Số tiền"), format!(r#"<w:tc><w:tcPr><w:tcW w:w="6355" w:type="dxa"/></w:tcPr><w:p>{}<w:r><w:t xml:space="preserve"> đồng</w:t></w:r></w:p></w:tc>"#, mf("SoTien", ""))]),
            ], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            p("Trân trọng."),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("merge-03-header-sections.docx", "merge", "Trường trộn ở đầu trang và hai section (dọc + ngang) — mỗi bản ghi lặp lại cả hai", Doc {
        body: [
            format!(r#"<w:p><w:pPr><w:sectPr>{}<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:pPr><w:r><w:t xml:space="preserve">Hồ sơ của </w:t></w:r>{}</w:p>"#, hdr_ref("header", "default", "rId20"), mf("\"Họ tên\"", "")),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Bảng kê (trang ngang) — đơn vị </w:t></w:r>{}</w:p>"#, mf("DonVi", "")),
        ].concat(),
        sect: sect(&hdr_ref("header", "default", "rId20")).replace(r#"<w:pgSz w:w="11906" w:h="16838"/>"#, r#"<w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/>"#),
        hf: vec![("rId20".into(), "header", format!("<w:p>{}</w:p>", mf("DonVi", "")))],
        ..Doc::default()
    }));

    // ---- SmartArt fallbacks: mc:Fallback picture (Part 3 §10.2.1) and the diagram
    // drawing part ([MS-ODRAWXML] dsp:drawing) referenced through dgm:relIds (§21.4.2.6).
    let alt_pic = |id: u32, rid: &str| {
        let pic = inline_pic(id, rid, 2_160_000, 1_080_000, "", "");
        let drawing = pic
            .trim_start_matches(r#"<w:r><w:rPr><w:noProof/></w:rPr>"#)
            .trim_end_matches("</w:r>");
        format!(
            r#"<w:r><mc:AlternateContent><mc:Choice Requires="w14"><w14:dummy/><w:t>[SmartArt]</w:t></mc:Choice><mc:Fallback>{drawing}</mc:Fallback></mc:AlternateContent></w:r>"#
        )
    };
    let diagram_ref = |id: u32, cx: i64, cy: i64| {
        format!(
            r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Sơ đồ {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{DGM}"><dgm:relIds xmlns:dgm="{DGM}" r:dm="rIdX1" r:lo="rIdX2" r:qs="rIdX3" r:cs="rIdX4"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
        )
    };
    let dsp_sp = |n: u32, x: i64, y: i64, cx: i64, cy: i64, rgb: &str, text: &str| {
        format!(
            r#"<dsp:sp modelId="{{00000000-0000-0000-0000-0000000000{n:02}}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{rgb}"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></dsp:spPr><dsp:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="1400"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>{text}</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#
        )
    };
    let diagram_parts = |shapes: &str| {
        let x = |root: &str, inner: &str| {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<{root} xmlns:dgm="{DGM}" xmlns:a="{A}">{inner}</{}>"#,
                root.split(' ').next().unwrap_or(root)
            )
        };
        vec![
            ("diagrams/data1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData",
                x("dgm:dataModel", &format!(r#"<dgm:ptLst><dgm:pt modelId="{{00000000-0000-0000-0000-000000000001}}" type="doc"/></dgm:ptLst><dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="rIdX5" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>"#))),
            ("diagrams/layout1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramLayout",
                x("dgm:layoutDef", "<dgm:layoutNode/>")),
            ("diagrams/quickStyle1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramQuickStyle",
                x("dgm:styleDef", r#"<dgm:styleLbl name="node0"/>"#)),
            ("diagrams/colors1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramColors+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramColors",
                x("dgm:colorsDef", "")),
            ("diagrams/drawing1.xml", "application/vnd.ms-office.drawingml.diagramDrawing+xml", "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing",
                format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dsp:drawing xmlns:dgm="{DGM}" xmlns:dsp="{DSP}" xmlns:a="{A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{shapes}</dsp:spTree></dsp:drawing>"#)),
        ]
    };
    // SmartArt saved with its data model only (no dsp:drawing): laid out by BlueOffice (CORE-SMART-002).
    let data_only_parts = |lo: &str, cat: &str, nodes: &[(&str, Option<usize>)]| {
        let x = |root: &str, attrs: &str, inner: &str| {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<{root} xmlns:dgm="{DGM}" xmlns:a="{A}"{attrs}>{inner}</{root}>"#
            )
        };
        vec![
            ("diagrams/data1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData",
                x("dgm:dataModel", "", &crate::slide_samples::smartart_data_model(lo, nodes))),
            ("diagrams/layout1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramLayout",
                x("dgm:layoutDef", &format!(r#" uniqueId="{lo}""#), &format!(r#"<dgm:catLst><dgm:cat type="{cat}" pri="1000"/></dgm:catLst><dgm:layoutNode name="diagram"/>"#))),
            ("diagrams/quickStyle1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramQuickStyle",
                x("dgm:styleDef", "", r#"<dgm:styleLbl name="node0"/>"#)),
            ("diagrams/colors1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramColors+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramColors",
                x("dgm:colorsDef", "", "")),
        ]
    };
    v.push(("smartart-04-datamodel.docx", "smartart", "SmartArt chỉ có data model (không có dsp:drawing): sơ đồ tổ chức 3 cấp do BlueOffice tự dựng bố cục", Doc {
        body: [
            p("Cơ cấu tổ chức:"),
            format!("<w:p>{}</w:p>", diagram_ref(61, 5_400_000, 2_700_000)),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        extra: data_only_parts("urn:bluestar.vn/layout/hierarchy", "hierarchy", &[
            ("Ban giám đốc", None),
            ("Phòng Kỹ thuật", Some(0)),
            ("Phòng Kinh doanh", Some(0)),
            ("Tổ Phần mềm", Some(1)),
            ("Tổ Hạ tầng", Some(1)),
            ("Tổ Bán hàng", Some(2)),
        ]),
        ..Doc::default()
    }));
    // A diagram converted to shapes (CORE-SMART-004): [MS-ODRAWXML] wordprocessingGroup `wpg:wgp`
    // (cNvGrpSpPr, grpSpPr with the child space, then shapes) of wordprocessingShape `wps:wsp`
    // (cNvPr, cNvSpPr, spPr, txbx with §17.17.1 txbxContent, bodyPr), as BlueDoc writes it.
    let wsp = |id: u32, x: i64, prst: &str, fill: &str, text: &str| {
        let txbx = if text.is_empty() {
            String::new()
        } else {
            format!(
                r#"<wps:txbx><w:txbxContent><w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:color w:val="FFFFFF"/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p></w:txbxContent></wps:txbx>"#
            )
        };
        format!(
            r#"<wps:wsp><wps:cNvPr id="{id}" name="Shape {id}"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="{x}" y="0"/><a:ext cx="1500000" cy="900000"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>{fill}<a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></wps:spPr>{txbx}<wps:bodyPr anchor="ctr" wrap="square" lIns="45720" rIns="45720" tIns="45720" bIns="45720"><a:normAutofit/></wps:bodyPr></wps:wsp>"#
        )
    };
    let solid = |rgb: &str| format!(r#"<a:solidFill><a:srgbClr val="{rgb}"/></a:solidFill>"#);
    let group = format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5400000" cy="900000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="71" name="Nhóm 71" descr="Tiếp nhận, Thẩm định, Trả kết quả"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="5400000" cy="900000"/><a:chOff x="0" y="0"/><a:chExt cx="6900000" cy="900000"/></a:xfrm></wpg:grpSpPr>{}{}{}{}</wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        wsp(72, 0, "homePlate", &solid("1F6FB2"), "Tiếp nhận"),
        wsp(73, 1_800_000, "chevron", &solid("2E8B57"), "Thẩm định"),
        wsp(74, 3_600_000, "chevron", &solid("C2571A"), "Trả kết quả"),
        // A shape filled with a picture (§20.1.8.14 blipFill), as a picture node becomes.
        wsp(
            75,
            5_400_000,
            "ellipse",
            r#"<a:blipFill><a:blip r:embed="rId30"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>"#,
            ""
        ),
    );
    v.push(("smartart-05-converted.docx", "smartart", "Sơ đồ đã chuyển thành hình: wpg:wgp gồm các wps:wsp hình dựng sẵn có chữ (quy trình 3 bước) và một hình tô ảnh, không gian con khác kích thước khung", Doc {
        body: [
            p("Quy trình (đã chuyển thành hình):"),
            format!("<w:p>{group}</w:p>"),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        images: vec![("rId30".into(), png(8, 8, [46, 117, 182]))],
        ..Doc::default()
    }));
    v.push(("smartart-01-fallback-picture.docx", "smartart", "mc:AlternateContent có mc:Fallback là ảnh (hiển thị ảnh thay cho Choice), trong đoạn và trong ô bảng", Doc {
        body: [
            format!("<w:p>{}</w:p>", alt_pic(31, "rId30")),
            table(&[9355], &[row("", &[format!(r#"<w:tc><w:tcPr><w:tcW w:w="9355" w:type="dxa"/></w:tcPr><w:p>{}</w:p></w:tc>"#, alt_pic(32, "rId30"))])], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        images: vec![("rId30".into(), png(8, 4, [46, 117, 182]))],
        ..Doc::default()
    }));
    v.push(("smartart-02-process.docx", "smartart", "SmartArt quy trình 3 bước: dgm:relIds + dsp:drawing (hình vẽ sẵn) — hiển thị bằng drawing fallback", Doc {
        body: [
            p("Quy trình xử lý hồ sơ:"),
            format!("<w:p>{}</w:p>", diagram_ref(41, 5_400_000, 900_000)),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        extra: diagram_parts(&[
            dsp_sp(1, 0, 0, 1_700_000, 900_000, "2E75B6", "Tiếp nhận"),
            dsp_sp(2, 1_850_000, 0, 1_700_000, 900_000, "548235", "Thẩm định"),
            dsp_sp(3, 3_700_000, 0, 1_700_000, 900_000, "C55A11", "Trả kết quả"),
        ].concat()),
        ..Doc::default()
    }));
    v.push((
        "smartart-03-grid.docx",
        "smartart",
        "SmartArt dạng lưới 2×2 có chữ tiếng Việt, đặt giữa trang (dsp:drawing)",
        Doc {
            body: [
                format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#,
                    diagram_ref(51, 3_600_000, 2_000_000)
                ),
                lorem(1),
            ]
            .concat(),
            sect: s.clone(),
            extra: diagram_parts(
                &[
                    dsp_sp(1, 0, 0, 1_750_000, 950_000, "7030A0", "Mục tiêu"),
                    dsp_sp(2, 1_850_000, 0, 1_750_000, 950_000, "2E75B6", "Nhiệm vụ"),
                    dsp_sp(3, 0, 1_050_000, 1_750_000, 950_000, "548235", "Giải pháp"),
                    dsp_sp(
                        4,
                        1_850_000,
                        1_050_000,
                        1_750_000,
                        950_000,
                        "C55A11",
                        "Kết quả",
                    ),
                ]
                .concat(),
            ),
            ..Doc::default()
        },
    ));

    // ---- Office Math (ECMA-376 Part 1, §22.1): inline and display equations.
    let mr = |t: &str| format!(r#"<m:r><m:t>{t}</m:t></m:r>"#);
    let frac = |n: &str, d: &str| format!("<m:f><m:num>{n}</m:num><m:den>{d}</m:den></m:f>");
    let sup = |e: &str, x: &str| format!("<m:sSup><m:e>{e}</m:e><m:sup>{x}</m:sup></m:sSup>");
    let sub = |e: &str, x: &str| format!("<m:sSub><m:e>{e}</m:e><m:sub>{x}</m:sub></m:sSub>");
    let math = |inner: &str| format!("<m:oMath>{inner}</m:oMath>");
    let display = |inner: &str| {
        format!(
            r#"<w:p><m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>{inner}</m:oMath></m:oMathPara></w:p>"#
        )
    };
    let quadratic = [
        sub(&mr("x"), &mr("1,2")),
        mr("="),
        frac(
            &[
                mr("−b±"),
                format!(
                    r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>{}{}</m:e></m:rad>"#,
                    sup(&mr("b"), &mr("2")),
                    mr("−4ac")
                ),
            ]
            .concat(),
            &mr("2a"),
        ),
    ]
    .concat();
    v.push(("math-01-inline.docx", "math", "Công thức trong dòng: phân số, mũ, chỉ số dưới, căn bậc hai (nghiệm phương trình bậc hai)", Doc {
        body: [
            format!(
                r#"<w:p><w:r><w:t xml:space="preserve">Nghiệm của phương trình </w:t></w:r>{}<w:r><w:t xml:space="preserve"> là </w:t></w:r>{}<w:r><w:t xml:space="preserve">, với điều kiện </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#,
                math(&[sup(&mr("ax"), &mr("2")), mr("+bx+c=0")].concat()),
                math(&quadratic),
                math(&[mr("Δ="), sup(&mr("b"), &mr("2")), mr("−4ac≥0")].concat()),
            ),
            lorem(1),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    let nary = |chr: &str, loc: &str, lo: &str, hi: &str, e: &str| {
        format!(
            r#"<m:nary><m:naryPr><m:chr m:val="{chr}"/><m:limLoc m:val="{loc}"/></m:naryPr><m:sub>{lo}</m:sub><m:sup>{hi}</m:sup><m:e>{e}</m:e></m:nary>"#
        )
    };
    let matrix = format!(
        r#"<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e><m:m><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr></m:m></m:e></m:d>"#,
        mr("1"),
        mr("0"),
        mr("0"),
        mr("1")
    );
    v.push(("math-02-display.docx", "math", "Công thức đứng riêng dòng: tổng Σ có cận trên/dưới, tích phân, ma trận trong ngoặc vuông, hàm sin", Doc {
        body: [
            p("Tổng và tích phân:"),
            display(&[nary("∑", "undOvr", &mr("k=1"), &mr("n"), &mr("k")), mr("="), frac(&mr("n(n+1)"), &mr("2"))].concat()),
            display(&[nary("∫", "subSup", &mr("0"), &mr("π"), &[format!(r#"<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>"#, r#"<m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>sin</m:t></m:r>"#, mr("x")), mr("dx")].concat()), mr("=2")].concat()),
            p("Ma trận đơn vị:"),
            display(&[sub(&mr("I"), &mr("2")), mr("="), matrix].concat()),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("math-03-delimiters.docx", "math", "Ngoặc co giãn theo nội dung, gạch trên, dấu mũ, giới hạn dưới (lim), hệ phương trình (eqArr)", Doc {
        body: [
            display(&[
                format!(r#"<m:func><m:fName><m:limLow><m:e><m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>lim</m:t></m:r></m:e><m:lim>{}</m:lim></m:limLow></m:fName><m:e>{}</m:e></m:func>"#, mr("x→0"), frac(&mr("sin x"), &mr("x"))),
                mr("=1"),
            ].concat()),
            display(&format!(
                r#"<m:d><m:e>{}</m:e></m:d>{}<m:bar><m:barPr><m:pos m:val="top"/></m:barPr><m:e>{}</m:e></m:bar><m:acc><m:accPr><m:chr m:val="⃗"/></m:accPr><m:e>{}</m:e></m:acc>"#,
                frac(&mr("a+b"), &mr("c")), mr("+"), mr("AB"), mr("v")
            )),
            display(&format!(
                r#"<m:d><m:dPr><m:begChr m:val="{{"/><m:endChr m:val=""/></m:dPr><m:e><m:eqArr><m:e>{}</m:e><m:e>{}</m:e></m:eqArr></m:e></m:d>"#,
                mr("x+y=3"), mr("x−y=1")
            )),
            lorem(1),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // Equations typed in the UnicodeMath linear format (bo-math, CORE-MATH-005), one display line each.
    let linear = |src: &str| {
        let x = bo_math::unicode_math::to_omml(src);
        display(
            x.trim_start_matches("<m:oMath>")
                .trim_end_matches("</m:oMath>"),
        )
    };
    v.push(("math-04-linear.docx", "math", "Phương trình nhập bằng cú pháp tuyến tính UnicodeMath: phân số, căn bậc n, tổng, tích phân, giới hạn, ma trận, hệ phương trình, dấu nhấn", Doc {
        body: [
            p("Nhập tuyến tính (UnicodeMath):"),
            linear("x=(-b±√(b^2-4ac))/2a"),
            linear("√(3&x+1)+∛8=e^(-x)"),
            linear("∑_(k=1)^n k^2=(n(n+1)(2n+1))/6"),
            linear("∫_0^∞▒e^(-x^2) dx=√π/2"),
            linear("lim_(x→0) sin x/x=1"),
            linear("A=[■(a&b@c&d)]"),
            linear("{█(x+y=3@x-y=1)"),
            linear("v\u{20D7}+x\u{302}+y\u{305}=\"hằng số\""),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // The same kinds of equations typed in LaTeX (bo-math, CORE-MATH-006).
    let tex = |src: &str| {
        let x = bo_math::latex::to_omml(src);
        display(
            x.trim_start_matches("<m:oMath>")
                .trim_end_matches("</m:oMath>"),
        )
    };
    v.push(("math-05-latex.docx", "math", "Phương trình nhập bằng LaTeX: \\frac, \\sqrt[n], \\sum/\\int có cận, \\lim, \\binom, cases, bmatrix, \\left…\\right, \\text", Doc {
        body: [
            p("Nhập LaTeX:"),
            tex(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}"),
            tex(r"\sqrt[3]{x+1}+\binom{n}{k}"),
            tex(r"\sum_{k=1}^{n} k^2=\frac{n(n+1)(2n+1)}{6}"),
            tex(r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}"),
            tex(r"\lim_{x\to 0}\frac{\sin x}{x}=1"),
            tex(r"|x|=\begin{cases} x & x\ge 0 \\ -x & x<0 \end{cases}"),
            tex(r"A=\begin{bmatrix} a & b \\ c & d \end{bmatrix},\ \left\langle u,v \right\rangle"),
            tex(r"\hat{x}+\vec{v}+\overline{y}=\text{hằng số}"),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // Equations with markup the in-place editor still does not keep (CORE-MATH-010, ADR-0015
    // addenda A and C): each starts its paragraph, so clicking it is refused and its markup kept.
    // ECMA-376 Part 1, §22.1.2 argPr (argument size), a run holding a break (w:br) between its
    // texts, a hidden radical degree with content (degHide), two oMath in one oMathPara.
    let lead = |eq: &str, label: &str| {
        format!(
            r#"<w:p>{}<w:r><w:t xml:space="preserve"> {label}</w:t></w:r></w:p>"#,
            math(eq)
        )
    };
    v.push(("math-06-unsupported.docx", "math", "Phương trình có phần tử mô hình sửa tại chỗ chưa giữ được (cỡ đối số, ngắt dòng trong run, bậc căn ẩn có nội dung, hai vùng toán trong một đoạn toán): bấm vào chỉ chọn, giữ nguyên OMML", Doc {
        body: [
            lead(&format!(r#"<m:f><m:num><m:argPr><m:argSz m:val="-1"/></m:argPr>{}</m:num><m:den>{}</m:den></m:f>"#, mr("a+b+c"), mr("2")), "cỡ đối số"),
            lead("<m:r><m:t>x=1</m:t><w:br/><m:t>y=2</m:t></m:r>", "ngắt dòng trong run"),
            lead(&format!(r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>"#, mr("3"), mr("x")), "bậc căn ẩn"),
            format!(r#"<w:p><m:oMathPara><m:oMath>{}</m:oMath><m:oMath>{}</m:oMath></m:oMathPara></w:p>"#, mr("a=1"), mr("b=2")),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    // Equations in the style other office applications write (CORE-MATH-010, ADR-0015 addendum C):
    // the math font on every run (w:rPr, ECMA-376 Part 1, §17.3.2.26 rFonts), control properties
    // (m:ctrlPr) in every structure, non-default structure properties (§22.1.2 fPr type, dPr
    // begChr / sepChr / endChr / grow, naryPr limLoc / grow / supHide, mPr mcs, accPr chr, barPr
    // pos), the run properties scr / sty, display properties (oMathParaPr jc) and groupChr, box,
    // borderBox, phant, sPre. Each starts its paragraph; all are edited in place.
    let font = r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:i/></w:rPr>"#;
    let upright = r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>"#;
    let fr = |t: &str| format!(r#"<m:r>{font}<m:t>{t}</m:t></m:r>"#);
    let ctrl = format!("<m:ctrlPr>{font}</m:ctrlPr>");
    v.push(("math-07-foreign-style.docx", "math", "Phương trình kiểu ứng dụng khác ghi: phông toán trên mọi run, m:ctrlPr, thuộc tính cấu trúc khác mặc định, ngoặc nhóm, hộp, khung, ẩn, chỉ số trước; sửa tại chỗ giữ nguyên thuộc tính", Doc {
        body: [
            lead(&[fr("y="), format!(r#"<m:f><m:fPr>{ctrl}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>"#, fr("a+b"), fr("c")), fr("+"), format!(r#"<m:sSup><m:sSupPr>{ctrl}</m:sSupPr><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>"#, fr("x"), fr("2"))].concat(), "phân số, chỉ số"),
            lead(&[format!(r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="subSup"/><m:grow m:val="1"/><m:supHide m:val="1"/>{ctrl}</m:naryPr><m:sub>{}</m:sub><m:sup/><m:e>{}</m:e></m:nary>"#, fr("k∈S"), fr("k")), fr("="), format!(r#"<m:f><m:fPr><m:type m:val="lin"/>{ctrl}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>"#, fr("n"), fr("2"))].concat(), "tổng, phân số tuyến tính"),
            lead(&[format!(r#"<m:d><m:dPr><m:begChr m:val="⟨"/><m:sepChr m:val=","/><m:endChr m:val="⟩"/><m:grow m:val="1"/>{ctrl}</m:dPr><m:e>{}</m:e><m:e>{}</m:e></m:d>"#, fr("u"), fr("v")), fr("+"), format!(r#"<m:func><m:funcPr>{ctrl}</m:funcPr><m:fName><m:r><m:rPr><m:sty m:val="p"/></m:rPr>{upright}<m:t>sin</m:t></m:r></m:fName><m:e>{}</m:e></m:func>"#, fr("θ")), fr("+"), r#"<m:r><m:rPr><m:scr m:val="script"/><m:sty m:val="i"/></m:rPr><w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:color w:val="C00000"/></w:rPr><m:t>L</m:t></m:r>"#.to_owned()].concat(), "ngoặc, hàm, chữ viết tay"),
            lead(&[format!(r#"<m:m><m:mPr><m:mcs><m:mc><m:mcPr><m:count m:val="2"/><m:mcJc m:val="center"/></m:mcPr></m:mc></m:mcs>{ctrl}</m:mPr><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr></m:m>"#, fr("1"), fr("0"), fr("0"), fr("1")), format!(r#"<m:acc><m:accPr><m:chr m:val="⃗"/>{ctrl}</m:accPr><m:e>{}</m:e></m:acc>"#, fr("v")), format!(r#"<m:bar><m:barPr><m:pos m:val="bot"/>{ctrl}</m:barPr><m:e>{}</m:e></m:bar>"#, fr("w"))].concat(), "ma trận, dấu, gạch dưới"),
            lead(&[format!(r#"<m:groupChr><m:groupChrPr><m:chr m:val="⏟"/>{ctrl}</m:groupChrPr><m:e>{}</m:e></m:groupChr>"#, fr("a+b+c")), format!(r#"<m:box><m:boxPr><m:opEmu m:val="1"/>{ctrl}</m:boxPr><m:e>{}</m:e></m:box>"#, fr("=="))].concat(), "ngoặc nhóm, hộp"),
            lead(&[format!(r#"<m:borderBox><m:borderBoxPr>{ctrl}</m:borderBoxPr><m:e>{}</m:e></m:borderBox>"#, fr("E=mc²")), format!(r#"<m:phant><m:phantPr><m:show m:val="0"/>{ctrl}</m:phantPr><m:e>{}</m:e></m:phant>"#, fr("y")), format!(r#"<m:sPre><m:sPrePr>{ctrl}</m:sPrePr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:sPre>"#, fr("6"), fr("14"), fr("C"))].concat(), "khung, ẩn, chỉ số trước"),
            format!(r#"<w:p><m:oMathPara><m:oMathParaPr><m:jc m:val="left"/></m:oMathParaPr><m:oMath>{}<m:rad><m:radPr><m:degHide m:val="1"/>{ctrl}</m:radPr><m:deg/><m:e>{}</m:e></m:rad></m:oMath></m:oMathPara></w:p>"#, fr("e="), fr("2")),
        ]
        .concat(),
        sect: s.clone(),
        ..Doc::default()
    }));

    // ---- comments (§17.13.4) with replies ([MS-DOCX] commentsExtended / commentsIds)
    let comments_part = |inner: &str| {
        (
            "comments.xml",
            CT_COMMENTS,
            REL_COMMENTS,
            wrap("comments", inner),
        )
    };
    let comment = |id: i32, author: &str, date: &str, pid: &str, text: &str| {
        format!(
            r#"<w:comment w:id="{id}" w:author="{author}" w:date="{date}" w:initials="{}"><w:p w14:paraId="{pid}" w14:textId="77777777"><w:pPr><w:pStyle w:val="CommentText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:annotationRef/></w:r><w:r><w:t>{text}</w:t></w:r></w:p></w:comment>"#,
            initials(author)
        )
    };
    let cref = |id: i32| {
        format!(
            r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="{id}"/></w:r>"#
        )
    };
    v.push(("comments-01-basic.docx", "comments", "Chú thích một đoạn chữ, chú thích trải qua hai đoạn, chú thích tại một điểm (chỉ commentReference)", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Căn cứ </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:t>Luật Tổ chức Chính phủ</w:t></w:r><w:commentRangeEnd w:id="0"/>{}<w:r><w:t>;</w:t></w:r></w:p>"#, cref(0)),
            format!(r#"<w:p><w:commentRangeStart w:id="1"/><w:r><w:t>Đoạn thứ nhất của vùng chú thích.</w:t></w:r></w:p><w:p><w:r><w:t>Đoạn thứ hai</w:t></w:r><w:commentRangeEnd w:id="1"/>{}</w:p>"#, cref(1)),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Chú thích tại điểm</w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, cref(2)),
        ].concat(),
        sect: s.clone(),
        extra: vec![comments_part(&[
            comment(0, "Nguyễn Văn An", "2026-11-07T08:00:00Z", "0A000001", "Ghi rõ năm ban hành."),
            comment(1, "Trần Thị Bình", "2026-11-07T08:30:00Z", "0A000002", "Hai đoạn này cần gộp."),
            comment(2, "Nguyễn Văn An", "2026-11-07T09:00:00Z", "0A000003", "Thêm ví dụ."),
        ].concat())],
        ..Doc::default()
    }));
    v.push(("comments-02-replies.docx", "comments", "Luồng trả lời (w15:commentEx paraIdParent), luồng đã xử lý (done), chú thích nhiều đoạn", Doc {
        body: [
            format!(r#"<w:p><w:commentRangeStart w:id="10"/><w:commentRangeStart w:id="11"/><w:r><w:t>Kinh phí thực hiện</w:t></w:r><w:commentRangeEnd w:id="10"/>{}<w:commentRangeEnd w:id="11"/>{}<w:r><w:t xml:space="preserve"> do ngân sách bảo đảm.</w:t></w:r></w:p>"#, cref(10), cref(11)),
            format!(r#"<w:p><w:commentRangeStart w:id="12"/><w:r><w:t>Thời hạn hoàn thành</w:t></w:r><w:commentRangeEnd w:id="12"/>{}</w:p>"#, cref(12)),
        ].concat(),
        sect: s.clone(),
        extra: vec![
            comments_part(&[
                comment(10, "Nguyễn Văn An", "2026-11-07T08:00:00Z", "0B000001", "Bao nhiêu?"),
                comment(11, "Trần Thị Bình", "2026-11-07T08:10:00Z", "0B000002", "150 triệu đồng."),
                r#"<w:comment w:id="12" w:author="Lê Văn Cường" w:date="2026-11-07T09:00:00Z" w:initials="LVC"><w:p w14:paraId="0B000003" w14:textId="77777777"><w:r><w:annotationRef/></w:r><w:r><w:t>Dòng một.</w:t></w:r></w:p><w:p w14:paraId="0B000004" w14:textId="77777777"><w:r><w:t>Dòng hai (đã xử lý).</w:t></w:r></w:p></w:comment>"#.to_owned(),
            ].concat()),
            ("commentsExtended.xml", CT_COMMENTS_EXT, REL_COMMENTS_EXT, format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w15:commentsEx {NS}><w15:commentEx w15:paraId="0B000001" w15:done="0"/><w15:commentEx w15:paraId="0B000002" w15:paraIdParent="0B000001" w15:done="0"/><w15:commentEx w15:paraId="0B000004" w15:done="1"/></w15:commentsEx>"#)),
        ],
        ..Doc::default()
    }));
    v.push(("comments-03-ids-tracked.docx", "comments", "commentsIds (w16cid durableId), chú thích trong bảng, chú thích trên vùng có track changes", Doc {
        body: [
            format!(r#"<w:p><w:commentRangeStart w:id="20"/><w:ins w:id="90" {d1}><w:r><w:t>Nội dung được chèn</w:t></w:r></w:ins><w:commentRangeEnd w:id="20"/>{}</w:p>"#, cref(20)),
            table(&[4677, 4678], &[row("", &[
                format!(r#"<w:tc><w:tcPr><w:tcW w:w="4677" w:type="dxa"/></w:tcPr><w:p><w:commentRangeStart w:id="21"/><w:r><w:t>Ô có chú thích</w:t></w:r><w:commentRangeEnd w:id="21"/>{}</w:p></w:tc>"#, cref(21)),
                cell(4678, "", "Ô thường"),
            ])], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        extra: vec![
            comments_part(&[
                comment(20, "Nguyễn Văn An", "2026-11-07T08:00:00Z", "0C000001", "Kiểm tra lại số liệu."),
                comment(21, "Trần Thị Bình", "2026-11-07T08:05:00Z", "0C000002", "Ô này cần cập nhật."),
            ].concat()),
            ("commentsExtended.xml", CT_COMMENTS_EXT, REL_COMMENTS_EXT, format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w15:commentsEx {NS}><w15:commentEx w15:paraId="0C000001" w15:done="0"/><w15:commentEx w15:paraId="0C000002" w15:done="0"/></w15:commentsEx>"#)),
            ("commentsIds.xml", CT_COMMENTS_IDS, REL_COMMENTS_IDS, r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w16cid:commentsIds xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:w16cid="http://schemas.microsoft.com/office/word/2016/wordml/cid" mc:Ignorable="w16cid"><w16cid:commentId w16cid:paraId="0C000001" w16cid:durableId="1A2B3C01"/><w16cid:commentId w16cid:paraId="0C000002" w16cid:durableId="1A2B3C02"/></w16cid:commentsIds>"#.to_owned()),
        ],
        ..Doc::default()
    }));

    v.push(("review-01-insert-delete.docx", "review", "Track changes: w:ins/w:del hai tác giả, chèn rồi xoá (ins ⊃ del), ins trong hyperlink, moveFrom/moveTo kèm range", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Điều 1. </w:t></w:r><w:ins w:id="101" {d1}><w:r><w:t xml:space="preserve">Phê duyệt </w:t></w:r></w:ins><w:del w:id="102" {d2}><w:r><w:delText xml:space="preserve">Ban hành </w:delText></w:r></w:del><w:r><w:t>kế hoạch năm 2027.</w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Kinh phí: </w:t></w:r><w:ins w:id="103" {d1}><w:del w:id="104" {d2}><w:r><w:rPr><w:b/></w:rPr><w:delText>120 triệu đồng</w:delText></w:r></w:del></w:ins><w:ins w:id="105" {d2}><w:r><w:rPr><w:b/></w:rPr><w:t>150 triệu đồng</w:t></w:r></w:ins><w:r><w:t xml:space="preserve">, xem </w:t></w:r><w:hyperlink w:anchor="PhuLuc"><w:ins w:id="106" {d1}><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>Phụ lục</w:t></w:r></w:ins></w:hyperlink><w:r><w:t>.</w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:moveFromRangeStart w:id="107" {d1} w:name="move1"/><w:moveFrom w:id="108" {d1}><w:r><w:t>Câu được di chuyển.</w:t></w:r></w:moveFrom><w:moveFromRangeEnd w:id="107"/><w:r><w:t xml:space="preserve"> Câu giữ nguyên.</w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:bookmarkStart w:id="1" w:name="PhuLuc"/><w:r><w:t xml:space="preserve">Phụ lục. </w:t></w:r><w:bookmarkEnd w:id="1"/><w:moveToRangeStart w:id="109" {d1} w:name="move1"/><w:moveTo w:id="110" {d1}><w:r><w:t>Câu được di chuyển.</w:t></w:r></w:moveTo><w:moveToRangeEnd w:id="109"/></w:p>"#),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("review-02-format-changes.docx", "review", "Track changes định dạng: rPrChange (đậm, màu, cỡ, phông), pPrChange (căn lề, thụt lề, giãn dòng, style)", Doc {
        body: [
            format!(r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="C00000"/><w:rPrChange w:id="201" {d1}><w:rPr/></w:rPrChange></w:rPr><w:t xml:space="preserve">Đậm và đỏ, </w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="32"/><w:rPrChange w:id="202" {d2}><w:rPr><w:i/><w:sz w:val="26"/></w:rPr></w:rPrChange></w:rPr><w:t>Arial 16pt (trước: nghiêng 13pt)</w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:pPr><w:spacing w:line="360" w:lineRule="auto"/><w:ind w:left="720" w:firstLine="567"/><w:jc w:val="both"/><w:pPrChange w:id="203" {d1}><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>Đoạn đổi sang căn đều, thụt lề và giãn dòng 1,5.</w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:pPrChange w:id="204" {d2}><w:pPr/></w:pPrChange></w:pPr><w:r><w:t>Đoạn đổi sang Heading 1</w:t></w:r></w:p>"#),
            lorem(1),
        ].concat(),
        sect: s.clone(),
        ..Doc::default()
    }));
    v.push(("review-04-tables-pictures-page-setup.docx", "review", "Track changes cấu trúc: hàng chèn/xoá (trPr ins/del), tcPrChange, tblPrChange, ảnh chèn/xoá trong w:ins/w:del, sectPrChange (đổi khổ giấy)", Doc {
        body: [
            table(&[4677, 4678], &[
                row("", &[cell(4677, "", "Hàng gốc"), cell(4678, "", "Không đổi")]),
                row(&format!(r#"<w:ins w:id="401" {d1}/>"#), &[cell(4677, "", "Hàng mới"), cell(4678, "", "chèn")]),
                row(&format!(r#"<w:del w:id="402" {d2}/>"#), &[cell(4677, "", "Hàng bị xoá"), cell(4678, "", "xoá")]),
                row("", &[
                    cell(4677, &format!(r#"<w:shd w:val="clear" w:color="auto" w:fill="FFEE00"/><w:tcPrChange w:id="403" {d1}><w:tcPr><w:tcW w:w="4677" w:type="dxa"/></w:tcPr></w:tcPrChange>"#), "Ô tô nền"),
                    cell(4678, "", "Ô thường"),
                ]),
            ], r#"<w:tblW w:w="4000" w:type="pct"/><w:jc w:val="center"/>"#).replace("</w:tblPr><w:tblGrid>", &format!(r#"<w:tblPrChange w:id="404" {d2}><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr></w:tblPrChange></w:tblPr><w:tblGrid>"#)),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Ảnh chèn: </w:t></w:r><w:ins w:id="405" {d1}>{}</w:ins><w:r><w:t xml:space="preserve"> ảnh xoá: </w:t></w:r><w:del w:id="406" {d2}>{}</w:del></w:p>"#, inline_pic(1, "rId20", 457_200, 457_200, "", ""), inline_pic(2, "rId20", 457_200, 457_200, "", "")),
            lorem(1),
        ].concat(),
        sect: format!(r#"<w:sectPr><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/><w:sectPrChange w:id="407" {d1}><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:sectPrChange></w:sectPr>"#),
        images: vec![("rId20".into(), png(4, 4, [200, 60, 30]))],
        ..Doc::default()
    }));
    v.push(("review-03-paragraph-marks.docx", "review", "Track changes dấu đoạn: ins/del trong pPr/rPr (kèm định dạng dấu đoạn), field trong vùng chèn, bảng có ô chứa thay đổi", Doc {
        body: [
            format!(r#"<w:p><w:pPr><w:rPr><w:ins w:id="301" {d1}/></w:rPr></w:pPr><w:ins w:id="302" {d1}><w:r><w:t>Đoạn mới được tách ra.</w:t></w:r></w:ins></w:p>"#),
            format!(r#"<w:p><w:pPr><w:rPr><w:del w:id="303" {d2}/><w:b/></w:rPr></w:pPr><w:r><w:t xml:space="preserve">Dấu đoạn này bị xoá, nối với đoạn sau </w:t></w:r></w:p>"#),
            format!(r#"<w:p><w:r><w:t xml:space="preserve">(đoạn sau). Trang </w:t></w:r><w:ins w:id="304" {d1}><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:ins></w:p>"#),
            table(&[4677, 4678], &[row("", &[
                format!(r#"<w:tc><w:tcPr><w:tcW w:w="4677" w:type="dxa"/></w:tcPr><w:p><w:del w:id="305" {d2}><w:r><w:delText>Ô cũ</w:delText></w:r></w:del><w:ins w:id="306" {d2}><w:r><w:t>Ô mới</w:t></w:r></w:ins></w:p></w:tc>"#),
                cell(4678, "", "Không đổi"),
            ])], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            lorem(1),
        ].concat(),
        sect: s,
        ..Doc::default()
    }));

    // ---- track changes: table structure (§17.13.5.3 cellDel, §17.13.5.4 cellIns,
    // §17.13.5.6 cellMerge, §17.13.5.33 tblGridChange, §17.13.5.36 tcPrChange,
    // §17.13.5.37 trPrChange)
    let grid_change = |id: i32, cols: &[i32]| {
        let g: String = cols
            .iter()
            .map(|w| format!(r#"<w:gridCol w:w="{w}"/>"#))
            .collect();
        format!(
            r#"<w:tblGridChange w:id="{id}"><w:tblGrid>{g}</w:tblGrid></w:tblGridChange></w:tblGrid>"#
        )
    };
    let tc_change = |id: i32, d: &str, old: &str| {
        format!(r#"<w:tcPrChange w:id="{id}" {d}><w:tcPr>{old}</w:tcPr></w:tcPrChange>"#)
    };
    let header_row = |id: i32, d: &str| {
        format!(r#"<w:tblHeader/><w:trPrChange w:id="{id}" {d}><w:trPr/></w:trPrChange>"#)
    };
    v.push(("review-05-columns-cells.docx", "review", "Track changes bảng: cột chèn (cellIns từng hàng + tblGridChange), cột xoá (cellDel), ô chèn đẩy phải / ô xoá đẩy trái, hàng tiêu đề lặp lại mới (trPrChange)", Doc {
        body: [
            p("Bảng có cột giữa được chèn và cột cuối bị xoá:"),
            table(&[3000, 3000, 3000], &(0..4).map(|r| {
                let hr = if r == 0 { header_row(501, d1) } else { String::new() };
                row(&hr, &[
                    cell(3000, "", &format!("Cột A{r}")),
                    cell(3000, &format!(r#"<w:cellIns w:id="{}" {d1}/>"#, 510 + r), &format!("Cột mới {r}")),
                    cell(3000, &format!(r#"<w:cellDel w:id="{}" {d2}/>"#, 520 + r), &format!("Cột xoá {r}")),
                ])
            }).collect::<Vec<_>>(), r#"<w:tblW w:w="0" w:type="auto"/>"#)
                .replace("</w:tblGrid>", &grid_change(530, &[4500, 4500])),
            p("Ô chèn đẩy sang phải và ô xoá đẩy sang trái:"),
            table(&[4500, 4500, 4500], &[
                row("", &[cell(4500, "", "Một"), cell(4500, "", "Hai")]),
                row("", &[cell(4500, &format!(r#"<w:cellIns w:id="504" {d2}/>"#), "Ô chèn"), cell(4500, "", "Ba"), cell(4500, "", "Bốn")]),
                row("", &[cell(4500, &format!(r#"<w:cellDel w:id="505" {d1}/>"#), "Ô xoá"), cell(4500, "", "Năm")]),
            ], r#"<w:tblW w:w="0" w:type="auto"/>"#).replace("</w:tblGrid>", &grid_change(506, &[4500, 4500])),
            lorem(1),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    v.push(("review-06-merge-split.docx", "review", "Track changes gộp/tách ô: gộp ngang (tcPrChange gridSpan + cellDel ô bị gộp), gộp dọc (cellMerge vMerge=cont/rest), tách ô (cellIns + tcPrChange), hàng không ngắt trang mới (trPrChange)", Doc {
        body: [
            p("Gộp ngang hai ô của hàng đầu, gộp dọc cột đầu của hai hàng sau:"),
            table(&[3000, 3000, 3000], &[
                row(&format!(r#"<w:cantSplit/><w:trPrChange w:id="601" {d2}><w:trPr/></w:trPrChange>"#), &[
                    cell(6000, &format!(r#"<w:gridSpan w:val="2"/>{}"#, tc_change(602, d1, r#"<w:tcW w:w="3000" w:type="dxa"/>"#)), "Ô gộp"),
                    cell(3000, &format!(r#"<w:cellDel w:id="612" {d1}/>"#), "bị gộp"),
                    cell(3000, "", "Giữ"),
                ]),
                row("", &[
                    cell(3000, &format!(r#"<w:vMerge w:val="restart"/><w:cellMerge w:id="603" {d2} w:vMerge="rest"/>"#), "Gộp dọc"),
                    cell(3000, "", "B1"),
                    cell(3000, "", "C1"),
                ]),
                row("", &[
                    cell(3000, &format!(r#"<w:vMerge/><w:cellMerge w:id="613" {d2} w:vMerge="cont"/>"#), ""),
                    cell(3000, "", "B2"),
                    cell(3000, "", "C2"),
                ]),
            ], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            p("Tách một ô thành hai cột:"),
            table(&[2000, 2500, 4500], &[
                row("", &[
                    cell(2000, &tc_change(604, d1, r#"<w:tcW w:w="4500" w:type="dxa"/>"#), "Nửa trái"),
                    cell(2500, &format!(r#"<w:cellIns w:id="614" {d1}/>"#), "Nửa phải"),
                    cell(4500, "", "Khác"),
                ]),
                row("", &[cell(4500, &format!(r#"<w:gridSpan w:val="2"/>{}"#, tc_change(605, d1, r#"<w:tcW w:w="4500" w:type="dxa"/>"#)), "Hàng hai"), cell(4500, "", "x")]),
            ], r#"<w:tblW w:w="0" w:type="auto"/>"#).replace("</w:tblGrid>", &grid_change(624, &[4500, 4500])),
            lorem(1),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    v.push(("review-07-row-properties.docx", "review", "Track changes thuộc tính hàng: bỏ hàng tiêu đề, đổi chiều cao (trPrChange), tách gộp dọc (cellMerge vMergeOrig), cột chèn trong bảng lồng nhau, ô xoá", Doc {
        body: [
            table(&[4677, 4678], &[
                row(&format!(r#"<w:trHeight w:val="600"/><w:trPrChange w:id="701" {d1}><w:trPr><w:trHeight w:val="400"/><w:tblHeader/></w:trPr></w:trPrChange>"#), &[cell(4677, "", "Hết là hàng tiêu đề"), cell(4678, "", "cao hơn")]),
                row("", &[
                    cell(4677, &format!(r#"<w:cellMerge w:id="702" {d2} w:vMergeOrig="rest"/>"#), "Đã tách"),
                    format!(r#"<w:tc><w:tcPr><w:tcW w:w="4678" w:type="dxa"/></w:tcPr>{}{}</w:tc>"#,
                        table(&[2000, 2000], &[row("", &[
                            cell(2000, "", "Lồng"),
                            cell(2000, &format!(r#"<w:cellIns w:id="703" {d1}/>"#), "Cột chèn"),
                        ])], r#"<w:tblW w:w="0" w:type="auto"/>"#).replace("</w:tblGrid>", &grid_change(713, &[4000])),
                        p("")),
                ]),
                row("", &[
                    cell(4677, &format!(r#"<w:cellMerge w:id="712" {d2} w:vMergeOrig="cont"/>"#), "Từng gộp"),
                    cell(4678, &format!(r#"<w:cellDel w:id="704" {d2}/>"#), "Ô xoá"),
                ]),
            ], r#"<w:tblW w:w="0" w:type="auto"/>"#),
            lorem(1),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));

    // ---- track changes inside headers, footers and notes (§17.10, §17.11, §17.13.5)
    let hdr_rev = |text: &str, id: i32, d: &str| {
        format!(
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">{text} </w:t></w:r><w:ins w:id="{id}" {d}><w:r><w:t>(sửa đổi)</w:t></w:r></w:ins><w:del w:id="{}" {d}><w:r><w:delText xml:space="preserve"> (bản cũ)</w:delText></w:r></w:del></w:p>"#,
            id + 1
        )
    };
    let fnote_rev = |id: i32, rid: i32, d: &str, t: &str| {
        format!(
            r#"<w:footnote w:id="{id}"><w:p><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> {t} </w:t></w:r><w:ins w:id="{rid}" {d}><w:r><w:t>bổ sung</w:t></w:r></w:ins><w:del w:id="{}" {d}><w:r><w:delText xml:space="preserve"> đã bỏ</w:delText></w:r></w:del></w:p></w:footnote>"#,
            rid + 1
        )
    };
    let fref = |id: i32| {
        format!(
            r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{id}"/></w:r>"#
        )
    };
    v.push(("review-08-header-footer.docx", "review", "Track changes trong đầu/chân trang: ins/del ở đầu trang, rPrChange ở chân trang, kèm thay đổi trong chú thích cuối trang", Doc {
        body: format!(r#"<w:p><w:r><w:t>Văn bản có đầu trang được sửa</w:t></w:r>{}</w:p>{}"#, fref(1), lorem(2)),
        sect: sect(&[hdr_ref("header", "default", "rId10"), hdr_ref("footer", "default", "rId11")].concat()),
        hf: vec![
            ("rId10".into(), "header", hdr_rev("CÔNG TY GIẢ ĐỊNH", 801, d1)),
            ("rId11".into(), "footer", format!(r#"<w:p><w:r><w:rPr><w:b/><w:rPrChange w:id="803" {d2}><w:rPr/></w:rPrChange></w:rPr><w:t>Chân trang in đậm (trước: thường)</w:t></w:r></w:p>"#)),
        ],
        footnotes: Some(fnote_rev(1, 804, d2, "Ghi chú có sửa đổi:")),
        ..Doc::default()
    }));
    v.push(("review-09-notes.docx", "review", "Track changes trong chú thích: footnote có ins/del, endnote có pPrChange và dấu đoạn chèn, đầu trang trang đầu có thay đổi", Doc {
        body: format!(r#"<w:p><w:r><w:t>Điều khoản</w:t></w:r>{}<w:r><w:t xml:space="preserve"> và nguồn</w:t></w:r><w:r><w:endnoteReference w:id="1"/></w:r></w:p>{}"#, fref(1), lorem(2)),
        sect: sect(&[hdr_ref("header", "first", "rId10"), "<w:titlePg/>".to_owned()].concat()).replace("<w:titlePg/><w:pgSz", "<w:pgSz").replace("<w:docGrid", "<w:titlePg/><w:docGrid"),
        hf: vec![("rId10".into(), "header", hdr_rev("TRANG ĐẦU", 901, d2))],
        footnotes: Some(fnote_rev(1, 903, d1, "Footnote:")),
        endnotes: Some(format!(r#"<w:endnote w:id="1"><w:p><w:pPr><w:jc w:val="both"/><w:pPrChange w:id="905" {d1}><w:pPr/></w:pPrChange></w:pPr><w:r><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> Endnote căn đều (trước: trái).</w:t></w:r></w:p><w:p><w:pPr><w:rPr><w:ins w:id="906" {d2}/></w:rPr></w:pPr><w:ins w:id="907" {d2}><w:r><w:t>Đoạn mới trong endnote.</w:t></w:r></w:ins></w:p><w:p><w:r><w:t>Đoạn cuối.</w:t></w:r></w:p></w:endnote>"#)),
        ..Doc::default()
    }));
    v.push(("review-10-stories-mixed.docx", "review", "Track changes trong đầu/chân trang chẵn lẻ (evenAndOddHeaders) và nhiều chú thích, kèm thay đổi ở thân văn bản", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Thân văn bản </w:t></w:r><w:ins w:id="1001" {d1}><w:r><w:t>có chèn</w:t></w:r></w:ins>{}</w:p>"#, fref(1)),
            lorem(6),
            format!(r#"<w:p><w:r><w:t>Chú thích thứ hai</w:t></w:r>{}</w:p>"#, fref(2)),
            lorem(6),
        ].concat(),
        sect: sect(&[hdr_ref("header", "default", "rId10"), hdr_ref("header", "even", "rId11"), hdr_ref("footer", "default", "rId12")].concat()),
        hf: vec![
            ("rId10".into(), "header", hdr_rev("Trang lẻ", 1002, d1)),
            ("rId11".into(), "header", hdr_rev("Trang chẵn", 1004, d2)),
            ("rId12".into(), "footer", format!(r#"<w:p><w:pPr><w:jc w:val="right"/><w:pPrChange w:id="1006" {d2}><w:pPr/></w:pPrChange></w:pPr><w:r><w:t>Chân trang căn phải</w:t></w:r></w:p>"#)),
        ],
        footnotes: Some(fnote_rev(1, 1007, d1, "Một:") + &fnote_rev(2, 1009, d2, "Hai:")),
        settings: Some(r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:evenAndOddHeaders/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()),
        ..Doc::default()
    }));

    // ---- track changes: floating picture moves (§17.13.5.22 moveFrom, §17.13.5.25
    // moveTo, §20.4.2.3 anchor)
    let moved_pic = |id: u32, x: i64, y: i64, wrap: &str| {
        wrap_pic(
            id,
            "rId20",
            (914_400, 685_800),
            (x, y),
            [0, 0, 114_300, 114_300],
            wrap,
        )
    };
    let mv = |tag: &str, id: i32, d: &str, inner: &str| {
        format!(r#"<w:{tag} w:id="{id}" {d}>{inner}</w:{tag}>"#)
    };
    v.push(("review-11-picture-move.docx", "review", "Track changes ảnh nổi đổi vị trí trong cùng đoạn: moveFrom (vị trí cũ) + moveTo (vị trí mới), bao chữ square", Doc {
        body: [
            format!(r#"<w:p>{}{}<w:r><w:t xml:space="preserve">{} </w:t></w:r></w:p>"#,
                mv("moveFrom", 1101, d1, &moved_pic(1, 0, 0, r#"<wp:wrapSquare wrapText="bothSides"/>"#)),
                mv("moveTo", 1102, d1, &moved_pic(2, 2_743_200, 228_600, r#"<wp:wrapSquare wrapText="bothSides"/>"#)),
                VI.concat()),
            lorem(2),
        ].concat(),
        sect: sect(""),
        images: vec![("rId20".into(), png(6, 4, [30, 110, 200]))],
        ..Doc::default()
    }));
    v.push(("review-12-picture-move-paragraph.docx", "review", "Track changes ảnh nổi chuyển sang đoạn khác: moveFrom/moveTo kèm moveFromRangeStart/End, moveToRangeStart/End, bao chữ top-and-bottom", Doc {
        body: [
            format!(r#"<w:p><w:moveFromRangeStart w:id="1201" {d2} w:name="move2"/>{}<w:moveFromRangeEnd w:id="1201"/><w:r><w:t>Đoạn có ảnh lúc đầu.</w:t></w:r></w:p>"#,
                mv("moveFrom", 1202, d2, &moved_pic(1, 0, 0, "<wp:wrapTopAndBottom/>"))),
            lorem(2),
            format!(r#"<w:p><w:moveToRangeStart w:id="1203" {d2} w:name="move2"/>{}<w:moveToRangeEnd w:id="1203"/><w:r><w:t>Ảnh đã chuyển tới đoạn này.</w:t></w:r></w:p>"#,
                mv("moveTo", 1204, d2, &moved_pic(2, 1_828_800, 0, "<wp:wrapTopAndBottom/>"))),
            lorem(1),
        ].concat(),
        sect: sect(""),
        images: vec![("rId20".into(), png(6, 4, [200, 140, 20]))],
        ..Doc::default()
    }));
    v.push(("review-13-picture-insert-delete-move.docx", "review", "Track changes ảnh nổi: chèn (w:ins), xoá (w:del) và di chuyển (moveFrom/moveTo, bao chữ tight) của hai tác giả", Doc {
        body: [
            format!(r#"<w:p>{}<w:r><w:t xml:space="preserve">Ảnh nổi được chèn. {} </w:t></w:r></w:p>"#,
                mv("ins", 1301, d1, &moved_pic(1, 3_657_600, 0, r#"<wp:wrapSquare wrapText="left"/>"#)), VI[0]),
            format!(r#"<w:p>{}<w:r><w:t xml:space="preserve">Ảnh nổi bị xoá. {} </w:t></w:r></w:p>"#,
                mv("del", 1302, d2, &moved_pic(2, 0, 0, r#"<wp:wrapSquare wrapText="right"/>"#)), VI[1]),
            format!(r#"<w:p>{}{}<w:r><w:t xml:space="preserve">Ảnh nổi dời chỗ. {} </w:t></w:r></w:p>"#,
                mv("moveFrom", 1303, d1, &moved_pic(3, 0, 0, &tight("bothSides", &[(0, 0), (21_600, 0), (21_600, 21_600), (0, 21_600)]))),
                mv("moveTo", 1304, d1, &moved_pic(4, 1_828_800, 114_300, &tight("bothSides", &[(0, 0), (21_600, 0), (21_600, 21_600), (0, 21_600)]))),
                VI[2]),
        ].concat(),
        sect: sect(""),
        images: vec![("rId20".into(), png(5, 5, [60, 160, 90]))],
        ..Doc::default()
    }));

    // ---- footnotes continued on the next page (§17.11.1 continuationSeparator) and
    // document-wide footnotePr / endnotePr in settings.xml (§17.11.11, §17.11.4)
    let long_note = |id: i32, paras: usize| {
        let mut x = format!(r#"<w:footnote w:id="{id}">"#);
        for i in 0..paras {
            let mark = if i == 0 {
                r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r>"#
            } else {
                ""
            };
            x.push_str(&format!(r#"<w:p>{mark}<w:r><w:t xml:space="preserve"> Đoạn {} của chú thích dài: {}</w:t></w:r></w:p>"#, i + 1, VI[i % VI.len()]));
        }
        x.push_str("</w:footnote>");
        x
    };
    let notice = r#"<w:footnote w:type="continuationNotice" w:id="1"><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>(xem tiếp trang sau)</w:t></w:r></w:p></w:footnote>"#;
    let settings = |extra: &str| {
        format!(
            r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/>{extra}<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#
        )
    };
    v.push(("notes-05-long-footnote.docx", "notes", "Chú thích cuối trang dài chuyển tiếp sang trang sau (continuationSeparator); footnotePr cấp tài liệu trong settings.xml: numFmt upperRoman, numStart 3", Doc {
        body: format!(r#"{}<w:p><w:r><w:t>Điều khoản có chú thích dài</w:t></w:r>{}</w:p>{}"#, lorem(30), fref(2), lorem(8)),
        sect: sect(""),
        footnotes: Some(long_note(2, 24)),
        settings: Some(settings(r#"<w:footnotePr><w:numFmt w:val="upperRoman"/><w:numStart w:val="3"/><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr>"#)),
        ..Doc::default()
    }));
    v.push(("notes-06-continuation-notice.docx", "notes", "Chú thích dài có thông báo tiếp trang (footnote type continuationNotice); settings footnotePr pos beneathText, numRestart eachPage; hai chú thích trên trang", Doc {
        body: format!(r#"{}<w:p><w:r><w:t>Chú thích ngắn</w:t></w:r>{}</w:p>{}<w:p><w:r><w:t>Chú thích dài</w:t></w:r>{}</w:p>{}"#, lorem(10), fref(2), lorem(18), fref(3), lorem(10)),
        sect: sect(""),
        footnotes: Some(format!(r#"{notice}<w:footnote w:id="2"><w:p><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Chú thích ngắn.</w:t></w:r></w:p></w:footnote>{}"#, long_note(3, 22))),
        settings: Some(settings(r#"<w:footnotePr><w:pos w:val="beneathText"/><w:numRestart w:val="eachPage"/><w:footnote w:id="-1"/><w:footnote w:id="0"/><w:footnote w:id="1"/></w:footnotePr>"#)),
        ..Doc::default()
    }));
    v.push(("notes-07-document-note-settings.docx", "notes", "footnotePr/endnotePr cấp tài liệu (settings.xml) bị section ghi đè: footnote lowerLetter của tài liệu, section 2 dùng decimal; endnote decimal cuối section (sectEnd); chú thích dài nhiều đoạn", Doc {
        body: format!(
            r#"<w:p><w:r><w:t>Phần một</w:t></w:r>{}<w:r><w:t xml:space="preserve"> nguồn</w:t></w:r><w:r><w:endnoteReference w:id="1"/></w:r></w:p>{}<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết phần một</w:t></w:r></w:p>{}<w:p><w:r><w:t>Phần hai</w:t></w:r>{}</w:p>{}"#,
            fref(2), lorem(26), lorem(2), fref(3), lorem(2)
        ),
        sect: r#"<w:sectPr><w:footnotePr><w:numFmt w:val="decimal"/><w:numRestart w:val="eachSect"/></w:footnotePr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/><w:cols w:space="708"/></w:sectPr>"#.to_owned(),
        footnotes: Some(long_note(2, 20) + &long_note(3, 2)),
        endnotes: Some(r#"<w:endnote w:id="1"><w:p><w:r><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> Nguồn giả định của phần một.</w:t></w:r></w:p></w:endnote>"#.to_owned()),
        settings: Some(settings(r#"<w:footnotePr><w:numFmt w:val="lowerLetter"/></w:footnotePr><w:endnotePr><w:pos w:val="sectEnd"/><w:numFmt w:val="decimal"/></w:endnotePr>"#)),
        ..Doc::default()
    }));

    // ---- page background (§17.2.1), page borders (§17.6.10) and VML watermarks
    // (Part 4, §19.1.2.23 textpath, §19.1.2.11 imagedata)
    let page = r#"<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/>"#;
    v.push(("page-01-borders-background.docx", "page", "Màu nền trang (w:background, displayBackgroundShape); section 1 viền nét đôi có bóng đo từ chữ; section 2 viền hoạ tiết (chấm tròn trên/dưới, ziczac trái/phải) đo từ mép trang, sau chữ, trừ trang đầu", Doc {
        body: [
            lorem(30),
            format!(r#"<w:p><w:pPr><w:sectPr>{page}<w:pgBorders><w:top w:val="double" w:sz="6" w:space="24" w:color="1F3A68" w:shadow="1"/><w:left w:val="double" w:sz="6" w:space="24" w:color="1F3A68" w:shadow="1"/><w:bottom w:val="double" w:sz="6" w:space="24" w:color="1F3A68" w:shadow="1"/><w:right w:val="double" w:sz="6" w:space="24" w:color="1F3A68" w:shadow="1"/></w:pgBorders><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 1</w:t></w:r></w:p>"#),
            lorem(80),
        ].concat(),
        sect: format!(r#"<w:sectPr>{page}<w:pgBorders w:zOrder="back" w:display="notFirstPage" w:offsetFrom="page"><w:top w:val="basicBlackDots" w:sz="10" w:space="24" w:color="1F6FB2"/><w:left w:val="zigZag" w:sz="10" w:space="24" w:color="2BA3A0"/><w:bottom w:val="basicBlackDots" w:sz="10" w:space="24" w:color="1F6FB2"/><w:right w:val="zigZag" w:sz="10" w:space="24" w:color="2BA3A0"/></w:pgBorders><w:cols w:space="708"/></w:sectPr>"#),
        background: Some(r#"<w:background w:color="FFF8E7"/>"#.to_owned()),
        settings: Some(settings_bg()),
        ..Doc::default()
    }));
    let wm_text = r##"<w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:pict><v:shape id="CorpusWatermarkText" style="position:absolute;margin-left:0;margin-top:0;width:430pt;height:107pt;rotation:315;z-index:-251657216;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="#a6a6a6" stroked="f" coordsize="21600,21600" path="m0,10800l21600,10800e"><v:fill opacity=".5"/><v:path textpathok="t"/><v:textpath on="t" fitshape="t" style="font-family:&quot;Calibri&quot;;font-size:1pt" string="BẢN NHÁP"/></v:shape></w:pict></w:r><w:r><w:t>Đầu trang – tài liệu thử nghiệm</w:t></w:r></w:p>"##;
    let wm_pic = r#"<w:p><w:r><w:pict><v:shape id="CorpusWatermarkPicture" style="position:absolute;margin-left:0;margin-top:0;width:300pt;height:200pt;z-index:-251656192;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" stroked="f" filled="f" coordsize="21600,21600" path="m0,0l21600,0,21600,21600,0,21600xe"><v:imagedata r:id="rId1" o:title="" gain="0.3" blacklevel="0.35"/></v:shape></w:pict></w:r><w:r><w:t>Trang bìa</w:t></w:r></w:p>"#;
    v.push(("page-02-watermarks.docx", "page", "Hình mờ: chữ VML chéo 315°, bán trong suốt ở header mặc định; ảnh làm nhạt (gain/blacklevel) ở header trang đầu (titlePg, quan hệ ảnh riêng của header)", Doc {
        body: lorem(60),
        sect: format!(r#"<w:sectPr>{}{}{page}<w:cols w:space="708"/><w:titlePg/></w:sectPr>"#, hdr_ref("header", "default", "rId10"), hdr_ref("header", "first", "rId11")),
        hf: vec![
            ("rId10".into(), "header", wm_text.to_owned()),
            ("rId11".into(), "header", wm_pic.to_owned()),
        ],
        images: vec![("rId20".into(), png(8, 6, [70, 120, 190]))],
        hf_images: vec![("rId11".into(), "rId1".into(), 0)],
        ..Doc::default()
    }));
    // ---- drop caps (§17.3.1.11 framePr), columns (§17.6.4 cols, §17.6.3 col),
    // line numbering (§17.6.8), hyphenation (§17.15.1.10), text direction (§17.6.20)
    let cap = |kind: &str, lines: u8, space: i32, letter: &str, font: &str| {
        format!(
            r#"<w:p><w:pPr><w:keepNext/><w:framePr w:dropCap="{kind}" w:lines="{lines}" w:hSpace="{space}" w:wrap="around" w:vAnchor="text" w:hAnchor="text"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}"/><w:sz w:val="86"/></w:rPr><w:t>{letter}</w:t></w:r></w:p>"#
        )
    };
    let long = |n: usize, text: &str| p(&vec![text; n].join(" "));
    v.push(("layout-01-drop-caps-columns.docx", "layout", "Chữ hoa thả 3 dòng (drop, hSpace) và 2 dòng ở lề (margin); section 2 continuous: 2 cột không đều (1,5 + 4,25 in, w:equalWidth=\"0\") có đường kẻ giữa (w:sep) và ngắt cột (w:br type=column); section 3: 3 cột đều có đường kẻ", Doc {
        body: [
            cap("drop", 3, 113, "N", "Times New Roman"),
            long(5, "gày xửa ngày xưa, ở một vương quốc nọ có một ông vua hiền lành cai quản muôn dân."),
            cap("margin", 2, 57, "T", "Arial"),
            long(4, "rong khu vườn của nhà vua có một cây táo nở hoa trắng xoá mỗi độ xuân về."),
            format!(r#"<w:p><w:pPr><w:sectPr>{page}<w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 1</w:t></w:r></w:p>"#),
            lorem(8),
            r#"<w:p><w:r><w:t>Trước ngắt cột</w:t></w:r><w:r><w:br w:type="column"/></w:r><w:r><w:t>Sau ngắt cột</w:t></w:r></w:p>"#.to_owned(),
            lorem(30),
            format!(r#"<w:p><w:pPr><w:sectPr><w:type w:val="continuous"/>{page}<w:cols w:num="2" w:space="708" w:sep="1" w:equalWidth="0"><w:col w:w="2160" w:space="567"/><w:col w:w="6628"/></w:cols></w:sectPr></w:pPr><w:r><w:t>Hết section 2</w:t></w:r></w:p>"#),
            lorem(40),
        ].concat(),
        sect: format!(r#"<w:sectPr>{page}<w:cols w:num="3" w:space="425" w:sep="1"/></w:sectPr>"#),
        ..Doc::default()
    }));
    let en = |text: &str| {
        format!(
            r#"<w:p><w:r><w:rPr><w:lang w:val="en-US"/></w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#
        )
    };
    let en_text = "International organisations demonstrate considerable responsibility regarding environmental documentation, establishing comprehensive information requirements for independent establishments and understanding everything wonderfully.";
    v.push(("layout-02-line-numbers-hyphenation.docx", "layout", "Đánh số dòng (lnNumType countBy=5, distance, restart newPage) với một đoạn suppressLineNumbers; gạch nối tự động (autoHyphenation, consecutiveHyphenLimit 2, hyphenationZone, doNotHyphenateCaps) cho đoạn tiếng Anh trong 3 cột, một đoạn suppressAutoHyphens; tiếng Việt chỉ ngắt ở dấu cách", Doc {
        body: [
            lorem(6),
            pr("<w:suppressLineNumbers/>", "", "Đoạn này không được đánh số dòng."),
            lorem(6),
            format!(r#"<w:p><w:pPr><w:sectPr>{page}<w:lnNumType w:countBy="5" w:distance="283" w:restart="newPage"/><w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section 1</w:t></w:r></w:p>"#),
            en(en_text),
            en(en_text),
            format!(r#"<w:p><w:pPr><w:suppressAutoHyphens/></w:pPr><w:r><w:rPr><w:lang w:val="en-US"/></w:rPr><w:t xml:space="preserve">{en_text}</w:t></w:r></w:p>"#),
            en("UNDERSTANDING INTERNATIONAL ORGANISATIONS REQUIRES DOCUMENTATION."),
            lorem(8),
        ].concat(),
        sect: format!(r#"<w:sectPr><w:type w:val="continuous"/>{page}<w:cols w:num="3" w:space="425"/></w:sectPr>"#),
        settings: Some(r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:autoHyphenation/><w:consecutiveHyphenLimit w:val="2"/><w:hyphenationZone w:val="227"/><w:doNotHyphenateCaps/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()),
        ..Doc::default()
    }));
    v.push(("layout-03-vertical-sections.docx", "layout", "Hướng chữ của section (w:textDirection trên sectPr): trang 1 ngang, section 2 chữ dọc từ trên xuống (tbRl, dòng xếp từ phải sang trái), section 3 từ dưới lên (btLr, dòng từ trái sang phải)", Doc {
        body: [
            lorem(4),
            format!(r#"<w:p><w:pPr><w:sectPr>{page}<w:cols w:space="708"/></w:sectPr></w:pPr><w:r><w:t>Hết section ngang</w:t></w:r></w:p>"#),
            lorem(6),
            format!(r#"<w:p><w:pPr><w:sectPr>{page}<w:cols w:space="708"/><w:textDirection w:val="tbRl"/></w:sectPr></w:pPr><w:r><w:t>Hết section dọc trên xuống</w:t></w:r></w:p>"#),
            lorem(6),
        ].concat(),
        sect: format!(r#"<w:sectPr>{page}<w:cols w:space="708"/><w:textDirection w:val="btLr"/></w:sectPr>"#),
        ..Doc::default()
    }));
    // DOC-FMT-001: character effects, spacing/scale, OpenType typography, ruby, combine.
    let big = r#"<w:sz w:val="40"/><w:szCs w:val="40"/>"#;
    v.push(("text-04-effects.docx", "text", "Hiệu ứng ký tự: viền rỗng (w:outline), bóng (w:shadow), nổi (w:emboss), chìm (w:imprint); hiệu ứng w14: phát sáng, bóng mờ, phản chiếu, viền chữ, tô chuyển màu; viền ký tự (w:bdr) và tô nền theo mẫu (w:shd pct25)", Doc {
        body: [
            rpara(&[(&format!("<w:outline/>{big}"), "Chữ viền rỗng "), (&format!("<w:shadow/>{big}"), "Chữ có bóng")]),
            rpara(&[(&format!(r#"<w:emboss/><w:color w:val="C0C0C0"/>{big}"#), "Chữ nổi "), (&format!(r#"<w:imprint/><w:color w:val="C0C0C0"/>{big}"#), "Chữ chìm")]),
            rpara(&[(&format!(r#"<w:color w:val="1F3A68"/>{big}<w14:glow w14:rad="101600"><w14:srgbClr w14:val="FFC000"><w14:alpha w14:val="40000"/></w14:srgbClr></w14:glow>"#), "Phát sáng vàng")]),
            rpara(&[(&format!(r#"{big}<w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl"><w14:srgbClr w14:val="000000"><w14:alpha w14:val="60000"/></w14:srgbClr></w14:shadow>"#), "Bóng mờ lệch phải dưới")]),
            rpara(&[(&format!(r#"{big}<w14:reflection w14:blurRad="6350" w14:stA="55000" w14:stPos="0" w14:endA="300" w14:endPos="45500" w14:dist="0" w14:dir="5400000" w14:fadeDir="5400000" w14:sx="100000" w14:sy="-100000" w14:kx="0" w14:ky="0" w14:algn="bl"/>"#), "Phản chiếu")]),
            rpara(&[(&format!(r#"<w:color w:val="FFFFFF"/>{big}<w14:textOutline w14:w="12700" w14:cap="flat" w14:cmpd="sng" w14:algn="ctr"><w14:solidFill><w14:srgbClr w14:val="C00000"/></w14:solidFill><w14:prstDash w14:val="solid"/><w14:round/></w14:textOutline>"#), "Viền chữ đỏ 1 pt")]),
            rpara(&[(&format!(r#"<w:b/>{big}<w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="C00000"/></w14:gs><w14:gs w14:pos="100000"><w14:schemeClr w14:val="accent1"><w14:lumMod w14:val="75000"/></w14:schemeClr></w14:gs></w14:gsLst><w14:lin w14:ang="0" w14:scaled="0"/></w14:gradFill></w14:textFill>"#), "Tô chuyển màu ngang")]),
            rpara(&[("", "Viền ký tự: "), (r#"<w:bdr w:val="single" w:sz="8" w:space="0" w:color="1F3A68"/>"#, "khung đơn"), ("", " và "), (r#"<w:shd w:val="pct25" w:color="auto" w:fill="FFFF00"/>"#, "tô nền mẫu 25 %")]),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    v.push(("text-05-spacing-scale.docx", "text", "Giãn/co ký tự (w:spacing ±), nâng/hạ (w:position ±), co giãn ngang (w:w 50/150/200 %), kerning từ 12 pt (w:kern)", Doc {
        body: [
            rpara(&[(r#"<w:spacing w:val="60"/>"#, "Giãn 3 pt "), (r#"<w:spacing w:val="-20"/>"#, "co 1 pt")]),
            rpara(&[("", "Chuẩn "), (r#"<w:position w:val="8"/>"#, "nâng 4 pt"), ("", " và "), (r#"<w:position w:val="-8"/>"#, "hạ 4 pt")]),
            rpara(&[(r#"<w:w w:val="50"/>"#, "Co 50 % "), (r#"<w:w w:val="150"/>"#, "giãn 150 % "), (r#"<w:w w:val="200"/>"#, "200 %")]),
            rpara(&[(&format!(r#"<w:rFonts w:ascii="Liberation Sans" w:hAnsi="Liberation Sans"/><w:kern w:val="24"/>{big}"#), "AVATAR Wave To"), (&format!(r#"<w:rFonts w:ascii="Liberation Sans" w:hAnsi="Liberation Sans"/>{big}"#), " AVATAR Wave To")]),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    v.push(("text-06-opentype.docx", "text", "Kiểu chữ OpenType w14: ligature none/standard (\"office ffi\" bằng Carlito), số kiểu cũ/bảng (numForm, numSpacing), bộ kiểu (stylisticSets), thay thế ngữ cảnh (cntxtAlts)", Doc {
        body: [
            rpara(&[(&format!(r#"<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/>{big}<w14:ligatures w14:val="none"/>"#), "office affine ffi — tắt")]),
            rpara(&[(&format!(r#"<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/>{big}<w14:ligatures w14:val="standard"/>"#), "office affine ffi — chuẩn")]),
            rpara(&[(&format!(r#"<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/>{big}<w14:numForm w14:val="oldStyle"/><w14:numSpacing w14:val="tabular"/>"#), "0123456789")]),
            rpara(&[(&format!(r#"<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/>{big}<w14:ligatures w14:val="standardContextual"/><w14:stylisticSets><w14:styleSet w14:id="1"/></w14:stylisticSets><w14:cntxtAlts/>"#), "Stylistic set 1")]),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    let ruby = |base: &str, guide: &str, align: &str| {
        format!(
            r#"<w:r><w:ruby><w:rubyPr><w:rubyAlign w:val="{align}"/><w:hps w:val="12"/><w:hpsRaise w:val="26"/><w:hpsBaseText w:val="26"/><w:lid w:val="vi-VN"/></w:rubyPr><w:rt><w:r><w:rPr><w:sz w:val="12"/></w:rPr><w:t>{guide}</w:t></w:r></w:rt><w:rubyBase><w:r><w:rPr><w:sz w:val="26"/></w:rPr><w:t>{base}</w:t></w:r></w:rubyBase></w:ruby></w:r>"#
        )
    };
    v.push(("text-07-ruby-combine.docx", "text", "Chú âm (w:ruby: rubyPr, rt, rubyBase; căn giữa/trái/phải) và hai dòng trong một (w:eastAsianLayout combine, ngoặc tròn/vuông), chữ ngang trong dọc (vert)", Doc {
        body: [
            format!(r#"<w:p><w:r><w:t xml:space="preserve">Chú âm: </w:t></w:r>{}<w:r><w:t xml:space="preserve"> · </w:t></w:r>{}<w:r><w:t xml:space="preserve"> · </w:t></w:r>{}</w:p>"#, ruby("Hà Nội", "thủ đô", "center"), ruby("Huế", "cố đô", "left"), ruby("ABC", "a-bê-xê", "right")),
            rpara(&[("", "Hai dòng trong một: "), (r#"<w:eastAsianLayout w:id="1" w:combine="1"/>"#, "BlueDoc"), ("", " — có ngoặc: "), (r#"<w:eastAsianLayout w:id="2" w:combine="1" w:combineBrackets="round"/>"#, "Dòng1Dòng2"), ("", " "), (r#"<w:eastAsianLayout w:id="3" w:combine="1" w:combineBrackets="square"/>"#, "ABCD")]),
            rpara(&[("", "Ngang trong dọc: "), (r#"<w:eastAsianLayout w:id="4" w:vert="1" w:vertCompress="1"/>"#, "12")]),
        ].concat(),
        sect: sect(""),
        ..Doc::default()
    }));
    // ---- building blocks: the glossary document part (§17.12)
    v.push(("blocks-01-autotext.docx", "blocks", "Glossary document (word/glossary/document.xml) có styles riêng: AutoText một đoạn chèn trong dòng (behavior content), AutoText nhiều đoạn có kiểu riêng của glossary (behavior p), Quick Part có định dạng trực tiếp và mô tả, mục giữ chỗ của content control (bbPlcHdr) được w:placeholder tham chiếu", Doc {
        body: [
            p("Gõ tên khối rồi nhấn F3 để chèn: kg, nn."),
            r#"<w:p><w:r><w:t xml:space="preserve">Người nhận: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Người nhận"/><w:id w:val="4101"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_4101"/></w:placeholder><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:color w:val="808080"/></w:rPr><w:t>Bấm để nhập tên</w:t></w:r></w:sdtContent></w:sdt></w:p>"#.to_owned(),
            lorem(2),
        ].concat(),
        sect: sect(""),
        glossary: Some(Gloss {
            parts: [
                doc_part(1, "kg", "autoTxt", "Hành chính", &format!("{}<w:description w:val=\"Lời kính gửi\"/>", part_kind("autoExp", "content")), r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Kính gửi: </w:t></w:r></w:p>"#),
                doc_part(2, "nn", "autoTxt", "Hành chính", &part_kind("normal", "p"), r#"<w:p><w:pPr><w:pStyle w:val="NoiNhan"/></w:pPr><w:r><w:t>Nơi nhận:</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="NoiNhanMuc"/></w:pPr><w:r><w:t>- Như trên;</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="NoiNhanMuc"/></w:pPr><w:r><w:t>- Lưu: VT.</w:t></w:r></w:p>"#),
                doc_part(3, "Lời cảm ơn", "docParts", "Thư từ", &format!("{}<w:description w:val=\"Câu kết thư &amp; lời chào\"/>", part_kind("normal", "content")), r#"<w:p><w:r><w:t xml:space="preserve">Xin </w:t></w:r><w:r><w:rPr><w:i/><w:color w:val="1F4E79"/></w:rPr><w:t>trân trọng cảm ơn</w:t></w:r><w:r><w:t xml:space="preserve"> sự quan tâm của Quý cơ quan.</w:t></w:r></w:p>"#),
                doc_part(4, "DefaultPlaceholder_4101", "placeholder", "General", &part_kind("bbPlcHdr", "content"), r#"<w:p><w:r><w:rPr><w:rStyle w:val="PlaceholderText"/></w:rPr><w:t>Bấm để nhập tên</w:t></w:r></w:p>"#),
            ].concat(),
            styles: Some(format!(r#"{}<w:style w:type="paragraph" w:customStyle="1" w:styleId="NoiNhan"><w:name w:val="Nơi nhận"/><w:basedOn w:val="Normal"/><w:next w:val="NoiNhanMuc"/><w:pPr><w:spacing w:before="120" w:after="0"/></w:pPr><w:rPr><w:b/><w:i/><w:sz w:val="24"/></w:rPr></w:style><w:style w:type="paragraph" w:customStyle="1" w:styleId="NoiNhanMuc"><w:name w:val="Nơi nhận mục"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:sz w:val="22"/></w:rPr></w:style><w:style w:type="character" w:styleId="PlaceholderText"><w:name w:val="Placeholder Text"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:semiHidden/><w:rPr><w:color w:val="808080"/></w:rPr></w:style>"#, default_styles())),
            ..Gloss::default()
        }),
        ..Doc::default()
    }));
    v.push(("blocks-02-galleries.docx", "blocks", "Glossary theo từng thư viện: đầu trang (hdrs, tab giữa/phải + sectPr cuối docPartBody), chân trang (ftrs, viền trên, PAGE/NUMPAGES), số trang (pgNumB), trang bìa (coverPg, behavior pg), bảng (tbls) và danh sách đánh số dùng numbering.xml riêng của glossary", Doc {
        body: lorem(3),
        sect: sect(""),
        glossary: Some(Gloss {
            parts: [
                doc_part(11, "Đầu trang ba phần", "hdrs", "Báo cáo", &part_kind("normal", "p"), &format!(r#"<w:p><w:pPr><w:tabs><w:tab w:val="center" w:pos="4677"/><w:tab w:val="right" w:pos="9355"/></w:tabs></w:pPr><w:r><w:t>[Cơ quan]</w:t></w:r><w:r><w:tab/><w:t>[Tên tài liệu]</w:t></w:r><w:r><w:tab/><w:t>[Ngày]</w:t></w:r></w:p>{}"#, sect(""))),
                doc_part(12, "Chân trang số trang", "ftrs", "Báo cáo", &part_kind("normal", "p"), &format!(r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="4" w:color="595959"/></w:pBdr><w:jc w:val="right"/></w:pPr><w:r><w:t xml:space="preserve">Trang </w:t></w:r>{}<w:r><w:t xml:space="preserve"> / </w:t></w:r>{}</w:p>"#, field_complex("PAGE", "1"), field_complex("NUMPAGES", "1"))),
                doc_part(13, "Số trang giữa", "pgNumB", "Đơn giản", &part_kind("normal", "p"), &format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#, field_simple("PAGE", "1"))),
                doc_part(14, "Bìa báo cáo", "coverPg", "Báo cáo", &format!("{}<w:description w:val=\"Trang bìa có khung tiêu đề\"/>", part_kind("normal", "pg")), &[
                    pr(r#"<w:spacing w:before="2400"/><w:jc w:val="center"/>"#, r#"<w:b/><w:sz w:val="28"/>"#, "[TÊN CƠ QUAN, TỔ CHỨC]"),
                    table(&[7000], &[row("", &[cell(7000, r#"<w:tcBorders><w:top w:val="single" w:sz="18" w:space="0" w:color="1F4E79"/><w:bottom w:val="single" w:sz="18" w:space="0" w:color="1F4E79"/></w:tcBorders>"#, "[TÊN TÀI LIỆU]")])], r#"<w:jc w:val="center"/>"#),
                    pr(r#"<w:jc w:val="center"/>"#, r#"<w:i/>"#, "[Địa danh], năm 20…"),
                ].concat()),
                doc_part(15, "Bảng ba cột", "tbls", "Chung", &part_kind("normal", "p"), &format!("{}{}", table(&[1000, 5355, 3000], &[
                    row(r#"<w:tblHeader/>"#, &[cell(1000, r#"<w:shd w:val="clear" w:color="auto" w:fill="DCE6F1"/>"#, "STT"), cell(5355, r#"<w:shd w:val="clear" w:color="auto" w:fill="DCE6F1"/>"#, "Nội dung"), cell(3000, r#"<w:shd w:val="clear" w:color="auto" w:fill="DCE6F1"/>"#, "Ghi chú")]),
                    row("", &[cell(1000, "", "1"), cell(5355, "", ""), cell(3000, "", "")]),
                ], ""), p(""))),
                doc_part(16, "Các bước thực hiện", "docParts", "Chung", &part_kind("normal", "p"), &[li(1, 0, "Bước một"), li(1, 0, "Bước hai"), li(1, 1, "Chi tiết của bước hai"), li(1, 0, "Bước ba")].concat()),
            ].concat(),
            numbering: Some(numbering(&[(0, &[("decimal", "%1."), ("lowerLetter", "%2)")])], r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#)),
            ..Gloss::default()
        }),
        ..Doc::default()
    }));
    v.push(("blocks-03-picture-custom.docx", "blocks", "Glossary có ảnh (quan hệ ảnh riêng của glossary tới ../media), thư viện tuỳ chỉnh custom1 và custQuickParts, hai mục trùng tên khác danh mục, w:style của docPart, khối chữ ký hai cột bằng bảng không viền", Doc {
        body: [p("Văn bản có khối dựng sẵn kèm ảnh."), lorem(1)].concat(),
        sect: sect(""),
        images: vec![("rId20".to_owned(), png(48, 24, [31, 78, 121]))],
        glossary: Some(Gloss {
            parts: [
                doc_part(21, "Biểu trưng", "custom1", "Nhận diện", &format!("{}<w:description w:val=\"Biểu trưng kèm tên cơ quan\"/>", part_kind("normal", "p")), &format!(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>{}"#, inline_pic(9001, "rId9", 914_400, 457_200, "", ""), pr(r#"<w:jc w:val="center"/>"#, r#"<w:b/><w:caps/>"#, "[Tên cơ quan, tổ chức]"))),
                doc_part(22, "Chữ ký", "custQuickParts", "Giám đốc", &part_kind("normal", "p"), &table(&[4677, 4678], &[row("", &[cell(4677, "", ""), cell(4678, "", "GIÁM ĐỐC")]), row("", &[cell(4677, "", ""), cell(4678, "", "[Họ và tên]")])], r#"<w:tblBorders><w:top w:val="nil"/><w:left w:val="nil"/><w:bottom w:val="nil"/><w:right w:val="nil"/><w:insideH w:val="nil"/><w:insideV w:val="nil"/></w:tblBorders>"#)),
                doc_part(23, "Chữ ký", "custQuickParts", "Trưởng phòng", &part_kind("normal", "p"), &pr(r#"<w:pStyle w:val="Heading1"/><w:jc w:val="right"/>"#, "", "TRƯỞNG PHÒNG")).replace("<w:category>", r#"<w:style w:val="Heading1"/><w:category>"#),
            ].concat(),
            images: vec![("rId9", 0)],
            ..Gloss::default()
        }),
        ..Doc::default()
    }));
    v
}

/// Settings with §17.15.1.32 displayBackgroundShape (before defaultTabStop, §17.15.1.78).
fn settings_bg() -> String {
    r#"<w:zoom w:percent="100"/><w:displayBackgroundShape/><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()
}

/// Write the corpus into `dir`; returns the README table.
/// The document the macro corpus file is made of (`macro_samples`).
pub fn macro_document() -> Result<Vec<u8>, bo_opc::OpcError> {
    Doc {
        body: p("Tệp mẫu có macro VBA. BlueOffice chỉ hiển thị mã, không bao giờ chạy."),
        sect: sect(""),
        ..Doc::default()
    }
    .build()
}

pub fn generate(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let mut readme = String::new();
    for (name, feature, desc, doc) in documents() {
        std::fs::write(dir.join(name), doc.build()?)?;
        readme.push_str(&format!(
            "| `doc/{name}` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | {feature}: {desc} |\n"
        ));
    }
    // Charts written by BlueDoc's own chart commands (CORE-CHART-007).
    for (name, desc, bytes) in crate::doc_charts::documents()? {
        std::fs::write(dir.join(name), bytes)?;
        readme.push_str(&format!(
            "| `doc/{name}` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/doc_charts.rs`) | chart: {desc} |\n"
        ));
    }
    // Mail-merge data sources (invented names) for the merge-* templates.
    let records = merge_records();
    let csv: String = records
        .iter()
        .map(|r| {
            r.iter()
                .map(|v| {
                    if v.contains([',', '"']) {
                        format!("\"{}\"", v.replace('"', "\"\""))
                    } else {
                        (*v).to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
                + "\n"
        })
        .collect();
    std::fs::write(dir.join("merge-data.csv"), csv)?;
    let mut wb = bo_model::sheet::Workbook::new();
    for (r, row) in records.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            wb.sheets[0].cells.insert(
                (r as u32, c as u32),
                bo_model::sheet::Cell {
                    value: bo_model::sheet::CellValue::Text((*v).to_owned()),
                    ..Default::default()
                },
            );
        }
    }
    std::fs::write(dir.join("merge-data.xlsx"), bo_xlsx::write_xlsx(&wb)?)?;
    for f in ["merge-data.csv", "merge-data.xlsx"] {
        readme.push_str(&format!(
            "| `doc/{f}` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | merge: nguồn dữ liệu trộn thư (3 bản ghi giả định) cho merge-01..03 |\n"
        ));
    }
    Ok(readme)
}

/// Mail-merge records: header row then three invented recipients.
fn merge_records() -> Vec<[&'static str; 4]> {
    vec![
        ["Họ tên", "ChucVu", "DonVi", "SoTien"],
        ["Nguyễn Văn An", "Giám đốc", "Sở Kế hoạch", "1.500.000"],
        ["Trần Thị Bình", "", "Phòng Tài chính, Kế toán", "2.000.000"],
        ["Lê Văn Cường", "Trưởng phòng", "Văn phòng UBND", "750.000"],
    ]
}

/// A photo-like `w × h` RGB PNG: a gradient tinted by `seed` plus low-amplitude
/// pseudo-random noise, so that it compresses like a real picture rather than a
/// flat colour (benchmark input, not part of the round-trip corpus).
pub(crate) fn photo_png(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut state = seed.wrapping_mul(2_654_435_761).max(1);
    let tint = seed.wrapping_mul(37).to_le_bytes();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let Ok(mut wr) = enc.write_header() else {
            return Vec::new();
        };
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                // xorshift32
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let noise = state.to_le_bytes()[0] & 0x1f;
                let gx = u8::try_from(x * 200 / w.max(1)).unwrap_or(200);
                let gy = u8::try_from(y * 200 / h.max(1)).unwrap_or(200);
                data.push(gx.wrapping_add(tint[0]) / 2 + noise);
                data.push(gy / 2 + noise);
                data.push(tint[1] / 2 + noise);
            }
        }
        if wr.write_image_data(&data).is_err() {
            return Vec::new();
        }
    }
    out
}

/// Benchmark document of BlueOffice-PROMPT §5 ("Mở .docx 300 trang có ảnh"): about
/// `pages` A4 pages of justified Vietnamese text (Times New Roman 13 pt) with one
/// inline 640×480 picture (8 × 6 cm) after every `image_every` paragraphs, cycling
/// through `distinct` different image parts. Not part of the round-trip corpus (it is
/// several MB); written by `bo-cli sample-big`.
pub fn big_document(
    pages: usize,
    image_every: usize,
    distinct: usize,
) -> Result<Vec<u8>, bo_opc::OpcError> {
    // Six body paragraphs and one 6 cm picture fill about one A4 page (measured).
    let paragraphs = pages * 6;
    let image_every = image_every.max(1);
    let distinct = distinct.max(1);
    let (cx, cy) = (2_880_000_i64, 2_160_000_i64); // 8 cm × 6 cm in EMU
    let mut body = String::new();
    let mut pic_id = 0_u32;
    for i in 0..paragraphs {
        // ECMA-376 Part 1, §17.3.1.13 jc; §17.3.1.12 ind
        body.push_str(&format!(
            r#"<w:p><w:pPr><w:jc w:val="both"/><w:ind w:firstLine="567"/></w:pPr><w:r><w:t xml:space="preserve">{} {} {} {}</w:t></w:r></w:p>"#,
            i + 1,
            VI[i % VI.len()],
            VI[(i + 1) % VI.len()],
            VI[(i + 2) % VI.len()]
        ));
        if i % image_every == image_every - 1 {
            pic_id += 1;
            let rid = format!("rIdImg{}", (pic_id as usize - 1) % distinct + 1);
            body.push_str(&format!(
                r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#,
                inline_pic(pic_id, &rid, cx, cy, "", "")
            ));
        }
    }
    let images = (0..distinct)
        .map(|k| {
            let seed = u32::try_from(k).unwrap_or(0) + 1;
            (format!("rIdImg{seed}"), photo_png(640, 480, seed))
        })
        .collect();
    Doc {
        body,
        sect: sect(""),
        images,
        ..Doc::default()
    }
    .build()
}

/// Realistic long document for CORE-PERF-001 ("`.docx` 1 000 trang mở < 8 s"): about
/// `pages` A4 pages of blocks of four justified paragraphs with one footnote (ECMA-376
/// Part 1, §17.11.14 footnoteReference) followed alternately by a 3 × 5 table
/// (§17.4.38 tbl) with a numbered caption (§17.16.5.56 SEQ) or an inline picture
/// (cycling through `distinct` image parts), a chapter heading (Heading1,
/// §17.3.1.20 outlineLvl) every 20 pages, a header and a footer with PAGE / NUMPAGES
/// fields (§17.16.5.44 / §17.16.5.42). Generated for benchmarks, never committed.
pub fn big_document_rich(pages: usize, distinct: usize) -> Result<Vec<u8>, bo_opc::OpcError> {
    let distinct = distinct.max(1);
    let (cx, cy) = (2_880_000_i64, 2_160_000_i64); // 8 cm × 6 cm in EMU
    let mut body = String::new();
    let mut notes = String::new();
    let mut pic_id = 0_u32;
    let mut table_no = 0_u32;
    // ECMA-376 Part 1, §17.3.1.13 jc; §17.3.1.12 ind
    let jc = r#"<w:pPr><w:jc w:val="both"/><w:ind w:firstLine="567"/></w:pPr>"#;
    // One unit (four paragraphs + a table or a picture, one footnote) fills about 5/8 of
    // an A4 page (measured: 1 600 units = 1 000 pages).
    let units = pages * 8 / 5;
    for k in 0..units {
        if k % 32 == 0 {
            // ECMA-376 Part 1, §17.3.1.27 pStyle
            body.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chương {}. Báo cáo giả định</w:t></w:r></w:p>"#,
                k / 32 + 1
            ));
        }
        let note = k + 1;
        for j in 0..4 {
            let i = k * 4 + j;
            let fref = if j == 1 {
                format!(
                    r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{note}"/></w:r>"#
                )
            } else {
                String::new()
            };
            body.push_str(&format!(
                r#"<w:p>{jc}<w:r><w:t xml:space="preserve">{} {} {}</w:t></w:r>{fref}<w:r><w:t xml:space="preserve"> {} {}</w:t></w:r></w:p>"#,
                i + 1,
                VI[i % VI.len()],
                VI[(i + 1) % VI.len()],
                VI[(i + 2) % VI.len()],
                VI[(i + 3) % VI.len()],
            ));
        }
        // ECMA-376 Part 1, §17.11.10 footnote
        notes.push_str(&format!(
            r#"<w:footnote w:id="{note}"><w:p><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Ghi chú {note}: {}</w:t></w:r></w:p></w:footnote>"#,
            VI[k % VI.len()]
        ));
        if k % 2 == 0 {
            table_no += 1;
            body.push_str(&format!(
                r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Bảng </w:t></w:r>{}<w:r><w:t xml:space="preserve">. Số liệu kỳ {}</w:t></w:r></w:p>"#,
                field_simple(r"SEQ Bang \* ARABIC", &table_no.to_string()),
                k + 1
            ));
            // ECMA-376 Part 1, §17.4.49 tblHeader
            let mut rows = vec![row(
                "<w:tblHeader/>",
                &[
                    cell(3000, "", "Đơn vị"),
                    cell(3000, "", "Kế hoạch"),
                    cell(3000, "", "Thực hiện"),
                ],
            )];
            for r in 0..4 {
                let v = (k * 7 + r * 13) % 1000;
                rows.push(row(
                    "",
                    &[
                        cell(3000, "", &format!("Phòng {}", r + 1)),
                        cell(3000, "", &format!("{}.000", v + 100)),
                        cell(3000, "", &format!("{}.500", v + 90)),
                    ],
                ));
            }
            body.push_str(&table(&[3000, 3000, 3000], &rows, ""));
        } else {
            pic_id += 1;
            let rid = format!("rIdImg{}", (pic_id as usize - 1) % distinct + 1);
            body.push_str(&format!(
                r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr>{}</w:p>"#,
                inline_pic(pic_id, &rid, cx, cy, "", "")
            ));
        }
    }
    let images = (0..distinct)
        .map(|k| {
            let seed = u32::try_from(k).unwrap_or(0) + 1;
            (format!("rIdImg{seed}"), photo_png(640, 480, seed))
        })
        .collect();
    Doc {
        body,
        sect: sect(
            &[
                hdr_ref("header", "default", "rId10"),
                hdr_ref("footer", "default", "rId11"),
            ]
            .concat(),
        ),
        hf: vec![
            (
                "rId10".into(),
                "header",
                p("CÔNG TY GIẢ ĐỊNH – Báo cáo tổng hợp"),
            ),
            (
                "rId11".into(),
                "footer",
                format!(
                    r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">Trang </w:t></w:r>{}<w:r><w:t>/</w:t></w:r>{}</w:p>"#,
                    field_complex("PAGE", "1"),
                    field_simple("NUMPAGES", "1")
                ),
            ),
        ],
        footnotes: Some(notes),
        images,
        ..Doc::default()
    }
    .build()
}

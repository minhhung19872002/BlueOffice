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
            &format!("<w:body>{}{}</w:body>", self.body, self.sect),
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
    // ---- fields to update: TOC (§17.16.5.68), SEQ (§17.16.5.56), REF/PAGEREF (§17.16.5.49/42)
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
    v
}

/// Write the corpus into `dir`; returns the README table.
pub fn generate(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let mut readme = String::new();
    for (name, feature, desc, doc) in documents() {
        std::fs::write(dir.join(name), doc.build()?)?;
        readme.push_str(&format!(
            "| `doc/{name}` | Tự sinh: `cargo run -p bo-cli -- corpus tests/corpus/doc` (`crates/bo-cli/src/corpus.rs`) | {feature}: {desc} |\n"
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
fn photo_png(w: u32, h: u32, seed: u32) -> Vec<u8> {
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

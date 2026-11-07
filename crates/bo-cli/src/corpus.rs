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
        sect: s,
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
    Ok(readme)
}

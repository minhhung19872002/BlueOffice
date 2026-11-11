//! Groups of shapes in WordprocessingML: a `w:drawing` (ECMA-376 Part 1, §17.3.3.9) whose
//! `a:graphicData@uri` is the wordprocessing group namespace and holds a `wpg:wgp`
//! ([MS-ODRAWXML] wordprocessingGroup: `cNvGrpSpPr`, `grpSpPr`, then `wps:wsp` shapes and nested
//! `wpg:grpSp` groups). This is what "Convert to shapes" makes of a diagram in BlueDoc
//! (CORE-SMART-004).
//!
//! In the document model a group is an [`bo_model::Image`] whose media
//! ([`bo_model::diagram::group_name`]) holds the shapes as a DrawingML shape tree in the form of a
//! diagram drawing part (`dsp:drawing`): that is what the shape renderer draws. Here the tree is
//! written as a `wpg:wgp` and a `wpg:wgp` of preset shapes with text is read into one. The two
//! differ in their text: a `wps:wsp` holds WordprocessingML paragraphs (`wps:txbx` /
//! `w:txbxContent`, §17.17.1), a DrawingML shape a text body (`a:p`, §21.1.2); paragraph
//! alignment and the runs' text, size, colour, bold and italic are carried over.

use bo_drawingml::dom::write_element;
use bo_model::slide::{XmlElement, XmlNode};
use std::fmt::Write as _;

pub(crate) const WPG_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup";
const WPS_NS: &str = crate::shape::WPS_NS;
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DGM_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const DSP_NS: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";

fn kid<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|c| c.local() == local)
}

fn esc(s: &str) -> String {
    bo_opc::xml::escape(s)
}

fn xml_of(el: &XmlElement) -> String {
    let mut s = String::new();
    write_element(el, &mut s);
    s
}

/// `el` renamed to `name`, namespace declarations dropped, picture fills (`a:blip@r:embed`,
/// §20.1.8.13) mapped through `blip` (the relationship namespace is declared on the blip).
fn renamed(
    el: &XmlElement,
    name: &str,
    blip: &mut dyn FnMut(&str) -> Option<String>,
) -> XmlElement {
    fn walk(el: &XmlElement, blip: &mut dyn FnMut(&str) -> Option<String>) -> XmlElement {
        let mut out = XmlElement::new(&el.name);
        let is_blip = el.local() == "blip";
        for (k, v) in &el.attrs {
            if k == "xmlns" || k.starts_with("xmlns:") {
                continue;
            }
            if is_blip && (k == "r:embed" || k == "r:link") {
                if let Some(to) = blip(v) {
                    out.attrs.push(("xmlns:r".into(), R_NS.into()));
                    out.attrs.push((k.clone(), to));
                }
                continue;
            }
            out.attrs.push((k.clone(), v.clone()));
        }
        for c in &el.children {
            out.children.push(match c {
                XmlNode::Element(e) => XmlNode::Element(walk(e, blip)),
                XmlNode::Text(t) => XmlNode::Text(t.clone()),
            });
        }
        out
    }
    let mut out = walk(el, blip);
    out.name = name.to_owned();
    out
}

/// §21.1.2.2.6 `a:p` → §17.3.1.22 `w:p`: alignment (§21.1.2.2.7 pPr@algn → §17.3.1.13 jc) and runs
/// (§21.1.2.3.8 r: text, §21.1.2.3.9 rPr@sz / @b / @i and a plain RGB fill → §17.3.2 sz, b, i,
/// color).
fn w_paragraph(p: &XmlElement) -> String {
    let jc = match kid(p, "pPr").and_then(|pp| pp.attr("algn")) {
        Some("ctr") => "center",
        Some("r") => "right",
        Some("just") => "both",
        _ => "left",
    };
    let mut out = format!(
        "<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/><w:jc w:val=\"{jc}\"/></w:pPr>"
    );
    for r in p.elements() {
        match r.local() {
            "r" | "fld" => {
                let rpr = kid(r, "rPr");
                let on = |a: &str| {
                    rpr.and_then(|x| x.attr(a))
                        .is_some_and(|v| v == "1" || v == "true")
                };
                let mut props = String::new();
                if on("b") {
                    props.push_str("<w:b/>");
                }
                if on("i") {
                    props.push_str("<w:i/>");
                }
                if let Some(hex) = rpr
                    .and_then(|x| kid(x, "solidFill"))
                    .and_then(|f| kid(f, "srgbClr"))
                    .and_then(|c| c.attr("val"))
                    .filter(|v| v.len() == 6 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    let _ = write!(props, "<w:color w:val=\"{hex}\"/>");
                }
                // `sz` is in hundredths of a point, `w:sz` in half-points.
                if let Some(sz) = rpr
                    .and_then(|x| x.attr("sz"))
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    let half = (sz / 50).max(2);
                    let _ = write!(props, "<w:sz w:val=\"{half}\"/><w:szCs w:val=\"{half}\"/>");
                }
                let text = kid(r, "t").map(XmlElement::text).unwrap_or_default();
                let _ = write!(
                    out,
                    "<w:r><w:rPr>{props}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    esc(&text)
                );
            }
            "br" => out.push_str("<w:r><w:br/></w:r>"),
            _ => {}
        }
    }
    out.push_str("</w:p>");
    out
}

#[allow(clippy::single_match_else)]
/// One `dsp:sp` → `wps:wsp`, or `dsp:grpSp` → `wpg:grpSp` (`id`: the next free drawing id).
fn w_node(el: &XmlElement, id: &mut u32, blip: &mut dyn FnMut(&str) -> Option<String>) -> String {
    let me = *id;
    *id += 1;
    match el.local() {
        "grpSp" => {
            let pr = kid(el, "grpSpPr").map_or_else(
                || "<wpg:grpSpPr/>".to_owned(),
                |p| xml_of(&renamed(p, "wpg:grpSpPr", blip)),
            );
            let mut out = format!(
                "<wpg:grpSp><wpg:cNvPr id=\"{me}\" name=\"Group {me}\"/><wpg:cNvGrpSpPr/>{pr}"
            );
            for c in el
                .elements()
                .filter(|c| matches!(c.local(), "sp" | "grpSp"))
            {
                out.push_str(&w_node(c, id, blip));
            }
            out.push_str("</wpg:grpSp>");
            out
        }
        _ => {
            // [MS-ODRAWXML] wsp: cNvPr?, cNvSpPr, spPr, style?, txbx?, bodyPr.
            let mut out =
                format!("<wps:wsp><wps:cNvPr id=\"{me}\" name=\"Shape {me}\"/><wps:cNvSpPr/>");
            match kid(el, "spPr") {
                Some(p) => out.push_str(&xml_of(&renamed(p, "wps:spPr", blip))),
                None => out.push_str("<wps:spPr/>"),
            }
            if let Some(s) = kid(el, "style") {
                out.push_str(&xml_of(&renamed(s, "wps:style", blip)));
            }
            let body = kid(el, "txBody");
            let paras: Vec<&XmlElement> = body
                .map(|b| b.elements().filter(|c| c.local() == "p").collect())
                .unwrap_or_default();
            if !paras.is_empty() {
                out.push_str("<wps:txbx><w:txbxContent>");
                for p in paras {
                    out.push_str(&w_paragraph(p));
                }
                out.push_str("</w:txbxContent></wps:txbx>");
            }
            match body.and_then(|b| kid(b, "bodyPr")) {
                Some(b) => out.push_str(&xml_of(&renamed(b, "wps:bodyPr", blip))),
                None => out.push_str("<wps:bodyPr/>"),
            }
            out.push_str("</wps:wsp>");
            out
        }
    }
}

/// The `a:graphic` holding the `wpg:wgp` of shape group `drawing` (the group's media: a
/// `dsp:drawing`) for a frame of `cx` × `cy` EMU. `id` is the next free drawing id (advanced by
/// the ids the shapes take); `blip` gives the relationship id of an image part.
pub(crate) fn group_graphic(
    drawing: &[u8],
    (cx, cy): (i64, i64),
    id: &mut u32,
    blip: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<String> {
    let root = bo_drawingml::dom::parse(drawing, "/group.xml").ok()?;
    let tree = kid(&root, "spTree")?;
    // The child space: the size the shapes were laid out for.
    let ch = kid(tree, "grpSpPr")
        .and_then(|g| kid(g, "xfrm"))
        .and_then(|x| kid(x, "chExt"))
        .and_then(|e| {
            Some((
                e.attr("cx")?.parse::<i64>().ok()?,
                e.attr("cy")?.parse::<i64>().ok()?,
            ))
        })
        .filter(|(w, h)| *w > 0 && *h > 0)
        .unwrap_or((cx, cy));
    let mut out = format!(
        "<a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{WPG_NS}\"><wpg:wgp xmlns:wpg=\"{WPG_NS}\" xmlns:wps=\"{WPS_NS}\"><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"{}\" cy=\"{}\"/></a:xfrm></wpg:grpSpPr>",
        ch.0, ch.1
    );
    for c in tree
        .elements()
        .filter(|c| matches!(c.local(), "sp" | "grpSp"))
    {
        out.push_str(&w_node(c, id, blip));
    }
    out.push_str("</wpg:wgp></a:graphicData></a:graphic>");
    Some(out)
}

/// §17.3.1.22 `w:p` → §21.1.2.2.6 `a:p` (the inverse of [`w_paragraph`]).
fn a_paragraph(p: &XmlElement) -> String {
    let algn = match kid(p, "pPr")
        .and_then(|pp| kid(pp, "jc"))
        .and_then(|j| j.attr("w:val"))
    {
        Some("center") => "ctr",
        Some("right" | "end") => "r",
        Some("both" | "distribute") => "just",
        _ => "l",
    };
    let mut out = format!("<a:p><a:pPr algn=\"{algn}\"/>");
    for r in p.elements().filter(|r| r.local() == "r") {
        let rpr = kid(r, "rPr");
        let on = |n: &str| {
            rpr.and_then(|x| kid(x, n))
                .is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")))
        };
        let mut attrs = String::from(" lang=\"vi-VN\"");
        if let Some(half) = rpr
            .and_then(|x| kid(x, "sz"))
            .and_then(|s| s.attr("w:val"))
            .and_then(|v| v.parse::<u32>().ok())
        {
            let _ = write!(attrs, " sz=\"{}\"", half * 50);
        }
        if on("b") {
            attrs.push_str(" b=\"1\"");
        }
        if on("i") {
            attrs.push_str(" i=\"1\"");
        }
        let fill = rpr
            .and_then(|x| kid(x, "color"))
            .and_then(|c| c.attr("w:val"))
            .filter(|v| v.len() == 6 && v.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|hex| format!("<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>"))
            .unwrap_or_default();
        for c in r.elements() {
            match c.local() {
                "t" => {
                    let _ = write!(
                        out,
                        "<a:r><a:rPr{attrs}>{fill}</a:rPr><a:t>{}</a:t></a:r>",
                        esc(&c.text())
                    );
                }
                "br" => out.push_str("<a:br/>"),
                _ => {}
            }
        }
    }
    out.push_str("</a:p>");
    out
}

/// One `wps:wsp` → `dsp:sp`, or `wpg:grpSp` → `dsp:grpSp`; `None` when the node is something a
/// group of preset shapes with text does not hold (a picture, a canvas, a custom geometry, a
/// linked text box, a text box with a table).
fn d_node(
    el: &XmlElement,
    n: &mut u32,
    blip: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<String> {
    *n += 1;
    let me = *n;
    match el.local() {
        "grpSp" => {
            let pr = kid(el, "grpSpPr").map_or_else(
                || "<dsp:grpSpPr/>".to_owned(),
                |p| xml_of(&renamed(p, "dsp:grpSpPr", blip)),
            );
            let mut out = format!(
                "<dsp:grpSp><dsp:nvGrpSpPr><dsp:cNvPr id=\"{me}\" name=\"\"/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr>{pr}"
            );
            for c in el.elements() {
                match c.local() {
                    "cNvPr" | "cNvGrpSpPr" | "grpSpPr" | "extLst" => {}
                    _ => out.push_str(&d_node(c, n, blip)?),
                }
            }
            out.push_str("</dsp:grpSp>");
            Some(out)
        }
        "wsp" => {
            let sp_pr = kid(el, "spPr")?;
            kid(sp_pr, "prstGeom")?;
            if kid(el, "linkedTxbx").is_some() {
                return None;
            }
            let mut out = format!(
                "<dsp:sp modelId=\"{me}\"><dsp:nvSpPr><dsp:cNvPr id=\"{me}\" name=\"\"/><dsp:cNvSpPr/></dsp:nvSpPr>{}",
                xml_of(&renamed(sp_pr, "dsp:spPr", blip))
            );
            if let Some(s) = kid(el, "style") {
                out.push_str(&xml_of(&renamed(s, "dsp:style", blip)));
            }
            let content = kid(el, "txbx").and_then(|t| kid(t, "txbxContent"));
            if let Some(content) = content {
                if content.elements().any(|b| b.local() != "p") {
                    return None;
                }
                let body = kid(el, "bodyPr").map_or_else(
                    || "<a:bodyPr/>".to_owned(),
                    |b| xml_of(&renamed(b, "a:bodyPr", blip)),
                );
                let _ = write!(out, "<dsp:txBody>{body}<a:lstStyle/>");
                for p in content.elements() {
                    out.push_str(&a_paragraph(p));
                }
                out.push_str("</dsp:txBody>");
            }
            out.push_str("</dsp:sp>");
            Some(out)
        }
        _ => None,
    }
}

/// The shapes of a `wpg:wgp` as a shape group's media (`dsp:drawing`); `None` unless it is a
/// group of preset shapes with text (anything else keeps the reader's previous handling).
/// `blip` gives the image part name of a relationship id of the part holding the drawing.
pub(crate) fn drawing_from_wgp(
    wgp: &XmlElement,
    blip: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<String> {
    let pr = kid(wgp, "grpSpPr")?;
    let mut body = String::new();
    let mut n = 0;
    let mut shapes = 0;
    for c in wgp.elements() {
        match c.local() {
            "cNvPr" | "cNvGrpSpPr" | "grpSpPr" | "extLst" => {}
            _ => {
                body.push_str(&d_node(c, &mut n, blip)?);
                shapes += 1;
            }
        }
    }
    if shapes == 0 {
        return None;
    }
    Some(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<dsp:drawing xmlns:dgm=\"{DGM_NS}\" xmlns:dsp=\"{DSP_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id=\"0\" name=\"\"/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr>{}{body}</dsp:spTree></dsp:drawing>",
        xml_of(&renamed(pr, "dsp:grpSpPr", blip))
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const DRAWING: &str = r#"<dsp:drawing xmlns:dgm="d" xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="r"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="4000" cy="2000"/><a:chOff x="0" y="0"/><a:chExt cx="4000" cy="2000"/></a:xfrm></dsp:grpSpPr><dsp:sp modelId="1"><dsp:nvSpPr><dsp:cNvPr id="1" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="1F6FB2"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></dsp:spPr><dsp:txBody><a:bodyPr anchor="ctr" wrap="square"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="1400" b="1"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>Bước &amp; 1</a:t></a:r></a:p></dsp:txBody></dsp:sp><dsp:sp modelId="2"><dsp:nvSpPr><dsp:cNvPr id="2" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="2000" y="0"/><a:ext cx="500" cy="500"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:blipFill><a:blip r:embed="/word/media/image7.png"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></dsp:spPr></dsp:sp></dsp:spTree></dsp:drawing>"#;

    // source: [MS-ODRAWXML] wordprocessingGroup (wgp: cNvGrpSpPr, grpSpPr, wsp…) and
    // wordprocessingShape (wsp: cNvPr, cNvSpPr, spPr, style, txbx, bodyPr); ECMA-376 Part 1,
    // §17.17.1 txbxContent, §17.3.2 run properties (sz in half-points).
    #[test]
    fn a_shape_tree_is_written_as_a_group_and_read_back() {
        let mut id = 5;
        let mut rel = |name: &str| Some(format!("rId-of-{}", name.rsplit('/').next().unwrap()));
        let g = group_graphic(DRAWING.as_bytes(), (8000, 4000), &mut id, &mut rel).unwrap();
        assert_eq!(id, 7, "two drawing ids taken");
        assert!(g.starts_with(&format!(
            "<a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{WPG_NS}\"><wpg:wgp "
        )));
        // The frame is twice the size the shapes were laid out for: the child space scales them.
        assert!(g.contains(
            r#"<a:ext cx="8000" cy="4000"/><a:chOff x="0" y="0"/><a:chExt cx="4000" cy="2000"/>"#
        ));
        assert!(g.contains(
            r#"<wps:wsp><wps:cNvPr id="5" name="Shape 5"/><wps:cNvSpPr/><wps:spPr><a:xfrm>"#
        ));
        // 14 pt bold white centred text as WordprocessingML: sz 28 half-points.
        assert!(g.contains(r#"<wps:txbx><w:txbxContent><w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val="FFFFFF"/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr><w:t xml:space="preserve">Bước &amp; 1</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr anchor="ctr" wrap="square"><a:normAutofit/></wps:bodyPr></wps:wsp>"#), "{g}");
        // The picture fill names a relationship; a shape without text still has its bodyPr.
        assert!(g.contains(&format!(
            r#"<a:blip xmlns:r="{R_NS}" r:embed="rId-of-image7.png"/>"#
        )));
        assert!(g.ends_with(
            "</a:blipFill></wps:spPr><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic>"
        ));
        // Read back: the same shapes, text and picture (by part name).
        let dom = bo_drawingml::dom::parse(
            format!("<x xmlns:w=\"w\" xmlns:a=\"a\">{g}</x>")
                .replace("xmlns:a=\"http", "xmlns:a2=\"http")
                .as_bytes(),
            "/x",
        )
        .unwrap();
        let wgp = crate::shape::find_local(&dom, "wgp").unwrap();
        let mut back =
            |rid: &str| Some(format!("/word/media/{}", rid.trim_start_matches("rId-of-")));
        let d = drawing_from_wgp(wgp, &mut back).unwrap();
        assert!(d.contains(r#"<a:chExt cx="4000" cy="2000"/>"#));
        assert!(d.contains(r#"<a:prstGeom prst="roundRect">"#));
        assert!(d.contains(r#"<a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="1400" b="1"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>Bước &amp; 1</a:t></a:r></a:p>"#), "{d}");
        assert!(d.contains(r#"r:embed="/word/media/image7.png""#));
        // A group holding anything else than preset shapes is not taken.
        let other = bo_drawingml::dom::parse(
            br#"<wpg:wgp xmlns:wpg="g" xmlns:pic="p"><wpg:cNvGrpSpPr/><wpg:grpSpPr/><pic:pic/></wpg:wgp>"#,
            "/x",
        )
        .unwrap();
        assert!(drawing_from_wgp(&other, &mut back).is_none());
        let empty = bo_drawingml::dom::parse(
            br#"<wpg:wgp xmlns:wpg="g"><wpg:cNvGrpSpPr/><wpg:grpSpPr/></wpg:wgp>"#,
            "/x",
        )
        .unwrap();
        assert!(drawing_from_wgp(&empty, &mut back).is_none());
    }
}

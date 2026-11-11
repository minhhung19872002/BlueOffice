//! Chartex charts on slides (CORE-CHART-004): a graphic frame (ECMA-376 Part 1, §19.3.1.21)
//! whose `graphicData` (uri = the chartex namespace, [MS-ODRAWXML] 2.24) holds `cx:chart r:id`,
//! wrapped by producers in `mc:AlternateContent` (ECMA-376 Part 3, §10.2.1) with a `Requires`
//! choice (`cx1`, `cx2`, …) and a fallback shape for applications without chartex.
//!
//! The reader keeps the wrapper in [`GraphicFrame::alt`] and models the frame itself, so the
//! editor selects, moves, resizes and names it like any frame; the writer puts the frame back in
//! its choice, and the fallback shape follows a moved or renamed frame.

use bo_model::slide::{GraphicFrame, ShapeNode, XmlElement, XmlNode};

/// Chartex namespace (also the `graphicData@uri`).
pub use bo_xlsx::chartex::CX_NS as NS_CX;

/// Is this frame a chartex chart?
#[must_use]
pub fn is_chartex(f: &GraphicFrame) -> bool {
    f.uri == NS_CX
}

fn is_cx_frame(e: &XmlElement) -> bool {
    e.name == "p:graphicFrame"
        && e.elements()
            .find(|g| g.name == "a:graphic")
            .and_then(|g| g.child("a:graphicData"))
            .and_then(|d| d.attr("uri"))
            == Some(NS_CX)
}

/// The chartex frame of an `mc:AlternateContent` whose choice holds just that frame.
#[must_use]
pub fn alternate_chartex_frame(el: &XmlElement) -> Option<&XmlElement> {
    if el.local() != "AlternateContent" {
        return None;
    }
    el.elements()
        .filter(|c| c.local() == "Choice")
        .find_map(|c| {
            let mut kids = c.elements();
            let f = kids.next()?;
            (kids.next().is_none() && is_cx_frame(f)).then_some(f)
        })
}

/// The wrapper to write for a chartex frame: `alt` with its choice frame replaced by `frame_xml`;
/// when the frame moved, was resized or renamed since it was read, the fallback shape follows.
#[must_use]
pub fn write_alternate(f: &GraphicFrame, alt: &XmlElement, frame_xml: &XmlElement) -> XmlElement {
    let (moved, renamed) = match alternate_chartex_frame(alt).map(crate::parse_node) {
        Some(ShapeNode::Frame(o)) => (o.xfrm != f.xfrm, o.nv.name != f.nv.name),
        _ => (true, true),
    };
    let mut out = alt.clone();
    for c in &mut out.children {
        let XmlNode::Element(c) = c else { continue };
        match c.local() {
            "Choice" if c.elements().any(is_cx_frame) => {
                for n in &mut c.children {
                    if matches!(n, XmlNode::Element(e) if is_cx_frame(e)) {
                        *n = XmlNode::Element(frame_xml.clone());
                    }
                }
            }
            "Fallback" => {
                for n in &mut c.children {
                    let XmlNode::Element(e) = n else { continue };
                    if moved {
                        crate::model3d::set_xfrm(e, &f.xfrm);
                    }
                    if renamed {
                        crate::model3d::set_cnv(e, &f.nv);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A new chartex frame on a slide: `mc:AlternateContent` with a choice requiring the chartex
/// namespace of `kind` (funnel charts came with the 2015/10/21 namespace `cx2`, region maps with
/// the 2016/5/10 one `cx4`, the others with the 2015/9/8 one `cx1`) and a fallback text box.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn new_frame_xml(
    id: u32,
    name: &str,
    (x, y, cx, cy): (i64, i64, i64, i64),
    rid: &str,
    requires: (&str, &str),
    fallback_text: &str,
) -> String {
    let esc = bo_opc::xml::escape;
    let (req, req_ns) = requires;
    format!(
        concat!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">"#,
            r#"<mc:Choice xmlns:{req}="{req_ns}" Requires="{req}"><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{name}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm><a:graphic><a:graphicData uri="{ns}"><cx:chart xmlns:cx="{ns}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{rid}"/></a:graphicData></a:graphic></p:graphicFrame></mc:Choice>"#,
            r#"<mc:Fallback><p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></mc:Fallback></mc:AlternateContent>"#
        ),
        req = req,
        req_ns = req_ns,
        id = id,
        name = esc(name),
        x = x,
        y = y,
        cx = cx,
        cy = cy,
        ns = NS_CX,
        rid = esc(rid),
        text = esc(fallback_text),
    )
}

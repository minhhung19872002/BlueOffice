//! Ink on slides (CORE-INK-001): the content part elements that reference an
//! InkML part ([MS-ODRAWXML] §1.3.2, §2.1.4).
//!
//! On a slide, ink is a `p:contentPart` (ECMA-376 Part 1, §19.3.1.14) whose
//! `r:id` points at an `application/inkml+xml` part. Producers write it inside
//! `mc:AlternateContent` (ECMA-376 Part 3, §10.2.1) with a `Requires="p14"`
//! choice that adds the [MS-PPTX] non-visual properties (`p14:nvContentPartPr`)
//! and frame (`p14:xfrm`), and a fallback (a picture or shape) for consumers
//! that do not understand ink. The model keeps these nodes as
//! [`ShapeNode::Other`]; this module finds and builds them.

use bo_drawingml::props::parse_xfrm;
use bo_model::slide::{ShapeNode, Xfrm, XmlElement};

/// [MS-PPTX] PowerPoint 2010 namespace (`p14`).
pub const P14_NS: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
/// Markup Compatibility namespace (ECMA-376 Part 3, §10.2.1).
pub const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// The `contentPart` element of a node: itself, or the one in a choice of its
/// `mc:AlternateContent`.
pub fn content_part(el: &XmlElement) -> Option<&XmlElement> {
    match el.local() {
        "contentPart" => Some(el),
        "AlternateContent" => el
            .elements()
            .filter(|c| c.local() == "Choice")
            .find_map(|c| c.elements().find(|e| e.local() == "contentPart")),
        _ => None,
    }
}

/// The content part of a shape-tree node, if it is ink.
pub fn node_content_part(n: &ShapeNode) -> Option<&XmlElement> {
    match n {
        ShapeNode::Other(el) => content_part(el),
        _ => None,
    }
}

/// `r:id` of a content part (ECMA-376 Part 1, §19.3.1.14 contentPart@r:id).
pub fn content_part_rid(cp: &XmlElement) -> Option<&str> {
    cp.attrs
        .iter()
        .find(|(k, _)| k.ends_with(":id") && k != "xml:id")
        .map(|(_, v)| v.as_str())
}

/// Frame of a content part ([MS-PPTX] `p14:xfrm`, an a:CT_Transform2D).
pub fn content_part_xfrm(cp: &XmlElement) -> Option<Xfrm> {
    cp.elements()
        .find(|e| e.local() == "xfrm")
        .map(parse_xfrm)
        .filter(|x| x.off.is_some() && x.ext.is_some())
}

/// `cNvPr` (id, name) of a content part ([MS-PPTX] `p14:nvContentPartPr`).
pub fn content_part_nv(cp: &XmlElement) -> Option<(u32, String)> {
    let nv = cp.elements().find(|e| e.local() == "nvContentPartPr")?;
    let c = nv.elements().find(|e| e.local() == "cNvPr")?;
    let id = c.attr("id")?.parse().ok()?;
    Some((id, c.attr("name").unwrap_or_default().to_owned()))
}

/// Replace the frame of the content part inside `el` (`p14:xfrm` off/ext, EMU).
pub fn set_content_part_frame(el: &mut XmlElement, off: (i64, i64), ext: (i64, i64)) {
    fn visit(e: &mut XmlElement, off: (i64, i64), ext: (i64, i64)) {
        if e.local() == "xfrm" && e.name.starts_with("p14:") {
            for c in &mut e.children {
                if let bo_model::slide::XmlNode::Element(c) = c {
                    match c.local() {
                        "off" => {
                            c.attrs = vec![
                                ("x".into(), off.0.to_string()),
                                ("y".into(), off.1.to_string()),
                            ];
                        }
                        "ext" => {
                            c.attrs = vec![
                                ("cx".into(), ext.0.to_string()),
                                ("cy".into(), ext.1.to_string()),
                            ];
                        }
                        _ => {}
                    }
                }
            }
            return;
        }
        for c in &mut e.children {
            if let bo_model::slide::XmlNode::Element(c) = c {
                visit(c, off, ext);
            }
        }
    }
    // Only the choice holds the ink frame; the fallback keeps its own markup.
    for c in &mut el.children {
        if let bo_model::slide::XmlNode::Element(c) = c {
            if c.local() == "Choice" {
                visit(c, off, ext);
            }
        }
    }
    if el.local() == "contentPart" {
        visit(el, off, ext);
    }
}

/// Markup of an ink object: `mc:AlternateContent` with a p14 choice holding
/// the `p:contentPart` (id, name, relationship, frame in EMU) and a fallback
/// (`fallback`, a shape-tree element such as a freeform `p:sp`).
pub fn ink_node_xml(
    id: u32,
    name: &str,
    rid: &str,
    off: (i64, i64),
    ext: (i64, i64),
    fallback: &str,
) -> String {
    format!(
        concat!(
            // ECMA-376 Part 3, §10.2.1 AlternateContent / §10.2.2 Choice@Requires / §10.2.3 Fallback.
            r#"<mc:AlternateContent xmlns:mc="{mc}"><mc:Choice xmlns:p14="{p14}" Requires="p14">"#,
            // ECMA-376 Part 1, §19.3.1.14 contentPart (@r:id); [MS-PPTX] p14:bwMode,
            // p14:nvContentPartPr (cNvPr, cNvContentPartPr, nvPr) and p14:xfrm.
            r#"<p:contentPart p14:bwMode="auto" r:id="{rid}">"#,
            r#"<p14:nvContentPartPr><p14:cNvPr id="{id}" name="{name}"/><p14:cNvContentPartPr/><p14:nvPr/></p14:nvContentPartPr>"#,
            r#"<p14:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p14:xfrm>"#,
            r#"</p:contentPart></mc:Choice><mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent>"#
        ),
        mc = MC_NS,
        p14 = P14_NS,
        rid = esc(rid),
        id = id,
        name = esc(name),
        x = off.0,
        y = off.1,
        cx = ext.0.max(1),
        cy = ext.1.max(1),
        fallback = fallback,
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_finds_the_content_part() {
        let xml = ink_node_xml(
            7,
            "Ink 7",
            "rId4",
            (12_700, 25_400),
            (127_000, 0),
            "<p:sp/>",
        );
        let wrapped = format!(r#"<p:spTree xmlns:p="p" xmlns:a="a" xmlns:r="r">{xml}</p:spTree>"#);
        let tree = bo_drawingml::dom::parse(wrapped.as_bytes(), "t").unwrap();
        let mut el = tree.elements().next().unwrap().clone();
        let cp = content_part(&el).unwrap();
        assert_eq!(content_part_rid(cp), Some("rId4"));
        assert_eq!(content_part_nv(cp), Some((7, "Ink 7".into())));
        let x = content_part_xfrm(cp).unwrap();
        assert_eq!(x.off, Some((12_700, 25_400)));
        // A zero extent is written as 1 EMU.
        assert_eq!(x.ext, Some((127_000, 1)));
        set_content_part_frame(&mut el, (1, 2), (3, 4));
        let x = content_part_xfrm(content_part(&el).unwrap()).unwrap();
        assert_eq!((x.off, x.ext), (Some((1, 2)), Some((3, 4))));
        let fb = el.elements().find(|c| c.local() == "Fallback").unwrap();
        assert_eq!(fb.elements().next().unwrap().name, "p:sp");
    }
}

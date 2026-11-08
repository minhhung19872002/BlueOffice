//! Office Math Markup (ECMA-376 Part 1, §22.1) → [`Node`] tree, for editing an equation of a document and
//! showing it in the linear format. Elements without a [`Node`] shape (`m:box`, `m:groupChr`, `m:sPre`,
//! `m:borderBox`, `m:phant`) keep their content; property elements are read for the values the tree holds.

use crate::{Node, Style};
use bo_model::slide::{XmlElement, XmlNode};

/// The equation of an `m:oMath` / `m:oMathPara` element (`None` when the XML does not parse).
#[must_use]
pub fn from_omml(xml: &str) -> Option<Vec<Node>> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    Some(crate::unicode_math::merge(children(&el)))
}

fn kid<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|e| e.local() == local)
}

fn val(el: &XmlElement) -> Option<&str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some("val"))
        .map(|(_, v)| v.as_str())
}

fn prop<'a>(el: &'a XmlElement, pr: &str, name: &str) -> Option<&'a str> {
    kid(kid(el, pr)?, name).and_then(val)
}

fn on(el: &XmlElement, pr: &str, name: &str) -> bool {
    kid(el, pr)
        .and_then(|p| kid(p, name))
        .is_some_and(|e| val(e).is_none_or(|v| matches!(v, "1" | "on" | "true")))
}

fn text(el: &XmlElement) -> String {
    el.children
        .iter()
        .map(|n| match n {
            XmlNode::Text(t) => t.clone(),
            XmlNode::Element(e) => text(e),
        })
        .collect()
}

fn children(el: &XmlElement) -> Vec<Node> {
    el.elements().flat_map(node).collect()
}

fn arg(el: &XmlElement, name: &str) -> Vec<Node> {
    kid(el, name).map(children).unwrap_or_default()
}

fn first_char(s: Option<&str>, default: char) -> Option<char> {
    match s {
        Some(v) => v.chars().next(),
        None => Some(default),
    }
}

fn node(el: &XmlElement) -> Vec<Node> {
    let n = match el.local() {
        l if l.ends_with("Pr") => return Vec::new(),
        // §22.1.2.98 r: m:t text, sty p = upright, nor = ordinary text.
        "r" => {
            let t: String = el
                .elements()
                .filter(|e| e.local() == "t")
                .map(text)
                .collect();
            let rpr = kid(el, "rPr");
            let style = if rpr.and_then(|p| kid(p, "nor")).is_some() {
                Style::Normal
            } else if rpr
                .and_then(|p| kid(p, "sty"))
                .and_then(val)
                .is_some_and(|v| v == "p")
            {
                Style::Upright
            } else {
                Style::Math
            };
            Node::Run(t, style)
        }
        "f" => Node::Frac {
            num: arg(el, "num"),
            den: arg(el, "den"),
            bar: prop(el, "fPr", "type") != Some("noBar"),
        },
        "sSup" | "sSub" | "sSubSup" => Node::Scripts {
            base: arg(el, "e"),
            sub: kid(el, "sub").map(children),
            sup: kid(el, "sup").map(children),
        },
        "rad" => Node::Rad {
            deg: if on(el, "radPr", "degHide") {
                None
            } else {
                kid(el, "deg").map(children).filter(|d| !d.is_empty())
            },
            body: arg(el, "e"),
        },
        "nary" => Node::Nary {
            chr: first_char(prop(el, "naryPr", "chr"), '∫').unwrap_or('∫'),
            sub: (!on(el, "naryPr", "subHide"))
                .then(|| arg(el, "sub"))
                .filter(|v| !v.is_empty()),
            sup: (!on(el, "naryPr", "supHide"))
                .then(|| arg(el, "sup"))
                .filter(|v| !v.is_empty()),
            body: arg(el, "e"),
        },
        "d" => Node::Delim {
            open: first_char(prop(el, "dPr", "begChr"), '('),
            close: first_char(prop(el, "dPr", "endChr"), ')'),
            parts: el
                .elements()
                .filter(|e| e.local() == "e")
                .map(children)
                .collect(),
        },
        "func" => Node::Func {
            name: arg(el, "fName"),
            arg: arg(el, "e"),
        },
        "limLow" | "limUpp" => Node::Limit {
            base: arg(el, "e"),
            lim: arg(el, "lim"),
            upper: el.local() == "limUpp",
        },
        "m" => Node::Matrix(
            el.elements()
                .filter(|r| r.local() == "mr")
                .map(|r| {
                    r.elements()
                        .filter(|e| e.local() == "e")
                        .map(children)
                        .collect()
                })
                .collect(),
        ),
        "eqArr" => Node::EqArray(
            el.elements()
                .filter(|e| e.local() == "e")
                .map(children)
                .collect(),
        ),
        "acc" => Node::Accent {
            chr: first_char(prop(el, "accPr", "chr"), '\u{302}').unwrap_or('\u{302}'),
            base: arg(el, "e"),
        },
        "bar" => Node::Bar {
            top: prop(el, "barPr", "pos") == Some("top"),
            base: arg(el, "e"),
        },
        // Containers and the shapes without a node: their content.
        _ => return children(el),
    };
    vec![n]
}

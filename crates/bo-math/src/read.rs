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

/// Whether two Office Math zones are the same markup (CORE-MATH-010, ADR-0015 addendum): equal after
/// a canonical form that only removes what carries no information — attribute order, `xmlns`
/// declarations, whitespace between elements, `xml:space` on §22.1.2.116 t, empty property
/// elements (`m:…Pr` without content) and the split of text into adjacent §22.1.2.98 r runs with
/// the same properties. Anything else (an element, attribute or property the other lacks) differs.
/// `false` when either does not parse.
#[must_use]
pub fn same_omml(a: &str, b: &str) -> bool {
    let parse = |s: &str| bo_drawingml::dom::parse(s.as_bytes(), "/word/document.xml").ok();
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => {
            bo_drawingml::dom::canonical(&prep(&x)) == bo_drawingml::dom::canonical(&prep(&y))
        }
        _ => false,
    }
}

/// Text of a `t` element.
fn t_text(el: &XmlElement) -> Option<String> {
    let mut s = String::new();
    for c in &el.children {
        match c {
            XmlNode::Text(t) => s.push_str(t),
            XmlNode::Element(_) => return None,
        }
    }
    Some(s)
}

/// A run as (properties, text) when it is properties followed by plain `t` elements.
fn run_parts(el: &XmlElement) -> Option<(Vec<XmlElement>, String)> {
    if el.local() != "r" || !el.attrs.is_empty() {
        return None;
    }
    let mut props = Vec::new();
    let mut text: Option<String> = None;
    for c in &el.children {
        match c {
            XmlNode::Text(t) if t.trim().is_empty() => {}
            XmlNode::Element(e) if e.local() == "t" && e.attrs.is_empty() => {
                text.get_or_insert_with(String::new).push_str(&t_text(e)?);
            }
            // Properties come before the text.
            XmlNode::Element(e) if text.is_none() && e.local() != "t" => props.push(e.clone()),
            XmlNode::Text(_) | XmlNode::Element(_) => return None,
        }
    }
    Some((props, text?))
}

fn t_name(run: &XmlElement) -> String {
    run.elements()
        .find(|x| x.local() == "t")
        .map_or_else(String::new, |x| x.name.clone())
}

fn make_run(name: &str, t_name: &str, props: Vec<XmlElement>, text: String) -> XmlElement {
    let mut children: Vec<XmlNode> = props.into_iter().map(XmlNode::Element).collect();
    children.push(XmlNode::Element(XmlElement {
        name: t_name.to_owned(),
        attrs: Vec::new(),
        children: vec![XmlNode::Text(text)],
    }));
    XmlElement {
        name: name.to_owned(),
        attrs: Vec::new(),
        children,
    }
}

fn prep(el: &XmlElement) -> XmlElement {
    let mut attrs = el.attrs.clone();
    if el.local() == "t" {
        attrs.retain(|(k, _)| k != "xml:space");
    }
    let mut children: Vec<XmlNode> = Vec::new();
    for c in &el.children {
        let XmlNode::Element(e) = c else {
            children.push(c.clone());
            continue;
        };
        let e = prep(e);
        let empty = e.attrs.is_empty()
            && e.children.iter().all(|n| match n {
                XmlNode::Text(t) => t.trim().is_empty(),
                XmlNode::Element(_) => false,
            });
        if empty && e.local().ends_with("Pr") {
            continue;
        }
        // Adjacent runs with the same properties are one run; a run's text is one `t`.
        if let Some((props, text)) = run_parts(&e) {
            let tn = t_name(&e);
            let key = |v: &[XmlElement]| {
                v.iter()
                    .map(bo_drawingml::dom::canonical)
                    .collect::<Vec<_>>()
            };
            let prev = children.iter().rposition(|n| match n {
                XmlNode::Text(t) => !t.trim().is_empty(),
                XmlNode::Element(_) => true,
            });
            let merged = prev.and_then(|i| match &children[i] {
                XmlNode::Element(p) if p.name == e.name && t_name(p) == tn => {
                    let (pp, pt) = run_parts(p)?;
                    (key(&pp) == key(&props)).then(|| (i, make_run(&e.name, &tn, pp, pt + &text)))
                }
                _ => None,
            });
            match merged {
                Some((i, r)) => children[i] = XmlNode::Element(r),
                None => children.push(XmlNode::Element(make_run(&e.name, &tn, props, text))),
            }
            continue;
        }
        children.push(XmlNode::Element(e));
    }
    XmlElement {
        name: el.name.clone(),
        attrs,
        children,
    }
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

#[cfg(test)]
mod tests {
    use super::same_omml;

    const NS: &str = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

    // source: XML 1.0 (attribute order and whitespace between elements carry no information);
    // ECMA-376 Part 1, §22.1.2.98 r / §22.1.2.116 t.
    #[test]
    fn equal_after_canonical_form_only() {
        let a = format!("<m:oMath {NS}><m:r><m:t>ab</m:t></m:r></m:oMath>");
        // Whitespace, split runs, xml:space, an empty property element.
        let b = format!(
            "<m:oMath {NS}>\n <m:r><m:rPr/><m:t>a</m:t></m:r>\n <m:r><m:t xml:space=\"preserve\">b</m:t></m:r></m:oMath>"
        );
        assert!(same_omml(&a, &b));
        assert!(same_omml(&b, &a));
        // Different text, an element or a property the other lacks: different.
        let c = format!("<m:oMath {NS}><m:r><m:t>a b</m:t></m:r></m:oMath>");
        assert!(!same_omml(&a, &c));
        let d =
            format!("<m:oMath {NS}><m:box><m:e><m:r><m:t>ab</m:t></m:r></m:e></m:box></m:oMath>");
        assert!(!same_omml(&a, &d));
        let e = format!(
            r#"<m:oMath {NS}><m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"#
        );
        assert!(!same_omml(&a, &e));
        // Runs of the same style merge, of different styles do not.
        let f = format!(
            r#"<m:oMath {NS}><m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>a</m:t></m:r><m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>b</m:t></m:r></m:oMath>"#
        );
        let g = format!(
            r#"<m:oMath {NS}><m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>ab</m:t></m:r></m:oMath>"#
        );
        assert!(same_omml(&f, &g));
        assert!(!same_omml(&e, &g));
        // Unparsable markup is never the same.
        assert!(!same_omml("<m:oMath", &a));
    }
}

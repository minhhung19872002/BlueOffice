//! Office Math Markup (ECMA-376 Part 1, §22.1) → [`Node`] tree, for editing an equation of a document and
//! showing it in the linear format. Property elements are read for the values the tree holds and kept
//! verbatim ([`crate::Pr`], [`crate::RunPr`]); other containers give their content.

use crate::omml::{node_managed, run_managed, sigs, write_el};
use crate::{Node, Pr, RunPr, Style};
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

/// `el` without whitespace-only text (indentation between elements), for keeping it verbatim.
fn clean(el: &XmlElement) -> XmlElement {
    XmlElement {
        name: el.name.clone(),
        attrs: el.attrs.clone(),
        children: el
            .children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Element(e) => Some(XmlNode::Element(clean(e))),
                XmlNode::Text(t) if t.trim().is_empty() && el.local() != "t" => None,
                XmlNode::Text(t) => Some(XmlNode::Text(t.clone())),
            })
            .collect(),
    }
}

/// The children of the property element `name` of `el`, verbatim (CORE-MATH-010).
fn kids_of(el: &XmlElement, name: &str) -> Vec<XmlElement> {
    kid(el, name)
        .map(|p| p.elements().map(clean).collect())
        .unwrap_or_default()
}

fn pr_of(el: &XmlElement, name: &str) -> Pr {
    Pr {
        kids: kids_of(el, name),
        read: None,
    }
}

/// The prefix of a qualified name (`m` of `m:r`).
fn prefix(name: &str) -> &str {
    name.rsplit_once(':').map_or("", |(p, _)| p)
}

/// A run (§22.1.2.98 r): its `m:t` text, its math run properties (`m:rPr`, the prefix of the run:
/// sty p = upright, nor = ordinary text) and its WordprocessingML run properties (`w:rPr`).
fn run(el: &XmlElement) -> Node {
    let t: String = el
        .elements()
        .filter(|e| e.local() == "t")
        .map(text)
        .collect();
    let mine = prefix(&el.name);
    let rprs = || el.elements().filter(|e| e.local() == "rPr");
    let mrpr = rprs().find(|e| prefix(&e.name) == mine);
    let wrpr = rprs().find(|e| prefix(&e.name) != mine);
    let style = if mrpr.and_then(|p| kid(p, "nor")).is_some() {
        Style::Normal
    } else if mrpr
        .and_then(|p| kid(p, "sty"))
        .and_then(val)
        .is_some_and(|v| v == "p")
    {
        Style::Upright
    } else {
        Style::Math
    };
    let math = Pr {
        kids: mrpr
            .map(|p| p.elements().map(clean).collect())
            .unwrap_or_default(),
        read: Some(sigs(&run_managed(style))),
    };
    Node::Run(
        t,
        style,
        RunPr {
            math,
            text: wrpr.map(clean),
        },
    )
}

fn node(el: &XmlElement) -> Vec<Node> {
    let mut n = match el.local() {
        l if l.ends_with("Pr") => return Vec::new(),
        "r" => return vec![run(el)],
        "f" => Node::Frac {
            num: arg(el, "num"),
            den: arg(el, "den"),
            bar: prop(el, "fPr", "type") != Some("noBar"),
            pr: pr_of(el, "fPr"),
        },
        "sSup" | "sSub" | "sSubSup" => Node::Scripts {
            base: arg(el, "e"),
            sub: kid(el, "sub").map(children),
            sup: kid(el, "sup").map(children),
            pr: pr_of(el, &format!("{}Pr", el.local())),
        },
        "rad" => Node::Rad {
            deg: if on(el, "radPr", "degHide") {
                None
            } else {
                kid(el, "deg").map(children).filter(|d| !d.is_empty())
            },
            body: arg(el, "e"),
            pr: pr_of(el, "radPr"),
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
            pr: pr_of(el, "naryPr"),
        },
        "d" => Node::Delim {
            open: first_char(prop(el, "dPr", "begChr"), '('),
            close: first_char(prop(el, "dPr", "endChr"), ')'),
            parts: el
                .elements()
                .filter(|e| e.local() == "e")
                .map(children)
                .collect(),
            pr: pr_of(el, "dPr"),
        },
        "func" => Node::Func {
            name: arg(el, "fName"),
            arg: arg(el, "e"),
            pr: pr_of(el, "funcPr"),
        },
        "limLow" | "limUpp" => Node::Limit {
            base: arg(el, "e"),
            lim: arg(el, "lim"),
            upper: el.local() == "limUpp",
            pr: pr_of(el, &format!("{}Pr", el.local())),
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
            pr_of(el, "mPr"),
        ),
        "eqArr" => Node::EqArray(
            el.elements()
                .filter(|e| e.local() == "e")
                .map(children)
                .collect(),
            pr_of(el, "eqArrPr"),
        ),
        "acc" => Node::Accent {
            chr: first_char(prop(el, "accPr", "chr"), '\u{302}').unwrap_or('\u{302}'),
            base: arg(el, "e"),
            pr: pr_of(el, "accPr"),
        },
        "bar" => Node::Bar {
            top: prop(el, "barPr", "pos") == Some("top"),
            base: arg(el, "e"),
            pr: pr_of(el, "barPr"),
        },
        // ECMA-376 Part 1, §22.1.2 groupChr, box, borderBox, phant: properties and a base.
        "groupChr" => Node::GroupChr {
            base: arg(el, "e"),
            pr: pr_of(el, "groupChrPr"),
        },
        "box" => Node::Boxed {
            base: arg(el, "e"),
            pr: pr_of(el, "boxPr"),
        },
        "borderBox" => Node::BorderBox {
            base: arg(el, "e"),
            pr: pr_of(el, "borderBoxPr"),
        },
        "phant" => Node::Phantom {
            base: arg(el, "e"),
            pr: pr_of(el, "phantPr"),
        },
        // §22.1.2 sPre: scripts before the base.
        "sPre" => Node::PreScripts {
            sub: arg(el, "sub"),
            sup: arg(el, "sup"),
            base: arg(el, "e"),
            pr: pr_of(el, "sPrePr"),
        },
        // Containers: their content.
        _ => return children(el),
    };
    let read = sigs(&node_managed(&n));
    if let Some(pr) = n.pr_mut() {
        pr.read = Some(read);
    }
    vec![n]
}

/// The display properties of an `m:oMathPara` zone (`m:oMathParaPr`, ECMA-376 Part 1, §22.1.2.79:
/// justification), as XML, to write back around the edited equation; `None` for an inline zone or
/// none.
#[must_use]
pub fn para_props(xml: &str) -> Option<String> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    if el.local() != "oMathPara" {
        return None;
    }
    let p = kid(&el, "oMathParaPr")?;
    let mut s = String::new();
    write_el(&mut s, &clean(p));
    Some(s)
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

//! Writes a [`Node`] tree as Office Math Markup (ECMA-376 Part 1, §22.1): an `m:oMath` element with the
//! `m:` prefix (bound by the containing part to `http://schemas.openxmlformats.org/officeDocument/2006/math`).
//!
//! Property elements read from a file ([`Pr`], [`RunPr`]) are written back verbatim; only the
//! properties whose modelled value changed while editing are set (CORE-MATH-010, ADR-0015 addendum C).

use crate::{Node, Pr, RunPr, Style};
use bo_model::slide::{XmlElement, XmlNode};
use std::fmt::Write;

/// The OMML namespace (ECMA-376 Part 1, §22.1).
pub const NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

/// `<m:oMath>…</m:oMath>` for `nodes`.
#[must_use]
pub fn to_omml(nodes: &[Node]) -> String {
    let mut s = String::from("<m:oMath>");
    write_all(&mut s, nodes);
    s.push_str("</m:oMath>");
    s
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// An element as XML (attributes in their order, an element without children self-closed).
pub(crate) fn write_el(s: &mut String, el: &XmlElement) {
    s.push('<');
    s.push_str(&el.name);
    for (k, v) in &el.attrs {
        let _ = write!(s, r#" {k}="{}""#, esc(v));
    }
    if el.children.is_empty() {
        s.push_str("/>");
        return;
    }
    s.push('>');
    for c in &el.children {
        match c {
            XmlNode::Element(e) => write_el(s, e),
            XmlNode::Text(t) => s.push_str(&esc(t)),
        }
    }
    let _ = write!(s, "</{}>", el.name);
}

fn write_all(s: &mut String, nodes: &[Node]) {
    for n in nodes {
        write(s, n);
    }
}

fn wrap(s: &mut String, tag: &str, nodes: &[Node]) {
    let _ = write!(s, "<m:{tag}>");
    write_all(s, nodes);
    let _ = write!(s, "</m:{tag}>");
}

/// What a modelled property element should be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Want {
    /// Not written (the default).
    Absent,
    /// `<m:key m:val="…"/>`.
    Val(String),
    /// `<m:key/>` (an on/off property that is on).
    Flag,
}

/// A modelled property of a property element: its local name, the element it should be, and the
/// signature of the value it comes from (compared with [`Pr::read`]).
pub(crate) struct Managed {
    pub key: &'static str,
    pub want: Want,
    pub sig: String,
}

fn managed(key: &'static str, want: Want, sig: impl Into<String>) -> Managed {
    Managed {
        key,
        want,
        sig: sig.into(),
    }
}

/// Children order of the property elements (the schema's sequences, ECMA-376 Part 1, §22.1.2 and
/// the math schema): where a property that was not in the file is inserted.
fn order(tag: &str) -> &'static [&'static str] {
    match tag {
        "rPr" => &["lit", "nor", "scr", "sty", "brk", "aln"],
        "fPr" => &["type", "ctrlPr"],
        "radPr" => &["degHide", "ctrlPr"],
        "naryPr" => &["chr", "limLoc", "grow", "subHide", "supHide", "ctrlPr"],
        "dPr" => &["begChr", "sepChr", "endChr", "grow", "shp", "ctrlPr"],
        "accPr" => &["chr", "ctrlPr"],
        "barPr" => &["pos", "ctrlPr"],
        _ => &["ctrlPr"],
    }
}

fn is_integral(chr: char) -> bool {
    matches!(chr, '∫' | '∬' | '∭' | '∮' | '∯' | '∰' | '⨌')
}

fn style_sig(style: Style) -> &'static str {
    match style {
        Style::Math => "math",
        Style::Upright => "upright",
        Style::Normal => "normal",
    }
}

/// The modelled properties of a run's `m:rPr`: `m:sty` (§22.1.2.90) and `m:nor` (§22.1.2.73).
pub(crate) fn run_managed(style: Style) -> Vec<Managed> {
    let sig = style_sig(style);
    vec![
        managed(
            "sty",
            if style == Style::Upright {
                Want::Val("p".into())
            } else {
                Want::Absent
            },
            sig,
        ),
        managed(
            "nor",
            if style == Style::Normal {
                Want::Flag
            } else {
                Want::Absent
            },
            sig,
        ),
    ]
}

fn on_off(on: bool) -> Want {
    if on {
        Want::Val("1".into())
    } else {
        Want::Absent
    }
}

fn opt_char(c: Option<char>) -> String {
    c.map(String::from).unwrap_or_default()
}

/// The modelled properties of a structure, in the order of [`Pr::read`].
pub(crate) fn node_managed(n: &Node) -> Vec<Managed> {
    match n {
        // §22.1.2.36 f, §22.1.2.117 type.
        Node::Frac { bar, .. } => vec![managed(
            "type",
            if *bar {
                Want::Absent
            } else {
                Want::Val("noBar".into())
            },
            bar.to_string(),
        )],
        // §22.1.2.88 rad, degHide.
        Node::Rad { deg, .. } => vec![managed(
            "degHide",
            on_off(deg.is_none()),
            deg.is_some().to_string(),
        )],
        // §22.1.2.70 nary, §22.1.2.72 naryPr (chr, limLoc, subHide, supHide).
        Node::Nary { chr, sub, sup, .. } => vec![
            managed("chr", Want::Val(chr.to_string()), chr.to_string()),
            managed(
                "limLoc",
                Want::Val(
                    if is_integral(*chr) {
                        "subSup"
                    } else {
                        "undOvr"
                    }
                    .into(),
                ),
                chr.to_string(),
            ),
            managed("subHide", on_off(sub.is_none()), sub.is_some().to_string()),
            managed("supHide", on_off(sup.is_none()), sup.is_some().to_string()),
        ],
        // §22.1.2.24 d, dPr (begChr, endChr; an empty value = no delimiter).
        Node::Delim { open, close, .. } => {
            let sig = format!("{open:?}{close:?}");
            let default = *open == Some('(') && *close == Some(')');
            let want = |c: Option<char>| {
                if default {
                    Want::Absent
                } else {
                    Want::Val(opt_char(c))
                }
            };
            vec![
                managed("begChr", want(*open), sig.clone()),
                managed("endChr", want(*close), sig),
            ]
        }
        // §22.1.2.1 acc, chr.
        Node::Accent { chr, .. } => {
            vec![managed("chr", Want::Val(chr.to_string()), chr.to_string())]
        }
        // §22.1.2.7 bar, pos.
        Node::Bar { top, .. } => vec![managed(
            "pos",
            Want::Val(if *top { "top" } else { "bot" }.into()),
            top.to_string(),
        )],
        _ => Vec::new(),
    }
}

/// The signatures of the modelled properties (what [`Pr::read`] keeps).
pub(crate) fn sigs(m: &[Managed]) -> Vec<String> {
    m.iter().map(|x| x.sig.clone()).collect()
}

/// The children of a property element: verbatim, but for the modelled properties whose value
/// changed since they were read (all of them for properties made here).
fn patched(pr: &Pr, tag: &str, managed: &[Managed]) -> Vec<XmlElement> {
    let mut kids = pr.kids.clone();
    let order = order(tag);
    let rank = |local: &str| order.iter().position(|o| *o == local).unwrap_or(usize::MAX);
    for (i, m) in managed.iter().enumerate() {
        if pr.read.as_ref().and_then(|r| r.get(i)) == Some(&m.sig) {
            continue;
        }
        let at = kids.iter().position(|k| k.local() == m.key);
        let el = |attrs: Vec<(String, String)>| XmlElement {
            name: format!("m:{}", m.key),
            attrs,
            children: Vec::new(),
        };
        let new = match &m.want {
            Want::Absent => {
                if let Some(i) = at {
                    kids.remove(i);
                }
                continue;
            }
            Want::Val(v) => el(vec![("m:val".into(), v.clone())]),
            Want::Flag => el(Vec::new()),
        };
        if let Some(i) = at {
            kids[i] = new;
        } else {
            let r = rank(m.key);
            let pos = kids
                .iter()
                .position(|k| rank(k.local()) > r)
                .unwrap_or(kids.len());
            kids.insert(pos, new);
        }
    }
    kids
}

/// `<m:{tag}>…</m:{tag}>` for the property element of a structure (nothing when it has no children).
fn write_pr(s: &mut String, tag: &str, pr: &Pr, managed: &[Managed]) {
    let kids = patched(pr, tag, managed);
    if kids.is_empty() {
        return;
    }
    let _ = write!(s, "<m:{tag}>");
    for k in &kids {
        write_el(s, k);
    }
    let _ = write!(s, "</m:{tag}>");
}

fn write_run(s: &mut String, t: &str, style: Style, pr: &RunPr) {
    s.push_str("<m:r>");
    write_pr(s, "rPr", &pr.math, &run_managed(style));
    // ECMA-376 Part 1, §17.3.2.28 rPr (WordprocessingML run properties of the math run), verbatim.
    if let Some(w) = &pr.text {
        write_el(s, w);
    }
    if t.starts_with(' ') || t.ends_with(' ') {
        let _ = write!(s, r#"<m:t xml:space="preserve">{}</m:t>"#, esc(t));
    } else {
        let _ = write!(s, "<m:t>{}</m:t>", esc(t));
    }
    s.push_str("</m:r>");
}

fn write(s: &mut String, n: &Node) {
    let m = node_managed(n);
    match n {
        // §22.1.2.98 r, §22.1.2.116 t (xml:space keeps spaces of literal text).
        Node::Run(t, style, pr) => write_run(s, t, *style, pr),
        // §22.1.2.36 f.
        Node::Frac { num, den, pr, .. } => {
            s.push_str("<m:f>");
            write_pr(s, "fPr", pr, &m);
            wrap(s, "num", num);
            wrap(s, "den", den);
            s.push_str("</m:f>");
        }
        Node::Scripts { base, sub, sup, pr } => {
            let tag = match (sub, sup) {
                (Some(_), Some(_)) => "sSubSup",
                (Some(_), None) => "sSub",
                _ => "sSup",
            };
            let _ = write!(s, "<m:{tag}>");
            write_pr(s, &format!("{tag}Pr"), pr, &m);
            wrap(s, "e", base);
            if let Some(b) = sub {
                wrap(s, "sub", b);
            }
            if let Some(p) = sup {
                wrap(s, "sup", p);
            }
            let _ = write!(s, "</m:{tag}>");
        }
        // §22.1.2.88 rad: properties, degree (empty when hidden), radicand.
        Node::Rad { deg, body, pr } => {
            s.push_str("<m:rad>");
            write_pr(s, "radPr", pr, &m);
            match deg {
                Some(d) => wrap(s, "deg", d),
                None => s.push_str("<m:deg/>"),
            }
            wrap(s, "e", body);
            s.push_str("</m:rad>");
        }
        // §22.1.2.70 nary.
        Node::Nary {
            sub, sup, body, pr, ..
        } => {
            s.push_str("<m:nary>");
            write_pr(s, "naryPr", pr, &m);
            wrap(s, "sub", sub.as_deref().unwrap_or_default());
            wrap(s, "sup", sup.as_deref().unwrap_or_default());
            wrap(s, "e", body);
            s.push_str("</m:nary>");
        }
        // §22.1.2.24 d.
        Node::Delim { parts, pr, .. } => {
            s.push_str("<m:d>");
            write_pr(s, "dPr", pr, &m);
            for p in parts {
                wrap(s, "e", p);
            }
            s.push_str("</m:d>");
        }
        Node::Func { name, arg, pr } => {
            s.push_str("<m:func>");
            write_pr(s, "funcPr", pr, &m);
            wrap(s, "fName", name);
            wrap(s, "e", arg);
            s.push_str("</m:func>");
        }
        Node::Limit {
            base,
            lim,
            upper,
            pr,
        } => {
            let tag = if *upper { "limUpp" } else { "limLow" };
            let _ = write!(s, "<m:{tag}>");
            write_pr(s, &format!("{tag}Pr"), pr, &m);
            wrap(s, "e", base);
            wrap(s, "lim", lim);
            let _ = write!(s, "</m:{tag}>");
        }
        Node::Matrix(rows, pr) => {
            s.push_str("<m:m>");
            write_pr(s, "mPr", pr, &m);
            for r in rows {
                s.push_str("<m:mr>");
                for c in r {
                    wrap(s, "e", c);
                }
                s.push_str("</m:mr>");
            }
            s.push_str("</m:m>");
        }
        Node::EqArray(rows, pr) => {
            s.push_str("<m:eqArr>");
            write_pr(s, "eqArrPr", pr, &m);
            for r in rows {
                wrap(s, "e", r);
            }
            s.push_str("</m:eqArr>");
        }
        // §22.1.2.1 acc.
        Node::Accent { base, pr, .. } => {
            s.push_str("<m:acc>");
            write_pr(s, "accPr", pr, &m);
            wrap(s, "e", base);
            s.push_str("</m:acc>");
        }
        // §22.1.2.7 bar.
        Node::Bar { base, pr, .. } => {
            s.push_str("<m:bar>");
            write_pr(s, "barPr", pr, &m);
            wrap(s, "e", base);
            s.push_str("</m:bar>");
        }
        // ECMA-376 Part 1, §22.1.2 groupChr, box, borderBox, phant: properties and the base.
        Node::GroupChr { base, pr } => one_base(s, "groupChr", pr, base),
        Node::Boxed { base, pr } => one_base(s, "box", pr, base),
        Node::BorderBox { base, pr } => one_base(s, "borderBox", pr, base),
        Node::Phantom { base, pr } => one_base(s, "phant", pr, base),
        // §22.1.2 sPre: properties, subscript, superscript, base.
        Node::PreScripts { sub, sup, base, pr } => {
            s.push_str("<m:sPre>");
            write_pr(s, "sPrePr", pr, &m);
            wrap(s, "sub", sub);
            wrap(s, "sup", sup);
            wrap(s, "e", base);
            s.push_str("</m:sPre>");
        }
    }
}

fn one_base(s: &mut String, tag: &str, pr: &Pr, base: &[Node]) {
    let _ = write!(s, "<m:{tag}>");
    write_pr(s, &format!("{tag}Pr"), pr, &[]);
    wrap(s, "e", base);
    let _ = write!(s, "</m:{tag}>");
}

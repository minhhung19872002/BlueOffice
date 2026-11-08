//! Writes a [`Node`] tree as Office Math Markup (ECMA-376 Part 1, §22.1): an `m:oMath` element with the
//! `m:` prefix (bound by the containing part to `http://schemas.openxmlformats.org/officeDocument/2006/math`).

use crate::{Node, Style};
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

fn val(s: &mut String, tag: &str, v: &str) {
    let _ = write!(s, r#"<m:{tag} m:val="{}"/>"#, esc(v));
}

fn write(s: &mut String, n: &Node) {
    match n {
        // §22.1.2.98 r, §22.1.2.116 t (xml:space keeps spaces of literal text).
        Node::Run(t, style) => {
            s.push_str("<m:r>");
            match style {
                Style::Math => {}
                Style::Upright => s.push_str(r#"<m:rPr><m:sty m:val="p"/></m:rPr>"#),
                Style::Normal => s.push_str("<m:rPr><m:nor/></m:rPr>"),
            }
            if t.starts_with(' ') || t.ends_with(' ') {
                let _ = write!(s, r#"<m:t xml:space="preserve">{}</m:t>"#, esc(t));
            } else {
                let _ = write!(s, "<m:t>{}</m:t>", esc(t));
            }
            s.push_str("</m:r>");
        }
        // §22.1.2.36 f, §22.1.2.117 type.
        Node::Frac { num, den, bar } => {
            s.push_str("<m:f>");
            if !bar {
                s.push_str("<m:fPr>");
                val(s, "type", "noBar");
                s.push_str("</m:fPr>");
            }
            wrap(s, "num", num);
            wrap(s, "den", den);
            s.push_str("</m:f>");
        }
        Node::Scripts { base, sub, sup } => {
            let tag = match (sub, sup) {
                (Some(_), Some(_)) => "sSubSup",
                (Some(_), None) => "sSub",
                _ => "sSup",
            };
            let _ = write!(s, "<m:{tag}>");
            wrap(s, "e", base);
            if let Some(b) = sub {
                wrap(s, "sub", b);
            }
            if let Some(p) = sup {
                wrap(s, "sup", p);
            }
            let _ = write!(s, "</m:{tag}>");
        }
        // §22.1.2.88 rad, §22.1.2.27 degHide.
        Node::Rad { deg, body } => {
            s.push_str("<m:rad>");
            if let Some(d) = deg {
                wrap(s, "deg", d);
            } else {
                s.push_str("<m:radPr>");
                val(s, "degHide", "1");
                s.push_str("</m:radPr><m:deg/>");
            }
            wrap(s, "e", body);
            s.push_str("</m:rad>");
        }
        // §22.1.2.70 nary, §22.1.2.72 naryPr (chr, limLoc, subHide, supHide).
        Node::Nary {
            chr,
            sub,
            sup,
            body,
        } => {
            s.push_str("<m:nary><m:naryPr>");
            val(s, "chr", &chr.to_string());
            let integral = matches!(chr, '∫' | '∬' | '∭' | '∮' | '∯' | '∰' | '⨌');
            val(s, "limLoc", if integral { "subSup" } else { "undOvr" });
            if sub.is_none() {
                val(s, "subHide", "1");
            }
            if sup.is_none() {
                val(s, "supHide", "1");
            }
            s.push_str("</m:naryPr>");
            wrap(s, "sub", sub.as_deref().unwrap_or_default());
            wrap(s, "sup", sup.as_deref().unwrap_or_default());
            wrap(s, "e", body);
            s.push_str("</m:nary>");
        }
        // §22.1.2.24 d, §22.1.2.26 dPr (begChr, endChr; an empty value = no delimiter).
        Node::Delim { open, close, parts } => {
            s.push_str("<m:d>");
            if *open != Some('(') || *close != Some(')') {
                s.push_str("<m:dPr>");
                val(s, "begChr", &open.map(String::from).unwrap_or_default());
                val(s, "endChr", &close.map(String::from).unwrap_or_default());
                s.push_str("</m:dPr>");
            }
            for p in parts {
                wrap(s, "e", p);
            }
            s.push_str("</m:d>");
        }
        Node::Func { name, arg } => {
            s.push_str("<m:func>");
            wrap(s, "fName", name);
            wrap(s, "e", arg);
            s.push_str("</m:func>");
        }
        Node::Limit { base, lim, upper } => {
            let tag = if *upper { "limUpp" } else { "limLow" };
            let _ = write!(s, "<m:{tag}>");
            wrap(s, "e", base);
            wrap(s, "lim", lim);
            let _ = write!(s, "</m:{tag}>");
        }
        Node::Matrix(rows) => {
            s.push_str("<m:m>");
            for r in rows {
                s.push_str("<m:mr>");
                for c in r {
                    wrap(s, "e", c);
                }
                s.push_str("</m:mr>");
            }
            s.push_str("</m:m>");
        }
        Node::EqArray(rows) => {
            s.push_str("<m:eqArr>");
            for r in rows {
                wrap(s, "e", r);
            }
            s.push_str("</m:eqArr>");
        }
        // §22.1.2.1 acc, §22.1.2.20 chr.
        Node::Accent { chr, base } => {
            s.push_str("<m:acc><m:accPr>");
            val(s, "chr", &chr.to_string());
            s.push_str("</m:accPr>");
            wrap(s, "e", base);
            s.push_str("</m:acc>");
        }
        // §22.1.2.7 bar, §22.1.2.84 pos.
        Node::Bar { top, base } => {
            s.push_str("<m:bar><m:barPr>");
            val(s, "pos", if *top { "top" } else { "bot" });
            s.push_str("</m:barPr>");
            wrap(s, "e", base);
            s.push_str("</m:bar>");
        }
    }
}

//! XML helpers shared by the SpreadsheetML reader and writer: attribute lists,
//! verbatim capture of unknown elements, `ST_Xstring` escaping and a canonical
//! form used to compare XML parts in tests.

use crate::{Result, XlsxError};
use bo_model::sheet::{RawAttr, RawXml};
use bo_opc::xml::{escape, unescape, XmlReader};
use quick_xml::events::{BytesStart, Event};
use std::fmt::Write as _;

/// All attributes of `e` as (qualified name, unescaped value), in document order.
pub(crate) fn attrs(e: &BytesStart<'_>) -> Vec<RawAttr> {
    e.attributes()
        .flatten()
        .map(|a| {
            let k: &str = a.key.as_ref();
            (k.to_owned(), unescape(&a.value))
        })
        .collect()
}

/// Remove the attribute whose *local* name is `name` and return its value.
pub(crate) fn take(attrs: &mut Vec<RawAttr>, name: &str) -> Option<String> {
    let i = attrs
        .iter()
        .position(|(k, _)| k.rsplit_once(':').map_or(k.as_str(), |(_, l)| l) == name)?;
    Some(attrs.remove(i).1)
}

/// Remove an attribute by exact qualified name.
pub(crate) fn take_q(attrs: &mut Vec<RawAttr>, qname: &str) -> Option<String> {
    let i = attrs.iter().position(|(k, _)| k == qname)?;
    Some(attrs.remove(i).1)
}

/// `xsd:boolean` (§22.9.2.1 / XML Schema): `1` or `true`.
pub(crate) fn pbool(v: &str) -> bool {
    v == "1" || v.eq_ignore_ascii_case("true")
}

pub(crate) fn take_bool(attrs: &mut Vec<RawAttr>, name: &str) -> Option<bool> {
    take(attrs, name).map(|v| pbool(&v))
}

pub(crate) fn take_u32(attrs: &mut Vec<RawAttr>, name: &str) -> Option<u32> {
    take(attrs, name).and_then(|v| v.trim().parse().ok())
}

pub(crate) fn take_f64(attrs: &mut Vec<RawAttr>, name: &str) -> Option<f64> {
    take(attrs, name)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// Local name of an element as an owned string.
pub(crate) fn lname(e: &BytesStart<'_>) -> String {
    let n = e.local_name();
    let s: &str = n.as_ref();
    s.to_owned()
}

/// Iterate over the direct children of the element whose `Start` was just read.
/// `f` receives each child start tag and whether it was empty; if `f` leaves a
/// non-empty child unconsumed it is skipped automatically.
pub(crate) fn for_children<'a>(
    r: &mut XmlReader<'a>,
    mut f: impl FnMut(&mut XmlReader<'a>, &BytesStart<'a>, bool) -> Result<()>,
) -> Result<()> {
    let end = r.depth() - 1;
    loop {
        match r.read()? {
            Event::Start(e) => {
                let d = r.depth();
                f(r, &e, false)?;
                if r.depth() == d {
                    r.skip_element()?;
                }
            }
            Event::Empty(e) => f(r, &e, true)?,
            Event::End(_) if r.depth() == end => return Ok(()),
            Event::Eof => return Err(XlsxError::Opc(r.err("unexpected end of document"))),
            _ => {}
        }
    }
}

/// Advance to the root element; returns it and whether it is empty.
pub(crate) fn root<'a>(r: &mut XmlReader<'a>) -> Result<(BytesStart<'a>, bool)> {
    loop {
        match r.read()? {
            Event::Start(e) => return Ok((e, false)),
            Event::Empty(e) => return Ok((e, true)),
            Event::Eof => return Err(XlsxError::Opc(r.err("no root element"))),
            _ => {}
        }
    }
}

/// Serialize the element whose start tag `e` was just read (and its whole
/// subtree) back to XML text, verbatim as far as the parser allows.
pub(crate) fn capture(r: &mut XmlReader<'_>, e: &BytesStart<'_>, empty: bool) -> Result<String> {
    let raw: &str = e;
    if empty {
        return Ok(format!("<{raw}/>"));
    }
    let mut s = format!("<{raw}>");
    let end = r.depth() - 1;
    loop {
        match r.read()? {
            Event::Start(x) => {
                let t: &str = &x;
                s.push('<');
                s.push_str(t);
                s.push('>');
            }
            Event::Empty(x) => {
                let t: &str = &x;
                s.push('<');
                s.push_str(t);
                s.push_str("/>");
            }
            Event::End(x) => {
                let t: &str = &x;
                s.push_str("</");
                s.push_str(t);
                s.push('>');
                if r.depth() == end {
                    return Ok(s);
                }
            }
            Event::Text(t) => s.push_str(&t),
            Event::GeneralRef(g) => {
                let t: &str = &g;
                s.push('&');
                s.push_str(t);
                s.push(';');
            }
            Event::CData(c) => {
                let t: &str = &c;
                s.push_str("<![CDATA[");
                s.push_str(t);
                s.push_str("]]>");
            }
            Event::Comment(c) => {
                let t: &str = &c;
                s.push_str("<!--");
                s.push_str(t);
                s.push_str("-->");
            }
            Event::PI(p) => {
                let t: &str = &p;
                s.push_str("<?");
                s.push_str(t);
                s.push_str("?>");
            }
            Event::Eof => return Err(XlsxError::Opc(r.err("unexpected end of document"))),
            Event::Decl(_) | Event::DocType(_) => {}
        }
    }
}

/// Text content of the current element (child elements are ignored).
pub(crate) fn read_text(r: &mut XmlReader<'_>) -> Result<String> {
    let mut s = String::new();
    read_text_into(r, &mut s)?;
    Ok(s)
}

/// [`read_text`] appending to `s` (callers reuse one buffer).
pub(crate) fn read_text_into(r: &mut XmlReader<'_>, s: &mut String) -> Result<()> {
    let end = r.depth() - 1;
    loop {
        match r.read()? {
            Event::Text(t) => {
                let t: &str = &t;
                if t.contains('&') {
                    s.push_str(&unescape(t));
                } else {
                    s.push_str(t);
                }
            }
            Event::GeneralRef(g) => {
                let t: &str = &g;
                s.push_str(&unescape(&format!("&{t};")));
            }
            Event::CData(c) => s.push_str(&c),
            Event::End(_) if r.depth() == end => return Ok(()),
            Event::Eof => return Err(XlsxError::Opc(r.err("unexpected end of document"))),
            _ => {}
        }
    }
}

/// Record an unknown child: elements named in `schema` anchor at their own slot,
/// others (extensions, `mc:AlternateContent`) at the slot of the previous sibling.
pub(crate) fn push_unknown(
    unknown: &mut Vec<RawXml>,
    prev_slot: &mut String,
    schema: &[&str],
    name: &str,
    xml: String,
) {
    if schema.contains(&name) {
        name.clone_into(prev_slot);
    }
    unknown.push(RawXml {
        after: prev_slot.clone(),
        xml,
    });
}

/// Emit unknown fragments anchored at `slot`.
pub(crate) fn emit_unknown(out: &mut String, unknown: &[RawXml], slot: &str) {
    for u in unknown.iter().filter(|u| u.after == slot) {
        out.push_str(&u.xml);
    }
}

/// Emit `known` elements in schema order, interleaving unknown fragments.
/// `known(slot, out)` writes the known element for a slot (if any).
pub(crate) fn emit_ordered(
    out: &mut String,
    schema: &[&str],
    unknown: &[RawXml],
    mut known: impl FnMut(&str, &mut String),
) {
    emit_unknown(out, unknown, "");
    for slot in schema {
        known(slot, out);
        emit_unknown(out, unknown, slot);
    }
    // Fragments anchored at a slot not in the schema (should not happen) are kept too.
    for u in unknown
        .iter()
        .filter(|u| !u.after.is_empty() && !schema.contains(&u.after.as_str()))
    {
        out.push_str(&u.xml);
    }
}

// --- writing --------------------------------------------------------------

/// Append ` name="value"` (escaped).
pub(crate) fn a(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, " {name}=\"{}\"", escape(value));
}

pub(crate) fn a_opt<T: ToString>(out: &mut String, name: &str, value: Option<&T>) {
    if let Some(v) = value {
        a(out, name, &v.to_string());
    }
}

pub(crate) fn a_optf(out: &mut String, name: &str, value: Option<f64>) {
    if let Some(v) = value {
        a(out, name, &fmt_f64(v));
    }
}

pub(crate) fn a_optb(out: &mut String, name: &str, value: Option<bool>) {
    if let Some(v) = value {
        a(out, name, if v { "1" } else { "0" });
    }
}

/// Write `name="1"` only when `value` is true.
pub(crate) fn a_true(out: &mut String, name: &str, value: bool) {
    if value {
        a(out, name, "1");
    }
}

pub(crate) fn a_raw(out: &mut String, attrs: &[RawAttr]) {
    for (k, v) in attrs {
        a(out, k, v);
    }
}

/// `xsd:double` text that round-trips exactly.
pub(crate) fn fmt_f64(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:?}");
        s.replace('e', "E")
    }
}

/// Encode text for an `ST_Xstring` element (§22.9.2.19): control characters as
/// `_xHHHH_`, and a literal `_xHHHH_` sequence protected by escaping its underscore.
pub(crate) fn xstring_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    for (i, c) in s.char_indices() {
        if c == '_' && looks_like_xescape(&bytes[i..]) {
            out.push_str("_x005F_");
        } else if (c as u32) < 0x20 && c != '\t' && c != '\n' {
            let _ = write!(out, "_x{:04X}_", c as u32);
        } else {
            out.push(c);
        }
    }
    escape(&out)
}

fn looks_like_xescape(b: &[u8]) -> bool {
    b.len() >= 7
        && b[0] == b'_'
        && (b[1] == b'x' || b[1] == b'X')
        && b[2..6].iter().all(u8::is_ascii_hexdigit)
        && b[6] == b'_'
}

/// Decode `_xHHHH_` escapes (§22.9.2.19 `ST_Xstring`).
pub(crate) fn xstring_decode(s: &str) -> String {
    if !s.contains("_x") && !s.contains("_X") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if looks_like_xescape(&b[i..]) {
            if let Some(c) = u32::from_str_radix(&s[i + 2..i + 6], 16)
                .ok()
                .and_then(char::from_u32)
            {
                out.push(c);
                i += 7;
                continue;
            }
        }
        // Advance one char.
        let ch = s[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

/// Does the text need `xml:space="preserve"`?
pub(crate) fn needs_preserve(s: &str) -> bool {
    s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) || s.contains('\n')
}

/// Canonical form of an XML document for comparisons: declaration, comments
/// and processing instructions dropped, attributes sorted by name, text
/// trimmed (whitespace-only text dropped), entities resolved and re-escaped,
/// empty elements expanded. Namespace prefixes are kept as written.
pub fn canonicalize_xml(data: &[u8]) -> std::result::Result<String, bo_opc::OpcError> {
    let mut r = XmlReader::new(data, "canonical");
    let mut out = String::new();
    let mut text = String::new();
    let flush = |out: &mut String, text: &mut String| {
        let t = text.trim();
        if !t.is_empty() {
            out.push_str(&escape(t));
        }
        text.clear();
    };
    let tag = |out: &mut String, e: &BytesStart<'_>| {
        let mut at: Vec<(String, String)> = e
            .attributes()
            .flatten()
            .map(|x| {
                let k: &str = x.key.as_ref();
                (k.to_owned(), unescape(&x.value))
            })
            .collect();
        at.sort();
        let n = e.name();
        let name: &str = n.as_ref();
        out.push('<');
        out.push_str(name);
        for (k, v) in at {
            let _ = write!(out, " {k}=\"{}\"", escape(&v));
        }
        out.push('>');
        name.to_owned()
    };
    loop {
        match r.read()? {
            Event::Start(e) => {
                flush(&mut out, &mut text);
                tag(&mut out, &e);
            }
            Event::Empty(e) => {
                flush(&mut out, &mut text);
                let n = tag(&mut out, &e);
                let _ = write!(out, "</{n}>");
            }
            Event::End(e) => {
                flush(&mut out, &mut text);
                let n: &str = &e;
                let _ = write!(out, "</{n}>");
            }
            Event::Text(t) => text.push_str(&unescape(&t)),
            Event::GeneralRef(g) => {
                let t: &str = &g;
                text.push_str(&unescape(&format!("&{t};")));
            }
            Event::CData(c) => text.push_str(&c),
            Event::Eof => break,
            _ => {}
        }
    }
    flush(&mut out, &mut text);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xstring_roundtrip() {
        for s in [
            "plain",
            "a\rb",
            "tab\tnl\n",
            "_x0041_ literal",
            "\u{1}\u{1f}",
            "Việt _x",
        ] {
            let enc = xstring_encode(s);
            assert_eq!(xstring_decode(&unescape(&enc)), s, "{enc}");
        }
        assert_eq!(xstring_decode("a_x000D_b"), "a\rb");
    }

    #[test]
    fn canonical_ignores_attr_order_and_whitespace() {
        let a = br#"<?xml version="1.0"?><x b="2" a="1">
            <y/> <z>t &amp; u</z></x>"#;
        let b = br#"<x a="1" b="2"><y></y><z>t &#38; u</z></x>"#;
        assert_eq!(canonicalize_xml(a).unwrap(), canonicalize_xml(b).unwrap());
        assert_ne!(
            canonicalize_xml(a).unwrap(),
            canonicalize_xml(br#"<x a="1" b="3"/>"#).unwrap()
        );
    }

    #[test]
    fn capture_is_verbatim() {
        let src = r#"<r><mc:AlternateContent xmlns:mc="m"><mc:Choice Requires="x"><a b="1&amp;2">t&lt;<![CDATA[c]]></a><e/></mc:Choice></mc:AlternateContent><n/></r>"#;
        let mut r = XmlReader::new(src.as_bytes(), "t");
        let (_, _) = root(&mut r).unwrap();
        let mut got = Vec::new();
        for_children(&mut r, |r, e, empty| {
            got.push(capture(r, e, empty)?);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            got[0],
            r#"<mc:AlternateContent xmlns:mc="m"><mc:Choice Requires="x"><a b="1&amp;2">t&lt;<![CDATA[c]]></a><e/></mc:Choice></mc:AlternateContent>"#
        );
        assert_eq!(got[1], "<n/>");
    }
}

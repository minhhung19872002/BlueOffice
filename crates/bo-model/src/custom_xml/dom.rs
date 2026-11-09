//! A small XML 1.0 tree for Custom XML Data Storage parts (ECMA-376 Part 1, §15.2.4):
//! elements, attributes, text, CDATA sections, comments and processing instructions.
//!
//! Text before the root element (XML declaration, comments, a document type declaration) and
//! after it is kept verbatim, so a part re-serialized after a value change differs from its
//! source only where it was changed. Only the predefined entities and character references are
//! expanded; a document type declaration is never interpreted (no entity expansion).

use std::fmt::Write as _;

/// Deepest element nesting accepted (protects the recursive walkers).
pub const MAX_DEPTH: usize = 256;

/// Node of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XNode {
    /// Child element.
    Element(XElement),
    /// Character data (entities expanded).
    Text(String),
    /// `<![CDATA[…]]>` section (its content).
    CData(String),
    /// `<!--…-->` comment (its content).
    Comment(String),
    /// `<?…?>` processing instruction (its content).
    Pi(String),
}

/// An element: qualified name, attributes in document order (`xmlns` declarations included),
/// children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XElement {
    /// Qualified name (`ns0:invoice`).
    pub name: String,
    /// Attributes (qualified names, values with entities expanded).
    pub attrs: Vec<(String, String)>,
    /// Child nodes.
    pub children: Vec<XNode>,
}

impl XElement {
    /// Prefix of the qualified name (`None` for an unprefixed name).
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        self.name.split_once(':').map(|(p, _)| p)
    }

    /// Local part of the qualified name.
    #[must_use]
    pub fn local(&self) -> &str {
        self.name.split_once(':').map_or(&self.name, |(_, l)| l)
    }

    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &XElement> {
        self.children.iter().filter_map(|c| match c {
            XNode::Element(e) => Some(e),
            _ => None,
        })
    }

    /// The element has child elements.
    #[must_use]
    pub fn has_elements(&self) -> bool {
        self.elements().next().is_some()
    }

    /// XPath 1.0 string-value (§5.2): the text of every descendant text node in document order.
    #[must_use]
    pub fn string_value(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }

    fn collect_text(&self, out: &mut String) {
        for c in &self.children {
            match c {
                XNode::Text(t) | XNode::CData(t) => out.push_str(t),
                XNode::Element(e) => e.collect_text(out),
                XNode::Comment(_) | XNode::Pi(_) => {}
            }
        }
    }

    /// Replace the text content (comments and processing instructions stay; text and CDATA
    /// go). Callers only do this to elements without child elements.
    pub fn set_text(&mut self, value: &str) {
        self.children
            .retain(|c| matches!(c, XNode::Comment(_) | XNode::Pi(_)));
        if !value.is_empty() {
            self.children.push(XNode::Text(value.to_owned()));
        }
    }

    /// Namespace declared by this element for `prefix` (`None`: the default namespace).
    #[must_use]
    pub fn declared_ns(&self, prefix: Option<&str>) -> Option<&str> {
        self.attrs
            .iter()
            .find_map(|(k, v)| match (prefix, k.as_str()) {
                (None, "xmlns") => Some(v.as_str()),
                (Some(p), k) if k.strip_prefix("xmlns:") == Some(p) => Some(v.as_str()),
                _ => None,
            })
    }
}

/// A parsed part: text before the root element, the root, text after it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XDocument {
    /// Verbatim text before the root (declaration, comments, doctype, whitespace).
    pub prolog: String,
    /// Document element.
    pub root: XElement,
    /// Verbatim text after the root.
    pub epilog: String,
}

/// Parse failure: a short reason and the byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlError {
    /// What is wrong.
    pub reason: &'static str,
    /// Byte offset in the input.
    pub at: usize,
}

impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.reason, self.at)
    }
}

impl std::error::Error for XmlError {}

type Res<T> = Result<T, XmlError>;

struct Parser<'a> {
    s: &'a str,
    pos: usize,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.' | '\u{B7}')
}

impl<'a> Parser<'a> {
    fn err<T>(&self, reason: &'static str) -> Res<T> {
        Err(XmlError {
            reason,
            at: self.pos,
        })
    }

    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.pos += r.len() - r.trim_start_matches([' ', '\t', '\r', '\n']).len();
    }

    /// Everything up to (excluding) `end`; the position moves past `end`.
    fn until(&mut self, end: &str, reason: &'static str) -> Res<&'a str> {
        match self.rest().find(end) {
            Some(i) => {
                let out = &self.s[self.pos..self.pos + i];
                self.pos += i + end.len();
                Ok(out)
            }
            None => self.err(reason),
        }
    }

    fn name(&mut self) -> Res<&'a str> {
        let r = self.rest();
        let n = r
            .char_indices()
            .find(|(_, c)| !is_name_char(*c))
            .map_or(r.len(), |(i, _)| i);
        let first_ok = r
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':');
        if n == 0 || !first_ok {
            return self.err("expected a name");
        }
        self.pos += n;
        Ok(&r[..n])
    }

    /// Prolog / epilog items: whitespace, comments, processing instructions, a doctype.
    fn misc(&mut self, doctype_ok: bool) -> Res<()> {
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("<?") {
                self.until("?>", "unterminated processing instruction")?;
            } else if r.starts_with("<!--") {
                self.pos += 4;
                self.until("-->", "unterminated comment")?;
            } else if doctype_ok && r.starts_with("<!DOCTYPE") {
                self.doctype()?;
            } else {
                return Ok(());
            }
        }
    }

    /// Skip a document type declaration (kept verbatim in the prolog, never interpreted).
    fn doctype(&mut self) -> Res<()> {
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        for (i, c) in self.rest().char_indices() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (None, '"' | '\'') => quote = Some(c),
                (None, '[') => depth += 1,
                (None, ']') => depth = depth.saturating_sub(1),
                (None, '>') if depth == 0 => {
                    self.pos += i + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        self.err("unterminated document type declaration")
    }

    fn element(&mut self, depth: usize) -> Res<XElement> {
        if depth > MAX_DEPTH {
            return self.err("elements nested too deeply");
        }
        if !self.rest().starts_with('<') {
            return self.err("expected an element");
        }
        self.pos += 1;
        let name = self.name()?.to_owned();
        let mut el = XElement {
            name,
            attrs: Vec::new(),
            children: Vec::new(),
        };
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.pos += 2;
                return Ok(el);
            }
            if r.starts_with('>') {
                self.pos += 1;
                break;
            }
            let k = self.name()?.to_owned();
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return self.err("expected '=' after an attribute name");
            }
            self.pos += 1;
            self.skip_ws();
            let Some(q @ ('"' | '\'')) = self.rest().chars().next() else {
                return self.err("expected a quoted attribute value");
            };
            self.pos += 1;
            let at = self.pos;
            let raw = self.until(if q == '"' { "\"" } else { "'" }, "unterminated attribute")?;
            if raw.contains('<') {
                return Err(XmlError {
                    reason: "'<' in an attribute value",
                    at,
                });
            }
            let v = unescape(raw, at)?;
            if el.attrs.iter().any(|(n, _)| *n == k) {
                return self.err("duplicate attribute");
            }
            el.attrs.push((k, v));
        }
        loop {
            let r = self.rest();
            if r.is_empty() {
                return self.err("unexpected end of document");
            }
            if r.starts_with("</") {
                self.pos += 2;
                let n = self.name()?;
                if n != el.name {
                    return self.err("mismatched end tag");
                }
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return self.err("expected '>'");
                }
                self.pos += 1;
                return Ok(el);
            } else if r.starts_with("<!--") {
                self.pos += 4;
                let c = self.until("-->", "unterminated comment")?;
                el.children.push(XNode::Comment(c.to_owned()));
            } else if r.starts_with("<![CDATA[") {
                self.pos += 9;
                let c = self.until("]]>", "unterminated CDATA section")?;
                el.children.push(XNode::CData(c.to_owned()));
            } else if r.starts_with("<?") {
                self.pos += 2;
                let c = self.until("?>", "unterminated processing instruction")?;
                el.children.push(XNode::Pi(c.to_owned()));
            } else if r.starts_with('<') {
                let child = self.element(depth + 1)?;
                el.children.push(XNode::Element(child));
            } else {
                let n = r.find('<').unwrap_or(r.len());
                let at = self.pos;
                let t = unescape(&r[..n], at)?;
                self.pos += n;
                if let Some(XNode::Text(prev)) = el.children.last_mut() {
                    prev.push_str(&t);
                } else {
                    el.children.push(XNode::Text(t));
                }
            }
        }
    }
}

/// Expand the predefined entities and character references (XML 1.0 §4.1, §4.6).
fn unescape(raw: &str, at: usize) -> Res<String> {
    if !raw.contains('&') {
        return Ok(raw.to_owned());
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let Some(end) = tail.find(';') else {
            return Err(XmlError {
                reason: "unterminated entity reference",
                at,
            });
        };
        let name = &tail[..end];
        let c = match name {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => out.push(c),
            None => {
                return Err(XmlError {
                    reason: "unknown entity",
                    at,
                })
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Parse a part's text (a UTF-8 byte order mark is skipped).
///
/// # Errors
/// Malformed XML (with the reason and offset).
pub fn parse(text: &str) -> Result<XDocument, XmlError> {
    let s = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut p = Parser { s, pos: 0 };
    p.misc(true)?;
    let prolog = s[..p.pos].to_owned();
    let root = p.element(0)?;
    let root_end = p.pos;
    p.misc(false)?;
    if !p.rest().trim().is_empty() {
        return p.err("content after the document element");
    }
    Ok(XDocument {
        prolog,
        root,
        epilog: s[root_end..].to_owned(),
    })
}

/// Decode a part's bytes: UTF-16 with a byte order mark, else UTF-8 (BOM skipped). Invalid
/// sequences become U+FFFD.
#[must_use]
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => {
                let _ = write!(out, "&#{};", u32::from(c));
            }
            c => out.push(c),
        }
    }
}

/// Serialize an element.
pub fn write_element(el: &XElement, out: &mut String) {
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if el.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &el.children {
        match c {
            XNode::Element(e) => write_element(e, out),
            XNode::Text(t) => escape_text(t, out),
            XNode::CData(t) => {
                out.push_str("<![CDATA[");
                out.push_str(t);
                out.push_str("]]>");
            }
            XNode::Comment(t) => {
                out.push_str("<!--");
                out.push_str(t);
                out.push_str("-->");
            }
            XNode::Pi(t) => {
                out.push_str("<?");
                out.push_str(t);
                out.push_str("?>");
            }
        }
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push('>');
}

/// Serialize a document. Its text is UTF-8, so an `encoding` pseudo-attribute of the XML
/// declaration other than UTF-8 is rewritten to `UTF-8`.
#[must_use]
pub fn write(doc: &XDocument) -> String {
    let mut out = utf8_declaration(&doc.prolog);
    write_element(&doc.root, &mut out);
    out.push_str(&doc.epilog);
    out
}

fn utf8_declaration(prolog: &str) -> String {
    let Some(decl_end) = prolog
        .starts_with("<?xml")
        .then(|| prolog.find("?>"))
        .flatten()
    else {
        return prolog.to_owned();
    };
    let decl = &prolog[..decl_end];
    let Some(i) = decl.find("encoding") else {
        return prolog.to_owned();
    };
    let after = &decl[i + "encoding".len()..];
    let Some(q_at) = after.find(['"', '\'']) else {
        return prolog.to_owned();
    };
    let q = &after[q_at..=q_at];
    let v_start = i + "encoding".len() + q_at + 1;
    let Some(v_len) = prolog[v_start..].find(q) else {
        return prolog.to_owned();
    };
    let mut out = String::with_capacity(prolog.len());
    out.push_str(&prolog[..v_start]);
    out.push_str("UTF-8");
    out.push_str(&prolog[v_start + v_len..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_back_unchanged_markup() {
        let src = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- c -->\n<r xmlns=\"urn:a\" b='1'>\n  <x>A &amp; B</x><y/><![CDATA[<raw>]]><?pi x?><!--k--></r>\n";
        let d = parse(src).expect("parse");
        assert_eq!(d.root.name, "r");
        assert_eq!(d.root.attrs[1], ("b".to_owned(), "1".to_owned()));
        assert_eq!(d.root.string_value(), "\n  A & B<raw>");
        let back = write(&d);
        // Single quotes become double quotes; everything else is as written.
        assert_eq!(
            back,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- c -->\n<r xmlns=\"urn:a\" b=\"1\">\n  <x>A &amp; B</x><y/><![CDATA[<raw>]]><?pi x?><!--k--></r>\n"
        );
    }

    #[test]
    fn character_references_and_errors() {
        let d = parse("<a v=\"&#x41;&#66;&lt;\">&apos;&quot;</a>").expect("parse");
        assert_eq!(d.root.attrs[0].1, "AB<");
        assert_eq!(d.root.string_value(), "'\"");
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a>&bogus;</a>").is_err());
        assert!(parse("<a/><b/>").is_err());
        assert!(parse("<a x=\"1\" x=\"2\"/>").is_err());
        assert!(parse("").is_err());
        let deep = "<a>".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn doctype_is_skipped_not_expanded() {
        let d = parse("<!DOCTYPE r [<!ENTITY e \"x>y\">]><r>&#65;</r>").expect("parse");
        assert!(d.prolog.starts_with("<!DOCTYPE"));
        assert_eq!(d.root.string_value(), "A");
        assert!(parse("<!DOCTYPE r [<!ENTITY e \"x\">]><r>&e;</r>").is_err());
    }

    #[test]
    fn decodes_utf16_with_bom() {
        let text = "<a>\u{1EA1}</a>";
        let mut le = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            le.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode(&le), text);
        assert_eq!(decode(b"\xEF\xBB\xBF<a/>"), "<a/>");
    }

    #[test]
    fn set_text_keeps_comments() {
        let mut d = parse("<a><!--n-->old<![CDATA[x]]></a>").expect("parse");
        d.root.set_text("new <v>");
        assert_eq!(write(&d), "<a><!--n-->new &lt;v&gt;</a>");
    }
}

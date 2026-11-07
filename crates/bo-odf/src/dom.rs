//! A small namespace-resolved XML tree used by the ODF readers.
//!
//! ODF documents are modest in size compared to the ZIP limits, and their
//! style resolution needs random access, so a tree is simpler than a pull
//! parser here. Depth and node count are capped (prompt §5.9).

use crate::{OdfError, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

/// Maximum element nesting.
pub const MAX_DEPTH: usize = 256;
/// Maximum number of nodes in one tree.
pub const MAX_NODES: usize = 20_000_000;

/// An attribute with resolved namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub ns: String,
    pub name: String,
    pub value: String,
}

/// An element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub ns: String,
    pub name: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
}

/// A child node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    /// Whether this element is `{ns}name`.
    pub fn is(&self, ns: &str, name: &str) -> bool {
        self.ns == ns && self.name == name
    }

    /// Attribute value.
    pub fn attr(&self, ns: &str, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.ns == ns && a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// First child element `{ns}name`.
    pub fn child(&self, ns: &str, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.is(ns, name))
    }

    /// Child elements `{ns}name`.
    pub fn children_named<'a>(
        &'a self,
        ns: &'a str,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.is(ns, name))
    }
}

fn xml_err(part: &str, message: impl Into<String>) -> OdfError {
    OdfError::Xml {
        part: part.to_owned(),
        message: message.into(),
    }
}

/// Resolve the predefined entities and character references.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(end) = tail.find(';') {
            if let Some(c) = entity(&tail[1..end]) {
                out.push(c);
                rest = &tail[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let n = name.strip_prefix('#')?;
            let v = if let Some(h) = n.strip_prefix('x').or_else(|| n.strip_prefix('X')) {
                u32::from_str_radix(h, 16).ok()?
            } else {
                n.parse().ok()?
            };
            char::from_u32(v)
        }
    }
}

/// Escape text for element content and attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Characters not allowed in XML 1.0.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            _ => out.push(c),
        }
    }
    out
}

fn ns_string(r: &ResolveResult<'_>) -> String {
    match r {
        ResolveResult::Bound(n) => n.0.to_owned(),
        _ => String::new(),
    }
}

fn start_element(reader: &NsReader<&[u8]>, ns: String, e: &BytesStart<'_>) -> Element {
    let attrs = e
        .attributes()
        .flatten()
        .filter(|a| a.key.as_namespace_binding().is_none())
        .map(|a| {
            let (ans, local) = reader.resolver().resolve_attribute(a.key);
            Attr {
                ns: ns_string(&ans),
                name: local.as_ref().to_owned(),
                value: unescape(&a.value),
            }
        })
        .collect();
    Element {
        ns,
        name: e.local_name().as_ref().to_owned(),
        attrs,
        children: Vec::new(),
    }
}

/// Parse `data` into a tree and return the root element.
pub fn parse(data: &[u8], part: &str) -> Result<Element> {
    let mut reader = NsReader::from_reader(data);
    let mut stack: Vec<Element> = Vec::new();
    let mut nodes = 0usize;
    loop {
        let (res, ev) = reader
            .read_resolved_event()
            .map_err(|e| xml_err(part, e.to_string()))?;
        let ns = ns_string(&res);
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(xml_err(part, "too many XML nodes"));
        }
        match ev {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(xml_err(part, "maximum XML nesting depth exceeded"));
                }
                let el = start_element(&reader, ns, &e);
                stack.push(el);
            }
            Event::Empty(e) => {
                let el = start_element(&reader, ns, &e);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(el)),
                    None => return Ok(el),
                }
            }
            Event::End(_) => {
                let Some(el) = stack.pop() else {
                    return Err(xml_err(part, "unbalanced end tag"));
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(el)),
                    None => return Ok(el),
                }
            }
            Event::Text(t) => {
                if let Some(parent) = stack.last_mut() {
                    push_text(parent, &t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(parent) = stack.last_mut() {
                    push_text(parent, &t.xml10_content());
                }
            }
            Event::GeneralRef(g) => {
                if let Some(parent) = stack.last_mut() {
                    let name: &str = &g.xml10_content();
                    if let Some(c) = entity(name) {
                        push_text(parent, &c.to_string());
                    }
                }
            }
            Event::Eof => return Err(xml_err(part, "no root element")),
            _ => {}
        }
    }
}

fn push_text(parent: &mut Element, s: &str) {
    if let Some(Node::Text(prev)) = parent.children.last_mut() {
        prev.push_str(s);
    } else {
        parent.children.push(Node::Text(s.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaces_and_entities() {
        let xml = br#"<?xml version="1.0"?><a xmlns="urn:x" xmlns:p="urn:p"><p:b p:k="1 &amp; 2">x &lt; y&#x1EA1;</p:b><c/></a>"#;
        let root = parse(xml, "t").unwrap();
        assert!(root.is("urn:x", "a"));
        let b = root.child("urn:p", "b").unwrap();
        assert_eq!(b.attr("urn:p", "k"), Some("1 & 2"));
        assert_eq!(b.children, vec![Node::Text("x < yạ".into())]);
        assert!(root.child("urn:x", "c").is_some());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"", "t").is_err());
        assert!(parse(b"<a><b></a>", "t").is_err());
        let deep = "<a>".repeat(MAX_DEPTH + 2);
        assert!(parse(deep.as_bytes(), "t").is_err());
    }
}

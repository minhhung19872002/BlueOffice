//! XML canonicalisation for signature verification: Canonical XML 1.0 (W3C REC-xml-c14n-20010315) and
//! Exclusive XML Canonicalization 1.0 (W3C REC-xml-exc-c14n-20020718), without comments, over a small DOM
//! that keeps character data exactly (line ends normalised as an XML processor does, XML 1.0 §2.11).
//!
//! The node-set is always an element and its descendants (what XML-DSig same-document references and
//! `SignedInfo` need); DTD defaults are not applied (OOXML signatures have no DTD).

use crate::{Result, SignError};
use bo_opc::xml::XmlReader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;

/// `xml:` namespace (Namespaces in XML §3).
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// A node of the DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
    /// Processing instruction (target, data).
    Pi(String, String),
}

/// An element: qualified name, attributes (qualified names, normalised values) in document order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

fn prefix_of(qname: &str) -> &str {
    qname.split_once(':').map_or("", |(p, _)| p)
}

fn local_of(qname: &str) -> &str {
    qname.split_once(':').map_or(qname, |(_, l)| l)
}

impl Element {
    pub fn local(&self) -> &str {
        local_of(&self.name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// First child element with this local name.
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.elements().find(|e| e.local() == local)
    }

    /// Concatenated character data of the element and its descendants.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for n in &self.children {
            match n {
                Node::Text(t) => s.push_str(t),
                Node::Element(e) => s.push_str(&e.text()),
                Node::Pi(..) => {}
            }
        }
        s
    }

    /// Namespace declarations of this element (prefix "" = default).
    fn declarations(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attrs.iter().filter_map(|(k, v)| {
            if k == "xmlns" {
                Some(("", v.as_str()))
            } else {
                k.strip_prefix("xmlns:").map(|p| (p, v.as_str()))
            }
        })
    }
}

/// XML 1.0 §2.11 end-of-line handling.
fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn element_of(e: &BytesStart<'_>) -> Result<Element> {
    let name: String = e.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| SignError::Malformed(format!("XML attribute: {err}")))?;
        let k: String = a.key.as_ref().to_owned();
        // XML 1.0 §3.3.3 attribute-value normalisation for CDATA attributes: each white space
        // character (after end-of-line handling) becomes a space, then references are replaced.
        let raw = normalize_eol(a.value.as_ref()).replace(['\t', '\n'], " ");
        attrs.push((k, bo_opc::xml::unescape(&raw)));
    }
    Ok(Element {
        name,
        attrs,
        children: Vec::new(),
    })
}

fn push_text(stack: &mut [Element], s: &str) {
    if let Some(top) = stack.last_mut() {
        if let Some(Node::Text(t)) = top.children.last_mut() {
            t.push_str(s);
        } else {
            top.children.push(Node::Text(s.to_owned()));
        }
    }
}

/// Parses a document into its root element (comments, the declaration and the DOCTYPE dropped).
pub(crate) fn parse(data: &[u8]) -> Result<Element> {
    let mut r = XmlReader::new(data, "signature");
    let mut stack: Vec<Element> = Vec::new();
    let finish = |stack: &mut Vec<Element>, el: Element| -> Option<Element> {
        match stack.last_mut() {
            Some(top) => {
                top.children.push(Node::Element(el));
                None
            }
            None => Some(el),
        }
    };
    loop {
        let ev = r
            .read()
            .map_err(|e| SignError::Malformed(format!("signature XML: {e}")))?;
        match ev {
            Event::Start(e) => stack.push(element_of(&e)?),
            Event::Empty(e) => {
                if let Some(root) = finish(&mut stack, element_of(&e)?) {
                    return Ok(root);
                }
            }
            Event::End(_) => {
                let el = stack
                    .pop()
                    .ok_or_else(|| SignError::Malformed("unbalanced end tag".into()))?;
                if let Some(root) = finish(&mut stack, el) {
                    return Ok(root);
                }
            }
            Event::Text(t) => push_text(&mut stack, &normalize_eol(&t)),
            Event::CData(c) => {
                let s: &str = &c;
                push_text(&mut stack, &normalize_eol(s));
            }
            Event::GeneralRef(g) => {
                let name: &str = &g;
                let s = bo_opc::xml::unescape(&format!("&{name};"));
                push_text(&mut stack, &s);
            }
            Event::PI(p) => {
                let raw: &str = &p;
                let (target, data) = raw.split_once(char::is_whitespace).unwrap_or((raw, ""));
                if !target.eq_ignore_ascii_case("xml") {
                    if let Some(top) = stack.last_mut() {
                        top.children
                            .push(Node::Pi(target.to_owned(), data.trim_start().to_owned()));
                    }
                }
            }
            Event::Eof => return Err(SignError::Malformed("signature XML has no root".into())),
            _ => {}
        }
    }
}

/// Canonicalisation algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Method {
    /// Canonical XML 1.0.
    Inclusive,
    /// Exclusive XML Canonicalization 1.0 with an `InclusiveNamespaces PrefixList` ("#default" = "").
    Exclusive(Vec<String>),
}

/// Algorithm URIs of XML-DSig (§6.5), with and without comments (comments are never in the DOM).
pub(crate) fn method_of(uri: &str, prefix_list: Option<&str>) -> Option<Method> {
    match uri {
        "http://www.w3.org/TR/2001/REC-xml-c14n-20010315"
        | "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments" => Some(Method::Inclusive),
        "http://www.w3.org/2001/10/xml-exc-c14n#"
        | "http://www.w3.org/2001/10/xml-exc-c14n#WithComments" => Some(Method::Exclusive(
            prefix_list
                .unwrap_or("")
                .split_whitespace()
                .map(|p| {
                    if p == "#default" {
                        String::new()
                    } else {
                        p.to_owned()
                    }
                })
                .collect(),
        )),
        _ => None,
    }
}

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
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
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
}

type NsMap = BTreeMap<String, String>;

struct Canon<'a> {
    method: &'a Method,
    out: String,
}

impl Canon<'_> {
    /// `in_scope`: namespaces in scope at the parent; `rendered`: namespaces already output by the
    /// nearest output ancestor; `xml_attrs`: inherited `xml:*` attributes (apex only, inclusive).
    fn element(
        &mut self,
        el: &Element,
        in_scope: &NsMap,
        rendered: &NsMap,
        xml_attrs: &[(String, String)],
    ) {
        let mut scope = in_scope.clone();
        for (p, u) in el.declarations() {
            scope.insert(p.to_owned(), u.to_owned());
        }
        let default_rendered = rendered.get("").map_or("", String::as_str);
        // Namespace declarations to output.
        let mut emit: Vec<(String, String)> = Vec::new();
        let consider = |p: &str, u: &str, emit: &mut Vec<(String, String)>| {
            if p == "xml" || emit.iter().any(|(q, _)| q == p) {
                return;
            }
            if p.is_empty() {
                if u.is_empty() {
                    // xmlns="" only to undo a default declared by an output ancestor.
                    if !default_rendered.is_empty() {
                        emit.push((String::new(), String::new()));
                    }
                } else if default_rendered != u {
                    emit.push((String::new(), u.to_owned()));
                }
            } else if rendered.get(p).map(String::as_str) != Some(u) {
                emit.push((p.to_owned(), u.to_owned()));
            }
        };
        match self.method {
            Method::Inclusive => {
                for (p, u) in &scope {
                    consider(p, u, &mut emit);
                }
                if !scope.contains_key("") {
                    consider("", "", &mut emit);
                }
            }
            Method::Exclusive(list) => {
                // Visibly utilised prefixes (Exc-C14N §3): the element's and its attributes'.
                let mut used: Vec<&str> = vec![prefix_of(&el.name)];
                for (k, _) in &el.attrs {
                    let p = prefix_of(k);
                    if !p.is_empty() && p != "xmlns" && k != "xmlns" {
                        used.push(p);
                    }
                }
                for p in used.into_iter().chain(list.iter().map(String::as_str)) {
                    let u = scope.get(p).map_or("", String::as_str);
                    if p.is_empty() || !u.is_empty() {
                        consider(p, u, &mut emit);
                    }
                }
            }
        }
        emit.sort();
        // Attributes: plain ones, then by namespace URI and local name (C14N §2.2).
        let mut attrs: Vec<(String, String, &str, &str)> = el
            .attrs
            .iter()
            .filter(|(k, _)| k != "xmlns" && !k.starts_with("xmlns:"))
            .map(|(k, v)| {
                let p = prefix_of(k);
                let uri = if p.is_empty() {
                    String::new()
                } else if p == "xml" {
                    XML_NS.to_owned()
                } else {
                    scope.get(p).cloned().unwrap_or_default()
                };
                (uri, local_of(k).to_owned(), k.as_str(), v.as_str())
            })
            .collect();
        for (k, v) in xml_attrs {
            if !el.attrs.iter().any(|(a, _)| a == k) {
                attrs.push((XML_NS.to_owned(), local_of(k).to_owned(), k, v));
            }
        }
        attrs.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        self.out.push('<');
        self.out.push_str(&el.name);
        for (p, u) in &emit {
            self.out
                .push_str(if p.is_empty() { " xmlns" } else { " xmlns:" });
            self.out.push_str(p);
            self.out.push_str("=\"");
            escape_attr(u, &mut self.out);
            self.out.push('"');
        }
        for (_, _, k, v) in &attrs {
            self.out.push(' ');
            self.out.push_str(k);
            self.out.push_str("=\"");
            escape_attr(v, &mut self.out);
            self.out.push('"');
        }
        self.out.push('>');
        let mut now_rendered = rendered.clone();
        for (p, u) in emit {
            now_rendered.insert(p, u);
        }
        for n in &el.children {
            match n {
                Node::Element(c) => self.element(c, &scope, &now_rendered, &[]),
                Node::Text(t) => escape_text(t, &mut self.out),
                Node::Pi(t, d) => {
                    self.out.push_str("<?");
                    self.out.push_str(t);
                    if !d.is_empty() {
                        self.out.push(' ');
                        self.out.push_str(d);
                    }
                    self.out.push_str("?>");
                }
            }
        }
        self.out.push_str("</");
        self.out.push_str(&el.name);
        self.out.push('>');
    }
}

/// Canonical form of the element at `path` (child-element indices from `root`; empty = the root) and
/// its descendants, with the namespaces (and, inclusive, `xml:*` attributes) its ancestors put in scope.
pub(crate) fn canonicalize(root: &Element, path: &[usize], method: &Method) -> Result<String> {
    let mut in_scope = NsMap::new();
    let mut xml_attrs: Vec<(String, String)> = Vec::new();
    let mut cur = root;
    for &i in path {
        for (p, u) in cur.declarations() {
            in_scope.insert(p.to_owned(), u.to_owned());
        }
        for (k, v) in &cur.attrs {
            if k.starts_with("xml:") {
                xml_attrs.retain(|(a, _)| a != k);
                xml_attrs.push((k.clone(), v.clone()));
            }
        }
        cur = cur
            .elements()
            .nth(i)
            .ok_or_else(|| SignError::Malformed("canonicalisation path".into()))?;
    }
    let inherited = match method {
        Method::Inclusive => xml_attrs,
        Method::Exclusive(_) => Vec::new(),
    };
    let mut c = Canon {
        method,
        out: String::new(),
    };
    c.element(cur, &in_scope, &NsMap::new(), &inherited);
    Ok(c.out)
}

/// Path (child-element indices) of the first element satisfying `pred`, depth first.
pub(crate) fn find_path(root: &Element, pred: &dyn Fn(&Element) -> bool) -> Option<Vec<usize>> {
    if pred(root) {
        return Some(Vec::new());
    }
    for (i, c) in root.elements().enumerate() {
        if let Some(mut p) = find_path(c, pred) {
            p.insert(0, i);
            return Some(p);
        }
    }
    None
}

/// Element at `path`.
pub(crate) fn at<'a>(root: &'a Element, path: &[usize]) -> Option<&'a Element> {
    let mut cur = root;
    for &i in path {
        cur = cur.elements().nth(i)?;
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: W3C Canonical XML 1.0 §3.3 "Start and End Tags" (input without the DTD, so `e9` gets
    // no defaulted attribute)
    #[test]
    fn start_and_end_tags_example() {
        let input = "<doc>\n   <e1   />\n   <e2   ></e2>\n   <e3   name = \"elem3\"   id=\"elem3\"   />\n   <e4   name=\"elem4\"   id=\"elem4\"   ></e4>\n   <e5 a:attr=\"out\" b:attr=\"sorted\" attr2=\"all\" attr=\"I'm\"\n      xmlns:b=\"http://www.ietf.org\"\n      xmlns:a=\"http://www.w3.org\"\n      xmlns=\"http://example.org\"/>\n   <e6 xmlns=\"\" xmlns:a=\"http://www.w3.org\">\n      <e7 xmlns=\"http://www.ietf.org\">\n         <e8 xmlns=\"\" xmlns:a=\"http://www.w3.org\">\n            <e9 xmlns=\"\" xmlns:a=\"http://www.ietf.org\"/>\n         </e8>\n      </e7>\n   </e6>\n</doc>";
        let expected = "<doc>\n   <e1></e1>\n   <e2></e2>\n   <e3 id=\"elem3\" name=\"elem3\"></e3>\n   <e4 id=\"elem4\" name=\"elem4\"></e4>\n   <e5 xmlns=\"http://example.org\" xmlns:a=\"http://www.w3.org\" xmlns:b=\"http://www.ietf.org\" attr=\"I'm\" attr2=\"all\" b:attr=\"sorted\" a:attr=\"out\"></e5>\n   <e6 xmlns:a=\"http://www.w3.org\">\n      <e7 xmlns=\"http://www.ietf.org\">\n         <e8 xmlns=\"\">\n            <e9 xmlns:a=\"http://www.ietf.org\"></e9>\n         </e8>\n      </e7>\n   </e6>\n</doc>";
        let root = parse(input.as_bytes()).unwrap();
        assert_eq!(
            canonicalize(&root, &[], &Method::Inclusive).unwrap(),
            expected
        );
    }

    // source: W3C Canonical XML 1.0 §3.4 "Character Modifications and Character References" (subset)
    #[test]
    fn character_references_and_escapes() {
        let input = "<doc>\r\n<text>First line&#x0d;&#10;Second line</text>\n<value>&#x32;</value>\n<compute><![CDATA[value>\"0\" && value<\"10\" ?\"valid\":\"error\"]]></compute>\n<norm attr=' &apos;   &#x20;&#13;&#xa;&#9;   &apos; '/>\n</doc>";
        let expected = "<doc>\n<text>First line&#xD;\nSecond line</text>\n<value>2</value>\n<compute>value&gt;\"0\" &amp;&amp; value&lt;\"10\" ?\"valid\":\"error\"</compute>\n<norm attr=\" '    &#xD;&#xA;&#x9;   ' \"></norm>\n</doc>";
        let root = parse(input.as_bytes()).unwrap();
        assert_eq!(
            canonicalize(&root, &[], &Method::Inclusive).unwrap(),
            expected
        );
    }

    // source: W3C Exclusive XML Canonicalization 1.0 §2.2 (only visibly utilised namespaces)
    #[test]
    fn subsets_inherit_namespaces_inclusively_but_not_exclusively() {
        let input =
            r#"<n0:a xmlns:n0="urn:a" xmlns:n1="urn:b" xml:lang="vi"><n0:b><n1:c/></n0:b></n0:a>"#;
        let root = parse(input.as_bytes()).unwrap();
        assert_eq!(
            canonicalize(&root, &[0], &Method::Inclusive).unwrap(),
            r#"<n0:b xmlns:n0="urn:a" xmlns:n1="urn:b" xml:lang="vi"><n1:c></n1:c></n0:b>"#
        );
        assert_eq!(
            canonicalize(&root, &[0], &Method::Exclusive(Vec::new())).unwrap(),
            r#"<n0:b xmlns:n0="urn:a"><n1:c xmlns:n1="urn:b"></n1:c></n0:b>"#
        );
        assert_eq!(
            canonicalize(&root, &[0], &Method::Exclusive(vec!["n1".into()])).unwrap(),
            r#"<n0:b xmlns:n0="urn:a" xmlns:n1="urn:b"><n1:c></n1:c></n0:b>"#
        );
    }
}

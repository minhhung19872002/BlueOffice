//! A tiny DOM over `quick-xml` used by the DrawingML / PresentationML readers
//! and writers. Parsing goes through [`bo_opc::xml::XmlReader`] so the nesting
//! depth limit (prompt §5.9) applies.

use bo_model::slide::{Extra, XmlElement, XmlNode};
use bo_opc::xml::{escape, unescape, XmlReader};
use quick_xml::events::{BytesStart, Event};

/// Parse result.
pub type DomResult<T> = Result<T, bo_opc::OpcError>;

fn start_to_element(e: &BytesStart<'_>) -> XmlElement {
    let name = e.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for a in e.attributes().with_checks(false).flatten() {
        let k = a.key.as_ref().to_owned();
        let raw = a.value.to_string();
        attrs.push((k, unescape(&raw)));
    }
    XmlElement {
        name,
        attrs,
        children: Vec::new(),
    }
}

fn push_text(stack: &mut [XmlElement], s: &str) {
    if let Some(top) = stack.last_mut() {
        if let Some(XmlNode::Text(t)) = top.children.last_mut() {
            t.push_str(s);
        } else {
            top.children.push(XmlNode::Text(s.to_owned()));
        }
    }
}

/// Parse a whole XML part into its root element. Comments, processing
/// instructions and the XML declaration are dropped.
pub fn parse(data: &[u8], part: &str) -> DomResult<XmlElement> {
    let mut r = XmlReader::new(data, part);
    let mut stack: Vec<XmlElement> = Vec::new();
    loop {
        match r.read()? {
            Event::Start(e) => stack.push(start_to_element(&e)),
            Event::Empty(e) => {
                let el = start_to_element(&e);
                match stack.last_mut() {
                    Some(top) => top.children.push(XmlNode::Element(el)),
                    None => return Ok(el),
                }
            }
            Event::End(_) => {
                let Some(el) = stack.pop() else {
                    return Err(r.err("unbalanced end tag"));
                };
                match stack.last_mut() {
                    Some(top) => top.children.push(XmlNode::Element(el)),
                    None => return Ok(el),
                }
            }
            Event::Text(t) => {
                let raw = <_ as AsRef<str>>::as_ref(&t).to_owned();
                push_text(&mut stack, &unescape(&raw));
            }
            Event::GeneralRef(g) => {
                let raw = <_ as AsRef<str>>::as_ref(&g).to_owned();
                push_text(&mut stack, &unescape(&format!("&{raw};")));
            }
            Event::CData(t) => {
                let raw = <_ as AsRef<str>>::as_ref(&t).to_owned();
                push_text(&mut stack, &raw);
            }
            Event::Eof => return Err(r.err("unexpected end of document")),
            _ => {}
        }
    }
}

/// Serialize an element (no XML declaration).
pub fn write_element(el: &XmlElement, out: &mut String) {
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape(v));
        out.push('"');
    }
    if el.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &el.children {
        match c {
            XmlNode::Element(e) => write_element(e, out),
            XmlNode::Text(t) => out.push_str(&escape(t)),
        }
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push('>');
}

/// Serialize a part: XML declaration + root element.
pub fn to_part_xml(root: &XmlElement) -> Vec<u8> {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    write_element(root, &mut s);
    s.into_bytes()
}

/// Canonical form for comparisons (prompt §7): attributes sorted, whitespace-only
/// text dropped (except inside `a:t`), `xmlns` declarations removed.
pub fn canonical(el: &XmlElement) -> String {
    fn canon(el: &XmlElement) -> XmlElement {
        let mut attrs: Vec<(String, String)> = el
            .attrs
            .iter()
            .filter(|(k, _)| k != "xmlns" && !k.starts_with("xmlns:"))
            .cloned()
            .collect();
        attrs.sort();
        let children = el
            .children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Element(e) => Some(XmlNode::Element(canon(e))),
                XmlNode::Text(t) if t.trim().is_empty() && el.local() != "t" => None,
                XmlNode::Text(t) => Some(XmlNode::Text(t.clone())),
            })
            .collect();
        XmlElement {
            name: el.name.clone(),
            attrs,
            children,
        }
    }
    let mut s = String::new();
    write_element(&canon(el), &mut s);
    s
}

// ---------------------------------------------------------------------------
// Reading helpers
// ---------------------------------------------------------------------------

/// Attribute bag: modelled attributes are taken out, the rest stays as "other".
pub struct AttrBag(Vec<(String, String)>);

impl AttrBag {
    /// Copy the attributes of an element.
    pub fn of(el: &XmlElement) -> Self {
        AttrBag(el.attrs.clone())
    }
    /// Remove and return an attribute.
    pub fn take(&mut self, name: &str) -> Option<String> {
        let i = self.0.iter().position(|(k, _)| k == name)?;
        Some(self.0.remove(i).1)
    }
    /// Take and parse a number (invalid values stay unmodelled).
    pub fn int<T: std::str::FromStr>(&mut self, name: &str) -> Option<T> {
        let i = self.0.iter().position(|(k, _)| k == name)?;
        let v = self.0[i].1.trim().parse::<T>().ok()?;
        self.0.remove(i);
        Some(v)
    }
    /// Take and parse an `xsd:boolean`.
    pub fn bool(&mut self, name: &str) -> Option<bool> {
        let i = self.0.iter().position(|(k, _)| k == name)?;
        let v = parse_bool(&self.0[i].1)?;
        self.0.remove(i);
        Some(v)
    }
    /// Remaining attributes.
    pub fn rest(self) -> Vec<(String, String)> {
        self.0
    }
}

/// `xsd:boolean`.
pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

/// Numeric attribute of an element.
pub fn attr_int<T: std::str::FromStr>(el: &XmlElement, name: &str) -> Option<T> {
    el.attr(name)?.trim().parse().ok()
}

/// Sort key of an element in a schema sequence: `2·i + 2` when the name is at
/// index `i`, otherwise right after the previous sibling (odd key).
pub fn key_for(schema: &[&str], name: &str, prev: u16) -> u16 {
    match schema.iter().position(|s| *s == name) {
        Some(i) => 2 * i as u16 + 2,
        None => prev | 1,
    }
}

/// Visit the child elements of `el`; `f` returns `true` when it modelled the
/// child. Unmodelled children are returned as [`Extra`]s keyed against `schema`.
pub fn walk<'a>(
    el: &'a XmlElement,
    schema: &[&str],
    mut f: impl FnMut(&'a XmlElement) -> bool,
) -> Vec<Extra> {
    let mut extra = Vec::new();
    let mut prev = 0u16;
    for c in el.elements() {
        let key = key_for(schema, &c.name, prev);
        prev = key;
        if !f(c) {
            extra.push(Extra {
                key,
                xml: c.clone(),
            });
        }
    }
    extra
}

// ---------------------------------------------------------------------------
// Writing helpers
// ---------------------------------------------------------------------------

/// Element builder.
pub struct El(pub XmlElement);

impl El {
    /// New element.
    pub fn new(name: &str) -> Self {
        El(XmlElement::new(name))
    }
    /// Add an attribute.
    #[must_use]
    pub fn a(mut self, k: &str, v: impl ToString) -> Self {
        self.0.attrs.push((k.to_owned(), v.to_string()));
        self
    }
    /// Add an optional attribute.
    #[must_use]
    pub fn opt<T: ToString>(self, k: &str, v: Option<T>) -> Self {
        match v {
            Some(v) => self.a(k, v),
            None => self,
        }
    }
    /// Add an optional boolean attribute (`1`/`0`).
    #[must_use]
    pub fn optb(self, k: &str, v: Option<bool>) -> Self {
        self.opt(k, v.map(|b| if b { "1" } else { "0" }))
    }
    /// Append unmodelled attributes.
    #[must_use]
    pub fn rest(mut self, attrs: &[(String, String)]) -> Self {
        self.0.attrs.extend(attrs.iter().cloned());
        self
    }
    /// Append a child element.
    #[must_use]
    pub fn child(mut self, c: XmlElement) -> Self {
        self.0.children.push(XmlNode::Element(c));
        self
    }
    /// Append text.
    #[must_use]
    pub fn text(mut self, t: &str) -> Self {
        self.0.children.push(XmlNode::Text(t.to_owned()));
        self
    }
    /// Finish.
    pub fn done(self) -> XmlElement {
        self.0
    }
}

/// Collects children with schema keys and merges extras back in order.
pub struct Kids<'s> {
    schema: &'s [&'s str],
    items: Vec<(u16, XmlElement)>,
}

impl<'s> Kids<'s> {
    /// New collector for a schema sequence.
    pub fn new(schema: &'s [&'s str]) -> Self {
        Kids {
            schema,
            items: Vec::new(),
        }
    }
    /// Add a modelled child (keyed by its name).
    pub fn push(&mut self, el: XmlElement) {
        let key = key_for(self.schema, &el.name, 0).max(2);
        self.items.push((key, el));
    }
    /// Add an optional modelled child.
    pub fn push_opt(&mut self, el: Option<XmlElement>) {
        if let Some(e) = el {
            self.push(e);
        }
    }
    /// Add extras.
    pub fn extras(&mut self, extra: &[Extra]) {
        for x in extra {
            self.items.push((x.key, x.xml.clone()));
        }
    }
    /// Append everything, in key order, to `parent`.
    pub fn append_to(mut self, parent: &mut XmlElement) {
        self.items.sort_by_key(|(k, _)| *k);
        parent
            .children
            .extend(self.items.into_iter().map(|(_, e)| XmlNode::Element(e)));
    }
    /// Build `parent` with the children.
    pub fn build(self, parent: El) -> XmlElement {
        let mut p = parent.done();
        self.append_to(&mut p);
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_write_roundtrip() {
        let xml = br#"<?xml version="1.0"?><a:b xmlns:a="x" k="1&amp;2"><a:t> hi &lt;there&gt; </a:t><c/></a:b>"#;
        let el = parse(xml, "/t.xml").unwrap();
        assert_eq!(el.attr("k"), Some("1&2"));
        assert_eq!(el.child("a:t").unwrap().text(), " hi <there> ");
        let mut s = String::new();
        write_element(&el, &mut s);
        let back = parse(s.as_bytes(), "/t.xml").unwrap();
        assert_eq!(back, el);
    }

    #[test]
    fn extras_keep_position() {
        let schema = ["a", "b", "c"];
        let el = parse(b"<r><a/><x:u/><b/><c/><x:v/></r>", "/t").unwrap();
        let extra = walk(&el, &schema, |e| e.name != "x:u" && e.name != "x:v");
        let mut k = Kids::new(&schema);
        k.push(XmlElement::new("c"));
        k.push(XmlElement::new("a"));
        k.push(XmlElement::new("b"));
        k.extras(&extra);
        let out = k.build(El::new("r"));
        let names: Vec<_> = out.elements().map(|e| e.name.clone()).collect();
        assert_eq!(names, ["a", "x:u", "b", "c", "x:v"]);
    }

    #[test]
    fn malformed_is_error() {
        assert!(parse(b"<a><b></a>", "/t").is_err());
        assert!(parse(b"<a>", "/t").is_err());
    }
}

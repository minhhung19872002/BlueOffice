//! A small, forgiving HTML parser: text → element tree.
//!
//! The tokenizer and tree builder follow the HTML Living Standard (§13.2
//! "Parsing HTML documents") only as far as an importer needs: tags, attributes,
//! comments, character references, void elements, raw-text elements and the
//! implied end tags of paragraphs, list items and table parts. Malformed markup
//! never fails; it yields the tree a lenient reader would expect.
//!
//! Scripts are dropped while tokenizing: the content of `script` (and of other
//! elements that carry code or foreign content) never reaches the tree.

use crate::entities;

/// Deepest element nesting kept; deeper start tags do not open a new level.
pub const MAX_DEPTH: usize = 96;
/// Most nodes kept; the rest of the input is ignored.
pub const MAX_NODES: usize = 2_000_000;

/// A node of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// An element.
    Element(Element),
    /// Character data (character references decoded).
    Text(String),
}

/// An element: lower-case name, attributes (lower-case names, decoded values)
/// and children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    /// Tag name, lower case.
    pub name: String,
    /// Attributes in source order.
    pub attrs: Vec<(String, String)>,
    /// Child nodes.
    pub children: Vec<Node>,
}

impl Element {
    /// Value of attribute `name` (lower case).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The element has `class` among its space-separated classes.
    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|x| x == class))
    }

    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// All text below the element, concatenated.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(n) = stack.pop() {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => stack.extend(e.children.iter().rev()),
            }
        }
        out
    }

    /// First descendant element named `name` (depth first, document order).
    pub fn find(&self, name: &str) -> Option<&Element> {
        let mut stack: Vec<&Element> = vec![self];
        while let Some(e) = stack.pop() {
            if e.name == name {
                return Some(e);
            }
            let kids: Vec<&Element> = e.elements().collect();
            stack.extend(kids.into_iter().rev());
        }
        None
    }

    /// Every descendant element (and the element itself) in document order.
    pub fn descendants(&self) -> Vec<&Element> {
        let mut out = Vec::new();
        let mut stack: Vec<&Element> = vec![self];
        while let Some(e) = stack.pop() {
            out.push(e);
            let kids: Vec<&Element> = e.elements().collect();
            stack.extend(kids.into_iter().rev());
        }
        out
    }
}

/// Elements without content (HTML §13.1.2 "Void elements", plus legacy ones).
fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "basefont"
            | "bgsound"
            | "br"
            | "col"
            | "embed"
            | "frame"
            | "hr"
            | "img"
            | "input"
            | "keygen"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Elements whose content is not markup and is thrown away: code, foreign
/// content and embedded documents.
fn is_dropped(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "noscript"
            | "template"
            | "iframe"
            | "object"
            | "applet"
            | "svg"
            | "math"
            | "noembed"
            | "noframes"
            | "xml"
            | "select"
            | "canvas"
            | "audio"
            | "video"
    )
}

/// Elements whose content is raw text kept as one text node.
fn is_raw_text(name: &str) -> bool {
    matches!(name, "style" | "title" | "textarea")
}

/// Block-level elements that end an open paragraph (HTML §13.2.6.4.7, "close a p element").
fn closes_p(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "center"
            | "dd"
            | "details"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "ul"
    )
}

/// The open elements that a start tag `name` closes implicitly: `(closed, barrier)` —
/// the innermost open element named in `closed` is closed (with everything inside it)
/// unless an element named in `barrier` is met first.
fn implied_end(name: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    const TABLE: &[&str] = &["table"];
    Some(match name {
        "li" => (&["li"], &["ul", "ol", "menu", "dir", "table"]),
        "dt" | "dd" => (&["dt", "dd"], &["dl", "table"]),
        "tr" => (&["tr"], TABLE),
        "td" | "th" => (&["td", "th"], &["tr", "table"]),
        "thead" | "tbody" | "tfoot" => (&["thead", "tbody", "tfoot"], TABLE),
        "caption" | "colgroup" => (&["caption", "colgroup"], TABLE),
        "option" => (&["option"], &["select"]),
        _ => return None,
    })
}

struct Builder {
    /// Open elements, the document root first.
    stack: Vec<Element>,
    nodes: usize,
}

impl Builder {
    fn top(&mut self) -> &mut Element {
        if self.stack.is_empty() {
            self.stack.push(Element::default());
        }
        let last = self.stack.len() - 1;
        &mut self.stack[last]
    }

    fn text(&mut self, t: &str) {
        if t.is_empty() || self.nodes >= MAX_NODES {
            return;
        }
        if let Some(Node::Text(prev)) = self.top().children.last_mut() {
            prev.push_str(t);
        } else {
            self.nodes += 1;
            self.top().children.push(Node::Text(t.to_owned()));
        }
    }

    /// Close the innermost open element (never the root).
    fn pop(&mut self) {
        if self.stack.len() > 1 {
            if let Some(e) = self.stack.pop() {
                self.top().children.push(Node::Element(e));
            }
        }
    }

    /// Index of the innermost open element named in `names`, stopping at `barrier`.
    fn open_index(&self, names: &[&str], barrier: &[&str]) -> Option<usize> {
        for i in (1..self.stack.len()).rev() {
            let n = self.stack[i].name.as_str();
            if names.contains(&n) {
                return Some(i);
            }
            if barrier.contains(&n) {
                return None;
            }
        }
        None
    }

    fn close_to(&mut self, index: usize) {
        while self.stack.len() > index {
            self.pop();
        }
    }

    fn start(&mut self, name: String, attrs: Vec<(String, String)>, self_closing: bool) {
        if self.nodes >= MAX_NODES {
            return;
        }
        if closes_p(&name) {
            if let Some(i) = self.open_index(&["p"], &["table", "td", "th", "caption", "button"]) {
                self.close_to(i);
            }
        }
        if let Some((closed, barrier)) = implied_end(&name) {
            if let Some(i) = self.open_index(closed, barrier) {
                self.close_to(i);
            }
        }
        self.nodes += 1;
        let el = Element {
            name,
            attrs,
            children: Vec::new(),
        };
        if is_void(&el.name) || self_closing || self.stack.len() > MAX_DEPTH {
            self.top().children.push(Node::Element(el));
        } else {
            self.top();
            self.stack.push(el);
        }
    }

    fn end(&mut self, name: &str) {
        // `</p>` without an open paragraph is an empty paragraph (§13.2.6.4.7).
        if name == "p" && self.open_index(&["p"], &["table", "td", "th"]).is_none() {
            self.start("p".into(), Vec::new(), true);
            return;
        }
        // `</br>` is treated as `<br>`.
        if name == "br" {
            self.start("br".into(), Vec::new(), true);
            return;
        }
        // An end tag closes its innermost open element; cells and rows do not
        // reach out of their table.
        let barrier: &[&str] = match name {
            "td" | "th" | "tr" | "thead" | "tbody" | "tfoot" | "caption" | "li" => &["table"],
            _ => &[],
        };
        if let Some(i) = self.open_index(&[name], barrier) {
            self.close_to(i);
        }
    }

    fn finish(mut self) -> Element {
        while self.stack.len() > 1 {
            self.pop();
        }
        self.stack.pop().unwrap_or_default()
    }
}

fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

/// Case-insensitive search for `</name` at or after `from`.
fn find_close(lower: &[u8], from: usize, name: &str) -> Option<usize> {
    let pat = format!("</{name}");
    let pat = pat.as_bytes();
    let mut i = from;
    while i + pat.len() <= lower.len() {
        if lower[i] == b'<' && lower[i..].starts_with(pat) {
            let after = lower.get(i + pat.len()).copied();
            if after.is_none_or(|c| c == b'>' || c == b'/' || c.is_ascii_whitespace()) {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Parse an attribute list starting at `i` (just after the tag name). Returns the
/// attributes, whether the tag is self-closing and the index after `>`.
fn parse_attrs(src: &str, mut i: usize) -> (Vec<(String, String)>, bool, usize) {
    let b = src.as_bytes();
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut self_closing = false;
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            self_closing = b[i] == b'/';
            i += 1;
        }
        if i >= b.len() {
            return (attrs, self_closing, i);
        }
        if b[i] == b'>' {
            return (attrs, self_closing, i + 1);
        }
        self_closing = false;
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        if i == start {
            // A stray `=`: skip it.
            i += 1;
            continue;
        }
        let name = src[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                let ve = src[vs..].find(char::from(q)).map_or(b.len(), |k| vs + k);
                value = entities::decode(&src[vs..ve]);
                i = (ve + 1).min(b.len());
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = entities::decode(&src[vs..i]);
            }
        }
        if attrs.len() < 256 && !attrs.iter().any(|(k, _)| *k == name) {
            attrs.push((name, value));
        }
    }
}

/// Parse HTML text into a tree. The returned element is a nameless root holding the
/// top-level nodes (usually one `html` element).
pub fn parse(src: &str) -> Element {
    let b = src.as_bytes();
    let lower: Vec<u8> = b.iter().map(u8::to_ascii_lowercase).collect();
    let mut out = Builder {
        stack: vec![Element::default()],
        nodes: 0,
    };
    let mut i = 0;
    let mut text_start = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let next = b.get(i + 1).copied().unwrap_or(0);
        let is_markup = is_name_start(next) || matches!(next, b'/' | b'!' | b'?');
        if !is_markup {
            i += 1;
            continue;
        }
        out.text(&entities::decode(&src[text_start..i]));
        if next == b'!' || next == b'?' {
            // Comment, doctype, CDATA section or processing instruction.
            let end = if src[i..].starts_with("<!--") {
                src[i + 4..].find("-->").map(|k| i + 4 + k + 3)
            } else if src[i..].starts_with("<![CDATA[") {
                src[i + 9..].find("]]>").map(|k| i + 9 + k + 3)
            } else {
                src[i..].find('>').map(|k| i + k + 1)
            };
            i = end.unwrap_or(b.len());
            text_start = i;
            continue;
        }
        if next == b'/' {
            let ns = i + 2;
            let mut ne = ns;
            while ne < b.len() && !b[ne].is_ascii_whitespace() && b[ne] != b'>' && b[ne] != b'/' {
                ne += 1;
            }
            let name = src[ns..ne].to_ascii_lowercase();
            i = src[ne..].find('>').map_or(b.len(), |k| ne + k + 1);
            text_start = i;
            if !name.is_empty() {
                out.end(&name);
            }
            continue;
        }
        let ns = i + 1;
        let mut ne = ns;
        while ne < b.len() && !b[ne].is_ascii_whitespace() && b[ne] != b'>' && b[ne] != b'/' {
            ne += 1;
        }
        let name = src[ns..ne].to_ascii_lowercase();
        let (attrs, self_closing, after) = parse_attrs(src, ne);
        i = after;
        text_start = i;
        // Namespaced office markup (`<o:p>`, `<v:shape>`, `<w:…>`) carries nothing
        // an importer shows.
        let foreign = name.contains(':');
        // `<o:p>` wraps ordinary inline content in exported documents: only the
        // tags go.
        if foreign && (self_closing || name == "o:p") {
            continue;
        }
        if is_dropped(&name) && self_closing && name != "script" {
            continue;
        }
        if foreign || is_dropped(&name) || is_raw_text(&name) {
            let close = find_close(&lower, i, &name);
            // Unterminated: a foreign element loses its tag only, a dropped or
            // raw-text element takes the rest of the input.
            let body_end = close.unwrap_or(if foreign { i } else { b.len() });
            if is_raw_text(&name) {
                out.start(name.clone(), attrs, false);
                let raw = &src[i..body_end];
                if name == "style" {
                    out.text(raw);
                } else {
                    out.text(&entities::decode(raw));
                }
                out.end(&name);
            }
            i = match close {
                Some(c) => src[c..].find('>').map_or(b.len(), |k| c + k + 1),
                None => body_end,
            };
            text_start = i;
            continue;
        }
        // The self-closing flag means nothing on HTML elements (§13.2.5.40).
        out.start(name, attrs, false);
    }
    out.text(&entities::decode(&src[text_start.min(b.len())..]));
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(e: &Element) -> String {
        let mut s = String::new();
        for n in &e.children {
            match n {
                Node::Text(t) => s.push_str(t),
                Node::Element(c) => {
                    s.push('<');
                    s.push_str(&c.name);
                    for (k, v) in &c.attrs {
                        s.push_str(&format!(" {k}={v:?}"));
                    }
                    s.push('>');
                    s.push_str(&dump(c));
                    s.push_str(&format!("</{}>", c.name));
                }
            }
        }
        s
    }

    #[test]
    fn tags_attributes_and_text() {
        let t = parse("<P Class=a id='x' data-v=1 hidden>T&amp;C &lt; 5 &#x1EA0;<BR/>x</p>tail");
        assert_eq!(
            dump(&t),
            "<p class=\"a\" id=\"x\" data-v=\"1\" hidden=\"\">T&C < 5 Ạ<br></br>x</p>tail"
        );
        assert_eq!(parse("a < b and 3<4").text(), "a < b and 3<4");
    }

    #[test]
    fn implied_end_tags() {
        assert_eq!(
            dump(&parse("<p>a<p>b<div>c</div>")),
            "<p>a</p><p>b</p><div>c</div>"
        );
        assert_eq!(
            dump(&parse("<ul><li>a<li>b<ul><li>c</ul><li>d</ul>")),
            "<ul><li>a</li><li>b<ul><li>c</li></ul></li><li>d</li></ul>"
        );
        assert_eq!(
            dump(&parse("<table><tr><td>1<td>2<tr><th>3</table>x")),
            "<table><tr><td>1</td><td>2</td></tr><tr><th>3</th></tr></table>x"
        );
        // A nested table does not close the outer cell.
        assert_eq!(
            dump(&parse("<table><tr><td><table><tr><td>i</table>o</table>")),
            "<table><tr><td><table><tr><td>i</td></tr></table>o</td></tr></table>"
        );
        assert_eq!(dump(&parse("a</b></p>c")), "a<p></p>c");
    }

    #[test]
    fn scripts_never_reach_the_tree() {
        let t = parse(
            "<p>a</p><script>document.write('<p>x</p>')</script><SCRIPT src=x></SCRIPT>\
             <style>p{color:red}</style><svg><script>1</script></svg><iframe src=x>y</iframe>b\
             <!-- <script>c</script> --><p onclick=\"evil()\">d</p><title>T &amp; t</title>",
        );
        let d = dump(&t);
        assert!(!d.contains("script"), "{d}");
        assert!(!d.contains("document.write") && !d.contains("svg") && !d.contains("iframe"));
        assert!(d.contains("<style>p{color:red}</style>"));
        assert!(d.contains("<title>T & t</title>"));
        // An unterminated script swallows the rest.
        assert_eq!(dump(&parse("a<script>b<p>c")), "a");
    }

    #[test]
    fn office_namespaced_markup_is_skipped() {
        let t = parse(
            "<p>a<o:p></o:p><v:shape id=x><v:imagedata src=y/></v:shape>b<o:p>&nbsp;</o:p></p>",
        );
        assert_eq!(dump(&t), "<p>ab\u{a0}</p>");
    }

    #[test]
    fn limits_hold() {
        let deep = "<div>".repeat(5000) + "x";
        let t = parse(&deep);
        assert_eq!(t.text(), "x");
        let mut depth = 0;
        let mut e = &t;
        while let Some(c) = e.elements().next() {
            depth += 1;
            e = c;
        }
        assert!(depth <= MAX_DEPTH + 1, "{depth}");
        // Broken markup of every kind returns.
        for s in [
            "<",
            "<a",
            "<a b",
            "<a b=",
            "<a b='",
            "</",
            "<!--",
            "<![CDATA[",
            "<p <p>",
            "&#;&#x;&",
            "<a =>",
            "<style>",
        ] {
            let _ = parse(s);
        }
    }
}

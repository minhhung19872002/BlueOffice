//! In-place edits of XML parts kept verbatim (rich data, metadata, feature
//! property bags): every element is indexed with its byte offsets so new
//! children can be appended and attributes rewritten without re-serializing —
//! and so without losing — anything the writer does not understand.

use bo_opc::xml::{escape, unescape, XmlReader};
use quick_xml::events::{BytesStart, Event};

/// One element of an indexed document.
#[derive(Debug, Clone)]
pub(crate) struct El {
    /// Qualified name as written (`xlrd:rvb`).
    pub name: String,
    /// Local name (`rvb`).
    pub local: String,
    /// Index of the parent element.
    pub parent: Option<usize>,
    /// Offset of `<` of the start tag.
    pub start: usize,
    /// Offset just after the start tag (`>` or `/>`).
    pub tag_end: usize,
    /// Offset of the end tag (`</`); `tag_end` for empty elements.
    pub inner_end: usize,
    /// The element is written `<x/>`.
    pub empty: bool,
    /// Attributes: qualified name and raw (still escaped) value.
    pub attrs: Vec<(String, String)>,
    /// Direct text content (unescaped).
    pub text: String,
}

impl El {
    /// Unescaped value of the attribute with local name `local`.
    pub fn attr(&self, local: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(q, _)| q.rsplit(':').next() == Some(local))
            .map(|(_, v)| unescape(v))
    }

    /// Namespace prefix of the element (`xlrd` for `xlrd:rvb`, empty for none).
    pub fn prefix(&self) -> &str {
        self.name.rsplit_once(':').map_or("", |(p, _)| p)
    }

    /// `name` qualified with this element's prefix.
    pub fn q(&self, local: &str) -> String {
        match self.prefix() {
            "" => local.to_owned(),
            p => format!("{p}:{local}"),
        }
    }
}

/// The elements of a document in document order (`None` for malformed XML).
pub(crate) fn index(data: &[u8], part: &str) -> Option<Vec<El>> {
    let mut r = XmlReader::new(data, part);
    let mut out: Vec<El> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        let p0 = r.pos();
        let ev = r.read().ok()?;
        let p1 = r.pos();
        match ev {
            Event::Start(e) => {
                out.push(new_el(&e, stack.last().copied(), p0, p1, false));
                stack.push(out.len() - 1);
            }
            Event::Empty(e) => out.push(new_el(&e, stack.last().copied(), p0, p1, true)),
            Event::End(_) => {
                let i = stack.pop()?;
                out[i].inner_end = p0;
            }
            Event::Text(t) => {
                if let Some(&i) = stack.last() {
                    let t: &str = &t;
                    out[i].text.push_str(&unescape(t));
                }
            }
            Event::GeneralRef(g) => {
                if let Some(&i) = stack.last() {
                    let t: &str = &g;
                    out[i].text.push_str(&unescape(&format!("&{t};")));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    stack.is_empty().then_some(out)
}

fn new_el(e: &BytesStart<'_>, parent: Option<usize>, start: usize, end: usize, empty: bool) -> El {
    let name = e.name().as_ref().to_owned();
    let local = e.local_name().as_ref().to_owned();
    let attrs = e
        .attributes()
        .with_checks(false)
        .flatten()
        .map(|a| (a.key.as_ref().to_owned(), a.value.to_string()))
        .collect();
    El {
        name,
        local,
        parent,
        start,
        tag_end: end,
        inner_end: end,
        empty,
        attrs,
        text: String::new(),
    }
}

/// Children of element `i`.
pub(crate) fn children(els: &[El], i: usize) -> impl Iterator<Item = (usize, &El)> {
    els.iter()
        .enumerate()
        .filter(move |(_, e)| e.parent == Some(i))
}

/// Edits of one element: its index, new attribute values, appended children.
type TagEdit = (usize, Vec<(String, String)>, String);

/// Pending edits of one document, applied together.
#[derive(Debug, Default)]
pub(crate) struct Patch {
    /// (start, end, replacement) — non-overlapping ranges.
    edits: Vec<(usize, usize, String)>,
    /// Per element: new attribute values and appended children.
    tags: Vec<TagEdit>,
}

impl Patch {
    /// Sets attribute `qname` of element `i` (added when missing).
    pub fn set_attr(&mut self, i: usize, qname: &str, value: &str) {
        let t = self.tag(i);
        t.1.retain(|(q, _)| q != qname);
        t.1.push((qname.to_owned(), value.to_owned()));
    }

    /// Appends `xml` as the last children of element `i`.
    pub fn append(&mut self, i: usize, xml: &str) {
        self.tag(i).2.push_str(xml);
    }

    /// Inserts `xml` right before element `i` (as its previous sibling).
    pub fn insert_before(&mut self, el: &El, xml: &str) {
        self.edits.push((el.start, el.start, xml.to_owned()));
    }

    fn tag(&mut self, i: usize) -> &mut TagEdit {
        let pos = if let Some(p) = self.tags.iter().position(|t| t.0 == i) {
            p
        } else {
            self.tags.push((i, Vec::new(), String::new()));
            self.tags.len() - 1
        };
        &mut self.tags[pos]
    }

    /// The patched document.
    pub fn apply(mut self, data: &[u8], els: &[El]) -> Vec<u8> {
        for (i, set, append) in std::mem::take(&mut self.tags) {
            let Some(el) = els.get(i) else { continue };
            if !set.is_empty() || el.empty {
                let mut tag = format!("<{}", el.name);
                let mut attrs = el.attrs.clone();
                for (q, v) in &set {
                    let v = escape(v);
                    match attrs.iter_mut().find(|(n, _)| n == q) {
                        Some(a) => a.1 = v,
                        None => attrs.push((q.clone(), v)),
                    }
                }
                for (q, v) in &attrs {
                    tag.push_str(&format!(" {q}=\"{v}\""));
                }
                if el.empty && !append.is_empty() {
                    tag.push('>');
                    tag.push_str(&append);
                    tag.push_str(&format!("</{}>", el.name));
                } else if el.empty {
                    tag.push_str("/>");
                } else {
                    tag.push('>');
                }
                self.edits.push((el.start, el.tag_end, tag));
                if !el.empty && !append.is_empty() {
                    self.edits.push((el.inner_end, el.inner_end, append));
                }
            } else if !append.is_empty() {
                self.edits.push((el.inner_end, el.inner_end, append));
            }
        }
        // Later offsets first so earlier ones stay valid; inserts at the same offset keep order.
        let mut edits: Vec<(usize, (usize, usize, String))> =
            self.edits.into_iter().enumerate().collect();
        edits.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(b.0.cmp(&a.0)));
        let mut out = data.to_vec();
        for (_, (s, e, x)) in edits {
            if s <= e && e <= out.len() {
                out.splice(s..e, x.into_bytes());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_into_empty_and_open_elements_and_rewrites_attributes() {
        let xml = br#"<?xml version="1.0"?><a:r xmlns:a="u" count="1"><a:x i="1">t&amp;u</a:x><a:y/></a:r>"#;
        let els = index(xml, "/t.xml").unwrap();
        assert_eq!(els.len(), 3);
        assert_eq!(els[1].text, "t&u");
        assert_eq!(els[1].q("z"), "a:z");
        let mut p = Patch::default();
        p.set_attr(0, "count", "2");
        p.append(0, "<a:x i=\"2\"/>");
        p.append(2, "<a:k/>");
        let out = String::from_utf8(p.apply(xml, &els)).unwrap();
        assert_eq!(
            out,
            r#"<?xml version="1.0"?><a:r xmlns:a="u" count="2"><a:x i="1">t&amp;u</a:x><a:y><a:k/></a:y><a:x i="2"/></a:r>"#
        );
    }
}

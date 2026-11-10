//! XML sources (XML 1.0): one row per repeating element.
//!
//! The repeating element is named by the query or detected: the local name occurring most
//! often (at least twice) under one parent. Each occurrence is a row whose columns are its
//! attributes, then its child elements: a child holding only text gives a column of that
//! name, a child with element children gives `child.grandchild` columns (deeper levels as
//! their text). Namespaces are ignored (local names).

use crate::value::{QTable, Val};
use crate::QueryError;
use bo_opc::xml::{unescape, XmlReader};
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;

/// Most elements read from one document.
const MAX_NODES: usize = 2_000_000;

#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn all_text(&self) -> String {
        let mut s = self.text.trim().to_owned();
        for c in &self.children {
            let t = c.all_text();
            if !t.is_empty() {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&t);
            }
        }
        s
    }
}

fn local(e: &BytesStart<'_>) -> String {
    let n = e.local_name();
    let s: &str = n.as_ref();
    s.to_owned()
}

fn attrs_of(e: &BytesStart<'_>) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .filter_map(|a| {
            let k: &str = a.key.as_ref();
            if k == "xmlns" || k.starts_with("xmlns:") {
                return None;
            }
            let l = k.rsplit_once(':').map_or(k, |(_, l)| l);
            Some((l.to_owned(), unescape(&a.value)))
        })
        .collect()
}

fn parse(bytes: &[u8]) -> Result<Node, QueryError> {
    let text = crate::text::decode(bytes, None).text;
    let mut r = XmlReader::new(text.as_bytes(), "xml");
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut count = 0usize;
    let err = |e: bo_opc::OpcError| QueryError::Source(format!("xml: {e}"));
    loop {
        match r.read().map_err(err)? {
            Event::Start(e) => {
                count += 1;
                stack.push(Node {
                    name: local(&e),
                    attrs: attrs_of(&e),
                    ..Node::default()
                });
            }
            Event::Empty(e) => {
                count += 1;
                let n = Node {
                    name: local(&e),
                    attrs: attrs_of(&e),
                    ..Node::default()
                };
                if let Some(top) = stack.last_mut() {
                    top.children.push(n);
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    if let Some(n) = stack.pop() {
                        if let Some(top) = stack.last_mut() {
                            top.children.push(n);
                        }
                    }
                }
            }
            Event::Text(t) => {
                let t: &str = &t;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&unescape(t));
                }
            }
            Event::GeneralRef(g) => {
                let t: &str = &g;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&unescape(&format!("&{t};")));
                }
            }
            Event::CData(c) => {
                let t: &str = &c;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(t);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if count > MAX_NODES {
            return Err(QueryError::Source("xml: document too large".into()));
        }
    }
    let mut doc = stack.into_iter().next().unwrap_or_default();
    match doc.children.len() {
        0 => Err(QueryError::Source("xml: no root element".into())),
        _ => Ok(doc.children.swap_remove(0)),
    }
}

/// Repeating element names of a document with their counts, most frequent first.
pub fn repeating_elements(bytes: &[u8]) -> Vec<(String, usize)> {
    let Ok(root) = parse(bytes) else {
        return Vec::new();
    };
    let mut best: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut stack = vec![&root];
    while let Some(n) = stack.pop() {
        let mut here: HashMap<&str, usize> = HashMap::new();
        for c in &n.children {
            *here.entry(c.name.as_str()).or_default() += 1;
        }
        for c in &n.children {
            let k = here[c.name.as_str()];
            if k >= 2 {
                let slot = best.entry(c.name.clone()).or_insert_with(|| {
                    order.push(c.name.clone());
                    0
                });
                *slot = (*slot).max(k);
            }
        }
        stack.extend(n.children.iter().rev());
    }
    let mut out: Vec<(String, usize)> = order
        .into_iter()
        .map(|n| {
            let k = best[&n];
            (n, k)
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.1));
    out
}

/// Read an XML source.
pub fn read_xml(bytes: &[u8], element: Option<&str>) -> Result<QTable, QueryError> {
    let root = parse(bytes)?;
    let name = match element.filter(|e| !e.trim().is_empty()) {
        Some(e) => e.trim().to_owned(),
        None => repeating_elements(bytes)
            .into_iter()
            .next()
            .map_or_else(|| root.name.clone(), |(n, _)| n),
    };
    let mut items = Vec::new();
    collect(&root, &name, &mut items);
    if items.is_empty() {
        return Err(QueryError::Source(format!("xml: no element {name}")));
    }
    let mut columns: Vec<String> = Vec::new();
    let mut recs: Vec<Vec<(String, String)>> = Vec::new();
    for it in items {
        let mut fields: Vec<(String, String)> = Vec::new();
        let push = |k: String, v: String, fields: &mut Vec<(String, String)>| {
            if !fields.iter().any(|(n, _)| *n == k) {
                fields.push((k, v));
            }
        };
        for (k, v) in &it.attrs {
            push(k.clone(), v.clone(), &mut fields);
        }
        for c in &it.children {
            if c.children.is_empty() {
                if c.attrs.is_empty() || !c.text.trim().is_empty() {
                    push(c.name.clone(), c.text.trim().to_owned(), &mut fields);
                }
                for (k, v) in &c.attrs {
                    push(format!("{}.{k}", c.name), v.clone(), &mut fields);
                }
            } else {
                for g in &c.children {
                    push(format!("{}.{}", c.name, g.name), g.all_text(), &mut fields);
                }
            }
        }
        if it.children.is_empty() && !it.text.trim().is_empty() {
            push(it.name.clone(), it.text.trim().to_owned(), &mut fields);
        }
        for (k, _) in &fields {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
        recs.push(fields);
    }
    let rows = recs
        .into_iter()
        .map(|fields| {
            let mut row = vec![Val::Null; columns.len()];
            for (k, v) in fields {
                if let Some(c) = columns.iter().position(|x| *x == k) {
                    row[c] = Val::Text(v);
                }
            }
            row
        })
        .collect();
    Ok(QTable::new(columns, rows))
}

fn collect<'a>(n: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    if n.name == name {
        out.push(n);
        return;
    }
    for c in &n.children {
        collect(c, name, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeating_rows() {
        let src = br#"<?xml version="1.0"?>
<orders xmlns="urn:x"><meta><by>me</by></meta>
  <order id="1"><customer>An</customer><total>10.5</total><ship><city>HN</city></ship></order>
  <order id="2"><customer>B&amp;C</customer><note><![CDATA[<x>]]></note></order>
</orders>"#;
        assert_eq!(repeating_elements(src), vec![("order".to_owned(), 2)]);
        let t = read_xml(src, None).unwrap();
        assert_eq!(
            t.columns,
            vec!["id", "customer", "total", "ship.city", "note"]
        );
        assert_eq!(t.rows[1][1], Val::Text("B&C".into()));
        assert_eq!(t.rows[1][4], Val::Text("<x>".into()));
        assert_eq!(t.rows[1][2], Val::Null);
        let t = read_xml(src, Some("by")).unwrap();
        assert_eq!(t.columns, vec!["by"]);
        assert!(read_xml(src, Some("zzz")).is_err());
        assert!(read_xml(b"<a><b>", None).is_err());
    }
}

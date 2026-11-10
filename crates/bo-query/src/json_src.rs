//! JSON sources (RFC 8259).
//!
//! The rows are the items of an array: below `path` (`data.items`, `$.data.items`,
//! `rows[0]`), or the first array found breadth-first. An array of objects gives one column
//! per key (in order of first appearance; nested objects flattened as `parent.child`, nested
//! arrays kept as JSON text); an array of arrays gives `Column1…`; an array of scalars gives
//! one `Value` column. A lone object is one row. Object members keep their file order.

use crate::value::{QTable, Val};
use crate::QueryError;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::collections::VecDeque;
use std::fmt;

/// A JSON value whose objects keep their member order.
#[derive(Debug, Clone, PartialEq)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<J, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<J, E> {
                Ok(J::Bool(b))
            }
            fn visit_i64<E>(self, n: i64) -> Result<J, E> {
                Ok(J::Num(n as f64))
            }
            fn visit_u64<E>(self, n: u64) -> Result<J, E> {
                Ok(J::Num(n as f64))
            }
            fn visit_f64<E>(self, n: f64) -> Result<J, E> {
                Ok(J::Num(n))
            }
            fn visit_str<E>(self, s: &str) -> Result<J, E> {
                Ok(J::Str(s.to_owned()))
            }
            fn visit_string<E>(self, s: String) -> Result<J, E> {
                Ok(J::Str(s))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<J, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element()? {
                    out.push(v);
                }
                Ok(J::Arr(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<J, A::Error> {
                let mut out: Vec<(String, J)> = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, J>()? {
                    // A repeated member: the last one wins, in the place of the first.
                    if let Some(slot) = out.iter_mut().find(|(n, _)| *n == k) {
                        slot.1 = v;
                    } else {
                        out.push((k, v));
                    }
                }
                Ok(J::Obj(out))
            }
        }
        d.deserialize_any(V)
    }
}

impl J {
    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self) -> String {
        match self {
            J::Null => "null".into(),
            J::Bool(b) => b.to_string(),
            J::Num(n) => crate::value::fmt_num(*n),
            J::Str(s) => serde_json::Value::String(s.clone()).to_string(),
            J::Arr(a) => format!("[{}]", a.iter().map(J::text).collect::<Vec<_>>().join(",")),
            J::Obj(m) => format!(
                "{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{}:{}", serde_json::Value::String(k.clone()), v.text()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

fn parse(bytes: &[u8]) -> Result<J, QueryError> {
    let text = crate::text::decode(bytes, None).text;
    serde_json::from_str(&text).map_err(|e| QueryError::Source(format!("json: {e}")))
}

/// Read a JSON source.
pub fn read_json(bytes: &[u8], path: Option<&str>) -> Result<QTable, QueryError> {
    let root = parse(bytes)?;
    let node = match path.filter(|p| !p.trim().is_empty() && p.trim() != "$") {
        Some(p) => {
            at_path(&root, p).ok_or_else(|| QueryError::Source(format!("json: no value at {p}")))?
        }
        None => first_array(&root).unwrap_or(&root),
    };
    Ok(to_table(node))
}

/// Paths of the arrays of a JSON document (for the navigator), breadth-first.
pub fn array_paths(bytes: &[u8]) -> Vec<String> {
    let Ok(root) = parse(bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut queue: VecDeque<(String, &J)> = VecDeque::new();
    queue.push_back((String::new(), &root));
    while let Some((p, v)) = queue.pop_front() {
        match v {
            J::Arr(items) => {
                out.push(if p.is_empty() { "$".into() } else { p.clone() });
                if let Some(first @ J::Obj(_)) = items.first() {
                    queue.push_back((format!("{p}[0]"), first));
                }
            }
            J::Obj(map) => {
                for (k, child) in map {
                    let path = if p.is_empty() {
                        k.clone()
                    } else {
                        format!("{p}.{k}")
                    };
                    queue.push_back((path, child));
                }
            }
            _ => {}
        }
        if out.len() >= 50 {
            break;
        }
    }
    out
}

fn at_path<'a>(root: &'a J, path: &str) -> Option<&'a J> {
    let path = path.trim();
    let path = path.strip_prefix('$').unwrap_or(path);
    let mut cur = root;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        let (name, idx) = match seg.find('[') {
            Some(i) => (&seg[..i], Some(&seg[i..])),
            None => (seg, None),
        };
        if !name.is_empty() {
            cur = cur.get(name)?;
        }
        if let Some(mut rest) = idx {
            while let Some(r) = rest.strip_prefix('[') {
                let end = r.find(']')?;
                let n: usize = r[..end].trim().parse().ok()?;
                cur = match cur {
                    J::Arr(a) => a.get(n)?,
                    _ => return None,
                };
                rest = &r[end + 1..];
            }
        }
    }
    Some(cur)
}

fn first_array(root: &J) -> Option<&J> {
    let mut queue = VecDeque::from([root]);
    while let Some(v) = queue.pop_front() {
        match v {
            J::Arr(_) => return Some(v),
            J::Obj(map) => queue.extend(map.iter().map(|(_, v)| v)),
            _ => {}
        }
    }
    None
}

fn scalar(v: &J) -> Val {
    match v {
        J::Null => Val::Null,
        J::Bool(b) => Val::Bool(*b),
        J::Num(n) => Val::Num(*n),
        J::Str(s) => Val::Text(s.clone()),
        J::Arr(_) | J::Obj(_) => Val::Text(v.text()),
    }
}

fn flatten(prefix: &str, v: &J, out: &mut Vec<(String, Val)>) {
    match v {
        J::Obj(map) if !map.is_empty() => {
            for (k, child) in map {
                let name = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&name, child, out);
            }
        }
        _ => out.push((prefix.to_owned(), scalar(v))),
    }
}

fn to_table(node: &J) -> QTable {
    let items: Vec<&J> = match node {
        J::Arr(items) => items.iter().collect(),
        other => vec![other],
    };
    if !items.is_empty() && items.iter().all(|i| matches!(i, J::Arr(_))) {
        let rows = items
            .iter()
            .map(|i| match i {
                J::Arr(a) => a.iter().map(scalar).collect(),
                _ => Vec::new(),
            })
            .collect();
        return QTable::from_rows(rows);
    }
    if !items.iter().any(|i| matches!(i, J::Obj(_))) {
        return QTable::new(
            vec!["Value".into()],
            items.iter().map(|i| vec![scalar(i)]).collect(),
        );
    }
    let mut columns: Vec<String> = Vec::new();
    let mut recs: Vec<Vec<(String, Val)>> = Vec::with_capacity(items.len());
    for i in items {
        let mut fields = Vec::new();
        if matches!(i, J::Obj(_)) {
            flatten("", i, &mut fields);
        } else {
            fields.push(("Value".into(), scalar(i)));
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
                    row[c] = v;
                }
            }
            row
        })
        .collect();
    QTable::new(columns, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_paths_and_shapes() {
        let src = br#"{"meta":{"n":2},"data":{"items":[{"id":1,"name":"A","tag":{"c":"x"},"list":[1,2]},{"id":2,"extra":true}]}}"#;
        let t = read_json(src, None).unwrap();
        assert_eq!(t.columns, vec!["id", "name", "tag.c", "list", "extra"]);
        assert_eq!(t.rows[0][3], Val::Text("[1,2]".into()));
        assert_eq!(
            t.rows[1],
            vec![
                Val::Num(2.0),
                Val::Null,
                Val::Null,
                Val::Null,
                Val::Bool(true)
            ]
        );
        let t = read_json(src, Some("$.data.items")).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert!(read_json(src, Some("nope")).is_err());
        assert_eq!(array_paths(src), vec!["data.items", "data.items[0].list"]);
        let t = read_json(b"[[1,2],[3]]", None).unwrap();
        assert_eq!(t.columns, vec!["Column1", "Column2"]);
        let t = read_json(b"[1,\"a\"]", None).unwrap();
        assert_eq!(t.columns, vec!["Value"]);
        let t = read_json(br#"{"rows":[[1],[2]]}"#, Some("rows[1]")).unwrap();
        assert_eq!(t.rows, vec![vec![Val::Num(2.0)]]);
        assert!(read_json(b"{bad", None).is_err());
    }
}

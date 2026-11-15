//! Incremental updates (ISO 32000-1, §7.5.6): changed and new objects are appended after the
//! original bytes with a cross-reference section (a table, or a stream when the file uses
//! cross-reference streams) whose `/Prev` points at the previous one.
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use std::fmt::Write as _;

fn name(out: &mut Vec<u8>, n: &[u8]) {
    out.push(b'/');
    for &c in n {
        if c.is_ascii_alphanumeric() || b"-_.+*,".contains(&c) {
            out.push(c);
        } else {
            out.extend_from_slice(format!("#{c:02X}").as_bytes());
        }
    }
}

fn real(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_owned()
    }
}

/// Serialise an object (streams get their `/Length` from the content).
pub(crate) fn object(out: &mut Vec<u8>, o: &Object) {
    match o {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Integer(i) => out.extend_from_slice(i.to_string().as_bytes()),
        Object::Real(r) => out.extend_from_slice(real(*r).as_bytes()),
        Object::Name(n) => name(out, n),
        Object::String(s, StringFormat::Hexadecimal) => {
            out.push(b'<');
            for c in s {
                out.extend_from_slice(format!("{c:02X}").as_bytes());
            }
            out.push(b'>');
        }
        Object::String(s, StringFormat::Literal) => {
            out.push(b'(');
            for &c in s {
                match c {
                    b'(' | b')' | b'\\' => {
                        out.push(b'\\');
                        out.push(c);
                    }
                    b'\r' => out.extend_from_slice(b"\\r"),
                    b'\n' => out.extend_from_slice(b"\\n"),
                    _ => out.push(c),
                }
            }
            out.push(b')');
        }
        Object::Array(a) => {
            out.push(b'[');
            for (i, v) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                object(out, v);
            }
            out.push(b']');
        }
        Object::Dictionary(d) => dict(out, d),
        Object::Stream(s) => {
            let mut d = s.dict.clone();
            d.set("Length", s.content.len() as i64);
            dict(out, &d);
            out.extend_from_slice(b"\nstream\n");
            out.extend_from_slice(&s.content);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Reference((n, g)) => out.extend_from_slice(format!("{n} {g} R").as_bytes()),
    }
}

fn dict(out: &mut Vec<u8>, d: &Dictionary) {
    out.extend_from_slice(b"<<");
    for (k, v) in d {
        name(out, k);
        out.push(b' ');
        object(out, v);
    }
    out.extend_from_slice(b">>");
}

/// Offset of the last cross-reference section (`startxref`).
fn last_startxref(bytes: &[u8]) -> Option<usize> {
    let tail = bytes.len().saturating_sub(2048);
    let p = bytes[tail..].windows(9).rposition(|w| w == b"startxref")? + tail;
    let s: String = bytes[p + 9..]
        .iter()
        .skip_while(|c| c.is_ascii_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .map(|&c| char::from(c))
        .collect();
    s.parse().ok()
}

/// Append an incremental update with `ids` (taken from `doc`) to `original`.
pub(crate) fn incremental(
    original: &[u8],
    doc: &Document,
    ids: &[ObjectId],
) -> Result<Vec<u8>, String> {
    if doc.is_encrypted() {
        return Err("encrypted documents cannot be updated".into());
    }
    let prev = last_startxref(original).ok_or("no startxref in the original file")?;
    let mut out = original.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut ids: Vec<ObjectId> = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let mut offsets = Vec::with_capacity(ids.len());
    for id in &ids {
        let o = doc.get_object(*id).map_err(|e| e.to_string())?;
        offsets.push((*id, out.len()));
        out.extend_from_slice(format!("{} {} obj\n", id.0, id.1).as_bytes());
        object(&mut out, o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let size = doc.max_id.max(ids.iter().map(|i| i.0).max().unwrap_or(0)) + 1;
    let xref_stream = doc
        .trailer
        .get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|t| t == b"XRef");
    let mut trailer = Dictionary::new();
    trailer.set("Size", i64::from(size) + i64::from(xref_stream));
    trailer.set("Prev", prev as i64);
    for k in [&b"Root"[..], b"Info", b"ID"] {
        if let Ok(v) = doc.trailer.get(k) {
            trailer.set(k.to_vec(), v.clone());
        }
    }
    let xref_at = out.len();
    if xref_stream {
        // Cross-reference stream: type 1 entries, offsets in 4 bytes, generation in 2.
        let mut entries = offsets.clone();
        let own = (size, 0_u16);
        entries.push((own, xref_at));
        let mut index = Vec::new();
        let mut data = Vec::new();
        for (id, off) in &entries {
            index.push(Object::Integer(i64::from(id.0)));
            index.push(Object::Integer(1));
            data.push(1_u8);
            data.extend_from_slice(&(*off as u32).to_be_bytes());
            data.extend_from_slice(&id.1.to_be_bytes());
        }
        trailer.set("Type", Object::Name(b"XRef".to_vec()));
        trailer.set("Index", Object::Array(index));
        trailer.set("W", Object::Array(vec![1.into(), 4.into(), 2.into()]));
        let stream = lopdf::Stream::new(trailer, data);
        out.extend_from_slice(format!("{} 0 obj\n", own.0).as_bytes());
        object(&mut out, &Object::Stream(stream));
        out.extend_from_slice(b"\nendobj\n");
    } else {
        out.extend_from_slice(b"xref\n");
        for (id, off) in &offsets {
            let mut line = String::new();
            let _ = write!(line, "{} 1\n{off:010} {:05} n \n", id.0, id.1);
            out.extend_from_slice(line.as_bytes());
        }
        out.extend_from_slice(b"trailer\n");
        dict(&mut out, &trailer);
        out.push(b'\n');
    }
    out.extend_from_slice(format!("startxref\n{xref_at}\n%%EOF\n").as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_objects() {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Contents", Object::string_literal("a (b)"));
        d.set(
            "Rect",
            vec![0.into(), 1.5.into(), Object::Real(-0.0), 3.into()],
        );
        d.set("P", Object::Reference((4, 0)));
        let mut out = Vec::new();
        object(&mut out, &Object::Dictionary(d));
        assert_eq!(
            String::from_utf8_lossy(&out),
            "<</Type /Annot/Contents (a \\(b\\))/Rect [0 1.5 0 3]/P 4 0 R>>"
        );
        assert_eq!(last_startxref(b"%PDF\nstartxref\n1234\n%%EOF"), Some(1234));
    }
}

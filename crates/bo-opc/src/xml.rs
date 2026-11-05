//! Small helpers around `quick-xml` shared by the format crates.
//!
//! All readers built from here enforce a maximum element depth to protect
//! against maliciously nested documents (prompt §5.9).

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Maximum element nesting accepted when parsing untrusted XML.
pub const MAX_DEPTH: usize = 256;

/// Escape text for XML element content / attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Return the (unescaped) value of attribute `local_name` (prefix ignored).
pub fn attr(e: &BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.local_name().as_ref().as_bytes() == local_name {
            let raw = a.value.to_string();
            return Some(unescape(&raw));
        }
    }
    None
}

/// Unescape the five predefined entities and numeric character references.
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
            let ent = &tail[1..end];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = rep {
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

/// A depth-limited pull reader over an in-memory XML buffer.
pub struct XmlReader<'a> {
    inner: Reader<&'a [u8]>,
    depth: usize,
    part: String,
}

impl<'a> XmlReader<'a> {
    /// Create a reader. `part` is used in error messages.
    pub fn new(data: &'a [u8], part: &str) -> Self {
        let mut inner = Reader::from_reader(data);
        inner.config_mut().trim_text(false);
        XmlReader {
            inner,
            depth: 0,
            part: part.to_owned(),
        }
    }

    /// Current element depth (number of open elements).
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Read the next event; `Empty` elements are reported as-is (no matching End).
    pub fn read(&mut self) -> crate::Result<Event<'a>> {
        let ev = self
            .inner
            .read_event()
            .map_err(|e| self.err(&e.to_string()))?;
        match &ev {
            Event::Start(_) => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(self.err("maximum XML nesting depth exceeded"));
                }
            }
            Event::End(_) => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        Ok(ev)
    }

    /// Skip the remainder of the element whose `Start` was just read.
    pub fn skip_element(&mut self) -> crate::Result<()> {
        let target = self.depth - 1;
        loop {
            match self.read()? {
                Event::End(_) if self.depth == target => return Ok(()),
                Event::Eof => return Err(self.err("unexpected end of document")),
                _ => {}
            }
        }
    }

    /// Build an XML error for this part.
    pub fn err(&self, message: &str) -> crate::OpcError {
        crate::OpcError::Xml {
            part: self.part.clone(),
            message: message.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        let s = "a<b>&\"c'";
        assert_eq!(unescape(&escape(s)), s);
        assert_eq!(unescape("&#x1EA0;&#273;"), "Ạđ");
        assert_eq!(unescape("a & b"), "a & b");
    }

    #[test]
    fn depth_limit() {
        let deep = "<a>".repeat(MAX_DEPTH + 1);
        let mut r = XmlReader::new(deep.as_bytes(), "/deep.xml");
        let mut failed = false;
        for _ in 0..=MAX_DEPTH + 1 {
            if r.read().is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed);
    }
}

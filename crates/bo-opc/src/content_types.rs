//! `[Content_Types].xml` — ECMA-376 Part 2, §10.1.2 Mapping Content Types.

use crate::xml::{attr, escape, XmlReader};
use crate::Result;
use quick_xml::events::Event;

/// Parsed content-type stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentTypes {
    /// `Default` elements: (extension lower-case, content type).
    pub defaults: Vec<(String, String)>,
    /// `Override` elements: (part name, content type).
    pub overrides: Vec<(String, String)>,
}

impl ContentTypes {
    /// Parse the XML stream.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = XmlReader::new(data, "/[Content_Types].xml");
        let mut ct = ContentTypes::default();
        loop {
            match r.next()? {
                Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref().as_bytes() {
                    // §10.1.2.2.2 Default
                    b"Default" => {
                        if let (Some(ext), Some(t)) = (attr(&e, b"Extension"), attr(&e, b"ContentType")) {
                            ct.defaults.push((ext.to_ascii_lowercase(), t));
                        }
                    }
                    // §10.1.2.2.3 Override
                    b"Override" => {
                        if let (Some(p), Some(t)) = (attr(&e, b"PartName"), attr(&e, b"ContentType")) {
                            ct.overrides.push((p, t));
                        }
                    }
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(ct)
    }

    /// Content type of `part_name` (overrides win over defaults; comparison is ASCII case-insensitive, §9.1.1.1).
    pub fn get(&self, part_name: &str) -> Option<&str> {
        if let Some((_, t)) = self
            .overrides
            .iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(part_name))
        {
            return Some(t);
        }
        let ext = part_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
        self.defaults.iter().find(|(e, _)| *e == ext).map(|(_, t)| t.as_str())
    }

    /// Set an override for `part_name`.
    pub fn set_override(&mut self, part_name: &str, content_type: &str) {
        if let Some(o) = self
            .overrides
            .iter_mut()
            .find(|(p, _)| p.eq_ignore_ascii_case(part_name))
        {
            o.1 = content_type.to_owned();
        } else {
            self.overrides.push((part_name.to_owned(), content_type.to_owned()));
        }
    }

    /// Serialize to XML.
    pub fn to_xml(&self) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
        );
        for (e, t) in &self.defaults {
            s.push_str(&format!(
                "<Default Extension=\"{}\" ContentType=\"{}\"/>",
                escape(e),
                escape(t)
            ));
        }
        for (p, t) in &self.overrides {
            s.push_str(&format!(
                "<Override PartName=\"{}\" ContentType=\"{}\"/>",
                escape(p),
                escape(t)
            ));
        }
        s.push_str("</Types>");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        let xml = br#"<Types xmlns="x"><Default Extension="XML" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="doc"/></Types>"#;
        let ct = ContentTypes::parse(xml).unwrap();
        assert_eq!(ct.get("/word/DOCUMENT.xml"), Some("doc"));
        assert_eq!(ct.get("/word/styles.xml"), Some("application/xml"));
        assert_eq!(ct.get("/a.bin"), None);
        let again = ContentTypes::parse(ct.to_xml().as_bytes()).unwrap();
        assert_eq!(again, ct);
    }
}

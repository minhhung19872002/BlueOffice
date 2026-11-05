//! Relationships — ECMA-376 Part 2, §9.3.

use crate::xml::{attr, escape, XmlReader};
use crate::Result;
use quick_xml::events::Event;

/// Target mode (§9.3.2.2 `TargetMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetMode {
    #[default]
    Internal,
    External,
}

/// One `Relationship` element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub mode: TargetMode,
}

/// Name of the relationships part for `source` (`""` or `"/"` = package).
///
/// `/word/document.xml` → `/word/_rels/document.xml.rels` (§9.3.3.1).
pub fn rels_part_name(source: &str) -> String {
    if source.is_empty() || source == "/" {
        return "/_rels/.rels".to_owned();
    }
    let (dir, file) = source.rsplit_once('/').unwrap_or(("", source));
    format!("{dir}/_rels/{file}.rels")
}

/// Resolve a relative internal target against the source part (§9.3.4 / RFC 3986).
pub fn resolve_target(source: &str, target: &str) -> String {
    if target.starts_with('/') {
        return normalize(target);
    }
    let base_dir = if source.is_empty() || source == "/" {
        ""
    } else {
        source.rsplit_once('/').map_or("", |(d, _)| d)
    };
    normalize(&format!("{base_dir}/{target}"))
}

fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
}

/// Parse a relationships part.
pub fn parse(data: &[u8], part: &str) -> Result<Vec<Relationship>> {
    let mut r = XmlReader::new(data, part);
    let mut v = Vec::new();
    loop {
        match r.next()? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref().as_bytes() == b"Relationship" => {
                let (Some(id), Some(rel_type), Some(target)) =
                    (attr(&e, b"Id"), attr(&e, b"Type"), attr(&e, b"Target"))
                else {
                    return Err(r.err("Relationship missing Id/Type/Target"));
                };
                let mode = match attr(&e, b"TargetMode").as_deref() {
                    Some("External") => TargetMode::External,
                    _ => TargetMode::Internal,
                };
                v.push(Relationship { id, rel_type, target, mode });
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(v)
}

/// Serialize relationships to XML.
pub fn to_xml(rels: &[Relationship]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for r in rels {
        s.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
            escape(&r.id),
            escape(&r.rel_type),
            escape(&r.target),
            if r.mode == TargetMode::External { " TargetMode=\"External\"" } else { "" }
        ));
    }
    s.push_str("</Relationships>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_resolution() {
        assert_eq!(rels_part_name("/"), "/_rels/.rels");
        assert_eq!(rels_part_name("/word/document.xml"), "/word/_rels/document.xml.rels");
        assert_eq!(resolve_target("/", "word/document.xml"), "/word/document.xml");
        assert_eq!(resolve_target("/word/document.xml", "styles.xml"), "/word/styles.xml");
        assert_eq!(resolve_target("/word/document.xml", "../customXml/item1.xml"), "/customXml/item1.xml");
        assert_eq!(resolve_target("/word/document.xml", "/word/media/a.png"), "/word/media/a.png");
    }

    #[test]
    fn parse_roundtrip() {
        let rels = vec![
            Relationship { id: "rId1".into(), rel_type: "t".into(), target: "a&b.xml".into(), mode: TargetMode::Internal },
            Relationship { id: "rId2".into(), rel_type: "h".into(), target: "https://x".into(), mode: TargetMode::External },
        ];
        let xml = to_xml(&rels);
        assert_eq!(parse(xml.as_bytes(), "/_rels/.rels").unwrap(), rels);
    }
}

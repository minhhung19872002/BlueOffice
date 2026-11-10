//! Flat OPC: a whole package as one XML file (`pkg:package`).
//!
//! The format is the "Flat OPC" serialisation described in the public Microsoft
//! documentation of the Open XML formats: the root `pkg:package` (namespace
//! `http://schemas.microsoft.com/office/2006/xmlPackage`) holds one `pkg:part` per
//! part of the ECMA-376 Part 2 package, with `pkg:name` (the part name, §8.1.1) and
//! `pkg:contentType` (§8.1.2). An XML part is embedded as the child of `pkg:xmlData`
//! (without its XML declaration); any other part is base64 text in `pkg:binaryData`,
//! broken into lines of 76 characters. The content types stream (§8.1.2.2) does not
//! appear: it is rebuilt from the `pkg:contentType` attributes.
//!
//! Converting package → Flat OPC → package keeps every part: binary parts byte for
//! byte, XML parts up to their XML declaration and the white space around the root
//! element.

use crate::content_types::ContentTypes;
use crate::package::{Limits, Package, CONTENT_TYPES};
use crate::xml::{escape, XmlReader};
use crate::{OpcError, Result};
use base64::Engine as _;
use quick_xml::events::Event;
use std::borrow::Cow;

/// Namespace of the Flat OPC elements.
pub const NS_PKG: &str = "http://schemas.microsoft.com/office/2006/xmlPackage";
/// Content type of relationship parts (ECMA-376 Part 2, §8.5.1).
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
/// Declaration given to XML parts taken out of `pkg:xmlData`.
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";
/// Base64 line length of `pkg:binaryData`.
const LINE: usize = 76;

/// The bytes are a Flat OPC document: an XML file whose root element is `package` in
/// the Flat OPC namespace (looked for in the first 4 KiB).
pub fn is_flat_opc(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"PK") {
        return false;
    }
    let head = decode_xml(&bytes[..bytes.len().min(4096)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if !head.starts_with('<') || !head.contains(NS_PKG) {
        return false;
    }
    // The first element (after the declaration, processing instructions and comments).
    let mut rest = head;
    loop {
        let Some(at) = rest.find('<') else {
            return false;
        };
        rest = &rest[at + 1..];
        match rest.chars().next() {
            Some('?' | '!') => {}
            Some(_) => {
                let name: &str = rest
                    .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .next()
                    .unwrap_or("");
                return name.rsplit(':').next() == Some("package");
            }
            None => return false,
        }
    }
}

/// XML bytes as UTF-8 text: UTF-16 with a byte order mark is transcoded, a UTF-8
/// byte order mark is dropped, anything else is read as UTF-8 (lossy).
fn decode_xml(bytes: &[u8]) -> Cow<'_, str> {
    let utf16 = |be: bool, b: &[u8]| -> String {
        let units = b.chunks_exact(2).map(|c| {
            if be {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(units)
            .map(|r| r.unwrap_or('\u{fffd}'))
            .collect()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        Cow::Owned(utf16(false, rest))
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        Cow::Owned(utf16(true, rest))
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
    }
}

fn bad(message: &str) -> OpcError {
    OpcError::Xml {
        part: "flat OPC".into(),
        message: message.to_owned(),
    }
}

/// Namespace declarations (`xmlns…="…"` attributes, raw) of a start tag.
fn ns_decls(e: &quick_xml::events::BytesStart<'_>) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .filter_map(|a| {
            let key: &str = a.key.as_ref();
            let prefix = key.strip_prefix("xmlns:")?.to_owned();
            Some((prefix, a.value.to_string()))
        })
        .collect()
}

/// `inner` (the content of `pkg:xmlData`) as a standalone XML part: an XML
/// declaration is added, and namespace prefixes declared on the Flat OPC wrapper
/// elements that the part uses without declaring them are declared on its root.
fn standalone_part(inner: &str, inherited: &[(String, String)]) -> Vec<u8> {
    let inner = inner.trim();
    let mut out = String::with_capacity(inner.len() + XML_DECL.len());
    out.push_str(XML_DECL);
    // The root start tag: the first `<` followed by a name character.
    let root = inner
        .char_indices()
        .find(|&(i, c)| {
            c == '<'
                && inner[i + 1..]
                    .chars()
                    .next()
                    .is_some_and(|n| n != '?' && n != '!')
        })
        .map(|(i, _)| i);
    let Some(root) = root else {
        out.push_str(inner);
        return out.into_bytes();
    };
    let tag_end = inner[root..].find('>').map_or(inner.len(), |k| root + k);
    let tag = &inner[root..tag_end];
    let name_end = tag
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(tag.len());
    let mut extra = String::new();
    for (prefix, uri) in inherited {
        let declared = tag.contains(&format!("xmlns:{prefix}="));
        let used = inner.contains(&format!("<{prefix}:")) || inner.contains(&format!(" {prefix}:"));
        if !declared && used {
            extra.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
        }
    }
    out.push_str(&inner[..root + name_end]);
    out.push_str(&extra);
    out.push_str(&inner[root + name_end..]);
    out.into_bytes()
}

/// Read a Flat OPC document into a package. The size limits of [`Limits`] apply to
/// the decoded parts.
pub fn read_flat(bytes: &[u8], limits: &Limits) -> Result<Package> {
    if bytes.len() as u64 > limits.max_total_size.saturating_mul(2) {
        return Err(OpcError::LimitExceeded("flat OPC document size".into()));
    }
    let text = decode_xml(bytes);
    let data = text.as_bytes();
    let mut r = XmlReader::new(data, "flat OPC");
    // (name, content type, bytes) in document order.
    let mut parts: Vec<(String, String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    let mut seen_root = false;
    let mut root_ns: Vec<(String, String)> = Vec::new();
    loop {
        match r.read()? {
            Event::Eof => break,
            Event::Start(e) if !seen_root => {
                if e.local_name().as_ref() != "package" {
                    return Err(bad("root element is not pkg:package"));
                }
                seen_root = true;
                root_ns = ns_decls(&e);
            }
            Event::Empty(e) if !seen_root => {
                if e.local_name().as_ref() != "package" {
                    return Err(bad("root element is not pkg:package"));
                }
                seen_root = true;
            }
            Event::Start(e) if r.depth() == 2 && e.local_name().as_ref() == "part" => {
                let (name, content_type) = part_header(&e)?;
                let mut ns = root_ns.clone();
                ns.extend(ns_decls(&e));
                let body = read_part_body(&mut r, &ns, limits)?;
                total += body.len() as u64;
                if total > limits.max_total_size {
                    return Err(OpcError::LimitExceeded("total part size".into()));
                }
                add_part(&mut parts, name, content_type, body, limits)?;
            }
            // An empty part.
            Event::Empty(e) if r.depth() == 1 && e.local_name().as_ref() == "part" => {
                let (name, content_type) = part_header(&e)?;
                add_part(&mut parts, name, content_type, Vec::new(), limits)?;
            }
            _ => {}
        }
    }
    if !seen_root {
        return Err(bad("no pkg:package element"));
    }
    Ok(build_package(parts))
}

/// Name and content type of a `pkg:part` start tag.
fn part_header(e: &quick_xml::events::BytesStart<'_>) -> Result<(String, String)> {
    let name = crate::xml::attr(e, b"name").ok_or_else(|| bad("part without name"))?;
    let content_type = crate::xml::attr(e, b"contentType").unwrap_or_default();
    if !name.starts_with('/')
        || name.contains('\\')
        || name.split('/').any(|s| s == "..")
        || name.eq_ignore_ascii_case(CONTENT_TYPES)
    {
        return Err(OpcError::InvalidPartName(name));
    }
    Ok((name, content_type))
}

/// Append a part; a repeated part name is invalid (§8.1.1.1) and the first one is kept.
fn add_part(
    parts: &mut Vec<(String, String, Vec<u8>)>,
    name: String,
    content_type: String,
    body: Vec<u8>,
    limits: &Limits,
) -> Result<()> {
    if parts.len() >= limits.max_entries {
        return Err(OpcError::LimitExceeded("number of parts".into()));
    }
    if !parts.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(&name)) {
        parts.push((name, content_type, body));
    }
    Ok(())
}

/// Content of a `pkg:part` whose start tag was just read: `pkg:xmlData` or
/// `pkg:binaryData` (an empty part when neither is present).
fn read_part_body(
    r: &mut XmlReader<'_>,
    inherited: &[(String, String)],
    limits: &Limits,
) -> Result<Vec<u8>> {
    let part_depth = r.depth();
    let mut body = Vec::new();
    loop {
        match r.read()? {
            Event::Eof => return Err(bad("unexpected end of document")),
            Event::End(_) if r.depth() < part_depth => return Ok(body),
            Event::Start(e)
                if r.depth() == part_depth + 1 && e.local_name().as_ref() == "xmlData" =>
            {
                let mut ns = inherited.to_vec();
                ns.extend(ns_decls(&e));
                let start = r.pos();
                let end = loop {
                    let before = r.pos();
                    match r.read()? {
                        Event::Eof => return Err(bad("unexpected end of document")),
                        Event::End(_) if r.depth() == part_depth => break before,
                        _ => {}
                    }
                };
                if (end.saturating_sub(start)) as u64 > limits.max_part_size {
                    return Err(OpcError::LimitExceeded("XML part size".into()));
                }
                body = standalone_part(&r.slice(start, end), &ns);
            }
            Event::Start(e)
                if r.depth() == part_depth + 1 && e.local_name().as_ref() == "binaryData" =>
            {
                let mut b64 = Vec::new();
                loop {
                    match r.read()? {
                        Event::Eof => return Err(bad("unexpected end of document")),
                        Event::End(_) if r.depth() == part_depth => break,
                        Event::Text(t) => b64.extend(
                            t.xml10_content()
                                .bytes()
                                .filter(|b| !b.is_ascii_whitespace()),
                        ),
                        Event::CData(t) => b64.extend(
                            t.xml10_content()
                                .bytes()
                                .filter(|b| !b.is_ascii_whitespace()),
                        ),
                        _ => {}
                    }
                    if b64.len() as u64 / 4 * 3 > limits.max_part_size {
                        return Err(OpcError::LimitExceeded("binary part size".into()));
                    }
                }
                // Padding is optional for readers.
                while b64.last() == Some(&b'=') {
                    b64.pop();
                }
                body = base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(&b64)
                    .map_err(|e| bad(&format!("invalid base64 in pkg:binaryData: {e}")))?;
            }
            _ => {}
        }
    }
}

/// Assemble the package: the content types stream is derived from the parts.
fn build_package(parts: Vec<(String, String, Vec<u8>)>) -> Package {
    let ext_of = |name: &str| -> Option<String> {
        let file = name.rsplit('/').next().unwrap_or(name);
        file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())
    };
    let mut ct = ContentTypes {
        defaults: vec![
            ("rels".into(), CT_RELS.into()),
            ("xml".into(), "application/xml".into()),
        ],
        overrides: Vec::new(),
    };
    // A Default for every other extension whose parts agree on one content type.
    for (name, ty, _) in &parts {
        let Some(ext) = ext_of(name) else { continue };
        if ty.is_empty() || ct.defaults.iter().any(|(e, _)| *e == ext) {
            continue;
        }
        let uniform = parts
            .iter()
            .filter(|(n, _, _)| ext_of(n).as_deref() == Some(ext.as_str()))
            .all(|(_, t, _)| t == ty);
        if uniform {
            ct.defaults.push((ext, ty.clone()));
        }
    }
    for (name, ty, _) in &parts {
        if ty.is_empty() {
            continue;
        }
        let default = ext_of(name).and_then(|ext| {
            ct.defaults
                .iter()
                .find(|(e, _)| *e == ext)
                .map(|(_, t)| t.clone())
        });
        if default.as_deref() != Some(ty.as_str()) {
            ct.overrides.push((name.clone(), ty.clone()));
        }
    }
    let mut pkg = Package::new();
    pkg.set_content_types(&ct);
    for (name, _, data) in parts {
        pkg.put_part(&name, data);
    }
    pkg
}

/// The part can be embedded in `pkg:xmlData`: an XML content type and a well-formed
/// document without a document type declaration. Returns the text after the XML
/// declaration.
fn embeddable_xml(content_type: &str, data: &[u8]) -> Option<String> {
    let ct = content_type.to_ascii_lowercase();
    if !(ct.ends_with("+xml") || ct == "application/xml" || ct == "text/xml") {
        return None;
    }
    let text = decode_xml(data);
    let mut body = text.trim_start_matches('\u{feff}').trim();
    if body.starts_with("<?xml") {
        let end = body.find("?>")?;
        body = body[end + 2..].trim_start();
    }
    if body.is_empty() || body.contains("<!DOCTYPE") {
        return None;
    }
    // Well formed, one root element, nothing but white space, comments and
    // processing instructions around it.
    let mut r = XmlReader::new(body.as_bytes(), "part");
    let mut roots = 0;
    let mut open: Vec<String> = Vec::new();
    loop {
        match r.read().ok()? {
            Event::Eof => break,
            Event::Start(e) => {
                if open.is_empty() {
                    roots += 1;
                }
                open.push(e.name().as_ref().to_owned());
            }
            Event::Empty(_) => {
                if open.is_empty() {
                    roots += 1;
                }
            }
            Event::End(e) => {
                let name = e.name();
                let name: &str = name.as_ref();
                if open.pop().as_deref() != Some(name) {
                    return None;
                }
            }
            Event::Text(t) => {
                if open.is_empty() && !t.xml10_content().trim().is_empty() {
                    return None;
                }
            }
            Event::CData(_) | Event::GeneralRef(_) if open.is_empty() => return None,
            Event::DocType(_) | Event::Decl(_) => return None,
            _ => {}
        }
    }
    (roots == 1 && open.is_empty()).then(|| body.to_owned())
}

/// Application the Flat OPC file belongs to, by the content type of the main part
/// (the `mso-application` processing instruction lets a shell pick the application).
fn progid(pkg: &Package, ct: &ContentTypes) -> Option<&'static str> {
    let main = pkg
        .related_part("/", crate::REL_OFFICE_DOCUMENT)
        .ok()
        .flatten()?;
    let ty = ct.get(&main)?.to_ascii_lowercase();
    if ty.contains("wordprocessingml") || ty.contains("ms-word") {
        Some("Word.Document")
    } else if ty.contains("spreadsheetml") || ty.contains("ms-excel") {
        Some("Excel.Sheet")
    } else if ty.contains("presentationml") || ty.contains("ms-powerpoint") {
        Some("PowerPoint.Show")
    } else {
        None
    }
}

/// Serialise a package as a Flat OPC document (UTF-8).
pub fn write_flat(pkg: &Package) -> Result<Vec<u8>> {
    let ct = pkg.content_types()?;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n");
    if let Some(id) = progid(pkg, &ct) {
        out.push_str(&format!("<?mso-application progid=\"{id}\"?>\r\n"));
    }
    out.push_str(&format!("<pkg:package xmlns:pkg=\"{NS_PKG}\">"));
    for p in pkg.parts() {
        if p.name.eq_ignore_ascii_case(CONTENT_TYPES) {
            continue;
        }
        let ty = ct.get(&p.name).unwrap_or("application/octet-stream");
        out.push_str(&format!(
            "<pkg:part pkg:name=\"{}\" pkg:contentType=\"{}\"",
            escape(&p.name),
            escape(ty)
        ));
        if let Some(xml) = embeddable_xml(ty, &p.data) {
            out.push_str("><pkg:xmlData>");
            out.push_str(&xml);
            out.push_str("</pkg:xmlData></pkg:part>");
        } else {
            out.push_str(" pkg:compression=\"store\"><pkg:binaryData>");
            let b64 = base64::engine::general_purpose::STANDARD.encode(&p.data);
            for (i, line) in b64.as_bytes().chunks(LINE).enumerate() {
                if i > 0 {
                    out.push_str("\r\n");
                }
                // Base64 text is ASCII.
                out.push_str(std::str::from_utf8(line).unwrap_or_default());
            }
            out.push_str("</pkg:binaryData></pkg:part>");
        }
    }
    out.push_str("</pkg:package>");
    Ok(out.into_bytes())
}

/// ZIP package bytes of a Flat OPC document (`None` when `bytes` are not Flat OPC).
pub fn flat_to_zip(bytes: &[u8]) -> Option<Result<Vec<u8>>> {
    is_flat_opc(bytes).then(|| read_flat(bytes, &Limits::default()).and_then(|p| p.write()))
}

/// Flat OPC document of ZIP package bytes.
pub fn zip_to_flat(bytes: &[u8]) -> Result<Vec<u8>> {
    write_flat(&Package::read(bytes, &Limits::default())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sample() -> Package {
        let mut p = Package::new();
        p.add_part(
            "/word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>Xin ch\xC3\xA0o &amp; t\xE1\xBA\xA1m bi\xE1\xBB\x87t</w:t></w:r></w:p></w:body></w:document>".to_vec(),
        )
        .unwrap();
        p.add_part(
            "/word/media/image1.png",
            "image/png",
            (0..=255u8).cycle().take(1000).collect(),
        )
        .unwrap();
        p.add_part("/word/media/image2.png", "image/png", vec![1, 2, 3])
            .unwrap();
        p.add_part(
            "/docProps/broken.xml",
            "application/xml",
            b"<a><b></a>".to_vec(),
        )
        .unwrap();
        p.set_relationships(
            "/",
            &[crate::Relationship {
                id: "rId1".into(),
                rel_type: crate::REL_OFFICE_DOCUMENT.into(),
                target: "word/document.xml".into(),
                mode: crate::TargetMode::Internal,
            }],
        );
        p
    }

    fn body(data: &[u8]) -> String {
        let s = String::from_utf8_lossy(data);
        let s = s.trim();
        match s.strip_prefix("<?xml") {
            Some(rest) => rest[rest.find("?>").unwrap() + 2..].trim().to_owned(),
            None => s.to_owned(),
        }
    }

    #[test]
    fn round_trip_keeps_every_part() {
        let pkg = sample();
        let flat = write_flat(&pkg).unwrap();
        let text = String::from_utf8(flat.clone()).unwrap();
        assert!(is_flat_opc(&flat));
        assert!(text.contains("<?mso-application progid=\"Word.Document\"?>"));
        assert!(text.contains("pkg:name=\"/word/media/image1.png\" pkg:contentType=\"image/png\""));
        // Base64 lines are 76 characters long.
        let b64 = text.split("<pkg:binaryData>").nth(1).unwrap();
        let b64 = &b64[..b64.find('<').unwrap()];
        let lines: Vec<&str> = b64.split("\r\n").collect();
        assert!(lines.len() > 2);
        assert!(lines[..lines.len() - 1].iter().all(|l| l.len() == 76));

        let back = read_flat(&flat, &Limits::default()).unwrap();
        let ct = back.content_types().unwrap();
        let ct0 = pkg.content_types().unwrap();
        for p in pkg.parts().iter().filter(|p| p.name != CONTENT_TYPES) {
            let q = back
                .part(&p.name)
                .unwrap_or_else(|| panic!("{} lost", p.name));
            assert_eq!(ct.get(&p.name), ct0.get(&p.name), "{}", p.name);
            if p.name.ends_with(".png") || p.name.ends_with("broken.xml") {
                assert_eq!(q.data, p.data, "{}", p.name);
            } else {
                assert_eq!(body(&q.data), body(&p.data), "{}", p.name);
            }
        }
        assert_eq!(back.parts().len(), pkg.parts().len());
        // One Default for the two pictures.
        assert!(ct
            .defaults
            .iter()
            .any(|(e, t)| e == "png" && t == "image/png"));
        // Flat → package → flat is stable.
        assert_eq!(write_flat(&back).unwrap(), flat);
        // Through the ZIP container too.
        let zip = flat_to_zip(&flat).unwrap().unwrap();
        assert_eq!(zip_to_flat(&zip).unwrap(), flat);
        assert!(flat_to_zip(&zip).is_none());
    }

    #[test]
    fn reads_inherited_namespaces_and_utf16() {
        let flat = format!(
            "<?xml version=\"1.0\"?><?mso-application progid=\"Word.Document\"?>\n<p:package xmlns:p=\"{NS_PKG}\" xmlns:w=\"urn:w\"><p:part p:name=\"/a.xml\" p:contentType=\"application/xml\"><p:xmlData>\n<w:a><w:b w:c=\"1\"/></w:a>\n</p:xmlData></p:part><p:part p:name=\"/b.bin\" p:contentType=\"application/x-b\"><p:binaryData>AQID\nBA==</p:binaryData></p:part><p:part p:name=\"/c.bin\" p:contentType=\"application/x-c\"/></p:package>"
        );
        let check = |bytes: &[u8]| {
            assert!(is_flat_opc(bytes));
            let pkg = read_flat(bytes, &Limits::default()).unwrap();
            assert_eq!(
                String::from_utf8_lossy(&pkg.part("/a.xml").unwrap().data),
                format!("{XML_DECL}<w:a xmlns:w=\"urn:w\"><w:b w:c=\"1\"/></w:a>")
            );
            assert_eq!(pkg.part("/b.bin").unwrap().data, vec![1, 2, 3, 4]);
            assert_eq!(pkg.part("/c.bin").unwrap().data, Vec::<u8>::new());
            let ct = pkg.content_types().unwrap();
            assert_eq!(ct.get("/b.bin"), Some("application/x-b"));
            assert_eq!(ct.get("/c.bin"), Some("application/x-c"));
        };
        check(flat.as_bytes());
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(flat.encode_utf16().flat_map(u16::to_le_bytes));
        check(&utf16);
    }

    #[test]
    fn rejects_bad_documents() {
        let l = Limits::default();
        assert!(!is_flat_opc(b"<html/>"));
        assert!(!is_flat_opc(b"PK\x03\x04"));
        assert!(!is_flat_opc(format!("<a xmlns=\"{NS_PKG}\"/>").as_bytes()));
        let wrap =
            |inner: &str| format!("<pkg:package xmlns:pkg=\"{NS_PKG}\">{inner}</pkg:package>");
        for inner in [
            "<pkg:part pkg:name=\"/../x\" pkg:contentType=\"a/b\"/>",
            "<pkg:part pkg:name=\"x\" pkg:contentType=\"a/b\"/>",
            "<pkg:part pkg:name=\"/[Content_Types].xml\" pkg:contentType=\"a/b\"/>",
            "<pkg:part pkg:contentType=\"a/b\"/>",
            "<pkg:part pkg:name=\"/a\" pkg:contentType=\"a/b\"><pkg:binaryData>!!!</pkg:binaryData></pkg:part>",
            "<pkg:part pkg:name=\"/a\" pkg:contentType=\"a/b\"><pkg:xmlData><a>",
        ] {
            assert!(read_flat(wrap(inner).as_bytes(), &l).is_err(), "{inner}");
        }
        let small = Limits {
            max_part_size: 8,
            ..Limits::default()
        };
        let big = wrap("<pkg:part pkg:name=\"/a\" pkg:contentType=\"a/b\"><pkg:binaryData>AAAAAAAAAAAAAAAAAAAAAAAA</pkg:binaryData></pkg:part>");
        assert!(matches!(
            read_flat(big.as_bytes(), &small),
            Err(OpcError::LimitExceeded(_))
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn mutated_flat_never_panics(seed in any::<u64>(), cuts in 1usize..6) {
            let mut data = write_flat(&sample()).unwrap();
            let mut s = seed | 1;
            for _ in 0..cuts {
                s ^= s << 13; s ^= s >> 7; s ^= s << 17;
                let at = (s as usize) % data.len().max(1);
                match s % 4 {
                    0 => data.truncate(at),
                    1 => { if at < data.len() { data[at] ^= 0x55; } }
                    2 => { data.splice(at..at, b"<pkg:part pkg:name=\"/x\"><pkg:xmlData>".iter().copied()); }
                    _ => { let end = (at + 40).min(data.len()); data.drain(at..end); }
                }
            }
            let _ = is_flat_opc(&data);
            let _ = read_flat(&data, &Limits::default());
        }
    }
}

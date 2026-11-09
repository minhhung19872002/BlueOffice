//! Package-level inspection and clean-up (ECMA-376 Part 2, Open Packaging
//! Conventions): document properties (§11 core properties, plus the extended
//! and custom property parts of ECMA-376 Part 1, §15.2.12), custom XML data
//! parts (Part 1, §15.2.4–15.2.5), the VBA project ([MS-OFFMACRO2]), comment
//! parts and embedded objects (Part 1, §15.2.10 embedded package / object).

use super::Category;
use crate::{EngineError, Result};
use bo_opc::{rels_part_name, resolve_target, Limits, Package, TargetMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Part 2, §11.1: core properties relationship.
pub const REL_CORE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
/// Part 1, §15.2.12.3: extended (application) properties.
pub const REL_APP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
/// Part 1, §15.2.12.2: custom properties.
pub const REL_CUSTOM: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties";
/// Part 1, §15.2.4: custom XML data storage.
pub const REL_CUSTOM_XML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
/// [MS-OFFMACRO2] VBA project.
pub const REL_VBA: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
/// Word key map customizations (macro key bindings).
pub const REL_KEYMAP: &str =
    "http://schemas.microsoft.com/office/2006/relationships/keyMapCustomizations";
/// Part 1, §15.2.6 comments (WordprocessingML, legacy PresentationML).
pub const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
/// Part 1, §13.3.3 comment authors (legacy PresentationML).
pub const REL_COMMENT_AUTHORS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";
/// [MS-PPTX] modern comments and authors.
pub const REL_MODERN_COMMENTS: &str =
    "http://schemas.microsoft.com/office/2018/10/relationships/comments";
/// [MS-PPTX] modern comment authors.
pub const REL_MODERN_AUTHORS: &str =
    "http://schemas.microsoft.com/office/2018/10/relationships/authors";
/// [MS-DOCX] people (comment authors).
pub const REL_PEOPLE: &str = "http://schemas.microsoft.com/office/2011/relationships/people";
/// [MS-DOCX] comment extensions.
pub const REL_COMMENTS_EXT: [&str; 3] = [
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended",
    "http://schemas.microsoft.com/office/2016/09/relationships/commentsIds",
    "http://schemas.microsoft.com/office/2018/08/relationships/commentsExtensible",
];

/// Namespaces that mark document server properties / content type schemas.
const SERVER_NS: [&str; 2] = [
    "http://schemas.microsoft.com/office/2006/metadata/properties",
    "http://schemas.microsoft.com/office/2006/metadata/contentType",
];

/// Core / extended properties that identify people or organisations.
const CORE_FIELDS: [&str; 8] = [
    "creator",
    "lastModifiedBy",
    "title",
    "subject",
    "keywords",
    "description",
    "category",
    "contentStatus",
];
const APP_FIELDS: [&str; 3] = ["Company", "Manager", "HyperlinkBase"];

/// What the package holds besides the document model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageScan {
    /// Personal properties found: (property, value).
    pub properties: Vec<(String, String)>,
    /// The title property.
    pub title: String,
    /// Custom properties (`property` elements of `docProps/custom.xml`).
    pub custom_properties: usize,
    /// Names of the custom properties.
    pub custom_names: Vec<String>,
    /// Custom XML data parts other than server properties.
    pub custom_xml: usize,
    /// Custom XML parts holding server properties or content type schemas.
    pub server_properties: usize,
    /// VBA project parts.
    pub macros: usize,
    /// Embedded objects / packages (`…/embeddings/…`).
    pub embedded: usize,
    /// Names of the embedded parts.
    pub embedded_names: Vec<String>,
    /// Comment threads stored in comment parts of a deck.
    pub deck_comments: usize,
    /// Authors of deck comments.
    pub deck_comment_authors: Vec<String>,
    /// Diagram (SmartArt) data parts.
    pub diagrams: usize,
    /// Chart parts.
    pub charts: usize,
    /// Newer chart parts (`chartEx`).
    pub chart_ex: usize,
    /// Audio / video media parts.
    pub media: usize,
}

fn lower(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Inspect the parts of a package (`name`, bytes).
pub fn scan<'a>(parts: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> PackageScan {
    let mut s = PackageScan::default();
    for (name, data) in parts {
        let n = lower(name);
        let file = n.rsplit('/').next().unwrap_or("");
        if n == "/docprops/core.xml" {
            let xml = String::from_utf8_lossy(data);
            for f in CORE_FIELDS {
                if let Some(v) = crate::element_text(&xml, f).filter(|v| !v.trim().is_empty()) {
                    if f == "title" {
                        v.trim().clone_into(&mut s.title);
                    }
                    s.properties.push((f.to_owned(), v.trim().to_owned()));
                }
            }
        } else if n == "/docprops/app.xml" {
            let xml = String::from_utf8_lossy(data);
            for f in APP_FIELDS {
                if let Some(v) = crate::element_text(&xml, f).filter(|v| !v.trim().is_empty()) {
                    s.properties.push((f.to_owned(), v.trim().to_owned()));
                }
            }
        } else if n == "/docprops/custom.xml" {
            let xml = String::from_utf8_lossy(data);
            s.custom_properties = count_elements(&xml, "property");
            s.custom_names = attr_values(&xml, "property", "name");
        } else if n.starts_with("/customxml/")
            && file.starts_with("item")
            && !file.starts_with("itemprops")
            && n.ends_with(".xml")
        {
            let xml = String::from_utf8_lossy(data);
            if SERVER_NS.iter().any(|ns| xml.contains(ns)) {
                s.server_properties += 1;
            } else {
                s.custom_xml += 1;
            }
        } else if file == "vbaproject.bin" {
            s.macros += 1;
        } else if n.contains("/embeddings/") {
            s.embedded += 1;
            s.embedded_names.push(file.to_owned());
        } else if n.starts_with("/ppt/comments/") && n.ends_with(".xml") {
            let xml = String::from_utf8_lossy(data);
            s.deck_comments += count_elements(&xml, "cm");
        } else if n == "/ppt/authors.xml" {
            let xml = String::from_utf8_lossy(data);
            s.deck_comment_authors
                .extend(attr_values(&xml, "author", "name"));
        } else if n == "/ppt/commentauthors.xml" {
            let xml = String::from_utf8_lossy(data);
            s.deck_comment_authors
                .extend(attr_values(&xml, "cmAuthor", "name"));
        } else if n.contains("/diagrams/data") {
            s.diagrams += 1;
        } else if n.contains("/charts/chartex") {
            s.chart_ex += 1;
        } else if n.contains("/charts/chart") && n.ends_with(".xml") {
            s.charts += 1;
        } else if n.contains("/media/")
            && [
                ".mp4", ".m4v", ".webm", ".mov", ".avi", ".wmv", ".mp3", ".wav", ".m4a", ".wma",
                ".ogg",
            ]
            .iter()
            .any(|e| n.ends_with(e))
        {
            s.media += 1;
        }
    }
    s
}

/// Start tags whose local name is `local` (any prefix).
pub fn count_elements(xml: &str, local: &str) -> usize {
    start_tags(xml)
        .filter(|(name, _)| bare(name) == local)
        .count()
}

/// Values of attribute `attr` on elements named `local` (any prefix).
fn attr_values(xml: &str, local: &str, attr: &str) -> Vec<String> {
    start_tags(xml)
        .filter(|(name, _)| bare(name) == local)
        .filter_map(|(_, tag)| attr_in(tag, attr))
        .collect()
}

fn bare(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// (qualified name, whole tag text) of every start / empty tag.
fn start_tags(xml: &str) -> impl Iterator<Item = (&str, &str)> {
    xml.match_indices('<').filter_map(move |(i, _)| {
        let rest = &xml[i + 1..];
        if rest.starts_with(['/', '?', '!']) {
            return None;
        }
        let end = rest.find('>')?;
        let tag = &rest[..end];
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next()?;
        Some((name, tag))
    })
}

/// Value of attribute `attr` on the first element named `local` (any prefix).
pub fn first_attr(xml: &str, local: &str, attr: &str) -> Option<String> {
    start_tags(xml)
        .find(|(name, _)| bare(name) == local)
        .and_then(|(_, tag)| attr_in(tag, attr))
}

/// `xml` with attribute `attr` of the first element named `local` set to `value`
/// (added when missing); unchanged when there is no such element.
pub fn set_first_attr(xml: &str, local: &str, attr: &str, value: &str) -> String {
    let Some((start, tag)) = xml.match_indices('<').find_map(|(i, _)| {
        let rest = &xml[i + 1..];
        let end = rest.find('>')?;
        let tag = &rest[..end];
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next()?;
        (!rest.starts_with(['/', '?', '!']) && bare(name) == local).then_some((i + 1, tag))
    }) else {
        return xml.to_owned();
    };
    let esc = bo_opc::xml::escape(value);
    let mut new_tag = String::new();
    let mut found = false;
    // Rebuild the tag's attributes, replacing `attr`.
    let (body, close) = match tag.strip_suffix('/') {
        Some(b) => (b, "/"),
        None => (tag, ""),
    };
    let name_len = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
    new_tag.push_str(&body[..name_len]);
    let mut rest = &body[name_len..];
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(end) = after[1..].find(q) else { break };
        let val = &after[1..=end];
        if key == attr {
            found = true;
            new_tag.push_str(&format!(" {key}=\"{esc}\""));
        } else {
            new_tag.push_str(&format!(" {key}={q}{val}{q}"));
        }
        rest = &after[end + 2..];
    }
    if !found {
        new_tag.push_str(&format!(" {attr}=\"{esc}\""));
    }
    new_tag.push_str(close);
    let mut out = String::with_capacity(xml.len() + esc.len() + 16);
    out.push_str(&xml[..start]);
    out.push_str(&new_tag);
    out.push_str(&xml[start + tag.len()..]);
    out
}

/// Value of attribute `attr` (exact qualified name) in a tag's text.
pub fn attr_in(tag: &str, attr: &str) -> Option<String> {
    let mut at = 0;
    while let Some(k) = tag[at..].find(attr) {
        let i = at + k;
        let before_ok = i == 0 || tag[..i].ends_with(char::is_whitespace);
        let rest = tag[i + attr.len()..].trim_start();
        if before_ok {
            if let Some(v) = rest.strip_prefix('=') {
                let v = v.trim_start();
                let q = v.chars().next()?;
                if q == '"' || q == '\'' {
                    let body = &v[1..];
                    let end = body.find(q)?;
                    return Some(bo_opc::xml::unescape(&body[..end]));
                }
            }
        }
        at = i + attr.len();
    }
    None
}

/// Package parts to drop on the next save.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scrub {
    /// Clear personal core / extended properties.
    #[serde(default)]
    pub properties: bool,
    /// Remove the custom properties part.
    #[serde(default)]
    pub custom_properties: bool,
    /// Remove custom XML data parts (other than server properties).
    #[serde(default)]
    pub custom_xml: bool,
    /// Remove server property / content type schema custom XML parts.
    #[serde(default)]
    pub server_properties: bool,
    /// Remove the VBA project (the file becomes macro-free).
    #[serde(default)]
    pub macros: bool,
    /// Remove comment parts (decks), or the leftover comment parts and people part
    /// of a document whose comments were removed.
    #[serde(default)]
    pub comments: bool,
}

impl Scrub {
    /// The category is queued.
    pub fn has(&self, c: Category) -> bool {
        match c {
            Category::Properties => self.properties,
            Category::CustomProperties => self.custom_properties,
            Category::CustomXml => self.custom_xml,
            Category::ServerProperties => self.server_properties,
            Category::Macros => self.macros,
            Category::Comments => self.comments,
            _ => false,
        }
    }

    /// Queue (`on`) or keep (`!on`) a package-level category; `false` for others.
    pub fn set(&mut self, c: Category, on: bool) -> bool {
        let f = match c {
            Category::Properties => &mut self.properties,
            Category::CustomProperties => &mut self.custom_properties,
            Category::CustomXml => &mut self.custom_xml,
            Category::ServerProperties => &mut self.server_properties,
            Category::Macros => &mut self.macros,
            Category::Comments => &mut self.comments,
            _ => return false,
        };
        *f = on;
        true
    }

    /// Nothing is queued.
    pub fn is_empty(&self) -> bool {
        *self == Scrub::default()
    }
}

/// Macro-free content type of a macro-enabled main part ([MS-OFFMACRO2]).
fn macro_free(ct: &str) -> Option<&'static str> {
    Some(match ct {
        "application/vnd.ms-word.document.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
        }
        "application/vnd.ms-word.template.macroEnabledTemplate.main+xml" => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml"
        }
        "application/vnd.ms-excel.sheet.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
        }
        "application/vnd.ms-excel.template.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml"
        }
        "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
        }
        "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml"
        }
        "application/vnd.ms-powerpoint.template.macroEnabled.main+xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml"
        }
        _ => return None,
    })
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(opc)`
fn opc(e: bo_opc::OpcError) -> EngineError {
    EngineError::Format(e.to_string())
}

/// Source part of a relationships part (`/_rels/.rels` → `/`), Part 2 §9.3.
fn rels_source(rels: &str) -> Option<String> {
    let (dir, file) = rels.rsplit_once("/_rels/")?;
    let file = file.strip_suffix(".rels")?;
    Some(if dir.is_empty() && file.is_empty() {
        "/".to_owned()
    } else {
        format!("{dir}/{file}")
    })
}

/// Remove an element (`<name …>…</name>` or `<name …/>`, exact qualified name).
fn strip_element(xml: &str, name: &str) -> String {
    let mut out = xml.to_owned();
    loop {
        let open = format!("<{name}");
        let Some(start) = out.match_indices(&open).map(|(i, _)| i).find(|&i| {
            out[i + open.len()..]
                .chars()
                .next()
                .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
        }) else {
            return out;
        };
        let Some(gt) = out[start..].find('>').map(|k| start + k) else {
            return out;
        };
        let end = if out[..gt].ends_with('/') {
            gt + 1
        } else {
            let close = format!("</{name}>");
            match out[gt..].find(&close) {
                Some(k) => gt + k + close.len(),
                None => return out,
            }
        };
        out.replace_range(start..end, "");
    }
}

/// Qualified names used for `local` in `xml` (`dc:creator`, `creator`, …).
fn qualified_names(xml: &str, local: &str) -> BTreeSet<String> {
    start_tags(xml)
        .filter(|(n, _)| bare(n) == local)
        .map(|(n, _)| n.to_owned())
        .collect()
}

/// Core properties with the personal fields removed (dates and revision kept).
fn clean_core(xml: &str) -> String {
    let mut out = xml.to_owned();
    for f in CORE_FIELDS.iter().chain(&["lastPrinted", "identifier"]) {
        for q in qualified_names(xml, f) {
            out = strip_element(&out, &q);
        }
    }
    out
}

/// Extended properties without company, manager and hyperlink base.
fn clean_app(xml: &str) -> String {
    let mut out = xml.to_owned();
    for f in APP_FIELDS {
        for q in qualified_names(xml, f) {
            out = strip_element(&out, &q);
        }
    }
    out
}

/// Apply `s` to saved package bytes: drop the queued parts with their
/// relationships and content type overrides, clear personal properties.
pub fn scrub(bytes: &[u8], s: &Scrub) -> Result<Vec<u8>> {
    if s.is_empty() || !bytes.starts_with(b"PK") {
        return Ok(bytes.to_vec());
    }
    let mut pkg = Package::read(bytes, &Limits::default()).map_err(opc)?;
    // Relationships of every part: (source, rel).
    let mut all_rels = Vec::new();
    for p in pkg.parts() {
        if p.name.ends_with(".rels") {
            if let Some(src) = rels_source(&p.name) {
                all_rels.push((src.clone(), pkg.relationships(&src).map_err(opc)?));
            }
        }
    }
    let want = |rel_type: &str, pred: &dyn Fn(&[u8]) -> bool| -> Vec<String> {
        let mut out = Vec::new();
        for (src, rels) in &all_rels {
            for r in rels {
                if r.rel_type == rel_type && r.mode == TargetMode::Internal {
                    let t = resolve_target(src, &r.target);
                    if pkg.part(&t).is_some_and(|p| pred(&p.data)) {
                        out.push(lower(&t));
                    }
                }
            }
        }
        out
    };
    let any = |_: &[u8]| true;
    let is_server = |d: &[u8]| {
        let x = String::from_utf8_lossy(d);
        SERVER_NS.iter().any(|ns| x.contains(ns))
    };
    let mut remove: BTreeSet<String> = BTreeSet::new();
    if s.custom_properties {
        remove.extend(want(REL_CUSTOM, &any));
    }
    if s.custom_xml {
        remove.extend(want(REL_CUSTOM_XML, &|d| !is_server(d)));
    }
    if s.server_properties {
        remove.extend(want(REL_CUSTOM_XML, &is_server));
    }
    if s.macros {
        remove.extend(want(REL_VBA, &any));
        remove.extend(want(REL_KEYMAP, &any));
    }
    if s.comments {
        for t in [
            REL_COMMENTS,
            REL_COMMENT_AUTHORS,
            REL_MODERN_COMMENTS,
            REL_MODERN_AUTHORS,
            REL_PEOPLE,
        ]
        .iter()
        .chain(REL_COMMENTS_EXT.iter())
        {
            // Worksheet notes are modelled (and written) by the workbook writer.
            remove.extend(want(t, &any).into_iter().filter(|n| !n.starts_with("/xl/")));
        }
    }
    // Parts only reachable from a removed part go too (vbaData.xml, itemProps1.xml, …).
    let mut queue: Vec<String> = remove.iter().cloned().collect();
    while let Some(n) = queue.pop() {
        for (src, rels) in &all_rels {
            if lower(src) != n {
                continue;
            }
            for r in rels.iter().filter(|r| r.mode == TargetMode::Internal) {
                let t = lower(&resolve_target(src, &r.target));
                if remove.insert(t.clone()) {
                    queue.push(t);
                }
            }
        }
    }
    let names: Vec<String> = pkg.parts().iter().map(|p| p.name.clone()).collect();
    for n in &names {
        if remove.contains(&lower(n)) {
            pkg.remove_part(n);
            pkg.remove_part(&rels_part_name(n));
        }
    }
    // Relationships pointing at removed parts.
    for (src, rels) in &all_rels {
        if remove.contains(&lower(src)) {
            continue;
        }
        let kept: Vec<_> = rels
            .iter()
            .filter(|r| {
                r.mode == TargetMode::External
                    || !remove.contains(&lower(&resolve_target(src, &r.target)))
            })
            .cloned()
            .collect();
        if kept.len() != rels.len() {
            pkg.set_relationships(src, &kept);
        }
    }
    let mut ct = pkg.content_types().map_err(opc)?;
    ct.overrides.retain(|(p, _)| !remove.contains(&lower(p)));
    if s.macros {
        for o in &mut ct.overrides {
            if let Some(t) = macro_free(&o.1) {
                t.clone_into(&mut o.1);
            }
        }
    }
    pkg.set_content_types(&ct);
    if s.properties {
        let core = pkg.related_part("/", REL_CORE).map_err(opc)?;
        let app = pkg.related_part("/", REL_APP).map_err(opc)?;
        for (part, clean) in [(core, clean_core as fn(&str) -> String), (app, clean_app)] {
            if let Some(p) = part.and_then(|n| pkg.part(&n).map(|p| (n, p.data.clone()))) {
                let xml = String::from_utf8_lossy(&p.1);
                pkg.put_part(&p.0, clean(&xml).into_bytes());
            }
        }
    }
    pkg.write().map_err(opc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_elements_with_any_form() {
        let x = "<a><dc:creator>Ann</dc:creator><b/><dc:creator/><cp:x>1</cp:x></a>";
        assert_eq!(strip_element(x, "dc:creator"), "<a><b/><cp:x>1</cp:x></a>");
        assert_eq!(
            strip_element("<Company>X</Company><CompanyY/>", "Company"),
            "<CompanyY/>"
        );
    }

    #[test]
    fn sets_attributes() {
        let x = r#"<w:drawing><wp:inline><wp:docPr id="1" name="P 1"/></wp:inline></w:drawing>"#;
        let y = set_first_attr(x, "docPr", "descr", "A <cat>");
        assert_eq!(
            y,
            r#"<w:drawing><wp:inline><wp:docPr id="1" name="P 1" descr="A &lt;cat&gt;"/></wp:inline></w:drawing>"#
        );
        let z = set_first_attr(&y, "docPr", "descr", "Dog");
        assert_eq!(first_attr(&z, "docPr", "descr").as_deref(), Some("Dog"));
        assert_eq!(first_attr(&z, "docPr", "name").as_deref(), Some("P 1"));
        assert_eq!(set_first_attr("<a/>", "docPr", "descr", "x"), "<a/>");
    }

    #[test]
    fn rels_sources() {
        assert_eq!(rels_source("/_rels/.rels").as_deref(), Some("/"));
        assert_eq!(
            rels_source("/word/_rels/document.xml.rels").as_deref(),
            Some("/word/document.xml")
        );
    }

    #[test]
    fn scans_parts() {
        let core = br#"<cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:creator>Ann</dc:creator><cp:lastModifiedBy>Bob</cp:lastModifiedBy><dc:title>T</dc:title></cp:coreProperties>"#;
        let custom = br#"<Properties><property fmtid="{x}" pid="2" name="Client"><vt:lpwstr>A</vt:lpwstr></property></Properties>"#;
        let server = br#"<p:properties xmlns:p="http://schemas.microsoft.com/office/2006/metadata/properties"/>"#;
        let s = scan([
            ("/docProps/core.xml", &core[..]),
            ("/docProps/custom.xml", &custom[..]),
            ("/customXml/item1.xml", &b"<r/>"[..]),
            ("/customXml/itemProps1.xml", &b"<r/>"[..]),
            ("/customXml/item2.xml", &server[..]),
            ("/word/vbaProject.bin", &b""[..]),
            ("/word/embeddings/oleObject1.bin", &b""[..]),
        ]);
        assert_eq!(s.properties.len(), 3);
        assert_eq!(s.title, "T");
        assert_eq!(s.custom_properties, 1);
        assert_eq!(s.custom_names, vec!["Client".to_owned()]);
        assert_eq!((s.custom_xml, s.server_properties), (1, 1));
        assert_eq!((s.macros, s.embedded), (1, 1));
    }

    #[test]
    fn cleans_core() {
        let x = r#"<cp:coreProperties><dc:creator>Ann</dc:creator><cp:lastModifiedBy>B</cp:lastModifiedBy><dcterms:created xsi:type="dcterms:W3CDTF">2020-01-01T00:00:00Z</dcterms:created></cp:coreProperties>"#;
        let c = clean_core(x);
        assert!(!c.contains("Ann") && !c.contains("lastModifiedBy"));
        assert!(c.contains("dcterms:created"));
    }
}

//! Document properties of the File › Info page (CORE-UI-002): core properties
//! (ECMA-376 Part 2 §11, `docProps/core.xml`) and extended properties (ECMA-376
//! Part 1 §22.2, `docProps/app.xml`) of `.docx`, `.xlsx` and `.pptx` packages.
//!
//! The values are read from the package that was opened; edits are kept beside
//! them and written into the saved package (only when something changed, so an
//! untouched file keeps its original parts byte for byte, ADR-0005).

use crate::{EngineError, Result};
use bo_opc::{Limits, Package};
use bo_opc::{Relationship, TargetMode};
use serde::{Deserialize, Serialize};

/// Part 2 §11.1 core properties part.
const CORE_PART: &str = "/docProps/core.xml";
/// Part 1 §22.2 extended properties part.
const APP_PART: &str = "/docProps/app.xml";
/// Part 2 §11.1 / Annex E.
const REL_CORE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
/// Part 1 §15.2.12.3.
const REL_APP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
/// Part 2 §11.2 namespaces.
const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
/// Part 1 §22.2 namespace.
const NS_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

/// Document properties shown and edited on the Info page (Part 2 §11 core
/// properties, Part 1 §22.2 extended properties). Empty = not set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocProperties {
    /// `dc:title`.
    pub title: String,
    /// `dc:subject`.
    pub subject: String,
    /// `dc:creator`.
    pub author: String,
    /// `cp:keywords`.
    pub keywords: String,
    /// `dc:description`.
    pub comments: String,
    /// `cp:category`.
    pub category: String,
    /// `cp:contentStatus`.
    pub status: String,
    /// `Company` (Part 1 §22.2).
    pub company: String,
    /// `Manager` (Part 1 §22.2).
    pub manager: String,
    /// `cp:lastModifiedBy`, read only.
    pub last_modified_by: String,
    /// `dcterms:created`, read only.
    pub created: String,
    /// `dcterms:modified`, read only.
    pub modified: String,
    /// `cp:revision`, read only.
    pub revision: String,
}

/// An editable property: (field, part, namespace, local name, fallback prefix).
type Field = (
    fn(&DocProperties) -> &str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const EDITABLE: [Field; 9] = [
    (|p| &p.title, CORE_PART, NS_DC, "title", "dc"),
    (|p| &p.subject, CORE_PART, NS_DC, "subject", "dc"),
    (|p| &p.author, CORE_PART, NS_DC, "creator", "dc"),
    (|p| &p.keywords, CORE_PART, NS_CP, "keywords", "cp"),
    (|p| &p.comments, CORE_PART, NS_DC, "description", "dc"),
    (|p| &p.category, CORE_PART, NS_CP, "category", "cp"),
    (|p| &p.status, CORE_PART, NS_CP, "contentStatus", "cp"),
    (|p| &p.company, APP_PART, NS_APP, "Company", ""),
    (|p| &p.manager, APP_PART, NS_APP, "Manager", ""),
];

impl DocProperties {
    /// Properties of a package (default when it is not an OPC package).
    pub fn read(bytes: &[u8]) -> DocProperties {
        let Ok(pkg) = Package::read(bytes, &Limits::default()) else {
            return DocProperties::default();
        };
        let text = |part: &str| {
            pkg.part(part)
                .map(|p| String::from_utf8_lossy(&p.data).into_owned())
                .unwrap_or_default()
        };
        let core = text(CORE_PART);
        let app = text(APP_PART);
        let get = |xml: &str, local: &str| {
            crate::element_text(xml, local)
                .map(|v| v.trim().to_owned())
                .unwrap_or_default()
        };
        DocProperties {
            title: get(&core, "title"),
            subject: get(&core, "subject"),
            author: get(&core, "creator"),
            keywords: get(&core, "keywords"),
            comments: get(&core, "description"),
            category: get(&core, "category"),
            status: get(&core, "contentStatus"),
            company: get(&app, "Company"),
            manager: get(&app, "Manager"),
            last_modified_by: get(&core, "lastModifiedBy"),
            created: get(&core, "created"),
            modified: get(&core, "modified"),
            revision: get(&core, "revision"),
        }
    }

    /// `self` with the fields `edit` sets (trimmed); read-only fields kept.
    #[must_use]
    pub fn with_edits(&self, edit: &PropsEdit) -> DocProperties {
        let t = |e: &Option<String>, old: &str| {
            e.as_deref()
                .map_or_else(|| old.to_owned(), |s| s.trim().to_owned())
        };
        DocProperties {
            title: t(&edit.title, &self.title),
            subject: t(&edit.subject, &self.subject),
            author: t(&edit.author, &self.author),
            keywords: t(&edit.keywords, &self.keywords),
            comments: t(&edit.comments, &self.comments),
            category: t(&edit.category, &self.category),
            status: t(&edit.status, &self.status),
            company: t(&edit.company, &self.company),
            manager: t(&edit.manager, &self.manager),
            ..self.clone()
        }
    }
}

/// Editable properties to change (`setDocProperties`); absent = unchanged,
/// empty = removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropsEdit {
    /// New `title`.
    pub title: Option<String>,
    /// New `subject`.
    pub subject: Option<String>,
    /// New `author`.
    pub author: Option<String>,
    /// New `keywords`.
    pub keywords: Option<String>,
    /// New `comments`.
    pub comments: Option<String>,
    /// New `category`.
    pub category: Option<String>,
    /// New `status`.
    pub status: Option<String>,
    /// New `company`.
    pub company: Option<String>,
    /// New `manager`.
    pub manager: Option<String>,
}

/// Properties of the open file: as read, and as edited.
#[derive(Debug, Clone, Default)]
pub struct PropsState {
    original: DocProperties,
    current: DocProperties,
}

impl PropsState {
    /// The properties of the package just opened.
    pub fn read(bytes: &[u8]) -> PropsState {
        let p = DocProperties::read(bytes);
        PropsState {
            original: p.clone(),
            current: p,
        }
    }

    /// Current values.
    pub fn get(&self) -> &DocProperties {
        &self.current
    }

    /// Change the editable values `edit` sets.
    pub fn set(&mut self, edit: &PropsEdit) {
        self.current = self.current.with_edits(edit);
    }

    /// Write the edited properties into saved package bytes (unchanged when
    /// nothing was edited).
    pub fn apply(&self, bytes: Vec<u8>) -> Result<Vec<u8>> {
        let changed: Vec<&Field> = EDITABLE
            .iter()
            .filter(|f| (f.0)(&self.original) != (f.0)(&self.current))
            .collect();
        if changed.is_empty() || !bytes.starts_with(b"PK") {
            return Ok(bytes);
        }
        let opc = |e: bo_opc::OpcError| EngineError::Format(e.to_string());
        let mut pkg = Package::read(&bytes, &Limits::default()).map_err(opc)?;
        for part in [CORE_PART, APP_PART] {
            let fields: Vec<&&Field> = changed.iter().filter(|f| f.1 == part).collect();
            if fields.is_empty() {
                continue;
            }
            let (rel, ct, root) = if part == CORE_PART {
                (
                    REL_CORE,
                    CT_CORE,
                    format!(
                        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="{NS_CP}" xmlns:dc="{NS_DC}" xmlns:dcterms="{NS_DCTERMS}"></cp:coreProperties>"#
                    ),
                )
            } else {
                (
                    REL_APP,
                    CT_APP,
                    format!(
                        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="{NS_APP}"></Properties>"#
                    ),
                )
            };
            // The part the package relationship names, else the usual name (created).
            let name = pkg
                .related_part("/", rel)
                .map_err(opc)?
                .unwrap_or_else(|| part.to_owned());
            let existing = pkg
                .part(&name)
                .map(|p| String::from_utf8_lossy(&p.data).into_owned());
            let mut xml = existing.clone().unwrap_or(root);
            for f in fields {
                xml = set_element(&xml, f.2, f.3, f.4, (f.0)(&self.current));
            }
            if existing.is_some() {
                pkg.put_part(&name, xml.into_bytes());
            } else {
                pkg.add_part(&name, ct, xml.into_bytes()).map_err(opc)?;
                let mut rels = pkg.relationships("/").map_err(opc)?;
                let mut n = rels.len() + 1;
                while rels.iter().any(|r| r.id == format!("rId{n}")) {
                    n += 1;
                }
                rels.push(Relationship {
                    id: format!("rId{n}"),
                    rel_type: rel.to_owned(),
                    target: name.trim_start_matches('/').to_owned(),
                    mode: TargetMode::Internal,
                });
                pkg.set_relationships("/", &rels);
            }
        }
        pkg.write().map_err(opc)
    }
}

impl crate::Engine {
    /// BlueDoc `docProperties` / `setDocProperties` commands.
    pub(crate) fn props_command(
        &mut self,
        cmd: &crate::edit::Command,
    ) -> Result<Option<crate::edit::EditResult>> {
        use crate::edit::Command;
        match cmd {
            Command::DocProperties => {}
            Command::SetDocProperties { props } => self.props.set(props),
            _ => return Ok(None),
        }
        if self.session.is_none() {
            return Err(EngineError::NoDocument);
        }
        let mut r = self.query_result()?;
        r.doc_properties = Some(Box::new(self.props.get().clone()));
        Ok(Some(r))
    }

    /// Document properties of the open document or presentation.
    pub fn doc_properties(&self) -> &DocProperties {
        self.props.get()
    }
}

/// A start or end tag found in `xml`: byte range of the whole tag and its name.
struct Tag<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    closing: bool,
    empty: bool,
}

fn tags(xml: &str) -> impl Iterator<Item = Tag<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || loop {
        let k = xml[at..].find('<')? + at;
        let end = xml[k..].find('>')? + k + 1;
        at = end;
        let inner = &xml[k + 1..end - 1];
        if inner.starts_with('?') || inner.starts_with('!') {
            continue;
        }
        let closing = inner.starts_with('/');
        let name = inner
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        return Some(Tag {
            start: k,
            end,
            name,
            closing,
            empty: inner.ends_with('/'),
        });
    })
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Prefix bound to `ns` on the root tag (`Some("")` = default namespace).
fn prefix_of(root: &str, ns: &str) -> Option<String> {
    let mut rest = root;
    while let Some(i) = rest.find("xmlns") {
        let after = &rest[i + 5..];
        let (prefix, tail) = match after.strip_prefix(':') {
            Some(t) => {
                let e = t.find('=')?;
                (t[..e].trim().to_owned(), &t[e..])
            }
            None => (String::new(), after),
        };
        let tail = tail.trim_start().strip_prefix('=')?.trim_start();
        let q = tail.chars().next()?;
        let body = &tail[1..];
        let close = body.find(q)?;
        if &body[..close] == ns {
            return Some(prefix);
        }
        rest = &body[close..];
    }
    None
}

/// `xml` with the first element `local_name` (any prefix) holding `value`:
/// replaced, removed when `value` is empty, or added at the end of the root.
fn set_element(xml: &str, ns: &str, local_name: &str, fallback: &str, value: &str) -> String {
    let all: Vec<Tag<'_>> = tags(xml).collect();
    let text = bo_opc::xml::escape(value);
    if let Some(i) = all
        .iter()
        .position(|t| !t.closing && local(t.name) == local_name)
    {
        let open = &all[i];
        let qname = open.name;
        let end = if open.empty {
            open.end
        } else {
            let mut depth = 0usize;
            let mut close = None;
            for t in &all[i + 1..] {
                if t.name == qname && !t.empty {
                    if t.closing {
                        if depth == 0 {
                            close = Some(t.end);
                            break;
                        }
                        depth -= 1;
                    } else {
                        depth += 1;
                    }
                }
            }
            match close {
                Some(e) => e,
                None => return xml.to_owned(),
            }
        };
        let mut out = String::with_capacity(xml.len() + text.len());
        out.push_str(&xml[..open.start]);
        if !value.is_empty() {
            // Keep the start tag's attributes (xsi:type of dcterms dates, …).
            let start_tag = &xml[open.start..open.end];
            let start_tag = if open.empty {
                format!("{}>", start_tag.trim_end_matches('>').trim_end_matches('/'))
            } else {
                start_tag.to_owned()
            };
            out.push_str(&start_tag);
            out.push_str(&text);
            out.push_str(&format!("</{qname}>"));
        }
        out.push_str(&xml[end..]);
        return out;
    }
    if value.is_empty() {
        return xml.to_owned();
    }
    let Some(root) = all.iter().find(|t| !t.closing) else {
        return xml.to_owned();
    };
    let root_tag = &xml[root.start..root.end];
    let (prefix, declare) = match prefix_of(root_tag, ns) {
        Some(p) => (p, None),
        None => (fallback.to_owned(), Some(fallback)),
    };
    let qname = if prefix.is_empty() {
        local_name.to_owned()
    } else {
        format!("{prefix}:{local_name}")
    };
    let element = format!("<{qname}>{text}</{qname}>");
    let mut new_root = root_tag.trim_end_matches('>').to_owned();
    let empty_root = new_root.ends_with('/');
    if empty_root {
        new_root.pop();
    }
    if let Some(p) = declare {
        if p.is_empty() {
            new_root.push_str(&format!(r#" xmlns="{ns}""#));
        } else {
            new_root.push_str(&format!(r#" xmlns:{p}="{ns}""#));
        }
    }
    new_root.push('>');
    let mut out = String::with_capacity(xml.len() + element.len() + 64);
    out.push_str(&xml[..root.start]);
    out.push_str(&new_root);
    if empty_root {
        out.push_str(&element);
        out.push_str(&format!("</{}>", root.name));
        out.push_str(&xml[root.end..]);
        return out;
    }
    // Insert before the root's end tag.
    let close = all
        .iter()
        .rev()
        .find(|t| t.closing && t.name == root.name)
        .map_or(xml.len(), |t| t.start);
    out.push_str(&xml[root.end..close]);
    out.push_str(&element);
    out.push_str(&xml[close..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: &str = r#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Old</dc:title><dc:creator>A</dc:creator></cp:coreProperties>"#;

    #[test]
    fn replaces_removes_and_adds_elements() {
        let x = set_element(CORE, NS_DC, "title", "dc", "New & <better>");
        assert!(x.contains("<dc:title>New &amp; &lt;better&gt;</dc:title>"));
        let x = set_element(&x, NS_DC, "creator", "dc", "");
        assert!(!x.contains("creator"));
        let x = set_element(&x, NS_CP, "keywords", "cp", "k1; k2");
        assert!(x.ends_with("<cp:keywords>k1; k2</cp:keywords></cp:coreProperties>"));
        assert_eq!(
            crate::element_text(&x, "keywords").as_deref(),
            Some("k1; k2")
        );
    }

    #[test]
    fn declares_a_missing_namespace_and_opens_an_empty_root() {
        let x = set_element(
            r#"<cp:coreProperties xmlns:cp="urn:x"/>"#,
            NS_DC,
            "subject",
            "dc",
            "S",
        );
        assert_eq!(
            x,
            format!(
                r#"<cp:coreProperties xmlns:cp="urn:x" xmlns:dc="{NS_DC}"><dc:subject>S</dc:subject></cp:coreProperties>"#
            )
        );
        let x = set_element(
            &format!(r#"<Properties xmlns="{NS_APP}"></Properties>"#),
            NS_APP,
            "Company",
            "",
            "B",
        );
        assert!(x.contains("<Company>B</Company></Properties>"));
    }
}

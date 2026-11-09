//! Custom file properties part (ECMA-376 Part 1, §22.3 Custom File Properties,
//! `docProps/custom.xml`), shared by the three OOXML formats.
//!
//! Reading lists every `property` (§22.3.2.2) with its value type (the
//! `vt:*` child, §22.4 Variant Types). Updating rewrites only the properties
//! that change: every other `property` element keeps its exact source text,
//! and the part is not touched at all when nothing changes.

use crate::xml::{attr, escape, unescape, XmlReader};
use crate::{Package, Relationship, Result, TargetMode};
use quick_xml::events::Event;

/// Package relationship type of the custom properties part (§15.2.12.2).
pub const REL_CUSTOM_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties";
/// Content type of the custom properties part (§15.2.12.2).
pub const CT_CUSTOM_PROPERTIES: &str =
    "application/vnd.openxmlformats-officedocument.custom-properties+xml";
/// Namespace of `Properties` / `property` (§22.3).
pub const CUSTOM_NS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties";
/// Namespace of the variant types (§22.4).
pub const VT_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes";
/// `fmtid` of user-defined properties (§22.3.2.2: the format id of the
/// property set; this value names the user-defined property set).
pub const FMTID_USER: &str = "{D5CDD505-2E9C-101B-9397-08002B2CF9AE}";
/// Part name used when a package has no custom properties part yet.
pub const DEFAULT_PART: &str = "/docProps/custom.xml";

/// One custom property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomProperty {
    /// `name` attribute.
    pub name: String,
    /// Local name of the value element (`lpwstr`, `bool`, `i4`, `filetime`, …).
    pub kind: String,
    /// Text of the value element (unescaped).
    pub value: String,
}

impl CustomProperty {
    /// A text property (`vt:lpwstr`, §22.4.2.29).
    pub fn text(name: &str, value: &str) -> Self {
        CustomProperty {
            name: name.to_owned(),
            kind: "lpwstr".into(),
            value: value.to_owned(),
        }
    }

    /// A boolean property (`vt:bool`, §22.4.2.6).
    pub fn boolean(name: &str, value: bool) -> Self {
        CustomProperty {
            name: name.to_owned(),
            kind: "bool".into(),
            value: if value { "true" } else { "false" }.into(),
        }
    }

    /// The value as a boolean (`true`/`1`, `false`/`0`), any value type.
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }
}

/// A parsed `property` element with its source span.
struct Item {
    prop: CustomProperty,
    pid: u32,
    start: usize,
    end: usize,
}

/// Root start tag end offset, `</…Properties>` offset and the properties.
fn scan(data: &[u8], part: &str) -> Result<(usize, usize, Vec<Item>)> {
    let mut r = XmlReader::new(data, part);
    let mut items = Vec::new();
    let mut root_end = 0;
    let mut close = data.len();
    let mut cur: Option<Item> = None;
    let mut in_value = false;
    loop {
        let before = r.pos();
        match r.read()? {
            Event::Start(_) | Event::Empty(_) if r.depth() <= 1 && root_end == 0 => {
                root_end = r.pos();
            }
            Event::Start(e)
                if e.local_name().as_ref().as_bytes() == b"property" && r.depth() == 2 =>
            {
                cur = Some(Item {
                    prop: CustomProperty {
                        name: attr(&e, b"name").unwrap_or_default(),
                        kind: String::new(),
                        value: String::new(),
                    },
                    pid: attr(&e, b"pid")
                        .and_then(|p| p.trim().parse().ok())
                        .unwrap_or(0),
                    start: before,
                    end: before,
                });
            }
            Event::Empty(e)
                if e.local_name().as_ref().as_bytes() == b"property" && r.depth() == 1 =>
            {
                items.push(Item {
                    prop: CustomProperty {
                        name: attr(&e, b"name").unwrap_or_default(),
                        kind: String::new(),
                        value: String::new(),
                    },
                    pid: attr(&e, b"pid")
                        .and_then(|p| p.trim().parse().ok())
                        .unwrap_or(0),
                    start: before,
                    end: r.pos(),
                });
            }
            Event::Start(e) if r.depth() == 3 => {
                if let Some(c) = cur.as_mut() {
                    if c.prop.kind.is_empty() {
                        c.prop.kind = String::from_utf8_lossy(e.local_name().as_ref().as_bytes())
                            .into_owned();
                        in_value = true;
                    }
                }
            }
            Event::Empty(e) if r.depth() == 2 => {
                if let Some(c) = cur.as_mut() {
                    if c.prop.kind.is_empty() {
                        c.prop.kind = String::from_utf8_lossy(e.local_name().as_ref().as_bytes())
                            .into_owned();
                    }
                }
            }
            Event::Text(t) if in_value => {
                if let Some(c) = cur.as_mut() {
                    c.prop
                        .value
                        .push_str(&unescape(&String::from_utf8_lossy(t.as_ref().as_bytes())));
                }
            }
            Event::GeneralRef(g) if in_value => {
                if let Some(c) = cur.as_mut() {
                    let g: &str = g.as_ref();
                    c.prop.value.push_str(&unescape(&format!("&{g};")));
                }
            }
            Event::CData(t) if in_value => {
                if let Some(c) = cur.as_mut() {
                    c.prop
                        .value
                        .push_str(&String::from_utf8_lossy(t.as_ref().as_bytes()));
                }
            }
            Event::End(_) => match r.depth() {
                2 => in_value = false,
                1 => {
                    if let Some(mut c) = cur.take() {
                        c.end = r.pos();
                        items.push(c);
                    }
                }
                0 => close = before,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((root_end, close, items))
}

/// Properties of a custom properties part.
pub fn parse_custom_properties(data: &[u8], part: &str) -> Result<Vec<CustomProperty>> {
    Ok(scan(data, part)?.2.into_iter().map(|i| i.prop).collect())
}

/// Name of the package's custom properties part, if it has one.
pub fn custom_properties_part(pkg: &Package) -> Result<Option<String>> {
    Ok(pkg
        .related_part("/", REL_CUSTOM_PROPERTIES)?
        .filter(|n| pkg.part(n).is_some()))
}

/// Custom properties of a package (empty when it has none).
pub fn read_custom_properties(pkg: &Package) -> Result<Vec<CustomProperty>> {
    match custom_properties_part(pkg)? {
        Some(name) => match pkg.part(&name) {
            Some(p) => parse_custom_properties(&p.data, &name),
            None => Ok(Vec::new()),
        },
        None => Ok(Vec::new()),
    }
}

fn property_xml(p: &CustomProperty, pid: u32, declare_vt: bool) -> String {
    let kind = if p.kind.is_empty() { "lpwstr" } else { &p.kind };
    let ns = if declare_vt {
        format!(" xmlns:vt=\"{VT_NS}\"")
    } else {
        String::new()
    };
    // §22.3.2.2 property: fmtid, pid (≥ 2, unique), name; one variant child.
    format!(
        "<property fmtid=\"{FMTID_USER}\" pid=\"{pid}\" name=\"{}\"><vt:{kind}{ns}>{}</vt:{kind}></property>",
        escape(&p.name),
        escape(&p.value)
    )
}

/// Sets (`Some`) or removes (`None`) properties by name (case-insensitive),
/// creating the part, its content type and package relationship when needed.
/// Returns whether the package changed; properties not named keep their XML.
pub fn update_custom_properties(
    pkg: &mut Package,
    changes: &[(String, Option<CustomProperty>)],
) -> Result<bool> {
    let existing = custom_properties_part(pkg)?;
    let name = existing.clone().unwrap_or_else(|| DEFAULT_PART.to_owned());
    let data = existing
        .as_ref()
        .and_then(|n| pkg.part(n))
        .map(|p| p.data.clone());
    let source = data.clone().unwrap_or_else(|| {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"{CUSTOM_NS}\" xmlns:vt=\"{VT_NS}\"></Properties>"
        )
        .into_bytes()
    });
    let (root_end, close, items) = scan(&source, &name)?;
    let text = String::from_utf8_lossy(&source).into_owned();
    let root_tag = text.get(..root_end).unwrap_or_default();
    let declare_vt = !root_tag.contains(VT_NS);
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut next_pid = items.iter().map(|i| i.pid).max().unwrap_or(1).max(1) + 1;
    let mut body = String::new();
    let mut changed = false;
    let mut cursor = root_end;
    for it in &items {
        body.push_str(text.get(cursor..it.start).unwrap_or_default());
        cursor = it.end;
        match changes.iter().find(|(n, _)| same(n, &it.prop.name)) {
            None => body.push_str(text.get(it.start..it.end).unwrap_or_default()),
            Some((_, None)) => changed = true,
            Some((_, Some(p))) => {
                if p.value == it.prop.value && (p.kind == it.prop.kind || p.kind.is_empty()) {
                    body.push_str(text.get(it.start..it.end).unwrap_or_default());
                } else {
                    changed = true;
                    let pid = if it.pid >= 2 { it.pid } else { next_pid };
                    body.push_str(&property_xml(p, pid, declare_vt));
                }
            }
        }
    }
    body.push_str(text.get(cursor..close).unwrap_or_default());
    for (n, p) in changes {
        let Some(p) = p else { continue };
        if items.iter().any(|i| same(&i.prop.name, n)) {
            continue;
        }
        changed = true;
        body.push_str(&property_xml(p, next_pid, declare_vt));
        next_pid += 1;
    }
    if !changed {
        return Ok(false);
    }
    let root_tag = if root_tag.trim_end().ends_with("/>") {
        // `<Properties …/>`: reopen it.
        format!("{}>", root_tag.trim_end().trim_end_matches("/>"))
    } else {
        root_tag.to_owned()
    };
    let rest = text.get(close..).unwrap_or_default();
    let close_tag = if rest.trim().is_empty() {
        "</Properties>"
    } else {
        rest
    };
    let xml = format!("{root_tag}{body}{close_tag}");
    if existing.is_some() {
        pkg.put_part(&name, xml.into_bytes());
    } else {
        pkg.add_part(&name, CT_CUSTOM_PROPERTIES, xml.into_bytes())?;
        let mut rels = pkg.relationships("/")?;
        let mut n = rels.len() + 1;
        while rels.iter().any(|r| r.id == format!("rId{n}")) {
            n += 1;
        }
        rels.push(Relationship {
            id: format!("rId{n}"),
            rel_type: REL_CUSTOM_PROPERTIES.into(),
            target: name.trim_start_matches('/').to_owned(),
            mode: TargetMode::Internal,
        });
        pkg.set_relationships("/", &rels);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Dự án"><vt:lpwstr>A &amp; B</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="_MarkAsFinal"><vt:bool>true</vt:bool></property></Properties>"#;

    fn package(custom: Option<&str>) -> Package {
        let mut p = Package::new();
        if let Some(x) = custom {
            p.add_part(DEFAULT_PART, CT_CUSTOM_PROPERTIES, x.as_bytes().to_vec())
                .unwrap();
            p.set_relationships(
                "/",
                &[Relationship {
                    id: "rId9".into(),
                    rel_type: REL_CUSTOM_PROPERTIES.into(),
                    target: "docProps/custom.xml".into(),
                    mode: TargetMode::Internal,
                }],
            );
        }
        p
    }

    #[test]
    fn reads_properties_with_types() {
        let props = parse_custom_properties(XML.as_bytes(), "/c.xml").unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0], CustomProperty::text("Dự án", "A & B"));
        assert_eq!(props[1].kind, "bool");
        assert_eq!(props[1].as_bool(), Some(true));
    }

    #[test]
    fn unchanged_values_leave_the_part_alone() {
        let mut p = package(Some(XML));
        let changed = update_custom_properties(
            &mut p,
            &[(
                "_markasfinal".into(),
                Some(CustomProperty::boolean("_MarkAsFinal", true)),
            )],
        )
        .unwrap();
        assert!(!changed);
        assert_eq!(p.part(DEFAULT_PART).unwrap().data, XML.as_bytes());
    }

    #[test]
    fn removes_and_adds_keeping_other_properties_verbatim() {
        let mut p = package(Some(XML));
        update_custom_properties(
            &mut p,
            &[
                ("_MarkAsFinal".into(), None),
                (
                    "BO_Label_Name".into(),
                    Some(CustomProperty::text("BO_Label_Name", "Nội bộ")),
                ),
            ],
        )
        .unwrap();
        let x = String::from_utf8(p.part(DEFAULT_PART).unwrap().data.clone()).unwrap();
        assert!(x.contains(r#"pid="2" name="Dự án"><vt:lpwstr>A &amp; B</vt:lpwstr>"#));
        assert!(!x.contains("_MarkAsFinal"));
        assert!(x.contains(r#"pid="4" name="BO_Label_Name"><vt:lpwstr>Nội bộ</vt:lpwstr>"#));
        assert!(x.ends_with("</Properties>"));
        let back = read_custom_properties(&p).unwrap();
        assert_eq!(back.len(), 2);
    }

    #[test]
    fn creates_the_part_with_relationship_and_content_type() {
        let mut p = package(None);
        assert_eq!(read_custom_properties(&p).unwrap(), Vec::new());
        update_custom_properties(
            &mut p,
            &[(
                "_MarkAsFinal".into(),
                Some(CustomProperty::boolean("_MarkAsFinal", true)),
            )],
        )
        .unwrap();
        let rels = p.relationships("/").unwrap();
        assert!(rels.iter().any(|r| r.rel_type == REL_CUSTOM_PROPERTIES));
        assert_eq!(
            p.content_types().unwrap().get(DEFAULT_PART),
            Some(CT_CUSTOM_PROPERTIES)
        );
        let back = read_custom_properties(&p).unwrap();
        assert_eq!(back, vec![CustomProperty::boolean("_MarkAsFinal", true)]);
        let x = String::from_utf8(p.part(DEFAULT_PART).unwrap().data.clone()).unwrap();
        assert!(x.contains(r#"pid="2""#));
    }

    #[test]
    fn empty_root_element_is_reopened() {
        let empty = r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"/>"#;
        let mut p = package(Some(empty));
        update_custom_properties(
            &mut p,
            &[("A".into(), Some(CustomProperty::text("A", "1")))],
        )
        .unwrap();
        let x = String::from_utf8(p.part(DEFAULT_PART).unwrap().data.clone()).unwrap();
        assert!(x.starts_with("<Properties xmlns="));
        assert!(x.contains("<vt:lpwstr xmlns:vt="));
        assert!(x.ends_with("</property></Properties>"));
        assert_eq!(read_custom_properties(&p).unwrap().len(), 1);
    }
}

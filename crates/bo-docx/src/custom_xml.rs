//! Custom XML data storage parts (ECMA-376 Part 1, §15.2.4) and their properties parts
//! (§15.2.5): read into [`Document::custom_xml`], written back when they changed.
//!
//! An item part is the target of a `customXml` relationship of the main document part; its
//! properties part (`ds:datastoreItem`, item id and schema references) is the target of the
//! item's `customXmlProps` relationship. Parts that did not change stay byte for byte.

use crate::Result;
use bo_model::custom_xml::{dom, parse_props, CustomXmlPart};
use bo_model::Document;
use bo_opc::{resolve_target, Package, Relationship, TargetMode};

/// §15.2.4 Custom XML Data Storage part relationship.
pub(crate) const REL_CUSTOM_XML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
/// §15.2.5 Custom XML Data Storage Properties part relationship.
pub(crate) const REL_CUSTOM_XML_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
/// §15.2.5 content type of the properties part.
pub(crate) const CT_CUSTOM_XML_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
/// §15.2.4: an item part is any XML (`application/xml`).
pub(crate) const CT_CUSTOM_XML: &str = "application/xml";

/// The custom XML parts of the main document part, in relationship order.
pub(crate) fn read_parts(pkg: &Package, main: &str) -> Result<Vec<CustomXmlPart>> {
    let mut out = Vec::new();
    for r in pkg.relationships(main)? {
        if r.rel_type != REL_CUSTOM_XML || r.mode != TargetMode::Internal {
            continue;
        }
        let name = resolve_target(main, &r.target);
        let Some(part) = pkg.part(&name) else {
            continue;
        };
        if out
            .iter()
            .any(|p: &CustomXmlPart| p.part_name.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        let props_part = pkg.related_part(&part.name, REL_CUSTOM_XML_PROPS)?;
        let (item_id, schema_refs) = props_part
            .as_deref()
            .and_then(|n| pkg.part(n))
            .map(|p| parse_props(&dom::decode(&p.data)))
            .unwrap_or_default();
        out.push(CustomXmlPart {
            part_name: part.name.clone(),
            xml: dom::decode(&part.data),
            item_id,
            schema_refs,
            props_part,
        });
    }
    Ok(out)
}

/// Relative reference from the folder of `source` to `target` (both absolute part names).
fn relative(source: &str, target: &str) -> String {
    let from: Vec<&str> = source
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, d)| d.to_vec())
        .unwrap_or_default();
    let to: Vec<&str> = target.trim_start_matches('/').split('/').collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend_from_slice(&to[common..]);
    parts.join("/")
}

/// Write the parts that changed since the document was read (all of them for a new
/// document): removed parts go with their properties and relationships, new parts get a
/// relationship from the main part. Returns whether `main_rels` changed.
pub(crate) fn write_parts(
    pkg: &mut Package,
    main: &str,
    main_rels: &mut Vec<Relationship>,
    doc: &Document,
) -> Result<bool> {
    let before: &[CustomXmlPart] = doc.source.as_ref().map_or(&[], |s| &s.custom_xml);
    if before == doc.custom_xml.as_slice() {
        return Ok(false);
    }
    let mut rels_changed = false;
    let mut ct = pkg.content_types()?;
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    for old in before {
        if doc
            .custom_xml
            .iter()
            .any(|p| same(&p.part_name, &old.part_name))
        {
            continue;
        }
        let mut gone = vec![
            old.part_name.clone(),
            bo_opc::rels_part_name(&old.part_name),
        ];
        gone.extend(old.props_part.clone());
        for name in &gone {
            pkg.remove_part(name);
            ct.overrides.retain(|(n, _)| !same(n, name));
        }
        let n = main_rels.len();
        main_rels.retain(|r| {
            !(r.rel_type == REL_CUSTOM_XML
                && same(&resolve_target(main, &r.target), &old.part_name))
        });
        rels_changed |= main_rels.len() != n;
    }
    for p in &doc.custom_xml {
        let prev = before.iter().find(|o| same(&o.part_name, &p.part_name));
        if prev == Some(p) {
            continue;
        }
        if prev.is_none_or(|o| o.xml != p.xml) {
            pkg.put_part(&p.part_name, p.xml.as_bytes().to_vec());
            if ct.get(&p.part_name).is_none() {
                ct.set_override(&p.part_name, CT_CUSTOM_XML);
            }
        }
        if prev.is_none() {
            let mut k = main_rels.len() + 1;
            while main_rels.iter().any(|r| r.id == format!("rId{k}")) {
                k += 1;
            }
            main_rels.push(Relationship {
                id: format!("rId{k}"),
                rel_type: REL_CUSTOM_XML.to_owned(),
                target: relative(main, &p.part_name),
                mode: TargetMode::Internal,
            });
            rels_changed = true;
        }
        let props_changed = prev.is_none_or(|o| {
            o.item_id != p.item_id || o.schema_refs != p.schema_refs || o.props_part != p.props_part
        });
        if let (true, Some(props)) = (props_changed, &p.props_part) {
            pkg.put_part(props, p.props_xml().into_bytes());
            ct.set_override(props, CT_CUSTOM_XML_PROPS);
            let mut rels = pkg.relationships(&p.part_name)?;
            rels.retain(|r| r.rel_type != REL_CUSTOM_XML_PROPS);
            let mut k = 1;
            while rels.iter().any(|r| r.id == format!("rId{k}")) {
                k += 1;
            }
            rels.push(Relationship {
                id: format!("rId{k}"),
                rel_type: REL_CUSTOM_XML_PROPS.to_owned(),
                target: relative(&p.part_name, props),
                mode: TargetMode::Internal,
            });
            pkg.set_relationships(&p.part_name, &rels);
        }
    }
    pkg.set_content_types(&ct);
    Ok(rels_changed)
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_targets() {
        assert_eq!(
            relative("/word/document.xml", "/customXml/item1.xml"),
            "../customXml/item1.xml"
        );
        assert_eq!(
            relative("/customXml/item1.xml", "/customXml/itemProps1.xml"),
            "itemProps1.xml"
        );
        assert_eq!(
            relative("/document.xml", "/customXml/item1.xml"),
            "customXml/item1.xml"
        );
    }
}

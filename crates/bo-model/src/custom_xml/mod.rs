//! Custom XML data storage (ECMA-376 Part 1, §15.2.4 Custom XML Data Storage part, §15.2.5
//! Custom XML Data Storage Properties part) and content control data binding (§17.5.2.6
//! dataBinding).
//!
//! A document keeps its custom XML parts as text ([`CustomXmlPart`]); a content control may
//! carry a [`DataBinding`] naming a part (`w:storeItemID`, matched against the part's
//! `ds:itemID`) and a node in it (`w:xpath`, resolved with `w:prefixMappings`). [`binding`]
//! keeps the two in step both ways.

pub mod binding;
pub mod dom;
pub mod xpath;

use serde::{Deserialize, Serialize};

/// Namespace of the custom XML data properties part (§15.2.5, `ds:datastoreItem`).
pub const NS_DS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/customXml";

/// Most nodes listed for one part by [`CustomXmlPart::outline`].
pub const OUTLINE_LIMIT: usize = 2000;

/// A Custom XML Data Storage part (§15.2.4) with its properties part (§15.2.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomXmlPart {
    /// Part name (`/customXml/item1.xml`).
    pub part_name: String,
    /// The part's XML text.
    pub xml: String,
    /// `ds:datastoreItem/@ds:itemID` (§15.2.5; a GUID such as `{5C3A…}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    /// `ds:schemaRefs/ds:schemaRef/@ds:uri` in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schema_refs: Vec<String>,
    /// Name of the properties part (`/customXml/itemProps1.xml`), when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props_part: Option<String>,
}

/// Compare GUID strings the way item ids are matched: case-insensitively, braces optional.
#[must_use]
pub fn same_item_id(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_matches(['{', '}']).to_ascii_lowercase();
    norm(a) == norm(b)
}

impl CustomXmlPart {
    /// The parsed XML (`None` when it is not well-formed).
    #[must_use]
    pub fn parse(&self) -> Option<dom::XDocument> {
        dom::parse(&self.xml).ok()
    }

    /// Qualified name and namespace URI of the document element.
    #[must_use]
    pub fn root(&self) -> Option<(String, String)> {
        let d = self.parse()?;
        let ns = d.root.declared_ns(d.root.prefix()).unwrap_or("").to_owned();
        Some((d.root.name, ns))
    }

    /// The part has the item id `id`.
    #[must_use]
    pub fn has_id(&self, id: &str) -> bool {
        self.item_id.as_deref().is_some_and(|i| same_item_id(i, id))
    }

    /// The properties part's XML (§15.2.5: `ds:datastoreItem` with `ds:schemaRefs`).
    #[must_use]
    pub fn props_xml(&self) -> String {
        let esc = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('"', "&quot;")
        };
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<ds:datastoreItem",
        );
        if let Some(id) = &self.item_id {
            s.push_str(" ds:itemID=\"");
            s.push_str(&esc(id));
            s.push('"');
        }
        s.push_str(" xmlns:ds=\"");
        s.push_str(NS_DS);
        s.push_str("\"><ds:schemaRefs>");
        for r in &self.schema_refs {
            s.push_str("<ds:schemaRef ds:uri=\"");
            s.push_str(&esc(r));
            s.push_str("\"/>");
        }
        s.push_str("</ds:schemaRefs></ds:datastoreItem>");
        s
    }
}

/// Read a properties part (§15.2.5): the item id and the schema references.
#[must_use]
pub fn parse_props(xml: &str) -> (Option<String>, Vec<String>) {
    let Ok(d) = dom::parse(xml) else {
        return (None, Vec::new());
    };
    let attr = |e: &dom::XElement, local: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k.rsplit(':').next() == Some(local))
            .map(|(_, v)| v.clone())
    };
    let id = attr(&d.root, "itemID");
    let refs = d
        .root
        .elements()
        .filter(|e| e.local() == "schemaRefs")
        .flat_map(|e| e.elements().filter(|r| r.local() == "schemaRef"))
        .filter_map(|r| attr(r, "uri"))
        .collect();
    (id, refs)
}

/// A content control's data binding (§17.5.2.6 dataBinding).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataBinding {
    /// `w:xpath`: the bound node.
    pub xpath: String,
    /// `w:prefixMappings` (§17.5.2.27 form: `xmlns:ns0='uri'`, space separated).
    #[serde(default)]
    pub prefix_mappings: String,
    /// `w:storeItemID`: the part's `ds:itemID`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_item_id: Option<String>,
}

/// A node a binding resolves to: the part index, the node and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Index into the document's parts.
    pub part: usize,
    /// The node.
    pub target: xpath::Target,
    /// Its value.
    pub value: String,
}

/// Resolve a binding against parsed parts (`None` entries: parts that are not well-formed).
///
/// A part whose `ds:itemID` equals `w:storeItemID` is searched; when no part has that id (or
/// none is named), the parts are searched in order and the first one where the path selects a
/// node is used (our choice for bindings whose store is missing).
#[must_use]
pub fn resolve_in(
    parts: &[CustomXmlPart],
    docs: &[Option<dom::XDocument>],
    b: &DataBinding,
) -> Option<Resolved> {
    let xp = xpath::XPath::parse(&b.xpath).ok()?;
    let maps = xpath::prefix_mappings(&b.prefix_mappings);
    let named = b
        .store_item_id
        .as_deref()
        .and_then(|id| parts.iter().position(|p| p.has_id(id)));
    let order: Vec<usize> = match named {
        Some(i) => vec![i],
        None => (0..parts.len()).collect(),
    };
    order.into_iter().find_map(|i| {
        let d = docs.get(i)?.as_ref()?;
        let target = xpath::select(d, &xp, &maps)?;
        let value = xpath::value(d, &target)?;
        Some(Resolved {
            part: i,
            target,
            value,
        })
    })
}

/// Resolve a binding against the document's parts.
#[must_use]
pub fn resolve(parts: &[CustomXmlPart], b: &DataBinding) -> Option<Resolved> {
    let docs: Vec<Option<dom::XDocument>> = parts.iter().map(CustomXmlPart::parse).collect();
    resolve_in(parts, &docs, b)
}

/// A new item id derived from `seed` (a GUID-shaped string; FNV-1a of the seed and the
/// existing ids, so ids differ within the document).
#[must_use]
pub fn new_item_id(seed: &str, parts: &[CustomXmlPart]) -> String {
    let mut salt = 0u64;
    loop {
        let h1 = crate::fingerprint(&(seed, salt, parts.len()));
        let h2 = crate::fingerprint(&(salt, seed, "item"));
        let id = format!(
            "{{{:08X}-{:04X}-4{:03X}-{:04X}-{:012X}}}",
            h1 >> 32,
            (h1 >> 16) & 0xFFFF,
            h1 & 0x0FFF,
            0x8000 | (h2 >> 48 & 0x3FFF),
            h2 & 0xFFFF_FFFF_FFFF
        );
        if !parts.iter().any(|p| p.has_id(&id)) {
            return id;
        }
        salt += 1;
    }
}

/// The first free `/customXml/itemN.xml` name (and its `itemPropsN.xml`).
#[must_use]
pub fn new_part_names(parts: &[CustomXmlPart], taken: &[String]) -> (String, String) {
    let used = |n: &str| {
        parts.iter().any(|p| {
            p.part_name.eq_ignore_ascii_case(n)
                || p.props_part
                    .as_deref()
                    .is_some_and(|q| q.eq_ignore_ascii_case(n))
        }) || taken.iter().any(|t| t.eq_ignore_ascii_case(n))
    };
    let mut k = 1;
    loop {
        let item = format!("/customXml/item{k}.xml");
        let props = format!("/customXml/itemProps{k}.xml");
        if !used(&item) && !used(&props) {
            return (item, props);
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(xml: &str, id: Option<&str>) -> CustomXmlPart {
        CustomXmlPart {
            part_name: "/customXml/item1.xml".into(),
            xml: xml.into(),
            item_id: id.map(str::to_owned),
            schema_refs: vec![],
            props_part: None,
        }
    }

    #[test]
    fn props_part_round_trip() {
        let mut p = part("<a/>", Some("{AB-1}"));
        p.schema_refs = vec!["urn:s1".into(), "urn:s&2".into()];
        let xml = p.props_xml();
        assert_eq!(
            parse_props(&xml),
            (
                Some("{AB-1}".into()),
                vec!["urn:s1".into(), "urn:s&2".into()]
            )
        );
    }

    #[test]
    fn binding_resolves_by_store_id_then_by_search() {
        let parts = vec![
            part(
                "<a xmlns=\"urn:x\"><v>one</v></a>",
                Some("{11111111-0000-0000-0000-000000000000}"),
            ),
            part(
                "<b xmlns=\"urn:y\"><v>two</v></b>",
                Some("{22222222-0000-0000-0000-000000000000}"),
            ),
        ];
        let b = |xp: &str, maps: &str, id: Option<&str>| DataBinding {
            xpath: xp.into(),
            prefix_mappings: maps.into(),
            store_item_id: id.map(str::to_owned),
        };
        let r = resolve(
            &parts,
            &b(
                "/ns0:b/ns0:v",
                "xmlns:ns0='urn:y'",
                Some("{22222222-0000-0000-0000-000000000000}"),
            ),
        )
        .expect("bound");
        assert_eq!((r.part, r.value.as_str()), (1, "two"));
        // Ids compare case-insensitively.
        assert!(resolve(
            &parts,
            &b(
                "/ns0:a/ns0:v",
                "xmlns:ns0='urn:x'",
                Some(
                    "{11111111-0000-0000-0000-000000000000}"
                        .to_lowercase()
                        .as_str()
                )
            ),
        )
        .is_some());
        // A named store that does not hold the node: unbound (no search of the others).
        assert!(resolve(
            &parts,
            &b(
                "/ns0:a/ns0:v",
                "xmlns:ns0='urn:x'",
                Some("{22222222-0000-0000-0000-000000000000}")
            ),
        )
        .is_none());
        // No store named: the first part where the path selects a node.
        let r = resolve(&parts, &b("/ns0:b/ns0:v", "xmlns:ns0='urn:y'", None)).expect("found");
        assert_eq!(r.part, 1);
    }

    #[test]
    fn new_ids_and_names_are_fresh() {
        let mut parts = vec![part("<a/>", None)];
        let id = new_item_id("x", &parts);
        assert_eq!(id.len(), 38);
        assert!(id.starts_with('{') && id.ends_with('}'));
        parts[0].item_id = Some(id.clone());
        assert_ne!(new_item_id("x", &parts), id);
        parts[0].props_part = Some("/customXml/itemProps1.xml".into());
        assert_eq!(
            new_part_names(&parts, &[]),
            (
                "/customXml/item2.xml".into(),
                "/customXml/itemProps2.xml".into()
            )
        );
        assert_eq!(
            new_part_names(&parts, &["/customXml/item2.xml".into()]).0,
            "/customXml/item3.xml"
        );
    }
}

//! Custom XML parts (ECMA-376 Part 1, §15.2.4 / §15.2.5) and content control data binding
//! (§17.5.2.6): adding, replacing and removing parts, setting a node's value, binding the
//! content control at the caret to a node. Each command is one undo step that includes the
//! part change and every bound control it refreshes; ordinary edits of a bound control
//! update its node in the same step (see [`sync_in`], run by `Session::edit`).

use crate::{para, EditError, Op, Result, Session, Tx};
use bo_model::custom_xml::binding::{self, SyncMode};
use bo_model::custom_xml::{
    dom, new_item_id, new_part_names, resolve, resolve_in, xpath, CustomXmlPart, DataBinding,
    OUTLINE_LIMIT,
};
use bo_model::RunContent;
use serde::Serialize;

/// Largest custom XML part accepted from a command (characters).
pub const MAX_PART_LEN: usize = 4 * 1024 * 1024;

/// Synchronise bound controls and parts inside a transaction, recording the changes.
pub(crate) fn sync_in(tx: &mut Tx<'_>, mode: SyncMode) {
    let out = binding::sync(tx.doc, mode, true);
    for e in out.edits {
        tx.record_applied(Op::Blocks {
            path: e.path,
            index: e.index,
            remove: e.remove,
            insert: e.insert,
        });
    }
    if let Some(old) = out.old_parts {
        let new = tx.doc.custom_xml.clone();
        tx.record_applied(Op::CustomXml { old, new });
    }
}

/// A part as the XML mapping pane shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomXmlInfo {
    /// Index of the part.
    pub index: usize,
    /// Part name.
    pub part_name: String,
    /// `ds:itemID`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    /// Qualified name of the document element.
    pub root: String,
    /// Namespace of the document element (`""`: none).
    pub namespace: String,
    /// Schema references (§15.2.5).
    pub schema_refs: Vec<String>,
    /// The part is not well-formed XML (no nodes listed).
    pub broken: bool,
    /// Elements and attributes with their binding paths.
    pub nodes: Vec<xpath::OutlineNode>,
    /// `w:prefixMappings` for those paths.
    pub prefix_mappings: String,
    /// Content controls bound to nodes of this part.
    pub bound_controls: usize,
}

fn err(key: &str) -> EditError {
    EditError::Unsupported(key.to_owned())
}

/// Check and normalise the text of a new part: well-formed XML, written back as UTF-8.
fn checked_xml(xml: &str) -> Result<String> {
    if xml.len() > MAX_PART_LEN {
        return Err(err("tooLarge"));
    }
    let d = dom::parse(xml).map_err(|_| err("badXml"))?;
    Ok(dom::write(&d))
}

impl Session {
    /// The document's custom XML parts with their node outlines (for the mapping pane).
    #[must_use]
    pub fn custom_xml_parts(&self) -> Vec<CustomXmlInfo> {
        let doc = self.doc();
        let bindings: Vec<DataBinding> = bo_model::index::body_paragraphs(doc)
            .iter()
            .filter_map(|path| bo_model::index::para_at(doc, path))
            .flat_map(|p| p.runs.iter().flat_map(|r| &r.content))
            .filter_map(|c| match c {
                RunContent::ControlStart(cc) => cc.data_binding.clone(),
                _ => None,
            })
            .collect();
        let parsed_all: Vec<Option<dom::XDocument>> =
            doc.custom_xml.iter().map(CustomXmlPart::parse).collect();
        let mut counts = vec![0; doc.custom_xml.len()];
        for b in &bindings {
            if let Some(r) = resolve_in(&doc.custom_xml, &parsed_all, b) {
                counts[r.part] += 1;
            }
        }
        doc.custom_xml
            .iter()
            .enumerate()
            .map(|(index, p)| {
                let parsed = parsed_all[index].as_ref();
                let (nodes, maps) = parsed
                    .as_ref()
                    .map(|d| xpath::outline(d, OUTLINE_LIMIT))
                    .unwrap_or_default();
                let (root, namespace) = p.root().unwrap_or_default();
                CustomXmlInfo {
                    index,
                    part_name: p.part_name.clone(),
                    item_id: p.item_id.clone(),
                    root,
                    namespace,
                    schema_refs: p.schema_refs.clone(),
                    broken: parsed.is_none(),
                    nodes,
                    prefix_mappings: xpath::format_mappings(&maps),
                    bound_controls: counts[index],
                }
            })
            .collect()
    }

    /// Add a custom XML part (§15.2.4) holding `xml`, with a properties part (§15.2.5) giving
    /// it a new item id. Controls whose binding names no store and whose path the new part
    /// satisfies show its values. Returns the part's index.
    ///
    /// # Errors
    /// `badXml` (not well-formed), `tooLarge`.
    pub fn add_custom_xml_part(&mut self, xml: &str) -> Result<usize> {
        let xml = checked_xml(xml)?;
        let index = self.doc().custom_xml.len();
        self.edit(|tx, sel| {
            let old = tx.doc.custom_xml.clone();
            let (part_name, props) = new_part_names(&old, &[]);
            let item_id = new_item_id(&xml, &old);
            let root_ns = dom::parse(&xml)
                .ok()
                .and_then(|d| d.root.declared_ns(d.root.prefix()).map(str::to_owned));
            let mut new = old.clone();
            new.push(CustomXmlPart {
                part_name,
                xml: xml.clone(),
                item_id: Some(item_id),
                schema_refs: root_ns.into_iter().collect(),
                props_part: Some(props),
            });
            tx.push(Op::CustomXml { old, new })?;
            sync_in(tx, SyncMode::FromParts);
            Ok(sel)
        })?;
        Ok(index)
    }

    /// Replace the XML of part `index` (its item id stays); bound controls show the new values.
    ///
    /// # Errors
    /// `badPart`, `badXml`, `tooLarge`.
    pub fn replace_custom_xml_part(&mut self, index: usize, xml: &str) -> Result<bool> {
        let xml = checked_xml(xml)?;
        if index >= self.doc().custom_xml.len() {
            return Err(err("badPart"));
        }
        if self.doc().custom_xml[index].xml == xml {
            return Ok(false);
        }
        self.edit(|tx, sel| {
            let old = tx.doc.custom_xml.clone();
            let mut new = old.clone();
            new[index].xml.clone_from(&xml);
            tx.push(Op::CustomXml { old, new })?;
            sync_in(tx, SyncMode::FromParts);
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(true)
    }

    /// Remove part `index` (and its properties). Controls bound to it keep their content.
    ///
    /// # Errors
    /// `badPart`.
    pub fn remove_custom_xml_part(&mut self, index: usize) -> Result<()> {
        if index >= self.doc().custom_xml.len() {
            return Err(err("badPart"));
        }
        self.edit(|tx, sel| {
            let old = tx.doc.custom_xml.clone();
            let mut new = old.clone();
            new.remove(index);
            tx.push(Op::CustomXml { old, new })?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Set the value of node `path` (resolved with `mappings`) of part `index`; every control
    /// bound to it shows the new value. Returns whether something changed.
    ///
    /// # Errors
    /// `badPart`, `badXml`, `badXPath`, `noNode`, `notLeaf`.
    pub fn set_custom_xml_value(
        &mut self,
        index: usize,
        path: &str,
        mappings: &str,
        value: &str,
    ) -> Result<bool> {
        let mut parts = self.doc().custom_xml.clone();
        let Some(old) =
            binding::set_node_value(&mut parts, index, path, mappings, value).map_err(err)?
        else {
            return Ok(false);
        };
        self.edit(|tx, sel| {
            tx.push(Op::CustomXml { old, new: parts })?;
            sync_in(tx, SyncMode::FromParts);
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(true)
    }

    /// Bind the content control around the caret to node `path` of part `index` (§17.5.2.6).
    /// The control then shows the node's value; when the node is empty and the control holds
    /// content (not its placeholder), the content goes to the node instead (our choice, so
    /// binding a filled-in control does not lose its text).
    ///
    /// # Errors
    /// `noControl`, `badPart`, `badXPath`, `noNode`, `locked` (§17.5.2.23 sdtLocked).
    pub fn bind_control(&mut self, index: usize, path: &str, mappings: &str) -> Result<()> {
        let (cc, para_id, s, _) = self.control_span().ok_or_else(|| err("noControl"))?;
        if cc.lock.as_deref() == Some("sdtLocked") || cc.lock.as_deref() == Some("sdtContentLocked")
        {
            return Err(err("locked"));
        }
        if matches!(
            cc.kind,
            bo_model::ControlKind::Picture
                | bo_model::ControlKind::Group
                | bo_model::ControlKind::RepeatingSection
                | bo_model::ControlKind::RepeatingSectionItem
        ) {
            return Err(err("notBindable"));
        }
        let part = self
            .doc()
            .custom_xml
            .get(index)
            .ok_or_else(|| err("badPart"))?;
        xpath::XPath::parse(path).map_err(|_| err("badXPath"))?;
        let b = DataBinding {
            xpath: path.to_owned(),
            prefix_mappings: mappings.to_owned(),
            store_item_id: part.item_id.clone(),
        };
        let r = resolve(&self.doc().custom_xml, &b).ok_or_else(|| err("noNode"))?;
        let mut bound = cc.clone();
        bound.data_binding = Some(b);
        let caret = self.selection;
        let has_content = !cc.placeholder && self.control_text().is_some_and(|t| !t.is_empty());
        let mode = if r.value.is_empty() && has_content {
            SyncMode::FromControls
        } else {
            SyncMode::FromParts
        };
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(para_id)?;
            atoms[s].1 = RunContent::ControlStart(Box::new(bound));
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            sync_in(tx, mode);
            Ok(caret)
        })?;
        self.fix_selection();
        Ok(())
    }

    /// Remove the data binding of the content control around the caret (its content stays).
    /// Returns whether it was bound.
    ///
    /// # Errors
    /// Stale document positions.
    pub fn unbind_control(&mut self) -> Result<bool> {
        let Some((cc, para_id, s, _)) = self.control_span() else {
            return Ok(false);
        };
        if cc.data_binding.is_none() {
            return Ok(false);
        }
        let mut free = cc;
        free.data_binding = None;
        let caret = self.selection;
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(para_id)?;
            atoms[s].1 = RunContent::ControlStart(Box::new(free));
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(caret)
        })?;
        Ok(true)
    }

    /// Text of the inline control around the caret.
    fn control_text(&self) -> Option<String> {
        let (_, para_id, s, e) = self.control_span()?;
        let paths = bo_model::index::body_paragraphs(self.doc());
        let p = bo_model::index::para_at(self.doc(), paths.get(para_id)?)?;
        let atoms = para::flatten(p);
        let mut out = String::new();
        for (_, c) in atoms.get(s + 1..e)? {
            c.push_text(&mut out);
        }
        Some(out)
    }

    /// Binding status of the control around the caret: `Some(true)` bound to an existing
    /// node, `Some(false)` its node is missing, `None` not bound.
    #[must_use]
    pub fn control_binding_status(&self) -> Option<bool> {
        let (cc, ..) = self.control_span()?;
        binding::status(self.doc(), &cc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pos, Selection};
    use bo_model::{
        Block, ContentControl, ControlKind, Document, ParaProps, Paragraph, Run, RunProps, Section,
    };

    const ID: &str = "{9F0C2B11-2D3E-4F50-8A61-7B8C9D0E1F21}";
    const MAPS: &str = "xmlns:ns0='urn:t'";

    fn run(c: RunContent) -> Run {
        Run {
            props: RunProps::default(),
            content: vec![c],
        }
    }

    fn control(binding: Option<&str>, kind: ControlKind) -> ContentControl {
        ContentControl {
            id: Some(7),
            alias: None,
            tag: None,
            kind,
            lock: None,
            placeholder: false,
            block: false,
            props_xml: None,
            data_binding: binding.map(|x| DataBinding {
                xpath: x.into(),
                prefix_mappings: MAPS.into(),
                store_item_id: Some(ID.into()),
            }),
            appearance: None,
            color: None,
            temporary: false,
        }
    }

    fn para(before: &str, cc: ContentControl, inner: &str) -> Block {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![
                run(RunContent::Text(before.into())),
                run(RunContent::ControlStart(Box::new(cc))),
                run(RunContent::Text(inner.into())),
                run(RunContent::ControlEnd(false)),
            ],
        ))
    }

    fn session() -> Session {
        let text = || ControlKind::Text { multi_line: false };
        let doc = Document {
            sections: vec![Section::new(vec![
                para("Name: ", control(Some("/ns0:r/ns0:name"), text()), "An"),
                para("", control(Some("/ns0:r/ns0:name"), text()), "An"),
                para("Free: ", control(None, text()), "abc"),
            ])],
            custom_xml: vec![CustomXmlPart {
                part_name: "/customXml/item1.xml".into(),
                xml: "<r xmlns=\"urn:t\"><name>An</name><code/></r>".into(),
                item_id: Some(ID.into()),
                schema_refs: vec![],
                props_part: Some("/customXml/itemProps1.xml".into()),
            }],
            ..Document::default()
        };
        Session::new(doc)
    }

    fn texts(s: &Session) -> Vec<String> {
        (0..3).filter_map(|i| s.para_text(i)).collect()
    }

    #[test]
    fn typing_in_a_bound_control_updates_node_and_twins_in_one_step() {
        let mut s = session();
        s.set_selection(Selection::caret(Pos::new(0, 7)));
        s.insert_text("x").unwrap();
        assert_eq!(texts(&s), ["Name: Axn", "Axn", "Free: abc"]);
        assert!(s.doc().custom_xml[0].xml.contains("<name>Axn</name>"));
        assert!(s.undo());
        assert_eq!(texts(&s), ["Name: An", "An", "Free: abc"]);
        assert!(s.doc().custom_xml[0].xml.contains("<name>An</name>"));
        assert!(s.redo());
        assert_eq!(texts(&s), ["Name: Axn", "Axn", "Free: abc"]);
        assert!(s.doc().custom_xml[0].xml.contains("<name>Axn</name>"));
    }

    #[test]
    fn editing_a_node_refreshes_every_bound_control() {
        let mut s = session();
        assert!(s
            .set_custom_xml_value(0, "/ns0:r/ns0:name", MAPS, "Bảo")
            .unwrap());
        assert_eq!(texts(&s), ["Name: Bảo", "Bảo", "Free: abc"]);
        assert!(!s
            .set_custom_xml_value(0, "/ns0:r/ns0:name", MAPS, "Bảo")
            .unwrap());
        assert!(matches!(
            s.set_custom_xml_value(0, "/ns0:r", MAPS, "v"),
            Err(EditError::Unsupported(k)) if k == "notLeaf"
        ));
        assert!(s.undo());
        assert_eq!(texts(&s), ["Name: An", "An", "Free: abc"]);
    }

    #[test]
    fn bind_and_unbind_the_control_at_the_caret() {
        let mut s = session();
        // The free control holds text and the node is empty: the text goes to the node.
        s.set_selection(Selection::caret(Pos::new(2, 7)));
        assert_eq!(s.control_binding_status(), None);
        s.bind_control(0, "/ns0:r/ns0:code", MAPS).unwrap();
        assert_eq!(s.control_binding_status(), Some(true));
        assert!(s.doc().custom_xml[0].xml.contains("<code>abc</code>"));
        // Bound to a node with a value: the control shows it.
        s.bind_control(0, "/ns0:r/ns0:name", MAPS).unwrap();
        assert_eq!(texts(&s)[2], "Free: An");
        assert!(matches!(
            s.bind_control(0, "/ns0:r/ns0:none", MAPS),
            Err(EditError::Unsupported(k)) if k == "noNode"
        ));
        assert!(s.unbind_control().unwrap());
        assert!(!s.unbind_control().unwrap());
        // Unbound: editing the node no longer reaches it.
        s.set_custom_xml_value(0, "/ns0:r/ns0:name", MAPS, "Z")
            .unwrap();
        assert_eq!(texts(&s), ["Name: Z", "Z", "Free: An"]);
        s.set_selection(Selection::caret(Pos::new(0, 1)));
        assert!(matches!(
            s.bind_control(0, "/ns0:r/ns0:name", MAPS),
            Err(EditError::Unsupported(k)) if k == "noControl"
        ));
    }

    #[test]
    fn add_replace_and_remove_parts() {
        let mut s = session();
        assert!(matches!(
            s.add_custom_xml_part("<a><b></a>"),
            Err(EditError::Unsupported(k)) if k == "badXml"
        ));
        let i = s
            .add_custom_xml_part(
                "<?xml version=\"1.0\" encoding=\"utf-16\"?><o:x xmlns:o=\"urn:o\"><o:v>1</o:v></o:x>",
            )
            .unwrap();
        assert_eq!(i, 1);
        let p = &s.doc().custom_xml[1];
        assert_eq!(p.part_name, "/customXml/item2.xml");
        assert_eq!(p.props_part.as_deref(), Some("/customXml/itemProps2.xml"));
        assert_eq!(p.schema_refs, ["urn:o"]);
        assert!(p
            .xml
            .starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert_ne!(p.item_id, s.doc().custom_xml[0].item_id);
        let info = s.custom_xml_parts();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].bound_controls, 2);
        assert_eq!(info[1].root, "o:x");
        assert_eq!(info[1].namespace, "urn:o");
        assert_eq!(info[1].prefix_mappings, "xmlns:ns0='urn:o'");
        assert_eq!(info[1].nodes[1].xpath, "/ns0:x[1]/ns0:v[1]");
        // Replacing the bound part refreshes the controls.
        assert!(s
            .replace_custom_xml_part(0, "<r xmlns=\"urn:t\"><name>Mới</name></r>")
            .unwrap());
        assert_eq!(texts(&s)[1], "Mới");
        // Removing it leaves the controls' text, now unbound.
        s.remove_custom_xml_part(0).unwrap();
        assert_eq!(texts(&s)[1], "Mới");
        s.set_selection(Selection::caret(Pos::new(1, 1)));
        assert_eq!(s.control_binding_status(), Some(false));
        assert!(s.undo());
        assert_eq!(s.doc().custom_xml.len(), 2);
        s.set_selection(Selection::caret(Pos::new(1, 1)));
        assert_eq!(s.control_binding_status(), Some(true));
        assert!(matches!(
            s.remove_custom_xml_part(9),
            Err(EditError::Unsupported(k)) if k == "badPart"
        ));
    }
}

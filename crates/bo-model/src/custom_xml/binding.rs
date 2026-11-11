//! Two-way data binding between content controls and custom XML parts (ECMA-376 Part 1,
//! §17.5.2.6 dataBinding).
//!
//! - [`SyncMode::FromParts`] (opening a document, editing a part): every bound control whose
//!   node exists shows the node's value.
//! - [`SyncMode::FromControls`] (after an edit of the body): a bound control whose content no
//!   longer matches its node was edited, so its value goes to the node, then every other
//!   control bound to that node shows the new value.
//!
//! A control whose node does not exist keeps its content ("unbound"). Only body controls are
//! synchronised; a bound control inside another bound control follows the outer one.
//!
//! Values and content: text controls show the node's text (a line feed is a line break, or a
//! new paragraph in a block-level control); a list control shows the display text of the item
//! whose value is the node's (§17.5.2.21 listItem), else the value itself; a date control shows
//! the date in its `w:dateFormat` (§17.5.2.18), and stores it as §17.5.2.40 storeMappedDataAs
//! says (`date`: `xsd:date`, `dateTime`: `xsd:dateTime`, `text` or absent: the shown text); a
//! check box ([MS-DOCX] w14:checkbox) is checked when the node is `true` or `1`.

use super::{dom, resolve_in, xpath, CustomXmlPart, DataBinding};
use crate::field::{format_date, DateTime};
use crate::index::{blocks_at_mut, body_paragraphs, para_at, ParaPath};
use crate::{Block, ContentControl, ControlKind, Document, Paragraph, Run, RunContent, RunProps};
use std::collections::BTreeMap;

/// One replacement in a block container, already applied (the data of an undoable edit).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdit {
    /// Container path (`[section, (table block, row, cell)*]`).
    pub path: Vec<usize>,
    /// First replaced block.
    pub index: usize,
    /// Blocks removed.
    pub remove: Vec<Block>,
    /// Blocks inserted.
    pub insert: Vec<Block>,
}

/// Direction of a synchronisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Controls take their nodes' values.
    FromParts,
    /// Edited controls update their nodes, then the other controls follow.
    FromControls,
}

/// What a synchronisation changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncOutcome {
    /// Container edits applied to the body, in order.
    pub edits: Vec<BlockEdit>,
    /// The parts before, when a node value changed (the parts were rewritten).
    pub old_parts: Option<Vec<CustomXmlPart>>,
    /// Bound controls whose node exists.
    pub bound: usize,
    /// Bound controls whose node does not exist.
    pub unbound: usize,
}

type Atom = (RunProps, RunContent);

/// Content items of a paragraph, one per item.
#[must_use]
pub fn atoms(p: &Paragraph) -> Vec<Atom> {
    p.runs
        .iter()
        .flat_map(|r| r.content.iter().map(|c| (r.props.clone(), c.clone())))
        .collect()
}

/// Group items back into runs (adjacent items with equal properties share a run).
#[must_use]
pub fn runs_of(atoms: Vec<Atom>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (props, item) in atoms {
        if matches!(&item, RunContent::Text(t) if t.is_empty()) {
            continue;
        }
        match runs.last_mut() {
            Some(r) if r.props == props => {
                if let (Some(RunContent::Text(prev)), RunContent::Text(t)) =
                    (r.content.last_mut(), &item)
                {
                    prev.push_str(t);
                } else {
                    r.content.push(item);
                }
            }
            _ => runs.push(Run {
                props,
                content: vec![item],
            }),
        }
    }
    runs
}

/// A bound control found in the body.
#[derive(Debug, Clone)]
struct Found {
    cc: ContentControl,
    binding: DataBinding,
    first: ParaPath,
    start: usize,
    last: ParaPath,
    end: usize,
    text: String,
}

/// Text of an item as a bound value sees it.
fn push_value_text(c: &RunContent, out: &mut String) {
    match c {
        RunContent::LineBreak => out.push('\n'),
        RunContent::Image(_) | RunContent::Opaque(_) | RunContent::Markup(_) => {}
        other => other.push_text(out),
    }
}

/// Every outermost bound control of the body, in document order.
fn find_bound(doc: &Document) -> Vec<Found> {
    struct Open {
        found: Option<Found>,
        block: bool,
    }
    let mut out = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    for path in body_paragraphs(doc) {
        let Some(p) = para_at(doc, &path) else {
            continue;
        };
        let has_markers = p
            .runs
            .iter()
            .flat_map(|r| &r.content)
            .any(|c| matches!(c, RunContent::ControlStart(_) | RunContent::ControlEnd(_)));
        let collecting = stack.iter().any(|o| o.found.is_some());
        if !has_markers && !collecting {
            continue;
        }
        if collecting && stack.iter().any(|o| o.block && o.found.is_some()) {
            // A new paragraph inside a block-level bound control.
            if let Some(f) = stack.iter_mut().find_map(|o| o.found.as_mut()) {
                if f.first != path {
                    f.text.push('\n');
                }
            }
        }
        for (i, (props, c)) in atoms(p).iter().enumerate() {
            match c {
                RunContent::ControlStart(cc) => {
                    let outer_bound = stack.iter().any(|o| o.found.is_some());
                    let found = match (&cc.data_binding, outer_bound) {
                        (Some(b), false) => Some(Found {
                            cc: (**cc).clone(),
                            binding: b.clone(),
                            first: path.clone(),
                            start: i,
                            last: path.clone(),
                            end: i,
                            text: String::new(),
                        }),
                        _ => None,
                    };
                    stack.push(Open {
                        found,
                        block: cc.block,
                    });
                }
                RunContent::ControlEnd(block) => {
                    // An end closes the innermost control of its level.
                    let Some(k) = stack.iter().rposition(|o| o.block == *block) else {
                        continue;
                    };
                    let mut o = stack.remove(k);
                    stack.truncate(k);
                    if let Some(mut f) = o.found.take() {
                        f.last.clone_from(&path);
                        f.end = i;
                        out.push(f);
                    }
                }
                c if props.del.is_none() => {
                    if let Some(f) = stack.iter_mut().find_map(|o| o.found.as_mut()) {
                        push_value_text(c, &mut f.text);
                    }
                }
                _ => {}
            }
        }
        // Inline controls end in their paragraph; an unbalanced one is dropped.
        while stack.last().is_some_and(|o| !o.block) {
            stack.pop();
        }
    }
    out.sort_by(|a, b| (&a.first, a.start).cmp(&(&b.first, b.start)));
    out
}

fn parse_bool(v: &str) -> bool {
    matches!(v.trim(), "true" | "1")
}

/// Date of a value (`xsd:date` / `xsd:dateTime`, ISO 8601).
fn date_of(v: &str) -> Option<DateTime> {
    DateTime::parse_iso(v.trim())
}

/// What a control shows for a node value.
#[must_use]
pub fn display_value(cc: &ContentControl, value: &str, vi: bool) -> String {
    match &cc.kind {
        ControlKind::Checkbox { on, off, .. } => {
            if parse_bool(value) { *on } else { *off }.to_string()
        }
        ControlKind::DropDown(items) | ControlKind::ComboBox(items) => items
            .iter()
            .find(|(_, v)| v == value)
            .map_or_else(|| value.to_owned(), |(d, _)| d.clone()),
        ControlKind::Date { format, .. } => match date_of(value) {
            Some(d) => format_date(&d, format, vi),
            None => value.to_owned(),
        },
        _ => value.to_owned(),
    }
}

/// The node value for what a control holds (`old`: the node's current value).
#[must_use]
pub fn stored_value(cc: &ContentControl, text: &str, old: &str, vi: bool) -> String {
    match &cc.kind {
        ControlKind::Checkbox { checked, .. } => {
            let numeric = matches!(old.trim(), "0" | "1");
            match (numeric, *checked) {
                (true, true) => "1",
                (true, false) => "0",
                (false, true) => "true",
                (false, false) => "false",
            }
            .to_owned()
        }
        ControlKind::DropDown(items) | ControlKind::ComboBox(items) => items
            .iter()
            .find(|(d, _)| d == text)
            .map_or_else(|| text.to_owned(), |(_, v)| v.clone()),
        ControlKind::Date {
            format,
            full_date,
            store_as,
            ..
        } => {
            let picked = full_date
                .as_deref()
                .and_then(date_of)
                .filter(|d| format_date(d, format, vi) == text);
            match (picked, store_as.as_deref()) {
                (Some(d), Some("date")) => format!("{:04}-{:02}-{:02}", d.year, d.month, d.day),
                (Some(d), Some("dateTime")) => format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                    d.year, d.month, d.day, d.hour, d.minute, d.second
                ),
                _ => text.to_owned(),
            }
        }
        _ => text.to_owned(),
    }
}

/// The control shows what the node holds.
fn in_sync(f: &Found, value: &str, vi: bool) -> bool {
    match &f.cc.kind {
        ControlKind::Checkbox { checked, .. } => *checked == parse_bool(value),
        _ if f.cc.placeholder => value.is_empty(),
        _ => f.text == display_value(&f.cc, value, vi),
    }
}

/// The control's properties once it shows `value`.
fn refreshed(cc: &ContentControl, value: &str) -> ContentControl {
    let mut cc = cc.clone();
    match &mut cc.kind {
        ControlKind::Checkbox { checked, .. } => *checked = parse_bool(value),
        ControlKind::Date { full_date, .. } => {
            *full_date = date_of(value).map(|d| {
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    d.year, d.month, d.day, d.hour, d.minute, d.second
                )
            });
        }
        _ => {}
    }
    if !value.is_empty() {
        cc.placeholder = false;
    }
    cc
}

/// Items for a line of text (`\t` a tab).
fn line_atoms(line: &str, props: &RunProps, out: &mut Vec<Atom>) {
    for (k, part) in line.split('\t').enumerate() {
        if k > 0 {
            out.push((props.clone(), RunContent::Tab));
        }
        if !part.is_empty() {
            out.push((props.clone(), RunContent::Text(part.to_owned())));
        }
    }
}

/// Properties for the new content: the first text item's in the control, else the start's.
fn content_props(span: &[Atom], fallback: &RunProps) -> RunProps {
    span.iter()
        .find(|a| matches!(a.1, RunContent::Text(_)))
        .map_or_else(|| fallback.clone(), |a| a.0.clone())
}

fn split_path(p: &[usize]) -> (Vec<usize>, usize) {
    let (last, prefix) = p.split_last().map_or((0, &[][..]), |(l, r)| (*l, r));
    (prefix.to_vec(), last)
}

/// Replace a control's content by `display`; the edit is applied and returned.
fn rewrite(doc: &mut Document, f: &Found, cc: ContentControl, display: &str) -> Option<BlockEdit> {
    let (prefix, a) = split_path(&f.first);
    let (prefix_b, b) = split_path(&f.last);
    if prefix != prefix_b || b < a {
        return None;
    }
    let blocks = blocks_at_mut(doc, &prefix)?;
    let old: Vec<Block> = blocks.get(a..=b)?.to_vec();
    let paras: Vec<&Paragraph> = old.iter().filter_map(Block::as_paragraph).collect();
    if paras.len() != old.len() {
        return None;
    }
    let first = atoms(paras[0]);
    let last = atoms(paras[paras.len() - 1]);
    let start = first.get(f.start)?.clone();
    let insert = if cc.block {
        let span: Vec<Atom> = first[f.start + 1..].to_vec();
        let props = content_props(&span, &start.0);
        let lines: Vec<&str> = display.split('\n').collect();
        let n = lines.len();
        let mut out = Vec::with_capacity(n);
        for (k, line) in lines.iter().enumerate() {
            let mut items: Vec<Atom> = Vec::new();
            if k == 0 {
                items.extend_from_slice(&first[..f.start]);
                items.push((
                    start.0.clone(),
                    RunContent::ControlStart(Box::new(cc.clone())),
                ));
            }
            line_atoms(line, &props, &mut items);
            if k + 1 == n {
                items.extend_from_slice(last.get(f.end..)?);
            }
            let src = if k == 0 {
                paras[0]
            } else {
                paras[paras.len() - 1]
            };
            out.push(Block::Paragraph(Paragraph {
                props: src.props.clone(),
                runs: runs_of(items),
                src: (k == 0).then(|| paras[0].src.clone()).flatten(),
            }));
        }
        out
    } else {
        if a != b || f.end <= f.start {
            return None;
        }
        let span = &first[f.start + 1..f.end];
        let props = content_props(span, &start.0);
        let mut items: Vec<Atom> = first[..f.start].to_vec();
        items.push((start.0.clone(), RunContent::ControlStart(Box::new(cc))));
        for (k, line) in display.split('\n').enumerate() {
            if k > 0 {
                items.push((props.clone(), RunContent::LineBreak));
            }
            line_atoms(line, &props, &mut items);
        }
        items.extend_from_slice(&first[f.end..]);
        vec![Block::Paragraph(Paragraph {
            props: paras[0].props.clone(),
            runs: runs_of(items),
            src: paras[0].src.clone(),
        })]
    };
    if insert == old {
        return None;
    }
    blocks.splice(a..=b, insert.iter().cloned());
    Some(BlockEdit {
        path: prefix,
        index: a,
        remove: old,
        insert,
    })
}

/// Bring bound controls and their nodes in step (see the module documentation). `vi`: format
/// dates the Vietnamese way (month names).
pub fn sync(doc: &mut Document, mode: SyncMode, vi: bool) -> SyncOutcome {
    let mut outcome = SyncOutcome::default();
    if doc.custom_xml.is_empty() {
        return outcome;
    }
    let found = find_bound(doc);
    if found.is_empty() {
        return outcome;
    }
    let mut docs: Vec<Option<dom::XDocument>> =
        doc.custom_xml.iter().map(CustomXmlPart::parse).collect();
    let resolved: Vec<_> = found
        .iter()
        .map(|f| resolve_in(&doc.custom_xml, &docs, &f.binding))
        .collect();
    outcome.bound = resolved.iter().filter(|r| r.is_some()).count();
    outcome.unbound = found.len() - outcome.bound;
    // Node values after this pass, by (part, node).
    let mut values: BTreeMap<(usize, xpath::Target), String> = BTreeMap::new();
    let mut sources = vec![false; found.len()];
    let mut changed_parts = vec![false; docs.len()];
    if mode == SyncMode::FromControls {
        for (i, f) in found.iter().enumerate() {
            let Some(r) = &resolved[i] else { continue };
            let key = (r.part, r.target.clone());
            if values.contains_key(&key) || in_sync(f, &r.value, vi) {
                continue;
            }
            let new = stored_value(&f.cc, &f.text, &r.value, vi);
            let Some(d) = docs[r.part].as_mut() else {
                continue;
            };
            if new != r.value && xpath::set_value(d, &r.target, &new) {
                changed_parts[r.part] = true;
            }
            sources[i] = true;
            values.insert(key, new);
        }
    }
    // Refresh in reverse order so earlier locations stay valid.
    for (i, f) in found.iter().enumerate().rev() {
        let Some(r) = &resolved[i] else { continue };
        if sources[i] {
            continue;
        }
        let value = values
            .get(&(r.part, r.target.clone()))
            .unwrap_or(&r.value)
            .clone();
        if in_sync(f, &value, vi) {
            continue;
        }
        let cc = refreshed(&f.cc, &value);
        let shown = if cc.placeholder {
            f.text.clone()
        } else {
            display_value(&cc, &value, vi)
        };
        if let Some(e) = rewrite(doc, f, cc, &shown) {
            outcome.edits.push(e);
        }
    }
    if changed_parts.iter().any(|c| *c) {
        outcome.old_parts = Some(doc.custom_xml.clone());
        for (k, d) in docs.iter().enumerate() {
            if let (true, Some(d)) = (changed_parts[k], d) {
                doc.custom_xml[k].xml = dom::write(d);
            }
        }
    }
    outcome
}

/// Set the value of the node `xpath` (with `mappings`) of part `part`. Returns the parts
/// before when something changed.
///
/// # Errors
/// A message key: `badPart`, `badXml`, `badXPath`, `noNode`, `notLeaf`.
pub fn set_node_value(
    parts: &mut [CustomXmlPart],
    part: usize,
    path: &str,
    mappings: &str,
    value: &str,
) -> Result<Option<Vec<CustomXmlPart>>, &'static str> {
    let p = parts.get(part).ok_or("badPart")?;
    let mut d = p.parse().ok_or("badXml")?;
    let xp = xpath::XPath::parse(path).map_err(|_| "badXPath")?;
    let t = xpath::select(&d, &xp, &xpath::prefix_mappings(mappings)).ok_or("noNode")?;
    if xpath::value(&d, &t).as_deref() == Some(value) {
        return Ok(None);
    }
    if !xpath::set_value(&mut d, &t, value) {
        return Err("notLeaf");
    }
    let old = parts.to_vec();
    parts[part].xml = dom::write(&d);
    Ok(Some(old))
}

/// Binding status of a control: `Some(true)` bound to an existing node, `Some(false)` bound
/// to a node that does not exist, `None` not bound.
#[must_use]
pub fn status(doc: &Document, cc: &ContentControl) -> Option<bool> {
    let b = cc.data_binding.as_ref()?;
    Some(super::resolve(&doc.custom_xml, b).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Paragraph, Section};

    const ID: &str = "{6A1C0DE1-0000-4000-8000-000000000001}";

    fn cc(kind: ControlKind, xpath: &str, block: bool) -> ContentControl {
        ContentControl {
            id: Some(1),
            alias: None,
            tag: None,
            kind,
            lock: None,
            placeholder: false,
            block,
            props_xml: None,
            data_binding: Some(DataBinding {
                xpath: xpath.into(),
                prefix_mappings: "xmlns:ns0='urn:t'".into(),
                store_item_id: Some(ID.into()),
            }),
            appearance: None,
            color: None,
            temporary: false,
        }
    }

    fn text(t: &str) -> Run {
        Run {
            props: RunProps::default(),
            content: vec![RunContent::Text(t.into())],
        }
    }

    fn marker(c: RunContent) -> Run {
        Run {
            props: RunProps::default(),
            content: vec![c],
        }
    }

    fn inline(before: &str, c: ContentControl, inner: &str) -> Paragraph {
        Paragraph::new(
            crate::ParaProps::default(),
            vec![
                text(before),
                marker(RunContent::ControlStart(Box::new(c))),
                text(inner),
                marker(RunContent::ControlEnd(false)),
                text("."),
            ],
        )
    }

    fn doc_with(paras: Vec<Paragraph>, xml: &str) -> Document {
        Document {
            sections: vec![Section::new(
                paras.into_iter().map(Block::Paragraph).collect(),
            )],
            custom_xml: vec![CustomXmlPart {
                part_name: "/customXml/item1.xml".into(),
                xml: xml.into(),
                item_id: Some(ID.into()),
                schema_refs: vec![],
                props_part: None,
            }],
            ..Document::default()
        }
    }

    fn texts(doc: &Document) -> Vec<String> {
        body_paragraphs(doc)
            .iter()
            .filter_map(|p| para_at(doc, p))
            .map(Paragraph::plain_text)
            .collect()
    }

    const XML: &str =
        "<r xmlns=\"urn:t\"><name>An</name><ok>false</ok><d>2026-03-05</d><c>b</c><missing/></r>";

    #[test]
    fn open_refreshes_controls_from_their_nodes() {
        let date = ControlKind::Date {
            format: "dd/MM/yyyy".into(),
            full_date: None,
            store_as: Some("date".into()),
            lid: None,
        };
        let mut doc = doc_with(
            vec![
                inline(
                    "Name: ",
                    cc(
                        ControlKind::Text { multi_line: false },
                        "/ns0:r/ns0:name",
                        false,
                    ),
                    "old",
                ),
                inline("Date: ", cc(date, "/ns0:r/ns0:d", false), "x"),
                inline(
                    "Pick: ",
                    cc(
                        ControlKind::DropDown(vec![
                            ("Alpha".into(), "a".into()),
                            ("Beta".into(), "b".into()),
                        ]),
                        "/ns0:r/ns0:c",
                        false,
                    ),
                    "Alpha",
                ),
                inline(
                    "Gone: ",
                    cc(ControlKind::RichText, "/ns0:r/ns0:nothere", false),
                    "kept",
                ),
            ],
            XML,
        );
        let out = sync(&mut doc, SyncMode::FromParts, false);
        assert_eq!((out.bound, out.unbound), (3, 1));
        assert_eq!(out.edits.len(), 3);
        assert!(out.old_parts.is_none());
        assert_eq!(
            texts(&doc),
            [
                "Name: An.",
                "Date: 05/03/2026.",
                "Pick: Beta.",
                "Gone: kept."
            ]
        );
        // In step now: nothing more to do.
        assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
    }

    #[test]
    fn edited_control_updates_its_node_and_the_other_controls() {
        let t = || {
            cc(
                ControlKind::Text { multi_line: false },
                "/ns0:r/ns0:name",
                false,
            )
        };
        let mut doc = doc_with(vec![inline("A ", t(), "An"), inline("B ", t(), "An")], XML);
        assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
        // The user types in the second control.
        let edited = inline("B ", t(), "Bình");
        doc.sections[0].blocks[1] = Block::Paragraph(edited);
        let out = sync(&mut doc, SyncMode::FromControls, false);
        let old = out.old_parts.expect("part changed");
        assert!(old[0].xml.contains("<name>An</name>"));
        assert!(doc.custom_xml[0].xml.contains("<name>Bình</name>"));
        assert_eq!(texts(&doc), ["A Bình.", "B Bình."]);
        assert_eq!(out.edits.len(), 1);
    }

    #[test]
    fn checkbox_and_date_store_their_formats() {
        let mut check = cc(
            ControlKind::Checkbox {
                checked: false,
                on: '\u{2612}',
                off: '\u{2610}',
            },
            "/ns0:r/ns0:ok",
            false,
        );
        let mut doc = doc_with(vec![inline("", check.clone(), "\u{2610}")], XML);
        assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
        check.kind = ControlKind::Checkbox {
            checked: true,
            on: '\u{2612}',
            off: '\u{2610}',
        };
        doc.sections[0].blocks[0] = Block::Paragraph(inline("", check, "\u{2612}"));
        sync(&mut doc, SyncMode::FromControls, false);
        assert!(doc.custom_xml[0].xml.contains("<ok>true</ok>"));

        let date = |full: Option<&str>, store: Option<&str>| {
            cc(
                ControlKind::Date {
                    format: "d/M/yyyy".into(),
                    full_date: full.map(str::to_owned),
                    store_as: store.map(str::to_owned),
                    lid: None,
                },
                "/ns0:r/ns0:d",
                false,
            )
        };
        for (store, expect) in [
            (Some("date"), "<d>2026-12-01</d>"),
            (Some("dateTime"), "<d>2026-12-01T00:00:00</d>"),
            (None, "<d>1/12/2026</d>"),
        ] {
            let mut doc = doc_with(vec![inline("", date(None, store), "5/3/2026")], XML);
            assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
            doc.sections[0].blocks[0] = Block::Paragraph(inline(
                "",
                date(Some("2026-12-01T00:00:00Z"), store),
                "1/12/2026",
            ));
            sync(&mut doc, SyncMode::FromControls, false);
            assert!(doc.custom_xml[0].xml.contains(expect), "{store:?}");
        }
    }

    #[test]
    fn placeholder_controls_wait_for_a_value() {
        let mut c = cc(
            ControlKind::Text { multi_line: false },
            "/ns0:r/ns0:missing",
            false,
        );
        c.placeholder = true;
        let mut doc = doc_with(vec![inline("", c.clone(), "Type here")], XML);
        // Empty node: the placeholder stays, and typing nothing pushes nothing.
        assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
        assert!(sync(&mut doc, SyncMode::FromControls, false)
            .old_parts
            .is_none());
        set_node_value(
            &mut doc.custom_xml,
            0,
            "/ns0:r/ns0:missing",
            "xmlns:ns0='urn:t'",
            "v",
        )
        .expect("set")
        .expect("changed");
        sync(&mut doc, SyncMode::FromParts, false);
        assert_eq!(texts(&doc), ["v."]);
    }

    #[test]
    fn block_control_takes_one_paragraph_per_line() {
        let c = cc(ControlKind::RichText, "/ns0:r/ns0:name", true);
        let paras = vec![
            Paragraph::new(
                crate::ParaProps::default(),
                vec![marker(RunContent::ControlStart(Box::new(c))), text("one")],
            ),
            Paragraph::new(
                crate::ParaProps::default(),
                vec![text("two"), marker(RunContent::ControlEnd(true))],
            ),
            Paragraph::new(crate::ParaProps::default(), vec![text("after")]),
        ];
        let mut doc = doc_with(paras, "<r xmlns=\"urn:t\"><name>x\ny\nz</name></r>");
        let out = sync(&mut doc, SyncMode::FromParts, false);
        assert_eq!(out.edits.len(), 1);
        assert_eq!(texts(&doc), ["x", "y", "z", "after"]);
        // Its text now matches the node.
        assert_eq!(sync(&mut doc, SyncMode::FromParts, false).edits.len(), 0);
        // Editing the last line goes back to the node.
        if let Block::Paragraph(p) = &mut doc.sections[0].blocks[2] {
            p.runs.insert(0, text("Z"));
        }
        sync(&mut doc, SyncMode::FromControls, false);
        assert!(doc.custom_xml[0].xml.contains("<name>x\ny\nZz</name>"));
    }

    #[test]
    fn set_node_value_errors() {
        let mut parts = doc_with(vec![], XML).custom_xml;
        assert_eq!(set_node_value(&mut parts, 3, "/a", "", "v"), Err("badPart"));
        assert_eq!(set_node_value(&mut parts, 0, "a", "", "v"), Err("badXPath"));
        assert_eq!(
            set_node_value(&mut parts, 0, "/ns0:q", "xmlns:ns0='urn:t'", "v"),
            Err("noNode")
        );
        assert_eq!(
            set_node_value(&mut parts, 0, "/ns0:r", "xmlns:ns0='urn:t'", "v"),
            Err("notLeaf")
        );
        assert_eq!(
            set_node_value(&mut parts, 0, "/ns0:r/ns0:c", "xmlns:ns0='urn:t'", "b"),
            Ok(None)
        );
    }
}

//! Atomic, invertible operations on a [`Document`] (prompt §5.2).
//!
//! Every edit is a [`Transaction`] of [`Op`]s; each op carries what it removes,
//! so its inverse is exact and undo/redo is unlimited. Ops address block
//! containers by path (`[section, (table block, row, cell)*]`, see
//! `bo_model::index`), which maps one-to-one to CRDT operations later (Phase 6).

use crate::EditError;
use bo_model::change::BlockChanges;
use bo_model::custom_xml::CustomXmlPart;
use bo_model::index::blocks_at_mut;
use bo_model::{
    Block, Comment, DocDesign, Document, Hyphenation, Numbering, Section, StyleSheet, ThemeFonts,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// An atomic edit.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Replace `remove` (currently at `index`) by `insert` in the block container at `path`.
    Blocks {
        /// Path prefix addressing the block container (see `bo_model::index::blocks_at`).
        path: Vec<usize>,
        /// Index of the first replaced block.
        index: usize,
        /// Blocks removed.
        remove: Vec<Block>,
        /// Blocks inserted in their place.
        insert: Vec<Block>,
    },
    /// Replace sections starting at `index`.
    Sections {
        /// Index of the first replaced section.
        index: usize,
        /// Sections removed.
        remove: Vec<Section>,
        /// Sections inserted in their place.
        insert: Vec<Section>,
    },
    /// Add, replace or remove a media part.
    Media {
        /// Part name of the media.
        name: String,
        /// Previous bytes (`None` = absent).
        old: Option<Arc<Vec<u8>>>,
        /// New bytes (`None` = removed).
        new: Option<Arc<Vec<u8>>>,
    },
    /// Replace the numbering definitions.
    Numbering {
        /// Previous definitions.
        old: Box<Numbering>,
        /// New definitions.
        new: Box<Numbering>,
    },
    /// Replace the style sheet.
    Styles {
        /// Previous style sheet.
        old: Box<StyleSheet>,
        /// New style sheet.
        new: Box<StyleSheet>,
    },
    /// Replace the comments (§17.13.4).
    Comments {
        /// Previous comments.
        old: Vec<Comment>,
        /// New comments.
        new: Vec<Comment>,
    },
    /// Replace the footnotes (`endnote` false) or endnotes (§17.11).
    Notes {
        /// `true` for endnotes, `false` for footnotes.
        endnote: bool,
        /// Previous notes by id.
        old: BTreeMap<i32, Vec<Block>>,
        /// New notes by id.
        new: BTreeMap<i32, Vec<Block>>,
    },
    /// Set the evenAndOddHeaders document setting (§17.15.1.35).
    EvenAndOddHeaders {
        /// Previous value.
        old: bool,
        /// New value.
        new: bool,
    },
    /// Replace the footnote continuation notice (§17.18.33 `continuationNotice`).
    ContinuationNotice {
        /// Previous notice.
        old: Option<Vec<Block>>,
        /// New notice.
        new: Option<Vec<Block>>,
    },
    /// Replace the page colour / theme settings and the theme font scheme
    /// (§17.2.1 background, §17.15.1.32 displayBackgroundShape, DrawingML theme).
    Design {
        /// Previous design and theme fonts.
        old: Box<(DocDesign, ThemeFonts)>,
        /// New design and theme fonts.
        new: Box<(DocDesign, ThemeFonts)>,
    },
    /// Replace the hyphenation settings (§17.15.1.10 autoHyphenation, …).
    Hyphenation {
        /// Previous settings.
        old: Hyphenation,
        /// New settings.
        new: Hyphenation,
    },
    /// Replace the custom XML parts (ECMA-376 Part 1, §15.2.4 / §15.2.5).
    CustomXml {
        /// Previous parts.
        old: Vec<CustomXmlPart>,
        /// New parts.
        new: Vec<CustomXmlPart>,
    },
    /// Replace the building blocks of the document (ECMA-376 Part 1, §17.12
    /// glossary document).
    Glossary {
        /// Previous glossary.
        old: Option<std::sync::Arc<bo_model::glossary::Glossary>>,
        /// New glossary.
        new: Option<std::sync::Arc<bo_model::glossary::Glossary>>,
    },
}

impl Op {
    /// The exact inverse.
    #[must_use]
    pub fn inverse(&self) -> Op {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => Op::Blocks {
                path: path.clone(),
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            Op::Sections {
                index,
                remove,
                insert,
            } => Op::Sections {
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            Op::Media { name, old, new } => Op::Media {
                name: name.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Op::Numbering { old, new } => Op::Numbering {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Styles { old, new } => Op::Styles {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Comments { old, new } => Op::Comments {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Notes { endnote, old, new } => Op::Notes {
                endnote: *endnote,
                old: new.clone(),
                new: old.clone(),
            },
            Op::EvenAndOddHeaders { old, new } => Op::EvenAndOddHeaders {
                old: *new,
                new: *old,
            },
            Op::ContinuationNotice { old, new } => Op::ContinuationNotice {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Design { old, new } => Op::Design {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Hyphenation { old, new } => Op::Hyphenation {
                old: *new,
                new: *old,
            },
            Op::CustomXml { old, new } => Op::CustomXml {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Glossary { old, new } => Op::Glossary {
                old: new.clone(),
                new: old.clone(),
            },
        }
    }

    /// Record which top-level blocks this op changes (incremental layout). Without
    /// the document a style sheet or numbering change marks everything;
    /// [`record_ops`] narrows it.
    pub fn record(&self, changes: &mut BlockChanges) {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => match path.as_slice() {
                [section] => changes.splice(*section, *index, remove.len(), insert.len()),
                // Nested container (table cell): its top-level table block changed.
                [section, block, ..] => changes.splice(*section, *block, 1, 1),
                [] => changes.mark_all(),
            },
            // Header/footer edits (ECMA-376 Part 1, §17.10): the section is replaced
            // by a copy that differs only there.
            Op::Sections {
                index,
                remove,
                insert,
            } if remove.len() == insert.len()
                && remove
                    .iter()
                    .zip(insert)
                    .all(|(a, b)| a.same_except_headers(b)) =>
            {
                changes.mark_headers(*index);
            }
            // §17.15.1.35 evenAndOddHeaders only selects which header a page shows.
            Op::EvenAndOddHeaders { .. } => changes.mark_headers(0),
            // Data parts are not laid out (bound controls change through `Blocks`).
            Op::CustomXml { .. } | Op::Glossary { .. } => {}
            _ => changes.mark_all(),
        }
    }

    /// Apply to `doc`, checking that the removed content matches.
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError> {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => {
                let blocks = blocks_at_mut(doc, path).ok_or(EditError::BadPath)?;
                let end = index + remove.len();
                if end > blocks.len() || blocks[*index..end] != remove[..] {
                    return Err(EditError::Conflict);
                }
                blocks.splice(*index..end, insert.iter().cloned());
            }
            Op::Sections {
                index,
                remove,
                insert,
            } => {
                let end = index + remove.len();
                if end > doc.sections.len() || doc.sections[*index..end] != remove[..] {
                    return Err(EditError::Conflict);
                }
                doc.sections.splice(*index..end, insert.iter().cloned());
            }
            Op::Media { name, old, new } => {
                if doc.media.get(name) != old.as_ref() {
                    return Err(EditError::Conflict);
                }
                match new {
                    Some(b) => doc.media.insert(name.clone(), b.clone()),
                    None => doc.media.remove(name),
                };
            }
            Op::Numbering { old, new } => {
                if doc.numbering != **old {
                    return Err(EditError::Conflict);
                }
                doc.numbering = (**new).clone();
            }
            Op::Styles { old, new } => {
                if doc.styles != **old {
                    return Err(EditError::Conflict);
                }
                doc.styles = (**new).clone();
            }
            Op::Comments { old, new } => {
                if doc.comments != *old {
                    return Err(EditError::Conflict);
                }
                doc.comments.clone_from(new);
            }
            Op::Notes { endnote, old, new } => {
                let notes = if *endnote {
                    &mut doc.endnotes
                } else {
                    &mut doc.footnotes
                };
                if notes != old {
                    return Err(EditError::Conflict);
                }
                notes.clone_from(new);
            }
            Op::EvenAndOddHeaders { old, new } => {
                if doc.even_and_odd_headers != *old {
                    return Err(EditError::Conflict);
                }
                doc.even_and_odd_headers = *new;
            }
            Op::ContinuationNotice { old, new } => {
                if doc.continuation_notice != *old {
                    return Err(EditError::Conflict);
                }
                doc.continuation_notice.clone_from(new);
            }
            Op::Design { old, new } => {
                if doc.design != old.0 || doc.theme_fonts != old.1 {
                    return Err(EditError::Conflict);
                }
                doc.design = new.0.clone();
                doc.theme_fonts = new.1.clone();
            }
            Op::Hyphenation { old, new } => {
                if doc.hyphenation != *old {
                    return Err(EditError::Conflict);
                }
                doc.hyphenation = *new;
            }
            Op::CustomXml { old, new } => {
                if doc.custom_xml != *old {
                    return Err(EditError::Conflict);
                }
                doc.custom_xml.clone_from(new);
            }
            Op::Glossary { old, new } => {
                if doc.glossary != *old {
                    return Err(EditError::Conflict);
                }
                doc.glossary.clone_from(new);
            }
        }
        Ok(())
    }
}

/// Caret position: body paragraph id (document order) and byte offset in its index text.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Pos {
    /// Body paragraph id (document order).
    #[serde(rename = "p")]
    pub para: usize,
    /// Byte offset in the paragraph's index text.
    #[serde(rename = "o")]
    pub offset: usize,
}

impl Pos {
    /// Position at byte `offset` of body paragraph `para`.
    pub fn new(para: usize, offset: usize) -> Pos {
        Pos { para, offset }
    }
}

/// Selection: `anchor` stays put, `focus` moves with Shift+arrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Selection {
    /// End that stays put.
    pub anchor: Pos,
    /// End that moves with the caret.
    pub focus: Pos,
}

impl Selection {
    /// Collapsed selection at `p`.
    pub fn caret(p: Pos) -> Selection {
        Selection {
            anchor: p,
            focus: p,
        }
    }
    /// `anchor` and `focus` coincide.
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
    /// (start, end) in document order.
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
}

/// A group of ops undone/redone together.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Ops in application order.
    pub ops: Vec<Op>,
    /// Selection before the transaction (restored on undo).
    pub before: Selection,
    /// Selection after the transaction (restored on redo).
    pub after: Selection,
}

impl Transaction {
    /// Apply all ops; on failure the document is restored.
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError> {
        for (i, op) in self.ops.iter().enumerate() {
            if let Err(e) = op.apply(doc) {
                for done in self.ops[..i].iter().rev() {
                    let _ = done.inverse().apply(doc);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// The inverse transaction.
    #[must_use]
    pub fn inverse(&self) -> Transaction {
        Transaction {
            ops: self.ops.iter().rev().map(Op::inverse).collect(),
            before: self.after,
            after: self.before,
        }
    }
}

/// Record which top-level blocks `ops` change (incremental layout), with `doc`
/// the document after them. Like [`Op::record`] for every op, except that a
/// style sheet replacement marks only the body blocks that use a changed
/// style (see [`record_styles`]); those are recorded last, in final indexes.
pub fn record_ops(ops: &[Op], doc: &Document, changes: &mut BlockChanges) {
    let mut sheets = Vec::new();
    let mut lists = Vec::new();
    let only_sections = ops.iter().all(|o| matches!(o, Op::Sections { .. }));
    for op in ops {
        match op {
            Op::Sections {
                index,
                remove,
                insert,
            } if only_sections && record_section_props(*index, remove, insert, doc, changes) => {}
            Op::Styles { old, new } => sheets.push((&**old, &**new)),
            Op::Numbering { old, new } => lists.push((&**old, &**new)),
            _ => op.record(changes),
        }
    }
    for (old, new) in sheets {
        record_styles(old, new, doc, changes);
    }
    for (old, new) in lists {
        record_numbering(old, new, doc, changes);
    }
}

/// Numbering instance ids whose labels may change between two numbering
/// definitions: list counters continue per abstract definition (ECMA-376 Part 1,
/// §17.9.1 abstractNum, §17.9.15 num), so every instance of an abstract that
/// changed, or that a changed instance used or now uses.
fn changed_lists(old: &Numbering, new: &Numbering) -> BTreeSet<i32> {
    let mut abstracts = BTreeSet::new();
    for a in &old.abstracts {
        if new.abstracts.iter().find(|b| b.id == a.id) != Some(a) {
            abstracts.insert(a.id);
        }
    }
    for a in &new.abstracts {
        if !old.abstracts.iter().any(|b| b.id == a.id) {
            abstracts.insert(a.id);
        }
    }
    let mut nums = BTreeSet::new();
    for (x, y) in [(old, new), (new, old)] {
        for n in &x.nums {
            if y.nums.iter().find(|m| m.id == n.id) != Some(n) {
                nums.insert(n.id);
                abstracts.insert(n.abstract_id);
            }
        }
    }
    for sheet in [old, new] {
        for n in &sheet.nums {
            if abstracts.contains(&n.abstract_id) {
                nums.insert(n.id);
            }
        }
    }
    nums
}

/// Does any of `blocks` show a list label of one of the `lists`: a paragraph
/// numbered directly (§17.3.1.19 numPr) or through its paragraph style
/// (`styled`: the style ids whose resolved numbering is one of them, `default`:
/// the default paragraph style is one of those). Text boxes count as using
/// every list (their content is not scanned).
fn blocks_number(
    blocks: &[Block],
    lists: &BTreeSet<i32>,
    styled: &BTreeSet<String>,
    default: bool,
) -> bool {
    blocks.iter().any(|b| match b {
        Block::Paragraph(p) => {
            let listed = match (p.props.num, &p.props.style) {
                (Some(n), _) => lists.contains(&n.num_id),
                (None, Some(id)) => styled.contains(id),
                (None, None) => default,
            };
            listed
                || p.runs.iter().any(|r| {
                    r.content
                        .iter()
                        .any(|c| matches!(c, bo_model::RunContent::Image(i) if i.shape.is_some()))
                })
        }
        Block::Table(t) => t
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .any(|c| blocks_number(&c.blocks, lists, styled, default)),
        Block::Opaque(_) => false,
    })
}

/// Record a numbering replacement: the top-level body blocks showing a label of
/// a changed list (and, through the flow's list counters, the later ones of the
/// same lists). Everything is marked when headers, footers, notes or comments
/// show such labels.
pub fn record_numbering(
    old: &Numbering,
    new: &Numbering,
    doc: &Document,
    changes: &mut BlockChanges,
) {
    if changes.is_all() {
        return;
    }
    changes.mark_parts();
    let lists = changed_lists(old, new);
    if lists.is_empty() {
        return;
    }
    // Paragraph styles whose resolved numbering (§17.7.4.17 with pPr/numPr) is a changed list.
    let styled: BTreeSet<String> = doc
        .styles
        .styles
        .iter()
        .filter(|s| s.kind == bo_model::StyleKind::Paragraph)
        .filter(|s| {
            doc.styles
                .resolve_paragraph(&s.id)
                .0
                .num
                .is_some_and(|n| lists.contains(&n.num_id))
        })
        .map(|s| s.id.clone())
        .collect();
    let default = doc
        .styles
        .default_of(bo_model::StyleKind::Paragraph)
        .is_some_and(|s| styled.contains(&s.id));
    let uses = |b: &[Block]| blocks_number(b, &lists, &styled, default);
    let elsewhere = doc.sections.iter().any(|s| {
        [&s.headers, &s.footers]
            .into_iter()
            .flat_map(|hf| [&hf.default, &hf.first, &hf.even])
            .flatten()
            .any(|b| uses(b))
    }) || doc
        .footnotes
        .values()
        .chain(doc.endnotes.values())
        .any(|b| uses(b))
        || doc.comments.iter().any(|c| uses(&c.blocks));
    if elsewhere {
        changes.mark_all();
        return;
    }
    for (si, s) in doc.sections.iter().enumerate() {
        for (bi, b) in s.blocks.iter().enumerate() {
            if uses(std::slice::from_ref(b)) {
                changes.splice(si, bi, 1, 1);
            }
        }
    }
}

/// Record a replacement of sections that changes only their page setup, columns
/// or break type (ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar, §17.6.4 cols,
/// §17.6.22 type) and/or their headers and footers. The flow restarts at the last
/// block of the previous non-empty section (a section's first checkpoint is taken
/// after its section break) and may resynchronise only after the changed
/// section; its headers are measured again. `false` (nothing recorded) for any
/// other change: blocks, page numbering, note properties, tracked section changes,
/// or a change of the first section.
fn record_section_props(
    index: usize,
    remove: &[Section],
    insert: &[Section],
    doc: &Document,
    changes: &mut BlockChanges,
) -> bool {
    if remove.len() != insert.len() || changes.is_all() {
        return false;
    }
    let simple = remove.iter().zip(insert).all(|(a, b)| {
        a.blocks == b.blocks
            && a.page_number_start == b.page_number_start
            && a.footnote_pr == b.footnote_pr
            && a.endnote_pr == b.endnote_pr
            && a.change == b.change
    });
    if !simple {
        return false;
    }
    let mut marks = Vec::new();
    for (i, (a, b)) in remove.iter().zip(insert).enumerate() {
        let si = index + i;
        if a.same_except_headers(b) {
            marks.push((si, None));
            continue;
        }
        let Some(prev) = (0..si).rev().find(|&s| {
            doc.sections
                .get(s)
                .is_some_and(|sec| !sec.blocks.is_empty())
        }) else {
            return false;
        };
        let Some(sec) = doc.sections.get(si) else {
            return false;
        };
        marks.push((si, Some((prev, sec.blocks.len()))));
    }
    for (si, props) in marks {
        if let Some((prev, n)) = props {
            let last = doc.sections[prev].blocks.len() - 1;
            changes.splice(prev, last, 1, 1);
            changes.splice(si, 0, n, n);
        }
        changes.mark_headers(si);
    }
    true
}

/// Style ids whose effective definition differs between two style sheets: the
/// styles added, removed or changed, and every style based on one of them
/// (ECMA-376 Part 1, §17.7.4.3 basedOn) in either sheet.
fn changed_styles(old: &StyleSheet, new: &StyleSheet) -> BTreeSet<String> {
    let mut direct = BTreeSet::new();
    for s in &old.styles {
        if new.styles.iter().find(|n| n.id == s.id) != Some(s) {
            direct.insert(s.id.clone());
        }
    }
    for s in &new.styles {
        if !old.styles.iter().any(|o| o.id == s.id) {
            direct.insert(s.id.clone());
        }
    }
    if direct.is_empty() {
        return direct;
    }
    let mut out = direct.clone();
    for sheet in [old, new] {
        for s in &sheet.styles {
            if sheet.chain(&s.id).iter().any(|c| direct.contains(&c.id)) {
                out.insert(s.id.clone());
            }
        }
    }
    out
}

/// Does any of `blocks` use one of the `styles` (paragraph, character or table
/// style, §17.7.4.17 pStyle / §17.7.4.24 rStyle / §17.7.4.47 tblStyle)? Text
/// boxes count as using every style (their content is not scanned).
fn blocks_use(blocks: &[Block], styles: &BTreeSet<String>) -> bool {
    let uses = |s: &Option<String>| s.as_ref().is_some_and(|id| styles.contains(id));
    blocks.iter().any(|b| match b {
        Block::Paragraph(p) => {
            uses(&p.props.style)
                || p.runs.iter().any(|r| {
                    uses(&r.props.style)
                        || r.content.iter().any(
                            |c| matches!(c, bo_model::RunContent::Image(i) if i.shape.is_some()),
                        )
                })
        }
        Block::Table(t) => {
            uses(&t.props.style)
                || t.rows
                    .iter()
                    .flat_map(|r| &r.cells)
                    .any(|c| blocks_use(&c.blocks, styles))
        }
        Block::Opaque(_) => false,
    })
}

/// Record a style sheet replacement: the top-level body blocks using a changed
/// style. Everything is marked when a default style (used by content naming no
/// style, §17.7.4.17) changed, or when headers, footers, notes or comments use a
/// changed style.
pub fn record_styles(
    old: &StyleSheet,
    new: &StyleSheet,
    doc: &Document,
    changes: &mut BlockChanges,
) {
    if changes.is_all() {
        return;
    }
    changes.mark_parts();
    let styles = changed_styles(old, new);
    if styles.is_empty() {
        return;
    }
    let default_changed = [
        bo_model::StyleKind::Paragraph,
        bo_model::StyleKind::Character,
        bo_model::StyleKind::Table,
    ]
    .into_iter()
    .any(|k| {
        let (a, b) = (old.default_of(k), new.default_of(k));
        a.map(|s| &s.id) != b.map(|s| &s.id)
            || a.is_some_and(|s| styles.contains(&s.id))
            || b.is_some_and(|s| styles.contains(&s.id))
    });
    let elsewhere = || {
        doc.sections.iter().any(|s| {
            [&s.headers, &s.footers]
                .into_iter()
                .flat_map(|hf| [&hf.default, &hf.first, &hf.even])
                .flatten()
                .any(|b| blocks_use(b, &styles))
        }) || doc
            .footnotes
            .values()
            .chain(doc.endnotes.values())
            .any(|b| blocks_use(b, &styles))
            || doc.comments.iter().any(|c| blocks_use(&c.blocks, &styles))
    };
    if default_changed || elsewhere() {
        changes.mark_all();
        return;
    }
    for (si, s) in doc.sections.iter().enumerate() {
        for (bi, b) in s.blocks.iter().enumerate() {
            if blocks_use(std::slice::from_ref(b), &styles) {
                changes.splice(si, bi, 1, 1);
            }
        }
    }
}

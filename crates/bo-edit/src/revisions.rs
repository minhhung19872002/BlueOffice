//! Tracked changes (ECMA-376 Part 1, §17.13.5): listing revisions, accepting or
//! rejecting them, and the primitives used to record new edits as revisions.
//!
//! Revisions live on the model: run insertions/deletions/moves and formatting
//! changes in [`RunProps`] (`ins`, `del`, `change`), paragraph-mark insertions /
//! deletions and paragraph formatting changes in [`ParaProps`].

use crate::para::{self, Atom};
use crate::Pos;
use bo_model::index::body_paragraphs;
use bo_model::{Block, Document, ParaProps, Paragraph, RevMark, RevisionInfo, RunProps};
use serde::Serialize;

/// Kind of a listed revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RevKind {
    /// §17.13.5.18 ins
    Insert,
    /// §17.13.5.14 del
    Delete,
    /// §17.13.5.25 moveTo
    MoveTo,
    /// §17.13.5.22 moveFrom
    MoveFrom,
    /// §17.13.5.31 rPrChange
    Format,
    /// §17.13.5.29 pPrChange
    ParagraphFormat,
    /// §17.13.5.16 ins on the paragraph mark
    ParagraphInsert,
    /// §17.13.5.15 del on the paragraph mark
    ParagraphDelete,
}

/// One revision of the main text, for the review pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionItem {
    pub id: i32,
    pub kind: RevKind,
    pub author: String,
    pub date: Option<String>,
    pub start: Pos,
    pub end: Pos,
    /// Affected text (shortened).
    pub text: String,
}

const MAX_TEXT: usize = 80;

fn push_text(item: &mut RevisionItem, t: &str) {
    if item.text.chars().count() < MAX_TEXT {
        item.text.extend(t.chars().take(MAX_TEXT));
    }
}

/// All revisions of the main text in document order; parts of one revision (same
/// id and kind) spread over several runs or paragraphs are reported once.
pub fn list(doc: &Document) -> Vec<RevisionItem> {
    let mut out: Vec<RevisionItem> = Vec::new();
    let mut add = |kind: RevKind, info: &RevisionInfo, start: Pos, end: Pos, text: &str| {
        if let Some(it) = out
            .iter_mut()
            .rev()
            .find(|i| i.id == info.id && i.kind == kind && i.author == info.author)
        {
            it.end = it.end.max(end);
            push_text(it, text);
            return;
        }
        let mut it = RevisionItem {
            id: info.id,
            kind,
            author: info.author.clone(),
            date: info.date.clone(),
            start,
            end,
            text: String::new(),
        };
        push_text(&mut it, text);
        out.push(it);
    };
    for (pid, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = bo_model::index::para_at(doc, path) else {
            continue;
        };
        let atoms = para::flatten(p);
        let len = para::len(&atoms);
        if let Some(c) = &p.props.change {
            add(
                RevKind::ParagraphFormat,
                &c.info,
                Pos::new(pid, 0),
                Pos::new(pid, len),
                "",
            );
        }
        let mut off = 0;
        for (props, item) in &atoms {
            let l = para::atom_len(item);
            let text = bo_model::index::item_text(item);
            let (s, e) = (Pos::new(pid, off), Pos::new(pid, off + l));
            if let Some(m) = &props.ins {
                let k = if m.moved {
                    RevKind::MoveTo
                } else {
                    RevKind::Insert
                };
                add(k, &m.info, s, e, &text);
            }
            if let Some(m) = &props.del {
                let k = if m.moved {
                    RevKind::MoveFrom
                } else {
                    RevKind::Delete
                };
                add(k, &m.info, s, e, &text);
            }
            if let Some(c) = &props.change {
                add(RevKind::Format, &c.info, s, e, &text);
            }
            off += l;
        }
        for (m, k) in [
            (&p.props.mark_ins, RevKind::ParagraphInsert),
            (&p.props.mark_del, RevKind::ParagraphDelete),
        ] {
            if let Some(m) = m {
                add(k, &m.info, Pos::new(pid, len), Pos::new(pid + 1, 0), "¶");
            }
        }
    }
    out
}

/// Largest revision id used in the document (0 when there is none).
pub fn max_id(doc: &Document) -> i32 {
    fn blocks(v: &[Block], m: &mut i32) {
        for b in v {
            match b {
                Block::Paragraph(p) => {
                    for x in [&p.props.mark_ins, &p.props.mark_del].into_iter().flatten() {
                        *m = (*m).max(x.info.id);
                    }
                    if let Some(c) = &p.props.change {
                        *m = (*m).max(c.info.id);
                    }
                    for r in &p.runs {
                        for x in [&r.props.ins, &r.props.del].into_iter().flatten() {
                            *m = (*m).max(x.info.id);
                        }
                        if let Some(c) = &r.props.change {
                            *m = (*m).max(c.info.id);
                        }
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for c in &row.cells {
                            blocks(&c.blocks, m);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut m = 0;
    for s in &doc.sections {
        blocks(&s.blocks, &mut m);
    }
    m
}

/// Which revisions to resolve.
pub(crate) type Filter<'a> = &'a dyn Fn(i32) -> bool;

/// Accept (`accept`) or reject the revisions selected by `f` in a block container.
/// Paragraphs whose mark disappears are merged with the following paragraph of the
/// same container (the merged paragraph keeps the following paragraph's properties,
/// which the remaining mark belongs to).
pub(crate) fn resolve_blocks(blocks: &[Block], f: Filter<'_>, accept: bool) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::with_capacity(blocks.len());
    // Content of preceding paragraphs whose mark was removed, waiting to be merged.
    let mut carry: Option<Vec<Atom>> = None;
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Paragraph(p) => {
                let (mut q, remove_mark) = resolve_paragraph(p, f, accept);
                if let Some(prev) = carry.take() {
                    let mut atoms = prev;
                    atoms.extend(para::flatten(&q));
                    q.runs = para::rebuild(atoms);
                }
                let next_is_para = matches!(blocks.get(i + 1), Some(Block::Paragraph(_)));
                if remove_mark && next_is_para {
                    carry = Some(para::flatten(&q));
                } else {
                    out.push(Block::Paragraph(q));
                }
            }
            Block::Table(t) => {
                let mut t2 = t.clone();
                for row in &mut t2.rows {
                    for c in &mut row.cells {
                        c.blocks = resolve_blocks(&c.blocks, f, accept);
                    }
                }
                out.push(Block::Table(t2));
            }
            Block::Opaque(_) => out.push(b.clone()),
        }
    }
    out
}

/// Resolve one paragraph; returns it and whether its mark must be removed.
fn resolve_paragraph(p: &Paragraph, f: Filter<'_>, accept: bool) -> (Paragraph, bool) {
    let hit = |m: &Option<RevMark>| m.as_ref().is_some_and(|m| f(m.info.id));
    let mut q = p.clone();
    let mut atoms: Vec<Atom> = Vec::new();
    for (mut props, item) in para::flatten(p) {
        let del_hit = hit(&props.del);
        let ins_hit = hit(&props.ins);
        // Accepting a deletion or rejecting an insertion removes the content.
        if (del_hit && accept) || (ins_hit && !accept) {
            continue;
        }
        if del_hit {
            props.del = None;
        }
        if ins_hit {
            props.ins = None;
        }
        if let Some(c) = props.change.clone().filter(|c| f(c.info.id)) {
            props = if accept {
                RunProps {
                    change: None,
                    ..props
                }
            } else {
                RunProps {
                    ins: props.ins,
                    del: props.del,
                    link: props.link,
                    change: None,
                    ..c.old.without_revisions()
                }
            };
        }
        atoms.push((props, item));
    }
    q.runs = para::rebuild(atoms);
    if let Some(c) = q.props.change.clone().filter(|c| f(c.info.id)) {
        q.props = if accept {
            ParaProps {
                change: None,
                ..q.props
            }
        } else {
            ParaProps {
                mark_ins: q.props.mark_ins.clone(),
                mark_del: q.props.mark_del.clone(),
                unknown: q.props.unknown.clone(),
                change: None,
                ..c.old.clone()
            }
        };
    }
    let del_hit = hit(&q.props.mark_del);
    let ins_hit = hit(&q.props.mark_ins);
    let remove = (del_hit && accept) || (ins_hit && !accept);
    if del_hit {
        q.props.mark_del = None;
    }
    if ins_hit {
        q.props.mark_ins = None;
    }
    (q, remove)
}

/// Author and clock used to record new revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tracker {
    pub author: String,
    pub date: Option<String>,
    pub next_id: i32,
}

impl Tracker {
    /// A fresh revision mark.
    pub fn mark(&mut self) -> RevMark {
        self.next_id += 1;
        RevMark {
            info: RevisionInfo {
                id: self.next_id,
                author: self.author.clone(),
                date: self.date.clone(),
            },
            moved: false,
        }
    }

    /// Content inserted by this author and not deleted since: deleting it removes it.
    pub fn owns(&self, props: &RunProps) -> bool {
        props.del.is_none()
            && props
                .ins
                .as_ref()
                .is_some_and(|m| m.info.author == self.author)
    }
}

/// Mark `a..b` of a paragraph's atoms as deleted by `mark`; the author's own
/// insertions are removed instead. Returns the number of index-text bytes removed.
pub(crate) fn mark_deleted(
    atoms: &mut Vec<Atom>,
    a: usize,
    b: usize,
    tracker: &Tracker,
    mark: &RevMark,
) -> usize {
    if b <= a {
        return 0;
    }
    let i = para::split_at(atoms, a);
    let j = para::split_at(atoms, b);
    let mut removed = 0;
    let mut kept: Vec<Atom> = Vec::new();
    for (mut props, item) in atoms.drain(i..j) {
        let l = para::atom_len(&item);
        if l > 0 && tracker.owns(&props) {
            removed += l;
            continue;
        }
        if l > 0 && props.del.is_none() {
            props.del = Some(mark.clone());
        }
        kept.push((props, item));
    }
    atoms.splice(i..i, kept);
    removed
}

/// The insertion mark to use for text typed at `off`: the author's insertion right
/// before the caret is continued (one revision per typing burst).
pub(crate) fn insertion_mark(atoms: &[Atom], off: usize, tracker: &mut Tracker) -> RevMark {
    let mut pos = 0;
    let mut prev: Option<&RunProps> = None;
    for (props, item) in atoms {
        let l = para::atom_len(item);
        if l > 0 && pos + l <= off {
            prev = Some(props);
        }
        pos += l;
        if pos >= off {
            break;
        }
    }
    match prev {
        Some(p) if tracker.owns(p) => p.ins.clone().unwrap_or_else(|| tracker.mark()),
        _ => tracker.mark(),
    }
}

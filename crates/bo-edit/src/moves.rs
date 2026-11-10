//! Tracked moves (ECMA-376 Part 1, §17.13.5.22 moveFrom, §17.13.5.25 moveTo) and
//! their named ranges (§17.13.5.23 moveFromRangeStart, §17.13.5.24 moveFromRangeEnd,
//! §17.13.5.27 moveToRangeStart, §17.13.5.26 moveToRangeEnd).
//!
//! A move is a pair of revisions: the content at its old place (moveFrom) and at
//! its new place (moveTo). The two halves carry their own revision ids; they are
//! tied together by range markers sharing one `w:name`. The markers are kept as
//! verbatim [`RunContent::Markup`] items (or [`Block::Opaque`] between paragraphs)
//! and parsed here, so files from other producers round-trip byte for byte.
//!
//! Accepting or rejecting either half resolves the whole move: accepting keeps the
//! moveTo content and drops the moveFrom content, rejecting does the reverse; the
//! markers of a fully resolved move are removed.

use crate::para::{self, Atom};
use crate::{EditError, Op, Pos, Result, Selection, Session};
use bo_model::index::body_paragraphs;
use bo_model::{Block, Document, ParaProps, Paragraph, Raw, RevMark, RunContent};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Kind of a move range marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveMarker {
    /// §17.13.5.23 moveFromRangeStart
    FromStart,
    /// §17.13.5.24 moveFromRangeEnd
    FromEnd,
    /// §17.13.5.27 moveToRangeStart
    ToStart,
    /// §17.13.5.26 moveToRangeEnd
    ToEnd,
}

/// Value of attribute `local` (any prefix) in the start tag `xml`.
fn attr(xml: &str, local: &str) -> Option<String> {
    let tag_end = xml.find('>').unwrap_or(xml.len());
    let tag = &xml[..tag_end];
    let key = format!("{local}=\"");
    let mut from = 0;
    while let Some(k) = tag[from..].find(&key) {
        let at = from + k;
        let ok = tag[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c == ':' || c.is_whitespace());
        let v0 = at + key.len();
        if ok {
            let v1 = tag[v0..].find('"').map(|e| v0 + e)?;
            return Some(
                tag[v0..v1]
                    .replace("&quot;", "\"")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&apos;", "'")
                    .replace("&amp;", "&"),
            );
        }
        from = v0;
    }
    None
}

/// The marker kind, `w:id` and `w:name` of a verbatim element, when it is a move
/// range marker.
pub(crate) fn move_marker(xml: &str) -> Option<(MoveMarker, String, Option<String>)> {
    let name = xml
        .strip_prefix('<')?
        .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .next()?;
    let kind = match name.rsplit(':').next()? {
        "moveFromRangeStart" => MoveMarker::FromStart,
        "moveFromRangeEnd" => MoveMarker::FromEnd,
        "moveToRangeStart" => MoveMarker::ToStart,
        "moveToRangeEnd" => MoveMarker::ToEnd,
        _ => return None,
    };
    Some((kind, attr(xml, "id")?, attr(xml, "name")))
}

/// Move ranges of a document: the revision ids of each named move and the name of
/// each range marker id.
#[derive(Debug, Default, Clone)]
pub(crate) struct MoveGroups {
    /// `w:name` → revision ids of its moveFrom and moveTo content.
    pub by_name: BTreeMap<String, Vec<i32>>,
    /// Range marker `w:id` → `w:name` (from the range starts).
    pub marker_names: HashMap<String, String>,
    /// Range marker ids whose end has no start.
    pub orphan_ends: HashSet<String>,
}

#[derive(Default)]
struct Walk {
    groups: MoveGroups,
    /// Open moveFrom / moveTo ranges, innermost last: (marker id, name).
    from: Vec<(String, String)>,
    to: Vec<(String, String)>,
    /// Ranges closed in the current paragraph: its mark still belongs to them (a
    /// range ending at the end of a paragraph covers the moved paragraph mark).
    closed_from: Option<String>,
    closed_to: Option<String>,
}

impl Walk {
    fn marker(&mut self, xml: &str) {
        let Some((kind, id, name)) = move_marker(xml) else {
            return;
        };
        match kind {
            MoveMarker::FromStart | MoveMarker::ToStart => {
                let name = name.unwrap_or_default();
                self.groups.by_name.entry(name.clone()).or_default();
                self.groups.marker_names.insert(id.clone(), name.clone());
                if kind == MoveMarker::FromStart {
                    self.from.push((id, name));
                } else {
                    self.to.push((id, name));
                }
            }
            MoveMarker::FromEnd | MoveMarker::ToEnd => {
                let open = if kind == MoveMarker::FromEnd {
                    &mut self.from
                } else {
                    &mut self.to
                };
                match open.iter().rposition(|(i, _)| *i == id) {
                    Some(k) => {
                        let (_, name) = open.remove(k);
                        if kind == MoveMarker::FromEnd {
                            self.closed_from = Some(name);
                        } else {
                            self.closed_to = Some(name);
                        }
                    }
                    None => {
                        if !self.groups.marker_names.contains_key(&id) {
                            self.groups.orphan_ends.insert(id);
                        }
                    }
                }
            }
        }
    }

    fn add(&mut self, from: bool, m: &RevMark, mark: bool) {
        if !m.moved {
            return;
        }
        let (open, closed) = if from {
            (&self.from, &self.closed_from)
        } else {
            (&self.to, &self.closed_to)
        };
        let name = open
            .last()
            .map(|(_, n)| n)
            .or(if mark { closed.as_ref() } else { None });
        if let Some(name) = name {
            let v = self.groups.by_name.entry(name.clone()).or_default();
            if !v.contains(&m.info.id) {
                v.push(m.info.id);
            }
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    self.closed_from = None;
                    self.closed_to = None;
                    for r in &p.runs {
                        for c in &r.content {
                            if let RunContent::Markup(raw) = c {
                                self.marker(&raw.xml);
                            } else {
                                if let Some(m) = &r.props.del {
                                    self.add(true, m, false);
                                }
                                if let Some(m) = &r.props.ins {
                                    self.add(false, m, false);
                                }
                            }
                        }
                    }
                    if let Some(m) = &p.props.mark_del {
                        self.add(true, m, true);
                    }
                    if let Some(m) = &p.props.mark_ins {
                        self.add(false, m, true);
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for c in &row.cells {
                            self.blocks(&c.blocks);
                        }
                    }
                }
                Block::Opaque(raw) => self.marker(&raw.xml),
            }
        }
    }
}

/// Every block container of a document: section bodies, headers/footers, notes.
pub(crate) fn containers(doc: &Document) -> Vec<&[Block]> {
    let mut out: Vec<&[Block]> = Vec::new();
    for s in &doc.sections {
        out.push(&s.blocks);
        for set in [&s.headers, &s.footers] {
            for b in [&set.default, &set.first, &set.even].into_iter().flatten() {
                out.push(b);
            }
        }
    }
    for b in doc.footnotes.values().chain(doc.endnotes.values()) {
        out.push(b);
    }
    out
}

/// The named moves of `doc`.
pub(crate) fn move_groups(doc: &Document) -> MoveGroups {
    let mut w = Walk::default();
    for c in containers(doc) {
        w.blocks(c);
        w.from.clear();
        w.to.clear();
    }
    w.groups
}

impl MoveGroups {
    /// Name of the move holding revision `id`.
    pub fn name_of(&self, id: i32) -> Option<&str> {
        self.by_name
            .iter()
            .find(|(_, v)| v.contains(&id))
            .map(|(n, _)| n.as_str())
    }

    /// `ids` with the other halves of the moves they belong to.
    pub fn expand(&self, ids: &[i32]) -> HashSet<i32> {
        let mut set: HashSet<i32> = ids.iter().copied().collect();
        for v in self.by_name.values() {
            if v.iter().any(|i| set.contains(i)) {
                set.extend(v.iter().copied());
            }
        }
        set
    }

    /// Range marker ids to remove once the revisions selected by `set` (`None`: all)
    /// are resolved: the markers of moves whose content is all resolved.
    pub fn dead_markers(&self, set: Option<&HashSet<i32>>) -> HashSet<String> {
        let dead_name = |name: &str| {
            let v = self.by_name.get(name).map_or(&[][..], Vec::as_slice);
            match set {
                None => true,
                Some(s) => !v.is_empty() && v.iter().all(|i| s.contains(i)),
            }
        };
        let mut out: HashSet<String> = self
            .marker_names
            .iter()
            .filter(|(_, n)| dead_name(n))
            .map(|(id, _)| id.clone())
            .collect();
        if set.is_none() {
            out.extend(self.orphan_ends.iter().cloned());
        }
        out
    }

    /// Ids used by range markers (they share the annotation id space, §17.13.1).
    pub fn max_marker_id(&self) -> i32 {
        self.marker_names
            .keys()
            .chain(&self.orphan_ends)
            .filter_map(|i| i.trim().parse::<i32>().ok())
            .max()
            .unwrap_or(0)
    }
}

fn is_dead(xml: &str, dead: &HashSet<String>) -> bool {
    move_marker(xml).is_some_and(|(_, id, _)| dead.contains(&id))
}

/// `blocks` without the move range markers whose id is in `dead`.
pub(crate) fn strip_markers(blocks: &[Block], dead: &HashSet<String>) -> Vec<Block> {
    if dead.is_empty() {
        return blocks.to_vec();
    }
    let mut out = Vec::with_capacity(blocks.len());
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                let mut q = p.clone();
                for r in &mut q.runs {
                    r.content.retain(
                        |c| !matches!(c, RunContent::Markup(raw) if is_dead(&raw.xml, dead)),
                    );
                }
                q.runs.retain(|r| !r.content.is_empty());
                out.push(Block::Paragraph(q));
            }
            Block::Table(t) => {
                let mut t = t.clone();
                for row in &mut t.rows {
                    for c in &mut row.cells {
                        c.blocks = strip_markers(&c.blocks, dead);
                    }
                }
                out.push(Block::Table(t));
            }
            Block::Opaque(raw) if is_dead(&raw.xml, dead) => {}
            Block::Opaque(_) => out.push(b.clone()),
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Range start marker (`CT_MoveBookmark`: id, name, author, date).
fn range_start(tag: &str, id: i32, name: &str, m: &RevMark) -> RunContent {
    let mut xml = format!(
        "<w:{tag} w:id=\"{id}\" w:author=\"{}\"",
        escape(&m.info.author)
    );
    if let Some(d) = &m.info.date {
        xml.push_str(&format!(" w:date=\"{}\"", escape(d)));
    }
    xml.push_str(&format!(" w:name=\"{}\"/>", escape(name)));
    RunContent::Markup(Raw::new(xml, 0))
}

/// Range end marker (`CT_MarkupRange`: id).
fn range_end(tag: &str, id: i32) -> RunContent {
    RunContent::Markup(Raw::new(format!("<w:{tag} w:id=\"{id}\"/>"), 0))
}

/// The old place of moved paragraphs: their content and marks become moveFrom
/// (except the last mark with `keep_last`, see [`Session::move_paragraphs`]).
fn move_from(
    paras: &[Paragraph],
    mark: &RevMark,
    range: i32,
    name: &str,
    keep_last: bool,
) -> Vec<Block> {
    let n = paras.len();
    paras
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut atoms: Vec<Atom> = para::flatten(p)
                .into_iter()
                .map(|(mut props, item)| {
                    if para::atom_len(&item) > 0 && props.del.is_none() {
                        props.del = Some(mark.clone());
                    }
                    (props, item)
                })
                .collect();
            if i == 0 {
                atoms.insert(
                    0,
                    (
                        bo_model::RunProps::default(),
                        range_start("moveFromRangeStart", range, name, mark),
                    ),
                );
            }
            if i + 1 == n {
                atoms.push((
                    bo_model::RunProps::default(),
                    range_end("moveFromRangeEnd", range),
                ));
            }
            let mut q = para::with_atoms(p, atoms);
            if q.props.mark_del.is_none() && !(keep_last && i + 1 == n) {
                q.props.mark_del = Some(mark.clone());
            }
            Block::Paragraph(q)
        })
        .collect()
}

/// The new place of moved paragraphs: a copy of their (not deleted) content and
/// marks as moveTo. Range markers, comment anchors and other verbatim markup stay at
/// the old place only. With `keep_last` the last mark is not marked.
fn move_to(
    paras: &[Paragraph],
    mark: &RevMark,
    range: i32,
    name: &str,
    keep_last: bool,
) -> Vec<Block> {
    let n = paras.len();
    paras
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut atoms: Vec<Atom> = para::flatten(p)
                .into_iter()
                .filter(|(props, item)| {
                    props.del.is_none()
                        && !matches!(
                            item,
                            RunContent::Markup(_)
                                | RunContent::CommentStart(_)
                                | RunContent::CommentEnd(_)
                                | RunContent::CommentRef(_)
                        )
                })
                .map(|(mut props, item)| {
                    props.ins = Some(mark.clone());
                    (props, item)
                })
                .collect();
            if i == 0 {
                atoms.insert(
                    0,
                    (
                        bo_model::RunProps::default(),
                        range_start("moveToRangeStart", range, name, mark),
                    ),
                );
            }
            if i + 1 == n {
                atoms.push((
                    bo_model::RunProps::default(),
                    range_end("moveToRangeEnd", range),
                ));
            }
            let mut q = para::with_atoms(p, atoms);
            q.props.mark_del = None;
            q.props.mark_ins = (!(keep_last && i + 1 == n)).then(|| mark.clone());
            Block::Paragraph(q)
        })
        .collect()
}

impl Session {
    /// Move the paragraphs touched by the selection above the previous paragraph
    /// (`up`) or below the next one, in the main text of one section. In
    /// track-changes mode the move is recorded as a tracked move (§17.13.5.22
    /// moveFrom at the old place, §17.13.5.25 moveTo at the new place, tied by named
    /// ranges); the moved copy is selected. Nothing changes at either end.
    pub fn move_paragraphs(&mut self, up: bool) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let (Some(pa), Some(pb)) = (paths.get(a.para), paths.get(b.para)) else {
            return Err(EditError::BadPosition);
        };
        let (&[si, from], &[sj, last]) = (pa.as_slice(), pb.as_slice()) else {
            return Err(EditError::Unsupported(
                "move: paragraph inside a table".into(),
            ));
        };
        if si != sj {
            return Err(EditError::Unsupported("move: several sections".into()));
        }
        let to = last + 1;
        let blocks = &self.doc.sections[si].blocks;
        let is_para = |i: usize| matches!(blocks.get(i), Some(Block::Paragraph(_)));
        if !(from..to).all(is_para) {
            return Err(EditError::Unsupported(
                "move: table in the selection".into(),
            ));
        }
        // The neighbour passed by the move.
        let neighbour = if up {
            if from == 0 {
                return Ok(self.selection);
            }
            from - 1
        } else {
            if to >= blocks.len() {
                return Ok(self.selection);
            }
            to
        };
        if !is_para(neighbour) {
            return Err(EditError::Unsupported(
                "move: table next to the selection".into(),
            ));
        }
        // Removing a paragraph mark joins the paragraph with the next one (§17.13.5.15).
        // When the moved paragraphs (up) or their copy (down) end the text, no
        // paragraph follows: the mark of the neighbour is tracked instead and the last
        // moved mark stays, which keeps the right paragraph properties only when the
        // neighbour and the last moved paragraph have the same ones.
        let tail = if up {
            !is_para(to)
        } else {
            !is_para(neighbour + 1)
        };
        if self.track && tail {
            let bare = |i: usize| match blocks.get(i) {
                Some(Block::Paragraph(p)) => Some(ParaProps {
                    mark_ins: None,
                    mark_del: None,
                    change: None,
                    ..p.props.clone()
                }),
                _ => None,
            };
            let free = match blocks.get(neighbour) {
                Some(Block::Paragraph(p)) => {
                    p.props.mark_ins.is_none() && p.props.mark_del.is_none()
                }
                _ => false,
            };
            if !free || bare(neighbour) != bare(to - 1) {
                return Err(EditError::Unsupported(
                    "move: the last paragraph cannot be moved while tracking".into(),
                ));
            }
        }
        let k = to - from;
        let names = move_groups(&self.doc);
        let first_id = a.para;
        self.edit(move |tx, _| {
            let lo = from.min(neighbour);
            let hi = to.max(neighbour + 1);
            let old: Vec<Block> = tx.doc.sections[si].blocks[lo..hi].to_vec();
            let moved: Vec<Paragraph> = tx.doc.sections[si].blocks[from..to]
                .iter()
                .filter_map(|b| match b {
                    Block::Paragraph(p) => Some(p.clone()),
                    _ => None,
                })
                .collect();
            let mut next = tx.doc.sections[si].blocks[neighbour].clone();
            let (new, sel_first) = if let Some(tr) = tx.track.as_mut() {
                let floor = names.max_marker_id();
                tr.next_id = tr.next_id.max(floor);
                let from_mark = RevMark {
                    moved: true,
                    ..tr.mark()
                };
                let to_mark = RevMark {
                    moved: true,
                    ..tr.mark()
                };
                let from_range = tr.mark().info.id;
                let to_range = tr.mark().info.id;
                let mut name = format!("move{}", from_mark.info.id);
                while names.by_name.contains_key(&name) {
                    name.push('_');
                }
                let src = move_from(&moved, &from_mark, from_range, &name, tail && up);
                let copy = move_to(&moved, &to_mark, to_range, &name, tail && !up);
                if let (true, Block::Paragraph(p)) = (tail, &mut next) {
                    if up {
                        p.props.mark_del = Some(from_mark.clone());
                    } else {
                        p.props.mark_ins = Some(to_mark.clone());
                    }
                }
                if up {
                    // [copy, neighbour, old place]
                    let mut v = copy;
                    v.push(next);
                    v.extend(src);
                    (v, first_id - 1)
                } else {
                    // [old place, neighbour, copy]
                    let mut v = src;
                    v.push(next);
                    v.extend(copy);
                    (v, first_id + k + 1)
                }
            } else {
                let mut v: Vec<Block> = Vec::with_capacity(k + 1);
                let moved = moved.into_iter().map(Block::Paragraph);
                if up {
                    v.extend(moved);
                    v.push(next);
                    (v, first_id - 1)
                } else {
                    v.push(next);
                    v.extend(moved);
                    (v, first_id + 1)
                }
            };
            tx.push(Op::Blocks {
                path: vec![si],
                index: lo,
                remove: old,
                insert: new,
            })?;
            let last_id = sel_first + k - 1;
            let paths = body_paragraphs(tx.doc);
            let end = paths
                .get(last_id)
                .and_then(|p| bo_model::index::para_at(tx.doc, p))
                .map_or(0, |p| para::len(&para::flatten(p)));
            Ok(Selection {
                anchor: Pos::new(sel_first, 0),
                focus: Pos::new(last_id, end),
            })
        })
    }
}

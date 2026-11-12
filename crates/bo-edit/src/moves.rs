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

/// A cut made while tracking: what it put on the clipboard and the deletion
/// revisions it recorded. Pasting that clipboard turns the pair into a move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CutRecord {
    /// Internal clipboard payload of the cut.
    pub internal: String,
    /// Revision ids of the tracked deletion made by the cut.
    pub ids: HashSet<i32>,
}

/// A paste in progress that may complete a tracked move ([`Session::move_paste_start`]).
#[derive(Debug, Clone)]
pub struct MovePaste {
    from: HashSet<i32>,
    ins_before: HashSet<i32>,
    depth: usize,
}

/// Ids of the tracked deletions (`del`) or insertions (`!del`) of the body that are not
/// part of a move yet (runs and paragraph marks).
fn rev_ids(doc: &Document, del: bool) -> HashSet<i32> {
    let mut out = HashSet::new();
    for path in body_paragraphs(doc) {
        let Some(p) = bo_model::index::para_at(doc, &path) else {
            continue;
        };
        let pick = |m: &Option<RevMark>| m.as_ref().filter(|m| !m.moved).map(|m| m.info.id);
        for r in &p.runs {
            let m = if del { &r.props.del } else { &r.props.ins };
            out.extend(pick(m));
        }
        let m = if del {
            &p.props.mark_del
        } else {
            &p.props.mark_ins
        };
        out.extend(pick(m));
    }
    out
}

/// The deletion (`del`) or insertion mark of a run.
fn side_of(props: &bo_model::RunProps, del: bool) -> Option<&RevMark> {
    if del {
        props.del.as_ref()
    } else {
        props.ins.as_ref()
    }
}

/// The deletion (`del`) or insertion mark of a paragraph mark.
fn mark_of(p: &Paragraph, del: bool) -> Option<&RevMark> {
    if del {
        p.props.mark_del.as_ref()
    } else {
        p.props.mark_ins.as_ref()
    }
}

/// `m` is one of `ids` and not part of a move yet.
fn hit(m: Option<&RevMark>, ids: &HashSet<i32>) -> bool {
    m.is_some_and(|m| !m.moved && ids.contains(&m.info.id))
}

impl Session {
    /// Before a cut: the deletions already in the body (to tell the cut's own).
    pub(crate) fn cut_start(&self) -> Option<HashSet<i32>> {
        self.track.then(|| rev_ids(&self.doc, true))
    }

    /// After a cut while tracking: remember its deletion so that pasting the same
    /// clipboard records a move (§17.13.5.22 moveFrom / §17.13.5.25 moveTo).
    pub(crate) fn cut_end(&mut self, before: Option<HashSet<i32>>, internal: &str) {
        self.last_cut = before.and_then(|before| {
            let ids: HashSet<i32> = rev_ids(&self.doc, true)
                .difference(&before)
                .copied()
                .collect();
            (!ids.is_empty()).then(|| CutRecord {
                internal: internal.to_owned(),
                ids,
            })
        });
    }

    /// Before a paste of internal clipboard `internal`: when it is the last cut's and
    /// tracking is on with the cut's deletion still in the text, the paste will be
    /// recorded as the other half of a move ([`Session::move_paste_finish`]).
    pub fn move_paste_start(&self, internal: Option<&str>) -> Option<MovePaste> {
        let rec = self.last_cut.as_ref()?;
        if !self.track || internal != Some(rec.internal.as_str()) {
            return None;
        }
        let dels = rev_ids(&self.doc, true);
        let from: HashSet<i32> = rec.ids.intersection(&dels).copied().collect();
        (!from.is_empty()).then(|| MovePaste {
            from,
            ins_before: rev_ids(&self.doc, false),
            depth: self.undo.len(),
        })
    }

    /// After the paste: the cut's deletion becomes moveFrom and the pasted insertion
    /// moveTo, tied by named range markers (§17.13.5.23–27); joined to the paste's undo
    /// step. Returns whether a move was recorded.
    pub fn move_paste_finish(&mut self, m: MovePaste) -> Result<bool> {
        let to: HashSet<i32> = rev_ids(&self.doc, false)
            .difference(&m.ins_before)
            .copied()
            .collect();
        if to.is_empty() || self.undo.len() <= m.depth {
            return Ok(false);
        }
        let from = m.from;
        let names = move_groups(&self.doc);
        let floor = names.max_marker_id();
        self.edit(move |tx, sel| {
            let Some(tr) = tx.track.as_mut() else {
                return Ok(sel);
            };
            tr.next_id = tr.next_id.max(floor);
            let mark = tr.mark();
            let from_range = mark.info.id;
            let to_range = tr.mark().info.id;
            let mut name = format!("move{from_range}");
            while names.by_name.contains_key(&name) {
                name.push('_');
            }
            for (del, ids, range, start, end) in [
                (
                    true,
                    &from,
                    from_range,
                    "moveFromRangeStart",
                    "moveFromRangeEnd",
                ),
                (false, &to, to_range, "moveToRangeStart", "moveToRangeEnd"),
            ] {
                let side = |props: &bo_model::RunProps| side_of(props, del).cloned();
                let pmark = |q: &Paragraph| mark_of(q, del).cloned();
                // Paragraphs holding the half, in document order.
                let n = body_paragraphs(tx.doc).len();
                let mut touched = Vec::new();
                for id in 0..n {
                    let (_, p, atoms) = tx.atoms(id)?;
                    if atoms
                        .iter()
                        .any(|(r, c)| para::atom_len(c) > 0 && hit(side(r).as_ref(), ids))
                        || hit(pmark(&p).as_ref(), ids)
                    {
                        touched.push(id);
                    }
                }
                let (Some(&first), Some(&last)) = (touched.first(), touched.last()) else {
                    continue;
                };
                // The marks of the half's revisions say "moved".
                let info = RevMark {
                    moved: true,
                    ..mark.clone()
                };
                for &id in &touched {
                    let (path, p, atoms) = tx.atoms(id)?;
                    let mut atoms: Vec<Atom> = atoms
                        .into_iter()
                        .map(|(mut r, c)| {
                            let slot = if del { &mut r.del } else { &mut r.ins };
                            if let Some(m) = slot.as_mut().filter(|m| hit(Some(m), ids)) {
                                m.moved = true;
                            }
                            (r, c)
                        })
                        .collect();
                    let is_mine = |a: &Atom| {
                        para::atom_len(&a.1) > 0
                            && side_of(&a.0, del)
                                .is_some_and(|m| m.moved && ids.contains(&m.info.id))
                    };
                    if id == first {
                        let at = atoms.iter().position(is_mine).unwrap_or(atoms.len());
                        atoms.insert(
                            at,
                            (
                                bo_model::RunProps::default(),
                                range_start(start, range, &name, &info),
                            ),
                        );
                    }
                    if id == last {
                        // After the last moved character; a moved paragraph mark is
                        // covered by a range closed at the end of its paragraph.
                        let at = if hit(pmark(&p).as_ref(), ids) {
                            atoms.len()
                        } else {
                            atoms
                                .iter()
                                .rposition(is_mine)
                                .map_or(atoms.len(), |i| i + 1)
                        };
                        atoms.insert(at, (bo_model::RunProps::default(), range_end(end, range)));
                    }
                    let mut q = para::with_atoms(&p, atoms);
                    let pm = if del {
                        &mut q.props.mark_del
                    } else {
                        &mut q.props.mark_ins
                    };
                    if let Some(m) = pm.as_mut().filter(|m| hit(Some(m), ids)) {
                        m.moved = true;
                    }
                    tx.replace_para(&path, q)?;
                }
            }
            Ok(sel)
        })?;
        self.merge_history_since(m.depth);
        self.last_cut = None;
        Ok(true)
    }
}

/// Zero-width place holder of a drop point while the dragged text is removed.
const DROP_MARK: &str = "<w:bookmarkStart w:id=\"-1\" w:name=\"_BlueOfficeDrop\"/>";

impl Session {
    /// Drag and drop of the selection to `target`: the text moves there (`copy`: a copy
    /// goes there and the original stays), in one undo step; the dropped text is
    /// selected. While tracking, a move is recorded as one (§17.13.5.22 moveFrom /
    /// §17.13.5.25 moveTo), a copy as an insertion. A drop inside the selection does
    /// nothing.
    pub fn move_selection_to(&mut self, target: Pos, copy: bool) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        if self.selection.is_collapsed() || (a <= target && target <= b) {
            return Ok(self.selection);
        }
        let clip = self.copy();
        let depth = self.undo.len();
        self.end_typing();
        let res = self.drop_clip(target, copy, &clip.internal, (a, b));
        self.merge_history_since(depth);
        self.end_typing();
        if res.is_err() && self.undo.len() > depth {
            self.undo();
            self.redo.clear();
        }
        res
    }

    fn drop_clip(
        &mut self,
        target: Pos,
        copy: bool,
        internal: &str,
        (a, b): (Pos, Pos),
    ) -> Result<Selection> {
        let at = if copy {
            target
        } else {
            // Mark the drop point (zero width: offsets do not move), remove the text,
            // then find the point again.
            self.edit(move |tx, sel| {
                let (path, p, mut atoms) = tx.atoms(target.para)?;
                let i = para::split_at(&mut atoms, target.offset);
                atoms.insert(
                    i,
                    (
                        bo_model::RunProps::default(),
                        RunContent::Markup(Raw::new(DROP_MARK, 0)),
                    ),
                );
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                Ok(sel)
            })?;
            self.set_selection(Selection {
                anchor: a,
                focus: b,
            });
            let before = self.cut_start();
            self.delete_backward()?;
            self.cut_end(before, internal);
            let found = (0..self.paragraph_count()).find_map(|id| {
                let path = body_paragraphs(&self.doc).get(id)?.clone();
                let p = bo_model::index::para_at(&self.doc, &path)?;
                let atoms = para::flatten(p);
                let mut off = 0;
                for (_, c) in &atoms {
                    if matches!(c, RunContent::Markup(r) if &*r.xml == DROP_MARK) {
                        return Some(Pos::new(id, off));
                    }
                    off += para::atom_len(c);
                }
                None
            });
            let Some(at) = found else {
                return Err(EditError::BadPosition);
            };
            self.edit(move |tx, sel| {
                let (path, p, atoms) = tx.atoms(at.para)?;
                let atoms = atoms
                    .into_iter()
                    .filter(|(_, c)| !matches!(c, RunContent::Markup(r) if &*r.xml == DROP_MARK))
                    .collect();
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                Ok(sel)
            })?;
            at
        };
        self.set_selection(Selection::caret(at));
        let mv = if copy {
            None
        } else {
            self.move_paste_start(Some(internal))
        };
        self.paste_internal(internal)?;
        if let Some(m) = mv {
            self.move_paste_finish(m)?;
        }
        let end = self.selection.focus;
        let sel = Selection {
            anchor: at,
            focus: end,
        };
        self.set_selection(sel);
        Ok(sel)
    }
}

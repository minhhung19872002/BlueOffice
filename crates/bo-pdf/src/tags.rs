//! Logical structure (ISO 32000-1 §14.7 structure tree, §14.6 marked content,
//! §14.8 tagged PDF) and document outline (§12.3.3).
//!
//! The exporters describe the structure as a tree of [`StructNode`]s whose
//! leaves are ranges of display-list commands. Each range becomes one
//! marked-content sequence `/Tag <</MCID n>> BDC … EMC` in the page's content
//! stream; commands outside every range are wrapped as `/Artifact` (§14.8.2.2).

use crate::options::{Bookmark, Role, StructKid, StructNode};
use bo_render::{Command, DisplayList, PageList};
use pdf_writer::types::TableHeaderScope;
use pdf_writer::{Finish, Name, Pdf, Ref, TextStr};

/// One marked-content sequence of a page: commands `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mark {
    pub start: usize,
    pub end: usize,
    pub mcid: i32,
    pub tag: &'static str,
}

enum EKid {
    Elem(Ref),
    Content(usize),
    Mcid { page: usize, mcid: i32 },
}

struct Elem {
    id: Ref,
    parent: Ref,
    role: Role,
    alt: Option<String>,
    lang: Option<String>,
    kids: Vec<EKid>,
    page: Option<usize>,
}

/// The structure tree with object numbers and MCIDs assigned.
pub(crate) struct Prepared {
    root: Ref,
    document: Ref,
    elems: Vec<Elem>,
    document_kids: Vec<Ref>,
    /// Marked-content sequences per page, sorted by `start`.
    pub marks: Vec<Vec<Mark>>,
    /// Owning element of each MCID, per page (the ParentTree arrays).
    owners: Vec<Vec<Ref>>,
}

struct Pending {
    page: usize,
    start: usize,
    end: usize,
    elem: usize,
}

fn keep(n: &StructNode) -> bool {
    !n.is_empty() || matches!(n.role, Role::TD | Role::TH | Role::TR | Role::Table)
}

fn walk(
    n: &StructNode,
    parent: Ref,
    alloc: &mut dyn FnMut() -> Ref,
    elems: &mut Vec<Elem>,
    pending: &mut Vec<Pending>,
) -> Ref {
    let id = alloc();
    let at = elems.len();
    elems.push(Elem {
        id,
        parent,
        role: n.role,
        alt: n.alt.clone(),
        lang: n.lang.clone(),
        kids: Vec::new(),
        page: None,
    });
    for k in &n.kids {
        match k {
            StructKid::Node(c) if keep(c) => {
                let r = walk(c, id, alloc, elems, pending);
                elems[at].kids.push(EKid::Elem(r));
            }
            StructKid::Node(_) => {}
            StructKid::Content { page, start, end } => {
                elems[at].kids.push(EKid::Content(pending.len()));
                pending.push(Pending {
                    page: *page,
                    start: *start,
                    end: *end,
                    elem: at,
                });
            }
        }
    }
    id
}

/// Assign object numbers and MCIDs; invalid or overlapping ranges are dropped.
pub(crate) fn prepare(
    nodes: &[StructNode],
    list: &DisplayList,
    alloc: &mut dyn FnMut() -> Ref,
) -> Prepared {
    let root = alloc();
    let document = alloc();
    let mut elems = Vec::new();
    let mut pending = Vec::new();
    let mut document_kids = Vec::new();
    for n in nodes.iter().filter(|n| keep(n)) {
        document_kids.push(walk(n, document, alloc, &mut elems, &mut pending));
    }
    let n_pages = list.pages.len();
    let mut marks: Vec<Vec<Mark>> = vec![Vec::new(); n_pages];
    let mut owners: Vec<Vec<Ref>> = vec![Vec::new(); n_pages];
    let mut resolved: Vec<Option<(usize, i32)>> = vec![None; pending.len()];
    for (page, p) in list.pages.iter().enumerate() {
        let mut idx: Vec<usize> = (0..pending.len())
            .filter(|&i| {
                let c = &pending[i];
                c.page == page && c.start < c.end && c.end <= p.commands.len()
            })
            .collect();
        idx.sort_by_key(|&i| (pending[i].start, i));
        let mut last_end = 0;
        for i in idx {
            let c = &pending[i];
            if c.start < last_end {
                continue;
            }
            last_end = c.end;
            let mcid = marks[page].len() as i32;
            let e = &elems[c.elem];
            marks[page].push(Mark {
                start: c.start,
                end: c.end,
                mcid,
                tag: e.role.tag(),
            });
            owners[page].push(e.id);
            resolved[i] = Some((page, mcid));
        }
    }
    for e in &mut elems {
        e.kids.retain_mut(|k| {
            if let EKid::Content(i) = *k {
                match resolved[i] {
                    Some((page, mcid)) => *k = EKid::Mcid { page, mcid },
                    None => return false,
                }
            }
            true
        });
        e.page = e.kids.iter().find_map(|k| match k {
            EKid::Mcid { page, .. } => Some(*page),
            _ => None,
        });
    }
    Prepared {
        root,
        document,
        elems,
        document_kids,
        marks,
        owners,
    }
}

impl Prepared {
    /// `true` when page `i` has marked content (it then gets `/StructParents i`).
    pub(crate) fn page_has_marks(&self, i: usize) -> bool {
        self.owners.get(i).is_some_and(|o| !o.is_empty())
    }

    /// Write the structure tree root, the elements and the parent tree.
    pub(crate) fn write(
        &self,
        pdf: &mut Pdf,
        alloc: &mut dyn FnMut() -> Ref,
        page_ids: &[Ref],
        lang: Option<&str>,
    ) {
        let mut arrays = Vec::new();
        for (i, o) in self.owners.iter().enumerate() {
            if !o.is_empty() {
                let id = alloc();
                pdf.indirect(id).array().items(o.iter().copied());
                arrays.push((i as i32, id));
            }
        }
        let mut root = pdf
            .indirect(self.root)
            .start::<pdf_writer::writers::StructTreeRoot>();
        root.child(self.document);
        {
            let mut tree = root.parent_tree();
            let mut nums = tree.nums();
            for (k, r) in &arrays {
                nums.insert(*k, *r);
            }
        }
        root.parent_tree_next_key(page_ids.len() as i32);
        root.finish();

        // §14.8.4.2 Document: the root element of the tagged content.
        let mut doc = pdf.struct_element(self.document);
        doc.kind(pdf_writer::types::StructRole::Document)
            .parent(self.root);
        if let Some(l) = lang {
            doc.lang(TextStr(l));
        }
        doc.children().items(self.document_kids.iter().copied());
        doc.finish();

        for e in &self.elems {
            let mut s = pdf.struct_element(e.id);
            s.kind(e.role.pdf()).parent(e.parent);
            if let Some(p) = e.page.and_then(|p| page_ids.get(p)) {
                s.page(*p);
            }
            if let Some(a) = &e.alt {
                s.alt(TextStr(a));
            }
            if let Some(l) = &e.lang {
                s.lang(TextStr(l));
            }
            if e.role == Role::TH {
                // §14.8.5.7 Table attributes: header cells head their column.
                s.attributes()
                    .push()
                    .table()
                    .scope(TableHeaderScope::Column);
            }
            let mut kids = s.children();
            for k in &e.kids {
                match k {
                    EKid::Elem(r) => {
                        kids.struct_element(*r);
                    }
                    EKid::Mcid { page, mcid } => {
                        let mut m = kids.marked_content_ref();
                        m.marked_content_id(*mcid);
                        if let Some(p) = page_ids.get(*page) {
                            m.page(*p);
                        }
                    }
                    EKid::Content(_) => {}
                }
            }
            kids.finish();
        }
    }

    /// Catalog entry value of `/StructTreeRoot`.
    pub(crate) fn root(&self) -> Ref {
        self.root
    }
}

/// Marked-content state while writing a page's content stream.
pub(crate) struct Marker<'m> {
    marks: &'m [Mark],
    next: usize,
    open: Option<Open>,
}

#[derive(Clone, Copy)]
enum Open {
    Tag(usize),
    Artifact,
}

impl<'m> Marker<'m> {
    pub(crate) fn new(marks: &'m [Mark]) -> Self {
        Marker {
            marks,
            next: 0,
            open: None,
        }
    }

    /// Called before command `i` is written.
    pub(crate) fn before(&mut self, c: &mut pdf_writer::Content, i: usize) {
        if let Some(Open::Tag(end)) = self.open {
            if i < end {
                return;
            }
            c.end_marked_content();
            self.open = None;
        }
        if let Some(m) = self.marks.get(self.next).filter(|m| m.start == i) {
            if matches!(self.open, Some(Open::Artifact)) {
                c.end_marked_content();
            }
            c.begin_marked_content_with_properties(Name(m.tag.as_bytes()))
                .properties()
                .identify(m.mcid);
            self.open = Some(Open::Tag(m.end));
            self.next += 1;
        } else if self.open.is_none() {
            // §14.8.2.2.2: content that is not part of the logical structure.
            c.begin_marked_content(Name(b"Artifact"));
            self.open = Some(Open::Artifact);
        }
    }

    /// Close the open sequence at the end of the page.
    pub(crate) fn close(&mut self, c: &mut pdf_writer::Content) {
        if self.open.take().is_some() {
            c.end_marked_content();
        }
    }
}

// ---------------------------------------------------------------------------
// Outline (bookmarks)
// ---------------------------------------------------------------------------

struct ONode<'b> {
    b: &'b Bookmark,
    parent: Option<usize>,
    kids: Vec<usize>,
    id: Ref,
}

fn descendants(nodes: &[ONode<'_>], i: usize) -> i32 {
    nodes[i]
        .kids
        .iter()
        .map(|&k| 1 + descendants(nodes, k))
        .sum()
}

/// Write the outline of `bookmarks` (all items open) and return its root.
pub(crate) fn write_outline(
    pdf: &mut Pdf,
    alloc: &mut dyn FnMut() -> Ref,
    bookmarks: &[Bookmark],
    page_ids: &[Ref],
    list: &DisplayList,
) -> Option<Ref> {
    let valid: Vec<&Bookmark> = bookmarks
        .iter()
        .filter(|b| b.page < page_ids.len())
        .collect();
    if valid.is_empty() {
        return None;
    }
    let root = alloc();
    let mut nodes: Vec<ONode<'_>> = Vec::new();
    let mut top: Vec<usize> = Vec::new();
    let mut stack: Vec<(u8, usize)> = Vec::new();
    for b in valid {
        let level = b.level.max(1);
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            stack.pop();
        }
        let i = nodes.len();
        let parent = stack.last().map(|(_, p)| *p);
        nodes.push(ONode {
            b,
            parent,
            kids: Vec::new(),
            id: alloc(),
        });
        match parent {
            Some(p) => nodes[p].kids.push(i),
            None => top.push(i),
        }
        stack.push((level, i));
    }
    let siblings = |i: usize| -> &[usize] {
        match nodes[i].parent {
            Some(p) => &nodes[p].kids,
            None => &top,
        }
    };
    for i in 0..nodes.len() {
        let sib = siblings(i);
        let pos = sib.iter().position(|&s| s == i).unwrap_or(0);
        let n = &nodes[i];
        let mut item = pdf.outline_item(n.id);
        item.title(TextStr(&n.b.title));
        item.parent(n.parent.map_or(root, |p| nodes[p].id));
        if pos > 0 {
            item.prev(nodes[sib[pos - 1]].id);
        }
        if let Some(&nx) = sib.get(pos + 1) {
            item.next(nodes[nx].id);
        }
        if let (Some(&f), Some(&l)) = (n.kids.first(), n.kids.last()) {
            item.first(nodes[f].id);
            item.last(nodes[l].id);
            item.count(descendants(&nodes, i));
        }
        let h = list.pages.get(n.b.page).map_or(0.0, |p| p.height);
        // §12.3.2.2 explicit destination [page /XYZ left top zoom].
        item.dest()
            .page(page_ids[n.b.page])
            .xyz(0.0, (h - n.b.y).clamp(0.0, h), None);
    }
    let total = top.len() as i32 + top.iter().map(|&t| descendants(&nodes, t)).sum::<i32>();
    let mut o = pdf.outline(root);
    if let (Some(&f), Some(&l)) = (top.first(), top.last()) {
        o.first(nodes[f].id);
        o.last(nodes[l].id);
    }
    o.count(total);
    Some(root)
}

// ---------------------------------------------------------------------------
// Structure from the display list alone (slides and sheets)
// ---------------------------------------------------------------------------

/// Paragraphs and figures of a page from its drawing commands: glyph runs on
/// nearby baselines form a `P`, every picture a `Figure`. Used where the
/// renderer keeps no link to the source model (slides).
#[must_use]
pub fn auto_paragraphs(page: &PageList, index: usize) -> Vec<StructNode> {
    let mut out: Vec<StructNode> = Vec::new();
    // (node, last baseline, last size, end command)
    let mut cur: Option<(StructNode, f32, f32, usize)> = None;
    for (i, c) in page.commands.iter().enumerate() {
        match c {
            Command::Glyphs { y, size, .. } => {
                if let Some((n, ly, ls, end)) = cur.as_mut() {
                    if (y - *ly).abs() <= ls.max(*size) * 1.6 {
                        n.push_content(index, *end, i + 1);
                        *ly = *y;
                        *ls = *size;
                        *end = i + 1;
                        continue;
                    }
                }
                if let Some((n, ..)) = cur.take() {
                    out.push(n);
                }
                let mut n = StructNode::new(Role::P);
                n.push_content(index, i, i + 1);
                cur = Some((n, *y, *size, i + 1));
            }
            Command::Image { .. } => {
                if let Some((n, ..)) = cur.take() {
                    out.push(n);
                }
                let mut f = StructNode::new(Role::Figure);
                f.push_content(index, i, i + 1);
                out.push(f);
            }
            _ => {}
        }
    }
    if let Some((n, ..)) = cur {
        out.push(n);
    }
    out
}

/// A table of the glyph runs of a page: runs on the same baseline form a row
/// (`TR`), each run a cell (`TD`), rows top to bottom and cells left to right;
/// pictures follow as `Figure`s. Used for printed sheets.
#[must_use]
pub fn auto_grid(page: &PageList, index: usize) -> Vec<StructNode> {
    let mut runs: Vec<(f32, f32, f32, usize)> = page
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Command::Glyphs { x, y, size, .. } => Some((*y, *x, *size, i)),
            _ => None,
        })
        .collect();
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut rows: Vec<Vec<(f32, usize)>> = Vec::new();
    let mut row_y = f32::NEG_INFINITY;
    for (y, x, size, i) in runs {
        if rows.is_empty() || (y - row_y).abs() > size * 0.5 {
            rows.push(Vec::new());
            row_y = y;
        }
        if let Some(r) = rows.last_mut() {
            r.push((x, i));
        }
    }
    let mut out = Vec::new();
    if !rows.is_empty() {
        let mut table = StructNode::new(Role::Table);
        for mut r in rows {
            r.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut tr = StructNode::new(Role::TR);
            for (_, i) in r {
                let mut td = StructNode::new(Role::TD);
                td.push_content(index, i, i + 1);
                tr.kids.push(StructKid::Node(td));
            }
            table.kids.push(StructKid::Node(tr));
        }
        out.push(table);
    }
    for (i, c) in page.commands.iter().enumerate() {
        if matches!(c, Command::Image { .. }) {
            let mut f = StructNode::new(Role::Figure);
            f.push_content(index, i, i + 1);
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_render::{Glyph, Rgba};

    fn glyphs(x: f32, y: f32) -> Command {
        Command::Glyphs {
            face: 0,
            size: 10.0,
            x,
            y,
            color: Rgba(0, 0, 0, 255),
            text: "a".into(),
            glyphs: vec![Glyph(1, 0.0, 0.0, 0)],
        }
    }

    fn image() -> Command {
        Command::Image {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            image: "i".into(),
            crop: None,
            rotation: 0.0,
        }
    }

    #[test]
    fn paragraphs_split_on_gaps_and_pictures() {
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                glyphs(0.0, 10.0),
                glyphs(0.0, 22.0), // next line (12 pt) → same P
                glyphs(0.0, 80.0), // far below → new P
                image(),
                glyphs(0.0, 82.0),
            ],
        };
        let n = auto_paragraphs(&page, 0);
        let roles: Vec<Role> = n.iter().map(|n| n.role).collect();
        assert_eq!(roles, [Role::P, Role::P, Role::Figure, Role::P]);
        assert_eq!(
            n[0].kids,
            [StructKid::Content {
                page: 0,
                start: 0,
                end: 2
            }]
        );
    }

    #[test]
    fn grid_rows_and_cells_in_reading_order() {
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![glyphs(50.0, 30.0), glyphs(0.0, 30.0), glyphs(0.0, 10.0)],
        };
        let t = auto_grid(&page, 0);
        let StructKid::Node(r0) = &t[0].kids[0] else {
            panic!()
        };
        assert_eq!(r0.kids.len(), 1); // y = 10 row: command 2
        let StructKid::Node(r1) = &t[0].kids[1] else {
            panic!()
        };
        let starts: Vec<usize> = r1
            .kids
            .iter()
            .map(|k| match k {
                StructKid::Node(td) => match td.kids[0] {
                    StructKid::Content { start, .. } => start,
                    StructKid::Node(_) => 99,
                },
                StructKid::Content { .. } => 99,
            })
            .collect();
        assert_eq!(starts, [1, 0]); // x 0 before x 50
    }

    #[test]
    fn marker_wraps_artifacts_and_tags() {
        let marks = [Mark {
            start: 1,
            end: 3,
            mcid: 0,
            tag: "P",
        }];
        let mut c = pdf_writer::Content::new();
        let mut m = Marker::new(&marks);
        for i in 0..4 {
            m.before(&mut c, i);
            c.move_to(0.0, 0.0);
        }
        m.close(&mut c);
        let s = String::from_utf8(c.finish().to_vec()).unwrap();
        let flat = s.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            flat,
            "/Artifact BMC 0 0 m EMC /P << /MCID 0 >> BDC 0 0 m 0 0 m EMC /Artifact BMC 0 0 m EMC"
        );
    }
}

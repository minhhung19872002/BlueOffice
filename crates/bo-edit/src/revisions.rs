//! Tracked changes (ECMA-376 Part 1, §17.13.5): listing revisions, accepting or
//! rejecting them, and the primitives used to record new edits as revisions.
//!
//! Revisions live on the model: run insertions/deletions/moves and formatting
//! changes in [`RunProps`] (`ins`, `del`, `change`), paragraph-mark insertions /
//! deletions and paragraph formatting changes in [`ParaProps`].

use crate::para::{self, Atom};
use crate::section::{hf_slot, HfVariant};
use crate::Pos;
use bo_model::index::body_paragraphs;
use bo_model::{
    Block, Cell, CellProps, Document, ParaProps, Paragraph, RevMark, RevisionInfo, RunProps,
    Section, Table, TableWidth,
};
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
    /// §17.13.5.19 ins in trPr (inserted table row)
    RowInsert,
    /// §17.13.5.12 del in trPr (deleted table row)
    RowDelete,
    /// §17.13.5.36 tcPrChange
    CellFormat,
    /// §17.13.5.34 tblPrChange
    TableFormat,
    /// §17.13.5.32 sectPrChange (page setup)
    SectionFormat,
    /// §17.13.5.4 cellIns (inserted cell; inserted columns are inserted cells)
    CellInsert,
    /// §17.13.5.3 cellDel (deleted cell; deleted columns are deleted cells)
    CellDelete,
    /// §17.13.5.6 cellMerge (vertical merge or split)
    CellMerge,
    /// §17.13.5.37 trPrChange (row properties, e.g. repeated header row)
    RowFormat,
    /// §17.13.5.33 tblGridChange (column widths)
    TableGrid,
}

/// Where a revision outside the main text lives: a header or footer of a section
/// (§17.10) or a footnote / endnote (§17.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StoryRef {
    /// A header of a section (§17.10.4 headerReference).
    Header {
        /// 0-based section index.
        section: usize,
        /// Which header of the section (default, first page, even pages).
        variant: crate::section::HfVariant,
    },
    /// A footer of a section (§17.10.3 footerReference).
    Footer {
        /// 0-based section index.
        section: usize,
        /// Which footer of the section (default, first page, even pages).
        variant: crate::section::HfVariant,
    },
    /// A footnote (§17.11.10 footnote).
    Footnote {
        /// Note id (`w:footnote/@w:id`).
        id: i32,
    },
    /// An endnote (§17.11.2 endnote).
    Endnote {
        /// Note id (`w:endnote/@w:id`).
        id: i32,
    },
}

/// One revision, for the review pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionItem {
    /// Revision id (`@w:id`).
    pub id: i32,
    /// Kind of change.
    pub kind: RevKind,
    /// Author name.
    pub author: String,
    /// Date of the change (`ST_DateTime`), if set.
    pub date: Option<String>,
    /// Start of the affected range: a position in the main text, or in the story's own
    /// paragraphs when `story` is set.
    pub start: Pos,
    /// End of the affected range.
    pub end: Pos,
    /// Affected text (shortened).
    pub text: String,
    /// Header/footer or note holding the revision (`None`: main text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story: Option<StoryRef>,
    /// Formatting changes: which properties changed (keys such as `bold`, `font`,
    /// `align`, `indent`; see [`run_detail`] and [`para_detail`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub detail: Vec<&'static str>,
}

const MAX_TEXT: usize = 80;

fn push_text(item: &mut RevisionItem, t: &str) {
    if item.text.chars().count() < MAX_TEXT {
        item.text.extend(t.chars().take(MAX_TEXT));
    }
}

/// All revisions: those of the main text in document order, then those of the
/// headers/footers (section order, default/first/even) and of the footnotes and
/// endnotes (positions are then relative to the story). Parts of one revision (same
/// id and kind) spread over several runs or paragraphs are reported once.
pub fn list(doc: &Document) -> Vec<RevisionItem> {
    let mut out = list_main(doc);
    let mut story = |blocks: &[Block], at: StoryRef| {
        if !blocks_have_revisions(blocks) {
            return;
        }
        let d = Document {
            sections: vec![Section::new(blocks.to_vec())],
            ..Document::default()
        };
        out.extend(list_main(&d).into_iter().map(|mut it| {
            it.story = Some(at);
            it
        }));
    };
    for (si, s) in doc.sections.iter().enumerate() {
        for (header, set) in [(true, &s.headers), (false, &s.footers)] {
            for v in [HfVariant::Default, HfVariant::First, HfVariant::Even] {
                if let Some(b) = hf_slot(set, v) {
                    let at = if header {
                        StoryRef::Header {
                            section: si,
                            variant: v,
                        }
                    } else {
                        StoryRef::Footer {
                            section: si,
                            variant: v,
                        }
                    };
                    story(b, at);
                }
            }
        }
    }
    for (id, b) in &doc.footnotes {
        story(b, StoryRef::Footnote { id: *id });
    }
    for (id, b) in &doc.endnotes {
        story(b, StoryRef::Endnote { id: *id });
    }
    out
}

/// Revisions of the main text (sections' blocks and section properties).
fn list_main(doc: &Document) -> Vec<RevisionItem> {
    let mut out: Vec<RevisionItem> = Vec::new();
    if !doc
        .sections
        .iter()
        .any(|s| s.change.is_some() || blocks_have_revisions(&s.blocks))
    {
        return out;
    }
    let mut add = |kind: RevKind,
                   info: &RevisionInfo,
                   start: Pos,
                   end: Pos,
                   text: &str,
                   detail: &[&'static str]| {
        if let Some(it) = out
            .iter_mut()
            .rev()
            .find(|i| i.id == info.id && i.kind == kind && i.author == info.author)
        {
            it.end = it.end.max(end);
            push_text(it, text);
            for d in detail {
                if !it.detail.contains(d) {
                    it.detail.push(d);
                }
            }
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
            story: None,
            detail: detail.to_vec(),
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
                &para_detail(&c.old, &p.props),
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
                add(k, &m.info, s, e, &text, &[]);
            }
            if let Some(m) = &props.del {
                let k = if m.moved {
                    RevKind::MoveFrom
                } else {
                    RevKind::Delete
                };
                add(k, &m.info, s, e, &text, &[]);
            }
            if let Some(c) = &props.change {
                add(
                    RevKind::Format,
                    &c.info,
                    s,
                    e,
                    &text,
                    &run_detail(&c.old, props),
                );
            }
            off += l;
        }
        for (m, k, moved) in [
            (&p.props.mark_ins, RevKind::ParagraphInsert, RevKind::MoveTo),
            (
                &p.props.mark_del,
                RevKind::ParagraphDelete,
                RevKind::MoveFrom,
            ),
        ] {
            if let Some(m) = m {
                // A moved paragraph mark is part of its move (§17.13.5.22 / .25).
                let k = if m.moved { moved } else { k };
                add(
                    k,
                    &m.info,
                    Pos::new(pid, len),
                    Pos::new(pid + 1, 0),
                    "¶",
                    &[],
                );
            }
        }
    }
    // Rows, cell/table properties and page setup, positioned on their paragraphs.
    let mut next = 0usize;
    let mut last_len = 0usize;
    let mut structural = Vec::new();
    for s in &doc.sections {
        let first = next;
        walk_structure(&s.blocks, &mut next, &mut last_len, &mut structural);
        if let Some(ch) = &s.change {
            let end = Pos::new(next.saturating_sub(1).max(first), last_len);
            structural.push((
                RevKind::SectionFormat,
                ch.info.clone(),
                Pos::new(first, 0),
                end,
            ));
        }
    }
    // Property and grid changes recorded by a structural edit (an inserted column
    // widens the grid and narrows the other cells) are part of that revision.
    let cell_ids: Vec<i32> = structural
        .iter()
        .filter(|s| {
            matches!(
                s.0,
                RevKind::CellInsert | RevKind::CellDelete | RevKind::CellMerge
            )
        })
        .map(|s| s.1.id)
        .collect();
    let any_ids: Vec<i32> = structural
        .iter()
        .filter(|s| s.0 != RevKind::TableGrid)
        .map(|s| s.1.id)
        .collect();
    structural.retain(|s| match s.0 {
        RevKind::CellFormat | RevKind::TableFormat => !cell_ids.contains(&s.1.id),
        RevKind::TableGrid => !any_ids.contains(&s.1.id),
        _ => true,
    });
    for (k, info, s, e) in structural {
        add(k, &info, s, e, "", &[]);
    }
    merge_move_halves(doc, &mut out);
    out.sort_by_key(|i| i.start);
    out
}

/// One item per half of a named move: a file gives every element its own id
/// (§17.13.1), so the text and paragraph marks of one half arrive with several ids
/// tied by their range name (§17.13.5.23 moveFromRangeStart `w:name`). Resolving the
/// kept id resolves the whole move ([`crate::moves::MoveGroups::expand`]).
fn merge_move_halves(doc: &Document, out: &mut Vec<RevisionItem>) {
    if !out
        .iter()
        .any(|i| matches!(i.kind, RevKind::MoveFrom | RevKind::MoveTo))
    {
        return;
    }
    let groups = crate::moves::move_groups(doc);
    let mut kept: Vec<RevisionItem> = Vec::with_capacity(out.len());
    for it in out.drain(..) {
        let name = matches!(it.kind, RevKind::MoveFrom | RevKind::MoveTo)
            .then(|| groups.name_of(it.id))
            .flatten();
        let into = name.and_then(|n| {
            kept.iter_mut().find(|k| {
                k.kind == it.kind && k.author == it.author && groups.name_of(k.id) == Some(n)
            })
        });
        match into {
            Some(k) => {
                k.start = k.start.min(it.start);
                k.end = k.end.max(it.end);
                if it.text != "¶" {
                    push_text(k, &it.text);
                }
            }
            None => kept.push(it),
        }
    }
    *out = kept;
}

type Structural = (RevKind, RevisionInfo, Pos, Pos);

/// Table revisions of `blocks` in document order; `next` counts body paragraphs (ids as
/// in `bo_model::index::body_paragraphs`), `last_len` is the length of the last one.
fn walk_structure(
    blocks: &[Block],
    next: &mut usize,
    last_len: &mut usize,
    out: &mut Vec<Structural>,
) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                *next += 1;
                *last_len = para::len(&para::flatten(p));
            }
            Block::Table(t) => {
                let t0 = *next;
                for row in &t.rows {
                    let r0 = *next;
                    for c in &row.cells {
                        let c0 = *next;
                        walk_structure(&c.blocks, next, last_len, out);
                        let end = Pos::new(next.saturating_sub(1).max(c0), *last_len);
                        let p = &c.props;
                        let marks = [
                            (p.change.as_ref().map(|c| &c.info), RevKind::CellFormat),
                            (p.ins.as_ref().map(|m| &m.info), RevKind::CellInsert),
                            (p.del.as_ref().map(|m| &m.info), RevKind::CellDelete),
                            (p.merge.as_ref().map(|m| &m.info), RevKind::CellMerge),
                        ];
                        for (info, k) in marks {
                            if let Some(info) = info {
                                out.push((k, info.clone(), Pos::new(c0, 0), end));
                            }
                        }
                    }
                    let end = Pos::new(next.saturating_sub(1).max(r0), *last_len);
                    if let Some(m) = &row.props.ins {
                        out.push((RevKind::RowInsert, m.info.clone(), Pos::new(r0, 0), end));
                    }
                    if let Some(m) = &row.props.del {
                        out.push((RevKind::RowDelete, m.info.clone(), Pos::new(r0, 0), end));
                    }
                    if let Some(ch) = &row.props.change {
                        out.push((RevKind::RowFormat, ch.info.clone(), Pos::new(r0, 0), end));
                    }
                }
                let end = Pos::new(next.saturating_sub(1).max(t0), *last_len);
                if let Some(ch) = &t.props.change {
                    out.push((RevKind::TableFormat, ch.info.clone(), Pos::new(t0, 0), end));
                }
                if let Some(g) = &t.grid_change {
                    out.push((RevKind::TableGrid, g.info.clone(), Pos::new(t0, 0), end));
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Cheap pre-check for [`list`]: any revision mark on a paragraph or run of `blocks`.
fn blocks_have_revisions(blocks: &[Block]) -> bool {
    blocks.iter().any(|b| match b {
        Block::Paragraph(p) => {
            p.props.change.is_some()
                || p.props.mark_ins.is_some()
                || p.props.mark_del.is_some()
                || p.runs.iter().any(|r| {
                    r.props.ins.is_some() || r.props.del.is_some() || r.props.change.is_some()
                })
        }
        Block::Table(t) => {
            t.props.change.is_some()
                || t.grid_change.is_some()
                || t.rows.iter().any(|r| {
                    r.props.ins.is_some()
                        || r.props.del.is_some()
                        || r.props.change.is_some()
                        || r.cells
                            .iter()
                            .any(|c| c.props.has_revision() || blocks_have_revisions(&c.blocks))
                })
        }
        Block::Opaque(_) => false,
    })
}

/// Largest revision id in `blocks`.
fn blocks_max_id(v: &[Block], m: &mut i32) {
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
                if let Some(c) = &t.props.change {
                    *m = (*m).max(c.info.id);
                }
                if let Some(g) = &t.grid_change {
                    *m = (*m).max(g.info.id);
                }
                for row in &t.rows {
                    for x in [&row.props.ins, &row.props.del].into_iter().flatten() {
                        *m = (*m).max(x.info.id);
                    }
                    if let Some(c) = &row.props.change {
                        *m = (*m).max(c.info.id);
                    }
                    for c in &row.cells {
                        let p = &c.props;
                        for info in [
                            p.change.as_ref().map(|c| &c.info),
                            p.ins.as_ref().map(|x| &x.info),
                            p.del.as_ref().map(|x| &x.info),
                            p.merge.as_ref().map(|x| &x.info),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            *m = (*m).max(info.id);
                        }
                        blocks_max_id(&c.blocks, m);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Largest revision id used in the document — main text, headers/footers and notes
/// (revision ids are unique across the document, §17.13.5) — 0 when there is none.
pub fn max_id(doc: &Document) -> i32 {
    let mut m = 0;
    for s in &doc.sections {
        if let Some(c) = &s.change {
            m = m.max(c.info.id);
        }
        blocks_max_id(&s.blocks, &mut m);
        for set in [&s.headers, &s.footers] {
            for b in [&set.default, &set.first, &set.even].into_iter().flatten() {
                blocks_max_id(b, &mut m);
            }
        }
    }
    for b in doc.footnotes.values().chain(doc.endnotes.values()) {
        blocks_max_id(b, &mut m);
    }
    // Move range markers share the id space (§17.13.5.23 moveFromRangeStart w:id).
    m.max(crate::moves::move_groups(doc).max_marker_id())
}

/// Which run properties a formatting change (§17.13.5.31 rPrChange) changed, as keys
/// the hosts translate; `other` when only properties not listed here differ.
pub fn run_detail(old: &RunProps, new: &RunProps) -> Vec<&'static str> {
    let (o, n) = (old, new);
    let checks: [(&'static str, bool); 14] = [
        ("style", o.style != n.style),
        (
            "font",
            o.font != n.font
                || o.font_east_asia != n.font_east_asia
                || o.font_cs != n.font_cs
                || o.font_h_ansi != n.font_h_ansi
                || o.theme_ascii != n.theme_ascii
                || o.theme_h_ansi != n.theme_h_ansi
                || o.theme_east_asia != n.theme_east_asia
                || o.theme_cs != n.theme_cs,
        ),
        ("size", o.size != n.size),
        ("bold", o.bold != n.bold),
        ("italic", o.italic != n.italic),
        ("underline", o.underline != n.underline),
        (
            "strike",
            o.strike != n.strike || o.double_strike != n.double_strike,
        ),
        ("color", o.color != n.color),
        (
            "highlight",
            o.highlight != n.highlight || o.shading != n.shading,
        ),
        ("vertAlign", o.vert_align != n.vert_align),
        ("caps", o.caps != n.caps || o.small_caps != n.small_caps),
        ("charSpacing", o.spacing != n.spacing || o.kern != n.kern),
        ("hidden", o.hidden != n.hidden),
        ("language", o.lang != n.lang),
    ];
    let mut v: Vec<&'static str> = checks.iter().filter(|c| c.1).map(|c| c.0).collect();
    if v.is_empty() {
        let bare = |p: &RunProps| RunProps {
            link: None,
            ..p.without_revisions()
        };
        if bare(o) != bare(n) {
            v.push("other");
        }
    }
    v
}

/// Which paragraph properties a formatting change (§17.13.5.29 pPrChange) changed, as
/// keys the hosts translate; `other` when only properties not listed here differ.
pub fn para_detail(old: &ParaProps, new: &ParaProps) -> Vec<&'static str> {
    let (o, n) = (old, new);
    let checks: [(&'static str, bool); 10] = [
        ("style", o.style != n.style),
        ("align", o.align != n.align),
        (
            "indent",
            o.indent_left != n.indent_left
                || o.indent_right != n.indent_right
                || o.indent_first_line != n.indent_first_line,
        ),
        (
            "paraSpacing",
            o.space_before != n.space_before
                || o.space_after != n.space_after
                || o.line_spacing != n.line_spacing
                || o.contextual_spacing != n.contextual_spacing,
        ),
        ("numbering", o.num != n.num),
        (
            "pagination",
            o.page_break_before != n.page_break_before
                || o.keep_next != n.keep_next
                || o.keep_lines != n.keep_lines
                || o.widow_control != n.widow_control,
        ),
        ("tabs", o.tabs != n.tabs),
        ("borders", o.borders != n.borders || o.shading != n.shading),
        ("outline", o.outline_level != n.outline_level),
        ("direction", o.bidi != n.bidi),
    ];
    let mut v: Vec<&'static str> = checks.iter().filter(|c| c.1).map(|c| c.0).collect();
    if v.is_empty() {
        let bare = |p: &ParaProps| ParaProps {
            mark_ins: None,
            mark_del: None,
            change: None,
            unknown: Vec::new(),
            ..p.clone()
        };
        if bare(o) != bare(n) {
            v.push("other");
        }
    }
    v
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
                if let Some(t2) = resolve_table(t, f, accept) {
                    out.push(Block::Table(t2));
                } else if carry.is_none() && i + 1 >= blocks.len() && out.is_empty() {
                    // A container never ends up empty.
                    out.push(Block::Paragraph(Paragraph::default()));
                }
            }
            Block::Opaque(_) => out.push(b.clone()),
        }
    }
    out
}

/// Resolve the revisions of a table: rows inserted/deleted (§17.13.5.19, §17.13.5.12),
/// cell and table property changes (§17.13.5.36, §17.13.5.34) and its content.
/// `None` when no row remains.
fn resolve_table(t: &Table, f: Filter<'_>, accept: bool) -> Option<Table> {
    let hit = |m: &Option<RevMark>| m.as_ref().is_some_and(|m| f(m.info.id));
    // Cells removed by resolving: accepted deletions, rejected insertions
    // (§17.13.5.3 cellDel, §17.13.5.4 cellIns).
    let gone = |c: &Cell| (accept && hit(&c.props.del)) || (!accept && hit(&c.props.ins));
    // Accepting deleted cells removes the grid columns left without cells.
    let removed = if accept {
        t.removed_columns(&gone)
    } else {
        Vec::new()
    };
    let spans = t.markup_spans();
    let mut t2 = t.clone();
    if let Some(c) = t2.props.change.take() {
        if !f(c.info.id) {
            t2.props.change = Some(c);
        } else if !accept {
            t2.props = c.old;
        }
    }
    // §17.13.5.33 tblGridChange
    if let Some(g) = t2.grid_change.take() {
        if !f(g.info.id) {
            t2.grid_change = Some(g);
        } else if !accept {
            t2.grid = g.old;
        }
    }
    for (row, sp) in t2.rows.iter_mut().zip(&spans) {
        let mut cells = Vec::with_capacity(row.cells.len());
        for (mut c, &(s, _)) in std::mem::take(&mut row.cells).into_iter().zip(sp) {
            if gone(&c) {
                continue;
            }
            // Columns removed under a kept spanning cell shrink it.
            let k = c.props.grid_span.max(1) as usize;
            let lost: Vec<usize> = (s..s + k)
                .filter(|g| removed.get(*g).copied().unwrap_or(false))
                .collect();
            if !lost.is_empty() {
                c.props.grid_span = (k - lost.len()).max(1) as u32;
                if let Some(TableWidth::Dxa(w)) = &mut c.props.width {
                    let cut: i32 = lost
                        .iter()
                        .filter_map(|g| t.grid.get(*g))
                        .map(|w| w.0)
                        .sum();
                    w.0 = (w.0 - cut).max(0);
                }
            }
            cells.push(c);
        }
        row.cells = cells;
    }
    if removed.iter().any(|r| *r) {
        let mut g = 0;
        t2.grid.retain(|_| {
            let keep = !removed.get(g).copied().unwrap_or(false);
            g += 1;
            keep
        });
    }
    t2.rows.retain(|row| {
        let gone = (hit(&row.props.del) && accept) || (hit(&row.props.ins) && !accept);
        !gone && !row.cells.is_empty()
    });
    for row in &mut t2.rows {
        if hit(&row.props.del) {
            row.props.del = None;
        }
        if hit(&row.props.ins) {
            row.props.ins = None;
        }
        // §17.13.5.37 trPrChange
        if let Some(ch) = row.props.change.take() {
            if !f(ch.info.id) {
                row.props.change = Some(ch);
            } else if !accept {
                row.props.header = ch.header;
                row.props.cant_split = ch.cant_split;
                row.props.height = ch.height;
            }
        }
        for c in &mut row.cells {
            if hit(&c.props.del) {
                c.props.del = None;
            }
            if hit(&c.props.ins) {
                c.props.ins = None;
            }
            if let Some(ch) = c.props.change.take() {
                if !f(ch.info.id) {
                    c.props.change = Some(ch);
                } else if !accept {
                    let keep = c.props.clone();
                    c.props = CellProps {
                        ins: keep.ins,
                        del: keep.del,
                        merge: keep.merge,
                        ..ch.old.bare()
                    };
                }
            }
            // §17.13.5.6 cellMerge
            if let Some(m) = c.props.merge.take() {
                if !f(m.info.id) {
                    c.props.merge = Some(m);
                } else if !accept {
                    c.props.v_merge = m.v_merge_orig;
                }
            }
            c.blocks = resolve_blocks(&c.blocks, f, accept);
        }
    }
    (!t2.rows.is_empty()).then_some(t2)
}

/// Resolve the page setup change of a section (§17.13.5.32 sectPrChange).
pub(crate) fn resolve_section(s: &Section, f: Filter<'_>, accept: bool) -> Option<Section> {
    let ch = s.change.as_ref().filter(|c| f(c.info.id))?;
    let mut n = s.clone();
    n.change = None;
    if !accept {
        n.page = ch.page;
        n.columns = ch.columns.clone();
        n.title_page = ch.title_page;
        n.break_type = ch.break_type;
        n.page_number_start = ch.page_number_start;
    }
    Some(n)
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

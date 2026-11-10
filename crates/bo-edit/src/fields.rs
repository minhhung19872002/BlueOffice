//! Field updating (ECMA-376 Part 1, §17.16): tables of contents and of figures
//! (§17.16.5.68 TOC), indexes (§17.16.5.29 INDEX), tables of authorities (§17.16.5.67
//! TOA), caption numbers (§17.16.5.56 SEQ), cross-references (§17.16.5.51 REF,
//! §17.16.5.45 PAGEREF) and mail-merge fields (§17.16.5.35 MERGEFIELD).
//!
//! Page numbers come from the layout: callers pass the displayed page number of
//! every body paragraph (by paragraph id, see `bo_model::index`).

use crate::para::{self, Atom};
use crate::refs;
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{
    fingerprint, Alignment, Block, Document, HalfPoints, ParaProps, Paragraph, Raw, Run,
    RunContent, RunProps, Style, StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Twips,
};
use std::collections::HashMap;

/// Result of [`update`]: new blocks for every section, a changed style sheet and
/// the number of fields whose result changed.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdate {
    /// New blocks of every section, in section order.
    pub sections: Vec<Vec<Block>>,
    /// The updated style sheet, when it had to change.
    pub styles: Option<StyleSheet>,
    /// Number of fields whose result changed.
    pub count: usize,
}

/// Complex-field structure marks (§17.16.18 fldChar, §17.16.23 instrText).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Mark {
    Begin,
    Separate,
    End,
    Instr(String),
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub(crate) fn mark_of(c: &RunContent) -> Option<Mark> {
    let RunContent::Opaque(raw) = c else {
        return None;
    };
    let x = &raw.xml;
    if x.contains("fldChar") {
        let has = |k: &str| x.contains(&format!("\"{k}\"")) || x.contains(&format!("'{k}'"));
        if has("begin") {
            Some(Mark::Begin)
        } else if has("separate") {
            Some(Mark::Separate)
        } else if has("end") {
            Some(Mark::End)
        } else {
            None
        }
    } else if x.contains("instrText") {
        let gt = x.find('>')?;
        let rest = &x[gt + 1..];
        if x[..gt].ends_with('/') {
            return Some(Mark::Instr(String::new()));
        }
        let end = rest.rfind("</").unwrap_or(rest.len());
        Some(Mark::Instr(unescape(&rest[..end])))
    } else {
        None
    }
}

/// Fields of a paragraph's atoms in order: (instruction, first atom, last atom). Both forms
/// count: simple fields (one [`RunContent::Field`] atom) and complex fields
/// (§17.16.18 fldChar begin … end, outermost only) complete inside the paragraph.
pub(crate) fn para_fields(atoms: &[Atom]) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    // (begin atom, instruction, instruction finished) of open complex fields.
    let mut stack: Vec<(usize, String, bool)> = Vec::new();
    for (i, (props, c)) in atoms.iter().enumerate() {
        if props.del.is_some() {
            continue;
        }
        if let RunContent::Field { instr, .. } = c {
            if stack.is_empty() {
                out.push((instr.trim().to_owned(), i, i));
            }
            continue;
        }
        match mark_of(c) {
            Some(Mark::Begin) => stack.push((i, String::new(), false)),
            Some(Mark::Instr(t)) => {
                if let Some(o) = stack.last_mut() {
                    if !o.2 {
                        o.1.push_str(&t);
                    }
                }
            }
            Some(Mark::Separate) => {
                if let Some(o) = stack.last_mut() {
                    o.2 = true;
                }
            }
            Some(Mark::End) => {
                if let Some((b, instr, _)) = stack.pop() {
                    if stack.is_empty() {
                        out.push((instr.trim().to_owned(), b, i));
                    }
                }
            }
            None => {}
        }
    }
    out
}

/// First word of a field instruction, upper-cased.
pub fn field_kind(instr: &str) -> String {
    instr
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

/// Arguments of an instruction: quoted strings stay together, switches included.
pub fn field_args(instr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in instr.chars() {
        match c {
            '"' => {
                if quoted {
                    out.push(std::mem::take(&mut cur));
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A TOC / INDEX / TOA / BIBLIOGRAPHY field spanning top-level paragraphs `first..=last` of a
/// section.
#[derive(Debug, Clone)]
pub(crate) struct TocRange {
    pub(crate) first: usize,
    pub(crate) last: usize,
    /// Atom index of `begin` in `first` and of `end` in `last`.
    begin: usize,
    end: usize,
    /// Atom index of `separate` in `first` (head = begin..=separate) when there.
    separate: Option<usize>,
    instr: String,
}

impl TocRange {
    /// The field instruction.
    pub(crate) fn instr(&self) -> &str {
        &self.instr
    }
}

/// Fields whose result is a run of paragraphs (§17.16.5.68 TOC, §17.16.5.29 INDEX,
/// §17.16.5.67 TOA, §17.16.5.7 BIBLIOGRAPHY).
const BLOCK_FIELDS: [&str; 4] = ["TOC", "INDEX", "TOA", "BIBLIOGRAPHY"];

/// TOC / INDEX / TOA fields among the top-level paragraphs of a block list.
pub(crate) fn toc_ranges(blocks: &[Block]) -> Vec<TocRange> {
    struct Open {
        bi: usize,
        ai: usize,
        instr: String,
        separate: Option<(usize, usize)>,
    }
    let mut out = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    for (bi, b) in blocks.iter().enumerate() {
        let Block::Paragraph(p) = b else {
            continue;
        };
        for (ai, (_, item)) in para::flatten(p).iter().enumerate() {
            match mark_of(item) {
                Some(Mark::Begin) => stack.push(Open {
                    bi,
                    ai,
                    instr: String::new(),
                    separate: None,
                }),
                Some(Mark::Instr(t)) => {
                    if let Some(o) = stack.last_mut() {
                        if o.separate.is_none() {
                            o.instr.push_str(&t);
                        }
                    }
                }
                Some(Mark::Separate) => {
                    if let Some(o) = stack.last_mut() {
                        o.separate.get_or_insert((bi, ai));
                    }
                }
                Some(Mark::End) => {
                    if let Some(o) = stack.pop() {
                        if stack.is_empty() && BLOCK_FIELDS.contains(&field_kind(&o.instr).as_str())
                        {
                            out.push(TocRange {
                                first: o.bi,
                                last: bi,
                                begin: o.ai,
                                end: ai,
                                separate: o.separate.filter(|s| s.0 == o.bi).map(|s| s.1),
                                instr: o.instr.trim().to_owned(),
                            });
                        }
                    }
                }
                None => {}
            }
        }
    }
    out
}

/// Heading levels included by a TOC instruction (`\o "1-3"`; default 1–9).
pub(crate) fn toc_levels(instr: &str) -> (u8, u8) {
    let args = field_args(instr);
    if let Some(i) = args.iter().position(|a| a.eq_ignore_ascii_case("\\o")) {
        if let Some(r) = args.get(i + 1) {
            let mut it = r.split('-').map(|x| x.trim().parse::<u8>().ok());
            if let (Some(Some(a)), Some(Some(b))) = (it.next(), it.next()) {
                return (a.clamp(1, 9), b.clamp(1, 9));
            }
        }
    }
    (1, 9)
}

/// Effective outline level (0-based) of a paragraph: direct, then its style chain (§17.3.1.20).
pub fn outline_level(doc: &Document, p: &Paragraph) -> Option<u8> {
    if let Some(l) = p.props.outline_level {
        return (l < 9).then_some(l);
    }
    let style = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    })?;
    doc.styles
        .resolve_paragraph(&style)
        .0
        .outline_level
        .filter(|l| *l < 9)
}

/// Displayed text of content (field results instead of placeholders, deletions left out).
pub(crate) fn display_text(atoms: &[Atom]) -> String {
    let mut s = String::new();
    for (props, item) in atoms {
        if props.del.is_some() {
            continue;
        }
        match item {
            RunContent::Text(t) => s.push_str(t),
            RunContent::Field { cached, .. } => s.push_str(cached),
            // A check box / drop-down form field shows its state (§17.16.17 ffData).
            RunContent::FormField(f) => s.push_str(&f.display().unwrap_or_default()),
            RunContent::Tab | RunContent::LineBreak => s.push(' '),
            RunContent::NoBreakHyphen => s.push('-'),
            _ => {}
        }
    }
    s
}

/// Bookmark marks of a paragraph: (atom index, name or "" for an end, id).
pub(crate) fn bookmark_marks(atoms: &[Atom]) -> Vec<(usize, Option<String>, String)> {
    let attr = |x: &str, a: &str| {
        let key = format!("{a}=\"");
        let i = x.find(&key)? + key.len();
        Some(unescape(&x[i..i + x[i..].find('"')?]))
    };
    let mut v = Vec::new();
    for (i, (_, c)) in atoms.iter().enumerate() {
        if let RunContent::Markup(raw) = c {
            let x = &raw.xml;
            if x.contains("bookmarkStart") {
                if let (Some(n), Some(id)) = (attr(x, "w:name"), attr(x, "w:id")) {
                    v.push((i, Some(n), id));
                }
            } else if x.contains("bookmarkEnd") {
                if let Some(id) = attr(x, "w:id") {
                    v.push((i, None, id));
                }
            }
        }
    }
    v
}

/// A bookmark of the main text.
#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    /// Bookmark name (`w:bookmarkStart @w:name`).
    pub name: String,
    /// Body paragraph id of its start.
    pub para: usize,
    /// Displayed text between its start and end.
    pub text: String,
}

/// Bookmarks of the main text in document order (ECMA-376 Part 1, §17.13.6.2).
pub fn bookmarks(doc: &Document) -> Vec<Bookmark> {
    let paths = body_paragraphs(doc);
    let mut out: Vec<Bookmark> = Vec::new();
    // id → index into `out` of bookmarks still open.
    let mut open: HashMap<String, usize> = HashMap::new();
    for (pid, path) in paths.iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let atoms = para::flatten(p);
        let marks = bookmark_marks(&atoms);
        let mut last = 0;
        // Text of segments between marks goes to every bookmark open at that point.
        let add_text =
            |from: usize, to: usize, open: &HashMap<String, usize>, out: &mut [Bookmark]| {
                let t = display_text(&atoms[from..to]);
                if !t.is_empty() {
                    for &i in open.values() {
                        out[i].text.push_str(&t);
                    }
                }
            };
        for (ai, name, id) in &marks {
            add_text(last, *ai, &open, &mut out);
            last = *ai + 1;
            match name {
                Some(n) => {
                    open.insert(id.clone(), out.len());
                    out.push(Bookmark {
                        name: n.clone(),
                        para: pid,
                        text: String::new(),
                    });
                }
                None => {
                    open.remove(id);
                }
            }
        }
        add_text(last, atoms.len(), &open, &mut out);
    }
    out
}

/// Largest bookmark id (`w:id`) in the main text.
pub(crate) fn max_bookmark_id(doc: &Document) -> i64 {
    let mut m = 0;
    for path in body_paragraphs(doc) {
        if let Some(p) = para_at(doc, &path) {
            for (_, _, id) in bookmark_marks(&para::flatten(p)) {
                m = m.max(id.parse::<i64>().unwrap_or(0));
            }
        }
    }
    m
}

/// Number formats of field results (§17.16.4.3 general formatting switches).
pub fn format_seq(n: i64, instr: &str) -> String {
    let args = field_args(instr);
    let fmt = args
        .iter()
        .position(|a| a == "\\*")
        .and_then(|i| args.get(i + 1))
        .map_or("ARABIC", String::as_str);
    let i = n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    match fmt {
        "ROMAN" => bo_model::format_number(i, &bo_model::NumFormat::UpperRoman),
        "roman" => bo_model::format_number(i, &bo_model::NumFormat::LowerRoman),
        "ALPHABETIC" => bo_model::format_number(i, &bo_model::NumFormat::UpperLetter),
        "alphabetic" => bo_model::format_number(i, &bo_model::NumFormat::LowerLetter),
        _ => n.to_string(),
    }
}

pub(crate) fn bookmark_xml(id: i64, name: &str) -> (Raw, Raw) {
    (
        Raw::new(
            format!("<w:bookmarkStart w:id=\"{id}\" w:name=\"{name}\"/>"),
            0,
        ),
        Raw::new(format!("<w:bookmarkEnd w:id=\"{id}\"/>"), 0),
    )
}

/// Built-in `TOC1`… paragraph styles and the `Caption` style (BlueDoc definitions).
pub fn builtin_field_style(id: &str) -> Option<Style> {
    if let Some(n) = id.strip_prefix("TOC").and_then(|n| n.parse::<i32>().ok()) {
        if !(1..=9).contains(&n) {
            return None;
        }
        let mut s = Style::new(id, &format!("toc {n}"), StyleKind::Paragraph);
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.indent_left = Some(Twips(220 * (n - 1)));
        s.ppr.space_after = Some(Twips(100));
        return Some(s);
    }
    // Index levels: each level indented a step more, wrapped lines hang (§17.16.5.29).
    if let Some(n) = id.strip_prefix("Index").and_then(|n| n.parse::<i32>().ok()) {
        if !(1..=9).contains(&n) {
            return None;
        }
        let mut s = Style::new(id, &format!("index {n}"), StyleKind::Paragraph);
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.indent_left = Some(Twips(240 * n));
        s.ppr.indent_first_line = Some(Twips(-240));
        s.ppr.space_after = Some(Twips(0));
        return Some(s);
    }
    let heading = |id: &str, name: &str| {
        let mut s = Style::new(id, name, StyleKind::Paragraph);
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.space_before = Some(Twips(120));
        s.ppr.space_after = Some(Twips(60));
        s.ppr.keep_next = Some(true);
        s.rpr.bold = Some(true);
        s
    };
    match id {
        "IndexHeading" => return Some(heading(id, "index heading")),
        "TOAHeading" => return Some(heading(id, "toa heading")),
        "TableofFigures" | "TableofAuthorities" => {
            let name = if id == "TableofFigures" {
                "table of figures"
            } else {
                "table of authorities"
            };
            let mut s = Style::new(id, name, StyleKind::Paragraph);
            s.based_on = Some("Normal".into());
            s.next = Some("Normal".into());
            s.ppr.space_after = Some(Twips(100));
            return Some(s);
        }
        _ => {}
    }
    // Bibliography entries: wrapped lines hang (the "hanging indent" of the APA, MLA and
    // Chicago reference lists); the number label of numeric styles sits in the hang.
    if id == "Bibliography" {
        let mut s = Style::new("Bibliography", "bibliography", StyleKind::Paragraph);
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.indent_left = Some(Twips(720));
        s.ppr.indent_first_line = Some(Twips(-720));
        return Some(s);
    }
    if id == "Caption" {
        let mut s = Style::new("Caption", "caption", StyleKind::Paragraph);
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.space_after = Some(Twips(200));
        s.rpr.italic = Some(true);
        s.rpr.size = Some(HalfPoints(20));
        return Some(s);
    }
    None
}

/// Add missing built-in styles; returns the new sheet when something was added.
pub(crate) fn ensure_styles(sheet: &StyleSheet, ids: &[String]) -> Option<StyleSheet> {
    let mut out = sheet.clone();
    for id in ids {
        if out.styles.iter().any(|s| &s.id == id) {
            continue;
        }
        if let Some(s) = builtin_field_style(id) {
            out.styles.push(s);
        }
    }
    (out != *sheet).then_some(out)
}

/// Content width (twips) of the first section, for the right-aligned TOC tab.
pub(crate) fn text_width(doc: &Document, section: usize) -> i32 {
    doc.sections.get(section).map_or(9355, |s| {
        let w = (s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0).max(1440);
        // Inside text columns (§17.6.4 cols) the tab stops at the column's right edge.
        match (s.columns.cols.first(), s.columns.count) {
            (Some(c), _) => c.width.0.max(720),
            (None, n) if n > 1 => {
                let n = i32::from(n);
                ((w - s.columns.space.0 * (n - 1)) / n).max(720)
            }
            _ => w,
        }
    })
}

/// The fixed atoms of a TOC field: begin, instruction, separate.
fn toc_head(instr: &str) -> Vec<Atom> {
    let r = |x: String| (RunProps::default(), RunContent::Opaque(Raw::new(x, 0)));
    vec![
        r("<w:fldChar w:fldCharType=\"begin\"/>".into()),
        r(format!(
            "<w:instrText xml:space=\"preserve\"> {} </w:instrText>",
            instr
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )),
        r("<w:fldChar w:fldCharType=\"separate\"/>".into()),
    ]
}

fn toc_end() -> Atom {
    (
        RunProps::default(),
        RunContent::Opaque(Raw::new("<w:fldChar w:fldCharType=\"end\"/>", 0)),
    )
}

/// A new paragraph holding an empty TOC field (filled by [`update`]).
pub fn new_toc_paragraph(instr: &str) -> Paragraph {
    let mut atoms = toc_head(instr);
    atoms.push(toc_end());
    Paragraph::new(ParaProps::default(), para::rebuild(atoms))
}

/// A new paragraph holding an empty TOC / INDEX / TOA field; `leader` (with the text
/// width `width`) sets the right tab stop its entries keep when updated (§17.3.1.37 tab).
pub fn new_block_field_paragraph(instr: &str, leader: Option<TabLeader>, width: i32) -> Paragraph {
    let mut p = new_toc_paragraph(instr);
    if let Some(leader) = leader {
        p.props.tabs = Some(vec![TabStop {
            pos: Twips(width),
            align: TabAlign::Right,
            leader,
        }]);
    }
    p
}

/// Replace the result of block field `t` with `entries` (the field's begin … separate
/// marks stay at the start of the first paragraph, its end mark at the end of the last).
/// Returns whether anything changed.
fn splice_block_field(blocks: &mut Vec<Block>, t: &TocRange, entries: Vec<Paragraph>) -> bool {
    let (Some(Block::Paragraph(first)), Some(Block::Paragraph(last))) =
        (blocks.get(t.first).cloned(), blocks.get(t.last).cloned())
    else {
        return false;
    };
    let fa = para::flatten(&first);
    let la = para::flatten(&last);
    let prefix: Vec<Atom> = fa[..t.begin.min(fa.len())].to_vec();
    let head: Vec<Atom> = match t.separate {
        Some(sep) if sep < fa.len() => fa[t.begin..=sep].to_vec(),
        _ => toc_head(&t.instr),
    };
    let end: Atom = la.get(t.end).cloned().unwrap_or_else(toc_end);
    let suffix: Vec<Atom> = la
        .get(t.end + 1..)
        .map(<[Atom]>::to_vec)
        .unwrap_or_default();
    let mut out: Vec<Block> = Vec::new();
    if entries.is_empty() {
        let mut a = prefix;
        a.extend(head);
        a.push(end);
        a.extend(suffix);
        out.push(Block::Paragraph(para::with_atoms(&first, a)));
    } else {
        let n = entries.len();
        for (i, mut e) in entries.into_iter().enumerate() {
            let mut a = Vec::new();
            if i == 0 {
                a.extend(prefix.clone());
                a.extend(head.clone());
            }
            a.extend(para::flatten(&e));
            if i + 1 == n {
                a.push(end.clone());
                a.extend(suffix.clone());
            }
            e.runs = para::rebuild(a);
            out.push(Block::Paragraph(e));
        }
    }
    // Same content (whatever the run grouping or source XML): keep the original.
    let same = |a: &Block, b: &Block| match (a, b) {
        (Block::Paragraph(x), Block::Paragraph(y)) => {
            x.props == y.props && para::flatten(x) == para::flatten(y)
        }
        _ => a == b,
    };
    let old = &blocks[t.first..=t.last];
    if old.len() != out.len() || !old.iter().zip(&out).all(|(a, b)| same(a, b)) {
        blocks.splice(t.first..=t.last, out);
        true
    } else {
        false
    }
}

/// Recompute only the citations (§17.16.5.8 CITATION) and bibliographies (§17.16.5.7
/// BIBLIOGRAPHY) from the document's sources — no page numbers needed.
pub(crate) fn update_cite(doc: &Document) -> FieldUpdate {
    let mut sections: Vec<Vec<Block>> = doc.sections.iter().map(|s| s.blocks.clone()).collect();
    let mut count = 0;
    let mut styles_needed: Vec<String> = Vec::new();
    if let Some(cite) = crate::cite::Context::of(doc, &sections) {
        count += cite.update_citations(&mut sections);
        for blocks in &mut sections {
            let mut ranges = toc_ranges(blocks);
            ranges.retain(|t| field_kind(&t.instr) == "BIBLIOGRAPHY");
            for t in ranges.into_iter().rev() {
                let entries = cite.bibliography(&t.instr, &mut styles_needed);
                if splice_block_field(blocks, &t, entries) {
                    count += 1;
                }
            }
        }
    }
    FieldUpdate {
        styles: ensure_styles(&doc.styles, &styles_needed),
        sections,
        count,
    }
}

/// Recompute TOC, SEQ, REF, PAGEREF fields. `pages[id]` = displayed page number of
/// body paragraph `id` (0 = unknown).
pub fn update(doc: &Document, pages: &[u32], env: &FieldEnv) -> FieldUpdate {
    let page = |id: usize| pages.get(id).copied().unwrap_or(0);
    let mut sections: Vec<Vec<Block>> = doc.sections.iter().map(|s| s.blocks.clone()).collect();
    let mut count = 0;
    let mut styles_needed: Vec<String> = Vec::new();
    // ---- 0. Citations (§17.16.5.8) from the document's sources.
    let cite = crate::cite::Context::of(doc, &sections);
    if let Some(c) = &cite {
        count += c.update_citations(&mut sections);
    }
    let paths = body_paragraphs(doc);
    let id_of: HashMap<Vec<usize>, usize> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i))
        .collect();

    // Bookmark pages of the current document (ids are valid for `pages`).
    let mut bm_page: HashMap<String, u32> = bookmarks(doc)
        .into_iter()
        .map(|b| (b.name, page(b.para)))
        .collect();

    // ---- 1. Tables of contents and figures, indexes, tables of authorities.
    let tocs: Vec<(usize, TocRange)> = sections
        .iter()
        .enumerate()
        .flat_map(|(si, b)| toc_ranges(b).into_iter().map(move |t| (si, t)))
        .collect();
    if !tocs.is_empty() {
        let in_toc = |path: &[usize]| {
            path.len() == 2
                && tocs
                    .iter()
                    .any(|(si, t)| path[0] == *si && (t.first..=t.last).contains(&path[1]))
        };
        let tocs_only = || tocs.iter().filter(|(_, t)| field_kind(&t.instr) == "TOC");
        // Headings go into tables of contents, captions of the listed labels into tables
        // of figures (`TOC \c` / `\a`, §17.16.5.68).
        let heading_toc = tocs_only().any(|(_, t)| refs::figure_label(&t.instr).is_none());
        let labels: Vec<String> = tocs_only()
            .filter_map(|(_, t)| refs::figure_label(&t.instr))
            .map(|(l, _)| l.to_lowercase())
            .collect();
        // Targets with their `_Toc` bookmark (added when missing).
        let mut next_bm = max_bookmark_id(doc);
        let mut targets: Vec<refs::Target> = Vec::new();
        let mut used_names: Vec<String> = bm_page.keys().cloned().collect();
        for (ordinal, path) in paths.iter().enumerate() {
            if in_toc(path) {
                continue;
            }
            let Some(p) = para_at(doc, path) else {
                continue;
            };
            let level = if heading_toc {
                outline_level(doc, p)
            } else {
                None
            };
            let atoms = para::flatten(p);
            let caption = if labels.is_empty() {
                None
            } else {
                refs::caption_of(&atoms).filter(|c| labels.contains(&c.0.to_lowercase()))
            };
            if level.is_none() && caption.is_none() {
                continue;
            }
            let text = display_text(&atoms).trim().to_owned();
            if text.is_empty() {
                continue;
            }
            let existing = bookmark_marks(&atoms)
                .into_iter()
                .find_map(|(_, n, _)| n.filter(|n| n.starts_with("_Toc")));
            let name = if let Some(n) = existing {
                n
            } else {
                let mut h = fingerprint(&(ordinal, &text)) % 1_000_000_000;
                let mut name = format!("_Toc{h:09}");
                while used_names.contains(&name) {
                    h = (h + 1) % 1_000_000_000;
                    name = format!("_Toc{h:09}");
                }
                used_names.push(name.clone());
                next_bm += 1;
                let (s, e) = bookmark_xml(next_bm, &name);
                let mut a = atoms.clone();
                a.insert(0, (RunProps::default(), RunContent::Markup(s)));
                a.push((RunProps::default(), RunContent::Markup(e)));
                if let Some(Block::Paragraph(q)) = block_at_mut(&mut sections, path) {
                    q.runs = para::rebuild(a);
                }
                let pid = id_of.get(path).copied().unwrap_or(0);
                bm_page.insert(name.clone(), page(pid));
                name
            };
            let pg = bm_page.get(&name).copied().unwrap_or(0);
            targets.push(refs::Target {
                level: level.map(|l| l + 1),
                caption,
                text,
                name,
                page: pg,
            });
        }
        // Index entries (§17.16.5.72 XE) and citations (§17.16.5.62 TA) with their pages.
        let marks = refs::collect_marks(doc, &in_toc);
        let spans = refs::bookmark_spans(doc);
        // Regenerate each field (reverse order keeps earlier indices valid).
        let mut tocs = tocs.clone();
        tocs.sort_by_key(|(si, t)| std::cmp::Reverse((*si, t.first)));
        for (si, t) in tocs {
            let Some(Block::Paragraph(first)) = sections[si].get(t.first).cloned() else {
                continue;
            };
            let layout = refs::EntryLayout {
                width: text_width(doc, si),
                leader: refs::leader_of(&first),
            };
            let entries: Vec<Paragraph> = match field_kind(&t.instr).as_str() {
                "INDEX" => refs::index(
                    &t.instr,
                    &marks,
                    &spans,
                    &page,
                    layout,
                    env.vi,
                    &mut styles_needed,
                ),
                "TOA" => refs::authorities(
                    &t.instr,
                    &marks,
                    &spans,
                    &page,
                    layout,
                    env.vi,
                    &mut styles_needed,
                ),
                // Without a sources part the result a file came with stays.
                "BIBLIOGRAPHY" => match &cite {
                    Some(c) => c.bibliography(&t.instr, &mut styles_needed),
                    None => continue,
                },
                _ => refs::toc(&t.instr, &targets, layout, &mut styles_needed),
            };
            if splice_block_field(&mut sections[si], &t, entries) {
                count += 1;
            }
        }
    }

    // ---- 2. SEQ numbers in document order (§17.16.5.56).
    let mut seq: HashMap<String, i64> = HashMap::new();
    visit_fields(&mut sections, &mut |instr, cached| {
        if field_kind(instr) != "SEQ" {
            return;
        }
        let args = field_args(instr);
        let Some(id) = args.get(1).cloned() else {
            return;
        };
        let n = seq.entry(id.to_ascii_lowercase()).or_insert(0);
        if let Some(i) = args.iter().position(|a| a.eq_ignore_ascii_case("\\r")) {
            *n = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1);
        } else if !args.iter().any(|a| a.eq_ignore_ascii_case("\\c")) {
            *n += 1;
        }
        let hidden = args.iter().any(|a| a.eq_ignore_ascii_case("\\h"));
        let v = if hidden {
            String::new()
        } else {
            format_seq(*n, instr)
        };
        if *cached != v {
            *cached = v;
            count += 1;
        }
    });

    // ---- 3. Document information, dates, user, variables, IF, `=` … (§17.16.5).
    let (n, set_vars) = evaluate_all(doc, &mut sections, env);
    count += n;

    // ---- 4. REF / PAGEREF (§17.16.5.51, §17.16.5.45) against the updated text.
    let probe = Document {
        sections: doc
            .sections
            .iter()
            .zip(&sections)
            .map(|(s, b)| bo_model::Section {
                blocks: b.clone(),
                ..s.clone()
            })
            .collect(),
        styles: doc.styles.clone(),
        ..Document::default()
    };
    let bm_text: HashMap<String, String> = bookmarks(&probe)
        .into_iter()
        .map(|b| (b.name, b.text.trim().to_owned()))
        .collect();
    visit_fields(&mut sections, &mut |instr, cached| {
        let kind = field_kind(instr);
        let args = field_args(instr);
        let Some(name) = args.get(1) else {
            return;
        };
        let v = match kind.as_str() {
            // A SET field defines a bookmark too (§17.16.5.57).
            "REF" => bm_text
                .get(name)
                .cloned()
                .or_else(|| set_vars.get(&name.to_ascii_lowercase()).cloned()),
            "PAGEREF" => bm_page
                .get(name)
                .filter(|p| **p > 0)
                .map(ToString::to_string),
            _ => None,
        };
        if let Some(v) = v {
            if *cached != v {
                *cached = v;
                count += 1;
            }
        }
    });

    let styles = if styles_needed.is_empty() {
        None
    } else {
        ensure_styles(&doc.styles, &styles_needed)
    };
    FieldUpdate {
        sections,
        styles,
        count,
    }
}

// ---------------------------------------------------------------------------
// Other fields (§17.16.5): document information, dates, user, variables, IF, `=`, …
// ---------------------------------------------------------------------------

/// What fields read from outside the document text: the clock, document properties (core and
/// extended properties, lower-case names: `title`, `subject`, `author`, `keywords`, `comments`,
/// `lastsavedby`, `template`, `revnum`, `edittime`, `createdate`, `savedate`, `printdate`,
/// `filename`, `filepath`, `filesize`, `company`, …), the user and the document variables.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldEnv {
    /// Now (DATE, TIME).
    pub now: Option<bo_model::field::DateTime>,
    /// Vietnamese month / day names and numbers in words (else English).
    pub vi: bool,
    /// Document properties by lower-case name.
    pub props: std::collections::BTreeMap<String, String>,
    /// USERNAME, USERINITIALS, USERADDRESS.
    pub user_name: String,
    /// User initials.
    pub user_initials: String,
    /// User address.
    pub user_address: String,
    /// §17.15.1.17 docVars (DOCVARIABLE).
    pub vars: std::collections::BTreeMap<String, String>,
}

/// Field kinds whose result this module computes (the others keep their cached result: links,
/// included files, databases, bibliographies, indexes, form fields, mail merge).
const COMPUTED: &[&str] = &[
    "DATE",
    "TIME",
    "CREATEDATE",
    "SAVEDATE",
    "PRINTDATE",
    "AUTHOR",
    "TITLE",
    "SUBJECT",
    "KEYWORDS",
    "COMMENTS",
    "LASTSAVEDBY",
    "TEMPLATE",
    "FILENAME",
    "FILESIZE",
    "REVNUM",
    "EDITTIME",
    "NUMWORDS",
    "NUMCHARS",
    "USERNAME",
    "USERINITIALS",
    "USERADDRESS",
    "DOCPROPERTY",
    "DOCVARIABLE",
    "QUOTE",
    "SYMBOL",
    "SET",
    "ASK",
    "FILLIN",
    "IF",
    "=",
    "SECTION",
    "STYLEREF",
    "COMPARE",
    "MACROBUTTON",
    "GOTOBUTTON",
    "AUTONUM",
    "AUTONUMLGL",
    "AUTONUMOUT",
    "LISTNUM",
    "PRINT",
    "PRIVATE",
    "ADVANCE",
    "TC",
    "TA",
    "XE",
    "RD",
    "INFO",
];

/// Numbers in a cell's text (a formula reads its first number).
fn cell_number(blocks: &[Block]) -> Option<f64> {
    let mut t = String::new();
    for b in blocks {
        if let Block::Paragraph(p) = b {
            t.push_str(&p.plain_text());
        }
    }
    let s: String = t
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-'))
        .collect();
    // 1.234,5 (Vietnamese) and 1,234.5 (English) both read.
    let s = if s.contains(',') && s.contains('.') {
        if s.rfind(',') > s.rfind('.') {
            s.replace('.', "").replace(',', ".")
        } else {
            s.replace(',', "")
        }
    } else if s.matches(',').count() == 1 && s.split(',').nth(1).is_some_and(|d| d.len() != 3) {
        s.replace(',', ".")
    } else {
        s.replace(',', "")
    };
    s.parse().ok()
}

/// A formula's view of the document: bookmarks and SET values, and the table around the field.
struct FormulaCtx<'a> {
    vars: &'a HashMap<String, String>,
    table: TableAt<'a>,
}

impl bo_model::field::FormulaEnv for FormulaCtx<'_> {
    fn bookmark(&self, name: &str) -> Option<f64> {
        let v = self.vars.get(&name.to_ascii_lowercase())?;
        let t: String = v.chars().filter(|c| !c.is_whitespace()).collect();
        t.replace(',', ".").parse().ok()
    }

    fn cells(&self, r: &str) -> Option<Vec<f64>> {
        let (t, row, col) = self.table?;
        let at = |ri: usize, ci: usize| -> Option<f64> {
            t.rows
                .get(ri)?
                .cells
                .get(ci)
                .and_then(|c| cell_number(&c.blocks))
        };
        let up = r.to_ascii_uppercase();
        let v: Vec<f64> = match up.as_str() {
            "ABOVE" => (0..row).filter_map(|ri| at(ri, col)).collect(),
            "BELOW" => (row + 1..t.rows.len())
                .filter_map(|ri| at(ri, col))
                .collect(),
            "LEFT" => (0..col).filter_map(|ci| at(row, ci)).collect(),
            "RIGHT" => (col + 1..t.rows.get(row).map_or(0, |x| x.cells.len()))
                .filter_map(|ci| at(row, ci))
                .collect(),
            _ => {
                let (a, b) = up.split_once(':').unwrap_or((&up, &up));
                let ((c0, r0), (c1, r1)) =
                    (bo_model::field::cell_ref(a)?, bo_model::field::cell_ref(b)?);
                let mut v = Vec::new();
                for ri in r0.min(r1)..=r0.max(r1) {
                    for ci in c0.min(c1)..=c0.max(c1) {
                        v.extend(at(ri, ci));
                    }
                }
                v
            }
        };
        Some(v)
    }
}

/// `expr1 op expr2` of IF / COMPARE: numbers compare as numbers, text as text (`?` and `*`
/// wildcards on the right with `=` / `<>`).
pub(crate) fn compare_values(a: &str, op: &str, b: &str) -> bool {
    let na = a.trim().replace(',', ".").parse::<f64>();
    let nb = b.trim().replace(',', ".").parse::<f64>();
    if let (Ok(x), Ok(y)) = (na, nb) {
        return match op {
            "=" => (x - y).abs() < 1e-12,
            "<>" => (x - y).abs() >= 1e-12,
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            ">=" => x >= y,
            _ => false,
        };
    }
    let wild = |pat: &str, s: &str| -> bool {
        fn m(p: &[char], s: &[char]) -> bool {
            match (p.first(), s.first()) {
                (None, None) => true,
                (Some('*'), _) => m(&p[1..], s) || (!s.is_empty() && m(p, &s[1..])),
                (Some('?'), Some(_)) => m(&p[1..], &s[1..]),
                (Some(x), Some(y)) if x == y => m(&p[1..], &s[1..]),
                _ => false,
            }
        }
        m(
            &pat.chars().collect::<Vec<_>>(),
            &s.chars().collect::<Vec<_>>(),
        )
    };
    match op {
        "=" => wild(b, a),
        "<>" => !wild(b, a),
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        _ => false,
    }
}

/// Split `IF`'s condition `a op b` (the operator may stand alone or be glued: `x>5`).
pub(crate) fn condition(w: &[String]) -> Option<(String, String, String, usize)> {
    const OPS: [&str; 6] = ["<=", ">=", "<>", "=", "<", ">"];
    if w.len() >= 3 && OPS.contains(&w[1].as_str()) {
        return Some((w[0].clone(), w[1].clone(), w[2].clone(), 3));
    }
    let first = w.first()?;
    for op in OPS {
        if let Some((a, b)) = first.split_once(op) {
            if !a.is_empty() && !b.is_empty() {
                return Some((a.into(), op.into(), b.into(), 1));
            }
        }
    }
    None
}

/// Plain words of the body (NUMWORDS) and characters without spaces (NUMCHARS).
fn counts(doc: &Document) -> (usize, usize) {
    let mut words = 0;
    let mut chars = 0;
    for path in body_paragraphs(doc) {
        if let Some(p) = para_at(doc, &path) {
            let t = p.plain_text();
            words += t.split_whitespace().count();
            chars += t.chars().filter(|c| !c.is_whitespace()).count();
        }
    }
    (words, chars)
}

/// Where a field sits: the table around it (with its row and column), if any.
type TableAt<'a> = Option<(&'a bo_model::Table, usize, usize)>;

/// Every paragraph of `blocks`, with the table cell it is in.
fn walk(blocks: &mut [Block], table: TableAt<'_>, f: &mut dyn FnMut(&mut Paragraph, TableAt<'_>)) {
    for b in blocks.iter_mut() {
        match b {
            Block::Paragraph(p) => f(p, table),
            Block::Table(t) => {
                let snapshot = t.clone();
                for (ri, row) in t.rows.iter_mut().enumerate() {
                    for (ci, cell) in row.cells.iter_mut().enumerate() {
                        walk(&mut cell.blocks, Some((&snapshot, ri, ci)), f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Evaluate the computed fields of every story of the body, in document order.
#[allow(clippy::too_many_lines)]
fn evaluate_all(
    doc: &Document,
    sections: &mut [Vec<Block>],
    env: &FieldEnv,
) -> (usize, HashMap<String, String>) {
    use bo_model::field::{format, has_switch, switch_values, words, DateTime, FieldValue};
    let mut count = 0;
    let mut vars: HashMap<String, String> = bookmarks(doc)
        .into_iter()
        .map(|b| (b.name.to_ascii_lowercase(), b.text.trim().to_owned()))
        .collect();
    let (nwords, nchars) = counts(doc);
    let prop = |k: &str| env.props.get(k).cloned().unwrap_or_default();
    let date_prop = |k: &str| env.props.get(k).and_then(|v| DateTime::parse_iso(v));
    let mut autonum = 0i64;
    // The last paragraph text of each style seen (STYLEREF).
    let mut last_style: HashMap<String, String> = HashMap::new();
    let style_name = |id: &str| {
        doc.styles
            .styles
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| id.to_owned(), |s| s.name.clone())
    };

    for (si, blocks) in sections.iter_mut().enumerate() {
        walk(blocks, None, &mut |p, table| {
            if let Some(st) = &p.props.style {
                last_style.insert(style_name(st).to_ascii_lowercase(), p.plain_text());
                last_style.insert(st.to_ascii_lowercase(), p.plain_text());
            }
            let mut changed = false;
            for r in &mut p.runs {
                for c in &mut r.content {
                    // A locked field keeps its result (§17.16.18 fldLock).
                    let RunContent::Field {
                        instr,
                        cached,
                        locked: false,
                    } = c
                    else {
                        continue;
                    };
                    let w = words(instr);
                    let kind = w
                        .first()
                        .map(|k| k.to_ascii_uppercase())
                        .unwrap_or_default();
                    let kind = if kind.starts_with('=') {
                        "=".to_owned()
                    } else {
                        kind
                    };
                    if !COMPUTED.contains(&kind.as_str()) {
                        continue;
                    }
                    let arg = |i: usize| w.get(i).filter(|a| !a.starts_with('\\')).cloned();
                    let text = |s: String| Some(FieldValue::Text(s));
                    let value: Option<FieldValue> = match kind.as_str() {
                        "DATE" | "TIME" => env.now.map(FieldValue::Date),
                        "CREATEDATE" => date_prop("createdate").map(FieldValue::Date),
                        "SAVEDATE" => date_prop("savedate").map(FieldValue::Date),
                        "PRINTDATE" => date_prop("printdate").map(FieldValue::Date),
                        "AUTHOR" => arg(1).or_else(|| Some(prop("author"))).and_then(text),
                        "TITLE" => arg(1).or_else(|| Some(prop("title"))).and_then(text),
                        "SUBJECT" => arg(1).or_else(|| Some(prop("subject"))).and_then(text),
                        "KEYWORDS" => arg(1).or_else(|| Some(prop("keywords"))).and_then(text),
                        "COMMENTS" => arg(1).or_else(|| Some(prop("comments"))).and_then(text),
                        "LASTSAVEDBY" => text(prop("lastsavedby")),
                        "TEMPLATE" => text(prop("template")),
                        "FILENAME" => text(if has_switch(instr, "\\p") {
                            prop("filepath")
                        } else {
                            prop("filename")
                        }),
                        "FILESIZE" => env
                            .props
                            .get("filesize")
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|b| {
                                FieldValue::Number(if has_switch(instr, "\\m") {
                                    (b / 1_048_576.0).round()
                                } else if has_switch(instr, "\\k") {
                                    (b / 1024.0).round()
                                } else {
                                    b
                                })
                            }),
                        "REVNUM" => env
                            .props
                            .get("revnum")
                            .and_then(|v| v.parse().ok())
                            .map(FieldValue::Number),
                        "EDITTIME" => env
                            .props
                            .get("edittime")
                            .and_then(|v| v.parse().ok())
                            .map(FieldValue::Number),
                        "NUMWORDS" => Some(FieldValue::Number(nwords as f64)),
                        "NUMCHARS" => Some(FieldValue::Number(nchars as f64)),
                        "USERNAME" => arg(1)
                            .or_else(|| Some(env.user_name.clone()))
                            .and_then(text),
                        "USERINITIALS" => arg(1)
                            .or_else(|| {
                                Some(if env.user_initials.is_empty() {
                                    env.user_name
                                        .split_whitespace()
                                        .filter_map(|x| x.chars().next())
                                        .collect()
                                } else {
                                    env.user_initials.clone()
                                })
                            })
                            .and_then(text),
                        "USERADDRESS" => arg(1)
                            .or_else(|| Some(env.user_address.clone()))
                            .and_then(text),
                        "DOCPROPERTY" | "INFO" => arg(1)
                            .and_then(|n| {
                                let k = n.to_ascii_lowercase().replace(' ', "");
                                env.props
                                    .get(&k)
                                    .cloned()
                                    .or_else(|| env.vars.get(&n).cloned())
                            })
                            .and_then(text),
                        "DOCVARIABLE" => arg(1)
                            .and_then(|n| env.vars.get(&n).cloned())
                            .and_then(text),
                        "QUOTE" => text(
                            w[1..]
                                .iter()
                                .filter(|a| !a.starts_with('\\'))
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(" "),
                        ),
                        "SYMBOL" => arg(1)
                            .and_then(|n| {
                                let code = n
                                    .strip_prefix("0x")
                                    .or_else(|| n.strip_prefix("0X"))
                                    .map_or_else(
                                        || n.parse::<u32>().ok(),
                                        |h| u32::from_str_radix(h, 16).ok(),
                                    )?;
                                char::from_u32(code).map(|c| c.to_string())
                            })
                            .and_then(text),
                        "SET" => {
                            if let (Some(n), Some(v)) = (arg(1), arg(2)) {
                                vars.insert(n.to_ascii_lowercase(), v);
                            }
                            text(String::new())
                        }
                        "ASK" | "FILLIN" => {
                            // No prompt while updating: the default answer (\d).
                            let d = switch_values(instr, "\\d").first().cloned();
                            if kind == "ASK" {
                                if let (Some(n), Some(v)) = (arg(1), d.clone()) {
                                    vars.insert(n.to_ascii_lowercase(), v);
                                }
                                text(String::new())
                            } else {
                                d.and_then(text)
                            }
                        }
                        "IF" | "COMPARE" => {
                            // Bookmark names in the operands read their value.
                            let rest: Vec<String> = w[1..]
                                .iter()
                                .take_while(|a| !a.starts_with('\\'))
                                .map(|a| {
                                    vars.get(&a.to_ascii_lowercase())
                                        .cloned()
                                        .unwrap_or_else(|| a.clone())
                                })
                                .collect();
                            condition(&rest).map(|(a, op, b, used)| {
                                let ok = compare_values(&a, &op, &b);
                                if kind == "COMPARE" {
                                    FieldValue::Number(f64::from(u8::from(ok)))
                                } else {
                                    let pick = if ok {
                                        rest.get(used)
                                    } else {
                                        rest.get(used + 1)
                                    };
                                    FieldValue::Text(pick.cloned().unwrap_or_default())
                                }
                            })
                        }
                        "=" => {
                            let expr: String = instr
                                .trim_start()
                                .trim_start_matches('=')
                                .split(" \\")
                                .next()
                                .unwrap_or("")
                                .to_owned();
                            let ctx = FormulaCtx { vars: &vars, table };
                            bo_model::field::formula(&expr, &ctx).map(FieldValue::Number)
                        }
                        "SECTION" => Some(FieldValue::Number((si + 1) as f64)),
                        "STYLEREF" => arg(1)
                            .and_then(|s| last_style.get(&s.to_ascii_lowercase()).cloned())
                            .and_then(text),
                        "MACROBUTTON" | "GOTOBUTTON" => {
                            text(w.get(2..).unwrap_or_default().join(" "))
                        }
                        "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT" | "LISTNUM" => {
                            autonum += 1;
                            Some(FieldValue::Text(if kind == "AUTONUM" {
                                format!("{autonum}.")
                            } else {
                                autonum.to_string()
                            }))
                        }
                        // Fields that show nothing (§17.16.5: entries, markers, printer codes).
                        _ => text(String::new()),
                    };
                    let Some(v) = value else {
                        continue;
                    };
                    // DATE / TIME without a picture: the default short forms.
                    let mut ins = instr.clone();
                    if matches!(v, FieldValue::Date(_)) && switch_values(instr, "\\@").is_empty() {
                        let pic = match (kind.as_str(), env.vi) {
                            ("TIME", true) => "HH:mm",
                            ("TIME", false) => "h:mm am/pm",
                            (_, true) => "dd/MM/yyyy",
                            (_, false) => "M/d/yyyy",
                        };
                        ins = format!("{ins} \\@ \"{pic}\"");
                    }
                    let out = format(&v, &ins, env.vi);
                    if *cached != out {
                        *cached = out;
                        changed = true;
                        count += 1;
                    }
                }
            }
            if changed {
                p.src = None;
            }
        });
    }
    (count, vars)
}

/// Mutable block at a body paragraph path inside per-section block lists.
fn block_at_mut<'a>(sections: &'a mut [Vec<Block>], path: &[usize]) -> Option<&'a mut Block> {
    let (&si, rest) = path.split_first()?;
    let mut blocks = sections.get_mut(si)?;
    let mut rest = rest;
    loop {
        match rest {
            [bi] => return blocks.get_mut(*bi),
            [bi, ri, ci, tail @ ..] => {
                let Block::Table(t) = blocks.get_mut(*bi)? else {
                    return None;
                };
                blocks = &mut t.rows.get_mut(*ri)?.cells.get_mut(*ci)?.blocks;
                rest = tail;
            }
            _ => return None,
        }
    }
}

/// Visit every simple field result of the main text in document order.
pub(crate) fn visit_fields(sections: &mut [Vec<Block>], f: &mut dyn FnMut(&str, &mut String)) {
    fn blocks(v: &mut [Block], f: &mut dyn FnMut(&str, &mut String)) {
        for b in v {
            match b {
                Block::Paragraph(p) => {
                    for r in &mut p.runs {
                        for c in &mut r.content {
                            // Locked fields keep their result (§17.16.18 fldLock).
                            if let RunContent::Field {
                                instr,
                                cached,
                                locked: false,
                            } = c
                            {
                                f(instr, cached);
                            }
                        }
                    }
                }
                Block::Table(t) => {
                    for row in &mut t.rows {
                        for c in &mut row.cells {
                            blocks(&mut c.blocks, f);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    for s in sections {
        blocks(s, f);
    }
}

/// Number of the next `SEQ label` field before body paragraph `before` (caption numbering).
pub fn next_seq(doc: &Document, label: &str, before: usize) -> i64 {
    let paths = body_paragraphs(doc);
    let mut n = 0;
    for path in paths.iter().take(before + 1) {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        for r in &p.runs {
            for c in &r.content {
                if let RunContent::Field { instr, .. } = c {
                    let args = field_args(instr);
                    if field_kind(instr) == "SEQ"
                        && args.get(1).is_some_and(|a| a.eq_ignore_ascii_case(label))
                    {
                        n += 1;
                    }
                }
            }
        }
    }
    n + 1
}

/// Caption paragraph: `label n[: text]` with a `SEQ label` field (§17.16.5.56).
pub fn caption_paragraph(label: &str, n: i64, text: &str) -> Paragraph {
    let mut runs = vec![
        Run::text(format!("{label} "), RunProps::default()),
        Run {
            props: RunProps::default(),
            content: vec![RunContent::Field {
                instr: format!("SEQ {label} \\* ARABIC"),
                cached: n.to_string(),
                locked: false,
            }],
        },
    ];
    if !text.trim().is_empty() {
        runs.push(Run::text(format!(": {}", text.trim()), RunProps::default()));
    }
    Paragraph::new(
        ParaProps {
            style: Some("Caption".into()),
            align: Some(Alignment::Center),
            ..ParaProps::default()
        },
        runs,
    )
}

/// A cross-reference target for the UI.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefTarget {
    /// Bookmark name, or `caption:<paragraph id>` for a caption without bookmark.
    pub id: String,
    /// Text shown in the cross-reference dialog.
    pub label: String,
}

/// Bookmarks (user-visible ones) and captions that can be cross-referenced.
pub fn ref_targets(doc: &Document) -> Vec<RefTarget> {
    let mut out: Vec<RefTarget> = bookmarks(doc)
        .into_iter()
        .filter(|b| !b.name.starts_with('_') || b.name.starts_with("_Ref"))
        .map(|b| RefTarget {
            label: if b.text.is_empty() {
                b.name.clone()
            } else {
                b.text.clone()
            },
            id: b.name,
        })
        .collect();
    for (pid, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let has_seq = p.runs.iter().any(|r| {
            r.content
                .iter()
                .any(|c| matches!(c, RunContent::Field { instr, .. } if field_kind(instr) == "SEQ"))
        });
        let atoms = para::flatten(p);
        let bookmarked = bookmark_marks(&atoms)
            .iter()
            .any(|(_, n, _)| n.as_ref().is_some_and(|n| n.starts_with("_Ref")));
        if has_seq && !bookmarked {
            out.push(RefTarget {
                id: format!("caption:{pid}"),
                label: display_text(&atoms),
            });
        }
    }
    out
}

/// Wrap the label and number of a caption paragraph (up to its SEQ field) in a new
/// `_Ref` bookmark; returns the new atoms and the bookmark name.
pub(crate) fn bookmark_caption(doc: &Document, p: &Paragraph, salt: usize) -> (Vec<Atom>, String) {
    let mut atoms = para::flatten(p);
    let end = atoms
        .iter()
        .position(
            |(_, c)| matches!(c, RunContent::Field { instr, .. } if field_kind(instr) == "SEQ"),
        )
        .map_or(atoms.len(), |i| i + 1);
    let names: Vec<String> = bookmarks(doc).into_iter().map(|b| b.name).collect();
    let mut h = fingerprint(&(salt, display_text(&atoms))) % 1_000_000_000;
    let mut name = format!("_Ref{h:09}");
    while names.contains(&name) {
        h = (h + 1) % 1_000_000_000;
        name = format!("_Ref{h:09}");
    }
    let id = max_bookmark_id(doc) + 1;
    let (s, e) = bookmark_xml(id, &name);
    atoms.insert(end, (RunProps::default(), RunContent::Markup(e)));
    atoms.insert(0, (RunProps::default(), RunContent::Markup(s)));
    (atoms, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_and_formats() {
        assert_eq!(
            field_args(r#"TOC \o "1-3" \h \z \u"#),
            vec!["TOC", "\\o", "1-3", "\\h", "\\z", "\\u"]
        );
        assert_eq!(toc_levels(r#"TOC \o "2-4""#), (2, 4));
        assert_eq!(toc_levels("TOC"), (1, 9));
        assert_eq!(format_seq(4, "SEQ Hình \\* ROMAN"), "IV");
        assert_eq!(format_seq(3, "SEQ Bảng \\* alphabetic"), "c");
        assert_eq!(format_seq(12, "SEQ Bảng"), "12");
        assert_eq!(field_kind(" pageref _Toc1 \\h"), "PAGEREF");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod eval_tests {
    use super::*;
    use bo_model::{Cell, Row, Section, Table};

    fn field(instr: &str) -> Paragraph {
        Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![RunContent::Field {
                    instr: instr.into(),
                    cached: "?".into(),
                    locked: false,
                }],
            }],
        )
    }

    fn result(p: &Block) -> String {
        let Block::Paragraph(p) = p else {
            return String::new();
        };
        p.runs
            .iter()
            .flat_map(|r| &r.content)
            .find_map(|c| match c {
                RunContent::Field { cached, .. } => Some(cached.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn env() -> FieldEnv {
        let mut props = std::collections::BTreeMap::new();
        for (k, v) in [
            ("title", "hợp đồng mua bán"),
            ("subject", "Mua bán thiết bị"),
            ("author", "Nguyễn Văn A"),
            ("keywords", "hợp đồng, thiết bị"),
            ("comments", "Bản nháp"),
            ("lastsavedby", "Trần Thị B"),
            ("template", "Normal.dotm"),
            ("revnum", "7"),
            ("edittime", "125"),
            ("createdate", "2026-01-15T08:30:00Z"),
            ("savedate", "2026-11-08T17:45:00Z"),
            ("printdate", "2026-11-09T09:00:00Z"),
            ("filename", "hop-dong.docx"),
            ("filepath", "D:\\Ho so\\hop-dong.docx"),
            ("filesize", "245760"),
            ("company", "Bluestar"),
        ] {
            props.insert(k.into(), v.into());
        }
        let mut vars = std::collections::BTreeMap::new();
        vars.insert("SoHopDong".into(), "HD-2026/15".into());
        FieldEnv {
            now: bo_model::field::DateTime::parse_iso("2026-11-09T14:05:00Z"),
            vi: true,
            props,
            user_name: "Lê Văn Cường".into(),
            user_initials: String::new(),
            user_address: "Hà Nội".into(),
            vars,
        }
    }

    // source: ECMA-376 Part 1, §17.16.5 — one row per field: instruction → expected result
    // (values from the environment above; formats per §17.16.4).
    #[test]
    fn every_field_kind() {
        let rows: &[(&str, &str)] = &[
            ("DATE", "09/11/2026"),
            ("DATE \\@ \"dddd, d MMMM yyyy\"", "Thứ Hai, 9 Tháng 11 2026"),
            ("TIME \\@ \"HH:mm\"", "14:05"),
            ("CREATEDATE \\@ \"dd/MM/yyyy\"", "15/01/2026"),
            ("SAVEDATE \\@ \"d/M/yy HH:mm\"", "8/11/26 17:45"),
            ("PRINTDATE \\@ \"yyyy-MM-dd\"", "2026-11-09"),
            ("AUTHOR", "Nguyễn Văn A"),
            ("TITLE \\* Caps", "Hợp Đồng Mua Bán"),
            ("SUBJECT \\* Upper", "MUA BÁN THIẾT BỊ"),
            ("KEYWORDS", "hợp đồng, thiết bị"),
            ("COMMENTS", "Bản nháp"),
            ("LASTSAVEDBY", "Trần Thị B"),
            ("TEMPLATE", "Normal.dotm"),
            ("FILENAME", "hop-dong.docx"),
            ("FILENAME \\p", "D:\\Ho so\\hop-dong.docx"),
            ("FILESIZE \\k", "240"),
            ("REVNUM", "7"),
            ("EDITTIME", "125"),
            ("USERNAME", "Lê Văn Cường"),
            ("USERINITIALS", "LVC"),
            ("USERADDRESS", "Hà Nội"),
            ("DOCPROPERTY Company", "Bluestar"),
            ("DOCVARIABLE SoHopDong", "HD-2026/15"),
            ("QUOTE \"Trích dẫn\"", "Trích dẫn"),
            ("SYMBOL 169", "©"),
            ("SYMBOL 0x2192", "→"),
            ("SET Gia 1500", ""),
            ("= Gia * 2 \\# \"#,##0\"", "3,000"),
            ("REF Gia", "1500"),
            ("IF Gia > 1000 \"đắt\" \"rẻ\"", "đắt"),
            ("IF \"Hà Nội\" = \"Hà*\" \"có\" \"không\"", "có"),
            ("COMPARE 3 <= 2", "0"),
            ("= 12.5 * 4 \\* CardText", "năm mươi"),
            ("= 5 \\* ROMAN", "V"),
            ("= 1234567.891 \\# \"#,##0.00\"", "1,234,567.89"),
            ("ASK Ten \"Tên?\" \\d \"Lan\"", ""),
            ("FILLIN \"Ghi chú?\" \\d \"Không có\"", "Không có"),
            ("SECTION", "1"),
            ("MACROBUTTON NoMacro Bấm vào đây", "Bấm vào đây"),
            ("AUTONUM", "1."),
            ("TC \"Mục lục\" \\l 1", ""),
            ("XE \"Thuật ngữ\"", ""),
            ("PRINT \"x\"", ""),
            ("INCLUDETEXT \"a.docx\"", "?"),
        ];
        let mut blocks: Vec<Block> = rows
            .iter()
            .map(|(i, _)| Block::Paragraph(field(i)))
            .collect();
        // A table with a formula under three numbers.
        let cell = |t: &str| Cell {
            blocks: vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run::text(t, RunProps::default())],
            ))],
            ..Cell::default()
        };
        let fcell = |i: &str| Cell {
            blocks: vec![Block::Paragraph(field(i))],
            ..Cell::default()
        };
        blocks.push(Block::Table(Table {
            rows: vec![
                Row {
                    cells: vec![cell("10"), cell("1,5")],
                    ..Row::default()
                },
                Row {
                    cells: vec![cell("20"), cell("2,5")],
                    ..Row::default()
                },
                Row {
                    cells: vec![cell("30"), fcell("= SUM(LEFT)")],
                    ..Row::default()
                },
                Row {
                    cells: vec![fcell("= SUM(ABOVE)"), fcell("= AVERAGE(B1:B2) \\# \"0.0\"")],
                    ..Row::default()
                },
            ],
            ..Table::default()
        }));
        let doc = Document {
            sections: vec![Section {
                blocks,
                ..Section::default()
            }],
            ..Document::default()
        };
        let up = update(&doc, &[], &env());
        let out = &up.sections[0];
        for (k, (instr, want)) in rows.iter().enumerate() {
            assert_eq!(result(&out[k]), *want, "{instr}");
        }
        let Block::Table(t) = out.last().unwrap() else {
            panic!()
        };
        // NUMWORDS counts the body as it was: one "?" per field paragraph, five numbers and
        // three "?" in the table.
        let mut d2 = doc.clone();
        d2.sections[0]
            .blocks
            .insert(0, Block::Paragraph(field("NUMWORDS")));
        let up2 = update(&d2, &[], &env());
        assert_eq!(
            result(&up2.sections[0][0]),
            (rows.len() + 1 + 8).to_string()
        );
        let r = |ri: usize, ci: usize| result(&t.rows[ri].cells[ci].blocks[0]);
        assert_eq!(r(2, 1), "30");
        assert_eq!(r(3, 0), "60");
        assert_eq!(r(3, 1), "2.0");
    }
}

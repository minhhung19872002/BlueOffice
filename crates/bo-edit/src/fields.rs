//! Field updating (ECMA-376 Part 1, §17.16): tables of contents (§17.16.5.68 TOC),
//! caption numbers (§17.16.5.56 SEQ), cross-references (§17.16.5.49 REF,
//! §17.16.5.42 PAGEREF) and mail-merge fields (§17.16.5.35 MERGEFIELD).
//!
//! Page numbers come from the layout: callers pass the displayed page number of
//! every body paragraph (by paragraph id, see `bo_model::index`).

use crate::para::{self, Atom};
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
enum Mark {
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

fn mark_of(c: &RunContent) -> Option<Mark> {
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

/// A TOC field spanning top-level paragraphs `first..=last` of a section.
#[derive(Debug, Clone)]
struct TocRange {
    first: usize,
    last: usize,
    /// Atom index of `begin` in `first` and of `end` in `last`.
    begin: usize,
    end: usize,
    /// Atom index of `separate` in `first` (head = begin..=separate) when there.
    separate: Option<usize>,
    instr: String,
}

/// TOC fields among the top-level paragraphs of a block list.
fn toc_ranges(blocks: &[Block]) -> Vec<TocRange> {
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
                        if stack.is_empty() && field_kind(&o.instr) == "TOC" {
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
fn toc_levels(instr: &str) -> (u8, u8) {
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
fn display_text(atoms: &[Atom]) -> String {
    let mut s = String::new();
    for (props, item) in atoms {
        if props.del.is_some() {
            continue;
        }
        match item {
            RunContent::Text(t) => s.push_str(t),
            RunContent::Field { cached, .. } => s.push_str(cached),
            RunContent::Tab | RunContent::LineBreak => s.push(' '),
            RunContent::NoBreakHyphen => s.push('-'),
            _ => {}
        }
    }
    s
}

/// Bookmark marks of a paragraph: (atom index, name or "" for an end, id).
fn bookmark_marks(atoms: &[Atom]) -> Vec<(usize, Option<String>, String)> {
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
fn max_bookmark_id(doc: &Document) -> i64 {
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

fn bookmark_xml(id: i64, name: &str) -> (Raw, Raw) {
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
fn text_width(doc: &Document, section: usize) -> i32 {
    doc.sections.get(section).map_or(9355, |s| {
        (s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0).max(1440)
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

/// Recompute TOC, SEQ, REF, PAGEREF fields. `pages[id]` = displayed page number of
/// body paragraph `id` (0 = unknown).
pub fn update(doc: &Document, pages: &[u32]) -> FieldUpdate {
    let page = |id: usize| pages.get(id).copied().unwrap_or(0);
    let mut sections: Vec<Vec<Block>> = doc.sections.iter().map(|s| s.blocks.clone()).collect();
    let mut count = 0;
    let mut styles_needed: Vec<String> = Vec::new();
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

    // ---- 1. Tables of contents.
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
        // Headings with their `_Toc` bookmark (added when missing).
        let mut next_bm = max_bookmark_id(doc);
        let mut headings: Vec<(u8, String, String, u32)> = Vec::new(); // level, text, bookmark, page
        let mut used_names: Vec<String> = bm_page.keys().cloned().collect();
        for (ordinal, path) in paths.iter().enumerate() {
            if in_toc(path) {
                continue;
            }
            let Some(p) = para_at(doc, path) else {
                continue;
            };
            let Some(level) = outline_level(doc, p) else {
                continue;
            };
            let atoms = para::flatten(p);
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
            headings.push((level + 1, text, name, pg));
        }
        // Regenerate each TOC (reverse order keeps earlier indices valid).
        let mut tocs = tocs;
        tocs.sort_by_key(|(si, t)| std::cmp::Reverse((*si, t.first)));
        for (si, t) in tocs {
            let blocks = &mut sections[si];
            let (Some(Block::Paragraph(first)), Some(Block::Paragraph(last))) =
                (blocks.get(t.first).cloned(), blocks.get(t.last).cloned())
            else {
                continue;
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
            let (lo, hi) = toc_levels(&t.instr);
            let link = field_args(&t.instr)
                .iter()
                .any(|a| a.eq_ignore_ascii_case("\\h"));
            let width = text_width(doc, si);
            let mut entries: Vec<Paragraph> = Vec::new();
            for (level, text, name, pg) in headings.iter().filter(|h| (lo..=hi).contains(&h.0)) {
                let style = format!("TOC{level}");
                if !styles_needed.contains(&style) {
                    styles_needed.push(style.clone());
                }
                let props = RunProps {
                    link: link.then(|| format!("#{name}")),
                    ..RunProps::default()
                };
                let pg_text = if *pg > 0 {
                    pg.to_string()
                } else {
                    String::new()
                };
                entries.push(Paragraph::new(
                    ParaProps {
                        style: Some(style),
                        tabs: Some(vec![TabStop {
                            pos: Twips(width),
                            align: TabAlign::Right,
                            leader: TabLeader::Dot,
                        }]),
                        ..ParaProps::default()
                    },
                    vec![
                        Run::text(text.clone(), props.clone()),
                        Run {
                            props: props.clone(),
                            content: vec![
                                RunContent::Tab,
                                RunContent::Field {
                                    instr: format!("PAGEREF {name} \\h"),
                                    cached: pg_text,
                                },
                            ],
                        },
                    ],
                ));
            }
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
                count += 1;
                blocks.splice(t.first..=t.last, out);
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

    // ---- 3. REF / PAGEREF (§17.16.5.49, §17.16.5.42) against the updated text.
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
            "REF" => bm_text.get(name).cloned(),
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
                            if let RunContent::Field { instr, cached } = c {
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

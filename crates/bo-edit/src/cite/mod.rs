//! Citations and bibliography (CORE-CITE-001): the document's sources live in its
//! bibliography part (ECMA-376 Part 1, §22.6 `b:Sources`, a custom XML part §15.2.4);
//! citations are `CITATION` fields (§17.16.5.8) and the reference list a `BIBLIOGRAPHY`
//! field (§17.16.5.7) whose results are written by BlueOffice's own style formatters
//! ([`style`]). Every command here is one undo step that changes the sources part and
//! refreshes every citation and bibliography.

pub mod bibtex;
pub mod names;
pub mod ris;
pub mod source;
pub mod style;

#[cfg(test)]
mod style_tests;
#[cfg(test)]
mod tests;

use crate::fields::{self, field_kind};
use crate::{para, para_clone, para_len, path_of, split_path, EditError, Op, Pos, Result};
use crate::{Selection, Session, Tx};
use bo_model::custom_xml::{new_item_id, new_part_names, CustomXmlPart};
use bo_model::{Block, Document, ParaProps, Paragraph, RunContent, RunProps};
use serde::Serialize;
pub use source::{Contributor, Person, Source, SourceList};
pub use style::{Citation, CiteItem, Entry, Formatter, Span, StyleId};

fn err(key: &str) -> EditError {
    EditError::Unsupported(key.to_owned())
}

/// Index of the document's sources part and its content.
#[must_use]
pub fn sources_part(doc: &Document) -> Option<(usize, SourceList)> {
    doc.custom_xml
        .iter()
        .enumerate()
        .find_map(|(i, p)| source::parse_sources(&p.xml).map(|l| (i, l)))
}

/// Visit the instruction of every simple field of `blocks` in document order.
fn each_field(blocks: &[Block], f: &mut dyn FnMut(&str)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for c in p.runs.iter().flat_map(|r| &r.content) {
                    if let RunContent::Field { instr, .. } = c {
                        f(instr);
                    }
                }
            }
            Block::Table(t) => {
                for cell in t.rows.iter().flat_map(|r| &r.cells) {
                    each_field(&cell.blocks, f);
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Tags cited in the body, in order of first appearance.
fn cited_tags(sections: &[Vec<Block>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for blocks in sections {
        each_field(blocks, &mut |instr| {
            if let Some(c) = Citation::parse(instr) {
                for it in c.items {
                    if !out.iter().any(|t| t.eq_ignore_ascii_case(&it.tag)) {
                        out.push(it.tag);
                    }
                }
            }
        });
    }
    out
}

/// The sources, style and citation order of a document: what its citation and
/// bibliography fields are computed from.
pub(crate) struct Context {
    list: SourceList,
    style: StyleId,
    cited: Vec<String>,
}

impl Context {
    /// The context of `doc` with body `sections`; `None` when it has no sources part
    /// (citations a file came with then keep their results).
    pub(crate) fn of(doc: &Document, sections: &[Vec<Block>]) -> Option<Context> {
        let (_, list) = sources_part(doc)?;
        Some(Context {
            style: StyleId::from_name(&list.style_name),
            cited: cited_tags(sections),
            list,
        })
    }

    fn formatter(&self) -> Formatter<'_> {
        Formatter::new(self.style, &self.list.sources, &self.cited)
    }

    /// Recompute the result of every citation; returns how many changed.
    pub(crate) fn update_citations(&self, sections: &mut [Vec<Block>]) -> usize {
        let f = self.formatter();
        let mut n = 0;
        fields::visit_fields(sections, &mut |instr, cached| {
            if let Some(c) = Citation::parse(instr) {
                let v = f.citation(&c);
                if *cached != v {
                    *cached = v;
                    n += 1;
                }
            }
        });
        n
    }

    /// The paragraphs of a bibliography field (`BIBLIOGRAPHY [\l lcid]`, §17.16.5.7).
    pub(crate) fn bibliography(&self, instr: &str, styles: &mut Vec<String>) -> Vec<Paragraph> {
        let lcid = bo_model::field::switch_values(instr, "\\l")
            .into_iter()
            .next();
        let entries = self.formatter().bibliography(lcid.as_deref());
        if !entries.is_empty() && !styles.iter().any(|s| s == "Bibliography") {
            styles.push("Bibliography".into());
        }
        entries.iter().map(entry_paragraph).collect()
    }
}

/// A bibliography entry as a paragraph of style `Bibliography`: the number label of a
/// numeric style, a tab, then the reference with its italic stretches (§17.3.2.16 i).
fn entry_paragraph(e: &Entry) -> Paragraph {
    let plain = RunProps::default();
    let italic = RunProps {
        italic: Some(true),
        ..RunProps::default()
    };
    let mut atoms = Vec::new();
    if let Some(l) = &e.label {
        atoms.extend(para::text_atoms(&format!("{l}\t"), &plain));
    }
    for s in &e.spans {
        atoms.extend(para::text_atoms(
            &s.text,
            if s.italic { &italic } else { &plain },
        ));
    }
    Paragraph::new(
        ParaProps {
            style: Some("Bibliography".into()),
            ..ParaProps::default()
        },
        para::rebuild(atoms),
    )
}

/// A source as the source manager lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    /// The source.
    #[serde(flatten)]
    pub source: Source,
    /// A citation refers to it.
    pub cited: bool,
    /// It is a placeholder (a tag without data).
    pub placeholder: bool,
    /// Its bibliography entry in the current style (empty for a placeholder).
    pub preview: Vec<Span>,
    /// Its citation in the current style.
    pub citation: String,
}

/// The citation state of a document.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiteState {
    /// The selected style.
    pub style: StyleId,
    /// The sources of the document.
    pub sources: Vec<SourceInfo>,
    /// The citation the caret touches (to edit it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<Citation>,
    /// The document has a bibliography field.
    pub has_bibliography: bool,
}

/// Preview of sources that are not in a document (the master list): each source's
/// bibliography entry and citation in `style`.
#[must_use]
pub fn preview(style: StyleId, sources: &[Source]) -> Vec<SourceInfo> {
    infos(style, sources, &[])
}

fn infos(style: StyleId, sources: &[Source], cited: &[String]) -> Vec<SourceInfo> {
    let f = Formatter::new(style, sources, cited);
    // The list is formatted once; each source picks its entry.
    let mut entries: std::collections::HashMap<String, Vec<Span>> = f
        .bibliography(None)
        .into_iter()
        .map(|e| (e.tag.to_lowercase(), e.spans))
        .collect();
    sources
        .iter()
        .map(|s| SourceInfo {
            cited: cited.iter().any(|t| t.eq_ignore_ascii_case(&s.tag)),
            placeholder: s.is_placeholder(),
            preview: entries.remove(&s.tag.to_lowercase()).unwrap_or_default(),
            citation: f.citation(&Citation {
                items: vec![CiteItem {
                    tag: s.tag.clone(),
                    ..CiteItem::default()
                }],
                lcid: None,
            }),
            source: s.clone(),
        })
        .collect()
}

/// Whether the caret at `s` (or the range `s..e`) touches the atom at `pos..pos + len`.
fn touches(collapsed: bool, s: usize, e: usize, pos: usize, len: usize) -> bool {
    if collapsed {
        pos == s || pos + len == s
    } else {
        pos < e && pos + len > s
    }
}

impl Tx<'_> {
    /// Write `list` to the sources part (created when the document has none).
    fn set_sources(&mut self, list: &SourceList) -> Result<()> {
        let old = self.doc.custom_xml.clone();
        let mut new = old.clone();
        let xml = source::write_sources(list);
        if let Some((i, _)) = sources_part(self.doc) {
            new[i].xml = xml;
        } else {
            let (part_name, props) = new_part_names(&old, &[]);
            new.push(CustomXmlPart {
                part_name,
                item_id: Some(new_item_id(&xml, &old)),
                xml,
                // §15.2.5: the schema the part follows.
                schema_refs: vec![source::NS.to_owned()],
                props_part: Some(props),
            });
        }
        if old != new {
            self.push(Op::CustomXml { old, new })?;
        }
        Ok(())
    }

    /// Recompute every citation and bibliography; returns how many changed.
    fn refresh_citations(&mut self) -> Result<usize> {
        let up = fields::update_cite(self.doc);
        if let Some(st) = up.styles {
            self.push(Op::Styles {
                old: Box::new(self.doc.styles.clone()),
                new: Box::new(st),
            })?;
        }
        for (si, blocks) in up.sections.into_iter().enumerate() {
            if self
                .doc
                .sections
                .get(si)
                .is_some_and(|s| s.blocks != blocks)
            {
                self.set_section_blocks(si, &blocks)?;
            }
        }
        Ok(up.count)
    }
}

impl Session {
    /// The sources, the style and the citation at the caret.
    #[must_use]
    pub fn cite_state(&self) -> CiteState {
        let doc = self.doc();
        let sections: Vec<Vec<Block>> = doc.sections.iter().map(|s| s.blocks.clone()).collect();
        let list = sources_part(doc).map(|(_, l)| l).unwrap_or_default();
        let style = StyleId::from_name(&list.style_name);
        let cited = cited_tags(&sections);
        CiteState {
            style,
            sources: infos(style, &list.sources, &cited),
            citation: self.citation_at_caret().map(|(_, _, c)| c),
            has_bibliography: sections.iter().any(|b| {
                fields::toc_ranges(b)
                    .iter()
                    .any(|t| field_kind(t.instr()) == "BIBLIOGRAPHY")
            }),
        }
    }

    /// The citation field the caret touches (or the selection covers): its paragraph, its
    /// atom index and the parsed citation.
    fn citation_at_caret(&self) -> Option<(usize, usize, Citation)> {
        let (a, b) = self.selection.ordered();
        if a.para != b.para {
            return None;
        }
        let paths = bo_model::index::body_paragraphs(self.doc());
        let p = bo_model::index::para_at(self.doc(), paths.get(a.para)?)?;
        let mut pos = 0;
        for (i, (_, c)) in para::flatten(p).iter().enumerate() {
            let l = para::atom_len(c);
            if let RunContent::Field { instr, .. } = c {
                if touches(a == b, a.offset, b.offset, pos, l) {
                    if let Some(cit) = Citation::parse(instr) {
                        return Some((a.para, i, cit));
                    }
                }
            }
            pos += l;
        }
        None
    }

    /// Change the sources (and the style) with `f` and refresh every citation and
    /// bibliography, in one undo step.
    fn cite_change(
        &mut self,
        f: impl FnOnce(&mut SourceList, &mut Tx<'_>) -> Result<()>,
    ) -> Result<usize> {
        let mut n = 0;
        self.edit(|tx, sel| {
            let mut list = sources_part(tx.doc).map(|(_, l)| l).unwrap_or_default();
            f(&mut list, tx)?;
            tx.set_sources(&list)?;
            n = tx.refresh_citations()?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Select the reference style: every citation and the bibliography are rewritten in it
    /// (one undo step). Returns the number of fields whose result changed.
    pub fn cite_set_style(&mut self, style: StyleId) -> Result<usize> {
        self.cite_change(|list, _| {
            if StyleId::from_name(&list.style_name) != style || list.style_name.is_empty() {
                list.style_name = style.id().to_owned();
                // The style file reference of another program no longer applies.
                list.selected_style.clear();
            }
            Ok(())
        })
    }

    /// Add sources to the document, or replace the ones with the same tag. A source without
    /// tag gets one ([`source::new_tag`]). `replace` names the tag of the source being
    /// edited: when the edit changes its tag, the citations follow.
    ///
    /// # Errors
    /// `tagTaken`: the edited source's new tag belongs to another source.
    pub fn cite_save_sources(
        &mut self,
        sources: Vec<Source>,
        replace: Option<&str>,
    ) -> Result<usize> {
        self.cite_change(|list, tx| {
            for mut s in sources {
                s.tag = s.tag.trim().to_owned();
                if !source::SOURCE_TYPES.contains(&s.kind.as_str()) {
                    s.kind = "Misc".into();
                }
                let taken: Vec<String> = list.sources.iter().map(|x| x.tag.clone()).collect();
                let pos = |list: &SourceList, t: &str| {
                    list.sources
                        .iter()
                        .position(|x| x.tag.eq_ignore_ascii_case(t))
                };
                if let Some((i, old)) = replace.and_then(|old| pos(list, old).map(|i| (i, old))) {
                    if s.tag.is_empty() {
                        s.tag = old.to_owned();
                    }
                    if !s.tag.eq_ignore_ascii_case(old) {
                        if pos(list, &s.tag).is_some() {
                            return Err(err("tagTaken"));
                        }
                        retag(tx, old, &s.tag)?;
                    }
                    list.sources[i] = s;
                } else {
                    if s.tag.is_empty() {
                        s.tag = source::new_tag(&s, &taken);
                    }
                    match pos(list, &s.tag) {
                        Some(i) => list.sources[i] = s,
                        None => list.sources.push(s),
                    }
                }
            }
            Ok(())
        })
    }

    /// Delete the source `tag`.
    ///
    /// # Errors
    /// `cited`: a citation refers to it; `noSource`.
    pub fn cite_delete_source(&mut self, tag: &str) -> Result<usize> {
        let sections: Vec<Vec<Block>> = self
            .doc()
            .sections
            .iter()
            .map(|s| s.blocks.clone())
            .collect();
        if cited_tags(&sections)
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag))
        {
            return Err(err("cited"));
        }
        self.cite_change(|list, _| {
            let n = list.sources.len();
            list.sources.retain(|s| !s.tag.eq_ignore_ascii_case(tag));
            if list.sources.len() == n {
                return Err(err("noSource"));
            }
            Ok(())
        })
    }

    /// Insert a citation at the caret (replacing the selection). `new_sources` are added
    /// to the document first (a new source or a placeholder made while citing).
    ///
    /// # Errors
    /// `noSource`: the citation names no source.
    pub fn cite_insert(&mut self, citation: &Citation, new_sources: Vec<Source>) -> Result<usize> {
        if citation.items.iter().all(|i| i.tag.trim().is_empty()) {
            return Err(err("noSource"));
        }
        let instr = citation.instr();
        let mut n = 0;
        self.edit(|tx, sel| {
            let mut list = sources_part(tx.doc).map(|(_, l)| l).unwrap_or_default();
            if list.style_name.is_empty() {
                list.style_name = StyleId::from_name("").id().to_owned();
            }
            for s in new_sources {
                if !list
                    .sources
                    .iter()
                    .any(|x| x.tag.eq_ignore_ascii_case(&s.tag))
                {
                    list.sources.push(s);
                }
            }
            tx.set_sources(&list)?;
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = RunProps {
                link: None,
                ..para::props_at(&atoms, at.offset)
            };
            let field = RunContent::Field {
                instr: instr.clone(),
                cached: String::new(),
                locked: false,
                codes: false,
            };
            tx.insert_atoms(at, vec![(props, field)])?;
            n = tx.refresh_citations()?;
            // The caret goes after the new citation.
            let (_, _, atoms) = tx.atoms(at.para)?;
            let mut pos = 0;
            let mut caret = at;
            for (_, c) in &atoms {
                let l = para::atom_len(c);
                if pos >= at.offset
                    && matches!(c, RunContent::Field { instr: i, .. } if *i == instr)
                {
                    caret = Pos::new(at.para, pos + l);
                    break;
                }
                pos += l;
            }
            Ok(Selection::caret(caret))
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Change the citation the caret touches: its sources, pages, what it leaves out.
    ///
    /// # Errors
    /// `noCitation`: the caret is not at a citation; `noSource`.
    pub fn cite_edit(&mut self, citation: &Citation) -> Result<usize> {
        if citation.items.iter().all(|i| i.tag.trim().is_empty()) {
            return Err(err("noSource"));
        }
        let (pid, index, _) = self.citation_at_caret().ok_or_else(|| err("noCitation"))?;
        let instr = citation.instr();
        let mut n = 0;
        self.edit(|tx, sel| {
            let (path, p, mut atoms) = tx.atoms(pid)?;
            if let Some((_, RunContent::Field { instr: i, .. })) = atoms.get_mut(index) {
                *i = instr;
            }
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            n = tx.refresh_citations()?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Insert a bibliography at the caret: an optional heading paragraph (style
    /// `Heading1`) and the `BIBLIOGRAPHY` field (§17.16.5.7; `lcid`: language of the words
    /// the style adds), filled from the sources.
    ///
    /// # Errors
    /// Inside a table.
    pub fn cite_insert_bibliography(
        &mut self,
        heading: Option<&str>,
        lcid: Option<&str>,
    ) -> Result<usize> {
        let mut instr = String::from("BIBLIOGRAPHY");
        if let Some(l) = lcid
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.chars().all(char::is_alphanumeric))
        {
            instr.push_str(" \\l ");
            instr.push_str(l);
        }
        let heading = heading.map(str::trim).filter(|h| !h.is_empty());
        let mut n = 0;
        self.edit(|tx, sel| {
            let mut list = sources_part(tx.doc).map(|(_, l)| l).unwrap_or_default();
            if list.style_name.is_empty() {
                list.style_name = StyleId::from_name("").id().to_owned();
            }
            tx.set_sources(&list)?;
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            if path.len() != 2 {
                return Err(err("inTable"));
            }
            let p = para_clone(tx.doc, &path)?;
            let len = para_len(&p);
            let (prefix, index) = split_path(&path);
            let insert_at = if at.offset == 0 {
                index
            } else {
                if at.offset < len {
                    tx.split(at)?;
                }
                index + 1
            };
            let mut blocks = Vec::new();
            if let Some(h) = heading {
                if tx.doc.style("Heading1").is_none() {
                    if let Some(st) = crate::builtin_style("Heading1") {
                        let old = tx.doc.styles.clone();
                        let mut new = old.clone();
                        new.styles.push(st);
                        tx.push(Op::Styles {
                            old: Box::new(old),
                            new: Box::new(new),
                        })?;
                    }
                }
                blocks.push(Block::Paragraph(Paragraph::new(
                    ParaProps {
                        style: Some("Heading1".into()),
                        ..ParaProps::default()
                    },
                    para::rebuild(para::text_atoms(h, &RunProps::default())),
                )));
            }
            blocks.push(Block::Paragraph(fields::new_toc_paragraph(&instr)));
            let added = blocks.len();
            tx.insert_blocks(prefix, insert_at, blocks)?;
            n = tx.refresh_citations()?;
            let first = if at.offset == 0 { at.para } else { at.para + 1 };
            let caret = Pos::new(
                (first + added - 1).min(tx.paths().len().saturating_sub(1)),
                0,
            );
            Ok(Selection::caret(caret))
        })?;
        self.fix_selection();
        Ok(n)
    }
}

/// Point every citation of `old` at `new` (a source's tag changed).
fn retag(tx: &mut Tx<'_>, old: &str, new: &str) -> Result<()> {
    for id in 0..tx.paths().len() {
        let (path, p, mut atoms) = tx.atoms(id)?;
        let mut changed = false;
        for (_, c) in &mut atoms {
            let RunContent::Field { instr, .. } = c else {
                continue;
            };
            let Some(mut cit) = Citation::parse(instr) else {
                continue;
            };
            let mut hit = false;
            for it in &mut cit.items {
                if it.tag.eq_ignore_ascii_case(old) {
                    it.tag = new.to_owned();
                    hit = true;
                }
            }
            if hit {
                *instr = cit.instr();
                changed = true;
            }
        }
        if changed {
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
        }
    }
    Ok(())
}

//! Proofing across the stories of a document (`docs/parity/BACKLOG.md` CORE-SPELL-005): the
//! paragraphs of the main text, headers and footers (§17.10), footnotes and endnotes (§17.11) and
//! comments (§17.13.4) in checking order, and corrections made anywhere in them as one undoable
//! transaction (tracked as revisions in track-changes mode, §17.13.5).

use crate::ops::Op;
use crate::para::{self, Atom};
use crate::section::{hf_slot, hf_slot_mut, HfVariant};
use crate::{revisions, EditError, Result, Session, Tx};
use bo_model::{Block, Document, Lang, Paragraph};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A story of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Story {
    /// The main text: paragraphs are numbered as body paragraph ids.
    Body,
    /// A header of a section (§17.10.4 headerReference).
    Header {
        /// 0-based section index.
        section: usize,
        /// Which header.
        variant: HfVariant,
    },
    /// A footer of a section (§17.10.3 footerReference).
    Footer {
        /// 0-based section index.
        section: usize,
        /// Which footer.
        variant: HfVariant,
    },
    /// A footnote (§17.11.10).
    Footnote {
        /// Note id.
        id: i32,
    },
    /// An endnote (§17.11.2).
    Endnote {
        /// Note id.
        id: i32,
    },
    /// A comment (§17.13.4.2), by its position in the comments part.
    Comment {
        /// 0-based index in `Document::comments`.
        index: usize,
    },
}

const VARIANTS: [HfVariant; 3] = [HfVariant::Default, HfVariant::First, HfVariant::Even];

/// Every story with its paragraphs, in checking order: main text, then headers and footers of each
/// section, footnotes, endnotes and comments.
#[must_use]
pub fn stories(doc: &Document) -> Vec<(Story, Vec<&Paragraph>)> {
    let mut out = Vec::new();
    let body: Vec<&Paragraph> = bo_model::index::body_paragraphs(doc)
        .iter()
        .filter_map(|p| bo_model::index::para_at(doc, p))
        .collect();
    out.push((Story::Body, body));
    for (si, s) in doc.sections.iter().enumerate() {
        for (header, set) in [(true, &s.headers), (false, &s.footers)] {
            for v in VARIANTS {
                if let Some(blocks) = hf_slot(set, v) {
                    let story = if header {
                        Story::Header {
                            section: si,
                            variant: v,
                        }
                    } else {
                        Story::Footer {
                            section: si,
                            variant: v,
                        }
                    };
                    out.push((story, paragraphs(blocks)));
                }
            }
        }
    }
    for (id, blocks) in &doc.footnotes {
        out.push((Story::Footnote { id: *id }, paragraphs(blocks)));
    }
    for (id, blocks) in &doc.endnotes {
        out.push((Story::Endnote { id: *id }, paragraphs(blocks)));
    }
    for (i, c) in doc.comments.iter().enumerate() {
        out.push((Story::Comment { index: i }, paragraphs(&c.blocks)));
    }
    out
}

/// Paragraphs of a block list in order, descending into table cells.
#[must_use]
pub fn paragraphs(blocks: &[Block]) -> Vec<&Paragraph> {
    let mut out = Vec::new();
    walk(blocks, &mut out);
    out
}

fn walk<'a>(blocks: &'a [Block], out: &mut Vec<&'a Paragraph>) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => out.push(p),
            Block::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        walk(&c.blocks, out);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// The `n`-th paragraph of a block list (order of [`paragraphs`]).
fn nth_mut<'a>(blocks: &'a mut [Block], n: &mut usize) -> Option<&'a mut Paragraph> {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                if *n == 0 {
                    return Some(p);
                }
                *n -= 1;
            }
            Block::Table(t) => {
                for r in &mut t.rows {
                    for c in &mut r.cells {
                        if let Some(p) = nth_mut(&mut c.blocks, n) {
                            return Some(p);
                        }
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
    None
}

/// A correction made by the proofing dialog: replace `start..end` (byte offsets in the paragraph
/// index text) by `text`, and/or set the language of that range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofEdit {
    /// Story of the paragraph.
    pub story: Story,
    /// Paragraph index in the story (body paragraph id for the main text).
    pub para: usize,
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// Replacement text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Language to give the range (§17.3.2.20 lang `@w:val`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl Tx<'_> {
    /// Apply the edits of one paragraph (sorted last first) to its atoms.
    fn proof_atoms(
        &mut self,
        text: &str,
        atoms: &mut Vec<Atom>,
        edits: &[&ProofEdit],
    ) -> Result<()> {
        for e in edits {
            if e.start > e.end
                || e.end > text.len()
                || !text.is_char_boundary(e.start)
                || !text.is_char_boundary(e.end)
            {
                return Err(EditError::Conflict);
            }
            let mut end = e.end;
            if let Some(t) = &e.text {
                let props = para::props_at(atoms, (e.start + 1).min(e.end.max(e.start)));
                let at = if let Some(tr) = self.track.as_mut() {
                    let mark = tr.mark();
                    let removed = revisions::mark_deleted(atoms, e.start, e.end, tr, &mark);
                    e.end - removed
                } else {
                    para::delete(atoms, e.start, e.end);
                    e.start
                };
                let new = self.mark_new(atoms, at, para::text_atoms(t, &props));
                let n = para::len(&new);
                para::insert(atoms, at, new);
                end = at + n;
            }
            if let Some(tag) = e.lang.as_ref().filter(|t| !t.trim().is_empty()) {
                let start = if e.text.is_some() && self.track.is_some() {
                    end - e.text.as_ref().map_or(0, String::len)
                } else {
                    e.start
                };
                para::format(atoms, start, end, &|r| {
                    let l = r.lang.take().unwrap_or_default();
                    r.lang = Some(Lang {
                        val: Some(tag.clone()),
                        ..l
                    });
                });
            }
        }
        Ok(())
    }

    /// Apply the edits of one paragraph of a block list in place.
    fn proof_blocks(
        &mut self,
        blocks: &mut [Block],
        para_index: usize,
        edits: &[&ProofEdit],
    ) -> Result<()> {
        let mut n = para_index;
        let p = nth_mut(blocks, &mut n).ok_or(EditError::BadPath)?;
        let text = bo_model::index::para_index_text(p);
        let mut atoms = para::flatten(p);
        self.proof_atoms(&text, &mut atoms, edits)?;
        *p = para::with_atoms(p, atoms);
        Ok(())
    }
}

impl Session {
    /// Apply proofing corrections anywhere in the document as one undoable transaction; returns the
    /// number of edits applied. The selection is kept (offsets in the main text after a changed
    /// word may move).
    pub fn proof_edit(&mut self, edits: &[ProofEdit]) -> Result<usize> {
        if edits.is_empty() {
            return Ok(0);
        }
        // Group by story and paragraph, last edit first inside a paragraph.
        let mut groups: BTreeMap<(usize, usize), Vec<&ProofEdit>> = BTreeMap::new();
        let order = stories(&self.doc);
        for e in edits {
            let si = order
                .iter()
                .position(|(s, _)| *s == e.story)
                .ok_or(EditError::BadPath)?;
            groups.entry((si, e.para)).or_default().push(e);
        }
        for g in groups.values_mut() {
            g.sort_by_key(|e| std::cmp::Reverse((e.start, e.end)));
        }
        let stories: Vec<Story> = order.into_iter().map(|(s, _)| s).collect();
        self.edit(|tx, sel| {
            for ((si, para), g) in &groups {
                match stories[*si] {
                    Story::Body => {
                        let (path, p, mut atoms) = tx.atoms(*para)?;
                        let text = bo_model::index::para_index_text(&p);
                        tx.proof_atoms(&text, &mut atoms, g)?;
                        tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                    }
                    Story::Header { section, variant } | Story::Footer { section, variant } => {
                        let header = matches!(stories[*si], Story::Header { .. });
                        let old = tx
                            .doc
                            .sections
                            .get(section)
                            .ok_or(EditError::BadPath)?
                            .clone();
                        let mut new = old.clone();
                        let set = if header {
                            &mut new.headers
                        } else {
                            &mut new.footers
                        };
                        let blocks = hf_slot_mut(set, variant)
                            .as_mut()
                            .ok_or(EditError::BadPath)?;
                        tx.proof_blocks(blocks, *para, g)?;
                        if new != old {
                            tx.push(Op::Sections {
                                index: section,
                                remove: vec![old],
                                insert: vec![new],
                            })?;
                        }
                    }
                    Story::Footnote { id } | Story::Endnote { id } => {
                        let endnote = matches!(stories[*si], Story::Endnote { .. });
                        let old = if endnote {
                            tx.doc.endnotes.clone()
                        } else {
                            tx.doc.footnotes.clone()
                        };
                        let mut new = old.clone();
                        let blocks = new.get_mut(&id).ok_or(EditError::BadPath)?;
                        tx.proof_blocks(blocks, *para, g)?;
                        if new != old {
                            tx.push(Op::Notes { endnote, old, new })?;
                        }
                    }
                    Story::Comment { index } => {
                        let old = tx.doc.comments.clone();
                        let mut new = old.clone();
                        let c = new.get_mut(index).ok_or(EditError::BadPath)?;
                        tx.proof_blocks(&mut c.blocks, *para, g)?;
                        if new != old {
                            tx.push(Op::Comments { old, new })?;
                        }
                    }
                }
            }
            Ok(sel)
        })?;
        Ok(edits.len())
    }

    /// In "detect language automatically" mode: after a word separator was typed, give the
    /// sentence before the caret the language `detect` finds for it when the text has another
    /// one (its own undo step, as AutoCorrect). Returns the language set.
    pub fn tag_language(
        &mut self,
        detect: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Option<String>> {
        let sel = self.selection;
        if !sel.is_collapsed() {
            return Ok(None);
        }
        let at = sel.focus;
        let paths = bo_model::index::body_paragraphs(&self.doc);
        let Some(p) = paths
            .get(at.para)
            .and_then(|path| bo_model::index::para_at(&self.doc, path))
        else {
            return Ok(None);
        };
        let text = bo_model::index::para_index_text(p);
        if at.offset == 0 || at.offset > text.len() || !text.is_char_boundary(at.offset) {
            return Ok(None);
        }
        // Only right after a separator that ends a word.
        let head = &text[..at.offset];
        let Some(last) = head.chars().last() else {
            return Ok(None);
        };
        if last.is_alphanumeric() {
            return Ok(None);
        }
        // From the start of the sentence (or paragraph) to the caret.
        let start = head[..head.len() - last.len_utf8()]
            .rfind(['.', '!', '?', '\n', '\u{2028}'])
            .map_or(0, |i| i + 1);
        let Some(lang) = detect(&head[start..]) else {
            return Ok(None);
        };
        let atoms = para::flatten(p);
        // Does any text of the range have another language?
        let mut pos = 0;
        let mut differs = false;
        for (props, item) in &atoms {
            let l = para::atom_len(item);
            if pos < at.offset && pos + l > start {
                let eff = crate::effective_run(&self.doc, p, props);
                if eff.lang.and_then(|l| l.val).as_deref() != Some(lang.as_str()) {
                    differs = true;
                }
            }
            pos += l;
        }
        if !differs {
            return Ok(None);
        }
        let edit = ProofEdit {
            story: Story::Body,
            para: at.para,
            start,
            end: at.offset,
            text: None,
            lang: Some(lang.clone()),
        };
        // Text typed next takes the language of the run before the caret.
        self.proof_edit(&[edit])?;
        Ok(Some(lang))
    }
}

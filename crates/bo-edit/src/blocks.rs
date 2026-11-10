//! Building blocks (ECMA-376 Part 1, §17.12 glossary document): taking the
//! selection as block content, inserting a block — at the caret, as a header or
//! footer, or as a cover page — with the styles, numbering and pictures it needs,
//! AutoText expansion (F3) and replacing the document's glossary, each as one
//! undo step.
//!
//! Content moves between documents the way the Organizer copies styles
//! ([`crate::styles::copy_styles`]): a style the target already has keeps the
//! target's definition, a missing one is copied with its `basedOn` chain; every
//! numbering instance (§17.9.15 num) the content uses is copied under a fresh id;
//! pictures are copied to the target's media under a free part name.

use crate::section::HfVariant;
use crate::{para, split_path, EditError, Op, Pos, Result, Selection, Session};
use bo_model::glossary::Glossary;
use bo_model::index::{blocks_at, body_paragraphs, para_at, para_index_text};
use bo_model::{Block, Document, NumRef, ParaProps, Paragraph, Run, RunContent, RunProps};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Media parts by name.
type Media = BTreeMap<String, Arc<Vec<u8>>>;

/// Where a building block goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTarget {
    /// At the caret (replacing the selection), following the block's behaviour
    /// (§17.18.14 `ST_DocPartBehavior`).
    Caret,
    /// At the start of the document, followed by a page break (cover pages).
    DocStart,
    /// Replacing a header (`header`) or footer of section `section` (§17.10).
    HeaderFooter {
        /// Section index.
        section: usize,
        /// Header (`true`) or footer.
        header: bool,
        /// Which variant of the section.
        variant: HfVariant,
    },
}

/// Visit every paragraph of `blocks`, descending into table cells and text boxes.
pub fn for_each_paragraph_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Paragraph)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for r in &mut p.runs {
                    for c in &mut r.content {
                        if let RunContent::Image(img) = c {
                            if let Some(text) = img.shape.as_mut().and_then(|s| s.text.as_mut()) {
                                for_each_paragraph_mut(text, f);
                            }
                        }
                    }
                }
                f(p);
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        for_each_paragraph_mut(&mut cell.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

fn for_each_table_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut bo_model::Table)) {
    for b in blocks {
        if let Block::Table(t) = b {
            f(t);
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for_each_table_mut(&mut cell.blocks, f);
                }
            }
        }
    }
}

/// Block content detached from its source part: no source XML (its relationship
/// ids belong to the part it was read from), no trailing section properties of a
/// `w:docPartBody` (§17.12.6), no note references, comment marks or revision
/// markup (those point into stories the block does not carry).
#[must_use]
pub fn detached(blocks: &[Block]) -> Vec<Block> {
    // The `src` fields are skipped by serde.
    let mut out: Vec<Block> = serde_json::to_value(blocks)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    out.retain(|b| !matches!(b, Block::Opaque(r) if r.xml.trim_start().starts_with("<w:sectPr")));
    for_each_paragraph_mut(&mut out, &mut |p| {
        for r in &mut p.runs {
            r.content.retain(|c| {
                !matches!(
                    c,
                    RunContent::FootnoteRef(_)
                        | RunContent::EndnoteRef(_)
                        | RunContent::NoteMark
                        | RunContent::Markup(_)
                        | RunContent::CommentStart(_)
                        | RunContent::CommentEnd(_)
                        | RunContent::CommentRef(_)
                )
            });
        }
        p.runs.retain(|r| !r.content.is_empty());
        p.props.mark_ins = None;
        p.props.mark_del = None;
    });
    out
}

/// Make `blocks` (content of `from`) usable in `to`: styles `to` lacks are copied,
/// numbering instances are copied under fresh ids and the paragraphs renumbered,
/// pictures missing from `media` (the target's media) are returned as
/// `(part name, bytes)` to add — under a new name when the name is taken by
/// another picture.
pub fn import_blocks(
    from: &Document,
    to: &mut Document,
    media: &Media,
    blocks: &mut [Block],
) -> Vec<(String, Arc<Vec<u8>>)> {
    // Styles (§17.7): paragraph, character and table styles in use.
    let mut ids = BTreeSet::new();
    for_each_paragraph_mut(blocks, &mut |p| {
        ids.extend(p.props.style.clone());
        for r in &p.runs {
            ids.extend(r.props.style.clone());
        }
    });
    for_each_table_mut(blocks, &mut |t| ids.extend(t.props.style.clone()));
    let missing: Vec<String> = ids
        .into_iter()
        .filter(|id| to.style(id).is_none() && from.style(id).is_some())
        .collect();
    if !missing.is_empty() {
        crate::styles::copy_styles(from, to, &missing);
    }
    // Numbering (§17.9): one new instance per instance used.
    let mut nums: BTreeMap<i32, Option<i32>> = BTreeMap::new();
    for_each_paragraph_mut(blocks, &mut |p| {
        if let Some(n) = p.props.num.filter(|n| n.num_id != 0) {
            let new = *nums
                .entry(n.num_id)
                .or_insert_with(|| crate::styles::copy_num(from, to, n.num_id));
            p.props.num = new.map(|num_id| NumRef { num_id, ..n });
        }
    });
    // Pictures: same bytes under the same name are shared.
    let mut added: Vec<(String, Arc<Vec<u8>>)> = Vec::new();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    for_each_paragraph_mut(blocks, &mut |p| {
        for r in &mut p.runs {
            for c in &mut r.content {
                let RunContent::Image(img) = c else { continue };
                if img.media.is_empty() {
                    continue;
                }
                if let Some(n) = renamed.get(&img.media) {
                    img.media.clone_from(n);
                    continue;
                }
                let Some(bytes) = from.media.get(&img.media) else {
                    continue;
                };
                let old = img.media.clone();
                if !has_bytes(media, &added, &old, bytes) {
                    if is_taken(media, &added, &old) {
                        let (stem, ext) = old.rsplit_once('.').unwrap_or((&old, "bin"));
                        let mut n = 1;
                        let name = loop {
                            let cand = format!("{stem}-b{n}.{ext}");
                            if has_bytes(media, &added, &cand, bytes)
                                || !is_taken(media, &added, &cand)
                            {
                                break cand;
                            }
                            n += 1;
                        };
                        if !has_bytes(media, &added, &name, bytes) {
                            added.push((name.clone(), bytes.clone()));
                        }
                        img.media.clone_from(&name);
                    } else {
                        added.push((old.clone(), bytes.clone()));
                    }
                }
                renamed.insert(old, img.media.clone());
            }
        }
    });
    added
}

/// `name` holds exactly `bytes` in the target's media or the parts being added.
fn has_bytes(
    media: &Media,
    added: &[(String, Arc<Vec<u8>>)],
    name: &str,
    bytes: &Arc<Vec<u8>>,
) -> bool {
    media.get(name).is_some_and(|b| b == bytes)
        || added.iter().any(|(n, b)| n == name && b == bytes)
}

fn is_taken(media: &Media, added: &[(String, Arc<Vec<u8>>)], name: &str) -> bool {
    media.contains_key(name) || added.iter().any(|(n, _)| n == name)
}

/// A scratch document holding the glossary's styles and numbering (what
/// [`import_blocks`] reads from or copies into) with the media of `media`.
#[must_use]
pub fn glossary_document(g: &Glossary, media: &Media) -> Document {
    Document {
        styles: g.styles.clone(),
        default_run: g.default_run.clone(),
        default_para: g.default_para.clone(),
        numbering: g.numbering.clone(),
        media: media.clone(),
        ..Document::default()
    }
}

/// A paragraph holding only a page break (§17.3.3.1 br `type="page"`).
fn page_break() -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::PageBreak],
        }],
    ))
}

/// Id of the first body paragraph at or under block path `path`.
fn first_para(doc: &Document, path: &[usize]) -> Option<usize> {
    body_paragraphs(doc)
        .iter()
        .position(|p| p.starts_with(path))
}

impl Session {
    /// The selection as building block content: the selected part of a single
    /// paragraph, or — across blocks — the first and last paragraphs cut at the
    /// selection ends with every block between them whole (tables included; a
    /// selection that starts or ends inside a table takes the whole table).
    pub fn selection_block_content(&self) -> Vec<Block> {
        let (a, b) = self.selection.ordered();
        let sliced = self.selected_blocks();
        if a.para == b.para {
            return detached(&sliced);
        }
        let paths = body_paragraphs(&self.doc);
        let (Some(pa), Some(pb)) = (paths.get(a.para), paths.get(b.para)) else {
            return detached(&sliced);
        };
        let top = |p: &[usize]| {
            (
                p.first().copied().unwrap_or(0),
                p.get(1).copied().unwrap_or(0),
            )
        };
        let (start, end) = (top(pa), top(pb));
        let mut out = Vec::new();
        for (si, sec) in self.doc.sections.iter().enumerate() {
            for (bi, block) in sec.blocks.iter().enumerate() {
                if (si, bi) < start || (si, bi) > end {
                    continue;
                }
                match block {
                    // The first / last paragraph: only its selected part.
                    Block::Paragraph(_) if (si, bi) == start && pa.len() == 2 => {
                        out.extend(sliced.first().cloned());
                    }
                    Block::Paragraph(_) if (si, bi) == end && pb.len() == 2 => {
                        out.extend(sliced.last().cloned());
                    }
                    other => out.push(other.clone()),
                }
            }
        }
        detached(&out)
    }

    /// Replace the building blocks stored in the document (one undo step).
    pub fn set_glossary(&mut self, new: Option<Arc<Glossary>>) -> Result<()> {
        if self.doc.glossary == new {
            return Ok(());
        }
        let old = self.doc.glossary.clone();
        self.edit(move |tx, sel| {
            tx.push(Op::Glossary { old, new })?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Insert building block content `blocks` of `from` (the document — or
    /// [`glossary_document`] — that defines its styles, numbering and pictures) at
    /// `target`, as one undo step. `behavior` is the block's §17.18.14 behaviour:
    /// `content` joins a single paragraph into the text at the caret, `p` puts the
    /// block in paragraphs of its own, `pg` on a page of its own.
    pub fn insert_building_block(
        &mut self,
        from: &Document,
        blocks: &[Block],
        behavior: &str,
        target: BlockTarget,
    ) -> Result<Selection> {
        let mut blocks = detached(blocks);
        if blocks.is_empty() {
            return Ok(self.selection);
        }
        let mut scratch = Document {
            styles: self.doc.styles.clone(),
            default_run: self.doc.default_run.clone(),
            default_para: self.doc.default_para.clone(),
            numbering: self.doc.numbering.clone(),
            ..Document::default()
        };
        let media = import_blocks(from, &mut scratch, &self.doc.media, &mut blocks);
        let old_styles = self.doc.styles.clone();
        let old_numbering = self.doc.numbering.clone();
        let behavior = behavior.to_owned();
        self.edit(move |tx, sel| {
            if scratch.styles != old_styles {
                tx.push(Op::Styles {
                    old: Box::new(old_styles),
                    new: Box::new(scratch.styles),
                })?;
            }
            if scratch.numbering != old_numbering {
                tx.push(Op::Numbering {
                    old: Box::new(old_numbering),
                    new: Box::new(scratch.numbering),
                })?;
            }
            for (name, bytes) in media {
                tx.push(Op::Media {
                    name,
                    old: None,
                    new: Some(bytes),
                })?;
            }
            match target {
                BlockTarget::HeaderFooter {
                    section,
                    header,
                    variant,
                } => {
                    let old = tx
                        .doc
                        .sections
                        .get(section)
                        .cloned()
                        .ok_or(EditError::BadPath)?;
                    let mut new = old.clone();
                    let set = if header {
                        &mut new.headers
                    } else {
                        &mut new.footers
                    };
                    *crate::section::hf_slot_mut(set, variant) = Some(blocks);
                    if new != old {
                        tx.push(Op::Sections {
                            index: section,
                            remove: vec![old],
                            insert: vec![new],
                        })?;
                    }
                    Ok(sel)
                }
                BlockTarget::DocStart => {
                    let mut blocks = blocks;
                    blocks.push(page_break());
                    let n = blocks.len();
                    tx.insert_blocks(vec![0], 0, blocks)?;
                    let caret = first_para(tx.doc, &[0, n]).ok_or(EditError::BadPath)?;
                    Ok(Selection::caret(Pos::new(caret, 0)))
                }
                BlockTarget::Caret => {
                    let (a, b) = sel.ordered();
                    let at = tx.delete_range(a, b)?;
                    // A single paragraph inserted as content joins the text.
                    if behavior != "p" && behavior != "pg" {
                        if let [Block::Paragraph(p)] = blocks.as_slice() {
                            let atoms = para::flatten(p);
                            return Ok(Selection::caret(tx.insert_atoms(at, atoms)?));
                        }
                    }
                    let (path, _, atoms) = tx.atoms(at.para)?;
                    let len = para::len(&atoms);
                    let (prefix, index) = split_path(&path);
                    let insert_at = if at.offset == 0 {
                        index
                    } else {
                        if at.offset < len {
                            tx.split(at)?;
                        }
                        index + 1
                    };
                    let mut out = Vec::with_capacity(blocks.len() + 3);
                    if behavior == "pg" && (insert_at > 0 || prefix != [0]) {
                        out.push(page_break());
                    }
                    for b in blocks {
                        out.push(match b {
                            Block::Paragraph(mut q) => {
                                q.runs = para::rebuild(tx.mark_new(&[], 0, para::flatten(&q)));
                                tx.new_mark(&mut q.props);
                                Block::Paragraph(q)
                            }
                            Block::Table(mut t) => {
                                if let Some(tr) = tx.track.as_mut() {
                                    // §17.13.5.19: a tracked new table is a set of inserted rows.
                                    let m = tr.mark();
                                    for r in &mut t.rows {
                                        r.props.ins = Some(m.clone());
                                    }
                                }
                                Block::Table(t)
                            }
                            other @ Block::Opaque(_) => other,
                        });
                    }
                    if behavior == "pg" {
                        out.push(page_break());
                    }
                    let container_len = blocks_at(tx.doc, &prefix).map_or(0, Vec::len);
                    if insert_at >= container_len {
                        // Content ends with a paragraph (§17.4.38 note on cell/body end).
                        out.push(Block::Paragraph(Paragraph::default()));
                    }
                    let n = out.len();
                    tx.insert_blocks(prefix.clone(), insert_at, out)?;
                    // The caret goes after the block.
                    let mut after = prefix.clone();
                    after.push(insert_at + n);
                    let caret = match first_para(tx.doc, &after) {
                        Some(id) => id,
                        None => {
                            let mut last = prefix;
                            last.push(insert_at + n - 1);
                            first_para(tx.doc, &last).ok_or(EditError::BadPath)?
                        }
                    };
                    Ok(Selection::caret(Pos::new(caret, 0)))
                }
            }
        })
    }

    /// AutoText (F3): which of `names` the text before the caret calls for, and
    /// where that text starts in the caret's paragraph. A name typed in full wins
    /// (the longest one, ignoring case, starting at a word boundary); otherwise the
    /// word before the caret when exactly one name starts with it.
    pub fn autotext_match(&self, names: &[&str]) -> Option<(usize, usize)> {
        let sel = self.selection;
        if !sel.is_collapsed() {
            return None;
        }
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(sel.focus.para)?)?;
        let text = para_index_text(p);
        let before = text.get(..sel.focus.offset)?;
        let lower = before.to_lowercase();
        let boundary = |start: usize| {
            before[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric())
        };
        // `to_lowercase` may change byte lengths: map the match back by characters.
        let start_of = |suffix_chars: usize| {
            before
                .char_indices()
                .rev()
                .nth(suffix_chars.checked_sub(1)?)
                .map(|(i, _)| i)
        };
        let mut best: Option<(usize, usize, usize)> = None; // (index, start, chars)
        for (i, name) in names.iter().enumerate() {
            let n = name.trim().to_lowercase();
            if n.is_empty() || !lower.ends_with(&n) {
                continue;
            }
            let chars = n.chars().count();
            let Some(start) = start_of(chars) else {
                continue;
            };
            if boundary(start) && best.is_none_or(|b| chars > b.2) {
                best = Some((i, start, chars));
            }
        }
        if let Some((i, start, _)) = best {
            return Some((i, start));
        }
        // A unique beginning of a name.
        let word_start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| !c.is_whitespace())
            .last()
            .map(|(i, _)| i)?;
        let word = before[word_start..].to_lowercase();
        let mut hits = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.trim().to_lowercase().starts_with(&word));
        match (hits.next(), hits.next()) {
            (Some((i, _)), None) => Some((i, word_start)),
            _ => None,
        }
    }

    /// AutoText (F3): replace the text from `start` to the caret in the caret's
    /// paragraph by the building block (one undo step).
    pub fn expand_autotext(
        &mut self,
        start: usize,
        from: &Document,
        blocks: &[Block],
        behavior: &str,
    ) -> Result<Selection> {
        let caret = self.selection.focus;
        let before = self.selection;
        self.selection = Selection {
            anchor: Pos::new(caret.para, start),
            focus: caret,
        };
        let res = self.insert_building_block(from, blocks, behavior, BlockTarget::Caret);
        if res.is_err() {
            self.selection = before;
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Section, Style, StyleKind, Table, Twips};

    fn para(text: &str) -> Block {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(text, RunProps::default())],
        ))
    }

    fn doc(blocks: Vec<Block>) -> Document {
        Document {
            sections: vec![Section::new(blocks)],
            ..Document::default()
        }
    }

    fn texts(d: &Document) -> Vec<String> {
        d.paragraphs().map(Paragraph::plain_text).collect()
    }

    #[test]
    fn selection_across_a_table_keeps_the_table() {
        let table = Block::Table(Table::simple(1, 2, Twips(9000)));
        let mut s = Session::new(doc(vec![para("Đầu bảng"), table, para("Cuối bảng")]));
        // Paragraph ids: 0, two cells (1, 2), 3.
        s.set_selection(Selection {
            anchor: Pos::new(0, "Đầu ".len()),
            focus: Pos::new(3, "Cuối".len()),
        });
        let c = s.selection_block_content();
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[0].as_paragraph().map(Paragraph::plain_text).as_deref(),
            Some("bảng")
        );
        assert!(matches!(c[1], Block::Table(_)));
        assert_eq!(
            c[2].as_paragraph().map(Paragraph::plain_text).as_deref(),
            Some("Cuối")
        );
    }

    #[test]
    fn content_block_joins_the_text_and_undo_restores() {
        let mut s = Session::new(doc(vec![para("Kính gửi: .")]));
        s.set_selection(Selection::caret(Pos::new(0, "Kính gửi: ".len())));
        let from = Document::default();
        s.insert_building_block(
            &from,
            &[para("Ban Giám đốc")],
            "content",
            BlockTarget::Caret,
        )
        .unwrap();
        assert_eq!(texts(s.doc()), ["Kính gửi: Ban Giám đốc."]);
        s.undo();
        assert_eq!(texts(s.doc()), ["Kính gửi: ."]);
    }

    #[test]
    fn paragraph_block_splits_the_paragraph_and_brings_its_style_and_list() {
        let mut from = Document::default();
        let mut st = Style::new("Quote1", "Trích dẫn", StyleKind::Paragraph);
        st.rpr.italic = Some(true);
        from.styles.styles.push(st);
        from.numbering.abstracts.push(bo_model::AbstractNum {
            id: 4,
            levels: Vec::new(),
        });
        from.numbering.nums.push(bo_model::Num {
            id: 7,
            abstract_id: 4,
            start_overrides: Vec::new(),
        });
        let mut q = Paragraph::new(
            ParaProps::default(),
            vec![Run::text("Mục một", RunProps::default())],
        );
        q.props.style = Some("Quote1".into());
        q.props.num = Some(NumRef { num_id: 7, ilvl: 0 });
        let table = Block::Table(Table::simple(2, 2, Twips(9000)));
        let mut s = Session::new(doc(vec![para("TrướcSau")]));
        s.set_selection(Selection::caret(Pos::new(0, "Trước".len())));
        let sel = s
            .insert_building_block(
                &from,
                &[Block::Paragraph(q), table],
                "p",
                BlockTarget::Caret,
            )
            .unwrap();
        let d = s.doc();
        assert_eq!(d.sections[0].blocks.len(), 4);
        assert_eq!(texts(d)[0], "Trước");
        assert_eq!(texts(d)[1], "Mục một");
        assert!(matches!(d.sections[0].blocks[2], Block::Table(_)));
        // Caret at the start of the paragraph after the block.
        assert_eq!(sel, Selection::caret(Pos::new(6, 0)));
        assert!(d.style("Quote1").is_some());
        let num = d.sections[0].blocks[1]
            .as_paragraph()
            .and_then(|p| p.props.num)
            .unwrap();
        assert!(d.numbering.nums.iter().any(|n| n.id == num.num_id));
        // One undo step removes the block, the style and the list.
        s.undo();
        assert_eq!(texts(s.doc()), ["TrướcSau"]);
        assert!(s.doc().style("Quote1").is_none());
        assert!(s.doc().numbering.nums.is_empty());
    }

    #[test]
    fn header_and_cover_page_targets() {
        let mut s = Session::new(doc(vec![para("Thân")]));
        let from = Document::default();
        s.insert_building_block(
            &from,
            &[para("Đầu trang")],
            "p",
            BlockTarget::HeaderFooter {
                section: 0,
                header: true,
                variant: HfVariant::Default,
            },
        )
        .unwrap();
        let h = s.doc().sections[0].headers.default.as_ref().unwrap();
        assert_eq!(
            h[0].as_paragraph().map(Paragraph::plain_text).as_deref(),
            Some("Đầu trang")
        );
        s.insert_building_block(&from, &[para("Bìa")], "pg", BlockTarget::DocStart)
            .unwrap();
        let d = s.doc();
        assert_eq!(texts(d)[0], "Bìa");
        let brk = d.sections[0].blocks[1].as_paragraph().unwrap();
        assert!(matches!(brk.runs[0].content[0], RunContent::PageBreak));
        assert_eq!(texts(d)[2], "Thân");
    }

    #[test]
    fn pictures_are_copied_under_a_free_name() {
        let img = |name: &str| {
            let image = bo_model::Image {
                media: name.into(),
                width: bo_model::Emu(100),
                height: bo_model::Emu(100),
                anchor: None,
                crop: None,
                rotation: 0,
                description: String::new(),
                shape: None,
                format: None,
                ole: None,
                src: None,
            };
            Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run {
                    props: RunProps::default(),
                    content: vec![RunContent::Image(Box::new(image))],
                }],
            ))
        };
        let mut from = Document::default();
        from.media
            .insert("/word/media/image1.png".into(), Arc::new(vec![1, 2, 3]));
        let mut target = doc(vec![para("x")]);
        target
            .media
            .insert("/word/media/image1.png".into(), Arc::new(vec![9]));
        let mut s = Session::new(target);
        s.insert_building_block(
            &from,
            &[img("/word/media/image1.png")],
            "p",
            BlockTarget::Caret,
        )
        .unwrap();
        let d = s.doc();
        assert_eq!(d.media.len(), 2);
        let (name, bytes) = d.media.iter().find(|(_, b)| ***b == [1, 2, 3]).unwrap();
        assert_ne!(name, "/word/media/image1.png");
        assert_eq!(bytes.len(), 3);
        let used = d.sections[0].blocks[0]
            .as_paragraph()
            .and_then(|p| match &p.runs[0].content[0] {
                RunContent::Image(i) => Some(i.media.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(&used, name);
    }

    #[test]
    fn autotext_matches_full_names_and_unique_beginnings() {
        let mut s = Session::new(doc(vec![para("Xin gửi nn"), para("ký thay chu")]));
        let names = ["Nơi nhận", "nn", "Chữ ký", "chu ky tat"];
        s.set_selection(Selection::caret(Pos::new(0, "Xin gửi nn".len())));
        assert_eq!(s.autotext_match(&names), Some((1, "Xin gửi ".len())));
        // "chu" starts only "chu ky tat".
        s.set_selection(Selection::caret(Pos::new(1, "ký thay chu".len())));
        assert_eq!(s.autotext_match(&names), Some((3, "ký thay ".len())));
        // No boundary before the name: not a match.
        let mut s = Session::new(doc(vec![para("xnn")]));
        s.set_selection(Selection::caret(Pos::new(0, 3)));
        assert_eq!(s.autotext_match(&["nn"]), None);
        // A name of several words, upper case typed.
        let mut s = Session::new(doc(vec![para("Thưa NƠI NHẬN")]));
        s.set_selection(Selection::caret(Pos::new(0, "Thưa NƠI NHẬN".len())));
        assert_eq!(s.autotext_match(&names), Some((0, "Thưa ".len())));
        let from = Document::default();
        s.expand_autotext("Thưa ".len(), &from, &[para("Nơi nhận:")], "content")
            .unwrap();
        assert_eq!(texts(s.doc()), ["Thưa Nơi nhận:"]);
    }

    #[test]
    fn glossary_replacement_is_undoable() {
        let mut s = Session::new(doc(vec![para("x")]));
        let g = Arc::new(Glossary::default());
        s.set_glossary(Some(g.clone())).unwrap();
        assert_eq!(s.doc().glossary, Some(g));
        s.undo();
        assert!(s.doc().glossary.is_none());
    }
}

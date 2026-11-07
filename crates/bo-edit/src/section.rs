//! Page setup, headers/footers, footnotes/endnotes, Format Painter, Clear
//! Formatting and Paste Special.
//!
//! Section properties: ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar, §17.6.4 cols,
//! §17.10 headers and footers (§17.10.6 titlePg, §17.15.1.35 evenAndOddHeaders);
//! notes: §17.11 (§17.11.14 footnoteReference, §17.11.13 footnoteRef).

use crate::para::{self, Atom};
use crate::{effective_run, html, EditError, Op, Result, SectionState, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{
    Block, Columns, HalfPoints, PageSetup, ParaProps, Paragraph, Run, RunContent, RunProps, Twips,
    Underline, VertAlign,
};
use serde::{Deserialize, Serialize};

/// Page setup sent by hosts (twips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSetupSpec {
    /// §17.6.13 pgSz @w:w / @w:h (orientation follows from width > height).
    pub width: i32,
    pub height: i32,
    /// §17.6.11 pgMar.
    pub margin_top: i32,
    pub margin_bottom: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    /// §17.6.11 pgMar @w:header / @w:footer: distance of the header/footer from the edge.
    pub header: i32,
    pub footer: i32,
    /// §17.6.4 cols @w:num / @w:space.
    #[serde(default = "one")]
    pub columns: u16,
    #[serde(default = "half_inch")]
    pub column_space: i32,
}

fn one() -> u16 {
    1
}

fn half_inch() -> i32 {
    720
}

/// Sections a page setup applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupScope {
    /// The section holding the caret.
    #[default]
    Section,
    /// Every section.
    Document,
}

/// Paste Special options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteMode {
    /// Keep the source formatting (internal format or HTML).
    #[default]
    Keep,
    /// Take the destination's formatting, keep bold/italic/underline/strike/sub/sup.
    Merge,
    /// Unformatted text.
    Text,
}

/// A footnote or endnote for the notes pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInfo {
    pub id: i32,
    pub endnote: bool,
    /// Text without the note mark (paragraphs joined by `\n`).
    pub text: String,
    /// Body paragraph holding the reference (when referenced).
    pub para: Option<usize>,
}

/// Formatting picked by the Format Painter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PickedFormat {
    pub run: RunProps,
    /// Paragraph formatting, picked when the source was a caret or whole paragraphs.
    pub para: Option<ParaProps>,
}

impl PageSetupSpec {
    /// Clamp to sane values: at least 1 inch of text area each way, margins ≥ 0.
    fn apply(self, page: &mut PageSetup, cols: &mut Columns) {
        let w = self.width.clamp(1440, 31_680);
        let h = self.height.clamp(1440, 31_680);
        let ml = self.margin_left.clamp(0, w - 720);
        let mr = self.margin_right.clamp(0, w - ml - 720);
        let mt = self.margin_top.clamp(0, h - 720);
        let mb = self.margin_bottom.clamp(0, h - mt - 720);
        *page = PageSetup {
            width: Twips(w),
            height: Twips(h),
            margin_top: Twips(mt),
            margin_bottom: Twips(mb),
            margin_left: Twips(ml),
            margin_right: Twips(mr),
            header: Twips(self.header.clamp(0, h / 2)),
            footer: Twips(self.footer.clamp(0, h / 2)),
        };
        *cols = Columns {
            count: self.columns.clamp(1, 45),
            space: Twips(self.column_space.clamp(0, 5760)),
        };
    }
}

/// Header/footer variant (§17.18.36 `ST_HdrFtr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HfVariant {
    #[default]
    Default,
    First,
    Even,
}

/// Blocks a header/footer set holds for a variant.
pub fn hf_slot(set: &bo_model::HeaderFooter, v: HfVariant) -> Option<&Vec<Block>> {
    match v {
        HfVariant::Default => set.default.as_ref(),
        HfVariant::First => set.first.as_ref(),
        HfVariant::Even => set.even.as_ref(),
    }
}

fn hf_slot_mut(set: &mut bo_model::HeaderFooter, v: HfVariant) -> &mut Option<Vec<Block>> {
    match v {
        HfVariant::Default => &mut set.default,
        HfVariant::First => &mut set.first,
        HfVariant::Even => &mut set.even,
    }
}

/// Header/footer content shown for a variant of section `si`: its own or the one
/// inherited from a previous section (§17.10.5).
pub fn effective_hf(
    doc: &bo_model::Document,
    si: usize,
    header: bool,
    v: HfVariant,
) -> Option<Vec<Block>> {
    let n = doc.sections.len();
    if n == 0 {
        return None;
    }
    doc.sections[..=si.min(n - 1)]
        .iter()
        .rev()
        .find_map(|s| hf_slot(if header { &s.headers } else { &s.footers }, v).cloned())
}

/// The section a header/footer variant of section `si` is taken from (§17.10.5):
/// the nearest section at or before `si` defining it, the first section when none
/// does (editing a linked header changes the one it is linked to).
pub fn hf_source(doc: &bo_model::Document, si: usize, header: bool, v: HfVariant) -> usize {
    let n = doc.sections.len();
    if n == 0 {
        return 0;
    }
    (0..=si.min(n - 1))
        .rev()
        .find(|&i| {
            let s = &doc.sections[i];
            hf_slot(if header { &s.headers } else { &s.footers }, v).is_some()
        })
        .unwrap_or(0)
}

/// The variant of section `si` is linked to the previous section (no own reference).
pub fn hf_linked(doc: &bo_model::Document, si: usize, header: bool, v: HfVariant) -> bool {
    si > 0
        && doc.sections.get(si).is_some_and(|s| {
            hf_slot(if header { &s.headers } else { &s.footers }, v).is_none()
        })
}

/// Text of note blocks without the note mark.
fn note_text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|p| {
            let mut s = String::new();
            for r in &p.runs {
                for c in &r.content {
                    if !matches!(c, RunContent::NoteMark) {
                        c.push_text(&mut s);
                    }
                }
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_start()
        .to_owned()
}

/// A note paragraph: the note mark (superscript), a space and `text` (§17.11.13).
fn note_paragraph(doc: &bo_model::Document, endnote: bool, text: &str) -> Paragraph {
    let (pstyle, cstyle) = if endnote {
        ("EndnoteText", "EndnoteReference")
    } else {
        ("FootnoteText", "FootnoteReference")
    };
    let mark = mark_props(doc, cstyle);
    let body = RunProps {
        size: doc.style(pstyle).is_none().then_some(HalfPoints(20)),
        ..RunProps::default()
    };
    let mut runs = vec![Run {
        props: mark,
        content: vec![RunContent::NoteMark],
    }];
    runs.push(Run::text(format!(" {text}"), body));
    Paragraph::new(
        ParaProps {
            style: doc.style(pstyle).map(|_| pstyle.to_owned()),
            ..ParaProps::default()
        },
        runs,
    )
}

/// Properties of a note reference: the reference character style when defined,
/// else direct superscript (§17.3.2.42 vertAlign).
fn mark_props(doc: &bo_model::Document, style: &str) -> RunProps {
    if doc.style(style).is_some() {
        RunProps {
            style: Some(style.to_owned()),
            ..RunProps::default()
        }
    } else {
        RunProps {
            vert_align: Some(VertAlign::Superscript),
            ..RunProps::default()
        }
    }
}

impl Session {
    /// Page setup of the section holding the caret.
    pub fn section_state(&self) -> SectionState {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        let si = paths
            .get(at.para)
            .and_then(|p| p.first().copied())
            .unwrap_or(0);
        let Some(s) = self.doc.sections.get(si) else {
            return SectionState::default();
        };
        let p = &s.page;
        SectionState {
            index: si,
            count: self.doc.sections.len(),
            width: p.width.0,
            height: p.height.0,
            margin_top: p.margin_top.0,
            margin_bottom: p.margin_bottom.0,
            margin_left: p.margin_left.0,
            margin_right: p.margin_right.0,
            header: p.header.0,
            footer: p.footer.0,
            columns: s.columns.count,
            column_space: s.columns.space.0,
            title_page: s.title_page,
            even_and_odd: self.doc.even_and_odd_headers,
        }
    }

    /// Apply a page setup to the caret's section or the whole document (one undo step).
    pub fn set_page_setup(&mut self, spec: PageSetupSpec, scope: SetupScope) -> Result<()> {
        let current = self.section_state().index;
        let targets: Vec<usize> = match scope {
            SetupScope::Section => vec![current],
            SetupScope::Document => (0..self.doc.sections.len()).collect(),
        };
        self.edit(|tx, sel| {
            for si in targets {
                let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
                let mut new = old.clone();
                spec.apply(&mut new.page, &mut new.columns);
                if new != old {
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![new],
                    })?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// "Different first page" (§17.10.6 titlePg) for section `si` and "different odd &
    /// even pages" (§17.15.1.35 evenAndOddHeaders, document-wide).
    pub fn set_header_options(
        &mut self,
        si: usize,
        title_page: Option<bool>,
        even_and_odd: Option<bool>,
    ) -> Result<()> {
        self.edit(|tx, sel| {
            if let Some(t) = title_page {
                let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
                if old.title_page != t {
                    let mut new = old.clone();
                    new.title_page = t;
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![new],
                    })?;
                }
            }
            if let Some(e) = even_and_odd {
                if tx.doc.even_and_odd_headers != e {
                    tx.push(Op::EvenAndOddHeaders {
                        old: tx.doc.even_and_odd_headers,
                        new: e,
                    })?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// Replace a header (`header`) or footer of section `si` (one undo step).
    pub fn set_header_footer(
        &mut self,
        si: usize,
        header: bool,
        v: HfVariant,
        blocks: Vec<Block>,
    ) -> Result<()> {
        self.edit(|tx, sel| {
            let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
            let mut new = old.clone();
            let set = if header {
                &mut new.headers
            } else {
                &mut new.footers
            };
            *hf_slot_mut(set, v) = Some(blocks);
            if new != old {
                tx.push(Op::Sections {
                    index: si,
                    remove: vec![old],
                    insert: vec![new],
                })?;
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// "Link to previous" for a header/footer variant of section `si` (one undo step).
    ///
    /// ECMA-376 Part 1, §17.10.5 headerReference: a section without a reference of a
    /// type inherits that type from the previous section. Linking removes the
    /// section's own reference; unlinking gives it a copy of the inherited content.
    /// The first section has no previous one.
    pub fn set_header_link(
        &mut self,
        si: usize,
        header: bool,
        v: HfVariant,
        linked: bool,
    ) -> Result<()> {
        if si == 0 {
            return Err(EditError::Unsupported(
                "the first section has no previous section".into(),
            ));
        }
        let content = if linked {
            None
        } else {
            let mut b = effective_hf(&self.doc, si, header, v).unwrap_or_default();
            if b.is_empty() {
                b.push(Block::Paragraph(Paragraph::default()));
            }
            Some(b)
        };
        self.edit(|tx, sel| {
            let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
            let mut new = old.clone();
            let set = if header {
                &mut new.headers
            } else {
                &mut new.footers
            };
            let slot = hf_slot_mut(set, v);
            if slot.is_some() != content.is_some() {
                *slot = content;
            }
            if new != old {
                tx.push(Op::Sections {
                    index: si,
                    remove: vec![old],
                    insert: vec![new],
                })?;
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// Show `blocks` as a header/footer without recording history (live preview while
    /// the header is edited; [`Session::set_header_footer`] records the final change).
    pub fn preview_header_footer(
        &mut self,
        si: usize,
        header: bool,
        v: HfVariant,
        blocks: Option<Vec<Block>>,
    ) {
        if let Some(s) = self.doc.sections.get_mut(si) {
            let set = if header {
                &mut s.headers
            } else {
                &mut s.footers
            };
            *hf_slot_mut(set, v) = blocks;
            self.version += 1;
            // Not an op: the incremental layout must redo everything.
            self.changes.mark_all();
        }
    }

    // -------------------------------------------------------------- notes

    /// Footnotes and endnotes in reference order.
    pub fn notes(&self) -> Vec<NoteInfo> {
        let paths = body_paragraphs(&self.doc);
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (id, path) in paths.iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            for r in &p.runs {
                for c in &r.content {
                    let (end, nid) = match c {
                        RunContent::FootnoteRef(n) => (false, *n),
                        RunContent::EndnoteRef(n) => (true, *n),
                        _ => continue,
                    };
                    let map = if end {
                        &self.doc.endnotes
                    } else {
                        &self.doc.footnotes
                    };
                    if let Some(b) = map.get(&nid) {
                        if seen.insert((end, nid)) {
                            out.push(NoteInfo {
                                id: nid,
                                endnote: end,
                                text: note_text(b),
                                para: Some(id),
                            });
                        }
                    }
                }
            }
        }
        out
    }

    /// Insert a footnote (or endnote) reference at the caret with a new note holding
    /// `text`; returns the note id.
    pub fn insert_note(&mut self, endnote: bool, text: &str) -> Result<i32> {
        let map = if endnote {
            &self.doc.endnotes
        } else {
            &self.doc.footnotes
        };
        // Ids -1/0 (and 1 in older files) are separators (§17.11.23).
        let id = map.keys().copied().max().unwrap_or(0).max(1) + 1;
        let mut new_map = map.clone();
        new_map.insert(
            id,
            vec![Block::Paragraph(note_paragraph(&self.doc, endnote, text))],
        );
        let old_map = map.clone();
        let mark = mark_props(
            &self.doc,
            if endnote {
                "EndnoteReference"
            } else {
                "FootnoteReference"
            },
        );
        self.edit(|tx, sel| {
            tx.push(Op::Notes {
                endnote,
                old: old_map,
                new: new_map,
            })?;
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let base = para::props_at(&atoms, at.offset);
            let props = RunProps {
                style: mark.style.clone(),
                vert_align: mark.vert_align,
                ..base
            };
            let c = if endnote {
                RunContent::EndnoteRef(id)
            } else {
                RunContent::FootnoteRef(id)
            };
            Ok(Selection::caret(tx.insert_atoms(at, vec![(props, c)])?))
        })?;
        Ok(id)
    }

    /// Replace the text of a note (one paragraph per line, the note mark is kept).
    pub fn set_note_text(&mut self, endnote: bool, id: i32, text: &str) -> Result<()> {
        let map = if endnote {
            &self.doc.endnotes
        } else {
            &self.doc.footnotes
        };
        let Some(old) = map.get(&id) else {
            return Err(EditError::Unsupported(format!("no note {id}")));
        };
        if note_text(old) == text {
            return Ok(());
        }
        // Keep the first paragraph's properties and the note mark's run properties.
        let first = old.iter().find_map(Block::as_paragraph);
        let mut lines = text.split('\n');
        let mut blocks = Vec::new();
        let head = note_paragraph(&self.doc, endnote, lines.next().unwrap_or(""));
        let head = match first {
            Some(p) => {
                let mark = p
                    .runs
                    .iter()
                    .find(|r| r.content.contains(&RunContent::NoteMark))
                    .map(|r| r.props.clone());
                let body = p
                    .runs
                    .iter()
                    .find(|r| r.content.iter().any(|c| matches!(c, RunContent::Text(_))))
                    .map(|r| r.props.clone());
                let mut q = head;
                q.props = ParaProps {
                    unknown: Vec::new(),
                    ..p.props.clone()
                };
                if let (Some(m), Some(r)) = (mark, q.runs.first_mut()) {
                    r.props = m;
                }
                if let (Some(b), Some(r)) = (body, q.runs.get_mut(1)) {
                    r.props = b;
                }
                q
            }
            None => head,
        };
        let props = head.props.clone();
        let body = head
            .runs
            .get(1)
            .map(|r| r.props.clone())
            .unwrap_or_default();
        blocks.push(Block::Paragraph(head));
        for l in lines {
            blocks.push(Block::Paragraph(Paragraph::new(
                props.clone(),
                vec![Run::text(l, body.clone())],
            )));
        }
        let mut new_map = map.clone();
        new_map.insert(id, blocks);
        let old_map = map.clone();
        self.edit(|tx, sel| {
            tx.push(Op::Notes {
                endnote,
                old: old_map,
                new: new_map,
            })?;
            Ok(sel)
        })?;
        Ok(())
    }

    // -------------------------------------------------------------- clear / painter

    /// Clear Formatting: remove direct character formatting of the selection and the
    /// paragraph formatting and style of the touched paragraphs (one undo step).
    pub fn clear_formatting(&mut self) -> Result<Selection> {
        let sel = self.selection;
        let ranges = self.selected_ranges(sel);
        let ids: Vec<usize> = ranges.iter().map(|r| r.0).collect();
        self.edit(|tx, sel| {
            for (id, s, e) in ranges {
                let (path, p, mut atoms) = tx.atoms(id)?;
                if s < e {
                    para::format(&mut atoms, s, e, &|r| {
                        let link = r.link.take();
                        let (ins, del) = (r.ins.take(), r.del.take());
                        *r = RunProps {
                            link,
                            ins,
                            del,
                            ..RunProps::default()
                        };
                    });
                }
                let mut q = para::with_atoms(&p, atoms);
                if ids.contains(&id) {
                    let pr = &mut q.props;
                    *pr = ParaProps {
                        unknown: std::mem::take(&mut pr.unknown),
                        mark_ins: pr.mark_ins.take(),
                        mark_del: pr.mark_del.take(),
                        change: pr.change.take(),
                        num: pr.num.take(),
                        ..ParaProps::default()
                    };
                }
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }

    /// Format Painter: remember the formatting at the selection start. Paragraph
    /// formatting is included when nothing or whole paragraphs are selected.
    pub fn pick_format(&mut self) -> PickedFormat {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let Some(p) = paths.get(a.para).and_then(|path| para_at(&self.doc, path)) else {
            return PickedFormat::default();
        };
        let atoms = para::flatten(p);
        let probe = if a == b { a.offset } else { a.offset + 1 };
        let run = para::props_at(&atoms, probe).without_revisions();
        let run = RunProps { link: None, ..run };
        let whole = a == b
            || (a.offset == 0 && self.para_text(b.para).is_some_and(|t| b.offset >= t.len()));
        let picked = PickedFormat {
            run,
            para: whole.then(|| ParaProps {
                unknown: Vec::new(),
                mark_ins: None,
                mark_del: None,
                change: None,
                ..p.props.clone()
            }),
        };
        self.painter = Some(picked.clone());
        picked
    }

    /// The picked Format Painter formatting, if any.
    pub fn picked_format(&self) -> Option<&PickedFormat> {
        self.painter.as_ref()
    }

    /// Forget the Format Painter formatting.
    pub fn clear_picked_format(&mut self) {
        self.painter = None;
    }

    /// Apply the picked formatting to the selection (the word at the caret when
    /// collapsed); one undo step.
    pub fn apply_picked_format(&mut self) -> Result<Selection> {
        let Some(picked) = self.painter.clone() else {
            return Ok(self.selection);
        };
        if self.selection.is_collapsed() {
            let w = self.word_at(self.selection.focus);
            self.set_selection(w);
        }
        let sel = self.selection;
        let ranges = self.selected_ranges(sel);
        self.edit(|tx, sel| {
            for (id, s, e) in ranges {
                let (path, p, mut atoms) = tx.atoms(id)?;
                if s < e {
                    para::format(&mut atoms, s, e, &|r| {
                        let keep = (r.link.take(), r.ins.take(), r.del.take());
                        *r = picked.run.clone();
                        (r.link, r.ins, r.del) = keep;
                    });
                }
                let mut q = para::with_atoms(&p, atoms);
                if let Some(pp) = &picked.para {
                    q.props = ParaProps {
                        unknown: q.props.unknown.clone(),
                        mark_ins: q.props.mark_ins.clone(),
                        mark_del: q.props.mark_del.clone(),
                        change: q.props.change.clone(),
                        ..pp.clone()
                    };
                }
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }

    // -------------------------------------------------------------- paste special

    /// Paste with a Paste Special mode (`internal` wins over `html` over `text` for
    /// `Keep` and `Merge`).
    pub fn paste_with(
        &mut self,
        mode: PasteMode,
        internal: Option<&str>,
        html_src: Option<&str>,
        text: Option<&str>,
    ) -> Result<Selection> {
        let internal = internal.filter(|s| !s.is_empty());
        let html_src = html_src.filter(|s| !s.is_empty());
        if mode == PasteMode::Text {
            let t = match (text.filter(|t| !t.is_empty()), html_src, internal) {
                (Some(t), _, _) => t.to_owned(),
                (None, Some(h), _) => blocks_plain(&html::from_html(h)),
                (None, None, Some(j)) => blocks_plain(
                    &serde_json::from_str::<Vec<Block>>(j)
                        .map_err(|e| EditError::Clipboard(e.to_string()))?,
                ),
                _ => String::new(),
            };
            return self.paste_text(&t);
        }
        let blocks: Vec<Block> = match (internal, html_src) {
            (Some(j), _) => {
                serde_json::from_str(j).map_err(|e| EditError::Clipboard(e.to_string()))?
            }
            (None, Some(h)) => html::from_html(h),
            (None, None) => return self.paste_text(text.unwrap_or("")),
        };
        if mode == PasteMode::Keep {
            return self.paste_blocks(blocks);
        }
        // Merge: destination formatting with the source's emphasis (§17.3.2.1 b, i, u, strike, vertAlign).
        let dest = self.caret_direct_props();
        let doc = &self.doc;
        let blocks = blocks
            .into_iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => {
                    let atoms: Vec<Atom> = para::flatten(&p)
                        .into_iter()
                        .map(|(r, c)| {
                            let src = effective_run(doc, &p, &r);
                            let mut n = dest.clone();
                            n.bold = src.bold.filter(|b| *b).or(n.bold);
                            n.italic = src.italic.filter(|b| *b).or(n.italic);
                            n.underline = src
                                .underline
                                .filter(|u| *u != Underline::None)
                                .or(n.underline);
                            n.strike = src.strike.filter(|b| *b).or(n.strike);
                            n.vert_align = src
                                .vert_align
                                .filter(|v| *v != VertAlign::Baseline)
                                .or(n.vert_align);
                            (n, c)
                        })
                        .collect();
                    Some(Block::Paragraph(Paragraph::new(
                        ParaProps::default(),
                        para::rebuild(atoms),
                    )))
                }
                _ => None,
            })
            .collect();
        self.paste_blocks_inner(blocks, true)
    }
}

/// Plain text of blocks (paragraphs joined by `\n`).
fn blocks_plain(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(Paragraph::plain_text)
        .collect::<Vec<_>>()
        .join("\n")
        .replace('\u{FFFC}', "")
}

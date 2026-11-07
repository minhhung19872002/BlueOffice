//! Editing for BlueDoc: positions and selection, transactions of invertible
//! [`Op`]s with unlimited undo/redo, text input and deletion, paragraph
//! split/merge, character/paragraph formatting, styles, lists, insertion of
//! tables, pictures, page and section breaks, find/replace and clipboard
//! fragments (internal JSON, HTML, plain text).
//!
//! Positions are `(body paragraph id, byte offset into its index text)`; the id
//! is the paragraph's index in [`bo_model::index::body_paragraphs`] (document
//! order, table cells included) and matches `bo_layout::LineSource::Body`.
#![forbid(unsafe_code)]

pub mod find;
pub mod html;
mod ops;
pub mod para;

pub use find::{find, FindOptions, Match};
pub use ops::{Op, Pos, Selection, Transaction};

use bo_model::index::{blocks_at, body_paragraphs, para_at, ParaPath};
use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, Image, LineSpacing, NumRef, ParaProps,
    Paragraph, RunContent, RunProps, Section, SectionBreak, StyleKind, Table, Twips, Underline,
    VertAlign,
};
use para::Atom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// Editing errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("position does not address a paragraph")]
    BadPath,
    #[error("operation does not match the document (stale edit)")]
    Conflict,
    #[error("invalid position")]
    BadPosition,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid clipboard data: {0}")]
    Clipboard(String),
    #[error("invalid regular expression: {0}")]
    Regex(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, EditError>;

/// Character formatting commands (ECMA-376 Part 1, §17.3.2 run properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CharFormat {
    Bold,
    Italic,
    Underline,
    Strike,
    Superscript,
    Subscript,
    Font {
        name: String,
    },
    /// Size in half-points (§17.3.2.38 sz).
    Size {
        half_points: u32,
    },
    /// `RRGGBB` or `None` for automatic.
    Color {
        hex: Option<String>,
    },
    /// `RRGGBB` or `None` to remove (§17.3.2.15 highlight).
    Highlight {
        hex: Option<String>,
    },
    /// Remove direct character formatting.
    Clear,
}

/// Paragraph formatting commands (ECMA-376 Part 1, §17.3.1 paragraph properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ParaFormat {
    Align {
        align: Alignment,
    },
    IndentLeft {
        twips: i32,
    },
    IndentFirstLine {
        twips: i32,
    },
    IncreaseIndent,
    DecreaseIndent,
    /// Line spacing in 240ths of a line (240 = single).
    LineSpacing {
        value: u32,
    },
    SpaceBefore {
        twips: i32,
    },
    SpaceAfter {
        twips: i32,
    },
    Bullets,
    Numbering,
    Style {
        id: String,
    },
    /// Remove direct paragraph formatting and the paragraph style.
    Clear,
}

/// Formatting at the selection start (drives ribbon toggle states).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub font: Option<String>,
    pub size_half_points: Option<u32>,
    pub color: Option<String>,
    pub highlight: Option<String>,
    pub align: Alignment,
    pub style: Option<String>,
    pub bullets: bool,
    pub numbering: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// Content copied to the clipboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub text: String,
    pub html: String,
    /// Lossless internal format (JSON of model blocks).
    pub internal: String,
}

/// Mime type used for the internal clipboard format.
pub const INTERNAL_MIME: &str = "application/x-blueoffice-blocks+json";

// ------------------------------------------------------------------ helpers

fn path_of(doc: &Document, paths: &[ParaPath], id: usize) -> Result<ParaPath> {
    let p = paths.get(id).ok_or(EditError::BadPosition)?;
    para_at(doc, p).ok_or(EditError::BadPath)?;
    Ok(p.clone())
}

fn para_clone(doc: &Document, path: &[usize]) -> Result<Paragraph> {
    para_at(doc, path).cloned().ok_or(EditError::BadPath)
}

fn split_path(path: &[usize]) -> (Vec<usize>, usize) {
    let (last, prefix) = path.split_last().map_or((0, &[][..]), |(l, p)| (*l, p));
    (prefix.to_vec(), last)
}

fn id_of(doc: &Document, path: &[usize]) -> Option<usize> {
    body_paragraphs(doc).iter().position(|p| p == path)
}

/// Index-text length of a paragraph.
pub fn para_len(p: &Paragraph) -> usize {
    para::len(&para::flatten(p))
}

/// Previous grapheme boundary before `off` in `text` (0 when none).
fn prev_boundary(text: &str, off: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|&i| i < off)
        .last()
        .unwrap_or(0)
}

/// Next grapheme boundary after `off`.
fn next_boundary(text: &str, off: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, g)| i + g.len())
        .find(|&e| e > off)
        .unwrap_or(text.len())
}

/// Effective run properties: defaults → paragraph style → character style → direct (§17.7.2).
pub fn effective_run(doc: &Document, p: &Paragraph, direct: &RunProps) -> RunProps {
    let mut r = doc.default_run.clone();
    let style = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    if let Some(id) = style {
        r = doc.styles.resolve_paragraph(&id).1.inherit(&r);
    }
    if let Some(id) = &direct.style {
        r = doc.styles.resolve_character(id).inherit(&r);
    }
    direct.inherit(&r)
}

/// Transaction builder: ops are applied as they are pushed.
struct Tx<'a> {
    doc: &'a mut Document,
    ops: Vec<Op>,
}

impl Tx<'_> {
    fn push(&mut self, op: Op) -> Result<()> {
        op.apply(self.doc)?;
        self.ops.push(op);
        Ok(())
    }

    fn paths(&self) -> Vec<ParaPath> {
        body_paragraphs(self.doc)
    }

    fn replace_para(&mut self, path: &[usize], new: Paragraph) -> Result<()> {
        let old = para_clone(self.doc, path)?;
        if old == new {
            return Ok(());
        }
        let (prefix, index) = split_path(path);
        self.push(Op::Blocks {
            path: prefix,
            index,
            remove: vec![Block::Paragraph(old)],
            insert: vec![Block::Paragraph(new)],
        })
    }

    fn atoms(&self, id: usize) -> Result<(ParaPath, Paragraph, Vec<Atom>)> {
        let paths = self.paths();
        let path = path_of(self.doc, &paths, id)?;
        let p = para_clone(self.doc, &path)?;
        let a = para::flatten(&p);
        Ok((path, p, a))
    }

    /// Delete `a..b` (document order). Returns the caret.
    fn delete_range(&mut self, a: Pos, b: Pos) -> Result<Pos> {
        if a >= b {
            return Ok(a.min(b));
        }
        let paths = self.paths();
        let pa = path_of(self.doc, &paths, a.para)?;
        let pb = path_of(self.doc, &paths, b.para)?;
        if a.para == b.para {
            let p = para_clone(self.doc, &pa)?;
            let mut atoms = para::flatten(&p);
            para::delete(&mut atoms, a.offset, b.offset);
            self.replace_para(&pa, para::with_atoms(&p, atoms))?;
            return Ok(a);
        }
        let (ca, ia) = split_path(&pa);
        let (cb, ib) = split_path(&pb);
        if ca == cb && ia < ib {
            let p = para_clone(self.doc, &pa)?;
            let q = para_clone(self.doc, &pb)?;
            let mut left = para::flatten(&p);
            let l = para::len(&left);
            para::delete(&mut left, a.offset, l);
            let mut right = para::flatten(&q);
            para::delete(&mut right, 0, b.offset);
            left.extend(right);
            let merged = para::with_atoms(&p, left);
            let blocks = blocks_at(self.doc, &ca).ok_or(EditError::BadPath)?;
            let remove = blocks[ia..=ib].to_vec();
            self.push(Op::Blocks {
                path: ca,
                index: ia,
                remove,
                insert: vec![Block::Paragraph(merged)],
            })?;
            return Ok(a);
        }
        // Range across containers (e.g. table cells): clear the text, keep the structure.
        for id in a.para..=b.para {
            let path = path_of(self.doc, &paths, id)?;
            let p = para_clone(self.doc, &path)?;
            let mut atoms = para::flatten(&p);
            let l = para::len(&atoms);
            let s = if id == a.para { a.offset } else { 0 };
            let e = if id == b.para { b.offset } else { l };
            para::delete(&mut atoms, s, e);
            self.replace_para(&path, para::with_atoms(&p, atoms))?;
        }
        Ok(a)
    }

    fn insert_atoms(&mut self, at: Pos, atoms: Vec<Atom>) -> Result<Pos> {
        let (path, p, mut a) = self.atoms(at.para)?;
        let n = para::len(&atoms);
        para::insert(&mut a, at.offset, atoms);
        self.replace_para(&path, para::with_atoms(&p, a))?;
        Ok(Pos::new(at.para, at.offset + n))
    }

    /// Split the paragraph at `at`; returns the start of the new (second) paragraph.
    fn split(&mut self, at: Pos) -> Result<Pos> {
        let (path, p, mut left) = self.atoms(at.para)?;
        let total = para::len(&left);
        let i = para::split_at(&mut left, at.offset);
        let right = left.split_off(i);
        let mut right_props = p.props.clone();
        // §17.7.4.10 next: a new paragraph after a heading gets the style's "next" style.
        if at.offset >= total {
            if let Some(next) = p
                .props
                .style
                .as_ref()
                .and_then(|id| self.doc.style(id))
                .and_then(|s| s.next.clone())
            {
                right_props.style = Some(next);
            }
        }
        right_props.unknown.clear();
        let first = para::with_atoms(&p, left);
        let second = Paragraph {
            props: right_props,
            runs: para::rebuild(right),
            src: None,
        };
        let (prefix, index) = split_path(&path);
        self.push(Op::Blocks {
            path: prefix,
            index,
            remove: vec![Block::Paragraph(p)],
            insert: vec![Block::Paragraph(first), Block::Paragraph(second)],
        })?;
        Ok(Pos::new(at.para + 1, 0))
    }

    fn insert_blocks(
        &mut self,
        container: Vec<usize>,
        index: usize,
        blocks: Vec<Block>,
    ) -> Result<()> {
        self.push(Op::Blocks {
            path: container,
            index,
            remove: Vec::new(),
            insert: blocks,
        })
    }
}

// ------------------------------------------------------------------ session

/// An editing session: the document, its selection and the undo history.
#[derive(Debug, Clone)]
pub struct Session {
    doc: Document,
    selection: Selection,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    /// Formatting applied to the next typed text (collapsed-selection formatting).
    pending: Option<RunProps>,
    typing: bool,
}

impl Session {
    /// Start editing `doc` with the caret at the beginning.
    pub fn new(doc: Document) -> Session {
        Session {
            doc,
            selection: Selection::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            pending: None,
            typing: false,
        }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// Consume the session and return the document.
    pub fn into_doc(self) -> Document {
        self.doc
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of body paragraphs.
    pub fn paragraph_count(&self) -> usize {
        body_paragraphs(&self.doc).len()
    }

    /// Clamp a position to an existing paragraph and a character boundary.
    pub fn clamp(&self, p: Pos) -> Pos {
        let paths = body_paragraphs(&self.doc);
        if paths.is_empty() {
            return Pos::default();
        }
        let para = p.para.min(paths.len() - 1);
        let text = para_at(&self.doc, &paths[para])
            .map(bo_model::index::para_index_text)
            .unwrap_or_default();
        let mut off = p.offset.min(text.len());
        while off > 0 && !text.is_char_boundary(off) {
            off -= 1;
        }
        Pos::new(para, off)
    }

    /// Set the selection (clamped).
    pub fn set_selection(&mut self, sel: Selection) {
        let s = Selection {
            anchor: self.clamp(sel.anchor),
            focus: self.clamp(sel.focus),
        };
        if s != self.selection {
            self.pending = None;
            self.typing = false;
        }
        self.selection = s;
    }

    /// Index text of a body paragraph.
    pub fn para_text(&self, id: usize) -> Option<String> {
        let paths = body_paragraphs(&self.doc);
        para_at(&self.doc, paths.get(id)?).map(bo_model::index::para_index_text)
    }

    fn edit(
        &mut self,
        f: impl FnOnce(&mut Tx<'_>, Selection) -> Result<Selection>,
    ) -> Result<Selection> {
        let before = self.selection;
        let mut tx = Tx {
            doc: &mut self.doc,
            ops: Vec::new(),
        };
        match f(&mut tx, before) {
            Ok(after) => {
                let ops = tx.ops;
                if !ops.is_empty() {
                    self.undo.push(Transaction { ops, before, after });
                    self.redo.clear();
                }
                self.selection = after;
                self.typing = false;
                Ok(after)
            }
            Err(e) => {
                for op in tx.ops.iter().rev() {
                    let _ = op.inverse().apply(tx.doc);
                }
                Err(e)
            }
        }
    }

    /// Undo the last transaction.
    pub fn undo(&mut self) -> bool {
        let Some(t) = self.undo.pop() else {
            return false;
        };
        if t.inverse().apply(&mut self.doc).is_err() {
            return false;
        }
        self.selection = t.before;
        self.redo.push(t);
        self.typing = false;
        true
    }

    /// Redo the last undone transaction.
    pub fn redo(&mut self) -> bool {
        let Some(t) = self.redo.pop() else {
            return false;
        };
        if t.apply(&mut self.doc).is_err() {
            return false;
        }
        self.selection = t.after;
        self.undo.push(t);
        self.typing = false;
        true
    }

    // -------------------------------------------------------------- text

    /// Type text at the selection. `\n` splits paragraphs; consecutive typing is one undo step per word.
    pub fn insert_text(&mut self, text: &str) -> Result<Selection> {
        if text.is_empty() {
            return Ok(self.selection);
        }
        let pending = self.pending.take();
        let mergeable = self.typing
            && self.selection.is_collapsed()
            && !text.contains('\n')
            && text.chars().count() == 1;
        let prev_after = self.undo.last().map(|t| t.after);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let result = self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = pending.unwrap_or_else(|| para::props_at(&atoms, at.offset));
            let mut caret = at;
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    caret = tx.split(caret)?;
                }
                caret = tx.insert_atoms(caret, para::text_atoms(line, &props))?;
            }
            Ok(Selection::caret(caret))
        })?;
        // Group single-character typing into one undo step per word.
        if mergeable
            && self.undo.len() >= 2
            && prev_after == Some(self.undo[self.undo.len() - 1].before)
        {
            let last = self.undo.pop();
            if let (Some(last), Some(prev)) = (last, self.undo.last_mut()) {
                prev.ops.extend(last.ops);
                prev.after = last.after;
            }
        }
        self.typing = !text.ends_with(char::is_whitespace);
        Ok(result)
    }

    /// Backspace.
    pub fn delete_backward(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            if a != b {
                return Ok(Selection::caret(tx.delete_range(a, b)?));
            }
            if a.offset > 0 {
                let (_, p, _) = tx.atoms(a.para)?;
                let text = bo_model::index::para_index_text(&p);
                let s = prev_boundary(&text, a.offset);
                return Ok(Selection::caret(tx.delete_range(Pos::new(a.para, s), a)?));
            }
            let (path, p, atoms) = tx.atoms(a.para)?;
            // Backspace at the start of a list item removes the numbering first.
            if p.props.num.is_some_and(|n| n.num_id != 0) {
                let mut q = p.clone();
                q.props.num = None;
                tx.replace_para(&path, q)?;
                return Ok(sel);
            }
            let (prefix, index) = split_path(&path);
            if index == 0 {
                return Ok(sel);
            }
            let blocks = blocks_at(tx.doc, &prefix).ok_or(EditError::BadPath)?;
            let Some(Block::Paragraph(prev)) = blocks.get(index - 1).cloned() else {
                return Ok(sel);
            };
            let mut merged = para::flatten(&prev);
            let caret = para::len(&merged);
            merged.extend(atoms);
            tx.push(Op::Blocks {
                path: prefix,
                index: index - 1,
                remove: vec![Block::Paragraph(prev.clone()), Block::Paragraph(p)],
                insert: vec![Block::Paragraph(para::with_atoms(&prev, merged))],
            })?;
            Ok(Selection::caret(Pos::new(a.para - 1, caret)))
        })
    }

    /// Delete (forward).
    pub fn delete_forward(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            if a != b {
                return Ok(Selection::caret(tx.delete_range(a, b)?));
            }
            let (path, p, atoms) = tx.atoms(a.para)?;
            let len = para::len(&atoms);
            if a.offset < len {
                let text = bo_model::index::para_index_text(&p);
                let e = next_boundary(&text, a.offset);
                return Ok(Selection::caret(tx.delete_range(a, Pos::new(a.para, e))?));
            }
            let (prefix, index) = split_path(&path);
            let blocks = blocks_at(tx.doc, &prefix).ok_or(EditError::BadPath)?;
            let Some(Block::Paragraph(next)) = blocks.get(index + 1).cloned() else {
                return Ok(sel);
            };
            let mut merged = atoms;
            merged.extend(para::flatten(&next));
            tx.push(Op::Blocks {
                path: prefix,
                index,
                remove: vec![Block::Paragraph(p.clone()), Block::Paragraph(next)],
                insert: vec![Block::Paragraph(para::with_atoms(&p, merged))],
            })?;
            Ok(sel)
        })
    }

    /// Enter: split the paragraph (an empty list item leaves the list instead).
    pub fn split_paragraph(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (path, p, atoms) = tx.atoms(at.para)?;
            if para::len(&atoms) == 0 && p.props.num.is_some_and(|n| n.num_id != 0) {
                let mut q = p.clone();
                q.props.num = None;
                tx.replace_para(&path, q)?;
                return Ok(Selection::caret(at));
            }
            Ok(Selection::caret(tx.split(at)?))
        })
    }

    // -------------------------------------------------------------- formatting

    fn selected_ranges(&self, sel: Selection) -> Vec<(usize, usize, usize)> {
        let (a, b) = sel.ordered();
        (a.para..=b.para)
            .map(|id| {
                let len = self.para_text(id).map_or(0, |t| t.len());
                let s = if id == a.para { a.offset } else { 0 };
                let e = if id == b.para { b.offset } else { len };
                (id, s, e)
            })
            .collect()
    }

    fn atoms_in_selection(&self) -> Vec<(Paragraph, RunProps)> {
        let mut out = Vec::new();
        let paths = body_paragraphs(&self.doc);
        for (id, s, e) in self.selected_ranges(self.selection) {
            let Some(p) = paths.get(id).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let atoms = para::flatten(p);
            for (props, _) in para::slice(&atoms, s, e) {
                out.push((p.clone(), props));
            }
        }
        out
    }

    fn all_effective(&self, f: impl Fn(&RunProps) -> bool) -> bool {
        let atoms = self.atoms_in_selection();
        !atoms.is_empty()
            && atoms
                .iter()
                .all(|(p, r)| f(&effective_run(&self.doc, p, r)))
    }

    /// Apply a character format to the selection (or to the next typed text when collapsed).
    pub fn format_text(&mut self, f: &CharFormat) -> Result<Selection> {
        let sel = self.selection;
        let caret_props = self.caret_props();
        let on = |get: fn(&RunProps) -> bool| {
            if sel.is_collapsed() {
                get(&caret_props)
            } else {
                self.all_effective(get)
            }
        };
        let change: Box<dyn Fn(&mut RunProps)> = match f {
            CharFormat::Bold => {
                let v = !on(|r| r.bold.unwrap_or(false));
                Box::new(move |r| r.bold = Some(v))
            }
            CharFormat::Italic => {
                let v = !on(|r| r.italic.unwrap_or(false));
                Box::new(move |r| r.italic = Some(v))
            }
            CharFormat::Underline => {
                let v = !on(|r| r.underline.is_some_and(|u| u != Underline::None));
                Box::new(move |r| {
                    r.underline = Some(if v {
                        Underline::Single
                    } else {
                        Underline::None
                    });
                })
            }
            CharFormat::Strike => {
                let v = !on(|r| r.strike.unwrap_or(false));
                Box::new(move |r| r.strike = Some(v))
            }
            CharFormat::Superscript | CharFormat::Subscript => {
                let want = if *f == CharFormat::Superscript {
                    VertAlign::Superscript
                } else {
                    VertAlign::Subscript
                };
                let is = if sel.is_collapsed() {
                    caret_props.vert_align == Some(want)
                } else {
                    self.all_effective(|r| r.vert_align == Some(want))
                };
                Box::new(move |r| {
                    r.vert_align = Some(if is { VertAlign::Baseline } else { want });
                })
            }
            CharFormat::Font { name } => {
                let n = name.clone();
                Box::new(move |r| {
                    r.font = Some(n.clone());
                    r.font_east_asia = None;
                    r.font_cs = None;
                })
            }
            CharFormat::Size { half_points } => {
                let v = (*half_points).clamp(2, 3276);
                Box::new(move |r| r.size = Some(HalfPoints(v)))
            }
            CharFormat::Color { hex } => {
                let c = hex.as_deref().and_then(Color::from_hex);
                Box::new(move |r| r.color = c)
            }
            CharFormat::Highlight { hex } => {
                let c = hex.as_deref().and_then(Color::from_hex);
                Box::new(move |r| r.highlight = c)
            }
            CharFormat::Clear => Box::new(|r| {
                let link = r.link.take();
                *r = RunProps {
                    link,
                    ..RunProps::default()
                };
            }),
        };
        if sel.is_collapsed() {
            let mut p = self
                .pending
                .clone()
                .unwrap_or_else(|| self.caret_direct_props());
            change(&mut p);
            self.pending = Some(p);
            return Ok(sel);
        }
        let ranges = self.selected_ranges(sel);
        self.edit(|tx, sel| {
            for (id, s, e) in ranges {
                if s >= e {
                    continue;
                }
                let (path, p, mut atoms) = tx.atoms(id)?;
                para::format(&mut atoms, s, e, &*change);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })
    }

    fn caret_direct_props(&self) -> RunProps {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        paths
            .get(at.para)
            .and_then(|p| para_at(&self.doc, p))
            .map(|p| para::props_at(&para::flatten(p), at.offset))
            .unwrap_or_default()
    }

    fn caret_props(&self) -> RunProps {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        let Some(p) = paths.get(at.para).and_then(|p| para_at(&self.doc, p)) else {
            return self.doc.default_run.clone();
        };
        let direct = self
            .pending
            .clone()
            .unwrap_or_else(|| para::props_at(&para::flatten(p), at.offset));
        effective_run(&self.doc, p, &direct)
    }

    /// Apply a paragraph format to every paragraph touched by the selection.
    pub fn format_paragraphs(&mut self, f: &ParaFormat) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let ids: Vec<usize> = (a.para..=b.para).collect();
        let first = self.para_props(a.para);
        let list_kind = |bullet: bool, doc: &Document, props: &ParaProps| {
            props.num.is_some_and(|n| {
                doc.numbering
                    .level(n)
                    .is_some_and(|l| (l.format == bo_model::NumFormat::Bullet) == bullet)
            })
        };
        let f = f.clone();
        self.edit(move |tx, sel| {
            // Lists need a numbering definition first (§17.9 numbering part).
            let mut list_num: Option<Option<NumRef>> = None;
            if let ParaFormat::Bullets | ParaFormat::Numbering = f {
                let bullet = f == ParaFormat::Bullets;
                if list_kind(bullet, tx.doc, &first) {
                    list_num = Some(None);
                } else {
                    let old = tx.doc.numbering.clone();
                    let mut new = old.clone();
                    let id = new.add_simple_list(bullet);
                    tx.push(Op::Numbering {
                        old: Box::new(old),
                        new: Box::new(new),
                    })?;
                    list_num = Some(Some(NumRef {
                        num_id: id,
                        ilvl: 0,
                    }));
                }
            }
            for id in ids {
                let (path, p, _) = tx.atoms(id)?;
                let mut q = p.clone();
                let pr = &mut q.props;
                match &f {
                    ParaFormat::Align { align } => pr.align = Some(*align),
                    ParaFormat::IndentLeft { twips } => pr.indent_left = Some(Twips(*twips)),
                    ParaFormat::IndentFirstLine { twips } => {
                        pr.indent_first_line = Some(Twips(*twips));
                    }
                    ParaFormat::IncreaseIndent | ParaFormat::DecreaseIndent => {
                        let up = f == ParaFormat::IncreaseIndent;
                        if let Some(n) = pr.num.as_mut().filter(|n| n.num_id != 0) {
                            n.ilvl = if up {
                                (n.ilvl + 1).min(8)
                            } else {
                                n.ilvl.saturating_sub(1)
                            };
                        } else {
                            let cur = pr.indent_left.map_or(0, |t| t.0);
                            let next = if up { cur + 720 } else { (cur - 720).max(0) };
                            pr.indent_left = Some(Twips(next));
                        }
                    }
                    ParaFormat::LineSpacing { value } => {
                        pr.line_spacing = Some(LineSpacing::Auto((*value).clamp(60, 2400)));
                    }
                    ParaFormat::SpaceBefore { twips } => {
                        pr.space_before = Some(Twips((*twips).max(0)));
                    }
                    ParaFormat::SpaceAfter { twips } => {
                        pr.space_after = Some(Twips((*twips).max(0)));
                    }
                    ParaFormat::Bullets | ParaFormat::Numbering => {
                        pr.num = list_num.flatten();
                    }
                    ParaFormat::Style { id } => pr.style = Some(id.clone()),
                    ParaFormat::Clear => {
                        let unknown = std::mem::take(&mut pr.unknown);
                        *pr = ParaProps {
                            unknown,
                            ..ParaProps::default()
                        };
                    }
                }
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }

    fn para_props(&self, id: usize) -> ParaProps {
        let paths = body_paragraphs(&self.doc);
        paths
            .get(id)
            .and_then(|p| para_at(&self.doc, p))
            .map(|p| p.props.clone())
            .unwrap_or_default()
    }

    /// Apply a style: paragraph styles to the touched paragraphs, character styles to the selection.
    pub fn apply_style(&mut self, id: &str) -> Result<Selection> {
        let Some(style) = self.doc.style(id).cloned() else {
            return Err(EditError::Unsupported(format!("unknown style {id}")));
        };
        match style.kind {
            StyleKind::Character => {
                let sid = id.to_owned();
                if self.selection.is_collapsed() {
                    let mut p = self
                        .pending
                        .clone()
                        .unwrap_or_else(|| self.caret_direct_props());
                    p.style = Some(sid);
                    self.pending = Some(p);
                    return Ok(self.selection);
                }
                let ranges = self.selected_ranges(self.selection);
                self.edit(|tx, sel| {
                    for (pid, s, e) in ranges {
                        let (path, p, mut atoms) = tx.atoms(pid)?;
                        para::format(&mut atoms, s, e, &|r| r.style = Some(sid.clone()));
                        tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                    }
                    Ok(sel)
                })
            }
            _ => self.format_paragraphs(&ParaFormat::Style { id: id.to_owned() }),
        }
    }

    /// Formatting at the selection start.
    pub fn format_state(&self) -> FormatState {
        let r = self.caret_props();
        let at = self.selection.ordered().0;
        let pp = self.para_props(at.para);
        let doc = &self.doc;
        let list = |bullet: bool| {
            pp.num.is_some_and(|n| {
                n.num_id != 0
                    && doc
                        .numbering
                        .level(n)
                        .is_some_and(|l| (l.format == bo_model::NumFormat::Bullet) == bullet)
            })
        };
        let style = pp.style.clone().or_else(|| {
            doc.styles
                .default_of(StyleKind::Paragraph)
                .map(|s| s.id.clone())
        });
        let align = style
            .as_ref()
            .map(|s| doc.styles.resolve_paragraph(s).0)
            .unwrap_or_default()
            .inherit(&doc.default_para)
            .align;
        FormatState {
            bold: r.bold.unwrap_or(false),
            italic: r.italic.unwrap_or(false),
            underline: r.underline.is_some_and(|u| u != Underline::None),
            strike: r.strike.unwrap_or(false),
            superscript: r.vert_align == Some(VertAlign::Superscript),
            subscript: r.vert_align == Some(VertAlign::Subscript),
            font: r.font.clone(),
            size_half_points: r.size.map(|s| s.0),
            color: r.color.map(Color::to_hex),
            highlight: r.highlight.map(Color::to_hex),
            align: pp.align.or(align).unwrap_or_default(),
            style,
            bullets: list(true),
            numbering: list(false),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
        }
    }

    // -------------------------------------------------------------- insertion

    /// Insert a `rows × cols` table at the caret (§17.4.38 tbl).
    pub fn insert_table(&mut self, rows: usize, cols: usize) -> Result<Selection> {
        let rows = rows.clamp(1, 500);
        let cols = cols.clamp(1, 63);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (path, _, atoms) = tx.atoms(at.para)?;
            let section = path.first().copied().unwrap_or(0);
            let width = tx.doc.sections.get(section).map_or(Twips(9355), |s| {
                Twips(s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0)
            });
            let table = Block::Table(Table::simple(rows, cols, width));
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
            let mut blocks = vec![table];
            let container_len = blocks_at(tx.doc, &prefix).map_or(0, Vec::len);
            if insert_at >= container_len {
                // A table is always followed by a paragraph (§17.4.38 note on cell/body end).
                blocks.push(Block::Paragraph(Paragraph::default()));
            }
            tx.insert_blocks(prefix.clone(), insert_at, blocks)?;
            let mut cell = prefix;
            cell.extend([insert_at, 0, 0, 0]);
            let id = id_of(tx.doc, &cell).ok_or(EditError::BadPath)?;
            Ok(Selection::caret(Pos::new(id, 0)))
        })
    }

    /// Insert an inline picture. `width`/`height` in EMU; `ext` is the file extension (png, jpeg…).
    pub fn insert_image(
        &mut self,
        bytes: Vec<u8>,
        ext: &str,
        width: i64,
        height: i64,
    ) -> Result<Selection> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        let mut n = self.doc.media.len() + 1;
        let name = loop {
            let cand = format!("/word/media/image{n}.{ext}");
            if !self.doc.media.contains_key(&cand) {
                break cand;
            }
            n += 1;
        };
        let bytes = Arc::new(bytes);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            tx.push(Op::Media {
                name: name.clone(),
                old: None,
                new: Some(bytes),
            })?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let img = Image {
                media: name,
                width: bo_model::Emu(width.max(1)),
                height: bo_model::Emu(height.max(1)),
                anchor: None,
                crop: None,
                rotation: 0,
                description: String::new(),
                src: None,
            };
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            Ok(Selection::caret(caret))
        })
    }

    /// Insert a page break (§17.3.3.1 br type="page") and continue in a new paragraph.
    pub fn insert_page_break(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let after = tx.insert_atoms(at, vec![(props, RunContent::PageBreak)])?;
            Ok(Selection::caret(tx.split(after)?))
        })
    }

    /// Insert a section break of `kind` at the caret (§17.6.22 type). Only in the main text, not in tables.
    pub fn insert_section_break(&mut self, kind: SectionBreak) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            if path.len() != 2 {
                return Err(EditError::Unsupported(
                    "section break inside a table".into(),
                ));
            }
            let next = tx.split(at)?;
            let si = path[0];
            let bi = path[1];
            let sec = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
            let mut first = sec.clone();
            first.blocks = sec.blocks[..=bi].to_vec();
            first.src = None;
            let mut second: Section = sec.clone();
            second.blocks = sec.blocks[bi + 1..].to_vec();
            second.break_type = kind;
            tx.push(Op::Sections {
                index: si,
                remove: vec![sec],
                insert: vec![first, second],
            })?;
            Ok(Selection::caret(next))
        })
    }

    // -------------------------------------------------------------- find / replace

    /// Find all matches in the main text.
    pub fn find(&self, query: &str, opts: &FindOptions) -> Result<Vec<Match>> {
        find::find(&self.doc, query, opts)
    }

    /// Replace one match (it must still be valid); returns the selection after the replacement.
    pub fn replace(&mut self, m: Match, with: &str) -> Result<Selection> {
        self.replace_matches(&[m], with, None)
    }

    /// Replace every match of `query`; returns the number of replacements (one undo step).
    pub fn replace_all(&mut self, query: &str, opts: &FindOptions, with: &str) -> Result<usize> {
        let matches = self.find(query, opts)?;
        let n = matches.len();
        if n > 0 {
            let re = opts.regex.then(|| find::build(query, opts)).transpose()?;
            self.replace_matches(&matches, with, re.as_ref())?;
        }
        Ok(n)
    }

    fn replace_matches(
        &mut self,
        matches: &[Match],
        with: &str,
        re: Option<&regex::Regex>,
    ) -> Result<Selection> {
        let mut ordered = matches.to_vec();
        ordered.sort_by_key(|m| std::cmp::Reverse((m.para, m.start)));
        self.edit(|tx, sel| {
            let mut last = sel;
            for m in ordered {
                let (path, p, mut atoms) = tx.atoms(m.para)?;
                let text = bo_model::index::para_index_text(&p);
                if m.end > text.len()
                    || !text.is_char_boundary(m.start)
                    || !text.is_char_boundary(m.end)
                {
                    return Err(EditError::Conflict);
                }
                let replacement = match re {
                    Some(re) => {
                        let mut dst = String::new();
                        if let Some(caps) = re.captures(&text[m.start..m.end]) {
                            caps.expand(with, &mut dst);
                        }
                        dst
                    }
                    None => with.to_owned(),
                };
                let props = para::props_at(&atoms, (m.start + 1).min(m.end.max(m.start)));
                para::delete(&mut atoms, m.start, m.end);
                para::insert(&mut atoms, m.start, para::text_atoms(&replacement, &props));
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                last = Selection {
                    anchor: Pos::new(m.para, m.start),
                    focus: Pos::new(m.para, m.start + replacement.len()),
                };
            }
            Ok(last)
        })
    }

    // -------------------------------------------------------------- clipboard

    /// Paragraphs of the selection (sliced at its ends).
    pub fn selected_blocks(&self) -> Vec<Block> {
        let paths = body_paragraphs(&self.doc);
        let mut out = Vec::new();
        for (id, s, e) in self.selected_ranges(self.selection) {
            let Some(p) = paths.get(id).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let atoms: Vec<Atom> = para::slice(&para::flatten(p), s, e)
                .into_iter()
                .filter(|a| !matches!(a.1, RunContent::Markup(_)))
                .collect();
            out.push(Block::Paragraph(Paragraph {
                props: ParaProps {
                    unknown: Vec::new(),
                    ..p.props.clone()
                },
                runs: para::rebuild(atoms),
                src: None,
            }));
        }
        out
    }

    /// Copy the selection.
    pub fn copy(&self) -> Clip {
        let blocks = self.selected_blocks();
        let text = blocks
            .iter()
            .filter_map(Block::as_paragraph)
            .map(Paragraph::plain_text)
            .collect::<Vec<_>>()
            .join("\n")
            .replace('\u{FFFC}', "");
        Clip {
            html: html::to_html(&self.doc, &blocks),
            internal: serde_json::to_string(&blocks).unwrap_or_default(),
            text,
        }
    }

    /// Cut = copy + delete.
    pub fn cut(&mut self) -> Result<Clip> {
        let c = self.copy();
        if !self.selection.is_collapsed() {
            self.delete_backward()?;
        }
        Ok(c)
    }

    /// Paste blocks produced by [`Session::copy`] (internal format).
    pub fn paste_internal(&mut self, json: &str) -> Result<Selection> {
        let blocks: Vec<Block> =
            serde_json::from_str(json).map_err(|e| EditError::Clipboard(e.to_string()))?;
        self.paste_blocks(blocks)
    }

    /// Paste HTML (from browsers and other applications).
    pub fn paste_html(&mut self, html: &str) -> Result<Selection> {
        self.paste_blocks(html::from_html(html))
    }

    /// Paste plain text (one paragraph per line).
    pub fn paste_text(&mut self, text: &str) -> Result<Selection> {
        let blocks = text
            .replace("\r\n", "\n")
            .split('\n')
            .map(|l| {
                Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    para::rebuild(para::text_atoms(l, &RunProps::default())),
                ))
            })
            .collect();
        self.paste_plain(blocks)
    }

    fn paste_plain(&mut self, blocks: Vec<Block>) -> Result<Selection> {
        // Plain text takes the formatting at the caret.
        let props = self.caret_direct_props();
        let blocks = blocks
            .into_iter()
            .map(|b| match b {
                Block::Paragraph(p) => {
                    let atoms = para::flatten(&p)
                        .into_iter()
                        .map(|(_, c)| (props.clone(), c))
                        .collect();
                    Block::Paragraph(Paragraph::new(ParaProps::default(), para::rebuild(atoms)))
                }
                other => other,
            })
            .collect();
        self.paste_blocks_inner(blocks, true)
    }

    fn paste_blocks(&mut self, blocks: Vec<Block>) -> Result<Selection> {
        self.paste_blocks_inner(blocks, false)
    }

    fn paste_blocks_inner(
        &mut self,
        blocks: Vec<Block>,
        keep_para_props: bool,
    ) -> Result<Selection> {
        let paras: Vec<Paragraph> = blocks
            .into_iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
            .collect();
        if paras.is_empty() {
            return Ok(self.selection);
        }
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            if paras.len() == 1 {
                let atoms = para::flatten(&paras[0]);
                return Ok(Selection::caret(tx.insert_atoms(at, atoms)?));
            }
            let (path, p, mut atoms) = tx.atoms(at.para)?;
            let i = para::split_at(&mut atoms, at.offset);
            let right = atoms.split_off(i);
            let n = paras.len();
            let mut out = Vec::with_capacity(n);
            let mut first = atoms;
            first.extend(para::flatten(&paras[0]));
            out.push(Block::Paragraph(para::with_atoms(&p, first)));
            for q in &paras[1..n - 1] {
                let mut q = q.clone();
                if keep_para_props {
                    q.props = ParaProps {
                        unknown: Vec::new(),
                        ..p.props.clone()
                    };
                }
                out.push(Block::Paragraph(q));
            }
            let last_src = &paras[n - 1];
            let mut last = para::flatten(last_src);
            let caret = para::len(&last);
            last.extend(right);
            let last_props = if keep_para_props {
                ParaProps {
                    unknown: Vec::new(),
                    ..p.props.clone()
                }
            } else {
                last_src.props.clone()
            };
            out.push(Block::Paragraph(Paragraph {
                props: last_props,
                runs: para::rebuild(last),
                src: None,
            }));
            let (prefix, index) = split_path(&path);
            tx.push(Op::Blocks {
                path: prefix,
                index,
                remove: vec![Block::Paragraph(p)],
                insert: out,
            })?;
            Ok(Selection::caret(Pos::new(at.para + n - 1, caret)))
        })
    }

    // -------------------------------------------------------------- word / paragraph ranges

    /// Selection of the word around `p` (double click).
    pub fn word_at(&self, p: Pos) -> Selection {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        for (i, w) in text.split_word_bound_indices() {
            if p.offset >= i && p.offset < i + w.len()
                || p.offset == text.len() && i + w.len() == text.len()
            {
                return Selection {
                    anchor: Pos::new(p.para, i),
                    focus: Pos::new(p.para, i + w.len()),
                };
            }
        }
        Selection::caret(p)
    }

    /// Selection of the whole paragraph (triple click).
    pub fn paragraph_at(&self, p: Pos) -> Selection {
        let p = self.clamp(p);
        let len = self.para_text(p.para).map_or(0, |t| t.len());
        Selection {
            anchor: Pos::new(p.para, 0),
            focus: Pos::new(p.para, len),
        }
    }

    /// Selection of the whole main text (Ctrl+A).
    pub fn select_all(&mut self) -> Selection {
        let n = self.paragraph_count();
        let last = n.saturating_sub(1);
        let len = self.para_text(last).map_or(0, |t| t.len());
        let s = Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(last, len),
        };
        self.set_selection(s);
        s
    }

    /// Previous/next word boundary from `p` (Ctrl+arrows).
    pub fn word_boundary(&self, p: Pos, forward: bool) -> Pos {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        let bounds: Vec<usize> = text
            .split_word_bound_indices()
            .filter(|(_, w)| !w.trim().is_empty())
            .map(|(i, _)| i)
            .collect();
        if forward {
            let next = bounds.iter().copied().find(|&i| i > p.offset);
            match next {
                Some(i) => Pos::new(p.para, i),
                None if p.offset < text.len() => Pos::new(p.para, text.len()),
                None if p.para + 1 < self.paragraph_count() => Pos::new(p.para + 1, 0),
                None => p,
            }
        } else {
            let prev = bounds.iter().copied().rev().find(|&i| i < p.offset);
            match prev {
                Some(i) => Pos::new(p.para, i),
                None if p.offset > 0 => Pos::new(p.para, 0),
                None if p.para > 0 => {
                    let l = self.para_text(p.para - 1).map_or(0, |t| t.len());
                    Pos::new(p.para - 1, l)
                }
                None => p,
            }
        }
    }

    /// Previous/next grapheme position (arrow keys).
    pub fn char_step(&self, p: Pos, forward: bool) -> Pos {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        if forward {
            if p.offset < text.len() {
                Pos::new(p.para, next_boundary(&text, p.offset))
            } else if p.para + 1 < self.paragraph_count() {
                Pos::new(p.para + 1, 0)
            } else {
                p
            }
        } else if p.offset > 0 {
            Pos::new(p.para, prev_boundary(&text, p.offset))
        } else if p.para > 0 {
            let l = self.para_text(p.para - 1).map_or(0, |t| t.len());
            Pos::new(p.para - 1, l)
        } else {
            p
        }
    }
}

#[cfg(test)]
mod tests;

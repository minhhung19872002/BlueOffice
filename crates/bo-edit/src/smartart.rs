//! SmartArt in BlueDoc (CORE-SMART-005): which diagram the SmartArt tools act on, inserting a new
//! one and replacing one's parts. A diagram is a `w:drawing` (ECMA-376 Part 1, §17.3.3.9) shown as
//! an [`Image`] whose media is its drawing part; its five parts (§21.4 data model, layout, quick
//! style, colours; [MS-ODRAWXML] drawing) are media parts of the document, so each edit — the
//! parts and the frame together — is one undo step.

use crate::ops::Op;
use crate::para::{self, Atom};
use crate::{EditError, Result, Selection, Session};
use bo_model::diagram;
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Image, RunContent};
use std::sync::Arc;

/// Is the atom a SmartArt frame (media is a diagram drawing or data model part)?
fn is_smartart(item: &RunContent) -> bool {
    // A group of shapes (a converted diagram) is held like a drawing part but is no diagram.
    matches!(item, RunContent::Image(img)
    if img.shape.is_none() && !diagram::is_group_name(&img.media)
        && diagram::split_name(&img.media).is_some_and(|(k, _, _)| {
            matches!(k, diagram::DiagramPart::Drawing | diagram::DiagramPart::Data)
        }))
}

/// Is the atom a chart frame (media is a chart or chartex part, `bo_model::chart_part`)?
fn is_chart(item: &RunContent) -> bool {
    matches!(item, RunContent::Image(img)
        if img.shape.is_none() && bo_model::chart_part::is_chart_name(&img.media))
}

/// Index of the SmartArt the tools act on among a paragraph's atoms: the first inside a non-empty
/// `range`, for a caret the one just before it, else just after it, else the paragraph's first
/// floating one.
fn smartart_index(atoms: &[Atom], range: (usize, usize)) -> Option<usize> {
    object_index(atoms, range, is_smartart)
}

/// [`smartart_index`] for the objects `pred` accepts.
fn object_index(
    atoms: &[Atom],
    (a, b): (usize, usize),
    pred: fn(&RunContent) -> bool,
) -> Option<usize> {
    let mut off = 0;
    let (mut before, mut after, mut floating) = (None, None, None);
    for (i, (props, item)) in atoms.iter().enumerate() {
        let l = para::atom_len(item);
        if pred(item) && props.del.is_none() {
            if b > a && a <= off && off + l <= b {
                return Some(i);
            }
            if b == a && off + l == a {
                before = Some(i);
            }
            if b == a && off == a && after.is_none() {
                after = Some(i);
            }
            if matches!(item, RunContent::Image(img) if img.anchor.is_some()) && floating.is_none()
            {
                floating = Some(i);
            }
        }
        off += l;
    }
    before.or(after).or(floating)
}

impl Session {
    /// (paragraph id, atom index) of the SmartArt at the selection.
    pub(crate) fn smartart_target(&self) -> Option<(usize, usize)> {
        self.object_target(smartart_index)
    }

    /// (paragraph id, atom index) of the chart at the selection (CORE-CHART-007 in BlueDoc).
    pub(crate) fn chart_target(&self) -> Option<(usize, usize)> {
        self.object_target(|atoms, range| object_index(atoms, range, is_chart))
    }

    /// The chart at the selection (a copy), if any.
    pub fn chart(&self) -> Option<Image> {
        self.object_at(self.chart_target()?)
    }

    /// Insert chart `img` at the selection with its `parts` (chart part, embedded workbook) as
    /// media, one undo step; the new chart stays selected.
    pub fn insert_chart(
        &mut self,
        parts: Vec<(String, Arc<Vec<u8>>)>,
        img: Image,
    ) -> Result<Selection> {
        self.insert_smartart(parts, img)
    }

    /// Replace the chart at the selection: its `parts` (name, bytes; `None` removes the part) and
    /// the frame changed by `f`, one undo step.
    pub fn update_chart(
        &mut self,
        parts: Vec<(String, Option<Arc<Vec<u8>>>)>,
        f: impl FnOnce(&mut Image),
    ) -> Result<Selection> {
        let at = self
            .chart_target()
            .ok_or_else(|| EditError::Unsupported("no chart here".into()))?;
        self.update_object(at, parts, f)
    }

    fn object_target(
        &self,
        index: impl Fn(&[Atom], (usize, usize)) -> Option<usize>,
    ) -> Option<(usize, usize)> {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(a.para)?)?;
        let atoms = para::flatten(p);
        let range = if a.para == b.para {
            (a.offset, b.offset)
        } else {
            (a.offset, usize::MAX)
        };
        index(&atoms, range).map(|i| (a.para, i))
    }

    /// The SmartArt at the selection (a copy), if any.
    pub fn smartart(&self) -> Option<Image> {
        self.object_at(self.smartart_target()?)
    }

    fn object_at(&self, (id, k): (usize, usize)) -> Option<Image> {
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(id)?)?;
        match para::flatten(p).into_iter().nth(k)?.1 {
            RunContent::Image(img) => Some(*img),
            _ => None,
        }
    }

    /// A free diagram number `n` for new parts `<dir>/<kind><n>.xml`: none of the five is a media
    /// part or one of `taken`.
    pub fn new_diagram_number(&self, dir: &str, taken: &[String]) -> usize {
        let mut n = self.part_number_start(1);
        while diagram::part_names(dir, n)
            .iter()
            .any(|p| self.doc.media.contains_key(p) || taken.contains(p))
        {
            n += 1;
        }
        n
    }

    /// Insert SmartArt `img` at the selection with its `parts` (name, bytes) as media, one undo step.
    pub fn insert_smartart(
        &mut self,
        parts: Vec<(String, Arc<Vec<u8>>)>,
        img: Image,
    ) -> Result<Selection> {
        self.edit(move |tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            for (name, bytes) in parts {
                let old = tx.doc.media.get(&name).cloned();
                tx.push(Op::Media {
                    name,
                    old,
                    new: Some(bytes),
                })?;
            }
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            // The new diagram stays selected (the SmartArt tools act on it).
            Ok(Selection {
                anchor: at,
                focus: caret,
            })
        })
    }

    /// Replace the SmartArt at the selection: its `parts` (name, bytes; `None` removes the part)
    /// are stored as media and `f` changes the frame, one undo step.
    pub fn update_smartart(
        &mut self,
        parts: Vec<(String, Option<Arc<Vec<u8>>>)>,
        f: impl FnOnce(&mut Image),
    ) -> Result<Selection> {
        let at = self
            .smartart_target()
            .ok_or_else(|| EditError::Unsupported("no SmartArt here".into()))?;
        self.update_object(at, parts, f)
    }

    fn update_object(
        &mut self,
        (id, k): (usize, usize),
        parts: Vec<(String, Option<Arc<Vec<u8>>>)>,
        f: impl FnOnce(&mut Image),
    ) -> Result<Selection> {
        self.edit(move |tx, sel| {
            for (name, new) in parts {
                let old = tx.doc.media.get(&name).cloned();
                if old.as_deref().map(Vec::as_slice) != new.as_deref().map(Vec::as_slice) {
                    tx.push(Op::Media { name, old, new })?;
                }
            }
            let (path, p, mut atoms) = tx.atoms(id)?;
            let (props, item) = atoms[k].clone();
            let RunContent::Image(img) = item else {
                return Err(EditError::BadPosition);
            };
            let mut img = *img;
            f(&mut img);
            atoms[k] = (props, RunContent::Image(Box::new(img)));
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(sel)
        })
    }
}

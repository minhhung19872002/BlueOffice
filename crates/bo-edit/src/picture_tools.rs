//! Picture tools on document pictures (CORE-PIC-001): which picture the tools act
//! on and undoable edits of it — adjustments, crop, crop shape, size and its media
//! part (compress / change picture). Pictures are `w:drawing` objects (ECMA-376
//! Part 1, §17.3.3.9 drawing; §20.4.2.8 inline, §20.4.2.3 anchor).

use crate::ops::Op;
use crate::para::{self, Atom};
use crate::{EditError, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Image, RunContent};
use std::sync::Arc;

/// Is the atom a picture the tools handle (not a shape, text box or SmartArt)?
fn is_picture(item: &RunContent) -> bool {
    matches!(item, RunContent::Image(img)
        if img.shape.is_none() && !img.media.is_empty() && !img.media.ends_with(".xml"))
}

/// Index of the picture the tools act on among a paragraph's atoms: the first one
/// inside a non-empty `range` (byte offsets); for a caret, the picture just before
/// it, else just after it, else the paragraph's first floating picture.
fn picture_index(atoms: &[Atom], range: (usize, usize)) -> Option<usize> {
    let (a, b) = range;
    let mut off = 0;
    let mut before = None;
    let mut after = None;
    let mut floating = None;
    for (i, (props, item)) in atoms.iter().enumerate() {
        let l = para::atom_len(item);
        if is_picture(item) && props.del.is_none() {
            if b > a && a <= off && off + l <= b {
                return Some(i);
            }
            if b == a {
                if off + l == a {
                    before = Some(i);
                }
                if off == a && after.is_none() {
                    after = Some(i);
                }
            }
            if let RunContent::Image(img) = item {
                if img.anchor.is_some() && floating.is_none() {
                    floating = Some(i);
                }
            }
        }
        off += l;
    }
    before.or(after).or(floating)
}

impl Session {
    /// (paragraph id, atom index) of the picture the tools act on.
    pub(crate) fn tools_target(&self) -> Option<(usize, usize)> {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(a.para)?)?;
        let atoms = para::flatten(p);
        let range = if a.para == b.para {
            (a.offset, b.offset)
        } else {
            (a.offset, usize::MAX)
        };
        picture_index(&atoms, range).map(|i| (a.para, i))
    }

    /// The picture the picture tools act on (a copy).
    pub fn tools_picture(&self) -> Option<Image> {
        let (id, k) = self.tools_target()?;
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(id)?)?;
        match para::flatten(p).into_iter().nth(k)?.1 {
            RunContent::Image(img) => Some(*img),
            _ => None,
        }
    }

    /// Change the picture the tools act on (one undo step). With `media`, the
    /// picture's media part becomes the new bytes under that name; the old part is
    /// removed when no other drawing uses it.
    pub fn update_picture(
        &mut self,
        media: Option<(String, Arc<Vec<u8>>)>,
        f: impl FnOnce(&mut Image),
    ) -> Result<Selection> {
        let (id, k) = self
            .tools_target()
            .ok_or_else(|| EditError::Unsupported("no picture here".into()))?;
        let old_media = self.tools_picture().map(|i| i.media).unwrap_or_default();
        self.edit(move |tx, sel| {
            let (path, p, mut atoms) = tx.atoms(id)?;
            let (props, item) = atoms[k].clone();
            let RunContent::Image(img) = item else {
                return Err(EditError::BadPosition);
            };
            let mut img = *img;
            if let Some((name, bytes)) = media {
                tx.push(Op::Media {
                    name: name.clone(),
                    old: None,
                    new: Some(bytes),
                })?;
                img.media = name;
            }
            f(&mut img);
            let changed_media = img.media != old_media;
            atoms[k] = (props, RunContent::Image(Box::new(img)));
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            if changed_media && !media_in_use(&*tx.doc, &old_media) {
                let old = tx.doc.media.get(&old_media).cloned();
                if old.is_some() {
                    tx.push(Op::Media {
                        name: old_media,
                        old,
                        new: None,
                    })?;
                }
            }
            Ok(sel)
        })
    }

    /// Every picture of the main text the tools handle (copies).
    pub fn body_pictures(&self) -> Vec<Image> {
        let mut out = Vec::new();
        for path in body_paragraphs(&self.doc) {
            if let Some(p) = para_at(&self.doc, &path) {
                for (_, item) in para::flatten(p) {
                    if is_picture(&item) {
                        if let RunContent::Image(img) = item {
                            out.push(*img);
                        }
                    }
                }
            }
        }
        out
    }

    /// Give the main text's pictures new media parts (one undo step): every picture
    /// showing `old` shows `new` instead (its crop cleared when `clear_crop`); old
    /// parts no drawing uses any more are removed.
    pub fn replace_media(&mut self, swaps: Vec<MediaSwap>) -> Result<Selection> {
        if swaps.is_empty() {
            return Ok(self.selection);
        }
        let ids: Vec<usize> = (0..body_paragraphs(&self.doc).len()).collect();
        self.edit(move |tx, sel| {
            for sw in &swaps {
                tx.push(Op::Media {
                    name: sw.new.clone(),
                    old: None,
                    new: Some(sw.bytes.clone()),
                })?;
            }
            for &id in &ids {
                let (path, p, mut atoms) = tx.atoms(id)?;
                let mut changed = false;
                for (_, item) in &mut atoms {
                    if let RunContent::Image(img) = item {
                        if let Some(sw) = swaps.iter().find(|s| s.old == img.media) {
                            img.media = sw.new.clone();
                            if sw.clear_crop {
                                img.crop = None;
                            }
                            changed = true;
                        }
                    }
                }
                if changed {
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            for sw in &swaps {
                if !media_in_use(&*tx.doc, &sw.old) {
                    let old = tx.doc.media.get(&sw.old).cloned();
                    if old.is_some() {
                        tx.push(Op::Media {
                            name: sw.old.clone(),
                            old,
                            new: None,
                        })?;
                    }
                }
            }
            Ok(sel)
        })
    }

    /// A media part name not used yet, `/word/media/image<n>.<ext>`.
    pub fn new_media_name(&self, ext: &str) -> String {
        let mut n = self.part_number_start(self.doc.media.len() + 1);
        loop {
            let cand = format!("/word/media/image{n}.{ext}");
            if !self.doc.media.contains_key(&cand) {
                return cand;
            }
            n += 1;
        }
    }
}

/// A media part replaced by [`Session::replace_media`].
#[derive(Debug, Clone, PartialEq)]
pub struct MediaSwap {
    /// Media part shown now.
    pub old: String,
    /// New media part name.
    pub new: String,
    /// Its bytes.
    pub bytes: Arc<Vec<u8>>,
    /// Clear the crop of the pictures (the cropped areas were removed).
    pub clear_crop: bool,
}

/// Does any drawing of the document (body, headers, footers, notes, comments)
/// still show media part `name`?
pub fn media_in_use(doc: &bo_model::Document, name: &str) -> bool {
    let quoted = format!("{name:?}");
    format!(
        "{:?}{:?}{:?}{:?}{:?}",
        doc.sections, doc.footnotes, doc.endnotes, doc.comments, doc.continuation_notice
    )
    .contains(&quoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pos;
    use bo_model::picture::PictureFormat;
    use bo_model::{Block, Document, Emu, ParaProps, Paragraph, Run, RunProps, Section};

    fn picture() -> Image {
        Image {
            media: "/word/media/image1.png".into(),
            width: Emu(100),
            height: Emu(100),
            anchor: None,
            crop: None,
            rotation: 0,
            description: String::new(),
            shape: None,
            format: None,
            src: None,
            ole: None,
        }
    }

    fn doc_with_picture() -> Document {
        let p = Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![
                    RunContent::Text("ab".into()),
                    RunContent::Image(Box::new(picture())),
                ],
            }],
        );
        let mut doc = Document {
            sections: vec![Section::new(vec![Block::Paragraph(p)])],
            ..Default::default()
        };
        doc.media
            .insert("/word/media/image1.png".into(), Arc::new(vec![1, 2, 3]));
        doc
    }

    #[test]
    fn picture_before_the_caret_is_edited_and_undone() {
        let mut s = Session::new(doc_with_picture());
        // Caret right after the picture (2 text bytes + the picture atom).
        let end = 2 + para::atom_len(&RunContent::Image(Box::new(picture())));
        s.set_selection(Selection::caret(Pos::new(0, end)));
        assert!(s.tools_picture().is_some());
        // A caret before the text has no picture next to it.
        s.set_selection(Selection::caret(Pos::new(0, 0)));
        assert!(s.tools_picture().is_none());
        s.set_selection(Selection::caret(Pos::new(0, end)));
        s.update_picture(None, |img| {
            img.crop = Some([10_000, 0, 10_000, 0]);
            img.format = Some(Box::new(PictureFormat {
                shape: Some("ellipse".into()),
                ..PictureFormat::default()
            }));
        })
        .unwrap();
        let img = s.tools_picture().unwrap();
        assert_eq!(img.crop, Some([10_000, 0, 10_000, 0]));
        // New media replaces the old part, which nothing else uses.
        s.update_picture(
            Some(("/word/media/image2.jpeg".into(), Arc::new(vec![9]))),
            |_| {},
        )
        .unwrap();
        assert!(!s.doc().media.contains_key("/word/media/image1.png"));
        assert_eq!(s.tools_picture().unwrap().media, "/word/media/image2.jpeg");
        assert!(s.undo());
        assert!(s.doc().media.contains_key("/word/media/image1.png"));
        assert!(!s.doc().media.contains_key("/word/media/image2.jpeg"));
        assert!(s.undo());
        assert_eq!(s.tools_picture().unwrap().crop, None);
    }
}

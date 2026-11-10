//! Embedded and linked objects in the document (CORE-EMBED-001): insert one at the
//! selection, change the object at the selection (new embedded package and preview)
//! and refresh linked objects. An object is a picture ([`Image`]) whose
//! [`Image::ole`] is set; its preview is the picture's media part and its embedded
//! package another part held in [`bo_model::Document::media`] once it is replaced.

use crate::ops::Op;
use crate::para;
use crate::{EditError, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::ole::OleObject;
use bo_model::{Emu, Image, RunContent};
use std::sync::Arc;

/// A part to add or replace: (part name, bytes).
pub type NewPart = (String, Arc<Vec<u8>>);

/// What [`Session::update_ole`] changes.
#[derive(Debug, Clone, Default)]
pub struct OleUpdate {
    /// New preview picture (a new media part).
    pub preview: Option<NewPart>,
    /// New bytes of the embedded package (same part name or a new one).
    pub package: Option<NewPart>,
    /// New shown size (EMU).
    pub size: Option<(i64, i64)>,
}

fn push_part(tx: &mut crate::Tx<'_>, (name, bytes): NewPart) -> Result<()> {
    let old = tx.doc.media.get(&name).cloned();
    tx.push(Op::Media {
        name,
        old,
        new: Some(bytes),
    })
}

fn apply(img: &mut Image, u: &OleUpdate) {
    if let Some((name, _)) = &u.preview {
        img.media.clone_from(name);
    }
    if let (Some((name, _)), Some(o)) = (&u.package, img.ole.as_mut()) {
        o.package.clone_from(name);
    }
    if let Some((w, h)) = u.size {
        img.width = Emu(w.max(1));
        img.height = Emu(h.max(1));
    }
}

impl Session {
    /// The object at the selection (a copy).
    pub fn ole_at_selection(&self) -> Option<Image> {
        self.tools_picture().filter(|i| i.ole.is_some())
    }

    /// Every object of the main text (copies), in document order.
    pub fn body_objects(&self) -> Vec<Image> {
        let mut out = Vec::new();
        for path in body_paragraphs(&self.doc) {
            if let Some(p) = para_at(&self.doc, &path) {
                for (_, item) in para::flatten(p) {
                    if let RunContent::Image(img) = item {
                        if img.ole.is_some() {
                            out.push(*img);
                        }
                    }
                }
            }
        }
        out
    }

    /// Insert an object at the selection, shown through `preview` at `width` × `height`
    /// EMU; `package` is its embedded part (none for a linked object).
    pub fn insert_ole(
        &mut self,
        preview: NewPart,
        package: Option<NewPart>,
        ole: OleObject,
        width: i64,
        height: i64,
    ) -> Result<Selection> {
        self.edit(move |tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let media = preview.0.clone();
            push_part(tx, preview)?;
            if let Some(p) = package {
                push_part(tx, p)?;
            }
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let img = Image {
                media,
                width: Emu(width.max(1)),
                height: Emu(height.max(1)),
                anchor: None,
                crop: None,
                rotation: 0,
                description: String::new(),
                shape: None,
                format: None,
                ole: Some(Box::new(ole)),
                src: None,
            };
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            Ok(Selection::caret(caret))
        })
    }

    /// Change the object at the selection (one undo step).
    pub fn update_ole(&mut self, u: OleUpdate) -> Result<Selection> {
        let (id, k) = self
            .tools_target()
            .filter(|_| self.ole_at_selection().is_some())
            .ok_or_else(|| EditError::Unsupported("no object here".into()))?;
        self.edit(move |tx, sel| {
            let (path, p, mut atoms) = tx.atoms(id)?;
            let (props, item) = atoms[k].clone();
            let RunContent::Image(mut img) = item else {
                return Err(EditError::BadPosition);
            };
            for part in [u.preview.clone(), u.package.clone()].into_iter().flatten() {
                push_part(tx, part)?;
            }
            apply(&mut img, &u);
            atoms[k] = (props, RunContent::Image(img));
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(sel)
        })
    }

    /// Give every linked object of the main text whose source is `source` a new preview
    /// and size (one undo step); returns how many changed.
    pub fn update_links(&mut self, source: &str, u: &OleUpdate) -> Result<usize> {
        let ids: Vec<usize> = (0..body_paragraphs(&self.doc).len()).collect();
        let mut count = 0;
        let src = source.to_owned();
        let u = u.clone();
        let hit = |img: &Image| {
            img.ole
                .as_ref()
                .and_then(|o| o.link.as_ref())
                .is_some_and(|l| l.source == src)
        };
        if !self.body_objects().iter().any(&hit) {
            return Ok(0);
        }
        self.edit(|tx, sel| {
            if let Some(part) = u.preview.clone() {
                push_part(tx, part)?;
            }
            for &id in &ids {
                let (path, p, mut atoms) = tx.atoms(id)?;
                let mut changed = false;
                for (_, item) in &mut atoms {
                    if let RunContent::Image(img) = item {
                        if hit(img) {
                            apply(img, &u);
                            changed = true;
                            count += 1;
                        }
                    }
                }
                if changed {
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            Ok(sel)
        })?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::ole::{OleLink, PROG_ID_XLSX};
    use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section};

    fn session() -> Session {
        let mut doc = Document::default();
        doc.sections.push(Section {
            blocks: vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run::text("ab", RunProps::default())],
            ))],
            ..Section::default()
        });
        Session::new(doc)
    }

    fn part(name: &str, b: u8) -> NewPart {
        (name.to_owned(), Arc::new(vec![b]))
    }

    #[test]
    fn insert_update_and_undo() {
        let mut s = session();
        s.set_selection(Selection::caret(crate::Pos::new(0, 1)));
        let ole = OleObject {
            prog_id: PROG_ID_XLSX.into(),
            package: "/word/embeddings/w1.xlsx".into(),
            ..OleObject::default()
        };
        s.insert_ole(
            part("/word/media/p1.png", 1),
            Some(part("/word/embeddings/w1.xlsx", 2)),
            ole,
            100,
            50,
        )
        .unwrap();
        assert_eq!(s.doc().media["/word/embeddings/w1.xlsx"].as_slice(), &[2]);
        // The caret is after the object: it is the object the tools act on.
        let img = s.ole_at_selection().unwrap();
        assert_eq!((img.width, img.height), (Emu(100), Emu(50)));
        s.update_ole(OleUpdate {
            preview: Some(part("/word/media/p2.png", 3)),
            package: Some(part("/word/embeddings/w1.xlsx", 4)),
            size: Some((200, 80)),
        })
        .unwrap();
        let img = s.ole_at_selection().unwrap();
        assert_eq!(img.media, "/word/media/p2.png");
        assert_eq!(img.width, Emu(200));
        assert_eq!(s.doc().media["/word/embeddings/w1.xlsx"].as_slice(), &[4]);
        s.undo();
        assert_eq!(s.doc().media["/word/embeddings/w1.xlsx"].as_slice(), &[2]);
        assert_eq!(s.ole_at_selection().unwrap().media, "/word/media/p1.png");
    }

    #[test]
    fn links_update_by_source() {
        let mut s = session();
        let ole = OleObject {
            prog_id: PROG_ID_XLSX.into(),
            link: Some(OleLink {
                source: "C:/a.xlsx".into(),
                ..OleLink::default()
            }),
            ..OleObject::default()
        };
        s.insert_ole(part("/word/media/p1.png", 1), None, ole.clone(), 10, 10)
            .unwrap();
        s.insert_ole(part("/word/media/p1.png", 1), None, ole, 10, 10)
            .unwrap();
        let u = OleUpdate {
            preview: Some(part("/word/media/p9.png", 9)),
            size: Some((30, 20)),
            ..OleUpdate::default()
        };
        assert_eq!(s.update_links("C:/other.xlsx", &u).unwrap(), 0);
        assert_eq!(s.update_links("C:/a.xlsx", &u).unwrap(), 2);
        assert!(s
            .body_objects()
            .iter()
            .all(|i| i.media == "/word/media/p9.png" && i.width == Emu(30)));
    }
}

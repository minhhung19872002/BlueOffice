//! Cover pages: a block-level content control (ECMA-376 Part 1, §17.5.2.29 sdt) around the cover's
//! paragraphs whose properties name the "Cover Pages" building block gallery (§17.5.2.11
//! docPartObj, §17.5.2.12 docPartGallery, §17.5.2.13 docPartUnique), as desktop word processors
//! store the cover they insert, so Insert › Cover Page › Remove Current Cover Page can find it.

use crate::para::{self, Atom};
use crate::{max_control_id, split_path, EditError, Pos, Result, Selection, Session};
use bo_model::index::body_paragraphs;
use bo_model::{Block, ContentControl, ControlKind, Paragraph, RunContent, RunProps};

/// §17.5.2.12 docPartGallery value of a cover page.
const COVER_GALLERY: &str = "Cover Pages";

/// The control's properties are those of a cover page (`docPartGallery` "Cover Pages").
pub fn is_cover(cc: &ContentControl) -> bool {
    cc.block
        && cc.props_xml.as_deref().is_some_and(|x| {
            x.contains("docPartObj")
                && (x.contains(&format!("w:val=\"{COVER_GALLERY}\""))
                    || x.contains(&format!("w:val='{COVER_GALLERY}'")))
        })
}

/// A new cover-page control with id `id`: `w:sdtPr` holds the id and the gallery object
/// (written as is while unchanged).
fn cover_control(id: i64) -> ContentControl {
    ContentControl {
        id: Some(id),
        alias: None,
        tag: None,
        kind: ControlKind::Other,
        lock: None,
        placeholder: false,
        block: true,
        // ECMA-376 Part 1, §17.5.2.38 sdtPr: id, then docPartObj (gallery, unique).
        props_xml: Some(format!(
            "<w:sdtPr><w:id w:val=\"{id}\"/><w:docPartObj><w:docPartGallery \
             w:val=\"{COVER_GALLERY}\"/><w:docPartUnique/></w:docPartObj></w:sdtPr>"
        )),
        data_binding: None,
        appearance: None,
        color: None,
        temporary: false,
    }
}

impl Session {
    /// The body paragraphs (first, last) of the document's cover page, if it has one.
    pub fn cover_page(&self) -> Option<(usize, usize)> {
        self.block_spans()
            .into_iter()
            .filter(|s| is_cover(&s.cc))
            .min_by_key(|s| s.first)
            .map(|s| (s.first, s.last))
    }

    /// Insert a cover page from `html` at the document's start, followed by a page break, in a
    /// cover-page control; an existing cover page is replaced (one undo step). The caret goes
    /// to the cover's start.
    pub fn insert_cover_page(&mut self, html: &str) -> Result<Selection> {
        let depth = self.history_len();
        let res = self.insert_cover_inner(html);
        if res.is_err() {
            // Take back the steps already made.
            while self.history_len() > depth {
                self.undo();
            }
            return res;
        }
        self.merge_history_since(depth);
        res
    }

    fn insert_cover_inner(&mut self, html: &str) -> Result<Selection> {
        self.remove_cover_page()?;
        let start = Selection::caret(Pos::new(0, 0));
        self.set_selection(start);
        self.insert_page_break()?;
        let before = body_paragraphs(&self.doc).len();
        self.set_selection(start);
        self.paste_html(html)?;
        let added = body_paragraphs(&self.doc).len().saturating_sub(before);
        // The pasted paragraphs and the one holding the page break.
        let last = added;
        let id = max_control_id(&self.doc) + 1;
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(0)?;
            let open: Atom = (
                RunProps::default(),
                RunContent::ControlStart(Box::new(cover_control(id))),
            );
            atoms.insert(0, open);
            let end: Atom = (RunProps::default(), RunContent::ControlEnd(true));
            if last == 0 {
                atoms.push(end);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            } else {
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                let (path, p, mut atoms) = tx.atoms(last)?;
                atoms.push(end);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(start)
        })
    }

    /// Delete the document's cover page (the cover-page control and its paragraphs).
    /// Returns whether there was one (one undo step).
    pub fn remove_cover_page(&mut self) -> Result<bool> {
        let Some((first, last)) = self.cover_page() else {
            return Ok(false);
        };
        let paths = body_paragraphs(&self.doc);
        let (pa, pb) = (
            paths.get(first).ok_or(EditError::BadPosition)?,
            paths.get(last).ok_or(EditError::BadPosition)?,
        );
        let ((ca, ia), (cb, ib)) = (split_path(pa), split_path(pb));
        if ca != cb || ib < ia {
            return Err(EditError::Unsupported(
                "cover page across containers".into(),
            ));
        }
        let total = body_paragraphs(&self.doc).len();
        let keep_one = last + 1 >= total && first == 0;
        self.edit(|tx, _| {
            let remove: Vec<Block> = (ia..=ib)
                .map(|i| block_at(tx.doc, &ca, i))
                .collect::<Result<_>>()?;
            // A document never ends up without a paragraph.
            let insert = if keep_one {
                vec![Block::Paragraph(Paragraph::default())]
            } else {
                Vec::new()
            };
            tx.push(crate::ops::Op::Blocks {
                path: ca.clone(),
                index: ia,
                remove,
                insert,
            })?;
            Ok(Selection::caret(Pos::new(first, 0)))
        })?;
        self.fix_selection();
        Ok(true)
    }
}

/// Block `index` of the container at `path` (a section's blocks or a cell's).
fn block_at(doc: &bo_model::Document, path: &[usize], index: usize) -> Result<Block> {
    let blocks: &[Block] = match path {
        [si] => &doc.sections.get(*si).ok_or(EditError::BadPath)?.blocks,
        _ => return Err(EditError::Unsupported("cover page in a table".into())),
    };
    blocks.get(index).cloned().ok_or(EditError::BadPath)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Document, ParaProps, Run, Section};

    fn doc(paras: &[&str]) -> Document {
        Document {
            sections: vec![Section::new(
                paras
                    .iter()
                    .map(|t| {
                        Block::Paragraph(Paragraph::new(
                            ParaProps::default(),
                            vec![Run::text(*t, RunProps::default())],
                        ))
                    })
                    .collect(),
            )],
            ..Default::default()
        }
    }

    fn texts(s: &Session) -> Vec<String> {
        (0..s.paragraph_count())
            .map(|i| s.para_text(i).unwrap_or_default())
            .collect()
    }

    #[test]
    fn a_cover_page_is_inserted_replaced_and_removed_in_one_step_each() {
        let mut s = Session::new(doc(&["Body one", "Body two"]));
        s.set_selection(Selection::caret(Pos::new(1, 3)));
        let depth = s.history_len();
        s.insert_cover_page("<p>Title</p><p>Subtitle</p>")
            .expect("insert");
        assert_eq!(s.history_len(), depth + 1, "one undo step");
        let (first, last) = s.cover_page().expect("cover");
        assert_eq!(first, 0);
        let t = texts(&s);
        assert_eq!(t[0], "Title");
        // The last paragraph of the cover holds the page break.
        assert_eq!(t[1], "Subtitle\u{c}");
        assert_eq!(last, 1);
        // The body follows the cover, which ends with the paragraph holding the page break.
        assert_eq!(t[last + 1], "Body one");
        assert_eq!(t.last().map(String::as_str), Some("Body two"));
        // A second cover replaces the first.
        s.insert_cover_page("<p>Other</p>").expect("replace");
        let t = texts(&s);
        assert_eq!(t[0], "Other\u{c}");
        assert_eq!(t.iter().filter(|x| x.as_str() == "Title").count(), 0);
        assert_eq!(t.iter().filter(|x| x.as_str() == "Body one").count(), 1);
        // Remove Current Cover Page leaves the body as it was.
        assert!(s.remove_cover_page().expect("remove"));
        assert_eq!(texts(&s), vec!["Body one", "Body two"]);
        assert!(s.cover_page().is_none());
        assert!(!s.remove_cover_page().expect("none"));
        // Undo brings the cover back.
        assert!(s.undo());
        assert!(s.cover_page().is_some());
    }

    #[test]
    fn a_cover_control_is_recognised_by_its_gallery() {
        let cc = cover_control(7);
        assert!(is_cover(&cc));
        let mut other = cc.clone();
        other.props_xml = Some(
            "<w:sdtPr><w:docPartObj><w:docPartGallery w:val=\"Table of Contents\"/>\
             </w:docPartObj></w:sdtPr>"
                .into(),
        );
        assert!(!is_cover(&other));
    }
}

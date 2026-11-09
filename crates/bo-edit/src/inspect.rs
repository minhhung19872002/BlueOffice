//! Whole-document replacements used by the document inspector and the
//! accessibility checker (CORE-INSPECT-001): the caller computes the cleaned
//! sections (ECMA-376 Part 1, §17.6) and comments (§17.13.4) and this records
//! them as one reversible transaction.

use crate::{Op, Result, Selection, Session};
use bo_model::{Comment, Section};

impl Session {
    /// Replace the sections and the comments by `sections` / `comments` as one undo
    /// step: one [`Op::Sections`] per changed section and one [`Op::Comments`] when
    /// the comments differ. Returns whether anything changed.
    pub fn replace_content(
        &mut self,
        sections: Vec<Section>,
        comments: Vec<Comment>,
    ) -> Result<bool> {
        let same_sections = self.doc.sections == sections;
        let same_comments = self.doc.comments == comments;
        if same_sections && same_comments {
            return Ok(false);
        }
        let sel = self.selection;
        self.edit(|tx, _| {
            if !same_sections {
                if tx.doc.sections.len() == sections.len() {
                    for (i, s) in sections.into_iter().enumerate() {
                        if tx.doc.sections[i] != s {
                            let old = tx.doc.sections[i].clone();
                            tx.push(Op::Sections {
                                index: i,
                                remove: vec![old],
                                insert: vec![s],
                            })?;
                        }
                    }
                } else {
                    let old = tx.doc.sections.clone();
                    tx.push(Op::Sections {
                        index: 0,
                        remove: old,
                        insert: sections,
                    })?;
                }
            }
            if !same_comments {
                let old = tx.doc.comments.clone();
                tx.push(Op::Comments { old, new: comments })?;
            }
            Ok(sel)
        })?;
        // Paragraphs may have gone: keep the selection inside the document.
        let s = self.selection;
        self.selection = Selection {
            anchor: self.clamp(s.anchor),
            focus: self.clamp(s.focus),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Block, Document, Paragraph, Run, RunProps};

    fn doc(texts: &[&str]) -> Document {
        Document {
            sections: vec![Section::new(
                texts
                    .iter()
                    .map(|t| {
                        Block::Paragraph(Paragraph::new(
                            bo_model::ParaProps::default(),
                            vec![Run::text(*t, RunProps::default())],
                        ))
                    })
                    .collect(),
            )],
            ..Document::default()
        }
    }

    #[test]
    fn replace_is_one_undo_step() {
        let mut s = Session::new(doc(&["a", "b"]));
        let new = doc(&["a"]).sections;
        assert!(s.replace_content(new.clone(), Vec::new()).unwrap());
        assert_eq!(s.doc().sections, new);
        assert!(!s.replace_content(new, Vec::new()).unwrap());
        assert!(s.undo());
        assert_eq!(s.doc().sections, doc(&["a", "b"]).sections);
    }
}

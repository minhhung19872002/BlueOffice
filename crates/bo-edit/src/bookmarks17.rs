//! Bookmarks of the main text for Insert › Links › Bookmark (BlueDoc round 17): list them
//! with their positions, add one around the selection, delete one, select one (Go To).
//! ECMA-376 Part 1, §17.13.6.2 bookmarkStart / §17.13.6.1 bookmarkEnd; they live in the
//! model as paragraph-level markup (`RunContent::Markup`) next to the runs.

use crate::fields::{bookmark_marks, bookmark_xml, max_bookmark_id};
use crate::{para, EditError, Pos, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{RunContent, RunProps};
use serde::Serialize;
use std::collections::HashMap;

/// A bookmark with the positions of its start and end (body paragraph id, byte offset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkEntry {
    /// Bookmark name (§17.13.6.2 @w:name).
    pub name: String,
    /// Where it starts.
    pub start: Pos,
    /// Where it ends (the start when its end is missing).
    pub end: Pos,
    /// A hidden bookmark (its name starts with `_`, as cross-references' `_Ref…`).
    pub hidden: bool,
}

/// Whether `name` can name a bookmark: a letter first, then letters, digits or `_`, at
/// most 40 characters (the limit other applications keep, [MS-OI29500] §2.1.152).
#[must_use]
pub fn valid_bookmark_name(name: &str) -> bool {
    let mut c = name.chars();
    c.next().is_some_and(char::is_alphabetic)
        && name.chars().count() <= 40
        && c.all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// Byte offset in the paragraph's index text of each atom (and of the end).
fn offsets(atoms: &[crate::para::Atom]) -> Vec<usize> {
    let mut out = Vec::with_capacity(atoms.len() + 1);
    let mut at = 0;
    for (_, c) in atoms {
        out.push(at);
        at += para::atom_len(c);
    }
    out.push(at);
    out
}

impl Session {
    /// The bookmarks of the main text in document order.
    #[must_use]
    pub fn bookmark_list(&self) -> Vec<BookmarkEntry> {
        let mut out: Vec<BookmarkEntry> = Vec::new();
        let mut open: HashMap<String, usize> = HashMap::new();
        for (pid, path) in body_paragraphs(&self.doc).iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            let atoms = para::flatten(p);
            let offs = offsets(&atoms);
            for (ai, name, id) in bookmark_marks(&atoms) {
                let pos = Pos::new(pid, offs.get(ai).copied().unwrap_or(0));
                match name {
                    Some(n) => {
                        open.insert(id, out.len());
                        out.push(BookmarkEntry {
                            hidden: n.starts_with('_'),
                            name: n,
                            start: pos,
                            end: pos,
                        });
                    }
                    None => {
                        if let Some(i) = open.remove(&id) {
                            out[i].end = pos;
                        }
                    }
                }
            }
        }
        out
    }

    /// Add bookmark `name` around the selection (one undo step); a bookmark of that name
    /// already there moves, as the reference's Add.
    pub fn add_bookmark(&mut self, name: &str) -> Result<()> {
        if !valid_bookmark_name(name) {
            return Err(EditError::Unsupported(format!("bookmark name {name}")));
        }
        let id = max_bookmark_id(&self.doc) + 1;
        let existing = self.bookmark_ids(name);
        let esc = escape(name);
        let (bs, be) = bookmark_xml(id, &esc);
        self.edit(|tx, sel| {
            // The old marks of that name go first.
            if !existing.is_empty() {
                for pid in 0..tx.paths().len() {
                    let (path, p, mut atoms) = tx.atoms(pid)?;
                    let before = atoms.len();
                    atoms.retain(|(_, c)| !is_mark_of(c, &existing));
                    if atoms.len() != before {
                        tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                    }
                }
            }
            let (a, b) = sel.ordered();
            let start = (RunProps::default(), RunContent::Markup(bs));
            let end = (RunProps::default(), RunContent::Markup(be));
            if a == b {
                let (path, p, mut atoms) = tx.atoms(a.para)?;
                para::insert(&mut atoms, a.offset, vec![start, end]);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            } else {
                let (path, p, mut atoms) = tx.atoms(b.para)?;
                para::insert(&mut atoms, b.offset, vec![end]);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                let (path, p, mut atoms) = tx.atoms(a.para)?;
                para::insert(&mut atoms, a.offset, vec![start]);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// Delete bookmark `name` (its marks; the text stays). `false` when there is none.
    pub fn delete_bookmark(&mut self, name: &str) -> Result<bool> {
        let ids = self.bookmark_ids(name);
        if ids.is_empty() {
            return Ok(false);
        }
        self.edit(|tx, sel| {
            for pid in 0..tx.paths().len() {
                let (path, p, mut atoms) = tx.atoms(pid)?;
                let before = atoms.len();
                atoms.retain(|(_, c)| !is_mark_of(c, &ids));
                if atoms.len() != before {
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            Ok(sel)
        })?;
        Ok(true)
    }

    /// Select the text of bookmark `name` (Go To). `false` when there is none.
    pub fn go_to_bookmark(&mut self, name: &str) -> bool {
        let Some(b) = self.bookmark_list().into_iter().find(|b| b.name == name) else {
            return false;
        };
        self.set_selection(Selection {
            anchor: b.start,
            focus: b.end,
        });
        true
    }

    /// `w:id`s of the bookmarks named `name`.
    fn bookmark_ids(&self, name: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for path in body_paragraphs(&self.doc) {
            if let Some(p) = para_at(&self.doc, &path) {
                for (_, n, id) in bookmark_marks(&para::flatten(p)) {
                    if n.as_deref() == Some(name) {
                        ids.push(id);
                    }
                }
            }
        }
        ids
    }
}

/// A bookmark start or end mark with one of `ids`.
fn is_mark_of(c: &RunContent, ids: &[String]) -> bool {
    let RunContent::Markup(raw) = c else {
        return false;
    };
    let x = &raw.xml;
    (x.contains("bookmarkStart") || x.contains("bookmarkEnd"))
        && ids.iter().any(|id| x.contains(&format!("w:id=\"{id}\"")))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Block, Document, ParaProps, Paragraph, Run, Section};

    fn session(texts: &[&str]) -> Session {
        let blocks = texts
            .iter()
            .map(|t| {
                Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    vec![Run::text(t, RunProps::default())],
                ))
            })
            .collect();
        Session::new(Document {
            sections: vec![Section {
                blocks,
                ..Section::default()
            }],
            ..Document::default()
        })
    }

    #[test]
    fn add_list_go_to_and_delete() {
        let mut s = session(&["Một hai ba", "bốn năm"]);
        s.set_selection(Selection {
            anchor: Pos::new(0, 4),
            focus: Pos::new(1, 3),
        });
        s.add_bookmark("Muc1").expect("add");
        s.set_selection(Selection::caret(Pos::new(0, 0)));
        s.add_bookmark("_Hidden").expect("add hidden");
        let list = s.bookmark_list();
        assert_eq!(list.len(), 2);
        let m = list.iter().find(|b| b.name == "Muc1").expect("Muc1");
        assert_eq!((m.start, m.end), (Pos::new(0, 4), Pos::new(1, 3)));
        assert!(list.iter().any(|b| b.name == "_Hidden" && b.hidden));
        assert!(s.go_to_bookmark("Muc1"));
        assert_eq!(s.selection().focus, Pos::new(1, 3));
        // Adding the same name again moves it.
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        s.add_bookmark("Muc1").expect("move");
        let m = s.bookmark_list();
        let m = m.iter().find(|b| b.name == "Muc1").expect("moved");
        assert_eq!(m.start, Pos::new(1, 0));
        assert!(s.delete_bookmark("Muc1").expect("delete"));
        assert!(!s.bookmark_list().iter().any(|b| b.name == "Muc1"));
        assert!(!s.delete_bookmark("Muc1").expect("none"));
        s.undo();
        assert!(s.bookmark_list().iter().any(|b| b.name == "Muc1"));
        assert!(s.add_bookmark("1bad").is_err());
        assert!(s.add_bookmark("has space").is_err());
    }
}

//! Document outline: paragraphs by outline level (ECMA-376 Part 1, §17.3.1.20
//! outlineLvl, through the heading styles §17.7.4), promote/demote, and moving a
//! heading together with its subordinate content. Every change is one undo step.
//!
//! A heading's *family* is the heading and the top-level blocks after it up to the
//! next paragraph whose outline level is the same or higher (a smaller number); a
//! body-text paragraph is its own family. Only top-level blocks of a section take
//! part (paragraphs inside tables move with their table).

use crate::fields::outline_level;
use crate::{EditError, Op, Pos, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at, OBJECT};
use bo_model::{Block, Document};
use serde::Serialize;

/// Longest text reported for an outline entry (characters).
const TEXT_MAX: usize = 160;

/// One top-level body paragraph of the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineEntry {
    /// Body paragraph id.
    pub para: usize,
    /// 0-based outline level (0 = Heading 1); `None` = body text.
    pub level: Option<u8>,
    /// Displayed text (objects and control characters left out, shortened).
    pub text: String,
    /// Section index.
    pub section: usize,
}

fn block_level(doc: &Document, b: &Block) -> Option<u8> {
    match b {
        Block::Paragraph(p) => outline_level(doc, p),
        _ => None,
    }
}

/// End (exclusive) of the family that starts at block `i` of `blocks`.
fn family_end(doc: &Document, blocks: &[Block], i: usize) -> usize {
    let Some(l) = blocks.get(i).and_then(|b| block_level(doc, b)) else {
        return i + 1;
    };
    let mut j = i + 1;
    while j < blocks.len() && block_level(doc, &blocks[j]).is_none_or(|m| m > l) {
        j += 1;
    }
    j
}

/// Text shown for a paragraph in the outline.
fn shown_text(text: &str) -> String {
    let mut s: String = text
        .chars()
        .map(|c| match c {
            '\t' | '\u{2028}' | '\u{000B}' => ' ',
            c => c,
        })
        .filter(|&c| c != OBJECT && !c.is_control() && c != '\u{00AD}')
        .take(TEXT_MAX)
        .collect();
    if text.chars().count() > TEXT_MAX {
        s.push('…');
    }
    s.trim().to_owned()
}

/// `(section, block)` of a top-level body paragraph.
fn top_level(doc: &Document, para: usize) -> Result<(usize, usize)> {
    match body_paragraphs(doc).get(para).map(Vec::as_slice) {
        Some(&[s, b]) => Ok((s, b)),
        Some(_) => Err(EditError::Unsupported(
            "outline: paragraph inside a table".into(),
        )),
        None => Err(EditError::BadPath),
    }
}

/// Body paragraph id of the top-level paragraph at (`section`, `block`).
fn para_id(doc: &Document, section: usize, block: usize) -> Option<usize> {
    body_paragraphs(doc)
        .iter()
        .position(|p| p.as_slice() == [section, block])
}

impl Session {
    /// The outline: every top-level body paragraph with its outline level.
    pub fn outline(&self) -> Vec<OutlineEntry> {
        let doc = self.doc();
        body_paragraphs(doc)
            .iter()
            .enumerate()
            .filter(|(_, path)| path.len() == 2)
            .filter_map(|(id, path)| {
                let p = para_at(doc, path)?;
                Some(OutlineEntry {
                    para: id,
                    level: outline_level(doc, p),
                    text: shown_text(&bo_model::index::para_index_text(p)),
                    section: path[0],
                })
            })
            .collect()
    }

    /// Body paragraph ids of the family of `para` (first, last inclusive).
    pub fn outline_family(&self, para: usize) -> Result<(usize, usize)> {
        let doc = self.doc();
        let (si, bi) = top_level(doc, para)?;
        let blocks = &doc.sections[si].blocks;
        let end = family_end(doc, blocks, bi);
        // The last paragraph of the family (tables in it contribute their paragraphs).
        let last = body_paragraphs(doc)
            .iter()
            .rposition(|p| p[0] == si && p.get(1).is_some_and(|&b| b < end))
            .unwrap_or(para);
        Ok((para, last))
    }

    /// Give a paragraph an outline level by applying the matching heading style
    /// (`Heading1`…`Heading9`), or make it body text (`Normal`) with `None`.
    pub fn set_outline_level(&mut self, para: usize, level: Option<u8>) -> Result<Selection> {
        if body_paragraphs(self.doc()).get(para).is_none() {
            return Err(EditError::BadPath);
        }
        let path = body_paragraphs(self.doc())[para].clone();
        let has_direct =
            para_at(self.doc(), &path).is_some_and(|p| p.props.outline_level.is_some());
        if has_direct {
            // A direct outline level (§17.3.1.20) is changed in place; 9 = body text.
            return self.edit(move |tx, sel| {
                let (path, p, _) = tx.atoms(para)?;
                let mut q = p.clone();
                q.props.outline_level = Some(level.map_or(9, |l| l.min(8)));
                tx.replace_para(&path, q)?;
                Ok(sel)
            });
        }
        let id = level.map_or_else(
            || "Normal".to_owned(),
            |l| format!("Heading{}", l.min(8) + 1),
        );
        let keep = self.selection();
        self.set_selection(Selection::caret(Pos::new(para, 0)));
        let res = self.apply_style(&id);
        // The selection stays where it was.
        self.set_selection(keep);
        res.map(|_| keep)
    }

    /// Move the family of `para` above the previous one (`up`) or below the next one.
    /// A heading passes the previous/next heading of the same or a higher level with
    /// its whole family; body text passes one block. Nothing changes at either end.
    pub fn move_outline(&mut self, para: usize, up: bool) -> Result<Selection> {
        let (si, bi) = top_level(self.doc(), para)?;
        let (from, to, dest) = {
            let doc = self.doc();
            let blocks = &doc.sections[si].blocks;
            let end = family_end(doc, blocks, bi);
            let level = block_level(doc, &blocks[bi]);
            if up {
                if bi == 0 {
                    return Ok(self.selection());
                }
                let target = level
                    .and_then(|l| {
                        (0..bi)
                            .rev()
                            .find(|&j| block_level(doc, &blocks[j]).is_some_and(|m| m <= l))
                    })
                    .unwrap_or(bi - 1);
                (bi, end, target)
            } else {
                if end >= blocks.len() {
                    return Ok(self.selection());
                }
                // The next family (a heading takes its subordinate content along).
                let next_end = if level.is_some() {
                    family_end(doc, blocks, end)
                } else {
                    end + 1
                };
                (bi, end, next_end)
            }
        };
        self.move_blocks(si, from, to, si, dest)
    }

    /// Move the family of `para` before the top-level block holding body paragraph
    /// `before` (any section), or to the end of the document with `None` (drag and drop).
    pub fn move_outline_to(&mut self, para: usize, before: Option<usize>) -> Result<Selection> {
        let doc = self.doc();
        let (si, bi) = top_level(doc, para)?;
        let end = family_end(doc, &doc.sections[si].blocks, bi);
        let (ts, tb) = if let Some(b) = before {
            // A paragraph in a table: before the table.
            let path = body_paragraphs(doc)
                .get(b)
                .cloned()
                .ok_or(EditError::BadPath)?;
            (path[0], path[1])
        } else {
            let last = doc.sections.len().saturating_sub(1);
            (last, doc.sections.get(last).map_or(0, |s| s.blocks.len()))
        };
        self.move_blocks(si, bi, end, ts, tb)
    }

    /// Move blocks `from..to` of section `si` before block `dest` of section `ts`
    /// (indexes as before the move); the caret goes to the start of the first moved block.
    fn move_blocks(
        &mut self,
        si: usize,
        from: usize,
        to: usize,
        ts: usize,
        dest: usize,
    ) -> Result<Selection> {
        if si == ts && (from..=to).contains(&dest) {
            return Ok(self.selection());
        }
        let n = to - from;
        if si != ts && self.doc().sections[si].blocks.len() == n {
            // A section keeps at least one block.
            return Err(EditError::Unsupported(
                "outline: section would be empty".into(),
            ));
        }
        self.edit(move |tx, _| {
            let first = if si == ts {
                // One op over the touched range: the moved family and what it passes.
                let (lo, hi) = (from.min(dest), to.max(dest));
                let old: Vec<Block> = tx.doc.sections[si].blocks[lo..hi].to_vec();
                let moved = old[from - lo..to - lo].to_vec();
                let mut rest = old.clone();
                rest.drain(from - lo..to - lo);
                let at = if dest < from {
                    dest - lo
                } else {
                    dest - lo - n
                };
                let mut new = rest;
                new.splice(at..at, moved);
                tx.push(Op::Blocks {
                    path: vec![si],
                    index: lo,
                    remove: old,
                    insert: new,
                })?;
                lo + at
            } else {
                let moved: Vec<Block> = tx.doc.sections[si].blocks[from..to].to_vec();
                tx.push(Op::Blocks {
                    path: vec![si],
                    index: from,
                    remove: moved.clone(),
                    insert: Vec::new(),
                })?;
                tx.push(Op::Blocks {
                    path: vec![ts],
                    index: dest,
                    remove: Vec::new(),
                    insert: moved,
                })?;
                dest
            };
            let id = (first..)
                .take(n)
                .find_map(|b| para_id(tx.doc, ts, b))
                .unwrap_or(0);
            Ok(Selection::caret(Pos::new(id, 0)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{ParaProps, Paragraph, Run, RunProps, Section};

    fn p(text: &str, level: Option<u8>) -> Block {
        let props = ParaProps {
            outline_level: level,
            ..ParaProps::default()
        };
        Block::Paragraph(Paragraph::new(
            props,
            vec![Run::text(text, RunProps::default())],
        ))
    }

    /// H1 A, body a, H2 A.1, body a1, H1 B, body b.
    fn session() -> Session {
        Session::new(Document {
            sections: vec![Section::new(vec![
                p("A", Some(0)),
                p("a", None),
                p("A.1", Some(1)),
                p("a1", None),
                p("B", Some(0)),
                p("b", None),
            ])],
            ..Default::default()
        })
    }

    fn texts(s: &Session) -> Vec<String> {
        (0..s.paragraph_count())
            .map(|i| s.para_text(i).unwrap_or_default())
            .collect()
    }

    #[test]
    fn outline_lists_levels_and_families() {
        let s = session();
        let o = s.outline();
        assert_eq!(o.len(), 6);
        assert_eq!(o[0].level, Some(0));
        assert_eq!(o[1].level, None);
        assert_eq!(o[2].text, "A.1");
        assert_eq!(s.outline_family(0).unwrap(), (0, 3));
        assert_eq!(s.outline_family(2).unwrap(), (2, 3));
        assert_eq!(s.outline_family(1).unwrap(), (1, 1));
    }

    #[test]
    fn move_heading_down_with_its_content_and_undo() {
        let mut s = session();
        let sel = s.move_outline(0, false).unwrap();
        assert_eq!(texts(&s), ["B", "b", "A", "a", "A.1", "a1"]);
        assert_eq!(sel.focus.para, 2);
        assert!(s.undo());
        assert_eq!(texts(&s), ["A", "a", "A.1", "a1", "B", "b"]);
        assert!(s.redo());
        assert_eq!(texts(&s), ["B", "b", "A", "a", "A.1", "a1"]);
    }

    #[test]
    fn move_heading_up_passes_previous_sibling_family() {
        let mut s = session();
        let sel = s.move_outline(4, true).unwrap();
        assert_eq!(texts(&s), ["B", "b", "A", "a", "A.1", "a1"]);
        assert_eq!(sel.focus.para, 0);
        // At the top nothing moves.
        s.move_outline(0, true).unwrap();
        assert_eq!(texts(&s)[0], "B");
    }

    #[test]
    fn move_body_text_one_block() {
        let mut s = session();
        s.move_outline(1, false).unwrap();
        assert_eq!(texts(&s), ["A", "A.1", "a", "a1", "B", "b"]);
    }

    #[test]
    fn drag_heading_before_another() {
        let mut s = session();
        // A.1 with its body text to the end of the document.
        s.move_outline_to(2, None).unwrap();
        assert_eq!(texts(&s), ["A", "a", "B", "b", "A.1", "a1"]);
        s.undo();
        // B before A.
        s.move_outline_to(4, Some(0)).unwrap();
        assert_eq!(texts(&s), ["B", "b", "A", "a", "A.1", "a1"]);
        // Into itself: no change, no undo step.
        let v = s.version();
        s.move_outline_to(0, Some(1)).unwrap();
        assert_eq!(s.version(), v);
    }

    #[test]
    fn promote_demote_through_heading_styles() {
        let mut s = session();
        s.set_outline_level(1, Some(1)).unwrap();
        assert_eq!(s.outline()[1].level, Some(1));
        assert!(s.doc().style("Heading2").is_some());
        s.set_outline_level(1, None).unwrap();
        assert_eq!(s.outline()[1].level, None);
        s.undo();
        assert_eq!(s.outline()[1].level, Some(1));
        // A direct outline level changes in place (9 = body text).
        s.set_outline_level(0, Some(2)).unwrap();
        assert_eq!(s.outline()[0].level, Some(2));
        s.set_outline_level(0, None).unwrap();
        assert_eq!(s.outline()[0].level, None);
    }
}

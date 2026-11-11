//! Content controls (ECMA-376 Part 1, §17.5.2 sdt) beyond filling one in: block-level controls
//! around paragraphs, repeating sections ([MS-DOCX] w15:repeatingSection /
//! w15:repeatingSectionItem), the properties of a control (§17.5.2.1 alias, §17.5.2.42 tag,
//! §17.5.2.23 lock) and placeholders (§17.5.2.39 showingPlcHdr).

use crate::para::{self, Atom};
use crate::{max_control_id, split_path, EditError, Pos, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Block, ContentControl, ControlKind, RunContent, RunProps};

/// A block-level control: its properties, and where its start and end markers are (body
/// paragraph id, atom index).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockSpan {
    pub cc: ContentControl,
    pub first: usize,
    pub first_atom: usize,
    pub last: usize,
    pub last_atom: usize,
    /// Nesting depth (0 = outermost).
    pub depth: usize,
}

/// Properties of a content control the properties dialog changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlProps {
    /// §17.5.2.1 alias (the title; empty removes it).
    #[serde(default)]
    pub title: String,
    /// §17.5.2.42 tag (empty removes it).
    #[serde(default)]
    pub tag: String,
    /// The control cannot be deleted (§17.5.2.23 lock `sdtLocked`).
    #[serde(default)]
    pub delete_locked: bool,
    /// Its contents cannot be edited (`contentLocked`; both: `sdtContentLocked`).
    #[serde(default)]
    pub content_locked: bool,
}

/// §17.5.2.23 lock value of the two check boxes.
fn lock_value(delete: bool, content: bool) -> Option<String> {
    match (delete, content) {
        (true, true) => Some("sdtContentLocked".into()),
        (true, false) => Some("sdtLocked".into()),
        (false, true) => Some("contentLocked".into()),
        (false, false) => None,
    }
}

impl Session {
    /// Every block-level control of the main text, in the order they end.
    pub(crate) fn block_spans(&self) -> Vec<BlockSpan> {
        let mut out = Vec::new();
        let mut open: Vec<(ContentControl, usize, usize)> = Vec::new();
        for (pid, path) in body_paragraphs(&self.doc).iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            for (i, (_, c)) in para::flatten(p).iter().enumerate() {
                match c {
                    RunContent::ControlStart(cc) if cc.block => {
                        open.push(((**cc).clone(), pid, i));
                    }
                    RunContent::ControlEnd(true) => {
                        if let Some((cc, first, first_atom)) = open.pop() {
                            out.push(BlockSpan {
                                cc,
                                first,
                                first_atom,
                                last: pid,
                                last_atom: i,
                                depth: open.len(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// The innermost block-level control around the caret's paragraph matching `pick`.
    fn block_span_at(&self, pick: impl Fn(&ContentControl) -> bool) -> Option<BlockSpan> {
        let pid = self.selection.focus.para;
        self.block_spans()
            .into_iter()
            .filter(|s| s.first <= pid && pid <= s.last && pick(&s.cc))
            .max_by_key(|s| s.depth)
    }

    /// The block-level control around the caret's paragraph (the innermost; an item of a
    /// repeating section stands for its section).
    #[must_use]
    pub fn block_control_at_caret(&self) -> Option<ContentControl> {
        let s = self.block_span_at(|_| true)?;
        if s.cc.kind == ControlKind::RepeatingSectionItem {
            return self
                .block_span_at(|c| c.kind == ControlKind::RepeatingSection)
                .map(|o| o.cc)
                .or(Some(s.cc));
        }
        Some(s.cc)
    }

    /// The caret is in an item of a repeating section (one can be added after it).
    #[must_use]
    pub fn in_repeating_item(&self) -> bool {
        self.block_span_at(|c| c.kind == ControlKind::RepeatingSectionItem)
            .is_some()
    }

    /// The content range (paragraph, from, to) of the innermost inline control around the
    /// caret.
    pub fn inline_control_range(&self) -> Option<(ContentControl, usize, usize, usize)> {
        let caret = self.selection.focus;
        let path = body_paragraphs(&self.doc).get(caret.para)?.clone();
        let atoms = para::flatten(para_at(&self.doc, &path)?);
        let mut pos = 0;
        let mut open: Vec<(usize, usize)> = Vec::new();
        for (i, (_, c)) in atoms.iter().enumerate() {
            match c {
                RunContent::ControlStart(cc) if !cc.block => open.push((i, pos)),
                RunContent::ControlEnd(false) => {
                    if let Some((s, from)) = open.pop() {
                        if from <= caret.offset && caret.offset <= pos {
                            if let RunContent::ControlStart(cc) = &atoms[s].1 {
                                return Some(((**cc).clone(), caret.para, from, pos));
                            }
                        }
                    }
                }
                _ => {}
            }
            pos += para::atom_len(c);
        }
        None
    }

    /// A click in a control showing its placeholder selects the whole placeholder, so typing
    /// replaces it (§17.5.2.39 showingPlcHdr). Returns whether the selection changed.
    pub fn select_control_placeholder(&mut self) -> bool {
        match self.inline_control_range() {
            Some((cc, para, from, to)) if cc.placeholder && from < to => {
                let sel = Selection {
                    anchor: Pos::new(para, from),
                    focus: Pos::new(para, to),
                };
                let changed = sel != self.selection;
                self.selection = sel;
                changed
            }
            _ => false,
        }
    }

    /// Select the content of the inline control around the caret (to replace a picture control's
    /// picture, for instance). Returns whether there is one.
    pub fn select_control_content(&mut self) -> bool {
        let Some((_, para, from, to)) = self.inline_control_range() else {
            return false;
        };
        self.selection = Selection {
            anchor: Pos::new(para, from),
            focus: Pos::new(para, to),
        };
        true
    }

    /// A check box control's content is selected as a whole (Space then toggles it).
    #[must_use]
    pub fn checkbox_selected(&self) -> bool {
        let (a, b) = self.selection.ordered();
        matches!(
            self.inline_control_range(),
            Some((cc, para, from, to))
                if matches!(cc.kind, ControlKind::Checkbox { .. })
                    && a == Pos::new(para, from)
                    && b == Pos::new(para, to)
                    && from < to
        )
    }

    /// The extent of the control around the caret: the content of the inline one, else the
    /// paragraphs of the block-level one (a repeating section as a whole).
    #[must_use]
    pub fn control_extent(&self) -> Option<Selection> {
        if let Some((_, para, from, to)) = self.inline_control_range() {
            return Some(Selection {
                anchor: Pos::new(para, from),
                focus: Pos::new(para, to),
            });
        }
        let s = self
            .block_span_at(|c| c.kind == ControlKind::RepeatingSection)
            .or_else(|| self.block_span_at(|_| true))?;
        let len = self.para_text(s.last).map_or(0, |t| t.len());
        Some(Selection {
            anchor: Pos::new(s.first, 0),
            focus: Pos::new(s.last, len),
        })
    }

    /// Wrap the paragraphs of the selection in a repeating section ([MS-DOCX]
    /// w15:repeatingSection) holding one item (w15:repeatingSectionItem), both block-level
    /// controls (§17.5.2.29 sdt). A control that only partly covers the paragraphs refuses it.
    pub fn insert_repeating_section(&mut self, title: &str) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let (pa, pb) = (
            paths.get(a.para).ok_or(EditError::BadPosition)?,
            paths.get(b.para).ok_or(EditError::BadPosition)?,
        );
        if split_path(pa).0 != split_path(pb).0 {
            return Err(EditError::Unsupported(
                "repeating section across cells".into(),
            ));
        }
        let partly = self.block_spans().into_iter().any(|s| {
            let inside = |p: usize| a.para <= p && p <= b.para;
            inside(s.first) != inside(s.last)
        });
        if partly {
            return Err(EditError::Unsupported(
                "repeating section across a content control".into(),
            ));
        }
        let mut id = max_control_id(&self.doc);
        let mut new_cc = |kind: ControlKind, alias: Option<String>| {
            id += 1;
            ContentControl {
                id: Some(id),
                alias,
                tag: None,
                kind,
                lock: None,
                placeholder: false,
                block: true,
                props_xml: None,
                data_binding: None,
            }
        };
        let title = title.trim();
        let section = new_cc(
            ControlKind::RepeatingSection,
            (!title.is_empty()).then(|| title.to_owned()),
        );
        let item = new_cc(ControlKind::RepeatingSectionItem, None);
        let sel = self.selection;
        self.edit(|tx, _| {
            let start = |cc: ContentControl| -> Atom {
                (RunProps::default(), RunContent::ControlStart(Box::new(cc)))
            };
            let end: Atom = (RunProps::default(), RunContent::ControlEnd(true));
            let (path, p, mut atoms) = tx.atoms(a.para)?;
            atoms.insert(0, start(item));
            atoms.insert(0, start(section));
            if a.para == b.para {
                atoms.push(end.clone());
                atoms.push(end);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            } else {
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                let (path, p, mut atoms) = tx.atoms(b.para)?;
                atoms.push(end.clone());
                atoms.push(end);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })
    }

    /// Add an item after the item of the repeating section around the caret ([MS-DOCX]
    /// w15:repeatingSectionItem): a copy of it, with new control ids; the caret goes to the
    /// start of the new item. Returns whether an item was added (one undo step).
    pub fn add_repeating_item(&mut self) -> Result<bool> {
        let Some(item) = self.block_span_at(|c| c.kind == ControlKind::RepeatingSectionItem) else {
            return Ok(false);
        };
        let section = self.block_span_at(|c| c.kind == ControlKind::RepeatingSection);
        if section.as_ref().is_some_and(|s| {
            matches!(
                s.cc.lock.as_deref(),
                Some("contentLocked" | "sdtContentLocked")
            )
        }) {
            return Ok(false);
        }
        let paths = body_paragraphs(&self.doc);
        let mut next_id = max_control_id(&self.doc);
        let mut copies: Vec<Block> = Vec::new();
        for pid in item.first..=item.last {
            let p = paths
                .get(pid)
                .and_then(|path| para_at(&self.doc, path))
                .ok_or(EditError::BadPosition)?;
            let mut atoms = para::flatten(p);
            // The copy ends with the controls the item's last paragraph closes after it, so the
            // original item's paragraph loses them; the copy starts at the item's own start.
            if pid == item.first {
                let mut k = 0;
                atoms.retain(|a| {
                    let keep = k >= item.first_atom
                        || !matches!(&a.1, RunContent::ControlStart(cc) if cc.block);
                    k += 1;
                    keep
                });
            }
            for a in &mut atoms {
                if let RunContent::ControlStart(cc) = &mut a.1 {
                    next_id += 1;
                    cc.id = Some(next_id);
                    cc.data_binding = None;
                }
            }
            let mut q = para::with_atoms(p, atoms);
            q.src = None;
            copies.push(Block::Paragraph(q));
        }
        let first_new = item.last + 1;
        self.edit(|tx, _| {
            let (path, p, atoms) = tx.atoms(item.last)?;
            let mut k = 0;
            let kept: Vec<Atom> = atoms
                .into_iter()
                .filter(|a| {
                    let keep = k <= item.last_atom || !matches!(a.1, RunContent::ControlEnd(true));
                    k += 1;
                    keep
                })
                .collect();
            tx.replace_para(&path, para::with_atoms(&p, kept))?;
            let (prefix, index) = split_path(&path);
            tx.insert_blocks(prefix, index + 1, copies)?;
            Ok(Selection::caret(Pos::new(first_new, 0)))
        })?;
        self.fix_selection();
        Ok(true)
    }

    /// Change the title, tag and locks of the control around the caret (inline, else the
    /// innermost block-level one). Returns whether a control was changed (one undo step).
    pub fn set_control_props(&mut self, props: &ControlProps) -> Result<bool> {
        let at = match self.control_span() {
            Some((_, pid, s, _)) => Some((pid, s)),
            None => self
                .block_span_at(|c| c.kind != ControlKind::RepeatingSectionItem)
                .map(|b| (b.first, b.first_atom)),
        };
        let Some((pid, s)) = at else {
            return Ok(false);
        };
        let opt = |v: &str| {
            let v = v.trim();
            (!v.is_empty()).then(|| v.to_owned())
        };
        let sel = self.selection;
        let mut changed = false;
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(pid)?;
            let Some((_, RunContent::ControlStart(cc))) = atoms.get_mut(s) else {
                return Err(EditError::BadPosition);
            };
            let before = (**cc).clone();
            cc.alias = opt(&props.title);
            cc.tag = opt(&props.tag);
            cc.lock = lock_value(props.delete_locked, props.content_locked);
            changed = **cc != before;
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(sel)
        })?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Document, Paragraph, Run, Section};

    fn doc_of(texts: &[&str]) -> Document {
        Document {
            sections: vec![Section {
                blocks: texts
                    .iter()
                    .map(|t| {
                        Block::Paragraph(Paragraph {
                            runs: vec![Run {
                                props: RunProps::default(),
                                content: vec![RunContent::Text((*t).into())],
                            }],
                            ..Paragraph::default()
                        })
                    })
                    .collect(),
                ..Section::default()
            }],
            ..Document::default()
        }
    }

    fn kinds(s: &Session) -> Vec<(String, usize, usize, usize)> {
        s.block_spans()
            .into_iter()
            .map(|b| (format!("{:?}", b.cc.kind), b.first, b.last, b.depth))
            .collect()
    }

    #[test]
    fn repeating_section_insert_and_add_items() {
        let mut s = Session::new(doc_of(&["Trước", "Mục: Tên", "Sau"]));
        s.set_selection(Selection::caret(Pos::new(1, 2)));
        s.insert_repeating_section("Thành viên").unwrap();
        assert_eq!(
            kinds(&s),
            vec![
                ("RepeatingSectionItem".into(), 1, 1, 1),
                ("RepeatingSection".into(), 1, 1, 0),
            ]
        );
        assert!(s.in_repeating_item());
        assert_eq!(
            s.block_control_at_caret().map(|c| c.alias),
            Some(Some("Thành viên".into()))
        );
        // The "+" of the item: a copy after it, inside the same section; caret at its start.
        assert!(s.add_repeating_item().unwrap());
        assert_eq!(s.selection(), Selection::caret(Pos::new(2, 0)));
        assert_eq!(s.para_text(2).as_deref(), Some("Mục: Tên"));
        assert_eq!(s.para_text(3).as_deref(), Some("Sau"));
        assert_eq!(
            kinds(&s),
            vec![
                ("RepeatingSectionItem".into(), 1, 1, 1),
                ("RepeatingSectionItem".into(), 2, 2, 1),
                ("RepeatingSection".into(), 1, 2, 0),
            ]
        );
        // Ids stay unique.
        let ids: Vec<Option<i64>> = s.block_spans().iter().map(|b| b.cc.id).collect();
        let mut u = ids.clone();
        u.sort_unstable();
        u.dedup();
        assert_eq!(u.len(), ids.len());
        // From the first item again: the new one goes between the two.
        s.set_selection(Selection::caret(Pos::new(1, 0)));
        assert!(s.add_repeating_item().unwrap());
        assert_eq!(kinds(&s).last().map(|k| (k.1, k.2)), Some((1, 3)));
        // One undo step each.
        assert!(s.undo());
        assert!(s.undo());
        assert_eq!(kinds(&s).len(), 2);
        // Outside an item nothing is added.
        s.set_selection(Selection::caret(Pos::new(0, 0)));
        assert!(!s.add_repeating_item().unwrap());
    }

    #[test]
    fn repeating_section_over_several_paragraphs() {
        let mut s = Session::new(doc_of(&["A", "B", "C"]));
        s.set_selection(Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(1, 1),
        });
        s.insert_repeating_section("").unwrap();
        s.set_selection(Selection::caret(Pos::new(1, 1)));
        assert!(s.add_repeating_item().unwrap());
        assert_eq!(s.selection(), Selection::caret(Pos::new(2, 0)));
        let texts: Vec<String> = (0..5).filter_map(|i| s.para_text(i)).collect();
        assert_eq!(texts, ["A", "B", "A", "B", "C"]);
        assert_eq!(
            kinds(&s).last().map(|k| (k.0.clone(), k.1, k.2)),
            Some(("RepeatingSection".into(), 0, 3))
        );
    }

    #[test]
    fn control_properties_and_placeholder_selection() {
        let mut s = Session::new(doc_of(&["Tên: "]));
        s.set_selection(Selection::caret(Pos::new(0, 5)));
        s.insert_control("text", "Nhập tên.", &[]).unwrap();
        // A click inside the placeholder selects all of it.
        s.set_selection(Selection::caret(Pos::new(0, 8)));
        assert!(s.select_control_placeholder());
        assert_eq!(
            s.selection(),
            Selection {
                anchor: Pos::new(0, 5),
                focus: Pos::new(0, 5 + "Nhập tên.".len()),
            }
        );
        let props = ControlProps {
            title: "Họ tên".into(),
            tag: "ho_ten".into(),
            delete_locked: true,
            content_locked: false,
        };
        assert!(s.set_control_props(&props).unwrap());
        let cc = s.control_at_caret().unwrap();
        assert_eq!(cc.alias.as_deref(), Some("Họ tên"));
        assert_eq!(cc.tag.as_deref(), Some("ho_ten"));
        assert_eq!(cc.lock.as_deref(), Some("sdtLocked"));
        // Same values: nothing changes.
        assert!(!s.set_control_props(&props).unwrap());
        // Typed text is no placeholder: a click no longer selects it.
        s.insert_text("An").unwrap();
        s.set_selection(Selection::caret(Pos::new(0, 6)));
        assert!(!s.select_control_placeholder());
    }
}

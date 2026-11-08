//! Floating pictures (ECMA-376 Part 1, §20.4.2.3 anchor): moving a picture to
//! another position. With track changes on, a move is recorded as the picture's
//! removal from its old position (§17.13.5.22 moveFrom) and its insertion at the new
//! one (§17.13.5.25 moveTo); rejecting it brings the picture back unchanged.

use crate::para::{self, Atom};
use crate::{EditError, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Emu, RevMark, RunContent};
use serde::Serialize;

/// The floating picture at the selection (picture position dialog).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureState {
    /// Offsets in points from the reference of `positionH` / `positionV`
    /// (§20.4.2.10, §20.4.2.11 posOffset).
    pub x: f32,
    /// Vertical offset in points (`positionV/posOffset`).
    pub y: f32,
    /// `relativeFrom` of `positionH` / `positionV` (§20.4.3.4 `ST_RelFromH`,
    /// §20.4.3.5 `ST_RelFromV`).
    pub h_relative: String,
    /// `relativeFrom` of `positionV` (§20.4.3.5 `ST_RelFromV`).
    pub v_relative: String,
}

/// Index of the floating picture an edit applies to in a paragraph's atoms: the one
/// inside `range` (byte offsets), else the first one of the paragraph. Pictures
/// deleted by a tracked change are skipped.
fn floating_at(atoms: &[Atom], range: Option<(usize, usize)>) -> Option<usize> {
    let mut off = 0;
    let mut first = None;
    for (i, (props, item)) in atoms.iter().enumerate() {
        let l = para::atom_len(item);
        if let RunContent::Image(img) = item {
            if img.anchor.is_some() && props.del.is_none() {
                if let Some((a, b)) = range {
                    if a <= off && off + l <= b && b > a {
                        return Some(i);
                    }
                }
                first.get_or_insert(i);
            }
        }
        off += l;
    }
    first
}

impl Session {
    /// The floating picture of the caret's paragraph (the selected one first).
    pub fn picture_state(&self) -> Option<PictureState> {
        let (a, b) = self.selection.ordered();
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(a.para)?)?;
        let atoms = para::flatten(p);
        let range = (a.para == b.para).then_some((a.offset, b.offset));
        let i = floating_at(&atoms, range)?;
        let RunContent::Image(img) = &atoms[i].1 else {
            return None;
        };
        let anchor = img.anchor.as_ref()?;
        Some(PictureState {
            x: anchor.x.to_pt(),
            y: anchor.y.to_pt(),
            h_relative: anchor.h_relative.clone(),
            v_relative: anchor.v_relative.clone(),
        })
    }

    /// Move the floating picture at the selection to offsets (`x`, `y`) points from
    /// its position references (§20.4.2.10 positionH / §20.4.2.11 positionV posOffset).
    /// In track-changes mode the move is a tracked move (moveFrom + moveTo); moving a
    /// picture the author inserted or already moved changes it in place.
    pub fn set_picture_position(&mut self, x: f32, y: f32) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let id = a.para;
        let range = (a.para == b.para).then_some((a.offset, b.offset));
        let paths = body_paragraphs(&self.doc);
        let p = paths
            .get(id)
            .and_then(|path| para_at(&self.doc, path))
            .ok_or(EditError::BadPosition)?;
        let atoms = para::flatten(p);
        let k = floating_at(&atoms, range)
            .ok_or_else(|| EditError::Unsupported("no floating picture here".into()))?;
        let (nx, ny) = (Emu::from_pt(x), Emu::from_pt(y));
        if let RunContent::Image(img) = &atoms[k].1 {
            if img.anchor.as_ref().is_some_and(|a| a.x == nx && a.y == ny) {
                return Ok(self.selection);
            }
        }
        self.edit(move |tx, sel| {
            let (path, p, mut atoms) = tx.atoms(id)?;
            let (props, item) = atoms[k].clone();
            let RunContent::Image(img) = &item else {
                return Err(EditError::BadPosition);
            };
            let mut moved = img.clone();
            if let Some(a) = moved.anchor.as_mut() {
                a.x = nx;
                a.y = ny;
            }
            match tx.track.as_mut() {
                Some(tr) if !tr.owns(&props) => {
                    let m = RevMark {
                        moved: true,
                        ..tr.mark()
                    };
                    let mut from = props.clone();
                    from.del = Some(m.clone());
                    let mut to = props.without_revisions();
                    to.ins = Some(m);
                    atoms[k] = (from, item.clone());
                    atoms.insert(k + 1, (to, RunContent::Image(moved)));
                }
                _ => atoms[k] = (props, RunContent::Image(moved)),
            }
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(self.selection)
    }
}

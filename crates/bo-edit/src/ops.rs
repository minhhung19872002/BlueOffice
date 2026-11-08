//! Atomic, invertible operations on a [`Document`] (prompt §5.2).
//!
//! Every edit is a [`Transaction`] of [`Op`]s; each op carries what it removes,
//! so its inverse is exact and undo/redo is unlimited. Ops address block
//! containers by path (`[section, (table block, row, cell)*]`, see
//! `bo_model::index`), which maps one-to-one to CRDT operations later (Phase 6).

use crate::EditError;
use bo_model::change::BlockChanges;
use bo_model::index::blocks_at_mut;
use bo_model::{Block, Comment, Document, Numbering, Section, StyleSheet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An atomic edit.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Replace `remove` (currently at `index`) by `insert` in the block container at `path`.
    Blocks {
        /// Path prefix addressing the block container (see `bo_model::index::blocks_at`).
        path: Vec<usize>,
        /// Index of the first replaced block.
        index: usize,
        /// Blocks removed.
        remove: Vec<Block>,
        /// Blocks inserted in their place.
        insert: Vec<Block>,
    },
    /// Replace sections starting at `index`.
    Sections {
        /// Index of the first replaced section.
        index: usize,
        /// Sections removed.
        remove: Vec<Section>,
        /// Sections inserted in their place.
        insert: Vec<Section>,
    },
    /// Add, replace or remove a media part.
    Media {
        /// Part name of the media.
        name: String,
        /// Previous bytes (`None` = absent).
        old: Option<Arc<Vec<u8>>>,
        /// New bytes (`None` = removed).
        new: Option<Arc<Vec<u8>>>,
    },
    /// Replace the numbering definitions.
    Numbering {
        /// Previous definitions.
        old: Box<Numbering>,
        /// New definitions.
        new: Box<Numbering>,
    },
    /// Replace the style sheet.
    Styles {
        /// Previous style sheet.
        old: Box<StyleSheet>,
        /// New style sheet.
        new: Box<StyleSheet>,
    },
    /// Replace the comments (§17.13.4).
    Comments {
        /// Previous comments.
        old: Vec<Comment>,
        /// New comments.
        new: Vec<Comment>,
    },
    /// Replace the footnotes (`endnote` false) or endnotes (§17.11).
    Notes {
        /// `true` for endnotes, `false` for footnotes.
        endnote: bool,
        /// Previous notes by id.
        old: BTreeMap<i32, Vec<Block>>,
        /// New notes by id.
        new: BTreeMap<i32, Vec<Block>>,
    },
    /// Set the evenAndOddHeaders document setting (§17.15.1.35).
    EvenAndOddHeaders {
        /// Previous value.
        old: bool,
        /// New value.
        new: bool,
    },
}

impl Op {
    /// The exact inverse.
    #[must_use]
    pub fn inverse(&self) -> Op {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => Op::Blocks {
                path: path.clone(),
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            Op::Sections {
                index,
                remove,
                insert,
            } => Op::Sections {
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            Op::Media { name, old, new } => Op::Media {
                name: name.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Op::Numbering { old, new } => Op::Numbering {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Styles { old, new } => Op::Styles {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Comments { old, new } => Op::Comments {
                old: new.clone(),
                new: old.clone(),
            },
            Op::Notes { endnote, old, new } => Op::Notes {
                endnote: *endnote,
                old: new.clone(),
                new: old.clone(),
            },
            Op::EvenAndOddHeaders { old, new } => Op::EvenAndOddHeaders {
                old: *new,
                new: *old,
            },
        }
    }

    /// Record which top-level blocks this op changes (incremental layout).
    pub fn record(&self, changes: &mut BlockChanges) {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => match path.as_slice() {
                [section] => changes.splice(*section, *index, remove.len(), insert.len()),
                // Nested container (table cell): its top-level table block changed.
                [section, block, ..] => changes.splice(*section, *block, 1, 1),
                [] => changes.mark_all(),
            },
            // Header/footer edits (ECMA-376 Part 1, §17.10): the section is replaced
            // by a copy that differs only there.
            Op::Sections {
                index,
                remove,
                insert,
            } if remove.len() == insert.len()
                && remove
                    .iter()
                    .zip(insert)
                    .all(|(a, b)| a.same_except_headers(b)) =>
            {
                changes.mark_headers(*index);
            }
            // §17.15.1.35 evenAndOddHeaders only selects which header a page shows.
            Op::EvenAndOddHeaders { .. } => changes.mark_headers(0),
            _ => changes.mark_all(),
        }
    }

    /// Apply to `doc`, checking that the removed content matches.
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError> {
        match self {
            Op::Blocks {
                path,
                index,
                remove,
                insert,
            } => {
                let blocks = blocks_at_mut(doc, path).ok_or(EditError::BadPath)?;
                let end = index + remove.len();
                if end > blocks.len() || blocks[*index..end] != remove[..] {
                    return Err(EditError::Conflict);
                }
                blocks.splice(*index..end, insert.iter().cloned());
            }
            Op::Sections {
                index,
                remove,
                insert,
            } => {
                let end = index + remove.len();
                if end > doc.sections.len() || doc.sections[*index..end] != remove[..] {
                    return Err(EditError::Conflict);
                }
                doc.sections.splice(*index..end, insert.iter().cloned());
            }
            Op::Media { name, old, new } => {
                if doc.media.get(name) != old.as_ref() {
                    return Err(EditError::Conflict);
                }
                match new {
                    Some(b) => doc.media.insert(name.clone(), b.clone()),
                    None => doc.media.remove(name),
                };
            }
            Op::Numbering { old, new } => {
                if doc.numbering != **old {
                    return Err(EditError::Conflict);
                }
                doc.numbering = (**new).clone();
            }
            Op::Styles { old, new } => {
                if doc.styles != **old {
                    return Err(EditError::Conflict);
                }
                doc.styles = (**new).clone();
            }
            Op::Comments { old, new } => {
                if doc.comments != *old {
                    return Err(EditError::Conflict);
                }
                doc.comments.clone_from(new);
            }
            Op::Notes { endnote, old, new } => {
                let notes = if *endnote {
                    &mut doc.endnotes
                } else {
                    &mut doc.footnotes
                };
                if notes != old {
                    return Err(EditError::Conflict);
                }
                notes.clone_from(new);
            }
            Op::EvenAndOddHeaders { old, new } => {
                if doc.even_and_odd_headers != *old {
                    return Err(EditError::Conflict);
                }
                doc.even_and_odd_headers = *new;
            }
        }
        Ok(())
    }
}

/// Caret position: body paragraph id (document order) and byte offset in its index text.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Pos {
    /// Body paragraph id (document order).
    #[serde(rename = "p")]
    pub para: usize,
    /// Byte offset in the paragraph's index text.
    #[serde(rename = "o")]
    pub offset: usize,
}

impl Pos {
    /// Position at byte `offset` of body paragraph `para`.
    pub fn new(para: usize, offset: usize) -> Pos {
        Pos { para, offset }
    }
}

/// Selection: `anchor` stays put, `focus` moves with Shift+arrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Selection {
    /// End that stays put.
    pub anchor: Pos,
    /// End that moves with the caret.
    pub focus: Pos,
}

impl Selection {
    /// Collapsed selection at `p`.
    pub fn caret(p: Pos) -> Selection {
        Selection {
            anchor: p,
            focus: p,
        }
    }
    /// `anchor` and `focus` coincide.
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
    /// (start, end) in document order.
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
}

/// A group of ops undone/redone together.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Ops in application order.
    pub ops: Vec<Op>,
    /// Selection before the transaction (restored on undo).
    pub before: Selection,
    /// Selection after the transaction (restored on redo).
    pub after: Selection,
}

impl Transaction {
    /// Apply all ops; on failure the document is restored.
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError> {
        for (i, op) in self.ops.iter().enumerate() {
            if let Err(e) = op.apply(doc) {
                for done in self.ops[..i].iter().rev() {
                    let _ = done.inverse().apply(doc);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// The inverse transaction.
    #[must_use]
    pub fn inverse(&self) -> Transaction {
        Transaction {
            ops: self.ops.iter().rev().map(Op::inverse).collect(),
            before: self.after,
            after: self.before,
        }
    }
}

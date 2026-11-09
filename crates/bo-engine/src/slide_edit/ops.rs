//! Invertible edit operations on a [`Presentation`] (prompt §5.2, ADR-0008).
//!
//! Every [`SlideOp`] carries the content it removes, so its inverse is exact
//! and `apply` can verify that the model still holds what the op expects
//! (stale operations are rejected instead of corrupting the deck).

use bo_model::slide::{
    Extra, Notes, Presentation, RawPart, Rel, ShapeNode, Slide, Timing, Transition,
};

/// Slide-level fields that are not part of the shape tree.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlideFields {
    /// Relationships of the slide part (ECMA-376 Part 2, §9.3).
    pub rels: Vec<Rel>,
    /// Speaker notes (§19.3.1.26 notes).
    pub notes: Option<Notes>,
    /// Animation timing tree (§19.3.1.48 timing).
    pub timing: Option<Timing>,
    /// Slide transition (§19.3.1.50 transition).
    pub transition: Option<Transition>,
}

impl SlideFields {
    /// Snapshot of these fields of `s`.
    pub fn of(s: &Slide) -> SlideFields {
        SlideFields {
            rels: s.info.rels.clone(),
            notes: s.notes.clone(),
            timing: s.timing.clone(),
            transition: s.transition.clone(),
        }
    }
    fn store(self, s: &mut Slide) {
        s.info.rels = self.rels;
        s.notes = self.notes;
        s.timing = self.timing;
        s.transition = self.transition;
    }
}

/// Presentation-level fields touched by edits (notes master registration, layouts,
/// masters and themes imported with slides pasted from another deck).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PresFields {
    /// Relationships of the presentation part.
    pub rels: Vec<Rel>,
    /// Preserved `presentation.xml` children (e.g. `p:notesMasterIdLst`).
    pub extra: Vec<Extra>,
    /// Slide layouts (§19.3.1.39 sldLayout).
    pub layouts: Vec<bo_model::slide::SlideLayout>,
    /// Slide masters (§19.3.1.42 sldMaster).
    pub masters: Vec<bo_model::slide::SlideMaster>,
    /// Themes (§20.1.6.9 theme).
    pub themes: Vec<bo_model::slide::Theme>,
    /// Slide size (§19.2.1.39 sldSz).
    pub slide_size: bo_model::slide::SlideSize,
}

impl PresFields {
    /// Snapshot of these fields of `p`.
    pub fn of(p: &Presentation) -> PresFields {
        PresFields {
            rels: p.info.rels.clone(),
            extra: p.extra.clone(),
            layouts: p.layouts.clone(),
            masters: p.masters.clone(),
            themes: p.themes.clone(),
            slide_size: p.slide_size.clone(),
        }
    }
}

/// One atomic, invertible edit.
#[derive(Debug, Clone, PartialEq)]
pub enum SlideOp {
    /// Splice the children of a container: the spTree of `slide` (`parent`
    /// empty) or the group at index path `parent`.
    Nodes {
        /// Slide index.
        slide: usize,
        /// Index path of the group from the spTree (empty: the spTree itself).
        parent: Vec<usize>,
        /// First child replaced.
        index: usize,
        /// Children removed at `index` (must match the model).
        remove: Vec<ShapeNode>,
        /// Children inserted in their place.
        insert: Vec<ShapeNode>,
    },
    /// Splice the slide list.
    Slides {
        /// First slide replaced.
        index: usize,
        /// Slides removed at `index` (must match the model).
        remove: Vec<Slide>,
        /// Slides inserted in their place.
        insert: Vec<Slide>,
    },
    /// Replace slide-level fields (relationships, notes, timing).
    Fields {
        /// Slide index.
        slide: usize,
        /// Fields expected before the op.
        before: Box<SlideFields>,
        /// Fields after the op.
        after: Box<SlideFields>,
    },
    /// Splice the preserved package parts (media, notes master, …).
    Parts {
        /// First part replaced in `Presentation::parts`.
        index: usize,
        /// Parts removed at `index` (must match the model).
        remove: Vec<RawPart>,
        /// Parts inserted in their place.
        insert: Vec<RawPart>,
    },
    /// Replace presentation-level fields.
    Pres {
        /// Fields expected before the op.
        before: Box<PresFields>,
        /// Fields after the op.
        after: Box<PresFields>,
    },
}

/// Why an op could not be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    /// The slide index is out of range.
    #[error("slide {0} does not exist")]
    NoSlide(usize),
    /// The group index path does not lead to a group.
    #[error("shape container not found")]
    NoContainer,
    /// The splice range lies outside the list.
    #[error("range out of bounds")]
    Range,
    /// The model no longer holds what the op expects to remove or replace.
    #[error("the presentation changed under the operation")]
    Stale,
}

/// Children of a container (spTree when `parent` is empty).
pub fn container_mut<'a>(
    pres: &'a mut Presentation,
    slide: usize,
    parent: &[usize],
) -> Result<&'a mut Vec<ShapeNode>, OpError> {
    let s = pres.slides.get_mut(slide).ok_or(OpError::NoSlide(slide))?;
    let mut nodes = &mut s.data.shapes;
    for &i in parent {
        match nodes.get_mut(i) {
            Some(ShapeNode::Group(g)) => nodes = &mut g.children,
            _ => return Err(OpError::NoContainer),
        }
    }
    Ok(nodes)
}

/// Read-only variant of [`container_mut`].
pub fn container<'a>(
    pres: &'a Presentation,
    slide: usize,
    parent: &[usize],
) -> Option<&'a Vec<ShapeNode>> {
    let s = pres.slides.get(slide)?;
    let mut nodes = &s.data.shapes;
    for &i in parent {
        match nodes.get(i) {
            Some(ShapeNode::Group(g)) => nodes = &g.children,
            _ => return None,
        }
    }
    Some(nodes)
}

/// Node at an index path.
pub fn node_at<'a>(pres: &'a Presentation, slide: usize, path: &[usize]) -> Option<&'a ShapeNode> {
    let (last, parent) = path.split_last()?;
    container(pres, slide, parent)?.get(*last)
}

fn splice<T: Clone + PartialEq>(
    v: &mut Vec<T>,
    index: usize,
    remove: &[T],
    insert: &[T],
) -> Result<(), OpError> {
    let end = index.checked_add(remove.len()).ok_or(OpError::Range)?;
    if end > v.len() {
        return Err(OpError::Range);
    }
    if v[index..end] != *remove {
        return Err(OpError::Stale);
    }
    v.splice(index..end, insert.iter().cloned());
    Ok(())
}

impl SlideOp {
    /// Apply to the presentation.
    pub fn apply(&self, pres: &mut Presentation) -> Result<(), OpError> {
        match self {
            SlideOp::Nodes {
                slide,
                parent,
                index,
                remove,
                insert,
            } => splice(container_mut(pres, *slide, parent)?, *index, remove, insert),
            SlideOp::Slides {
                index,
                remove,
                insert,
            } => splice(&mut pres.slides, *index, remove, insert),
            SlideOp::Fields {
                slide,
                before,
                after,
            } => {
                let s = pres
                    .slides
                    .get_mut(*slide)
                    .ok_or(OpError::NoSlide(*slide))?;
                if SlideFields::of(s) != **before {
                    return Err(OpError::Stale);
                }
                (**after).clone().store(s);
                Ok(())
            }
            SlideOp::Parts {
                index,
                remove,
                insert,
            } => splice(&mut pres.parts, *index, remove, insert),
            SlideOp::Pres { before, after } => {
                if PresFields::of(pres) != **before {
                    return Err(OpError::Stale);
                }
                pres.info.rels.clone_from(&after.rels);
                pres.extra.clone_from(&after.extra);
                pres.layouts.clone_from(&after.layouts);
                pres.masters.clone_from(&after.masters);
                pres.themes.clone_from(&after.themes);
                pres.slide_size.clone_from(&after.slide_size);
                Ok(())
            }
        }
    }

    /// The exact inverse.
    #[must_use]
    pub fn inverse(&self) -> SlideOp {
        match self {
            SlideOp::Nodes {
                slide,
                parent,
                index,
                remove,
                insert,
            } => SlideOp::Nodes {
                slide: *slide,
                parent: parent.clone(),
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            SlideOp::Slides {
                index,
                remove,
                insert,
            } => SlideOp::Slides {
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            SlideOp::Fields {
                slide,
                before,
                after,
            } => SlideOp::Fields {
                slide: *slide,
                before: after.clone(),
                after: before.clone(),
            },
            SlideOp::Parts {
                index,
                remove,
                insert,
            } => SlideOp::Parts {
                index: *index,
                remove: insert.clone(),
                insert: remove.clone(),
            },
            SlideOp::Pres { before, after } => SlideOp::Pres {
                before: after.clone(),
                after: before.clone(),
            },
        }
    }

    /// Slide whose rendering changes (`None` = the slide list itself changes).
    pub fn touched_slide(&self) -> Option<usize> {
        match self {
            SlideOp::Nodes { slide, .. } | SlideOp::Fields { slide, .. } => Some(*slide),
            SlideOp::Slides { .. } | SlideOp::Parts { .. } | SlideOp::Pres { .. } => None,
        }
    }
}

/// Ops applied so far by one command; rolled back if the command fails.
pub struct Tx<'a> {
    /// The presentation being edited.
    pub pres: &'a mut Presentation,
    /// Ops applied so far, in order.
    pub ops: Vec<SlideOp>,
}

impl<'a> Tx<'a> {
    /// Starts an empty transaction on `pres`.
    pub fn new(pres: &'a mut Presentation) -> Self {
        Tx {
            pres,
            ops: Vec::new(),
        }
    }

    /// Apply and record an op.
    pub fn op(&mut self, op: SlideOp) -> Result<(), OpError> {
        op.apply(self.pres)?;
        self.ops.push(op);
        Ok(())
    }

    /// Undo everything applied so far.
    pub fn rollback(&mut self) {
        while let Some(op) = self.ops.pop() {
            // Inverses of just-applied ops always apply.
            let _ = op.inverse().apply(self.pres);
        }
    }

    /// Replace the node at `path`.
    pub fn replace_node(
        &mut self,
        slide: usize,
        path: &[usize],
        node: ShapeNode,
    ) -> Result<(), OpError> {
        let (last, parent) = path.split_last().ok_or(OpError::NoContainer)?;
        let old = container(self.pres, slide, parent)
            .and_then(|c| c.get(*last))
            .cloned()
            .ok_or(OpError::Range)?;
        if old == node {
            return Ok(());
        }
        self.op(SlideOp::Nodes {
            slide,
            parent: parent.to_vec(),
            index: *last,
            remove: vec![old],
            insert: vec![node],
        })
    }

    /// Remove the node at `path`; returns it.
    pub fn remove_node(&mut self, slide: usize, path: &[usize]) -> Result<ShapeNode, OpError> {
        let (last, parent) = path.split_last().ok_or(OpError::NoContainer)?;
        let old = container(self.pres, slide, parent)
            .and_then(|c| c.get(*last))
            .cloned()
            .ok_or(OpError::Range)?;
        self.op(SlideOp::Nodes {
            slide,
            parent: parent.to_vec(),
            index: *last,
            remove: vec![old.clone()],
            insert: vec![],
        })?;
        Ok(old)
    }

    /// Insert nodes into a container at `index`.
    pub fn insert_nodes(
        &mut self,
        slide: usize,
        parent: &[usize],
        index: usize,
        nodes: Vec<ShapeNode>,
    ) -> Result<(), OpError> {
        self.op(SlideOp::Nodes {
            slide,
            parent: parent.to_vec(),
            index,
            remove: vec![],
            insert: nodes,
        })
    }

    /// Replace slide-level fields of `slide` through `f`.
    pub fn fields(
        &mut self,
        slide: usize,
        f: impl FnOnce(&mut SlideFields),
    ) -> Result<(), OpError> {
        let s = self.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
        let before = SlideFields::of(s);
        let mut after = before.clone();
        f(&mut after);
        if after == before {
            return Ok(());
        }
        self.op(SlideOp::Fields {
            slide,
            before: Box::new(before),
            after: Box::new(after),
        })
    }
}

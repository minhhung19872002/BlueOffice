//! BlueSlide commands of the "Check document" panel (CORE-INSPECT-001):
//! `inspect`, `inspectRemove`, `inspectKeep`, `inspectFix`. Model removals and
//! fixes are one undoable transaction; package parts go on the next save.

use super::ops::SlideOp;
use super::{refuse, Outcome, SlideCommand, SlideEditResult};
use crate::inspect::{self, Category, Fix, Target};
use crate::{EngineError, Result};
use bo_model::slide::{GraphicContent, ShapeNode};

impl crate::Engine {
    /// Check document report of the open presentation.
    pub fn inspect_presentation(&self) -> Result<inspect::Report> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        Ok(inspect::slide::report(pres, &self.scrub))
    }

    fn with_report(&mut self, out: Outcome) -> Result<SlideEditResult> {
        let mut r = self.finish_slide_command(out)?;
        r.inspection = Some(Box::new(self.inspect_presentation()?));
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    pub(super) fn slide_inspect_command(
        &mut self,
        cmd: &SlideCommand,
    ) -> Result<Option<SlideEditResult>> {
        let mut out = Outcome::default();
        match cmd {
            SlideCommand::Inspect => {}
            SlideCommand::InspectKeep { categories } => {
                for c in categories {
                    self.scrub.set(*c, false);
                }
            }
            SlideCommand::InspectRemove { categories } => {
                for c in categories {
                    self.scrub.set(*c, true);
                }
                let cats = categories.clone();
                let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
                let ed = &mut self.slides;
                ed.clamp(pres);
                ed.mutate(pres, &mut out, |tx, sel| {
                    let n = tx.pres.slides.len();
                    for si in 0..n {
                        let mut idx = inspect::slide::removable_nodes(tx.pres, si, &cats);
                        idx.sort_unstable();
                        for i in idx.into_iter().rev() {
                            tx.remove_node(si, &[i])?;
                        }
                    }
                    if cats.contains(&Category::Notes) {
                        for si in inspect::slide::slides_with_notes(tx.pres) {
                            tx.fields(si, |f| f.notes = None)?;
                        }
                    }
                    if cats.contains(&Category::HiddenSlides) {
                        let hidden = inspect::slide::hidden_slides(tx.pres);
                        // A deck keeps at least one slide.
                        if hidden.len() < tx.pres.slides.len() {
                            for i in hidden.into_iter().rev() {
                                let old = tx.pres.slides[i].clone();
                                tx.op(SlideOp::Slides {
                                    index: i,
                                    remove: vec![old],
                                    insert: vec![],
                                })?;
                            }
                        }
                    }
                    let n = tx.pres.slides.len();
                    sel.slide = sel.slide.min(n.saturating_sub(1));
                    sel.ids.clear();
                    Ok(())
                });
                ed.clamp(pres);
            }
            SlideCommand::InspectFix { fix } => {
                let fix = fix.clone();
                let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
                let ed = &mut self.slides;
                ed.clamp(pres);
                ed.mutate(pres, &mut out, |tx, _| {
                    let (slide, id) = match &fix {
                        Fix::AltText {
                            target: Target::Shape { slide, id },
                            ..
                        }
                        | Fix::HeaderRow {
                            target: Target::Shape { slide, id },
                        } => (*slide, *id),
                        _ => return refuse("inspectTarget"),
                    };
                    let Some(i) = tx.pres.slides.get(slide).and_then(|s| {
                        s.data
                            .shapes
                            .iter()
                            .position(|n| n.nv().is_some_and(|nv| nv.id == id))
                    }) else {
                        return refuse("inspectTarget");
                    };
                    let mut node = tx.pres.slides[slide].data.shapes[i].clone();
                    match (&fix, &mut node) {
                        (Fix::AltText { text, .. }, n) => {
                            let nv = match n {
                                ShapeNode::Shape(s) => &mut s.nv,
                                ShapeNode::Picture(p) => &mut p.nv,
                                ShapeNode::Group(g) => &mut g.nv,
                                ShapeNode::Connector(c) => &mut c.nv,
                                ShapeNode::Frame(f) => &mut f.nv,
                                ShapeNode::Other(_) => return refuse("inspectTarget"),
                            };
                            // §20.1.2.2.8 cNvPr `descr`.
                            let t = text.trim();
                            nv.descr = (!t.is_empty()).then(|| t.to_owned());
                        }
                        (Fix::HeaderRow { .. }, ShapeNode::Frame(f)) => match &mut f.content {
                            GraphicContent::Table(t) => {
                                t.props = Some(inspect::slide::with_first_row(t.props.as_ref()));
                            }
                            GraphicContent::Other(_) => return refuse("inspectTarget"),
                        },
                        _ => return refuse("inspectTarget"),
                    }
                    tx.replace_node(slide, &[i], node)?;
                    Ok(())
                });
                ed.clamp(pres);
            }
            _ => return Ok(None),
        }
        Ok(Some(self.with_report(out)?))
    }
}

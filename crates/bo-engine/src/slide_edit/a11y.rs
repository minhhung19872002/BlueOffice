//! Accessible structure of slides for the screen-reader mirror of the canvas (WCAG 2.2
//! 1.3.1 Info and Relationships, 4.1.2 Name, Role, Value): `slideOutline` lists the shapes of
//! slides `from..=to` in reading order — title placeholders first (ECMA-376 Part 1,
//! §19.3.1.36 ph `@type` `title` / `ctrTitle`), then top to bottom and left to right, the shapes
//! of groups (§19.3.1.22 grpSp) in place — with their text (§19.3.1.51 txBody) and alternative
//! text (§19.3.1.12 cNvPr `@descr`) for pictures, charts, SmartArt and other graphics. Hidden
//! shapes (`@hidden`) are left out. The query never changes the deck or the selection.

use super::geom::{place_slide, Placed};
use super::{kind_of, ops, text};
use crate::{Engine, EngineError, Result};
use bo_model::slide::{Presentation, ShapeNode};
use serde::{Deserialize, Serialize};

/// An accessibility query of BlueSlide.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum A11yCommand {
    /// Shapes of slides `from..=to` (0-based; clamped to the deck) in reading order.
    SlideOutline {
        /// First slide.
        from: usize,
        /// Last slide.
        to: usize,
    },
}

impl A11yCommand {
    /// Parses a JSON command when it is an accessibility query.
    pub fn parse(v: &serde_json::Value) -> Option<A11yCommand> {
        if v.get("cmd")?.as_str()? != "slideOutline" {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// A shape of the accessible structure.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A11yShape {
    /// Shape id (`cNvPr@id`).
    pub id: u32,
    /// Shape name (`cNvPr@name`).
    pub name: String,
    /// `shape`, `picture`, `table`, `chart`, `smartart`, … (as in the selection).
    pub kind: &'static str,
    /// Placeholder type (`title`, `body`, … ; `obj` when untyped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Text (`\n` paragraphs, `\u{b}` line breaks), when not empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Alternative text (`cNvPr@descr`), when not empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// One slide of the accessible structure.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct A11ySlide {
    /// Slide index (0-based).
    pub index: usize,
    /// Shapes in reading order.
    pub shapes: Vec<A11yShape>,
}

#[derive(Serialize)]
struct Answer {
    slides: Vec<A11ySlide>,
}

fn is_title(n: &ShapeNode) -> bool {
    n.nv()
        .and_then(|nv| nv.placeholder.as_ref())
        .and_then(|ph| ph.kind.as_deref())
        .is_some_and(|k| k == "title" || k == "ctrTitle")
}

/// The shapes of `placed` (one level of the shape tree) in reading order, into `out`.
fn collect(pres: &Presentation, slide: usize, placed: &[Placed], out: &mut Vec<A11yShape>) {
    let mut order: Vec<(&Placed, &ShapeNode)> = placed
        .iter()
        .filter_map(|p| Some((p, ops::node_at(pres, slide, &p.path)?)))
        .collect();
    let top = |p: &Placed| p.b.cy - p.b.h / 2.0;
    let left = |p: &Placed| p.b.cx - p.b.w / 2.0;
    order.sort_by(|(a, na), (b, nb)| {
        is_title(nb)
            .cmp(&is_title(na))
            .then(top(a).total_cmp(&top(b)))
            .then(left(a).total_cmp(&left(b)))
    });
    for (p, n) in order {
        let nv = n.nv().cloned().unwrap_or_default();
        if nv.hidden == Some(true) {
            continue;
        }
        if matches!(n, ShapeNode::Group(_)) {
            collect(pres, slide, &p.children, out);
            continue;
        }
        let text = match n {
            ShapeNode::Shape(s) => s
                .text
                .as_ref()
                .map(text::body_text)
                .filter(|t| !t.trim().is_empty()),
            _ => None,
        };
        out.push(A11yShape {
            id: nv.id,
            name: nv.name,
            kind: kind_of(n),
            placeholder: nv
                .placeholder
                .map(|ph| ph.kind.unwrap_or_else(|| "obj".into())),
            text,
            alt: nv.descr.filter(|d| !d.trim().is_empty()),
        });
    }
}

/// Shapes of slide `slide` in reading order.
pub fn slide_shapes(pres: &Presentation, slide: usize) -> Vec<A11yShape> {
    let mut out = Vec::new();
    collect(pres, slide, &place_slide(pres, slide), &mut out);
    out
}

impl Engine {
    /// Answer an accessibility query of the open presentation (JSON).
    pub(crate) fn slide_a11y_command(&self, cmd: &A11yCommand) -> Result<String> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let A11yCommand::SlideOutline { from, to } = *cmd;
        let n = pres.slides.len();
        let slides = (from.min(n)..to.saturating_add(1).min(n))
            .map(|index| A11ySlide {
                index,
                shapes: slide_shapes(pres, index),
            })
            .collect();
        Ok(serde_json::to_string(&Answer { slides }).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::Value;

    fn corpus(rel: &str) -> Vec<u8> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide")
            .join(rel);
        std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    #[test]
    fn outline_lists_title_first_then_reading_order() {
        let mut e = Engine::with_bundled_fonts();
        e.open(&corpus("slide-text-bullets.pptx"), "d.pptx")
            .unwrap();
        let before = e.slide_command_json(r#"{"cmd":"state"}"#).unwrap();
        let v: Value = serde_json::from_str(
            &e.slide_command_json(r#"{"cmd":"slideOutline","from":0,"to":99}"#)
                .unwrap(),
        )
        .unwrap();
        let slides = v["slides"].as_array().unwrap();
        assert_ne!(slides.len(), 0);
        assert_eq!(slides[0]["index"], 0);
        let shapes = slides[0]["shapes"].as_array().unwrap();
        assert_ne!(shapes.len(), 0);
        // A title placeholder, when the slide has one, comes first.
        if let Some(i) = shapes
            .iter()
            .position(|s| s["placeholder"] == "title" || s["placeholder"] == "ctrTitle")
        {
            assert_eq!(i, 0);
            assert!(shapes[0]["text"].as_str().is_some_and(|t| !t.is_empty()));
        }
        // The query changes nothing.
        assert_eq!(e.slide_command_json(r#"{"cmd":"state"}"#).unwrap(), before);
        // Out-of-range slides: nothing.
        let v: Value = serde_json::from_str(
            &e.slide_command_json(r#"{"cmd":"slideOutline","from":500,"to":600}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["slides"], serde_json::json!([]));
    }

    #[test]
    fn pictures_carry_their_alternative_text() {
        use bo_model::slide::ShapeNode;
        let mut e = Engine::with_bundled_fonts();
        e.open(&corpus("slide-text-bullets.pptx"), "d.pptx")
            .unwrap();
        // Give the first shape a description and hide the second: what the outline shows.
        let pres = e.pres.as_mut().unwrap();
        let mut ids = Vec::new();
        for n in &mut pres.slides[0].data.shapes {
            if let ShapeNode::Shape(s) = n {
                match ids.len() {
                    0 => s.nv.descr = Some("Mô tả thay thế".into()),
                    1 => s.nv.hidden = Some(true),
                    _ => {}
                }
                ids.push(s.nv.id);
            }
        }
        assert!(ids.len() >= 2, "the sample has two text shapes");
        let shapes = super::slide_shapes(e.pres.as_ref().unwrap(), 0);
        let first = shapes.iter().find(|s| s.id == ids[0]).unwrap();
        assert_eq!(first.alt.as_deref(), Some("Mô tả thay thế"));
        assert!(
            shapes.iter().all(|s| s.id != ids[1]),
            "hidden shapes are left out"
        );
    }
}

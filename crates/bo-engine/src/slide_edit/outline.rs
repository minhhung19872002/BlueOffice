//! Outline view (SLIDE-VIEW-001): the deck as a list of slide titles and the
//! paragraphs of each slide's body placeholder, and the one command that
//! writes an edited outline back.
//!
//! * Title = the slide's `title` / `ctrTitle` placeholder; body = its first
//!   `body`, `subTitle` or content (`obj`, no `type`) placeholder
//!   (ECMA-376 Part 1, §19.3.1.36 ph, §19.7.10 ST_PlaceholderType).
//! * Bullet level = paragraph level (§21.1.2.2.7 `pPr@lvl`, 0–8).
//! * [`cmd_apply`] receives the whole outline: slides it no longer names are
//!   deleted (a title demoted into the previous slide), new entries become new
//!   slides, and changed titles / bodies are rewritten keeping the formatting of
//!   unchanged runs — all in one undoable step.

use super::build;
use super::ops::{SlideOp, Tx};
use super::text::{body_text, set_text};
use super::{layout_for_new, next_shape_id, refuse, EditError, SlideSel, R};
use bo_model::slide::{ParaProps, Presentation, Shape, ShapeNode, Slide, TextBody};
use serde::{Deserialize, Serialize};

/// Most bullet levels a body can have (§21.1.2.2.7: `lvl` 0–8).
pub const MAX_LEVEL: u8 = 8;

/// One body paragraph of the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineBullet {
    /// Paragraph text (line breaks inside it are `\u{b}`).
    pub text: String,
    /// Level 0–8.
    #[serde(default)]
    pub level: u8,
}

/// A slide in the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineSlide {
    /// Title text (paragraphs separated by `\n`).
    pub title: String,
    /// Paragraphs of the body placeholder.
    pub bullets: Vec<OutlineBullet>,
    /// The slide has a title placeholder.
    pub has_title: bool,
    /// The slide has a body placeholder.
    pub has_body: bool,
}

/// One slide of an edited outline.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineEdit {
    /// The existing slide this entry stands for (`None`: a new slide).
    #[serde(default)]
    pub slide: Option<usize>,
    /// Title text.
    #[serde(default)]
    pub title: String,
    /// Body paragraphs.
    #[serde(default)]
    pub bullets: Vec<OutlineBullet>,
}

fn placeholder(s: &Shape) -> Option<&bo_model::slide::Placeholder> {
    s.nv.placeholder.as_ref()
}

fn is_title(kind: Option<&str>) -> bool {
    matches!(kind, Some("title" | "ctrTitle"))
}

fn is_body(kind: Option<&str>) -> bool {
    matches!(kind, None | Some("body" | "subTitle"))
}

/// Indices (top level) of the title and body placeholders of a slide.
fn parts(s: &Slide) -> (Option<usize>, Option<usize>) {
    let mut title = None;
    let mut body = None;
    for (i, n) in s.data.shapes.iter().enumerate() {
        let ShapeNode::Shape(sh) = n else { continue };
        let Some(ph) = placeholder(sh) else { continue };
        let kind = ph.kind.as_deref();
        if title.is_none() && is_title(kind) {
            title = Some(i);
        } else if body.is_none() && is_body(kind) {
            body = Some(i);
        }
    }
    (title, body)
}

fn shape_at(s: &Slide, i: usize) -> Option<&Shape> {
    match s.data.shapes.get(i)? {
        ShapeNode::Shape(sh) => Some(sh),
        _ => None,
    }
}

fn bullets_of(tb: &TextBody) -> Vec<OutlineBullet> {
    tb.paragraphs
        .iter()
        .map(|p| OutlineBullet {
            text: super::text::para_text(p),
            level: p
                .props
                .as_ref()
                .and_then(|pp| pp.lvl)
                .unwrap_or(0)
                .min(MAX_LEVEL),
        })
        .collect()
}

/// The outline of a slide.
pub fn slide_outline(s: &Slide) -> OutlineSlide {
    let (t, b) = parts(s);
    let title = t
        .and_then(|i| shape_at(s, i))
        .and_then(|sh| sh.text.as_ref())
        .map(body_text)
        .unwrap_or_default();
    let bullets = b
        .and_then(|i| shape_at(s, i))
        .map(|sh| {
            sh.text.as_ref().map_or_else(
                || {
                    vec![OutlineBullet {
                        text: String::new(),
                        level: 0,
                    }]
                },
                bullets_of,
            )
        })
        .unwrap_or_default();
    OutlineSlide {
        title,
        bullets,
        has_title: t.is_some(),
        has_body: b.is_some(),
    }
}

/// The outline of the deck.
pub fn outline(pres: &Presentation) -> Vec<OutlineSlide> {
    pres.slides.iter().map(slide_outline).collect()
}

/// Placeholder of the slide's layout matching `want` (`kind`, `idx`, name).
fn layout_placeholder(
    pres: &Presentation,
    s: &Slide,
    want: fn(Option<&str>) -> bool,
) -> Option<(Option<String>, Option<u32>, String)> {
    let l = pres.layouts.get(s.layout)?;
    l.data.shapes.iter().find_map(|n| {
        let ShapeNode::Shape(sh) = n else { return None };
        let ph = sh.nv.placeholder.as_ref()?;
        want(ph.kind.as_deref()).then(|| (ph.kind.clone(), ph.idx, sh.nv.name.clone()))
    })
}

/// Text of the body as levels: set the text, then each paragraph's level.
fn set_bullets(tb: &mut TextBody, bullets: &[OutlineBullet]) {
    let text = bullets
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    set_text(tb, &text);
    for (p, b) in tb.paragraphs.iter_mut().zip(bullets) {
        let level = b.level.min(MAX_LEVEL);
        let cur = p.props.as_ref().and_then(|pp| pp.lvl).unwrap_or(0);
        if cur != level {
            // §21.1.2.2.7 pPr@lvl.
            p.props.get_or_insert_with(ParaProps::default).lvl = Some(level);
        }
    }
}

/// Write title and bullets into slide `i`.
fn write_slide(tx: &mut Tx<'_>, i: usize, edit: &OutlineEdit) -> R<()> {
    let s = tx
        .pres
        .slides
        .get(i)
        .cloned()
        .ok_or(EditError::Refused("noSlide"))?;
    let now = slide_outline(&s);
    let (t, b) = parts(&s);
    if now.title != edit.title {
        if let Some(t) = t {
            if let Some(sh) = shape_at(&s, t) {
                let mut sh = sh.clone();
                set_text(sh.text.get_or_insert_with(TextBody::default), &edit.title);
                tx.replace_node(i, &[t], ShapeNode::Shape(Box::new(sh)))?;
            }
        } else if !edit.title.is_empty() {
            // No title placeholder: add the layout's (§19.3.1.36: position and
            // formatting are inherited from it).
            let (kind, idx, name) = layout_placeholder(tx.pres, &s, is_title).unwrap_or((
                Some("title".into()),
                None,
                "Title".into(),
            ));
            let id = next_shape_id(tx.pres, i);
            let Some(ShapeNode::Shape(mut sh)) =
                build::placeholder(id, kind.as_deref(), idx, &name)
            else {
                return refuse("build");
            };
            set_text(sh.text.get_or_insert_with(TextBody::default), &edit.title);
            tx.insert_nodes(i, &[], 0, vec![ShapeNode::Shape(sh)])?;
        }
    }
    let wanted = if edit.bullets.is_empty() {
        vec![OutlineBullet {
            text: String::new(),
            level: 0,
        }]
    } else {
        edit.bullets.clone()
    };
    let s = tx
        .pres
        .slides
        .get(i)
        .cloned()
        .ok_or(EditError::Refused("noSlide"))?;
    let (_, b2) = parts(&s);
    let b = b2.or(b);
    if let Some(b) = b {
        if let Some(sh) = shape_at(&s, b) {
            let cur = sh.text.as_ref().map(bullets_of).unwrap_or_default();
            if cur != wanted {
                let mut sh = sh.clone();
                set_bullets(sh.text.get_or_insert_with(TextBody::default), &wanted);
                tx.replace_node(i, &[b], ShapeNode::Shape(Box::new(sh)))?;
            }
        }
    } else if edit.bullets.iter().any(|x| !x.text.is_empty()) {
        let (kind, idx, name) = layout_placeholder(tx.pres, &s, is_body).unwrap_or((
            Some("body".into()),
            Some(1),
            "Text Placeholder".into(),
        ));
        let id = next_shape_id(tx.pres, i);
        let Some(ShapeNode::Shape(mut sh)) = build::placeholder(id, kind.as_deref(), idx, &name)
        else {
            return refuse("build");
        };
        set_bullets(sh.text.get_or_insert_with(TextBody::default), &wanted);
        let at = s.data.shapes.len();
        tx.insert_nodes(i, &[], at, vec![ShapeNode::Shape(sh)])?;
    }
    Ok(())
}

/// Replace the deck's outline by `slides` (see the module docs); `current`
/// is the entry to select afterwards.
pub(crate) fn cmd_apply(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    slides: &[OutlineEdit],
    current: Option<usize>,
) -> R<()> {
    let n = tx.pres.slides.len();
    // Existing slides keep their order: the outline cannot reorder them.
    let mut last = None;
    for e in slides {
        if let Some(i) = e.slide {
            if i >= n || last.is_some_and(|l| i <= l) {
                return refuse("outlineOrder");
            }
            last = Some(i);
        }
    }
    if slides.is_empty() {
        return refuse("outlineEmpty");
    }
    // Delete the slides the outline no longer names (from the end).
    for i in (0..n).rev() {
        if !slides.iter().any(|e| e.slide == Some(i)) {
            let s = tx.pres.slides[i].clone();
            tx.op(SlideOp::Slides {
                index: i,
                remove: vec![s],
                insert: vec![],
            })?;
        }
    }
    for (p, e) in slides.iter().enumerate() {
        if e.slide.is_none() {
            // New slide after the previous one, on its content layout.
            let layout = layout_for_new(tx.pres, p.saturating_sub(1))
                .ok_or(EditError::Refused("noLayout"))?;
            let slide = build::slide_from_layout(tx.pres, layout);
            tx.op(SlideOp::Slides {
                index: p,
                remove: vec![],
                insert: vec![slide],
            })?;
        }
        write_slide(tx, p, e)?;
    }
    let count = tx.pres.slides.len();
    *sel = SlideSel {
        slide: current.unwrap_or(sel.slide).min(count.saturating_sub(1)),
        ids: vec![],
    };
    Ok(())
}

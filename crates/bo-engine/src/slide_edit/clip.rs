//! Internal clipboard of the slide editor: shapes keep their full XML (as the
//! model, with `Extra`) and the package parts they reference (pictures,
//! media), slides are kept whole. Pasting allocates new shape ids,
//! relationship ids and — across decks — new part names (ECMA-376 Part 2,
//! §9.3 relationships; Part 1, §19.3.1.43 sp, §19.3.1.37 pic).

use super::geom::{BoxR, Rect, EMU_PT};
use bo_model::slide::{Presentation, RawPart, Rel, ShapeNode, Slide, XmlElement, XmlNode};

/// A relationship referenced by copied shapes, with the part it targets.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipRel {
    pub rel: Rel,
    /// Target part (internal relationships), copied for pasting into other decks.
    pub part: Option<RawPart>,
}

/// What the clipboard holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipKind {
    Shapes {
        /// Slide the shapes were copied from.
        slide: usize,
        nodes: Vec<ShapeNode>,
        rels: Vec<ClipRel>,
        /// Box around the copied shapes (slide points).
        bounds: Rect,
    },
    Slides(Vec<Slide>),
}

/// Clipboard content and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Deck the content was copied from (`Engine` open counter).
    pub deck: u64,
    pub kind: ClipKind,
    /// Pastes onto the source slide so far (each one is offset a bit more).
    pub pastes: usize,
}

/// Visit every attribute of an element tree.
fn walk_attrs(el: &mut XmlElement, f: &mut impl FnMut(&str, &mut String)) {
    for (k, v) in &mut el.attrs {
        f(k, v);
    }
    for c in &mut el.children {
        if let XmlNode::Element(e) = c {
            walk_attrs(e, f);
        }
    }
}

/// Relationship ids a node refers to (`r:embed`, `r:id`, `r:link`, …).
pub fn rel_ids(n: &ShapeNode) -> Vec<String> {
    let mut el = bo_pptx::write_node(n);
    let mut out = Vec::new();
    walk_attrs(&mut el, &mut |k, v| {
        if k.starts_with("r:") && !out.contains(v) {
            out.push(v.clone());
        }
    });
    out
}

/// Rewrite relationship ids of a node (`map`: old → new).
pub fn remap_rels(n: &ShapeNode, map: &[(String, String)]) -> ShapeNode {
    if map.iter().all(|(a, b)| a == b) {
        return n.clone();
    }
    let mut el = bo_pptx::write_node(n);
    walk_attrs(&mut el, &mut |k, v| {
        if k.starts_with("r:") {
            if let Some((_, new)) = map.iter().find(|(old, _)| old == v) {
                v.clone_from(new);
            }
        }
    });
    bo_pptx::parse_node(&el)
}

/// Part name a slide relationship points to.
pub fn rel_part(slide_part: &str, rel: &Rel) -> Option<String> {
    if rel.external {
        return None;
    }
    let base = if slide_part.is_empty() {
        "/ppt/slides/slide1.xml"
    } else {
        slide_part
    };
    Some(bo_opc::resolve_target(base, &rel.target))
}

/// First relationship id `rIdN` not used by a slide.
pub fn free_rel_id(rels: &[Rel]) -> String {
    (1..=rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| rels.iter().all(|r| &r.id != id))
        .unwrap_or_default()
}

/// Move a node by (dx, dy) points (its own transform, top-level space).
pub fn shift(n: &mut ShapeNode, dx: f64, dy: f64) {
    let (ex, ey) = ((dx * EMU_PT).round() as i64, (dy * EMU_PT).round() as i64);
    let x = match n {
        ShapeNode::Shape(s) => s.props.xfrm.as_mut(),
        ShapeNode::Picture(p) => p.props.xfrm.as_mut(),
        ShapeNode::Connector(c) => c.props.xfrm.as_mut(),
        ShapeNode::Group(g) => g.props.xfrm.as_mut(),
        ShapeNode::Frame(f) => Some(&mut f.xfrm),
        ShapeNode::Other(_) => None,
    };
    if let Some(x) = x {
        if let Some((ox, oy)) = x.off {
            x.off = Some((ox + ex, oy + ey));
        }
    }
}

/// Plain text of copied shapes (paragraphs on their own lines).
pub fn plain_text(nodes: &[ShapeNode]) -> String {
    let mut out: Vec<String> = Vec::new();
    bo_pptx::visit(nodes, &mut |n| match n {
        ShapeNode::Shape(s) => {
            if let Some(t) = &s.text {
                let t = t.text();
                if !t.trim().is_empty() {
                    out.push(t);
                }
            }
        }
        ShapeNode::Frame(f) => {
            if let bo_model::slide::GraphicContent::Table(t) = &f.content {
                for r in &t.rows {
                    let cells: Vec<String> = r
                        .cells
                        .iter()
                        .map(|c| {
                            c.text
                                .as_ref()
                                .map(bo_model::slide::TextBody::text)
                                .unwrap_or_default()
                        })
                        .collect();
                    out.push(cells.join("\t"));
                }
            }
        }
        _ => {}
    });
    out.join("\n")
}

/// Relationships (and parts) of a slide that copied nodes refer to.
pub fn collect_rels(pres: &Presentation, slide: usize, nodes: &[ShapeNode]) -> Vec<ClipRel> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = Vec::new();
    for n in nodes {
        for id in rel_ids(n) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    s.info
        .rels
        .iter()
        .filter(|r| ids.contains(&r.id))
        .map(|r| ClipRel {
            rel: r.clone(),
            part: rel_part(&s.info.part_name, r).and_then(|name| {
                pres.parts
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&name))
                    .cloned()
            }),
        })
        .collect()
}

/// Union of boxes.
pub fn bounds_of(boxes: &[BoxR]) -> Rect {
    Rect::union_all(boxes.iter().map(BoxR::bounds)).unwrap_or_default()
}

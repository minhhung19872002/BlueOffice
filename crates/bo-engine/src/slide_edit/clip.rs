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
    /// The relationship as it was in the source slide part.
    pub rel: Rel,
    /// Target part (internal relationships), copied for pasting into other decks.
    pub part: Option<RawPart>,
}

/// What the clipboard holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipKind {
    /// Shapes copied from one slide.
    Shapes {
        /// Slide the shapes were copied from.
        slide: usize,
        /// The copied shapes, top-level nodes of the shape tree in z-order.
        nodes: Vec<ShapeNode>,
        /// Relationships the copied shapes reference (pictures, media, charts).
        rels: Vec<ClipRel>,
        /// Box around the copied shapes (slide points).
        bounds: Rect,
    },
    /// Whole slides, in deck order.
    Slides(Vec<Slide>),
}

/// Clipboard content and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Deck the content was copied from (`Engine` open counter).
    pub deck: u64,
    /// The copied content.
    pub kind: ClipKind,
    /// Pastes onto the source slide so far (each one is offset a bit more).
    pub pastes: usize,
    /// Design of the source deck for copied slides (pasting into another deck).
    pub source: Option<Box<SlideSource>>,
}

/// What copied slides need from their deck to be pasted into another one: its
/// layouts, masters and themes (ECMA-376 Part 1, §19.3.1.39 sldLayout, §19.3.1.42
/// sldMaster, §20.1.6.9 theme) and the parts their relationships target
/// (pictures, media, …).
#[derive(Debug, Clone, PartialEq)]
pub struct SlideSource {
    /// Slide layouts of the source deck.
    pub layouts: Vec<bo_model::slide::SlideLayout>,
    /// Slide masters of the source deck.
    pub masters: Vec<bo_model::slide::SlideMaster>,
    /// Themes of the source deck.
    pub themes: Vec<bo_model::slide::Theme>,
    /// Parts targeted by the copied slides (and their designs), with the parts
    /// those reference in turn: a chart's embedded workbook, colour and style
    /// parts (§21.2 `c:externalData`, [MS-ODRAWXML] chartStyle / chartColorStyle)
    /// and the `.rels` parts linking them (ECMA-376 Part 2, §9.3).
    pub parts: Vec<RawPart>,
    /// §19.2.1.39 sldSz of the source deck.
    pub slide_size: bo_model::slide::SlideSize,
}

/// `(Id, Type, Target, external)` of the relationships of a `.rels` part.
pub fn rels_of(data: &[u8]) -> Vec<(String, String, String, bool)> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Vec::new();
    };
    let attr = |el: &str, name: &str| -> Option<String> {
        let at = el.find(&format!(" {name}=\""))? + name.len() + 3;
        let end = el[at..].find('"')? + at;
        Some(bo_opc::xml::unescape(&el[at..end]))
    };
    text.split("<Relationship ")
        .skip(1)
        .filter_map(|el| {
            let el = format!(" {}", el.split('>').next().unwrap_or(""));
            Some((
                attr(&el, "Id")?,
                attr(&el, "Type")?,
                attr(&el, "Target")?,
                attr(&el, "TargetMode").is_some_and(|m| m == "External"),
            ))
        })
        .collect()
}

/// A `.rels` part with the given relationships.
pub fn rels_xml(rels: &[(String, String, String, bool)]) -> Vec<u8> {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (id, t, target, ext) in rels {
        out.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
            bo_opc::xml::escape(id),
            bo_opc::xml::escape(t),
            bo_opc::xml::escape(target),
            if *ext { " TargetMode=\"External\"" } else { "" }
        ));
    }
    out.push_str("</Relationships>");
    out.into_bytes()
}

/// Structural relationship types (re-targeted by the writer from the model).
pub fn is_structural(rel_type: &str) -> bool {
    let t = rel_type.rsplit('/').next().unwrap_or(rel_type);
    matches!(
        t,
        "slideLayout" | "slideMaster" | "theme" | "notesSlide" | "notesMaster" | "slide"
    )
}

/// The source design and the parts targeted by the slides (and their layouts,
/// masters and themes) of deck `pres`.
pub fn slide_source(pres: &Presentation, slides: &[usize]) -> SlideSource {
    let mut parts: Vec<RawPart> = Vec::new();
    let mut add = |owner: &str, rels: &[Rel]| {
        for r in rels.iter().filter(|r| !is_structural(&r.rel_type)) {
            if let Some(name) = rel_part(owner, r) {
                if parts.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                if let Some(p) = pres
                    .parts
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&name))
                {
                    parts.push(p.clone());
                }
            }
        }
    };
    for &i in slides {
        if let Some(s) = pres.slides.get(i) {
            add(&s.info.part_name, &s.info.rels);
        }
    }
    for l in &pres.layouts {
        add(&l.info.part_name, &l.info.rels);
    }
    for m in &pres.masters {
        add(&m.info.part_name, &m.info.rels);
    }
    for t in &pres.themes {
        add(&t.info.part_name, &t.info.rels);
    }
    // Parts referenced by the copied parts (chart → workbook, colours, style).
    let find = |name: &str| {
        pres.parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .cloned()
    };
    let mut i = 0;
    while i < parts.len() && parts.len() < 10_000 {
        let name = parts[i].name.clone();
        i += 1;
        if name.ends_with(".rels") {
            continue;
        }
        let Some(rp) = find(&bo_opc::rels_part_name(&name)) else {
            continue;
        };
        for (_, _, target, ext) in rels_of(&rp.data) {
            if ext {
                continue;
            }
            let t = bo_opc::resolve_target(&name, &target);
            if !parts.iter().any(|p| p.name.eq_ignore_ascii_case(&t)) {
                if let Some(p) = find(&t) {
                    parts.push(p);
                }
            }
        }
        if !parts.iter().any(|p| p.name.eq_ignore_ascii_case(&rp.name)) {
            parts.push(rp);
        }
    }
    SlideSource {
        layouts: pres.layouts.clone(),
        masters: pres.masters.clone(),
        themes: pres.themes.clone(),
        parts,
        slide_size: pres.slide_size.clone(),
    }
}

/// Relative target of part `to` from part `from` (ECMA-376 Part 2, §9.3.4.4).
pub fn relative_target(from: &str, to: &str) -> String {
    let dir: Vec<&str> = from
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, d)| d.to_vec())
        .unwrap_or_default();
    let target: Vec<&str> = to.trim_start_matches('/').split('/').collect();
    let common = dir
        .iter()
        .zip(&target)
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    let mut out: Vec<&str> = vec![".."; dir.len() - common];
    out.extend(&target[common..]);
    out.join("/")
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

/// Remove the elements of a node that refer to one of the relationships `ids`
/// (`a:hlinkClick` / `a:hlinkHover` of a removed slide link, §21.1.2.3.5).
pub fn strip_rel_refs(n: &ShapeNode, ids: &[String]) -> ShapeNode {
    fn strip(el: &mut XmlElement, ids: &[String]) {
        el.children.retain(|c| match c {
            XmlNode::Element(e) => !e
                .attrs
                .iter()
                .any(|(k, v)| k.starts_with("r:") && ids.contains(v)),
            XmlNode::Text(_) => true,
        });
        for c in &mut el.children {
            if let XmlNode::Element(e) = c {
                strip(e, ids);
            }
        }
    }
    if !rel_ids(n).iter().any(|id| ids.contains(id)) {
        return n.clone();
    }
    let mut el = bo_pptx::write_node(n);
    strip(&mut el, ids);
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

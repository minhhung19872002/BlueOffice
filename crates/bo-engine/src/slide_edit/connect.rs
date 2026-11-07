//! Adjust handles of preset shapes (ECMA-376 Part 1, §20.1.9.5 avLst,
//! §20.1.9.1 ahLst) and connectors attached to connection sites
//! (§19.3.1.19 cxnSp, §20.1.2.2.10 stCxn / endCxn, §20.1.9.9 cxn).

use super::geom::{find, BoxR, Placed, EMU_PT};
use bo_model::slide::{Connector, Geometry, Presentation, ShapeNode, Xfrm, XmlElement, XmlNode};
use serde::Serialize;

/// Flips of a node's own transform.
pub fn flips(pres: &Presentation, slide: usize, n: &ShapeNode) -> (bool, bool) {
    let x = super::geom::node_xfrm(pres, slide, n).unwrap_or_default();
    (x.flip_h.unwrap_or(false), x.flip_v.unwrap_or(false))
}

/// Local point (origin top-left of the unrotated box) → slide point, with
/// the shape's flips (§20.1.7.6 xfrm: flip, then rotate about the centre).
pub fn to_slide(b: &BoxR, f: (bool, bool), x: f64, y: f64) -> (f64, f64) {
    let lx = if f.0 { b.w - x } else { x };
    let ly = if f.1 { b.h - y } else { y };
    b.to_slide(lx - b.w / 2.0, ly - b.h / 2.0)
}

/// Slide point → local point (inverse of [`to_slide`]).
pub fn to_local(b: &BoxR, f: (bool, bool), x: f64, y: f64) -> (f64, f64) {
    let (lx, ly) = b.to_local(x, y);
    let (lx, ly) = (lx + b.w / 2.0, ly + b.h / 2.0);
    (
        if f.0 { b.w - lx } else { lx },
        if f.1 { b.h - ly } else { ly },
    )
}

/// Geometry of a shape or connector.
pub fn geometry(n: &ShapeNode) -> Option<&Geometry> {
    match n {
        ShapeNode::Shape(s) => s.props.geometry.as_ref(),
        ShapeNode::Connector(c) => c.props.geometry.as_ref(),
        ShapeNode::Picture(p) => p.props.geometry.as_ref(),
        _ => None,
    }
}

fn geometry_mut(n: &mut ShapeNode) -> Option<&mut Option<Geometry>> {
    match n {
        ShapeNode::Shape(s) => Some(&mut s.props.geometry),
        ShapeNode::Connector(c) => Some(&mut c.props.geometry),
        _ => None,
    }
}

/// An adjust handle in slide points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AdjustOut {
    pub index: usize,
    pub x: f64,
    pub y: f64,
}

/// Adjust handles of a placed preset shape.
pub fn adjust_handles(
    pres: &Presentation,
    slide: usize,
    p: &Placed,
    n: &ShapeNode,
) -> Vec<AdjustOut> {
    let Some(Geometry::Preset { name, av }) = geometry(n) else {
        return Vec::new();
    };
    if matches!(n, ShapeNode::Picture(_)) {
        return Vec::new();
    }
    let f = flips(pres, slide, n);
    bo_drawingml::handles::handle_points(name, av, p.b.w, p.b.h)
        .into_iter()
        .map(|h| {
            let (x, y) = to_slide(&p.b, f, h.x, h.y);
            AdjustOut {
                index: h.index,
                x,
                y,
            }
        })
        .collect()
}

/// Drag adjust handle `index` to slide point (x, y): the node with new avLst values.
pub fn adjust(
    pres: &Presentation,
    slide: usize,
    p: &Placed,
    n: &ShapeNode,
    index: usize,
    x: f64,
    y: f64,
) -> Option<ShapeNode> {
    let Some(Geometry::Preset { name, av }) = geometry(n) else {
        return None;
    };
    let f = flips(pres, slide, n);
    let (lx, ly) = to_local(&p.b, f, x, y);
    let new_av = bo_drawingml::handles::drag_handle(name, av, p.b.w, p.b.h, index, lx, ly)?;
    let mut out = n.clone();
    let name = name.clone();
    if let Some(g) = geometry_mut(&mut out) {
        *g = Some(Geometry::Preset { name, av: new_av });
    }
    Some(out)
}

/// Connection sites of a placed shape in slide points.
pub fn sites(pres: &Presentation, slide: usize, p: &Placed, n: &ShapeNode) -> Vec<(f64, f64)> {
    if matches!(
        n,
        ShapeNode::Connector(_) | ShapeNode::Group(_) | ShapeNode::Other(_)
    ) {
        return Vec::new();
    }
    let f = flips(pres, slide, n);
    bo_drawingml::handles::connection_sites(geometry(n), p.b.w, p.b.h)
        .into_iter()
        .map(|s| to_slide(&p.b, f, s.x, s.y))
        .collect()
}

/// `a:stCxn` / `a:endCxn` of a connector: (shape id, site index).
pub fn cxn_ref(c: &Connector, end: usize) -> Option<(u32, u32)> {
    let name = if end == 0 { "a:stCxn" } else { "a:endCxn" };
    let el = c.nv.drawing_props.as_ref()?.child(name)?;
    Some((el.attr("id")?.parse().ok()?, el.attr("idx")?.parse().ok()?))
}

/// Set or remove `a:stCxn` / `a:endCxn` (CT_NonVisualConnectorProperties:
/// cxnSpLocks, stCxn, endCxn, extLst).
pub fn set_cxn_ref(c: &mut Connector, end: usize, to: Option<(u32, u32)>) {
    let name = if end == 0 { "a:stCxn" } else { "a:endCxn" };
    let dp =
        c.nv.drawing_props
            .get_or_insert_with(|| XmlElement::new("p:cNvCxnSpPr"));
    dp.children
        .retain(|k| !matches!(k, XmlNode::Element(e) if e.name == name));
    let Some((id, idx)) = to else { return };
    let mut el = XmlElement::new(name);
    el.attrs = vec![
        ("id".into(), id.to_string()),
        ("idx".into(), idx.to_string()),
    ];
    let order = ["a:cxnSpLocks", "a:stCxn", "a:endCxn", "a:extLst"];
    let rank = |n: &str| order.iter().position(|o| *o == n).unwrap_or(order.len());
    let me = rank(name);
    let at = dp
        .children
        .iter()
        .position(|k| matches!(k, XmlNode::Element(e) if rank(&e.name) > me))
        .unwrap_or(dp.children.len());
    dp.children.insert(at, XmlNode::Element(el));
}

/// End points (start, end) of a placed connector in slide points.
pub fn ends(p: &Placed, x: &Xfrm) -> [(f64, f64); 2] {
    let f = (x.flip_h.unwrap_or(false), x.flip_v.unwrap_or(false));
    [to_slide(&p.b, f, 0.0, 0.0), to_slide(&p.b, f, p.b.w, p.b.h)]
}

/// Transform of a connector going from `a` to `b` (slide points, top level).
pub fn xfrm_between(base: &Xfrm, a: (f64, f64), b: (f64, f64)) -> Xfrm {
    let emu = |v: f64| (v * EMU_PT).round() as i64;
    Xfrm {
        off: Some((emu(a.0.min(b.0)), emu(a.1.min(b.1)))),
        ext: Some((emu((b.0 - a.0).abs()), emu((b.1 - a.1).abs()))),
        rot: None,
        flip_h: (b.0 < a.0).then_some(true),
        flip_v: (b.1 < a.1).then_some(true),
        ch_off: base.ch_off,
        ch_ext: base.ch_ext,
    }
}

/// Position of connection site `idx` of shape `id` on the slide.
pub fn site_pos(
    pres: &Presentation,
    slide: usize,
    placed: &[Placed],
    id: u32,
    idx: u32,
) -> Option<(f64, f64)> {
    let p = find(placed, id)?;
    let n = super::ops::node_at(pres, slide, &p.path)?;
    sites(pres, slide, p, n).get(idx as usize).copied()
}

/// Visit placed shapes depth-first.
pub fn walk<'p>(ps: &'p [Placed], f: &mut dyn FnMut(&'p Placed)) {
    for p in ps {
        f(p);
        walk(&p.children, f);
    }
}

/// Nearest connection site to a point within `tol` points, excluding `skip`:
/// (shape id, site index, position).
pub fn nearest_site(
    pres: &Presentation,
    slide: usize,
    placed: &[Placed],
    x: f64,
    y: f64,
    tol: f64,
    skip: u32,
) -> Option<(u32, u32, (f64, f64))> {
    let mut best: Option<(u32, u32, (f64, f64), f64)> = None;
    walk(placed, &mut |p| {
        if p.id == skip {
            return;
        }
        if let Some(n) = super::ops::node_at(pres, slide, &p.path) {
            for (i, s) in sites(pres, slide, p, n).into_iter().enumerate() {
                let d = (s.0 - x).hypot(s.1 - y);
                if d <= tol && best.as_ref().is_none_or(|b| d < b.3) {
                    best = Some((p.id, i as u32, s, d));
                }
            }
        }
    });
    best.map(|(id, i, s, _)| (id, i, s))
}

/// Route a top-level connector to the sites it is attached to; `None` when
/// nothing changes.
pub fn reroute(
    pres: &Presentation,
    slide: usize,
    placed: &[Placed],
    p: &Placed,
    c: &Connector,
) -> Option<Connector> {
    let st = cxn_ref(c, 0);
    let en = cxn_ref(c, 1);
    if st.is_none() && en.is_none() {
        return None;
    }
    let x = c.props.xfrm.as_ref()?;
    let [a0, b0] = ends(p, x);
    let a = st
        .and_then(|(id, i)| site_pos(pres, slide, placed, id, i))
        .unwrap_or(a0);
    let b = en
        .and_then(|(id, i)| site_pos(pres, slide, placed, id, i))
        .unwrap_or(b0);
    let nx = xfrm_between(x, a, b);
    if nx == *x {
        return None;
    }
    let mut out = c.clone();
    out.props.xfrm = Some(nx);
    Some(out)
}

/// Endpoints and attachment state of a selected connector (UI).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorOut {
    pub id: u32,
    pub ends: [[f64; 2]; 2],
    pub attached: [bool; 2],
}

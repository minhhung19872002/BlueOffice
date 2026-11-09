//! Shape outline tools (CORE-SHAPE-001): edit points of a freeform
//! (ECMA-376 Part 1, §20.1.9.8 custGeom with literal coordinates, §20.1.9.15
//! path) and merging shapes with polygon boolean operations (union, combine,
//! fragment, intersect, subtract) into new freeforms.
//!
//! The geometry work lives in [`bo_drawingml::freeform`] and
//! [`bo_drawingml::boolean`] (shared with the other DrawingML hosts); this
//! module maps between slide points and path coordinates and builds the
//! undoable transactions.

use super::connect::{flips, to_local, to_slide, visual};
use super::geom::{find, place_slide, BoxR, Placed, EMU_PT};
use super::ops::{self, Tx};
use super::{next_shape_id, node_clone, refuse, the_one, EditError, SlideSel, R};
use bo_drawingml::boolean::{self, BoolOp, Contour, Region};
use bo_drawingml::freeform::{editable, EditPath, PointKind};
use bo_drawingml::geom::FillMode;
use bo_model::slide::{
    CustomGeometry, GeomPath, Geometry, PathCommand, Presentation, ShapeNode, Xfrm,
};
use serde::{Deserialize, Serialize};

/// Flattening tolerance of curves for merging, in points.
const FLATTEN_TOL: f64 = 0.05;

/// Type of an anchor point (how its control points move together).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PointType {
    /// Independent control points.
    Corner,
    /// Collinear control points.
    Smooth,
    /// Collinear control points of equal length.
    Symmetric,
}

impl From<PointKind> for PointType {
    fn from(k: PointKind) -> Self {
        match k {
            PointKind::Corner => PointType::Corner,
            PointKind::Smooth => PointType::Smooth,
            PointKind::Symmetric => PointType::Symmetric,
        }
    }
}

impl From<PointType> for PointKind {
    fn from(k: PointType) -> Self {
        match k {
            PointType::Corner => PointKind::Corner,
            PointType::Smooth => PointKind::Smooth,
            PointType::Symmetric => PointKind::Symmetric,
        }
    }
}

/// One edit of a freeform's outline (slide points; `path` indexes the
/// geometry's paths, `point` / `segment` count across its subpaths).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PointEdit {
    /// Move an anchor point (its control points follow).
    Move {
        /// Path index.
        path: usize,
        /// Anchor index.
        point: usize,
        /// Target x in slide points.
        x: f64,
        /// Target y in slide points.
        y: f64,
    },
    /// Move a control point: `side` 0 = incoming, 1 = outgoing.
    Handle {
        /// Path index.
        path: usize,
        /// Anchor index.
        point: usize,
        /// 0 = incoming control point, 1 = outgoing.
        side: usize,
        /// Target x in slide points.
        x: f64,
        /// Target y in slide points.
        y: f64,
    },
    /// Add an anchor on the outline nearest to (x, y) (within `tolerance` points).
    Add {
        /// Pointer x in slide points.
        x: f64,
        /// Pointer y in slide points.
        y: f64,
        /// Largest distance to the outline, points (default: any).
        #[serde(default)]
        tolerance: Option<f64>,
    },
    /// Delete an anchor point.
    Delete {
        /// Path index.
        path: usize,
        /// Anchor index.
        point: usize,
    },
    /// Make a segment straight or curved.
    Segment {
        /// Path index.
        path: usize,
        /// Segment index.
        segment: usize,
        /// Curved (cubic Bézier) instead of straight.
        curved: bool,
    },
    /// Change the type of an anchor point.
    PointType {
        /// Path index.
        path: usize,
        /// Anchor index.
        point: usize,
        /// New type.
        point_type: PointType,
    },
}

/// Merge operation on two or more selected shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeOp {
    /// One shape covering all of them.
    Union,
    /// Union without the overlaps (exclusive or).
    Combine,
    /// Every piece of the overlay as its own shape.
    Fragment,
    /// The area common to all of them.
    Intersect,
    /// The first selected shape minus the others.
    Subtract,
}

/// An anchor point of the selected freeform (slide points).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointOut {
    /// Position.
    pub x: f64,
    /// Position.
    pub y: f64,
    /// Point type read from the control points.
    pub kind: PointType,
    /// Incoming and outgoing control points (`None` beside straight segments).
    pub handles: [Option<[f64; 2]>; 2],
}

/// A segment of the selected freeform.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentOut {
    /// Curved (cubic Bézier).
    pub curved: bool,
    /// Its middle point (slide points).
    pub mid: [f64; 2],
}

/// One path of the selected freeform.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathOut {
    /// Anchor points.
    pub points: Vec<PointOut>,
    /// Segments.
    pub segments: Vec<SegmentOut>,
    /// The outline as an SVG path in slide points (for the editing overlay).
    pub d: String,
}

/// The selected freeform's editable outline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeformOut {
    /// Shape id.
    pub id: u32,
    /// Paths.
    pub paths: Vec<PathOut>,
}

/// Map between a path's coordinates and slide points for a placed shape.
struct Space {
    b: BoxR,
    f: (bool, bool),
    sx: f64,
    sy: f64,
}

impl Space {
    fn new(b: BoxR, f: (bool, bool), p: &GeomPath) -> Space {
        // §20.1.9.15 path@w/@h: the path's own coordinate space.
        let sx = p.w.filter(|v| *v > 0).map_or(1.0, |v| b.w / v as f64);
        let sy = p.h.filter(|v| *v > 0).map_or(1.0, |v| b.h / v as f64);
        Space { b, f, sx, sy }
    }
    fn to_slide(&self, q: (f64, f64)) -> [f64; 2] {
        let (x, y) = to_slide(&self.b, self.f, q.0 * self.sx, q.1 * self.sy);
        [x, y]
    }
    fn to_path(&self, x: f64, y: f64) -> (f64, f64) {
        let (lx, ly) = to_local(&self.b, self.f, x, y);
        let d = |v: f64, s: f64| if s.abs() > 1e-12 { v / s } else { v };
        (d(lx, self.sx), d(ly, self.sy))
    }
}

fn custom_of(n: &ShapeNode) -> Option<&CustomGeometry> {
    match n {
        ShapeNode::Shape(s) => match &s.props.geometry {
            Some(Geometry::Custom(c)) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

/// Editable outline of a placed freeform (custom geometry with literal paths).
pub fn freeform_out(
    pres: &Presentation,
    slide: usize,
    p: &Placed,
    n: &ShapeNode,
) -> Option<FreeformOut> {
    let c = custom_of(n)?;
    let f = visual(p, flips(pres, slide, n));
    let mut paths = Vec::new();
    for gp in &c.paths {
        let ep = EditPath::parse(gp)?;
        let sp = Space::new(p.b, f, gp);
        let points = (0..ep.point_count())
            .filter_map(|k| {
                let q = ep.point(k)?;
                let [hi, ho] = ep.handles(k);
                let [x, y] = sp.to_slide(q);
                Some(PointOut {
                    x,
                    y,
                    kind: ep.kind(k).into(),
                    handles: [hi.map(|h| sp.to_slide(h)), ho.map(|h| sp.to_slide(h))],
                })
            })
            .collect();
        let segments = (0..ep.segment_count())
            .map(|j| SegmentOut {
                curved: ep.is_curved(j),
                mid: ep.seg_at(j, 0.5).map_or([0.0, 0.0], |q| sp.to_slide(q)),
            })
            .collect();
        let mut d = String::new();
        for s in ep.segs() {
            use bo_drawingml::Seg;
            let pt = |x: f64, y: f64| {
                let [a, b] = sp.to_slide((x, y));
                format!("{a:.3} {b:.3}")
            };
            match s {
                Seg::Move(x, y) => d.push_str(&format!("M{}", pt(x, y))),
                Seg::Line(x, y) => d.push_str(&format!("L{}", pt(x, y))),
                Seg::Quad(a, b, x, y) => d.push_str(&format!("Q{} {}", pt(a, b), pt(x, y))),
                Seg::Cubic(a, b, c, e, x, y) => {
                    d.push_str(&format!("C{} {} {}", pt(a, b), pt(c, e), pt(x, y)));
                }
                Seg::Close => d.push('Z'),
            }
        }
        paths.push(PathOut {
            points,
            segments,
            d,
        });
    }
    Some(FreeformOut { id: p.id, paths })
}

/// Size of a shape in EMU (its own xfrm ext, else its visual box).
fn ext_emu(pres: &Presentation, slide: usize, p: &Placed, n: &ShapeNode) -> (i64, i64) {
    super::geom::node_xfrm(pres, slide, n)
        .and_then(|x| x.ext)
        .unwrap_or((
            (p.b.w * EMU_PT).round() as i64,
            (p.b.h * EMU_PT).round() as i64,
        ))
}

/// Replace a shape's geometry by its editable custom form.
fn editable_node(pres: &Presentation, slide: usize, p: &Placed, n: &ShapeNode) -> R<ShapeNode> {
    let ShapeNode::Shape(s) = n else {
        return refuse("noFreeform");
    };
    if p.b.w < 0.01 || p.b.h < 0.01 {
        return refuse("noFreeform");
    }
    let (w, h) = ext_emu(pres, slide, p, n);
    let c = editable(s.props.geometry.as_ref(), w, h);
    let mut out = s.clone();
    out.props.geometry = Some(Geometry::Custom(c));
    Ok(ShapeNode::Shape(out))
}

/// Edit points: turn the selected shape's outline into a freeform.
pub fn cmd_convert(tx: &mut Tx<'_>, sel: &SlideSel) -> R<()> {
    let (p, n) = the_one(tx, sel)?;
    let new = editable_node(tx.pres, sel.slide, &p, &n)?;
    if new != n {
        tx.replace_node(sel.slide, &p.path, new)?;
    }
    Ok(())
}

/// Apply one point edit to the selected shape (converted to a freeform first).
pub fn cmd_point_edit(tx: &mut Tx<'_>, sel: &SlideSel, edit: &PointEdit) -> R<()> {
    let (p, n) = the_one(tx, sel)?;
    let node = editable_node(tx.pres, sel.slide, &p, &n)?;
    let ShapeNode::Shape(mut s) = node else {
        return refuse("noFreeform");
    };
    let Some(Geometry::Custom(mut c)) = s.props.geometry.take() else {
        return refuse("noFreeform");
    };
    let f = visual(&p, flips(tx.pres, sel.slide, &n));
    let mut eps: Vec<EditPath> = c
        .paths
        .iter()
        .map(EditPath::parse)
        .collect::<Option<_>>()
        .ok_or(EditError::Refused("noFreeform"))?;
    let spaces: Vec<Space> = c.paths.iter().map(|g| Space::new(p.b, f, g)).collect();
    let path_of = |i: usize| -> R<usize> {
        if i < eps.len() {
            Ok(i)
        } else {
            refuse("noPoint")
        }
    };
    let ok = match *edit {
        PointEdit::Move { path, point, x, y } => {
            let i = path_of(path)?;
            let q = spaces[i].to_path(x, y);
            eps[i].move_point(point, q.0, q.1)
        }
        PointEdit::Handle {
            path,
            point,
            side,
            x,
            y,
        } => {
            let i = path_of(path)?;
            let q = spaces[i].to_path(x, y);
            eps[i].move_handle(point, side, q.0, q.1)
        }
        PointEdit::Add { x, y, tolerance } => {
            // Nearest segment over all paths, measured on the slide.
            let mut best: Option<(usize, usize, f64, f64)> = None;
            for (i, ep) in eps.iter().enumerate() {
                let q = spaces[i].to_path(x, y);
                if let Some((j, t, _)) = ep.nearest_segment(q.0, q.1) {
                    let at = ep.seg_at(j, t).map(|v| spaces[i].to_slide(v));
                    let d = at.map_or(f64::MAX, |[ax, ay]| (ax - x).hypot(ay - y));
                    if best.is_none_or(|b| d < b.3) {
                        best = Some((i, j, t, d));
                    }
                }
            }
            match best {
                Some((i, j, t, d)) if tolerance.is_none_or(|tol| d <= tol) => {
                    eps[i].insert_point(j, t).is_some()
                }
                _ => return refuse("noSegment"),
            }
        }
        PointEdit::Delete { path, point } => {
            let i = path_of(path)?;
            eps[i].delete_point(point)
        }
        PointEdit::Segment {
            path,
            segment,
            curved,
        } => {
            let i = path_of(path)?;
            eps[i].set_curved(segment, curved)
        }
        PointEdit::PointType {
            path,
            point,
            point_type,
        } => {
            let i = path_of(path)?;
            eps[i].set_kind(point, point_type.into())
        }
    };
    if !ok {
        return refuse("noPoint");
    }
    // Refit the box to the new outline (local points), so the shape's frame,
    // selection and handles follow the edited points.
    let mut bb: Option<[f64; 4]> = None;
    for (ep, sp) in eps.iter().zip(&spaces) {
        if let Some([x0, y0, x1, y1]) = ep.bounds() {
            let r = [x0 * sp.sx, y0 * sp.sy, x1 * sp.sx, y1 * sp.sy];
            let e = bb.get_or_insert(r);
            e[0] = e[0].min(r[0]);
            e[1] = e[1].min(r[1]);
            e[2] = e[2].max(r[2]);
            e[3] = e[3].max(r[3]);
        }
    }
    let Some([x0, y0, x1, y1]) = bb else {
        return refuse("noPoint");
    };
    let (nw, nh) = ((x1 - x0).max(0.01), (y1 - y0).max(0.01));
    let refit = x0.abs() > 1e-3
        || y0.abs() > 1e-3
        || (x1 - p.b.w).abs() > 1e-3
        || (y1 - p.b.h).abs() > 1e-3;
    for ((ep, sp), gp) in eps.iter_mut().zip(&spaces).zip(c.paths.iter_mut()) {
        if refit {
            let (dx, dy) = (x0 / sp.sx, y0 / sp.sy);
            ep.map(&|(x, y)| (x - dx, y - dy));
            // Same scale: the path space grows with the box.
            if gp.w.is_some_and(|v| v > 0) {
                gp.w = Some((nw / sp.sx).round().max(1.0) as i64);
            }
            if gp.h.is_some_and(|v| v > 0) {
                gp.h = Some((nh / sp.sy).round().max(1.0) as i64);
            }
        }
        *gp = ep.write(gp);
    }
    s.props.geometry = Some(Geometry::Custom(c));
    if refit {
        let base = super::geom::node_xfrm(tx.pres, sel.slide, &n).unwrap_or_default();
        let (cx, cy) = to_slide(&p.b, f, f64::midpoint(x0, x1), f64::midpoint(y0, y1));
        let nb = BoxR {
            cx,
            cy,
            w: nw,
            h: nh,
            rot: p.b.rot,
        };
        s.props.xfrm = Some(p.frame.to_xfrm(&nb, &base));
    }
    tx.replace_node(sel.slide, &p.path, ShapeNode::Shape(s))?;
    Ok(())
}

/// Filled region of a placed shape in slide points (paths with `fill="none"`
/// only outline, §20.1.9.15).
fn region_of(pres: &Presentation, slide: usize, p: &Placed, n: &ShapeNode) -> Option<Region> {
    let ShapeNode::Shape(s) = n else { return None };
    let f = visual(p, flips(pres, slide, n));
    let mut warn = Vec::new();
    let e = bo_drawingml::evaluate_geometry(s.props.geometry.as_ref(), p.b.w, p.b.h, &mut warn);
    let polys: Vec<Vec<Contour>> = e
        .paths
        .iter()
        .filter(|q| q.fill != FillMode::None)
        .map(|q| {
            boolean::flatten(&q.segs, FLATTEN_TOL)
                .into_iter()
                .map(|c| {
                    c.into_iter()
                        .map(|(x, y)| to_slide(&p.b, f, x, y))
                        .collect()
                })
                .collect()
        })
        .collect();
    Some(Region { polys })
}

/// Custom geometry and box (slide points) of contours.
fn geometry_of(cs: &[Contour]) -> Option<(CustomGeometry, Xfrm)> {
    let mut b: Option<[f64; 4]> = None;
    for &(x, y) in cs.iter().flatten() {
        let e = b.get_or_insert([x, y, x, y]);
        e[0] = e[0].min(x);
        e[1] = e[1].min(y);
        e[2] = e[2].max(x);
        e[3] = e[3].max(y);
    }
    let [x0, y0, x1, y1] = b?;
    let emu = |v: f64| (v * EMU_PT).round() as i64;
    let (w, h) = (emu(x1 - x0).max(1), emu(y1 - y0).max(1));
    let mut cmds = Vec::new();
    for c in cs {
        for (i, &(x, y)) in c.iter().enumerate() {
            let (px, py) = (emu(x - x0).to_string(), emu(y - y0).to_string());
            // §20.1.9.16 moveTo, §20.1.9.13 lnTo, §20.1.9.4 close.
            cmds.push(if i == 0 {
                PathCommand::MoveTo(px, py)
            } else {
                PathCommand::LineTo(px, py)
            });
        }
        cmds.push(PathCommand::Close);
    }
    let geom = CustomGeometry {
        rect: Some(["l".into(), "t".into(), "r".into(), "b".into()]),
        paths: vec![GeomPath {
            w: Some(w),
            h: Some(h),
            commands: cmds,
            ..GeomPath::default()
        }],
        ..CustomGeometry::default()
    };
    let x = Xfrm {
        off: Some((emu(x0), emu(y0))),
        ext: Some((w, h)),
        ..Xfrm::default()
    };
    Some((geom, x))
}

/// Merge the selected shapes (selection order: the first one gives the
/// formatting and is what `subtract` subtracts from).
pub fn cmd_merge(tx: &mut Tx<'_>, sel: &mut SlideSel, op: MergeOp) -> R<()> {
    let placed = place_slide(tx.pres, sel.slide);
    let items: Vec<Placed> = sel
        .ids
        .iter()
        .filter_map(|id| find(&placed, *id).cloned())
        .collect();
    if items.len() < 2 {
        return refuse("mergeNeedsTwo");
    }
    let mut regions = Vec::new();
    let mut nodes = Vec::new();
    for p in &items {
        if p.path.len() != 1 {
            return refuse("mergeShapesOnly");
        }
        let n = node_clone(tx.pres, sel.slide, &p.path)?;
        let r =
            region_of(tx.pres, sel.slide, p, &n).ok_or(EditError::Refused("mergeShapesOnly"))?;
        regions.push(r);
        nodes.push(n);
    }
    let bop = match op {
        MergeOp::Union => Some(BoolOp::Union),
        MergeOp::Combine => Some(BoolOp::Xor),
        MergeOp::Intersect => Some(BoolOp::Intersect),
        MergeOp::Subtract => Some(BoolOp::Subtract),
        MergeOp::Fragment => None,
    };
    let pieces: Vec<Vec<Contour>> = if let Some(bop) = bop {
        let r = boolean::combine(&regions, bop);
        if r.is_empty() {
            Vec::new()
        } else {
            vec![r]
        }
    } else {
        boolean::fragment(&regions)
    };
    if pieces.is_empty() {
        return refuse("mergeEmpty");
    }
    let mut out = Vec::new();
    for (next, piece) in (next_shape_id(tx.pres, sel.slide)..).zip(pieces.iter()) {
        // Formatting: the first selected shape covering the piece.
        let from = boolean::sample_inside(piece)
            .and_then(|pt| regions.iter().position(|r| r.contains(pt)))
            .unwrap_or(0);
        let ShapeNode::Shape(mut s) = nodes[from].clone() else {
            return refuse("mergeShapesOnly");
        };
        let (geom, xfrm) = geometry_of(piece).ok_or(EditError::Refused("mergeEmpty"))?;
        s.nv.id = next;
        s.nv.name = format!("Freeform {next}");
        s.nv.placeholder = None;
        s.props.xfrm = Some(xfrm);
        s.props.geometry = Some(Geometry::Custom(geom));
        if op == MergeOp::Fragment && from != 0 {
            s.text = None;
        }
        out.push(ShapeNode::Shape(s));
    }
    // Replace the merged shapes: the result goes where the lowest one was.
    let mut idx: Vec<usize> = items.iter().map(|p| p.path[0]).collect();
    idx.sort_unstable();
    idx.dedup();
    let at = idx[0];
    for i in idx.iter().rev() {
        tx.remove_node(sel.slide, &[*i])?;
    }
    sel.ids = out.iter().filter_map(|n| n.nv().map(|nv| nv.id)).collect();
    tx.insert_nodes(sel.slide, &[], at, out)?;
    Ok(())
}

/// Points of a connector's drawn path on the slide (first path, curves by
/// their end points): for tests and hit testing.
pub fn connector_points(pres: &Presentation, slide: usize, id: u32) -> Option<Vec<(f64, f64)>> {
    let placed = place_slide(pres, slide);
    let p = find(&placed, id)?;
    let n = ops::node_at(pres, slide, &p.path)?;
    let ShapeNode::Connector(c) = n else {
        return None;
    };
    let f = visual(p, flips(pres, slide, n));
    let mut warn = Vec::new();
    let e = bo_drawingml::evaluate_geometry(c.props.geometry.as_ref(), p.b.w, p.b.h, &mut warn);
    let path = e.paths.first()?;
    Some(
        path.segs
            .iter()
            .filter_map(|s| match *s {
                bo_drawingml::Seg::Move(x, y)
                | bo_drawingml::Seg::Line(x, y)
                | bo_drawingml::Seg::Quad(_, _, x, y)
                | bo_drawingml::Seg::Cubic(_, _, _, _, x, y) => Some(to_slide(&p.b, f, x, y)),
                bo_drawingml::Seg::Close => None,
            })
            .collect(),
    )
}

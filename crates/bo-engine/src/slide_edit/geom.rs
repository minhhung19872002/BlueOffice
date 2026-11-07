//! Shape geometry on a slide: where every shape sits in slide points, hit
//! testing (z-order, group transforms), selection handles and the box maths of
//! move / resize / rotate.
//!
//! The placement mirrors `bo_pptx::render` exactly (ECMA-376 Part 1, §20.1.7.6
//! xfrm: `off`/`ext` in the parent's child coordinate space, rotation about the
//! box centre; §20.1.7.5 group `chOff`/`chExt` scale the children), so what is
//! hit is what is drawn.

use bo_model::slide::{Presentation, ShapeNode, Xfrm};
use bo_pptx::inherit::{chain, effective_xfrm, Level};
use serde::{Deserialize, Serialize};

/// EMU per point (§20.1.10.16 ST_Coordinate: 12 700 EMU = 1 pt).
pub const EMU_PT: f64 = 12_700.0;

/// 2D affine transform `[a, b, c, d, e, f]` (same convention as the renderer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aff(pub [f64; 6]);

impl Aff {
    pub const ID: Aff = Aff([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    #[must_use]
    pub fn translate(x: f64, y: f64) -> Aff {
        Aff([1.0, 0.0, 0.0, 1.0, x, y])
    }
    /// `self ∘ o` (apply `o` first).
    #[must_use]
    pub fn then(self, o: Aff) -> Aff {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = o.0;
        Aff([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }
    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
    /// Inverse (identity when singular).
    #[must_use]
    pub fn invert(self) -> Aff {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return Aff::ID;
        }
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        Aff([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)])
    }
    /// Rotation (clockwise, 60000ths °) and flips about the centre of a w × h box.
    pub fn rot_flip(w: f64, h: f64, rot: i32, flip_h: bool, flip_v: bool) -> Aff {
        let ang = f64::from(rot) / 60_000.0 * std::f64::consts::PI / 180.0;
        let (s, c) = ang.sin_cos();
        let sx = if flip_h { -1.0 } else { 1.0 };
        let sy = if flip_v { -1.0 } else { 1.0 };
        Aff::translate(w / 2.0, h / 2.0)
            .then(Aff([c, s, -s, c, 0.0, 0.0]))
            .then(Aff([sx, 0.0, 0.0, sy, 0.0, 0.0]))
            .then(Aff::translate(-w / 2.0, -h / 2.0))
    }
}

/// Coordinate context of a shape container (spTree or group), as in the renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub aff: Aff,
    pub off: (f64, f64),
    pub ch: (f64, f64),
    pub scale: (f64, f64),
    /// Accumulated rotation of the enclosing groups, degrees clockwise.
    pub rot: f64,
}

impl Frame {
    pub fn root() -> Frame {
        Frame {
            aff: Aff::ID,
            off: (0.0, 0.0),
            ch: (0.0, 0.0),
            scale: (1.0, 1.0),
            rot: 0.0,
        }
    }
    /// xfrm → (x, y, w, h) in the frame's affine input space.
    fn rect(&self, x: &Xfrm) -> Option<(f64, f64, f64, f64)> {
        let (ox, oy) = x.off?;
        let (cx, cy) = x.ext?;
        Some((
            self.off.0 + (ox as f64 / EMU_PT - self.ch.0) * self.scale.0,
            self.off.1 + (oy as f64 / EMU_PT - self.ch.1) * self.scale.1,
            cx as f64 / EMU_PT * self.scale.0,
            cy as f64 / EMU_PT * self.scale.1,
        ))
    }
    /// Frame of the children of a group with transform `x`.
    fn group(&self, x: &Xfrm) -> Option<Frame> {
        let r = self.rect(x)?;
        let (chx, chy) = x.ch_off.unwrap_or(x.off.unwrap_or((0, 0)));
        let (chw, chh) = x.ch_ext.unwrap_or(x.ext.unwrap_or((0, 0)));
        let sx = if chw > 0 {
            r.2 / (chw as f64 / EMU_PT)
        } else {
            1.0
        };
        let sy = if chh > 0 {
            r.3 / (chh as f64 / EMU_PT)
        } else {
            1.0
        };
        Some(Frame {
            aff: self.aff.then(Aff::translate(r.0, r.1)).then(Aff::rot_flip(
                r.2,
                r.3,
                x.rot.unwrap_or(0),
                x.flip_h.unwrap_or(false),
                x.flip_v.unwrap_or(false),
            )),
            off: (0.0, 0.0),
            ch: (chx as f64 / EMU_PT, chy as f64 / EMU_PT),
            scale: (sx, sy),
            rot: self.rot + f64::from(x.rot.unwrap_or(0)) / 60_000.0,
        })
    }
    /// A visual box (slide points) → xfrm off/ext/rot in this frame's child space (EMU).
    pub fn to_xfrm(&self, b: &BoxR, base: &Xfrm) -> Xfrm {
        let (ix, iy) = self.aff.invert().apply(b.cx, b.cy);
        let sx = if self.scale.0.abs() > 1e-9 {
            self.scale.0
        } else {
            1.0
        };
        let sy = if self.scale.1.abs() > 1e-9 {
            self.scale.1
        } else {
            1.0
        };
        let qx = self.ch.0 + (ix - self.off.0) / sx;
        let qy = self.ch.1 + (iy - self.off.1) / sy;
        let w = (b.w / sx).max(0.0);
        let h = (b.h / sy).max(0.0);
        let mut x = base.clone();
        x.off = Some((
            ((qx - w / 2.0) * EMU_PT).round() as i64,
            ((qy - h / 2.0) * EMU_PT).round() as i64,
        ));
        x.ext = Some(((w * EMU_PT).round() as i64, (h * EMU_PT).round() as i64));
        let rot = normalize_deg(b.rot - self.rot);
        x.rot = if rot.abs() < 1e-9 {
            base.rot.filter(|r| *r == 0)
        } else {
            Some((rot * 60_000.0).round() as i32 % 21_600_000)
        };
        x
    }
}

/// Normalise an angle to [0, 360).
pub fn normalize_deg(a: f64) -> f64 {
    let r = a.rem_euclid(360.0);
    if (r - 360.0).abs() < 1e-9 {
        0.0
    } else {
        r
    }
}

/// A (possibly rotated) box in slide points: centre, size, rotation (degrees clockwise).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct BoxR {
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64,
}

impl BoxR {
    /// Axis-aligned box from a rectangle.
    #[must_use]
    pub fn from_rect(r: Rect) -> BoxR {
        BoxR {
            cx: r.x + r.w / 2.0,
            cy: r.y + r.h / 2.0,
            w: r.w,
            h: r.h,
            rot: 0.0,
        }
    }
    fn rot_rad(&self) -> f64 {
        self.rot.to_radians()
    }
    /// Local (centre-relative, unrotated) → slide.
    pub fn to_slide(&self, lx: f64, ly: f64) -> (f64, f64) {
        let (s, c) = self.rot_rad().sin_cos();
        (self.cx + lx * c - ly * s, self.cy + lx * s + ly * c)
    }
    /// Slide → local (centre-relative, unrotated).
    pub fn to_local(&self, x: f64, y: f64) -> (f64, f64) {
        let (s, c) = self.rot_rad().sin_cos();
        let (dx, dy) = (x - self.cx, y - self.cy);
        (dx * c + dy * s, -dx * s + dy * c)
    }
    /// Corners nw, ne, se, sw.
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(x, y)| {
            let (a, b) = self.to_slide(x, y);
            [a, b]
        })
    }
    /// Axis-aligned bounds.
    pub fn bounds(&self) -> Rect {
        let c = self.corners();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for [x, y] in c {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
    /// Point inside, with a tolerance in points (thin lines stay clickable).
    pub fn contains(&self, x: f64, y: f64, tol: f64) -> bool {
        let (lx, ly) = self.to_local(x, y);
        lx.abs() <= self.w / 2.0 + tol && ly.abs() <= self.h / 2.0 + tol
    }
    #[must_use]
    pub fn translated(&self, dx: f64, dy: f64) -> BoxR {
        BoxR {
            cx: self.cx + dx,
            cy: self.cy + dy,
            ..*self
        }
    }
}

/// Axis-aligned rectangle in slide points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    #[must_use]
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect {
            x,
            y,
            w: self.right().max(o.right()) - x,
            h: self.bottom().max(o.bottom()) - y,
        }
    }
    pub fn union_all(rs: impl IntoIterator<Item = Rect>) -> Option<Rect> {
        rs.into_iter().reduce(|a, b| a.union(&b))
    }
}

/// A shape placed on the slide.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// `cNvPr@id`.
    pub id: u32,
    /// Index path in the shape tree (spTree → group children).
    pub path: Vec<usize>,
    /// Visual box in slide points.
    pub b: BoxR,
    /// Container frame (maps edits back to the xfrm).
    pub frame: Frame,
    /// Placed children (groups only).
    pub children: Vec<Placed>,
}

/// Transform of a node, using placeholder inheritance for slide shapes without one.
pub fn node_xfrm(pres: &Presentation, slide: usize, n: &ShapeNode) -> Option<Xfrm> {
    match n {
        ShapeNode::Shape(s) => {
            let ch = chain(pres, Level::Slide(slide), s);
            effective_xfrm(&ch)
        }
        ShapeNode::Picture(p) => p.props.xfrm.clone(),
        ShapeNode::Connector(c) => c.props.xfrm.clone(),
        ShapeNode::Group(g) => g.props.xfrm.clone(),
        ShapeNode::Frame(f) => Some(f.xfrm.clone()),
        ShapeNode::Other(_) => None,
    }
}

/// Is the node hidden (`cNvPr@hidden`)?
pub fn is_hidden(n: &ShapeNode) -> bool {
    n.nv().is_some_and(|nv| nv.hidden == Some(true))
}

/// Place every visible shape of a slide (top level first; groups carry their children).
pub fn place_slide(pres: &Presentation, slide: usize) -> Vec<Placed> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    place_nodes(
        pres,
        slide,
        &s.data.shapes,
        Frame::root(),
        &mut Vec::new(),
        &mut out,
    );
    out
}

fn place_nodes(
    pres: &Presentation,
    slide: usize,
    nodes: &[ShapeNode],
    frame: Frame,
    path: &mut Vec<usize>,
    out: &mut Vec<Placed>,
) {
    for (i, n) in nodes.iter().enumerate() {
        path.push(i);
        if let Some(p) = place_node(pres, slide, n, frame, path) {
            out.push(p);
        }
        path.pop();
    }
}

fn place_node(
    pres: &Presentation,
    slide: usize,
    n: &ShapeNode,
    frame: Frame,
    path: &mut Vec<usize>,
) -> Option<Placed> {
    let nv = n.nv()?;
    if is_hidden(n) {
        return None;
    }
    let x = node_xfrm(pres, slide, n);
    if let ShapeNode::Group(g) = n {
        let mut children = Vec::new();
        let inner = match x.as_ref().and_then(|x| frame.group(x)) {
            Some(f) => f,
            None => frame,
        };
        place_nodes(pres, slide, &g.children, inner, path, &mut children);
        let b = match x.as_ref().and_then(|x| frame_box(&frame, x)) {
            Some(b) => b,
            None => BoxR::from_rect(Rect::union_all(children.iter().map(|c| c.b.bounds()))?),
        };
        return Some(Placed {
            id: nv.id,
            path: path.clone(),
            b,
            frame,
            children,
        });
    }
    let b = frame_box(&frame, x.as_ref()?)?;
    Some(Placed {
        id: nv.id,
        path: path.clone(),
        b,
        frame,
        children: Vec::new(),
    })
}

/// Visual box of an xfrm inside a frame.
fn frame_box(frame: &Frame, x: &Xfrm) -> Option<BoxR> {
    let r = frame.rect(x)?;
    let (cx, cy) = frame.aff.apply(r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
    Some(BoxR {
        cx,
        cy,
        w: r.2,
        h: r.3,
        rot: normalize_deg(frame.rot + f64::from(x.rot.unwrap_or(0)) / 60_000.0),
    })
}

/// Find a placed shape (any depth) by id.
pub fn find(placed: &[Placed], id: u32) -> Option<&Placed> {
    for p in placed {
        if p.id == id {
            return Some(p);
        }
        if let Some(c) = find(&p.children, id) {
            return Some(c);
        }
    }
    None
}

/// Hit test at slide point (x, y): topmost first. Returns the top-level shape
/// (or the innermost one when `deep`).
pub fn hit_test(placed: &[Placed], x: f64, y: f64, tol: f64, deep: bool) -> Option<u32> {
    for p in placed.iter().rev() {
        if p.children.is_empty() {
            if p.b.contains(x, y, tol) {
                return Some(p.id);
            }
        } else if let Some(inner) = hit_test(&p.children, x, y, tol, deep) {
            return Some(if deep { inner } else { p.id });
        }
    }
    None
}

/// Selection handle kinds (compass points + rotation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HandleKind {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
    Rot,
}

impl HandleKind {
    /// (sx, sy): which edges the handle moves (−1 left/top, +1 right/bottom).
    pub fn signs(self) -> (f64, f64) {
        match self {
            HandleKind::Nw => (-1.0, -1.0),
            HandleKind::N | HandleKind::Rot => (0.0, -1.0),
            HandleKind::Ne => (1.0, -1.0),
            HandleKind::E => (1.0, 0.0),
            HandleKind::Se => (1.0, 1.0),
            HandleKind::S => (0.0, 1.0),
            HandleKind::Sw => (-1.0, 1.0),
            HandleKind::W => (-1.0, 0.0),
        }
    }
    pub const RESIZE: [HandleKind; 8] = [
        HandleKind::Nw,
        HandleKind::N,
        HandleKind::Ne,
        HandleKind::E,
        HandleKind::Se,
        HandleKind::S,
        HandleKind::Sw,
        HandleKind::W,
    ];
}

/// A handle position in slide points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Handle {
    pub kind: HandleKind,
    pub x: f64,
    pub y: f64,
}

/// Distance of the rotation handle above the top edge, in points.
pub const ROT_HANDLE_OFFSET: f64 = 18.0;

/// The eight resize handles and the rotation handle of a box.
pub fn handles(b: &BoxR) -> Vec<Handle> {
    let mut out: Vec<Handle> = HandleKind::RESIZE
        .iter()
        .map(|&k| {
            let (sx, sy) = k.signs();
            let (x, y) = b.to_slide(sx * b.w / 2.0, sy * b.h / 2.0);
            Handle { kind: k, x, y }
        })
        .collect();
    let (x, y) = b.to_slide(0.0, -b.h / 2.0 - ROT_HANDLE_OFFSET);
    out.push(Handle {
        kind: HandleKind::Rot,
        x,
        y,
    });
    out
}

/// Minimum size of a resized box, in points.
pub const MIN_SIZE: f64 = 1.0;

/// Resize a box by dragging `handle` by (dx, dy) slide points; the opposite
/// handle stays in place. `keep_aspect` keeps the width/height ratio.
pub fn resize_box(b: &BoxR, handle: HandleKind, dx: f64, dy: f64, keep_aspect: bool) -> BoxR {
    let (sx, sy) = handle.signs();
    if handle == HandleKind::Rot {
        return *b;
    }
    // Delta in the box's own (unrotated) axes.
    let (s, c) = b.rot.to_radians().sin_cos();
    let lx = dx * c + dy * s;
    let ly = -dx * s + dy * c;
    let mut w = if sx == 0.0 { b.w } else { b.w + sx * lx };
    let mut h = if sy == 0.0 { b.h } else { b.h + sy * ly };
    w = w.max(MIN_SIZE);
    h = h.max(MIN_SIZE);
    if keep_aspect && b.w > 0.0 && b.h > 0.0 {
        let ratio = b.w / b.h;
        if sx != 0.0 && sy != 0.0 {
            let kx = w / b.w;
            let ky = h / b.h;
            let k = if (kx - 1.0).abs() >= (ky - 1.0).abs() {
                kx
            } else {
                ky
            };
            w = (b.w * k).max(MIN_SIZE);
            h = (b.h * k).max(MIN_SIZE);
        } else if sx != 0.0 {
            h = (w / ratio).max(MIN_SIZE);
        } else {
            w = (h * ratio).max(MIN_SIZE);
        }
    }
    // Keep the anchor (opposite edge) fixed: shift the centre along the moved axes.
    let mcx = if sx == 0.0 { 0.0 } else { sx * (w - b.w) / 2.0 };
    let mcy = if sy == 0.0 { 0.0 } else { sy * (h - b.h) / 2.0 };
    let (cx, cy) = b.to_slide(mcx, mcy);
    BoxR {
        cx,
        cy,
        w,
        h,
        rot: b.rot,
    }
}

/// Scale a box from `from` to `to` (multi-selection resize).
pub fn map_box(b: &BoxR, from: &Rect, to: &Rect) -> BoxR {
    let kx = if from.w > 0.0 { to.w / from.w } else { 1.0 };
    let ky = if from.h > 0.0 { to.h / from.h } else { 1.0 };
    BoxR {
        cx: to.x + (b.cx - from.x) * kx,
        cy: to.y + (b.cy - from.y) * ky,
        w: (b.w * kx).max(MIN_SIZE.min(b.w)),
        h: (b.h * ky).max(MIN_SIZE.min(b.h)),
        rot: b.rot,
    }
}

/// Angle (degrees clockwise from "up") of the pointer around a centre.
pub fn pointer_angle(cx: f64, cy: f64, x: f64, y: f64) -> f64 {
    normalize_deg((x - cx).atan2(-(y - cy)).to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn resize_keeps_opposite_corner() {
        let b = BoxR {
            cx: 50.0,
            cy: 50.0,
            w: 40.0,
            h: 20.0,
            rot: 0.0,
        };
        let r = resize_box(&b, HandleKind::Se, 10.0, 10.0, false);
        assert!(close(r.w, 50.0) && close(r.h, 30.0));
        assert_eq!(r.corners()[0], b.corners()[0]);
        // Aspect lock on a corner: the larger relative change wins.
        let r = resize_box(&b, HandleKind::Se, 40.0, 0.0, true);
        assert!(close(r.w, 80.0) && close(r.h, 40.0));
        // Aspect lock on an edge.
        let r = resize_box(&b, HandleKind::E, 20.0, 0.0, true);
        assert!(close(r.w, 60.0) && close(r.h, 30.0));
        // West handle moves the left edge only.
        let r = resize_box(&b, HandleKind::W, -10.0, 5.0, false);
        assert!(close(r.w, 50.0) && close(r.h, 20.0) && close(r.cx, 45.0));
    }

    #[test]
    fn resize_rotated_box_in_local_axes() {
        let b = BoxR {
            cx: 0.0,
            cy: 0.0,
            w: 40.0,
            h: 20.0,
            rot: 90.0,
        };
        // Rotated 90°: the local +x axis points down in slide space.
        let r = resize_box(&b, HandleKind::E, 0.0, 10.0, false);
        assert!(close(r.w, 50.0) && close(r.h, 20.0));
        let w_before = b.to_slide(-20.0, 0.0);
        let w_after = r.to_slide(-25.0, 0.0);
        assert!(close(w_before.0, w_after.0) && close(w_before.1, w_after.1));
    }

    #[test]
    fn rotated_hit_and_handles() {
        let b = BoxR {
            cx: 100.0,
            cy: 100.0,
            w: 100.0,
            h: 10.0,
            rot: 90.0,
        };
        assert!(b.contains(100.0, 140.0, 0.0));
        assert!(!b.contains(140.0, 100.0, 0.0));
        let h = handles(&b);
        let rot = h.iter().find(|h| h.kind == HandleKind::Rot).unwrap();
        assert!(close(rot.x, 100.0 + 5.0 + ROT_HANDLE_OFFSET) && close(rot.y, 100.0));
        assert!(close(pointer_angle(0.0, 0.0, 1.0, 0.0), 90.0));
        assert!(close(pointer_angle(0.0, 0.0, 0.0, -1.0), 0.0));
    }

    #[test]
    fn frame_round_trip() {
        let parent = Frame::root();
        let x = Xfrm {
            off: Some((127_000, 254_000)),
            ext: Some((1_270_000, 635_000)),
            ch_off: Some((0, 0)),
            ch_ext: Some((2_540_000, 1_270_000)),
            rot: Some(5_400_000),
            ..Xfrm::default()
        };
        let g = parent.group(&x).unwrap();
        let child = Xfrm {
            off: Some((254_000, 127_000)),
            ext: Some((508_000, 254_000)),
            ..Xfrm::default()
        };
        let b = frame_box(&g, &child).unwrap();
        assert!(close(b.w, 20.0) && close(b.h, 10.0) && close(b.rot, 90.0));
        let back = g.to_xfrm(&b, &child);
        assert_eq!(back.off, child.off);
        assert_eq!(back.ext, child.ext);
        assert_eq!(back.rot, None);
    }
}

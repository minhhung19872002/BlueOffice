//! Freeform editing ("edit points") of shape outlines: a preset or formula
//! geometry is baked into a custom geometry with literal coordinates
//! (ECMA-376 Part 1, §20.1.9.8 custGeom, §20.1.9.15 path), whose anchor points,
//! Bézier control points and segments can then be edited.
//!
//! A path is modelled as subpaths of anchor points joined by straight or cubic
//! segments (§20.1.9.13 lnTo, §20.1.9.7 cubicBezTo; §20.1.9.21 quadBezTo is
//! raised to a cubic, which is exact). Point types are not stored in the file
//! format; they are read from the geometry: a *smooth* point has collinear
//! control points, a *symmetric* one also has equal handle lengths, any other
//! point is a *corner*.

use crate::boolean::{cubic_at, Pt};
use crate::dom::El;
use crate::geom::{evaluate, FillMode, Seg};
use bo_model::slide::{CustomGeometry, Extra, GeomPath, Geometry, Guide, PathCommand};

/// Kind of a segment between two anchor points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Span {
    /// Straight line.
    Line,
    /// Cubic Bézier with two control points.
    Cubic(Pt, Pt),
}

/// Type of an anchor point (how its two control points move together).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointKind {
    /// Independent control points (a sharp corner).
    Corner,
    /// Collinear control points (a smooth curve), lengths independent.
    Smooth,
    /// Collinear control points of equal length.
    Symmetric,
}

/// One subpath: anchors and the segments between them.
#[derive(Debug, Clone, PartialEq)]
pub struct SubPath {
    /// Anchor points.
    pub pts: Vec<Pt>,
    /// Segment `i` joins `pts[i]` to `pts[(i + 1) % n]`; a closed subpath has
    /// one segment per point (the last one closes it), an open one one less.
    pub segs: Vec<Span>,
    /// Closed (§20.1.9.4 close).
    pub closed: bool,
}

/// An editable path (coordinates in the path's own space, §20.1.9.15 w/h).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditPath {
    /// Subpaths in drawing order.
    pub subs: Vec<SubPath>,
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn len(a: Pt) -> f64 {
    a.0.hypot(a.1)
}

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

impl EditPath {
    /// Read a path whose coordinates are all literal numbers (no guides, no
    /// arcs); `None` otherwise.
    pub fn parse(p: &GeomPath) -> Option<EditPath> {
        let mut out = EditPath::default();
        let mut cur: Option<SubPath> = None;
        let mut pen = (0.0, 0.0);
        let pt = |x: &str, y: &str| Some((num(x)?, num(y)?));
        let start_if_needed = |cur: &mut Option<SubPath>, pen: Pt| {
            if cur.is_none() {
                *cur = Some(SubPath {
                    pts: vec![pen],
                    segs: Vec::new(),
                    closed: false,
                });
            }
        };
        for c in &p.commands {
            match c {
                PathCommand::MoveTo(x, y) => {
                    if let Some(s) = cur.take() {
                        out.subs.push(s);
                    }
                    pen = pt(x, y)?;
                    cur = Some(SubPath {
                        pts: vec![pen],
                        segs: Vec::new(),
                        closed: false,
                    });
                }
                PathCommand::LineTo(x, y) => {
                    start_if_needed(&mut cur, pen);
                    pen = pt(x, y)?;
                    let s = cur.as_mut()?;
                    s.segs.push(Span::Line);
                    s.pts.push(pen);
                }
                PathCommand::QuadTo(a) => {
                    start_if_needed(&mut cur, pen);
                    let q = pt(&a[0], &a[1])?;
                    let e = pt(&a[2], &a[3])?;
                    // Degree elevation: c1 = p0 + ⅔(q − p0), c2 = e + ⅔(q − e).
                    let c1 = lerp(pen, q, 2.0 / 3.0);
                    let c2 = lerp(e, q, 2.0 / 3.0);
                    let s = cur.as_mut()?;
                    s.segs.push(Span::Cubic(c1, c2));
                    s.pts.push(e);
                    pen = e;
                }
                PathCommand::CubicTo(a) => {
                    start_if_needed(&mut cur, pen);
                    let c1 = pt(&a[0], &a[1])?;
                    let c2 = pt(&a[2], &a[3])?;
                    let e = pt(&a[4], &a[5])?;
                    let s = cur.as_mut()?;
                    s.segs.push(Span::Cubic(c1, c2));
                    s.pts.push(e);
                    pen = e;
                }
                PathCommand::ArcTo { .. } => return None,
                PathCommand::Close => {
                    if let Some(mut s) = cur.take() {
                        let n = s.pts.len();
                        if n > 1 && len(sub(s.pts[n - 1], s.pts[0])) < 0.5 {
                            // The last segment already returns to the start.
                            s.pts.pop();
                        } else {
                            s.segs.push(Span::Line);
                        }
                        s.closed = true;
                        pen = s.pts[0];
                        out.subs.push(s);
                    }
                }
            }
        }
        if let Some(s) = cur.take() {
            out.subs.push(s);
        }
        // Lone move-tos draw nothing.
        out.subs.retain(|s| !s.segs.is_empty());
        Some(out)
    }

    /// Write the path back with the attributes of `like` (w, h, fill, stroke).
    pub fn write(&self, like: &GeomPath) -> GeomPath {
        let f = |v: f64| format!("{}", v.round() as i64);
        let mut cmds = Vec::new();
        for s in &self.subs {
            let n = s.pts.len();
            if n == 0 {
                continue;
            }
            cmds.push(PathCommand::MoveTo(f(s.pts[0].0), f(s.pts[0].1)));
            for (i, sp) in s.segs.iter().enumerate() {
                let to = s.pts[(i + 1) % n];
                let closing = s.closed && i + 1 == n;
                match sp {
                    Span::Line if closing => {}
                    Span::Line => cmds.push(PathCommand::LineTo(f(to.0), f(to.1))),
                    Span::Cubic(a, b) => cmds.push(PathCommand::CubicTo([
                        f(a.0),
                        f(a.1),
                        f(b.0),
                        f(b.1),
                        f(to.0),
                        f(to.1),
                    ])),
                }
            }
            if s.closed {
                cmds.push(PathCommand::Close);
            }
        }
        GeomPath {
            commands: cmds,
            ..like.clone()
        }
    }

    /// Evaluated segments (path space).
    pub fn segs(&self) -> Vec<Seg> {
        let mut out = Vec::new();
        for s in &self.subs {
            let n = s.pts.len();
            if n == 0 {
                continue;
            }
            out.push(Seg::Move(s.pts[0].0, s.pts[0].1));
            for (i, sp) in s.segs.iter().enumerate() {
                let to = s.pts[(i + 1) % n];
                match sp {
                    Span::Line => out.push(Seg::Line(to.0, to.1)),
                    Span::Cubic(a, b) => out.push(Seg::Cubic(a.0, a.1, b.0, b.1, to.0, to.1)),
                }
            }
            if s.closed {
                out.push(Seg::Close);
            }
        }
        out
    }

    /// Number of anchor points.
    pub fn point_count(&self) -> usize {
        self.subs.iter().map(|s| s.pts.len()).sum()
    }

    /// Number of segments.
    pub fn segment_count(&self) -> usize {
        self.subs.iter().map(|s| s.segs.len()).sum()
    }

    /// (subpath, index) of anchor `k`.
    fn locate(&self, k: usize) -> Option<(usize, usize)> {
        let mut k = k;
        for (si, s) in self.subs.iter().enumerate() {
            if k < s.pts.len() {
                return Some((si, k));
            }
            k -= s.pts.len();
        }
        None
    }

    /// (subpath, index) of segment `j`.
    fn locate_seg(&self, j: usize) -> Option<(usize, usize)> {
        let mut j = j;
        for (si, s) in self.subs.iter().enumerate() {
            if j < s.segs.len() {
                return Some((si, j));
            }
            j -= s.segs.len();
        }
        None
    }

    /// Anchor `k`.
    pub fn point(&self, k: usize) -> Option<Pt> {
        let (s, i) = self.locate(k)?;
        Some(self.subs[s].pts[i])
    }

    /// Segment indices (within the subpath) entering and leaving anchor `i`.
    fn around(s: &SubPath, i: usize) -> (Option<usize>, Option<usize>) {
        let n = s.pts.len();
        let jin = if i > 0 {
            Some(i - 1)
        } else if s.closed {
            Some(n - 1)
        } else {
            None
        };
        let jout = (i < s.segs.len()).then_some(i);
        (jin, jout)
    }

    /// Control points of anchor `k`: incoming (end of the previous curve) and
    /// outgoing (start of the next curve); `None` beside straight segments.
    pub fn handles(&self, k: usize) -> [Option<Pt>; 2] {
        let Some((si, i)) = self.locate(k) else {
            return [None, None];
        };
        let s = &self.subs[si];
        let (jin, jout) = Self::around(s, i);
        let hin = jin.and_then(|j| match s.segs[j] {
            Span::Cubic(_, b) => Some(b),
            Span::Line => None,
        });
        let hout = jout.and_then(|j| match s.segs[j] {
            Span::Cubic(a, _) => Some(a),
            Span::Line => None,
        });
        [hin, hout]
    }

    /// Type of anchor `k`, read from its control points.
    pub fn kind(&self, k: usize) -> PointKind {
        let Some(p) = self.point(k) else {
            return PointKind::Corner;
        };
        let [Some(hin), Some(hout)] = self.handles(k) else {
            return PointKind::Corner;
        };
        let a = sub(p, hin);
        let b = sub(hout, p);
        let (la, lb) = (len(a), len(b));
        if la < 1e-9 || lb < 1e-9 {
            return PointKind::Corner;
        }
        let sin = (a.0 * b.1 - a.1 * b.0) / (la * lb);
        let cos = (a.0 * b.0 + a.1 * b.1) / (la * lb);
        if sin.abs() > 0.01 || cos <= 0.0 {
            PointKind::Corner
        } else if (la - lb).abs() <= 0.01 * la.max(lb) {
            PointKind::Symmetric
        } else {
            PointKind::Smooth
        }
    }

    /// Is segment `j` curved?
    pub fn is_curved(&self, j: usize) -> bool {
        self.locate_seg(j)
            .is_some_and(|(s, i)| matches!(self.subs[s].segs[i], Span::Cubic(..)))
    }

    /// Point of segment `j` at parameter `t`.
    pub fn seg_at(&self, j: usize, t: f64) -> Option<Pt> {
        let (si, i) = self.locate_seg(j)?;
        let s = &self.subs[si];
        let p0 = s.pts[i];
        let p1 = s.pts[(i + 1) % s.pts.len()];
        Some(match s.segs[i] {
            Span::Line => lerp(p0, p1, t),
            Span::Cubic(a, b) => cubic_at(p0, a, b, p1, t),
        })
    }

    /// Nearest segment to a point: (segment, parameter, distance).
    pub fn nearest_segment(&self, x: f64, y: f64) -> Option<(usize, f64, f64)> {
        const N: usize = 64;
        let mut best: Option<(usize, f64, f64)> = None;
        for j in 0..self.segment_count() {
            let d = |t: f64| {
                self.seg_at(j, t)
                    .map_or(f64::MAX, |p| (p.0 - x).hypot(p.1 - y))
            };
            let (mut bt, mut bd) = (0.0, f64::MAX);
            for k in 0..=N {
                let t = k as f64 / N as f64;
                let v = d(t);
                if v < bd {
                    (bt, bd) = (t, v);
                }
            }
            // Refine by ternary search around the best sample.
            let (mut lo, mut hi) = (
                (bt - 1.0 / N as f64).max(0.0),
                (bt + 1.0 / N as f64).min(1.0),
            );
            for _ in 0..40 {
                let m1 = lo + (hi - lo) / 3.0;
                let m2 = hi - (hi - lo) / 3.0;
                if d(m1) <= d(m2) {
                    hi = m2;
                } else {
                    lo = m1;
                }
            }
            let t = f64::midpoint(lo, hi);
            if d(t) < bd {
                (bt, bd) = (t, d(t));
            }
            if best.is_none_or(|b| bd < b.2) {
                best = Some((j, bt, bd));
            }
        }
        best
    }

    /// Move anchor `k` to (x, y); its control points move with it.
    pub fn move_point(&mut self, k: usize, x: f64, y: f64) -> bool {
        let Some((si, i)) = self.locate(k) else {
            return false;
        };
        let s = &mut self.subs[si];
        let p = s.pts[i];
        let d = (x - p.0, y - p.1);
        s.pts[i] = (x, y);
        let (jin, jout) = Self::around(s, i);
        if let Some(j) = jin {
            if let Span::Cubic(a, b) = s.segs[j] {
                s.segs[j] = Span::Cubic(a, (b.0 + d.0, b.1 + d.1));
            }
        }
        if let Some(j) = jout {
            if let Span::Cubic(a, b) = s.segs[j] {
                s.segs[j] = Span::Cubic((a.0 + d.0, a.1 + d.1), b);
            }
        }
        true
    }

    /// Move control point `side` (0 = incoming, 1 = outgoing) of anchor `k`;
    /// the other one follows the point's type (smooth: stays collinear,
    /// symmetric: mirrored, corner: stays).
    pub fn move_handle(&mut self, k: usize, side: usize, x: f64, y: f64) -> bool {
        let kind = self.kind(k);
        let Some((si, i)) = self.locate(k) else {
            return false;
        };
        let s = &mut self.subs[si];
        let p = s.pts[i];
        let (jin, jout) = Self::around(s, i);
        let (mine, other) = if side == 0 { (jin, jout) } else { (jout, jin) };
        let Some(j) = mine else { return false };
        let Span::Cubic(a, b) = s.segs[j] else {
            return false;
        };
        s.segs[j] = if side == 0 {
            Span::Cubic(a, (x, y))
        } else {
            Span::Cubic((x, y), b)
        };
        let v = (x - p.0, y - p.1);
        let lv = len(v);
        if kind == PointKind::Corner || lv < 1e-9 {
            return true;
        }
        if let Some(o) = other {
            if let Span::Cubic(a, b) = s.segs[o] {
                let cur = if side == 0 { a } else { b };
                let l = if kind == PointKind::Symmetric {
                    lv
                } else {
                    len(sub(cur, p))
                };
                let h = (p.0 - v.0 / lv * l, p.1 - v.1 / lv * l);
                s.segs[o] = if side == 0 {
                    Span::Cubic(h, b)
                } else {
                    Span::Cubic(a, h)
                };
            }
        }
        true
    }

    /// Add an anchor on segment `j` at parameter `t` (the outline is unchanged:
    /// lines are split, curves subdivided with de Casteljau's construction).
    /// Returns the new anchor's index.
    pub fn insert_point(&mut self, j: usize, t: f64) -> Option<usize> {
        let (si, i) = self.locate_seg(j)?;
        let t = t.clamp(0.0, 1.0);
        let first: usize = self.subs[..si].iter().map(|s| s.pts.len()).sum();
        let s = &mut self.subs[si];
        let n = s.pts.len();
        let p0 = s.pts[i];
        let p1 = s.pts[(i + 1) % n];
        match s.segs[i] {
            Span::Line => {
                s.pts.insert(i + 1, lerp(p0, p1, t));
                s.segs.insert(i + 1, Span::Line);
            }
            Span::Cubic(a, b) => {
                let ab = lerp(p0, a, t);
                let bc = lerp(a, b, t);
                let cd = lerp(b, p1, t);
                let abc = lerp(ab, bc, t);
                let bcd = lerp(bc, cd, t);
                let m = lerp(abc, bcd, t);
                s.segs[i] = Span::Cubic(ab, abc);
                s.segs.insert(i + 1, Span::Cubic(bcd, cd));
                s.pts.insert(i + 1, m);
            }
        }
        Some(first + i + 1)
    }

    /// Delete anchor `k`, joining its two segments (an open subpath keeps at
    /// least two anchors, a closed one three).
    pub fn delete_point(&mut self, k: usize) -> bool {
        let Some((si, i)) = self.locate(k) else {
            return false;
        };
        let s = &mut self.subs[si];
        let n = s.pts.len();
        let join = |prev: Pt, next: Pt, a: Span, b: Span| match (a, b) {
            (Span::Line, Span::Line) => Span::Line,
            _ => Span::Cubic(
                match a {
                    Span::Cubic(c1, _) => c1,
                    Span::Line => lerp(prev, next, 1.0 / 3.0),
                },
                match b {
                    Span::Cubic(_, c2) => c2,
                    Span::Line => lerp(prev, next, 2.0 / 3.0),
                },
            ),
        };
        if s.closed {
            if n <= 3 {
                return false;
            }
            let jin = (i + n - 1) % n;
            let merged = join(s.pts[jin], s.pts[(i + 1) % n], s.segs[jin], s.segs[i]);
            if i == 0 {
                s.pts.remove(0);
                s.segs.remove(0);
                let last = s.segs.len() - 1;
                s.segs[last] = merged;
            } else {
                s.segs[i - 1] = merged;
                s.segs.remove(i);
                s.pts.remove(i);
            }
        } else {
            if n <= 2 {
                return false;
            }
            if i == 0 {
                s.pts.remove(0);
                s.segs.remove(0);
            } else if i == n - 1 {
                s.pts.pop();
                s.segs.pop();
            } else {
                s.segs[i - 1] = join(s.pts[i - 1], s.pts[i + 1], s.segs[i - 1], s.segs[i]);
                s.segs.remove(i);
                s.pts.remove(i);
            }
        }
        true
    }

    /// Make segment `j` straight or curved (a straight segment becomes a curve
    /// with its control points at a third and two thirds: same outline).
    pub fn set_curved(&mut self, j: usize, curved: bool) -> bool {
        let Some((si, i)) = self.locate_seg(j) else {
            return false;
        };
        let s = &mut self.subs[si];
        let p0 = s.pts[i];
        let p1 = s.pts[(i + 1) % s.pts.len()];
        s.segs[i] = match (s.segs[i], curved) {
            (Span::Line, true) => Span::Cubic(lerp(p0, p1, 1.0 / 3.0), lerp(p0, p1, 2.0 / 3.0)),
            (Span::Cubic(..), false) => Span::Line,
            (same, _) => same,
        };
        true
    }

    /// Change the type of anchor `k`. Smooth and symmetric points get collinear
    /// control points along the direction from the previous to the next anchor
    /// (straight neighbouring segments become curves); a corner gets its
    /// control points pointing at the neighbouring anchors.
    pub fn set_kind(&mut self, k: usize, kind: PointKind) -> bool {
        let Some((si, i)) = self.locate(k) else {
            return false;
        };
        let s = &mut self.subs[si];
        let n = s.pts.len();
        let p = s.pts[i];
        let (jin, jout) = Self::around(s, i);
        let prev = jin.map(|j| s.pts[j]);
        let next = jout.map(|j| s.pts[(j + 1) % n]);
        match kind {
            PointKind::Corner => {
                if let (Some(j), Some(q)) = (jin, prev) {
                    if let Span::Cubic(a, _) = s.segs[j] {
                        s.segs[j] = Span::Cubic(a, lerp(p, q, 1.0 / 3.0));
                    }
                }
                if let (Some(j), Some(q)) = (jout, next) {
                    if let Span::Cubic(_, b) = s.segs[j] {
                        s.segs[j] = Span::Cubic(lerp(p, q, 1.0 / 3.0), b);
                    }
                }
            }
            PointKind::Smooth | PointKind::Symmetric => {
                let (Some(ji), Some(jo), Some(q0), Some(q1)) = (jin, jout, prev, next) else {
                    return false;
                };
                let mut d = sub(q1, q0);
                if len(d) < 1e-9 {
                    d = (1.0, 0.0);
                }
                let ld = len(d);
                let d = (d.0 / ld, d.1 / ld);
                let lin = match s.segs[ji] {
                    Span::Cubic(_, b) if len(sub(p, b)) > 1e-9 => len(sub(p, b)),
                    _ => len(sub(p, q0)) / 3.0,
                };
                let lout = match s.segs[jo] {
                    Span::Cubic(a, _) if len(sub(a, p)) > 1e-9 => len(sub(a, p)),
                    _ => len(sub(q1, p)) / 3.0,
                };
                let (lin, lout) = if kind == PointKind::Symmetric {
                    let m = f64::midpoint(lin, lout);
                    (m, m)
                } else {
                    (lin, lout)
                };
                let hin = (p.0 - d.0 * lin, p.1 - d.1 * lin);
                let hout = (p.0 + d.0 * lout, p.1 + d.1 * lout);
                let a = match s.segs[ji] {
                    Span::Cubic(a, _) => a,
                    Span::Line => lerp(q0, p, 1.0 / 3.0),
                };
                s.segs[ji] = Span::Cubic(a, hin);
                let b = match s.segs[jo] {
                    Span::Cubic(_, b) => b,
                    Span::Line => lerp(p, q1, 2.0 / 3.0),
                };
                s.segs[jo] = Span::Cubic(hout, b);
            }
        }
        true
    }

    /// Apply a map to every anchor and control point.
    pub fn map(&mut self, f: &dyn Fn(Pt) -> Pt) {
        for s in &mut self.subs {
            for p in &mut s.pts {
                *p = f(*p);
            }
            for sp in &mut s.segs {
                if let Span::Cubic(a, b) = sp {
                    *sp = Span::Cubic(f(*a), f(*b));
                }
            }
        }
    }

    /// Bounds of the drawn outline `[minx, miny, maxx, maxy]` (curves sampled).
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b: Option<[f64; 4]> = None;
        let mut add = |p: Pt| {
            let r = b.get_or_insert([p.0, p.1, p.0, p.1]);
            r[0] = r[0].min(p.0);
            r[1] = r[1].min(p.1);
            r[2] = r[2].max(p.0);
            r[3] = r[3].max(p.1);
        };
        for j in 0..self.segment_count() {
            let curved = self.is_curved(j);
            let n = if curved { 32 } else { 1 };
            for k in 0..=n {
                if let Some(p) = self.seg_at(j, k as f64 / n as f64) {
                    add(p);
                }
            }
        }
        for s in &self.subs {
            for &p in &s.pts {
                add(p);
            }
        }
        b
    }
}

fn fmt(v: f64) -> String {
    format!("{}", v.round() as i64)
}

/// `a:cxnLst` whose sites are guides `cxnX{i}` / `cxnY{i}` (§20.1.9.10 cxnLst,
/// §20.1.9.9 cxn: `ang` in 60000ths of a degree, §20.1.9.17 pos).
pub fn cxn_list(angles: &[f64]) -> Extra {
    let mut l = El::new("a:cxnLst");
    for (i, &ang) in angles.iter().enumerate() {
        let a = ((ang * 60_000.0).round() as i64).rem_euclid(21_600_000);
        l = l.child(
            El::new("a:cxn")
                .a("ang", a)
                .child(
                    El::new("a:pos")
                        .a("x", format!("cxnX{i}"))
                        .a("y", format!("cxnY{i}"))
                        .done(),
                )
                .done(),
        );
    }
    // Position key of cxnLst in CT_CustomGeometry2D (avLst, gdLst, ahLst, cxnLst, …).
    Extra {
        key: 8,
        xml: l.done(),
    }
}

/// Most connection sites a freeform gets from its outline.
pub const MAX_OUTLINE_SITES: usize = 64;

/// Does the custom geometry carry connection sites made by [`outline_sites`]?
pub fn has_outline_sites(c: &CustomGeometry) -> bool {
    c.gd.iter().any(|g| g.name == "vtxX0")
}

/// Connection sites of a freeform at the vertices of its outline (CORE-SHAPE-001: after edit
/// points or a merge the glue points follow the new outline, as a freeform's do): each end
/// point of the literal paths (the closing point of a contour once), pointing away from the
/// paths' centre. The sites replace the previous `a:cxnLst` (§20.1.9.10) and its guides; they
/// are guides proportional to the shape's size (§20.1.9.11), so they scale with it. Nothing
/// changes when a path is not literal (guide references, arcs).
pub fn outline_sites(c: &mut CustomGeometry) {
    let num = |s: &str| s.trim().parse::<f64>().ok();
    // (x, y, path w, path h) per vertex.
    let mut pts: Vec<(f64, f64, Option<i64>, Option<i64>)> = Vec::new();
    for p in &c.paths {
        let mut start: Option<(f64, f64)> = None;
        let mut last: Option<(f64, f64)> = None;
        for cmd in &p.commands {
            let end = match cmd {
                PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => Some((num(x), num(y))),
                PathCommand::QuadTo(v) => Some((num(&v[2]), num(&v[3]))),
                PathCommand::CubicTo(v) => Some((num(&v[4]), num(&v[5]))),
                PathCommand::ArcTo { .. } => return,
                PathCommand::Close => {
                    // The closing point repeats the contour's start: drop it.
                    if let (Some(s), Some(l)) = (start, last) {
                        if (s.0 - l.0).abs() < 0.5 && (s.1 - l.1).abs() < 0.5 && pts.len() > 1 {
                            pts.pop();
                        }
                    }
                    start = None;
                    None
                }
            };
            let Some((x, y)) = end else { continue };
            let (Some(x), Some(y)) = (x, y) else { return };
            if matches!(cmd, PathCommand::MoveTo(..)) {
                start = Some((x, y));
            }
            last = Some((x, y));
            pts.push((x, y, p.w, p.h));
        }
    }
    if pts.is_empty() {
        return;
    }
    // Evenly spread when the outline has many points (flattened curves of a merge).
    if pts.len() > MAX_OUTLINE_SITES {
        let n = pts.len();
        pts = (0..MAX_OUTLINE_SITES)
            .map(|i| pts[i * n / MAX_OUTLINE_SITES])
            .collect();
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y, _, _) in &pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (cx, cy) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    c.gd.retain(|g| {
        !(g.name.starts_with("cxnX")
            || g.name.starts_with("cxnY")
            || g.name.starts_with("vtxX")
            || g.name.starts_with("vtxY"))
    });
    c.extra.retain(|e| e.xml.local() != "cxnLst");
    let guide = |name: String, axis: &str, v: f64, of: Option<i64>| match of.filter(|v| *v > 0) {
        Some(of) => rel_guide(name, axis, v, of),
        // Path in shape coordinates (no path size): the value itself.
        None => Guide {
            name,
            fmla: format!("val {}", v.round() as i64),
        },
    };
    let mut angles = Vec::new();
    for (i, &(x, y, w, h)) in pts.iter().enumerate() {
        c.gd.push(guide(format!("vtxX{i}"), "w", x, w));
        c.gd.push(guide(format!("vtxY{i}"), "h", y, h));
        // §20.1.9.9 cxn@ang: degrees clockwise from +x (y grows down).
        let (dx, dy) = (x - cx, y - cy);
        angles.push(if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            0.0
        } else {
            dy.atan2(dx).to_degrees().rem_euclid(360.0)
        });
    }
    let mut list = cxn_list(&angles);
    // The positions name the vertex guides.
    for (i, e) in list
        .xml
        .children
        .iter_mut()
        .filter_map(|n| match n {
            bo_model::slide::XmlNode::Element(e) => Some(e),
            bo_model::slide::XmlNode::Text(_) => None,
        })
        .enumerate()
    {
        for p in &mut e.children {
            if let bo_model::slide::XmlNode::Element(pos) = p {
                pos.attrs = vec![
                    ("x".into(), format!("vtxX{i}")),
                    ("y".into(), format!("vtxY{i}")),
                ];
            }
        }
    }
    let at = c
        .extra
        .iter()
        .position(|e| e.key > list.key)
        .unwrap_or(c.extra.len());
    c.extra.insert(at, list);
}

/// Guide `name = v / of` of the shape's width or height (`*/ w v of`, §20.1.9.11),
/// so text rectangles and connection sites scale with the shape.
fn rel_guide(name: String, axis: &str, v: f64, of: i64) -> Guide {
    Guide {
        name,
        fmla: format!("*/ {axis} {} {}", v.round() as i64, of.max(1)),
    }
}

/// Bake any geometry, evaluated for a `w × h` box (EMU), into a custom
/// geometry with literal path coordinates (arcs become cubic Béziers); the text
/// rectangle and the connection sites become guides proportional to the shape's
/// size (§20.1.9.8 custGeom, §20.1.9.12 gdLst, §20.1.9.22 rect).
pub fn bake(geom: Option<&Geometry>, w: i64, h: i64) -> CustomGeometry {
    let (wf, hf) = (w as f64, h as f64);
    let mut warn = Vec::new();
    let e = evaluate(geom, wf, hf, &mut warn);
    let mut out = CustomGeometry::default();
    for p in &e.paths {
        let mut cmds = Vec::new();
        for s in &p.segs {
            cmds.push(match *s {
                Seg::Move(x, y) => PathCommand::MoveTo(fmt(x), fmt(y)),
                Seg::Line(x, y) => PathCommand::LineTo(fmt(x), fmt(y)),
                Seg::Quad(a, b, x, y) => PathCommand::QuadTo([fmt(a), fmt(b), fmt(x), fmt(y)]),
                Seg::Cubic(a, b, c, d, x, y) => {
                    PathCommand::CubicTo([fmt(a), fmt(b), fmt(c), fmt(d), fmt(x), fmt(y)])
                }
                Seg::Close => PathCommand::Close,
            });
        }
        out.paths.push(GeomPath {
            w: (w > 0).then_some(w),
            h: (h > 0).then_some(h),
            fill: fill_name(p.fill).map(str::to_owned),
            stroke: (!p.stroke).then_some(false),
            extrusion_ok: None,
            commands: cmds,
        });
    }
    let [l, t, r, b] = e.text_rect;
    out.gd.push(rel_guide("txL".into(), "w", l, w));
    out.gd.push(rel_guide("txT".into(), "h", t, h));
    out.gd.push(rel_guide("txR".into(), "w", r, w));
    out.gd.push(rel_guide("txB".into(), "h", b, h));
    out.rect = Some(["txL".into(), "txT".into(), "txR".into(), "txB".into()]);
    let sites = crate::handles::connection_sites(geom, wf, hf);
    for (i, s) in sites.iter().enumerate() {
        out.gd.push(rel_guide(format!("cxnX{i}"), "w", s.x, w));
        out.gd.push(rel_guide(format!("cxnY{i}"), "h", s.y, h));
    }
    if !sites.is_empty() {
        let angles: Vec<f64> = sites.iter().map(|s| s.angle).collect();
        out.extra.push(cxn_list(&angles));
    }
    out
}

fn fill_name(f: FillMode) -> Option<&'static str> {
    match f {
        FillMode::Norm => None,
        FillMode::None => Some("none"),
        FillMode::Lighten => Some("lighten"),
        FillMode::LightenLess => Some("lightenLess"),
        FillMode::Darken => Some("darken"),
        FillMode::DarkenLess => Some("darkenLess"),
    }
}

/// The geometry as an editable custom geometry: kept when it already has
/// literal paths only, baked otherwise.
pub fn editable(geom: Option<&Geometry>, w: i64, h: i64) -> CustomGeometry {
    if let Some(Geometry::Custom(c)) = geom {
        let literal = c.paths.iter().all(|p| EditPath::parse(p).is_some());
        if literal {
            return c.clone();
        }
    }
    bake(geom, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{eval_custom, eval_preset};

    const W: i64 = 2_000_000;
    const H: i64 = 1_000_000;

    /// Points along evaluated segments (end points and curve samples).
    fn samples(segs: &[Seg]) -> Vec<Pt> {
        let mut out = Vec::new();
        let mut pen = (0.0, 0.0);
        let mut start = pen;
        for s in segs {
            match *s {
                Seg::Move(x, y) => {
                    pen = (x, y);
                    start = pen;
                    out.push(pen);
                }
                Seg::Line(x, y) => {
                    pen = (x, y);
                    out.push(pen);
                }
                Seg::Quad(a, b, x, y) => {
                    for k in 1..=8 {
                        let t = k as f64 / 8.0;
                        let u = 1.0 - t;
                        out.push((
                            u * u * pen.0 + 2.0 * u * t * a + t * t * x,
                            u * u * pen.1 + 2.0 * u * t * b + t * t * y,
                        ));
                    }
                    pen = (x, y);
                }
                Seg::Cubic(a, b, c, d, x, y) => {
                    for k in 1..=8 {
                        out.push(cubic_at(pen, (a, b), (c, d), (x, y), k as f64 / 8.0));
                    }
                    pen = (x, y);
                }
                Seg::Close => pen = start,
            }
        }
        out
    }

    #[test]
    fn baking_keeps_the_outline() {
        for name in [
            "roundRect",
            "ellipse",
            "star5",
            "donut",
            "wedgeRectCallout",
            "can",
        ] {
            let geom = Geometry::Preset {
                name: name.into(),
                av: vec![],
            };
            let pre = eval_preset(name, &[], W as f64, H as f64).unwrap().unwrap();
            let baked = bake(Some(&geom), W, H);
            let got = eval_custom(&baked, &[], W as f64, H as f64).unwrap();
            assert_eq!(pre.paths.len(), got.paths.len(), "{name}");
            for (a, b) in pre.paths.iter().zip(&got.paths) {
                let (sa, sb) = (samples(&a.segs), samples(&b.segs));
                assert_eq!(sa.len(), sb.len(), "{name}");
                for (p, q) in sa.iter().zip(&sb) {
                    // Coordinates are rounded to whole EMU.
                    assert!(
                        (p.0 - q.0).abs() <= 1.0 && (p.1 - q.1).abs() <= 1.0,
                        "{name}: {p:?} {q:?}"
                    );
                }
                assert_eq!(a.fill, b.fill);
                assert_eq!(a.stroke, b.stroke);
            }
            // The baked paths are literal and editable.
            assert!(baked.paths.iter().all(|p| EditPath::parse(p).is_some()));
        }
    }

    #[test]
    fn baked_rect_has_corner_points_and_sites() {
        let g = Geometry::Preset {
            name: "rect".into(),
            av: vec![],
        };
        let c = bake(Some(&g), W, H);
        let p = EditPath::parse(&c.paths[0]).unwrap();
        assert_eq!(p.point_count(), 4);
        assert_eq!(p.segment_count(), 4);
        assert_eq!(p.point(2), Some((W as f64, H as f64)));
        assert_eq!(p.kind(0), PointKind::Corner);
        // rect cxnLst (annex): t, l, b, r midpoints.
        let sites = crate::handles::connection_sites(
            Some(&Geometry::Custom(c.clone())),
            W as f64,
            H as f64,
        );
        assert_eq!(sites.len(), 4);
        assert!(sites
            .iter()
            .any(|s| s.x == W as f64 / 2.0 && s.y == 0.0 && s.angle == 270.0));
        // Written back unchanged.
        assert_eq!(p.write(&c.paths[0]), c.paths[0]);
    }

    fn square() -> EditPath {
        EditPath {
            subs: vec![SubPath {
                pts: vec![(0.0, 0.0), (90.0, 0.0), (90.0, 90.0), (0.0, 90.0)],
                segs: vec![Span::Line; 4],
                closed: true,
            }],
        }
    }

    #[test]
    fn point_edits() {
        let mut p = square();
        // Move a point.
        assert!(p.move_point(2, 120.0, 100.0));
        assert_eq!(p.point(2), Some((120.0, 100.0)));
        // Add a point in the middle of the top edge.
        let k = p.insert_point(0, 0.5).unwrap();
        assert_eq!(k, 1);
        assert_eq!(p.point(1), Some((45.0, 0.0)));
        assert_eq!(p.point_count(), 5);
        // Delete it again: four straight sides.
        assert!(p.delete_point(1));
        assert_eq!(p.point_count(), 4);
        assert!(!p.is_curved(0));
        // Curve the closing segment: same outline, controls at ⅓ and ⅔.
        assert!(p.set_curved(3, true));
        assert_eq!(p.subs[0].segs[3], Span::Cubic((0.0, 60.0), (0.0, 30.0)));
        // Smooth the first point: the handles are collinear with (prev → next).
        assert!(p.set_kind(0, PointKind::Smooth));
        // Both handles are 30 long (⅓ of each side): read back as symmetric.
        assert_eq!(p.kind(0), PointKind::Symmetric);
        // prev = (0, 90), next = (90, 0): direction (1, −1)/√2.
        let [hin, hout] = p.handles(0);
        let (hin, hout) = (hin.unwrap(), hout.unwrap());
        assert!((hin.0 + hin.1).abs() < 1e-9 && hin.0 < 0.0, "{hin:?}");
        assert!((hout.0 + hout.1).abs() < 1e-9 && hout.0 > 0.0, "{hout:?}");
        // Symmetric: equal lengths (mean of 30 and 30 = 30).
        assert!(p.set_kind(0, PointKind::Symmetric));
        assert_eq!(p.kind(0), PointKind::Symmetric);
        let [a, b] = p.handles(0);
        assert!((len(a.unwrap()) - 30.0).abs() < 1e-9 && (len(b.unwrap()) - 30.0).abs() < 1e-9);
        // Dragging a symmetric handle mirrors the other one.
        assert!(p.move_handle(0, 1, 40.0, 0.0));
        assert_eq!(p.handles(0)[0], Some((-40.0, 0.0)));
        // Corner: handles point at the neighbours (⅓ of the way).
        assert!(p.set_kind(0, PointKind::Corner));
        assert_eq!(p.handles(0), [Some((0.0, 30.0)), Some((30.0, 0.0))]);
        assert_eq!(p.kind(0), PointKind::Corner);
        // A corner handle moves alone.
        assert!(p.move_handle(0, 1, 10.0, 5.0));
        assert_eq!(p.handles(0)[0], Some((0.0, 30.0)));
        // Straight segments have no handles.
        assert!(!p.move_handle(2, 1, 1.0, 1.0));
    }

    #[test]
    fn subdivision_keeps_the_curve() {
        let mut p = EditPath {
            subs: vec![SubPath {
                pts: vec![(0.0, 0.0), (100.0, 0.0)],
                segs: vec![Span::Cubic((0.0, 50.0), (100.0, 50.0))],
                closed: false,
            }],
        };
        let before: Vec<Pt> = (0..=10)
            .map(|k| p.seg_at(0, k as f64 / 10.0).unwrap())
            .collect();
        // Mid-point of this symmetric curve: (50, 37.5) (= ¾·50).
        let k = p.insert_point(0, 0.5).unwrap();
        assert_eq!(p.point(k), Some((50.0, 37.5)));
        // The two halves trace the original curve.
        for (i, q) in before.iter().enumerate() {
            let t = i as f64 / 10.0;
            let got = if t <= 0.5 {
                p.seg_at(0, t * 2.0).unwrap()
            } else {
                p.seg_at(1, t * 2.0 - 1.0).unwrap()
            };
            assert!((got.0 - q.0).abs() < 1e-9 && (got.1 - q.1).abs() < 1e-9);
        }
        // Nearest segment to a point near the second half.
        let (j, t, d) = p.nearest_segment(75.0, 30.0).unwrap();
        assert_eq!(j, 1);
        assert!(t > 0.0 && d < 5.0);
        // Deleting the end points of an open path stops at two anchors.
        assert!(p.delete_point(2));
        assert!(!p.delete_point(0));
    }

    #[test]
    fn closed_curve_parse_and_delete_first() {
        // Ellipse: 4 cubic arcs, closed; the closing arc returns to the start.
        let e = Geometry::Preset {
            name: "ellipse".into(),
            av: vec![],
        };
        let c = bake(Some(&e), 1000, 1000);
        let mut p = EditPath::parse(&c.paths[0]).unwrap();
        assert_eq!(p.point_count(), 4);
        assert_eq!(p.segment_count(), 4);
        assert!(p.is_curved(3));
        assert_eq!(p.kind(1), PointKind::Symmetric);
        assert!(p.delete_point(0));
        assert_eq!(p.point_count(), 3);
        assert_eq!(p.segment_count(), 3);
        assert!(!p.delete_point(0));
        // Round trip: the written path parses to the same structure.
        let w = p.write(&c.paths[0]);
        let back = EditPath::parse(&w).unwrap();
        assert_eq!(back.point_count(), 3);
        assert!(back.subs[0].closed);
    }
}

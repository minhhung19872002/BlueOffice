//! Polygon boolean operations for merging shapes (union, intersect, subtract,
//! exclusive or, fragment) on flattened DrawingML outlines (§20.1.9 paths).
//!
//! Own implementation of the edge-classification method found in computational
//! geometry textbooks (overlay of two planar subdivisions):
//!
//! 1. every edge of both operands is split at all intersection points (proper
//!    crossings, touching vertices and the ends of collinear overlaps);
//! 2. coincident pieces are kept once;
//! 3. each piece is classified by testing a point just left and just right of
//!    its midpoint against both operands (nonzero winding, like the renderer);
//!    a piece is on the result's boundary when the operation gives different
//!    answers on its two sides, and is oriented so the result lies on its left;
//! 4. the oriented pieces are linked into closed contours, taking the sharpest
//!    left turn at vertices shared by several contours.
//!
//! Results are lists of contours: outer boundaries have a positive signed area
//! (shoelace formula in the input coordinates), holes a negative one, so both
//! the nonzero and the even-odd fill rule draw them correctly.

use crate::geom::Seg;
use std::collections::{HashMap, HashSet};

/// A point.
pub type Pt = (f64, f64);
/// A closed polygon (the last point connects back to the first).
pub type Contour = Vec<Pt>;

/// Boolean operation on two regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// Points in either region.
    Union,
    /// Points in both regions.
    Intersect,
    /// Points in the first region but not in the second.
    Subtract,
    /// Points in exactly one region (combine).
    Xor,
}

impl BoolOp {
    fn apply(self, a: bool, b: bool) -> bool {
        match self {
            BoolOp::Union => a || b,
            BoolOp::Intersect => a && b,
            BoolOp::Subtract => a && !b,
            BoolOp::Xor => a != b,
        }
    }
}

/// A filled region: the union of polygons, each filled with the nonzero
/// winding rule over its contours (one polygon per DrawingML path).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Region {
    /// Polygons (each a list of contours).
    pub polys: Vec<Vec<Contour>>,
}

impl Region {
    /// Region of one polygon.
    pub fn new(contours: Vec<Contour>) -> Region {
        Region {
            polys: vec![contours],
        }
    }

    /// Is the point inside (nonzero winding in any polygon)?
    pub fn contains(&self, p: Pt) -> bool {
        self.polys.iter().any(|cs| winding(cs, p) != 0)
    }

    fn is_empty(&self) -> bool {
        self.polys.iter().all(|cs| cs.iter().all(|c| c.len() < 3))
    }
}

/// Signed area of a contour (shoelace formula).
pub fn signed_area(c: &[Pt]) -> f64 {
    let n = c.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (x0, y0) = c[i];
        let (x1, y1) = c[(i + 1) % n];
        s += x0 * y1 - x1 * y0;
    }
    s / 2.0
}

/// Net area of a result (outer contours minus holes).
pub fn area(cs: &[Contour]) -> f64 {
    cs.iter().map(|c| signed_area(c)).sum()
}

/// Winding number of contours around `p`.
pub fn winding(cs: &[Contour], p: Pt) -> i32 {
    let mut w = 0;
    for c in cs {
        let n = c.len();
        for i in 0..n {
            let a = c[i];
            let b = c[(i + 1) % n];
            let side = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 {
                if b.1 > p.1 && side > 0.0 {
                    w += 1;
                }
            } else if b.1 <= p.1 && side < 0.0 {
                w -= 1;
            }
        }
    }
    w
}

/// Flatten evaluated path segments into contours (open subpaths are closed,
/// as filling does). Curves are split until they deviate less than `tol`.
pub fn flatten(segs: &[Seg], tol: f64) -> Vec<Contour> {
    let tol = tol.max(1e-9);
    let mut out: Vec<Contour> = Vec::new();
    let mut cur: Contour = Vec::new();
    let mut pen = (0.0, 0.0);
    let flush = |cur: &mut Contour, out: &mut Vec<Contour>| {
        // Drop a repeated closing point.
        if cur.len() > 1 && dist(cur[0], cur[cur.len() - 1]) < 1e-12 {
            cur.pop();
        }
        if cur.len() >= 3 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for s in segs {
        match *s {
            Seg::Move(x, y) => {
                flush(&mut cur, &mut out);
                pen = (x, y);
                cur.push(pen);
            }
            Seg::Line(x, y) => {
                if cur.is_empty() {
                    cur.push(pen);
                }
                pen = (x, y);
                cur.push(pen);
            }
            Seg::Quad(x1, y1, x, y) => {
                if cur.is_empty() {
                    cur.push(pen);
                }
                let (p0, p1, p2) = (pen, (x1, y1), (x, y));
                // |B''| = 2·|p0 − 2p1 + p2|; error ≤ |B''|·h²/8.
                let m = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
                let n = steps((m / (4.0 * tol)).sqrt());
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
                        u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
                    ));
                }
                pen = p2;
            }
            Seg::Cubic(x1, y1, x2, y2, x, y) => {
                if cur.is_empty() {
                    cur.push(pen);
                }
                let (p0, p1, p2, p3) = (pen, (x1, y1), (x2, y2), (x, y));
                // |B''| ≤ 6·max|pᵢ − 2pᵢ₊₁ + pᵢ₊₂|; error ≤ |B''|·h²/8.
                let m = (p0.0 - 2.0 * p1.0 + p2.0)
                    .hypot(p0.1 - 2.0 * p1.1 + p2.1)
                    .max((p1.0 - 2.0 * p2.0 + p3.0).hypot(p1.1 - 2.0 * p2.1 + p3.1));
                let n = steps((3.0 * m / (4.0 * tol)).sqrt());
                for i in 1..=n {
                    cur.push(cubic_at(p0, p1, p2, p3, i as f64 / n as f64));
                }
                pen = p3;
            }
            Seg::Close => {
                if let Some(&f) = cur.first() {
                    pen = f;
                }
                flush(&mut cur, &mut out);
            }
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn steps(v: f64) -> usize {
    if v.is_finite() {
        (v.ceil() as usize).clamp(1, 512)
    } else {
        1
    }
}

/// Point of a cubic Bézier at `t`.
pub fn cubic_at(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f64) -> Pt {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn cross(a: Pt, b: Pt) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

/// Vertices merged within a tolerance (a grid hash).
struct Pool {
    pts: Vec<Pt>,
    eps: f64,
    grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Pool {
    fn new(eps: f64) -> Pool {
        Pool {
            pts: Vec::new(),
            eps,
            grid: HashMap::new(),
        }
    }
    fn cell(&self, p: Pt) -> (i64, i64) {
        (
            (p.0 / self.eps).floor() as i64,
            (p.1 / self.eps).floor() as i64,
        )
    }
    fn id(&mut self, p: Pt) -> usize {
        let (cx, cy) = self.cell(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.grid.get(&(cx + dx, cy + dy)) {
                    for &i in v {
                        if dist(self.pts[i], p) <= self.eps {
                            return i;
                        }
                    }
                }
            }
        }
        let i = self.pts.len();
        self.pts.push(p);
        self.grid.entry((cx, cy)).or_default().push(i);
        i
    }
}

/// Bounding box of all points of the regions.
fn extent(rs: &[&Region]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for r in rs {
        for cs in &r.polys {
            for c in cs {
                for &(x, y) in c {
                    let e = b.get_or_insert([x, y, x, y]);
                    e[0] = e[0].min(x);
                    e[1] = e[1].min(y);
                    e[2] = e[2].max(x);
                    e[3] = e[3].max(y);
                }
            }
        }
    }
    b
}

/// Boolean operation `a op b`: the boundary of the result as contours (outer
/// boundaries counter-clockwise in the shoelace sense, holes clockwise).
pub fn boolean(a: &Region, b: &Region, op: BoolOp) -> Vec<Contour> {
    let Some(ext) = extent(&[a, b]) else {
        return Vec::new();
    };
    let scale = (ext[2] - ext[0]).max(ext[3] - ext[1]).max(1e-9);
    let eps = scale * 1e-9;
    // 1. All edges.
    let mut edges: Vec<(Pt, Pt)> = Vec::new();
    for r in [a, b] {
        for cs in &r.polys {
            for c in cs {
                let n = c.len();
                if n < 2 {
                    continue;
                }
                for i in 0..n {
                    let (p, q) = (c[i], c[(i + 1) % n]);
                    if dist(p, q) > eps {
                        edges.push((p, q));
                    }
                }
            }
        }
    }
    // 2. Split parameters per edge.
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; edges.len()];
    let bbox = |e: &(Pt, Pt)| {
        [
            e.0 .0.min(e.1 .0),
            e.0 .1.min(e.1 .1),
            e.0 .0.max(e.1 .0),
            e.0 .1.max(e.1 .1),
        ]
    };
    let boxes: Vec<[f64; 4]> = edges.iter().map(bbox).collect();
    // Sweep over x to skip far-apart pairs.
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by(|&i, &j| boxes[i][0].total_cmp(&boxes[j][0]));
    for (k, &i) in order.iter().enumerate() {
        for &j in &order[k + 1..] {
            if boxes[j][0] > boxes[i][2] + eps {
                break;
            }
            if boxes[j][1] > boxes[i][3] + eps || boxes[i][1] > boxes[j][3] + eps {
                continue;
            }
            intersect(&edges[i], &edges[j], eps, &mut splits, i, j);
        }
    }
    // 3. Split into pieces between merged vertices; keep each piece once.
    let mut pool = Pool::new(eps * 100.0);
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (e, ts) in edges.iter().zip(splits.iter_mut()) {
        ts.sort_by(f64::total_cmp);
        let d = sub(e.1, e.0);
        let ids: Vec<usize> = ts
            .iter()
            .map(|&t| pool.id((e.0 .0 + d.0 * t, e.0 .1 + d.1 * t)))
            .collect();
        for w in ids.windows(2) {
            let (p, q) = (w[0], w[1]);
            if p == q {
                continue;
            }
            let key = (p.min(q), p.max(q));
            if seen.insert(key) {
                pieces.push((p, q));
            }
        }
    }
    // 4. Classify and orient.
    let delta = scale * 1e-6;
    let mut kept: Vec<(usize, usize)> = Vec::new();
    for &(p, q) in &pieces {
        let (pp, qq) = (pool.pts[p], pool.pts[q]);
        let len = dist(pp, qq);
        let m = (f64::midpoint(pp.0, qq.0), f64::midpoint(pp.1, qq.1));
        let dl = delta.min(len * 0.25);
        // Left normal of p → q.
        let nrm = (-(qq.1 - pp.1) / len * dl, (qq.0 - pp.0) / len * dl);
        let l = (m.0 + nrm.0, m.1 + nrm.1);
        let r = (m.0 - nrm.0, m.1 - nrm.1);
        let in_l = op.apply(a.contains(l), b.contains(l));
        let in_r = op.apply(a.contains(r), b.contains(r));
        match (in_l, in_r) {
            (true, false) => kept.push((p, q)),
            (false, true) => kept.push((q, p)),
            _ => {}
        }
    }
    link(&pool.pts, &kept, eps)
}

/// Record the intersection parameters of edges `i` and `j`.
fn intersect(e: &(Pt, Pt), f: &(Pt, Pt), eps: f64, splits: &mut [Vec<f64>], i: usize, j: usize) {
    let r = sub(e.1, e.0);
    let s = sub(f.1, f.0);
    let (lr, ls) = (r.0.hypot(r.1), s.0.hypot(s.1));
    let d = cross(r, s);
    let qp = sub(f.0, e.0);
    // Parameter of a point projected on an edge.
    let proj = |o: Pt, v: Pt, l: f64, p: Pt| ((p.0 - o.0) * v.0 + (p.1 - o.1) * v.1) / (l * l);
    if d.abs() > 1e-12 * lr * ls {
        let t = cross(qp, s) / d;
        let u = cross(qp, r) / d;
        let te = eps / lr;
        let ue = eps / ls;
        if t >= -te && t <= 1.0 + te && u >= -ue && u <= 1.0 + ue {
            splits[i].push(t.clamp(0.0, 1.0));
            splits[j].push(u.clamp(0.0, 1.0));
        }
        return;
    }
    // Parallel: collinear overlap when f lies on e's line.
    if (cross(qp, r) / lr).abs() > eps {
        return;
    }
    for p in [f.0, f.1] {
        let t = proj(e.0, r, lr, p);
        if t > 0.0 && t < 1.0 {
            splits[i].push(t);
        }
    }
    for p in [e.0, e.1] {
        let u = proj(f.0, s, ls, p);
        if u > 0.0 && u < 1.0 {
            splits[j].push(u);
        }
    }
}

/// Link oriented edges into closed contours.
fn link(pts: &[Pt], edges: &[(usize, usize)], eps: f64) -> Vec<Contour> {
    let mut out_of: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, &(p, _)) in edges.iter().enumerate() {
        out_of.entry(p).or_default().push(k);
    }
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let first = edges[start].0;
        let mut ring = vec![first];
        let mut cur = start;
        loop {
            let (p, q) = edges[cur];
            if q == first {
                break;
            }
            ring.push(q);
            let din = sub(pts[q], pts[p]);
            let next = out_of.get(&q).and_then(|v| {
                v.iter().copied().filter(|&k| !used[k]).max_by(|&a, &b| {
                    let ta = turn(din, sub(pts[edges[a].1], pts[q]));
                    let tb = turn(din, sub(pts[edges[b].1], pts[q]));
                    ta.total_cmp(&tb)
                })
            });
            let Some(n) = next else { break };
            used[n] = true;
            cur = n;
        }
        let ring: Contour = ring.into_iter().map(|i| pts[i]).collect();
        let ring = simplify(ring, eps);
        if ring.len() >= 3 && signed_area(&ring).abs() > eps * eps {
            out.push(ring);
        }
    }
    out
}

/// Signed turn angle from direction `a` to `b` (left = positive).
fn turn(a: Pt, b: Pt) -> f64 {
    cross(a, b).atan2(a.0 * b.0 + a.1 * b.1)
}

/// Drop vertices on a straight line between their neighbours.
fn simplify(mut c: Contour, eps: f64) -> Contour {
    let mut changed = true;
    while changed && c.len() >= 3 {
        changed = false;
        let n = c.len();
        for i in 0..n {
            let a = c[(i + n - 1) % n];
            let b = c[i];
            let d = c[(i + 1) % n];
            let ab = sub(b, a);
            let bd = sub(d, b);
            let l = dist(a, d).max(1e-300);
            if (cross(ab, bd) / l).abs() <= eps * 10.0 && ab.0 * bd.0 + ab.1 * bd.1 >= 0.0 {
                c.remove(i);
                changed = true;
                break;
            }
        }
    }
    c
}

/// Split a result into connected pieces: each outer boundary with the holes it
/// directly contains.
pub fn components(cs: &[Contour]) -> Vec<Vec<Contour>> {
    let mut outers: Vec<(usize, f64)> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        let a = signed_area(c);
        if a > 0.0 {
            outers.push((i, a));
        } else {
            holes.push(i);
        }
    }
    let mut groups: Vec<Vec<Contour>> = outers.iter().map(|&(i, _)| vec![cs[i].clone()]).collect();
    for h in holes {
        let Some(p) = inner_point(&cs[h], true) else {
            continue;
        };
        // Smallest outer boundary around the hole.
        let best = outers
            .iter()
            .enumerate()
            .filter(|(_, &(i, _))| winding(std::slice::from_ref(&cs[i]), p) != 0)
            .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
            .map(|(k, _)| k);
        if let Some(k) = best {
            groups[k].push(cs[h].clone());
        }
    }
    groups
}

/// A point just beside the first usable edge of a contour: on its left
/// (`left`) or right side.
fn inner_point(c: &[Pt], left: bool) -> Option<Pt> {
    let n = c.len();
    let ext = c
        .iter()
        .fold(0.0f64, |m, p| m.max(p.0.abs()).max(p.1.abs()));
    for i in 0..n {
        let (p, q) = (c[i], c[(i + 1) % n]);
        let len = dist(p, q);
        if len <= 0.0 {
            continue;
        }
        let d = (ext.max(1.0) * 1e-6).min(len * 0.25) * if left { 1.0 } else { -1.0 };
        let m = (f64::midpoint(p.0, q.0), f64::midpoint(p.1, q.1));
        return Some((m.0 - (q.1 - p.1) / len * d, m.1 + (q.0 - p.0) / len * d));
    }
    None
}

/// A point inside a piece (outer contour first, as from [`components`]).
pub fn sample_inside(piece: &[Contour]) -> Option<Pt> {
    let c = piece.first()?;
    // Interior lies left of a counter-clockwise outer boundary.
    inner_point(c, signed_area(c) > 0.0)
}

fn as_region(cs: Vec<Contour>) -> Region {
    Region::new(cs)
}

/// Fold an operation over several regions: union, intersect and xor combine
/// all of them; subtract removes the union of the others from the first.
pub fn combine(regions: &[Region], op: BoolOp) -> Vec<Contour> {
    let Some(first) = regions.first() else {
        return Vec::new();
    };
    // Normalise the first region into clean contours.
    let mut acc = boolean(first, &Region::default(), BoolOp::Union);
    match op {
        BoolOp::Subtract => {
            let rest = combine(&regions[1..], BoolOp::Union);
            if !rest.is_empty() {
                acc = boolean(&as_region(acc), &as_region(rest), BoolOp::Subtract);
            }
        }
        _ => {
            for r in &regions[1..] {
                acc = boolean(&as_region(acc), r, op);
            }
        }
    }
    acc
}

/// Fragment: cut the regions along each other's outlines into the pieces of
/// the overlay (every connected piece of every intersection and difference).
pub fn fragment(regions: &[Region]) -> Vec<Vec<Contour>> {
    let mut pieces: Vec<Vec<Contour>> = Vec::new();
    let mut covered: Vec<Contour> = Vec::new();
    for r in regions {
        if r.is_empty() {
            continue;
        }
        let mut next = Vec::new();
        for p in pieces {
            let pr = as_region(p);
            for o in [BoolOp::Intersect, BoolOp::Subtract] {
                let c = boolean(&pr, r, o);
                if !c.is_empty() {
                    next.push(c);
                }
            }
        }
        let fresh = if covered.is_empty() {
            boolean(r, &Region::default(), BoolOp::Union)
        } else {
            boolean(r, &as_region(covered.clone()), BoolOp::Subtract)
        };
        if !fresh.is_empty() {
            next.push(fresh);
        }
        covered = if covered.is_empty() {
            boolean(r, &Region::default(), BoolOp::Union)
        } else {
            boolean(&as_region(covered), r, BoolOp::Union)
        };
        pieces = next;
    }
    pieces.iter().flat_map(|p| components(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn square(x: f64, y: f64, s: f64) -> Contour {
        vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s)]
    }

    fn reg(c: Contour) -> Region {
        Region::new(vec![c])
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Vertex set of a result, rounded, sorted.
    fn verts(cs: &[Contour]) -> Vec<(i64, i64)> {
        let mut v: Vec<(i64, i64)> = cs
            .iter()
            .flatten()
            .map(|p| (p.0.round() as i64, p.1.round() as i64))
            .collect();
        v.sort_unstable();
        v
    }

    // Two 10×10 squares overlapping in a 5×5 corner: A = [0,10]², B = [5,15]².
    // By hand: |A| = |B| = 100, |A∩B| = 25, |A∪B| = 175, |A−B| = 75,
    // |A xor B| = 175 − 25 = 150.
    #[test]
    fn overlapping_squares() {
        let a = reg(square(0.0, 0.0, 10.0));
        let b = reg(square(5.0, 5.0, 10.0));
        let u = boolean(&a, &b, BoolOp::Union);
        assert_eq!(u.len(), 1);
        assert!(close(area(&u), 175.0, 1e-9), "{u:?}");
        // Union outline: an L-shaped octagon with the two crossing points.
        assert_eq!(
            verts(&u),
            vec![
                (0, 0),
                (0, 10),
                (5, 10),
                (5, 15),
                (10, 0),
                (10, 5),
                (15, 5),
                (15, 15)
            ]
        );
        let i = boolean(&a, &b, BoolOp::Intersect);
        assert!(close(area(&i), 25.0, 1e-9));
        assert_eq!(verts(&i), vec![(5, 5), (5, 10), (10, 5), (10, 10)]);
        let s = boolean(&a, &b, BoolOp::Subtract);
        assert!(close(area(&s), 75.0, 1e-9));
        assert_eq!(s.len(), 1);
        assert_eq!(verts(&s).len(), 6);
        let x = boolean(&a, &b, BoolOp::Xor);
        assert!(close(area(&x), 150.0, 1e-9));
        // Xor of corner-overlapping squares: two L-shapes touching at (5,10) and (10,5).
        assert_eq!(components(&x).len(), 2);
        // Fragment: A−B, A∩B, B−A.
        let f = fragment(&[a, b]);
        assert_eq!(f.len(), 3);
        let mut areas: Vec<f64> = f.iter().map(|p| area(p)).collect();
        areas.sort_by(f64::total_cmp);
        assert!(close(areas[0], 25.0, 1e-9) && close(areas[1], 75.0, 1e-9));
        assert!(close(areas[2], 75.0, 1e-9));
    }

    #[test]
    fn orientation_and_reverse_input() {
        // Clockwise input gives the same result as counter-clockwise.
        let mut cw = square(0.0, 0.0, 10.0);
        cw.reverse();
        let u = boolean(&reg(cw), &reg(square(5.0, 0.0, 10.0)), BoolOp::Union);
        assert_eq!(u.len(), 1);
        assert!(close(area(&u), 150.0, 1e-9));
        // Shared edge (collinear overlap) merges into one 15×10 rectangle.
        assert_eq!(verts(&u), vec![(0, 0), (0, 10), (15, 0), (15, 10)]);
    }

    #[test]
    fn disjoint_and_contained() {
        let a = reg(square(0.0, 0.0, 10.0));
        let far = reg(square(20.0, 0.0, 5.0));
        assert_eq!(boolean(&a, &far, BoolOp::Union).len(), 2);
        assert_eq!(boolean(&a, &far, BoolOp::Intersect), Vec::<Contour>::new());
        // A 4×4 square inside: subtract leaves a hole (area 100 − 16 = 84).
        let inner = reg(square(3.0, 3.0, 4.0));
        let s = boolean(&a, &inner, BoolOp::Subtract);
        assert_eq!(s.len(), 2);
        assert!(close(area(&s), 84.0, 1e-9));
        let comps = components(&s);
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].len(), 2, "outer + hole");
        assert!(signed_area(&comps[0][1]) < 0.0);
        // The hole is outside the result, its ring inside.
        let res = Region::new(s.clone());
        assert!(!res.contains((5.0, 5.0)) && res.contains((1.0, 1.0)));
        // Fragment of nested squares: ring + inner square.
        assert_eq!(fragment(&[a, inner]).len(), 2);
    }

    #[test]
    fn hole_input_with_nonzero_rule() {
        // A ring given as outer (ccw) + inner (cw): area 100 − 16.
        let mut hole = square(3.0, 3.0, 4.0);
        hole.reverse();
        let ring = Region::new(vec![square(0.0, 0.0, 10.0), hole]);
        assert!(!ring.contains((5.0, 5.0)));
        let n = boolean(&ring, &Region::default(), BoolOp::Union);
        assert!(close(area(&n), 84.0, 1e-9));
        // Filling the hole with a 6×6 square centred on it: 100 − 16 + 16 = 100.
        let u = boolean(&ring, &reg(square(2.0, 2.0, 6.0)), BoolOp::Union);
        assert!(close(area(&u), 100.0, 1e-9));
        assert_eq!(u.len(), 1);
        // Intersect with the 6×6: 36 − 16 = 20.
        let i = boolean(&ring, &reg(square(2.0, 2.0, 6.0)), BoolOp::Intersect);
        assert!(close(area(&i), 20.0, 1e-9));
    }

    fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Contour {
        (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / n as f64;
                (cx + r * t.cos(), cy + r * t.sin())
            })
            .collect()
    }

    #[test]
    fn circles() {
        // Two unit-radius-10 circles 10 apart: lens area = 2r²·acos(d/2r) − (d/2)·√(4r² − d²)
        // = 200·acos(0.5) − 5·√300 = 200π/3 − 50√3.
        let n = 2000;
        let a = reg(circle(0.0, 0.0, 10.0, n));
        let b = reg(circle(10.0, 0.0, 10.0, n));
        let lens = 200.0 * PI / 3.0 - 50.0 * 3f64.sqrt();
        let i = boolean(&a, &b, BoolOp::Intersect);
        assert!((area(&i) - lens).abs() / lens < 1e-3, "{}", area(&i));
        let u = boolean(&a, &b, BoolOp::Union);
        let full = 2.0 * PI * 100.0 - lens;
        assert!((area(&u) - full).abs() / full < 1e-3);
        let x = combine(&[a.clone(), b.clone()], BoolOp::Xor);
        assert!((area(&x) - (full - lens)).abs() / full < 1e-3);
        // Flattened Bézier circle (preset ellipse): area ≈ πr².
        let e = crate::geom::eval_preset("ellipse", &[], 20.0, 20.0)
            .unwrap()
            .unwrap();
        let cs = flatten(&e.paths[0].segs, 0.001);
        assert!((area(&cs).abs() - PI * 100.0).abs() / (PI * 100.0) < 1e-3);
    }

    #[test]
    fn three_way_ops() {
        let a = reg(square(0.0, 0.0, 10.0));
        let b = reg(square(5.0, 0.0, 10.0));
        let c = reg(square(0.0, 5.0, 10.0));
        // Subtract: A − (B ∪ C) = [0,5]×[0,5] → 25.
        let s = combine(&[a.clone(), b.clone(), c.clone()], BoolOp::Subtract);
        assert!(close(area(&s), 25.0, 1e-9));
        // Intersection of all three: [5,10]×[5,10] → 25.
        let i = combine(&[a.clone(), b.clone(), c.clone()], BoolOp::Intersect);
        assert!(close(area(&i), 25.0, 1e-9));
        // Union: 15×10 + 10×5 − overlaps: by inclusion–exclusion
        // 300 − (50 + 50 + 25) + 25 = 200.
        let u = combine(&[a.clone(), b.clone(), c.clone()], BoolOp::Union);
        assert!(close(area(&u), 200.0, 1e-9));
        // Fragment: the overlay has 5 faces: A only (25), A∩B only (25),
        // A∩C only (25), A∩B∩C (25), B only (50), C only (50), B∩C only (0 → absent).
        let f = fragment(&[a, b, c]);
        let total: f64 = f.iter().map(|p| area(p)).sum();
        assert!(close(total, 200.0, 1e-9));
        assert_eq!(f.len(), 6);
    }

    #[test]
    fn flatten_closes_and_keeps_lines() {
        let segs = [
            Seg::Move(0.0, 0.0),
            Seg::Line(4.0, 0.0),
            Seg::Line(4.0, 3.0),
            Seg::Close,
            Seg::Move(10.0, 10.0),
            Seg::Line(12.0, 10.0),
        ];
        let cs = flatten(&segs, 0.1);
        // The second subpath has two points only: no area, dropped.
        assert_eq!(cs.len(), 1);
        assert!(close(area(&cs), 6.0, 1e-12));
    }
}

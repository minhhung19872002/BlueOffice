//! Even-odd fills and clips (ISO 32000-1 §8.5.3.3.3, `f*` `B*` `b*` `W*`) on display lists
//! that only fill by the nonzero winding rule: subpaths are re-oriented so that each one
//! winds opposite to the subpath that encloses it. For subpaths that do not cross each other
//! (shapes with holes, islands in holes, glyph outlines drawn as paths by generators that give
//! every contour the same direction) both rules then paint the same area. A subpath that
//! crosses itself keeps its nonzero result.
use bo_render::PathSeg;

/// Largest amount of containment work (subpaths × points) before the path is left as it is.
const MAX_WORK: usize = 4_000_000;

struct Sub {
    segs: Vec<PathSeg>,
    start: (f32, f32),
    poly: Vec<(f32, f32)>,
    area: f32,
}

fn cubic(p0: (f32, f32), c1: (f32, f32), c2: (f32, f32), p: (f32, f32), out: &mut Vec<(f32, f32)>) {
    for k in 1..=8 {
        let t = k as f32 / 8.0;
        let u = 1.0 - t;
        let a = u * u * u;
        let b = 3.0 * u * u * t;
        let c = 3.0 * u * t * t;
        let d = t * t * t;
        out.push((
            a * p0.0 + b * c1.0 + c * c2.0 + d * p.0,
            a * p0.1 + b * c1.1 + c * c2.1 + d * p.1,
        ));
    }
}

fn split(segs: &[PathSeg]) -> Vec<Sub> {
    let mut subs: Vec<Sub> = Vec::new();
    let mut cur = (0.0, 0.0);
    for s in segs {
        match *s {
            PathSeg::M(x, y) => {
                subs.push(Sub {
                    segs: Vec::new(),
                    start: (x, y),
                    poly: vec![(x, y)],
                    area: 0.0,
                });
                cur = (x, y);
            }
            seg => {
                if subs.is_empty() {
                    subs.push(Sub {
                        segs: Vec::new(),
                        start: cur,
                        poly: vec![cur],
                        area: 0.0,
                    });
                }
                let Some(sub) = subs.last_mut() else {
                    continue;
                };
                match seg {
                    PathSeg::L(x, y) => {
                        sub.poly.push((x, y));
                        cur = (x, y);
                    }
                    PathSeg::Q(x1, y1, x, y) => {
                        // As a cubic for flattening.
                        let c1 = (
                            cur.0 + 2.0 / 3.0 * (x1 - cur.0),
                            cur.1 + 2.0 / 3.0 * (y1 - cur.1),
                        );
                        let c2 = (x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
                        cubic(cur, c1, c2, (x, y), &mut sub.poly);
                        cur = (x, y);
                    }
                    PathSeg::C(x1, y1, x2, y2, x, y) => {
                        cubic(cur, (x1, y1), (x2, y2), (x, y), &mut sub.poly);
                        cur = (x, y);
                    }
                    PathSeg::Z => cur = sub.start,
                    PathSeg::M(..) => {}
                }
                sub.segs.push(seg);
            }
        }
    }
    for s in &mut subs {
        let p = &s.poly;
        let mut a = 0.0;
        for i in 0..p.len() {
            let (x0, y0) = p[i];
            let (x1, y1) = p[(i + 1) % p.len()];
            a += x0 * y1 - x1 * y0;
        }
        s.area = a / 2.0;
    }
    subs
}

/// Even-odd point in polygon test.
fn inside(poly: &[(f32, f32)], (px, py): (f32, f32)) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[(i + n - 1) % n];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            c = !c;
        }
    }
    c
}

/// The subpath drawn backwards (same shape, opposite direction).
fn reversed(sub: &Sub) -> Vec<PathSeg> {
    // Points where each segment starts.
    let mut starts = Vec::with_capacity(sub.segs.len());
    let mut cur = sub.start;
    for s in &sub.segs {
        starts.push(cur);
        cur = match *s {
            PathSeg::M(x, y)
            | PathSeg::L(x, y)
            | PathSeg::Q(_, _, x, y)
            | PathSeg::C(_, _, _, _, x, y) => (x, y),
            PathSeg::Z => sub.start,
        };
    }
    let closed = matches!(sub.segs.last(), Some(PathSeg::Z));
    // A closing `Z` becomes a line walked the other way, so the reversed path starts at the
    // point where the original ended.
    let mut out = vec![PathSeg::M(cur.0, cur.1)];
    for (s, from) in sub.segs.iter().zip(starts.iter()).rev() {
        match *s {
            PathSeg::L(..) => out.push(PathSeg::L(from.0, from.1)),
            PathSeg::Q(x1, y1, _, _) => out.push(PathSeg::Q(x1, y1, from.0, from.1)),
            PathSeg::C(x1, y1, x2, y2, _, _) => {
                out.push(PathSeg::C(x2, y2, x1, y1, from.0, from.1));
            }
            // The closing line, walked the other way.
            PathSeg::Z => out.push(PathSeg::L(from.0, from.1)),
            PathSeg::M(..) => {}
        }
    }
    if closed {
        out.push(PathSeg::Z);
    }
    out
}

/// A path that fills by the nonzero rule what `segs` fills by the even-odd rule (see the
/// module notes for the limits).
pub(crate) fn nonzero_equivalent(segs: &[PathSeg]) -> Vec<PathSeg> {
    let subs = split(segs);
    let points: usize = subs.iter().map(|s| s.poly.len()).sum();
    if subs.is_empty() || subs.len().saturating_mul(points) > MAX_WORK {
        return segs.to_vec();
    }
    // Subpaths that cross themselves or each other (a star drawn in one stroke): the even-odd
    // area's own outline, flattened.
    if points <= MAX_CROSSING_EDGES {
        let edges = edges_of(&subs);
        if crosses(&edges) {
            return boundary(&edges).unwrap_or_else(|| segs.to_vec());
        }
    }
    if subs.len() < 2 {
        return segs.to_vec();
    }
    let mut out = Vec::with_capacity(segs.len() + subs.len());
    for (i, s) in subs.iter().enumerate() {
        if s.area.abs() < 1e-9 {
            out.push(PathSeg::M(s.start.0, s.start.1));
            out.extend_from_slice(&s.segs);
            continue;
        }
        // Nesting depth: larger subpaths that enclose this one's first corner.
        let depth = subs
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && o.area.abs() > s.area.abs() && inside(&o.poly, s.poly[0]))
            .count();
        let want_positive = depth % 2 == 0;
        if (s.area > 0.0) == want_positive {
            out.push(PathSeg::M(s.start.0, s.start.1));
            out.extend_from_slice(&s.segs);
        } else {
            out.extend(reversed(s));
        }
    }
    out
}

/// Most flattened edges for the crossing test (quadratic work).
const MAX_CROSSING_EDGES: usize = 3000;

type Pt = (f64, f64);

/// Closed flattened edges of every subpath (zero-length ones left out).
fn edges_of(subs: &[Sub]) -> Vec<(Pt, Pt)> {
    let mut out = Vec::new();
    for s in subs {
        let p = &s.poly;
        for i in 0..p.len() {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            if a != b {
                out.push((
                    (f64::from(a.0), f64::from(a.1)),
                    (f64::from(b.0), f64::from(b.1)),
                ));
            }
        }
    }
    out
}

/// Parameters along `a→b` and `c→d` where the two cross inside both (not at their ends).
fn cross_at(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<(f64, f64)> {
    let r = (b.0 - a.0, b.1 - a.1);
    let s = (d.0 - c.0, d.1 - c.1);
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-12 {
        return None;
    }
    let q = (c.0 - a.0, c.1 - a.1);
    let t = (q.0 * s.1 - q.1 * s.0) / den;
    let u = (q.0 * r.1 - q.1 * r.0) / den;
    let e = 1e-7;
    (t > e && t < 1.0 - e && u > e && u < 1.0 - e).then_some((t, u))
}

fn crosses(edges: &[(Pt, Pt)]) -> bool {
    (0..edges.len()).any(|i| {
        (i + 1..edges.len()).any(|j| {
            let (a, b) = edges[i];
            let (c, d) = edges[j];
            cross_at(a, b, c, d).is_some()
        })
    })
}

/// Even-odd point in the area of closed edges.
fn inside_edges(edges: &[(Pt, Pt)], p: Pt) -> bool {
    let mut c = false;
    for &((x0, y0), (x1, y1)) in edges {
        if (y0 > p.1) != (y1 > p.1) && p.0 < (x1 - x0) * (p.1 - y0) / (y1 - y0) + x0 {
            c = !c;
        }
    }
    c
}

/// The outline of the even-odd area of `edges`: edges cut where they cross, those with the
/// area on one side only kept and turned so the area lies on the same side of each, joined
/// into closed loops. Filled by the nonzero rule this paints exactly the even-odd area.
fn boundary(edges: &[(Pt, Pt)]) -> Option<Vec<PathSeg>> {
    use std::collections::{HashMap, HashSet};
    let mut cuts: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; edges.len()];
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            let (a, b) = edges[i];
            let (c, d) = edges[j];
            if let Some((t, u)) = cross_at(a, b, c, d) {
                cuts[i].push(t);
                cuts[j].push(u);
            }
        }
    }
    let key = |p: Pt| ((p.0 * 1024.0).round() as i64, (p.1 * 1024.0).round() as i64);
    let mut ids: HashMap<(i64, i64), usize> = HashMap::new();
    let mut verts: Vec<Pt> = Vec::new();
    let mut vid = |p: Pt, verts: &mut Vec<Pt>| {
        *ids.entry(key(p)).or_insert_with(|| {
            verts.push(p);
            verts.len() - 1
        })
    };
    let mut kept: Vec<(usize, usize)> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        let ts = &mut cuts[i];
        ts.sort_by(f64::total_cmp);
        for w in ts.windows(2) {
            let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let (p, q) = (at(w[0]), at(w[1]));
            let (va, vb) = (vid(p, &mut verts), vid(q, &mut verts));
            if va == vb || !seen.insert((va.min(vb), va.max(vb))) {
                continue;
            }
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let len = dx.hypot(dy);
            let eps = (len * 1e-3).clamp(1e-5, 1e-2);
            let m = (f64::midpoint(p.0, q.0), f64::midpoint(p.1, q.1));
            let (nx, ny) = (-dy / len * eps, dx / len * eps);
            let left = inside_edges(edges, (m.0 + nx, m.1 + ny));
            let right = inside_edges(edges, (m.0 - nx, m.1 - ny));
            match (left, right) {
                (true, false) => kept.push((va, vb)),
                (false, true) => kept.push((vb, va)),
                _ => {}
            }
        }
    }
    if kept.is_empty() {
        return None;
    }
    let mut from: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, &(a, _)) in kept.iter().enumerate() {
        from.entry(a).or_default().push(k);
    }
    let mut used = vec![false; kept.len()];
    let mut out = Vec::new();
    let f = |v: f64| v as f32;
    for start in 0..kept.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut at) = kept[start];
        out.push(PathSeg::M(f(verts[first].0), f(verts[first].1)));
        out.push(PathSeg::L(f(verts[at].0), f(verts[at].1)));
        while at != first {
            let Some(next) = from
                .get(&at)
                .and_then(|list| list.iter().copied().find(|&k| !used[k]))
            else {
                break;
            };
            used[next] = true;
            at = kept[next].1;
            out.push(PathSeg::L(f(verts[at].0), f(verts[at].1)));
        }
        out.push(PathSeg::Z);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_star_in_one_stroke_leaves_its_centre_empty() {
        // Pentagram: nonzero fills the centre, even-odd does not.
        let pts: Vec<(f32, f32)> = (0..5)
            .map(|k| {
                let a = std::f32::consts::FRAC_PI_2 + k as f32 * 4.0 * std::f32::consts::PI / 5.0;
                (100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let mut star = vec![PathSeg::M(pts[0].0, pts[0].1)];
        star.extend(pts[1..].iter().map(|p| PathSeg::L(p.0, p.1)));
        star.push(PathSeg::Z);
        assert_ne!(winding(&star, (0.0, 0.0)), 0);
        let fixed = nonzero_equivalent(&star);
        assert_eq!(winding(&fixed, (0.0, 0.0)), 0, "{fixed:?}");
        // The points of the star stay filled.
        assert_ne!(winding(&fixed, (0.0, 80.0)), 0);
        assert_ne!(winding(&fixed, (-70.0, 25.0)), 0);
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<PathSeg> {
        vec![
            PathSeg::M(x, y),
            PathSeg::L(x + w, y),
            PathSeg::L(x + w, y + h),
            PathSeg::L(x, y + h),
            PathSeg::Z,
        ]
    }

    fn winding(segs: &[PathSeg], p: (f32, f32)) -> i32 {
        let mut w = 0;
        for s in split(segs) {
            let poly = &s.poly;
            for i in 0..poly.len() {
                let (x0, y0) = poly[i];
                let (x1, y1) = poly[(i + 1) % poly.len()];
                if y0 <= p.1 && y1 > p.1 || y1 <= p.1 && y0 > p.1 {
                    let x = x0 + (p.1 - y0) / (y1 - y0) * (x1 - x0);
                    if x > p.0 {
                        w += if y1 > y0 { 1 } else { -1 };
                    }
                }
            }
        }
        w
    }

    #[test]
    fn nested_subpaths_wind_alternately() {
        // Three squares drawn the same way round: even-odd paints the outer ring and the
        // innermost square.
        let mut segs = rect(0.0, 0.0, 100.0, 100.0);
        segs.extend(rect(20.0, 20.0, 60.0, 60.0));
        segs.extend(rect(40.0, 40.0, 20.0, 20.0));
        assert_ne!(winding(&segs, (30.0, 30.0)), 0, "nonzero fills the hole");
        let fixed = nonzero_equivalent(&segs);
        assert_ne!(winding(&fixed, (10.0, 10.0)), 0);
        assert_eq!(winding(&fixed, (30.0, 30.0)), 0);
        assert_ne!(winding(&fixed, (50.0, 50.0)), 0);
        // Separate shapes stay filled.
        let mut two = rect(0.0, 0.0, 10.0, 10.0);
        two.extend(rect(20.0, 0.0, 10.0, 10.0));
        let fixed = nonzero_equivalent(&two);
        assert_ne!(winding(&fixed, (5.0, 5.0)), 0);
        assert_ne!(winding(&fixed, (25.0, 5.0)), 0);
    }

    #[test]
    fn reversing_keeps_curves_and_closing_lines() {
        let segs = vec![
            PathSeg::M(0.0, 0.0),
            PathSeg::C(0.0, 10.0, 10.0, 10.0, 10.0, 0.0),
            PathSeg::L(5.0, -5.0),
            PathSeg::Z,
        ];
        let sub = split(&segs).remove(0);
        let r = reversed(&sub);
        assert_eq!(r[0], PathSeg::M(0.0, 0.0));
        assert_eq!(r[1], PathSeg::L(5.0, -5.0));
        assert_eq!(r[2], PathSeg::L(10.0, 0.0));
        assert_eq!(r[3], PathSeg::C(10.0, 10.0, 0.0, 10.0, 0.0, 0.0));
        assert_eq!(r[4], PathSeg::Z);
        let back = split(&r).remove(0);
        assert!((back.area + sub.area).abs() < 1e-3);
    }
}

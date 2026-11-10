//! Ink → shape recogniser (CORE-INK-002): deterministic geometry, no trained
//! model. Every stroke is resampled to equal arc-length spacing and smoothed,
//! then judged open or closed:
//!
//! - open: a straight line when its chord is close to its path length and no
//!   sample strays far from the fitted line;
//! - closed: a least-squares conic fit (ellipse / circle) and a polygon fit
//!   (Douglas–Peucker on the loop, nearly straight or crowded corners merged)
//!   are compared by their mean distance to the ink; a polygon is classified
//!   by its corner count, turning and angles (triangle, rectangle / square,
//!   diamond, pentagon, hexagon, star — outline or pentagram).
//!
//! An arrow is a line plus one or two short strokes near one of its ends,
//! pointing back along it. Thresholds are BlueSlide's own choices; a stroke
//! that clears none of them stays ink. Coordinates are slide points (y down);
//! rotations are degrees clockwise, as DrawingML `xfrm@rot` (ECMA-376 Part 1,
//! §20.1.7.6).

use serde::Serialize;

/// A point in slide points.
pub type Pt = (f64, f64);

/// Smallest recognised stroke: bounding-box diagonal in points.
pub const MIN_SIZE: f64 = 8.0;
/// Samples per resampled stroke.
const SAMPLES: usize = 96;
/// Line: chord / path length at least this.
const LINE_STRAIGHTNESS: f64 = 0.93;
/// Line: largest distance from the fitted line, relative to its length.
const LINE_DEVIATION: f64 = 0.075;
/// Closed: the end comes back within this share of the box diagonal.
const CLOSE_GAP: f64 = 0.2;
/// Polygon: Douglas–Peucker tolerance relative to the box diagonal.
const DP_TOLERANCE: f64 = 0.04;
/// Polygon: corners turning less than this (degrees) are not corners.
const MERGE_TURN: f64 = 28.0;
/// Polygon: edges shorter than this share of the perimeter are merged.
const MERGE_EDGE: f64 = 0.06;
/// Largest mean distance of the ink from a polygon, relative to half the diagonal.
const POLY_MAX_ERR: f64 = 0.05;
/// Largest mean distance of the ink from an ellipse, relative to half the diagonal.
const ELLIPSE_MAX_ERR: f64 = 0.06;
/// Rotations within this many degrees of an axis snap to it.
const SNAP_DEG: f64 = 7.0;
/// Corner angles within this many degrees of 90° count as right angles.
const RIGHT_TOL: f64 = 20.0;

/// What a stroke (or a group of strokes) was recognised as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    /// A straight line.
    Line,
    /// A line with an arrowhead at one or both ends.
    Arrow,
    /// A rectangle (any aspect).
    Rectangle,
    /// A rectangle with equal sides.
    Square,
    /// A triangle (apex anywhere over its base).
    Triangle,
    /// An ellipse.
    Ellipse,
    /// An ellipse with equal axes.
    Circle,
    /// A rhombus standing on a corner.
    Diamond,
    /// A regular-looking pentagon.
    Pentagon,
    /// A regular-looking hexagon.
    Hexagon,
    /// A five-pointed star (outline or pentagram).
    Star,
}

impl ShapeKind {
    /// The DrawingML preset geometry (ECMA-376 Part 1, §20.1.10.56 ST_ShapeType).
    #[must_use]
    pub fn preset(self) -> &'static str {
        match self {
            ShapeKind::Line | ShapeKind::Arrow => "line",
            ShapeKind::Rectangle | ShapeKind::Square => "rect",
            ShapeKind::Triangle => "triangle",
            ShapeKind::Ellipse | ShapeKind::Circle => "ellipse",
            ShapeKind::Diamond => "diamond",
            ShapeKind::Pentagon => "pentagon",
            ShapeKind::Hexagon => "hexagon",
            ShapeKind::Star => "star5",
        }
    }
}

/// Where the recognised shape goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fit {
    /// A line from `from` to `to`, with arrowheads at either end.
    Line {
        /// Start.
        from: Pt,
        /// End.
        to: Pt,
        /// Arrowhead at the start.
        head_from: bool,
        /// Arrowhead at the end.
        head_to: bool,
    },
    /// A preset in a rotated box: centre, unrotated size, rotation (degrees
    /// clockwise) and, for a triangle, the apex position along the top edge
    /// (0..=1, the preset's `adj`).
    Frame {
        /// Centre of the box.
        center: Pt,
        /// Unrotated width.
        w: f64,
        /// Unrotated height.
        h: f64,
        /// Rotation, degrees clockwise.
        rot: f64,
        /// Triangle apex position (0..=1).
        adj: Option<f64>,
    },
}

/// A recognised shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recognized {
    /// The shape.
    pub kind: ShapeKind,
    /// Its geometry.
    pub fit: Fit,
    /// 0..=1: how far inside its acceptance threshold the fit is.
    pub confidence: f64,
}

/// One recognised group of strokes (indices into the input).
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Stroke indices (ascending).
    pub strokes: Vec<usize>,
    /// What they make.
    pub rec: Recognized,
}

// ------------------------------------------------------------------ helpers

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn path_len(p: &[Pt]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn bbox(p: &[Pt]) -> (f64, f64, f64, f64) {
    p.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
    )
}

fn diag(p: &[Pt]) -> f64 {
    let (x0, y0, x1, y1) = bbox(p);
    if p.is_empty() {
        0.0
    } else {
        (x1 - x0).hypot(y1 - y0)
    }
}

fn centroid(p: &[Pt]) -> Pt {
    let n = p.len().max(1) as f64;
    let (sx, sy) = p.iter().fold((0.0, 0.0), |(a, b), q| (a + q.0, b + q.1));
    (sx / n, sy / n)
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    dist((a.0 + t * dx, a.1 + t * dy), p)
}

fn deg(rad: f64) -> f64 {
    rad.to_degrees()
}

/// `v` wrapped into `(-period/2, period/2]`.
fn wrap(v: f64, period: f64) -> f64 {
    let mut r = v.rem_euclid(period);
    if r > period / 2.0 {
        r -= period;
    }
    r
}

/// Mean of angles (degrees) with the given period.
fn circular_mean(angles: impl Iterator<Item = f64>, period: f64) -> f64 {
    let k = std::f64::consts::TAU / period;
    let (s, c) = angles.fold((0.0, 0.0), |(s, c), a| {
        (s + (a * k).sin(), c + (a * k).cos())
    });
    wrap((s.atan2(c)) / k, period)
}

/// `a` snapped to the nearest multiple of `step` when within [`SNAP_DEG`].
fn snap(a: f64, step: f64) -> f64 {
    let k = (a / step).round() * step;
    if (a - k).abs() <= SNAP_DEG {
        k + 0.0
    } else {
        a
    }
}

fn rotate(p: Pt, deg: f64) -> Pt {
    let (s, c) = deg.to_radians().sin_cos();
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

/// Box of `pts` in a frame rotated by `rot`: centre (slide), width, height.
fn frame_of(pts: &[Pt], rot: f64) -> (Pt, f64, f64) {
    let local: Vec<Pt> = pts.iter().map(|&p| rotate(p, -rot)).collect();
    let (x0, y0, x1, y1) = bbox(&local);
    let c = rotate((x0.midpoint(x1), y0.midpoint(y1)), rot);
    (c, x1 - x0, y1 - y0)
}

fn frame(kind: ShapeKind, pts: &[Pt], rot: f64, confidence: f64) -> Option<Recognized> {
    let (center, w, h) = frame_of(pts, rot);
    (w >= 1.0 && h >= 1.0).then_some(Recognized {
        kind,
        fit: Fit::Frame {
            center,
            w,
            h,
            rot,
            adj: None,
        },
        confidence,
    })
}

/// Finite points without consecutive duplicates.
fn clean(raw: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(raw.len());
    for &p in raw {
        if !(p.0.is_finite() && p.1.is_finite()) {
            continue;
        }
        if out.last().is_some_and(|&q| dist(p, q) < 1e-9) {
            continue;
        }
        out.push(p);
    }
    out
}

/// `n` points at equal arc-length spacing along `p` (both ends kept).
#[must_use]
pub fn resample(p: &[Pt], n: usize) -> Vec<Pt> {
    let total = path_len(p);
    let (Some(&first), Some(&last)) = (p.first(), p.last()) else {
        return Vec::new();
    };
    if n < 2 || total <= 0.0 {
        return vec![first; n.max(1)];
    }
    let step = total / (n - 1) as f64;
    let mut out = vec![first];
    let mut acc = 0.0;
    let mut prev = first;
    let mut i = 1;
    while i < p.len() && out.len() < n - 1 {
        let cur = p[i];
        let d = dist(prev, cur);
        if d > 0.0 && acc + d >= step {
            let t = (step - acc) / d;
            let q = (prev.0 + t * (cur.0 - prev.0), prev.1 + t * (cur.1 - prev.1));
            out.push(q);
            prev = q;
            acc = 0.0;
        } else {
            acc += d;
            prev = cur;
            i += 1;
        }
    }
    while out.len() < n - 1 {
        out.push(last);
    }
    out.push(last);
    out
}

/// 1-2-1 smoothing; the ends of an open stroke stay put.
fn smooth(p: &[Pt], closed: bool) -> Vec<Pt> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    (0..n)
        .map(|i| {
            if !closed && (i == 0 || i == n - 1) {
                return p[i];
            }
            let a = p[(i + n - 1) % n];
            let b = p[i];
            let c = p[(i + 1) % n];
            ((a.0 + 2.0 * b.0 + c.0) / 4.0, (a.1 + 2.0 * b.1 + c.1) / 4.0)
        })
        .collect()
}

// --------------------------------------------------------------------- lines

/// Total least-squares direction of `p` (unit vector).
fn principal_dir(p: &[Pt]) -> Pt {
    let c = centroid(p);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for q in p {
        let (dx, dy) = (q.0 - c.0, q.1 - c.1);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let a = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    (a.cos(), a.sin())
}

/// A straight open stroke → its end points (snapped to 45° steps nearby).
fn line_of(r: &[Pt]) -> Option<(Pt, Pt, f64)> {
    let (&first, &last) = (r.first()?, r.last()?);
    let chord = dist(first, last);
    let len = path_len(r);
    if chord < MIN_SIZE || len <= 0.0 {
        return None;
    }
    let straight = chord / len;
    let c = centroid(r);
    let mut d = principal_dir(r);
    if (last.0 - first.0) * d.0 + (last.1 - first.1) * d.1 < 0.0 {
        d = (-d.0, -d.1);
    }
    let dev = r
        .iter()
        .map(|q| ((q.0 - c.0) * d.1 - (q.1 - c.1) * d.0).abs())
        .fold(0.0, f64::max);
    if straight < LINE_STRAIGHTNESS || dev > LINE_DEVIATION * chord {
        return None;
    }
    let t0 = (first.0 - c.0) * d.0 + (first.1 - c.1) * d.1;
    let t1 = (last.0 - c.0) * d.0 + (last.1 - c.1) * d.1;
    let a = deg(d.1.atan2(d.0));
    let s = snap(a, 45.0);
    let d = if (s - a).abs() > 0.0 {
        let (sn, cs) = s.to_radians().sin_cos();
        (cs, sn)
    } else {
        d
    };
    let from = (c.0 + t0 * d.0, c.1 + t0 * d.1);
    let to = (c.0 + t1 * d.0, c.1 + t1 * d.1);
    let confidence = ((straight - LINE_STRAIGHTNESS) / (1.0 - LINE_STRAIGHTNESS))
        .min(1.0 - dev / (LINE_DEVIATION * chord))
        .clamp(0.0, 1.0);
    Some((from, to, confidence))
}

// ------------------------------------------------------------------- closing

/// The loop of a closed stroke (overshoot trimmed), or `None` when open.
fn closed_loop(r: &[Pt]) -> Option<Vec<Pt>> {
    let n = r.len();
    let d = diag(r);
    let len = path_len(r);
    if n < 8 || d < MIN_SIZE || len < 1.8 * d {
        return None;
    }
    let first = r[0];
    let (k, gap) =
        (n / 2..n)
            .map(|k| (k, dist(r[k], first)))
            .fold(
                (n - 1, f64::INFINITY),
                |best, c| if c.1 < best.1 { c } else { best },
            );
    if gap > CLOSE_GAP * d || path_len(&r[k..]) > 0.3 * len {
        return None;
    }
    let lp = resample(&r[..=k], SAMPLES + 1);
    // Drop the closing duplicate of the first point.
    Some(smooth(&lp[..lp.len() - 1], true))
}

// ------------------------------------------------------------------- ellipse

/// An ellipse: centre, semi-axes (`a` ≥ `b`) and the major axis angle (degrees).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ellipse {
    c: Pt,
    a: f64,
    b: f64,
    angle: f64,
}

/// Solve `m · x = v` (Gaussian elimination with partial pivoting).
fn solve<const N: usize>(mut m: [[f64; N]; N], mut v: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let piv = (col..N).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[piv][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, piv);
        v.swap(col, piv);
        for row in col + 1..N {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (k, cell) in m[row].iter_mut().enumerate().skip(col) {
                *cell -= f * pivot_row[k];
            }
            v[row] -= f * v[col];
        }
    }
    let mut x = [0.0; N];
    for i in (0..N).rev() {
        let s: f64 = (i + 1..N).map(|k| m[i][k] * x[k]).sum();
        x[i] = (v[i] - s) / m[i][i];
    }
    Some(x)
}

/// Least-squares conic `A u² + B uv + C v² + D u + E v = 1` on centred,
/// normalised coordinates, read back as an ellipse.
fn fit_ellipse(p: &[Pt]) -> Option<Ellipse> {
    let c0 = centroid(p);
    let s = (p.iter().map(|q| dist(*q, c0).powi(2)).sum::<f64>() / p.len().max(1) as f64).sqrt();
    if s <= 0.0 {
        return None;
    }
    let mut m = [[0.0; 5]; 5];
    let mut v = [0.0; 5];
    for q in p {
        let (u, w) = ((q.0 - c0.0) / s, (q.1 - c0.1) / s);
        let f = [u * u, u * w, w * w, u, w];
        for i in 0..5 {
            for j in 0..5 {
                m[i][j] += f[i] * f[j];
            }
            v[i] += f[i];
        }
    }
    let [a, b, c, d, e] = solve(m, v)?;
    if 4.0 * a * c - b * b <= 1e-9 {
        return None;
    }
    let [u0, w0] = solve([[2.0 * a, b], [b, 2.0 * c]], [-d, -e])?;
    let k = 1.0 - (a * u0 * u0 + b * u0 * w0 + c * w0 * w0 + d * u0 + e * w0);
    if k <= 0.0 {
        return None;
    }
    let mean = a.midpoint(c);
    let root = (((a - c) / 2.0).powi(2) + (b / 2.0).powi(2)).sqrt();
    let (lmin, lmax) = (mean - root, mean + root);
    if lmin <= 0.0 {
        return None;
    }
    // Along 0.5·atan2(B, A − C) the form is largest: the minor axis.
    let minor = 0.5 * b.atan2(a - c);
    Some(Ellipse {
        c: (c0.0 + u0 * s, c0.1 + w0 * s),
        a: (k / lmin).sqrt() * s,
        b: (k / lmax).sqrt() * s,
        angle: wrap(deg(minor) + 90.0, 180.0),
    })
}

/// Mean (approximate) distance of `p` from the ellipse.
fn ellipse_error(e: &Ellipse, p: &[Pt]) -> f64 {
    let sum: f64 = p
        .iter()
        .map(|q| {
            let l = rotate((q.0 - e.c.0, q.1 - e.c.1), -e.angle);
            let r = ((l.0 / e.a).powi(2) + (l.1 / e.b).powi(2)).sqrt();
            let len = l.0.hypot(l.1);
            if r > 1e-9 {
                (1.0 - 1.0 / r).abs() * len
            } else {
                e.b
            }
        })
        .sum();
    sum / p.len().max(1) as f64
}

fn ellipse_rec(e: &Ellipse, confidence: f64) -> Option<Recognized> {
    if e.b / e.a < 0.08 {
        return None;
    }
    let (kind, w, h, rot) = if e.b / e.a >= 0.85 {
        let d = e.a + e.b;
        (ShapeKind::Circle, d, d, 0.0)
    } else {
        let a = snap(e.angle, 90.0);
        if a.abs() == 90.0 {
            (ShapeKind::Ellipse, 2.0 * e.b, 2.0 * e.a, 0.0)
        } else {
            (ShapeKind::Ellipse, 2.0 * e.a, 2.0 * e.b, a)
        }
    };
    Some(Recognized {
        kind,
        fit: Fit::Frame {
            center: e.c,
            w,
            h,
            rot,
            adj: None,
        },
        confidence,
    })
}

// ------------------------------------------------------------------- polygon

/// Douglas–Peucker on an open polyline: indices kept (ends included).
fn douglas_peucker(p: &[Pt], eps: f64) -> Vec<usize> {
    let n = p.len();
    if n < 3 {
        return (0..n).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (i, d) = (a + 1..b)
            .map(|i| (i, seg_dist(p[i], p[a], p[b])))
            .fold((a, -1.0), |m, c| if c.1 > m.1 { c } else { m });
        if d > eps {
            keep[i] = true;
            stack.push((a, i));
            stack.push((i, b));
        }
    }
    (0..n).filter(|&i| keep[i]).collect()
}

/// Turn at each vertex of a closed polygon (degrees, signed; + = clockwise on screen).
fn turns(v: &[Pt]) -> Vec<f64> {
    let n = v.len();
    (0..n)
        .map(|i| {
            let a = v[(i + n - 1) % n];
            let b = v[i];
            let c = v[(i + 1) % n];
            let a1 = (b.1 - a.1).atan2(b.0 - a.0);
            let a2 = (c.1 - b.1).atan2(c.0 - b.0);
            wrap(deg(a2 - a1), 360.0)
        })
        .collect()
}

fn perimeter(v: &[Pt]) -> f64 {
    let n = v.len();
    (0..n).map(|i| dist(v[i], v[(i + 1) % n])).sum()
}

/// Corners of a closed loop.
fn polygon(lp: &[Pt]) -> Vec<Pt> {
    let n = lp.len();
    if n < 4 {
        return lp.to_vec();
    }
    let c = centroid(lp);
    let s = (0..n)
        .max_by(|&i, &j| dist(lp[i], c).total_cmp(&dist(lp[j], c)))
        .unwrap_or(0);
    let ring: Vec<Pt> = (0..=n).map(|i| lp[(s + i) % n]).collect();
    let idx = douglas_peucker(&ring, DP_TOLERANCE * diag(lp));
    let mut v: Vec<Pt> = idx[..idx.len() - 1].iter().map(|&i| ring[i]).collect();
    loop {
        if v.len() <= 3 {
            break;
        }
        let t = turns(&v);
        let per = perimeter(&v);
        let k = v.len();
        // The flattest corner, or the flatter end of the shortest edge.
        let (flat, ft) = t
            .iter()
            .enumerate()
            .map(|(i, a)| (i, a.abs()))
            .fold((0, f64::INFINITY), |m, c| if c.1 < m.1 { c } else { m });
        if ft < MERGE_TURN {
            v.remove(flat);
            continue;
        }
        let (short, sl) = (0..k)
            .map(|i| (i, dist(v[i], v[(i + 1) % k])))
            .fold((0, f64::INFINITY), |m, c| if c.1 < m.1 { c } else { m });
        if sl < MERGE_EDGE * per {
            let j = (short + 1) % k;
            // Two corners close together: one corner at their middle.
            let m = (v[short].0.midpoint(v[j].0), v[short].1.midpoint(v[j].1));
            v[short] = m;
            v.remove(j);
            continue;
        }
        break;
    }
    v
}

/// Where the lines through `p1` along `d1` and `p2` along `d2` cross.
fn intersect(p1: Pt, d1: Pt, p2: Pt, d2: Pt) -> Option<Pt> {
    let den = d1.0 * d2.1 - d1.1 * d2.0;
    if den.abs() < 1e-6 {
        return None;
    }
    let t = ((p2.0 - p1.0) * d2.1 - (p2.1 - p1.1) * d2.0) / den;
    Some((p1.0 + t * d1.0, p1.1 + t * d1.1))
}

/// Corners of a convex polygon moved to where its edges, fitted to the ink
/// between the corners, meet (smoothing and hand-drawn rounding cut corners).
fn refine(v: &[Pt], lp: &[Pt]) -> Vec<Pt> {
    let k = v.len();
    let reach = 0.15 * diag(lp);
    let edge = |i: usize| (v[i], v[(i + 1) % k]);
    let lines: Vec<Option<(Pt, Pt)>> = (0..k)
        .map(|i| {
            let (a, b) = edge(i);
            let l = dist(a, b);
            let pts: Vec<Pt> = lp
                .iter()
                .copied()
                .filter(|&q| {
                    let d = seg_dist(q, a, b);
                    dist(q, a) > 0.2 * l
                        && dist(q, b) > 0.2 * l
                        && (0..k).all(|j| {
                            let (c, e) = edge(j);
                            j == i || seg_dist(q, c, e) >= d
                        })
                })
                .collect();
            (pts.len() >= 3).then(|| (centroid(&pts), principal_dir(&pts)))
        })
        .collect();
    (0..k)
        .map(|i| match (lines[(i + k - 1) % k], lines[i]) {
            (Some((p1, d1)), Some((p2, d2))) => intersect(p1, d1, p2, d2)
                .filter(|x| dist(*x, v[i]) <= reach)
                .unwrap_or(v[i]),
            _ => v[i],
        })
        .collect()
}

/// Mean distance of `p` from the closed polygon `v`.
fn polygon_error(v: &[Pt], p: &[Pt]) -> f64 {
    let k = v.len();
    if k < 2 {
        return f64::INFINITY;
    }
    let sum: f64 = p
        .iter()
        .map(|&q| {
            (0..k)
                .map(|i| seg_dist(q, v[i], v[(i + 1) % k]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    sum / p.len().max(1) as f64
}

/// Interior angle at each corner of a convex polygon (degrees).
fn interior(v: &[Pt]) -> Vec<f64> {
    turns(v).iter().map(|t| 180.0 - t.abs()).collect()
}

/// The frame rotation (degrees, unsnapped) putting edge `base` of a triangle
/// at the bottom with the third corner above it, and where that corner falls
/// along the edge (0..1 = over it).
fn triangle_on(v: &[Pt], base: usize) -> Option<(f64, f64)> {
    let (b0, b1, apex) = (v[base], v[(base + 1) % 3], v[(base + 2) % 3]);
    let w = dist(b0, b1);
    if w <= 0.0 {
        return None;
    }
    let u = ((b1.0 - b0.0) / w, (b1.1 - b0.1) / w);
    // Local y runs from the apex down to the base (the preset's apex is on
    // its top edge, §20.1.10.56 `triangle`); local x = (y.1, −y.0) keeps the
    // frame a rotation, not a mirror.
    let n0 = (-u.1, u.0);
    let n = if (apex.0 - b0.0) * n0.0 + (apex.1 - b0.1) * n0.1 > 0.0 {
        n0
    } else {
        (u.1, -u.0)
    };
    let x = (-n.1, n.0);
    let t = ((apex.0 - b0.0) * u.0 + (apex.1 - b0.1) * u.1) / w;
    Some((deg(x.1.atan2(x.0)), t))
}

fn triangle(v: &[Pt], confidence: f64) -> Option<Recognized> {
    let e = |i: usize| dist(v[i], v[(i + 1) % 3]);
    let mut order = [0, 1, 2];
    order.sort_by(|&i, &j| e(j).total_cmp(&e(i)));
    let longest = order[0];
    let (b0, b1, apex) = (v[longest], v[(longest + 1) % 3], v[(longest + 2) % 3]);
    let w = dist(b0, b1);
    let h = ((apex.0 - b0.0) * (b1.1 - b0.1) - (apex.1 - b0.1) * (b1.0 - b0.0)).abs() / w.max(1e-9);
    if w < MIN_SIZE || h < 0.12 * w {
        return None;
    }
    // An edge lying along an axis (a right triangle's leg) is preferred as
    // the base over the longest edge, when the third corner stands over it.
    let snapped = order.iter().find_map(|&b| {
        let (raw, t) = triangle_on(v, b)?;
        let s = snap(raw, 90.0);
        ((s - raw).abs() > 0.0 || raw.rem_euclid(90.0) == 0.0)
            .then_some(())
            .filter(|()| (-0.05..=1.05).contains(&t))
            .map(|()| (b, s))
    });
    let (base, rot) = match snapped {
        Some(r) => r,
        None => (longest, triangle_on(v, longest)?.0),
    };
    let apex = v[(base + 2) % 3];
    let (center, w, h) = frame_of(v, rot);
    let lx = |p: Pt| rotate((p.0 - center.0, p.1 - center.1), -rot).0;
    let mut adj = ((lx(apex) + w / 2.0) / w).clamp(0.0, 1.0);
    for s in [0.0, 0.5, 1.0] {
        if (adj - s).abs() <= 0.07 {
            adj = s;
        }
    }
    Some(Recognized {
        kind: ShapeKind::Triangle,
        fit: Fit::Frame {
            center,
            w,
            h,
            rot,
            adj: Some(adj),
        },
        confidence,
    })
}

fn quad(v: &[Pt], confidence: f64) -> Option<Recognized> {
    let ang = interior(v);
    let len: Vec<f64> = (0..4).map(|i| dist(v[i], v[(i + 1) % 4])).collect();
    let (lmin, lmax) = len
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(a, b), &l| (a.min(l), b.max(l)));
    if lmin <= 0.0 {
        return None;
    }
    let rhombus = lmax / lmin <= 1.3;
    let right = ang.iter().all(|a| (a - 90.0).abs() <= RIGHT_TOL);
    if right {
        let edge = |i: usize| {
            let (a, b) = (v[i], v[(i + 1) % 4]);
            deg((b.1 - a.1).atan2(b.0 - a.0))
        };
        let theta = circular_mean((0..4).map(edge), 90.0);
        if rhombus && theta.abs() >= 45.0 - 12.0 {
            // A square standing on a corner reads as a diamond.
            return frame(ShapeKind::Diamond, v, 0.0, confidence);
        }
        let rot = snap(theta, 90.0);
        let (center, w, h) = frame_of(v, rot);
        let (kind, w, h) = if w.max(h) / w.min(h).max(1e-9) <= 1.15 {
            let s = w.midpoint(h);
            (ShapeKind::Square, s, s)
        } else {
            (ShapeKind::Rectangle, w, h)
        };
        return Some(Recognized {
            kind,
            fit: Fit::Frame {
                center,
                w,
                h,
                rot,
                adj: None,
            },
            confidence,
        });
    }
    if rhombus {
        // Diagonals cross at right angles; the frame follows the first one.
        let d = |a: Pt, b: Pt| deg((b.1 - a.1).atan2(b.0 - a.0));
        let d1 = d(v[0], v[2]);
        let d2 = d(v[1], v[3]);
        if wrap(d1 - d2, 180.0).abs() < 90.0 - RIGHT_TOL {
            return None;
        }
        let rot = snap(circular_mean([d1, d2 - 90.0].into_iter(), 90.0), 90.0);
        return frame(ShapeKind::Diamond, v, rot, confidence);
    }
    None
}

/// Rotation of a regular polygon whose first corner sits at `base` degrees,
/// corners every `period` degrees.
fn regular_rot(v: &[Pt], base: f64, period: f64) -> f64 {
    let c = centroid(v);
    let off = circular_mean(
        v.iter().map(|q| deg((q.1 - c.1).atan2(q.0 - c.0)) - base),
        period,
    );
    snap(off, period)
}

fn regular(
    kind: ShapeKind,
    v: &[Pt],
    base: f64,
    period: f64,
    confidence: f64,
) -> Option<Recognized> {
    let k = v.len() as f64;
    let ideal = 180.0 - 360.0 / k;
    let ang = interior(v);
    let len: Vec<f64> = (0..v.len())
        .map(|i| dist(v[i], v[(i + 1) % v.len()]))
        .collect();
    let (lmin, lmax) = len
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(a, b), &l| (a.min(l), b.max(l)));
    if ang.iter().any(|a| (a - ideal).abs() > 30.0) || lmax > 2.2 * lmin {
        return None;
    }
    frame(kind, v, regular_rot(v, base, period), confidence)
}

fn classify_polygon(v: &[Pt], confidence: f64) -> Option<Recognized> {
    let t = turns(v);
    let total: f64 = t.iter().sum();
    let convex = t.iter().all(|a| *a > 0.0) || t.iter().all(|a| *a < 0.0);
    match v.len() {
        3 => triangle(v, confidence),
        4 if convex => quad(v, confidence),
        // A pentagram drawn in one stroke turns twice around (720°).
        5 if total.abs() > 540.0 => {
            frame(ShapeKind::Star, v, regular_rot(v, -90.0, 72.0), confidence)
        }
        5 if convex => regular(ShapeKind::Pentagon, v, -90.0, 72.0, confidence),
        6 if convex => regular(ShapeKind::Hexagon, v, 0.0, 60.0, confidence),
        10 => {
            // An outline star: tips and notches alternate.
            let c = centroid(v);
            let r: Vec<f64> = v.iter().map(|q| dist(*q, c)).collect();
            let even: Vec<f64> = r.iter().step_by(2).copied().collect();
            let odd: Vec<f64> = r.iter().skip(1).step_by(2).copied().collect();
            let (outer, inner, tips) = if even.iter().sum::<f64>() >= odd.iter().sum::<f64>() {
                (
                    &even,
                    &odd,
                    v.iter().step_by(2).copied().collect::<Vec<Pt>>(),
                )
            } else {
                (&odd, &even, v.iter().skip(1).step_by(2).copied().collect())
            };
            let omin = outer.iter().copied().fold(f64::INFINITY, f64::min);
            let imax = inner.iter().copied().fold(0.0, f64::max);
            if imax >= 0.8 * omin {
                return None;
            }
            frame(
                ShapeKind::Star,
                &tips,
                regular_rot(&tips, -90.0, 72.0),
                confidence,
            )
        }
        _ => None,
    }
}

fn recognize_closed(lp: &[Pt]) -> Option<Recognized> {
    let scale = diag(lp) / 2.0;
    if scale <= 0.0 {
        return None;
    }
    let ell = fit_ellipse(lp).map(|e| {
        let err = ellipse_error(&e, lp) / scale;
        (e, err)
    });
    let mut poly = polygon(lp);
    let t = turns(&poly);
    // Convex, turning once (a pentagram turns twice).
    if (3..=6).contains(&poly.len())
        && t.iter().sum::<f64>().abs() < 540.0
        && (t.iter().all(|a| *a > 0.0) || t.iter().all(|a| *a < 0.0))
    {
        poly = refine(&poly, lp);
    }
    let perr = polygon_error(&poly, lp) / scale;
    let ell_err = ell.map_or(f64::INFINITY, |(_, e)| e);
    if perr <= POLY_MAX_ERR && perr < ell_err {
        if let Some(r) = classify_polygon(&poly, 1.0 - perr / POLY_MAX_ERR) {
            return Some(r);
        }
    }
    match ell {
        Some((e, err)) if err <= ELLIPSE_MAX_ERR => ellipse_rec(&e, 1.0 - err / ELLIPSE_MAX_ERR),
        _ => None,
    }
}

// -------------------------------------------------------------------- strokes

/// Recognise one stroke on its own.
#[must_use]
pub fn recognize_stroke(raw: &[Pt]) -> Option<Recognized> {
    let pts = clean(raw);
    if pts.len() < 2 || diag(&pts) < MIN_SIZE {
        return None;
    }
    let r = smooth(&resample(&pts, SAMPLES), false);
    if let Some(lp) = closed_loop(&r) {
        return recognize_closed(&lp);
    }
    let (from, to, confidence) = line_of(&r)?;
    Some(Recognized {
        kind: ShapeKind::Line,
        fit: Fit::Line {
            from,
            to,
            head_from: false,
            head_to: false,
        },
        confidence,
    })
}

/// Which end of the line `from`–`to` the stroke is an arrowhead stroke of
/// (`false` = `from`, `true` = `to`): short, close to that end, leaning back
/// along the line and off to its side.
#[must_use]
pub fn head_end(from: Pt, to: Pt, stroke: &[Pt]) -> Option<bool> {
    let pts = clean(stroke);
    let len = dist(from, to);
    if pts.is_empty() || len < MIN_SIZE {
        return None;
    }
    let hl = path_len(&pts);
    if hl > 0.6 * len || diag(&pts) < 2.0 {
        return None;
    }
    let dir = ((to.0 - from.0) / len, (to.1 - from.1) / len);
    let c = centroid(&pts);
    let mut best: Option<(bool, f64)> = None;
    for (at_to, e, back) in [(false, from, dir), (true, to, (-dir.0, -dir.1))] {
        let near = pts
            .iter()
            .map(|&p| dist(p, e))
            .fold(f64::INFINITY, f64::min);
        let far = pts.iter().map(|&p| dist(p, e)).fold(0.0, f64::max);
        if near > (0.15 * len).max(8.0) || far > 0.45 * len {
            continue;
        }
        let (vx, vy) = (c.0 - e.0, c.1 - e.1);
        let vl = vx.hypot(vy);
        // Within 65° of pointing back along the line.
        if vl <= 0.0 || (vx * back.0 + vy * back.1) / vl < 0.42 {
            continue;
        }
        let side = pts
            .iter()
            .map(|p| ((p.0 - e.0) * dir.1 - (p.1 - e.1) * dir.0).abs())
            .fold(0.0, f64::max);
        if side < 0.04 * len {
            continue;
        }
        if best.is_none_or(|(_, d)| near < d) {
            best = Some((at_to, near));
        }
    }
    best.map(|(b, _)| b)
}

/// Recognise a set of strokes: arrows (a line and its head strokes) first,
/// then each remaining stroke alone. Strokes in no unit stay ink.
#[must_use]
pub fn recognize_strokes(strokes: &[Vec<Pt>]) -> Vec<Unit> {
    let recs: Vec<Option<Recognized>> = strokes.iter().map(|s| recognize_stroke(s)).collect();
    let n = strokes.len();
    let mut used = vec![false; n];
    let mut units = Vec::new();
    let line_len = |i: usize| match recs[i].map(|r| r.fit) {
        Some(Fit::Line { from, to, .. }) => dist(from, to),
        _ => 0.0,
    };
    let mut lines: Vec<usize> = (0..n)
        .filter(|&i| recs[i].is_some_and(|r| r.kind == ShapeKind::Line))
        .collect();
    lines.sort_by(|&a, &b| line_len(b).total_cmp(&line_len(a)).then(a.cmp(&b)));
    for &i in &lines {
        if used[i] {
            continue;
        }
        let Some(rec) = recs[i] else { continue };
        let Fit::Line { from, to, .. } = rec.fit else {
            continue;
        };
        let mut members = vec![i];
        let (mut hf, mut ht) = (0, 0);
        for j in 0..n {
            if j == i || used[j] {
                continue;
            }
            match head_end(from, to, &strokes[j]) {
                Some(false) if hf < 2 => {
                    hf += 1;
                    members.push(j);
                }
                Some(true) if ht < 2 => {
                    ht += 1;
                    members.push(j);
                }
                _ => {}
            }
        }
        for &m in &members {
            used[m] = true;
        }
        let arrow = hf + ht > 0;
        members.sort_unstable();
        units.push(Unit {
            strokes: members,
            rec: Recognized {
                kind: if arrow {
                    ShapeKind::Arrow
                } else {
                    ShapeKind::Line
                },
                fit: Fit::Line {
                    from,
                    to,
                    head_from: hf > 0,
                    head_to: ht > 0,
                },
                confidence: rec.confidence,
            },
        });
    }
    for i in 0..n {
        if let (false, Some(rec)) = (used[i], recs[i]) {
            used[i] = true;
            units.push(Unit {
                strokes: vec![i],
                rec,
            });
        }
    }
    units.sort_by_key(|u| u.strokes.first().copied().unwrap_or(usize::MAX));
    units
}

#[cfg(test)]
#[path = "ink_recognize_tests.rs"]
mod tests;

//! Morph of shape outlines ([MS-PPTX] p159:morph): two matched shapes drawn with plain outlines
//! (solid or no fill, solid or no line) change shape point by point instead of cross-fading.
//!
//! Each outline is read from the shape's drawing (its paths before its text, transforms applied),
//! flattened and resampled into the same number of points per ring, then the rings of the two
//! shapes are put in correspondence (the cyclic start and the direction that bring the points
//! closest, each ring normalised to its own box). When the drawings differ in topology — another
//! number of paths or rings, an open against a closed ring, a fill that is not plain — there is no
//! outline morph and the shapes cross-fade as before.

use bo_render::{Command, PageList, Paint, PathSeg};
use serde::Serialize;

/// Points per ring after resampling.
pub const RING_POINTS: usize = 96;

/// Most paths a shape may have to morph point by point.
const MAX_PATHS: usize = 8;

/// One path of an outline: its rings (resampled, slide points), fill and line.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlinePath {
    /// Rings of the path, each [`RING_POINTS`] points `[x, y]`.
    pub rings: Vec<Vec<[f64; 2]>>,
    /// Whether each ring is closed.
    pub closed: Vec<bool>,
    /// Fill `[r, g, b, a]`, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 4]>,
    /// Line `[r, g, b, a]` and width (points), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<([u8; 4], f64)>,
}

/// The outlines of a matched pair, path for path and point for point.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorphOutline {
    /// On the previous slide.
    pub from: Vec<OutlinePath>,
    /// On this slide.
    pub to: Vec<OutlinePath>,
}

type Mat = [f64; 6];

fn mul(m: Mat, n: Mat) -> Mat {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

fn ap(m: Mat, x: f32, y: f32) -> [f64; 2] {
    let (x, y) = (f64::from(x), f64::from(y));
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

/// Rings of a path through `m`, curves flattened (16 steps each): (points, closed).
fn flatten(segs: &[PathSeg], m: Mat) -> Vec<(Vec<[f64; 2]>, bool)> {
    let mut out: Vec<(Vec<[f64; 2]>, bool)> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut last = [0.0, 0.0];
    let flush = |cur: &mut Vec<[f64; 2]>, closed: bool, out: &mut Vec<(Vec<[f64; 2]>, bool)>| {
        if cur.len() >= 2 {
            out.push((std::mem::take(cur), closed));
        } else {
            cur.clear();
        }
    };
    for s in segs {
        match *s {
            PathSeg::M(x, y) => {
                flush(&mut cur, false, &mut out);
                last = ap(m, x, y);
                cur.push(last);
            }
            PathSeg::L(x, y) => {
                last = ap(m, x, y);
                cur.push(last);
            }
            PathSeg::Q(a, b, x, y) => {
                let (p0, p1, p2) = (last, ap(m, a, b), ap(m, x, y));
                for k in 1..=16 {
                    let t = f64::from(k) / 16.0;
                    let u = 1.0 - t;
                    cur.push([
                        u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                        u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
                    ]);
                }
                last = p2;
            }
            PathSeg::C(a, b, c, d, x, y) => {
                let (p0, p1, p2, p3) = (last, ap(m, a, b), ap(m, c, d), ap(m, x, y));
                for k in 1..=16 {
                    let t = f64::from(k) / 16.0;
                    let u = 1.0 - t;
                    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push([
                        b0 * p0[0] + b1 * p1[0] + b2 * p2[0] + b3 * p3[0],
                        b0 * p0[1] + b1 * p1[1] + b2 * p2[1] + b3 * p3[1],
                    ]);
                }
                last = p3;
            }
            PathSeg::Z => {
                if let Some(&first) = cur.first() {
                    last = first;
                }
                flush(&mut cur, true, &mut out);
            }
        }
    }
    flush(&mut cur, false, &mut out);
    out
}

/// `n` points evenly spaced along a ring (by length; a closed ring comes back to its start).
#[must_use]
pub fn resample(pts: &[[f64; 2]], closed: bool, n: usize) -> Vec<[f64; 2]> {
    let mut poly = pts.to_vec();
    if closed {
        if let Some(&f) = pts.first() {
            poly.push(f);
        }
    }
    let lens: Vec<f64> = poly
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .collect();
    let total: f64 = lens.iter().sum();
    if total <= 0.0 || poly.len() < 2 {
        return vec![pts.first().copied().unwrap_or([0.0, 0.0]); n];
    }
    // A closed ring: n points around (the last one before the start); open: both ends included.
    let steps = if closed {
        n
    } else {
        n.saturating_sub(1).max(1)
    };
    let mut out = Vec::with_capacity(n);
    let (mut seg, mut acc) = (0usize, 0.0);
    for k in 0..n {
        let want = total * k as f64 / steps as f64;
        while seg + 1 < lens.len() && acc + lens[seg] < want {
            acc += lens[seg];
            seg += 1;
        }
        let l = lens[seg];
        let t = if l > 0.0 {
            ((want - acc) / l).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (a, b) = (poly[seg], poly[seg + 1]);
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    out
}

fn rgba(c: bo_render::Rgba) -> [u8; 4] {
    [c.0, c.1, c.2, c.3]
}

/// The plain outlines a drawing starts with (its paths before its text) and the rest of it
/// (its text). `None` when something before the text is not a plain path (a picture, a
/// gradient, a shadow), or there are no paths or too many.
#[must_use]
pub fn split_outlines(page: &PageList) -> Option<(Vec<OutlinePath>, PageList)> {
    let mut m: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut stack: Vec<Mat> = Vec::new();
    let mut paths = Vec::new();
    let mut rest = Vec::new();
    let mut text = false;
    for c in &page.commands {
        match c {
            Command::PushTransform { m: t } => {
                stack.push(m);
                m = mul(m, t.map(f64::from));
                rest.push(c.clone());
            }
            Command::PopTransform => {
                m = stack.pop().unwrap_or(m);
                rest.push(c.clone());
            }
            Command::PushClip { .. }
            | Command::PushClipPath { .. }
            | Command::PopClip
            | Command::PushOpacity { .. }
            | Command::PopOpacity => rest.push(c.clone()),
            Command::Glyphs { .. } => {
                text = true;
                rest.push(c.clone());
            }
            _ if text => rest.push(c.clone()),
            Command::Path { segs, fill, stroke } => {
                let fill = match fill {
                    None => None,
                    Some(Paint::Solid { color }) => Some(rgba(*color)),
                    Some(_) => return None,
                };
                if stroke.as_ref().is_some_and(|s| !s.dash.is_empty()) {
                    return None;
                }
                let stroke = stroke.as_ref().map(|s| {
                    // A line width through the transform (its mean scale).
                    let k = ((m[0] * m[3] - m[1] * m[2]).abs()).sqrt();
                    (rgba(s.color), f64::from(s.width) * k)
                });
                let rings = flatten(segs, m);
                if rings.is_empty() {
                    continue;
                }
                paths.push(OutlinePath {
                    closed: rings.iter().map(|r| r.1).collect(),
                    rings: rings
                        .iter()
                        .map(|(p, closed)| resample(p, *closed, RING_POINTS))
                        .collect(),
                    fill,
                    stroke,
                });
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                dash: None,
            } => {
                let k = ((m[0] * m[3] - m[1] * m[2]).abs()).sqrt();
                paths.push(OutlinePath {
                    rings: vec![resample(
                        &[ap(m, *x1, *y1), ap(m, *x2, *y2)],
                        false,
                        RING_POINTS,
                    )],
                    closed: vec![false],
                    fill: None,
                    stroke: Some((rgba(*color), f64::from(*width) * k)),
                });
            }
            _ => return None,
        }
    }
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return None;
    }
    Some((
        paths,
        PageList {
            width: page.width,
            height: page.height,
            commands: rest,
        },
    ))
}

/// A ring's points in its own box, `[0, 1]` each way.
fn normalised(r: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in r {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (w, h) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
    r.iter()
        .map(|p| [(p[0] - x0) / w, (p[1] - y0) / h])
        .collect()
}

/// Ring `b` started and run so that its points come closest to ring `a`'s (closed rings: every
/// start, both directions; open rings: both directions).
#[must_use]
pub fn align(a: &[[f64; 2]], b: &[[f64; 2]], closed: bool) -> Vec<[f64; 2]> {
    let n = a.len().min(b.len());
    if n == 0 {
        return b.to_vec();
    }
    let (na, nb) = (normalised(a), normalised(b));
    let cost = |shift: usize, rev: bool| -> f64 {
        (0..n)
            .map(|i| {
                let j = if rev {
                    (shift + n - i) % n
                } else {
                    (shift + i) % n
                };
                let (p, q) = (na[i], nb[j]);
                (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)
            })
            .sum()
    };
    let mut best = (f64::MAX, 0usize, false);
    let shifts: Vec<usize> = if closed { (0..n).collect() } else { vec![0] };
    for &s in &shifts {
        for rev in [false, true] {
            // An open ring runs backwards from its last point.
            let s = if !closed && rev { n - 1 } else { s };
            let c = cost(s, rev);
            if c < best.0 {
                best = (c, s, rev);
            }
        }
    }
    let (_, s, rev) = best;
    (0..n)
        .map(|i| b[if rev { (s + n - i) % n } else { (s + i) % n }])
        .collect()
}

/// The outline morph of two drawings of a matched shape, and their drawings without the outlines
/// (the text, which still moves with the shape's box). `None` when their outlines differ in
/// topology or are not plain.
#[must_use]
pub fn morph_outline(from: &PageList, to: &PageList) -> Option<(MorphOutline, PageList, PageList)> {
    let (a, ra) = split_outlines(from)?;
    let (mut b, rb) = split_outlines(to)?;
    if a.len() != b.len() {
        return None;
    }
    for (pa, pb) in a.iter().zip(b.iter_mut()) {
        if pa.closed != pb.closed || pa.fill.is_some() != pb.fill.is_some() {
            return None;
        }
        for (k, closed) in pa.closed.iter().enumerate() {
            pb.rings[k] = align(&pa.rings[k], &pb.rings[k], *closed);
        }
    }
    Some((MorphOutline { from: a, to: b }, ra, rb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_render::{Rgba, Stroke};

    fn path(segs: Vec<PathSeg>, fill: Option<Rgba>) -> Command {
        Command::Path {
            segs,
            fill: fill.map(|color| Paint::Solid { color }),
            stroke: Some(Stroke {
                width: 1.0,
                color: Rgba(0, 0, 0, 255),
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        }
    }

    fn page(c: Vec<Command>) -> PageList {
        PageList {
            width: 100.0,
            height: 100.0,
            commands: c,
        }
    }

    fn square(x: f32, y: f32, s: f32) -> Vec<PathSeg> {
        vec![
            PathSeg::M(x, y),
            PathSeg::L(x + s, y),
            PathSeg::L(x + s, y + s),
            PathSeg::L(x, y + s),
            PathSeg::Z,
        ]
    }

    // source: arc-length resampling — a 10 × 10 square's perimeter of 40 in 8 points: every 5
    // units along it, from its first corner.
    #[test]
    fn rings_resample_by_length() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let r = resample(&sq, true, 8);
        assert_eq!(
            r,
            vec![
                [0.0, 0.0],
                [5.0, 0.0],
                [10.0, 0.0],
                [10.0, 5.0],
                [10.0, 10.0],
                [5.0, 10.0],
                [0.0, 10.0],
                [0.0, 5.0]
            ]
        );
        // Open: both ends.
        assert_eq!(
            resample(&[[0.0, 0.0], [9.0, 0.0]], false, 4),
            vec![[0.0, 0.0], [3.0, 0.0], [6.0, 0.0], [9.0, 0.0]]
        );
    }

    // source: the same square started at another corner and run the other way is brought back
    // into correspondence (each point to its own corner).
    #[test]
    fn rings_align_start_and_direction() {
        let a = resample(
            &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            true,
            8,
        );
        let b = resample(
            &[[10.0, 10.0], [10.0, 0.0], [0.0, 0.0], [0.0, 10.0]],
            true,
            8,
        );
        assert_eq!(align(&a, &b, true), a);
    }

    // source: a square into a transformed triangle — plain outlines morph point by point (the
    // text after them stays a drawing of its own); a picture or another number of rings does not.
    #[test]
    fn plain_outlines_morph_and_others_do_not() {
        let text = Command::Glyphs {
            face: 0,
            size: 10.0,
            x: 0.0,
            y: 0.0,
            color: Rgba(0, 0, 0, 255),
            text: "a".into(),
            glyphs: vec![],
        };
        let a = page(vec![
            path(square(0.0, 0.0, 10.0), Some(Rgba(255, 0, 0, 255))),
            text.clone(),
        ]);
        let tri = vec![
            PathSeg::M(0.0, 0.0),
            PathSeg::L(10.0, 0.0),
            PathSeg::C(10.0, 5.0, 5.0, 10.0, 0.0, 10.0),
            PathSeg::Z,
        ];
        let b = page(vec![
            Command::PushTransform {
                m: [2.0, 0.0, 0.0, 2.0, 50.0, 0.0],
            },
            path(tri, Some(Rgba(0, 0, 255, 255))),
            text.clone(),
            Command::PopTransform,
        ]);
        let (o, ra, rb) = morph_outline(&a, &b).unwrap();
        assert_eq!(o.from.len(), 1);
        assert_eq!(o.from[0].rings[0].len(), RING_POINTS);
        assert_eq!(o.to[0].rings[0].len(), RING_POINTS);
        // In slide points: the second shape is at 50… scaled twice; its line twice as wide.
        assert!(o.to[0].rings[0].iter().all(|p| p[0] >= 50.0 - 1e-9));
        assert!((o.to[0].stroke.unwrap().1 - 2.0).abs() < 1e-9);
        assert_eq!(o.to[0].fill, Some([0, 0, 255, 255]));
        assert_eq!(ra.commands, vec![text.clone()]);
        assert_eq!(rb.commands.len(), 3, "the text keeps its transform");
        // Two rings against one; a picture; a gradient: no outline morph.
        let mut two = square(0.0, 0.0, 10.0);
        two.extend(square(20.0, 0.0, 5.0));
        assert!(morph_outline(&a, &page(vec![path(two, None)])).is_none());
        let pic = page(vec![Command::Image {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            image: "x".into(),
            crop: None,
            rotation: 0.0,
        }]);
        assert!(morph_outline(&a, &pic).is_none());
        assert!(split_outlines(&page(vec![text])).is_none());
    }
}

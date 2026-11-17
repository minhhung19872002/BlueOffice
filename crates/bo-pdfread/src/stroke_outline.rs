//! The area a stroke covers as a fillable path (ISO 32000-1 §8.5.3.2): line width, caps,
//! joins and dashes, flattened. Used to paint a stroke with a pattern (§8.7.3: the pattern is
//! the stroke's colour), which the display list cannot express as a stroke: the outline is
//! filled (or clipped to) with the pattern. Every piece winds the same way, so the nonzero
//! rule paints their union.
use bo_render::PathSeg;

type Pt = (f32, f32);

/// Polylines of the subpaths (curves flattened) and whether each is closed.
fn polylines(segs: &[PathSeg]) -> Vec<(Vec<Pt>, bool)> {
    let mut out: Vec<(Vec<Pt>, bool)> = Vec::new();
    let mut cur = (0.0, 0.0);
    for s in segs {
        match *s {
            PathSeg::M(x, y) => {
                out.push((vec![(x, y)], false));
                cur = (x, y);
            }
            PathSeg::L(x, y) => {
                if out.is_empty() {
                    out.push((vec![cur], false));
                }
                if let Some(p) = out.last_mut() {
                    p.0.push((x, y));
                }
                cur = (x, y);
            }
            PathSeg::Q(x1, y1, x, y) => {
                if let Some(p) = out.last_mut() {
                    for k in 1..=8 {
                        let t = k as f32 / 8.0;
                        let u = 1.0 - t;
                        p.0.push((
                            u * u * cur.0 + 2.0 * u * t * x1 + t * t * x,
                            u * u * cur.1 + 2.0 * u * t * y1 + t * t * y,
                        ));
                    }
                }
                cur = (x, y);
            }
            PathSeg::C(x1, y1, x2, y2, x, y) => {
                if let Some(p) = out.last_mut() {
                    for k in 1..=12 {
                        let t = k as f32 / 12.0;
                        let u = 1.0 - t;
                        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        p.0.push((
                            a * cur.0 + b * x1 + c * x2 + d * x,
                            a * cur.1 + b * y1 + c * y2 + d * y,
                        ));
                    }
                }
                cur = (x, y);
            }
            PathSeg::Z => {
                if let Some(p) = out.last_mut() {
                    p.1 = true;
                    if let Some(&first) = p.0.first() {
                        cur = first;
                    }
                }
            }
        }
    }
    out.retain(|p| p.0.len() > 1);
    out
}

/// Cut a polyline into its dashes (`dash`: on / off lengths, at least one positive).
fn dashes(line: &[Pt], dash: &[f32]) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    let mut piece: Vec<Pt> = vec![line[0]];
    let (mut k, mut left, mut on) = (0, dash[0], true);
    for w in line.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut len = (b.0 - a.0).hypot(b.1 - a.1);
        while len > 1e-6 {
            let step = left.min(len);
            let t = step / len;
            let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if on {
                piece.push(p);
            }
            len -= step;
            left -= step;
            a = p;
            if left <= 1e-6 {
                if on && piece.len() > 1 {
                    out.push(std::mem::take(&mut piece));
                }
                on = !on;
                k = (k + 1) % dash.len();
                left = dash[k].max(0.0);
                piece = vec![a];
            }
        }
    }
    if on && piece.len() > 1 {
        out.push(piece);
    }
    out
}

fn area(p: &[Pt]) -> f32 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f32>()
}

fn push_poly(out: &mut Vec<PathSeg>, mut p: Vec<Pt>) {
    if p.len() < 3 {
        return;
    }
    if area(&p) < 0.0 {
        p.reverse();
    }
    out.push(PathSeg::M(p[0].0, p[0].1));
    out.extend(p[1..].iter().map(|q| PathSeg::L(q.0, q.1)));
    out.push(PathSeg::Z);
}

fn disk(c: Pt, r: f32) -> Vec<Pt> {
    (0..16)
        .map(|k| {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// The stroke of `segs` (`width`, `cap` 0 butt / 1 round / 2 square, `join` 0 miter /
/// 1 round / 2 bevel, `dash` lengths) as a path to fill by the nonzero rule.
pub(crate) fn outline(
    segs: &[PathSeg],
    width: f32,
    cap: u8,
    join: u8,
    dash: &[f32],
) -> Vec<PathSeg> {
    let h = width.max(0.01) / 2.0;
    let mut out = Vec::new();
    let dashed = !dash.is_empty() && dash.iter().any(|d| *d > 0.0);
    for (mut line, closed) in polylines(segs) {
        if closed {
            line.push(line[0]);
        }
        let pieces = if dashed {
            dashes(&line, dash)
        } else {
            vec![line]
        };
        for piece in pieces {
            let ring = closed && !dashed;
            let n = piece.len();
            for i in 0..n - 1 {
                let (a, b) = (piece[i], piece[i + 1]);
                let len = (b.0 - a.0).hypot(b.1 - a.1);
                if len < 1e-6 {
                    continue;
                }
                let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
                let (nx, ny) = (-uy * h, ux * h);
                // Square caps reach half the width past the ends.
                let ext = |first: bool| {
                    if cap == 2 && !ring && (if first { i == 0 } else { i + 2 == n }) {
                        h
                    } else {
                        0.0
                    }
                };
                let (ea, eb) = (ext(true), ext(false));
                let a2 = (a.0 - ux * ea, a.1 - uy * ea);
                let b2 = (b.0 + ux * eb, b.1 + uy * eb);
                push_poly(
                    &mut out,
                    vec![
                        (a2.0 + nx, a2.1 + ny),
                        (b2.0 + nx, b2.1 + ny),
                        (b2.0 - nx, b2.1 - ny),
                        (a2.0 - nx, a2.1 - ny),
                    ],
                );
                // Join at the end of this segment.
                let next = if i + 2 < n {
                    Some(piece[i + 2])
                } else if ring && n > 2 {
                    Some(piece[1])
                } else {
                    None
                };
                if let Some(c) = next {
                    if join == 1 {
                        push_poly(&mut out, disk(b, h));
                    } else {
                        let l2 = (c.0 - b.0).hypot(c.1 - b.1).max(1e-6);
                        let (vx, vy) = ((c.0 - b.0) / l2, (c.1 - b.1) / l2);
                        let (mx, my) = (-vy * h, vx * h);
                        // Bevel on both sides (the outer one fills the gap, the inner one
                        // lies inside the stroke anyway).
                        push_poly(
                            &mut out,
                            vec![b, (b.0 + nx, b.1 + ny), (b.0 + mx, b.1 + my)],
                        );
                        push_poly(
                            &mut out,
                            vec![b, (b.0 - nx, b.1 - ny), (b.0 - mx, b.1 - my)],
                        );
                    }
                }
            }
            if cap == 1 && !ring {
                push_poly(&mut out, disk(piece[0], h));
                push_poly(&mut out, disk(piece[n - 1], h));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dashed_line_becomes_one_box_per_dash() {
        let line = [PathSeg::M(0.0, 0.0), PathSeg::L(100.0, 0.0)];
        let o = outline(&line, 4.0, 0, 0, &[10.0, 10.0]);
        let boxes = o.iter().filter(|s| matches!(s, PathSeg::M(..))).count();
        assert_eq!(boxes, 5);
        let solid = outline(&line, 4.0, 2, 0, &[]);
        let xs: Vec<f32> = solid
            .iter()
            .filter_map(|s| match s {
                PathSeg::M(x, _) | PathSeg::L(x, _) => Some(*x),
                _ => None,
            })
            .collect();
        // Square caps: two units past each end.
        assert!(xs.iter().any(|x| (x + 2.0).abs() < 1e-4));
        assert!(xs.iter().any(|x| (x - 102.0).abs() < 1e-4));
    }
}

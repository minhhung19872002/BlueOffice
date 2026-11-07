//! Smart guides: while moving or resizing, edges and centres of the moving
//! box snap to the slide's edges/centre and to other shapes' edges/centres
//! within a tolerance; the matching alignments are returned as guide lines
//! for the UI to draw.

use super::geom::Rect;
use serde::Serialize;

/// Orientation of a guide line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuideAxis {
    /// Vertical line at `x = pos` (from `y = from` to `y = to`).
    V,
    /// Horizontal line at `y = pos`.
    H,
}

/// A guide line in slide points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Guide {
    pub axis: GuideAxis,
    pub pos: f64,
    pub from: f64,
    pub to: f64,
}

/// Which features of the moving box may snap on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edges {
    /// Moving: start, centre and end edges.
    All,
    /// Resizing: only the start (left/top) edge moves.
    Start,
    /// Resizing: only the end (right/bottom) edge moves.
    End,
    /// Nothing moves on this axis.
    None,
}

/// A snap target: position plus the span (on the other axis) of what defines it.
#[derive(Debug, Clone, Copy)]
struct Target {
    pos: f64,
    span: (f64, f64),
}

fn targets(others: &[Rect], slide: (f64, f64), vertical: bool) -> Vec<Target> {
    let (w, h) = slide;
    let (len, other_len) = if vertical { (w, h) } else { (h, w) };
    let mut t = vec![
        Target {
            pos: 0.0,
            span: (0.0, other_len),
        },
        Target {
            pos: len / 2.0,
            span: (0.0, other_len),
        },
        Target {
            pos: len,
            span: (0.0, other_len),
        },
    ];
    for r in others {
        let (a, size, sa, ssize) = if vertical {
            (r.x, r.w, r.y, r.h)
        } else {
            (r.y, r.h, r.x, r.w)
        };
        for pos in [a, a + size / 2.0, a + size] {
            t.push(Target {
                pos,
                span: (sa, sa + ssize),
            });
        }
    }
    t
}

fn features(start: f64, size: f64, edges: Edges) -> Vec<f64> {
    match edges {
        Edges::All => vec![start, start + size / 2.0, start + size],
        Edges::Start => vec![start],
        Edges::End => vec![start + size],
        Edges::None => vec![],
    }
}

/// Best correction (target − feature) within `tol`, if any.
fn best(features: &[f64], targets: &[Target], tol: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for f in features {
        for t in targets {
            let d = t.pos - f;
            if d.abs() <= tol && best.is_none_or(|b| d.abs() < b.abs()) {
                best = Some(d);
            }
        }
    }
    best
}

/// Guides for the features that coincide with a target after snapping.
fn lines(
    features: &[f64],
    targets: &[Target],
    own_span: (f64, f64),
    axis: GuideAxis,
    out: &mut Vec<Guide>,
) {
    for t in targets {
        if features.iter().any(|f| (t.pos - f).abs() < 0.01) {
            let from = t.span.0.min(own_span.0);
            let to = t.span.1.max(own_span.1);
            if !out
                .iter()
                .any(|g| g.axis == axis && (g.pos - t.pos).abs() < 0.01)
            {
                out.push(Guide {
                    axis,
                    pos: t.pos,
                    from,
                    to,
                });
            } else if let Some(g) = out
                .iter_mut()
                .find(|g| g.axis == axis && (g.pos - t.pos).abs() < 0.01)
            {
                g.from = g.from.min(from);
                g.to = g.to.max(to);
            }
        }
    }
}

/// Snap result: correction to apply and the guides to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snap {
    pub dx: f64,
    pub dy: f64,
    pub guides: Vec<Guide>,
}

/// Snap `moving` (already moved/resized) against the slide and `others`.
/// `ex`/`ey` say which features may snap on each axis.
pub fn snap(
    moving: &Rect,
    others: &[Rect],
    slide: (f64, f64),
    tol: f64,
    ex: Edges,
    ey: Edges,
) -> Snap {
    let tx = targets(others, slide, true);
    let ty = targets(others, slide, false);
    let fx = features(moving.x, moving.w, ex);
    let fy = features(moving.y, moving.h, ey);
    let dx = best(&fx, &tx, tol).unwrap_or(0.0);
    let dy = best(&fy, &ty, tol).unwrap_or(0.0);
    let snapped = Rect {
        x: moving.x + if ex == Edges::End { 0.0 } else { dx },
        y: moving.y + if ey == Edges::End { 0.0 } else { dy },
        w: moving.w
            + match ex {
                Edges::Start => -dx,
                Edges::End => dx,
                _ => 0.0,
            },
        h: moving.h
            + match ey {
                Edges::Start => -dy,
                Edges::End => dy,
                _ => 0.0,
            },
    };
    let mut guides = Vec::new();
    lines(
        &features(snapped.x, snapped.w, ex),
        &tx,
        (snapped.y, snapped.bottom()),
        GuideAxis::V,
        &mut guides,
    );
    lines(
        &features(snapped.y, snapped.h, ey),
        &ty,
        (snapped.x, snapped.right()),
        GuideAxis::H,
        &mut guides,
    );
    Snap { dx, dy, guides }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLIDE: (f64, f64) = (960.0, 540.0);

    #[test]
    fn snaps_to_slide_centre() {
        // Box centre at 478 → slide centre 480 (tolerance 4).
        let r = Rect {
            x: 428.0,
            y: 100.0,
            w: 100.0,
            h: 50.0,
        };
        let s = snap(&r, &[], SLIDE, 4.0, Edges::All, Edges::All);
        assert_eq!(s.dx, 2.0);
        assert_eq!(s.dy, 0.0);
        assert!(s
            .guides
            .iter()
            .any(|g| g.axis == GuideAxis::V && g.pos == 480.0 && g.from == 0.0 && g.to == 540.0));
    }

    #[test]
    fn snaps_to_other_shape_edges() {
        let other = Rect {
            x: 200.0,
            y: 300.0,
            w: 80.0,
            h: 40.0,
        };
        // Left edge at 203 → other's left 200; top at 337 → other's bottom 340.
        let r = Rect {
            x: 203.0,
            y: 337.0,
            w: 30.0,
            h: 30.0,
        };
        let s = snap(&r, &[other], SLIDE, 4.0, Edges::All, Edges::All);
        assert_eq!((s.dx, s.dy), (-3.0, 3.0));
        let v = s.guides.iter().find(|g| g.axis == GuideAxis::V).unwrap();
        assert_eq!(v.pos, 200.0);
        assert_eq!((v.from, v.to), (300.0, 370.0));
        // Outside the tolerance nothing snaps.
        let far = Rect { x: 210.0, ..r };
        assert_eq!(
            snap(&far, &[other], SLIDE, 4.0, Edges::All, Edges::None).dx,
            0.0
        );
    }

    #[test]
    fn resize_snaps_only_the_moving_edge() {
        let r = Rect {
            x: 100.0,
            y: 100.0,
            w: 378.0,
            h: 50.0,
        };
        // Right edge 478 is within 4 of the centre line 480.
        let s = snap(&r, &[], SLIDE, 4.0, Edges::End, Edges::None);
        assert_eq!(s.dx, 2.0);
        // The left edge 100 is not a candidate when only the right edge moves.
        let s = snap(
            &Rect { x: 2.0, ..r },
            &[],
            SLIDE,
            4.0,
            Edges::End,
            Edges::None,
        );
        assert_eq!(s.dx, 0.0);
    }
}

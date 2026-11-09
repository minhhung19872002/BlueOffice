//! Preset text warps — ECMA-376 Part 1, §20.1.9.19 `prstTxWarp` and the
//! presetTextWarpDefinitions annex (§20.1.10.76 `ST_TextShapeType`).
//!
//! The 40 definitions live in `textwarps.dsl` (same line format as `presets.dsl`). The
//! specification defines the guide formulas and the paths, but not how glyphs follow
//! them; the mapping below is BlueOffice's own reading of the paths:
//!
//! * **one path** (`textArchUp`, `textArchDown`, `textCircle`): the text follows the
//!   path. Horizontal position in the text block → arc-length fraction along the path;
//!   the glyphs stand on the path's normal, on the side towards the centre of the
//!   box (so the text stays inside the shape);
//! * **two or more paths**: envelopes. Paths are taken in pairs — disjoint pairs
//!   `(0,1) (2,3) …` for an even count, consecutive pairs `(0,1) (1,2) …` for an odd
//!   count — and the text block is split into that many equal horizontal bands. Within
//!   a band a point at (u, v) lands at `(1 − v)·top(u) + v·bottom(u)`, `top(u)` and
//!   `bottom(u)` being the points at arc-length fraction u of the two paths (each path
//!   read left to right).
//!
//! Outlines are flattened before mapping (straight edges bend with the warp).

use crate::geom::{eval_custom, Seg};
use bo_model::slide::{CustomGeometry, Guide};
use std::collections::HashMap;
use std::sync::OnceLock;

const DEFS: &str = include_str!("textwarps.dsl");

fn table() -> &'static HashMap<&'static str, CustomGeometry> {
    static T: OnceLock<HashMap<&'static str, CustomGeometry>> = OnceLock::new();
    T.get_or_init(|| crate::presets::parse_defs(DEFS))
}

/// Definition of a preset text warp (`None` for unknown names and `textNoShape`).
pub fn warp_preset(name: &str) -> Option<CustomGeometry> {
    table().get(name).cloned()
}

/// Names of all preset text warps of the annex, sorted (`textNoShape` excluded: no paths).
pub fn warp_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = table().keys().copied().collect();
    v.sort_unstable();
    v
}

/// A path flattened to a polyline with cumulative arc lengths.
#[derive(Debug, Clone, PartialEq)]
struct Polyline {
    pts: Vec<(f64, f64)>,
    cum: Vec<f64>,
}

impl Polyline {
    fn new(pts: Vec<(f64, f64)>) -> Option<Self> {
        if pts.len() < 2 {
            return None;
        }
        let mut cum = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        cum.push(0.0);
        for w in pts.windows(2) {
            acc += (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
            cum.push(acc);
        }
        (acc > 1e-9).then_some(Polyline { pts, cum })
    }

    fn len(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// Point and unit tangent at arc-length fraction `u` (clamped to 0..=1); the tangent
    /// is the chord over ±0.5 % of the length (smooth across polyline vertices).
    fn at(&self, u: f64) -> ((f64, f64), (f64, f64)) {
        let u = u.clamp(0.0, 1.0);
        let p = self.point(u);
        let (a, b) = (self.point(u - 0.005), self.point(u + 0.005));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = dx.hypot(dy);
        if n > 1e-12 {
            (p, (dx / n, dy / n))
        } else {
            (p, self.segment(u).1)
        }
    }

    fn point(&self, u: f64) -> (f64, f64) {
        self.segment(u).0
    }

    /// Point and direction of the polyline segment at fraction `u`.
    fn segment(&self, u: f64) -> ((f64, f64), (f64, f64)) {
        let s = u.clamp(0.0, 1.0) * self.len();
        let i = match self.cum.binary_search_by(|c| c.total_cmp(&s)) {
            Ok(i) | Err(i) => i.clamp(1, self.pts.len() - 1),
        };
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let seg = self.cum[i] - self.cum[i - 1];
        let t = if seg > 0.0 {
            (s - self.cum[i - 1]) / seg
        } else {
            0.0
        };
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = dx.hypot(dy).max(1e-12);
        ((a.0 + dx * t, a.1 + dy * t), (dx / n, dy / n))
    }
}

/// Flatten the first subpath of `segs` (curves into `steps` lines each).
fn flatten_first(segs: &[Seg], steps: usize) -> Vec<(f64, f64)> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut pen = (0.0, 0.0);
    for s in segs {
        match *s {
            Seg::Move(x, y) => {
                if pts.len() > 1 {
                    break;
                }
                pts.clear();
                pen = (x, y);
                pts.push(pen);
            }
            Seg::Line(x, y) => {
                pen = (x, y);
                pts.push(pen);
            }
            Seg::Quad(x1, y1, x, y) => {
                for k in 1..=steps {
                    let t = k as f64 / steps as f64;
                    let m = 1.0 - t;
                    pts.push((
                        m * m * pen.0 + 2.0 * m * t * x1 + t * t * x,
                        m * m * pen.1 + 2.0 * m * t * y1 + t * t * y,
                    ));
                }
                pen = (x, y);
            }
            Seg::Cubic(x1, y1, x2, y2, x, y) => {
                for k in 1..=steps {
                    pts.push(cubic_at(
                        pen,
                        (x1, y1),
                        (x2, y2),
                        (x, y),
                        k as f64 / steps as f64,
                    ));
                }
                pen = (x, y);
            }
            Seg::Close => {
                if let Some(&f) = pts.first() {
                    pts.push(f);
                }
            }
        }
    }
    pts
}

fn cubic_at(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let m = 1.0 - t;
    let (a, b, c, d) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    /// Text follows one path; `towards`: the path normal (left of travel, y down)
    /// points to the box centre, so the text's bottom sits on the path.
    Follow { path: Polyline, towards: bool },
    /// Envelope bands (top, bottom), top to bottom.
    Bands(Vec<(Polyline, Polyline)>),
}

/// A preset text warp evaluated for a `w × h` text box.
#[derive(Debug, Clone, PartialEq)]
pub struct TextWarp {
    mode: Mode,
    /// The box the warp was evaluated for.
    size: (f64, f64),
}

impl TextWarp {
    /// Evaluate preset `name` (with adjust-value overrides `av`, §20.1.9.5 avLst) for a
    /// `w × h` box. `None` for `textNoShape`, unknown names or degenerate paths.
    pub fn new(name: &str, av: &[Guide], w: f64, h: f64) -> Option<TextWarp> {
        let def = warp_preset(name)?;
        let ev = eval_custom(&def, av, w, h).ok()?;
        let mut lines: Vec<Polyline> = Vec::new();
        for p in &ev.paths {
            lines.push(Polyline::new(flatten_first(&p.segs, 24))?);
        }
        let mode = match lines.len() {
            0 => return None,
            1 => {
                let path = lines.pop()?;
                let (p, t) = path.at(0.5);
                let n = (t.1, -t.0);
                let towards = n.0 * (w / 2.0 - p.0) + n.1 * (h / 2.0 - p.1) > 0.0;
                Mode::Follow { path, towards }
            }
            n => {
                // Each envelope path read left to right.
                for l in &mut lines {
                    let (a, b) = (l.pts[0], l.pts[l.pts.len() - 1]);
                    if b.0 < a.0 - 1e-9 {
                        let mut pts = l.pts.clone();
                        pts.reverse();
                        *l = Polyline::new(pts)?;
                    }
                }
                let pairs: Vec<(usize, usize)> = if n % 2 == 0 {
                    (0..n).step_by(2).map(|i| (i, i + 1)).collect()
                } else {
                    (0..n - 1).map(|i| (i, i + 1)).collect()
                };
                Mode::Bands(
                    pairs
                        .into_iter()
                        .map(|(a, b)| (lines[a].clone(), lines[b].clone()))
                        .collect(),
                )
            }
        };
        Some(TextWarp { mode, size: (w, h) })
    }

    /// Where the point at fraction (`u`, `v`) of the text block (0 = left / top,
    /// 1 = right / bottom) lands; `text_h` is the block height (used when following
    /// a single path, where the glyphs keep their height).
    pub fn map(&self, u: f64, v: f64, text_h: f64) -> (f64, f64) {
        match &self.mode {
            Mode::Follow { path, towards } => {
                let (p, t) = path.at(u);
                let n = (t.1, -t.0);
                let off = if *towards {
                    (1.0 - v) * text_h
                } else {
                    -v * text_h
                };
                (p.0 + n.0 * off, p.1 + n.1 * off)
            }
            Mode::Bands(bands) => {
                let k = bands.len() as f64;
                let i = ((v * k).floor().max(0.0) as usize).min(bands.len() - 1);
                let lv = v * k - i as f64;
                let (top, bottom) = &bands[i];
                let (a, _) = top.at(u);
                let (b, _) = bottom.at(u);
                (a.0 + (b.0 - a.0) * lv, a.1 + (b.1 - a.1) * lv)
            }
        }
    }

    /// Height a `bw × bh` text block gets when it follows a single path: stretched along
    /// the path, it keeps its proportions, but stays within a third of the smaller box side
    /// (BlueOffice's choice). Envelope warps ignore it.
    pub fn follow_height(&self, bw: f64, bh: f64) -> f64 {
        match &self.mode {
            Mode::Follow { path, .. } => {
                let k = path.len() / bw.max(1e-9);
                (bh * k).min(self.size.0.min(self.size.1) / 3.0).max(bh)
            }
            Mode::Bands(_) => bh,
        }
    }

    /// The paths of the warp flattened (for gallery previews), in box coordinates.
    pub fn guides(&self) -> Vec<Vec<(f64, f64)>> {
        match &self.mode {
            Mode::Follow { path, .. } => vec![path.pts.clone()],
            Mode::Bands(b) => b
                .iter()
                .flat_map(|(t, d)| [t.pts.clone(), d.pts.clone()])
                .collect(),
        }
    }

    /// Map an outline drawn inside `src` (`[x0, y0, x1, y1]`, the text block) through
    /// the warp. Curves are flattened and long lines split so they bend with it.
    pub fn warp_segs(&self, segs: &[Seg], src: [f64; 4]) -> Vec<Seg> {
        let (bw, bh) = ((src[2] - src[0]).max(1e-9), (src[3] - src[1]).max(1e-9));
        let max_len = (bw.hypot(bh) / 96.0).max(0.25);
        let th = self.follow_height(bw, bh);
        let map = |(x, y): (f64, f64)| self.map((x - src[0]) / bw, (y - src[1]) / bh, th);
        let mut out = Vec::with_capacity(segs.len() * 4);
        let mut pen = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        let line_to = |out: &mut Vec<Seg>, from: (f64, f64), to: (f64, f64)| {
            let len = (to.0 - from.0).hypot(to.1 - from.1);
            let n = ((len / max_len).ceil() as usize).clamp(1, 64);
            for k in 1..=n {
                let t = k as f64 / n as f64;
                let p = map((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t));
                out.push(Seg::Line(p.0, p.1));
            }
        };
        for s in segs {
            match *s {
                Seg::Move(x, y) => {
                    pen = (x, y);
                    start = pen;
                    let p = map(pen);
                    out.push(Seg::Move(p.0, p.1));
                }
                Seg::Line(x, y) => {
                    line_to(&mut out, pen, (x, y));
                    pen = (x, y);
                }
                Seg::Quad(x1, y1, x, y) => {
                    let c1 = (
                        pen.0 + 2.0 / 3.0 * (x1 - pen.0),
                        pen.1 + 2.0 / 3.0 * (y1 - pen.1),
                    );
                    let c2 = (x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
                    let mut prev = pen;
                    for k in 1..=8 {
                        let p = cubic_at(pen, c1, c2, (x, y), f64::from(k) / 8.0);
                        line_to(&mut out, prev, p);
                        prev = p;
                    }
                    pen = (x, y);
                }
                Seg::Cubic(x1, y1, x2, y2, x, y) => {
                    let mut prev = pen;
                    for k in 1..=8 {
                        let p = cubic_at(pen, (x1, y1), (x2, y2), (x, y), f64::from(k) / 8.0);
                        line_to(&mut out, prev, p);
                        prev = p;
                    }
                    pen = (x, y);
                }
                Seg::Close => {
                    line_to(&mut out, pen, start);
                    pen = start;
                    out.push(Seg::Close);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
        (a.0 - b.0).abs() < tol && (a.1 - b.1).abs() < tol
    }

    #[test]
    fn annex_definitions_are_all_there() {
        let names = warp_names();
        // ST_TextShapeType has 41 values; textNoShape has no definition.
        assert_eq!(names.len(), 40, "{names:?}");
        assert!(names.contains(&"textArchUp") && names.contains(&"textWave1"));
        assert!(warp_preset("textNoShape").is_none());
        // Every preset evaluates for a 200 × 100 box.
        for n in names {
            assert!(TextWarp::new(n, &[], 200.0, 100.0).is_some(), "{n}");
        }
    }

    /// textArchUp, adj = cd2 (180°), box 200 × 100: adval = 180°, v1 = 0, so
    /// end = v2 = 360°, d1 = 180°; w2 = 90° > 0 → c2 = d1; v1 = 0 → c1 = c2;
    /// w1 = −90° → swAng = c1 = 180°. Start (x1, y1) = ellipse point at 180° =
    /// (hc − wd2, vc) = (0, 50); the arc sweeps over the top (100, 0) to (200, 50).
    #[test]
    fn arch_up_follows_the_upper_half_ellipse() {
        let w = TextWarp::new("textArchUp", &[], 200.0, 100.0).unwrap();
        assert!(close(w.map(0.0, 0.0, 20.0), (0.0, 50.0), 1e-6));
        assert!(close(w.map(1.0, 0.0, 20.0), (200.0, 50.0), 1e-6));
        // Half the arc length: the top of the ellipse (symmetry).
        assert!(close(w.map(0.5, 0.0, 20.0), (100.0, 0.0), 0.05));
        // The normal there points up, away from the centre: the text hangs below the
        // path, its top on it; half-way down a 20 pt block = 10 pt below.
        assert!(
            close(w.map(0.5, 0.5, 20.0), (100.0, 10.0), 0.05),
            "{:?}",
            w.map(0.5, 0.5, 20.0)
        );
        assert!(close(w.map(0.5, 1.0, 20.0), (100.0, 20.0), 0.05));
    }

    /// textWave1, adj1 = 12500, adj2 = 0, box 200 × 100: y1 = 12.5, dy2 = y1·10/3 = 41.667,
    /// y2 = −29.167, y3 = 54.167, y4 = 87.5, y5 = 45.833, y6 = 129.167; of = 0 so x2 = x6 = 0,
    /// x5 = x10 = 200, dx3 = 200/3, x3 = x7 = 66.667, x4 = x8 = 133.333.
    /// Top: M(0,12.5) C(66.667,−29.167)(133.333,54.167)(200,12.5) — point-symmetric about
    /// (100, 12.5) (P1 + P2 = 2·(100, 12.5)), so half its length is there; the bottom wave the
    /// same about (100, 87.5).
    #[test]
    fn wave1_envelope_maps_between_the_waves() {
        let w = TextWarp::new("textWave1", &[], 200.0, 100.0).unwrap();
        assert!(close(w.map(0.0, 0.0, 1.0), (0.0, 12.5), 1e-6));
        assert!(close(w.map(1.0, 1.0, 1.0), (200.0, 87.5), 1e-6));
        assert!(close(w.map(0.5, 0.0, 1.0), (100.0, 12.5), 0.05));
        assert!(close(w.map(0.5, 1.0, 1.0), (100.0, 87.5), 0.05));
        assert!(close(w.map(0.5, 0.5, 1.0), (100.0, 50.0), 0.05));
        // A quarter down at the left edge: 12.5 + 0.25 · 75.
        assert!(close(w.map(0.0, 0.25, 1.0), (0.0, 31.25), 1e-6));
        // Adjust value override: adj1 = 0 flattens both waves to the box edges.
        let flat = TextWarp::new(
            "textWave1",
            &[Guide {
                name: "adj1".into(),
                fmla: "val 0".into(),
            }],
            200.0,
            100.0,
        )
        .unwrap();
        assert!(close(flat.map(0.3, 0.0, 1.0), (60.0, 0.0), 1e-6));
        assert!(close(flat.map(0.3, 1.0, 1.0), (60.0, 100.0), 1e-6));
    }

    #[test]
    fn plain_is_the_identity_for_the_full_box() {
        // textPlain with adj = 50000: top line l..r at t, bottom line l..r at b.
        let w = TextWarp::new("textPlain", &[], 200.0, 100.0).unwrap();
        let segs = [
            Seg::Move(10.0, 0.0),
            Seg::Line(30.0, 0.0),
            Seg::Line(30.0, 10.0),
            Seg::Close,
        ];
        let out = w.warp_segs(&segs, [10.0, 0.0, 30.0, 10.0]);
        let Seg::Move(x, y) = out[0] else { panic!() };
        assert!(close((x, y), (0.0, 0.0), 1e-6));
        let last_line = out
            .iter()
            .rev()
            .find_map(|s| match *s {
                Seg::Line(x, y) => Some((x, y)),
                _ => None,
            })
            .unwrap();
        // The closing edge ends back at the start.
        assert!(close(last_line, (0.0, 0.0), 1e-6));
        assert!(out
            .iter()
            .any(|s| matches!(*s, Seg::Line(x, y) if close((x, y), (200.0, 100.0), 1e-6))));
        assert_eq!(out.last(), Some(&Seg::Close));
    }

    #[test]
    fn deflate_inflate_uses_two_bands() {
        // Four paths → bands (0,1) and (2,3): v = 0.5 is the bottom of the first band.
        let w = TextWarp::new("textDeflateInflate", &[], 200.0, 100.0).unwrap();
        let Mode::Bands(b) = &w.mode else { panic!() };
        assert_eq!(b.len(), 2);
        // textButton: three paths → overlapping pairs (0,1), (1,2).
        let w = TextWarp::new("textButton", &[], 200.0, 100.0).unwrap();
        let Mode::Bands(b) = &w.mode else { panic!() };
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].1, b[1].0);
    }
}

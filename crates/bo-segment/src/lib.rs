//! Picture background removal for BlueOffice (CORE-PIC-002): an interactive
//! graph-cut segmentation of the GrabCut family (Rother, Kolmogorov, Blake,
//! SIGGRAPH 2004), written from the published descriptions, with no learnt weights.
//!
//! * The picture is reduced to at most [`WORK_SIDE`] pixels on its longer side.
//! * Pixels outside a rectangle (the picture minus a margin, or the user's) are
//!   background; inside they are probably foreground. Strokes of the user ("keep" /
//!   "remove") are hard constraints.
//! * Foreground and background colours are modelled by Gaussian mixtures
//!   ([`COMPONENTS`] components each, k-means initialisation); the labelling is the
//!   minimum cut of the 8-connected pixel graph ([`maxflow`]), and models and cut are
//!   iterated at most [`MAX_ITERATIONS`] times (on a half-size copy first for pictures
//!   over [`COARSE_SIDE`], then at most [`FINE_ITERATIONS`] more at the working size).
//! * The mask is brought back to the picture's size by a joint bilateral upsampling
//!   guided by the full-size colours (edge-aware), then feathered by a 1–2 pixel ramp.
//!
//! A [`SegmentationProvider`] can supply the initial estimate instead of the
//! rectangle (e.g. a licensed salient-object model run by the host); only the
//! graph-cut provider ships.

#![forbid(unsafe_code)]

pub mod gmm;
pub mod maxflow;

use gmm::{Gmm, Rgb};
use maxflow::Graph;

/// Longest side of the picture the segmentation works on.
pub const WORK_SIDE: u32 = 512;
/// Gaussian components per colour model.
pub const COMPONENTS: usize = 5;
/// Bound on the model ↔ cut iterations.
pub const MAX_ITERATIONS: usize = 5;
/// Default margin of the initial rectangle (fraction of each side).
pub const DEFAULT_MARGIN: f64 = 0.05;
/// Radius of a mark stroke, as a fraction of the picture's longer side.
pub const MARK_RADIUS: f64 = 1.0 / 80.0;

/// Smoothness weight of the pairwise term (GrabCut's γ).
const GAMMA: f64 = 50.0;
/// Integer scale of the graph capacities.
const SCALE: f64 = 1000.0;
/// Largest data term (after the per-pixel shift), in model units.
const MAX_DATA: f64 = 200.0;
/// Capacity of a hard constraint (more than any pixel's pairwise sum).
const HARD: i64 = 100_000_000;
/// Colour spread of the edge-aware upsampling (RGB units).
const UPSAMPLE_SIGMA: f64 = 24.0;

/// An RGBA picture (8 bits per channel, rows top to bottom).
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub rgba: &'a [u8],
}

/// An owned RGBA picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub rgba: Vec<u8>,
}

impl OwnedImage {
    /// Borrowed view.
    pub fn view(&self) -> Image<'_> {
        Image {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }
}

/// What a mark stroke asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// "Mark areas to keep".
    Keep,
    /// "Mark areas to remove".
    Remove,
}

/// A stroke of the user: a polyline in fractions (0..=1) of the picture's width and
/// height, painted with radius [`MARK_RADIUS`].
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// Keep or remove.
    pub kind: MarkKind,
    /// Points (x, y) as fractions of the picture's size.
    pub points: Vec<[f64; 2]>,
}

/// A coverage mask (255 = kept, 0 = removed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// One byte per pixel.
    pub alpha: Vec<u8>,
}

/// What to segment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Request {
    /// Rectangle holding the foreground: left, top, right, bottom edges as fractions
    /// of the picture (default: the picture minus [`DEFAULT_MARGIN`]).
    pub rect: Option<[f64; 4]>,
    /// Strokes, later ones over earlier ones.
    pub marks: Vec<Mark>,
}

/// Supplies the initial foreground estimate of a picture.
pub trait SegmentationProvider {
    /// Identifier (for diagnostics).
    fn id(&self) -> &'static str;
    /// Initial estimate at the size of `image` (≥ 128 = probably foreground), or
    /// `None` to start from the rectangle `rect` (pixels: left, top, right, bottom).
    fn initial_mask(&self, image: &Image<'_>, rect: [u32; 4]) -> Option<Mask>;
}

/// The graph-cut provider: starts from the rectangle.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphCut;

impl SegmentationProvider for GraphCut {
    fn id(&self) -> &'static str {
        "graph-cut"
    }

    fn initial_mask(&self, _: &Image<'_>, _: [u32; 4]) -> Option<Mask> {
        None
    }
}

/// A provider handing over a mask computed elsewhere (any size; resampled).
#[derive(Debug, Clone)]
pub struct PriorMask(pub Mask);

impl SegmentationProvider for PriorMask {
    fn id(&self) -> &'static str {
        "prior-mask"
    }

    fn initial_mask(&self, image: &Image<'_>, _: [u32; 4]) -> Option<Mask> {
        let m = &self.0;
        if m.width == 0 || m.height == 0 || m.alpha.len() != (m.width * m.height) as usize {
            return None;
        }
        let (w, h) = (image.width, image.height);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let sy = (u64::from(y) * u64::from(m.height) / u64::from(h)) as u32;
            for x in 0..w {
                let sx = (u64::from(x) * u64::from(m.width) / u64::from(w)) as u32;
                alpha.push(m.alpha[(sy * m.width + sx) as usize]);
            }
        }
        Some(Mask {
            width: w,
            height: h,
            alpha,
        })
    }
}

/// Result of [`segment`]: the binary mask at the working size and the picture it was
/// computed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segmentation {
    /// Working picture (reduced copy).
    pub work: OwnedImage,
    /// Binary mask of `work` (255 = keep).
    pub mask: Mask,
    /// Model ↔ cut iterations run.
    pub iterations: usize,
}

/// Size of the working copy of a `w × h` picture.
pub fn work_size(w: u32, h: u32) -> (u32, u32) {
    let long = w.max(h).max(1);
    if long <= WORK_SIDE {
        return (w.max(1), h.max(1));
    }
    let k = f64::from(WORK_SIDE) / f64::from(long);
    (
        ((f64::from(w) * k).round() as u32).max(1),
        ((f64::from(h) * k).round() as u32).max(1),
    )
}

/// Reduced copy (area average) of `img` at `ww × wh`.
pub fn downscale(img: &Image<'_>, ww: u32, wh: u32) -> OwnedImage {
    let (w, h) = (img.width as usize, img.height as usize);
    let (ww, wh) = (ww as usize, wh as usize);
    if w == ww && h == wh {
        return OwnedImage {
            width: img.width,
            height: img.height,
            rgba: img.rgba.to_vec(),
        };
    }
    let mut out = vec![0u8; ww * wh * 4];
    for ty in 0..wh {
        let y0 = ty * h / wh;
        let y1 = ((ty + 1) * h / wh).max(y0 + 1).min(h);
        for tx in 0..ww {
            let x0 = tx * w / ww;
            let x1 = ((tx + 1) * w / ww).max(x0 + 1).min(w);
            let mut acc = [0u64; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * 4;
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += u64::from(img.rgba.get(i + c).copied().unwrap_or(0));
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            let o = (ty * ww + tx) * 4;
            for c in 0..4 {
                out[o + c] = ((acc[c] + n / 2) / n) as u8;
            }
        }
    }
    OwnedImage {
        width: ww as u32,
        height: wh as u32,
        rgba: out,
    }
}

/// Default rectangle: the picture minus [`DEFAULT_MARGIN`] on every side.
pub fn default_rect() -> [f64; 4] {
    [
        DEFAULT_MARGIN,
        DEFAULT_MARGIN,
        1.0 - DEFAULT_MARGIN,
        1.0 - DEFAULT_MARGIN,
    ]
}

/// Pixel rectangle (left, top, right, bottom; right/bottom exclusive) of a fraction
/// rectangle on a `w × h` picture; at least one pixel.
fn pixel_rect(r: [f64; 4], w: u32, h: u32) -> [u32; 4] {
    let c = |v: f64| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let (l, t, rr, b) = (c(r[0]), c(r[1]), c(r[2]), c(r[3]));
    let (l, rr) = (l.min(rr), l.max(rr));
    let (t, b) = (t.min(b), t.max(b));
    let x0 = (l * f64::from(w)).round() as u32;
    let y0 = (t * f64::from(h)).round() as u32;
    let x1 = ((rr * f64::from(w)).round() as u32).clamp(x0 + 1, w.max(x0 + 1));
    let y1 = ((b * f64::from(h)).round() as u32).clamp(y0 + 1, h.max(y0 + 1));
    [
        x0.min(w.saturating_sub(1)),
        y0.min(h.saturating_sub(1)),
        x1.min(w),
        y1.min(h),
    ]
}

const HARD_BG: u8 = 0;
const HARD_FG: u8 = 1;
const PR_BG: u8 = 2;
const PR_FG: u8 = 3;

fn is_fg(l: u8) -> bool {
    l == HARD_FG || l == PR_FG
}

/// Paint the strokes on the label map (`w × h`) as hard labels.
fn paint_marks(labels: &mut [u8], w: u32, h: u32, marks: &[Mark]) {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let r = (wf.max(hf) * MARK_RADIUS).max(1.5);
    for m in marks {
        let label = match m.kind {
            MarkKind::Keep => HARD_FG,
            MarkKind::Remove => HARD_BG,
        };
        let pts: Vec<[f64; 2]> = m
            .points
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .map(|p| [p[0] * wf, p[1] * hf])
            .collect();
        let segs: Vec<([f64; 2], [f64; 2])> = if pts.len() == 1 {
            vec![(pts[0], pts[0])]
        } else {
            pts.windows(2).map(|s| (s[0], s[1])).collect()
        };
        for (a, b) in segs {
            let x0 = (a[0].min(b[0]) - r).floor().max(0.0) as u32;
            let x1 = ((a[0].max(b[0]) + r).ceil().max(0.0) as u32).min(w);
            let y0 = (a[1].min(b[1]) - r).floor().max(0.0) as u32;
            let y1 = ((a[1].max(b[1]) + r).ceil().max(0.0) as u32).min(h);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let t = if len2 > 0.0 {
                        (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
                    if (px - cx).powi(2) + (py - cy).powi(2) <= r * r {
                        labels[(y * w + x) as usize] = label;
                    }
                }
            }
        }
    }
}

fn colours(img: &Image<'_>) -> Vec<Rgb> {
    img.rgba
        .chunks_exact(4)
        .map(|p| {
            // Transparent pixels count as white (what a page shows behind them).
            let a = f64::from(p[3]) / 255.0;
            let mix = |c: u8| f64::from(c) * a + 255.0 * (1.0 - a);
            [mix(p[0]), mix(p[1]), mix(p[2])]
        })
        .collect()
}

/// Neighbour offsets of the 8-connected grid, each edge once: right, down, down-right,
/// down-left; with their lengths.
const DIRS: [(i32, i32, f64); 4] = [
    (1, 0, 1.0),
    (0, 1, 1.0),
    (1, 1, std::f64::consts::SQRT_2),
    (-1, 1, std::f64::consts::SQRT_2),
];

/// Integer pairwise capacities of every pixel towards each of [`DIRS`] (0 off-grid).
fn pairwise(z: &[Rgb], w: usize, h: usize) -> Vec<[i32; 4]> {
    let d2 =
        |a: &Rgb, b: &Rgb| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let neighbour = |x: usize, y: usize, d: usize| {
        let (dx, dy, _) = DIRS[d];
        let nx = x as i64 + i64::from(dx);
        let ny = y as i64 + i64::from(dy);
        (nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h)
            .then(|| ny as usize * w + nx as usize)
    };
    // β = 1 / (2 ⟨‖z_m − z_n‖²⟩) over every neighbour pair.
    let (mut sum, mut count) = (0.0f64, 0usize);
    for y in 0..h {
        for x in 0..w {
            for d in 0..4 {
                if let Some(n) = neighbour(x, y, d) {
                    sum += d2(&z[y * w + x], &z[n]);
                    count += 1;
                }
            }
        }
    }
    let beta = if sum > 0.0 {
        count as f64 / (2.0 * sum)
    } else {
        0.0
    };
    let mut out = vec![[0i32; 4]; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            for d in 0..4 {
                if let Some(n) = neighbour(x, y, d) {
                    let v = GAMMA / DIRS[d].2 * (-beta * d2(&z[i], &z[n])).exp();
                    out[i][d] = (v * SCALE).round() as i32;
                }
            }
        }
    }
    out
}

/// Initial label map of a `w × h` picture: hard background outside the rectangle,
/// probable foreground inside (or the provider's estimate), then the strokes.
fn initial_labels(view: &Image<'_>, req: &Request, provider: &dyn SegmentationProvider) -> Vec<u8> {
    let (w, h) = (view.width, view.height);
    let rect = pixel_rect(req.rect.unwrap_or_else(default_rect), w, h);
    let prior = provider.initial_mask(view, rect);
    let mut labels = vec![HARD_BG; (w * h) as usize];
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let i = (y * w + x) as usize;
            labels[i] = match &prior {
                Some(p) if p.alpha.get(i).copied().unwrap_or(255) < 128 => PR_BG,
                _ => PR_FG,
            };
        }
    }
    paint_marks(&mut labels, w, h, &req.marks);
    labels
}

fn split(z: &[Rgb], labels: &[u8]) -> (Vec<Rgb>, Vec<Rgb>) {
    let mut fg = Vec::new();
    let mut bg = Vec::new();
    for (p, l) in z.iter().zip(labels) {
        if is_fg(*l) {
            fg.push(*p);
        } else {
            bg.push(*p);
        }
    }
    (fg, bg)
}

/// GrabCut iterations on a `w × h` label map: refit the colour models to the current
/// labels, cut, relabel the probable pixels; at most `max` times, stopping when no
/// label changes. Returns the iterations run and the models (`None` when one side has
/// no pixels).
fn iterate(
    z: &[Rgb],
    w: usize,
    h: usize,
    labels: &mut [u8],
    models: Option<(Gmm, Gmm)>,
    max: usize,
) -> (usize, Option<(Gmm, Gmm)>) {
    let n = w * h;
    let (mut fg_pts, mut bg_pts) = split(z, labels);
    if fg_pts.is_empty() || bg_pts.is_empty() {
        return (0, None);
    }
    let (mut fg, mut bg) = models.unwrap_or_else(|| {
        (
            Gmm::from_kmeans(&fg_pts, COMPONENTS),
            Gmm::from_kmeans(&bg_pts, COMPONENTS),
        )
    });
    let pair = pairwise(z, w, h);
    let mut iterations = 0;
    for _ in 0..max {
        iterations += 1;
        fg = fg.refit(&fg_pts);
        bg = bg.refit(&bg_pts);
        let mut g = Graph::new(n, n * 4);
        for (i, l) in labels.iter().enumerate() {
            match *l {
                HARD_FG => g.add_tweights(i, HARD, 0),
                HARD_BG => g.add_tweights(i, 0, HARD),
                _ => {
                    // The source (foreground) link pays −ln p_bg when cut, the sink −ln p_fg.
                    let dbg = bg.neg_log_likelihood(&z[i]);
                    let dfg = fg.neg_log_likelihood(&z[i]);
                    let m = dbg.min(dfg);
                    let s = ((dbg - m).min(MAX_DATA) * SCALE).round() as i64;
                    let t = ((dfg - m).min(MAX_DATA) * SCALE).round() as i64;
                    g.add_tweights(i, s, t);
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                for (d, &(dx, dy, _)) in DIRS.iter().enumerate() {
                    let c = pair[i][d];
                    if c > 0 {
                        let j = ((y as i64 + i64::from(dy)) as usize) * w
                            + (x as i64 + i64::from(dx)) as usize;
                        g.add_edge(i, j, c, c);
                    }
                }
            }
        }
        g.maxflow();
        let mut changed = 0usize;
        for (i, l) in labels.iter_mut().enumerate() {
            if *l == PR_FG || *l == PR_BG {
                let nl = if g.is_source(i) { PR_FG } else { PR_BG };
                if nl != *l {
                    *l = nl;
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            break;
        }
        let (f, b) = split(z, labels);
        if f.is_empty() || b.is_empty() {
            break;
        }
        fg_pts = f;
        bg_pts = b;
    }
    (iterations, Some((fg, bg)))
}

/// Longest side of the coarse level the colour models are first learnt on.
pub const COARSE_SIDE: u32 = 256;
/// Bound on the iterations at the working size after the coarse level.
pub const FINE_ITERATIONS: usize = 2;

/// Segment `img`: the binary keep mask of its working copy. Pictures larger than
/// [`COARSE_SIDE`] are first iterated on a half-size copy (the models converge there
/// at a fraction of the cost); the working size then starts from those labels and
/// models.
pub fn segment(
    img: &Image<'_>,
    req: &Request,
    provider: &dyn SegmentationProvider,
) -> Segmentation {
    let (ww, wh) = work_size(img.width, img.height);
    let work = downscale(img, ww, wh);
    let (w, h) = (ww as usize, wh as usize);
    let view = work.view();
    let mut labels = initial_labels(&view, req, provider);
    let z = colours(&view);
    let mut iterations = 0;
    let mut models = None;
    if ww.max(wh) > COARSE_SIDE {
        let (cw, ch) = ((ww / 2).max(1), (wh / 2).max(1));
        let coarse = downscale(&view, cw, ch);
        let cview = coarse.view();
        let mut cl = initial_labels(&cview, req, provider);
        let cz = colours(&cview);
        let (it, m) = iterate(&cz, cw as usize, ch as usize, &mut cl, None, MAX_ITERATIONS);
        iterations += it;
        if m.is_some() {
            // Probable pixels start from the coarse labelling.
            for y in 0..h {
                let cy = (y * ch as usize / h).min(ch as usize - 1);
                for x in 0..w {
                    let cx = (x * cw as usize / w).min(cw as usize - 1);
                    let l = &mut labels[y * w + x];
                    if *l == PR_FG || *l == PR_BG {
                        *l = if is_fg(cl[cy * cw as usize + cx]) {
                            PR_FG
                        } else {
                            PR_BG
                        };
                    }
                }
            }
            models = m;
        }
    }
    let max = if models.is_some() {
        FINE_ITERATIONS
    } else {
        MAX_ITERATIONS
    };
    let (it, _) = iterate(&z, w, h, &mut labels, models, max);
    iterations += it;
    Segmentation {
        mask: Mask {
            width: ww,
            height: wh,
            alpha: labels
                .iter()
                .map(|l| if is_fg(*l) { 255 } else { 0 })
                .collect(),
        },
        work,
        iterations,
    }
}

/// The keep mask of `seg` at the size of `img` (the picture `seg` was computed from):
/// joint bilateral upsampling guided by the picture's colours, a threshold, then a
/// 3 × 3 box feather (an alpha ramp of 1–2 pixels across the edge).
pub fn full_mask(img: &Image<'_>, seg: &Segmentation) -> Mask {
    let (w, h) = (img.width as usize, img.height as usize);
    let (lw, lh) = (seg.mask.width as usize, seg.mask.height as usize);
    let lo = &seg.mask.alpha;
    let mut bin = vec![0u8; w * h];
    if (w, h) == (lw, lh) {
        bin.copy_from_slice(lo);
    } else {
        let px = |i: usize| -> Rgb {
            let p = &img.rgba[i * 4..i * 4 + 4];
            [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
        };
        let lpx = |i: usize| -> Rgb {
            let p = &seg.work.rgba[i * 4..i * 4 + 4];
            [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
        };
        let (sx, sy) = (lw as f64 / w as f64, lh as f64 / h as f64);
        let inv2s2 = 1.0 / (2.0 * UPSAMPLE_SIGMA * UPSAMPLE_SIGMA);
        for y in 0..h {
            let v = (y as f64 + 0.5) * sy - 0.5;
            let cy = v.round().clamp(0.0, (lh - 1) as f64) as usize;
            for x in 0..w {
                let u = (x as f64 + 0.5) * sx - 0.5;
                let cx = u.round().clamp(0.0, (lw - 1) as f64) as usize;
                let ys = cy.saturating_sub(1)..(cy + 2).min(lh);
                let xs = cx.saturating_sub(1)..(cx + 2).min(lw);
                let first = lo[cy * lw + cx];
                let uniform = ys
                    .clone()
                    .all(|yy| xs.clone().all(|xx| lo[yy * lw + xx] == first));
                let i = y * w + x;
                if uniform {
                    bin[i] = first;
                    continue;
                }
                let c = px(i);
                let (mut num, mut den) = (0.0, 0.0);
                for yy in ys.clone() {
                    for xx in xs.clone() {
                        let q = yy * lw + xx;
                        let ds = (xx as f64 - u).powi(2) + (yy as f64 - v).powi(2);
                        let lc = lpx(q);
                        let dc = (c[0] - lc[0]).powi(2)
                            + (c[1] - lc[1]).powi(2)
                            + (c[2] - lc[2]).powi(2);
                        let wgt = (-ds / (2.0 * 0.64) - dc * inv2s2).exp();
                        num += wgt * f64::from(lo[q]);
                        den += wgt;
                    }
                }
                let a = if den > 1e-12 {
                    num / den
                } else {
                    f64::from(first)
                };
                bin[i] = if a >= 127.5 { 255 } else { 0 };
            }
        }
    }
    Mask {
        width: w as u32,
        height: h as u32,
        alpha: feather(&bin, w, h),
    }
}

/// 3 × 3 box filter (separable, edges clamped).
fn feather(a: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut rows = vec![0u16; w * h];
    for y in 0..h {
        for x in 0..w {
            let l = a[y * w + x.saturating_sub(1)];
            let r = a[y * w + (x + 1).min(w - 1)];
            rows[y * w + x] = u16::from(l) + u16::from(a[y * w + x]) + u16::from(r);
        }
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let t = rows[y.saturating_sub(1) * w + x];
            let b = rows[(y + 1).min(h - 1) * w + x];
            let s = u32::from(t) + u32::from(rows[y * w + x]) + u32::from(b);
            out[y * w + x] = ((s + 4) / 9) as u8;
        }
    }
    out
}

/// Segment and give the full-size keep mask.
pub fn remove_background(
    img: &Image<'_>,
    req: &Request,
    provider: &dyn SegmentationProvider,
) -> (Mask, Segmentation) {
    let seg = segment(img, req, provider);
    (full_mask(img, &seg), seg)
}

/// Multiply the alpha of an RGBA buffer by `mask`.
pub fn apply_mask(rgba: &mut [u8], mask: &Mask) {
    for (p, m) in rgba.chunks_exact_mut(4).zip(&mask.alpha) {
        p[3] = ((u16::from(p[3]) * u16::from(*m) + 127) / 255) as u8;
    }
}

/// Tint of the removed areas in previews.
pub const TINT: [u8; 3] = [0xB4, 0x1E, 0xB4];

/// The working picture with the removed areas tinted (opaque RGBA).
pub fn tinted(seg: &Segmentation) -> OwnedImage {
    let mut out = seg.work.rgba.clone();
    for (p, m) in out.chunks_exact_mut(4).zip(&seg.mask.alpha) {
        let a = f64::from(p[3]) / 255.0;
        for c in 0..3 {
            let v = f64::from(p[c]) * a + 255.0 * (1.0 - a);
            p[c] = if *m < 128 {
                (v * 0.35 + f64::from(TINT[c]) * 0.65).round() as u8
            } else {
                v.round() as u8
            };
        }
        p[3] = 255;
    }
    OwnedImage {
        width: seg.work.width,
        height: seg.work.height,
        rgba: out,
    }
}

#[cfg(test)]
mod tests;

//! Reflection and soft edges of a shape — ECMA-376 Part 1, §20.1.8.50 `reflection` and
//! §20.1.8.53 `softEdge` (children of §20.1.8.26 `effectLst`).
//!
//! The effects stay opaque in the model (`Effect::Other`, written back verbatim); this
//! module reads them and computes the geometry the renderers draw with plain display-list
//! commands:
//!
//! * a reflection is the shape's drawing again under an affine map (scale `sx`/`sy` and
//!   skew `kx`/`ky` about the `algn` point of the shape's box, then moved `dist` along
//!   `dir`), faded by the alpha ramp `stA`@`stPos` → `endA`@`endPos` along `fadeDir`. The
//!   ramp is drawn as bands (clip polygon + constant opacity) across the reflected box,
//!   measured in page space from the side the fade direction starts at;
//! * soft edges fade the outline over `rad` towards the inside: the shape is drawn
//!   [`SOFT_EDGE_STEPS`] times, each copy clipped to the outline inset by a further
//!   `rad / SOFT_EDGE_STEPS` and composited with an opacity chosen so that a point
//!   covered by k copies ends up with opacity k / steps.
//!
//! Band and step counts and the inset-by-scaling (exact for rectangles and ellipses) are
//! BlueOffice's own choices.

use bo_model::slide::XmlElement;

const EMU_PT: f64 = 12_700.0;

/// Number of opacity steps of a soft edge.
pub const SOFT_EDGE_STEPS: usize = 8;
/// Number of bands of a reflection's fade.
pub const REFLECTION_BANDS: usize = 24;

fn num(el: &XmlElement, name: &str, default: f64) -> f64 {
    el.attr(name)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default)
}

/// A §20.1.8.50 reflection, in points / degrees / fractions.
#[derive(Debug, Clone, PartialEq)]
pub struct Reflection {
    /// `blurRad` (points).
    pub blur: f64,
    /// `stA` start opacity (0..=1).
    pub st_a: f64,
    /// `stPos` start position (0..=1).
    pub st_pos: f64,
    /// `endA` end opacity (0..=1).
    pub end_a: f64,
    /// `endPos` end position (0..=1).
    pub end_pos: f64,
    /// `dist` (points).
    pub dist: f64,
    /// `dir` (degrees, clockwise from +x).
    pub dir: f64,
    /// `fadeDir` (degrees).
    pub fade_dir: f64,
    /// `sx` horizontal scale (1 = 100 %).
    pub sx: f64,
    /// `sy` vertical scale (negative: mirrored).
    pub sy: f64,
    /// `kx` horizontal skew (degrees).
    pub kx: f64,
    /// `ky` vertical skew (degrees).
    pub ky: f64,
    /// `algn` (§20.1.10.53 ST_RectAlignment).
    pub algn: String,
}

impl Reflection {
    /// Read a `a:reflection` element; attribute defaults are the schema's
    /// (stA 100 %, endA 0 %, endPos 100 %, fadeDir 90°, sx = sy = 100 %, algn b).
    pub fn parse(el: &XmlElement) -> Reflection {
        Reflection {
            blur: num(el, "blurRad", 0.0) / EMU_PT,
            st_a: num(el, "stA", 100_000.0) / 100_000.0,
            st_pos: num(el, "stPos", 0.0) / 100_000.0,
            end_a: num(el, "endA", 0.0) / 100_000.0,
            end_pos: num(el, "endPos", 100_000.0) / 100_000.0,
            dist: num(el, "dist", 0.0) / EMU_PT,
            dir: num(el, "dir", 0.0) / 60_000.0,
            fade_dir: num(el, "fadeDir", 5_400_000.0) / 60_000.0,
            sx: num(el, "sx", 100_000.0) / 100_000.0,
            sy: num(el, "sy", 100_000.0) / 100_000.0,
            kx: num(el, "kx", 0.0) / 60_000.0,
            ky: num(el, "ky", 0.0) / 60_000.0,
            algn: el.attr("algn").unwrap_or("b").to_owned(),
        }
    }

    /// Affine map `[a, b, c, d, e, f]` (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`) taking the
    /// shape's drawing to its reflection, for the shape's page-space box `[x0, y0, x1, y1]`.
    pub fn transform(&self, bbox: [f64; 4]) -> [f64; 6] {
        let (ax, ay) = align_point(&self.algn, bbox);
        let (tx, ty) = (self.kx.to_radians().tan(), self.ky.to_radians().tan());
        // Scale, then skew: [[1, tx], [ty, 1]] · diag(sx, sy).
        let (a, c, b, d) = (self.sx, tx * self.sy, ty * self.sx, self.sy);
        let r = self.dir.to_radians();
        let (dx, dy) = (self.dist * r.cos(), self.dist * r.sin());
        [
            a,
            b,
            c,
            d,
            ax + dx - (a * ax + c * ay),
            ay + dy - (b * ax + d * ay),
        ]
    }

    /// Opacity of the reflection at position `pos` (0..=1) along the fade.
    pub fn alpha_at(&self, pos: f64) -> f64 {
        let v = if pos <= self.st_pos {
            self.st_a
        } else if pos >= self.end_pos || self.end_pos <= self.st_pos {
            self.end_a
        } else {
            let t = (pos - self.st_pos) / (self.end_pos - self.st_pos);
            self.st_a + (self.end_a - self.st_a) * t
        };
        v.clamp(0.0, 1.0)
    }

    /// Fade bands over the page-space box `[x0, y0, x1, y1]` of the reflected drawing: clip
    /// polygons (extended by `pad` beyond the box) and their opacity. Neighbouring bands of
    /// equal opacity are merged; nearly invisible ones (under 0.5 %) dropped.
    pub fn bands(&self, bbox: [f64; 4], n: usize, pad: f64) -> Vec<(Vec<(f64, f64)>, f64)> {
        let r = self.fade_dir.to_radians();
        let f = (r.cos(), r.sin());
        let q = (-f.1, f.0);
        let corners = [
            (bbox[0], bbox[1]),
            (bbox[2], bbox[1]),
            (bbox[2], bbox[3]),
            (bbox[0], bbox[3]),
        ];
        let proj = |v: (f64, f64)| corners.iter().map(move |c| c.0 * v.0 + c.1 * v.1);
        let smin = proj(f).fold(f64::INFINITY, f64::min);
        let smax = proj(f).fold(f64::NEG_INFINITY, f64::max);
        let qmin = proj(q).fold(f64::INFINITY, f64::min) - pad;
        let qmax = proj(q).fold(f64::NEG_INFINITY, f64::max) + pad;
        let n = n.max(1);
        let len = smax - smin;
        let mut out: Vec<(f64, f64, f64)> = Vec::new();
        for k in 0..n {
            let a = (self.alpha_at((k as f64 + 0.5) / n as f64) * 255.0).round() / 255.0;
            let s0 = if k == 0 {
                smin - pad
            } else {
                smin + len * k as f64 / n as f64
            };
            let s1 = if k + 1 == n {
                smax + pad
            } else {
                smin + len * (k + 1) as f64 / n as f64
            };
            match out.last_mut() {
                Some(l) if (l.2 - a).abs() < 1e-9 => l.1 = s1,
                _ => out.push((s0, s1, a)),
            }
        }
        let pt = |s: f64, t: f64| (s * f.0 + t * q.0, s * f.1 + t * q.1);
        out.into_iter()
            .filter(|b| b.2 >= 0.005)
            .map(|(s0, s1, a)| {
                (
                    vec![pt(s0, qmin), pt(s1, qmin), pt(s1, qmax), pt(s0, qmax)],
                    a,
                )
            })
            .collect()
    }
}

/// Point of a `[x0, y0, x1, y1]` box named by a §20.1.10.53 ST_RectAlignment value.
pub fn align_point(algn: &str, b: [f64; 4]) -> (f64, f64) {
    let cx = f64::midpoint(b[0], b[2]);
    let cy = f64::midpoint(b[1], b[3]);
    match algn {
        "tl" => (b[0], b[1]),
        "t" => (cx, b[1]),
        "tr" => (b[2], b[1]),
        "l" => (b[0], cy),
        "ctr" => (cx, cy),
        "r" => (b[2], cy),
        "bl" => (b[0], b[3]),
        "br" => (b[2], b[3]),
        _ => (cx, b[3]),
    }
}

/// Radius (points) of a §20.1.8.53 `a:softEdge` element.
pub fn soft_edge_radius(el: &XmlElement) -> f64 {
    num(el, "rad", 0.0) / EMU_PT
}

/// Soft-edge steps for a `w × h` shape (local coordinates): for each copy, outermost first,
/// the inset of its clip (points), the local map `[a, b, c, d, e, f]` shrinking the outline
/// by that inset about the centre, and the copy's opacity. A shape thinner than twice the
/// radius fades over (just under) half its smaller side instead.
pub fn soft_edge_steps(rad: f64, w: f64, h: f64) -> Vec<(f64, [f64; 6], f64)> {
    let n = SOFT_EDGE_STEPS;
    let rad = rad.min(w.min(h) / 2.0 * 0.95);
    if rad <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    for k in 1..=n {
        let inset = rad * k as f64 / n as f64;
        let (sw, sh) = (w - 2.0 * inset, h - 2.0 * inset);
        let (sx, sy) = (sw / w.max(1e-9), sh / h.max(1e-9));
        let m = [sx, 0.0, 0.0, sy, w / 2.0 * (1.0 - sx), h / 2.0 * (1.0 - sy)];
        // After k − 1 copies the opacity is (k − 1)/n; one more of 1/(n − k + 1) makes it k/n.
        out.push((inset, m, 1.0 / (n - k + 1) as f64));
    }
    out
}

/// Schema order of `effectLst` children (§20.1.8.26 CT_EffectList).
pub const EFFECT_LST: &[&str] = &[
    "a:blur",
    "a:fillOverlay",
    "a:glow",
    "a:innerShdw",
    "a:outerShdw",
    "a:prstShdw",
    "a:reflection",
    "a:softEdge",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn el(attrs: &[(&str, &str)]) -> XmlElement {
        let mut e = XmlElement::new("a:reflection");
        e.attrs = attrs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        e
    }

    fn apply(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    #[test]
    fn mirrored_copy_below_the_shape() {
        // A typical reflection: stA 52 %, endA 0.3 %, endPos 35 %, 4 pt away straight down,
        // sy −100 % about the bottom-left corner.
        let r = Reflection::parse(&el(&[
            ("blurRad", "6350"),
            ("stA", "52000"),
            ("endA", "300"),
            ("endPos", "35000"),
            ("dist", "50800"),
            ("dir", "5400000"),
            ("sy", "-100000"),
            ("algn", "bl"),
            ("rotWithShape", "0"),
        ]));
        assert!((r.blur - 0.5).abs() < 1e-9 && (r.dist - 4.0).abs() < 1e-9);
        // Shape box (100, 100)–(300, 200): the bottom edge stays put (+4 pt), the top edge
        // flips to 200 + 4 + 100 = 304.
        let m = r.transform([100.0, 100.0, 300.0, 200.0]);
        let (x, y) = apply(m, 100.0, 100.0);
        assert!(
            (x - 100.0).abs() < 1e-9 && (y - 304.0).abs() < 1e-9,
            "{x} {y}"
        );
        let (x, y) = apply(m, 300.0, 200.0);
        assert!(
            (x - 300.0).abs() < 1e-9 && (y - 204.0).abs() < 1e-9,
            "{x} {y}"
        );
        // Alpha ramp: 0.52 at the near edge, linear to 0.003 at 35 %, then 0.003.
        assert!((r.alpha_at(0.0) - 0.52).abs() < 1e-9);
        assert!((r.alpha_at(0.175) - f64::midpoint(0.52, 0.003)).abs() < 1e-9);
        assert!((r.alpha_at(0.35) - 0.003).abs() < 1e-9);
        assert!((r.alpha_at(0.9) - 0.003).abs() < 1e-9);
    }

    #[test]
    fn fade_bands_run_down_the_reflection() {
        let r = Reflection::parse(&el(&[("stA", "50000"), ("endPos", "50000")]));
        // 10 bands over a 100 pt high box (y 0..100): band k has alpha at (k + 0.5)/10.
        let b = r.bands([0.0, 0.0, 50.0, 100.0], 10, 0.0);
        // Bands 0..4 fade 0.5 → 0; bands 5..9 have alpha 0 (dropped).
        assert_eq!(b.len(), 5);
        let ys: Vec<(f64, f64)> = b
            .iter()
            .map(|(p, _)| {
                let lo = p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
                let hi = p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
                (lo, hi)
            })
            .collect();
        assert!((ys[0].0 - 0.0).abs() < 1e-9 && (ys[0].1 - 10.0).abs() < 1e-9);
        assert!((ys[4].0 - 40.0).abs() < 1e-9 && (ys[4].1 - 50.0).abs() < 1e-9);
        // Alpha at 5 %: 0.5 · (1 − 0.05/0.5) = 0.45; at 45 %: 0.05 (rounded to 1/255).
        assert!((b[0].1 - 0.45).abs() < 0.003, "{}", b[0].1);
        assert!((b[4].1 - 0.05).abs() < 0.003, "{}", b[4].1);
    }

    #[test]
    fn soft_edge_insets_and_opacity() {
        let mut e = XmlElement::new("a:softEdge");
        e.attrs.push(("rad".into(), "101600".into()));
        let rad = soft_edge_radius(&e);
        assert!((rad - 8.0).abs() < 1e-9);
        let s = soft_edge_steps(rad, 100.0, 50.0);
        assert_eq!(s.len(), SOFT_EDGE_STEPS);
        // Copy k is inset k pt (8 pt over 8 steps): copy 1 maps (0, 0) to (1, 1) and
        // (100, 50) to (99, 49); the last copy is inset by the full radius.
        let (inset, m, a) = s[0];
        assert!((inset - 1.0).abs() < 1e-9 && (a - 1.0 / 8.0).abs() < 1e-9);
        let (x, y) = apply(m, 0.0, 0.0);
        assert!((x - 1.0).abs() < 1e-9 && (y - 1.0).abs() < 1e-9);
        let (x, y) = apply(m, 100.0, 50.0);
        assert!((x - 99.0).abs() < 1e-9 && (y - 49.0).abs() < 1e-9);
        assert!((s[7].0 - 8.0).abs() < 1e-9 && (s[7].2 - 1.0).abs() < 1e-9);
        // Composited opacity after k copies is k / 8.
        let mut acc = 0.0;
        for (k, (_, _, a)) in s.iter().enumerate() {
            acc += a * (1.0 - acc);
            assert!((acc - (k + 1) as f64 / 8.0).abs() < 1e-9);
        }
        // A 10 pt high shape cannot fade over 8 pt from both sides: the fade stops just
        // short of the middle (0.95 · 5 pt) and still ends fully opaque.
        let thin = soft_edge_steps(rad, 100.0, 10.0);
        assert_eq!(thin.len(), SOFT_EDGE_STEPS);
        assert!((thin[7].0 - 4.75).abs() < 1e-9 && (thin[7].2 - 1.0).abs() < 1e-9);
    }
}

//! SVG → DrawingML shapes (CORE-PIC-001 "convert to shapes"): the basic shapes and
//! paths of an SVG picture (W3C SVG 1.1, chapters 8 Paths and 9 Basic Shapes) become
//! custom-geometry paths (ECMA-376 Part 1, §20.1.9.8 custGeom, §20.1.9.15 path) with
//! their solid fill and stroke.
//!
//! Supported: `path` (all commands, arcs converted to cubic Béziers per SVG 1.1
//! Appendix F.6), `rect` (rounded corners too), `circle`, `ellipse`, `line`,
//! `polyline`, `polygon`, nested `g`/`svg`, `transform` (matrix, translate, scale,
//! rotate, skewX, skewY), the root `viewBox`, and `fill` / `stroke` /
//! `stroke-width` / `opacity` from attributes or `style`. Text, images, gradients,
//! `use` references and clipping are skipped (counted in [`SvgDoc::skipped`]).

use crate::color::preset_color;
use crate::dom::parse;
use bo_model::slide::{GeomPath, PathCommand, XmlElement};

/// Circle/ellipse quarter arc control distance: 4/3·(√2 − 1).
pub const KAPPA: f64 = 0.552_284_749_830_793_4;

/// Path segment in SVG user units (after transforms).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSeg {
    /// Move to (x, y).
    M(f64, f64),
    /// Line to (x, y).
    L(f64, f64),
    /// Cubic Bézier: two control points then the end point.
    C(f64, f64, f64, f64, f64, f64),
    /// Close the subpath.
    Z,
}

/// One converted SVG element.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgShape {
    /// Path segments (document coordinates).
    pub segs: Vec<PathSeg>,
    /// Fill colour `0xRRGGBB` (`None`: not filled).
    pub fill: Option<u32>,
    /// Stroke colour `0xRRGGBB` and width in user units (`None`: no outline).
    pub stroke: Option<(u32, f64)>,
    /// Opacity 0…1 (fill and stroke).
    pub opacity: f64,
}

impl SvgShape {
    /// Bounding box `[x0, y0, x1, y1]` of the points (control points included).
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b: Option<[f64; 4]> = None;
        let mut add = |x: f64, y: f64| {
            let r = b.get_or_insert([x, y, x, y]);
            r[0] = r[0].min(x);
            r[1] = r[1].min(y);
            r[2] = r[2].max(x);
            r[3] = r[3].max(y);
        };
        for s in &self.segs {
            match *s {
                PathSeg::M(x, y) | PathSeg::L(x, y) => add(x, y),
                PathSeg::C(a, b2, c, d, x, y) => {
                    add(a, b2);
                    add(c, d);
                    add(x, y);
                }
                PathSeg::Z => {}
            }
        }
        b
    }

    /// The segments as a §20.1.9.15 path in a coordinate space of `scale` units per
    /// user unit, relative to the box origin (`x0`, `y0`) and sized `w` × `h` user units.
    pub fn geom_path(&self, x0: f64, y0: f64, w: f64, h: f64, scale: f64) -> GeomPath {
        let n = |v: f64| format!("{}", v.round() as i64);
        let px = |x: f64| n((x - x0) * scale);
        let py = |y: f64| n((y - y0) * scale);
        let commands = self
            .segs
            .iter()
            .map(|s| match *s {
                // §20.1.9.16 moveTo, §20.1.9.13 lnTo, §20.1.9.7 cubicBezTo, §20.1.9.4 close
                PathSeg::M(x, y) => PathCommand::MoveTo(px(x), py(y)),
                PathSeg::L(x, y) => PathCommand::LineTo(px(x), py(y)),
                PathSeg::C(a, b, c, d, x, y) => {
                    PathCommand::CubicTo([px(a), py(b), px(c), py(d), px(x), py(y)])
                }
                PathSeg::Z => PathCommand::Close,
            })
            .collect();
        GeomPath {
            w: Some((w * scale).round() as i64),
            h: Some((h * scale).round() as i64),
            fill: self.fill.is_none().then(|| "none".to_owned()),
            stroke: self.stroke.is_none().then_some(false),
            extrusion_ok: None,
            commands,
        }
    }
}

/// A parsed SVG picture.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SvgDoc {
    /// Picture width in user units of the output (after the viewBox mapping).
    pub width: f64,
    /// Picture height.
    pub height: f64,
    /// Converted elements in painting order.
    pub shapes: Vec<SvgShape>,
    /// Number of elements that could not be converted.
    pub skipped: usize,
}

type Mat = [f64; 6];
const ID: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` then `b` (apply `a` first).
fn mul(b: &Mat, a: &Mat) -> Mat {
    [
        b[0] * a[0] + b[2] * a[1],
        b[1] * a[0] + b[3] * a[1],
        b[0] * a[2] + b[2] * a[3],
        b[1] * a[2] + b[3] * a[3],
        b[0] * a[4] + b[2] * a[5] + b[4],
        b[1] * a[4] + b[3] * a[5] + b[5],
    ]
}

fn ap(m: &Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// Numbers of an SVG attribute (comma / whitespace separated, exponents allowed).
pub fn numbers(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' {
            let start = i;
            i += 1;
            let mut dot = c == b'.';
            while i < b.len() {
                let d = b[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == b'.' && !dot {
                    dot = true;
                    i += 1;
                } else if (d == b'e' || d == b'E')
                    && b.get(i + 1)
                        .is_some_and(|n| n.is_ascii_digit() || *n == b'-' || *n == b'+')
                {
                    i += 2;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    break;
                } else {
                    break;
                }
            }
            if let Ok(v) = s[start..i].parse::<f64>() {
                out.push(v);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// SVG 1.1 §7.6 `transform` attribute.
pub fn parse_transform(s: &str) -> Mat {
    let mut m = ID;
    let mut rest = s;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().trim_start_matches(',').trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let v = numbers(&rest[open + 1..open + close]);
        let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
        let t: Mat = match name {
            "matrix" if v.len() >= 6 => [g(0), g(1), g(2), g(3), g(4), g(5)],
            "translate" => [1.0, 0.0, 0.0, 1.0, g(0), g(1)],
            "scale" => {
                let sx = v.first().copied().unwrap_or(1.0);
                [sx, 0.0, 0.0, v.get(1).copied().unwrap_or(sx), 0.0, 0.0]
            }
            "rotate" => {
                let (s, c) = g(0).to_radians().sin_cos();
                let r = [c, s, -s, c, 0.0, 0.0];
                let (cx, cy) = (g(1), g(2));
                mul(
                    &[1.0, 0.0, 0.0, 1.0, cx, cy],
                    &mul(&r, &[1.0, 0.0, 0.0, 1.0, -cx, -cy]),
                )
            }
            "skewX" => [1.0, 0.0, g(0).to_radians().tan(), 1.0, 0.0, 0.0],
            "skewY" => [1.0, g(0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            _ => ID,
        };
        // The list applies right to left: m · t.
        m = mul(&m, &t);
        rest = &rest[open + close + 1..];
    }
    m
}

/// CSS colour: `#rgb`, `#rrggbb`, `rgb(r, g, b)` (numbers or percentages), names.
pub fn parse_color(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        return match h.len() {
            3 => {
                let v = u32::from_str_radix(h, 16).ok()?;
                let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
                Some((r * 17) << 16 | (g * 17) << 8 | (b * 17))
            }
            6 => u32::from_str_radix(h, 16).ok(),
            _ => None,
        };
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return None;
        }
        let mut c = 0u32;
        for p in parts {
            let v = if let Some(pc) = p.strip_suffix('%') {
                pc.trim().parse::<f64>().ok()? * 255.0 / 100.0
            } else {
                p.parse::<f64>().ok()?
            };
            c = (c << 8) | v.round().clamp(0.0, 255.0) as u32;
        }
        return Some(c);
    }
    let lower = s.to_ascii_lowercase();
    // CSS colour keywords; DrawingML preset colours (§20.1.10.48) share the names.
    preset_color(&lower).or_else(|| {
        let first = lower.get(..1)?.to_owned();
        preset_color(&(first + lower.get(1..)?))
    })
}

#[derive(Debug, Clone, Copy)]
struct Style {
    fill: Option<u32>,
    stroke: Option<u32>,
    stroke_width: f64,
    opacity: f64,
}

fn style_value<'a>(el: &'a XmlElement, name: &str) -> Option<&'a str> {
    let from_style = el.attr("style").and_then(|st| {
        st.split(';').find_map(|d| {
            let (k, v) = d.split_once(':')?;
            (k.trim() == name).then_some(v.trim())
        })
    });
    from_style.or_else(|| el.attr(name))
}

fn paint(v: &str, current: Option<u32>) -> Option<u32> {
    match v.trim() {
        "none" | "transparent" => None,
        "currentColor" => current,
        other => parse_color(other).or(current),
    }
}

fn inherit(el: &XmlElement, parent: &Style) -> Style {
    let mut s = *parent;
    if let Some(v) = style_value(el, "fill") {
        s.fill = paint(v, parent.fill);
    }
    if let Some(v) = style_value(el, "stroke") {
        s.stroke = paint(v, parent.stroke);
    }
    if let Some(w) = style_value(el, "stroke-width").and_then(|v| numbers(v).first().copied()) {
        s.stroke_width = w;
    }
    if let Some(o) = style_value(el, "opacity").and_then(|v| v.trim().parse::<f64>().ok()) {
        s.opacity *= o.clamp(0.0, 1.0);
    }
    s
}

fn num(el: &XmlElement, name: &str) -> f64 {
    el.attr(name)
        .and_then(|v| numbers(v).first().copied())
        .unwrap_or(0.0)
}

/// Endpoint arc (SVG 1.1 Appendix F.6.5) as cubic Béziers (≤ 90° each), appended to `out`.
#[allow(clippy::too_many_arguments)]
fn arc(
    out: &mut Vec<PathSeg>,
    (x1, y1): (f64, f64),
    mut rx: f64,
    mut ry: f64,
    phi_deg: f64,
    large: bool,
    sweep: bool,
    (x2, y2): (f64, f64),
) {
    if (x1 - x2).abs() < 1e-12 && (y1 - y2).abs() < 1e-12 {
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    if rx < 1e-12 || ry < 1e-12 {
        out.push(PathSeg::L(x2, y2));
        return;
    }
    let (sp, cp) = phi_deg.to_radians().sin_cos();
    // Step 1: (x1', y1').
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cp * dx + sp * dy;
    let y1p = -sp * dx + cp * dy;
    // F.6.6: scale radii up when too small.
    let lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lam > 1.0 {
        let k = lam.sqrt();
        rx *= k;
        ry *= k;
    }
    // Step 2: centre (cx', cy').
    let num = (rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p).max(0.0);
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut co = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        co = -co;
    }
    let cxp = co * rx * y1p / ry;
    let cyp = -co * ry * x1p / rx;
    // Step 3: centre.
    let cx = cp * cxp - sp * cyp + f64::midpoint(x1, x2);
    let cy = sp * cxp + cp * cyp + f64::midpoint(y1, y2);
    // Step 4: angles.
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dth = ang(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && dth > 0.0 {
        dth -= std::f64::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f64::consts::TAU;
    }
    let n = (dth.abs() / std::f64::consts::FRAC_PI_2 - 1e-9)
        .ceil()
        .max(1.0) as usize;
    let step = dth / n as f64;
    let t = 4.0 / 3.0 * (step / 4.0).tan();
    let pt = |a: f64| {
        let (s, c) = a.sin_cos();
        (
            cx + rx * c * cp - ry * s * sp,
            cy + rx * c * sp + ry * s * cp,
        )
    };
    let dpt = |a: f64| {
        let (s, c) = a.sin_cos();
        (-rx * s * cp - ry * c * sp, -rx * s * sp + ry * c * cp)
    };
    let mut a = th1;
    for i in 0..n {
        let b = a + step;
        let (p0x, p0y) = pt(a);
        let (d0x, d0y) = dpt(a);
        let (p1x, p1y) = if i + 1 == n { (x2, y2) } else { pt(b) };
        let (d1x, d1y) = dpt(b);
        out.push(PathSeg::C(
            p0x + t * d0x,
            p0y + t * d0y,
            p1x - t * d1x,
            p1y - t * d1y,
            p1x,
            p1y,
        ));
        a = b;
    }
}

/// SVG 1.1 §8.3 path data → segments (untransformed).
pub fn parse_path(d: &str) -> Vec<PathSeg> {
    // Tokenize into command letters and numbers (flags may be packed: "a1 1 0 01 1 1").
    #[derive(Clone, Copy)]
    enum Tok {
        Cmd(u8),
        Num(f64),
    }
    let mut toks = Vec::new();
    let b = d.as_bytes();
    let mut i = 0;
    let mut cmd = 0u8;
    let mut arg = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() && c != b'e' && c != b'E' {
            toks.push(Tok::Cmd(c));
            cmd = c.to_ascii_lowercase();
            arg = 0;
            i += 1;
        } else if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' {
            // Arc flags (4th and 5th argument of `a`) are single digits.
            if cmd == b'a' && (arg % 7 == 3 || arg % 7 == 4) && (c == b'0' || c == b'1') {
                toks.push(Tok::Num(f64::from(c - b'0')));
                arg += 1;
                i += 1;
                continue;
            }
            let start = i;
            i += 1;
            let mut dot = c == b'.';
            while i < b.len() {
                let x = b[i];
                if x.is_ascii_digit() {
                    i += 1;
                } else if x == b'.' && !dot {
                    dot = true;
                    i += 1;
                } else if (x == b'e' || x == b'E')
                    && b.get(i + 1)
                        .is_some_and(|n| n.is_ascii_digit() || *n == b'-' || *n == b'+')
                {
                    i += 2;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    break;
                } else {
                    break;
                }
            }
            if let Ok(v) = d[start..i].parse::<f64>() {
                toks.push(Tok::Num(v));
                arg += 1;
            }
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    // Last control point (cubic / quadratic) for the smooth commands.
    let mut last_c: Option<(f64, f64)> = None;
    let mut last_q: Option<(f64, f64)> = None;
    let mut k = 0;
    let mut cur = b'M';
    let take = |k: &mut usize, n: usize| -> Option<Vec<f64>> {
        let mut v = Vec::with_capacity(n);
        for j in 0..n {
            match toks.get(*k + j) {
                Some(Tok::Num(f)) => v.push(*f),
                _ => return None,
            }
        }
        *k += n;
        Some(v)
    };
    while k < toks.len() {
        if let Tok::Cmd(c) = toks[k] {
            cur = c;
            k += 1;
            if c == b'Z' || c == b'z' {
                out.push(PathSeg::Z);
                x = sx;
                y = sy;
                last_c = None;
                last_q = None;
                continue;
            }
        } else if cur == b'Z' || cur == b'z' {
            k += 1;
            continue;
        }
        let rel = cur.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        let mut lc = None;
        let mut lq = None;
        match cur.to_ascii_uppercase() {
            b'M' => {
                let Some(v) = take(&mut k, 2) else { break };
                x = ox + v[0];
                y = oy + v[1];
                sx = x;
                sy = y;
                out.push(PathSeg::M(x, y));
                // Further pairs are implicit line-tos.
                cur = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let Some(v) = take(&mut k, 2) else { break };
                x = ox + v[0];
                y = oy + v[1];
                out.push(PathSeg::L(x, y));
            }
            b'H' => {
                let Some(v) = take(&mut k, 1) else { break };
                x = ox + v[0];
                out.push(PathSeg::L(x, y));
            }
            b'V' => {
                let Some(v) = take(&mut k, 1) else { break };
                y = oy + v[0];
                out.push(PathSeg::L(x, y));
            }
            b'C' => {
                let Some(v) = take(&mut k, 6) else { break };
                let (c2x, c2y) = (ox + v[2], oy + v[3]);
                out.push(PathSeg::C(
                    ox + v[0],
                    oy + v[1],
                    c2x,
                    c2y,
                    ox + v[4],
                    oy + v[5],
                ));
                x = ox + v[4];
                y = oy + v[5];
                lc = Some((c2x, c2y));
            }
            b'S' => {
                let Some(v) = take(&mut k, 4) else { break };
                let (c1x, c1y) = last_c.map_or((x, y), |(px, py)| (2.0 * x - px, 2.0 * y - py));
                let (c2x, c2y) = (ox + v[0], oy + v[1]);
                out.push(PathSeg::C(c1x, c1y, c2x, c2y, ox + v[2], oy + v[3]));
                x = ox + v[2];
                y = oy + v[3];
                lc = Some((c2x, c2y));
            }
            b'Q' | b'T' => {
                let smooth = cur.eq_ignore_ascii_case(&b't');
                let (qx, qy, ex, ey) = if smooth {
                    let Some(v) = take(&mut k, 2) else { break };
                    let (qx, qy) = last_q.map_or((x, y), |(px, py)| (2.0 * x - px, 2.0 * y - py));
                    (qx, qy, ox + v[0], oy + v[1])
                } else {
                    let Some(v) = take(&mut k, 4) else { break };
                    (ox + v[0], oy + v[1], ox + v[2], oy + v[3])
                };
                // Quadratic → cubic: control points 2/3 of the way to the quadratic one.
                out.push(PathSeg::C(
                    x + 2.0 / 3.0 * (qx - x),
                    y + 2.0 / 3.0 * (qy - y),
                    ex + 2.0 / 3.0 * (qx - ex),
                    ey + 2.0 / 3.0 * (qy - ey),
                    ex,
                    ey,
                ));
                x = ex;
                y = ey;
                lq = Some((qx, qy));
            }
            b'A' => {
                let Some(v) = take(&mut k, 7) else { break };
                let end = (ox + v[5], oy + v[6]);
                arc(
                    &mut out,
                    (x, y),
                    v[0],
                    v[1],
                    v[2],
                    v[3] != 0.0,
                    v[4] != 0.0,
                    end,
                );
                (x, y) = end;
            }
            _ => {
                k += 1;
            }
        }
        last_c = lc;
        last_q = lq;
    }
    out
}

fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<PathSeg> {
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    vec![
        PathSeg::M(cx + rx, cy),
        PathSeg::C(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        PathSeg::C(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        PathSeg::C(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        PathSeg::C(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        PathSeg::Z,
    ]
}

fn rect(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Vec<PathSeg> {
    if rx <= 0.0 || ry <= 0.0 {
        return vec![
            PathSeg::M(x, y),
            PathSeg::L(x + w, y),
            PathSeg::L(x + w, y + h),
            PathSeg::L(x, y + h),
            PathSeg::Z,
        ];
    }
    let (rx, ry) = (rx.min(w / 2.0), ry.min(h / 2.0));
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    vec![
        PathSeg::M(x + rx, y),
        PathSeg::L(x + w - rx, y),
        PathSeg::C(x + w - rx + kx, y, x + w, y + ry - ky, x + w, y + ry),
        PathSeg::L(x + w, y + h - ry),
        PathSeg::C(
            x + w,
            y + h - ry + ky,
            x + w - rx + kx,
            y + h,
            x + w - rx,
            y + h,
        ),
        PathSeg::L(x + rx, y + h),
        PathSeg::C(x + rx - kx, y + h, x, y + h - ry + ky, x, y + h - ry),
        PathSeg::L(x, y + ry),
        PathSeg::C(x, y + ry - ky, x + rx - kx, y, x + rx, y),
        PathSeg::Z,
    ]
}

fn points(s: &str, close: bool) -> Vec<PathSeg> {
    let v = numbers(s);
    let mut out = Vec::new();
    for (i, p) in v.chunks_exact(2).enumerate() {
        out.push(if i == 0 {
            PathSeg::M(p[0], p[1])
        } else {
            PathSeg::L(p[0], p[1])
        });
    }
    if close && !out.is_empty() {
        out.push(PathSeg::Z);
    }
    out
}

fn transformed(segs: Vec<PathSeg>, m: &Mat) -> Vec<PathSeg> {
    segs.into_iter()
        .map(|s| match s {
            PathSeg::M(x, y) => {
                let (x, y) = ap(m, x, y);
                PathSeg::M(x, y)
            }
            PathSeg::L(x, y) => {
                let (x, y) = ap(m, x, y);
                PathSeg::L(x, y)
            }
            PathSeg::C(a, b, c, d, x, y) => {
                let (a, b) = ap(m, a, b);
                let (c, d) = ap(m, c, d);
                let (x, y) = ap(m, x, y);
                PathSeg::C(a, b, c, d, x, y)
            }
            PathSeg::Z => PathSeg::Z,
        })
        .collect()
}

fn walk(el: &XmlElement, m: &Mat, st: &Style, doc: &mut SvgDoc) {
    for c in el.elements() {
        let tm = c.attr("transform").map_or(ID, parse_transform);
        let m2 = mul(m, &tm);
        let st2 = inherit(c, st);
        let segs = match c.local() {
            "g" | "svg" | "a" | "switch" => {
                walk(c, &m2, &st2, doc);
                continue;
            }
            "path" => parse_path(c.attr("d").unwrap_or_default()),
            "rect" => {
                let (rx, ry) = match (c.attr("rx"), c.attr("ry")) {
                    (Some(_), None) => (num(c, "rx"), num(c, "rx")),
                    (None, Some(_)) => (num(c, "ry"), num(c, "ry")),
                    _ => (num(c, "rx"), num(c, "ry")),
                };
                rect(
                    num(c, "x"),
                    num(c, "y"),
                    num(c, "width"),
                    num(c, "height"),
                    rx,
                    ry,
                )
            }
            "circle" => {
                let r = num(c, "r");
                ellipse(num(c, "cx"), num(c, "cy"), r, r)
            }
            "ellipse" => ellipse(num(c, "cx"), num(c, "cy"), num(c, "rx"), num(c, "ry")),
            "line" => vec![
                PathSeg::M(num(c, "x1"), num(c, "y1")),
                PathSeg::L(num(c, "x2"), num(c, "y2")),
            ],
            "polyline" => points(c.attr("points").unwrap_or_default(), false),
            "polygon" => points(c.attr("points").unwrap_or_default(), true),
            "defs" | "title" | "desc" | "metadata" | "style" | "clipPath" | "mask"
            | "linearGradient" | "radialGradient" | "pattern" | "symbol" | "marker" => continue,
            _ => {
                doc.skipped += 1;
                continue;
            }
        };
        if segs.is_empty() {
            continue;
        }
        let open_line = matches!(c.local(), "line" | "polyline");
        // Stroke width scales with the transform (mean of the axis scales).
        let k = ((m2[0] * m2[3] - m2[1] * m2[2]).abs()).sqrt();
        let shape = SvgShape {
            segs: transformed(segs, &m2),
            fill: if open_line { None } else { st2.fill },
            stroke: st2.stroke.map(|c| (c, st2.stroke_width * k)),
            opacity: st2.opacity,
        };
        if shape.fill.is_some() || shape.stroke.is_some() {
            doc.shapes.push(shape);
        }
    }
}

/// Parse SVG bytes (`None` when not well-formed XML or not an `svg` root).
pub fn parse_svg(bytes: &[u8]) -> Option<SvgDoc> {
    // Drop a DOCTYPE declaration (internal subsets are not supported by the parser).
    let text = std::str::from_utf8(bytes).ok()?;
    let cleaned;
    let text = if let Some(i) = text.find("<!DOCTYPE") {
        let end = text[i..].find('>')? + i + 1;
        cleaned = format!("{}{}", &text[..i], &text[end..]);
        cleaned.as_str()
    } else {
        text
    };
    let root = parse(text.as_bytes(), "svg").ok()?;
    if root.local() != "svg" {
        return None;
    }
    let vb = root
        .attr("viewBox")
        .map(numbers)
        .filter(|v| v.len() == 4 && v[2] > 0.0 && v[3] > 0.0);
    let len = |n: &str| {
        root.attr(n)
            .filter(|v| !v.trim().ends_with('%'))
            .and_then(|v| numbers(v).first().copied())
            .filter(|v| *v > 0.0)
    };
    let (w, h) = match (len("width"), len("height"), &vb) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some(v)) => (w, w * v[3] / v[2]),
        (None, Some(h), Some(v)) => (h * v[2] / v[3], h),
        (_, _, Some(v)) => (v[2], v[3]),
        _ => (300.0, 150.0),
    };
    // viewBox → viewport (preserveAspectRatio xMidYMid meet, SVG 1.1 §7.8).
    let m = match &vb {
        Some(v) => {
            let s = (w / v[2]).min(h / v[3]);
            let tx = (w - v[2] * s) / 2.0 - v[0] * s;
            let ty = (h - v[3] * s) / 2.0 - v[1] * s;
            [s, 0.0, 0.0, s, tx, ty]
        }
        None => ID,
    };
    let base = Style {
        fill: Some(0),
        stroke: None,
        stroke_width: 1.0,
        opacity: 1.0,
    };
    let st = inherit(&root, &base);
    let mut doc = SvgDoc {
        width: w,
        height: h,
        ..SvgDoc::default()
    };
    walk(&root, &m, &st, &mut doc);
    Some(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(p: &GeomPath) -> Vec<String> {
        p.commands
            .iter()
            .map(|c| match c {
                PathCommand::MoveTo(x, y) => format!("M{x},{y}"),
                PathCommand::LineTo(x, y) => format!("L{x},{y}"),
                PathCommand::CubicTo(v) => format!("C{}", v.join(",")),
                PathCommand::Close => "Z".into(),
                _ => "?".into(),
            })
            .collect()
    }

    #[test]
    fn rect_to_path() {
        let d = parse_svg(br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect x="10" y="20" width="30" height="40" fill="#ff0000"/></svg>"##).unwrap();
        assert_eq!((d.width, d.height), (100.0, 50.0));
        let s = &d.shapes[0];
        assert_eq!(s.fill, Some(0xFF_0000));
        assert_eq!(s.bounds(), Some([10.0, 20.0, 40.0, 60.0]));
        // Path space: 10 units per user unit, relative to the box origin (10, 20).
        let p = s.geom_path(10.0, 20.0, 30.0, 40.0, 10.0);
        assert_eq!((p.w, p.h), (Some(300), Some(400)));
        assert_eq!(cmds(&p), ["M0,0", "L300,0", "L300,400", "L0,400", "Z"]);
        assert_eq!(p.stroke, Some(false));
        assert_eq!(p.fill, None);
    }

    #[test]
    fn circle_to_beziers() {
        // r = 10 at (10, 10): kappa·10 = 5.5228 → 5.52 at scale 1, 55.23 → 55 at scale 10.
        let d = parse_svg(br#"<svg width="20" height="20"><circle cx="10" cy="10" r="10" fill="blue" stroke="black" stroke-width="2"/></svg>"#).unwrap();
        let s = &d.shapes[0];
        assert_eq!(s.fill, Some(0x00_00FF));
        assert_eq!(s.stroke, Some((0, 2.0)));
        let p = s.geom_path(0.0, 0.0, 20.0, 20.0, 10.0);
        // Centre (100, 100), r = 100, k = 55.23 → 155 / 45 after rounding.
        assert_eq!(
            cmds(&p),
            [
                "M200,100",
                "C200,155,155,200,100,200",
                "C45,200,0,155,0,100",
                "C0,45,45,0,100,0",
                "C155,0,200,45,200,100",
                "Z"
            ]
        );
    }

    #[test]
    fn path_commands_and_relative() {
        let segs = parse_path("M10 10 L20 10 l0 10 h-10 z C 0 0, 5 5, 10 10 S 20 20 30 30");
        assert_eq!(
            segs,
            vec![
                PathSeg::M(10.0, 10.0),
                PathSeg::L(20.0, 10.0),
                PathSeg::L(20.0, 20.0),
                PathSeg::L(10.0, 20.0),
                PathSeg::Z,
                // After z the current point is the subpath start (10, 10).
                PathSeg::C(0.0, 0.0, 5.0, 5.0, 10.0, 10.0),
                // S reflects (5, 5) about (10, 10) → (15, 15).
                PathSeg::C(15.0, 15.0, 20.0, 20.0, 30.0, 30.0),
            ]
        );
        // Implicit line-tos after a move; quadratic raised to cubic (2/3 rule):
        // from (0,0) control (30,0) to (30,30): c1 = (20, 0), c2 = (30, 10).
        let q = parse_path("m0,0 10,0Q30 0 30 30");
        assert_eq!(
            q,
            vec![
                PathSeg::M(0.0, 0.0),
                PathSeg::L(10.0, 0.0),
                PathSeg::C(
                    10.0 + 2.0 / 3.0 * 20.0,
                    0.0,
                    30.0,
                    30.0 - 2.0 / 3.0 * 30.0,
                    30.0,
                    30.0
                ),
            ]
        );
    }

    #[test]
    fn arc_half_circle() {
        // Half circle radius 10 from (0,0) to (20,0), sweep = 1 (clockwise on screen,
        // through (10, −10)): two quarter arcs.
        let segs = parse_path("M0 0 A10 10 0 0 1 20 0");
        assert_eq!(segs.len(), 3);
        let PathSeg::C(a, b, c, d, x, y) = segs[1] else {
            panic!()
        };
        // First quarter: (0,0) → (10,−10); controls (0, −k·10), (10 − k·10, −10).
        let k = KAPPA * 10.0;
        for (got, want) in [
            (a, 0.0),
            (b, -k),
            (c, 10.0 - k),
            (d, -10.0),
            (x, 10.0),
            (y, -10.0),
        ] {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
        // Second quarter: (10,−10) → (20,0); controls (10 + k·10, −10), (20, −k·10).
        let PathSeg::C(a, b, c, d, x, y) = segs[2] else {
            panic!()
        };
        for (got, want) in [
            (a, 10.0 + k),
            (b, -10.0),
            (c, 20.0),
            (d, -k),
            (x, 20.0),
            (y, 0.0),
        ] {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
    }

    #[test]
    fn transforms_viewbox_and_styles() {
        let d = parse_svg(br##"<svg viewBox="0 0 10 10" width="100" height="100" fill="#00ff00"><g transform="translate(1,2) scale(2)"><polygon points="0,0 2,0 2,1"/></g><line x1="0" y1="0" x2="10" y2="0" style="stroke:#123456;stroke-width:0.5"/><text>x</text></svg>"##).unwrap();
        assert_eq!(d.skipped, 1);
        assert_eq!(d.shapes.len(), 2);
        // (2,0) → scale 2 → (4,0) → translate → (5,2) → viewBox ×10 → (50,20).
        assert_eq!(d.shapes[0].segs[1], PathSeg::L(50.0, 20.0));
        assert_eq!(d.shapes[0].fill, Some(0x00_FF00));
        assert_eq!(d.shapes[1].fill, None);
        assert_eq!(d.shapes[1].stroke, Some((0x12_3456, 5.0)));
        assert_eq!(parse_transform("rotate(90)"), {
            let (s, c) = 90f64.to_radians().sin_cos();
            [c, s, -s, c, 0.0, 0.0]
        });
        assert_eq!(parse_color("#abc"), Some(0xAA_BBCC));
        assert_eq!(parse_color("rgb(255, 0, 50%)"), Some(0xFF_0080));
    }
}

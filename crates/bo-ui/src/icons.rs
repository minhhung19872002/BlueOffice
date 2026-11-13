//! The interface icon set, shared with the React interface: `assets/icons/ui-icons.json`
//! (exported from `packages/ui/src/icons` by a test) parsed into display-list paths.
use crate::Painter;
use bo_render::{PathSeg, Rgba};
use serde::Deserialize;
use std::collections::HashMap;

const ICONS: &str = include_str!("../../../assets/icons/ui-icons.json");

#[derive(Deserialize)]
struct Part {
    d: String,
    #[serde(default)]
    fill: bool,
    stroke: String,
    #[serde(default)]
    accent: bool,
}

#[derive(Deserialize)]
struct File {
    grid: f32,
    stroke: f32,
    icons: HashMap<String, Vec<Part>>,
}

struct Shape {
    segs: Vec<PathSeg>,
    fill: bool,
    /// `None`: no stroke; `Some(true)`: the colour on an accent fill.
    stroke: Option<bool>,
    accent: bool,
}

/// Parsed icons by command id.
pub struct Icons {
    grid: f32,
    stroke: f32,
    shapes: HashMap<String, Vec<Shape>>,
}

/// Colours an icon is drawn with.
#[derive(Debug, Clone, Copy)]
pub struct IconColors {
    /// Strokes and fills in the icon colour.
    pub ink: Rgba,
    /// Accent parts.
    pub accent: Rgba,
    /// Strokes drawn on an accent fill.
    pub on: Rgba,
}

impl Icons {
    /// Load the shared icon set.
    pub fn load() -> Self {
        let file: File = serde_json::from_str(ICONS).unwrap_or(File {
            grid: 24.0,
            stroke: 1.5,
            icons: HashMap::new(),
        });
        let shapes = file
            .icons
            .into_iter()
            .map(|(name, parts)| {
                let shapes = parts
                    .into_iter()
                    .map(|p| Shape {
                        segs: parse_path(&p.d),
                        fill: p.fill,
                        stroke: match p.stroke.as_str() {
                            "none" => None,
                            "on" => Some(true),
                            _ => Some(false),
                        },
                        accent: p.accent,
                    })
                    .collect();
                (name, shapes)
            })
            .collect();
        Self {
            grid: file.grid,
            stroke: file.stroke,
            shapes,
        }
    }

    /// Whether `name` has an icon.
    pub fn has(&self, name: &str) -> bool {
        self.shapes.contains_key(name)
    }

    /// Draw icon `name` in a `size`-pixel square at (`x`, `y`).
    pub fn draw(&self, p: &mut dyn Painter, name: &str, x: f32, y: f32, size: f32, c: IconColors) {
        let Some(shapes) = self.shapes.get(name) else {
            return;
        };
        let k = size / self.grid;
        // Keep the 1.5 / 24 stroke proportion, but never thinner than one device pixel.
        let width = (self.stroke * k).max(1.0);
        for shape in shapes {
            let segs: Vec<PathSeg> = shape.segs.iter().map(|s| transform(*s, k, x, y)).collect();
            let base = if shape.accent { c.accent } else { c.ink };
            let fill = shape.fill.then_some(base);
            let stroke = shape.stroke.map(|on| (if on { c.on } else { base }, width));
            p.path(&segs, fill, stroke);
        }
    }
}

fn transform(s: PathSeg, k: f32, dx: f32, dy: f32) -> PathSeg {
    let t = |x: f32, y: f32| (x * k + dx, y * k + dy);
    match s {
        PathSeg::M(x, y) => {
            let (x, y) = t(x, y);
            PathSeg::M(x, y)
        }
        PathSeg::L(x, y) => {
            let (x, y) = t(x, y);
            PathSeg::L(x, y)
        }
        PathSeg::Q(a, b, x, y) => {
            let ((a, b), (x, y)) = (t(a, b), t(x, y));
            PathSeg::Q(a, b, x, y)
        }
        PathSeg::C(a, b, c, d, x, y) => {
            let ((a, b), (c, d), (x, y)) = (t(a, b), t(c, d), t(x, y));
            PathSeg::C(a, b, c, d, x, y)
        }
        PathSeg::Z => PathSeg::Z,
    }
}

/// Cursor over SVG path data (SVG 1.1 §8.3.9 grammar: compact numbers, packed arc flags).
struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn skip(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b',' | b'\t' | b'\n' | b'\r')
        {
            self.i += 1;
        }
    }

    fn command(&mut self) -> Option<char> {
        self.skip();
        let c = *self.b.get(self.i)? as char;
        (c.is_ascii_alphabetic() && c != 'e' && c != 'E').then(|| {
            self.i += 1;
            c
        })
    }

    fn at_number(&mut self) -> bool {
        self.skip();
        self.b
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
    }

    fn number(&mut self) -> Option<f32> {
        if !self.at_number() {
            return None;
        }
        let start = self.i;
        if matches!(self.b[self.i], b'-' | b'+') {
            self.i += 1;
        }
        let mut dot = false;
        while let Some(&c) = self.b.get(self.i) {
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !dot {
                dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E')
                && self
                    .b
                    .get(self.i + 1)
                    .is_some_and(|n| n.is_ascii_digit() || matches!(n, b'-' | b'+'))
            {
                self.i += 2;
                while self.b.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
                break;
            } else {
                break;
            }
        }
        std::str::from_utf8(&self.b[start..self.i])
            .ok()?
            .parse()
            .ok()
    }

    /// One arc flag: a single `0` or `1` character.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let c = *self.b.get(self.i)?;
        self.i += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

/// Parse SVG path data into display-list segments (arcs become cubic Béziers).
pub fn parse_path(d: &str) -> Vec<PathSeg> {
    let mut out = Vec::new();
    let mut lx = Lexer {
        b: d.as_bytes(),
        i: 0,
    };
    let (mut cx, mut cy, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    // Last control points, for the smooth S / T forms.
    let mut last_c: Option<(f32, f32)> = None;
    let mut last_q: Option<(f32, f32)> = None;
    let Some(mut cmd) = lx.command() else {
        return out;
    };
    loop {
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        let ok = (|| -> Option<()> {
            match cmd.to_ascii_uppercase() {
                'Z' => {
                    out.push(PathSeg::Z);
                    (cx, cy) = (sx, sy);
                    (last_c, last_q) = (None, None);
                }
                'M' => {
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    (sx, sy) = (cx, cy);
                    out.push(PathSeg::M(cx, cy));
                    // Further pairs after a move are line-tos.
                    cmd = if rel { 'l' } else { 'L' };
                    (last_c, last_q) = (None, None);
                }
                'L' => {
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    out.push(PathSeg::L(cx, cy));
                    (last_c, last_q) = (None, None);
                }
                'H' => {
                    cx = lx.number()? + ox;
                    out.push(PathSeg::L(cx, cy));
                    (last_c, last_q) = (None, None);
                }
                'V' => {
                    cy = lx.number()? + oy;
                    out.push(PathSeg::L(cx, cy));
                    (last_c, last_q) = (None, None);
                }
                'C' => {
                    let (x1, y1) = (lx.number()? + ox, lx.number()? + oy);
                    let (x2, y2) = (lx.number()? + ox, lx.number()? + oy);
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    out.push(PathSeg::C(x1, y1, x2, y2, cx, cy));
                    (last_c, last_q) = (Some((x2, y2)), None);
                }
                'S' => {
                    let (x1, y1) =
                        last_c.map_or((cx, cy), |(px, py)| (2.0 * cx - px, 2.0 * cy - py));
                    let (x2, y2) = (lx.number()? + ox, lx.number()? + oy);
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    out.push(PathSeg::C(x1, y1, x2, y2, cx, cy));
                    (last_c, last_q) = (Some((x2, y2)), None);
                }
                'Q' => {
                    let (x1, y1) = (lx.number()? + ox, lx.number()? + oy);
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    out.push(PathSeg::Q(x1, y1, cx, cy));
                    (last_c, last_q) = (None, Some((x1, y1)));
                }
                'T' => {
                    let (x1, y1) =
                        last_q.map_or((cx, cy), |(px, py)| (2.0 * cx - px, 2.0 * cy - py));
                    (cx, cy) = (lx.number()? + ox, lx.number()? + oy);
                    out.push(PathSeg::Q(x1, y1, cx, cy));
                    (last_c, last_q) = (None, Some((x1, y1)));
                }
                'A' => {
                    let (rx, ry, rot) = (lx.number()?, lx.number()?, lx.number()?);
                    let (large, sweep) = (lx.flag()?, lx.flag()?);
                    let (x, y) = (lx.number()? + ox, lx.number()? + oy);
                    arc(&mut out, (cx, cy), (rx, ry, rot), (large, sweep), (x, y));
                    (cx, cy) = (x, y);
                    (last_c, last_q) = (None, None);
                }
                _ => return None,
            }
            Some(())
        })();
        if ok.is_none() {
            break;
        }
        // Repeated parameters continue the same command; otherwise read the next letter.
        if cmd.eq_ignore_ascii_case(&'z') || !lx.at_number() {
            match lx.command() {
                Some(c) => cmd = c,
                None => break,
            }
        }
    }
    out
}

/// SVG elliptical arc (endpoint parameterisation, SVG 1.1 §F.6.5) as cubic segments.
fn arc(
    out: &mut Vec<PathSeg>,
    (x1, y1): (f32, f32),
    (rx, ry, rot): (f32, f32, f32),
    (large, sweep): (bool, bool),
    (x2, y2): (f32, f32),
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || (x1 == x2 && y1 == y2) {
        out.push(PathSeg::L(x2, y2));
        return;
    }
    let phi = rot.to_radians();
    let (sin, cos) = phi.sin_cos();
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let (cxp, cyp) = (coef * rx * y1p / ry, -coef * ry * x1p / rx);
    let (cx, cy) = (
        cos * cxp - sin * cyp + f32::midpoint(x1, x2),
        sin * cxp + cos * cyp + f32::midpoint(y1, y2),
    );
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }
    let pieces = (delta.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / pieces as f32;
    let t = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |a: f32| {
        let (s, c) = a.sin_cos();
        (
            cx + rx * c * cos - ry * s * sin,
            cy + rx * c * sin + ry * s * cos,
        )
    };
    let deriv = |a: f32| {
        let (s, c) = a.sin_cos();
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };
    let mut a = theta1;
    for i in 0..pieces {
        let b = a + step;
        let (p0, p3) = (point(a), point(b));
        let (d0, d3) = (deriv(a), deriv(b));
        let end = if i + 1 == pieces { (x2, y2) } else { p3 };
        out.push(PathSeg::C(
            p0.0 + t * d0.0,
            p0.1 + t * d0.1,
            p3.0 - t * d3.0,
            p3.1 - t * d3.1,
            end.0,
            end.1,
        ));
        a = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_svg_paths_with_arcs() {
        // Lucide's bold: relative arcs with packed flags ("010 8").
        let segs = parse_path("M6 12h9a4 4 0 010 8H7a1 1 0 01-1-1V5a1 1 0 011-1h7a4 4 0 010 8");
        assert!(matches!(segs[0], PathSeg::M(6.0, 12.0)));
        assert!(matches!(segs[1], PathSeg::L(15.0, 12.0)));
        // The half circle ends at (15, 20).
        let PathSeg::C(.., x, y) = segs[3] else {
            panic!("{segs:?}")
        };
        assert!(
            (x - 15.0).abs() < 1e-3 && (y - 20.0).abs() < 1e-3,
            "{x} {y}"
        );
        let PathSeg::C(.., x, y) = *segs.last().unwrap() else {
            panic!()
        };
        assert!(
            (x - 14.0).abs() < 1e-3 && (y - 12.0).abs() < 1e-3,
            "{x} {y}"
        );
    }

    #[test]
    fn every_exported_icon_parses() {
        let icons = Icons::load();
        assert!(icons.shapes.len() > 300);
        for (name, shapes) in &icons.shapes {
            for s in shapes {
                assert!(!s.segs.is_empty(), "{name}");
                assert!(matches!(s.segs[0], PathSeg::M(..)), "{name}");
            }
        }
        assert!(icons.has("format.bold") && icons.has("edit.undo"));
    }
}

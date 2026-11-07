//! Shape geometry engine — ECMA-376 Part 1, §20.1.9 (Shape Definitions and
//! Attributes) and the guide formula language of §20.1.9.11 (`gd@fmla`).
//!
//! Preset shapes (§20.1.9.18 prstGeom) are described in [`crate::presets`] in a
//! compact text form transcribed from the presetShapeDefinitions annex and are
//! evaluated by the same code as custom geometry (§20.1.9.8 custGeom).

use bo_model::slide::{CustomGeometry, GeomPath, Geometry, Guide, PathCommand};
use std::collections::HashMap;
use std::f64::consts::PI;

/// Path segment in shape coordinates (EMU or any unit the caller chose for w/h).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

/// §20.1.10.37 ST_PathFillMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    None,
    Norm,
    Lighten,
    LightenLess,
    Darken,
    DarkenLess,
}

/// One evaluated path of a geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapePath {
    pub segs: Vec<Seg>,
    pub fill: FillMode,
    pub stroke: bool,
}

/// Evaluated geometry: paths plus the text rectangle (l, t, r, b).
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluated {
    pub paths: Vec<ShapePath>,
    pub text_rect: [f64; 4],
}

impl Evaluated {
    /// Bounding box of all path points (control points included), `[minx, miny, maxx, maxy]`.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b: Option<[f64; 4]> = None;
        let mut add = |x: f64, y: f64| {
            let r = b.get_or_insert([x, y, x, y]);
            r[0] = r[0].min(x);
            r[1] = r[1].min(y);
            r[2] = r[2].max(x);
            r[3] = r[3].max(y);
        };
        for p in &self.paths {
            for s in &p.segs {
                match *s {
                    Seg::Move(x, y) | Seg::Line(x, y) => add(x, y),
                    Seg::Quad(a, b2, x, y) => {
                        add(a, b2);
                        add(x, y);
                    }
                    Seg::Cubic(a, b2, c, d, x, y) => {
                        add(a, b2);
                        add(c, d);
                        add(x, y);
                    }
                    Seg::Close => {}
                }
            }
        }
        b
    }
}

/// Guide evaluation error (unknown guide, malformed formula).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeomError(pub String);

impl std::fmt::Display for GeomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Angle unit: 60 000ths of a degree → radians.
fn ang(v: f64) -> f64 {
    v / 60_000.0 * PI / 180.0
}

/// Guide variables.
#[derive(Debug, Clone)]
pub struct Guides {
    vars: HashMap<String, f64>,
}

impl Guides {
    /// Built-in guides for a shape of size w × h (§20.1.9.11, "built-in shape guides").
    pub fn new(w: f64, h: f64) -> Self {
        let ss = w.min(h);
        let ls = w.max(h);
        let mut vars = HashMap::new();
        let mut set = |k: &str, v: f64| {
            vars.insert(k.to_owned(), v);
        };
        set("l", 0.0);
        set("t", 0.0);
        set("r", w);
        set("b", h);
        set("w", w);
        set("h", h);
        set("hc", w / 2.0);
        set("vc", h / 2.0);
        set("ss", ss);
        set("ls", ls);
        for d in [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 32.0] {
            set(&format!("wd{d}"), w / d);
            set(&format!("hd{d}"), h / d);
            set(&format!("ssd{d}"), ss / d);
        }
        set("cd2", 10_800_000.0);
        set("cd4", 5_400_000.0);
        set("cd8", 2_700_000.0);
        set("3cd4", 16_200_000.0);
        set("3cd8", 8_100_000.0);
        set("5cd8", 13_500_000.0);
        set("7cd8", 18_900_000.0);
        Guides { vars }
    }

    /// Value of an argument: guide name or numeric literal.
    pub fn arg(&self, a: &str) -> Result<f64, GeomError> {
        if let Some(v) = self.vars.get(a) {
            return Ok(*v);
        }
        a.parse::<f64>()
            .map_err(|_| GeomError(format!("unknown guide '{a}'")))
    }

    /// Value of a guide.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.vars.get(name).copied()
    }

    /// Evaluate a formula (§20.1.9.11 `fmla`).
    pub fn eval(&self, fmla: &str) -> Result<f64, GeomError> {
        let mut it = fmla.split_whitespace();
        let op = it.next().ok_or_else(|| GeomError("empty formula".into()))?;
        let args: Vec<f64> = it.map(|a| self.arg(a)).collect::<Result<_, _>>()?;
        let need = |n: usize| {
            if args.len() < n {
                Err(GeomError(format!("formula '{fmla}' needs {n} arguments")))
            } else {
                Ok(())
            }
        };
        let v = match op {
            // Multiply divide: x·y/z
            "*/" => {
                need(3)?;
                if args[2] == 0.0 {
                    0.0
                } else {
                    args[0] * args[1] / args[2]
                }
            }
            // Add subtract: x + y − z
            "+-" => {
                need(3)?;
                args[0] + args[1] - args[2]
            }
            // Add divide: (x + y)/z
            "+/" => {
                need(3)?;
                if args[2] == 0.0 {
                    0.0
                } else {
                    (args[0] + args[1]) / args[2]
                }
            }
            // If else: x > 0 ? y : z
            "?:" => {
                need(3)?;
                if args[0] > 0.0 {
                    args[1]
                } else {
                    args[2]
                }
            }
            "abs" => {
                need(1)?;
                args[0].abs()
            }
            // ArcTan: atan2(y, x) in 60000ths of a degree
            "at2" => {
                need(2)?;
                args[1].atan2(args[0]) * 180.0 / PI * 60_000.0
            }
            // Cosine ArcTan: x·cos(atan2(z, y))
            "cat2" => {
                need(3)?;
                args[0] * args[2].atan2(args[1]).cos()
            }
            "cos" => {
                need(2)?;
                args[0] * ang(args[1]).cos()
            }
            "max" => {
                need(2)?;
                args[0].max(args[1])
            }
            "min" => {
                need(2)?;
                args[0].min(args[1])
            }
            // Modulo: √(x² + y² + z²)
            "mod" => {
                need(3)?;
                (args[0] * args[0] + args[1] * args[1] + args[2] * args[2]).sqrt()
            }
            // Pin: clamp y to [x, z]
            "pin" => {
                need(3)?;
                if args[1] < args[0] {
                    args[0]
                } else if args[1] > args[2] {
                    args[2]
                } else {
                    args[1]
                }
            }
            // Sine ArcTan: x·sin(atan2(z, y))
            "sat2" => {
                need(3)?;
                args[0] * args[2].atan2(args[1]).sin()
            }
            "sin" => {
                need(2)?;
                args[0] * ang(args[1]).sin()
            }
            "sqrt" => {
                need(1)?;
                args[0].max(0.0).sqrt()
            }
            "tan" => {
                need(2)?;
                args[0] * ang(args[1]).tan()
            }
            "val" => {
                need(1)?;
                args[0]
            }
            _ => return Err(GeomError(format!("unknown formula operator '{op}'"))),
        };
        Ok(if v.is_finite() { v } else { 0.0 })
    }

    /// Evaluate and define a guide.
    pub fn define(&mut self, g: &Guide) -> Result<(), GeomError> {
        let v = self.eval(&g.fmla)?;
        self.vars.insert(g.name.clone(), v);
        Ok(())
    }
}

/// Append an elliptical arc (§20.1.9.3 arcTo) to `segs` as cubic Béziers.
///
/// The pen is on the ellipse (radii `wr`, `hr`) at angle `st`; the arc sweeps
/// `sw` (both in 60000ths of a degree, clockwise in y-down space). Angles are
/// visual angles from the centre, converted to the ellipse parameter.
fn arc_to(segs: &mut Vec<Seg>, pen: &mut (f64, f64), wr: f64, hr: f64, st: f64, sw: f64) {
    let to_param = |a: f64| -> f64 {
        // Point at visual angle a: tan(t) = (wr/hr)·tan(a).
        (wr * a.sin()).atan2(hr * a.cos())
    };
    let st_r = ang(st);
    let sw_r = ang(sw);
    let t0 = to_param(st_r);
    let mut t1 = to_param(st_r + sw_r);
    // Keep the sweep direction and number of turns of the visual sweep.
    let full = 2.0 * PI;
    if sw_r.abs() >= full - 1e-9 {
        t1 = t0 + full * sw_r.signum();
    } else if sw_r > 0.0 {
        while t1 <= t0 {
            t1 += full;
        }
        while t1 - t0 > full {
            t1 -= full;
        }
    } else if sw_r < 0.0 {
        while t1 >= t0 {
            t1 -= full;
        }
        while t0 - t1 > full {
            t1 += full;
        }
    } else {
        return;
    }
    let cx = pen.0 - wr * t0.cos();
    let cy = pen.1 - hr * t0.sin();
    let total = t1 - t0;
    let n = (total.abs() / (PI / 2.0)).ceil().max(1.0) as usize;
    let step = total / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut a = t0;
    for _ in 0..n {
        let b = a + step;
        let (sa, ca) = a.sin_cos();
        let (sb, cb) = b.sin_cos();
        let p1 = (cx + wr * (ca - k * sa), cy + hr * (sa + k * ca));
        let p2 = (cx + wr * (cb + k * sb), cy + hr * (sb - k * cb));
        let p3 = (cx + wr * cb, cy + hr * sb);
        segs.push(Seg::Cubic(p1.0, p1.1, p2.0, p2.1, p3.0, p3.1));
        a = b;
    }
    *pen = (cx + wr * t1.cos(), cy + hr * t1.sin());
}

fn eval_path(p: &GeomPath, g: &Guides, w: f64, h: f64) -> Result<ShapePath, GeomError> {
    // §20.1.9.15: path w/h define the coordinate space of this path.
    let sx = match p.w {
        Some(pw) if pw > 0 => w / pw as f64,
        _ => 1.0,
    };
    let sy = match p.h {
        Some(ph) if ph > 0 => h / ph as f64,
        _ => 1.0,
    };
    let x = |a: &str| g.arg(a).map(|v| v * sx);
    let y = |a: &str| g.arg(a).map(|v| v * sy);
    let mut segs = Vec::new();
    let mut pen = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for c in &p.commands {
        match c {
            PathCommand::MoveTo(a, b) => {
                pen = (x(a)?, y(b)?);
                start = pen;
                segs.push(Seg::Move(pen.0, pen.1));
            }
            PathCommand::LineTo(a, b) => {
                pen = (x(a)?, y(b)?);
                segs.push(Seg::Line(pen.0, pen.1));
            }
            PathCommand::ArcTo {
                w_r,
                h_r,
                st_ang,
                sw_ang,
            } => {
                if segs.is_empty() {
                    segs.push(Seg::Move(pen.0, pen.1));
                }
                arc_to(
                    &mut segs,
                    &mut pen,
                    x(w_r)?,
                    y(h_r)?,
                    g.arg(st_ang)?,
                    g.arg(sw_ang)?,
                );
            }
            PathCommand::QuadTo(p) => {
                let s = Seg::Quad(x(&p[0])?, y(&p[1])?, x(&p[2])?, y(&p[3])?);
                pen = (x(&p[2])?, y(&p[3])?);
                segs.push(s);
            }
            PathCommand::CubicTo(p) => {
                let s = Seg::Cubic(
                    x(&p[0])?,
                    y(&p[1])?,
                    x(&p[2])?,
                    y(&p[3])?,
                    x(&p[4])?,
                    y(&p[5])?,
                );
                pen = (x(&p[4])?, y(&p[5])?);
                segs.push(s);
            }
            PathCommand::Close => {
                segs.push(Seg::Close);
                pen = start;
            }
        }
    }
    let fill = match p.fill.as_deref() {
        Some("none") => FillMode::None,
        Some("lighten") => FillMode::Lighten,
        Some("lightenLess") => FillMode::LightenLess,
        Some("darken") => FillMode::Darken,
        Some("darkenLess") => FillMode::DarkenLess,
        _ => FillMode::Norm,
    };
    Ok(ShapePath {
        segs,
        fill,
        stroke: p.stroke.unwrap_or(true),
    })
}

/// Evaluate a custom geometry for a shape of size `w × h`, with adjust-value
/// overrides `av` (names matching the definition's avLst).
pub fn eval_custom(
    def: &CustomGeometry,
    av: &[Guide],
    w: f64,
    h: f64,
) -> Result<Evaluated, GeomError> {
    let mut g = Guides::new(w, h);
    for d in &def.av {
        match av.iter().find(|o| o.name == d.name) {
            Some(o) => g.define(o)?,
            None => g.define(d)?,
        }
    }
    for d in &def.gd {
        g.define(d)?;
    }
    let paths = def
        .paths
        .iter()
        .map(|p| eval_path(p, &g, w, h))
        .collect::<Result<Vec<_>, _>>()?;
    let text_rect = match &def.rect {
        Some([l, t, r, b]) => [g.arg(l)?, g.arg(t)?, g.arg(r)?, g.arg(b)?],
        None => [0.0, 0.0, w, h],
    };
    Ok(Evaluated { paths, text_rect })
}

/// Evaluate a preset by name; `None` for unknown presets.
pub fn eval_preset(
    name: &str,
    av: &[Guide],
    w: f64,
    h: f64,
) -> Option<Result<Evaluated, GeomError>> {
    let def = crate::presets::preset(name)?;
    Some(eval_custom(&def, av, w, h))
}

/// Plain rectangle geometry.
pub fn rect_geometry(w: f64, h: f64) -> Evaluated {
    Evaluated {
        paths: vec![ShapePath {
            segs: vec![
                Seg::Move(0.0, 0.0),
                Seg::Line(w, 0.0),
                Seg::Line(w, h),
                Seg::Line(0.0, h),
                Seg::Close,
            ],
            fill: FillMode::Norm,
            stroke: true,
        }],
        text_rect: [0.0, 0.0, w, h],
    }
}

/// Evaluate any geometry; unknown presets and evaluation errors fall back to a
/// rectangle and push a warning.
pub fn evaluate(geom: Option<&Geometry>, w: f64, h: f64, warnings: &mut Vec<String>) -> Evaluated {
    let res = match geom {
        None => return rect_geometry(w, h),
        Some(Geometry::Preset { name, av }) => {
            let Some(r) = eval_preset(name, av, w, h) else {
                warnings.push(format!("unknown preset geometry '{name}', drawn as rect"));
                return rect_geometry(w, h);
            };
            r
        }
        Some(Geometry::Custom(c)) => eval_custom(c, &[], w, h),
    };
    match res {
        Ok(e) => e,
        Err(e) => {
            warnings.push(format!("geometry error: {e}; drawn as rect"));
            rect_geometry(w, h)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas() {
        let mut g = Guides::new(200.0, 100.0);
        let e = |g: &Guides, f: &str| g.eval(f).unwrap();
        assert_eq!(e(&g, "*/ w 3 4"), 150.0);
        assert_eq!(e(&g, "+- w 10 h"), 110.0);
        assert_eq!(e(&g, "+/ w h 2"), 150.0);
        assert_eq!(e(&g, "?: -1 1 2"), 2.0);
        assert_eq!(e(&g, "abs -5"), 5.0);
        assert!((e(&g, "at2 1 1") - 2_700_000.0).abs() < 1e-6);
        assert!((e(&g, "cos 100 cd4")).abs() < 1e-9);
        assert!((e(&g, "sin 100 cd4") - 100.0).abs() < 1e-9);
        assert!((e(&g, "cat2 10 1 0") - 10.0).abs() < 1e-9);
        assert!((e(&g, "sat2 10 0 1") - 10.0).abs() < 1e-9);
        assert_eq!(e(&g, "max w h"), 200.0);
        assert_eq!(e(&g, "min w h"), 100.0);
        assert_eq!(e(&g, "mod 3 4 0"), 5.0);
        assert_eq!(e(&g, "pin 0 150 100"), 100.0);
        assert_eq!(e(&g, "pin 0 -3 100"), 0.0);
        assert_eq!(e(&g, "sqrt 16"), 4.0);
        assert!((e(&g, "tan 10 cd8") - 10.0).abs() < 1e-9);
        assert_eq!(e(&g, "val 7"), 7.0);
        assert_eq!(e(&g, "*/ ss 1 2"), 50.0);
        g.define(&Guide {
            name: "x".into(),
            fmla: "*/ w 1 4".into(),
        })
        .unwrap();
        assert_eq!(e(&g, "+- x 0 0"), 50.0);
        assert!(g.eval("foo 1").is_err());
        assert!(g.eval("+- nope 0 0").is_err());
    }

    #[test]
    fn quarter_arc_is_exact_at_ends() {
        let mut segs = vec![Seg::Move(0.0, 50.0)];
        let mut pen = (0.0, 50.0);
        // From 180° sweeping +90° on a 100×50 ellipse: ends at the top (50, 0).
        arc_to(&mut segs, &mut pen, 50.0, 50.0, 10_800_000.0, 5_400_000.0);
        assert!((pen.0 - 50.0).abs() < 1e-9 && pen.1.abs() < 1e-9, "{pen:?}");
        assert_eq!(segs.len(), 2);
    }
}

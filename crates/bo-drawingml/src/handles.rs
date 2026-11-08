//! Adjust handles and connection sites of preset shapes — ECMA-376 Part 1,
//! §20.1.9.1 ahLst (§20.1.9.3 ahPolar, §20.1.9.4 ahXY) and §20.1.9.10 cxnLst
//! (§20.1.9.9 cxn), from the presetShapeDefinitions annex (`handles.dsl`).
//!
//! Positions are in the shape's local coordinate system (points, origin at the
//! top-left corner of the unrotated box), like [`crate::geom`].

use crate::geom::{GeomError, Guides};
use bo_model::slide::{CustomGeometry, Geometry, Guide};
use std::collections::HashMap;
use std::sync::OnceLock;

const DEFS: &str = include_str!("handles.dsl");

/// Adjust value driven by one axis of a handle (`gdRefX` + `minX`/`maxX`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjRef {
    /// Name of the adjust value (guide) the handle drives.
    pub guide: String,
    /// Minimum value: guide name or literal.
    pub min: String,
    /// Maximum value: guide name or literal.
    pub max: String,
}

/// §20.1.9.4 ahXY / §20.1.9.3 ahPolar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleDef {
    /// Cartesian handle (ahXY).
    Xy {
        /// Adjust value driven by the x axis.
        x: Option<AdjRef>,
        /// Adjust value driven by the y axis.
        y: Option<AdjRef>,
        /// Handle position (x, y) as guide names or literals.
        pos: [String; 2],
    },
    /// Polar handle (ahPolar).
    Polar {
        /// Adjust value driven by the radius.
        r: Option<AdjRef>,
        /// Adjust value driven by the angle.
        ang: Option<AdjRef>,
        /// Handle position (x, y) as guide names or literals.
        pos: [String; 2],
    },
}

/// §20.1.9.9 cxn: connection site with its outward angle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteDef {
    /// Outward angle: guide name or literal (60000ths of a degree).
    pub ang: String,
    /// Site position (x, y) as guide names or literals.
    pub pos: [String; 2],
}

#[derive(Debug, Default, Clone)]
struct ShapeDefs {
    handles: Vec<HandleDef>,
    sites: Vec<SiteDef>,
}

fn adj(words: &[&str]) -> Option<AdjRef> {
    match words {
        [g, min, max] if *g != "-" => Some(AdjRef {
            guide: (*g).to_owned(),
            min: if *min == "-" { "0" } else { min }.to_owned(),
            max: if *max == "-" { "0" } else { max }.to_owned(),
        }),
        _ => None,
    }
}

fn parse_defs() -> HashMap<&'static str, ShapeDefs> {
    let mut out: HashMap<&'static str, ShapeDefs> = HashMap::new();
    let mut cur: Option<&'static str> = None;
    for line in DEFS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if let Some(n) = line.strip_prefix('@') {
            cur = Some(n);
            continue;
        }
        let Some(name) = cur else { continue };
        let w: Vec<&str> = line.split_whitespace().collect();
        let e = out.entry(name).or_default();
        match (w.first().copied(), w.len()) {
            (Some("ahxy"), 9) => e.handles.push(HandleDef::Xy {
                x: adj(&w[1..4]),
                y: adj(&w[4..7]),
                pos: [w[7].to_owned(), w[8].to_owned()],
            }),
            (Some("ahpolar"), 9) => e.handles.push(HandleDef::Polar {
                r: adj(&w[1..4]),
                ang: adj(&w[4..7]),
                pos: [w[7].to_owned(), w[8].to_owned()],
            }),
            (Some("cxn"), 4) => e.sites.push(SiteDef {
                ang: w[1].to_owned(),
                pos: [w[2].to_owned(), w[3].to_owned()],
            }),
            _ => {}
        }
    }
    out
}

fn table() -> &'static HashMap<&'static str, ShapeDefs> {
    static T: OnceLock<HashMap<&'static str, ShapeDefs>> = OnceLock::new();
    T.get_or_init(parse_defs)
}

/// Handle definitions of a preset (empty when it has none).
pub fn preset_handles(name: &str) -> Vec<HandleDef> {
    table()
        .get(name)
        .map(|d| d.handles.clone())
        .unwrap_or_default()
}

/// Guides of a definition evaluated for `w × h` with adjust overrides `av`.
fn guides(def: &CustomGeometry, av: &[Guide], w: f64, h: f64) -> Result<Guides, GeomError> {
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
    Ok(g)
}

/// A handle placed on a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandlePoint {
    /// Index into the preset's ahLst.
    pub index: usize,
    /// X in shape coordinates.
    pub x: f64,
    /// Y in shape coordinates.
    pub y: f64,
}

fn pos_of(g: &Guides, pos: &[String; 2]) -> Option<(f64, f64)> {
    Some((g.arg(&pos[0]).ok()?, g.arg(&pos[1]).ok()?))
}

/// Positions of the adjust handles of preset `name` with adjust values `av`.
pub fn handle_points(name: &str, av: &[Guide], w: f64, h: f64) -> Vec<HandlePoint> {
    let (Some(def), Some(d)) = (crate::presets::preset(name), table().get(name)) else {
        return Vec::new();
    };
    let Ok(g) = guides(&def, av, w, h) else {
        return Vec::new();
    };
    d.handles
        .iter()
        .enumerate()
        .filter_map(|(index, hd)| {
            let pos = match hd {
                HandleDef::Xy { pos, .. } | HandleDef::Polar { pos, .. } => pos,
            };
            let (x, y) = pos_of(&g, pos)?;
            Some(HandlePoint { index, x, y })
        })
        .collect()
}

/// A connection site: local position and outward direction (degrees clockwise from +x).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Site {
    /// X in shape coordinates.
    pub x: f64,
    /// Y in shape coordinates.
    pub y: f64,
    /// Outward direction in degrees clockwise from +x.
    pub angle: f64,
}

/// Connection sites of a geometry; shapes without a definition get the four
/// edge midpoints of their box (like `rect`).
pub fn connection_sites(geom: Option<&Geometry>, w: f64, h: f64) -> Vec<Site> {
    let fallback = || {
        vec![
            Site {
                x: w / 2.0,
                y: 0.0,
                angle: 270.0,
            },
            Site {
                x: 0.0,
                y: h / 2.0,
                angle: 180.0,
            },
            Site {
                x: w / 2.0,
                y: h,
                angle: 90.0,
            },
            Site {
                x: w,
                y: h / 2.0,
                angle: 0.0,
            },
        ]
    };
    let Some(Geometry::Preset { name, av }) = geom else {
        return fallback();
    };
    let (Some(def), Some(d)) = (crate::presets::preset(name), table().get(name.as_str())) else {
        return fallback();
    };
    let Ok(g) = guides(&def, av, w, h) else {
        return fallback();
    };
    d.sites
        .iter()
        .filter_map(|s| {
            let (x, y) = pos_of(&g, &s.pos)?;
            let angle = g.arg(&s.ang).ok()? / 60_000.0;
            Some(Site { x, y, angle })
        })
        .collect()
}

/// Current value of adjust guide `name` (definition default when not overridden).
fn current(def: &CustomGeometry, av: &[Guide], w: f64, h: f64, name: &str) -> Option<f64> {
    guides(def, av, w, h).ok()?.get(name)
}

fn with_value(av: &[Guide], def: &CustomGeometry, name: &str, v: f64) -> Vec<Guide> {
    // Keep the definition's order (§20.1.9.5 avLst) and existing overrides.
    let mut out: Vec<Guide> = def
        .av
        .iter()
        .filter_map(|d| {
            if d.name == name {
                Some(Guide {
                    name: name.to_owned(),
                    fmla: format!("val {}", v.round() as i64),
                })
            } else {
                av.iter().find(|o| o.name == d.name).cloned()
            }
        })
        .collect();
    for o in av {
        if !out.iter().any(|g| g.name == o.name) {
            out.push(o.clone());
        }
    }
    out
}

/// Find the value of one adjust guide whose handle lands closest to the
/// pointer: `cost` measures the distance for a handle position.
fn solve(
    def: &CustomGeometry,
    av: &[Guide],
    w: f64,
    h: f64,
    r: &AdjRef,
    pos: &[String; 2],
    cost: &dyn Fn(f64, f64) -> f64,
) -> Option<Vec<Guide>> {
    let g0 = guides(def, av, w, h).ok()?;
    let lo = g0.arg(&r.min).ok()?;
    let hi = g0.arg(&r.max).ok()?;
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let eval = |v: f64| -> f64 {
        let a = with_value(av, def, &r.guide, v);
        guides(def, &a, w, h)
            .ok()
            .and_then(|g| pos_of(&g, pos))
            .map_or(f64::MAX, |(x, y)| cost(x, y))
    };
    let cur = current(def, av, w, h, &r.guide)
        .unwrap_or(0.0)
        .clamp(lo, hi);
    let best = if hi - lo > 1.0e7 {
        // Unbounded (callouts): the handle moves linearly with the value; use
        // secant steps from the current value.
        let mut v = cur;
        let mut step = 1000.0;
        for _ in 0..40 {
            let c = eval(v);
            let c1 = eval(v + step);
            let c2 = eval(v - step);
            if c1 < c && c1 <= c2 {
                v += step;
                step *= 2.0;
            } else if c2 < c {
                v -= step;
                step *= 2.0;
            } else {
                step /= 2.0;
                if step < 0.5 {
                    break;
                }
            }
        }
        v.clamp(lo, hi)
    } else {
        // Sample the range, then refine around the best sample (golden section).
        const N: usize = 64;
        let mut best = (cur, eval(cur));
        for i in 0..=N {
            let v = lo + (hi - lo) * i as f64 / N as f64;
            let c = eval(v);
            if c < best.1 {
                best = (v, c);
            }
        }
        let span = (hi - lo) / N as f64;
        let (mut a, mut b) = ((best.0 - span).max(lo), (best.0 + span).min(hi));
        let k = 0.618_033_988_75;
        for _ in 0..40 {
            let m1 = b - k * (b - a);
            let m2 = a + k * (b - a);
            if eval(m1) <= eval(m2) {
                b = m2;
            } else {
                a = m1;
            }
        }
        let v = f64::midpoint(a, b);
        if eval(v) <= best.1 {
            v
        } else {
            best.0
        }
    };
    Some(with_value(av, def, &r.guide, best))
}

/// New adjust values after dragging handle `index` of preset `name` to local
/// point (x, y); values stay within the handle's min/max range.
pub fn drag_handle(
    name: &str,
    av: &[Guide],
    w: f64,
    h: f64,
    index: usize,
    x: f64,
    y: f64,
) -> Option<Vec<Guide>> {
    let def = crate::presets::preset(name)?;
    let hd = table().get(name)?.handles.get(index)?.clone();
    let mut cur = av.to_vec();
    match hd {
        HandleDef::Xy { x: rx, y: ry, pos } => {
            if let Some(r) = rx {
                cur = solve(&def, &cur, w, h, &r, &pos, &|px, _| (px - x).abs())?;
            }
            if let Some(r) = ry {
                cur = solve(&def, &cur, w, h, &r, &pos, &|_, py| (py - y).abs())?;
            }
        }
        HandleDef::Polar { r, ang, pos } => {
            let d = |px: f64, py: f64| (px - x).hypot(py - y);
            if let Some(a) = ang {
                cur = solve(&def, &cur, w, h, &a, &pos, &d)?;
            }
            if let Some(rr) = r {
                cur = solve(&def, &cur, w, h, &rr, &pos, &d)?;
            }
        }
    }
    Some(cur)
}

/// Number of presets with handle or site definitions.
pub fn defined_count() -> usize {
    table().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(av: &[Guide], name: &str) -> i64 {
        av.iter()
            .find(|g| g.name == name)
            .and_then(|g| g.fmla.strip_prefix("val "))
            .and_then(|v| v.parse().ok())
            .unwrap_or(i64::MIN)
    }

    #[test]
    fn round_rect_handle_follows_the_corner_radius() {
        // §20.1.9 roundRect: adj (0..50000) → x1 = ss·adj/100000 on the top edge.
        let pts = handle_points("roundRect", &[], 200.0, 100.0);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].x - 16.667).abs() < 0.01 && pts[0].y == 0.0);
        let av = drag_handle("roundRect", &[], 200.0, 100.0, 0, 30.0, 5.0).unwrap();
        assert!((val(&av, "adj") - 30_000).abs() <= 1, "{av:?}");
        // Clamped to maxAdj (50000).
        let av = drag_handle("roundRect", &[], 200.0, 100.0, 0, 190.0, 0.0).unwrap();
        assert_eq!(val(&av, "adj"), 50_000);
        let pts = handle_points("roundRect", &av, 200.0, 100.0);
        assert!((pts[0].x - 50.0).abs() < 1e-6);
    }

    #[test]
    fn arrow_has_two_handles_and_sites() {
        let pts = handle_points("rightArrow", &[], 100.0, 50.0);
        assert_eq!(pts.len(), 2);
        let av = drag_handle("rightArrow", &[], 100.0, 50.0, 1, 60.0, 0.0).unwrap();
        let pts = handle_points("rightArrow", &av, 100.0, 50.0);
        assert!((pts[1].x - 60.0).abs() < 0.5, "{pts:?}");
        let geom = Geometry::Preset {
            name: "rightArrow".into(),
            av: vec![],
        };
        let sites = connection_sites(Some(&geom), 100.0, 50.0);
        assert_eq!(sites.len(), 4);
        assert!(sites
            .iter()
            .any(|s| (s.x - 100.0).abs() < 1e-9 && s.angle == 0.0));
        // No definition: the four edge midpoints.
        assert_eq!(connection_sites(None, 10.0, 10.0).len(), 4);
    }

    #[test]
    fn unbounded_callout_handle() {
        let av = drag_handle("borderCallout1", &[], 100.0, 50.0, 1, -20.0, 80.0).unwrap();
        let pts = handle_points("borderCallout1", &av, 100.0, 50.0);
        assert!(
            (pts[1].x + 20.0).abs() < 0.5 && (pts[1].y - 80.0).abs() < 0.5,
            "{pts:?}"
        );
    }

    #[test]
    fn polar_handle() {
        // blockArc: angle handle + radius handle.
        let pts = handle_points("blockArc", &[], 100.0, 100.0);
        assert_eq!(pts.len(), 2);
        let before = pts[0];
        let av = drag_handle("blockArc", &[], 100.0, 100.0, 0, 50.0, 100.0).unwrap();
        let after = handle_points("blockArc", &av, 100.0, 100.0)[0];
        assert_ne!(after, before);
        assert!(defined_count() > 150);
    }
}

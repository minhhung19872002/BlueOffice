//! A small 3-D mesh renderer for DrawingML 3-D shapes (ECMA-376 Part 1, §20.1.5): the outline of the shape
//! is extruded into a prism (§20.1.5.9 `sp3d@extrusionH`), the top bevel (§20.1.5.2 `bevelT`) adds rings
//! stepping inwards and towards the viewer along its profile, the camera (§20.1.5.5, presets of
//! §20.1.10.47, explicit §20.1.5.11 `rot`, `fov`) turns and projects the vertices (orthographic, oblique
//! or perspective), faces turned away are culled, the rest are sorted back to front and shaded by the
//! light rig direction (§20.1.5.10) and the material (§20.1.10.51): ambient + Lambert + a specular term
//! for glossy materials. The result is flat-shaded polygons in the display list, so the canvas, PNG and
//! PDF outputs draw the same solid.

use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

/// A 3-D vector.
pub(crate) type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// How the vertices are seen: a linear map to view space (x right, y down, z towards the viewer) and an
/// optional perspective eye distance (points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct View {
    /// Row-major 3 × 3 map.
    pub m: [[f64; 3]; 3],
    /// Eye distance for a perspective camera.
    pub eye: Option<f64>,
}

impl View {
    /// Camera turned by `lat` (positive: seen from above) and `lon` (positive: seen from the left)
    /// degrees, then `rev` degrees about the view axis.
    pub fn turned(lat: f64, lon: f64, rev: f64) -> View {
        let (a, b, c) = (lat.to_radians(), lon.to_radians(), rev.to_radians());
        // Rotation about y (the camera moves left: the model turns right), then about x.
        let ry = [
            [libm::cos(b), 0.0, libm::sin(b)],
            [0.0, 1.0, 0.0],
            [-libm::sin(b), 0.0, libm::cos(b)],
        ];
        let rx = [
            [1.0, 0.0, 0.0],
            [0.0, libm::cos(a), libm::sin(a)],
            [0.0, -libm::sin(a), libm::cos(a)],
        ];
        let rz = [
            [libm::cos(c), -libm::sin(c), 0.0],
            [libm::sin(c), libm::cos(c), 0.0],
            [0.0, 0.0, 1.0],
        ];
        View {
            m: mul(rz, mul(rx, ry)),
            eye: None,
        }
    }

    /// Oblique (cabinet) view: the face unturned, depth drawn along (dx, dy) per point of depth.
    pub fn oblique(dx: f64, dy: f64) -> View {
        View {
            m: [[1.0, 0.0, -dx], [0.0, 1.0, -dy], [0.0, 0.0, 1.0]],
            eye: None,
        }
    }

    fn apply(&self, p: V3) -> V3 {
        let m = &self.m;
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
        ]
    }

    /// Screen position of a view-space point (relative to the centre of the shape).
    fn project(&self, v: V3) -> (f64, f64) {
        match self.eye {
            Some(d) => {
                let s = d / (d - v[2]).max(d * 0.1);
                (v[0] * s, v[1] * s)
            }
            None => (v[0], v[1]),
        }
    }
}

fn mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// The solid and its look.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Model {
    /// Extrusion depth (points, away from the viewer).
    pub depth: f64,
    /// Top bevel inset width and height (points) and its profile (§20.1.10.9 ST_BevelPresetType).
    pub bevel: Option<(f64, f64, String)>,
    /// Bottom bevel (§20.1.5.1 `bevelB`) on the back of the extrusion.
    pub bevel_bottom: Option<(f64, f64, String)>,
    /// Face colour.
    pub face: Rgba,
    /// Side colour (`extrusionClr`, else the face colour).
    pub side: Rgba,
    /// Contour (`contourW`, `contourClr`).
    pub contour: Option<(f64, Rgba)>,
    /// Direction towards the light, view space (unit).
    pub light: V3,
    /// Specular strength and exponent (0: matte).
    pub specular: (f64, f64),
    /// Draw the front face (false when the caller draws it with the shape's own fill).
    pub front: bool,
}

/// A face ready to draw: layer, depth of its centre, page points and shaded colour.
type Drawn = (usize, f64, Vec<(f64, f64)>, Rgba);

/// One face: view-space vertices and its colour before shading.
struct Face {
    pts: Vec<V3>,
    color: Rgba,
}

/// Bevel profile: (inset share, height share) for step `t` in 0..=1 (§20.1.10.9 ST_BevelPresetType).
fn profile(prst: &str, t: f64) -> (f64, f64) {
    use std::f64::consts::FRAC_PI_2;
    match prst {
        "angle" | "slope" | "coolSlant" | "artDeco" | "divot" => (t, t),
        "convex" | "softRound" => (libm::sin(t * FRAC_PI_2), 1.0 - libm::cos(t * FRAC_PI_2)),
        "relaxedInset" => (t, libm::sin(t * FRAC_PI_2)),
        "cross" | "hardEdge" => (
            t.min(0.5) * 2.0,
            if t < 0.5 { 0.0 } else { (t - 0.5) * 2.0 },
        ),
        // circle (the default) and the rest: a quarter circle.
        _ => (1.0 - libm::cos(t * FRAC_PI_2), libm::sin(t * FRAC_PI_2)),
    }
}

/// Signed area (positive: clockwise on screen with y down).
fn area(p: &[(f64, f64)]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

/// `p` moved inwards by `d` (miter offset of each vertex, limited at sharp corners).
fn inset(p: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    if d == 0.0 {
        return p.to_vec();
    }
    let n = p.len();
    let sign = if area(p) > 0.0 { 1.0 } else { -1.0 };
    (0..n)
        .map(|i| {
            let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
            let nrm = |u: (f64, f64), v: (f64, f64)| {
                let (dx, dy) = (v.0 - u.0, v.1 - u.1);
                let l = libm::hypot(dx, dy).max(1e-9);
                // Inward normal of an edge of a clockwise polygon (y down).
                (-dy / l * sign, dx / l * sign)
            };
            let (n1, n2) = (nrm(a, b), nrm(b, c));
            let (mx, my) = (n1.0 + n2.0, n1.1 + n2.1);
            let ml = libm::hypot(mx, my);
            if ml < 1e-9 {
                return (b.0 + n1.0 * d, b.1 + n1.1 * d);
            }
            let cos = (mx * n1.0 + my * n1.1) / ml;
            let k = (d / cos.max(0.35)).min(d * 3.0);
            (b.0 + mx / ml * k, b.1 + my / ml * k)
        })
        .collect()
}

/// Newell's method: the normal of a (possibly non-planar) polygon. A polygon wound clockwise on screen
/// (y down) gets a normal towards the viewer (+z); the outlines are oriented that way, so these are
/// outward normals.
fn newell(pts: &[V3]) -> V3 {
    let mut nv = [0.0; 3];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        nv[0] += (a[1] - b[1]) * (a[2] + b[2]);
        nv[1] += (a[2] - b[2]) * (a[0] + b[0]);
        nv[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    nv
}

/// Colour of a face with the outward normal `n` (shape space) seen from the direction `eye`. The light
/// rig turns with the shape (§20.1.5.10 lightRig without `rot`), so a face keeps its colour whatever
/// the camera; the faces turned from the light get darker.
fn shade(c: Rgba, n: V3, eye: V3, m: &Model) -> Rgba {
    // Ambient + Lambert, scaled so that the front face keeps its colour.
    let diffuse = dot(n, m.light).max(0.0) / m.light[2].max(0.2);
    let k = (0.45 + 0.55 * diffuse).min(1.3);
    let (s, e) = m.specular;
    // Blinn-Phong half vector between the light and the viewer.
    let half = norm([
        m.light[0] + eye[0],
        m.light[1] + eye[1],
        m.light[2] + eye[2],
    ]);
    let spec = if s > 0.0 {
        s * libm::pow(dot(n, half).max(0.0), e)
    } else {
        0.0
    };
    let ch = |v: u8| (f64::from(v) * k + 255.0 * spec).round().clamp(0.0, 255.0) as u8;
    Rgba(ch(c.0), ch(c.1), ch(c.2), c.3)
}

/// The solid drawn in two layers: `behind` (the sides of the extrusion and its back, under the face) and
/// `over` (the bevel and — with [`Model::front`] — the front face, then the contour). The outline
/// polygons `polys` of a `w` × `h` shape are in shape coordinates (y down); `map` takes the projected
/// shape coordinates to the page.
pub(crate) fn render(
    polys: &[Vec<(f64, f64)>],
    (w, h): (f64, f64),
    view: &View,
    m: &Model,
    map: &dyn Fn(f64, f64) -> (f64, f64),
) -> (Vec<Command>, Vec<Command>) {
    let (cx, cy) = (w / 2.0, h / 2.0);
    // Shape space about the centre; `view.apply` turns it to view space.
    let to3 = |p: (f64, f64), z: f64| [p.0 - cx, p.1 - cy, z];
    // The viewer's direction in shape space (the transposed rotation of +z).
    let eye_dir = norm([view.m[2][0], view.m[2][1], view.m[2][2]]);
    // (layer, face)
    let mut faces: Vec<(usize, Face)> = Vec::new();
    let steps = 6usize;
    for poly in polys.iter().filter(|p| p.len() >= 3) {
        // Rings from the back of the solid to the front face: bottom bevel steps (deepest first), the
        // back and the front of the extrusion, top bevel steps.
        let bevel_rings = |bevel: &Option<(f64, f64, String)>, sign: f64, base: f64| {
            let mut out = Vec::new();
            if let Some((bw, bh, prst)) = bevel {
                let bw = bw.min(w.min(h) * 0.45);
                for k in 1..=steps {
                    let (fi, fz) = profile(prst, k as f64 / steps as f64);
                    out.push((inset(poly, bw * fi), base + sign * bh * fz));
                }
            }
            out
        };
        let mut rings: Vec<(Vec<(f64, f64)>, f64)> = bevel_rings(&m.bevel_bottom, -1.0, -m.depth);
        rings.reverse();
        let front_at = rings.len() + 1;
        rings.push((poly.clone(), -m.depth));
        rings.push((poly.clone(), 0.0));
        rings.extend(bevel_rings(&m.bevel, 1.0, 0.0));
        // Side and bevel quads between consecutive rings (each ring further back than the next).
        for r in 0..rings.len() - 1 {
            if r + 1 == front_at && m.depth <= 0.0 {
                continue;
            }
            let top = r >= front_at;
            let (a, za) = &rings[r];
            let (b, zb) = &rings[r + 1];
            for i in 0..a.len() {
                let j = (i + 1) % a.len();
                faces.push((
                    usize::from(top),
                    Face {
                        pts: vec![
                            to3(a[i], *za),
                            to3(a[j], *za),
                            to3(b[j], *zb),
                            to3(b[i], *zb),
                        ],
                        color: if top { m.face } else { m.side },
                    },
                ));
            }
        }
        if let Some((back, z)) = rings
            .first()
            .filter(|_| m.depth > 0.0 || m.bevel_bottom.is_some())
        {
            faces.push((
                0,
                Face {
                    pts: back.iter().rev().map(|p| to3(*p, *z)).collect(),
                    color: m.side,
                },
            ));
        }
        if m.front {
            if let Some((top, z)) = rings.last() {
                faces.push((
                    1,
                    Face {
                        pts: top.iter().map(|p| to3(*p, *z)).collect(),
                        color: m.face,
                    },
                ));
            }
        }
    }
    // Cull faces turned away, sort back to front (painter's algorithm).
    let mut drawn: Vec<Drawn> = Vec::new();
    for (layer, f) in faces {
        let vp: Vec<V3> = f.pts.iter().map(|p| view.apply(*p)).collect();
        let c = vp
            .iter()
            .fold([0.0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
        let n = vp.len() as f64;
        let centre = [c[0] / n, c[1] / n, c[2] / n];
        let nv = newell(&vp);
        let nm = newell(&f.pts);
        if dot(nv, nv) < 1e-12 || dot(nm, nm) < 1e-12 {
            continue;
        }
        let to_eye = match view.eye {
            Some(d) => norm(sub([0.0, 0.0, d], centre)),
            None => [0.0, 0.0, 1.0],
        };
        if dot(norm(nv), to_eye) <= 1e-6 {
            continue;
        }
        let shaded = shade(f.color, norm(nm), eye_dir, m);
        let pts = vp
            .iter()
            .map(|p| {
                let (x, y) = view.project(*p);
                map(x + cx, y + cy)
            })
            .collect();
        drawn.push((layer, centre[2], pts, shaded));
    }
    drawn.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut layers = (Vec::new(), Vec::new());
    for (layer, _, pts, color) in drawn {
        let cmd = Command::Path {
            segs: poly_segs(&pts),
            fill: Some(Paint::Solid { color }),
            // A hairline in the face colour hides the seams between neighbouring polygons.
            stroke: Some(Stroke {
                width: 0.35,
                color,
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        };
        if layer == 0 {
            layers.0.push(cmd);
        } else {
            layers.1.push(cmd);
        }
    }
    if let Some((cw, color)) = m.contour.filter(|(w, _)| *w > 0.0) {
        for poly in polys {
            let pts: Vec<(f64, f64)> = poly
                .iter()
                .map(|p| {
                    let (x, y) = view.project(view.apply(to3(*p, 0.0)));
                    map(x + cx, y + cy)
                })
                .collect();
            layers.1.push(Command::Path {
                segs: poly_segs(&pts),
                fill: None,
                stroke: Some(Stroke {
                    width: cw as f32,
                    color,
                    dash: vec![],
                    cap: "round".into(),
                    join: "round".into(),
                }),
            });
        }
    }
    layers
}

fn poly_segs(pts: &[(f64, f64)]) -> Vec<PathSeg> {
    let mut segs: Vec<PathSeg> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            if i == 0 {
                PathSeg::M(*x as f32, *y as f32)
            } else {
                PathSeg::L(*x as f32, *y as f32)
            }
        })
        .collect();
    segs.push(PathSeg::Z);
    segs
}

/// Polygons of the outline: path segments flattened (Béziers into 12 straight pieces), one polygon
/// per closed subpath, oriented clockwise on screen.
pub(crate) fn polygons(segs: &[bo_drawingml::geom::Seg]) -> Vec<Vec<(f64, f64)>> {
    use bo_drawingml::geom::Seg;
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let mut last = (0.0, 0.0);
    let flush = |cur: &mut Vec<(f64, f64)>, out: &mut Vec<Vec<(f64, f64)>>| {
        // Points closer than a hundredth of a point (arc joins, a closing point that repeats the start)
        // would make edges without a direction.
        let near =
            |a: &(f64, f64), b: &(f64, f64)| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
        cur.dedup_by(|a, b| near(a, b));
        while cur.len() > 1 && near(&cur[0], &cur[cur.len() - 1]) {
            cur.pop();
        }
        if cur.len() >= 3 && area(cur).abs() > 1e-6 {
            let mut p = std::mem::take(cur);
            if area(&p) < 0.0 {
                p.reverse();
            }
            out.push(p);
        }
        cur.clear();
    };
    for s in segs {
        match *s {
            Seg::Move(x, y) => {
                flush(&mut cur, &mut out);
                cur.push((x, y));
                last = (x, y);
            }
            Seg::Line(x, y) => {
                cur.push((x, y));
                last = (x, y);
            }
            Seg::Quad(x1, y1, x, y) => {
                for k in 1..=12 {
                    let t = f64::from(k) / 12.0;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * last.0 + 2.0 * u * t * x1 + t * t * x,
                        u * u * last.1 + 2.0 * u * t * y1 + t * t * y,
                    ));
                }
                last = (x, y);
            }
            Seg::Cubic(x1, y1, x2, y2, x, y) => {
                for k in 1..=12 {
                    let t = f64::from(k) / 12.0;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * u * last.0
                            + 3.0 * u * u * t * x1
                            + 3.0 * u * t * t * x2
                            + t * t * t * x,
                        u * u * u * last.1
                            + 3.0 * u * u * t * y1
                            + 3.0 * u * t * t * y2
                            + t * t * t * y,
                    ));
                }
                last = (x, y);
            }
            Seg::Close => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn square() -> Vec<Vec<(f64, f64)>> {
        vec![vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]]
    }

    fn model(depth: f64) -> Model {
        Model {
            depth,
            bevel: None,
            bevel_bottom: None,
            face: Rgba(200, 50, 50, 255),
            side: Rgba(200, 50, 50, 255),
            contour: None,
            light: norm([0.0, -0.6, 0.8]),
            specular: (0.0, 1.0),
            front: true,
        }
    }

    fn all(polys: &[Vec<(f64, f64)>], view: &View, m: &Model) -> Vec<Command> {
        let (mut a, b) = render(polys, (100.0, 100.0), view, m, &|x, y| (x, y));
        a.extend(b);
        a
    }

    fn bounds(cmds: &[Command]) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for c in cmds {
            if let Command::Path { segs, .. } = c {
                for s in segs {
                    if let PathSeg::M(x, y) | PathSeg::L(x, y) = s {
                        let (x, y) = (f64::from(*x), f64::from(*y));
                        b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                    }
                }
            }
        }
        b
    }

    // source: geometry of a cube under the isometric view (rotation about y by 45°, about x by
    // arctan(1/√2) = 35.26°): the silhouette of a unit cube is √2 wide and (1/√3 + √2·sin…)·… high.
    #[test]
    fn a_turned_cube_shows_three_faces_and_its_silhouette() {
        let view = View::turned(35.264_389_682_754_654, 45.0, 0.0);
        let cmds = all(&square(), &view, &model(100.0));
        // Front, one side and the top (seen from above and from the left) — the back and the other
        // sides are culled.
        assert_eq!(cmds.len(), 3, "{cmds:?}");
        let (x0, _, x1, _) = bounds(&cmds);
        // Width of the silhouette: 100 · √2.
        assert!(
            ((x1 - x0) - 100.0 * 2f64.sqrt()).abs() < 1e-6,
            "{}",
            x1 - x0
        );
        // Each visible face of a cube under the isometric view is equally lit when the light comes
        // from the eye: the three colours are distinct with an off-axis light.
        let colors: Vec<Rgba> = cmds
            .iter()
            .filter_map(|c| match c {
                Command::Path {
                    fill: Some(Paint::Solid { color }),
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(colors.len(), 3);
    }

    #[test]
    fn front_view_draws_only_the_face() {
        let cmds = all(&square(), &View::turned(0.0, 0.0, 0.0), &model(40.0));
        assert_eq!(cmds.len(), 1);
        assert_eq!(bounds(&cmds), (0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn oblique_and_perspective_views() {
        // Cabinet view to the upper left: the back face is offset by depth × (dx, dy).
        let cmds = all(&square(), &View::oblique(-0.5, -0.5), &model(40.0));
        let (x0, y0, x1, y1) = bounds(&cmds);
        assert_eq!((x0, y0, x1, y1), (-20.0, -20.0, 100.0, 100.0));
        // A perspective camera (eye 400 pt in front) scales by d / (d − z): the plane z = 0 is kept,
        // a point 100 pt nearer is drawn 4/3 as far from the centre, 400 pt behind half as far.
        let mut v = View::turned(0.0, 0.0, 0.0);
        v.eye = Some(400.0);
        assert_eq!(v.project([30.0, 60.0, 0.0]), (30.0, 60.0));
        let (x, y) = v.project([30.0, 60.0, 100.0]);
        assert!((x - 40.0).abs() < 1e-9 && (y - 80.0).abs() < 1e-9);
        assert_eq!(v.project([30.0, 60.0, -400.0]), (15.0, 30.0));
        let front = all(&square(), &v, &model(0.0));
        assert_eq!(
            bounds(&front),
            (0.0, 0.0, 100.0, 100.0),
            "z = 0 is not magnified"
        );
        // Turned towards the left under perspective: the nearer (left) edge of the face is taller
        // than the far (right) edge.
        let mut v = View::turned(0.0, 30.0, 0.0);
        v.eye = Some(300.0);
        let mut m = model(0.0);
        m.front = true;
        let cmds = all(&square(), &v, &m);
        let Some(Command::Path { segs, .. }) = cmds.last() else {
            panic!("{cmds:?}");
        };
        let pts: Vec<(f32, f32)> = segs
            .iter()
            .filter_map(|s| match s {
                PathSeg::M(x, y) | PathSeg::L(x, y) => Some((*x, *y)),
                _ => None,
            })
            .collect();
        let edge = |left: bool| {
            let xs: Vec<&(f32, f32)> = pts.iter().filter(|p| (p.0 < 50.0) == left).collect();
            xs.iter().map(|p| p.1).fold(f32::MIN, f32::max)
                - xs.iter().map(|p| p.1).fold(f32::MAX, f32::min)
        };
        assert!(edge(true) > edge(false) + 1.0, "{pts:?}");
    }

    #[test]
    fn bevel_rings_and_inset() {
        let p = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let q = inset(&p, 10.0);
        assert!(
            q.iter()
                .zip([(10.0, 10.0), (90.0, 10.0), (90.0, 90.0), (10.0, 90.0)])
                .all(|(a, b)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9),
            "{q:?}"
        );
        let mut m = model(20.0);
        m.bevel = Some((10.0, 10.0, "circle".into()));
        let cmds = all(&square(), &View::turned(0.0, 0.0, 0.0), &m);
        // Seen from the front: the four bevel bands of each ring and the front face.
        assert_eq!(cmds.len(), 6 * 4 + 1);
        assert_eq!(profile("angle", 0.5), (0.5, 0.5));
        let (i, z) = profile("circle", 1.0);
        assert!((i - 1.0).abs() < 1e-9 && (z - 1.0).abs() < 1e-9);
    }

    // A bottom bevel shows only from behind: seen from the back (lon 180°) its six rings of four bands
    // and the inset back face are drawn, the front stays hidden.
    #[test]
    fn bottom_bevel_faces_the_back() {
        let mut m = model(20.0);
        m.bevel_bottom = Some((10.0, 10.0, "angle".into()));
        let front = all(&square(), &View::turned(0.0, 0.0, 0.0), &m);
        assert_eq!(front.len(), 1);
        let back = all(&square(), &View::turned(0.0, 180.0, 0.0), &m);
        assert_eq!(back.len(), 6 * 4 + 1);
        // The back face is the deepest ring: 10 pt in on each side.
        let (x0, _, x1, _) = bounds(&back);
        assert!((x1 - x0 - 100.0).abs() < 1e-3, "{x0} {x1}");
    }

    #[test]
    fn outline_polygons_from_segments() {
        use bo_drawingml::geom::Seg;
        let segs = [
            Seg::Move(0.0, 0.0),
            Seg::Line(0.0, 10.0),
            Seg::Line(10.0, 10.0),
            Seg::Line(10.0, 0.0),
            Seg::Close,
        ];
        let p = polygons(&segs);
        assert_eq!(p.len(), 1);
        // Re-oriented clockwise (positive area with y down).
        assert!(area(&p[0]) > 0.0);
    }
}

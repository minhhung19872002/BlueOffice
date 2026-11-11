//! 3-D charts (ECMA-376 Part 1, §21.2.2.15 bar3DChart, §21.2.2.96 line3DChart, §21.2.2.4 area3DChart,
//! §21.2.2.140 pie3DChart, §21.2.2.202 surface3DChart) drawn as projected solids: the scene (categories
//! across, values up, series one behind the other) is built from boxes, cylinders, cones and pyramids
//! (§21.2.2.177 shape), ribbons, slabs, pie wedges and a surface grid, seen through the §21.2.2.228 view3D
//! camera (rotX, rotY, perspective, or right-angle axes as an oblique drawing), its hidden faces culled,
//! sorted back to front and shaded by one light from the upper left. The floor, side and back walls
//! carry the grid lines of the value axis.

use super::{fmt_num, nice_step, surface_bands, Align, ColorFn, LabelFn, Ser, AXIS, GRID, TEXT};
use crate::numfmt::Locale;
use bo_model::sheet::{ChartKind, ChartPlot, View3D};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

/// A point of the scene: x right, y up, z away from the viewer.
pub(super) type P3 = [f64; 3];

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: P3) -> P3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

/// The camera of a §21.2.2.228 view3D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Cam {
    /// rotY and rotX (radians).
    turn: f64,
    tilt: f64,
    /// Right-angle axes: depth drawn along this screen direction (x, up) per unit.
    oblique: Option<(f64, f64)>,
    /// Eye distance of a perspective camera (scene units).
    eye: Option<f64>,
}

impl Cam {
    /// The camera for a scene `extent` units across.
    pub(super) fn new(v: &View3D, extent: f64) -> Cam {
        let (turn, tilt) = (
            f64::from(v.rot_y).to_radians(),
            f64::from(v.rot_x).to_radians(),
        );
        if v.r_ang_ax {
            // Right-angle axes: the front stays square-on, the depth goes off sideways and up.
            return Cam {
                turn,
                tilt,
                oblique: Some((turn.sin(), tilt.sin())),
                eye: None,
            };
        }
        // §21.2.2.135 perspective: the field of view, taken in half degrees (30 = 15°); 0 is
        // orthographic.
        let fov = f64::from(v.perspective) / 2.0;
        Cam {
            turn,
            tilt,
            oblique: None,
            eye: (fov >= 1.0).then(|| extent / 2.0 / (fov / 2.0).to_radians().tan() + extent / 2.0),
        }
    }

    /// Screen position (x right, y up) and depth (larger: further away) of a scene point relative to
    /// the centre of the scene.
    pub(super) fn view(&self, p: P3) -> P3 {
        if let Some((ox, oy)) = self.oblique {
            return [p[0] + p[2] * ox, p[1] + p[2] * oy, p[2]];
        }
        let (st, ct) = self.turn.sin_cos();
        let (sx, cx) = self.tilt.sin_cos();
        // Turn about the vertical axis (rotY: the depth recedes to the right), then tilt (rotX: seen
        // from above, the far side shows higher).
        let x = p[0] * ct + p[2] * st;
        let d = -p[0] * st + p[2] * ct;
        let up = p[1] * cx + d * sx;
        let depth = d * cx - p[1] * sx;
        match self.eye {
            Some(e) => {
                let k = e / (e + depth).max(e * 0.1);
                [x * k, up * k, depth]
            }
            None => [x, up, depth],
        }
    }
}

/// One polygon of the scene.
#[derive(Debug, Clone)]
pub(super) struct Face {
    pts: Vec<P3>,
    color: Rgba,
    /// Outward direction of a face of a closed solid (hidden when turned away); `None`: seen from both
    /// sides.
    out: Option<P3>,
    /// Only the outline (surface wireframe).
    wire: bool,
    /// Drawn in its own colour, unlit (walls and floor).
    flat: bool,
}

impl Face {
    fn solid(pts: Vec<P3>, color: Rgba, out: P3) -> Face {
        Face {
            pts,
            color,
            out: Some(out),
            wire: false,
            flat: false,
        }
    }

    fn wall(pts: Vec<P3>, color: Rgba) -> Face {
        Face {
            flat: true,
            ..Face::open(pts, color)
        }
    }

    fn open(pts: Vec<P3>, color: Rgba) -> Face {
        Face {
            pts,
            color,
            out: None,
            wire: false,
            flat: false,
        }
    }
}

/// Newell's method.
fn newell(pts: &[P3]) -> P3 {
    let mut n = [0.0; 3];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

fn shoelace(p: &[(f64, f64)]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

/// Towards the light: from the upper left, in front.
const LIGHT: P3 = [-0.42, 0.72, -0.55];

fn shade(c: Rgba, n: P3) -> Rgba {
    let l = norm(LIGHT);
    // The front (0, 0, −1) keeps its colour; faces turned from the light get darker.
    let front = -l[2];
    let k = (0.55 + 0.45 * dot(n, l).max(0.0) / front).min(1.2);
    let ch = |v: u8| (f64::from(v) * k).round().clamp(0.0, 255.0) as u8;
    Rgba(ch(c.0), ch(c.1), ch(c.2), c.3)
}

/// A projected face ready to draw.
struct Drawn {
    pts: Vec<(f64, f64)>,
    color: Rgba,
    wire: bool,
}

/// Projects `groups` of faces (drawn group after group; within a group the solids are sorted back to
/// front by their faces' depth) and fits the drawing into `[l, t, r, b]`. Returns the commands and the
/// fitting map of scene points (relative to `centre`) to the page.
pub(super) struct Scene {
    pub cam: Cam,
    pub centre: P3,
    /// (groups of objects; each object a list of faces).
    pub groups: Vec<Vec<Vec<Face>>>,
}

/// Maps projected points into the plot rect.
#[derive(Debug, Clone, Copy)]
pub(super) struct Fit {
    s: f64,
    ox: f64,
    oy: f64,
}

impl Fit {
    pub(super) fn at(&self, cam: &Cam, centre: P3, p: P3) -> (f64, f64) {
        let v = cam.view([p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]);
        (self.ox + v[0] * self.s, self.oy - v[1] * self.s)
    }
}

impl Scene {
    fn project(&self, p: P3) -> P3 {
        self.cam.view([
            p[0] - self.centre[0],
            p[1] - self.centre[1],
            p[2] - self.centre[2],
        ])
    }

    /// Fits the scene into `[l, t, r, b]` and draws it.
    pub(super) fn draw(&self, [l, t, r, b]: [f64; 4], out: &mut Vec<Command>) -> Fit {
        let mut lo = (f64::MAX, f64::MAX);
        let mut hi = (f64::MIN, f64::MIN);
        for p in self.groups.iter().flatten().flatten().flat_map(|f| &f.pts) {
            let v = self.project(*p);
            lo = (lo.0.min(v[0]), lo.1.min(v[1]));
            hi = (hi.0.max(v[0]), hi.1.max(v[1]));
        }
        if lo.0 > hi.0 {
            return Fit {
                s: 1.0,
                ox: l,
                oy: b,
            };
        }
        let s = ((r - l) / (hi.0 - lo.0).max(1e-9))
            .min((b - t) / (hi.1 - lo.1).max(1e-9))
            .max(1e-6);
        let fit = Fit {
            s,
            ox: f64::midpoint(l, r) - f64::midpoint(lo.0, hi.0) * s,
            oy: f64::midpoint(t, b) + f64::midpoint(lo.1, hi.1) * s,
        };
        for group in &self.groups {
            // Objects back to front by their mean depth.
            let mut objs: Vec<(f64, Vec<Drawn>, Vec<f64>)> = Vec::new();
            for obj in group {
                let mut drawn = Vec::new();
                let mut depths = Vec::new();
                let mut sum = 0.0;
                let mut n = 0.0;
                for f in obj {
                    let vp: Vec<P3> = f.pts.iter().map(|p| self.project(*p)).collect();
                    let d = vp.iter().map(|v| v[2]).sum::<f64>() / vp.len().max(1) as f64;
                    sum += d;
                    n += 1.0;
                    let screen: Vec<(f64, f64)> = vp
                        .iter()
                        .map(|v| (fit.ox + v[0] * s, fit.oy - v[1] * s))
                        .collect();
                    if let Some(o) = f.out {
                        // Orient the polygon about its outward direction; seen from that side it
                        // turns clockwise in (x, up) — counter-clockwise on the page (y down).
                        let nw = newell(&f.pts);
                        let flip = dot(nw, o) < 0.0;
                        let area = shoelace(&screen);
                        // On the page (y down) a face seen from outside has a positive area.
                        let seen = if flip { area < 0.0 } else { area > 0.0 };
                        if !seen || area.abs() < 1e-9 {
                            continue;
                        }
                    }
                    // A closed solid's face is lit on its outer side, a two-sided face on the side
                    // the viewer sees.
                    let nw = norm(newell(&f.pts));
                    let keep = f.out.map_or(shoelace(&screen) > 0.0, |o| dot(nw, o) >= 0.0);
                    let n = if keep { nw } else { [-nw[0], -nw[1], -nw[2]] };
                    depths.push(d);
                    drawn.push(Drawn {
                        pts: screen,
                        color: if f.wire || f.flat {
                            f.color
                        } else {
                            shade(f.color, n)
                        },
                        wire: f.wire,
                    });
                }
                objs.push((if n > 0.0 { sum / n } else { 0.0 }, drawn, depths));
            }
            objs.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (_, drawn, depths) in objs {
                let mut order: Vec<usize> = (0..drawn.len()).collect();
                order.sort_by(|a, b| depths[*b].total_cmp(&depths[*a]));
                for i in order {
                    let d = &drawn[i];
                    let mut segs: Vec<PathSeg> = d
                        .pts
                        .iter()
                        .enumerate()
                        .map(|(k, (x, y))| {
                            if k == 0 {
                                PathSeg::M(*x as f32, *y as f32)
                            } else {
                                PathSeg::L(*x as f32, *y as f32)
                            }
                        })
                        .collect();
                    segs.push(PathSeg::Z);
                    out.push(Command::Path {
                        segs,
                        fill: (!d.wire).then_some(Paint::Solid { color: d.color }),
                        stroke: Some(Stroke {
                            width: if d.wire { 1.0 } else { 0.35 },
                            color: d.color,
                            dash: vec![],
                            cap: "butt".into(),
                            join: "round".into(),
                        }),
                    });
                }
            }
        }
        fit
    }
}

/// An upright prism or pyramid of `sides` (4: box, more: cylinder / cone) standing on the (x, z) base
/// centred at (`cx`, `cz`), half sizes `hx`, `hz`, from `y0` to `y1`; `taper` is the top's share of
/// the base (0: cone / pyramid). `axes` maps the logical (x, y, z) to the scene.
#[allow(clippy::too_many_arguments)]
fn prism(
    (cx, cz): (f64, f64),
    (hx, hz): (f64, f64),
    (y0, y1): (f64, f64),
    sides: usize,
    taper: f64,
    color: Rgba,
    axes: &dyn Fn(P3) -> P3,
    out: &mut Vec<Face>,
) {
    let ring = |scale: f64, y: f64| -> Vec<P3> {
        (0..sides)
            .map(|k| {
                let a = if sides == 4 {
                    std::f64::consts::FRAC_PI_4 + std::f64::consts::FRAC_PI_2 * k as f64
                } else {
                    std::f64::consts::TAU * k as f64 / sides as f64
                };
                let r = if sides == 4 {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
                [
                    cx + a.cos() * hx * r * scale,
                    y,
                    cz + a.sin() * hz * r * scale,
                ]
            })
            .collect()
    };
    let (bottom, top) = (ring(1.0, y0), ring(taper, y1));
    let up = if y1 >= y0 { 1.0 } else { -1.0 };
    let dir = |v: P3| {
        let o = axes(v);
        let z = axes([0.0, 0.0, 0.0]);
        [o[0] - z[0], o[1] - z[1], o[2] - z[2]]
    };
    for k in 0..sides {
        let j = (k + 1) % sides;
        let mid = [
            f64::midpoint(bottom[k][0], bottom[j][0]) - cx,
            0.0,
            f64::midpoint(bottom[k][2], bottom[j][2]) - cz,
        ];
        out.push(Face::solid(
            vec![axes(bottom[k]), axes(bottom[j]), axes(top[j]), axes(top[k])],
            color,
            dir(mid),
        ));
    }
    out.push(Face::solid(
        bottom.iter().map(|p| axes(*p)).collect(),
        color,
        dir([0.0, -up, 0.0]),
    ));
    if taper > 0.0 {
        out.push(Face::solid(
            top.iter().map(|p| axes(*p)).collect(),
            color,
            dir([0.0, up, 0.0]),
        ));
    }
}

/// Value range and tick step of the plotted values (stacked groups summed, percent groups 0..100).
fn value_range(plot: &ChartPlot, ss: &[Ser]) -> (f64, f64, f64) {
    let n = ss.iter().map(|s| s.vals.len()).max().unwrap_or(0);
    let stacked = matches!(plot.grouping.as_deref(), Some("stacked" | "percentStacked"))
        && plot.kind != ChartKind::Line;
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    if plot.grouping.as_deref() == Some("percentStacked") && plot.kind != ChartKind::Line {
        hi = 100.0;
    } else if stacked {
        for i in 0..n {
            let (mut pos, mut neg) = (0.0, 0.0);
            for s in ss {
                match s.vals.get(i).copied().flatten() {
                    Some(v) if v >= 0.0 => pos += v,
                    Some(v) => neg += v,
                    None => {}
                }
            }
            hi = hi.max(pos);
            lo = lo.min(neg);
        }
    } else {
        for v in ss.iter().flat_map(|s| s.vals.iter().flatten()) {
            hi = hi.max(*v);
            lo = lo.min(*v);
        }
    }
    if plot.kind == ChartKind::Surface {
        // A surface starts at its lowest value, not at zero.
        let vals: Vec<f64> = ss
            .iter()
            .flat_map(|s| s.vals.iter().flatten().copied())
            .collect();
        if let Some(m) = vals.iter().copied().reduce(f64::min) {
            lo = m;
        }
    }
    let step = nice_step((hi - lo).max(1e-9), 5.0);
    let lo = (lo / step).floor() * step;
    let hi = ((hi / step).ceil() * step).max(lo + step);
    (lo, hi, step)
}

/// Draws a 3-D chart group `plot` with its series `ss` into `area`.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub(super) fn draw(
    plot: &ChartPlot,
    ss: &[Ser],
    view: &View3D,
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    if plot.kind == ChartKind::Pie {
        draw_pie(ss.first(), view, [l, t, r, b], out);
        return;
    }
    let n = ss.iter().map(|s| s.vals.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }
    let grouping = plot.grouping.as_deref().unwrap_or(match plot.kind {
        ChartKind::Bar => "clustered",
        _ => "standard",
    });
    let horizontal = plot.kind == ChartKind::Bar && plot.bar_dir.as_deref() == Some("bar");
    let percent = grouping == "percentStacked" && plot.kind != ChartKind::Line;
    let stacked = matches!(grouping, "stacked" | "percentStacked") && plot.kind != ChartKind::Line;
    // Series one behind the other: line and surface groups, standard bar / area groups.
    let rows_each = matches!(plot.kind, ChartKind::Line | ChartKind::Surface)
        || (grouping == "standard" && !stacked);
    let rows = if rows_each { ss.len().max(1) } else { 1 };
    let nf = n as f64;
    let rd = f64::from(view.depth_percent.clamp(20, 2000)) / 100.0;
    let depth = rows as f64 * rd;
    // Height of the value axis in category widths (§21.2.2.79 hPercent, else from the plot's shape).
    let (pw, ph) = ((r - l).max(1.0), (b - t).max(1.0));
    let aspect = if horizontal { pw / ph } else { ph / pw };
    let height = view
        .h_percent
        .map_or(nf * aspect.clamp(0.35, 1.6) * 0.85, |h| {
            nf * f64::from(h) / 100.0
        });
    let (vmin, vmax, step) = value_range(plot, ss);
    let vs = |v: f64| (v - vmin) / (vmax - vmin) * height;
    // Logical (category, value, depth) → scene.
    let axes = move |p: P3| -> P3 {
        if horizontal {
            [p[1], nf - p[0], p[2]]
        } else {
            p
        }
    };
    let (sw, sh) = if horizontal {
        (height, nf)
    } else {
        (nf, height)
    };
    let centre = [sw / 2.0, sh / 2.0, depth / 2.0];
    // Walls, floor and the value grid on them.
    let wall = Rgba(0xF4, 0xF5, 0xF7, 255);
    let floor = Rgba(0xE9, 0xEB, 0xEE, 255);
    let mut walls: Vec<Vec<Face>> = Vec::new();
    let base = vs(vmin.max(0.0).min(vmax));
    let q = |a: P3, b: P3, c: P3, d: P3| vec![axes(a), axes(b), axes(c), axes(d)];
    walls.push(vec![Face::wall(
        q(
            [0.0, 0.0, 0.0],
            [nf, 0.0, 0.0],
            [nf, 0.0, depth],
            [0.0, 0.0, depth],
        ),
        floor,
    )]);
    walls.push(vec![Face::wall(
        q(
            [0.0, 0.0, depth],
            [nf, 0.0, depth],
            [nf, height, depth],
            [0.0, height, depth],
        ),
        wall,
    )]);
    walls.push(vec![Face::wall(
        q(
            [0.0, 0.0, 0.0],
            [0.0, 0.0, depth],
            [0.0, height, depth],
            [0.0, height, 0.0],
        ),
        wall,
    )]);
    // Objects.
    let mut objs: Vec<Vec<Face>> = Vec::new();
    let shape = plot.style.as_deref().unwrap_or("box");
    let (sides, taper) = match shape {
        "cylinder" => (24, 1.0),
        "cone" | "coneToMax" => (24, 0.0),
        "pyramid" | "pyramidToMax" => (4, 0.0),
        _ => (4, 1.0),
    };
    let to_max = shape.ends_with("ToMax");
    let totals: Vec<f64> = (0..n)
        .map(|i| {
            ss.iter()
                .filter_map(|s| s.vals.get(i).copied().flatten())
                .map(f64::abs)
                .sum()
        })
        .collect();
    match plot.kind {
        ChartKind::Bar => {
            let per_slot = if stacked || rows_each {
                1
            } else {
                ss.len().max(1)
            };
            // §21.2.2.75 gapWidth 150 %: the gap is one and a half bars wide.
            let bw = 1.0 / (per_slot as f64 + 1.5);
            let mut pos = vec![0.0; n];
            let mut neg = vec![0.0; n];
            for (j, s) in ss.iter().enumerate() {
                let row = if rows_each { j } else { 0 };
                let cz = (row as f64 + 0.5) * rd;
                let hz = (rd * 0.6).min(bw * 1.2) / 2.0;
                for i in 0..n {
                    let Some(mut v) = s.vals.get(i).copied().flatten() else {
                        continue;
                    };
                    if percent && totals[i] > 0.0 {
                        v = v / totals[i] * 100.0;
                    }
                    let cx = if per_slot > 1 {
                        i as f64 + bw * (0.75 + j as f64 + 0.5)
                    } else {
                        i as f64 + 0.5
                    };
                    let (v0, v1) = if stacked {
                        let acc = if v >= 0.0 { &mut pos[i] } else { &mut neg[i] };
                        let from = *acc;
                        *acc += v;
                        (from, *acc)
                    } else {
                        (0.0, v)
                    };
                    let (y0, y1) = (vs(v0.clamp(vmin, vmax)), vs(v1.clamp(vmin, vmax)));
                    if (y1 - y0).abs() < 1e-9 {
                        continue;
                    }
                    // "To max" shapes narrow towards the axis maximum: the top keeps the share left.
                    let tp = if to_max && taper == 0.0 {
                        1.0 - (y1 - base).abs() / (height - base).max(1e-9)
                    } else {
                        taper
                    };
                    let mut faces = Vec::new();
                    prism(
                        (cx, cz),
                        (bw / 2.0, hz),
                        (y0, y1),
                        sides,
                        tp.clamp(0.0, 1.0),
                        s.pts.get(i).copied().unwrap_or(s.color),
                        &axes,
                        &mut faces,
                    );
                    objs.push(faces);
                }
            }
        }
        ChartKind::Line => {
            for (j, s) in ss.iter().enumerate() {
                let (z0, z1) = ((j as f64 + 0.3) * rd, (j as f64 + 0.7) * rd);
                let pts: Vec<Option<(f64, f64)>> = (0..n)
                    .map(|i| {
                        s.vals
                            .get(i)
                            .copied()
                            .flatten()
                            .map(|v| (i as f64 + 0.5, vs(v.clamp(vmin, vmax))))
                    })
                    .collect();
                // A tape: each segment a thin slab (top, bottom, front and back) of the row's depth.
                let th = height * 0.035;
                for w in pts.windows(2) {
                    if let (Some(a), Some(c)) = (w[0], w[1]) {
                        let up = [-(c.1 - a.1), c.0 - a.0, 0.0];
                        let mut faces = Vec::new();
                        for (y, o) in [(th, up), (0.0, [-up[0], -up[1], 0.0])] {
                            faces.push(Face::solid(
                                q(
                                    [a.0, a.1 + y, z0],
                                    [c.0, c.1 + y, z0],
                                    [c.0, c.1 + y, z1],
                                    [a.0, a.1 + y, z1],
                                ),
                                s.color,
                                axes_dir(&axes, o),
                            ));
                        }
                        for (z, o) in [(z0, -1.0), (z1, 1.0)] {
                            faces.push(Face::solid(
                                q(
                                    [a.0, a.1, z],
                                    [c.0, c.1, z],
                                    [c.0, c.1 + th, z],
                                    [a.0, a.1 + th, z],
                                ),
                                s.color,
                                axes_dir(&axes, [0.0, 0.0, o]),
                            ));
                        }
                        objs.push(faces);
                    }
                }
            }
        }
        ChartKind::Area => {
            let mut acc = vec![0.0; n];
            for (j, s) in ss.iter().enumerate() {
                let row = if rows_each { j } else { 0 };
                let (z0, z1) = ((row as f64 + 0.2) * rd, (row as f64 + 0.8) * rd);
                let mut lower = Vec::with_capacity(n);
                let mut upper = Vec::with_capacity(n);
                for i in 0..n {
                    let mut v = s.vals.get(i).copied().flatten().unwrap_or(0.0);
                    if percent && totals[i] > 0.0 {
                        v = v / totals[i] * 100.0;
                    }
                    let from = if stacked { acc[i] } else { 0.0 };
                    if stacked {
                        acc[i] += v;
                    }
                    let x = i as f64 + 0.5;
                    lower.push((x, vs(from.clamp(vmin, vmax))));
                    upper.push((x, vs((from + v).clamp(vmin, vmax))));
                }
                let c = s.color;
                let mut faces = Vec::new();
                let outline: Vec<(f64, f64)> = upper
                    .iter()
                    .copied()
                    .chain(lower.iter().rev().copied())
                    .collect();
                faces.push(Face::solid(
                    outline.iter().map(|p| axes([p.0, p.1, z0])).collect(),
                    c,
                    axes_dir(&axes, [0.0, 0.0, -1.0]),
                ));
                faces.push(Face::solid(
                    outline.iter().map(|p| axes([p.0, p.1, z1])).collect(),
                    c,
                    axes_dir(&axes, [0.0, 0.0, 1.0]),
                ));
                for k in 0..n.saturating_sub(1) {
                    for (edge, up) in [(&upper, 1.0), (&lower, -1.0)] {
                        let (a, e) = (edge[k], edge[k + 1]);
                        let o = [-(e.1 - a.1) * up, (e.0 - a.0) * up, 0.0];
                        faces.push(Face::solid(
                            q(
                                [a.0, a.1, z0],
                                [e.0, e.1, z0],
                                [e.0, e.1, z1],
                                [a.0, a.1, z1],
                            ),
                            c,
                            axes_dir(&axes, o),
                        ));
                    }
                }
                for (k, side) in [(0usize, -1.0), (n - 1, 1.0)] {
                    let (lo, hi) = (lower[k], upper[k]);
                    faces.push(Face::solid(
                        q(
                            [lo.0, lo.1, z0],
                            [hi.0, hi.1, z0],
                            [hi.0, hi.1, z1],
                            [lo.0, lo.1, z1],
                        ),
                        c,
                        axes_dir(&axes, [side, 0.0, 0.0]),
                    ));
                }
                objs.push(faces);
            }
        }
        ChartKind::Surface => {
            let bands = surface_bands(ss);
            let band_of = |v: f64| {
                bands
                    .iter()
                    .position(|(a, z)| v >= *a && v <= *z)
                    .unwrap_or(0)
            };
            let m = ss.len();
            let row_z = |j: usize| j as f64 * rd + rd / 2.0;
            let rows_v: Vec<&Ser> = if m == 1 {
                vec![&ss[0], &ss[0]]
            } else {
                ss.iter().collect()
            };
            let zs: Vec<f64> = if m == 1 {
                vec![0.1 * rd, 0.9 * rd]
            } else {
                (0..m).map(row_z).collect()
            };
            let mut faces = Vec::new();
            for j in 0..rows_v.len() - 1 {
                for i in 0..n.saturating_sub(1) {
                    let v = |s: &Ser, i: usize| s.vals.get(i).copied().flatten();
                    let (Some(a), Some(c), Some(d), Some(e)) = (
                        v(rows_v[j], i),
                        v(rows_v[j], i + 1),
                        v(rows_v[j + 1], i + 1),
                        v(rows_v[j + 1], i),
                    ) else {
                        continue;
                    };
                    let mean = (a + c + d + e) / 4.0;
                    let (x0, x1) = (i as f64 + 0.5, i as f64 + 1.5);
                    let mut f = Face::open(
                        q(
                            [x0, vs(a), zs[j]],
                            [x1, vs(c), zs[j]],
                            [x1, vs(d), zs[j + 1]],
                            [x0, vs(e), zs[j + 1]],
                        ),
                        color(None, band_of(mean)),
                    );
                    f.wire = plot.flag;
                    faces.push(f);
                }
            }
            // The surface is one object whose faces sort by depth.
            objs.push(faces);
        }
        _ => {}
    }
    let extent = sw.max(sh).max(depth);
    let scene = Scene {
        cam: Cam::new(view, extent),
        centre,
        groups: vec![walls, objs],
    };
    // Room for the value labels on the left (under for horizontal bars) and the categories under.
    let (ml, mb, mr) = if horizontal {
        (font * 4.0, font * 1.8, font)
    } else {
        (
            font * 3.6,
            font * 1.8,
            if rows_each && rows > 1 {
                font * 4.0
            } else {
                font
            },
        )
    };
    // Grid lines go with the walls (drawn first, under the objects).
    let mut body = Vec::new();
    let fit = scene.draw([l + ml, t + font * 0.5, r - mr, b - mb], &mut body);
    let at = |p: P3| fit.at(&scene.cam, centre, axes(p));
    // The walls are the first three paths; the grid goes right after them.
    let walls_end = body
        .iter()
        .take(3)
        .filter(|c| matches!(c, Command::Path { .. }))
        .count();
    let mut grid = Vec::new();
    let mut v = vmin;
    let mut guard = 0;
    while v <= vmax + step * 0.5 && guard < 60 {
        let y = vs(v);
        let pts = [at([0.0, y, 0.0]), at([0.0, y, depth]), at([nf, y, depth])];
        grid.push(Command::Path {
            segs: vec![
                PathSeg::M(pts[0].0 as f32, pts[0].1 as f32),
                PathSeg::L(pts[1].0 as f32, pts[1].1 as f32),
                PathSeg::L(pts[2].0 as f32, pts[2].1 as f32),
            ],
            fill: None,
            stroke: Some(Stroke {
                width: 0.5,
                color: GRID,
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        });
        let text = if percent {
            format!("{}%", fmt_num(v, locale))
        } else {
            fmt_num(v, locale)
        };
        let (x, yy) = if horizontal {
            at([nf, y, 0.0])
        } else {
            at([0.0, y, 0.0])
        };
        if horizontal {
            label(
                &text,
                [x - font * 3.0, yy + 2.0, x + font * 3.0, yy + font * 1.6],
                Align::Center,
                font,
                TEXT,
                &mut grid,
            );
        } else {
            label(
                &text,
                [x - font * 4.0, yy - font, x - 3.0, yy + font],
                Align::Right,
                font,
                TEXT,
                &mut grid,
            );
        }
        v += step;
        guard += 1;
    }
    // Floor edges.
    let edge = |a: (f64, f64), e: (f64, f64)| Command::Path {
        segs: vec![
            PathSeg::M(a.0 as f32, a.1 as f32),
            PathSeg::L(e.0 as f32, e.1 as f32),
        ],
        fill: None,
        stroke: Some(Stroke {
            width: 0.75,
            color: AXIS,
            dash: vec![],
            cap: "butt".into(),
            join: "round".into(),
        }),
    };
    grid.push(edge(at([0.0, base, 0.0]), at([nf, base, 0.0])));
    grid.push(edge(at([0.0, 0.0, 0.0]), at([0.0, height, 0.0])));
    let tail = body.split_off(walls_end);
    out.extend(body);
    out.extend(grid);
    out.extend(tail);
    // Category labels along the front edge.
    let cats = ss.iter().map(|s| &s.cats).max_by_key(|c| c.len());
    let slot = (at([1.0, 0.0, 0.0]).0 - at([0.0, 0.0, 0.0]).0)
        .abs()
        .max(font * 2.0);
    for i in 0..n {
        let text = cats
            .and_then(|c| c.get(i))
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        let (x, y) = at([i as f64 + 0.5, 0.0, 0.0]);
        if horizontal {
            label(
                &text,
                [x - font * 4.0, y - font, x - 3.0, y + font],
                Align::Right,
                font,
                TEXT,
                out,
            );
        } else {
            label(
                &text,
                [x - slot / 2.0, y + 2.0, x + slot / 2.0, y + font * 1.6],
                Align::Center,
                font,
                TEXT,
                out,
            );
        }
    }
    // Series names along the depth (series one behind the other).
    if rows_each && rows > 1 && !horizontal {
        for (j, s) in ss.iter().enumerate() {
            let (x, y) = at([nf, 0.0, (j as f64 + 0.5) * rd]);
            label(
                &s.name,
                [x + 3.0, y - font, x + font * 4.0, y + font],
                Align::Left,
                font,
                TEXT,
                out,
            );
        }
    }
}

fn axes_dir(axes: &dyn Fn(P3) -> P3, v: P3) -> P3 {
    let (o, z) = (axes(v), axes([0.0, 0.0, 0.0]));
    [o[0] - z[0], o[1] - z[1], o[2] - z[2]]
}

/// §21.2.2.140 pie3DChart: wedges of a tilted disc (rotX: tilt, rotY: angle of the first slice),
/// exploded slices pushed out along their middle.
fn draw_pie(s: Option<&Ser>, view: &View3D, [l, t, r, b]: [f64; 4], out: &mut Vec<Command>) {
    let Some(s) = s else {
        return;
    };
    let vals: Vec<f64> = s.vals.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    if total <= 0.0 {
        return;
    }
    let thick = 0.22;
    let mut tops = Vec::new();
    let mut sides = Vec::new();
    let mut a0 = f64::from(view.rot_y).to_radians();
    // Clockwise from 12 o'clock seen from above: the far side (+z) is 12 o'clock.
    let at = |a: f64, rad: f64, y: f64, (ox, oz): (f64, f64)| {
        [ox + a.sin() * rad, y, oz + a.cos() * rad]
    };
    for (i, v) in vals.iter().enumerate() {
        if *v <= 0.0 {
            continue;
        }
        let sweep = v / total * std::f64::consts::TAU;
        let a1 = a0 + sweep;
        let mid = f64::midpoint(a0, a1);
        let ex = f64::from(s.explosion) / 100.0;
        let off = (mid.sin() * ex, mid.cos() * ex);
        let c = s.pts.get(i).copied().unwrap_or(s.color);
        let steps = ((sweep / std::f64::consts::TAU * 72.0).ceil() as usize).max(2);
        let arc: Vec<f64> = (0..=steps)
            .map(|k| a0 + sweep * k as f64 / steps as f64)
            .collect();
        let centre_top = [off.0, thick, off.1];
        let mut top = vec![centre_top];
        top.extend(arc.iter().map(|a| at(*a, 1.0, thick, off)));
        let mut bottom = vec![[off.0, 0.0, off.1]];
        bottom.extend(arc.iter().map(|a| at(*a, 1.0, 0.0, off)));
        tops.push(vec![Face::solid(top, c, [0.0, 1.0, 0.0])]);
        let mut faces = vec![Face::solid(bottom, c, [0.0, -1.0, 0.0])];
        for w in arc.windows(2) {
            let m = f64::midpoint(w[0], w[1]);
            faces.push(Face::solid(
                vec![
                    at(w[0], 1.0, 0.0, off),
                    at(w[1], 1.0, 0.0, off),
                    at(w[1], 1.0, thick, off),
                    at(w[0], 1.0, thick, off),
                ],
                c,
                [m.sin(), 0.0, m.cos()],
            ));
        }
        for (a, turn) in [
            (a0, -std::f64::consts::FRAC_PI_2),
            (a1, std::f64::consts::FRAC_PI_2),
        ] {
            let n = a + turn;
            faces.push(Face::solid(
                vec![
                    [off.0, 0.0, off.1],
                    at(a, 1.0, 0.0, off),
                    at(a, 1.0, thick, off),
                    [off.0, thick, off.1],
                ],
                c,
                [n.sin(), 0.0, n.cos()],
            ));
        }
        sides.push(faces);
        a0 = a1;
    }
    // A pie is seen from above with its tilt; the turn is the first slice's angle.
    let cam_view = View3D {
        rot_y: 0,
        r_ang_ax: false,
        rot_x: view.rot_x.clamp(10, 90),
        ..view.clone()
    };
    let scene = Scene {
        cam: Cam::new(&cam_view, 2.5),
        centre: [0.0, thick / 2.0, 0.0],
        groups: vec![sides.into_iter().flatten().map(|f| vec![f]).collect(), tops],
    };
    scene.draw([l, t, r, b], out);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ser(vals: &[f64]) -> Ser {
        Ser {
            name: "S".into(),
            cats: (1..=vals.len()).map(|i| format!("C{i}")).collect(),
            vals: vals.iter().map(|v| Some(*v)).collect(),
            color: Rgba(0x44, 0x72, 0xC4, 255),
            pts: vec![Rgba(0x44, 0x72, 0xC4, 255); vals.len()],
            sizes: vec![],
            explosion: 0,
            levels: vec![],
            subtotals: vec![],
            trendlines: vec![],
            err_bars: None,
            hidden: vec![],
            opts: crate::chart::elements::SerOpts::default(),
        }
    }

    // source: projection geometry — a point straight ahead stays put; rotY turns the depth to the right,
    // rotX raises it; right-angle axes shift the depth by (sin rotY, sin rotX) per unit.
    #[test]
    fn camera_turns_and_tilts() {
        let v = View3D {
            rot_x: 0,
            rot_y: 90,
            r_ang_ax: false,
            perspective: 0,
            ..View3D::default()
        };
        let c = Cam::new(&v, 10.0);
        let p = c.view([0.0, 0.0, 1.0]);
        assert!(
            (p[0] - 1.0).abs() < 1e-12 && p[1].abs() < 1e-12 && p[2].abs() < 1e-12,
            "{p:?}"
        );
        let v = View3D {
            rot_x: 90,
            rot_y: 0,
            r_ang_ax: false,
            perspective: 0,
            ..View3D::default()
        };
        let p = Cam::new(&v, 10.0).view([0.0, 0.0, 1.0]);
        assert!(p[0].abs() < 1e-12 && (p[1] - 1.0).abs() < 1e-12, "{p:?}");
        let v = View3D {
            rot_x: 30,
            rot_y: 30,
            r_ang_ax: true,
            ..View3D::default()
        };
        let p = Cam::new(&v, 10.0).view([1.0, 2.0, 2.0]);
        assert!(
            (p[0] - 2.0).abs() < 1e-12 && (p[1] - 3.0).abs() < 1e-12,
            "{p:?}"
        );
        // Perspective: nearer points are drawn further from the centre.
        let v = View3D {
            r_ang_ax: false,
            perspective: 60,
            ..View3D::default()
        };
        let c = Cam::new(&v, 10.0);
        assert!(c.view([1.0, 0.0, -2.0])[0] > c.view([1.0, 0.0, 2.0])[0]);
    }

    fn paths(out: &[Command]) -> usize {
        out.iter()
            .filter(|c| matches!(c, Command::Path { fill: Some(_), .. }))
            .count()
    }

    #[test]
    fn a_box_shows_three_faces() {
        let mut faces = Vec::new();
        prism(
            (0.5, 0.5),
            (0.2, 0.2),
            (0.0, 1.0),
            4,
            1.0,
            Rgba(200, 0, 0, 255),
            &|p| p,
            &mut faces,
        );
        assert_eq!(faces.len(), 6);
        let v = View3D {
            rot_x: 15,
            rot_y: 20,
            r_ang_ax: false,
            perspective: 0,
            ..View3D::default()
        };
        let scene = Scene {
            cam: Cam::new(&v, 2.0),
            centre: [0.5, 0.5, 0.5],
            groups: vec![vec![faces]],
        };
        let mut out = Vec::new();
        scene.draw([0.0, 0.0, 100.0, 100.0], &mut out);
        // Seen from the front, above and the left: front, top and left side (rotY turns the right
        // side away).
        assert_eq!(paths(&out), 3);
        // Right-angle axes seen from the front only: one face.
        let mut faces = Vec::new();
        prism(
            (0.5, 0.5),
            (0.2, 0.2),
            (0.0, 1.0),
            4,
            1.0,
            Rgba(200, 0, 0, 255),
            &|p| p,
            &mut faces,
        );
        let flat = Scene {
            cam: Cam::new(&View3D::default(), 2.0),
            centre: [0.5, 0.5, 0.5],
            groups: vec![vec![faces]],
        };
        let mut out = Vec::new();
        flat.draw([0.0, 0.0, 100.0, 100.0], &mut out);
        assert_eq!(paths(&out), 1);
    }

    #[test]
    fn charts_draw_solids_and_labels() {
        let ss = vec![ser(&[3.0, 5.0, 2.0]), ser(&[1.0, 4.0, 6.0])];
        let mut labels = Vec::new();
        let mut lab = |t: &str, _: [f64; 4], _: Align, _: f64, _: Rgba, _: &mut Vec<Command>| {
            labels.push(t.to_owned());
        };
        let color =
            |c: Option<&bo_model::sheet::ChartColor>, i: usize| super::super::default_color(c, i);
        let view = View3D::new_chart(false);
        for (kind, grouping, style) in [
            (ChartKind::Bar, "clustered", None),
            (ChartKind::Bar, "standard", Some("cylinder")),
            (ChartKind::Bar, "stacked", Some("pyramid")),
            (ChartKind::Line, "standard", None),
            (ChartKind::Area, "stacked", None),
            (ChartKind::Surface, "standard", None),
        ] {
            let plot = ChartPlot {
                kind: kind.clone(),
                grouping: Some(grouping.into()),
                style: style.map(str::to_owned),
                three_d: true,
                ..ChartPlot::default()
            };
            let mut out = Vec::new();
            draw(
                &plot,
                &ss,
                &view,
                [0.0, 0.0, 300.0, 200.0],
                9.0,
                &Locale::EN_US,
                &color,
                &mut lab,
                &mut out,
            );
            // Three walls, then the solids: a surface of 3 categories × 2 series has 2 cells.
            let min = if kind == ChartKind::Surface { 5 } else { 7 };
            assert!(paths(&out) >= min, "{kind:?} {grouping}: {}", paths(&out));
        }
        let mut out = Vec::new();
        let pie = ChartPlot {
            kind: ChartKind::Pie,
            three_d: true,
            ..ChartPlot::default()
        };
        let mut s = ser(&[1.0, 1.0, 2.0]);
        s.explosion = 10;
        draw(
            &pie,
            &[s],
            &View3D::new_chart(true),
            [0.0, 0.0, 200.0, 200.0],
            9.0,
            &Locale::EN_US,
            &color,
            &mut lab,
            &mut out,
        );
        // Three tops and some of the sides.
        assert!(paths(&out) >= 6, "{}", paths(&out));
        drop(lab);
        // Value axis 0..8 (stacked 3 + 1 … 2 + 6 = 8) and the categories.
        assert!(
            labels.contains(&"8".to_owned()) && labels.contains(&"C2".to_owned()),
            "{labels:?}"
        );
    }
}

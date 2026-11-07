//! Chart → display list (basic bar / column / line / area / pie), drawn from the
//! cached values of a DrawingML chart part (ECMA-376 Part 1, §21.2).
//! Coordinates are local to the graphic frame (points).

use bo_drawingml::chart::{Chart, ChartKind, Series};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

/// Host callbacks: colour of series `i` (or data point `i` for varied pies),
/// and a text label drawn into `rect` = [l, t, r, b] with alignment `l|ctr|r`.
pub(crate) type LabelFn<'a> = dyn FnMut(&str, [f64; 4], &str, f64, &mut Vec<Command>) + 'a;

pub(crate) struct ChartHost<'a> {
    pub series_color: &'a dyn Fn(&Series, usize, Option<usize>) -> Rgba,
    pub label: &'a mut LabelFn<'a>,
}

const GRID: Rgba = Rgba(217, 217, 217, 255);
const AXIS: Rgba = Rgba(134, 134, 134, 255);

fn rect(x: f64, y: f64, w: f64, h: f64, c: Rgba) -> Command {
    Command::Path {
        segs: vec![
            PathSeg::M(x as f32, y as f32),
            PathSeg::L((x + w) as f32, y as f32),
            PathSeg::L((x + w) as f32, (y + h) as f32),
            PathSeg::L(x as f32, (y + h) as f32),
            PathSeg::Z,
        ],
        fill: Some(Paint::Solid { color: c }),
        stroke: None,
    }
}

fn line(pts: &[(f64, f64)], c: Rgba, width: f64) -> Command {
    let mut segs = Vec::with_capacity(pts.len());
    for (i, (x, y)) in pts.iter().enumerate() {
        segs.push(if i == 0 {
            PathSeg::M(*x as f32, *y as f32)
        } else {
            PathSeg::L(*x as f32, *y as f32)
        });
    }
    Command::Path {
        segs,
        fill: None,
        stroke: Some(Stroke {
            width: width as f32,
            color: c,
            dash: vec![],
            cap: "round".into(),
            join: "round".into(),
        }),
    }
}

/// "Nice" axis step for a range (1, 2, 5 × 10ⁿ).
fn nice_step(range: f64, ticks: f64) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let raw = range / ticks;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let k = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    k * mag
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Draw a chart into a `w × h` box.
pub(crate) fn draw_chart(
    chart: &Chart,
    w: f64,
    h: f64,
    host: &mut ChartHost<'_>,
    out: &mut Vec<Command>,
) {
    let font = (h / 22.0).clamp(6.0, 14.0);
    let mut top = 4.0;
    if let Some(t) = &chart.title {
        let th = font * 1.8;
        (host.label)(t, [0.0, top, w, top + th], "ctr", font * 1.4, out);
        top += th + 2.0;
    }
    let mut right = w - 4.0;
    let pie = matches!(chart.kind, ChartKind::Pie { .. });
    // Legend entries: series, or categories of the first series for pies.
    let entries: Vec<(String, Rgba)> = if pie {
        chart
            .series
            .first()
            .map(|s| {
                s.categories
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.clone(), (host.series_color)(s, 0, Some(i))))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        chart
            .series
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.clone(), (host.series_color)(s, i, None)))
            .collect()
    };
    if chart.legend && !entries.is_empty() {
        let lw = (w * 0.22).max(40.0);
        let row = font * 1.6;
        let total = row * entries.len() as f64;
        let mut y = top + ((h - top) - total).max(0.0) / 2.0;
        for (name, c) in &entries {
            let x = w - lw;
            out.push(rect(x, y + row * 0.3, font * 0.8, font * 0.8, *c));
            (host.label)(name, [x + font * 1.1, y, w, y + row], "l", font, out);
            y += row;
        }
        right = w - lw - 6.0;
    }
    let bottom = h - 4.0;
    if pie {
        draw_pie(chart, [6.0, top, right, bottom], host, out);
        return;
    }
    let (ChartKind::Bar { .. } | ChartKind::Line | ChartKind::Area) = chart.kind else {
        // Unsupported plot kind: frame only.
        out.push(line(
            &[
                (6.0, top),
                (right, top),
                (right, bottom),
                (6.0, bottom),
                (6.0, top),
            ],
            GRID,
            0.75,
        ));
        return;
    };
    draw_xy(chart, [6.0, top, right, bottom], font, host, out);
}

fn draw_xy(
    chart: &Chart,
    [l, t, r, b]: [f64; 4],
    font: f64,
    host: &mut ChartHost<'_>,
    out: &mut Vec<Command>,
) {
    let (horizontal, stacked, percent, gap) = match &chart.kind {
        ChartKind::Bar {
            horizontal,
            grouping,
            gap_width,
        } => (
            *horizontal,
            grouping == "stacked" || grouping == "percentStacked",
            grouping == "percentStacked",
            gap_width / 100.0,
        ),
        _ => (false, false, false, 1.5),
    };
    let ncat = chart
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);
    if ncat == 0 {
        return;
    }
    let cats: Vec<String> = (0..ncat)
        .map(|i| {
            chart
                .series
                .iter()
                .find_map(|s| s.categories.get(i).filter(|c| !c.is_empty()).cloned())
                .unwrap_or_else(|| (i + 1).to_string())
        })
        .collect();
    let val = |s: &Series, i: usize| s.values.get(i).copied().flatten().unwrap_or(0.0);
    let totals: Vec<f64> = (0..ncat)
        .map(|i| chart.series.iter().map(|s| val(s, i).abs()).sum::<f64>())
        .collect();
    let (mut vmin, mut vmax) = (0.0_f64, 0.0_f64);
    for (i, &total) in totals.iter().enumerate() {
        if stacked {
            let (mut pos, mut neg) = (0.0, 0.0);
            for s in &chart.series {
                let v = if percent && total > 0.0 {
                    val(s, i) / total * 100.0
                } else {
                    val(s, i)
                };
                if v >= 0.0 {
                    pos += v;
                } else {
                    neg += v;
                }
            }
            vmax = vmax.max(pos);
            vmin = vmin.min(neg);
        } else {
            for s in &chart.series {
                vmax = vmax.max(val(s, i));
                vmin = vmin.min(val(s, i));
            }
        }
    }
    if vmax <= vmin {
        vmax = vmin + 1.0;
    }
    let step = nice_step(vmax - vmin, 5.0);
    let vmax = (vmax / step).ceil() * step;
    let vmin = (vmin / step).floor() * step;
    // Axis label space.
    let label_w = font * 3.5;
    let label_h = font * 1.6;
    let (pl, pt, pr, pb) = if horizontal {
        (l + label_w * 1.3, t, r, b - label_h)
    } else {
        (l + label_w, t + font * 0.5, r, b - label_h)
    };
    let (pw, ph) = ((pr - pl).max(1.0), (pb - pt).max(1.0));
    // Value → offset along the value axis.
    let vpos = |v: f64| (v - vmin) / (vmax - vmin);
    // Gridlines and value labels.
    let mut v = vmin;
    let mut guard = 0;
    while v <= vmax + step * 0.5 && guard < 50 {
        let f = vpos(v);
        let label = fmt_num(v);
        if horizontal {
            let x = pl + f * pw;
            out.push(line(&[(x, pt), (x, pb)], GRID, 0.5));
            (host.label)(
                &label,
                [x - label_w / 2.0, pb, x + label_w / 2.0, b],
                "ctr",
                font,
                out,
            );
        } else {
            let y = pb - f * ph;
            out.push(line(&[(pl, y), (pr, y)], GRID, 0.5));
            (host.label)(
                &label,
                [l, y - label_h / 2.0, pl - 3.0, y + label_h / 2.0],
                "r",
                font,
                out,
            );
        }
        v += step;
        guard += 1;
    }
    // Category axis.
    let slot = if horizontal { ph } else { pw } / ncat as f64;
    let zero = vpos(0.0_f64.clamp(vmin, vmax));
    if horizontal {
        let x = pl + zero * pw;
        out.push(line(&[(x, pt), (x, pb)], AXIS, 0.75));
    } else {
        let y = pb - zero * ph;
        out.push(line(&[(pl, y), (pr, y)], AXIS, 0.75));
    }
    for (i, c) in cats.iter().enumerate() {
        let c0 = i as f64 * slot;
        if horizontal {
            // Categories run bottom-up on a horizontal bar chart (§21.2.2.37 orientation minMax).
            let y = pb - c0 - slot;
            (host.label)(c, [l, y, pl - 3.0, y + slot], "r", font, out);
        } else {
            (host.label)(c, [pl + c0, pb + 1.0, pl + c0 + slot, b], "ctr", font, out);
        }
    }
    let nser = chart.series.len().max(1);
    match chart.kind {
        ChartKind::Bar { .. } => {
            let group = slot / (1.0 + gap);
            let bar = if stacked { group } else { group / nser as f64 };
            for (i, &total) in totals.iter().enumerate() {
                let (mut pos, mut neg) = (0.0, 0.0);
                for (si, s) in chart.series.iter().enumerate() {
                    let mut v = val(s, i);
                    if percent && total > 0.0 {
                        v = v / total * 100.0;
                    }
                    let (from, to) = if stacked {
                        let base = if v >= 0.0 { pos } else { neg };
                        if v >= 0.0 {
                            pos += v;
                        } else {
                            neg += v;
                        }
                        (base, base + v)
                    } else {
                        (0.0, v)
                    };
                    let (a, z) = (vpos(from), vpos(to));
                    let (lo, hi) = (a.min(z), a.max(z));
                    let off = i as f64 * slot
                        + (slot - group) / 2.0
                        + if stacked { 0.0 } else { si as f64 * bar };
                    let pt_color = s.point_fills.iter().any(|(p, _)| *p == i);
                    let c = (host.series_color)(s, si, pt_color.then_some(i));
                    if horizontal {
                        out.push(rect(pl + lo * pw, pb - off - bar, (hi - lo) * pw, bar, c));
                    } else {
                        out.push(rect(pl + off, pb - hi * ph, bar, (hi - lo) * ph, c));
                    }
                }
            }
        }
        ChartKind::Line | ChartKind::Area => {
            for (si, s) in chart.series.iter().enumerate() {
                let pts: Vec<(f64, f64)> = (0..ncat)
                    .map(|i| (pl + (i as f64 + 0.5) * slot, pb - vpos(val(s, i)) * ph))
                    .collect();
                let c = (host.series_color)(s, si, None);
                if chart.kind == ChartKind::Area {
                    let mut segs = vec![PathSeg::M(pts[0].0 as f32, (pb - zero * ph) as f32)];
                    segs.extend(pts.iter().map(|(x, y)| PathSeg::L(*x as f32, *y as f32)));
                    segs.push(PathSeg::L(
                        pts[pts.len() - 1].0 as f32,
                        (pb - zero * ph) as f32,
                    ));
                    segs.push(PathSeg::Z);
                    out.push(Command::Path {
                        segs,
                        fill: Some(Paint::Solid { color: c }),
                        stroke: None,
                    });
                } else {
                    out.push(line(&pts, c, 2.25));
                }
            }
        }
        _ => {}
    }
}

/// Wedge from angle `a0` sweeping `sw` (radians, clockwise from 12 o'clock) as cubic Béziers.
fn wedge(cx: f64, cy: f64, r: f64, inner: f64, a0: f64, sw: f64) -> Vec<PathSeg> {
    let pt = |rad: f64, a: f64| (cx + rad * a.sin(), cy - rad * a.cos());
    let arc = |segs: &mut Vec<PathSeg>, rad: f64, from: f64, sweep: f64| {
        let n = (sweep.abs() / (std::f64::consts::FRAC_PI_2))
            .ceil()
            .max(1.0) as usize;
        let d = sweep / n as f64;
        let k = 4.0 / 3.0 * (d / 4.0).tan() * rad;
        for i in 0..n {
            let s = from + d * i as f64;
            let e = s + d;
            let p0 = pt(rad, s);
            let p3 = pt(rad, e);
            // Tangent of (sin a, −cos a) is (cos a, sin a).
            let c1 = (p0.0 + k * s.cos(), p0.1 + k * s.sin());
            let c2 = (p3.0 - k * e.cos(), p3.1 - k * e.sin());
            segs.push(PathSeg::C(
                c1.0 as f32,
                c1.1 as f32,
                c2.0 as f32,
                c2.1 as f32,
                p3.0 as f32,
                p3.1 as f32,
            ));
        }
    };
    let mut segs = Vec::new();
    if inner > 0.0 {
        let s = pt(r, a0);
        segs.push(PathSeg::M(s.0 as f32, s.1 as f32));
        arc(&mut segs, r, a0, sw);
        let e = pt(inner, a0 + sw);
        segs.push(PathSeg::L(e.0 as f32, e.1 as f32));
        arc(&mut segs, inner, a0 + sw, -sw);
    } else {
        segs.push(PathSeg::M(cx as f32, cy as f32));
        let s = pt(r, a0);
        segs.push(PathSeg::L(s.0 as f32, s.1 as f32));
        arc(&mut segs, r, a0, sw);
    }
    segs.push(PathSeg::Z);
    segs
}

fn draw_pie(
    chart: &Chart,
    [l, t, r, b]: [f64; 4],
    host: &mut ChartHost<'_>,
    out: &mut Vec<Command>,
) {
    let Some(s) = chart.series.first() else {
        return;
    };
    let doughnut = matches!(chart.kind, ChartKind::Pie { doughnut: true });
    let vals: Vec<f64> = s.values.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = vals.iter().sum();
    if total <= 0.0 {
        return;
    }
    let rad = ((r - l).min(b - t) / 2.0 - 2.0).max(1.0);
    let (cx, cy) = (f64::midpoint(l, r), f64::midpoint(t, b));
    let inner = if doughnut { rad * 0.5 } else { 0.0 };
    let mut a = 0.0;
    for (i, v) in vals.iter().enumerate() {
        let sw = v / total * std::f64::consts::TAU;
        if sw <= 0.0 {
            continue;
        }
        let c = (host.series_color)(s, 0, Some(i));
        out.push(Command::Path {
            segs: wedge(cx, cy, rad, inner, a, sw),
            fill: Some(Paint::Solid { color: c }),
            stroke: Some(Stroke {
                width: 0.75,
                color: Rgba(255, 255, 255, 255),
                dash: vec![],
                cap: "butt".into(),
                join: "round".into(),
            }),
        });
        a += sw;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(10.0, 5.0), 2.0);
        assert_eq!(nice_step(3.0, 5.0), 1.0);
        assert_eq!(nice_step(470.0, 5.0), 100.0);
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(3.0), "3");
    }

    #[test]
    fn wedge_closes() {
        let w = wedge(0.0, 0.0, 10.0, 0.0, 0.0, std::f64::consts::PI);
        assert!(matches!(w.last(), Some(PathSeg::Z)));
        // Half pie: ends at 6 o'clock.
        let Some(PathSeg::C(_, _, _, _, x, y)) = w.get(w.len() - 2) else {
            panic!()
        };
        assert!(x.abs() < 1e-4 && (y - 10.0).abs() < 1e-4);
    }
}

//! Chart element options resolved for drawing (CORE-CHART-005): value axis scales with fixed
//! bounds / units / logarithms, tick labels with number formats and display units, minor
//! gridlines and tick marks, reversed axes (the plot mirrored), series markers and smooth lines,
//! data labels of series and single points, and trend lines through a set intercept.
//!
//! Elements: ECMA-376 Part 1, §21.2.2 — `c:scaling` (logBase, orientation, min, max),
//! `c:majorUnit` / `c:minorUnit`, `c:dispUnits`, `c:numFmt`, `c:majorTickMark` /
//! `c:minorTickMark`, `c:minorGridlines`, `c:marker`, `c:smooth`, `c:dLbls` / `c:dLbl`,
//! `c:trendline/c:intercept`.

use super::{auto_scale, circle, fmt_num, nice_step, Align, Ser, AXIS};
use crate::numfmt::{FormatOptions, Locale};
use bo_model::chart_style::DataLabels;
use bo_model::sheet::{AxisOptions, ChartSeries, Marker};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

/// Colour of minor gridlines drawn in the default look (lighter than the major ones).
pub(crate) const GRID_MINOR: Rgba = Rgba(0xEE, 0xF0, 0xF2, 255);
/// Length of a tick mark on one side of its axis (points).
const TICK: f64 = 3.0;

/// Options of one series beyond its data and colours.
#[derive(Debug, Clone, Default)]
pub(crate) struct SerOpts {
    /// Line width in points; `None` = the chart's.
    pub line_w: Option<f64>,
    /// Marker of its own; `None` = the chart's.
    pub marker: Option<Marker>,
    /// smooth.
    pub smooth: bool,
    /// Data labels of the series; `None` = the chart's.
    pub labels: Option<DataLabels>,
    /// Data labels of single points.
    pub point_labels: Vec<(usize, DataLabels)>,
}

impl SerOpts {
    pub(crate) fn of(s: &ChartSeries) -> SerOpts {
        SerOpts {
            line_w: s
                .line_w
                .map(|w| (f64::from(w) / 12_700.0).clamp(0.25, 12.0)),
            marker: s.marker.clone(),
            smooth: s.smooth,
            labels: s.labels.clone(),
            point_labels: s.point_labels.clone(),
        }
    }
}

/// The scale of a value axis: bounds and major unit in the axis' own space (the logarithm of the
/// values on a §21.2.2.100 logBase axis).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ValScale {
    pub lo: f64,
    pub hi: f64,
    pub step: f64,
    pub log: Option<f64>,
    /// Slack of the last tick as a share of the step: half a step on an automatic scale (whose
    /// maximum is a whole number of steps), next to nothing when a bound or the unit is fixed.
    tol: f64,
}

impl ValScale {
    pub(crate) fn tf(&self, v: f64) -> f64 {
        match self.log {
            Some(base) => v.max(1e-300).log(base),
            None => v,
        }
    }

    pub(crate) fn untf(&self, t: f64) -> f64 {
        match self.log {
            Some(base) => base.powf(t),
            None => t,
        }
    }

    /// Position of value `v` along the axis (0 = minimum, 1 = maximum). On a log axis values at
    /// or below the axis minimum (bars from zero) sit at the minimum.
    pub(crate) fn pos(&self, v: f64) -> f64 {
        let f = (self.tf(v) - self.lo) / (self.hi - self.lo);
        if self.log.is_some() {
            f.max(0.0)
        } else {
            f
        }
    }

    /// Major ticks in the axis' space, from the minimum upwards.
    pub(crate) fn ticks(&self) -> Vec<f64> {
        let mut out = Vec::new();
        let mut tv = self.lo;
        while tv <= self.hi + self.step * self.tol && out.len() < 60 {
            out.push(tv);
            tv += self.step;
        }
        out
    }

    /// Position of a tick of [`Self::ticks`] along the axis (0..=1).
    pub(crate) fn frac(&self, tv: f64) -> f64 {
        (tv - self.lo) / (self.hi - self.lo)
    }

    /// Positions (0..1) of the minor divisions: every `unit` (a fifth of the major unit by default),
    /// the multiples 2, 3, … of each power on a log axis; those on a major tick are left out.
    pub(crate) fn minor(&self, unit: Option<f64>) -> Vec<f64> {
        let mut out = Vec::new();
        let span = self.hi - self.lo;
        if let Some(b) = self.log {
            let n = (b.round() as i64).clamp(2, 20);
            let mut d = self.lo.floor();
            while d < self.hi && out.len() < 600 {
                for k in 2..n {
                    let t = d + (k as f64).log(b);
                    if t > self.lo && t < self.hi {
                        out.push((t - self.lo) / span);
                    }
                }
                d += 1.0;
            }
            return out;
        }
        let u = unit
            .filter(|u| u.is_finite() && *u > 0.0 && span / *u <= 600.0)
            .unwrap_or(self.step / 5.0);
        if u <= 0.0 || !u.is_finite() {
            return out;
        }
        let mut k = 1.0;
        while out.len() < 600 {
            let t = self.lo + u * k;
            if t >= self.hi - u * 1e-6 {
                break;
            }
            let m = (t - self.lo) / self.step;
            if (m - m.round()).abs() > 1e-6 {
                out.push((t - self.lo) / span);
            }
            k += 1.0;
        }
        out
    }
}

/// The scale of a value axis over the data range `vmin`..`vmax` (`positive_min`: the smallest
/// positive value, `f64::MAX` without one) with the fixed bounds, logarithm base and major unit of
/// the axis (§21.2.2.160 scaling, majorUnit).
pub(crate) fn val_scale(
    vmin: f64,
    vmax: f64,
    positive_min: f64,
    (min, max): (Option<f64>, Option<f64>),
    log: Option<f64>,
    major: Option<f64>,
) -> ValScale {
    let log = log.filter(|b| *b > 1.0 && b.is_finite());
    let fixed = min.is_some() || max.is_some() || (major.is_some() && log.is_none());
    let mut s = ValScale {
        lo: 0.0,
        hi: 1.0,
        step: 1.0,
        log,
        tol: if fixed { 1e-6 } else { 0.5 },
    };
    let (lo, hi, step) = if log.is_some() {
        let lo = match min.filter(|m| *m > 0.0) {
            Some(m) => s.tf(m),
            None if positive_min < f64::MAX => s.tf(positive_min).floor(),
            None => 0.0,
        };
        let hi = match max.filter(|m| *m > 0.0) {
            Some(m) => s.tf(m),
            None => s.tf(vmax.max(s.untf(lo + 1.0))).ceil().max(lo + 1.0),
        };
        (lo, if hi > lo { hi } else { lo + 1.0 }, 1.0)
    } else {
        let major = major.filter(|u| u.is_finite() && *u > 0.0);
        let (lo0, hi0) = (min.unwrap_or(vmin), max.unwrap_or(vmax));
        match major.filter(|u| (hi0 - lo0).abs() / *u <= 400.0) {
            Some(u) => (
                min.unwrap_or((vmin / u).floor() * u),
                max.unwrap_or((vmax / u).ceil() * u),
                u,
            ),
            // A fixed bound sets the range the unit is chosen for.
            None if (min.is_some() || max.is_some()) && hi0 > lo0 => {
                let step = nice_step(hi0 - lo0, 5.0);
                (
                    min.unwrap_or((lo0 / step).floor() * step),
                    max.unwrap_or((hi0 / step).ceil() * step),
                    step,
                )
            }
            None => auto_scale(vmin, vmax, min, max),
        }
    };
    let (lo, hi) = if hi > lo { (lo, hi) } else { (lo, lo + step) };
    s.lo = lo;
    s.hi = hi;
    s.step = step;
    s
}

/// A number in a number format code (the general format when there is none).
pub(crate) fn fmt_code(v: f64, code: Option<&str>, locale: &Locale) -> String {
    match code.filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("general")) {
        Some(code) => {
            let opts = FormatOptions {
                locale: *locale,
                date1904: false,
            };
            crate::numfmt::format_number(v, code, &opts).text
        }
        None => fmt_num(v, locale),
    }
}

/// The tick label of value `v`: divided by the display unit (dispUnits), in the axis'
/// number format (numFmt).
pub(crate) fn fmt_tick(v: f64, o: &AxisOptions, locale: &Locale) -> String {
    fmt_code(v / o.unit_factor(), o.num_fmt.as_deref(), locale)
}

/// The display unit label of an axis (dispUnitsLbl): BlueOffice writes the factor as a
/// power of ten (`×10^3`), which needs no translation.
pub(crate) fn unit_label(o: &AxisOptions) -> Option<String> {
    let f = o.unit_factor();
    (o.disp_unit_label && f > 1.0).then(|| format!("×10^{}", f.log10().round() as i64))
}

/// A tick mark (majorTickMark / minorTickMark: `in`, `out`, `cross`) at
/// `(x, y)` on an axis; `along_x`: the axis is horizontal (the tick is vertical). `inward` is the
/// direction (±1) from the axis into the plot.
pub(crate) fn tick(
    kind: Option<&str>,
    (x, y): (f64, f64),
    along_x: bool,
    inward: f64,
    len: f64,
    out: &mut Vec<Command>,
) {
    let (a, z) = match kind {
        Some("in") => (0.0, inward * len),
        Some("out") => (-inward * len, 0.0),
        Some("cross") => (-inward * len, inward * len),
        _ => return,
    };
    let (x1, y1, x2, y2) = if along_x {
        (x, y + a, x, y + z)
    } else {
        (x + a, y, x + z, y)
    };
    out.push(Command::Line {
        x1: x1 as f32,
        y1: y1 as f32,
        x2: x2 as f32,
        y2: y2 as f32,
        width: 0.75,
        color: AXIS,
        dash: None,
    });
}

/// Length of a major / minor tick mark.
pub(crate) fn tick_len(minor: bool) -> f64 {
    if minor {
        TICK * 0.6
    } else {
        TICK
    }
}

/// A reversed axis (orientation `maxMin`) is drawn by mirroring the plot — shapes,
/// lines and the boxes of the labels — about the centre of its area, so the axis it crosses moves
/// to the opposite side with it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mirror {
    /// `left + right` of the area when mirrored horizontally.
    pub x: Option<f64>,
    /// `top + bottom` of the area when mirrored vertically.
    pub y: Option<f64>,
}

impl Mirror {
    pub(crate) fn any(&self) -> bool {
        self.x.is_some() || self.y.is_some()
    }

    pub(crate) fn rect(&self, [l, t, r, b]: [f64; 4]) -> [f64; 4] {
        let (l, r) = self.x.map_or((l, r), |s| (s - r, s - l));
        let (t, b) = self.y.map_or((t, b), |s| (s - b, s - t));
        [l, t, r, b]
    }

    pub(crate) fn align(&self, a: Align) -> Align {
        match (self.x, a) {
            (Some(_), Align::Left) => Align::Right,
            (Some(_), Align::Right) => Align::Left,
            _ => a,
        }
    }

    /// Mirrors the shapes of `cmds`; the index ranges `skip` (label glyphs, already placed in
    /// mirrored boxes) stay.
    pub(crate) fn apply(&self, cmds: &mut [Command], skip: &[(usize, usize)]) {
        let fx = |x: f32| self.x.map_or(x, |s| s as f32 - x);
        let fy = |y: f32| self.y.map_or(y, |s| s as f32 - y);
        for (i, cmd) in cmds.iter_mut().enumerate() {
            if skip.iter().any(|(a, z)| i >= *a && i < *z) {
                continue;
            }
            match cmd {
                Command::FillRect { x, y, w, h, .. } => {
                    if self.x.is_some() {
                        *x = fx(*x) - *w;
                    }
                    if self.y.is_some() {
                        *y = fy(*y) - *h;
                    }
                }
                Command::Line { x1, y1, x2, y2, .. } => {
                    (*x1, *x2) = (fx(*x1), fx(*x2));
                    (*y1, *y2) = (fy(*y1), fy(*y2));
                }
                Command::Path { segs, fill, .. } => {
                    for s in segs.iter_mut() {
                        *s = match *s {
                            PathSeg::M(x, y) => PathSeg::M(fx(x), fy(y)),
                            PathSeg::L(x, y) => PathSeg::L(fx(x), fy(y)),
                            PathSeg::Q(a, b, x, y) => PathSeg::Q(fx(a), fy(b), fx(x), fy(y)),
                            PathSeg::C(a, b, c, d, x, y) => {
                                PathSeg::C(fx(a), fy(b), fx(c), fy(d), fx(x), fy(y))
                            }
                            PathSeg::Z => PathSeg::Z,
                        };
                    }
                    if let Some(Paint::Linear { x1, y1, x2, y2, .. }) = fill {
                        (*x1, *x2) = (fx(*x1), fx(*x2));
                        (*y1, *y2) = (fy(*y1), fy(*y2));
                    }
                }
                _ => {}
            }
        }
    }
}

fn line_stroke(c: Rgba, w: f64) -> Stroke {
    Stroke {
        width: w as f32,
        color: c,
        dash: vec![],
        cap: "round".into(),
        join: "round".into(),
    }
}

/// A straight piece of a marker: from one point to another.
type Segment = ((f64, f64), (f64, f64));

/// One marker (§21.2.2.174 symbol) of `size` points centred on `(x, y)`.
pub(crate) fn draw_marker(
    symbol: &str,
    (x, y): (f64, f64),
    size: f64,
    c: Rgba,
    out: &mut Vec<Command>,
) {
    let r = (size / 2.0).clamp(1.0, 36.0);
    let white = Some(line_stroke(Rgba(255, 255, 255, 255), 0.75));
    let fill = Some(Paint::Solid { color: c });
    let poly = |pts: &[(f64, f64)]| -> Vec<PathSeg> {
        let mut segs: Vec<PathSeg> = pts
            .iter()
            .enumerate()
            .map(|(i, (px, py))| {
                if i == 0 {
                    PathSeg::M(*px as f32, *py as f32)
                } else {
                    PathSeg::L(*px as f32, *py as f32)
                }
            })
            .collect();
        segs.push(PathSeg::Z);
        segs
    };
    let strokes = |lines: &[Segment], out: &mut Vec<Command>| {
        let mut segs = Vec::new();
        for (a, z) in lines {
            segs.push(PathSeg::M(a.0 as f32, a.1 as f32));
            segs.push(PathSeg::L(z.0 as f32, z.1 as f32));
        }
        out.push(Command::Path {
            segs,
            fill: None,
            stroke: Some(line_stroke(c, (r * 0.45).clamp(1.0, 2.5))),
        });
    };
    let k = r * std::f64::consts::FRAC_1_SQRT_2;
    match symbol {
        "none" => {}
        "square" => out.push(Command::Path {
            segs: poly(&[
                (x - r, y - r),
                (x + r, y - r),
                (x + r, y + r),
                (x - r, y + r),
            ]),
            fill,
            stroke: white,
        }),
        "diamond" => out.push(Command::Path {
            segs: poly(&[
                (x, y - r * 1.2),
                (x + r * 1.2, y),
                (x, y + r * 1.2),
                (x - r * 1.2, y),
            ]),
            fill,
            stroke: white,
        }),
        "triangle" => out.push(Command::Path {
            segs: poly(&[
                (x, y - r * 1.15),
                (x + r * 1.15, y + r),
                (x - r * 1.15, y + r),
            ]),
            fill,
            stroke: white,
        }),
        "x" => strokes(
            &[
                ((x - k, y - k), (x + k, y + k)),
                ((x - k, y + k), (x + k, y - k)),
            ],
            out,
        ),
        "plus" => strokes(&[((x - r, y), (x + r, y)), ((x, y - r), (x, y + r))], out),
        "star" => strokes(
            &[
                ((x - k, y - k), (x + k, y + k)),
                ((x - k, y + k), (x + k, y - k)),
                ((x, y - r), (x, y + r)),
            ],
            out,
        ),
        "dash" => strokes(&[((x - r, y), (x + r, y))], out),
        "dot" => out.push(Command::Path {
            segs: circle(x, y, (r * 0.5).max(1.0)),
            fill,
            stroke: None,
        }),
        // circle, auto
        _ => out.push(Command::Path {
            segs: circle(x, y, r),
            fill,
            stroke: white,
        }),
    }
}

/// The markers of a series with a marker of its own; `false` when the chart's look applies.
pub(crate) fn own_markers(s: &Ser, pts: &[(f64, f64)], out: &mut Vec<Command>) -> bool {
    let Some(m) = &s.opts.marker else {
        return false;
    };
    let size = f64::from(m.size.unwrap_or(5).clamp(2, 72));
    for p in pts {
        draw_marker(&m.symbol, *p, size, s.color, out);
    }
    true
}

/// A smooth curve through `pts` (smooth): a Catmull-Rom spline as cubic Béziers.
pub(crate) fn smooth_path(pts: &[(f64, f64)]) -> Vec<PathSeg> {
    let n = pts.len();
    let mut segs = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = pts[i];
        if i == 0 {
            segs.push(PathSeg::M(x as f32, y as f32));
            continue;
        }
        let p0 = pts[i.saturating_sub(2)];
        let p1 = pts[i - 1];
        let p3 = pts[(i + 1).min(n - 1)];
        let c1 = (p1.0 + (x - p0.0) / 6.0, p1.1 + (y - p0.1) / 6.0);
        let c2 = (x - (p3.0 - p1.0) / 6.0, y - (p3.1 - p1.1) / 6.0);
        segs.push(PathSeg::C(
            c1.0 as f32,
            c1.1 as f32,
            c2.0 as f32,
            c2.1 as f32,
            x as f32,
            y as f32,
        ));
    }
    segs
}

/// The line of a series through `pts`: straight pieces, or a smooth curve.
pub(crate) fn series_line(s: &Ser, pts: &[(f64, f64)], width: f64) -> Command {
    if s.opts.smooth && pts.len() > 2 {
        Command::Path {
            segs: smooth_path(pts),
            fill: None,
            stroke: Some(line_stroke(s.color, width)),
        }
    } else {
        super::polyline(pts, s.color, width)
    }
}

/// The data label settings of point `i` of series `s` (dLbl of the point, else
/// §21.2.2.49 dLbls of the series, else the chart's); `None` when nothing is shown.
pub(crate) fn labels_at<'a>(
    chart: Option<&'a DataLabels>,
    s: &'a Ser,
    i: usize,
) -> Option<&'a DataLabels> {
    let d = s
        .opts
        .point_labels
        .iter()
        .find(|(k, _)| *k == i)
        .map(|(_, d)| d)
        .or(s.opts.labels.as_ref())
        .or(chart)?;
    d.any().then_some(d)
}

/// Text of a data label: series name, category, value and percentage joined by the separator
/// (`"; "` by default), numbers in the label's number format.
pub(crate) fn label_text(
    d: &DataLabels,
    series: &str,
    cat: &str,
    v: f64,
    percent: Option<f64>,
    locale: &Locale,
) -> Option<String> {
    let mut parts = Vec::new();
    if d.series {
        parts.push(series.to_owned());
    }
    // A point without a category name shows the other parts only.
    if d.category && !cat.is_empty() {
        parts.push(cat.to_owned());
    }
    let code = d.num_fmt.as_deref();
    if d.value {
        // A percentage format belongs to the percentage, not to the value beside it.
        let own = code.filter(|c| !(d.percent && percent.is_some() && c.contains('%')));
        parts.push(fmt_code(v, own, locale));
    }
    if let Some(p) = percent.filter(|_| d.percent) {
        parts.push(match code.filter(|c| c.contains('%')) {
            Some(c) => fmt_code(p, Some(c), locale),
            None => format!("{}%", (p * 100.0).round()),
        });
    }
    (!parts.is_empty()).then(|| parts.join(d.separator.as_deref().unwrap_or("; ")))
}

/// Least squares of `y − c0 = a₁x + … + a_k x^k` (a fit through the set intercept `c0`,
/// `c:trendline/c:intercept`): the coefficients `c0, a₁, …, a_k`, from the normal equations
/// solved by Gaussian elimination with partial pivoting. `None` when the system is singular.
pub(crate) fn fit_through(pts: &[(f64, f64)], c0: f64, k: usize) -> Option<Vec<f64>> {
    let k = k.clamp(1, 6);
    let mut a = vec![vec![0.0_f64; k + 1]; k];
    for (x, y) in pts {
        for (j, row) in a.iter_mut().enumerate() {
            let xj = x.powi(j as i32 + 1);
            for (m, cell) in row.iter_mut().take(k).enumerate() {
                *cell += xj * x.powi(m as i32 + 1);
            }
            row[k] += xj * (y - c0);
        }
    }
    for col in 0..k {
        let pivot = (col..k).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        for row in 0..k {
            if row != col {
                let f = a[row][col] / a[col][col];
                if f != 0.0 {
                    let src = a[col].clone();
                    for (cell, s) in a[row].iter_mut().zip(&src).skip(col) {
                        *cell -= f * s;
                    }
                }
            }
        }
    }
    let mut out = vec![c0];
    for (j, row) in a.iter().enumerate() {
        let v = row[k] / row[j];
        if !v.is_finite() {
            return None;
        }
        out.push(v);
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // source: the scale is arithmetic on the bounds — an automatic scale over 0..47 takes the
    // "nice" unit 10 (0, 10, … 50); a fixed unit of 25 gives 0, 25, 50; a fixed maximum of 500 on
    // a base-10 log axis from 1 ends at log10(500) with ticks at 1, 10, 100.
    #[test]
    fn scales_with_fixed_bounds_units_and_logarithms() {
        let auto = val_scale(0.0, 47.0, 1.0, (None, None), None, None);
        assert_eq!((auto.lo, auto.hi, auto.step), (0.0, 50.0, 10.0));
        assert_eq!(auto.ticks().len(), 6);
        let unit = val_scale(0.0, 47.0, 1.0, (None, None), None, Some(25.0));
        assert_eq!(unit.ticks(), vec![0.0, 25.0, 50.0]);
        assert_eq!(unit.minor(Some(12.5)).len(), 2);
        let fixed = val_scale(0.0, 47.0, 1.0, (Some(-20.0), Some(100.0)), None, None);
        assert_eq!((fixed.lo, fixed.hi, fixed.step), (-20.0, 100.0, 50.0));
        // -20, 30, 80: the last whole step below the fixed maximum.
        assert_eq!(fixed.ticks(), vec![-20.0, 30.0, 80.0]);
        let log = val_scale(0.0, 47.0, 1.0, (None, Some(500.0)), Some(10.0), None);
        assert_eq!(log.lo, 0.0);
        assert!((log.hi - 500.0_f64.log10()).abs() < 1e-12);
        assert_eq!(log.ticks(), vec![0.0, 1.0, 2.0]);
        assert!((log.pos(10.0) - 1.0 / 500.0_f64.log10()).abs() < 1e-12);
        // 2…9 × 1, 2…9 × 10 and 200, 300, 400 below 500.
        assert_eq!(log.minor(None).len(), 8 + 8 + 3);
        assert_eq!(log.pos(0.0), 0.0);
    }

    // source: dispUnits divide the labels; number format codes of §18.8.30.
    #[test]
    fn tick_labels_use_display_units_and_number_formats() {
        let vi = Locale::default();
        let o = AxisOptions {
            disp_unit: Some("thousands".into()),
            disp_unit_label: true,
            num_fmt: Some("0.0".into()),
            ..AxisOptions::default()
        };
        let text = fmt_tick(12_500.0, &o, &vi);
        assert!(text == "12,5" || text == "12.5", "{text}");
        assert_eq!(unit_label(&o).as_deref(), Some("×10^3"));
        assert_eq!(unit_label(&AxisOptions::default()), None);
        assert_eq!(fmt_tick(3.0, &AxisOptions::default(), &vi), "3");
    }

    // source: y = 3x + 1 through the set intercept 1 has slope 3; y = x² + 2 through 2 has the
    // coefficients (2, 0, 1); y = 2x forced through 4 minimises Σ(2x − 4 − ax)²: a = 2 − 4Σx/Σx².
    #[test]
    fn fits_through_a_set_intercept() {
        let line: Vec<(f64, f64)> = (1..=5)
            .map(|x| (f64::from(x), 3.0 * f64::from(x) + 1.0))
            .collect();
        let c = fit_through(&line, 1.0, 1).unwrap();
        assert!((c[0] - 1.0).abs() < 1e-12 && (c[1] - 3.0).abs() < 1e-9);
        let quad: Vec<(f64, f64)> = (1..=6)
            .map(|x| (f64::from(x), f64::from(x * x) + 2.0))
            .collect();
        let c = fit_through(&quad, 2.0, 2).unwrap();
        assert!(c[1].abs() < 1e-7 && (c[2] - 1.0).abs() < 1e-8, "{c:?}");
        let off: Vec<(f64, f64)> = (1..=4)
            .map(|x| (f64::from(x), 2.0 * f64::from(x)))
            .collect();
        let c = fit_through(&off, 4.0, 1).unwrap();
        // Σx = 10, Σx² = 30.
        assert!((c[1] - (2.0 - 40.0 / 30.0)).abs() < 1e-12);
        assert!(fit_through(&[(0.0, 1.0), (0.0, 2.0)], 0.0, 1).is_none());
    }

    // source: mirroring about the centre of [10, 110] maps x to 120 − x; a box keeps its width.
    #[test]
    fn mirrored_shapes_and_label_boxes() {
        let m = Mirror {
            x: Some(120.0),
            y: None,
        };
        assert_eq!(m.rect([10.0, 5.0, 30.0, 9.0]), [90.0, 5.0, 110.0, 9.0]);
        assert_eq!(m.align(Align::Left), Align::Right);
        let mut cmds = vec![
            super::super::rect(20.0, 0.0, 10.0, 4.0, AXIS),
            super::super::rect(20.0, 0.0, 10.0, 4.0, AXIS),
        ];
        m.apply(&mut cmds, &[(1, 2)]);
        assert!(matches!(cmds[0], Command::FillRect { x, w, .. } if x == 90.0 && w == 10.0));
        assert!(matches!(cmds[1], Command::FillRect { x, .. } if x == 20.0));
    }

    // source: a Catmull-Rom spline passes through its points.
    #[test]
    fn smooth_lines_pass_through_the_points() {
        let pts = [(0.0, 0.0), (10.0, 5.0), (20.0, 0.0), (30.0, 8.0)];
        let segs = smooth_path(&pts);
        assert_eq!(segs.len(), 4);
        for (seg, p) in segs.iter().zip(&pts).skip(1) {
            assert!(
                matches!(seg, PathSeg::C(_, _, _, _, x, y) if (*x, *y) == (p.0 as f32, p.1 as f32))
            );
        }
    }
}

//! Least-squares regression (`LINEST`, `LOGEST`, `TREND`, `GROWTH`; ECMA-376 Part 1, §18.17.7)
//! and exponential-smoothing forecasts (`FORECAST.ETS*`, [MS-XLSX] future functions).
//!
//! Regression solves the normal equations by Gauss–Jordan elimination with partial pivoting;
//! a variable that is a linear combination of earlier ones gets coefficient 0. Forecasting uses
//! additive Holt–Winters smoothing (level, trend, season) with the smoothing constants chosen to
//! minimise the one-step squared error.

use super::stats::norm_s_inv_f;
use super::{f, flatten, num, opt_bool, opt_num, shape, FnSpec};
use crate::eval::{Ctx, R};
use crate::value::{Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("LINEST", 1, 4, linest),
    f("LOGEST", 1, 4, logest),
    f("TREND", 1, 4, trend),
    f("GROWTH", 1, 4, growth),
    f("FORECAST.ETS", 3, 6, forecast_ets),
    f("FORECAST.ETS.CONFINT", 3, 7, forecast_ets_confint),
    f("FORECAST.ETS.SEASONALITY", 2, 4, forecast_ets_seasonality),
    f("FORECAST.ETS.STAT", 3, 6, forecast_ets_stat),
];

// ------------------------------------------------------------------ data

/// Numbers of an argument in row-major order with its shape; any non-number is `#VALUE!`.
fn matrix(ctx: &Ctx, v: &Value) -> Result<(Vec<f64>, usize, usize), ErrorKind> {
    let (r, c) = shape(ctx, v);
    let mut out = Vec::with_capacity(r * c);
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => out.push(n),
            Value::Error(e) => return Err(e),
            _ => return Err(ErrorKind::Value),
        }
    }
    Ok((out, r, c))
}

/// Observations: y (n values) and the regressors as n rows of k values.
struct Data {
    y: Vec<f64>,
    x: Vec<Vec<f64>>,
    k: usize,
    /// y is one row (the x variables are rows of the x range).
    by_row: bool,
}

fn data(ctx: &Ctx, args: &[Value]) -> Result<Data, ErrorKind> {
    let (y, yr, yc) = matrix(ctx, &args[0])?;
    let n = y.len();
    let by_row = yr == 1 && yc > 1;
    let x = match args.get(1) {
        None | Some(Value::Empty) => (1..=n).map(|i| vec![i as f64]).collect(),
        Some(v) => {
            let (xs, xr, xc) = matrix(ctx, v)?;
            regressors(&xs, xr, xc, n, yr, yc)?
        }
    };
    let k = x.first().map_or(0, Vec::len);
    if n == 0 || k == 0 {
        return Err(ErrorKind::Value);
    }
    Ok(Data { y, x, k, by_row })
}

/// Split an x range into n observations: x shaped like y is one variable; otherwise the columns
/// (y a column) or the rows (y a row) are the variables.
fn regressors(
    xs: &[f64],
    xr: usize,
    xc: usize,
    n: usize,
    yr: usize,
    yc: usize,
) -> Result<Vec<Vec<f64>>, ErrorKind> {
    if xr == yr && xc == yc {
        return Ok(xs.iter().map(|v| vec![*v]).collect());
    }
    if yc == 1 && xr == n {
        return Ok((0..n).map(|i| xs[i * xc..(i + 1) * xc].to_vec()).collect());
    }
    if yr == 1 && xc == n {
        return Ok((0..n)
            .map(|i| (0..xr).map(|j| xs[j * xc + i]).collect())
            .collect());
    }
    Err(ErrorKind::Ref)
}

// ------------------------------------------------------------------ least squares

struct Fit {
    /// Coefficients of the x variables (in their order) and the constant.
    m: Vec<f64>,
    b: f64,
    /// Standard errors of `m` and of `b` (`None` without a constant).
    se_m: Vec<f64>,
    se_b: Option<f64>,
    r2: f64,
    se_y: f64,
    f: f64,
    df: f64,
    ss_reg: f64,
    ss_resid: f64,
}

/// Inverse of a symmetric matrix by Gauss–Jordan elimination; columns whose pivot vanishes are
/// reported as dropped (their rows and columns of the inverse are 0).
fn invert(mut a: Vec<Vec<f64>>) -> (Vec<Vec<f64>>, Vec<bool>) {
    let p = a.len();
    let mut inv: Vec<Vec<f64>> = (0..p)
        .map(|i| (0..p).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    let mut dropped = vec![false; p];
    let scale = (0..p)
        .map(|i| a[i][i].abs())
        .fold(0.0, f64::max)
        .max(1e-300);
    for col in 0..p {
        // Partial pivoting among the rows not yet used.
        let piv = (col..p)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap_or(col);
        if a[piv][col].abs() <= 1e-13 * scale {
            dropped[col] = true;
            for row in 0..p {
                a[row][col] = 0.0;
                a[col][row] = 0.0;
                inv[row][col] = 0.0;
                inv[col][row] = 0.0;
            }
            continue;
        }
        a.swap(col, piv);
        inv.swap(col, piv);
        let d = a[col][col];
        for j in 0..p {
            a[col][j] /= d;
            inv[col][j] /= d;
        }
        for row in 0..p {
            if row != col {
                let f = a[row][col];
                if f != 0.0 {
                    for j in 0..p {
                        a[row][j] -= f * a[col][j];
                        inv[row][j] -= f * inv[col][j];
                    }
                }
            }
        }
    }
    for (i, d) in dropped.iter().enumerate() {
        if *d {
            inv[i].fill(0.0);
            for row in &mut inv {
                row[i] = 0.0;
            }
        }
    }
    (inv, dropped)
}

fn least_squares(d: &Data, constant: bool) -> Result<Fit, ErrorKind> {
    let n = d.y.len();
    let p = d.k + usize::from(constant);
    let row = |i: usize| -> Vec<f64> {
        let mut r = d.x[i].clone();
        if constant {
            r.push(1.0);
        }
        r
    };
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for i in 0..n {
        let r = row(i);
        for a in 0..p {
            xty[a] += r[a] * d.y[i];
            for b in 0..p {
                xtx[a][b] += r[a] * r[b];
            }
        }
    }
    let (inv, dropped) = invert(xtx);
    let beta: Vec<f64> = (0..p)
        .map(|a| (0..p).map(|b| inv[a][b] * xty[b]).sum())
        .collect();
    let used = dropped.iter().filter(|d| !**d).count();
    let ss_resid: f64 = (0..n)
        .map(|i| {
            let r = row(i);
            let e = d.y[i] - r.iter().zip(&beta).map(|(x, b)| x * b).sum::<f64>();
            e * e
        })
        .sum();
    let ss_tot: f64 = if constant {
        let my = d.y.iter().sum::<f64>() / n as f64;
        d.y.iter().map(|y| (y - my) * (y - my)).sum()
    } else {
        d.y.iter().map(|y| y * y).sum()
    };
    let df = n as f64 - used as f64;
    let ss_reg = ss_tot - ss_resid;
    let se_y = if df > 0.0 {
        (ss_resid / df).sqrt()
    } else {
        0.0
    };
    let regressors = used - usize::from(constant && !dropped[p - 1]);
    let f = if df > 0.0 && regressors > 0 && ss_resid > 0.0 {
        (ss_reg / regressors as f64) / (ss_resid / df)
    } else {
        f64::INFINITY
    };
    let se = |a: usize| (inv[a][a] * se_y * se_y).max(0.0).sqrt();
    Ok(Fit {
        m: beta[..d.k].to_vec(),
        b: if constant { beta[p - 1] } else { 0.0 },
        se_m: (0..d.k).map(se).collect(),
        se_b: constant.then(|| se(p - 1)),
        r2: if ss_tot == 0.0 { 1.0 } else { ss_reg / ss_tot },
        se_y,
        f,
        df,
        ss_reg,
        ss_resid,
    })
}

/// The LINEST layout: coefficients in reverse variable order then the constant; with `stats`
/// four more rows (standard errors, r² and se(y), F and df, the sums of squares).
fn linest_array(fit: &Fit, k: usize, stats: bool, map: fn(f64) -> f64) -> Value {
    let cols = k + 1;
    let na = Value::Error(ErrorKind::NA);
    let numv = |x: f64| {
        if x.is_finite() {
            Value::number(x)
        } else {
            Value::Error(ErrorKind::Num)
        }
    };
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut first: Vec<Value> = fit.m.iter().rev().map(|m| numv(map(*m))).collect();
    first.push(numv(map(fit.b)));
    rows.push(first);
    if stats {
        let mut se: Vec<Value> = fit.se_m.iter().rev().map(|s| numv(*s)).collect();
        se.push(fit.se_b.map_or(na.clone(), numv));
        rows.push(se);
        let pad = |a: f64, b: f64| {
            let mut r = vec![numv(a), numv(b)];
            r.resize(cols, na.clone());
            r
        };
        rows.push(pad(fit.r2, fit.se_y));
        rows.push(pad(fit.f, fit.df));
        rows.push(pad(fit.ss_reg, fit.ss_resid));
        if cols == 1 {
            // A single column holds only the first value of each pair.
            for r in &mut rows {
                r.truncate(1);
            }
        }
    }
    Array::from_rows(rows).map_or(na, |a| Value::Array(a.into()))
}

fn ln_y(d: &mut Data) -> Result<(), ErrorKind> {
    for y in &mut d.y {
        if *y <= 0.0 {
            return Err(ErrorKind::Num);
        }
        *y = y.ln();
    }
    Ok(())
}

fn linest(ctx: &mut Ctx, args: &[Value]) -> R {
    let d = data(ctx, args)?;
    let constant = opt_bool(ctx, args, 2, true)?;
    let stats = opt_bool(ctx, args, 3, false)?;
    let fit = least_squares(&d, constant)?;
    Ok(linest_array(&fit, d.k, stats, |x| x))
}

fn logest(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut d = data(ctx, args)?;
    ln_y(&mut d)?;
    let constant = opt_bool(ctx, args, 2, true)?;
    let stats = opt_bool(ctx, args, 3, false)?;
    let fit = least_squares(&d, constant)?;
    Ok(linest_array(&fit, d.k, stats, f64::exp))
}

/// Fitted values at `new_x` (default: the known x), shaped like the new x observations.
fn predict(ctx: &Ctx, args: &[Value], log: bool) -> R {
    let mut d = data(ctx, args)?;
    if log {
        ln_y(&mut d)?;
    }
    let constant = opt_bool(ctx, args, 3, true)?;
    let fit = least_squares(&d, constant)?;
    let (points, rows, cols): (Vec<Vec<f64>>, usize, usize) = match args.get(2) {
        None | Some(Value::Empty) => {
            let (r, c) = shape(ctx, &args[0]);
            (d.x.clone(), r, c)
        }
        Some(v) => {
            let (xs, xr, xc) = matrix(ctx, v)?;
            if d.k == 1 {
                (xs.iter().map(|x| vec![*x]).collect(), xr, xc)
            } else if !d.by_row && xc == d.k {
                (
                    (0..xr).map(|i| xs[i * xc..(i + 1) * xc].to_vec()).collect(),
                    xr,
                    1,
                )
            } else if d.by_row && xr == d.k {
                (
                    (0..xc)
                        .map(|i| (0..xr).map(|j| xs[j * xc + i]).collect())
                        .collect(),
                    1,
                    xc,
                )
            } else {
                return Err(ErrorKind::Ref);
            }
        }
    };
    let out: Vec<Value> = points
        .iter()
        .map(|p| {
            let v = fit.b + p.iter().zip(&fit.m).map(|(x, m)| x * m).sum::<f64>();
            Value::number(if log { v.exp() } else { v })
        })
        .collect();
    Ok(Value::Array(Array::new(rows, cols, out).into()))
}

fn trend(ctx: &mut Ctx, args: &[Value]) -> R {
    predict(ctx, args, false)
}

fn growth(ctx: &mut Ctx, args: &[Value]) -> R {
    predict(ctx, args, true)
}

// ------------------------------------------------------------------ exponential smoothing

/// An evenly spaced series prepared from values and their timeline.
struct Series {
    y: Vec<f64>,
    start: f64,
    step: f64,
}

/// Values sorted by the timeline, duplicates aggregated, missing points completed (linear
/// interpolation, or zero when `completion` is 0).
fn series(
    ctx: &Ctx,
    values: &Value,
    timeline: &Value,
    completion: f64,
    aggregation: f64,
) -> Result<Series, ErrorKind> {
    let vs = flatten(ctx, values);
    let ts = flatten(ctx, timeline);
    if vs.len() != ts.len() {
        return Err(ErrorKind::NA);
    }
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for (v, t) in vs.iter().zip(&ts) {
        let Value::Number(t) = t else {
            return Err(ErrorKind::Value);
        };
        match v {
            Value::Number(v) => pts.push((*t, *v)),
            Value::Error(e) => return Err(*e),
            _ => {}
        }
    }
    if pts.len() < 3 {
        return Err(ErrorKind::Num);
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Aggregate equal time points (1 AVERAGE, 2 COUNT, 3 COUNTA, 4 MAX, 5 MEDIAN, 6 MIN, 7 SUM).
    let mut merged: Vec<(f64, Vec<f64>)> = Vec::new();
    for (t, v) in pts {
        match merged.last_mut() {
            Some((lt, g)) if *lt == t => g.push(v),
            _ => merged.push((t, vec![v])),
        }
    }
    let agg = |g: &Vec<f64>| -> Result<f64, ErrorKind> {
        let mut s = g.clone();
        s.sort_by(f64::total_cmp);
        Ok(match aggregation as i64 {
            1 => g.iter().sum::<f64>() / g.len() as f64,
            2 | 3 => g.len() as f64,
            4 => s[s.len() - 1],
            5 => {
                let n = s.len();
                if n % 2 == 1 {
                    s[n / 2]
                } else {
                    f64::midpoint(s[n / 2 - 1], s[n / 2])
                }
            }
            6 => s[0],
            7 => g.iter().sum(),
            _ => return Err(ErrorKind::Num),
        })
    };
    let pts: Vec<(f64, f64)> = merged
        .iter()
        .map(|(t, g)| agg(g).map(|v| (*t, v)))
        .collect::<Result<_, _>>()?;
    let step = pts
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .fold(f64::INFINITY, f64::min);
    if step.is_nan() || step <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let start = pts[0].0;
    let mut y: Vec<f64> = Vec::new();
    for w in pts.windows(2) {
        let gap = (w[1].0 - w[0].0) / step;
        if (gap - gap.round()).abs() > 1e-6 {
            return Err(ErrorKind::Num);
        }
        y.push(w[0].1);
        let g = gap.round() as usize;
        for j in 1..g {
            y.push(if completion == 0.0 {
                0.0
            } else {
                w[0].1 + (w[1].1 - w[0].1) * j as f64 / g as f64
            });
        }
    }
    y.push(pts[pts.len() - 1].1);
    Ok(Series { y, start, step })
}

/// Season length: the lag (2 ..= n/2) with the strongest autocorrelation of the detrended
/// series, the shortest among near-equal peaks; 1 when no lag correlates clearly.
fn detect_season(y: &[f64]) -> usize {
    let n = y.len();
    if n < 4 {
        return 1;
    }
    // Remove the linear trend.
    let nf = n as f64;
    let mt = (nf - 1.0) / 2.0;
    let my = y.iter().sum::<f64>() / nf;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, v) in y.iter().enumerate() {
        sxy += (i as f64 - mt) * (v - my);
        sxx += (i as f64 - mt) * (i as f64 - mt);
    }
    let slope = sxy / sxx;
    let r: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(i, v)| v - my - slope * (i as f64 - mt))
        .collect();
    let var: f64 = r.iter().map(|x| x * x).sum();
    if var <= 1e-12 * y.iter().map(|x| x * x).sum::<f64>().max(1e-300) {
        return 1;
    }
    let acf = |lag: usize| -> f64 {
        (0..n - lag).map(|i| r[i] * r[i + lag]).sum::<f64>() / var * nf / (nf - lag as f64)
    };
    let lags: Vec<(usize, f64)> = (2..=n / 2).map(|l| (l, acf(l))).collect();
    let best = lags.iter().map(|l| l.1).fold(f64::MIN, f64::max);
    if best < 0.5 {
        return 1;
    }
    lags.iter().find(|l| l.1 >= 0.9 * best).map_or(1, |l| l.0)
}

/// A fitted additive Holt–Winters model.
struct Model {
    level: f64,
    trend: f64,
    season: Vec<f64>,
    /// Index of the last observation.
    last: usize,
    alpha: f64,
    beta: f64,
    gamma: f64,
    /// One-step errors of the fitted observations.
    errors: Vec<f64>,
}

impl Model {
    /// Forecast `h` (> 0) steps after the last observation.
    fn forecast(&self, h: f64) -> f64 {
        let m = self.season.len();
        let s = if m > 1 {
            let k = h.ceil().max(1.0) as usize;
            self.season[(self.last + k) % m]
        } else {
            0.0
        };
        self.level + h * self.trend + s
    }
}

fn run_model(y: &[f64], m: usize, alpha: f64, beta: f64, gamma: f64) -> Model {
    let n = y.len();
    let (mut level, mut trend, mut season, first) = if m > 1 {
        let mean = |a: &[f64]| a.iter().sum::<f64>() / a.len() as f64;
        let m1 = mean(&y[..m]);
        let m2 = mean(&y[m..2 * m]);
        let b0 = (m2 - m1) / m as f64;
        let half = (m as f64 - 1.0) / 2.0;
        let season: Vec<f64> = (0..m)
            .map(|i| y[i] - (m1 + b0 * (i as f64 - half)))
            .collect();
        (m1 + b0 * half, b0, season, m)
    } else {
        (y[0], y[1] - y[0], vec![0.0], 1)
    };
    let mut errors = Vec::with_capacity(n);
    for (t, &obs) in y.iter().enumerate().skip(first) {
        let si = t % season.len();
        let s = if m > 1 { season[si] } else { 0.0 };
        errors.push(obs - (level + trend + s));
        let new_level = alpha * (obs - s) + (1.0 - alpha) * (level + trend);
        trend = beta * (new_level - level) + (1.0 - beta) * trend;
        if m > 1 {
            season[si] = gamma * (obs - new_level) + (1.0 - gamma) * s;
        }
        level = new_level;
    }
    Model {
        level,
        trend,
        season,
        last: n - 1,
        alpha,
        beta,
        gamma: if m > 1 { gamma } else { 0.0 },
        errors,
    }
}

/// The model whose smoothing constants (grid search, then refinement) minimise the one-step SSE.
fn fit_model(y: &[f64], m: usize) -> Model {
    let sse = |a: f64, b: f64, g: f64| -> f64 {
        run_model(y, m, a, b, g).errors.iter().map(|e| e * e).sum()
    };
    let grid: Vec<f64> = (0..=10).map(|i| f64::from(i) / 10.0).collect();
    let gs: &[f64] = if m > 1 { &grid } else { &[0.0] };
    let mut best = (f64::INFINITY, 0.5, 0.1, 0.1);
    for &a in &grid[1..] {
        for &b in &grid {
            for &g in gs {
                let e = sse(a, b, g);
                if e < best.0 - 1e-12 * best.0.abs() {
                    best = (e, a, b, g);
                }
            }
        }
    }
    // Coordinate refinement with shrinking steps inside [0, 1] (alpha > 0).
    let mut step = 0.05;
    while step > 1e-4 {
        let mut improved = false;
        for dim in 0..3 {
            if dim == 2 && m <= 1 {
                continue;
            }
            for dir in [-1.0, 1.0] {
                let mut p = [best.1, best.2, best.3];
                p[dim] = (p[dim] + dir * step).clamp(if dim == 0 { 1e-3 } else { 0.0 }, 1.0);
                let e = sse(p[0], p[1], p[2]);
                if e < best.0 - 1e-12 * best.0.abs() {
                    best = (e, p[0], p[1], p[2]);
                    improved = true;
                }
            }
        }
        if !improved {
            step /= 2.0;
        }
    }
    run_model(y, m, best.1, best.2, best.3)
}

/// The series, its season length and the fitted model for the ETS functions; `values` and
/// `timeline` are at `vi` and `vi + 1`, seasonality / completion / aggregation follow.
fn ets(ctx: &Ctx, args: &[Value], vi: usize) -> Result<(Series, usize, Model), ErrorKind> {
    let season_arg = opt_num(ctx, args, vi + 2, 1.0)?.trunc();
    let completion = opt_num(ctx, args, vi + 3, 1.0)?.trunc();
    let aggregation = opt_num(ctx, args, vi + 4, 1.0)?.trunc();
    if !(0.0..=8760.0).contains(&season_arg) || !(0.0..=1.0).contains(&completion) {
        return Err(ErrorKind::Num);
    }
    let s = series(ctx, &args[vi], &args[vi + 1], completion, aggregation)?;
    let mut m = match season_arg as usize {
        1 => detect_season(&s.y),
        0 => 1,
        k => k,
    };
    if 2 * m > s.y.len() {
        m = 1;
    }
    let model = fit_model(&s.y, m);
    Ok((s, m, model))
}

/// Steps from the last observation to `target`.
fn horizon(s: &Series, target: f64) -> Result<f64, ErrorKind> {
    if target < s.start {
        return Err(ErrorKind::Num);
    }
    Ok((target - s.start) / s.step - (s.y.len() - 1) as f64)
}

fn forecast_ets(ctx: &mut Ctx, args: &[Value]) -> R {
    let target = ctx.num(&args[0])?;
    let (s, _, model) = ets(ctx, args, 1)?;
    let h = horizon(&s, target)?;
    if h <= 0.0 {
        // Inside the history: the observed (completed) series, interpolated.
        let pos = (target - s.start) / s.step;
        let i = pos.floor() as usize;
        let fr = pos - pos.floor();
        let a = s.y[i];
        let b = s.y.get(i + 1).copied().unwrap_or(a);
        return num(a + fr * (b - a));
    }
    num(model.forecast(h))
}

fn forecast_ets_confint(ctx: &mut Ctx, args: &[Value]) -> R {
    let target = ctx.num(&args[0])?;
    let level = opt_num(ctx, args, 3, 0.95)?;
    if level <= 0.0 || level >= 1.0 {
        return Err(ErrorKind::Num);
    }
    // FORECAST.ETS.CONFINT(target, values, timeline, [confidence], [seasonality], …).
    let mut shifted: Vec<Value> = vec![args[0].clone(), args[1].clone(), args[2].clone()];
    shifted.extend(args.iter().skip(4).cloned());
    let (s, m, model) = ets(ctx, &shifted, 1)?;
    let h = horizon(&s, target)?.ceil().max(1.0) as usize;
    let n = model.errors.len() as f64;
    let mse = model.errors.iter().map(|e| e * e).sum::<f64>() / n.max(1.0);
    // Variance multiplier of the h-step forecast of the additive model.
    let mut c = 1.0;
    for j in 1..h {
        let season = if m > 1 && j % m == 0 {
            model.gamma
        } else {
            0.0
        };
        c += (model.alpha * (1.0 + j as f64 * model.beta) + season).powi(2);
    }
    num(norm_s_inv_f(0.5 + level / 2.0)? * (mse * c).sqrt())
}

fn forecast_ets_seasonality(ctx: &mut Ctx, args: &[Value]) -> R {
    // FORECAST.ETS.SEASONALITY(values, timeline, [completion], [aggregation]).
    let completion = opt_num(ctx, args, 2, 1.0)?.trunc();
    let aggregation = opt_num(ctx, args, 3, 1.0)?.trunc();
    let s = series(ctx, &args[0], &args[1], completion, aggregation)?;
    let m = detect_season(&s.y);
    Ok(Value::number(if 2 * m > s.y.len() {
        1.0
    } else {
        m as f64
    }))
}

fn forecast_ets_stat(ctx: &mut Ctx, args: &[Value]) -> R {
    // FORECAST.ETS.STAT(values, timeline, statistic, [seasonality], [completion], [aggregation]).
    let kind = ctx.num(&args[2])?.trunc();
    let mut shifted: Vec<Value> = vec![args[0].clone(), args[1].clone()];
    shifted.extend(args.iter().skip(3).cloned());
    let (s, m, model) = ets(ctx, &shifted, 0)?;
    let e = &model.errors;
    let n = e.len() as f64;
    let y = &s.y;
    let first = if m > 1 { m } else { 1 };
    let value = match kind as i64 {
        1 => model.alpha,
        2 => model.beta,
        3 => model.gamma,
        // MASE: MAE over the in-sample one-step naive forecast's MAE.
        4 => {
            let naive: f64 =
                y.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (y.len() - 1) as f64;
            let mae = e.iter().map(|x| x.abs()).sum::<f64>() / n;
            if naive == 0.0 {
                0.0
            } else {
                mae / naive
            }
        }
        // SMAPE
        5 => {
            e.iter()
                .zip(&y[first..])
                .map(|(err, obs)| {
                    let fc = obs - err;
                    let d = obs.abs() + fc.abs();
                    if d == 0.0 {
                        0.0
                    } else {
                        2.0 * err.abs() / d
                    }
                })
                .sum::<f64>()
                / n
        }
        6 => e.iter().map(|x| x.abs()).sum::<f64>() / n,
        7 => (e.iter().map(|x| x * x).sum::<f64>() / n).sqrt(),
        8 => s.step,
        _ => return Err(ErrorKind::Num),
    };
    num(value)
}

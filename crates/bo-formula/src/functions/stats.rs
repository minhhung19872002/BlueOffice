//! Statistical functions (ECMA-376 Part 1, §18.17.7), including `SUBTOTAL` and `AGGREGATE`.

use std::f64::consts::PI;

use super::math::{ifs_mask, paired_numbers, resized_range};
use super::{f, flatten, gather, num, numbers, opt_num, s, FnSpec, Gather, VAR};
use crate::eval::{Ctx, R};
use crate::value::{num_eq, text_to_number, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("AVERAGE", 1, VAR, average),
    f("AVERAGEA", 1, VAR, averagea),
    f("AVERAGEIF", 2, 3, averageif),
    f("AVERAGEIFS", 3, VAR, averageifs),
    f("COUNT", 1, VAR, count),
    f("COUNTA", 1, VAR, counta),
    f("COUNTBLANK", 1, 1, countblank),
    f("COUNTIF", 2, 2, countif),
    f("COUNTIFS", 2, VAR, countifs),
    f("MAX", 1, VAR, max),
    f("MAXA", 1, VAR, maxa),
    f("MIN", 1, VAR, min),
    f("MINA", 1, VAR, mina),
    f("MAXIFS", 3, VAR, maxifs),
    f("MINIFS", 3, VAR, minifs),
    f("MEDIAN", 1, VAR, median),
    f("MODE", 1, VAR, mode),
    f("MODE.SNGL", 1, VAR, mode),
    f("LARGE", 2, 2, large),
    f("SMALL", 2, 2, small),
    f("RANK", 2, 3, rank_eq),
    f("RANK.EQ", 2, 3, rank_eq),
    f("RANK.AVG", 2, 3, rank_avg),
    f("STDEV", 1, VAR, stdev_s),
    f("STDEV.S", 1, VAR, stdev_s),
    f("STDEVP", 1, VAR, stdev_p),
    f("STDEV.P", 1, VAR, stdev_p),
    f("STDEVA", 1, VAR, stdeva),
    f("VAR", 1, VAR, var_s),
    f("VAR.S", 1, VAR, var_s),
    f("VARP", 1, VAR, var_p),
    f("VAR.P", 1, VAR, var_p),
    f("VARA", 1, VAR, vara),
    f("PERCENTILE", 2, 2, percentile_inc),
    f("PERCENTILE.INC", 2, 2, percentile_inc),
    f("PERCENTILE.EXC", 2, 2, percentile_exc),
    f("QUARTILE", 2, 2, quartile_inc),
    f("QUARTILE.INC", 2, 2, quartile_inc),
    f("QUARTILE.EXC", 2, 2, quartile_exc),
    f("CORREL", 2, 2, correl),
    f("PEARSON", 2, 2, correl),
    f("RSQ", 2, 2, rsq),
    f("SLOPE", 2, 2, slope),
    f("INTERCEPT", 2, 2, intercept),
    f("FORECAST", 3, 3, forecast),
    f("FORECAST.LINEAR", 3, 3, forecast),
    f("COVAR", 2, 2, covariance_p),
    f("COVARIANCE.P", 2, 2, covariance_p),
    f("COVARIANCE.S", 2, 2, covariance_s),
    s("NORM.DIST", 4, 4, norm_dist),
    s("NORMDIST", 4, 4, norm_dist),
    s("NORM.INV", 3, 3, norm_inv),
    s("NORMINV", 3, 3, norm_inv),
    s("NORM.S.DIST", 2, 2, norm_s_dist),
    s("NORMSDIST", 1, 1, normsdist),
    s("NORM.S.INV", 1, 1, norm_s_inv),
    s("NORMSINV", 1, 1, norm_s_inv),
    s("STANDARDIZE", 3, 3, standardize),
    f("GEOMEAN", 1, VAR, geomean),
    f("HARMEAN", 1, VAR, harmean),
    f("AVEDEV", 1, VAR, avedev),
    f("DEVSQ", 1, VAR, devsq),
    f("SUBTOTAL", 2, VAR, subtotal),
    f("AGGREGATE", 3, VAR, aggregate),
];

const A_MODE: Gather = Gather {
    a_mode: true,
    skip_errors: false,
    skip_hidden: false,
    skip_nested: false,
};

// ------------------------------------------------------------ primitives

fn mean(v: &[f64]) -> Result<f64, ErrorKind> {
    if v.is_empty() {
        return Err(ErrorKind::Div0);
    }
    Ok(v.iter().sum::<f64>() / v.len() as f64)
}

fn sum_sq_dev(v: &[f64]) -> Result<f64, ErrorKind> {
    let m = mean(v)?;
    Ok(v.iter().map(|x| (x - m) * (x - m)).sum())
}

fn variance(v: &[f64], sample: bool) -> Result<f64, ErrorKind> {
    let n = v.len() as f64;
    let d = if sample { n - 1.0 } else { n };
    if d < 1.0 {
        return Err(ErrorKind::Div0);
    }
    Ok(sum_sq_dev(v)? / d)
}

fn sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v
}

fn median_of(v: Vec<f64>) -> Result<f64, ErrorKind> {
    if v.is_empty() {
        return Err(ErrorKind::Num);
    }
    let v = sorted(v);
    let n = v.len();
    Ok(if n % 2 == 1 {
        v[n / 2]
    } else {
        f64::midpoint(v[n / 2 - 1], v[n / 2])
    })
}

fn mode_of(v: &[f64]) -> Result<f64, ErrorKind> {
    let mut best: Option<(f64, usize)> = None;
    for (i, x) in v.iter().enumerate() {
        // Count occurrences; ties resolved by first occurrence.
        if v[..i].iter().any(|y| y == x) {
            continue;
        }
        let c = v.iter().filter(|y| *y == x).count();
        if c > 1 && best.is_none_or(|(_, bc)| c > bc) {
            best = Some((*x, c));
        }
    }
    best.map(|(x, _)| x).ok_or(ErrorKind::NA)
}

fn kth(v: Vec<f64>, k: f64, largest: bool) -> Result<f64, ErrorKind> {
    let k = k.ceil();
    if k < 1.0 || k > v.len() as f64 {
        return Err(ErrorKind::Num);
    }
    let v = sorted(v);
    let i = k as usize - 1;
    Ok(if largest { v[v.len() - 1 - i] } else { v[i] })
}

fn percentile_inc_of(v: Vec<f64>, k: f64) -> Result<f64, ErrorKind> {
    if v.is_empty() || !(0.0..=1.0).contains(&k) {
        return Err(ErrorKind::Num);
    }
    let v = sorted(v);
    let rank = k * (v.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let frac = rank - rank.floor();
    Ok(if lo + 1 < v.len() {
        v[lo] + frac * (v[lo + 1] - v[lo])
    } else {
        v[lo]
    })
}

fn percentile_exc_of(v: Vec<f64>, k: f64) -> Result<f64, ErrorKind> {
    let n = v.len() as f64;
    if v.is_empty() || k <= 0.0 || k >= 1.0 {
        return Err(ErrorKind::Num);
    }
    let rank = k * (n + 1.0);
    if rank < 1.0 || rank > n {
        return Err(ErrorKind::Num);
    }
    let v = sorted(v);
    let lo = rank.floor() as usize - 1;
    let frac = rank - rank.floor();
    Ok(if lo + 1 < v.len() {
        v[lo] + frac * (v[lo + 1] - v[lo])
    } else {
        v[lo]
    })
}

// ------------------------------------------------------------ averages & counts

fn average(ctx: &mut Ctx, args: &[Value]) -> R {
    num(mean(&numbers(ctx, args)?)?)
}

fn averagea(ctx: &mut Ctx, args: &[Value]) -> R {
    num(mean(&gather(ctx, args, A_MODE)?)?)
}

fn masked_numbers(values: &[Value], mask: &[bool]) -> Vec<f64> {
    values
        .iter()
        .zip(mask)
        .filter_map(|(v, m)| match (v, m) {
            (Value::Number(n), true) => Some(*n),
            _ => None,
        })
        .collect()
}

fn averageif(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mask, _) = ifs_mask(ctx, &args[..2])?;
    let target = match args.get(2) {
        Some(v) if !matches!(v, Value::Empty) => resized_range(ctx, &args[0], v)?,
        _ => flatten(ctx, &args[0]),
    };
    num(mean(&masked_numbers(&target, &mask))?)
}

fn averageifs(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mask, shp) = ifs_mask(ctx, &args[1..])?;
    if super::shape(ctx, &args[0]) != shp {
        return Err(ErrorKind::Value);
    }
    num(mean(&masked_numbers(&flatten(ctx, &args[0]), &mask))?)
}

fn count(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut n = 0usize;
    for a in args {
        ctx.for_each_value(a, &mut |v, in_range| match v {
            Value::Number(_) => n += 1,
            Value::Bool(_) if !in_range => n += 1,
            Value::Text(s) if !in_range && text_to_number(s, ctx.date_system).is_some() => n += 1,
            _ => {}
        });
    }
    Ok(Value::Number(n as f64))
}

fn counta(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut n = 0usize;
    for a in args {
        ctx.for_each_value(a, &mut |v, in_range| {
            if !(in_range && matches!(v, Value::Empty)) {
                n += 1;
            }
        });
    }
    Ok(Value::Number(n as f64))
}

fn countblank(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut n = 0usize;
    ctx.for_each_value(&args[0], &mut |v, _| {
        if matches!(v, Value::Empty) || matches!(v, Value::Text(s) if s.is_empty()) {
            n += 1;
        }
    });
    Ok(Value::Number(n as f64))
}

fn countif(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mask, _) = ifs_mask(ctx, args)?;
    Ok(Value::Number(mask.iter().filter(|m| **m).count() as f64))
}

fn countifs(ctx: &mut Ctx, args: &[Value]) -> R {
    countif(ctx, args)
}

fn extreme(v: &[f64], largest: bool) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter()
        .copied()
        .fold(if largest { f64::MIN } else { f64::MAX }, |a, b| {
            if largest {
                a.max(b)
            } else {
                a.min(b)
            }
        })
}

fn max(ctx: &mut Ctx, args: &[Value]) -> R {
    num(extreme(&numbers(ctx, args)?, true))
}

fn maxa(ctx: &mut Ctx, args: &[Value]) -> R {
    num(extreme(&gather(ctx, args, A_MODE)?, true))
}

fn min(ctx: &mut Ctx, args: &[Value]) -> R {
    num(extreme(&numbers(ctx, args)?, false))
}

fn mina(ctx: &mut Ctx, args: &[Value]) -> R {
    num(extreme(&gather(ctx, args, A_MODE)?, false))
}

fn extreme_ifs(ctx: &Ctx, args: &[Value], largest: bool) -> R {
    let (mask, shp) = ifs_mask(ctx, &args[1..])?;
    if super::shape(ctx, &args[0]) != shp {
        return Err(ErrorKind::Value);
    }
    num(extreme(
        &masked_numbers(&flatten(ctx, &args[0]), &mask),
        largest,
    ))
}

fn maxifs(ctx: &mut Ctx, args: &[Value]) -> R {
    extreme_ifs(ctx, args, true)
}

fn minifs(ctx: &mut Ctx, args: &[Value]) -> R {
    extreme_ifs(ctx, args, false)
}

fn median(ctx: &mut Ctx, args: &[Value]) -> R {
    num(median_of(numbers(ctx, args)?)?)
}

fn mode(ctx: &mut Ctx, args: &[Value]) -> R {
    num(mode_of(&numbers(ctx, args)?)?)
}

/// Numbers of a single array argument (ranges and arrays: numbers only).
fn array_numbers(ctx: &Ctx, v: &Value) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => out.push(n),
            Value::Error(e) => return Err(e),
            Value::Text(s) if !matches!(v, Value::Ref(_) | Value::Refs(_) | Value::Array(_)) => {
                out.push(text_to_number(&s, ctx.date_system).ok_or(ErrorKind::Value)?);
            }
            _ => {}
        }
    }
    Ok(out)
}

fn large(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    num(kth(v, ctx.num(&args[1])?, true)?)
}

fn small(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    num(kth(v, ctx.num(&args[1])?, false)?)
}

fn rank_common(ctx: &Ctx, args: &[Value], avg: bool) -> R {
    let x = ctx.num(&args[0])?;
    let v = array_numbers(ctx, &args[1])?;
    let ascending = opt_num(ctx, args, 2, 0.0)? != 0.0;
    if !v.iter().any(|y| num_eq(*y, x)) {
        return Err(ErrorKind::NA);
    }
    let better = v
        .iter()
        .filter(|y| !num_eq(**y, x) && if ascending { **y < x } else { **y > x })
        .count() as f64;
    let ties = v.iter().filter(|y| num_eq(**y, x)).count() as f64;
    Ok(Value::Number(if avg {
        better + f64::midpoint(ties, 1.0)
    } else {
        better + 1.0
    }))
}

fn rank_eq(ctx: &mut Ctx, args: &[Value]) -> R {
    rank_common(ctx, args, false)
}

fn rank_avg(ctx: &mut Ctx, args: &[Value]) -> R {
    rank_common(ctx, args, true)
}

fn stdev_s(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&numbers(ctx, args)?, true)?.sqrt())
}

fn stdev_p(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&numbers(ctx, args)?, false)?.sqrt())
}

fn stdeva(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&gather(ctx, args, A_MODE)?, true)?.sqrt())
}

fn var_s(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&numbers(ctx, args)?, true)?)
}

fn var_p(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&numbers(ctx, args)?, false)?)
}

fn vara(ctx: &mut Ctx, args: &[Value]) -> R {
    num(variance(&gather(ctx, args, A_MODE)?, true)?)
}

fn percentile_inc(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    num(percentile_inc_of(v, ctx.num(&args[1])?)?)
}

fn percentile_exc(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    num(percentile_exc_of(v, ctx.num(&args[1])?)?)
}

fn quartile_inc(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    let q = ctx.num(&args[1])?.trunc();
    if !(0.0..=4.0).contains(&q) {
        return Err(ErrorKind::Num);
    }
    num(percentile_inc_of(v, q / 4.0)?)
}

fn quartile_exc(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = array_numbers(ctx, &args[0])?;
    let q = ctx.num(&args[1])?.trunc();
    if !(1.0..=3.0).contains(&q) {
        return Err(ErrorKind::Num);
    }
    num(percentile_exc_of(v, q / 4.0)?)
}

// ------------------------------------------------------------ paired statistics

struct Moments {
    n: f64,
    mx: f64,
    my: f64,
    sxx: f64,
    syy: f64,
    sxy: f64,
}

/// Moments of (x, y) pairs; `args[0]` holds y values and `args[1]` x values
/// when `y_first` (SLOPE/INTERCEPT/FORECAST convention).
fn moments(ctx: &Ctx, a: &Value, b: &Value) -> Result<Moments, ErrorKind> {
    let pairs = paired_numbers(ctx, &[a.clone(), b.clone()])?;
    let n = pairs.len() as f64;
    if pairs.is_empty() {
        return Err(ErrorKind::Div0);
    }
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let mut m = Moments {
        n,
        mx,
        my,
        sxx: 0.0,
        syy: 0.0,
        sxy: 0.0,
    };
    for (x, y) in pairs {
        m.sxx += (x - mx) * (x - mx);
        m.syy += (y - my) * (y - my);
        m.sxy += (x - mx) * (y - my);
    }
    Ok(m)
}

fn correl_of(ctx: &Ctx, args: &[Value]) -> Result<f64, ErrorKind> {
    let m = moments(ctx, &args[0], &args[1])?;
    if m.sxx == 0.0 || m.syy == 0.0 {
        return Err(ErrorKind::Div0);
    }
    Ok(m.sxy / (m.sxx * m.syy).sqrt())
}

fn correl(ctx: &mut Ctx, args: &[Value]) -> R {
    num(correl_of(ctx, args)?)
}

fn rsq(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = correl_of(ctx, args)?;
    num(r * r)
}

/// (slope, intercept) for known_y = args[0], known_x = args[1].
fn regression(ctx: &Ctx, ys: &Value, xs: &Value) -> Result<(f64, f64), ErrorKind> {
    let m = moments(ctx, xs, ys)?;
    if m.sxx == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let b = m.sxy / m.sxx;
    Ok((b, m.my - b * m.mx))
}

fn slope(ctx: &mut Ctx, args: &[Value]) -> R {
    num(regression(ctx, &args[0], &args[1])?.0)
}

fn intercept(ctx: &mut Ctx, args: &[Value]) -> R {
    num(regression(ctx, &args[0], &args[1])?.1)
}

fn forecast(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let (b, a) = regression(ctx, &args[1], &args[2])?;
    num(a + b * x)
}

fn covariance_p(ctx: &mut Ctx, args: &[Value]) -> R {
    let m = moments(ctx, &args[0], &args[1])?;
    num(m.sxy / m.n)
}

fn covariance_s(ctx: &mut Ctx, args: &[Value]) -> R {
    let m = moments(ctx, &args[0], &args[1])?;
    if m.n < 2.0 {
        return Err(ErrorKind::Div0);
    }
    num(m.sxy / (m.n - 1.0))
}

// ------------------------------------------------------------ normal distribution

/// Standard normal density.
pub(crate) fn phi(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Standard normal CDF: Taylor series for |x| < 3 (Marsaglia), continued
/// fraction for the tails.
pub(crate) fn norm_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x.abs() < 3.0 {
        let mut term = x;
        let mut sum = x;
        let x2 = x * x;
        let mut k = 1.0;
        while term.abs() > 1e-17 * sum.abs().max(1e-300) {
            k += 2.0;
            term *= x2 / k;
            sum += term;
            if k > 500.0 {
                break;
            }
        }
        return 0.5 + phi(x) * sum;
    }
    if x > 0.0 {
        return 1.0 - norm_cdf(-x);
    }
    if x < -40.0 {
        return 0.0;
    }
    // Lower tail Q = phi(z) / (z + 1/(z + 2/(z + 3/(z + ...)))) with z = -x.
    let z = -x;
    let mut cf = z;
    for n in (1..=300).rev() {
        cf = z + f64::from(n) / cf;
    }
    phi(z) / cf
}

/// Inverse standard normal CDF by Halley iteration from a rational starting
/// estimate (Abramowitz & Stegun 26.2.23).
pub(crate) fn norm_s_inv_f(p: f64) -> Result<f64, ErrorKind> {
    if p <= 0.0 || p >= 1.0 {
        return Err(ErrorKind::Num);
    }
    let q = if p < 0.5 { p } else { 1.0 - p };
    let t = (-2.0 * q.ln()).sqrt();
    let mut x = t
        - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t)
            / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t);
    if p < 0.5 {
        x = -x;
    }
    for _ in 0..50 {
        let e = norm_cdf(x) - p;
        let d = phi(x);
        if d == 0.0 {
            break;
        }
        let u = e / d;
        let step = u / (1.0 + x * u / 2.0);
        x -= step;
        if step.abs() < 1e-15 * x.abs().max(1.0) {
            break;
        }
    }
    Ok(x)
}

fn norm_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let mu = ctx.num(&args[1])?;
    let sd = ctx.num(&args[2])?;
    let cumulative = ctx.boolean(&args[3])?;
    if sd <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let z = (x - mu) / sd;
    num(if cumulative { norm_cdf(z) } else { phi(z) / sd })
}

fn norm_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = ctx.num(&args[0])?;
    let mu = ctx.num(&args[1])?;
    let sd = ctx.num(&args[2])?;
    if sd <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(mu + sd * norm_s_inv_f(p)?)
}

fn norm_s_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let z = ctx.num(&args[0])?;
    let cumulative = ctx.boolean(&args[1])?;
    num(if cumulative { norm_cdf(z) } else { phi(z) })
}

fn normsdist(ctx: &mut Ctx, args: &[Value]) -> R {
    num(norm_cdf(ctx.num(&args[0])?))
}

fn norm_s_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    num(norm_s_inv_f(ctx.num(&args[0])?)?)
}

fn standardize(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let mu = ctx.num(&args[1])?;
    let sd = ctx.num(&args[2])?;
    if sd <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num((x - mu) / sd)
}

fn geomean(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    if v.is_empty() || v.iter().any(|x| *x <= 0.0) {
        return Err(ErrorKind::Num);
    }
    num((v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp())
}

fn harmean(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    if v.is_empty() || v.iter().any(|x| *x <= 0.0) {
        return Err(ErrorKind::Num);
    }
    num(v.len() as f64 / v.iter().map(|x| 1.0 / x).sum::<f64>())
}

fn avedev(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    if v.is_empty() {
        return Err(ErrorKind::Num);
    }
    let m = mean(&v)?;
    num(v.iter().map(|x| (x - m).abs()).sum::<f64>() / v.len() as f64)
}

fn devsq(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    if v.is_empty() {
        return Err(ErrorKind::Num);
    }
    num(sum_sq_dev(&v)?)
}

// ------------------------------------------------------------ SUBTOTAL / AGGREGATE

fn count_nonblank(ctx: &Ctx, args: &[Value], opt: Gather) -> f64 {
    let mut n = 0usize;
    for a in args {
        if let Value::Ref(area) = a {
            ctx.for_each_area(area, &mut |r, c, v| {
                if opt.skip_hidden && ctx.provider.is_row_hidden(area.sheet, r) {
                    return;
                }
                if opt.skip_nested
                    && ctx
                        .provider
                        .cell_formula(area.sheet, r, c)
                        .is_some_and(|f| {
                            let u = f.to_uppercase();
                            u.contains("SUBTOTAL(") || u.contains("AGGREGATE(")
                        })
                {
                    return;
                }
                if opt.skip_errors && v.is_error() {
                    return;
                }
                if !matches!(v, Value::Empty) {
                    n += 1;
                }
            });
        } else {
            ctx.for_each_value(a, &mut |v, in_range| {
                if !(in_range && matches!(v, Value::Empty) || opt.skip_errors && v.is_error()) {
                    n += 1;
                }
            });
        }
    }
    n as f64
}

/// Apply statistic `code` (SUBTOTAL 1-11 numbering) to gathered numbers.
fn basic_stat(code: i64, v: &[f64]) -> Result<f64, ErrorKind> {
    match code {
        1 => mean(v),
        2 => Ok(v.len() as f64),
        4 => Ok(extreme(v, true)),
        5 => Ok(extreme(v, false)),
        6 => Ok(if v.is_empty() {
            0.0
        } else {
            v.iter().product()
        }),
        7 => variance(v, true).map(f64::sqrt),
        8 => variance(v, false).map(f64::sqrt),
        9 => Ok(v.iter().sum()),
        10 => variance(v, true),
        11 => variance(v, false),
        _ => Err(ErrorKind::Value),
    }
}

fn subtotal(ctx: &mut Ctx, args: &[Value]) -> R {
    let code = ctx.num(&args[0])?.trunc() as i64;
    let (base, hidden) = match code {
        1..=11 => (code, false),
        101..=111 => (code - 100, true),
        _ => return Err(ErrorKind::Value),
    };
    let opt = Gather {
        a_mode: false,
        skip_errors: false,
        skip_hidden: hidden,
        skip_nested: true,
    };
    let refs = &args[1..];
    if base == 3 {
        return Ok(Value::Number(count_nonblank(ctx, refs, opt)));
    }
    let v = gather(ctx, refs, opt)?;
    num(basic_stat(base, &v)?)
}

fn aggregate(ctx: &mut Ctx, args: &[Value]) -> R {
    let code = ctx.num(&args[0])?.trunc() as i64;
    let option = match &args[1] {
        Value::Empty => 0,
        v => ctx.num(v)?.trunc() as i64,
    };
    if !(0..=7).contains(&option) {
        return Err(ErrorKind::Value);
    }
    let opt = Gather {
        a_mode: false,
        skip_errors: matches!(option, 2 | 3 | 6 | 7),
        skip_hidden: matches!(option, 1 | 3 | 5 | 7),
        skip_nested: option <= 3,
    };
    match code {
        1..=13 => {
            let refs = &args[2..];
            if code == 3 {
                return Ok(Value::Number(count_nonblank(ctx, refs, opt)));
            }
            let v = gather(ctx, refs, opt)?;
            let r = match code {
                1..=11 => basic_stat(code, &v)?,
                12 => median_of(v)?,
                _ => mode_of(&v)?,
            };
            num(r)
        }
        14..=19 => {
            let k = ctx.num(args.get(3).ok_or(ErrorKind::Value)?)?;
            let v = gather(ctx, &args[2..3], opt)?;
            let r = match code {
                14 => kth(v, k, true)?,
                15 => kth(v, k, false)?,
                16 => percentile_inc_of(v, k)?,
                17 => {
                    let q = k.trunc();
                    if !(0.0..=4.0).contains(&q) {
                        return Err(ErrorKind::Num);
                    }
                    percentile_inc_of(v, q / 4.0)?
                }
                18 => percentile_exc_of(v, k)?,
                _ => {
                    let q = k.trunc();
                    if !(1.0..=3.0).contains(&q) {
                        return Err(ErrorKind::Num);
                    }
                    percentile_exc_of(v, q / 4.0)?
                }
            };
            num(r)
        }
        _ => Err(ErrorKind::Value),
    }
}

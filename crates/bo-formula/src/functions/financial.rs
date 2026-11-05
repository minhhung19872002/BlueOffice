//! Financial functions (ECMA-376 Part 1, §18.17.7). The time-value-of-money
//! functions solve `pv·(1+r)^n + pmt·(1+r·type)·((1+r)^n − 1)/r + fv = 0`
//! (or `pv + pmt·n + fv = 0` when `r = 0`), as in the PV definition.

use super::{f, flatten, num, opt_num, s, FnSpec, VAR};
use crate::eval::{Ctx, R};
use crate::value::{ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("PMT", 3, 5, pmt),
    s("IPMT", 4, 6, ipmt),
    s("PPMT", 4, 6, ppmt),
    s("PV", 3, 5, pv),
    s("FV", 3, 5, fv),
    f("NPV", 2, VAR, npv),
    f("XNPV", 3, 3, xnpv),
    f("IRR", 1, 2, irr),
    f("XIRR", 2, 3, xirr),
    s("RATE", 3, 6, rate),
    s("NPER", 3, 5, nper),
    s("SLN", 3, 3, sln),
    s("SYD", 4, 4, syd),
    s("DB", 4, 5, db),
    s("DDB", 4, 5, ddb),
    s("EFFECT", 2, 2, effect),
    s("NOMINAL", 2, 2, nominal),
    s("CUMIPMT", 6, 6, cumipmt),
    s("CUMPRINC", 6, 6, cumprinc),
];

fn pay_type(ctx: &Ctx, args: &[Value], i: usize) -> Result<f64, ErrorKind> {
    Ok(if opt_num(ctx, args, i, 0.0)? == 0.0 { 0.0 } else { 1.0 })
}

pub(crate) fn pmt_f(r: f64, n: f64, pv: f64, fv: f64, t: f64) -> Result<f64, ErrorKind> {
    if n == 0.0 {
        return Err(ErrorKind::Num);
    }
    if r == 0.0 {
        return Ok(-(pv + fv) / n);
    }
    let g = (1.0 + r).powf(n);
    Ok(-(fv + pv * g) * r / ((1.0 + r * t) * (g - 1.0)))
}

pub(crate) fn fv_f(r: f64, n: f64, pmt: f64, pv: f64, t: f64) -> f64 {
    if r == 0.0 {
        return -(pv + pmt * n);
    }
    let g = (1.0 + r).powf(n);
    -(pv * g + pmt * (1.0 + r * t) * (g - 1.0) / r)
}

fn pv_f(r: f64, n: f64, pmt: f64, fv: f64, t: f64) -> f64 {
    if r == 0.0 {
        return -(fv + pmt * n);
    }
    let g = (1.0 + r).powf(n);
    -(fv + pmt * (1.0 + r * t) * (g - 1.0) / r) / g
}

fn ipmt_f(r: f64, per: f64, n: f64, pv: f64, fv: f64, t: f64) -> Result<f64, ErrorKind> {
    if per < 1.0 || per > n {
        return Err(ErrorKind::Num);
    }
    let p = pmt_f(r, n, pv, fv, t)?;
    if t == 1.0 && per == 1.0 {
        return Ok(0.0);
    }
    // Interest on the balance outstanding after per-1 periods.
    let balance = fv_f(r, per - 1.0, p, pv, t);
    let mut i = balance * r;
    if t == 1.0 {
        i /= 1.0 + r;
    }
    Ok(i)
}

fn pmt(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?;
    let pv = ctx.num(&args[2])?;
    let fv = opt_num(ctx, args, 3, 0.0)?;
    let t = pay_type(ctx, args, 4)?;
    num(pmt_f(r, n, pv, fv, t)?)
}

fn ipmt_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64, f64, f64, f64), ErrorKind> {
    Ok((
        ctx.num(&args[0])?,
        ctx.num(&args[1])?,
        ctx.num(&args[2])?,
        ctx.num(&args[3])?,
        opt_num(ctx, args, 4, 0.0)?,
        pay_type(ctx, args, 5)?,
    ))
}

fn ipmt(ctx: &mut Ctx, args: &[Value]) -> R {
    let (r, per, n, pv, fv, t) = ipmt_args(ctx, args)?;
    num(ipmt_f(r, per, n, pv, fv, t)?)
}

fn ppmt(ctx: &mut Ctx, args: &[Value]) -> R {
    let (r, per, n, pv, fv, t) = ipmt_args(ctx, args)?;
    num(pmt_f(r, n, pv, fv, t)? - ipmt_f(r, per, n, pv, fv, t)?)
}

fn pv(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?;
    let p = ctx.num(&args[2])?;
    let fv = opt_num(ctx, args, 3, 0.0)?;
    let t = pay_type(ctx, args, 4)?;
    num(pv_f(r, n, p, fv, t))
}

fn fv(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?;
    let p = ctx.num(&args[2])?;
    let pv = opt_num(ctx, args, 3, 0.0)?;
    let t = pay_type(ctx, args, 4)?;
    num(fv_f(r, n, p, pv, t))
}

/// Numbers from cash-flow arguments (references/arrays: numbers only).
fn cash_flows(ctx: &Ctx, args: &[Value]) -> Result<Vec<f64>, ErrorKind> {
    super::numbers(ctx, args)
}

fn npv(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    if r == -1.0 {
        return Err(ErrorKind::Div0);
    }
    let flows = cash_flows(ctx, &args[1..])?;
    num(flows
        .iter()
        .enumerate()
        .map(|(i, v)| v / (1.0 + r).powi(i as i32 + 1))
        .sum())
}

fn dated_flows(ctx: &Ctx, values: &Value, dates: &Value) -> Result<(Vec<f64>, Vec<f64>), ErrorKind> {
    let to_nums = |v: &Value| -> Result<Vec<f64>, ErrorKind> {
        flatten(ctx, v)
            .iter()
            .map(|x| match x {
                Value::Number(n) => Ok(*n),
                Value::Error(e) => Err(*e),
                _ => Err(ErrorKind::Value),
            })
            .collect()
    };
    let v = to_nums(values)?;
    let d: Vec<f64> = to_nums(dates)?.into_iter().map(f64::trunc).collect();
    if v.len() != d.len() || v.is_empty() {
        return Err(ErrorKind::Num);
    }
    if d.iter().any(|x| *x < d[0] || *x < 0.0) {
        return Err(ErrorKind::Num);
    }
    Ok((v, d))
}

fn xnpv_f(r: f64, v: &[f64], d: &[f64]) -> f64 {
    v.iter()
        .zip(d)
        .map(|(x, di)| x / (1.0 + r).powf((di - d[0]) / 365.0))
        .sum()
}

fn xnpv(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    if r <= -1.0 {
        return Err(ErrorKind::Num);
    }
    let (v, d) = dated_flows(ctx, &args[1], &args[2])?;
    num(xnpv_f(r, &v, &d))
}

/// Find a root of `f` near `guess` by Newton's method with numeric derivative,
/// falling back to bisection on a bracketing interval.
fn solve(f: &dyn Fn(f64) -> f64, guess: f64, lo: f64, hi: f64) -> Result<f64, ErrorKind> {
    let mut x = guess;
    for _ in 0..100 {
        let y = f(x);
        if !y.is_finite() {
            break;
        }
        if y.abs() < 1e-10 {
            return Ok(x);
        }
        let h = 1e-7 * x.abs().max(1e-3);
        let d = (f(x + h) - f(x - h)) / (2.0 * h);
        if d == 0.0 || !d.is_finite() {
            break;
        }
        let nx = x - y / d;
        if !nx.is_finite() || nx <= lo {
            break;
        }
        if (nx - x).abs() < 1e-12 * nx.abs().max(1.0) {
            return Ok(nx);
        }
        x = nx;
    }
    // Bisection over a scan for a sign change.
    let mut a = lo + 1e-9;
    let fa0 = f(a);
    let mut prev = (a, fa0);
    let mut b = None;
    let steps = 2000;
    for i in 1..=steps {
        let xi = lo + 1e-9 + (hi - lo) * f64::from(i) / f64::from(steps);
        let yi = f(xi);
        if yi.is_finite() && prev.1.is_finite() && yi.signum() != prev.1.signum() {
            a = prev.0;
            b = Some(xi);
            break;
        }
        prev = (xi, yi);
    }
    let mut b = b.ok_or(ErrorKind::Num)?;
    let mut fa = f(a);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        let fm = f(m);
        if fm.abs() < 1e-12 || (b - a).abs() < 1e-15 {
            return Ok(m);
        }
        if fm.signum() == fa.signum() {
            a = m;
            fa = fm;
        } else {
            b = m;
        }
    }
    Ok(0.5 * (a + b))
}

fn irr(ctx: &mut Ctx, args: &[Value]) -> R {
    let flows = cash_flows(ctx, &args[..1])?;
    let guess = opt_num(ctx, args, 1, 0.1)?;
    if !flows.iter().any(|v| *v > 0.0) || !flows.iter().any(|v| *v < 0.0) {
        return Err(ErrorKind::Num);
    }
    let f = |r: f64| -> f64 {
        flows
            .iter()
            .enumerate()
            .map(|(i, v)| v / (1.0 + r).powi(i as i32))
            .sum()
    };
    num(solve(&f, guess, -1.0, 10.0)?)
}

fn xirr(ctx: &mut Ctx, args: &[Value]) -> R {
    let (v, d) = dated_flows(ctx, &args[0], &args[1])?;
    let guess = opt_num(ctx, args, 2, 0.1)?;
    if !v.iter().any(|x| *x > 0.0) || !v.iter().any(|x| *x < 0.0) {
        return Err(ErrorKind::Num);
    }
    let f = |r: f64| xnpv_f(r, &v, &d);
    num(solve(&f, guess, -1.0, 10.0)?)
}

fn rate(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?;
    let p = ctx.num(&args[1])?;
    let pv = ctx.num(&args[2])?;
    let fv = opt_num(ctx, args, 3, 0.0)?;
    let t = pay_type(ctx, args, 4)?;
    let guess = opt_num(ctx, args, 5, 0.1)?;
    if n <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let f = |r: f64| -> f64 {
        if r.abs() < 1e-12 {
            pv + p * n + fv
        } else {
            let g = (1.0 + r).powf(n);
            pv * g + p * (1.0 + r * t) * (g - 1.0) / r + fv
        }
    };
    num(solve(&f, guess, -1.0, 10.0)?)
}

fn nper(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let p = ctx.num(&args[1])?;
    let pv = ctx.num(&args[2])?;
    let fv = opt_num(ctx, args, 3, 0.0)?;
    let t = pay_type(ctx, args, 4)?;
    if r == 0.0 {
        if p == 0.0 {
            return Err(ErrorKind::Num);
        }
        return num(-(pv + fv) / p);
    }
    let a = p * (1.0 + r * t);
    let num_ = a - fv * r;
    let den = a + pv * r;
    if den == 0.0 || num_ / den <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num((num_ / den).ln() / (1.0 + r).ln())
}

fn sln(ctx: &mut Ctx, args: &[Value]) -> R {
    let cost = ctx.num(&args[0])?;
    let salvage = ctx.num(&args[1])?;
    let life = ctx.num(&args[2])?;
    if life == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num((cost - salvage) / life)
}

fn syd(ctx: &mut Ctx, args: &[Value]) -> R {
    let cost = ctx.num(&args[0])?;
    let salvage = ctx.num(&args[1])?;
    let life = ctx.num(&args[2])?;
    let per = ctx.num(&args[3])?;
    if life <= 0.0 || per <= 0.0 || per > life || salvage < 0.0 {
        return Err(ErrorKind::Num);
    }
    num((cost - salvage) * (life - per + 1.0) * 2.0 / (life * (life + 1.0)))
}

fn db(ctx: &mut Ctx, args: &[Value]) -> R {
    let cost = ctx.num(&args[0])?;
    let salvage = ctx.num(&args[1])?;
    let life = ctx.num(&args[2])?.trunc();
    let period = ctx.num(&args[3])?.trunc();
    let month = opt_num(ctx, args, 4, 12.0)?.trunc();
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period <= 0.0 || !(1.0..=12.0).contains(&month) {
        return Err(ErrorKind::Num);
    }
    let max_period = if month < 12.0 { life + 1.0 } else { life };
    if period > max_period {
        return Err(ErrorKind::Num);
    }
    if cost == 0.0 {
        return Ok(Value::Number(0.0));
    }
    // Rate rounded to three decimal places.
    let rate = ((1.0 - (salvage / cost).powf(1.0 / life)) * 1000.0).round() / 1000.0;
    let mut total = 0.0;
    let mut dep = 0.0;
    let mut p = 1.0;
    while p <= period {
        dep = if p == 1.0 {
            cost * rate * month / 12.0
        } else if p == life + 1.0 {
            (cost - total) * rate * (12.0 - month) / 12.0
        } else {
            (cost - total) * rate
        };
        total += dep;
        p += 1.0;
    }
    num(dep)
}

fn ddb(ctx: &mut Ctx, args: &[Value]) -> R {
    let cost = ctx.num(&args[0])?;
    let salvage = ctx.num(&args[1])?;
    let life = ctx.num(&args[2])?;
    let period = ctx.num(&args[3])?;
    let factor = opt_num(ctx, args, 4, 2.0)?;
    if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period <= 0.0 || period > life || factor <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let rate = (factor / life).min(1.0);
    let mut total = 0.0;
    let mut dep = 0.0;
    let mut p = 1.0;
    while p <= period.ceil() {
        dep = ((cost - total) * rate).min((cost - salvage - total).max(0.0));
        total += dep;
        p += 1.0;
    }
    num(dep)
}

fn effect(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?.trunc();
    if r <= 0.0 || n < 1.0 {
        return Err(ErrorKind::Num);
    }
    num((1.0 + r / n).powf(n) - 1.0)
}

fn nominal(ctx: &mut Ctx, args: &[Value]) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?.trunc();
    if r <= 0.0 || n < 1.0 {
        return Err(ErrorKind::Num);
    }
    num(n * ((1.0 + r).powf(1.0 / n) - 1.0))
}

fn cumulative(ctx: &Ctx, args: &[Value], interest: bool) -> R {
    let r = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?;
    let pv = ctx.num(&args[2])?;
    let start = ctx.num(&args[3])?.trunc();
    let end = ctx.num(&args[4])?.trunc();
    let t = ctx.num(&args[5])?;
    if r <= 0.0 || n <= 0.0 || pv <= 0.0 || start < 1.0 || end < start || end > n || (t != 0.0 && t != 1.0) {
        return Err(ErrorKind::Num);
    }
    let p = pmt_f(r, n, pv, 0.0, t)?;
    let mut total = 0.0;
    let mut k = start;
    while k <= end {
        let i = ipmt_f(r, k, n, pv, 0.0, t)?;
        total += if interest { i } else { p - i };
        k += 1.0;
    }
    num(total)
}

fn cumipmt(ctx: &mut Ctx, args: &[Value]) -> R {
    cumulative(ctx, args, true)
}

fn cumprinc(ctx: &mut Ctx, args: &[Value]) -> R {
    cumulative(ctx, args, false)
}

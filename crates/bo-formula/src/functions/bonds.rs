//! Securities and the remaining financial functions (ECMA-376 Part 1, §18.17.7): coupon
//! schedules, bond price / yield / duration, discounted securities, Treasury bills, accrued
//! interest, French depreciation (`AMORLINC`, `AMORDEGRC`), `VDB`, `MIRR` and small helpers.
//!
//! Day counts follow the `basis` argument: 0 US (NASD) 30/360, 1 actual/actual, 2 actual/360,
//! 3 actual/365, 4 European 30/360. Coupon dates step back from maturity by 12/frequency
//! months, keeping month ends when maturity is a month end.

use super::datetime::{days_30_360, year_frac};
use super::{f, flatten, num, opt_bool, opt_num, s, FnSpec};
use crate::date::{days_in_month, serial_to_ymd, ymd_to_serial, DateSystem};
use crate::eval::{Ctx, R};
use crate::value::{ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("COUPPCD", 3, 4, couppcd),
    s("COUPNCD", 3, 4, coupncd),
    s("COUPNUM", 3, 4, coupnum),
    s("COUPDAYBS", 3, 4, coupdaybs),
    s("COUPDAYS", 3, 4, coupdays),
    s("COUPDAYSNC", 3, 4, coupdaysnc),
    s("PRICE", 6, 7, price),
    s("YIELD", 6, 7, yield_fn),
    s("DURATION", 5, 6, duration),
    s("MDURATION", 5, 6, mduration),
    s("ODDFPRICE", 8, 9, oddfprice),
    s("ODDFYIELD", 8, 9, oddfyield),
    s("ODDLPRICE", 7, 8, oddlprice),
    s("ODDLYIELD", 7, 8, oddlyield),
    s("ACCRINT", 6, 8, accrint),
    s("ACCRINTM", 4, 5, accrintm),
    s("DISC", 4, 5, disc),
    s("INTRATE", 4, 5, intrate),
    s("RECEIVED", 4, 5, received),
    s("PRICEDISC", 4, 5, pricedisc),
    s("YIELDDISC", 4, 5, yielddisc),
    s("PRICEMAT", 5, 6, pricemat),
    s("YIELDMAT", 5, 6, yieldmat),
    s("TBILLPRICE", 3, 3, tbillprice),
    s("TBILLYIELD", 3, 3, tbillyield),
    s("TBILLEQ", 3, 3, tbilleq),
    s("DOLLARDE", 2, 2, dollarde),
    s("DOLLARFR", 2, 2, dollarfr),
    f("FVSCHEDULE", 2, 2, fvschedule),
    s("ISPMT", 4, 4, ispmt),
    f("MIRR", 3, 3, mirr),
    s("PDURATION", 3, 3, pduration),
    s("RRI", 3, 3, rri),
    s("VDB", 5, 7, vdb),
    s("AMORLINC", 6, 7, amorlinc),
    s("AMORDEGRC", 6, 7, amordegrc),
];

// ------------------------------------------------------------------ dates

/// A date argument as a whole serial.
fn date_arg(ctx: &Ctx, v: &Value) -> Result<f64, ErrorKind> {
    let n = ctx.num(v)?.trunc();
    if n < 0.0 {
        return Err(ErrorKind::Num);
    }
    Ok(n)
}

fn basis_arg(ctx: &Ctx, args: &[Value], i: usize) -> Result<i64, ErrorKind> {
    let b = opt_num(ctx, args, i, 0.0)?.trunc() as i64;
    if !(0..=4).contains(&b) {
        return Err(ErrorKind::Num);
    }
    Ok(b)
}

fn freq_arg(ctx: &Ctx, v: &Value) -> Result<f64, ErrorKind> {
    let f = ctx.num(v)?.trunc();
    if f != 1.0 && f != 2.0 && f != 4.0 {
        return Err(ErrorKind::Num);
    }
    Ok(f)
}

fn ymd(serial: f64, sys: DateSystem) -> Result<(i64, u32, u32), ErrorKind> {
    serial_to_ymd(serial, sys).ok_or(ErrorKind::Num)
}

/// `months` months after `date`; the day is kept, clipped to the month, or set to the month's
/// end when `eom`.
fn add_months(date: f64, months: i64, eom: bool, sys: DateSystem) -> Result<f64, ErrorKind> {
    let (y, m, d) = ymd(date, sys)?;
    let total = y * 12 + i64::from(m) - 1 + months;
    let (ny, nm) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    let last = days_in_month(ny, nm);
    let nd = if eom { last } else { d.min(last) };
    ymd_to_serial(ny, nm, nd, sys).ok_or(ErrorKind::Num)
}

fn is_month_end(date: f64, sys: DateSystem) -> Result<bool, ErrorKind> {
    let (y, m, d) = ymd(date, sys)?;
    Ok(d == days_in_month(y, m))
}

/// Days from `a` to `b` under `basis` (30/360 variants count months of 30 days).
fn day_count(a: f64, b: f64, basis: i64, sys: DateSystem) -> Result<f64, ErrorKind> {
    match basis {
        0 | 4 => Ok(days_30_360(ymd(a, sys)?, ymd(b, sys)?, basis == 0) as f64),
        _ => Ok(b - a),
    }
}

/// Days in a year under `basis` (actual/actual uses the average of the years spanned).
fn year_days(basis: i64) -> f64 {
    match basis {
        3 => 365.0,
        _ => 360.0,
    }
}

/// Coupon schedule around `settlement`: the previous and next coupon dates and the number of
/// coupons still to be paid.
struct Coupons {
    pcd: f64,
    ncd: f64,
    num: f64,
}

fn coupons(settle: f64, maturity: f64, freq: f64, sys: DateSystem) -> Result<Coupons, ErrorKind> {
    if settle >= maturity {
        return Err(ErrorKind::Num);
    }
    let step = (12.0 / freq) as i64;
    let eom = is_month_end(maturity, sys)?;
    let mut n: i64 = 1;
    let mut pcd = add_months(maturity, -step, eom, sys)?;
    while pcd > settle {
        n += 1;
        pcd = add_months(maturity, -step * n, eom, sys)?;
    }
    let ncd = add_months(maturity, -step * (n - 1), eom, sys)?;
    Ok(Coupons {
        pcd,
        ncd,
        num: n as f64,
    })
}

/// Days in the coupon period holding `settle`.
fn period_days(c: &Coupons, freq: f64, basis: i64) -> f64 {
    match basis {
        1 => c.ncd - c.pcd,
        3 => 365.0 / freq,
        _ => 360.0 / freq,
    }
}

/// (A, DSC, E): days from the previous coupon to settlement, from settlement to the next coupon
/// and in the period.
fn coupon_days(
    settle: f64,
    c: &Coupons,
    freq: f64,
    basis: i64,
    sys: DateSystem,
) -> Result<(f64, f64, f64), ErrorKind> {
    let e = period_days(c, freq, basis);
    let a = day_count(c.pcd, settle, basis, sys)?;
    let dsc = match basis {
        0 => e - a,
        4 => day_count(settle, c.ncd, 4, sys)?,
        _ => c.ncd - settle,
    };
    Ok((a, dsc, e))
}

/// settlement, maturity, frequency at `fi`, basis at `fi + 1`.
fn coupon_args(ctx: &Ctx, args: &[Value], fi: usize) -> Result<(f64, f64, f64, i64), ErrorKind> {
    let settle = date_arg(ctx, &args[0])?;
    let maturity = date_arg(ctx, &args[1])?;
    let freq = freq_arg(ctx, &args[fi])?;
    let basis = basis_arg(ctx, args, fi + 1)?;
    if settle >= maturity {
        return Err(ErrorKind::Num);
    }
    Ok((settle, maturity, freq, basis))
}

fn couppcd(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, _) = coupon_args(ctx, args, 2)?;
    Ok(Value::number(coupons(s, m, f, ctx.date_system)?.pcd))
}

fn coupncd(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, _) = coupon_args(ctx, args, 2)?;
    Ok(Value::number(coupons(s, m, f, ctx.date_system)?.ncd))
}

fn coupnum(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, _) = coupon_args(ctx, args, 2)?;
    Ok(Value::number(coupons(s, m, f, ctx.date_system)?.num))
}

fn coupdaybs(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, b) = coupon_args(ctx, args, 2)?;
    let c = coupons(s, m, f, ctx.date_system)?;
    num(coupon_days(s, &c, f, b, ctx.date_system)?.0)
}

fn coupdays(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, b) = coupon_args(ctx, args, 2)?;
    let c = coupons(s, m, f, ctx.date_system)?;
    num(period_days(&c, f, b))
}

fn coupdaysnc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (s, m, f, b) = coupon_args(ctx, args, 2)?;
    let c = coupons(s, m, f, ctx.date_system)?;
    num(coupon_days(s, &c, f, b, ctx.date_system)?.1)
}

// ------------------------------------------------------------------ coupon bonds

/// A bond's position in its coupon schedule: frequency, coupons left and the (A, DSC, E) days.
#[derive(Clone, Copy)]
struct Sched {
    freq: f64,
    n: f64,
    a: f64,
    dsc: f64,
    e: f64,
}

/// Price per 100 face value of a bond paying `rate`.
fn bond_price(rate: f64, yld: f64, redemption: f64, sc: Sched) -> f64 {
    let Sched { freq, n, a, dsc, e } = sc;
    let coupon = 100.0 * rate / freq;
    if n == 1.0 {
        return (redemption + coupon) / (1.0 + dsc / e * yld / freq) - coupon * a / e;
    }
    let base = 1.0 + yld / freq;
    let mut p = redemption / base.powf(n - 1.0 + dsc / e);
    for k in 1..=n as u64 {
        p += coupon / base.powf(k as f64 - 1.0 + dsc / e);
    }
    p - coupon * a / e
}

/// The y with `price(y) = target` for a decreasing `price` (bisection after bracketing).
fn solve_yield(price: impl Fn(f64) -> f64, target: f64) -> Result<f64, ErrorKind> {
    let (mut lo, mut hi) = (-0.999_999, 1.0);
    let mut grow = 0;
    while price(hi) > target {
        hi *= 2.0;
        grow += 1;
        if grow > 60 {
            return Err(ErrorKind::Num);
        }
    }
    if price(lo) < target {
        return Err(ErrorKind::Num);
    }
    for _ in 0..300 {
        let mid = f64::midpoint(lo, hi);
        if mid == lo || mid == hi {
            break;
        }
        if price(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(f64::midpoint(lo, hi))
}

fn bond_setup(ctx: &Ctx, args: &[Value]) -> Result<Sched, ErrorKind> {
    let (s, m, f, b) = coupon_args(ctx, args, 5)?;
    let c = coupons(s, m, f, ctx.date_system)?;
    let (a, dsc, e) = coupon_days(s, &c, f, b, ctx.date_system)?;
    Ok(Sched {
        freq: f,
        n: c.num,
        a,
        dsc,
        e,
    })
}

fn price(ctx: &mut Ctx, args: &[Value]) -> R {
    let rate = ctx.num(&args[2])?;
    let yld = ctx.num(&args[3])?;
    let red = ctx.num(&args[4])?;
    let sc = bond_setup(ctx, args)?;
    if rate < 0.0 || yld < 0.0 || red <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(bond_price(rate, yld, red, sc))
}

fn yield_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    let rate = ctx.num(&args[2])?;
    let pr = ctx.num(&args[3])?;
    let red = ctx.num(&args[4])?;
    let sc = bond_setup(ctx, args)?;
    let Sched {
        freq: f,
        n,
        a,
        dsc,
        e,
    } = sc;
    if rate < 0.0 || pr <= 0.0 || red <= 0.0 {
        return Err(ErrorKind::Num);
    }
    if n == 1.0 {
        let coupon = rate / f;
        let paid = pr / 100.0 + a / e * coupon;
        return num((red / 100.0 + coupon - paid) / paid * f * e / dsc);
    }
    num(solve_yield(|y| bond_price(rate, y, red, sc), pr)?)
}

/// Macaulay duration (years) of a bond with face value 100.
fn macaulay(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64), ErrorKind> {
    let (s, m, f, b) = coupon_args(ctx, args, 4)?;
    let rate = ctx.num(&args[2])?;
    let yld = ctx.num(&args[3])?;
    if rate < 0.0 || yld < 0.0 {
        return Err(ErrorKind::Num);
    }
    let c = coupons(s, m, f, ctx.date_system)?;
    let (_, dsc, e) = coupon_days(s, &c, f, b, ctx.date_system)?;
    let base = 1.0 + yld / f;
    let coupon = 100.0 * rate / f;
    let (mut weighted, mut total) = (0.0, 0.0);
    for k in 1..=c.num as u64 {
        let t = k as f64 - 1.0 + dsc / e;
        let cf = if k as f64 == c.num {
            coupon + 100.0
        } else {
            coupon
        };
        let pv = cf / base.powf(t);
        weighted += t * pv;
        total += pv;
    }
    Ok((weighted / total / f, yld, f))
}

fn duration(ctx: &mut Ctx, args: &[Value]) -> R {
    num(macaulay(ctx, args)?.0)
}

fn mduration(ctx: &mut Ctx, args: &[Value]) -> R {
    let (d, y, f) = macaulay(ctx, args)?;
    num(d / (1.0 + y / f))
}

// ------------------------------------------------------------------ odd periods

/// Quasi-coupon periods of `freq` from `from` forwards to `to` (exclusive start, inclusive end),
/// anchored on `anchor` (a coupon date of the schedule).
fn quasi_periods(
    anchor: f64,
    from: f64,
    to: f64,
    freq: f64,
    sys: DateSystem,
) -> Result<Vec<(f64, f64)>, ErrorKind> {
    let step = (12.0 / freq) as i64;
    let eom = is_month_end(anchor, sys)?;
    // Step back from the anchor to the period holding `from`.
    let mut k: i64 = 0;
    while add_months(anchor, -step * k, eom, sys)? > from {
        k += 1;
    }
    let mut out = Vec::new();
    let mut start = add_months(anchor, -step * k, eom, sys)?;
    let mut i = 1;
    while start < to {
        let end = add_months(anchor, -step * k + step * i, eom, sys)?;
        out.push((start, end));
        start = end;
        i += 1;
        if i > 10_000 {
            return Err(ErrorKind::Num);
        }
    }
    Ok(out)
}

/// Odd last period: Σ (days of `[a, b]` within each quasi period) / (its length).
fn quasi_fraction(
    periods: &[(f64, f64)],
    a: f64,
    b: f64,
    freq: f64,
    basis: i64,
    sys: DateSystem,
) -> Result<f64, ErrorKind> {
    let mut sum = 0.0;
    for &(ps, pe) in periods {
        let lo = a.max(ps);
        let hi = b.min(pe);
        if hi > lo {
            let len = match basis {
                1 => pe - ps,
                3 => 365.0 / freq,
                _ => 360.0 / freq,
            };
            sum += day_count(lo, hi, basis, sys)? / len;
        }
    }
    Ok(sum)
}

/// Arguments of the odd-last-period functions.
struct OddLast {
    settle: f64,
    maturity: f64,
    last: f64,
    rate: f64,
    value: f64,
    red: f64,
    freq: f64,
    basis: i64,
}

fn odd_last_args(ctx: &Ctx, args: &[Value]) -> Result<OddLast, ErrorKind> {
    let o = OddLast {
        settle: date_arg(ctx, &args[0])?,
        maturity: date_arg(ctx, &args[1])?,
        last: date_arg(ctx, &args[2])?,
        rate: ctx.num(&args[3])?,
        value: ctx.num(&args[4])?,
        red: ctx.num(&args[5])?,
        freq: freq_arg(ctx, &args[6])?,
        basis: basis_arg(ctx, args, 7)?,
    };
    if !(o.last < o.settle && o.settle < o.maturity)
        || o.rate < 0.0
        || o.value < 0.0
        || o.red <= 0.0
    {
        return Err(ErrorKind::Num);
    }
    Ok(o)
}

/// (Σ DC/NL, Σ A/NL, Σ DSC/NL) over the quasi periods of an odd last period.
fn odd_last_sums(o: &OddLast, sys: DateSystem) -> Result<(f64, f64, f64), ErrorKind> {
    let p = quasi_periods(o.last, o.last, o.maturity, o.freq, sys)?;
    let dc = quasi_fraction(&p, o.last, o.maturity, o.freq, o.basis, sys)?;
    let a = quasi_fraction(&p, o.last, o.settle, o.freq, o.basis, sys)?;
    let dsc = quasi_fraction(&p, o.settle, o.maturity, o.freq, o.basis, sys)?;
    Ok((dc, a, dsc))
}

fn oddlprice(ctx: &mut Ctx, args: &[Value]) -> R {
    let o = odd_last_args(ctx, args)?;
    let (dc, a, dsc) = odd_last_sums(&o, ctx.date_system)?;
    let c = 100.0 * o.rate / o.freq;
    num((o.red + dc * c) / (1.0 + dsc * o.value / o.freq) - a * c)
}

fn oddlyield(ctx: &mut Ctx, args: &[Value]) -> R {
    let o = odd_last_args(ctx, args)?;
    let (dc, a, dsc) = odd_last_sums(&o, ctx.date_system)?;
    let c = 100.0 * o.rate / o.freq;
    let paid = o.value + a * c;
    num((o.red + dc * c - paid) / paid * o.freq / dsc)
}

/// Arguments of the odd-first-period functions.
struct OddFirst {
    settle: f64,
    maturity: f64,
    issue: f64,
    first: f64,
    rate: f64,
    value: f64,
    red: f64,
    freq: f64,
    basis: i64,
}

fn odd_first_args(ctx: &Ctx, args: &[Value]) -> Result<OddFirst, ErrorKind> {
    let o = OddFirst {
        settle: date_arg(ctx, &args[0])?,
        maturity: date_arg(ctx, &args[1])?,
        issue: date_arg(ctx, &args[2])?,
        first: date_arg(ctx, &args[3])?,
        rate: ctx.num(&args[4])?,
        value: ctx.num(&args[5])?,
        red: ctx.num(&args[6])?,
        freq: freq_arg(ctx, &args[7])?,
        basis: basis_arg(ctx, args, 8)?,
    };
    if !(o.issue < o.settle && o.settle < o.first && o.first < o.maturity)
        || o.rate < 0.0
        || o.value < 0.0
        || o.red <= 0.0
    {
        return Err(ErrorKind::Num);
    }
    Ok(o)
}

/// Price of a bond whose first coupon period (issue → first coupon) is odd: the first coupon is
/// prorated over the quasi-coupon periods it spans, and settlement is discounted from the first
/// coupon date in quasi periods.
fn odd_first_price(o: &OddFirst, yld: f64, sys: DateSystem) -> Result<f64, ErrorKind> {
    let f = o.freq;
    let c = 100.0 * o.rate / f;
    let base = 1.0 + yld / f;
    // Regular coupons after the first one.
    let regular = coupons(o.first, o.maturity, f, sys)?.num;
    let periods = quasi_periods(o.first, o.issue, o.first, f, sys)?;
    // Accrued interest and the prorated first coupon over the quasi periods.
    let dc = quasi_fraction(&periods, o.issue, o.first, f, o.basis, sys)?;
    let a = quasi_fraction(&periods, o.issue, o.settle, f, o.basis, sys)?;
    // Time from settlement to the first coupon, in quasi periods.
    let to_first = quasi_fraction(&periods, o.settle, o.first, f, o.basis, sys)?;
    let mut p = c * dc / base.powf(to_first);
    for k in 1..=regular as u64 {
        p += c / base.powf(to_first + k as f64);
    }
    p += o.red / base.powf(to_first + regular);
    Ok(p - c * a)
}

fn oddfprice(ctx: &mut Ctx, args: &[Value]) -> R {
    let o = odd_first_args(ctx, args)?;
    num(odd_first_price(&o, o.value, ctx.date_system)?)
}

fn oddfyield(ctx: &mut Ctx, args: &[Value]) -> R {
    let o = odd_first_args(ctx, args)?;
    let sys = ctx.date_system;
    // The schedule does not depend on the yield: check it once.
    odd_first_price(&o, 0.0, sys)?;
    num(solve_yield(
        |y| odd_first_price(&o, y, sys).unwrap_or(f64::NAN),
        o.value,
    )?)
}

// ------------------------------------------------------------------ accrued interest

fn accrint(ctx: &mut Ctx, args: &[Value]) -> R {
    let issue = date_arg(ctx, &args[0])?;
    let first = date_arg(ctx, &args[1])?;
    let settle = date_arg(ctx, &args[2])?;
    let rate = ctx.num(&args[3])?;
    let par = opt_num(ctx, args, 4, 1000.0)?;
    let freq = freq_arg(ctx, &args[5])?;
    let basis = basis_arg(ctx, args, 6)?;
    let from_issue = opt_bool(ctx, args, 7, true)?;
    if rate <= 0.0 || par <= 0.0 || issue >= settle {
        return Err(ErrorKind::Num);
    }
    let sys = ctx.date_system;
    // Without `calc_method`, a settlement after the first coupon accrues from the last coupon.
    let start = if !from_issue && settle > first {
        let step = (12.0 / freq) as i64;
        let eom = is_month_end(first, sys)?;
        let mut k = 0;
        while add_months(first, step * (k + 1), eom, sys)? <= settle {
            k += 1;
        }
        add_months(first, step * k, eom, sys)?
    } else {
        issue
    };
    let periods = quasi_periods(first, start, settle, freq, sys)?;
    let frac = quasi_fraction(&periods, start, settle, freq, basis, sys)?;
    num(par * rate / freq * frac)
}

fn accrintm(ctx: &mut Ctx, args: &[Value]) -> R {
    let issue = date_arg(ctx, &args[0])?;
    let settle = date_arg(ctx, &args[1])?;
    let rate = ctx.num(&args[2])?;
    let par = opt_num(ctx, args, 3, 1000.0)?;
    let basis = basis_arg(ctx, args, 4)?;
    if rate <= 0.0 || par <= 0.0 || issue >= settle {
        return Err(ErrorKind::Num);
    }
    num(par * rate * year_frac(issue, settle, basis, ctx.date_system)?)
}

// ------------------------------------------------------------------ discounted securities

/// settlement, maturity, two numbers and the basis at 4; the year fraction of the term.
fn discount_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64), ErrorKind> {
    let s = date_arg(ctx, &args[0])?;
    let m = date_arg(ctx, &args[1])?;
    let a = ctx.num(&args[2])?;
    let b = ctx.num(&args[3])?;
    let basis = basis_arg(ctx, args, 4)?;
    if s >= m || a <= 0.0 || b <= 0.0 {
        return Err(ErrorKind::Num);
    }
    Ok((a, b, year_frac(s, m, basis, ctx.date_system)?))
}

fn disc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (pr, red, t) = discount_args(ctx, args)?;
    num((red - pr) / red / t)
}

fn intrate(ctx: &mut Ctx, args: &[Value]) -> R {
    let (inv, red, t) = discount_args(ctx, args)?;
    num((red - inv) / inv / t)
}

fn received(ctx: &mut Ctx, args: &[Value]) -> R {
    let (inv, d, t) = discount_args(ctx, args)?;
    let k = 1.0 - d * t;
    if k <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(inv / k)
}

fn pricedisc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (d, red, t) = discount_args(ctx, args)?;
    num(red - d * red * t)
}

fn yielddisc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (pr, red, t) = discount_args(ctx, args)?;
    num((red - pr) / pr / t)
}

/// Securities paying interest at maturity: (DIM, A, DSM) as year fractions.
fn at_maturity(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64, f64, f64), ErrorKind> {
    let s = date_arg(ctx, &args[0])?;
    let m = date_arg(ctx, &args[1])?;
    let issue = date_arg(ctx, &args[2])?;
    let rate = ctx.num(&args[3])?;
    let v = ctx.num(&args[4])?;
    let basis = basis_arg(ctx, args, 5)?;
    if s >= m || issue > s || rate < 0.0 || v < 0.0 {
        return Err(ErrorKind::Num);
    }
    let sys = ctx.date_system;
    let b = year_days(basis);
    let frac = |a: f64, c: f64| -> Result<f64, ErrorKind> {
        if basis == 1 {
            year_frac(a, c, 1, sys)
        } else {
            Ok(day_count(a, c, basis, sys)? / b)
        }
    };
    Ok((frac(issue, m)?, frac(issue, s)?, frac(s, m)?, rate, v))
}

fn pricemat(ctx: &mut Ctx, args: &[Value]) -> R {
    let (dim, a, dsm, rate, yld) = at_maturity(ctx, args)?;
    num((100.0 + dim * rate * 100.0) / (1.0 + dsm * yld) - a * rate * 100.0)
}

fn yieldmat(ctx: &mut Ctx, args: &[Value]) -> R {
    let (dim, a, dsm, rate, pr) = at_maturity(ctx, args)?;
    if pr <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let paid = pr / 100.0 + a * rate;
    num((1.0 + dim * rate - paid) / paid / dsm)
}

// ------------------------------------------------------------------ Treasury bills

fn tbill_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64), ErrorKind> {
    let s = date_arg(ctx, &args[0])?;
    let m = date_arg(ctx, &args[1])?;
    let v = ctx.num(&args[2])?;
    let dsm = m - s;
    if dsm <= 0.0 || v <= 0.0 {
        return Err(ErrorKind::Num);
    }
    // At most one year after settlement.
    if m > add_months(s, 12, false, ctx.date_system)? {
        return Err(ErrorKind::Num);
    }
    Ok((dsm, v))
}

fn tbillprice(ctx: &mut Ctx, args: &[Value]) -> R {
    let (dsm, d) = tbill_args(ctx, args)?;
    let p = 100.0 * (1.0 - d * dsm / 360.0);
    if p <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(p)
}

fn tbillyield(ctx: &mut Ctx, args: &[Value]) -> R {
    let (dsm, pr) = tbill_args(ctx, args)?;
    num((100.0 - pr) / pr * 360.0 / dsm)
}

fn tbilleq(ctx: &mut Ctx, args: &[Value]) -> R {
    let (dsm, d) = tbill_args(ctx, args)?;
    if dsm <= 182.0 {
        return num(365.0 * d / (360.0 - d * dsm));
    }
    // Longer bills: the bond-equivalent yield with one semi-annual compounding.
    let price = 100.0 * (1.0 - d * dsm / 360.0);
    let t = dsm / 365.0;
    let disc = t * t - (2.0 * t - 1.0) * (1.0 - 100.0 / price);
    if disc < 0.0 {
        return Err(ErrorKind::Num);
    }
    num((-t + disc.sqrt()) / (t - 0.5))
}

// ------------------------------------------------------------------ small helpers

fn fraction_arg(ctx: &Ctx, v: &Value) -> Result<f64, ErrorKind> {
    let f = ctx.num(v)?.trunc();
    if f < 0.0 {
        return Err(ErrorKind::Num);
    }
    if f == 0.0 {
        return Err(ErrorKind::Div0);
    }
    Ok(f)
}

fn dollarde(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let f = fraction_arg(ctx, &args[1])?;
    let digits = 10f64.powf(f.log10().ceil());
    let int = x.trunc();
    num(int + (x - int) * digits / f)
}

fn dollarfr(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let f = fraction_arg(ctx, &args[1])?;
    let digits = 10f64.powf(f.log10().ceil());
    let int = x.trunc();
    num(int + (x - int) * f / digits)
}

fn fvschedule(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut p = ctx.num(&args[0])?;
    for r in flatten(ctx, &args[1]) {
        match r {
            Value::Number(r) => p *= 1.0 + r,
            Value::Empty => {}
            Value::Error(e) => return Err(e),
            _ => return Err(ErrorKind::Value),
        }
    }
    num(p)
}

fn ispmt(ctx: &mut Ctx, args: &[Value]) -> R {
    let rate = ctx.num(&args[0])?;
    let per = ctx.num(&args[1])?;
    let nper = ctx.num(&args[2])?;
    let pv = ctx.num(&args[3])?;
    if nper == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(pv * rate * (per / nper - 1.0))
}

fn mirr(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut v = Vec::new();
    for x in flatten(ctx, &args[0]) {
        match x {
            Value::Number(n) => v.push(n),
            Value::Error(e) => return Err(e),
            _ => {}
        }
    }
    let fr = ctx.num(&args[1])?;
    let rr = ctx.num(&args[2])?;
    let n = v.len() as f64;
    let (mut npv_pos, mut npv_neg) = (0.0, 0.0);
    for (i, x) in v.iter().enumerate() {
        let i = i as f64;
        if *x > 0.0 {
            npv_pos += x / (1.0 + rr).powf(i);
        } else {
            npv_neg += x / (1.0 + fr).powf(i);
        }
    }
    if npv_pos == 0.0 || npv_neg == 0.0 || n < 2.0 {
        return Err(ErrorKind::Div0);
    }
    // ((FV of the inflows at rr) / (PV of the outflows at fr))^(1/(n−1)) − 1.
    num((-npv_pos * (1.0 + rr).powf(n - 1.0) / npv_neg).powf(1.0 / (n - 1.0)) - 1.0)
}

fn pduration(ctx: &mut Ctx, args: &[Value]) -> R {
    let rate = ctx.num(&args[0])?;
    let pv = ctx.num(&args[1])?;
    let fv = ctx.num(&args[2])?;
    if rate <= 0.0 || pv <= 0.0 || fv <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num((fv.ln() - pv.ln()) / rate.ln_1p())
}

fn rri(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?;
    let pv = ctx.num(&args[1])?;
    let fv = ctx.num(&args[2])?;
    if n <= 0.0 || pv == 0.0 {
        return Err(ErrorKind::Num);
    }
    num((fv / pv).powf(1.0 / n) - 1.0)
}

/// Depreciation of each whole period under declining balance with `factor`, switching to
/// straight line over the remaining life when that is larger (unless `no_switch`).
fn vdb_periods(cost: f64, salvage: f64, life: f64, factor: f64, no_switch: bool) -> Vec<f64> {
    let mut out = Vec::new();
    let mut book = cost;
    let n = life.ceil() as usize;
    let mut sl: Option<f64> = None;
    for p in 0..n {
        let remaining_life = life - p as f64;
        let ddb = (book * factor / life).min((book - salvage).max(0.0));
        let dep = if let Some(d) = sl {
            d.min((book - salvage).max(0.0))
        } else {
            let line = (book - salvage) / remaining_life;
            if !no_switch && line > ddb {
                sl = Some(line);
                line
            } else {
                ddb
            }
        };
        out.push(dep);
        book -= dep;
    }
    out
}

fn vdb(ctx: &mut Ctx, args: &[Value]) -> R {
    let cost = ctx.num(&args[0])?;
    let salvage = ctx.num(&args[1])?;
    let life = ctx.num(&args[2])?;
    let start = ctx.num(&args[3])?;
    let end = ctx.num(&args[4])?;
    let factor = opt_num(ctx, args, 5, 2.0)?;
    let no_switch = opt_bool(ctx, args, 6, false)?;
    if cost < 0.0
        || salvage < 0.0
        || life <= 0.0
        || start < 0.0
        || end < start
        || end > life
        || factor <= 0.0
    {
        return Err(ErrorKind::Num);
    }
    let per = vdb_periods(cost, salvage, life, factor, no_switch);
    // Cumulative depreciation up to a (possibly fractional) time.
    let cum = |t: f64| -> f64 {
        let whole = t.floor() as usize;
        let mut s: f64 = per.iter().take(whole).sum();
        if let Some(next) = per.get(whole) {
            s += next * (t - t.floor());
        }
        s
    };
    num(cum(end) - cum(start))
}

/// cost, purchase date, end of the first period, salvage, period, rate, basis.
type AmorArgs = (f64, f64, f64, f64, f64, f64, i64);

fn amor_args(ctx: &Ctx, args: &[Value]) -> Result<AmorArgs, ErrorKind> {
    let cost = ctx.num(&args[0])?;
    let bought = date_arg(ctx, &args[1])?;
    let first = date_arg(ctx, &args[2])?;
    let salvage = ctx.num(&args[3])?;
    let period = ctx.num(&args[4])?.trunc();
    let rate = ctx.num(&args[5])?;
    let basis = basis_arg(ctx, args, 6)?;
    if basis == 2
        || cost < 0.0
        || salvage < 0.0
        || salvage > cost
        || period < 0.0
        || rate <= 0.0
        || first < bought
    {
        return Err(ErrorKind::Num);
    }
    Ok((cost, bought, first, salvage, period, rate, basis))
}

fn amorlinc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (cost, bought, first, salvage, period, rate, basis) = amor_args(ctx, args)?;
    let full = cost * rate;
    let first_dep = year_frac(bought, first, basis, ctx.date_system)? * rate * cost;
    let delta = cost - salvage;
    let whole = ((delta - first_dep) / full).floor().max(0.0);
    num(if period == 0.0 {
        first_dep.min(delta)
    } else if period <= whole {
        full
    } else if period == whole + 1.0 {
        (delta - full * whole - first_dep).max(0.0)
    } else {
        0.0
    })
}

fn amordegrc(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mut cost, bought, first, salvage, period, rate, basis) = amor_args(ctx, args)?;
    // Coefficient by the asset's life 1/rate.
    let life = 1.0 / rate;
    let coef = if life < 3.0 {
        1.0
    } else if life < 5.0 {
        1.5
    } else if life <= 6.0 {
        2.0
    } else {
        2.5
    };
    let r = rate * coef;
    let mut dep = (year_frac(bought, first, basis, ctx.date_system)? * r * cost).round();
    cost -= dep;
    let mut rest = cost - salvage;
    for n in 0..period as u64 {
        dep = (r * cost).round();
        rest -= dep;
        if rest < 0.0 {
            // The period before the last takes half the remaining value, the last nothing more.
            return num(match period as u64 - n {
                0 | 1 => (cost * 0.5).round(),
                _ => 0.0,
            });
        }
        cost -= dep;
    }
    num(dep)
}

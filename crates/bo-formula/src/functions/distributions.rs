//! Probability distributions and the remaining descriptive statistics (ECMA-376 Part 1,
//! §18.17.7), with the compatibility names of the older functions.
//!
//! Special functions: log-gamma (Lanczos, g = 7), the regularised incomplete gamma function
//! (series / continued fraction) and the regularised incomplete beta function (continued
//! fraction, modified Lentz). Inverse distributions bisect the monotone CDF to full precision.

use std::f64::consts::PI;

use super::stats::{norm_cdf, norm_s_inv_f, phi};
use super::{f, flatten, gather, num, numbers, opt_num, s, shape, FnSpec, Gather, VAR};
use crate::eval::{Ctx, R};
use crate::value::{Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("BETA.DIST", 4, 6, beta_dist),
    s("BETADIST", 3, 5, betadist),
    s("BETA.INV", 3, 5, beta_inv),
    s("BETAINV", 3, 5, beta_inv),
    s("BINOM.DIST", 4, 4, binom_dist),
    s("BINOMDIST", 4, 4, binom_dist),
    s("BINOM.DIST.RANGE", 3, 4, binom_dist_range),
    s("BINOM.INV", 3, 3, binom_inv),
    s("CRITBINOM", 3, 3, binom_inv),
    s("CHISQ.DIST", 3, 3, chisq_dist),
    s("CHISQ.DIST.RT", 2, 2, chisq_dist_rt),
    s("CHIDIST", 2, 2, chisq_dist_rt),
    s("CHISQ.INV", 2, 2, chisq_inv),
    s("CHISQ.INV.RT", 2, 2, chisq_inv_rt),
    s("CHIINV", 2, 2, chisq_inv_rt),
    f("CHISQ.TEST", 2, 2, chisq_test),
    f("CHITEST", 2, 2, chisq_test),
    s("CONFIDENCE.NORM", 3, 3, confidence_norm),
    s("CONFIDENCE", 3, 3, confidence_norm),
    s("CONFIDENCE.T", 3, 3, confidence_t),
    s("EXPON.DIST", 3, 3, expon_dist),
    s("EXPONDIST", 3, 3, expon_dist),
    s("F.DIST", 4, 4, f_dist),
    s("F.DIST.RT", 3, 3, f_dist_rt),
    s("FDIST", 3, 3, f_dist_rt),
    s("F.INV", 3, 3, f_inv),
    s("F.INV.RT", 3, 3, f_inv_rt),
    s("FINV", 3, 3, f_inv_rt),
    f("F.TEST", 2, 2, f_test),
    f("FTEST", 2, 2, f_test),
    s("FISHER", 1, 1, fisher),
    s("FISHERINV", 1, 1, fisherinv),
    s("GAMMA", 1, 1, gamma),
    s("GAMMA.DIST", 4, 4, gamma_dist),
    s("GAMMADIST", 4, 4, gamma_dist),
    s("GAMMA.INV", 3, 3, gamma_inv),
    s("GAMMAINV", 3, 3, gamma_inv),
    s("GAMMALN", 1, 1, gammaln),
    s("GAMMALN.PRECISE", 1, 1, gammaln),
    s("GAUSS", 1, 1, gauss),
    s("PHI", 1, 1, phi_fn),
    s("HYPGEOM.DIST", 5, 5, hypgeom_dist),
    s("HYPGEOMDIST", 4, 4, hypgeomdist),
    s("LOGNORM.DIST", 4, 4, lognorm_dist),
    s("LOGNORMDIST", 3, 3, lognormdist),
    s("LOGNORM.INV", 3, 3, lognorm_inv),
    s("LOGINV", 3, 3, lognorm_inv),
    s("NEGBINOM.DIST", 4, 4, negbinom_dist),
    s("NEGBINOMDIST", 3, 3, negbinomdist),
    s("POISSON.DIST", 3, 3, poisson_dist),
    s("POISSON", 3, 3, poisson_dist),
    s("T.DIST", 3, 3, t_dist),
    s("T.DIST.2T", 2, 2, t_dist_2t),
    s("T.DIST.RT", 2, 2, t_dist_rt),
    s("TDIST", 3, 3, tdist),
    s("T.INV", 2, 2, t_inv),
    s("T.INV.2T", 2, 2, t_inv_2t),
    s("TINV", 2, 2, t_inv_2t),
    f("T.TEST", 4, 4, t_test),
    f("TTEST", 4, 4, t_test),
    s("WEIBULL.DIST", 4, 4, weibull_dist),
    s("WEIBULL", 4, 4, weibull_dist),
    f("Z.TEST", 2, 3, z_test),
    f("ZTEST", 2, 3, z_test),
    // Descriptive statistics
    f("KURT", 1, VAR, kurt),
    f("SKEW", 1, VAR, skew),
    f("SKEW.P", 1, VAR, skew_p),
    f("STDEVPA", 1, VAR, stdevpa),
    f("VARPA", 1, VAR, varpa),
    f("TRIMMEAN", 2, 2, trimmean),
    f("PERCENTRANK", 2, 3, percentrank_inc),
    f("PERCENTRANK.INC", 2, 3, percentrank_inc),
    f("PERCENTRANK.EXC", 2, 3, percentrank_exc),
    f("MODE.MULT", 1, VAR, mode_mult),
    f("FREQUENCY", 2, 2, frequency),
    f("PROB", 3, 4, prob),
    f("STEYX", 2, 2, steyx),
];

// ------------------------------------------------------------------ special functions

/// Lanczos coefficients for g = 7, n = 9.
const LANCZOS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// ln Γ(x) for x > 0.
pub(crate) fn ln_gamma(x: f64) -> f64 {
    if x < 0.5 {
        // Reflection: Γ(x) Γ(1−x) = π / sin(πx).
        return (PI / (PI * x).sin()).abs().ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = LANCZOS[0];
    let t = x + 7.5;
    for (i, c) in LANCZOS.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Γ(x); whole arguments up to 170 are exact products.
fn gamma_f(x: f64) -> f64 {
    if x.fract() == 0.0 && (1.0..=171.0).contains(&x) {
        return (1..x as u32).map(f64::from).product();
    }
    if x < 0.5 {
        return PI / ((PI * x).sin() * gamma_f(1.0 - x));
    }
    ln_gamma(x).exp()
}

/// ln B(a, b).
fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// Regularised lower incomplete gamma P(a, x).
pub(crate) fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // Σ xⁿ / (a (a+1) … (a+n))
        let mut term = 1.0 / a;
        let mut sum = term;
        let mut ap = a;
        for _ in 0..10_000 {
            ap += 1.0;
            term *= x / ap;
            sum += term;
            if term.abs() < sum.abs() * 1e-17 {
                break;
            }
        }
        (sum.ln() - x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        1.0 - gamma_q_cf(a, x)
    }
}

/// Regularised upper incomplete gamma Q(a, x) = 1 − P(a, x).
fn gamma_q(a: f64, x: f64) -> f64 {
    if x < a + 1.0 {
        1.0 - gamma_p(a, x)
    } else {
        gamma_q_cf(a, x)
    }
}

/// Q(a, x) by its continued fraction (modified Lentz).
fn gamma_q_cf(a: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..10_000 {
        let an = -f64::from(i) * (f64::from(i) - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x + a * x.ln() - ln_gamma(a)).exp() * h
}

/// Regularised incomplete beta function I_x(a, b).
pub(crate) fn beta_i(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front = (a * x.ln() + b * (1.0 - x).ln() - ln_beta(a, b)).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_cf(x, a, b) / a
    } else {
        1.0 - front * beta_cf(1.0 - x, b, a) / b
    }
}

/// The continued fraction of I_x(a, b) (modified Lentz).
fn beta_cf(x: f64, a: f64, b: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..10_000 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        // Even step d₂ₘ = m (b−m) x / ((a+2m−1)(a+2m)).
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        // Odd step d₂ₘ₊₁ = −(a+m)(a+b+m) x / ((a+2m)(a+2m+1)).
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h
}

/// The x in `lo..hi` with `cdf(x) = p` for an increasing `cdf` (the upper bound grows while
/// the CDF is still below `p`).
fn invert(cdf: impl Fn(f64) -> f64, p: f64, mut lo: f64, mut hi: f64) -> f64 {
    let mut grow = 0;
    while cdf(hi) < p && grow < 2000 {
        lo = hi;
        hi *= 2.0;
        grow += 1;
    }
    for _ in 0..400 {
        let mid = f64::midpoint(lo, hi);
        if mid == lo || mid == hi {
            break;
        }
        if cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// ln C(n, k).
fn ln_choose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

// ------------------------------------------------------------------ distribution kernels

fn chisq_cdf(x: f64, k: f64) -> f64 {
    gamma_p(k / 2.0, x / 2.0)
}

fn chisq_pdf(x: f64, k: f64) -> f64 {
    if x == 0.0 {
        return match k {
            k if k < 2.0 => f64::INFINITY,
            2.0 => 0.5,
            _ => 0.0,
        };
    }
    ((k / 2.0 - 1.0) * x.ln() - x / 2.0 - k / 2.0 * 2f64.ln() - ln_gamma(k / 2.0)).exp()
}

fn t_cdf(x: f64, v: f64) -> f64 {
    let tail = 0.5 * beta_i(v / (v + x * x), v / 2.0, 0.5);
    if x > 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

fn t_pdf(x: f64, v: f64) -> f64 {
    (ln_gamma(f64::midpoint(v, 1.0))
        - ln_gamma(v / 2.0)
        - 0.5 * (v * PI).ln()
        - f64::midpoint(v, 1.0) * (1.0 + x * x / v).ln())
    .exp()
}

fn f_cdf(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    beta_i(a * x / (a * x + b), a / 2.0, b / 2.0)
}

fn f_pdf(x: f64, a: f64, b: f64) -> f64 {
    if x == 0.0 {
        return if a < 2.0 {
            f64::INFINITY
        } else if a == 2.0 {
            1.0
        } else {
            0.0
        };
    }
    (0.5 * (a * (a * x).ln() + b * b.ln() - (a + b) * (a * x + b).ln())
        - x.ln()
        - ln_beta(a / 2.0, b / 2.0))
    .exp()
}

fn binom_pmf(k: f64, n: f64, p: f64) -> f64 {
    if p == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    if p == 1.0 {
        return if k == n { 1.0 } else { 0.0 };
    }
    (ln_choose(n, k) + k * p.ln() + (n - k) * (1.0 - p).ln()).exp()
}

fn binom_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k >= n {
        return 1.0;
    }
    if n <= 1000.0 {
        return (0..=k as u32)
            .map(|i| binom_pmf(f64::from(i), n, p))
            .sum::<f64>()
            .min(1.0);
    }
    beta_i(1.0 - p, n - k, k + 1.0)
}

fn poisson_pmf(k: f64, l: f64) -> f64 {
    if l == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    (k * l.ln() - l - ln_gamma(k + 1.0)).exp()
}

fn poisson_cdf(k: f64, l: f64) -> f64 {
    if k <= 1000.0 {
        return (0..=k as u32)
            .map(|i| poisson_pmf(f64::from(i), l))
            .sum::<f64>()
            .min(1.0);
    }
    gamma_q(k + 1.0, l)
}

// ------------------------------------------------------------------ argument helpers

fn n_at(ctx: &Ctx, args: &[Value], i: usize) -> Result<f64, ErrorKind> {
    ctx.num(&args[i])
}

fn b_at(ctx: &Ctx, args: &[Value], i: usize) -> Result<bool, ErrorKind> {
    ctx.boolean(&args[i])
}

/// Degrees of freedom: truncated, at least 1.
fn dof(x: f64) -> Result<f64, ErrorKind> {
    let d = x.trunc();
    if !(1.0..=1e10).contains(&d) {
        return Err(ErrorKind::Num);
    }
    Ok(d)
}

fn prob_open(p: f64) -> Result<f64, ErrorKind> {
    if p <= 0.0 || p >= 1.0 {
        return Err(ErrorKind::Num);
    }
    Ok(p)
}

fn prob_closed(p: f64) -> Result<f64, ErrorKind> {
    if !(0.0..=1.0).contains(&p) {
        return Err(ErrorKind::Num);
    }
    Ok(p)
}

// ------------------------------------------------------------------ beta

fn beta_core(ctx: &Ctx, args: &[Value], cum: bool, from: usize) -> R {
    let x = n_at(ctx, args, 0)?;
    let a = n_at(ctx, args, 1)?;
    let b = n_at(ctx, args, 2)?;
    let lo = opt_num(ctx, args, from, 0.0)?;
    let hi = opt_num(ctx, args, from + 1, 1.0)?;
    if a <= 0.0 || b <= 0.0 || lo >= hi || x < lo || x > hi {
        return Err(ErrorKind::Num);
    }
    let z = (x - lo) / (hi - lo);
    if cum {
        num(beta_i(z, a, b))
    } else {
        if (z == 0.0 && a < 1.0) || (z == 1.0 && b < 1.0) {
            return Err(ErrorKind::Num);
        }
        let ln = (a - 1.0) * z.ln() + (b - 1.0) * (1.0 - z).ln() - ln_beta(a, b);
        let d = if (z == 0.0 && a == 1.0) || (z == 1.0 && b == 1.0) {
            (-ln_beta(a, b)).exp()
        } else {
            ln.exp()
        };
        num(d / (hi - lo))
    }
}

fn beta_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let cum = b_at(ctx, args, 3)?;
    beta_core(ctx, args, cum, 4)
}

fn betadist(ctx: &mut Ctx, args: &[Value]) -> R {
    beta_core(ctx, args, true, 3)
}

fn beta_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = n_at(ctx, args, 0)?;
    let a = n_at(ctx, args, 1)?;
    let b = n_at(ctx, args, 2)?;
    let lo = opt_num(ctx, args, 3, 0.0)?;
    let hi = opt_num(ctx, args, 4, 1.0)?;
    if p <= 0.0 || p > 1.0 || a <= 0.0 || b <= 0.0 || lo >= hi {
        return Err(ErrorKind::Num);
    }
    let z = if p == 1.0 {
        1.0
    } else {
        invert(|z| beta_i(z, a, b), p, 0.0, 1.0)
    };
    num(lo + z * (hi - lo))
}

// ------------------------------------------------------------------ binomial

fn binom_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64), ErrorKind> {
    let k = n_at(ctx, args, 0)?.trunc();
    let n = n_at(ctx, args, 1)?.trunc();
    let p = prob_closed(n_at(ctx, args, 2)?)?;
    if k < 0.0 || k > n {
        return Err(ErrorKind::Num);
    }
    Ok((k, n, p))
}

fn binom_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let (k, n, p) = binom_args(ctx, args)?;
    if b_at(ctx, args, 3)? {
        num(binom_cdf(k, n, p))
    } else {
        num(binom_pmf(k, n, p))
    }
}

fn binom_dist_range(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = n_at(ctx, args, 0)?.trunc();
    let p = prob_closed(n_at(ctx, args, 1)?)?;
    let s1 = n_at(ctx, args, 2)?.trunc();
    let s2 = match args.get(3) {
        None | Some(Value::Empty) => s1,
        Some(v) => ctx.num(v)?.trunc(),
    };
    if n < 0.0 || s1 < 0.0 || s1 > n || s2 < s1 || s2 > n {
        return Err(ErrorKind::Num);
    }
    num((s1 as u64..=s2 as u64)
        .map(|k| binom_pmf(k as f64, n, p))
        .sum())
}

fn binom_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = n_at(ctx, args, 0)?.trunc();
    let p = prob_closed(n_at(ctx, args, 1)?)?;
    let alpha = prob_closed(n_at(ctx, args, 2)?)?;
    if n < 0.0 {
        return Err(ErrorKind::Num);
    }
    // The smallest k whose cumulative probability reaches alpha.
    let mut cum = 0.0;
    for k in 0..=n as u64 {
        cum += binom_pmf(k as f64, n, p);
        if cum >= alpha * (1.0 - 1e-15) {
            return Ok(Value::number(k as f64));
        }
    }
    Ok(Value::number(n))
}

// ------------------------------------------------------------------ chi-squared

fn chisq_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let k = dof(n_at(ctx, args, 1)?)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    if b_at(ctx, args, 2)? {
        num(chisq_cdf(x, k))
    } else {
        num(chisq_pdf(x, k))
    }
}

fn chisq_dist_rt(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let k = dof(n_at(ctx, args, 1)?)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    num(gamma_q(k / 2.0, x / 2.0))
}

fn chisq_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = prob_closed(n_at(ctx, args, 0)?)?;
    let k = dof(n_at(ctx, args, 1)?)?;
    if p == 1.0 {
        return Err(ErrorKind::Num);
    }
    num(invert(|x| chisq_cdf(x, k), p, 0.0, k.max(1.0)))
}

fn chisq_inv_rt(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = prob_closed(n_at(ctx, args, 0)?)?;
    let k = dof(n_at(ctx, args, 1)?)?;
    if p == 0.0 {
        return Err(ErrorKind::Num);
    }
    num(invert(|x| -gamma_q(k / 2.0, x / 2.0), -p, 0.0, k.max(1.0)))
}

fn chisq_test(ctx: &mut Ctx, args: &[Value]) -> R {
    let (r, c) = shape(ctx, &args[0]);
    if shape(ctx, &args[1]) != (r, c) {
        return Err(ErrorKind::NA);
    }
    let a = flatten(ctx, &args[0]);
    let e = flatten(ctx, &args[1]);
    let mut x2 = 0.0;
    for (a, e) in a.iter().zip(&e) {
        if let (Value::Number(a), Value::Number(e)) = (a, e) {
            if *e == 0.0 {
                return Err(ErrorKind::Div0);
            }
            x2 += (a - e) * (a - e) / e;
        }
    }
    let df = if r == 1 || c == 1 {
        (r * c) as f64 - 1.0
    } else {
        ((r - 1) * (c - 1)) as f64
    };
    if df < 1.0 {
        return Err(ErrorKind::NA);
    }
    num(gamma_q(df / 2.0, x2 / 2.0))
}

// ------------------------------------------------------------------ confidence

fn confidence_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64), ErrorKind> {
    let alpha = prob_open(n_at(ctx, args, 0)?)?;
    let sd = n_at(ctx, args, 1)?;
    let n = n_at(ctx, args, 2)?.trunc();
    if sd <= 0.0 || n < 1.0 {
        return Err(ErrorKind::Num);
    }
    Ok((alpha, sd, n))
}

fn confidence_norm(ctx: &mut Ctx, args: &[Value]) -> R {
    let (alpha, sd, n) = confidence_args(ctx, args)?;
    num(norm_s_inv_f(1.0 - alpha / 2.0)? * sd / n.sqrt())
}

fn confidence_t(ctx: &mut Ctx, args: &[Value]) -> R {
    let (alpha, sd, n) = confidence_args(ctx, args)?;
    if n == 1.0 {
        return Err(ErrorKind::Div0);
    }
    let t = invert(|x| t_cdf(x, n - 1.0), 1.0 - alpha / 2.0, 0.0, 10.0);
    num(t * sd / n.sqrt())
}

// ------------------------------------------------------------------ exponential, F

fn expon_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let l = n_at(ctx, args, 1)?;
    if x < 0.0 || l <= 0.0 {
        return Err(ErrorKind::Num);
    }
    if b_at(ctx, args, 2)? {
        num(-(-l * x).exp_m1())
    } else {
        num(l * (-l * x).exp())
    }
}

fn f_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64, f64), ErrorKind> {
    let x = n_at(ctx, args, 0)?;
    let a = dof(n_at(ctx, args, 1)?)?;
    let b = dof(n_at(ctx, args, 2)?)?;
    Ok((x, a, b))
}

fn f_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, a, b) = f_args(ctx, args)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    if b_at(ctx, args, 3)? {
        num(f_cdf(x, a, b))
    } else {
        num(f_pdf(x, a, b))
    }
}

fn f_rt(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    beta_i(b / (b + a * x), b / 2.0, a / 2.0)
}

fn f_dist_rt(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, a, b) = f_args(ctx, args)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    num(f_rt(x, a, b))
}

fn f_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let (p, a, b) = f_args(ctx, args)?;
    let p = prob_closed(p)?;
    if p == 1.0 {
        return Err(ErrorKind::Num);
    }
    num(invert(|x| f_cdf(x, a, b), p, 0.0, 2.0))
}

fn f_inv_rt(ctx: &mut Ctx, args: &[Value]) -> R {
    let (p, a, b) = f_args(ctx, args)?;
    let p = prob_closed(p)?;
    if p == 0.0 {
        return Err(ErrorKind::Num);
    }
    num(invert(|x| -f_rt(x, a, b), -p, 0.0, 2.0))
}

/// Sample variance and count of the numbers of a range.
fn sample_var(v: &[f64]) -> Result<(f64, f64), ErrorKind> {
    let n = v.len() as f64;
    if n < 2.0 {
        return Err(ErrorKind::Div0);
    }
    let m = v.iter().sum::<f64>() / n;
    Ok((
        v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0),
        n,
    ))
}

fn range_numbers(ctx: &Ctx, v: &Value) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => out.push(n),
            Value::Error(e) => return Err(e),
            _ => {}
        }
    }
    Ok(out)
}

fn f_test(ctx: &mut Ctx, args: &[Value]) -> R {
    let (v1, n1) = sample_var(&range_numbers(ctx, &args[0])?)?;
    let (v2, n2) = sample_var(&range_numbers(ctx, &args[1])?)?;
    if v1 == 0.0 || v2 == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let left = f_cdf(v1 / v2, n1 - 1.0, n2 - 1.0);
    num(2.0 * left.min(1.0 - left))
}

// ------------------------------------------------------------------ Fisher, gamma, Gauss

fn fisher(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    if x <= -1.0 || x >= 1.0 {
        return Err(ErrorKind::Num);
    }
    num(x.atanh())
}

fn fisherinv(ctx: &mut Ctx, args: &[Value]) -> R {
    num(n_at(ctx, args, 0)?.tanh())
}

fn gamma(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    if x <= 0.0 && x.fract() == 0.0 {
        return Err(ErrorKind::Num);
    }
    num(gamma_f(x))
}

fn gamma_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let a = n_at(ctx, args, 1)?;
    let b = n_at(ctx, args, 2)?;
    if x < 0.0 || a <= 0.0 || b <= 0.0 {
        return Err(ErrorKind::Num);
    }
    if b_at(ctx, args, 3)? {
        num(gamma_p(a, x / b))
    } else if x == 0.0 {
        num(if a < 1.0 {
            f64::INFINITY
        } else if a == 1.0 {
            1.0 / b
        } else {
            0.0
        })
    } else {
        num(((a - 1.0) * x.ln() - x / b - a * b.ln() - ln_gamma(a)).exp())
    }
}

fn gamma_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = prob_closed(n_at(ctx, args, 0)?)?;
    let a = n_at(ctx, args, 1)?;
    let b = n_at(ctx, args, 2)?;
    if a <= 0.0 || b <= 0.0 || p == 1.0 {
        return Err(ErrorKind::Num);
    }
    num(invert(|x| gamma_p(a, x / b), p, 0.0, (a * b).max(1.0)))
}

fn gammaln(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(ln_gamma(x))
}

fn gauss(ctx: &mut Ctx, args: &[Value]) -> R {
    num(norm_cdf(n_at(ctx, args, 0)?) - 0.5)
}

fn phi_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    num(phi(n_at(ctx, args, 0)?))
}

// ------------------------------------------------------------------ hypergeometric

/// P(X = s) and P(X ≤ s) for s successes in a sample of n from N with K successes.
fn hypgeom(s: f64, n: f64, k: f64, big_n: f64, cum: bool) -> Result<f64, ErrorKind> {
    if s < 0.0 || n <= 0.0 || k <= 0.0 || big_n <= 0.0 || n > big_n || k > big_n || s > n || s > k {
        return Err(ErrorKind::Num);
    }
    if s < (n - (big_n - k)).max(0.0) {
        return Err(ErrorKind::Num);
    }
    let pmf = |i: f64| (ln_choose(k, i) + ln_choose(big_n - k, n - i) - ln_choose(big_n, n)).exp();
    if !cum {
        return Ok(pmf(s));
    }
    let lo = (n - (big_n - k)).max(0.0) as u64;
    Ok((lo..=s as u64).map(|i| pmf(i as f64)).sum::<f64>().min(1.0))
}

fn hypgeom_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let v: Vec<f64> = (0..4)
        .map(|i| n_at(ctx, args, i).map(f64::trunc))
        .collect::<Result<_, _>>()?;
    num(hypgeom(v[0], v[1], v[2], v[3], b_at(ctx, args, 4)?)?)
}

fn hypgeomdist(ctx: &mut Ctx, args: &[Value]) -> R {
    let v: Vec<f64> = (0..4)
        .map(|i| n_at(ctx, args, i).map(f64::trunc))
        .collect::<Result<_, _>>()?;
    num(hypgeom(v[0], v[1], v[2], v[3], false)?)
}

// ------------------------------------------------------------------ log-normal

fn lognorm(ctx: &Ctx, args: &[Value], cum: bool) -> R {
    let x = n_at(ctx, args, 0)?;
    let mu = n_at(ctx, args, 1)?;
    let sd = n_at(ctx, args, 2)?;
    if x <= 0.0 || sd <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let z = (x.ln() - mu) / sd;
    if cum {
        num(norm_cdf(z))
    } else {
        num(phi(z) / (x * sd))
    }
}

fn lognorm_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let cum = b_at(ctx, args, 3)?;
    lognorm(ctx, args, cum)
}

fn lognormdist(ctx: &mut Ctx, args: &[Value]) -> R {
    lognorm(ctx, args, true)
}

fn lognorm_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = prob_open(n_at(ctx, args, 0)?)?;
    let mu = n_at(ctx, args, 1)?;
    let sd = n_at(ctx, args, 2)?;
    if sd <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num((mu + sd * norm_s_inv_f(p)?).exp())
}

// ------------------------------------------------------------------ negative binomial, Poisson

fn negbinom(ctx: &Ctx, args: &[Value], cum: bool) -> R {
    let fails = n_at(ctx, args, 0)?.trunc();
    let succ = n_at(ctx, args, 1)?.trunc();
    let p = prob_closed(n_at(ctx, args, 2)?)?;
    if fails < 0.0 || succ < 1.0 || p == 0.0 {
        return Err(ErrorKind::Num);
    }
    if cum {
        return num(beta_i(p, succ, fails + 1.0));
    }
    if p == 1.0 {
        return num(if fails == 0.0 { 1.0 } else { 0.0 });
    }
    num((ln_choose(fails + succ - 1.0, fails) + succ * p.ln() + fails * (1.0 - p).ln()).exp())
}

fn negbinom_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let cum = b_at(ctx, args, 3)?;
    negbinom(ctx, args, cum)
}

fn negbinomdist(ctx: &mut Ctx, args: &[Value]) -> R {
    negbinom(ctx, args, false)
}

fn poisson_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let k = n_at(ctx, args, 0)?.trunc();
    let l = n_at(ctx, args, 1)?;
    if k < 0.0 || l < 0.0 {
        return Err(ErrorKind::Num);
    }
    if b_at(ctx, args, 2)? {
        num(poisson_cdf(k, l))
    } else {
        num(poisson_pmf(k, l))
    }
}

// ------------------------------------------------------------------ Student t

fn t_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    if b_at(ctx, args, 2)? {
        num(t_cdf(x, v))
    } else {
        num(t_pdf(x, v))
    }
}

fn t_dist_2t(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    num(beta_i(v / (v + x * x), v / 2.0, 0.5))
}

fn t_dist_rt(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    num(1.0 - t_cdf(x, v))
}

fn tdist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    let tails = n_at(ctx, args, 2)?.trunc();
    if x < 0.0 || (tails != 1.0 && tails != 2.0) {
        return Err(ErrorKind::Num);
    }
    num(tails * 0.5 * beta_i(v / (v + x * x), v / 2.0, 0.5))
}

/// The t with P(T ≤ t) = p.
fn t_quantile(p: f64, v: f64) -> f64 {
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, v);
    }
    invert(|x| t_cdf(x, v), p, 0.0, 8.0)
}

fn t_inv(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = prob_open(n_at(ctx, args, 0)?)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    num(t_quantile(p, v))
}

fn t_inv_2t(ctx: &mut Ctx, args: &[Value]) -> R {
    let p = n_at(ctx, args, 0)?;
    let v = dof(n_at(ctx, args, 1)?)?;
    if p <= 0.0 || p > 1.0 {
        return Err(ErrorKind::Num);
    }
    num(t_quantile(1.0 - p / 2.0, v))
}

fn t_test(ctx: &mut Ctx, args: &[Value]) -> R {
    let tails = ctx.num(&args[2])?.trunc();
    let kind = ctx.num(&args[3])?.trunc();
    if (tails != 1.0 && tails != 2.0) || !(1.0..=3.0).contains(&kind) {
        return Err(ErrorKind::Num);
    }
    let (t, v) = if kind == 1.0 {
        // Paired: the differences of positions holding numbers in both ranges.
        if shape(ctx, &args[0]).0 * shape(ctx, &args[0]).1
            != shape(ctx, &args[1]).0 * shape(ctx, &args[1]).1
        {
            return Err(ErrorKind::NA);
        }
        let a = flatten(ctx, &args[0]);
        let b = flatten(ctx, &args[1]);
        let d: Vec<f64> = a
            .iter()
            .zip(&b)
            .filter_map(|p| match p {
                (Value::Number(x), Value::Number(y)) => Some(x - y),
                _ => None,
            })
            .collect();
        let (var, n) = sample_var(&d)?;
        if var == 0.0 {
            return Err(ErrorKind::Div0);
        }
        let m = d.iter().sum::<f64>() / n;
        (m / (var / n).sqrt(), n - 1.0)
    } else {
        let a = range_numbers(ctx, &args[0])?;
        let b = range_numbers(ctx, &args[1])?;
        let (va, na) = sample_var(&a)?;
        let (vb, nb) = sample_var(&b)?;
        let ma = a.iter().sum::<f64>() / na;
        let mb = b.iter().sum::<f64>() / nb;
        if kind == 2.0 {
            let pooled = ((na - 1.0) * va + (nb - 1.0) * vb) / (na + nb - 2.0);
            let se = (pooled * (1.0 / na + 1.0 / nb)).sqrt();
            if se == 0.0 {
                return Err(ErrorKind::Div0);
            }
            ((ma - mb) / se, na + nb - 2.0)
        } else {
            let (qa, qb) = (va / na, vb / nb);
            if qa + qb == 0.0 {
                return Err(ErrorKind::Div0);
            }
            // Welch–Satterthwaite degrees of freedom.
            let v = (qa + qb).powi(2) / (qa * qa / (na - 1.0) + qb * qb / (nb - 1.0));
            ((ma - mb) / (qa + qb).sqrt(), v)
        }
    };
    let t = t.abs();
    num(tails * 0.5 * beta_i(v / (v + t * t), v / 2.0, 0.5))
}

// ------------------------------------------------------------------ Weibull, z-test

fn weibull_dist(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n_at(ctx, args, 0)?;
    let a = n_at(ctx, args, 1)?;
    let b = n_at(ctx, args, 2)?;
    if x < 0.0 || a <= 0.0 || b <= 0.0 {
        return Err(ErrorKind::Num);
    }
    let z = (x / b).powf(a);
    if b_at(ctx, args, 3)? {
        num(-(-z).exp_m1())
    } else {
        num(a / b.powf(a) * x.powf(a - 1.0) * (-z).exp())
    }
}

fn z_test(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = range_numbers(ctx, &args[0])?;
    let mu = ctx.num(&args[1])?;
    let n = v.len() as f64;
    if n == 0.0 {
        return Err(ErrorKind::NA);
    }
    let sd = match args.get(2) {
        None | Some(Value::Empty) => sample_var(&v)?.0.sqrt(),
        Some(s) => ctx.num(s)?,
    };
    if sd <= 0.0 {
        return Err(ErrorKind::Div0);
    }
    let m = v.iter().sum::<f64>() / n;
    num(1.0 - norm_cdf((m - mu) / (sd / n.sqrt())))
}

// ------------------------------------------------------------------ descriptive statistics

fn mean_sd(v: &[f64]) -> Result<(f64, f64), ErrorKind> {
    let (var, n) = sample_var(v)?;
    Ok((v.iter().sum::<f64>() / n, var.sqrt()))
}

fn kurt(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    let n = v.len() as f64;
    if n < 4.0 {
        return Err(ErrorKind::Div0);
    }
    let (m, sd) = mean_sd(&v)?;
    if sd == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let s4: f64 = v.iter().map(|x| ((x - m) / sd).powi(4)).sum();
    num(n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0)) * s4
        - 3.0 * (n - 1.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0)))
}

fn skew(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    let n = v.len() as f64;
    if n < 3.0 {
        return Err(ErrorKind::Div0);
    }
    let (m, sd) = mean_sd(&v)?;
    if sd == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let s3: f64 = v.iter().map(|x| ((x - m) / sd).powi(3)).sum();
    num(n / ((n - 1.0) * (n - 2.0)) * s3)
}

fn skew_p(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    let n = v.len() as f64;
    if n < 1.0 {
        return Err(ErrorKind::Div0);
    }
    let m = v.iter().sum::<f64>() / n;
    let sd = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n).sqrt();
    if sd == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(v.iter().map(|x| ((x - m) / sd).powi(3)).sum::<f64>() / n)
}

const A_MODE: Gather = Gather {
    a_mode: true,
    skip_errors: false,
    skip_hidden: false,
    skip_nested: false,
};

fn pop_var_a(ctx: &Ctx, args: &[Value]) -> Result<f64, ErrorKind> {
    let v = gather(ctx, args, A_MODE)?;
    let n = v.len() as f64;
    if n == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let m = v.iter().sum::<f64>() / n;
    Ok(v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n)
}

fn stdevpa(ctx: &mut Ctx, args: &[Value]) -> R {
    num(pop_var_a(ctx, args)?.sqrt())
}

fn varpa(ctx: &mut Ctx, args: &[Value]) -> R {
    num(pop_var_a(ctx, args)?)
}

fn trimmean(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut v = range_numbers(ctx, &args[0])?;
    let pct = ctx.num(&args[1])?;
    if v.is_empty() || !(0.0..1.0).contains(&pct) {
        return Err(ErrorKind::Num);
    }
    v.sort_by(f64::total_cmp);
    // Points excluded: n·percent rounded down to an even number, half from each end.
    let cut = ((v.len() as f64 * pct / 2.0).floor()) as usize;
    let kept = &v[cut..v.len() - cut];
    num(kept.iter().sum::<f64>() / kept.len() as f64)
}

/// Percent rank of `x` in `v` with `rank(i) = f(count below, n)`, interpolated between the
/// neighbouring values and truncated to `sig` digits.
fn percentrank(ctx: &Ctx, args: &[Value], f: fn(f64, f64) -> f64) -> R {
    let mut v = range_numbers(ctx, &args[0])?;
    let x = ctx.num(&args[1])?;
    let sig = opt_num(ctx, args, 2, 3.0)?.trunc();
    if v.is_empty() || sig < 1.0 {
        return Err(ErrorKind::Num);
    }
    v.sort_by(f64::total_cmp);
    let n = v.len() as f64;
    if x < v[0] || x > v[v.len() - 1] {
        return Err(ErrorKind::NA);
    }
    let rank_of = |y: f64| f(v.iter().filter(|z| **z < y).count() as f64, n);
    let r = if v.contains(&x) {
        rank_of(x)
    } else {
        let lo = v
            .iter()
            .copied()
            .filter(|z| *z < x)
            .fold(f64::MIN, f64::max);
        let hi = v
            .iter()
            .copied()
            .filter(|z| *z > x)
            .fold(f64::MAX, f64::min);
        let (a, b) = (rank_of(lo), rank_of(hi));
        a + (x - lo) / (hi - lo) * (b - a)
    };
    let scale = 10f64.powf(sig);
    // Truncated (not rounded); the nudge keeps exact decimals such as 0.333… intact.
    num(((r * scale) + 1e-9).floor() / scale)
}

fn percentrank_inc(ctx: &mut Ctx, args: &[Value]) -> R {
    if range_numbers(ctx, &args[0])?.len() == 1 {
        let x = ctx.num(&args[1])?;
        let v = range_numbers(ctx, &args[0])?;
        return if (x - v[0]).abs() == 0.0 {
            Ok(Value::number(1.0))
        } else {
            Err(ErrorKind::NA)
        };
    }
    percentrank(ctx, args, |below, n| below / (n - 1.0))
}

fn percentrank_exc(ctx: &mut Ctx, args: &[Value]) -> R {
    percentrank(ctx, args, |below, n| (below + 1.0) / (n + 1.0))
}

fn mode_mult(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for x in &v {
        match counts.iter_mut().find(|(y, _)| y == x) {
            Some(c) => c.1 += 1,
            None => counts.push((*x, 1)),
        }
    }
    let best = counts.iter().map(|c| c.1).max().unwrap_or(0);
    if best < 2 {
        return Err(ErrorKind::NA);
    }
    let modes: Vec<Value> = counts
        .iter()
        .filter(|c| c.1 == best)
        .map(|c| Value::number(c.0))
        .collect();
    Array::column(modes)
        .map(|a| Value::Array(a.into()))
        .ok_or(ErrorKind::NA)
}

fn frequency(ctx: &mut Ctx, args: &[Value]) -> R {
    let data = range_numbers(ctx, &args[0])?;
    let bins = range_numbers(ctx, &args[1])?;
    let mut order: Vec<usize> = (0..bins.len()).collect();
    order.sort_by(|a, b| bins[*a].total_cmp(&bins[*b]));
    let mut counts = vec![0.0; bins.len() + 1];
    for x in data {
        // The smallest bin at or above the value; above every bin → the extra last count.
        match order.iter().find(|&&i| x <= bins[i]) {
            Some(&i) => counts[i] += 1.0,
            None => counts[bins.len()] += 1.0,
        }
    }
    Array::column(counts.into_iter().map(Value::number).collect())
        .map(|a| Value::Array(a.into()))
        .ok_or(ErrorKind::NA)
}

fn prob(ctx: &mut Ctx, args: &[Value]) -> R {
    let (r, c) = shape(ctx, &args[0]);
    if shape(ctx, &args[1]) != (r, c) {
        return Err(ErrorKind::NA);
    }
    let xs = flatten(ctx, &args[0]);
    let ps = flatten(ctx, &args[1]);
    let lo = ctx.num(&args[2])?;
    let hi = match args.get(3) {
        None | Some(Value::Empty) => lo,
        Some(v) => ctx.num(v)?,
    };
    let mut total = 0.0;
    let mut sum = 0.0;
    for (x, p) in xs.iter().zip(&ps) {
        let Value::Number(p) = p else { continue };
        if !(0.0..=1.0).contains(p) {
            return Err(ErrorKind::Num);
        }
        total += p;
        if let Value::Number(x) = x {
            if *x >= lo && *x <= hi {
                sum += p;
            }
        }
    }
    if (total - 1.0).abs() > 1e-7 {
        return Err(ErrorKind::Num);
    }
    num(sum)
}

fn steyx(ctx: &mut Ctx, args: &[Value]) -> R {
    let pairs = super::math::paired_numbers(ctx, args)?;
    let n = pairs.len() as f64;
    if n < 3.0 {
        return Err(ErrorKind::Div0);
    }
    let my = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let mx = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut syy, mut sxx, mut sxy) = (0.0, 0.0, 0.0);
    for (y, x) in &pairs {
        syy += (y - my) * (y - my);
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    if sxx == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(((syy - sxy * sxy / sxx) / (n - 2.0)).max(0.0).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_function_identities() {
        // Γ(1/2) = √π, Γ(5) = 24, I_x(a, b) = 1 − I_{1−x}(b, a), P + Q = 1.
        assert!((gamma_f(0.5) - PI.sqrt()).abs() < 1e-14);
        assert!((gamma_f(5.0) - 24.0).abs() < 1e-12);
        for (x, a, b) in [(0.3, 2.0, 5.0), (0.9, 0.5, 0.5), (0.5, 10.0, 3.0)] {
            assert!((beta_i(x, a, b) + beta_i(1.0 - x, b, a) - 1.0).abs() < 1e-14);
        }
        for (a, x) in [(0.5, 0.2), (3.0, 7.0), (10.0, 9.5)] {
            assert!((gamma_p(a, x) + gamma_q(a, x) - 1.0).abs() < 1e-14);
        }
        // χ² with 2 degrees of freedom is exponential with mean 2.
        assert!((chisq_cdf(3.0, 2.0) - (1.0 - (-1.5f64).exp())).abs() < 1e-14);
    }
}

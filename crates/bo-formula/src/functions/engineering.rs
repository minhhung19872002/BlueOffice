//! Engineering functions (ECMA-376 Part 1, §18.17.7): number-base conversion, bit operations,
//! `DELTA` / `GESTEP`, the error function, Bessel functions, complex numbers and `CONVERT`.
//!
//! Complex numbers are text `x+yi` / `x+yj` (§18.17.7 COMPLEX); numbers in results use the
//! General text form (15 significant digits).

use std::f64::consts::PI;

use super::{f, flatten, num, opt_num, s, FnSpec, VAR};
use crate::eval::{Ctx, R};
use crate::value::{number_to_text, round_sig15, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    // Number bases
    s("BIN2DEC", 1, 1, bin2dec),
    s("BIN2HEX", 1, 2, bin2hex),
    s("BIN2OCT", 1, 2, bin2oct),
    s("DEC2BIN", 1, 2, dec2bin),
    s("DEC2HEX", 1, 2, dec2hex),
    s("DEC2OCT", 1, 2, dec2oct),
    s("HEX2BIN", 1, 2, hex2bin),
    s("HEX2DEC", 1, 1, hex2dec),
    s("HEX2OCT", 1, 2, hex2oct),
    s("OCT2BIN", 1, 2, oct2bin),
    s("OCT2DEC", 1, 1, oct2dec),
    s("OCT2HEX", 1, 2, oct2hex),
    // Bits
    s("BITAND", 2, 2, bitand),
    s("BITOR", 2, 2, bitor),
    s("BITXOR", 2, 2, bitxor),
    s("BITLSHIFT", 2, 2, bitlshift),
    s("BITRSHIFT", 2, 2, bitrshift),
    // Comparison
    s("DELTA", 1, 2, delta),
    s("GESTEP", 1, 2, gestep),
    // Error function
    s("ERF", 1, 2, erf_fn),
    s("ERF.PRECISE", 1, 1, erf_precise),
    s("ERFC", 1, 1, erfc_fn),
    s("ERFC.PRECISE", 1, 1, erfc_fn),
    // Bessel functions
    s("BESSELJ", 2, 2, besselj),
    s("BESSELY", 2, 2, bessely),
    s("BESSELI", 2, 2, besseli),
    s("BESSELK", 2, 2, besselk),
    // Complex numbers
    s("COMPLEX", 2, 3, complex),
    s("IMABS", 1, 1, imabs),
    s("IMAGINARY", 1, 1, imaginary),
    s("IMREAL", 1, 1, imreal),
    s("IMARGUMENT", 1, 1, imargument),
    s("IMCONJUGATE", 1, 1, imconjugate),
    s("IMCOS", 1, 1, imcos),
    s("IMCOSH", 1, 1, imcosh),
    s("IMCOT", 1, 1, imcot),
    s("IMCSC", 1, 1, imcsc),
    s("IMCSCH", 1, 1, imcsch),
    s("IMSEC", 1, 1, imsec),
    s("IMSECH", 1, 1, imsech),
    s("IMSIN", 1, 1, imsin),
    s("IMSINH", 1, 1, imsinh),
    s("IMTAN", 1, 1, imtan),
    s("IMEXP", 1, 1, imexp),
    s("IMLN", 1, 1, imln),
    s("IMLOG10", 1, 1, imlog10),
    s("IMLOG2", 1, 1, imlog2),
    s("IMSQRT", 1, 1, imsqrt),
    s("IMPOWER", 2, 2, impower),
    s("IMDIV", 2, 2, imdiv),
    s("IMSUB", 2, 2, imsub),
    f("IMSUM", 1, VAR, imsum),
    f("IMPRODUCT", 1, VAR, improduct),
    // Units
    s("CONVERT", 3, 3, convert),
];

// ------------------------------------------------------------------ number bases

/// Digits of a base-`radix` argument (at most 10; a number argument is read as its digits).
fn base_digits(ctx: &Ctx, v: &Value) -> Result<String, ErrorKind> {
    let t = match ctx.scalar(v) {
        Value::Bool(_) => return Err(ErrorKind::Value),
        Value::Number(n) => number_to_text(n),
        Value::Error(e) => return Err(e),
        other => other.as_text()?,
    };
    if t.chars().count() > 10 {
        return Err(ErrorKind::Num);
    }
    Ok(t)
}

/// Value of a 10-digit two's-complement number in `radix` (2, 8 or 16).
fn parse_base(t: &str, radix: u32) -> Result<i64, ErrorKind> {
    let mut v: i64 = 0;
    for c in t.chars() {
        let d = c.to_digit(radix).ok_or(ErrorKind::Num)?;
        v = v * i64::from(radix) + i64::from(d);
    }
    let full = i64::from(radix).pow(10);
    // The top half of the 10-digit range is negative.
    if t.chars().count() == 10 && v >= full / 2 {
        v -= full;
    }
    Ok(v)
}

/// `n` in `radix` with 10-digit two's complement for negatives; `places` pads positives.
fn format_base(n: i64, radix: u32, places: Option<f64>) -> R {
    let full = i64::from(radix).pow(10);
    if n < -full / 2 || n >= full / 2 {
        return Err(ErrorKind::Num);
    }
    let digits = |mut x: i64| {
        if x == 0 {
            return "0".to_owned();
        }
        let mut out = Vec::new();
        while x > 0 {
            let d = u32::try_from(x % i64::from(radix)).unwrap_or(0);
            out.push(
                char::from_digit(d, radix)
                    .unwrap_or('0')
                    .to_ascii_uppercase(),
            );
            x /= i64::from(radix);
        }
        out.iter().rev().collect::<String>()
    };
    if n < 0 {
        return Ok(Value::Text(digits(full + n)));
    }
    let s = digits(n);
    match places {
        None => Ok(Value::Text(s)),
        Some(p) => {
            let p = p.trunc();
            if !(1.0..=10.0).contains(&p) || (p as usize) < s.len() {
                return Err(ErrorKind::Num);
            }
            Ok(Value::Text(format!("{s:0>width$}", width = p as usize)))
        }
    }
}

fn places(ctx: &Ctx, args: &[Value]) -> Result<Option<f64>, ErrorKind> {
    match args.get(1) {
        None | Some(Value::Empty) => Ok(None),
        Some(v) => ctx.num(v).map(Some),
    }
}

fn from_base(ctx: &Ctx, args: &[Value], radix: u32) -> Result<i64, ErrorKind> {
    parse_base(&base_digits(ctx, &args[0])?, radix)
}

fn base_to_base(ctx: &Ctx, args: &[Value], from: u32, to: u32) -> R {
    let n = from_base(ctx, args, from)?;
    format_base(n, to, places(ctx, args)?)
}

fn dec_to_base(ctx: &Ctx, args: &[Value], to: u32) -> R {
    let n = ctx.num(&args[0])?.trunc();
    if n.abs() > 1e12 {
        return Err(ErrorKind::Num);
    }
    format_base(n as i64, to, places(ctx, args)?)
}

fn bin2dec(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::number(from_base(ctx, args, 2)? as f64))
}
fn oct2dec(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::number(from_base(ctx, args, 8)? as f64))
}
fn hex2dec(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::number(from_base(ctx, args, 16)? as f64))
}
fn bin2hex(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 2, 16)
}
fn bin2oct(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 2, 8)
}
fn hex2bin(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 16, 2)
}
fn hex2oct(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 16, 8)
}
fn oct2bin(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 8, 2)
}
fn oct2hex(ctx: &mut Ctx, args: &[Value]) -> R {
    base_to_base(ctx, args, 8, 16)
}
fn dec2bin(ctx: &mut Ctx, args: &[Value]) -> R {
    dec_to_base(ctx, args, 2)
}
fn dec2oct(ctx: &mut Ctx, args: &[Value]) -> R {
    dec_to_base(ctx, args, 8)
}
fn dec2hex(ctx: &mut Ctx, args: &[Value]) -> R {
    dec_to_base(ctx, args, 16)
}

// ------------------------------------------------------------------ bits

/// Bit operations work on whole numbers in `0 .. 2^48`.
const BIT_LIMIT: f64 = 281_474_976_710_656.0;

fn bit_arg(ctx: &Ctx, v: &Value) -> Result<u64, ErrorKind> {
    let n = ctx.num(v)?;
    if n < 0.0 || n >= BIT_LIMIT || n.fract() != 0.0 {
        return Err(ErrorKind::Num);
    }
    Ok(n as u64)
}

fn bits(ctx: &Ctx, args: &[Value], op: fn(u64, u64) -> u64) -> R {
    let (a, b) = (bit_arg(ctx, &args[0])?, bit_arg(ctx, &args[1])?);
    Ok(Value::number(op(a, b) as f64))
}

fn bitand(ctx: &mut Ctx, args: &[Value]) -> R {
    bits(ctx, args, |a, b| a & b)
}
fn bitor(ctx: &mut Ctx, args: &[Value]) -> R {
    bits(ctx, args, |a, b| a | b)
}
fn bitxor(ctx: &mut Ctx, args: &[Value]) -> R {
    bits(ctx, args, |a, b| a ^ b)
}

fn shift(ctx: &Ctx, args: &[Value], sign: f64) -> R {
    let n = bit_arg(ctx, &args[0])?;
    let k = ctx.num(&args[1])?.trunc() * sign;
    if k.abs() > 53.0 {
        return Err(ErrorKind::Num);
    }
    let r = if k >= 0.0 {
        (n as f64) * 2f64.powi(k as i32)
    } else {
        (n >> (-k) as u32) as f64
    };
    if r >= BIT_LIMIT {
        return Err(ErrorKind::Num);
    }
    Ok(Value::number(r))
}

fn bitlshift(ctx: &mut Ctx, args: &[Value]) -> R {
    shift(ctx, args, 1.0)
}
fn bitrshift(ctx: &mut Ctx, args: &[Value]) -> R {
    shift(ctx, args, -1.0)
}

// ------------------------------------------------------------------ comparison

fn delta(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = ctx.num(&args[0])?;
    let b = opt_num(ctx, args, 1, 0.0)?;
    Ok(Value::number(if a == b { 1.0 } else { 0.0 }))
}

fn gestep(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = ctx.num(&args[0])?;
    let b = opt_num(ctx, args, 1, 0.0)?;
    Ok(Value::number(if a >= b { 1.0 } else { 0.0 }))
}

// ------------------------------------------------------------------ error function

/// erf(x) = 2/√π ∫₀ˣ e^(−t²) dt: Maclaurin series near 0, the continued fraction of erfc beyond.
pub(crate) fn erf(x: f64) -> f64 {
    if x.abs() >= 2.5 {
        return x.signum() * (1.0 - erfc_cf(x.abs()));
    }
    // Σ (−1)ⁿ x^(2n+1) / (n! (2n+1))
    let mut term = x;
    let mut sum = x;
    let x2 = x * x;
    for n in 1..200 {
        term *= -x2 / f64::from(n);
        let add = term / f64::from(2 * n + 1);
        sum += add;
        if add.abs() < 1e-17 * sum.abs() {
            break;
        }
    }
    sum * 2.0 / PI.sqrt()
}

/// erfc(x) = 1 − erf(x).
pub(crate) fn erfc(x: f64) -> f64 {
    if x >= 2.5 {
        erfc_cf(x)
    } else {
        1.0 - erf(x)
    }
}

/// erfc for x ≥ 2.5: e^(−x²)/√π · 1/(x + ½/(x + 1/(x + 3⁄2/(x + …)))).
fn erfc_cf(x: f64) -> f64 {
    let mut f = x;
    for k in (1..=200).rev() {
        f = x + f64::from(k) / 2.0 / f;
    }
    (-x * x).exp() / PI.sqrt() / f
}

fn erf_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    let lower = ctx.num(&args[0])?;
    match args.get(1) {
        None | Some(Value::Empty) => num(erf(lower)),
        Some(u) => {
            let upper = ctx.num(u)?;
            num(erf(upper) - erf(lower))
        }
    }
}

fn erf_precise(ctx: &mut Ctx, args: &[Value]) -> R {
    num(erf(ctx.num(&args[0])?))
}

fn erfc_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    num(erfc(ctx.num(&args[0])?))
}

// ------------------------------------------------------------------ Bessel functions

use std::f64::consts::EULER_GAMMA;

/// J₀(x) … J_kmax(x) for x > 0 by Miller's backward recurrence, normalised with
/// J₀ + 2 Σ J₂ₖ = 1.
fn bessel_j_all(x: f64, kmax: usize) -> Vec<f64> {
    let top = kmax.max(x.ceil() as usize);
    let m = {
        let m = top + 20 + (40.0 * top as f64).sqrt() as usize;
        m + m % 2
    };
    let mut j = vec![0.0; m + 2];
    j[m] = 1e-30;
    for k in (1..=m).rev() {
        j[k - 1] = 2.0 * k as f64 / x * j[k] - j[k + 1];
        if j[k - 1].abs() > 1e250 {
            for v in &mut j[k - 1..] {
                *v *= 1e-250;
            }
        }
    }
    let norm: f64 = j[0] + 2.0 * j.iter().skip(2).step_by(2).sum::<f64>();
    j.truncate(kmax.max(1) + 1);
    for v in &mut j {
        *v /= norm;
    }
    j
}

fn bessel_j(n: usize, x: f64) -> f64 {
    if x == 0.0 {
        return if n == 0 { 1.0 } else { 0.0 };
    }
    let v = bessel_j_all(x.abs(), n)[n];
    if x < 0.0 && n % 2 == 1 {
        -v
    } else {
        v
    }
}

/// Y₀ and Y₁ from the Neumann series (A&S 9.1.89 and its derivative), then
/// Y_{k+1} = (2k/x) Y_k − Y_{k−1}.
fn bessel_y(n: usize, x: f64) -> f64 {
    let kmax = 2 * (x.ceil() as usize + 40);
    let j = bessel_j_all(x, kmax + 1);
    let l = (x / 2.0).ln() + EULER_GAMMA;
    let mut s0 = 0.0;
    let mut s1 = 0.0;
    let mut k = 1;
    while 2 * k < j.len() - 1 {
        let sign = if k % 2 == 1 { -1.0 } else { 1.0 };
        s0 += sign * j[2 * k] / k as f64;
        s1 += sign * (j[2 * k - 1] - j[2 * k + 1]) / k as f64;
        k += 1;
    }
    let y0 = 2.0 / PI * (l * j[0] - 2.0 * s0);
    let y1 = -2.0 / PI * (j[0] / x - l * j[1]) + 2.0 / PI * s1;
    if n == 0 {
        return y0;
    }
    let (mut a, mut b) = (y0, y1);
    for k in 1..n {
        let c = 2.0 * k as f64 / x * b - a;
        a = b;
        b = c;
    }
    b
}

/// I_n(x) = Σ (x/2)^(2k+n) / (k! (k+n)!).
fn bessel_i(n: usize, x: f64) -> f64 {
    if x == 0.0 {
        return if n == 0 { 1.0 } else { 0.0 };
    }
    let ax = x.abs();
    let ln_fact: f64 = (1..=n).map(|i| (i as f64).ln()).sum();
    let mut term = (n as f64 * (ax / 2.0).ln() - ln_fact).exp();
    let mut sum = term;
    let q = ax * ax / 4.0;
    for k in 1..100_000 {
        term *= q / (k as f64 * (k + n) as f64);
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    if x < 0.0 && n % 2 == 1 {
        -sum
    } else {
        sum
    }
}

/// K_n(x) = ∫₀^∞ e^(−x cosh t) cosh(n t) dt by the trapezoid rule (exponentially convergent
/// for this analytic, double-exponentially decaying integrand).
fn bessel_k(n: usize, x: f64) -> f64 {
    let h = 0.0625;
    let nf = n as f64;
    // Scaled by e^x so the integrand stays representable.
    let g = |t: f64| (-x * (t.cosh() - 1.0)).exp() * (nf * t).cosh();
    let mut sum = g(0.0) / 2.0;
    let mut k = 1;
    loop {
        let t = k as f64 * h;
        let v = g(t);
        sum += v;
        if (v < 1e-18 * sum && x * (t.cosh() - 1.0) > nf * t) || k > 200_000 {
            break;
        }
        k += 1;
    }
    sum * h * (-x).exp()
}

fn bessel_args(ctx: &Ctx, args: &[Value]) -> Result<(f64, usize), ErrorKind> {
    let x = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?.trunc();
    if n < 0.0 || n > 1e5 {
        return Err(ErrorKind::Num);
    }
    Ok((x, n as usize))
}

fn besselj(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, n) = bessel_args(ctx, args)?;
    num(bessel_j(n, x))
}

fn bessely(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, n) = bessel_args(ctx, args)?;
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(bessel_y(n, x))
}

fn besseli(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, n) = bessel_args(ctx, args)?;
    num(bessel_i(n, x))
}

fn besselk(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, n) = bessel_args(ctx, args)?;
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(bessel_k(n, x))
}

// ------------------------------------------------------------------ complex numbers

#[derive(Clone, Copy, Debug, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    const fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    fn div(self, o: C) -> Result<C, ErrorKind> {
        let d = o.re * o.re + o.im * o.im;
        if d == 0.0 {
            return Err(ErrorKind::Num);
        }
        Ok(C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        ))
    }
    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    fn exp(self) -> C {
        let e = self.re.exp();
        C::new(e * self.im.cos(), e * self.im.sin())
    }
    fn ln(self) -> Result<C, ErrorKind> {
        if self.re == 0.0 && self.im == 0.0 {
            return Err(ErrorKind::Num);
        }
        Ok(C::new(self.abs().ln(), self.arg()))
    }
    fn sin(self) -> C {
        C::new(
            self.re.sin() * self.im.cosh(),
            self.re.cos() * self.im.sinh(),
        )
    }
    fn cos(self) -> C {
        C::new(
            self.re.cos() * self.im.cosh(),
            -self.re.sin() * self.im.sinh(),
        )
    }
    fn sinh(self) -> C {
        C::new(
            self.re.sinh() * self.im.cos(),
            self.re.cosh() * self.im.sin(),
        )
    }
    fn cosh(self) -> C {
        C::new(
            self.re.cosh() * self.im.cos(),
            self.re.sinh() * self.im.sin(),
        )
    }
    fn powf(self, n: f64) -> Result<C, ErrorKind> {
        if self.re == 0.0 && self.im == 0.0 {
            return if n > 0.0 {
                Ok(C::new(0.0, 0.0))
            } else {
                Err(ErrorKind::Num)
            };
        }
        // Whole powers by repeated squaring keep exact Gaussian integers exact.
        if n.fract() == 0.0 && n.abs() <= 1024.0 {
            let mut k = n.abs() as u32;
            let (mut base, mut acc) = (self, ONE);
            while k > 0 {
                if k & 1 == 1 {
                    acc = acc.mul(base);
                }
                base = base.mul(base);
                k >>= 1;
            }
            return if n < 0.0 { ONE.div(acc) } else { Ok(acc) };
        }
        let r = self.abs().powf(n);
        let t = self.arg() * n;
        Ok(C::new(r * t.cos(), r * t.sin()))
    }
}

const ONE: C = C::new(1.0, 0.0);

/// Text number fully parsed as a decimal (no `inf`/`nan` spellings).
fn plain_number(s: &str) -> Option<f64> {
    if s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
    {
        return None;
    }
    s.parse().ok()
}

/// A complex number argument and its suffix (`None` when it has no imaginary part).
fn parse_complex(ctx: &Ctx, v: &Value) -> Result<(C, Option<char>), ErrorKind> {
    let t = match ctx.scalar(v) {
        Value::Number(n) => return Ok((C::new(n, 0.0), None)),
        Value::Bool(_) => return Err(ErrorKind::Value),
        Value::Error(e) => return Err(e),
        Value::Empty => return Ok((C::new(0.0, 0.0), None)),
        other => other.as_text()?,
    };
    if t.is_empty() {
        return Ok((C::new(0.0, 0.0), None));
    }
    let Some(suffix) = t.chars().last().filter(|c| *c == 'i' || *c == 'j') else {
        return plain_number(&t)
            .map(|n| (C::new(n, 0.0), None))
            .ok_or(ErrorKind::Num);
    };
    let body = &t[..t.len() - 1];
    // The imaginary part starts at the last sign that is not an exponent's.
    let b = body.as_bytes();
    let split = (1..b.len())
        .rev()
        .find(|&k| (b[k] == b'+' || b[k] == b'-') && !matches!(b[k - 1], b'e' | b'E'));
    let (re, im) = match split {
        Some(k) => (&body[..k], &body[k..]),
        None => ("", body),
    };
    let re = if re.is_empty() {
        0.0
    } else {
        plain_number(re).ok_or(ErrorKind::Num)?
    };
    let im = match im {
        "" | "+" => 1.0,
        "-" => -1.0,
        s => plain_number(s).ok_or(ErrorKind::Num)?,
    };
    Ok((C::new(re, im), Some(suffix)))
}

/// `x+yi` text of a complex number (§18.17.7 COMPLEX).
fn format_complex(z: C, suffix: char) -> R {
    if !z.re.is_finite() || !z.im.is_finite() {
        return Err(ErrorKind::Num);
    }
    let (re, im) = (round_sig15(z.re), round_sig15(z.im));
    let imag = |im: f64| match im {
        1.0 => suffix.to_string(),
        -1.0 => format!("-{suffix}"),
        _ => format!("{}{suffix}", number_to_text(im)),
    };
    let s = if im == 0.0 {
        number_to_text(re)
    } else if re == 0.0 {
        imag(im)
    } else if im > 0.0 {
        format!("{}+{}", number_to_text(re), imag(im))
    } else {
        format!("{}{}", number_to_text(re), imag(im))
    };
    Ok(Value::Text(s))
}

/// Suffix shared by several arguments (`i` when none has one; mixing is `#VALUE!`).
fn common_suffix(list: &[Option<char>]) -> Result<char, ErrorKind> {
    let mut out: Option<char> = None;
    for s in list.iter().flatten() {
        match out {
            Some(o) if o != *s => return Err(ErrorKind::Value),
            _ => out = Some(*s),
        }
    }
    Ok(out.unwrap_or('i'))
}

fn complex(ctx: &mut Ctx, args: &[Value]) -> R {
    let re = ctx.num(&args[0])?;
    let im = ctx.num(&args[1])?;
    let suffix = match args.get(2) {
        None | Some(Value::Empty) => 'i',
        Some(v) => match ctx.text(v)?.as_str() {
            "" | "i" => 'i',
            "j" => 'j',
            _ => return Err(ErrorKind::Value),
        },
    };
    format_complex(C::new(re, im), suffix)
}

/// A one-argument complex function.
fn unary(ctx: &Ctx, args: &[Value], op: impl Fn(C) -> Result<C, ErrorKind>) -> R {
    let (z, s) = parse_complex(ctx, &args[0])?;
    format_complex(op(z)?, s.unwrap_or('i'))
}

fn imabs(ctx: &mut Ctx, args: &[Value]) -> R {
    num(parse_complex(ctx, &args[0])?.0.abs())
}
fn imaginary(ctx: &mut Ctx, args: &[Value]) -> R {
    num(parse_complex(ctx, &args[0])?.0.im)
}
fn imreal(ctx: &mut Ctx, args: &[Value]) -> R {
    num(parse_complex(ctx, &args[0])?.0.re)
}
fn imargument(ctx: &mut Ctx, args: &[Value]) -> R {
    let z = parse_complex(ctx, &args[0])?.0;
    if z.re == 0.0 && z.im == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(z.arg())
}
fn imconjugate(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(C::new(z.re, -z.im)))
}
fn imcos(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(z.cos()))
}
fn imcosh(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(z.cosh()))
}
fn imsin(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(z.sin()))
}
fn imsinh(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(z.sinh()))
}
fn imtan(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| z.sin().div(z.cos()))
}
fn imcot(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| z.cos().div(z.sin()))
}
fn imsec(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| ONE.div(z.cos()))
}
fn imcsc(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| ONE.div(z.sin()))
}
fn imsech(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| ONE.div(z.cosh()))
}
fn imcsch(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| ONE.div(z.sinh()))
}
fn imexp(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| Ok(z.exp()))
}
fn imln(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, C::ln)
}
fn imlog10(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| {
        let l = z.ln()?;
        Ok(C::new(
            l.re / std::f64::consts::LN_10,
            l.im / std::f64::consts::LN_10,
        ))
    })
}
fn imlog2(ctx: &mut Ctx, args: &[Value]) -> R {
    unary(ctx, args, |z| {
        let l = z.ln()?;
        Ok(C::new(
            l.re / std::f64::consts::LN_2,
            l.im / std::f64::consts::LN_2,
        ))
    })
}
fn imsqrt(ctx: &mut Ctx, args: &[Value]) -> R {
    // Principal root in algebraic form: √((|z|+x)/2) ± i √((|z|−x)/2).
    unary(ctx, args, |z| {
        let r = z.abs();
        let re = f64::midpoint(r, z.re).sqrt();
        let im = f64::midpoint(r, -z.re).sqrt();
        Ok(C::new(re, if z.im < 0.0 { -im } else { im }))
    })
}
fn impower(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[1])?;
    unary(ctx, args, |z| z.powf(n))
}

fn binary(ctx: &Ctx, args: &[Value], op: impl Fn(C, C) -> Result<C, ErrorKind>) -> R {
    let (a, sa) = parse_complex(ctx, &args[0])?;
    let (b, sb) = parse_complex(ctx, &args[1])?;
    let suffix = common_suffix(&[sa, sb])?;
    format_complex(op(a, b)?, suffix)
}

fn imdiv(ctx: &mut Ctx, args: &[Value]) -> R {
    binary(ctx, args, C::div)
}
fn imsub(ctx: &mut Ctx, args: &[Value]) -> R {
    binary(ctx, args, |a, b| Ok(a.sub(b)))
}

/// Fold every complex number of the arguments (ranges and arrays included; blanks skipped).
fn fold(ctx: &Ctx, args: &[Value], init: C, op: fn(C, C) -> C) -> R {
    let mut acc = init;
    let mut suffixes = Vec::new();
    for a in args {
        for v in flatten(ctx, a) {
            if matches!(v, Value::Empty) {
                continue;
            }
            let (z, s) = parse_complex(ctx, &v)?;
            suffixes.push(s);
            acc = op(acc, z);
        }
    }
    format_complex(acc, common_suffix(&suffixes)?)
}

fn imsum(ctx: &mut Ctx, args: &[Value]) -> R {
    fold(ctx, args, C::new(0.0, 0.0), C::add)
}
fn improduct(ctx: &mut Ctx, args: &[Value]) -> R {
    fold(ctx, args, ONE, C::mul)
}

// ------------------------------------------------------------------ CONVERT

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Mass,
    Distance,
    Time,
    Pressure,
    Force,
    Energy,
    Power,
    Magnetism,
    Temperature,
    Volume,
    Area,
    Information,
    Speed,
}

/// A unit: names, kind, size in the kind's SI unit, whether it takes a metric prefix and the
/// power the prefix is raised to (2 for areas such as `m2`, 3 for volumes).
struct Unit {
    names: &'static [&'static str],
    kind: Kind,
    factor: f64,
    prefix: bool,
    power: i32,
}

const fn u(names: &'static [&'static str], kind: Kind, factor: f64, prefix: bool) -> Unit {
    Unit {
        names,
        kind,
        factor,
        prefix,
        power: 1,
    }
}

const fn up(names: &'static [&'static str], kind: Kind, factor: f64, power: i32) -> Unit {
    Unit {
        names,
        kind,
        factor,
        prefix: true,
        power,
    }
}

// Exact definitions: international inch 0.0254 m, avoirdupois pound 0.45359237 kg,
// standard gravity 9.80665 m/s², US gallon 231 in³, UK gallon 4.54609 L, Julian year
// 365.25 d, light-year c·year, parsec 648000/π au (au = 149 597 870 700 m), US survey foot
// 1200/3937 m, IT calorie 4.1868 J, thermochemical calorie 4.184 J, IT Btu 1055.05585262 J,
// eV 1.602176634e-19 J (SI 2019), mechanical horsepower 550 ft·lbf/s, metric horsepower
// 75 kgf·m/s, mmHg 13.5951 g/cm³ · g₀ · 1 mm, atomic mass unit CODATA 2018.
const IN: f64 = 0.0254;
const FT: f64 = 0.3048;
const MI: f64 = 1609.344;
const NMI: f64 = 1852.0;
const LB: f64 = 0.453_592_37;
const G0: f64 = 9.806_65;
const LY: f64 = 9_460_730_472_580_800.0;
const PICA_PT: f64 = IN / 72.0;
const LBF: f64 = LB * G0;
const HP: f64 = 550.0 * FT * LBF;
const GAL: f64 = 231.0 * IN * IN * IN;
const UK_GAL: f64 = 4.546_09e-3;

const UNITS: &[Unit] = &[
    // Mass (kg)
    u(&["g"], Kind::Mass, 1e-3, true),
    u(&["sg"], Kind::Mass, LBF / FT, false),
    u(&["lbm"], Kind::Mass, LB, false),
    u(&["u"], Kind::Mass, 1.660_539_066_6e-27, true),
    u(&["ozm"], Kind::Mass, LB / 16.0, false),
    u(&["grain"], Kind::Mass, LB / 7000.0, false),
    u(&["cwt", "shweight"], Kind::Mass, 100.0 * LB, false),
    u(
        &["uk_cwt", "lcwt", "hweight"],
        Kind::Mass,
        112.0 * LB,
        false,
    ),
    u(&["stone"], Kind::Mass, 14.0 * LB, false),
    u(&["ton"], Kind::Mass, 2000.0 * LB, false),
    u(&["uk_ton", "LTON", "brton"], Kind::Mass, 2240.0 * LB, false),
    // Distance (m)
    u(&["m"], Kind::Distance, 1.0, true),
    u(&["mi"], Kind::Distance, MI, false),
    u(&["Nmi"], Kind::Distance, NMI, false),
    u(&["in"], Kind::Distance, IN, false),
    u(&["ft"], Kind::Distance, FT, false),
    u(&["yd"], Kind::Distance, 3.0 * FT, false),
    u(&["ang"], Kind::Distance, 1e-10, true),
    u(&["ell"], Kind::Distance, 45.0 * IN, false),
    u(&["ly"], Kind::Distance, LY, true),
    u(
        &["parsec", "pc"],
        Kind::Distance,
        149_597_870_700.0 * 648_000.0 / PI,
        false,
    ),
    u(&["Picapt", "Pica"], Kind::Distance, PICA_PT, false),
    u(&["pica"], Kind::Distance, IN / 6.0, false),
    u(
        &["survey_mi"],
        Kind::Distance,
        5280.0 * 1200.0 / 3937.0,
        false,
    ),
    // Time (s)
    u(&["yr"], Kind::Time, 365.25 * 86_400.0, false),
    u(&["day", "d"], Kind::Time, 86_400.0, false),
    u(&["hr"], Kind::Time, 3600.0, false),
    u(&["mn", "min"], Kind::Time, 60.0, false),
    u(&["sec", "s"], Kind::Time, 1.0, true),
    // Pressure (Pa)
    u(&["Pa", "p"], Kind::Pressure, 1.0, true),
    u(&["atm", "at"], Kind::Pressure, 101_325.0, true),
    u(&["mmHg"], Kind::Pressure, 13.5951 * G0, true),
    u(&["psi"], Kind::Pressure, LBF / (IN * IN), false),
    u(&["Torr"], Kind::Pressure, 101_325.0 / 760.0, false),
    // Force (N)
    u(&["N"], Kind::Force, 1.0, true),
    u(&["dyn", "dy"], Kind::Force, 1e-5, true),
    u(&["lbf"], Kind::Force, LBF, false),
    u(&["pond"], Kind::Force, G0 * 1e-3, true),
    // Energy (J)
    u(&["J"], Kind::Energy, 1.0, true),
    u(&["e"], Kind::Energy, 1e-7, true),
    u(&["c"], Kind::Energy, 4.184, true),
    u(&["cal"], Kind::Energy, 4.1868, true),
    u(&["eV", "ev"], Kind::Energy, 1.602_176_634e-19, true),
    u(&["HPh", "hh"], Kind::Energy, HP * 3600.0, false),
    u(&["Wh", "wh"], Kind::Energy, 3600.0, true),
    u(&["flb"], Kind::Energy, LBF * FT, false),
    u(&["BTU", "btu"], Kind::Energy, 1_055.055_852_62, false),
    // Power (W)
    u(&["HP", "h"], Kind::Power, HP, false),
    u(&["PS"], Kind::Power, 75.0 * G0, false),
    u(&["W", "w"], Kind::Power, 1.0, true),
    // Magnetism (T)
    u(&["T"], Kind::Magnetism, 1.0, true),
    u(&["ga"], Kind::Magnetism, 1e-4, true),
    // Temperature (handled by `to_kelvin` / `from_kelvin`)
    u(&["C", "cel"], Kind::Temperature, 1.0, false),
    u(&["F", "fah"], Kind::Temperature, 1.0, false),
    u(&["K", "kel"], Kind::Temperature, 1.0, true),
    u(&["Rank"], Kind::Temperature, 1.0, false),
    u(&["Reau"], Kind::Temperature, 1.0, false),
    // Volume (m³)
    u(&["tsp"], Kind::Volume, GAL / 768.0, false),
    u(&["tspm"], Kind::Volume, 5e-6, false),
    u(&["tbs"], Kind::Volume, GAL / 256.0, false),
    u(&["oz"], Kind::Volume, GAL / 128.0, false),
    u(&["cup"], Kind::Volume, GAL / 16.0, false),
    u(&["pt", "us_pt"], Kind::Volume, GAL / 8.0, false),
    u(&["uk_pt"], Kind::Volume, UK_GAL / 8.0, false),
    u(&["qt"], Kind::Volume, GAL / 4.0, false),
    u(&["uk_qt"], Kind::Volume, UK_GAL / 4.0, false),
    u(&["gal"], Kind::Volume, GAL, false),
    u(&["uk_gal"], Kind::Volume, UK_GAL, false),
    u(&["l", "L", "lt"], Kind::Volume, 1e-3, true),
    up(&["ang3", "ang^3"], Kind::Volume, 1e-30, 3),
    u(&["barrel"], Kind::Volume, 42.0 * GAL, false),
    u(&["bushel"], Kind::Volume, 2150.42 * IN * IN * IN, false),
    u(&["ft3", "ft^3"], Kind::Volume, FT * FT * FT, false),
    u(&["in3", "in^3"], Kind::Volume, IN * IN * IN, false),
    u(&["ly3", "ly^3"], Kind::Volume, LY * LY * LY, false),
    up(&["m3", "m^3"], Kind::Volume, 1.0, 3),
    u(&["mi3", "mi^3"], Kind::Volume, MI * MI * MI, false),
    u(&["yd3", "yd^3"], Kind::Volume, 27.0 * FT * FT * FT, false),
    u(&["Nmi3", "Nmi^3"], Kind::Volume, NMI * NMI * NMI, false),
    u(
        &["Picapt3", "Picapt^3", "Pica3", "Pica^3"],
        Kind::Volume,
        PICA_PT * PICA_PT * PICA_PT,
        false,
    ),
    u(
        &["GRT", "regton"],
        Kind::Volume,
        100.0 * FT * FT * FT,
        false,
    ),
    u(&["MTON"], Kind::Volume, 40.0 * FT * FT * FT, false),
    // Area (m²)
    u(&["uk_acre"], Kind::Area, 4840.0 * 9.0 * FT * FT, false),
    u(
        &["us_acre"],
        Kind::Area,
        43_560.0 * (1200.0 / 3937.0) * (1200.0 / 3937.0),
        false,
    ),
    up(&["ang2", "ang^2"], Kind::Area, 1e-20, 2),
    u(&["ar"], Kind::Area, 100.0, true),
    u(&["ft2", "ft^2"], Kind::Area, FT * FT, false),
    u(&["ha"], Kind::Area, 10_000.0, false),
    u(&["in2", "in^2"], Kind::Area, IN * IN, false),
    u(&["ly2", "ly^2"], Kind::Area, LY * LY, false),
    up(&["m2", "m^2"], Kind::Area, 1.0, 2),
    u(&["Morgen"], Kind::Area, 2500.0, false),
    u(&["mi2", "mi^2"], Kind::Area, MI * MI, false),
    u(&["Nmi2", "Nmi^2"], Kind::Area, NMI * NMI, false),
    u(
        &["Picapt2", "Picapt^2", "Pica2", "Pica^2"],
        Kind::Area,
        PICA_PT * PICA_PT,
        false,
    ),
    u(&["yd2", "yd^2"], Kind::Area, 9.0 * FT * FT, false),
    // Information (bit)
    u(&["bit"], Kind::Information, 1.0, true),
    u(&["byte"], Kind::Information, 8.0, true),
    // Speed (m/s)
    u(&["admkn"], Kind::Speed, 6080.0 * FT / 3600.0, false),
    u(&["kn"], Kind::Speed, NMI / 3600.0, false),
    u(&["m/h", "m/hr"], Kind::Speed, 1.0 / 3600.0, true),
    u(&["m/s", "m/sec"], Kind::Speed, 1.0, true),
    u(&["mph"], Kind::Speed, MI / 3600.0, false),
];

/// SI decimal prefixes and the binary prefixes (bit and byte only).
const PREFIXES: &[(&str, f64)] = &[
    ("da", 1e1),
    ("Y", 1e24),
    ("Z", 1e21),
    ("E", 1e18),
    ("P", 1e15),
    ("T", 1e12),
    ("G", 1e9),
    ("M", 1e6),
    ("k", 1e3),
    ("h", 1e2),
    ("e", 1e1),
    ("d", 1e-1),
    ("c", 1e-2),
    ("m", 1e-3),
    ("u", 1e-6),
    ("n", 1e-9),
    ("p", 1e-12),
    ("f", 1e-15),
    ("a", 1e-18),
    ("z", 1e-21),
    ("y", 1e-24),
];
const BINARY_PREFIXES: &[(&str, f64)] = &[
    ("ki", 1024.0),
    ("Mi", 1_048_576.0),
    ("Gi", 1_073_741_824.0),
    ("Ti", 1_099_511_627_776.0),
    ("Pi", 1_125_899_906_842_624.0),
    ("Ei", 1_152_921_504_606_846_976.0),
    ("Zi", 1_180_591_620_717_411_303_424.0),
    ("Yi", 1_208_925_819_614_629_174_706_176.0),
];

/// A unit name with an optional prefix: the unit and the prefix's multiplier.
fn find_unit(name: &str) -> Option<(&'static Unit, f64)> {
    let exact = |n: &str| UNITS.iter().find(|u| u.names.contains(&n));
    if let Some(u) = exact(name) {
        return Some((u, 1.0));
    }
    for (p, m) in BINARY_PREFIXES {
        if let Some(rest) = name.strip_prefix(p) {
            if let Some(u) = exact(rest).filter(|u| u.kind == Kind::Information) {
                return Some((u, *m));
            }
        }
    }
    for (p, m) in PREFIXES {
        if let Some(rest) = name.strip_prefix(p) {
            if let Some(u) = exact(rest).filter(|u| u.prefix) {
                return Some((u, m.powi(u.power)));
            }
        }
    }
    None
}

fn to_kelvin(name: &str, v: f64) -> f64 {
    match name {
        "C" | "cel" => v + 273.15,
        "F" | "fah" => (v - 32.0) * 5.0 / 9.0 + 273.15,
        "Rank" => v * 5.0 / 9.0,
        "Reau" => v * 1.25 + 273.15,
        _ => v,
    }
}

fn from_kelvin(name: &str, k: f64) -> f64 {
    match name {
        "C" | "cel" => k - 273.15,
        "F" | "fah" => (k - 273.15) * 9.0 / 5.0 + 32.0,
        "Rank" => k * 9.0 / 5.0,
        "Reau" => (k - 273.15) / 1.25,
        _ => k,
    }
}

fn convert(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = ctx.num(&args[0])?;
    let from = ctx.text(&args[1])?;
    let to = ctx.text(&args[2])?;
    let (Some((a, pa)), Some((b, pb))) = (find_unit(&from), find_unit(&to)) else {
        return Err(ErrorKind::NA);
    };
    if a.kind != b.kind {
        return Err(ErrorKind::NA);
    }
    let r = if a.kind == Kind::Temperature {
        let base = |n: &str, m: f64| {
            if m == 1.0 {
                n.to_owned()
            } else {
                "K".to_owned()
            }
        };
        // Only kelvin takes a prefix: mK, kK …
        let k = to_kelvin(&base(a.names[0], pa), v * pa);
        from_kelvin(&base(b.names[0], pb), k) / pb
    } else {
        v * a.factor * pa / (b.factor * pb)
    };
    num(round_sig15(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_functions_match_their_identities() {
        // erf(x) + erfc(x) = 1 and erf(−x) = −erf(x), across the series/fraction switch.
        for x in [0.1, 0.9, 2.49, 2.5, 3.7] {
            assert!((erf(x) + erfc(x) - 1.0).abs() < 1e-15);
            assert!((erf(-x) + erf(x)).abs() < 1e-15);
        }
        // Wronskians: J₁Y₀ − J₀Y₁ = 2/(πx) and I₀K₁ + I₁K₀ = 1/x.
        for x in [0.3, 1.0, 4.5, 12.0, 30.0] {
            let w = bessel_j(1, x) * bessel_y(0, x) - bessel_j(0, x) * bessel_y(1, x);
            assert!((w - 2.0 / (PI * x)).abs() < 1e-12 / x, "x={x} w={w}");
            let w = bessel_i(0, x) * bessel_k(1, x) + bessel_i(1, x) * bessel_k(0, x);
            assert!((w * x - 1.0).abs() < 1e-12, "x={x} {w}");
        }
    }
}

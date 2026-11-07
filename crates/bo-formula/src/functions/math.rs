//! Math & trigonometry functions (ECMA-376 Part 1, §18.17.7).

use std::f64::consts::PI;

use super::{f, flatten, num, numbers, opt_num, s, shape, FnSpec, VAR};
use crate::criteria::Criterion;
use crate::eval::{Ctx, R};
use crate::value::{round_sig15, Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    f("SUM", 1, VAR, sum),
    f("PRODUCT", 1, VAR, product),
    f("SUMSQ", 1, VAR, sumsq),
    f("SUMPRODUCT", 1, VAR, sumproduct),
    f("SUMIF", 2, 3, sumif),
    f("SUMIFS", 3, VAR, sumifs),
    s("ROUND", 2, 2, round),
    s("ROUNDUP", 2, 2, roundup),
    s("ROUNDDOWN", 2, 2, rounddown),
    s("MROUND", 2, 2, mround),
    s("INT", 1, 1, int),
    s("TRUNC", 1, 2, trunc),
    s("MOD", 2, 2, modulo),
    s("ABS", 1, 1, abs),
    s("POWER", 2, 2, power),
    s("SQRT", 1, 1, sqrt),
    s("SQRTPI", 1, 1, sqrtpi),
    s("EXP", 1, 1, exp),
    s("LN", 1, 1, ln),
    s("LOG", 1, 2, log),
    s("LOG10", 1, 1, log10),
    s("CEILING", 2, 2, ceiling),
    s("CEILING.MATH", 1, 3, ceiling_math),
    s("CEILING.PRECISE", 1, 2, ceiling_precise),
    s("ISO.CEILING", 1, 2, ceiling_precise),
    s("FLOOR", 2, 2, floor),
    s("FLOOR.MATH", 1, 3, floor_math),
    s("FLOOR.PRECISE", 1, 2, floor_precise),
    f("RAND", 0, 0, rand).volatile(),
    s("RANDBETWEEN", 2, 2, randbetween).volatile(),
    s("RANDARRAY", 0, 5, randarray).volatile(),
    s("SEQUENCE", 1, 4, sequence),
    f("PI", 0, 0, pi),
    s("SIN", 1, 1, sin),
    s("COS", 1, 1, cos),
    s("TAN", 1, 1, tan),
    s("ASIN", 1, 1, asin),
    s("ACOS", 1, 1, acos),
    s("ATAN", 1, 1, atan),
    s("ATAN2", 2, 2, atan2),
    s("SINH", 1, 1, sinh),
    s("COSH", 1, 1, cosh),
    s("TANH", 1, 1, tanh),
    s("ASINH", 1, 1, asinh),
    s("ACOSH", 1, 1, acosh),
    s("ATANH", 1, 1, atanh),
    s("COT", 1, 1, cot),
    s("COTH", 1, 1, coth),
    s("CSC", 1, 1, csc),
    s("CSCH", 1, 1, csch),
    s("SEC", 1, 1, sec),
    s("SECH", 1, 1, sech),
    s("ACOT", 1, 1, acot),
    s("ACOTH", 1, 1, acoth),
    s("DEGREES", 1, 1, degrees),
    s("RADIANS", 1, 1, radians),
    s("SIGN", 1, 1, sign),
    s("EVEN", 1, 1, even),
    s("ODD", 1, 1, odd),
    s("FACT", 1, 1, fact),
    s("FACTDOUBLE", 1, 1, factdouble),
    f("GCD", 1, VAR, gcd),
    f("LCM", 1, VAR, lcm),
    s("QUOTIENT", 2, 2, quotient),
    s("COMBIN", 2, 2, combin),
    s("COMBINA", 2, 2, combina),
    s("PERMUT", 2, 2, permut),
    s("PERMUTATIONA", 2, 2, permutationa),
    f("MULTINOMIAL", 1, VAR, multinomial),
    f("SUMX2MY2", 2, 2, sumx2my2),
    f("SUMX2PY2", 2, 2, sumx2py2),
    f("SUMXMY2", 2, 2, sumxmy2),
];

fn n1(ctx: &Ctx, args: &[Value]) -> Result<f64, ErrorKind> {
    ctx.num(&args[0])
}

fn n2(ctx: &Ctx, args: &[Value]) -> Result<(f64, f64), ErrorKind> {
    Ok((ctx.num(&args[0])?, ctx.num(&args[1])?))
}

fn sum(ctx: &mut Ctx, args: &[Value]) -> R {
    num(numbers(ctx, args)?.iter().sum())
}

fn product(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = numbers(ctx, args)?;
    if v.is_empty() {
        return Ok(Value::Number(0.0));
    }
    num(v.iter().product())
}

fn sumsq(ctx: &mut Ctx, args: &[Value]) -> R {
    num(numbers(ctx, args)?.iter().map(|x| x * x).sum())
}

fn sumproduct(ctx: &mut Ctx, args: &[Value]) -> R {
    let first_shape = shape(ctx, &args[0]);
    let mut acc: Option<Vec<f64>> = None;
    for a in args {
        if let Value::Error(e) = a {
            return Err(*e);
        }
        if shape(ctx, a) != first_shape {
            return Err(ErrorKind::Value);
        }
        let vals: Vec<f64> = flatten(ctx, a)
            .into_iter()
            .map(|v| match v {
                Value::Number(n) => Ok(n),
                Value::Error(e) => Err(e),
                _ => Ok(0.0),
            })
            .collect::<Result<_, _>>()?;
        acc = Some(match acc {
            None => vals,
            Some(prev) => prev.iter().zip(&vals).map(|(a, b)| a * b).collect(),
        });
    }
    num(acc.unwrap_or_default().iter().sum())
}

/// Pairs (criteria-range value, sum-range value) where the sum range is resized
/// to the shape of the criteria range from its top-left cell.
pub(crate) fn resized_range(
    ctx: &Ctx,
    base: &Value,
    other: &Value,
) -> Result<Vec<Value>, ErrorKind> {
    let (rows, cols) = shape(ctx, base);
    match other {
        Value::Ref(a) => {
            let r2 = a.r1 + rows as u32 - 1;
            let c2 = a.c1 + cols as u32 - 1;
            let area = crate::value::Area::new(a.sheet, a.r1, a.c1, r2, c2);
            Ok(flatten(ctx, &Value::Ref(area)))
        }
        v => {
            if shape(ctx, v) != (rows, cols) {
                return Err(ErrorKind::Value);
            }
            Ok(flatten(ctx, v))
        }
    }
}

/// Indices of cells satisfying all (range, criterion) pairs; ranges must share a shape.
pub(crate) fn ifs_mask(
    ctx: &Ctx,
    pairs: &[Value],
) -> Result<(Vec<bool>, (usize, usize)), ErrorKind> {
    if !pairs.len().is_multiple_of(2) || pairs.is_empty() {
        return Err(ErrorKind::Value);
    }
    let shp = shape(ctx, &pairs[0]);
    let mut mask: Option<Vec<bool>> = None;
    for pair in pairs.chunks(2) {
        if matches!(pair[0], Value::Error(_)) {
            return Err(pair[0].error().unwrap_or(ErrorKind::Value));
        }
        if shape(ctx, &pair[0]) != shp {
            return Err(ErrorKind::Value);
        }
        let crit = Criterion::parse(&ctx.scalar(&pair[1]), ctx.date_system);
        let vals = flatten(ctx, &pair[0]);
        let m: Vec<bool> = vals.iter().map(|v| crit.matches(v)).collect();
        mask = Some(match mask {
            None => m,
            Some(prev) => prev.iter().zip(&m).map(|(a, b)| *a && *b).collect(),
        });
    }
    Ok((mask.unwrap_or_default(), shp))
}

fn sum_masked(values: &[Value], mask: &[bool]) -> R {
    let mut total = 0.0;
    for (v, &m) in values.iter().zip(mask) {
        if m {
            match v {
                Value::Number(n) => total += n,
                Value::Error(e) => return Err(*e),
                _ => {}
            }
        }
    }
    num(total)
}

fn sumif(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mask, _) = ifs_mask(ctx, &args[..2])?;
    let target = match args.get(2) {
        Some(v) if !matches!(v, Value::Empty) => resized_range(ctx, &args[0], v)?,
        _ => flatten(ctx, &args[0]),
    };
    sum_masked(&target, &mask)
}

fn sumifs(ctx: &mut Ctx, args: &[Value]) -> R {
    let (mask, shp) = ifs_mask(ctx, &args[1..])?;
    if shape(ctx, &args[0]) != shp {
        return Err(ErrorKind::Value);
    }
    sum_masked(&flatten(ctx, &args[0]), &mask)
}

/// Round half away from zero at `digits` decimals, after snapping to 15
/// significant digits so that decimal inputs like 2.675 round as written.
pub(crate) fn round_digits(x: f64, digits: f64, mode: RoundMode) -> f64 {
    let d = digits.trunc().clamp(-308.0, 308.0) as i32;
    let apply = |y: f64| -> f64 {
        let y = round_sig15(y);
        let a = y.abs();
        let r = match mode {
            RoundMode::HalfAway => a.round(),
            RoundMode::Up => a.ceil(),
            RoundMode::Down => a.floor(),
        };
        r.copysign(y)
    };
    if d >= 0 {
        let p = 10f64.powi(d);
        if !(x * p).is_finite() {
            return x;
        }
        apply(x * p) / p
    } else {
        let p = 10f64.powi(-d);
        apply(x / p) * p
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RoundMode {
    HalfAway,
    Up,
    Down,
}

fn round(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, d) = n2(ctx, args)?;
    num(round_digits(x, d, RoundMode::HalfAway))
}

fn roundup(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, d) = n2(ctx, args)?;
    num(round_digits(x, d, RoundMode::Up))
}

fn rounddown(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, d) = n2(ctx, args)?;
    num(round_digits(x, d, RoundMode::Down))
}

fn mround(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, m) = n2(ctx, args)?;
    if m == 0.0 || x == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if x.signum() != m.signum() {
        return Err(ErrorKind::Num);
    }
    num(round_sig15(x / m).round() * m)
}

fn int(ctx: &mut Ctx, args: &[Value]) -> R {
    num(n1(ctx, args)?.floor())
}

fn trunc(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let d = opt_num(ctx, args, 1, 0.0)?;
    num(round_digits(x, d, RoundMode::Down))
}

fn modulo(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, d) = n2(ctx, args)?;
    if d == 0.0 {
        return Err(ErrorKind::Div0);
    }
    let q = round_sig15(n / d).floor();
    num(n - d * q)
}

fn abs(ctx: &mut Ctx, args: &[Value]) -> R {
    num(n1(ctx, args)?.abs())
}

fn power(ctx: &mut Ctx, args: &[Value]) -> R {
    let (a, b) = n2(ctx, args)?;
    if a == 0.0 && b == 0.0 {
        return Err(ErrorKind::Num);
    }
    if a == 0.0 && b < 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(a.powf(b))
}

fn sqrt(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    num(x.sqrt())
}

fn sqrtpi(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    if x < 0.0 {
        return Err(ErrorKind::Num);
    }
    num((x * PI).sqrt())
}

fn exp(ctx: &mut Ctx, args: &[Value]) -> R {
    num(n1(ctx, args)?.exp())
}

fn ln(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(x.ln())
}

fn log(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let b = opt_num(ctx, args, 1, 10.0)?;
    if x <= 0.0 || b <= 0.0 {
        return Err(ErrorKind::Num);
    }
    if b == 1.0 {
        return Err(ErrorKind::Div0);
    }
    let r = if b == 10.0 {
        x.log10()
    } else {
        x.ln() / b.ln()
    };
    num(r)
}

fn log10(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    num(x.log10())
}

/// Multiple of `sig` reached from `x` by rounding the quotient with `op`.
fn to_multiple(x: f64, sig: f64, op: fn(f64) -> f64) -> f64 {
    op(round_sig15(x / sig)) * sig
}

fn ceiling(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, sig) = n2(ctx, args)?;
    if sig == 0.0 || x == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if x > 0.0 && sig < 0.0 {
        return Err(ErrorKind::Num);
    }
    if x < 0.0 && sig < 0.0 {
        // Both negative: round away from zero.
        return num(-to_multiple(-x, -sig, f64::ceil));
    }
    num(to_multiple(x, sig, f64::ceil))
}

fn ceiling_math(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let sig = opt_num(ctx, args, 1, 1.0)?.abs();
    let mode = opt_num(ctx, args, 2, 0.0)?;
    if sig == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if x < 0.0 && mode != 0.0 {
        return num(-to_multiple(-x, sig, f64::ceil));
    }
    num(to_multiple(x, sig, f64::ceil))
}

fn ceiling_precise(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let sig = opt_num(ctx, args, 1, 1.0)?.abs();
    if sig == 0.0 {
        return Ok(Value::Number(0.0));
    }
    num(to_multiple(x, sig, f64::ceil))
}

fn floor(ctx: &mut Ctx, args: &[Value]) -> R {
    let (x, sig) = n2(ctx, args)?;
    if x == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if sig == 0.0 {
        return Err(ErrorKind::Div0);
    }
    if x > 0.0 && sig < 0.0 {
        return Err(ErrorKind::Num);
    }
    if x < 0.0 && sig < 0.0 {
        // Both negative: round toward zero.
        return num(-to_multiple(-x, -sig, f64::floor));
    }
    num(to_multiple(x, sig, f64::floor))
}

fn floor_math(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let sig = opt_num(ctx, args, 1, 1.0)?.abs();
    let mode = opt_num(ctx, args, 2, 0.0)?;
    if sig == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if x < 0.0 && mode != 0.0 {
        return num(-to_multiple(-x, sig, f64::floor));
    }
    num(to_multiple(x, sig, f64::floor))
}

fn floor_precise(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = n1(ctx, args)?;
    let sig = opt_num(ctx, args, 1, 1.0)?.abs();
    if sig == 0.0 {
        return Ok(Value::Number(0.0));
    }
    num(to_multiple(x, sig, f64::floor))
}

fn rand(ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Number(ctx.random()))
}

fn randbetween(ctx: &mut Ctx, args: &[Value]) -> R {
    let (lo, hi) = n2(ctx, args)?;
    let (lo, hi) = (lo.ceil(), hi.floor());
    if lo > hi {
        return Err(ErrorKind::Num);
    }
    num(lo + (ctx.random() * (hi - lo + 1.0)).floor())
}

fn dims(ctx: &Ctx, args: &[Value], i: usize) -> Result<usize, ErrorKind> {
    let n = opt_num(ctx, args, i, 1.0)?.trunc();
    if n < 1.0 {
        return Err(ErrorKind::Calc);
    }
    if n > 1_048_576.0 {
        return Err(ErrorKind::Num);
    }
    Ok(n as usize)
}

fn randarray(ctx: &mut Ctx, args: &[Value]) -> R {
    let rows = dims(ctx, args, 0)?;
    let cols = dims(ctx, args, 1)?;
    let lo = opt_num(ctx, args, 2, 0.0)?;
    let hi = opt_num(ctx, args, 3, 1.0)?;
    let whole = super::opt_bool(ctx, args, 4, false)?;
    if lo > hi || rows * cols > 1 << 24 {
        return Err(ErrorKind::Value);
    }
    let data = (0..rows * cols)
        .map(|_| {
            let r = ctx.random();
            Value::Number(if whole {
                lo.ceil() + (r * (hi.floor() - lo.ceil() + 1.0)).floor()
            } else {
                lo + r * (hi - lo)
            })
        })
        .collect();
    Ok(Array::new(rows, cols, data).into())
}

fn sequence(ctx: &mut Ctx, args: &[Value]) -> R {
    let rows = dims(ctx, args, 0)?;
    let cols = dims(ctx, args, 1)?;
    let start = opt_num(ctx, args, 2, 1.0)?;
    let step = opt_num(ctx, args, 3, 1.0)?;
    if rows * cols > 1 << 24 {
        return Err(ErrorKind::Value);
    }
    let data = (0..rows * cols)
        .map(|i| Value::number(start + step * i as f64))
        .collect();
    Ok(Array::new(rows, cols, data).into())
}

fn pi(_ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Number(PI))
}

macro_rules! unary {
    ($name:ident, $body:expr) => {
        fn $name(ctx: &mut Ctx, args: &[Value]) -> R {
            let x = n1(ctx, args)?;
            let g: fn(f64) -> Result<f64, ErrorKind> = $body;
            num(g(x)?)
        }
    };
}

/// Trigonometric arguments beyond 2^27 are rejected (precision loss).
fn trig_arg(x: f64) -> Result<f64, ErrorKind> {
    if x.abs() >= 134_217_728.0 {
        Err(ErrorKind::Num)
    } else {
        Ok(x)
    }
}

unary!(sin, |x| Ok(trig_arg(x)?.sin()));
unary!(cos, |x| Ok(trig_arg(x)?.cos()));
unary!(tan, |x| Ok(trig_arg(x)?.tan()));
unary!(asin, |x| if x.abs() > 1.0 {
    Err(ErrorKind::Num)
} else {
    Ok(x.asin())
});
unary!(acos, |x| if x.abs() > 1.0 {
    Err(ErrorKind::Num)
} else {
    Ok(x.acos())
});
unary!(atan, |x| Ok(x.atan()));
unary!(sinh, |x| Ok(x.sinh()));
unary!(cosh, |x| Ok(x.cosh()));
unary!(tanh, |x| Ok(x.tanh()));
unary!(asinh, |x| Ok(x.asinh()));
unary!(acosh, |x| if x < 1.0 {
    Err(ErrorKind::Num)
} else {
    Ok(x.acosh())
});
unary!(atanh, |x| if x.abs() >= 1.0 {
    Err(ErrorKind::Num)
} else {
    Ok(x.atanh())
});
unary!(cot, |x| {
    let t = trig_arg(x)?.tan();
    if x == 0.0 {
        Err(ErrorKind::Div0)
    } else {
        Ok(1.0 / t)
    }
});
unary!(coth, |x| if x == 0.0 {
    Err(ErrorKind::Div0)
} else {
    Ok(1.0 / x.tanh())
});
unary!(csc, |x| if x == 0.0 {
    Err(ErrorKind::Div0)
} else {
    Ok(1.0 / trig_arg(x)?.sin())
});
unary!(csch, |x| if x == 0.0 {
    Err(ErrorKind::Div0)
} else {
    Ok(1.0 / x.sinh())
});
unary!(sec, |x| Ok(1.0 / trig_arg(x)?.cos()));
unary!(sech, |x| Ok(1.0 / x.cosh()));
unary!(acot, |x| Ok(PI / 2.0 - x.atan()));
unary!(acoth, |x| if x.abs() <= 1.0 {
    Err(ErrorKind::Num)
} else {
    Ok(0.5 * ((x + 1.0) / (x - 1.0)).ln())
});
unary!(degrees, |x| Ok(x * 180.0 / PI));
unary!(radians, |x| Ok(x * PI / 180.0));
unary!(sign, |x| Ok(if x > 0.0 {
    1.0
} else if x < 0.0 {
    -1.0
} else {
    0.0
}));

fn atan2(ctx: &mut Ctx, args: &[Value]) -> R {
    // ATAN2(x_num, y_num): angle of the point (x, y).
    let (x, y) = n2(ctx, args)?;
    if x == 0.0 && y == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(y.atan2(x))
}

/// Round away from zero to the next integer with the given parity.
fn to_parity(x: f64, odd: bool) -> f64 {
    let a = round_sig15(x.abs()).ceil();
    let is_odd = a % 2.0 == 1.0;
    let r = if odd {
        if is_odd {
            a
        } else {
            a + 1.0
        }
    } else if is_odd {
        a + 1.0
    } else {
        a
    };
    r.copysign(if x == 0.0 { 1.0 } else { x })
}

fn even(ctx: &mut Ctx, args: &[Value]) -> R {
    num(to_parity(n1(ctx, args)?, false))
}

fn odd(ctx: &mut Ctx, args: &[Value]) -> R {
    num(to_parity(n1(ctx, args)?, true))
}

pub(crate) fn factorial(n: f64) -> Result<f64, ErrorKind> {
    if n < 0.0 {
        return Err(ErrorKind::Num);
    }
    let n = n.trunc();
    if n > 170.0 {
        return Err(ErrorKind::Num);
    }
    Ok((1..=n as u32).fold(1.0, |acc, k| acc * f64::from(k)))
}

fn fact(ctx: &mut Ctx, args: &[Value]) -> R {
    num(factorial(n1(ctx, args)?)?)
}

fn factdouble(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = n1(ctx, args)?;
    if n < -1.0 {
        return Err(ErrorKind::Num);
    }
    let n = n.trunc() as i64;
    let mut acc = 1.0;
    let mut k = n;
    while k > 1 {
        acc *= k as f64;
        k -= 2;
    }
    num(acc)
}

fn int_args(ctx: &Ctx, args: &[Value]) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    for a in args {
        for v in flatten(ctx, a) {
            let n = match v {
                Value::Number(n) => n,
                Value::Empty => continue,
                Value::Error(e) => return Err(e),
                other => ctx.num(&other)?,
            };
            if n < 0.0 || n >= 9.007_199_254_740_992e15 {
                return Err(ErrorKind::Num);
            }
            out.push(n.trunc());
        }
    }
    Ok(out)
}

fn gcd2(mut a: f64, mut b: f64) -> f64 {
    while b != 0.0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn gcd(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = int_args(ctx, args)?;
    num(v.into_iter().fold(0.0, gcd2))
}

fn lcm(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = int_args(ctx, args)?;
    let mut acc = 1.0;
    for n in v {
        if n == 0.0 {
            return Ok(Value::Number(0.0));
        }
        acc = acc / gcd2(acc, n) * n;
    }
    num(acc)
}

fn quotient(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, d) = n2(ctx, args)?;
    if d == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num((n / d).trunc())
}

pub(crate) fn combin_f(n: f64, k: f64) -> Result<f64, ErrorKind> {
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 || k > n {
        return Err(ErrorKind::Num);
    }
    let k = k.min(n - k);
    let mut acc = 1.0;
    let mut i = 1.0;
    while i <= k {
        acc = acc * (n - k + i) / i;
        i += 1.0;
    }
    Ok(acc.round())
}

fn combin(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, k) = n2(ctx, args)?;
    num(combin_f(n, k)?)
}

fn combina(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, k) = n2(ctx, args)?;
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 {
        return Err(ErrorKind::Num);
    }
    if n == 0.0 && k == 0.0 {
        return Ok(Value::Number(1.0));
    }
    num(combin_f(n + k - 1.0, k)?)
}

fn permut(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, k) = n2(ctx, args)?;
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 || k > n {
        return Err(ErrorKind::Num);
    }
    let mut acc = 1.0;
    let mut i = 0.0;
    while i < k {
        acc *= n - i;
        i += 1.0;
    }
    num(acc)
}

fn permutationa(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, k) = n2(ctx, args)?;
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 {
        return Err(ErrorKind::Num);
    }
    num(n.powf(k))
}

fn multinomial(ctx: &mut Ctx, args: &[Value]) -> R {
    let v = int_args(ctx, args)?;
    let total: f64 = v.iter().sum();
    let mut acc = factorial(total)?;
    for n in v {
        acc /= factorial(n)?;
    }
    num(acc.round())
}

pub(crate) fn paired_numbers(ctx: &Ctx, args: &[Value]) -> Result<Vec<(f64, f64)>, ErrorKind> {
    if shape(ctx, &args[0]) != shape(ctx, &args[1]) {
        return Err(ErrorKind::NA);
    }
    let a = super::numeric_cells(ctx, &args[0])?;
    let b = super::numeric_cells(ctx, &args[1])?;
    Ok(a.into_iter()
        .zip(b)
        .filter_map(|(x, y)| Some((x?, y?)))
        .collect())
}

fn sumx2my2(ctx: &mut Ctx, args: &[Value]) -> R {
    num(paired_numbers(ctx, args)?
        .iter()
        .map(|(x, y)| x * x - y * y)
        .sum())
}

fn sumx2py2(ctx: &mut Ctx, args: &[Value]) -> R {
    num(paired_numbers(ctx, args)?
        .iter()
        .map(|(x, y)| x * x + y * y)
        .sum())
}

fn sumxmy2(ctx: &mut Ctx, args: &[Value]) -> R {
    num(paired_numbers(ctx, args)?
        .iter()
        .map(|(x, y)| (x - y) * (x - y))
        .sum())
}

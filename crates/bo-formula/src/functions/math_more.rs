//! More math functions (ECMA-376 Part 1, §18.17.7 and [MS-XLSX] future functions): Roman and
//! Arabic numerals, `BASE` / `DECIMAL`, matrix functions, `PERCENTOF` and `SERIESSUM`.

use super::{f, flatten, num, opt_num, s, shape, FnSpec};
use crate::eval::{Ctx, R};
use crate::value::{Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("ROMAN", 1, 2, roman),
    s("ARABIC", 1, 1, arabic),
    s("BASE", 2, 3, base),
    s("DECIMAL", 2, 2, decimal),
    f("MMULT", 2, 2, mmult),
    f("MDETERM", 1, 1, mdeterm),
    f("MINVERSE", 1, 1, minverse),
    s("MUNIT", 1, 1, munit),
    f("PERCENTOF", 2, 2, percentof),
    f("SERIESSUM", 4, 4, seriessum),
];

// ------------------------------------------------------------------ Roman numerals

/// Roman numeral of 0 … 3999. Form 0 is classic (subtractive pairs IV, IX, XL, XC, CD, CM);
/// each further form allows more subtractive pairs: 1 adds L and V before larger numerals
/// (LD, LM, VL, VC), 2 adds X before D and M (XD, XM), 3 adds V before D and M (VD, VM) and
/// 4 adds I before L, C, D and M (IL, IC, ID, IM), the most concise.
fn roman_text(mut n: u32, form: u32) -> String {
    // (value, numeral, the least form using it), largest first.
    const TABLE: [(u32, &str, u32); 25] = [
        (1000, "M", 0),
        (999, "IM", 4),
        (995, "VM", 3),
        (990, "XM", 2),
        (950, "LM", 1),
        (900, "CM", 0),
        (500, "D", 0),
        (499, "ID", 4),
        (495, "VD", 3),
        (490, "XD", 2),
        (450, "LD", 1),
        (400, "CD", 0),
        (100, "C", 0),
        (99, "IC", 4),
        (95, "VC", 1),
        (90, "XC", 0),
        (50, "L", 0),
        (49, "IL", 4),
        (45, "VL", 1),
        (40, "XL", 0),
        (10, "X", 0),
        (9, "IX", 0),
        (5, "V", 0),
        (4, "IV", 0),
        (1, "I", 0),
    ];
    let mut out = String::new();
    for (v, s, min_form) in TABLE {
        if min_form > form {
            continue;
        }
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

fn roman(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?.trunc();
    let form = match args.get(1) {
        None | Some(Value::Empty) => 0.0,
        Some(Value::Bool(b)) => {
            if *b {
                0.0
            } else {
                4.0
            }
        }
        Some(v) => ctx.num(v)?.trunc(),
    };
    if !(0.0..=3999.0).contains(&n) || !(0.0..=4.0).contains(&form) {
        return Err(ErrorKind::Value);
    }
    Ok(Value::Text(roman_text(n as u32, form as u32)))
}

fn arabic(ctx: &mut Ctx, args: &[Value]) -> R {
    let t = ctx.text(&args[0])?;
    let t = t.trim();
    let (neg, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t),
    };
    if body.chars().count() > 255 {
        return Err(ErrorKind::Value);
    }
    let value = |c: char| match c.to_ascii_uppercase() {
        'I' => Some(1i64),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let vals: Vec<i64> = body
        .chars()
        .map(value)
        .collect::<Option<_>>()
        .ok_or(ErrorKind::Value)?;
    let mut total = 0;
    for (i, v) in vals.iter().enumerate() {
        // A numeral before a larger one is subtracted.
        if vals.get(i + 1).is_some_and(|n| n > v) {
            total -= v;
        } else {
            total += v;
        }
    }
    Ok(Value::number(if neg { -total } else { total } as f64))
}

// ------------------------------------------------------------------ BASE / DECIMAL

fn radix_arg(ctx: &Ctx, v: &Value) -> Result<u32, ErrorKind> {
    let r = ctx.num(v)?.trunc();
    if !(2.0..=36.0).contains(&r) {
        return Err(ErrorKind::Num);
    }
    Ok(r as u32)
}

fn base(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?.trunc();
    let radix = radix_arg(ctx, &args[1])?;
    let min_len = opt_num(ctx, args, 2, 0.0)?.trunc();
    if !(0.0..9.007_199_254_740_992e15).contains(&n) || !(0.0..=255.0).contains(&min_len) {
        return Err(ErrorKind::Num);
    }
    let mut x = n as u64;
    let mut digits = Vec::new();
    loop {
        let d = u32::try_from(x % u64::from(radix)).unwrap_or(0);
        digits.push(
            char::from_digit(d, radix)
                .unwrap_or('0')
                .to_ascii_uppercase(),
        );
        x /= u64::from(radix);
        if x == 0 {
            break;
        }
    }
    let s: String = digits.iter().rev().collect();
    Ok(Value::Text(format!(
        "{s:0>width$}",
        width = min_len as usize
    )))
}

fn decimal(ctx: &mut Ctx, args: &[Value]) -> R {
    let t = ctx.text(&args[0])?;
    let radix = radix_arg(ctx, &args[1])?;
    if t.chars().count() > 255 {
        return Err(ErrorKind::Value);
    }
    let mut v: f64 = 0.0;
    for c in t.trim().chars() {
        let d = c.to_digit(radix).ok_or(ErrorKind::Num)?;
        v = v * f64::from(radix) + f64::from(d);
    }
    num(v)
}

// ------------------------------------------------------------------ matrices

/// A numeric matrix; blanks or text are `#VALUE!`.
fn matrix(ctx: &Ctx, v: &Value) -> Result<(usize, usize, Vec<f64>), ErrorKind> {
    let (r, c) = shape(ctx, v);
    let mut out = Vec::with_capacity(r * c);
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => out.push(n),
            Value::Error(e) => return Err(e),
            _ => return Err(ErrorKind::Value),
        }
    }
    Ok((r, c, out))
}

fn array(rows: usize, cols: usize, data: Vec<f64>) -> R {
    if data.iter().any(|x| !x.is_finite()) {
        return Err(ErrorKind::Num);
    }
    Ok(Value::Array(
        Array::new(rows, cols, data.into_iter().map(Value::number).collect()).into(),
    ))
}

fn mmult(ctx: &mut Ctx, args: &[Value]) -> R {
    let (ar, ac, a) = matrix(ctx, &args[0])?;
    let (br, bc, b) = matrix(ctx, &args[1])?;
    if ac != br {
        return Err(ErrorKind::Value);
    }
    let mut out = vec![0.0; ar * bc];
    for i in 0..ar {
        for j in 0..bc {
            out[i * bc + j] = (0..ac).map(|k| a[i * ac + k] * b[k * bc + j]).sum();
        }
    }
    array(ar, bc, out)
}

/// LU decomposition with partial pivoting: the determinant and, when asked, the inverse.
fn lu(n: usize, mut m: Vec<f64>, inverse: bool) -> (f64, Option<Vec<f64>>) {
    let mut inv: Vec<f64> = (0..n * n)
        .map(|i| f64::from(u8::from(i / n == i % n)))
        .collect();
    let mut det = 1.0;
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| m[i * n + col].abs().total_cmp(&m[j * n + col].abs()))
            .unwrap_or(col);
        if m[piv * n + col] == 0.0 {
            return (0.0, None);
        }
        if piv != col {
            for k in 0..n {
                m.swap(piv * n + k, col * n + k);
                inv.swap(piv * n + k, col * n + k);
            }
            det = -det;
        }
        let d = m[col * n + col];
        det *= d;
        for k in 0..n {
            m[col * n + k] /= d;
            inv[col * n + k] /= d;
        }
        for r in 0..n {
            if r != col {
                let f = m[r * n + col];
                if f != 0.0 {
                    for k in 0..n {
                        m[r * n + k] -= f * m[col * n + k];
                        inv[r * n + k] -= f * inv[col * n + k];
                    }
                }
            }
        }
    }
    (det, inverse.then_some(inv))
}

fn square(ctx: &Ctx, v: &Value) -> Result<(usize, Vec<f64>), ErrorKind> {
    let (r, c, m) = matrix(ctx, v)?;
    if r != c {
        return Err(ErrorKind::Value);
    }
    Ok((r, m))
}

fn mdeterm(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, m) = square(ctx, &args[0])?;
    num(lu(n, m, false).0)
}

fn minverse(ctx: &mut Ctx, args: &[Value]) -> R {
    let (n, m) = square(ctx, &args[0])?;
    match lu(n, m, true) {
        (_, Some(inv)) => array(n, n, inv),
        _ => Err(ErrorKind::Num),
    }
}

fn munit(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?.trunc();
    if !(1.0..=1_048_576.0).contains(&n) {
        return Err(ErrorKind::Value);
    }
    let n = n as usize;
    if n * n > 10_000_000 {
        return Err(ErrorKind::Num);
    }
    array(
        n,
        n,
        (0..n * n)
            .map(|i| f64::from(u8::from(i / n == i % n)))
            .collect(),
    )
}

/// Sum of the numbers of a range or array (text and blanks ignored).
fn total(ctx: &Ctx, v: &Value) -> Result<f64, ErrorKind> {
    let mut s = 0.0;
    for x in flatten(ctx, v) {
        match x {
            Value::Number(n) => s += n,
            Value::Error(e) => return Err(e),
            _ => {}
        }
    }
    Ok(s)
}

fn percentof(ctx: &mut Ctx, args: &[Value]) -> R {
    let part = total(ctx, &args[0])?;
    let all = total(ctx, &args[1])?;
    if all == 0.0 {
        return Err(ErrorKind::Div0);
    }
    num(part / all)
}

fn seriessum(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let n = ctx.num(&args[1])?;
    let m = ctx.num(&args[2])?;
    let mut sum = 0.0;
    for (i, c) in flatten(ctx, &args[3]).into_iter().enumerate() {
        let c = match c {
            Value::Number(c) => c,
            Value::Error(e) => return Err(e),
            _ => return Err(ErrorKind::Value),
        };
        sum += c * x.powf(n + i as f64 * m);
    }
    num(sum)
}

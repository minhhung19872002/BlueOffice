//! Date & time functions (ECMA-376 Part 1, §18.17.7; serial dates per §18.17.4).

use std::collections::HashSet;

use super::{f, flatten, m, opt_num, s, FnSpec};
use crate::date::{
    date_serial, days_in_month, is_leap_year, parse_datetime_text, serial_to_hms, serial_to_ymd,
    time_fraction, weekday_sun0, ymd_to_serial, DateSystem,
};
use crate::eval::{Ctx, R};
use crate::value::{ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("DATE", 3, 3, date),
    s("DATEVALUE", 1, 1, datevalue),
    s("TIME", 3, 3, time),
    s("TIMEVALUE", 1, 1, timevalue),
    s("YEAR", 1, 1, year),
    s("MONTH", 1, 1, month),
    s("DAY", 1, 1, day),
    s("HOUR", 1, 1, hour),
    s("MINUTE", 1, 1, minute),
    s("SECOND", 1, 1, second),
    f("TODAY", 0, 0, today).volatile(),
    f("NOW", 0, 0, now).volatile(),
    s("WEEKDAY", 1, 2, weekday),
    s("WEEKNUM", 1, 2, weeknum),
    s("ISOWEEKNUM", 1, 1, isoweeknum),
    s("EDATE", 2, 2, edate),
    s("EOMONTH", 2, 2, eomonth),
    s("DATEDIF", 3, 3, datedif),
    s("DAYS", 2, 2, days),
    s("DAYS360", 2, 3, days360),
    m("NETWORKDAYS", 2, 3, 0b11, networkdays),
    m("NETWORKDAYS.INTL", 2, 4, 0b111, networkdays_intl),
    m("WORKDAY", 2, 3, 0b11, workday),
    m("WORKDAY.INTL", 2, 4, 0b111, workday_intl),
    s("YEARFRAC", 2, 3, yearfrac),
];

/// A date argument as a non-negative serial.
fn serial(ctx: &Ctx, v: &Value) -> Result<f64, ErrorKind> {
    let n = ctx.num(v)?;
    if n < 0.0 || n > ctx.date_system.max_serial() + 1.0 {
        return Err(ErrorKind::Num);
    }
    Ok(n)
}

fn ymd(ctx: &Ctx, v: &Value) -> Result<(i64, u32, u32), ErrorKind> {
    let n = serial(ctx, v)?;
    serial_to_ymd(n, ctx.date_system).ok_or(ErrorKind::Num)
}

fn date(ctx: &mut Ctx, args: &[Value]) -> R {
    let y = ctx.num(&args[0])?.trunc() as i64;
    let mo = ctx.num(&args[1])?.trunc();
    let d = ctx.num(&args[2])?.trunc();
    if mo.abs() > 1e7 || d.abs() > 1e9 {
        return Err(ErrorKind::Num);
    }
    date_serial(y, mo as i64, d as i64, ctx.date_system)
        .map(Value::Number)
        .ok_or(ErrorKind::Num)
}

fn datevalue(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Text(s) => parse_datetime_text(s, ctx.date_system)
            .filter(|n| *n >= 1.0)
            .map(|n| Value::Number(n.floor()))
            .ok_or(ErrorKind::Value),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn time(ctx: &mut Ctx, args: &[Value]) -> R {
    let h = ctx.num(&args[0])?.trunc();
    let mi = ctx.num(&args[1])?.trunc();
    let s = ctx.num(&args[2])?.trunc();
    let t = time_fraction(h, mi, s);
    if t < 0.0 {
        return Err(ErrorKind::Num);
    }
    Ok(Value::Number(t - t.floor()))
}

fn timevalue(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Text(s) => parse_datetime_text(s, ctx.date_system)
            .map(|n| Value::Number(n - n.floor()))
            .ok_or(ErrorKind::Value),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn year(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(ymd(ctx, &args[0])?.0 as f64))
}

fn month(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(f64::from(ymd(ctx, &args[0])?.1)))
}

fn day(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(f64::from(ymd(ctx, &args[0])?.2)))
}

fn hms(ctx: &Ctx, v: &Value) -> Result<(u32, u32, u32), ErrorKind> {
    let n = serial(ctx, v)?;
    Ok(serial_to_hms(n))
}

fn hour(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(f64::from(hms(ctx, &args[0])?.0)))
}

fn minute(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(f64::from(hms(ctx, &args[0])?.1)))
}

fn second(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(f64::from(hms(ctx, &args[0])?.2)))
}

fn today(ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Number(ctx.now().floor()))
}

fn now(ctx: &mut Ctx, _args: &[Value]) -> R {
    Ok(Value::Number(ctx.now()))
}

fn weekday(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = serial(ctx, &args[0])?;
    let kind = opt_num(ctx, args, 1, 1.0)?.trunc() as i64;
    let w = i64::from(weekday_sun0(n, ctx.date_system)); // Sunday = 0
    let r = match kind {
        1 | 17 => w + 1,
        2 | 11 => (w + 6) % 7 + 1,
        3 => (w + 6) % 7,
        12..=16 => {
            // 12: Tuesday = 1 ... 16: Saturday = 1
            let start = kind - 10; // day index (Sun=0) that maps to 1
            (w - start).rem_euclid(7) + 1
        }
        _ => return Err(ErrorKind::Num),
    };
    Ok(Value::Number(r as f64))
}

fn year_start(y: i64, system: DateSystem) -> Result<f64, ErrorKind> {
    ymd_to_serial(y, 1, 1, system).ok_or(ErrorKind::Num)
}

fn iso_week(n: f64, system: DateSystem) -> Result<f64, ErrorKind> {
    let w = i64::from(weekday_sun0(n, system));
    let mon1 = (w + 6) % 7 + 1; // Monday = 1 .. Sunday = 7
    let thursday = n.floor() - mon1 as f64 + 4.0;
    let (y, _, _) = serial_to_ymd(thursday.max(1.0), system).ok_or(ErrorKind::Num)?;
    let jan1 = year_start(y, system).unwrap_or(1.0);
    Ok(((thursday - jan1) / 7.0).floor() + 1.0)
}

fn weeknum(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = serial(ctx, &args[0])?;
    let kind = opt_num(ctx, args, 1, 1.0)?.trunc() as i64;
    let start_dow = match kind {
        1 | 17 => 0,
        2 | 11 => 1,
        12..=16 => kind - 10,
        21 => return Ok(Value::Number(iso_week(n, ctx.date_system)?)),
        _ => return Err(ErrorKind::Num),
    };
    let (y, _, _) = serial_to_ymd(n, ctx.date_system).ok_or(ErrorKind::Num)?;
    let jan1 = year_start(y, ctx.date_system).unwrap_or(0.0);
    let jan1_dow = i64::from(weekday_sun0(jan1, ctx.date_system));
    let offset = (jan1_dow - start_dow).rem_euclid(7) as f64;
    Ok(Value::Number(
        ((n.floor() - jan1 + offset) / 7.0).floor() + 1.0,
    ))
}

fn isoweeknum(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = serial(ctx, &args[0])?;
    Ok(Value::Number(iso_week(n, ctx.date_system)?))
}

fn add_months(ctx: &Ctx, args: &[Value], end_of_month: bool) -> R {
    let (y, mo, d) = ymd(ctx, &args[0])?;
    let months = ctx.num(&args[1])?.trunc();
    if months.abs() > 120_000.0 {
        return Err(ErrorKind::Num);
    }
    let total = y * 12 + i64::from(mo) - 1 + months as i64;
    let ny = total.div_euclid(12);
    let nm = (total.rem_euclid(12) + 1) as u32;
    if !((1900..=9999).contains(&ny) || ctx.date_system == DateSystem::Date1904 && ny >= 1904) {
        return Err(ErrorKind::Num);
    }
    let dim = days_in_month(ny, nm);
    let nd = if end_of_month { dim } else { d.clamp(1, dim) };
    ymd_to_serial(ny, nm, nd, ctx.date_system)
        .map(Value::Number)
        .ok_or(ErrorKind::Num)
}

fn edate(ctx: &mut Ctx, args: &[Value]) -> R {
    add_months(ctx, args, false)
}

fn eomonth(ctx: &mut Ctx, args: &[Value]) -> R {
    add_months(ctx, args, true)
}

fn datedif(ctx: &mut Ctx, args: &[Value]) -> R {
    let s1 = serial(ctx, &args[0])?.floor();
    let s2 = serial(ctx, &args[1])?.floor();
    if s1 > s2 {
        return Err(ErrorKind::Num);
    }
    let unit = ctx.text(&args[2])?.to_ascii_uppercase();
    let (y1, m1, d1) = serial_to_ymd(s1, ctx.date_system).ok_or(ErrorKind::Num)?;
    let (y2, m2, d2) = serial_to_ymd(s2, ctx.date_system).ok_or(ErrorKind::Num)?;
    let mut months = (y2 - y1) * 12 + i64::from(m2) - i64::from(m1);
    if d2 < d1 {
        months -= 1;
    }
    let r = match unit.as_str() {
        "Y" => (months / 12) as f64,
        "M" => months as f64,
        "D" => s2 - s1,
        "YM" => (months % 12) as f64,
        "MD" => {
            if d2 >= d1 {
                f64::from(d2 - d1)
            } else {
                let (py, pm) = if m2 == 1 { (y2 - 1, 12) } else { (y2, m2 - 1) };
                f64::from(days_in_month(py, pm) + d2) - f64::from(d1)
            }
        }
        "YD" => {
            // Days between the dates as if the start were in the end's year (or the year before).
            let mut ys = y2;
            if (m1, d1) > (m2, d2) {
                ys -= 1;
            }
            let start = ymd_to_serial(ys, m1, d1.min(days_in_month(ys, m1)), ctx.date_system)
                .ok_or(ErrorKind::Num)?;
            s2 - start
        }
        _ => return Err(ErrorKind::Num),
    };
    Ok(Value::Number(r))
}

fn days(ctx: &mut Ctx, args: &[Value]) -> R {
    let end = serial(ctx, &args[0])?.floor();
    let start = serial(ctx, &args[1])?.floor();
    Ok(Value::Number(end - start))
}

fn is_last_of_month(y: i64, m: u32, d: u32) -> bool {
    d == days_in_month(y, m)
}

fn days360(ctx: &mut Ctx, args: &[Value]) -> R {
    let (y1, m1, d1) = ymd(ctx, &args[0])?;
    let (y2, m2, d2) = ymd(ctx, &args[1])?;
    let european = super::opt_bool(ctx, args, 2, false)?;
    let (mut d1, mut d2) = (i64::from(d1), i64::from(d2));
    let (y1, m1, y2, mut m2) = (y1, i64::from(m1), y2, i64::from(m2));
    let mut y2 = y2;
    if european {
        d1 = d1.min(30);
        d2 = d2.min(30);
    } else {
        if is_last_of_month(y1, m1 as u32, d1 as u32) {
            d1 = 30;
        }
        if d2 == 31 {
            if d1 < 30 {
                d2 = 1;
                m2 += 1;
                if m2 > 12 {
                    m2 = 1;
                    y2 += 1;
                }
            } else {
                d2 = 30;
            }
        }
    }
    Ok(Value::Number(
        ((y2 - y1) * 360 + (m2 - m1) * 30 + (d2 - d1)) as f64,
    ))
}

/// Weekend mask indexed Sunday = 0.
fn weekend_mask(ctx: &Ctx, v: Option<&Value>) -> Result<[bool; 7], ErrorKind> {
    let v = match v {
        None | Some(Value::Empty) => return Ok(weekend_from_code(1).unwrap_or_default()),
        Some(v) => ctx.scalar(v),
    };
    match v {
        Value::Text(s) => {
            if s.len() != 7 || !s.bytes().all(|b| b == b'0' || b == b'1') || s == "1111111" {
                return Err(ErrorKind::Value);
            }
            // String is Monday..Sunday.
            let mut mask = [false; 7];
            for (i, b) in s.bytes().enumerate() {
                mask[(i + 1) % 7] = b == b'1';
            }
            Ok(mask)
        }
        Value::Error(e) => Err(e),
        other => {
            let code = other.as_number()?.trunc() as i64;
            weekend_from_code(code).ok_or(ErrorKind::Num)
        }
    }
}

fn weekend_from_code(code: i64) -> Option<[bool; 7]> {
    let mut mask = [false; 7];
    match code {
        1..=7 => {
            // 1: Sat+Sun, 2: Sun+Mon, ..., 7: Fri+Sat
            let first = (code + 5) % 7; // Sun=0 index of first weekend day
            mask[first as usize] = true;
            mask[((first + 1) % 7) as usize] = true;
        }
        // 11: Sunday only, 12: Monday only, ..., 17: Saturday only.
        11..=17 => mask[(code - 11) as usize] = true,
        _ => return None,
    }
    Some(mask)
}

fn holidays(ctx: &Ctx, v: Option<&Value>) -> Result<HashSet<i64>, ErrorKind> {
    let mut out = HashSet::new();
    if let Some(v) = v {
        for x in flatten(ctx, v) {
            match x {
                Value::Number(n) => {
                    out.insert(n.floor() as i64);
                }
                Value::Text(s) => {
                    let n = parse_datetime_text(&s, ctx.date_system).ok_or(ErrorKind::Value)?;
                    out.insert(n.floor() as i64);
                }
                Value::Error(e) => return Err(e),
                _ => {}
            }
        }
    }
    Ok(out)
}

fn is_workday(day: i64, mask: &[bool; 7], hol: &HashSet<i64>, system: DateSystem) -> bool {
    !mask[weekday_sun0(day as f64, system) as usize] && !hol.contains(&day)
}

fn count_workdays(
    ctx: &Ctx,
    start: &Value,
    end: &Value,
    mask: &[bool; 7],
    hol: &HashSet<i64>,
) -> R {
    let a = serial(ctx, start)?.floor() as i64;
    let b = serial(ctx, end)?.floor() as i64;
    let (lo, hi, sign) = if a <= b { (a, b, 1.0) } else { (b, a, -1.0) };
    let count = (lo..=hi)
        .filter(|d| is_workday(*d, mask, hol, ctx.date_system))
        .count();
    Ok(Value::Number(sign * count as f64))
}

fn networkdays(ctx: &mut Ctx, args: &[Value]) -> R {
    let mask = weekend_from_code(1).unwrap_or_default();
    let hol = holidays(ctx, args.get(2))?;
    count_workdays(ctx, &args[0], &args[1], &mask, &hol)
}

fn networkdays_intl(ctx: &mut Ctx, args: &[Value]) -> R {
    let mask = weekend_mask(ctx, args.get(2))?;
    let hol = holidays(ctx, args.get(3))?;
    count_workdays(ctx, &args[0], &args[1], &mask, &hol)
}

fn add_workdays(ctx: &Ctx, start: &Value, n: &Value, mask: &[bool; 7], hol: &HashSet<i64>) -> R {
    let mut d = serial(ctx, start)?.floor() as i64;
    let n = ctx.num(n)?.trunc();
    if n.abs() > 1e7 {
        return Err(ErrorKind::Num);
    }
    let step = if n < 0.0 { -1 } else { 1 };
    let mut left = n.abs() as i64;
    let max = ctx.date_system.max_serial() as i64;
    while left > 0 {
        d += step;
        if d < 0 || d > max {
            return Err(ErrorKind::Num);
        }
        if is_workday(d, mask, hol, ctx.date_system) {
            left -= 1;
        }
    }
    Ok(Value::Number(d as f64))
}

fn workday(ctx: &mut Ctx, args: &[Value]) -> R {
    let mask = weekend_from_code(1).unwrap_or_default();
    let hol = holidays(ctx, args.get(2))?;
    add_workdays(ctx, &args[0], &args[1], &mask, &hol)
}

fn workday_intl(ctx: &mut Ctx, args: &[Value]) -> R {
    let mask = weekend_mask(ctx, args.get(2))?;
    let hol = holidays(ctx, args.get(3))?;
    add_workdays(ctx, &args[0], &args[1], &mask, &hol)
}

fn last_of_feb(y: i64, m: u32, d: u32) -> bool {
    m == 2 && d == days_in_month(y, 2)
}

/// Days between two dates counting months of 30 days, as the `basis` of YEARFRAC and the
/// bond functions defines them: `us` is basis 0 (US / NASD: the last day of February counts
/// as the 30th, a 31st becomes the 30th), otherwise basis 4 (European: every 31st is the 30th).
/// DAYS360 has its own US rule and does not use this.
pub(crate) fn days_30_360(a: (i64, u32, u32), b: (i64, u32, u32), us: bool) -> i64 {
    let ((y1, m1, d1), (y2, m2, d2)) = (a, b);
    let (mut dd1, mut dd2) = (i64::from(d1), i64::from(d2));
    if us {
        if last_of_feb(y1, m1, d1) && last_of_feb(y2, m2, d2) {
            dd2 = 30;
        }
        if last_of_feb(y1, m1, d1) {
            dd1 = 30;
        }
        if dd2 == 31 && dd1 >= 30 {
            dd2 = 30;
        }
        if dd1 == 31 {
            dd1 = 30;
        }
    } else {
        dd1 = dd1.min(30);
        dd2 = dd2.min(30);
    }
    (y2 - y1) * 360 + (i64::from(m2) - i64::from(m1)) * 30 + (dd2 - dd1)
}

fn yearfrac(ctx: &mut Ctx, args: &[Value]) -> R {
    let a = serial(ctx, &args[0])?.floor();
    let b = serial(ctx, &args[1])?.floor();
    let basis = opt_num(ctx, args, 2, 0.0)?.trunc() as i64;
    if !(0..=4).contains(&basis) {
        return Err(ErrorKind::Num);
    }
    Ok(Value::Number(year_frac(a, b, basis, ctx.date_system)?))
}

/// Fraction of a year between two whole serials under `basis` (0 US 30/360, 1 actual/actual,
/// 2 actual/360, 3 actual/365, 4 European 30/360).
pub(crate) fn year_frac(a: f64, b: f64, basis: i64, sys: DateSystem) -> Result<f64, ErrorKind> {
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    let (y1, m1, d1) = serial_to_ymd(a, sys).ok_or(ErrorKind::Num)?;
    let (y2, m2, d2) = serial_to_ymd(b, sys).ok_or(ErrorKind::Num)?;
    let thirty = |us: bool| days_30_360((y1, m1, d1), (y2, m2, d2), us) as f64 / 360.0;
    let r = match basis {
        // US (NASD) 30/360.
        0 => thirty(true),
        1 => {
            let within_year = y1 == y2 || (y2 == y1 + 1 && (m1, d1) >= (m2, d2));
            if within_year {
                let leap = if y1 == y2 {
                    is_leap_year(y1)
                } else {
                    (is_leap_year(y1) && (m1, d1) <= (2, 29))
                        || (is_leap_year(y2) && (m2, d2) >= (2, 29))
                };
                (b - a) / if leap { 366.0 } else { 365.0 }
            } else {
                let start = ymd_to_serial(y1, 1, 1, sys).ok_or(ErrorKind::Num)?;
                let end = ymd_to_serial(y2 + 1, 1, 1, sys).unwrap_or(sys.max_serial() + 1.0);
                let avg = (end - start) / (y2 - y1 + 1) as f64;
                (b - a) / avg
            }
        }
        2 => (b - a) / 360.0,
        3 => (b - a) / 365.0,
        _ => thirty(false),
    };
    Ok(r)
}

//! Text functions (ECMA-376 Part 1, §18.17.7). Positions count Unicode characters.

use super::{f, flatten, m, opt_bool, opt_num, opt_text, s, FnSpec, VAR};
use crate::criteria::wildcard_match;
use crate::eval::{Ctx, FormatInput, R};
use crate::value::{parse_number_text, text_to_number, Array, ErrorKind, Value};

use crate::numfmt::format_number;

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("LEFT", 1, 2, left),
    s("RIGHT", 1, 2, right),
    s("MID", 3, 3, mid),
    s("LEN", 1, 1, len),
    s("FIND", 2, 3, find),
    s("SEARCH", 2, 3, search),
    s("SUBSTITUTE", 3, 4, substitute),
    s("REPLACE", 4, 4, replace),
    s("UPPER", 1, 1, upper),
    s("LOWER", 1, 1, lower),
    s("PROPER", 1, 1, proper),
    s("TRIM", 1, 1, trim),
    s("CLEAN", 1, 1, clean),
    s("TEXT", 2, 2, text),
    s("VALUE", 1, 1, value),
    f("CONCAT", 1, VAR, concat),
    s("CONCATENATE", 1, VAR, concatenate),
    m("TEXTJOIN", 3, VAR, 0b11, textjoin),
    m("TEXTSPLIT", 2, 6, 0b1, textsplit),
    m("TEXTBEFORE", 2, 6, 0b1, textbefore),
    m("TEXTAFTER", 2, 6, 0b1, textafter),
    s("REPT", 2, 2, rept),
    s("EXACT", 2, 2, exact),
    s("CHAR", 1, 1, char_fn),
    s("CODE", 1, 1, code),
    s("UNICHAR", 1, 1, unichar),
    s("UNICODE", 1, 1, unicode),
    s("NUMBERVALUE", 1, 3, numbervalue),
    s("T", 1, 1, t),
    s("FIXED", 1, 3, fixed),
];

fn txt(ctx: &Ctx, args: &[Value], i: usize) -> Result<String, ErrorKind> {
    ctx.text(&args[i])
}

fn count_arg(ctx: &Ctx, args: &[Value], i: usize, default: f64) -> Result<usize, ErrorKind> {
    let n = opt_num(ctx, args, i, default)?;
    if n < 0.0 {
        return Err(ErrorKind::Value);
    }
    Ok(n.trunc().min(1e9) as usize)
}

fn left(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let n = count_arg(ctx, args, 1, 1.0)?;
    Ok(Value::Text(s.chars().take(n).collect()))
}

fn right(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let n = count_arg(ctx, args, 1, 1.0)?;
    let len = s.chars().count();
    Ok(Value::Text(s.chars().skip(len.saturating_sub(n)).collect()))
}

fn mid(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let start = ctx.num(&args[1])?.trunc();
    let n = ctx.num(&args[2])?.trunc();
    if start < 1.0 || n < 0.0 {
        return Err(ErrorKind::Value);
    }
    Ok(Value::Text(
        s.chars()
            .skip(start as usize - 1)
            .take(n.min(1e9) as usize)
            .collect(),
    ))
}

fn len(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Number(txt(ctx, args, 0)?.chars().count() as f64))
}

/// Character index of `needle` in `hay` starting at char `start` (0-based).
fn char_find(
    hay: &[char],
    needle: &[char],
    start: usize,
    eq: impl Fn(&[char], usize) -> bool,
) -> Option<usize> {
    if needle.is_empty() {
        return (start <= hay.len()).then_some(start);
    }
    (start..hay.len()).find(|&i| eq(hay, i))
}

fn find(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle: Vec<char> = txt(ctx, args, 0)?.chars().collect();
    let hay: Vec<char> = txt(ctx, args, 1)?.chars().collect();
    let start = opt_num(ctx, args, 2, 1.0)?.trunc();
    if start < 1.0 || start as usize > hay.len() + 1 {
        return Err(ErrorKind::Value);
    }
    let pos = char_find(&hay, &needle, start as usize - 1, |h, i| {
        h[i..].starts_with(&needle)
    });
    pos.map(|p| Value::Number((p + 1) as f64))
        .ok_or(ErrorKind::Value)
}

fn search(ctx: &mut Ctx, args: &[Value]) -> R {
    let needle = txt(ctx, args, 0)?;
    let hay: Vec<char> = txt(ctx, args, 1)?.chars().collect();
    let start = opt_num(ctx, args, 2, 1.0)?.trunc();
    if start < 1.0 || start as usize > hay.len() + 1 {
        return Err(ErrorKind::Value);
    }
    let start = start as usize - 1;
    if needle.is_empty() {
        return Ok(Value::Number((start + 1) as f64));
    }
    // Wildcard search: find the first position where the pattern matches a prefix.
    let pattern = format!("{needle}*");
    for i in start..hay.len() {
        let rest: String = hay[i..].iter().collect();
        if wildcard_match(&pattern, &rest) {
            return Ok(Value::Number((i + 1) as f64));
        }
    }
    Err(ErrorKind::Value)
}

fn substitute(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let old = txt(ctx, args, 1)?;
    let new = txt(ctx, args, 2)?;
    if old.is_empty() {
        return Ok(Value::Text(s));
    }
    match args.get(3) {
        None | Some(Value::Empty) => Ok(Value::Text(s.replace(&old, &new))),
        Some(v) => {
            let n = ctx.num(v)?.trunc();
            if n < 1.0 {
                return Err(ErrorKind::Value);
            }
            let n = n as usize;
            match s.match_indices(&old).nth(n - 1) {
                Some((i, _)) => {
                    let mut out = String::with_capacity(s.len());
                    out.push_str(&s[..i]);
                    out.push_str(&new);
                    out.push_str(&s[i + old.len()..]);
                    Ok(Value::Text(out))
                }
                None => Ok(Value::Text(s)),
            }
        }
    }
}

fn replace(ctx: &mut Ctx, args: &[Value]) -> R {
    let s: Vec<char> = txt(ctx, args, 0)?.chars().collect();
    let start = ctx.num(&args[1])?.trunc();
    let n = ctx.num(&args[2])?.trunc();
    let new = txt(ctx, args, 3)?;
    if start < 1.0 || n < 0.0 {
        return Err(ErrorKind::Value);
    }
    let a = (start as usize - 1).min(s.len());
    let b = (a + n.min(1e9) as usize).min(s.len());
    let mut out: String = s[..a].iter().collect();
    out.push_str(&new);
    out.extend(&s[b..]);
    Ok(Value::Text(out))
}

fn upper(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Text(txt(ctx, args, 0)?.to_uppercase()))
}

fn lower(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Text(txt(ctx, args, 0)?.to_lowercase()))
}

fn proper(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let mut out = String::with_capacity(s.len());
    let mut prev_letter = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if prev_letter {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            prev_letter = true;
        } else {
            out.push(c);
            prev_letter = false;
        }
    }
    Ok(Value::Text(out))
}

fn trim(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    Ok(Value::Text(
        s.split(' ')
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn clean(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    Ok(Value::Text(
        s.chars().filter(|c| u32::from(*c) >= 32).collect(),
    ))
}

fn text(ctx: &mut Ctx, args: &[Value]) -> R {
    let fmt = txt(ctx, args, 1)?;
    let n = match &args[0] {
        Value::Error(e) => return Err(*e),
        Value::Number(n) => Some(*n),
        Value::Empty => Some(0.0),
        Value::Text(s) => text_to_number(s, ctx.date_system),
        _ => None,
    };
    let s = match n {
        Some(_) => String::new(),
        None => txt(ctx, args, 0)?,
    };
    // An empty format code yields empty text (§18.17.7 TEXT).
    if fmt.is_empty() {
        return Ok(Value::Text(String::new()));
    }
    if let Some(render) = ctx.text_formatter {
        // Host-provided full number-format engine (§18.8.31 format codes).
        let input = match n {
            Some(n) => FormatInput::Number(n),
            None => FormatInput::Text(&s),
        };
        return render(input, &fmt, ctx.date_system)
            .map(Value::Text)
            .ok_or(ErrorKind::Value);
    }
    match n {
        Some(n) => Ok(Value::Text(format_number(n, &fmt, ctx.date_system))),
        None => Ok(Value::Text(crate::numfmt::format_text(&s, &fmt))),
    }
}

fn value(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Number(n) => Ok(Value::Number(*n)),
        Value::Empty => Ok(Value::Number(0.0)),
        Value::Text(s) => text_to_number(s, ctx.date_system)
            .map(Value::Number)
            .ok_or(ErrorKind::Value),
        Value::Error(e) => Err(*e),
        _ => Err(ErrorKind::Value),
    }
}

fn concat(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut out = String::new();
    for a in args {
        for v in flatten(ctx, a) {
            out.push_str(&v.as_text()?);
        }
    }
    Ok(Value::Text(out))
}

fn concatenate(ctx: &mut Ctx, args: &[Value]) -> R {
    let mut out = String::new();
    for a in args {
        out.push_str(&ctx.text(a)?);
    }
    Ok(Value::Text(out))
}

fn textjoin(ctx: &mut Ctx, args: &[Value]) -> R {
    let delim = txt(ctx, args, 0)?;
    let ignore_empty = ctx.boolean(&args[1])?;
    let mut parts = Vec::new();
    for a in &args[2..] {
        for v in flatten(ctx, a) {
            let s = v.as_text()?;
            if !(ignore_empty && s.is_empty()) {
                parts.push(s);
            }
        }
    }
    Ok(Value::Text(parts.join(&delim)))
}

/// Delimiters from a scalar or array argument.
fn delimiters(ctx: &Ctx, v: Option<&Value>) -> Result<Vec<String>, ErrorKind> {
    match v {
        None | Some(Value::Empty) => Ok(Vec::new()),
        Some(v) => flatten(ctx, v).iter().map(Value::as_text).collect(),
    }
}

/// Split `s` at any of the delimiters (earliest, longest match first).
fn split_any(s: &str, delims: &[String], ci: bool) -> Vec<String> {
    let delims: Vec<&String> = delims.iter().filter(|d| !d.is_empty()).collect();
    if delims.is_empty() {
        return vec![s.to_owned()];
    }
    let hay = if ci { s.to_lowercase() } else { s.to_owned() };
    let mut out = Vec::new();
    let mut last = 0;
    let mut i = 0;
    while i < hay.len() {
        let hit = delims
            .iter()
            .filter_map(|d| {
                let d = if ci { d.to_lowercase() } else { (*d).clone() };
                hay[i..].starts_with(&d).then_some(d.len())
            })
            .max();
        if let Some(l) = hit {
            out.push(s[last..i].to_owned());
            i += l;
            last = i;
        } else {
            i += hay[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out.push(s[last..].to_owned());
    out
}

fn textsplit(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let col_d = delimiters(ctx, args.get(1))?;
    let row_d = delimiters(ctx, args.get(2))?;
    let ignore = opt_bool(ctx, args, 3, false)?;
    let ci = opt_num(ctx, args, 4, 0.0)? == 1.0;
    let pad = match args.get(5) {
        None | Some(Value::Empty) => Value::Error(ErrorKind::NA),
        Some(v) => ctx.scalar(v),
    };
    if col_d.iter().all(String::is_empty) && row_d.iter().all(String::is_empty) {
        return Err(ErrorKind::Value);
    }
    let mut rows: Vec<Vec<Value>> = Vec::new();
    for line in split_any(&s, &row_d, ci) {
        if ignore && line.is_empty() {
            continue;
        }
        let cells: Vec<Value> = split_any(&line, &col_d, ci)
            .into_iter()
            .filter(|c| !(ignore && c.is_empty()))
            .map(Value::Text)
            .collect();
        rows.push(cells);
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return Err(ErrorKind::Calc);
    }
    for r in &mut rows {
        r.resize(width, pad.clone());
    }
    Array::from_rows(rows)
        .map(Value::from)
        .ok_or(ErrorKind::Calc)
}

/// Shared implementation of TEXTBEFORE / TEXTAFTER.
fn text_around(ctx: &Ctx, args: &[Value], before: bool) -> R {
    let s = txt(ctx, args, 0)?;
    let delims = delimiters(ctx, args.get(1))?;
    let instance = opt_num(ctx, args, 2, 1.0)?.trunc() as i64;
    let ci = opt_num(ctx, args, 3, 0.0)? == 1.0;
    let match_end = opt_bool(ctx, args, 4, false)?;
    let not_found = || -> R {
        match args.get(5) {
            None | Some(Value::Empty) => Err(ErrorKind::NA),
            Some(v) => Ok(ctx.scalar(v)),
        }
    };
    let len = s.chars().count();
    if instance == 0 || instance.unsigned_abs() as usize > len.max(1) {
        return Err(ErrorKind::Value);
    }
    let hay = if ci { s.to_lowercase() } else { s.clone() };
    // All matches as (byte start, byte end).
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i <= hay.len() {
        let hit = delims
            .iter()
            .filter_map(|d| {
                let d = if ci { d.to_lowercase() } else { d.clone() };
                hay[i..].starts_with(&d).then_some(d.len())
            })
            .max();
        match hit {
            Some(l) if l > 0 => {
                matches.push((i, i + l));
                i += l;
            }
            Some(_) => {
                matches.push((i, i));
                i += hay[i..].chars().next().map_or(1, char::len_utf8);
            }
            None => i += hay[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
    if match_end {
        if instance > 0 {
            matches.push((hay.len(), hay.len()));
        } else {
            matches.insert(0, (0, 0));
        }
    }
    let idx = if instance > 0 {
        instance as usize - 1
    } else {
        match matches.len().checked_sub(instance.unsigned_abs() as usize) {
            Some(i) => i,
            None => return not_found(),
        }
    };
    let Some(&(a, b)) = matches.get(idx) else {
        return not_found();
    };
    Ok(Value::Text(if before {
        s[..a].to_owned()
    } else {
        s[b..].to_owned()
    }))
}

fn textbefore(ctx: &mut Ctx, args: &[Value]) -> R {
    text_around(ctx, args, true)
}

fn textafter(ctx: &mut Ctx, args: &[Value]) -> R {
    text_around(ctx, args, false)
}

fn rept(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let n = ctx.num(&args[1])?.trunc();
    if n < 0.0 || s.len() as f64 * n > 32_767.0 {
        return Err(ErrorKind::Value);
    }
    Ok(Value::Text(s.repeat(n as usize)))
}

fn exact(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::Bool(txt(ctx, args, 0)? == txt(ctx, args, 1)?))
}

fn char_fn(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?.trunc();
    if !(1.0..=255.0).contains(&n) {
        return Err(ErrorKind::Value);
    }
    // Code points 1-255 interpreted as ISO-8859-1.
    char::from_u32(n as u32)
        .map(|c| Value::Text(c.to_string()))
        .ok_or(ErrorKind::Value)
}

fn code(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let c = s.chars().next().ok_or(ErrorKind::Value)?;
    let u = u32::from(c);
    Ok(Value::Number(if u <= 255 { f64::from(u) } else { 63.0 }))
}

fn unichar(ctx: &mut Ctx, args: &[Value]) -> R {
    let n = ctx.num(&args[0])?.trunc();
    if n < 1.0 || n > 1_114_111.0 {
        return Err(ErrorKind::Value);
    }
    char::from_u32(n as u32)
        .map(|c| Value::Text(c.to_string()))
        .ok_or(ErrorKind::NA)
}

fn unicode(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let c = s.chars().next().ok_or(ErrorKind::Value)?;
    Ok(Value::Number(f64::from(u32::from(c))))
}

fn numbervalue(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = txt(ctx, args, 0)?;
    let dec = opt_text(ctx, args, 1, ".")?;
    let group = opt_text(ctx, args, 2, ",")?;
    let dec = dec.chars().next().ok_or(ErrorKind::Value)?;
    let group = group.chars().next();
    if Some(dec) == group {
        return Err(ErrorKind::Value);
    }
    let mut cleaned = String::new();
    let mut seen_dec = false;
    for c in s.chars() {
        if c.is_whitespace() {
            continue;
        }
        if c == dec {
            if seen_dec {
                return Err(ErrorKind::Value);
            }
            seen_dec = true;
            cleaned.push('.');
        } else if Some(c) == group {
            if seen_dec {
                return Err(ErrorKind::Value);
            }
        } else {
            cleaned.push(c);
        }
    }
    if cleaned.is_empty() {
        return Ok(Value::Number(0.0));
    }
    // Trailing percent signs each divide by 100.
    let pct = cleaned.chars().rev().take_while(|c| *c == '%').count();
    let body = &cleaned[..cleaned.len() - pct];
    let mut n = parse_number_text(body).ok_or(ErrorKind::Value)?;
    for _ in 0..pct {
        n /= 100.0;
    }
    Ok(Value::Number(n))
}

fn t(_ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Text(s) => Ok(Value::Text(s.clone())),
        Value::Error(e) => Err(*e),
        _ => Ok(Value::Text(String::new())),
    }
}

fn fixed(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let d = opt_num(ctx, args, 1, 2.0)?.trunc();
    let no_commas = opt_bool(ctx, args, 2, false)?;
    if d > 127.0 {
        return Err(ErrorKind::Value);
    }
    let rounded = super::math::round_digits(x, d, super::math::RoundMode::HalfAway);
    let decimals = d.max(0.0) as usize;
    let s = format!("{:.*}", decimals, rounded.abs());
    let (int_part, frac) = s.split_once('.').unwrap_or((&s, ""));
    let int_part = if no_commas {
        int_part.to_owned()
    } else {
        let n = int_part.len();
        let mut out = String::new();
        for (i, c) in int_part.chars().enumerate() {
            if i > 0 && (n - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    };
    let mut out = String::new();
    if rounded < 0.0 {
        out.push('-');
    }
    out.push_str(&int_part);
    if decimals > 0 {
        out.push('.');
        out.push_str(frac);
    }
    Ok(Value::Text(out))
}

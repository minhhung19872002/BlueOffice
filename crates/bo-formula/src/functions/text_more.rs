//! More text functions (ECMA-376 Part 1, §18.17.7 and [MS-XLSX] future functions): byte
//! variants, width conversion, `DOLLAR`, `BAHTTEXT`, `PHONETIC`, `ARRAYTOTEXT` /
//! `VALUETOTEXT`, `REGEX*`, and BlueOffice's `BO.DOCSO` (Vietnamese amount in words).

use regex::{Regex, RegexBuilder};

use super::{f, flatten, opt_num, opt_text, s, shape, FnSpec};
use crate::eval::{Ctx, R};
use crate::value::{number_to_text, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("LENB", 1, 1, lenb),
    s("LEFTB", 1, 2, leftb),
    s("RIGHTB", 1, 2, rightb),
    s("MIDB", 3, 3, midb),
    s("FINDB", 2, 3, findb),
    s("SEARCHB", 2, 3, searchb),
    s("REPLACEB", 4, 4, replaceb),
    s("ASC", 1, 1, asc),
    s("DBCS", 1, 1, dbcs),
    s("DOLLAR", 1, 2, dollar),
    s("BAHTTEXT", 1, 1, bahttext),
    f("PHONETIC", 1, 1, phonetic),
    f("ARRAYTOTEXT", 1, 2, arraytotext),
    s("VALUETOTEXT", 1, 2, valuetotext),
    s("REGEXTEST", 2, 3, regextest),
    s("REGEXEXTRACT", 2, 4, regexextract),
    s("REGEXREPLACE", 3, 5, regexreplace),
    s("BO.DOCSO", 1, 2, docso),
];

// ------------------------------------------------------------------ byte variants

/// Bytes of a character in a double-byte character set: East Asian wide and full-width
/// characters count 2, everything else 1.
fn char_bytes(c: char) -> usize {
    let u = u32::from(c);
    let wide = matches!(u,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD);
    if wide {
        2
    } else {
        1
    }
}

/// Character index at byte position `b` (0-based); a position inside a wide character rounds
/// up when `up`, else down.
fn char_at_byte(s: &str, b: usize, up: bool) -> usize {
    let mut pos = 0;
    for (i, c) in s.chars().enumerate() {
        if pos >= b {
            return i;
        }
        let w = char_bytes(c);
        if pos + w > b {
            return if up { i + 1 } else { i };
        }
        pos += w;
    }
    s.chars().count()
}

fn byte_len(s: &str) -> usize {
    s.chars().map(char_bytes).sum()
}

fn byte_count(ctx: &Ctx, args: &[Value], i: usize, default: f64) -> Result<usize, ErrorKind> {
    let n = opt_num(ctx, args, i, default)?;
    if n < 0.0 {
        return Err(ErrorKind::Value);
    }
    Ok(n.trunc().min(1e9) as usize)
}

fn lenb(ctx: &mut Ctx, args: &[Value]) -> R {
    Ok(Value::number(byte_len(&ctx.text(&args[0])?) as f64))
}

fn leftb(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    let n = byte_count(ctx, args, 1, 1.0)?;
    Ok(Value::Text(
        s.chars().take(char_at_byte(&s, n, false)).collect(),
    ))
}

fn rightb(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    let n = byte_count(ctx, args, 1, 1.0)?;
    let total = byte_len(&s);
    let start = char_at_byte(&s, total.saturating_sub(n), true);
    Ok(Value::Text(s.chars().skip(start).collect()))
}

fn midb(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    let start = ctx.num(&args[1])?.trunc();
    let n = byte_count(ctx, args, 2, 0.0)?;
    if start < 1.0 {
        return Err(ErrorKind::Value);
    }
    let a = start as usize - 1;
    let from = char_at_byte(&s, a, true);
    let to = char_at_byte(&s, a + n, false);
    Ok(Value::Text(
        s.chars().skip(from).take(to.saturating_sub(from)).collect(),
    ))
}

/// FINDB / SEARCHB: FIND / SEARCH from the character at the start byte, reported in bytes.
fn find_bytes(ctx: &mut Ctx, args: &[Value], fold: bool) -> R {
    let hay = ctx.text(&args[1])?;
    let start_b = opt_num(ctx, args, 2, 1.0)?.trunc();
    if start_b < 1.0 || start_b as usize > byte_len(&hay) + 1 {
        return Err(ErrorKind::Value);
    }
    let start = char_at_byte(&hay, start_b as usize - 1, true);
    let call = [
        args[0].clone(),
        Value::Text(hay.clone()),
        Value::number((start + 1) as f64),
    ];
    let found = if fold {
        super::text::search(ctx, &call)?
    } else {
        super::text::find(ctx, &call)?
    };
    let Value::Number(pos) = found else {
        return Err(ErrorKind::Value);
    };
    let before: usize = hay.chars().take(pos as usize - 1).map(char_bytes).sum();
    Ok(Value::number((before + 1) as f64))
}

fn findb(ctx: &mut Ctx, args: &[Value]) -> R {
    find_bytes(ctx, args, false)
}

fn searchb(ctx: &mut Ctx, args: &[Value]) -> R {
    find_bytes(ctx, args, true)
}

fn replaceb(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    let start = ctx.num(&args[1])?.trunc();
    let n = byte_count(ctx, args, 2, 0.0)?;
    let new = ctx.text(&args[3])?;
    if start < 1.0 {
        return Err(ErrorKind::Value);
    }
    let a = start as usize - 1;
    let from = char_at_byte(&s, a, false);
    let to = char_at_byte(&s, a + n, true).max(from);
    let mut out: String = s.chars().take(from).collect();
    out.push_str(&new);
    out.extend(s.chars().skip(to));
    Ok(Value::Text(out))
}

// ------------------------------------------------------------------ width conversion

/// Full-width ASCII forms (U+FF01–FF5E) and the ideographic space to their half-width forms.
fn asc(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    Ok(Value::Text(
        s.chars()
            .map(|c| match u32::from(c) {
                0x3000 => ' ',
                u @ 0xFF01..=0xFF5E => char::from_u32(u - 0xFEE0).unwrap_or(c),
                _ => c,
            })
            .collect(),
    ))
}

/// Half-width ASCII to the full-width forms.
fn dbcs(ctx: &mut Ctx, args: &[Value]) -> R {
    let s = ctx.text(&args[0])?;
    Ok(Value::Text(
        s.chars()
            .map(|c| match u32::from(c) {
                0x20 => '\u{3000}',
                u @ 0x21..=0x7E => char::from_u32(u + 0xFEE0).unwrap_or(c),
                _ => c,
            })
            .collect(),
    ))
}

// ------------------------------------------------------------------ DOLLAR, BAHTTEXT

/// Currency text: `$` with thousands separators, negatives in parentheses.
fn dollar(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let d = opt_num(ctx, args, 1, 2.0)?;
    let fixed_args = [Value::number(x.abs()), Value::number(d)];
    let Value::Text(body) = super::text::fixed(ctx, &fixed_args)? else {
        return Err(ErrorKind::Value);
    };
    let rounded = super::math::round_digits(x, d.trunc(), super::math::RoundMode::HalfAway);
    Ok(Value::Text(if rounded < 0.0 {
        format!("(${body})")
    } else {
        format!("${body}")
    }))
}

/// Thai words for 0 … 999 999 (the digit before a ten / unit follows Thai counting rules).
fn thai_under_million(n: u64, out: &mut String) {
    const DIGITS: [&str; 10] = [
        "",
        "หนึ่ง",
        "สอง",
        "สาม",
        "สี่",
        "ห้า",
        "หก",
        "เจ็ด",
        "แปด",
        "เก้า",
    ];
    const PLACES: [&str; 6] = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];
    let digits: Vec<u64> = n.to_string().bytes().map(|b| u64::from(b - b'0')).collect();
    let len = digits.len();
    for (i, d) in digits.iter().enumerate() {
        let place = len - 1 - i;
        let d = *d as usize;
        if d == 0 {
            continue;
        }
        match (place, d) {
            (1, 1) => out.push_str("สิบ"),
            (1, 2) => out.push_str("ยี่สิบ"),
            (0, 1) if len > 1 => out.push_str("เอ็ด"),
            _ => {
                out.push_str(DIGITS[d]);
                out.push_str(PLACES[place]);
            }
        }
    }
}

/// Thai words for a whole number, millions grouped with ล้าน.
fn thai_words(n: u64) -> String {
    if n == 0 {
        return "ศูนย์".to_owned();
    }
    let mut out = String::new();
    let millions = n / 1_000_000;
    if millions > 0 {
        out.push_str(&thai_words(millions));
        out.push_str("ล้าน");
    }
    thai_under_million(n % 1_000_000, &mut out);
    out
}

fn bahttext(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let cents_total = (x.abs() * 100.0).round();
    if cents_total > 9.0e15 {
        return Err(ErrorKind::Num);
    }
    let baht = (cents_total / 100.0).trunc() as u64;
    let satang = (cents_total % 100.0) as u64;
    let mut out = String::new();
    if x < 0.0 && cents_total > 0.0 {
        out.push_str("ลบ");
    }
    if baht > 0 || satang == 0 {
        out.push_str(&thai_words(baht));
        out.push_str("บาท");
    }
    if satang == 0 {
        out.push_str("ถ้วน");
    } else {
        out.push_str(&thai_words(satang));
        out.push_str("สตางค์");
    }
    Ok(Value::Text(out))
}

// ------------------------------------------------------------------ PHONETIC, *TOTEXT

/// The phonetic guide of a cell; cells without one give their text.
fn phonetic(ctx: &mut Ctx, args: &[Value]) -> R {
    match &args[0] {
        Value::Ref(_) | Value::Refs(_) => {}
        Value::Error(e) => return Err(*e),
        _ => return Err(ErrorKind::Value),
    }
    let first = flatten(ctx, &args[0])
        .into_iter()
        .next()
        .unwrap_or(Value::Empty);
    match first {
        Value::Error(e) => Err(e),
        Value::Empty => Ok(Value::Text(String::new())),
        v => Ok(Value::Text(v.as_text()?)),
    }
}

/// A value as text: concise (text as is) or strict (text quoted, as in a formula).
fn value_text(v: &Value, strict: bool) -> String {
    match v {
        Value::Text(t) if strict => format!("\"{}\"", t.replace('"', "\"\"")),
        Value::Text(t) => t.clone(),
        Value::Number(n) => number_to_text(*n),
        Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        Value::Error(e) => e.as_str().to_owned(),
        _ => String::new(),
    }
}

fn format_arg(ctx: &Ctx, args: &[Value], i: usize) -> Result<bool, ErrorKind> {
    match opt_num(ctx, args, i, 0.0)?.trunc() {
        0.0 => Ok(false),
        1.0 => Ok(true),
        _ => Err(ErrorKind::Value),
    }
}

fn arraytotext(ctx: &mut Ctx, args: &[Value]) -> R {
    let strict = format_arg(ctx, args, 1)?;
    let (rows, cols) = shape(ctx, &args[0]);
    let values = flatten(ctx, &args[0]);
    if !strict {
        let parts: Vec<String> = values.iter().map(|v| value_text(v, false)).collect();
        return Ok(Value::Text(parts.join(", ")));
    }
    let mut out = String::from("{");
    for r in 0..rows {
        if r > 0 {
            out.push(';');
        }
        for c in 0..cols {
            if c > 0 {
                out.push(',');
            }
            out.push_str(&value_text(&values[r * cols + c], true));
        }
    }
    out.push('}');
    Ok(Value::Text(out))
}

fn valuetotext(ctx: &mut Ctx, args: &[Value]) -> R {
    let strict = format_arg(ctx, args, 1)?;
    Ok(Value::Text(value_text(&ctx.scalar(&args[0]), strict)))
}

// ------------------------------------------------------------------ REGEX*

/// A pattern compiled with case sensitivity 0 (sensitive, default) or 1 (insensitive).
fn pattern(ctx: &Ctx, args: &[Value], pi: usize, ci: usize) -> Result<Regex, ErrorKind> {
    let p = ctx.text(&args[pi])?;
    let insensitive = match opt_num(ctx, args, ci, 0.0)?.trunc() {
        0.0 => false,
        1.0 => true,
        _ => return Err(ErrorKind::Value),
    };
    RegexBuilder::new(&p)
        .case_insensitive(insensitive)
        .size_limit(1 << 20)
        .build()
        .map_err(|_| ErrorKind::Value)
}

fn regextest(ctx: &mut Ctx, args: &[Value]) -> R {
    let text = ctx.text(&args[0])?;
    let re = pattern(ctx, args, 1, 2)?;
    Ok(Value::Bool(re.is_match(&text)))
}

fn regexextract(ctx: &mut Ctx, args: &[Value]) -> R {
    let text = ctx.text(&args[0])?;
    let mode = opt_num(ctx, args, 2, 0.0)?.trunc();
    let re = pattern(ctx, args, 1, 3)?;
    let column = |v: Vec<Value>| -> R {
        crate::value::Array::column(v)
            .map(|a| Value::Array(a.into()))
            .ok_or(ErrorKind::NA)
    };
    match mode {
        // The first match.
        0.0 => re
            .find(&text)
            .map(|m| Value::Text(m.as_str().to_owned()))
            .ok_or(ErrorKind::NA),
        // Every match, one per row.
        1.0 => column(
            re.find_iter(&text)
                .map(|m| Value::Text(m.as_str().to_owned()))
                .collect(),
        ),
        // The capture groups of the first match, one per column.
        2.0 => {
            let caps = re.captures(&text).ok_or(ErrorKind::NA)?;
            let groups: Vec<Value> = caps
                .iter()
                .skip(1)
                .map(|g| Value::Text(g.map_or(String::new(), |m| m.as_str().to_owned())))
                .collect();
            if groups.is_empty() {
                return Err(ErrorKind::NA);
            }
            crate::value::Array::row(groups)
                .map(|a| Value::Array(a.into()))
                .ok_or(ErrorKind::NA)
        }
        _ => Err(ErrorKind::Value),
    }
}

fn regexreplace(ctx: &mut Ctx, args: &[Value]) -> R {
    let text = ctx.text(&args[0])?;
    let replacement = ctx.text(&args[2])?;
    let occurrence = opt_num(ctx, args, 3, 0.0)?.trunc();
    let re = pattern(ctx, args, 1, 4)?;
    // `$1` style group references; a literal `$` is written `$$`.
    if occurrence == 0.0 {
        return Ok(Value::Text(
            re.replace_all(&text, replacement.as_str()).into_owned(),
        ));
    }
    let matches: Vec<_> = re.captures_iter(&text).collect();
    let n = matches.len() as f64;
    // Positive: the n-th match from the start; negative: from the end.
    let idx = if occurrence > 0.0 {
        occurrence - 1.0
    } else {
        n + occurrence
    };
    if idx < 0.0 || idx >= n {
        return Ok(Value::Text(text));
    }
    let caps = &matches[idx as usize];
    let whole = caps.get(0).ok_or(ErrorKind::Value)?;
    let mut expanded = String::new();
    caps.expand(&replacement, &mut expanded);
    Ok(Value::Text(format!(
        "{}{}{}",
        &text[..whole.start()],
        expanded,
        &text[whole.end()..]
    )))
}

// ------------------------------------------------------------------ BO.DOCSO

const VI_DIGITS: [&str; 10] = [
    "không", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín",
];

/// Vietnamese words for 0 … 999; `full` reads leading zero hundreds ("không trăm") as needed
/// inside a larger number.
fn vi_triple(n: u64, full: bool, out: &mut Vec<&'static str>) {
    let (h, t, u) = (
        (n / 100) as usize,
        ((n / 10) % 10) as usize,
        (n % 10) as usize,
    );
    if h > 0 || full {
        out.push(VI_DIGITS[h]);
        out.push("trăm");
    }
    match t {
        0 => {
            if u > 0 && (h > 0 || full) {
                out.push("linh");
            }
        }
        1 => out.push("mười"),
        _ => {
            out.push(VI_DIGITS[t]);
            out.push("mươi");
        }
    }
    match u {
        0 => {}
        1 if t >= 2 => out.push("mốt"),
        5 if t >= 1 => out.push("lăm"),
        _ => out.push(VI_DIGITS[u]),
    }
}

/// Vietnamese words for a whole number: groups of three digits with nghìn / triệu / tỷ, larger
/// amounts repeating tỷ (nghìn tỷ, triệu tỷ …).
fn vi_words(n: u64) -> Vec<&'static str> {
    if n == 0 {
        return vec!["không"];
    }
    // Split into billions (tỷ) recursively, then millions / thousands / units.
    let billions = n / 1_000_000_000;
    let rest = n % 1_000_000_000;
    let mut out = Vec::new();
    if billions > 0 {
        out.extend(vi_words(billions));
        out.push("tỷ");
    }
    let groups = [
        (rest / 1_000_000, "triệu"),
        ((rest / 1000) % 1000, "nghìn"),
        (rest % 1000, ""),
    ];
    for (g, unit) in groups {
        if g == 0 {
            continue;
        }
        // Inside a larger number, a group below 100 still reads its hundreds ("không trăm").
        let full = !out.is_empty();
        vi_triple(g, full, &mut out);
        if !unit.is_empty() {
            out.push(unit);
        }
    }
    out
}

/// `BO.DOCSO(number, [unit])`: an amount in Vietnamese words, capitalised, followed by the unit
/// (default "đồng"; empty text for none). Decimals are read after "phẩy".
fn docso(ctx: &mut Ctx, args: &[Value]) -> R {
    let x = ctx.num(&args[0])?;
    let unit = opt_text(ctx, args, 1, "đồng")?;
    if x.abs() >= 1e15 {
        return Err(ErrorKind::Num);
    }
    let text = number_to_text(x.abs());
    let (int_part, frac) = text.split_once('.').unwrap_or((&text, ""));
    let int: u64 = int_part.parse().map_err(|_| ErrorKind::Num)?;
    let mut words: Vec<&str> = Vec::new();
    if x < 0.0 {
        words.push("âm");
    }
    words.extend(vi_words(int));
    if !frac.is_empty() {
        words.push("phẩy");
        // Leading zeros are read one by one, the rest as a number.
        let digits = frac.trim_start_matches('0');
        words.extend(std::iter::repeat_n("không", frac.len() - digits.len()));
        if let Ok(f) = digits.parse::<u64>() {
            words.extend(vi_words(f));
        }
    }
    let mut out = words.join(" ");
    if !unit.is_empty() {
        out.push(' ');
        out.push_str(&unit);
    }
    let mut chars = out.chars();
    let capital = match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    };
    Ok(Value::Text(capital))
}

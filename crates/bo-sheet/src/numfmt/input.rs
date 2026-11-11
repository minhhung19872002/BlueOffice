//! Recognition of dates and times typed into a cell.
//!
//! What the user types is stored as a serial date-time (ECMA-376 Part 1,
//! §18.17.4 "Dates and Times") together with a built-in date / time number
//! format (§18.8.30: id 14 short date, 20 `h:mm`, 21 `h:mm:ss`, 18 / 19 with
//! AM/PM, 22 short date + time), like a value read from a file.
//!
//! Accepted forms (separators `/`, `-`, and `.` for locales whose decimal
//! separator is not `.`):
//! * day-month-year in the locale order: `dd/mm/yyyy` (vi-VN), `m/d/yyyy`
//!   (en-US); two-digit years 00–29 → 20xx, 30–99 → 19xx;
//! * ISO 8601 `yyyy-mm-dd` (also `yyyy/mm/dd`), optionally followed by `T` or a
//!   space and a time;
//! * a time `h:mm`, `h:mm:ss` (seconds may have a decimal part), optionally
//!   followed by `AM`/`PM` (or the locale's designators, e.g. `SA`/`CH`);
//! * a date followed by a space and a time;
//! * a date whose month is written as a word (see [`parse_text_date`]):
//!   `15 Mar 2024`, `15-Mar-24`, `March 15, 2024`, `15 tháng 3 năm 2024`,
//!   `15/Thg3/2024`, `15 Tháng Ba 2024` → short date with month name (id 15
//!   `d-mmm-yy`); month and four-digit year only (`Mar 2024`, `tháng 3/2024`)
//!   → the first of that month (id 17 `mmm-yy`); optionally followed by a time
//!   (id 22).
//!
//! Anything else — two-part inputs such as `1/2` (left as text), invalid days
//! or months (`31/02/2024`, `13/1/2024` in en-US), numbers, a day and month
//! without a year (which would depend on today's date) — is not a date.

use super::date::datetime_to_serial;
use super::Locale;

/// A recognised date / time: its serial value and built-in number format id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedDate {
    /// Date serial in the 1900 date system (§18.17.4.1), fractional part = time of day.
    pub serial: f64,
    /// Built-in number format id (§18.8.30).
    pub num_fmt_id: u32,
}

/// `h`, `m`, seconds, and whether an AM/PM designator was given.
struct Time {
    hour: u32,
    minute: u32,
    second: f64,
    ampm: bool,
}

fn number(s: &str, max_len: usize) -> Option<u32> {
    (!s.is_empty() && s.len() <= max_len && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// `(year, month, day)` when valid.
fn ymd(y: i32, m: u32, d: u32) -> Option<(i32, u32, u32)> {
    ((1900..=9999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m))
        .then_some((y, m, d))
}

/// Year from 2 or 4 digits (two-digit years: 00–29 → 2000s, 30–99 → 1900s).
fn year(s: &str) -> Option<i32> {
    let v = number(s, 4)?;
    match s.len() {
        4 => i32::try_from(v).ok(),
        1 | 2 => Some(if v < 30 { 2000 } else { 1900 } + v as i32),
        _ => None,
    }
}

fn parse_date(s: &str, locale: &Locale) -> Option<(i32, u32, u32)> {
    let sep = ['/', '-', '.']
        .into_iter()
        .filter(|c| *c != '.' || locale.decimal != '.')
        .find(|c| s.contains(*c))?;
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    // ISO 8601 (four-digit year first).
    if parts[0].len() == 4 && sep != '.' {
        let y = i32::try_from(number(parts[0], 4)?).ok()?;
        return ymd(y, number(parts[1], 2)?, number(parts[2], 2)?);
    }
    let a = number(parts[0], 2)?;
    let b = number(parts[1], 2)?;
    let y = year(parts[2])?;
    // Day-first unless the locale's short date starts with the month.
    if locale.short_date.starts_with('m') {
        ymd(y, a, b)
    } else {
        ymd(y, b, a)
    }
}

fn parse_time(s: &str, locale: &Locale) -> Option<Time> {
    let lower = s.trim().to_lowercase();
    let mut body = lower.as_str();
    let mut pm = None;
    for (des, is_pm) in [
        ("am", false),
        ("pm", true),
        (&*locale.am.to_lowercase(), false),
        (&*locale.pm.to_lowercase(), true),
    ] {
        if let Some(rest) = body.strip_suffix(des) {
            body = rest.trim_end();
            pm = Some(is_pm);
            break;
        }
    }
    let parts: Vec<&str> = body.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut hour = number(parts[0], 2)?;
    let minute = number(parts[1], 2)?;
    let second = match parts.get(2) {
        Some(p) => {
            let (int, frac) = p.split_once(['.', ',']).unwrap_or((p, ""));
            let whole = number(int, 2)?;
            if !frac.is_empty() && number(frac, 3).is_none() {
                return None;
            }
            format!("{whole}.{}", if frac.is_empty() { "0" } else { frac })
                .parse::<f64>()
                .ok()?
        }
        None => 0.0,
    };
    if minute > 59 || second >= 60.0 {
        return None;
    }
    match pm {
        Some(p) => {
            if hour == 0 || hour > 12 {
                return None;
            }
            hour = hour % 12 + if p { 12 } else { 0 };
        }
        None if hour > 23 => return None,
        None => {}
    }
    Some(Time {
        hour,
        minute,
        second,
        ampm: pm.is_some(),
    })
}

/// Recognise a typed date and/or time (see the module documentation).
pub fn parse_typed_date(input: &str, locale: &Locale, date1904: bool) -> Option<TypedDate> {
    let s = input.trim();
    if s.is_empty() || s.starts_with(['-', '+']) {
        return None;
    }
    parse_numeric_date(s, locale, date1904).or_else(|| parse_text_date(s, locale, date1904))
}

/// [`parse_typed_date`], and also a day and month without a year (`1/2`, `15-3`,
/// `15 tháng 3`, `Mar 15`) taken in `year` — the current year, as a spreadsheet does
/// when such a date is typed — with the built-in format 16 `d-mmm` (§18.8.30).
pub fn parse_typed_date_in_year(
    input: &str,
    locale: &Locale,
    date1904: bool,
    year: i32,
) -> Option<TypedDate> {
    if let Some(t) = parse_typed_date(input, locale, date1904) {
        return Some(t);
    }
    let s = input.trim();
    if s.is_empty() || s.starts_with(['-', '+']) || s.contains(':') {
        return None;
    }
    let numeric = s
        .split_once('/')
        .or_else(|| s.split_once('-'))
        .filter(|(a, b)| !a.contains(['/', '-']) && !b.contains(['/', '-']));
    let (a, b) = if let Some((a, b)) = numeric {
        (number(a, 2)?, number(b, 2)?)
    } else {
        let ws = words(s)?;
        let toks = tokens(&ws, locale)?;
        let ([Tok::Num(d), Tok::Month(m)] | [Tok::Month(m), Tok::Num(d)]) = toks.as_slice() else {
            return None;
        };
        let d = number(d, 2)?;
        // Written in the locale's day / month order, read back below.
        if locale.short_date.starts_with('m') {
            (*m, d)
        } else {
            (d, *m)
        }
    };
    let (m, d) = if locale.short_date.starts_with('m') {
        (a, b)
    } else {
        (b, a)
    };
    let (y, m, d) = ymd(year, m, d)?;
    Some(TypedDate {
        serial: datetime_to_serial(y, m, d, 0, 0, 0.0, date1904)?,
        num_fmt_id: 16,
    })
}

/// Number format a typed number earns: a built-in id (ECMA-376 Part 1, §18.8.30) or a
/// format code (§18.8.31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedFormat {
    /// Built-in number format id.
    Builtin(u32),
    /// Custom format code.
    Code(String),
}

/// A number recognised in what was typed, with the format its writing implies.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedNumber {
    /// The value.
    pub value: f64,
    /// Format given to a General cell (`None`: stays General).
    pub format: Option<TypedFormat>,
}

/// Recognises a number typed the way a spreadsheet user writes it and the number format
/// the writing implies (given to the cell when its format is General):
/// * `10%` → 0.1, `0%` (id 9); `12,5%` → `0.00%` (id 10);
/// * `1.234` (vi-VN; `1,234` en-US) → 1234, `#,##0` (id 3); with decimals `#,##0.00` (id 4);
/// * the locale currency symbol before or after (`15.000 ₫`, `₫15000`, `$12` en-US) →
///   a currency format with the locale tag (`#,##0 [$₫-42A]`);
/// * `1e3` → 1000, `0.00E+00` (id 11);
/// * `0 1/2` → 0.5, `# ?/?` (id 12); `1 3/16` → `# ??/??` (id 13);
/// * `(5)` → −5 (General).
///
/// Thousands separators must group by three digits, otherwise `None`.
pub fn parse_typed_number(input: &str, locale: &Locale) -> Option<TypedNumber> {
    let mut s = input.trim();
    let mut negative = false;
    if let Some(inner) = s.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        negative = true;
        s = inner.trim();
    }
    if let Some(rest) = s.strip_prefix('-') {
        if negative {
            return None;
        }
        negative = true;
        s = rest.trim_start();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim_start();
    }
    let cur = locale.currency;
    let mut currency = false;
    if !cur.is_empty() {
        if let Some(rest) = s.strip_prefix(cur) {
            currency = true;
            s = rest.trim_start();
        } else if let Some(rest) = s.strip_suffix(cur) {
            currency = true;
            s = rest.trim_end();
        }
        if currency {
            // `₫-5` / `-5 ₫`.
            if let Some(rest) = s.strip_prefix('-') {
                if negative {
                    return None;
                }
                negative = true;
                s = rest.trim_start();
            }
        }
    }
    let mut percent = false;
    if let Some(rest) = s.strip_suffix('%') {
        percent = true;
        s = rest.trim_end();
    }
    if s.is_empty() || (percent && currency) {
        return None;
    }
    let sign = if negative { -1.0 } else { 1.0 };
    // Fraction: whole number, a space, numerator / denominator.
    if let Some((whole, frac)) = s.split_once(' ') {
        let (n, d) = frac.trim().split_once('/')?;
        let (w, n, d) = (number(whole, 9)?, number(n, 9)?, number(d, 9)?);
        if d == 0 || percent || currency {
            return None;
        }
        let value = sign * (f64::from(w) + f64::from(n) / f64::from(d));
        let id = if d < 10 { 12 } else { 13 };
        return Some(TypedNumber {
            value,
            format: Some(TypedFormat::Builtin(id)),
        });
    }
    // Mantissa and exponent.
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let (int, dec) = match mantissa.split_once(locale.decimal) {
        Some((i, d)) => (i, Some(d)),
        None => (mantissa, None),
    };
    if dec.is_some_and(|d| !d.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let grouped = int.contains(locale.thousands);
    let digits: String = if grouped {
        let groups: Vec<&str> = int.split(locale.thousands).collect();
        let first = groups.first()?;
        if first.is_empty()
            || first.len() > 3
            || groups[1..].iter().any(|g| g.len() != 3)
            || !groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        groups.concat()
    } else {
        if !int.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        int.to_owned()
    };
    if digits.is_empty() && dec.is_none_or(str::is_empty) {
        return None;
    }
    let exp: i32 = match exponent {
        Some(e) => {
            let body = e.strip_prefix(['+', '-']).unwrap_or(e);
            if body.is_empty() || body.len() > 3 || !body.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            e.parse().ok()?
        }
        None => 0,
    };
    let text = format!(
        "{}.{}e{exp}",
        if digits.is_empty() { "0" } else { &digits },
        dec.filter(|d| !d.is_empty()).unwrap_or("0")
    );
    let mut value: f64 = text.parse().ok()?;
    if percent {
        value /= 100.0;
    }
    if !value.is_finite() {
        return None;
    }
    let decimals = dec.is_some_and(|d| !d.is_empty());
    let format = if currency {
        let body = if decimals { "#,##0.00" } else { "#,##0" };
        let tag = format!("[${cur}-{:X}]", locale.lcid & 0xFFFF);
        Some(TypedFormat::Code(if locale.currency_after {
            format!("{body} {tag}")
        } else {
            format!("{tag}{body}")
        }))
    } else if percent {
        Some(TypedFormat::Builtin(if decimals { 10 } else { 9 }))
    } else if exponent.is_some() {
        Some(TypedFormat::Builtin(11))
    } else if grouped {
        Some(TypedFormat::Builtin(if decimals { 4 } else { 3 }))
    } else {
        None
    };
    Some(TypedNumber {
        value: sign * value,
        format,
    })
}

/// Dates written with digits only, and times.
fn parse_numeric_date(s: &str, locale: &Locale, date1904: bool) -> Option<TypedDate> {
    // Time only.
    if s.contains(':') && !s.contains(['/']) && parse_date_prefix(s, locale).is_none() {
        let t = parse_time(s, locale)?;
        let serial =
            (f64::from(t.hour) * 3600.0 + f64::from(t.minute) * 60.0 + t.second) / 86_400.0;
        let seconds = s.matches(':').count() == 2;
        let id = match (t.ampm, seconds) {
            (true, false) => 18,
            (true, true) => 19,
            (false, false) => 20,
            (false, true) => 21,
        };
        return Some(TypedDate {
            serial,
            num_fmt_id: id,
        });
    }
    let (date_part, time_part) = match s.split_once(['T', ' ']) {
        Some((d, t)) if !t.trim().is_empty() => (d, Some(t.trim())),
        _ => (s, None),
    };
    let (y, m, d) = parse_date(date_part, locale)?;
    let time = match time_part {
        Some(t) => Some(parse_time(t, locale)?),
        None => None,
    };
    let (h, mi, sec) = time
        .as_ref()
        .map_or((0, 0, 0.0), |t| (t.hour, t.minute, t.second));
    let serial = datetime_to_serial(y, m, d, h, mi, sec, date1904)?;
    Some(TypedDate {
        serial,
        num_fmt_id: if time.is_some() { 22 } else { 14 },
    })
}

/// A token of a date with a month name.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok<'a> {
    Num(&'a str),
    Month(u32),
}

/// Lower-cased words and digit runs of `s`; separators are white space and
/// `-` `/` `,` `.`. `None` for any other character.
fn words(s: &str) -> Option<Vec<(bool, String)>> {
    let mut out: Vec<(bool, String)> = Vec::new();
    let mut prev: Option<bool> = None;
    for ch in s.chars() {
        if ch.is_whitespace() || matches!(ch, '-' | '/' | ',' | '.') {
            prev = None;
            continue;
        }
        let digit = ch.is_ascii_digit();
        if !digit && !ch.is_alphabetic() {
            return None;
        }
        match out.last_mut() {
            Some((d, w)) if prev == Some(digit) && *d == digit => w.extend(ch.to_lowercase()),
            _ => out.push((digit, ch.to_lowercase().collect())),
        }
        prev = Some(digit);
    }
    Some(out)
}

/// Month phrases of the locale as lower-case word lists, with the month number:
/// full and abbreviated names (`Tháng Mười Một`, `Thg 11`, `March`, `Mar`).
fn month_phrases(locale: &Locale) -> Vec<(Vec<String>, u32)> {
    let mut out = Vec::new();
    for (i, name) in locale
        .months
        .iter()
        .chain(locale.months_short.iter())
        .enumerate()
    {
        let m = u32::try_from(i % 12).unwrap_or(0) + 1;
        if let Some(w) = words(name) {
            out.push((w.into_iter().map(|(_, w)| w).collect(), m));
        }
    }
    out
}

/// Collapses the month phrase(s) of `words` into [`Tok::Month`]. Besides the
/// locale's names, `tháng` / `thang` / `thg` followed by `1`–`12` is a month
/// for vi-VN, and the word `năm` ("year") before a number is dropped.
fn tokens<'a>(ws: &'a [(bool, String)], locale: &Locale) -> Option<Vec<Tok<'a>>> {
    let vi = locale.lcid & 0xFFFF == 0x42A;
    let phrases = month_phrases(locale);
    let mut out = Vec::new();
    let mut i = 0;
    while i < ws.len() {
        let (digit, w) = &ws[i];
        if *digit {
            out.push(Tok::Num(w));
            i += 1;
            continue;
        }
        if vi && matches!(w.as_str(), "tháng" | "thang" | "thg") {
            if let Some(m) = ws
                .get(i + 1)
                .filter(|(d, _)| *d)
                .and_then(|(_, n)| number(n, 2))
            {
                if (1..=12).contains(&m) {
                    out.push(Tok::Month(m));
                    i += 2;
                    continue;
                }
            }
        }
        // Longest month name starting here.
        let best = phrases
            .iter()
            .filter(|(p, _)| {
                ws.len() >= i + p.len() && p.iter().zip(&ws[i..]).all(|(a, (d, b))| !*d && a == b)
            })
            .max_by_key(|(p, _)| p.len());
        if let Some((p, m)) = best {
            out.push(Tok::Month(*m));
            i += p.len();
            continue;
        }
        if vi && matches!(w.as_str(), "năm" | "nam") && ws.get(i + 1).is_some_and(|(d, _)| *d) {
            i += 1;
            continue;
        }
        return None;
    }
    Some(out)
}

/// A date with the month written as a word, optionally followed by a time
/// (module documentation). The result uses the built-in formats of
/// ECMA-376 Part 1, §18.8.30: 15 `d-mmm-yy`, 17 `mmm-yy`, 22 date + time.
fn parse_text_date(s: &str, locale: &Locale, date1904: bool) -> Option<TypedDate> {
    // A time starts at the white-space separated chunk holding the first `:`.
    let (date_part, time_part) = match s.find(':') {
        Some(colon) => {
            let start = s[..colon].rfind(char::is_whitespace)?;
            (&s[..start], Some(s[start..].trim()))
        }
        None => (s, None),
    };
    let ws = words(date_part)?;
    let toks = tokens(&ws, locale)?;
    let day = |d: &str| number(d, 2);
    let (y, m, d, id) = match toks.as_slice() {
        [Tok::Num(d), Tok::Month(m), Tok::Num(y)] | [Tok::Month(m), Tok::Num(d), Tok::Num(y)] => {
            (year(y)?, *m, day(d)?, 15)
        }
        [Tok::Month(m), Tok::Num(y)] if y.len() == 4 => (year(y)?, *m, 1, 17),
        _ => return None,
    };
    let (y, m, d) = ymd(y, m, d)?;
    let time = match time_part {
        Some(t) => Some(parse_time(t, locale)?),
        None => None,
    };
    let (h, mi, sec) = time
        .as_ref()
        .map_or((0, 0, 0.0), |t| (t.hour, t.minute, t.second));
    let serial = datetime_to_serial(y, m, d, h, mi, sec, date1904)?;
    Some(TypedDate {
        serial,
        num_fmt_id: if time.is_some() { 22 } else { id },
    })
}

/// The date at the start of `s` (before a space / `T`), if any.
fn parse_date_prefix(s: &str, locale: &Locale) -> Option<(i32, u32, u32)> {
    let d = s.split_once(['T', ' ']).map_or(s, |(d, _)| d);
    parse_date(d, locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vi(s: &str) -> Option<(f64, u32)> {
        parse_typed_date(s, &Locale::VI_VN, false).map(|t| (t.serial, t.num_fmt_id))
    }

    fn en(s: &str) -> Option<(f64, u32)> {
        parse_typed_date(s, &Locale::EN_US, false).map(|t| (t.serial, t.num_fmt_id))
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (serial 45351 = 2024-02-29)
    #[test]
    fn locale_order() {
        assert_eq!(vi("29/02/2024"), Some((45351.0, 14)));
        assert_eq!(vi("29-2-2024"), Some((45351.0, 14)));
        assert_eq!(vi("29.02.2024"), Some((45351.0, 14)));
        assert_eq!(en("2/29/2024"), Some((45351.0, 14)));
        // The other order is not a valid date in either locale.
        assert_eq!(vi("2/29/2024"), None);
        assert_eq!(en("29/2/2024"), None);
        // Ambiguous day/month: the locale decides.
        assert_eq!(vi("03/04/2024"), en("4/3/2024"));
        assert_ne!(vi("03/04/2024"), en("03/04/2024"));
        // en-US: `.` is the decimal separator, never a date separator.
        assert_eq!(en("29.02.2024"), None);
    }

    #[test]
    fn iso_and_two_digit_years() {
        assert_eq!(vi("2024-02-29"), Some((45351.0, 14)));
        assert_eq!(en("2024-02-29"), Some((45351.0, 14)));
        assert_eq!(en("2024/2/29"), Some((45351.0, 14)));
        assert_eq!(vi("29/2/24"), Some((45351.0, 14)));
        assert_eq!(vi("1/1/30").map(|x| x.0), Some(10_959.0)); // 1930-01-01
        assert_eq!(vi("2024-02-29T18:00").unwrap(), (45351.75, 22));
        assert_eq!(vi("29/02/2024 18:00").unwrap(), (45351.75, 22));
        assert_eq!(en("2/29/2024 6:00 PM").unwrap(), (45351.75, 22));
    }

    #[test]
    fn times() {
        assert_eq!(vi("18:00"), Some((0.75, 20)));
        assert_eq!(vi("6:30:15").map(|x| x.1), Some(21));
        assert_eq!(en("6:00 PM"), Some((0.75, 18)));
        assert_eq!(en("12:00 AM"), Some((0.0, 18)));
        assert_eq!(vi("6:00 CH"), Some((0.75, 18)));
        assert_eq!(en("6:00:30 pm").map(|x| x.1), Some(19));
        assert_eq!(vi("24:00"), None);
        assert_eq!(vi("10:60"), None);
        assert_eq!(en("13:00 PM"), None);
    }

    #[test]
    fn not_dates() {
        for s in [
            "",
            "1/2",
            "1/2/3/4",
            "31/02/2024",
            "00/01/2024",
            "abc",
            "12",
            "1.234",
            "1,5",
            "-1/2/2024",
            "=1/2/2024",
            "1/1/1899",
            "2024-13-01",
            "1:2:3:4",
            "ngày 1/2/2024",
            "29/02/2023",
        ] {
            assert_eq!(vi(s), None, "{s}");
            assert_eq!(en(s), None, "{s}");
        }
    }

    // source: ECMA-376 Part 1, §18.8.30 (built-in 15 d-mmm-yy, 17 mmm-yy, 22)
    #[test]
    fn month_names() {
        let feb29 = Some((45351.0, 15));
        assert_eq!(en("29 Feb 2024"), feb29);
        assert_eq!(en("29-Feb-24"), feb29);
        assert_eq!(en("29-feb-2024"), feb29);
        assert_eq!(en("February 29, 2024"), feb29);
        assert_eq!(en("Feb. 29 2024"), feb29);
        assert_eq!(vi("29 tháng 2 năm 2024"), feb29);
        assert_eq!(vi("29 Tháng 2 2024"), feb29);
        assert_eq!(vi("29/Thg2/2024"), feb29);
        assert_eq!(vi("29 thg 2 24"), feb29);
        assert_eq!(vi("29 thang 2 nam 2024"), feb29);
        assert_eq!(vi("29 Tháng Hai 2024"), feb29);
        // Multi-word names: the longest match wins (Mười Một, not Mười).
        assert_eq!(
            vi("1 tháng mười một 2024"),
            vi("1/11/2024").map(|x| (x.0, 15))
        );
        // "Tháng Năm" is May; "năm" before a year is the word "year".
        assert_eq!(
            vi("1 tháng năm năm 2024"),
            vi("1/5/2024").map(|x| (x.0, 15))
        );
        // Month and four-digit year → first of the month.
        let feb1 = en("2/1/2024").map(|x| (x.0, 17));
        assert_eq!(en("Feb 2024"), feb1);
        assert_eq!(en("February 2024"), feb1);
        assert_eq!(vi("tháng 2/2024"), feb1);
        assert_eq!(vi("tháng 2 năm 2024"), feb1);
        // With a time.
        assert_eq!(en("29 Feb 2024 6:00 PM"), Some((45351.75, 22)));
        assert_eq!(vi("29 tháng 2 năm 2024 18:00"), Some((45351.75, 22)));
    }

    #[test]
    fn not_month_dates() {
        for s in [
            "Feb 24",            // two-digit year alone would be ambiguous with a day
            "29 Feb",            // no year: would depend on today
            "30 Feb 2024",       // invalid day
            "29 Feb 2023",       // not a leap year
            "29 Feb 2024 25:00", // invalid time
            "Feb",
            "March madness 2024",
            "29 Feb 2024 extra",
            "29 tháng 13 2024",
            "29 Feb 2024!",
        ] {
            assert_eq!(en(s), None, "{s}");
            assert_eq!(vi(s), None, "{s}");
        }
        // Names of the other locale are not months.
        assert_eq!(en("29 tháng 2 năm 2024"), None);
        assert_eq!(vi("29 Feb 2024"), None);
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (serial 45323 = 2024-02-01), §18.8.30 (id 16 d-mmm)
    #[test]
    fn day_and_month_take_the_given_year() {
        let vi = |s: &str| {
            parse_typed_date_in_year(s, &Locale::VI_VN, false, 2024)
                .map(|t| (t.serial, t.num_fmt_id))
        };
        let en = |s: &str| {
            parse_typed_date_in_year(s, &Locale::EN_US, false, 2024)
                .map(|t| (t.serial, t.num_fmt_id))
        };
        // vi-VN reads day first: 1/2 = 1 February; en-US month first: 2/1.
        assert_eq!(vi("1/2"), Some((45323.0, 16)));
        assert_eq!(vi("1-2"), Some((45323.0, 16)));
        assert_eq!(en("2/1"), Some((45323.0, 16)));
        assert_eq!(vi("1 tháng 2"), Some((45323.0, 16)));
        assert_eq!(en("Feb 1"), Some((45323.0, 16)));
        assert_eq!(en("1 Feb"), Some((45323.0, 16)));
        // Full dates keep their own format; invalid days are not dates.
        assert_eq!(vi("29/02/2024"), Some((45351.0, 14)));
        // `1:2` is a time (h:mm), not a day and month.
        assert_eq!(vi("1:2").map(|x| x.1), Some(20));
        for s in ["31/2", "13/13", "1/2/3/4", "-1/2", "1/", "abc", "12"] {
            assert_eq!(vi(s), None, "{s}");
        }
    }

    // source: ECMA-376 Part 1, §18.8.30 (ids 3, 4, 9–13), §18.8.31 (`[$₫-42A]` currency tag)
    #[test]
    fn typed_numbers_and_their_formats() {
        let vi = |s: &str| parse_typed_number(s, &Locale::VI_VN);
        let en = |s: &str| parse_typed_number(s, &Locale::EN_US);
        let b = |v: f64, id: u32| {
            Some(TypedNumber {
                value: v,
                format: Some(TypedFormat::Builtin(id)),
            })
        };
        let code = |v: f64, c: &str| {
            Some(TypedNumber {
                value: v,
                format: Some(TypedFormat::Code(c.into())),
            })
        };
        assert_eq!(vi("10%"), b(0.1, 9));
        assert_eq!(vi("12,5%"), b(0.125, 10));
        assert_eq!(en("12.5%"), b(0.125, 10));
        assert_eq!(vi("1.234"), b(1234.0, 3));
        assert_eq!(vi("1.234.567,5"), b(1_234_567.5, 4));
        assert_eq!(en("1,234"), b(1234.0, 3));
        assert_eq!(vi("15.000 ₫"), code(15000.0, "#,##0 [$₫-42A]"));
        assert_eq!(vi("15000₫"), code(15000.0, "#,##0 [$₫-42A]"));
        assert_eq!(vi("₫12,5"), code(12.5, "#,##0.00 [$₫-42A]"));
        assert_eq!(vi("-15.000 ₫"), code(-15000.0, "#,##0 [$₫-42A]"));
        assert_eq!(en("$12"), code(12.0, "[$$-409]#,##0"));
        assert_eq!(vi("1e3"), b(1000.0, 11));
        assert_eq!(vi("1,5E-2"), b(0.015, 11));
        assert_eq!(vi("0 1/2"), b(0.5, 12));
        assert_eq!(vi("-1 3/16"), b(-1.1875, 13));
        assert_eq!(
            vi("(5)"),
            Some(TypedNumber {
                value: -5.0,
                format: None
            })
        );
        assert_eq!(
            vi("1,5"),
            Some(TypedNumber {
                value: 1.5,
                format: None
            })
        );
        for s in [
            "", "abc", "1.23", "12.34.5", "1 234", "1 1/0", "e5", "%", "₫", "(-5)", "1/2", "10%₫",
            "$12",
        ] {
            assert_eq!(vi(s), None, "{s}");
        }
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (1904 date system base 1904-01-01)
    #[test]
    fn date_1904() {
        let t = parse_typed_date("2/1/1904", &Locale::VI_VN, true).unwrap();
        assert_eq!(t.serial, 1.0);
    }
}

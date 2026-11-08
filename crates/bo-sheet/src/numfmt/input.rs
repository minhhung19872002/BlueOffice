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

    // source: ECMA-376 Part 1, §18.17.4.1 (1904 date system base 1904-01-01)
    #[test]
    fn date_1904() {
        let t = parse_typed_date("2/1/1904", &Locale::VI_VN, true).unwrap();
        assert_eq!(t.serial, 1.0);
    }
}

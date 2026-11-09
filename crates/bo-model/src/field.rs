//! Field results and their formatting (ECMA-376 Part 1, §17.16): the instruction's words and
//! switches (§17.16.1 syntax), the general formatting switches — §17.16.4.1 date-time pictures
//! (`\@`), §17.16.4.2 numeric pictures (`\#`), §17.16.4.3 general formats (`\*`: case, Arabic,
//! letters, Roman numerals, ordinals, numbers in words, hexadecimal) — and §17.16.5.1 `=` formulas.
//! Numbers in words and month / day names are Vietnamese or English.

use crate::write::{format_number, NumFormat};

/// A calendar date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DateTime {
    /// Year (e.g. 2026).
    pub year: i32,
    /// Month 1..=12.
    pub month: u32,
    /// Day 1..=31.
    pub day: u32,
    /// Hour 0..=23.
    pub hour: u32,
    /// Minute 0..=59.
    pub minute: u32,
    /// Second 0..=59.
    pub second: u32,
}

impl DateTime {
    /// Parse `YYYY-MM-DD[THH:MM[:SS]]` (ISO 8601, a trailing `Z` or offset ignored).
    #[must_use]
    pub fn parse_iso(s: &str) -> Option<DateTime> {
        let s = s.trim();
        let (date, time) = s.split_once(['T', ' ']).unwrap_or((s, ""));
        let mut d = date.split('-');
        let year = d.next()?.parse().ok()?;
        let month = d.next()?.parse().ok()?;
        let day = d.next()?.parse().ok()?;
        let time = time.trim_end_matches('Z');
        let time = time.split(['+']).next().unwrap_or("");
        let mut t = time.split(':');
        let mut num = || {
            t.next()
                .and_then(|v| v.split('.').next()?.parse::<u32>().ok())
        };
        let (hour, minute, second) = (num().unwrap_or(0), num().unwrap_or(0), num().unwrap_or(0));
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Day of the week, 0 = Sunday (Gregorian calendar).
    #[must_use]
    pub fn weekday(&self) -> u32 {
        // Sakamoto's method.
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = if self.month < 3 {
            self.year - 1
        } else {
            self.year
        };
        let m = self.month as usize;
        ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) % 12] + self.day as i32).rem_euclid(7)) as u32
    }
}

/// The value of a field before formatting.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// Text.
    Text(String),
    /// A number.
    Number(f64),
    /// A date and time.
    Date(DateTime),
}

/// The words of a field instruction (§17.16.1): the field type, arguments and switches, quoted
/// arguments unquoted (`\"` inside quotes is a quote).
#[must_use]
pub fn words(instr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut it = instr.chars().peekable();
    let mut had = false;
    while let Some(c) = it.next() {
        match c {
            '"' => {
                quoted = !quoted;
                had = true;
            }
            '\\' if quoted && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            c if c.is_whitespace() && !quoted => {
                if had || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                had = false;
            }
            c => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The value of switch `sw` (`\@`, `\#`, `\*`, `\d`, …): the word after it; every value of a
/// switch that repeats (`\* Upper \* MERGEFORMAT`).
#[must_use]
pub fn switch_values(instr: &str, sw: &str) -> Vec<String> {
    let w = words(instr);
    w.iter()
        .enumerate()
        .filter(|(_, x)| x.eq_ignore_ascii_case(sw))
        .filter_map(|(i, _)| w.get(i + 1).cloned())
        .collect()
}

/// Whether switch `sw` (`\h`, `\p`, …) is present.
#[must_use]
pub fn has_switch(instr: &str, sw: &str) -> bool {
    words(instr).iter().any(|x| x.eq_ignore_ascii_case(sw))
}

const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS_EN: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const DAYS_VI: [&str; 7] = [
    "Chủ Nhật",
    "Thứ Hai",
    "Thứ Ba",
    "Thứ Tư",
    "Thứ Năm",
    "Thứ Sáu",
    "Thứ Bảy",
];

/// §17.16.4.1 date-time picture.
#[must_use]
pub fn format_date(d: &DateTime, picture: &str, vi: bool) -> String {
    let c: Vec<char> = picture.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let run = |i: usize, ch: char| c[i..].iter().take_while(|x| **x == ch).count();
    let pm = d.hour >= 12;
    while i < c.len() {
        let ch = c[i];
        match ch {
            '\'' => {
                let end = c[i + 1..]
                    .iter()
                    .position(|x| *x == '\'')
                    .map_or(c.len(), |p| i + 1 + p);
                out.extend(&c[i + 1..end.min(c.len())]);
                i = end + 1;
                continue;
            }
            'M' => {
                let n = run(i, 'M');
                let m = (d.month as usize).clamp(1, 12);
                out.push_str(&match n {
                    1 => d.month.to_string(),
                    2 => format!("{:02}", d.month),
                    3 if vi => format!("Thg {m}"),
                    3 => MONTHS_EN[m - 1][..3].to_owned(),
                    _ if vi => format!("Tháng {m}"),
                    _ => MONTHS_EN[m - 1].to_owned(),
                });
                i += n;
                continue;
            }
            'd' | 'D' => {
                let n = run(i, ch);
                let w = d.weekday() as usize;
                out.push_str(&match n {
                    1 => d.day.to_string(),
                    2 => format!("{:02}", d.day),
                    3 if vi => DAYS_VI[w].replace("Thứ ", "Th ").replace("Chủ Nhật", "CN"),
                    3 => DAYS_EN[w][..3].to_owned(),
                    _ if vi => DAYS_VI[w].to_owned(),
                    _ => DAYS_EN[w].to_owned(),
                });
                i += n;
                continue;
            }
            'y' | 'Y' => {
                let n = run(i, ch);
                out.push_str(&if n <= 2 {
                    format!("{:02}", d.year.rem_euclid(100))
                } else {
                    d.year.to_string()
                });
                i += n;
                continue;
            }
            'h' | 'H' => {
                let n = run(i, ch);
                let h = if ch == 'h' {
                    match d.hour % 12 {
                        0 => 12,
                        x => x,
                    }
                } else {
                    d.hour
                };
                out.push_str(&if n >= 2 {
                    format!("{h:02}")
                } else {
                    h.to_string()
                });
                i += n;
                continue;
            }
            'm' => {
                let n = run(i, 'm');
                out.push_str(&if n >= 2 {
                    format!("{:02}", d.minute)
                } else {
                    d.minute.to_string()
                });
                i += n;
                continue;
            }
            's' | 'S' => {
                let n = run(i, ch);
                out.push_str(&if n >= 2 {
                    format!("{:02}", d.second)
                } else {
                    d.second.to_string()
                });
                i += n;
                continue;
            }
            'A' | 'a' => {
                let rest: String = c[i..].iter().take(5).collect();
                if rest.eq_ignore_ascii_case("am/pm") {
                    let s = if pm { "PM" } else { "AM" };
                    out.push_str(&if ch == 'a' {
                        s.to_lowercase()
                    } else {
                        s.to_owned()
                    });
                    i += 5;
                    continue;
                }
                out.push(ch);
            }
            other => out.push(other),
        }
        i += 1;
    }
    out
}

/// §17.16.4.2 numeric picture: `0` (digit), `#` (digit or space), `x` (drop digits on the left),
/// `.` decimal point, `,` grouping, `-` / `+` signs, `;` positive;negative;zero sections, `'text'`
/// literals, other characters as is.
#[must_use]
pub fn format_picture(v: f64, picture: &str) -> String {
    let sections: Vec<&str> = picture.split(';').collect();
    let (pic, value, neg_in_section) = match (sections.len(), v) {
        (n, x) if n >= 3 && x == 0.0 => (sections[2], 0.0, false),
        (n, x) if n >= 2 && x < 0.0 => (sections[1], -x, true),
        _ => (sections[0], v, false),
    };
    // Split the picture into literal prefix / number part / literal suffix.
    let chars: Vec<char> = pic.chars().collect();
    let is_num = |c: char| matches!(c, '0' | '#' | 'x' | '.' | ',');
    let mut literal_mask = vec![false; chars.len()];
    let mut q = false;
    for (k, c) in chars.iter().enumerate() {
        if *c == '\'' {
            q = !q;
            literal_mask[k] = true;
        } else if q {
            literal_mask[k] = true;
        }
    }
    let first = (0..chars.len()).find(|&k| !literal_mask[k] && is_num(chars[k]));
    let last = (0..chars.len())
        .rev()
        .find(|&k| !literal_mask[k] && is_num(chars[k]));
    let (Some(first), Some(last)) = (first, last) else {
        return pic.replace('\'', "");
    };
    let lit = |r: std::ops::Range<usize>| -> String {
        r.filter(|&k| chars[k] != '\'' || !literal_mask[k])
            .filter(|&k| !(chars[k] == '\'' && literal_mask[k]))
            .map(|k| chars[k])
            .collect::<String>()
    };
    let num: String = chars[first..=last].iter().collect();
    let (int_pic, frac_pic) = num.split_once('.').unwrap_or((num.as_str(), ""));
    let decimals = frac_pic
        .chars()
        .filter(|c| matches!(c, '0' | '#' | 'x'))
        .count();
    let grouping = int_pic.contains(',');
    let neg = value < 0.0;
    let s = format!("{:.*}", decimals, value.abs());
    let (ip, fp) = s.split_once('.').unwrap_or((s.as_str(), ""));
    // Integer digits: at least as many as `0`s; `x` keeps only as many digits as the picture.
    let int_slots: Vec<char> = int_pic
        .chars()
        .filter(|c| matches!(c, '0' | '#' | 'x'))
        .collect();
    let mut digits: Vec<char> = ip.chars().collect();
    if int_slots.contains(&'x') && digits.len() > int_slots.len() {
        digits = digits[digits.len() - int_slots.len()..].to_vec();
    }
    let zeros = int_slots.iter().filter(|c| **c == '0').count();
    while digits.len() < zeros {
        digits.insert(0, '0');
    }
    let hashes = int_slots.len().saturating_sub(digits.len());
    if digits == ['0'] && zeros == 0 && !int_slots.is_empty() {
        digits.clear();
    }
    let mut int_s: String = digits.iter().collect();
    if grouping && int_s.len() > 3 {
        let mut g = String::new();
        for (k, ch) in int_s.chars().enumerate() {
            if k > 0 && (int_s.len() - k).is_multiple_of(3) {
                g.push(',');
            }
            g.push(ch);
        }
        int_s = g;
    }
    // `#` slots with no digit show a space.
    let int_s = " ".repeat(hashes.min(int_slots.iter().filter(|c| **c == '#').count())) + &int_s;
    let int_s = int_s.trim_start().to_owned();
    let mut out = lit(0..first);
    let sign = if (neg || v < 0.0) && !neg_in_section {
        "-"
    } else {
        ""
    };
    // An explicit sign in the picture: `-` shows minus only, `+` shows plus or minus.
    let has_minus = out.contains('-');
    let has_plus = out.contains('+');
    if has_plus {
        out = out.replacen('+', if v < 0.0 { "-" } else { "+" }, 1);
    } else if has_minus {
        out = out.replacen('-', if v < 0.0 { "-" } else { "" }, 1);
    } else {
        out.push_str(sign);
    }
    out.push_str(if int_s.is_empty() && decimals == 0 {
        "0"
    } else {
        &int_s
    });
    if decimals > 0 {
        out.push('.');
        out.push_str(fp);
    }
    out.push_str(&lit(last + 1..chars.len()));
    out
}

const ONES_EN: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS_EN: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn words_en(n: u64) -> String {
    if n < 20 {
        return ONES_EN[n as usize].to_owned();
    }
    if n < 100 {
        let (t, o) = (n / 10, n % 10);
        return if o == 0 {
            TENS_EN[t as usize].to_owned()
        } else {
            format!("{}-{}", TENS_EN[t as usize], ONES_EN[o as usize])
        };
    }
    if n < 1000 {
        let (h, r) = (n / 100, n % 100);
        return if r == 0 {
            format!("{} hundred", ONES_EN[h as usize])
        } else {
            format!("{} hundred {}", ONES_EN[h as usize], words_en(r))
        };
    }
    for (scale, name) in [
        (1_000_000_000_000u64, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ] {
        if n >= scale {
            let (q, r) = (n / scale, n % scale);
            return if r == 0 {
                format!("{} {name}", words_en(q))
            } else {
                format!("{} {name} {}", words_en(q), words_en(r))
            };
        }
    }
    n.to_string()
}

const ONES_VI: [&str; 10] = [
    "không", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín",
];

/// Three digits in Vietnamese (`full`: say "không trăm" / "linh" for the leading zeros).
fn triple_vi(n: u64, full: bool) -> String {
    let (h, t, o) = (n / 100, n / 10 % 10, n % 10);
    let mut w: Vec<String> = Vec::new();
    if h > 0 || full {
        w.push(format!("{} trăm", ONES_VI[h as usize]));
    }
    match t {
        0 if o > 0 && (h > 0 || full) => w.push("linh".into()),
        0 => {}
        1 => w.push("mười".into()),
        _ => w.push(format!("{} mươi", ONES_VI[t as usize])),
    }
    match o {
        0 => {}
        1 if t >= 2 => w.push("mốt".into()),
        4 if t >= 2 => w.push("tư".into()),
        5 if t >= 1 => w.push("lăm".into()),
        _ => w.push(ONES_VI[o as usize].into()),
    }
    w.join(" ")
}

fn words_vi(n: u64) -> String {
    if n == 0 {
        return "không".into();
    }
    let groups = ["", " nghìn", " triệu", " tỷ", " nghìn tỷ", " triệu tỷ"];
    let mut parts: Vec<String> = Vec::new();
    let mut k = 0;
    let mut v = n;
    let mut chunks = Vec::new();
    while v > 0 {
        chunks.push(v % 1000);
        v /= 1000;
    }
    for (i, c) in chunks.iter().enumerate().rev() {
        if *c == 0 {
            continue;
        }
        // Not the leading group: read the hundreds even when zero.
        let full = i + 1 < chunks.len();
        parts.push(format!(
            "{}{}",
            triple_vi(*c, full),
            groups.get(i).unwrap_or(&"")
        ));
        k += 1;
    }
    let _ = k;
    parts.join(" ")
}

/// A number in words (§17.16.4.3 CardText).
#[must_use]
pub fn card_text(n: u64, vi: bool) -> String {
    if vi {
        words_vi(n)
    } else {
        words_en(n)
    }
}

/// An ordinal in words (§17.16.4.3 OrdText).
#[must_use]
pub fn ord_text(n: u64, vi: bool) -> String {
    if vi {
        return match n {
            1 => "thứ nhất".into(),
            4 => "thứ tư".into(),
            _ => format!("thứ {}", words_vi(n)),
        };
    }
    let c = words_en(n);
    // The last word turns ordinal.
    let (head, last) = c
        .rsplit_once([' ', '-'])
        .map_or(("", c.as_str()), |(h, l)| (h, l));
    let sep = if head.is_empty() {
        ""
    } else if c[head.len()..].starts_with('-') {
        "-"
    } else {
        " "
    };
    let ord = match last {
        "one" => "first".to_owned(),
        "two" => "second".to_owned(),
        "three" => "third".to_owned(),
        "five" => "fifth".to_owned(),
        "eight" => "eighth".to_owned(),
        "nine" => "ninth".to_owned(),
        "twelve" => "twelfth".to_owned(),
        l if l.ends_with('y') => format!("{}ieth", &l[..l.len() - 1]),
        l => format!("{l}th"),
    };
    format!("{head}{sep}{ord}")
}

/// §17.16.4.3 general formatting of a value (`format` is the `\*` switch value).
fn general(v: &FieldValue, text: &str, format: &str, vi: bool) -> String {
    let int = match v {
        FieldValue::Number(x) => Some(*x),
        FieldValue::Text(t) => t.trim().parse::<f64>().ok(),
        FieldValue::Date(_) => None,
    };
    let n = int.map(f64::round);
    let f = format;
    match f {
        _ if f.eq_ignore_ascii_case("Upper") => text.to_uppercase(),
        _ if f.eq_ignore_ascii_case("Lower") => text.to_lowercase(),
        _ if f.eq_ignore_ascii_case("FirstCap") => {
            let mut c = text.chars();
            c.next().map_or_else(String::new, |x| {
                x.to_uppercase().collect::<String>() + c.as_str()
            })
        }
        _ if f.eq_ignore_ascii_case("Caps") => text
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                c.next().map_or_else(String::new, |x| {
                    x.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
                })
            })
            .collect::<Vec<_>>()
            .join(" "),
        "Arabic" => n.map_or_else(|| text.to_owned(), |x| format!("{x}")),
        "ALPHABETIC" | "alphabetic" => n.filter(|x| *x >= 1.0).map_or_else(
            || text.to_owned(),
            |x| {
                let s = format_number(x as i32, &NumFormat::UpperLetter);
                if f == "alphabetic" {
                    s.to_lowercase()
                } else {
                    s
                }
            },
        ),
        "ROMAN" | "roman" => n.filter(|x| *x >= 1.0).map_or_else(
            || text.to_owned(),
            |x| {
                let s = format_number(x as i32, &NumFormat::UpperRoman);
                if f == "roman" {
                    s.to_lowercase()
                } else {
                    s
                }
            },
        ),
        _ if f.eq_ignore_ascii_case("Ordinal") => n.map_or_else(
            || text.to_owned(),
            |x| {
                let k = x as i64;
                if vi {
                    return format!("thứ {k}");
                }
                let suffix = match (k % 100, k % 10) {
                    (11..=13, _) => "th",
                    (_, 1) => "st",
                    (_, 2) => "nd",
                    (_, 3) => "rd",
                    _ => "th",
                };
                format!("{k}{suffix}")
            },
        ),
        _ if f.eq_ignore_ascii_case("CardText") => n
            .filter(|x| *x >= 0.0)
            .map_or_else(|| text.to_owned(), |x| card_text(x as u64, vi)),
        _ if f.eq_ignore_ascii_case("OrdText") => n
            .filter(|x| *x >= 1.0)
            .map_or_else(|| text.to_owned(), |x| ord_text(x as u64, vi)),
        _ if f.eq_ignore_ascii_case("DollarText") => int.filter(|x| *x >= 0.0).map_or_else(
            || text.to_owned(),
            |x| {
                let whole = x.trunc() as u64;
                let cents = ((x - x.trunc()) * 100.0).round() as u64;
                if vi {
                    format!("{} và {cents:02}/100", card_text(whole, true))
                } else {
                    format!("{} and {cents:02}/100", card_text(whole, false))
                }
            },
        ),
        _ if f.eq_ignore_ascii_case("Hex") => n
            .filter(|x| *x >= 0.0)
            .map_or_else(|| text.to_owned(), |x| format!("{:X}", x as u64)),
        // MERGEFORMAT, CHARFORMAT and unknown formats: the text as is.
        _ => text.to_owned(),
    }
}

/// Plain text of a value (numbers without needless decimals).
#[must_use]
pub fn plain(v: &FieldValue, vi: bool) -> String {
    match v {
        FieldValue::Text(t) => t.clone(),
        FieldValue::Number(x) => {
            if x.fract() == 0.0 && x.abs() < 1e15 {
                format!("{x:.0}")
            } else {
                let s = format!("{x}");
                if vi {
                    s.replace('.', ",")
                } else {
                    s
                }
            }
        }
        FieldValue::Date(d) => format_date(d, if vi { "dd/MM/yyyy" } else { "M/d/yyyy" }, vi),
    }
}

/// The result of a field with value `v` after the switches of `instr`: `\@` for dates, `\#` for
/// numbers, then each `\*` format.
#[must_use]
pub fn format(v: &FieldValue, instr: &str, vi: bool) -> String {
    let mut text = match (
        v,
        switch_values(instr, "\\@").first(),
        switch_values(instr, "\\#").first(),
    ) {
        (FieldValue::Date(d), Some(p), _) => format_date(d, p, vi),
        (FieldValue::Number(x), _, Some(p)) => format_picture(*x, p),
        (FieldValue::Text(t), _, Some(p)) => t
            .trim()
            .parse::<f64>()
            .map_or_else(|_| t.clone(), |x| format_picture(x, p)),
        _ => plain(v, vi),
    };
    for f in switch_values(instr, "\\*") {
        text = general(v, &text, &f, vi);
    }
    text
}

// ---------------------------------------------------------------------------
// §17.16.5.1 `=` formulas
// ---------------------------------------------------------------------------

/// What a formula reads: bookmarks (numbers) and table cells (`A1`, ranges, `ABOVE`, `LEFT`, …).
pub trait FormulaEnv {
    /// The number a bookmark holds.
    fn bookmark(&self, name: &str) -> Option<f64>;
    /// Numbers of a cell reference or range (`A1`, `A1:B3`, `ABOVE`, `BELOW`, `LEFT`, `RIGHT`).
    fn cells(&self, reference: &str) -> Option<Vec<f64>>;
}

struct P<'a> {
    t: Vec<char>,
    i: usize,
    env: &'a dyn FormulaEnv,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.t.get(self.i).is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
    }

    fn eat(&mut self, s: &str) -> bool {
        self.ws();
        let n = s.chars().count();
        if self.t.len() >= self.i + n && self.t[self.i..self.i + n].iter().collect::<String>() == s
        {
            self.i += n;
            true
        } else {
            false
        }
    }

    fn cmp(&mut self) -> Option<f64> {
        let a = self.add()?;
        for op in ["<=", ">=", "<>", "=", "<", ">"] {
            if self.eat(op) {
                let b = self.add()?;
                let r = match op {
                    "<=" => a <= b,
                    ">=" => a >= b,
                    "<>" => (a - b).abs() > 1e-12,
                    "=" => (a - b).abs() <= 1e-12,
                    "<" => a < b,
                    _ => a > b,
                };
                return Some(f64::from(u8::from(r)));
            }
        }
        Some(a)
    }

    fn add(&mut self) -> Option<f64> {
        let mut v = self.mul()?;
        loop {
            if self.eat("+") {
                v += self.mul()?;
            } else if self.eat("-") {
                v -= self.mul()?;
            } else {
                return Some(v);
            }
        }
    }

    fn mul(&mut self) -> Option<f64> {
        let mut v = self.pow()?;
        loop {
            if self.eat("*") {
                v *= self.pow()?;
            } else if self.eat("/") {
                v /= self.pow()?;
            } else {
                return Some(v);
            }
        }
    }

    fn pow(&mut self) -> Option<f64> {
        let b = self.unary()?;
        if self.eat("^") {
            return Some(b.powf(self.pow()?));
        }
        Some(b)
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat("-") {
            return Some(-self.unary()?);
        }
        if self.eat("+") {
            return self.unary();
        }
        let v = self.atom()?;
        Some(if self.eat("%") { v / 100.0 } else { v })
    }

    fn word(&mut self) -> String {
        self.ws();
        let st = self.i;
        while self
            .t
            .get(self.i)
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '$'))
        {
            self.i += 1;
        }
        self.t[st..self.i].iter().collect()
    }

    /// Function arguments: each a list of numbers (a range or an expression).
    fn args(&mut self) -> Option<Vec<Vec<f64>>> {
        let mut out = Vec::new();
        if self.eat(")") {
            return Some(out);
        }
        loop {
            let save = self.i;
            let w = self.word();
            self.ws();
            let next = self.t.get(self.i).copied();
            let as_ref = if !w.is_empty() && matches!(next, Some(',' | ')' | ';')) {
                self.env.cells(&w)
            } else {
                None
            };
            if let Some(v) = as_ref {
                out.push(v);
            } else {
                self.i = save;
                out.push(vec![self.cmp()?]);
            }
            if self.eat(")") {
                return Some(out);
            }
            if !(self.eat(",") || self.eat(";")) {
                return None;
            }
        }
    }

    fn atom(&mut self) -> Option<f64> {
        self.ws();
        let c = *self.t.get(self.i)?;
        if c == '(' {
            self.i += 1;
            let v = self.cmp()?;
            return self.eat(")").then_some(v);
        }
        if c.is_ascii_digit() || c == '.' {
            let st = self.i;
            while self
                .t
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || *c == '.')
            {
                self.i += 1;
            }
            // `,` separates arguments (not thousands).
            let s: String = self.t[st..self.i].iter().collect();
            return s.parse().ok();
        }
        let w = self.word();
        if w.is_empty() {
            return None;
        }
        if self.eat("(") {
            let a = self.args()?;
            let flat: Vec<f64> = a.iter().flatten().copied().collect();
            let first = |k: usize| a.get(k).and_then(|v| v.first()).copied();
            let up = w.to_ascii_uppercase();
            return match up.as_str() {
                "SUM" => Some(flat.iter().sum()),
                "PRODUCT" => Some(flat.iter().product()),
                "AVERAGE" if !flat.is_empty() => Some(flat.iter().sum::<f64>() / flat.len() as f64),
                "COUNT" => Some(flat.len() as f64),
                "MAX" => flat.iter().copied().reduce(f64::max),
                "MIN" => flat.iter().copied().reduce(f64::min),
                "ABS" => first(0).map(f64::abs),
                "INT" => first(0).map(f64::trunc),
                "SIGN" => first(0).map(|x| {
                    if x > 0.0 {
                        1.0
                    } else if x < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }),
                "MOD" => Some(first(0)? % first(1)?),
                "ROUND" => {
                    // Decimal rounding: nudge past binary representation error (2.345 → 2.35).
                    let k = 10f64.powi(first(1)? as i32);
                    Some((first(0)? * k * (1.0 + 4.0 * f64::EPSILON)).round() / k)
                }
                "AND" => Some(f64::from(u8::from(first(0)? != 0.0 && first(1)? != 0.0))),
                "OR" => Some(f64::from(u8::from(first(0)? != 0.0 || first(1)? != 0.0))),
                "NOT" => Some(f64::from(u8::from(first(0)? == 0.0))),
                "IF" => Some(if first(0)? == 0.0 {
                    first(2)?
                } else {
                    first(1)?
                }),
                "DEFINED" => Some(f64::from(u8::from(!a.is_empty()))),
                "TRUE" => Some(1.0),
                "FALSE" => Some(0.0),
                _ => None,
            };
        }
        match w.to_ascii_uppercase().as_str() {
            "TRUE" => Some(1.0),
            "FALSE" => Some(0.0),
            _ => self
                .env
                .bookmark(&w)
                .or_else(|| self.env.cells(&w).and_then(|v| v.first().copied())),
        }
    }
}

/// Evaluate a §17.16.5.1 `=` formula (`SUM(ABOVE)`, `Price*1.1`, `IF(A1>5, 1, 0)`, …).
#[must_use]
pub fn formula(expr: &str, env: &dyn FormulaEnv) -> Option<f64> {
    let mut p = P {
        t: expr.chars().collect(),
        i: 0,
        env,
    };
    let v = p.cmp()?;
    p.ws();
    (p.i == p.t.len() && v.is_finite()).then_some(v)
}

/// Column letters and row of a cell reference (`B3` → (1, 2)), 0-based.
#[must_use]
pub fn cell_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_ascii_uppercase();
    let letters: String = s.chars().take_while(char::is_ascii_alphabetic).collect();
    let digits = &s[letters.len()..];
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    let col = letters
        .chars()
        .fold(0usize, |a, c| a * 26 + (c as usize - 'A' as usize + 1));
    let row: usize = digits.parse().ok()?;
    (row >= 1).then_some((col - 1, row - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: DateTime = DateTime {
        year: 2026,
        month: 11,
        day: 9,
        hour: 14,
        minute: 5,
        second: 7,
    };

    // source: ECMA-376 Part 1, §17.16.4.1 date-time pictures; 9 November 2026 is a Monday.
    #[test]
    fn date_pictures() {
        assert_eq!(D.weekday(), 1);
        assert_eq!(format_date(&D, "dd/MM/yyyy", true), "09/11/2026");
        assert_eq!(format_date(&D, "d MMMM yyyy", false), "9 November 2026");
        assert_eq!(
            format_date(&D, "dddd, d MMMM yyyy", true),
            "Thứ Hai, 9 Tháng 11 2026"
        );
        assert_eq!(
            format_date(&D, "'Ngày' dd 'tháng' MM 'năm' yyyy", true),
            "Ngày 09 tháng 11 năm 2026"
        );
        assert_eq!(format_date(&D, "h:mm am/pm", false), "2:05 pm");
        assert_eq!(format_date(&D, "HH:mm:ss", false), "14:05:07");
        assert_eq!(format_date(&D, "MMM d, yy", false), "Nov 9, 26");
        assert_eq!(DateTime::parse_iso("2026-11-09T14:05:07Z"), Some(D));
    }

    // source: ECMA-376 Part 1, §17.16.4.2 numeric pictures.
    #[test]
    fn numeric_pictures() {
        assert_eq!(format_picture(1234.567, "#,##0.00"), "1,234.57");
        assert_eq!(format_picture(5.0, "00.0"), "05.0");
        assert_eq!(format_picture(-12.5, "0.0;(0.0)"), "(12.5)");
        assert_eq!(format_picture(0.0, "0.00;-0.00;'không'"), "không");
        assert_eq!(format_picture(1234.0, "x##"), "234");
        assert_eq!(format_picture(-3.0, "-0"), "-3");
        assert_eq!(format_picture(3.0, "+0"), "+3");
        assert_eq!(format_picture(0.25, "0%"), "0%");
        assert_eq!(format_picture(1_500_000.0, "#,##0 'đ'"), "1,500,000 đ");
    }

    // source: ECMA-376 Part 1, §17.16.4.3 general formatting switches.
    #[test]
    fn general_formats() {
        let n = |x: f64, s: &str| format(&FieldValue::Number(x), &format!("PAGE \\* {s}"), false);
        assert_eq!(n(4.0, "ROMAN"), "IV");
        assert_eq!(n(4.0, "roman"), "iv");
        assert_eq!(n(28.0, "ALPHABETIC"), "BB");
        assert_eq!(n(23.0, "Ordinal"), "23rd");
        assert_eq!(n(12.0, "Ordinal"), "12th");
        assert_eq!(n(125.0, "CardText"), "one hundred twenty-five");
        assert_eq!(n(21.0, "OrdText"), "twenty-first");
        assert_eq!(n(255.0, "Hex"), "FF");
        let t = |s: &str, f: &str| {
            format(
                &FieldValue::Text(s.into()),
                &format!("TITLE \\* {f}"),
                false,
            )
        };
        assert_eq!(t("hợp đồng mua bán", "Upper"), "HỢP ĐỒNG MUA BÁN");
        assert_eq!(t("hợp đồng mua bán", "Caps"), "Hợp Đồng Mua Bán");
        assert_eq!(t("hợp đồng", "FirstCap"), "Hợp đồng");
        assert_eq!(t("Hợp Đồng", "Lower \\* MERGEFORMAT"), "hợp đồng");
        // Vietnamese numbers in words.
        assert_eq!(card_text(125, true), "một trăm hai mươi lăm");
        assert_eq!(
            card_text(1_005_021, true),
            "một triệu không trăm linh năm nghìn không trăm hai mươi mốt"
        );
        assert_eq!(card_text(14, true), "mười bốn");
        assert_eq!(card_text(24, true), "hai mươi tư");
        assert_eq!(ord_text(4, true), "thứ tư");
        assert_eq!(n(12.5, "DollarText"), "twelve and 50/100");
        assert_eq!(
            words("REF \"tên có cách\" \\h"),
            vec!["REF", "tên có cách", "\\h"]
        );
    }

    struct Env;
    impl FormulaEnv for Env {
        fn bookmark(&self, name: &str) -> Option<f64> {
            (name == "Gia").then_some(200.0)
        }
        fn cells(&self, r: &str) -> Option<Vec<f64>> {
            match r.to_ascii_uppercase().as_str() {
                "ABOVE" => Some(vec![1.0, 2.0, 3.0]),
                "A1" => Some(vec![10.0]),
                "A1:B2" => Some(vec![1.0, 2.0, 3.0, 4.0]),
                _ => None,
            }
        }
    }

    // source: ECMA-376 Part 1, §17.16.5.1 = (formula) operators and functions.
    #[test]
    fn formulas() {
        let f = |s: &str| formula(s, &Env).unwrap_or_else(|| panic!("{s}"));
        assert_eq!(f("SUM(ABOVE)"), 6.0);
        assert!((f("Gia*1.1") - 220.0).abs() < 1e-9);
        assert_eq!(f("(2+3)*4-2^3"), 12.0);
        assert_eq!(f("AVERAGE(A1:B2)"), 2.5);
        assert_eq!(f("IF(A1>5, 1, 0)"), 1.0);
        assert_eq!(f("MAX(1, 7, 3) + MIN(A1:B2)"), 8.0);
        assert_eq!(f("ROUND(2.345, 2)"), 2.35);
        assert_eq!(f("MOD(10, 3)"), 1.0);
        assert_eq!(f("50%"), 0.5);
        assert_eq!(f("A1 >= 10"), 1.0);
        assert!(formula("1+", &Env).is_none());
        assert_eq!(cell_ref("B3"), Some((1, 2)));
        assert_eq!(cell_ref("AA1"), Some((26, 0)));
    }
}

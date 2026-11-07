//! A compact number-format renderer used by `TEXT` (ECMA-376 Part 1, §18.8.31
//! numFmt format codes: sections separated by `;`, digit placeholders `0 # ?`,
//! `.`, `,`, `%`, `E+`, date/time codes `y m d h s AM/PM`, literals).
//!
//! Supported: up to 4 sections, `[condition]`/`[color]` brackets are skipped,
//! `[h]`/`[m]`/`[s]` elapsed time, quoted and backslash literals. Literals placed
//! between digit placeholders are emitted after the number.

use crate::date::{serial_to_hms, serial_to_ymd, weekday_sun0, DateSystem};
use crate::value::number_to_text;

const MONTH_NAMES: [&str; 12] = [
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
const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Split a format code into sections at unquoted `;`.
fn sections(fmt: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_quote = false;
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                push_last(&mut out, c);
            }
            '\\' if !in_quote => {
                push_last(&mut out, c);
                if let Some(n) = chars.next() {
                    push_last(&mut out, n);
                }
            }
            ';' if !in_quote => out.push(String::new()),
            _ => push_last(&mut out, c),
        }
    }
    out
}

fn push_last(v: &mut [String], c: char) {
    if let Some(last) = v.last_mut() {
        last.push(c);
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Lit(String),
    Digit(char),
    Dot,
    Comma,
    Percent,
    Exp(bool),
    At,
    Date(String),
    AmPm(bool),
    General,
}

fn tokenize(sec: &str) -> Vec<Tok> {
    let chars: Vec<char> = sec.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                out.push(Tok::Lit(s));
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    out.push(Tok::Lit(n.to_string()));
                    i += 1;
                }
            }
            '_' => {
                out.push(Tok::Lit(" ".into()));
                i += 1;
            }
            '*' => i += 1,
            '[' => {
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != ']' {
                    s.push(chars[i]);
                    i += 1;
                }
                let l = s.to_ascii_lowercase();
                if !l.is_empty() && l.chars().all(|ch| ch == 'h' || ch == 'm' || ch == 's') {
                    out.push(Tok::Date(format!("[{l}]")));
                }
            }
            '0' | '#' | '?' => out.push(Tok::Digit(c)),
            '.' => out.push(Tok::Dot),
            ',' => out.push(Tok::Comma),
            '%' => out.push(Tok::Percent),
            '@' => out.push(Tok::At),
            'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => {
                out.push(Tok::Exp(chars[i + 1] == '+'));
                i += 1;
            }
            'G' | 'g'
                if chars[i..]
                    .iter()
                    .take(7)
                    .collect::<String>()
                    .eq_ignore_ascii_case("general") =>
            {
                out.push(Tok::General);
                i += 6;
            }
            'A' | 'a' => {
                let rest: String = chars[i..].iter().take(5).collect();
                if rest.eq_ignore_ascii_case("AM/PM") {
                    out.push(Tok::AmPm(true));
                    i += 4;
                } else if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case("A/P") {
                    out.push(Tok::AmPm(false));
                    i += 2;
                } else {
                    out.push(Tok::Lit(c.to_string()));
                }
            }
            'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
                let lc = c.to_ascii_lowercase();
                let mut s = String::new();
                while i < chars.len() && chars[i].to_ascii_lowercase() == lc {
                    s.push(lc);
                    i += 1;
                }
                out.push(Tok::Date(s));
                continue;
            }
            _ => out.push(Tok::Lit(c.to_string())),
        }
        i += 1;
    }
    out
}

/// Format a number with a format code (`TEXT`, cell display).
pub fn format_number(x: f64, fmt: &str, system: DateSystem) -> String {
    let secs = sections(fmt);
    let (sec, value, auto_minus) = match secs.len() {
        0 | 1 => (secs.first().cloned().unwrap_or_default(), x, true),
        2 => {
            if x < 0.0 {
                (secs[1].clone(), -x, false)
            } else {
                (secs[0].clone(), x, false)
            }
        }
        _ => {
            if x > 0.0 {
                (secs[0].clone(), x, false)
            } else if x < 0.0 {
                (secs[1].clone(), -x, false)
            } else {
                (secs[2].clone(), x, false)
            }
        }
    };
    let toks = tokenize(&sec);
    if toks.is_empty() {
        return String::new();
    }
    if toks
        .iter()
        .any(|t| matches!(t, Tok::Date(_) | Tok::AmPm(_)))
    {
        return format_date(value, &toks, system);
    }
    if toks.iter().any(|t| matches!(t, Tok::General)) {
        let mut out = String::new();
        for t in &toks {
            match t {
                Tok::General => out.push_str(&number_to_text(if auto_minus {
                    value
                } else {
                    value.abs()
                })),
                Tok::Lit(s) => out.push_str(s),
                _ => {}
            }
        }
        return out;
    }
    format_numeric(value, &toks, auto_minus)
}

/// Format text with a format code: the 4th section (or a section containing `@`).
pub fn format_text(s: &str, fmt: &str) -> String {
    let secs = sections(fmt);
    let sec = if secs.len() >= 4 {
        secs[3].clone()
    } else if secs.len() == 1 && secs[0].contains('@') {
        secs[0].clone()
    } else {
        return s.to_owned();
    };
    let mut out = String::new();
    for t in tokenize(&sec) {
        match t {
            Tok::At => out.push_str(s),
            Tok::Lit(l) => out.push_str(&l),
            _ => {}
        }
    }
    out
}

fn round_half_away(x: f64, decimals: usize) -> f64 {
    let p = 10f64.powi(decimals as i32);
    let y = crate::value::round_sig15(x * p);
    y.abs().round().copysign(y) / p
}

fn group_thousands(digits: &str) -> String {
    let mut out = String::new();
    let n = digits.len();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Best rational approximation `n/d` of `x` in `[0, 1]` with `d <= max_den`.
fn best_fraction(x: f64, max_den: u64) -> (u64, u64) {
    let mut best = (0u64, 1u64);
    let mut best_err = f64::INFINITY;
    for d in 1..=max_den.max(1) {
        let n = (x * d as f64).round();
        let err = (x - n / d as f64).abs();
        if err < best_err - 1e-12 {
            best_err = err;
            best = (n as u64, d);
        }
    }
    best
}

fn pad_left(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_owned()
    } else {
        " ".repeat(width - n) + s
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_owned()
    } else {
        s.to_owned() + &" ".repeat(width - n)
    }
}

/// Fraction formats such as `# ?/?`, `# ??/??`, `?/4` (ECMA-376 Part 1, §18.8.31:
/// `/` between digit placeholders; a literal denominator fixes the denominator).
fn format_fraction(x: f64, toks: &[Tok], auto_minus: bool) -> Option<String> {
    let is_lit_digit = |t: &Tok| matches!(t, Tok::Lit(s) if s.len() == 1 && s.as_bytes()[0].is_ascii_digit());
    let slash = toks.iter().enumerate().position(|(i, t)| {
        *t == Tok::Lit("/".into())
            && i > 0
            && matches!(toks[i - 1], Tok::Digit(_))
            && toks.get(i + 1).is_some_and(|n| matches!(n, Tok::Digit(_)) || is_lit_digit(n))
    })?;
    let mut num_start = slash;
    while num_start > 0 && matches!(toks[num_start - 1], Tok::Digit(_)) {
        num_start -= 1;
    }
    let num_width = slash - num_start;
    let first_digit = toks.iter().position(|t| matches!(t, Tok::Digit(_)))?;
    let int_toks: Vec<char> = toks[first_digit..num_start]
        .iter()
        .filter_map(|t| if let Tok::Digit(c) = t { Some(*c) } else { None })
        .collect();
    let has_int = !int_toks.is_empty();
    let mut den_end = slash + 1;
    let (max_den, fixed_den, den_width) = if matches!(toks[slash + 1], Tok::Digit(_)) {
        while den_end < toks.len() && matches!(toks[den_end], Tok::Digit(_)) {
            den_end += 1;
        }
        let w = den_end - slash - 1;
        (10u64.pow(w.min(4) as u32) - 1, None, w)
    } else {
        let mut s = String::new();
        while den_end < toks.len() && is_lit_digit(&toks[den_end]) {
            if let Tok::Lit(l) = &toks[den_end] {
                s.push_str(l);
            }
            den_end += 1;
        }
        let d: u64 = s.parse().ok().filter(|d| *d > 0)?;
        (d, Some(d), s.len())
    };
    let negative = x < 0.0;
    let a = x.abs();
    let (mut int, frac) = if has_int { (a.trunc(), a.fract()) } else { (0.0, a) };
    let (mut n, d) = match fixed_den {
        Some(d) => ((frac * d as f64).round() as u64, d),
        None if has_int => best_fraction(frac, max_den),
        None => {
            // Improper fraction: approximate the whole value.
            let (bn, bd) = best_fraction(frac.fract(), max_den);
            (bn + frac.trunc() as u64 * bd, bd)
        }
    };
    if has_int && n == d {
        int += 1.0;
        n = 0;
    }
    let mut out = String::new();
    if negative && auto_minus && (int > 0.0 || n > 0) {
        out.push('-');
    }
    for t in &toks[..first_digit] {
        if let Tok::Lit(s) = t {
            out.push_str(s);
        }
    }
    if has_int {
        let min_int = int_toks.iter().filter(|c| **c == '0').count();
        let mut s = if int == 0.0 { String::new() } else { format!("{int:.0}") };
        while s.len() < min_int {
            s.insert(0, '0');
        }
        let width = int_toks.iter().filter(|c| **c != '#').count();
        out.push_str(&pad_left(&s, width));
    }
    let mut frac_str = String::new();
    for t in &toks[first_digit..num_start] {
        if let Tok::Lit(s) = t {
            frac_str.push_str(s);
        }
    }
    let between = frac_str.clone();
    frac_str.push_str(&pad_left(&n.to_string(), num_width));
    frac_str.push('/');
    frac_str.push_str(&pad_right(&d.to_string(), den_width));
    if has_int && n == 0 {
        if int == 0.0 {
            out.push('0');
        }
        out.push_str(&" ".repeat(frac_str.chars().count()));
    } else if has_int && int == 0.0 {
        out.push_str(&frac_str[between.len()..]);
    } else {
        out.push_str(&frac_str);
    }
    for t in &toks[den_end..] {
        match t {
            Tok::Lit(s) => out.push_str(s),
            Tok::Percent => out.push('%'),
            _ => {}
        }
    }
    Some(out)
}

fn format_numeric(x: f64, toks: &[Tok], auto_minus: bool) -> String {
    if let Some(s) = format_fraction(x, toks, auto_minus) {
        return s;
    }
    let mut x = x;
    let first_digit = toks.iter().position(|t| matches!(t, Tok::Digit(_)));
    let last_digit = toks.iter().rposition(|t| matches!(t, Tok::Digit(_)));
    let Some((first, last)) = first_digit.zip(last_digit) else {
        // Only literals (and maybe %): output literals.
        let mut out = String::new();
        if auto_minus && x < 0.0 {
            out.push('-');
        }
        for t in toks {
            match t {
                Tok::Lit(s) => out.push_str(s),
                Tok::Percent => out.push('%'),
                _ => {}
            }
        }
        return out;
    };
    let percent = toks.iter().filter(|t| **t == Tok::Percent).count();
    for _ in 0..percent {
        x *= 100.0;
    }
    let exp_pos = toks.iter().position(|t| matches!(t, Tok::Exp(_)));
    let dot_pos = toks[first..=last]
        .iter()
        .position(|t| *t == Tok::Dot)
        .map(|p| p + first);
    let int_end = dot_pos.or(exp_pos).unwrap_or(last + 1);
    // Commas right after the last integer placeholder scale by 1000.
    let mut scale_commas = 0;
    let mut j = int_end;
    while j > first && toks[j - 1] == Tok::Comma {
        scale_commas += 1;
        j -= 1;
    }
    if dot_pos.is_none() && exp_pos.is_none() {
        let mut k = last + 1;
        while k < toks.len() && toks[k] == Tok::Comma {
            scale_commas += 1;
            k += 1;
        }
    }
    for _ in 0..scale_commas {
        x /= 1000.0;
    }
    let int_toks: Vec<char> = toks[first..int_end]
        .iter()
        .filter_map(|t| {
            if let Tok::Digit(c) = t {
                Some(*c)
            } else {
                None
            }
        })
        .collect();
    let thousands = toks[first..j].contains(&Tok::Comma);
    let frac_end = exp_pos.unwrap_or(last + 1);
    let frac_toks: Vec<char> = match dot_pos {
        Some(d) => toks[d + 1..frac_end]
            .iter()
            .filter_map(|t| {
                if let Tok::Digit(c) = t {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect(),
        None => Vec::new(),
    };
    let negative = x < 0.0;
    let mut a = x.abs();
    let mut exp_str = String::new();
    if let Some(ep) = exp_pos {
        let int_count = int_toks.len().max(1) as i32;
        let e = if a == 0.0 {
            0
        } else {
            let raw = a.log10().floor() as i32;
            raw - (int_count - 1)
        };
        a /= 10f64.powi(e);
        // Rounding may push the mantissa to the next power of ten.
        let r = round_half_away(a, frac_toks.len());
        let (a2, e2) = if r >= 10f64.powi(int_count) {
            (r / 10.0, e + 1)
        } else {
            (r, e)
        };
        a = a2;
        let plus = matches!(toks[ep], Tok::Exp(true));
        let exp_digits = toks[ep + 1..]
            .iter()
            .take_while(|t| matches!(t, Tok::Digit(_)))
            .count()
            .max(1);
        exp_str.push('E');
        if e2 < 0 {
            exp_str.push('-');
        } else if plus {
            exp_str.push('+');
        }
        exp_str.push_str(&format!("{:0width$}", e2.abs(), width = exp_digits));
    }
    let rounded = round_half_away(a, frac_toks.len());
    let s = format!("{:.*}", frac_toks.len(), rounded);
    let (int_part, frac_part) = s.split_once('.').unwrap_or((&s, ""));
    let int_part = if int_part == "0" { "" } else { int_part };
    let min_int = int_toks.iter().filter(|c| **c == '0').count();
    let space_int = int_toks.iter().filter(|c| **c == '?').count();
    let mut int_str = int_part.to_owned();
    while int_str.len() < min_int {
        int_str.insert(0, '0');
    }
    if thousands {
        int_str = group_thousands(&int_str);
    }
    while int_str.len() < min_int + space_int {
        int_str.insert(0, ' ');
    }
    let mut frac_str = String::new();
    let fchars: Vec<char> = frac_part.chars().collect();
    // Trailing zeros: '#' drops them, '?' turns them into spaces.
    let mut keep = fchars.len();
    while keep > 0 && fchars[keep - 1] == '0' && frac_toks[keep - 1] != '0' {
        keep -= 1;
    }
    for (i, c) in fchars.iter().enumerate() {
        if i < keep {
            frac_str.push(*c);
        } else if frac_toks[i] == '?' {
            frac_str.push(' ');
        }
    }
    let mut out = String::new();
    let zero_result = int_str.trim().is_empty() && frac_str.trim_matches(['0', ' ']).is_empty();
    if negative && auto_minus && !zero_result {
        out.push('-');
    }
    for t in &toks[..first] {
        match t {
            Tok::Lit(s) => out.push_str(s),
            Tok::Percent => out.push('%'),
            _ => {}
        }
    }
    out.push_str(&int_str);
    if dot_pos.is_some() {
        out.push('.');
        out.push_str(&frac_str);
    }
    out.push_str(&exp_str);
    let after = exp_pos.map_or(last + 1, |e| {
        e + 1
            + toks[e + 1..]
                .iter()
                .take_while(|t| matches!(t, Tok::Digit(_)))
                .count()
    });
    for t in toks[first..].iter().take(after.saturating_sub(first)) {
        if let Tok::Lit(s) = t {
            out.push_str(s);
        }
    }
    for t in &toks[after.min(toks.len())..] {
        match t {
            Tok::Lit(s) => out.push_str(s),
            Tok::Percent => out.push('%'),
            _ => {}
        }
    }
    out
}

fn format_date(x: f64, toks: &[Tok], system: DateSystem) -> String {
    let Some((y, mo, d)) = serial_to_ymd(x, system) else {
        return "#".repeat(8);
    };
    let has_ampm = toks.iter().any(|t| matches!(t, Tok::AmPm(_)));
    // Seconds precision from a ".0" following seconds.
    let frac_digits = toks
        .windows(2)
        .find_map(|w| match w {
            [Tok::Date(s), Tok::Dot] if s.starts_with('s') => Some(()),
            _ => None,
        })
        .map_or(0, |()| {
            let start = toks.iter().position(|t| *t == Tok::Dot).unwrap_or(0) + 1;
            toks[start..]
                .iter()
                .take_while(|t| **t == Tok::Digit('0'))
                .count()
        });
    let total_secs = if frac_digits > 0 {
        (x - x.floor()) * 86_400.0
    } else {
        f64::from({
            let (h, m, s) = serial_to_hms(x);
            h * 3600 + m * 60 + s
        })
    };
    let secs_int = total_secs.floor() as u32;
    let (h, mi, s) = (secs_int / 3600, (secs_int / 60) % 60, secs_int % 60);
    let mut out = String::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Date(code) => {
                let prev_is_hour = toks[..i]
                    .iter()
                    .rev()
                    .find(|t| matches!(t, Tok::Date(_)))
                    .is_some_and(
                        |t| matches!(t, Tok::Date(c) if c.starts_with('h') || c.starts_with("[h")),
                    );
                let next_is_sec = toks[i + 1..]
                    .iter()
                    .find(|t| matches!(t, Tok::Date(_)))
                    .is_some_and(|t| matches!(t, Tok::Date(c) if c.starts_with('s')));
                let part = match code.as_str() {
                    "yy" => format!("{:02}", y.rem_euclid(100)),
                    c if c.starts_with('y') => format!("{y:04}"),
                    "m" | "mm" if prev_is_hour || next_is_sec => {
                        if code.len() == 2 {
                            format!("{mi:02}")
                        } else {
                            mi.to_string()
                        }
                    }
                    "m" => mo.to_string(),
                    "mm" => format!("{mo:02}"),
                    "mmm" => MONTH_NAMES[(mo - 1) as usize][..3].to_owned(),
                    "mmmmm" => MONTH_NAMES[(mo - 1) as usize][..1].to_owned(),
                    c if c.starts_with('m') => MONTH_NAMES[(mo - 1) as usize].to_owned(),
                    "d" => d.to_string(),
                    "dd" => format!("{d:02}"),
                    "ddd" => DAY_NAMES[weekday_sun0(x, system) as usize][..3].to_owned(),
                    c if c.starts_with('d') => {
                        DAY_NAMES[weekday_sun0(x, system) as usize].to_owned()
                    }
                    "h" | "hh" => {
                        let hh = if has_ampm {
                            if h % 12 == 0 {
                                12
                            } else {
                                h % 12
                            }
                        } else {
                            h
                        };
                        if code.len() == 2 {
                            format!("{hh:02}")
                        } else {
                            hh.to_string()
                        }
                    }
                    "s" => s.to_string(),
                    "ss" => format!("{s:02}"),
                    "[h]" | "[hh]" => ((x * 24.0).floor() as i64).to_string(),
                    "[m]" | "[mm]" => ((x * 1440.0).floor() as i64).to_string(),
                    "[s]" | "[ss]" => ((x * 86_400.0).floor() as i64).to_string(),
                    other => other.to_owned(),
                };
                out.push_str(&part);
            }
            Tok::AmPm(full) => {
                let pm = h >= 12;
                out.push_str(match (full, pm) {
                    (true, false) => "AM",
                    (true, true) => "PM",
                    (false, false) => "A",
                    (false, true) => "P",
                });
            }
            Tok::Dot if frac_digits > 0 => {
                let frac = total_secs - total_secs.floor();
                let digits = format!("{frac:.frac_digits$}");
                out.push_str(digits.trim_start_matches('0'));
                i += 1 + frac_digits;
                continue;
            }
            Tok::Lit(l) => out.push_str(l),
            Tok::Dot => out.push('.'),
            Tok::Comma => out.push(','),
            Tok::Digit(c) => out.push(*c),
            Tok::Percent => out.push('%'),
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_formats() {
        let s = DateSystem::Date1900;
        assert_eq!(format_number(1234.567, "0.00", s), "1234.57");
        assert_eq!(format_number(1234.567, "#,##0.00", s), "1,234.57");
        assert_eq!(format_number(0.256, "0%", s), "26%");
        assert_eq!(format_number(0.5, "0.0%", s), "50.0%");
        assert_eq!(format_number(-5.0, "0", s), "-5");
        assert_eq!(format_number(-5.0, "0;(0)", s), "(5)");
        assert_eq!(format_number(0.0, "0;-0;\"zero\"", s), "zero");
        assert_eq!(format_number(12345.0, "0.00E+00", s), "1.23E+04");
        assert_eq!(format_number(5.0, "000", s), "005");
        assert_eq!(format_number(1.5, "#.##", s), "1.5");
        assert_eq!(format_number(1_234_567.0, "#,##0,", s), "1,235");
        assert_eq!(format_number(3.0, "\"$\"0.00", s), "$3.00");
    }

    #[test]
    fn fraction_formats() {
        let s = DateSystem::Date1900;
        assert_eq!(format_number(4.34, "# ?/?", s), "4 1/3");
        assert_eq!(format_number(1.25, "# ?/4", s), "1 1/4");
        assert_eq!(format_number(0.5, "?/4", s), "2/4");
        assert_eq!(format_number(5.75, "?/?", s), "23/4");
        assert_eq!(format_number(3.0, "# ?/?", s), "3    ");
        assert_eq!(format_number(-1.5, "# ?/?", s), "-1 1/2");
        assert_eq!(format_number(0.3125, "# ??/??", s), " 5/16");
    }

    #[test]
    fn date_formats() {
        let s = DateSystem::Date1900;
        // 2020-01-15 = serial 43845
        assert_eq!(format_number(43_845.0, "yyyy-mm-dd", s), "2020-01-15");
        assert_eq!(format_number(43_845.0, "dd/mm/yyyy", s), "15/01/2020");
        assert_eq!(format_number(43_845.0, "mmm d, yyyy", s), "Jan 15, 2020");
        assert_eq!(format_number(43_845.75, "hh:mm", s), "18:00");
        assert_eq!(format_number(43_845.75, "h:mm AM/PM", s), "6:00 PM");
        assert_eq!(format_number(1.5, "[h]:mm", s), "36:00");
        assert_eq!(format_number(43_845.0, "dddd", s), "Wednesday");
    }
}

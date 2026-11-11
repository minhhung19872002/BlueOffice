//! Number format engine — parses and applies spreadsheet format codes
//! (ECMA-376 Part 1, §18.8.31 `numFmts` / §18.8.30 `numFmt`, format code syntax).
//!
//! Supported:
//! * up to four sections `positive;negative;zero;text`, conditions (`[>100]`),
//!   colours (`[Red]`, `[Color12]`);
//! * digit placeholders `0 # ?`, decimal point, thousands separator and scaling
//!   commas, `%`, scientific `E+ E- e+ e-`, fractions (`# ?/?`, `?/8`);
//! * dates and times `y m d h s`, `AM/PM`, `A/P`, elapsed `[h] [m] [s]`,
//!   fractional seconds `ss.00`, Buddhist years `bbbb`;
//! * text `@`, literals (`"…"`, `\x`, plain characters), `_x` padding (the width of
//!   `x`, reported in [`Formatted::skips`]) and `*x` fill;
//! * locale tags `[$-42A]`, currency tags `[$₫-42A]`, system date/time `[$-F800]` `[$-F400]`;
//! * calendars: `[$-CCLLLL]` calendar byte (Hijri, Thai Buddhist, East Asian / Vietnamese
//!   lunisolar — see [`calendar`]), `B1` / `B2` prefixes, era / sexagenary year `g`;
//! * East Asian numerals `[DBNum1]`…`[DBNum4]` ([`dbnum`]);
//! * `General`.

mod builtin;
pub mod calendar;
mod date;
pub mod dbnum;
pub mod input;
mod locale;

pub use builtin::builtin_format;
pub use date::{
    civil_from_days, datetime_to_serial, days_from_civil, serial_to_datetime, DateTime,
};
pub use input::{
    parse_typed_date, parse_typed_date_in_year, parse_typed_number, TypedDate, TypedFormat,
    TypedNumber,
};
pub use locale::Locale;

use calendar::Calendar;

use bo_model::sheet::CellValue;

/// A colour named by a `[Color]` section tag (§18.8.31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmtColor {
    /// `[Black]`.
    Black,
    /// `[Blue]`.
    Blue,
    /// `[Cyan]`.
    Cyan,
    /// `[Green]`.
    Green,
    /// `[Magenta]`.
    Magenta,
    /// `[Red]`.
    Red,
    /// `[White]`.
    White,
    /// `[Yellow]`.
    Yellow,
    /// `[ColorN]`, N = 1–56 (index into the default palette).
    Indexed(u8),
}

impl FmtColor {
    /// sRGB value of the colour (`[ColorN]` uses the default indexed palette, §18.8.27).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            FmtColor::Black => (0, 0, 0),
            FmtColor::Blue => (0, 0, 255),
            FmtColor::Cyan => (0, 255, 255),
            FmtColor::Green => (0, 255, 0),
            FmtColor::Magenta => (255, 0, 255),
            FmtColor::Red => (255, 0, 0),
            FmtColor::White => (255, 255, 255),
            FmtColor::Yellow => (255, 255, 0),
            // [ColorN] is 1-based and maps to palette entries 8.. (the first 8 duplicate).
            FmtColor::Indexed(n) => crate::color::indexed_rgb(u32::from(n) + 7),
        }
    }
}

/// Result of formatting a value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formatted {
    /// Display text.
    pub text: String,
    /// Colour requested by the section, if any.
    pub color: Option<FmtColor>,
    /// `*x` repeat fill: byte offset in `text` where the fill character `x`
    /// should be repeated to fill the cell width.
    pub fill: Option<(usize, char)>,
    /// `_x` padding: byte offset in `text` of a placeholder space that should be
    /// as wide as the character `x` (§18.8.31: "skip the width of the next character").
    pub skips: Vec<(usize, char)>,
    /// The value cannot be shown with this format (negative date, out of range);
    /// renderers display `#` characters across the cell.
    pub invalid: bool,
}

impl Formatted {
    fn plain(text: String) -> Self {
        Formatted {
            text,
            ..Formatted::default()
        }
    }

    /// `_x`: a placeholder space standing for the width of `x`.
    fn skip(&mut self, c: char) {
        self.skips.push((self.text.len(), c));
        self.text.push(' ');
    }

    /// `*x`: remember where the fill character repeats.
    fn fill_at(&mut self, c: char) {
        self.fill = Some((self.text.len(), c));
    }

    /// Insert `s` at the start of the text, shifting the fill / skip offsets.
    fn prepend(&mut self, s: &str) {
        self.text.insert_str(0, s);
        if let Some((p, c)) = self.fill {
            self.fill = Some((p + s.len(), c));
        }
        for (p, _) in &mut self.skips {
            *p += s.len();
        }
    }
}

/// Options that are not part of the format code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormatOptions {
    /// Separators, month/day names and currency of the user's locale.
    pub locale: Locale,
    /// Workbook uses the 1904 date system (`workbookPr/@date1904`).
    pub date1904: bool,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Digit {
    Zero,
    Hash,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Elapsed {
    Hours,
    Minutes,
    Seconds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Lit(String),
    Digit(Digit),
    Point,
    Comma,
    Percent,
    Exp {
        plus: bool,
        upper: bool,
    },
    Slash,
    At,
    Fill(char),
    Skip(char),
    General,
    Year(u8),
    Buddhist(u8),
    Month(u8),
    Minute(u8),
    Day(u8),
    Hour(u8),
    Second(u8),
    SubSec(u8),
    /// `AM/PM` (`upper`=true keeps locale text) or `A/P` (`short`).
    AmPm {
        short: bool,
        lower: bool,
    },
    Elapsed(Elapsed, u8),
    /// `g`…`ggg`: era name; the sexagenary year name in a lunisolar calendar.
    Era(u8),
}

impl Tok {
    fn is_date(&self) -> bool {
        matches!(
            self,
            Tok::Year(_)
                | Tok::Buddhist(_)
                | Tok::Month(_)
                | Tok::Minute(_)
                | Tok::Day(_)
                | Tok::Hour(_)
                | Tok::Second(_)
                | Tok::SubSec(_)
                | Tok::AmPm { .. }
                | Tok::Elapsed(..)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Condition {
    op: Op,
    value: f64,
}

impl Condition {
    fn matches(self, v: f64) -> bool {
        match self.op {
            Op::Lt => v < self.value,
            Op::Le => v <= self.value,
            Op::Gt => v > self.value,
            Op::Ge => v >= self.value,
            Op::Eq => v == self.value,
            Op::Ne => v != self.value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    General,
    Number,
    Date,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
struct Section {
    toks: Vec<Tok>,
    color: Option<FmtColor>,
    cond: Option<Condition>,
    lcid: Option<u32>,
    /// Calendar of the date parts (locale tag calendar byte or `B1`/`B2`).
    calendar: calendar::Calendar,
    /// `[DBNum1]`…`[DBNum4]` numeral style.
    dbnum: Option<u8>,
    kind: Kind,
}

/// A parsed number format code.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberFormat {
    sections: Vec<Section>,
}

fn split_sections(code: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = code.chars();
    let mut in_quote = false;
    let mut in_bracket = false;
    while let Some(c) = chars.next() {
        let cur = out.last_mut().expect("non-empty");
        match c {
            '"' if !in_bracket => {
                in_quote = !in_quote;
                cur.push(c);
            }
            '\\' | '_' | '*' if !in_quote && !in_bracket => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '[' if !in_quote => {
                in_bracket = true;
                cur.push(c);
            }
            ']' if !in_quote => {
                in_bracket = false;
                cur.push(c);
            }
            ';' if !in_quote && !in_bracket => out.push(String::new()),
            _ => cur.push(c),
        }
    }
    out
}

fn color_by_name(s: &str) -> Option<FmtColor> {
    let l = s.to_ascii_lowercase();
    Some(match l.as_str() {
        "black" => FmtColor::Black,
        "blue" => FmtColor::Blue,
        "cyan" => FmtColor::Cyan,
        "green" => FmtColor::Green,
        "magenta" => FmtColor::Magenta,
        "red" => FmtColor::Red,
        "white" => FmtColor::White,
        "yellow" => FmtColor::Yellow,
        _ => {
            let n: u8 = l.strip_prefix("color")?.parse().ok()?;
            if (1..=56).contains(&n) {
                FmtColor::Indexed(n)
            } else {
                return None;
            }
        }
    })
}

fn parse_condition(s: &str) -> Option<Condition> {
    let (op, rest) = if let Some(r) = s.strip_prefix("<=") {
        (Op::Le, r)
    } else if let Some(r) = s.strip_prefix(">=") {
        (Op::Ge, r)
    } else if let Some(r) = s.strip_prefix("<>") {
        (Op::Ne, r)
    } else if let Some(r) = s.strip_prefix('<') {
        (Op::Lt, r)
    } else if let Some(r) = s.strip_prefix('>') {
        (Op::Gt, r)
    } else {
        let r = s.strip_prefix('=')?;
        (Op::Eq, r)
    };
    let value: f64 = rest.trim().parse().ok()?;
    Some(Condition { op, value })
}

fn push_lit(toks: &mut Vec<Tok>, s: &str) {
    if let Some(Tok::Lit(prev)) = toks.last_mut() {
        prev.push_str(s);
    } else {
        toks.push(Tok::Lit(s.to_owned()));
    }
}

fn parse_section(src: &str, locale_hint: &Locale) -> Section {
    let chars: Vec<char> = src.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut color = None;
    let mut cond = None;
    let mut lcid = None;
    let mut cal = None;
    let mut dbnum = None;
    let mut i = 0;
    let run = |i: usize, lower: char| -> usize {
        chars[i..]
            .iter()
            .take_while(|c| c.to_ascii_lowercase() == lower)
            .count()
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|&x| x == '"')
                    .map_or(chars.len(), |p| i + 1 + p);
                let s: String = chars[i + 1..end].iter().collect();
                push_lit(&mut toks, &s);
                i = end + 1;
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    push_lit(&mut toks, &n.to_string());
                }
                i += 2;
            }
            '_' => {
                if let Some(n) = chars.get(i + 1) {
                    toks.push(Tok::Skip(*n));
                }
                i += 2;
            }
            '*' => {
                if let Some(n) = chars.get(i + 1) {
                    toks.push(Tok::Fill(*n));
                }
                i += 2;
            }
            '[' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|&x| x == ']')
                    .map_or(chars.len(), |p| i + 1 + p);
                let inner: String = chars[i + 1..end.min(chars.len())].iter().collect();
                i = end + 1;
                let lower = inner.to_ascii_lowercase();
                if let Some(col) = color_by_name(&inner) {
                    color = Some(col);
                } else if let Some(cnd) = parse_condition(&inner) {
                    cond = Some(cnd);
                } else if let Some(rest) = inner.strip_prefix('$') {
                    // Locale / currency tag: [$<currency>-<lcid hex>]
                    // The symbol ends at the first '-'; the rest is the locale id
                    // (or a non-numeric qualifier like `x-euro2`, ignored).
                    let (sym, id) = rest.split_once('-').unwrap_or((rest, ""));
                    if !sym.is_empty() {
                        push_lit(&mut toks, sym);
                    }
                    if let Ok(v) = u32::from_str_radix(id, 16) {
                        lcid = Some(v);
                        // Bits 16–23: calendar identifier (Win32 CALID).
                        if (v >> 16) & 0xFF != 0 {
                            cal = Some(calendar::Calendar::from_tag(v, locale_hint.lcid));
                        }
                    }
                } else if let Some(n) = lower
                    .strip_prefix("dbnum")
                    .and_then(|n| n.parse::<u8>().ok())
                    .filter(|n| (1..=4).contains(n))
                {
                    dbnum = Some(n);
                } else if !lower.is_empty() && lower.chars().all(|c| c == 'h') {
                    toks.push(Tok::Elapsed(Elapsed::Hours, lower.len() as u8));
                } else if !lower.is_empty() && lower.chars().all(|c| c == 'm') {
                    toks.push(Tok::Elapsed(Elapsed::Minutes, lower.len() as u8));
                } else if !lower.is_empty() && lower.chars().all(|c| c == 's') {
                    toks.push(Tok::Elapsed(Elapsed::Seconds, lower.len() as u8));
                }
                // Other tags ([natnum], …) are ignored.
            }
            '0' => {
                toks.push(Tok::Digit(Digit::Zero));
                i += 1;
            }
            '#' => {
                toks.push(Tok::Digit(Digit::Hash));
                i += 1;
            }
            '?' => {
                toks.push(Tok::Digit(Digit::Question));
                i += 1;
            }
            '.' => {
                toks.push(Tok::Point);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '%' => {
                toks.push(Tok::Percent);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '@' => {
                toks.push(Tok::At);
                i += 1;
            }
            'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => {
                toks.push(Tok::Exp {
                    plus: chars[i + 1] == '+',
                    upper: c == 'E',
                });
                i += 2;
            }
            _ if chars[i..].len() >= 7
                && chars[i..i + 7]
                    .iter()
                    .collect::<String>()
                    .eq_ignore_ascii_case("general") =>
            {
                toks.push(Tok::General);
                i += 7;
            }
            'A' | 'a'
                if chars[i..].len() >= 5
                    && chars[i..i + 5]
                        .iter()
                        .collect::<String>()
                        .eq_ignore_ascii_case("am/pm") =>
            {
                toks.push(Tok::AmPm {
                    short: false,
                    lower: c == 'a',
                });
                i += 5;
            }
            'A' | 'a'
                if chars[i..].len() >= 3
                    && chars[i..i + 3]
                        .iter()
                        .collect::<String>()
                        .eq_ignore_ascii_case("a/p") =>
            {
                toks.push(Tok::AmPm {
                    short: true,
                    lower: c == 'a',
                });
                i += 3;
            }
            'y' | 'Y' | 'e' | 'E' => {
                let n = if c.eq_ignore_ascii_case(&'e') {
                    1
                } else {
                    run(i, 'y')
                };
                toks.push(Tok::Year(if c.eq_ignore_ascii_case(&'e') {
                    4
                } else {
                    n as u8
                }));
                i += n;
            }
            // `B1` / `B2`: Gregorian / Hijri calendar for the section.
            'b' | 'B' if matches!(chars.get(i + 1), Some('1' | '2')) => {
                cal = Some(if chars[i + 1] == '2' {
                    calendar::Calendar::Hijri
                } else {
                    calendar::Calendar::Gregorian
                });
                i += 2;
            }
            'g' | 'G' => {
                let n = run(i, 'g');
                toks.push(Tok::Era(n.min(3) as u8));
                i += n;
            }
            'b' | 'B' if run(i, 'b') >= 2 => {
                let n = run(i, 'b');
                toks.push(Tok::Buddhist(n as u8));
                i += n;
            }
            'm' | 'M' => {
                let n = run(i, 'm');
                toks.push(Tok::Month(n as u8));
                i += n;
            }
            'd' | 'D' => {
                let n = run(i, 'd');
                toks.push(Tok::Day(n as u8));
                i += n;
            }
            'h' | 'H' => {
                let n = run(i, 'h');
                toks.push(Tok::Hour(n as u8));
                i += n;
            }
            's' | 'S' => {
                let n = run(i, 's');
                toks.push(Tok::Second(n as u8));
                i += n;
            }
            _ => {
                push_lit(&mut toks, &c.to_string());
                i += 1;
            }
        }
    }

    // System long date / time tags replace the section (§18.8.31 locale tags).
    if let Some(id) = lcid {
        let sys = match id & 0xFFFF {
            0xF800 => Some(locale_hint.long_date),
            0xF400 => Some(locale_hint.long_time),
            _ => None,
        };
        if let Some(code) = sys {
            let mut s = parse_section(code, locale_hint);
            s.color = color;
            s.cond = cond;
            return s;
        }
    }

    let is_date = toks.iter().any(Tok::is_date);
    if !is_date {
        // Era letters only mean something next to date parts.
        for t in &mut toks {
            if let Tok::Era(n) = *t {
                *t = Tok::Lit("g".repeat(usize::from(n)));
            }
        }
    }
    let kind = if is_date {
        resolve_date_tokens(&mut toks);
        Kind::Date
    } else if toks.contains(&Tok::At) {
        Kind::Text
    } else if toks.contains(&Tok::General) {
        Kind::General
    } else {
        Kind::Number
    };
    Section {
        toks,
        color,
        cond,
        lcid,
        calendar: cal.unwrap_or_default(),
        dbnum,
        kind,
    }
}

/// Date-section post-processing: `m` after hours / before seconds means minutes;
/// `.0…` after seconds is a fraction of a second; `/` and digits are literals.
fn resolve_date_tokens(toks: &mut Vec<Tok>) {
    // Fractional seconds.
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        if toks[i] == Tok::Point
            && matches!(
                out.last(),
                Some(Tok::Second(_) | Tok::Elapsed(Elapsed::Seconds, _))
            )
        {
            let n = toks[i + 1..]
                .iter()
                .take_while(|t| **t == Tok::Digit(Digit::Zero))
                .count();
            if n > 0 {
                out.push(Tok::SubSec(n.min(3) as u8));
                i += 1 + n;
                continue;
            }
        }
        out.push(toks[i].clone());
        i += 1;
    }
    // Literals for everything that is not a date part.
    for t in &mut out {
        let lit = match t {
            Tok::Slash => Some("/".to_owned()),
            Tok::Point => Some(".".to_owned()),
            Tok::Comma => Some(",".to_owned()),
            Tok::Percent => Some("%".to_owned()),
            Tok::Digit(Digit::Zero) => Some("0".to_owned()),
            Tok::Digit(Digit::Hash) => Some("#".to_owned()),
            Tok::Digit(Digit::Question) => Some("?".to_owned()),
            _ => None,
        };
        if let Some(l) = lit {
            *t = Tok::Lit(l);
        }
    }
    // Month vs minute.
    let idx: Vec<usize> = (0..out.len()).filter(|&i| out[i].is_date()).collect();
    for (k, &i) in idx.iter().enumerate() {
        if let Tok::Month(n) = out[i] {
            if n > 2 {
                continue;
            }
            let prev_hour = k > 0
                && matches!(
                    out[idx[k - 1]],
                    Tok::Hour(_) | Tok::Elapsed(Elapsed::Hours, _)
                );
            let next_sec = idx.get(k + 1).is_some_and(|&j| {
                matches!(out[j], Tok::Second(_) | Tok::Elapsed(Elapsed::Seconds, _))
            });
            if prev_hour || next_sec {
                out[i] = Tok::Minute(n);
            }
        }
    }
    *toks = out;
}

impl NumberFormat {
    /// Parse a format code. Parsing is lenient: unknown characters are literals.
    pub fn parse(code: &str) -> NumberFormat {
        Self::parse_with(code, &Locale::default())
    }

    /// Parse with a locale used for system date/time tags (`[$-F800]`).
    pub fn parse_with(code: &str, locale: &Locale) -> NumberFormat {
        let code = if code.trim().is_empty() {
            "General"
        } else {
            code
        };
        let sections = split_sections(code)
            .iter()
            .take(4)
            .map(|s| parse_section(s, locale))
            .collect();
        NumberFormat { sections }
    }

    /// Is this a date/time format (first section)?
    pub fn is_date(&self) -> bool {
        self.sections.first().is_some_and(|s| s.kind == Kind::Date)
    }

    /// Format a cell value.
    pub fn format(&self, value: &CellValue, opts: &FormatOptions) -> Formatted {
        match value {
            CellValue::Empty => Formatted::default(),
            CellValue::Number(n) => self.format_number(*n, opts),
            CellValue::Text(_) | CellValue::RichText(_) => {
                self.format_text(&value.as_text().unwrap_or_default(), opts)
            }
            CellValue::Bool(b) => Formatted::plain(
                if *b {
                    opts.locale.true_text
                } else {
                    opts.locale.false_text
                }
                .to_owned(),
            ),
            CellValue::Error(e) => Formatted::plain(e.as_str().to_owned()),
            CellValue::Date(d) => Formatted::plain(d.clone()),
        }
    }

    /// Format a text value: the text section (4th, or any section with `@`).
    pub fn format_text(&self, text: &str, opts: &FormatOptions) -> Formatted {
        let section = self
            .sections
            .get(3)
            .or_else(|| self.sections.iter().find(|s| s.kind == Kind::Text));
        match section {
            Some(s) => render_text(s, text, opts),
            None => Formatted::plain(text.to_owned()),
        }
    }

    /// Select the section for a number: returns (section, show minus sign).
    fn pick(&self, v: f64) -> Option<(&Section, bool)> {
        let num: Vec<&Section> = self
            .sections
            .iter()
            .enumerate()
            .filter(|(i, s)| !(s.kind == Kind::Text && (*i == 3 || self.sections.len() == 1)))
            .map(|(_, s)| s)
            .take(3)
            .collect();
        let first = *num.first()?;
        let has_cond = num.iter().any(|s| s.cond.is_some());
        if !has_cond {
            return Some(match num.len() {
                1 => (first, true),
                2 => {
                    if v >= 0.0 {
                        (first, false)
                    } else {
                        (num[1], false)
                    }
                }
                _ => {
                    if v > 0.0 {
                        (first, false)
                    } else if v < 0.0 {
                        (num[1], false)
                    } else {
                        (num[2], false)
                    }
                }
            });
        }
        // Conditional sections: explicit conditions are tested in order; a
        // section chosen by an explicit condition shows the value's own sign.
        let c0 = first.cond.unwrap_or(Condition {
            op: Op::Gt,
            value: 0.0,
        });
        if c0.matches(v) {
            return Some((first, true));
        }
        if let Some(s1) = num.get(1) {
            match s1.cond {
                Some(c) if c.matches(v) => return Some((s1, true)),
                None if num.len() == 2 => return Some((s1, true)),
                None if v < 0.0 => return Some((s1, false)),
                Some(_) | None => {}
            }
        }
        if let Some(s2) = num.get(2) {
            return Some((s2, true));
        }
        None
    }

    /// Format a number.
    pub fn format_number(&self, v: f64, opts: &FormatOptions) -> Formatted {
        if !v.is_finite() {
            return Formatted::plain("#NUM!".into());
        }
        // A text-only code (`@`) shows numbers as General.
        if self.sections.iter().all(|s| s.kind == Kind::Text) {
            return Formatted::plain(format_general(v, &opts.locale));
        }
        let Some((section, signed)) = self.pick(v) else {
            return Formatted {
                text: "#".repeat(8),
                invalid: true,
                ..Formatted::default()
            };
        };
        let neg = v < 0.0 && signed;
        let mut out = match section.kind {
            Kind::General => render_general_section(section, v.abs(), opts),
            Kind::Number | Kind::Text => render_number(section, v.abs(), opts),
            Kind::Date => {
                if v < 0.0 {
                    return Formatted {
                        text: "#".repeat(8),
                        invalid: true,
                        color: section.color,
                        ..Formatted::default()
                    };
                }
                render_date(section, v, opts)
            }
        };
        if neg
            && ((!out.invalid && out.text.chars().any(|c| c.is_ascii_digit() && c != '0'))
                || section.kind == Kind::General)
        {
            out.prepend("-");
        }
        if let (Some(style), false) = (section.dbnum, out.invalid) {
            out = apply_dbnum(&out, section, style, opts);
        }
        out.color = section.color;
        out
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Rewrite the digits of a formatted value in the section's `[DBNum]` style,
/// keeping the fill / skip markers on the same characters.
fn apply_dbnum(out: &Formatted, s: &Section, style: u8, opts: &FormatOptions) -> Formatted {
    let lcid = s.lcid.unwrap_or(opts.locale.lcid);
    let date = s.kind == Kind::Date;
    let conv = |t: &str| {
        dbnum::apply(
            t,
            style,
            lcid,
            date,
            opts.locale.decimal,
            opts.locale.thousands,
        )
    };
    // Split at the marker offsets so the markers survive the conversion.
    let mut cuts: Vec<usize> = out.skips.iter().map(|(p, _)| *p).collect();
    if let Some((p, _)) = out.fill {
        cuts.push(p);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut res = Formatted {
        invalid: out.invalid,
        color: out.color,
        ..Formatted::default()
    };
    let mut start = 0;
    for cut in cuts {
        if cut < start || cut > out.text.len() || !out.text.is_char_boundary(cut) {
            continue;
        }
        res.text.push_str(&conv(&out.text[start..cut]));
        if let Some((_, c)) = out.fill.filter(|(p, _)| *p == cut) {
            res.fill = Some((res.text.len(), c));
        }
        if let Some((_, c)) = out.skips.iter().find(|(p, _)| *p == cut) {
            res.skips.push((res.text.len(), *c));
            res.text.push(' ');
            start = cut + 1;
        } else {
            start = cut;
        }
    }
    res.text
        .push_str(&conv(&out.text[start.min(out.text.len())..]));
    res
}

fn render_text(s: &Section, text: &str, _opts: &FormatOptions) -> Formatted {
    let mut out = Formatted {
        color: s.color,
        ..Formatted::default()
    };
    for t in &s.toks {
        match t {
            Tok::Lit(l) => out.text.push_str(l),
            Tok::At | Tok::General => out.text.push_str(text),
            Tok::Skip(c) => out.skip(*c),
            Tok::Fill(c) => out.fill_at(*c),
            _ => {}
        }
    }
    out
}

fn render_general_section(s: &Section, v: f64, opts: &FormatOptions) -> Formatted {
    let mut out = Formatted::default();
    for t in &s.toks {
        match t {
            Tok::General => out.text.push_str(&format_general(v, &opts.locale)),
            Tok::Lit(l) => out.text.push_str(l),
            Tok::Skip(c) => out.skip(*c),
            Tok::Fill(c) => out.fill_at(*c),
            _ => {}
        }
    }
    out
}

/// Decimal digits of `|v|` rounded half away from zero to `places` decimals,
/// after first rounding to 15 significant digits (spreadsheet precision).
/// Returns (integer digits without leading zeros, exactly `places` fraction digits).
fn round_decimal(v: f64, places: usize) -> (String, String) {
    let v = v.abs();
    if v == 0.0 {
        return (String::new(), "0".repeat(places));
    }
    let s = format!("{v:.14e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i64 = exp.parse().unwrap_or(0);
    let mut digits: Vec<u8> = mant
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    let mut point = exp + 1; // number of integer digits
    let cut = point + places as i64;
    if cut < 0 {
        return (String::new(), "0".repeat(places));
    }
    let cut_u = cut as usize;
    if cut_u < digits.len() {
        let round_up = digits[cut_u] >= 5;
        digits.truncate(cut_u);
        if round_up {
            let mut i = cut_u;
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    // Digit at absolute position k (0 = first integer digit when point > 0).
    let get = |k: i64| -> char {
        if k < 0 || k as usize >= digits.len() {
            '0'
        } else {
            char::from(b'0' + digits[k as usize])
        }
    };
    let int: String = if point > 0 {
        (0..point).map(get).collect()
    } else {
        String::new()
    };
    let frac: String = (0..places as i64).map(|i| get(point + i)).collect();
    let int = int.trim_start_matches('0').to_owned();
    (int, frac)
}

/// `General` number format: up to 11 characters, scientific for very large or small values.
pub fn format_general(v: f64, locale: &Locale) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    let sci = format!("{a:.14e}");
    let exp: i64 = sci
        .split_once('e')
        .and_then(|(_, e)| e.parse().ok())
        .unwrap_or(0);
    let body = if (-10..11).contains(&exp) {
        let int_digits = (exp + 1).max(1);
        let decimals = (10 - int_digits).max(0) as usize;
        let (i, f) = round_decimal(a, decimals);
        let f = f.trim_end_matches('0');
        // Small magnitudes (below 1E-4) switch to scientific.
        if exp < -4 {
            general_sci(a, locale)
        } else {
            let mut s = if i.is_empty() { "0".to_owned() } else { i };
            if !f.is_empty() {
                s.push(locale.decimal);
                s.push_str(f);
            }
            s
        }
    } else {
        general_sci(a, locale)
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

fn general_sci(a: f64, locale: &Locale) -> String {
    let s = format!("{a:.5e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i64 = e.parse().unwrap_or(0);
    let m = m.trim_end_matches('0').trim_end_matches('.');
    let m = m.replace('.', &locale.decimal.to_string());
    format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// Numeric section layout.
struct Layout {
    int_ph: Vec<Digit>,
    frac_ph: Vec<Digit>,
    exp_ph: Vec<Digit>,
    grouping: bool,
    scale_commas: i32,
    percent: i32,
}

fn analyze(toks: &[Tok]) -> Layout {
    let point = toks.iter().position(|t| *t == Tok::Point);
    let exp = toks.iter().position(|t| matches!(t, Tok::Exp { .. }));
    let int_end = point.or(exp).unwrap_or(toks.len());
    let mut l = Layout {
        int_ph: Vec::new(),
        frac_ph: Vec::new(),
        exp_ph: Vec::new(),
        grouping: false,
        scale_commas: 0,
        percent: 0,
    };
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Digit(d) if i < int_end => l.int_ph.push(*d),
            Tok::Digit(d) if exp.is_some_and(|e| i > e) => l.exp_ph.push(*d),
            Tok::Digit(d) => l.frac_ph.push(*d),
            Tok::Percent => l.percent += 1,
            Tok::Comma => {
                let prev_digit = toks[..i]
                    .iter()
                    .rev()
                    .find(|t| **t != Tok::Comma)
                    .is_some_and(|t| matches!(t, Tok::Digit(_)));
                let next_digit = toks[i + 1..]
                    .iter()
                    .find(|t| **t != Tok::Comma)
                    .is_some_and(|t| matches!(t, Tok::Digit(_)));
                if prev_digit && next_digit && i < int_end {
                    l.grouping = true;
                } else if prev_digit && !next_digit {
                    l.scale_commas += 1;
                }
            }
            _ => {}
        }
    }
    l
}

/// Fill integer placeholders right-to-left. Returns the string for each placeholder.
fn fill_int(ph: &[Digit], digits: &str) -> Vec<String> {
    let d: Vec<char> = digits.chars().collect();
    let mut out = vec![String::new(); ph.len()];
    let mut k = d.len();
    for (slot, p) in ph.iter().enumerate().rev() {
        if k > 0 {
            k -= 1;
            out[slot].push(d[k]);
        } else {
            match p {
                Digit::Zero => out[slot].push('0'),
                Digit::Question => out[slot].push(' '),
                Digit::Hash => {}
            }
        }
    }
    if k > 0 {
        if let Some(first) = out.first_mut() {
            let extra: String = d[..k].iter().collect();
            first.insert_str(0, &extra);
        }
    }
    out
}

/// Fill fraction placeholders left-to-right, then trim trailing zeros for `#` / `?`.
fn fill_frac(ph: &[Digit], digits: &str) -> Vec<String> {
    let d: Vec<char> = digits.chars().collect();
    let mut out: Vec<String> = ph
        .iter()
        .enumerate()
        .map(|(i, _)| d.get(i).copied().unwrap_or('0').to_string())
        .collect();
    for i in (0..ph.len()).rev() {
        if out[i] != "0" {
            break;
        }
        match ph[i] {
            Digit::Zero => break,
            Digit::Hash => out[i].clear(),
            Digit::Question => out[i] = " ".into(),
        }
    }
    out
}

fn group(int_text: &str, sep: char) -> String {
    // Group only the digit run; keep leading spaces from `?`.
    let lead = int_text.len() - int_text.trim_start().len();
    let digits = &int_text[lead..];
    let n = digits.chars().count();
    let mut out = int_text[..lead].to_owned();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) && c.is_ascii_digit() {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn render_number(s: &Section, v: f64, opts: &FormatOptions) -> Formatted {
    if s.toks.contains(&Tok::Slash) && is_fraction(&s.toks) {
        return render_fraction(s, v, opts);
    }
    let l = analyze(&s.toks);
    let mut v = v;
    for _ in 0..l.percent {
        v *= 100.0;
    }
    for _ in 0..l.scale_commas {
        v /= 1000.0;
    }
    let places = l.frac_ph.len();
    let exp_tok = s.toks.iter().find_map(|t| match t {
        Tok::Exp { plus, upper } => Some((*plus, *upper)),
        _ => None,
    });
    let (int_digits, frac_digits, exponent) = if exp_tok.is_some() {
        sci_parts(v, &l)
    } else {
        let (i, f) = round_decimal(v, places);
        (i, f, 0)
    };
    let mut ints = fill_int(&l.int_ph, &int_digits);
    if l.grouping {
        let joined: String = ints.concat();
        let grouped = group(&joined, opts.locale.thousands);
        for (k, slot) in ints.iter_mut().enumerate() {
            *slot = if k == 0 {
                grouped.clone()
            } else {
                String::new()
            };
        }
    }
    let fracs = fill_frac(&l.frac_ph, &frac_digits);
    let exp_digits = {
        let e = exponent.unsigned_abs().to_string();
        fill_int(&l.exp_ph, &e)
    };

    let mut out = Formatted::default();
    let (mut ii, mut fi, mut ei) = (0, 0, 0);
    let mut seen_point = false;
    let mut seen_exp = false;
    let has_int_ph = !l.int_ph.is_empty();
    for t in &s.toks {
        match t {
            Tok::Digit(_) if seen_exp => {
                out.text
                    .push_str(exp_digits.get(ei).map_or("", String::as_str));
                ei += 1;
            }
            Tok::Digit(_) if seen_point => {
                out.text.push_str(fracs.get(fi).map_or("", String::as_str));
                fi += 1;
            }
            Tok::Digit(_) => {
                out.text.push_str(ints.get(ii).map_or("", String::as_str));
                ii += 1;
            }
            Tok::Point => {
                if seen_point {
                    out.text.push('.');
                } else {
                    if !has_int_ph {
                        out.text.push_str(&int_digits);
                    }
                    out.text.push(opts.locale.decimal);
                    seen_point = true;
                }
            }
            Tok::Exp { plus, upper } => {
                seen_exp = true;
                out.text.push(if *upper { 'E' } else { 'e' });
                if exponent < 0 {
                    out.text.push('-');
                } else if *plus {
                    out.text.push('+');
                }
            }
            Tok::Percent => out.text.push('%'),
            Tok::Lit(x) => out.text.push_str(x),
            Tok::Skip(c) => out.skip(*c),
            Tok::Fill(c) => out.fill_at(*c),
            Tok::Slash => out.text.push('/'),
            // Comma: grouping / scaling handled above; @ / General: no text here.
            _ => {}
        }
    }
    let _ = exp_tok;
    out
}

/// Scientific notation parts: (mantissa integer digits, mantissa fraction digits, exponent).
fn sci_parts(v: f64, l: &Layout) -> (String, String, i32) {
    let places = l.frac_ph.len();
    if v == 0.0 {
        let (i, f) = round_decimal(0.0, places);
        return (i, f, 0);
    }
    let e10: i32 = format!("{v:e}")
        .split_once('e')
        .and_then(|(_, e)| e.parse().ok())
        .unwrap_or(0);
    let ip = l.int_ph.len().max(1) as i32;
    let engineering = ip > 1 && l.int_ph.contains(&Digit::Hash);
    let mut exp = if engineering {
        e10 - e10.rem_euclid(ip)
    } else {
        e10 - (ip - 1)
    };
    for _ in 0..2 {
        let m = v / 10f64.powi(exp);
        let (i, f) = round_decimal(m, places);
        let limit = ip;
        if i.len() as i32 > limit {
            exp += if engineering { ip } else { 1 };
            continue;
        }
        return (i, f, exp);
    }
    let (i, f) = round_decimal(v / 10f64.powi(exp), places);
    (i, f, exp)
}

fn is_fraction(toks: &[Tok]) -> bool {
    let Some(p) = toks.iter().position(|t| *t == Tok::Slash) else {
        return false;
    };
    let before = p > 0 && matches!(toks[p - 1], Tok::Digit(_));
    let after = toks.get(p + 1).is_some_and(|t| match t {
        Tok::Digit(_) => true,
        Tok::Lit(l) => l.starts_with(|c: char| c.is_ascii_digit()),
        _ => false,
    });
    before && after
}

fn render_fraction(s: &Section, v: f64, opts: &FormatOptions) -> Formatted {
    let toks = &s.toks;
    let slash = toks.iter().position(|t| *t == Tok::Slash).unwrap_or(0);
    // Numerator: digit run immediately before the slash.
    let mut num_start = slash;
    while num_start > 0 && matches!(toks[num_start - 1], Tok::Digit(_)) {
        num_start -= 1;
    }
    let num_ph: Vec<Digit> = toks[num_start..slash]
        .iter()
        .filter_map(|t| match t {
            Tok::Digit(d) => Some(*d),
            _ => None,
        })
        .collect();
    let int_ph: Vec<Digit> = toks[..num_start]
        .iter()
        .filter_map(|t| match t {
            Tok::Digit(d) => Some(*d),
            _ => None,
        })
        .collect();
    // Denominator: placeholders or a fixed number.
    let mut den_end = slash + 1;
    let mut den_ph: Vec<Digit> = Vec::new();
    let mut fixed: Option<u64> = None;
    let mut den_tail = String::new();
    match toks.get(slash + 1) {
        Some(Tok::Lit(l)) => {
            let digits: String = l.chars().take_while(char::is_ascii_digit).collect();
            fixed = digits.parse().ok().filter(|d| *d > 0);
            den_tail = l[digits.len()..].to_owned();
            den_end = slash + 2;
        }
        _ => {
            while let Some(Tok::Digit(d)) = toks.get(den_end) {
                den_ph.push(*d);
                den_end += 1;
            }
        }
    }
    let has_int = !int_ph.is_empty();
    let mut whole = if has_int { v.trunc() } else { 0.0 };
    let frac = v - whole;
    let (mut num, den) = if let Some(d) = fixed {
        ((frac * d as f64).round() as u64, d)
    } else {
        let max_den = 10u64.pow(den_ph.len().clamp(1, 5) as u32) - 1;
        best_fraction(frac, max_den.max(1))
    };
    if has_int && num == den {
        whole += 1.0;
        num = 0;
    }
    if !has_int {
        // Improper fraction: whole part folded into the numerator.
        num += (whole as u64) * den;
    }

    let mut out = Formatted::default();
    let whole_digits = if whole == 0.0 {
        String::new()
    } else {
        format!("{whole:.0}")
    };
    let int_text = fill_int(&int_ph, &whole_digits);
    let mut int_text = int_text;
    if l_grouping(toks, num_start) {
        let joined = int_text.concat();
        let g = group(&joined, opts.locale.thousands);
        for (k, s) in int_text.iter_mut().enumerate() {
            *s = if k == 0 { g.clone() } else { String::new() };
        }
    }
    let int_empty = int_text.iter().all(|s| s.trim().is_empty());
    let blank_fraction = has_int && num == 0;
    let num_text = {
        let s = num.to_string();
        let w = num_ph.len();
        let pad = if num_ph.contains(&Digit::Zero) {
            '0'
        } else {
            ' '
        };
        if s.len() < w && !num_ph.iter().all(|d| *d == Digit::Hash) {
            format!("{}{s}", pad.to_string().repeat(w - s.len()))
        } else {
            s
        }
    };
    let den_text = if fixed.is_some() {
        format!("{den}{den_tail}")
    } else {
        let s = den.to_string();
        let w = den_ph.len();
        if s.len() < w && den_ph.contains(&Digit::Question) {
            format!("{s}{}", " ".repeat(w - s.len()))
        } else {
            s
        }
    };
    let blank_width = num_text.chars().count() + 1 + den_text.chars().count();
    let any_question = num_ph.contains(&Digit::Question) || den_ph.contains(&Digit::Question);

    let mut ii = 0;
    for (i, t) in toks.iter().enumerate() {
        if i >= num_start && i < den_end {
            if i == num_start {
                if blank_fraction {
                    if any_question {
                        out.text.push_str(&" ".repeat(blank_width));
                    }
                } else {
                    out.text.push_str(&num_text);
                    out.text.push('/');
                    out.text.push_str(&den_text);
                }
            }
            continue;
        }
        match t {
            Tok::Digit(_) => {
                let s = int_text.get(ii).map_or("", String::as_str);
                ii += 1;
                if blank_fraction && whole == 0.0 && int_empty && ii == int_ph.len() {
                    out.text.push('0');
                } else {
                    out.text.push_str(s);
                }
            }
            // Separator literals between an empty integer part and the numerator are dropped.
            Tok::Lit(l) if i < num_start && int_empty && !blank_fraction && has_int => {
                let _ = l;
            }
            Tok::Lit(l) => out.text.push_str(l),
            Tok::Skip(c) => out.skip(*c),
            Tok::Fill(c) => out.fill_at(*c),
            Tok::Percent => out.text.push('%'),
            _ => {}
        }
    }
    out
}

fn l_grouping(toks: &[Tok], end: usize) -> bool {
    toks[..end].windows(3).any(|w| {
        matches!(w[0], Tok::Digit(_)) && w[1] == Tok::Comma && matches!(w[2], Tok::Digit(_))
    })
}

/// Closest fraction n/d to `x` (0 ≤ x < 1) with d ≤ `max_den`; ties prefer the smaller d.
fn best_fraction(x: f64, max_den: u64) -> (u64, u64) {
    let mut best = (0u64, 1u64);
    let mut best_err = f64::INFINITY;
    for d in 1..=max_den {
        let n = (x * d as f64).round();
        let err = (x - n / d as f64).abs();
        if err < best_err - 1e-12 {
            best_err = err;
            best = (n as u64, d);
            if err == 0.0 {
                break;
            }
        }
    }
    best
}

fn render_date(s: &Section, v: f64, opts: &FormatOptions) -> Formatted {
    let sub = s
        .toks
        .iter()
        .find_map(|t| match t {
            Tok::SubSec(n) => Some(u32::from(*n)),
            _ => None,
        })
        .unwrap_or(0);
    let Some(dt) = serial_to_datetime(v, opts.date1904, sub) else {
        return Formatted {
            text: "#".repeat(8),
            invalid: true,
            ..Formatted::default()
        };
    };
    let names = s.lcid.and_then(Locale::from_lcid).unwrap_or(opts.locale);
    let twelve = s.toks.iter().any(|t| matches!(t, Tok::AmPm { .. }));
    let scale = 10f64.powi(sub as i32);
    let total_units = (v * 86_400.0 * scale).round();
    let total_secs = (total_units / scale).floor();
    let mut out = Formatted::default();
    let pad = |n: u64, w: u8| -> String {
        if w >= 2 {
            format!("{n:02}")
        } else {
            n.to_string()
        }
    };
    let elapsed_present = s.toks.iter().any(|t| matches!(t, Tok::Elapsed(..)));
    // Year / month / day in the section's calendar (weekday and time are shared).
    let cd = s.calendar.convert(dt.year, dt.month, dt.day);
    let vi = s
        .lcid
        .filter(|l| l & 0xFFFF != 0)
        .unwrap_or(opts.locale.lcid)
        & 0xFFFF
        == 0x42A;
    for t in &s.toks {
        match t {
            Tok::Year(n) | Tok::Buddhist(n) => {
                let y = if matches!(t, Tok::Buddhist(_)) {
                    dt.year + 543
                } else {
                    cd.year
                };
                if *n <= 2 {
                    out.text.push_str(&format!("{:02}", y.rem_euclid(100)));
                } else {
                    out.text.push_str(&format!("{y:04}"));
                }
            }
            Tok::Month(n) if !matches!(s.calendar, Calendar::Gregorian | Calendar::Thai) => {
                if *n <= 2 {
                    out.text.push_str(&pad(u64::from(cd.month), *n));
                } else {
                    out.text
                        .push_str(&calendar_month_name(s.calendar, cd, *n, vi));
                }
            }
            Tok::Month(n) => {
                let m = dt.month as usize - 1;
                match n {
                    1 | 2 => out.text.push_str(&pad(u64::from(dt.month), *n)),
                    3 => out.text.push_str(names.months_short[m]),
                    5 => out
                        .text
                        .push_str(&names.months[m].chars().take(1).collect::<String>()),
                    _ => out.text.push_str(names.months[m]),
                }
            }
            Tok::Day(n) => {
                let w = dt.weekday as usize;
                match n {
                    1 | 2 => out.text.push_str(&pad(u64::from(cd.day), *n)),
                    3 => out.text.push_str(names.days_short[w]),
                    _ => out.text.push_str(names.days[w]),
                }
            }
            Tok::Hour(n) => {
                let h = if twelve {
                    match dt.hour % 12 {
                        0 => 12,
                        h => h,
                    }
                } else {
                    dt.hour
                };
                out.text.push_str(&pad(u64::from(h), *n));
            }
            Tok::Minute(n) => {
                let m = if elapsed_present {
                    ((total_secs / 60.0) as u64) % 60
                } else {
                    u64::from(dt.minute)
                };
                out.text.push_str(&pad(m, *n));
            }
            Tok::Second(n) => out.text.push_str(&pad(u64::from(dt.second), *n)),
            Tok::SubSec(n) => {
                out.text.push(opts.locale.decimal);
                out.text
                    .push_str(&format!("{:0w$}", dt.subsec, w = usize::from(*n)));
            }
            Tok::AmPm { short, lower } => {
                let pm = dt.hour >= 12;
                if *short {
                    let c = if pm { 'P' } else { 'A' };
                    out.text
                        .push(if *lower { c.to_ascii_lowercase() } else { c });
                } else {
                    out.text.push_str(if pm { names.pm } else { names.am });
                }
            }
            Tok::Elapsed(unit, n) => {
                let val = match unit {
                    Elapsed::Hours => (total_secs / 3600.0).floor() as u64,
                    Elapsed::Minutes => (total_secs / 60.0).floor() as u64,
                    Elapsed::Seconds => total_secs as u64,
                };
                out.text.push_str(&pad(val, *n));
            }
            Tok::Era(_) => match s.calendar {
                Calendar::Lunar { .. } => {
                    out.text.push_str(&calendar::sexagenary_year(cd.year, vi));
                }
                Calendar::Hijri => out.text.push_str("AH"),
                Calendar::Gregorian | Calendar::Thai => {}
            },
            Tok::Lit(l) => out.text.push_str(l),
            Tok::Skip(c) => out.skip(*c),
            Tok::Fill(c) => out.fill_at(*c),
            _ => {}
        }
    }
    out
}

/// `mmm` / `mmmm` / `mmmmm` month text in a Hijri or lunisolar section (own wording:
/// Vietnamese "Giêng … Chạp" with "nhuận" for a leap month, otherwise 正月 … 十二月
/// with the 閏 prefix; Hijri months use their transliterated names).
fn calendar_month_name(cal: Calendar, cd: calendar::CalDate, n: u8, vi: bool) -> String {
    let name = match cal {
        Calendar::Hijri => calendar::hijri_month_name(cd.month).to_owned(),
        _ if vi => {
            let base = calendar::lunar_month_name_vi(cd.month);
            let leap = if cd.leap { " nhuận" } else { "" };
            if n >= 4 {
                format!("tháng {base}{leap}")
            } else {
                format!("{base}{leap}")
            }
        }
        _ => {
            const CN: [&str; 12] = [
                "正", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一", "十二",
            ];
            let leap = if cd.leap { "閏" } else { "" };
            format!("{leap}{}月", CN[(cd.month.clamp(1, 12) - 1) as usize])
        }
    };
    if n == 5 {
        name.chars().take(1).collect()
    } else {
        name
    }
}

/// Format `value` with format `code`.
pub fn format_value(value: &CellValue, code: &str, opts: &FormatOptions) -> Formatted {
    NumberFormat::parse_with(code, &opts.locale).format(value, opts)
}

/// Format a number with format `code`.
pub fn format_number(v: f64, code: &str, opts: &FormatOptions) -> Formatted {
    NumberFormat::parse_with(code, &opts.locale).format_number(v, opts)
}

/// Renderer for the `TEXT` worksheet function (ECMA-376 Part 1, §18.17.7 TEXT
/// with §18.8.31 format codes), installed into the formula engine as its
/// [`bo_formula::TextFormatter`].
///
/// Format codes inside formulas use the invariant separators (`.` decimal,
/// `,` thousands), so the en-US locale supplies separators and default names;
/// a locale tag in the code (`[$-42A]`) still switches month/day names.
/// Colours and `*x` fills have no meaning in text and are dropped; a value the
/// code cannot show (e.g. a negative date) returns `None` (`TEXT` → `#VALUE!`).
pub fn text_function_format(
    input: bo_formula::FormatInput<'_>,
    code: &str,
    system: bo_formula::DateSystem,
) -> Option<String> {
    let opts = FormatOptions {
        locale: Locale::EN_US,
        date1904: system == bo_formula::DateSystem::Date1904,
    };
    let fmt = NumberFormat::parse_with(code, &opts.locale);
    let out = match input {
        bo_formula::FormatInput::Number(n) => fmt.format_number(n, &opts),
        bo_formula::FormatInput::Text(s) => fmt.format_text(s, &opts),
    };
    (!out.invalid).then_some(out.text)
}

/// Format code for a number format id: custom formats from the stylesheet win,
/// then the built-in table, then `General`.
pub fn format_code(
    styles: &bo_model::sheet::Stylesheet,
    num_fmt_id: u32,
    locale: &Locale,
) -> String {
    styles
        .custom_num_fmt(num_fmt_id)
        .map(str::to_owned)
        .or_else(|| builtin_format(num_fmt_id, locale))
        .unwrap_or_else(|| "General".into())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_calendar;

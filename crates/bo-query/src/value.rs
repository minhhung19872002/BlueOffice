//! Values, column types and the table a query evaluates to.

use bo_formula::date::{serial_to_ymd, ymd_to_serial};
use bo_formula::DateSystem;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// One value of a query table.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Val {
    /// No value.
    #[default]
    Null,
    /// A number.
    Num(f64),
    /// Text.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// A date as a serial number of the 1900 date system (ECMA-376 Part 1, §18.17.4.1).
    Date(f64),
    /// A conversion or formula error (the text is shown in the cell, e.g. `#VALUE!`).
    Err(String),
}

impl Val {
    /// Text shown for the value (numbers with `.` as decimal separator, dates as ISO 8601).
    pub fn display(&self) -> String {
        match self {
            Val::Null => String::new(),
            Val::Num(n) => fmt_num(*n),
            Val::Text(s) | Val::Err(s) => s.clone(),
            Val::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            Val::Date(d) => fmt_date(*d),
        }
    }

    /// Is the value null or empty text?
    pub fn is_blank(&self) -> bool {
        match self {
            Val::Null => true,
            Val::Text(s) => s.is_empty(),
            _ => false,
        }
    }

    /// Numeric value, when the value is (or reads as) a number.
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Val::Num(n) | Val::Date(n) => Some(*n),
            Val::Bool(b) => Some(f64::from(u8::from(*b))),
            Val::Text(s) => parse_number(s, '.'),
            _ => None,
        }
    }

    /// Key used to compare values for grouping, joining and duplicate removal.
    pub fn key(&self) -> String {
        match self {
            Val::Null => "\u{0}".into(),
            Val::Num(n) | Val::Date(n) => format!("n{}", fmt_num(*n)),
            Val::Text(s) => format!("t{s}"),
            Val::Bool(b) => format!("b{b}"),
            Val::Err(e) => format!("e{e}"),
        }
    }

    /// Sort order: numbers and dates, then text (case-insensitive), then booleans, then
    /// errors; nulls last.
    pub fn compare(&self, other: &Val) -> Ordering {
        fn rank(v: &Val) -> u8 {
            match v {
                Val::Num(_) | Val::Date(_) => 0,
                Val::Text(_) => 1,
                Val::Bool(_) => 2,
                Val::Err(_) => 3,
                Val::Null => 4,
            }
        }
        match (self, other) {
            (Val::Num(a) | Val::Date(a), Val::Num(b) | Val::Date(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (Val::Text(a), Val::Text(b)) => a
                .to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b)),
            (Val::Bool(a), Val::Bool(b)) => a.cmp(b),
            (Val::Err(a), Val::Err(b)) => a.cmp(b),
            _ => rank(self).cmp(&rank(other)),
        }
    }

    /// JSON form for previews: `null`, a number, a string or a boolean.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Val::Null => serde_json::Value::Null,
            Val::Num(n) => serde_json::Number::from_f64(*n)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            Val::Bool(b) => serde_json::Value::Bool(*b),
            Val::Text(_) | Val::Date(_) | Val::Err(_) => serde_json::Value::String(self.display()),
        }
    }
}

/// Number text without a trailing `.0` (`3`, `2.5`, `-0.125`).
pub fn fmt_num(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// ISO 8601 text of a 1900-system serial (`2024-01-31`, with the time when there is one).
pub fn fmt_date(serial: f64) -> String {
    let Some((y, m, d)) = serial_to_ymd(serial.floor(), DateSystem::Date1900) else {
        return fmt_num(serial);
    };
    let frac = serial - serial.floor();
    if frac.abs() < 1e-9 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        let secs = (frac * 86_400.0).round() as u32;
        format!(
            "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
            secs / 3600,
            secs / 60 % 60,
            secs % 60
        )
    }
}

/// A number written with `decimal` as the decimal separator (`1234.5`, `1,234.5`, `-3`,
/// `12%`, `1e3`). Group separators (the other of `.`/`,`, spaces) are accepted.
pub fn parse_number(text: &str, decimal: char) -> Option<f64> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    let (s, pct) = match s.strip_suffix('%') {
        Some(r) => (r.trim_end(), true),
        None => (s, false),
    };
    let group = if decimal == ',' { '.' } else { ',' };
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == decimal {
            out.push('.');
        } else if c == group || c == ' ' || c == '\u{a0}' {
            // group separator
        } else if c.is_ascii_digit() || matches!(c, '+' | '-' | 'e' | 'E') {
            out.push(c);
        } else {
            return None;
        }
    }
    if !out.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: f64 = out.parse().ok()?;
    n.is_finite().then_some(if pct { n / 100.0 } else { n })
}

/// A date (and optional time) written as `yyyy-mm-dd`, `yyyy/mm/dd`, `dd/mm/yyyy` or
/// `dd-mm-yyyy` (day first when ambiguous unless `month_first`), optionally followed by
/// `hh:mm[:ss]` or an ISO `T` time; the 1900-system serial.
pub fn parse_date(text: &str, month_first: bool) -> Option<f64> {
    let s = text.trim();
    let (date, time) = match s.find(['T', ' ']) {
        Some(i) => (&s[..i], Some(s[i + 1..].trim())),
        None => (s, None),
    };
    let parts: Vec<&str> = date.split(['-', '/', '.']).collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let n: Vec<i64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
    if n.len() != 3 {
        return None;
    }
    let (y, m, d) = if parts[0].len() == 4 {
        (n[0], n[1], n[2])
    } else if parts[2].len() == 4 {
        if month_first {
            (n[2], n[0], n[1])
        } else {
            (n[2], n[1], n[0])
        }
    } else {
        return None;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut serial = ymd_to_serial(y, m as u32, d as u32, DateSystem::Date1900)?;
    if let Some(t) = time.filter(|t| !t.is_empty()) {
        let t = t.trim_end_matches('Z');
        let t = t.split(['+']).next().unwrap_or(t);
        let hms: Vec<&str> = t.split(':').collect();
        if hms.len() < 2 || hms.len() > 3 {
            return None;
        }
        let h: f64 = hms[0].parse().ok()?;
        let mi: f64 = hms[1].parse().ok()?;
        let se: f64 = hms.get(2).map_or(Some(0.0), |x| x.parse().ok())?;
        if !(0.0..24.0).contains(&h) || !(0.0..60.0).contains(&mi) || !(0.0..60.0).contains(&se) {
            return None;
        }
        serial += (h * 3600.0 + mi * 60.0 + se) / 86_400.0;
    }
    Some(serial)
}

/// The type of a column (the "change type" step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColType {
    /// Values as they are.
    #[default]
    Any,
    /// Text.
    Text,
    /// Decimal number.
    Number,
    /// Whole number.
    Integer,
    /// Date (and time).
    Date,
    /// TRUE / FALSE.
    Bool,
}

impl ColType {
    /// `value` converted to this type (`Val::Err` when it does not convert).
    pub fn convert(self, value: &Val, decimal: char, month_first: bool) -> Val {
        if matches!(value, Val::Null | Val::Err(_)) {
            return value.clone();
        }
        let fail = || Val::Err("#VALUE!".into());
        match self {
            ColType::Any => value.clone(),
            ColType::Text => match value {
                Val::Text(_) => value.clone(),
                _ => Val::Text(value.display()),
            },
            ColType::Number | ColType::Integer => {
                let n = match value {
                    Val::Text(s) if s.trim().is_empty() => return Val::Null,
                    Val::Text(s) => parse_number(s, decimal),
                    other => other.as_num(),
                };
                match n {
                    Some(n) if self == ColType::Integer => Val::Num(n.round()),
                    Some(n) => Val::Num(n),
                    None => fail(),
                }
            }
            ColType::Date => match value {
                Val::Date(_) => value.clone(),
                Val::Num(n) => Val::Date(*n),
                Val::Text(s) if s.trim().is_empty() => Val::Null,
                Val::Text(s) => parse_date(s, month_first).map_or_else(fail, Val::Date),
                _ => fail(),
            },
            ColType::Bool => match value {
                Val::Bool(_) => value.clone(),
                Val::Num(n) => Val::Bool(*n != 0.0),
                Val::Text(s) => match s.trim().to_lowercase().as_str() {
                    "true" | "yes" | "1" | "đúng" | "có" => Val::Bool(true),
                    "false" | "no" | "0" | "sai" | "không" => Val::Bool(false),
                    "" => Val::Null,
                    _ => fail(),
                },
                _ => fail(),
            },
        }
    }

    /// The type that fits every non-blank value of a column (text values read as numbers,
    /// dates or booleans count as those).
    pub fn detect<'a>(values: impl Iterator<Item = &'a Val>) -> ColType {
        let (mut num, mut int, mut date, mut boolean, mut seen) = (true, true, true, true, false);
        for v in values {
            if v.is_blank() {
                continue;
            }
            seen = true;
            let (n, d, b) = match v {
                Val::Num(n) => (Some(*n), None, false),
                Val::Date(d) => (None, Some(*d), false),
                Val::Bool(_) => (None, None, true),
                Val::Text(s) => (
                    parse_number(s, '.').filter(|_| looks_numeric(s)),
                    parse_date(s, false),
                    matches!(s.trim().to_lowercase().as_str(), "true" | "false"),
                ),
                _ => (None, None, false),
            };
            num &= n.is_some();
            int &= n.is_some_and(|n| n.fract() == 0.0);
            date &= d.is_some();
            boolean &= b;
        }
        if !seen {
            ColType::Any
        } else if int {
            ColType::Integer
        } else if num {
            ColType::Number
        } else if date {
            ColType::Date
        } else if boolean {
            ColType::Bool
        } else {
            ColType::Text
        }
    }
}

/// Text that reads as a plain number (no group separators, so `1,5` stays text for
/// type detection: it may be a decimal comma or two values).
fn looks_numeric(s: &str) -> bool {
    let s = s.trim();
    let digits = s.trim_start_matches(['-', '+']);
    !digits.is_empty()
        && !digits.starts_with('.')
        && digits
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '-' | '+'))
        // Leading zeros (codes, phone numbers) stay text.
        && (digits.len() <= 1 || !digits.starts_with('0') || digits.starts_with("0."))
}

/// The table a query evaluates to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QTable {
    /// Column names, unique.
    pub columns: Vec<String>,
    /// Column types (same length as `columns`).
    pub types: Vec<ColType>,
    /// Rows (each as long as `columns`).
    pub rows: Vec<Vec<Val>>,
}

impl QTable {
    /// A table with `columns` (types `Any`) and `rows`, padded or cut to the column count.
    pub fn new(columns: Vec<String>, rows: Vec<Vec<Val>>) -> QTable {
        let n = columns.len();
        let rows = rows
            .into_iter()
            .map(|mut r| {
                r.resize(n, Val::Null);
                r
            })
            .collect();
        QTable {
            types: vec![ColType::Any; n],
            columns: unique_names(columns),
            rows,
        }
    }

    /// A table of raw rows with generated column names `Column1`, `Column2`, ….
    pub fn from_rows(rows: Vec<Vec<Val>>) -> QTable {
        let n = rows.iter().map(Vec::len).max().unwrap_or(0);
        QTable::new((1..=n).map(|k| format!("Column{k}")).collect(), rows)
    }

    /// Index of a column (exact name first, then case-insensitive).
    pub fn col(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name).or_else(|| {
            let l = name.to_lowercase();
            self.columns.iter().position(|c| c.to_lowercase() == l)
        })
    }
}

/// `names` made unique (case-insensitive) by appending `_2`, `_3`, …; blanks become
/// `Column<n>`.
pub fn unique_names(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for (k, n) in names.into_iter().enumerate() {
        let base = if n.trim().is_empty() {
            format!("Column{}", k + 1)
        } else {
            n
        };
        let taken =
            |s: &str, out: &[String]| out.iter().any(|o| o.to_lowercase() == s.to_lowercase());
        let mut name = base.clone();
        let mut i = 2;
        while taken(&name, &out) {
            name = format!("{base}_{i}");
            i += 1;
        }
        out.push(name);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_dates_and_types() {
        assert_eq!(parse_number("1,234.5", '.'), Some(1234.5));
        assert_eq!(parse_number("1.234,5", ','), Some(1234.5));
        assert_eq!(parse_number("12%", '.'), Some(0.12));
        assert_eq!(parse_number("abc", '.'), None);
        assert_eq!(parse_date("2024-01-31", false), Some(45322.0));
        assert_eq!(parse_date("31/01/2024", false), Some(45322.0));
        assert_eq!(parse_date("01/31/2024", true), Some(45322.0));
        assert_eq!(parse_date("2024-01-31T12:00:00Z", false), Some(45322.5));
        assert_eq!(fmt_date(45322.0), "2024-01-31");
        let vals = [Val::Text("1".into()), Val::Text("2".into()), Val::Null];
        assert_eq!(ColType::detect(vals.iter()), ColType::Integer);
        let vals = [Val::Text("1.5".into()), Val::Text("2".into())];
        assert_eq!(ColType::detect(vals.iter()), ColType::Number);
        let vals = [Val::Text("007".into())];
        assert_eq!(ColType::detect(vals.iter()), ColType::Text);
        let vals = [Val::Text("2024-01-02".into())];
        assert_eq!(ColType::detect(vals.iter()), ColType::Date);
        assert_eq!(
            ColType::Number.convert(&Val::Text("x".into()), '.', false),
            Val::Err("#VALUE!".into())
        );
        assert_eq!(
            unique_names(vec!["a".into(), "A".into(), String::new()]),
            vec!["a", "A_2", "Column3"]
        );
    }
}

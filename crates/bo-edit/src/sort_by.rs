//! Sort Text by Number / Date (BlueDoc round 24): the selected paragraphs ordered by the first
//! number or the first date in their text, as the reference's Sort Text dialog does with its
//! "Type" box. Paragraphs without a number (date) sort before the others, ties keep their
//! order, each paragraph keeps its formatting; one undo step. Text order is
//! [`Session::sort_paragraphs`] (Vietnamese dictionary order).

use crate::{Pos, Result, Selection, Session};
use bo_model::Paragraph;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// What the paragraphs are compared by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortType {
    /// Their text (Vietnamese dictionary order).
    #[default]
    Text,
    /// The first number in the text (`-1 234,5`, `12.5`, `3`).
    Number,
    /// The first date in the text (`2026-11-24`, `24/11/2026`, `November 24, 2026`, …).
    Date,
}

/// The first number of `s`: a sign, digits with `,` / `.` / space thousands groups and a
/// decimal `,` or `.` (the last separator followed by 1–2 or 4+ digits, or a lone one, is the
/// decimal mark).
#[must_use]
pub fn first_number(s: &str) -> Option<f64> {
    let chars: Vec<char> = s.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let neg = start > 0 && matches!(chars[start - 1], '-' | '\u{2212}');
    let mut body = String::new();
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        let next_digit = chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if c.is_ascii_digit() || (matches!(c, ',' | '.' | '\u{a0}') && next_digit) {
            body.push(c);
            i += 1;
        } else {
            break;
        }
    }
    // The decimal mark: the last separator unless it is followed by exactly three digits
    // and another separator of the same kind comes before it (a thousands group).
    let seps: Vec<(usize, char)> = body
        .char_indices()
        .filter(|(_, c)| !c.is_ascii_digit())
        .collect();
    let decimal = seps.last().and_then(|&(at, c)| {
        let tail = body.len() - at - c.len_utf8();
        let grouped = tail == 3 && (seps.len() > 1 || c == '\u{a0}');
        (!grouped && c != '\u{a0}').then_some(at)
    });
    let mut out = String::new();
    for (at, c) in body.char_indices() {
        if c.is_ascii_digit() {
            out.push(c);
        } else if Some(at) == decimal {
            out.push('.');
        }
    }
    let v: f64 = out.parse().ok()?;
    Some(if neg { -v } else { v })
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn month_of(word: &str) -> Option<u32> {
    let w = word.to_lowercase();
    if w.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| w.starts_with(m))
        .map(|i| i as u32 + 1)
}

fn year4(y: u32) -> u32 {
    match y {
        0..=29 => 2000 + y,
        30..=99 => 1900 + y,
        _ => y,
    }
}

fn valid(y: u32, m: u32, d: u32) -> Option<i64> {
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| i64::from(y * 10_000 + m * 100 + d))
}

/// The first date of `s` as `yyyymmdd`: ISO `y-m-d`, numeric `d/m/y` (`day_first`) or `m/d/y`
/// with `/`, `-` or `.`, and `Month d, yyyy` / `d Month yyyy` with English month names.
#[must_use]
pub fn first_date(s: &str, day_first: bool) -> Option<i64> {
    // Tokens: runs of digits and runs of letters, with the separators between them.
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        let same = cur.chars().last().is_some_and(|l| {
            l.is_ascii_digit() == c.is_ascii_digit() && l.is_alphabetic() == c.is_alphabetic()
        });
        if c.is_ascii_digit() || c.is_alphabetic() {
            if !same && !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        } else {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            if matches!(c, '/' | '-' | '.') {
                toks.push(c.to_string());
            }
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let num = |t: &str| -> Option<u32> {
        (t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 4)
            .then(|| t.parse().ok())
            .flatten()
    };
    for i in 0..toks.len() {
        // Numeric a SEP b SEP c.
        if let (Some(a), Some(b), Some(c), Some(sep)) = (
            num(&toks[i]),
            toks.get(i + 2).and_then(|t| num(t)),
            toks.get(i + 4).and_then(|t| num(t)),
            toks.get(i + 1),
        ) {
            if matches!(sep.as_str(), "/" | "-" | ".") && toks.get(i + 3) == Some(sep) {
                let v = if toks[i].len() == 4 {
                    valid(a, b, c)
                } else if day_first {
                    valid(year4(c), b, a)
                } else {
                    valid(year4(c), a, b)
                };
                if v.is_some() {
                    return v;
                }
            }
        }
        // Month d yyyy.
        if let Some(m) = month_of(&toks[i]) {
            if let (Some(d), Some(y)) = (
                toks.get(i + 1).and_then(|t| num(t)),
                toks.get(i + 2).and_then(|t| num(t)),
            ) {
                if let Some(v) = valid(year4(y), m, d) {
                    return Some(v);
                }
            }
        }
        // d Month yyyy.
        if let (Some(d), Some(m), Some(y)) = (
            num(&toks[i]),
            toks.get(i + 1).and_then(|t| month_of(t)),
            toks.get(i + 2).and_then(|t| num(t)),
        ) {
            if let Some(v) = valid(year4(y), m, d) {
                return Some(v);
            }
        }
    }
    None
}

fn cmp_keys(x: Option<f64>, y: Option<f64>) -> Ordering {
    match (x, y) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

impl Session {
    /// Sort the selected paragraphs by `by` (see [`SortType`]), ascending or descending;
    /// `day_first` reads `1/2/2026` as 1 February. One undo step.
    pub fn sort_paragraphs_by(
        &mut self,
        descending: bool,
        by: SortType,
        day_first: bool,
    ) -> Result<Selection> {
        if by == SortType::Text {
            return self.sort_paragraphs(descending);
        }
        let (a, b) = self.selection.ordered();
        if b.para <= a.para {
            return Ok(self.selection);
        }
        let ids: Vec<usize> = (a.para..=b.para).collect();
        self.edit(move |tx, _| {
            let mut paras = Vec::with_capacity(ids.len());
            let mut paths = Vec::with_capacity(ids.len());
            for id in &ids {
                let (path, p, _) = tx.atoms(*id)?;
                paths.push(path);
                paras.push(p);
            }
            let mut keyed: Vec<(Option<f64>, Paragraph)> = paras
                .into_iter()
                .map(|p| {
                    let text = p.plain_text();
                    let k = match by {
                        SortType::Date => first_date(&text, day_first).map(|d| d as f64),
                        _ => first_number(&text),
                    };
                    (k, p)
                })
                .collect();
            keyed.sort_by(|(x, _), (y, _)| {
                let o = cmp_keys(*x, *y);
                if descending {
                    o.reverse()
                } else {
                    o
                }
            });
            for (path, (_, p)) in paths.iter().zip(keyed) {
                tx.replace_para(path, p)?;
            }
            Ok(Selection {
                anchor: Pos::new(a.para, 0),
                focus: Pos::new(a.para, 0),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_in_text() {
        assert_eq!(first_number("Item 12 of 30"), Some(12.0));
        assert_eq!(first_number("-3,5 kg"), Some(-3.5));
        assert_eq!(first_number("1,234,567 people"), Some(1_234_567.0));
        assert_eq!(first_number("1.234.567"), Some(1_234_567.0));
        assert_eq!(first_number("12.75"), Some(12.75));
        assert_eq!(first_number("no digits"), None);
    }

    #[test]
    fn dates_in_text() {
        assert_eq!(first_date("Due 2026-11-24.", false), Some(20_261_124));
        assert_eq!(first_date("on 3/4/2026", true), Some(20_260_403));
        assert_eq!(first_date("on 3/4/2026", false), Some(20_260_304));
        assert_eq!(first_date("November 24, 2026", false), Some(20_261_124));
        assert_eq!(first_date("Meeting 5 Jan 26", false), Some(20_260_105));
        assert_eq!(first_date("no date here", false), None);
    }

    fn session(paras: &[&str]) -> Session {
        let p = |t: &str| {
            bo_model::Block::Paragraph(bo_model::Paragraph::new(
                bo_model::ParaProps::default(),
                vec![bo_model::Run::text(t, bo_model::RunProps::default())],
            ))
        };
        Session::new(bo_model::Document {
            sections: vec![bo_model::Section::new(paras.iter().map(|t| p(t)).collect())],
            ..Default::default()
        })
    }

    fn texts(s: &Session) -> Vec<String> {
        (0..s.paragraph_count())
            .map(|i| s.para_text(i).unwrap_or_default())
            .collect()
    }

    fn all(s: &mut Session) {
        let last = s.paragraph_count() - 1;
        s.set_selection(Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(last, 1),
        });
    }

    #[test]
    fn sorts_by_number_and_date() {
        let mut s = session(&["item 10", "item 9", "none", "item 100"]);
        all(&mut s);
        s.sort_paragraphs_by(false, SortType::Number, false)
            .unwrap();
        assert_eq!(texts(&s), ["none", "item 9", "item 10", "item 100"]);
        assert!(s.undo());
        assert_eq!(texts(&s)[0], "item 10");
        all(&mut s);
        s.sort_paragraphs_by(true, SortType::Number, false).unwrap();
        assert_eq!(texts(&s)[0], "item 100");

        let mut s = session(&["b 2/1/2026", "a 1/2/2026", "c 2025-12-31"]);
        all(&mut s);
        s.sort_paragraphs_by(false, SortType::Date, true).unwrap();
        assert_eq!(texts(&s), ["c 2025-12-31", "b 2/1/2026", "a 1/2/2026"]);
    }
}

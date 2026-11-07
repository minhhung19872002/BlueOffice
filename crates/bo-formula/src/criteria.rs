//! Criteria strings used by `COUNTIF`, `SUMIF(S)`, `AVERAGEIF(S)`, `MAXIFS`, ...
//! (ECMA-376 Part 1, §18.17.7 COUNTIF: a criterion is a number, expression or
//! text such as `">5"`, `"<>a"`, `"ab*"`; `*` `?` are wildcards and `~` escapes).

use std::cmp::Ordering;

use crate::date::{parse_datetime_text, DateSystem};
use crate::value::{cmp_text, num_eq, parse_number_text, ErrorKind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq)]
enum Operand {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
    /// `"="` alone: only truly blank cells.
    Blank,
}

/// A parsed criterion.
#[derive(Clone, Debug, PartialEq)]
pub struct Criterion {
    op: Op,
    operand: Operand,
}

impl Criterion {
    /// Build a criterion from a criteria argument value.
    pub fn parse(v: &Value, system: DateSystem) -> Criterion {
        match v.first() {
            Value::Number(n) => Criterion {
                op: Op::Eq,
                operand: Operand::Number(*n),
            },
            Value::Bool(b) => Criterion {
                op: Op::Eq,
                operand: Operand::Bool(*b),
            },
            Value::Error(e) => Criterion {
                op: Op::Eq,
                operand: Operand::Error(*e),
            },
            Value::Text(s) => Self::parse_text(s, system),
            _ => Criterion {
                op: Op::Eq,
                operand: Operand::Text(String::new()),
            },
        }
    }

    fn parse_text(s: &str, system: DateSystem) -> Criterion {
        let (op, rest, has_op) = if let Some(r) = s.strip_prefix(">=") {
            (Op::Ge, r, true)
        } else if let Some(r) = s.strip_prefix("<=") {
            (Op::Le, r, true)
        } else if let Some(r) = s.strip_prefix("<>") {
            (Op::Ne, r, true)
        } else if let Some(r) = s.strip_prefix('>') {
            (Op::Gt, r, true)
        } else if let Some(r) = s.strip_prefix('<') {
            (Op::Lt, r, true)
        } else if let Some(r) = s.strip_prefix('=') {
            (Op::Eq, r, true)
        } else {
            (Op::Eq, s, false)
        };
        let operand = if rest.is_empty() {
            if has_op && op == Op::Eq {
                Operand::Blank
            } else {
                Operand::Text(String::new())
            }
        } else if let Some(n) = parse_number_text(rest) {
            Operand::Number(n)
        } else if rest.eq_ignore_ascii_case("TRUE") {
            Operand::Bool(true)
        } else if rest.eq_ignore_ascii_case("FALSE") {
            Operand::Bool(false)
        } else if let Some(e) = ErrorKind::parse(rest) {
            Operand::Error(e)
        } else if let Some(n) = parse_datetime_text(rest, system) {
            Operand::Number(n)
        } else {
            Operand::Text(rest.to_owned())
        };
        Criterion { op, operand }
    }

    /// Whether a cell value satisfies the criterion.
    pub fn matches(&self, v: &Value) -> bool {
        let eq = || -> bool {
            match (&self.operand, v) {
                (Operand::Blank, Value::Empty) => true,
                (Operand::Number(a), Value::Number(b)) => num_eq(*a, *b),
                (Operand::Bool(a), Value::Bool(b)) => a == b,
                (Operand::Error(a), Value::Error(b)) => a == b,
                (Operand::Text(p), Value::Text(t)) => wildcard_match(p, t),
                (Operand::Text(p), Value::Empty) => p.is_empty(),
                _ => false,
            }
        };
        match self.op {
            Op::Eq => eq(),
            Op::Ne => !eq(),
            _ => {
                let ord = match (&self.operand, v) {
                    (Operand::Number(a), Value::Number(b)) => {
                        if num_eq(*a, *b) {
                            Ordering::Equal
                        } else {
                            b.partial_cmp(a).unwrap_or(Ordering::Equal)
                        }
                    }
                    (Operand::Text(a), Value::Text(b)) => cmp_text(b, a),
                    (Operand::Bool(a), Value::Bool(b)) => b.cmp(a),
                    _ => return false,
                };
                match self.op {
                    Op::Lt => ord == Ordering::Less,
                    Op::Le => ord != Ordering::Greater,
                    Op::Gt => ord == Ordering::Greater,
                    _ => ord != Ordering::Less,
                }
            }
        }
    }
}

/// Whether text contains unescaped wildcard characters.
pub fn has_wildcards(p: &str) -> bool {
    p.contains(['*', '?', '~'])
}

#[derive(Clone, Copy, PartialEq)]
enum Pat {
    Char(char),
    Any,
    Star,
}

fn compile(p: &str) -> Vec<Pat> {
    let mut out = Vec::new();
    let mut chars = p.chars();
    while let Some(c) = chars.next() {
        match c {
            '~' => match chars.next() {
                Some(n) => out.extend(n.to_lowercase().map(Pat::Char)),
                None => out.push(Pat::Char('~')),
            },
            '*' => out.push(Pat::Star),
            '?' => out.push(Pat::Any),
            _ => out.extend(c.to_lowercase().map(Pat::Char)),
        }
    }
    out
}

/// Case-insensitive match of `text` against a pattern with `*`, `?` and `~` escapes.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = compile(pattern);
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some(Pat::Star) => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some(Pat::Any) => {
                pi += 1;
                ti += 1;
            }
            Some(Pat::Char(c)) if *c == t[ti] => {
                pi += 1;
                ti += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    p[pi..].iter().all(|x| *x == Pat::Star)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        assert!(wildcard_match("a*", "Apple"));
        assert!(wildcard_match("?pple", "apple"));
        assert!(!wildcard_match("?pple", "pple"));
        assert!(wildcard_match("*~*", "a*"));
        assert!(!wildcard_match("*~*", "ab"));
        assert!(wildcard_match("a*b*c", "aXXbYYc"));
        assert!(wildcard_match("", ""));
        assert!(!wildcard_match("", "x"));
    }

    #[test]
    fn criteria() {
        let c = Criterion::parse(&">5".into(), DateSystem::Date1900);
        assert!(c.matches(&6.0.into()));
        assert!(!c.matches(&5.0.into()));
        assert!(!c.matches(&"7".into()));
        let c = Criterion::parse(&"<>a".into(), DateSystem::Date1900);
        assert!(c.matches(&"b".into()));
        assert!(!c.matches(&"A".into()));
        assert!(c.matches(&Value::Empty));
    }
}

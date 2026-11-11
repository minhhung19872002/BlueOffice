//! Formulas as people type them in a locale whose list separator is `;` and whose
//! decimal separator is `,` (vi-VN, de-DE, fr-FR…), turned into the stored form of
//! ECMA-376 Part 1, §18.17 (arguments separated by `,`, decimals with `.`).
//!
//! Spreadsheet programs run with such a regional setting expect `=IF(A1>5;"a";"b")`
//! and `=ROUND(A1*1,5;0)`; BlueSheet keeps showing the stored form and accepts both.

use crate::parser::parse;

/// What sits around a character while scanning formula text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Code,
    /// Inside `"…"` (a `""` pair is an escaped quote).
    Text,
    /// Inside `'…'` (a quoted sheet name).
    Sheet,
}

/// Byte classes of `body`: for each char, whether it is formula code at brace and
/// bracket depth 0 (where `;` and `,` can be separators).
fn top_level(body: &str) -> Vec<(usize, char, bool)> {
    let mut out = Vec::with_capacity(body.len());
    let mut ctx = Ctx::Code;
    let (mut braces, mut brackets) = (0u32, 0u32);
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        match ctx {
            Ctx::Text | Ctx::Sheet => {
                let q = if ctx == Ctx::Text { '"' } else { '\'' };
                out.push((at, c, false));
                if c == q {
                    if chars.get(i + 1).is_some_and(|&(_, n)| n == q) {
                        out.push((chars[i + 1].0, q, false));
                        i += 1;
                    } else {
                        ctx = Ctx::Code;
                    }
                }
            }
            Ctx::Code => {
                match c {
                    '"' => ctx = Ctx::Text,
                    '\'' => ctx = Ctx::Sheet,
                    '{' => braces += 1,
                    '}' => braces = braces.saturating_sub(1),
                    '[' => brackets += 1,
                    ']' => brackets = brackets.saturating_sub(1),
                    _ => {}
                }
                let top = ctx == Ctx::Code && braces == 0 && brackets == 0 && c != '}' && c != ']';
                out.push((at, c, top));
            }
        }
        i += 1;
    }
    out
}

/// Whether the `,` at index `i` of `chars` joins the two halves of a number written
/// with a decimal comma (`1,5`, `0,25`): digits on both sides, and the digits before
/// it are not the end of a reference or a name (`A1,5`).
fn decimal_comma(chars: &[(usize, char, bool)], i: usize) -> bool {
    let next = chars.get(i + 1).is_some_and(|c| c.1.is_ascii_digit());
    if !next || i == 0 || !chars[i - 1].1.is_ascii_digit() {
        return false;
    }
    let mut j = i;
    while j > 0 && chars[j - 1].1.is_ascii_digit() {
        j -= 1;
    }
    // What precedes the integer part: nothing, an operator, a parenthesis, a separator.
    j == 0 || !(chars[j - 1].1.is_alphanumeric() || matches!(chars[j - 1].1, '$' | '.' | '_'))
}

/// The stored form of a typed formula (`=` optional), or `None` when it parses as
/// typed or no reading of it parses.
///
/// * `;` outside text, sheet names, array constants and structured references is
///   the argument separator: it becomes `,`, and with `decimal` = `,` the commas
///   inside numbers become `.` (`=ROUND(1,25;1)` → `=ROUND(1.25,1)`);
/// * otherwise, with `decimal` = `,`, a comma between digits outside any parentheses
///   (where it cannot separate arguments) is a decimal one (`=A1*1,5` → `=A1*1.5`);
/// * missing closing parentheses at the end are added (`=SUM(A1:A3` → `=SUM(A1:A3)`).
#[must_use]
pub fn localized_input(formula: &str, decimal: char) -> Option<String> {
    let chars = top_level(formula);
    let semicolons = chars.iter().any(|&(_, c, top)| top && c == ';');
    if semicolons {
        // Argument separators `;`: every top-level `,` between digits is a decimal one.
        let s: String = chars
            .iter()
            .enumerate()
            .map(|(i, &(_, c, top))| match c {
                ';' if top => ',',
                ',' if top && decimal == ',' && decimal_comma(&chars, i) => '.',
                _ => c,
            })
            .collect();
        return if parse(&s).is_ok() {
            Some(s)
        } else {
            close_parens(&s)
        };
    }
    if decimal == ',' {
        let mut depth = 0i32;
        let mut changed = false;
        let mut s = String::with_capacity(formula.len());
        for (i, &(_, c, top)) in chars.iter().enumerate() {
            match c {
                '(' if top => depth += 1,
                ')' if top => depth -= 1,
                _ => {}
            }
            if c == ',' && top && depth == 0 && decimal_comma(&chars, i) {
                s.push('.');
                changed = true;
            } else {
                s.push(c);
            }
        }
        if changed && parse(&s).is_ok() {
            return Some(s);
        }
    }
    if parse(formula).is_ok() {
        return None;
    }
    close_parens(formula)
}

/// `text` with the parentheses left open at its end closed, when that parses.
fn close_parens(text: &str) -> Option<String> {
    let open = top_level(text)
        .iter()
        .fold(0i64, |n, &(_, c, top)| match c {
            '(' if top => n + 1,
            ')' if top => n - 1,
            _ => n,
        });
    if open <= 0 || open > 64 {
        return None;
    }
    let mut s = text.to_owned();
    for _ in 0..open {
        s.push(')');
    }
    parse(&s).is_ok().then_some(s)
}

#[cfg(test)]
mod tests {
    use super::localized_input;

    #[test]
    fn semicolons_become_commas() {
        assert_eq!(
            localized_input("SUM(A1;B1)", ','),
            Some("SUM(A1,B1)".into())
        );
        assert_eq!(
            localized_input("=IF(A1>5;\"lớn; rất\";\"nhỏ\")", ','),
            Some("=IF(A1>5,\"lớn; rất\",\"nhỏ\")".into())
        );
        // Decimal commas of the same formula.
        assert_eq!(
            localized_input("=ROUND(A1*1,25;1)", ','),
            Some("=ROUND(A1*1.25,1)".into())
        );
        // A sheet name with a semicolon, an array constant's row separator.
        assert_eq!(
            localized_input("=SUM('a;b'!A1;{1,2;3,4})", ','),
            Some("=SUM('a;b'!A1,{1,2;3,4})".into())
        );
        // en-US decimal point with semicolons.
        assert_eq!(
            localized_input("=ROUND(2.5;0)", '.'),
            Some("=ROUND(2.5,0)".into())
        );
    }

    #[test]
    fn decimal_comma_only_when_needed() {
        // Parses as typed: unchanged (two arguments).
        assert_eq!(localized_input("=SUM(1,5)", ','), None);
        assert_eq!(localized_input("=A1*1,5", ','), Some("=A1*1.5".into()));
        // No decimal comma in en-US.
        assert_eq!(localized_input("=A1*1,5", '.'), None);
    }

    #[test]
    fn closes_parentheses() {
        assert_eq!(
            localized_input("=SUM(A1:A3", ','),
            Some("=SUM(A1:A3)".into())
        );
        assert_eq!(
            localized_input("=ROUND(SUM(A1;A2);1", ','),
            Some("=ROUND(SUM(A1,A2),1)".into())
        );
        // Still wrong: nothing to offer.
        assert_eq!(localized_input("=Z99+", ','), None);
        assert_eq!(localized_input("=SUM(A1))", ','), None);
    }
}

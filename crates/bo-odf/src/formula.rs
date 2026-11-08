//! Formula text between A1 (SpreadsheetML, ECMA-376 Part 1, §18.17) and
//! OpenFormula (ODF 1.3 Part 4, §5): references are written in brackets
//! (`[.A1]`, `[.A1:.B2]`, `[Sheet2.A1]`, `['My sheet'.A1]`, §5.8), function
//! parameters are separated by `;` (§5.10) and the attribute value carries the
//! `of:` namespace prefix (Part 3, §19.643 `table:formula`).

/// Whether `s` is a cell reference such as `A1`, `$B$7`.
fn is_cell(s: &str) -> bool {
    let t = s.replace('$', "");
    let split = t.find(|c: char| c.is_ascii_digit());
    match split {
        Some(i) if i > 0 && i <= 3 => {
            t[..i].chars().all(|c| c.is_ascii_alphabetic())
                && t[i..].chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// A1 formula text (without `=`) → `of:=…`.
pub fn to_openformula(a1: &str) -> String {
    let chars: Vec<char> = a1.chars().collect();
    let mut out = String::from("of:=");
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            // String literal ("" escapes a quote), copied verbatim.
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        out.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == ',' {
            out.push(';');
            i += 1;
            continue;
        }
        // Quoted sheet name: 'My sheet'!A1
        let (sheet, start) = if c == '\'' {
            let mut j = i + 1;
            let mut name = String::from("'");
            while j < chars.len() {
                if chars[j] == '\'' {
                    if chars.get(j + 1) == Some(&'\'') {
                        name.push_str("''");
                        j += 2;
                        continue;
                    }
                    break;
                }
                name.push(chars[j]);
                j += 1;
            }
            name.push('\'');
            if chars.get(j + 1) == Some(&'!') {
                (Some(name), j + 2)
            } else {
                // Not a reference: copy as is.
                out.extend(&chars[i..=j.min(chars.len() - 1)]);
                i = j + 1;
                continue;
            }
        } else {
            (None, i)
        };
        if sheet.is_some() || c.is_ascii_alphanumeric() || c == '$' || c == '_' {
            let mut j = start;
            while j < chars.len()
                && (chars[j].is_alphanumeric() || matches!(chars[j], '$' | '_' | '.'))
            {
                j += 1;
            }
            let word: String = chars[start..j].iter().collect();
            // Unquoted sheet: Sheet2!A1
            let (sheet, word, j) = if sheet.is_none() && chars.get(j) == Some(&'!') {
                let mut k = j + 1;
                while k < chars.len() && (chars[k].is_ascii_alphanumeric() || chars[k] == '$') {
                    k += 1;
                }
                (Some(word), chars[j + 1..k].iter().collect::<String>(), k)
            } else {
                (sheet, word, j)
            };
            if is_cell(&word) && chars.get(j) != Some(&'(') {
                let prefix = sheet.unwrap_or_default();
                // Range: A1:B2
                let mut k = j;
                let mut second = None;
                if chars.get(k) == Some(&':') {
                    let mut e = k + 1;
                    while e < chars.len() && (chars[e].is_ascii_alphanumeric() || chars[e] == '$') {
                        e += 1;
                    }
                    let w2: String = chars[k + 1..e].iter().collect();
                    if is_cell(&w2) {
                        second = Some(w2);
                        k = e;
                    }
                }
                out.push('[');
                out.push_str(&prefix);
                out.push('.');
                out.push_str(&word);
                if let Some(w2) = second {
                    out.push_str(":.");
                    out.push_str(&w2);
                }
                out.push(']');
                i = k;
                continue;
            }
            if let Some(s) = sheet {
                out.push_str(&s);
                out.push('!');
            }
            out.push_str(&word);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `of:=…` (or a formula in another known namespace) → A1 text without `=`.
/// `None` for formulas in an unknown syntax.
pub fn from_openformula(f: &str) -> Option<String> {
    let body = f
        .strip_prefix("of:=")
        .or_else(|| f.strip_prefix("oooc:="))
        .or_else(|| f.strip_prefix('='))?;
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                out.push(c);
                i += 1;
                while i < chars.len() {
                    out.push(chars[i]);
                    if chars[i] == '"' {
                        if chars.get(i + 1) == Some(&'"') {
                            out.push('"');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            ';' => {
                out.push(',');
                i += 1;
            }
            '[' => {
                let end = chars[i..].iter().position(|&ch| ch == ']')? + i;
                let inner: String = chars[i + 1..end].iter().collect();
                for (k, part) in inner.split(':').enumerate() {
                    if k > 0 {
                        out.push(':');
                    }
                    let part = part.trim_start_matches('$');
                    match part.rsplit_once('.') {
                        Some((sheet, cell)) if !sheet.is_empty() && k == 0 => {
                            out.push_str(sheet.trim_start_matches('$'));
                            out.push('!');
                            out.push_str(cell);
                        }
                        Some((_, cell)) => out.push_str(cell),
                        None => out.push_str(part),
                    }
                }
                i = end + 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_and_openformula_round_trip() {
        for (a1, of) in [
            ("SUM(A1:B2)", "of:=SUM([.A1:.B2])"),
            ("A1*2+$B$3", "of:=[.A1]*2+[.$B$3]"),
            ("IF(C1>500,D1,\"a,b\")", "of:=IF([.C1]>500;[.D1];\"a,b\")"),
            ("Sheet2!A1+'Bảng 1'!B2:C3", "of:=[Sheet2.A1]+['Bảng 1'.B2:.C3]"),
            ("VLOOKUP(5,$A$1:$B$1000,2,FALSE)", "of:=VLOOKUP(5;[.$A$1:.$B$1000];2;FALSE)"),
            ("LOG10(100)+Rate", "of:=LOG10(100)+Rate"),
        ] {
            assert_eq!(to_openformula(a1), of, "{a1}");
            assert_eq!(from_openformula(of).as_deref(), Some(a1), "{of}");
        }
        assert_eq!(from_openformula("msoxl:=A1"), None);
        assert_eq!(from_openformula("of:=[$Sheet1.A1]").as_deref(), Some("Sheet1!A1"));
    }
}

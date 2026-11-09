//! Translation of find patterns (special characters, wildcards) to regular
//! expressions over story text, and expansion of replacement text.
//!
//! Story text is the paragraphs' index texts joined by `\n` (the paragraph mark);
//! tabs are `\t`, manual line breaks U+2028, page breaks U+000C, column breaks
//! U+000B and inline objects U+FFFC (see `bo_model::index`).
//!
//! Special characters follow the publicly documented find syntax of word processors:
//! `^p` paragraph mark, `^t` tab, `^l` manual line break, `^m` page break, `^n` column
//! break, `^s` non-breaking space, `^~` non-breaking hyphen, `^-` optional hyphen,
//! `^+` em dash, `^=` en dash, `^g` inline object, `^w` white space, `^#` any digit,
//! `^$` any letter, `^?` any character, `^^` a caret, `^nnn` character code (decimal).

use crate::{EditError, Result};

/// Paragraph mark in story text.
const PARA: char = '\n';
/// Any character but a paragraph mark.
const ANY: &str = "[^\\n]";

fn err(msg: impl Into<String>) -> EditError {
    EditError::Regex(msg.into())
}

/// Character of the decimal code `^nnn` (`^13` is the paragraph mark, `^11` a line
/// break, `^12` a page break, `^14` a column break, `^9` a tab).
fn code_char(n: u32) -> Option<char> {
    match n {
        0 => None,
        13 | 10 => Some(PARA),
        11 => Some('\u{2028}'),
        12 => Some('\u{000C}'),
        14 => Some('\u{000B}'),
        n => char::from_u32(n),
    }
}

/// Read the digits after `^` (at most five); `None` when there are none.
fn take_code(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<u32> {
    let mut n: Option<u32> = None;
    for _ in 0..5 {
        match chars.peek().and_then(|c| c.to_digit(10)) {
            Some(d) => {
                n = Some(n.unwrap_or(0) * 10 + d);
                chars.next();
            }
            None => break,
        }
    }
    n
}

/// Literal character of a special code shared by find and replace.
fn special_char(c: char) -> Option<char> {
    Some(match c {
        'p' => PARA,
        't' => '\t',
        'l' => '\u{2028}',
        'm' => '\u{000C}',
        's' => '\u{00A0}',
        '~' => '\u{2011}',
        '-' => '\u{00AD}',
        '+' => '\u{2014}',
        '=' => '\u{2013}',
        '^' => '^',
        _ => return None,
    })
}

/// Regular expression of the special code after `^` in a find pattern.
fn special_regex(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String> {
    if let Some(n) = take_code(chars) {
        let c = code_char(n).ok_or_else(|| err(format!("^{n}")))?;
        return Ok(regex::escape(c.encode_utf8(&mut [0; 4])));
    }
    let c = chars.next().ok_or_else(|| err("^"))?;
    if let Some(ch) = special_char(c) {
        return Ok(regex::escape(ch.encode_utf8(&mut [0; 4])));
    }
    Ok(match c {
        'n' => "\\x0B".into(),
        'g' => "\u{FFFC}".into(),
        'w' => "[ \\t\u{00A0}]+".into(),
        '#' => "[0-9]".into(),
        '$' => "\\p{L}".into(),
        '?' => ANY.into(),
        _ => return Err(err(format!("^{c}"))),
    })
}

/// Regular expression of a plain query with special characters.
pub fn translate_special(query: &str) -> Result<String> {
    let mut out = String::new();
    let mut chars = query.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' {
            out.push_str(&special_regex(&mut chars)?);
        } else {
            out.push_str(&regex::escape(c.encode_utf8(&mut [0; 4])));
        }
    }
    Ok(out)
}

/// Escape one character for use inside a regex character class.
fn class_char(c: char, out: &mut String) {
    if matches!(c, '\\' | ']' | '[' | '^' | '-' | '&' | '~') {
        out.push('\\');
    }
    out.push(c);
}

/// Regular expression of a wildcard pattern:
/// `?` any character, `*` any run of characters (shortest), `[abc]` `[a-z]` one of,
/// `[!x]` none of, `{n}` `{n,}` `{n,m}` (also with `;`) repetition of the previous
/// item, `@` one or more of the previous item, `<` start of word, `>` end of word,
/// `( )` group (`\1` … `\9` in the replacement), `\x` the character `x` literally,
/// and the special characters `^p` `^t` ….
pub fn translate_wildcards(query: &str) -> Result<String> {
    let mut out = String::new();
    let mut chars = query.chars().peekable();
    let mut groups = 0usize;
    let mut open = 0usize;
    // Something that a repetition can follow was just written.
    let mut atom = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let x = chars.next().ok_or_else(|| err("\\"))?;
                out.push_str(&regex::escape(x.encode_utf8(&mut [0; 4])));
                atom = true;
            }
            '^' => {
                out.push_str(&format!("(?:{})", special_regex(&mut chars)?));
                atom = true;
            }
            '?' => {
                out.push_str(ANY);
                atom = true;
            }
            '*' => {
                out.push_str("[^\\n]*?");
                atom = false;
            }
            '@' => {
                if !atom {
                    return Err(err("@"));
                }
                out.push('+');
                atom = false;
            }
            '{' => {
                if !atom {
                    return Err(err("{"));
                }
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(';') => body.push(','),
                        Some(d @ ('0'..='9' | ',')) => body.push(d),
                        _ => return Err(err("{")),
                    }
                }
                let (lo, hi) = match body.split_once(',') {
                    Some((lo, hi)) => (lo, Some(hi)),
                    None => (body.as_str(), None),
                };
                if (lo.is_empty() && hi.is_none_or(str::is_empty))
                    || hi.is_some_and(|h| h.contains(','))
                {
                    return Err(err("{"));
                }
                let lo = if lo.is_empty() { "0" } else { lo };
                match hi {
                    None => out.push_str(&format!("{{{lo}}}")),
                    Some(h) => out.push_str(&format!("{{{lo},{h}}}")),
                }
                atom = false;
            }
            '[' => {
                let mut class = String::from("[");
                if chars.peek() == Some(&'!') {
                    chars.next();
                    class.push('^');
                    class.push_str("\\n");
                }
                let mut items = 0;
                loop {
                    let x = match chars.next() {
                        Some(']') if items > 0 => break,
                        Some('\\') => chars.next().ok_or_else(|| err("["))?,
                        Some(x) => x,
                        None => return Err(err("[")),
                    };
                    items += 1;
                    if chars.peek() == Some(&'-') {
                        let mut ahead = chars.clone();
                        ahead.next();
                        if let Some(&y) = ahead.peek().filter(|&&y| y != ']') {
                            if y < x {
                                return Err(err(format!("[{x}-{y}]")));
                            }
                            chars.next();
                            chars.next();
                            class_char(x, &mut class);
                            class.push('-');
                            class_char(y, &mut class);
                            continue;
                        }
                    }
                    class_char(x, &mut class);
                }
                class.push(']');
                out.push_str(&class);
                atom = true;
            }
            '(' => {
                groups += 1;
                open += 1;
                if groups > 9 {
                    return Err(err("("));
                }
                out.push('(');
                atom = false;
            }
            ')' => {
                if open == 0 {
                    return Err(err(")"));
                }
                open -= 1;
                out.push(')');
                atom = true;
            }
            '<' => {
                out.push_str("\\b{start}");
                atom = false;
            }
            '>' => {
                out.push_str("\\b{end}");
                atom = false;
            }
            c => {
                out.push_str(&regex::escape(c.encode_utf8(&mut [0; 4])));
                atom = true;
            }
        }
    }
    if open != 0 {
        return Err(err("("));
    }
    Ok(out)
}

/// Expand a replacement: `^p` `^t` `^l` `^m` `^s` `^~` `^-` `^+` `^=` `^^` `^nnn`,
/// `^&` (the matched text) and, in wildcard mode, `\1` … `\9` (groups) and `\x`
/// (the character `x`). Unknown codes are kept as typed.
pub fn expand_replacement(
    with: &str,
    caps: Option<&regex::Captures<'_>>,
    wildcards: bool,
) -> String {
    let whole = caps.and_then(|c| c.get(0)).map_or("", |m| m.as_str());
    expand_parts(
        with,
        whole,
        |i| caps.and_then(|c| c.get(i)).map(|m| m.as_str()),
        wildcards,
    )
}

/// Plain-mode replacement with `^&` standing for `matched`.
pub(crate) fn expand_with_text(with: &str, matched: &str) -> String {
    expand_parts(with, matched, |_| None, false)
}

fn expand_parts<'a>(
    with: &str,
    whole: &str,
    group: impl Fn(usize) -> Option<&'a str>,
    wildcards: bool,
) -> String {
    let mut out = String::new();
    let mut chars = with.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '^' => {
                if let Some(n) = take_code(&mut chars) {
                    match code_char(n) {
                        Some(ch) => out.push(ch),
                        None => out.push_str(&format!("^{n}")),
                    }
                    continue;
                }
                match chars.next() {
                    Some('&') => out.push_str(whole),
                    Some(x) => {
                        if let Some(ch) = special_char(x) {
                            out.push(ch);
                        } else {
                            out.push('^');
                            out.push(x);
                        }
                    }
                    None => out.push('^'),
                }
            }
            '\\' if wildcards => match chars.next() {
                Some(d @ '1'..='9') => {
                    let i = d.to_digit(10).map_or(0, |i| i as usize);
                    if let Some(g) = group(i) {
                        out.push_str(g);
                    }
                }
                Some(x) => out.push(x),
                None => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    out
}

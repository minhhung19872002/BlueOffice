//! Basic grammar and style checks for running text (`docs/parity/BACKLOG.md` CORE-GRAM-001): extra spaces,
//! spacing around punctuation, a capital letter at the start of a sentence, a word written twice, unbalanced
//! brackets and quotes, and Vietnamese tone marks placed in the other style than the rest of the document.
//!
//! The rules are written for this project from the usual typographic conventions of Vietnamese and English
//! text (a space after, none before `, . ; : ! ?`; sentences begin with a capital letter). Each issue has a
//! byte range in the checked text and, when the fix is mechanical, the text to put there.

use crate::vi::{self, ToneStyle};
use std::ops::Range;

/// What a grammar issue is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Two or more spaces between words.
    ExtraSpace,
    /// A space before `, . ; : ! ? ) ] }`.
    SpaceBeforePunctuation,
    /// No space after `, ;` (before a letter) or after `. ! ?` (before a capital letter).
    MissingSpace,
    /// A sentence starting with a small letter.
    Capitalize,
    /// The same word twice in a row.
    RepeatedWord,
    /// A bracket or quote that is never closed (or closed without being opened).
    Unbalanced,
    /// A Vietnamese tone mark placed in the other style than the document (`hòa` among `hoà`).
    ToneStyle,
}

impl Rule {
    /// Stable identifier (messages are looked up by it).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Rule::ExtraSpace => "extraSpace",
            Rule::SpaceBeforePunctuation => "spaceBeforePunctuation",
            Rule::MissingSpace => "missingSpace",
            Rule::Capitalize => "capitalize",
            Rule::RepeatedWord => "repeatedWord",
            Rule::Unbalanced => "unbalanced",
            Rule::ToneStyle => "toneStyle",
        }
    }
}

/// One grammar or style issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Byte range in the checked text.
    pub range: Range<usize>,
    /// The rule.
    pub rule: Rule,
    /// Text that fixes it (replacing `range`), when there is a mechanical fix.
    pub replacement: Option<String>,
}

/// Punctuation that takes no space before it.
const CLOSING: &[char] = &[',', '.', ';', ':', '!', '?', ')', ']', '}', '…'];

/// Words that may legitimately be written twice: Vietnamese full reduplication (`từ từ`, `ngày ngày`,
/// `người người`) and English `had had`, `that that`.
const DOUBLE_OK: &[&str] = &[
    "ai",
    "bao",
    "chiều",
    "dần",
    "đâu",
    "đời",
    "đêm",
    "gì",
    "hà",
    "lâu",
    "mãi",
    "năm",
    "ngày",
    "nhà",
    "nào",
    "người",
    "rồi",
    "sáng",
    "song",
    "tháng",
    "thường",
    "từ",
    "xa",
    "xưa",
    "had",
    "that",
];

/// Abbreviations ending with a full stop after which a sentence goes on (`v.v. và`, `e.g. the`).
const ABBREVIATIONS: &[&str] = &[
    "v.v.", "vv.", "tp.", "tx.", "q.", "p.", "tr.", "ths.", "ts.", "pgs.", "gs.", "bs.", "ks.",
    "ông.", "e.g.", "i.e.", "etc.", "vs.", "mr.", "mrs.", "ms.", "dr.", "st.", "no.", "fig.",
    "cf.", "approx.",
];

/// Byte ranges of the words (letters, digits and inner apostrophes / hyphens) of `text`.
fn word_spans(text: &str) -> Vec<Range<usize>> {
    crate::words(text)
}

/// The word (without trailing dots) that ends right before byte `end`, plus its start.
fn token_before(text: &str, end: usize) -> &str {
    let head = &text[..end];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace() || "([{\"“".contains(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    &head[start..]
}

fn capitalized(word: &str) -> String {
    let mut cs = word.chars();
    cs.next()
        .map(|c| c.to_uppercase().chain(cs).collect())
        .unwrap_or_default()
}

/// Issues of one paragraph, without the tone style rule (that one needs the whole document: see
/// [`tone_style_issues`]).
#[must_use]
pub fn check(text: &str) -> Vec<Issue> {
    let mut out = Vec::new();
    spaces(text, &mut out);
    capitals(text, &mut out);
    repeated(text, &mut out);
    balance(text, &mut out);
    out.sort_by_key(|i| (i.range.start, i.range.end));
    out
}

fn spaces(text: &str, out: &mut Vec<Issue>) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        if c == ' ' {
            let mut j = i;
            while j < chars.len() && chars[j].1 == ' ' {
                j += 1;
            }
            let end = chars.get(j).map_or(text.len(), |x| x.0);
            let next = chars.get(j).map(|x| x.1);
            // Spaces at the start or end of the paragraph are left alone (indentation, trailing).
            if i > 0 && next.is_some() {
                let next_c = next.unwrap_or(' ');
                let before_ellipsis = text[end..].starts_with("...");
                // `[ ]`, `( )` are drawn boxes; ` .docx`, ` .NET` are names, not punctuation.
                let after_open =
                    chars[i - 1].1 == '(' || chars[i - 1].1 == '[' || chars[i - 1].1 == '{';
                let glued = text[end + next_c.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
                let name = next_c == '.' && glued;
                if CLOSING.contains(&next_c)
                    && !before_ellipsis
                    && next_c != '…'
                    && !after_open
                    && !name
                {
                    let punct_end = end + next_c.len_utf8();
                    out.push(Issue {
                        range: at..punct_end,
                        rule: Rule::SpaceBeforePunctuation,
                        // "a ,b" → "a, b"
                        replacement: Some(
                            if glued && next_c != ')' && next_c != ']' && next_c != '}' {
                                format!("{next_c} ")
                            } else {
                                next_c.to_string()
                            },
                        ),
                    });
                } else if j - i >= 2 && next_c != '\t' {
                    out.push(Issue {
                        range: at..end,
                        rule: Rule::ExtraSpace,
                        replacement: Some(" ".into()),
                    });
                }
            }
            i = j;
            continue;
        }
        // Missing space after punctuation.
        if let (Some(&(_, prev)), Some(&(_, next))) = (
            i.checked_sub(1).and_then(|k| chars.get(k)),
            chars.get(i + 1),
        ) {
            let missing = match c {
                // 1,5 and 3;4 are numbers; a,b without space is a mistake.
                ',' | ';' => prev.is_alphabetic() && next.is_alphabetic(),
                // a.Next sentence; not U.S, e.g, file.txt, www.example.com.
                '.' | '!' | '?' => {
                    prev.is_lowercase()
                        && next.is_uppercase()
                        && token_before(text, at).chars().filter(|c| c.is_alphabetic()).count() > 1
                        && !token_before(text, at).contains('.')
                        // BlueOffice.Api, System.Text: code names.
                        && !token_before(text, at).chars().skip(1).any(char::is_uppercase)
                }
                _ => false,
            };
            if missing {
                out.push(Issue {
                    range: at..at + c.len_utf8(),
                    rule: Rule::MissingSpace,
                    replacement: Some(format!("{c} ")),
                });
            }
        }
        i += 1;
    }
}

fn capitals(text: &str, out: &mut Vec<Issue>) {
    let spans = word_spans(text);
    for (n, w) in spans.iter().enumerate() {
        let word = &text[w.clone()];
        let first = word.chars().next().unwrap_or(' ');
        if !first.is_lowercase() || word.chars().any(char::is_uppercase) {
            continue; // iPhone, eBay: mixed case is deliberate
        }
        let before = text[..w.start].trim_end_matches([' ', '"', '“', '(', '\'', '‘']);
        let starts_sentence = if n == 0 {
            // Paragraph start, unless it is a list marker (a) b. …) or the text begins with a symbol.
            before.is_empty() && !text[w.end..].starts_with([')', '.'])
        } else {
            let gap = &text[before.len()..w.start];
            let ends = before.ends_with(['.', '!', '?']) && !before.ends_with("...");
            let abbreviation = before.ends_with('.')
                && ABBREVIATIONS
                    .contains(&token_before(text, before.len()).to_lowercase().as_str());
            // a.b. / U.S. — any dotted token is an abbreviation; numbers (1.5., 2.) are list items.
            let token = token_before(text, before.len());
            let dotted = token.trim_end_matches('.').contains('.')
                || token.chars().next().is_some_and(|c| c.is_ascii_digit());
            ends && !abbreviation && !dotted && !gap.is_empty() && gap.starts_with(' ')
        };
        if starts_sentence {
            out.push(Issue {
                range: w.clone(),
                rule: Rule::Capitalize,
                replacement: Some(capitalized(word)),
            });
        }
    }
}

fn repeated(text: &str, out: &mut Vec<Issue>) {
    let spans = word_spans(text);
    for pair in spans.windows(2) {
        let (a, b) = (&text[pair[0].clone()], &text[pair[1].clone()]);
        let gap = &text[pair[0].end..pair[1].start];
        if gap.is_empty() || !gap.chars().all(|c| c == ' ') {
            continue;
        }
        if !a.chars().any(char::is_alphabetic) || a.to_lowercase() != b.to_lowercase() {
            continue;
        }
        if DOUBLE_OK.contains(&a.to_lowercase().as_str()) {
            continue;
        }
        out.push(Issue {
            range: pair[0].start..pair[1].end,
            rule: Rule::RepeatedWord,
            replacement: Some(a.to_owned()),
        });
    }
}

fn balance(text: &str, out: &mut Vec<Issue>) {
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut straight: Vec<usize> = Vec::new();
    let flag = |at: usize, c: char, out: &mut Vec<Issue>| {
        out.push(Issue {
            range: at..at + c.len_utf8(),
            rule: Rule::Unbalanced,
            replacement: None,
        });
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(at, c)) in chars.iter().enumerate() {
        match c {
            '(' | '[' | '{' | '“' => stack.push((c, at)),
            ')' | ']' | '}' | '”' => {
                let open = match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '“',
                };
                // 1) 2) a) list markers close nothing.
                let marker = c == ')'
                    && k > 0
                    && chars[..k]
                        .iter()
                        .rev()
                        .take_while(|(_, x)| !x.is_whitespace())
                        .all(|(_, x)| x.is_alphanumeric())
                    && chars[..k]
                        .iter()
                        .rev()
                        .take_while(|(_, x)| !x.is_whitespace())
                        .count()
                        <= 3
                    && !stack.iter().any(|(o, _)| *o == '(');
                if let Some(p) = stack.iter().rposition(|(o, _)| *o == open) {
                    stack.truncate(p);
                } else if !marker {
                    flag(at, c, out);
                }
            }
            '"' if straight.pop().is_none() => straight.push(at),
            _ => {}
        }
    }
    for (c, at) in stack {
        flag(at, c, out);
    }
    for at in straight {
        flag(at, '"', out);
    }
}

/// Vietnamese words of `text` whose tone placement has a style, with that style.
#[must_use]
pub fn tone_styles(text: &str) -> Vec<(Range<usize>, ToneStyle)> {
    word_spans(text)
        .into_iter()
        .filter_map(|r| vi::tone_style(&text[r.clone()]).map(|s| (r, s)))
        .collect()
}

/// The words written in the other style than `style`, with the respelling.
#[must_use]
pub fn tone_style_issues(text: &str, style: ToneStyle) -> Vec<Issue> {
    tone_styles(text)
        .into_iter()
        .filter(|(_, s)| *s != style)
        .filter_map(|(r, _)| {
            vi::restyle(&text[r.clone()], style).map(|fixed| Issue {
                range: r,
                rule: Rule::ToneStyle,
                replacement: Some(fixed),
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// (rule id, flagged text, replacement).
    type Found = (&'static str, String, Option<String>);

    /// (rule, flagged text, replacement) of every issue.
    fn found(text: &str) -> Vec<Found> {
        check(text)
            .into_iter()
            .map(|i| (i.rule.id(), text[i.range].to_owned(), i.replacement))
            .collect()
    }

    fn one(rule: &'static str, flagged: &str, fix: Option<&str>) -> Vec<Found> {
        vec![(rule, flagged.to_owned(), fix.map(str::to_owned))]
    }

    // source: test table written for this project from the conventions in the module documentation.
    #[test]
    fn rule_table() {
        let cases: &[(&str, Vec<Found>)] = &[
            // Correct text gives nothing.
            ("Hôm nay trời đẹp. Chúng tôi đi học, rồi về nhà!", vec![]),
            (
                "The cat sat (on the mat). It was 1,5 m long; really.",
                vec![],
            ),
            ("a) Mục thứ nhất, b) mục thứ hai.", vec![]),
            ("Các loại quả: cam, quýt, v.v. và nhiều loại khác.", vec![]),
            ("Xem e.g. the manual, U.S. law and www.example.com.", vec![]),
            ("Đi từ từ thôi, ngày ngày đều thế.", vec![]),
            ("He said \"yes\" and left… then came back...", vec![]),
            ("    Thụt đầu dòng bằng dấu cách.", vec![]),
            ("1. bước một", vec![]),
            (
                "Ô `[ ]` chưa làm, mở tệp .docx hoặc gói BlueOffice.Api.",
                vec![],
            ),
            ("Hai việc chạy song song.", vec![]),
            // Extra spaces.
            ("Xin  chào", one("extraSpace", "  ", Some(" "))),
            // Space before punctuation.
            (
                "Xin chào , bạn",
                one("spaceBeforePunctuation", " ,", Some(",")),
            ),
            ("Thật sao ?", one("spaceBeforePunctuation", " ?", Some("?"))),
            // Missing space after punctuation.
            ("Có cam,quýt", one("missingSpace", ",", Some(", "))),
            ("Xong rồi.Bây giờ", one("missingSpace", ".", Some(". "))),
            // Capital letter at the start of a sentence / paragraph.
            ("hôm nay", one("capitalize", "hôm", Some("Hôm"))),
            (
                "Tốt. bây giờ thì sao?",
                one("capitalize", "bây", Some("Bây")),
            ),
            ("Is it? yes.", one("capitalize", "yes", Some("Yes"))),
            // Repeated word.
            (
                "Tôi tôi đi học",
                one("repeatedWord", "Tôi tôi", Some("Tôi")),
            ),
            (
                "This is the the end.",
                one("repeatedWord", "the the", Some("the")),
            ),
            // Unbalanced brackets and quotes.
            ("Ghi chú (xem thêm.", one("unbalanced", "(", None)),
            ("Ghi chú xem thêm).", one("unbalanced", ")", None)),
            ("Anh nói “đi thôi.", one("unbalanced", "“", None)),
            ("He said \"go.", one("unbalanced", "\"", None)),
        ];
        for (text, want) in cases {
            assert_eq!(&found(text), want, "{text}");
        }
    }

    #[test]
    fn several_issues_in_order() {
        let got = found("xin  chào ,bạn bạn");
        let rules: Vec<&str> = got.iter().map(|g| g.0).collect();
        assert_eq!(
            rules,
            [
                "capitalize",
                "extraSpace",
                "spaceBeforePunctuation",
                "repeatedWord"
            ]
        );
    }

    // source: older (hòa, thủy) and newer (hoà, thuỷ) placements of the Vietnamese orthography.
    #[test]
    fn tone_style_follows_the_document() {
        let text = "Hoà bình, khoẻ mạnh và thủy chung.";
        let styles = tone_styles(text);
        assert_eq!(styles.len(), 3);
        let issues = tone_style_issues(text, ToneStyle::New);
        assert_eq!(issues.len(), 1);
        assert_eq!(&text[issues[0].range.clone()], "thủy");
        assert_eq!(issues[0].replacement.as_deref(), Some("thuỷ"));
        let issues = tone_style_issues(text, ToneStyle::Old);
        let fixed: Vec<_> = issues
            .iter()
            .map(|i| i.replacement.clone().unwrap())
            .collect();
        assert_eq!(fixed, ["Hòa", "khỏe"]);
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_ranges_are_valid(s in "\\PC{0,60}") {
            for i in check(&s) {
                proptest::prop_assert!(s.is_char_boundary(i.range.start) && s.is_char_boundary(i.range.end));
                proptest::prop_assert!(i.range.start < i.range.end);
            }
            let _ = tone_style_issues(&s, ToneStyle::New);
        }
    }
}

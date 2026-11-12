//! Flash Fill: learn a text transformation from a few examples and apply it to
//! the remaining rows (programming by example).
//!
//! Written from first principles. Every source string is split into tokens of
//! several kinds (words, digit runs, letter-and-digit runs, fields between a
//! delimiter, all digits, all letters, the whole text). An *atom* picks one
//! token (counted from the start or from the end), optionally keeps only its
//! first or last characters and optionally changes its case. A *program* is a
//! concatenation of atoms and literal text.
//!
//! An atom may also *repeat* over every token of its kind (initials of every
//! word, however many words a name has), pick a token *nested* inside a field
//! (the first label of the domain of an e-mail address) and *convert* a number
//! (leading zeros dropped or padded to two digits, month 1–12 written as a
//! Vietnamese or English month name). Month names in the sources ("tháng Ba",
//! "tháng 3", "March", "Mar") form their own token kind holding the month
//! number, so dates written in words can be rewritten as numbers and back.
//!
//! For each example the ways of producing its output form a graph whose nodes
//! are character positions in the output and whose edges are atoms or literal
//! characters. The graphs of all examples are walked together (a node is one
//! position per example, an edge must be valid in every example), and the
//! cheapest path from the start to the end is the learned program. Costs prefer
//! few, simple atoms and penalise literal letters and digits, so a program that
//! explains the output by the source wins over one that copies the example.
//!
//! When several cheapest programs exist and they disagree on a row to fill, the
//! pattern is ambiguous and no result is returned; likewise when no program fits
//! every example or the only fit is constant text.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use unicode_normalization::UnicodeNormalization;

/// Kinds of token a source string is split into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// The whole text, trimmed.
    Whole,
    /// A run of letters (Unicode, combining marks included).
    Word,
    /// A run of decimal digits.
    Digits,
    /// A run of letters and digits.
    Alnum,
    /// The text between two occurrences of a delimiter, trimmed.
    Field(char),
    /// Every digit of the text, joined.
    AllDigits,
    /// Every letter of the text, joined.
    AllLetters,
    /// A month written in words ("tháng Ba", "tháng Mười Hai", "tháng 3",
    /// "March", "Mar"), as its number `1`–`12`.
    Month,
}

/// Number conversion applied to a token before its characters are taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Conv {
    /// Unchanged.
    Same,
    /// A digit run as a number: leading zeros dropped (`"03"` → `"3"`).
    Number,
    /// A number below 100 padded to two digits (`"3"` → `"03"`).
    Pad2,
    /// Month 1–12 as its Vietnamese name ("Một" … "Mười Hai").
    MonthVi,
    /// Month 1–12 as its English name ("January" …).
    MonthEn,
    /// Month 1–12 as its three-letter English abbreviation ("Jan" …).
    MonthEnShort,
}

/// How a repeating atom joins the texts of the tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Join {
    /// Directly (`"NVA"`).
    Concat,
    /// With a separator character (`"N.V.A"`, `"ha-noi"`).
    With(char),
}

/// Case change applied to an atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Case {
    /// Unchanged.
    Same,
    /// Lower case.
    Lower,
    /// Upper case.
    Upper,
    /// First letter of each word upper case, the rest lower case.
    Title,
}

/// Part of a token kept by an atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Take {
    /// The whole token.
    All,
    /// The first `n` characters.
    Prefix(usize),
    /// The last `n` characters.
    Suffix(usize),
}

/// One token of one source column, transformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom {
    /// Source column (index into the inputs of a row).
    pub col: usize,
    /// Kind of token.
    pub kind: TokenKind,
    /// Token index: `0, 1, …` from the start, `-1, -2, …` from the end.
    pub index: i32,
    /// Characters kept.
    pub take: Take,
    /// Case change.
    pub case: Case,
    /// Number conversion applied to the token first.
    pub conv: Conv,
    /// A token inside the selected one (kind, index): `Field('@')[-1]` then
    /// `Field('.')[0]` is the first label of a domain.
    pub inner: Option<(TokenKind, i32)>,
    /// Repeat over every token of the kind (index ignored), joined.
    pub repeat: Option<Join>,
}

impl Atom {
    /// A plain atom: one token, no conversion, nesting or repetition.
    #[must_use]
    pub fn flat(col: usize, kind: TokenKind, index: i32, take: Take, case: Case) -> Atom {
        Atom {
            col,
            kind,
            index,
            take,
            case,
            conv: Conv::Same,
            inner: None,
            repeat: None,
        }
    }
}

/// A piece of a program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Part {
    /// Constant text.
    Literal(String),
    /// Text taken from a source.
    Atom(Atom),
}

/// A learned string program: the concatenation of its parts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Program {
    /// The parts, in order.
    pub parts: Vec<Part>,
}

/// One example: the source texts of a row and the output the user typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// Source texts (one per source column).
    pub inputs: Vec<String>,
    /// The expected output.
    pub output: String,
}

impl Example {
    /// An example from string slices.
    #[must_use]
    pub fn new(inputs: &[&str], output: &str) -> Example {
        Example {
            inputs: inputs.iter().map(|s| (*s).to_owned()).collect(),
            output: output.to_owned(),
        }
    }
}

/// Delimiters for [`TokenKind::Field`].
const DELIMITERS: [char; 10] = [' ', ',', ';', '.', '-', '_', '@', '/', '|', ':'];
/// Longest prefix / suffix an atom keeps.
const MAX_TAKE: usize = 8;
/// Tokens of one kind considered per source.
const MAX_TOKENS: usize = 32;
/// Longest source / output text considered (characters).
const MAX_LEN: usize = 512;
/// Longest prefix / suffix a repeating atom keeps from each token.
const MAX_REPEAT_TAKE: usize = 3;
/// Separators a repeating atom may join tokens with.
const JOINS: [Join; 7] = [
    Join::Concat,
    Join::With(' '),
    Join::With('.'),
    Join::With('-'),
    Join::With('_'),
    Join::With(','),
    Join::With('/'),
];
/// Vietnamese month names, capitalised as in a date written out in words.
const MONTHS_VI: [&str; 12] = [
    "Một",
    "Hai",
    "Ba",
    "Tư",
    "Năm",
    "Sáu",
    "Bảy",
    "Tám",
    "Chín",
    "Mười",
    "Mười Một",
    "Mười Hai",
];
/// English month names.
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
/// Search budget (graph nodes expanded).
const MAX_EXPANSIONS: usize = 200_000;
/// Cheapest programs compared for ambiguity.
const MAX_PROGRAMS: usize = 64;

fn is_letter(c: char) -> bool {
    // Combining diacritics (U+0300–U+036F) belong to the letter they follow.
    c.is_alphabetic() || ('\u{300}'..='\u{36f}').contains(&c)
}

fn normalize(s: &str) -> String {
    // Vietnamese text may arrive decomposed; compare in NFC.
    s.nfc().collect()
}

/// Month number of a Vietnamese month word after "tháng" (lower case; the
/// traditional "giêng" / "chạp" for 1 / 12 and "bốn" / "tư" for 4 accepted).
fn month_vi(word: &str, next: Option<&str>) -> Option<(u32, usize)> {
    let n = match word {
        "một" | "giêng" => 1,
        "hai" => 2,
        "ba" => 3,
        "tư" | "bốn" => 4,
        "năm" => 5,
        "sáu" => 6,
        "bảy" => 7,
        "tám" => 8,
        "chín" => 9,
        "mười" => {
            return Some(match next {
                Some("một") => (11, 2),
                Some("hai") => (12, 2),
                _ => (10, 1),
            })
        }
        "chạp" => 12,
        _ => {
            let n: u32 = word.parse().ok()?;
            return (word.chars().all(|c| c.is_ascii_digit()) && (1..=12).contains(&n))
                .then_some((n, 1));
        }
    };
    Some((n, 1))
}

/// Months written in words in `s`, as numbers: "tháng" followed by a month
/// word or number, and English month names or their abbreviations.
fn month_tokens(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let words = runs(&lower, |c| is_letter(c) || c.is_ascii_digit());
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].as_str();
        if w == "tháng" {
            if let Some((n, used)) = words
                .get(i + 1)
                .and_then(|m| month_vi(m, words.get(i + 2).map(String::as_str)))
            {
                out.push(n.to_string());
                i += 1 + used;
                continue;
            }
        } else if let Some(k) = MONTHS_EN.iter().position(|m| {
            let m = m.to_lowercase();
            w == m || (w.len() == 3 && m.starts_with(w)) || (w == "sept" && m == "september")
        }) {
            out.push((k + 1).to_string());
        }
        i += 1;
    }
    out
}

/// The number conversion of a token, `None` when it does not apply.
fn convert(tok: &str, conv: Conv) -> Option<String> {
    if conv == Conv::Same {
        return Some(tok.to_owned());
    }
    if tok.is_empty() || tok.len() > 18 || !tok.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: u64 = tok.parse().ok()?;
    let month = || usize::try_from(n).ok().filter(|m| (1..=12).contains(m));
    Some(match conv {
        Conv::Same | Conv::Number => n.to_string(),
        Conv::Pad2 => {
            if n >= 100 {
                return None;
            }
            format!("{n:02}")
        }
        Conv::MonthVi => MONTHS_VI[month()? - 1].to_owned(),
        Conv::MonthEn => MONTHS_EN[month()? - 1].to_owned(),
        Conv::MonthEnShort => MONTHS_EN[month()? - 1].chars().take(3).collect(),
    })
}

/// Token `index` (from the start, or from the end when negative).
fn pick(toks: &[String], index: i32) -> Option<&String> {
    let i = if index >= 0 {
        usize::try_from(index).ok()?
    } else {
        toks.len()
            .checked_sub(usize::try_from(index.unsigned_abs()).ok()?)?
    };
    toks.get(i)
}

fn index_cost(index: i32) -> u32 {
    // 0, 1, 2 … → 0, 2, 4 …; -1, -2 … → 1, 3 …: the last token is nearly
    // as natural as the first.
    if index >= 0 {
        index.unsigned_abs() * 2
    } else {
        index.unsigned_abs() * 2 - 1
    }
}

fn kind_cost(kind: TokenKind) -> u32 {
    match kind {
        TokenKind::Whole => 20,
        TokenKind::Word | TokenKind::Digits => 22,
        TokenKind::Field(_) | TokenKind::Month => 24,
        TokenKind::AllDigits => 25,
        TokenKind::Alnum => 26,
        TokenKind::AllLetters => 28,
    }
}

fn runs(s: &str, keep: impl Fn(char) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if keep(c) {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The tokens of `s` of one kind (empty when the kind does not apply).
fn tokens(s: &str, kind: TokenKind) -> Vec<String> {
    let mut v = match kind {
        TokenKind::Whole => {
            let t = s.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_owned()]
            }
        }
        TokenKind::Word => runs(s, is_letter),
        TokenKind::Digits => runs(s, |c| c.is_ascii_digit()),
        TokenKind::Alnum => runs(s, |c| is_letter(c) || c.is_ascii_digit()),
        TokenKind::Field(d) => {
            if s.contains(d) {
                s.split(d).map(|f| f.trim().to_owned()).collect()
            } else {
                Vec::new()
            }
        }
        TokenKind::AllDigits => {
            let t: String = s.chars().filter(char::is_ascii_digit).collect();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t]
            }
        }
        TokenKind::AllLetters => {
            let t: String = s.chars().filter(|&c| is_letter(c)).collect();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t]
            }
        }
        TokenKind::Month => month_tokens(s),
    };
    v.truncate(MAX_TOKENS);
    v
}

fn single(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whole | TokenKind::AllDigits | TokenKind::AllLetters
    )
}

fn apply_case(s: &str, case: Case) -> String {
    match case {
        Case::Same => s.to_owned(),
        Case::Lower => s.to_lowercase(),
        Case::Upper => s.to_uppercase(),
        Case::Title => {
            let mut out = String::with_capacity(s.len());
            let mut prev_letter = false;
            for c in s.chars() {
                if prev_letter {
                    out.extend(c.to_lowercase());
                } else {
                    out.extend(c.to_uppercase());
                }
                prev_letter = is_letter(c);
            }
            out
        }
    }
}

fn apply_take(s: &str, take: Take) -> String {
    match take {
        Take::All => s.to_owned(),
        Take::Prefix(n) => s.chars().take(n).collect(),
        Take::Suffix(n) => {
            let len = s.chars().count();
            s.chars().skip(len.saturating_sub(n)).collect()
        }
    }
}

impl Atom {
    /// The text this atom produces from the sources of a row, `None` when the
    /// token does not exist there.
    #[must_use]
    pub fn eval(&self, inputs: &[String]) -> Option<String> {
        let src = normalize(inputs.get(self.col)?);
        let toks = tokens(&src, self.kind);
        match self.repeat {
            None => self.transform(pick(&toks, self.index)?),
            Some(join) => {
                let mut out = String::new();
                for tok in &toks {
                    let t = self.transform(tok)?;
                    if t.is_empty() {
                        continue;
                    }
                    if let (Join::With(c), false) = (join, out.is_empty()) {
                        out.push(c);
                    }
                    out.push_str(&t);
                }
                (!out.is_empty()).then_some(out)
            }
        }
    }

    /// Nested token, conversion, characters kept and case of one token.
    fn transform(&self, tok: &str) -> Option<String> {
        let nested;
        let tok = match self.inner {
            Some((kind, index)) => {
                nested = pick(&tokens(tok, kind), index)?.clone();
                nested.as_str()
            }
            None => tok,
        };
        let tok = convert(tok, self.conv)?;
        Some(apply_case(&apply_take(&tok, self.take), self.case))
    }

    fn cost(&self) -> u32 {
        let take = match self.take {
            Take::All => 0,
            Take::Prefix(_) => 5,
            Take::Suffix(_) => 6,
        };
        let case = if self.case == Case::Same { 0 } else { 3 };
        let conv = match self.conv {
            Conv::Same => 0,
            Conv::Number => 4,
            Conv::Pad2 => 5,
            Conv::MonthVi | Conv::MonthEn | Conv::MonthEnShort => 6,
        };
        // A nested token costs more than any plain one, so it is only chosen
        // when no plain token explains the output.
        let inner = self
            .inner
            .map_or(0, |(k, i)| 12 + kind_cost(k) / 4 + index_cost(i));
        // Repeating: no index, a fixed premium (a separator one more).
        let (index, repeat) = match self.repeat {
            None => (index_cost(self.index), 0),
            Some(Join::Concat) => (0, 8),
            Some(Join::With(_)) => (0, 9),
        };
        kind_cost(self.kind) + index + take + case + conv + inner + repeat
    }
}

fn literal_cost(c: char) -> u32 {
    if is_letter(c) || c.is_alphanumeric() {
        30
    } else {
        6
    }
}

/// Extra cost of an atom whose text starts or ends inside a word of the
/// output (`"n"` of `"Ông"` taken from `"An"` is a coincidence, not a
/// pattern). An upper-case letter after a letter starts a new word (initials).
fn split_penalty(out: &str, i: usize, j: usize) -> u32 {
    let same_word = |a: char, b: char| {
        (is_letter(a) && is_letter(b) && !b.is_uppercase())
            || (a.is_ascii_digit() && b.is_ascii_digit())
    };
    let mut p = 0;
    if let (Some(a), Some(b)) = (out[..i].chars().next_back(), out[i..].chars().next()) {
        if same_word(a, b) {
            p += 15;
        }
    }
    if let (Some(a), Some(b)) = (out[..j].chars().next_back(), out[j..].chars().next()) {
        if same_word(a, b) {
            p += 15;
        }
    }
    p
}

impl Program {
    /// Runs the program on the sources of a row (`None` when an atom's token
    /// is missing there).
    #[must_use]
    pub fn run(&self, inputs: &[String]) -> Option<String> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                Part::Literal(s) => out.push_str(s),
                Part::Atom(a) => out.push_str(&a.eval(inputs)?),
            }
        }
        Some(out)
    }

    fn has_atom(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, Part::Atom(_)))
    }
}

/// Every atom whose text occurs in `output`, with the byte span it covers.
fn candidates(inputs: &[String], output: &str) -> Vec<(usize, usize, Atom)> {
    let mut found = Vec::new();
    let mut kinds = vec![
        TokenKind::Whole,
        TokenKind::Word,
        TokenKind::Digits,
        TokenKind::Alnum,
        TokenKind::AllDigits,
        TokenKind::AllLetters,
        TokenKind::Month,
    ];
    kinds.extend(DELIMITERS.iter().map(|&d| TokenKind::Field(d)));
    let mut seen = std::collections::HashSet::new();
    let mut add = |found: &mut Vec<(usize, usize, Atom)>, s: &str, atom: Atom| {
        if s.is_empty() {
            return;
        }
        // Every occurrence, overlapping ones included.
        let mut from = 0;
        while let Some(p) = output[from..].find(s) {
            let at = from + p;
            if seen.insert((at, atom)) {
                found.push((at, at + s.len(), atom));
            }
            from = at + output[at..].chars().next().map_or(1, char::len_utf8);
            if from >= output.len() {
                break;
            }
        }
    };
    for (col, src) in inputs.iter().enumerate() {
        let src = normalize(src);
        if src.chars().count() > MAX_LEN {
            continue;
        }
        for &kind in &kinds {
            let toks = tokens(&src, kind);
            let n = toks.len();
            for (k, tok) in toks.iter().enumerate() {
                let len = tok.chars().count();
                let mut takes = vec![Take::All];
                for t in 1..len.min(MAX_TAKE + 1) {
                    takes.push(Take::Prefix(t));
                    takes.push(Take::Suffix(t));
                }
                let mut indices = Vec::with_capacity(2);
                indices.extend(i32::try_from(k).ok());
                if !single(kind) {
                    indices.extend(i32::try_from(n - k).ok().map(|m| -m));
                }
                for &take in &takes {
                    let part = apply_take(tok, take);
                    for case in CASES {
                        let s = apply_case(&part, case);
                        for &index in &indices {
                            add(&mut found, &s, Atom::flat(col, kind, index, take, case));
                        }
                    }
                }
                // Numbers converted (whole token only).
                if matches!(kind, TokenKind::Digits | TokenKind::Month) {
                    for conv in CONVS {
                        let Some(c) = convert(tok, conv) else {
                            continue;
                        };
                        for case in CASES {
                            let s = apply_case(&c, case);
                            for &index in &indices {
                                let mut atom = Atom::flat(col, kind, index, Take::All, case);
                                atom.conv = conv;
                                add(&mut found, &s, atom);
                            }
                        }
                    }
                }
                // A token nested inside a field (whole inner token only).
                if let TokenKind::Field(d) = kind {
                    for inner_kind in inner_kinds(d) {
                        let inner = tokens(tok, inner_kind);
                        let m = inner.len();
                        for (j, it) in inner.iter().enumerate() {
                            let mut inner_idx = Vec::with_capacity(2);
                            inner_idx.extend(i32::try_from(j).ok());
                            inner_idx.extend(i32::try_from(m - j).ok().map(|x| -x));
                            for case in CASES {
                                let s = apply_case(it, case);
                                for &index in &indices {
                                    for &ii in &inner_idx {
                                        let mut atom =
                                            Atom::flat(col, kind, index, Take::All, case);
                                        atom.inner = Some((inner_kind, ii));
                                        add(&mut found, &s, atom);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Every token of the kind, joined (initials of every word).
            if n >= 2 && !single(kind) && kind != TokenKind::Month {
                let mut takes = vec![Take::All];
                for t in 1..=MAX_REPEAT_TAKE {
                    takes.push(Take::Prefix(t));
                    takes.push(Take::Suffix(t));
                }
                for &take in &takes {
                    for case in CASES {
                        for join in JOINS {
                            let mut atom = Atom::flat(0, kind, 0, take, case);
                            atom.repeat = Some(join);
                            if let Some(s) = atom.eval(std::slice::from_ref(&src)) {
                                atom.col = col;
                                add(&mut found, &s, atom);
                            }
                        }
                    }
                }
            }
        }
    }
    found
}

const CASES: [Case; 4] = [Case::Same, Case::Lower, Case::Upper, Case::Title];
const CONVS: [Conv; 5] = [
    Conv::Number,
    Conv::Pad2,
    Conv::MonthVi,
    Conv::MonthEn,
    Conv::MonthEnShort,
];

/// Token kinds looked for inside a field split on `d`.
fn inner_kinds(d: char) -> Vec<TokenKind> {
    let mut v = vec![TokenKind::Word, TokenKind::Digits, TokenKind::Alnum];
    v.extend(
        DELIMITERS
            .iter()
            .filter(|&&e| e != d)
            .map(|&e| TokenKind::Field(e)),
    );
    v
}

/// An edge of the joint graph.
#[derive(Clone)]
enum Edge {
    /// One literal character.
    Char(char),
    /// Candidate atom (index into the candidate list).
    Atom(usize),
}

/// The cheapest programs consistent with every example (several when tied),
/// or `None` when no program that uses a source fits them all.
#[must_use]
pub fn learn(examples: &[Example]) -> Option<Vec<Program>> {
    let first = examples.first()?;
    let outputs: Vec<String> = examples.iter().map(|e| normalize(&e.output)).collect();
    if outputs
        .iter()
        .any(|o| o.is_empty() || o.chars().count() > MAX_LEN)
    {
        return None;
    }
    let cands = candidates(&first.inputs, &outputs[0]);
    // What each candidate produces in the other examples.
    let evals: Vec<Vec<Option<String>>> = cands
        .iter()
        .map(|(_, _, a)| examples.iter().skip(1).map(|e| a.eval(&e.inputs)).collect())
        .collect();
    let mut by_start: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, (s, _, _)) in cands.iter().enumerate() {
        by_start.entry(*s).or_default().push(i);
    }

    let start: Vec<usize> = vec![0; examples.len()];
    let goal: Vec<usize> = outputs.iter().map(String::len).collect();
    let mut ids: HashMap<Vec<usize>, usize> = HashMap::new();
    let mut nodes: Vec<Vec<usize>> = Vec::new();
    let mut dist: Vec<u32> = Vec::new();
    let mut preds: Vec<Vec<(usize, Edge)>> = Vec::new();
    let mut done: Vec<bool> = Vec::new();
    let mut node_id = |pos: Vec<usize>,
                       nodes: &mut Vec<Vec<usize>>,
                       dist: &mut Vec<u32>,
                       preds: &mut Vec<Vec<(usize, Edge)>>,
                       done: &mut Vec<bool>| {
        *ids.entry(pos.clone()).or_insert_with(|| {
            nodes.push(pos);
            dist.push(u32::MAX);
            preds.push(Vec::new());
            done.push(false);
            nodes.len() - 1
        })
    };
    let s = node_id(start, &mut nodes, &mut dist, &mut preds, &mut done);
    dist[s] = 0;
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((0u32, s)));
    let mut goal_id = None;
    let mut expansions = 0;
    while let Some(Reverse((d, n))) = heap.pop() {
        if done[n] || d > dist[n] {
            continue;
        }
        done[n] = true;
        if nodes[n] == goal {
            goal_id = Some(n);
            break;
        }
        expansions += 1;
        if expansions > MAX_EXPANSIONS {
            return None;
        }
        let pos = nodes[n].clone();
        let mut edges: Vec<(Vec<usize>, u32, Edge)> = Vec::new();
        // One literal character, the same in every example.
        if let Some(c) = outputs[0][pos[0]..].chars().next() {
            if outputs
                .iter()
                .zip(&pos)
                .all(|(o, &p)| o[p..].starts_with(c))
            {
                let next = pos.iter().map(|p| p + c.len_utf8()).collect();
                edges.push((next, literal_cost(c), Edge::Char(c)));
            }
        }
        // Atoms that continue every example's output.
        for &ci in by_start.get(&pos[0]).map_or(&[][..], Vec::as_slice) {
            let (_, end0, atom) = cands[ci];
            let mut next = vec![end0];
            let fits = evals[ci].iter().enumerate().all(|(e, v)| {
                let (o, p) = (&outputs[e + 1], pos[e + 1]);
                match v {
                    Some(t) if !t.is_empty() && o[p..].starts_with(t.as_str()) => {
                        next.push(p + t.len());
                        true
                    }
                    _ => false,
                }
            });
            if fits {
                // Part of a token cut at a space (" An" as the last three characters
                // of "Nguyễn Văn An") is a coincidence of lengths, not a pattern.
                let text = &outputs[0][pos[0]..end0];
                let cut_at_space = atom.take != Take::All
                    && (text.starts_with(char::is_whitespace)
                        || text.ends_with(char::is_whitespace));
                let w = atom.cost()
                    + split_penalty(&outputs[0], pos[0], end0)
                    + if cut_at_space { 15 } else { 0 };
                edges.push((next, w, Edge::Atom(ci)));
            }
        }
        for (next, w, edge) in edges {
            let m = node_id(next, &mut nodes, &mut dist, &mut preds, &mut done);
            let nd = d + w;
            if nd < dist[m] {
                dist[m] = nd;
                preds[m] = vec![(n, edge)];
                heap.push(Reverse((nd, m)));
            } else if nd == dist[m] && !done[m] {
                preds[m].push((n, edge));
            }
        }
    }
    let goal_id = goal_id?;

    // Every cheapest path, as programs (adjacent characters merged).
    let mut programs: Vec<Program> = Vec::new();
    let mut stack: Vec<(usize, Vec<Edge>)> = vec![(goal_id, Vec::new())];
    while let Some((n, path)) = stack.pop() {
        if programs.len() >= MAX_PROGRAMS {
            break;
        }
        if n == s {
            let mut parts: Vec<Part> = Vec::new();
            for e in path.iter().rev() {
                match e {
                    Edge::Char(c) => {
                        if let Some(Part::Literal(l)) = parts.last_mut() {
                            l.push(*c);
                        } else {
                            parts.push(Part::Literal(c.to_string()));
                        }
                    }
                    Edge::Atom(ci) => parts.push(Part::Atom(cands[*ci].2)),
                }
            }
            let p = Program { parts };
            if !programs.contains(&p) {
                programs.push(p);
            }
            continue;
        }
        for (prev, e) in &preds[n] {
            let mut next = path.clone();
            next.push(e.clone());
            stack.push((*prev, next));
        }
    }
    // A constant is not a pattern.
    programs.retain(Program::has_atom);
    if programs.is_empty() {
        None
    } else {
        Some(programs)
    }
}

/// Learns from `examples` and fills `rows` (the sources of each row to fill).
/// `None` when no consistent program exists or the cheapest programs disagree
/// on some row; otherwise one entry per row (`None` where the pattern does not
/// apply, e.g. a missing token).
#[must_use]
pub fn flash_fill(examples: &[Example], rows: &[Vec<String>]) -> Option<Vec<Option<String>>> {
    let programs = learn(examples)?;
    let (best, rest) = programs.split_first()?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let v = best.run(row);
        if rest.iter().any(|p| p.run(row) != v) {
            return None;
        }
        out.push(v.filter(|s| !s.is_empty()));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_words_table() {
        // (text, month numbers found) — worked out by hand.
        let table: &[(&str, &[&str])] = &[
            ("ngày 5 tháng Ba năm 2024", &["3"]),
            ("Tháng Mười Hai", &["12"]),
            ("tháng mười một", &["11"]),
            ("tháng Mười", &["10"]),
            ("tháng Giêng và tháng Chạp", &["1", "12"]),
            ("tháng tư / tháng Bốn", &["4", "4"]),
            ("Ngày 20 tháng 10 năm 2020", &["10"]),
            ("tháng 13", &[]),
            ("March 5, 2024", &["3"]),
            ("5 sep 1945, Sept", &["9", "9"]),
            ("năm 2024", &[]),
        ];
        for (text, want) in table {
            assert_eq!(&month_tokens(&normalize(text)), want, "{text}");
        }
    }

    #[test]
    fn conversions_table() {
        let table: &[(&str, Conv, Option<&str>)] = &[
            ("03", Conv::Number, Some("3")),
            ("0", Conv::Number, Some("0")),
            ("3", Conv::Pad2, Some("03")),
            ("123", Conv::Pad2, None),
            ("4", Conv::MonthVi, Some("Tư")),
            ("12", Conv::MonthVi, Some("Mười Hai")),
            ("13", Conv::MonthVi, None),
            ("09", Conv::MonthEn, Some("September")),
            ("5", Conv::MonthEnShort, Some("May")),
            ("ab", Conv::Number, None),
            ("ab", Conv::Same, Some("ab")),
        ];
        for (tok, conv, want) in table {
            assert_eq!(convert(tok, *conv).as_deref(), *want, "{tok} {conv:?}");
        }
    }

    #[test]
    fn repeating_and_nested_atoms() {
        let row = |s: &str| vec![s.to_owned()];
        let mut each = Atom::flat(0, TokenKind::Word, 0, Take::Prefix(1), Case::Upper);
        each.repeat = Some(Join::With('.'));
        assert_eq!(each.eval(&row("lê văn  hưu")).as_deref(), Some("L.V.H"));
        assert_eq!(each.eval(&row("")), None);
        let mut nested = Atom::flat(0, TokenKind::Field('@'), -1, Take::All, Case::Same);
        nested.inner = Some((TokenKind::Field('.'), 0));
        assert_eq!(nested.eval(&row("a@mail.gov.vn")).as_deref(), Some("mail"));
        assert_eq!(nested.eval(&row("a@localhost")), None);
        // Costs: nesting and repetition are dearer than the plain token.
        let plain = Atom::flat(0, TokenKind::Word, 0, Take::Prefix(1), Case::Upper);
        assert!(each.cost() > plain.cost());
        assert!(
            nested.cost() > Atom::flat(0, TokenKind::Field('@'), -1, Take::All, Case::Same).cost()
        );
    }
}

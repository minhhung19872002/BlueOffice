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
        let i = if self.index >= 0 {
            usize::try_from(self.index).ok()?
        } else {
            toks.len()
                .checked_sub(usize::try_from(self.index.unsigned_abs()).ok()?)?
        };
        let tok = toks.get(i)?;
        Some(apply_case(&apply_take(tok, self.take), self.case))
    }

    fn cost(&self) -> u32 {
        let kind = match self.kind {
            TokenKind::Whole => 20,
            TokenKind::Word | TokenKind::Digits => 22,
            TokenKind::Field(_) => 24,
            TokenKind::AllDigits => 25,
            TokenKind::Alnum => 26,
            TokenKind::AllLetters => 28,
        };
        // 0, 1, 2 … → 0, 2, 4 …; -1, -2 … → 1, 3 …: the last token is nearly
        // as natural as the first.
        let index = if self.index >= 0 {
            self.index.unsigned_abs() * 2
        } else {
            self.index.unsigned_abs() * 2 - 1
        };
        let take = match self.take {
            Take::All => 0,
            Take::Prefix(_) => 5,
            Take::Suffix(_) => 6,
        };
        let case = if self.case == Case::Same { 0 } else { 3 };
        kind + index + take + case
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
    ];
    kinds.extend(DELIMITERS.iter().map(|&d| TokenKind::Field(d)));
    let mut seen = std::collections::HashSet::new();
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
                    for case in [Case::Same, Case::Lower, Case::Upper, Case::Title] {
                        let s = apply_case(&part, case);
                        if s.is_empty() {
                            continue;
                        }
                        // Every occurrence, overlapping ones included.
                        let mut from = 0;
                        while let Some(p) = output[from..].find(s.as_str()) {
                            let at = from + p;
                            for &index in &indices {
                                let atom = Atom {
                                    col,
                                    kind,
                                    index,
                                    take,
                                    case,
                                };
                                if seen.insert((at, atom)) {
                                    found.push((at, at + s.len(), atom));
                                }
                            }
                            from = at + output[at..].chars().next().map_or(1, char::len_utf8);
                            if from >= output.len() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    found
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
                let w = atom.cost() + split_penalty(&outputs[0], pos[0], end0);
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

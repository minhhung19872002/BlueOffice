//! Find in the main text: case sensitivity, whole words, regular expressions,
//! special characters (`^p`, `^t`, `^l`, `^#` …), wildcards and formatting (bold,
//! italic, underline, style, font, colour, highlight).
//!
//! Search runs over *stories*: runs of consecutive paragraphs in the same container
//! (the body of a section, one table cell) whose index texts are joined by a
//! paragraph mark (`\n`), so `^p` matches across paragraph boundaries. Matches never
//! cross a container (a table cell end is not a paragraph mark).

mod pattern;

use crate::{EditError, Result};
use bo_model::index::{body_paragraphs, para_at, para_index, para_index_text, ParaPath};
use bo_model::{Document, Paragraph, RunProps, StyleKind};
use serde::{Deserialize, Serialize};

pub use pattern::{expand_replacement, translate_special, translate_wildcards};

/// Search options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindOptions {
    /// Compare case-sensitively.
    pub match_case: bool,
    /// Match whole words only.
    pub whole_word: bool,
    /// Treat the query as a regular expression (replacement uses `$1`).
    pub regex: bool,
    /// Wildcard pattern (`?`, `*`, `[a-z]`, `[!x]`, `{n,m}`, `@`, `<`, `>`, `( )`;
    /// replacement uses `\1`). Always case-sensitive; whole word is ignored.
    pub wildcards: bool,
}

/// A match: body paragraph id and byte range of its index text. A match running over
/// paragraph marks ends in paragraph `ep` (`end` is then an offset in that paragraph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Match {
    /// Body paragraph id.
    #[serde(rename = "p")]
    pub para: usize,
    /// Start byte offset in the index text.
    pub start: usize,
    /// End byte offset (exclusive) in the index text of the end paragraph.
    pub end: usize,
    /// End paragraph when it is not `para` (the match contains paragraph marks).
    #[serde(rename = "ep", default, skip_serializing_if = "Option::is_none")]
    pub end_para: Option<usize>,
}

impl Match {
    /// Match within one paragraph.
    #[must_use]
    pub fn new(para: usize, start: usize, end: usize) -> Self {
        Self {
            para,
            start,
            end,
            end_para: None,
        }
    }

    /// Paragraph of the end of the match.
    #[must_use]
    pub fn last_para(&self) -> usize {
        self.end_para.unwrap_or(self.para)
    }

    /// The match spans a paragraph mark.
    #[must_use]
    pub fn is_multi_para(&self) -> bool {
        self.last_para() != self.para
    }
}

/// Upper bound on reported matches (keeps the UI responsive on huge documents).
pub const MAX_MATCHES: usize = 10_000;

/// How the query is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    Wildcard,
    Regex,
}

/// A compiled query.
#[derive(Debug, Clone)]
pub struct Searcher {
    re: regex::Regex,
    mode: Mode,
}

impl Searcher {
    /// Compile `query` with `opts`.
    pub fn new(query: &str, opts: &FindOptions) -> Result<Self> {
        let (mode, pat, case_insensitive) = if opts.regex {
            let pat = if opts.whole_word {
                format!(r"\b(?:{query})\b")
            } else {
                query.to_owned()
            };
            (Mode::Regex, pat, !opts.match_case)
        } else if opts.wildcards {
            // Wildcard search is always case-sensitive and ignores "whole word".
            (Mode::Wildcard, translate_wildcards(query)?, false)
        } else {
            let pat = translate_special(query)?;
            let pat = if opts.whole_word {
                format!(r"\b(?:{pat})\b")
            } else {
                pat
            };
            (Mode::Plain, pat, !opts.match_case)
        };
        let re = regex::RegexBuilder::new(&pat)
            .case_insensitive(case_insensitive)
            .multi_line(true)
            .size_limit(1 << 22)
            .build()
            .map_err(|e| EditError::Regex(e.to_string()))?;
        Ok(Self { re, mode })
    }

    /// The compiled regular expression (over story text, paragraphs joined by `\n`).
    #[must_use]
    pub fn regex(&self) -> &regex::Regex {
        &self.re
    }

    /// Replacement text of one match: `$1` groups in regex mode, `\1` groups in
    /// wildcard mode, `^p` `^t` `^&` … in plain and wildcard modes. `\n` in the
    /// result is a paragraph mark.
    #[must_use]
    pub fn expand(&self, caps: &regex::Captures<'_>, with: &str) -> String {
        match self.mode {
            Mode::Regex => {
                let mut dst = String::new();
                caps.expand(with, &mut dst);
                dst
            }
            Mode::Wildcard => expand_replacement(with, Some(caps), true),
            Mode::Plain => expand_replacement(with, Some(caps), false),
        }
    }
}

/// Compile the search pattern.
pub fn build(query: &str, opts: &FindOptions) -> Result<regex::Regex> {
    Searcher::new(query, opts).map(|s| s.re)
}

/// Consecutive paragraphs of one container, joined by `\n`.
#[derive(Debug, Clone, Default)]
pub struct Story {
    /// Joined index texts.
    pub text: String,
    /// `(paragraph id, start byte offset in text)` of each paragraph.
    pub paras: Vec<(usize, usize)>,
}

impl Story {
    /// Paragraph id and offset of a byte offset of the story text.
    #[must_use]
    pub fn pos(&self, off: usize) -> (usize, usize) {
        let i = self
            .paras
            .partition_point(|&(_, s)| s <= off)
            .saturating_sub(1);
        self.paras
            .get(i)
            .map_or((0, off), |&(id, s)| (id, off.saturating_sub(s)))
    }

    /// Story offset of `(paragraph, offset)`, if the paragraph is in the story.
    #[must_use]
    pub fn offset(&self, para: usize, off: usize) -> Option<usize> {
        self.paras
            .iter()
            .find(|&&(id, _)| id == para)
            .map(|&(_, s)| s + off)
    }

    /// Match of the story byte range `a..b`.
    #[must_use]
    pub fn to_match(&self, a: usize, b: usize) -> Match {
        let (p, s) = self.pos(a);
        let (q, e) = self.pos(b);
        Match {
            para: p,
            start: s,
            end: e,
            end_para: (q != p).then_some(q),
        }
    }
}

fn container(path: &ParaPath) -> (&[usize], usize) {
    match path.split_last() {
        Some((last, prefix)) => (prefix, *last),
        None => (&[], 0),
    }
}

/// Stories of the main text, in document order.
#[must_use]
pub fn stories(doc: &Document) -> Vec<Story> {
    let paths = body_paragraphs(doc);
    let mut out: Vec<Story> = Vec::new();
    let mut prev: Option<(Vec<usize>, usize)> = None;
    for (id, path) in paths.iter().enumerate() {
        let text = para_at(doc, path).map(para_index_text).unwrap_or_default();
        let (prefix, index) = container(path);
        let continues = prev
            .as_ref()
            .is_some_and(|(pp, pi)| pp.as_slice() == prefix && pi + 1 == index);
        match out.last_mut() {
            Some(st) if continues => {
                st.text.push('\n');
                st.paras.push((id, st.text.len()));
                st.text.push_str(&text);
            }
            _ => out.push(Story {
                text,
                paras: vec![(id, 0)],
            }),
        }
        prev = Some((prefix.to_vec(), index));
    }
    out
}

/// Every non-empty match with its captures, in document order; `f` returns `false`
/// to stop.
pub fn scan(
    doc: &Document,
    searcher: &Searcher,
    mut f: impl FnMut(&Story, &regex::Captures<'_>, Match) -> bool,
) {
    for st in stories(doc) {
        for caps in searcher.re.captures_iter(&st.text) {
            let Some(m) = caps.get(0) else { continue };
            if m.start() == m.end() {
                continue;
            }
            if !f(&st, &caps, st.to_match(m.start(), m.end())) {
                return;
            }
        }
    }
}

/// Formatting a match must have (Find by format); `None` fields are not checked.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindFormat {
    /// §17.3.2.1 b
    pub bold: Option<bool>,
    /// §17.3.2.16 i
    pub italic: Option<bool>,
    /// §17.3.2.40 u (any style but `none`)
    pub underline: Option<bool>,
    /// Paragraph style (§17.3.1.27 pStyle) or character style (§17.3.2.29 rStyle) id.
    pub style: Option<String>,
    /// Font family of the run (§17.3.2.26 rFonts ascii), case-insensitive.
    pub font: Option<String>,
    /// Text colour `RRGGBB` (§17.3.2.6 color).
    pub color: Option<String>,
    /// §17.3.2.15 highlight present.
    pub highlight: Option<bool>,
}

impl FindFormat {
    /// No criterion set.
    pub fn is_empty(&self) -> bool {
        *self == FindFormat::default()
    }

    fn matches(&self, doc: &Document, p: &Paragraph, direct: &RunProps) -> bool {
        let rp = crate::effective_run(doc, p, direct);
        let para_style = p.props.style.clone().or_else(|| {
            doc.styles
                .default_of(StyleKind::Paragraph)
                .map(|s| s.id.clone())
        });
        let font = rp
            .theme_ascii
            .as_deref()
            .and_then(|t| doc.theme_fonts.resolve(t))
            .or(rp.font.as_deref());
        self.bold.is_none_or(|b| rp.bold.unwrap_or(false) == b)
            && self.italic.is_none_or(|b| rp.italic.unwrap_or(false) == b)
            && self.underline.is_none_or(|b| {
                !matches!(rp.underline, None | Some(bo_model::Underline::None)) == b
            })
            && self.highlight.is_none_or(|b| rp.highlight.is_some() == b)
            && self.style.as_ref().is_none_or(|s| {
                para_style.as_deref() == Some(s.as_str()) || direct.style.as_deref() == Some(s)
            })
            && self
                .font
                .as_ref()
                .is_none_or(|f| font.is_some_and(|x| x.eq_ignore_ascii_case(f)))
            && self.color.as_ref().is_none_or(|c| {
                rp.color.unwrap_or(bo_model::Color::BLACK).to_hex() == c.to_ascii_uppercase()
            })
    }
}

/// Byte ranges of `p`'s index text whose runs have the format `fmt` (merged).
fn formatted_ranges(doc: &Document, p: &Paragraph, fmt: &FindFormat) -> Vec<(usize, usize)> {
    let (_, spans) = para_index(p);
    let ok: Vec<bool> = p
        .runs
        .iter()
        .map(|r| fmt.matches(doc, p, &r.props))
        .collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for s in spans.iter().filter(|s| s.end > s.start && ok[s.run]) {
        match out.last_mut() {
            Some(l) if l.1 == s.start => l.1 = s.end,
            _ => out.push((s.start, s.end)),
        }
    }
    out
}

/// Formatted ranges per paragraph id (computed on demand).
struct FormatFilter<'a> {
    doc: &'a Document,
    fmt: &'a FindFormat,
    paths: Vec<ParaPath>,
    cache: std::collections::HashMap<usize, Vec<(usize, usize)>>,
}

impl<'a> FormatFilter<'a> {
    fn new(doc: &'a Document, fmt: &'a FindFormat) -> Self {
        Self {
            doc,
            fmt,
            paths: body_paragraphs(doc),
            cache: std::collections::HashMap::new(),
        }
    }

    /// Single-paragraph match whose text all has the format.
    fn accepts(&mut self, m: &Match) -> bool {
        if m.is_multi_para() {
            return false;
        }
        let (doc, fmt) = (self.doc, self.fmt);
        let paths = &self.paths;
        let ranges = self.cache.entry(m.para).or_insert_with(|| {
            paths
                .get(m.para)
                .and_then(|path| para_at(doc, path))
                .map(|p| formatted_ranges(doc, p, fmt))
                .unwrap_or_default()
        });
        ranges.iter().any(|&(a, b)| a <= m.start && m.end <= b)
    }
}

/// Matches of `query` whose text all has the format `fmt`; with an empty query, every
/// stretch of text with that format (Find by format).
pub fn find_formatted(
    doc: &Document,
    query: &str,
    opts: &FindOptions,
    fmt: &FindFormat,
) -> Result<Vec<Match>> {
    if fmt.is_empty() {
        return find(doc, query, opts);
    }
    if query.is_empty() {
        let mut out = Vec::new();
        for (id, path) in body_paragraphs(doc).iter().enumerate() {
            let Some(p) = para_at(doc, path) else {
                continue;
            };
            out.extend(
                formatted_ranges(doc, p, fmt)
                    .into_iter()
                    .map(|(start, end)| Match::new(id, start, end)),
            );
            if out.len() >= MAX_MATCHES {
                out.truncate(MAX_MATCHES);
                break;
            }
        }
        return Ok(out);
    }
    Ok(replacements(doc, query, opts, None, Some(fmt))?
        .into_iter()
        .map(|(m, _)| m)
        .collect())
}

/// All matches of `query` in body paragraphs, in document order.
pub fn find(doc: &Document, query: &str, opts: &FindOptions) -> Result<Vec<Match>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    Ok(replacements(doc, query, opts, None, None)?
        .into_iter()
        .map(|(m, _)| m)
        .collect())
}

/// Matches of `query` (with the format `fmt`, if any) and, with `with`, the
/// replacement text of each one.
pub fn replacements(
    doc: &Document,
    query: &str,
    opts: &FindOptions,
    with: Option<&str>,
    fmt: Option<&FindFormat>,
) -> Result<Vec<(Match, String)>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let searcher = Searcher::new(query, opts)?;
    let mut filter = fmt
        .filter(|f| !f.is_empty())
        .map(|f| FormatFilter::new(doc, f));
    let mut out = Vec::new();
    scan(doc, &searcher, |_, caps, m| {
        if filter.as_mut().is_none_or(|f| f.accepts(&m)) {
            let r = with.map(|w| searcher.expand(caps, w)).unwrap_or_default();
            out.push((m, r));
        }
        out.len() < MAX_MATCHES
    });
    Ok(out)
}

/// Replacement text for the match `m` of `query` (found again in its story so that
/// groups are expanded in context); without a query only special characters and
/// `^&` (the matched text) are expanded.
pub fn replacement_for(
    doc: &Document,
    m: &Match,
    query: Option<(&str, &FindOptions)>,
    with: &str,
) -> Result<String> {
    let st = stories(doc)
        .into_iter()
        .find(|st| st.offset(m.para, 0).is_some())
        .ok_or(EditError::Conflict)?;
    let a = st.offset(m.para, m.start).ok_or(EditError::Conflict)?;
    let b = st.offset(m.last_para(), m.end).ok_or(EditError::Conflict)?;
    if a > b || b > st.text.len() || !st.text.is_char_boundary(a) || !st.text.is_char_boundary(b) {
        return Err(EditError::Conflict);
    }
    match query.filter(|(q, _)| !q.is_empty()) {
        Some((q, opts)) => {
            let searcher = Searcher::new(q, opts)?;
            let found = searcher
                .re
                .captures_iter(&st.text)
                .find(|c| c.get(0).is_some_and(|x| x.start() == a && x.end() == b))
                .map(|c| searcher.expand(&c, with));
            found.ok_or(EditError::Conflict)
        }
        None => Ok(pattern::expand_with_text(with, &st.text[a..b])),
    }
}

#[cfg(test)]
mod tests;

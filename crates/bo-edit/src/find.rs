//! Find in the main text: case sensitivity, whole words, regular expressions,
//! special characters (`^p`, `^t`, `^l`, `^#` …), wildcards and formatting (bold,
//! italic, underline, style, font, colour, highlight).
//!
//! Search runs over *stories*: runs of consecutive paragraphs in the same container
//! (the body of a section, one table cell) whose index texts are joined by a
//! paragraph mark (`\n`), so `^p` matches across paragraph boundaries. Matches never
//! cross a container (a table cell end is not a paragraph mark).

mod forms;
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
    /// Sounds like (English): words with the query's Soundex code ("Smith" finds "Smyth").
    pub sounds_like: bool,
    /// Find all word forms (English): the query's inflections ("run" finds "runs", "ran",
    /// "running").
    pub word_forms: bool,
    /// Match prefix: the match starts a word.
    pub match_prefix: bool,
    /// Match suffix: the match ends a word.
    pub match_suffix: bool,
    /// Ignore punctuation characters between the query's letters ("dont" finds "don't").
    pub ignore_punctuation: bool,
    /// Ignore white-space characters between the query's letters ("upto" finds "up to").
    pub ignore_space: bool,
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
    /// Sounds like: the Soundex code a matched word must have.
    soundex: Option<String>,
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
        } else if opts.sounds_like {
            // Any word; the Soundex code decides (`Searcher::accepts`).
            (Mode::Plain, r"\b\p{L}+\b".to_owned(), true)
        } else {
            let pat = if opts.word_forms {
                let forms = forms::word_forms(query.trim());
                let alts: Vec<String> = forms.iter().map(|f| regex::escape(f)).collect();
                format!(r"\b(?:{})\b", alts.join("|"))
            } else if opts.ignore_punctuation || opts.ignore_space {
                loose_pattern(query, opts.ignore_punctuation, opts.ignore_space)
            } else {
                translate_special(query)?
            };
            let pat = if opts.whole_word && !opts.word_forms {
                format!(r"\b(?:{pat})\b")
            } else {
                pat
            };
            let pat = match (opts.match_prefix, opts.match_suffix) {
                (true, true) => format!(r"\b(?:{pat})\b"),
                (true, false) => format!(r"\b(?:{pat})"),
                (false, true) => format!(r"(?:{pat})\b"),
                (false, false) => pat,
            };
            (Mode::Plain, pat, !opts.match_case)
        };
        let re = regex::RegexBuilder::new(&pat)
            .case_insensitive(case_insensitive)
            .multi_line(true)
            .size_limit(1 << 22)
            .build()
            .map_err(|e| EditError::Regex(e.to_string()))?;
        let soundex = (opts.sounds_like && !opts.regex && !opts.wildcards)
            .then(|| forms::soundex(query.trim()))
            .flatten();
        Ok(Self { re, mode, soundex })
    }

    /// Whether the matched text `m` counts (Sounds like checks its Soundex code).
    #[must_use]
    pub fn accepts(&self, m: &str) -> bool {
        self.soundex
            .as_deref()
            .is_none_or(|code| forms::soundex(m).as_deref() == Some(code))
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

/// The query with its punctuation (and / or white space) left out and any run of them
/// allowed between its other characters.
fn loose_pattern(query: &str, punct: bool, space: bool) -> String {
    let skip = |c: char| (punct && c.is_ascii_punctuation()) || (space && c.is_whitespace());
    let gap = match (punct, space) {
        (true, true) => r"[\p{P}\s]*",
        (true, false) => r"\p{P}*",
        _ => r"\s*",
    };
    let chars: Vec<String> = query
        .chars()
        .filter(|c| !skip(*c))
        .map(|c| regex::escape(&c.to_string()))
        .collect();
    chars.join(gap)
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
            if m.start() == m.end() || !searcher.accepts(m.as_str()) {
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
    /// Font size in half points (§17.3.2.38 sz).
    pub size: Option<u32>,
    /// §17.3.2.37 strike.
    pub strike: Option<bool>,
    /// §17.3.2.33 smallCaps.
    pub small_caps: Option<bool>,
    /// §17.3.2.5 caps.
    pub caps: Option<bool>,
    /// §17.3.2.41 vanish (hidden).
    pub hidden: Option<bool>,
    /// Paragraph alignment (§17.3.1.13 jc): `left`, `center`, `right` or `both`.
    pub align: Option<String>,
    /// Outline level 1–9, 10 = body text (§17.3.1.20 outlineLvl + 1).
    pub outline_level: Option<u8>,
    /// Left indent, twips (§17.3.1.12 ind).
    pub indent_left: Option<i32>,
    /// Right indent, twips.
    pub indent_right: Option<i32>,
    /// First-line indent (negative: hanging), twips.
    pub first_line: Option<i32>,
    /// Space before, twips (§17.3.1.33 spacing).
    pub space_before: Option<i32>,
    /// Space after, twips.
    pub space_after: Option<i32>,
    /// Line spacing: `single`, `1.5`, `double`, `multiple:<240ths>`, `atLeast:<twips>`,
    /// `exactly:<twips>` (§17.3.1.33 spacing line / lineRule).
    pub line_spacing: Option<String>,
    /// A tab stop at this position, twips (§17.3.1.38 tabs).
    pub tab: Option<i32>,
    /// Proofing language (§17.3.2.20 lang `@w:val`), BCP 47; `"none"`: no language set.
    pub lang: Option<String>,
    /// §17.3.2.21 noProof.
    pub no_proof: Option<bool>,
    /// The paragraph is a text frame (§17.3.1.11 framePr).
    pub frame: Option<bool>,
    /// §17.3.1.44 widowControl.
    pub widow_control: Option<bool>,
    /// §17.3.1.15 keepNext.
    pub keep_next: Option<bool>,
    /// §17.3.1.14 keepLines.
    pub keep_lines: Option<bool>,
    /// §17.3.1.23 pageBreakBefore.
    pub page_break_before: Option<bool>,
    /// §17.3.1.34 suppressLineNumbers.
    pub suppress_line_numbers: Option<bool>,
    /// §17.3.1.34 suppressAutoHyphens.
    pub suppress_auto_hyphens: Option<bool>,
    /// §17.3.1.18 mirrorIndents (Paragraph dialog, round 13; set only, not searched).
    pub mirror_indents: Option<bool>,
    /// §17.3.1.9 contextualSpacing (don't add space between paragraphs of the same style).
    pub contextual_spacing: Option<bool>,
    /// §17.3.2.9 dstrike.
    pub double_strike: Option<bool>,
    /// §17.3.2.42 vertAlign superscript.
    pub superscript: Option<bool>,
    /// §17.3.2.42 vertAlign subscript.
    pub subscript: Option<bool>,
}

/// Line spacing word of [`FindFormat::line_spacing`].
fn line_spacing_word(ls: bo_model::LineSpacing) -> String {
    match ls {
        bo_model::LineSpacing::Auto(240) => "single".into(),
        bo_model::LineSpacing::Auto(360) => "1.5".into(),
        bo_model::LineSpacing::Auto(480) => "double".into(),
        bo_model::LineSpacing::Auto(n) => format!("multiple:{n}"),
        bo_model::LineSpacing::AtLeast(t) => format!("atLeast:{}", t.0),
        bo_model::LineSpacing::Exact(t) => format!("exactly:{}", t.0),
    }
}

fn parse_line_spacing(s: &str) -> Option<bo_model::LineSpacing> {
    use bo_model::{LineSpacing, Twips};
    let num = |v: &str| v.trim().parse::<i64>().ok();
    Some(match s {
        "single" => LineSpacing::Auto(240),
        "1.5" => LineSpacing::Auto(360),
        "double" => LineSpacing::Auto(480),
        _ => match s.split_once(':')? {
            ("multiple", v) => LineSpacing::Auto(u32::try_from(num(v)?).ok()?),
            ("atLeast", v) => LineSpacing::AtLeast(Twips(i32::try_from(num(v)?).ok()?)),
            ("exactly", v) => LineSpacing::Exact(Twips(i32::try_from(num(v)?).ok()?)),
            _ => return None,
        },
    })
}

fn align_word(a: bo_model::Alignment) -> &'static str {
    match a {
        bo_model::Alignment::Left => "left",
        bo_model::Alignment::Center => "center",
        bo_model::Alignment::Right => "right",
        bo_model::Alignment::Justify => "both",
    }
}

fn style_kind(doc: &Document, id: &str) -> Option<StyleKind> {
    doc.styles
        .styles
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.kind)
}

impl FindFormat {
    /// No criterion set.
    pub fn is_empty(&self) -> bool {
        *self == FindFormat::default()
    }

    /// Whether a paragraph property is part of the search.
    fn has_para(&self) -> bool {
        self.align.is_some()
            || self.outline_level.is_some()
            || self.indent_left.is_some()
            || self.indent_right.is_some()
            || self.first_line.is_some()
            || self.space_before.is_some()
            || self.space_after.is_some()
            || self.line_spacing.is_some()
            || self.tab.is_some()
            || self.frame.is_some()
            || self.widow_control.is_some()
            || self.keep_next.is_some()
            || self.keep_lines.is_some()
            || self.page_break_before.is_some()
            || self.suppress_line_numbers.is_some()
            || self.suppress_auto_hyphens.is_some()
            || self.mirror_indents.is_some()
            || self.contextual_spacing.is_some()
    }

    /// The paragraph's own properties match (alignment, indents, spacing, tabs, frame).
    fn para_matches(&self, doc: &Document, p: &Paragraph) -> bool {
        if !self.has_para() {
            return true;
        }
        let pp = crate::effective_para(doc, &p.props);
        let tw = |v: Option<bo_model::Twips>| v.map_or(0, |t| t.0);
        self.align
            .as_deref()
            .is_none_or(|a| align_word(pp.align.unwrap_or_default()) == a)
            && self
                .outline_level
                .is_none_or(|l| pp.outline_level.map_or(10, |o| o.saturating_add(1)) == l)
            && self.indent_left.is_none_or(|v| tw(pp.indent_left) == v)
            && self.indent_right.is_none_or(|v| tw(pp.indent_right) == v)
            && self
                .first_line
                .is_none_or(|v| tw(pp.indent_first_line) == v)
            && self.space_before.is_none_or(|v| tw(pp.space_before) == v)
            && self.space_after.is_none_or(|v| tw(pp.space_after) == v)
            && self
                .line_spacing
                .as_deref()
                .is_none_or(|w| line_spacing_word(pp.line_spacing.unwrap_or_default()) == w)
            && self.tab.is_none_or(|pos| {
                pp.tabs.as_ref().is_some_and(|ts| {
                    ts.iter()
                        .any(|t| t.pos.0 == pos && t.align != bo_model::TabAlign::Clear)
                })
            })
            && self.frame.is_none_or(|f| pp.frame.is_some() == f)
            && [
                (self.widow_control, pp.widow_control, true),
                (self.keep_next, pp.keep_next, false),
                (self.keep_lines, pp.keep_lines, false),
                (self.page_break_before, pp.page_break_before, false),
                (self.suppress_line_numbers, pp.suppress_line_numbers, false),
                (self.suppress_auto_hyphens, pp.suppress_auto_hyphens, false),
                (self.contextual_spacing, pp.contextual_spacing, false),
            ]
            .iter()
            .all(|&(want, have, default)| want.is_none_or(|w| have.unwrap_or(default) == w))
    }

    /// Run properties `rp` with this format applied (Replace with formatting).
    /// `kind`: the kind of [`Self::style`] in the document ([`Self::style_kind`]).
    pub fn apply_run(&self, kind: Option<StyleKind>, rp: &mut RunProps) {
        let flag = |v: Option<bool>, slot: &mut Option<bool>| {
            if let Some(b) = v {
                *slot = Some(b);
            }
        };
        flag(self.bold, &mut rp.bold);
        flag(self.italic, &mut rp.italic);
        flag(self.strike, &mut rp.strike);
        flag(self.small_caps, &mut rp.small_caps);
        flag(self.caps, &mut rp.caps);
        flag(self.hidden, &mut rp.hidden);
        flag(self.no_proof, &mut rp.no_proof);
        flag(self.double_strike, &mut rp.double_strike);
        match (self.superscript, self.subscript) {
            (Some(true), _) => rp.vert_align = Some(bo_model::VertAlign::Superscript),
            (_, Some(true)) => rp.vert_align = Some(bo_model::VertAlign::Subscript),
            (Some(false), _) | (_, Some(false)) => {
                rp.vert_align = Some(bo_model::VertAlign::Baseline);
            }
            _ => {}
        }
        if let Some(b) = self.underline {
            rp.underline = Some(if b {
                bo_model::Underline::Single
            } else {
                bo_model::Underline::None
            });
        }
        match self.font.as_deref() {
            Some(t @ ("+Body" | "+Headings")) => {
                let theme = if t == "+Body" {
                    "minorHAnsi"
                } else {
                    "majorHAnsi"
                };
                rp.theme_ascii = Some(theme.into());
                rp.theme_h_ansi = Some(theme.into());
            }
            Some(f) => {
                rp.font = Some(f.to_owned());
                rp.font_h_ansi = Some(f.to_owned());
                rp.theme_ascii = None;
                rp.theme_h_ansi = None;
            }
            None => {}
        }
        if let Some(sz) = self.size {
            rp.size = Some(bo_model::HalfPoints(sz));
        }
        if let Some(c) = self.color.as_deref().and_then(bo_model::Color::from_hex) {
            rp.color = Some(c);
            rp.theme_color = None;
        }
        match self.highlight {
            // The reference highlights in the pen colour of the Home tab (yellow by default).
            Some(true) => rp.highlight = bo_model::Color::from_hex("FFFF00"),
            Some(false) => rp.highlight = None,
            None => {}
        }
        if let Some(l) = &self.lang {
            let mut lang = rp.lang.clone().unwrap_or_default();
            lang.val = (l != "none").then(|| l.clone());
            rp.lang = Some(lang);
        }
        if let Some(s) = &self.style {
            if kind == Some(StyleKind::Character) {
                rp.style = Some(s.clone());
            }
        }
    }

    /// Paragraph properties `pp` with this format applied (paragraph style, alignment,
    /// indents, spacing, outline level, tab stop).
    pub fn apply_para(&self, kind: Option<StyleKind>, pp: &mut bo_model::ParaProps) {
        if let Some(s) = &self.style {
            if kind == Some(StyleKind::Paragraph) {
                pp.style = Some(s.clone());
            }
        }
        if let Some(a) = self.align.as_deref() {
            pp.align = Some(match a {
                "center" => bo_model::Alignment::Center,
                "right" => bo_model::Alignment::Right,
                "both" => bo_model::Alignment::Justify,
                _ => bo_model::Alignment::Left,
            });
        }
        if let Some(l) = self.outline_level {
            pp.outline_level = (1..=9).contains(&l).then(|| l - 1);
        }
        let set = |v: Option<i32>, slot: &mut Option<bo_model::Twips>| {
            if let Some(v) = v {
                *slot = Some(bo_model::Twips(v));
            }
        };
        set(self.indent_left, &mut pp.indent_left);
        set(self.indent_right, &mut pp.indent_right);
        set(self.first_line, &mut pp.indent_first_line);
        set(self.space_before, &mut pp.space_before);
        set(self.space_after, &mut pp.space_after);
        if let Some(ls) = self.line_spacing.as_deref().and_then(parse_line_spacing) {
            pp.line_spacing = Some(ls);
        }
        for (v, slot) in [
            (self.widow_control, &mut pp.widow_control),
            (self.keep_next, &mut pp.keep_next),
            (self.keep_lines, &mut pp.keep_lines),
            (self.page_break_before, &mut pp.page_break_before),
            (self.suppress_line_numbers, &mut pp.suppress_line_numbers),
            (self.suppress_auto_hyphens, &mut pp.suppress_auto_hyphens),
            (self.mirror_indents, &mut pp.mirror_indents),
            (self.contextual_spacing, &mut pp.contextual_spacing),
        ] {
            if let Some(b) = v {
                *slot = Some(b);
            }
        }
        if let Some(pos) = self.tab {
            let tabs = pp.tabs.get_or_insert_with(Vec::new);
            if !tabs.iter().any(|t| t.pos.0 == pos) {
                tabs.push(bo_model::TabStop {
                    pos: bo_model::Twips(pos),
                    align: bo_model::TabAlign::Left,
                    leader: bo_model::TabLeader::None,
                });
                tabs.sort_by_key(|t| t.pos.0);
            }
        }
    }

    /// Whether applying the format changes paragraph properties.
    pub fn touches_para(&self, kind: Option<StyleKind>) -> bool {
        self.has_para() || (self.style.is_some() && kind == Some(StyleKind::Paragraph))
    }

    /// The kind of the style searched for / applied, in `doc`.
    pub fn style_kind(&self, doc: &Document) -> Option<StyleKind> {
        self.style.as_deref().and_then(|s| style_kind(doc, s))
    }

    fn matches(&self, doc: &Document, p: &Paragraph, direct: &RunProps) -> bool {
        let rp = crate::effective_run(doc, p, direct);
        let lang_ok = self.lang.as_deref().is_none_or(|want| {
            match (want, rp.lang.as_ref().and_then(|l| l.val.as_deref())) {
                ("none", None) => true,
                (w, Some(h)) => w.eq_ignore_ascii_case(h),
                _ => false,
            }
        });
        let extra = lang_ok
            && self.size.is_none_or(|sz| rp.size.map_or(22, |h| h.0) == sz)
            && self.strike.is_none_or(|b| rp.strike.unwrap_or(false) == b)
            && self
                .small_caps
                .is_none_or(|b| rp.small_caps.unwrap_or(false) == b)
            && self.caps.is_none_or(|b| rp.caps.unwrap_or(false) == b)
            && self.hidden.is_none_or(|b| rp.hidden.unwrap_or(false) == b)
            && self
                .double_strike
                .is_none_or(|b| rp.double_strike.unwrap_or(false) == b)
            && self.superscript.is_none_or(|b| {
                matches!(rp.vert_align, Some(bo_model::VertAlign::Superscript)) == b
            })
            && self
                .subscript
                .is_none_or(|b| matches!(rp.vert_align, Some(bo_model::VertAlign::Subscript)) == b)
            && self
                .no_proof
                .is_none_or(|b| rp.no_proof.unwrap_or(false) == b);
        if !extra {
            return false;
        }
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
            && self.font.as_deref().is_none_or(|f| match f {
                // The theme's body / heading font (§17.3.2.26 asciiTheme minor… / major…).
                "+Body" => rp
                    .theme_ascii
                    .as_deref()
                    .is_some_and(|t| t.starts_with("minor")),
                "+Headings" => rp
                    .theme_ascii
                    .as_deref()
                    .is_some_and(|t| t.starts_with("major")),
                _ => font.is_some_and(|x| x.eq_ignore_ascii_case(f)),
            })
            && self.color.as_ref().is_none_or(|c| {
                rp.color.unwrap_or(bo_model::Color::BLACK).to_hex() == c.to_ascii_uppercase()
            })
    }
}

/// Byte ranges of `p`'s index text whose runs have the format `fmt` (merged).
fn formatted_ranges(doc: &Document, p: &Paragraph, fmt: &FindFormat) -> Vec<(usize, usize)> {
    if !fmt.para_matches(doc, p) {
        return Vec::new();
    }
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

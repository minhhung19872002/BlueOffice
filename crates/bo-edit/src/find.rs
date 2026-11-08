//! Find in the main text: case sensitivity, whole words, regular expressions and
//! formatting (bold, italic, underline, style, font, colour, highlight).

use crate::{EditError, Result};
use bo_model::index::{body_paragraphs, para_at, para_index, para_index_text};
use bo_model::{Document, Paragraph, RunProps, StyleKind};
use serde::{Deserialize, Serialize};

/// Search options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindOptions {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// A match: body paragraph id and byte range of its index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Match {
    #[serde(rename = "p")]
    pub para: usize,
    pub start: usize,
    pub end: usize,
}

/// Upper bound on reported matches (keeps the UI responsive on huge documents).
pub const MAX_MATCHES: usize = 10_000;

/// Compile the search pattern.
pub fn build(query: &str, opts: &FindOptions) -> Result<regex::Regex> {
    let mut pat = if opts.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    if opts.whole_word {
        pat = format!(r"\b(?:{pat})\b");
    }
    regex::RegexBuilder::new(&pat)
        .case_insensitive(!opts.match_case)
        .size_limit(1 << 22)
        .build()
        .map_err(|e| EditError::Regex(e.to_string()))
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
    let re = if query.is_empty() {
        None
    } else {
        Some(build(query, opts)?)
    };
    let mut out = Vec::new();
    for (id, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let ranges = formatted_ranges(doc, p, fmt);
        if ranges.is_empty() {
            continue;
        }
        match &re {
            None => out.extend(ranges.into_iter().map(|(start, end)| Match {
                para: id,
                start,
                end,
            })),
            Some(re) => {
                let text = para_index_text(p);
                for m in re.find_iter(&text) {
                    let inside = ranges.iter().any(|&(a, b)| a <= m.start() && m.end() <= b);
                    if m.start() < m.end() && inside {
                        out.push(Match {
                            para: id,
                            start: m.start(),
                            end: m.end(),
                        });
                    }
                }
            }
        }
        if out.len() >= MAX_MATCHES {
            out.truncate(MAX_MATCHES);
            break;
        }
    }
    Ok(out)
}

/// All matches of `query` in body paragraphs, in document order.
pub fn find(doc: &Document, query: &str, opts: &FindOptions) -> Result<Vec<Match>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let re = build(query, opts)?;
    let mut out = Vec::new();
    for (id, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let text = para_index_text(p);
        for m in re.find_iter(&text) {
            if m.start() < m.end() {
                out.push(Match {
                    para: id,
                    start: m.start(),
                    end: m.end(),
                });
                if out.len() >= MAX_MATCHES {
                    return Ok(out);
                }
            }
        }
    }
    Ok(out)
}

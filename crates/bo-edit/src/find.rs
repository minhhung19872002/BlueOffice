//! Find in the main text: case sensitivity, whole words and regular expressions.

use crate::{EditError, Result};
use bo_model::index::{body_paragraphs, para_at, para_index_text};
use bo_model::Document;
use serde::{Deserialize, Serialize};

/// Search options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindOptions {
    /// Compare case-sensitively.
    pub match_case: bool,
    /// Match whole words only.
    pub whole_word: bool,
    /// Treat the query as a regular expression.
    pub regex: bool,
}

/// A match: body paragraph id and byte range of its index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Match {
    /// Body paragraph id.
    #[serde(rename = "p")]
    pub para: usize,
    /// Start byte offset in the index text.
    pub start: usize,
    /// End byte offset (exclusive) in the index text.
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

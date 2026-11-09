//! Spelling and grammar of plain text units outside BlueDoc (`docs/parity/BACKLOG.md`
//! CORE-SPELL-006): the text of a BlueSheet cell, the paragraphs of a BlueSlide shape, table
//! cell, speaker notes or SmartArt node. Checking goes through the same [`SpellState`] as BlueDoc
//! (dictionaries, user words, ignored words and grammar rules are shared); this module only turns
//! its findings into issues with byte and UTF-16 offsets and applies corrections to a string.

use crate::spell::{Span, SpellState};
use bo_spell::vi::ToneStyle;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ops::Range;

/// Most suggestions computed for one text (suggesting is the slow part of checking).
const MAX_SUGGESTED: usize = 40;

/// One spelling or grammar issue of a text unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextIssue {
    /// Start byte offset in the text.
    pub start: usize,
    /// End byte offset in the text.
    pub end: usize,
    /// Start offset in UTF-16 code units (for text inputs of the UI).
    pub from: usize,
    /// End offset in UTF-16 code units.
    pub to: usize,
    /// The flagged text.
    pub word: String,
    /// A grammar or style issue (rather than a misspelt word).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub grammar: bool,
    /// Grammar rule id (`bo_spell::grammar::Rule::id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
    /// Corrections, best first.
    pub suggestions: Vec<String>,
    /// Language the word was checked in (`None`: any language).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Sentence text before the flagged text.
    pub before: String,
    /// Sentence text after the flagged text.
    pub after: String,
}

/// Proofing options shared by BlueSheet and BlueSlide (`spellOptions` / `slideSpellOptions`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsIn {
    /// Check spelling at all.
    pub enabled: Option<bool>,
    /// Report grammar and style issues.
    pub grammar: Option<bool>,
    /// Words in capitals are not checked.
    pub ignore_caps: Option<bool>,
    /// Words with digits are not checked.
    pub ignore_digits: Option<bool>,
}

/// Current proofing options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsOut {
    /// Spelling is checked.
    pub enabled: bool,
    /// Grammar and style issues are reported.
    pub grammar: bool,
    /// Words in capitals are not checked.
    pub ignore_caps: bool,
    /// Words with digits are not checked.
    pub ignore_digits: bool,
    /// Languages with a dictionary (primary subtags such as `vi`, `en`).
    pub languages: Vec<String>,
}

impl SpellState {
    /// Apply options; returns the current ones.
    pub(crate) fn set_options(&mut self, o: &OptionsIn) -> OptionsOut {
        if let Some(v) = o.enabled {
            self.enabled = v;
        }
        if let Some(v) = o.grammar {
            self.grammar = v;
        }
        if let Some(v) = o.ignore_caps {
            self.speller.ignore_all_caps = v;
        }
        if let Some(v) = o.ignore_digits {
            self.speller.ignore_digits = v;
        }
        self.invalidate();
        self.options()
    }

    /// Current options.
    pub(crate) fn options(&self) -> OptionsOut {
        OptionsOut {
            enabled: self.enabled,
            grammar: self.grammar,
            ignore_caps: self.speller.ignore_all_caps,
            ignore_digits: self.speller.ignore_digits,
            languages: languages(self),
        }
    }

    /// Ignore a word everywhere (`ignore`) or add it to the user dictionary.
    pub(crate) fn learn(&mut self, word: &str, ignore: bool) {
        if ignore {
            self.speller.ignore(word);
        } else {
            self.speller.add_user_word(word);
        }
        self.invalidate();
    }
}

/// Languages with a dictionary (primary subtags).
pub(crate) fn languages(spell: &SpellState) -> Vec<String> {
    ["vi", "en"]
        .iter()
        .filter(|l| spell.speller.has_language(Some(l)))
        .map(|l| (*l).to_owned())
        .collect()
}

/// One language span over the whole text.
pub(crate) fn whole(text: &str, lang: Option<&str>) -> Vec<Span> {
    vec![Span {
        range: 0..text.len(),
        lang: lang
            .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("x-none"))
            .map(str::to_owned),
        skip: false,
    }]
}

/// UTF-16 offset of byte offset `at` of `text`.
pub(crate) fn utf16_at(text: &str, at: usize) -> usize {
    text.get(..at).map_or(0, |s| s.encode_utf16().count())
}

/// Text shown around an issue: line breaks and tabs as spaces.
fn shown(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '\u{b}' | '\t' | '\n' | '\u{2028}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn text_issue(
    text: &str,
    range: Range<usize>,
    grammar: Option<&'static str>,
    suggestions: Vec<String>,
    lang: Option<String>,
) -> TextIssue {
    let s = bo_spell::lang::sentence_around(text, &range);
    TextIssue {
        from: utf16_at(text, range.start),
        to: utf16_at(text, range.end),
        word: text[range.clone()].to_owned(),
        grammar: grammar.is_some(),
        rule: grammar,
        suggestions,
        lang,
        before: shown(&text[s.start.min(range.start)..range.start]),
        after: shown(&text[range.end..s.end.max(range.end)]),
        start: range.start,
        end: range.end,
    }
}

/// Issues of one text in its language spans, sorted by position; `style` is the tone placement
/// most used around it (grammar issues for the other placement), `suggest` asks for corrections
/// of misspelt words. Also returns the languages found without a checker.
pub(crate) fn check(
    spell: &mut SpellState,
    text: &str,
    spans: &[Span],
    style: Option<ToneStyle>,
    suggest: bool,
) -> (Vec<TextIssue>, BTreeSet<String>) {
    if !spell.enabled || text.trim().is_empty() {
        return (Vec::new(), BTreeSet::new());
    }
    let a = spell.analysis(text, spans);
    let unchecked = a.unchecked.clone();
    let misspelt = a.misspelt.clone();
    let mut out = Vec::new();
    for (n, m) in misspelt.into_iter().enumerate() {
        let suggestions = if suggest && n < MAX_SUGGESTED {
            spell.suggest(&text[m.range.clone()], m.lang.as_deref())
        } else {
            Vec::new()
        };
        out.push(text_issue(text, m.range, None, suggestions, m.lang));
    }
    for i in spell.issues(text, spans, style) {
        out.push(text_issue(
            text,
            i.range,
            Some(i.rule.id()),
            i.replacement.into_iter().collect(),
            None,
        ));
    }
    out.sort_by_key(|i| (i.start, i.end));
    (out, unchecked)
}

/// The tone placement used most in `texts` (`None` when grammar is off or no word shows one).
pub(crate) fn style_of(spell: &mut SpellState, texts: &[(String, Vec<Span>)]) -> Option<ToneStyle> {
    if spell.grammar {
        spell.document_style(texts)
    } else {
        None
    }
}

/// A correction of a text: replace bytes `start..end` with `text`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Replace {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// Replacement text.
    pub text: String,
    /// The text expected at `start..end` (the correction is refused when it changed).
    #[serde(default)]
    pub word: Option<String>,
}

/// Checks that corrections fit `text` (char boundaries, expected words) and do not overlap;
/// returns them sorted from the last to the first.
pub(crate) fn plan<'a>(text: &str, edits: &[&'a Replace]) -> Option<Vec<&'a Replace>> {
    let mut v: Vec<&Replace> = edits.to_vec();
    v.sort_by_key(|e| std::cmp::Reverse((e.start, e.end)));
    let mut limit = text.len();
    for e in &v {
        if e.start > e.end
            || e.end > limit
            || !text.is_char_boundary(e.start)
            || !text.is_char_boundary(e.end)
        {
            return None;
        }
        if e.word
            .as_deref()
            .is_some_and(|w| w != &text[e.start..e.end])
        {
            return None;
        }
        limit = e.start;
    }
    Some(v)
}

/// `text` with the corrections applied (`None`: they do not fit, see [`plan`]).
pub(crate) fn apply(text: &str, edits: &[&Replace]) -> Option<String> {
    let mut s = text.to_owned();
    for e in plan(text, edits)? {
        s.replace_range(e.start..e.end, &e.text);
    }
    Some(s)
}

/// Replace bytes `start..end` of the text spread over `parts` (each part a piece of the text with
/// its own formatting): the replacement goes into the part where the range starts, the rest of
/// the range is cut from the following parts. `editable(i)` says whether part `i` may change
/// (fields and line breaks may not). Returns false when the range crosses a part that may not.
pub(crate) fn splice_parts(
    parts: &mut [&mut String],
    editable: &dyn Fn(usize) -> bool,
    start: usize,
    end: usize,
    with: &str,
) -> bool {
    // Check first: nothing changes when the range touches a fixed part.
    let mut at = 0;
    let mut first = None;
    for (i, p) in parts.iter().enumerate() {
        let (a, b) = (at, at + p.len());
        at = b;
        let touches = if start == end {
            first.is_none() && a <= start && start <= b
        } else {
            a < end && start < b
        };
        if !touches {
            continue;
        }
        let (s, e) = (start.max(a) - a, end.min(b) - a);
        if !editable(i) || !p.is_char_boundary(s) || !p.is_char_boundary(e) {
            return false;
        }
        first.get_or_insert(i);
    }
    let Some(first) = first else {
        return false;
    };
    let mut at = 0;
    for (i, p) in parts.iter_mut().enumerate() {
        let (a, b) = (at, at + p.len());
        at = b;
        if i < first {
            continue;
        }
        if i > first && a >= end {
            break;
        }
        let (s, e) = (start.max(a) - a, end.min(b) - a);
        p.replace_range(s..e, if i == first { with } else { "" });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(start: usize, end: usize, text: &str) -> Replace {
        Replace {
            start,
            end,
            text: text.into(),
            word: None,
        }
    }

    // source: hand-worked offsets of the strings below.
    #[test]
    fn apply_corrections_from_the_end() {
        let a = rep(0, 4, "Xin");
        let b = rep(5, 10, "chào");
        assert_eq!(
            apply("Xinn chaò bạn", &[&a, &b]).as_deref(),
            Some("Xin chào bạn")
        );
        // Overlapping corrections and ranges off char boundaries are refused.
        let c = rep(2, 6, "x");
        assert_eq!(apply("Xinn chaò bạn", &[&a, &c]), None);
        let d = rep(9, 10, "x"); // starts inside the two bytes of "ò" (8..10)
        assert_eq!(apply("Xinn chaò bạn", &[&d]), None);
        // The expected word must still be there.
        let mut e = rep(0, 4, "Xin");
        e.word = Some("Xinn".into());
        assert!(apply("Xinn", &[&e]).is_some());
        e.word = Some("Yinn".into());
        assert!(apply("Xinn", &[&e]).is_none());
    }

    #[test]
    fn splice_over_runs() {
        // "doc" + "umnet" + " is" formatted separately: replace "documnet" (0..8).
        let (mut a, mut b, mut c) = ("doc".to_owned(), "umnet".to_owned(), " is".to_owned());
        let ok = {
            let mut parts = [&mut a, &mut b, &mut c];
            splice_parts(&mut parts, &|_| true, 0, 8, "document")
        };
        assert!(ok);
        assert_eq!(
            (a.as_str(), b.as_str(), c.as_str()),
            ("document", "", " is")
        );
        // A fixed part in the range refuses the correction and changes nothing.
        let (mut a, mut b) = ("ab".to_owned(), "cd".to_owned());
        let ok = {
            let mut parts = [&mut a, &mut b];
            splice_parts(&mut parts, &|i| i == 0, 1, 3, "x")
        };
        assert!(!ok);
        assert_eq!((a.as_str(), b.as_str()), ("ab", "cd"));
    }

    // source: misspellings of common Vietnamese and English words.
    #[test]
    fn issues_with_utf16_offsets() {
        let mut s = SpellState::default();
        let text = "Bảng tính chaò";
        let (issues, _) = check(&mut s, text, &whole(text, Some("vi-VN")), None, true);
        assert_eq!(issues.len(), 1, "{issues:?}");
        let i = &issues[0];
        assert_eq!(i.word, "chaò");
        // "Bảng tính " is 10 UTF-16 units and 13 bytes ("ả", "í" take two bytes each).
        assert_eq!((i.from, i.to), (10, 14));
        assert_eq!((i.start, i.end), (13, 18));
        assert!(i.suggestions.contains(&"chào".to_owned()), "{i:?}");
        assert_eq!(i.before, "Bảng tính ");
    }
}

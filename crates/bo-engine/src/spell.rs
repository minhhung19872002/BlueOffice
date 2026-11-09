//! Spelling as you type (BlueDoc): misspelt words of the laid-out body text are reported as marks to draw
//! under the text (an overlay — never part of the display list, PDF or print), with suggestions for the
//! word under the pointer (`docs/parity/BACKLOG.md` CORE-SPELL-004). Grammar and style issues
//! (`bo_spell::grammar`, CORE-GRAM-001) come as marks with `grammar: true` (drawn blue); Vietnamese tone
//! placement is checked against the style most used in the document.
//!
//! Vietnamese is always checked (rule based, `bo_spell::vi`); other languages come from Hunspell
//! dictionaries loaded with [`crate::Engine::load_dictionary`] (native builds bundle `en_US`).
//!
//! Each run is checked in its resolved language (§17.3.2.20 lang `@w:val`, through docDefaults,
//! paragraph style, character style and direct formatting, CORE-SPELL-005): `vi-*` with the
//! Vietnamese rules, `en-*` with the English dictionary; a language without a checker is not
//! checked and reported. Text without any language is correct when any checker accepts it.
//! Runs with `noProof` (§17.3.2.21), hidden text (§17.3.2.41 vanish) and deleted text are skipped.
//! Two rules keep Vietnamese text typed in runs tagged with another language usable: a word with a
//! Vietnamese letter (`ă â đ ê ô ơ ư`, tone marks) is checked as Vietnamese, and in a paragraph
//! whose text reads as Vietnamese (`bo_spell::lang::detect`) a word the Vietnamese rules accept is
//! correct too.

use crate::edit::{Command, EditResult};
use crate::nav;
use crate::Engine;
use bo_layout::{Line, LineSource};
use bo_model::{Document, Paragraph};
use bo_spell::grammar::{self, Issue};
use bo_spell::vi::ToneStyle;
use bo_spell::Speller;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::ops::Range;

/// A stretch of a paragraph index text with one proofing language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    /// Byte range in the index text.
    pub range: Range<usize>,
    /// Resolved language (`None`: not set, or `x-none`).
    pub lang: Option<String>,
    /// Not checked: `noProof`, hidden or deleted text.
    pub skip: bool,
}

/// Language spans of a paragraph from its runs' resolved properties.
pub(crate) fn spans(doc: &Document, p: &Paragraph) -> Vec<Span> {
    let (_, items) = bo_model::index::para_index(p);
    let mut out: Vec<Span> = Vec::new();
    let mut resolved: HashMap<usize, (Option<String>, bool)> = HashMap::new();
    for it in items {
        if it.end == it.start {
            continue;
        }
        let (lang, skip) = resolved
            .entry(it.run)
            .or_insert_with(|| {
                let direct = &p.runs[it.run].props;
                let eff = bo_edit::effective_run(doc, p, direct);
                let lang = eff
                    .lang
                    .and_then(|l| l.val)
                    .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("x-none"));
                let skip = eff.no_proof.unwrap_or(false)
                    || eff.hidden.unwrap_or(false)
                    || direct.del.is_some();
                (lang, skip)
            })
            .clone();
        match out.last_mut() {
            Some(s) if s.range.end == it.start && s.lang == lang && s.skip == skip => {
                s.range.end = it.end;
            }
            _ => out.push(Span {
                range: it.start..it.end,
                lang,
                skip,
            }),
        }
    }
    out
}

fn span_at(spans: &[Span], at: usize) -> Option<&Span> {
    spans
        .iter()
        .find(|s| s.range.start <= at && at < s.range.end)
}

/// A misspelt word: its range and the language it was checked in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Misspelt {
    pub range: Range<usize>,
    pub lang: Option<String>,
}

/// A misspelt word on a page (points): draw a wavy line from `x` to `x + w` at `y`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpellMark {
    /// 0-based page.
    pub page: usize,
    /// Left end (points).
    pub x: f32,
    /// Baseline of the wavy line (points).
    pub y: f32,
    /// Length (points).
    pub w: f32,
    /// A grammar / style issue (blue) rather than a misspelt word (red).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub grammar: bool,
}

/// The misspelt word under the pointer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellInfo {
    /// The word as written.
    pub word: String,
    /// Body paragraph id of the word (select `start..end` in it to replace the word).
    pub para: usize,
    /// Start byte offset in the paragraph index text.
    pub start: usize,
    /// End byte offset in the paragraph index text.
    pub end: usize,
    /// Corrections, best first.
    pub suggestions: Vec<String>,
    /// Grammar rule id when this is a grammar / style issue (`word` is then the flagged text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
}

/// Cached checks of one paragraph text.
pub(crate) struct Analysis {
    pub misspelt: Vec<Misspelt>,
    pub issues: Vec<Issue>,
    pub styles: Vec<(Range<usize>, ToneStyle)>,
    /// Languages of checked text without a checker.
    pub unchecked: BTreeSet<String>,
}

/// A paragraph to check: index text and language spans.
pub(crate) type ProofText = (String, Vec<Span>);

/// Speller of the engine plus a cache of misspelt ranges per paragraph text.
pub(crate) struct SpellState {
    pub speller: Speller,
    pub enabled: bool,
    pub grammar: bool,
    /// Tag typed text with the language detected for it (off by default).
    pub auto_language: bool,
    /// Ignored grammar issues: (rule id, flagged text).
    ignored: HashSet<(String, String)>,
    cache: HashMap<String, Analysis>,
}

impl Default for SpellState {
    fn default() -> Self {
        let mut speller = Speller::new();
        speller.add_language("vi", Box::new(bo_spell::vi::Vietnamese));
        SpellState {
            speller,
            enabled: true,
            grammar: true,
            auto_language: false,
            ignored: HashSet::new(),
            cache: HashMap::new(),
        }
    }
}

/// Cache key of a paragraph: its text and language spans.
fn key(text: &str, spans: &[Span]) -> String {
    let mut k = String::with_capacity(text.len() + 16);
    k.push_str(text);
    for s in spans {
        let _ = write!(
            k,
            "\u{1}{}-{}:{}:{}",
            s.range.start,
            s.range.end,
            s.lang.as_deref().unwrap_or(""),
            u8::from(s.skip)
        );
    }
    k
}

impl SpellState {
    /// Checks one word of a paragraph in its language: `None` when no checker serves it.
    fn check_word(&self, word: &str, lang: Option<&str>, para_vi: bool) -> (bool, Option<String>) {
        let vi_word = bo_spell::lang::has_vietnamese_letter(word);
        let non_vi = lang.is_some_and(|l| bo_spell::primary_language(l) != "vi");
        let vi_ok = || self.speller.check_in(word, Some("vi")) == Some(true);
        // A word written with Vietnamese letters is Vietnamese whatever the run says.
        let lang = if non_vi && vi_word && self.speller.has_language(Some("vi")) {
            Some("vi-VN")
        } else {
            lang
        };
        match self.speller.check_in(word, lang) {
            Some(true) => (true, None),
            Some(false) if non_vi && para_vi && vi_ok() => (true, None),
            Some(false) => (false, None),
            None => (true, lang.map(str::to_owned)),
        }
    }

    pub(crate) fn analysis(&mut self, text: &str, spans: &[Span]) -> &Analysis {
        let k = key(text, spans);
        if !self.cache.contains_key(&k) {
            let para_vi = bo_spell::lang::detect(text) == Some("vi-VN");
            let mut misspelt = Vec::new();
            let mut unchecked = BTreeSet::new();
            for r in bo_spell::checkable_words(text) {
                let Some(span) = span_at(spans, r.start) else {
                    continue;
                };
                if span.skip {
                    continue;
                }
                let word = &text[r.clone()];
                let lang = span.lang.as_deref();
                let (ok, none) = self.check_word(word, lang, para_vi);
                if let Some(l) = none {
                    unchecked.insert(l);
                } else if !ok {
                    let checked_in = if lang.is_some_and(|l| bo_spell::primary_language(l) != "vi")
                        && bo_spell::lang::has_vietnamese_letter(word)
                    {
                        Some("vi-VN".to_owned())
                    } else {
                        span.lang.clone()
                    };
                    misspelt.push(Misspelt {
                        range: r,
                        lang: checked_in,
                    });
                }
            }
            let skipped = |r: &Range<usize>| {
                spans.iter().any(|s| {
                    s.skip && s.range.start < r.end.max(r.start + 1) && r.start < s.range.end
                })
            };
            let a = Analysis {
                misspelt,
                issues: grammar::check(text)
                    .into_iter()
                    .filter(|i| !skipped(&i.range))
                    .collect(),
                styles: grammar::tone_styles(text)
                    .into_iter()
                    .filter(|(r, _)| !skipped(r))
                    .collect(),
                unchecked,
            };
            if self.cache.len() > 20_000 {
                self.cache.clear();
            }
            self.cache.insert(k.clone(), a);
        }
        &self.cache[&k]
    }

    /// Suggestions for a misspelt word in the language it was checked in.
    pub(crate) fn suggest(&self, word: &str, lang: Option<&str>) -> Vec<String> {
        self.speller.suggest_in(word, lang)
    }

    /// The tone placement most used in `texts` (`None` when no word shows one).
    pub(crate) fn document_style(&mut self, texts: &[ProofText]) -> Option<ToneStyle> {
        let (mut old, mut new) = (0usize, 0usize);
        for (t, sp) in texts {
            for (_, s) in &self.analysis(t, sp).styles {
                match s {
                    ToneStyle::Old => old += 1,
                    ToneStyle::New => new += 1,
                }
            }
        }
        match (old, new) {
            (0, 0) => None,
            _ if old > new => Some(ToneStyle::Old),
            _ => Some(ToneStyle::New),
        }
    }

    /// Grammar issues of a paragraph not ignored, the tone style ones included.
    pub(crate) fn issues(
        &mut self,
        text: &str,
        spans: &[Span],
        style: Option<ToneStyle>,
    ) -> Vec<Issue> {
        if !self.grammar {
            return Vec::new();
        }
        let a = self.analysis(text, spans);
        let mut out = a.issues.clone();
        let other_style = style.filter(|st| a.styles.iter().any(|(_, s)| s != st));
        if let Some(style) = other_style {
            let styled: Vec<Range<usize>> = a.styles.iter().map(|(r, _)| r.clone()).collect();
            out.extend(
                grammar::tone_style_issues(text, style)
                    .into_iter()
                    .filter(|i| {
                        styled
                            .iter()
                            .any(|r| r.start <= i.range.start && i.range.end <= r.end)
                    }),
            );
        }
        out.retain(|i| {
            !self
                .ignored
                .contains(&(i.rule.id().to_owned(), text[i.range.clone()].to_owned()))
        });
        out
    }

    /// Ignore a grammar issue (rule id, flagged text) from now on.
    pub(crate) fn ignore_issue(&mut self, rule: &str, text: &str) {
        self.ignored.insert((rule.to_owned(), text.to_owned()));
    }

    /// Forget cached results (dictionaries or user words changed).
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }
}

impl Engine {
    /// Loads a Hunspell dictionary (`.aff` + `.dic` bytes) for English spelling (web hosts fetch
    /// them; native builds bundle `en_US`), used for every `en-*` language.
    pub fn load_dictionary(&mut self, aff: &[u8], dic: &[u8]) -> crate::Result<()> {
        self.load_dictionary_for("en", aff, dic)
    }

    /// Loads a Hunspell dictionary for the language `lang` (BCP 47; its primary subtag counts).
    pub fn load_dictionary_for(&mut self, lang: &str, aff: &[u8], dic: &[u8]) -> crate::Result<()> {
        let d =
            bo_spell::Dictionary::from_aff_dic(aff, dic).map_err(crate::EngineError::BadCommand)?;
        self.spell.speller.add_language(lang, Box::new(d));
        self.spell.invalidate();
        Ok(())
    }

    /// Index text and language spans of every body paragraph by id.
    pub(crate) fn body_texts(&self) -> Vec<ProofText> {
        let Some(doc) = self.document() else {
            return Vec::new();
        };
        bo_model::index::body_paragraphs(doc)
            .iter()
            .map(|p| {
                bo_model::index::para_at(doc, p)
                    .map(|para| (bo_model::index::para_index(para).0, spans(doc, para)))
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Marks of the misspelt words on `pages` (all pages when `None`).
    pub fn spelling_marks(&mut self, pages: Option<&[usize]>) -> Vec<SpellMark> {
        if !self.spell.enabled {
            return Vec::new();
        }
        let texts = self.body_texts();
        let Some(layout) = self.layout() else {
            return Vec::new();
        };
        let lines: Vec<(usize, Line)> = layout
            .pages
            .iter()
            .enumerate()
            .filter(|(i, _)| pages.is_none_or(|p| p.contains(i)))
            .flat_map(|(i, p)| {
                p.lines
                    .iter()
                    .filter(|l| matches!(l.source, LineSource::Body(_)))
                    .map(move |l| (i, l.clone()))
            })
            .collect();
        let style = if self.spell.grammar {
            self.spell.document_style(&texts)
        } else {
            None
        };
        let mut out = Vec::new();
        for (page, l) in lines {
            let LineSource::Body(id) = l.source else {
                continue;
            };
            let Some((text, sp)) = texts.get(id as usize) else {
                continue;
            };
            let (ls, le) = (l.start as usize, l.end as usize);
            let mut ranges: Vec<(Range<usize>, bool)> = self
                .spell
                .analysis(text, sp)
                .misspelt
                .iter()
                .map(|m| (m.range.clone(), false))
                .collect();
            ranges.extend(
                self.spell
                    .issues(text, sp, style)
                    .into_iter()
                    .map(|i| (i.range, true)),
            );
            for (r, grammar) in ranges {
                let (s, e) = (r.start.max(ls), r.end.min(le));
                if s >= e {
                    continue;
                }
                let x0 = nav::caret_x(&l, s);
                let x1 = nav::caret_x(&l, e);
                if x1 > x0 {
                    out.push(SpellMark {
                        page,
                        x: x0,
                        y: l.baseline + 1.5,
                        w: x1 - x0,
                        grammar,
                    });
                }
            }
        }
        out
    }

    /// The misspelt word at a point, with suggestions.
    pub fn spell_at(&mut self, page: usize, x: f32, y: f32) -> Option<SpellInfo> {
        let layout = self.layout()?;
        let pos = nav::hit_test(layout, page, x, y)?;
        let texts = self.body_texts();
        let (text, sp) = texts.get(pos.para)?.clone();
        let hit = self
            .spell
            .analysis(&text, &sp)
            .misspelt
            .iter()
            .find(|m| m.range.start <= pos.offset && pos.offset <= m.range.end)
            .cloned();
        if let Some(m) = hit {
            let word = text[m.range.clone()].to_owned();
            return Some(SpellInfo {
                suggestions: self.spell.suggest(&word, m.lang.as_deref()),
                word,
                para: pos.para,
                start: m.range.start,
                end: m.range.end,
                rule: None,
            });
        }
        let style = self.spell.document_style(&texts);
        let issue = self
            .spell
            .issues(&text, &sp, style)
            .into_iter()
            .find(|i| i.range.start <= pos.offset && pos.offset <= i.range.end)?;
        Some(SpellInfo {
            word: text[issue.range.clone()].to_owned(),
            para: pos.para,
            start: issue.range.start,
            end: issue.range.end,
            suggestions: issue.replacement.into_iter().collect(),
            rule: Some(issue.rule.id()),
        })
    }

    /// The result of a command that changes nothing: the open header/footer or note editor stays open
    /// (its selection and story state are reported, as its own commands do).
    pub(crate) fn query_result(&self) -> crate::Result<EditResult> {
        if self.hf.is_some() {
            self.story_result(Vec::new(), None)
        } else if self.note.is_some() {
            self.note_result(Vec::new(), None)
        } else {
            self.edit_result(Vec::new(), (None, None, None), None)
        }
    }

    /// Spelling commands (they never change the document).
    pub(crate) fn spell_command(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        match cmd {
            Command::Spelling { pages } => {
                let marks = self.spelling_marks(pages.as_deref());
                let mut r = self.query_result()?;
                r.spelling = Some(marks);
                Ok(Some(r))
            }
            Command::SpellAt { page, x, y } => {
                let info = self.spell_at(*page, *x, *y);
                let mut r = self.query_result()?;
                r.spell = info;
                Ok(Some(r))
            }
            Command::SpellIgnore { word } | Command::SpellAddWord { word } => {
                if matches!(cmd, Command::SpellIgnore { .. }) {
                    self.spell.speller.ignore(word);
                } else {
                    self.spell.speller.add_user_word(word);
                }
                self.spell.invalidate();
                let mut r = self.query_result()?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::SetGrammar { on } => {
                self.spell.grammar = *on;
                let mut r = self.query_result()?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::GrammarIgnore { rule, text } => {
                self.spell.ignored.insert((rule.clone(), text.clone()));
                let mut r = self.query_result()?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::SetSpelling { on } => {
                self.spell.enabled = *on;
                let mut r = self.query_result()?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::ProofCheck => {
                let report = self.proof_check();
                let mut r = self.query_result()?;
                r.proof = Some(Box::new(report));
                Ok(Some(r))
            }
            Command::ProofOptions {
                ignore_caps,
                ignore_digits,
                auto_language,
            } => {
                if let Some(v) = ignore_caps {
                    self.spell.speller.ignore_all_caps = *v;
                }
                if let Some(v) = ignore_digits {
                    self.spell.speller.ignore_digits = *v;
                }
                if let Some(v) = auto_language {
                    self.spell.auto_language = *v;
                }
                self.spell.invalidate();
                let mut r = self.query_result()?;
                r.proof_options = Some(self.proof_options());
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::ProofEdit { .. } => {
                // Corrections go to the document itself: leave the header/footer or note editor.
                self.close_header_footer()?;
                self.close_note()?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    // source: misspellings of common Vietnamese and English words
    #[test]
    fn marks_and_suggestions_for_misspelt_words() {
        let mut e =
            engine("Xin chaò các bạn, the documnet is ready.\nĐây là văn bản đúng chính tả.");
        let marks = e.spelling_marks(None);
        assert_eq!(marks.len(), 2, "{marks:?}");
        assert!(marks.iter().all(|m| m.page == 0 && m.w > 0.0));
        // The word under the first mark, with its correction.
        let m = &marks[0];
        let info = e.spell_at(0, m.x + m.w / 2.0, m.y - 4.0).unwrap();
        assert_eq!(info.word, "chaò");
        assert!(info.suggestions.contains(&"chào".to_owned()), "{info:?}");
        let m = &marks[1];
        let info = e.spell_at(0, m.x + m.w / 2.0, m.y - 4.0).unwrap();
        assert_eq!(info.word, "documnet");
        assert!(
            info.suggestions.contains(&"document".to_owned()),
            "{info:?}"
        );
        // A correct word gives nothing.
        assert!(e.spell_at(0, 1.0, 1.0).is_none());
    }

    #[test]
    fn commands_ignore_add_and_switch_off() {
        let mut e = engine("Một tư sai: documnet và BlueOffise.");
        let r = e.command(Command::Spelling { pages: None }).unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(2));
        let r = e
            .command(Command::SpellAddWord {
                word: "BlueOffise".into(),
            })
            .unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(1));
        let r = e
            .command(Command::SpellIgnore {
                word: "documnet".into(),
            })
            .unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(0));
        let r = e.command(Command::SetSpelling { on: false }).unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(0));
    }

    // source: grammar rule table of bo_spell::grammar (CORE-GRAM-001).
    #[test]
    fn grammar_marks_menu_and_ignore() {
        let mut e = engine("Xin chào  bạn. hoà bình và hoà hợp, nhưng hòa thuận.");
        let marks = e.spelling_marks(None);
        let grammar: Vec<_> = marks.iter().filter(|m| m.grammar).collect();
        // Extra space, small letter after a full stop, "hòa" in the older style.
        assert_eq!(grammar.len(), 3, "{marks:?}");
        assert_eq!(marks.len(), 3, "no misspelt word: {marks:?}");
        let m = grammar[1];
        let info = e.spell_at(0, m.x + m.w / 2.0, m.y - 4.0).unwrap();
        assert_eq!(info.rule, Some("capitalize"));
        assert_eq!(info.word, "hoà");
        assert_eq!(info.suggestions, ["Hoà"]);
        let m = grammar[2];
        let info = e.spell_at(0, m.x + m.w / 2.0, m.y - 4.0).unwrap();
        assert_eq!(info.rule, Some("toneStyle"));
        assert_eq!(info.suggestions, ["hoà"]);
        let r = e
            .command(Command::GrammarIgnore {
                rule: "toneStyle".into(),
                text: "hòa".into(),
            })
            .unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(2));
        let r = e
            .command_json(r#"{"cmd":"grammarIgnore","rule":"extraSpace","text":"  ","now":1}"#)
            .unwrap();
        assert!(r.contains(r#""spelling":[{"#), "{r}");
        assert_eq!(e.spelling_marks(None).len(), 1);
        let r = e.command(Command::SetGrammar { on: false }).unwrap();
        assert_eq!(r.spelling.as_ref().map(Vec::len), Some(0));
    }

    // Regression: spelling refreshes while a header is open must keep it open (the result carries the
    // story state, as the header editor's own results do).
    #[test]
    fn spelling_keeps_the_header_editor_open() {
        let mut e = engine("Thân văn bản");
        let r = e
            .command(Command::EditHeaderFooter {
                page: 0,
                header: true,
            })
            .unwrap();
        assert!(r.story.is_some());
        let r = e.command(Command::Spelling { pages: None }).unwrap();
        assert!(r.story.is_some(), "header editor still open");
        let r = e.command(Command::SetGrammar { on: true }).unwrap();
        assert!(r.story.is_some());
        let r = e.command(Command::Equation).unwrap();
        assert!(r.story.is_some());
    }
}

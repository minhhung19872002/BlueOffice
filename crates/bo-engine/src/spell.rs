//! Spelling as you type (BlueDoc): misspelt words of the laid-out body text are reported as marks to draw
//! under the text (an overlay — never part of the display list, PDF or print), with suggestions for the
//! word under the pointer (`docs/parity/BACKLOG.md` CORE-SPELL-004). Grammar and style issues
//! (`bo_spell::grammar`, CORE-GRAM-001) come as marks with `grammar: true` (drawn blue); Vietnamese tone
//! placement is checked against the style most used in the document.
//!
//! Vietnamese is always checked (rule based, `bo_spell::vi`); other languages come from Hunspell
//! dictionaries loaded with [`crate::Engine::load_dictionary`] (native builds bundle `en_US`). A word is
//! correct when any language accepts it (Vietnamese documents mix in English terms).

use crate::edit::{Command, EditResult};
use crate::nav;
use crate::Engine;
use bo_layout::{Line, LineSource};
use bo_spell::grammar::{self, Issue};
use bo_spell::vi::ToneStyle;
use bo_spell::Speller;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

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
struct Analysis {
    misspelt: Vec<Range<usize>>,
    issues: Vec<Issue>,
    styles: Vec<(Range<usize>, ToneStyle)>,
}

/// Speller of the engine plus a cache of misspelt ranges per paragraph text.
pub(crate) struct SpellState {
    pub speller: Speller,
    pub enabled: bool,
    pub grammar: bool,
    /// Ignored grammar issues: (rule id, flagged text).
    ignored: HashSet<(String, String)>,
    cache: HashMap<String, Analysis>,
}

impl Default for SpellState {
    fn default() -> Self {
        let mut speller = Speller::new();
        speller.add(Box::new(bo_spell::vi::Vietnamese));
        SpellState {
            speller,
            enabled: true,
            grammar: true,
            ignored: HashSet::new(),
            cache: HashMap::new(),
        }
    }
}

impl SpellState {
    fn analysis(&mut self, text: &str) -> &Analysis {
        if !self.cache.contains_key(text) {
            let a = Analysis {
                misspelt: self.speller.misspelt(text),
                issues: grammar::check(text),
                styles: grammar::tone_styles(text),
            };
            if self.cache.len() > 20_000 {
                self.cache.clear();
            }
            self.cache.insert(text.to_owned(), a);
        }
        &self.cache[text]
    }

    /// The tone placement most used in `texts` (`None` when no word shows one).
    fn document_style(&mut self, texts: &[String]) -> Option<ToneStyle> {
        let (mut old, mut new) = (0usize, 0usize);
        for t in texts {
            for (_, s) in &self.analysis(t).styles {
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
    fn issues(&mut self, text: &str, style: Option<ToneStyle>) -> Vec<Issue> {
        if !self.grammar {
            return Vec::new();
        }
        let mut out = self.analysis(text).issues.clone();
        if let Some(style) = style {
            if self.analysis(text).styles.iter().any(|(_, s)| *s != style) {
                out.extend(grammar::tone_style_issues(text, style));
            }
        }
        out.retain(|i| {
            !self
                .ignored
                .contains(&(i.rule.id().to_owned(), text[i.range.clone()].to_owned()))
        });
        out
    }

    /// Forget cached results (dictionaries or user words changed).
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }
}

impl Engine {
    /// Loads a Hunspell dictionary (`.aff` + `.dic` bytes) for spelling (web hosts fetch them; native
    /// builds bundle `en_US`).
    pub fn load_dictionary(&mut self, aff: &[u8], dic: &[u8]) -> crate::Result<()> {
        let d =
            bo_spell::Dictionary::from_aff_dic(aff, dic).map_err(crate::EngineError::BadCommand)?;
        self.spell.speller.add(Box::new(d));
        self.spell.invalidate();
        Ok(())
    }

    /// Index text of every body paragraph by id.
    fn body_texts(&self) -> Vec<String> {
        let Some(doc) = self.document() else {
            return Vec::new();
        };
        bo_model::index::body_paragraphs(doc)
            .iter()
            .map(|p| {
                bo_model::index::para_at(doc, p)
                    .map(|para| bo_model::index::para_index(para).0)
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
            let Some(text) = texts.get(id as usize) else {
                continue;
            };
            let (ls, le) = (l.start as usize, l.end as usize);
            let mut ranges: Vec<(Range<usize>, bool)> = self
                .spell
                .analysis(text)
                .misspelt
                .iter()
                .map(|r| (r.clone(), false))
                .collect();
            ranges.extend(
                self.spell
                    .issues(text, style)
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
        let text = texts.get(pos.para)?.clone();
        let word = bo_spell::words(&text)
            .into_iter()
            .find(|r| r.start <= pos.offset && pos.offset <= r.end);
        if let Some(r) = word {
            let word = text[r.clone()].to_owned();
            if !self.spell.speller.check(&word) {
                return Some(SpellInfo {
                    suggestions: self.spell.speller.suggest(&word),
                    word,
                    para: pos.para,
                    start: r.start,
                    end: r.end,
                    rule: None,
                });
            }
        }
        let style = self.spell.document_style(&texts);
        let issue = self
            .spell
            .issues(&text, style)
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

    /// Spelling commands (they never change the document).
    pub(crate) fn spell_command(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        match cmd {
            Command::Spelling { pages } => {
                let marks = self.spelling_marks(pages.as_deref());
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.spelling = Some(marks);
                Ok(Some(r))
            }
            Command::SpellAt { page, x, y } => {
                let info = self.spell_at(*page, *x, *y);
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
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
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::SetGrammar { on } => {
                self.spell.grammar = *on;
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::GrammarIgnore { rule, text } => {
                self.spell.ignored.insert((rule.clone(), text.clone()));
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
            }
            Command::SetSpelling { on } => {
                self.spell.enabled = *on;
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.spelling = Some(self.spelling_marks(None));
                Ok(Some(r))
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
}

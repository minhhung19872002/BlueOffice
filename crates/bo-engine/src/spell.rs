//! Spelling as you type (BlueDoc): misspelt words of the laid-out body text are reported as marks to draw
//! under the text (an overlay — never part of the display list, PDF or print), with suggestions for the
//! word under the pointer (`docs/parity/BACKLOG.md` CORE-SPELL-004).
//!
//! Vietnamese is always checked (rule based, `bo_spell::vi`); other languages come from Hunspell
//! dictionaries loaded with [`crate::Engine::load_dictionary`] (native builds bundle `en_US`). A word is
//! correct when any language accepts it (Vietnamese documents mix in English terms).

use crate::edit::{Command, EditResult};
use crate::nav;
use crate::Engine;
use bo_layout::{Line, LineSource};
use bo_spell::Speller;
use serde::Serialize;
use std::collections::HashMap;
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
}

/// Speller of the engine plus a cache of misspelt ranges per paragraph text.
pub(crate) struct SpellState {
    pub speller: Speller,
    pub enabled: bool,
    cache: HashMap<String, Vec<Range<usize>>>,
}

impl Default for SpellState {
    fn default() -> Self {
        let mut speller = Speller::new();
        speller.add(Box::new(bo_spell::vi::Vietnamese));
        SpellState {
            speller,
            enabled: true,
            cache: HashMap::new(),
        }
    }
}

impl SpellState {
    fn misspelt(&mut self, text: &str) -> &[Range<usize>] {
        if !self.cache.contains_key(text) {
            let r = self.speller.misspelt(text);
            if self.cache.len() > 20_000 {
                self.cache.clear();
            }
            self.cache.insert(text.to_owned(), r);
        }
        self.cache.get(text).map_or(&[], Vec::as_slice)
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
        let mut out = Vec::new();
        for (page, l) in lines {
            let LineSource::Body(id) = l.source else {
                continue;
            };
            let Some(text) = texts.get(id as usize) else {
                continue;
            };
            let (ls, le) = (l.start as usize, l.end as usize);
            for r in self.spell.misspelt(text) {
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
        let text = self.body_texts().get(pos.para)?.clone();
        let r = bo_spell::words(&text)
            .into_iter()
            .find(|r| r.start <= pos.offset && pos.offset <= r.end)?;
        let word = text[r.clone()].to_owned();
        if self.spell.speller.check(&word) {
            return None;
        }
        Some(SpellInfo {
            suggestions: self.spell.speller.suggest(&word),
            word,
            para: pos.para,
            start: r.start,
            end: r.end,
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
}

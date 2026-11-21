//! Smart cut and paste of the main text (BlueDoc round 10): like the desktop word processor's
//! "Use smart cut and paste" (on by default), cutting or moving away a word leaves no double
//! space and no space before punctuation, and pasting or dropping a word adds the space it needs
//! on either side and drops a space of its own that would double one. Measured on the reference
//! with COM `Selection.Cut` / `Paste` (docs/ui/REPORT.md "Vòng 10"):
//! "The quick brown fox." − "fox" → "The quick brown."; "quick brown" − "quick" → "brown";
//! "one two, three" − "two" → "one, three"; "The quick|fox" + "brown" → "The quick brown fox";
//! "qu|ick" + "brown" → "qu brown ick"; "a (|b)" + "x" → "a (x b)"; "fox|." + "brown " →
//! "fox brown.".
use crate::edit::{Command, EditResult};
use crate::Engine;
use bo_edit::{Pos, Selection, Session};

/// Characters a word pasted right after needs no space from.
fn opens(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | '[' | '{' | '"' | '“' | '‘' | '\'' | '/' | '\t')
}

/// Characters a space before which is removed when the word between them goes away.
fn closes(c: char) -> bool {
    matches!(
        c,
        ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '”' | '’'
    )
}

/// The text is one or more whole words: it starts and ends (spaces aside) with a letter or digit.
fn wordlike(text: &str) -> bool {
    let t = text.trim_matches(' ');
    !t.contains(['\n', '\r'])
        && t.chars().next().is_some_and(char::is_alphanumeric)
        && t.chars().next_back().is_some_and(char::is_alphanumeric)
}

fn char_before(s: &Session, at: Pos) -> Option<char> {
    s.para_text(at.para)?.get(..at.offset)?.chars().next_back()
}

fn char_at(s: &Session, at: Pos) -> Option<char> {
    s.para_text(at.para)?.get(at.offset..)?.chars().next()
}

/// Delete the one character after `at` (a space).
fn delete_after(s: &mut Session, at: Pos) -> bo_edit::Result<()> {
    s.set_selection(Selection {
        anchor: at,
        focus: Pos::new(at.para, at.offset + 1),
    });
    s.delete_forward()?;
    Ok(())
}

/// After `text` was cut from the caret: remove the space it leaves doubled, before punctuation
/// or at a paragraph's start or end. True when a space went.
pub(crate) fn fix_after_cut(s: &mut Session, text: &str) -> bo_edit::Result<bool> {
    if !wordlike(text) || s.track_changes() {
        return Ok(false);
    }
    let at = s.selection().focus;
    let (before, after) = (char_before(s, at), char_at(s, at));
    if before == Some(' ') && after.is_none_or(|c| c == ' ' || closes(c)) {
        delete_after(s, Pos::new(at.para, at.offset - 1))?;
        s.set_selection(Selection::caret(Pos::new(at.para, at.offset - 1)));
        return Ok(true);
    }
    if before.is_none() && after == Some(' ') {
        delete_after(s, at)?;
        s.set_selection(Selection::caret(at));
        return Ok(true);
    }
    Ok(false)
}

/// After text was pasted from `start` to the caret (one paragraph): add the spaces it needs
/// and drop its own spaces that would double one. Keeps the pasted text selected when it was
/// (`select`), else the caret after it.
pub(crate) fn fix_after_paste(s: &mut Session, start: Pos, select: bool) -> bo_edit::Result<bool> {
    let end = s.selection().focus;
    if s.track_changes() || end.para != start.para || end.offset <= start.offset {
        return Ok(false);
    }
    let Some(text) = s.para_text(start.para) else {
        return Ok(false);
    };
    let Some(pasted) = text.get(start.offset..end.offset) else {
        return Ok(false);
    };
    if !wordlike(pasted) {
        return Ok(false);
    }
    let (first, last) = (pasted.chars().next(), pasted.chars().next_back());
    let (before, after) = (char_before(s, start), char_at(s, end));
    let (mut a, mut b) = (start, end);
    let mut changed = false;
    // The end first, so `start` keeps its place.
    if last == Some(' ') && after.is_none_or(|c| c == ' ' || closes(c)) {
        delete_after(s, Pos::new(b.para, b.offset - 1))?;
        b.offset -= 1;
        changed = true;
    } else if last.is_some_and(char::is_alphanumeric) && after.is_some_and(char::is_alphanumeric) {
        s.set_selection(Selection::caret(b));
        s.insert_text(" ")?;
        changed = true;
    }
    if first == Some(' ') && before.is_none_or(|c| c == ' ') {
        delete_after(s, a)?;
        b.offset -= 1;
        changed = true;
    } else if first.is_some_and(char::is_alphanumeric) && before.is_some_and(|c| !opens(c)) {
        s.set_selection(Selection::caret(a));
        s.insert_text(" ")?;
        a.offset += 1;
        b.offset += 1;
        changed = true;
    }
    if changed {
        s.set_selection(if select {
            Selection {
                anchor: a,
                focus: b,
            }
        } else {
            Selection::caret(b)
        });
    }
    Ok(changed)
}

/// An editing error as an engine error.
#[allow(clippy::needless_pass_by_value)] // used as map_err(bad).
pub(crate) fn bad(e: bo_edit::EditError) -> crate::EngineError {
    crate::EngineError::BadCommand(e.to_string())
}

impl Engine {
    /// Smart cut and paste is on (File › Options › Advanced, on by default).
    pub fn smart_cut_paste(&self) -> bool {
        !self.smart_off
    }

    /// Switch smart cut and paste on or off.
    pub fn set_smart_cut_paste(&mut self, on: bool) {
        self.smart_off = !on;
    }

    /// Cut and paste of the main text with smart spacing; other commands as they are.
    pub(crate) fn smart_wrap(
        &mut self,
        cmd: Command,
        now: Option<i64>,
    ) -> crate::Result<EditResult> {
        let main = self.hf.is_none() && self.note.is_none() && self.textbox.is_none();
        let smart = main
            && !self.smart_off
            && matches!(cmd, Command::Cut | Command::Paste { .. })
            && self.session.is_some();
        if !smart {
            return self.af_wrap(cmd, now);
        }
        let (depth, start) = match self.session.as_ref() {
            Some(s) => (s.history_len(), s.selection().ordered().0),
            None => return self.af_wrap(cmd, now),
        };
        let cut = matches!(cmd, Command::Cut);
        let mut r = self.command_inner(cmd, now)?;
        let text = r.clip.as_ref().map(|c| c.text.clone()).unwrap_or_default();
        let Some(s) = self.session.as_mut() else {
            return Ok(r);
        };
        if s.history_len() == depth {
            return Ok(r);
        }
        let fixed = if cut {
            fix_after_cut(s, &text).map_err(bad)?
        } else {
            fix_after_paste(s, start, false).map_err(bad)?
        };
        if !fixed {
            return Ok(r);
        }
        s.merge_history_since(depth);
        let clip = r.clip.take();
        let mut dirty = std::mem::take(&mut r.dirty);
        dirty.extend(self.relayout()?);
        dirty.sort_unstable();
        dirty.dedup();
        let mut out = self.edit_result(dirty, (None, None, None), None)?;
        out.clip = clip;
        Ok(out)
    }
}

#[cfg(test)]
#[path = "smart_cut_tests.rs"]
mod tests;

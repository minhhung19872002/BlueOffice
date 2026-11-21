//! "Automatically add words to list" of the AutoCorrect Exceptions dialog (BlueDoc round 9):
//! a correction made as you type and undone right away (Ctrl+Z or the AutoCorrect Options
//! mark's Change Back) teaches AutoCorrect, as the desktop word processor does — an undone
//! sentence capital adds the abbreviation before it to the First Letter list, undone two
//! initial capitals add the word to INitial CAps, any other undone replacement adds the word as
//! typed to Other Corrections (then neither the replacement list nor the spelling checker's
//! suggestion changes it again).
use crate::edit::{Command, EditResult};
use crate::Engine;
use bo_edit::autocorrect::Rule;

/// The last correction made as you type: the rule, the history length right after it and
/// the paragraph it changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LastCorrection {
    rule: Rule,
    depth: usize,
    para: usize,
}

/// What to compare once an Undo has run.
pub(crate) struct Learn {
    last: LastCorrection,
    corrected: String,
}

impl Engine {
    /// Before a command: an Undo of the correction just made is noted with the corrected text.
    pub(crate) fn ac_learn_before(&self, cmd: &Command) -> Option<Learn> {
        if !matches!(cmd, Command::Undo) {
            return None;
        }
        let last = self.ac_last?;
        let s = self.session.as_ref()?;
        if s.history_len() != last.depth {
            return None;
        }
        Some(Learn {
            last,
            corrected: s.para_text(last.para)?,
        })
    }

    /// After a command: remember a correction it made, or learn from the undone one.
    pub(crate) fn ac_learn_after(&mut self, learn: Option<Learn>, r: &EditResult) {
        if let Some(l) = learn {
            self.ac_last = None;
            let original = self.session.as_ref().and_then(|s| s.para_text(l.last.para));
            if let Some(original) = original {
                self.ac_learn(l.last.rule, &l.corrected, &original);
            }
            return;
        }
        let Some(s) = self.session.as_ref() else {
            return;
        };
        self.ac_last = r.corrected.map(|rule| {
            let at = s.selection().focus;
            // A correction made by Enter changed the paragraph before the new one.
            let para = if at.offset == 0
                && at.para > 0
                && s.para_text(at.para).is_some_and(|t| t.is_empty())
            {
                at.para - 1
            } else {
                at.para
            };
            LastCorrection {
                rule,
                depth: s.history_len(),
                para,
            }
        });
    }

    /// Add the word the undone correction had changed to the matching exception list.
    fn ac_learn(&mut self, rule: Rule, corrected: &str, original: &str) {
        let Some(o) = self.autocorrect.as_mut() else {
            return;
        };
        let Some((start, end)) = changed_word(corrected, original) else {
            return;
        };
        let word = &original[start..end];
        match rule {
            Rule::Capitalize if o.auto_add_first => {
                // The abbreviation before the word whose capital was undone.
                let before = original[..start].trim_end();
                let abbr = before
                    .rsplit(char::is_whitespace)
                    .next()
                    .unwrap_or_default();
                if abbr.ends_with('.') && abbr.len() > 1 && !o.exceptions.iter().any(|x| x == abbr)
                {
                    o.exceptions.push(abbr.to_owned());
                }
            }
            Rule::TwoCaps if o.auto_add_caps => {
                if !o.caps_exceptions.iter().any(|x| x == word) {
                    o.caps_exceptions.push(word.to_owned());
                }
            }
            Rule::Replace | Rule::Spelling | Rule::CapsLock if o.auto_add_other => {
                if !o.is_other_exception(word) {
                    o.other_exceptions.push(word.to_owned());
                }
            }
            _ => {}
        }
    }
}

/// The word of `original` around the part that differs from `corrected` (byte range).
fn changed_word(corrected: &str, original: &str) -> Option<(usize, usize)> {
    if corrected == original {
        return None;
    }
    let pre = corrected
        .char_indices()
        .zip(original.chars())
        .find(|((_, a), b)| a != b)
        .map_or(corrected.len().min(original.len()), |((i, _), _)| i);
    let suf = corrected
        .chars()
        .rev()
        .zip(original.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(_, b)| b.len_utf8())
        .sum::<usize>()
        .min(original.len() - pre);
    let (mut s, mut e) = (pre, original.len() - suf);
    let word = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
    while let Some(c) = original[..s].chars().next_back().filter(|c| word(*c)) {
        s -= c.len_utf8();
    }
    while let Some(c) = original[e..].chars().next().filter(|c| word(*c)) {
        e += c.len_utf8();
    }
    (e > s).then_some((s, e))
}

#[cfg(test)]
mod tests {
    use super::changed_word;
    use crate::edit::Command;
    use crate::Engine;

    fn engine() -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
        e.command(Command::AutoCorrect {
            options: None,
            reset: true,
        })
        .expect("autocorrect on");
        e
    }

    fn typed(e: &mut Engine, t: &str) {
        for c in t.chars() {
            e.command(Command::InsertText {
                text: c.to_string(),
            })
            .expect("type");
        }
    }

    #[test]
    fn the_changed_word_is_found_around_the_difference() {
        assert_eq!(changed_word("a Ids b", "a IDs b"), Some((2, 5)));
        assert_eq!(changed_word("x the ", "x teh "), Some((2, 5)));
        assert_eq!(changed_word("same", "same"), None);
    }

    #[test]
    fn undone_corrections_teach_the_exception_lists() {
        let mut e = engine();
        typed(&mut e, "We use ZQuad ");
        e.command(Command::Undo).expect("undo");
        let o = e.autocorrect_options().expect("options");
        assert!(o.caps_exceptions.iter().any(|w| w == "ZQuad"));
        typed(&mut e, "and teh ");
        e.command(Command::Undo).expect("undo");
        let o = e.autocorrect_options().expect("options");
        assert!(o.is_other_exception("teh"), "{:?}", o.other_exceptions);
        // Learnt: the word stays as typed from now on.
        typed(&mut e, "teh ");
        let s = e.session().expect("session");
        assert!(s.para_text(0).unwrap_or_default().ends_with("teh teh "));
        typed(&mut e, "zq. word ");
        e.command(Command::Undo).expect("undo");
        let o = e.autocorrect_options().expect("options");
        assert!(
            o.exceptions.iter().any(|w| w == "zq."),
            "{:?}",
            o.exceptions
        );
        // Undo of a later edit teaches nothing.
        let n = e.autocorrect_options().expect("o").caps_exceptions.len();
        typed(&mut e, "THis x");
        e.command(Command::Undo).expect("undo");
        assert_eq!(e.autocorrect_options().expect("o").caps_exceptions.len(), n);
    }

    #[test]
    fn auto_add_off_keeps_the_lists() {
        let mut e = engine();
        let mut o = e.autocorrect_options().expect("options").clone();
        o.auto_add_caps = false;
        e.command(Command::AutoCorrect {
            options: Some(Box::new(o)),
            reset: false,
        })
        .expect("options");
        typed(&mut e, "We use ZQuad ");
        e.command(Command::Undo).expect("undo");
        let o = e.autocorrect_options().expect("options");
        assert!(!o.caps_exceptions.iter().any(|w| w == "ZQuad"));
    }
}

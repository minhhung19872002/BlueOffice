//! AutoFormat As You Type of BlueDoc round 10, measured on the reference (posted keys into its
//! own instance, results read with COM; docs/ui/REPORT.md "Vòng 10"):
//!
//! - **Format beginning of list item like the one before** (on by default): after "• **Term**:
//!   rest" + Enter, typing "Next: more" gives "**Next**: more" — the new item starts with the
//!   formatting of the previous item's lead, which ends when the same punctuation is typed;
//!   without it ("Plain") the whole text stays bold.
//! - **Built-in Heading styles** (off by default): a line of at most five words that starts with
//!   a letter and does not end in punctuation, followed by two Enters, becomes Heading 1; the
//!   second Enter does not add a paragraph. A line after an empty paragraph, a single letter or
//!   a line starting with a tab stays Normal.
//! - **Define styles based on your formatting** (off by default): the reference creates no
//!   style for paragraphs formatted by hand (centred bold, italic), so neither does BlueDoc.
use crate::edit::{Command, EditResult};
use crate::Engine;
use bo_edit::{CharFormat, Pos, Session};

/// The formatted lead the next list item starts with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListLead {
    /// Paragraph of the new item.
    para: usize,
    /// Typing this character ends the lead's formatting.
    stop: char,
    /// Toggles applied at the item's start (undone at `stop`).
    formats: Vec<CharFormat>,
}

/// Punctuation that ends a list item's formatted lead.
fn lead_stop(c: char) -> bool {
    matches!(c, ':' | '.' | ',' | ';' | '-' | '–' | '—' | ')')
}

/// The formatted lead of list paragraph `para`: the character ending it and the toggles of its
/// formatting (bold, italic, underline that the text after it does not have).
fn lead_of(s: &Session, para: usize) -> Option<(char, Vec<CharFormat>)> {
    let doc = s.doc();
    let p = bo_model::index::para_by_id(doc, para)?;
    let (text, spans) = bo_model::index::para_index(p);
    let first = spans.iter().find(|sp| sp.end > sp.start)?;
    let lead = p.runs.get(first.run)?;
    let base = |r: &bo_model::RunProps| {
        (
            r.bold.unwrap_or(false),
            r.italic.unwrap_or(false),
            r.underline.is_some_and(|u| u != bo_model::Underline::None),
        )
    };
    let fmt = base(&lead.props);
    if fmt == (false, false, false) {
        return None;
    }
    // The lead: the leading items of that formatting.
    let end = spans
        .iter()
        .take_while(|sp| p.runs.get(sp.run).is_some_and(|r| base(&r.props) == fmt))
        .map(|sp| sp.end)
        .last()?;
    let stop = text.get(end..)?.chars().next().filter(|c| lead_stop(*c))?;
    let after = spans
        .iter()
        .find(|sp| sp.start >= end && sp.end > sp.start)?;
    let rest = base(&p.runs.get(after.run)?.props);
    let mut formats = Vec::new();
    if fmt.0 && !rest.0 {
        formats.push(CharFormat::Bold);
    }
    if fmt.1 && !rest.1 {
        formats.push(CharFormat::Italic);
    }
    if fmt.2 && !rest.2 {
        formats.push(CharFormat::Underline);
    }
    (!formats.is_empty()).then_some((stop, formats))
}

fn in_list(s: &Session, para: usize) -> bool {
    bo_model::index::para_by_id(s.doc(), para)
        .is_some_and(|p| p.props.num.is_some_and(|n| n.num_id != 0))
}

fn body_level(s: &Session, para: usize) -> bool {
    bo_model::index::para_path(s.doc(), para).is_some_and(|p| p.len() == 2)
}

/// The paragraph qualifies as a heading typed as you go (see the module notes).
fn heading_like(text: &str) -> bool {
    let t = text.trim_end();
    let words = t.split_whitespace().count();
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    !t.starts_with([' ', '\t'])
        && (1..=5).contains(&words)
        && letters >= 2
        && t.chars().next().is_some_and(char::is_alphabetic)
        && !t
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_punctuation() || matches!(c, '…' | '”' | '’'))
}

impl Engine {
    /// AutoFormat As You Type of round 10 around `cmd` (see the module notes).
    pub(crate) fn af_wrap(&mut self, cmd: Command, now: Option<i64>) -> crate::Result<EditResult> {
        // Only the two flags (the options hold the long replacement list).
        let opts = self
            .autocorrect
            .as_ref()
            .filter(|o| o.enabled)
            .map(|o| (o.builtin_headings, o.format_list_beginning));
        let Some((headings, list_beginning)) = opts else {
            self.list_lead = None;
            return self.command_inner(cmd, now);
        };
        match &cmd {
            Command::SplitParagraph => {
                let Some(s) = self.session.as_ref() else {
                    return self.command_inner(cmd, now);
                };
                let sel = s.selection();
                let at = sel.focus;
                let text = s.para_text(at.para).unwrap_or_default();
                // Built-in Heading styles: the second Enter after a short line.
                if headings
                    && sel.is_collapsed()
                    && text.is_empty()
                    && at.para > 0
                    && body_level(s, at.para)
                    && body_level(s, at.para - 1)
                    && !in_list(s, at.para - 1)
                    && s.para_text(at.para - 1).is_some_and(|t| heading_like(&t))
                    && (at.para == 1 || s.para_text(at.para - 2).is_some_and(|t| !t.is_empty()))
                {
                    self.list_lead = None;
                    let caret = sel;
                    self.command_inner(
                        Command::SetSelection {
                            selection: bo_edit::Selection::caret(Pos::new(at.para - 1, 0)),
                        },
                        now,
                    )?;
                    self.command_inner(
                        Command::ApplyStyle {
                            id: "Heading1".into(),
                        },
                        now,
                    )?;
                    return self.command_inner(Command::SetSelection { selection: caret }, now);
                }
                let lead = (list_beginning
                    && sel.is_collapsed()
                    && at.offset == text.len()
                    && !text.is_empty()
                    && in_list(s, at.para))
                .then(|| lead_of(s, at.para))
                .flatten();
                let r = self.command_inner(cmd, now)?;
                self.list_lead = None;
                let Some((stop, formats)) = lead else {
                    return Ok(r);
                };
                let Some(s) = self.session.as_mut() else {
                    return Ok(r);
                };
                let p = s.selection().focus.para;
                if !in_list(s, p) || !s.para_text(p).is_some_and(|t| t.is_empty()) {
                    return Ok(r);
                }
                for f in &formats {
                    s.format_text(f).map_err(crate::smart_cut::bad)?;
                }
                self.list_lead = Some(ListLead {
                    para: p,
                    stop,
                    formats,
                });
                Ok(r)
            }
            Command::InsertText { text } => {
                let lead = self.list_lead.clone();
                let mut chars = text.chars();
                let typed = (chars.next(), chars.next());
                if let (Some(lead), Some(s)) = (lead, self.session.as_mut()) {
                    let at = s.selection().focus;
                    if at.para != lead.para {
                        self.list_lead = None;
                    } else if typed == (Some(lead.stop), None) {
                        for f in &lead.formats {
                            s.format_text(f).map_err(crate::smart_cut::bad)?;
                        }
                        self.list_lead = None;
                    }
                }
                self.command_inner(cmd, now)
            }
            Command::State | Command::Spelling { .. } => self.command_inner(cmd, now),
            _ => {
                self.list_lead = None;
                self.command_inner(cmd, now)
            }
        }
    }
}

#[cfg(test)]
#[path = "autoformat10_tests.rs"]
mod tests;

//! Names of the undo / redo steps of the main text, for the quick access toolbar's undo
//! list (BlueDoc round 8): one entry per transaction of the session's history, kept in step
//! with it after every command (typing that joins the last step extends its text).
use crate::edit::{Command, EditResult};
use crate::Engine;

/// One undo or redo step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoLabel {
    /// Variant name of the command that made the step (`InsertText`, `FormatText`, …), or
    /// `AutoCorrect` for a correction made as you type.
    pub kind: String,
    /// The text typed in the step (`InsertText`), the formatting's variant name
    /// (`FormatText`: `Bold`, `Size`, …), else empty.
    pub text: String,
}

/// Undo and redo step names (most recent last).
#[derive(Debug, Clone, Default)]
pub(crate) struct UndoLabels {
    undo: Vec<UndoLabel>,
    redo: Vec<UndoLabel>,
}

/// What the history looked like before a command.
pub(crate) struct Before {
    len: usize,
    version: u64,
}

/// The value's variant name without formatting its fields (the writer stops at the first
/// character that is not part of an identifier).
fn variant_name(cmd: &dyn std::fmt::Debug) -> String {
    struct Ident(String);
    impl std::fmt::Write for Ident {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            for c in s.chars() {
                if c.is_alphanumeric() || c == '_' {
                    self.0.push(c);
                } else {
                    return Err(std::fmt::Error);
                }
            }
            Ok(())
        }
    }
    let mut w = Ident(String::new());
    let _ = std::fmt::write(&mut w, format_args!("{cmd:?}"));
    w.0
}

impl Engine {
    pub(crate) fn undo_labels_before(&self) -> Option<Before> {
        let s = self.session.as_ref()?;
        Some(Before {
            len: s.history_len(),
            version: s.version(),
        })
    }

    /// Bring the step names in line with the history after `cmd` (`typed`: its text when
    /// it typed some, `kind`: its variant name).
    pub(crate) fn undo_labels_after(
        &mut self,
        before: Option<Before>,
        kind: &str,
        typed: Option<String>,
        r: &EditResult,
    ) {
        let (Some(b), Some(s)) = (before, self.session.as_ref()) else {
            return;
        };
        let (len, version, can_redo) = (s.history_len(), s.version(), s.can_redo());
        let l = &mut self.undo_labels;
        match kind {
            "Undo" if len < b.len => {
                for _ in len..b.len {
                    if let Some(x) = l.undo.pop() {
                        l.redo.push(x);
                    }
                }
            }
            "Redo" if len > b.len => {
                for _ in b.len..len {
                    let x = l.redo.pop().unwrap_or_else(|| UndoLabel {
                        kind: "Edit".into(),
                        text: String::new(),
                    });
                    l.undo.push(x);
                }
            }
            _ if len > b.len => {
                l.redo.clear();
                let n = len - b.len;
                // A correction made as you type is the last step; the typing before it may
                // have joined the previous step.
                let corrected = usize::from(r.corrected.is_some()).min(n);
                if corrected == 1 && n == 1 {
                    if let (Some(last), Some(t)) = (l.undo.last_mut(), typed.as_ref()) {
                        if last.kind == "InsertText" {
                            last.text.push_str(t);
                        }
                    }
                }
                for i in 0..n {
                    let auto = i + corrected >= n;
                    l.undo.push(UndoLabel {
                        kind: if auto {
                            "AutoCorrect".into()
                        } else {
                            kind.to_owned()
                        },
                        text: if i == 0 && !auto {
                            typed.clone().unwrap_or_default()
                        } else {
                            String::new()
                        },
                    });
                }
            }
            // Typing joined the last step.
            _ if len == b.len && version != b.version => {
                l.redo.clear();
                if let (Some(last), Some(t)) = (l.undo.last_mut(), typed) {
                    if last.kind == "InsertText" {
                        last.text.push_str(&t);
                    }
                }
            }
            _ => {}
        }
        // Steps dropped or merged elsewhere (history limits, script runs): keep the newest.
        if l.undo.len() > len {
            let extra = l.undo.len() - len;
            l.undo.drain(..extra);
        }
        while l.undo.len() < len {
            l.undo.insert(
                0,
                UndoLabel {
                    kind: "Edit".into(),
                    text: String::new(),
                },
            );
        }
        if !can_redo {
            l.redo.clear();
        }
    }

    /// Names of the steps Undo would revert, most recent first (main text).
    pub fn undo_list(&self) -> Vec<UndoLabel> {
        self.undo_labels.undo.iter().rev().cloned().collect()
    }

    /// Typing joins one undo step until something else happens (a caret move, a
    /// correction, formatting), across word boundaries, as on the desktop word processor;
    /// off: one step per word.
    pub fn set_typing_runs(&mut self, on: bool) {
        self.typing_runs = on;
        if let Some(s) = self.session.as_mut() {
            s.set_typing_runs(on);
        }
    }

    /// Prepare `cmd` for the step names: its variant name and typed text.
    pub(crate) fn undo_label_of(cmd: &Command) -> (String, Option<String>) {
        match cmd {
            Command::InsertText { text } => ("InsertText".into(), Some(text.clone())),
            // The reference names a step by the formatting ("Bold").
            Command::FormatText { format } => ("FormatText".into(), Some(variant_name(format))),
            other => (variant_name(other), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
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
    fn variant_names_stop_before_the_fields() {
        assert_eq!(variant_name(&Command::Undo), "Undo");
        assert_eq!(
            variant_name(&Command::InsertText {
                text: "x".repeat(10_000)
            }),
            "InsertText"
        );
    }

    #[test]
    fn typing_runs_join_words_and_labels_follow_undo_and_redo() {
        let mut e = engine();
        e.set_typing_runs(true);
        typed(&mut e, "one two");
        let list = e.undo_list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "one two");
        e.command(Command::SelectAll).expect("select");
        e.command(Command::FormatText {
            format: bo_edit::CharFormat::Bold,
        })
        .expect("bold");
        let list = e.undo_list();
        let kinds: Vec<&str> = list.iter().map(|l| l.kind.as_str()).collect();
        assert_eq!(kinds, ["FormatText", "InsertText"]);
        assert_eq!(list[0].text, "Bold");
        e.command(Command::Undo).expect("undo");
        assert_eq!(e.undo_list().len(), 1);
        e.command(Command::Redo).expect("redo");
        assert_eq!(e.undo_list()[0].kind, "FormatText");
        e.command(Command::Undo).expect("undo");
        e.command(Command::Undo).expect("undo");
        assert_eq!(e.undo_list(), Vec::new());
    }

    #[test]
    fn without_typing_runs_each_word_is_a_step() {
        let mut e = engine();
        typed(&mut e, "one two");
        assert_eq!(e.undo_list().len(), 2);
    }
}

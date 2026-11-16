//! Review > Spelling (F7): the Spelling pane walks the deck's misspelt words
//! from the current slide on, as the reference presentation app does — the flagged word is
//! selected on its slide, the pane offers Ignore Once / Ignore All / Add and the corrections
//! with Change / Change All; at the end "Spell check complete". Checking and corrections are
//! the engine's (`slideSpellCheck`, `slideSpellIgnore`, `slideSpellAddWord`,
//! [`bo_engine::Engine::slide_spell_fix`]).
use super::selpane::{SideHit, SideKind, SpellShown};
use super::SlideWindow;
use bo_engine::slide_edit::SlideCommand;
use serde_json::{json, Value};

/// The Spelling pane's walk through the deck.
#[derive(Debug, Default)]
pub(super) struct SpellRun {
    /// The issue shown (`None`: the check is complete).
    issue: Option<Value>,
    /// Occurrences skipped with Ignore Once.
    skipped: Vec<String>,
    /// The chosen correction.
    chosen: usize,
}

/// Identity of an occurrence (target, paragraph, offset, word).
fn key(issue: &Value) -> String {
    format!(
        "{}|{}|{}|{}",
        issue
            .get("target")
            .map(Value::to_string)
            .unwrap_or_default(),
        issue.get("para").and_then(Value::as_u64).unwrap_or(0),
        issue.get("start").and_then(Value::as_u64).unwrap_or(0),
        issue
            .get("word")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

fn word(issue: &Value) -> String {
    issue
        .get("word")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn suggestions(issue: &Value) -> Vec<String> {
    issue
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// `label` with "&" before its access key `key` (the last occurrence for the
/// reference's "Change A&ll"; none when the letter is missing in this language).
pub(super) fn access(label: &str, key: char) -> String {
    let lower = label.to_lowercase();
    let at = if key == 'l' {
        lower.rfind(key)
    } else {
        lower.find(key)
    };
    match at.filter(|i| label.is_char_boundary(*i)) {
        Some(i) => format!("{}&{}", &label[..i], &label[i..]),
        None => label.to_owned(),
    }
}

impl SlideWindow {
    /// Alt + the underlined letter presses a Spelling pane button.
    pub(super) fn spelling_key(&mut self, input: &crate::Input) -> Option<bool> {
        use crate::{Input, Key};
        if self.side.kind != SideKind::Spelling {
            return None;
        }
        let Input::Key(Key::Letter(c), m) = input else {
            return None;
        };
        if !m.alt || m.ctrl {
            return None;
        }
        let hit = match c.to_ascii_lowercase() {
            'i' => SideHit::IgnoreOnce,
            'g' => SideHit::IgnoreAll,
            'a' => SideHit::AddWord,
            'c' => SideHit::Change,
            'l' => SideHit::ChangeAll,
            _ => return None,
        };
        Some(self.spelling_hit(&hit))
    }

    /// Review > Spelling / F7: open the Spelling pane at the first issue from the current
    /// slide on.
    pub(super) fn start_spelling(&mut self) -> bool {
        if self.side.kind != SideKind::Spelling {
            self.open_side(SideKind::Spelling);
        }
        self.spelling = SpellRun::default();
        self.next_spelling();
        self.relayout_pending = true;
        true
    }

    /// Every misspelt word of the deck in checking order (the reference's Spelling pane
    /// walks spelling only, no grammar or style issues).
    fn deck_issues(&mut self) -> Vec<Value> {
        let mut all = self
            .view
            .spell_command(&json!({ "cmd": "slideSpellCheck" }))
            .and_then(|v| v.get("issues").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        all.retain(|i| !i.get("grammar").and_then(Value::as_bool).unwrap_or(false));
        all
    }

    /// Go to the next issue not skipped, from the current slide on (wrapping round).
    fn next_spelling(&mut self) {
        let cur = self.view.current() as u64;
        let issues: Vec<Value> = self
            .deck_issues()
            .into_iter()
            .filter(|i| !self.spelling.skipped.contains(&key(i)))
            .collect();
        let slide = |i: &Value| i.get("slide").and_then(Value::as_u64).unwrap_or(0);
        let next = issues
            .iter()
            .find(|i| slide(i) >= cur)
            .or_else(|| issues.first())
            .cloned();
        self.spelling.chosen = 0;
        self.spelling.issue = next;
        let Some(issue) = &self.spelling.issue else {
            return;
        };
        let s = slide(issue) as usize;
        if s != self.view.current() {
            self.view.go_to(s);
        }
        // Select the flagged text in its shape (other text units: the slide is shown).
        let target = issue.get("target");
        if target.and_then(|t| t.get("kind")).and_then(Value::as_str) == Some("shape") {
            if let Some(id) = target
                .and_then(|t| t.get("id"))
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                let at = |k: &str| issue.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
                self.view.run(SlideCommand::Select { ids: vec![id] });
                self.view.run(SlideCommand::TextSelect {
                    id: Some(id),
                    start: at("bodyFrom"),
                    end: at("bodyTo"),
                    cell: None,
                });
            }
        }
    }

    /// What the pane shows now.
    pub(super) fn spell_shown(&self) -> SpellShown {
        match &self.spelling.issue {
            None => SpellShown::default(),
            Some(i) => SpellShown {
                word: Some(word(i)),
                grammar: i.get("grammar").and_then(Value::as_bool).unwrap_or(false),
                suggestions: suggestions(i),
                chosen: self.spelling.chosen,
            },
        }
    }

    /// A press on one of the Spelling pane's buttons or corrections.
    pub(super) fn spelling_hit(&mut self, hit: &SideHit) -> bool {
        let Some(issue) = self.spelling.issue.clone() else {
            return false;
        };
        match hit {
            SideHit::Suggestion(i) => {
                self.spelling.chosen = *i;
                return true;
            }
            SideHit::IgnoreOnce => self.spelling.skipped.push(key(&issue)),
            SideHit::IgnoreAll => {
                self.view.spell_learn(&word(&issue), false);
            }
            SideHit::AddWord => {
                self.view.spell_learn(&word(&issue), true);
            }
            SideHit::Change | SideHit::ChangeAll => {
                let Some(with) = suggestions(&issue).get(self.spelling.chosen).cloned() else {
                    return false;
                };
                if *hit == SideHit::Change {
                    self.view.spell_fix(&issue, &with);
                } else {
                    let w = word(&issue);
                    let all: Vec<Value> = self
                        .deck_issues()
                        .into_iter()
                        .filter(|i| word(i) == w)
                        .collect();
                    let edits: Vec<(&Value, &str)> =
                        all.iter().map(|i| (i, with.as_str())).collect();
                    self.view.spell_fix_many(&edits);
                }
            }
            _ => return false,
        }
        self.next_spelling();
        true
    }
}

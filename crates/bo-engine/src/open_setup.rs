//! Set-up of a freshly opened BlueDoc document in one engine call (CORE-MISC-003).
//!
//! When a document opens the host used to send five commands one after another —
//! `setAuthor`, `setIdentity`, `autoCorrect`, `state` and `fontList` — each a round trip
//! to the engine worker (and each answered with a full editing state). `openSetup` runs
//! the same steps in the same order and answers once: the editing state, with the
//! protection state of `setIdentity`, the AutoCorrect options and, when asked, the font
//! list. The separate commands keep working.

use crate::edit::{Command, EditResult};
use crate::security::SecurityCommand;
use crate::Result;
use serde::Deserialize;

/// The `openSetup` command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSetup {
    /// Author of new revisions and comments (`setAuthor`; empty = the default author).
    #[serde(default)]
    pub author: String,
    /// Names of the user for editable regions (`setIdentity`).
    #[serde(default)]
    pub names: Vec<String>,
    /// AutoCorrect options (`autoCorrect`'s `options`); absent with `reset` = defaults.
    #[serde(default)]
    pub auto_correct: Option<Box<bo_edit::AutoCorrectOptions>>,
    /// Restore the default AutoCorrect options.
    #[serde(default)]
    pub reset_auto_correct: bool,
    /// Also report the font list (`fontList`).
    #[serde(default)]
    pub fonts: bool,
}

impl OpenSetup {
    /// Parses a JSON command when it is `openSetup`.
    pub fn parse(v: &serde_json::Value) -> Option<OpenSetup> {
        if v.get("cmd")?.as_str()? != "openSetup" {
            return None;
        }
        let mut o = v.as_object()?.clone();
        o.remove("cmd");
        serde_json::from_value(serde_json::Value::Object(o)).ok()
    }
}

impl crate::Engine {
    /// Run the document-open set-up (see the module docs); `now` is the host clock.
    pub fn doc_open_setup(&mut self, s: OpenSetup, now: Option<i64>) -> Result<EditResult> {
        let mut error = None;
        let mut dirty = Vec::new();
        // First error of the steps and every page they changed.
        let mut keep = |r: &EditResult| {
            if error.is_none() {
                error.clone_from(&r.error);
            }
            dirty.extend_from_slice(&r.dirty);
        };
        keep(&self.command_at(Command::SetAuthor { name: s.author }, now)?);
        let sec = self.doc_security(SecurityCommand::SetIdentity { names: s.names })?;
        keep(&sec);
        let ac = self.command_at(
            Command::AutoCorrect {
                options: s.auto_correct,
                reset: s.reset_auto_correct,
            },
            now,
        )?;
        keep(&ac);
        let fonts = if s.fonts {
            let f = self.command_at(Command::FontList, now)?;
            keep(&f);
            f.fonts
        } else {
            None
        };
        let mut r = self.command_at(Command::State, now)?;
        // The protection state as `setIdentity` reported it (the host asks how to open
        // a final or read-only file from it).
        if sec.security.is_some() {
            r.security = sec.security;
        }
        r.autocorrect = ac.autocorrect;
        r.fonts = fonts;
        if r.error.is_none() {
            r.error = error;
        }
        dirty.append(&mut r.dirty);
        dirty.sort_unstable();
        dirty.dedup();
        r.dirty = dirty;
        Ok(r)
    }
}

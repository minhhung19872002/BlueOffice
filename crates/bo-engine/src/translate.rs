//! Whole-document translation in BlueDoc (`docs/parity/BACKLOG.md` CORE-LANG-001). Translating
//! belongs to the platform (`TranslationProvider`); the engine answers two commands:
//!
//! * `translationSegments` — the text segments of every body paragraph (see
//!   [`bo_edit::translate`]); a query that never changes the document or the selection.
//! * `translationApply` — writes translated segments back, sharing each segment's new text
//!   among its runs by proportion so run formatting (§17.3.2.28 rPr) and paragraph properties
//!   (§17.3.1.26 pPr) are kept; one undo step.

use crate::edit::EditResult;
use crate::{EngineError, Result};
use bo_edit::translate::TextSegment;
use serde::Deserialize;

/// A translation command (`cmd` starts with `translation`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TranslateCommand {
    /// Text segments of the body paragraphs → `translation`.
    TranslationSegments,
    /// Replace the text of these segments → `count` (segments changed).
    TranslationApply {
        /// Paragraph id, segment index and new text of each segment.
        segments: Vec<TextSegment>,
    },
}

impl TranslateCommand {
    /// Parses a JSON command when it is a translation command.
    pub fn parse(v: &serde_json::Value) -> Option<TranslateCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("translation") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

impl crate::Engine {
    /// Run a translation command.
    pub fn doc_translate_command(&mut self, cmd: TranslateCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        match cmd {
            TranslateCommand::TranslationSegments => {
                let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
                let segments = bo_edit::translate::text_segments(s.doc());
                let mut r = self.query_result()?;
                r.translation = Some(segments);
                Ok(r)
            }
            TranslateCommand::TranslationApply { segments } => {
                let read_only = self.sec.read_only;
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let res = if read_only {
                    Err(crate::security::READ_ONLY.to_owned())
                } else if s.forms_locked() {
                    Err("protected".to_owned())
                } else {
                    s.apply_translation(&segments).map_err(|e| e.to_string())
                };
                let count = res.as_ref().ok().copied();
                let dirty = self.relayout()?;
                let mut r = self.edit_result(dirty, (None, count, None), res.err())?;
                r.collab = self.collab_after_edit();
                Ok(r)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::{json, Value};

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    fn cmd(e: &mut Engine, json: &str) -> Value {
        serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
    }

    #[test]
    fn segments_then_apply_keeps_paragraphs() {
        let mut e = engine("Xin chào\nHôm nay trời đẹp\n");
        let r = cmd(&mut e, r#"{"cmd":"translationSegments"}"#);
        let segs = r["translation"].as_array().unwrap().clone();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], json!({"p": 0, "s": 0, "text": "Xin chào"}));
        assert_eq!(segs[1]["text"], "Hôm nay trời đẹp");
        let apply = json!({
            "cmd": "translationApply",
            "segments": [
                {"p": 0, "s": 0, "text": "Hello"},
                {"p": 1, "s": 0, "text": "The weather is nice today"}
            ]
        });
        let r = cmd(&mut e, &apply.to_string());
        assert_eq!(r["count"], 2);
        assert!(r.get("error").is_none_or(Value::is_null), "{r}");
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        assert_eq!(r["speech"]["paras"].as_array().unwrap().len(), 2);
        assert_eq!(r["speech"]["paras"][0]["text"], "Hello");
        assert_eq!(r["speech"]["paras"][1]["text"], "The weather is nice today");
        // One undo step.
        cmd(&mut e, r#"{"cmd":"undo"}"#);
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        assert_eq!(r["speech"]["paras"][0]["text"], "Xin chào");
    }

    #[test]
    fn unknown_translation_command_is_refused() {
        let mut e = engine("a");
        assert!(e.command_json(r#"{"cmd":"translationNope"}"#).is_err());
    }
}

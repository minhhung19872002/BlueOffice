//! Read aloud in BlueDoc (`docs/parity/BACKLOG.md` CORE-LANG-003): the text the host speaks and
//! the rectangles of the word and paragraph being spoken. Speaking itself belongs to the platform
//! (W3C Web Speech API `speechSynthesis`); the engine only answers two queries that never change
//! the document or the selection:
//!
//! * `speechText` — body paragraphs in reading order with their index text (the offsets of
//!   `bo_edit::Pos`), the resolved language of their text (§17.3.2.20 lang `@w:val`, through
//!   styles and docDefaults), the language the text reads as, and the byte ranges not to speak
//!   (hidden text §17.3.2.14 vanish, deleted text §17.13.5.14 del).
//! * `speechRects` — highlight rectangles of byte ranges of body paragraphs (word and paragraph).

use crate::edit::EditResult;
use crate::nav::{self, SelRect};
use crate::{EngineError, Result};
use bo_edit::{Pos, Selection};
use serde::{Deserialize, Serialize};

/// A read-aloud query (`cmd` starts with `speech`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SpeechCommand {
    /// Body paragraphs `from..=to` (absent: the whole document).
    SpeechText {
        /// First paragraph id.
        #[serde(default)]
        from: Option<usize>,
        /// Last paragraph id.
        #[serde(default)]
        to: Option<usize>,
    },
    /// Highlight rectangles of each range.
    SpeechRects {
        /// Byte ranges of body paragraphs.
        ranges: Vec<SpeechRange>,
    },
}

/// A byte range of a body paragraph's index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SpeechRange {
    /// Body paragraph id.
    pub p: usize,
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
}

/// One body paragraph to speak.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeechPara {
    /// Body paragraph id.
    pub p: usize,
    /// Index text (offsets of `bo_edit::Pos`; objects are U+FFFC).
    pub text: String,
    /// Resolved language of the first spoken run (`None`: not set or `x-none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Language the text reads as (`bo_spell::lang::detect`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected: Option<&'static str>,
    /// Byte ranges `[start, end)` not to speak (hidden or deleted text).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<[usize; 2]>,
}

/// Answer of a read-aloud query.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct SpeechInfo {
    /// Paragraphs (`speechText`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paras: Option<Vec<SpeechPara>>,
    /// Rectangles of each range, in order (`speechRects`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rects: Option<Vec<Vec<SelRect>>>,
}

impl SpeechCommand {
    /// Parses a JSON command when it is a read-aloud query.
    pub fn parse(v: &serde_json::Value) -> Option<SpeechCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("speech") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The paragraph's spoken language and unspoken ranges.
fn para_speech(
    doc: &bo_model::Document,
    p: &bo_model::Paragraph,
) -> (Option<String>, Vec<[usize; 2]>) {
    let (_, items) = bo_model::index::para_index(p);
    let mut lang = None;
    let mut skip: Vec<[usize; 2]> = Vec::new();
    for it in items {
        if it.end == it.start {
            continue;
        }
        let direct = &p.runs[it.run].props;
        let eff = bo_edit::effective_run(doc, p, direct);
        // §17.3.2.14 vanish (hidden) and §17.13.5.14 del (deleted run) are not read.
        if eff.hidden.unwrap_or(false) || direct.del.is_some() {
            match skip.last_mut() {
                Some(last) if last[1] == it.start => last[1] = it.end,
                _ => skip.push([it.start, it.end]),
            }
            continue;
        }
        if lang.is_none() {
            lang = eff
                .lang
                .and_then(|l| l.val)
                .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("x-none"));
        }
    }
    (lang, skip)
}

impl crate::Engine {
    /// Answer a read-aloud query (never changes the document or the selection).
    pub fn doc_speech_command(&mut self, cmd: SpeechCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let mut info = SpeechInfo::default();
        match cmd {
            SpeechCommand::SpeechText { from, to } => {
                let doc = s.doc();
                let paths = bo_model::index::body_paragraphs(doc);
                let n = paths.len();
                let from = from.unwrap_or(0).min(n);
                let to = to.map_or(n, |t| t.saturating_add(1).min(n));
                let mut paras = Vec::new();
                for (id, path) in paths.iter().enumerate().take(to).skip(from) {
                    let Some(p) = bo_model::index::para_at(doc, path) else {
                        continue;
                    };
                    let text = bo_model::index::para_index_text(p);
                    let (lang, skip) = para_speech(doc, p);
                    paras.push(SpeechPara {
                        p: id,
                        detected: bo_spell::lang::detect(&text),
                        text,
                        lang,
                        skip,
                    });
                }
                info.paras = Some(paras);
            }
            SpeechCommand::SpeechRects { ranges } => {
                let layout = self.layout.document();
                let rects = ranges
                    .iter()
                    .map(|r| {
                        let a = s.clamp(Pos::new(r.p, r.start));
                        let b = s.clamp(Pos::new(r.p, r.end.max(r.start)));
                        nav::selection_rects(
                            layout,
                            Selection {
                                anchor: a,
                                focus: b,
                            },
                        )
                    })
                    .collect();
                info.rects = Some(rects);
            }
        }
        let mut r = self.query_result()?;
        r.speech = Some(Box::new(info));
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::Value;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    fn cmd(e: &mut Engine, json: &str) -> Value {
        serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
    }

    #[test]
    fn text_lists_paragraphs_with_language() {
        let mut e = engine("Xin chào các bạn, hôm nay trời đẹp\nThe quick brown fox jumps");
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0]["p"], 0);
        assert_eq!(paras[0]["text"], "Xin chào các bạn, hôm nay trời đẹp");
        assert_eq!(paras[0]["detected"], "vi-VN");
        assert_eq!(paras[1]["p"], 1);
        assert!(paras[1].get("skip").is_none());
        // Range of paragraphs.
        let r = cmd(&mut e, r#"{"cmd":"speechText","from":1,"to":1}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0]["p"], 1);
    }

    #[test]
    fn rects_of_a_word_leave_the_selection_alone() {
        let mut e = engine("một hai ba\nfour five");
        let before = cmd(&mut e, r#"{"cmd":"state"}"#)["selection"].clone();
        // "một" is 5 bytes (ộ is 3 bytes in UTF-8).
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":0,"end":5},{"p":1,"start":0,"end":9},{"p":1,"start":2,"end":2}]}"#,
        );
        let rects = r["speech"]["rects"].as_array().unwrap();
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].as_array().unwrap().len(), 1);
        assert!(rects[0][0]["w"].as_f64().unwrap() > 0.0);
        assert_eq!(rects[1].as_array().unwrap().len(), 1);
        // A collapsed range has no rectangle.
        assert_eq!(rects[2], serde_json::json!([]));
        // The whole paragraph is wider than its first word.
        let y0 = rects[0][0]["y"].as_f64().unwrap();
        let y1 = rects[1][0]["y"].as_f64().unwrap();
        assert!(y1 > y0);
        assert_eq!(r["selection"], before);
        // Offsets inside a multi-byte character are clamped, not an error.
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":1,"end":4}]}"#,
        );
        assert_eq!(r["speech"]["rects"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hidden_and_deleted_text_is_skipped_and_language_resolved() {
        use bo_model::{Block, Lang, Paragraph, RevMark, Run, RunProps, Section};
        let lang = |v: &str| {
            Some(Lang {
                val: Some(v.into()),
                ..Default::default()
            })
        };
        // "Một " hidden (§17.3.2.14), "hai " deleted (§17.13.5.14), "ba" tagged vi-VN.
        let runs = vec![
            Run::text(
                "Một ",
                RunProps {
                    hidden: Some(true),
                    lang: lang("en-US"),
                    ..Default::default()
                },
            ),
            Run::text(
                "hai ",
                RunProps {
                    del: Some(RevMark::default()),
                    ..Default::default()
                },
            ),
            Run::text(
                "ba",
                RunProps {
                    lang: lang("vi-VN"),
                    ..Default::default()
                },
            ),
        ];
        let mut doc = crate::plain_text_document("x");
        doc.sections = vec![Section::new(vec![
            Block::Paragraph(Paragraph::new(bo_model::ParaProps::default(), runs)),
            Block::Paragraph(Paragraph::new(
                bo_model::ParaProps::default(),
                vec![Run::text(
                    "plain",
                    RunProps {
                        lang: lang("x-none"),
                        ..Default::default()
                    },
                )],
            )),
        ])];
        let mut e = Engine::with_bundled_fonts();
        e.load(doc, "t.docx").unwrap();
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras[0]["text"], "Một hai ba");
        // "Một " is 6 bytes, "hai " 4: one merged byte range [0, 10).
        assert_eq!(paras[0]["skip"], serde_json::json!([[0, 10]]));
        // The language of the first spoken run, not of the hidden one.
        assert_eq!(paras[0]["lang"], "vi-VN");
        assert!(paras[1].get("lang").is_none());
        // The skipped range maps to the highlight of the spoken word "ba".
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":10,"end":12}]}"#,
        );
        assert_eq!(r["speech"]["rects"][0].as_array().unwrap().len(), 1);
    }

    #[test]
    fn other_commands_are_not_speech() {
        let v: Value = serde_json::from_str(r#"{"cmd":"state"}"#).unwrap();
        assert!(super::SpeechCommand::parse(&v).is_none());
    }
}

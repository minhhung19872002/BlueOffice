//! Word count of a BlueDoc document (status bar and the Word Count dialog, CORE-UI-007).
//!
//! `wordCount` is a query: it never changes the document or the selection. It counts the
//! body (tables included) and, with `notes`, also the text boxes (§17.17.1 txbxContent),
//! footnotes and endnotes (§17.11); when text is selected the same figures are given for
//! the selection. Words are whitespace-separated, as in the `DocInfo` word count.

use crate::edit::EditResult;
use crate::{EngineError, Result};
use bo_model::{Block, Paragraph, RunContent};
use serde::{Deserialize, Serialize};

/// The word count query.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum WordCountCommand {
    /// Count the document and the selection → `wordCount`.
    WordCount {
        /// Include text boxes, footnotes and endnotes.
        #[serde(default)]
        notes: bool,
    },
}

impl WordCountCommand {
    /// Parses a JSON command when it is the word count query.
    pub fn parse(v: &serde_json::Value) -> Option<WordCountCommand> {
        if v.get("cmd")?.as_str()? != "wordCount" {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// Figures of a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Whitespace-separated words.
    pub words: usize,
    /// Characters without spaces.
    pub chars: usize,
    /// Characters with spaces (paragraph marks are not counted).
    pub chars_with_spaces: usize,
    /// Paragraphs with text.
    pub paragraphs: usize,
}

impl Counts {
    fn add_paragraph(&mut self, text: &str) {
        let mut any = false;
        for c in text.chars() {
            // Objects (U+FFFC) and line / page breaks are not characters of the count.
            if c == '\u{FFFC}' || c == '\n' || c == '\r' || c == '\u{c}' {
                continue;
            }
            self.chars_with_spaces += 1;
            if !c.is_whitespace() {
                self.chars += 1;
                any = true;
            }
        }
        self.words += text
            .split_whitespace()
            .filter(|w| w.chars().any(|c| c != '\u{FFFC}'))
            .count();
        self.paragraphs += usize::from(any);
    }
}

/// Answer of `wordCount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WordCountInfo {
    /// Pages of the layout.
    pub pages: usize,
    /// The document (with text boxes and notes when asked).
    pub document: Counts,
    /// The selection; `None` when nothing is selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<Counts>,
    /// Text boxes, footnotes and endnotes were counted.
    pub notes: bool,
}

fn text_boxes<'a>(p: &'a Paragraph, out: &mut Vec<&'a [Block]>) {
    for r in &p.runs {
        for c in &r.content {
            if let RunContent::Image(img) = c {
                if let Some(blocks) = img.shape.as_ref().and_then(|s| s.text.as_ref()) {
                    out.push(blocks);
                }
            }
        }
    }
}

/// Counts `blocks`; with `boxes`, the text boxes anchored in them as well.
fn count_blocks(blocks: &[Block], boxes: bool, c: &mut Counts) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                c.add_paragraph(&p.plain_text());
                if boxes {
                    let mut inner = Vec::new();
                    text_boxes(p, &mut inner);
                    for blocks in inner {
                        count_blocks(blocks, boxes, c);
                    }
                }
            }
            Block::Table(t) => {
                for r in &t.rows {
                    for cell in &r.cells {
                        count_blocks(&cell.blocks, boxes, c);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

impl crate::Engine {
    /// Answer the word count query.
    pub fn doc_word_count(&mut self, cmd: &WordCountCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let WordCountCommand::WordCount { notes } = *cmd;
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let doc = s.doc();
        let mut document = Counts::default();
        for sec in &doc.sections {
            count_blocks(&sec.blocks, notes, &mut document);
        }
        if notes {
            for blocks in doc.footnotes.values().chain(doc.endnotes.values()) {
                count_blocks(blocks, true, &mut document);
            }
        }
        let selection = (!s.selection().is_collapsed()).then(|| {
            let mut c = Counts::default();
            count_blocks(&s.selected_blocks(), notes, &mut c);
            c
        });
        let mut r = self.query_result()?;
        r.word_count = Some(WordCountInfo {
            pages: r.info.page_count,
            document,
            selection,
            notes,
        });
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
    fn counts_document_and_selection() {
        let mut e = engine("Xin chào bạn\n\nHôm nay trời đẹp\n");
        let r = cmd(&mut e, r#"{"cmd":"wordCount"}"#);
        let d = &r["wordCount"]["document"];
        assert_eq!(d["words"], 7);
        assert_eq!(d["chars"], 10 + 13);
        assert_eq!(d["charsWithSpaces"], 12 + 16);
        assert_eq!(d["paragraphs"], 2);
        assert_eq!(r["wordCount"]["pages"], 1);
        assert!(r["wordCount"].get("selection").is_none());
        // "Xin chào" selected (byte offsets of the index text).
        let sel = r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":9}}}"#;
        cmd(&mut e, sel);
        let r = cmd(&mut e, r#"{"cmd":"wordCount","notes":true}"#);
        let s = &r["wordCount"]["selection"];
        assert_eq!(s["words"], 2);
        assert_eq!(s["chars"], 7);
        assert_eq!(s["charsWithSpaces"], 8);
        assert_eq!(r["wordCount"]["notes"], true);
        // The query leaves the selection alone.
        assert_eq!(r["selection"]["focus"]["o"], 9);
    }

    #[test]
    fn notes_are_counted_only_when_asked() {
        let mut e = engine("Một hai ba\n");
        cmd(
            &mut e,
            r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":3},"focus":{"p":0,"o":3}}}"#,
        );
        let r = cmd(
            &mut e,
            r#"{"cmd":"insertNote","endnote":false,"text":"chú thích dài"}"#,
        );
        assert!(r.get("error").is_none_or(Value::is_null), "{r}");
        let body = cmd(&mut e, r#"{"cmd":"wordCount"}"#);
        assert_eq!(body["wordCount"]["document"]["words"], 3);
        let all = cmd(&mut e, r#"{"cmd":"wordCount","notes":true}"#);
        assert_eq!(all["wordCount"]["document"]["words"], 6);
    }
}

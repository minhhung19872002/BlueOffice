//! Insert › Links › Bookmark for BlueDoc (round 17): add a bookmark around the selection,
//! delete one, select one (Go To), list them (ECMA-376 Part 1, §17.13.6.2 bookmarkStart,
//! §17.13.6.1 bookmarkEnd; see [`bo_edit::bookmarks17`]).

use crate::edit::EditResult;
use crate::{Engine, EngineError, Result};
use bo_edit::bookmarks17::BookmarkEntry;
use serde::Deserialize;

/// A bookmark command of the document (`cmd` starts with `bookmark`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BookmarkCommand {
    /// Add bookmark `name` around the selection (moves one of that name).
    BookmarkAdd {
        /// Bookmark name.
        name: String,
    },
    /// Delete bookmark `name`.
    BookmarkDelete {
        /// Bookmark name.
        name: String,
    },
    /// Select the text of bookmark `name`.
    BookmarkGoTo {
        /// Bookmark name.
        name: String,
    },
}

impl BookmarkCommand {
    /// Parses a JSON command when it is a bookmark command.
    pub fn parse(v: &serde_json::Value) -> Option<BookmarkCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("bookmark") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

impl Engine {
    /// Bookmarks of the main text in document order.
    pub fn doc_bookmarks(&self) -> Vec<BookmarkEntry> {
        self.session
            .as_ref()
            .map(bo_edit::Session::bookmark_list)
            .unwrap_or_default()
    }

    /// Run a bookmark command; the edit result carries the new selection and pages.
    pub fn doc_bookmark_command(&mut self, cmd: BookmarkCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let error = match cmd {
            BookmarkCommand::BookmarkAdd { .. } | BookmarkCommand::BookmarkDelete { .. }
                if read_only =>
            {
                Some(crate::security::READ_ONLY.to_owned())
            }
            BookmarkCommand::BookmarkAdd { name } => {
                s.add_bookmark(&name).err().map(|e| e.to_string())
            }
            BookmarkCommand::BookmarkDelete { name } => match s.delete_bookmark(&name) {
                Ok(true) => None,
                Ok(false) => Some("noBookmark".into()),
                Err(e) => Some(e.to_string()),
            },
            BookmarkCommand::BookmarkGoTo { name } => {
                (!s.go_to_bookmark(&name)).then(|| "noBookmark".to_owned())
            }
        };
        let dirty = self.relayout()?;
        self.story_result(dirty, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_commands_round_trip_through_docx() {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("b.docx").expect("new");
        e.command_json(r#"{"cmd":"insertText","text":"Alpha beta gamma"}"#)
            .expect("text");
        e.command_json(
            r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":6},"focus":{"p":0,"o":10}}}"#,
        )
        .expect("select");
        let add = BookmarkCommand::parse(&serde_json::json!({"cmd":"bookmarkAdd","name":"Beta"}))
            .expect("parse");
        let r = e.doc_bookmark_command(add).expect("add");
        assert!(r.error.is_none(), "{:?}", r.error);
        let bad = e
            .doc_bookmark_command(BookmarkCommand::BookmarkAdd { name: "9x".into() })
            .expect("bad name");
        assert!(bad.error.is_some());
        let bytes = e.save_docx().expect("save");
        let mut back = Engine::with_bundled_fonts();
        back.open(&bytes, "b.docx").expect("open");
        let list = back.doc_bookmarks();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Beta");
        assert_eq!((list[0].start.offset, list[0].end.offset), (6, 10));
        let r = back
            .doc_bookmark_command(BookmarkCommand::BookmarkGoTo {
                name: "Beta".into(),
            })
            .expect("go to");
        assert_eq!(r.selection.focus.offset, 10);
        back.doc_bookmark_command(BookmarkCommand::BookmarkDelete {
            name: "Beta".into(),
        })
        .expect("delete");
        assert_eq!(back.doc_bookmarks(), Vec::new());
    }
}

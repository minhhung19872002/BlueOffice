//! Paste as a hyperlink: the clipboard text (a URL or an e-mail address) becomes
//! a hyperlink run with the destination's formatting plus the link look
//! (ECMA-376 Part 1, §17.16.22 hyperlink; §17.3.2.6 color, §17.3.2.40 u).

use crate::autocorrect::{link_props, link_target};
use crate::para;
use crate::{EditError, Result, Selection, Session};
use bo_model::{Block, ParaProps, Paragraph};

/// Hyperlink target of clipboard text that is a single URL / e-mail address.
#[must_use]
pub fn clipboard_link(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.chars().any(char::is_whitespace) {
        return None;
    }
    link_target(t)
}

/// Blocks of an internal clip (a block list, or blocks with the notes they reference).
pub(crate) fn clip_blocks(json: &str) -> Result<Vec<Block>> {
    match serde_json::from_str::<Vec<Block>>(json) {
        Ok(b) => Ok(b),
        Err(_) => serde_json::from_str::<crate::ClipBlocks>(json)
            .map(|c| c.blocks)
            .map_err(|e| EditError::Clipboard(e.to_string())),
    }
}

impl Session {
    /// Paste `text` as a hyperlink to itself (one undo step); refused when the text
    /// is not a URL or an e-mail address.
    pub fn paste_link(&mut self, text: &str) -> Result<Selection> {
        let target = clipboard_link(text)
            .ok_or_else(|| EditError::Clipboard("the clipboard holds no link".into()))?;
        let mut props = self.caret_direct_props();
        link_props(&mut props, &target);
        let p = Paragraph::new(
            ParaProps::default(),
            para::rebuild(para::text_atoms(text.trim(), &props)),
        );
        self.paste_blocks_inner(vec![Block::Paragraph(p)], true)
    }
}

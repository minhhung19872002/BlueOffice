//! Footnotes and endnotes edited in place on the page (ECMA-376 Part 1, §17.11).
//!
//! A click in the footnote area at the bottom of a page (or on an endnote) opens
//! that note in a *story session*: a [`bo_edit::Session`] over a one-section
//! document holding the note's blocks. The note's lines in the main layout carry
//! [`bo_layout::LineSource::Note`] with the paragraph index inside the note, so
//! caret, selection and hit testing work directly on the page (no separate
//! layout). Every change is mirrored into the main document without history (live
//! display); closing the note records one undoable change in the main session.

use crate::edit::{Command, EditResult};
use crate::nav::{self, Story};
use crate::story::{sub_command, StoryState};
use crate::{Engine, EngineError, Result};
use bo_edit::section::HfVariant;
use bo_edit::{Pos, Selection, Session};
use bo_model::{Block, Document, Paragraph, Section};

/// The open note editor.
pub(crate) struct NoteEdit {
    pub endnote: bool,
    pub id: i32,
    /// Page the note was clicked on.
    pub page: usize,
    pub sub: Session,
    /// The note's content before editing.
    original: Vec<Block>,
    version: u64,
}

impl NoteEdit {
    fn story(&self) -> Story {
        Story::Note {
            endnote: self.endnote,
            id: self.id,
        }
    }
}

fn note_doc(main: &Document, blocks: Vec<Block>) -> Document {
    Document {
        sections: vec![Section::new(blocks)],
        default_run: main.default_run.clone(),
        default_para: main.default_para.clone(),
        styles: main.styles.clone(),
        numbering: main.numbering.clone(),
        media: main.media.clone(),
        default_tab_stop: main.default_tab_stop,
        ..Document::default()
    }
}

fn sub_blocks(s: &Session) -> Vec<Block> {
    s.doc()
        .sections
        .first()
        .map(|s| s.blocks.clone())
        .unwrap_or_default()
}

impl Engine {
    /// Note shown at (`x`, `y`) of page `pi`: the note of the nearest note line when
    /// the point is within the page's footnote/endnote lines.
    pub(crate) fn note_at(&self, pi: usize, y: f32) -> Option<(bool, i32)> {
        let page = self.layout()?.pages.get(pi)?;
        let mut best: Option<((bool, i32), f32)> = None;
        let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
        for l in &page.lines {
            let Some(k) = nav::note_of(l) else { continue };
            top = top.min(l.top);
            bottom = bottom.max(l.top + l.height);
            let d = if y < l.top {
                l.top - y
            } else if y > l.top + l.height {
                y - l.top - l.height
            } else {
                0.0
            };
            if best.is_none_or(|b| d < b.1) {
                best = Some((k, d));
            }
        }
        // A little slack around the note area (paragraph spacing).
        (y >= top - 2.0 && y <= bottom + 2.0).then_some(best?.0)
    }

    /// First page showing a note.
    fn note_page(&self, endnote: bool, id: i32) -> Option<usize> {
        self.layout()?.pages.iter().position(|p| {
            p.lines
                .iter()
                .any(|l| nav::note_of(l) == Some((endnote, id)))
        })
    }

    /// Open a footnote (`endnote` false) or endnote for editing on page `page`.
    pub fn open_note(&mut self, endnote: bool, id: i32, page: usize) -> Result<()> {
        self.close_header_footer()?;
        self.close_note()?;
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let mut blocks = s
            .note_blocks(endnote, id)
            .cloned()
            .ok_or_else(|| EngineError::BadCommand(format!("no note {id}")))?;
        let original = blocks.clone();
        if !blocks.iter().any(|b| matches!(b, Block::Paragraph(_))) {
            blocks.push(Block::Paragraph(Paragraph::default()));
        }
        let mut sub = Session::new(note_doc(s.doc(), blocks));
        sub.set_author(s.author());
        let version = sub.version();
        self.note = Some(NoteEdit {
            endnote,
            id,
            page,
            sub,
            original,
            version,
        });
        Ok(())
    }

    /// Close the note editor, recording its change (one undo step).
    pub fn close_note(&mut self) -> Result<bool> {
        let Some(ne) = self.note.take() else {
            return Ok(false);
        };
        let blocks = sub_blocks(&ne.sub);
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        s.preview_note(ne.endnote, ne.id, ne.original.clone());
        if blocks != ne.original {
            s.set_note_blocks(ne.endnote, ne.id, blocks)
                .map_err(|e| EngineError::BadCommand(e.to_string()))?;
        }
        self.relayout()?;
        Ok(true)
    }

    /// Open the note just inserted at the caret with the caret at the end of its text.
    pub(crate) fn open_new_note(
        &mut self,
        endnote: bool,
        id: i32,
        dirty: Vec<usize>,
    ) -> Result<Option<EditResult>> {
        let Some(page) = self.note_page(endnote, id) else {
            return Ok(None);
        };
        self.open_note(endnote, id, page)?;
        if let Some(ne) = self.note.as_mut() {
            let last = ne.sub.paragraph_count().saturating_sub(1);
            let end = ne.sub.para_text(last).map_or(0, |t| t.len());
            ne.sub.set_selection(Selection::caret(Pos::new(last, end)));
        }
        self.note_result(dirty, None).map(Some)
    }

    /// Run `cmd` in the note editor. Returns `None` when the command is not a story
    /// command (the editor is then closed and the command runs on the body).
    pub(crate) fn note_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        if let Command::Click { page, y, .. } = cmd {
            let current = self.note.as_ref().map(|n| (n.endnote, n.id));
            match self.note_at(*page, *y) {
                Some(k) if Some(k) == current => {}
                Some(k) => self.open_note(k.0, k.1, *page)?,
                None => {
                    self.close_note()?;
                    return Ok(None);
                }
            }
        }
        let Some(mut ne) = self.note.take() else {
            return Ok(None);
        };
        let mut error = None;
        let mut clip = None;
        let mut page = ne.page;
        let res = {
            let layout = self.layout.document();
            nav::with_story(ne.story(), || {
                sub_command(&mut ne.sub, layout, (0.0, None), cmd, &mut clip, &mut page)
            })
        };
        ne.page = page;
        let handled = match res {
            Ok(h) => h,
            Err(e) => {
                error = Some(e.to_string());
                true
            }
        };
        if !handled {
            self.note = Some(ne);
            self.close_note()?;
            return Ok(None);
        }
        let mut dirty = Vec::new();
        if ne.sub.version() != ne.version {
            ne.version = ne.sub.version();
            let blocks = sub_blocks(&ne.sub);
            if let Some(s) = self.session.as_mut() {
                s.preview_note(ne.endnote, ne.id, blocks);
            }
            dirty = self.relayout()?;
        }
        self.note = Some(ne);
        let mut r = self.note_result(dirty, error)?;
        r.clip = clip;
        Ok(Some(r))
    }

    /// Edit result while a note is open: its selection on the page.
    pub(crate) fn note_result(
        &self,
        dirty: Vec<usize>,
        error: Option<String>,
    ) -> Result<EditResult> {
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        if let Some(ne) = self.note.as_ref() {
            let layout = self.layout.document();
            let sel = ne.sub.selection();
            r.selection = sel;
            nav::with_story(ne.story(), || {
                r.caret = nav::caret_rect(layout, sel.focus);
                r.rects = nav::selection_rects(layout, sel);
            });
            let section = r.format.section;
            r.format = ne.sub.format_state();
            r.format.section = section;
            r.format.table = None;
            r.story = Some(StoryState {
                kind: if ne.endnote { "endnote" } else { "footnote" },
                section: 0,
                variant: HfVariant::Default,
                page: r.caret.map_or(ne.page, |c| c.page),
                title_page: false,
                even_and_odd: false,
                linked: false,
                can_link: false,
                note: Some(ne.id),
            });
        }
        Ok(r)
    }
}

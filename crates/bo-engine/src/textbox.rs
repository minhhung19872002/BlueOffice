//! Shapes and text boxes in BlueDoc (ECMA-376 Part 1, §20.4 drawing objects,
//! [MS-ODRAWXML] `wps:wsp`, §17.17.1 txbxContent).
//!
//! A click on a shape reports it as the selected drawing object (the host shows its
//! frame and handles and sends [`Command::SetDrawingFrame`] to move or resize it).
//! A click on a text box also opens its content in a *story session* like a
//! footnote (`crate::notes`): a [`bo_edit::Session`] over a one-section document
//! holding the text box blocks. Its lines in the main layout carry
//! [`bo_layout::LineSource::TextBox`], so caret, selection and hit testing work on the
//! page; each change is mirrored into the main document without history and closing
//! the text box records one undoable change.

use crate::edit::{Command, EditResult};
use crate::nav::{self, Story};
use crate::story::{sub_command, StoryState};
use crate::{Engine, EngineError, Result};
use bo_edit::section::HfVariant;
use bo_edit::shapes::ShapeSpec;
use bo_edit::{Pos, Selection, Session};
use bo_layout::{LineSource, PageItem};
use bo_model::{Block, Document, Paragraph, Section};
use serde::Serialize;

/// EMU per point.
const EMU_PT: f32 = 12_700.0;

/// The open text box editor.
pub(crate) struct BoxEdit {
    pub para: usize,
    pub at: usize,
    pub page: usize,
    pub sub: Session,
    original: Vec<Block>,
    version: u64,
}

impl BoxEdit {
    fn story(&self) -> Story {
        Story::TextBox {
            para: self.para as u32,
            at: self.at as u32,
        }
    }
}

/// A drawing object on a page, reported to hosts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingInfo {
    /// Body paragraph id and index-text offset of the object (command address).
    pub para: usize,
    /// Index-text offset of the object in that paragraph.
    pub at: usize,
    /// 0-based page index the object is drawn on.
    pub page: usize,
    /// Frame on the page (points).
    pub x: f32,
    /// Top edge on the page, in points.
    pub y: f32,
    /// Width in points.
    pub w: f32,
    /// Height in points.
    pub h: f32,
    /// The shape holds a text box.
    pub text_box: bool,
    /// Floating (§20.4.2.3 anchor): it can be moved; inline objects only resize.
    pub anchored: bool,
}

fn box_doc(main: &Document, blocks: Vec<Block>) -> Document {
    Document {
        sections: vec![Section::new(blocks)],
        default_run: main.default_run.clone(),
        default_para: main.default_para.clone(),
        styles: main.styles.clone(),
        numbering: main.numbering.clone(),
        media: main.media.clone(),
        default_tab_stop: main.default_tab_stop,
        theme_fonts: main.theme_fonts.clone(),
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
    /// The topmost body drawing object whose frame contains (`x`, `y`) on page `pi`.
    pub(crate) fn drawing_hit(&self, pi: usize, x: f32, y: f32) -> Option<DrawingInfo> {
        let page = self.layout()?.pages.get(pi)?;
        let s = self.session.as_ref()?;
        page.items.iter().rev().find_map(|it| {
            let PageItem::Shape {
                x: sx,
                y: sy,
                w,
                h,
                source: LineSource::TextBox { para, at, .. },
                ..
            } = it
            else {
                return None;
            };
            if *para == LineSource::NO_PARA {
                return None;
            }
            let inside = x >= sx - 2.0 && x <= sx + w + 2.0 && y >= sy - 2.0 && y <= sy + h + 2.0;
            if !inside {
                return None;
            }
            let img = s.drawing(*para as usize, *at as usize)?;
            Some(DrawingInfo {
                para: *para as usize,
                at: *at as usize,
                page: pi,
                x: *sx,
                y: *sy,
                w: *w,
                h: *h,
                text_box: img.shape.as_ref().is_some_and(|sh| sh.text.is_some()),
                anchored: img.anchor.is_some(),
            })
        })
    }

    /// Frame of the drawing object (`para`, `at`) on its page.
    fn drawing_frame(&self, para: usize, at: usize) -> Option<DrawingInfo> {
        let layout = self.layout()?;
        for (pi, page) in layout.pages.iter().enumerate() {
            for it in &page.items {
                if let PageItem::Shape {
                    x,
                    y,
                    w,
                    h,
                    source: LineSource::TextBox { para: p, at: a, .. },
                    ..
                } = it
                {
                    if *p as usize == para && *a as usize == at {
                        return self
                            .drawing_hit(pi, x + w / 2.0, y + h / 2.0)
                            .filter(|d| d.para == para && d.at == at);
                    }
                }
            }
        }
        None
    }

    /// Open the text box at (`para`, `at`) for editing on page `page`.
    pub fn open_text_box(&mut self, para: usize, at: usize, page: usize) -> Result<()> {
        self.close_header_footer()?;
        self.close_note()?;
        self.close_text_box()?;
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let original = s
            .drawing(para, at)
            .and_then(|i| i.shape.as_ref())
            .and_then(|sh| sh.text.clone())
            .ok_or_else(|| EngineError::BadCommand(format!("no text box at {para}:{at}")))?;
        let mut blocks = original.clone();
        if !blocks.iter().any(|b| matches!(b, Block::Paragraph(_))) {
            blocks.push(Block::Paragraph(Paragraph::default()));
        }
        let mut sub = Session::new(box_doc(s.doc(), blocks));
        sub.set_author(s.author());
        let version = sub.version();
        self.textbox = Some(BoxEdit {
            para,
            at,
            page,
            sub,
            original,
            version,
        });
        Ok(())
    }

    /// Close the text box editor, recording its change (one undo step).
    pub fn close_text_box(&mut self) -> Result<bool> {
        let Some(be) = self.textbox.take() else {
            return Ok(false);
        };
        let blocks = sub_blocks(&be.sub);
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        s.preview_text_box(be.para, be.at, be.original.clone());
        if blocks != be.original {
            s.set_text_box(be.para, be.at, blocks)
                .map_err(|e| EngineError::BadCommand(e.to_string()))?;
        }
        self.relayout()?;
        Ok(true)
    }

    /// Commands about drawing objects, and commands sent to an open text box.
    pub(crate) fn drawing_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        match cmd {
            Command::InsertShape {
                preset,
                text_box,
                width,
                height,
            } => {
                self.close_text_box()?;
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let spec = ShapeSpec {
                    preset: preset.clone(),
                    text_box: *text_box,
                };
                let w = (width.clamp(4.0, 2000.0) * EMU_PT) as i64;
                let h = (height.clamp(4.0, 2000.0) * EMU_PT) as i64;
                let res = s.insert_shape(&spec, w, h);
                let dirty = self.relayout()?;
                let mut error = None;
                let mut placed = None;
                match res {
                    Ok(p) => placed = Some(p),
                    Err(e) => error = Some(e.to_string()),
                }
                if let (Some((para, at)), true) = (placed, *text_box) {
                    let page = self.drawing_frame(para, at).map_or(0, |d| d.page);
                    self.open_text_box(para, at, page)?;
                    let mut r = self.box_result(dirty, error)?;
                    r.drawing = self.drawing_frame(para, at);
                    r.collab = self.collab_after_edit();
                    return Ok(Some(r));
                }
                let mut r = self.edit_result(dirty, (None, None, None), error)?;
                r.drawing = placed.and_then(|(p, a)| self.drawing_frame(p, a));
                r.collab = self.collab_after_edit();
                Ok(Some(r))
            }
            Command::SetDrawingFrame {
                para,
                at,
                dx,
                dy,
                width,
                height,
            } => {
                self.close_text_box()?;
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let size = match (width, height, s.drawing(*para, *at)) {
                    (None, None, _) | (_, _, None) => None,
                    (w, h, Some(img)) => Some((
                        w.map_or(img.width.0, |w| (w.max(1.0) * EMU_PT) as i64),
                        h.map_or(img.height.0, |h| (h.max(1.0) * EMU_PT) as i64),
                    )),
                };
                let delta = ((dx * EMU_PT) as i64, (dy * EMU_PT) as i64);
                let error = s
                    .set_drawing_frame(*para, *at, delta, size)
                    .err()
                    .map(|e| e.to_string());
                let dirty = self.relayout()?;
                let mut r = self.edit_result(dirty, (None, None, None), error)?;
                r.drawing = self.drawing_frame(*para, *at);
                r.collab = self.collab_after_edit();
                Ok(Some(r))
            }
            Command::EditTextBox { para, at } => {
                let page = self.drawing_frame(*para, *at).map_or(0, |d| d.page);
                if let Err(e) = self.open_text_box(*para, *at, page) {
                    let r =
                        self.edit_result(Vec::new(), (None, None, None), Some(e.to_string()))?;
                    return Ok(Some(r));
                }
                if let Some(be) = self.textbox.as_mut() {
                    let last = be.sub.paragraph_count().saturating_sub(1);
                    let end = be.sub.para_text(last).map_or(0, |t| t.len());
                    be.sub.set_selection(Selection::caret(Pos::new(last, end)));
                }
                let mut r = self.box_result(Vec::new(), None)?;
                r.drawing = self.drawing_frame(*para, *at);
                Ok(Some(r))
            }
            Command::CloseTextBox => {
                self.close_text_box()?;
                Ok(Some(self.edit_result(
                    Vec::new(),
                    (None, None, None),
                    None,
                )?))
            }
            Command::Click {
                page, x, y, count, ..
            } => {
                let hit = self.drawing_hit(*page, *x, *y);
                let current = self.textbox.as_ref().map(|b| (b.para, b.at));
                match &hit {
                    Some(d) if d.text_box && *count == 1 => {
                        if current != Some((d.para, d.at)) {
                            self.open_text_box(d.para, d.at, *page)?;
                        }
                        let mut r = self.text_box_command(cmd)?;
                        if let Some(r) = r.as_mut() {
                            r.drawing = hit;
                        }
                        Ok(r)
                    }
                    Some(_) if current.is_none() => {
                        // A shape without text: the body caret moves, the shape is reported.
                        Ok(None)
                    }
                    _ => {
                        self.close_text_box()?;
                        Ok(None)
                    }
                }
            }
            _ if self.textbox.is_some() => self.text_box_command(cmd),
            _ => Ok(None),
        }
    }

    /// Run `cmd` in the open text box (`None`: not a story command; the box is closed).
    fn text_box_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        let Some(mut be) = self.textbox.take() else {
            return Ok(None);
        };
        let mut error = None;
        let mut clip = None;
        let mut page = be.page;
        let res = {
            let layout = self.layout.document();
            nav::with_story(be.story(), || {
                let ac = self.autocorrect.as_ref();
                sub_command(
                    &mut be.sub,
                    layout,
                    (0.0, None),
                    cmd,
                    &mut clip,
                    &mut page,
                    ac,
                )
            })
        };
        be.page = page;
        let handled = match res {
            Ok(h) => h,
            Err(e) => {
                error = Some(e.to_string());
                true
            }
        };
        if !handled {
            self.textbox = Some(be);
            self.close_text_box()?;
            return Ok(None);
        }
        let mut dirty = Vec::new();
        if be.sub.version() != be.version {
            be.version = be.sub.version();
            let blocks = sub_blocks(&be.sub);
            if let Some(s) = self.session.as_mut() {
                s.preview_text_box(be.para, be.at, blocks);
            }
            dirty = self.relayout()?;
        }
        self.textbox = Some(be);
        let mut r = self.box_result(dirty, error)?;
        r.clip = clip;
        Ok(Some(r))
    }

    /// Edit result while a text box is open: its selection on the page.
    fn box_result(&self, dirty: Vec<usize>, error: Option<String>) -> Result<EditResult> {
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        if let Some(be) = self.textbox.as_ref() {
            let layout = self.layout.document();
            let sel = be.sub.selection();
            r.selection = sel;
            nav::with_story(be.story(), || {
                r.caret = nav::caret_rect(layout, sel.focus);
                r.rects = nav::selection_rects(layout, sel);
            });
            let section = r.format.section;
            r.format = be.sub.format_state();
            r.format.section = section;
            r.format.table = None;
            r.story = Some(StoryState {
                kind: "textBox",
                section: 0,
                variant: HfVariant::Default,
                page: r.caret.map_or(be.page, |c| c.page),
                title_page: false,
                even_and_odd: false,
                linked: false,
                can_link: false,
                note: None,
            });
            if r.drawing.is_none() {
                r.drawing = self.drawing_frame(be.para, be.at);
            }
        }
        Ok(r)
    }
}

//! Editing commands over the JSON bridge (ADR-0002, ADR-0008).
//!
//! Hosts send one [`Command`] as JSON and receive an [`EditResult`]: the new
//! selection with its geometry, ribbon state and the pages whose display list
//! changed (so the canvas only repaints those).

use crate::nav::{self, CaretRect, Motion, SelRect};
use bo_edit::{CharFormat, FindOptions, FormatState, Match, ParaFormat, Pos, Selection};
use bo_model::SectionBreak;
use serde::{Deserialize, Serialize};

/// Section break kinds accepted from hosts (§17.18.77 `ST_SectionMark`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BreakKind {
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
}

impl From<BreakKind> for SectionBreak {
    fn from(k: BreakKind) -> Self {
        match k {
            BreakKind::NextPage => SectionBreak::NextPage,
            BreakKind::Continuous => SectionBreak::Continuous,
            BreakKind::EvenPage => SectionBreak::EvenPage,
            BreakKind::OddPage => SectionBreak::OddPage,
        }
    }
}

/// An editing command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    /// Return the current state without changing anything.
    State,
    SetSelection {
        selection: Selection,
    },
    /// Mouse click: `count` 1 = caret, 2 = word, 3 = paragraph; `extend` = Shift held.
    Click {
        page: usize,
        x: f32,
        y: f32,
        #[serde(default)]
        extend: bool,
        #[serde(default = "one")]
        count: u8,
    },
    /// Mouse drag to a point (extends the selection).
    DragTo {
        page: usize,
        x: f32,
        y: f32,
    },
    Move {
        motion: Motion,
        #[serde(default)]
        extend: bool,
    },
    SelectAll,
    InsertText {
        text: String,
    },
    DeleteBackward,
    DeleteForward,
    SplitParagraph,
    FormatText {
        format: CharFormat,
    },
    FormatParagraph {
        format: ParaFormat,
    },
    ApplyStyle {
        id: String,
    },
    InsertTable {
        rows: usize,
        cols: usize,
    },
    InsertPageBreak,
    InsertSectionBreak {
        kind: BreakKind,
    },
    Undo,
    Redo,
    Find {
        query: String,
        #[serde(default)]
        options: FindOptions,
    },
    /// Select a match returned by `find`.
    SelectMatch {
        #[serde(rename = "match")]
        m: Match,
    },
    Replace {
        #[serde(rename = "match")]
        m: Match,
        with: String,
    },
    ReplaceAll {
        query: String,
        #[serde(default)]
        options: FindOptions,
        with: String,
    },
    Copy,
    Cut,
    /// Paste: the internal format wins over HTML, HTML over plain text.
    Paste {
        #[serde(default)]
        internal: Option<String>,
        #[serde(default)]
        html: Option<String>,
        #[serde(default)]
        text: Option<String>,
    },
}

fn one() -> u8 {
    1
}

/// Clipboard payload returned by copy/cut.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipOut {
    pub text: String,
    pub html: String,
    pub internal: String,
}

/// Result of a command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub info: crate::DocInfo,
    pub selection: Selection,
    pub caret: Option<CaretRect>,
    pub rects: Vec<SelRect>,
    /// Pages whose display list changed (all pages after a page-count change).
    pub dirty: Vec<usize>,
    pub format: FormatState,
    /// Document changed since it was opened or last saved.
    pub modified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<Match>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<ClipOut>,
    /// Error message of a command that could not be applied (state is unchanged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl crate::Engine {
    /// Run a JSON command and return the [`EditResult`] as JSON.
    pub fn command_json(&mut self, json: &str) -> crate::Result<String> {
        let cmd: Command = serde_json::from_str(json)
            .map_err(|e| crate::EngineError::BadCommand(e.to_string()))?;
        let r = self.command(cmd)?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Run a command.
    pub fn command(&mut self, cmd: Command) -> crate::Result<EditResult> {
        let mut extra = (None, None, None);
        let mut error = None;
        let mut changed = false; // set when the document version moves
        let keep_goal = matches!(
            cmd,
            Command::Move {
                motion: Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown,
                ..
            }
        );
        {
            let layout = self.layout.as_ref().ok_or(crate::EngineError::NoDocument)?;
            let s = self
                .session
                .as_mut()
                .ok_or(crate::EngineError::NoDocument)?;
            let before_version = s.version();
            let sel = s.selection();
            let res: bo_edit::Result<()> = (|| {
                match cmd {
                    Command::State => {}
                    Command::SetSelection { selection } => s.set_selection(selection),
                    Command::Click {
                        page,
                        x,
                        y,
                        extend,
                        count,
                    } => {
                        if let Some(p) = nav::hit_test(layout, page, x, y) {
                            let new = match count {
                                2 => s.word_at(p),
                                3.. => s.paragraph_at(p),
                                _ if extend => Selection {
                                    anchor: sel.anchor,
                                    focus: p,
                                },
                                _ => Selection::caret(p),
                            };
                            s.set_selection(new);
                        }
                    }
                    Command::DragTo { page, x, y } => {
                        if let Some(p) = nav::hit_test(layout, page, x, y) {
                            s.set_selection(Selection {
                                anchor: sel.anchor,
                                focus: p,
                            });
                        }
                    }
                    Command::Move { motion, extend } => {
                        let from = sel.focus;
                        let to = match motion {
                            Motion::Left if !extend && !sel.is_collapsed() => sel.ordered().0,
                            Motion::Right if !extend && !sel.is_collapsed() => sel.ordered().1,
                            Motion::Left => s.char_step(from, false),
                            Motion::Right => s.char_step(from, true),
                            Motion::WordLeft => s.word_boundary(from, false),
                            Motion::WordRight => s.word_boundary(from, true),
                            Motion::DocStart => Pos::new(0, 0),
                            Motion::DocEnd => {
                                let last = s.paragraph_count().saturating_sub(1);
                                Pos::new(last, s.para_text(last).map_or(0, |t| t.len()))
                            }
                            _ => match nav::vertical(layout, from, motion, self.goal_x) {
                                Some((p, x)) => {
                                    if keep_goal {
                                        self.goal_x = Some(x);
                                    }
                                    p
                                }
                                None if motion == Motion::Up => Pos::new(from.para, 0),
                                None if motion == Motion::Down => Pos::new(
                                    from.para,
                                    s.para_text(from.para).map_or(0, |t| t.len()),
                                ),
                                None => from,
                            },
                        };
                        s.set_selection(if extend {
                            Selection {
                                anchor: sel.anchor,
                                focus: to,
                            }
                        } else {
                            Selection::caret(to)
                        });
                    }
                    Command::SelectAll => {
                        s.select_all();
                    }
                    Command::InsertText { text } => {
                        s.insert_text(&text)?;
                    }
                    Command::DeleteBackward => {
                        s.delete_backward()?;
                    }
                    Command::DeleteForward => {
                        s.delete_forward()?;
                    }
                    Command::SplitParagraph => {
                        s.split_paragraph()?;
                    }
                    Command::FormatText { format } => {
                        s.format_text(&format)?;
                    }
                    Command::FormatParagraph { format } => {
                        s.format_paragraphs(&format)?;
                    }
                    Command::ApplyStyle { id } => {
                        s.apply_style(&id)?;
                    }
                    Command::InsertTable { rows, cols } => {
                        s.insert_table(rows, cols)?;
                    }
                    Command::InsertPageBreak => {
                        s.insert_page_break()?;
                    }
                    Command::InsertSectionBreak { kind } => {
                        s.insert_section_break(kind.into())?;
                    }
                    Command::Undo => {
                        s.undo();
                    }
                    Command::Redo => {
                        s.redo();
                    }
                    Command::Find { query, options } => {
                        extra.0 = Some(s.find(&query, &options)?);
                    }
                    Command::SelectMatch { m } => s.set_selection(Selection {
                        anchor: Pos::new(m.para, m.start),
                        focus: Pos::new(m.para, m.end),
                    }),
                    Command::Replace { m, with } => {
                        s.replace(m, &with)?;
                    }
                    Command::ReplaceAll {
                        query,
                        options,
                        with,
                    } => {
                        extra.1 = Some(s.replace_all(&query, &options, &with)?);
                    }
                    Command::Copy => {
                        let c = s.copy();
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                        });
                    }
                    Command::Cut => {
                        let c = s.cut()?;
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                        });
                    }
                    Command::Paste {
                        internal,
                        html,
                        text,
                    } => {
                        if let Some(j) = internal.filter(|j| !j.is_empty()) {
                            s.paste_internal(&j)?;
                        } else if let Some(h) = html.filter(|h| !h.is_empty()) {
                            s.paste_html(&h)?;
                        } else if let Some(t) = text {
                            s.paste_text(&t)?;
                        }
                    }
                }
                Ok(())
            })();
            if let Err(e) = res {
                error = Some(e.to_string());
            }
            changed |= s.version() != before_version;
        }
        if !keep_goal {
            self.goal_x = None;
        }
        let dirty = if changed {
            self.relayout()?
        } else {
            Vec::new()
        };
        let r = self.edit_result(dirty, extra, error)?;
        Ok(r)
    }

    /// Insert a picture (PNG or JPEG) at the selection; sized at 96 dpi, capped to the text width.
    pub fn insert_image(&mut self, bytes: Vec<u8>, name: &str) -> crate::Result<EditResult> {
        let (w, h) =
            image_size(&bytes).ok_or_else(|| crate::EngineError::Unsupported(name.to_owned()))?;
        let ext = match bo_render::image::sniff(&bytes) {
            bo_render::image::Format::Jpeg => "jpeg",
            _ => "png",
        };
        let s = self
            .session
            .as_mut()
            .ok_or(crate::EngineError::NoDocument)?;
        let max_w = s.doc().sections.first().map_or(453.0, |sec| {
            (sec.page.width.0 - sec.page.margin_left.0 - sec.page.margin_right.0) as f32 / 20.0
        });
        // 96 dpi: 1 px = 0.75 pt = 9525 EMU.
        let mut wpt = w as f32 * 0.75;
        let mut hpt = h as f32 * 0.75;
        if wpt > max_w {
            hpt *= max_w / wpt;
            wpt = max_w;
        }
        let res = s.insert_image(bytes, ext, (wpt * 12_700.0) as i64, (hpt * 12_700.0) as i64);
        let error = res.err().map(|e| e.to_string());
        let dirty = self.relayout()?;
        self.edit_result(dirty, (None, None, None), error)
    }

    fn edit_result(
        &self,
        dirty: Vec<usize>,
        extra: (Option<Vec<Match>>, Option<usize>, Option<ClipOut>),
        error: Option<String>,
    ) -> crate::Result<EditResult> {
        let layout = self.layout.as_ref().ok_or(crate::EngineError::NoDocument)?;
        let s = self
            .session
            .as_ref()
            .ok_or(crate::EngineError::NoDocument)?;
        let sel = s.selection();
        Ok(EditResult {
            info: self.info()?,
            selection: sel,
            caret: nav::caret_rect(layout, sel.focus),
            rects: nav::selection_rects(layout, sel),
            dirty,
            format: s.format_state(),
            modified: self.saved_doc.as_ref() != Some(s.doc()),
            matches: extra.0,
            count: extra.1,
            clip: extra.2,
            error,
        })
    }
}

/// Pixel size of a PNG (IHDR) or JPEG (SOFn) picture.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    match bo_render::image::sniff(bytes) {
        bo_render::image::Format::Png => {
            // ISO/IEC 15948 §11.2.2 IHDR: width and height follow the chunk header.
            let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
            let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
            (w > 0 && h > 0).then_some((w, h))
        }
        bo_render::image::Format::Jpeg => {
            bo_render::image::jpeg_info(bytes).map(|(w, h, _)| (w, h))
        }
        bo_render::image::Format::Other => None,
    }
}

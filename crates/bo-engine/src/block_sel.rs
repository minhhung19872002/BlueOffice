//! Block (column) selection of the main text (BlueDoc round 9): Alt+drag or Ctrl+Shift+F8 and
//! the arrows select a rectangle across lines, as the desktop word processor does. The block
//! is a pair of corners (a text position and a horizontal position each); on every line between
//! the corners' lines it covers the characters whose middle lies between the two x. Copy puts one
//! paragraph per line on the clipboard, Delete / Backspace / typing remove the pieces, character
//! formatting and change case apply to each piece; every such edit is one undo step. Any other
//! command that moves the caret or edits the document ends the block.
use crate::edit::{Command, EditResult};
use crate::nav::{self, Motion, SelRect};
use crate::Engine;
use bo_edit::{CharFormat, Pos, Selection};

/// Corners of the block: the text position and horizontal position (points from the page's
/// left edge) where it started and where it extends to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BlockSel {
    anchor: Pos,
    ax: f32,
    focus: Pos,
    fx: f32,
}

/// One line's part of the block.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    from: Pos,
    to: Pos,
    rect: SelRect,
}

impl Engine {
    /// A block selection is active.
    pub fn block_active(&self) -> bool {
        self.block.is_some()
    }

    /// Text of each line of the block, top to bottom (tests, hosts).
    pub fn block_text(&self) -> Vec<String> {
        let Some(s) = self.session.as_ref() else {
            return Vec::new();
        };
        self.block_pieces()
            .iter()
            .map(|p| {
                s.para_text(p.from.para)
                    .and_then(|t| t.get(p.from.offset..p.to.offset).map(str::to_owned))
                    .unwrap_or_default()
                    .replace('\u{FFFC}', "")
            })
            .collect()
    }

    /// Run `cmd`, with the block selection's own commands and the commands that act on a block.
    pub(crate) fn block_wrap(
        &mut self,
        cmd: Command,
        now: Option<i64>,
    ) -> crate::Result<EditResult> {
        let main = self.hf.is_none() && self.note.is_none() && self.textbox.is_none();
        if !main {
            self.block = None;
            return self.command_inner(cmd, now);
        }
        match cmd {
            Command::BlockSelect { page, x, y, extend } => {
                let at = nav::hit_test(self.layout.document(), page, x, y);
                if let Some(at) = at {
                    self.block = Some(match self.block.filter(|_| extend) {
                        Some(b) => BlockSel {
                            focus: at,
                            fx: x,
                            ..b
                        },
                        None => BlockSel {
                            anchor: at,
                            ax: x,
                            focus: at,
                            fx: x,
                        },
                    });
                }
                self.block_result(Vec::new(), None)
            }
            Command::BlockMove { motion } => {
                if self.block.is_none() {
                    self.block_from_caret();
                }
                self.block_move(motion);
                self.block_result(Vec::new(), None)
            }
            Command::MoveText { to, copy } => {
                self.block = None;
                self.move_text(to, copy, now)
            }
            Command::BlockClear => {
                self.block = None;
                self.command_inner(Command::State, now)
            }
            cmd if self.block.is_some() => self.block_edit(cmd, now),
            cmd => self.command_inner(cmd, now),
        }
    }

    /// Start a block at the caret (Ctrl+Shift+F8).
    fn block_from_caret(&mut self) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        let at = s.selection().focus;
        let x = nav::caret_rect(self.layout.document(), at).map_or(0.0, |c| c.x);
        self.block = Some(BlockSel {
            anchor: at,
            ax: x,
            focus: at,
            fx: x,
        });
    }

    /// Move the block's moving corner: Left / Right by a character on its line (the column
    /// follows the character), Up / Down a line keeping the column, Home / End to the line ends.
    fn block_move(&mut self, motion: Motion) {
        let (Some(b), Some(s)) = (self.block, self.session.as_ref()) else {
            return;
        };
        let layout = self.layout.document();
        let lines = nav::body_lines(layout);
        let Some((i, _, line)) = nav::line_of(&lines, b.focus) else {
            return;
        };
        let step = |p: Pos, fwd: bool| -> Pos {
            let n = s.char_step(p, fwd);
            // Stay on the corner's line.
            if n.para == p.para && n.offset >= line.start as usize && n.offset <= line.end as usize
            {
                n
            } else {
                p
            }
        };
        let (focus, fx) = match motion {
            Motion::Left | Motion::WordLeft => {
                let p = step(b.focus, false);
                (p, nav::caret_x(line, p.offset))
            }
            Motion::Right | Motion::WordRight => {
                let p = step(b.focus, true);
                (p, nav::caret_x(line, p.offset))
            }
            Motion::Up | Motion::Down | Motion::ParaUp | Motion::ParaDown => {
                let up = matches!(motion, Motion::Up | Motion::ParaUp);
                let j = if up { i.checked_sub(1) } else { Some(i + 1) };
                match j.and_then(|j| lines.get(j)) {
                    Some(&(_, l)) => match nav::body_id(l) {
                        Some(id) => {
                            // The first stop at or right of the column, as the reference.
                            let off = l
                                .carets
                                .iter()
                                .find(|c| c.1 >= b.fx - 0.5)
                                .map_or_else(|| nav::offset_at(l, b.fx), |c| c.0 as usize);
                            (Pos::new(id, off), b.fx)
                        }
                        None => (b.focus, b.fx),
                    },
                    None => (b.focus, b.fx),
                }
            }
            Motion::LineStart => {
                let p = Pos::new(b.focus.para, line.start as usize);
                (p, nav::caret_x(line, p.offset))
            }
            Motion::LineEnd => {
                let len = s.para_text(b.focus.para).map_or(0, |t| t.len());
                let p = Pos::new(b.focus.para, (line.end as usize).min(len));
                (p, nav::caret_x(line, p.offset))
            }
            _ => (b.focus, b.fx),
        };
        self.block = Some(BlockSel { focus, fx, ..b });
    }

    /// The block as drawn: one rectangle per page from the start corner's x to the moving
    /// corner's x, from the top of its first line to the bottom of its last (the space
    /// between paragraphs included), as the reference highlights a column selection.
    fn block_rects(&self) -> Vec<SelRect> {
        let Some(b) = self.block else {
            return Vec::new();
        };
        let layout = self.layout.document();
        let lines = nav::body_lines(layout);
        let (Some((ia, ..)), Some((ib, ..))) = (
            nav::line_of(&lines, b.anchor),
            nav::line_of(&lines, b.focus),
        ) else {
            return Vec::new();
        };
        let (x0, x1) = (b.ax.min(b.fx), b.ax.max(b.fx));
        if x1 - x0 < 0.01 {
            return Vec::new();
        }
        let mut out: Vec<SelRect> = Vec::new();
        for &(page, l) in lines.get(ia.min(ib)..=ia.max(ib)).unwrap_or_default() {
            if l.rotation.is_some() {
                continue;
            }
            match out.last_mut().filter(|r| r.page == page) {
                Some(r) => {
                    let bottom = (r.y + r.h).max(l.top + l.height);
                    r.y = r.y.min(l.top);
                    r.h = bottom - r.y;
                }
                None => out.push(SelRect {
                    page,
                    x: x0,
                    y: l.top,
                    w: x1 - x0,
                    h: l.height,
                }),
            }
        }
        out
    }

    /// The block's piece on every line between its corners' lines (lines with no character
    /// between the two x have none).
    fn block_pieces(&self) -> Vec<Piece> {
        let (Some(b), Some(s)) = (self.block, self.session.as_ref()) else {
            return Vec::new();
        };
        let layout = self.layout.document();
        let lines = nav::body_lines(layout);
        let (Some((ia, ..)), Some((ib, ..))) = (
            nav::line_of(&lines, b.anchor),
            nav::line_of(&lines, b.focus),
        ) else {
            return Vec::new();
        };
        let (i0, i1) = (ia.min(ib), ia.max(ib));
        let (x0, x1) = (b.ax.min(b.fx), b.ax.max(b.fx));
        let mut out = Vec::new();
        for &(page, l) in lines.get(i0..=i1).unwrap_or_default() {
            let Some(id) = nav::body_id(l) else { continue };
            if l.rotation.is_some() {
                continue;
            }
            let len = s.para_text(id).map_or(0, |t| t.len());
            let stops: Vec<(u32, f32)> = l.carets.iter().collect();
            // Characters whose middle is inside [x0, x1].
            let mut from: Option<(usize, f32)> = None;
            let mut to: Option<(usize, f32)> = None;
            for w in stops.windows(2) {
                let ((o0, a), (o1, c)) = (w[0], w[1]);
                let mid = a.midpoint(c);
                if (o1 as usize) > len || o1 <= o0 {
                    continue;
                }
                if mid >= x0 && mid <= x1 {
                    if from.is_none() {
                        from = Some((o0 as usize, a));
                    }
                    to = Some((o1 as usize, c));
                }
            }
            let (Some((fo, fx)), Some((to, tx))) = (from, to) else {
                continue;
            };
            out.push(Piece {
                from: Pos::new(id, fo),
                to: Pos::new(id, to),
                rect: SelRect {
                    page,
                    x: fx,
                    y: l.top,
                    w: (tx - fx).max(0.5),
                    h: l.height,
                },
            });
        }
        out
    }

    /// A command while the block is active: the ones that act on the block, others end it.
    fn block_edit(&mut self, cmd: Command, now: Option<i64>) -> crate::Result<EditResult> {
        let pieces = self.block_pieces();
        let restore = self.block.map(|b| Selection {
            anchor: b.anchor,
            focus: b.focus,
        });
        match cmd {
            Command::Copy | Command::Cut => {
                let clip = self.block_clip(&pieces, now)?;
                let mut dirty = Vec::new();
                if matches!(cmd, Command::Cut) {
                    dirty = self.block_delete(&pieces, now)?;
                    self.block = None;
                } else if let (Some(sel), Some(s)) = (restore, self.session.as_mut()) {
                    s.set_selection(sel);
                }
                self.block_result(dirty, Some(clip))
            }
            Command::DeleteBackward
            | Command::DeleteForward
            | Command::DeleteWordBackward
            | Command::DeleteWordForward => {
                let dirty = self.block_delete(&pieces, now)?;
                self.block = None;
                self.block_result(dirty, None)
            }
            Command::InsertText { .. } => {
                let mut dirty = self.block_delete(&pieces, now)?;
                self.block = None;
                let r = self.command_inner(cmd, now)?;
                dirty.extend(r.dirty);
                self.block_result(dirty, None)
            }
            Command::FormatText { .. } | Command::ChangeCase { .. } => {
                let depth = self
                    .session
                    .as_ref()
                    .map_or(0, bo_edit::Session::history_len);
                let mut dirty = Vec::new();
                // A toggle turns the property on in every piece unless all have it (then off), as
                // for one selection.
                let flag = |f: &bo_edit::FormatState| match &cmd {
                    Command::FormatText { format } => match format {
                        CharFormat::Bold => Some(f.bold),
                        CharFormat::Italic => Some(f.italic),
                        CharFormat::Underline => Some(f.underline),
                        CharFormat::Strike => Some(f.strike),
                        CharFormat::Superscript => Some(f.superscript),
                        CharFormat::Subscript => Some(f.subscript),
                        _ => None,
                    },
                    _ => None,
                };
                let states: Vec<Option<bool>> = pieces
                    .iter()
                    .map(|p| {
                        let s = self.session.as_mut()?;
                        s.set_selection(Selection {
                            anchor: p.from,
                            focus: p.to,
                        });
                        flag(&s.format_state())
                    })
                    .collect();
                let all_on = states.iter().all(|f| *f == Some(true));
                for (p, state) in pieces.iter().zip(&states) {
                    if state.is_some() && !all_on && *state == Some(true) {
                        continue;
                    }
                    self.command_inner(
                        Command::SetSelection {
                            selection: Selection {
                                anchor: p.from,
                                focus: p.to,
                            },
                        },
                        now,
                    )?;
                    dirty.extend(self.command_inner(cmd.clone(), now)?.dirty);
                }
                if let Some(s) = self.session.as_mut() {
                    s.merge_history_since(depth);
                    // The ribbon shows the formatting of the block's first piece.
                    if let Some(p) = pieces.first() {
                        s.set_selection(Selection {
                            anchor: p.from,
                            focus: p.to,
                        });
                    }
                }
                self.block_result(dirty, None)
            }
            // Queries keep the block; anything that moves the caret or edits ends it.
            cmd => {
                let before = self.session.as_ref().map(|s| (s.selection(), s.version()));
                let r = self.command_inner(cmd, now)?;
                let after = self.session.as_ref().map(|s| (s.selection(), s.version()));
                if before == after {
                    let mut r = r;
                    r.rects = self.block_rects();
                    Ok(r)
                } else {
                    self.block = None;
                    Ok(r)
                }
            }
        }
    }

    /// Clipboard of the block: one paragraph per line.
    fn block_clip(
        &mut self,
        pieces: &[Piece],
        now: Option<i64>,
    ) -> crate::Result<crate::edit::ClipOut> {
        let mut blocks: Vec<bo_model::Block> = Vec::new();
        let mut text = Vec::new();
        for p in pieces {
            let r = self.command_inner(
                Command::SetSelection {
                    selection: Selection {
                        anchor: p.from,
                        focus: p.to,
                    },
                },
                now,
            )?;
            drop(r);
            let Some(s) = self.session.as_ref() else {
                break;
            };
            let c = s.copy();
            text.push(c.text);
            if let Ok(b) = serde_json::from_str::<Vec<bo_model::Block>>(&c.internal) {
                blocks.extend(b);
            }
        }
        let html = self
            .session
            .as_ref()
            .map(|s| bo_edit::html::to_html(s.doc(), &blocks))
            .unwrap_or_default();
        Ok(crate::edit::ClipOut {
            text: text.join("\n"),
            html,
            internal: serde_json::to_string(&blocks).unwrap_or_default(),
            png: None,
        })
    }

    /// Delete every piece (last first, so earlier positions stay valid) as one undo step; the
    /// caret ends at the first piece. Returns the changed pages.
    fn block_delete(&mut self, pieces: &[Piece], now: Option<i64>) -> crate::Result<Vec<usize>> {
        let depth = self
            .session
            .as_ref()
            .map_or(0, bo_edit::Session::history_len);
        let mut dirty = Vec::new();
        for p in pieces.iter().rev() {
            self.command_inner(
                Command::SetSelection {
                    selection: Selection {
                        anchor: p.from,
                        focus: p.to,
                    },
                },
                now,
            )?;
            dirty.extend(self.command_inner(Command::DeleteForward, now)?.dirty);
        }
        if let Some(s) = self.session.as_mut() {
            s.merge_history_since(depth);
            let at = pieces
                .first()
                .map(|p| p.from)
                .or(self.block.map(|b| b.anchor));
            if let Some(at) = at {
                s.set_selection(Selection {
                    anchor: at,
                    focus: at,
                });
            }
        }
        Ok(dirty)
    }

    /// Result after a block command: the caret at the moving corner, the block's rectangles.
    fn block_result(
        &mut self,
        mut dirty: Vec<usize>,
        clip: Option<crate::edit::ClipOut>,
    ) -> crate::Result<EditResult> {
        if let (Some(b), Some(s)) = (self.block, self.session.as_mut()) {
            if dirty.is_empty() {
                s.set_selection(Selection {
                    anchor: b.anchor,
                    focus: b.focus,
                });
            }
        }
        dirty.extend(self.relayout()?);
        dirty.sort_unstable();
        dirty.dedup();
        let mut r = self.edit_result(dirty, (None, None, clip), None)?;
        if self.block.is_some() {
            r.rects = self.block_rects();
        }
        Ok(r)
    }
}

#[cfg(test)]
#[path = "block_sel_tests.rs"]
mod tests;

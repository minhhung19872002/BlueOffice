//! BlueDoc parity round 9 (docs/ui/REPORT.md "Vòng 9"): pointer and keyboard gestures of the
//! desktop word processor that select or move text — block (column) selection with Alt+drag or
//! Ctrl+Shift+F8 and the arrows, dragging in the selection bar line by line (paragraph by
//! paragraph after a double click), auto-scroll while dragging past the view, and drag-and-drop
//! of the selection (Ctrl copies) with the dotted drop caret. A child module of the shell.
use super::DocWindow;
use crate::{Input, Key, Mods, Painter, Rgba};
use bo_edit::Pos;
use bo_engine::edit::Command;
use bo_engine::nav::Motion;

/// Round-9 state of the window.
#[derive(Default)]
pub(super) struct R9 {
    /// Ctrl+Shift+F8: the arrows extend a block selection until another key or Esc.
    pub(super) block_keys: bool,
    /// An Alt+drag block selection is in progress.
    pub(super) block_drag: bool,
    /// A drag that began in the selection bar: the range the press selected and whether it
    /// extends by paragraphs (a double click) instead of lines.
    pub(super) bar: Option<((Pos, Pos), bool)>,
    /// A press inside the selection: it may become a drag-and-drop.
    pub(super) dnd: Option<Dnd>,
    /// Keys held with the last release (Ctrl copies on a drop).
    pub(super) release_mods: Mods,
}

/// A press inside the selection, maybe dragged.
pub(super) struct Dnd {
    /// Where the press was (window pixels) and its click data, replayed when it is a click.
    press: (f32, f32),
    /// The pointer moved far enough to drag the text.
    moving: bool,
    /// Where the text would be dropped.
    target: Option<Pos>,
}

/// Pointer travel (device pixels at 100 %) before a press inside the selection drags it.
const DRAG_START: f32 = 4.0;
/// Grey of the drop caret (dotted), as on the reference.
const DROP_CARET: Rgba = Rgba(64, 64, 64, 255);

fn motion_of(k: Key, ctrl: bool) -> Option<Motion> {
    Some(match k {
        Key::Left if ctrl => Motion::WordLeft,
        Key::Right if ctrl => Motion::WordRight,
        Key::Left => Motion::Left,
        Key::Right => Motion::Right,
        Key::Up if ctrl => Motion::ParaUp,
        Key::Down if ctrl => Motion::ParaDown,
        Key::Up => Motion::Up,
        Key::Down => Motion::Down,
        Key::Home => Motion::LineStart,
        Key::End => Motion::LineEnd,
        _ => return None,
    })
}

impl DocWindow {
    /// Keys held with the next release (the host reads them from the mouse message).
    pub fn set_release_mods(&mut self, mods: Mods) {
        self.r9.release_mods = mods;
    }

    /// A press in the document: block selection (Alt), the start of a selection-bar drag or of
    /// a drag-and-drop; `None` lets the round-8 gestures and the view take it.
    pub(super) fn r9_press(&mut self, x: f32, y: f32, count: u8, shift: bool) -> Option<bool> {
        let mods = self.r8.pointer;
        self.r9.bar = None;
        self.r9.dnd = None;
        self.r9.block_drag = false;
        self.r9.block_keys = false;
        if self.view.story().is_some() || self.view.engine().math_active() {
            return None;
        }
        let block = self.view.engine().block_active();
        let (page, px, py) = self.r8_page_point(x, y)?;
        if mods.alt && !shift && count == 1 {
            self.r8.pointer = Mods::default();
            self.r9.block_drag = true;
            return Some(self.view.run_command(Command::BlockSelect {
                page,
                x: px,
                y: py,
                extend: false,
            }));
        }
        if block {
            self.view.run_command(Command::BlockClear);
        }
        if self.r8_in_selection_bar(page, px, py) {
            if count <= 2 && !mods.ctrl {
                let r = self.r8_press(x, y, count, shift);
                let sel = self.view.engine().session()?.selection();
                // Shift+press extends from the old anchor: drag on from the line pressed.
                let start = if shift {
                    self.r8_line_at(page, py)
                } else {
                    Some(sel.ordered())
                };
                self.r9.bar = start.map(|s| (s, count == 2));
                return r;
            }
            return None;
        }
        if count == 1 && !shift && !mods.ctrl && self.r9_in_selection(page, px, py) {
            self.r8.pointer = Mods::default();
            self.r9.dnd = Some(Dnd {
                press: (x, y),
                moving: false,
                target: None,
            });
            return Some(false);
        }
        None
    }

    /// The point is on the highlighted selection.
    fn r9_in_selection(&self, page: usize, px: f32, py: f32) -> bool {
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let sel = s.selection();
        if sel.is_collapsed() {
            return false;
        }
        let (a, b) = sel.ordered();
        self.view
            .engine()
            .range_rects(a, b)
            .iter()
            .any(|r| r.page == page && px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h)
    }

    /// A drag that one of the round-9 gestures owns; `None` for the view's own drag.
    pub(super) fn r9_drag(&mut self, x: f32, y: f32) -> Option<bool> {
        if !(self.r9.block_drag || self.r9.bar.is_some() || self.r9.dnd.is_some()) {
            return None;
        }
        let scrolled = self.r9_autoscroll(y);
        let top = self.view_top();
        let bottom = self.size.1 - self.status_h();
        let y = y.clamp(top, (bottom - 1.0).max(top));
        if self.r9.block_drag {
            let Some((page, px, py)) = self.r8_page_point(x, y) else {
                return Some(scrolled);
            };
            return Some(
                self.view.run_command(Command::BlockSelect {
                    page,
                    x: px,
                    y: py,
                    extend: true,
                }) || scrolled,
            );
        }
        if let Some(((a, b), para)) = self.r9.bar {
            let Some((page, _, py)) = self.r9_page_row(y) else {
                return Some(scrolled);
            };
            let Some((la, lb)) = self.r8_line_at(page, py) else {
                return Some(scrolled);
            };
            let (la, lb) = if para {
                self.r8_paragraph_with_mark(la).unwrap_or((la, lb))
            } else {
                (la, lb)
            };
            let sel = if la >= a { (a, lb) } else { (b, la) };
            return Some(self.r8_select(sel.0, sel.1) || scrolled);
        }
        let dnd = self.r9.dnd.as_ref()?;
        let (x0, y0) = dnd.press;
        let moved = (x - x0).hypot(y - y0) >= DRAG_START * self.scale.max(1.0);
        if !dnd.moving && !moved {
            return Some(scrolled);
        }
        let target = self.r8_page_point(x, y).and_then(|(page, px, py)| {
            bo_engine::nav::hit_test(self.view.engine().layout()?, page, px, py)
        });
        if let Some(d) = self.r9.dnd.as_mut() {
            d.moving = true;
            d.target = target;
        }
        Some(true)
    }

    /// The page and point of the body row at window height `y` (any x: the selection bar).
    fn r9_page_row(&self, y: f32) -> Option<(usize, f32, f32)> {
        let slot = self
            .view
            .slots()
            .into_iter()
            .find(|p| y >= p.y as f32 && y < (p.y + p.height) as f32)?;
        Some((slot.index, 0.0, (y - slot.y as f32) / self.view.scale()))
    }

    /// Past the top or bottom of the view, a drag scrolls the document.
    fn r9_autoscroll(&mut self, y: f32) -> bool {
        let top = self.view_top();
        let bottom = self.size.1 - self.status_h();
        let over = if y < top {
            y - top
        } else if y > bottom {
            y - bottom
        } else {
            0.0
        };
        over != 0.0 && self.view.handle(Input::Wheel(over * 0.5))
    }

    /// A drag of a round-9 gesture: the pointer rests past the view (auto-scroll tick).
    pub(super) fn r9_dragging(&self) -> bool {
        self.r9.block_drag
            || self.r9.bar.is_some()
            || self.r9.dnd.as_ref().is_some_and(|d| d.moving)
    }

    /// The button went up: end the gesture (dropping dragged text, or placing the caret where
    /// a press inside the selection did not drag).
    pub(super) fn r9_release(&mut self) -> bool {
        self.r9.block_drag = false;
        self.r9.bar = None;
        let Some(d) = self.r9.dnd.take() else {
            return false;
        };
        let copy = std::mem::take(&mut self.r9.release_mods).ctrl;
        if !d.moving {
            let (x, y) = d.press;
            return self.view.handle(Input::Press {
                x,
                y,
                count: 1,
                shift: false,
            });
        }
        match d.target {
            Some(to) => self.view.run_command(Command::MoveText { to, copy }),
            None => true,
        }
    }

    /// Keys of the block selection: Ctrl+Shift+F8 starts it, the arrows then extend it, Esc
    /// ends it; other keys act on the block (copy, delete, format) and end the key mode.
    pub(super) fn r9_key(&mut self, k: Key, m: Mods) -> Option<bool> {
        if k == Key::F(8) && m.ctrl && m.shift && !m.alt {
            self.r8.extend = None;
            self.r9.block_keys = true;
            return Some(true);
        }
        let block = self.view.engine().block_active();
        if k == Key::Escape && (self.r9.block_keys || block) {
            self.r9.block_keys = false;
            return Some(self.view.run_command(Command::BlockClear));
        }
        if self.r9.block_keys {
            if let Some(motion) = motion_of(k, m.ctrl) {
                return Some(self.view.run_command(Command::BlockMove { motion }));
            }
            self.r9.block_keys = false;
        }
        None
    }

    /// The dotted drop caret of a drag-and-drop.
    pub(super) fn r9_paint(&self, p: &mut dyn Painter) {
        let Some(to) = self.r9.dnd.as_ref().and_then(|d| d.target) else {
            return;
        };
        let Some(c) = self
            .view
            .engine()
            .layout()
            .and_then(|l| bo_engine::nav::caret_rect(l, to))
        else {
            return;
        };
        let Some(slot) = self.view.slots().into_iter().find(|s| s.index == c.page) else {
            return;
        };
        let s = self.view.scale();
        let unit = self.scale.round().max(1.0);
        let x = (slot.x as f32 + c.x * s).round();
        let (y0, h) = (slot.y as f32 + c.y * s, c.h * s);
        let mut y = y0.round();
        while y < y0 + h {
            p.fill_rect(x, y, unit, unit, DROP_CARET);
            y += 2.0 * unit;
        }
    }

    /// The selection as it is (tests).
    #[cfg(test)]
    pub(super) fn r9_selection(&self) -> Option<bo_edit::Selection> {
        self.view
            .engine()
            .session()
            .map(bo_edit::Session::selection)
    }
}

#[cfg(test)]
#[path = "doc_r9_tests.rs"]
mod tests;

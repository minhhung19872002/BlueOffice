//! BlueDoc parity round 10 (docs/ui/REPORT.md "Vòng 10"): drag and drop between applications
//! (the host's OLE drag carries the selection out as text / HTML / internal form and brings
//! text in from other applications), the pointer shapes of the document area, and the round's
//! context menus. A child module of the shell.
use super::{DocWindow, Request};
use bo_edit::Pos;
use bo_engine::edit::Command;

/// Round-10 state of the window.
#[derive(Default)]
pub(super) struct R10 {
    /// The host runs OLE drag and drop ([`DocWindow::set_ole_drag`]).
    pub(super) ole: bool,
    /// The current OLE drag of our selection was dropped back into this window.
    pub(super) dropped_inside: bool,
    /// Where text dragged in from another application would drop.
    pub(super) drop_to: Option<Pos>,
    /// Last pointer position over the window (pointer shape).
    pub(super) pointer: (f32, f32),
    /// The open context menu's own rows and mini toolbar.
    pub(super) ctx: Option<super::ctx10::Ctx>,
}

/// Pointer shapes the host maps to system cursors (see the host's `cursor`).
pub mod pointer {
    /// Arrow.
    pub const ARROW: u8 = 3;
    /// Text (I-beam).
    pub const TEXT: u8 = 4;
}

impl DocWindow {
    /// The host drags text with OLE drag and drop: a drag of the selection becomes
    /// [`Request::DragText`], and [`DocWindow::drag_over`] / [`DocWindow::drop_data`] /
    /// [`DocWindow::drag_done`] report it.
    pub fn set_ole_drag(&mut self, on: bool) {
        self.r10.ole = on;
    }

    /// The selection started to move: hand it to the host's drag.
    pub(super) fn r10_drag_started(&mut self) {
        if !self.r10.ole {
            return;
        }
        self.r10.dropped_inside = false;
        if let Some(c) = self.view.copy(false) {
            self.requests.push(Request::DragText {
                text: c.text,
                html: c.html,
                internal: c.internal,
            });
        }
    }

    /// Text position under window point (`x`, `y`), if it is on a page.
    fn r10_pos_at(&self, x: f32, y: f32) -> Option<Pos> {
        let (page, px, py) = self.r8_page_point(x, y)?;
        bo_engine::nav::hit_test(self.view.engine().layout()?, page, px, py)
    }

    /// Text is dragged over (`x`, `y`): show where it would drop; false where it cannot.
    pub fn drag_over(&mut self, x: f32, y: f32) -> bool {
        let to = self.r10_pos_at(x, y);
        if let Some(d) = self.r9.dnd.as_mut() {
            // Our own selection: not inside itself.
            let inside = to.is_some_and(|p| {
                self.view.engine().session().is_some_and(|s| {
                    let (a, b) = s.selection().ordered();
                    p > a && p < b
                })
            });
            d.target = to.filter(|_| !inside);
            return d.target.is_some();
        }
        self.r10.drop_to = to;
        to.is_some()
    }

    /// The drag left the window: no drop caret.
    pub fn drag_leave(&mut self) {
        if let Some(d) = self.r9.dnd.as_mut() {
            d.target = None;
        }
        self.r10.drop_to = None;
    }

    /// Text dropped at (`x`, `y`): our own selection moves (or is copied) there; text from
    /// another application is pasted there (with smart spacing). True when it was taken.
    pub fn drop_data(
        &mut self,
        x: f32,
        y: f32,
        copy: bool,
        internal: Option<String>,
        html: Option<String>,
        text: Option<String>,
    ) -> bool {
        let to = self.r10_pos_at(x, y);
        self.r10.drop_to = None;
        if let Some(d) = self.r9.dnd.as_mut() {
            d.target = None;
            let Some(to) = to else { return false };
            let inside = self.view.engine().session().is_some_and(|s| {
                let (a, b) = s.selection().ordered();
                to > a && to < b
            });
            if inside {
                return false;
            }
            self.r10.dropped_inside = true;
            self.view.run_command(Command::MoveText { to, copy });
            return true;
        }
        let Some(to) = to else { return false };
        if internal.is_none() && html.is_none() && text.is_none() {
            return false;
        }
        self.view.run_command(Command::SetSelection {
            selection: bo_edit::Selection::caret(to),
        });
        let mode = self.r3_paste_mode(internal.is_some(), html.is_some());
        self.view.paste_with(internal, html, text, Some(mode));
        true
    }

    /// The host's drag of our selection ended with OLE effect `effect`: moved into another
    /// application (2) takes the text out of the document (smart cut).
    pub fn drag_done(&mut self, effect: u32) {
        let inside = std::mem::take(&mut self.r10.dropped_inside);
        if self.r9.dnd.take().is_some() && !inside && effect == 2 {
            self.view.run_command(Command::Cut);
        }
        self.doc_press = false;
        self.last_drag = None;
    }

    /// Pointer shape at the last pointer position: an arrow over the interface, a menu or
    /// dialog, the scroll bar and the selection (which can be dragged), text elsewhere in the
    /// document area.
    pub fn cursor(&self) -> u8 {
        let (x, y) = self.r10.pointer;
        if self.menu.is_some() || self.backstage.is_some() || self.r10_modal() {
            return pointer::ARROW;
        }
        let status_top = self.size.1 - self.status_h();
        if y < self.view_top() || y >= status_top || x >= self.doc_right() - self.sb_w() {
            return pointer::ARROW;
        }
        if x < self.nav_w() {
            return pointer::ARROW;
        }
        if let Some((page, px, py)) = self.r8_page_point(x, y) {
            if self.r10_in_selection(page, px, py) {
                return pointer::ARROW;
            }
        }
        pointer::TEXT
    }

    /// Widths in points of the "+---" runs of the typed line `text` of body paragraph `para`
    /// (from each `+` to the next, as laid out).
    pub(super) fn r10_plus_widths(&self, para: usize, text: &str) -> Vec<f32> {
        let Some(layout) = self.view.engine().layout() else {
            return Vec::new();
        };
        let xs: Vec<f32> = text
            .match_indices('+')
            .filter_map(|(i, _)| bo_engine::nav::caret_rect(layout, Pos::new(para, i)))
            .map(|c| c.x)
            .collect();
        xs.windows(2).map(|w| (w[1] - w[0]).max(1.0)).collect()
    }

    /// Give the columns of the table holding the caret (just inserted, one row) `widths`.
    pub(super) fn r10_set_column_widths(&mut self, widths: &[f32]) {
        let Some(at) = self.view.engine().session().map(|s| s.selection().focus) else {
            return;
        };
        for (i, w) in widths.iter().enumerate() {
            self.view.run_command(Command::SetSelection {
                selection: bo_edit::Selection::caret(Pos::new(at.para + i, 0)),
            });
            self.view.run_command(Command::TableEdit {
                edit: bo_edit::TableEdit::ColumnWidth {
                    twips: (w * 20.0).round() as i32,
                },
            });
        }
        self.view.run_command(Command::SetSelection {
            selection: bo_edit::Selection::caret(at),
        });
    }

    /// A dialog or pane over the document takes the pointer.
    fn r10_modal(&self) -> bool {
        self.dialog.is_some() || self.find_dialog.is_some()
    }

    fn r10_in_selection(&self, page: usize, px: f32, py: f32) -> bool {
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
}

#[cfg(test)]
#[path = "doc_r10_tests.rs"]
mod tests;

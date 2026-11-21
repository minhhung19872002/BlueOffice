//! BlueDoc round 8: mouse gestures of the desktop word processor measured on the same copy
//! (`wmouse.ps1`: posted clicks on the reference, its selection read back): the selection
//! bar left of the text selects the line (with its trailing spaces, and the paragraph mark
//! on a paragraph's last line), a double click there the paragraph, a triple click or a
//! Ctrl+click the whole document; Ctrl+click in the text selects the sentence with the
//! spaces after it.
use super::super::DocWindow;
use crate::Mods;
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;
use bo_layout::LineSource;

impl DocWindow {
    /// Keys held with the next pointer press (the host reads them from the mouse message).
    pub fn set_pointer_mods(&mut self, mods: Mods) {
        self.r8.pointer = mods;
    }

    /// A press in the document: the round-8 gestures, `None` for the view's own click.
    pub(in crate::shell) fn r8_press(
        &mut self,
        x: f32,
        y: f32,
        count: u8,
        shift: bool,
    ) -> Option<bool> {
        let mods = std::mem::take(&mut self.r8.pointer);
        if self.view.story().is_some() || self.view.engine().math_active() {
            return None;
        }
        self.r8.extend = None;
        let (page, px, py) = self.r8_page_point(x, y)?;
        if self.r8_in_selection_bar(page, px, py) {
            if count >= 3 || mods.ctrl {
                return Some(self.view.run_command(Command::SelectAll));
            }
            let (a, b) = self.r8_line_at(page, py)?;
            let sel = if count == 2 {
                let (p0, p1) = self.r8_paragraph_with_mark(a)?;
                Selection {
                    anchor: p0,
                    focus: p1,
                }
            } else if shift {
                let anchor = self.view.engine().session()?.selection().anchor;
                Selection {
                    anchor,
                    focus: if b > anchor { b } else { a },
                }
            } else {
                Selection {
                    anchor: a,
                    focus: b,
                }
            };
            return Some(
                self.view
                    .run_command(Command::SetSelection { selection: sel }),
            );
        }
        // A triple click selects the paragraph with its mark (tpl_para).
        if count >= 3 && !shift && self.view.format().table.is_none() {
            let handled = self.view.handle(crate::Input::Press { x, y, count, shift });
            let at = self.view.engine().session()?.selection().ordered().0;
            if let Some((a, b)) = self.r8_paragraph_with_mark(at) {
                self.r8_select(a, b);
                return Some(true);
            }
            return Some(handled);
        }
        if mods.ctrl && !shift && count == 1 {
            return Some(
                self.view
                    .run_command(Command::SelectSentence { page, x: px, y: py }),
            );
        }
        None
    }

    /// The paragraph at `p` up to the next paragraph's start (its mark selected), or to its
    /// end in the last paragraph.
    fn r8_paragraph_with_mark(&self, p: Pos) -> Option<(Pos, Pos)> {
        let s = self.view.engine().session()?;
        let len = s.para_text(p.para).map_or(0, |t| t.len());
        let end = if p.para + 1 < s.paragraph_count() {
            Pos::new(p.para + 1, 0)
        } else {
            Pos::new(p.para, len)
        };
        Some((Pos::new(p.para, 0), end))
    }

    /// Page index and point (points from the page's top-left corner) under a window pixel.
    fn r8_page_point(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let s = self.view.scale();
        let slot = self
            .view
            .slots()
            .into_iter()
            .find(|p| y >= p.y as f32 && y < (p.y + p.height) as f32)?;
        if x < slot.x as f32 || x >= (slot.x + slot.width) as f32 {
            return None;
        }
        Some((slot.index, (x - slot.x as f32) / s, (y - slot.y as f32) / s))
    }

    /// The point is in the selection bar: left of the page's text column (the left margin)
    /// and left of the line's text.
    fn r8_in_selection_bar(&self, page: usize, px: f32, py: f32) -> bool {
        let margin = self.view.format().section.margin_left as f32 / 20.0;
        let left = self
            .r8_line_left(page, py)
            .map_or(margin, |l| l.min(margin));
        px < left - 1.0
    }

    /// Left edge of the body line nearest `py` on `page`.
    fn r8_line_left(&self, page: usize, py: f32) -> Option<f32> {
        self.r8_nearest_line(page, py)?.carets.first().map(|c| c.1)
    }

    fn r8_nearest_line(&self, page: usize, py: f32) -> Option<&bo_layout::Line> {
        let p = self.view.engine().layout()?.pages.get(page)?;
        p.lines
            .iter()
            .filter(|l| matches!(l.source, LineSource::Body(_)) && l.rotation.is_none())
            .min_by(|a, b| {
                let d = |l: &bo_layout::Line| {
                    if py < l.top {
                        l.top - py
                    } else if py > l.top + l.height {
                        py - l.top - l.height
                    } else {
                        0.0
                    }
                };
                d(a).total_cmp(&d(b))
            })
    }

    /// The line under `py` as a selection range: its text with the trailing spaces, up to the
    /// next paragraph's start on the paragraph's last line (its mark selected too).
    fn r8_line_at(&self, page: usize, py: f32) -> Option<(Pos, Pos)> {
        let line = self.r8_nearest_line(page, py)?;
        let LineSource::Body(id) = line.source else {
            return None;
        };
        let (id, start, end) = (id as usize, line.start as usize, line.end as usize);
        let s = self.view.engine().session()?;
        let len = s.para_text(id).map_or(0, |t| t.len());
        let to = if end >= len {
            if id + 1 < s.paragraph_count() {
                Pos::new(id + 1, 0)
            } else {
                Pos::new(id, len)
            }
        } else {
            Pos::new(id, end)
        };
        Some((Pos::new(id, start.min(len)), to))
    }
}

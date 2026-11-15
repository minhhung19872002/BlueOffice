//! Slide Sorter view, Reading view and moving slides by dragging their thumbnails (N6,
//! round 2), laid out as the reference presentation app at 125 %: sorter images 400 × 225
//! device pixels (320 logical at 100 % zoom), 46 px apart across and 71 px down, the first
//! one 39 px from the left and 24 px below the ribbon, its number 15 px under the image.
use super::{inside, Drag, Focus, SlideView, ACCENT, ACCENT_TEXT, HANDLE_FILL, R, SLIDE_CANVAS};
use super::{THUMB_EDGE, THUMB_NUMBER};
use crate::{Key, Mods, PageStore, Painter};
use bo_engine::slide_edit::SlideCommand;
use bo_render::Rgba;

/// Image keys of the sorter's slide images (by slide id).
pub(super) const SORT_KEY: usize = 5 << 40;

/// How the editing area shows the presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Thumbnails, slide and notes.
    Normal,
    /// A grid of every slide.
    Sorter,
    /// The slide show inside the window.
    Reading,
}

/// Insertion line of a slide being dragged.
const DROP: Rgba = Rgba(0x44, 0x44, 0x44, 255);

impl SlideView {
    /// The current view.
    pub fn mode(&self) -> ViewMode {
        if self.reading {
            ViewMode::Reading
        } else if self.sorter {
            ViewMode::Sorter
        } else {
            ViewMode::Normal
        }
    }

    /// Switch views (Reading view plays the show inside the window).
    pub fn set_mode(&mut self, mode: ViewMode) -> bool {
        if mode == self.mode() {
            return false;
        }
        if self.reading {
            self.reading = false;
            self.end_show();
        }
        if self.text_edit().is_some() {
            self.run(SlideCommand::TextExit);
        }
        self.drag = None;
        self.sorter = mode == ViewMode::Sorter;
        self.slide_sel = vec![self.current];
        if mode == ViewMode::Sorter {
            self.focus = Focus::Thumbs;
            self.reveal_sorted(self.current);
        }
        if mode == ViewMode::Reading {
            self.reading = true;
            self.start_show(self.current);
        }
        true
    }

    /// Slides chosen in the sorter or the thumbnails pane (at least the current one).
    pub fn selected_slides(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .slide_sel
            .iter()
            .copied()
            .filter(|i| *i < self.ids.len())
            .collect();
        if v.is_empty() {
            v.push(self.current);
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Sorter image size and pitch (device pixels) and the number of columns.
    pub(super) fn sort_geom(&self) -> (f32, f32, f32, f32, usize) {
        let w = self.px(320.0);
        let h = (w * self.slide_size.1 / self.slide_size.0).round();
        let (gx, gy) = (self.px(36.8), self.px(56.8));
        let avail = self.size.0 - self.px(31.2) - self.sb_w();
        let cols = (((avail - w) / (w + gx)).floor() as usize + 1).max(1);
        (w, h, w + gx, h + gy, cols)
    }

    /// Image rectangle of slide `i` in the sorter (window pixels).
    pub fn sort_rect(&self, i: usize) -> R {
        let (w, h, px_, py_, cols) = self.sort_geom();
        let (c, r) = (i % cols, i / cols);
        [
            self.origin.0 + self.px(31.2) + c as f32 * px_,
            self.origin.1 + self.px(19.2) + r as f32 * py_ - self.sorter_scroll,
            w,
            h,
        ]
    }

    pub(super) fn sort_content_h(&self) -> f32 {
        let (_, _, _, py, cols) = self.sort_geom();
        self.px(19.2) + self.ids.len().div_ceil(cols) as f32 * py
    }

    pub(super) fn clamp_sorter_scroll(&mut self) {
        let max = (self.sort_content_h() - self.size.1).max(0.0);
        self.sorter_scroll = self.sorter_scroll.clamp(0.0, max).round();
    }

    pub(super) fn reveal_sorted(&mut self, i: usize) {
        let [_, y, _, h] = self.sort_rect(i);
        let top = self.origin.1;
        if y < top {
            self.sorter_scroll -= top - y + self.px(19.2);
        } else if y + h + self.px(40.0) > top + self.size.1 {
            self.sorter_scroll += y + h + self.px(40.0) - top - self.size.1;
        }
        self.clamp_sorter_scroll();
    }

    pub(super) fn visible_sorted(&self) -> std::ops::Range<usize> {
        let (_, _, _, py, cols) = self.sort_geom();
        let first_row = ((self.sorter_scroll - self.px(19.2)) / py).floor().max(0.0) as usize;
        let rows = (self.size.1 / py).ceil() as usize + 2;
        let n = self.ids.len();
        (first_row * cols).min(n)..((first_row + rows) * cols).min(n)
    }

    /// Slide under a window position in the sorter.
    pub(super) fn sort_at(&self, x: f32, y: f32) -> Option<usize> {
        self.visible_sorted()
            .find(|&i| inside(self.sort_rect(i), x, y))
    }

    /// Where a dragged slide would go (insertion index 0..=n) in the sorter.
    pub(super) fn sort_drop_at(&self, x: f32, y: f32) -> usize {
        let n = self.ids.len();
        let (w, _, _, _, _) = self.sort_geom();
        (0..n)
            .find(|&i| {
                let [rx, ry, _, rh] = self.sort_rect(i);
                y < ry + rh + self.px(40.0) && (y < ry - self.px(20.0) || x < rx + w / 2.0)
            })
            .unwrap_or(n)
    }

    /// Where a dragged slide would go in the thumbnails pane.
    pub(super) fn thumb_drop_at(&self, y: f32) -> usize {
        let n = self.ids.len();
        (0..n)
            .find(|&i| {
                let [_, ty, _, th] = self.thumb_rect(i);
                y < ty + th / 2.0
            })
            .unwrap_or(n)
    }

    /// Choose slide `i` by a click (Shift: extend from the current one).
    pub(super) fn pick_slide(&mut self, i: usize, shift: bool) -> bool {
        if shift {
            let (a, b) = (self.current.min(i), self.current.max(i));
            self.slide_sel = (a..=b).collect();
            return true;
        }
        if !self.slide_sel.contains(&i) || self.slide_sel.len() < 2 {
            self.slide_sel = vec![i];
        }
        self.go_to(i);
        true
    }

    /// A press on a thumbnail (pane or sorter): choose it, ready to drag.
    pub(super) fn press_slide(&mut self, i: usize, x: f32, y: f32, count: u8, shift: bool) -> bool {
        self.focus = Focus::Thumbs;
        if count >= 2 && self.sorter {
            // Double click: open the slide in Normal view.
            self.go_to(i);
            return self.set_mode(ViewMode::Normal);
        }
        self.pick_slide(i, shift);
        self.drag = Some(Drag::Slides {
            from: (x, y),
            moved: false,
            to: None,
        });
        true
    }

    /// Dragging chosen slides: the insertion point follows the pointer.
    pub(super) fn drag_slides(&mut self, x: f32, y: f32) -> bool {
        let Some(Drag::Slides { from, moved, .. }) = self.drag else {
            return false;
        };
        if !moved && (x - from.0).hypot(y - from.1) < super::DRAG_SLOP * self.display_scale * 2.0 {
            return false;
        }
        let to = if self.sorter {
            self.sort_drop_at(x, y)
        } else {
            self.thumb_drop_at(y)
        };
        self.drag = Some(Drag::Slides {
            from,
            moved: true,
            to: Some(to),
        });
        true
    }

    /// Drop dragged slides before slide `to` (one undo step).
    pub(super) fn drop_slides(&mut self, to: usize) -> bool {
        let sel = self.selected_slides();
        let n = self.ids.len();
        let before = sel.iter().filter(|i| **i < to).count();
        let at = to - before;
        let mut order: Vec<usize> = (0..n).filter(|i| !sel.contains(i)).collect();
        for (k, s) in sel.iter().enumerate() {
            order.insert((at + k).min(order.len()), *s);
        }
        if order.iter().copied().eq(0..n) {
            return false;
        }
        let since = self
            .engine
            .slide_command(SlideCommand::HistoryLen)
            .ok()
            .and_then(|r| r.history)
            .unwrap_or(0);
        let mut cur: Vec<usize> = (0..n).collect();
        for (pos, want) in order.iter().enumerate() {
            let Some(at) = cur.iter().position(|c| c == want) else {
                continue;
            };
            if at != pos {
                self.run(SlideCommand::MoveSlide { from: at, to: pos });
                let v = cur.remove(at);
                cur.insert(pos, v);
            }
        }
        self.run(SlideCommand::MergeHistory { since });
        // The moved slides stay chosen at their new places.
        self.slide_sel = (at..at + sel.len()).collect();
        self.go_to(at);
        true
    }

    /// Keys in the sorter.
    pub(super) fn sorter_key(&mut self, key: Key, m: Mods) -> bool {
        let (_, _, _, _, cols) = self.sort_geom();
        let n = self.ids.len();
        let cur = self.current;
        let to = match key {
            Key::Left => cur.saturating_sub(1),
            Key::Right => (cur + 1).min(n.saturating_sub(1)),
            Key::Up => cur.saturating_sub(cols),
            Key::Down => (cur + cols).min(n.saturating_sub(1)),
            Key::Home => 0,
            Key::End => n.saturating_sub(1),
            Key::Enter => return self.set_mode(ViewMode::Normal),
            Key::Delete | Key::Backspace => return self.delete_slides(),
            Key::Letter('a') if m.ctrl => {
                self.slide_sel = (0..n).collect();
                return true;
            }
            _ => return false,
        };
        self.pick_slide(to, m.shift);
        self.reveal_sorted(to);
        true
    }

    /// Delete the chosen slides (one undo step); the last slide stays.
    pub(super) fn delete_slides(&mut self) -> bool {
        let sel = self.selected_slides();
        if sel.len() >= self.ids.len() {
            return false;
        }
        let since = self
            .engine
            .slide_command(SlideCommand::HistoryLen)
            .ok()
            .and_then(|r| r.history)
            .unwrap_or(0);
        for i in sel.iter().rev() {
            self.run(SlideCommand::DeleteSlide { index: Some(*i) });
        }
        self.run(SlideCommand::MergeHistory { since });
        let keep = sel[0].min(self.ids.len().saturating_sub(1));
        self.slide_sel = vec![keep];
        self.go_to(keep);
        true
    }

    pub(super) fn sorter_wheel(&mut self, dy: f32) -> bool {
        let before = self.sorter_scroll;
        self.sorter_scroll -= dy * self.px(60.0);
        self.clamp_sorter_scroll();
        (before - self.sorter_scroll).abs() > 0.5
    }

    /// Bring the visible sorter images up to date.
    pub(super) fn prepare_sorter(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let (w, _, _, _, _) = self.sort_geom();
        let scale = w / self.slide_size.0;
        for i in self.visible_sorted() {
            let key = SORT_KEY + self.ids[i];
            self.ensure(store, key, i, scale)?;
        }
        Ok(())
    }

    /// The sorter: every slide with its number, the chosen ones framed in the accent.
    pub(super) fn paint_sorter(&self, p: &mut dyn Painter) {
        let (ox, oy) = self.origin;
        let (w, h) = self.size;
        p.fill_rect(ox, oy, w, h, SLIDE_CANVAS);
        let one = self.display_scale.round().max(1.0);
        let frame = self.px(2.4).max(2.0);
        let chosen = self.selected_slides();
        for i in self.visible_sorted() {
            let [x, y, tw, th] = self.sort_rect(i);
            let key = SORT_KEY + self.ids[i];
            let on = chosen.contains(&i);
            if on {
                let o = one + frame;
                p.fill_rect(x - o, y - o, tw + 2.0 * o, th + 2.0 * o, ACCENT);
                p.fill_rect(
                    x - one,
                    y - one,
                    tw + 2.0 * one,
                    th + 2.0 * one,
                    SLIDE_CANVAS,
                );
            } else {
                p.fill_rect(x - one, y - one, tw + 2.0 * one, th + 2.0 * one, THUMB_EDGE);
            }
            if self.shown.contains_key(&key) {
                p.page(key, x as i32, y as i32);
            } else {
                p.fill_rect(x, y, tw, th, HANDLE_FILL);
            }
            let color = if on { ACCENT_TEXT } else { THUMB_NUMBER };
            // Number under the image's left edge, its ink 15 px below (reference).
            p.text(
                &(i + 1).to_string(),
                x.round(),
                (y + th + self.px(4.8)).round(),
                self.number_style(),
                color,
            );
        }
        if let Some(Drag::Slides { to: Some(t), .. }) = self.drag {
            // Insertion bar before slide `t` (after the last one at the end).
            let (r, after) = if t < self.ids.len() {
                (self.sort_rect(t), false)
            } else {
                (self.sort_rect(self.ids.len().saturating_sub(1)), true)
            };
            let bx = if after {
                r[0] + r[2] + self.px(16.0)
            } else {
                r[0] - self.px(20.0)
            };
            p.fill_rect(bx.round(), r[1], self.px(2.4).max(2.0), r[3], DROP);
        }
    }

    /// Insertion line of a slide dragged in the thumbnails pane.
    pub(super) fn paint_thumb_drop(&self, p: &mut dyn Painter) {
        let Some(Drag::Slides { to: Some(t), .. }) = self.drag else {
            return;
        };
        let n = self.ids.len();
        let y = if t < n {
            self.thumb_rect(t)[1] - self.px(10.8)
        } else {
            let r = self.thumb_rect(n.saturating_sub(1));
            r[1] + r[3] + self.px(10.8)
        };
        let [x, _, w, _] = self.thumb_rect(0);
        p.fill_rect(x, y.round(), w, self.px(2.4).max(2.0), DROP);
    }
}

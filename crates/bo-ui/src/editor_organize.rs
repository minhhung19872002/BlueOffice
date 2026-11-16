//! BlueEditor page grid (Organize tab): page thumbnails in rows, selection (click, Shift+click,
//! arrows, Ctrl+A), drag to move pages with a drop marker, wheel scrolling. The window applies
//! the page edits; this module only lays out, hit tests and paints.
use crate::reader_view::{ReaderView, THUMB_KEY};
use crate::theme::Theme;
use crate::{Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use std::collections::BTreeSet;

const CANVAS: Rgba = Rgba(240, 240, 240, 255);
const PAGE_EDGE: Rgba = Rgba(119, 120, 122, 255);
const TEXT: Rgba = Rgba(51, 51, 51, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// Thumbnail box (logical pixels at scale 1): the page fits a square of this size.
const THUMB: f32 = 150.0;
/// Space around a thumbnail inside its cell, and the label height below it.
const PAD: f32 = 14.0;
const LABEL: f32 = 26.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    from: (f32, f32),
    moved: bool,
    /// A click without moving keeps only this page selected (pressed inside a selection).
    collapse: Option<usize>,
    /// Insertion index under the pointer.
    to: Option<usize>,
}

/// Page grid state.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Organize {
    /// Selected page indices.
    pub selected: BTreeSet<usize>,
    anchor: usize,
    scroll: f32,
    /// Grid area in window pixels.
    pub frame: [f32; 4],
    /// Display scale.
    pub scale: f32,
    /// Cell rectangles in content pixels (scroll applied when drawn).
    cells: Vec<[f32; 4]>,
    columns: usize,
    content_h: f32,
    drag: Option<Drag>,
    /// Thumbnail size factor (1 = 150 logical pixels; 0 before the first change).
    zoom: f32,
    /// Zoom slider track (window pixels, from the last paint) and the knob is dragged.
    pub(crate) slider: [f32; 4],
    pub sliding: bool,
}

/// Thumbnail size range of the zoom slider.
pub(crate) const ZOOM_MIN: f32 = 0.5;
pub(crate) const ZOOM_MAX: f32 = 2.5;

impl Organize {
    fn px(&self, v: f32) -> f32 {
        (v * self.scale.max(0.5)).round()
    }

    /// Thumbnail size factor.
    pub fn zoom(&self) -> f32 {
        if self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        }
    }

    /// Set the thumbnail size factor (kept within the slider's range).
    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    fn thumb(&self) -> f32 {
        self.px(THUMB * self.zoom())
    }

    /// The zoom slider is under a window point: set the zoom from it and start sliding.
    pub fn slider_press(&mut self, x: f32, y: f32) -> bool {
        let s = self.slider;
        let pad = self.px(6.0);
        if s[2] <= 0.0
            || !inside(
                [s[0] - pad, s[1] - pad, s[2] + 2.0 * pad, s[3] + 2.0 * pad],
                x,
                y,
            )
        {
            return false;
        }
        self.sliding = true;
        self.slide_to(x);
        true
    }

    /// The slider knob follows the pointer.
    pub fn slide_to(&mut self, x: f32) {
        let s = self.slider;
        let t = ((x - s[0]) / s[2].max(1.0)).clamp(0.0, 1.0);
        self.set_zoom(ZOOM_MIN + t * (ZOOM_MAX - ZOOM_MIN));
    }

    /// The pointer moved far enough for the press to be a drag.
    pub fn drag_moved(&self) -> bool {
        self.drag.is_some_and(|d| d.moved)
    }

    /// Forget the drag (it was dropped somewhere else).
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    fn cell_size(&self) -> (f32, f32) {
        let t = self.thumb();
        (
            t + 2.0 * self.px(PAD),
            t + 2.0 * self.px(PAD) + self.px(LABEL),
        )
    }

    /// Lay the cells out for pages of `sizes` (points).
    pub fn layout(&mut self, sizes: &[(f32, f32)]) {
        let (cw, ch) = self.cell_size();
        let gap = self.px(8.0);
        let avail = (self.frame[2] - 2.0 * gap).max(cw);
        let cols = ((avail + gap) / (cw + gap)).floor().max(1.0) as usize;
        // Centre the rows.
        let used = cols as f32 * cw + (cols - 1) as f32 * gap;
        let x0 = ((self.frame[2] - used) / 2.0).max(gap).round();
        self.columns = cols;
        self.cells = (0..sizes.len())
            .map(|i| {
                let (c, r) = (i % cols, i / cols);
                [
                    x0 + c as f32 * (cw + gap),
                    gap + r as f32 * (ch + gap),
                    cw,
                    ch,
                ]
            })
            .collect();
        let rows = sizes.len().div_ceil(cols);
        self.content_h = gap + rows as f32 * (ch + gap);
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max = (self.content_h - self.frame[3]).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    /// Scroll by `dy` pixels.
    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll += dy;
        self.clamp_scroll();
    }

    /// Cell `i` in window pixels.
    pub(crate) fn cell(&self, i: usize) -> Option<[f32; 4]> {
        let c = self.cells.get(i)?;
        Some([
            self.frame[0] + c[0],
            self.frame[1] + c[1] - self.scroll,
            c[2],
            c[3],
        ])
    }

    /// Thumbnail rectangle of page `i` (size `pt`) in window pixels and its scale.
    fn thumb_rect(&self, i: usize, pt: (f32, f32)) -> Option<([f32; 4], f32)> {
        let c = self.cell(i)?;
        let t = self.thumb();
        let s = t / pt.0.max(pt.1).max(1.0);
        let (w, h) = ((pt.0 * s).round(), (pt.1 * s).round());
        let x = (c[0] + (c[2] - w) / 2.0).round();
        let y = (c[1] + self.px(PAD) + (t - h)).round();
        Some(([x, y, w, h], s))
    }

    /// Pages whose cells show, with the thumbnail scale (device pixels per point).
    pub fn visible_thumbs(&self, sizes: &[(f32, f32)]) -> Vec<(usize, f32)> {
        let top = self.frame[1];
        let bottom = top + self.frame[3];
        (0..self.cells.len())
            .filter_map(|i| {
                let c = self.cell(i)?;
                if c[1] + c[3] < top || c[1] > bottom {
                    return None;
                }
                let (_, s) = self.thumb_rect(i, *sizes.get(i)?)?;
                Some((i, s))
            })
            .collect()
    }

    /// Page under a window point.
    pub fn page_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.cells.len()).find(|&i| self.cell(i).is_some_and(|c| inside(c, x, y)))
    }

    /// Insertion index for a drop at a window point: before the cell under it when on its
    /// left half, after it otherwise; past the end below the last row.
    fn drop_index(&self, x: f32, y: f32) -> Option<usize> {
        if !inside(self.frame, x, y) && self.cells.is_empty() {
            return None;
        }
        let n = self.cells.len();
        let row_of = |i: usize| self.cell(i).map_or(0.0, |c| c[1]);
        // The row under the pointer (or the nearest).
        let mut best: Option<(usize, f32)> = None;
        for i in 0..n {
            let Some(c) = self.cell(i) else { continue };
            let dy = if y < c[1] {
                c[1] - y
            } else if y > c[1] + c[3] {
                y - c[1] - c[3]
            } else {
                0.0
            };
            let dx = if x < c[0] {
                c[0] - x
            } else if x > c[0] + c[2] {
                x - c[0] - c[2]
            } else {
                0.0
            };
            let d = dy * 4.0 + dx;
            if best.is_none_or(|b| d < b.1) {
                best = Some((i, d));
            }
        }
        let (i, _) = best?;
        let c = self.cell(i)?;
        let after = x > c[0] + c[2] / 2.0;
        let _ = row_of;
        Some(if after { i + 1 } else { i })
    }

    /// Press on the grid: select (Shift extends from the anchor) and start a drag.
    pub fn press(&mut self, x: f32, y: f32, shift: bool, ctrl: bool) -> Option<usize> {
        let Some(i) = self.page_at(x, y) else {
            if !ctrl && !shift {
                self.selected.clear();
            }
            return None;
        };
        let collapse =
            (!shift && !ctrl && self.selected.len() > 1 && self.selected.contains(&i)).then_some(i);
        if ctrl && !shift {
            // Ctrl+click adds or removes one page.
            if !self.selected.remove(&i) {
                self.selected.insert(i);
            }
            self.anchor = i;
            if !self.selected.contains(&i) {
                return Some(i);
            }
        } else if shift {
            let (a, b) = (self.anchor.min(i), self.anchor.max(i));
            self.selected = (a..=b).collect();
        } else if !self.selected.contains(&i) {
            self.selected.clear();
            self.selected.insert(i);
            self.anchor = i;
        }
        self.drag = Some(Drag {
            from: (x, y),
            moved: false,
            collapse,
            to: None,
        });
        Some(i)
    }

    /// A drag is in progress (button down after a press on a page).
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Pointer moved with the button down.
    pub fn drag_to(&mut self, x: f32, y: f32) {
        let threshold = self.px(5.0);
        let to = self.drop_index(x, y);
        let edge = self.px(30.0);
        if y < self.frame[1] + edge {
            self.scroll_by(-self.px(12.0));
        } else if y > self.frame[1] + self.frame[3] - edge {
            self.scroll_by(self.px(12.0));
        }
        if let Some(d) = self.drag.as_mut() {
            if (x - d.from.0).abs() + (y - d.from.1).abs() > threshold {
                d.moved = true;
            }
            d.to = to;
        }
    }

    /// Button released: the pages to move and the insertion index when it was a drag that
    /// changes the order. A click without moving selects only the page under the pointer.
    pub fn release(&mut self, x: f32, y: f32) -> Option<(Vec<usize>, usize)> {
        let d = self.drag.take()?;
        if !d.moved {
            if let Some(i) = d.collapse.filter(|&i| self.page_at(x, y) == Some(i)) {
                self.select_range(i, i);
            }
            return None;
        }
        let to = d.to?;
        let pages: Vec<usize> = self.selected.iter().copied().collect();
        // Dropping inside the selected block changes nothing.
        let first = *pages.first()?;
        let last = *pages.last()?;
        let contiguous = last - first + 1 == pages.len();
        if contiguous && to >= first && to <= last + 1 {
            return None;
        }
        Some((pages, to))
    }

    /// Select pages `a..=b` (anchor `a`).
    pub fn select_range(&mut self, a: usize, b: usize) {
        self.selected = (a.min(b)..=a.max(b)).collect();
        self.anchor = a;
    }

    /// Keep the selection within `n` pages.
    pub fn clamp(&mut self, n: usize) {
        self.selected.retain(|&i| i < n);
        if self.selected.is_empty() && n > 0 {
            let i = self.anchor.min(n - 1);
            self.selected.insert(i);
            self.anchor = i;
        }
    }

    /// Arrow keys move the selection (Shift extends it); Home / End go to the ends.
    pub fn step(&mut self, k: Key, shift: bool, n: usize) {
        if n == 0 {
            return;
        }
        let cur = self.selected.iter().next_back().copied().unwrap_or(0);
        let cols = self.columns.max(1);
        let next = match k {
            Key::Left => cur.saturating_sub(1),
            Key::Right => (cur + 1).min(n - 1),
            Key::Up => cur.saturating_sub(cols),
            Key::Down => (cur + cols).min(n - 1),
            Key::Home => 0,
            _ => n - 1,
        };
        if shift {
            let a = self.anchor;
            self.selected = (a.min(next)..=a.max(next)).collect();
        } else {
            self.select_range(next, next);
        }
        // Keep it in view.
        if let Some(c) = self.cell(next) {
            if c[1] < self.frame[1] {
                self.scroll_by(c[1] - self.frame[1]);
            } else if c[1] + c[3] > self.frame[1] + self.frame[3] {
                self.scroll_by(c[1] + c[3] - self.frame[1] - self.frame[3]);
            }
        }
    }

    /// Paint the grid over the document area.
    pub fn paint(
        &mut self,
        p: &mut dyn Painter,
        v: &ReaderView,
        key_base: usize,
        labels: &[String],
        theme: &Theme,
    ) {
        let f = self.frame;
        p.fill_rect(f[0], f[1], f[2], f[3], CANVAS);
        p.push_clip(f[0], f[1], f[2], f[3]);
        let st = TextStyle {
            size: 14.0 / 1.375 * self.scale,
            bold: false,
        };
        let tint = Rgba(theme.accent.0, theme.accent.1, theme.accent.2, 36);
        for i in 0..self.cells.len() {
            let Some(c) = self.cell(i) else { continue };
            if c[1] + c[3] < f[1] || c[1] > f[1] + f[3] {
                continue;
            }
            let selected = self.selected.contains(&i);
            if selected {
                p.path(
                    &rounded(c[0], c[1], c[2], c[3], self.px(4.0)),
                    Some(tint),
                    None,
                );
            }
            let Some((r, _)) = self.thumb_rect(i, v.page_size(i)) else {
                continue;
            };
            let edge = if selected { theme.accent } else { PAGE_EDGE };
            let bw = if selected { self.px(2.0) } else { 1.0 };
            p.fill_rect(r[0] - bw, r[1] - bw, r[2] + 2.0 * bw, r[3] + 2.0 * bw, edge);
            p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
            if v.has_image(THUMB_KEY + i) {
                p.page(key_base + THUMB_KEY + i, r[0] as i32, r[1] as i32);
            }
            let label = labels.get(i).cloned().unwrap_or_default();
            let lw = label.chars().count() as f32 * st.size * 0.55;
            p.text(
                &label,
                (c[0] + (c[2] - lw) / 2.0).round(),
                r[1] + r[3] + self.px(7.0),
                st,
                if selected { theme.accent_text } else { TEXT },
            );
        }
        // Drop marker.
        if let Some(Drag {
            moved: true,
            to: Some(to),
            ..
        }) = self.drag
        {
            let (cell, right) = if to < self.cells.len() {
                (self.cell(to), false)
            } else {
                (self.cell(to.saturating_sub(1)), true)
            };
            if let Some(c) = cell {
                let gap = self.px(8.0);
                let x = if right {
                    c[0] + c[2] + gap / 2.0
                } else {
                    c[0] - gap / 2.0
                };
                let w = self.px(3.0);
                p.fill_rect((x - w / 2.0).round(), c[1], w, c[3], theme.accent);
            }
        }
        p.pop_clip();
        self.paint_slider(p, theme);
        // Scroll thumb.
        if self.content_h > f[3] {
            let track = f[3];
            let th = (track * f[3] / self.content_h).max(self.px(24.0));
            let max = (self.content_h - f[3]).max(1.0);
            let y = f[1] + (track - th) * (self.scroll / max);
            let w = self.px(4.0);
            p.fill_rect(
                f[0] + f[2] - w - self.px(4.0),
                y,
                w,
                th,
                Rgba(133, 133, 133, 255),
            );
        }
    }
}

impl Organize {
    /// Thumbnail size slider at the grid's bottom right: a track with minus and plus marks
    /// and a round knob, as the view zoom slider of the status bar.
    fn paint_slider(&mut self, p: &mut dyn Painter, theme: &Theme) {
        let f = self.frame;
        let w = self.px(120.0);
        let (x, y) = (f[0] + f[2] - w - self.px(32.0), f[1] + f[3] - self.px(22.0));
        self.slider = [x, y, w, 1.0];
        let back = [
            x - self.px(22.0),
            y - self.px(12.0),
            w + self.px(44.0),
            self.px(24.0),
        ];
        p.path(
            &rounded(back[0], back[1], back[2], back[3], self.px(4.0)),
            Some(Rgba(255, 255, 255, 230)),
            Some((Rgba(204, 204, 204, 255), 1.0)),
        );
        let ink = Rgba(102, 102, 102, 255);
        p.fill_rect(x, y.round(), w, 1.0, Rgba(160, 160, 160, 255));
        let m = self.px(4.0);
        let (lx, rx) = (x - self.px(12.0), x + w + self.px(12.0));
        p.fill_rect(lx - m, y.round(), 2.0 * m + 1.0, 1.0, ink);
        p.fill_rect(rx - m, y.round(), 2.0 * m + 1.0, 1.0, ink);
        p.fill_rect(rx, y.round() - m, 1.0, 2.0 * m + 1.0, ink);
        let t = (self.zoom() - ZOOM_MIN) / (ZOOM_MAX - ZOOM_MIN);
        let kx = x + t * w;
        let r = self.px(5.0);
        p.path(
            &rounded(kx - r, y - r, 2.0 * r, 2.0 * r, r),
            Some(Rgba(255, 255, 255, 255)),
            Some((if self.sliding { theme.accent } else { ink }, 1.0)),
        );
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<PathSeg> {
    let r = r.min(w / 2.0).min(h / 2.0);
    vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::Q(x + w, y, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::Q(x + w, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::Q(x, y + h, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::Q(x, y, x + r, y),
        PathSeg::Z,
    ]
}

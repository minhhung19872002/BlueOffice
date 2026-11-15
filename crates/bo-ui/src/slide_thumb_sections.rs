//! Sections in the thumbnails pane (N6, round 5): a header row before each section's slides
//! ([MS-PPTX] `p14:sectionLst`, read by the engine as `SectionOut`) with a disclosure arrow
//! and the section's name; a collapsed section hides its slides and shows their count, as
//! the reference presentation app's slide pane does.
use super::{SlideView, R, THUMB_NUMBER};
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

impl SlideView {
    fn section_header_h(&self) -> f32 {
        self.px(28.0)
    }

    /// Sections of the deck (none outside the normal slide list).
    pub fn sections(&self) -> &[bo_engine::slide_edit::sections::SectionOut] {
        self.state.as_ref().map_or(&[], |s| s.sections.as_slice())
    }

    /// Content y of each thumbnail (`None`: in a collapsed section) and of each section
    /// header, from the top of the pane's content.
    fn thumb_layout(&self) -> (Vec<Option<f32>>, Vec<f32>, f32) {
        let n = self.ids.len();
        let pitch = self.thumb_pitch();
        let sections = self.sections();
        if sections.is_empty() {
            let ys = (0..n)
                .map(|i| Some(self.px(20.0) + i as f32 * pitch))
                .collect();
            return (ys, Vec::new(), self.px(20.0) + n as f32 * pitch);
        }
        let mut ys = vec![None; n];
        let mut heads = Vec::with_capacity(sections.len());
        let mut y = self.px(6.0);
        let mut next = 0;
        for s in sections {
            // Slides before the first section (a damaged list) keep their rows.
            while next < s.first.min(n) {
                ys[next] = Some(y + self.px(14.0));
                y += pitch;
                next += 1;
            }
            heads.push(y);
            y += self.section_header_h();
            let collapsed = self.collapsed.contains(&s.id);
            for slot in ys.iter_mut().skip(s.first).take(s.count) {
                if !collapsed {
                    *slot = Some(y + self.px(14.0));
                    y += pitch;
                }
            }
            next = (s.first + s.count).max(next);
        }
        while next < n {
            ys[next] = Some(y + self.px(14.0));
            y += pitch;
            next += 1;
        }
        (ys, heads, y + self.px(14.0))
    }

    /// Content y of thumbnail `i` and whether it is hidden in a collapsed section.
    pub(super) fn thumb_y(&self, i: usize) -> (f32, bool) {
        if self.sections().is_empty() {
            return (self.px(20.0) + i as f32 * self.thumb_pitch(), false);
        }
        let (ys, heads, _) = self.thumb_layout();
        if let Some(y) = ys.get(i).copied().flatten() {
            return (y, false);
        }
        // Hidden: at its section's header.
        let s = self
            .sections()
            .iter()
            .position(|s| i >= s.first && i < s.first + s.count)
            .unwrap_or(0);
        (heads.get(s).copied().unwrap_or(0.0), true)
    }

    /// Whether thumbnail `i` is hidden in a collapsed section.
    pub(super) fn thumb_hidden(&self, i: usize) -> bool {
        !self.sections().is_empty() && self.thumb_y(i).1
    }

    /// Height of the pane's content.
    pub(super) fn thumbs_height(&self) -> f32 {
        if self.sections().is_empty() {
            return self.px(20.0) + self.ids.len() as f32 * self.thumb_pitch();
        }
        self.thumb_layout().2
    }

    /// Indices of the thumbnails in view (hidden ones in between included).
    pub(super) fn thumbs_in_view(&self) -> std::ops::Range<usize> {
        let n = self.ids.len();
        if self.sections().is_empty() {
            let pitch = self.thumb_pitch().max(1.0);
            let top = self.thumb_scroll - self.px(20.0);
            let first = (top / pitch).floor().max(0.0) as usize;
            let last = ((top + self.size.1) / pitch).ceil().max(0.0) as usize + 1;
            return first.min(n)..last.min(n);
        }
        let (ys, _, _) = self.thumb_layout();
        let h = self.slide_px(self.thumb_scale()).1;
        let (top, bottom) = (self.thumb_scroll, self.thumb_scroll + self.size.1);
        let shown: Vec<usize> = (0..n)
            .filter(|&i| ys[i].is_some_and(|y| y + h >= top && y <= bottom))
            .collect();
        match (shown.first(), shown.last()) {
            (Some(&a), Some(&b)) => a..b + 1,
            _ => 0..0,
        }
    }

    /// Section header rows in window pixels: (section index, rectangle).
    pub fn section_rows(&self) -> Vec<(usize, R)> {
        if self.sections().is_empty() || self.sorter {
            return Vec::new();
        }
        let (_, heads, _) = self.thumb_layout();
        let r = self.thumbs_rect();
        heads
            .into_iter()
            .enumerate()
            .map(|(i, y)| {
                (
                    i,
                    [
                        r[0],
                        self.origin.1 + y - self.thumb_scroll,
                        r[2],
                        self.section_header_h(),
                    ],
                )
            })
            .collect()
    }

    /// The section header at a window position.
    pub fn section_at(&self, x: f32, y: f32) -> Option<usize> {
        self.section_rows()
            .into_iter()
            .find(|(_, r)| crate::spin_box::inside(*r, x, y))
            .map(|(i, _)| i)
    }

    /// Collapse or expand section `i`.
    pub fn toggle_section(&mut self, i: usize) -> bool {
        let Some(id) = self.sections().get(i).map(|s| s.id.clone()) else {
            return false;
        };
        if let Some(k) = self.collapsed.iter().position(|c| *c == id) {
            self.collapsed.remove(k);
        } else {
            self.collapsed.push(id);
        }
        self.clamp_thumb_scroll();
        true
    }

    /// Collapse (or expand) every section.
    pub fn collapse_all(&mut self, collapse: bool) -> bool {
        self.collapsed = if collapse {
            self.sections().iter().map(|s| s.id.clone()).collect()
        } else {
            Vec::new()
        };
        self.clamp_thumb_scroll();
        true
    }

    /// Whether section `i` is collapsed.
    pub fn section_collapsed(&self, i: usize) -> bool {
        self.sections()
            .get(i)
            .is_some_and(|s| self.collapsed.contains(&s.id))
    }

    /// Index of the section holding the current slide.
    pub fn current_section(&self) -> Option<usize> {
        let cur = self.current;
        self.sections()
            .iter()
            .position(|s| cur >= s.first && cur < s.first + s.count)
            .or_else(|| (!self.sections().is_empty()).then_some(0))
    }

    /// Section headers: a disclosure arrow, the name, the slide count when collapsed.
    pub(super) fn paint_section_rows(&self, p: &mut dyn Painter) {
        let rows = self.section_rows();
        if rows.is_empty() {
            return;
        }
        let style = TextStyle {
            size: self.number_style().size,
            bold: false,
        };
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        for (i, [x, y, w, h]) in rows {
            let Some(s) = self.sections().get(i) else {
                continue;
            };
            let collapsed = self.collapsed.contains(&s.id);
            if i == self.current_section().unwrap_or(usize::MAX) {
                p.fill_rect(x, y, w, h, Rgba(0xe1, 0xe1, 0xe1, 255));
            }
            let (ax, ay) = (x + self.px(14.0), y + h / 2.0);
            let k = self.px(4.0);
            let tri = if collapsed {
                vec![
                    PathSeg::M(ax - k / 2.0, ay - k),
                    PathSeg::L(ax + k / 2.0, ay),
                    PathSeg::L(ax - k / 2.0, ay + k),
                    PathSeg::Z,
                ]
            } else {
                vec![
                    PathSeg::M(ax - k, ay - k / 2.0),
                    PathSeg::L(ax + k, ay - k / 2.0),
                    PathSeg::L(ax, ay + k / 2.0),
                    PathSeg::Z,
                ]
            };
            p.path(&tri, Some(THUMB_NUMBER), None);
            let label = if collapsed {
                format!("{} ({})", s.name, s.count)
            } else {
                s.name.clone()
            };
            p.text(
                &label,
                x + self.px(26.0),
                (y + (h - style.size * 1.33) / 2.0).round(),
                style,
                ink,
            );
        }
    }
}

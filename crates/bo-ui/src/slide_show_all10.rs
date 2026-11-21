//! "See All Slides" while presenting (round 10 resume of the slide series), measured on the
//! reference presentation app at 1920 × 1080 (G, `-` or the right-click menu): a grid of the
//! slides on a near-black ground (26, 26, 26), each image a sixth of the screen wide with
//! 42 px gaps from 42 px in and 104 px down, the number under its left edge in grey
//! (168, 168, 168), the current slide in a 3 px accent frame one pixel off the image with its
//! number in the accent colour, and a round back button (44 px, grey ring and arrow) at the
//! top left. Arrows move the choice, Enter or a click shows that slide, Esc / G / the back
//! button go back to the slide shown before.
use super::{SlideView, ACCENT, R, THUMB_KEY};
use crate::{Input, Key, PageStore, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Ground of the grid.
const GROUND: Rgba = Rgba(26, 26, 26, 255);
/// Numbers, back button.
const GREY: Rgba = Rgba(168, 168, 168, 255);

/// The grid's state while it is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overview {
    /// The highlighted slide.
    pub sel: usize,
    /// Scrolled rows (device px).
    pub scroll: f32,
}

impl SlideView {
    /// Whether the slide grid is open.
    pub fn show_overview(&self) -> Option<Overview> {
        self.show_all
    }

    /// Open the slide grid on the slide shown now.
    pub fn open_show_overview(&mut self) -> bool {
        if self.show.is_none() {
            return false;
        }
        let sel = self
            .show_slide()
            .unwrap_or(0)
            .min(self.ids.len().saturating_sub(1));
        self.show_all = Some(Overview { sel, scroll: 0.0 });
        self.show_tools.screen = None;
        self.keep_overview_sel_visible();
        true
    }

    /// Grid geometry: (columns, image width, image height, gap, left, top), device px.
    fn overview_grid(&self) -> (usize, f32, f32, f32, f32, f32) {
        let [_, _, w, h] = self.show_area();
        // Sizes relative to the screen as the reference's at 1920 × 1080.
        let k = (w / 1920.0).min(h / 1080.0).max(0.1);
        let tw = (320.0 * k).round().max(8.0);
        let th = (tw * self.slide_size.1 / self.slide_size.0.max(1.0)).round();
        let gap = (42.0 * k).round();
        let left = gap;
        let top = (104.0 * k).round();
        let cols = (((w - left) / (tw + gap)).floor() as usize).max(1);
        (cols, tw, th, gap, left, top)
    }

    /// Height of one grid row (image, number, gap).
    fn overview_row_h(&self) -> f32 {
        let (_, _, th, gap, _, _) = self.overview_grid();
        th + gap + (12.0 * self.display_scale).round()
    }

    /// Rectangle of slide `i`'s image in the grid (window px).
    pub(crate) fn overview_rect(&self, i: usize) -> R {
        let (cols, tw, th, gap, left, top) = self.overview_grid();
        let [ax, ay, ..] = self.show_area();
        let scroll = self.show_all.map_or(0.0, |o| o.scroll);
        let (c, r) = (i % cols, i / cols);
        [
            ax + left + c as f32 * (tw + gap),
            ay + top + r as f32 * self.overview_row_h() - scroll,
            tw,
            th,
        ]
    }

    /// The back button (window px).
    fn overview_back(&self) -> R {
        let [ax, ay, w, h] = self.show_area();
        let k = (w / 1920.0).min(h / 1080.0).max(0.1);
        [
            ax + (25.0 * k).round(),
            ay + (25.0 * k).round(),
            (44.0 * k).round(),
            (44.0 * k).round(),
        ]
    }

    fn keep_overview_sel_visible(&mut self) {
        let Some(o) = self.show_all else { return };
        let [_, ay, _, h] = self.show_area();
        let r = self.overview_rect(o.sel);
        let mut scroll = o.scroll;
        let (.., top) = self.overview_grid();
        if r[1] < ay + top {
            scroll -= ay + top - r[1];
        } else if r[1] + r[3] + top / 2.0 > ay + h {
            scroll += r[1] + r[3] + top / 2.0 - (ay + h);
        }
        if let Some(o) = self.show_all.as_mut() {
            o.scroll = scroll.max(0.0).round();
        }
    }

    fn overview_hit(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.ids.len()).find(|&i| {
            let r = self.overview_rect(i);
            x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
        })
    }

    /// Input while the grid is open; `None` when it is closed.
    pub(super) fn overview_input(&mut self, input: &Input) -> Option<bool> {
        let o = self.show_all?;
        let n = self.ids.len();
        if n == 0 {
            self.show_all = None;
            return Some(true);
        }
        let (cols, ..) = self.overview_grid();
        let moved = |me: &mut Self, sel: usize| {
            if let Some(o) = me.show_all.as_mut() {
                o.sel = sel.min(n - 1);
            }
            me.keep_overview_sel_visible();
            true
        };
        Some(match *input {
            Input::Key(Key::Left, _) => moved(self, o.sel.saturating_sub(1)),
            Input::Key(Key::Right, _) => moved(self, o.sel + 1),
            Input::Key(Key::Up, _) => moved(self, o.sel.saturating_sub(cols)),
            Input::Key(Key::Down, _) => moved(self, o.sel + cols),
            Input::Key(Key::Home, _) => moved(self, 0),
            Input::Key(Key::End, _) => moved(self, n - 1),
            Input::Key(Key::Enter, _) => self.overview_choose(o.sel),
            Input::Key(Key::Escape, _) => self.close_show_overview(),
            Input::Text(ref t) if matches!(t.as_str(), "g" | "G" | "-") => {
                self.close_show_overview()
            }
            Input::Press { x, y, .. } => {
                let b = self.overview_back();
                if x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3] {
                    return Some(self.close_show_overview());
                }
                match self.overview_hit(x, y) {
                    Some(i) => self.overview_choose(i),
                    None => false,
                }
            }
            Input::Wheel(dy) => {
                let rows = n.div_ceil(cols) as f32;
                let [_, _, _, h] = self.show_area();
                let (.., top) = self.overview_grid();
                let max = (top + rows * self.overview_row_h() - h).max(0.0);
                let before = o.scroll;
                if let Some(o) = self.show_all.as_mut() {
                    o.scroll = (o.scroll + dy).clamp(0.0, max).round();
                }
                self.show_all.is_some_and(|o| o.scroll != before)
            }
            _ => false,
        })
    }

    /// Back to the show where it was.
    pub fn close_show_overview(&mut self) -> bool {
        self.show_all.take().is_some()
    }

    fn overview_choose(&mut self, i: usize) -> bool {
        self.show_all = None;
        if self.show_slide() == Some(i) {
            return true;
        }
        self.show_jump(i)
    }

    /// Images of the grid rows in view (thumbnail images at the grid's size).
    pub(super) fn prepare_overview(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let Some(_) = self.show_all else {
            return Ok(());
        };
        let (_, tw, ..) = self.overview_grid();
        let scale = tw / self.slide_size.0.max(1.0);
        let [_, ay, _, h] = self.show_area();
        for i in 0..self.ids.len() {
            let r = self.overview_rect(i);
            if r[1] + r[3] < ay || r[1] > ay + h {
                continue;
            }
            self.ensure(store, THUMB_KEY + self.ids[i], i, scale)?;
        }
        Ok(())
    }

    /// Paint the grid over the whole show area.
    pub(super) fn paint_overview(&self, p: &mut dyn Painter, text_size: f32) {
        let Some(o) = self.show_all else { return };
        let [ax, ay, w, h] = self.show_area();
        p.fill_rect(ax, ay, w, h, GROUND);
        p.push_clip(ax, ay, w, h);
        let k = (w / 1920.0).min(h / 1080.0).max(0.1);
        let frame = (3.0 * k).round().max(2.0);
        let one = 1.0f32.max(k.round());
        let style = TextStyle {
            size: (text_size * 1.45).round(),
            bold: false,
        };
        for i in 0..self.ids.len() {
            let [x, y, tw, th] = self.overview_rect(i);
            if y + th + 40.0 * k < ay || y > ay + h {
                continue;
            }
            let selected = i == o.sel;
            if selected {
                let off = one + frame;
                p.fill_rect(x - off, y - off, tw + 2.0 * off, th + 2.0 * off, ACCENT);
                p.fill_rect(x - one, y - one, tw + 2.0 * one, th + 2.0 * one, GROUND);
            }
            let key = THUMB_KEY + self.ids[i];
            if self.shown.contains_key(&key) {
                p.page(key, x as i32, y as i32);
            } else {
                p.fill_rect(x, y, tw, th, Rgba(255, 255, 255, 255));
            }
            let n = (i + 1).to_string();
            let color = if selected { ACCENT } else { GREY };
            p.text(&n, x, (y + th + 9.0 * k).round(), style, color);
        }
        p.pop_clip();
        // Back button: a grey ring with an arrow pointing left.
        let [bx, by, bw, _] = self.overview_back();
        let (cx, cy, r) = (bx + bw / 2.0, by + bw / 2.0, bw / 2.0 - one);
        let ring: Vec<PathSeg> = (0..=48)
            .map(|s| {
                let a = s as f32 / 48.0 * std::f32::consts::TAU;
                let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
                if s == 0 {
                    PathSeg::M(px, py)
                } else {
                    PathSeg::L(px, py)
                }
            })
            .collect();
        let line = (1.6 * k).max(1.0);
        p.path(&ring, None, Some((GREY, line)));
        let a = r * 0.5;
        let arrow = vec![
            PathSeg::M(cx + a, cy),
            PathSeg::L(cx - a, cy),
            PathSeg::M(cx - a + a * 0.75, cy - a * 0.75),
            PathSeg::L(cx - a, cy),
            PathSeg::L(cx - a + a * 0.75, cy + a * 0.75),
        ];
        p.path(&arrow, None, Some((GREY, (2.2 * k).max(1.0))));
    }
}

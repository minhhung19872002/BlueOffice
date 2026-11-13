//! Drop-down menus (font and size lists) and tooltips, drawn over the window.
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::Rgba;

/// What choosing a menu item does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Set the font family.
    Font(String),
    /// Set the font size in half-points.
    Size(u32),
}

/// One menu row.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Text shown.
    pub label: String,
    /// What it does.
    pub choice: Choice,
    /// Shown as the current value.
    pub checked: bool,
    /// Dimmed (e.g. a font that is not installed).
    pub dim: bool,
}

/// An open drop-down list anchored under a control.
pub struct Menu {
    items: Vec<MenuItem>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    item_h: f32,
    scroll: f32,
    hover: Option<usize>,
    pressed: bool,
    scale: f32,
}

impl Menu {
    /// A list of `items` under the control at (`x`, `y`, `w`, `h`), at most to `max_bottom`.
    pub fn open(items: Vec<MenuItem>, anchor: [f32; 4], max_bottom: f32, scale: f32) -> Self {
        let [x, y, w, h] = anchor;
        let item_h = (26.0 * scale).round();
        let top = y + h + (2.0 * scale).round();
        let full = items.len() as f32 * item_h + (8.0 * scale).round();
        let height = full.min((max_bottom - top - 8.0 * scale).max(item_h * 3.0));
        let current = items.iter().position(|i| i.checked);
        let mut m = Self {
            items,
            x,
            y: top,
            w: w.max((200.0 * scale).round()),
            h: height.round(),
            item_h,
            scroll: 0.0,
            hover: current,
            pressed: false,
            scale,
        };
        // Show the current value in the middle of the list.
        if let Some(i) = current {
            m.scroll = (i as f32 * item_h - m.h / 2.0).max(0.0);
            m.clamp();
        }
        m
    }

    fn pad(&self) -> f32 {
        (4.0 * self.scale).round()
    }

    fn clamp(&mut self) {
        let max = (self.items.len() as f32 * self.item_h + 2.0 * self.pad() - self.h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max).round();
    }

    /// Whether a window position is over the menu.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let i = ((y - self.y - self.pad() + self.scroll) / self.item_h).floor();
        (i >= 0.0 && (i as usize) < self.items.len()).then_some(i as usize)
    }

    /// Pointer moved: `true` when the highlighted row changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        if h.is_none() {
            return false;
        }
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Button pressed over the menu.
    pub fn press(&mut self, x: f32, y: f32) {
        self.pressed = self.contains(x, y);
        self.hover(x, y);
    }

    /// Button released: the chosen item, if released over one after pressing in the menu.
    pub fn release(&mut self, x: f32, y: f32) -> Option<Choice> {
        let pressed = std::mem::take(&mut self.pressed);
        let i = self.index_at(x, y)?;
        pressed.then(|| self.items[i].choice.clone())
    }

    /// Scroll by `dy` device pixels.
    pub fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp();
        before != self.scroll
    }

    /// Move the highlight by `delta` rows (keyboard), keeping it in view.
    pub fn step(&mut self, delta: i32) {
        let n = self.items.len() as i32;
        if n == 0 {
            return;
        }
        let i = self.hover.map_or(0, |i| i as i32 + delta).clamp(0, n - 1) as usize;
        self.hover = Some(i);
        let top = i as f32 * self.item_h;
        let inner = self.h - 2.0 * self.pad();
        if top < self.scroll {
            self.scroll = top;
        } else if top + self.item_h > self.scroll + inner {
            self.scroll = top + self.item_h - inner;
        }
        self.clamp();
    }

    /// The highlighted item's choice (Enter).
    pub fn current(&self) -> Option<Choice> {
        self.hover.map(|i| self.items[i].choice.clone())
    }

    /// Paint the menu with a soft shadow.
    pub fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let s = self.scale;
        for (k, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (k * s).round();
            p.fill_rect(
                self.x - o + 2.0 * s,
                self.y - o + 3.0 * s,
                self.w + 2.0 * o,
                self.h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(self.x, self.y, self.w, self.h, t.border);
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            self.w - 2.0,
            self.h - 2.0,
            t.surface,
        );
        p.push_clip(self.x + 1.0, self.y + 1.0, self.w - 2.0, self.h - 2.0);
        let style = TextStyle {
            size: (13.0 * s).round(),
            bold: false,
        };
        let line = style.size * 1.33;
        let first = (self.scroll / self.item_h).floor() as usize;
        let last = ((self.scroll + self.h) / self.item_h).ceil() as usize;
        for i in first..last.min(self.items.len()) {
            let item = &self.items[i];
            let y = self.y + self.pad() + i as f32 * self.item_h - self.scroll;
            if self.hover == Some(i) {
                p.fill_rect(
                    self.x + self.pad(),
                    y,
                    self.w - 2.0 * self.pad(),
                    self.item_h,
                    t.hover,
                );
            }
            if item.checked {
                // Accent bar on the left marks the current value.
                p.fill_rect(
                    self.x + self.pad(),
                    y + (5.0 * s).round(),
                    (3.0 * s).round(),
                    self.item_h - (10.0 * s).round(),
                    t.accent,
                );
            }
            let color = if item.dim { t.text_muted } else { t.text };
            p.text(
                &item.label,
                self.x + (16.0 * s).round(),
                y + (self.item_h - line) / 2.0,
                style,
                color,
            );
        }
        p.pop_clip();
    }
}

/// Paint a tooltip box with `text` below the control at `anchor`, kept inside `width`.
pub fn paint_tooltip(
    p: &mut dyn Painter,
    t: &Theme,
    text: &str,
    text_w: f32,
    anchor: [f32; 4],
    width: f32,
    scale: f32,
) {
    let [ax, ay, aw, ah] = anchor;
    let style = TextStyle {
        size: (12.0 * scale).round(),
        bold: false,
    };
    let pad = (8.0 * scale).round();
    let (w, h) = (
        text_w + 2.0 * pad,
        (style.size * 1.33 + 2.0 * (5.0 * scale)).round(),
    );
    let x = (ax + aw / 2.0 - w / 2.0)
        .clamp(2.0, (width - w - 2.0).max(2.0))
        .round();
    let y = (ay + ah + (6.0 * scale)).round();
    p.fill_rect(x + 1.0, y + 2.0, w, h, Rgba(0, 0, 0, 30));
    p.fill_rect(x, y, w, h, t.border);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, t.surface);
    p.text(
        text,
        x + pad,
        y + (h - style.size * 1.33) / 2.0,
        style,
        t.text,
    );
}

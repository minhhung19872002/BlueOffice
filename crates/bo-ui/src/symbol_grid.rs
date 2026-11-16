//! Insert › Symbol drop-down of BlueDoc: recently used symbols as a 5 × 4 grid of large
//! glyphs, then "More Symbols…", as on a desktop word processor. "More Symbols…" opens a
//! larger grid of the common symbol blocks (Latin-1, Vietnamese letters, punctuation,
//! currency, arrows, mathematical operators) in the same drop-down.
use crate::theme::Theme;
use crate::{Painter, TextMeasure, TextStyle};
use bo_render::Rgba;

const MENU_EDGE: Rgba = Rgba(198, 198, 198, 255);
const CELL_EDGE: Rgba = Rgba(225, 225, 225, 255);

/// The symbols shown before any was used (a common set; most recent first once used).
pub(crate) const DEFAULT_RECENT: [char; 20] = [
    '€', '£', '¥', '©', '®', '™', '±', '≠', '≤', '≥', '÷', '×', '∞', 'µ', 'α', 'β', 'π', 'Ω', '∑',
    '☺',
];

/// Characters of the larger grid (More Symbols…).
fn more_symbols() -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    // Latin-1 punctuation and signs, Unicode §C1 Controls and Latin-1 Supplement.
    out.extend(('\u{a1}'..='\u{bf}').filter(|c| *c != '\u{ad}'));
    out.extend(['×', '÷']);
    // Vietnamese letters with horn and breve, Latin Extended-A / -B.
    out.extend(['Ă', 'ă', 'Đ', 'đ', 'Ơ', 'ơ', 'Ư', 'ư', '₫']);
    // General punctuation.
    out.extend([
        '–', '—', '‘', '’', '“', '”', '„', '†', '‡', '•', '…', '‰', '′', '″', '‹', '›',
    ]);
    // Currency.
    out.extend(['€', '£', '¥', '₩', '₹', '₽', '₺', '₿']);
    // Letter-like symbols and number forms.
    out.extend([
        '℃', '℉', '№', '℗', '™', 'Ω', '℮', '⅓', '⅔', '⅛', '⅜', '⅝', '⅞',
    ]);
    // Arrows.
    out.extend(['←', '↑', '→', '↓', '↔', '↕', '⇐', '⇒', '⇔']);
    // Mathematical operators.
    out.extend([
        '∀', '∂', '∃', '∅', '∇', '∈', '∉', '∏', '∑', '−', '√', '∝', '∞', '∠', '∧', '∨', '∩', '∪',
        '∫', '≈', '≠', '≡', '≤', '≥', '⊂', '⊃', '⊆', '⊇',
    ]);
    // Greek letters.
    out.extend('α'..='ω');
    // Geometric shapes and miscellaneous symbols.
    out.extend([
        '■', '□', '▲', '△', '▼', '▽', '◆', '◇', '○', '●', '★', '☆', '☎', '☐', '☑', '☒', '☺', '♠',
        '♣', '♥', '♦', '✓', '✔', '✗',
    ]);
    out.dedup();
    out
}

/// What a press in the drop-down picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolPick {
    /// Insert this character.
    Char(char),
    /// Show the larger grid.
    More,
}

/// The open drop-down.
pub(crate) struct SymbolGrid {
    x: f32,
    y: f32,
    scale: f32,
    /// Characters in the grid.
    chars: Vec<char>,
    cols: usize,
    /// The larger grid is shown.
    pub(crate) more: bool,
    hover: Option<usize>,
    /// Label of the "More Symbols…" entry (already translated).
    more_label: String,
    /// Measured glyph sizes, in grid order.
    glyphs: Vec<(f32, f32)>,
}

impl SymbolGrid {
    pub(crate) fn open(anchor: [f32; 4], recent: &[char], more_label: String, scale: f32) -> Self {
        Self {
            x: anchor[0],
            y: anchor[1] + anchor[3],
            scale,
            chars: recent.iter().copied().take(20).collect(),
            cols: 5,
            more: false,
            hover: None,
            more_label,
            glyphs: Vec::new(),
        }
    }

    /// Switch to the larger grid of symbol blocks.
    pub(crate) fn show_more(&mut self, window_w: f32) {
        self.more = true;
        self.chars = more_symbols();
        self.cols = 16;
        self.hover = None;
        let (w, _) = self.size();
        self.x = self.x.min((window_w - w - self.px(4.0)).max(0.0));
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Side of a symbol cell (the reference's gallery: about 37 px at 125 %).
    fn cell(&self) -> f32 {
        if self.more {
            self.px(26.0)
        } else {
            self.px(30.0)
        }
    }

    fn rows(&self) -> usize {
        self.chars.len().div_ceil(self.cols)
    }

    fn origin(&self) -> (f32, f32) {
        (self.x + self.px(4.0), self.y + self.px(4.0))
    }

    fn item_h(&self) -> f32 {
        self.px(28.0)
    }

    fn item_top(&self) -> f32 {
        self.origin().1 + self.rows() as f32 * self.cell() + self.px(5.0)
    }

    fn size(&self) -> (f32, f32) {
        let w = self.cols as f32 * self.cell() + self.px(8.0);
        let entry = if self.more { 0.0 } else { self.item_h() };
        (w, self.item_top() - self.y + entry + self.px(3.0))
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let (w, h) = self.size();
        x >= self.x && x < self.x + w && y >= self.y && y < self.y + h
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        let (gx, gy) = self.origin();
        let c = self.cell();
        if x >= gx && y >= gy && x < gx + self.cols as f32 * c {
            let row = ((y - gy) / c) as usize;
            let i = row * self.cols + ((x - gx) / c) as usize;
            if row < self.rows() && i < self.chars.len() {
                return Some(i);
            }
        }
        let top = self.item_top();
        (!self.more && self.contains(x, y) && y >= top && y < top + self.item_h()).then_some(1000)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn press(&self, x: f32, y: f32) -> Option<SymbolPick> {
        match self.index_at(x, y)? {
            1000 => Some(SymbolPick::More),
            i => self.chars.get(i).copied().map(SymbolPick::Char),
        }
    }

    /// Centre of the cell of `c` (tests).
    #[cfg(test)]
    pub(crate) fn center_of(&self, c: char) -> Option<(f32, f32)> {
        let i = self.chars.iter().position(|x| *x == c)?;
        let (gx, gy) = self.origin();
        let s = self.cell();
        Some((
            gx + (i % self.cols) as f32 * s + s / 2.0,
            gy + (i / self.cols) as f32 * s + s / 2.0,
        ))
    }

    /// Centre of the "More Symbols…" entry (tests).
    #[cfg(test)]
    pub(crate) fn more_center(&self) -> (f32, f32) {
        (
            self.x + self.px(40.0),
            self.item_top() + self.item_h() / 2.0,
        )
    }

    fn glyph_style(&self) -> TextStyle {
        TextStyle {
            size: if self.more {
                self.px(15.0)
            } else {
                self.px(18.0)
            },
            bold: false,
        }
    }

    /// Measure the glyphs to centre them in their cells.
    pub(crate) fn measure(&mut self, m: &dyn TextMeasure) {
        let st = self.glyph_style();
        self.glyphs = self
            .chars
            .iter()
            .map(|c| m.measure(&c.to_string(), st))
            .collect();
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let (w, h) = self.size();
        p.fill_rect(self.x, self.y, w, h, MENU_EDGE);
        p.fill_rect(self.x + 1.0, self.y + 1.0, w - 2.0, h - 2.0, t.surface);
        let (gx, gy) = self.origin();
        let c = self.cell();
        let glyph = self.glyph_style();
        for (i, ch) in self.chars.iter().enumerate() {
            let (x, y) = (
                gx + (i % self.cols) as f32 * c,
                gy + (i / self.cols) as f32 * c,
            );
            let hovered = self.hover == Some(i);
            p.fill_rect(
                x,
                y,
                c - 1.0,
                c - 1.0,
                if hovered { t.accent } else { CELL_EDGE },
            );
            p.fill_rect(
                x + 1.0,
                y + 1.0,
                c - 3.0,
                c - 3.0,
                if hovered {
                    t.hover
                } else {
                    Rgba(255, 255, 255, 255)
                },
            );
            let s = ch.to_string();
            let (gw, gh) = self
                .glyphs
                .get(i)
                .copied()
                .unwrap_or((glyph.size * 0.6, glyph.size * 1.33));
            p.text(
                &s,
                x + (c - 1.0 - gw) / 2.0,
                y + (c - 1.0 - gh) / 2.0,
                glyph,
                t.text,
            );
        }
        if !self.more {
            let top = self.item_top();
            p.fill_rect(self.x + 1.0, top - self.px(3.0), w - 2.0, 1.0, MENU_EDGE);
            if self.hover == Some(1000) {
                p.fill_rect(self.x + 1.0, top, w - 2.0, self.item_h(), t.hover);
            }
            let style = TextStyle {
                size: self.px(12.0),
                bold: false,
            };
            // Own mark before the entry: a small omega-like sign in the accent colour.
            p.text(
                "Ω",
                self.x + self.px(10.0),
                top + (self.item_h() - style.size * 1.33) / 2.0,
                style,
                t.accent,
            );
            p.text(
                &self.more_label,
                self.x + self.px(34.0),
                top + (self.item_h() - style.size * 1.33) / 2.0,
                style,
                t.text,
            );
        }
    }
}

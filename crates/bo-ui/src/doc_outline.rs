//! Outline view of BlueDoc (View › Outline): every body paragraph on one line under the
//! ribbon, indented by outline level (§17.3.1.20 outlineLvl through the heading styles),
//! headings with subordinate content marked ⊕, without ⊖, body text with a small dot, as the
//! reference word processor's outline. The Outlining tab promotes, demotes and moves the
//! selected paragraph and filters by level; Tab / Shift+Tab and Alt+Shift+arrows do the same
//! from the keyboard; a double click or Enter goes back to Print Layout at that paragraph.
use crate::{Font, Key, Mods, Painter, TextMeasure};
use bo_engine::views::OutlineItem;
use bo_render::{PathSeg, Rgba};

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const HEADING: Rgba = Rgba(0x2f, 0x54, 0x96, 255);
const SYMBOL: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const SELECTED: Rgba = Rgba(0xdc, 0xdc, 0xdc, 255);
const HOVER: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);

/// What an outline input does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutlineAction {
    None,
    Redraw,
    /// An Outlining command for the selected paragraph (`outline.promote`, …).
    Command(&'static str),
    /// Go back to Print Layout at this body paragraph.
    Open(usize),
    /// Edit the text of body paragraph `para` at byte offset `at` (typing in the outline).
    Edit {
        /// Body paragraph id.
        para: usize,
        /// Byte offset of the caret in the paragraph's text.
        at: usize,
        /// What the key does.
        edit: OutlineEdit,
    },
}

/// A text edit typed in the outline (round 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutlineEdit {
    /// Insert text at the caret.
    Type(String),
    /// Enter: a new item at the same level after the caret.
    Split,
    /// Backspace.
    Back,
    /// Delete.
    Forward,
}

struct Row {
    para: usize,
    level: Option<u8>,
    text: String,
    /// List label (`1.`, `1.1.`) of a numbered paragraph and the tab stop after it
    /// (points from the label's start; 0 = a space).
    label: String,
    tab: f32,
    /// A heading followed by deeper content.
    parent: bool,
}

/// The outline view.
pub(crate) struct OutlineView {
    rect: [f32; 4],
    scale: f32,
    rows: Vec<Row>,
    /// Index of the selected row.
    selected: usize,
    /// Show headings up to this level (1-based); `None`: everything.
    pub(crate) show_level: Option<u8>,
    scroll: f32,
    hover: Option<usize>,
    body_label: String,
    /// Look of each level (Heading 1–9, then body text) from the document's styles: the
    /// reference shows the text with its formatting.
    looks: Vec<Option<bo_engine::gallery::StylePreview>>,
    /// Show the text with its formatting (else the plain interface face).
    pub(crate) formatting: bool,
    /// Show only the first line of each paragraph.
    pub(crate) first_line: bool,
    /// Measured label widths (12 px reference size), by row.
    label_w: Vec<f32>,
    /// Collapsed headings (paragraph ids): their subordinate content is hidden.
    collapsed: std::collections::HashSet<usize>,
    /// Caret in the selected row's text (characters).
    caret: usize,
    /// Width of the selected row's text before the caret (measured at layout, device px).
    caret_w: f32,
}

/// Row height at 100 % per level when the style's size is unknown (Heading 1 is the tallest).
fn row_h(level: Option<u8>) -> f32 {
    match level {
        Some(0) => 24.0,
        Some(1) => 21.0,
        Some(_) => 19.0,
        None => 17.0,
    }
}

/// One outline level step and the default tab interval: 0.5" (36 pt) at 100 %, as measured on
/// the reference (level symbols 60 px apart at 125 %).
const STEP: f32 = 48.0;

/// Where the text after a list label starts, from the outline's left edge: the first default
/// tab stop (every 0.5" from that edge, §17.15.1.25 defaultTabStop) past the label's end —
/// the reference drops the paragraph indents in Outline view, so the level's own hanging stop
/// does not apply (measured: "1." → 12 px gap, "1.1." past the stop → next stop, 125 %).
pub(crate) fn text_stop(label_end: f32, unit: f32) -> f32 {
    if unit <= 0.0 {
        return label_end;
    }
    ((label_end + 1.0) / unit).ceil() * unit
}

impl OutlineView {
    pub(crate) fn new(body_label: String) -> Self {
        Self {
            rect: [0.0; 4],
            scale: 1.0,
            rows: Vec::new(),
            selected: 0,
            show_level: None,
            scroll: 0.0,
            hover: None,
            body_label,
            looks: vec![None; 10],
            formatting: true,
            first_line: false,
            collapsed: std::collections::HashSet::new(),
            label_w: Vec::new(),
            caret: 0,
            caret_w: 0.0,
        }
    }

    /// Looks of Heading 1–9 and body text (index 9).
    pub(crate) fn set_looks(&mut self, looks: Vec<Option<bo_engine::gallery::StylePreview>>) {
        self.looks = looks;
    }

    fn look(&self, level: Option<u8>) -> Option<&bo_engine::gallery::StylePreview> {
        self.looks
            .get(usize::from(level.unwrap_or(9).min(9)))
            .and_then(Option::as_ref)
    }

    fn line_h(&self, level: Option<u8>) -> f32 {
        let pt = self.look(level).map_or(0.0, |l| l.size);
        // One text line of the style's font, no paragraph spacing (reference: 27 px between a
        // 16 pt and a 14 pt heading at 125 %).
        if pt > 0.0 {
            (pt * 4.0 / 3.0 * 1.1).max(16.0)
        } else {
            row_h(level)
        }
    }

    /// Take the document's outline; select the row of `caret` (body paragraph id).
    pub(crate) fn set_items(
        &mut self,
        items: &[OutlineItem],
        caret: Option<usize>,
        labels: &std::collections::HashMap<usize, (String, f32)>,
    ) {
        let keep = self.rows.get(self.selected).map(|r| r.para);
        self.rows = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let level = it.entry.level;
                let parent = level.is_some_and(|l| {
                    items
                        .get(i + 1)
                        .is_some_and(|n| n.entry.level.is_none_or(|nl| nl > l))
                });
                Row {
                    para: it.entry.para,
                    level,
                    text: it.entry.text.clone(),
                    label: labels
                        .get(&it.entry.para)
                        .map(|l| l.0.clone())
                        .unwrap_or_default(),
                    tab: labels.get(&it.entry.para).map_or(0.0, |l| l.1),
                    parent,
                }
            })
            .collect();
        let want = keep.or(caret);
        self.selected = want
            .and_then(|p| self.rows.iter().position(|r| r.para == p))
            .unwrap_or(0);
        let len = self
            .rows
            .get(self.selected)
            .map_or(0, |r| r.text.chars().count());
        // A newly selected item gets the caret at its end; the same item keeps it.
        let same = keep.is_some() && self.rows.get(self.selected).map(|r| r.para) == keep;
        self.caret = if same { self.caret.min(len) } else { len };
    }

    /// Select the row of body paragraph `para` with the caret at character `at` (after an edit).
    pub(crate) fn select_para(&mut self, para: usize, at: usize) {
        if let Some(i) = self.rows.iter().position(|r| r.para == para) {
            self.selected = i;
            self.caret = at.min(self.rows[i].text.chars().count());
            self.reveal();
        }
    }

    /// The caret: body paragraph and character index in its text.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn caret(&self) -> Option<(usize, usize)> {
        self.rows.get(self.selected).map(|r| (r.para, self.caret))
    }

    /// The edit a typed key makes at the caret.
    fn edit(&self, edit: OutlineEdit) -> OutlineAction {
        let Some(r) = self.rows.get(self.selected) else {
            return OutlineAction::None;
        };
        let at = r
            .text
            .char_indices()
            .nth(self.caret)
            .map_or(r.text.len(), |(b, _)| b);
        OutlineAction::Edit {
            para: r.para,
            at,
            edit,
        }
    }

    /// Text typed while the outline has the focus.
    pub(crate) fn type_text(&self, text: &str) -> OutlineAction {
        if text.chars().all(char::is_control) {
            return OutlineAction::None;
        }
        self.edit(OutlineEdit::Type(text.to_owned()))
    }

    fn move_caret(&mut self, to: usize) -> OutlineAction {
        let len = self
            .rows
            .get(self.selected)
            .map_or(0, |r| r.text.chars().count());
        self.caret = to.min(len);
        OutlineAction::Redraw
    }

    /// The selected paragraph and its level.
    pub(crate) fn selected(&self) -> Option<(usize, Option<u8>)> {
        self.rows.get(self.selected).map(|r| (r.para, r.level))
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn shown(&self, r: &Row) -> bool {
        match self.show_level {
            None => true,
            Some(n) => r.level.is_some_and(|l| l < n),
        }
    }

    /// Rows hidden under a collapsed heading.
    fn hidden(&self) -> Vec<bool> {
        let mut out = vec![false; self.rows.len()];
        let mut under: Option<u8> = None;
        for (i, r) in self.rows.iter().enumerate() {
            if let Some(l) = under {
                if r.level.is_none_or(|rl| rl > l) {
                    out[i] = true;
                    continue;
                }
                under = None;
            }
            if r.level.is_some() && self.collapsed.contains(&r.para) {
                under = r.level;
            }
        }
        out
    }

    /// Expand (`true`) or collapse the selected heading; `false` when nothing changed.
    pub(crate) fn expand(&mut self, open: bool) -> bool {
        let Some(r) = self.rows.get(self.selected).filter(|r| r.parent) else {
            return false;
        };
        let para = r.para;
        if open {
            self.collapsed.remove(&para)
        } else {
            self.collapsed.insert(para)
        }
    }

    /// Visible rows with their top (content coordinates) and height.
    fn placed(&self) -> Vec<(usize, f32, f32)> {
        let mut y = self.px(15.0);
        let mut out = Vec::new();
        let hidden = self.hidden();
        for (i, r) in self.rows.iter().enumerate() {
            if !self.shown(r) || hidden[i] {
                continue;
            }
            let h = self.px(self.line_h(r.level));
            out.push((i, y, h));
            y += h;
        }
        out
    }

    /// The font a row's text is drawn with (the style's look with Show Text Formatting).
    fn row_font(&self, level: Option<u8>) -> Font<'_> {
        let size = match level {
            Some(0) => 16.0,
            Some(1) => 13.0,
            Some(2) => 12.0,
            _ => 11.0,
        };
        match self.look(level).filter(|_| self.formatting) {
            Some(l) => Font {
                family: l.font.as_str(),
                size: (l.size * 4.0 / 3.0 * self.scale).round(),
                weight: if l.bold { 700 } else { 400 },
                italic: l.italic,
                underline: l.underline,
            },
            None => Font {
                family: if level.is_some() {
                    "Calibri Light"
                } else {
                    "Calibri"
                },
                size: self.px(size),
                weight: 400,
                italic: false,
                underline: false,
            },
        }
    }

    pub(crate) fn layout(&mut self, rect: [f32; 4], scale: f32, text: &dyn TextMeasure) {
        self.rect = rect;
        self.scale = scale;
        // Label widths at a 12 px reference size (scaled to each row's font when drawn).
        let f = Font {
            family: "Times New Roman",
            size: (12.0 * scale).round(),
            weight: 700,
            italic: false,
            underline: false,
        };
        self.label_w = self
            .rows
            .iter()
            .map(|r| {
                if r.label.is_empty() {
                    0.0
                } else {
                    text.measure_font(&r.label, &f).0
                }
            })
            .collect();
        self.caret_w = match self.rows.get(self.selected) {
            Some(r) if self.caret > 0 => {
                let before: String = r.text.chars().take(self.caret).collect();
                text.measure_font(&before, &self.row_font(r.level)).0
            }
            _ => 0.0,
        };
        self.clamp();
    }

    fn content_h(&self) -> f32 {
        self.placed()
            .last()
            .map_or(0.0, |(_, y, h)| y + h + self.px(12.0))
    }

    fn clamp(&mut self) {
        let max = (self.content_h() - self.rect[3]).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        let cy = y - self.rect[1] + self.scroll;
        self.placed()
            .into_iter()
            .find(|(_, top, h)| cy >= *top && cy < top + h)
            .map(|(i, ..)| i)
    }

    pub(crate) fn hover(&mut self, _x: f32, y: f32) -> bool {
        let h = self.row_at(y);
        std::mem::replace(&mut self.hover, h) != h
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let old = self.scroll;
        self.scroll += dy;
        self.clamp();
        old != self.scroll
    }

    pub(crate) fn press(&mut self, _x: f32, y: f32, count: u8) -> OutlineAction {
        match self.row_at(y) {
            Some(i) if count >= 2 && self.rows[i].parent => {
                self.selected = i;
                let open = self.collapsed.contains(&self.rows[i].para);
                self.expand(open);
                OutlineAction::Redraw
            }
            Some(i) if count >= 2 => OutlineAction::Open(self.rows[i].para),
            Some(i) => {
                self.selected = i;
                self.caret = self.rows[i].text.chars().count();
                OutlineAction::Redraw
            }
            None => OutlineAction::None,
        }
    }

    fn step(&mut self, down: bool) -> OutlineAction {
        let shown: Vec<usize> = self.placed().into_iter().map(|(i, ..)| i).collect();
        let at = shown.iter().position(|i| *i == self.selected).unwrap_or(0);
        let next = if down {
            (at + 1).min(shown.len().saturating_sub(1))
        } else {
            at.saturating_sub(1)
        };
        if let Some(i) = shown.get(next) {
            if self.selected != *i {
                self.caret = self.rows[*i].text.chars().count();
            }
            self.selected = *i;
        }
        self.reveal();
        OutlineAction::Redraw
    }

    fn reveal(&mut self) {
        if let Some((_, top, h)) = self
            .placed()
            .into_iter()
            .find(|(i, ..)| *i == self.selected)
        {
            if top < self.scroll {
                self.scroll = top;
            } else if top + h > self.scroll + self.rect[3] {
                self.scroll = top + h - self.rect[3];
            }
        }
    }

    pub(crate) fn key(&mut self, key: Key, m: Mods) -> OutlineAction {
        match key {
            Key::Up if m.alt && m.shift => OutlineAction::Command("outline.moveUp"),
            Key::Down if m.alt && m.shift => OutlineAction::Command("outline.moveDown"),
            Key::Left if m.alt && m.shift => OutlineAction::Command("outline.promote"),
            Key::Right if m.alt && m.shift => OutlineAction::Command("outline.demote"),
            Key::Tab if m.shift => OutlineAction::Command("outline.promote"),
            Key::Tab => OutlineAction::Command("outline.demote"),
            Key::Up => self.step(false),
            Key::Down => self.step(true),
            // Typing edits the selected item as in the reference's outline: Enter starts a new
            // item at the same level (a double click goes back to Print Layout).
            Key::Enter => self.edit(OutlineEdit::Split),
            Key::Backspace if self.caret > 0 => self.edit(OutlineEdit::Back),
            Key::Delete => self.edit(OutlineEdit::Forward),
            Key::Left => self.move_caret(self.caret.saturating_sub(1)),
            Key::Right => self.move_caret(self.caret + 1),
            Key::Home => self.move_caret(0),
            Key::End => self.move_caret(usize::MAX),
            _ => OutlineAction::None,
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, WHITE);
        let indent = self.px(STEP);
        let left = x + self.px(24.0);
        for (i, top, rh) in self.placed() {
            let ry = y + top - self.scroll;
            if ry + rh < y || ry > y + h {
                continue;
            }
            let r = &self.rows[i];
            let lx = left + f32::from(r.level.unwrap_or(9).min(8)) * indent;
            let lx = if r.level.is_none() {
                // Body text sits one step under the heading before it.
                let prev = self.rows[..i].iter().rev().find_map(|p| p.level);
                left + prev.map_or(0.0, |l| f32::from(l + 1) * indent)
            } else {
                lx
            };
            let cy = ry + rh / 2.0;
            // Heading symbols 18 px across at 125 % as the reference.
            let k = self.px(7.0);
            let sx = if r.level.is_some() {
                lx + k
            } else {
                lx - self.px(17.0)
            };
            if i == self.selected {
                circle(p, sx, cy, k + self.px(3.0), SELECTED);
            } else if self.hover == Some(i) {
                circle(p, sx, cy, k + self.px(3.0), HOVER);
            }
            if r.level.is_some() {
                circle(p, lx + k, cy, k, SYMBOL);
                circle(p, lx + k, cy, k - self.scale.max(1.0) * 1.2, WHITE);
                let s = self.px(3.0);
                p.fill_rect(
                    (lx + k - s).round(),
                    (cy - 0.5).round(),
                    2.0 * s,
                    self.scale.max(1.0).round(),
                    SYMBOL,
                );
                if r.parent {
                    p.fill_rect(
                        (lx + k - 0.5).round(),
                        (cy - s).round(),
                        self.scale.max(1.0).round(),
                        2.0 * s,
                        SYMBOL,
                    );
                }
            } else {
                // The body text's small circle sits 17 px (100 %) before its text.
                circle(p, sx, cy, self.px(3.0), SYMBOL);
                circle(p, sx, cy, self.px(3.0) - self.scale.max(1.0), WHITE);
            }
            let (size, weight, color) = match r.level {
                Some(0) => (16.0, 400, HEADING),
                Some(1) => (13.0, 400, HEADING),
                Some(2) => (12.0, 400, Rgba(0x1f, 0x37, 0x63, 255)),
                Some(_) => (11.0, 400, HEADING),
                None => (11.0, 400, INK),
            };
            let look = self.look(r.level).filter(|_| self.formatting);
            let font = match look {
                Some(l) => Font {
                    family: l.font.as_str(),
                    size: (l.size * 4.0 / 3.0 * self.scale).round(),
                    weight: if l.bold { 700 } else { 400 },
                    italic: l.italic,
                    underline: l.underline,
                },
                None => Font {
                    family: if r.level.is_some() {
                        "Calibri Light"
                    } else {
                        "Calibri"
                    },
                    size: self.px(size),
                    weight,
                    italic: false,
                    underline: false,
                },
            };
            let color = look
                .and_then(|l| crate::theme::parse_color(&format!("#{}", l.color)))
                .unwrap_or(color);
            // The list label first ("1.", "1.1."), then the text at the list's tab stop
            // (as in Print Layout), as the reference.
            let mut text: String = if self.first_line {
                r.text.chars().take(90).collect()
            } else {
                r.text.clone()
            };
            // Headings: the label 32 px (125 %) after the symbol's left; body text at its level.
            let tx = if r.level.is_some() {
                lx + self.px(25.6)
            } else {
                lx
            };
            let ty = (cy - font.size * 0.68).round();
            if !r.label.is_empty() {
                p.text_font(&r.label, tx, ty, &font, color);
                let label_w = self.label_w.get(i).copied().unwrap_or(0.0) * font.size
                    / self.px(12.0).max(1.0);
                let gap = if r.tab <= 0.0 {
                    // A space suffix: a space after the label.
                    label_w + font.size * 0.3
                } else {
                    text_stop(tx - left + label_w, self.px(STEP)) - (tx - left)
                };
                p.text_font(&text, tx + gap, ty, &font, color);
                text.clear();
                if i == self.selected {
                    self.paint_caret(p, tx + gap, ty, font.size);
                }
            } else if i == self.selected {
                self.paint_caret(p, tx, ty, font.size);
            }
            if !text.is_empty() {
                p.text_font(&text, tx, ty, &font, color);
            }
        }
        if self.rows.is_empty() {
            let font = Font {
                family: "Segoe UI",
                size: self.px(12.0),
                weight: 400,
                italic: false,
                underline: false,
            };
            p.text_font(&self.body_label, left, y + self.px(12.0), &font, SYMBOL);
        }
        p.pop_clip();
    }
}

impl OutlineView {
    /// The text caret of the selected row, its text starting at `x`.
    fn paint_caret(&self, p: &mut dyn Painter, x: f32, ty: f32, size: f32) {
        let w = self.scale.round().max(1.0);
        p.fill_rect(
            (x + self.caret_w).round(),
            (ty + size * 0.1).round(),
            w,
            (size * 1.2).round(),
            Rgba(0, 0, 0, 255),
        );
    }
}

fn circle(p: &mut dyn Painter, cx: f32, cy: f32, r: f32, c: Rgba) {
    let mut v = Vec::with_capacity(26);
    for i in 0..=24 {
        let a = i as f32 / 24.0 * std::f32::consts::TAU;
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        v.push(if i == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    v.push(PathSeg::Z);
    p.path(&v, Some(c), None);
}

#[cfg(test)]
mod stop_tests {
    use super::text_stop;

    #[test]
    fn label_text_goes_to_the_next_half_inch_stop() {
        // 125 %: stops every 60 px from the outline's left edge (measured on the reference).
        // "1." ends at 48 -> text at 60; "1.1." of level 2 ends at 124 (past 120) -> 180.
        assert!((text_stop(48.0, 60.0) - 60.0).abs() < 0.01);
        assert!((text_stop(124.0, 60.0) - 180.0).abs() < 0.01);
        assert!((text_stop(10.0, 0.0) - 10.0).abs() < 0.01);
    }
}

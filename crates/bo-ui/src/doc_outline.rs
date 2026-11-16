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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineAction {
    None,
    Redraw,
    /// An Outlining command for the selected paragraph (`outline.promote`, …).
    Command(&'static str),
    /// Go back to Print Layout at this body paragraph.
    Open(usize),
}

struct Row {
    para: usize,
    level: Option<u8>,
    text: String,
    /// List label (`1.`, `1.1.`) of a numbered paragraph.
    label: String,
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
    /// Collapsed headings (paragraph ids): their subordinate content is hidden.
    collapsed: std::collections::HashSet<usize>,
}

/// Row height at 100 % per level (Heading 1 is the tallest).
fn row_h(level: Option<u8>) -> f32 {
    match level {
        Some(0) => 30.0,
        Some(1) => 26.0,
        Some(_) => 23.0,
        None => 21.0,
    }
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
        (pt * 4.0 / 3.0 * 1.35).max(row_h(level))
    }

    /// Take the document's outline; select the row of `caret` (body paragraph id).
    pub(crate) fn set_items(
        &mut self,
        items: Vec<OutlineItem>,
        caret: Option<usize>,
        labels: &std::collections::HashMap<usize, String>,
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
                    label: labels.get(&it.entry.para).cloned().unwrap_or_default(),
                    parent,
                }
            })
            .collect();
        let want = keep.or(caret);
        self.selected = want
            .and_then(|p| self.rows.iter().position(|r| r.para == p))
            .unwrap_or(0);
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
        let mut y = self.px(12.0);
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

    pub(crate) fn layout(&mut self, rect: [f32; 4], scale: f32, _text: &dyn TextMeasure) {
        self.rect = rect;
        self.scale = scale;
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
            Key::Enter => self
                .rows
                .get(self.selected)
                .map_or(OutlineAction::None, |r| OutlineAction::Open(r.para)),
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
        let indent = self.px(24.0);
        let left = x + self.px(28.0);
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
            let k = self.px(5.0);
            if i == self.selected {
                circle(p, lx + k, cy, k + self.px(3.0), SELECTED);
            } else if self.hover == Some(i) {
                circle(p, lx + k, cy, k + self.px(3.0), HOVER);
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
                circle(p, lx + k, cy, self.px(3.0), SYMBOL);
                circle(p, lx + k, cy, self.px(3.0) - self.scale.max(1.0), WHITE);
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
            // The list label first ("1.", "1.1."), then the text, as the reference.
            let text = if r.label.is_empty() {
                r.text.clone()
            } else {
                format!("{}   {}", r.label, r.text)
            };
            let text = if self.first_line {
                text.chars().take(90).collect()
            } else {
                text
            };
            p.text_font(
                &text,
                lx + self.px(18.0),
                (cy - font.size * 0.68).round(),
                &font,
                color,
            );
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

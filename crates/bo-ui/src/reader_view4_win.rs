//! BlueReader View tab, round 4 (a child of `reader_window`): Text Viewer (the document's
//! text laid out as plain text), split windows (vertical, horizontal, spreadsheet) with a pane
//! per scroll position, Read aloud through the host's speech, and AutoScroll moved every
//! frame at the reference's speeds.
use super::{inside, ReaderRequest, ReaderWindow, TEXT};
use crate::popup::{Choice, MenuItem};
use crate::reader_viewx::{text_lines, AUTO_SPEEDS};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_render::Rgba;
use std::time::Instant;

/// Split of the document area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Split {
    /// Side by side.
    Vertical,
    /// One above the other.
    Horizontal,
    /// Four panes.
    Spreadsheet,
}

/// Grey of the split bars (the window's strip colour, measured on the reference's splitter).
const SPLIT_BAR: Rgba = Rgba(199, 204, 217, 255);
/// Grey frame around the Text Viewer's sheet.
const TV_FRAME: Rgba = Rgba(160, 160, 160, 255);

/// State of the round 4 View additions.
#[derive(Debug, Default)]
pub(crate) struct View4 {
    /// Text Viewer: on, its lines for the document of tab key `tv_doc`, the scroll.
    pub(crate) text_viewer: bool,
    tv_doc: Option<usize>,
    tv_lines: Vec<String>,
    tv_scroll: (f32, f32),
    /// Split panes: the kind, the pane the view is in, each pane's scroll, the whole area.
    pub(crate) split: Option<Split>,
    active: usize,
    scrolls: Vec<(f32, f32)>,
    area: [f32; 4],
    panes: Vec<[f32; 4]>,
    /// Where the bars sit (fraction of the area's width / height; 0 is the middle, so the
    /// default is an even split), and which bars a press is dragging (vertical, horizontal).
    bar_at: (f32, f32),
    bar_drag: Option<(bool, bool)>,
    /// AutoScroll: the last frame's time and the movement not yet applied (device pixels).
    auto_last: Option<Instant>,
    auto_rest: f32,
    /// Read aloud: speaking, paused, rate −10…10.
    pub(crate) reading: bool,
    paused: bool,
    pub(crate) rate: i32,
}

impl ReaderWindow {
    /// Commands of the round 4 View additions; `true` when `cmd` was one.
    pub(crate) fn command_view4(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        match cmd {
            "view.textViewer" => {
                self.view4.text_viewer = !self.view4.text_viewer;
                self.view4.tv_doc = None;
                self.view4.tv_scroll = (0.0, 0.0);
                if self.view4.text_viewer {
                    self.viewx.reflow = false;
                    self.text_viewer_lines();
                }
            }
            "menu.split" => {
                let items = [
                    ("split.vertical", "pdfView.splitVertical"),
                    ("split.horizontal", "pdfView.splitHorizontal"),
                    ("split.spreadsheet", "pdfView.splitSpreadsheet"),
                ]
                .iter()
                .map(|(c, k)| MenuItem {
                    label: self.t(k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: self.view4.split
                        == Some(match *c {
                            "split.vertical" => Split::Vertical,
                            "split.horizontal" => Split::Horizontal,
                            _ => Split::Spreadsheet,
                        }),
                    dim: false,
                })
                .collect();
                let r = self.control_rect("menu.split").unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "menu.split"));
            }
            "split.vertical" | "split.horizontal" | "split.spreadsheet" => {
                let kind = match cmd {
                    "split.vertical" => Split::Vertical,
                    "split.horizontal" => Split::Horizontal,
                    _ => Split::Spreadsheet,
                };
                // Choosing the split shown again removes it, like the reference's toggle.
                self.set_split((self.view4.split != Some(kind)).then_some(kind), m);
            }
            "menu.read" => {
                let r = self.view4.reading;
                let items = [
                    ("read.page", "pdfView.readPage", true, false),
                    ("read.from", "pdfView.readFrom", true, false),
                    ("read.pause", "pdfView.readPause", r, self.view4.paused),
                    ("read.stop", "pdfView.readStop", r, false),
                    ("read.slower", "pdfView.readSlower", true, false),
                    (
                        "read.normal",
                        "pdfView.readNormal",
                        true,
                        self.view4.rate == 0,
                    ),
                    ("read.faster", "pdfView.readFaster", true, false),
                ]
                .iter()
                .map(|(c, k, on, checked)| MenuItem {
                    label: self.t(k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: *checked,
                    dim: !on,
                })
                .collect();
                let r = self.control_rect("menu.read").unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "menu.read"));
            }
            "read.page" | "read.from" => {
                let Some(t) = self.tabs.get_mut(self.active) else {
                    return true;
                };
                let first = t.view.current_page();
                let last = if cmd == "read.page" {
                    first
                } else {
                    t.view.page_count().saturating_sub(1)
                };
                let mut text = String::new();
                for i in first..=last {
                    if let Some(layer) = t.view.text(i) {
                        text.push_str(&layer.text());
                        text.push('\n');
                    }
                }
                self.view4.reading = true;
                self.view4.paused = false;
                self.requests.push(ReaderRequest::Speak {
                    text,
                    rate: self.view4.rate,
                });
            }
            "read.pause" => {
                self.view4.paused = !self.view4.paused;
                self.requests
                    .push(ReaderRequest::SpeechControl(if self.view4.paused {
                        1
                    } else {
                        2
                    }));
            }
            "read.stop" => {
                self.view4.reading = false;
                self.view4.paused = false;
                self.requests.push(ReaderRequest::SpeechControl(0));
            }
            "read.slower" | "read.normal" | "read.faster" => {
                self.view4.rate = match cmd {
                    "read.slower" => (self.view4.rate - 2).max(-10),
                    "read.faster" => (self.view4.rate + 2).min(10),
                    _ => 0,
                };
                self.requests
                    .push(ReaderRequest::SpeechControl(10 + self.view4.rate));
            }
            _ => return false,
        }
        for (id, on) in [
            ("view.textViewer", self.view4.text_viewer),
            ("menu.split", self.view4.split.is_some()),
            ("menu.read", self.view4.reading),
        ] {
            if on {
                self.on.insert(id);
            } else {
                self.on.remove(id);
            }
        }
        true
    }

    /// Lines of the Text Viewer for the active document.
    fn text_viewer_lines(&mut self) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if self.view4.tv_doc == Some(t.key_base) {
            return;
        }
        let mut lines = Vec::new();
        for i in 0..t.view.page_count() {
            if let Some(layer) = t.view.text(i) {
                lines.extend(text_lines(layer.chars()));
            }
            // A blank line between pages.
            lines.push(String::new());
        }
        self.view4.tv_lines = lines;
        self.view4.tv_doc = Some(t.key_base);
    }

    /// Text Viewer text style (the reference: interface font, 17 px rows at 125 %).
    fn tv_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(10.4),
            bold: false,
        }
    }

    /// The Text Viewer's sheet (inside the document area, 8 px of grey around at 125 %).
    fn tv_sheet(&self) -> [f32; 4] {
        let d = self.view4_area();
        let g = self.px(6.4);
        [d[0] + g, d[1] + g, (d[2] - g).max(1.0), (d[3] - g).max(1.0)]
    }

    fn tv_scroll_by(&mut self, dx: f32, dy: f32) {
        let lh = self.px(13.6);
        let sheet = self.tv_sheet();
        let content = self.view4.tv_lines.len() as f32 * lh + self.px(51.2);
        let max_y = (content - sheet[3]).max(0.0);
        let st = self.tv_style();
        let widest = self
            .view4
            .tv_lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as f32
            * st.size
            * 0.5;
        let max_x = (widest + self.px(51.2) - sheet[2]).max(0.0);
        let (x, y) = self.view4.tv_scroll;
        self.view4.tv_scroll = ((x + dx).clamp(0.0, max_x), (y + dy).clamp(0.0, max_y));
    }

    /// The whole document area (all panes of a split window).
    pub(crate) fn view4_area(&self) -> [f32; 4] {
        if self.view4.split.is_some() {
            self.view4.area
        } else {
            self.doc_rect
        }
    }

    /// Split the document area (or remove the split).
    fn set_split(&mut self, split: Option<Split>, m: &dyn TextMeasure) {
        let scroll = self
            .tabs
            .get(self.active)
            .map_or((0.0, 0.0), |t| t.view.scroll_state().0);
        self.view4.split = split;
        self.view4.active = 0;
        self.view4.bar_at = (0.0, 0.0);
        self.view4.bar_drag = None;
        let n = match split {
            None => 0,
            Some(Split::Spreadsheet) => 4,
            Some(_) => 2,
        };
        self.view4.scrolls = vec![scroll; n];
        self.relayout(m);
    }

    /// Panes of a split window: the document area divided by 6 px bars; the view takes the
    /// active pane (called at the end of the window's layout).
    pub(crate) fn view4_layout(&mut self) {
        let Some(split) = self.view4.split else {
            self.view4.panes.clear();
            return;
        };
        let d = self.doc_rect;
        self.view4.area = d;
        let bar = self.px(4.8).round();
        let (fx, fy) = self.view4.bar_at;
        let (hw, hh) = (
            ((d[2] - bar) * (0.5 + fx)).floor(),
            ((d[3] - bar) * (0.5 + fy)).floor(),
        );
        let (rw, rh) = (d[2] - bar - hw, d[3] - bar - hh);
        let (x2, y2) = (d[0] + hw + bar, d[1] + hh + bar);
        self.view4.panes = match split {
            Split::Vertical => vec![[d[0], d[1], hw, d[3]], [x2, d[1], rw, d[3]]],
            Split::Horizontal => vec![[d[0], d[1], d[2], hh], [d[0], y2, d[2], rh]],
            Split::Spreadsheet => vec![
                [d[0], d[1], hw, hh],
                [x2, d[1], rw, hh],
                [d[0], y2, hw, rh],
                [x2, y2, rw, rh],
            ],
        };
        let k = self.view4.active.min(self.view4.panes.len() - 1);
        self.doc_rect = self.view4.panes[k];
    }

    /// Move the view to pane `k` (a click or the wheel over it).
    fn activate_pane(&mut self, k: usize, m: &dyn TextMeasure) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let now = t.view.scroll_state().0;
        let old = self.view4.active;
        if let Some(s) = self.view4.scrolls.get_mut(old) {
            *s = now;
        }
        self.view4.active = k;
        let next = self.view4.scrolls.get(k).copied().unwrap_or(now);
        self.relayout(m);
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_scroll(next);
        }
    }

    /// Input of the round 4 additions first: `Some(redraw)` when handled.
    pub(crate) fn route_view4(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(r) = self.drag_split_bar(input, m) {
            return Some(r);
        }
        // A press or the wheel over another pane moves the view there first.
        if self.view4.split.is_some() && matches!(input, Input::Press { .. } | Input::Wheel(_)) {
            let (x, y) = match input {
                Input::Press { x, y, .. } => (*x, *y),
                _ => self.pointer,
            };
            if let Some(k) = self.view4.panes.iter().position(|r| inside(*r, x, y)) {
                if k != self.view4.active {
                    self.activate_pane(k, m);
                }
            }
        }
        if self.view4.text_viewer {
            let area = self.view4_area();
            let line = self.px(13.6);
            match input {
                Input::Wheel(dy) if inside(area, self.pointer.0, self.pointer.1) => {
                    self.tv_scroll_by(0.0, -dy * self.px(1.0));
                    return Some(true);
                }
                Input::Key(Key::Down, _) => self.tv_scroll_by(0.0, line),
                Input::Key(Key::Up, _) => self.tv_scroll_by(0.0, -line),
                Input::Key(Key::PageDown, _) => self.tv_scroll_by(0.0, area[3] * 0.9),
                Input::Key(Key::PageUp, _) => self.tv_scroll_by(0.0, -area[3] * 0.9),
                Input::Key(Key::Right, _) => self.tv_scroll_by(line * 2.0, 0.0),
                Input::Key(Key::Left, _) => self.tv_scroll_by(-line * 2.0, 0.0),
                Input::Key(Key::Home, _) => self.view4.tv_scroll = (0.0, 0.0),
                Input::Key(Key::End, _) => self.tv_scroll_by(0.0, f32::MAX / 4.0),
                // Clicks on the sheet do nothing (no page tools on plain text).
                Input::Press { x, y, .. } if inside(area, *x, *y) => return Some(false),
                _ => return None,
            }
            return Some(true);
        }
        None
    }

    /// Which split bars are at (`x`, `y`): (the vertical one, the horizontal one).
    pub(crate) fn split_bar_at(&self, x: f32, y: f32) -> Option<(bool, bool)> {
        let split = self.view4.split?;
        let a = self.view4.area;
        if !inside(a, x, y) || self.view4.panes.iter().any(|r| inside(*r, x, y)) {
            return None;
        }
        let p0 = self.view4.panes.first()?;
        let near_v = (x - (p0[0] + p0[2])).abs() <= self.px(6.0);
        let near_h = (y - (p0[1] + p0[3])).abs() <= self.px(6.0);
        Some(match split {
            Split::Vertical => (true, false),
            Split::Horizontal => (false, true),
            Split::Spreadsheet => (near_v, near_h),
        })
    }

    /// Dragging a split bar moves it (the panes keep at least a tenth of the area).
    fn drag_split_bar(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        match input {
            Input::Press { x, y, .. } => {
                let bars = self.split_bar_at(*x, *y)?;
                self.view4.bar_drag = Some(bars);
                Some(false)
            }
            Input::Drag { x, y } => {
                let (v, h) = self.view4.bar_drag?;
                let a = self.view4.area;
                let bar = self.px(4.8).round();
                if v {
                    let f = (x - a[0] - bar / 2.0) / (a[2] - bar).max(1.0) - 0.5;
                    self.view4.bar_at.0 = f.clamp(-0.4, 0.4);
                }
                if h {
                    let f = (y - a[1] - bar / 2.0) / (a[3] - bar).max(1.0) - 0.5;
                    self.view4.bar_at.1 = f.clamp(-0.4, 0.4);
                }
                self.relayout(m);
                Some(true)
            }
            Input::Release { .. } => self.view4.bar_drag.take().map(|_| true),
            _ => None,
        }
    }

    /// Each frame before page images are prepared: the other panes' pages and AutoScroll.
    pub(crate) fn prepare_view4(&mut self) {
        let panes: Vec<([f32; 4], (f32, f32))> = if self.view4.split.is_some() {
            self.view4
                .panes
                .iter()
                .zip(&self.view4.scrolls)
                .enumerate()
                .filter(|(k, _)| *k != self.view4.active)
                .map(|(_, (r, s))| (*r, *s))
                .collect()
        } else {
            Vec::new()
        };
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_extra_views(panes);
        }
        self.auto_frame();
    }

    /// AutoScroll moves the document by its speed times the time since the last frame.
    fn auto_frame(&mut self) {
        let Some(speed) = self.viewx.auto else {
            self.view4.auto_last = None;
            self.view4.auto_rest = 0.0;
            return;
        };
        let now = Instant::now();
        let Some(last) = self.view4.auto_last.replace(now) else {
            return;
        };
        let dt = now.duration_since(last).as_secs_f32().min(0.25);
        let rate = AUTO_SPEEDS[speed.min(AUTO_SPEEDS.len() - 1)];
        self.view4.auto_rest += self.px(rate) * dt;
        let step = self.view4.auto_rest.floor();
        if step < 1.0 {
            return;
        }
        self.view4.auto_rest -= step;
        if self.view4.text_viewer {
            self.tv_scroll_by(0.0, step);
            return;
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let before = t.view.scroll_state().0;
        let page = t.view.current_page();
        t.view.scroll_by(0.0, step);
        if t.view.scroll_state().0 == before && t.view.current_page() == page {
            // The end of the document: stop.
            self.viewx.auto = None;
            self.on.remove("view.autoScroll");
        }
    }

    /// Frames are wanted without input (AutoScroll moving).
    pub(crate) fn view4_animating(&self) -> bool {
        self.viewx.auto.is_some()
    }

    /// Text Viewer: the document's text on a white sheet; `true` when it painted.
    pub(crate) fn paint_text_viewer(&self, p: &mut dyn Painter) -> bool {
        if !self.view4.text_viewer {
            return false;
        }
        let d = self.view4_area();
        p.fill_rect(d[0], d[1], d[2], d[3], self.doc_canvas());
        let s = self.tv_sheet();
        p.fill_rect(s[0] - 1.0, s[1] - 1.0, s[2] + 1.0, s[3] + 1.0, TV_FRAME);
        p.fill_rect(s[0], s[1], s[2], s[3], super::WHITE);
        p.push_clip(s[0], s[1], s[2], s[3]);
        let st = self.tv_style();
        let lh = self.px(13.6);
        let (sx, sy) = self.view4.tv_scroll;
        let x = s[0] + self.px(25.6) - sx;
        let mut y = s[1] + self.px(25.6) - sy;
        for line in &self.view4.tv_lines {
            if y + lh >= s[1] && y <= s[1] + s[3] && !line.is_empty() {
                p.text(line, x.round(), y.round(), st, TEXT);
            }
            y += lh;
            if y > s[1] + s[3] {
                break;
            }
        }
        p.pop_clip();
        true
    }

    /// The other panes of a split window and the bars between them.
    pub(crate) fn paint_split(&self, p: &mut dyn Painter) {
        if self.view4.split.is_none() {
            return;
        }
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let v = &t.view;
        let a = self.view4.area;
        // Bars: the area's background between the panes.
        p.push_clip(a[0], a[1], a[2], a[3]);
        p.fill_rect(a[0], a[1], a[2], a[3], SPLIT_BAR);
        p.pop_clip();
        for (k, r) in self.view4.panes.iter().enumerate() {
            p.fill_rect(r[0], r[1], r[2], r[3], self.doc_canvas());
            if k == self.view4.active {
                continue;
            }
            let scroll = self.view4.scrolls.get(k).copied().unwrap_or((0.0, 0.0));
            p.push_clip(r[0], r[1], r[2], r[3]);
            p.raw_colours(true);
            for i in v.visible_pages_at(*r, scroll) {
                let Some(pr) = v.page_rect_at(i, *r, scroll) else {
                    continue;
                };
                p.fill_rect(
                    pr[0] - 1.0,
                    pr[1] - 1.0,
                    pr[2] + 2.0,
                    pr[3] + 2.0,
                    super::PAGE_EDGE,
                );
                p.fill_rect(pr[0], pr[1], pr[2], pr[3], self.paper());
                if v.has_image(i) {
                    let (ox, oy) = v.image_offset(i);
                    p.page(t.key_base + i, (pr[0] + ox) as i32, (pr[1] + oy) as i32);
                }
            }
            p.raw_colours(false);
            p.pop_clip();
        }
    }
}

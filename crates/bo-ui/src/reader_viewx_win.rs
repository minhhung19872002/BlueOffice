//! BlueReader View tab additions in the window (round 3, a child of `reader_window`): Night
//! Mode, Reflow, AutoScroll, a separate cover page in facing layouts, rulers and a grid.
use super::{ReaderWindow, TEXT};
use crate::reader_viewx::{paragraphs, ruler_ticks, wrap};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_render::Rgba;

/// Ruler strip, its ticks and the grid lines.
const RULER_BG: Rgba = Rgba(248, 248, 250, 255);
const RULER_EDGE: Rgba = Rgba(199, 204, 217, 255);
const RULER_TICK: Rgba = Rgba(110, 110, 112, 255);
const GRID: Rgba = Rgba(64, 140, 255, 70);
const GRID_MAJOR: Rgba = Rgba(64, 140, 255, 120);
/// AutoScroll speeds (logical pixels per 100 ms tick).
const AUTO_SPEEDS: [f32; 6] = [2.0, 4.0, 7.0, 11.0, 16.0, 24.0];

/// State of the View additions.
#[derive(Debug, Default)]
pub(crate) struct ViewX {
    pub(crate) night: bool,
    pub(crate) reflow: bool,
    /// Reflow: the page shown, its text lines (wrapped for `width`) and the scroll.
    reflow_page: Option<usize>,
    reflow_width: f32,
    reflow_lines: Vec<String>,
    reflow_size: f32,
    reflow_scroll: f32,
    /// AutoScroll speed index while scrolling.
    pub(crate) auto: Option<usize>,
    pub(crate) rulers: bool,
    pub(crate) grid: bool,
}

impl ReaderWindow {
    /// View tab commands of the additions; `true` when `cmd` was one.
    pub(crate) fn command_viewx(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        match cmd {
            "view.night" => {
                self.viewx.night = !self.viewx.night;
                self.sync_viewx();
            }
            "view.reflow" => {
                self.viewx.reflow = !self.viewx.reflow;
                self.viewx.reflow_page = None;
                self.viewx.reflow_scroll = 0.0;
                if self.viewx.reflow {
                    self.reflow_layout(m);
                }
            }
            "view.autoScroll" => {
                self.viewx.auto = match self.viewx.auto {
                    Some(_) => None,
                    None => Some(1),
                };
            }
            "view.cover" => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    let on = !t.view.cover();
                    t.view.set_cover(on);
                }
            }
            "view.rulers" => {
                self.viewx.rulers = !self.viewx.rulers;
                self.layout_doc();
            }
            "view.grid" => self.viewx.grid = !self.viewx.grid,
            "view.reverse" => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    let on = !t.view.reverse();
                    t.view.set_reverse(on);
                }
            }
            "view.wordCount" => self.word_count(),
            "menu.viewSetting" => {
                let items: Vec<crate::popup::MenuItem> = [
                    ("zoom.in", "pdfReader.zoomIn"),
                    ("zoom.out", "pdfReader.zoomOut"),
                    ("zoom.actual", "pdfReader.actualSize"),
                    ("zoom.page", "pdfReader.fitPage"),
                    ("zoom.width", "pdfReader.fitWidth"),
                    ("panel.bookmarks", "pdfReader.bookmarks"),
                    ("panel.pages", "pdfReader.pages"),
                    ("view.rulers", "pdfView.rulers"),
                    ("view.grid", "pdfView.grid"),
                    ("view.fullscreen", "pdfReader.fullScreen"),
                ]
                .iter()
                .map(|(c, k)| crate::popup::MenuItem {
                    label: self.t(k),
                    choice: crate::popup::Choice::Command((*c).to_owned()),
                    checked: (*c == "view.rulers" && self.viewx.rulers)
                        || (*c == "view.grid" && self.viewx.grid),
                    dim: false,
                })
                .collect();
                let r = self.control_rect("menu.viewSetting").unwrap_or([0.0; 4]);
                self.menu = Some((
                    crate::popup::Menu::open(items, r, self.size, self.scale),
                    "menu.viewSetting",
                ));
            }
            _ => return false,
        }
        self.sync_viewx_buttons();
        true
    }

    /// Pressed look of the View toggles.
    fn sync_viewx_buttons(&mut self) {
        let cover = self.tabs.get(self.active).is_some_and(|t| t.view.cover());
        let reverse = self.tabs.get(self.active).is_some_and(|t| t.view.reverse());
        for (id, on) in [
            ("view.night", self.viewx.night),
            ("view.reflow", self.viewx.reflow),
            ("view.autoScroll", self.viewx.auto.is_some()),
            ("view.cover", cover),
            ("view.reverse", reverse),
            ("view.rulers", self.viewx.rulers),
            ("view.grid", self.viewx.grid),
        ] {
            if on {
                self.on.insert(id);
            } else {
                self.on.remove(id);
            }
        }
    }

    /// Pages, words and characters of the document (the reference's Word Count).
    fn word_count(&mut self) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let n = t.view.page_count();
        let (mut words, mut chars, mut spaces) = (0_usize, 0_usize, 0_usize);
        for i in 0..n {
            if let Some(layer) = t.view.text(i) {
                let text = layer.text();
                words += text.split_whitespace().count();
                for c in text.chars() {
                    if c == ' ' {
                        spaces += 1;
                    } else if !c.is_whitespace() {
                        chars += 1;
                    }
                }
            }
        }
        let lines = vec![
            self.strings
                .fmt("pdfView.countPages", &[("n", &n.to_string())]),
            self.strings
                .fmt("pdfView.countWords", &[("n", &words.to_string())]),
            self.strings.fmt(
                "pdfView.countChars",
                &[("n", &(chars + spaces).to_string())],
            ),
            self.strings
                .fmt("pdfView.countCharsNoSpaces", &[("n", &chars.to_string())]),
        ];
        let d = crate::dialog::Dialog::new(
            crate::dialog::DialogKind::WordCount,
            self.t("pdfView.wordCount"),
            Vec::new(),
            vec![(self.t("pdfReader.ok"), crate::dialog::DialogButton::Ok)],
            lines,
        );
        self.dialog = Some((d, None));
    }

    /// Night Mode follows the window to every document.
    pub(crate) fn sync_viewx(&mut self) {
        let night = self.viewx.night;
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.set_night(night);
        }
    }

    /// Input of the additions first: `Some(redraw)` when handled.
    pub(crate) fn route_viewx(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(speed) = self.viewx.auto {
            match input {
                Input::Key(Key::Escape, _) => {
                    self.viewx.auto = None;
                    self.sync_viewx_buttons();
                    return Some(true);
                }
                Input::Key(Key::Down, _) => {
                    self.viewx.auto = Some((speed + 1).min(AUTO_SPEEDS.len() - 1));
                    return Some(false);
                }
                Input::Key(Key::Up, _) => {
                    self.viewx.auto = Some(speed.saturating_sub(1));
                    return Some(false);
                }
                // A click stops scrolling (and does what it does).
                Input::Press { .. } => {
                    self.viewx.auto = None;
                    self.sync_viewx_buttons();
                }
                _ => {}
            }
        }
        if self.viewx.reflow {
            let d = self.doc_rect;
            match input {
                Input::Wheel(dy) if super::inside(d, self.pointer.0, self.pointer.1) => {
                    self.reflow_scroll_by(-dy * self.px(1.0), m);
                    return Some(true);
                }
                Input::Key(Key::PageDown | Key::Down, _) => {
                    let step = if matches!(input, Input::Key(Key::PageDown, _)) {
                        d[3] * 0.9
                    } else {
                        self.viewx.reflow_size * 1.5
                    };
                    self.reflow_scroll_by(step, m);
                    return Some(true);
                }
                Input::Key(Key::PageUp | Key::Up, _) => {
                    let step = if matches!(input, Input::Key(Key::PageUp, _)) {
                        d[3] * 0.9
                    } else {
                        self.viewx.reflow_size * 1.5
                    };
                    self.reflow_scroll_by(-step, m);
                    return Some(true);
                }
                _ => {}
            }
        }
        None
    }

    /// A timer tick: AutoScroll moves the document; `true` when it moved.
    pub(crate) fn tick_viewx(&mut self, m: &dyn TextMeasure) -> bool {
        let Some(speed) = self.viewx.auto else {
            return false;
        };
        let dy = self.px(AUTO_SPEEDS[speed.min(AUTO_SPEEDS.len() - 1)]);
        if self.viewx.reflow {
            self.reflow_scroll_by(dy, m);
            return true;
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let before = t.view.scroll_state().0;
        let page = t.view.current_page();
        t.view.scroll_by(0.0, dy);
        let moved = t.view.scroll_state().0 != before || t.view.current_page() != page;
        if !moved {
            // The end of the document: stop.
            self.viewx.auto = None;
            self.sync_viewx_buttons();
            return true;
        }
        true
    }

    fn reflow_scroll_by(&mut self, dy: f32, m: &dyn TextMeasure) {
        self.reflow_layout(m);
        let lh = (self.viewx.reflow_size * 1.45).round();
        let content = self.viewx.reflow_lines.len() as f32 * lh + self.px(48.0);
        let max = (content - self.doc_rect[3]).max(0.0);
        let y = self.viewx.reflow_scroll + dy;
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let page = self.viewx.reflow_page.unwrap_or(0);
        if y > max + 1.0 && self.viewx.reflow_scroll >= max - 0.5 && page + 1 < t.view.page_count()
        {
            t.view.go_to(page + 1, None);
            self.viewx.reflow_scroll = 0.0;
            self.reflow_layout(m);
        } else if y < -1.0 && self.viewx.reflow_scroll <= 0.5 && page > 0 {
            t.view.go_to(page - 1, None);
            self.viewx.reflow_page = None;
            self.reflow_layout(m);
            let content = self.viewx.reflow_lines.len() as f32 * lh + self.px(48.0);
            self.viewx.reflow_scroll = (content - self.doc_rect[3]).max(0.0);
        } else {
            self.viewx.reflow_scroll = y.clamp(0.0, max);
        }
    }

    /// Text size of reflowed text: the page's usual text size at the zoom.
    fn reflow_style(&self) -> TextStyle {
        TextStyle {
            size: self.viewx.reflow_size,
            bold: false,
        }
    }

    /// Wrap the current page's text to the document area (again when the page or the width
    /// changed).
    pub(crate) fn reflow_layout(&mut self, m: &dyn TextMeasure) {
        if !self.viewx.reflow {
            return;
        }
        let width = (self.doc_rect[2] - self.px(64.0)).max(self.px(120.0));
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let page = t.view.current_page();
        if self.viewx.reflow_page == Some(page) && (self.viewx.reflow_width - width).abs() < 0.5 {
            return;
        }
        let zoom_px = t.view.px_per_pt();
        let (text, size) = match t.view.text(page) {
            Some(layer) => {
                let mut sizes: Vec<f32> = layer.chars().iter().map(|c| c.size).collect();
                sizes.sort_by(f32::total_cmp);
                let median = sizes.get(sizes.len() / 2).copied().unwrap_or(11.0);
                (layer.text(), median)
            }
            None => (String::new(), 11.0),
        };
        // Body text at the page's size and zoom, at least 12 px.
        let px = (size * zoom_px).clamp(self.px(12.0), self.px(48.0)).round();
        let st = TextStyle {
            size: px,
            bold: false,
        };
        let paras = paragraphs(&text);
        let lines = wrap(&paras, width, &|s: &str| m.measure(s, st).0);
        self.viewx.reflow_page = Some(page);
        self.viewx.reflow_width = width;
        self.viewx.reflow_lines = lines;
        self.viewx.reflow_size = px;
    }

    /// Room for the rulers along the top and left of the document area.
    pub(crate) fn viewx_layout(&mut self) {
        if self.viewx.rulers {
            let r = self.px(16.0);
            let d = &mut self.doc_rect;
            *d = [d[0] + r, d[1] + r, (d[2] - r).max(1.0), (d[3] - r).max(1.0)];
        }
        // Reflowed text wraps to the new width when next painted.
        self.viewx.reflow_page = None;
    }

    /// Canvas colour of the document area (the reference keeps it in Night Mode).
    pub(crate) fn doc_canvas(&self) -> Rgba {
        self.look.canvas
    }

    /// Paper colour under page images.
    pub(crate) fn paper(&self) -> Rgba {
        if self.viewx.night {
            crate::reader_viewx::NIGHT_PAPER
        } else {
            super::WHITE
        }
    }

    /// Reflow replaces the page drawing; `true` when it painted the document area.
    pub(crate) fn paint_reflow(&self, p: &mut dyn Painter) -> bool {
        if !self.viewx.reflow || self.viewx.reflow_page.is_none() {
            return false;
        }
        let d = self.doc_rect;
        let (bg, ink) = if self.viewx.night {
            (
                crate::reader_viewx::NIGHT_PAPER,
                crate::reader_viewx::NIGHT_TEXT,
            )
        } else {
            (super::WHITE, TEXT)
        };
        p.fill_rect(d[0], d[1], d[2], d[3], bg);
        p.push_clip(d[0], d[1], d[2], d[3]);
        let st = self.reflow_style();
        let lh = (st.size * 1.45).round();
        let x = d[0] + self.px(32.0);
        let mut y = d[1] + self.px(24.0) - self.viewx.reflow_scroll;
        for line in &self.viewx.reflow_lines {
            if y + lh >= d[1] && y <= d[1] + d[3] && !line.is_empty() {
                p.text(line, x, y.round(), st, ink);
            }
            y += lh;
        }
        p.pop_clip();
        true
    }

    /// Rulers (inches from the current page's top-left corner) and the grid over the pages.
    pub(crate) fn paint_viewx_overlay(&self, p: &mut dyn Painter) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let v = &t.view;
        let d = self.doc_rect;
        let inch = v.px_per_pt() * 72.0;
        if self.viewx.grid && !self.viewx.reflow {
            p.push_clip(d[0], d[1], d[2], d[3]);
            for i in v.visible_pages() {
                let Some(r) = v.page_rect(i) else { continue };
                // Lines every quarter inch, darker every inch.
                let step = inch / 4.0;
                if step < 3.0 {
                    continue;
                }
                let mut k = 1;
                while k as f32 * step < r[2] {
                    let x = (r[0] + k as f32 * step).round();
                    let c = if k % 4 == 0 { GRID_MAJOR } else { GRID };
                    p.fill_rect(x, r[1], 1.0, r[3], c);
                    k += 1;
                }
                let mut k = 1;
                while k as f32 * step < r[3] {
                    let y = (r[1] + k as f32 * step).round();
                    let c = if k % 4 == 0 { GRID_MAJOR } else { GRID };
                    p.fill_rect(r[0], y, r[2], 1.0, c);
                    k += 1;
                }
            }
            p.pop_clip();
        }
        if !self.viewx.rulers {
            return;
        }
        let rs = self.px(16.0);
        let origin = v
            .page_rect(v.current_page())
            .map_or((d[0], d[1]), |r| (r[0], r[1]));
        let st = TextStyle {
            size: self.px(8.0),
            bold: false,
        };
        // Top ruler.
        let (tx, ty) = (d[0], d[1] - rs);
        p.fill_rect(tx - rs, ty, d[2] + rs, rs, RULER_BG);
        p.fill_rect(tx - rs, ty + rs - 1.0, d[2] + rs, 1.0, RULER_EDGE);
        p.push_clip(tx, ty, d[2], rs);
        for (x, level, unit) in ruler_ticks(origin.0, inch, tx, tx + d[2]) {
            let h = [rs * 0.55, rs * 0.35, rs * 0.2][usize::from(level)];
            p.fill_rect(x.round(), ty + rs - 1.0 - h, 1.0, h, RULER_TICK);
            if level == 0 {
                p.text(&unit.to_string(), x.round() + 2.0, ty, st, RULER_TICK);
            }
        }
        p.pop_clip();
        // Left ruler.
        let (lx, ly) = (d[0] - rs, d[1]);
        p.fill_rect(lx, ly, rs, d[3], RULER_BG);
        p.fill_rect(lx + rs - 1.0, ly - rs, 1.0, d[3] + rs, RULER_EDGE);
        p.push_clip(lx, ly, rs, d[3]);
        for (y, level, unit) in ruler_ticks(origin.1, inch, ly, ly + d[3]) {
            let w = [rs * 0.55, rs * 0.35, rs * 0.2][usize::from(level)];
            p.fill_rect(lx + rs - 1.0 - w, y.round(), w, 1.0, RULER_TICK);
            if level == 0 {
                p.text(&unit.to_string(), lx + 1.0, y.round() + 1.0, st, RULER_TICK);
            }
        }
        p.pop_clip();
    }
}

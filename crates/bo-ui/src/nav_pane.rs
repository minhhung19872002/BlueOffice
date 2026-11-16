//! Navigation pane of BlueDoc (View › Navigation Pane, Ctrl+F), laid out after a desktop word
//! processor's: title with a close button, a search box, Headings / Pages / Results tabs and
//! the list of the chosen tab, on the canvas grey with a line on its right edge.
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Key, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Measured on the reference at 125 %: pane 437 px wide on grey 230 with a 191 edge, search
/// box white framed 171, current row 194,213,242 framed 163,189,227, list text 68.
const PANE_BG: Rgba = Rgba(230, 230, 230, 255);
const PANE_EDGE: Rgba = Rgba(191, 191, 191, 255);
const BOX_EDGE: Rgba = Rgba(171, 171, 171, 255);
const ROW_TEXT: Rgba = Rgba(68, 68, 68, 255);
const MUTED: Rgba = Rgba(118, 118, 118, 255);

/// Tab of the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavTab {
    Headings,
    Pages,
    Results,
}

/// Page thumbnails side by side in the Pages tab.
const THUMB_COLS: usize = 2;

const TABS: [(NavTab, &str); 3] = [
    (NavTab::Headings, "views.nav.headings"),
    (NavTab::Pages, "views.nav.pages"),
    (NavTab::Results, "views.nav.results"),
];

/// A heading listed in the pane.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Heading {
    /// Body paragraph id.
    pub(crate) para: usize,
    /// 0-based outline level.
    pub(crate) level: u8,
    pub(crate) text: String,
    /// 0-based page.
    pub(crate) page: usize,
}

/// What a press or key in the pane asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NavAction {
    /// Close the pane.
    Close,
    /// Put the caret at the start of paragraph `para`.
    Heading(usize),
    /// Show page `n` (0-based).
    Page(usize),
    /// Select search result `n`.
    Result(usize),
    /// Search for the query typed in the box.
    Search,
    /// Redraw only.
    Redraw,
}

/// The navigation pane.
pub(crate) struct NavPane {
    pub(crate) headings: Vec<Heading>,
    /// Search results: (before, match, after) texts.
    pub(crate) results: Vec<(String, String, String)>,
    /// Number of pages (Pages tab).
    pub(crate) pages: usize,
    /// Height / width of the pages (thumbnail shape).
    pub(crate) aspect: f32,
    pub(crate) query: String,
    pub(crate) tab: NavTab,
    /// The search box has the keyboard.
    pub(crate) focus: bool,
    /// Highlighted row of the current tab (heading at the caret, page shown, result chosen).
    pub(crate) current: Option<usize>,
    /// A search ran (the Results tab shows "nothing matches" when empty).
    pub(crate) searched: bool,
    scale: f32,
    rect: [f32; 4],
    scroll: f32,
    hover: Option<usize>,
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl NavPane {
    pub(crate) fn new() -> Self {
        Self {
            headings: Vec::new(),
            results: Vec::new(),
            pages: 0,
            aspect: 1.414,
            query: String::new(),
            tab: NavTab::Headings,
            focus: false,
            current: None,
            searched: false,
            scale: 1.0,
            rect: [0.0; 4],
            scroll: 0.0,
            hover: None,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Width of the pane at display `scale` (350 logical px, as the reference).
    pub(crate) fn width(scale: f32) -> f32 {
        (350.0 * scale).round()
    }

    /// Place the pane at the left of the window, from `top` to `bottom`.
    pub(crate) fn layout(&mut self, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        self.rect = [0.0, top, Self::width(scale), (bottom - top).max(0.0)];
        self.clamp();
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(32.0);
        [
            self.rect[2] - self.px(14.0) - s,
            self.rect[1] + self.px(10.0),
            s,
            s,
        ]
    }

    fn search_rect(&self) -> [f32; 4] {
        [
            self.px(12.0),
            self.rect[1] + self.px(56.0),
            self.rect[2] - self.px(28.0),
            self.px(28.0),
        ]
    }

    fn tab_rects(&self, text: Option<&dyn TextMeasure>, strings: &Strings) -> [[f32; 4]; 3] {
        let y = self.rect[1] + self.px(100.0);
        let h = self.px(32.0);
        let style = self.tab_style(true);
        let mut x = self.px(12.0);
        let mut out = [[0.0; 4]; 3];
        for (i, (_, key)) in TABS.iter().enumerate() {
            let label = strings.get(key);
            let w = text.map_or(label.chars().count() as f32 * style.size * 0.55, |t| {
                t.measure(label, style).0
            });
            out[i] = [x, y, w + self.px(4.0), h];
            x += w + self.px(28.0);
        }
        out
    }

    fn tab_style(&self, bold: bool) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold,
        }
    }

    fn list_top(&self) -> f32 {
        self.rect[1] + self.px(142.0)
    }

    fn row_h(&self) -> f32 {
        if self.tab == NavTab::Pages {
            self.thumb_size().1 + self.px(30.0)
        } else {
            self.px(22.0)
        }
    }

    fn rows(&self) -> usize {
        match self.tab {
            NavTab::Headings => self.headings.len(),
            NavTab::Pages => self.pages.div_ceil(THUMB_COLS),
            NavTab::Results => self.results.len(),
        }
    }

    /// Size of a page thumbnail (device px): two to a row, as the pane shows pages.
    pub(crate) fn thumb_size(&self) -> (f32, f32) {
        let gap = self.px(16.0);
        let w = ((self.rect[2] - gap * (THUMB_COLS as f32 + 1.0)) / THUMB_COLS as f32)
            .floor()
            .max(1.0);
        (w, (w * self.aspect).floor())
    }

    /// The page thumbnails in view: page index and rectangle (device px).
    pub(crate) fn thumb_rects(&self) -> Vec<(usize, [f32; 4])> {
        if self.tab != NavTab::Pages || self.rect[3] <= 0.0 {
            return Vec::new();
        }
        let (w, h) = self.thumb_size();
        let gap = self.px(16.0);
        let top = self.list_top();
        let bottom = self.rect[1] + self.rect[3];
        let rh = self.row_h();
        (0..self.pages)
            .filter_map(|i| {
                let (r, c) = (i / THUMB_COLS, i % THUMB_COLS);
                let x = (gap + c as f32 * (w + gap)).round();
                let y = (top + r as f32 * rh - self.scroll + self.px(6.0)).round();
                (y + h > top && y < bottom).then_some((i, [x, y, w, h]))
            })
            .collect()
    }

    /// The page whose thumbnail is at (`x`, `y`).
    fn thumb_at(&self, x: f32, y: f32) -> Option<usize> {
        self.thumb_rects()
            .into_iter()
            .find(|(_, r)| inside([r[0], r[1], r[2], r[3] + self.px(24.0)], x, y))
            .map(|(i, _)| i)
    }

    fn clamp(&mut self) {
        let view = (self.rect[1] + self.rect[3] - self.list_top()).max(0.0);
        let content = self.rows() as f32 * self.row_h() + self.px(8.0);
        self.scroll = self.scroll.clamp(0.0, (content - view).max(0.0));
    }

    fn row_at(&self, x: f32, y: f32) -> Option<usize> {
        let top = self.list_top();
        if x < 0.0 || x >= self.rect[2] || y < top || y >= self.rect[1] + self.rect[3] {
            return None;
        }
        let i = ((y - top + self.scroll) / self.row_h()).floor() as usize;
        (i < self.rows()).then_some(i)
    }

    /// Whether window point (`x`, `y`) is over the pane.
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    /// Switch to `tab`.
    pub(crate) fn show(&mut self, tab: NavTab) {
        if self.tab != tab {
            self.tab = tab;
            self.scroll = 0.0;
            self.hover = None;
            self.current = None;
        }
    }

    /// Bring row `i` of the current tab into view.
    pub(crate) fn reveal(&mut self, i: usize) {
        let view = (self.rect[1] + self.rect[3] - self.list_top()).max(0.0);
        let (top, h) = (i as f32 * self.row_h(), self.row_h());
        if top < self.scroll {
            self.scroll = top;
        } else if top + h > self.scroll + view {
            self.scroll = top + h - view;
        }
        self.clamp();
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = if self.tab == NavTab::Pages {
            self.thumb_at(x, y)
        } else {
            self.row_at(x, y)
        };
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp();
        self.scroll != before
    }

    /// A press at (`x`, `y`) in the pane.
    pub(crate) fn press(&mut self, x: f32, y: f32, strings: &Strings) -> NavAction {
        if inside(self.close_rect(), x, y) {
            return NavAction::Close;
        }
        if inside(self.search_rect(), x, y) {
            self.focus = true;
            return NavAction::Redraw;
        }
        self.focus = false;
        for (i, r) in self.tab_rects(None, strings).iter().enumerate() {
            let hit = [r[0] - self.px(6.0), r[1], r[2] + self.px(12.0), r[3]];
            if inside(hit, x, y) {
                self.show(TABS[i].0);
                return NavAction::Redraw;
            }
        }
        let at = if self.tab == NavTab::Pages {
            self.thumb_at(x, y)
        } else {
            self.row_at(x, y)
        };
        match at {
            Some(i) => {
                self.current = Some(i);
                match self.tab {
                    NavTab::Headings => NavAction::Heading(self.headings[i].para),
                    NavTab::Pages => NavAction::Page(i),
                    NavTab::Results => NavAction::Result(i),
                }
            }
            None => NavAction::Redraw,
        }
    }

    /// Text typed while the search box has the keyboard.
    pub(crate) fn text(&mut self, s: &str) {
        self.query
            .extend(s.chars().filter(|c| !c.is_control() || *c == '\t'));
    }

    /// A key while the search box has the keyboard.
    pub(crate) fn key(&mut self, key: Key) -> NavAction {
        match key {
            Key::Backspace => {
                self.query.pop();
                NavAction::Redraw
            }
            Key::Enter => NavAction::Search,
            Key::Escape => {
                self.focus = false;
                NavAction::Redraw
            }
            Key::Down if !self.results.is_empty() => {
                let i = self.current.map_or(0, |i| (i + 1) % self.results.len());
                self.current = Some(i);
                self.reveal(i);
                NavAction::Result(i)
            }
            Key::Up if !self.results.is_empty() => {
                let n = self.results.len();
                let i = self.current.map_or(n - 1, |i| (i + n - 1) % n);
                self.current = Some(i);
                self.reveal(i);
                NavAction::Result(i)
            }
            _ => NavAction::Redraw,
        }
    }

    /// Paint the pane.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme, strings: &Strings) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, PANE_BG);
        p.fill_rect(x + w - 1.0, y, 1.0, h, PANE_EDGE);
        // Title and close button.
        let title = TextStyle {
            size: self.px(18.0),
            bold: false,
        };
        p.text(
            strings.get("views.nav.title"),
            self.px(14.0),
            y + self.px(12.0),
            title,
            t.text,
        );
        let c = self.close_rect();
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.0));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((t.text, self.scale.max(1.0))),
        );
        // Search box.
        let b = self.search_rect();
        let edge = if self.focus { t.accent } else { BOX_EDGE };
        p.fill_rect(b[0], b[1], b[2], b[3], edge);
        p.fill_rect(
            b[0] + 1.0,
            b[1] + 1.0,
            b[2] - 2.0,
            b[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let ty = |r: [f32; 4], st: TextStyle| r[1] + (r[3] - st.size * 1.33) / 2.0;
        if self.query.is_empty() && !self.focus {
            p.text(
                strings.get("views.nav.search"),
                b[0] + self.px(8.0),
                ty(b, style),
                style,
                MUTED,
            );
        } else {
            p.push_clip(b[0] + 2.0, b[1], b[2] - 4.0, b[3]);
            p.text(
                &self.query,
                b[0] + self.px(8.0),
                ty(b, style),
                style,
                t.text,
            );
            p.pop_clip();
        }
        // Magnifier at the right of the box (own drawing).
        let (mx, my, r) = (
            b[0] + b[2] - self.px(16.0),
            b[1] + b[3] / 2.0 - self.px(1.0),
            self.px(4.0),
        );
        let lw = self.scale.max(1.0);
        p.path(
            &[
                PathSeg::M(mx - r, my),
                PathSeg::Q(mx - r, my - r, mx, my - r),
                PathSeg::Q(mx + r, my - r, mx + r, my),
                PathSeg::Q(mx + r, my + r, mx, my + r),
                PathSeg::Q(mx - r, my + r, mx - r, my),
                PathSeg::M(mx - r * 0.7, my + r * 0.7),
                PathSeg::L(mx - r * 1.9, my + r * 1.9),
            ],
            None,
            Some((t.text, lw)),
        );
        // Tabs: the current one bold in the accent colour over a 3 px bar.
        for (i, r) in self.tab_rects(None, strings).iter().enumerate() {
            let (tab, key) = TABS[i];
            let on = tab == self.tab;
            let st = self.tab_style(on);
            let color = if on { t.accent_text } else { t.text };
            p.text(strings.get(key), r[0], ty(*r, st), st, color);
            if on {
                p.fill_rect(r[0], r[1] + r[3], r[2], self.px(3.0), t.accent);
            }
        }
        // List.
        let top = self.list_top();
        p.push_clip(0.0, top, w - 1.0, y + h - top);
        let rh = self.row_h();
        let band = |p: &mut dyn Painter, ry: f32, strong: bool| {
            let (fill, frame) = if strong {
                (mix(t.accent, 0.28), mix(t.accent, 0.42))
            } else {
                (mix(t.accent, 0.12), mix(t.accent, 0.12))
            };
            let bx = self.px(12.0);
            let bw = w - self.px(28.0);
            p.fill_rect(bx, ry, bw, rh, frame);
            p.fill_rect(bx + 1.0, ry + 1.0, bw - 2.0, rh - 2.0, fill);
        };
        let empty = match self.tab {
            NavTab::Headings if self.headings.is_empty() => Some("views.nav.noHeadings"),
            NavTab::Results if self.searched && self.results.is_empty() => {
                Some("views.nav.noResults")
            }
            _ => None,
        };
        if let Some(key) = empty {
            p.text(
                strings.get(key),
                self.px(16.0),
                top + self.px(4.0),
                style,
                MUTED,
            );
        }
        // Pages: thumbnails two to a row, the page shown framed in the accent colour,
        // its number under it (the page images come from the host's thumbnail store).
        if self.tab == NavTab::Pages {
            for (i, r) in self.thumb_rects() {
                let [tx, ty0, tw, th] = r;
                let (frame, k) = if self.current == Some(i) {
                    (t.accent, self.px(2.0).max(1.0))
                } else if self.hover == Some(i) {
                    (mix(t.accent, 0.42), 1.0)
                } else {
                    (Rgba(200, 200, 200, 255), 1.0)
                };
                p.fill_rect(tx - k, ty0 - k, tw + 2.0 * k, th + 2.0 * k, frame);
                p.fill_rect(tx, ty0, tw, th, Rgba(255, 255, 255, 255));
                p.page(crate::THUMB_KEY + i, tx as i32, ty0 as i32);
                let label = (i + 1).to_string();
                let lw = label.chars().count() as f32 * style.size * 0.55;
                p.text(
                    &label,
                    (tx + (tw - lw) / 2.0).round(),
                    ty0 + th + self.px(4.0),
                    style,
                    ROW_TEXT,
                );
            }
            p.pop_clip();
            p.pop_clip();
            return;
        }
        for i in 0..self.rows() {
            let ry = top + i as f32 * rh - self.scroll;
            if ry + rh < top || ry > y + h {
                continue;
            }
            if self.current == Some(i) {
                band(p, ry, true);
            } else if self.hover == Some(i) {
                band(p, ry, false);
            }
            let tx = ty([0.0, ry, 0.0, rh], style);
            match self.tab {
                NavTab::Headings => {
                    let hd = &self.headings[i];
                    let indent = self.px(18.0) + f32::from(hd.level) * self.px(16.0);
                    p.text(&hd.text, indent, tx, style, ROW_TEXT);
                }
                NavTab::Pages => {
                    let label = strings.fmt("views.nav.page", &[("n", &(i + 1).to_string())]);
                    p.text(&label, self.px(18.0), tx, style, ROW_TEXT);
                }
                NavTab::Results => {
                    let (before, hit, after) = &self.results[i];
                    let bold = TextStyle {
                        bold: true,
                        ..style
                    };
                    // Context before, the match in bold, context after (estimated widths:
                    // the pane has no text measure while painting).
                    let mut cx = self.px(18.0);
                    let tail: String = before
                        .chars()
                        .rev()
                        .take(18)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    let tail = if tail.len() < before.len() {
                        format!("…{tail}")
                    } else {
                        tail
                    };
                    p.text(&tail, cx, tx, style, ROW_TEXT);
                    cx += tail.chars().count() as f32 * style.size * 0.5;
                    p.text(hit, cx, tx, bold, t.text);
                    cx += hit.chars().count() as f32 * style.size * 0.58;
                    p.text(after, cx, tx, style, ROW_TEXT);
                }
            }
        }
        p.pop_clip();
        p.pop_clip();
    }
}

/// `accent` over white at `k`.
fn mix(accent: Rgba, k: f32) -> Rgba {
    let m = |a: u8| (f32::from(a) * k + 255.0 * (1.0 - k)).round() as u8;
    Rgba(m(accent.0), m(accent.1), m(accent.2), 255)
}

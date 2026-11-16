//! BlueDoc widgets of parity round 3, measured black-box on the reference word processor at
//! 125 % (docs/ui/REPORT.md rounds 65+): the Paste Options drop-down of the Paste split
//! button, the Styles pane (Alt+Ctrl+Shift+S) and the Restrict Editing pane. Icon shapes are
//! BlueOffice's own drawings.
use crate::theme::Theme;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
/// Header band of the Paste Options menu (measured 235).
const BAND: Rgba = Rgba(0xeb, 0xeb, 0xeb, 255);
/// Menu frame (measured 148).
const FRAME: Rgba = Rgba(0x94, 0x94, 0x94, 255);
/// Separator under the icon row (measured 230).
const SEP: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
/// Hovered menu row / pane row.
const HOVER: Rgba = Rgba(0xd5, 0xe1, 0xf2, 255);
/// Pane background (the reference's task panes: 243) and the list's white box.
const PANE: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
const PANE_EDGE: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
/// Current style frame in the Styles pane (dark blue, as the reference).
const CURRENT: Rgba = Rgba(0x2b, 0x57, 0x9a, 255);
const LINK: Rgba = Rgba(0x1e, 0x5c, 0xb3, 255);
const HEADING_LINE: Rgba = Rgba(0xd9, 0xd9, 0xd9, 255);

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn ui_font(size: f32, weight: u16) -> Font<'static> {
    Font {
        family: "Segoe UI",
        size,
        weight,
        italic: false,
        underline: false,
    }
}

fn cross(p: &mut dyn Painter, r: [f32; 4], k: f32, ink: Rgba, w: f32) {
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    p.path(
        &[
            PathSeg::M(cx - k, cy - k),
            PathSeg::L(cx + k, cy + k),
            PathSeg::M(cx + k, cy - k),
            PathSeg::L(cx - k, cy + k),
        ],
        None,
        Some((ink, w)),
    );
}

fn chevron(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, ink: Rgba, w: f32) {
    p.path(
        &[
            PathSeg::M(cx - k, cy - k / 2.0),
            PathSeg::L(cx, cy + k / 2.0),
            PathSeg::L(cx + k, cy - k / 2.0),
        ],
        None,
        Some((ink, w)),
    );
}

/// Check box of the panes (13 px at 100 %, dark frame as the reference's task panes).
fn check_box(p: &mut dyn Painter, r: [f32; 4], on: bool, enabled: bool, s: f32) {
    let edge = if enabled {
        Rgba(0x33, 0x33, 0x33, 255)
    } else {
        Rgba(0xbc, 0xbc, 0xbc, 255)
    };
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
    if on {
        let q = r[2] / 4.0;
        p.path(
            &[
                PathSeg::M(r[0] + q * 0.9, r[1] + r[3] / 2.0),
                PathSeg::L(r[0] + r[2] / 2.0 - q / 3.0, r[1] + r[3] - q * 1.1),
                PathSeg::L(r[0] + r[2] - q * 0.8, r[1] + q),
            ],
            None,
            Some((INK, (1.6 * s).max(1.4))),
        );
    }
}

// ---------------------------------------------------------------------------------------
// Paste Options menu.

/// What the Paste Options menu chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PastePick {
    /// Paste with a mode (`keep`, `merge`, `text`).
    Mode(&'static str),
    /// Paste Special… dialog.
    Special,
    /// Set Default Paste… dialog.
    Defaults,
}

/// Paste modes of the icon row, in the reference's order.
pub(crate) const PASTE_MODES: [&str; 3] = ["keep", "merge", "text"];

/// The drop-down of the Paste split button: "Paste Options:" over a row of three paste
/// modes, then Paste Special… and Set Default Paste… (229 × 155 px at 125 %).
pub(crate) struct PasteMenu {
    pub(crate) rect: [f32; 4],
    scale: f32,
    hover: Option<usize>,
    /// Labels: header, the three mode names, Paste Special…, Set Default Paste….
    labels: [String; 6],
}

impl PasteMenu {
    /// Open under the Paste button `anchor` (window pixels).
    pub(crate) fn open(anchor: [f32; 4], labels: [String; 6], scale: f32) -> Self {
        let w = (183.0 * scale).round();
        let h = (124.0 * scale).round();
        Self {
            rect: [anchor[0], anchor[1] + anchor[3], w, h],
            scale,
            hover: None,
            labels,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Icon buttons of the row (35 px squares at 100 %, from x = 2).
    fn icon_rect(&self, i: usize) -> [f32; 4] {
        let [x, y, ..] = self.rect;
        [
            x + self.px(2.0) + i as f32 * self.px(40.0),
            y + self.px(28.0),
            self.px(35.0),
            self.px(35.0),
        ]
    }

    fn row_rect(&self, i: usize) -> [f32; 4] {
        let [x, y, w, _] = self.rect;
        [
            x + 1.0,
            y + self.px(66.0) + i as f32 * self.px(29.0),
            w - 2.0,
            self.px(29.0),
        ]
    }

    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        (0..3)
            .find(|i| inside(self.icon_rect(*i), x, y))
            .or_else(|| {
                (0..2)
                    .find(|i| inside(self.row_rect(*i), x, y))
                    .map(|i| i + 3)
            })
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// The entry under (x, y).
    pub(crate) fn pick(&self, x: f32, y: f32) -> Option<PastePick> {
        Some(match self.hit(x, y)? {
            i @ 0..=2 => PastePick::Mode(PASTE_MODES[i]),
            3 => PastePick::Special,
            _ => PastePick::Defaults,
        })
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        let s = self.scale;
        for (k, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (k * s).round();
            p.fill_rect(
                x - o + 2.0 * s,
                y - o + 3.0 * s,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, self.px(27.0), BAND);
        let bold = ui_font(self.px(12.0), 700);
        p.text_font(
            &self.labels[0],
            x + self.px(9.0),
            y + self.px(5.0),
            &bold,
            Rgba(0x44, 0x44, 0x44, 255),
        );
        for i in 0..3 {
            let r = self.icon_rect(i);
            if self.hover == Some(i) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0, 0, 0, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xc6, 0xc6, 0xc6, 255),
                );
            }
            paste_icon(
                p,
                PASTE_MODES[i],
                r[0] + self.px(5.0),
                r[1] + self.px(5.0),
                self.px(25.0),
            );
        }
        p.fill_rect(x + 1.0, y + self.px(64.0), w - 2.0, 1.0, SEP);
        let font = ui_font(self.px(12.0), 400);
        for i in 0..2 {
            let r = self.row_rect(i);
            if self.hover == Some(i + 3) {
                p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
            }
            p.text_font(
                &self.labels[4 + i],
                x + self.px(37.0),
                r[1] + self.px(6.0),
                &font,
                INK,
            );
        }
    }
}

/// BlueOffice's paste mode icons: a clipboard with a brush (keep the source look), with a
/// bent arrow (take the surrounding formatting) or with a plain "A" (text only).
pub(crate) fn paste_icon(p: &mut dyn Painter, mode: &str, x: f32, y: f32, size: f32) {
    let u = size / 24.0;
    let r = |x0: f32, y0: f32, w: f32, h: f32, c: Rgba, p: &mut dyn Painter| {
        p.fill_rect(x + x0 * u, y + y0 * u, w * u, h * u, c);
    };
    let board = Rgba(0xc8, 0x8a, 0x3c, 255);
    let paper = WHITE;
    r(2.0, 3.0, 14.0, 18.0, board, p);
    r(3.5, 5.0, 11.0, 14.5, paper, p);
    r(6.0, 1.5, 6.0, 3.5, Rgba(0x6e, 0x6e, 0x6e, 255), p);
    for k in 0..3 {
        r(
            5.0,
            8.0 + k as f32 * 3.0,
            7.0,
            1.2,
            Rgba(0x9a, 0x9a, 0x9a, 255),
            p,
        );
    }
    let blue = Rgba(0x2f, 0x6f, 0xba, 255);
    match mode {
        "keep" => {
            // Brush handle and bristles.
            p.path(
                &[
                    PathSeg::M(x + 22.0 * u, y + 9.0 * u),
                    PathSeg::L(x + 15.0 * u, y + 17.0 * u),
                ],
                None,
                Some((Rgba(0x5a, 0x3a, 0x22, 255), 2.2 * u)),
            );
            p.path(
                &[
                    PathSeg::M(x + 14.5 * u, y + 16.0 * u),
                    PathSeg::L(x + 17.0 * u, y + 18.5 * u),
                    PathSeg::L(x + 13.0 * u, y + 23.0 * u),
                    PathSeg::L(x + 10.0 * u, y + 20.5 * u),
                    PathSeg::Z,
                ],
                Some(blue),
                None,
            );
        }
        "merge" => {
            p.path(
                &[
                    PathSeg::M(x + 12.0 * u, y + 14.0 * u),
                    PathSeg::L(x + 12.0 * u, y + 20.0 * u),
                    PathSeg::L(x + 20.0 * u, y + 20.0 * u),
                ],
                None,
                Some((blue, 2.0 * u)),
            );
            p.path(
                &[
                    PathSeg::M(x + 19.0 * u, y + 16.5 * u),
                    PathSeg::L(x + 23.0 * u, y + 20.0 * u),
                    PathSeg::L(x + 19.0 * u, y + 23.5 * u),
                    PathSeg::Z,
                ],
                Some(blue),
                None,
            );
        }
        _ => {
            r(12.0, 12.0, 11.0, 11.0, Rgba(0x40, 0x40, 0x40, 255), p);
            r(13.0, 13.0, 9.0, 9.0, paper, p);
            p.path(
                &[
                    PathSeg::M(x + 14.5 * u, y + 21.0 * u),
                    PathSeg::L(x + 17.5 * u, y + 14.0 * u),
                    PathSeg::L(x + 20.5 * u, y + 21.0 * u),
                    PathSeg::M(x + 15.7 * u, y + 18.5 * u),
                    PathSeg::L(x + 19.3 * u, y + 18.5 * u),
                ],
                None,
                Some((INK, 1.4 * u)),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Styles pane.

/// One style of the Styles pane.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneStyle {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `¶` paragraph, `a` character, `¶a` linked.
    pub(crate) mark: &'static str,
    pub(crate) preview: Option<bo_engine::gallery::StylePreview>,
}

/// What a click in a side pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PaneAction {
    None,
    Redraw,
    Close,
    /// Apply style `id` (`""` clears the formatting).
    ApplyStyle(String),
    /// A button / link of the pane (`new`, `settings`, `start`, `stop`, `editType`).
    Button(&'static str),
}

/// The Styles pane: the recommended styles with their kind marks, Show Preview, Disable
/// Linked Styles, New Style / Style Inspector / Manage Styles and Options… (the reference's
/// floating pane is 250 × 625 px at 125 %; BlueDoc docks it at the right like the other
/// panes).
pub(crate) struct StylesPane {
    pub(crate) rect: [f32; 4],
    scale: f32,
    pub(crate) styles: Vec<PaneStyle>,
    pub(crate) current: Option<String>,
    pub(crate) show_preview: bool,
    pub(crate) disable_linked: bool,
    scroll: f32,
    hover: Option<usize>,
    /// Texts: title, Clear All, Show Preview, Disable Linked Styles, Options….
    pub(crate) labels: [String; 5],
}

/// Row height of the Styles pane list (32 px at 125 %).
const STYLE_ROW: f32 = 25.6;

impl StylesPane {
    pub(crate) fn new(labels: [String; 5]) -> Self {
        Self {
            rect: [0.0; 4],
            scale: 1.0,
            styles: Vec::new(),
            current: None,
            show_preview: false,
            disable_linked: false,
            scroll: 0.0,
            hover: None,
            labels,
        }
    }

    /// Width at display `scale` (250 px at 125 %).
    pub(crate) fn width(scale: f32) -> f32 {
        (200.0 * scale).round()
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, (bottom - top).max(0.0)];
        self.clamp();
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(26.0);
        [
            self.rect[0] + self.rect[2] - self.px(8.0) - s,
            self.rect[1] + self.px(8.0),
            s,
            s,
        ]
    }

    /// The list box (white, framed).
    fn list_rect(&self) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        let top = y + self.px(48.0);
        [
            x + self.px(10.0),
            top,
            w - self.px(20.0),
            (h - self.px(48.0) - self.px(108.0)).max(0.0),
        ]
    }

    fn check_rect(&self, i: usize) -> [f32; 4] {
        let l = self.list_rect();
        [
            l[0] + self.px(2.0),
            l[1] + l[3] + self.px(14.0) + i as f32 * self.px(29.0),
            self.px(13.0),
            self.px(13.0),
        ]
    }

    /// New Style, Style Inspector, Manage Styles, Options….
    fn button_rect(&self, i: usize) -> [f32; 4] {
        let l = self.list_rect();
        let y = l[1] + l[3] + self.px(72.0);
        let b = self.px(28.0);
        if i < 3 {
            [l[0] + i as f32 * self.px(40.0), y, b, b]
        } else {
            let x = l[0] + 3.0 * self.px(40.0);
            [x, y, l[0] + l[2] - x, b]
        }
    }

    fn rows(&self) -> usize {
        self.styles.len() + 1
    }

    fn row_h(&self) -> f32 {
        (STYLE_ROW * self.scale).round()
    }

    fn clamp(&mut self) {
        let l = self.list_rect();
        let max = (self.rows() as f32 * self.row_h() - l[3] + self.px(4.0)).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let old = self.scroll;
        self.scroll += dy;
        self.clamp();
        old != self.scroll
    }

    fn row_at(&self, x: f32, y: f32) -> Option<usize> {
        let l = self.list_rect();
        if !inside(l, x, y) {
            return None;
        }
        let i = ((y - l[1] - self.px(2.0) + self.scroll) / self.row_h()).floor();
        (i >= 0.0 && (i as usize) < self.rows()).then_some(i as usize)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.row_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> PaneAction {
        if inside(self.close_rect(), x, y) {
            return PaneAction::Close;
        }
        if let Some(i) = self.row_at(x, y) {
            return PaneAction::ApplyStyle(if i == 0 {
                String::new()
            } else {
                self.styles[i - 1].id.clone()
            });
        }
        for i in 0..2 {
            let r = self.check_rect(i);
            let wide = [
                r[0],
                r[1] - self.px(4.0),
                self.list_rect()[2],
                r[3] + self.px(8.0),
            ];
            if inside(wide, x, y) {
                if i == 0 {
                    self.show_preview = !self.show_preview;
                } else {
                    self.disable_linked = !self.disable_linked;
                }
                return PaneAction::Redraw;
            }
        }
        if inside(self.button_rect(0), x, y) {
            return PaneAction::Button("new");
        }
        PaneAction::None
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme, icons: &crate::icons::Icons) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, PANE);
        p.fill_rect(x, y, 1.0, h, PANE_EDGE);
        let title = ui_font(self.px(20.0), 300);
        p.text_font(
            &self.labels[0],
            x + self.px(10.0),
            y + self.px(8.0),
            &title,
            INK,
        );
        let c = self.close_rect();
        cross(p, c, self.px(5.0), INK, self.scale.max(1.0));
        chevron(
            p,
            c[0] - self.px(16.0),
            c[1] + c[3] / 2.0,
            self.px(5.0),
            INK,
            self.scale.max(1.0),
        );
        let l = self.list_rect();
        p.fill_rect(l[0], l[1], l[2], l[3], PANE_EDGE);
        p.fill_rect(l[0] + 1.0, l[1] + 1.0, l[2] - 2.0, l[3] - 2.0, WHITE);
        p.push_clip(l[0] + 1.0, l[1] + 1.0, l[2] - 2.0, l[3] - 2.0);
        let rh = self.row_h();
        let font = ui_font(self.px(11.5), 400);
        let mark_font = ui_font(self.px(10.0), 400);
        for i in 0..self.rows() {
            let ry = l[1] + self.px(2.0) + i as f32 * rh - self.scroll;
            if ry + rh < l[1] || ry > l[1] + l[3] {
                continue;
            }
            let r = [l[0] + self.px(4.0), ry, l[2] - self.px(8.0), rh];
            let style = i.checked_sub(1).and_then(|k| self.styles.get(k));
            let current = style.is_some_and(|s| Some(&s.id) == self.current.as_ref());
            if current {
                p.fill_rect(r[0], r[1] + 1.0, r[2], r[3] - 2.0, CURRENT);
                p.fill_rect(r[0] + 2.0, r[1] + 3.0, r[2] - 4.0, r[3] - 6.0, WHITE);
            } else if self.hover == Some(i) {
                p.fill_rect(r[0], r[1] + 1.0, r[2], r[3] - 2.0, HOVER);
            }
            let (name, mark) = match style {
                None => (self.labels[1].as_str(), ""),
                Some(s) => (
                    s.name.as_str(),
                    if self.disable_linked && s.mark == "¶a" {
                        "¶"
                    } else {
                        s.mark
                    },
                ),
            };
            match style
                .and_then(|s| s.preview.as_ref())
                .filter(|_| self.show_preview)
            {
                Some(pv) => {
                    let f = Font {
                        family: "Segoe UI",
                        size: self.px(pv.size.clamp(8.0, 16.0)),
                        weight: if pv.bold { 700 } else { 400 },
                        italic: pv.italic,
                        underline: pv.underline,
                    };
                    let color = crate::theme::parse_color(&format!("#{}", pv.color)).unwrap_or(INK);
                    p.text_font(
                        name,
                        r[0] + self.px(6.0),
                        ry + (rh - f.size * 1.33) / 2.0,
                        &f,
                        color,
                    );
                }
                None => {
                    p.text_font(name, r[0] + self.px(6.0), ry + self.px(5.0), &font, INK);
                }
            }
            if !mark.is_empty() {
                p.text_font(
                    mark,
                    r[0] + r[2] - self.px(20.0),
                    ry + self.px(6.0),
                    &mark_font,
                    Rgba(0x60, 0x60, 0x60, 255),
                );
            }
        }
        p.pop_clip();
        let label_font = ui_font(self.px(12.5), 400);
        for (i, on) in [self.show_preview, self.disable_linked]
            .into_iter()
            .enumerate()
        {
            let r = self.check_rect(i);
            check_box(p, r, on, true, self.scale);
            p.text_font(
                &self.labels[2 + i],
                r[0] + self.px(20.0),
                r[1] - self.px(4.0),
                &label_font,
                INK,
            );
        }
        for i in 0..4 {
            let r = self.button_rect(i);
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xad, 0xad, 0xad, 255));
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let enabled = i == 0;
            if i < 3 {
                let name = ["styles.newStyle", "styles.inspector", "styles.manage"][i];
                icons.draw(
                    p,
                    name,
                    r[0] + self.px(4.0),
                    r[1] + self.px(4.0),
                    self.px(20.0),
                    crate::icons::IconColors {
                        ink: if enabled { t.text } else { DIM },
                        accent: t.accent,
                        on: t.accent_on,
                        tinted: enabled,
                    },
                );
            } else {
                p.text_font(
                    &self.labels[4],
                    r[0] + self.px(6.0),
                    r[1] + self.px(5.0),
                    &label_font,
                    DIM,
                );
            }
        }
        p.pop_clip();
    }
}

// ---------------------------------------------------------------------------------------
// Restrict Editing pane.

/// Editing restriction kinds of the pane, as `ST_DocProtect` values (§17.18.29).
pub(crate) const RESTRICT_KINDS: [&str; 4] = ["trackedChanges", "comments", "forms", "readOnly"];

/// The Restrict Editing pane (Review › Protect › Restrict Editing): 1. Formatting
/// restrictions, 2. Editing restrictions with the kind drop-down, 3. Start enforcement; while
/// the document is protected, Stop Protection.
pub(crate) struct RestrictPane {
    pub(crate) rect: [f32; 4],
    scale: f32,
    pub(crate) limit_formatting: bool,
    pub(crate) allow_only: bool,
    /// Index into [`RESTRICT_KINDS`] (the reference starts on read only).
    pub(crate) kind: usize,
    /// The document is protected now.
    pub(crate) protected: bool,
    hover: Option<&'static str>,
    /// Texts: title, 1. heading, limit check, Settings…, 2. heading, allow check (two
    /// lines), kind names ×4, 3. heading, ready text (two lines), Start button, See also,
    /// Restrict permission…, Stop Protection, protected note.
    pub(crate) labels: Vec<String>,
}

impl RestrictPane {
    pub(crate) fn new(labels: Vec<String>, protected: bool) -> Self {
        Self {
            rect: [0.0; 4],
            scale: 1.0,
            limit_formatting: false,
            allow_only: false,
            kind: 3,
            protected,
            hover: None,
            labels,
        }
    }

    /// Width at display `scale` (274 px at 125 %).
    pub(crate) fn width(scale: f32) -> f32 {
        (219.0 * scale).round()
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn label(&self, i: usize) -> &str {
        self.labels.get(i).map_or("", String::as_str)
    }

    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, (bottom - top).max(0.0)];
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(26.0);
        [
            self.rect[0] + self.rect[2] - self.px(8.0) - s,
            self.rect[1] + self.px(8.0),
            s,
            s,
        ]
    }

    /// Rectangles of the active parts, by id.
    fn parts(&self) -> Vec<(&'static str, [f32; 4])> {
        let [x, y, w, h] = self.rect;
        let l = x + self.px(14.0);
        let mut out = vec![("close", self.close_rect())];
        if self.protected {
            out.push((
                "stop",
                [l, y + self.px(116.0), self.px(130.0), self.px(22.0)],
            ));
            return out;
        }
        out.push((
            "limit",
            [l, y + self.px(56.0), w - self.px(20.0), self.px(16.0)],
        ));
        out.push((
            "settings",
            [l, y + self.px(76.0), self.px(60.0), self.px(16.0)],
        ));
        out.push((
            "allow",
            [l, y + self.px(121.0), w - self.px(20.0), self.px(30.0)],
        ));
        out.push((
            "kind",
            [l, y + self.px(155.0), self.px(146.0), self.px(20.0)],
        ));
        out.push((
            "start",
            [l, y + self.px(256.0), self.px(162.0), self.px(22.0)],
        ));
        out.push((
            "permission",
            [l, y + h - self.px(30.0), self.px(110.0), self.px(16.0)],
        ));
        out
    }

    fn part_at(&self, x: f32, y: f32) -> Option<&'static str> {
        self.parts()
            .into_iter()
            .find(|(_, r)| inside(*r, x, y))
            .map(|(id, _)| id)
    }

    /// Rectangle of the kind drop-down (window pixels), to anchor its menu.
    pub(crate) fn kind_rect(&self) -> [f32; 4] {
        self.parts()
            .into_iter()
            .find(|(id, _)| *id == "kind")
            .map_or([0.0; 4], |(_, r)| r)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.part_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> PaneAction {
        match self.part_at(x, y) {
            Some("close") => PaneAction::Close,
            Some("limit") => {
                self.limit_formatting = !self.limit_formatting;
                PaneAction::Redraw
            }
            Some("allow") => {
                self.allow_only = !self.allow_only;
                PaneAction::Redraw
            }
            Some("kind") if self.allow_only => PaneAction::Button("editType"),
            Some("start") if self.allow_only || self.limit_formatting => {
                PaneAction::Button("start")
            }
            Some("stop") => PaneAction::Button("stop"),
            _ => PaneAction::None,
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, PANE);
        p.fill_rect(x, y, 1.0, h, PANE_EDGE);
        let title = ui_font(self.px(20.0), 300);
        p.text_font(
            self.label(0),
            x + self.px(10.0),
            y + self.px(8.0),
            &title,
            INK,
        );
        let c = self.close_rect();
        cross(p, c, self.px(5.0), INK, self.scale.max(1.0));
        chevron(
            p,
            c[0] - self.px(16.0),
            c[1] + c[3] / 2.0,
            self.px(5.0),
            INK,
            self.scale.max(1.0),
        );
        let head = ui_font(self.px(13.0), 600);
        let small = ui_font(self.px(10.0), 400);
        let l = x + self.px(14.0);
        let heading = |p: &mut dyn Painter, text: &str, hy: f32| {
            p.text_font(text, l, hy, &head, Rgba(0x26, 0x26, 0x26, 255));
            p.fill_rect(l, hy + self.px(19.0), w - self.px(20.0), 1.0, HEADING_LINE);
        };
        if self.protected {
            heading(p, self.label(16), y + self.px(40.0));
            p.text_font(self.label(17), l, y + self.px(68.0), &small, INK);
            let r = self.parts()[1].1;
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xad, 0xad, 0xad, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                if self.hover == Some("stop") {
                    HOVER
                } else {
                    WHITE
                },
            );
            p.text_font(
                self.label(15),
                r[0] + self.px(8.0),
                r[1] + self.px(4.0),
                &small,
                INK,
            );
            p.pop_clip();
            return;
        }
        heading(p, self.label(1), y + self.px(38.0));
        let cb = |p: &mut dyn Painter, cy: f32, on: bool, enabled: bool| {
            check_box(
                p,
                [l, cy, self.px(11.0), self.px(11.0)],
                on,
                enabled,
                self.scale,
            );
        };
        cb(p, y + self.px(59.0), self.limit_formatting, true);
        p.text_font(
            self.label(2),
            l + self.px(16.0),
            y + self.px(57.0),
            &small,
            INK,
        );
        let link = ui_font(self.px(10.0), 400);
        p.text_font(self.label(3), l, y + self.px(78.0), &link, LINK);
        heading(p, self.label(4), y + self.px(104.0));
        cb(p, y + self.px(124.0), self.allow_only, true);
        let allow: Vec<&str> = self.label(5).split('\n').collect();
        for (i, line) in allow.iter().enumerate() {
            p.text_font(
                line,
                l + self.px(16.0),
                y + self.px(122.0) + i as f32 * self.px(13.5),
                &small,
                INK,
            );
        }
        let k = self.kind_rect();
        let on = self.allow_only;
        p.fill_rect(k[0], k[1], k[2], k[3], Rgba(0xc8, 0xc8, 0xc8, 255));
        p.fill_rect(k[0] + 1.0, k[1] + 1.0, k[2] - 2.0, k[3] - 2.0, WHITE);
        p.text_font(
            self.label(6 + self.kind),
            k[0] + self.px(4.0),
            k[1] + self.px(3.0),
            &small,
            if on { INK } else { DIM },
        );
        p.fill_rect(
            k[0] + k[2] - self.px(16.0),
            k[1] + 1.0,
            1.0,
            k[3] - 2.0,
            Rgba(0xc8, 0xc8, 0xc8, 255),
        );
        chevron(
            p,
            k[0] + k[2] - self.px(8.0),
            k[1] + k[3] / 2.0,
            self.px(3.0),
            if on { INK } else { DIM },
            self.scale.max(1.0),
        );
        heading(p, self.label(10), y + self.px(206.0));
        for (i, line) in self.label(11).split('\n').enumerate() {
            p.text_font(
                line,
                l,
                y + self.px(224.0) + i as f32 * self.px(13.5),
                &small,
                Rgba(0x44, 0x44, 0x44, 255),
            );
        }
        let s = self
            .parts()
            .into_iter()
            .find(|(id, _)| *id == "start")
            .map_or([0.0; 4], |(_, r)| r);
        let can = self.allow_only || self.limit_formatting;
        p.fill_rect(
            s[0],
            s[1],
            s[2],
            s[3],
            if can {
                Rgba(0xad, 0xad, 0xad, 255)
            } else {
                Rgba(0xdc, 0xdc, 0xdc, 255)
            },
        );
        p.fill_rect(
            s[0] + 1.0,
            s[1] + 1.0,
            s[2] - 2.0,
            s[3] - 2.0,
            if can && self.hover == Some("start") {
                HOVER
            } else {
                WHITE
            },
        );
        p.text_font(
            self.label(12),
            s[0] + self.px(9.0),
            s[1] + self.px(4.0),
            &small,
            if can { INK } else { DIM },
        );
        heading(p, self.label(13), y + h - self.px(54.0));
        p.text_font(self.label(14), l, y + h - self.px(28.0), &link, LINK);
        p.pop_clip();
    }
}

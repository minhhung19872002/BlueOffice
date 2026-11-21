//! Slide Show Help (F1 or right-click menu › Help while presenting), laid out as the
//! reference's dialog measured at 125 %: 850 × 680 px with a caption, a tab strip, a white page
//! holding a heading and a two-column table of keys and what they do, and an OK button at
//! the lower right. Wording and the lists are BlueSlide's own: only what the show does.
use super::SlideWindow;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Pages: (tab label key, fallback, heading key, fallback, rows (keys, what key, fallback)).
type Row = (&'static str, &'static str, &'static str);
const PAGES: [(&str, &str, &str, &str, &[Row]); 5] = [
    (
        "slideR10.help.tabGeneral",
        "General",
        "slideR10.help.headGeneral",
        "Moving through the show",
        &[
            (
                "N, Space, \u{2192}, \u{2193}, Enter, Page Down, click",
                "slideR10.help.next",
                "Next animation or slide",
            ),
            (
                "P, Backspace, \u{2190}, \u{2191}, Page Up",
                "slideR10.help.prev",
                "Previous animation or slide",
            ),
            (
                "Number, then Enter",
                "slideR10.help.goto",
                "Go to that slide",
            ),
            (
                "Home / End",
                "slideR10.help.firstLast",
                "First / last slide",
            ),
            ("G, -, Ctrl+S", "slideR10.help.all", "See all slides"),
            (
                "+  (then - to step back)",
                "slideR10.help.zoom",
                "Zoom in on the slide",
            ),
            (
                "B or .",
                "slideR10.help.black",
                "Black screen (again to come back)",
            ),
            (
                "W or ,",
                "slideR10.help.white",
                "White screen (again to come back)",
            ),
            (
                "S",
                "slideR10.help.pause",
                "Pause / resume automatic advance",
            ),
            (
                "H",
                "slideR10.help.hidden",
                "Next slide, even when it is hidden",
            ),
            (
                "Ctrl+H / Ctrl+U",
                "slideR10.help.hideArrow",
                "Hide the pointer now / after a pause",
            ),
            (
                "Ctrl+T",
                "slideR10.help.taskbar",
                "Show the taskbar (any key returns)",
            ),
            ("Right-click", "slideR10.help.menu", "Slide show menu"),
            ("F1", "slideR10.help.f1", "This help"),
            ("Esc, Ctrl+Break", "slideR10.help.end", "End the show"),
        ],
    ),
    (
        "slideR10.help.tabRehearse",
        "Rehearse & record",
        "slideR10.help.headRehearse",
        "Rehearsing timings",
        &[
            (
                "Slide Show \u{203a} Rehearse Timings",
                "slideR10.help.rehearse",
                "Time each slide while presenting",
            ),
            ("S", "slideR10.help.pauseTimer", "Pause / resume the clock"),
            (
                "Esc",
                "slideR10.help.endRehearse",
                "Stop; you are asked to keep the timings",
            ),
        ],
    ),
    (
        "slideR10.help.tabMedia",
        "Media",
        "slideR10.help.headMedia",
        "Video and audio",
        // Round 22: only what the show does — no media keys are listed.
        &[(
            "",
            "slideR22.help.media",
            "Video and audio show their first frame; the show has no media keys yet.",
        )],
    ),
    (
        "slideR10.help.tabInk",
        "Ink & laser pointer",
        "slideR10.help.headInk",
        "Drawing on slides",
        &[
            ("Ctrl+P", "slideR10.help.pen", "Pen"),
            ("Ctrl+I", "slideR10.help.highlighter", "Highlighter"),
            ("Ctrl+L", "slideR10.help.laser", "Laser pointer"),
            ("Ctrl+E", "slideR10.help.eraser", "Eraser (click a stroke)"),
            ("E", "slideR10.help.eraseAll", "Erase all ink on the slide"),
            ("Ctrl+A, Esc", "slideR10.help.arrow", "Back to the arrow"),
        ],
    ),
    (
        "slideR10.help.tabTouch",
        "Touch",
        "slideR10.help.headTouch",
        "Touch screens",
        // Round 22: the gestures Windows turns into mouse input, which the show handles.
        &[
            ("Tap", "slideR22.help.tap", "Next animation or slide"),
            (
                "Press and hold",
                "slideR22.help.hold",
                "Open the show menu (Previous, See All Slides, Pointer Options…)",
            ),
            (
                "Two-finger scroll",
                "slideR22.help.swipe",
                "Next / previous slide (touchpad)",
            ),
        ],
    ),
];

/// Logical size of the dialog (reference: 850 × 680 px at 125 %).
const W: f32 = 680.0;
const H: f32 = 544.0;
const CAPTION_H: f32 = 31.2;

/// Hot areas of the help dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HelpHit {
    Tab(usize),
    Ok,
    Close,
}

impl SlideWindow {
    fn help_px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Frame of the dialog (window px), centred.
    fn help_frame(&self) -> [f32; 4] {
        let (w, h) = (self.help_px(W), self.help_px(H));
        [
            ((self.size.0 - w) / 2.0).round(),
            ((self.size.1 - h) / 2.0).round(),
            w,
            h,
        ]
    }

    pub(super) fn help_tabs(&self) -> Vec<[f32; 4]> {
        let [x, y, ..] = self.help_frame();
        let mut tx = x + self.help_px(12.0);
        PAGES
            .iter()
            .map(|(k, f, ..)| {
                let label = self.t(k, f);
                // Measured label width (an estimate until the first measure).
                let measured = self
                    .help_tab_w
                    .iter()
                    .find(|(l, _)| *l == label)
                    .map(|(_, w)| *w);
                let w = measured.map_or_else(
                    || self.help_px(label.chars().count() as f32 * 6.4 + 12.0),
                    |w| (w + self.help_px(12.0)).round(),
                );
                let r = [tx, y + self.help_px(CAPTION_H + 6.0), w, self.help_px(20.0)];
                tx += w;
                r
            })
            .collect()
    }

    pub(super) fn help_ok(&self) -> [f32; 4] {
        let [x, y, w, h] = self.help_frame();
        let (bw, bh) = (self.help_px(84.0), self.help_px(26.4));
        [
            x + w - bw - self.help_px(14.0),
            y + h - bh - self.help_px(9.6),
            bw,
            bh,
        ]
    }

    fn help_close(&self) -> [f32; 4] {
        let [x, y, w, _] = self.help_frame();
        let b = self.help_px(CAPTION_H);
        [x + w - b - self.help_px(8.0), y, b, b]
    }

    fn help_hit(&self, px: f32, py: f32) -> Option<HelpHit> {
        let inside = |r: [f32; 4]| px >= r[0] && px < r[0] + r[2] && py >= r[1] && py < r[1] + r[3];
        if inside(self.help_ok()) {
            return Some(HelpHit::Ok);
        }
        if inside(self.help_close()) {
            return Some(HelpHit::Close);
        }
        self.help_tabs()
            .into_iter()
            .position(inside)
            .map(HelpHit::Tab)
    }

    /// Measure the tab labels (after Help opened or the language changed).
    pub(super) fn measure_help(&mut self, text: &dyn crate::TextMeasure) {
        if !self.show_help || !self.help_tab_w.is_empty() {
            return;
        }
        let style = TextStyle {
            size: self.help_px(12.0),
            bold: false,
        };
        self.help_tab_w = PAGES
            .iter()
            .map(|(k, f, ..)| {
                let label = self.t(k, f);
                let w = text.measure(&label, style).0;
                (label, w)
            })
            .collect();
    }

    /// Input while Help is open (it is modal over the show).
    pub(super) fn help_input(&mut self, input: &Input) -> bool {
        match *input {
            Input::Key(Key::Escape | Key::Enter, _) => {
                self.show_help = false;
                true
            }
            Input::Key(k @ (Key::Tab | Key::Right | Key::Left), m) if k != Key::Tab || m.ctrl => {
                let back = k == Key::Left || m.shift;
                let n = PAGES.len();
                self.help_tab = if back {
                    (self.help_tab + n - 1) % n
                } else {
                    (self.help_tab + 1) % n
                };
                true
            }
            Input::Press { x, y, .. } => match self.help_hit(x, y) {
                Some(HelpHit::Ok | HelpHit::Close) => {
                    self.show_help = false;
                    true
                }
                Some(HelpHit::Tab(i)) => {
                    self.help_tab = i;
                    true
                }
                None => false,
            },
            Input::Move { x, y } => {
                let h = self.help_hit(x, y).map(|h| match h {
                    HelpHit::Ok => 100,
                    HelpHit::Close => 101,
                    HelpHit::Tab(i) => i,
                });
                std::mem::replace(&mut self.help_hover, h) != h
            }
            _ => false,
        }
    }

    /// Paint the Help dialog over the show.
    pub(super) fn paint_show_help(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.help_frame();
        let px = |v: f32| self.help_px(v);
        for (k, a) in [(6.0, 14u8), (3.0, 22), (1.0, 30)] {
            let o = px(k);
            p.fill_rect(
                x - o,
                y - o + px(3.0),
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        let face = Rgba(0xf3, 0xf3, 0xf3, 255);
        let edge = Rgba(0xa0, 0xa0, 0xa0, 255);
        let line = Rgba(0xd0, 0xd0, 0xd0, 255);
        let ink = Rgba(0x1a, 0x1a, 0x1a, 255);
        p.fill_rect(x, y, w, h, edge);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, face);
        let body = TextStyle {
            size: px(12.0),
            bold: false,
        };
        let lh = body.size * 1.33;
        // Caption: title in grey, a close cross.
        p.text(
            &self.t("slideR10.help.title", "Slide Show Help"),
            x + px(14.0),
            y + ((px(CAPTION_H) - lh) / 2.0).round(),
            body,
            Rgba(0x60, 0x60, 0x60, 255),
        );
        let c = self.help_close();
        if self.help_hover == Some(101) {
            p.fill_rect(
                c[0],
                c[1] + 1.0,
                c[2],
                c[3] - 1.0,
                Rgba(0xe8, 0x11, 0x23, 255),
            );
        }
        let (cx, cy, r) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, px(5.0));
        p.path(
            &[
                PathSeg::M(cx - r, cy - r),
                PathSeg::L(cx + r, cy + r),
                PathSeg::M(cx + r, cy - r),
                PathSeg::L(cx - r, cy + r),
            ],
            None,
            Some((
                if self.help_hover == Some(101) {
                    Rgba(255, 255, 255, 255)
                } else {
                    Rgba(0x40, 0x40, 0x40, 255)
                },
                self.scale.max(1.0),
            )),
        );
        // Tabs and the page under them.
        let tabs = self.help_tabs();
        let page_top = tabs[0][1] + tabs[0][3] - 1.0;
        let ok = self.help_ok();
        let page = [
            x + px(12.0),
            page_top,
            w - px(24.0),
            ok[1] - px(9.6) - page_top,
        ];
        p.fill_rect(page[0], page[1], page[2], page[3], line);
        p.fill_rect(
            page[0] + 1.0,
            page[1] + 1.0,
            page[2] - 2.0,
            page[3] - 2.0,
            Rgba(0xf9, 0xf9, 0xf9, 255),
        );
        for (i, (r, (k, f, ..))) in tabs.iter().zip(PAGES.iter()).enumerate() {
            let on = i == self.help_tab;
            if on {
                p.fill_rect(r[0], r[1] - px(2.0), r[2], r[3] + px(2.0), line);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] - px(2.0) + 1.0,
                    r[2] - 2.0,
                    r[3] + px(2.0),
                    Rgba(0xf9, 0xf9, 0xf9, 255),
                );
            } else if self.help_hover == Some(i) {
                p.fill_rect(r[0], r[1], r[2], r[3] - 1.0, Rgba(0xe5, 0xf1, 0xfb, 255));
            }
            p.text(
                &self.t(k, f),
                r[0] + px(6.0),
                r[1] + ((r[3] - lh) / 2.0).round(),
                body,
                ink,
            );
        }
        // Heading and the table.
        let (_, _, hk, hf, rows) = PAGES[self.help_tab.min(PAGES.len() - 1)];
        p.text(
            &self.t(hk, hf),
            page[0] + px(16.0),
            page[1] + px(14.0),
            TextStyle {
                size: px(16.0),
                bold: true,
            },
            ink,
        );
        let mut ry = page[1] + px(48.0);
        for (keys, k, f) in rows {
            p.text(keys, page[0] + px(16.0), ry, body, ink);
            p.text(&self.t(k, f), page[0] + px(334.0), ry, body, ink);
            ry += px(21.0);
        }
        // OK: the default button, framed in the focus blue.
        let hover = self.help_hover == Some(100);
        p.fill_rect(ok[0], ok[1], ok[2], ok[3], Rgba(0x00, 0x78, 0xd4, 255));
        p.fill_rect(
            ok[0] + 2.0,
            ok[1] + 2.0,
            ok[2] - 4.0,
            ok[3] - 4.0,
            if hover {
                Rgba(0xe5, 0xf1, 0xfb, 255)
            } else {
                Rgba(0xfd, 0xfd, 0xfd, 255)
            },
        );
        let label = self.t("common.ok", "OK");
        let tw = label.chars().count() as f32 * body.size * 0.55;
        p.text(
            &label,
            (ok[0] + (ok[2] - tw) / 2.0).round(),
            ok[1] + ((ok[3] - lh) / 2.0).round(),
            body,
            ink,
        );
    }
}

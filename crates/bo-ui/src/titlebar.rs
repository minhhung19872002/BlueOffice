//! Self-drawn title bar (ADR-0016 / ADR-0018): app mark and name, quick access buttons, the
//! document name and save state, command search, and the window buttons. The host maps
//! [`NcHit`] to the platform's hit-test codes (on Windows, `HTMAXBUTTON` shows snap layouts).
use crate::icons::{IconColors, Icons};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{commands, Painter, TextMeasure, TextStyle};
use bo_edit::FormatState;
use bo_render::{PathSeg, Rgba};

/// What a point of the title bar is, for the window manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NcHit {
    /// Ordinary content (buttons, search box, anything below the title bar).
    Client,
    /// Drag area: moves the window, double click maximises, right click opens the system menu.
    Caption,
    /// Minimise button.
    Minimize,
    /// Maximise / restore button.
    Maximize,
    /// Close button.
    Close,
}

/// Quick access buttons (command ids) after the app name.
const QUICK: [&str; 3] = ["file.save", "edit.undo", "edit.redo"];
/// Red of a hovered close button and its glyph (window-manager convention, tokens.css).
const CLOSE_BG: Rgba = Rgba(0xC4, 0x2B, 0x1C, 255);

#[derive(Clone, Copy, Default)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl R {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// The title bar of a document window.
pub struct TitleBar {
    scale: f32,
    width: f32,
    maximized: bool,
    quick: Vec<(&'static str, R)>,
    search: R,
    buttons: [R; 3],
    title_x: f32,
    /// Width of the document name; the save state follows it.
    name_w: f32,
    hover: Option<(NcHit, Option<&'static str>)>,
    pressed: Option<&'static str>,
}

impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

impl TitleBar {
    /// An empty title bar; call [`TitleBar::layout`].
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            width: 0.0,
            maximized: false,
            quick: Vec::new(),
            search: R::default(),
            buttons: [R::default(); 3],
            title_x: 0.0,
            name_w: 0.0,
            hover: None,
            pressed: None,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Height in device pixels.
    pub fn height(&self) -> f32 {
        self.px(36.0)
    }

    /// Show the restore glyph instead of maximise.
    pub fn set_maximized(&mut self, maximized: bool) -> bool {
        std::mem::replace(&mut self.maximized, maximized) != maximized
    }

    /// Lay out for a window `width` pixels wide.
    pub fn layout(
        &mut self,
        text: &dyn TextMeasure,
        strings: &Strings,
        name: &str,
        width: f32,
        scale: f32,
    ) {
        self.scale = scale;
        self.width = width;
        let h = self.height();
        let bw = self.px(46.0);
        for (i, b) in self.buttons.iter_mut().enumerate() {
            *b = R {
                x: width - bw * (3 - i) as f32,
                y: 0.0,
                w: bw,
                h,
            };
        }
        let app = text.measure("BlueDoc", self.style(true)).0;
        let mut x = self.px(40.0) + app + self.px(12.0);
        let size = self.px(30.0);
        self.quick = QUICK
            .iter()
            .map(|id| {
                let r = R {
                    x,
                    y: ((h - size) / 2.0).round(),
                    w: size,
                    h: size,
                };
                x += size + self.px(2.0);
                (*id, r)
            })
            .collect();
        let sw = self.px(300.0).min((width * 0.25).max(self.px(160.0)));
        self.search = R {
            x: (width - 3.0 * bw - sw - self.px(16.0)).round(),
            y: ((h - self.px(26.0)) / 2.0).round(),
            w: sw,
            h: self.px(26.0),
        };
        self.name_w = text.measure(name, self.style(false)).0;
        let state_w = ["titleBar.saved", "titleBar.unsaved"]
            .iter()
            .map(|k| text.measure(strings.get(k), self.style(false)).0)
            .fold(0.0, f32::max);
        let total = self.name_w + self.px(12.0) + state_w;
        // Centred in the window, but never over the quick access buttons or the search box.
        let centred = ((width - total) / 2.0).round();
        self.title_x = centred
            .min(self.search.x - total - self.px(12.0))
            .max(x + self.px(12.0));
    }

    fn style(&self, bold: bool) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold,
        }
    }

    /// What is at a window position (y below the bar is [`NcHit::Client`]).
    pub fn hit(&self, x: f32, y: f32) -> NcHit {
        self.hit_full(x, y).0
    }

    fn hit_full(&self, x: f32, y: f32) -> (NcHit, Option<&'static str>) {
        if y < 0.0 || y >= self.height() {
            return (NcHit::Client, None);
        }
        let kinds = [NcHit::Minimize, NcHit::Maximize, NcHit::Close];
        if let Some(i) = self.buttons.iter().position(|b| b.contains(x, y)) {
            return (kinds[i], None);
        }
        if let Some((id, _)) = self.quick.iter().find(|(_, r)| r.contains(x, y)) {
            return (NcHit::Client, Some(id));
        }
        if self.search.contains(x, y) {
            return (NcHit::Client, None);
        }
        (NcHit::Caption, None)
    }

    /// Pointer moved (also outside the client area): `true` when the highlight changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit_full(x, y);
        let h = (h.0 != NcHit::Caption && (h.0 != NcHit::Client || h.1.is_some())).then_some(h);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Button pressed: `true` when it went down on a quick access button.
    pub fn press(&mut self, x: f32, y: f32) -> bool {
        self.pressed = self.hit_full(x, y).1;
        self.pressed.is_some()
    }

    /// Button released: the quick access command activated, if any.
    pub fn release(&mut self, x: f32, y: f32) -> Option<&'static str> {
        let was = self.pressed.take()?;
        (self.hit_full(x, y).1 == Some(was)).then_some(was)
    }

    /// Paint the bar.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        icons: &Icons,
        strings: &Strings,
        name: &str,
        modified: bool,
        format: &FormatState,
    ) {
        let h = self.height();
        p.fill_rect(0.0, 0.0, self.width, h, t.bg);
        let ink = IconColors {
            ink: t.text,
            accent: t.accent,
            on: t.accent_on,
            tinted: false,
        };
        let mark = self.px(20.0);
        icons.draw(
            p,
            "app.doc",
            self.px(12.0),
            ((h - mark) / 2.0).round(),
            mark,
            ink,
        );
        let line = self.style(false).size * 1.33;
        let ty = ((h - line) / 2.0).round();
        p.text("BlueDoc", self.px(40.0), ty, self.style(true), t.text);
        for (id, r) in &self.quick {
            let enabled = match *id {
                "file.save" => modified,
                other => commands::enabled(other, format),
            };
            if enabled && self.hover.is_some_and(|h| h.1 == Some(*id)) {
                let c = if self.pressed == Some(id) {
                    t.active
                } else {
                    t.hover
                };
                p.fill_rect(r.x, r.y, r.w, r.h, c);
            }
            let size = self.px(18.0);
            let colors = IconColors {
                ink: if enabled { t.text } else { t.border },
                ..ink
            };
            icons.draw(
                p,
                id,
                r.x + (r.w - size) / 2.0,
                r.y + (r.h - size) / 2.0,
                size,
                colors,
            );
        }
        // Document name and save state.
        p.text(name, self.title_x, ty, self.style(false), t.text);
        let state = strings.get(if modified {
            "titleBar.unsaved"
        } else {
            "titleBar.saved"
        });
        p.text(
            state,
            self.title_x + self.name_w + self.px(12.0),
            ty,
            self.style(false),
            t.text_muted,
        );
        // Command search box (opens in N4).
        let s = self.search;
        p.fill_rect(s.x, s.y, s.w, s.h, t.border);
        p.fill_rect(s.x + 1.0, s.y + 1.0, s.w - 2.0, s.h - 2.0, t.surface);
        let icon = self.px(16.0);
        icons.draw(
            p,
            "edit.find",
            s.x + self.px(8.0),
            s.y + (s.h - icon) / 2.0,
            icon,
            IconColors {
                ink: t.text_muted,
                ..ink
            },
        );
        p.text(
            strings.get("commandSearch.placeholder"),
            s.x + self.px(30.0),
            s.y + (s.h - line) / 2.0,
            self.style(false),
            t.text_muted,
        );
        // Window buttons with drawn glyphs (no system icon font).
        for (i, b) in self.buttons.iter().enumerate() {
            let kind = [NcHit::Minimize, NcHit::Maximize, NcHit::Close][i];
            let hot = self.hover.is_some_and(|h| h.0 == kind);
            let close = kind == NcHit::Close;
            if hot {
                p.fill_rect(b.x, b.y, b.w, b.h, if close { CLOSE_BG } else { t.hover });
            }
            let c = if hot && close {
                Rgba(255, 255, 255, 255)
            } else {
                t.text
            };
            let g = self.px(10.0);
            let (cx, cy) = (
                (b.x + b.w / 2.0).round() + 0.5,
                (b.y + b.h / 2.0).round() + 0.5,
            );
            let (l, tp, r, bt) = (cx - g / 2.0, cy - g / 2.0, cx + g / 2.0, cy + g / 2.0);
            let w = self.scale.max(1.0).round();
            let segs: Vec<PathSeg> = match kind {
                NcHit::Minimize => vec![PathSeg::M(l, cy), PathSeg::L(r, cy)],
                NcHit::Maximize if self.maximized => {
                    let o = self.px(2.0);
                    vec![
                        PathSeg::M(l, tp + o),
                        PathSeg::L(r - o, tp + o),
                        PathSeg::L(r - o, bt),
                        PathSeg::L(l, bt),
                        PathSeg::Z,
                        PathSeg::M(l + o, tp + o),
                        PathSeg::L(l + o, tp),
                        PathSeg::L(r, tp),
                        PathSeg::L(r, bt - o),
                        PathSeg::L(r - o, bt - o),
                    ]
                }
                NcHit::Maximize => vec![
                    PathSeg::M(l, tp),
                    PathSeg::L(r, tp),
                    PathSeg::L(r, bt),
                    PathSeg::L(l, bt),
                    PathSeg::Z,
                ],
                _ => vec![
                    PathSeg::M(l, tp),
                    PathSeg::L(r, bt),
                    PathSeg::M(r, tp),
                    PathSeg::L(l, bt),
                ],
            };
            p.path(&segs, None, Some((c, w)));
        }
    }
}

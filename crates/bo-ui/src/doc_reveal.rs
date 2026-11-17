//! BlueDoc's Reveal Formatting (Shift+F1) and Style Inspector panes (parity round 5): the
//! formatting at the caret as the engine reports it (`revealFormatting`, `styleInspector`,
//! ECMA-376 Part 1, §17.7 styles and direct formatting), in sections of lines; the Style
//! Inspector's sections carry a clear button for their level (`styleClear`). Docked at the
//! right like the other panes, in the interface's greys.
use crate::{Font, Painter};
use bo_render::Rgba;

const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const DIM: Rgba = Rgba(0x60, 0x60, 0x60, 255);
const LINE: Rgba = Rgba(0xc8, 0xc8, 0xc8, 255);
const BG: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// One section: its header, lines and the clear command of its level (Style Inspector).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InfoSection {
    pub(crate) header: String,
    pub(crate) lines: Vec<String>,
    pub(crate) clear: Option<&'static str>,
}

/// What a press on the pane does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InfoHit {
    None,
    Close,
    /// Clear this formatting level (`paragraphStyle`, `paragraphDirect`, `characterStyle`,
    /// `characterDirect`).
    Clear(&'static str),
}

/// The pane.
pub(crate) struct InfoPane {
    pub(crate) rect: [f32; 4],
    scale: f32,
    pub(crate) title: String,
    pub(crate) sections: Vec<InfoSection>,
    scroll: f32,
}

impl InfoPane {
    pub(crate) fn new(title: String) -> Self {
        Self {
            rect: [0.0; 4],
            scale: 1.0,
            title,
            sections: Vec::new(),
            scroll: 0.0,
        }
    }

    /// Width at display `scale` (250 px at 125 %, as the Styles pane).
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
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
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

    fn line_h(&self) -> f32 {
        self.px(17.0)
    }

    /// Each section's box (header above it) in pane coordinates after scrolling.
    fn boxes(&self) -> Vec<[f32; 4]> {
        let [x, y, w, _] = self.rect;
        let mut top = y + self.px(48.0) - self.scroll;
        let mut out = Vec::new();
        for s in &self.sections {
            top += self.px(20.0);
            let h = self.line_h() * s.lines.len().max(1) as f32 + self.px(8.0);
            out.push([x + self.px(10.0), top, w - self.px(20.0), h]);
            top += h + self.px(10.0);
        }
        out
    }

    fn clear_rect(b: [f32; 4], k: f32) -> [f32; 4] {
        let s = (18.0 * k).round();
        [
            b[0] + b[2] - s - (3.0 * k).round(),
            b[1] + (3.0 * k).round(),
            s,
            s,
        ]
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let old = self.scroll;
        let content: f32 = self
            .boxes()
            .last()
            .map_or(0.0, |b| b[1] + b[3] + self.scroll - self.rect[1]);
        let max = (content - self.rect[3] + self.px(12.0)).max(0.0);
        self.scroll = (self.scroll + dy).clamp(0.0, max);
        old != self.scroll
    }

    pub(crate) fn press(&self, x: f32, y: f32) -> InfoHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if inside(self.close_rect()) {
            return InfoHit::Close;
        }
        for (s, b) in self.sections.iter().zip(self.boxes()) {
            if let Some(level) = s.clear {
                if inside(Self::clear_rect(b, self.scale)) {
                    return InfoHit::Clear(level);
                }
            }
        }
        InfoHit::None
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, BG);
        p.fill_rect(x, y, self.scale.round().max(1.0), h, LINE);
        let title = Font {
            family: "Segoe UI",
            size: self.px(14.5),
            weight: 400,
            italic: false,
            underline: false,
        };
        p.text_font(
            &self.title,
            x + self.px(10.0),
            y + self.px(10.0),
            &title,
            INK,
        );
        // Close: an own thin cross.
        let c = self.close_rect();
        let k = self.px(4.5);
        let (cx, cy) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0);
        let stroke = Some((DIM, self.scale.max(1.0)));
        p.path(
            &[
                bo_render::PathSeg::M(cx - k, cy - k),
                bo_render::PathSeg::L(cx + k, cy + k),
                bo_render::PathSeg::M(cx + k, cy - k),
                bo_render::PathSeg::L(cx - k, cy + k),
            ],
            None,
            stroke,
        );
        let head = Font {
            family: "Segoe UI",
            size: self.px(12.0),
            weight: 600,
            italic: false,
            underline: false,
        };
        let body = Font {
            weight: 400,
            ..head
        };
        let top_clip = y + self.px(44.0);
        p.push_clip(x, top_clip, w, (h - (top_clip - y)).max(0.0));
        for (s, b) in self.sections.iter().zip(self.boxes()) {
            p.text_font(&s.header, b[0], b[1] - self.px(19.0), &head, INK);
            p.fill_rect(b[0], b[1], b[2], b[3], LINE);
            p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
            p.push_clip(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0);
            for (i, line) in s.lines.iter().enumerate() {
                let indent = if line.starts_with(' ') {
                    self.px(14.0)
                } else {
                    self.px(4.0)
                };
                p.text_font(
                    line.trim_start(),
                    b[0] + indent,
                    b[1] + self.px(3.0) + i as f32 * self.line_h(),
                    &body,
                    INK,
                );
            }
            p.pop_clip();
            if s.clear.is_some() {
                // Clear the level: an own eraser-like cross in a square.
                let r = Self::clear_rect(b, self.scale);
                p.fill_rect(r[0], r[1], r[2], r[3], LINE);
                p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                let k = self.px(3.5);
                let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                p.path(
                    &[
                        bo_render::PathSeg::M(cx - k, cy - k),
                        bo_render::PathSeg::L(cx + k, cy + k),
                        bo_render::PathSeg::M(cx + k, cy - k),
                        bo_render::PathSeg::L(cx - k, cy + k),
                    ],
                    None,
                    Some((Rgba(0xc4, 0x2b, 0x1c, 255), self.scale.max(1.0))),
                );
            }
        }
        p.pop_clip();
        p.pop_clip();
    }
}

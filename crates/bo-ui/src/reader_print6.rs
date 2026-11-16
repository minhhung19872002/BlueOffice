//! BlueReader round 6 (a child of `reader_print`): the Print dialog's Advanced and Page Setting
//! panels, over the dialog like the reference's sub-dialogs.
//!
//! - Advanced: print as image with its resolution (72 / 150 / 300 / 600 dpi) and the colour
//!   output (colour, grayscale, black and white).
//! - Page Setting: the paper (the printer's, or Letter / Legal / A3 / A4 / A5, ISO 216 and
//!   ANSI sizes) and the margins in millimetres, which the printed area keeps free.
use super::{PrintDialog, BUTTON, BUTTON_EDGE, FIELD, FIELD_BG, GROUP, TEXT, WHITE};
use crate::strings::Strings;
use crate::{Key, Painter};
use bo_render::Rgba;

/// Resolutions offered for printing as an image (dots per inch).
pub(crate) const DPIS: [u32; 4] = [72, 150, 300, 600];
/// Papers: label key (empty: the size as text), size in points (portrait); the first is the
/// printer's own paper.
pub(crate) const PAPERS: [(&str, &str, f32, f32); 6] = [
    ("pdfPrint.paperPrinter", "", 0.0, 0.0),
    ("", "Letter", 612.0, 792.0),
    ("", "Legal", 612.0, 1008.0),
    ("", "A3", 841.89, 1190.55),
    ("", "A4", 595.28, 841.89),
    ("", "A5", 419.53, 595.28),
];

/// Colour output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintColor {
    /// As the document.
    #[default]
    Color,
    /// Shades of grey.
    Gray,
    /// Black and white (each dot either).
    BlackWhite,
}

/// Values when a panel opened: resolution, colour, as image, paper, margins.
type Saved = (usize, PrintColor, bool, usize, [Vec<char>; 4]);

/// Which panel is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Panel {
    Advanced,
    PageSetting,
}

/// Values of the panels and the open one.
#[derive(Debug, Clone)]
pub(crate) struct Sub6 {
    pub dpi: usize,
    pub color: PrintColor,
    pub paper: usize,
    /// Margins in millimetres being typed: top, bottom, left, right.
    pub margins: [Vec<char>; 4],
    pub open: Option<Panel>,
    /// Field taking typing (a margin).
    focus: usize,
    /// Copy of the values when the panel opened (Cancel restores them).
    saved: Option<Saved>,
    /// Click boxes of the last paint.
    hits: std::cell::RefCell<Vec<([f32; 4], String)>>,
}

impl Default for Sub6 {
    fn default() -> Self {
        Self {
            dpi: 1,
            color: PrintColor::Color,
            paper: 0,
            margins: std::array::from_fn(|_| vec!['0']),
            open: None,
            focus: 0,
            saved: None,
            hits: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl Sub6 {
    /// Margins in points: top, bottom, left, right.
    pub(crate) fn margins_pt(&self) -> [f32; 4] {
        self.margins.clone().map(|m| {
            let s: String = m.into_iter().collect();
            s.parse::<f32>().unwrap_or(0.0).clamp(0.0, 100.0) / 25.4 * 72.0
        })
    }

    /// The chosen paper size in points, when not the printer's own.
    pub(crate) fn paper_pt(&self) -> Option<(f32, f32)> {
        let p = PAPERS.get(self.paper)?;
        (p.2 > 0.0).then_some((p.2, p.3))
    }
}

impl PrintDialog {
    /// Open the Advanced or Page Setting panel.
    pub(crate) fn open_sub(&mut self, panel: Panel) {
        let s = &mut self.sub;
        s.saved = Some((s.dpi, s.color, self.as_image, s.paper, s.margins.clone()));
        s.open = Some(panel);
        s.focus = 0;
    }

    /// A click while a panel is open (always taken).
    pub(crate) fn press_sub(&mut self, x: f32, y: f32) {
        let hit = self
            .sub
            .hits
            .borrow()
            .iter()
            .find(|(r, _)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            .map(|h| h.1.clone());
        let Some(id) = hit else { return };
        if let Some(v) = id.strip_prefix("dpi.") {
            self.sub.dpi = v.parse().unwrap_or(1);
        } else if let Some(v) = id.strip_prefix("paper.") {
            self.sub.paper = v.parse().unwrap_or(0);
        } else if let Some(v) = id.strip_prefix("margin.") {
            self.sub.focus = v.parse().unwrap_or(0);
        } else {
            match id.as_str() {
                "image" => self.as_image = !self.as_image,
                "color.0" => self.sub.color = PrintColor::Color,
                "color.1" => self.sub.color = PrintColor::Gray,
                "color.2" => self.sub.color = PrintColor::BlackWhite,
                "ok" => self.close_sub(true),
                "cancel" => self.close_sub(false),
                _ => {}
            }
        }
    }

    fn close_sub(&mut self, keep: bool) {
        if let (false, Some((dpi, color, image, paper, margins))) = (keep, self.sub.saved.take()) {
            self.sub.dpi = dpi;
            self.sub.color = color;
            self.as_image = image;
            self.sub.paper = paper;
            self.sub.margins = margins;
        }
        self.sub.open = None;
        if self.sub.color == PrintColor::Gray {
            self.gray = true;
        }
        self.preview_at = 0;
    }

    /// Typing while a panel is open: digits of the focused margin.
    pub(crate) fn text_sub(&mut self, s: &str) {
        if self.sub.open == Some(Panel::PageSetting) {
            let f = &mut self.sub.margins[self.sub.focus.min(3)];
            if f.len() == 1 && f[0] == '0' {
                f.clear();
            }
            f.extend(s.chars().filter(|c| c.is_ascii_digit() || *c == '.'));
            f.truncate(5);
        }
    }

    /// A key while a panel is open.
    pub(crate) fn key_sub(&mut self, key: Key) {
        match key {
            Key::Escape => self.close_sub(false),
            Key::Enter => self.close_sub(true),
            Key::Backspace => {
                if let Some(f) = self.sub.margins.get_mut(self.sub.focus) {
                    f.pop();
                }
            }
            Key::Tab => self.sub.focus = (self.sub.focus + 1) % 4,
            _ => {}
        }
    }

    /// Paint the open panel over the dialog.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_sub(&self, p: &mut dyn Painter, s: &Strings, caret: bool) {
        let Some(panel) = self.sub.open else { return };
        let f = self.frame;
        let (w, h) = (self.px(420.0), self.px(300.0));
        let r = [
            (f[0] + (f[2] - w) / 2.0).round(),
            (f[1] + (f[3] - h) / 2.0).round(),
            w,
            h,
        ];
        let mut hits = Vec::new();
        p.fill_rect(f[0], f[1], f[2], f[3], Rgba(0, 0, 0, 30));
        p.fill_rect(
            r[0] - 1.0,
            r[1] - 1.0,
            r[2] + 2.0,
            r[3] + 2.0,
            Rgba(0, 0, 0, 255),
        );
        p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
        let st = self.style();
        let line = st.size * 1.33;
        let title = match panel {
            Panel::Advanced => s.get("pdfPrint.advanced"),
            Panel::PageSetting => s.get("pdfPrint.pageSetting"),
        };
        p.text(title, r[0] + self.px(14.0), r[1] + self.px(8.0), st, TEXT);
        let x = r[0] + self.px(18.0);
        let mut y = r[1] + self.px(40.0);
        let row = self.px(24.0);
        // A round option or a check box.
        let option = |p: &mut dyn Painter, x: f32, y: f32, on: bool, round: bool, label: &str| {
            let b = (12.8 * self.scale).round();
            let by = (y + (row - b) / 2.0).round();
            if round {
                let k = b / 2.0;
                p.path(
                    &crate::reader_window::rounded(x, by, b, b, k),
                    Some(FIELD_BG),
                    Some((Rgba(90, 90, 90, 255), 1.0)),
                );
                if on {
                    let d = (b * 0.5).round();
                    let o = ((b - d) / 2.0).round();
                    p.path(
                        &crate::reader_window::rounded(x + o, by + o, d, d, d / 2.0),
                        Some(TEXT),
                        None,
                    );
                }
            } else {
                p.fill_rect(x, by, b, b, Rgba(90, 90, 90, 255));
                p.fill_rect(x + 1.0, by + 1.0, b - 2.0, b - 2.0, FIELD_BG);
                if on {
                    let k = b / 4.0;
                    p.path(
                        &[
                            bo_render::PathSeg::M(x + k, by + 2.0 * k),
                            bo_render::PathSeg::L(x + 1.8 * k, by + 3.0 * k),
                            bo_render::PathSeg::L(x + 3.2 * k, by + k),
                        ],
                        None,
                        Some((TEXT, (1.6 * self.scale).max(1.5))),
                    );
                }
            }
            p.text(
                label,
                x + b + (6.4 * self.scale).round(),
                y + ((row - line) / 2.0).round(),
                st,
                TEXT,
            );
        };
        match panel {
            Panel::Advanced => {
                option(p, x, y, self.as_image, false, s.get("pdfPrint.asImage"));
                hits.push(([x, y, self.px(200.0), row], "image".to_owned()));
                y += row + self.px(6.0);
                p.text(
                    s.get("pdfPrint.resolution"),
                    x,
                    y + ((row - line) / 2.0).round(),
                    st,
                    TEXT,
                );
                let mut ox = x + self.px(110.0);
                for (k, d) in DPIS.iter().enumerate() {
                    option(p, ox, y, self.sub.dpi == k, true, &format!("{d} dpi"));
                    hits.push(([ox, y, self.px(72.0), row], format!("dpi.{k}")));
                    ox += self.px(72.0);
                }
                y += row + self.px(14.0);
                p.fill_rect(x, y, r[2] - self.px(36.0), 1.0, GROUP);
                y += self.px(10.0);
                p.text(
                    s.get("pdfPrint.colorOutput"),
                    x,
                    y + ((row - line) / 2.0).round(),
                    st,
                    TEXT,
                );
                y += row;
                for (k, key) in [
                    "pdfPrint.color",
                    "pdfPrint.grayscaleOut",
                    "pdfPrint.blackWhite",
                ]
                .iter()
                .enumerate()
                {
                    let on = matches!(
                        (k, self.sub.color),
                        (0, PrintColor::Color)
                            | (1, PrintColor::Gray)
                            | (2, PrintColor::BlackWhite)
                    );
                    option(p, x + self.px(12.0), y, on, true, s.get(key));
                    hits.push(([x, y, self.px(240.0), row], format!("color.{k}")));
                    y += row;
                }
            }
            Panel::PageSetting => {
                p.text(
                    s.get("pdfPrint.paper"),
                    x,
                    y + ((row - line) / 2.0).round(),
                    st,
                    TEXT,
                );
                y += row;
                for (k, (key, name, pw, ph)) in PAPERS.iter().enumerate() {
                    let label = if key.is_empty() {
                        format!(
                            "{name} ({:.0} × {:.0} mm)",
                            pw / 72.0 * 25.4,
                            ph / 72.0 * 25.4
                        )
                    } else {
                        s.get(key).to_owned()
                    };
                    let (cx, cy) = (
                        x + self.px(12.0) + (k % 2) as f32 * self.px(190.0),
                        y + (k / 2) as f32 * row,
                    );
                    option(p, cx, cy, self.sub.paper == k, true, &label);
                    hits.push(([cx, cy, self.px(185.0), row], format!("paper.{k}")));
                }
                y += 3.0 * row + self.px(12.0);
                p.text(
                    s.get("pdfPrint.margins"),
                    x,
                    y + ((row - line) / 2.0).round(),
                    st,
                    TEXT,
                );
                y += row;
                let labels = [
                    "pdfPrint.top",
                    "pdfPrint.bottom",
                    "pdfPrint.left",
                    "pdfPrint.right",
                ];
                for (k, key) in labels.iter().enumerate() {
                    let (cx, cy) = (
                        x + self.px(12.0) + (k % 2) as f32 * self.px(190.0),
                        y + (k / 2) as f32 * (row + self.px(6.0)),
                    );
                    p.text(s.get(key), cx, cy + ((row - line) / 2.0).round(), st, TEXT);
                    let fr = [cx + self.px(70.0), cy, self.px(70.0), row];
                    let edge = if self.sub.focus == k {
                        Rgba(0, 120, 212, 255)
                    } else {
                        FIELD
                    };
                    p.fill_rect(fr[0], fr[1], fr[2], fr[3], edge);
                    p.fill_rect(fr[0] + 1.0, fr[1] + 1.0, fr[2] - 2.0, fr[3] - 2.0, FIELD_BG);
                    let v: String = self.sub.margins[k].iter().collect();
                    p.text(
                        &v,
                        fr[0] + self.px(5.0),
                        cy + ((row - line) / 2.0).round(),
                        st,
                        TEXT,
                    );
                    if caret && self.sub.focus == k {
                        let cw = v.chars().count() as f32 * st.size * 0.55;
                        p.fill_rect(
                            fr[0] + self.px(5.0) + cw,
                            cy + self.px(4.0),
                            1.0,
                            row - self.px(8.0),
                            TEXT,
                        );
                    }
                    p.text(
                        "mm",
                        fr[0] + fr[2] + self.px(6.0),
                        cy + ((row - line) / 2.0).round(),
                        st,
                        TEXT,
                    );
                    hits.push((fr, format!("margin.{k}")));
                }
            }
        }
        // OK / Cancel.
        let (bw, bh) = (self.px(80.0), self.px(26.0));
        let by = r[1] + r[3] - bh - self.px(12.0);
        for (k, (key, id)) in [("pdfReader.ok", "ok"), ("pdfPrint.cancel", "cancel")]
            .iter()
            .enumerate()
        {
            let bx = r[0] + r[2] - (2.0 - k as f32) * (bw + self.px(10.0));
            p.fill_rect(bx, by, bw, bh, BUTTON_EDGE);
            p.fill_rect(bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0, BUTTON);
            let label = s.get(key);
            let tw = label.chars().count() as f32 * st.size * 0.5;
            p.text(
                label,
                (bx + (bw - tw) / 2.0).round(),
                by + ((bh - line) / 2.0).round(),
                st,
                TEXT,
            );
            hits.push(([bx, by, bw, bh], (*id).to_owned()));
        }
        *self.sub.hits.borrow_mut() = hits;
    }
}

/// Black and white output: each pixel black or white by its luminance (RGBA bytes).
pub fn black_white(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        let l = 0.299 * f32::from(px[0]) + 0.587 * f32::from(px[1]) + 0.114 * f32::from(px[2]);
        let v = if l < 160.0 { 0 } else { 255 };
        px[0] = v;
        px[1] = v;
        px[2] = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margins_and_papers_in_points() {
        let mut s = Sub6 {
            margins: [
                vec!['1', '0'],
                vec!['0'],
                vec!['2', '5', '.', '4'],
                vec!['x'],
            ],
            ..Sub6::default()
        };
        let m = s.margins_pt();
        assert!((m[0] - 28.35).abs() < 0.01 && m[1] == 0.0 && (m[2] - 72.0).abs() < 0.01);
        assert_eq!(m[3], 0.0);
        assert_eq!(s.paper_pt(), None);
        s.paper = 4;
        assert_eq!(s.paper_pt(), Some((595.28, 841.89)));
        let mut px = vec![200, 200, 200, 255, 90, 10, 10, 255];
        black_white(&mut px);
        assert_eq!(px, vec![255, 255, 255, 255, 0, 0, 0, 255]);
    }
}

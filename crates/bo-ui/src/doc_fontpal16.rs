//! Home › Font Color ▾ of BlueDoc (round 16), laid out as the reference word processor's
//! font colour palette (measured at 125 %, 404 px wide): High-contrast only (on by default:
//! colours with too little contrast against the page are left out and an Additional Colors
//! section of dark colours is shown), Automatic at the top, Theme Colors (10 + 5 tints),
//! Standard Colors, Additional Colors, Recent Colors, More Colors… and Gradient ▸.
use crate::{Painter, TextStyle};
use bo_render::Rgba;

/// What a press on the palette chose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FpPick {
    Nothing,
    /// A colour (`RRGGBB`).
    Color(String),
    /// Automatic.
    Automatic,
    More,
    /// Gradient ▸, with the row's rectangle (its sub-menu opens beside it).
    Gradient([f32; 4]),
}

/// Dark colours of Additional Colors (sampled from the reference's cells, black-box).
const ADDITIONAL: [&str; 20] = [
    "212121", "4B4B4B", "A20000", "590000", "501000", "7F3F00", "C88800", "989800", "4F4F00",
    "2B2B00", "60A500", "005E00", "003400", "00AA48", "009EDE", "004F88", "004E9A", "00204F",
    "002465", "3F0065",
];

const W: f32 = 404.0;
const CELL: f32 = 34.0;
const PITCH: f32 = 40.0;
const LEFT: f32 = 4.0;
const ROW: f32 = 36.0;
const EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const SEP: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const HOT: Rgba = Rgba(0xC5, 0xC5, 0xC5, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

fn rgb(hex: &str) -> (f32, f32, f32) {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    (
        ((v >> 16) & 255) as f32 / 255.0,
        ((v >> 8) & 255) as f32 / 255.0,
        (v & 255) as f32 / 255.0,
    )
}

/// WCAG relative luminance.
fn luminance(hex: &str) -> f32 {
    let f = |c: f32| {
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = rgb(hex);
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

/// Contrast ratio of two colours.
pub(crate) fn contrast(a: &str, b: &str) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The open Font Color palette.
pub(crate) struct FontPalette {
    x: f32,
    y: f32,
    scale: f32,
    /// 60 theme colours then 10 standard colours.
    colors: Vec<String>,
    pub(crate) recent: Vec<String>,
    /// High-contrast only (on by default, as the reference).
    pub(crate) high_contrast: bool,
    /// The page colour the contrast is measured against.
    page: String,
    current: Option<String>,
    /// Labels: High-contrast only, Automatic, Theme Colors, Standard Colors, Additional
    /// Colors, Recent Colors, More Colors…, Gradient.
    labels: [String; 8],
    hover: Option<Hit>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Check,
    Automatic,
    /// Colour cell: index into the shown cells.
    Cell(usize),
    More,
    Gradient,
}

impl FontPalette {
    pub(crate) fn open(
        anchor: [f32; 4],
        size: (f32, f32),
        scale: f32,
        colors: Vec<String>,
        labels: [String; 8],
        current: Option<String>,
        page: Option<String>,
    ) -> Self {
        let k = scale / 1.25;
        let x = anchor[0].min(size.0 - W * k).max(0.0);
        Self {
            x,
            y: anchor[1] + anchor[3],
            scale,
            colors,
            recent: Vec::new(),
            high_contrast: true,
            page: page.unwrap_or_else(|| "FFFFFF".to_owned()),
            current,
            labels,
            hover: None,
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    /// Cell `i` (colour `hex`) is offered: High-contrast only leaves out theme and standard
    /// colours under 3.5:1 against the page (the reference keeps #7F7F7F at 4:1 and drops
    /// #5B9BD5 at 3:1); Additional and Recent Colors always show.
    fn shown(&self, i: usize, hex: &str) -> bool {
        !self.high_contrast || i >= self.colors.len() || contrast(hex, &self.page) >= 3.5
    }

    /// Sections from the top (reference px): theme row, tints, standard row, additional
    /// rows (when on), recent row (when any), the command rows; total height.
    fn layout(&self) -> (f32, f32, f32, Option<f32>, Option<f32>, f32, f32) {
        let theme = 136.0;
        let tints = 190.0;
        let standard = 434.0;
        let mut y = 486.0;
        let additional = self.high_contrast.then(|| {
            let a = y + 46.0;
            y += 139.0;
            a
        });
        let recent = (!self.recent.is_empty()).then(|| {
            let r = y + 46.0;
            y += 98.0;
            r
        });
        (
            theme,
            tints,
            standard,
            additional,
            recent,
            y,
            y + 2.0 * ROW + 2.0,
        )
    }

    /// Every cell: (colour, rectangle in reference px).
    fn cells(&self) -> Vec<(String, [f32; 4])> {
        let (theme, tints, standard, additional, recent, ..) = self.layout();
        let mut out = Vec::new();
        let x = |i: usize| LEFT + (i % 10) as f32 * PITCH;
        for (i, c) in self.colors.iter().enumerate() {
            let r = match i {
                0..=9 => [x(i), theme, CELL, CELL],
                10..=59 => [x(i), tints + ((i - 10) / 10) as f32 * ROW, CELL, ROW],
                _ => [x(i), standard, CELL, CELL],
            };
            out.push((c.clone(), r));
        }
        if let Some(a) = additional {
            for (i, c) in ADDITIONAL.iter().enumerate() {
                out.push((
                    (*c).to_owned(),
                    [x(i), a + (i / 10) as f32 * 41.0, CELL, CELL],
                ));
            }
        }
        if let Some(r) = recent {
            for (i, c) in self.recent.iter().take(10).enumerate() {
                out.push((c.clone(), [x(i), r, CELL, CELL]));
            }
        }
        out
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let h = self.layout().6;
        x >= self.x && x < self.x + self.px(W) && y >= self.y && y < self.y + self.px(h)
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        if !self.contains(x, y) {
            return None;
        }
        let (lx, ly) = ((x - self.x) / self.k(), (y - self.y) / self.k());
        if ly < 48.0 {
            return Some(Hit::Check);
        }
        if ly < 89.0 {
            return Some(Hit::Automatic);
        }
        let (.., cmds, _) = self.layout();
        if ly >= cmds {
            return Some(if ly < cmds + ROW {
                Hit::More
            } else {
                Hit::Gradient
            });
        }
        self.cells()
            .iter()
            .enumerate()
            .position(|(i, (c, r))| {
                self.shown(i, c)
                    && (r[0]..r[0] + r[2]).contains(&lx)
                    && (r[1]..r[1] + r[3]).contains(&ly)
            })
            .map(Hit::Cell)
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> Option<FpPick> {
        if !self.contains(x, y) {
            return None;
        }
        Some(match self.hit(x, y) {
            Some(Hit::Check) => {
                self.high_contrast = !self.high_contrast;
                FpPick::Nothing
            }
            Some(Hit::Automatic) => FpPick::Automatic,
            Some(Hit::More) => FpPick::More,
            Some(Hit::Gradient) => {
                let cmds = self.layout().5;
                FpPick::Gradient([
                    self.x + self.px(W) - 2.0,
                    self.y + self.px(cmds + ROW),
                    1.0,
                    0.0,
                ])
            }
            Some(Hit::Cell(i)) => self
                .cells()
                .get(i)
                .map_or(FpPick::Nothing, |(c, _)| FpPick::Color(c.clone())),
            None => FpPick::Nothing,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let (theme, _, standard, additional, recent, cmds, h) = self.layout();
        let (x, y, w, hh) = (self.x, self.y, self.px(W), self.px(h));
        p.fill_rect(x - 1.0, y - 1.0, w + 2.0, hh + 2.0, Rgba(0, 0, 0, 30));
        p.fill_rect(x, y, w, hh, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, hh - 2.0, WHITE);
        let st = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(14.5),
            bold: true,
        };
        let lh = st.size * 1.33;
        let line =
            |p: &mut dyn Painter, ry: f32| p.fill_rect(x + 1.0, y + self.px(ry), w - 2.0, 1.0, SEP);
        // High-contrast only: a check box that is on by default.
        let b = self.px(18.0);
        let (bx, by) = (x + self.px(8.0), y + self.px(15.0));
        p.fill_rect(bx, by, b, b, EDGE);
        p.fill_rect(bx + 1.0, by + 1.0, b - 2.0, b - 2.0, WHITE);
        if self.high_contrast {
            p.path(
                &[
                    bo_render::PathSeg::M(bx + b * 0.2, by + b * 0.5),
                    bo_render::PathSeg::L(bx + b * 0.42, by + b * 0.72),
                    bo_render::PathSeg::L(bx + b * 0.8, by + b * 0.28),
                ],
                None,
                Some((INK, (1.4 * self.k()).max(1.0))),
            );
        }
        p.text(
            &self.labels[0],
            x + self.px(33.0),
            y + self.px(24.0) - lh / 2.0,
            st,
            INK,
        );
        line(p, 48.0);
        // Automatic: a framed row with a black swatch (current when no colour is set).
        let ar = [
            x + self.px(2.0),
            y + self.px(50.0),
            w - self.px(4.0),
            self.px(38.0),
        ];
        if self.hover == Some(Hit::Automatic) || self.current.is_none() {
            p.fill_rect(ar[0], ar[1], ar[2], ar[3], HOT);
        }
        let sw = self.px(20.0);
        p.fill_rect(
            x + self.px(14.0),
            ar[1] + (ar[3] - sw) / 2.0,
            sw,
            sw,
            Rgba(0, 0, 0, 255),
        );
        p.text(
            &self.labels[1],
            x + self.px(46.0),
            ar[1] + (ar[3] - lh) / 2.0,
            st,
            INK,
        );
        line(p, 89.0);
        p.text(
            &self.labels[2],
            x + self.px(16.0),
            y + self.px(theme - 24.0) - lh / 2.0,
            bold,
            INK,
        );
        line(p, standard - 47.0);
        p.text(
            &self.labels[3],
            x + self.px(16.0),
            y + self.px(standard - 24.0) - lh / 2.0,
            bold,
            INK,
        );
        if let Some(a) = additional {
            line(p, a - 47.0);
            p.text(
                &self.labels[4],
                x + self.px(16.0),
                y + self.px(a - 24.0) - lh / 2.0,
                bold,
                INK,
            );
        }
        if let Some(r) = recent {
            line(p, r - 47.0);
            p.text(
                &self.labels[5],
                x + self.px(16.0),
                y + self.px(r - 24.0) - lh / 2.0,
                bold,
                INK,
            );
        }
        for (i, (c, r)) in self.cells().iter().enumerate() {
            let (cx, cy, cw, ch) = (
                x + self.px(r[0]),
                y + self.px(r[1]),
                self.px(r[2]),
                self.px(r[3]),
            );
            if !self.shown(i, c) {
                // Left out by High-contrast only: an empty cell, as the reference.
                p.fill_rect(cx, cy, cw, 1.0, Rgba(0xE0, 0xE0, 0xE0, 255));
                p.fill_rect(cx, cy + ch - 1.0, cw, 1.0, Rgba(0xE0, 0xE0, 0xE0, 255));
                p.fill_rect(cx, cy, 1.0, ch, Rgba(0xE0, 0xE0, 0xE0, 255));
                p.fill_rect(cx + cw - 1.0, cy, 1.0, ch, Rgba(0xE0, 0xE0, 0xE0, 255));
                continue;
            }
            p.fill_rect(cx, cy, cw, ch, crate::doc_wm14::hex_rgba(c));
            if self.hover == Some(Hit::Cell(i))
                || self
                    .current
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(c))
            {
                let o = Rgba(0xF2, 0x94, 0x36, 255);
                p.fill_rect(cx - 1.0, cy - 1.0, cw + 2.0, 1.0, o);
                p.fill_rect(cx - 1.0, cy + ch, cw + 2.0, 1.0, o);
                p.fill_rect(cx - 1.0, cy, 1.0, ch, o);
                p.fill_rect(cx + cw, cy, 1.0, ch, o);
            }
        }
        line(p, cmds - 1.0);
        for (i, label) in self.labels[6..].iter().enumerate() {
            let ry = y + self.px(cmds + i as f32 * ROW);
            let hot = if i == 0 { Hit::More } else { Hit::Gradient };
            if self.hover == Some(hot) {
                p.fill_rect(x + 2.0, ry, w - 4.0, self.px(ROW), HOT);
            }
            // Own icons: a four-colour wheel for More Colors…, a grey ramp for Gradient.
            let s = self.px(20.0);
            let (ix, iy) = (x + self.px(8.0), ry + (self.px(ROW) - s) / 2.0);
            if i == 0 {
                let q = s / 2.0;
                p.fill_rect(ix, iy, q, q, Rgba(0xE8, 0x4C, 0x3D, 255));
                p.fill_rect(ix + q, iy, q, q, Rgba(0xF1, 0xC4, 0x0F, 255));
                p.fill_rect(ix, iy + q, q, q, Rgba(0x34, 0x98, 0xDB, 255));
                p.fill_rect(ix + q, iy + q, q, q, Rgba(0x2E, 0xCC, 0x71, 255));
            } else {
                let n = 8;
                for j in 0..n {
                    let v = 220 - (j * 100 / n) as u8;
                    let sx = ix + s * j as f32 / n as f32;
                    p.fill_rect(sx, iy, s / n as f32 + 0.5, s, Rgba(v, v, v, 255));
                }
                // The sub-menu mark at the row's end.
                let (mx, my, a) = (x + w - self.px(16.0), ry + self.px(ROW) / 2.0, self.px(4.0));
                p.path(
                    &[
                        bo_render::PathSeg::M(mx - a / 2.0, my - a),
                        bo_render::PathSeg::L(mx + a / 2.0, my),
                        bo_render::PathSeg::L(mx - a / 2.0, my + a),
                        bo_render::PathSeg::Z,
                    ],
                    Some(INK),
                    None,
                );
            }
            p.text(
                label,
                x + self.px(36.0),
                ry + (self.px(ROW) - lh) / 2.0,
                st,
                INK,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_contrast_leaves_out_pale_colours() {
        assert!(contrast("FFFF00", "FFFFFF") < 4.0);
        assert!(contrast("0070C0", "FFFFFF") >= 4.0);
        let labels = std::array::from_fn(|i| format!("l{i}"));
        let colors: Vec<String> = ["FFFF00", "0070C0"]
            .iter()
            .chain(std::iter::repeat_n(&"000000", 68))
            .map(|s| (*s).to_owned())
            .collect();
        let mut p = FontPalette::open(
            [0.0, 0.0, 10.0, 10.0],
            (1500.0, 1100.0),
            1.25,
            colors,
            labels,
            None,
            None,
        );
        // The first theme cell (yellow) is not offered; the second (blue) is.
        let (k, ty) = (1.0, 136.0 + 10.0);
        assert_eq!(p.press(LEFT + 10.0, ty * k), Some(FpPick::Nothing));
        assert_eq!(
            p.press(LEFT + PITCH + 10.0, ty * k),
            Some(FpPick::Color("0070C0".into()))
        );
        // Unticked, everything is offered and Additional Colors goes away.
        assert_eq!(p.press(20.0, 20.0), Some(FpPick::Nothing));
        assert!(!p.high_contrast);
        assert_eq!(
            p.press(LEFT + 10.0, ty * k),
            Some(FpPick::Color("FFFF00".into()))
        );
        assert_eq!(p.press(20.0, 60.0), Some(FpPick::Automatic));
    }
}

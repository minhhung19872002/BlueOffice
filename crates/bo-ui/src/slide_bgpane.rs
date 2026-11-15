//! Format Background pane of BlueSlide (N6, round 4): docked right of the editing area like
//! the reference presentation app's, with a solid fill palette (theme colours, tints and
//! shades, standard colours), Hide background graphics, Apply to All and Reset Background.
use crate::slide_pickers::{hex, rgb, tint, STANDARD};
use crate::spin_box::{inside, paint_check};
use crate::{Painter, TextStyle};
use bo_render::Rgba;

/// What a press in the pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgHit {
    /// Close the pane.
    Close,
    /// Solid fill `RRGGBB` for the current slide.
    Color(String),
    /// Toggle "Hide background graphics".
    HideGraphics,
    /// Give every slide the current slide's background.
    ApplyAll,
    /// Back to the layout's background.
    Reset,
}

/// The pane's state and layout.
#[derive(Debug, Default)]
pub struct BgPane {
    /// Shown.
    pub shown: bool,
    /// Background graphics hidden on the current slide.
    pub hide_graphics: bool,
    /// Last colour chosen (shown as current).
    pub current: Option<String>,
    frame: [f32; 4],
    scale: f32,
    swatches: Vec<([f32; 4], String)>,
    close: [f32; 4],
    hide: [f32; 4],
    apply: [f32; 4],
    reset: [f32; 4],
}

impl BgPane {
    /// Lay out in `r` (window px) at `scale`, with the theme's colours (accents 1–6, dark 1,
    /// light 1).
    pub fn layout(&mut self, r: [f32; 4], scale: f32, theme: &[Rgba]) {
        self.frame = r;
        self.scale = scale;
        let px = |v: f32| (v * scale).round();
        let [x, y, w, _] = r;
        self.close = [x + w - px(30.0), y + px(8.0), px(22.0), px(22.0)];
        let sw = px(17.6);
        let gap = px(4.0);
        let left = x + px(16.0);
        let mut top = y + px(96.0);
        self.swatches.clear();
        let dark = theme.get(6).copied().unwrap_or(Rgba(0, 0, 0, 255));
        let light = theme.get(7).copied().unwrap_or(Rgba(255, 255, 255, 255));
        let mut base = vec![light, dark];
        base.extend(theme.iter().take(6).copied());
        base.push(rgb(0xA5_A5A5));
        base.push(rgb(0x44_546A));
        for (row, t) in [0.0f32, 0.8, 0.6, 0.4, -0.25, -0.5].iter().enumerate() {
            for (c, col) in base.iter().enumerate() {
                let v = if row == 0 {
                    *col
                } else if *col == Rgba(255, 255, 255, 255) {
                    tint(*col, -t.abs() * 0.6)
                } else {
                    tint(*col, *t)
                };
                self.swatches
                    .push(([left + c as f32 * (sw + gap), top, sw, sw], hex(v)));
            }
            top += sw + if row == 0 { px(4.0) } else { 0.0 };
        }
        top += px(10.0);
        for (c, v) in STANDARD.iter().enumerate() {
            self.swatches
                .push(([left + c as f32 * (sw + gap), top, sw, sw], hex(rgb(*v))));
        }
        top += sw + px(20.0);
        self.hide = [left, top, w - px(32.0), px(24.0)];
        top += px(40.0);
        let bw = px(120.0);
        self.apply = [left, top, bw, px(28.0)];
        self.reset = [left + bw + px(10.0), top, bw + px(16.0), px(28.0)];
    }

    /// Whether a window position is over the pane.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.shown && inside(self.frame, x, y)
    }

    /// A press in the pane.
    pub fn press(&self, x: f32, y: f32) -> Option<BgHit> {
        if inside(self.close, x, y) {
            return Some(BgHit::Close);
        }
        if let Some((_, c)) = self.swatches.iter().find(|(r, _)| inside(*r, x, y)) {
            return Some(BgHit::Color(c.clone()));
        }
        if inside(self.hide, x, y) {
            return Some(BgHit::HideGraphics);
        }
        if inside(self.apply, x, y) {
            return Some(BgHit::ApplyAll);
        }
        inside(self.reset, x, y).then_some(BgHit::Reset)
    }

    /// Paint with `labels`: title, Fill, Solid fill, Hide background graphics, Apply to All,
    /// Reset Background.
    pub fn paint(&self, p: &mut dyn Painter, labels: &[String; 6], ink: Rgba) {
        if !self.shown {
            return;
        }
        let px = |v: f32| (v * self.scale).round();
        let [x, y, w, h] = self.frame;
        // Reference task panes: light grey 230 under a 191 divider.
        p.fill_rect(x, y, w, h, Rgba(0xe6, 0xe6, 0xe6, 255));
        p.fill_rect(x, y, 1.0, h, Rgba(0xbf, 0xbf, 0xbf, 255));
        let title = TextStyle {
            size: px(18.0),
            bold: false,
        };
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        let bold = TextStyle {
            bold: true,
            ..style
        };
        p.text(&labels[0], x + px(16.0), y + px(10.0), title, ink);
        // Close cross.
        let [cx, cy, cw, ch] = self.close;
        let k = px(5.0);
        let (mx, my) = (cx + cw / 2.0, cy + ch / 2.0);
        p.path(
            &[
                bo_render::PathSeg::M(mx - k, my - k),
                bo_render::PathSeg::L(mx + k, my + k),
                bo_render::PathSeg::M(mx + k, my - k),
                bo_render::PathSeg::L(mx - k, my + k),
            ],
            None,
            Some((ink, 1.0)),
        );
        p.text(&labels[1], x + px(16.0), y + px(48.0), bold, ink);
        // "Solid fill" chosen (a filled radio button).
        let ry = y + px(70.0);
        let r = px(6.0);
        let segs = |c: f32, cy: f32, r: f32| {
            let k = r * 0.552;
            vec![
                bo_render::PathSeg::M(c + r, cy),
                bo_render::PathSeg::C(c + r, cy + k, c + k, cy + r, c, cy + r),
                bo_render::PathSeg::C(c - k, cy + r, c - r, cy + k, c - r, cy),
                bo_render::PathSeg::C(c - r, cy - k, c - k, cy - r, c, cy - r),
                bo_render::PathSeg::C(c + k, cy - r, c + r, cy - k, c + r, cy),
                bo_render::PathSeg::Z,
            ]
        };
        let rc = x + px(22.0);
        p.path(
            &segs(rc, ry + r, r),
            Some(Rgba(255, 255, 255, 255)),
            Some((Rgba(0x60, 0x60, 0x60, 255), 1.0)),
        );
        p.path(&segs(rc, ry + r, r * 0.5), Some(ink), None);
        p.text(&labels[2], x + px(34.0), ry - px(2.0), style, ink);
        for (r, c) in &self.swatches {
            let col = u32::from_str_radix(c, 16).map_or(Rgba(0, 0, 0, 255), rgb);
            let edge = if self.current.as_deref() == Some(c.as_str()) {
                Rgba(0xd2, 0x47, 0x26, 255)
            } else {
                Rgba(0xc8, 0xc8, 0xc8, 255)
            };
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, col);
        }
        let [hx, hy, _, hh] = self.hide;
        let b = px(14.4);
        paint_check(
            p,
            [hx, hy + (hh - b) / 2.0, b, b],
            self.hide_graphics,
            true,
            ink,
        );
        p.text(
            &labels[3],
            hx + b + px(8.0),
            hy + (hh - style.size * 1.33) / 2.0,
            style,
            ink,
        );
        for (r, label) in [(self.apply, &labels[4]), (self.reset, &labels[5])] {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xad, 0xad, 0xad, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(0xfd, 0xfd, 0xfd, 255),
            );
            let tw = crate::slide_pickers::estimate(label, style);
            p.text(
                label,
                (r[0] + (r[2] - tw) / 2.0).round(),
                r[1] + (r[3] - style.size * 1.33) / 2.0,
                style,
                ink,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_hits_swatches_and_buttons() {
        let mut pane = BgPane {
            shown: true,
            ..BgPane::default()
        };
        pane.layout(
            [1000.0, 200.0, 342.0, 700.0],
            1.25,
            &[Rgba(0x44, 0x72, 0xc4, 255); 8],
        );
        let (r, c) = pane.swatches[3].clone();
        assert_eq!(pane.press(r[0] + 2.0, r[1] + 2.0), Some(BgHit::Color(c)));
        let a = pane.apply;
        assert_eq!(pane.press(a[0] + 3.0, a[1] + 3.0), Some(BgHit::ApplyAll));
        let h = pane.hide;
        assert_eq!(
            pane.press(h[0] + 3.0, h[1] + 3.0),
            Some(BgHit::HideGraphics)
        );
        assert_eq!(pane.swatches.len(), 70);
    }
}

//! Format Background pane of BlueSlide (N6, round 4): docked right of the editing area like
//! the reference presentation app's, with a solid fill palette (theme colours, tints and
//! shades, standard colours), Hide background graphics, Apply to All and Reset Background.
//! Round 20: the Fill choices Solid / Gradient / Pattern fill, preset gradients from the
//! theme's accents with four directions, and twelve preset patterns.
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
    /// Fill kind chosen: 0 solid, 1 gradient, 2 pattern.
    Kind(u8),
    /// A linear gradient through `colors` (`RRGGBB`) at `angle` degrees.
    Gradient {
        /// Stops, evenly spaced.
        colors: Vec<String>,
        /// Direction in degrees (90: top to bottom).
        angle: u16,
    },
    /// A preset pattern (§20.1.10.51) in `fg` on `bg`.
    Pattern {
        /// Preset name.
        preset: String,
        /// Foreground `RRGGBB`.
        fg: String,
        /// Background `RRGGBB`.
        bg: String,
    },
}

/// Preset patterns offered (§20.1.10.51 ST_PresetPatternVal).
pub const PATTERNS: [&str; 12] = [
    "pct10",
    "pct25",
    "pct50",
    "ltHorz",
    "ltVert",
    "ltDnDiag",
    "ltUpDiag",
    "smGrid",
    "lgGrid",
    "smCheck",
    "dotGrid",
    "horzBrick",
];

/// Gradient directions offered (degrees): down, right, diagonal down, diagonal up.
pub const ANGLES: [u16; 4] = [90, 0, 45, 135];

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
    /// Fill kind shown: 0 solid, 1 gradient, 2 pattern.
    pub kind: u8,
    /// The last gradient or pattern chosen (Apply to All gives it to every slide).
    pub last: Option<BgHit>,
    radios: [[f32; 4]; 3],
    /// Gradient and pattern tiles.
    tiles: Vec<([f32; 4], BgHit)>,
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
        for (i, r) in self.radios.iter_mut().enumerate() {
            *r = [left, y + px(70.0 + 24.0 * i as f32), w - px(32.0), px(22.0)];
        }
        let mut top = y + px(150.0);
        self.swatches.clear();
        self.tiles.clear();
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
        if self.kind != 0 {
            self.swatches.clear();
            top = y + px(150.0);
            let t = px(40.0);
            let tg = px(8.0);
            if self.kind == 1 {
                // Preset gradients: light tint to each accent; then the directions of the
                // current (or first) preset.
                let presets: Vec<Vec<String>> = theme
                    .iter()
                    .take(6)
                    .map(|c| vec![hex(tint(*c, 0.8)), hex(*c)])
                    .collect();
                for (i, colors) in presets.iter().enumerate() {
                    self.tiles.push((
                        [left + i as f32 * (t + tg), top, t, t],
                        BgHit::Gradient {
                            colors: colors.clone(),
                            angle: 90,
                        },
                    ));
                }
                top += t + px(30.0);
                let colors = match &self.last {
                    Some(BgHit::Gradient { colors, .. }) => colors.clone(),
                    _ => presets.first().cloned().unwrap_or_default(),
                };
                for (i, a) in ANGLES.iter().enumerate() {
                    self.tiles.push((
                        [left + i as f32 * (t + tg), top, t, t],
                        BgHit::Gradient {
                            colors: colors.clone(),
                            angle: *a,
                        },
                    ));
                }
                top += t + px(20.0);
            } else {
                let fg = theme
                    .first()
                    .map_or_else(|| "4472C4".to_owned(), |c| hex(*c));
                for (i, preset) in PATTERNS.iter().enumerate() {
                    let (row, col) = (i / 6, i % 6);
                    self.tiles.push((
                        [
                            left + col as f32 * (t + tg),
                            top + row as f32 * (t + tg),
                            t,
                            t,
                        ],
                        BgHit::Pattern {
                            preset: (*preset).to_owned(),
                            fg: fg.clone(),
                            bg: "FFFFFF".to_owned(),
                        },
                    ));
                }
                top += 2.0 * (t + tg) + px(12.0);
            }
        }
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
        if let Some(i) = self.radios.iter().position(|r| inside(*r, x, y)) {
            return Some(BgHit::Kind(i as u8));
        }
        if let Some((_, c)) = self.swatches.iter().find(|(r, _)| inside(*r, x, y)) {
            return Some(BgHit::Color(c.clone()));
        }
        if let Some((_, h)) = self.tiles.iter().find(|(r, _)| inside(*r, x, y)) {
            return Some(h.clone());
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
    /// Reset Background, Gradient fill, Pattern fill.
    pub fn paint(&self, p: &mut dyn Painter, labels: &[String; 8], ink: Rgba) {
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
        // The fill kinds as radio buttons, the chosen one filled.
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
        for (i, label) in [&labels[2], &labels[6], &labels[7]].into_iter().enumerate() {
            let ry = self.radios[i][1];
            p.path(
                &segs(rc, ry + r, r),
                Some(Rgba(255, 255, 255, 255)),
                Some((Rgba(0x60, 0x60, 0x60, 255), 1.0)),
            );
            if usize::from(self.kind) == i {
                p.path(&segs(rc, ry + r, r * 0.5), Some(ink), None);
            }
            p.text(label, x + px(34.0), ry - px(2.0), style, ink);
        }
        for (r, h) in &self.tiles {
            let chosen = self.last.as_ref() == Some(h);
            let edge = if chosen {
                Rgba(0xd2, 0x47, 0x26, 255)
            } else {
                Rgba(0xc8, 0xc8, 0xc8, 255)
            };
            p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, edge);
            paint_tile(p, *r, h);
        }
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

/// A gradient or pattern tile's preview (own drawing): gradients as bands of the blended
/// colours along the direction, patterns as their marks in the foreground colour.
fn paint_tile(p: &mut dyn Painter, r: [f32; 4], h: &BgHit) {
    let [x, y, w, hh] = r;
    let col = |c: &str| u32::from_str_radix(c, 16).map_or(Rgba(0, 0, 0, 255), rgb);
    let mix = |a: Rgba, b: Rgba, t: f32| {
        let m = |u: u8, v: u8| (f32::from(u) + (f32::from(v) - f32::from(u)) * t).round() as u8;
        Rgba(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), 255)
    };
    match h {
        BgHit::Gradient { colors, angle } => {
            let (a, b) = (
                col(colors.first().map_or("FFFFFF", String::as_str)),
                col(colors.last().map_or("000000", String::as_str)),
            );
            let n = w.max(hh).round().max(1.0) as usize;
            let rad = f32::from(*angle).to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            p.push_clip(x, y, w, hh);
            // Thin bands across the direction: rows for down, columns for right, and for the
            // diagonals one-pixel rows coloured by their position along the direction.
            for i in 0..n {
                let f = i as f32 / (n - 1).max(1) as f32;
                if *angle == 90 {
                    p.fill_rect(x, y + f * (hh - 1.0), w, 1.0, mix(a, b, f));
                } else if *angle == 0 {
                    p.fill_rect(x + f * (w - 1.0), y, 1.0, hh, mix(a, b, f));
                } else {
                    let yy = y + f * (hh - 1.0);
                    for j in 0..4 {
                        let xx = x + w * j as f32 / 4.0;
                        let along =
                            ((xx - x) * dx + (yy - y) * dy.abs()) / (w * dx + hh * dy.abs());
                        let t = if dy < 0.0 { 1.0 - along } else { along };
                        p.fill_rect(xx, yy, w / 4.0 + 1.0, 1.0, mix(a, b, t.clamp(0.0, 1.0)));
                    }
                }
            }
            p.pop_clip();
        }
        BgHit::Pattern { preset, fg, bg } => {
            let (f, b) = (col(fg), col(bg));
            p.fill_rect(x, y, w, hh, b);
            p.push_clip(x, y, w, hh);
            let step = 4.0;
            let n = (w / step).ceil() as i32;
            let dot = |p: &mut dyn Painter, every: i32| {
                for i in 0..n {
                    for j in 0..n {
                        if (i + j * 3) % every == 0 {
                            p.fill_rect(x + i as f32 * step, y + j as f32 * step, 1.5, 1.5, f);
                        }
                    }
                }
            };
            match preset.as_str() {
                "pct10" => dot(p, 5),
                "pct25" => dot(p, 3),
                "pct50" => dot(p, 2),
                "ltHorz" => (0..n).for_each(|j| p.fill_rect(x, y + j as f32 * step, w, 1.0, f)),
                "ltVert" => (0..n).for_each(|i| p.fill_rect(x + i as f32 * step, y, 1.0, hh, f)),
                "ltDnDiag" | "ltUpDiag" => {
                    for k in -n..n {
                        let o = k as f32 * step;
                        let segs = if preset == "ltDnDiag" {
                            [
                                bo_render::PathSeg::M(x + o, y),
                                bo_render::PathSeg::L(x + o + hh, y + hh),
                            ]
                        } else {
                            [
                                bo_render::PathSeg::M(x + o, y + hh),
                                bo_render::PathSeg::L(x + o + hh, y),
                            ]
                        };
                        p.path(&segs, None, Some((f, 1.0)));
                    }
                }
                "smGrid" | "lgGrid" => {
                    let s = if preset == "smGrid" { step } else { step * 2.0 };
                    let m = (w / s).ceil() as i32;
                    for k in 0..m {
                        p.fill_rect(x, y + k as f32 * s, w, 1.0, f);
                        p.fill_rect(x + k as f32 * s, y, 1.0, hh, f);
                    }
                }
                "smCheck" => {
                    for i in 0..n {
                        for j in 0..n {
                            if (i + j) % 2 == 0 {
                                p.fill_rect(
                                    x + i as f32 * step,
                                    y + j as f32 * step,
                                    step,
                                    step,
                                    f,
                                );
                            }
                        }
                    }
                }
                "dotGrid" => {
                    for i in 0..n {
                        for j in 0..n {
                            if i % 2 == 0 && j % 2 == 0 {
                                p.fill_rect(x + i as f32 * step, y + j as f32 * step, 1.0, 1.0, f);
                            }
                        }
                    }
                }
                _ => {
                    // Bricks: rows with joints offset every other row.
                    let s = step * 2.0;
                    let m = (w / s).ceil() as i32;
                    for j in 0..m {
                        let yy = y + j as f32 * s;
                        p.fill_rect(x, yy, w, 1.0, f);
                        let off = if j % 2 == 0 { 0.0 } else { s };
                        for i in 0..m {
                            p.fill_rect(x + off + i as f32 * s * 2.0, yy, 1.0, s, f);
                        }
                    }
                }
            }
            p.pop_clip();
        }
        _ => {}
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
        // Gradient fill: six presets and four directions; pattern fill: twelve patterns.
        let rg = pane.radios[1];
        assert_eq!(pane.press(rg[0] + 3.0, rg[1] + 3.0), Some(BgHit::Kind(1)));
        pane.kind = 1;
        pane.layout(
            [1000.0, 200.0, 342.0, 700.0],
            1.25,
            &[Rgba(0x44, 0x72, 0xc4, 255); 8],
        );
        assert_eq!(pane.swatches.len(), 0);
        assert_eq!(pane.tiles.len(), 10);
        let (r, h) = pane.tiles[8].clone();
        assert_eq!(pane.press(r[0] + 2.0, r[1] + 2.0), Some(h.clone()));
        assert!(matches!(h, BgHit::Gradient { angle: 45, .. }));
        pane.kind = 2;
        pane.layout(
            [1000.0, 200.0, 342.0, 700.0],
            1.25,
            &[Rgba(0x44, 0x72, 0xc4, 255); 8],
        );
        assert_eq!(pane.tiles.len(), 12);
        // Apply to All stays below the tiles.
        assert!(pane.apply[1] > pane.tiles[11].0[1] + pane.tiles[11].0[3]);
    }
}

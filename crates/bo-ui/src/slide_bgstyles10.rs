//! Design › Background Styles as the reference presentation app shows it (round 10 resume of
//! the slide series): a 4 × 3 gallery of backgrounds from the theme's Light 1, Light 2,
//! Dark 2 and Dark 1 colours — solid, then a subtle and an intense lighter gradient — each
//! with "Aa" in the theme's heading font, then Format Background… and Reset Slide
//! Background. Measured at 125 %: 505 × 370 px, tiles 113 × 86 px every 120 / 93 px from
//! 7 px in, rows 38 px. A choice applies to every slide, as the reference's.
use super::SlideWindow;
use crate::popup::Choice;
use crate::ribbon::RibbonHit;
use crate::{Font, Input, Key, Painter};
use bo_engine::slide_edit::master::MasterCommand;
use bo_render::Rgba;

/// One background of the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgStyle {
    /// A plain colour.
    Solid(Rgba),
    /// A top-to-bottom gradient.
    Gradient(Rgba, Rgba),
}

/// The open gallery.
#[derive(Debug, Clone)]
pub struct BgStyles {
    /// Outer frame (device px).
    pub rect: [f32; 4],
    /// Tiles and their backgrounds, row by row.
    pub tiles: Vec<([f32; 4], BgStyle, Rgba)>,
    /// Format Background…, Reset Slide Background.
    pub rows: [[f32; 4]; 2],
    /// Hovered tile (0–11) or row (12, 13).
    pub hover: Option<usize>,
    /// Heading font of the theme.
    pub font: String,
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);

fn mix(c: Rgba, w: f32) -> Rgba {
    let m = |v: u8| (f32::from(v) + (255.0 - f32::from(v)) * w).round() as u8;
    Rgba(m(c.0), m(c.1), m(c.2), 255)
}

fn hex(c: Rgba) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// The 12 backgrounds from (light 1, light 2, dark 2, dark 1): solid, subtle, intense.
pub fn styles(colors: [Rgba; 4]) -> Vec<BgStyle> {
    let mut v = Vec::with_capacity(12);
    for row in 0..3 {
        for c in colors {
            v.push(match row {
                0 => BgStyle::Solid(c),
                1 => BgStyle::Gradient(mix(c, 0.2), mix(c, 0.1)),
                _ => BgStyle::Gradient(mix(c, 0.55), mix(c, 0.45)),
            });
        }
    }
    v
}

impl SlideWindow {
    /// The current theme's light 1, light 2, dark 2, dark 1 and heading font.
    fn bg_theme(&self) -> ([Rgba; 4], String) {
        let engine = self.view.engine();
        let theme = engine.presentation().and_then(|p| p.themes.first());
        let slot = |name: &str, fallback: Rgba| {
            theme
                .and_then(|t| t.colors.colors.iter().find(|(n, _)| n == name))
                .and_then(|(_, c)| match &c.kind {
                    bo_model::slide::ColorKind::Srgb(v) => Some(*v),
                    bo_model::slide::ColorKind::System { last, .. } => *last,
                    _ => None,
                })
                .map_or(fallback, |v| {
                    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
                })
        };
        let colors = [
            slot("lt1", WHITE),
            slot("lt2", Rgba(0xe7, 0xe6, 0xe6, 255)),
            slot("dk2", Rgba(0x44, 0x54, 0x6a, 255)),
            slot("dk1", Rgba(0, 0, 0, 255)),
        ];
        let font = theme
            .map(|t| t.fonts.major.latin.typeface.clone())
            .filter(|f| !f.is_empty() && !f.starts_with('+'))
            .unwrap_or_else(|| "Calibri Light".to_owned());
        (colors, font)
    }

    /// Open the gallery under the Background Styles button.
    pub(super) fn open_bg_styles(&mut self) -> bool {
        let Some(b) = self.ribbon.rect_of(RibbonHit::Command("design.background")) else {
            return false;
        };
        let s = self.scale;
        let px = |v: f32| (v * s).round();
        let (colors, font) = self.bg_theme();
        let (w, h) = (px(404.0), px(296.0));
        let x = b[0].min(self.size.0 - w - 2.0).max(0.0).round();
        let y = (b[1] + b[3]).round();
        let ink = colors[2];
        let tiles = styles(colors)
            .into_iter()
            .enumerate()
            .map(|(i, st)| {
                let (c, r) = ((i % 4) as f32, (i / 4) as f32);
                let rect = [
                    x + px(5.6) + c * px(96.0),
                    y + px(5.6) + r * px(74.4),
                    px(90.4),
                    px(68.8),
                ];
                let base = match st {
                    BgStyle::Solid(c) | BgStyle::Gradient(c, _) => c,
                };
                let dark = u32::from(base.0) + u32::from(base.1) + u32::from(base.2) < 3 * 140;
                (rect, st, if dark { WHITE } else { ink })
            })
            .collect();
        let ry = y + px(5.6) + 3.0 * px(74.4) + px(2.0);
        self.menu = None;
        self.bgs = Some(BgStyles {
            rect: [x, y, w, h],
            tiles,
            rows: [
                [x + 1.0, ry, w - 2.0, px(30.4)],
                [x + 1.0, ry + px(30.4), w - 2.0, px(30.4)],
            ],
            hover: None,
            font,
        });
        true
    }

    fn bgs_hit(&self, x: f32, y: f32) -> Option<usize> {
        let g = self.bgs.as_ref()?;
        let inside = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        g.tiles
            .iter()
            .position(|(r, ..)| inside(r))
            .or_else(|| g.rows.iter().position(inside).map(|i| 12 + i))
    }

    /// Input while the gallery is open.
    pub(super) fn bgs_input(&mut self, input: &Input) -> Option<bool> {
        let g = self.bgs.as_ref()?;
        let r = g.rect;
        let inside = |x: f32, y: f32| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        Some(match *input {
            Input::Move { x, y } => {
                let h = self.bgs_hit(x, y);
                let g = self.bgs.as_mut()?;
                std::mem::replace(&mut g.hover, h) != h
            }
            Input::Press { x, y, .. } => {
                if inside(x, y) {
                    return Some(false);
                }
                self.bgs = None;
                true
            }
            Input::Release { x, y } => match self.bgs_hit(x, y) {
                Some(i) => self.choose_bg_style(i),
                None => false,
            },
            Input::Key(Key::Escape, _) => {
                self.bgs = None;
                true
            }
            _ => false,
        })
    }

    /// Apply gallery entry `i` (a tile, Format Background… or Reset Slide Background).
    pub(super) fn choose_bg_style(&mut self, i: usize) -> bool {
        let Some(g) = self.bgs.take() else {
            return false;
        };
        match i {
            12 => self.choose(Choice::Command("ins:formatBg".into())),
            13 => self.choose(Choice::Command("bg:".into())),
            _ => match g.tiles.get(i).map(|t| t.1) {
                Some(BgStyle::Solid(c)) => self.view.run_master(MasterCommand::SetBackground {
                    color: Some(hex(c)),
                    all: true,
                }),
                Some(BgStyle::Gradient(a, b)) => {
                    self.view.run_master(MasterCommand::SetBackgroundFill {
                        kind: "gradient".into(),
                        colors: vec![hex(a), hex(b)],
                        angle: Some(90.0),
                        preset: None,
                        all: true,
                    })
                }
                None => false,
            },
        }
    }

    /// Paint the open gallery.
    pub(super) fn paint_bg_styles(&self, p: &mut dyn Painter) {
        let Some(g) = &self.bgs else { return };
        let s = self.scale;
        let [x, y, w, h] = g.rect;
        // Soft shadow, grey frame, white ground.
        for (k, a) in [(3.0f32, 10u8), (2.0, 18), (1.0, 28)] {
            p.fill_rect(
                x - k + 2.0,
                y - k + 2.0,
                w + 2.0 * k,
                h + 2.0 * k,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, Rgba(148, 148, 148, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        let font = Font {
            family: &g.font,
            size: (24.0 * s).round(),
            weight: 400,
            italic: false,
            underline: false,
        };
        for (i, (r, st, ink)) in g.tiles.iter().enumerate() {
            let [tx, ty, tw, th] = *r;
            if g.hover == Some(i) {
                p.fill_rect(tx - 3.0, ty - 3.0, tw + 6.0, th + 6.0, Rgba(0, 0, 0, 255));
                p.fill_rect(tx - 1.0, ty - 1.0, tw + 2.0, th + 2.0, WHITE);
            }
            p.fill_rect(tx, ty, tw, th, Rgba(98, 108, 129, 255));
            let (ix, iy, iw, ih) = (tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0);
            match *st {
                BgStyle::Solid(c) => p.fill_rect(ix, iy, iw, ih, c),
                BgStyle::Gradient(a, b) => {
                    let n = 24;
                    for k in 0..n {
                        let f = k as f32 / (n - 1) as f32;
                        let l = |u: u8, v: u8| {
                            (f32::from(u) + (f32::from(v) - f32::from(u)) * f).round() as u8
                        };
                        let band = ih / n as f32;
                        p.fill_rect(
                            ix,
                            (iy + band * k as f32).floor(),
                            iw,
                            band.ceil() + 1.0,
                            Rgba(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), 255),
                        );
                    }
                    p.fill_rect(ix, iy + ih, iw, 1.0, Rgba(98, 108, 129, 255));
                }
            }
            p.text_font(
                "Aa",
                tx + (10.0 * s).round(),
                ty + (8.0 * s).round(),
                &font,
                *ink,
            );
        }
        let body = crate::TextStyle {
            size: (13.0 * s).round(),
            bold: false,
        };
        let labels = [
            self.t("slideNative.formatBackgroundMenu", "Format Background..."),
            self.t("slideR10.bgReset", "Reset Slide Background"),
        ];
        p.fill_rect(
            x + 1.0,
            g.rows[0][1] - 1.0,
            w - 2.0,
            1.0,
            Rgba(225, 225, 225, 255),
        );
        for (k, (r, label)) in g.rows.iter().zip(labels).enumerate() {
            if g.hover == Some(12 + k) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(229, 241, 251, 255));
            }
            let isz = (16.0 * s).round();
            let colors = crate::icons::IconColors {
                ink: self.theme.text,
                accent: self.theme.accent,
                on: self.theme.accent_on,
                tinted: k == 0,
            };
            self.icons.draw(
                p,
                "design.background",
                r[0] + (8.0 * s).round(),
                r[1] + (r[3] - isz) / 2.0,
                isz,
                colors,
            );
            p.text(
                &label,
                r[0] + (36.0 * s).round(),
                r[1] + (r[3] - body.size * 1.33) / 2.0,
                body,
                Rgba(38, 38, 38, 255),
            );
        }
    }
}

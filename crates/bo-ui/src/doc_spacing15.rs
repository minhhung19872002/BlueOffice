//! Design › Paragraph Spacing (BlueDoc round 15), laid out from the reference word
//! processor's menu captured at 125 % (311 × 552 px): a "Style Set" band with Default, a
//! "Built-In" band with six presets in 63 px rows, and Custom Paragraph Spacing…. The preset
//! values were read back black-box from the reference (Normal style after each one); the
//! icons are BlueOffice's own drawings.
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const BAND: Rgba = Rgba(0xeb, 0xeb, 0xeb, 255);
const HOT: Rgba = Rgba(0xc5, 0xc5, 0xc5, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const LABEL: Rgba = Rgba(0x44, 0x44, 0x44, 255);
const NAME: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const RULE: Rgba = Rgba(0xe2, 0xe4, 0xe7, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const LINE: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const ACCENT: Rgba = Rgba(0x2f, 0x6f, 0xba, 255);

/// Menu size and rows (reference px).
const W: f32 = 311.0;
const H: f32 = 552.0;
const DEFAULT_Y: f32 = 37.0;
const DEFAULT_H: f32 = 63.0;
const BUILT_Y: f32 = 100.0;
const ROWS_Y: f32 = 135.0;
const ROW_H: f32 = 63.0;
const FOOT_Y: f32 = 514.0;

/// The six presets: id, en-US name, space before / after (twips), line spacing word of
/// `FindFormat::line_spacing` (read back from the reference: 0 / 0 single, 0 / 4 pt single,
/// 0 / 6 pt 1.15, 0 / 10 pt 1.15, 0 / 6 pt 1.5 lines, 0 / 8 pt double).
pub(crate) const PRESETS: [(&str, &str, i32, i32, &str); 6] = [
    ("noSpace", "No Paragraph Space", 0, 0, "single"),
    ("compact", "Compact", 0, 80, "single"),
    ("tight", "Tight", 0, 120, "multiple:276"),
    ("open", "Open", 0, 200, "multiple:276"),
    ("relaxed", "Relaxed", 0, 120, "1.5"),
    ("double", "Double", 0, 160, "double"),
];

/// What a press chose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SpacingPick {
    Nothing,
    /// The style set's own spacing.
    Default,
    /// Preset `i` of [`PRESETS`].
    Preset(usize),
    /// Custom Paragraph Spacing….
    Custom,
}

/// The open Paragraph Spacing menu.
pub(crate) struct SpacingMenu {
    x: f32,
    y: f32,
    scale: f32,
    /// Style Set, Default, Built-In, the six preset names, Custom Paragraph Spacing….
    labels: Vec<String>,
    /// The row in use (0 = Default, 1–6 presets).
    pub(crate) current: Option<usize>,
    hover: Option<usize>,
}

impl SpacingMenu {
    pub(crate) fn open(
        anchor: [f32; 4],
        size: (f32, f32),
        scale: f32,
        labels: Vec<String>,
        current: Option<usize>,
    ) -> Self {
        let k = scale / 1.25;
        let x = anchor[0].min(size.0 - W * k).max(0.0);
        Self {
            x,
            y: anchor[1] + anchor[3],
            scale,
            labels,
            current,
            hover: None,
        }
    }

    fn k(&self) -> f32 {
        self.scale / 1.25
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.k()).round()
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.px(W) && y >= self.y && y < self.y + self.px(H)
    }

    /// Row under (x, y): 0 Default, 1–6 presets, 7 Custom….
    fn row_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let ly = (y - self.y) / self.k();
        if (DEFAULT_Y..DEFAULT_Y + DEFAULT_H).contains(&ly) {
            Some(0)
        } else if (ROWS_Y..ROWS_Y + 6.0 * ROW_H).contains(&ly) {
            Some(1 + ((ly - ROWS_Y) / ROW_H) as usize)
        } else if ly >= FOOT_Y {
            Some(7)
        } else {
            None
        }
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.row_at(x, y);
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    /// `None`: the press was outside (the menu closes).
    pub(crate) fn press(&self, x: f32, y: f32) -> Option<SpacingPick> {
        if !self.contains(x, y) {
            return None;
        }
        Some(match self.row_at(x, y) {
            Some(0) => SpacingPick::Default,
            Some(i @ 1..=6) => SpacingPick::Preset(i - 1),
            Some(_) => SpacingPick::Custom,
            None => SpacingPick::Nothing,
        })
    }

    /// Centre of row i (0 Default, 1–6 presets, 7 Custom…; tests).
    #[cfg(test)]
    pub(crate) fn row_center(&self, i: usize) -> (f32, f32) {
        let ly = match i {
            0 => DEFAULT_Y + DEFAULT_H / 2.0,
            1..=6 => ROWS_Y + (i as f32 - 0.5) * ROW_H,
            _ => FOOT_Y + 15.0,
        };
        (self.x + self.px(150.0), self.y + self.px(ly))
    }

    fn label(&self, i: usize) -> &str {
        self.labels.get(i).map_or("", String::as_str)
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let (x, y, w, h) = (self.x, self.y, self.px(W), self.px(H));
        p.fill_rect(x, y, w, h, EDGE);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        let st = TextStyle {
            size: self.px(14.5),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(14.5),
            bold: true,
        };
        let lh = st.size * 1.33;
        let band = |p: &mut dyn Painter, top: f32, bottom: f32, text: &str| {
            let (by, bh) = (y + self.px(top), self.px(bottom - top));
            p.fill_rect(x + 1.0, by, w - 2.0, bh, BAND);
            p.text(text, x + self.px(12.0), by + (bh - lh) / 2.0, bold, LABEL);
        };
        band(p, 2.0, DEFAULT_Y, self.label(0));
        band(p, BUILT_Y, ROWS_Y, self.label(2));
        // Default (the style set's spacing) and the presets.
        for row in 0..7 {
            let (top, rh) = if row == 0 {
                (DEFAULT_Y, DEFAULT_H)
            } else {
                (ROWS_Y + (row - 1) as f32 * ROW_H, ROW_H)
            };
            let (ry, rhp) = (y + self.px(top), self.px(rh));
            let on = self.current == Some(row);
            if on || self.hover == Some(row) {
                p.fill_rect(x + 2.0, ry, w - 4.0, rhp, HOT);
            }
            if on {
                let t = self.px(2.0).max(1.0);
                outline(p, [x + 2.0, ry, w - 4.0, rhp], t, INK);
            }
            let name = if row == 0 {
                self.label(1)
            } else {
                self.label(2 + row)
            };
            p.text(name, x + self.px(74.0), ry + (rhp - lh) / 2.0, st, NAME);
            let icon = [
                x + self.px(12.0),
                ry + self.px(10.0),
                self.px(42.0),
                self.px(42.0),
            ];
            if row == 0 {
                paint_default_icon(p, icon);
            } else {
                paint_preset_icon(p, icon, row - 1);
            }
        }
        p.fill_rect(x + 1.0, y + self.px(FOOT_Y - 1.0), w - 2.0, 1.0, RULE);
        let fy = y + self.px(FOOT_Y);
        let fh = self.px(H - FOOT_Y - 1.0);
        if self.hover == Some(7) {
            p.fill_rect(x + 2.0, fy, w - 4.0, fh, HOT);
        }
        p.text(
            self.label(9),
            x + self.px(46.0),
            fy + (fh - lh) / 2.0,
            st,
            INK,
        );
    }
}

fn outline(p: &mut dyn Painter, r: [f32; 4], t: f32, c: Rgba) {
    p.fill_rect(r[0], r[1], r[2], t, c);
    p.fill_rect(r[0], r[1] + r[3] - t, r[2], t, c);
    p.fill_rect(r[0], r[1], t, r[3], c);
    p.fill_rect(r[0] + r[2] - t, r[1], t, r[3], c);
}

/// A preset: a sheet whose grey text lines are spread by the preset's spacing, with a blue
/// two-headed gauge on the right (own drawing).
fn paint_preset_icon(p: &mut dyn Painter, r: [f32; 4], i: usize) {
    let k = r[2] / 42.0;
    p.fill_rect(r[0], r[1], r[2], r[3], LINE);
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
    // Line pitch and the gap after each two-line paragraph grow with the preset.
    let pitch = [3.0, 3.0, 3.5, 3.5, 4.5, 6.0][i.min(5)] * k;
    let gap = [0.0, 2.5, 4.0, 7.0, 4.0, 5.5][i.min(5)] * k;
    let mut ly = r[1] + 5.0 * k;
    let mut n = 0;
    while ly < r[1] + r[3] - 4.0 * k {
        let lw = if n % 2 == 1 { 16.0 } else { 24.0 } * k;
        p.fill_rect(r[0] + 5.0 * k, ly.round(), lw.round(), k.max(1.0), LINE);
        ly += pitch;
        if n % 2 == 1 {
            ly += gap;
        }
        n += 1;
    }
    let cx = r[0] + 34.0 * k;
    let (top, bottom) = (r[1] + 8.0 * k, r[1] + r[3] - 8.0 * k);
    p.fill_rect(cx - k, top, 2.0 * k, bottom - top, ACCENT);
    let a = 3.5 * k;
    for (yy, s) in [(top, -1.0), (bottom, 1.0)] {
        p.path(
            &[
                PathSeg::M(cx - a, yy),
                PathSeg::L(cx + a, yy),
                PathSeg::L(cx, yy + s * a),
                PathSeg::Z,
            ],
            Some(ACCENT),
            None,
        );
    }
}

/// Default: a sheet with even lines and a blue turning-back arrow (own drawing).
fn paint_default_icon(p: &mut dyn Painter, r: [f32; 4]) {
    let k = r[2] / 42.0;
    p.fill_rect(r[0], r[1], r[2], r[3], LINE);
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
    for i in 0..6 {
        let ly = r[1] + (16.0 + 4.0 * i as f32) * k;
        p.fill_rect(r[0] + 5.0 * k, ly.round(), 30.0 * k, k.max(1.0), LINE);
    }
    let (cx, cy, rr) = (r[0] + 21.0 * k, r[1] + 10.0 * k, 5.0 * k);
    p.path(
        &[
            PathSeg::M(cx - rr, cy + 2.0 * k),
            PathSeg::L(cx - rr, cy),
            PathSeg::L(cx + rr, cy),
            PathSeg::L(cx + rr, cy + 3.0 * k),
        ],
        None,
        Some((ACCENT, 2.0 * k)),
    );
    let a = 3.0 * k;
    p.path(
        &[
            PathSeg::M(cx - rr - a, cy + 2.0 * k),
            PathSeg::L(cx - rr + a, cy + 2.0 * k),
            PathSeg::L(cx - rr, cy + 2.0 * k + a),
            PathSeg::Z,
        ],
        Some(ACCENT),
        None,
    );
}

/// Which row matches document defaults (space before / after in twips, line spacing word):
/// a preset, else Default when they are the style set's own (`default`).
pub(crate) fn current_row(
    now: (i32, i32, String),
    default: Option<&(i32, i32, String)>,
) -> Option<usize> {
    if let Some(i) = PRESETS
        .iter()
        .position(|p| (p.2, p.3, p.4) == (now.0, now.1, now.2.as_str()))
    {
        return Some(i + 1);
    }
    (default == Some(&now)).then_some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> SpacingMenu {
        SpacingMenu::open(
            [100.0, 100.0, 80.0, 30.0],
            (1500.0, 1100.0),
            1.25,
            (0..10).map(|i| format!("l{i}")).collect(),
            Some(0),
        )
    }

    #[test]
    fn rows_follow_the_reference_metrics() {
        let m = menu();
        assert_eq!(
            m.press(150.0, 130.0 + 37.0 + 30.0),
            Some(SpacingPick::Default)
        );
        assert_eq!(
            m.press(150.0, 130.0 + 135.0 + 63.0 * 2.0 + 10.0),
            Some(SpacingPick::Preset(2))
        );
        assert_eq!(m.press(150.0, 130.0 + 530.0), Some(SpacingPick::Custom));
        assert_eq!(m.press(150.0, 130.0 + 110.0), Some(SpacingPick::Nothing));
        assert_eq!(m.press(50.0, 50.0), None);
    }

    #[test]
    fn current_row_finds_presets() {
        let d = (0, 160, "multiple:259".to_owned());
        assert_eq!(current_row((0, 80, "single".into()), Some(&d)), Some(2));
        assert_eq!(current_row(d.clone(), Some(&d)), Some(0));
        assert_eq!(current_row((0, 1, "single".into()), Some(&d)), None);
    }
}

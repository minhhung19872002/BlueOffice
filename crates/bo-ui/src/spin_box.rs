//! Small ribbon controls drawn in the reference desktop apps' style (N6, BlueSlide round 3):
//! a number box with up / down arrows (spin edit: transition Duration, After; effect
//! Duration, Delay), a drop-down box (Sound, Start) and a check box. Measured on the
//! reference presentation app at 125 %: boxes framed 1 px grey 148 on white, 32 px high,
//! the arrow column 16 px wide, check boxes 18 px framed grey 128.
//!
//! Stateless painting and hit testing, plus [`SpinEdit`] for typing a value.
use crate::{Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const FRAME: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const CHECK_FRAME: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const WHITE: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const ARROW: Rgba = Rgba(0x60, 0x60, 0x60, 255);
const HOVER: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const SEPARATOR: Rgba = Rgba(0xdc, 0xdc, 0xdc, 255);

/// Part of a spin box under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinHit {
    /// The number field.
    Field,
    /// The up arrow (larger value).
    Up,
    /// The down arrow.
    Down,
}

/// Width of the arrow column of spin and drop-down boxes (device px).
fn arrows_w(scale: f32) -> f32 {
    (12.8 * scale).round()
}

/// Which part of the spin box `r` (`[x, y, w, h]`, device px) holds (`x`, `y`).
#[must_use]
pub fn spin_hit(r: [f32; 4], scale: f32, x: f32, y: f32) -> Option<SpinHit> {
    if x < r[0] || x >= r[0] + r[2] || y < r[1] || y >= r[1] + r[3] {
        return None;
    }
    if x < r[0] + r[2] - arrows_w(scale) {
        return Some(SpinHit::Field);
    }
    Some(if y < r[1] + r[3] / 2.0 {
        SpinHit::Up
    } else {
        SpinHit::Down
    })
}

/// Whether (`x`, `y`) is inside `r`.
#[must_use]
pub fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn frame(p: &mut dyn Painter, r: [f32; 4], color: Rgba) {
    let [x, y, w, h] = r;
    p.fill_rect(x, y, w, h, color);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
}

/// A thin chevron (`up`: pointing up) centred on (`cx`, `cy`), `k` px half wide.
fn chevron(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, up: bool, color: Rgba) {
    let dy = if up { -k / 2.0 } else { k / 2.0 };
    p.path(
        &[
            PathSeg::M(cx - k, cy - dy),
            PathSeg::L(cx, cy + dy),
            PathSeg::L(cx + k, cy - dy),
        ],
        None,
        Some((color, 1.0)),
    );
}

/// How a box shows its value.
#[derive(Debug, Clone, Copy)]
pub struct BoxLook<'a> {
    /// Text in the field.
    pub text: &'a str,
    /// Interface text style.
    pub style: TextStyle,
    /// Text colour.
    pub ink: Rgba,
    /// Can be used (dimmed otherwise).
    pub enabled: bool,
    /// Typing in it: draw the caret after this many device px of text.
    pub caret: Option<f32>,
    /// Part under the pointer.
    pub hover: Option<SpinHit>,
}

/// Draw a spin box in `r`: the value left in the field, a small up and a down chevron in
/// the arrow column.
pub fn paint_spin(p: &mut dyn Painter, r: [f32; 4], scale: f32, look: &BoxLook<'_>) {
    let [x, y, w, h] = r;
    frame(p, r, FRAME);
    let aw = arrows_w(scale);
    let ax = x + w - aw - 1.0;
    match look.hover {
        Some(SpinHit::Up) => p.fill_rect(ax, y + 1.0, aw, (h / 2.0).floor() - 1.0, HOVER),
        Some(SpinHit::Down) => p.fill_rect(
            ax,
            y + (h / 2.0).floor(),
            aw,
            h - (h / 2.0).floor() - 1.0,
            HOVER,
        ),
        _ => {}
    }
    let ink = if look.enabled { look.ink } else { DIM };
    let tx = x + (2.4 * scale).round();
    let ty = (y + (h - look.style.size * 1.33) / 2.0).round();
    p.text(look.text, tx, ty, look.style, ink);
    if let Some(cw) = look.caret {
        p.fill_rect(
            (tx + cw).round(),
            y + (5.0 * scale).round(),
            1.0,
            h - (10.0 * scale).round(),
            look.ink,
        );
    }
    let arrow = if look.enabled { ARROW } else { DIM };
    let k = (2.4 * scale).round();
    let cx = (ax + aw / 2.0).round() + 0.5;
    chevron(p, cx, y + h * 0.3, k, true, arrow);
    chevron(p, cx, y + h * 0.7, k, false, arrow);
}

/// Draw a drop-down box in `r`: the value, a separator and the arrow.
pub fn paint_combo(p: &mut dyn Painter, r: [f32; 4], scale: f32, look: &BoxLook<'_>) {
    let [x, y, w, h] = r;
    frame(p, r, FRAME);
    let aw = (16.0 * scale).round();
    let ax = x + w - aw - 1.0;
    if look.hover.is_some() {
        p.fill_rect(ax, y + 1.0, aw, h - 2.0, HOVER);
    }
    p.fill_rect(ax, y + 1.0, 1.0, h - 2.0, SEPARATOR);
    let ink = if look.enabled { look.ink } else { DIM };
    let ty = (y + (h - look.style.size * 1.33) / 2.0).round();
    p.text(look.text, x + (2.4 * scale).round(), ty, look.style, ink);
    let k = (2.8 * scale).round();
    let (cx, cy) = ((ax + aw / 2.0).round() + 0.5, (y + h / 2.0).round());
    p.path(
        &[
            PathSeg::M(cx - k, cy - k / 2.0),
            PathSeg::L(cx + k, cy - k / 2.0),
            PathSeg::L(cx, cy + k / 2.0),
            PathSeg::Z,
        ],
        Some(if look.enabled { ARROW } else { DIM }),
        None,
    );
}

/// Draw a check box (`r` is the square) with a tick when `on`.
pub fn paint_check(p: &mut dyn Painter, r: [f32; 4], on: bool, enabled: bool, ink: Rgba) {
    let [x, y, w, h] = r;
    frame(p, r, if enabled { CHECK_FRAME } else { DIM });
    if on {
        let c = if enabled { ink } else { DIM };
        p.path(
            &[
                PathSeg::M(x + w * 0.22, y + h * 0.52),
                PathSeg::L(x + w * 0.42, y + h * 0.72),
                PathSeg::L(x + w * 0.8, y + h * 0.3),
            ],
            None,
            Some((c, (w / 14.0).max(1.0))),
        );
    }
}

/// A value being typed into a spin box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpinEdit {
    /// Which box (caller's id).
    pub id: &'static str,
    /// Text typed so far.
    pub text: String,
}

/// What a key does to a [`SpinEdit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinKey {
    /// The text changed (or nothing happened that needs more than a redraw).
    Edited,
    /// Commit the text.
    Commit,
    /// Leave without changing the value.
    Cancel,
    /// Step the value up.
    Up,
    /// Step the value down.
    Down,
    /// Not for the box.
    Ignored,
}

impl SpinEdit {
    /// Type `s` (digits and separators only).
    pub fn type_text(&mut self, s: &str) -> bool {
        let ok: String = s
            .chars()
            .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ':'))
            .collect();
        if ok.is_empty() || self.text.len() >= 12 {
            return false;
        }
        self.text.push_str(&ok);
        true
    }

    /// Apply a key.
    pub fn key(&mut self, k: Key) -> SpinKey {
        match k {
            Key::Backspace => {
                self.text.pop();
                SpinKey::Edited
            }
            Key::Enter | Key::Tab => SpinKey::Commit,
            Key::Escape => SpinKey::Cancel,
            Key::Up => SpinKey::Up,
            Key::Down => SpinKey::Down,
            _ => SpinKey::Ignored,
        }
    }
}

/// Seconds as the reference's Duration box shows them (`02.00`).
#[must_use]
pub fn fmt_seconds(ms: u32) -> String {
    format!("{:05.2}", f64::from(ms) / 1000.0)
}

/// Time as the reference's After box shows it (`00:03.00`, minutes and seconds).
#[must_use]
pub fn fmt_clock(ms: u32) -> String {
    let s = f64::from(ms) / 1000.0;
    let m = (s / 60.0).floor();
    format!("{:02}:{:05.2}", m as u32, s - m * 60.0)
}

/// Milliseconds of a typed time: seconds (`2`, `2.5`, `2,5`) or minutes and seconds
/// (`1:05.5`).
#[must_use]
pub fn parse_ms(text: &str) -> Option<u32> {
    let t = text.trim().replace(',', ".");
    let (m, s) = match t.split_once(':') {
        Some((m, s)) => (m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        None => (0.0, t.parse::<f64>().ok()?),
    };
    let ms = ((m * 60.0 + s) * 1000.0).round();
    (ms.is_finite() && (0.0..=86_400_000.0).contains(&ms)).then_some(ms as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_like_the_reference() {
        assert_eq!(fmt_seconds(2000), "02.00");
        assert_eq!(fmt_seconds(750), "00.75");
        assert_eq!(fmt_clock(0), "00:00.00");
        assert_eq!(fmt_clock(65_500), "01:05.50");
        assert_eq!(parse_ms("02.00"), Some(2000));
        assert_eq!(parse_ms("1,25"), Some(1250));
        assert_eq!(parse_ms("01:05.50"), Some(65_500));
        assert_eq!(parse_ms("x"), None);
    }

    #[test]
    fn hits_the_field_and_the_arrows() {
        let r = [100.0, 10.0, 100.0, 32.0];
        assert_eq!(spin_hit(r, 1.25, 120.0, 20.0), Some(SpinHit::Field));
        assert_eq!(spin_hit(r, 1.25, 195.0, 15.0), Some(SpinHit::Up));
        assert_eq!(spin_hit(r, 1.25, 195.0, 35.0), Some(SpinHit::Down));
        assert_eq!(spin_hit(r, 1.25, 205.0, 20.0), None);
    }

    #[test]
    fn typing_keeps_digits_and_separators() {
        let mut e = SpinEdit {
            id: "x",
            text: String::new(),
        };
        assert!(e.type_text("1"));
        assert!(!e.type_text("a"));
        assert!(e.type_text(",5"));
        assert_eq!(e.text, "1,5");
        assert_eq!(e.key(Key::Backspace), SpinKey::Edited);
        assert_eq!(e.text, "1,");
        assert_eq!(e.key(Key::Enter), SpinKey::Commit);
    }
}

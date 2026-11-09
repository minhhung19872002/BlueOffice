//! Pure layout helpers for cell text, shared by the grid renderer
//! ([`crate::render`], which also feeds PNG / PDF output):
//!
//! * `_x` padding and `*x` repeat fill of number format codes (ECMA-376 Part 1,
//!   §18.8.31): a `_x` gap is as wide as `x`; the `*x` character repeats to fill
//!   the cell width that the rest of the text leaves free;
//! * `shrinkToFit` (§18.8.1 alignment): the font is scaled down so the text fits
//!   the cell width;
//! * `textRotation` (§18.8.1: 1–90 degrees
//!   counter-clockwise, 91–180 meaning 1–90 degrees clockwise, 255 vertical
//!   stacked text): the bounding box and placement of a rotated line.
//!
//! Widths come from a caller-supplied `measure` function so the arithmetic can
//! be tested without fonts.

use crate::geometry::Rect;
use crate::numfmt::Formatted;

/// A run of text placed at `x` (points from the start of the line).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Text of the run.
    pub text: String,
    /// Horizontal offset from the line start.
    pub x: f64,
}

/// Text run, `_x` gap or `*x` fill position of a line.
enum Piece {
    Run(String),
    Gap(char),
    Fill,
}

/// A single formatted line with `_x` gaps and `*x` fill resolved.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LineLayout {
    /// Runs to draw, in order.
    pub segments: Vec<Segment>,
    /// Total advance width of the line.
    pub width: f64,
    /// The whole line as text (gaps as spaces, fill repeated) — for wrapping / hit tests.
    pub text: String,
}

/// Lay out one line of formatted text for a cell `avail` points wide.
///
/// `_x` placeholders (see [`Formatted::skips`]) become gaps exactly as wide as
/// `x`; the `*x` fill character is repeated as many whole times as fit in the
/// width left over by the rest of the line (none when the line is already too wide).
pub fn layout_line(
    fmt: &Formatted,
    avail: f64,
    measure: &mut dyn FnMut(&str) -> f64,
) -> LineLayout {
    let text = fmt.text.as_str();
    // Marker offsets, in order: (offset, Some(gap char)) for skips, (offset, None) for the fill.
    let mut marks: Vec<(usize, Option<char>)> = fmt
        .skips
        .iter()
        .filter(|(p, _)| *p < text.len() && text.is_char_boundary(*p))
        .map(|(p, c)| (*p, Some(*c)))
        .collect();
    if let Some((p, _)) = fmt
        .fill
        .filter(|(p, _)| *p <= text.len() && text.is_char_boundary(*p))
    {
        marks.push((p, None));
    }
    marks.sort_by_key(|(p, c)| (*p, c.is_some()));

    let mut pieces = Vec::new();
    let mut start = 0;
    for (p, gap) in marks {
        if p < start {
            continue;
        }
        if p > start {
            pieces.push(Piece::Run(text[start..p].to_owned()));
        }
        if let Some(c) = gap {
            pieces.push(Piece::Gap(c));
            // Skip the placeholder space (one byte).
            start = p + 1;
        } else {
            pieces.push(Piece::Fill);
            start = p;
        }
    }
    if start < text.len() {
        pieces.push(Piece::Run(text[start..].to_owned()));
    }

    let widths: Vec<f64> = pieces
        .iter()
        .map(|pc| match pc {
            Piece::Run(s) => measure(s),
            Piece::Gap(c) => measure(&c.to_string()),
            Piece::Fill => 0.0,
        })
        .collect();
    let base: f64 = widths.iter().sum();
    let fill_run = fmt.fill.map(|(_, c)| {
        let one = c.to_string();
        let cw = measure(&one).max(0.1);
        let n = ((avail - base) / cw).floor().max(0.0) as usize;
        (one.repeat(n.min(1000)), cw * n.min(1000) as f64)
    });

    let mut out = LineLayout::default();
    let mut x = 0.0;
    for (pc, w) in pieces.iter().zip(widths) {
        match pc {
            Piece::Run(s) => {
                out.segments.push(Segment { text: s.clone(), x });
                out.text.push_str(s);
                x += w;
            }
            Piece::Gap(_) => {
                out.text.push(' ');
                x += w;
            }
            Piece::Fill => {
                if let Some((s, fw)) = &fill_run {
                    if !s.is_empty() {
                        out.segments.push(Segment { text: s.clone(), x });
                        out.text.push_str(s);
                        x += fw;
                    }
                }
            }
        }
    }
    out.width = x;
    out
}

/// Smallest font scale `shrinkToFit` applies (keeps the text legible).
pub const MIN_SHRINK: f64 = 0.1;

/// Font scale factor for `shrinkToFit` (§18.8.1): text `width` points wide
/// shrinks proportionally to fit `avail`; text that already fits is unchanged.
pub fn shrink_scale(width: f64, avail: f64) -> f64 {
    if width <= avail || width <= 0.0 {
        1.0
    } else {
        (avail / width).clamp(MIN_SHRINK, 1.0)
    }
}

/// Rotation in degrees counter-clockwise for a `textRotation` value (§18.8.1):
/// 1–90 → +1…+90, 91–180 → −1…−90; 0 and 255 (stacked) are not rotations.
pub fn rotation_degrees(text_rotation: u32) -> Option<f64> {
    match text_rotation {
        1..=90 => Some(f64::from(text_rotation)),
        91..=180 => Some(-f64::from(text_rotation - 90)),
        _ => None,
    }
}

/// Size (width, height) of the axis-aligned box around a `w`×`h` line rotated by `deg`.
pub fn rotated_extent(w: f64, h: f64, deg: f64) -> (f64, f64) {
    let (sin, cos) = deg.to_radians().sin_cos();
    (
        (w * cos).abs() + (h * sin).abs(),
        (w * sin).abs() + (h * cos).abs(),
    )
}

/// Placement of a box inside a cell along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Left / top.
    Start,
    /// Centred.
    Center,
    /// Right / bottom.
    End,
}

/// Geometry of a rotated single line in a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotatedLine {
    /// Centre of the rotated box (the rotation origin).
    pub cx: f64,
    /// Centre of the rotated box.
    pub cy: f64,
    /// Width of the axis-aligned bounding box.
    pub bw: f64,
    /// Height of the axis-aligned bounding box.
    pub bh: f64,
    /// Baseline start in the rotated frame (x = −w/2, y = ascent − (ascent + descent)/2).
    pub local_x: f64,
    /// Baseline in the rotated frame.
    pub local_y: f64,
    /// sin of the angle.
    pub sin: f64,
    /// cos of the angle.
    pub cos: f64,
}

impl RotatedLine {
    /// Place a line `w` wide with font `ascent` / `descent` rotated `deg` degrees
    /// counter-clockwise in `rect`, aligned by `h` / `v` with `pad` (horizontal
    /// padding; half of it vertically) and `indent` from the left edge.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rect: Rect,
        w: f64,
        ascent: f64,
        descent: f64,
        deg: f64,
        h: Place,
        v: Place,
        pad: f64,
        indent: f64,
    ) -> RotatedLine {
        let lh = ascent + descent;
        let (bw, bh) = rotated_extent(w, lh, deg);
        let cx = match h {
            Place::Start => rect.x + pad + indent + bw / 2.0,
            Place::End => rect.x + rect.w - pad - bw / 2.0,
            Place::Center => rect.x + rect.w / 2.0,
        };
        let cy = match v {
            Place::Start => rect.y + pad / 2.0 + bh / 2.0,
            Place::Center => rect.y + rect.h / 2.0,
            Place::End => rect.y + rect.h - pad / 2.0 - bh / 2.0,
        };
        let (sin, cos) = deg.to_radians().sin_cos();
        RotatedLine {
            cx,
            cy,
            bw,
            bh,
            local_x: -w / 2.0,
            local_y: ascent - lh / 2.0,
            sin,
            cos,
        }
    }

    /// Transform matrix `[a b c d e f]` (screen y down, counter-clockwise on screen):
    /// x' = cos·x + sin·y + cx, y' = −sin·x + cos·y + cy.
    pub fn matrix(&self) -> [f64; 6] {
        [self.cos, -self.sin, self.sin, self.cos, self.cx, self.cy]
    }

    /// Page position of a point given in the rotated frame.
    pub fn to_page(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.cx + self.cos * x + self.sin * y,
            self.cy - self.sin * x + self.cos * y,
        )
    }

    /// Page position of the start of the baseline.
    pub fn baseline_start(&self) -> (f64, f64) {
        self.to_page(self.local_x, self.local_y)
    }
}

#[cfg(test)]
mod tests;

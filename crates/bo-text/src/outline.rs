//! Glyph outlines, shared by the rasterizer, the PDF writer (for Type 3 fallback)
//! and the web canvas (which receives SVG path strings).

use crate::{FaceId, FontManager};
use std::fmt::Write as _;

/// Path command in font units, y axis pointing **up** (font convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

struct Collector(Vec<PathCmd>);

impl ttf_parser::OutlineBuilder for Collector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(PathCmd::MoveTo(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(PathCmd::LineTo(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(PathCmd::QuadTo(x1, y1, x, y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(PathCmd::CubicTo(x1, y1, x2, y2, x, y));
    }
    fn close(&mut self) {
        self.0.push(PathCmd::Close);
    }
}

impl FontManager {
    /// Outline of a glyph (empty for blank glyphs such as space).
    pub fn outline(&self, id: FaceId, glyph_id: u16) -> Vec<PathCmd> {
        let mut c = Collector(Vec::new());
        if let Some(face) = self.face(id) {
            let _ = face.outline_glyph(ttf_parser::GlyphId(glyph_id), &mut c);
        }
        c.0
    }
}

/// Encode a path as an SVG path string **with y flipped** (y down), still in font units,
/// so the canvas can draw it with `scale(size / unitsPerEm)` directly.
pub fn outline_svg_path(cmds: &[PathCmd]) -> String {
    let mut s = String::new();
    for c in cmds {
        let _ = match *c {
            PathCmd::MoveTo(x, y) => write!(s, "M{x} {}", -y),
            PathCmd::LineTo(x, y) => write!(s, "L{x} {}", -y),
            PathCmd::QuadTo(x1, y1, x, y) => write!(s, "Q{x1} {} {x} {}", -y1, -y),
            PathCmd::CubicTo(x1, y1, x2, y2, x, y) => {
                write!(s, "C{x1} {} {x2} {} {x} {}", -y1, -y2, -y)
            }
            PathCmd::Close => write!(s, "Z"),
        };
    }
    s
}

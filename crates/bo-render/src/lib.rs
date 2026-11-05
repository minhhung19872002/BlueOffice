//! Display list: a backend-independent list of drawing commands per page.
//!
//! The web canvas (`packages/canvas-host`), the PNG rasterizer (`bo-raster`) and
//! the PDF writer (`bo-pdf`) all consume the same display list, so what is shown
//! on screen is what is printed and exported (prompt §5.5).
//!
//! Coordinates are points (1/72 inch), origin top-left, y down. Conversion to
//! device pixels (zoom × devicePixelRatio) happens in each backend.
#![forbid(unsafe_code)]

use bo_layout::LayoutDocument;
use bo_model::Underline;
use bo_text::FontManager;
use serde::{Deserialize, Serialize};

/// RGBA colour, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl From<bo_model::Color> for Rgba {
    fn from(c: bo_model::Color) -> Self {
        Rgba(c.r, c.g, c.b, 255)
    }
}

/// A glyph: `[glyph id, x, y, cluster]` relative to the run origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Glyph(pub u16, pub f32, pub f32, pub u32);

/// Drawing command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Command {
    /// Filled axis-aligned rectangle.
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgba,
    },
    /// Straight line segment.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Rgba,
    },
    /// Glyph run: `face` indexes the engine's font list; `size` in points.
    Glyphs {
        face: u32,
        size: f32,
        x: f32,
        y: f32,
        color: Rgba,
        text: String,
        glyphs: Vec<Glyph>,
    },
    /// Intersect the clip with a rectangle (paired with `PopClip`).
    PushClip {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    PopClip,
}

/// Commands of one page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageList {
    pub width: f32,
    pub height: f32,
    pub commands: Vec<Command>,
}

/// Display list of a whole document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DisplayList {
    pub pages: Vec<PageList>,
}

impl PageList {
    /// JSON encoding used across the WASM / Tauri bridge.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Build the display list for a laid-out document.
pub fn build_display_list(layout: &LayoutDocument, fonts: &FontManager) -> DisplayList {
    let pages = layout
        .pages
        .iter()
        .map(|page| {
            let mut commands = Vec::new();
            for line in &page.lines {
                for f in &line.fragments {
                    let color: Rgba = f.color.into();
                    if !f.glyphs.is_empty() && !f.text.trim().is_empty() {
                        commands.push(Command::Glyphs {
                            face: f.face.0,
                            size: f.size,
                            x: f.x,
                            y: f.y,
                            color,
                            text: f.text.clone(),
                            glyphs: f
                                .glyphs
                                .iter()
                                .map(|g| Glyph(g.id, g.x, g.y, g.cluster))
                                .collect(),
                        });
                    }
                    let info = fonts.info(f.face);
                    let scale = f.size / f32::from(info.units_per_em);
                    let thickness = (f32::from(info.underline_thickness) * scale).max(0.5);
                    // Underline: ECMA-376 Part 1, §17.3.2.40; position from the font's `post` table.
                    let mut deco = |y: f32| {
                        commands.push(Command::Line {
                            x1: f.x,
                            y1: y,
                            x2: f.x + f.width,
                            y2: y,
                            width: thickness,
                            color,
                        });
                    };
                    match f.underline {
                        Underline::None => {}
                        Underline::Single => deco(f.y - f32::from(info.underline_position) * scale),
                        Underline::Double => {
                            let y = f.y - f32::from(info.underline_position) * scale;
                            deco(y - thickness);
                            deco(y + thickness * 1.5);
                        }
                    }
                    if f.strike {
                        deco(f.y - f32::from(info.strikeout_position) * scale);
                    }
                }
            }
            PageList {
                width: page.width,
                height: page.height,
                commands,
            }
        })
        .collect();
    DisplayList { pages }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape() {
        let p = PageList {
            width: 10.0,
            height: 20.0,
            commands: vec![
                Command::Glyphs {
                    face: 1,
                    size: 12.0,
                    x: 1.0,
                    y: 2.0,
                    color: Rgba(0, 0, 0, 255),
                    text: "A".into(),
                    glyphs: vec![Glyph(36, 0.0, 0.0, 0)],
                },
                Command::PopClip,
            ],
        };
        let j = p.to_json();
        assert!(j.contains(r#""op":"glyphs""#));
        assert!(j.contains(r#""glyphs":[[36,0.0,0.0,0]]"#));
        assert!(j.contains(r#"{"op":"popClip"}"#));
        let back: PageList = serde_json::from_str(&j).unwrap();
        assert_eq!(back, p);
    }
}

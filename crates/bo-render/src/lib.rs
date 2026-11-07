//! Display list: a backend-independent list of drawing commands per page.
//!
//! The web canvas (`packages/canvas-host`), the PNG rasterizer (`bo-raster`) and
//! the PDF writer (`bo-pdf`) all consume the same display list, so what is shown
//! on screen is what is printed and exported (prompt §5.5).
//!
//! Coordinates are points (1/72 inch), origin top-left, y down. Conversion to
//! device pixels (zoom × devicePixelRatio) happens in each backend.
#![forbid(unsafe_code)]

pub mod image;

pub use image::Media;

use bo_layout::{Layer, LayoutDocument, Line, PageItem};
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
    /// Straight line segment; `dash` = (on, off) lengths for dashed/dotted lines.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Rgba,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<[f32; 2]>,
    },
    /// Picture: `media` names the image part (fetched by hosts via the engine),
    /// drawn into the `w`×`h` box at (`x`, `y`); `crop` = l,t,r,b fractions of the
    /// source to cut away; `rotation` in degrees clockwise about the box centre.
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        media: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<[f32; 4]>,
        #[serde(default, skip_serializing_if = "is_zero")]
        rotation: f32,
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

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Dash pattern of a border style (ECMA-376 Part 1, §17.18.2 `ST_Border`).
fn border_dash(style: &str, width: f32) -> Option<[f32; 2]> {
    match style {
        "dotted" => Some([width, width * 2.0]),
        "dashed" | "dashSmallGap" | "dotDash" | "dotDotDash" | "dashDotStroked" => {
            Some([width * 3.0, width * 2.0])
        }
        _ => None,
    }
}

fn item_commands(item: &PageItem, out: &mut Vec<Command>) {
    match item {
        PageItem::Rect {
            x, y, w, h, color, ..
        } => out.push(Command::FillRect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            color: (*color).into(),
        }),
        PageItem::Rule {
            x1,
            y1,
            x2,
            y2,
            width,
            color,
            style,
        } => {
            let color: Rgba = (*color).into();
            let mut line = |off: f32, w: f32| {
                let horizontal = (y1 - y2).abs() < f32::EPSILON;
                let (dx, dy) = if horizontal { (0.0, off) } else { (off, 0.0) };
                out.push(Command::Line {
                    x1: x1 + dx,
                    y1: y1 + dy,
                    x2: x2 + dx,
                    y2: y2 + dy,
                    width: w,
                    color,
                    dash: border_dash(style, w),
                });
            };
            match style.as_str() {
                // Double lines: two strokes with a gap equal to the stroke width.
                "double" | "thinThickSmallGap" | "thickThinSmallGap" => {
                    let w = (width / 3.0).max(0.25);
                    line(-w, w);
                    line(w, w);
                }
                "thick" => line(0.0, *width * 1.5),
                _ => line(0.0, *width),
            }
        }
        PageItem::Image {
            x,
            y,
            w,
            h,
            media,
            crop,
            rotation,
            ..
        } => out.push(Command::Image {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            media: media.clone(),
            crop: *crop,
            rotation: *rotation,
        }),
    }
}

fn line_commands(line: &Line, fonts: &FontManager, commands: &mut Vec<Command>) {
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
        let mut deco = |y: f32, width: f32, dash: Option<[f32; 2]>| {
            commands.push(Command::Line {
                x1: f.x,
                y1: y,
                x2: f.x + f.width,
                y2: y,
                width,
                color,
                dash,
            });
        };
        let uy = f.y - f32::from(info.underline_position) * scale;
        match f.underline {
            Underline::None => {}
            Underline::Single | Underline::Words | Underline::Wave => deco(uy, thickness, None),
            Underline::Thick => deco(uy, thickness * 2.0, None),
            Underline::Double => {
                deco(uy - thickness, thickness, None);
                deco(uy + thickness * 1.5, thickness, None);
            }
            Underline::Dotted => deco(uy, thickness, Some([thickness, thickness * 2.0])),
            Underline::Dash => deco(uy, thickness, Some([thickness * 4.0, thickness * 2.0])),
        }
        let sy = f.y - f32::from(info.strikeout_position) * scale;
        if f.strike {
            deco(sy, thickness, None);
        }
        if f.double_strike {
            deco(sy - thickness, thickness, None);
            deco(sy + thickness, thickness, None);
        }
    }
}

/// Build the display list for a laid-out document. Paint order per page:
/// back items (shading, highlights, behind-text pictures) → text → front items
/// (borders, pictures).
pub fn build_display_list(layout: &LayoutDocument, fonts: &FontManager) -> DisplayList {
    let pages = layout
        .pages
        .iter()
        .map(|page| {
            let mut commands = Vec::new();
            for item in page.items.iter().filter(|i| i.layer() == Layer::Back) {
                item_commands(item, &mut commands);
            }
            for line in &page.lines {
                line_commands(line, fonts, &mut commands);
            }
            for item in page.items.iter().filter(|i| i.layer() == Layer::Front) {
                item_commands(item, &mut commands);
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

/// Media part names referenced by a page list.
pub fn media_of(page: &PageList) -> Vec<&str> {
    let mut v: Vec<&str> = page
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Image { media, .. } => Some(media.as_str()),
            _ => None,
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
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
                Command::Image {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                    media: "/word/media/a.png".into(),
                    crop: None,
                    rotation: 0.0,
                },
                Command::Line {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 0.0,
                    width: 1.0,
                    color: Rgba(0, 0, 0, 255),
                    dash: None,
                },
            ],
        };
        let j = p.to_json();
        assert!(j.contains(
            r#"{"op":"image","x":0.0,"y":0.0,"w":10.0,"h":10.0,"media":"/word/media/a.png"}"#
        ));
        assert!(!j.contains("dash"));
        assert_eq!(media_of(&p), vec!["/word/media/a.png"]);
        assert!(j.contains(r#""op":"glyphs""#));
        assert!(j.contains(r#""glyphs":[[36,0.0,0.0,0]]"#));
        assert!(j.contains(r#"{"op":"popClip"}"#));
        let back: PageList = serde_json::from_str(&j).unwrap();
        assert_eq!(back, p);
    }
}

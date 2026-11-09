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
pub mod metafile;

pub use image::{ImageSource, Media};

use bo_layout::{Layer, LayoutDocument, Line, LineSource, Page, PageItem, PathCmd};
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
        /// Left edge (points).
        x: f32,
        /// Top edge (points).
        y: f32,
        /// Width (points).
        w: f32,
        /// Height (points).
        h: f32,
        /// Fill colour.
        color: Rgba,
    },
    /// Straight line segment; `dash` = (on, off) lengths for dashed/dotted lines.
    Line {
        /// Start x (points).
        x1: f32,
        /// Start y (points).
        y1: f32,
        /// End x (points).
        x2: f32,
        /// End y (points).
        y2: f32,
        /// Stroke width (points).
        width: f32,
        /// Stroke colour.
        color: Rgba,
        /// (on, off) dash lengths in points; `None` = solid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<[f32; 2]>,
    },

    /// Glyph run: `face` indexes the engine's font list; `size` in points.
    Glyphs {
        /// Index into the engine's font list.
        face: u32,
        /// Font size (points).
        size: f32,
        /// Run origin x (points).
        x: f32,
        /// Run origin y on the baseline (points).
        y: f32,
        /// Text colour.
        color: Rgba,
        /// Source text of the run (for selection, search and PDF text).
        text: String,
        /// Positioned glyphs relative to the run origin.
        glyphs: Vec<Glyph>,
    },
    /// Intersect the clip with a rectangle (paired with `PopClip`).
    PushClip {
        /// Left edge (points).
        x: f32,
        /// Top edge (points).
        y: f32,
        /// Width (points).
        w: f32,
        /// Height (points).
        h: f32,
    },
    /// Restore the clip saved by the matching `PushClip` / `PushClipPath`.
    PopClip,
    /// Picture drawn into the `w`×`h` box at (`x`, `y`). `image` is an id the host
    /// resolves to PNG/JPEG bytes (`Engine::image_bytes`: a slide media id or a
    /// document media part such as `/word/media/image1.png`); `crop` = l,t,r,b
    /// fractions of the source cut away; `rotation` in degrees clockwise about the box centre.
    Image {
        /// Left edge of the box (points).
        x: f32,
        /// Top edge of the box (points).
        y: f32,
        /// Box width (points).
        w: f32,
        /// Box height (points).
        h: f32,
        /// Picture id resolved by the host.
        image: String,
        /// Left, top, right, bottom fractions of the source cut away.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<[f32; 4]>,
        /// Clockwise rotation in degrees about the box centre.
        #[serde(default, skip_serializing_if = "is_zero")]
        rotation: f32,
    },
    /// Filled and/or stroked vector path (DrawingML shapes, ECMA-376 Part 1, §20.1.9).
    /// Coordinates are already in page points (after the current transform).
    Path {
        /// Path segments (points).
        segs: Vec<PathSeg>,
        /// Fill paint, if filled.
        fill: Option<Paint>,
        /// Stroke, if outlined.
        stroke: Option<Stroke>,
    },
    /// Concatenate an affine transform `[a, b, c, d, e, f]`
    /// (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`) onto the current one (paired with `PopTransform`).
    PushTransform {
        /// Affine matrix `[a, b, c, d, e, f]`.
        m: [f32; 6],
    },
    /// Restore the transform saved by the matching `PushTransform`.
    PopTransform,
    /// Intersect the clip with a path (nonzero rule; paired with `PopClip`).
    PushClipPath {
        /// Clip path segments (points).
        segs: Vec<PathSeg>,
    },
    /// Multiply the opacity of the following commands by `alpha` (0..=1),
    /// paired with `PopOpacity` (DrawingML §20.1.8.4 alphaModFix of pictures).
    PushOpacity {
        /// Opacity factor in 0..=1.
        alpha: f32,
    },
    /// Restore the opacity saved by the matching `PushOpacity`.
    PopOpacity,
    /// Filled path blurred by `radius` points (soft shadow, §20.1.8.49
    /// outerShdw@blurRad); grown by `spread` points first (outline stroked
    /// `2 × spread` wide: glow, §20.1.8.32); coordinates as `Path`.
    BlurPath {
        /// Path segments (points).
        segs: Vec<PathSeg>,
        /// Shadow/glow colour.
        color: Rgba,
        /// Blur radius (points).
        radius: f32,
        /// Outline growth before blurring (points).
        #[serde(default, skip_serializing_if = "is_zero")]
        spread: f32,
    },
}

/// Path segment; JSON: `{"M":[x,y]}`, `{"L":[x,y]}`, `{"C":[x1,y1,x2,y2,x,y]}`, `{"Q":[x1,y1,x,y]}`, `"Z"`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PathSeg {
    /// Move to.
    M(f32, f32),
    /// Line to.
    L(f32, f32),
    /// Quadratic Bézier.
    Q(f32, f32, f32, f32),
    /// Cubic Bézier.
    C(f32, f32, f32, f32, f32, f32),
    /// Close subpath.
    Z,
}

/// Gradient colour stop; `offset` in 0..=1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position along the gradient in 0..=1.
    pub offset: f32,
    /// Colour at this stop.
    pub color: Rgba,
}

/// Fill paint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Paint {
    /// Uniform colour.
    Solid {
        /// Fill colour.
        color: Rgba,
    },
    /// Linear gradient from (x1,y1) to (x2,y2), pad spread.
    Linear {
        /// Start x (points).
        x1: f32,
        /// Start y (points).
        y1: f32,
        /// End x (points).
        x2: f32,
        /// End y (points).
        y2: f32,
        /// Colour stops in increasing offset.
        stops: Vec<GradientStop>,
    },
    /// Radial gradient centred at (cx,cy) with radius r, pad spread.
    Radial {
        /// Centre x (points).
        cx: f32,
        /// Centre y (points).
        cy: f32,
        /// Radius (points).
        r: f32,
        /// Colour stops from centre outwards.
        stops: Vec<GradientStop>,
    },
}

/// Stroke style. `dash` lengths are in points (empty = solid).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Line width (points).
    pub width: f32,
    /// Stroke colour.
    pub color: Rgba,
    /// Dash pattern lengths (points); empty = solid.
    pub dash: Vec<f32>,
    /// `"butt" | "round" | "square"`.
    pub cap: String,
    /// `"miter" | "round" | "bevel"`.
    pub join: String,
}

/// Commands of one page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageList {
    /// Page width (points).
    pub width: f32,
    /// Page height (points).
    pub height: f32,
    /// Drawing commands in painting order.
    pub commands: Vec<Command>,
}

/// Display list of a whole document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DisplayList {
    /// Pages in document order.
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
    // Semi-transparent items (watermarks) are wrapped in an opacity group.
    let alpha = match item {
        PageItem::Image { alpha, .. } | PageItem::Shape { alpha, .. } => *alpha,
        _ => 1.0,
    };
    if alpha < 0.999 {
        out.push(Command::PushOpacity {
            alpha: alpha.clamp(0.0, 1.0),
        });
        item_commands_opaque(item, out);
        out.push(Command::PopOpacity);
    } else {
        item_commands_opaque(item, out);
    }
}

fn item_commands_opaque(item: &PageItem, out: &mut Vec<Command>) {
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
            image: media.clone(),
            crop: *crop,
            rotation: *rotation,
        }),
        PageItem::Shape { x, y, paths, .. } => {
            for p in paths {
                let segs = p
                    .segs
                    .iter()
                    .map(|s| match *s {
                        PathCmd::M(a, b) => PathSeg::M(x + a, y + b),
                        PathCmd::L(a, b) => PathSeg::L(x + a, y + b),
                        PathCmd::Q(a, b, c, d) => PathSeg::Q(x + a, y + b, x + c, y + d),
                        PathCmd::C(a, b, c, d, e, f) => {
                            PathSeg::C(x + a, y + b, x + c, y + d, x + e, y + f)
                        }
                        PathCmd::Z => PathSeg::Z,
                    })
                    .collect();
                out.push(Command::Path {
                    segs,
                    fill: p.fill.map(|c| Paint::Solid { color: c.into() }),
                    stroke: p.stroke.as_ref().map(|(w, c, dash)| Stroke {
                        width: *w,
                        color: (*c).into(),
                        dash: shape_dash(dash.as_deref(), *w),
                        cap: "butt".into(),
                        join: "miter".into(),
                    }),
                });
            }
        }
    }
}

/// Dash lengths of a preset line dash (ECMA-376 Part 1, §20.1.10.48
/// `ST_PresetLineDashVal`, in multiples of the line width).
fn shape_dash(name: Option<&str>, w: f32) -> Vec<f32> {
    let pat: &[f32] = match name {
        Some("dot" | "sysDot") => &[1.0, 1.0],
        Some("dash" | "sysDash") => &[4.0, 3.0],
        Some("lgDash") => &[8.0, 3.0],
        Some("dashDot" | "sysDashDot") => &[4.0, 3.0, 1.0, 3.0],
        Some("lgDashDot") => &[8.0, 3.0, 1.0, 3.0],
        Some("lgDashDotDot" | "sysDashDotDot") => &[8.0, 3.0, 1.0, 3.0, 1.0, 3.0],
        _ => &[],
    };
    pat.iter().map(|v| v * w.max(0.5)).collect()
}

/// Horizontal spans `(x0, x1)` of a fragment that carry ink: the whole fragment, or
/// for `words` underlining (§17.18.99) only the runs of non-space characters.
fn underline_spans(f: &bo_layout::Fragment, words: bool) -> Vec<(f32, f32)> {
    if !words {
        return vec![(f.x, f.x + f.width)];
    }
    let mut out = Vec::new();
    let mut cur: Option<(f32, f32)> = None;
    for (i, g) in f.glyphs.iter().enumerate() {
        let next = f.glyphs.get(i + 1).map_or(f.width, |n| n.x);
        let space = f
            .text
            .get(g.cluster as usize..)
            .and_then(|s| s.chars().next())
            .is_some_and(char::is_whitespace);
        if space {
            out.extend(cur.take());
        } else {
            let (x0, x1) = (f.x + g.x, f.x + next.max(g.x));
            cur = Some(cur.map_or((x0, x1), |(a, _)| (a, x1)));
        }
    }
    out.extend(cur);
    out
}

/// Wavy line from `x0` to `x1` around `y` (quadratic arcs).
fn wave(x0: f32, x1: f32, y: f32, amp: f32) -> Vec<PathSeg> {
    let step = (amp * 4.0).max(1.5);
    let mut segs = vec![PathSeg::M(x0, y)];
    let mut x = x0;
    let mut up = true;
    while x < x1 - 0.01 {
        let nx = (x + step).min(x1);
        let cy = if up { y - amp * 2.0 } else { y + amp * 2.0 };
        segs.push(PathSeg::Q(f32::midpoint(x, nx), cy, nx, y));
        x = nx;
        up = !up;
    }
    segs
}

/// Underline of a fragment in every `ST_Underline` style (ECMA-376 Part 1, §17.18.99):
/// heavy styles are twice as thick, dashes and dots are stroke dash patterns.
fn underline_commands(
    f: &bo_layout::Fragment,
    uy: f32,
    t: f32,
    color: Rgba,
    out: &mut Vec<Command>,
) {
    use Underline as U;
    let heavy = matches!(
        f.underline,
        U::Thick
            | U::DottedHeavy
            | U::DashedHeavy
            | U::DashLongHeavy
            | U::DashDotHeavy
            | U::DashDotDotHeavy
            | U::WavyHeavy
    );
    let w = if heavy { t * 2.0 } else { t };
    let dash: Vec<f32> = match f.underline {
        U::Dotted | U::DottedHeavy => vec![w, w * 2.0],
        U::Dash | U::DashedHeavy => vec![w * 4.0, w * 2.0],
        U::DashLong | U::DashLongHeavy => vec![w * 8.0, w * 3.0],
        U::DotDash | U::DashDotHeavy => vec![w * 4.0, w * 2.0, w, w * 2.0],
        U::DotDotDash | U::DashDotDotHeavy => vec![w * 4.0, w * 2.0, w, w * 2.0, w, w * 2.0],
        _ => Vec::new(),
    };
    let stroke = |width: f32, dash: Vec<f32>| Stroke {
        width,
        color,
        dash,
        cap: "butt".into(),
        join: "miter".into(),
    };
    for (x0, x1) in underline_spans(f, f.underline == U::Words) {
        if x1 - x0 <= 0.01 {
            continue;
        }
        let mut line = |y: f32, width: f32| {
            out.push(Command::Line {
                x1: x0,
                y1: y,
                x2: x1,
                y2: y,
                width,
                color,
                dash: None,
            });
        };
        match f.underline {
            U::None => {}
            U::Single | U::Words | U::Thick => line(uy, w),
            U::Double => {
                line(uy - t, t);
                line(uy + t * 1.5, t);
            }
            U::Wave | U::WavyHeavy => out.push(Command::Path {
                segs: wave(x0, x1, uy + w * 0.5, w * 0.75),
                fill: None,
                stroke: Some(stroke(w, Vec::new())),
            }),
            U::WavyDouble => {
                for dy in [-t * 0.8, t * 1.6] {
                    out.push(Command::Path {
                        segs: wave(x0, x1, uy + dy, t * 0.6),
                        fill: None,
                        stroke: Some(stroke(t, Vec::new())),
                    });
                }
            }
            _ => out.push(Command::Path {
                segs: vec![PathSeg::M(x0, uy), PathSeg::L(x1, uy)],
                fill: None,
                stroke: Some(stroke(w, dash.clone())),
            }),
        }
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
                text: f.text.to_string(),
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
        let uy = f.y - f32::from(info.underline_position) * scale;
        underline_commands(f, uy, thickness, color, commands);
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
        .map(|page| build_page_list(page, fonts))
        .collect();
    DisplayList { pages }
}

/// Display list of one laid-out page (see [`build_display_list`]).
pub fn build_page_list(page: &Page, fonts: &FontManager) -> PageList {
    let mut commands = Vec::new();
    // Page colour (ECMA-376 Part 1, §17.2.1 background) under everything.
    if let Some(c) = page.decor.background {
        commands.push(Command::FillRect {
            x: 0.0,
            y: 0.0,
            w: page.width,
            h: page.height,
            color: c.into(),
        });
    }
    // Page borders behind the text (§17.6.10 pgBorders zOrder="back").
    if page.decor.borders_behind {
        for item in &page.decor.borders {
            item_commands(item, &mut commands);
        }
    }
    for item in page.items.iter().filter(|i| i.layer() == Layer::Back) {
        item_commands(item, &mut commands);
    }
    let boxed = |l: &Line| matches!(l.source, LineSource::TextBox { .. });
    for line in page.lines.iter().filter(|l| !boxed(l)) {
        line_commands(line, fonts, &mut commands);
    }
    for item in page.items.iter().filter(|i| i.layer() == Layer::Front) {
        item_commands(item, &mut commands);
    }
    if !page.decor.borders_behind {
        for item in &page.decor.borders {
            item_commands(item, &mut commands);
        }
    }
    // Text box text goes over its shape (§17.17.1 txbxContent).
    for line in page.lines.iter().filter(|l| boxed(l)) {
        line_commands(line, fonts, &mut commands);
    }
    PageList {
        width: page.width,
        height: page.height,
        commands,
    }
}

/// Media part names referenced by a page list.
pub fn media_of(page: &PageList) -> Vec<&str> {
    let mut v: Vec<&str> = page
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Image { image, .. } => Some(image.as_str()),
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

    /// Page colour first, borders behind the text before it, watermarks wrapped in
    /// an opacity group (ECMA-376 Part 1, §17.2.1, §17.6.10).
    #[test]
    fn page_decor_paint_order() {
        let rule = PageItem::Rule {
            x1: 0.0,
            y1: 5.0,
            x2: 10.0,
            y2: 5.0,
            width: 1.0,
            color: bo_model::Color::BLACK,
            style: "single".into(),
        };
        let wm = PageItem::Image {
            x: 1.0,
            y: 1.0,
            w: 5.0,
            h: 5.0,
            media: "/word/media/w.png".into(),
            crop: None,
            rotation: 0.0,
            layer: Layer::Back,
            alpha: 0.3,
        };
        let mut page = Page {
            width: 100.0,
            height: 200.0,
            lines: Vec::new(),
            items: vec![wm],
            number: 1,
            section: 0,
            decor: bo_layout::PageDecor {
                background: bo_model::Color::from_hex("FDF6E3"),
                borders: vec![rule],
                borders_behind: true,
            },
        };
        let fm = FontManager::new();
        let c = build_page_list(&page, &fm).commands;
        assert!(matches!(
            c[0],
            Command::FillRect { x, y, w, h, color: Rgba(0xFD, 0xF6, 0xE3, 255) }
                if x == 0.0 && y == 0.0 && w == 100.0 && h == 200.0
        ));
        assert!(matches!(c[1], Command::Line { .. }));
        assert!(matches!(c[2], Command::PushOpacity { alpha } if (alpha - 0.3).abs() < 1e-6));
        assert!(matches!(c[3], Command::Image { .. }));
        assert!(matches!(c[4], Command::PopOpacity));
        // In front of the text: the borders come last.
        page.decor.borders_behind = false;
        let c = build_page_list(&page, &fm).commands;
        assert!(matches!(c.last(), Some(Command::Line { .. })));
    }

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
                    image: "/word/media/a.png".into(),
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
            r#"{"op":"image","x":0.0,"y":0.0,"w":10.0,"h":10.0,"image":"/word/media/a.png"}"#
        ));
        assert!(!j.contains("dash"));
        assert_eq!(media_of(&p), vec!["/word/media/a.png"]);
        assert!(j.contains(r#""op":"glyphs""#));
        assert!(j.contains(r#""glyphs":[[36,0.0,0.0,0]]"#));
        assert!(j.contains(r#"{"op":"popClip"}"#));
        let back: PageList = serde_json::from_str(&j).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn json_shape_vector_commands() {
        let p = PageList {
            width: 10.0,
            height: 20.0,
            commands: vec![
                Command::Image {
                    x: 1.0,
                    y: 2.0,
                    w: 3.0,
                    h: 4.0,
                    image: "/ppt/media/image1.png".into(),
                    crop: Some([0.1, 0.0, 0.0, 0.2]),
                    rotation: 0.0,
                },
                Command::PushTransform {
                    m: [1.0, 0.0, 0.0, 1.0, 5.0, 6.0],
                },
                Command::Path {
                    segs: vec![PathSeg::M(0.0, 0.0), PathSeg::L(1.0, 0.0), PathSeg::Z],
                    fill: Some(Paint::Solid {
                        color: Rgba(1, 2, 3, 255),
                    }),
                    stroke: Some(Stroke {
                        width: 1.0,
                        color: Rgba(0, 0, 0, 255),
                        dash: vec![],
                        cap: "butt".into(),
                        join: "miter".into(),
                    }),
                },
                Command::PopTransform,
            ],
        };
        let j = p.to_json();
        assert!(j.contains(r#""op":"image""#));
        assert!(j.contains(r#""image":"/ppt/media/image1.png""#));
        assert!(j.contains(r#""op":"pushTransform","m":[1.0,0.0,0.0,1.0,5.0,6.0]"#));
        assert!(j.contains(r#""segs":[{"M":[0.0,0.0]},{"L":[1.0,0.0]},"Z"]"#));
        assert!(j.contains(r#""fill":{"type":"solid","color":[1,2,3,255]}"#));
        assert!(j.contains(r#"{"op":"popTransform"}"#));
        let back: PageList = serde_json::from_str(&j).unwrap();
        assert_eq!(back, p);
    }
}

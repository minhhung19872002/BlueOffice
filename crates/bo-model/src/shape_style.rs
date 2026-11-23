//! Shape Format fills and effects of word-processing shapes (BlueDoc round 20,
//! docs/ui/REPORT.md "Vòng 110"): a gradient (ECMA-376 Part 1, §20.1.8.33 gradFill with
//! §20.1.8.41 lin / §20.1.8.46 path), a picture or texture (§20.1.8.14 blipFill with
//! §20.1.8.56 stretch or §20.1.8.58 tile), the transparency of the fill and of the line
//! (§20.1.2.3.1 alpha of the colour) and the Shape Effects (§20.1.8.26 effectLst, §20.1.5.9
//! scene3d, §20.1.5.12 sp3d — the same model as Picture Effects, [`PicEffects`]).
//!
//! [`crate::Shape::fill`] keeps a solid colour for readers of the model that only know solid
//! fills (the first stop of a gradient); the style, when set, is what is drawn and written.

use crate::picture_style::PicEffects;
use serde::{Deserialize, Serialize};

/// A gradient stop (§20.1.8.37 gs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapeStop {
    /// `pos` (1/1000 %, 0..=100 000).
    pub pos: i32,
    /// Colour `0xRRGGBB`.
    pub color: u32,
    /// Opacity (1/1000 %, 100 000 = opaque; §20.1.2.3.1 alpha).
    pub alpha: i32,
}

/// §20.1.8.33 gradFill.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapeGradient {
    /// `linear` (§20.1.8.41 lin), or the §20.1.10.63 ST_PathShadeType of §20.1.8.46 path:
    /// `circle`, `rect`, `shape`.
    pub kind: String,
    /// `lin@ang` (60 000ths of a degree, clockwise from +x).
    pub angle: i32,
    /// `path/fillToRect` l, t, r, b (1/1000 %): where a path gradient starts.
    pub focus: [i32; 4],
    /// Stops in increasing position.
    pub stops: Vec<ShapeStop>,
}

/// §20.1.8.14 blipFill of a shape: a picture stretched over the shape or tiled as a texture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapePicture {
    /// Media part (key into [`crate::Document::media`]).
    pub media: String,
    /// Tiled (§20.1.8.58 tile) rather than stretched.
    pub tile: bool,
    /// Tile scale `sx` / `sy` (1/1000 %, 100 000 = the picture's own size).
    pub scale: i32,
    /// Opacity (1/1000 %; §20.1.8.4 alphaModFix of the blip).
    pub alpha: i32,
}

/// Fill (other than a solid colour), transparency and effects of a shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapeStyle {
    /// Gradient fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<ShapeGradient>,
    /// Picture or texture fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<ShapePicture>,
    /// Opacity of the solid fill (1/1000 %); `None` = opaque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_alpha: Option<i32>,
    /// Opacity of the line (1/1000 %); `None` = opaque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_alpha: Option<i32>,
    /// Shape Effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<PicEffects>,
}

impl ShapeStyle {
    /// Nothing beyond a plain solid fill.
    pub fn is_empty(&self) -> bool {
        self.gradient.is_none()
            && self.picture.is_none()
            && self.fill_alpha.is_none_or(|a| a >= 100_000)
            && self.line_alpha.is_none_or(|a| a >= 100_000)
            && self.effects.as_ref().is_none_or(PicEffects::is_empty)
    }
}

/// Rotation angles (x tilt, y turn, z roll in degrees) and perspective strength of a
/// §20.1.10.47 camera preset — BlueOffice's own values for the families of names (the same
/// as the picture warp of `bo-raster`'s picture styles).
pub fn camera_angles(name: &str) -> (f64, f64, f64, f64) {
    let persp = if name.starts_with("perspective") {
        0.15
    } else {
        0.0
    };
    let n = name.to_ascii_lowercase();
    let (mut rx, mut ry, mut rz) = (0.0, 0.0, 0.0);
    if n.starts_with("isometric") {
        rx = 30.0;
        ry = if n.contains("left") { 40.0 } else { -40.0 };
        if n.contains("offaxis1") {
            rx = 15.0;
        } else if n.contains("offaxis2") {
            rx = -15.0;
        } else if n.contains("topup") || n.contains("topdown") {
            rx = 45.0;
            ry = 0.0;
        } else if n.contains("bottom") {
            rx = -45.0;
            ry = 0.0;
        }
        if n.contains("down") && !n.contains("top") && !n.contains("bottom") {
            rz = if ry > 0.0 { -8.0 } else { 8.0 };
        }
    } else if n.starts_with("oblique") {
        rx = if n.contains("top") {
            25.0
        } else if n.contains("bottom") {
            -25.0
        } else {
            0.0
        };
        ry = if n.contains("left") {
            25.0
        } else if n.contains("right") {
            -25.0
        } else {
            0.0
        };
    } else {
        if n.contains("left") {
            ry = 20.0;
        }
        if n.contains("right") {
            ry = -20.0;
        }
        if n.contains("above") {
            rx = 25.0;
        }
        if n.contains("below") {
            rx = -25.0;
        }
        if n.contains("relaxed") {
            rx = if n.contains("moderately") { 30.0 } else { 50.0 };
        }
        if n.contains("contrasting") {
            rx = 10.0;
            ry *= 1.5;
        }
        if n.contains("heroic") {
            rx = -15.0;
            ry *= if n.contains("extreme") { 1.8 } else { 1.3 };
        }
    }
    (rx, ry, rz, persp)
}

/// A point projection of a `w` × `h` box turned by camera preset `name` and fitted back into
/// the same box (the plane turned, seen in perspective, scaled to fit, centred).
pub fn camera_projection(name: &str, w: f64, h: f64) -> impl Fn(f64, f64) -> (f64, f64) {
    let (rx, ry, rz, persp) = camera_angles(name);
    let (sx, cx) = rx.to_radians().sin_cos();
    let (sy, cy) = ry.to_radians().sin_cos();
    let (sz, cz) = rz.to_radians().sin_cos();
    let (hw, hh) = (w / 2.0, h / 2.0);
    let dist = w.max(h).max(1e-6) * if persp > 0.0 { 1.0 / persp } else { 1e9 };
    let project = move |x: f64, y: f64| {
        let (x, y) = (x - hw, y - hh);
        let (x, y) = (x * cz - y * sz, x * sz + y * cz);
        let (x, z) = (x * cy, -x * sy);
        let (y, z) = (y * cx - z * sx, y * sx + z * cx);
        let k = dist / (dist + z);
        (x * k, y * k)
    };
    let pts = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| project(x, y));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let k = (w / (x1 - x0).max(1e-6)).min(h / (y1 - y0).max(1e-6));
    let (ox, oy) = ((w - (x1 - x0) * k) / 2.0, (h - (y1 - y0) * k) / 2.0);
    move |x: f64, y: f64| {
        let (px, py) = project(x, y);
        ((px - x0) * k + ox, (py - y0) * k + oy)
    }
}

/// Opacity in 0..=1 of a 1/1000 % value.
pub fn opacity(alpha: i32) -> f32 {
    (alpha.clamp(0, 100_000) as f32) / 100_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_style_and_opacity() {
        assert!(ShapeStyle::default().is_empty());
        let s = ShapeStyle {
            fill_alpha: Some(50_000),
            ..ShapeStyle::default()
        };
        assert!(!s.is_empty());
        assert!((opacity(50_000) - 0.5).abs() < 1e-6);
        assert!((opacity(200_000) - 1.0).abs() < 1e-6);
    }
}

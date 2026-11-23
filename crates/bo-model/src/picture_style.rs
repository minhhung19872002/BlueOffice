//! Picture styles of word-processing pictures (BlueDoc round 19, docs/ui/REPORT.md
//! "Vòng 109"): Picture Border (ECMA-376 Part 1, §20.1.2.2.24 `ln` of `pic:spPr`),
//! Picture Effects (§20.1.8.26 `effectLst`: §20.1.8.49 outerShdw, §20.1.8.40 innerShdw,
//! §20.1.8.32 glow, §20.1.8.50 reflection, §20.1.8.53 softEdge; §20.1.5.9 scene3d camera
//! and §20.1.5.12 sp3d bevelT) and Artistic Effects ([MS-ODRAWXML] 2.3.1 `a14:imgEffect`
//! artistic effects).
//!
//! Everything is kept in OOXML units (EMU, 60 000ths of a degree, 1/1000 of a percent) so
//! the model stays `Eq + Hash`. The renderers draw the effects through *derived image ids*
//! (see [`crate::picture::FX_MARK`]): tokens appended to the picture's key name pixel
//! operations BlueOffice's raster code applies (its own approximations), so every backend
//! that resolves derived ids draws them without new display-list commands.

use serde::{Deserialize, Serialize};

/// Picture Border: §20.1.2.2.24 `ln` with a solid colour.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicLine {
    /// Colour `0xRRGGBB`.
    pub color: u32,
    /// Width (`ln@w`, EMU).
    pub width: i64,
    /// §20.1.10.48 ST_PresetLineDashVal other than `solid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
}

/// §20.1.8.49 outerShdw / §20.1.8.40 innerShdw.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicShadow {
    /// Inner shadow (innerShdw) rather than outer.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inner: bool,
    /// `blurRad` (EMU).
    pub blur: i64,
    /// `dist` (EMU).
    pub dist: i64,
    /// `dir` (60 000ths of a degree, clockwise from +x).
    pub dir: i32,
    /// Colour `0xRRGGBB`.
    pub color: u32,
    /// Opacity (1/1000 %).
    pub alpha: i32,
    /// `sx` / `sy` scale of an outer shadow (1/1000 %; 100 000 = same size).
    pub sx: i32,
    /// See `sx`.
    pub sy: i32,
}

/// §20.1.8.32 glow.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicGlow {
    /// `rad` (EMU).
    pub rad: i64,
    /// Colour `0xRRGGBB`.
    pub color: u32,
    /// Opacity (1/1000 %).
    pub alpha: i32,
}

/// §20.1.8.50 reflection (mirrored below the picture, as the presets of the reference).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicReflection {
    /// `blurRad` (EMU).
    pub blur: i64,
    /// `stA` (1/1000 %).
    pub st_a: i32,
    /// `endA` (1/1000 %).
    pub end_a: i32,
    /// `endPos` (1/1000 %).
    pub end_pos: i32,
    /// `dist` (EMU).
    pub dist: i64,
}

/// §20.1.5.12 sp3d `bevelT` (§20.1.5.1 bevel).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicBevel {
    /// §20.1.10.9 ST_BevelPresetType (`circle`, `relaxedInset`, `cross`, …).
    pub prst: String,
    /// `w` (EMU).
    pub w: i64,
    /// `h` (EMU).
    pub h: i64,
}

/// Picture Effects of a picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicEffects {
    /// Shadow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<PicShadow>,
    /// Glow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<PicGlow>,
    /// Reflection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<PicReflection>,
    /// §20.1.8.53 softEdge `rad` (EMU).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_edge: Option<i64>,
    /// Bevel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bevel: Option<PicBevel>,
    /// 3-D Rotation: §20.1.5.5 camera `prst` (§20.1.10.47 ST_PresetCameraType) other than
    /// `orthographicFront`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
}

impl PicEffects {
    /// No effect at all.
    pub fn is_empty(&self) -> bool {
        self.shadow.is_none()
            && self.glow.is_none()
            && self.reflection.is_none()
            && self.soft_edge.is_none_or(|r| r <= 0)
            && self.bevel.is_none()
            && self.camera.is_none()
    }
}

/// Artistic effect ([MS-ODRAWXML] `a14:imgEffect` child, e.g. `artisticBlur`): the element
/// name without prefix and its attributes, kept as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PicArtistic {
    /// Element local name (`artisticBlur`, `artisticPencilGrayscale`, …).
    pub name: String,
    /// Attributes (name, value).
    pub attrs: Vec<(String, String)>,
}

/// The artistic effects of [MS-ODRAWXML] 2.3.1 with their main amount attribute and
/// the reference's default value of it.
pub const ARTISTIC: [(&str, &str, i32); 22] = [
    ("artisticMarker", "size", 23),
    ("artisticPencilGrayscale", "pencilSize", 50),
    ("artisticPencilSketch", "pressure", 7),
    ("artisticLineDrawing", "pencilSize", 50),
    ("artisticChalkSketch", "pressure", 2),
    ("artisticPaintBrush", "brushSize", 2),
    ("artisticGlowDiffused", "intensity", 2),
    ("artisticBlur", "radius", 10),
    ("artisticLightScreen", "gridSize", 2),
    ("artisticWatercolorSponge", "brushSize", 2),
    ("artisticFilmGrain", "grainSize", 25),
    ("artisticMosaicBubbles", "pressure", 50),
    ("artisticGlass", "scaling", 5),
    ("artisticCement", "crackSpacing", 55),
    ("artisticTexturizer", "scaling", 100),
    ("artisticCrisscrossEtching", "pressure", 4),
    ("artisticPastelsSmooth", "scaling", 50),
    ("artisticPlasticWrap", "smoothness", 5),
    ("artisticCutout", "numberOfShades", 4),
    ("artisticPhotocopy", "detail", 3),
    ("artisticGlowEdges", "smoothness", 10),
    ("artisticPaintStrokes", "intensity", 7),
];

impl PicArtistic {
    /// The effect with its default amount.
    pub fn preset(name: &str) -> Option<PicArtistic> {
        let (n, a, v) = ARTISTIC.iter().find(|(n, _, _)| *n == name)?;
        Some(PicArtistic {
            name: (*n).to_owned(),
            attrs: vec![((*a).to_owned(), v.to_string())],
        })
    }

    /// The main amount (the first numeric attribute), or the default.
    pub fn amount(&self) -> i32 {
        self.attrs
            .iter()
            .find_map(|(_, v)| v.trim().parse::<i32>().ok())
            .or_else(|| {
                ARTISTIC
                    .iter()
                    .find(|(n, _, _)| *n == self.name)
                    .map(|a| a.2)
            })
            .unwrap_or(0)
    }
}

/// Scale of the fractions in derived-id tokens (1/1000 of a percent).
pub const FRAC: f64 = 100_000.0;

/// A pixel operation named by a derived-id token (see the module docs).
#[derive(Debug, Clone, PartialEq)]
pub enum StyleOp {
    /// `crop=l,t,r,b`: cut the picture to the visible part first (1/1000 %).
    Crop([i32; 4]),
    /// `art=name@amount`: artistic effect.
    Artistic(String, i32),
    /// `soft=rx,ry`: soft edges, radius as fractions of the width and height.
    Soft(f64, f64),
    /// `bevel=wx,wy`: lit edge, width as fractions of the width and height.
    Bevel(f64, f64),
    /// `cam=preset`: 3-D rotation (perspective warp).
    Camera(String),
    /// `ish=RRGGBB,alpha,bx,by,dx,dy`: inner shadow (alpha 0..=1; fractions).
    InnerShadow(u32, f64, f64, f64, f64, f64),
    /// `shadow=RRGGBB,alpha,bx,by,px,py`: silhouette of the picture in a colour, blurred,
    /// on a canvas padded by `px`/`py` of the size on each side (outer shadow).
    Silhouette(u32, f64, f64, f64, f64, f64),
    /// `glow=RRGGBB,alpha,rx,ry`: silhouette grown and blurred by the radius, padded by it.
    Glow(u32, f64, f64, f64),
    /// `refl=stA,endA,endPos,bx,by`: the picture upside down, fading from `stA` at its top
    /// to `endA` at `endPos` (fractions), blurred.
    Reflection(f64, f64, f64, f64, f64),
}

fn nums(v: &str) -> Vec<f64> {
    v.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn hex(v: &str) -> Option<u32> {
    u32::from_str_radix(v.trim(), 16).ok()
}

/// The style operations of a derived image id, in the order they apply.
pub fn style_ops(id: &str) -> Vec<StyleOp> {
    let Some((_, key)) = id.split_once(crate::picture::FX_MARK) else {
        return Vec::new();
    };
    let f = |v: i32| f64::from(v) / FRAC;
    let mut out = Vec::new();
    for item in key.split(';') {
        let Some((k, v)) = item.split_once('=') else {
            continue;
        };
        let n = nums(v);
        let op = match k {
            "crop" => {
                let c: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                match c[..] {
                    [l, t, r, b] => Some(StyleOp::Crop([l, t, r, b])),
                    _ => None,
                }
            }
            "art" => {
                let (name, amt) = v.split_once('@').unwrap_or((v, "0"));
                Some(StyleOp::Artistic(name.to_owned(), amt.parse().unwrap_or(0)))
            }
            "soft" if n.len() == 2 => Some(StyleOp::Soft(n[0] / FRAC, n[1] / FRAC)),
            "bevel" if n.len() == 2 => Some(StyleOp::Bevel(n[0] / FRAC, n[1] / FRAC)),
            "cam" => Some(StyleOp::Camera(v.to_owned())),
            "ish" | "shadow" | "glow" | "refl" => {
                let parts: Vec<&str> = v.split(',').collect();
                let rest: Vec<f64> = parts
                    .iter()
                    .skip(usize::from(k != "refl"))
                    .filter_map(|x| x.trim().parse::<i32>().ok())
                    .map(f)
                    .collect();
                let color = parts.first().and_then(|c| hex(c)).unwrap_or(0);
                match (k, rest.as_slice()) {
                    ("ish", &[a, bx, by, dx, dy]) => {
                        Some(StyleOp::InnerShadow(color, a, bx, by, dx, dy))
                    }
                    ("shadow", &[a, bx, by, px, py]) => {
                        Some(StyleOp::Silhouette(color, a, bx, by, px, py))
                    }
                    ("glow", &[a, rx, ry]) => Some(StyleOp::Glow(color, a, rx, ry)),
                    ("refl", &[st, end, pos, bx, by]) => {
                        Some(StyleOp::Reflection(st, end, pos, bx, by))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        out.extend(op);
    }
    out
}

/// The picture's display id (`Image::display_id`) of a drawn image id: the in-place style
/// tokens removed. An effect image (shadow, glow, reflection) keeps its tokens, so it never
/// stands for the picture (a click on a shadow does not select it).
pub fn strip_style(id: &str) -> std::borrow::Cow<'_, str> {
    let Some((base, key)) = id.split_once(crate::picture::FX_MARK) else {
        return std::borrow::Cow::Borrowed(id);
    };
    let own = |t: &str| {
        ["crop=", "art=", "soft=", "bevel=", "cam=", "ish="]
            .iter()
            .any(|p| t.starts_with(p))
    };
    if !key.split(';').any(own) {
        return std::borrow::Cow::Borrowed(id);
    }
    let rest: Vec<&str> = key.split(';').filter(|t| !own(t)).collect();
    std::borrow::Cow::Owned(if rest.is_empty() {
        base.to_owned()
    } else {
        format!("{base}{}{}", crate::picture::FX_MARK, rest.join(";"))
    })
}

/// Token text of a fraction (1/1000 %).
#[allow(clippy::cast_possible_truncation)]
pub fn frac(v: f64) -> i32 {
    (v * FRAC).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_parse_back() {
        let id = "/word/media/a.png#fx:b=1;crop=1,2,3,4;art=artisticBlur@10;soft=5000,2500;\
                  bevel=1000,2000;cam=perspectiveRelaxed;ish=FF0000,50000,1000,1000,500,500;\
                  shadow=000000,40000,2000,3000,4000,5000;glow=00FF00,60000,7000,8000;\
                  refl=50000,0,90000,100,200";
        let ops = style_ops(id);
        assert_eq!(ops.len(), 9, "{ops:?}");
        assert_eq!(ops[0], StyleOp::Crop([1, 2, 3, 4]));
        assert_eq!(ops[1], StyleOp::Artistic("artisticBlur".into(), 10));
        assert_eq!(ops[2], StyleOp::Soft(0.05, 0.025));
        assert_eq!(ops[4], StyleOp::Camera("perspectiveRelaxed".into()));
        assert_eq!(ops[6], StyleOp::Silhouette(0, 0.4, 0.02, 0.03, 0.04, 0.05));
        assert_eq!(ops[7], StyleOp::Glow(0x00FF00, 0.6, 0.07, 0.08));
        assert_eq!(ops[8], StyleOp::Reflection(0.5, 0.0, 0.9, 0.001, 0.002));
        assert!(style_ops("/word/media/a.png").is_empty());
    }

    #[test]
    fn artistic_presets_have_amounts() {
        let a = PicArtistic::preset("artisticPencilGrayscale").expect("preset");
        assert_eq!(a.amount(), 50);
        assert!(PicArtistic::preset("nothing").is_none());
        assert!(PicEffects::default().is_empty());
    }
}

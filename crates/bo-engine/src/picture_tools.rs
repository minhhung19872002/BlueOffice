//! Picture tools shared by BlueDoc and BlueSlide (CORE-PIC-001): the state of the
//! selected picture, gallery previews drawn from the picture itself, compression
//! settings and reports, and the picture of an SVG (PNG fallback drawn from its
//! converted shapes).

use base64::Engine as _;
use bo_drawingml::svg::{PathSeg as SvgSeg, SvgDoc};
use bo_model::picture::{visible_size, PictureFx};
use bo_raster::picfx::{self, CompressOptions};
use bo_render::{Command, PageList, Paint, PathSeg, Rgba, Stroke};
use serde::{Deserialize, Serialize};

/// EMU per SVG user unit (CSS pixel = 1/96 in).
pub const EMU_PER_PX: f64 = 9525.0;

/// State of the selected picture for the picture tools.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PictureInfo {
    /// Colour adjustments.
    pub fx: PictureFx,
    /// §20.1.8.55 srcRect (l, t, r, b in 1/1000 %).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<[i32; 4]>,
    /// Preset shape the picture is cropped to (§20.1.9.18 prstGeom), `rect` by default.
    pub shape: String,
    /// Media part of the picture.
    pub media: String,
    /// Picture pixels.
    pub pixel_width: u32,
    /// Picture pixels.
    pub pixel_height: u32,
    /// Size of the encoded picture in bytes.
    pub bytes: usize,
    /// Displayed width (points).
    pub width: f64,
    /// Displayed height (points).
    pub height: f64,
    /// Resolution of the picture as shown (pixels per inch).
    pub ppi: f64,
    /// The picture has an SVG version that can become shapes.
    pub svg: bool,
    /// Gallery previews (PNG base64), in the order asked for.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub previews: Vec<String>,
    /// Result of the last "Compress pictures".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed: Option<CompressReport>,
}

/// What "Compress pictures" did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompressReport {
    /// Pictures re-encoded.
    pub pictures: usize,
    /// Bytes of those pictures before.
    pub before: usize,
    /// Bytes after.
    pub after: usize,
}

impl CompressReport {
    /// Bytes saved.
    pub fn saved(&self) -> usize {
        self.before.saturating_sub(self.after)
    }
}

/// "Compress pictures" settings sent by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CompressSpec {
    /// Target pixels per inch (`None` keeps the resolution).
    pub ppi: Option<f64>,
    /// Remove cropped areas.
    pub drop_cropped: bool,
    /// JPEG quality (1–100) for opaque pictures.
    pub jpeg_quality: Option<u8>,
    /// Every picture of the document (else the selected ones).
    pub all: bool,
}

impl CompressSpec {
    /// Encoder options.
    pub fn options(&self) -> CompressOptions {
        CompressOptions {
            ppi: self.ppi,
            drop_cropped: self.drop_cropped,
            jpeg_quality: self.jpeg_quality.map(|q| q.clamp(1, 100)),
        }
    }
}

/// A picture tools command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PictureCommand {
    /// State of the selected picture.
    PictureInfo,
    /// Previews of the selected picture with each adjustment set applied.
    PicturePreviews {
        /// Adjustment sets (gallery entries).
        presets: Vec<PictureFx>,
        /// Longest preview side in pixels.
        #[serde(default)]
        size: Option<u32>,
    },
    /// Set the colour adjustments of the selected pictures.
    PictureFx {
        /// New adjustments (replace the modelled ones).
        fx: PictureFx,
    },
    /// Set the crop rectangle (§20.1.8.55 srcRect); the frame follows so the
    /// picture keeps its scale.
    PictureCrop {
        /// l, t, r, b in 1/1000 %.
        crop: [i32; 4],
    },
    /// Crop to an aspect ratio `w:h`, centred.
    PictureCropAspect {
        /// Ratio such as `1:1`, `4:3`, `16:9`.
        ratio: String,
    },
    /// Crop to a preset shape (§20.1.9.18 prstGeom).
    PictureCropShape {
        /// Preset name (`ellipse`, `roundRect`, …).
        preset: String,
    },
    /// Fill the frame with the picture (crop the overflow).
    PictureFill,
    /// Fit the whole picture inside the frame.
    PictureFit,
    /// Compress pictures.
    PictureCompress(CompressSpec),
    /// Replace the picture, keeping size, position and adjustments.
    PictureChange {
        /// New picture, base64.
        data: String,
        /// File name.
        #[serde(default)]
        name: String,
    },
    /// Remove adjustments and crop (and restore the picture's own size).
    PictureReset {
        /// Also restore the size.
        #[serde(default)]
        size: bool,
    },
    /// Convert the selected SVG picture into shapes.
    PictureToShapes,
    /// Insert an SVG picture (with a PNG fallback).
    PictureInsertSvg {
        /// SVG document, base64.
        data: String,
        /// File name.
        #[serde(default)]
        name: String,
    },
}

impl PictureCommand {
    /// Parses a JSON command when it is a picture tools command (`cmd` starts with
    /// `picture`).
    pub fn parse(v: &serde_json::Value) -> Option<PictureCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("picture") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// Largest preview side in pixels.
pub const MAX_PREVIEW: u32 = 256;

/// PNG (base64) previews of `bytes` with each of `presets` applied.
pub fn previews(bytes: &[u8], presets: &[PictureFx], size: u32) -> Vec<String> {
    let size = size.clamp(16, MAX_PREVIEW);
    presets
        .iter()
        .take(64)
        .map(|fx| {
            picfx::preview_png(bytes, fx, size)
                .map(|p| base64::engine::general_purpose::STANDARD.encode(p))
                .unwrap_or_default()
        })
        .collect()
}

/// Picture info from the picture bytes and its displayed size in points.
pub fn info(
    media: &str,
    bytes: &[u8],
    fx: PictureFx,
    crop: Option<[i32; 4]>,
    shape: Option<&str>,
    (width, height): (f64, f64),
) -> PictureInfo {
    let (pw, ph) = crate::edit::image_size(bytes).unwrap_or((0, 0));
    // Pixels shown across the displayed width.
    let (vw, _) = visible_size(f64::from(pw), f64::from(ph), crop);
    let ppi = if width > 0.0 {
        vw / (width / 72.0)
    } else {
        0.0
    };
    PictureInfo {
        fx,
        crop,
        shape: shape.unwrap_or("rect").to_owned(),
        media: media.to_owned(),
        pixel_width: pw,
        pixel_height: ph,
        bytes: bytes.len(),
        width,
        height,
        ppi: (ppi * 10.0).round() / 10.0,
        ..PictureInfo::default()
    }
}

/// Decode base64 picture data from a command.
pub fn decode_data(data: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()
}

/// Bytes look like an SVG document.
pub fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let s = String::from_utf8_lossy(head);
    s.contains("<svg")
}

/// A picture the backends draw (PNG / JPEG); other raster formats become PNG.
pub fn drawable(bytes: Vec<u8>) -> Option<(Vec<u8>, &'static str, &'static str)> {
    match bo_render::image::sniff(&bytes) {
        bo_render::image::Format::Png => Some((bytes, "png", "image/png")),
        bo_render::image::Format::Jpeg => Some((bytes, "jpeg", "image/jpeg")),
        bo_render::image::Format::Other => {
            bo_raster::transcode_to_png(&bytes).map(|p| (p, "png", "image/png"))
        }
    }
}

fn segs(s: &[SvgSeg], k: f32) -> Vec<PathSeg> {
    s.iter()
        .map(|g| match *g {
            SvgSeg::M(x, y) => PathSeg::M(x as f32 * k, y as f32 * k),
            SvgSeg::L(x, y) => PathSeg::L(x as f32 * k, y as f32 * k),
            SvgSeg::C(a, b, c, d, x, y) => PathSeg::C(
                a as f32 * k,
                b as f32 * k,
                c as f32 * k,
                d as f32 * k,
                x as f32 * k,
                y as f32 * k,
            ),
            SvgSeg::Z => PathSeg::Z,
        })
        .collect()
}

fn rgba(c: u32, opacity: f64) -> Rgba {
    Rgba(
        (c >> 16) as u8,
        (c >> 8) as u8,
        c as u8,
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// PNG of an SVG picture drawn from its converted shapes (the fallback picture
/// stored next to the SVG), at most `max_side` pixels on the longer side.
pub fn svg_png(doc: &SvgDoc, max_side: f32) -> Option<Vec<u8>> {
    let (w, h) = (doc.width as f32, doc.height as f32);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let scale = (max_side / w.max(h)).min(4.0);
    let commands = doc
        .shapes
        .iter()
        .map(|s| Command::Path {
            segs: segs(&s.segs, 1.0),
            fill: s.fill.map(|c| Paint::Solid {
                color: rgba(c, s.opacity),
            }),
            stroke: s.stroke.map(|(c, width)| Stroke {
                width: width as f32,
                color: rgba(c, s.opacity),
                dash: Vec::new(),
                cap: "butt".into(),
                join: "miter".into(),
            }),
        })
        .collect();
    let page = PageList {
        width: w,
        height: h,
        commands,
    };
    let fonts = bo_text::FontManager::new();
    let pm = bo_raster::render_layer(&page, &fonts, scale, &bo_raster::Images::new()).ok()?;
    pm.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_fallback_is_drawn() {
        let doc = bo_drawingml::svg::parse_svg(
            br#"<svg width="20" height="10"><rect width="10" height="10" fill="red"/></svg>"#,
        )
        .unwrap();
        let png = svg_png(&doc, 40.0).unwrap();
        let img = bo_raster::picfx::decode_rgba(&png).unwrap();
        // Scale 2: 40×20 px; red on the left half, transparent on the right.
        assert_eq!((img.width, img.height), (40, 20));
        assert_eq!(&img.pixels[..4], &[255, 0, 0, 255]);
        assert_eq!(img.pixels[(10 * 40 + 30) * 4 + 3], 0);
    }

    #[test]
    fn info_reports_resolution() {
        let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
            width: 300,
            height: 200,
            pixels: vec![255; 300 * 200 * 4],
        })
        .unwrap();
        // 300 px shown 2 inches wide (144 pt) = 150 ppi; cropping half the width = 75 ppi.
        let i = info(
            "/m.png",
            &png,
            PictureFx::default(),
            None,
            None,
            (144.0, 96.0),
        );
        assert_eq!((i.pixel_width, i.pixel_height, i.ppi), (300, 200, 150.0));
        let i = info(
            "/m.png",
            &png,
            PictureFx::default(),
            Some([25_000, 0, 25_000, 0]),
            None,
            (144.0, 96.0),
        );
        assert_eq!(i.ppi, 75.0);
        assert_eq!(i.shape, "rect");
    }
}

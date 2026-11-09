//! Pictures in cells and checkbox cell controls: geometry and drawing.
//!
//! * Pictures ([MS-XLSX] §2.3.6.1.10 local image, §2.3.6.1.11 web image from
//!   `IMAGE`) are drawn inside their cell according to the sizing mode
//!   (`ImageSizing` 0 fit keeping the aspect ratio, 1 fill, 2 original size,
//!   3 custom height / width in pixels). The pixel size of each picture comes
//!   from an [`ImageStates`] map the host fills: stored pictures from their
//!   header ([`pixel_size`]), web pictures once the host has fetched them.
//! * A checkbox ([MS-XLSX] §2.3.9.4) is drawn for boolean and empty cells whose
//!   format carries the control; TRUE is checked.
//!
//! Layout choices (BlueOffice's own): pictures sit one pixel inside the cell
//! edges; fit centres the picture, original and custom sizes start at the top-left
//! corner. The checkbox is a square of `min(cell height − 4 pt, 12 pt)` centred
//! in the cell.

use crate::geometry::{Rect, PT_PER_PX};
use bo_formula::Value;
use bo_model::sheet::{CellImage, CellImageSource, ImageSizing};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};
use std::collections::BTreeMap;

/// What the host knows about the pixels of a picture.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageState {
    /// Decodable picture of this pixel size.
    Ready {
        /// Width in pixels.
        width: f64,
        /// Height in pixels.
        height: f64,
    },
    /// Being fetched (web pictures): nothing is drawn yet.
    Pending,
    /// Could not be fetched or decoded: the cell shows this error literal
    /// (`#CONNECT!`, `#BLOCKED!`, `#VALUE!`).
    Failed(String),
}

/// Picture states by media id ([`CellImage::media_id`]).
pub type ImageStates = BTreeMap<String, ImageState>;

/// The picture of an `IMAGE` result, for the cell model.
pub fn formula_image(v: &Value) -> Option<Box<CellImage>> {
    let Value::Image(img) = v else {
        return None;
    };
    let sizing = ImageSizing::from_code(u32::from(img.sizing)).unwrap_or_default();
    Some(Box::new(CellImage {
        source: CellImageSource::Web {
            url: img.source.clone(),
            sizing,
            height: img.height,
            width: img.width,
        },
        alt_text: img.alt_text.clone(),
        // [MS-XLSX] §2.3.6.1.11 CalcOrigin: 1 = created by a formula.
        calc_origin: 1,
    }))
}

/// Pixel size of a PNG (ISO/IEC 15948 §11.2.2 IHDR), JPEG (ITU T.81 §B.2.2 SOFn),
/// GIF (GIF89a logical screen descriptor) or BMP (`BITMAPINFOHEADER`) picture.
pub fn pixel_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be = |r: std::ops::Range<usize>| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(r)?.try_into().ok()?))
    };
    let (w, h) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (be(16..20)?, be(20..24)?)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let (w, h, _) = bo_render::image::jpeg_info(bytes)?;
        (w, h)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let w = u16::from_le_bytes(bytes.get(6..8)?.try_into().ok()?);
        let h = u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?);
        (u32::from(w), u32::from(h))
    } else if bytes.starts_with(b"BM") {
        let w = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
        let h = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
        (w.unsigned_abs(), h.unsigned_abs())
    } else {
        return None;
    };
    (w > 0 && h > 0).then_some((w, h))
}

/// Box (points) of a picture of `natural` pixels in `cell`, for `sizing` with the
/// custom `height` / `width` in pixels (sizing 3; one missing follows the aspect
/// ratio).
pub fn placement(
    cell: Rect,
    natural: (f64, f64),
    sizing: ImageSizing,
    height: Option<f64>,
    width: Option<f64>,
) -> Option<Rect> {
    let (nw, nh) = natural;
    if nw <= 0.0 || nh <= 0.0 {
        return None;
    }
    let pad = PT_PER_PX;
    let inner = Rect {
        x: cell.x + pad,
        y: cell.y + pad,
        w: (cell.w - 2.0 * pad).max(0.0),
        h: (cell.h - 2.0 * pad).max(0.0),
    };
    let r = match sizing {
        ImageSizing::Fit => {
            let s = (inner.w / nw).min(inner.h / nh);
            let (w, h) = (nw * s, nh * s);
            Rect {
                x: inner.x + (inner.w - w) / 2.0,
                y: inner.y + (inner.h - h) / 2.0,
                w,
                h,
            }
        }
        ImageSizing::Fill => inner,
        ImageSizing::Original => Rect {
            x: inner.x,
            y: inner.y,
            w: nw * PT_PER_PX,
            h: nh * PT_PER_PX,
        },
        ImageSizing::Custom => {
            let (w, h) = match (width, height) {
                (Some(w), Some(h)) => (w, h),
                (Some(w), None) => (w, w * nh / nw),
                (None, Some(h)) => (h * nw / nh, h),
                (None, None) => (nw, nh),
            };
            Rect {
                x: inner.x,
                y: inner.y,
                w: w * PT_PER_PX,
                h: h * PT_PER_PX,
            }
        }
    };
    (r.w > 0.0 && r.h > 0.0).then_some(r)
}

/// Drawing of a picture in `cell`: an `Image` command clipped to the cell (or,
/// for original and custom sizes, left to overflow), or the error text of a failed
/// picture is returned as `Err` for the caller to draw as cell text.
pub fn draw(img: &CellImage, cell: Rect, states: &ImageStates) -> Result<Vec<Command>, String> {
    let id = img.media_id();
    let natural = match states.get(&id) {
        Some(ImageState::Ready { width, height }) => (*width, *height),
        Some(ImageState::Failed(e)) => return Err(e.clone()),
        Some(ImageState::Pending) => return Ok(Vec::new()),
        // Stored pictures can be measured here; unknown web pictures wait for the host.
        None => match img.data().and_then(pixel_size) {
            Some((w, h)) => (f64::from(w), f64::from(h)),
            None if img.data().is_some() => return Err("#VALUE!".into()),
            None => return Ok(Vec::new()),
        },
    };
    let (sizing, h, w) = match &img.source {
        CellImageSource::Web {
            sizing,
            height,
            width,
            ..
        } => (*sizing, *height, *width),
        CellImageSource::Local { .. } => (ImageSizing::Fit, None, None),
    };
    let Some(r) = placement(cell, natural, sizing, h, w) else {
        return Ok(Vec::new());
    };
    let image = Command::Image {
        x: r.x as f32,
        y: r.y as f32,
        w: r.w as f32,
        h: r.h as f32,
        image: id,
        crop: None,
        rotation: 0.0,
    };
    Ok(match sizing {
        ImageSizing::Fit | ImageSizing::Fill => vec![
            Command::PushClip {
                x: cell.x as f32,
                y: cell.y as f32,
                w: cell.w as f32,
                h: cell.h as f32,
            },
            image,
            Command::PopClip,
        ],
        ImageSizing::Original | ImageSizing::Custom => vec![image],
    })
}

/// Box of the checkbox drawn in `cell` (points).
pub fn checkbox_rect(cell: Rect) -> Rect {
    let s = (cell.h - 4.0).clamp(4.0, 12.0).min(cell.w.max(0.0));
    Rect {
        x: cell.x + (cell.w - s) / 2.0,
        y: cell.y + (cell.h - s) / 2.0,
        w: s,
        h: s,
    }
}

/// Is the point (`x`, `y`) on the checkbox of `cell` (2 pt tolerance)?
pub fn checkbox_hit(cell: Rect, x: f64, y: f64) -> bool {
    let b = checkbox_rect(cell);
    let t = 2.0;
    x >= b.x - t && x <= b.x + b.w + t && y >= b.y - t && y <= b.y + b.h + t
}

/// Checkbox outline colour and checked fill (BlueOffice palette).
const BOX_LINE: Rgba = Rgba(0x5A, 0x60, 0x6B, 255);
const BOX_ON: Rgba = Rgba(0x1E, 0x5A, 0xA8, 255);
const TICK: Rgba = Rgba(255, 255, 255, 255);

/// Drawing commands of a checkbox in `cell`.
pub fn checkbox(cell: Rect, checked: bool) -> Vec<Command> {
    let b = checkbox_rect(cell);
    let (x, y, w, h) = (b.x as f32, b.y as f32, b.w as f32, b.h as f32);
    let r = (w * 0.15).min(2.0);
    // Rounded square (quadratic corners).
    let square = vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::Q(x + w, y, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::Q(x + w, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::Q(x, y + h, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::Q(x, y, x + r, y),
        PathSeg::Z,
    ];
    let stroke = |color: Rgba, width: f32| Stroke {
        width,
        color,
        dash: Vec::new(),
        cap: "round".into(),
        join: "round".into(),
    };
    if !checked {
        return vec![Command::Path {
            segs: square,
            fill: Some(Paint::Solid {
                color: Rgba(255, 255, 255, 255),
            }),
            stroke: Some(stroke(BOX_LINE, 1.0)),
        }];
    }
    vec![
        Command::Path {
            segs: square,
            fill: Some(Paint::Solid { color: BOX_ON }),
            stroke: Some(stroke(BOX_ON, 1.0)),
        },
        Command::Path {
            segs: vec![
                PathSeg::M(x + w * 0.22, y + h * 0.52),
                PathSeg::L(x + w * 0.42, y + h * 0.72),
                PathSeg::L(x + w * 0.78, y + h * 0.3),
            ],
            fill: None,
            stroke: Some(stroke(TICK, (w * 0.14).max(1.0))),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: Rect = Rect {
        x: 100.0,
        y: 50.0,
        w: 60.0,
        h: 20.0,
    };

    fn near(a: Rect, b: (f64, f64, f64, f64)) {
        let d = (a.x - b.0).abs() + (a.y - b.1).abs() + (a.w - b.2).abs() + (a.h - b.3).abs();
        assert!(d < 1e-9, "{a:?} != {b:?}");
    }

    // The inner box of CELL is 0.75 pt inside each edge: x 100.75, y 50.75, 58.5 × 18.5.

    #[test]
    fn fit_keeps_the_ratio_and_centres() {
        // 200×100 px: scale = min(58.5/200, 18.5/100) = 0.185 pt/px → 37 × 18.5 pt,
        // x = 100.75 + (58.5 − 37)/2 = 111.5.
        let r = placement(CELL, (200.0, 100.0), ImageSizing::Fit, None, None).unwrap();
        near(r, (111.5, 50.75, 37.0, 18.5));
        // 10×100 px (tall): scale = min(5.85, 0.185) = 0.185 → 1.85 × 18.5 pt.
        let r = placement(CELL, (10.0, 100.0), ImageSizing::Fit, None, None).unwrap();
        near(r, (100.75 + (58.5 - 1.85) / 2.0, 50.75, 1.85, 18.5));
    }

    #[test]
    fn fill_original_and_custom() {
        let r = placement(CELL, (200.0, 100.0), ImageSizing::Fill, None, None).unwrap();
        near(r, (100.75, 50.75, 58.5, 18.5));
        // Original: 200 × 100 px at 0.75 pt/px = 150 × 75 pt from the top-left.
        let r = placement(CELL, (200.0, 100.0), ImageSizing::Original, None, None).unwrap();
        near(r, (100.75, 50.75, 150.0, 75.0));
        // Custom height 40 px only: width = 40 × 200/100 = 80 px → 60 × 30 pt.
        let r = placement(CELL, (200.0, 100.0), ImageSizing::Custom, Some(40.0), None).unwrap();
        near(r, (100.75, 50.75, 60.0, 30.0));
        // Custom width 50 px only: height = 50 × 100/200 = 25 px → 37.5 × 18.75 pt.
        let r = placement(CELL, (200.0, 100.0), ImageSizing::Custom, None, Some(50.0)).unwrap();
        near(r, (100.75, 50.75, 37.5, 18.75));
        // Both: 10 × 20 px → 15 × 7.5 pt, ratio ignored.
        let r = placement(
            CELL,
            (200.0, 100.0),
            ImageSizing::Custom,
            Some(10.0),
            Some(20.0),
        )
        .unwrap();
        near(r, (100.75, 50.75, 15.0, 7.5));
    }

    #[test]
    fn pixel_sizes_from_headers() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&3u32.to_be_bytes());
        png.extend_from_slice(&2u32.to_be_bytes());
        assert_eq!(pixel_size(&png), Some((3, 2)));
        let gif = b"GIF89a\x05\x00\x07\x00";
        assert_eq!(pixel_size(gif), Some((5, 7)));
        assert_eq!(pixel_size(b"nope"), None);
    }

    #[test]
    fn checkbox_box_and_hits() {
        // 20 pt high cell: side = min(20 − 4, 12) = 12, centred: x = 100 + 24, y = 54.
        let b = checkbox_rect(CELL);
        near(b, (124.0, 54.0, 12.0, 12.0));
        assert!(checkbox_hit(CELL, 130.0, 60.0));
        assert!(checkbox_hit(CELL, 122.5, 60.0));
        assert!(!checkbox_hit(CELL, 105.0, 60.0));
        assert_eq!(checkbox(CELL, false).len(), 1);
        assert_eq!(checkbox(CELL, true).len(), 2);
    }

    #[test]
    fn web_pictures_wait_for_the_host_and_report_failures() {
        let img = CellImage {
            source: CellImageSource::Web {
                url: "https://example.vn/a.png".into(),
                sizing: ImageSizing::Fill,
                height: None,
                width: None,
            },
            alt_text: None,
            calc_origin: 1,
        };
        let mut states = ImageStates::new();
        assert_eq!(draw(&img, CELL, &states), Ok(Vec::new()));
        states.insert(img.media_id(), ImageState::Failed("#CONNECT!".into()));
        assert_eq!(draw(&img, CELL, &states), Err("#CONNECT!".into()));
        states.insert(
            img.media_id(),
            ImageState::Ready {
                width: 4.0,
                height: 4.0,
            },
        );
        let cmds = draw(&img, CELL, &states).unwrap();
        assert!(
            matches!(&cmds[1], Command::Image { image, .. } if image == "url:https://example.vn/a.png")
        );
    }
}

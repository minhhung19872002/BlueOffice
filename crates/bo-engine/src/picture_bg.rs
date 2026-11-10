//! Picture background removal shared by BlueDoc, BlueSheet and BlueSlide
//! (CORE-PIC-002): the settings sent by the "Remove background" mode, previews with
//! the proposed removal tinted, and the result picture (a PNG with alpha).
//!
//! The segmentation is [`bo_segment`] (graph cut, no learnt weights). A host may send
//! the initial estimate of its own segmentation provider (`prior`); it then replaces
//! the rectangle as the starting point.
//!
//! Stored settings ([MS-ODRAWXML] `a14:backgroundRemoval`) are in 1/1000 % of the
//! picture; strokes become mark lines (each stroke segment one mark).

use base64::Engine as _;
use bo_model::picture::{BackgroundMark, BackgroundRemoval};
use bo_segment::{GraphCut, Image, Mark, MarkKind, Mask, PriorMask, Request, SegmentationProvider};
use serde::{Deserialize, Serialize};

/// Fraction ↔ ST_PositiveFixedPercentage.
const PCT: f64 = 100_000.0;

/// A stroke of the mode: points as fractions (0..=1) of the picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkSpec {
    /// "Mark areas to keep" (else "to remove").
    pub keep: bool,
    /// Points `[x, y]`.
    pub points: Vec<[f64; 2]>,
}

/// An initial estimate from a host segmentation provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PriorSpec {
    /// Width of the mask.
    pub width: u32,
    /// Height of the mask.
    pub height: u32,
    /// One byte per pixel (≥ 128 = foreground), base64.
    pub data: String,
}

/// Settings of a background removal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RemovalSpec {
    /// Rectangle holding the subject: left, top, right, bottom as fractions (default:
    /// the picture minus a margin).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
    /// Strokes, in order.
    pub marks: Vec<MarkSpec>,
    /// Initial estimate of a host provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior: Option<PriorSpec>,
    /// Previews also return the working picture itself (for a host provider).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub source: bool,
}

/// A preview of the mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundPreview {
    /// The (reduced) picture with the removed areas tinted, PNG base64.
    pub image: String,
    /// Its width.
    pub width: u32,
    /// Its height.
    pub height: u32,
    /// Share of the picture kept, in 1/1000 %.
    pub kept: u32,
    /// Time of the segmentation in milliseconds (0 where no clock is available).
    pub ms: u32,
    /// The working picture, PNG base64 (when asked for with `source`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn to_pct(v: f64) -> i32 {
    (v.clamp(0.0, 1.0) * PCT).round() as i32
}

fn from_pct(v: i32) -> f64 {
    f64::from(v.clamp(0, 100_000)) / PCT
}

impl RemovalSpec {
    /// The segmentation request.
    pub fn request(&self) -> Request {
        Request {
            rect: self.rect,
            marks: self
                .marks
                .iter()
                .map(|m| Mark {
                    kind: if m.keep {
                        MarkKind::Keep
                    } else {
                        MarkKind::Remove
                    },
                    points: m.points.clone(),
                })
                .collect(),
        }
    }

    /// The provider of the initial estimate.
    pub fn provider(&self) -> Box<dyn SegmentationProvider> {
        let prior = self.prior.as_ref().and_then(|p| {
            let data = base64::engine::general_purpose::STANDARD
                .decode(p.data.trim())
                .ok()?;
            let ok = p.width > 0 && data.len() == (p.width as usize) * (p.height as usize);
            ok.then_some(PriorMask(Mask {
                width: p.width,
                height: p.height,
                alpha: data,
            }))
        });
        match prior {
            Some(p) => Box::new(p),
            None => Box::new(GraphCut),
        }
    }

    /// Settings as stored in the file.
    pub fn settings(&self) -> BackgroundRemoval {
        let [l, t, r, b] = self.rect.unwrap_or_else(bo_segment::default_rect);
        let mut marks = Vec::new();
        for m in &self.marks {
            let pts: Vec<[i32; 2]> = m
                .points
                .iter()
                .filter(|p| p[0].is_finite() && p[1].is_finite())
                .map(|p| [to_pct(p[0]), to_pct(p[1])])
                .collect();
            let lines: Vec<[i32; 4]> = match pts.len() {
                0 => vec![],
                1 => vec![[pts[0][0], pts[0][1], pts[0][0], pts[0][1]]],
                _ => pts
                    .windows(2)
                    .map(|s| [s[0][0], s[0][1], s[1][0], s[1][1]])
                    .collect(),
            };
            marks.extend(
                lines
                    .into_iter()
                    .map(|line| BackgroundMark { keep: m.keep, line }),
            );
        }
        BackgroundRemoval {
            rect: [to_pct(l), to_pct(t), to_pct(r), to_pct(b)],
            marks,
        }
    }

    /// Settings read from a file.
    pub fn from_settings(s: &BackgroundRemoval) -> RemovalSpec {
        let [l, t, r, b] = s.rect;
        let rect = (r > l && b > t).then(|| [from_pct(l), from_pct(t), from_pct(r), from_pct(b)]);
        RemovalSpec {
            rect,
            marks: s
                .marks
                .iter()
                .map(|m| {
                    let [x1, y1, x2, y2] = m.line;
                    MarkSpec {
                        keep: m.keep,
                        points: vec![[from_pct(x1), from_pct(y1)], [from_pct(x2), from_pct(y2)]],
                    }
                })
                .collect(),
            prior: None,
            source: false,
        }
    }
}

/// Upper bound on the pixels of a picture whose background is removed.
pub const MAX_PIXELS: u64 = 40_000_000;

fn decode(bytes: &[u8]) -> Option<bo_raster::codecs::Rgba> {
    let img = bo_raster::picfx::decode_rgba(bytes)?;
    (u64::from(img.width) * u64::from(img.height) <= MAX_PIXELS && img.width > 0 && img.height > 0)
        .then_some(img)
}

fn image(img: &bo_raster::codecs::Rgba) -> Image<'_> {
    Image {
        width: img.width,
        height: img.height,
        rgba: &img.pixels,
    }
}

/// Preview of removing the background of `bytes` with `spec` (`None` when the picture
/// cannot be decoded).
pub fn preview(
    bytes: &[u8],
    spec: &RemovalSpec,
    now: &dyn Fn() -> f64,
) -> Option<BackgroundPreview> {
    let img = decode(bytes)?;
    let t0 = now();
    let seg = bo_segment::segment(&image(&img), &spec.request(), spec.provider().as_ref());
    let ms = (now() - t0).max(0.0).round() as u32;
    let kept = seg.mask.alpha.iter().filter(|a| **a >= 128).count() as u64 * 100_000
        / (seg.mask.alpha.len().max(1) as u64);
    let t = bo_segment::tinted(&seg);
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: t.width,
        height: t.height,
        pixels: t.rgba,
    })?;
    Some(BackgroundPreview {
        image: base64::engine::general_purpose::STANDARD.encode(png),
        width: seg.work.width,
        height: seg.work.height,
        kept: kept as u32,
        ms,
        source: spec
            .source
            .then(|| {
                bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
                    width: seg.work.width,
                    height: seg.work.height,
                    pixels: seg.work.rgba.clone(),
                })
            })
            .flatten()
            .map(|p| base64::engine::general_purpose::STANDARD.encode(p)),
    })
}

/// The picture `bytes` with its background removed: a PNG with alpha, the size of the
/// original.
pub fn remove(bytes: &[u8], spec: &RemovalSpec) -> Option<Vec<u8>> {
    let mut img = decode(bytes)?;
    let (mask, _) =
        bo_segment::remove_background(&image(&img), &spec.request(), spec.provider().as_ref());
    bo_segment::apply_mask(&mut img.pixels, &mask);
    bo_raster::rgba_to_png(img)
}

/// Milliseconds of a monotonic clock (0 in the browser build, where the host times
/// the command).
pub fn clock_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

/// Can the background of `bytes` be removed (a picture the segmentation decodes)?
pub fn removable(bytes: &[u8]) -> bool {
    !crate::picture_tools::is_svg(bytes) && crate::edit::image_size(bytes).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A red disk on a blue → green gradient (PNG).
    pub(crate) fn disk_png(w: u32, h: u32) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        let (cx, cy, r) = (
            f64::from(w) / 2.0,
            f64::from(h) / 2.0,
            f64::from(w.min(h)) * 0.3,
        );
        for y in 0..h {
            for x in 0..w {
                let inside =
                    (f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2) <= r * r;
                let t = f64::from(x) / f64::from(w);
                let c = if inside {
                    [210, 45, 40]
                } else {
                    [
                        (25.0 + 15.0 * t) as u8,
                        (70.0 + 120.0 * t) as u8,
                        (200.0 - 130.0 * t) as u8,
                    ]
                };
                pixels.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
            width: w,
            height: h,
            pixels,
        })
        .unwrap()
    }

    fn corner(png: &[u8]) -> (u32, u32, u8) {
        let i = bo_raster::picfx::decode_rgba(png).unwrap();
        (i.width, i.height, i.pixels[3])
    }

    #[test]
    fn settings_round_trip() {
        let spec = RemovalSpec {
            rect: Some([0.1, 0.2, 0.9, 0.8]),
            marks: vec![
                MarkSpec {
                    keep: true,
                    points: vec![[0.5, 0.5]],
                },
                MarkSpec {
                    keep: false,
                    points: vec![[0.1, 0.1], [0.2, 0.1], [0.3, 0.15]],
                },
            ],
            prior: None,
            source: false,
        };
        let s = spec.settings();
        assert_eq!(s.rect, [10_000, 20_000, 90_000, 80_000]);
        assert_eq!(s.marks.len(), 3);
        assert_eq!(s.marks[0].line, [50_000, 50_000, 50_000, 50_000]);
        assert_eq!(s.marks[2].line, [20_000, 10_000, 30_000, 15_000]);
        let back = RemovalSpec::from_settings(&s);
        assert_eq!(back.rect, spec.rect);
        assert_eq!(back.marks.len(), 3);
        assert!(back.marks[0].keep && !back.marks[1].keep);
        // Default rectangle when none was given.
        assert_eq!(
            RemovalSpec::default().settings().rect,
            [5000, 5000, 95_000, 95_000]
        );
    }

    #[test]
    fn preview_and_result() {
        let png = disk_png(160, 120);
        let p = preview(&png, &RemovalSpec::default(), &|| 0.0).unwrap();
        assert_eq!((p.width, p.height), (160, 120));
        // The disk is π·36² ≈ 4072 of 19200 pixels ≈ 21 %.
        assert!((19_000..23_000).contains(&p.kept), "{}", p.kept);
        let out = remove(&png, &RemovalSpec::default()).unwrap();
        let img = bo_raster::picfx::decode_rgba(&out).unwrap();
        assert_eq!((img.width, img.height), (160, 120));
        assert_eq!(img.pixels[3], 0);
        assert_eq!(img.pixels[((60 * 160 + 80) * 4 + 3) as usize], 255);
        assert!(remove(b"not a picture", &RemovalSpec::default()).is_none());
    }

    #[test]
    fn prior_from_a_host_provider() {
        let png = disk_png(80, 60);
        // A prior saying "everything is foreground" over the whole picture: kept.
        let spec = RemovalSpec {
            rect: Some([0.0, 0.0, 1.0, 1.0]),
            marks: vec![],
            prior: Some(PriorSpec {
                width: 4,
                height: 3,
                data: base64::engine::general_purpose::STANDARD.encode([255u8; 12]),
            }),
            source: true,
        };
        let p = preview(&png, &spec, &|| 0.0).unwrap();
        assert_eq!(p.kept, 100_000);
        // The working picture comes back for the provider (80 × 60, opaque).
        let src = base64::engine::general_purpose::STANDARD
            .decode(p.source.unwrap())
            .unwrap();
        assert_eq!(corner(&src), (80, 60, 255));
        // A malformed prior falls back to the graph-cut provider.
        let bad = RemovalSpec {
            prior: Some(PriorSpec {
                width: 4,
                height: 3,
                data: "AAAA".into(),
            }),
            ..RemovalSpec::default()
        };
        assert_eq!(bad.provider().id(), "graph-cut");
        assert_eq!(spec.provider().id(), "prior-mask");
    }
}

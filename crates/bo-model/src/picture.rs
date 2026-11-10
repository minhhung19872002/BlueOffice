//! Picture adjustments shared by BlueDoc, BlueSheet and BlueSlide (CORE-PIC-001):
//! the colour transforms of a picture's `a:blip` (ECMA-376 Part 1, §20.1.8.13 blip
//! children), crop rectangles (§20.1.8.55 srcRect) and the geometry helpers the
//! picture tools use (crop to an aspect ratio, fill, fit).
//!
//! The renderers draw an adjusted picture through a *derived image id*: the media
//! id followed by `#fx:` and the adjustments ([`PictureFx::derived_id`]), which every
//! display list backend resolves by decoding the original picture and applying the
//! transforms to its pixels. The file keeps the original picture and the adjustment
//! elements, so adjusting is lossless and "reset" restores the original.

use serde::{Deserialize, Serialize};

/// Separator of a derived image id (`/word/media/image1.png#fx:b=20000`).
pub const FX_MARK: &str = "#fx:";

/// §20.1.8.16 clrChange: pixels of colour `from` become `to` with opacity `alpha`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ColorChange {
    /// `clrFrom` as `0xRRGGBB`.
    pub from: u32,
    /// `clrTo` as `0xRRGGBB`.
    pub to: u32,
    /// Opacity of `clrTo` in 1/1000 of a percent (0 = transparent, 100 000 = opaque).
    pub alpha: i32,
}

/// Colour and correction adjustments of a picture (children of `a:blip`).
/// Percentages are in 1/1000 of a percent (ST_FixedPercentage, §20.1.10.38).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PictureFx {
    /// §20.1.8.42 lum@bright: brightness change, −100 000 … 100 000.
    pub bright: i32,
    /// §20.1.8.42 lum@contrast: contrast change, −100 000 … 100 000.
    pub contrast: i32,
    /// [MS-ODRAWXML] `a14:sharpenSoften@amount` (CT_PictureEffectSharpenSoften, in
    /// `a14:imgProps/a14:imgLayer/a14:imgEffect`):
    /// positive sharpens, negative softens, −100 000 … 100 000.
    pub sharpen: i32,
    /// [MS-ODRAWXML] `a14:saturation@sat` (CT_PictureEffectSaturation): 100 000 keeps the colours,
    /// 0 is grey, up to 400 000.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation: Option<i32>,
    /// §20.1.8.34 grayscl.
    pub grayscale: bool,
    /// §20.1.8.11 biLevel@thresh: black below the luminance threshold, else white.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bilevel: Option<i32>,
    /// §20.1.8.23 duotone: the dark and the light colour (`0xRRGGBB`), interpolated
    /// by each pixel's luminance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duotone: Option<[u32; 2]>,
    /// §20.1.8.16 clrChange.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_change: Option<ColorChange>,
    /// §20.1.8.4 alphaModFix@amt: opacity, 0 … 100 000.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<i32>,
}

impl PictureFx {
    /// Nothing changes the picture.
    pub fn is_identity(&self) -> bool {
        self.bright == 0
            && self.contrast == 0
            && self.sharpen == 0
            && self.saturation.is_none_or(|s| s == 100_000)
            && !self.grayscale
            && self.bilevel.is_none()
            && self.duotone.is_none()
            && self.color_change.is_none()
            && self.alpha.is_none_or(|a| a >= 100_000)
    }

    /// The colour transforms (corrections and recolour) without the opacity.
    #[must_use]
    pub fn without_alpha(&self) -> PictureFx {
        PictureFx {
            alpha: None,
            ..self.clone()
        }
    }

    /// Values clamped to their schema ranges.
    #[must_use]
    pub fn clamped(&self) -> PictureFx {
        let pct = |v: i32| v.clamp(-100_000, 100_000);
        PictureFx {
            bright: pct(self.bright),
            contrast: pct(self.contrast),
            sharpen: pct(self.sharpen),
            saturation: self.saturation.map(|s| s.clamp(0, 400_000)),
            grayscale: self.grayscale,
            bilevel: self.bilevel.map(|t| t.clamp(0, 100_000)),
            duotone: self.duotone.map(|d| d.map(|c| c & 0xFF_FFFF)),
            color_change: self.color_change.map(|c| ColorChange {
                from: c.from & 0xFF_FFFF,
                to: c.to & 0xFF_FFFF,
                alpha: c.alpha.clamp(0, 100_000),
            }),
            alpha: self.alpha.map(|a| a.clamp(0, 100_000)),
        }
    }

    /// Compact text of the adjustments (the part after [`FX_MARK`] in a derived id).
    pub fn to_key(&self) -> String {
        let mut p: Vec<String> = Vec::new();
        if self.bright != 0 {
            p.push(format!("b={}", self.bright));
        }
        if self.contrast != 0 {
            p.push(format!("c={}", self.contrast));
        }
        if self.sharpen != 0 {
            p.push(format!("s={}", self.sharpen));
        }
        if let Some(s) = self.saturation.filter(|s| *s != 100_000) {
            p.push(format!("sat={s}"));
        }
        if self.grayscale {
            p.push("g".into());
        }
        if let Some(t) = self.bilevel {
            p.push(format!("bl={t}"));
        }
        if let Some([a, b]) = self.duotone {
            p.push(format!("d={a:06X}-{b:06X}"));
        }
        if let Some(c) = self.color_change {
            p.push(format!("cc={:06X}-{:06X}-{}", c.from, c.to, c.alpha));
        }
        if let Some(a) = self.alpha.filter(|a| *a < 100_000) {
            p.push(format!("a={a}"));
        }
        p.join(";")
    }

    /// Parse [`PictureFx::to_key`] text (unknown items are ignored).
    pub fn from_key(key: &str) -> PictureFx {
        let mut fx = PictureFx::default();
        let hex = |s: &str| u32::from_str_radix(s, 16).ok();
        for item in key.split(';') {
            let (k, v) = item.split_once('=').unwrap_or((item, ""));
            let n = v.parse::<i32>().ok();
            match k {
                "b" => fx.bright = n.unwrap_or(0),
                "c" => fx.contrast = n.unwrap_or(0),
                "s" => fx.sharpen = n.unwrap_or(0),
                "sat" => fx.saturation = n,
                "g" => fx.grayscale = true,
                "bl" => fx.bilevel = n,
                "d" => {
                    if let Some((a, b)) = v.split_once('-') {
                        if let (Some(a), Some(b)) = (hex(a), hex(b)) {
                            fx.duotone = Some([a, b]);
                        }
                    }
                }
                "cc" => {
                    let mut it = v.split('-');
                    if let (Some(f), Some(t), Some(a)) = (
                        it.next().and_then(hex),
                        it.next().and_then(hex),
                        it.next().and_then(|a| a.parse().ok()),
                    ) {
                        fx.color_change = Some(ColorChange {
                            from: f,
                            to: t,
                            alpha: a,
                        });
                    }
                }
                "a" => fx.alpha = n,
                _ => {}
            }
        }
        fx.clamped()
    }

    /// Image id the renderers resolve to `media` with these adjustments applied
    /// (`media` itself when nothing changes).
    pub fn derived_id(&self, media: &str) -> String {
        if self.is_identity() {
            media.to_owned()
        } else {
            format!("{media}{FX_MARK}{}", self.to_key())
        }
    }
}

/// Split a derived image id into the media id and its adjustments.
pub fn split_fx_id(id: &str) -> Option<(&str, PictureFx)> {
    let (base, key) = id.split_once(FX_MARK)?;
    Some((base, PictureFx::from_key(key)))
}

/// Picture formatting of a word-processing picture: adjustments of its `a:blip`
/// and the preset shape it is cropped to (§20.1.9.18 prstGeom of `pic:spPr`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PictureFormat {
    /// Colour adjustments.
    pub fx: PictureFx,
    /// Preset shape other than `rect`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Background removed (CORE-PIC-002): the media part shows the result, the
    /// original picture and the removal settings are kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal: Option<Box<RemovedBackground>>,
}

impl PictureFormat {
    /// Nothing differs from a plain picture.
    pub fn is_plain(&self) -> bool {
        self.fx.is_identity()
            && self.shape.as_deref().is_none_or(|s| s == "rect")
            && self.removal.is_none()
    }
}

/// A picture whose background was removed: the original media part (the
/// [MS-ODRAWXML] `a14:imgLayer@r:embed` of the blip) and the settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RemovedBackground {
    /// Media part of the original picture.
    pub original: String,
    /// Settings ([MS-ODRAWXML] `a14:backgroundRemoval`).
    pub settings: BackgroundRemoval,
}

/// [MS-ODRAWXML] `a14:backgroundRemoval` (CT_PictureEffectBackgroundRemoval): the
/// rectangle holding the foreground and the user's marks, in 1/1000 % of the
/// picture's width / height (ST_PositiveFixedPercentage).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BackgroundRemoval {
    /// Left, top, right, bottom edges of the rectangle.
    pub rect: [i32; 4],
    /// Marks, in order.
    pub marks: Vec<BackgroundMark>,
}

/// A mark line of a background removal ([MS-ODRAWXML] `a14:foregroundMark` /
/// `a14:backgroundMark`, CT_PictureEffectBackgroundRemovalMark: `x1 y1 x2 y2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BackgroundMark {
    /// Foreground mark (keep) rather than background mark (remove).
    pub keep: bool,
    /// x1, y1, x2, y2.
    pub line: [i32; 4],
}

/// Clip of a derived picture: the preset shape over the visible (cropped) area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipShape {
    /// Preset shape name.
    pub name: String,
    /// Crop of the picture (l, t, r, b in 1/1000 %): the shape spans what remains.
    pub crop: [i32; 4],
}

/// Image id of a picture with `format` (see [`PictureFx::derived_id`]); a crop shape
/// adds `shape=<name>@l,t,r,b` so the backends can mask the picture.
pub fn display_id(media: &str, format: &PictureFormat, crop: Option<[i32; 4]>) -> String {
    let mut key = format.fx.to_key();
    if let Some(s) = format.shape.as_deref().filter(|s| *s != "rect") {
        let [l, t, r, b] = crop.unwrap_or([0; 4]);
        if !key.is_empty() {
            key.push(';');
        }
        key.push_str(&format!("shape={s}@{l},{t},{r},{b}"));
    }
    if key.is_empty() {
        media.to_owned()
    } else {
        format!("{media}{FX_MARK}{key}")
    }
}

/// The crop shape of a derived image id, if any.
pub fn clip_of_id(id: &str) -> Option<ClipShape> {
    let (_, key) = id.split_once(FX_MARK)?;
    let v = key.split(';').find_map(|i| i.strip_prefix("shape="))?;
    let (name, rest) = v.split_once('@').unwrap_or((v, "0,0,0,0"));
    let n: Vec<i32> = rest.split(',').filter_map(|x| x.parse().ok()).collect();
    let crop = if let [l, t, r, b] = n[..] {
        [l, t, r, b]
    } else {
        [0; 4]
    };
    Some(ClipShape {
        name: name.to_owned(),
        crop,
    })
}

/// Full scale of crop and percentage values (1/1000 of a percent).
pub const PCT: f64 = 100_000.0;

/// §20.1.8.55 srcRect that cuts the picture (`img_w` × `img_h`, any unit) to the
/// aspect ratio `rw`:`rh`, centred: the longer side loses equal parts at both ends.
/// Values in 1/1000 of a percent of the picture (l, t, r, b).
pub fn aspect_crop(img_w: f64, img_h: f64, rw: f64, rh: f64) -> [i32; 4] {
    if img_w <= 0.0 || img_h <= 0.0 || rw <= 0.0 || rh <= 0.0 {
        return [0; 4];
    }
    let a = img_w / img_h;
    let r = rw / rh;
    if (a - r).abs() < 1e-9 {
        return [0; 4];
    }
    if a > r {
        // Too wide: keep r/a of the width.
        let cut = ((1.0 - r / a) / 2.0 * PCT).round() as i32;
        [cut, 0, cut, 0]
    } else {
        // Too tall: keep a/r of the height.
        let cut = ((1.0 - a / r) / 2.0 * PCT).round() as i32;
        [0, cut, 0, cut]
    }
}

/// Size of the picture part shown through `crop` (same unit as `img_w`/`img_h`).
pub fn visible_size(img_w: f64, img_h: f64, crop: Option<[i32; 4]>) -> (f64, f64) {
    let [l, t, r, b] = crop.unwrap_or([0; 4]).map(f64::from);
    (
        img_w * (1.0 - (l + r) / PCT).max(0.0),
        img_h * (1.0 - (t + b) / PCT).max(0.0),
    )
}

/// "Fit": the largest box with the picture's aspect ratio inside the frame
/// `frame_w` × `frame_h` (the whole picture shows, nothing cropped).
pub fn fit_size(img_w: f64, img_h: f64, frame_w: f64, frame_h: f64) -> (f64, f64) {
    if img_w <= 0.0 || img_h <= 0.0 {
        return (frame_w, frame_h);
    }
    let k = (frame_w / img_w).min(frame_h / img_h);
    (img_w * k, img_h * k)
}

/// "Fill": the srcRect that fills the frame `frame_w` × `frame_h` with the picture
/// (no distortion; the overflowing part is cropped, centred).
pub fn fill_crop(img_w: f64, img_h: f64, frame_w: f64, frame_h: f64) -> [i32; 4] {
    aspect_crop(img_w, img_h, frame_w, frame_h)
}

/// Named aspect ratios of the crop menu (`w`:`h`).
pub fn aspect_ratio(name: &str) -> Option<(f64, f64)> {
    let (w, h) = name.split_once(':')?;
    let (w, h) = (w.trim().parse::<f64>().ok()?, h.trim().parse::<f64>().ok()?);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_round_trip() {
        let fx = PictureFx {
            bright: 20_000,
            contrast: -40_000,
            sharpen: 25_000,
            saturation: Some(0),
            grayscale: true,
            bilevel: Some(50_000),
            duotone: Some([0x00_0000, 0xFF_EEDD]),
            color_change: Some(ColorChange {
                from: 0xFF_FFFF,
                to: 0xFF_FFFF,
                alpha: 0,
            }),
            alpha: Some(50_000),
        };
        let id = fx.derived_id("/word/media/image1.png");
        assert_eq!(
            id,
            "/word/media/image1.png#fx:b=20000;c=-40000;s=25000;sat=0;g;bl=50000;d=000000-FFEEDD;cc=FFFFFF-FFFFFF-0;a=50000"
        );
        let (base, back) = split_fx_id(&id).unwrap();
        assert_eq!(base, "/word/media/image1.png");
        assert_eq!(back, fx);
        assert_eq!(PictureFx::default().derived_id("x.png"), "x.png");
        // Crop shape: carried with the crop so the mask covers the visible part.
        let f = PictureFormat {
            fx: PictureFx {
                grayscale: true,
                ..PictureFx::default()
            },
            shape: Some("ellipse".into()),
            ..PictureFormat::default()
        };
        let id = display_id("/m.png", &f, Some([10_000, 0, 10_000, 0]));
        assert_eq!(id, "/m.png#fx:g;shape=ellipse@10000,0,10000,0");
        assert_eq!(
            clip_of_id(&id),
            Some(ClipShape {
                name: "ellipse".into(),
                crop: [10_000, 0, 10_000, 0]
            })
        );
        assert!(split_fx_id(&id).unwrap().1.grayscale);
        assert_eq!(
            display_id("/m.png", &PictureFormat::default(), None),
            "/m.png"
        );
    }

    #[test]
    fn aspect_crops_hand_derived() {
        // 400×300 (4:3) to 1:1: keep 300/400 = 75 % of the width → 12.5 % each side.
        assert_eq!(aspect_crop(400.0, 300.0, 1.0, 1.0), [12_500, 0, 12_500, 0]);
        // 400×300 to 16:9: a = 4/3 < r = 16/9 → keep (4/3)/(16/9) = 75 % of the height.
        assert_eq!(aspect_crop(400.0, 300.0, 16.0, 9.0), [0, 12_500, 0, 12_500]);
        // 1920×1080 (16:9) to 4:3: keep (4/3)/(16/9) = 75 % of the width.
        assert_eq!(
            aspect_crop(1920.0, 1080.0, 4.0, 3.0),
            [12_500, 0, 12_500, 0]
        );
        // 300×400 (3:4) to 2:3: a = 0.75 > r = 0.6667 → keep 0.6667/0.75 = 88.889 %
        // of the width → 5.5556 % each side.
        assert_eq!(aspect_crop(300.0, 400.0, 2.0, 3.0), [5_556, 0, 5_556, 0]);
        // Same ratio: nothing cut.
        assert_eq!(aspect_crop(400.0, 300.0, 4.0, 3.0), [0; 4]);
        assert_eq!(aspect_ratio("16:9"), Some((16.0, 9.0)));
        assert_eq!(aspect_ratio("0:9"), None);
    }

    #[test]
    fn fit_and_fill() {
        // 200×100 picture in a 100×100 frame: fit is 100×50; fill crops 25 % each side.
        assert_eq!(fit_size(200.0, 100.0, 100.0, 100.0), (100.0, 50.0));
        assert_eq!(
            fill_crop(200.0, 100.0, 100.0, 100.0),
            [25_000, 0, 25_000, 0]
        );
        assert_eq!(
            visible_size(200.0, 100.0, Some([25_000, 0, 25_000, 10_000])),
            (100.0, 90.0)
        );
    }
}

//! Pictures inside cells and cell controls (checkboxes).
//!
//! * A picture placed in a cell is a rich value of type `_localImage`
//!   ([MS-XLSX] §2.3.6.1.10 Local Image Type): its bytes live in a media part
//!   referenced through the Rich Value Rels part (§2.1.21), and the cell points
//!   at it through `c/@vm` → `metadata/valueMetadata` → `futureMetadata
//!   name="XLRICHVALUE"` → `xlrd:rvb/@i` ([MS-XLSX] §2.2.4.4; ECMA-376 Part 1,
//!   §18.9). The cell's own value is the fallback `#VALUE!`.
//! * The `IMAGE` function returns a web picture ([MS-XLSX] §2.3.6.1.11 Web Image
//!   Type); its bytes are fetched by the host and never stored in the model.
//! * A checkbox is a cell control carried by the cell format ([MS-XLSX] §2.3.9
//!   Feature Property Bag: `XFComplements` → `XFComplement` → `XFControls` →
//!   `Checkbox`), see [`super::CellXf::checkbox`].

use serde::{Deserialize, Serialize};

/// `CalcOrigin` of a picture stored directly in a cell without a formula
/// ([MS-XLSX] §2.3.6.1.10: `Standalone` = 5).
pub const CALC_ORIGIN_STANDALONE: u32 = 5;
/// `CalcOrigin` of a decorative standalone picture (`StandaloneDecorative` = 6).
pub const CALC_ORIGIN_DECORATIVE: u32 = 6;

/// How a picture is sized inside its cell ([MS-XLSX] §2.3.6.1.11 `ImageSizing`;
/// the `sizing` argument of `IMAGE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageSizing {
    /// 0: fit inside the cell, keeping the aspect ratio.
    #[default]
    Fit,
    /// 1: fill the cell, ignoring the aspect ratio.
    Fill,
    /// 2: the picture's original size (may exceed the cell).
    Original,
    /// 3: the size given by `height` / `width` (pixels).
    Custom,
}

impl ImageSizing {
    /// The numeric code (0–3) of [MS-XLSX] `ImageSizing`.
    pub fn code(self) -> u32 {
        match self {
            ImageSizing::Fit => 0,
            ImageSizing::Fill => 1,
            ImageSizing::Original => 2,
            ImageSizing::Custom => 3,
        }
    }

    /// Sizing from its numeric code.
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0 => ImageSizing::Fit,
            1 => ImageSizing::Fill,
            2 => ImageSizing::Original,
            3 => ImageSizing::Custom,
            _ => return None,
        })
    }
}

/// Where the pixels of an in-cell picture come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CellImageSource {
    /// A picture stored in the workbook (`_localImage`).
    Local {
        /// Encoded picture bytes (PNG, JPEG, GIF, …).
        data: Vec<u8>,
        /// File extension of the media part (`png`, `jpeg`, …), lower case.
        ext: String,
        /// Stable id of the bytes (`cellimg:` + hash), used by renderers to cache them.
        key: String,
    },
    /// The result of `IMAGE(url, …)`: fetched by the host, never stored.
    Web {
        /// Source URL (`http` / `https`).
        url: String,
        /// `sizing` argument.
        sizing: ImageSizing,
        /// Custom height in pixels (`sizing` 3).
        height: Option<f64>,
        /// Custom width in pixels (`sizing` 3).
        width: Option<f64>,
    },
}

/// A picture shown inside a cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellImage {
    /// Pixels.
    pub source: CellImageSource,
    /// Alternative text (`Text` key).
    #[serde(default)]
    pub alt_text: Option<String>,
    /// `CalcOrigin` ([MS-XLSX] §2.3.6.1.10); 5 for pictures placed in a cell.
    #[serde(default)]
    pub calc_origin: u32,
}

impl CellImage {
    /// A picture stored in the workbook, placed in a cell.
    pub fn local(data: Vec<u8>, ext: &str, alt_text: Option<String>) -> Self {
        let key = format!("cellimg:{:016x}", fnv1a(&data));
        CellImage {
            source: CellImageSource::Local {
                data,
                ext: ext.to_ascii_lowercase(),
                key,
            },
            alt_text,
            calc_origin: CALC_ORIGIN_STANDALONE,
        }
    }

    /// Bytes of a stored picture (`None` for web pictures).
    pub fn data(&self) -> Option<&[u8]> {
        match &self.source {
            CellImageSource::Local { data, .. } => Some(data),
            CellImageSource::Web { .. } => None,
        }
    }

    /// Id renderers use to fetch / cache the pixels: `cellimg:<hash>` for stored
    /// pictures, `url:<url>` for web pictures.
    pub fn media_id(&self) -> String {
        match &self.source {
            CellImageSource::Local { key, .. } => key.clone(),
            CellImageSource::Web { url, .. } => format!("url:{url}"),
        }
    }

    /// Sizing mode (stored pictures always fit).
    pub fn sizing(&self) -> ImageSizing {
        match &self.source {
            CellImageSource::Local { .. } => ImageSizing::Fit,
            CellImageSource::Web { sizing, .. } => *sizing,
        }
    }
}

/// 64-bit FNV-1a hash (content id of picture bytes; not cryptographic).
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_images_with_the_same_bytes_share_an_id() {
        let a = CellImage::local(vec![1, 2, 3], "PNG", None);
        let b = CellImage::local(vec![1, 2, 3], "png", Some("x".into()));
        let c = CellImage::local(vec![1, 2, 4], "png", None);
        assert_eq!(a.media_id(), b.media_id());
        assert_ne!(a.media_id(), c.media_id());
        assert!(a.media_id().starts_with("cellimg:"));
        assert_eq!(a.calc_origin, CALC_ORIGIN_STANDALONE);
    }

    #[test]
    fn sizing_codes() {
        for c in 0..4 {
            assert_eq!(ImageSizing::from_code(c).map(ImageSizing::code), Some(c));
        }
        assert_eq!(ImageSizing::from_code(4), None);
    }
}

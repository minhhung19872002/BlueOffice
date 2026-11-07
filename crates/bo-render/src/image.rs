//! Picture sources and format sniffing shared by the raster and PDF backends.

use std::collections::BTreeMap;
use std::sync::Arc;

/// Binary parts keyed by part name (`/word/media/image1.png`).
pub type Media = BTreeMap<String, Arc<Vec<u8>>>;

/// Where backends find the encoded bytes of `Image` commands (by `image` id).
pub trait ImageSource {
    fn image(&self, id: &str) -> Option<&[u8]>;
}

impl ImageSource for BTreeMap<String, Vec<u8>> {
    fn image(&self, id: &str) -> Option<&[u8]> {
        self.get(id).map(Vec::as_slice)
    }
}

impl ImageSource for Media {
    fn image(&self, id: &str) -> Option<&[u8]> {
        self.get(id).map(|b| b.as_slice())
    }
}

/// Look up in several sources in order.
pub struct Chain<'a>(pub &'a dyn ImageSource, pub &'a dyn ImageSource);

impl ImageSource for Chain<'_> {
    fn image(&self, id: &str) -> Option<&[u8]> {
        self.0.image(id).or_else(|| self.1.image(id))
    }
}

/// Container format sniffed from magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Other,
}

/// Detect the format of `bytes`.
pub fn sniff(bytes: &[u8]) -> Format {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Format::Png
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Format::Jpeg
    } else {
        Format::Other
    }
}

/// JPEG frame header: (width, height, components) from the first SOFn marker (T.81 §B.2.2).
pub fn jpeg_info(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
        let is_sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_sof {
            let seg = bytes.get(i + 4..i + 2 + len)?;
            let h = u32::from(u16::from_be_bytes([*seg.get(1)?, *seg.get(2)?]));
            let w = u32::from(u16::from_be_bytes([*seg.get(3)?, *seg.get(4)?]));
            let n = *seg.get(5)?;
            return Some((w, h, n));
        }
        i += 2 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_header() {
        // SOI, APP0 (len 4), SOF0 (len 11): 8-bit, 3×2, 3 components.
        let j = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00,
            0x02, 0x00, 0x03, 0x03, 0x01, 0x11, 0x00,
        ];
        assert_eq!(jpeg_info(&j), Some((3, 2, 3)));
        assert_eq!(sniff(&j), Format::Jpeg);
        assert!(jpeg_info(&[0xFF, 0xD8, 0x00]).is_none());
    }
}

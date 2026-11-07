//! Picture decoding shared by the raster and PDF backends.
//!
//! Supported: PNG (ISO/IEC 15948) and baseline/progressive JPEG (ITU T.81).
//! Other formats (EMF/WMF, GIF, TIFF) are drawn as a grey placeholder box.

use std::collections::BTreeMap;
use std::sync::Arc;

/// Binary parts keyed by part name (`/word/media/image1.png`).
pub type Media = BTreeMap<String, Arc<Vec<u8>>>;

/// Decoding limits (prompt §5.9: reject absurd sizes instead of allocating them).
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// A decoded image, straight (non-premultiplied) RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
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

/// Decode a PNG or JPEG picture.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    match sniff(bytes) {
        Format::Png => decode_png(bytes),
        Format::Jpeg => decode_jpeg(bytes),
        Format::Other => None,
    }
}

fn decode_png(bytes: &[u8]) -> Option<Decoded> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let (w, h) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == (w * h * 4) as usize).then_some(Decoded {
        width: w,
        height: h,
        rgba,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Option<Decoded> {
    let (w, h, _) = jpeg_info(bytes)?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    let px = dec.decode().ok()?;
    let info = dec.info()?;
    let rgba: Vec<u8> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => px
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        jpeg_decoder::PixelFormat::L16 => px
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], 255])
            .collect(),
        jpeg_decoder::PixelFormat::CMYK32 => px
            .chunks_exact(4)
            .flat_map(|p| {
                // Adobe JPEGs store inverted CMYK; approximate conversion to sRGB.
                let k = u16::from(p[3]);
                let f = |c: u8| ((u16::from(c) * k) / 255) as u8;
                [f(p[0]), f(p[1]), f(p[2]), 255]
            })
            .collect(),
    };
    Some(Decoded {
        width: u32::from(info.width),
        height: u32::from(info.height),
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×1 RGB PNG built with the `png` encoder.
    pub(crate) fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 0, 0, 255]).unwrap();
        }
        out
    }

    #[test]
    fn png_roundtrip() {
        let d = decode(&tiny_png()).unwrap();
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(d.rgba, vec![255, 0, 0, 255, 0, 0, 255, 255]);
        assert_eq!(sniff(b"GIF89a"), Format::Other);
        assert!(decode(b"\x89PNG\r\n\x1a\nbroken").is_none());
    }

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

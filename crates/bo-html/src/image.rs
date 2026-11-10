//! Pictures that travel with a page: `data:` URLs (RFC 2397) and the pixel size of
//! PNG (ISO/IEC 15948 §11.2.2 IHDR), JPEG (ITU-T T.81 §B.2.2 frame header), GIF
//! (GIF89a §18 logical screen descriptor) and BMP (`BITMAPINFOHEADER`) files.

use base64::Engine as _;

/// What a picture file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    /// File extension (`png`, `jpeg`, `gif`, `bmp`).
    pub ext: &'static str,
    /// Media type.
    pub mime: &'static str,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Largest `data:` URL payload decoded (bytes of text).
const MAX_DATA_URL: usize = 96 * 1024 * 1024;

/// Bytes of a `data:` URL (`data:[<type>][;base64],<data>`), `None` for other URLs.
pub fn data_url(url: &str) -> Option<Vec<u8>> {
    let url = url.trim();
    if url.len() > MAX_DATA_URL || !url.get(..5)?.eq_ignore_ascii_case("data:") {
        return None;
    }
    let (meta, data) = url[5..].split_once(',')?;
    if meta.to_ascii_lowercase().ends_with(";base64") {
        let clean: Vec<u8> = data
            .bytes()
            .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
            .collect();
        base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(clean)
            .ok()
    } else {
        Some(percent_decode(data))
    }
}

/// Percent-decoding (RFC 3986 §2.1); malformed escapes stay as they are.
pub fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| char::from(c).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
    ])))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
    ])))
}

/// Recognise a picture by its signature and read its pixel size.
pub fn sniff(b: &[u8]) -> Option<ImageInfo> {
    let info = |ext, mime, width: u32, height: u32| {
        (width > 0 && height > 0 && width <= 65_535 && height <= 65_535).then_some(ImageInfo {
            ext,
            mime,
            width,
            height,
        })
    };
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
        return info("png", "image/png", w, h);
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return info("gif", "image/gif", le16(b, 6)?, le16(b, 8)?);
    }
    if b.starts_with(b"BM") && b.len() > 26 {
        let w = i32::from_le_bytes(b.get(18..22)?.try_into().ok()?);
        let h = i32::from_le_bytes(b.get(22..26)?.try_into().ok()?);
        return info("bmp", "image/bmp", w.unsigned_abs(), h.unsigned_abs());
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        // Walk the marker segments to the frame header (SOF0–SOF15 except DHT, JPG, DAC).
        let mut i = 2;
        while i + 4 <= b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            match marker {
                0xFF => i += 1,
                0xD0..=0xD9 | 0x01 | 0x00 => i += 2,
                0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                    return info("jpeg", "image/jpeg", be16(b, i + 7)?, be16(b, i + 5)?);
                }
                _ => i += 2 + be16(b, i + 2)?.max(2) as usize,
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1×1 PNG (signature, IHDR, IDAT, IEND).
    pub(crate) const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x56, 0xC7, 0x2F, 0x0D, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn data_urls() {
        assert_eq!(
            data_url("data:text/plain;base64,SGk="),
            Some(b"Hi".to_vec())
        );
        assert_eq!(data_url("DATA:;BASE64,SG\nk"), Some(b"Hi".to_vec()));
        assert_eq!(data_url("data:,a%20b%zz%4"), Some(b"a b%zz%4".to_vec()));
        assert_eq!(data_url("http://x/y.png"), None);
        assert_eq!(data_url("data:image/png;base64,***"), None);
        assert_eq!(data_url("data:"), None);
        assert_eq!(data_url("dat"), None);
    }

    #[test]
    fn sizes() {
        let p = sniff(PNG_1X1).unwrap();
        assert_eq!((p.ext, p.width, p.height), ("png", 1, 1));
        let gif = b"GIF89a\x02\x00\x03\x00\x00\x00\x00";
        assert_eq!(sniff(gif).map(|g| (g.width, g.height)), Some((2, 3)));
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00,
            0x10, 0x00, 0x20, 0x03,
        ];
        assert_eq!(
            sniff(&jpeg).map(|g| (g.ext, g.width, g.height)),
            Some(("jpeg", 32, 16))
        );
        let mut bmp = vec![0u8; 30];
        bmp[..2].copy_from_slice(b"BM");
        bmp[18] = 5;
        bmp[22..26].copy_from_slice(&(-7i32).to_le_bytes());
        assert_eq!(sniff(&bmp).map(|g| (g.width, g.height)), Some((5, 7)));
        for junk in [
            &b""[..],
            b"\x89PNG\r\n\x1a\n",
            b"GIF89a",
            b"\xFF\xD8\xFF",
            b"<svg/>",
            b"BM",
        ] {
            assert_eq!(sniff(junk), None);
        }
    }
}

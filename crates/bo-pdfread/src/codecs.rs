//! Image codecs at the end of a filter chain (ISO 32000-1, §7.4.6 CCITTFaxDecode, §7.4.7
//! JBIG2Decode, §7.4.9 JPXDecode), decoded with the hayro decoders (MIT / Apache-2.0).
//! Bi-level codecs return packed 1-bit rows in the PDF convention (0 = black unless
//! `/BlackIs1`), so the caller treats them like any other 1-bit image.
use crate::store::Document;
use lopdf::{Dictionary, Object};

/// Largest decoded picture (pixels).
const MAX_PIXELS: u64 = 24 * 1024 * 1024;

/// Packs rows of black/white pixels MSB first, rows padded to whole bytes.
struct Rows {
    width: usize,
    height: usize,
    data: Vec<u8>,
    row: usize,
    x: usize,
    /// Bit value of a black pixel.
    black_bit: bool,
}

impl Rows {
    fn new(width: usize, height: usize, black_bit: bool) -> Self {
        let row_bytes = width.div_ceil(8);
        // Start white: all bits are the white value.
        let fill = if black_bit { 0 } else { 0xff };
        Self {
            width,
            height,
            data: vec![fill; row_bytes * height],
            row: 0,
            x: 0,
            black_bit,
        }
    }

    fn row_bytes(&self) -> usize {
        self.width.div_ceil(8)
    }

    fn set(&mut self, x: usize, black: bool) {
        if self.row >= self.height || x >= self.width {
            return;
        }
        let i = self.row * self.row_bytes() + x / 8;
        let bit = 0x80_u8 >> (x % 8);
        if black == self.black_bit {
            self.data[i] |= bit;
        } else {
            self.data[i] &= !bit;
        }
    }
}

impl hayro_ccitt::Decoder for Rows {
    fn push_row(&mut self, changes: &[u32]) {
        // Pixels from changes[0] to changes[1] are black, then white to changes[2], …
        let mut black = false;
        let mut start = 0_usize;
        for &c in changes.iter().chain(std::iter::once(&(self.width as u32))) {
            let end = (c as usize).min(self.width);
            if black {
                for x in start..end {
                    self.set(x, true);
                }
            }
            start = end;
            black = !black;
        }
        self.row += 1;
    }
}

impl hayro_jbig2::Decoder for Rows {
    fn push_pixel(&mut self, black: bool) {
        if black {
            self.set(self.x, true);
        }
        self.x += 1;
    }

    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        let n = chunk_count as usize * 8;
        if black {
            for x in self.x..self.x + n {
                self.set(x, true);
            }
        }
        self.x += n;
    }

    fn next_line(&mut self) {
        self.row += 1;
        self.x = 0;
    }
}

fn int(d: &Dictionary, key: &[u8], default: i64) -> i64 {
    d.get(key)
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(default)
}

fn flag(d: &Dictionary, key: &[u8], default: bool) -> bool {
    d.get(key)
        .ok()
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(default)
}

/// CCITT Group 3 / Group 4 fax data (§7.4.6, Table 11) as packed 1-bit rows.
pub(crate) fn ccitt(
    data: &[u8],
    params: &Dictionary,
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let k = int(params, b"K", 0);
    let columns = int(params, b"Columns", 1728);
    let columns = usize::try_from(columns).ok().filter(|c| *c > 0)?;
    let rows = usize::try_from(int(params, b"Rows", 0)).unwrap_or(0);
    let rows = if rows > 0 { rows } else { height };
    if (columns as u64) * (rows as u64) > MAX_PIXELS {
        return None;
    }
    let settings = hayro_ccitt::DecodeSettings {
        columns: u32::try_from(columns).ok()?,
        rows: u32::try_from(rows).ok()?,
        end_of_block: flag(params, b"EndOfBlock", true),
        end_of_line: flag(params, b"EndOfLine", false),
        rows_are_byte_aligned: flag(params, b"EncodedByteAlign", false),
        encoding: match k {
            k if k < 0 => hayro_ccitt::EncodingMode::Group4,
            0 => hayro_ccitt::EncodingMode::Group3_1D,
            _ => hayro_ccitt::EncodingMode::Group3_2D,
        },
        strict: false,
    };
    let black_is_1 = flag(params, b"BlackIs1", false);
    let mut out = Rows::new(columns, rows, black_is_1);
    let mut ctx = hayro_ccitt::DecoderContext::new(settings);
    // A truncated stream keeps the rows decoded so far.
    let _ = hayro_ccitt::decode(data, &mut out, &mut ctx);
    if out.row == 0 {
        return None;
    }
    let white = if black_is_1 { 0 } else { 0xff };
    Some(fit(out.data, (columns, rows), (width, height), white))
}

/// A JBIG2 page in the embedded organization (§7.4.7) with optional `/JBIG2Globals`, as packed
/// 1-bit rows (0 = black, the DeviceGray convention).
pub(crate) fn jbig2(
    data: &[u8],
    globals: Option<&[u8]>,
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let settings = hayro_jbig2::DecodeSettings::default();
    let globals = globals.and_then(|g| hayro_jbig2::Globals::new(g, &settings).ok());
    let page = hayro_jbig2::Page::new_embedded(data, globals.as_ref(), &settings).ok()?;
    let (pw, ph) = (page.width() as usize, page.height() as usize);
    if pw == 0 || ph == 0 || (pw as u64) * (ph as u64) > MAX_PIXELS {
        return None;
    }
    let mut out = Rows::new(pw, ph, false);
    page.decode(&mut out).ok()?;
    Some(fit(out.data, (pw, ph), (width, height), 0xff))
}

/// Rows of `w × h` packed bits cut or padded with `white` bytes to `width × height`.
fn fit(
    data: Vec<u8>,
    (w, h): (usize, usize),
    (width, height): (usize, usize),
    white: u8,
) -> Vec<u8> {
    if (w, h) == (width, height) {
        return data;
    }
    let (src_row, dst_row) = (w.div_ceil(8), width.div_ceil(8));
    let mut out = vec![white; dst_row * height];
    for y in 0..height.min(h) {
        let n = src_row.min(dst_row);
        out[y * dst_row..y * dst_row + n].copy_from_slice(&data[y * src_row..y * src_row + n]);
    }
    out
}

/// Decoded colour picture: RGB samples, alpha, width, height.
pub(crate) type Pixels = (Vec<[u8; 3]>, Vec<u8>, usize, usize);

/// A JPEG 2000 image (§7.4.9) as RGB samples, alpha and its size.
pub(crate) fn jpx(data: &[u8]) -> Option<Pixels> {
    let settings = hayro_jpeg2000::DecodeSettings::default();
    let image = hayro_jpeg2000::Image::new(data, &settings).ok()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_PIXELS {
        return None;
    }
    let channels = usize::from(image.color_space().num_channels());
    let cmyk = matches!(image.color_space(), hayro_jpeg2000::ColorSpace::CMYK);
    let has_alpha = image.has_alpha();
    let mut ctx = hayro_jpeg2000::DecoderContext::default();
    let decoded = image.decode(&mut ctx).ok()?;
    let stride = decoded.components().len();
    if stride == 0 {
        return None;
    }
    let samples = decoded.data_u8();
    let n = w * h;
    if samples.len() < n * stride {
        return None;
    }
    let mut rgb = Vec::with_capacity(n);
    let mut alpha = Vec::with_capacity(n);
    for px in samples.chunks_exact(stride).take(n) {
        let c = |i: usize| px.get(i).copied().unwrap_or(0);
        rgb.push(match (channels.min(stride), cmyk) {
            (4, true) => {
                let v = |i: usize| f32::from(c(i)) / 255.0;
                crate::color::cmyk_to_rgb(v(0), v(1), v(2), v(3))
                    .map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
            }
            (1 | 2, _) => [c(0); 3],
            _ => [c(0), c(1), c(2)],
        });
        alpha.push(if has_alpha { c(stride - 1) } else { 255 });
    }
    Some((rgb, alpha, w, h))
}

/// The `/JBIG2Globals` stream of an image's decode parameters, decoded.
pub(crate) fn jbig2_globals(doc: &Document, dict: &Dictionary) -> Option<Vec<u8>> {
    let resolve = |o: &Object| doc.dereference(o).map_or(o.clone(), |(_, o)| o.clone());
    let parms = resolve(dict.get(b"DecodeParms").or_else(|_| dict.get(b"DP")).ok()?);
    let dicts: Vec<Dictionary> = match parms {
        Object::Dictionary(d) => vec![d],
        Object::Array(a) => a
            .iter()
            .filter_map(|o| resolve(o).as_dict().ok().cloned())
            .collect(),
        _ => Vec::new(),
    };
    dicts.iter().find_map(|d| {
        let id = d.get(b"JBIG2Globals").ok()?.as_reference().ok()?;
        let Object::Stream(s) = doc.get_object(id).ok()? else {
            return None;
        };
        crate::filters::stream_bytes(doc, s).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ccitt_rows_pack_black_runs() {
        let mut r = Rows::new(10, 2, false);
        hayro_ccitt::Decoder::push_row(&mut r, &[2, 4, 9]);
        // Black at 2..4 and 9..10 → bits 0 there (BlackIs1 false), white 1 elsewhere.
        assert_eq!(&r.data[0..2], &[0b1100_1111, 0b1011_1111]);
        let mut j = Rows::new(9, 1, false);
        hayro_jbig2::Decoder::push_pixel_chunk(&mut j, true, 1);
        hayro_jbig2::Decoder::push_pixel(&mut j, false);
        assert_eq!(&j.data[..], &[0, 0xff]);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(jpx(b"not a jpeg 2000 file").is_none());
        assert!(jbig2(&[0xff; 40], None, 8, 8).is_none());
        let p = Dictionary::new();
        let _ = ccitt(&[0x00; 16], &p, 16, 4);
    }
}

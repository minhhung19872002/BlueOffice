//! Picture decoders written from the public format specifications: GIF (GIF89a
//! specification, CompuServe 1990: first frame, LZW, interlacing, transparency),
//! BMP / DIB (Microsoft `BITMAPINFOHEADER` / `BITMAPCOREHEADER` documentation, also
//! [MS-WMF] §2.2.2.9 DeviceIndependentBitmap: 1/4/8/16/24/32 bit, BI_RGB, BI_RLE8,
//! BI_RLE4, BI_BITFIELDS) and TIFF (TIFF Revision 6.0, Adobe 1992: baseline strips,
//! no compression, PackBits, LZW with the horizontal predictor; bilevel, grey,
//! palette, RGB and RGBA).
//!
//! Every decoder returns straight (non-premultiplied) RGBA and never panics on bad
//! input (`None`).

/// Decoded picture: width, height and straight RGBA pixels, row by row.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Largest accepted picture (pixels), to bound memory on hostile input.
const MAX_PIXELS: u64 = 64 * 1024 * 1024;

fn alloc(w: u32, h: u32) -> Option<Vec<u8>> {
    let n = u64::from(w) * u64::from(h);
    if w == 0 || h == 0 || n > MAX_PIXELS {
        return None;
    }
    Some(vec![0; usize::try_from(n * 4).ok()?])
}

/// Container formats handled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gif,
    Bmp,
    Tiff,
}

/// Sniff GIF / BMP / TIFF magic bytes.
pub fn kind(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else if bytes.starts_with(b"BM") && bytes.len() > 26 {
        Some(Kind::Bmp)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(Kind::Tiff)
    } else {
        None
    }
}

/// Decode a GIF, BMP or TIFF picture.
pub fn decode(bytes: &[u8]) -> Option<Rgba> {
    match kind(bytes)? {
        Kind::Gif => decode_gif(bytes),
        Kind::Bmp => decode_bmp(bytes),
        Kind::Tiff => decode_tiff(bytes),
    }
}

fn u16le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn i32le(b: &[u8], i: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

// ------------------------------------------------------------------ GIF

/// Variable-length-code LZW decoder of GIF (LSB-first codes, clear and end codes).
fn gif_lzw(data: &[u8], min_size: u8, max_out: usize) -> Vec<u8> {
    let min = u32::from(min_size.clamp(2, 11));
    let clear = 1u32 << min;
    let eoi = clear + 1;
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut length = vec![0u16; 4096];
    for i in 0..clear {
        suffix[i as usize] = i as u8;
        length[i as usize] = 1;
    }
    let mut out = Vec::with_capacity(max_out);
    let (mut size, mut next) = (min + 1, eoi + 1);
    let mut prev: Option<u32> = None;
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut stack = Vec::new();
    for &byte in data {
        acc |= u32::from(byte) << bits;
        bits += 8;
        while bits >= size {
            let code = acc & ((1 << size) - 1);
            acc >>= size;
            bits -= size;
            if code == clear {
                size = min + 1;
                next = eoi + 1;
                prev = None;
                continue;
            }
            if code == eoi || out.len() >= max_out {
                return out;
            }
            // Expand `code` (or the KwKwK case: prev + first char of prev).
            let (emit, first) = if code < next {
                (code, None)
            } else if let Some(p) = prev {
                (p, Some(()))
            } else {
                return out;
            };
            stack.clear();
            let mut c = emit;
            loop {
                stack.push(suffix[c as usize]);
                if length[c as usize] <= 1 {
                    break;
                }
                c = u32::from(prefix[c as usize]);
            }
            let head = *stack.last().unwrap_or(&0);
            out.extend(stack.iter().rev());
            if first.is_some() {
                out.push(head);
            }
            if let Some(p) = prev {
                if next < 4096 {
                    prefix[next as usize] = p as u16;
                    suffix[next as usize] = head;
                    length[next as usize] = length[p as usize].saturating_add(1);
                    next += 1;
                    if next == (1 << size) && size < 12 {
                        size += 1;
                    }
                }
            }
            prev = Some(code);
        }
    }
    out
}

/// First frame of a GIF composited on a transparent canvas.
pub fn decode_gif(b: &[u8]) -> Option<Rgba> {
    let w = u32::from(u16le(b, 6)?);
    let h = u32::from(u16le(b, 8)?);
    let flags = *b.get(10)?;
    let mut i = 13;
    let mut global: &[u8] = &[];
    if flags & 0x80 != 0 {
        let n = 3 * (2usize << (flags & 7));
        global = b.get(i..i + n)?;
        i += n;
    }
    let mut px = alloc(w, h)?;
    let mut transparent: Option<u8> = None;
    loop {
        match *b.get(i)? {
            // Extension: graphic control (0xF9) gives the transparent index.
            0x21 => {
                let label = *b.get(i + 1)?;
                i += 2;
                if label == 0xF9 && *b.get(i)? >= 4 && b.get(i + 1)? & 1 != 0 {
                    transparent = Some(*b.get(i + 4)?);
                }
                loop {
                    let n = usize::from(*b.get(i)?);
                    i += 1 + n;
                    if n == 0 {
                        break;
                    }
                }
            }
            // Image descriptor.
            0x2C => {
                let (fx, fy) = (u32::from(u16le(b, i + 1)?), u32::from(u16le(b, i + 3)?));
                let (fw, fh) = (u32::from(u16le(b, i + 5)?), u32::from(u16le(b, i + 7)?));
                let f = *b.get(i + 9)?;
                i += 10;
                let mut palette = global;
                if f & 0x80 != 0 {
                    let n = 3 * (2usize << (f & 7));
                    palette = b.get(i..i + n)?;
                    i += n;
                }
                let min = *b.get(i)?;
                i += 1;
                let mut data = Vec::new();
                loop {
                    let n = usize::from(*b.get(i)?);
                    i += 1;
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(b.get(i..i + n)?);
                    i += n;
                }
                let total = (fw as usize).checked_mul(fh as usize)?;
                let idx = gif_lzw(&data, min, total);
                // Interlaced rows come in passes 0,8,…; 4,12,…; 2,6,…; 1,3,…
                let rows: Vec<u32> = if f & 0x40 != 0 {
                    [(0, 8), (4, 8), (2, 4), (1, 2)]
                        .iter()
                        .flat_map(|&(s, st)| (s..fh).step_by(st))
                        .collect()
                } else {
                    (0..fh).collect()
                };
                for (k, &v) in idx.iter().enumerate() {
                    let (rx, ri) = ((k as u32) % fw.max(1), (k as u32) / fw.max(1));
                    let Some(&ry) = rows.get(ri as usize) else {
                        break;
                    };
                    let (x, y) = (fx + rx, fy + ry);
                    if x >= w || y >= h || Some(v) == transparent {
                        continue;
                    }
                    let c = usize::from(v) * 3;
                    if let Some(rgb) = palette.get(c..c + 3) {
                        let o = ((y * w + x) * 4) as usize;
                        px[o..o + 3].copy_from_slice(rgb);
                        px[o + 3] = 255;
                    }
                }
                return Some(Rgba {
                    width: w,
                    height: h,
                    pixels: px,
                });
            }
            _ => return None,
        }
    }
}

// ------------------------------------------------------------------ BMP

/// A BMP file (`BITMAPFILEHEADER` + DIB).
pub fn decode_bmp(b: &[u8]) -> Option<Rgba> {
    let off = u32le(b, 10)? as usize;
    decode_dib_at(b.get(14..)?, off.checked_sub(14))
}

/// A packed DIB: `BITMAPINFOHEADER` / `BITMAPCOREHEADER`, colour table and bits
/// (the form embedded in metafile records).
pub fn decode_dib(b: &[u8]) -> Option<Rgba> {
    decode_dib_at(b, None)
}

/// Bit masks of a 16/32-bit DIB (`BI_BITFIELDS`, or the 5-5-5 / 8-8-8 defaults).
fn mask_value(v: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = mask >> shift;
    (((v & mask) >> shift) * 255 / max.max(1)) as u8
}

/// DIB with the pixel bits at `bits_at` (from the start of the header) when known.
pub fn decode_dib_at(b: &[u8], bits_at: Option<usize>) -> Option<Rgba> {
    let hs = u32le(b, 0)? as usize;
    let (w, h, bpp, comp, used, pal_entry) = if hs == 12 {
        (
            i32::from(u16le(b, 4)?),
            i32::from(u16le(b, 6)?),
            u16le(b, 10)?,
            0,
            0,
            3usize,
        )
    } else {
        (
            i32le(b, 4)?,
            i32le(b, 8)?,
            u16le(b, 14)?,
            u32le(b, 16)?,
            u32le(b, 32)?,
            4usize,
        )
    };
    let top_down = h < 0;
    let (w, h) = (w.unsigned_abs(), h.unsigned_abs());
    let mut masks = match bpp {
        16 => [0x7C00, 0x03E0, 0x001F, 0],
        _ => [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0],
    };
    let mut table_at = hs;
    if comp == 3 {
        // BI_BITFIELDS: three masks after a 40-byte header (inside larger headers).
        let at = if hs == 40 { hs } else { 40 };
        for (k, m) in masks.iter_mut().take(3).enumerate() {
            *m = u32le(b, at + k * 4)?;
        }
        if hs >= 56 {
            masks[3] = u32le(b, 52)?;
        }
        if hs == 40 {
            table_at += 12;
        }
    }
    let colors = if bpp <= 8 {
        if used > 0 {
            (used as usize).min(256)
        } else {
            1usize << bpp
        }
    } else {
        used as usize
    };
    let table = b.get(table_at..table_at + colors * pal_entry)?;
    let palette: Vec<[u8; 3]> = table
        .chunks(pal_entry)
        .map(|c| [c[2], c[1], c[0]])
        .collect();
    let bits_at = bits_at.unwrap_or(table_at + colors * pal_entry);
    let bits = b.get(bits_at..)?;
    let mut px = alloc(w, h)?;
    let put = |px: &mut [u8], x: u32, row: u32, rgba: [u8; 4]| {
        if x >= w || row >= h {
            return;
        }
        let y = if top_down { row } else { h - 1 - row };
        let o = ((y * w + x) * 4) as usize;
        px[o..o + 4].copy_from_slice(&rgba);
    };
    let pal = |i: usize| -> [u8; 4] {
        let c = palette.get(i).copied().unwrap_or([0, 0, 0]);
        [c[0], c[1], c[2], 255]
    };
    match comp {
        // BI_RLE8 (1) / BI_RLE4 (2): runs and absolute blocks, escape codes 0/1/2.
        1 | 2 => {
            let four = comp == 2;
            let (mut x, mut row, mut i) = (0u32, 0u32, 0usize);
            while i + 1 < bits.len() {
                let (n, v) = (bits[i], bits[i + 1]);
                i += 2;
                if n > 0 {
                    for k in 0..u32::from(n) {
                        let c = if four {
                            if k % 2 == 0 {
                                v >> 4
                            } else {
                                v & 15
                            }
                        } else {
                            v
                        };
                        put(&mut px, x, row, pal(usize::from(c)));
                        x += 1;
                    }
                    continue;
                }
                match v {
                    0 => {
                        x = 0;
                        row += 1;
                    }
                    1 => break,
                    2 => {
                        x += u32::from(*bits.get(i)?);
                        row += u32::from(*bits.get(i + 1)?);
                        i += 2;
                    }
                    count => {
                        let count = usize::from(count);
                        let bytes = if four { count.div_ceil(2) } else { count };
                        let data = bits.get(i..i + bytes)?;
                        for k in 0..count {
                            let c = if four {
                                let b = data[k / 2];
                                if k % 2 == 0 {
                                    b >> 4
                                } else {
                                    b & 15
                                }
                            } else {
                                data[k]
                            };
                            put(&mut px, x, row, pal(usize::from(c)));
                            x += 1;
                        }
                        i += bytes + bytes % 2;
                    }
                }
            }
        }
        0 | 3 => {
            let stride = ((u64::from(w) * u64::from(bpp)).div_ceil(32) * 4) as usize;
            for row in 0..h {
                let line = bits.get(row as usize * stride..(row as usize + 1) * stride)?;
                for x in 0..w {
                    let xi = x as usize;
                    let rgba = match bpp {
                        1 | 2 | 4 | 8 => {
                            let per = 8 / usize::from(bpp);
                            let byte = line[xi / per];
                            let shift = 8 - usize::from(bpp) * (xi % per + 1);
                            pal(usize::from((byte >> shift) & ((1 << bpp) - 1)))
                        }
                        16 => {
                            let v = u32::from(u16::from_le_bytes([line[xi * 2], line[xi * 2 + 1]]));
                            [
                                mask_value(v, masks[0]),
                                mask_value(v, masks[1]),
                                mask_value(v, masks[2]),
                                255,
                            ]
                        }
                        24 => [line[xi * 3 + 2], line[xi * 3 + 1], line[xi * 3], 255],
                        32 => {
                            let v = u32::from_le_bytes(line[xi * 4..xi * 4 + 4].try_into().ok()?);
                            [
                                mask_value(v, masks[0]),
                                mask_value(v, masks[1]),
                                mask_value(v, masks[2]),
                                if masks[3] == 0 {
                                    255
                                } else {
                                    mask_value(v, masks[3])
                                },
                            ]
                        }
                        _ => return None,
                    };
                    put(&mut px, x, row, rgba);
                }
            }
        }
        _ => return None,
    }
    Some(Rgba {
        width: w,
        height: h,
        pixels: px,
    })
}

// ------------------------------------------------------------------ TIFF

struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, i: usize) -> Option<u16> {
        let s: [u8; 2] = self.b.get(i..i + 2)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(s)
        } else {
            u16::from_be_bytes(s)
        })
    }
    fn u32(&self, i: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(i..i + 4)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    }
    /// Values of an IFD entry at `e` (types BYTE 1, SHORT 3, LONG 4).
    fn values(&self, e: usize) -> Option<Vec<u32>> {
        let ty = self.u16(e + 2)?;
        let n = self.u32(e + 4)? as usize;
        let size = match ty {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 => 4,
            _ => return None,
        };
        if n > 1 << 20 {
            return None;
        }
        let at = if n * size <= 4 {
            e + 8
        } else {
            self.u32(e + 8)? as usize
        };
        (0..n)
            .map(|k| match size {
                1 => self.b.get(at + k).map(|v| u32::from(*v)),
                2 => self.u16(at + k * 2).map(u32::from),
                _ => self.u32(at + k * 4),
            })
            .collect()
    }
}

/// TIFF LZW (MSB-first codes, code 256 clear, 257 end, "early change").
fn tiff_lzw(data: &[u8], max_out: usize) -> Vec<u8> {
    let mut table: Vec<Vec<u8>> = (0..=255u8).map(|v| vec![v]).collect();
    table.push(Vec::new());
    table.push(Vec::new());
    let mut out = Vec::with_capacity(max_out);
    let (mut acc, mut bits, mut size) = (0u64, 0u32, 9u32);
    let mut prev: Option<usize> = None;
    for &byte in data {
        acc = (acc << 8) | u64::from(byte);
        bits += 8;
        while bits >= size {
            let code = ((acc >> (bits - size)) & ((1 << size) - 1)) as usize;
            bits -= size;
            if code == 256 {
                table.truncate(258);
                size = 9;
                prev = None;
                continue;
            }
            if code == 257 || out.len() >= max_out {
                return out;
            }
            let entry = if code < table.len() {
                table[code].clone()
            } else if let Some(p) = prev {
                let mut e = table[p].clone();
                e.push(table[p][0]);
                e
            } else {
                return out;
            };
            if let Some(p) = prev {
                if table.len() < 4096 {
                    let mut n = table[p].clone();
                    n.push(entry[0]);
                    table.push(n);
                }
            }
            out.extend_from_slice(&entry);
            prev = Some(code);
            if table.len() + 1 >= (1 << size) && size < 12 {
                size += 1;
            }
        }
    }
    out
}

/// PackBits (TIFF 6.0 §9): run-length bytes.
fn packbits(data: &[u8], max_out: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(max_out);
    let mut i = 0;
    while i < data.len() && out.len() < max_out {
        let n = data[i] as i8;
        i += 1;
        if n >= 0 {
            let k = n as usize + 1;
            let Some(s) = data.get(i..i + k) else { break };
            out.extend_from_slice(s);
            i += k;
        } else if n != -128 {
            let Some(&v) = data.get(i) else { break };
            out.extend(std::iter::repeat_n(v, (1 - i32::from(n)) as usize));
            i += 1;
        }
    }
    out
}

/// First image of a baseline TIFF.
pub fn decode_tiff(b: &[u8]) -> Option<Rgba> {
    let t = Tiff {
        b,
        le: b.starts_with(b"II"),
    };
    let ifd = t.u32(4)? as usize;
    let n = usize::from(t.u16(ifd)?);
    let mut tag = std::collections::HashMap::new();
    for k in 0..n.min(512) {
        let e = ifd + 2 + k * 12;
        if let Some(v) = t.values(e) {
            tag.insert(t.u16(e)?, v);
        }
    }
    let one = |id: u16, d: u32| tag.get(&id).and_then(|v| v.first().copied()).unwrap_or(d);
    let (w, h) = (one(256, 0), one(257, 0));
    let bps = one(258, 1);
    let comp = one(259, 1);
    let photo = one(262, 1);
    let spp = one(277, 1).clamp(1, 4);
    let rps = one(278, h).max(1);
    let predictor = one(317, 1);
    let offsets = tag.get(&273)?;
    let counts = tag.get(&279)?;
    if one(284, 1) != 1 || !(bps == 1 || bps == 4 || bps == 8) {
        return None;
    }
    let row_bytes = (w as usize * spp as usize * bps as usize).div_ceil(8);
    let mut data = Vec::new();
    for (k, (&o, &c)) in offsets.iter().zip(counts).enumerate() {
        let strip = b.get(o as usize..(o as usize).checked_add(c as usize)?)?;
        let rows = rps.min(h.saturating_sub(k as u32 * rps)) as usize;
        let want = rows * row_bytes;
        let mut s = match comp {
            1 => strip.to_vec(),
            5 => tiff_lzw(strip, want),
            32773 => packbits(strip, want),
            _ => return None,
        };
        s.resize(want, 0);
        if predictor == 2 && bps == 8 {
            // Horizontal differencing (§14).
            let spp = spp as usize;
            for row in s.chunks_mut(row_bytes) {
                for i in spp..row.len() {
                    row[i] = row[i].wrapping_add(row[i - spp]);
                }
            }
        }
        data.extend_from_slice(&s);
    }
    let colormap = tag.get(&320);
    let mut px = alloc(w, h)?;
    for y in 0..h as usize {
        let Some(row) = data.get(y * row_bytes..(y + 1) * row_bytes) else {
            break;
        };
        for x in 0..w as usize {
            let sample = |s: usize| -> u8 {
                let bit = (x * spp as usize + s) * bps as usize;
                let byte = row.get(bit / 8).copied().unwrap_or(0);
                match bps {
                    1 => ((byte >> (7 - bit % 8)) & 1) * 255,
                    4 => ((byte >> (4 - bit % 8)) & 15) * 17,
                    _ => byte,
                }
            };
            let rgba = match photo {
                0 => {
                    let v = 255 - sample(0);
                    [v, v, v, 255]
                }
                1 => {
                    let v = sample(0);
                    [v, v, v, 255]
                }
                2 => [
                    sample(0),
                    sample(1.min(spp as usize - 1)),
                    sample(2.min(spp as usize - 1)),
                    if spp >= 4 { sample(3) } else { 255 },
                ],
                3 => {
                    let bit = x * bps as usize;
                    let byte = row.get(bit / 8).copied().unwrap_or(0);
                    let i = match bps {
                        4 => usize::from((byte >> (4 - bit % 8)) & 15),
                        1 => usize::from((byte >> (7 - bit % 8)) & 1),
                        _ => usize::from(byte),
                    };
                    let cm = colormap?;
                    let k = cm.len() / 3;
                    let g = |c: usize| (cm.get(c * k + i).copied().unwrap_or(0) >> 8) as u8;
                    [g(0), g(1), g(2), 255]
                }
                _ => return None,
            };
            let o = (y * w as usize + x) * 4;
            px[o..o + 4].copy_from_slice(&rgba);
        }
    }
    Some(Rgba {
        width: w,
        height: h,
        pixels: px,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_24_and_8_bit() {
        // 2×2, 24-bit, bottom-up: rows are padded to 8 bytes.
        let mut b = b"BM".to_vec();
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&54u32.to_le_bytes());
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&2i32.to_le_bytes());
        b.extend_from_slice(&2i32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        b.extend_from_slice(&[0; 24]);
        // bottom row: blue, green; top row: red, white (BGR order)
        b.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0]);
        b.extend_from_slice(&[0, 0, 255, 255, 255, 255, 0, 0]);
        let r = decode(&b).unwrap();
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(&r.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&r.pixels[8..12], &[0, 0, 255, 255]);
        assert!(decode(&b[..30]).is_none());
    }

    #[test]
    fn gif_lzw_round_trip_of_a_known_stream() {
        // 2×1 image, indices [1, 0], min code size 2: codes clear(4) 1 0 eoi(5) at 3 bits.
        // Bits LSB first: 100 001 000 101 → 0b00_001_100 = 0x0C, 0b0000_1010 … = 0x0A
        let idx = gif_lzw(&[0x0C, 0x0A], 2, 16);
        assert_eq!(idx, vec![1, 0]);
    }

    #[test]
    fn packbits_runs() {
        assert_eq!(packbits(&[0xFE, 7, 1, 1, 2], 10), vec![7, 7, 7, 1, 2]);
    }
}

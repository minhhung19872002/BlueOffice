//! Animated GIF writer and reader, written from the GIF89a specification
//! (CompuServe, 1990): logical screen descriptor (§18), graphic control extension
//! (§23: delay in 1/100 s, disposal), image descriptor with a local colour table
//! (§20, §21), variable-length-code LZW (Appendix F) and the application extension
//! block (§26) carrying the looping sub-block that viewers commonly honour
//! (`NETSCAPE2.0`, sub-block 1 = loop count, 0 = forever).
//!
//! Frames are true-colour RGBA; each one gets its own palette of up to 256 colours by
//! median cut over a 5-bit-per-channel histogram (exact when the frame has at most 256
//! colours), mapped with optional Floyd–Steinberg error diffusion.

use crate::codecs::gif_lzw;
use std::collections::HashMap;

/// Options of a GIF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GifOptions {
    /// Loop count of the animation: `Some(0)` = forever, `Some(n)` = n more times,
    /// `None` = play once (no looping block).
    pub repeat: Option<u16>,
    /// Floyd–Steinberg dithering when a frame has more than 256 colours.
    pub dither: bool,
}

impl Default for GifOptions {
    fn default() -> Self {
        Self {
            repeat: Some(0),
            dither: true,
        }
    }
}

/// Errors of the GIF writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GifError {
    /// Width / height of zero or above 65 535.
    InvalidSize(u32, u32),
    /// A frame whose pixel buffer is not `width × height × 4` bytes.
    FrameSize(usize),
}

impl std::fmt::Display for GifError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GifError::InvalidSize(w, h) => write!(f, "invalid GIF size {w}x{h}"),
            GifError::FrameSize(n) => write!(f, "frame has {n} bytes, wrong for the GIF size"),
        }
    }
}

impl std::error::Error for GifError {}

/// Writes an animated GIF frame by frame.
pub struct GifEncoder {
    width: u16,
    height: u16,
    dither: bool,
    out: Vec<u8>,
    frames: usize,
}

impl GifEncoder {
    /// Start a GIF of `width × height` pixels: header, screen descriptor and looping block.
    pub fn new(width: u32, height: u32, opts: GifOptions) -> Result<Self, GifError> {
        let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
            return Err(GifError::InvalidSize(width, height));
        };
        if w == 0 || h == 0 {
            return Err(GifError::InvalidSize(width, height));
        }
        let mut out = Vec::with_capacity(1 << 16);
        out.extend_from_slice(b"GIF89a");
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        // No global colour table (every frame has its own), background 0, no aspect ratio.
        out.extend_from_slice(&[0, 0, 0]);
        if let Some(n) = opts.repeat {
            out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
            out.extend_from_slice(b"NETSCAPE2.0");
            out.extend_from_slice(&[3, 1]);
            out.extend_from_slice(&n.to_le_bytes());
            out.push(0);
        }
        Ok(Self {
            width: w,
            height: h,
            dither: opts.dither,
            out,
            frames: 0,
        })
    }

    /// Frames written so far.
    pub fn frame_count(&self) -> usize {
        self.frames
    }

    /// Append a full-size frame (straight RGBA, alpha ignored) shown for `delay_cs`
    /// hundredths of a second.
    pub fn add_frame(&mut self, rgba: &[u8], delay_cs: u16) -> Result<(), GifError> {
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        if rgba.len() != w * h * 4 {
            return Err(GifError::FrameSize(rgba.len()));
        }
        let (palette, indices) = quantize(rgba, w, h, self.dither);
        // Table size 2^(bits) with bits ≥ 1 (§21); LZW minimum code size ≥ 2 (Appendix F).
        let bits = palette_bits(palette.len());
        let o = &mut self.out;
        // Graphic control extension: disposal 1 (leave in place), no transparency.
        o.extend_from_slice(&[0x21, 0xF9, 4, 1 << 2]);
        o.extend_from_slice(&delay_cs.to_le_bytes());
        o.extend_from_slice(&[0, 0]);
        // Image descriptor at (0, 0), local colour table, not interlaced.
        o.push(0x2C);
        o.extend_from_slice(&[0, 0, 0, 0]);
        o.extend_from_slice(&self.width.to_le_bytes());
        o.extend_from_slice(&self.height.to_le_bytes());
        o.push(0x80 | (bits - 1));
        for k in 0..(1usize << bits) {
            o.extend_from_slice(&palette.get(k).copied().unwrap_or([0, 0, 0]));
        }
        let min = bits.max(2);
        o.push(min);
        let data = lzw_encode(&indices, min);
        for chunk in data.chunks(255) {
            o.push(chunk.len() as u8);
            o.extend_from_slice(chunk);
        }
        o.push(0);
        self.frames += 1;
        Ok(())
    }

    /// Close the file (trailer) and return its bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.out.push(0x3B);
        self.out
    }
}

/// Bits of a colour table holding `n` colours (1..=8).
fn palette_bits(n: usize) -> u8 {
    let mut bits = 1u8;
    while (1usize << bits) < n && bits < 8 {
        bits += 1;
    }
    bits
}

// ------------------------------------------------------------------ LZW

/// GIF LZW (Appendix F): LSB-first variable-length codes from `min + 1` to 12 bits,
/// a clear code first and whenever the table is full, the end code last.
pub fn lzw_encode(indices: &[u8], min: u8) -> Vec<u8> {
    let min = u32::from(min.clamp(2, 8));
    let clear = 1u32 << min;
    let eoi = clear + 1;
    let mut w = BitWriter::default();
    let mut width = min + 1;
    let mut hi = eoi;
    let mut overflow = 1u32 << width;
    let mut table: HashMap<u32, u32> = HashMap::with_capacity(4096);
    w.put(clear, width);
    let Some((&first, rest)) = indices.split_first() else {
        w.put(eoi, width);
        return w.finish();
    };
    let mut saved = u32::from(first) & (clear - 1);
    for &b in rest {
        let lit = u32::from(b) & (clear - 1);
        let key = (saved << 8) | lit;
        if let Some(&code) = table.get(&key) {
            saved = code;
            continue;
        }
        w.put(saved, width);
        saved = lit;
        hi += 1;
        if hi == overflow {
            width += 1;
            overflow <<= 1;
        }
        if hi == 4095 {
            // Table full: clear and start again (the code 4095 is never assigned).
            w.put(clear, width.min(12));
            width = min + 1;
            hi = eoi;
            overflow = 1 << width;
            table.clear();
            continue;
        }
        table.insert(key, hi);
    }
    w.put(saved, width);
    hi += 1;
    if hi == overflow && width < 12 {
        width += 1;
    }
    w.put(eoi, width.min(12));
    w.finish()
}

#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    bits: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, width: u32) {
        self.acc |= code << self.bits;
        self.bits += width;
        while self.bits >= 8 {
            self.out.push((self.acc & 0xFF) as u8);
            self.acc >>= 8;
            self.bits -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.out.push((self.acc & 0xFF) as u8);
        }
        self.out
    }
}

// ------------------------------------------------------------------ quantization

/// Palette (≤ 256 colours) and one index per pixel of an RGBA frame.
pub fn quantize(rgba: &[u8], w: usize, h: usize, dither: bool) -> (Vec<[u8; 3]>, Vec<u8>) {
    let n = w * h;
    // Exact palette when the frame has few colours (flat slides, text).
    let mut exact: HashMap<[u8; 3], u8> = HashMap::new();
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut few = true;
    for px in rgba.chunks_exact(4).take(n) {
        let c = [px[0], px[1], px[2]];
        if let std::collections::hash_map::Entry::Vacant(e) = exact.entry(c) {
            if palette.len() == 256 {
                few = false;
                break;
            }
            e.insert(palette.len() as u8);
            palette.push(c);
        }
    }
    if few {
        let idx = rgba
            .chunks_exact(4)
            .take(n)
            .map(|px| exact.get(&[px[0], px[1], px[2]]).copied().unwrap_or(0))
            .collect();
        if palette.is_empty() {
            palette.push([0, 0, 0]);
        }
        return (palette, idx);
    }
    let palette = median_cut(rgba, 256);
    let mut near = Nearest::new(&palette);
    let mut idx = vec![0u8; n];
    if !dither {
        for (i, px) in rgba.chunks_exact(4).take(n).enumerate() {
            idx[i] = near.find(i32::from(px[0]), i32::from(px[1]), i32::from(px[2]));
        }
        return (palette, idx);
    }
    // Floyd–Steinberg: 7/16 right, 3/16 below left, 5/16 below, 1/16 below right.
    let mut cur = vec![[0i32; 3]; w + 2];
    let mut next = vec![[0i32; 3]; w + 2];
    for y in 0..h {
        next.fill([0; 3]);
        for x in 0..w {
            let o = (y * w + x) * 4;
            let mut c = [0i32; 3];
            for k in 0..3 {
                c[k] = (i32::from(rgba[o + k]) + cur[x + 1][k] / 16).clamp(0, 255);
            }
            let i = near.find(c[0], c[1], c[2]);
            idx[y * w + x] = i;
            let p = palette[usize::from(i)];
            for k in 0..3 {
                let err = c[k] - i32::from(p[k]);
                cur[x + 2][k] += err * 7;
                next[x][k] += err * 3;
                next[x + 1][k] += err * 5;
                next[x + 2][k] += err;
            }
        }
        std::mem::swap(&mut cur, &mut next);
    }
    (palette, idx)
}

/// Histogram bin of a colour: 5 bits per channel.
fn bin(r: i32, g: i32, b: i32) -> usize {
    (((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3)) as usize
}

/// Palette by median cut: boxes of the 5-bit histogram split along their longest side at
/// the median pixel, the box with the most pixels × widest side first; colour = mean.
fn median_cut(rgba: &[u8], max: usize) -> Vec<[u8; 3]> {
    #[derive(Clone, Copy, Default)]
    struct Bin {
        n: u64,
        sum: [u64; 3],
    }
    let mut hist = vec![Bin::default(); 1 << 15];
    for px in rgba.chunks_exact(4) {
        let b = &mut hist[bin(i32::from(px[0]), i32::from(px[1]), i32::from(px[2]))];
        b.n += 1;
        for (sum, &v) in b.sum.iter_mut().zip(px) {
            *sum += u64::from(v);
        }
    }
    let mut boxes: Vec<Vec<usize>> = vec![(0..hist.len()).filter(|&i| hist[i].n > 0).collect()];
    let coord = |i: usize, k: usize| (i >> (10 - 5 * k)) & 31;
    let extent = |b: &[usize]| -> (usize, usize) {
        // (widest axis, its width)
        let mut best = (0, 0);
        for k in 0..3 {
            let lo = b.iter().map(|&i| coord(i, k)).min().unwrap_or(0);
            let hi = b.iter().map(|&i| coord(i, k)).max().unwrap_or(0);
            if hi - lo > best.1 {
                best = (k, hi - lo);
            }
        }
        best
    };
    while boxes.len() < max {
        let mut pick: Option<(usize, u64)> = None;
        for (j, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            let n: u64 = b.iter().map(|&i| hist[i].n).sum();
            let score = n * (extent(b).1 as u64 + 1);
            if pick.is_none_or(|(_, s)| score > s) {
                pick = Some((j, score));
            }
        }
        let Some((j, _)) = pick else {
            break;
        };
        let mut b = std::mem::take(&mut boxes[j]);
        let (axis, _) = extent(&b);
        b.sort_unstable_by_key(|&i| coord(i, axis));
        let total: u64 = b.iter().map(|&i| hist[i].n).sum();
        let mut acc = 0;
        let mut cut = 1;
        for (k, &i) in b.iter().enumerate() {
            acc += hist[i].n;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let tail = b.split_off(cut);
        boxes[j] = b;
        boxes.push(tail);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n: u64 = b.iter().map(|&i| hist[i].n).sum::<u64>().max(1);
            let mut c = [0u8; 3];
            for (k, v) in c.iter_mut().enumerate() {
                *v = (b.iter().map(|&i| hist[i].sum[k]).sum::<u64>() / n) as u8;
            }
            c
        })
        .collect()
}

/// Nearest palette colour, cached per histogram bin.
struct Nearest<'a> {
    palette: &'a [[u8; 3]],
    cache: Vec<i16>,
}

impl<'a> Nearest<'a> {
    fn new(palette: &'a [[u8; 3]]) -> Self {
        Self {
            palette,
            cache: vec![-1; 1 << 15],
        }
    }

    fn find(&mut self, r: i32, g: i32, b: i32) -> u8 {
        let key = bin(r, g, b);
        if let Ok(i) = u8::try_from(self.cache[key]) {
            return i;
        }
        // Centre of the bin.
        let (cr, cg, cb) = ((r & !7) + 4, (g & !7) + 4, (b & !7) + 4);
        let mut best = (0usize, i32::MAX);
        for (i, p) in self.palette.iter().enumerate() {
            let d = (i32::from(p[0]) - cr).pow(2)
                + (i32::from(p[1]) - cg).pow(2)
                + (i32::from(p[2]) - cb).pow(2);
            if d < best.1 {
                best = (i, d);
            }
        }
        self.cache[key] = best.0 as i16;
        best.0 as u8
    }
}

// ------------------------------------------------------------------ reader

/// One decoded frame of an animated GIF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifFrame {
    /// The whole canvas after the frame, straight RGBA.
    pub rgba: Vec<u8>,
    /// Delay in 1/100 s.
    pub delay_cs: u16,
    /// Colour table the frame used.
    pub palette: Vec<[u8; 3]>,
}

/// A decoded animated GIF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifAnimation {
    /// Canvas width.
    pub width: u32,
    /// Canvas height.
    pub height: u32,
    /// Loop count of the looping block (`None` = no block).
    pub repeat: Option<u16>,
    /// The frames in order.
    pub frames: Vec<GifFrame>,
}

/// Decode every frame of a GIF (`None` on malformed input). Disposal methods other than
/// "leave in place" are treated as such; interlaced frames are supported.
pub fn decode_animation(b: &[u8]) -> Option<GifAnimation> {
    if !(b.starts_with(b"GIF89a") || b.starts_with(b"GIF87a")) {
        return None;
    }
    let rd = |i: usize| -> Option<u16> { Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?])) };
    let (w, h) = (u32::from(rd(6)?), u32::from(rd(8)?));
    if w == 0 || h == 0 {
        return None;
    }
    let flags = *b.get(10)?;
    let mut i = 13;
    let mut global: Vec<[u8; 3]> = Vec::new();
    if flags & 0x80 != 0 {
        let n = 2usize << (flags & 7);
        global = b
            .get(i..i + 3 * n)?
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        i += 3 * n;
    }
    let mut canvas = vec![0u8; (w * h * 4) as usize];
    let mut out = GifAnimation {
        width: w,
        height: h,
        repeat: None,
        frames: Vec::new(),
    };
    let (mut delay, mut transparent) = (0u16, None::<u8>);
    let sub_blocks = |mut i: usize| -> Option<(Vec<u8>, usize)> {
        let mut data = Vec::new();
        loop {
            let n = usize::from(*b.get(i)?);
            i += 1;
            if n == 0 {
                return Some((data, i));
            }
            data.extend_from_slice(b.get(i..i + n)?);
            i += n;
        }
    };
    loop {
        match *b.get(i)? {
            0x3B => return Some(out),
            0x21 => {
                let label = *b.get(i + 1)?;
                if label == 0xFF && b.get(i + 3..i + 14) == Some(b"NETSCAPE2.0") {
                    let (data, j) = sub_blocks(i + 14)?;
                    if data.first() == Some(&1) && data.len() >= 3 {
                        out.repeat = Some(u16::from_le_bytes([data[1], data[2]]));
                    }
                    i = j;
                    continue;
                }
                let (data, j) = sub_blocks(i + 2)?;
                if label == 0xF9 && data.len() >= 4 {
                    delay = u16::from_le_bytes([data[1], data[2]]);
                    transparent = (data[0] & 1 != 0).then_some(data[3]);
                }
                i = j;
            }
            0x2C => {
                let (fx, fy) = (u32::from(rd(i + 1)?), u32::from(rd(i + 3)?));
                let (fw, fh) = (u32::from(rd(i + 5)?), u32::from(rd(i + 7)?));
                let f = *b.get(i + 9)?;
                i += 10;
                let mut palette = global.clone();
                if f & 0x80 != 0 {
                    let n = 2usize << (f & 7);
                    palette = b
                        .get(i..i + 3 * n)?
                        .chunks_exact(3)
                        .map(|c| [c[0], c[1], c[2]])
                        .collect();
                    i += 3 * n;
                }
                let min = *b.get(i)?;
                let (data, j) = sub_blocks(i + 1)?;
                i = j;
                let total = (fw as usize).checked_mul(fh as usize)?;
                let idx = gif_lzw(&data, min, total);
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
                    if let Some(rgb) = palette.get(usize::from(v)) {
                        let o = ((y * w + x) * 4) as usize;
                        canvas[o..o + 3].copy_from_slice(rgb);
                        canvas[o + 3] = 255;
                    }
                }
                out.frames.push(GifFrame {
                    rgba: canvas.clone(),
                    delay_cs: delay,
                    palette,
                });
                delay = 0;
                transparent = None;
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let c = f(x, y);
                v.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        v
    }

    #[test]
    fn lzw_round_trips_through_the_reader() {
        // Long, repetitive input fills the 12-bit table several times (clear codes).
        let data: Vec<u8> = (0..200_000u32)
            .map(|i| ((i * 7 + i / 13) % 251) as u8)
            .collect();
        let enc = lzw_encode(&data, 8);
        assert_eq!(gif_lzw(&enc, 8, data.len()), data);
        for min in 2..=8u8 {
            let mask = (1u32 << min) - 1;
            let d: Vec<u8> = (0..5000u32).map(|i| ((i * i / 7) & mask) as u8).collect();
            assert_eq!(gif_lzw(&lzw_encode(&d, min), min, d.len()), d, "min {min}");
        }
        assert_eq!(gif_lzw(&lzw_encode(&[], 2), 2, 10), Vec::<u8>::new());
    }

    #[test]
    fn few_colours_round_trip_exactly_with_delays_and_loop() {
        let (w, h) = (37, 21);
        let a = frame(w, h, |x, y| {
            if (x / 4 + y / 3) % 2 == 0 {
                [200, 10, 30]
            } else {
                [5, 90, 250]
            }
        });
        let b = frame(w, h, |x, _| [(x * 6) as u8, 255 - (x * 6) as u8, 7]);
        let mut enc = GifEncoder::new(w as u32, h as u32, GifOptions::default()).unwrap();
        enc.add_frame(&a, 7).unwrap();
        enc.add_frame(&b, 250).unwrap();
        assert_eq!(enc.frame_count(), 2);
        let bytes = enc.finish();
        assert!(bytes.starts_with(b"GIF89a"));
        assert_eq!(bytes.last(), Some(&0x3B));
        // The first-frame reader of `codecs` agrees too.
        assert_eq!(crate::codecs::decode_gif(&bytes).unwrap().pixels, a);
        let anim = decode_animation(&bytes).unwrap();
        assert_eq!((anim.width, anim.height, anim.repeat), (37, 21, Some(0)));
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].rgba, a);
        assert_eq!(anim.frames[1].rgba, b);
        assert_eq!(anim.frames[0].delay_cs, 7);
        assert_eq!(anim.frames[1].delay_cs, 250);
        // Two colours → a 2-entry table; 37 colours → 64 entries, first ones as seen.
        assert_eq!(anim.frames[0].palette, vec![[200, 10, 30], [5, 90, 250]]);
        assert_eq!(anim.frames[1].palette.len(), 64);
        assert_eq!(anim.frames[1].palette[0], [0, 255, 7]);
    }

    #[test]
    fn no_loop_block_when_playing_once() {
        let f = frame(4, 4, |_, _| [1, 2, 3]);
        let mut enc = GifEncoder::new(
            4,
            4,
            GifOptions {
                repeat: None,
                dither: false,
            },
        )
        .unwrap();
        enc.add_frame(&f, 10).unwrap();
        let bytes = enc.finish();
        assert!(!bytes.windows(11).any(|w| w == b"NETSCAPE2.0"));
        assert_eq!(decode_animation(&bytes).unwrap().repeat, None);
        let mut enc = GifEncoder::new(
            4,
            4,
            GifOptions {
                repeat: Some(3),
                dither: false,
            },
        )
        .unwrap();
        enc.add_frame(&f, 10).unwrap();
        assert_eq!(decode_animation(&enc.finish()).unwrap().repeat, Some(3));
    }

    #[test]
    fn photo_like_frames_are_quantized_close_to_the_source() {
        let (w, h) = (64, 48);
        let src = frame(w, h, |x, y| {
            [(x * 4) as u8, (y * 5) as u8, ((x + y) * 2) as u8]
        });
        for dither in [false, true] {
            let mut enc = GifEncoder::new(
                w as u32,
                h as u32,
                GifOptions {
                    repeat: Some(0),
                    dither,
                },
            )
            .unwrap();
            enc.add_frame(&src, 4).unwrap();
            let anim = decode_animation(&enc.finish()).unwrap();
            let got = &anim.frames[0];
            assert!(got.palette.len() <= 256);
            let err: f64 = got
                .rgba
                .iter()
                .zip(&src)
                .map(|(a, b)| f64::from((i32::from(*a) - i32::from(*b)).abs()))
                .sum::<f64>()
                / (w * h * 3) as f64;
            assert!(err < 6.0, "mean error {err} (dither {dither})");
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(GifEncoder::new(0, 10, GifOptions::default()).is_err());
        assert!(GifEncoder::new(70_000, 10, GifOptions::default()).is_err());
        let mut enc = GifEncoder::new(2, 2, GifOptions::default()).unwrap();
        assert_eq!(enc.add_frame(&[0; 3], 1), Err(GifError::FrameSize(3)));
        assert!(decode_animation(b"not a gif").is_none());
    }
}

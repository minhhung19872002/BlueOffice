//! Page background fill effects (Design › Page Color › Fill Effects): the VML fill of the
//! page background (ECMA-376 Part 1, §17.2.1 background; Part 4, §19.1.2.2 background,
//! §19.1.2.5 fill) resolved into what the display list draws.

use bo_model::{Color, Document, PageFill, PageFillKind};
use serde::Serialize;

/// A page background fill ready to draw over the whole page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PageFillPaint {
    /// Linear gradient across the page along `angle` (degrees, VML convention: 0 = top to
    /// bottom, -90 = left to right); `stops` = (offset 0..=1 along the page's extent in that
    /// direction, colour).
    Linear {
        /// VML angle in degrees.
        angle: f32,
        /// Colour stops.
        stops: Vec<(f32, Color)>,
    },
    /// Rectangular gradient around a focus point (fractions of the page): offset 0 at the
    /// point, 1 at the farthest page edge, constant along concentric rectangles.
    Box {
        /// Focus x as a fraction of the page width.
        fx: f32,
        /// Focus y as a fraction of the page height.
        fy: f32,
        /// Colour stops.
        stops: Vec<(f32, Color)>,
    },
    /// A picture repeated at `w` × `h` points from the page's top-left corner.
    Tile {
        /// Media part of the picture.
        media: String,
        /// Tile width (points).
        w: f32,
        /// Tile height (points).
        h: f32,
    },
    /// Two-colour 8 × 8 pattern (set bits `fg`); `mix` is its average colour.
    Pattern {
        /// Tile rows, top to bottom; bit 7 = left column.
        tile: [u8; 8],
        /// Foreground (the fill colour).
        fg: Color,
        /// Background (`color2`).
        bg: Color,
        /// Average colour.
        mix: Color,
    },
    /// A picture stretched over the page.
    Picture {
        /// Media part of the picture.
        media: String,
    },
}

/// Colour stops of a VML gradient: the colours from the fill colour to `color2` (or the
/// preset `colors`) placed by `focus` — 100 % keeps them in order, 0 % reverses them,
/// in between they run to the focus and mirror back, a negative focus swaps the ends
/// (ECMA-376 Part 4, §19.1.2.5 focus).
#[must_use]
pub fn gradient_stops(fill: &PageFill, color: Color) -> Vec<(f32, Color)> {
    let mut base: Vec<(f32, Color)> = if fill.stops.len() >= 2 {
        fill.stops
            .iter()
            .map(|(p, c)| (*p as f32 / 1000.0, *c))
            .collect()
    } else {
        vec![(0.0, color), (1.0, fill.second())]
    };
    if fill.focus < 0 {
        base = base.iter().rev().map(|(t, c)| (1.0 - t, *c)).collect();
    }
    let g = (fill.focus.unsigned_abs() as f32 / 100.0).clamp(0.0, 1.0);
    let reversed = || base.iter().rev().map(|(t, c)| (1.0 - t, *c)).collect();
    if g >= 0.999 {
        base
    } else if g <= 0.001 {
        reversed()
    } else {
        let mut out: Vec<(f32, Color)> = base.iter().map(|(t, c)| (t * g, *c)).collect();
        out.extend(
            base.iter()
                .rev()
                .skip(1)
                .map(|(t, c)| (g + (1.0 - t) * (1.0 - g), *c)),
        );
        out
    }
}

/// Pixel size of a PNG, JPEG, GIF or BMP picture.
#[must_use]
pub fn image_pixels(b: &[u8]) -> Option<(u32, u32)> {
    let be32 = |i: usize| Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?));
    let le16 = |i: usize| {
        Some(u32::from(u16::from_le_bytes(
            b.get(i..i + 2)?.try_into().ok()?,
        )))
    };
    let le32 = |i: usize| Some(i32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
    if b.starts_with(b"\x89PNG") {
        return Some((be32(16)?, be32(20)?));
    }
    if b.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if b.starts_with(b"BM") {
        return Some((le32(18)?.unsigned_abs(), le32(22)?.unsigned_abs()));
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        // T.81 §B.2.2: the first SOFn frame header.
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                return None;
            }
            let m = b[i + 1];
            if m == 0xFF {
                i += 1;
                continue;
            }
            let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
            if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                let h = u32::from(u16::from_be_bytes([b[i + 5], b[i + 6]]));
                let w = u32::from(u16::from_be_bytes([b[i + 7], b[i + 8]]));
                return Some((w, h));
            }
            i += 2 + len;
        }
    }
    None
}

/// Size in points of a picture's pixels: its resolution when it states one (PNG `pHYs`,
/// JFIF density, BMP pixels per metre), else 96 dpi.
#[must_use]
pub fn image_points(b: &[u8]) -> Option<(f32, f32)> {
    let (w, h) = image_pixels(b)?;
    let dpi = image_dpi(b)
        .filter(|d| (24.0..=2400.0).contains(d))
        .unwrap_or(96.0);
    Some((w as f32 * 72.0 / dpi, h as f32 * 72.0 / dpi))
}

fn image_dpi(b: &[u8]) -> Option<f32> {
    if b.starts_with(b"\x89PNG") {
        let mut i = 8;
        while i + 12 <= b.len() {
            let len = u32::from_be_bytes(b[i..i + 4].try_into().ok()?) as usize;
            let ty = &b[i + 4..i + 8];
            if ty == b"pHYs" && len >= 9 {
                let d = b.get(i + 8..i + 17)?;
                let x = u32::from_be_bytes(d[0..4].try_into().ok()?);
                return (d[8] == 1).then_some(x as f32 * 0.0254);
            }
            if ty == b"IDAT" {
                return None;
            }
            i += 12 + len;
        }
        return None;
    }
    if b.starts_with(&[0xFF, 0xD8]) && b.get(6..11) == Some(b"JFIF\0") {
        let unit = *b.get(13)?;
        let x = f32::from(u16::from_be_bytes([*b.get(14)?, *b.get(15)?]));
        return match unit {
            1 => Some(x),
            2 => Some(x * 2.54),
            _ => None,
        };
    }
    if b.starts_with(b"BM") {
        let x = i32::from_le_bytes(b.get(38..42)?.try_into().ok()?);
        return (x > 0).then_some(x as f32 * 0.0254);
    }
    None
}

/// The 8 × 8 bit tile of a pattern fill's bitmap (a two-colour GIF or BMP, as desktop word
/// processors store VML pattern fills): bit set = pixel darker than the mean, i.e. the
/// pattern (fill colour) pixels.
#[must_use]
pub fn pattern_tile(b: &[u8]) -> Option<[u8; 8]> {
    let (w, h, px) = if b.starts_with(b"GIF8") {
        decode_gif(b)?
    } else if b.starts_with(b"BM") {
        decode_bmp(b)?
    } else {
        return None;
    };
    if w == 0 || h == 0 {
        return None;
    }
    let mean = px.iter().map(|&v| u32::from(v)).sum::<u32>() / px.len() as u32;
    let mut tile = [0u8; 8];
    for (y, row) in tile.iter_mut().enumerate() {
        for x in 0..8 {
            let v = px[(y % h) * w + (x % w)];
            if u32::from(v) < mean || (u32::from(v) == mean && v < 128) {
                *row |= 0x80 >> x;
            }
        }
    }
    Some(tile)
}

/// Luminance pixels of a GIF's first image (GIF89a: LZW, global / local colour table).
fn decode_gif(b: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let le = |i: usize| -> Option<usize> {
        Some(usize::from(u16::from_le_bytes([
            *b.get(i)?,
            *b.get(i + 1)?,
        ])))
    };
    let flags = *b.get(10)?;
    let mut i = 13;
    let mut palette: Vec<u8> = Vec::new();
    let table = |at: usize, f: u8| -> Option<(Vec<u8>, usize)> {
        let n = 2usize << (f & 7);
        let raw = b.get(at..at + 3 * n)?;
        Some((
            raw.chunks(3)
                .map(|c| {
                    ((u32::from(c[0]) * 30 + u32::from(c[1]) * 59 + u32::from(c[2]) * 11) / 100)
                        as u8
                })
                .collect(),
            3 * n,
        ))
    };
    if flags & 0x80 != 0 {
        let (p, n) = table(i, flags)?;
        palette = p;
        i += n;
    }
    loop {
        match *b.get(i)? {
            0x21 => {
                // Extension: label, then sub-blocks.
                i += 2;
                while *b.get(i)? != 0 {
                    i += 1 + usize::from(b[i]);
                }
                i += 1;
            }
            0x2C => {
                let (w, h) = (le(i + 5)?, le(i + 7)?);
                let lf = *b.get(i + 9)?;
                i += 10;
                if lf & 0x80 != 0 {
                    let (p, n) = table(i, lf)?;
                    palette = p;
                    i += n;
                }
                let min = usize::from(*b.get(i)?);
                i += 1;
                let mut data = Vec::new();
                while *b.get(i)? != 0 {
                    let n = usize::from(b[i]);
                    data.extend_from_slice(b.get(i + 1..i + 1 + n)?);
                    i += 1 + n;
                }
                let idx = lzw(&data, min, w * h)?;
                let px = idx
                    .iter()
                    .map(|&k| palette.get(usize::from(k)).copied().unwrap_or(0))
                    .collect();
                return Some((w, h, px));
            }
            _ => return None,
        }
    }
}

/// GIF LZW decoding (variable code size, little-endian bit order).
fn lzw(data: &[u8], min: usize, limit: usize) -> Option<Vec<u8>> {
    if !(2..=11).contains(&min) {
        return None;
    }
    let clear = 1usize << min;
    let end = clear + 1;
    let mut dict: Vec<Vec<u8>> = Vec::new();
    let reset = |d: &mut Vec<Vec<u8>>| {
        d.clear();
        d.extend((0..clear).map(|k| vec![k as u8]));
        d.push(Vec::new());
        d.push(Vec::new());
    };
    reset(&mut dict);
    let mut size = min + 1;
    let (mut acc, mut bits, mut pos) = (0u32, 0usize, 0usize);
    let mut prev: Option<usize> = None;
    let mut out = Vec::new();
    while out.len() < limit {
        while bits < size {
            let Some(&byte) = data.get(pos) else {
                return Some(out);
            };
            acc |= u32::from(byte) << bits;
            bits += 8;
            pos += 1;
        }
        let code = (acc & ((1 << size) - 1)) as usize;
        acc >>= size;
        bits -= size;
        if code == clear {
            reset(&mut dict);
            size = min + 1;
            prev = None;
            continue;
        }
        if code == end {
            break;
        }
        let entry = if code < dict.len() {
            dict[code].clone()
        } else {
            let p = dict.get(prev?)?.clone();
            let mut e = p.clone();
            e.push(*p.first()?);
            e
        };
        if let Some(p) = prev {
            let mut n = dict.get(p)?.clone();
            n.push(*entry.first()?);
            dict.push(n);
            if dict.len() == (1 << size) && size < 12 {
                size += 1;
            }
        }
        out.extend_from_slice(&entry);
        prev = Some(code);
    }
    Some(out)
}

/// Luminance pixels of an uncompressed 1/4/8/24/32-bit BMP.
fn decode_bmp(b: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let u32le =
        |i: usize| -> Option<u32> { Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?)) };
    let off = u32le(10)? as usize;
    let hdr = u32le(14)? as usize;
    let w = i32::from_le_bytes(b.get(18..22)?.try_into().ok()?);
    let h = i32::from_le_bytes(b.get(22..26)?.try_into().ok()?);
    let bpp = usize::from(u16::from_le_bytes(b.get(28..30)?.try_into().ok()?));
    if u32le(30)? != 0 {
        return None;
    }
    let (w, top_down, h) = (w.unsigned_abs() as usize, h < 0, h.unsigned_abs() as usize);
    let lum = |r: u8, g: u8, bl: u8| {
        ((u32::from(r) * 30 + u32::from(g) * 59 + u32::from(bl) * 11) / 100) as u8
    };
    let pal: Vec<u8> = if bpp <= 8 {
        let n = match u32le(46)? {
            0 => 1usize << bpp,
            n => n as usize,
        };
        let at = 14 + hdr;
        b.get(at..at + 4 * n)?
            .chunks(4)
            .map(|c| lum(c[2], c[1], c[0]))
            .collect()
    } else {
        Vec::new()
    };
    let stride = (w * bpp).div_ceil(32) * 4;
    let mut px = vec![0u8; w * h];
    for y in 0..h {
        let row = b.get(off + y * stride..off + (y + 1) * stride)?;
        let dy = if top_down { y } else { h - 1 - y };
        for x in 0..w {
            px[dy * w + x] = match bpp {
                1 => *pal.get(usize::from((row[x / 8] >> (7 - x % 8)) & 1))?,
                4 => *pal.get(usize::from((row[x / 2] >> (4 * (1 - x % 2))) & 15))?,
                8 => *pal.get(usize::from(row[x]))?,
                24 | 32 => {
                    let k = x * bpp / 8;
                    lum(row[k + 2], row[k + 1], row[k])
                }
                _ => return None,
            };
        }
    }
    Some((w, h, px))
}

fn mix(tile: [u8; 8], fg: Color, bg: Color) -> Color {
    let n: u32 = tile.iter().map(|r| r.count_ones()).sum();
    let f = |a: u8, b: u8| ((u32::from(a) * n + u32::from(b) * (64 - n) + 32) / 64) as u8;
    Color {
        r: f(fg.r, bg.r),
        g: f(fg.g, bg.g),
        b: f(fg.b, bg.b),
    }
}

/// The paint of the document's page fill effect, if any (pictures that are missing or
/// cannot be read draw nothing but the page colour).
#[must_use]
pub fn page_fill_paint(doc: &Document) -> Option<PageFillPaint> {
    let fill = doc.design.background_fill.as_ref()?;
    let color = doc.design.background.unwrap_or(Color::WHITE);
    let media = || {
        let m = fill.image.as_ref()?;
        doc.media.get(m).map(|b| (m.clone(), b.as_slice()))
    };
    Some(match fill.kind {
        PageFillKind::Gradient => PageFillPaint::Linear {
            angle: fill.angle as f32,
            stops: gradient_stops(fill, color),
        },
        PageFillKind::GradientRadial => PageFillPaint::Box {
            fx: (fill.focus_position.0 as f32 / 1000.0).clamp(0.0, 1.0),
            fy: (fill.focus_position.1 as f32 / 1000.0).clamp(0.0, 1.0),
            stops: gradient_stops(fill, color),
        },
        PageFillKind::Tile => {
            let (m, b) = media()?;
            let (w, h) = image_points(b)?;
            PageFillPaint::Tile { media: m, w, h }
        }
        PageFillKind::Pattern => {
            let (_, b) = media()?;
            let tile = pattern_tile(b)?;
            let bg = fill.second();
            PageFillPaint::Pattern {
                tile,
                fg: color,
                bg,
                mix: mix(tile, color, bg),
            }
        }
        PageFillKind::Frame => PageFillPaint::Picture { media: media()?.0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(h: &str) -> Color {
        Color::from_hex(h).unwrap_or(Color::BLACK)
    }

    #[test]
    fn focus_places_the_second_colour() {
        let mut f = PageFill {
            color2: Some(c("FFFFFF")),
            focus: 100,
            ..PageFill::default()
        };
        let k = c("000000");
        assert_eq!(gradient_stops(&f, k), vec![(0.0, k), (1.0, c("FFFFFF"))]);
        f.focus = 0;
        assert_eq!(gradient_stops(&f, k), vec![(0.0, c("FFFFFF")), (1.0, k)]);
        f.focus = 50;
        assert_eq!(
            gradient_stops(&f, k),
            vec![(0.0, k), (0.5, c("FFFFFF")), (1.0, k)]
        );
        f.focus = -50;
        assert_eq!(
            gradient_stops(&f, k),
            vec![(0.0, c("FFFFFF")), (0.5, k), (1.0, c("FFFFFF"))]
        );
    }

    #[test]
    fn gif_pattern_bits() {
        // 8 × 8 two-colour GIF: a left-column stripe (black) on white, LZW min code 2.
        let mut px = Vec::new();
        for _ in 0..8 {
            px.extend_from_slice(&[0u8, 1, 1, 1, 1, 1, 1, 1]);
        }
        let gif = tiny_gif(&px);
        assert_eq!(image_pixels(&gif), Some((8, 8)));
        assert_eq!(pattern_tile(&gif), Some([0x80; 8]));
    }

    /// An uncompressed-LZW GIF (a clear code before every pixel keeps the code size at 3).
    fn tiny_gif(idx: &[u8]) -> Vec<u8> {
        let mut g = b"GIF89a".to_vec();
        g.extend_from_slice(&[8, 0, 8, 0, 0x80, 0, 0]);
        g.extend_from_slice(&[0, 0, 0, 255, 255, 255]);
        g.extend_from_slice(&[0x2C, 0, 0, 0, 0, 8, 0, 8, 0, 0, 2]);
        let (mut acc, mut bits, mut bytes) = (0u32, 0, Vec::new());
        let put = |v: u32, acc: &mut u32, bits: &mut u32, out: &mut Vec<u8>| {
            *acc |= v << *bits;
            *bits += 3;
            while *bits >= 8 {
                out.push((*acc & 0xFF) as u8);
                *acc >>= 8;
                *bits -= 8;
            }
        };
        for &k in idx {
            put(4, &mut acc, &mut bits, &mut bytes);
            put(u32::from(k), &mut acc, &mut bits, &mut bytes);
        }
        put(5, &mut acc, &mut bits, &mut bytes);
        if bits > 0 {
            bytes.push((acc & 0xFF) as u8);
        }
        for ch in bytes.chunks(255) {
            g.push(ch.len() as u8);
            g.extend_from_slice(ch);
        }
        g.extend_from_slice(&[0, 0x3B]);
        g
    }
}

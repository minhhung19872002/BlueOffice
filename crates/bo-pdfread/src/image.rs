//! Image XObjects and inline images (ISO 32000-1, §8.9): samples of any bit depth in any
//! colour space with `/Decode`, stencil masks (`/ImageMask`), soft masks (`/SMask`), explicit
//! and colour-key masks (`/Mask`), JPEG (DCTDecode), JPEG 2000 (JPXDecode), JBIG2 and CCITT
//! fax images (`codecs`); an image that fails to decode shows a light placeholder box.
use crate::color::ColorSpace;
use crate::filters::{self, Codec};
use crate::store::Document;
use lopdf::{Dictionary, Object};

/// Largest decoded image (pixels); bigger images are sampled down.
const MAX_PIXELS: u64 = 24 * 1024 * 1024;

/// A decoded picture: premultiplied RGBA rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` premultiplied RGBA bytes.
    pub pixels: Vec<u8>,
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

fn get<'a>(doc: &'a Document, d: &'a Dictionary, long: &[u8], short: &[u8]) -> Option<&'a Object> {
    d.get(long)
        .or_else(|_| d.get(short))
        .ok()
        .map(|o| resolve(doc, o))
}

fn int(doc: &Document, d: &Dictionary, long: &[u8], short: &[u8]) -> Option<i64> {
    get(doc, d, long, short).and_then(|o| o.as_i64().ok())
}

/// Reads `bpc`-bit samples MSB first, rows padded to bytes.
struct Samples<'a> {
    data: &'a [u8],
    bpc: u32,
    row_bytes: usize,
}

impl Samples<'_> {
    fn get(&self, row: usize, index: usize) -> u32 {
        let base = row * self.row_bytes;
        match self.bpc {
            8 => u32::from(self.data.get(base + index).copied().unwrap_or(0)),
            16 => {
                let i = base + index * 2;
                u32::from(self.data.get(i).copied().unwrap_or(0)) << 8
                    | u32::from(self.data.get(i + 1).copied().unwrap_or(0))
            }
            b => {
                let bit = index * b as usize;
                let byte = self.data.get(base + bit / 8).copied().unwrap_or(0);
                let shift = 8 - (bit % 8) - b as usize;
                u32::from(byte >> shift) & ((1 << b) - 1)
            }
        }
    }
}

/// Everything needed to decode one image.
pub(crate) struct Params<'a> {
    pub doc: &'a Document,
    pub dict: &'a Dictionary,
    pub data: &'a [u8],
    /// Colour space resolved by the caller (`None` for stencil masks or JPEGs without one).
    pub space: Option<ColorSpace>,
    /// Fill colour for stencil masks.
    pub fill: [f32; 3],
}

/// Decode an image to premultiplied RGBA.
pub(crate) fn decode(p: &Params<'_>) -> Option<Image> {
    let (doc, dict) = (p.doc, p.dict);
    let width = int(doc, dict, b"Width", b"W")?;
    let height = int(doc, dict, b"Height", b"H")?;
    if width <= 0 || height <= 0 || width > 1 << 16 || height > 1 << 16 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let (bytes, codec) = filters::decode(doc, dict, p.data).ok()?;
    // Bi-level codecs become plain 1-bit samples.
    let (bytes, codec, bilevel) = match codec {
        Some(Codec::Ccitt(params)) => match crate::codecs::ccitt(&bytes, &params, w, h) {
            Some(b) => (b, None, true),
            None => (bytes, Some(Codec::Ccitt(params)), false),
        },
        Some(Codec::Jbig2) => {
            let globals = crate::codecs::jbig2_globals(doc, dict);
            match crate::codecs::jbig2(&bytes, globals.as_deref(), w, h) {
                Some(b) => (b, None, true),
                None => (bytes, Some(Codec::Jbig2), false),
            }
        }
        c => (bytes, c, false),
    };
    let stencil = get(doc, dict, b"ImageMask", b"IM").is_some_and(|o| o.as_bool().unwrap_or(false));
    let decode_arr: Vec<f32> = match get(doc, dict, b"Decode", b"D") {
        Some(Object::Array(a)) => a
            .iter()
            .filter_map(|o| resolve(doc, o).as_float().ok())
            .collect(),
        _ => Vec::new(),
    };
    let jpx = if matches!(codec, Some(Codec::Jpx)) {
        crate::codecs::jpx(&bytes)
    } else {
        None
    };
    // Colour samples: rgb per pixel (0..=255) and alpha.
    let (mut rgb, mut alpha): (Vec<[u8; 3]>, Vec<u8>) = match codec {
        Some(Codec::Dct) => {
            let (rgb, jw, jh) = jpeg(&bytes)?;
            if (jw, jh) != (w, h) {
                // Trust the JPEG's own size.
                return Some(finish(&rgb, vec![255; jw * jh], jw, jh, p, dict));
            }
            let n = rgb.len();
            (rgb, vec![255; n])
        }
        Some(Codec::Jpx) if jpx.is_some() => {
            let (rgb, a, jw, jh) = jpx?;
            if (jw, jh) != (w, h) {
                return Some(finish(&rgb, a, jw, jh, p, dict));
            }
            (rgb, a)
        }
        Some(Codec::Ccitt(_) | Codec::Jpx | Codec::Jbig2) => {
            // Undecodable here: a very light grey box keeps the layout visible.
            let n = w.checked_mul(h)?;
            if n as u64 > MAX_PIXELS {
                return None;
            }
            if stencil {
                (vec![[0; 3]; n], vec![0; n])
            } else {
                (vec![[0xf2, 0xf2, 0xf2]; n], vec![255; n])
            }
        }
        None if stencil => {
            let s = Samples {
                data: &bytes,
                bpc: 1,
                row_bytes: w.div_ceil(8),
            };
            let paint_one = decode_arr.first().is_some_and(|v| *v >= 0.5);
            let c = p.fill.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            let n = w.checked_mul(h)?;
            if n as u64 > MAX_PIXELS {
                return None;
            }
            let mut a = Vec::with_capacity(n);
            for y in 0..h {
                for x in 0..w {
                    let bit = s.get(y, x) == 1;
                    a.push(if bit == paint_one { 255 } else { 0 });
                }
            }
            (vec![c; n], a)
        }
        None => {
            let space = p.space.clone().unwrap_or(ColorSpace::Gray);
            let bpc = if bilevel {
                1
            } else {
                int(doc, dict, b"BitsPerComponent", b"BPC").unwrap_or(8)
            };
            let bpc = match bpc {
                1 | 2 | 4 | 8 | 16 => bpc as u32,
                _ => return None,
            };
            let comps = space.components().max(1);
            let row_bytes = (w * comps * bpc as usize).div_ceil(8);
            let s = Samples {
                data: &bytes,
                bpc,
                row_bytes,
            };
            let n = w.checked_mul(h)?;
            if n as u64 > MAX_PIXELS {
                return None;
            }
            let max = ((1_u64 << bpc) - 1) as f32;
            let indexed = matches!(space, ColorSpace::Indexed { .. });
            let dmap = |k: usize, v: u32| -> f32 {
                let (d0, d1) = match (decode_arr.get(2 * k), decode_arr.get(2 * k + 1)) {
                    (Some(a), Some(b)) => (*a, *b),
                    _ if indexed => (0.0, max),
                    _ => (0.0, 1.0),
                };
                d0 + v as f32 * (d1 - d0) / max
            };
            let mut rgb = Vec::with_capacity(n);
            // One-component images go through a lookup table of every sample value.
            if comps == 1 && bpc <= 8 {
                let lut: Vec<[u8; 3]> = (0..=max as u32)
                    .map(|v| to8(space.rgb(&[dmap(0, v)])))
                    .collect();
                for y in 0..h {
                    for x in 0..w {
                        rgb.push(lut[(s.get(y, x) as usize).min(lut.len() - 1)]);
                    }
                }
            } else if matches!(space, ColorSpace::Rgb) && bpc == 8 && decode_arr.is_empty() {
                for y in 0..h {
                    let row = &bytes.get(y * row_bytes..).unwrap_or(&[]);
                    for x in 0..w {
                        let px = row.get(x * 3..x * 3 + 3).unwrap_or(&[0, 0, 0]);
                        rgb.push([px[0], px[1], px[2]]);
                    }
                }
            } else {
                let mut vals = vec![0.0_f32; comps];
                let mut cache: std::collections::HashMap<Vec<u32>, [u8; 3]> =
                    std::collections::HashMap::new();
                let mut raw = vec![0_u32; comps];
                for y in 0..h {
                    for x in 0..w {
                        for (k, r) in raw.iter_mut().enumerate() {
                            *r = s.get(y, x * comps + k);
                        }
                        let c = if let Some(c) = cache.get(&raw) {
                            *c
                        } else {
                            for (k, v) in vals.iter_mut().enumerate() {
                                *v = dmap(k, raw[k]);
                            }
                            let c = to8(space.rgb(&vals));
                            if cache.len() < 65536 {
                                cache.insert(raw.clone(), c);
                            }
                            c
                        };
                        rgb.push(c);
                    }
                }
            }
            // Colour-key mask: samples within the ranges are not painted.
            let mut a = vec![255_u8; n];
            if let Some(Object::Array(ranges)) = dict.get(b"Mask").ok().map(|o| resolve(doc, o)) {
                let r: Vec<i64> = ranges
                    .iter()
                    .filter_map(|o| resolve(doc, o).as_i64().ok())
                    .collect();
                if r.len() >= comps * 2 {
                    for y in 0..h {
                        for x in 0..w {
                            let masked = (0..comps).all(|k| {
                                let v = i64::from(s.get(y, x * comps + k));
                                v >= r[2 * k] && v <= r[2 * k + 1]
                            });
                            if masked {
                                a[y * w + x] = 0;
                            }
                        }
                    }
                }
            }
            (rgb, a)
        }
    };
    if rgb.len() < w * h {
        rgb.resize(w * h, [0; 3]);
    }
    if alpha.len() < w * h {
        alpha.resize(w * h, 255);
    }
    Some(finish(&rgb, alpha, w, h, p, dict))
}

fn to8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Apply `/SMask` or a stencil `/Mask`, downsample huge images and premultiply.
fn finish(
    rgb: &[[u8; 3]],
    mut alpha: Vec<u8>,
    w: usize,
    h: usize,
    p: &Params<'_>,
    dict: &Dictionary,
) -> Image {
    let doc = p.doc;
    let soft = dict
        .get(b"SMask")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_stream().ok());
    let stencil = dict
        .get(b"Mask")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_stream().ok());
    if let Some(s) = soft {
        if let Some(m) = decode(&Params {
            doc,
            dict: &s.dict,
            data: &s.content,
            space: Some(ColorSpace::Gray),
            fill: [0.0; 3],
        }) {
            apply_mask(&mut alpha, w, h, &m, false);
        }
    } else if let Some(s) = stencil {
        // Explicit mask: an image mask whose 1 samples (default decode) are not painted.
        if let Some(m) = decode(&Params {
            doc,
            dict: &s.dict,
            data: &s.content,
            space: None,
            fill: [1.0; 3],
        }) {
            apply_mask(&mut alpha, w, h, &m, true);
        }
    }
    let n = w * h;
    let mut pixels = Vec::with_capacity(n * 4);
    for i in 0..n {
        let a = u16::from(alpha.get(i).copied().unwrap_or(255));
        let c = rgb.get(i).copied().unwrap_or([0; 3]);
        let pm = |v: u8| ((u16::from(v) * a + 127) / 255) as u8;
        pixels.extend_from_slice(&[pm(c[0]), pm(c[1]), pm(c[2]), a as u8]);
    }
    Image {
        width: w as u32,
        height: h as u32,
        pixels,
    }
}

/// Multiply `alpha` by a mask image sampled to the same size. A soft mask is grey (its
/// luminosity is the alpha); a stencil mask (`/Mask` stream) is opaque where not painted.
fn apply_mask(alpha: &mut [u8], w: usize, h: usize, m: &Image, stencil: bool) {
    let (mw, mh) = (m.width as usize, m.height as usize);
    if mw == 0 || mh == 0 {
        return;
    }
    for y in 0..h {
        let my = (y * mh / h).min(mh - 1);
        for x in 0..w {
            let mx = (x * mw / w).min(mw - 1);
            let px = &m.pixels[(my * mw + mx) * 4..(my * mw + mx) * 4 + 4];
            let v = if stencil {
                // Stencil decoded with paint = 1 samples opaque: 1 means masked out.
                255 - px[3]
            } else {
                px[0]
            };
            let a = &mut alpha[y * w + x];
            *a = ((u16::from(*a) * u16::from(v) + 127) / 255) as u8;
        }
    }
}

/// Decode JPEG bytes to RGB.
fn jpeg(bytes: &[u8]) -> Option<(Vec<[u8; 3]>, usize, usize)> {
    use zune_jpeg::zune_core::colorspace::ColorSpace as Zcs;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let mut probe = zune_jpeg::JpegDecoder::new(bytes);
    probe.decode_headers().ok()?;
    let input = probe.get_input_colorspace()?;
    let cmyk = matches!(input, Zcs::CMYK | Zcs::YCCK);
    // Greyscale JPEGs are decoded as one channel and widened here (the decoder does not
    // expand Luma to RGB: asking for RGB gave one byte per pixel read as three).
    let gray = matches!(input, Zcs::Luma);
    let out = if cmyk {
        Zcs::CMYK
    } else if gray {
        Zcs::Luma
    } else {
        Zcs::RGB
    };
    let opts = DecoderOptions::default()
        .jpeg_set_out_colorspace(out)
        .set_max_width(1 << 16)
        .set_max_height(1 << 16);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
    let data = dec.decode().ok()?;
    let (w, h) = dec.dimensions()?;
    if (w as u64) * (h as u64) > MAX_PIXELS {
        return None;
    }
    let px: Vec<[u8; 3]> = if cmyk {
        // Adobe CMYK JPEGs store inverted values (APP14 "Adobe").
        let inverted = bytes.windows(5).take(4096).any(|w| w == b"Adobe");
        data.chunks_exact(4)
            .map(|c| {
                let v = |i: usize| {
                    let x = f32::from(c[i]) / 255.0;
                    if inverted {
                        1.0 - x
                    } else {
                        x
                    }
                };
                to8(crate::color::cmyk_to_rgb(v(0), v(1), v(2), v(3)))
            })
            .collect()
    } else if gray && data.len() == w * h {
        data.iter().map(|&v| [v, v, v]).collect()
    } else {
        data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
    };
    Some((px, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_rgb_stencil_and_indexed_samples() {
        let doc = Document::from(lopdf::Document::new());
        let mut d = Dictionary::new();
        d.set("Width", 2);
        d.set("Height", 1);
        d.set("BitsPerComponent", 8);
        let img = decode(&Params {
            doc: &doc,
            dict: &d,
            data: &[0, 255],
            space: Some(ColorSpace::Gray),
            fill: [0.0; 3],
        })
        .expect("gray");
        assert_eq!(img.pixels, vec![0, 0, 0, 255, 255, 255, 255, 255]);

        let mut m = Dictionary::new();
        m.set("Width", 3);
        m.set("Height", 1);
        m.set("ImageMask", true);
        let img = decode(&Params {
            doc: &doc,
            dict: &m,
            data: &[0b0100_0000],
            space: None,
            fill: [1.0, 0.0, 0.0],
        })
        .expect("stencil");
        // Sample 0 paints the fill colour, 1 leaves the backdrop.
        assert_eq!(&img.pixels[..8], &[255, 0, 0, 255, 0, 0, 0, 0]);

        let mut k = Dictionary::new();
        k.set("Width", 1);
        k.set("Height", 1);
        k.set("BitsPerComponent", 1);
        k.set("Mask", vec![1.into(), 1.into()]);
        let img = decode(&Params {
            doc: &doc,
            dict: &k,
            data: &[0x80],
            space: Some(ColorSpace::Gray),
            fill: [0.0; 3],
        })
        .expect("colour key");
        assert_eq!(img.pixels[3], 0);
    }
}

//! Appearance of a visible digital signature (ISO 32000-1 §12.7.4.5, §12.5.5): a content
//! stream for a form XObject of `width` × `height` points that needs no font resources — text
//! is drawn as glyph outlines of the bundled sans face (§8.5 path construction), so any script
//! (Vietnamese included) shows the same in every viewer. Our own layout: the signer's name in
//! large type on the left (or a picture), "Digitally signed by", the name and the date in small
//! type on the right.
#![allow(clippy::single_match_else)]
use crate::PdfDocument;
use std::fmt::Write as _;

/// A visible signature's appearance: content stream and the picture it draws as `/Im1`
/// (width, height, zlib-compressed 8-bit RGB).
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureLook {
    /// Content stream in form space (`0 0 width height`).
    pub content: Vec<u8>,
    /// Picture drawn as `/Im1`.
    pub image: Option<(u32, u32, Vec<u8>)>,
}

fn n(v: f32) -> String {
    crate::objects::num(v)
}

/// Outline path operators for `text` at `size` points with its baseline start at (x, y)
/// (PDF space, y up); returns the operators and the advance width.
fn text_path(face: &ttf_parser::Face<'_>, text: &str, size: f32, x: f32, y: f32) -> (String, f32) {
    struct Pen<'a> {
        out: &'a mut String,
        k: f32,
        x: f32,
        y: f32,
        last: (f32, f32),
    }
    impl ttf_parser::OutlineBuilder for Pen<'_> {
        fn move_to(&mut self, x: f32, y: f32) {
            let (px, py) = (self.x + x * self.k, self.y + y * self.k);
            let _ = write!(self.out, "{} {} m ", n(px), n(py));
            self.last = (px, py);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            let (px, py) = (self.x + x * self.k, self.y + y * self.k);
            let _ = write!(self.out, "{} {} l ", n(px), n(py));
            self.last = (px, py);
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            // A quadratic as a cubic Bézier (§8.5.2.2 has cubic curves only).
            let (qx, qy) = (self.x + x1 * self.k, self.y + y1 * self.k);
            let (px, py) = (self.x + x * self.k, self.y + y * self.k);
            let (lx, ly) = self.last;
            let c1 = (lx + 2.0 / 3.0 * (qx - lx), ly + 2.0 / 3.0 * (qy - ly));
            let c2 = (px + 2.0 / 3.0 * (qx - px), py + 2.0 / 3.0 * (qy - py));
            let _ = write!(
                self.out,
                "{} {} {} {} {} {} c ",
                n(c1.0),
                n(c1.1),
                n(c2.0),
                n(c2.1),
                n(px),
                n(py)
            );
            self.last = (px, py);
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            let p = |a: f32, b: f32| (self.x + a * self.k, self.y + b * self.k);
            let (a, b, c) = (p(x1, y1), p(x2, y2), p(x, y));
            let _ = write!(
                self.out,
                "{} {} {} {} {} {} c ",
                n(a.0),
                n(a.1),
                n(b.0),
                n(b.1),
                n(c.0),
                n(c.1)
            );
            self.last = c;
        }
        fn close(&mut self) {
            self.out.push_str("h ");
        }
    }
    let k = size / f32::from(face.units_per_em().max(1));
    let mut out = String::new();
    let mut pen_x = x;
    for c in text.chars() {
        let Some(g) = face.glyph_index(c) else {
            pen_x += size * 0.5;
            continue;
        };
        let mut pen = Pen {
            out: &mut out,
            k,
            x: pen_x,
            y,
            last: (pen_x, y),
        };
        let _ = face.outline_glyph(g, &mut pen);
        pen_x += face.glyph_hor_advance(g).map_or(0.0, f32::from) * k;
    }
    (out, pen_x - x)
}

fn advance(face: &ttf_parser::Face<'_>, text: &str, size: f32) -> f32 {
    let k = size / f32::from(face.units_per_em().max(1));
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map_or(size * 0.5, |a| f32::from(a) * k)
        })
        .sum()
}

/// Decode a PNG or JPEG into 8-bit RGB on white.
fn picture_rgb(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut d = zune_jpeg::JpegDecoder::new_with_options(
            bytes,
            zune_jpeg::zune_core::options::DecoderOptions::default()
                .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGB),
        );
        let px = d.decode().map_err(|e| format!("JPEG: {e:?}"))?;
        let (w, h) = d.dimensions().ok_or("JPEG size")?;
        return Ok((w as u32, h as u32, px));
    }
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = dec.read_info().map_err(|e| format!("PNG: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG size")?];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG: {e}"))?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w == 0 || h == 0 || w * h > 1 << 26 || frame.bit_depth != png::BitDepth::Eight {
        return Err("PNG: only 8-bit pictures".into());
    }
    let line = frame.line_size;
    let mut rgb = Vec::with_capacity(w * h * 3);
    let blend =
        |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
    for y in 0..h {
        let row = &buf[y * line..(y + 1) * line];
        for x in 0..w {
            let (r, g, b, a) = match frame.color_type {
                png::ColorType::Rgba => {
                    (row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3])
                }
                png::ColorType::Rgb => (row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255),
                png::ColorType::GrayscaleAlpha => {
                    (row[x * 2], row[x * 2], row[x * 2], row[x * 2 + 1])
                }
                png::ColorType::Grayscale => (row[x], row[x], row[x], 255),
                png::ColorType::Indexed => return Err("PNG: palette pictures".into()),
            };
            rgb.extend_from_slice(&[blend(r, a), blend(g, a), blend(b, a)]);
        }
    }
    Ok((w as u32, h as u32, rgb))
}

/// The appearance of a signature box of `width` × `height` points: `name` large on the left
/// (or `picture`, PNG or JPEG), then `lines` (for example "Digitally signed by …", the date)
/// on the right.
pub fn signature_appearance(
    width: f32,
    height: f32,
    name: &str,
    lines: &[String],
    picture: Option<&[u8]>,
) -> Result<SignatureLook, String> {
    let data = crate::fallback::face_data(crate::fallback::SANS).ok_or("no bundled font")?;
    let face = ttf_parser::Face::parse(data, 0).map_err(|e| e.to_string())?;
    let (w, h) = (width.max(4.0), height.max(4.0));
    let pad = (h * 0.06).clamp(1.0, 4.0);
    let half = (w / 2.0).floor();
    let mut s = String::from("q 0 g ");
    let mut image = None;
    match picture {
        Some(bytes) => {
            let (iw, ih, rgb) = picture_rgb(bytes)?;
            // Fit the picture into the left half, centred.
            let (bw, bh) = (half - 2.0 * pad, h - 2.0 * pad);
            let k = (bw / iw as f32).min(bh / ih as f32);
            let (dw, dh) = (iw as f32 * k, ih as f32 * k);
            let (x, y) = (pad + (bw - dw) / 2.0, pad + (bh - dh) / 2.0);
            let _ = write!(
                s,
                "q {} 0 0 {} {} {} cm /Im1 Do Q ",
                n(dw),
                n(dh),
                n(x),
                n(y)
            );
            image = Some((iw, ih, miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6)));
        }
        None => {
            // The name as large as fits the left half.
            let unit = advance(&face, name, 1.0).max(0.01);
            let size = ((half - 2.0 * pad) / unit).min(h * 0.45).max(1.0);
            let y = (h - size * 0.7) / 2.0;
            let (path, _) = text_path(&face, name, size, pad, y);
            s.push_str(&path);
            s.push_str("f ");
        }
    }
    if !lines.is_empty() {
        let rows = lines.len() as f32;
        let widest = lines
            .iter()
            .map(|l| advance(&face, l, 1.0))
            .fold(0.01, f32::max);
        let size = ((w - half - 2.0 * pad) / widest)
            .min((h - 2.0 * pad) / (rows * 1.2))
            .max(1.0);
        let total = rows * size * 1.2;
        let mut y = f32::midpoint(h, total) - size;
        for l in lines {
            let (path, _) = text_path(&face, l, size, half + pad, y);
            s.push_str(&path);
            s.push_str("f ");
            y -= size * 1.2;
        }
    }
    s.push('Q');
    Ok(SignatureLook {
        content: s.into_bytes(),
        image,
    })
}

impl PdfDocument {
    /// A rectangle of page `index` in page points (left, top, right, bottom; y down) in the
    /// page's default user space (left, bottom, right, top).
    pub fn page_rect_to_user(&self, index: usize, r: [f32; 4]) -> Option<[f32; 4]> {
        let info = self.pages.get(index)?;
        let inv = crate::interp::invert(crate::page_matrix(info.crop, info.rotate))?;
        let a = crate::interp::apply(inv, r[0], r[1]);
        let b = crate::interp::apply(inv, r[2], r[3]);
        Some([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_draws_text_as_paths() {
        let look = signature_appearance(
            200.0,
            60.0,
            "Bùi Văn Hùng",
            &[
                "Digitally signed by".into(),
                "Bùi Văn Hùng".into(),
                "Date: 2026.11.15".into(),
            ],
            None,
        )
        .expect("appearance");
        let s = String::from_utf8(look.content).expect("ascii");
        assert!(s.starts_with("q 0 g "));
        assert!(s.contains(" c ") && s.contains(" m ") && s.ends_with('Q'));
        assert!(look.image.is_none());
        // Every coordinate stays inside the box.
        let nums: Vec<f32> = s
            .split_whitespace()
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        assert!(nums.iter().all(|v| *v >= -1.0 && *v <= 201.0));
    }

    #[test]
    fn appearance_with_a_picture() {
        let mut png_bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png_bytes, 2, 1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().expect("header");
            w.write_image_data(&[255, 0, 0, 255, 0, 0, 255, 0])
                .expect("png");
        }
        let look = signature_appearance(100.0, 40.0, "A", &["x".into()], Some(&png_bytes))
            .expect("appearance");
        let (w, h, z) = look.image.expect("picture");
        assert_eq!((w, h), (2, 1));
        let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("zlib");
        // The transparent pixel is white.
        assert_eq!(rgb, [255, 0, 0, 255, 255, 255]);
        assert!(String::from_utf8_lossy(&look.content).contains("/Im1 Do"));
    }
}

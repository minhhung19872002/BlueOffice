//! Baseline JPEG encoder written from ITU-T T.81 (ISO/IEC 10918-1): sequential DCT,
//! 8-bit samples, Huffman coding with the example tables of Annex K.3, quantization
//! tables of Annex K.1 scaled by a quality factor, JFIF 1.02 YCbCr (full range,
//! no chroma subsampling). Used by "Compress pictures" (CORE-PIC-001).

use crate::codecs::Rgba;

/// Zig-zag order: natural (row-major) index of each zig-zag position (T.81 Figure A.6).
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// T.81 Table K.1: luminance quantization table (natural order).
const Q_LUMA: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// T.81 Table K.2: chrominance quantization table (natural order).
const Q_CHROMA: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

// T.81 Table K.3–K.6: code lengths (BITS) and values (HUFFVAL).
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// Huffman code table: (code, length) by symbol value (T.81 Annex C).
struct Huff {
    codes: [(u16, u8); 256],
}

impl Huff {
    fn new(bits: &[u8; 16], values: &[u8]) -> Huff {
        let mut codes = [(0u16, 0u8); 256];
        let mut code: u16 = 0;
        let mut k = 0;
        for (i, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                if let Some(&v) = values.get(k) {
                    codes[usize::from(v)] = (code, i as u8 + 1);
                }
                code = code.wrapping_add(1);
                k += 1;
            }
            code <<= 1;
        }
        Huff { codes }
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (code & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            // T.81 §F.1.2.3: a 0xFF data byte is followed by a stuffed zero byte.
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
            self.acc &= (1 << self.n) - 1;
        }
    }

    fn flush(&mut self) {
        // Pad with 1-bits (T.81 §F.1.2.3).
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
    }
}

/// Quantization table for `quality` (1..=100); the usual linear scaling of the
/// Annex K tables (50 = the tables as printed, 100 = all ones).
fn scaled(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = u32::from(quality.clamp(1, 100));
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    base.map(|v| ((u32::from(v) * scale + 50) / 100).clamp(1, 255) as u16)
}

/// Forward DCT of an 8×8 block (T.81 §A.3.3), separable form.
fn fdct(block: &[f32; 64]) -> [f32; 64] {
    let mut cos = [[0f32; 8]; 8];
    for (x, row) in cos.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            *c = ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    let cu = |u: usize| {
        if u == 0 {
            std::f32::consts::FRAC_1_SQRT_2
        } else {
            1.0
        }
    };
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += block[y * 8 + x] * cos[x][u];
            }
            tmp[y * 8 + u] = s * cu(u) / 2.0;
        }
    }
    let mut out = [0f32; 64];
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += tmp[y * 8 + u] * cos[y][v];
            }
            out[v * 8 + u] = s * cu(v) / 2.0;
        }
    }
    out
}

/// Number of bits of a coefficient magnitude (T.81 Table F.1 / F.2 category).
fn category(v: i32) -> u32 {
    32 - v.unsigned_abs().leading_zeros()
}

fn encode_block(
    w: &mut BitWriter,
    coef: &[f32; 64],
    q: &[u16; 64],
    prev_dc: &mut i32,
    dc: &Huff,
    ac: &Huff,
) {
    let mut zz = [0i32; 64];
    for (i, &n) in ZIGZAG.iter().enumerate() {
        zz[i] = (coef[n] / f32::from(q[n])).round() as i32;
    }
    // DC difference (T.81 §F.1.2.1).
    let diff = zz[0] - *prev_dc;
    *prev_dc = zz[0];
    let cat = category(diff);
    let (code, len) = dc.codes[cat as usize];
    w.put(u32::from(code), u32::from(len));
    let bits = if diff < 0 { diff - 1 } else { diff };
    w.put(bits as u32, cat);
    // AC coefficients with run lengths (T.81 §F.1.2.2).
    let mut run = 0u32;
    for &v in &zz[1..] {
        if v == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            let (c, l) = ac.codes[0xF0];
            w.put(u32::from(c), u32::from(l));
            run -= 16;
        }
        let cat = category(v);
        let (c, l) = ac.codes[((run << 4) | cat) as usize];
        w.put(u32::from(c), u32::from(l));
        let bits = if v < 0 { v - 1 } else { v };
        w.put(bits as u32, cat);
        run = 0;
    }
    if run > 0 {
        let (c, l) = ac.codes[0x00];
        w.put(u32::from(c), u32::from(l));
    }
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// Encode straight RGBA pixels (alpha ignored: composite first) as a baseline JPEG
/// with `quality` 1..=100.
pub fn encode(img: &Rgba, quality: u8) -> Option<Vec<u8>> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || w > 65_535 || h > 65_535 || img.pixels.len() < w * h * 4 {
        return None;
    }
    let ql = scaled(&Q_LUMA, quality);
    let qc = scaled(&Q_CHROMA, quality);
    let mut out = vec![0xFF, 0xD8];
    // JFIF APP0: version 1.02, no density units, 1:1 aspect.
    segment(
        &mut out,
        0xE0,
        &[b'J', b'F', b'I', b'F', 0, 1, 2, 0, 0, 1, 0, 1, 0, 0],
    );
    for (id, q) in [(0u8, &ql), (1, &qc)] {
        let mut body = vec![id];
        body.extend(ZIGZAG.iter().map(|&n| q[n] as u8));
        segment(&mut out, 0xDB, &body);
    }
    // SOF0 (T.81 §B.2.2): 8-bit, three components, 1×1 sampling.
    let mut sof = vec![8];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.extend_from_slice(&[3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    // DHT (T.81 §B.2.4.2).
    for (class_id, bits, vals) in [
        (0x00u8, &DC_LUMA_BITS, &DC_VALUES[..]),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES[..]),
        (0x01, &DC_CHROMA_BITS, &DC_VALUES[..]),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES[..]),
    ] {
        let mut body = vec![class_id];
        body.extend_from_slice(bits);
        body.extend_from_slice(vals);
        segment(&mut out, 0xC4, &body);
    }
    // SOS (T.81 §B.2.3).
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let huff = [
        (
            Huff::new(&DC_LUMA_BITS, &DC_VALUES),
            Huff::new(&AC_LUMA_BITS, &AC_LUMA_VALUES),
        ),
        (
            Huff::new(&DC_CHROMA_BITS, &DC_VALUES),
            Huff::new(&AC_CHROMA_BITS, &AC_CHROMA_VALUES),
        ),
    ];
    let mut bw = BitWriter { out, acc: 0, n: 0 };
    let mut prev = [0i32; 3];
    let px = |x: usize, y: usize| {
        let i = (y.min(h - 1) * w + x.min(w - 1)) * 4;
        let p = &img.pixels[i..i + 3];
        (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]))
    };
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            let mut blocks = [[0f32; 64]; 3];
            for y in 0..8 {
                for x in 0..8 {
                    // JFIF 1.02 RGB → YCbCr, level-shifted by 128 (T.81 §A.3.1).
                    let (r, g, b) = px(bx + x, by + y);
                    let yy = 0.299 * r + 0.587 * g + 0.114 * b;
                    let cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    let cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                    blocks[0][y * 8 + x] = yy - 128.0;
                    blocks[1][y * 8 + x] = cb;
                    blocks[2][y * 8 + x] = cr;
                }
            }
            for (c, block) in blocks.iter().enumerate() {
                let (q, (dc, ac)) = if c == 0 {
                    (&ql, &huff[0])
                } else {
                    (&qc, &huff[1])
                };
                encode_block(&mut bw, &fdct(block), q, &mut prev[c], dc, ac);
            }
        }
    }
    bw.flush();
    let mut out = bw.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_complete() {
        let n = |b: &[u8; 16]| b.iter().map(|v| usize::from(*v)).sum::<usize>();
        assert_eq!(n(&AC_LUMA_BITS), AC_LUMA_VALUES.len());
        assert_eq!(n(&AC_CHROMA_BITS), AC_CHROMA_VALUES.len());
        assert_eq!(n(&DC_LUMA_BITS), 12);
        assert_eq!(n(&DC_CHROMA_BITS), 12);
        // Quality 50 = Annex K tables; 100 = all ones.
        assert_eq!(scaled(&Q_LUMA, 50), Q_LUMA);
        assert!(scaled(&Q_LUMA, 100).iter().all(|v| *v == 1));
    }

    #[test]
    fn decodes_back_close_to_the_source() {
        // 19×13 gradient (not a multiple of 8: edge blocks are padded).
        let (w, h) = (19u32, 13u32);
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixels.extend_from_slice(&[(x * 13) as u8, (y * 19) as u8, 128, 255]);
            }
        }
        let img = Rgba {
            width: w,
            height: h,
            pixels,
        };
        let jpg = encode(&img, 95).unwrap();
        assert!(jpg.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(jpg.ends_with(&[0xFF, 0xD9]));
        let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGB);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(&jpg, opts);
        let data = dec.decode().unwrap();
        assert_eq!(dec.dimensions().unwrap(), (19, 13));
        let mut max = 0i32;
        for i in 0..(w * h) as usize {
            for c in 0..3 {
                let d = i32::from(data[i * 3 + c]) - i32::from(img.pixels[i * 4 + c]);
                max = max.max(d.abs());
            }
        }
        assert!(max <= 12, "max error {max}");
    }
}

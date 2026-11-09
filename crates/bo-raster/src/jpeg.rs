//! Baseline JPEG encoder written from ITU-T T.81 (ISO/IEC 10918-1):
//! sequential DCT, Huffman coding (§F.1.2), 8-bit samples, three components
//! without subsampling (4:4:4), the example quantization tables of Annex K.1
//! and the typical Huffman tables of Annex K.3. The file is wrapped as JFIF
//! (JPEG File Interchange Format 1.02: APP0 marker, YCbCr per ITU-R BT.601,
//! full range).
//!
//! Quality 1..=100 scales the Annex K tables with the usual linear rule
//! (`q < 50`: 5000 / q %, otherwise 200 − 2q %), which is our own choice of
//! mapping — T.81 does not define a "quality".

/// Zig-zag order: index in the zig-zag sequence → natural (row-major) index (T.81 Figure A.6).
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Luminance quantization table, natural order (T.81 Table K.1).
const LUMA_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Chrominance quantization table, natural order (T.81 Table K.2).
const CHROMA_Q: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

// Typical Huffman tables (T.81 Annex K.3): BITS (codes of each length 1..16) and HUFFVAL.
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALS: [u8; 162] = [
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
const AC_CHROMA_VALS: [u8; 162] = [
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

/// Huffman code table: (code, length) per symbol value (T.81 Annex C).
struct Huff {
    code: [u16; 256],
    len: [u8; 256],
}

impl Huff {
    /// Generate the codes from BITS / HUFFVAL (T.81 §C.2, Figures C.1–C.3).
    fn new(bits: &[u8; 16], vals: &[u8]) -> Self {
        let mut h = Huff {
            code: [0; 256],
            len: [0; 256],
        };
        let mut code: u16 = 0;
        let mut k = 0;
        for (i, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                if let Some(&v) = vals.get(k) {
                    h.code[usize::from(v)] = code;
                    h.len[usize::from(v)] = (i + 1) as u8;
                }
                code = code.wrapping_add(1);
                k += 1;
            }
            code <<= 1;
        }
        h
    }
}

/// Entropy-coded segment writer with 0xFF byte stuffing (T.81 §B.1.1.5).
struct Bits<'a> {
    out: &'a mut Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits<'_> {
    fn put(&mut self, value: u16, len: u8) {
        if len == 0 {
            return;
        }
        let len = u32::from(len);
        self.acc = (self.acc << len) | (u32::from(value) & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            let b = ((self.acc >> (self.n - 8)) & 0xFF) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1 << self.n) - 1;
    }

    /// Pad the last byte with 1-bits (T.81 §F.1.2.3).
    fn flush(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad as u8);
        }
    }
}

/// Quantization table scaled for `quality`, natural order, entries in 1..=255.
fn scaled(table: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = u32::from(quality.clamp(1, 100));
    let s = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let mut out = [0u16; 64];
    for (o, &t) in out.iter_mut().zip(table) {
        *o = ((u32::from(t) * s + 50) / 100).clamp(1, 255) as u16;
    }
    out
}

/// Forward DCT of a level-shifted 8×8 block (T.81 §A.3.3), natural order.
fn fdct(block: &[f32; 64], cos: &[[f32; 8]; 8]) -> [f32; 64] {
    let mut tmp = [0f32; 64];
    // Rows: tmp[y][u] = Σx s(y,x)·cos((2x+1)uπ/16).
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += block[y * 8 + x] * cos[x][u];
            }
            tmp[y * 8 + u] = s;
        }
    }
    let mut out = [0f32; 64];
    for v in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += tmp[y * 8 + u] * cos[y][v];
            }
            let cu = if u == 0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            let cv = if v == 0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            out[v * 8 + u] = 0.25 * cu * cv * s;
        }
    }
    out
}

/// Bit size category of a coefficient difference and its additional bits (T.81 §F.1.2.1).
fn category(v: i32) -> (u8, u16) {
    let a = v.unsigned_abs();
    let size = (32 - a.leading_zeros()) as u8;
    let bits = if v < 0 { v - 1 } else { v };
    (size, (bits as u32 & ((1u32 << size) - 1)) as u16)
}

fn encode_block(bits: &mut Bits<'_>, coefs: &[i32; 64], pred: &mut i32, dc: &Huff, ac: &Huff) {
    // DC difference (§F.1.2.1).
    let diff = coefs[0] - *pred;
    *pred = coefs[0];
    let (s, extra) = category(diff);
    bits.put(dc.code[usize::from(s)], dc.len[usize::from(s)]);
    bits.put(extra, s);
    // AC coefficients in zig-zag order with run lengths (§F.1.2.2).
    let mut run = 0u8;
    for &k in &ZIGZAG[1..] {
        let c = coefs[k];
        if c == 0 {
            run += 1;
            continue;
        }
        while run >= 16 {
            bits.put(ac.code[0xF0], ac.len[0xF0]); // ZRL
            run -= 16;
        }
        let (s, extra) = category(c);
        let sym = usize::from((run << 4) | s);
        bits.put(ac.code[sym], ac.len[sym]);
        bits.put(extra, s);
        run = 0;
    }
    if run > 0 {
        bits.put(ac.code[0], ac.len[0]); // EOB
    }
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// Encode straight RGBA pixels (row by row, `width × height × 4` bytes) as a
/// baseline JFIF JPEG. Transparent pixels are composited over white. Returns
/// `None` for an empty or inconsistent image (JPEG dimensions are 16-bit).
pub fn encode_jpeg(rgba: &[u8], width: u32, height: u32, quality: u8) -> Option<Vec<u8>> {
    let (w, h) = (usize::try_from(width).ok()?, usize::try_from(height).ok()?);
    if w == 0 || h == 0 || w > 65_535 || h > 65_535 || rgba.len() != w * h * 4 {
        return None;
    }
    let lq = scaled(&LUMA_Q, quality);
    let cq = scaled(&CHROMA_Q, quality);
    let mut out = Vec::with_capacity(w * h / 4 + 1024);
    out.extend_from_slice(&[0xFF, 0xD8]); // SOI
                                          // APP0 JFIF 1.02, no density units (aspect 1:1), no thumbnail.
    segment(
        &mut out,
        0xE0,
        b"JFIF\0\x01\x02\x00\x00\x01\x00\x01\x00\x00",
    );
    // DQT (§B.2.4.1): 8-bit precision, tables 0 and 1, zig-zag order.
    let mut dqt = Vec::with_capacity(130);
    for (id, t) in [(0u8, &lq), (1u8, &cq)] {
        dqt.push(id);
        dqt.extend(ZIGZAG.iter().map(|&k| t[k] as u8));
    }
    segment(&mut out, 0xDB, &dqt);
    // SOF0 (§B.2.2): baseline, 8-bit, three components 1×1 sampling.
    let mut sof = vec![8];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.push(3);
    sof.extend_from_slice(&[1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    // DHT (§B.2.4.2): class/id, BITS, HUFFVAL.
    let mut dht = Vec::new();
    for (tc, bits, vals) in [
        (0x00u8, &DC_LUMA_BITS, &DC_LUMA_VALS[..]),
        (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS[..]),
        (0x01, &DC_CHROMA_BITS, &DC_CHROMA_VALS[..]),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS[..]),
    ] {
        dht.push(tc);
        dht.extend_from_slice(bits);
        dht.extend_from_slice(vals);
    }
    segment(&mut out, 0xC4, &dht);
    // SOS (§B.2.3): three components, Y uses tables 0/0, chroma 1/1; Ss=0, Se=63, Ah/Al=0.
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dc_l = Huff::new(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_l = Huff::new(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_c = Huff::new(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_c = Huff::new(&AC_CHROMA_BITS, &AC_CHROMA_VALS);
    let mut cos = [[0f32; 8]; 8];
    for (x, row) in cos.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            *c = (((2 * x + 1) * u) as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    let mut bits = Bits {
        out: &mut out,
        acc: 0,
        n: 0,
    };
    let mut pred = [0i32; 3];
    let mut blocks = [[0f32; 64]; 3];
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            for yy in 0..8 {
                // Edge blocks repeat the last row / column.
                let y = (by + yy).min(h - 1);
                for xx in 0..8 {
                    let x = (bx + xx).min(w - 1);
                    let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    let a = f32::from(p[3]) / 255.0;
                    let over = |c: u8| f32::from(c) * a + 255.0 * (1.0 - a);
                    let (r, g, b) = (over(p[0]), over(p[1]), over(p[2]));
                    // JFIF colour conversion (BT.601, full range), level-shifted by 128.
                    let i = yy * 8 + xx;
                    blocks[0][i] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    blocks[1][i] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    blocks[2][i] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                }
            }
            for (c, block) in blocks.iter().enumerate() {
                let q = if c == 0 { &lq } else { &cq };
                let d = fdct(block, &cos);
                let mut coefs = [0i32; 64];
                for k in 0..64 {
                    coefs[k] = (d[k] / f32::from(q[k])).round() as i32;
                }
                let (dc, ac) = if c == 0 {
                    (&dc_l, &ac_l)
                } else {
                    (&dc_c, &ac_c)
                };
                encode_block(&mut bits, &coefs, &mut pred[c], dc, ac);
            }
        }
    }
    bits.flush();
    out.extend_from_slice(&[0xFF, 0xD9]); // EOI
    Some(out)
}

/// [`encode_jpeg`] for a decoded [`crate::codecs::Rgba`] picture (alpha ignored: composite
/// first).
pub fn encode(img: &crate::codecs::Rgba, quality: u8) -> Option<Vec<u8>> {
    encode_jpeg(&img.pixels, img.width, img.height, quality)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> (Vec<u8>, usize, usize) {
        let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGB);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
        let px = dec.decode().expect("decodes");
        let (w, h) = dec.dimensions().expect("dimensions");
        (px, w, h)
    }

    #[test]
    fn categories_follow_table_f1() {
        // T.81 Table F.1: SSSS 0 for 0, 1 for ±1, 2 for ±2..3, 3 for ±4..7.
        assert_eq!(category(0), (0, 0));
        assert_eq!(category(1), (1, 1));
        assert_eq!(category(-1), (1, 0));
        assert_eq!(category(3), (2, 3));
        assert_eq!(category(-3), (2, 0));
        assert_eq!(category(-4), (3, 3));
        assert_eq!(category(255), (8, 255));
    }

    #[test]
    fn huffman_codes_follow_annex_k() {
        // Table K.3: DC luminance category 0 = "00", category 2 = "011", 11 = "111111110".
        let h = Huff::new(&DC_LUMA_BITS, &DC_LUMA_VALS);
        assert_eq!((h.code[0], h.len[0]), (0b00, 2));
        assert_eq!((h.code[2], h.len[2]), (0b011, 3));
        assert_eq!((h.code[11], h.len[11]), (0b1_1111_1110, 9));
        // Table K.5: AC luminance EOB (0/0) = "1010", ZRL (F/0) = "11111111001".
        let a = Huff::new(&AC_LUMA_BITS, &AC_LUMA_VALS);
        assert_eq!((a.code[0], a.len[0]), (0b1010, 4));
        assert_eq!((a.code[0xF0], a.len[0xF0]), (0b111_1111_1001, 11));
    }

    #[test]
    fn quality_scaling() {
        // q = 50 keeps the Annex K tables, q = 100 makes every step 1.
        assert_eq!(scaled(&LUMA_Q, 50), LUMA_Q);
        assert!(scaled(&LUMA_Q, 100).iter().all(|&v| v == 1));
        // q = 25: 200 % of 16 = 32.
        assert_eq!(scaled(&LUMA_Q, 25)[0], 32);
    }

    #[test]
    fn round_trip_flat_colours_within_tolerance() {
        // 21×13 (not a multiple of 8): left half red, right half blue, one
        // transparent pixel (composited over white).
        let (w, h) = (21usize, 13usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let c: [u8; 4] = if x < 10 {
                    [200, 30, 40, 255]
                } else {
                    [20, 60, 220, 255]
                };
                px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        let jpg = encode_jpeg(&px, w as u32, h as u32, 90).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
        assert_eq!(&jpg[jpg.len() - 2..], &[0xFF, 0xD9]);
        assert_eq!(bo_render::image::jpeg_info(&jpg), Some((21, 13, 3)));
        let (rgb, dw, dh) = decode(&jpg);
        assert_eq!((dw, dh), (w, h));
        // Pixels away from the colour edge stay within a small error.
        for (x, want) in [(2usize, [200i32, 30, 40]), (18, [20, 60, 220])] {
            for y in [1usize, 6, 11] {
                let i = (y * w + x) * 3;
                for c in 0..3 {
                    let got = i32::from(rgb[i + c]);
                    assert!(
                        (got - want[c]).abs() <= 8,
                        "({x},{y}) c{c}: {got} vs {}",
                        want[c]
                    );
                }
            }
        }
    }

    #[test]
    fn gradient_round_trip_and_white_composite() {
        let (w, h) = (32usize, 16usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = (x * 8) as u8;
                px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let (rgb, _, _) = decode(&encode_jpeg(&px, w as u32, h as u32, 95).unwrap());
        let mut max_err = 0;
        for y in 0..h {
            for x in 0..w {
                let got = i32::from(rgb[(y * w + x) * 3]);
                max_err = max_err.max((got - (x as i32 * 8)).abs());
            }
        }
        assert!(max_err <= 6, "max error {max_err}");
        // Fully transparent pixels are composited over white.
        let clear = vec![0u8; 8 * 8 * 4];
        let (rgb, _, _) = decode(&encode_jpeg(&clear, 8, 8, 90).unwrap());
        assert!(rgb.iter().all(|&v| v >= 250));
        assert!(encode_jpeg(&px, 0, 16, 90).is_none());
        assert!(encode_jpeg(&px[..10], w as u32, h as u32, 90).is_none());
    }
}

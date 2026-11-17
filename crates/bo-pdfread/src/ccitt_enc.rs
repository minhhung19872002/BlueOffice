//! Encoders for bi-level pictures (BlueEditor round 10, PDF Optimizer › Monochrome Images):
//! CCITT Group 4 (ITU-T T.6 two-dimensional coding with the T.4 run-length code tables;
//! ISO 32000-1 §7.4.6 `CCITTFaxDecode` with `/K -1`) and a JBIG2 page in the embedded
//! organization holding one immediate lossless generic region coded with MMR (ITU-T T.88
//! §7.4.6 generic region segment, §6.2 MMR = T.6; ISO 32000-1 §7.4.7 `JBIG2Decode`). JBIG2's
//! symbol coding (the "lossy" text compression) is not done: both qualities write the lossless
//! MMR region.
//!
//! Pixels are given one byte each, 1 = black.

/// Bits written most significant first.
struct Bits {
    out: Vec<u8>,
    acc: u8,
    n: u8,
}

impl Bits {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }

    /// Append a code given as a string of '0' / '1'.
    fn code(&mut self, s: &str) {
        for b in s.bytes() {
            self.acc = (self.acc << 1) | u8::from(b == b'1');
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc);
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc << (8 - self.n));
        }
        self.out
    }
}

// T.4 Table 2: terminating codes, runs 0–63.
const WHITE_TERM: [&str; 64] = [
    "00110101", "000111", "0111", "1000", "1011", "1100", "1110", "1111", "10011", "10100",
    "00111", "01000", "001000", "000011", "110100", "110101", "101010", "101011", "0100111",
    "0001100", "0001000", "0010111", "0000011", "0000100", "0101000", "0101011", "0010011",
    "0100100", "0011000", "00000010", "00000011", "00011010", "00011011", "00010010", "00010011",
    "00010100", "00010101", "00010110", "00010111", "00101000", "00101001", "00101010", "00101011",
    "00101100", "00101101", "00000100", "00000101", "00001010", "00001011", "01010010", "01010011",
    "01010100", "01010101", "00100100", "00100101", "01011000", "01011001", "01011010", "01011011",
    "01001010", "01001011", "00110010", "00110011", "00110100",
];
const BLACK_TERM: [&str; 64] = [
    "0000110111",
    "010",
    "11",
    "10",
    "011",
    "0011",
    "0010",
    "00011",
    "000101",
    "000100",
    "0000100",
    "0000101",
    "0000111",
    "00000100",
    "00000111",
    "000011000",
    "0000010111",
    "0000011000",
    "0000001000",
    "00001100111",
    "00001101000",
    "00001101100",
    "00000110111",
    "00000101000",
    "00000010111",
    "00000011000",
    "000011001010",
    "000011001011",
    "000011001100",
    "000011001101",
    "000001101000",
    "000001101001",
    "000001101010",
    "000001101011",
    "000011010010",
    "000011010011",
    "000011010100",
    "000011010101",
    "000011010110",
    "000011010111",
    "000001101100",
    "000001101101",
    "000011011010",
    "000011011011",
    "000001010100",
    "000001010101",
    "000001010110",
    "000001010111",
    "000001100100",
    "000001100101",
    "000001010010",
    "000001010011",
    "000000100100",
    "000000110111",
    "000000111000",
    "000000100111",
    "000000101000",
    "000001011000",
    "000001011001",
    "000000101011",
    "000000101100",
    "000001011010",
    "000001100110",
    "000001100111",
];
// T.4 Table 3a: make-up codes 64–1728 (index = run / 64 - 1).
const WHITE_MAKEUP: [&str; 27] = [
    "11011",
    "10010",
    "010111",
    "0110111",
    "00110110",
    "00110111",
    "01100100",
    "01100101",
    "01101000",
    "01100111",
    "011001100",
    "011001101",
    "011010010",
    "011010011",
    "011010100",
    "011010101",
    "011010110",
    "011010111",
    "011011000",
    "011011001",
    "011011010",
    "011011011",
    "010011000",
    "010011001",
    "010011010",
    "011000",
    "010011011",
];
const BLACK_MAKEUP: [&str; 27] = [
    "0000001111",
    "000011001000",
    "000011001001",
    "000001011011",
    "000000110011",
    "000000110100",
    "000000110101",
    "0000001101100",
    "0000001101101",
    "0000001001010",
    "0000001001011",
    "0000001001100",
    "0000001001101",
    "0000001110010",
    "0000001110011",
    "0000001110100",
    "0000001110101",
    "0000001110110",
    "0000001110111",
    "0000001010010",
    "0000001010011",
    "0000001010100",
    "0000001010101",
    "0000001011010",
    "0000001011011",
    "0000001100100",
    "0000001100101",
];
// T.4 Table 3b: extended make-up codes 1792–2560 (both colours).
const EXT_MAKEUP: [&str; 13] = [
    "00000001000",
    "00000001100",
    "00000001101",
    "000000010010",
    "000000010011",
    "000000010100",
    "000000010101",
    "000000010110",
    "000000010111",
    "000000011100",
    "000000011101",
    "000000011110",
    "000000011111",
];

/// A run of `len` pixels of one colour (make-up codes, then a terminating code).
fn run(b: &mut Bits, mut len: usize, black: bool) {
    while len >= 2560 + 64 {
        b.code(EXT_MAKEUP[12]);
        len -= 2560;
    }
    if len >= 64 {
        let m = len / 64;
        if m <= 27 {
            b.code(if black {
                BLACK_MAKEUP[m - 1]
            } else {
                WHITE_MAKEUP[m - 1]
            });
        } else {
            b.code(EXT_MAKEUP[m - 28]);
        }
        len -= m * 64;
    }
    b.code(if black {
        BLACK_TERM[len]
    } else {
        WHITE_TERM[len]
    });
}

/// First changing element of `line` at or after `from` (the pixel before the line is white);
/// `line.len()` when there is none.
fn change(line: &[u8], from: usize) -> usize {
    let w = line.len();
    let mut x = from;
    while x < w {
        let before = if x == 0 { 0 } else { line[x - 1] };
        if line[x] != before {
            return x;
        }
        x += 1;
    }
    w
}

/// T.6 coding of `h` rows of `w` pixels (1 = black), ending with EOFB.
pub(crate) fn g4(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut b = Bits::new();
    let white = vec![0_u8; w];
    let mut reference: &[u8] = &white;
    for y in 0..h {
        let Some(line) = px.get(y * w..(y + 1) * w) else {
            break;
        };
        // a0 starts on the imaginary white pixel before the line.
        let mut a0: Option<usize> = None;
        let mut black = false;
        loop {
            let start = a0.map_or(0, |a| a + 1);
            let a1 = change(line, start);
            let mut b1 = change(reference, start);
            if b1 < w && (reference[b1] == 1) == black {
                b1 = change(reference, b1 + 1);
            }
            let b2 = if b1 < w { change(reference, b1 + 1) } else { w };
            if b2 < a1 {
                // Pass mode.
                b.code("0001");
                a0 = Some(b2);
            } else if a1.abs_diff(b1) <= 3 {
                // Vertical mode.
                b.code(match a1 as isize - b1 as isize {
                    0 => "1",
                    1 => "011",
                    2 => "000011",
                    3 => "0000011",
                    -1 => "010",
                    -2 => "000010",
                    _ => "0000010",
                });
                a0 = Some(a1);
                black = !black;
            } else {
                // Horizontal mode: a0a1 in the colour of a0, a1a2 in the other.
                let a2 = if a1 < w { change(line, a1 + 1) } else { w };
                b.code("001");
                run(&mut b, a1 - a0.unwrap_or(0), black);
                run(&mut b, a2 - a1, !black);
                a0 = Some(a2);
            }
            if a0.is_some_and(|a| a >= w) {
                break;
            }
        }
        reference = line;
    }
    // EOFB: two EOL codes.
    b.code("000000000001000000000001");
    b.finish()
}

/// T.4 one-dimensional coding (Modified Huffman) of `h` rows of `w` pixels, no EOL codes
/// (`/K 0 /EndOfBlock false`).
pub(crate) fn g3_1d(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut b = Bits::new();
    for y in 0..h {
        let Some(line) = px.get(y * w..(y + 1) * w) else {
            break;
        };
        let mut x = 0;
        let mut black = false;
        while x < w {
            let end = if x == 0 && !black {
                change(line, 0)
            } else {
                change(line, x + 1)
            };
            run(&mut b, end - x, black);
            x = end;
            black = !black;
        }
        if w == 0 {
            run(&mut b, 0, false);
        }
    }
    b.finish()
}

fn be32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// Segment header (T.88 §7.2): number, type, no referred-to segments, page 1, data length.
fn segment(out: &mut Vec<u8>, number: u32, kind: u8, data: &[u8]) {
    be32(out, number);
    out.push(kind);
    out.push(0);
    out.push(1);
    be32(out, u32::try_from(data.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(data);
}

/// A JBIG2 page (embedded organization) holding the picture as one MMR generic region.
pub(crate) fn jbig2_mmr(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (w32, h32) = (u32::try_from(w).unwrap_or(0), u32::try_from(h).unwrap_or(0));
    let mut out = Vec::new();
    // Page information (type 48, §7.4.8): size, no resolution, default pixel 0, no striping.
    let mut page = Vec::new();
    be32(&mut page, w32);
    be32(&mut page, h32);
    be32(&mut page, 0);
    be32(&mut page, 0);
    page.push(0);
    page.extend_from_slice(&[0, 0]);
    segment(&mut out, 0, 48, &page);
    // Immediate lossless generic region (type 39, §7.4.6): region information, MMR flag, data.
    let mut region = Vec::new();
    be32(&mut region, w32);
    be32(&mut region, h32);
    be32(&mut region, 0);
    be32(&mut region, 0);
    region.push(0);
    region.push(1);
    region.extend_from_slice(&g4(px, w, h));
    segment(&mut out, 1, 39, &region);
    // End of page (type 49).
    segment(&mut out, 2, 49, &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// Rows with black runs of every length 0..2700 and some 2-D structure.
    fn picture(w: usize, h: usize) -> Vec<u8> {
        let mut px = vec![0_u8; w * h];
        let mut s = 11_u32;
        for y in 0..h {
            let mut x = 0;
            let mut black = y % 3 == 0;
            while x < w {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let len = if y % 7 == 0 {
                    (y * 37 + x) % 2700 + 1
                } else {
                    (s >> 16) as usize % 90 + 1
                };
                for p in px.iter_mut().skip(y * w + x).take(len.min(w - x)) {
                    *p = u8::from(black);
                }
                x += len;
                black = !black;
            }
        }
        px
    }

    /// Unpack decoded rows (1 bit a pixel, 0 = black) to 1 = black bytes.
    fn unpack(rows: &[u8], w: usize, h: usize) -> Vec<u8> {
        let stride = w.div_ceil(8);
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let bit = rows[y * stride + x / 8] >> (7 - x % 8) & 1;
                out.push(1 - bit);
            }
        }
        out
    }

    #[test]
    fn group4_round_trips() {
        for (w, h) in [(1, 1), (7, 3), (3000, 40), (517, 211)] {
            let px = picture(w, h);
            let data = g4(&px, w, h);
            let params = dictionary! { "K" => -1, "Columns" => w as i64, "Rows" => h as i64 };
            let rows = crate::codecs::ccitt(&data, &params, w, h).expect("decodes");
            assert_eq!(unpack(&rows, w, h), px, "{w}x{h}");
        }
        // A white page is a few bytes.
        assert!(g4(&vec![0; 2400 * 3000], 2400, 3000).len() < 1000);
    }

    #[test]
    fn group3_round_trips() {
        for (w, h) in [(9, 2), (3000, 30), (517, 61)] {
            let px = picture(w, h);
            let data = g3_1d(&px, w, h);
            let params = dictionary! {
                "K" => 0, "Columns" => w as i64, "Rows" => h as i64, "EndOfBlock" => false,
            };
            let rows = crate::codecs::ccitt(&data, &params, w, h).expect("decodes");
            assert_eq!(unpack(&rows, w, h), px, "{w}x{h}");
        }
    }

    #[test]
    fn jbig2_mmr_round_trips() {
        let (w, h) = (613, 97);
        let px = picture(w, h);
        let data = jbig2_mmr(&px, w, h);
        let rows = crate::codecs::jbig2(&data, None, w, h).expect("decodes");
        assert_eq!(unpack(&rows, w, h), px);
    }
}
